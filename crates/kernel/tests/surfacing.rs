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


/// FeatureScript's `opCreateBSplineSurface` (std's `bSplineSurface` examples,
/// with their knots padded and control points overlapped as std does).
#[test]
fn bspline_surface_from_featurescript_definition() {
    let inch = 0.0254;
    let grid = |rows: &[&[[f64; 3]]]| -> (Vec<DVec3>, usize, usize) {
        let pts = rows.iter().flat_map(|r| r.iter().map(|p| DVec3::from_array(*p) * inch)).collect();
        (pts, rows.len(), rows[0].len())
    };
    // A bump on the XY plane: clamped, degree 2 both ways.
    let (poles, nu, nv) = grid(&[
        &[[-2., 2., 0.], [-1., 2., 0.], [0., 2., 0.], [1., 2., 0.], [2., 2., 0.]],
        &[[-2., 1., 0.], [-1., 1., 0.], [0., 1., 0.], [1., 1., 0.], [2., 1., 0.]],
        &[[-2., 0., 0.], [-1., 0., 0.], [0., 0., 1.], [1., 0., 0.], [2., 0., 0.]],
        &[[-2., -2., 0.], [-1., -2., 0.], [0., -2., 0.], [1., -2., 0.], [2., -2., 0.]],
    ]);
    let bump = BSplineSurfaceData {
        poles,
        nu,
        nv,
        weights: None,
        u_knots: vec![0.0, 0.0, 0.0, 0.1, 1.0, 1.0, 1.0],
        v_knots: vec![0.0, 0.0, 0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0, 1.0],
        u_degree: 2,
        v_degree: 2,
        u_periodic: false,
        v_periodic: false,
    };
    let sheet = make_bspline_surface(&bump, &[], false).unwrap();
    assert!(sheet.is_valid());
    assert_eq!((sheet.count(ShapeType::Face), sheet.count(ShapeType::Edge), sheet.count(ShapeType::Solid)), (1, 4, 0));
    // Clamped: the corners are the corner control points.
    let b = sheet.bounding_box(true).unwrap();
    assert!((b.min.x + 2.0 * inch).abs() < 1e-6 && (b.max.y - 2.0 * inch).abs() < 1e-6 && (b.min.y + 2.0 * inch).abs() < 1e-6, "{b:?}");
    assert!(b.max.z > 0.1 * inch && b.max.z < inch, "{b:?}");
    let full = area(&sheet);
    // face_bspline gives the surface back in the same form (open).
    same_surface(&sheet.faces()[0].face_bspline().unwrap(), &bump);

    // Inside a square loop of four lines in (u, v): a smaller sheet.
    let line = |a: DVec2, b: DVec2| BSplineCurve2dData { poles: vec![a, b], weights: None, knots: vec![0.0, 0.0, 1.0, 1.0], degree: 1, periodic: false };
    let (p0, p1, p2, p3) = (DVec2::new(0.25, 0.25), DVec2::new(0.75, 0.25), DVec2::new(0.75, 0.75), DVec2::new(0.25, 0.75));
    let trimmed = make_bspline_surface(&bump, &[line(p0, p1), line(p1, p2), line(p2, p3), line(p3, p0)], false).unwrap();
    assert!(trimmed.is_valid());
    assert_eq!((trimmed.count(ShapeType::Face), trimmed.count(ShapeType::Edge)), (1, 4));
    let a = area(&trimmed);
    assert!(a > 0.05 * full && a < 0.6 * full, "{a} of {full}");
    // An open loop is refused.
    assert!(make_bspline_surface(&bump, &[line(p0, p1), line(p1, p2), line(p2, p3)], false).is_err());
    // So are knots that don't fit the control points.
    let mut bad = bump.clone();
    bad.u_knots.pop();
    assert!(make_bspline_surface(&bad, &[], false).is_err());

    // A sheared tube: periodic in u (4 rows + the 2 overlapping ones), linear in v.
    let (poles, nu, nv) = grid(&[
        &[[0., 0., 1.], [-1., 0., 0.], [-2., 0., -1.]],
        &[[1., 1., 1.], [0., 1., 0.], [-1., 1., -1.]],
        &[[2., 0., 1.], [1., 0., 0.], [0., 0., -1.]],
        &[[1., -1., 1.], [0., -1., 0.], [-1., -1., -1.]],
        &[[0., 0., 1.], [-1., 0., 0.], [-2., 0., -1.]],
        &[[1., 1., 1.], [0., 1., 0.], [-1., 1., -1.]],
    ]);
    let tube = BSplineSurfaceData {
        poles,
        nu,
        nv,
        weights: None,
        u_knots: vec![-0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5],
        v_knots: vec![0.0, 0.0, 0.5, 1.0, 1.0],
        u_degree: 2,
        v_degree: 1,
        u_periodic: true,
        v_periodic: false,
    };
    let t = make_bspline_surface(&tube, &[], true).unwrap();
    // Periodic in u: the overlapping rows and the unrolled knots come back.
    same_surface(&t.faces()[0].face_bspline().unwrap(), &tube);
    // Closed round u: one face with a seam (two end loops + the seam), and no
    // solid since its ends are open.
    assert!(t.is_valid());
    assert_eq!((t.count(ShapeType::Face), t.count(ShapeType::Edge), t.count(ShapeType::Solid)), (1, 3, 0));

    // A torus-like net periodic both ways (8 x 8 + overlaps): with make_solid, a solid.
    let (big, small) = (0.03, 0.01);
    let ring = |i: usize, j: usize| {
        let (a, b) = (2.0 * PI * (i % 8) as f64 / 8.0, 2.0 * PI * (j % 8) as f64 / 8.0);
        DVec3::new((big + small * b.cos()) * a.cos(), (big + small * b.cos()) * a.sin(), small * b.sin())
    };
    let knots: Vec<f64> = (0..13).map(|i| (i as f64 - 2.0) / 8.0).collect();
    let torus = BSplineSurfaceData {
        poles: (0..10).flat_map(|i| (0..10).map(move |j| (i, j))).map(|(i, j)| ring(i, j)).collect(),
        nu: 10,
        nv: 10,
        weights: None,
        u_knots: knots.clone(),
        v_knots: knots,
        u_degree: 2,
        v_degree: 2,
        u_periodic: true,
        v_periodic: true,
    };
    let solid = make_bspline_surface(&torus, &[], true).unwrap();
    assert!(solid.is_valid());
    assert_eq!(solid.count(ShapeType::Solid), 1);
    let v = volume_properties(&solid).unwrap().mass;
    let exact = 2.0 * PI * PI * big * small * small; // the torus through the net (the spline lies inside)
    assert!(v > 0.6 * exact && v < exact, "{v} vs {exact}");
    assert_eq!(make_bspline_surface(&torus, &[], false).unwrap().count(ShapeType::Solid), 0);
}

