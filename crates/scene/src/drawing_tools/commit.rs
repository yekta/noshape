//! Drawing slice 5: what the drawing tools commit, as plain functions of
//! the picks. The app's tool clicks (`drawing_tools.rs`, `drawing_extra.rs`)
//! and the agent API build the same views, annotations, breaks, crops and
//! tables with them.

use super::geom::*;
use super::{EdgeRef, Pick, PickedPoint, SheetPick, TableTool};
use noshape_model::drawing::build::ViewPlacement;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

/// The Section view tool's second click: snapped to horizontal / vertical
/// from `a` when within 8 degrees of it.
pub fn ortho_point(a: [f64; 2], c: [f64; 2]) -> [f64; 2] {
    let d = sub(c, a);
    let ang = d[1].atan2(d[0]).to_degrees().rem_euclid(180.0);
    if !(8.0..=172.0).contains(&ang) {
        [c[0], a[1]]
    } else if (ang - 90.0).abs() < 8.0 {
        [a[0], c[1]]
    } else {
        c
    }
}

impl SheetPick<'_> {
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

    /// A snap point of view `pid` under `c` off the current cutting line: a
    /// jog for the Section view tool (help drawing_section_view step 7).
    pub fn section_jog_at(&self, pid: ViewId, c: [f64; 2]) -> Option<[f64; 2]> {
        let super::Tool::Section { line: Some((a, b)), jogs, .. } = self.tool else { return None };
        let q = self.snap_at(c).filter(|s| s.view == pid)?.p;
        let line = super::jogged_line(*a, *b, jogs);
        let off = line.windows(2).all(|w| views::dist_to_segment(q, w[0], w[1]) > 1e-6);
        off.then_some(q)
    }

    /// Edge to edge centerline on one edge: when it is a cylinder / cone
    /// silhouette, the view's axis line parallel to it, spanning the edge.
    pub fn silhouette_centerline(&self, a: &PickedPoint, b: &PickedPoint) -> Option<([f64; 2], [f64; 2])> {
        let dir = norm(sub(b.p, a.p));
        let n = [-dir[1], dir[0]];
        let off = |ax: &[[f64; 2]; 2]| dot(sub(ax[0], a.p), n).abs();
        let axis = self.geoms.get(&a.view).and_then(|g| g.axes.iter().filter(|ax| (dot(norm(sub(ax[1], ax[0])), dir)).abs() > 1.0 - 1e-6).min_by(|x, y| off(x).total_cmp(&off(y))).copied());
        let edge_len = len(sub(b.p, a.p));
        let ax = axis.filter(|ax| off(ax) < edge_len.max(1e-6) * 2.0)?;
        let o = ax[0];
        let t0 = dot(sub(a.p, o), dir);
        let t1 = dot(sub(b.p, o), dir);
        Some(([o[0] + dir[0] * t0, o[1] + dir[1] * t0], [o[0] + dir[0] * t1, o[1] + dir[1] * t1]))
    }
}

impl SheetPick<'_> {
    /// Auxiliary view (Normal): the straight model edge under the cursor:
    /// its view, its sheet direction and its midpoint (view coordinates).
    pub fn aux_edge(&self) -> Option<(ViewId, [f64; 2], [f64; 2])> {
        let Some(Pick::Line { a, b }) = self.line_pick() else { return None };
        let pl = self.placement(a.view)?;
        Some((a.view, norm(sub(pl.to_sheet(b.p), pl.to_sheet(a.p))), [(a.p[0] + b.p[0]) / 2.0, (a.p[1] + b.p[1]) / 2.0]))
    }

    /// Why a view can't be the parent of a new view of `tool` (the status
    /// text the tool shows), if it can't.
    pub fn view_refusal(&self, tool: &str, view: ViewId) -> Option<&'static str> {
        let sheet = self.drawing.sheets.get(self.sheet)?;
        let v = sheet.view(view)?;
        let detail = matches!(v.kind, ViewKind::Detail { .. });
        match tool {
            // Help H:projected_view.
            "projected" if detail => Some("Projected views can't be created from detail views"),
            "auxiliary" if detail || !v.breaks.is_empty() => Some("Auxiliary views can't be created from detail or break views"),
            "angular_section" if !v.breaks.is_empty() => Some("Angular section views can't be created from views with breaks"),
            "break" if detail => Some("Break views can't be created from detail views"),
            // Help drawing_section_view: no breaks on views with angular cut lines.
            "break" if sheet.has_angular_cut(view) => Some("Break views can't be created on views with angular section lines"),
            "crop" if detail || !v.breaks.is_empty() || v.flat => Some("Crop views can't be created from detail, break or flat pattern views"),
            _ => None,
        }
    }
}

/// Edge to edge centerline on two edges: their middle line, the ends paired
/// along the first edge.
pub fn edge_pair_centerline(a1: [f64; 2], b1: [f64; 2], a: [f64; 2], b: [f64; 2]) -> ([f64; 2], [f64; 2]) {
    let dir = norm(sub(b1, a1));
    let (a2, b2) = if dot(sub(b, a), dir) >= 0.0 { (a, b) } else { (b, a) };
    let mid = |p: [f64; 2], q: [f64; 2]| [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5];
    (mid(a1, a2), mid(b1, b2))
}

