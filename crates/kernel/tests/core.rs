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
    // A mirror's faces have left-handed plane frames: their normals still point out of the part, and a face
    // and its mirror image parallel to the mirror plane keep one normal.
    let m = b.transformed(&Transform::Mirror(Plane::from_normal(DVec3::new(2.0, 0.0, 0.0), DVec3::X))).unwrap();
    let mid = DVec3::new(3.5, 0.5, 0.5);
    for f in m.faces() {
        let n = f.face_plane().unwrap().unwrap().normal;
        let c = f.centroid().unwrap();
        assert!((c + n * 0.1 - mid).length() > (c - mid).length(), "mirrored face at {c} has normal {n}");
    }
    let top = |s: &Shape| s.faces().into_iter().find(|f| f.centroid().unwrap().z > 0.99).unwrap();
    assert!(close(angle_between(&top(&b), &top(&m)).unwrap(), 0.0, 1e-9));
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
    // Onshape's edges are drawn; the cylinder's seam isn't.
    let shown = s.shown_topology().unwrap();
    for i in 0..s.edges().len() {
        assert_eq!(t.edge_polyline(i).len() >= 2, shown.edges[i], "edge {i}");
    }
    assert_eq!(shown.edges.iter().filter(|&&e| !e).count(), 1);
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

