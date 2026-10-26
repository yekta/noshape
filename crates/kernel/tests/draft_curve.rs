//! Parting-line draft about a curved parting line (`draft_parting_curve`).

use glam::DVec3;
use noshape_kernel as k;
use std::f64::consts::PI;

/// A Ø20 × 20 cylinder whose side face is split along the curve where
/// `cutter` meets it.
fn split_cylinder(cutter: &k::Shape) -> k::Shape {
    let cyl = k::make_cylinder(&k::Plane::from_normal(DVec3::ZERO, DVec3::Z), 0.01, 0.02, 2.0 * PI).unwrap();
    let halves = k::split(&[cyl], std::slice::from_ref(cutter), &k::BooleanOptions::default()).unwrap().shape.solids();
    assert_eq!(halves.len(), 2);
    k::fuse_all(&halves, &k::BooleanOptions { unify: false, ..Default::default() }).unwrap().shape
}

fn volume(s: &k::Shape) -> f64 {
    k::volume_properties(s).unwrap().mass
}

/// The lower side face and the parting edges between it and the upper one.
fn lower_face_and_parting(s: &k::Shape) -> (k::Shape, Vec<k::Shape>) {
    let faces = s.faces();
    let side: Vec<&k::Shape> = faces.iter().filter(|f| !f.is_planar()).collect();
    let lower = side.iter().min_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap();
    let upper = side.iter().max_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap();
    let up_edges = upper.edges();
    let parting: Vec<k::Shape> = lower.edges().into_iter().filter(|e| up_edges.iter().any(|u| u.is_same(e))).collect();
    ((*lower).clone(), parting)
}

/// On a level parting line both OCCT's draft and the ruled construction
/// apply: the same solid. A positive angle leans into the material.
#[test]
fn level_parting_line_matches_occt() {
    let level = k::make_plane_face(&k::Plane::from_normal(DVec3::new(0.0, 0.0, 0.01), DVec3::Z), -0.05, 0.05, -0.05, 0.05).unwrap();
    let s = split_cylinder(&level);
    let (lower, parting) = lower_face_and_parting(&s);
    assert!(!parting.is_empty());
    let a = 5f64.to_radians();
    let ours = k::draft_parting_curve(&s, std::slice::from_ref(&lower), &parting, DVec3::Z, &[a]).unwrap().shape;
    // Into the material: the lower half narrows to the bottom (a frustum).
    let tn = a.tan();
    let (r0, h) = (0.01, 0.01);
    let r1 = r0 - h * tn;
    let frustum = PI * h / 3.0 * (r0 * r0 + r0 * r1 + r1 * r1);
    let v = volume(&ours);
    let expect = PI * r0 * r0 * h + frustum;
    assert!((v - expect).abs() < 1e-12, "{v} vs {expect}");
    let neutral = k::Plane::from_normal(DVec3::new(0.0, 0.0, 0.01), DVec3::Z);
    for occt_angle in [a, -a] {
        if let Ok(r) = k::draft(&s, std::slice::from_ref(&lower), DVec3::Z, &neutral, occt_angle) {
            eprintln!("OCCT angle {occt_angle}: {} (ours {v})", volume(&r.shape));
        }
    }
    // Out of the material: it widens.
    let out = volume(&k::draft_parting_curve(&s, std::slice::from_ref(&lower), &parting, DVec3::Z, &[-a]).unwrap().shape);
    let r1 = r0 + h * tn;
    let expect = PI * r0 * r0 * h + PI * h / 3.0 * (r0 * r0 + r0 * r1 + r1 * r1);
    assert!((out - expect).abs() < 1e-12, "{out} vs {expect}");
}

/// A 30° sloped ellipse: the ruled construction drafts it (OCCT can't).
#[test]
fn sloped_ellipse_parting_line() {
    let n = DVec3::new(0.0, -(30f64.to_radians().sin()), 30f64.to_radians().cos());
    let cutter = k::make_plane_face(&k::Plane::from_normal(DVec3::new(0.0, 0.0, 0.01), n), -0.05, 0.05, -0.05, 0.05).unwrap();
    let s = split_cylinder(&cutter);
    let (lower, parting) = lower_face_and_parting(&s);
    let v0 = volume(&s);
    let r = k::draft_parting_curve(&s, std::slice::from_ref(&lower), &parting, DVec3::Z, &[5f64.to_radians()]).unwrap().shape;
    assert!(r.is_valid());
    let v = volume(&r);
    assert!(v < v0 && v > 0.8 * v0, "{v} vs {v0}");
    eprintln!("sloped: {} faces, volume {v}", r.faces().len());
}

/// The upper face against OCCT, both signs (calibration printout).
#[test]
fn level_parting_line_upper_face_matches_occt() {
    let level = k::make_plane_face(&k::Plane::from_normal(DVec3::new(0.0, 0.0, 0.01), DVec3::Z), -0.05, 0.05, -0.05, 0.05).unwrap();
    let s = split_cylinder(&level);
    let faces = s.faces();
    let upper = faces.iter().filter(|f| !f.is_planar()).max_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap().clone();
    let lower_edges = faces.iter().filter(|f| !f.is_planar()).min_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap().edges();
    let parting: Vec<k::Shape> = upper.edges().into_iter().filter(|e| lower_edges.iter().any(|u| u.is_same(e))).collect();
    let a = 5f64.to_radians();
    let neutral = k::Plane::from_normal(DVec3::new(0.0, 0.0, 0.01), DVec3::Z);
    let ours = volume(&k::draft_parting_curve(&s, std::slice::from_ref(&upper), &parting, DVec3::Z, &[a]).unwrap().shape);
    let occt = volume(&k::draft(&s, std::slice::from_ref(&upper), DVec3::Z, &neutral, a).unwrap().shape);
    let occt_neg = volume(&k::draft(&s, std::slice::from_ref(&upper), DVec3::Z, &neutral, -a).unwrap().shape);
    eprintln!("upper: ours(+a) {ours}, occt(+a) {occt}, occt(-a) {occt_neg}");
    assert!((ours - occt).abs() < 1e-11 || (ours - occt_neg).abs() < 1e-11);
}

/// Live A1 ("psb (10-25 m)"): the upper face of the sloped split drafted 5°
/// into the material: 5974.3440838 mm³.
#[test]
fn sloped_ellipse_upper_face_as_live() {
    let n = DVec3::new(0.0, -0.5, 3f64.sqrt() / 2.0);
    let cutter = k::make_plane_face(&k::Plane::from_normal(DVec3::new(0.0, 0.0, 0.01), n), -0.05, 0.05, -0.05, 0.05).unwrap();
    let s = split_cylinder(&cutter);
    let faces = s.faces();
    let side: Vec<&k::Shape> = faces.iter().filter(|f| !f.is_planar()).collect();
    let upper = (*side.iter().max_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap()).clone();
    let lower = side.iter().min_by(|a, b| a.centroid().unwrap().z.total_cmp(&b.centroid().unwrap().z)).unwrap().edges();
    let parting: Vec<k::Shape> = upper.edges().into_iter().filter(|e| lower.iter().any(|u| u.is_same(e))).collect();
    let r = k::draft_parting_curve(&s, std::slice::from_ref(&upper), &parting, DVec3::Z, &[5f64.to_radians()]).unwrap().shape;
    // Live's surface is a B-spline fit of the draft surface (~1e-5 mm).
    let v = volume(&r) * 1e9;
    assert!((v - 5974.3440838).abs() < 0.02, "{v} mm³");
    assert_eq!(r.faces().len(), 4);
}


