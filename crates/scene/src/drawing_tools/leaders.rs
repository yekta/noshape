//! Note and callout leaders (live drw2 10-21 g "Note leaders: grips, nodes,
//! Add leader"; (6): a callout's leaders work the same): their paths on the
//! sheet, what a point hits on them, and the edits of their menus (Add node,
//! Remove node, Add leader from a node, Remove leaders, Delete). Shared by
//! the editor and the agent API.

use super::jog::seg_dist;
use noshape_model::drawing::build::BuiltSheet;
use noshape_model::drawing::sheet::{LeaderPath, note_leader_paths};
use noshape_model::drawing::{Annotation, AnnotationKind, Leader, LeaderStart, note, remove_leader, remove_leader_node};

/// What a sheet point hits on a note's or callout's leaders (`leader`: the
/// note's leader index, a callout's `CalloutStyle::all_leaders` index).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LeaderHit {
    /// Leader `leader`'s line, segment `seg` of its path, at `p`.
    Stroke { id: u64, leader: usize, seg: usize, p: [f64; 2] },
    Node { id: u64, leader: usize, node: usize },
    /// The end of a leader's landing (leaders from the note's side).
    Bend { id: u64, leader: usize },
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// A note's unrotated box (sheet mm).
fn note_bbox(a: &Annotation) -> Option<[f64; 4]> {
    let AnnotationKind::Note { text, height_mm, style } = &a.kind else { return None };
    Some(note::layout(&note::fractions(text), *height_mm, style, a.position).bbox)
}

/// A note's or callout's leader paths on the sheet, by leader index (a
/// callout's as last drawn in `built`).
pub fn leader_paths(built: Option<&BuiltSheet>, a: &Annotation) -> Vec<(usize, LeaderPath)> {
    match &a.kind {
        AnnotationKind::Note { style, .. } => {
            let Some(bbox) = note_bbox(a) else { return Vec::new() };
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

/// The segment of `path` nearest `p` and its distance.
pub fn nearest_segment(path: &LeaderPath, p: [f64; 2]) -> Option<(usize, f64)> {
    path.points.windows(2).map(|w| seg_dist(p, w[0], w[1])).enumerate().min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Edit the leaders of note or callout `a` as one list (a callout's
/// `CalloutStyle::all_leaders`). False for other annotations.
pub fn with_leaders(a: &mut Annotation, f: impl FnOnce(&mut Vec<Leader>)) -> bool {
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

/// The leaders of note or callout `a` (a callout's `all_leaders`).
pub fn leaders_of(a: &Annotation) -> Option<Vec<Leader>> {
    match &a.kind {
        AnnotationKind::Note { style, .. } => Some(style.leaders.clone()),
        AnnotationKind::Balloon { view, anchor, callout, .. } => Some(callout.all_leaders(*view, *anchor)),
        _ => None,
    }
}

/// Add node: a node at `p` on segment `seg` of leader `leader`'s path,
/// whose nodes start at path point `head`.
pub fn add_node(a: &mut Annotation, leader: usize, seg: usize, head: usize, p: [f64; 2]) -> bool {
    with_leaders(a, |l| {
        if let Some(ld) = l.get_mut(leader) {
            // Segment `seg` runs from path point `seg` to `seg + 1`.
            let at = (seg + 1).saturating_sub(head).min(ld.nodes.len());
            ld.nodes.insert(at, p);
        }
    })
}

/// Remove node `node` of leader `leader` (its branches start at the bend).
pub fn remove_node(a: &mut Annotation, leader: usize, node: usize) -> bool {
    with_leaders(a, |l| remove_leader_node(l, leader, node))
}

/// Remove leaders: a note's all, a callout's added ones. False for other
/// annotations.
pub fn remove_leaders(a: &mut Annotation) -> bool {
    match &mut a.kind {
        AnnotationKind::Note { style, .. } => style.leaders.clear(),
        AnnotationKind::Balloon { callout, .. } => callout.leaders.clear(),
        _ => return false,
    }
    true
}

/// Delete on a leader: that leader (a callout's leader to the anchor
/// leaves it in space).
pub fn delete_leader(a: &mut Annotation, leader: usize) -> bool {
    match &mut a.kind {
        AnnotationKind::Balloon { view, anchor, callout, .. } => {
            callout.remove_leader(leader, *view, *anchor);
            true
        }
        _ => with_leaders(a, |l| remove_leader(l, leader)),
    }
}

/// Drag a leader's anchor grip (help note: off an edge detaches it, onto
/// one re-attaches it): its end goes to `point` in `view` (view
/// coordinates), or to sheet point `point` when `view` is None. A
/// callout's leader to the anchor moves only onto a model point (`r`: its
/// BOM item follows it). False when nothing moved.
pub fn move_leader_end(a: &mut Annotation, leader: usize, view: Option<noshape_model::drawing::ViewId>, point: [f64; 2], r: Option<noshape_model::drawing::PointRef>) -> bool {
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
    with_leaders(a, |l| {
        if let Some(ld) = l.get_mut(leader) {
            ld.view = view;
            ld.point = point;
            moved = true;
        }
    });
    moved
}

/// Add leader: `leader` (with its `start`) to annotation `a`. Notes and
/// callouts keep the start; GD&T, weld and surface finish leaders start at
/// the symbol. False for annotations without leaders.
pub fn add_leader(a: &mut Annotation, leader: Leader) -> bool {
    let plain = Leader { start: LeaderStart::Side, ..leader.clone() };
    match &mut a.kind {
        AnnotationKind::Note { style, .. } => style.leaders.push(leader),
        AnnotationKind::Balloon { callout, .. } => callout.leaders.push(leader),
        AnnotationKind::Gdt { leaders, .. } | AnnotationKind::SurfaceFinish { leaders, .. } => leaders.push(plain),
        AnnotationKind::Weld { spec, .. } => spec.leaders.push(plain),
        _ => return false,
    }
    true
}

/// Where the note's own Add leader starts (live (5)): the existing
/// landing's bend, else the note's side.
pub fn note_leader_start(a: &Annotation) -> LeaderStart {
    match &a.kind {
        AnnotationKind::Note { style, .. } => style.leaders.iter().position(|l| l.start.is_side()).map_or(LeaderStart::Side, LeaderStart::Bend),
        _ => LeaderStart::Side,
    }
}

/// Where a branch of `a` from `from` starts (sheet mm).
pub fn branch_point(built: Option<&BuiltSheet>, a: &Annotation, from: LeaderStart) -> Option<[f64; 2]> {
    let leaders = leaders_of(a)?;
    match from {
        LeaderStart::Bend(j) => leader_paths(built, a).into_iter().find(|(i, _)| *i == j).map(|(_, p)| p.bend()),
        LeaderStart::Node(j, n) => leaders.get(j).and_then(|l| l.nodes.get(n)).copied(),
        LeaderStart::Side => None,
    }
}
