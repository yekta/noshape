//! Wrap / unwrap, isocline curves and surface fitting.

use glam::{DVec2, DVec3};
use noshape_kernel::*;
use std::f64::consts::PI;

fn area(s: &Shape) -> f64 {
    s.faces().iter().map(|f| f.area().unwrap()).sum()
}

/// 40 x 20 rectangle with a centered circular hole of radius 5 on the plane
/// z = 10 (tangent to a cylinder of radius 10 around the X axis).
fn holed_plate() -> (Shape, Plane) {
    let plane = Plane::new(DVec3::new(0.0, 0.0, 10.0), DVec3::Z, DVec3::X);
    let outer = vec![
        Segment2d::Line { a: DVec2::new(-20.0, -10.0), b: DVec2::new(20.0, -10.0) },
        Segment2d::Line { a: DVec2::new(20.0, -10.0), b: DVec2::new(20.0, 10.0) },
        Segment2d::Line { a: DVec2::new(20.0, 10.0), b: DVec2::new(-20.0, 10.0) },
        Segment2d::Line { a: DVec2::new(-20.0, 10.0), b: DVec2::new(-20.0, -10.0) },
    ];
    let hole = vec![Segment2d::Circle { center: DVec2::ZERO, radius: 5.0 }];
    (make_face_2d(&plane, &outer, &[hole]).unwrap(), plane)
}

fn cylinder_x() -> Developable {
    Developable::Cylinder { origin: DVec3::ZERO, axis: DVec3::X, x_dir: DVec3::Z, radius: 10.0 }
}

#[test]
fn wrap_plate_onto_cylinder_preserves_area() {
    let (face, plane) = holed_plate();
    let src = Developable::Plane { origin: plane.origin, x_dir: DVec3::X, normal: DVec3::Z };
    let dst = cylinder_x();
    // Tangent line (x axis at z = 10) stays in place: source (x, y) -> flat (R*phi, h) = (y?, x).
    let a_s = src.flat(DVec3::new(0.0, 0.0, 10.0), 0.0);
    let a_d = dst.flat(DVec3::new(0.0, 0.0, 10.0), 0.0);
    // source X (along the axis) -> cylinder axial (flat y)
    let (sx, _, sn) = src.flat_axes(a_s, 0.0);
    let (dx, dy, dn) = dst.flat_axes(a_d, 0.0);
    assert!(sx.dot(DVec3::X) > 0.99 && dy.dot(DVec3::X) > 0.99);
    let mirror = (sn.dot(DVec3::Z) > 0.0) != (dn.dot(DVec3::Z) > 0.0);
    let _ = dx;
    let pl = Placement2d::align(a_s, DVec2::X, a_d, DVec2::Y, mirror);
    let res = wrap_faces(std::slice::from_ref(&face), &src, 0.0, &dst, 0.0, &pl, 1e-7).unwrap();
    let out = res.shape.clone();
    assert_eq!(out.faces().len(), 1);
    let f = &out.faces()[0];
    assert!(matches!(f.face_surface().unwrap(), Surface::Cylinder { .. }));
    let a0 = area(&face);
    let a1 = area(&out);
    assert!((a0 - a1).abs() < 1e-6 * a0, "area {a0} -> {a1}");
    // Every vertex lies on the cylinder; the axial extent is kept.
    let bb = out.bounding_box(true).unwrap();
    assert!((bb.max.x - 20.0).abs() < 1e-6 && (bb.min.x + 20.0).abs() < 1e-6, "{bb:?}");
    for v in out.vertices() {
        let p = v.vertex_point().unwrap();
        assert!(((p.y * p.y + p.z * p.z).sqrt() - 10.0).abs() < 1e-7);
    }
    // History: the source face generated the result.
    assert!(!res.history.generated(0, ShapeType::Face, 0).is_empty());
    // Two wires (outer + hole).
    assert_eq!(f.wires().len(), 2);
    assert!(f.is_valid());

    // Unwrap back onto the original plane: same area, back in place.
    let back = Placement2d::align(a_d, DVec2::Y, a_s, DVec2::X, mirror);
    let res2 = wrap_faces(std::slice::from_ref(f), &dst, 0.0, &src, 0.0, &back, 1e-7).unwrap();
    let a2 = area(&res2.shape);
    assert!((a2 - a0).abs() < 1e-6 * a0, "unwrap area {a2}");
    let bb2 = res2.shape.bounding_box(true).unwrap();
    assert!((bb2.max.y - 10.0).abs() < 1e-6 && (bb2.min.y + 10.0).abs() < 1e-6 && (bb2.max.z - 10.0).abs() < 1e-6, "{bb2:?}");
}

