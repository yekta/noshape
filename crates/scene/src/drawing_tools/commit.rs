//! Drawing slice 5: what the drawing tools commit, as plain functions of
//! the picks. The app's tool clicks (`drawing_tools.rs`, `drawing_extra.rs`)
//! and the agent API build the same views, annotations, breaks, crops and
//! tables with them.

use super::geom::*;
use super::{DimTool, EdgeRef, Pick, PickedPoint, Profile, SheetPick, TableTool};
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

/// A callout placed in empty space: no leader, its text at `c`.
pub fn free_callout(id: u64, c: [f64; 2], text: String, style: CalloutStyle) -> Annotation {
    let mut kind = AnnotationKind::balloon(ViewId(0), c, text);
    if let AnnotationKind::Balloon { callout, .. } = &mut kind {
        *callout = CalloutStyle { free: true, ..style };
    }
    Annotation { id, kind, position: c, ..Default::default() }
}

impl SheetPick<'_> {
    /// A callout with a leader clicked on a face or a silhouette (no edge,
    /// no snap point): the part on the line of sight (qa5 D8).
    pub fn finish_callout(&self, a: &mut Annotation, anchor: &PickedPoint) {
        if let AnnotationKind::Balloon { callout, .. } = &mut a.kind
            && callout.part.is_none()
            && anchor.r.is_none()
        {
            callout.part = self.part_under(anchor.view, anchor.p);
            callout.on_face |= super::pending::leader_dot(false, callout.part.is_some() && views::view_source(self.current_sheet(), anchor.view).and_then(|(k, _)| self.sources.get(&k)).is_some_and(|s| s.assembly));
        }
    }
}

