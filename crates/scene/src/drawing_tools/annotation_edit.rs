//! The drawing annotation menus' edits (jogs, leaders and leader nodes,
//! foreshortening, datum target dimensions, groups, Detach, Align
//! centermark), shared by the drawing editor and the agent API. Note and
//! callout leaders (live drw2 10-21 g; (6): a callout's leaders work the
//! same): their paths, what a point hits on them, Add node / Remove node /
//! Add leader from a node / Delete / Move by grip.

use super::SheetPick;
use super::jog::seg_dist;
use noshape_model::drawing::sheet::{LeaderPath, LinearFrame, note_leader_paths};
use noshape_model::drawing::build::BuiltSheet;
use noshape_model::drawing::{Annotation, AnnotationKind, ExtJog, Leader, LeaderStart, PointRef, Sheet, ViewId, remove_leader, remove_leader_node};

fn find(s: &mut Sheet, id: u64) -> Option<&mut Annotation> {
    s.annotations.iter_mut().find(|a| a.id == id)
}

/// Add jog on extension line `k` (live drawing/331–340): a 45° diagonal
/// 0.44 in (11.2 mm) outward, centered on the line; a line shorter than
/// about 44.7 mm first moves the dimension line away from the part by
/// `grow`. Returns the jog and `grow`.
pub fn jog_for(f: &LinearFrame, k: usize) -> (ExtJog, f64) {
    let along = |p: [f64; 2]| p[0] * f.dir[0] + p[1] * f.dir[1];
    let (mine, other) = if k == 0 { (f.dp, f.dq) } else { (f.dq, f.dp) };
    const JOG: f64 = 11.2;
    const MIN_LEG: f64 = 44.7;
    let shift = if along(mine) <= along(other) { -JOG } else { JOG };
    let l = f.legs[k].2;
    let grow = (MIN_LEG - l).max(0.0);
    let at = ((l + grow - JOG) / 2.0) / (l + grow).max(1e-9);
    (ExtJog { at, shift, drop: JOG }, grow)
}

/// Apply Add jog (`jog_for`) to dimension `id`.
pub fn add_jog(s: &mut Sheet, id: u64, k: usize, jog: ExtJog, grow: f64) {
    if let Some(x) = find(s, id) {
        if let AnnotationKind::LinearDimension { offset, .. } = &mut x.kind {
            *offset += grow * offset.signum();
        }
        x.jogs[k] = Some(jog);
    }
}

/// A linear dimension's menu › Horizontal / Vertical / Aligned ("Dimension
/// orientation"). False when it isn't a linear dimension.
pub fn set_dimension_orientation(s: &mut Sheet, id: u64, o: noshape_model::drawing::DimOrientation) -> bool {
    match find(s, id).map(|x| &mut x.kind) {
        Some(AnnotationKind::LinearDimension { orientation, .. }) => {
            *orientation = o;
            true
        }
        _ => false,
    }
}

/// A selected dimension's flip node: its arrows the other way ("Flip arrows").
pub fn flip_arrows(s: &mut Sheet, id: u64) -> bool {
    find(s, id).map(|x| x.flip_arrows = !x.flip_arrows).is_some()
}

/// A maximum / minimum dimension's "Arc condition (end N)" › Center /
/// Minimum / Maximum ("Arc condition"); `end` 0 or 1.
pub fn set_arc_condition(s: &mut Sheet, id: u64, end: usize, c: noshape_model::drawing::ArcCondition) -> bool {
    match find(s, id).map(|x| &mut x.kind) {
        Some(AnnotationKind::MaxMinDimension { ca, cb, .. }) => {
            if end == 0 { *ca = c } else { *cb = c }
            true
        }
        _ => false,
    }
}

/// A section view's "Cutting line angle" panel: the two-point cutting line
/// turned to `deg` about its middle, its length kept. False without one.
pub fn set_cutting_line_angle(s: &mut Sheet, view: ViewId, deg: f64) -> bool {
    let Some(p) = s.section_points(view).filter(|p| p.len() == 2) else { return false };
    let m = [(p[0][0] + p[1][0]) / 2.0, (p[0][1] + p[1][1]) / 2.0];
    let h = (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]) / 2.0;
    let (sn, c) = deg.to_radians().sin_cos();
    s.set_section_points(view, vec![[m[0] - c * h, m[1] - sn * h], [m[0] + c * h, m[1] + sn * h]]);
    true
}

/// Remove jog on extension line `k`.
pub fn remove_jog(s: &mut Sheet, id: u64, k: usize) {
    if let Some(x) = find(s, id) {
        x.jogs[k] = None;
    }
}

