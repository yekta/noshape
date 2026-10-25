use noshape_kernel::*;
use std::f64::consts::PI;

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

fn rect(w: f64, h: f64) -> Vec<Segment2d> {
    let p = [DVec2::new(0.0, 0.0), DVec2::new(w, 0.0), DVec2::new(w, h), DVec2::new(0.0, h)];
    (0..4).map(|i| Segment2d::Line { a: p[i], b: p[(i + 1) % 4] }).collect()
}

#[test]
fn primitives() {
    let b = make_box(&Plane::xy(), 10.0, 20.0, 30.0).unwrap();
    assert_eq!(b.shape_type(), ShapeType::Solid);
    assert_eq!(b.faces().len(), 6);
    assert_eq!(b.edges().len(), 12);
    assert_eq!(b.vertices().len(), 8);
    assert!(close(b.volume().unwrap(), 6000.0, 1e-9));
    let c = make_cylinder(&Plane::xy(), 5.0, 10.0, 2.0 * PI).unwrap();
    assert!(close(c.volume().unwrap(), PI * 25.0 * 10.0, 1e-6));
    let s = make_sphere(&Plane::xy(), 2.0, 2.0 * PI).unwrap();
    assert!(close(s.volume().unwrap(), 4.0 / 3.0 * PI * 8.0, 1e-6));
    let k = make_cone(&Plane::xy(), 3.0, 0.0, 6.0, 2.0 * PI).unwrap();
    assert!(close(k.volume().unwrap(), PI * 9.0 * 6.0 / 3.0, 1e-6));
    let t = make_torus(&Plane::xy(), 10.0, 2.0, 2.0 * PI).unwrap();
    assert!(close(t.volume().unwrap(), 2.0 * PI * PI * 10.0 * 4.0, 1e-6));
    let w = make_wedge(&Plane::xy(), 10.0, 10.0, 10.0, 0.0).unwrap();
    assert!(close(w.volume().unwrap(), 500.0, 1e-6));
    assert!(b.is_valid());
    let bb = b.bounding_box(true).unwrap();
    assert!((bb.max - DVec3::new(10.0, 20.0, 30.0)).length() < 1e-6);
}

#[test]
fn topology_queries() {
    let b = make_box(&Plane::xy(), 1.0, 1.0, 1.0).unwrap();
    let ef = b.edge_faces().unwrap();
    assert_eq!(ef.len(), 12);
    assert!(ef.iter().all(|f| f.len() == 2));
    let vf = b.vertex_faces().unwrap();
    assert!(vf.iter().all(|f| f.len() == 3));
    assert_eq!(b.adjacent_faces(0).unwrap().len(), 4);
    let f0 = b.sub_shape(ShapeType::Face, 0).unwrap();
    assert_eq!(b.index_of(&f0, ShapeType::Face), Some(0));
    assert_eq!(b.faces()[3], b.sub_shape(ShapeType::Face, 3).unwrap());
    // Shapes are usable as hash keys.
    let set: std::collections::HashSet<Shape> = b.faces().into_iter().chain(b.faces()).collect();
    assert_eq!(set.len(), 6);
    for f in b.faces() {
        match f.face_surface().unwrap() {
            Surface::Plane(_) => {}
            other => panic!("unexpected surface {other:?}"),
        }
        assert!(close(f.area().unwrap(), 1.0, 1e-9));
        let n = f.face_plane().unwrap().unwrap().normal;
        let c = f.centroid().unwrap();
        // Outward normals: centroid + normal is outside the unit cube center.
        assert!((c + n * 0.1 - DVec3::splat(0.5)).length() > (c - DVec3::splat(0.5)).length());
    }
    let e = &b.edges()[0];
    let info = e.edge_info().unwrap();
    assert!(matches!(info.curve, Curve::Line { .. }));
    assert!(close(info.length, 1.0, 1e-9));
}