impl SheetPick<'_> {
    /// The Max/min and Foreshorten dimension tools' click at the cursor:
    /// what it adds to `picks`.
    pub fn more_dimension_pick(&self, kind: DimTool, picks: &mut Vec<Pick>) {
        match (kind, picks.len()) {
            (DimTool::MaxMin, 0) => picks.extend(self.circle_at()),
            (DimTool::MaxMin, 1) => {
                let Some(Pick::CircleAt { view, center, .. }) = picks.first().cloned() else { return };
                let second = match self.circle_at() {
                    Some(Pick::CircleAt { view: v, center: c2, .. }) if v == view && len(sub(c2, center)) < 1e-9 => None,
                    Some(p @ Pick::CircleAt { view: v, .. }) if v == view => Some(p),
                    _ => self.snap_at(self.cursor).filter(|p| p.view == view).map(Pick::Point),
                };
                picks.extend(second);
            }
            (DimTool::Foreshorten, 0) => picks.extend(self.axis_pick()),
            (DimTool::Foreshorten, 1) => {
                let view = match &picks[0] {
                    Pick::Line { a, .. } => a.view,
                    _ => return,
                };
                // A snap point, a model edge, any drawn line (silhouettes have no
                // model edge), else the clicked point itself.
                let e = self
                    .snap_at(self.cursor)
                    .filter(|p| p.view == view)
                    .map(Pick::Point)
                    .or_else(|| self.line_pick().filter(|l| matches!(l, Pick::Line { a, .. } if a.view == view)))
                    .or_else(|| self.raw_line_pick().filter(|l| matches!(l, Pick::Line { a, .. } if a.view == view)))
                    .or_else(|| self.placement(view).filter(|pl| pl.contains(self.cursor, 2.0)).map(|pl| Pick::Point(PickedPoint { view, p: pl.to_view(self.cursor), r: None })));
                picks.extend(e);
            }
            _ => {}
        }
    }

    /// A chamfer dimension in Drawing properties' "Dimensions" style: its
    /// two legs as linear dimensions (None: kept as a chamfer callout).
    pub fn chamfer_as_dimensions(&self, a: &Annotation) -> Option<Vec<Annotation>> {
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

/// A circle picked by the Centermark tool.
#[derive(Clone, Debug)]
pub struct CmCircle {
    pub view: ViewId,
    pub center: [f64; 2],
    pub radius: f64,
    pub r: Option<PointRef>,
}

/// The Centermark dialog: `mode` 0 Single, 1 Circular, 2 Linear, and its
/// options (the defaults are the dialog's when it opens).
#[derive(Clone, Copy, Debug)]
pub struct CentermarkOpts {
    pub mode: u8,
    pub reference_center: bool,
    pub bolt_circle: bool,
    pub radial_lines: bool,
    pub pattern: bool,
    pub connecting: bool,
}

impl Default for CentermarkOpts {
    fn default() -> Self {
        Self { mode: 0, reference_center: true, bolt_circle: true, radial_lines: false, pattern: true, connecting: true }
    }
}

/// Centermark (Single): the mark of one circle, placed at `position`.
pub fn centermark(id: u64, c: &CmCircle, position: [f64; 2]) -> Annotation {
    let refs = c.r.clone().map(|point| vec![AnnotationRef { slot: 0, point }]).unwrap_or_default();
    Annotation { id, kind: AnnotationKind::Centermark { view: c.view, center: c.center, size: c.radius * 1.25, angle: 0.0, arms: [0.0; 4] }, position, refs, ..Default::default() }
}

/// Circular / Linear centermark ✓: the marks (and their centerlines, bolt
/// circle, reference center) of `picks` (one view), and the edit's name.
pub fn centermark_set(picks: &[CmCircle], o: &CentermarkOpts) -> (Vec<(AnnotationKind, Vec<AnnotationRef>)>, &'static str) {
    let mut new: Vec<(AnnotationKind, Vec<AnnotationRef>)> = Vec::new();
    let name = if o.mode == 1 { "Circular centermark" } else { "Linear centermark" };
    let Some(first) = picks.first() else { return (new, name) };
    let view = first.view;
    let centers: Vec<[f64; 2]> = picks.iter().map(|p| p.center).collect();
    let mark = |c: &CmCircle, angle: f64| (AnnotationKind::Centermark { view, center: c.center, size: c.radius * 1.25, angle, arms: [0.0; 4] }, c.r.clone().map(|point| vec![AnnotationRef { slot: 0, point }]).unwrap_or_default());
    if o.mode == 1 {
        // Pattern center: the circle through the first three centers (or the mean).
        let pc = match centers.len() {
            1 | 2 => {
                let n = centers.len() as f64;
                centers.iter().fold([0.0, 0.0], |a, c| [a[0] + c[0] / n, a[1] + c[1] / n])
            }
            _ => AnnotationKind::circle_centerline(&centers[..3]).map(|(c, _)| c).unwrap_or(centers[0]),
        };
        for c in picks {
            let angle = if o.pattern { (c.center[1] - pc[1]).atan2(c.center[0] - pc[0]) } else { 0.0 };
            new.push(mark(c, angle));
            if o.radial_lines {
                new.push((AnnotationKind::Centerline { view, a: pc, b: c.center }, vec![]));
            }
        }
        if o.bolt_circle && centers.len() >= 2 {
            // Through three hole centers (associative), else center + a hole.
            let (pts, refs): (Vec<[f64; 2]>, Vec<AnnotationRef>) = if centers.len() >= 3 {
                (centers[..3].to_vec(), picks[..3].iter().enumerate().filter_map(|(k, p)| p.r.clone().map(|point| AnnotationRef { slot: k as u8, point })).collect())
            } else {
                (vec![pc, centers[0]], vec![])
            };
            new.push((AnnotationKind::CircleCenterline { view, points: pts }, refs));
        }
        if o.reference_center {
            let s = picks.iter().map(|p| p.radius).fold(f64::MAX, f64::min) * 0.6;
            new.push((AnnotationKind::Centermark { view, center: pc, size: s, angle: 0.0, arms: [0.0; 4] }, vec![]));
        }
    } else {
        for c in picks {
            new.push(mark(c, 0.0));
        }
        if o.connecting {
            for w in picks.windows(2) {
                new.push((AnnotationKind::Centerline { view, a: w[0].center, b: w[1].center }, vec![]));
            }
        }
    }
    (new, name)
}

/// The Note panel's ✓ for a new note at `position`: the note (empty text:
/// none). New notes take Drawing properties › Notes text height (else All
/// annotations'); a typed `height` becomes the new Notes default.
pub fn new_note(d: &mut Drawing, sheet: usize, text: String, height: Option<f64>, style: noshape_model::drawing::note::NoteStyle, position: [f64; 2]) -> Option<Annotation> {
    if text.trim().is_empty() {
        return None;
    }
    let id = d.sheets.get(sheet)?.next_annotation_id();
    let default_h = d.props.ann_type(noshape_model::drawing::sheet::AnnType::Notes).height.unwrap_or(d.props.ann_text_height);
    let height_mm = height.unwrap_or(default_h);
    if height.is_some_and(|h| (h - default_h).abs() > 1e-9) {
        d.props.ann_type_mut(noshape_model::drawing::sheet::AnnType::Notes).height = Some(height_mm);
    }
    Some(Annotation { id, kind: AnnotationKind::Note { text, height_mm, style }, position, ..Default::default() })
}

/// The Geometric tolerance tool's click: the frame (and its extra rows /
/// texts) placed at `position`, without leaders.
pub fn gdt(id: u64, position: [f64; 2], frame: Vec<String>, extra: GdtExtra) -> Annotation {
    Annotation { id, kind: AnnotationKind::Gdt { anchor: position, frame, leaders: vec![], extra: Box::new(extra) }, position, ..Default::default() }
}

/// The Surface finish panel's ✓: the symbol with `value` ("Ra 3.2") where
/// the tool was clicked.
pub fn surface_finish(id: u64, position: [f64; 2], value: String) -> Annotation {
    Annotation { id, kind: AnnotationKind::SurfaceFinish { anchor: position, value, leaders: Vec::new() }, position, ..Default::default() }
}

/// The Surface finish panel's starting value.
pub const SURFACE_FINISH_DEFAULT: &str = "Ra 3.2";

impl SheetPick<'_> {
    /// The Weld symbol tool's first click: the entity the arrow points at
    /// (a snap point or a view's geometry, in its view), else the sheet point.
    pub fn weld_target(&self, c: [f64; 2]) -> (Option<ViewId>, [f64; 2]) {
        match self.snap_at(c).or_else(|| self.any_point(c)) {
            Some(p) => (Some(p.view), p.p),
            None => (None, c),
        }
    }
}

/// The Weld symbol tool's second click: the symbol with `spec`, its
/// reference line starting at `position`.
pub fn weld(id: u64, view: Option<ViewId>, target: [f64; 2], spec: noshape_model::drawing::weld::WeldSpec, position: [f64; 2]) -> Annotation {
    Annotation { id, kind: AnnotationKind::Weld { view, target, spec: Box::new(spec) }, position, ..Default::default() }
}

