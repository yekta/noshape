use glam::DVec3;
use noshape_kernel::*;
use std::f64::consts::PI;

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

fn rect_face(plane: &Plane, w: f64, h: f64) -> Shape {
    let p = [DVec2::new(-w / 2.0, -h / 2.0), DVec2::new(w / 2.0, -h / 2.0), DVec2::new(w / 2.0, h / 2.0), DVec2::new(-w / 2.0, h / 2.0)];
    let segs: Vec<_> = (0..4).map(|i| Segment2d::Line { a: p[i], b: p[(i + 1) % 4] }).collect();
    make_face_2d(plane, &segs, &[]).unwrap()
}

fn box10() -> Shape {
    make_box(&Plane::xy(), 10.0, 10.0, 10.0).unwrap()
}

fn face_at_z(s: &Shape, z: f64) -> Shape {
    s.faces().into_iter().find(|f| close(f.centroid().unwrap().z, z, 1e-7) && f.is_planar()).unwrap()
}

#[test]
fn sweep_along_arc() {
    // Circle profile in the XZ plane at (10,0,0), swept 90 degrees around Z.
    let path = make_arc(DVec3::ZERO, DVec3::Z, DVec3::X, 10.0, 0.0, PI / 2.0).unwrap();
    let prof_plane = Plane::new(DVec3::new(10.0, 0.0, 0.0), DVec3::Y, DVec3::X);
    let prof = make_face_2d(&prof_plane, &[Segment2d::Circle { center: DVec2::ZERO, radius: 1.0 }], &[]).unwrap();
    let r = sweep(&prof, &path, &SweepOptions::default()).unwrap();
    assert_eq!(r.shape.shape_type(), ShapeType::Solid);
    // Pappus: area * path length of the centroid.
    assert!(close(r.shape.volume().unwrap(), PI * 1.0 * (10.0 * PI / 2.0), 1e-4));
    assert_eq!(r.history.first_faces().len(), 1);
    assert_eq!(r.history.last_faces().len(), 1);
    let circle_edge = prof.index_of(&prof.edges()[0], ShapeType::Edge).unwrap() as u32;
    assert!(!r.history.generated(0, ShapeType::Edge, circle_edge).is_empty());
    // Pipe and other modes
    let p = sweep_pipe(&prof, &path).unwrap();
    assert!(close(p.shape.volume().unwrap(), PI * 5.0 * PI, 1e-4));
    let f = sweep(&prof, &path, &SweepOptions { mode: SweepMode::Frenet, ..Default::default() }).unwrap();
    assert!(close(f.shape.volume().unwrap(), PI * 5.0 * PI, 1e-4));
    let d = sweep(&prof, &path, &SweepOptions { mode: SweepMode::Discrete, ..Default::default() }).unwrap();
    assert!(d.shape.volume().unwrap() > 0.0);
    // Scaled sweep along a line: frustum.
    let line = make_line(DVec3::ZERO, DVec3::new(0.0, 0.0, 10.0)).unwrap();
    let c = make_face_2d(&Plane::xy(), &[Segment2d::Circle { center: DVec2::ZERO, radius: 2.0 }], &[]).unwrap();
    let s = sweep(&c, &line, &SweepOptions { scale_end: 0.5, ..Default::default() }).unwrap();
    assert!(close(s.shape.volume().unwrap(), PI * 10.0 / 3.0 * (4.0 + 2.0 + 1.0), 1e-3), "{}", s.shape.volume().unwrap());
    // Face with hole uses MakePipe.
    let ring = make_face_2d(&Plane::xy(), &[Segment2d::Circle { center: DVec2::ZERO, radius: 2.0 }], &[vec![Segment2d::Circle { center: DVec2::ZERO, radius: 1.0 }]]).unwrap();
    let t = sweep(&ring, &line, &SweepOptions::default()).unwrap();
    assert!(close(t.shape.volume().unwrap(), PI * 3.0 * 10.0, 1e-6));
}