#[test]
fn extrude_rect_with_hole() {
    let plane = Plane::xy();
    let hole = vec![Segment2d::Circle { center: DVec2::new(5.0, 5.0), radius: 2.0 }];
    let tp = make_face_2d_tracked(&plane, &rect(10.0, 10.0), &[hole]).unwrap();
    let face = tp.shape.clone();
    assert_eq!(face.shape_type(), ShapeType::Face);
    assert_eq!(tp.segment_edges.len(), 5);
    assert!(close(face.area().unwrap(), 100.0 - PI * 4.0, 1e-6));
    let r = extrude(&face, DVec3::new(0.0, 0.0, 3.0)).unwrap();
    assert!(close(r.shape.volume().unwrap(), 3.0 * (100.0 - PI * 4.0), 1e-6));
    assert!(r.shape.is_valid());
    // Every profile edge generates exactly one side face.
    for (seg, &ei) in tp.segment_edges.iter().enumerate() {
        let g = r.history.generated(0, ShapeType::Edge, ei);
        let faces: Vec<_> = g.iter().filter(|(k, _)| *k == ShapeType::Face).collect();
        assert_eq!(faces.len(), 1, "segment {seg} generated {g:?}");
    }
    assert_eq!(r.history.first_faces().len(), 1);
    assert_eq!(r.history.last_faces().len(), 1);
    let top = r.shape.sub_shape(ShapeType::Face, r.history.last_faces()[0] as usize).unwrap();
    assert!(close(top.centroid().unwrap().z, 3.0, 1e-9));
    let bot = r.shape.sub_shape(ShapeType::Face, r.history.first_faces()[0] as usize).unwrap();
    assert!(close(bot.centroid().unwrap().z, 0.0, 1e-9));
}

#[test]
fn boolean_history() {
    let b = make_box(&Plane::xy(), 10.0, 10.0, 10.0).unwrap();
    let c = make_cylinder(&Plane::new(DVec3::new(5.0, 5.0, 5.0), DVec3::Z, DVec3::X), 2.0, 10.0, 2.0 * PI).unwrap();
    let r = fuse(&b, &c).unwrap();
    assert!(r.shape.is_valid());
    // The box top face (z = 10) is modified into exactly one face (with a hole).
    let top = b
        .faces()
        .iter()
        .position(|f| close(f.centroid().unwrap().z, 10.0, 1e-9))
        .unwrap() as u32;
    let m = r.history.modified(0, ShapeType::Face, top);
    assert_eq!(m.len(), 1, "{:?}", r.history.of_input(0, ShapeType::Face, top).collect::<Vec<_>>());
    // Bottom face of the box is kept unchanged.
    let bottom = b
        .faces()
        .iter()
        .position(|f| close(f.centroid().unwrap().z, 0.0, 1e-9))
        .unwrap() as u32;
    assert!(r.history.of_input(0, ShapeType::Face, bottom).any(|r| r.relation == Relation::Kept));
    assert!(!r.history.kept_faces().is_empty());
    // Cylinder bottom cap is inside the box: deleted.
    let cyl_bottom = c
        .faces()
        .iter()
        .position(|f| close(f.centroid().unwrap().z, 5.0, 1e-9))
        .unwrap() as u32;
    assert!(r.history.is_deleted(1, ShapeType::Face, cyl_bottom));
    let cut_r = cut(&b, &c).unwrap();
    assert!(close(cut_r.shape.volume().unwrap(), 1000.0 - PI * 4.0 * 5.0, 1e-6));
    let com = common(&b, &c).unwrap();
    assert!(close(com.shape.volume().unwrap(), PI * 4.0 * 5.0, 1e-6));
}