/// The Revision table dialog (defaults as it opens for a new table).
#[derive(Clone, Copy, Debug)]
pub struct RevisionTableOpts {
    pub see_sheet1: bool,
    pub visible_rows: u32,
    pub newest_top: bool,
    pub hide_callouts: bool,
    pub corner: TableCorner,
}

impl Default for RevisionTableOpts {
    fn default() -> Self {
        Self { see_sheet1: false, visible_rows: 5, newest_top: false, hide_callouts: false, corner: TableCorner::TopRight }
    }
}

/// The dialog's settings as a revision table kind (`old`: the edited
/// table's, keeping its texts).
pub fn revision_table_kind(o: &RevisionTableOpts, old: Option<&TableKind>) -> TableKind {
    let mut k = old.cloned().filter(|k| matches!(k, TableKind::Revision { .. })).unwrap_or_else(TableKind::revision);
    if let TableKind::Revision { see_sheet1, visible_rows, newest_top, hide_callouts, .. } = &mut k {
        (*see_sheet1, *visible_rows, *newest_top, *hide_callouts) = (o.see_sheet1, o.visible_rows, o.newest_top, o.hide_callouts);
    }
    k
}

/// Revision table tool click: the table with its fixed corner at `c`.
pub fn revision_table(id: u64, o: &RevisionTableOpts, c: [f64; 2]) -> Table {
    let mut t = Table::new(id, revision_table_kind(o, None), c);
    t.corner = o.corner;
    t
}

/// The revision new callouts get: the latest (pending first), else "A".
pub fn latest_revision(info: &noshape_model::drawing::build::SheetInfo) -> String {
    info.revisions.last().map(|r| r.revision.clone()).unwrap_or_else(|| noshape_model::release::next_label(None))
}

/// Add revision callout: the triangle with `revision` at `c`.
pub fn revision_callout(id: u64, c: [f64; 2], revision: String) -> Annotation {
    Annotation { id, kind: AnnotationKind::RevisionCallout { anchor: c, revision, group: None }, position: c, ..Default::default() }
}

/// Jogged section: the orthogonal continuation (in the view's frame) from
/// `last` toward `target`.
pub fn ortho_view(last: [f64; 2], target: [f64; 2]) -> [f64; 2] {
    let d = sub(target, last);
    if d[0].abs() >= d[1].abs() { [target[0], last[1]] } else { [last[0], target[1]] }
}

impl SheetPick<'_> {
    /// A click for the jogged / aligned section and broken-out tools: a snap
    /// point (of `view` when given), else the point in the view under `c`.
    pub fn view_point(&self, c: [f64; 2], view: Option<ViewId>) -> Option<PickedPoint> {
        if let Some(p) = self.snap_at(c).filter(|p| view.is_none_or(|v| v == p.view)) {
            return Some(p);
        }
        let v = view.or_else(|| self.view_at(c))?;
        let pl = self.placement(v)?;
        Some(PickedPoint { view: v, p: pl.to_view(c), r: None })
    }

    /// A cutting line's two end segments extended past the view's geometry.
    pub fn extend_line_ends(&self, view: ViewId, points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let mut pts = points.to_vec();
        let Some(g) = self.geoms.get(&view) else { return pts };
        let n = pts.len();
        if n < 2 {
            return pts;
        }
        let margin = (g.max[0] - g.min[0]).max(g.max[1] - g.min[1]) * 0.08 + 1e-4;
        let corners = [[g.min[0] - margin, g.min[1] - margin], [g.max[0] + margin, g.min[1] - margin], [g.max[0] + margin, g.max[1] + margin], [g.min[0] - margin, g.max[1] + margin]];
        let reach = |from: [f64; 2], dir: [f64; 2]| corners.iter().map(|k| dot(sub(*k, from), dir)).fold(0.0f64, f64::max);
        let d0 = norm(sub(pts[0], pts[1]));
        let t0 = reach(pts[1], d0).max(len(sub(pts[0], pts[1])));
        pts[0] = add2(pts[1], mul(d0, t0));
        let d1 = norm(sub(pts[n - 1], pts[n - 2]));
        let t1 = reach(pts[n - 2], d1).max(len(sub(pts[n - 1], pts[n - 2])));
        pts[n - 1] = add2(pts[n - 2], mul(d1, t1));
        pts
    }
}

/// A general table cell typed (✓ of the cell editor): the cell grid grows
/// to hold `row`, `col`.
pub fn set_general_cell(t: &mut Table, row: usize, col: usize, text: &str) {
    if let TableKind::General { cells, .. } = &mut t.kind {
        while cells.len() <= row {
            cells.push(vec![]);
        }
        let n = cells.iter().map(|r| r.len()).max().unwrap_or(0).max(col + 1);
        for r in cells.iter_mut() {
            r.resize(n, String::new());
        }
        cells[row][col] = text.to_string();
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
        _ => TableKind::Hole { view, origin: origin.as_ref().map(|o| o.p).unwrap_or([0.0; 2]), origin_ref: origin.and_then(|o| o.r), precision: None, flip_x: false, flip_y: false , show_location: true, bottom_to_top: false },
    };
    let name = match kind {
        TableTool::Bom => "BOM table",
        TableTool::CutList => "Cut list table",
        _ => "Hole table",
    };
    (Table::new(id, tk, position), name)
}