#[test]
fn loft_rect_to_circle() {
    let r = rect_face(&Plane::xy(), 4.0, 4.0);
    let c = make_wire_2d(&Plane::xy().offset(5.0), &[Segment2d::Circle { center: DVec2::ZERO, radius: 2.0 }]).unwrap();
    let l = loft(&[r.clone(), c.clone()], &LoftOptions { solid: true, ..Default::default() }).unwrap();
    assert_eq!(l.shape.shape_type(), ShapeType::Solid);
    let v = l.shape.volume().unwrap();
    assert!(v > PI * 4.0 * 5.0 * 0.9 && v < 16.0 * 5.0, "volume {v}");
    assert!(l.shape.is_valid());
    assert_eq!(l.history.first_faces().len(), 1);
    assert_eq!(l.history.last_faces().len(), 1);
    for i in 0..4u32 {
        assert!(!l.history.generated(0, ShapeType::Edge, i).is_empty());
    }
    let ruled = loft(&[r.clone(), rect_face(&Plane::xy().offset(5.0), 2.0, 2.0)], &LoftOptions { solid: true, ruled: true, ..Default::default() }).unwrap();
    assert!(close(ruled.shape.volume().unwrap(), 5.0 / 3.0 * (16.0 + 4.0 + 8.0), 1e-6));
    // To a point (pyramid).
    let apex = make_vertex(DVec3::new(0.0, 0.0, 3.0)).unwrap();
    let pyr = loft(&[r.clone(), apex], &LoftOptions { solid: true, ruled: true, ..Default::default() }).unwrap();
    assert!(close(pyr.shape.volume().unwrap(), 16.0, 1e-6));
    // Vertex matching does not break the loft.
    let m = loft(&[r, rect_face(&Plane::xy().offset(5.0), 2.0, 2.0)], &LoftOptions {
        solid: true, ruled: true,
        start_points: Some(vec![DVec3::new(2.0, 2.0, 0.0), DVec3::new(1.0, 1.0, 5.0)]), ..Default::default()
    }).unwrap();
    assert!(close(m.shape.volume().unwrap(), 5.0 / 3.0 * 28.0, 1e-6));
}

/// Live "psb (10-24) loft end derivative": □20 -> □10 20 up with Start
/// Normal to profile m1 bulges (live 5476.391496; the ruled frustum is
/// 4666.67). The corner rule with a natural end gives 5476.310271 exactly;
/// Parasolid's interior columns carry a small extra bump (1.5e-5).
#[test]
fn loft_end_condition_normal() {
    let a = rect_face(&Plane::xy(), 20.0, 20.0);
    let b = rect_face(&Plane::xy().offset(20.0), 10.0, 10.0);
    let l = loft(&[a, b], &LoftOptions { solid: true, start: LoftEnd::Normal(1.0), ..Default::default() }).unwrap();
    let v = l.shape.volume().unwrap();
    assert!(close(v, 5476.310270954884, 1e-9), "volume {v}");
    assert!(l.shape.is_valid());
}

#[test]
fn extrude_variants() {
    let f = rect_face(&Plane::xy(), 2.0, 2.0);
    let s = extrude_symmetric(&f, DVec3::new(0.0, 0.0, 4.0)).unwrap();
    let bb = s.shape.bounding_box(true).unwrap();
    assert!(close(bb.min.z, -2.0, 1e-6) && close(bb.max.z, 2.0, 1e-6));
    assert_eq!(s.history.first_faces().len(), 1);
    let t = extrude_between(&f, DVec3::new(0.0, 0.0, -1.0), DVec3::new(0.0, 0.0, 3.0), 0.0).unwrap();
    assert!(close(t.shape.volume().unwrap(), 16.0, 1e-9));
    let first = t.shape.sub_shape(ShapeType::Face, t.history.first_faces()[0] as usize).unwrap();
    assert!(close(first.centroid().unwrap().z, -1.0, 1e-9));
    // Draft: positive angle narrows the profile.
    let d = extrude_between(&f, DVec3::ZERO, DVec3::new(0.0, 0.0, 2.0), 5f64.to_radians()).unwrap();
    let v = d.shape.volume().unwrap();
    assert!(v < 8.0 && v > 6.0, "draft volume {v}");
    assert!(d.shape.is_valid());
    assert_eq!(d.history.last_faces().len(), 1);
    // Surface extrude of a wire.
    let w = make_polyline(&[DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0)], false).unwrap();
    let srf = extrude(&w, DVec3::Z).unwrap();
    assert_eq!(srf.shape.faces().len(), 2);
    // Up to point.
    let u = extrude_up_to_point(&f, DVec3::Z, DVec3::new(5.0, 5.0, 3.0)).unwrap();
    assert!(close(u.shape.volume().unwrap(), 12.0, 1e-9));
}