/// qa MASS-INERTIA-APPROX: the mass properties shown are exact on curved
/// parts too (inertia about the centre of mass, density 1): a tube cut and
/// revolved (Ø40 / Ø20 × 30; the revolved one's Ixy read -870 mm⁵ and its
/// centre 5.8e-5 off the axis) and a torus R30 r5.
#[test]
fn mass_properties_of_tubes_and_a_torus_are_exact() {
    let outer = make_cylinder(&Plane::xy(), 20.0, 30.0, 2.0 * PI).unwrap();
    let inner = make_cylinder(&Plane::new(DVec3::new(0.0, 0.0, -1.0), DVec3::Z, DVec3::X), 10.0, 32.0, 2.0 * PI).unwrap();
    let cut = boolean(BooleanOp::Cut, &[outer], &[inner], &BooleanOptions::default()).unwrap().shape;
    let pts = [DVec3::new(10.0, 0.0, 0.0), DVec3::new(20.0, 0.0, 0.0), DVec3::new(20.0, 0.0, 30.0), DVec3::new(10.0, 0.0, 30.0)];
    let face = make_face_from_wire(&make_polyline(&pts, true).unwrap(), true).unwrap();
    let revolved = revolve(&face, DVec3::ZERO, DVec3::Z, 2.0 * PI).unwrap().shape;
    let (r1, r2, h) = (20.0f64, 10.0f64, 30.0f64);
    let m = PI * (r1 * r1 - r2 * r2) * h;
    let tube = (m, DVec3::new(0.0, 0.0, 15.0), m * (3.0 * (r1 * r1 + r2 * r2) + h * h) / 12.0, m * (r1 * r1 + r2 * r2) / 2.0);
    let torus = make_torus(&Plane::xy(), 30.0, 5.0, 2.0 * PI).unwrap();
    let tm = 2.0 * PI * PI * 30.0 * 25.0;
    let ring = (tm, DVec3::ZERO, tm * (4.0 * 900.0 + 5.0 * 25.0) / 8.0, tm * (4.0 * 900.0 + 3.0 * 25.0) / 4.0);
    for (what, s, (mass, c, ixx, izz)) in [("cut tube", &cut, tube), ("revolved tube", &revolved, tube), ("torus", &torus, ring)] {
        let p = mass_properties(s).unwrap();
        let i = p.inertia;
        assert!(close(p.mass, mass, 1e-9 * mass), "{what}: mass {}", p.mass);
        assert!((p.center - c).length() < 1e-9, "{what}: centre {}", p.center);
        assert!(close(i.x_axis.x, ixx, 1e-9 * ixx) && close(i.y_axis.y, ixx, 1e-9 * ixx) && close(i.z_axis.z, izz, 1e-9 * izz), "{what}: {i}");
        assert!(i.y_axis.x.abs() < 1e-8 * ixx && i.z_axis.x.abs() < 1e-8 * ixx && i.z_axis.y.abs() < 1e-8 * ixx, "{what}: {i}");
    }
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

/// Contact of two parts (Boolean's "touching" check): face to face, along an edge, at a corner, a cylinder lying on a
/// face and a ball resting on it (contacts inside both faces), and each moved 1 µm away.
#[test]
fn touching_parts_at_faces_edges_points_and_inside_faces() {
    let mm = 1e-3;
    let at = |x: f64, y: f64, z: f64| Plane::new(DVec3::new(x, y, z) * mm, DVec3::Z, DVec3::X);
    let a = make_box(&at(0.0, 0.0, 0.0), 10.0 * mm, 10.0 * mm, 10.0 * mm).unwrap();
    let cube = |x: f64, y: f64, z: f64| make_box(&at(x, y, z), 10.0 * mm, 10.0 * mm, 10.0 * mm).unwrap();
    let rod = |z: f64| make_cylinder(&Plane::new(DVec3::new(2.0, 5.0, z) * mm, DVec3::X, DVec3::Y), 3.0 * mm, 6.0 * mm, 2.0 * PI).unwrap();
    let ball = |z: f64| make_sphere(&at(5.0, 5.0, z), 3.0 * mm, 2.0 * PI).unwrap();
    let gap = 1e-3;
    for (what, near, apart) in [
        ("face", cube(10.0, 0.0, 0.0), cube(10.0 + gap, 0.0, 0.0)),
        ("edge", cube(10.0, 10.0, 0.0), cube(10.0 + gap, 10.0, 0.0)),
        ("corner", cube(10.0, 10.0, 10.0), cube(10.0, 10.0, 10.0 + gap)),
        ("rod", rod(13.0), rod(13.0 + gap)),
        ("ball", ball(13.0), ball(13.0 + gap)),
    ] {
        assert!(touching(&a, &near, 1e-9).unwrap(), "{what}");
        assert!(touching(&near, &a, 1e-9).unwrap(), "{what}, the other way");
        assert!(!touching(&a, &apart, 1e-9).unwrap(), "{what} 1 µm away");
    }
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
    // Onshape's text layout (live "api (10-25 b)").
    let text = std::fs::read_to_string(dir.join("b_ascii.stl")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!((lines[0], lines[lines.len() - 1], lines.len()), ("solid Mesh", "endsolid Mesh", 2 + 7 * 12), "{text}");
    assert!(lines[1].starts_with("  facet normal ") && lines[2] == "    outer loop" && lines[3].starts_with("      vertex ") && lines[6] == "    endloop" && lines[7] == "  endfacet", "{text}");
    assert!(text.ends_with('\n') && !text.contains("-0 ") && !text.contains("-0\n") && !text.contains('E'), "{text}");
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

/// A crash inside a kernel call fails that call with an error (qa
/// DRAFT-SEGV-DOC: a Draft's null dereference in OCCT killed the app on
/// every open of the document); the process and later calls go on. On
/// Windows by structured exceptions (cpp/seh.cpp, WIN-CRASH-GUARD).
#[test]
fn a_crash_inside_the_kernel_fails_only_that_call() {
    for kind in [0, 1, 2, 0] {
        // Uncaptured: the last line names the kind if the guard misses one and the process dies.
        let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("crash kind {kind}\n").as_bytes());
        let e = crash_inside_kernel(kind).unwrap_err();
        assert!(e.to_string().contains("the kernel crashed"), "{e}");
    }
    let b = make_box_corners(DVec3::ZERO, DVec3::ONE).unwrap();
    assert!((b.volume().unwrap() - 1.0).abs() < 1e-12);
}

/// A blend that never finishes fails after its CPU budget (qa FILLET-HANG:
/// OCCT's blend approximation ran > 1 h): here a loop under a 0.3 s budget.
#[cfg(target_os = "linux")]
#[test]
fn a_kernel_call_over_its_budget_fails() {
    let t = std::time::Instant::now();
    let e = crash_inside_kernel(3).unwrap_err();
    assert!(e.to_string().contains("took too long"), "{e}");
    assert!(t.elapsed().as_secs_f64() < 5.0);
    assert!((make_box_corners(DVec3::ZERO, DVec3::ONE).unwrap().volume().unwrap() - 1.0).abs() < 1e-12);
}

/// A budget never ends a call inside malloc / free (qa BUDGET-LONGJMP: a jump
/// out of the C library left its heap locked, the next free() waited forever):
/// ten 50 ms budgets over a loop that does nothing but allocate, then the heap
/// still works.
/// qa BOP-PAIRS-GPF: a CPU budget that runs out while the evaluating thread
/// takes part in an OCCT thread pool loop stops the work after the loop
/// (every item done), not inside it (the workers would go on using the
/// abandoned stack frames).
#[cfg(target_os = "linux")]
#[test]
fn a_budget_never_stops_a_call_inside_an_occt_thread_pool_loop() {
    // Without the hold (OCCT patch 0005's pool depth) the process died or the
    // loop stopped part way, here, every time.
    for _ in 0..3 {
        let e = crash_inside_kernel(5).unwrap_err().to_string();
        assert!(e.contains("budget 1 after 4096 of 4096"), "{e}");
    }
}

/// ... but a pool job of the evaluating thread alone (OCCT's serial loops go
/// through the pool launcher too) is still stopped inside: the first patch
/// 0005 counted those as well, and budgeted drafts / shells ran on for minutes
/// (qa DRAFT-BUDGET-POOL).
#[cfg(target_os = "linux")]
#[test]
fn a_budget_still_stops_a_pool_job_of_its_own_thread() {
    let t = std::time::Instant::now();
    let e = crash_inside_kernel(7).unwrap_err().to_string();
    assert!(e.contains("budget 1 after 0 of 4"), "{e}");
    assert!(t.elapsed().as_secs_f64() < 5.0, "{:?}", t.elapsed());
}

/// qa CLI-SUPPRESS-SEGV: a fault inside a budgeted op is caught by the
/// crash guard past the budget's frame; the budget must end with it (its jump
/// buffer, its timer), so the next budgeted op gets its own budget and no
/// tick jumps into the dead frame (the CLI died on the second fault).
#[cfg(target_os = "linux")]
#[test]
fn a_fault_inside_a_budget_leaves_no_budget_behind() {
    for _ in 0..3 {
        assert!(crash_inside_kernel(8).is_err());
        let t = std::time::Instant::now();
        let e = crash_inside_kernel(3).unwrap_err().to_string();
        assert!(e.contains("spin took too long"), "{e}");
        assert!(t.elapsed().as_secs_f64() < 10.0, "{:?}", t.elapsed());
    }
}

/// CRASH-1388: a fault recovered past a budget left that budget's deadline (on its dead frame) as the thread's, so a
/// later op's deadline check wrote into the stack there (a fillet's fault, then "stack smashing detected" in a sheet
/// metal extrude's prism); the guard puts the deadline back.
#[cfg(not(windows))]
#[test]
fn a_fault_inside_a_budget_leaves_no_deadline_behind() {
    assert!(crash_inside_kernel(8).is_err());
    let e = crash_inside_kernel(14).unwrap_err().to_string();
    assert!(e.contains("no deadline"), "{e}");
    let e = crash_inside_kernel(13).unwrap_err().to_string();
    assert!(e.contains("fault, deadline restored"), "{e}");
}

#[cfg(target_os = "linux")]
#[test]
fn a_budget_never_stops_a_call_inside_the_allocator() {
    let t = std::time::Instant::now();
    let e = crash_inside_kernel(4).unwrap_err();
    assert!(e.to_string().contains("allocation took too long"), "{e}");
    assert!(t.elapsed().as_secs_f64() < 20.0);
    let v: Vec<u64> = (0..1 << 20).collect();
    assert_eq!(v.len(), 1 << 20);
}

/// qa GUARD-SEGV-MALLOC-DEADLOCK: a fault inside the C library (where
/// malloc's arena lock may be held) or while holding a kernel lock isn't
/// recovered: the process dies of it, with a message, instead of freezing at
/// its next allocation or lock. Run in a child process (this test's binary
/// again).
#[cfg(target_os = "linux")]
#[test]
fn a_fault_inside_the_c_library_ends_the_process() {
    if let Ok(kind) = std::env::var("NOSHAPE_TEST_LIBC_FAULT") {
        let _ = crash_inside_kernel(kind.parse().unwrap());
        std::process::exit(0);
    }
    use std::os::unix::process::ExitStatusExt;
    // 9: inside the C library; 10: holding a kernel lock (LockedSection);
    // 11: holding a plain std::mutex (as OCCT's own, counted by the
    // pthread_mutex_lock wrapper).
    // 12: glibc's abort from inside free() (a corrupt chunk).
    for (kind, signal, says) in [(9, 11, "inside the C library"), (10, 11, "holding a lock"), (11, 11, "holding a lock"), (12, 6, "aborted")] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "kernel_core::a_fault_inside_the_c_library_ends_the_process", "--nocapture", "--test-threads", "1"])
            .env("NOSHAPE_TEST_LIBC_FAULT", kind.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // Dies at once, never hangs (a recovery could wait forever on malloc's lock).
        let t = std::time::Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            if t.elapsed().as_secs() > 60 {
                let _ = child.kill();
                panic!("{kind}: the child hung");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let mut err = String::new();
        std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut err).unwrap();
        assert_eq!(status.signal(), Some(signal), "{kind}: {status:?}: {err}");
        assert!(err.contains(says), "{kind}: {err}");
    }
    // Faults in our own code are still recovered.
    assert!(crash_inside_kernel(1).is_err());
}