impl SheetPick<'_> {
    /// The Broken-out section dialog's default depth: half the model's
    /// depth along the line of sight (10 mm when the view isn't built).
    pub fn broken_out_default_depth(&self, view: ViewId) -> f64 {
        (|| {
            let g = self.geoms.get(&view)?;
            let (key, part) = views::view_source(self.drawing.sheets.get(self.sheet)?, view)?;
            let src = self.sources.get(&key)?;
            Some(views::model_depth(&src.context, part.as_ref(), g.frame.as_ref()?) / 2.0)
        })()
        .unwrap_or(0.01)
    }
}

/// Edit hatch section... (live asb 10-23 views (4)): the region of the
/// view's cut containing `at` (view coordinates) takes `style` (its `at` is
/// set to `at`; None: back to the drawing's hatch). False when `at` isn't
/// in a region of `geo`'s cut.
pub fn set_section_hatch(v: &mut DrawingView, geo: &views::ViewGeometry, at: [f64; 2], style: Option<SectionHatch>) -> bool {
    let Some(regions) = views::section_regions(&geo.section_segments) else { return false };
    let Some(k) = views::section_region_at(&regions, at) else { return false };
    v.section_hatches.retain(|h| views::section_region_at(&regions, h.at) != Some(k));
    if let Some(h) = style {
        v.section_hatches.push(SectionHatch { at, ..h });
    }
    true
}

/// A crop or broken-out boundary left on the sheet when its cut goes:
/// view coordinates (meters).
pub enum Boundary {
    Circle { center: [f64; 2], radius: f64 },
    Polygon(Vec<[f64; 2]>),
    Spline(Vec<[f64; 2]>),
}

/// Remove crop / Remove section keep their boundary (live asb 10-23 views
/// (3)): it stays on the sheet as a plain sketch circle, polygon or closed
/// spline (sheet mm) in view `view`'s placement.
pub fn keep_boundary(sheet: &mut Sheet, pl: &ViewPlacement, b: Boundary) {
    use noshape_sketch::DVec2;
    let at = |p: [f64; 2]| {
        let s = pl.to_sheet(p);
        DVec2::new(s[0], s[1])
    };
    let sk = &mut sheet.sketch.sketch;
    match b {
        Boundary::Circle { center, radius } => {
            sk.add_circle(at(center), pl.len_to_sheet(radius));
        }
        Boundary::Polygon(pts) if pts.len() >= 2 => {
            let q: Vec<DVec2> = pts.iter().map(|p| at(*p)).collect();
            for (i, a) in q.iter().enumerate() {
                sk.add_line(*a, q[(i + 1) % q.len()]);
            }
        }
        Boundary::Spline(pts) if pts.len() >= 3 => {
            let q: Vec<DVec2> = pts.iter().map(|p| at(*p)).collect();
            if sk.add_closed_fit_spline(&q).is_err() {
                for (i, a) in q.iter().enumerate() {
                    sk.add_line(*a, q[(i + 1) % q.len()]);
                }
            }
        }
        _ => {}
    }
}

/// A title block field's text (its cell edited as a note); a key the
/// layout lacks is added.
pub fn set_title_field(sheet: &mut Sheet, key: &str, text: &str) {
    match sheet.title_block.fields.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = text.to_string(),
        None => sheet.title_block.fields.push((key.to_string(), text.to_string())),
    }
}

/// Remove crop: the view uncropped, its boundary kept as a sketch entity.
pub fn remove_crop(sheet: &mut Sheet, pl: Option<&ViewPlacement>, view: ViewId) {
    let Some(crop) = sheet.view_mut(view).and_then(|v| v.crop.take()) else { return };
    if let Some(pl) = pl {
        let b = match crop {
            Crop::Circle { center, radius } => Boundary::Circle { center, radius },
            Crop::Polygon(p) => Boundary::Polygon(p),
            Crop::Spline(p) => Boundary::Spline(p),
        };
        keep_boundary(sheet, pl, b);
    }
}

/// Remove section on broken-out section `index` of `view`: that one goes,
/// its profile kept as a sketch spline (polygon).
pub fn remove_broken_out(sheet: &mut Sheet, pl: Option<&ViewPlacement>, view: ViewId, index: usize) {
    let Some(v) = sheet.view_mut(view) else { return };
    if index >= v.broken_out.len() {
        return;
    }
    let b = v.broken_out.remove(index);
    if let Some(pl) = pl {
        keep_boundary(sheet, pl, if b.spline { Boundary::Spline(b.profile) } else { Boundary::Polygon(b.profile) });
    }
}

/// Why a broken-out section can't go on view `view` (help
/// broken_out_section_view: not from Section, Detail or Aligned section
/// views), or None.
pub fn broken_out_refused(sheet: &noshape_model::drawing::Sheet, view: ViewId) -> Option<&'static str> {
    use noshape_model::drawing::ViewKind;
    matches!(sheet.view(view)?.kind, ViewKind::Section { .. } | ViewKind::Detail { .. } | ViewKind::AlignedSection { .. }).then_some("Broken-out sections can't be created from section, detail or aligned section views")
}

/// The Broken-out section dialog's ✓: a blind depth (meters, > 0) or up to
/// a model point. `None`: invalid depth. New profiles are splines.
pub fn broken_out(profile: Vec<[f64; 2]>, spline: bool, depth: Option<f64>, depth_ref: Option<PointRef>) -> Option<BrokenOut> {
    let depth = depth.filter(|d| d.is_finite() && *d > 0.0).or(depth_ref.as_ref().map(|_| 0.0))?;
    Some(BrokenOut { profile, spline, depth, depth_ref })
}