#[test]
fn extrude_up_to_face_and_body() {
    // Inclined plane face as target.
    let target_plane = Plane::new(DVec3::new(0.0, 0.0, 5.0), DVec3::new(0.0, 0.0, 1.0), DVec3::X);
    let tgt = make_plane_face(&target_plane, -10.0, 10.0, -10.0, 10.0).unwrap();
    let f = rect_face(&Plane::xy(), 2.0, 2.0);
    let r = extrude_up_to(&f, DVec3::Z, &tgt, 0.0).unwrap();
    assert!(close(r.shape.volume().unwrap(), 20.0, 1e-6), "{}", r.shape.volume().unwrap());
    assert_eq!(r.history.first_faces().len(), 1);
    assert_eq!(r.history.last_faces().len(), 1);
    // Up to a body: a box floating at z in [3, 6].
    let body = make_box(&Plane::new(DVec3::new(-5.0, -5.0, 3.0), DVec3::Z, DVec3::X), 10.0, 10.0, 3.0).unwrap();
    let r2 = extrude_up_to(&f, DVec3::Z, &body, 0.0).unwrap();
    assert!(close(r2.shape.volume().unwrap(), 12.0, 1e-6));
    // Target that does not cover the profile -> error.
    let small = make_plane_face(&target_plane, -0.1, 0.1, -0.1, 0.1).unwrap();
    assert!(extrude_up_to(&f, DVec3::Z, &small, 0.0).is_err());
}

#[test]
fn draft_faces() {
    let b = box10();
    let sides: Vec<Shape> = b
        .faces()
        .into_iter()
        .filter(|f| f.face_plane().unwrap().unwrap().normal.z.abs() < 0.5)
        .collect();
    assert_eq!(sides.len(), 4);
    let r = draft(&b, &sides, DVec3::Z, &Plane::xy(), 3f64.to_radians()).unwrap();
    assert!(r.shape.is_valid());
    let v = r.shape.volume().unwrap();
    assert!((v - 1000.0).abs() > 1.0, "volume {v}");
    for (i, _) in sides.iter().enumerate() {
        let fi = b.index_of(&sides[i], ShapeType::Face).unwrap() as u32;
        assert_eq!(r.history.modified(0, ShapeType::Face, fi).len(), 1);
    }
}

#[test]
fn thicken_and_offset() {
    let f = rect_face(&Plane::xy(), 2.0, 2.0);
    let t = thicken(&f, 1.0, 0.0).unwrap();
    assert!(close(t.shape.volume().unwrap().abs(), 4.0, 1e-6));
    let t2 = thicken(&f, 1.0, 0.5).unwrap();
    assert!(close(t2.shape.volume().unwrap().abs(), 6.0, 1e-6));
    let o = offset_shape(&f, 2.0, JoinType::Arc).unwrap();
    assert!(close(o.shape.bounding_box(true).unwrap().center().z, 2.0, 1e-6));
    let b = box10();
    let ob = offset_shape(&b, 1.0, JoinType::Intersection).unwrap();
    assert!(close(ob.shape.volume().unwrap().abs(), 1728.0, 1e-6));
}

#[test]
fn delete_and_offset_faces() {
    let b = box10();
    let hole = make_cylinder(&Plane::new(DVec3::new(5.0, 5.0, -1.0), DVec3::Z, DVec3::X), 2.0, 12.0, 2.0 * PI).unwrap();
    let holed = cut(&b, &hole).unwrap().shape;
    let cyl: Vec<Shape> = holed.faces().into_iter().filter(|f| matches!(f.face_surface().unwrap(), Surface::Cylinder { .. })).collect();
    assert!(!cyl.is_empty());
    let r = delete_faces(&holed, &cyl).unwrap();
    assert!(close(r.shape.volume().unwrap(), 1000.0, 1e-6));
    let top = face_at_z(&b, 10.0);
    let o = offset_faces(&b, &[top], 2.0).unwrap();
    assert!(close(o.shape.volume().unwrap(), 1200.0, 1e-6), "{}", o.shape.volume().unwrap());
    assert!(matches!(replace_face(&b, &[], &b), Err(KernelError::Unsupported(_))));
}