#[test]
fn wrap_onto_cone_and_overlap_error() {
    let (face, plane) = holed_plate();
    let src = Developable::Plane { origin: plane.origin, x_dir: DVec3::X, normal: DVec3::Z };
    // Cone around X, radius 10 at x = 0, half angle 10 degrees.
    let dst = Developable::Cone { origin: DVec3::ZERO, axis: DVec3::X, x_dir: DVec3::Z, radius: 10.0, half_angle: 10f64.to_radians() };
    let a_s = src.flat(DVec3::new(0.0, 0.0, 10.0), 0.0);
    let a_d = dst.flat(DVec3::new(0.0, 0.0, 10.0), 0.0);
    let (gx, gy, _) = dst.flat_axes(a_d, 0.0);
    // Generator direction (axial-ish) in flat coordinates.
    let genr = DVec3::new(10f64.to_radians().cos(), 0.0, 10f64.to_radians().sin());
    let d_axial = DVec2::new(genr.dot(gx), genr.dot(gy));
    let pl = Placement2d::align(a_s, DVec2::X, a_d, d_axial, false);
    let res = wrap_faces(std::slice::from_ref(&face), &src, 0.0, &dst, 0.0, &pl, 1e-7).unwrap();
    let a0 = area(&face);
    let a1 = area(&res.shape);
    assert!((a0 - a1).abs() < 1e-4 * a0, "cone area {a0} -> {a1}");
    assert!(matches!(res.shape.faces()[0].face_surface().unwrap(), Surface::Cone { .. }));

    // A strip longer than the circumference fails cleanly.
    let long = make_plane_face(&plane, -5.0, 5.0, -40.0, 40.0).unwrap();
    let cyl = cylinder_x();
    let pl = Placement2d::align(a_s, DVec2::X, cyl.flat(DVec3::new(0.0, 0.0, 10.0), 0.0), DVec2::Y, false);
    assert!(wrap_faces(&[long], &src, 0.0, &cyl, 0.0, &pl, 1e-7).is_err());
}

#[test]
fn isocline_on_sphere_and_split() {
    let sphere = make_sphere(&Plane::xy(), 10.0, 2.0 * PI).unwrap();
    let face = sphere.faces()[0].clone();
    // normal . Z = sin(30 deg): latitude 30 degrees, a circle at z = 5.
    let curves = isocline_curves(&face, DVec3::Z, 30f64.to_radians(), false).unwrap();
    assert!(!curves.is_empty());
    let total: f64 = curves.iter().map(|e| e.length().unwrap()).sum();
    let expected = 2.0 * PI * 10.0 * 30f64.to_radians().cos();
    assert!((total - expected).abs() < 1e-3 * expected, "isocline length {total} vs {expected}");
    for e in &curves {
        for t in [0.1, 0.5, 0.9] {
            let info = e.edge_info().unwrap();
            let p = e.point_at(info.first_param + (info.last_param - info.first_param) * t).unwrap();
            assert!((p.z - 5.0).abs() < 1e-4, "point {p:?}");
        }
    }
    // Split the face along the curve.
    let ext = isocline_curves(&face, DVec3::Z, 30f64.to_radians(), true).unwrap();
    let split = imprint(&sphere, &ext).unwrap();
    assert!(split.shape.faces().len() >= 2, "split into {} faces", split.shape.faces().len());
    let a: f64 = split.shape.faces().iter().map(|f| f.area().unwrap()).sum();
    assert!((a - 4.0 * PI * 100.0).abs() < 1e-3 * a);
}

#[test]
fn isocline_on_drafted_box_side() {
    // A cylinder side face: normal . Z = 0 everywhere -> angle 0 has no clean
    // curve; angle 10 deg none.
    let cyl = make_cylinder(&Plane::xy(), 5.0, 10.0, 2.0 * PI).unwrap();
    let side = cyl.faces().into_iter().find(|f| matches!(f.face_surface().unwrap(), Surface::Cylinder { .. })).unwrap();
    let c = isocline_curves(&side, DVec3::Z, 10f64.to_radians(), false).unwrap();
    assert!(c.is_empty());
    // Relative to X, the 30 degree isoclines are the two lines at phi = +-60 deg.
    let c = isocline_curves(&side, DVec3::X, 30f64.to_radians(), false).unwrap();
    assert_eq!(c.len(), 2, "two vertical lines");
    for e in &c {
        assert!((e.length().unwrap() - 10.0).abs() < 1e-5);
        let (a, _) = e.endpoints().unwrap();
        assert!((a.x - 2.5).abs() < 1e-5, "{a:?}");
    }
}

#[test]
fn fit_surface_through_points() {
    // z = 0.05 (x^2 - y^2) sampled on a jittered grid.
    let f = |x: f64, y: f64| 0.05 * (x * x - y * y);
    let mut pts = Vec::new();
    for i in 0..7 {
        for j in 0..6 {
            let x = -6.0 + 2.0 * i as f64 + 0.1 * ((i * 7 + j * 3) % 5) as f64;
            let y = -5.0 + 2.0 * j as f64;
            pts.push(DVec3::new(x, y, f(x, y)));
        }
    }
    let fit = fit_surface(&pts, &[], &SurfaceFitOptions { tolerance: 1e-6, interpolate: true, smooth: false }).unwrap();
    assert!(fit.max_deviation < 1e-3, "deviation {}", fit.max_deviation);
    assert!(fit.face.area().unwrap() > 100.0);
    // Dense noisy samples: smoothing fit stays close.
    let mut dense = Vec::new();
    for i in 0..60 {
        for j in 0..50 {
            let x = -6.0 + 12.0 * i as f64 / 59.0;
            let y = -5.0 + 10.0 * j as f64 / 49.0;
            let noise = 1e-3 * (((i * 31 + j * 17) % 7) as f64 - 3.0);
            dense.push(DVec3::new(x, y, f(x, y) + noise));
        }
    }
    let fit = fit_surface(&dense, &[], &SurfaceFitOptions { tolerance: 1e-4, interpolate: false, smooth: false }).unwrap();
    assert!(fit.max_deviation < 0.05, "dense deviation {}", fit.max_deviation);
    // Collinear points are rejected.
    assert!(fit_surface(&[DVec3::ZERO, DVec3::X, DVec3::X * 2.0], &[], &SurfaceFitOptions::default()).is_err());
}

