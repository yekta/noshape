//! Drawing editor input: mouse/keyboard handling, selection and dragging,
//! and the view / dimension / annotation tools (with live previews).

use crate::drawing_scene::Overlay;
use crate::drawing_view::*;
use gpui_kit::component::menu::PopupMenu;
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
fn auto_orientation(p: [f64; 2], q: [f64; 2], c: [f64; 2]) -> DimOrientation {
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
fn intersect(a0: [f64; 2], a1: [f64; 2], b0: [f64; 2], b1: [f64; 2]) -> Option<[f64; 2]> {
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
        if std::mem::discriminant(&tool) == std::mem::discriminant(&self.tool) && !matches!(tool, Tool::Select | Tool::InsertView { .. } | Tool::Dimension { .. }) {
            self.tool = Tool::Select;
        } else {
            self.tool = tool;
        }
        // Tools that start from the selected view.
        let sel_view = self.selection.iter().find_map(|s| if let Sel::View(v) = s { Some(*v) } else { None });
        match &mut self.tool {
            Tool::Projected { parent } | Tool::Section { parent, .. } | Tool::Detail { parent, .. } if parent.is_none() => *parent = sel_view,
            _ => {}
        }
        self.overlay = Overlay::default();
        self.hover_snap = None;
        self.status = None;
        self.ensure_sources(false, cx);
        self.update_preview(cx);
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.panel.is_some() {
            self.panel = None;
            cx.notify();
            return;
        }
        if !matches!(self.tool, Tool::Select) {
            self.tool = Tool::Select;
            self.overlay = Overlay::default();
            self.hover_snap = None;
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
            for x in &sel {
                match x {
                    Sel::View(v) => s.remove_view(*v),
                    Sel::Annotation(a) => s.annotations.retain(|n| n.id != *a),
                }
            }
        });
    }

    /// Prompt shown in the status bar for the active tool.
    pub fn prompt(&self) -> Option<&'static str> {
        Some(match &self.tool {
            Tool::Select => return None,
            Tool::InsertView { .. } => "Click to place the view",
            Tool::Projected { parent: None } => "Select the view to project from",
            Tool::Projected { .. } => "Move away from the parent view and click to place (Esc to finish)",
            Tool::Auxiliary { parent: None } => "Select a straight edge to fold about",
            Tool::Auxiliary { .. } => "Click to place the auxiliary view",
            Tool::Section { parent: None, .. } | Tool::Section { p0: None, .. } => "Click the first point of the cutting line in a view",
            Tool::Section { line: None, .. } => "Click the second point of the cutting line",
            Tool::Section { .. } => "Drag to the side to view from and click to place",
            Tool::Detail { center: None, .. } => "Click the center of the detail circle",
            Tool::Detail { radius: None, .. } => "Click to set the detail radius",
            Tool::Detail { .. } => "Click to place the detail view",
            Tool::Dimension { kind: DimTool::Radius | DimTool::Diameter, picks } if picks.is_empty() => "Select a circle or arc",
            Tool::Dimension { kind: DimTool::Angle, picks } if picks.len() < 2 => "Select two straight edges",
            Tool::Dimension { picks, .. } if picks.is_empty() => "Select a point or a straight edge",
            Tool::Dimension { picks, .. } if picks.len() == 1 && matches!(picks[0], Pick::Point(_)) => "Select the second point",
            Tool::Dimension { .. } => "Click to place the dimension",
            Tool::Note => "Click to place the note",
            Tool::Centermark => "Select a circle or arc",
            Tool::Centerline { first: None } => "Select the first point",
            Tool::Centerline { .. } => "Select the second point",
            Tool::Balloon { anchor: None } => "Select the point the callout points to",
            Tool::Balloon { .. } => "Click to place the callout",
        })
    }

    // ------------------------------------------------------------ picking

    /// The entity under the cursor: the picked geometry/annotation, else the
    /// view whose rectangle contains the point.
    pub fn sel_at(&self, pick: Option<u32>, s: [f64; 2]) -> Option<Sel> {
        if let Some(sel) = pick.and_then(|p| self.sel_of_pick(p)) {
            return Some(sel);
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
        }
        best.map(|(_, p)| p)
    }

    /// The hovered projected edge: (view, model edge).
    fn hovered_edge(&self) -> Option<(ViewId, views::ModelEdge)> {
        let h = self.hover?;
        let PickTarget::Edge { view, edge } = self.built.as_ref()?.picks.get(h as usize)?.clone() else { return None };
        let g = self.geoms.get(&view)?;
        let m = g.edges.get(edge as usize)?.model?;
        Some((view, g.model_edges.get(m as usize)?.clone()))
    }

    fn line_pick(&self) -> Option<Pick> {
        let (view, me) = self.hovered_edge()?;
        let EdgeGeom::Line { a, b } = me.geom else { return None };
        let pa = me.ends[0].clone().map(|(r, p)| PickedPoint { view, p, r: Some(r) }).unwrap_or(PickedPoint { view, p: a, r: None });
        let pb = me.ends[1].clone().map(|(r, p)| PickedPoint { view, p, r: Some(r) }).unwrap_or(PickedPoint { view, p: b, r: None });
        if len(sub(pa.p, pb.p)) < 1e-9 {
            return None;
        }
        Some(Pick::Line { a: pa, b: pb })
    }

    fn circle_pick(&self, s: [f64; 2]) -> Option<Pick> {
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
        g.circles()
            .into_iter()
            .filter(|(c, r, _)| (len(sub(vp, *c)) - r).abs() < tol)
            .min_by(|a, b| (len(sub(vp, a.0)) - a.1).abs().total_cmp(&(len(sub(vp, b.0)) - b.1).abs()))
            .map(|(center, radius, r)| Pick::Circle { view, center, radius, r })
    }

    /// A point in a view: snap, else the point on a hovered edge, else free.
    fn any_point(&self, s: [f64; 2]) -> Option<PickedPoint> {
        if let Some(p) = self.snap_at(s) {
            return Some(p);
        }
        let view = self.view_at(s)?;
        let pl = self.placement(view)?;
        Some(PickedPoint { view, p: pl.to_view(s), r: None })
    }

    // ------------------------------------------------------------ previews

    fn pick_sheet(&self, p: &PickedPoint) -> Option<[f64; 2]> {
        Some(self.placement(p.view)?.to_sheet(p.p))
    }

    /// The annotation the active dimension tool would create at the cursor.
    fn pending_annotation(&self) -> Option<Annotation> {
        let c = self.cursor;
        let id = self.current_sheet().next_annotation_id();
        let ann = |kind, refs: Vec<(u8, Option<PointRef>)>| Annotation {
            id,
            kind,
            position: c,
            refs: refs.into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect(),
        };
        match &self.tool {
            Tool::Dimension { kind: kind @ (DimTool::Linear | DimTool::Aligned | DimTool::Horizontal | DimTool::Vertical), picks } => {
                let (a, b) = match picks.as_slice() {
                    [Pick::Line { a, b }] => (a.clone(), b.clone()),
                    [Pick::Point(a), Pick::Point(b)] => (a.clone(), b.clone()),
                    _ => return None,
                };
                let (ps, qs) = (self.pick_sheet(&a)?, self.pick_sheet(&b)?);
                let o = match kind {
                    DimTool::Aligned => DimOrientation::Aligned,
                    DimTool::Horizontal => DimOrientation::Horizontal,
                    DimTool::Vertical => DimOrientation::Vertical,
                    _ => auto_orientation(ps, qs, c),
                };
                let offset = dim_offset(ps, qs, o, c);
                Some(ann(AnnotationKind::LinearDimension { view: a.view, a: a.p, b: b.p, offset, orientation: o, override_text: None }, vec![(0, a.r), (1, b.r)]))
            }
            Tool::Dimension { kind: kind @ (DimTool::Radius | DimTool::Diameter), picks } => {
                let [Pick::Circle { view, center, radius, r }] = picks.as_slice() else { return None };
                let cs = self.placement(*view)?.to_sheet(*center);
                let d = sub(c, cs);
                Some(ann(AnnotationKind::RadialDimension { view: *view, center: *center, radius: *radius, diameter: *kind == DimTool::Diameter, angle: d[1].atan2(d[0]) }, vec![(0, r.clone())]))
            }
            Tool::Dimension { kind: DimTool::Angle, picks } => {
                let [Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, b: b2 }] = picks.as_slice() else { return None };
                if a1.view != a2.view {
                    return None;
                }
                let v = intersect(a1.p, b1.p, a2.p, b2.p)?;
                let far = |a: &PickedPoint, b: &PickedPoint| if len(sub(a.p, v)) > len(sub(b.p, v)) { a.clone() } else { b.clone() };
                let near = |a: &PickedPoint, b: &PickedPoint| if len(sub(a.p, v)) > len(sub(b.p, v)) { b.clone() } else { a.clone() };
                let (mut pa, mut pb) = (far(a1, b1), far(a2, b2));
                let ang = |p: [f64; 2]| (p[1] - v[1]).atan2(p[0] - v[0]);
                let mut sweep = ang(pb.p) - ang(pa.p);
                while sweep < 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                if sweep > std::f64::consts::PI {
                    std::mem::swap(&mut pa, &mut pb);
                }
                let pl = self.placement(a1.view)?;
                let radius = len(sub(c, pl.to_sheet(v))).max(3.0);
                let (n1, n2) = (near(a1, b1), near(a2, b2));
                let vref = match (&n1.r, &n2.r) {
                    (Some(x), Some(y)) if x == y && len(sub(n1.p, v)) < 1e-7 => Some(x.clone()),
                    _ => None,
                };
                let refs = if vref.is_some() { vec![(0, vref), (1, pa.r.clone()), (2, pb.r.clone())] } else { vec![] };
                Some(ann(AnnotationKind::AngularDimension { view: a1.view, vertex: v, a: pa.p, b: pb.p, radius }, refs))
            }
            Tool::Balloon { anchor: Some(a) } => {
                let n = self.current_sheet().annotations.iter().filter(|x| matches!(x.kind, AnnotationKind::Balloon { .. })).count() + 1;
                Some(ann(AnnotationKind::Balloon { view: a.view, anchor: a.p, text: n.to_string() }, vec![(0, a.r.clone())]))
            }
            _ => None,
        }
    }

    /// The view the active view tool would create at the cursor.
    fn pending_view(&self) -> Option<DrawingView> {
        let c = self.cursor;
        let sheet = &self.drawing.sheets.get(self.sheet)?;
        let id = sheet.next_view_id();
        let mk = |kind: ViewKind, position: [f64; 2], parent: Option<&DrawingView>| DrawingView {
            id,
            kind,
            position,
            scale: None,
            style: parent.map(|p| p.style).unwrap_or(RenderStyle::HiddenLinesRemoved),
            show_tangent_edges: parent.map(|p| p.show_tangent_edges).unwrap_or(true),
            show_centerlines: false,
            label: None,
        };
        match &self.tool {
            Tool::InsertView { source, orientation, part } => {
                Some(mk(ViewKind::Base { source: *source, config: Default::default(), orientation: *orientation, part: part.clone() }, c, None))
            }
            Tool::Projected { parent: Some(pid) } => {
                let parent = sheet.view(*pid)?;
                let d = sub(c, parent.position);
                if len(d) < 15.0 {
                    return None;
                }
                let dir = views::snap_direction(d);
                let pos = if dir[1] == 0.0 {
                    [c[0], parent.position[1]]
                } else if dir[0] == 0.0 {
                    [parent.position[0], c[1]]
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
                Some(mk(ViewKind::Auxiliary { parent: *pid, edge_dir: *edge_dir }, pos, Some(parent)))
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
                Some(v)
            }
            Tool::Detail { parent: Some(pid), center: Some(center), radius: Some(r) } => {
                let parent = sheet.view(*pid)?;
                let mut v = mk(ViewKind::Detail { parent: *pid, center: *center, radius: *r, label: sheet.next_label() }, c, Some(parent));
                v.scale = Some(views::view_scale(sheet, *pid) * 2.0);
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
            Tool::Dimension { kind: DimTool::Linear | DimTool::Aligned | DimTool::Horizontal | DimTool::Vertical, .. }
            | Tool::Centerline { .. }
            | Tool::Balloon { anchor: None }
            | Tool::Section { p0: None, .. }
            | Tool::Detail { center: None, .. } => self.snap_at(c),
            _ => None,
        };
        self.hover_snap = snap.as_ref().and_then(|s| self.pick_sheet(s));
        if let Some(p) = self.hover_snap {
            overlay.points.push(p);
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
                        Pick::Circle { view, center, radius, .. } => {
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
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None } => {
                if let Some(pl) = self.placement(*pid) {
                    let a = pl.to_sheet(*p0);
                    let b = self.ortho_point(a, c);
                    overlay.lines.push((vec![a, b], true));
                }
            }
            Tool::Detail { parent: Some(pid), center: Some(ctr), radius: None } => {
                if let Some(pl) = self.placement(*pid) {
                    let cs = pl.to_sheet(*ctr);
                    overlay.circle(cs, len(sub(c, cs)), true);
                }
            }
            _ => {}
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
        self.overlay = overlay;
        let had_preview = self.preview.is_some();
        if preview.is_some() || had_preview {
            self.set_preview(preview, cx);
        }
        self.redraw(cx);
    }

    /// Snap a second point to horizontal/vertical from `a` when close.
    fn ortho_point(&self, a: [f64; 2], c: [f64; 2]) -> [f64; 2] {
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

    fn tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor;
        let tool = self.tool.clone();
        match tool {
            Tool::Select => {}
            Tool::InsertView { .. } => {
                if let Some(v) = self.pending_view() {
                    let id = v.id;
                    self.edit_sheet("Insert view", cx, |s| s.views.push(v));
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
                if let Some(Pick::Line { a, b }) = self.line_pick()
                    && let Some(pl) = self.placement(a.view)
                {
                    let d = norm(sub(pl.to_sheet(b.p), pl.to_sheet(a.p)));
                    self.tool = Tool::Auxiliary { parent: Some((a.view, d)) };
                } else {
                    self.status = Some("Select a straight edge of a view".into());
                }
            }
            Tool::Section { p0: None, .. } => {
                let pick = self.snap_at(c).or_else(|| self.view_at(c).and_then(|v| self.placement(v).map(|pl| PickedPoint { view: v, p: pl.to_view(c), r: None })));
                if let Some(pp) = pick {
                    self.tool = Tool::Section { parent: Some(pp.view), p0: Some(pp.p), line: None };
                }
            }
            Tool::Section { parent: Some(pid), p0: Some(p0), line: None } => {
                let Some(pl) = self.placement(pid) else { return };
                let a = pl.to_sheet(p0);
                let b = self.ortho_point(a, c);
                if len(sub(b, a)) < 2.0 {
                    return;
                }
                // Extend the cutting line across the whole view.
                let e = norm(sub(b, a));
                let corners = [pl.min, [pl.max[0], pl.min[1]], pl.max, [pl.min[0], pl.max[1]]];
                let ts: Vec<f64> = corners.iter().map(|k| dot(sub(*k, a), e)).collect();
                let t0 = ts.iter().cloned().fold(f64::MAX, f64::min) - 6.0;
                let t1 = ts.iter().cloned().fold(f64::MIN, f64::max) + 6.0;
                let la = pl.to_view([a[0] + e[0] * t0, a[1] + e[1] * t0]);
                let lb = pl.to_view([a[0] + e[0] * t1, a[1] + e[1] * t1]);
                self.tool = Tool::Section { parent: Some(pid), p0: Some(p0), line: Some((la, lb)) };
            }
            Tool::Section { line: Some(_), .. } | Tool::Detail { radius: Some(_), .. } => {
                if let Some(v) = self.pending_view() {
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
                if let Some(a) = self.pending_annotation() {
                    self.edit_sheet(kind.name(), cx, |s| s.annotations.push(a));
                    self.tool = Tool::Dimension { kind, picks: vec![] };
                } else {
                    match kind {
                        DimTool::Radius | DimTool::Diameter => {
                            if let Some(p) = self.circle_pick(c) {
                                picks = vec![p];
                            }
                        }
                        DimTool::Angle => {
                            if let Some(p) = self.line_pick() {
                                picks.push(p);
                            }
                        }
                        _ => {
                            if let Some(p) = self.snap_at(c) {
                                if picks.first().is_some_and(|f| matches!(f, Pick::Point(a) if a.view != p.view)) {
                                    picks.clear();
                                }
                                picks.push(Pick::Point(p));
                            } else if picks.is_empty()
                                && let Some(l) = self.line_pick()
                            {
                                picks.push(l);
                            }
                        }
                    }
                    self.tool = Tool::Dimension { kind, picks };
                }
            }
            Tool::Note => self.open_note_panel(None, c, window, cx),
            Tool::Centermark => {
                if let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(c) {
                    let id = self.current_sheet().next_annotation_id();
                    let refs = r.map(|point| vec![AnnotationRef { slot: 0, point }]).unwrap_or_default();
                    self.edit_sheet("Centermark", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Centermark { view, center, size: radius * 1.25 }, position: c, refs }));
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
                    self.edit_sheet("Centerline", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Centerline { view: a.view, a: a.p, b: b.p }, position: c, refs }));
                    self.tool = Tool::Centerline { first: None };
                }
            }
            Tool::Balloon { anchor: None } => {
                if let Some(p) = self.any_point(c) {
                    self.tool = Tool::Balloon { anchor: Some(p) };
                }
            }
            Tool::Balloon { anchor: Some(_) } => {
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
        if self.menu.is_some() {
            self.menu = None;
            cx.notify();
        }
        if e.button == MouseButton::Left && e.click_count >= 2 && matches!(self.tool, Tool::Select) {
            self.double_click(window, cx);
            return;
        }
        self.update_hover(cx);
        self.drag = Drag::Pending { button: e.button, start: self.cursor, start_px: lp, pick: self.hover };
        cx.stop_propagation();
    }

    pub fn on_mouse_move(&mut self, e: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
        if let Drag::Pending { button, start, start_px, pick } = self.drag.clone()
            && len(sub(lp, start_px)) > 3.0
        {
            self.drag = match button {
                MouseButton::Right | MouseButton::Middle => Drag::Pan { last_px: start_px },
                MouseButton::Left if matches!(self.tool, Tool::Select) => self.start_drag(pick, start),
                _ => Drag::None,
            };
        }
        match self.drag.clone() {
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
                }
                self.set_preview(Some(dr), cx);
            }
            Drag::MoveAnnotation { start, orig } => {
                let a = self.dragged_annotation(&orig, start);
                let mut dr = self.drawing.clone();
                if let Some(s) = dr.sheets.get_mut(self.sheet)
                    && let Some(x) = s.annotations.iter_mut().find(|x| x.id == a.id)
                {
                    *x = a;
                }
                self.set_preview(Some(dr), cx);
            }
            _ => {
                self.update_hover(cx);
                if !matches!(self.tool, Tool::Select) {
                    self.update_preview(cx);
                }
            }
        }
    }

    pub fn on_mouse_up(&mut self, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        self.cursor = self.to_sheet(lp);
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
            Drag::MoveAnnotation { .. } => {
                if let Some(p) = self.preview.take() {
                    self.commit("Move annotation", p, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    pub fn on_scroll(&mut self, e: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let lp = self.local(e.position);
        let dy = match e.delta {
            ScrollDelta::Lines(l) => l.y as f64 * 40.0,
            ScrollDelta::Pixels(px) => f64::from(f32::from(px.y)),
        };
        if dy.abs() < 0.01 {
            return;
        }
        self.zoom_by((dy * 0.0025).exp(), lp, cx);
    }

    pub fn on_key(&mut self, e: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        if k.modifiers.control || k.modifiers.platform || k.modifiers.alt || self.panel.is_some() {
            return;
        }
        let tool = match (k.key.as_str(), k.modifiers.shift) {
            ("d", false) => Tool::Dimension { kind: DimTool::Linear, picks: vec![] },
            ("d", true) => Tool::Dimension { kind: DimTool::Diameter, picks: vec![] },
            ("r", true) => Tool::Dimension { kind: DimTool::Radius, picks: vec![] },
            _ => return,
        };
        self.set_tool(tool, cx);
        cx.stop_propagation();
    }

    fn select_click(&mut self, pick: Option<u32>, m: Modifiers, cx: &mut Context<Self>) {
        let sel = self.sel_at(pick, self.cursor);
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

    fn start_drag(&mut self, pick: Option<u32>, start: [f64; 2]) -> Drag {
        let Some(sel) = self.sel_at(pick, start) else { return Drag::None };
        if !self.selection.contains(&sel) {
            self.selection = vec![sel];
        }
        let sheet = &self.drawing.sheets[self.sheet];
        match sel {
            Sel::View(id) => {
                // Move the view with all views derived from it; a projected
                // (or section/auxiliary) view slides along its alignment axis.
                let ids = sheet.descendants(id);
                let orig = ids.iter().filter_map(|v| sheet.view(*v).map(|x| (*v, x.position))).collect();
                let constraint = match &sheet.view(id).map(|v| v.kind.clone()) {
                    Some(ViewKind::Projected { .. }) => views::projected_direction(sheet, id).filter(|d| d[0] == 0.0 || d[1] == 0.0).map(|d| (d, d)),
                    Some(ViewKind::Section { .. }) => views::section_cut(sheet, id).map(|c| (c.toward_view, c.toward_view)),
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
            AnnotationKind::SurfaceFinish { anchor, .. } | AnnotationKind::Gdt { anchor, .. } => {
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
            Some(PickTarget::Annotation(id)) => {
                let note = self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).cloned();
                if let Some(Annotation { kind: AnnotationKind::Note { .. }, position, .. }) = note {
                    self.open_note_panel(Some(id), position, window, cx);
                }
            }
            Some(t) => {
                if let Some(v) = t.view() {
                    self.open_scale_panel(Some(v), window, cx);
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
        let entries = self.context_entries(sel);
        let ed = cx.entity();
        let menu = PopupMenu::build(window, cx, move |m, window, cx| fill_menu(m, &entries, &ed, window, cx));
        let sub = cx.subscribe_in(&menu, window, |this: &mut Self, _, _: &DismissEvent, _, cx| {
            this.menu = None;
            cx.notify();
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.menu = Some((menu, position, sub));
        cx.notify();
    }

    fn context_entries(&self, sel: Option<Sel>) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        match sel {
            Some(Sel::View(id)) => {
                let Some(v) = self.drawing.sheets[self.sheet].view(id).cloned() else { return out };
                out.push(MenuEntry::item("Projected view", move |t, _, cx| t.set_tool(Tool::Projected { parent: Some(id) }, cx)));
                out.push(MenuEntry::item("Section view", move |t, _, cx| t.set_tool(Tool::Section { parent: Some(id), p0: None, line: None }, cx)));
                out.push(MenuEntry::item("Detail view", move |t, _, cx| t.set_tool(Tool::Detail { parent: Some(id), center: None, radius: None }, cx)));
                out.push(MenuEntry::item("Auxiliary view", move |t, _, cx| t.set_tool(Tool::Auxiliary { parent: None }, cx)));
                out.push(MenuEntry::Separator);
                let hidden = v.style == RenderStyle::HiddenLinesVisible;
                out.push(MenuEntry::check("Show hidden lines", hidden, move |t, _, cx| {
                    t.edit_view("Hidden lines", id, cx, |v| v.style = if hidden { RenderStyle::HiddenLinesRemoved } else { RenderStyle::HiddenLinesVisible })
                }));
                let tangent = v.show_tangent_edges;
                out.push(MenuEntry::check("Show tangent edges", tangent, move |t, _, cx| t.edit_view("Tangent edges", id, cx, |v| v.show_tangent_edges = !tangent)));
                let cl = v.show_centerlines;
                out.push(MenuEntry::check("Automatic centermarks", cl, move |t, _, cx| t.edit_view("Centermarks", id, cx, |v| v.show_centerlines = !cl)));
                let mut scales = vec![MenuEntry::check("Parent / sheet scale", v.scale.is_none(), move |t, _, cx| t.edit_view("View scale", id, cx, |v| v.scale = None))];
                for s in [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0] {
                    scales.push(MenuEntry::check(noshape_model::drawing::sheet::format_scale(s), v.scale == Some(s), move |t, _, cx| t.edit_view("View scale", id, cx, |v| v.scale = Some(s))));
                }
                scales.push(MenuEntry::item("Custom…", move |t, w, cx| t.open_scale_panel(Some(id), w, cx)));
                out.push(MenuEntry::Submenu("Scale".into(), scales));
                out.push(MenuEntry::item("View label…", move |t, w, cx| t.open_label_panel(id, w, cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Delete view", move |t, _, cx| {
                    t.selection.clear();
                    t.edit_sheet("Delete view", cx, |s| s.remove_view(id));
                }));
            }
            Some(Sel::Annotation(id)) => {
                let a = self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id).cloned();
                if let Some(a) = &a {
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
                    if let AnnotationKind::RadialDimension { diameter, .. } = a.kind {
                        out.push(MenuEntry::item(if diameter { "Show as radius" } else { "Show as diameter" }, move |t, _, cx| {
                            t.edit_sheet("Radial dimension", cx, |s| {
                                if let Some(AnnotationKind::RadialDimension { diameter, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                                    *diameter = !*diameter;
                                }
                            })
                        }));
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
            None => {
                out.push(MenuEntry::item("Update drawing", |t, _, cx| t.update_drawing(cx)));
                out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
                out.push(MenuEntry::Separator);
                out.push(MenuEntry::item("Insert note", |t, _, cx| t.set_tool(Tool::Note, cx)));
                out.push(MenuEntry::item("Edit title block…", |t, w, cx| t.open_title_panel(w, cx)));
                out.push(MenuEntry::Submenu("Sheet size".into(), self.sheet_size_entries()));
                out.push(MenuEntry::item("Sheet scale…", |t, w, cx| t.open_scale_panel(None, w, cx)));
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
        [SheetSize::A4, SheetSize::A3, SheetSize::A2, SheetSize::A1, SheetSize::A0, SheetSize::AnsiA, SheetSize::AnsiB, SheetSize::AnsiC, SheetSize::AnsiD, SheetSize::AnsiE]
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