/// Onshape's STEP of the G24 corner-blend part (dx I1): its four blends are
/// rational B-spline patches closing to a point, with no pcurves in the
/// file. OCCT's projection drifted near the point and each blend lost a
/// sliver (13.18188 mm²); live gives 13.20006-13.20009 and 7582.977 mm³.
#[test]
fn step_blends_closing_to_a_point_keep_their_area() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/step/g25-b.step");
    let parts = read_step(&path, LengthUnit::Millimeter).unwrap();
    assert_eq!(parts.len(), 1);
    let s = &parts[0].shape;
    assert!(s.is_valid());
    let blends: Vec<f64> = s.faces().iter().map(|f| surface_properties(f).unwrap().mass).filter(|a| (a - 13.2).abs() < 0.05).collect();
    assert_eq!(blends.len(), 4, "{blends:?}");
    assert!(blends.iter().all(|a| (a - 13.20007).abs() < 3e-5), "{blends:?}");
    let v = s.volume().unwrap();
    assert!((v - 7582.977).abs() < 2e-3, "{v}");
}

/// Every edge is handed out FORWARD (Parasolid's edge direction, live "fs (10-26 b)"): its start and end are its
/// curve at its first and last parameters, whichever way the first face using it ran (a box's and a filleted
/// cylinder's edges; stored reversed, `start` was the curve's end while the parameters and tangents were not).
#[test]
fn edges_run_their_curves_way() {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(0.04, 0.02, 0.01)).unwrap();
    let c = make_cylinder(&Plane::xy(), 0.01, 0.02, std::f64::consts::TAU).unwrap();
    let rims: Vec<Shape> = c.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| matches!(i.curve, Curve::Circle { .. }))).collect();
    let f = fillet(&c, &rims[..1], 0.002).unwrap().shape;
    for s in [b, f] {
        for (i, e) in s.edges().iter().enumerate() {
            let info = e.edge_info().unwrap();
            if info.degenerated {
                continue;
            }
            let (p0, p1) = (e.point_at(info.first_param).unwrap(), e.point_at(info.last_param).unwrap());
            assert!((p0 - info.start).length() < 1e-9 && (p1 - info.end).length() < 1e-9, "edge {i}: {:?} .. {:?} vs {p0:?} .. {p1:?}", info.start, info.end);
        }
    }
}

