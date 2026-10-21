//! Drawing picking (slice 2): what lies under a sheet point, given the
//! built sheet and the view geometry. A borrowed view of the editor's
//! state, so the app's `DrawingEditor` and the headless drawing tools share
//! one implementation (moved from `crates/app/src/drawing_view.rs`,
//! `drawing_tools.rs`, `drawing_more.rs`, `drawing_smart.rs` and
//! `drawing_images.rs`).

use super::geom::{len, sub};
use super::{DimTool, Pick, PickedPoint, Sel, TableTool, Tool};
use noshape_model::drawing::build::{BuiltSheet, PickTarget, ViewPlacement};
use noshape_model::drawing::views::{self, EdgeGeom, SnapKind, Sources, ViewGeometry};
use noshape_model::drawing::{AnnotationKind, Drawing, PointRef, Sheet, ViewId};
use std::collections::HashMap;
use std::sync::Arc;

/// The editor state picking reads.
#[derive(Clone, Copy)]
pub struct SheetPick<'a> {
    /// The drawing as committed.
    pub drawing: &'a Drawing,
    /// The drawing shown (a preview while one is open, else the drawing).
    pub display: &'a Drawing,
    pub sheet: usize,
    pub built: Option<&'a BuiltSheet>,
    pub geoms: &'a HashMap<ViewId, Arc<ViewGeometry>>,
    pub tool: &'a Tool,
    /// The hovered pick id.
    pub hover: Option<u32>,
    /// The pointer, in sheet millimeters.
    pub cursor: [f64; 2],
    /// Pixels per sheet millimeter.
    pub zoom: f64,
    /// Shift+Q: midpoints and quadrant points snap.
    pub snap_mid_quad: bool,
    pub selection: &'a [Sel],
    /// The views' model sources.
    pub sources: &'a Sources,
}

/// The smallest view rectangle containing `s` (with a margin in mm).
fn smallest_view(b: &BuiltSheet, s: [f64; 2], margin: f64) -> Option<&ViewPlacement> {
    b.placements
        .iter()
        .filter(|p| p.contains(s, margin))
        .min_by(|a, c| ((a.max[0] - a.min[0]) * (a.max[1] - a.min[1])).total_cmp(&((c.max[0] - c.min[0]) * (c.max[1] - c.min[1]))))
}

