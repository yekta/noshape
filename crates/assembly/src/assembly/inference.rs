//! Which inference point the pointer brings a mate connector to, shared by the assembly's mate dialogs and the Part
//! Studio's Mate connector (live mc-infer-1122: the same looks and reach in both). The points are
//! [`super::eval::inference_candidates`]; each view projects them to the screen itself.

use crate::reference::{Inference, Reference};
use glam::DVec2;

/// Over a face, how near (px, in x and in y: a square) the pointer brings the connector to a corner or another point.
pub const FACE_REACH_PX: f64 = 47.5;
/// Over a face, how near (px) the pointer brings it to an edge's midpoint.
pub const MIDPOINT_REACH_PX: f64 = 41.5;
/// Over an edge, how near (px) the pointer brings it to an end; else it sits at the midpoint (a circle's centre).
pub const EDGE_END_REACH_PX: f64 = 15.0;

/// The hovered entity's inference points as the chooser sees them: each point's reference and where it lies on the
/// screen (None: behind the eye).
pub struct OnScreen<'a> {
    pub on: &'a Reference,
    pub points: Vec<(&'a Reference, Option<DVec2>)>,
}

/// The chosen point's index, as live (asm/feel-1117 reach-corner-strip, asm-feel3-1118, mc-infer-1122): over a planar
/// face a point within the square around the pointer, else the face's centroid (a cylinder's side at its axis
/// midpoint at any height); over an edge its midpoint (a circle's centre) unless an end is within 15 px. `entered`:
/// the pointer has just come onto the entity, so the face's centroid (its points from the next move on: live2
/// place-1118 21:05). Shift locks the entity, not the point: the same rule wherever the pointer is (mc-infer-1122
/// shift.png). Other entities (a vertex): the nearest point.
pub fn choose(s: &OnScreen, pointer: DVec2, entered: bool) -> Option<usize> {
    let near = s.points.iter().enumerate().filter_map(|(i, (r, at))| Some((i, *r, (*at)? - pointer)));
    let primary = s.points.first().is_some_and(|(r, _)| is_primary(r));
    if !primary || !matches!(s.on, Reference::Face(_) | Reference::Edge(_)) {
        return near.min_by(|a, b| a.2.length().total_cmp(&b.2.length())).map(|(i, ..)| i);
    }
    let best = match s.on {
        Reference::Face(_) if entered => None,
        Reference::Face(_) => near.filter(|(_, r, d)| d.x.abs().max(d.y.abs()) <= face_reach(r)).min_by(|a, b| a.2.length().total_cmp(&b.2.length())),
        _ => near.filter(|(.., d)| d.length() <= EDGE_END_REACH_PX).min_by(|a, b| a.2.length().total_cmp(&b.2.length())),
    };
    Some(best.map_or(0, |(i, ..)| i))
}

/// Whether a hovered vertex shows its connector glyph: reached from a face or an edge (or another vertex reached so)
/// it does, straight from empty space it doesn't (live mc-infer-1122 reach-corner d=6..0, timing.json corner rows).
/// `before`: the entity hovered last and whether it showed the glyph.
pub fn vertex_glyph(now: &Reference, before: Option<(&Reference, bool)>) -> bool {
    matches!(now, Reference::Vertex(_)) && before.is_some_and(|(r, glyph)| !matches!(r, Reference::Vertex(_)) || glyph)
}

/// The entity's own first point: a face's centroid (a cylinder side's axis midpoint), an edge's midpoint (a circle's
/// centre).
fn is_primary(r: &Reference) -> bool {
    match r {
        Reference::Implicit { inference: Inference::Centroid | Inference::MidAxis, on } => matches!(**on, Reference::Face(_)),
        Reference::Implicit { inference: Inference::Midpoint | Inference::CenterRim, on } => matches!(**on, Reference::Edge(_)),
        _ => false,
    }
}

fn face_reach(r: &Reference) -> f64 {
    match r {
        Reference::Implicit { inference: Inference::Midpoint, .. } => MIDPOINT_REACH_PX,
        _ => FACE_REACH_PX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::reference::{FeatureId, Role, TopoName};
    use std::sync::Arc;

    fn implicit(on: &Reference, inference: Inference) -> Reference {
        Reference::Implicit { on: Box::new(on.clone()), inference }
    }

    fn face() -> TopoName {
        TopoName::Face { feature: FeatureId(1), role: Role::CapEnd(None) }
    }

    fn edge() -> TopoName {
        TopoName::Edge { faces: [Arc::new(face()), Arc::new(face())], ordinal: 0 }
    }

    /// A face's centroid at the origin, a corner 67 px off and an edge midpoint 40 px off (the Part Studio's view in
    /// mc-infer-1122); the centroid competes as one of the points.
    #[test]
    fn a_face_takes_a_point_in_its_square_else_its_centroid() {
        let (face, edge) = (Reference::Face(face()), Reference::Edge(edge()));
        let (c, k, m) = (implicit(&face, Inference::Centroid), implicit(&face, Inference::Vertex), implicit(&edge, Inference::Midpoint));
        let s = OnScreen { on: &face, points: vec![(&c, Some(DVec2::ZERO)), (&k, Some(DVec2::new(60.0, 30.0))), (&m, Some(DVec2::new(0.0, 40.0)))] };
        assert_eq!(choose(&s, DVec2::new(10.0, 5.0), false), Some(0), "far from both: the centroid");
        assert_eq!(choose(&s, DVec2::new(60.0, 30.0), true), Some(0), "just entered: the centroid");
        assert_eq!(choose(&s, DVec2::new(40.0, 20.0), false), Some(1), "the corner within 47.5 px in x and y");
        assert_eq!(choose(&s, DVec2::new(5.0, 10.0), false), Some(0), "the midpoint 30 px off, the centroid nearer");
        assert_eq!(choose(&s, DVec2::new(0.0, 30.0), false), Some(2), "the midpoint 10 px off, the centroid 30");
    }

    #[test]
    fn an_edge_takes_an_end_within_15_px_else_its_midpoint() {
        let edge = Reference::Edge(edge());
        let (m, e) = (implicit(&edge, Inference::Midpoint), implicit(&edge, Inference::Vertex));
        let s = OnScreen { on: &edge, points: vec![(&m, Some(DVec2::ZERO)), (&e, Some(DVec2::new(50.0, 0.0)))] };
        assert_eq!(choose(&s, DVec2::new(34.0, 0.0), false), Some(0), "16 px from the end: the midpoint");
        assert_eq!(choose(&s, DVec2::new(36.0, 0.0), false), Some(1), "14 px from the end: the end");
    }

    #[test]
    fn a_vertex_shows_its_glyph_only_when_reached_from_a_face_or_edge() {
        let (v, f) = (Reference::Vertex(TopoName::Vertex { faces: Arc::from([Arc::new(face())]), ordinal: 0 }), Reference::Face(face()));
        assert!(!vertex_glyph(&v, None), "from empty space");
        assert!(vertex_glyph(&v, Some((&f, false))), "from its face");
        assert!(vertex_glyph(&v, Some((&v, true))), "from a vertex reached from a face");
        assert!(!vertex_glyph(&v, Some((&v, false))), "from a vertex reached from empty space");
        assert!(!vertex_glyph(&f, Some((&f, false))), "not a vertex");
    }
}