/// The leaders of an annotation that has them (GD&T, notes, callouts,
/// surface finish symbols, weld symbols).
pub fn leaders_mut(a: &mut Annotation) -> Option<&mut Vec<Leader>> {
    match &mut a.kind {
        AnnotationKind::Gdt { leaders, .. } | AnnotationKind::SurfaceFinish { leaders, .. } => Some(leaders),
        AnnotationKind::Note { style, .. } => Some(&mut style.leaders),
        AnnotationKind::Weld { spec, .. } => Some(&mut spec.leaders),
        AnnotationKind::Balloon { callout, .. } => Some(&mut callout.leaders),
        AnnotationKind::HoleCallout { leaders, .. } => Some(leaders),
        _ => None,
    }
}

/// The leaders of an annotation that has them.
pub fn leaders(a: &Annotation) -> Option<&Vec<Leader>> {
    match &a.kind {
        AnnotationKind::Gdt { leaders, .. } | AnnotationKind::SurfaceFinish { leaders, .. } => Some(leaders),
        AnnotationKind::Note { style, .. } => Some(&style.leaders),
        AnnotationKind::Weld { spec, .. } => Some(&spec.leaders),
        AnnotationKind::Balloon { callout, .. } => Some(&callout.leaders),
        AnnotationKind::HoleCallout { leaders, .. } => Some(leaders),
        _ => None,
    }
}

/// Add leader's click at `c`: a leader to the snap point (or any point of
/// a view) there, else to the sheet point.
pub fn leader_to(pk: &SheetPick, c: [f64; 2]) -> Leader {
    match pk.snap_at(c).or_else(|| pk.any_point(c)) {
        Some(p) => Leader { view: Some(p.view), point: p.p, nodes: Vec::new(), ..Default::default() },
        None => Leader { view: None, point: c, nodes: Vec::new(), ..Default::default() },
    }
}

/// The new leader on annotation `id` (a note's or callout's starts at
/// `start`: a callout's indexes [`all_leaders`]).
pub fn add_leader(s: &mut Sheet, id: u64, leader: Leader, start: LeaderStart) {
    let Some(a) = find(s, id) else { return };
    let branches = matches!(a.kind, AnnotationKind::Note { .. } | AnnotationKind::Balloon { .. });
    if let Some(l) = leaders_mut(a) {
        l.push(if branches { Leader { start, ..leader } } else { leader });
    }
}

/// A note's own Add leader: from the existing landing's bend (live: that
/// side, wherever the pointer is), else from the note's side.
pub fn note_leader_start(a: &Annotation) -> LeaderStart {
    let first = match &a.kind {
        AnnotationKind::Note { style, .. } => style.leaders.iter().position(|l| l.start.is_side()),
        _ => None,
    };
    first.map_or(LeaderStart::Side, LeaderStart::Bend)
}

/// Remove leaders.
pub fn remove_leaders(s: &mut Sheet, id: u64) {
    if let Some(l) = find(s, id).and_then(leaders_mut) {
        l.clear();
    }
}

/// A note's unrotated box (sheet mm) and its rotation.
pub fn note_box(a: &Annotation) -> Option<([f64; 4], f64)> {
    let AnnotationKind::Note { text, height_mm, style } = &a.kind else { return None };
    let l = noshape_model::drawing::note::layout(&noshape_model::drawing::note::fractions(text), *height_mm, style, a.position);
    Some((l.bbox, if style.leaders.is_empty() { style.angle } else { 0.0 }))
}

impl SheetPick<'_> {
    /// A note's leader paths on the sheet.
    pub fn note_leader_paths(&self, a: &Annotation) -> Vec<LeaderPath> {
        let AnnotationKind::Note { style, .. } = &a.kind else { return Vec::new() };
        let Some((bbox, _)) = note_box(a) else { return Vec::new() };
        let targets: Vec<[f64; 2]> = style
            .leaders
            .iter()
            .map(|ld| match ld.view {
                Some(v) => self.placement(v).map(|pl| pl.to_sheet(ld.point)).unwrap_or(ld.point),
                None => ld.point,
            })
            .collect();
        note_leader_paths(bbox, &style.leaders, &targets)
    }
}

/// Add node at sheet point `p` on segment `seg` of a note leader's path
/// (`head`: the path point the nodes start at).
pub fn add_node(l: &mut [Leader], leader: usize, seg: usize, head: usize, p: [f64; 2]) {
    if let Some(ld) = l.get_mut(leader) {
        // Segment `seg` runs from path point `seg` to `seg + 1`; the nodes
        // start at path point `head`.
        let at = (seg + 1).saturating_sub(head).min(ld.nodes.len());
        ld.nodes.insert(at, p);
    }
}