#[test]
fn fillet_and_chamfer() {
    let b = make_box(&Plane::xy(), 10.0, 10.0, 10.0).unwrap();
    let edges = b.edges();
    let r = fillet(&b, &edges[..1], 1.0).unwrap();
    assert!(r.shape.volume().unwrap() < 1000.0);
    let g = r.history.generated(0, ShapeType::Edge, 0);
    assert!(g.iter().any(|(k, _)| *k == ShapeType::Face), "{g:?}");
    let (_, fi) = g.iter().find(|(k, _)| *k == ShapeType::Face).unwrap();
    let blend = r.shape.sub_shape(ShapeType::Face, *fi as usize).unwrap();
    assert!(matches!(blend.face_surface().unwrap(), Surface::Cylinder { .. }));
    let c = chamfer(&b, &edges[..1], ChamferSpec::Equal(1.0), &[]).unwrap();
    assert!(close(c.shape.volume().unwrap(), 1000.0 - 0.5 * 10.0, 1e-6));
    let g = c.history.generated(0, ShapeType::Edge, 0);
    assert!(g.iter().any(|(k, _)| *k == ShapeType::Face));
    let c2 = chamfer(&b, &edges[..1], ChamferSpec::TwoDistances(1.0, 2.0), &[]).unwrap();
    assert!(close(c2.shape.volume().unwrap(), 1000.0 - 1.0 * 10.0, 1e-6));
    let c3 = chamfer(&b, &edges[..1], ChamferSpec::DistanceAngle(1.0, PI / 4.0), &[]).unwrap();
    assert!(close(c3.shape.volume().unwrap(), 1000.0 - 0.5 * 10.0, 1e-6));
}

#[test]
fn fillet_too_big_is_error_not_crash() {
    let b = make_box(&Plane::xy(), 10.0, 10.0, 10.0).unwrap();
    let e = b.edges();
    let r = fillet(&b, &e, 20.0);
    assert!(r.is_err(), "expected error");
    // Invalid inputs are rejected cleanly too.
    assert!(make_box(&Plane::xy(), -1.0, 1.0, 1.0).is_err());
    assert!(b.sub_shape(ShapeType::Face, 99).is_err());
    let other = make_box(&Plane::xy(), 1.0, 1.0, 1.0).unwrap();
    assert!(fillet(&b, &other.edges()[..1], 0.1).is_err());
}

#[test]
fn tessellation_sanity() {
    let b = make_box(&Plane::xy(), 10.0, 10.0, 10.0).unwrap();
    let c = make_cylinder(&Plane::new(DVec3::new(5.0, 5.0, 5.0), DVec3::Z, DVec3::X), 2.0, 10.0, 2.0 * PI).unwrap();
    let s = fuse(&b, &c).unwrap().shape;
    let t = tessellate(&s, 0.05, 0.3, true).unwrap();
    assert_eq!(t.faces.len(), s.faces().len());
    assert_eq!(t.edges.len(), s.edges().len());
    assert_eq!(t.vertices.len(), s.vertices().len());
    let total: u32 = t.faces.iter().map(|f| f.index_count).sum();
    assert_eq!(total as usize, t.indices.len());
    let mut next = 0;
    for f in &t.faces {
        assert_eq!(f.first_index, next);
        assert!(f.index_count > 0);
        next += f.index_count;
        for &i in &t.indices[f.first_index as usize..(f.first_index + f.index_count) as usize] {
            assert!(i >= f.first_vertex && i < f.first_vertex + f.vertex_count);
        }
    }
    for n in &t.normals {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((l - 1.0).abs() < 1e-3, "normal length {l}");
    }
    for (i, e) in s.edges().iter().enumerate() {
        if !e.edge_info().unwrap().degenerated {
            assert!(t.edge_polyline(i).len() >= 2, "edge {i} has empty polyline");
        }
    }
    // Normals point outward: for the bottom face (z=0) normal is -Z.
    let bottom = s.faces().iter().position(|f| f.centroid().unwrap().z.abs() < 1e-9).unwrap();
    let r = t.faces[bottom];
    let n = t.normals[r.first_vertex as usize];
    assert!(n[2] < -0.99);
    // Triangle winding matches the normal.
    let tri = &t.indices[r.first_index as usize..r.first_index as usize + 3];
    let p: Vec<DVec3> = tri.iter().map(|&i| { let q = t.positions[i as usize]; DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64) }).collect();
    assert!((p[1] - p[0]).cross(p[2] - p[0]).z < 0.0);
}

