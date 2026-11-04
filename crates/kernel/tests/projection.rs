//! PROJ-RULED: point projection and point distances on B-spline faces whose
//! interior BRepExtrema's global search misses (ruled lofts, curved
//! parting-line drafts).

use glam::DVec3;
use noshape_kernel as k;
use std::f64::consts::PI;

/// A ruled loft between two wavy closed splines (degree 3 × 1 B-spline).
fn ruled_sheet() -> k::Shape {
    let ring = |r: f64, z: f64| -> k::Shape {
        let pts: Vec<DVec3> = (0..48)
            .map(|i| {
                let t = i as f64 / 48.0 * 2.0 * PI;
                let rr = r * (1.0 + 0.05 * (3.0 * t).cos());
                DVec3::new(rr * t.cos(), rr * t.sin(), z + 0.002 * t.sin())
            })
            .collect();
        k::make_wire(&[k::make_interpolated_spline(&pts, &[], true).unwrap()]).unwrap()
    };
    let opts = k::LoftOptions { ruled: true, solid: false, ..Default::default() };
    k::loft(&[ring(0.01, 0.0), ring(0.009, 0.01)], &opts).unwrap().shape
}

/// Points of the face, and points 1 mm off along the normal: projection and
/// distance find them (they read the boundary before).
/// `on_face` keeps the samples inside a trimmed face (its UV box is larger).
fn check(face: &k::Shape, on_face: &dyn Fn(DVec3) -> bool) {
    let b = face.uv_bounds().unwrap();
    let mut checked = 0;
    for (fu, fv) in [(0.5, 0.4), (0.25, 0.2), (0.75, 0.1), (0.1, 0.9), (0.6, 0.6), (0.3, 0.3), (0.9, 0.15)] {
        let (u, v) = (b[0] + fu * (b[1] - b[0]), b[2] + fv * (b[3] - b[2]));
        let on = face.point_at_uv(u, v).unwrap();
        if !on_face(on) {
            continue;
        }
        checked += 1;
        let q = face.project_point(on).unwrap();
        assert!(q.distance < 1e-9, "({fu}, {fv}): {} mm", q.distance * 1e3);
        let n = face.normal_at_uv(u, v).unwrap().normalize();
        let off = on + n * 0.001;
        let q = face.project_point(off).unwrap();
        assert!((q.distance - 0.001).abs() < 1e-7, "({fu}, {fv}) off: {} mm", q.distance * 1e3);
        let d = k::distance(&k::make_vertex(off).unwrap(), face).unwrap();
        assert!((d.distance - 0.001).abs() < 1e-7, "({fu}, {fv}) distance: {} mm", d.distance * 1e3);
    }
    assert!(checked >= 3, "{checked} samples on the face");
}

#[test]
fn ruled_loft_interior_points() {
    let sheet = ruled_sheet();
    for f in sheet.faces() {
        check(&f, &|_| true);
    }
}

#[test]
fn curved_draft_face_interior_points() {
    let cyl = k::make_cylinder(&k::Plane::from_normal(DVec3::ZERO, DVec3::Z), 0.01, 0.02, 2.0 * PI).unwrap();
    let n = DVec3::new(0.0, -0.5, 3f64.sqrt() / 2.0);
    let cutter = k::make_plane_face(&k::Plane::from_normal(DVec3::new(0.0, 0.0, 0.01), n), -0.05, 0.05, -0.05, 0.05).unwrap();
    let halves = k::split(&[cyl], &[cutter], &k::BooleanOptions::default()).unwrap().shape.solids();
    let s = k::fuse_all(&halves, &k::BooleanOptions { unify: false, ..Default::default() }).unwrap().shape;
    let faces = s.faces();
    let side: Vec<&k::Shape> = faces.iter().filter(|f| !f.is_planar()).collect();
    let upper = (*side.iter().max_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap()).clone();
    let lower = side.iter().min_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap().edges();
    let parting: Vec<k::Shape> = upper.edges().into_iter().filter(|e| lower.iter().any(|u| u.is_same(e))).collect();
    let r = k::draft_parting_curve(&s, std::slice::from_ref(&upper), &parting, DVec3::Z, &[5f64.to_radians()]).unwrap().shape;
    let drafted = r.faces().into_iter().find(|f| matches!(f.face_surface(), Ok(k::Surface::BSpline { .. }))).unwrap();
    check(&drafted, &|p| r.classify_point(p, 1e-7).unwrap() == k::PointClass::OnBoundary);
}