/// Add broken-out section `b` to a view, or replace its `edit`th one.
pub fn set_broken_out(v: &mut DrawingView, edit: Option<usize>, b: BrokenOut) {
    match edit {
        Some(i) if i < v.broken_out.len() => v.broken_out[i] = b,
        _ => v.broken_out.push(b),
    }
}

impl SheetPick<'_> {
    /// The Hatch region dialog's click: the smallest closed sheet-sketch
    /// profile containing sheet point `s`, else the smallest closed region
    /// of the visible edges of the view there (view coordinates). Detail and
    /// break views take no hatches.
    pub fn hatch_profile_at(&self, s: [f64; 2]) -> Option<(Profile, Option<ViewId>)> {
        use noshape_sketch::DVec2;
        let sheet = self.drawing.sheets.get(self.sheet)?;
        let c = DVec2::new(s[0], s[1]);
        let regions = sheet.sketch.sketch.regions();
        let pts = |l: &noshape_sketch::ProfileLoop, tol: f64| l.tessellate(tol).into_iter().map(|p| [p.x, p.y]).collect::<Vec<_>>();
        if let Some(r) = regions.iter().filter(|r| r.contains(c)).min_by(|a, b| a.area().abs().total_cmp(&b.area().abs())) {
            return Some(((pts(&r.outer, 0.05), r.holes.iter().map(|h| pts(h, 0.05)).collect()), None));
        }
        let v = self.view_at(s)?;
        let view = sheet.view(v)?;
        if matches!(view.kind, ViewKind::Detail { .. }) || !view.breaks.is_empty() {
            return None;
        }
        let pl = self.placement(v)?;
        let g = self.geoms.get(&v)?;
        let mut sk = noshape_sketch::Sketch::new();
        let mut n = 0;
        for e in g.edges.iter().filter(|e| e.visible) {
            for w in e.points.windows(2) {
                if ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt() > 1e-9 {
                    sk.add_line(DVec2::new(w[0][0], w[0][1]), DVec2::new(w[1][0], w[1][1]));
                    n += 1;
                }
            }
            if n > 6000 {
                break;
            }
        }
        let p = pl.to_view(s);
        let c = DVec2::new(p[0], p[1]);
        let regions = sk.regions();
        let r = regions.iter().filter(|r| r.contains(c)).min_by(|a, b| a.area().abs().total_cmp(&b.area().abs()))?;
        Some(((pts(&r.outer, 1e-5), r.holes.iter().map(|h| pts(h, 1e-5)).collect()), Some(v)))
    }
}

/// The Hatch region dialog's profile list after a click picked `picked`: a
/// profile already listed toggles off; editing a hatch, it replaces the one.
pub fn hatch_toggle(profiles: &mut Vec<(Profile, Option<ViewId>)>, picked: (Profile, Option<ViewId>), editing: bool) {
    match profiles.iter().position(|p| *p == picked) {
        Some(i) => {
            profiles.remove(i);
        }
        None if editing => *profiles = vec![picked],
        None => profiles.push(picked),
    }
}

/// The Hatch region dialog's ✓ on the sheet sketch: replace hatch `edit`
/// (if any) and add `regions`.
pub fn apply_hatches(sketch: &mut noshape_model::drawing::sheet_sketch::SheetSketch, edit: Option<u64>, regions: Vec<noshape_model::drawing::sheet_sketch::HatchRegion>) {
    if let Some(id) = edit {
        sketch.hatches.retain(|x| x.id != id);
    }
    sketch.hatches.extend(regions);
}

/// The Custom table dialog's table kind: Part Studio `studio`, custom table
/// `name` of Feature Studio `fs` (at `fs_version`, another document's
/// version, or this workspace); editing keeps the pinned Part Studio
/// version of `old`.
pub fn custom_table_kind(studio: noshape_model::document::TabId, fs: noshape_model::document::TabId, name: String, fs_version: Option<String>, bottom_to_top: bool, old: Option<&TableKind>) -> TableKind {
    let version = match old {
        Some(TableKind::Custom { version, .. }) => version.clone(),
        _ => None,
    };
    TableKind::Custom { studio, version, fs, name, fs_version, bottom_to_top }
}

/// A custom table placed by the Custom table dialog's click.
pub fn custom_table(id: u64, kind: TableKind, corner: TableCorner, position: [f64; 2]) -> Table {
    let mut t = Table::new(id, kind, position);
    t.corner = corner;
    t
}

impl SheetPick<'_> {
    /// Insert image: the box from the drag start to the pointer, the image's
    /// aspect kept; a plain click places it 30 mm high at the click.
    pub fn image_box(&self, start: [f64; 2], aspect: f64) -> ([f64; 2], [f64; 2]) {
        let c = self.cursor;
        let (dx, dy) = ((c[0] - start[0]).abs(), (c[1] - start[1]).abs());
        if dx.max(dy) < self.tol_mm(4.0) {
            return (start, [30.0 * aspect, 30.0]);
        }
        let h = (dx / aspect).max(dy);
        let w = h * aspect;
        let x = if c[0] < start[0] { start[0] - w } else { start[0] };
        let y = if c[1] < start[1] { start[1] - h } else { start[1] };
        ([x, y], [w, h])
    }
}