/// Mass properties › Show calculation variance (skasm audit G31): every value gets an error bound from a second,
/// coarser integration. A block's (exact) and a holed cylinder's (OCCT's adaptive integrator) are tiny next to the
/// values; a cut by a tilted cylinder (the segmented Gauss path's B-spline-free mix) too; none negative.
#[test]
fn mass_property_bounds_are_small_and_cover_every_value() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(0.06, 0.04, 0.02)).unwrap();
    let rod = make_cylinder(&Plane::new(DVec3::new(0.03, 0.02, -0.01), DVec3::new(0.3, 0.0, 1.0).normalize(), DVec3::Y), 0.005, 0.05, std::f64::consts::TAU).unwrap();
    let holed = cut(&block, &rod).unwrap().shape;
    for (name, s) in [("block", &block), ("holed", &holed)] {
        for kind in [PropsKind::Volume, PropsKind::Surface] {
            let p = if kind == PropsKind::Volume { mass_properties(s).unwrap() } else { surface_properties(s).unwrap() };
            let b = mass_property_bounds(s, kind).unwrap();
            assert!(b.mass >= 0.0 && b.mass <= 1e-6 * p.mass, "{name} {kind:?}: ±{} of {}", b.mass, p.mass);
            for i in 0..3 {
                assert!(b.center[i] >= 0.0 && b.center[i] <= 1e-6 * 0.08, "{name} {kind:?}: centre ±{:?}", b.center);
            }
            let imax = p.inertia.to_cols_array().iter().fold(0.0f64, |m, v| m.max(v.abs()));
            for v in b.inertia.to_cols_array() {
                assert!(v > 0.0 && v <= 1e-6 * imax, "{name} {kind:?}: inertia ±{v} of {imax}");
            }
        }
    }
    let edges = block.edges();
    let b = mass_property_bounds(&edges[0], PropsKind::Linear).unwrap();
    assert!(b.mass >= 0.0 && b.mass < 1e-12, "a line's length ±{}", b.mass);
}

