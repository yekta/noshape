//! Parasolid sweeps an extrude from its far end back toward the sketch (live
//! "fs (10-26 b)", oracle CP2 / P6): a block's side edges run toward the
//! sketch (−z for an extrude up), its profile edges keep the sketch's
//! directions, and a cylinder's frame sits on the axis at the far end cap,
//! z back toward the sketch.

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
    assert!((origin - DVec3::new(100.0, 0.0, 20.0) * mm).length() < 1e-9 && axis.normalize().z < -1.0 + 1e-9, "{origin:?} {axis:?}");
}

/// The cylinder of an extruded circle as live's evSurfaceDefinition ("fs
/// (10-26 b)" (a), (b), (e)): origin on the axis at the far end, z back
/// toward the sketch, x gp_Ax2's default for that z. The part stays valid
/// and takes a boolean across the re-framed face.
#[test]
fn an_extruded_cylinder_is_framed_toward_its_sketch() {
    let mm = 1e-3;
    let cylinder = |center: DVec3, normal: DVec3, v: DVec3| {
        let disc = make_face_from_wire(&make_wire(&[make_circle(center * mm, normal, 10.0 * mm).unwrap()]).unwrap(), true).unwrap();
        let s = extrude(&disc, v * mm).unwrap().shape;
        let frame = s.faces().into_iter().find_map(|f| match f.face_surface().ok()? {
            Surface::Cylinder { origin, axis, x_dir, .. } => Some((origin / mm, axis, x_dir)),
            _ => None,
        });
        (s, frame.expect("a cylinder"))
    };
    let near = |a: DVec3, b: DVec3| (a - b).length() < 1e-6;
    // (a) Top, 20 up; (b) the same down; (e) on Front (normal −y), 30 along −y.
    for (center, normal, v, origin, z, x) in [
        (DVec3::new(100.0, 0.0, 0.0), DVec3::Z, DVec3::new(0.0, 0.0, 20.0), DVec3::new(100.0, 0.0, 20.0), -DVec3::Z, -DVec3::X),
        (DVec3::new(150.0, 0.0, 0.0), DVec3::Z, DVec3::new(0.0, 0.0, -20.0), DVec3::new(150.0, 0.0, -20.0), DVec3::Z, DVec3::X),
        (DVec3::new(300.0, 0.0, 50.0), -DVec3::Y, DVec3::new(0.0, -30.0, 0.0), DVec3::new(300.0, -30.0, 50.0), DVec3::Y, DVec3::Z),
    ] {
        let (s, (o, a, xd)) = cylinder(center, normal, v);
        assert!(near(o, origin) && near(a, z) && near(xd, x), "{center:?}: origin {o:?} z {a:?} x {xd:?}");
        assert!(s.is_valid());
        let want = std::f64::consts::PI * 100.0 * v.length();
        assert!((s.volume().unwrap() / mm.powi(3) - want).abs() < 1e-6 * want);
        // A box through the side face: the cut part is valid with the right volume.
        let a0 = center * mm - DVec3::splat(3.0 * mm) + v * (0.5 * mm);
        let b = make_box_corners(a0, a0 + DVec3::splat(30.0 * mm)).unwrap();
        let r = cut(&s, &b).unwrap().shape;
        assert!(r.is_valid(), "{center:?}: cut invalid");
        let inter = common(&s, &b).unwrap().shape;
        assert!((r.volume().unwrap() + inter.volume().unwrap() - s.volume().unwrap()).abs() < 1e-9 * s.volume().unwrap());
    }
    // The seam at the new frame's u = 0, as OCCT's primitives: a disc with
    // a hub, fused, meets its mirror images exactly (with the seam left at
    // u = pi the common lost 80 % of the part: the demo Flange's symmetry).
    let disc = |r: f64, z0: f64, h: f64| extrude(&make_face_from_wire(&make_wire(&[make_circle(DVec3::new(0.0, 0.0, z0) * mm, DVec3::Z, r * mm).unwrap()]).unwrap(), true).unwrap(), DVec3::new(0.0, 0.0, h) * mm).unwrap().shape;
    let part = fuse(&disc(40.0, 0.0, 8.0), &disc(18.0, 8.0, 14.0)).unwrap().shape;
    let v = part.volume().unwrap();
    for n in [DVec3::X, DVec3::Y] {
        let m = part.transformed(&Transform::Mirror(Plane::from_normal(DVec3::ZERO, n))).unwrap();
        let c = common(&part, &m).unwrap().shape.volume().unwrap();
        assert!((c / v - 1.0).abs() < 1e-9, "mirror {n:?}: {c} of {v}");
    }
}
