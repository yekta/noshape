//! Onshape's topology has no seam edges, no degenerated edges and no
//! vertices on full circles (`Shape::shown_topology`; live, lead 10-25 SEAM:
//! cylinder 3 faces / 2 circles / 0 vertices, sphere 1 / 0 / 0, ring
//! r10..15 h20 4 / 4 / 0, cylinder with a 5 mm chamfer 4 / 3 / 0).

use noshape_kernel::*;
use std::f64::consts::PI;

fn counts(s: &Shape) -> (usize, usize, usize) {
    let t = s.shown_topology().unwrap();
    (s.count(ShapeType::Face), t.edge_count(), t.vertex_count())
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

/// A notch cut into a torus (R 20, r 5) by a box beyond x = 22, |y| <= 3: its four section curves each cross one of the
/// torus's seams, where OCCT approximates each side as a B-spline of its own. Parasolid's torus has no seam: live's
/// notch is 4 faces, 6 edges, 4 vertices (c2-rings-loft's slots through its rounds), on both Booleans.
#[test]
fn a_section_across_a_tori_seam_is_one_edge() {
    let frame = Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
    let torus = make_torus(&frame, 20.0, 5.0, 2.0 * PI).unwrap();
    let notch = make_box_corners(DVec3::new(22.0, -3.0, -10.0), DVec3::new(30.0, 3.0, 10.0)).unwrap();
    for rust in [true, false] {
        set_rust_boolean(Some(rust));
        let r = unify_same_domain(&boolean(BooleanOp::Cut, std::slice::from_ref(&torus), std::slice::from_ref(&notch), &BooleanOptions::default()).unwrap().shape).unwrap().shape;
        set_rust_boolean(None);
        assert!(r.is_valid());
        assert_eq!(counts(&r), (4, 6, 4), "rust {rust}");
    }
}

/// bushing2-0-joshua's Extrude 4: a tube (Ø75 / Ø50 along y) with notches at one end and two windows through its
/// wall. Each cylinder's seam splits its notched end's arc (live has no seam: 18 faces, 40 edges, 24 vertices). The
/// bore's seam moves to a notch corner though a window lies nearer its end circle (a hole's loop isn't the loop round
/// the axis); the outer wall's every corner lines up with a window, so its seam stays, its arc in two pieces shown as
/// one edge with no vertex between (live 2069): live's 18 / 40 / 24 either way.
#[test]
fn a_seam_moves_to_the_loop_round_the_axis_past_a_window() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/seam/bushing-extrude4.brep");
    let s = read_brep(&path).unwrap();
    assert_eq!(counts(&s), (18, 40, 24));
    let u = unify_same_domain_whole(&s).unwrap().shape;
    assert!(u.is_valid());
    assert!((u.volume().unwrap() - s.volume().unwrap()).abs() < 1e-9 * s.volume().unwrap());
    assert_eq!(counts(&u), (18, 40, 24));
}

/// A face pattern of a hole through a tube's wall, stitched (Parasolid's face pattern copies the faces: motor-mount-jack's
/// slots, 5 ms in live): the copies' rims holes of the tube's two cylinders, their seams moved clear of every hole first,
/// the walls copied. The same part as cutting all four holes.
#[test]
fn a_face_pattern_round_a_tube_is_stitched_as_the_cut() {
    let frame = Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
    let tube = boolean(BooleanOp::Cut, &[make_cylinder(&frame, 15.0, 40.0, 2.0 * PI).unwrap()], &[make_cylinder(&frame, 12.0, 40.0, 2.0 * PI).unwrap()], &BooleanOptions::default()).unwrap().shape;
    let hole = |a: f64| make_cylinder(&Plane::new(DVec3::new(0.0, 0.0, 20.0), DVec3::new(-a.sin(), a.cos(), 0.0), DVec3::Z), 3.0, 20.0, 2.0 * PI).unwrap();
    let seed = boolean(BooleanOp::Cut, std::slice::from_ref(&tube), &[hole(0.0)], &BooleanOptions::default()).unwrap().shape;
    let walls: Vec<Shape> = seed.faces().into_iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { radius, .. }) if (radius - 3.0).abs() < 1e-9)).collect();
    assert_eq!(walls.len(), 1);
    let turns: Vec<Transform> = (1..4).map(|i| Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::Z, angle: i as f64 * PI / 2.0 }).collect();
    let (stitched, codes) = stitch_face_copies(&seed, &walls, &turns).unwrap();
    assert!(stitched.is_valid());
    assert_eq!(codes.len(), stitched.count(ShapeType::Face));
    assert_eq!(codes.iter().filter(|&&c| c < 0).count(), 3, "a wall a copy");
    let all: Vec<Shape> = (0..4).map(|i| hole(i as f64 * PI / 2.0)).collect();
    let cut = boolean(BooleanOp::Cut, &[tube], &all, &BooleanOptions::default()).unwrap().shape;
    let v = |s: &Shape| s.volume().unwrap();
    assert!((v(&stitched) - v(&cut)).abs() < 1e-9 * v(&cut), "{} vs {}", v(&stitched), v(&cut));
    // (Live's: each rim one closed edge; the plain cut leaves rims split at the seams, 8 / 14 / 4.)
    assert_eq!(counts(&stitched), (8, 12, 0));
}

/// An elliptic pocket cut into a cylinder across its seam: the pocket's outline on the side is one ring (Parasolid has
/// no seam to split it), not two halves and two vertices.
#[test]
fn a_pockets_outline_across_the_seam_is_one_ring() {
    let mm = 1e-3;
    let cylinder = make_cylinder(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 10.0 * mm, 40.0 * mm, 2.0 * PI).unwrap();
    // Through the +x wall, where the cylinder's seam runs.
    let plane = Plane::new(DVec3::new(5.0 * mm, 0.0, 20.0 * mm), DVec3::X, DVec3::Y);
    let slot = make_face_2d(&plane, &[Segment2d::Ellipse { center: glam::DVec2::ZERO, major_radius: 12.0 * mm, minor_radius: 4.0 * mm, rotation: PI / 2.0 }], &[]).unwrap();
    let cutter = extrude(&slot, DVec3::new(10.0 * mm, 0.0, 0.0)).unwrap().shape;
    let part = cut(&cylinder, &cutter).unwrap().shape;
    // The side, the two ends, the pocket's wall and floor; the end circles, the outline on the side and the floor's ellipse.
    assert_eq!(counts(&part), (5, 4, 0));
}