#[test]
fn split_and_section() {
    let b = box10();
    let r = split_by_plane(&b, &Plane::new(DVec3::new(0.0, 0.0, 4.0), DVec3::Z, DVec3::X)).unwrap();
    let solids = r.shape.solids();
    assert_eq!(solids.len(), 2);
    let mut vols: Vec<f64> = solids.iter().map(|s| s.volume().unwrap()).collect();
    vols.sort_by(f64::total_cmp);
    assert!(close(vols[0], 400.0, 1e-6) && close(vols[1], 600.0, 1e-6));
    // Side faces are split into two pieces each.
    let side = b.faces().into_iter().position(|f| f.face_plane().unwrap().unwrap().normal.x < -0.5).unwrap() as u32;
    assert_eq!(r.history.modified(0, ShapeType::Face, side).len(), 2);
    // Split by another solid.
    let tool = make_box(&Plane::new(DVec3::new(5.0, -1.0, -1.0), DVec3::Z, DVec3::X), 10.0, 12.0, 12.0).unwrap();
    let r2 = split(std::slice::from_ref(&b), &[tool], &BooleanOptions::default()).unwrap();
    assert_eq!(r2.shape.solids().len(), 2);
    // Imprint a line onto the top face.
    let line = make_line(DVec3::new(0.0, 5.0, 10.0), DVec3::new(10.0, 5.0, 10.0)).unwrap();
    let im = imprint(&b, &[line]).unwrap();
    assert_eq!(im.shape.faces().len(), 7);
    // Plane section and caps.
    let sp = Plane::new(DVec3::new(0.0, 0.0, 5.0), DVec3::Z, DVec3::X);
    let wires = section_wires(&b, &sp).unwrap();
    assert_eq!(wires.len(), 1);
    assert!(close(wires[0].length().unwrap(), 40.0, 1e-6));
    let caps = section_caps(&b, &sp).unwrap();
    assert!(close(caps.area().unwrap(), 100.0, 1e-6));
    let holed = cut(&b, &make_cylinder(&Plane::new(DVec3::new(5.0, 5.0, -1.0), DVec3::Z, DVec3::X), 2.0, 12.0, 2.0 * PI).unwrap()).unwrap().shape;
    let caps2 = section_caps(&holed, &sp).unwrap();
    assert!(close(caps2.area().unwrap(), 100.0 - 4.0 * PI, 1e-6));
    // Intersection of two shapes.
    let s = section(&b, &make_plane_face(&sp, -1.0, 20.0, -1.0, 20.0).unwrap()).unwrap();
    assert_eq!(s.shape.edges().len(), 4);
}

#[test]
fn booleans_multi_and_unify() {
    let a = make_box(&Plane::xy(), 10.0, 10.0, 10.0).unwrap();
    let b = make_box(&Plane::new(DVec3::new(10.0, 0.0, 0.0), DVec3::Z, DVec3::X), 10.0, 10.0, 10.0).unwrap();
    let c = make_box(&Plane::new(DVec3::new(20.0, 0.0, 0.0), DVec3::Z, DVec3::X), 10.0, 10.0, 10.0).unwrap();
    let plain = boolean(BooleanOp::Fuse, std::slice::from_ref(&a), &[b.clone(), c.clone()], &BooleanOptions::default()).unwrap();
    assert!(close(plain.shape.volume().unwrap(), 3000.0, 1e-9));
    assert_eq!(plain.shape.faces().len(), 14);
    let u = boolean(BooleanOp::Fuse, std::slice::from_ref(&a), &[b, c], &BooleanOptions { unify: true, ..Default::default() }).unwrap();
    assert_eq!(u.shape.faces().len(), 6);
    // The top faces of all three boxes are merged into one output face.
    let top_a = a.faces().into_iter().position(|f| close(f.centroid().unwrap().z, 10.0, 1e-9)).unwrap() as u32;
    let ma = u.history.modified(0, ShapeType::Face, top_a);
    let mc = u.history.modified(2, ShapeType::Face, top_a);
    assert_eq!(ma.len(), 1);
    assert_eq!(ma, mc);
    // Coincident faces between boxes are deleted.
    let right_a = a.faces().into_iter().position(|f| close(f.centroid().unwrap().x, 10.0, 1e-9)).unwrap() as u32;
    assert!(u.history.is_deleted(0, ShapeType::Face, right_a));
    let fz = boolean(BooleanOp::Fuse, std::slice::from_ref(&a), &[a.translated(DVec3::new(5.0, 0.0, 0.0)).unwrap()], &BooleanOptions { fuzzy: 1e-5, glue: Glue::Off, ..Default::default() }).unwrap();
    assert!(close(fz.shape.volume().unwrap(), 1500.0, 1e-9));
}

#[test]
fn sew_fill_enclose() {
    let b = box10();
    let faces = b.faces();
    let r = sew(&faces, 1e-6, true).unwrap();
    assert_eq!(r.shape.shape_type(), ShapeType::Solid);
    assert!(close(r.shape.volume().unwrap(), 1000.0, 1e-6));
    assert!(sew(&faces[..5], 1e-6, true).is_err());
    // Fill the top boundary of a box with a planar patch.
    let top = face_at_z(&b, 10.0);
    let edges: Vec<FillEdge> = top.edges().into_iter().map(|e| FillEdge { edge: e, support: None, continuity: Continuity::Position }).collect();
    let f = fill_surface(&edges, &[]).unwrap();
    assert!(close(f.shape.area().unwrap(), 100.0, 1e-3));
    // Enclose: six planes around a region.
    let mut planes = Vec::new();
    for (o, n) in [(DVec3::ZERO, DVec3::Z), (DVec3::new(0.0, 0.0, 2.0), DVec3::Z), (DVec3::ZERO, DVec3::X), (DVec3::new(3.0, 0.0, 0.0), DVec3::X), (DVec3::ZERO, DVec3::Y), (DVec3::new(0.0, 4.0, 0.0), DVec3::Y)] {
        let p = Plane::from_normal(o, n);
        planes.push(make_plane_face(&p, -10.0, 10.0, -10.0, 10.0).unwrap());
    }
    let e = enclose(&planes).unwrap();
    let vols: Vec<f64> = e.shape.solids().iter().map(|s| s.volume().unwrap()).collect();
    assert!(vols.iter().any(|v| close(*v, 24.0, 1e-6)), "{vols:?}");
}