impl<'a> SheetPick<'a> {
    pub fn current_sheet(&self) -> &'a Sheet {
        let d = self.display;
        &d.sheets[self.sheet.min(d.sheets.len().saturating_sub(1))]
    }

    /// Sheet millimeters of `px` screen pixels.
    pub fn tol_mm(&self, px: f64) -> f64 {
        px / self.zoom
    }

    /// The pick the active tool highlights under the cursor.
    pub fn hover_at(&self) -> Option<u32> {
        let tol = self.tol_mm(5.0);
        let hover = self.built.and_then(|b| b.hit(self.cursor, tol));
        let hover = match (self.tool, hover) {
            // 2 point linear takes points only: no edge highlight.
            (Tool::Dimension { kind: DimTool::Point2, .. }, _) => None,
            // The smart dimension also dimensions drawing sketch geometry (SK7).
            (Tool::Dimension { kind: DimTool::Linear, .. }, Some(h)) => match self.built.and_then(|b| b.picks.get(h as usize)) {
                Some(PickTarget::Edge { .. } | PickTarget::SketchEntity(_)) => Some(h),
                _ => None,
            },
            // Dimension-like tools only care about view edges.
            (Tool::Dimension { .. } | Tool::Auxiliary { .. } | Tool::Centermark | Tool::Centerline { .. } | Tool::EdgeCenterline { .. } | Tool::VirtualSharp { .. } | Tool::Balloon { .. } | Tool::Ordinate { .. }, Some(h)) => match self.built.and_then(|b| b.picks.get(h as usize)) {
                Some(PickTarget::Edge { .. }) => Some(h),
                _ => None,
            },
            // Sketch tools and constraints: sketch geometry and view edges.
            (Tool::Sketch { .. } | Tool::Constrain { .. }, Some(h)) => match self.built.and_then(|b| b.picks.get(h as usize)) {
                Some(PickTarget::Edge { .. } | PickTarget::SketchEntity(_)) => Some(h),
                _ => None,
            },
            (
                Tool::Select
                | Tool::Projected { parent: None }
                | Tool::Section { parent: None, .. }
                | Tool::BrokenOut { parent: None, .. }
                | Tool::CropPoly { parent: None, .. }
                | Tool::Detail { parent: None, .. }
                | Tool::JoggedSection { parent: None, .. }
                | Tool::AlignedSection { parent: None, .. }
                | Tool::Crop { parent: None, .. }
                | Tool::Break { parent: None, .. }
                | Tool::Table { kind: TableTool::Bom | TableTool::Hole | TableTool::CutList, view: None, .. },
                h,
            ) => h,
            _ => None,
        };
        // In select mode, hovering anywhere inside a view highlights it.
        match (hover, self.tool, self.built) {
            (None, Tool::Select, Some(b)) => smallest_view(b, self.cursor, 1.0).and_then(|p| b.picks.iter().position(|t| *t == PickTarget::View(p.id))).map(|i| i as u32),
            (h, _, _) => h,
        }
    }

    pub fn placement(&self, view: ViewId) -> Option<ViewPlacement> {
        self.built?.placement(view).copied()
    }

    /// The view under a sheet point (smallest containing view rectangle).
    pub fn view_at(&self, s: [f64; 2]) -> Option<ViewId> {
        if let Some(h) = self.hover
            && let Some(Sel::View(v)) = self.sel_of_pick(h)
        {
            return Some(v);
        }
        smallest_view(self.built?, s, 2.0).map(|p| p.id)
    }

    /// Pick ids belonging to a selection entry.
    pub fn picks_of(&self, sel: Sel) -> Vec<u32> {
        let Some(b) = self.built else { return vec![] };
        b.picks
            .iter()
            .enumerate()
            .filter(|(_, t)| match (sel, t) {
                (Sel::View(v), t) => t.view() == Some(v),
                (Sel::Annotation(a), PickTarget::Annotation(b) | PickTarget::Inspection(b)) => a == *b,
                (Sel::Table(a), PickTarget::Table(b)) => a == *b,
                (Sel::Break(v, i), PickTarget::BreakLine { view, index }) => v == *view && i == *index,
                (Sel::Sketch(a), PickTarget::SketchEntity(b)) => a == *b,
                (Sel::Hatch(a), PickTarget::Hatch(b)) => a == *b,
                (Sel::Image(a), PickTarget::Image(b)) => a == *b,
                (Sel::AutoCentermark(v, c), PickTarget::AutoCentermark { view, center }) => v == *view && c == *center,
                (Sel::ViewLabel(v), PickTarget::ViewLabel(w)) | (Sel::AuxIndicator(v), PickTarget::AuxIndicator(w)) => v == *w,
                _ => false,
            })
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// The selectable entity a pick id belongs to.
    pub fn sel_of_pick(&self, pick: u32) -> Option<Sel> {
        match self.built?.picks.get(pick as usize)? {
            PickTarget::View(v) | PickTarget::Edge { view: v, .. } | PickTarget::SectionLine(v) | PickTarget::DetailCircle(v) => Some(Sel::View(*v)),
            PickTarget::Annotation(a) | PickTarget::Inspection(a) => Some(Sel::Annotation(*a)),
            PickTarget::Table(t) => Some(Sel::Table(*t)),
            PickTarget::BreakLine { view, index } => Some(Sel::Break(*view, *index)),
            PickTarget::SketchEntity(id) => Some(Sel::Sketch(*id)),
            PickTarget::Hatch(id) => Some(Sel::Hatch(*id)),
            PickTarget::Image(id) => Some(Sel::Image(*id)),
            PickTarget::AuxIndicator(v) => Some(Sel::AuxIndicator(*v)),
            PickTarget::ViewLabel(v) => Some(Sel::ViewLabel(*v)),
            PickTarget::AutoCentermark { view, center } => Some(Sel::AutoCentermark(*view, *center)),
            _ => None,
        }
    }

    /// The image under a sheet point (topmost).
    pub fn image_at(&self, c: [f64; 2]) -> Option<u64> {
        self.current_sheet().images.iter().rev().find(|i| c[0] >= i.pos[0] && c[1] >= i.pos[1] && c[0] <= i.pos[0] + i.size[0] && c[1] <= i.pos[1] + i.size[1]).map(|i| i.id)
    }

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
        smallest_view(self.built?, s, 1.0).map(|p| Sel::View(p.id))
    }

    /// Snap point (vertex / circle center) of a view near a sheet point.
    pub fn snap_at(&self, s: [f64; 2]) -> Option<PickedPoint> {
        let built = self.built?;
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
                        EdgeGeom::Line { a, b } => cand.push(([(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5], PointRef::Midpoint { body: me.body.clone(), name: (*me.name).clone() })),
                        EdgeGeom::Circle { center, radius, face_on: true, mid, .. } => {
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
    /// The part of the view edge nearest to `p` (view coordinates) within
    /// the pick tolerance.
    pub fn part_near(&self, view: ViewId, p: [f64; 2]) -> Option<noshape_model::BodyId> {
        let pl = self.placement(view)?;
        let g = self.geoms.get(&view)?;
        let s = pl.to_sheet(p);
        let tol = self.tol_mm(6.0);
        g.edges
            .iter()
            .filter_map(|e| {
                let me = g.model_edges.get(e.model? as usize)?;
                let d = e.points.windows(2).map(|w| views::dist_to_segment(s, pl.to_sheet(w[0]), pl.to_sheet(w[1]))).fold(f64::INFINITY, f64::min);
                (d < tol).then(|| (d, me.body.clone()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, b)| b)
    }

    /// The part under `p` (view coordinates): the front-most body on the
    /// line of sight (faces and silhouettes).
    pub fn part_under(&self, view: ViewId, p: [f64; 2]) -> Option<noshape_model::BodyId> {
        let frame = self.geoms.get(&view)?.frame?;
        let (key, _) = views::view_source(&self.drawing.sheets[self.sheet], view)?;
        views::body_under(self.sources.get(&key)?, &frame, p)
    }

    pub fn hovered_edge(&self) -> Option<(ViewId, views::ModelEdge)> {
        let h = self.hover?;
        let PickTarget::Edge { view, edge } = self.built?.picks.get(h as usize)?.clone() else { return None };
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

    pub fn pick_sheet(&self, p: &PickedPoint) -> Option<[f64; 2]> {
        Some(self.placement(p.view)?.to_sheet(p.p))
    }

    /// The axis under the cursor: a centerline annotation, an automatic
    /// centerline of a view, else a straight edge.
    pub fn axis_pick(&self) -> Option<Pick> {
        let c = self.cursor;
        let built = self.built?;
        let tol = self.tol_mm(6.0);
        for a in &built.resolved.annotations {
            if let AnnotationKind::Centerline { view, a: pa, b: pb } = &a.kind
                && let Some(pl) = self.placement(*view)
                && views::dist_to_segment(c, pl.to_sheet(*pa), pl.to_sheet(*pb)) < tol
            {
                let r = |slot: u8| a.refs.iter().find(|x| x.slot == slot).map(|x| x.point.clone());
                return Some(Pick::Line { a: PickedPoint { view: *view, p: *pa, r: r(0) }, b: PickedPoint { view: *view, p: *pb, r: r(1) } });
            }
        }
        for pl in &built.placements {
            let Some(g) = self.geoms.get(&pl.id) else { continue };
            for ax in &g.axes {
                if views::dist_to_segment(c, pl.to_sheet(ax[0]), pl.to_sheet(ax[1])) < tol {
                    return Some(Pick::Line { a: PickedPoint { view: pl.id, p: ax[0], r: None }, b: PickedPoint { view: pl.id, p: ax[1], r: None } });
                }
            }
        }
        self.line_pick()
    }

    /// The hovered drawn line of a view (also silhouettes and cut edges
    /// without a model edge), as its end points.
    pub fn raw_line_pick(&self) -> Option<Pick> {
        let h = self.hover?;
        let PickTarget::Edge { view, edge } = self.built?.picks.get(h as usize)?.clone() else { return None };
        let pts = &self.geoms.get(&view)?.edges.get(edge as usize)?.points;
        let (a, b) = (*pts.first()?, *pts.last()?);
        let l = len(sub(b, a));
        if l < 1e-9 || pts.iter().any(|p| ((p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])).abs() / l > l * 1e-3) {
            return None;
        }
        Some(Pick::Line { a: PickedPoint { view, p: a, r: None }, b: PickedPoint { view, p: b, r: None } })
    }

    /// The circle under the cursor with the clicked point on it.
    pub fn circle_at(&self) -> Option<Pick> {
        let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(self.cursor) else { return None };
        let at = self.placement(view)?.to_view(self.cursor);
        Some(Pick::CircleAt { view, center, radius, r, at })
    }

    /// What the smart tool picks under the cursor: a vertex or center
    /// snap, else the hovered edge (line, arc or circle).
    pub fn smart_pick(&self, c: [f64; 2]) -> Option<Pick> {
        if let Some(p) = self.snap_at(c) {
            return Some(Pick::Point(p));
        }
        self.line_pick().or_else(|| self.arc_pick()).or_else(|| self.hovered_edge().and_then(|_| self.circle_pick(c)))
    }
}