/// An image file's width / height (Insert image keeps it).
pub fn image_aspect(bytes: &[u8]) -> Result<f64, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    Ok(img.width() as f64 / img.height().max(1) as f64)
}

/// Insert image: the image goes on the sheet; its id.
pub fn add_sheet_image(s: &mut Sheet, name: String, data_base64: std::sync::Arc<str>, pos: [f64; 2], size: [f64; 2]) -> u64 {
    let id = s.images.iter().map(|i| i.id).max().unwrap_or(0) + 1;
    s.images.push(SheetImage { id, name, data_base64, pos, size });
    id
}

/// Insert DXF or DWG: the file's model-space wireframe as sheet sketch
/// geometry at its own coordinates, in the file's unit (else the
/// drawing's); the entities created (the caller solves the sketch).
/// Insert DXF or DWG: the file's model-space wireframe becomes sheet
/// sketch geometry and its TEXT / MTEXT editable notes (live: "simple
/// notes become editable Notes", colors dropped), at the file's own
/// coordinates in its unit (else the drawing's). The sheet sketch is left
/// unsolved. Returns the entity and note counts.
pub fn insert_cad(sheet: &mut Sheet, bytes: &[u8], units_mm: bool) -> Result<(usize, usize), String> {
    let d = noshape_sketch::cad::parse_cad(bytes).map_err(|e| e.to_string())?;
    let (spaces, _) = noshape_sketch::cad::parse_cad_spaces(bytes).map_err(|e| e.to_string())?;
    let scale = d.unit_scale.map(|m| m * 1000.0).unwrap_or(if units_mm { 1.0 } else { 25.4 });
    let n = sheet.sketch.sketch.insert_drawing(&d, scale, noshape_sketch::DVec2::ZERO).len();
    let mut notes = 0;
    for t in spaces.iter().filter(|s| s.model).flat_map(|s| &s.texts) {
        // TEXT's %% control codes (MTEXT comes plain).
        let text = t.text.replace("%%c", "Ø").replace("%%C", "Ø").replace("%%d", "°").replace("%%D", "°").replace("%%p", "±").replace("%%P", "±");
        if text.trim().is_empty() {
            continue;
        }
        // The file's anchor is the baseline start; a note's is its top left.
        let (h, p) = (t.height * scale, t.position * scale);
        let (s, c) = t.rotation.sin_cos();
        let style = note::NoteStyle { angle: t.rotation, ..Default::default() };
        let id = sheet.next_annotation_id();
        sheet.annotations.push(Annotation { id, kind: AnnotationKind::Note { text, height_mm: h, style }, position: [p.x - s * h, p.y + c * h], ..Default::default() });
        notes += 1;
    }
    Ok((n, notes))
}

// ------------------------------------------------------------ drawing sketch

impl SheetPick<'_> {
    /// The sheet sketch point nearest `c` (within 6 px), if any.
    pub fn sketch_point_at(&self, c: [f64; 2]) -> Option<(noshape_sketch::EntityId, [f64; 2])> {
        let tol = self.tol_mm(6.0);
        let sk = &self.drawing.sheets.get(self.sheet)?.sketch.sketch;
        sk.entities()
            .filter(|e| matches!(e.geometry, noshape_sketch::Geometry::Point { .. }) && !e.external)
            .filter_map(|e| sk.point(e.id).map(|p| (e.id, [p.x, p.y])))
            .filter(|(_, p)| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt() < tol)
            .min_by(|a, b| {
                let d = |p: [f64; 2]| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2);
                d(a.1).total_cmp(&d(b.1))
            })
    }

    /// The hovered sheet sketch entity (curves via their pick id).
    pub fn sketch_entity_at(&self) -> Option<noshape_sketch::EntityId> {
        match self.built?.picks.get(self.hover? as usize)? {
            noshape_model::drawing::build::PickTarget::SketchEntity(id) => Some(noshape_sketch::EntityId(*id)),
            _ => None,
        }
    }

    /// Where a sketch click lands: an existing sketch point, a view snap
    /// point, or the pointer (with horizontal / vertical inference from
    /// `from`, within 1.5°), else `aligned` (the pointer aligned with woken
    /// end points). Returns (point, existing point id, inferred H/V).
    pub fn sketch_target(&self, from: Option<[f64; 2]>, aligned: [f64; 2]) -> ([f64; 2], Option<noshape_sketch::EntityId>, Option<super::SkCon>) {
        let c = self.cursor;
        if let Some((id, p)) = self.sketch_point_at(c) {
            return (p, Some(id), None);
        }
        if let Some(p) = self.snap_at(c).and_then(|s| self.pick_sheet(&s)) {
            return (p, None, None);
        }
        if let Some(f) = from {
            let d = [c[0] - f[0], c[1] - f[1]];
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if l > 1e-6 {
                let a = d[1].atan2(d[0]).to_degrees().rem_euclid(180.0);
                if !(1.5..=178.5).contains(&a) {
                    return ([c[0], f[1]], None, Some(super::SkCon::Horizontal));
                }
                if (a - 90.0).abs() < 1.5 {
                    return ([f[0], c[1]], None, Some(super::SkCon::Vertical));
                }
            }
        }
        (aligned, None, None)
    }
}

fn v2(p: [f64; 2]) -> noshape_sketch::DVec2 {
    noshape_sketch::DVec2::new(p[0], p[1])
}