#[test]
fn holes_and_rib() {
    let plate = make_box(&Plane::xy(), 50.0, 50.0, 10.0).unwrap();
    // Counterbored hole profile: (radius, depth)
    let prof = [DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0), DVec2::new(5.0, 3.0), DVec2::new(2.5, 3.0), DVec2::new(2.5, 10.0), DVec2::new(0.0, 10.0)];
    let tool = revolve_profile_tool(&prof, &Plane::xy()).unwrap();
    assert!(close(tool.volume().unwrap(), PI * (25.0 * 3.0 + 6.25 * 7.0), 1e-6));
    let frames: Vec<Plane> = [(10.0, 10.0), (40.0, 40.0)].iter().map(|&(x, y)| Plane::new(DVec3::new(x, y, 10.0), DVec3::Z, DVec3::X)).collect();
    let r = cut_holes(&plate, &prof, &frames).unwrap();
    assert!(close(r.shape.volume().unwrap(), 25000.0 - 2.0 * PI * (75.0 + 6.25 * 7.0), 1e-6));
    // Rib: an L-shaped base, rib along the inner corner diagonal.
    let base = fuse(&make_box(&Plane::xy(), 20.0, 2.0, 20.0).unwrap(), &make_box(&Plane::xy(), 20.0, 20.0, 2.0).unwrap()).unwrap().shape;
    let plane = Plane::new(DVec3::new(10.0, 0.0, 0.0), DVec3::X, DVec3::Y);
    let wire = make_polyline(&[DVec3::new(10.0, 2.0, 15.0), DVec3::new(10.0, 15.0, 2.0)], false).unwrap();
    let rr = rib(&base, &wire, &plane, 2.0).unwrap();
    assert_eq!(rr.shape.solids().len(), 1);
    // Rib region: triangle (2,2)-(2,15)-(15,2) between the profile and the
    // inner corner: area 84.5, times 2 thick.
    let added = rr.shape.volume().unwrap() - base.volume().unwrap();
    assert!((added - 169.0).abs() < 1e-6, "rib added {added}");
    // OCCT's LinearForm may fail on this input, but must not crash.
    let _ = rib_linear_form(&base, &wire, &plane, 2.0, true);
}

#[test]
fn hlr_box_iso() {
    let b = box10();
    let dir = DVec3::new(-1.0, -1.0, -1.0);
    for exact in [true, false] {
        let edges = project_hlr(&b, dir, DVec3::Z, exact).unwrap();
        let visible: Vec<_> = edges.iter().filter(|e| e.visibility == Visibility::Visible).collect();
        let hidden: Vec<_> = edges.iter().filter(|e| e.visibility == Visibility::Hidden).collect();
        assert_eq!(visible.len(), 9, "exact={exact}: {visible:#?}");
        assert_eq!(hidden.len(), 3, "exact={exact}");
        assert!(edges.iter().all(|e| e.class != EdgeClass::Smooth), "exact={exact}");
        if exact {
            assert!(edges.iter().all(|e| e.class == EdgeClass::Sharp));
        }
        let mut srcs: Vec<u32> = edges.iter().filter_map(|e| e.source_edge).collect();
        srcs.sort();
        srcs.dedup();
        assert_eq!(srcs.len(), 12, "exact={exact}");
    }
    // A cylinder has silhouette (outline) edges.
    let c = make_cylinder(&Plane::xy(), 5.0, 10.0, 2.0 * PI).unwrap();
    let e = project_hlr(&c, DVec3::new(0.0, -1.0, 0.0), DVec3::Z, true).unwrap();
    assert!(e.iter().any(|x| x.class == EdgeClass::Outline && x.source_edge.is_none()));
}

