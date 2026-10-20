//! Drawing editor input: mouse/keyboard handling, selection and dragging,
//! and the view / dimension / annotation tools (with live previews).

use crate::drawing_scene::Overlay;
use crate::navigation as nav;
use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::views::{self, EdgeGeom, SnapKind};
use noshape_model::drawing::*;

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}
fn add2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn norm(a: [f64; 2]) -> [f64; 2] {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l]
}

/// Dimension offset that puts the dimension line through the cursor
/// (mirrors `SheetGeometry::linear_dimension`).
pub fn dim_offset(p: [f64; 2], q: [f64; 2], o: DimOrientation, c: [f64; 2]) -> f64 {
    let nrm = match o {
        DimOrientation::Horizontal => [0.0, 1.0],
        DimOrientation::Vertical => [1.0, 0.0],
        DimOrientation::Aligned => {
            let d = norm(sub(q, p));
            [-d[1], d[0]]
        }
    };
    dot(c, nrm) - dot(p, nrm).max(dot(q, nrm))
}

/// Orientation of a smart linear dimension from the cursor position.
pub(crate) fn auto_orientation(p: [f64; 2], q: [f64; 2], c: [f64; 2]) -> DimOrientation {
    let (x0, x1) = (p[0].min(q[0]), p[0].max(q[0]));
    let (y0, y1) = (p[1].min(q[1]), p[1].max(q[1]));
    let ex = if c[0] < x0 { x0 - c[0] } else if c[0] > x1 { c[0] - x1 } else { 0.0 };
    let ey = if c[1] < y0 { y0 - c[1] } else if c[1] > y1 { c[1] - y1 } else { 0.0 };
    if (x1 - x0) < 1e-6 {
        return DimOrientation::Vertical;
    }
    if (y1 - y0) < 1e-6 {
        return DimOrientation::Horizontal;
    }
    if ex > ey && ex > 0.0 {
        DimOrientation::Vertical
    } else if ey > 0.0 {
        DimOrientation::Horizontal
    } else {
        DimOrientation::Aligned
    }
}

/// Intersection of two infinite 2D lines.
pub(crate) fn intersect(a0: [f64; 2], a1: [f64; 2], b0: [f64; 2], b1: [f64; 2]) -> Option<[f64; 2]> {
    let r = sub(a1, a0);
    let s = sub(b1, b0);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-14 * (len(r) * len(s)).max(1e-30) {
        return None;
    }
    let t = ((b0[0] - a0[0]) * s[1] - (b0[1] - a0[1]) * s[0]) / den;
    Some([a0[0] + r[0] * t, a0[1] + r[1] * t])
}