/// The segment of `path` nearest `p` and its distance.
pub fn nearest_segment(path: &LeaderPath, p: [f64; 2]) -> Option<(usize, f64)> {
    path.points.windows(2).enumerate().map(|(i, w)| (i, seg_dist(p, w[0], w[1]))).min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Foreshorten (radial dimensions): the jog at 55 % of the radius, 20 % to
/// the side. None for other kinds.
pub fn foreshortened(k: &AnnotationKind) -> Option<AnnotationKind> {
    let &AnnotationKind::RadialDimension { view, center, radius, angle, .. } = k else { return None };
    let (d, w) = ([angle.cos(), angle.sin()], [-angle.sin(), angle.cos()]);
    let jog = [center[0] + d[0] * radius * 0.55 + w[0] * radius * 0.2, center[1] + d[1] * radius * 0.55 + w[1] * radius * 0.2];
    Some(AnnotationKind::ForeshortenedRadial { view, center, radius, angle, jog })
}

/// Remove foreshorten: back to a radius dimension.
pub fn unforeshortened(k: &AnnotationKind) -> Option<AnnotationKind> {
    let &AnnotationKind::ForeshortenedRadial { view, center, radius, angle, .. } = k else { return None };
    Some(AnnotationKind::RadialDimension { view, center, radius, diameter: false, angle, dim_line: false, knee: None })
}

/// Datum targets: Hide dimension (`hide`) / Show dimensions ▸ This target
/// or All targets (`all`).
pub fn set_target_dims_hidden(s: &mut Sheet, id: u64, hide: bool, all: bool) {
    for x in s.annotations.iter_mut().filter(|x| (all || x.id == id) && matches!(x.kind, AnnotationKind::DatumTarget { .. })) {
        if let Some(o) = x.target.as_mut() {
            o.hide_dims = hide;
        }
    }
}

/// Group (notes and callouts with a view): they move with the view.
pub fn group_with_view(s: &mut Sheet, ids: &[u64], view: Option<ViewId>) {
    s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.view_group = view);
}

/// Group annotation: a new annotation group of `ids` (None: Remove
/// annotation from group).
pub fn group_annotations(s: &mut Sheet, ids: &[u64], group: bool) {
    let g = group.then(|| s.annotations.iter().filter_map(|x| x.ann_group).max().unwrap_or(0) + 1);
    s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.ann_group = g);
}

/// Detach from geometry: the annotation keeps its place, no longer follows
/// the model.
pub fn detach(s: &mut Sheet, id: u64) {
    if let Some(x) = find(s, id) {
        x.refs.clear();
    }
}

/// Align centermark: its arms turn to the straight edge from `a` to `b`
/// (view coordinates, as `Pick::Line` gives them).
pub fn align_centermark(s: &mut Sheet, id: u64, a: [f64; 2], b: [f64; 2]) {
    let angle = (b[1] - a[1]).atan2(b[0] - a[0]);
    if let Some(AnnotationKind::Centermark { angle: x, .. }) = find(s, id).map(|x| &mut x.kind) {
        *x = angle;
    }
}

/// What a sheet point hits on a note's or callout's leaders (`leader`: the
/// note's leader index, a callout's [`all_leaders`] index).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LeaderHit {
    /// Leader `leader`'s line, segment `seg` of its path, at `p`.
    Stroke { id: u64, leader: usize, seg: usize, p: [f64; 2] },
    Node { id: u64, leader: usize, node: usize },
    /// The end of a leader's landing (leaders from the note's side).
    Bend { id: u64, leader: usize },
}

/// A note's or callout's leaders as one list: a note's own, a callout's
/// `CalloutStyle::all_leaders` (0 = the leader to the anchor, then the
/// added ones).
pub fn all_leaders(a: &Annotation) -> Option<Vec<Leader>> {
    match &a.kind {
        AnnotationKind::Note { style, .. } => Some(style.leaders.clone()),
        AnnotationKind::Balloon { view, anchor, callout, .. } => Some(callout.all_leaders(*view, *anchor)),
        _ => None,
    }
}

/// Edit [`all_leaders`] of note or callout `a`. False for other annotations.
pub fn with_all_leaders(a: &mut Annotation, f: impl FnOnce(&mut Vec<Leader>)) -> bool {
    match &mut a.kind {
        AnnotationKind::Note { style, .. } => f(&mut style.leaders),
        AnnotationKind::Balloon { view, anchor, callout, .. } => {
            let mut all = callout.all_leaders(*view, *anchor);
            f(&mut all);
            callout.set_all_leaders(all);
        }
        _ => return false,
    }
    true
}