/// CUT-CLOSED-EDGES-SPLIT (dx's bushing2-0-joshua, live 5/6/0: each loop of the hole one closed edge): a tube r37.5 /
/// r25 along y, its seam at +z, an r5 hole down z through its top wall; noshape's Boolean and OCCT's.
#[test]
fn a_hole_across_a_tubes_seam_leaves_whole_loops() {
    let mm = 1e-3;
    let frame = Plane::new(glam::DVec3::new(0.0, -12.5 * mm, 0.0), glam::DVec3::Y, glam::DVec3::Z);
    let tube = cut(&make_cylinder(&frame, 37.5 * mm, 25.0 * mm, 2.0 * PI).unwrap(), &make_cylinder(&frame, 25.0 * mm, 25.0 * mm, 2.0 * PI).unwrap()).unwrap().shape;
    let hole = make_cylinder(&Plane::new(glam::DVec3::ZERO, glam::DVec3::Z, glam::DVec3::X), 5.0 * mm, 50.0 * mm, 2.0 * PI).unwrap();
    for rust in [true, false] {
        set_rust_boolean(Some(rust));
        let r = unify_same_domain(&cut(&tube, &hole).unwrap().shape).unwrap().shape;
        set_rust_boolean(None);
        assert!(r.is_valid());
        let shown = r.shown_topology().unwrap();
        let counts = (r.faces().len(), shown.edges.iter().filter(|e| **e).count(), shown.vertices.iter().filter(|v| **v).count());
        assert_eq!(counts, (5, 6, 0), "rust {rust}");
        let v = volume_properties(&r).unwrap().mass / (mm * mm * mm);
        assert!((v - 60374.1).abs() < 0.5, "{v}");
    }
}