impl DrawingEditor {
    // ------------------------------------------------------------ tools

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        // Clicking the active tool's button again exits it.
        // Dimension tools exit only when the same kind is chosen again.
        let same_dim = matches!((&tool, &self.tool), (Tool::Dimension { kind: a, .. }, Tool::Dimension { kind: b, .. }) if a == b)
            || matches!((&tool, &self.tool), (Tool::Sketch { tool: a, .. }, Tool::Sketch { tool: b, .. }) if a == b)
            || matches!((&tool, &self.tool), (Tool::Constrain { kind: a, .. }, Tool::Constrain { kind: b, .. }) if a == b);
        if std::mem::discriminant(&tool) == std::mem::discriminant(&self.tool) && (same_dim || !matches!(tool, Tool::Select | Tool::InsertView { .. } | Tool::Dimension { .. } | Tool::Sketch { .. } | Tool::Constrain { .. })) {
            self.tool = Tool::Select;
        } else {
            // A new Insert view starts from the drawing's View simplification default.
            if matches!(tool, Tool::InsertView { .. }) && !matches!(self.tool, Tool::InsertView { .. }) {
                self.insert.simplification = self.drawing.props.view_defaults.simplification;
                self.insert.simp_value = None;
            }
            self.tool = tool;
        }
        // Tools that start from the selected view.
        let sel_view = self.selection.iter().find_map(|s| if let Sel::View(v) = s { Some(*v) } else { None });
        match &mut self.tool {
            Tool::Projected { parent }
            | Tool::Section { parent, .. }
            | Tool::Detail { parent, .. }
            | Tool::JoggedSection { parent, .. }
            | Tool::AlignedSection { parent, .. }
            | Tool::Crop { parent, .. }
            | Tool::Break { parent, .. }
            | Tool::BrokenOut { parent, .. }
            | Tool::CropPoly { parent, .. }
                if parent.is_none() =>
            {
                *parent = sel_view
            }
            Tool::Table { kind: TableTool::Bom | TableTool::Hole | TableTool::CutList | TableTool::Custom, view, .. } if view.is_none() => *view = sel_view,
            _ => {}
        }
        self.overlay = Overlay::default();
        self.note_leader = None;
        self.hover_snap = None;
        self.status = None;
        self.snap_mid_quad = false;
        self.ensure_sources(false, cx);
        self.update_preview(cx);
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.zoom_window.take().is_some() || self.rotate_to_edge.take().is_some() || self.align_views.take().is_some() || self.sheet_props.take().is_some() || self.shortcut_toolbar.take().is_some() || self.measure.take().is_some() || self.align_centermark.take().is_some() || self.painter.take().is_some() || self.cancel_inspection(cx) || self.cancel_hatch(cx) {
            self.status = None;
            cx.notify();
            return;
        }
        if self.insert.chooser.take().is_some() {
            cx.notify();
            return;
        }
        if self.panel.is_some() {
            self.panel = None;
            cx.notify();
            return;
        }
        if self.weld_dlg.is_some() {
            self.close_weld_dlg(false, cx);
            return;
        }
        if self.gdt_dlg.is_some() {
            self.close_gdt_dlg(false, cx);
            return;
        }
        if self.custom_tables.dlg.as_ref().is_some_and(|d| d.picker.is_some()) {
            if let Some(d) = &mut self.custom_tables.dlg {
                d.picker = None;
            }
            cx.notify();
            return;
        }
        if self.custom_tables.dlg.as_ref().is_some_and(|d| d.table.is_some()) {
            self.custom_tables.dlg = None;
            cx.notify();
            return;
        }
        if self.stack_dlg.take().is_some() {
            cx.notify();
            return;
        }
        if self.callout_dlg.as_ref().is_some_and(|d| d.edit.is_some()) {
            self.callout_dlg = None;
            cx.notify();
            return;
        }
        if self.detail_escape(cx) {
            return;
        }
        if !matches!(self.tool, Tool::Select) {
            // Esc ends a spline (keeping it) and exits the sketch tool.
            self.finish_sketch(cx);
            self.tool = Tool::Select;
            self.overlay = Overlay::default();
            self.note_leader = None;
            self.hover_snap = None;
            // Onshape: Esc ends the tool and nothing stays selected (the
            // base view of a projected-view chain is no longer orange).
            self.selection.clear();
            self.set_preview(None, cx);
            self.redraw(cx);
            return;
        }
        self.selection.clear();
        self.redraw(cx);
    }

    pub fn delete_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        let sel = std::mem::take(&mut self.selection);
        self.edit_sheet("Delete", cx, |s| {
            // Breaks first (indices shift when removed).
            let mut breaks: Vec<(ViewId, usize)> = sel.iter().filter_map(|x| if let Sel::Break(v, i) = x { Some((*v, *i)) } else { None }).collect();
            breaks.sort_by_key(|b| std::cmp::Reverse(b.1));
            for (v, i) in breaks {
                if let Some(v) = s.view_mut(v)
                    && i < v.breaks.len()
                {
                    v.breaks.remove(i);
                }
            }
            for x in &sel {
                match x {
                    Sel::View(v) => s.remove_view(*v),
                    // Callouts grouped with the annotation go with it.
                    Sel::Annotation(a) => s.annotations.retain(|n| n.id != *a && n.group != Some(*a) && !matches!(n.kind, AnnotationKind::RevisionCallout { group: Some(g), .. } if g == *a)),
                    Sel::Table(t) => s.tables.retain(|n| n.id != *t),
                    Sel::Sketch(e) => {
                        s.sketch.sketch.remove_entity(noshape_sketch::EntityId(*e));
                    }
                    Sel::Hatch(h) => s.sketch.hatches.retain(|x| x.id != *h),
                    Sel::Image(i) => s.images.retain(|x| x.id != *i),
                    Sel::Break(..) => {}
                }
            }
            let _ = s.sketch.sketch.solve();
        });
    }

    /// Prompt shown in the status bar for the active tool.
    pub fn prompt(&self) -> Option<&'static str> {
        if let Some(p) = self.more_prompt() {
            return Some(p);
        }
        Some(match &self.tool {
            Tool::Select => return None,
            Tool::Sketch { tool, pts, .. } => tool.prompt(pts.len()),
            Tool::Constrain { kind: crate::drawing_sketch::SkCon::Horizontal | crate::drawing_sketch::SkCon::Vertical, .. } => "Select a line, or two points",
            Tool::Constrain { picks, .. } if picks.is_empty() => "Select the first entity (sketch geometry or a view edge)",
            Tool::Constrain { .. } => "Select the second entity",
            Tool::PlaceImage { .. } => "Click and drag to place the image",
            Tool::InsertView { .. } => "Click to place the view",
            Tool::Projected { parent: None } => "Select the view to project from",
            Tool::Projected { .. } => "Move away from the parent view and click to place (Esc to finish)",
            Tool::Auxiliary { parent: None } => "Select a straight edge to fold about",
            Tool::Auxiliary { .. } => "Click to place the auxiliary view",
            Tool::Section { p0: None, partial: false, .. } if self.section_axis().is_some() => "Click in a view to place the cutting line",
            Tool::Section { parent: None, .. } | Tool::Section { p0: None, .. } => "Click the first point of the cutting line in a view",
            Tool::Section { line: None, .. } => "Click the second point of the cutting line",
            Tool::Section { .. } => "Drag to the side to view from and click to place",
            Tool::Detail { center: None, .. } => match self.detail_dlg.as_ref().map(|d| (d.pick_region, d.shape, d.pts.len())) {
                Some((true, ..)) => "Click a closed sketch region for the detail profile",
                Some((_, 1, 0)) => "Click the first corner of the rectangle (Esc: pick a closed region)",
                Some((_, 1, _)) => "Click the opposite corner",
                Some((_, 2 | 3, 0)) => "Click the first profile point (Esc: pick a closed region)",
                Some((_, 2 | 3, _)) => "Click the next point; click the first point or double-click to close",
                _ => "Click the center of the detail circle (Esc: pick a closed region)",
            },
            Tool::Detail { radius: None, .. } => "Click to set the detail radius",
            Tool::Detail { .. } => "Click to place the detail view",
            Tool::HoleCallout { circle: None } => "Select a circle of a hole or thread",
            Tool::HoleCallout { .. } => "Drag to place the callout and click",
            Tool::EdgeCenterline { first: None } => "Select a cylinder silhouette edge, or the first of two edges",
            Tool::EdgeCenterline { .. } => "Select the second edge",
            Tool::CircleCenterline { three: true, points } if points.len() < 3 => "Select three points on the circle (hole centers, end, mid or quad points)",
            Tool::CircleCenterline { points, .. } if points.is_empty() => "Select the center of the circle",
            Tool::CircleCenterline { .. } => "Select a point on the circle",
            Tool::VirtualSharp { first: None } => "Select the first straight edge",
            Tool::VirtualSharp { .. } => "Select the second straight edge",
            Tool::Dimension { kind: DimTool::Chamfer, picks } if picks.is_empty() => "Select the chamfer edge",
            Tool::Dimension { kind: DimTool::Chamfer, picks } if picks.len() == 1 => "Select the adjacent edge",
            Tool::Dimension { kind: DimTool::ArcLength, picks } if picks.is_empty() => "Select an arc",
            Tool::Dimension { kind: DimTool::Radius | DimTool::Diameter, picks } if picks.is_empty() => "Select a circle or arc",
            Tool::Dimension { kind: DimTool::Angle, picks } if picks.len() < 2 => "Select two straight edges",
            Tool::Dimension { kind: DimTool::Angle3, picks } if picks.is_empty() => "Select the vertex of the angle",
            Tool::Dimension { kind: DimTool::Angle3, picks } if picks.len() < 3 => "Select a point on each side of the angle",
            Tool::Dimension { picks, .. } if picks.is_empty() => "Select a circle, arc, line, circle center or point",
            Tool::Dimension { picks, .. } if picks.len() == 1 && matches!(picks[0], Pick::Point(_)) => "Select a second entity",
            Tool::Dimension { kind: DimTool::Linear, picks } if picks.len() == 1 => "Click to place the dimension, or select a second entity",
            Tool::Dimension { .. } => "Click to place the dimension",
            Tool::Note if self.note_leader.is_some() => "Click to place the note text",
            Tool::Note => "Click a point for the leader, or click to place the note",
            Tool::Centermark => "Select a circle or arc",
            Tool::Centerline { first: None } => "Select the first point",
            Tool::Centerline { .. } => "Select the second point",
            Tool::Balloon { anchor: None } => "Select the point the callout points to (in an assembly view: a part, for its BOM item)",
            Tool::Balloon { .. } => "Click to place the callout",
            Tool::JoggedSection { parent: None, .. } | Tool::JoggedSection { done: false, .. } => "Click the points of the cutting line (orthogonal jogs); Enter or double-click to finish",
            Tool::JoggedSection { .. } => "Drag to the side to view from and click to place",
            Tool::AlignedSection { center: None, .. } => "Click the center of the cutting line (e.g. a hole center)",
            Tool::AlignedSection { a: None, .. } => "Click a point on the leg the view looks at",
            Tool::AlignedSection { b: None, .. } => "Click a point on the leg revolved into the view plane",
            Tool::AlignedSection { .. } => "Drag to the side to view from and click to place",
            Tool::Crop { first: None, circle: false, .. } => "Click the first corner of the crop rectangle in a view",
            Tool::Crop { first: None, circle: true, .. } => "Click the center of the crop circle in a view",
            Tool::Crop { circle: false, .. } => "Click the opposite corner",
            Tool::Crop { .. } => "Click to set the crop radius",
            Tool::Break { first: None, .. } => "Click the first break line in a view",
            Tool::Break { .. } => "Click the second break line",
            Tool::Ordinate { origin: None, .. } => "Select the datum (zero) point",
            Tool::Ordinate { horizontal: None, .. } => "Move up/down (x values) or sideways (y values) and click to place the zero",
            Tool::Ordinate { .. } => "Select points to dimension (Esc to finish)",
            Tool::Table { kind: TableTool::General | TableTool::Revision, .. } => "Click to place the table (top-right corner)",
            Tool::Table { kind: TableTool::Bom | TableTool::CutList | TableTool::Custom, .. } => "Click to place the table",
            Tool::Table { view: None, .. } => "Select the view the table is for",
            Tool::Table { kind: TableTool::Hole, origin: None, .. } => "Select the origin (a vertex or circle center)",
            Tool::Table { .. } => "Click to place the table (top-right corner)",
            Tool::Gdt => "Click to place the geometric tolerance",
            Tool::Datum { anchor: None } => "Click the edge or point the datum is attached to",
            Tool::Datum { .. } => "Click to place the datum box",
            Tool::SurfaceFinish => "Click the edge the symbol sits on",
            Tool::Weld if self.weld_dlg.as_ref().is_some_and(|d| d.target.is_some()) => "Click to place the symbol",
            Tool::Weld => "Select an entity to associate the symbol",
            Tool::RevisionCallout { .. } => "Click to place the revision callout",
            _ => return None,
        })
    }

    // ------------------------------------------------------------ picking

    /// The entity under the cursor: the picked geometry/annotation, else the
    /// view whose rectangle contains the point.
    pub fn sel_at(&self, pick: Option<u32>, s: [f64; 2]) -> Option<Sel> {
        if let Some(sel) = pick.and_then(|p| self.sel_of_pick(p)) {
            return Some(sel);
        }
        // Inserted images (borderless images have no pickable lines).
        if let Some(id) = self.image_at(s) {
            return Some(Sel::Image(id));
        }
        let b = self.built.as_ref()?;
        b.placements
            .iter()
            .filter(|p| p.contains(s, 1.0))
            .min_by(|a, c| ((a.max[0] - a.min[0]) * (a.max[1] - a.min[1])).total_cmp(&((c.max[0] - c.min[0]) * (c.max[1] - c.min[1]))))
            .map(|p| Sel::View(p.id))
    }

    /// Snap point (vertex / circle center) of a view near a sheet point.
    pub fn snap_at(&self, s: [f64; 2]) -> Option<PickedPoint> {
        let built = self.built.as_ref()?;
        let mut best: Option<(f64, PickedPoint)> = None;
        for pl in &built.placements {
            if !pl.contains(s, 3.0) {
                continue;
            }
            let Some(g) = self.geoms.get(&pl.id) else { continue };
            let vp = pl.to_view(s);
            let tol = self.tol_mm(8.0) / (pl.scale * 1000.0);
            if let Some(sn) = g.snap_near(vp, tol) {
                let d = len(sub(sn.p, vp)) * pl.scale * 1000.0;
                // Prefer vertices over centers at equal distance.
                let d = if sn.kind == SnapKind::Center { d + 0.01 } else { d };
                if best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                    best = Some((d, PickedPoint { view: pl.id, p: sn.p, r: sn.point.clone() }));
                }
            }
            // Construction annotations: virtual sharps and circle centerline
            // centers (and their quadrants) are snap points too.
            for a in self.current_sheet().annotations.iter().filter(|a| a.kind.view() == Some(pl.id)) {
                let cand: Option<([f64; 2], Option<PointRef>)> = match &a.kind {
                    AnnotationKind::VirtualSharp { a1, b1, a2, b2, .. } => AnnotationKind::line_intersection(*a1, *b1, *a2, *b2).map(|x| (x, a.refs.iter().find(|r| r.slot == 4).map(|r| r.point.clone()))),
                    AnnotationKind::CircleCenterline { points, .. } => AnnotationKind::circle_centerline(points).map(|(cc, _)| (cc, None)),
                    _ => None,
                };
                if let Some((p, r)) = cand {
                    let d = len(sub(p, vp)) * pl.scale * 1000.0;
                    if len(sub(p, vp)) <= tol && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                        best = Some((d, PickedPoint { view: pl.id, p, r }));
                    }
                }
            }
            // Shift+Q: edge midpoints and circle quadrant points.
            if self.snap_mid_quad && matches!(self.tool, Tool::Dimension { .. }) {
                for me in &g.model_edges {
                    let mut cand: Vec<([f64; 2], PointRef)> = Vec::new();
                    match me.geom {
                        views::EdgeGeom::Line { a, b } => cand.push(([(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5], PointRef::Midpoint { body: me.body.clone(), name: (*me.name).clone() })),
                        views::EdgeGeom::Circle { center, radius, face_on: true, mid, .. } => {
                            let closed = me.ends[0].is_none();
                            if !closed {
                                cand.push((mid, PointRef::Midpoint { body: me.body.clone(), name: (*me.name).clone() }));
                            }
                            for k in 0..4u8 {
                                let a = f64::from(k) * std::f64::consts::FRAC_PI_2;
                                cand.push(([center[0] + radius * a.cos(), center[1] + radius * a.sin()], PointRef::Quadrant { body: me.body.clone(), name: (*me.name).clone(), k }));
                            }
                        }
                        _ => {}
                    }
                    for (p, r) in cand {
                        let d = len(sub(p, vp)) * pl.scale * 1000.0;
                        if len(sub(p, vp)) <= tol && best.as_ref().is_none_or(|(bd, _)| d + 0.02 < *bd) {
                            best = Some((d, PickedPoint { view: pl.id, p, r: Some(r) }));
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// The hovered projected edge: (view, model edge).
    pub fn hovered_edge(&self) -> Option<(ViewId, views::ModelEdge)> {
        let h = self.hover?;
        let PickTarget::Edge { view, edge } = self.built.as_ref()?.picks.get(h as usize)?.clone() else { return None };
        let g = self.geoms.get(&view)?;
        let m = g.edges.get(edge as usize)?.model?;
        Some((view, g.model_edges.get(m as usize)?.clone()))
    }

    pub fn line_pick(&self) -> Option<Pick> {
        let (view, me) = self.hovered_edge()?;
        let EdgeGeom::Line { a, b } = me.geom else { return None };
        let pa = me.ends[0].clone().map(|(r, p)| PickedPoint { view, p, r: Some(r) }).unwrap_or(PickedPoint { view, p: a, r: None });
        let pb = me.ends[1].clone().map(|(r, p)| PickedPoint { view, p, r: Some(r) }).unwrap_or(PickedPoint { view, p: b, r: None });
        if len(sub(pa.p, pb.p)) < 1e-9 {
            return None;
        }
        Some(Pick::Line { a: pa, b: pb })
    }

    /// The hovered arc (open circular edge).
    pub fn arc_pick(&self) -> Option<Pick> {
        let (view, me) = self.hovered_edge()?;
        let EdgeGeom::Circle { center, radius, mid, .. } = me.geom else { return None };
        let (Some((ra, pa)), Some((rb, pb))) = (me.ends[0].clone(), me.ends[1].clone()) else { return None };
        if len(sub(pa, pb)) < 1e-9 {
            return None;
        }
        Some(Pick::Arc { view, center, radius, a: PickedPoint { view, p: pa, r: Some(ra) }, b: PickedPoint { view, p: pb, r: Some(rb) }, mid, r: me.center_ref() })
    }

    pub fn circle_pick(&self, s: [f64; 2]) -> Option<Pick> {
        if let Some((view, me)) = self.hovered_edge()
            && let EdgeGeom::Circle { center, radius, .. } = me.geom
        {
            return Some(Pick::Circle { view, center, radius, r: me.center_ref() });
        }
        let view = self.view_at(s)?;
        let pl = self.placement(view)?;
        let g = self.geoms.get(&view)?;
        let vp = pl.to_view(s);
        let tol = self.tol_mm(6.0) / (pl.scale * 1000.0);
        // Circle centerlines (bolt circles) can be dimensioned too.
        let construction: Vec<([f64; 2], f64, Option<PointRef>)> = self
            .current_sheet()
            .annotations
            .iter()
            .filter(|a| a.kind.view() == Some(view))
            .filter_map(|a| if let AnnotationKind::CircleCenterline { points, .. } = &a.kind { AnnotationKind::circle_centerline(points).map(|(c, r)| (c, r, None)) } else { None })
            .collect();
        g.circles()
            .into_iter()
            .chain(construction)
            .filter(|(c, r, _)| (len(sub(vp, *c)) - r).abs() < tol)
            .min_by(|a, b| (len(sub(vp, a.0)) - a.1).abs().total_cmp(&(len(sub(vp, b.0)) - b.1).abs()))
            .map(|(center, radius, r)| Pick::Circle { view, center, radius, r })
    }

    /// A point in a view: snap, else the point on a hovered edge, else free.
    pub fn any_point(&self, s: [f64; 2]) -> Option<PickedPoint> {
        if let Some(p) = self.snap_at(s) {
            return Some(p);
        }
        let view = self.view_at(s)?;
        let pl = self.placement(view)?;
        Some(PickedPoint { view, p: pl.to_view(s), r: None })
    }

    // ------------------------------------------------------------ previews

    pub fn pick_sheet(&self, p: &PickedPoint) -> Option<[f64; 2]> {
        Some(self.placement(p.view)?.to_sheet(p.p))
    }

    /// The annotation the active dimension tool would create at the cursor.
    pub fn pending_annotation(&self) -> Option<Annotation> {
        let c = self.cursor;
        let id = self.current_sheet().next_annotation_id();
        let ann = |kind, refs: Vec<(u8, Option<PointRef>)>| Annotation {
            id,
            kind,
            position: c,
            refs: refs.into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect(),
            ..Default::default()
        };
        if let Some(a) = self.extra_pending_annotation(id) {
            return Some(a);
        }
        match &self.tool {
            Tool::Dimension { kind: DimTool::Linear, picks } => {
                let (a, b) = match picks.as_slice() {
                    [Pick::Line { a, b }] => (a.clone(), b.clone()),
                    [Pick::Point(a), Pick::Point(b)] if a.view == b.view => (a.clone(), b.clone()),
                    // The smart Dimension (d): the picks decide the kind (drawing_smart).
                    _ => {
                        let (kind, refs) = self.smart_dimension(picks)?;
                        return Some(ann(kind, refs));
                    }
                };
                let (ps, qs) = (self.pick_sheet(&a)?, self.pick_sheet(&b)?);
                // Horizontal / vertical / aligned follows the cursor (Onshape).
                let o = auto_orientation(ps, qs, c);
                let offset = dim_offset(ps, qs, o, c);
                Some(ann(AnnotationKind::LinearDimension { view: a.view, a: a.p, b: b.p, offset, orientation: o, override_text: None }, vec![(0, a.r), (1, b.r)]))
            }
            Tool::Dimension { kind: kind @ (DimTool::Radius | DimTool::Diameter), picks } => {
                let [Pick::Circle { view, center, radius, r }] = picks.as_slice() else { return None };
                let cs = self.placement(*view)?.to_sheet(*center);
                let d = sub(c, cs);
                Some(ann(AnnotationKind::RadialDimension { view: *view, center: *center, radius: *radius, diameter: *kind == DimTool::Diameter, angle: d[1].atan2(d[0]) }, vec![(0, r.clone())]))
            }
            Tool::Dimension { kind: DimTool::Angle3, picks } => {
                let [Pick::Point(v), Pick::Point(pa), Pick::Point(pb)] = picks.as_slice() else { return None };
                if v.view != pa.view || v.view != pb.view {
                    return None;
                }
                // The smaller angle between the legs (Onshape measures < 180°).
                let ang = |p: [f64; 2]| (p[1] - v.p[1]).atan2(p[0] - v.p[0]);
                let mut sweep = ang(pb.p) - ang(pa.p);
                while sweep < 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                let (a, b) = if sweep > std::f64::consts::PI { (pb, pa) } else { (pa, pb) };
                let pl = self.placement(v.view)?;
                let radius = len(sub(c, pl.to_sheet(v.p))).max(3.0);
                Some(ann(AnnotationKind::AngularDimension { view: v.view, vertex: v.p, a: a.p, b: b.p, radius }, vec![(0, v.r.clone()), (1, a.r.clone()), (2, b.r.clone())]))
            }
            Tool::Dimension { kind: DimTool::Angle, picks } => {
                let [Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, b: b2 }] = picks.as_slice() else { return None };
                let (kind, refs) = self.angular_from_lines(a1, b1, a2, b2)?;
                Some(ann(kind, refs))
            }

            Tool::HoleCallout { circle: Some((view, center, radius, r)) } => Some(ann(AnnotationKind::HoleCallout { view: *view, center: *center, radius: *radius }, vec![(0, r.clone())])),
            Tool::Balloon { anchor: Some(a) } => {
                // In assembly views a callout on a part shows its BOM item (empty text = linked).
                let linked = views::view_source(self.current_sheet(), a.view).and_then(|(k, _)| self.sources.get(&k)).is_some_and(|s| s.assembly) && a.r.is_some();
                // The Callout dialog's texts and border (empty centre = the
                // BOM item number).
                let (text, style) = self.callout_values_now().unwrap_or_default();
                let mut kind = AnnotationKind::balloon(a.view, a.p, text);
                // On an assembly part (a face): a dot instead of an arrow.
                if let AnnotationKind::Balloon { callout, .. } = &mut kind {
                    *callout = style;
                    callout.on_face = linked;
                }
                Some(ann(kind, vec![(0, a.r.clone())]))
            }
            _ => None,
        }
    }

    /// The view the active view tool would create at the cursor.
    pub fn pending_view(&self) -> Option<DrawingView> {
        let c = self.cursor;
        let sheet = &self.drawing.sheets.get(self.sheet)?;
        let id = sheet.next_view_id();
        // Base views take Drawing properties › Views › Insert view
        // defaults; derived views copy their parent's.
        let mk = |kind: ViewKind, position: [f64; 2], parent: Option<&DrawingView>| {
            let mut v = DrawingView { id, kind, position, scale: None, label: None, ..Default::default() };
            match parent {
                Some(p) => {
                    v.style = p.style;
                    v.show_tangent_edges = p.show_tangent_edges;
                    v.tangent_phantom = p.tangent_phantom;
                    v.show_centerlines = p.show_centerlines;
                    v.auto_centerlines = p.auto_centerlines;
                }
                None => self.drawing.props.view_defaults.apply(&mut v),
            }
            v
        };
        if let Some(v) = self.extra_pending_view(id) {
            return Some(v);
        }
        match &self.tool {
            Tool::InsertView { source, orientation, part } => {
                let mut v = mk(ViewKind::Base { source: *source, config: Default::default(), orientation: *orientation, part: part.clone() }, c, None);
                // Insert view panel › View scale (None: the sheet scale), View simplification.
                v.scale = self.insert.scale;
                v.simplification = Some(self.insert.simplification);
                v.flat = self.insert.flat;
                // All views of a part use the same version: a pinned
                // reference pins the new view too.
                v.pinned = self.drawing.sheets.iter().flat_map(|s| &s.views).find(|o| matches!(&o.kind, ViewKind::Base { source: s2, part: p2, .. } if s2 == source && p2 == part) && o.pinned.is_some()).and_then(|o| o.pinned.clone());
                // A sketch view faces its sketch plane.
                if let Some(f) = &self.insert.sketch {
                    v.sketch = Some(*f);
                    v.custom_frame = self.insert_sketch_frame(*source);
                }
                Some(v)
            }
            Tool::Projected { parent: Some(pid) } => {
                let parent = sheet.view(*pid)?;
                let d = sub(c, parent.position);
                if len(d) < 15.0 {
                    return None;
                }
                // Direction in the parent's (unrotated) view frame.
                let rot = views::view_rotation(sheet, *pid);
                let dir = views::snap_direction(views::rotate2(d, -rot));
                let pos = if dir[0] == 0.0 || dir[1] == 0.0 {
                    let axis = views::rotate2(dir, rot);
                    let t = dot(d, axis);
                    [parent.position[0] + axis[0] * t, parent.position[1] + axis[1] * t]
                } else {
                    c
                };
                Some(mk(ViewKind::Projected { parent: *pid, direction: Some(dir) }, pos, Some(parent)))
            }
            Tool::Auxiliary { parent: Some((pid, edge_dir)) } => {
                let parent = sheet.view(*pid)?;
                let n = norm([-edge_dir[1], edge_dir[0]]);
                let t = dot(sub(c, parent.position), n);
                if t.abs() < 15.0 {
                    return None;
                }
                let pos = [parent.position[0] + n[0] * t, parent.position[1] + n[1] * t];
                let mut v = mk(ViewKind::Auxiliary { parent: *pid, edge_dir: *edge_dir }, pos, Some(parent));
                // Auxiliary view dialog: label, Reverse, reference indicator.
                if let Some(d) = &self.aux_dlg {
                    v.flip = d.reverse;
                    v.aux_ref = d.anchor.map(|anchor| noshape_model::drawing::AuxRef { anchor, letter: self.aux_letter.clone(), show: d.show_ref });
                }
                Some(v)
            }
            Tool::Section { parent: Some(pid), line: Some((a, b)), .. } => {
                let parent = sheet.view(*pid)?;
                let pl = self.placement(*pid)?;
                let (sa, sb) = (pl.to_sheet(*a), pl.to_sheet(*b));
                let e = norm(sub(sb, sa));
                let n = [-e[1], e[0]];
                let t = dot(sub(c, parent.position), n);
                let pos = [parent.position[0] + n[0] * t, parent.position[1] + n[1] * t];
                let mut v = mk(ViewKind::Section { parent: *pid, line: vec![*a, *b], label: sheet.next_label() }, pos, Some(parent));
                v.style = RenderStyle::HiddenLinesRemoved;
                v.partial = matches!(self.tool, Tool::Section { partial: true, .. });
                // Section view dialog: Show cut geometry only, Section depth.
                self.apply_section_options(&mut v);
                Some(v)
            }
            Tool::Detail { parent: Some(pid), center: Some(center), radius: Some(r) } => {
                let parent = sheet.view(*pid)?;
                let mut v = mk(ViewKind::Detail { parent: *pid, center: *center, radius: *r, label: sheet.next_label() }, c, Some(parent));
                v.scale = Some(views::view_scale(sheet, *pid) * 2.0);
                // Detail view dialog: profile, scale, label, style, options.
                self.apply_detail_options(&mut v, views::view_scale(sheet, *pid));
                Some(v)
            }
            _ => None,
        }
    }

    /// Rebuild the preview drawing and the overlay for the current tool state.
    pub fn update_preview(&mut self, cx: &mut Context<Self>) {
        let mut overlay = Overlay::default();
        let c = self.cursor;
        let snap = match &self.tool {
            Tool::Dimension { kind: DimTool::Linear | DimTool::Angle3 | DimTool::MaxMin | DimTool::Foreshorten, .. }
            | Tool::BrokenOut { .. }
            | Tool::CropPoly { .. }
            | Tool::DatumTarget { anchor: None }
            | Tool::GdtLeader { .. }
            | Tool::Centerline { .. }
            | Tool::CircleCenterline { .. }
            | Tool::Balloon { anchor: None }
            | Tool::Section { p0: None, .. }
            | Tool::Detail { center: None, .. }
            | Tool::JoggedSection { done: false, .. }
            | Tool::AlignedSection { b: None, .. }
            | Tool::Ordinate { .. }
            | Tool::Table { kind: TableTool::Hole, origin: None, .. }
            | Tool::Datum { anchor: None }
            | Tool::Note
            | Tool::Weld
            | Tool::Crop { .. } => self.snap_at(c),
            _ => None,
        };
        self.hover_snap = snap.as_ref().and_then(|s| self.pick_sheet(s));
        if let (Some(p), Some(s)) = (self.hover_snap, &snap) {
            // Shape-coded orange snap marker (live: drawing/10).
            overlay.marks.push((p, crate::drawing_scene::SnapMark::of(s.r.as_ref()), self.tol_mm(9.0)));
        }
        let mut preview: Option<Drawing> = None;
        match &self.tool {
            Tool::Dimension { picks, .. } => {
                if let [Pick::Point(a)] = picks.as_slice()
                    && let Some(sa) = self.pick_sheet(a)
                {
                    overlay.lines.push((vec![sa, self.hover_snap.unwrap_or(c)], true));
                }
                for p in picks {
                    match p {
                        Pick::Point(a) => overlay.points.extend(self.pick_sheet(a)),
                        Pick::Line { a, b } => {
                            if let (Some(sa), Some(sb)) = (self.pick_sheet(a), self.pick_sheet(b)) {
                                overlay.lines.push((vec![sa, sb], false));
                            }
                        }
                        Pick::Circle { view, center, radius, .. } | Pick::Arc { view, center, radius, .. } | Pick::CircleAt { view, center, radius, .. } => {
                            if let Some(pl) = self.placement(*view) {
                                overlay.circle(pl.to_sheet(*center), pl.len_to_sheet(*radius), false);
                            }
                        }
                    }
                }
            }
            Tool::Centerline { first: Some(a) } => {
                if let Some(sa) = self.pick_sheet(a) {
                    overlay.lines.push((vec![sa, self.hover_snap.unwrap_or(c)], true));
                }
            }
            Tool::CircleCenterline { three, points } if !points.is_empty() => {
                let at = self.hover_snap.unwrap_or(c);
                let mut pts: Vec<[f64; 2]> = points.iter().filter_map(|p| self.pick_sheet(p)).collect();
                overlay.points.extend(pts.iter().copied());
                pts.push(at);
                if (*three && pts.len() == 3) || (!*three && pts.len() == 2) {
                    if let Some((cc, r)) = AnnotationKind::circle_centerline(&pts) {
                        overlay.circle(cc, r, true);
                    }
                } else if pts.len() == 2 {
                    overlay.lines.push((pts, true));
                }
            }
            Tool::EdgeCenterline { first: Some((a, b)) } | Tool::VirtualSharp { first: Some((a, b, _)) } => {
                if let (Some(sa), Some(sb)) = (self.pick_sheet(a), self.pick_sheet(b)) {
                    overlay.lines.push((vec![sa, sb], false));
                }
            }
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None, .. } => {
                if let Some(pl) = self.placement(*pid) {
                    let a = pl.to_sheet(*p0);
                    let b = self.ortho_point(a, c);
                    overlay.lines.push((vec![a, b], true));
                }
            }
            // Vertical / horizontal section: the cutting line follows the cursor across the view.
            Tool::Section { p0: None, partial: false, .. } if self.section_axis().is_some() => {
                let at = self.hover_snap.unwrap_or(c);
                if let (Some(e), Some(v)) = (self.section_axis(), self.view_at(at))
                    && let Some((la, lb)) = self.extend_cut(v, at, e)
                    && let Some(pl) = self.placement(v)
                {
                    overlay.lines.push((vec![pl.to_sheet(la), pl.to_sheet(lb)], true));
                }
            }
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: None } => {
                if let Some(pl) = self.placement(*pid) {
                    let cs = pl.to_sheet(*ctr);
                    overlay.circle(cs, len(sub(c, cs)), true);
                }
            }
            Tool::Sketch { .. } => self.sketch_overlay(&mut overlay),
            Tool::Weld => self.weld_overlay(&mut overlay),
            Tool::Note if self.note_leader.is_some() => {
                if let Some(a) = self.note_leader.as_ref().and_then(|l| Some(self.placement(l.view?)?.to_sheet(l.point))) {
                    overlay.lines.push((vec![a, self.cursor], true));
                }
            }
            Tool::PlaceImage { .. } => self.image_overlay(&mut overlay),
            Tool::Detail { center: None, .. } if self.detail_dlg.as_ref().is_some_and(|d| d.shape != 0) => self.detail_overlay(&mut overlay),
            _ => self.extra_overlay(&mut overlay),
        }
        if let Some(v) = self.pending_view() {
            let mut d = self.drawing.clone();
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                s.views.push(v);
            }
            preview = Some(d);
        } else if let Some(a) = self.pending_annotation() {
            let mut d = self.drawing.clone();
            if let Some(s) = d.sheets.get_mut(self.sheet) {
                s.annotations.push(a);
            }
            preview = Some(d);
        }
        overlay.infer = self.infer_lines.clone();
        for (c, r) in self.centermark_overlay() {
            overlay.circle(c, r, false);
        }
        self.overlay = overlay;
        let had_preview = self.preview.is_some();
        if preview.is_some() || had_preview {
            self.set_preview(preview, cx);
        }
        self.redraw(cx);
    }

    /// Snap a second point to horizontal/vertical from `a` when close.
    pub fn ortho_point(&self, a: [f64; 2], c: [f64; 2]) -> [f64; 2] {
        let d = sub(c, a);
        let ang = d[1].atan2(d[0]).to_degrees().rem_euclid(180.0);
        if ang < 8.0 || ang > 172.0 {
            [c[0], a[1]]
        } else if (ang - 90.0).abs() < 8.0 {
            [a[0], c[1]]
        } else {
            c
        }
    }

    // ------------------------------------------------------------ clicks

    /// A cutting line through sheet point `a` along sheet direction `e`,
    /// extended across view `pid` (view coordinates).
    pub fn extend_cut(&self, pid: ViewId, a: [f64; 2], e: [f64; 2]) -> Option<([f64; 2], [f64; 2])> {
        let pl = self.placement(pid)?;
        let corners = [pl.min, [pl.max[0], pl.min[1]], pl.max, [pl.min[0], pl.max[1]]];
        let ts: Vec<f64> = corners.iter().map(|k| dot(sub(*k, a), e)).collect();
        let t0 = ts.iter().cloned().fold(f64::MAX, f64::min) - 6.0;
        let t1 = ts.iter().cloned().fold(f64::MIN, f64::max) + 6.0;
        Some((pl.to_view([a[0] + e[0] * t0, a[1] + e[1] * t0]), pl.to_view([a[0] + e[0] * t1, a[1] + e[1] * t1])))
    }

    fn tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Centermark dialog: Single places at once, Circular / Linear collect circles.
        if matches!(self.tool, Tool::Centermark) && self.centermark_click(cx) {
            return;
        }
        let c = self.cursor;
        if self.sketch_click(cx) {
            return;
        }
        if matches!(self.tool, Tool::Section { .. }) && self.section_field_click(cx) {
            return;
        }
        if matches!(self.tool, Tool::Detail { .. }) && self.detail_click(cx) {
            return;
        }
        if self.weld_click(cx) || self.stack_click(cx) || self.table_dlg_click(cx) || self.custom_dlg_click(cx) || self.gdt_click(cx) {
            return;
        }
        if self.extra_tool_click(window, cx) {
            self.update_preview(cx);
            return;
        }
        let tool = self.tool.clone();
        match tool {
            Tool::Select => {}
            Tool::InsertView { .. } => {
                if let Some(v) = self.pending_view() {
                    let id = v.id;
                    self.edit_sheet("Insert view", cx, |s| s.views.push(v));
                    // Placing a view on an out-of-date drawing updates it.
                    if self.out_of_date {
                        self.update_drawing(cx);
                    }
                    // Onshape activates the projected view tool next.
                    self.tool = Tool::Projected { parent: Some(id) };
                    self.selection = vec![Sel::View(id)];
                }
            }
            Tool::Projected { parent: None } => {
                if let Some(v) = self.view_at(c) {
                    self.tool = Tool::Projected { parent: Some(v) };
                }
            }
            Tool::Projected { parent: Some(_) } | Tool::Auxiliary { parent: Some(_) } => {
                if let Some(v) = self.pending_view() {
                    let name = if matches!(self.tool, Tool::Auxiliary { .. }) { "Auxiliary view" } else { "Projected view" };
                    self.edit_sheet(name, cx, |s| s.views.push(v));
                    if matches!(self.tool, Tool::Auxiliary { .. }) {
                        self.tool = Tool::Select;
                    }
                }
            }
            Tool::Auxiliary { parent: None } => {
                // Auxiliary view dialog: Normal / In line / Axis (drawing_aux).
                if let Some((v, d, anchor)) = self.aux_pick() {
                    let kind = self.current_sheet().view(v).map(|x| (matches!(x.kind, ViewKind::Detail { .. }), !x.breaks.is_empty()));
                    if kind.is_some_and(|(det, brk)| det || brk) {
                        self.status = Some("Auxiliary views can't be created from detail or break views".into());
                    } else {
                        if let Some(dl) = &mut self.aux_dlg {
                            dl.anchor = Some(anchor);
                        }
                        self.status = None;
                        self.tool = Tool::Auxiliary { parent: Some((v, d)) };
                    }
                } else {
                    self.status = Some(match self.aux_dlg.as_ref().map(|d| d.mode) {
                        Some(1) => "Select a sketch line or centerline",
                        Some(2) => "Select a hole axis",
                        _ => "Select a straight edge of a view",
                    }
                    .into());
                }
            }
            Tool::Section { p0: None, partial, .. } => {
                // A click just outside a view still cuts it (8 mm around it).
                let near = self.view_at(c).or_else(|| {
                    let b = self.built.as_ref()?;
                    b.placements.iter().filter(|p| p.contains(c, 8.0)).min_by(|x, y| x.dist_to(c).total_cmp(&y.dist_to(c))).map(|p| p.id)
                });
                let pick = self.snap_at(c).or_else(|| near.and_then(|v| self.placement(v).map(|pl| PickedPoint { view: v, p: pl.to_view(c), r: None })));
                if pick.is_none() {
                    self.status = Some("Select a view to section".into());
                }
                if let Some(pp) = pick {
                    // Section view dialog › Vertical / Horizontal: one click places the whole line.
                    let axis_line = self.section_axis().filter(|_| !partial).and_then(|e| self.placement(pp.view).and_then(|pl| self.extend_cut(pp.view, pl.to_sheet(pp.p), e)));
                    self.tool = Tool::Section { parent: Some(pp.view), p0: Some(pp.p), line: axis_line, partial };
                }
            }
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None, partial } => {
                let Some(pl) = self.placement(pid) else { return };
                let a = pl.to_sheet(p0);
                let b = self.ortho_point(a, c);
                if len(sub(b, a)) < 2.0 {
                    return;
                }
                if partial {
                    // A partial section uses the line as clicked (snapped).
                    let q = self.snap_at(c).filter(|q| q.view == pid).map(|q| q.p).unwrap_or(pl.to_view(b));
                    self.tool = Tool::Section { parent: Some(pid), p0: Some(p0), line: Some((p0, q)), partial };
                    self.update_preview(cx);
                    return;
                }
                // Extend the cutting line across the whole view.
                let Some((la, lb)) = self.extend_cut(pid, a, norm(sub(b, a))) else { return };
                self.tool = Tool::Section { parent: Some(pid), p0: Some(p0), line: Some((la, lb)), partial };
            }
            Tool::Section { line: Some(_), .. } | Tool::Detail { radius: Some(_), .. } => {
                if let Some(mut v) = self.pending_view() {
                    if let (ViewKind::Section { label, .. }, Some(l)) = (&mut v.kind, self.section_label(cx)) {
                        *label = l;
                    }
                    let id = v.id;
                    let name = if matches!(self.tool, Tool::Section { .. }) { "Section view" } else { "Detail view" };
                    self.edit_sheet(name, cx, |s| s.views.push(v));
                    self.tool = Tool::Select;
                    self.selection = vec![Sel::View(id)];
                }
            }
            Tool::Detail { center: None, .. } => {
                let pick = self.snap_at(c).or_else(|| self.view_at(c).and_then(|v| self.placement(v).map(|pl| PickedPoint { view: v, p: pl.to_view(c), r: None })));
                if let Some(pp) = pick {
                    self.tool = Tool::Detail { parent: Some(pp.view), center: Some(pp.p), radius: None };
                }
            }
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: None } => {
                if let Some(pl) = self.placement(pid) {
                    let r = len(sub(c, pl.to_sheet(ctr))) / (pl.scale * 1000.0);
                    if r > 1e-6 {
                        self.tool = Tool::Detail { parent: Some(pid), center: Some(ctr), radius: Some(r) };
                    }
                }
            }
            Tool::Dimension { kind, mut picks } => {
                // Smart Dimension: a click on another entity adds it (a second
                // pick) instead of placing; on a picked edge it deselects it.
                let more = kind == DimTool::Linear && picks.len() == 1 && self.smart_takes(c, &picks[0]);
                if !more && let Some(a) = self.pending_annotation() {
                    // Chamfer, Dimensions style: the two legs as linear dimensions.
                    let anns = self.chamfer_as_dimensions(&a).unwrap_or_else(|| vec![a]);
                    self.edit_sheet(kind.name(), cx, |s| s.annotations.extend(anns));
                    self.tool = Tool::Dimension { kind, picks: vec![] };
                } else {
                    match kind {
                        DimTool::Radius | DimTool::Diameter => {
                            if let Some(p) = self.circle_pick(c) {
                                picks = vec![p];
                            }
                        }
                        DimTool::Angle | DimTool::Chamfer => {
                            if let Some(p) = self.line_pick() {
                                picks.push(p);
                            }
                        }
                        DimTool::ArcLength => {
                            if let Some(p) = self.arc_pick() {
                                picks = vec![p];
                            }
                        }
                        DimTool::MaxMin | DimTool::Foreshorten => self.more_dimension_pick(kind, &mut picks),
                        DimTool::Angle3 => {
                            if let Some(p) = self.snap_at(c).filter(|p| picks.first().is_none_or(|f| matches!(f, Pick::Point(v) if v.view == p.view))) {
                                picks.push(Pick::Point(p));
                            }
                        }
                        _ => self.smart_click(c, &mut picks),
                    }
                    self.tool = Tool::Dimension { kind, picks };
                }
            }
            Tool::HoleCallout { circle: None } => {
                if let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(c) {
                    self.tool = Tool::HoleCallout { circle: Some((view, center, radius, r)) };
                }
            }
            Tool::HoleCallout { circle: Some(_) } => {
                if let Some(a) = self.pending_annotation() {
                    self.edit_sheet("Hole callout", cx, |s| s.annotations.push(a));
                    self.tool = Tool::HoleCallout { circle: None };
                }
            }
            Tool::EdgeCenterline { first } => {
                let Some(Pick::Line { a, b }) = self.line_pick() else {
                    self.status = Some("Select a straight edge".into());
                    return;
                };
                match first {
                    None => {
                        // A cylinder / cone silhouette: its axis (the view's axis line parallel to it).
                        let dir = norm(sub(b.p, a.p));
                        let axis = self.geoms.get(&a.view).and_then(|g| {
                            g.axes
                                .iter()
                                .filter(|ax| (dot(norm(sub(ax[1], ax[0])), dir)).abs() > 1.0 - 1e-6)
                                .min_by(|x, y| {
                                    let d = |ax: &[[f64; 2]; 2]| {
                                        let n = [-dir[1], dir[0]];
                                        dot(sub(ax[0], a.p), n).abs()
                                    };
                                    d(x).total_cmp(&d(y))
                                })
                                .copied()
                        });
                        let edge_len = len(sub(b.p, a.p));
                        match axis.filter(|ax| {
                            let n = [-dir[1], dir[0]];
                            dot(sub(ax[0], a.p), n).abs() < edge_len.max(1e-6) * 2.0
                        }) {
                            Some(ax) => {
                                // Span the silhouette edge along the axis.
                                let o = ax[0];
                                let t0 = dot(sub(a.p, o), dir);
                                let t1 = dot(sub(b.p, o), dir);
                                let (p, q) = ([o[0] + dir[0] * t0, o[1] + dir[1] * t0], [o[0] + dir[0] * t1, o[1] + dir[1] * t1]);
                                let id = self.current_sheet().next_annotation_id();
                                self.edit_sheet("Edge to edge centerline", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Centerline { view: a.view, a: p, b: q }, position: c, ..Default::default() }));
                            }
                            None => self.tool = Tool::EdgeCenterline { first: Some((a, b)) },
                        }
                    }
                    Some((a1, b1)) => {
                        if a.view != a1.view {
                            self.status = Some("Select an edge in the same view".into());
                            return;
                        }
                        // The middle line of the two edges (ends paired along the first edge).
                        let dir = norm(sub(b1.p, a1.p));
                        let (a2, b2) = if dot(sub(b.p, a.p), dir) >= 0.0 { (a.p, b.p) } else { (b.p, a.p) };
                        let mid = |p: [f64; 2], q: [f64; 2]| [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5];
                        let (p, q) = (mid(a1.p, a2), mid(b1.p, b2));
                        let id = self.current_sheet().next_annotation_id();
                        self.edit_sheet("Edge to edge centerline", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Centerline { view: a.view, a: p, b: q }, position: c, ..Default::default() }));
                        self.tool = Tool::EdgeCenterline { first: None };
                    }
                }
            }
            Tool::CircleCenterline { three, mut points } => {
                if let Some(p) = self.snap_at(c).filter(|p| points.first().is_none_or(|f| f.view == p.view)) {
                    points.push(p);
                }
                let need = if three { 3 } else { 2 };
                if points.len() >= need {
                    let view = points[0].view;
                    let refs: Vec<AnnotationRef> = points.iter().enumerate().filter_map(|(k, p)| p.r.clone().map(|point| AnnotationRef { slot: k as u8, point })).collect();
                    let pts: Vec<[f64; 2]> = points.iter().map(|p| p.p).collect();
                    if AnnotationKind::circle_centerline(&pts).is_some() {
                        let id = self.current_sheet().next_annotation_id();
                        let name = if three { "3 point circle centerline" } else { "2 point circle centerline" };
                        self.edit_sheet(name, cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::CircleCenterline { view, points: pts }, position: c, refs, ..Default::default() }));
                    }
                    points.clear();
                }
                self.tool = Tool::CircleCenterline { three, points };
            }
            Tool::VirtualSharp { first } => {
                let Some((view, me)) = self.hovered_edge() else { return };
                let Some(Pick::Line { a, b }) = self.line_pick() else {
                    self.status = Some("Select a straight edge".into());
                    return;
                };
                let edge = Some((me.body.clone(), (*me.name).clone()));
                match first {
                    None => self.tool = Tool::VirtualSharp { first: Some((a, b, edge)) },
                    Some((a1, b1, e1)) if a1.view == view => {
                        if AnnotationKind::line_intersection(a1.p, b1.p, a.p, b.p).is_none() {
                            self.status = Some("The edges are parallel".into());
                            return;
                        }
                        let mut refs: Vec<AnnotationRef> = [(0u8, &a1.r), (1, &b1.r), (2, &a.r), (3, &b.r)].into_iter().filter_map(|(slot, r)| r.clone().map(|point| AnnotationRef { slot, point })).collect();
                        // Slot 4: the intersection itself, for associative dimensions to it.
                        if let (Some((body, name)), Some((body2, name2))) = (e1, edge) {
                            refs.push(AnnotationRef { slot: 4, point: PointRef::Intersection { body, name, body2, name2 } });
                        }
                        let id = self.current_sheet().next_annotation_id();
                        self.edit_sheet("Virtual sharp", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::VirtualSharp { view, a1: a1.p, b1: b1.p, a2: a.p, b2: b.p }, position: c, refs, ..Default::default() }));
                        self.tool = Tool::VirtualSharp { first: None };
                    }
                    Some(_) => self.status = Some("Select an edge in the same view".into()),
                }
            }
            Tool::Note => {
                // A click on a snap point anchors a leader; the next click
                // places the text.
                match (self.note_leader.is_some(), self.snap_at(c)) {
                    (false, Some(p)) => self.note_leader = Some(Leader { view: Some(p.view), point: p.p, nodes: Vec::new() }),
                    _ => self.open_note_panel(None, c, window, cx),
                }
            }
            Tool::Centerline { first: None } => {
                if let Some(p) = self.snap_at(c) {
                    self.tool = Tool::Centerline { first: Some(p) };
                }
            }
            Tool::Centerline { first: Some(a) } => {
                if let Some(b) = self.snap_at(c).filter(|b| b.view == a.view) {
                    let id = self.current_sheet().next_annotation_id();
                    let refs: Vec<AnnotationRef> = [(0u8, a.r.clone()), (1u8, b.r.clone())].into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect();
                    self.edit_sheet("Centerline", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Centerline { view: a.view, a: a.p, b: b.p }, position: c, refs, ..Default::default() }));
                    self.tool = Tool::Centerline { first: None };
                }
            }
            Tool::Balloon { anchor: None } => {
                if let Some(p) = self.any_point(c) {
                    self.tool = Tool::Balloon { anchor: Some(p) };
                } else {
                    // Empty space: a callout without a leader, right there.
                    self.refresh_callout_values(cx);
                    let (text, style) = self.callout_values_now().unwrap_or_default();
                    let mut kind = AnnotationKind::balloon(ViewId(0), c, text);
                    if let AnnotationKind::Balloon { callout, .. } = &mut kind {
                        *callout = noshape_model::drawing::CalloutStyle { free: true, ..style };
                    }
                    let id = self.current_sheet().next_annotation_id();
                    self.edit_sheet("Callout", cx, |s| s.annotations.push(Annotation { id, kind, position: c, ..Default::default() }));
                }
            }
            Tool::Balloon { anchor: Some(_) } => {
                self.refresh_callout_values(cx);
                if let Some(a) = self.pending_annotation() {
                    self.edit_sheet("Callout", cx, |s| s.annotations.push(a));
                    self.tool = Tool::Balloon { anchor: None };
                }
            }
            _ => {}
        }
        self.update_preview(cx);
    }

    // ------------------------------------------------------------ mouse

    pub fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
        window.focus(&self.focus, cx);
        if self.shortcut_toolbar.take().is_some() {
            cx.notify();
        }
        // Double middle click: zoom to fit (live navigation/45-46).
        if e.button == MouseButton::Middle && e.click_count >= 2 {
            self.drag = Drag::None;
            self.zoom_to_fit(cx);
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.painter.is_some() && self.painter_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.image_mouse_down() {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.hide_parts_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.hatch_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.inspection.is_some() && self.inspection_click(window, cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.measure_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.zoom_window_down(lp, cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && self.align_centermark.is_some() && self.align_centermark_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.rotate_to_edge.is_some() && self.rotate_to_edge_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.align_views.is_some() && self.align_views_click(cx) {
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::Select) {
            self.double_click(window, cx);
            return;
        }
        if e.button == MouseButton::Left && matches!(self.tool, Tool::Select) && self.flip_node_click(cx) {
            cx.stop_propagation();
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && self.finish_sketch(cx) {
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::Detail { .. }) && self.finish_detail_profile(cx) {
            return;
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::JoggedSection { done: false, .. } | Tool::BrokenOut { done: false, .. } | Tool::CropPoly { .. }) {
            self.confirm(window, cx);
            return;
        }
        self.update_hover(cx);
        self.drag = Drag::Pending { button: e.button, start: self.cursor, start_px: lp, pick: self.hover };
        cx.stop_propagation();
    }

    pub fn on_mouse_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
        if self.zoom_window_move(lp, cx) {
            return;
        }
        if let Drag::Pending { button, start, start_px, pick } = self.drag.clone()
            && len(sub(lp, start_px)) > 3.0
        {
            self.drag = match button {
                MouseButton::Right | MouseButton::Middle => {
                    // Preferences › Mouse controls, "2D Drawing" column
                    // (default: right or middle drag, with or without Ctrl, pans).
                    let mut buttons = nav::Buttons::default();
                    buttons.set(button, true);
                    let r = nav::ModeResolver::new(nav::MouseControls::current(), nav::Space::Drawing);
                    match r.resolve(buttons, r.input_key(e.modifiers.into()), nav::Mode::None) {
                        nav::Mode::Pan => Drag::Pan { last_px: start_px },
                        nav::Mode::Zoom => Drag::Zoom { anchor: start_px, last_px: start_px },
                        _ => Drag::None,
                    }
                }
                MouseButton::Left if matches!(self.tool, Tool::Select) => {
                    // Alt+drag copies: a duplicate stays, the dragged one moves.
                    if e.modifiers.alt {
                        self.alt_duplicate(pick, start, cx);
                    }
                    self.start_drag(pick, start)
                }
                _ => Drag::None,
            };
        }
        match self.drag.clone() {
            Drag::Zoom { anchor, last_px } => {
                // Drag zoom: device pixels / DRAG_ZOOM_FACTOR, up zooms in.
                let amount = -(lp[1] - last_px[1]) * window.scale_factor() as f64 / nav::DRAG_ZOOM_FACTOR;
                self.zoom_by(1.0 / nav::zoom_scale(amount), anchor, cx);
                self.drag = Drag::Zoom { anchor, last_px: lp };
            }
            Drag::Pan { last_px } => {
                self.center[0] -= (lp[0] - last_px[0]) / self.zoom;
                self.center[1] += (lp[1] - last_px[1]) / self.zoom;
                self.drag = Drag::Pan { last_px: lp };
                self.redraw(cx);
            }
            Drag::MoveViews { start, orig, constraint } => {
                let mut d = sub(self.cursor, start);
                if let Some((axis, _)) = constraint {
                    let t = dot(d, axis);
                    d = [axis[0] * t, axis[1] * t];
                }
                let mut dr = self.drawing.clone();
                if let Some(s) = dr.sheets.get_mut(self.sheet) {
                    for (id, p) in &orig {
                        if let Some(v) = s.view_mut(*id) {
                            v.position = [p[0] + d[0], p[1] + d[1]];
                        }
                    }
                    // Notes and callouts grouped with a moved view go along.
                    for a in s.annotations.iter_mut().filter(|a| a.view_group.is_some_and(|v| orig.iter().any(|(o, _)| *o == v))) {
                        a.position = [a.position[0] + d[0], a.position[1] + d[1]];
                    }
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::MoveTable { .. } | Drag::ResizeTable { .. } | Drag::TableDivider { .. } | Drag::MoveSectionLine { .. } | Drag::MoveDetailCircle { .. } | Drag::MoveSectionPoint { .. } | Drag::NoteGrip { .. } => {
                let drag = self.drag.clone();
                self.extra_drag_move(&drag, cx);
            }
            Drag::MoveAnnotation { start, orig } => {
                self.infer_lines.clear();
                if !e.modifiers.shift {
                    // Inference on the dragged annotation's own position.
                    let offset = sub(orig.position, start);
                    let raw = self.cursor;
                    self.cursor = [raw[0] + offset[0], raw[1] + offset[1]];
                    self.infer_placement(Some(orig.id));
                    self.cursor = [self.cursor[0] - offset[0], self.cursor[1] - offset[1]];
                }
                let a = self.dragged_annotation(&orig, start);
                let mut dr = self.drawing.clone();
                // Group annotation: the other members follow (not with Shift).
                if let (Some(g), false) = (orig.ann_group, e.modifiers.shift)
                    && let Some(s) = dr.sheets.get_mut(self.sheet)
                {
                    let d = sub(a.position, orig.position);
                    for x in s.annotations.iter_mut().filter(|x| x.ann_group == Some(g) && x.id != orig.id) {
                        x.position = [x.position[0] + d[0], x.position[1] + d[1]];
                    }
                }
                if let Some(s) = dr.sheets.get_mut(self.sheet)
                    && let Some(x) = s.annotations.iter_mut().find(|x| x.id == a.id)
                {
                    *x = a;
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::MoveInspection(_) => self.update_hover(cx),
            Drag::MoveImage { id, start, orig } => {
                let d = sub(self.cursor, start);
                let mut dr = self.drawing.clone();
                if let Some(im) = dr.sheets[self.sheet].images.iter_mut().find(|i| i.id == id) {
                    im.pos = [orig[0] + d[0], orig[1] + d[1]];
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::MoveSketch { id, start, orig } => {
                let dr = self.drag_sketch(id, &orig, sub(self.cursor, start));
                self.set_preview(Some(dr), cx);
            }
            _ => {
                self.update_hover(cx);
                self.inspection_hover(cx);
                if !matches!(self.tool, Tool::Select) {
                    // Placing a dimension or annotation: align with the others
                    // (Shift turns inferencing off).
                    self.infer_lines.clear();
                    if !e.modifiers.shift && self.pending_annotation().is_some() {
                        self.infer_placement(None);
                    } else if !e.modifiers.shift && self.placing_table() {
                        self.infer_table_placement();
                    }
                    self.update_preview(cx);
                }
            }
        }
    }

    /// A table tool waiting for the placing click.
    pub fn placing_table(&self) -> bool {
        match &self.tool {
            Tool::Table { kind: TableTool::Custom, .. } => self.custom_dlg_placing(),
            Tool::Table { kind: TableTool::General | TableTool::Revision, .. } => true,
            Tool::Table { kind: TableTool::Hole, view: Some(_), origin, .. } => origin.is_some(),
            Tool::Table { view: Some(_), .. } => true,
            _ => false,
        }
    }

    /// Placing a table: the cursor (the table's fixed corner) snaps to the
    /// x / y of the border frame, the title block, views and other tables
    /// nearby, shown as dashed inference lines.
    pub fn infer_table_placement(&mut self) {
        let tol = self.tol_mm(6.0);
        let c = self.cursor;
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        if let Some(b) = &self.built {
            let (w, h) = (b.geometry.width, b.geometry.height);
            xs.extend([10.0, w - 10.0]);
            ys.extend([10.0, h - 10.0]);
            if let Some(r) = b.title_rect {
                xs.extend([r[0], r[2]]);
                ys.extend([r[1], r[3]]);
            }
            for p in &b.placements {
                xs.extend([p.min[0], p.max[0]]);
                ys.extend([p.min[1], p.max[1]]);
            }
            for (_, l) in &b.tables {
                xs.extend([l.min()[0], l.max()[0]]);
                ys.extend([l.min()[1], l.max()[1]]);
            }
        }
        let best = |v: &[f64], x: f64| v.iter().copied().filter(|a| (a - x).abs() < tol).min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()));
        let mut snapped = c;
        if let Some(x) = best(&xs, c[0]) {
            snapped[0] = x;
        }
        if let Some(y) = best(&ys, c[1]) {
            snapped[1] = y;
        }
        if snapped[0] != c[0] {
            self.infer_lines.push(([snapped[0], snapped[1] + 40.0], snapped));
        }
        if snapped[1] != c[1] {
            self.infer_lines.push(([snapped[0] - 40.0, snapped[1]], snapped));
        }
        self.cursor = snapped;
    }

    /// Onshape's placement inferencing: the cursor snaps to the x / y of
    /// other dimensions and annotations nearby, shown as red dashed lines.
    pub fn infer_placement(&mut self, skip: Option<u64>) {
        let tol = self.tol_mm(6.0);
        let c = self.cursor;
        let sheet = self.current_sheet();
        let pts: Vec<[f64; 2]> = sheet.annotations.iter().filter(|a| Some(a.id) != skip).map(|a| a.position).chain(sheet.tables.iter().map(|t| t.position)).collect();
        let best = |axis: usize| pts.iter().filter(|p| (p[axis] - c[axis]).abs() < tol).min_by(|a, b| (a[axis] - c[axis]).abs().total_cmp(&(b[axis] - c[axis]).abs())).copied();
        let (bx, by) = (best(0), best(1));
        let mut snapped = c;
        if let Some(p) = bx {
            snapped[0] = p[0];
        }
        if let Some(p) = by {
            snapped[1] = p[1];
        }
        if let Some(p) = bx {
            self.infer_lines.push((p, snapped));
        }
        if let Some(p) = by {
            self.infer_lines.push((p, snapped));
        }
        self.cursor = snapped;
    }

    pub fn on_mouse_up(&mut self, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
        if e.button == MouseButton::Left && self.zoom_window_up(cx) {
            return;
        }
        if e.button == MouseButton::Left && self.image_mouse_up(cx) {
            return;
        }
        // Placement inference also applies to the click that places.
        self.infer_lines.clear();
        if e.button == MouseButton::Left && !e.modifiers.shift && matches!(self.drag, Drag::Pending { .. }) && !matches!(self.tool, Tool::Select) && self.pending_annotation().is_some() {
            self.infer_placement(None);
            self.infer_lines.clear();
        } else if e.button == MouseButton::Left && !e.modifiers.shift && matches!(self.drag, Drag::Pending { .. }) && self.placing_table() {
            self.infer_table_placement();
            self.infer_lines.clear();
        }
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Pending { button: MouseButton::Left, pick, .. } => {
                if matches!(self.tool, Tool::Select) {
                    self.select_click(pick, e.modifiers, cx);
                } else {
                    self.tool_click(window, cx);
                }
            }
            Drag::Pending { button: MouseButton::Right, pick, .. } => self.open_context_menu(pick, e.position, window, cx),
            Drag::MoveViews { .. } => {
                if let Some(p) = self.preview.take() {
                    self.commit("Move view", p, cx);
                }
            }
            Drag::MoveAnnotation { orig, .. } => {
                if let Some(mut p) = self.preview.take() {
                    self.group_callout_on_drop(&orig, &mut p);
                    self.commit("Move annotation", p, cx);
                }
            }
            Drag::MoveInspection(id) => self.drop_inspection(id, cx),
            Drag::MoveSketch { .. } | Drag::MoveImage { .. } => {
                if let Some(p) = self.preview.take() {
                    self.commit(if matches!(drag, Drag::MoveImage { .. }) { "Move image" } else { "Move sketch" }, p, cx);
                }
            }
            Drag::MoveTable { .. } | Drag::ResizeTable { .. } | Drag::TableDivider { .. } | Drag::MoveSectionLine { .. } | Drag::MoveDetailCircle { .. } | Drag::MoveSectionPoint { .. } | Drag::NoteGrip { .. } => {
                let name = match drag {
                    Drag::MoveTable { .. } => "Move table",
                    Drag::TableDivider { .. } => "Resize cells",
                    Drag::ResizeTable { .. } => "Resize table",
                    Drag::MoveSectionLine { .. } => "Move cutting line",
                    Drag::MoveSectionPoint { .. } => "Edit cutting line",
                    Drag::NoteGrip { grip: 2, .. } => "Rotate note",
                    Drag::NoteGrip { .. } => "Resize note",
                    _ => "Move detail circle",
                };
                if let Some(p) = self.preview.take() {
                    self.commit(name, p, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// Wheel / two-finger scroll: zoom about the pointer with Onshape's amounts
    /// (navigation.rs); horizontal scrolls do nothing.
    pub fn on_scroll(&mut self, e: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        let reverse = nav::MouseControls::current().reverse_scroll;
        let Some(amount) = nav::wheel_zoom_amount(e.delta, nav::Platform::current(), reverse, e.modifiers.shift) else { return };
        self.zoom_by(1.0 / nav::zoom_scale(amount), lp, cx);
    }

    /// Trackpad pinch: the sheet scales with the fingers about their centre.
    pub fn on_pinch(&mut self, e: &PinchEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if e.delta == 0.0 {
            return;
        }
        let lp = self.local(e.position);
        self.zoom_by(nav::pinch_ratio(e.delta), lp, cx);
    }

    pub fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        // Ctrl+M: maximum / minimum dimension (Onshape).
        if (k.modifiers.control || k.modifiers.platform) && !k.modifiers.shift && k.key == "m" && self.panel.is_none() {
            self.set_tool(Tool::Dimension { kind: DimTool::MaxMin, picks: vec![] }, cx);
            cx.stop_propagation();
            return;
        }
        // Ctrl+S: Sheets panel (Onshape's "Display sheet menu").
        if (k.modifiers.control || k.modifiers.platform) && !k.modifiers.shift && k.key == "s" {
            self.toggle_sheets_panel(cx);
            cx.stop_propagation();
            return;
        }
        // Typing in a dialog's text box (focus inside the editor, not on it)
        // never triggers drawing shortcuts.
        if k.modifiers.control || k.modifiers.platform || k.modifiers.alt || self.panel.is_some() || (self.focus.contains_focused(window, cx) && !self.focus.is_focused(window)) {
            return;
        }
        // Shift+Q: midpoints and quad points on / off for the current dimension command.
        if k.modifiers.shift && k.key == "q" && matches!(self.tool, Tool::Dimension { .. }) {
            self.snap_mid_quad = !self.snap_mid_quad;
            self.status = Some(if self.snap_mid_quad { "Midpoints and quad points on".into() } else { "Midpoints and quad points off".into() });
            self.update_preview(cx);
            cx.stop_propagation();
            return;
        }
        match k.key.as_str() {
            "enter" => {
                self.confirm(window, cx);
                cx.stop_propagation();
                return;
            }
            "pagedown" | "pageup" | "home" | "end" => {
                let n = self.drawing.sheets.len();
                let i = match k.key.as_str() {
                    "pagedown" => (self.sheet + 1).min(n - 1),
                    "pageup" => self.sheet.saturating_sub(1),
                    "home" => 0,
                    _ => n - 1,
                };
                self.switch_sheet(i, cx);
                cx.stop_propagation();
                return;
            }
            _ => {}
        }
        let tool = match (k.key.as_str(), k.modifiers.shift) {
            ("d", false) => Tool::Dimension { kind: DimTool::Linear, picks: vec![] },
            ("d", true) => Tool::Dimension { kind: DimTool::Diameter, picks: vec![] },
            ("r", true) => Tool::Dimension { kind: DimTool::Radius, picks: vec![] },
            ("h", true) => Tool::Dimension { kind: DimTool::Foreshorten, picks: vec![] },
            // Drawing sketch tools and constraints (Onshape's drawing shortcuts).
            ("l", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::Line, pts: vec![], from: None },
            ("c", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::Circle, pts: vec![], from: None },
            ("g", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::CornerRect, pts: vec![], from: None },
            ("r", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::CenterRect, pts: vec![], from: None },
            ("m", false) => Tool::Sketch { tool: crate::drawing_sketch::SkTool::Trim, pts: vec![], from: None },
            ("h", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Horizontal, picks: vec![] },
            ("v", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Vertical, picks: vec![] },
            ("i", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Coincident, picks: vec![] },
            ("l", true) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Perpendicular, picks: vec![] },
            ("b", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Parallel, picks: vec![] },
            ("t", false) => Tool::Constrain { kind: crate::drawing_sketch::SkCon::Tangent, picks: vec![] },
            _ => return,
        };
        self.set_tool(tool, cx);
        cx.stop_propagation();
    }

    fn select_click(&mut self, pick: Option<u32>, m: Modifiers, cx: &mut Context<Self>) {
        if self.place_paste_table(cx) {
            return;
        }
        let sel = self.sel_at(pick, self.cursor);
        if self.table_cell_click(sel, m.shift, cx) {
            return;
        }
        let additive = m.shift || m.control || m.platform;
        match sel {
            Some(s) if additive => {
                if let Some(i) = self.selection.iter().position(|x| *x == s) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(s);
                }
            }
            Some(s) => self.selection = vec![s],
            None if !additive => self.selection.clear(),
            None => {}
        }
        self.status = None;
        self.redraw(cx);
    }

    /// Alt+drag (Onshape: copy the hovered entity): leave a duplicate in place.
    fn alt_duplicate(&mut self, pick: Option<u32>, start: [f64; 2], cx: &mut Context<Self>) {
        let Some(sel) = self.sel_at(pick, start) else { return };
        match sel {
            Sel::Annotation(id) => self.edit_sheet("Copy annotation", cx, |s| {
                if let Some(mut a) = s.annotations.iter().find(|a| a.id == id).cloned() {
                    a.id = s.next_annotation_id();
                    s.annotations.push(a);
                }
            }),
            Sel::Table(id) => self.edit_sheet("Copy table", cx, |s| {
                if let Some(mut t) = s.tables.iter().find(|t| t.id == id).cloned() {
                    t.id = s.next_table_id();
                    s.tables.push(t);
                }
            }),
            Sel::View(id) => {
                if let Ok(clip) = self.drawing.copy_views(self.sheet, &[id], Some(&self.sources)) {
                    self.edit_sheet("Copy view", cx, |s| {
                        // The copy keeps the original's place: paste around the same center.
                        let n = clip.sheet.views.len().max(1) as f64;
                        let c = clip.sheet.views.iter().fold([0.0, 0.0], |acc, v| [acc[0] + v.position[0] / n, acc[1] + v.position[1] / n]);
                        s.paste_views(&clip, Some(c));
                    });
                }
            }
            Sel::Break(..) | Sel::Sketch(_) | Sel::Hatch(_) | Sel::Image(_) => {}
        }
    }

    fn start_drag(&mut self, pick: Option<u32>, start: [f64; 2]) -> Drag {
        if let Some(d) = self.note_grip_drag(start) {
            return d;
        }
        if let Some(d) = self.extra_start_drag(pick, start) {
            return d;
        }
        if let Some(id) = self.inspection_at(pick) {
            return Drag::MoveInspection(id);
        }
        let Some(sel) = self.sel_at(pick, start) else { return Drag::None };
        if !self.selection.contains(&sel) {
            self.selection = vec![sel];
        }
        let sheet = &self.drawing.sheets[self.sheet];
        match sel {
            Sel::View(id) => {
                // Move the view with all views derived from it; a projected
                // (or section/auxiliary) view slides along its alignment axis.
                // Views with "Suppress alignment with parent" stay where they are.
                let ids: Vec<ViewId> = sheet.descendants(id).into_iter().filter(|v| *v == id || !sheet.view(*v).is_some_and(|x| x.unaligned)).collect();
                let orig = ids.iter().filter_map(|v| sheet.view(*v).map(|x| (*v, x.position))).collect();
                let prot = sheet.parent_of(id).map(|p| views::view_rotation(sheet, p)).unwrap_or(0.0);
                let unaligned = sheet.view(id).is_some_and(|v| v.unaligned);
                let constraint = match &sheet.view(id).map(|v| v.kind.clone()).filter(|_| !unaligned) {
                    Some(ViewKind::Projected { .. }) => views::projected_direction(sheet, id).filter(|d| d[0] == 0.0 || d[1] == 0.0).map(|d| views::rotate2(d, prot)).map(|d| (d, d)),
                    Some(ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) => views::section_cut(sheet, id).map(|c| views::rotate2(c.toward_view, prot)).map(|d| (d, d)),
                    Some(ViewKind::Auxiliary { edge_dir, .. }) => {
                        let n = norm([-edge_dir[1], edge_dir[0]]);
                        Some((n, n))
                    }
                    _ => None,
                };
                Drag::MoveViews { start, orig, constraint }
            }
            Sel::Annotation(aid) => match self.built.as_ref().and_then(|b| b.resolved.annotations.iter().find(|a| a.id == aid).cloned()) {
                Some(a) if !matches!(a.kind, AnnotationKind::Centermark { .. } | AnnotationKind::Centerline { .. }) => Drag::MoveAnnotation { start, orig: a },
                _ => Drag::None,
            },
            // Drag a sketch entity (its points move, the sketch solves).
            Sel::Sketch(id) => Drag::MoveSketch { id, start, orig: sheet.sketch.clone() },
            Sel::Image(id) => match sheet.images.iter().find(|i| i.id == id) {
                Some(im) => Drag::MoveImage { id, start, orig: im.pos },
                None => Drag::None,
            },
            Sel::Table(_) | Sel::Break(..) | Sel::Hatch(_) => Drag::None,
        }
    }

    /// The annotation `orig` (resolved) dragged from `start` to the cursor.
    fn dragged_annotation(&self, orig: &Annotation, start: [f64; 2]) -> Annotation {
        let c = self.cursor;
        let d = sub(c, start);
        let mut a = orig.clone();
        let pl = a.kind.view().and_then(|v| self.placement(v));
        match &mut a.kind {
            AnnotationKind::LinearDimension { a: pa, b: pb, offset, orientation, .. } => {
                if let Some(pl) = pl {
                    *offset = dim_offset(pl.to_sheet(*pa), pl.to_sheet(*pb), *orientation, c);
                }
            }
            AnnotationKind::RadialDimension { center, angle, .. } => {
                if let Some(pl) = pl {
                    let v = sub(c, pl.to_sheet(*center));
                    *angle = v[1].atan2(v[0]);
                }
            }
            AnnotationKind::AngularDimension { vertex, radius, .. } => {
                if let Some(pl) = pl {
                    *radius = len(sub(c, pl.to_sheet(*vertex))).max(3.0);
                }
            }
            AnnotationKind::MaxMinDimension { a: pa, b: pb, offset, orientation, .. } => {
                if let Some(pl) = pl {
                    *offset = dim_offset(pl.to_sheet(*pa), pl.to_sheet(*pb), *orientation, c);
                }
            }
            AnnotationKind::ForeshortenedRadial { jog, .. } => {
                // Dragging moves the false center.
                if let Some(pl) = pl {
                    *jog = pl.to_view(add2(pl.to_sheet(*jog), d));
                }
            }
            AnnotationKind::SurfaceFinish { anchor, .. } | AnnotationKind::Gdt { anchor, .. } | AnnotationKind::RevisionCallout { anchor, .. } => {
                *anchor = [anchor[0] + d[0], anchor[1] + d[1]];
            }
            _ => a.position = [orig.position[0] + d[0], orig.position[1] + d[1]],
        }
        // Keep the stored (unresolved) inputs of the committed annotation;
        // only the placement changes.
        if let Some(committed) = self.drawing.sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|x| x.id == a.id)) {
            let mut out = committed.clone();
            out.position = a.position;
            match (&mut out.kind, &a.kind) {
                (AnnotationKind::LinearDimension { offset, .. }, AnnotationKind::LinearDimension { offset: o, .. }) => *offset = *o,
                (AnnotationKind::RadialDimension { angle, .. }, AnnotationKind::RadialDimension { angle: g, .. }) => *angle = *g,
                (AnnotationKind::AngularDimension { radius, .. }, AnnotationKind::AngularDimension { radius: r, .. }) => *radius = *r,
                (AnnotationKind::SurfaceFinish { anchor, .. }, AnnotationKind::SurfaceFinish { anchor: n, .. }) => *anchor = *n,
                (AnnotationKind::Gdt { anchor, .. }, AnnotationKind::Gdt { anchor: n, .. }) => *anchor = *n,
                (AnnotationKind::RevisionCallout { anchor, .. }, AnnotationKind::RevisionCallout { anchor: n, .. }) => *anchor = *n,
                (AnnotationKind::MaxMinDimension { offset, .. }, AnnotationKind::MaxMinDimension { offset: o, .. }) => *offset = *o,
                (AnnotationKind::ForeshortenedRadial { jog, .. }, AnnotationKind::ForeshortenedRadial { jog: j, .. }) => *jog = *j,
                _ => {}
            }
            return out;
        }
        a
    }

    fn double_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_hover(cx);
        let Some(h) = self.hover else { return };
        match self.built.as_ref().and_then(|b| b.picks.get(h as usize)).cloned() {
            Some(PickTarget::TitleBlock) => self.open_title_panel(window, cx),
            Some(PickTarget::Inspection(id)) => self.edit_inspection(id, window, cx),
            Some(PickTarget::Annotation(id)) => {
                let note = self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).cloned();
                match note {
                    Some(Annotation { kind: AnnotationKind::Note { .. }, position, .. }) => self.open_note_panel(Some(id), position, window, cx),
                    Some(Annotation { kind: AnnotationKind::Weld { .. }, .. }) => self.open_weld_edit(id, window, cx),
                    Some(a) => self.open_annotation_panel(&a, window, cx),
                    None => {}
                }
            }
            Some(PickTarget::Table(id)) => {
                // On a divider: fit it to the text; in a cell: edit the cell;
                // a custom table's fixed corner: its properties (help).
                if self.custom_corner_at(id) {
                    self.open_custom_props(id, cx);
                } else if !self.fit_table_divider(id, cx) {
                    self.open_cell_panel(id, window, cx)
                }
            }
            Some(t) => {
                // Onshape: double-clicking a view opens View properties.
                if let Some(v) = t.view() {
                    self.open_view_props(v, window, cx);
                }
            }
            None => {}
        }
    }

    // ------------------------------------------------------------ context menu

    fn open_context_menu(&mut self, pick: Option<u32>, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.tool, Tool::Select) {
            self.cancel(cx);
            return;
        }
        let sel = self.sel_at(pick, self.cursor);
        if let Some(s) = sel
            && !self.selection.contains(&s)
        {
            self.selection = vec![s];
            self.redraw(cx);
        }
        let entries = self.context_entries(sel, cx);
        let ed = cx.entity();
        // Onshape's right-click menu (os_menu: 12 px, 22 px rows).
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
        if !menu.read(cx).is_empty() {
            crate::os_menu::open(position, menu, window, cx);
        }
        cx.notify();
    }

    fn context_entries(&self, sel: Option<Sel>, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        // The auxiliary view reference indicator has its own menu.
        let on_indicator = self.hover.and_then(|h| self.built.as_ref()?.picks.get(h as usize).cloned());
        if let (Some(Sel::View(id)), Some(PickTarget::AuxIndicator(_))) = (sel, on_indicator) {
            return vec![
                MenuEntry::item("Edit auxiliary view", move |t, w, cx| t.edit_aux_view(id, w, cx)),
                MenuEntry::item("Flip direction", move |t, _, cx| t.edit_view("Flip direction", id, cx, |v| v.flip = !v.flip)),
                MenuEntry::Separator,
                MenuEntry::item("Clear selection", |t, _, cx| {
                    t.selection.clear();
                    t.redraw(cx);
                }),
                MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
                MenuEntry::item("Delete", move |t, _, cx| {
                    t.selection.clear();
                    t.edit_sheet("Delete view", cx, |s| s.remove_view(id));
                }),
            ];
        }
        match sel {
            Some(Sel::View(id)) => out = self.onshape_view_entries(id, cx),
            Some(Sel::Annotation(id)) => {
                let a = self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).cloned();
                if let Some(a) = &a {
                    out.extend(self.inspection_entries(a));
                    if let AnnotationKind::Note { .. } = a.kind {
                        let pos = a.position;
                        out.push(MenuEntry::item("Edit note…", move |t, w, cx| t.open_note_panel(Some(id), pos, w, cx)));
                    }
                    if let AnnotationKind::LinearDimension { orientation, .. } = a.kind {
                        for (label, o) in [("Horizontal", DimOrientation::Horizontal), ("Vertical", DimOrientation::Vertical), ("Aligned", DimOrientation::Aligned)] {
                            out.push(MenuEntry::check(label, orientation == o, move |t, _, cx| {
                                t.edit_sheet("Dimension orientation", cx, |s| {
                                    if let Some(AnnotationKind::LinearDimension { orientation, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                        *orientation = o;
                                    }
                                })
                            }));
                        }
                    }
                    out.extend(self.annotation_more_entries(a));
                    if let AnnotationKind::RadialDimension { diameter, .. } = a.kind {
                        out.push(MenuEntry::item(if diameter { "Show as radius" } else { "Show as diameter" }, move |t, _, cx| {
                            t.edit_sheet("Radial dimension", cx, |s| {
                                if let Some(AnnotationKind::RadialDimension { diameter, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                    *diameter = !*diameter;
                                }
                            })
                        }));
                    }
                    out.extend(self.annotation_extra_entries(a));
                    // Group / Remove from group (with a selected view), Group
                    // annotation / Remove annotation from group (notes, callouts).
                    if matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. }) {
                        let sel_view = self.selection.iter().find_map(|s| if let Sel::View(v) = s { Some(*v) } else { None });
                        let anns: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(x) = s { Some(*x) } else { None }).chain([id]).collect();
                        if a.view_group.is_some() {
                            out.push(MenuEntry::item("Remove from group", move |t, _, cx| t.edit_sheet("Remove from group", cx, |s| s.annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.view_group = None))));
                        } else if let Some(v) = sel_view {
                            let ids = anns.clone();
                            out.push(MenuEntry::item("Group", move |t, _, cx| t.edit_sheet("Group", cx, |s| s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.view_group = Some(v)))));
                        }
                        if a.ann_group.is_some() {
                            let ids = anns.clone();
                            out.push(MenuEntry::item("Remove annotation from group", move |t, _, cx| t.edit_sheet("Remove annotation from group", cx, |s| s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.ann_group = None))));
                        } else if anns.iter().collect::<std::collections::HashSet<_>>().len() >= 2 {
                            let ids = anns.clone();
                            out.push(MenuEntry::item("Group annotation", move |t, _, cx| {
                                let g = t.current_sheet().annotations.iter().filter_map(|x| x.ann_group).max().unwrap_or(0) + 1;
                                t.edit_sheet("Group annotation", cx, |s| s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.ann_group = Some(g)));
                            }));
                        }
                    }
                    // Copy (with the rest of the selection).
                    if matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. } | AnnotationKind::Gdt { .. } | AnnotationKind::Datum { .. } | AnnotationKind::DatumTarget { .. } | AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Weld { .. } | AnnotationKind::HoleCallout { .. }) {
                        out.push(MenuEntry::item("Copy", move |t, _, cx| {
                            if !t.selection.contains(&Sel::Annotation(id)) {
                                t.selection = vec![Sel::Annotation(id)];
                            }
                            t.copy_entities(cx);
                        }));
                    }
                    // Move to ▸ a format layer (or back to the drawing).
                    if matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. } | AnnotationKind::Gdt { .. } | AnnotationKind::Datum { .. } | AnnotationKind::DatumTarget { .. } | AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Weld { .. }) {
                        let cur = a.layer;
                        let layers = SheetLayer::FORMAT.iter().copied().chain((!cur.is_drawing()).then_some(SheetLayer::Drawing));
                        let items = layers
                            .filter(|l| *l != cur)
                            .map(|l| {
                                MenuEntry::item(l.name(), move |t, _, cx| {
                                    let ids: Vec<u64> = t.selection.iter().filter_map(|s| if let Sel::Annotation(x) = s { Some(*x) } else { None }).chain([id]).collect();
                                    t.edit_sheet("Move to", cx, |s| s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.layer = l));
                                })
                            })
                            .collect();
                        out.push(MenuEntry::Submenu("Move to".into(), items));
                    }
                    if !a.refs.is_empty() {
                        out.push(MenuEntry::item("Detach from geometry", move |t, _, cx| {
                            t.edit_sheet("Detach", cx, |s| {
                                if let Some(x) = s.annotations.iter_mut().find(|x| x.id == id) {
                                    x.refs.clear();
                                }
                            })
                        }));
                    }
                    out.push(MenuEntry::Separator);
                }
                out.push(MenuEntry::item("Delete", move |t, _, cx| {
                    t.selection.clear();
                    t.edit_sheet("Delete annotation", cx, |s| s.annotations.retain(|x| x.id != id));
                }));
            }
            Some(Sel::Table(id)) => out.extend(self.table_entries(id, cx)),
            Some(Sel::Sketch(id)) => out.extend(self.sketch_entries(id)),
            Some(Sel::Image(id)) => out.extend(self.image_entries(id)),
            Some(Sel::Hatch(id)) => {
                out.push(MenuEntry::item("Edit hatch", move |t, _, cx| t.open_hatch_dialog(Some(id), cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Delete", move |t, _, cx| {
                    t.selection.clear();
                    t.edit_sheet("Delete hatch", cx, |s| s.sketch.hatches.retain(|h| h.id != id));
                }));
            }
            Some(Sel::Break(view, index)) => out.extend(self.break_entries(view, index)),
            None => {
                // Onshape's empty-sheet menu: (Paste) | Sheet properties…, Drawing
                // properties… | Zoom to fit, Zoom to window, Zoom to sheet; then
                // noshape's sheet setup shortcuts.
                if crate::drawing_props::view_clipboard(cx).is_some() {
                    out.push(MenuEntry::item("Paste", |t, _, cx| t.paste_views(cx)));
                }
                if self.clipboard_cells(cx).is_some() {
                    out.push(MenuEntry::item("Paste as table", |t, _, cx| {
                        t.start_paste_table(cx);
                    }));
                }
                let si = self.sheet;
                out.push(MenuEntry::item("Sheet properties…", move |t, w, cx| t.open_sheet_props(si, w, cx)));
                out.push(MenuEntry::item("Drawing properties…", |t, _, cx| t.open_drawing_props(cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
                out.push(MenuEntry::item("Zoom to window", |t, _, cx| t.start_zoom_window(cx)));
                out.push(MenuEntry::item("Zoom to sheet", |t, _, cx| t.zoom_to_fit(cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Update drawing", |t, _, cx| t.update_drawing(cx)));
                out.push(MenuEntry::item("Insert note", |t, _, cx| t.set_tool(Tool::Note, cx)));
                out.push(MenuEntry::item("Edit title block…", |t, w, cx| t.open_title_panel(w, cx)));
                out.push(MenuEntry::Submenu("Sheet size".into(), self.sheet_size_entries()));
                out.push(MenuEntry::Submenu("Template".into(), self.template_entries()));
                out.push(MenuEntry::Submenu("Title block".into(), self.title_style_entries()));
                out.push(MenuEntry::item("Sheet scale…", |t, w, cx| t.open_scale_panel(None, w, cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Insert table", |t, _, cx| t.set_tool(Tool::Table { kind: TableTool::General, view: None, origin: None }, cx)));
                if self.has_revision_table() {
                    let rev = self.latest_revision();
                    out.push(MenuEntry::item("Add revision callout", move |t, _, cx| t.set_tool(Tool::RevisionCallout { revision: rev.clone() }, cx)));
                }
                out.push(MenuEntry::item("Add sheet", move |t, _, cx| t.add_sheet_after(si, cx)));
            }
        }
        out
    }

    pub fn edit_view(&mut self, description: &str, id: ViewId, cx: &mut Context<Self>, f: impl FnOnce(&mut DrawingView)) {
        self.edit_sheet(description, cx, |s| {
            if let Some(v) = s.view_mut(id) {
                f(v);
            }
        });
    }

    pub fn sheet_size_entries(&self) -> Vec<MenuEntry> {
        let cur = self.current_sheet().size;
        SheetSize::STANDARD
            .into_iter()
            .map(|s| {
                let (w, h) = s.size_mm();
                MenuEntry::check(format!("{s:?}  ({w} × {h} mm)"), s == cur, move |t, _, cx| {
                    t.edit_sheet("Sheet size", cx, |sh| sh.size = s);
                    t.fitted = false;
                })
            })
            .collect()
    }
}

impl DrawingEditor {
    /// A chamfer dimension under Dimensions style (Drawing properties ›
    /// Dimensions › Chamfer, 45° or not): its two legs as aligned linear
    /// dimensions (live drawing/255–259); None for Note style.
    fn chamfer_as_dimensions(&self, a: &Annotation) -> Option<Vec<Annotation>> {
        let AnnotationKind::ChamferDimension { view, a: pa, b: pb, c: pc, d: pd } = a.kind else { return None };
        let u = [pb[0] - pa[0], pb[1] - pa[1]];
        let w = [pd[0] - pc[0], pd[1] - pc[1]];
        let (lu, lw) = (len(u), len(w));
        if lu < 1e-12 || lw < 1e-12 {
            return None;
        }
        let wn = [w[0] / lw, w[1] / lw];
        let along = u[0] * wn[0] + u[1] * wn[1];
        let ang = (along.abs() / lu).min(1.0).acos().to_degrees();
        let p = &self.drawing.props;
        let as_dims = if (ang - 45.0).abs() < 0.05 { p.chamfer_45_dimension } else { p.chamfer_dimension };
        if !as_dims {
            return None;
        }
        let corner = [pa[0] + wn[0] * along, pa[1] + wn[1] * along];
        let id = a.id;
        let leg = |k: u64, x: [f64; 2], y: [f64; 2], slot: Option<(u8, u8)>| {
            let refs: Vec<AnnotationRef> = match slot {
                Some((from, to)) => a.refs.iter().filter(|r| r.slot == from).map(|r| AnnotationRef { slot: to, point: r.point.clone() }).collect(),
                None => Vec::new(),
            };
            Annotation { id: id + k, kind: AnnotationKind::LinearDimension { view, a: x, b: y, offset: 6.0, orientation: DimOrientation::Aligned, override_text: None }, position: a.position, refs, ..Default::default() }
        };
        Some(vec![leg(0, pa, corner, Some((0, 0))), leg(1, corner, pb, Some((1, 1)))])
    }
}