#[test]
fn helix_and_curves() {
    let h = make_helix(&Plane::xy(), 5.0, 2.0, 10.0, false, 0.0).unwrap();
    let len = h.length().unwrap();
    let expected = 5.0 * ((2.0 * PI * 5.0f64).powi(2) + 4.0).sqrt();
    assert!(close(len, expected, 1e-3), "{len} vs {expected}");
    let bb = h.bounding_box(true).unwrap();
    assert!(close(bb.max.z, 10.0, 1e-4));
    let tapered = make_helix(&Plane::xy(), 5.0, 2.0, 10.0, true, 0.1).unwrap();
    assert!(tapered.length().unwrap() > 0.0);
    let s = make_interpolated_spline(&[DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0), DVec3::new(2.0, 0.0, 1.0)], &[], false).unwrap();
    assert!(matches!(s.edge_curve().unwrap(), Curve::BSpline { .. }));
    let bs = make_bspline(&[DVec3::ZERO, DVec3::X, DVec3::new(2.0, 1.0, 0.0)], None, &[0.0, 1.0], &[3, 3], 2, false).unwrap();
    assert!(bs.length().unwrap() > 2.0);
    let a = make_arc_3pts(DVec3::X, DVec3::Y, -DVec3::X).unwrap();
    assert!(close(a.length().unwrap(), PI, 1e-9));
    // 2D: ellipse, bspline, interpolated
    let e = make_edge_2d(&Plane::xy(), &Segment2d::Ellipse { center: DVec2::ZERO, major_radius: 2.0, minor_radius: 1.0, rotation: 0.3 }).unwrap();
    assert!(matches!(e.edge_curve().unwrap(), Curve::Ellipse { .. }));
    let w = make_wire_2d(&Plane::xy(), &[
        Segment2d::Line { a: DVec2::new(0.0, 0.0), b: DVec2::new(4.0, 0.0) },
        Segment2d::Arc { center: DVec2::new(4.0, 2.0), radius: 2.0, start_angle: -PI / 2.0, end_angle: PI / 2.0 },
        Segment2d::Line { a: DVec2::new(4.0, 4.0), b: DVec2::new(0.0, 4.0) },
        Segment2d::BSpline { poles: vec![DVec2::new(0.0, 4.0), DVec2::new(-1.0, 2.0), DVec2::new(0.0, 0.0)], weights: None, knots: vec![0.0, 1.0], mults: vec![3, 3], degree: 2, periodic: false },
    ]).unwrap();
    assert!(w.is_closed().unwrap());
    let f = make_face_from_wire(&w, true).unwrap();
    assert!(f.area().unwrap() > 16.0);
    let off = offset_wire_2d(&w, 0.5, JoinType::Arc, false).unwrap();
    assert!(off.length().unwrap() > w.length().unwrap());
    // Projection of a line onto a cylinder.
    let cyl = make_cylinder(&Plane::xy(), 5.0, 10.0, 2.0 * PI).unwrap();
    let line = make_line(DVec3::new(-2.0, -10.0, 5.0), DVec3::new(2.0, -10.0, 5.0)).unwrap();
    let pr = project_curves(&line, &cyl, Some(DVec3::Y)).unwrap();
    assert!(!pr.edges().is_empty());
}

#[test]
fn geometry_queries() {
    let c = make_cylinder(&Plane::xy(), 5.0, 10.0, 2.0 * PI).unwrap();
    let side = c.faces().into_iter().find(|f| matches!(f.face_surface().unwrap(), Surface::Cylinder { .. })).unwrap();
    if let Surface::Cylinder { radius, axis, .. } = side.face_surface().unwrap() {
        assert!(close(radius, 5.0, 1e-12));
        assert!((axis - DVec3::Z).length() < 1e-12);
    }
    let pr = side.project_point(DVec3::new(10.0, 0.0, 5.0)).unwrap();
    assert!(close(pr.distance, 5.0, 1e-9));
    let n = side.normal_at_point(DVec3::new(10.0, 0.0, 5.0)).unwrap();
    assert!((n - DVec3::X).length() < 1e-9);
    let n2 = side.normal_at_uv(pr.u, pr.v).unwrap();
    assert!((n2 - DVec3::X).length() < 1e-9);
    let uv = side.uv_bounds().unwrap();
    assert!(close(uv[1] - uv[0], 2.0 * PI, 1e-9));
    let circ = c.edges().into_iter().find(|e| matches!(e.edge_curve().unwrap(), Curve::Circle { .. })).unwrap();
    assert!(circ.is_closed().unwrap());
    let i = circ.edge_info().unwrap();
    let p = circ.point_at(i.first_param).unwrap();
    assert!(close(p.truncate().length(), 5.0, 1e-9));
    let t = circ.tangent_at(i.first_param).unwrap();
    assert!(close(t.length(), 1.0, 1e-9));
    assert_eq!(c.classify_point(DVec3::new(0.0, 0.0, 5.0), 1e-7).unwrap(), PointClass::Inside);
    assert_eq!(c.classify_point(DVec3::new(9.0, 0.0, 5.0), 1e-7).unwrap(), PointClass::Outside);
    let b = box10();
    let f0 = &b.faces();
    let a = angle_between(&f0[0], &f0[1]).unwrap();
    assert!(close(a, PI, 1e-9) || close(a, PI / 2.0, 1e-9) || a.abs() < 1e-9);
    let fixed = b.fixed(0.0).unwrap();
    assert!(fixed.is_valid());
}

