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
