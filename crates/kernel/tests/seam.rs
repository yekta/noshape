//! Onshape's topology has no seam edges, no degenerated edges and no
//! vertices on full circles (`Shape::shown_topology`; live, lead 10-25 SEAM:
//! cylinder 3 faces / 2 circles / 0 vertices, sphere 1 / 0 / 0, ring
//! r10..15 h20 4 / 4 / 0, cylinder with a 5 mm chamfer 4 / 3 / 0).

use noshape_kernel::*;
use std::f64::consts::PI;

fn counts(s: &Shape) -> (usize, usize, usize) {
    let t = s.shown_topology().unwrap();
    (s.count(ShapeType::Face), t.edges.iter().filter(|&&e| e).count(), t.vertices.iter().filter(|&&v| v).count())
}

#[test]
fn seams_poles_and_circle_vertices_are_not_topology() {
    let frame = Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
    let cylinder = make_cylinder(&frame, 25.0, 40.0, 2.0 * PI).unwrap();
    assert_eq!(counts(&cylinder), (3, 2, 0));
    assert_eq!(counts(&make_sphere(&frame, 10.0, 2.0 * PI).unwrap()), (1, 0, 0));
    let ring = boolean(BooleanOp::Cut, &[make_cylinder(&frame, 15.0, 20.0, 2.0 * PI).unwrap()], &[make_cylinder(&frame, 10.0, 20.0, 2.0 * PI).unwrap()], &BooleanOptions::default()).unwrap().shape;
    assert_eq!(counts(&ring), (4, 4, 0));
    let top: Vec<Shape> = cylinder.edges().into_iter().filter(|e| e.bounding_box(false).unwrap().min.z > 39.9 && e.edge_info().unwrap().closed).collect();
    assert_eq!(top.len(), 1);
    let chamfered = chamfer(&cylinder, &top, ChamferSpec::Equal(5.0), &[]).unwrap().shape;
    assert_eq!(counts(&chamfered), (4, 3, 0));
    // A box keeps all of its 12 edges and 8 vertices; a cone's apex is a vertex.
    assert_eq!(counts(&make_box(&frame, 10.0, 20.0, 30.0).unwrap()), (6, 12, 8));
    assert_eq!(counts(&make_cone(&frame, 10.0, 0.0, 20.0, 2.0 * PI).unwrap()), (2, 1, 1));
}