#[test]
fn concurrent_tessellation_of_shared_shapes() {
    let b = box10();
    let c = make_cylinder(&Plane::new(DVec3::new(5.0, 5.0, 5.0), DVec3::Z, DVec3::X), 2.0, 10.0, 2.0 * PI).unwrap();
    let f = fuse(&b, &c).unwrap().shape;
    std::thread::scope(|s| {
        for i in 0..8 {
            let (b, f) = (b.clone(), f.clone());
            s.spawn(move || {
                let d = 0.5 / (i + 1) as f64;
                let t1 = tessellate(&b, d, 0.5, true).unwrap();
                let t2 = tessellate(&f, d, 0.5, true).unwrap();
                assert!(t1.triangle_count() > 0 && t2.triangle_count() > 0);
            });
        }
    });
}

#[test]
fn thicken_always_gives_positive_volume() {
    // Every combination of face orientation and thickness sides must give an
    // outward-oriented solid with volume area * (t1 + t2).
    let f = rect_face(&Plane::xy(), 4.0, 2.0);
    for face in [f.clone(), f.reversed().unwrap()] {
        for (t1, t2) in [(1.0, 0.0), (0.0, 1.0), (0.5, 0.25), (2.0, 1.0)] {
            let r = thicken(&face, t1, t2).unwrap();
            let v = r.shape.volume().unwrap();
            assert!(close(v, 8.0 * (t1 + t2), 1e-6), "t1={t1} t2={t2}: volume {v}");
            let vp = volume_properties(&r.shape).unwrap().mass;
            assert!(vp > 0.0, "inside-out solid for t1={t1} t2={t2}");
        }
    }
    // A curved sheet (half cylinder face).
    let c = make_cylinder(&Plane::xy(), 5.0, 10.0, PI).unwrap();
    let side = c.faces().into_iter().find(|x| matches!(x.face_surface().unwrap(), Surface::Cylinder { .. })).unwrap();
    for face in [side.clone(), side.reversed().unwrap()] {
        let r = thicken(&face, 1.0, 0.0).unwrap();
        assert!(r.shape.volume().unwrap() > 0.0);
    }
}

#[test]
fn project_curves_accepts_compounds() {
    let cyl = make_cylinder(&Plane::xy(), 5.0, 10.0, 2.0 * PI).unwrap();
    // A compound of two separate lines.
    let l1 = make_line(DVec3::new(-2.0, -10.0, 3.0), DVec3::new(2.0, -10.0, 3.0)).unwrap();
    let l2 = make_line(DVec3::new(-2.0, -10.0, 7.0), DVec3::new(2.0, -10.0, 7.0)).unwrap();
    let pr = project_curves(&Shape::compound(&[l1.clone(), l2.clone()]), &cyl, Some(DVec3::Y)).unwrap();
    assert!(pr.edges().len() >= 2, "{} edges", pr.edges().len());
    // A compound of faces as the curves (their boundaries are projected) and
    // a compound of faces as the target.
    let f = rect_face(&Plane::new(DVec3::new(0.0, -10.0, 5.0), -DVec3::Y, DVec3::X), 2.0, 2.0);
    let side = cyl.faces().into_iter().find(|x| matches!(x.face_surface().unwrap(), Surface::Cylinder { .. })).unwrap();
    let target = Shape::compound(std::slice::from_ref(&side));
    let pr = project_curves(&Shape::compound(&[f]), &target, Some(DVec3::Y)).unwrap();
    assert!(pr.edges().len() >= 4, "{} edges", pr.edges().len());
    // Normal projection of a compound.
    let pn = project_curves(&Shape::compound(&[l1, l2]), &target, None).unwrap();
    assert!(!pn.edges().is_empty());
}