/// A note's or callout's leader paths on the sheet, by [`all_leaders`]
/// index (a callout's as last drawn in `built`).
pub fn leader_paths(built: Option<&BuiltSheet>, a: &Annotation) -> Vec<(usize, LeaderPath)> {
    match &a.kind {
        AnnotationKind::Note { style, .. } => {
            let Some((bbox, _)) = note_box(a) else { return Vec::new() };
            let targets: Vec<[f64; 2]> = style
                .leaders
                .iter()
                .map(|ld| match ld.view {
                    Some(v) => built.and_then(|b| b.placement(v)).map(|pl| pl.to_sheet(ld.point)).unwrap_or(ld.point),
                    None => ld.point,
                })
                .collect();
            note_leader_paths(bbox, &style.leaders, &targets).into_iter().enumerate().collect()
        }
        AnnotationKind::Balloon { .. } => built.map(|b| b.geometry.leader_paths.iter().filter(|(id, ..)| *id == a.id).map(|(_, i, p)| (*i, p.clone())).collect()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The leader under sheet point `c` among `annotations`: a bend or node
/// within `near` first, else a line within `on_line` (live: exactly on the
/// stroke; a few px off is the sheet).
pub fn leader_hit<'a>(built: Option<&BuiltSheet>, annotations: impl IntoIterator<Item = &'a Annotation>, c: [f64; 2], near: f64, on_line: f64) -> Option<LeaderHit> {
    let dist = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let mut stroke = None;
    for a in annotations {
        for (leader, path) in &leader_paths(built, a) {
            let (id, leader) = (a.id, *leader);
            for (k, p) in path.points.iter().enumerate() {
                if dist(*p, c) >= near {
                    continue;
                }
                if path.head == 2 && k == 1 {
                    return Some(LeaderHit::Bend { id, leader });
                }
                if k >= path.head && k + 1 < path.points.len() {
                    return Some(LeaderHit::Node { id, leader, node: k - path.head });
                }
            }
            if stroke.is_none()
                && let Some(seg) = path.points.windows(2).position(|w| seg_dist(c, w[0], w[1]) < on_line)
            {
                stroke = Some(LeaderHit::Stroke { id, leader, seg, p: c });
            }
        }
    }
    stroke
}

/// Remove node `node` of leader `leader` of note or callout `a` (its
/// branches start at the bend).
pub fn remove_node(a: &mut Annotation, leader: usize, node: usize) -> bool {
    with_all_leaders(a, |l| remove_leader_node(l, leader, node))
}

/// Delete on a leader of note or callout `a` (a callout's leader to the
/// anchor leaves it in space).
pub fn delete_leader(a: &mut Annotation, leader: usize) -> bool {
    match &mut a.kind {
        AnnotationKind::Balloon { view, anchor, callout, .. } => {
            callout.remove_leader(leader, *view, *anchor);
            true
        }
        _ => with_all_leaders(a, |l| remove_leader(l, leader)),
    }
}

/// Drag a leader's anchor grip (help note: off an edge detaches it, onto
/// one re-attaches it): its end goes to `point` in `view` (view
/// coordinates), or to sheet point `point` when `view` is None. A
/// callout's leader to the anchor moves only onto a model point (`r`: its
/// BOM item follows it). False when nothing moved.
pub fn move_leader_end(a: &mut Annotation, leader: usize, view: Option<ViewId>, point: [f64; 2], r: Option<PointRef>) -> bool {
    if let AnnotationKind::Balloon { view: v, anchor, callout, .. } = &mut a.kind
        && leader == 0
    {
        let (Some(nv), Some(r)) = (view, r) else { return false };
        *v = nv;
        *anchor = point;
        callout.free = false;
        callout.part = None;
        a.refs.retain(|x| x.slot != 0);
        a.refs.push(noshape_model::drawing::AnnotationRef { slot: 0, point: r });
        return true;
    }
    let mut moved = false;
    with_all_leaders(a, |l| {
        if let Some(ld) = l.get_mut(leader) {
            ld.view = view;
            ld.point = point;
            moved = true;
        }
    });
    moved
}

/// Where a branch of note or callout `a` from `from` starts (sheet mm).
pub fn branch_point(built: Option<&BuiltSheet>, a: &Annotation, from: LeaderStart) -> Option<[f64; 2]> {
    let leaders = all_leaders(a)?;
    match from {
        LeaderStart::Bend(j) => leader_paths(built, a).into_iter().find(|(i, _)| *i == j).map(|(_, p)| p.bend()),
        LeaderStart::Node(j, n) => leaders.get(j).and_then(|l| l.nodes.get(n)).copied(),
        LeaderStart::Side => None,
    }
}