/// A BREP's counts as Onshape's and its edges (NOSHAPE_BREP_EDGES = the file).
#[test]
#[ignore = "T2 probe: NOSHAPE_BREP_EDGES"]
fn brep_edges_probe() {
    let Ok(path) = std::env::var("NOSHAPE_BREP_EDGES") else { return };
    let mm = 1e-3;
    let mut r = read_brep(std::path::Path::new(&path)).unwrap();
    if std::env::var_os("NOSHAPE_BREP_UNIFY").is_some() {
        r = unify_same_domain_whole(&r).unwrap().shape;
    }
    // (NOSHAPE_BREP_JOIN: as a blend's result is: seams moved off loops, loop pieces joined.)
    if std::env::var_os("NOSHAPE_BREP_JOIN").is_some() {
        r = noshape_kernel::join_edges(&r).unwrap().shape;
    }
    let shown = r.shown_topology().unwrap();
    println!("{} / {} / {}", r.faces().len(), shown.edges.iter().filter(|e| **e).count(), shown.vertices.iter().filter(|v| **v).count());
    for (i, f) in r.faces().iter().enumerate() {
        let mm = 1e-3;
        println!("F{i} area {:.4} at {:.3?} {}", f.area().unwrap_or(0.0) / (mm * mm), f.centroid().unwrap_or_default() / mm, format!("{:?}", f.face_surface().ok()).chars().take(110).collect::<String>());
    }
    for (i, e) in r.edges().iter().enumerate() {
        let x = e.edge_info().unwrap();
        println!("  E{i} shown {} {} {:.3?} -> {:.3?} len {:.4}", shown.edges[i], format!("{:?}", x.curve).chars().take(10).collect::<String>(), x.start / mm, x.end / mm, x.length / mm);
    }
    // Shown vertices where just two shown edges of the same two faces meet (Parasolid would have one edge there).
    let ve = r.vertex_edges().unwrap();
    let ef = r.edge_faces().unwrap();
    for (v, es) in ve.iter().enumerate() {
        let es: Vec<usize> = es.iter().map(|&e| e as usize).filter(|&e| shown.edges[e]).collect();
        if !shown.vertices[v] || es.len() != 2 {
            continue;
        }
        let (mut a, mut b) = (ef[es[0]].clone(), ef[es[1]].clone());
        a.sort();
        b.sort();
        if a == b {
            println!("  joint V{v} between E{} and E{} (faces {a:?})", es[0], es[1]);
        }
    }
}

/// A face's open wire (a slit: one straight edge inside a face, as a thickened seam leaves after a unify) is dropped;
/// its closed wires stay, and a shape without one comes back as it is.
#[test]
fn open_wires_are_dropped() {
    let pl = Plane::xy();
    let outer = make_wire(&make_edges_2d(&pl, &rect(0.04, 0.02)).unwrap()).unwrap();
    let slit = make_wire(&make_edges_2d(&pl, &[Segment2d::Line { a: glam::DVec2::new(0.01, 0.01), b: glam::DVec2::new(0.012, 0.01) }]).unwrap()).unwrap();
    let face = make_face(&pl, &outer, &[slit]).unwrap();
    assert_eq!(face.sub_shapes(ShapeType::Wire).len(), 2);
    let clean = drop_open_wires(&face).unwrap();
    assert_eq!(clean.sub_shapes(ShapeType::Wire).len(), 1);
    assert!(clean.is_valid() && close(clean.area().unwrap(), 0.04 * 0.02, 1e-15));
    let plain = make_face(&pl, &outer, &[]).unwrap();
    assert!(drop_open_wires(&plain).unwrap().is_same(&plain));
}

/// A dense B-spline face in patches over its knot spans: the patches cover it (their areas add up to its own), and a
/// face of few spans comes back whole.
#[test]
fn a_dense_face_comes_in_patches() {
    let n = 40;
    let knots: Vec<f64> = (0..n + 4).map(|i| ((i as f64 - 3.0) / (n as f64 - 3.0)).clamp(0.0, 1.0)).collect();
    let poles: Vec<glam::DVec3> = (0..n * n).map(|k| glam::DVec3::new((k / n) as f64, (k % n) as f64, 0.2 * ((k / n) as f64 * 0.6).sin()) * 1e-3).collect();
    let data = BSplineSurfaceData { poles, nu: n, nv: n, weights: None, u_knots: knots.clone(), v_knots: knots, u_degree: 3, v_degree: 3, u_periodic: false, v_periodic: false };
    let face = make_bspline_surface(&data, &[], false).unwrap().faces().remove(0);
    let patches = face_patches(&face, 10).unwrap();
    assert_eq!(patches.faces().len(), 16);
    let sum: f64 = patches.faces().iter().map(|f| f.area().unwrap()).sum();
    assert!(close(sum, face.area().unwrap(), 1e-4 * face.area().unwrap()), "{sum} vs {}", face.area().unwrap());
    assert_eq!(face_patches(&face, 64).unwrap().faces().len(), 1);
}