/// A plate's face with 400 holes (a Fill pattern's): a point over the face projects onto the plane, a point over a
/// hole onto the hole's rim, a point over the plate's corner onto the corner (the fast paths: the plane's foot when
/// inside, else the nearest edge by the edges' boxes).
#[test]
fn projection_onto_a_face_with_many_holes() {
    let mut s = k::make_box_corners(DVec3::ZERO, DVec3::new(0.1, 0.1, 0.002)).unwrap();
    let mut holes = Vec::new();
    for i in 0..20 {
        for j in 0..20 {
            let c = DVec3::new(0.0025 + 0.005 * i as f64, 0.0025 + 0.005 * j as f64, -0.001);
            holes.push(k::make_cylinder(&k::Plane::from_normal(c, DVec3::Z), 0.001, 0.004, 2.0 * PI).unwrap());
        }
    }
    s = k::cut(&s, &k::fuse_all(&holes, &k::BooleanOptions::default()).unwrap().shape).unwrap().shape;
    let top = s.faces().into_iter().find(|f| f.is_planar() && (f.centroid().unwrap().z - 0.002).abs() < 1e-9).unwrap();
    assert_eq!(top.edges().len(), 404);
    let h = 0.003;
    // Over the face between holes.
    let q = top.project_point(DVec3::new(0.005, 0.005, 0.002 + h)).unwrap();
    assert!((q.distance - h).abs() < 1e-12 && q.point.distance(DVec3::new(0.005, 0.005, 0.002)) < 1e-12, "{q:?}");
    // Over a hole's centre: its rim, r 1 mm away sideways.
    let c = DVec3::new(0.0475, 0.0525, 0.002);
    let q = top.project_point(c + DVec3::Z * h).unwrap();
    assert!((q.distance - (h * h + 0.001f64.powi(2)).sqrt()).abs() < 1e-9 && ((q.point - c).length() - 0.001).abs() < 1e-9, "{q:?}");
    // Past the plate's corner.
    let q = top.project_point(DVec3::new(-0.001, -0.001, 0.002)).unwrap();
    assert!(q.point.distance(DVec3::new(0.0, 0.0, 0.002)) < 1e-9, "{q:?}");
    // On a hole's rim (a Draft's tangent check projects edge midpoints) and on the plate's edge: the point itself.
    for p in [c + DVec3::X * 0.001, DVec3::new(0.05, 0.0, 0.002)] {
        let q = top.project_point(p).unwrap();
        assert!(q.distance < 1e-9 && q.point.distance(p) < 1e-9, "{p:?}: {q:?}");
        // And just above it.
        let q = top.project_point(p + DVec3::Z * h).unwrap();
        assert!((q.distance - h).abs() < 1e-9 && q.point.distance(p) < 1e-9, "{p:?} + h: {q:?}");
    }
}

/// A B-spline face of many poles (a displacement map's template, here 64 × 64 bumps): a point's projection is its
/// nearest point on the face as the exhaustive distance finds it, or nearer (on the surface, above a bump, past its
/// edges), the point it gives lies on the face, and its distance is the distance to that point.
#[test]
fn projection_onto_a_bspline_of_many_poles() {
    let n = 64;
    let knots: Vec<f64> = (0..n + 4).map(|i| ((i as f64 - 3.0) / (n as f64 - 3.0)).clamp(0.0, 1.0)).collect();
    let poles: Vec<DVec3> = (0..n * n)
        .map(|k| {
            let (i, j) = ((k / n) as f64, (k % n) as f64);
            DVec3::new(i, j, 0.3 * (0.7 * i).sin() * (0.5 * j).cos()) * 1e-3
        })
        .collect();
    let data = k::BSplineSurfaceData { poles, nu: n, nv: n, weights: None, u_knots: knots.clone(), v_knots: knots, u_degree: 3, v_degree: 3, u_periodic: false, v_periodic: false };
    let face = k::make_bspline_surface(&data, &[], false).unwrap().faces().remove(0);
    let mm = |x: f64, y: f64, z: f64| DVec3::new(x, y, z) * 1e-3;
    let points = [mm(10.0, 20.0, 0.0), mm(31.3, 7.7, 0.8), mm(5.5, 50.2, -1.5), mm(-3.0, 30.0, 0.2), mm(70.0, 70.0, 1.0), mm(40.0, -2.0, -0.4), mm(22.2, 41.1, 5.0)];
    for p in points {
        let q = face.project_point(p).unwrap();
        let reference = k::distance(&k::make_vertex(p).unwrap(), &face).unwrap().distance;
        assert!(q.distance <= reference + 1e-9, "{p:?}: {} against {reference}", q.distance);
        assert!((q.point.distance(p) - q.distance).abs() < 1e-12, "{p:?}: {q:?}");
        let on = k::distance(&k::make_vertex(q.point).unwrap(), &face).unwrap().distance;
        assert!(on < 1e-9, "{p:?}: the point given is {on} off the face");
    }
    // A point of the surface is its own projection.
    let s = face.point_at_uv(0.37, 0.61).unwrap();
    let q = face.project_point(s).unwrap();
    assert!(q.distance < 1e-9 && q.point.distance(s) < 1e-9, "{q:?}");
}