/// Line tool: a line from `a` (or existing point `from`) to `p` (or existing
/// point `end`), Horizontal / Vertical when inferred; its end point.
pub fn sketch_line(sk: &mut noshape_sketch::Sketch, from: Option<noshape_sketch::EntityId>, a: [f64; 2], end: Option<noshape_sketch::EntityId>, p: [f64; 2], infer: Option<super::SkCon>) -> noshape_sketch::EntityId {
    use noshape_sketch::ConstraintKind;
    let start = from.unwrap_or_else(|| sk.add_point(v2(a)));
    let end = end.unwrap_or_else(|| sk.add_point(v2(p)));
    if let Ok(line) = sk.add_line_between(start, end) {
        match infer {
            Some(super::SkCon::Horizontal) => {
                let _ = sk.add_constraint(ConstraintKind::Horizontal { a: line, b: None });
            }
            Some(super::SkCon::Vertical) => {
                let _ = sk.add_constraint(ConstraintKind::Vertical { a: line, b: None });
            }
            _ => {}
        }
    }
    end
}

/// Center point circle (on existing center point `from`, else at `a`)
/// through `p`, or a corner / center point rectangle from `a` to `p`.
pub fn sketch_two_click(sk: &mut noshape_sketch::Sketch, tool: super::SkTool, from: Option<noshape_sketch::EntityId>, a: [f64; 2], p: [f64; 2]) {
    match tool {
        super::SkTool::Circle => {
            let r = ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
            if r > 1e-6 {
                match from {
                    Some(c) => {
                        let _ = sk.add_circle_at(c, r);
                    }
                    None => {
                        sk.center_circle(v2(a), r);
                    }
                }
            }
        }
        super::SkTool::CornerRect => {
            sk.corner_rectangle(v2(a), v2(p));
        }
        _ => {
            sk.center_rectangle(v2(a), v2(p));
        }
    }
}

/// The spline through the clicked points (repeats dropped); false with
/// fewer than two.
pub fn sketch_spline(sk: &mut noshape_sketch::Sketch, pts: &[[f64; 2]]) -> bool {
    let mut pts = pts.to_vec();
    pts.dedup_by(|a, b| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() < 1e-6);
    if pts.len() < 2 {
        return false;
    }
    let p: Vec<noshape_sketch::DVec2> = pts.iter().map(|p| v2(*p)).collect();
    let _ = sk.add_fit_spline(&p);
    true
}

/// Trim: a point is deleted, a curve trimmed at `c`.
pub fn sketch_trim(sk: &mut noshape_sketch::Sketch, id: noshape_sketch::EntityId, c: [f64; 2]) {
    if matches!(sk.entity(id).map(|e| &e.geometry), Some(noshape_sketch::Geometry::Point { .. })) {
        sk.remove_entity(id);
    } else {
        let _ = sk.trim(id, v2(c));
    }
}

// ------------------------------------------------------------ inspection items

/// Inspection item: annotation `id`'s number (None removes it).
pub fn set_inspection(s: &mut Sheet, id: u64, n: Option<u32>) {
    if let Some(a) = s.annotations.iter_mut().find(|a| a.id == id) {
        a.inspection = n;
    }
}

/// Drag an inspection item onto another annotation: the number moves.
pub fn move_inspection(s: &mut Sheet, from: u64, to: u64) {
    let n = s.annotations.iter_mut().find(|a| a.id == from).and_then(|a| a.inspection.take());
    set_inspection(s, to, n);
}

/// A Drawing properties panel edit: with ☑ Lock drawing properties on
/// before and after, nothing but the lock changes; the drawing's metric flag
/// follows the chosen units.
pub fn set_drawing_props(d: &mut Drawing, f: impl FnOnce(&mut DrawingProps)) {
    let before = d.props.clone();
    f(&mut d.props);
    if before.locked && d.props.locked {
        d.props = before;
        return;
    }
    if let Some(u) = d.props.units {
        d.units_mm = !u.imperial();
    }
}