/// with_deadline: an operation past its deadline stops (fails) and says so; one within it runs as usual.
#[test]
fn a_deadline_stops_a_long_operation() {
    let plate = make_box_corners(glam::DVec3::ZERO, glam::DVec3::new(0.2, 0.2, 0.002)).unwrap();
    let holes: Vec<Shape> = (0..900).map(|k| make_cylinder(&Plane::from_normal(glam::DVec3::new(0.005 + 0.0065 * (k / 30) as f64, 0.005 + 0.0065 * (k % 30) as f64, -0.001), glam::DVec3::Z), 0.002, 0.004, 2.0 * PI).unwrap()).collect();
    let tools = Shape::compound(&holes);
    // (No time: stopped at the cut's first check whatever its path and the machine's speed (a 10 ms deadline let a fast
    // machine finish it, the watcher's first look being at 50 ms).)
    let (r, late) = with_deadline(0.0, || cut(&plate, &tools));
    assert!(late && r.is_err(), "{:?}", r.map(|x| x.shape.faces().len()));
    let (r, late) = with_deadline(60.0, || cut(&plate, &make_box_corners(glam::DVec3::ZERO, glam::DVec3::splat(0.01)).unwrap()));
    assert!(!late && r.is_ok());
}

/// bool / lead P0 (11-06): an offset face (tests/data/budget, from draft-pool-hang.noshape's Shell 20) evaluated at a
/// singular point far outside its parameters, where Shell's Boolean asked for it (PointNearEdge). OCCT's retry,
/// stepping towards the surface's centre, moved by less than the numbers' spacing at 1.4e9 and never ended: only
/// Linux's CPU timer stopped it, and the document never opened on macOS. Patch 0023: the evaluation fails at once.
#[test]
fn an_offset_face_far_outside_its_parameters_fails_at_once() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/budget/offset-face-far-singular-point.brep");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let face = read_brep(&path).unwrap();
        let _ = tx.send(face.point_at_uv(-1433470578.9116597, 90_437_419.388_804_3).is_err());
    });
    let failed = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the evaluation never ended");
    assert!(failed, "a point at a singular parameter far outside the face");
}

/// A face's template as a displacement map makes it: the face's points at its normalized parameters (an n × n grid,
/// std evFaceTangentPlanes) moved `d` along its normals, as a degree-3 B-spline's poles.
fn moved_face(face: &Shape, n: usize, d: f64) -> Shape {
    let [u0, u1, v0, v1] = face.uv_bounds().unwrap();
    let mut poles = Vec::with_capacity(n * n);
    for i in 0..n {
        for j in 0..n {
            let (u, v) = (u0 + (u1 - u0) * i as f64 / (n - 1) as f64, v0 + (v1 - v0) * j as f64 / (n - 1) as f64);
            poles.push(face.point_at_uv(u, v).unwrap() + face.normal_at_uv(u, v).unwrap() * d);
        }
    }
    let knots: Vec<f64> = (0..n + 4).map(|i| ((i as f64 - 3.0) / (n as f64 - 3.0)).clamp(0.0, 1.0)).collect();
    let data = BSplineSurfaceData { poles, nu: n, nv: n, weights: None, u_knots: knots.clone(), v_knots: knots, u_degree: 3, v_degree: 3, u_periodic: false, v_periodic: false };
    make_bspline_surface(&data, &[], false).unwrap().faces().remove(0)
}

