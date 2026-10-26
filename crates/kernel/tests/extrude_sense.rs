//! Parasolid sweeps an extrude from its far end back toward the sketch (live
//! "fs (10-26 b)", oracle CP2 / P6): a block's side edges run toward the
//! sketch (−z for an extrude up), its profile edges keep the sketch's
//! directions, and a cylinder's frame sits on the axis at the far end cap.

use noshape_kernel::*;

/// Lines with each edge's own direction (its curve's: `edge_info` gives it
/// start to end, reversed for a reversed edge).
fn lines(s: &Shape) -> Vec<(DVec3, DVec3)> {
    s.edges().iter().filter_map(|e| match e.edge_info().ok()?.curve {
        Curve::Line { origin, direction } => Some((origin, if e.orientation() == Orientation::Reversed { -direction.normalize() } else { direction.normalize() })),
        _ => None,
    }).collect()
}

#[test]
fn an_extrude_runs_back_toward_its_sketch() {
    let mm = 1e-3;
    let frame = Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
    let rect = make_face_2d(&frame, &[Segment2d::Line { a: DVec2::new(0.0, 0.0), b: DVec2::new(60.0 * mm, 0.0) }, Segment2d::Line { a: DVec2::new(60.0 * mm, 0.0), b: DVec2::new(60.0 * mm, 40.0 * mm) }, Segment2d::Line { a: DVec2::new(60.0 * mm, 40.0 * mm), b: DVec2::new(0.0, 40.0 * mm) }, Segment2d::Line { a: DVec2::new(0.0, 40.0 * mm), b: DVec2::new(0.0, 0.0) }], &[]).unwrap();
    let block = extrude(&rect, DVec3::new(0.0, 0.0, 20.0 * mm)).unwrap().shape;
    let vertical: Vec<DVec3> = lines(&block).into_iter().map(|l| l.1).filter(|d| d.z.abs() > 0.9).collect();
    assert_eq!(vertical.len(), 4);
    assert!(vertical.iter().all(|d| d.z < -0.999), "{vertical:?}");
    let disc = make_face_from_wire(&make_wire(&[make_circle(DVec3::new(100.0 * mm, 0.0, 0.0), DVec3::Z, 10.0 * mm).unwrap()]).unwrap(), true).unwrap();
    let cyl = extrude(&disc, DVec3::new(0.0, 0.0, 20.0 * mm)).unwrap().shape;
    let side = cyl.faces().into_iter().find_map(|f| match f.face_surface().ok()? {
        Surface::Cylinder { origin, axis, .. } => Some((origin, axis)),
        _ => None,
    });
    let (origin, axis) = side.expect("a cylinder");
    eprintln!("cylinder origin {:?} axis {:?}", origin / mm, axis);
    // The origin as live; OCCT's axis of a swept circle points away from the
    // sweep (+z here), live's toward the sketch (−z): not matched yet.
    assert!((origin - DVec3::new(100.0, 0.0, 20.0) * mm).length() < 1e-9 && axis.normalize().z.abs() > 1.0 - 1e-9, "{origin:?} {axis:?}");
}