impl SheetPick<'_> {
    /// Auxiliary view: the fold line under the pointer for the dialog's
    /// mode (0 Normal: a straight model edge; 1 In line: a sheet sketch line
    /// or a centerline annotation; 2 Axis: a hole's axis seen side-on, the
    /// view looking down it): (view, edge direction on the sheet, anchor in
    /// view coordinates).
    pub fn aux_pick(&self, mode: u8) -> Option<(ViewId, [f64; 2], [f64; 2])> {
        let c = self.cursor;
        let sheet = self.drawing.sheets.get(self.sheet)?;
        match mode {
            1 => {
                let tol = self.tol_mm(6.0);
                let sk = &sheet.sketch.sketch;
                let sketch_line = sk.entities().find_map(|e| {
                    let noshape_sketch::Geometry::Line { start, end } = e.geometry else { return None };
                    let (a, b) = (sk.point(start)?, sk.point(end)?);
                    let (a, b) = ([a.x, a.y], [b.x, b.y]);
                    (views::dist_to_segment(c, a, b) < tol).then_some((a, b))
                });
                let (sa, sb) = sketch_line.or_else(|| {
                    sheet.annotations.iter().find_map(|a| {
                        let AnnotationKind::Centerline { view, a: pa, b: pb } = &a.kind else { return None };
                        let pl = self.placement(*view)?;
                        let (sa, sb) = (pl.to_sheet(*pa), pl.to_sheet(*pb));
                        (views::dist_to_segment(c, sa, sb) < tol).then_some((sa, sb))
                    })
                })?;
                let mid = [(sa[0] + sb[0]) / 2.0, (sa[1] + sb[1]) / 2.0];
                let v = self.view_at(mid)?;
                let pl = self.placement(v)?;
                Some((v, norm(sub(sb, sa)), pl.to_view(mid)))
            }
            2 => {
                let v = self.view_at(c)?;
                let pl = self.placement(v)?;
                let g = self.geoms.get(&v)?;
                let vp = pl.to_view(c);
                let tol = self.tol_mm(8.0) / (pl.scale * 1000.0);
                let axis = g
                    .axes
                    .iter()
                    .copied()
                    .filter(|a| views::dist_to_segment(vp, a[0], a[1]) < tol)
                    .min_by(|a, b| views::dist_to_segment(vp, a[0], a[1]).total_cmp(&views::dist_to_segment(vp, b[0], b[1])))
                    .or_else(|| match self.line_pick() {
                        Some(Pick::Line { a, b }) => Some([a.p, b.p]),
                        _ => None,
                    })?;
                let (sa, sb) = (pl.to_sheet(axis[0]), pl.to_sheet(axis[1]));
                let d = norm(sub(sb, sa));
                Some((v, [-d[1], d[0]], [(axis[0][0] + axis[1][0]) / 2.0, (axis[0][1] + axis[1][1]) / 2.0]))
            }
            _ => self.aux_edge(),
        }
    }
}

impl SheetPick<'_> {
    /// Datum tool's first click on a GD&T frame edge or a dimension's lines
    /// (the red crosshair): the point on the line and where it is.
    pub fn datum_attach_at(&self, c: [f64; 2]) -> Option<([f64; 2], DatumOn)> {
        let b = self.built?;
        let sheet = self.drawing.sheets.get(self.sheet)?;
        let tol = self.tol_mm(5.0);
        let mut best: Option<(f64, [f64; 2], DatumOn)> = None;
        let mut count: std::collections::HashMap<u64, u32> = Default::default();
        for l in &b.geometry.lines {
            let Some(noshape_model::drawing::build::PickTarget::Annotation(id)) = b.picks.get(l.pick as usize) else { continue };
            let line = *count.entry(*id).and_modify(|n| *n += 1).or_insert(0);
            let ok = sheet.annotations.iter().find(|a| a.id == *id).is_some_and(|a| matches!(a.kind, AnnotationKind::Gdt { .. }) || a.kind.is_dimension());
            if !ok {
                continue;
            }
            let n = l.points.len();
            for seg in 0..if l.closed && n > 2 { n } else { n.saturating_sub(1) } {
                let (w0, w1) = (l.points[seg], l.points[(seg + 1) % n]);
                let d = sub(w1, w0);
                let dd = (d[0] * d[0] + d[1] * d[1]).max(1e-12);
                let t = (((c[0] - w0[0]) * d[0] + (c[1] - w0[1]) * d[1]) / dd).clamp(0.0, 1.0);
                let q = [w0[0] + d[0] * t, w0[1] + d[1] * t];
                let dist = len(sub(c, q));
                if dist < tol && best.as_ref().is_none_or(|b| dist < b.0) {
                    best = Some((dist, q, DatumOn { ann: *id, line, seg: seg as u32, t }));
                }
            }
        }
        best.map(|(_, q, o)| (q, o))
    }
}

/// Delete: the selected views (with their labels and indicators),
/// annotations (callouts grouped with them; a datum from the model is only
/// hidden), tables, sketch entities, hatches, images, view breaks and
/// automatic centermarks.
pub fn delete_selection(s: &mut Sheet, sel: &[super::Sel]) {
    use super::Sel;
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
    for x in sel {
        match x {
            // A label's Delete is its view's (live u-944: the view menu).
            Sel::View(v) | Sel::ViewLabel(v) | Sel::AuxIndicator(v) => s.remove_view(*v),
            // Callouts grouped with the annotation go with it.
            // A datum from the model is hidden, not removed (MBD status › Show).
            Sel::Annotation(a) if s.annotations.iter().any(|n| n.id == *a && n.model.is_some()) => {
                if let Some(m) = s.annotations.iter_mut().find(|n| n.id == *a).and_then(|n| n.model.as_mut()) {
                    m.hidden = true;
                }
            }
            Sel::Annotation(a) => s.annotations.retain(|n| n.id != *a && n.group != Some(*a) && !matches!(n.kind, AnnotationKind::RevisionCallout { group: Some(g), .. } if g == *a)),
            Sel::Table(t) => s.tables.retain(|n| n.id != *t),
            // A hole table's tag axis isn't deleted on its own (live: no Delete in its menu).
            Sel::HoleAxis(_) => {}
            Sel::Sketch(e) => {
                s.sketch.sketch.remove_entity(noshape_sketch::EntityId(*e));
            }
            Sel::Hatch(h) => s.sketch.hatches.retain(|x| x.id != *h),
            Sel::Image(i) => s.images.retain(|x| x.id != *i),
            // Automatic centermarks are deleted one at a time (help H:creating_a_drawing).
            Sel::AutoCentermark(v, c) => {
                if let Some(v) = s.view_mut(*v) {
                    v.deleted_centermarks.push(*c);
                }
            }
            Sel::Break(..) => {}
        }
    }
    let _ = s.sketch.sketch.solve();
}