/// DISPLACEMENT-MAP: a face replaced by a template built over its own parameters takes the face's boundary carried
/// onto the template, its neighbours the new edges (no intersection with the template): a block's top 1 mm up gives
/// exactly 40 × 20 × 11 mm, a cylinder's round top exactly π 10² 21 mm (the wall's seam drawn to the moved vertex),
/// each with the faces it had. A template that isn't such a surface (a tilted one) is refused.
#[test]
fn a_face_is_replaced_by_its_moved_template() {
    let mm = 1e-3;
    let top_of = |s: &Shape, z: f64| s.faces().into_iter().find(|f| f.is_planar() && (f.centroid().unwrap().z - z).abs() < 1e-9).unwrap();
    let block = make_box_corners(glam::DVec3::ZERO, glam::DVec3::new(40.0, 20.0, 10.0) * mm).unwrap();
    let top = top_of(&block, 10.0 * mm);
    let out = replace_face_mapped(&block, &top, &moved_face(&top, 12, mm)).unwrap();
    assert!(out.is_valid() && out.faces().len() == 6);
    // (Within the volume integral's own accuracy over a B-spline face.)
    let want = 40.0 * 20.0 * 11.0 * mm * mm * mm;
    assert!(close(out.volume().unwrap(), want, 1e-6 * want), "{}", out.volume().unwrap() / (mm * mm * mm));
    let cyl = make_cylinder(&Plane::xy(), 10.0 * mm, 20.0 * mm, 2.0 * PI).unwrap();
    let top = top_of(&cyl, 20.0 * mm);
    let out = replace_face_mapped(&cyl, &top, &moved_face(&top, 12, mm)).unwrap();
    assert!(out.is_valid() && out.faces().len() == 3);
    let want = PI * 100.0 * 21.0 * mm * mm * mm;
    assert!(close(out.volume().unwrap(), want, 1e-6 * want), "{} vs {}", out.volume().unwrap() / (mm * mm * mm), want / (mm * mm * mm));
    // Tilted: not the face moved along its normals.
    let tilted = transform(&moved_face(&top_of(&block, 10.0 * mm), 12, mm), &Transform::Rotate { origin: glam::DVec3::ZERO, axis: glam::DVec3::X, angle: 0.3 }, true).unwrap().shape;
    assert!(replace_face_mapped(&block, &top_of(&block, 10.0 * mm), &tilted).is_err());
}

/// A wire of edges whose ends meet within the tolerance but not bit for bit (a sketch profile's junctions: an arc's
/// ends as OCCT computes them, the lines' as the sketch does) is the same wire in every process: OCCT's list form
/// merged each junction's vertices in an order hashed by address, so the junctions moved in their last bits from run
/// to run (cf-lighten's Lighten healed 7 faces or 8 after). Each child process builds it and prints its vertices' bits.
#[test]
fn a_wire_of_nearly_meeting_edges_is_the_same_every_run() {
    if std::env::var_os("NOSHAPE_WIRE_BITS_CHILD").is_some() {
        // (cf-lighten's Sketch 1 region its Extrude 1 profile came from.)
        let segs = [
            Segment2d::Arc { center: DVec2::ZERO, radius: 0.01905, start_angle: 1.3316034036979192, end_angle: 4.670710247962112 },
            Segment2d::Arc { center: DVec2::new(0.06469062499999997, 0.2652761307338255), radius: 0.25400000000000056, start_angle: -1.8099892498918742, end_angle: -1.7461836980808452 },
            Segment2d::Arc { center: DVec2::ZERO, radius: 0.025400000000000002, start_angle: 0.6401951382622604, end_angle: 5.393444495775527 },
            Segment2d::Line { a: DVec2::new(0.015992180704553008, -0.019733478059199863), b: DVec2::new(-0.0007937500000000193, -0.019033456358147356) },
        ];
        let w = make_wire_2d(&Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), &segs).unwrap();
        let bits: Vec<String> = w.vertices().iter().map(|v| v.vertex_point().unwrap()).map(|p| format!("{:x}/{:x}/{:x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())).collect();
        println!("WIREBITS {}", bits.join(" "));
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let runs: Vec<String> = (0..6)
        .map(|_| {
            let out = std::process::Command::new(&exe).args(["--exact", "kernel_core::a_wire_of_nearly_meeting_edges_is_the_same_every_run", "--nocapture", "--test-threads=1"]).env("NOSHAPE_WIRE_BITS_CHILD", "1").output().unwrap();
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            text.lines().find_map(|l| l.split_once("WIREBITS ").map(|(_, b)| b.to_string())).unwrap_or_else(|| format!("no wire: {text} {}", String::from_utf8_lossy(&out.stderr)))
        })
        .collect();
    assert!(!runs[0].starts_with("no wire"), "{}", runs[0]);
    assert!(runs.iter().all(|r| *r == runs[0]), "{runs:#?}");
}