#[test]
fn mass_properties_box() {
    let b = make_box(&Plane::xy(), 2.0, 4.0, 6.0).unwrap();
    let p = volume_properties(&b).unwrap();
    assert!(close(p.mass, 48.0, 1e-9));
    assert!((p.center - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-9);
    // Ixx about COM = m (b^2 + c^2) / 12 with unit density.
    assert!(close(p.inertia.x_axis.x, 48.0 * (16.0 + 36.0) / 12.0, 1e-6));
    assert!(close(p.inertia.y_axis.y, 48.0 * (4.0 + 36.0) / 12.0, 1e-6));
    assert!(p.inertia.x_axis.y.abs() < 1e-6);
    let s = surface_properties(&b).unwrap();
    assert!(close(s.mass, 2.0 * (8.0 + 12.0 + 24.0), 1e-9));
    let d = distance(&b, &make_box(&Plane::new(DVec3::new(5.0, 0.0, 0.0), DVec3::Z, DVec3::X), 1.0, 1.0, 1.0).unwrap()).unwrap();
    assert!(close(d.distance, 3.0, 1e-9));
}

#[test]
fn brep_and_bin_round_trip() {
    let b = make_box(&Plane::xy(), 1.0, 2.0, 3.0).unwrap();
    let r = fillet(&b, &b.edges()[..3], 0.2).unwrap().shape;
    let v = r.volume().unwrap();
    let bytes = r.to_brep_bytes().unwrap();
    let r2 = Shape::from_brep_bytes(&bytes).unwrap();
    assert!(close(r2.volume().unwrap(), v, 1e-9));
    assert_eq!(r2.faces().len(), r.faces().len());
    let bin = r.to_bin_bytes(false).unwrap();
    let r3 = Shape::from_bin_bytes(&bin).unwrap();
    assert!(close(r3.volume().unwrap(), v, 1e-9));
    assert!(Shape::from_bin_bytes(b"garbage").is_err());
    assert!(Shape::from_brep_bytes(b"garbage").is_err());
}

#[test]
fn step_round_trip() {
    let dir = std::env::temp_dir().join(format!("noshape-kernel-step-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("parts.step");
    let b = make_box(&Plane::xy(), 10.0, 20.0, 30.0).unwrap();
    let c = make_cylinder(&Plane::new(DVec3::new(50.0, 0.0, 0.0), DVec3::Z, DVec3::X), 5.0, 10.0, 2.0 * PI).unwrap();
    write_step(
        &path,
        &[
            ExportShape { shape: b.clone(), name: Some("Block".into()), color: Some([1.0, 0.0, 0.0]) },
            ExportShape { shape: c.clone(), name: Some("Pin".into()), color: None },
        ],
        LengthUnit::Millimeter,
    )
    .unwrap();
    let parts = read_step(&path, LengthUnit::Millimeter).unwrap();
    assert_eq!(parts.len(), 2);
    let total: f64 = parts.iter().map(|p| p.shape.volume().unwrap()).sum();
    assert!(close(total, 6000.0 + PI * 25.0 * 10.0, 1e-4));
    let block = parts.iter().find(|p| p.name.as_deref() == Some("Block")).expect("name preserved");
    let col = block.color.expect("color preserved");
    assert!(col[0] > 0.9 && col[1] < 0.1);
    // Unit conversion: read back in meters.
    let m = read_step(&path, LengthUnit::Meter).unwrap();
    let tm: f64 = m.iter().map(|p| p.shape.volume().unwrap()).sum();
    assert!(close(tm, (6000.0 + PI * 250.0) * 1e-9, 1e-4));
    // STL / OBJ export
    write_stl(&b, &dir.join("b.stl"), 0.1, StlFormat::Binary).unwrap();
    assert_eq!(std::fs::metadata(dir.join("b.stl")).unwrap().len(), 84 + 50 * 12);
    // Onshape's binary STL header is 80 zero bytes (live G35).
    assert!(std::fs::read(dir.join("b.stl")).unwrap()[..80].iter().all(|&x| x == 0));
    write_stl(&b, &dir.join("b_ascii.stl"), 0.1, StlFormat::Ascii).unwrap();
    write_obj(&c, &dir.join("c.obj"), 0.1).unwrap();
    assert!(read_step(&dir.join("missing.step"), LengthUnit::Millimeter).is_err());
    // IGES round trip.
    write_iges(&dir.join("b.igs"), std::slice::from_ref(&b), LengthUnit::Millimeter).unwrap();
    let ig = read_iges(&dir.join("b.igs"), LengthUnit::Millimeter).unwrap();
    assert!(close(ig.bounding_box(true).unwrap().diagonal(), b.bounding_box(true).unwrap().diagonal(), 1e-4));
    // BREP file round trip.
    write_brep(&c, &dir.join("c.brep")).unwrap();
    assert!(close(read_brep(&dir.join("c.brep")).unwrap().volume().unwrap(), c.volume().unwrap(), 1e-9));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn revolve_and_transform() {
    let plane = Plane::xz();
    let prof = make_face_2d(&plane, &[
        Segment2d::Line { a: DVec2::new(2.0, 0.0), b: DVec2::new(4.0, 0.0) },
        Segment2d::Line { a: DVec2::new(4.0, 0.0), b: DVec2::new(4.0, 3.0) },
        Segment2d::Line { a: DVec2::new(4.0, 3.0), b: DVec2::new(2.0, 3.0) },
        Segment2d::Line { a: DVec2::new(2.0, 3.0), b: DVec2::new(2.0, 0.0) },
    ], &[]).unwrap();
    let full = revolve(&prof, DVec3::ZERO, DVec3::Z, 2.0 * PI).unwrap();
    assert!(close(full.shape.volume().unwrap(), PI * (16.0 - 4.0) * 3.0, 1e-6));
    let half = revolve_between(&prof, DVec3::ZERO, DVec3::Z, -PI / 4.0, PI / 4.0).unwrap();
    assert!(close(half.shape.volume().unwrap(), PI * 12.0 * 3.0 / 4.0, 1e-6));
    assert_eq!(half.history.first_faces().len(), 1);
    assert_eq!(half.history.last_faces().len(), 1);
    let moved = transform(&full.shape, &Transform::Translate(DVec3::new(1.0, 2.0, 3.0)), false).unwrap();
    let c = moved.shape.bounding_box(true).unwrap().center();
    assert!((c - DVec3::new(1.0, 2.0, 4.5)).length() < 1e-6);
    assert_eq!(moved.history.modified(0, ShapeType::Face, 0).len(), 1);
    let m = transform(&full.shape, &Transform::Mirror(Plane::new(DVec3::new(10.0, 0.0, 0.0), DVec3::X, DVec3::Y)), true).unwrap();
    assert!(close(m.shape.volume().unwrap(), full.shape.volume().unwrap(), 1e-9));
    let s = transform(&full.shape, &Transform::ScaleXyz { center: DVec3::ZERO, factors: DVec3::new(1.0, 1.0, 2.0) }, true).unwrap();
    assert!(close(s.shape.volume().unwrap(), 2.0 * full.shape.volume().unwrap(), 1e-4));
    let sc = transform(&full.shape, &Transform::Scale { center: DVec3::ZERO, factor: 2.0 }, true).unwrap();
    assert!(close(sc.shape.volume().unwrap(), 8.0 * full.shape.volume().unwrap(), 1e-6));
}

#[test]
fn shell_solid() {
    let b = make_box(&Plane::xy(), 10.0, 10.0, 10.0).unwrap();
    let top = b.faces().into_iter().find(|f| close(f.centroid().unwrap().z, 10.0, 1e-9)).unwrap();
    let r = shell(&b, &[top], 1.0, ShellSide::Inside, JoinType::Arc).unwrap();
    assert!(close(r.shape.volume().unwrap(), 1000.0 - 8.0 * 8.0 * 9.0, 1e-6));
    let o = shell(&b, &[], 1.0, ShellSide::Inside, JoinType::Arc).unwrap();
    assert!(close(o.shape.volume().unwrap(), 1000.0 - 512.0, 1e-6));
    let o = shell(&b, &[], 1.0, ShellSide::Outside, JoinType::Intersection).unwrap();
    assert!(close(o.shape.volume().unwrap(), 1728.0 - 1000.0, 1e-6));
}