#[test]
fn bspline_data_round_trip_and_edit() {
    let poles = [DVec3::ZERO, DVec3::new(1.0, 2.0, 0.0), DVec3::new(3.0, 2.0, 1.0), DVec3::new(4.0, 0.0, 0.0)];
    let e = make_bspline(&poles, None, &[0.0, 1.0], &[4, 4], 3, false).unwrap();
    let d = e.edge_bspline().unwrap();
    assert_eq!(d.degree, 3);
    assert_eq!(d.poles.len(), 4);
    assert!(d.weights.is_none());
    for (a, b) in d.poles.iter().zip(poles.iter()) {
        assert!((*a - *b).length() < 1e-12);
    }
    // Reversed edge: poles follow the edge direction.
    let r = e.reversed().unwrap().edge_bspline().unwrap();
    assert!((r.poles[0] - poles[3]).length() < 1e-12);
    // Edit a control point and a weight, rebuild.
    let mut d2 = d.clone();
    d2.poles[1] += DVec3::new(0.0, 0.0, 1.0);
    d2.weights = Some(vec![1.0, 2.0, 1.0, 1.0]);
    let e2 = make_bspline_edge(&d2).unwrap();
    let back = e2.edge_bspline().unwrap();
    assert!((back.poles[1] - d2.poles[1]).length() < 1e-12);
    assert!((back.weights.unwrap()[1] - 2.0).abs() < 1e-12);
    // Lines and arcs convert exactly (same endpoints, same length).
    let a = make_arc(DVec3::ZERO, DVec3::Z, DVec3::X, 2.0, 0.0, PI / 2.0).unwrap();
    let ad = a.edge_bspline().unwrap();
    assert!(ad.weights.is_some());
    let a2 = make_bspline_edge(&ad).unwrap();
    assert!(close(a2.length().unwrap(), PI, 1e-6), "{}", a2.length().unwrap());
    let l = make_line(DVec3::ZERO, DVec3::new(3.0, 4.0, 0.0)).unwrap();
    let ld = l.edge_bspline().unwrap();
    assert!((ld.poles[0]).length() < 1e-12 && (ld.poles.last().unwrap() - DVec3::new(3.0, 4.0, 0.0)).length() < 1e-12);
}

#[test]
fn interpolated_spline_keeps_tangent_magnitudes() {
    let pts = [DVec3::ZERO, DVec3::new(5.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 0.0)];
    let d = DVec3::new(1.0, 1.0, 0.0).normalize();
    let max_y = |t: f64| {
        let e = make_interpolated_spline(&pts, &[d * t, DVec3::ZERO, DVec3::ZERO], false).unwrap();
        let i = e.edge_info().unwrap();
        (0..=100).map(|k| e.point_at(i.first_param + (i.last_param - i.first_param) * k as f64 / 100.0).unwrap().y).fold(f64::MIN, f64::max)
    };
    let small = max_y(1.0);
    let big = max_y(8.0);
    assert!(big > small * 2.0, "magnitude ignored: {small} vs {big}");
    // Start/end tangent form.
    let e = make_interpolated_spline(&pts, &[d * 8.0, DVec3::X], false).unwrap();
    let e1 = make_interpolated_spline(&pts, &[d, DVec3::X], false).unwrap();
    let span = |e: &Shape| {
        let i = e.edge_info().unwrap();
        (0..=100).map(|k| e.point_at(i.first_param + (i.last_param - i.first_param) * k as f64 / 100.0).unwrap().y.abs()).fold(0.0, f64::max)
    };
    assert!(span(&e) > span(&e1) * 2.0);
}

/// Sweep with the profile locked to faces ("Lock profile faces"): a 2 x 4 mm
/// rectangle swept a quarter turn round a Ø40 cylinder on its side face stays
/// radial, so it sweeps the Pappus volume of a revolve.
#[test]
fn sweep_locked_to_cylinder_face() {
    let mm = 1e-3;
    let cyl = make_cylinder(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 20.0 * mm, 20.0 * mm, std::f64::consts::TAU).unwrap();
    let side: Vec<Shape> = cyl.faces().into_iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { .. }))).collect();
    let path = make_wire(&[make_arc(DVec3::new(0.0, 0.0, 10.0) * mm, DVec3::Z, DVec3::X, 20.0 * mm, 0.0, std::f64::consts::FRAC_PI_2).unwrap()]).unwrap();
    let p = |x: f64, z: f64| DVec3::new(x, 0.0, z) * mm;
    let pts = [p(19.0, 8.0), p(21.0, 8.0), p(21.0, 12.0), p(19.0, 12.0)];
    let edges: Vec<Shape> = (0..4).map(|i| make_line(pts[i], pts[(i + 1) % 4]).unwrap()).collect();
    let profile = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    let res = sweep_locked_to_faces(&[profile], &path, &Shape::compound(&side), &SweepOptions::default()).unwrap();
    assert!(res.shape.is_valid());
    let v = volume_properties(&res.shape).unwrap().mass / mm.powi(3);
    let want = 8.0 * std::f64::consts::FRAC_PI_2 * 20.0;
    assert!((v - want).abs() < 1e-3, "{v} vs {want}");
    // A path off the faces is refused.
    let off = make_wire(&[make_arc(DVec3::new(0.0, 0.0, 10.0) * mm, DVec3::Z, DVec3::X, 25.0 * mm, 0.0, 1.0).unwrap()]).unwrap();
    let face = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    assert!(sweep_locked_to_faces(&[face], &off, &Shape::compound(&side), &SweepOptions::default()).is_err());
}
