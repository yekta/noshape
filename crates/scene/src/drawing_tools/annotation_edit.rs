//! The drawing annotation menus' edits (jogs, leaders and leader nodes,
//! foreshortening, datum target dimensions, groups, Detach, Align
//! centermark), shared by the drawing editor and the agent API.

use super::SheetPick;
use super::jog::seg_dist;
use noshape_model::drawing::sheet::{LeaderPath, LinearFrame, note_leader_paths};
use noshape_model::drawing::{Annotation, AnnotationKind, ExtJog, Leader, LeaderStart, Sheet, ViewId};

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

/// The new leader on annotation `id` (a note's starts at `start`).
pub fn add_leader(s: &mut Sheet, id: u64, leader: Leader, start: LeaderStart) {
    let Some(a) = find(s, id) else { return };
    let note = matches!(a.kind, AnnotationKind::Note { .. });
    if let Some(l) = leaders_mut(a) {
        l.push(if note { Leader { start, ..leader } } else { leader });
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
    Some(AnnotationKind::RadialDimension { view, center, radius, diameter: false, angle, dim_line: false })
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
