//! Which inference point the pointer brings a mate connector to, shared by the assembly's mate dialogs and the Part
//! Studio's Mate connector (live mc-infer-1122: the same looks and reach in both). The points are
//! [`super::eval::inference_candidates`]; each view projects them to the screen itself.

use crate::reference::{Inference, Reference};
use glam::DVec2;

/// Over a face, how near (px, in x and in y: a square) the pointer brings the connector to a marker, whatever the zoom
/// (live's COORDINATESYSTEM_INFERENCE_PICK_RADIUS_PX; mc-reach-1122 R1-R3: a corner swept along the screen's x from
/// 50 px, an edge midpoint swept at 30 degrees from 58 px, 50 px in x).
pub const FACE_REACH_PX: f64 = 50.0;

/// The hovered entity's inference points as the chooser sees them: each point's reference and where it lies on the
/// screen (None: behind the eye).
pub struct OnScreen<'a> {
    pub on: &'a Reference,
    pub points: Vec<(&'a Reference, Option<DVec2>)>,
}

/// The chosen point's index, as live (asm/feel-1117 reach-corner-strip, asm-feel3-1118, mc-infer-1122, mc-reach-1122):
/// over a planar face the nearest point (the centroid too) of those within the square around the pointer, else the
/// face's centroid (a cylinder's side at its axis
/// midpoint at any height); over a straight edge its midpoint or the nearer of its ends, half-way between (live
/// mc-reach-1124's 1.8 s steps; mc-infer-1122's d=10 midpoint was its laggy sweep), over an arc its centre. `entered`:
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
        Reference::Face(_) if !entered => near.filter(|(.., d)| d.x.abs().max(d.y.abs()) <= FACE_REACH_PX).min_by(|a, b| a.2.length().total_cmp(&b.2.length())),
        // Along an edge its midpoint or the nearer end, switching half-way (live mc-reach-1124: the corner end, Z along
        // the edge, from 27 px of a 56 px half).
        // An arc: its centre wherever along it (qa FEEL-MC-ARC-PICK-VERTEX-ASM: live's hinge knuckle closes on the
        // centre picked 4 mm from the arc's end); its ends from the vertices themselves.
        Reference::Edge(_) if matches!(s.points.first(), Some((Reference::Implicit { inference: Inference::CenterRim, .. }, _))) => None,
        Reference::Edge(_) => near.filter(|(i, r, _)| *i == 0 || matches!(r, Reference::Implicit { inference: Inference::Vertex, .. })).min_by(|a, b| a.2.length().total_cmp(&b.2.length())),
        _ => None,
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
    /// mc-infer-1122); the centroid competes as one of the points. Zoomed in (mc-reach-1122 R1-R3) a marker takes the
    /// connector within 50 px in x and y however far the centroid is.
    #[test]
    fn a_face_takes_a_point_in_its_square_else_its_centroid() {
        let (face, edge) = (Reference::Face(face()), Reference::Edge(edge()));
        let (c, k, m) = (implicit(&face, Inference::Centroid), implicit(&face, Inference::Vertex), implicit(&edge, Inference::Midpoint));
        let s = OnScreen { on: &face, points: vec![(&c, Some(DVec2::ZERO)), (&k, Some(DVec2::new(60.0, 30.0))), (&m, Some(DVec2::new(0.0, 40.0)))] };
        assert_eq!(choose(&s, DVec2::new(10.0, 5.0), false), Some(0), "far from both: the centroid");
        assert_eq!(choose(&s, DVec2::new(60.0, 30.0), true), Some(0), "just entered: the centroid");
        assert_eq!(choose(&s, DVec2::new(40.0, 20.0), false), Some(1), "the corner within 50 px in x and y");
        assert_eq!(choose(&s, DVec2::new(5.0, 10.0), false), Some(0), "the midpoint 30 px off, the centroid nearer");
        assert_eq!(choose(&s, DVec2::new(0.0, 30.0), false), Some(2), "the midpoint 10 px off, the centroid 30");
        let zoomed = OnScreen { on: &face, points: vec![(&c, Some(DVec2::ZERO)), (&k, Some(DVec2::new(245.0, 0.0))), (&m, Some(DVec2::new(-212.0, 122.5)))] };
        assert_eq!(choose(&zoomed, DVec2::new(195.0, 0.0), false), Some(1), "4x: the corner 50 px off");
        assert_eq!(choose(&zoomed, DVec2::new(193.0, 0.0), false), Some(0), "4x: the corner 52 px off: the centroid");
        let toward_mid = |d: f64| DVec2::new(-212.0, 122.5) * (1.0 - d / 245.0);
        assert_eq!(choose(&zoomed, toward_mid(56.0), false), Some(2), "4x: the midpoint 56 px off at 30 degrees");
        assert_eq!(choose(&zoomed, toward_mid(60.0), false), Some(0), "4x: the midpoint 60 px off: the centroid");
    }

    #[test]
    fn an_edge_takes_its_nearer_end_past_half_way() {
        let edge = Reference::Edge(edge());
        let (m, e) = (implicit(&edge, Inference::Midpoint), implicit(&edge, Inference::Vertex));
        // Live mc-reach-1124: the midpoint to 26 px, the corner end from 27 px of the 56.2 px half.
        let s = OnScreen { on: &edge, points: vec![(&m, Some(DVec2::ZERO)), (&e, Some(DVec2::new(56.2, 0.0)))] };
        assert_eq!(choose(&s, DVec2::new(26.0, 0.0), false), Some(0), "26 px along: the midpoint");
        assert_eq!(choose(&s, DVec2::new(29.0, 0.0), false), Some(1), "29 px along: the corner end");
        assert_eq!(choose(&s, DVec2::new(46.0, 0.0), false), Some(1), "10 px from the corner: its end");
    }

    /// qa FEEL-MC-ARC-PICK-VERTEX-ASM (hinge knuckle, a 270 degree arc): live takes the arc's centre even 4 mm from its
    /// end; the end vertex only from the vertex itself.
    #[test]
    fn an_arc_takes_its_centre_however_near_its_end() {
        let edge = Reference::Edge(edge());
        let (c, m, e) = (implicit(&edge, Inference::CenterRim), implicit(&edge, Inference::Midpoint), implicit(&edge, Inference::Vertex));
        let s = OnScreen { on: &edge, points: vec![(&c, Some(DVec2::ZERO)), (&m, Some(DVec2::new(-70.0, -70.0))), (&e, Some(DVec2::new(100.0, 0.0)))] };
        assert_eq!(choose(&s, DVec2::new(100.0, 8.0), false), Some(0), "8 px from the end: the centre");
        assert_eq!(choose(&s, DVec2::new(-70.0, -70.0), false), Some(0), "over its midpoint: the centre");
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