fn slot_refs<'a>(v: impl IntoIterator<Item = (u8, &'a Option<PointRef>)>) -> Vec<AnnotationRef> {
    v.into_iter().filter_map(|(slot, r)| r.clone().map(|point| AnnotationRef { slot, point })).collect()
}

/// 2 point centerline from `a` to `b` (one view), placed at `position`.
pub fn centerline(id: u64, a: &PickedPoint, b: &PickedPoint, position: [f64; 2]) -> Annotation {
    let refs = slot_refs([(0u8, &a.r), (1u8, &b.r)]);
    Annotation { id, kind: AnnotationKind::Centerline { view: a.view, a: a.p, b: b.p }, position, refs, ..Default::default() }
}

/// Edge to edge centerline `p`-`q` in `view` (no point references).
pub fn edge_centerline(id: u64, view: ViewId, p: [f64; 2], q: [f64; 2], position: [f64; 2]) -> Annotation {
    Annotation { id, kind: AnnotationKind::Centerline { view, a: p, b: q }, position, ..Default::default() }
}

/// 3 point / 2 point circle centerline through `points` (one view); None
/// when they make no circle.
pub fn circle_centerline(id: u64, points: &[PickedPoint], position: [f64; 2]) -> Option<Annotation> {
    let view = points.first()?.view;
    let pts: Vec<[f64; 2]> = points.iter().map(|p| p.p).collect();
    AnnotationKind::circle_centerline(&pts)?;
    let refs = slot_refs(points.iter().enumerate().map(|(k, p)| (k as u8, &p.r)));
    Some(Annotation { id, kind: AnnotationKind::CircleCenterline { view, points: pts }, position, refs, ..Default::default() })
}

/// Virtual sharp of two straight edges (`a1`-`b1` with its model edge, then
/// `a`-`b`) in `view`; the status text when they are parallel.
pub fn virtual_sharp(id: u64, view: ViewId, first: (&PickedPoint, &PickedPoint, &Option<EdgeRef>), second: (&PickedPoint, &PickedPoint, &Option<EdgeRef>), position: [f64; 2]) -> Result<Annotation, &'static str> {
    let ((a1, b1, e1), (a, b, edge)) = (first, second);
    if AnnotationKind::line_intersection(a1.p, b1.p, a.p, b.p).is_none() {
        return Err("The edges are parallel");
    }
    let mut refs = slot_refs([(0u8, &a1.r), (1, &b1.r), (2, &a.r), (3, &b.r)]);
    // Slot 4: the intersection itself, for associative dimensions to it.
    if let (Some((body, name)), Some((body2, name2))) = (e1.clone(), edge.clone()) {
        refs.push(AnnotationRef { slot: 4, point: PointRef::Intersection { body, name, body2, name2 } });
    }
    Ok(Annotation { id, kind: AnnotationKind::VirtualSharp { view, a1: a1.p, b1: b1.p, a2: a.p, b2: b.p }, position, refs, ..Default::default() })
}

/// Break view: the band between sheet points `f` and `c` of the view at
/// `pl` (across x when `vertical`); None when it is empty.
pub fn view_break(pl: &ViewPlacement, vertical: bool, f: [f64; 2], c: [f64; 2], style: BreakStyle, gap_mm: f64) -> Option<ViewBreak> {
    let (a, b) = (pl.to_view(f), pl.to_view(c));
    let k = if vertical { 0 } else { 1 };
    let (lo, hi) = (a[k].min(b[k]), a[k].max(b[k]));
    (hi - lo > 1e-6).then_some(ViewBreak { vertical, a: lo, b: hi, gap_mm, style })
}

/// Crop view: a circle (center `f`, through `c`) or the rectangle with
/// corners `f` and `c` (sheet points) of the view at `pl`.
pub fn view_crop(pl: &ViewPlacement, circle: bool, f: [f64; 2], c: [f64; 2]) -> Option<Crop> {
    if circle {
        let r = len(sub(c, f)) / (pl.scale * 1000.0);
        (r > 1e-7).then(|| Crop::Circle { center: pl.to_view(f), radius: r })
    } else {
        let corners = [f, [c[0], f[1]], c, [f[0], c[1]]];
        (len(sub(c, f)) > 1.0).then(|| Crop::Polygon(corners.iter().map(|k| pl.to_view(*k)).collect()))
    }
}

/// The Break view dialog's default gap: 6 mm, or 0.25 in in inch drawings.
pub fn default_break_gap_mm(units_mm: bool) -> f64 {
    if units_mm { 6.0 } else { 0.25 * 25.4 }
}

/// A BOM, cut list or hole table of `view` placed at `position`, and the
/// edit's description.
pub fn view_table(id: u64, kind: TableTool, view: ViewId, origin: Option<PickedPoint>, position: [f64; 2]) -> (Table, &'static str) {
    let tk = match kind {
        TableTool::Bom => TableKind::Bom { view, structured: false, bottom_to_top: false, overrides: Vec::new() },
        TableTool::CutList => TableKind::CutList { view, bottom_to_top: false },
        _ => TableKind::Hole { view, origin: origin.as_ref().map(|o| o.p).unwrap_or([0.0; 2]), origin_ref: origin.and_then(|o| o.r), precision: None, flip_x: false, flip_y: false },
    };
    let name = match kind {
        TableTool::Bom => "BOM table",
        TableTool::CutList => "Cut list table",
        _ => "Hole table",
    };
    (Table::new(id, tk, position), name)
}