/// Wrapped onto a cylinder, the plate's sides along the axis are lines and
/// the sides around it are arcs, as in Onshape (std sheet metal Bend calls
/// evLine on the wrapped bend's edges).
#[test]
fn wrapped_straight_sides_are_lines_and_arcs() {
    let (face, plane) = holed_plate();
    let src = Developable::Plane { origin: plane.origin, x_dir: DVec3::X, normal: DVec3::Z };
    let dst = cylinder_x();
    let a_s = src.flat(DVec3::new(0.0, 0.0, 10.0), 0.0);
    let a_d = dst.flat(DVec3::new(0.0, 0.0, 10.0), 0.0);
    let (_, _, sn) = src.flat_axes(a_s, 0.0);
    let (_, _, dn) = dst.flat_axes(a_d, 0.0);
    let mirror = (sn.dot(DVec3::Z) > 0.0) != (dn.dot(DVec3::Z) > 0.0);
    let pl = Placement2d::align(a_s, DVec2::X, a_d, DVec2::Y, mirror);
    let out = wrap_faces(std::slice::from_ref(&face), &src, 0.0, &dst, 0.0, &pl, 1e-7).unwrap().shape;
    let kinds: Vec<String> = out.edges().iter().map(|e| format!("{:?}", e.edge_info().unwrap().curve).split([' ', '{', '(']).next().unwrap_or_default().to_string()).collect();
    let (mut lines, mut arcs) = (0, 0);
    for e in out.edges() {
        let i = e.edge_info().unwrap();
        let along = (i.end - i.start).normalize_or_zero().dot(DVec3::X).abs() > 1.0 - 1e-9;
        match i.curve {
            Curve::Line { .. } if along => lines += 1,
            Curve::Circle { radius, .. } if (radius - 10.0).abs() < 1e-7 => arcs += 1,
            _ => {}
        }
    }
    assert_eq!((lines, arcs), (2, 2), "{kinds:?}");
}

/// Two B-spline surfaces in FeatureScript's form, equal to 1e-9.
fn same_surface(got: &BSplineSurfaceData, want: &BSplineSurfaceData) {
    let near = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert_eq!((got.nu, got.nv, got.u_degree, got.v_degree, got.u_periodic, got.v_periodic), (want.nu, want.nv, want.u_degree, want.v_degree, want.u_periodic, want.v_periodic));
    assert!(near(&got.u_knots, &want.u_knots) && near(&got.v_knots, &want.v_knots), "knots {:?} {:?}", got.u_knots, got.v_knots);
    assert!(got.poles.iter().zip(&want.poles).all(|(a, b)| a.distance(*b) < 1e-9), "poles {:?}", got.poles);
    assert_eq!(got.weights.is_some(), want.weights.is_some());
}
