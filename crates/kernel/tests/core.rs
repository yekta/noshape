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

/// The quick centre (a split piece's place) is within a hair of the exact one: a box and a tube cut through by a
/// tilted B-spline sheet, both pieces.
#[test]
fn a_solids_quick_centre_is_the_exact_ones() {
    let tube = make_cylinder(&Plane::xy(), 20.0, 30.0, 2.0 * PI).unwrap();
    let bore = make_cylinder(&Plane::new(DVec3::new(0.0, 0.0, -1.0), DVec3::Z, DVec3::X), 10.0, 32.0, 2.0 * PI).unwrap();
    let tube = boolean(BooleanOp::Cut, &[tube], &[bore], &BooleanOptions::default()).unwrap().shape;
    let bx = make_box(&Plane::new(DVec3::new(5.0, -3.0, 2.0), DVec3::Z, DVec3::X), 2.0, 4.0, 6.0).unwrap();
    let wave = |y: f64, lift: f64| {
        let pts: Vec<DVec3> = (0..4).map(|i| DVec3::new(-30.0 + 20.0 * i as f64, y, 12.0 + lift * (i % 2) as f64)).collect();
        make_wire(&[make_bspline(&pts, None, &[0.0, 1.0], &[4, 4], 3, false).unwrap()]).unwrap()
    };
    let wavy = loft(&[wave(-30.0, 2.0), wave(30.0, -3.0)], &LoftOptions::default()).unwrap().shape;
    let pieces = split(std::slice::from_ref(&tube), &[wavy], &BooleanOptions::default()).unwrap().shape.solids();
    assert_eq!(pieces.len(), 2);
    for (what, s) in [("box", &bx), ("tube", &tube), ("piece 1", &pieces[0]), ("piece 2", &pieces[1])] {
        let exact = volume_properties(s).unwrap().center;
        let quick = volume_centre_estimate(s).unwrap();
        assert!((quick - exact).length() < 1e-6, "{what}: {quick} vs {exact}");
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

/// A face's area by its boundary's line integral (a plane's, cylinder's, cone's, sphere's or torus's, a surface of
/// revolution's, an offset of one): the primitives' closed forms, and a rounded box's offset faces as the surface
/// properties' integral.
#[test]
fn face_areas_by_their_boundary() {
    let sum = |s: &Shape| s.faces().iter().map(|f| f.area().unwrap()).sum::<f64>();
    let ball = make_sphere(&Plane::xy(), 2.0, 2.0 * PI).unwrap();
    assert!(close(sum(&ball), 4.0 * PI * 4.0, 1e-12), "{}", sum(&ball));
    let ring = make_torus(&Plane::xy(), 3.0, 1.0, 2.0 * PI).unwrap();
    assert!(close(sum(&ring), 4.0 * PI * PI * 3.0, 1e-12), "{}", sum(&ring));
    let can = make_cylinder(&Plane::xy(), 1.5, 2.0, 2.0 * PI).unwrap();
    assert!(close(sum(&can), 2.0 * PI * 1.5 * 2.0 + 2.0 * PI * 1.5 * 1.5, 1e-12), "{}", sum(&can));
    // (A cone frustum's side: pi (r1 + r2) times its slant.)
    let cone = make_cone(&Plane::xy(), 2.0, 1.0, 3.0, 2.0 * PI).unwrap();
    let side = PI * 3.0 * (1.0f64 + 9.0).sqrt();
    assert!(close(sum(&cone), side + PI * 4.0 + PI, 1e-12), "{}", sum(&cone));
    let b = make_box(&Plane::xy(), 4.0, 3.0, 2.0).unwrap();
    let rounded = fillet(&b, &b.edges(), 0.4).unwrap().shape;
    let grown = offset_shape(&rounded, 0.25, JoinType::Arc).unwrap().shape;
    for s in [&rounded, &grown] {
        for f in s.faces() {
            let (a, p) = (f.area().unwrap(), surface_properties(&f).unwrap().mass);
            assert!(close(a, p, 1e-9), "{a} against {p}: {:?}", f.face_surface().map(|x| std::mem::discriminant(&x)));
        }
    }
}

/// A cylinder split by circles round it (c2-hunting-knife's Split 4: rings round its handle, each closed on the handle
/// face's seam): its side in three bands of a third of its area each, a circle's vertex on the seam or not; the
/// circles' edges and vertices Modified (a crossing of the seam Generated by both edges), as OCCT's splitter gives them.
#[test]
fn a_cylinder_split_by_circles_round_it() {
    let (r, h) = (0.01, 0.03);
    let rings: Vec<Shape> = [0.01, 0.02].iter().map(|&z| make_circle(DVec3::new(0.0, 0.0, z), DVec3::Z, r).unwrap()).collect();
    let start = rings[0].vertices()[0].vertex_point().unwrap();
    let at = DVec3::new(start.x, start.y, 0.0).normalize();
    for seam in [at, -at, DVec3::new(at.y, -at.x, 0.0)] {
        let cyl = make_cylinder(&Plane::new(DVec3::ZERO, DVec3::Z, seam), r, h, 2.0 * PI).unwrap();
        let out = split(std::slice::from_ref(&cyl), &rings, &BooleanOptions::default()).unwrap();
        let s = &out.shape;
        assert!(s.is_valid());
        assert_eq!((s.solids().len(), s.faces().len()), (1, 5), "seam {seam}");
        let v = s.volume().unwrap();
        assert!((v - PI * r * r * h).abs() <= 1e-9 * v, "volume {v}");
        let bands: Vec<f64> = s.faces().iter().filter(|f| !f.is_planar()).map(|f| f.area().unwrap()).collect();
        assert_eq!(bands.len(), 3, "seam {seam}");
        for a in bands {
            assert!((a - 2.0 * PI * r * h / 3.0).abs() <= 1e-9 * a, "band {a} with the seam at {seam}");
        }
        for arg in [1, 2] {
            for kind in [ShapeType::Edge, ShapeType::Vertex] {
                assert!(out.history.of_input(arg, kind, 0).all(|x| matches!(x.relation, Relation::Modified | Relation::Generated)), "ring {arg}'s {kind:?}: {:?}", out.history.of_input(arg, kind, 0).collect::<Vec<_>>());
                assert!(!out.history.modified(arg, kind, 0).is_empty(), "ring {arg}'s {kind:?}");
            }
        }
    }
}

/// Measure's minimum distance between two parts (part_distance: boxes, coarse meshes, exact pairs) as BRepExtrema's
/// whole-shape one: face to face, edge to edge, apart, a cylinder lying near a box, a ball off a box's corner, a ring
/// round a box; the points on their parts and that far apart.
#[test]
fn part_distance_as_brepextrema() {
    let mm = 1e-3;
    let at = |x: f64, y: f64, z: f64| Plane::new(DVec3::new(x, y, z) * mm, DVec3::Z, DVec3::X);
    let cube = |x: f64, y: f64, z: f64| make_box(&at(x, y, z), 10.0 * mm, 10.0 * mm, 10.0 * mm).unwrap();
    let a = cube(0.0, 0.0, 0.0);
    let rod = make_cylinder(&Plane::new(DVec3::new(2.0, 5.0, 14.0) * mm, DVec3::X, DVec3::Y), 3.0 * mm, 6.0 * mm, 2.0 * PI).unwrap();
    let ball = make_sphere(&at(14.0, 14.0, 14.0), 3.0 * mm, 2.0 * PI).unwrap();
    let ring = make_torus(&at(5.0, 5.0, 5.0), 12.0 * mm, 1.5 * mm, 2.0 * PI).unwrap();
    for (what, b) in [("face", cube(10.0, 2.0, 0.0)), ("edge", cube(10.0, 10.0, 3.0)), ("apart", cube(13.0, 4.0, 2.0)), ("rod", rod), ("ball", ball), ("ring", ring)] {
        let want = distance(&a, &b).unwrap();
        let got = part_distance(&a, &b).unwrap();
        assert!((got.distance - want.distance).abs() <= 1e-9, "{what}: {} against {}", got.distance, want.distance);
        let on = |p: DVec3, s: &Shape| distance(&make_vertex(p).unwrap(), s).unwrap().distance;
        assert!(on(got.point_a, &a) <= 1e-9 && on(got.point_b, &b) <= 1e-9, "{what}: points off their parts");
        assert!(((got.point_b - got.point_a).length() - got.distance).abs() <= 1e-9, "{what}: points not that far apart");
    }
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

/// A STEP part whose cylinder comes as two bands with their seams half a turn apart, each ended by a round (a torus,
/// not a plane: c2-gamma-imported's PEM FH4 studs, live one face): read as one cylinder face, each round keeping its own
/// seam's vertex on the circle it meets.
#[test]
fn a_step_cylinder_in_two_bands_between_rounds_reads_as_one_face() {
    let p = |x: f64, y: f64| DVec2::new(x, y);
    let line = |a: DVec2, b: DVec2| Segment2d::Line { a, b };
    let lower = [
        line(p(0.0, -2.0), p(4.0, -2.0)),
        Segment2d::Arc { center: p(4.0, 0.0), radius: 2.0, start_angle: -PI / 2.0, end_angle: 0.0 },
        line(p(6.0, 0.0), p(6.0, 10.0)),
        line(p(6.0, 10.0), p(0.0, 10.0)),
        line(p(0.0, 10.0), p(0.0, -2.0)),
    ];
    let upper = [
        line(p(0.0, 10.0), p(6.0, 10.0)),
        line(p(6.0, 10.0), p(6.0, 20.0)),
        Segment2d::Arc { center: p(4.0, 20.0), radius: 2.0, start_angle: 0.0, end_angle: PI / 2.0 },
        line(p(4.0, 22.0), p(0.0, 22.0)),
        line(p(0.0, 22.0), p(0.0, 10.0)),
    ];
    let solid = |segs: &[Segment2d], start: f64| {
        let f = make_face_2d(&Plane::xy(), segs, &[]).unwrap();
        revolve_between(&f, DVec3::ZERO, DVec3::Y, start, start + 2.0 * PI).unwrap().shape
    };
    let cylinders = |s: &Shape| -> Vec<f64> { s.faces().iter().filter(|x| matches!(x.face_surface(), Ok(Surface::Cylinder { .. }))).map(|x| x.area().unwrap()).collect() };
    let fused = fuse(&solid(&lower, 0.0), &solid(&upper, PI)).unwrap().shape;
    let dir = std::env::temp_dir().join(format!("noshape-kernel-step-bands-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bands.step");
    write_step(&path, &[ExportShape { shape: fused.clone(), name: Some("Stud".into()), color: None }], LengthUnit::Millimeter).unwrap();
    let parts = read_step(&path, LengthUnit::Millimeter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let s = &parts[0].shape;
    let cyl = cylinders(s);
    // The bottom and top discs, the two rounds, one cylinder 12 mm across and 20 mm long.
    assert_eq!(s.faces().len(), 5, "{cyl:?}");
    assert!(cyl.len() == 1 && (cyl[0] - 240.0 * PI).abs() < 1e-6, "{cyl:?}");
    let v = fused.volume().unwrap();
    assert!(s.is_valid() && (s.volume().unwrap() - v).abs() < 1e-6 * v);
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

/// This thread's CPU time (user + system, s): what a CPU budget counts, unlike the wall clock a loaded host stretches
/// (main red 11-12 at load 79 on 8 cores).
#[cfg(target_os = "linux")]
fn thread_cpu_secs() -> f64 {
    let stat = std::fs::read_to_string("/proc/thread-self/stat").unwrap();
    let fields: Vec<&str> = stat[stat.rfind(')').unwrap() + 2..].split(' ').collect();
    // (utime and stime, the stat's fields 14 and 15, in USER_HZ ticks: 100 a second on Linux.)
    (fields[11].parse::<f64>().unwrap() + fields[12].parse::<f64>().unwrap()) / 100.0
}

/// A blend that never finishes fails after its CPU budget (qa FILLET-HANG:
/// OCCT's blend approximation ran > 1 h): here a loop under a 0.3 s budget.
#[cfg(target_os = "linux")]
#[test]
fn a_kernel_call_over_its_budget_fails() {
    let t = thread_cpu_secs();
    let e = crash_inside_kernel(3).unwrap_err();
    assert!(e.to_string().contains("took too long"), "{e}");
    assert!(thread_cpu_secs() - t < 5.0, "{} s of CPU", thread_cpu_secs() - t);
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
    let t = thread_cpu_secs();
    let e = crash_inside_kernel(7).unwrap_err().to_string();
    assert!(e.contains("budget 1 after 0 of 4"), "{e}");
    assert!(thread_cpu_secs() - t < 5.0, "{} s of CPU", thread_cpu_secs() - t);
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
        let t = thread_cpu_secs();
        let e = crash_inside_kernel(3).unwrap_err().to_string();
        assert!(e.contains("spin took too long"), "{e}");
        assert!(thread_cpu_secs() - t < 10.0, "{} s of CPU", thread_cpu_secs() - t);
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

/// The same for a ScopedWork's bound (a Draft's, a defeature's, HLR's): left behind after a recovered fault, every
/// later stop check on the thread counted into the dead frame (the display mesher read garbage indices after it).
#[cfg(not(windows))]
#[test]
fn a_fault_inside_a_work_bound_leaves_no_bound_behind() {
    let e = crash_inside_kernel(15).unwrap_err().to_string();
    assert!(e.contains("fault, work bound restored"), "{e}");
}

#[cfg(target_os = "linux")]
#[test]
fn a_budget_never_stops_a_call_inside_the_allocator() {
    let t = thread_cpu_secs();
    let e = crash_inside_kernel(4).unwrap_err();
    assert!(e.to_string().contains("allocation took too long"), "{e}");
    assert!(thread_cpu_secs() - t < 20.0, "{} s of CPU", thread_cpu_secs() - t);
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
            if t.elapsed().as_secs() > 300 {
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

/// APP-SIGTERM-IGNORED-MAC: a fault (or OCCT's std::terminate) recovered on a thread that blocks SIGTERM, as every
/// thread made from a macOS GCD worker does, leaves the process ending on SIGTERM. The guards' jumps restored their
/// saved mask with macOS's sigprocmask, which sets every thread's (abort() too): the app then ignored SIGTERM, so
/// logout, shutdown and the updater's restart couldn't quit it. Run in a child process (this test's binary again).
#[cfg(unix)]
#[test]
fn a_recovered_fault_leaves_the_process_ending_on_sigterm() {
    const SIGABRT: i32 = 6;
    const SIGTERM: i32 = 15;
    if let Ok(kind) = std::env::var("NOSHAPE_TEST_TERM_AFTER_FAULT") {
        let kind: u8 = kind.parse().unwrap();
        std::thread::spawn(move || {
            block_on_this_thread(&[SIGABRT, SIGTERM]);
            assert!(crash_inside_kernel(kind).is_err());
        })
        .join()
        .unwrap();
        println!("recovered");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    use std::os::unix::process::ExitStatusExt;
    // 1: a raised SIGFPE; 2: an OCCT exception through a noexcept frame (std::terminate).
    for kind in [1, 2] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "kernel_core::a_recovered_fault_leaves_the_process_ending_on_sigterm", "--nocapture", "--test-threads", "1"])
            .env("NOSHAPE_TEST_TERM_AFTER_FAULT", kind.to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut out = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("recovered") {
            line.clear();
            if std::io::BufRead::read_line(&mut out, &mut line).unwrap() == 0 {
                panic!("{kind}: the child ended before its fault was recovered: {:?}", child.wait());
            }
        }
        let term = std::process::Command::new("kill").args(["-TERM", &child.id().to_string()]).status().unwrap();
        assert!(term.success());
        let t = std::time::Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            if t.elapsed().as_secs() > 60 {
                let _ = child.kill();
                panic!("{kind}: the child ignored SIGTERM");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert_eq!(status.signal(), Some(SIGTERM), "{kind}: {status:?}");
    }
}

/// Blocks `signals` on the calling thread only.
#[cfg(unix)]
fn block_on_this_thread(signals: &[i32]) {
    #[cfg(target_os = "macos")]
    type SigSet = u32;
    #[cfg(not(target_os = "macos"))]
    type SigSet = [u64; 16];
    unsafe extern "C" {
        fn pthread_sigmask(how: i32, set: *const SigSet, old: *mut SigSet) -> i32;
    }
    let bits = signals.iter().fold(0u64, |b, s| b | 1 << (s - 1));
    // (SIG_BLOCK: 1 on macOS, 0 on Linux.)
    #[cfg(target_os = "macos")]
    let (how, set): (i32, SigSet) = (1, bits as u32);
    #[cfg(not(target_os = "macos"))]
    let (how, set): (i32, SigSet) = (0, std::array::from_fn(|k| if k == 0 { bits } else { 0 }));
    assert_eq!(unsafe { pthread_sigmask(how, &set, std::ptr::null_mut()) }, 0);
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

/// A box's edges and corners rounded, the rounds deleted (Reapply fillet's first step on a rounded box: cylinders and
/// sphere corners): the box again, its six planes made again where they meet, without OCCT's Defeaturing.
#[test]
fn a_rounded_boxs_rounds_deleted_give_the_box() {
    let b = make_box(&Plane::xy(), 40.0, 30.0, 20.0).unwrap();
    let rounded = fillet(&b, &b.edges(), 3.0).unwrap().shape;
    let rounds: Vec<Shape> = rounded.faces().into_iter().filter(|f| !matches!(f.face_surface(), Ok(Surface::Plane(_)))).collect();
    assert_eq!(rounds.len(), 20);
    let r = delete_faces(&rounded, &rounds).unwrap();
    assert!(r.shape.is_valid());
    assert_eq!(r.shape.faces().len(), 6);
    assert!(close(r.shape.volume().unwrap(), 24000.0, 1e-6), "{}", r.shape.volume().unwrap());
    // Every kept face's image, the rounds deleted.
    for (i, f) in rounded.faces().iter().enumerate() {
        let planar = matches!(f.face_surface(), Ok(Surface::Plane(_)));
        assert_eq!(r.history.modified(0, ShapeType::Face, i as u32).len(), usize::from(planar), "face {i}");
        assert_eq!(r.history.is_deleted(0, ShapeType::Face, i as u32), !planar, "face {i}");
    }
}

/// cf-part-draft's Part Draft 2 input in small: an open box shelled 2 mm, its vertical and bottom edges rounded first
/// (outer rounds 4 mm, the shell's inner ones 2 mm; sphere corners), split down the middle; every round of a half
/// deleted gives the sharp shelled half (40 × 30 × 20 less 36 × 26 × 18, halved).
#[test]
fn a_shelled_rounded_boxs_half_loses_its_rounds() {
    let b = make_box(&Plane::xy(), 40.0, 30.0, 20.0).unwrap();
    let edges: Vec<Shape> = b.edges().into_iter().filter(|e| e.endpoints().is_ok_and(|(p, q)| p.z.max(q.z) < 1e-9 || (p - q).cross(DVec3::Z).length() < 1e-9)).collect();
    assert_eq!(edges.len(), 8);
    let rounded = fillet(&b, &edges, 4.0).unwrap().shape;
    let top: Vec<Shape> = rounded.faces().into_iter().filter(|f| f.face_plane().ok().flatten().is_some_and(|p| (p.origin.z - 20.0).abs() < 1e-9)).collect();
    assert_eq!(top.len(), 1);
    let shelled = shell(&rounded, &top, 2.0, ShellSide::Inside, JoinType::Arc).unwrap().shape;
    let halves = split_by_plane(&shelled, &Plane::from_normal(DVec3::new(20.0, 0.0, 0.0), DVec3::X)).unwrap().shape.solids();
    assert_eq!(halves.len(), 2);
    for half in halves {
        let rounds: Vec<Shape> = half.faces().into_iter().filter(|f| !matches!(f.face_surface(), Ok(Surface::Plane(_)))).collect();
        assert!(rounds.len() >= 6, "{}", rounds.len());
        let r = delete_faces(&half, &rounds).unwrap();
        assert!(r.shape.is_valid());
        assert!(r.shape.faces().iter().all(|f| matches!(f.face_surface(), Ok(Surface::Plane(_)))));
        assert!(close(r.shape.volume().unwrap(), (24000.0 - 36.0 * 26.0 * 18.0) / 2.0, 1e-6), "{}", r.shape.volume().unwrap());
    }
}

/// A mirrored copy shows the faces its original shows: pieces of one periodic surface OCCT split at its seam read as
/// one face in the copy too (each face's surface was copied apart, so the copy's pieces no longer shared it:
/// c2-fan-blade-sweep's Mirror 3, 25 faces for live's 16).
#[test]
fn a_mirrored_copy_shows_its_originals_faces() {
    let shown = |s: &Shape| {
        let t = s.shown_topology().unwrap();
        t.faces_joined.iter().enumerate().filter(|(i, j)| *i == **j).count()
    };
    let cyl = make_cylinder(&Plane::xy(), 10.0, 20.0, 2.0 * PI).unwrap();
    let cutter = make_box_corners(DVec3::new(-20.0, -20.0, -1.0), DVec3::new(-5.0, 20.0, 21.0)).unwrap();
    let part = boolean(BooleanOp::Cut, &[cyl], &[cutter], &BooleanOptions::default()).unwrap().shape;
    let raw = part.faces().len();
    let original = shown(&part);
    assert!(original < raw, "the cut wall across the seam: {raw} faces, {original} shown");
    let m = transform(&part, &Transform::Mirror(Plane::from_normal(DVec3::new(30.0, 0.0, 0.0), DVec3::X)), false).unwrap().shape;
    assert_eq!(shown(&m), original, "raw faces {raw}, shown in the original {original}");
}

/// Faces split at a seam whose curves on them are straight B-splines (a projection's, not lines) show as one face, as
/// with lines (c2-fan-blade-sweep's Boolean 2: the right bolt's offset hole wall showed a face more than the left's).
#[test]
fn faces_split_at_a_seam_whose_curves_are_straight_bsplines_show_as_one() {
    let shown = |s: &Shape| s.shown_topology().unwrap().faces_joined.iter().enumerate().filter(|(i, j)| *i == **j).count();
    let cyl = make_cylinder(&Plane::xy(), 10.0, 20.0, 2.0 * PI).unwrap();
    let cutter = make_box_corners(DVec3::new(-20.0, -20.0, -1.0), DVec3::new(-5.0, 20.0, 21.0)).unwrap();
    let part = boolean(BooleanOp::Cut, &[cyl], &[cutter], &BooleanOptions::default()).unwrap().shape;
    // Each curve on a face along v at u = 0 or 2π written as the same line, a degree 1 B-spline.
    let text = String::from_utf8(part.to_brep_bytes().unwrap()).unwrap();
    let (mut in_2d, mut swapped) = (false, 0);
    let mut out = String::new();
    for l in text.lines() {
        in_2d = (in_2d || l.starts_with("Curve2ds")) && !l.starts_with("Curves");
        let w: Vec<f64> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if in_2d && w.len() == 5 && w[0] == 1.0 && w[3] == 0.0 && w[4].abs() == 1.0 && (w[1].abs() < 1e-12 || (w[1] - 2.0 * PI).abs() < 1e-12) {
            let (u, v, s) = (w[1], w[2], w[4]);
            out += &format!("7 0 0  1 2 2  {u:e} {:e}  {u:e} {:e} \n-100 2 100 2 \n", v - 100.0 * s, v + 100.0 * s);
            swapped += 1;
            continue;
        }
        out += l;
        out.push('\n');
    }
    let respun = Shape::from_brep_bytes(out.as_bytes()).unwrap();
    assert!(swapped >= 2 && respun.faces().len() == part.faces().len(), "{swapped} curves swapped");
    assert!(shown(&part) < part.faces().len());
    assert_eq!(shown(&respun), shown(&part), "{} raw faces", part.faces().len());
}

/// A ring revolved from half a turn round its profile above its axis (its cylinders' parameters running from π: the seam at −z) with two
/// grooves cut either side of −z: the tread between them, split at that seam, is one face as live shows it (infento's
/// tyre, Boolean 2: 392 faces for live's 391).
#[test]
fn a_tread_across_a_seam_at_half_a_turn_shows_one_face() {
    let p = |x: f64, z: f64| DVec3::new(x, 0.0, z);
    let edges = [make_line(p(0.0, 15.0), p(8.0, 15.0)).unwrap(), make_line(p(8.0, 15.0), p(8.0, 20.0)).unwrap(), make_line(p(8.0, 20.0), p(0.0, 20.0)).unwrap(), make_line(p(0.0, 20.0), p(0.0, 15.0)).unwrap()];
    let profile = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    let ring = revolve_between(&profile, DVec3::ZERO, DVec3::X, PI, 3.0 * PI).unwrap().shape;
    let outer = ring.faces().into_iter().find(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { radius, .. }) if (radius - 20.0).abs() < 1e-9)).unwrap();
    assert!((outer.uv_bounds().unwrap()[0] - PI).abs() < 1e-9, "the outer wall's parameters from π: {:?}", outer.uv_bounds());
    let groove = |y: f64| make_box_corners(DVec3::new(-1.0, y - 0.5, -25.0), DVec3::new(9.0, y + 0.5, -18.0)).unwrap();
    let cut = boolean(BooleanOp::Cut, &[ring], &[groove(-3.0), groove(3.0)], &BooleanOptions::default()).unwrap().shape;
    let tread: Vec<Shape> = cut.faces().into_iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { radius, .. }) if (radius - 20.0).abs() < 1e-9)).collect();
    assert_eq!(tread.len(), 3, "the outer wall in three: the rest and the tread's halves");
    assert_eq!(shown_faces(&cut), cut.faces().len() - 1, "{} raw faces", cut.faces().len());
}

/// A 20-turn thread swept round a shaft and fused onto it in one feature (asm-hand-drill-pinslot's Sweep 2, live
/// 28 ms): the sweep's self-intersection check gives up past its CPU second finding nothing, and the feature's budget
/// isn't spent by it, else the fuse after it stopped at once ("BOPAlgo_AlertIntersectionFailed"). In metres.
#[test]
fn a_threads_self_intersection_check_leaves_the_features_budget() {
    let mm = 1e-3;
    let frame = Plane::xy();
    let helix = make_helix(&frame, 4.0 * mm, 1.0 * mm, 20.0 * mm, false, 0.0).unwrap();
    let p = |x: f64, z: f64| DVec3::new(x, 0.0, z) * mm;
    let edges = [make_line(p(3.9, -0.3), p(4.5, 0.0)).unwrap(), make_line(p(4.5, 0.0), p(3.9, 0.3)).unwrap(), make_line(p(3.9, 0.3), p(3.9, -0.3)).unwrap()];
    let profile = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    let thread = sweep(&profile, &helix, &SweepOptions::default()).unwrap().shape;
    let shaft = make_cylinder(&Plane::from_normal(DVec3::new(0.0, 0.0, -1.0) * mm, DVec3::Z), 4.0 * mm, 22.0 * mm, 2.0 * PI).unwrap();
    let ((faulty, fused), spent) = with_budget_scope(|| {
        let faulty = thread.self_intersects();
        (faulty, boolean(BooleanOp::Fuse, std::slice::from_ref(&shaft), std::slice::from_ref(&thread), &BooleanOptions::default()))
    });
    assert!(!faulty && !spent, "self-intersects {faulty}, budget spent {spent}");
    let fused = fused.unwrap().shape;
    assert!(fused.is_valid() && fused.count(ShapeType::Solid) == 1);
    let want = shaft.volume().unwrap() + thread.volume().unwrap();
    assert!(fused.volume().unwrap() > shaft.volume().unwrap() && fused.volume().unwrap() < want, "{} of {want} m³", fused.volume().unwrap());
}

/// A guided loft between two smooth closed profiles (no corners on either: c2-hunting-knife's handle, Loft 1): one side
/// face round the loop through the guides, as live, where ours refused ("Loft guides need a profile with corners yet").
#[test]
fn a_guided_loft_of_smooth_closed_profiles_has_one_side_face() {
    let disc = |z: f64| {
        let w = make_wire(&[make_circle(DVec3::new(0.0, 0.0, z), DVec3::Z, 10.0).unwrap()]).unwrap();
        make_face_from_wire(&w, true).unwrap()
    };
    let guides = [make_line(DVec3::new(10.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 30.0)).unwrap(), make_line(DVec3::new(-10.0, 0.0, 0.0), DVec3::new(-10.0, 0.0, 30.0)).unwrap()];
    let s = loft_guided_curved(&disc(0.0), &disc(30.0), &guides, true).unwrap();
    assert!(s.is_valid());
    let shown = s.shown_topology().unwrap().faces_joined.iter().enumerate().filter(|(i, j)| *i == **j).count();
    assert_eq!(shown, 3, "two caps and one side ({} raw faces)", s.faces().len());
    let v = s.volume().unwrap();
    assert!((v - PI * 100.0 * 30.0).abs() < 1.0, "{v} mm³");
}

/// A surface loft through three circles to a point along two guides (c2-surf-workshop-knifehandle's Loft 1: six profiles
/// to a point between two guides, live one face): one face through every profile and guide, its first circle two edges
/// between the guides' ends, as live's (1/2/3 with the apex), where ours refused more than two profiles.
#[test]
fn a_guided_surface_loft_through_more_profiles_is_one_face() {
    let stations = [(0.0, 10.0), (10.0, 9.0), (20.0, 6.0)];
    let mut profiles: Vec<Shape> = stations.iter().map(|&(z, r)| make_wire(&[make_circle(DVec3::new(0.0, 0.0, z), DVec3::Z, r).unwrap()]).unwrap()).collect();
    let apex = DVec3::new(0.0, 0.0, 30.0);
    profiles.push(make_vertex(apex).unwrap());
    let guides = [1.0, -1.0].map(|side: f64| {
        let mut pts: Vec<DVec3> = stations.iter().map(|&(z, r)| DVec3::new(side * r, 0.0, z)).collect();
        pts.push(apex);
        make_interpolated_spline(&pts, &[], false).unwrap()
    });
    let s = loft_guided_multi(&profiles, &guides).unwrap();
    assert!(s.is_valid());
    let shown = s.shown_topology().unwrap();
    assert_eq!((shown.face_count(), shown.edge_count(), shown.vertex_count()), (1, 2, 3), "faces / edges / vertices shown");
    let face = &s.faces()[0];
    for &(z, r) in &stations {
        for k in 0..12 {
            let a = k as f64 * PI / 6.0;
            let d = face.project_point(DVec3::new(r * a.cos(), r * a.sin(), z)).unwrap().distance;
            assert!(d < 1e-3, "profile at z {z}, angle {a}: {d} mm off the face");
        }
    }
    for g in &guides {
        let i = g.edge_info().unwrap();
        for k in 1..10 {
            let p = g.point_at(i.first_param + (i.last_param - i.first_param) * k as f64 / 10.0).unwrap();
            let d = face.project_point(p).unwrap().distance;
            assert!(d < 1e-3, "guide point {p}: {d} mm off the face");
        }
    }
}

/// A guide off a profile is live's LOFT_FAILED, never "All guides should intersect each profile boundary.": beside a
/// middle circle by 3 mm (loft-stations3 J/K), past, at or 2 mm short of an end circle (loft-guide-past-end-1123 L/M/N).
#[test]
fn a_guided_loft_refuses_a_guide_off_a_profile_as_live() {
    let circle = |x: f64, cz: f64, r: f64| make_wire(&[make_circle(DVec3::new(x, 0.0, cz), DVec3::X, r).unwrap()]).unwrap();
    let guides = |ends: &[f64]| {
        [1.0, -1.0].map(|side: f64| {
            let mut pts = vec![DVec3::new(0.0, 0.0, 10.0 * side), DVec3::new(40.0, 0.0, if side > 0.0 { 18.0 } else { -6.0 })];
            pts.extend(ends.iter().map(|&x| DVec3::new(x, 0.0, 10.0 * side)));
            make_interpolated_spline(&pts, &[], false).unwrap()
        })
    };
    let refused = |profiles: &[Shape], g: &[Shape], case: &str| {
        let e = loft_guided_multi(profiles, g).unwrap_err();
        assert!(matches!(&e, KernelError::OperationFailed(m) if !m.starts_with("All guides")), "{case}: {e:?}");
    };
    let on = guides(&[100.0]);
    assert!(loft_guided_multi(&[circle(0.0, 0.0, 10.0), circle(40.0, 6.0, 12.0), circle(100.0, 0.0, 10.0)], &on).is_ok());
    for (cz, r) in [(6.0, 9.0), (9.0, 12.0)] {
        refused(&[circle(0.0, 0.0, 10.0), circle(40.0, cz, r), circle(100.0, 0.0, 10.0)], &on, &format!("middle ({cz}, {r})"));
    }
    let r7 = [circle(0.0, 0.0, 10.0), circle(40.0, 6.0, 12.0), circle(100.0, 0.0, 7.0)];
    refused(&r7, &guides(&[100.0, 110.0]), "L: past an R 7 end");
    refused(&r7, &on, "M: at an R 7 end");
    refused(&[circle(0.0, 0.0, 10.0), circle(40.0, 6.0, 12.0), circle(100.0, 0.0, 10.0)], &guides(&[98.0]), "N: 2 mm short");
}

/// c2-surf-workshop-knifehandle's Loft 1 itself (its sketches, in metres): P1 two Bézier halves, P2..P6 ellipses, to the
/// tip, between the side profile's two Bézier guides. Live's face 21,477.23 mm²: each profile runs by its own parameter
/// between the guides (P1 by its Bézier halves, not by arc length) and each guide by its own parameter.
#[test]
fn the_knifehandle_loft_through_six_profiles_has_lives_area() {
    let mm = 1e-3;
    let world = |x: f64, sy: f64| DVec3::new(x, 0.0, sy) * mm;
    let bezier = |poles: Vec<DVec3>| {
        let d = poles.len() as u32 - 1;
        make_bspline(&poles, None, &[0.0, 1.0], &[d + 1, d + 1], d, false).unwrap()
    };
    // P1 on x = 13.088534 mm, sketch x along -y: a degree 6 half and its mirror, bottom to top.
    let half = [(0.0, -39.929341001), (13.897868518, -39.929341001), (23.964719847, -18.368937075), (10.491231456, 9.514288045), (13.514378108, 11.358790092), (7.282994688, 11.358790092), (0.0, 11.358790092)];
    let p1 = [1.0, -1.0].map(|side: f64| bezier(half.iter().map(|&(x, y)| DVec3::new(13.088533799892277, side * x, y) * mm).collect()));
    let mut profiles = vec![make_wire(&p1).unwrap()];
    // P2..P6: (x, centre's height, major radius, minor radius, major axis up), major axis vertical.
    let ellipses = [
        (56.76422151249488, -5.396874827, 18.589808577, 18.013450331, false),
        (84.11949756067832, -7.409208184, 21.151688768, 19.936648229, true),
        (117.97999101203521, -7.124904157, 20.671268574, 16.629852814, false),
        (144.16767353635868, -5.876122036, 18.377846988, 15.809816512, false),
        (176.41576262634834, -12.64299533, 21.675825829, 16.068877808, false),
    ];
    for &(x, c, a, b, up) in &ellipses {
        let plane = Plane::new(DVec3::new(x, 0.0, 0.0) * mm, -DVec3::X, -DVec3::Y);
        let e = Segment2d::Ellipse { center: DVec2::new(0.0, c) * mm, major_radius: a * mm, minor_radius: b * mm, rotation: if up { PI / 2.0 } else { -PI / 2.0 } };
        profiles.push(make_wire_2d(&plane, &[e]).unwrap());
    }
    profiles.push(make_vertex(world(188.5745936940105, -0.09705362453543558)).unwrap());
    let bottom = [(188.574593694, -0.097053625), (188.574593694, -23.949988974), (181.720774878, -43.860322652), (164.75575443, -55.288146106), (161.103565503, -32.196872724), (170.175139118, -4.86434097), (135.999543945, -3.809294856), (138.462661973, -33.982364275), (131.055951353, -36.872441924), (78.009917225, -39.019658758), (60.871466283, -53.459747556), (39.724033385, 16.181488802), (34.833628461, -10.011939084), (43.784817478, -43.290136078), (21.982817276, -46.04495312), (0.0, -36.650845871)];
    let top = [(188.574593694, -0.097053625), (188.574593694, 9.330931271), (178.167120712, 10.424711799), (154.79101156, 11.999027016), (127.304970462, 15.763460889), (70.934995102, 14.814262678), (-2.308197176, 10.497376289)];
    let guides = [&bottom[..], &top[..]].map(|g| bezier(g.iter().map(|&(x, y)| world(x, y)).collect()));
    let s = loft_guided_multi(&profiles, &guides).unwrap();
    assert!(s.is_valid());
    assert_eq!(s.faces().len(), 1);
    let area = s.area().unwrap() / (mm * mm);
    assert!(close(area, 21477.23, 2e-3), "{area} mm², live 21,477.23");
}

/// That loft's first circle (its two edges) moved 5 mm on along the face (c2-surf-workshop-knifehandle's Extend handle):
/// still two edges there, their ends carried along with them, 1/2/3 as live, where ours made the new end one edge.
#[test]
fn a_moved_boundary_keeps_its_edges_ends() {
    let stations = [(0.0, 10.0), (10.0, 9.0), (20.0, 6.0)];
    let mut profiles: Vec<Shape> = stations.iter().map(|&(z, r)| make_wire(&[make_circle(DVec3::new(0.0, 0.0, z), DVec3::Z, r).unwrap()]).unwrap()).collect();
    let apex = DVec3::new(0.0, 0.0, 30.0);
    profiles.push(make_vertex(apex).unwrap());
    let guides = [1.0, -1.0].map(|side: f64| {
        let mut pts: Vec<DVec3> = stations.iter().map(|&(z, r)| DVec3::new(side * r, 0.0, z)).collect();
        pts.push(apex);
        make_interpolated_spline(&pts, &[], false).unwrap()
    });
    let s = loft_guided_multi(&profiles, &guides).unwrap();
    let face = s.faces()[0].clone();
    let on_rim = |e: &Shape| e.edge_info().is_ok_and(|i| i.length > 1.0 && e.point_at(0.5 * (i.first_param + i.last_param)).is_ok_and(|p| p.z.abs() < 1e-6));
    let rim: Vec<Shape> = face.edges().into_iter().filter(on_rim).collect();
    assert_eq!(rim.len(), 2, "the first circle's two edges");
    let moved = extend_face_at(&face, &rim, 5.0).unwrap();
    assert!(moved.is_valid());
    let shown = moved.shown_topology().unwrap();
    assert_eq!((shown.face_count(), shown.edge_count(), shown.vertex_count()), (1, 2, 3), "faces / edges / vertices shown");
    assert!(moved.area().unwrap() > face.area().unwrap() + 5.0 * 2.0 * PI * 10.0 * 0.9, "moved on about 5 mm round the rim");
}

/// A circle of two arcs (R12, at z 30) with straight guides at y ±10 → ±12 from the profile below, all times `k`
/// (1e-3: metres, as the app models; its sewing tolerance is 1 µm there).
fn loft_to_two_arcs(bottom: &Shape, k: f64) -> Shape {
    let c = DVec3::new(0.0, 0.0, 30.0) * k;
    let top = [make_arc(c, DVec3::Z, DVec3::X, 12.0 * k, -PI / 2.0, PI / 2.0).unwrap(), make_arc(c, DVec3::Z, DVec3::X, 12.0 * k, PI / 2.0, 3.0 * PI / 2.0).unwrap()];
    let top = make_face_from_wire(&make_wire(&top).unwrap(), true).unwrap();
    let guides = [(10.0, 12.0), (-10.0, -12.0)].map(|(a, b)| make_line(DVec3::new(0.0, a, 0.0) * k, DVec3::new(0.0, b, 30.0) * k).unwrap());
    loft_guided_curved(bottom, &top, &guides, true).unwrap()
}

fn shown_faces(s: &Shape) -> usize {
    s.shown_topology().unwrap().faces_joined.iter().enumerate().filter(|(i, j)| *i == **j).count()
}

/// A guided loft from a stadium (its curvature jumps where a line meets an arc) to a circle of two arcs makes one side
/// face (live 3023: 3 faces) lying on both profiles (one cubic across the loop missed them by tens of µm, and its caps
/// didn't sew: c2-hunting-knife's handle); a pin from inside ends on that face. The stadium's lines are split where the
/// guides meet them, lines still, each piece an edge (live 3070: the handle's 20 edges and 20 vertices). In metres.
#[test]
fn a_guided_loft_of_a_stadium_to_a_circle_has_one_side_face_on_its_profiles() {
    let mm = 1e-3;
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0) * mm;
    let e = [
        make_line(p(-10.0, -10.0), p(10.0, -10.0)).unwrap(),
        make_arc(p(10.0, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, -PI / 2.0, PI / 2.0).unwrap(),
        make_line(p(10.0, 10.0), p(-10.0, 10.0)).unwrap(),
        make_arc(p(-10.0, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, PI / 2.0, 3.0 * PI / 2.0).unwrap(),
    ];
    let s = loft_to_two_arcs(&make_face_from_wire(&make_wire(&e).unwrap(), true).unwrap(), mm);
    assert!(s.is_valid());
    assert_eq!(shown_faces(&s), 3, "two caps and one side ({} raw faces)", s.faces().len());
    let side: Vec<Shape> = s.faces().into_iter().filter(|f| f.face_plane().ok().flatten().is_none()).collect();
    assert_eq!(side.len(), 1);
    let t = s.shown_topology().unwrap();
    let shown: Vec<Shape> = s.edges().into_iter().enumerate().filter(|(i, _)| t.edges[*i] && t.joined[*i] == *i).map(|(_, e)| e).collect();
    let below: Vec<Shape> = shown.iter().filter(|e| e.endpoints().is_ok_and(|(a, b)| a.z.abs() < 1e-9 && b.z.abs() < 1e-9)).cloned().collect();
    assert_eq!((shown.len(), below.len()), (8, 6), "the stadium's 4 edges and 2 splits, the circle's 2 arcs");
    assert_eq!(below.iter().filter(|e| matches!(e.edge_curve(), Ok(Curve::Line { .. }))).count(), 4);
    assert_eq!(t.vertices.iter().filter(|v| **v).count(), 8);
    let pin = make_face_from_wire(&make_wire(&[make_circle(DVec3::new(0.0, 0.0, 15.0) * mm, DVec3::Y, 2.0 * mm).unwrap()]).unwrap(), true).unwrap();
    let r = extrude_up_to(&pin, DVec3::Y, &side[0], 0.0).unwrap();
    let bb = r.shape.bounding_box(true).unwrap();
    assert!((bb.max.y / mm - 11.0).abs() < 0.5, "the pin ends on the side, near y 11 mm: {} mm", bb.max.y / mm);
}

/// A guided solid loft is one closed shell at either scale (in mm its sewing tolerance is 0.1 µm, and an unsewn seam
/// once came back as an open "solid", BRepCheck NotClosed): the stadium to the circle in mm and in metres, valid, the
/// same volume.
#[test]
fn a_guided_solid_loft_is_closed_at_either_scale() {
    let volume = |mm: f64| {
        let p = |x: f64, y: f64| DVec3::new(x, y, 0.0) * mm;
        let e = [
            make_line(p(-10.0, -10.0), p(10.0, -10.0)).unwrap(),
            make_arc(p(10.0, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, -PI / 2.0, PI / 2.0).unwrap(),
            make_line(p(10.0, 10.0), p(-10.0, 10.0)).unwrap(),
            make_arc(p(-10.0, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, PI / 2.0, 3.0 * PI / 2.0).unwrap(),
        ];
        let s = loft_to_two_arcs(&make_face_from_wire(&make_wire(&e).unwrap(), true).unwrap(), mm);
        assert!(s.is_valid() && s.count(ShapeType::Shell) == 1, "{mm}");
        s.volume().unwrap() / mm.powi(3)
    };
    let (a, b) = (volume(1.0), volume(1e-3));
    assert!((a - b).abs() < 1e-6 * a, "{a} / {b} mm³");
}

/// A slot cut into a guided loft's end keeps the vertices where the guides met its profiles (live's handle, Extrude 8:
/// 8/32/28; a unify merged the split lines back).
#[test]
fn a_cut_into_a_guided_loft_keeps_its_guide_vertices() {
    let mm = 1e-3;
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0) * mm;
    let e = [
        make_line(p(-10.0, -10.0), p(10.0, -10.0)).unwrap(),
        make_arc(p(10.0, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, -PI / 2.0, PI / 2.0).unwrap(),
        make_line(p(10.0, 10.0), p(-10.0, 10.0)).unwrap(),
        make_arc(p(-10.0, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, PI / 2.0, 3.0 * PI / 2.0).unwrap(),
    ];
    // The circle above one edge, split by the guides too.
    let top = make_face_from_wire(&make_wire(&[make_circle(DVec3::new(0.0, 0.0, 30.0) * mm, DVec3::Z, 12.0 * mm).unwrap()]).unwrap(), true).unwrap();
    let guides = [(10.0, 12.0), (-10.0, -12.0)].map(|(a, b)| make_line(DVec3::new(0.0, a, 0.0) * mm, DVec3::new(0.0, b, 30.0) * mm).unwrap());
    let s = loft_guided_curved(&make_face_from_wire(&make_wire(&e).unwrap(), true).unwrap(), &top, &guides, true).unwrap();
    let slot = make_box_corners(DVec3::new(-2.0, -5.0, -1.0) * mm, DVec3::new(2.0, 5.0, 15.0) * mm).unwrap();
    let r = boolean(BooleanOp::Cut, &[s], &[slot], &BooleanOptions::default()).unwrap().shape;
    assert!(r.is_valid());
    let t = r.shown_topology().unwrap();
    let shown = r.edges().iter().enumerate().filter(|(i, _)| t.edges[*i] && t.joined[*i] == *i).count();
    assert_eq!((shown, t.vertices.iter().filter(|v| **v).count()), (8 + 12, 8 + 8), "the loft's 8 edges and vertices, the pocket's 12 and 8");
}

/// Live 3023: a profile joint turning by up to 3° is smoothed over (no side edge from it), past 3° it is a corner:
/// regular polygons lofted to a circle, 150 sides (2.4°) one side face, 60 sides (6°) a face per side (in metres).
#[test]
fn a_guided_lofts_profile_joints_under_three_degrees_are_smooth() {
    // Its sides' middles on the guides at y ±10 (a corner there would line up with the circle's joints).
    let polygon = |n: usize| {
        let p = |i: usize| {
            let a = std::f64::consts::TAU * (i as f64 + 0.5) / n as f64;
            DVec3::new(a.sin(), a.cos(), 0.0) * 10e-3 / (PI / n as f64).cos()
        };
        make_face_from_wire(&make_polyline(&(0..n).map(p).collect::<Vec<_>>(), true).unwrap(), true).unwrap()
    };
    for (n, sides) in [(150, 1), (60, 60)] {
        let s = loft_to_two_arcs(&polygon(n), 1e-3);
        assert!(s.is_valid(), "{n} sides");
        assert_eq!(shown_faces(&s), sides + 2, "{n} sides: the caps and {sides} side faces");
    }
}

/// Live 3023 / 3053: joints lined up across a loft are corners however smooth: a 40 × 20 stadium lofted (2 guides) to
/// the same stadium has a side face per segment; to the stadium turned 90° (its joints off the bottom's), one side face.
/// In metres, as the app models (its sewing tolerance is 1 µm there).
#[test]
fn a_guided_lofts_lined_up_joints_are_corners() {
    let mm = 1e-3;
    let stadium = |z: f64, turned: bool| {
        let at = |x: f64, y: f64| if turned { DVec3::new(-y, x, z) * mm } else { DVec3::new(x, y, z) * mm };
        let axis = if turned { DVec3::Y } else { DVec3::X };
        let e = [
            make_line(at(-10.0, -10.0), at(10.0, -10.0)).unwrap(),
            make_arc(at(10.0, 0.0), DVec3::Z, axis, 10.0 * mm, -PI / 2.0, PI / 2.0).unwrap(),
            make_line(at(10.0, 10.0), at(-10.0, 10.0)).unwrap(),
            make_arc(at(-10.0, 0.0), DVec3::Z, axis, 10.0 * mm, PI / 2.0, 3.0 * PI / 2.0).unwrap(),
        ];
        make_face_from_wire(&make_wire(&e).unwrap(), true).unwrap()
    };
    for (turned, top_y, sides) in [(false, 10.0, 4), (true, 20.0, 1)] {
        let guides = [1.0, -1.0].map(|k: f64| make_line(DVec3::new(0.0, 10.0 * k, 0.0) * mm, DVec3::new(0.0, top_y * k, 30.0) * mm).unwrap());
        let s = loft_guided_curved(&stadium(0.0, false), &stadium(30.0, turned), &guides, true).unwrap();
        assert!(s.is_valid(), "turned {turned}");
        assert_eq!(shown_faces(&s), sides + 2, "turned {turned}: the caps and {sides} side faces");
    }
}

/// Live 3070: with as many joints on both profiles between two guides, a guided loft pairs them in order (its
/// sections run joint to joint, not by arc length): a 20 × 20 stadium lofted to a 40 × 20 one along straight guides
/// at its lines' middles passes halfway between their joints (10, -10, 0) and (20, -10, 30). In metres.
#[test]
fn a_guided_loft_pairs_its_profiles_joints() {
    let mm = 1e-3;
    let stadium = |half: f64, z: f64| {
        let at = |x: f64, y: f64| DVec3::new(x, y, z) * mm;
        let e = [
            make_line(at(-half, -10.0), at(half, -10.0)).unwrap(),
            make_arc(at(half, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, -PI / 2.0, PI / 2.0).unwrap(),
            make_line(at(half, 10.0), at(-half, 10.0)).unwrap(),
            make_arc(at(-half, 0.0), DVec3::Z, DVec3::X, 10.0 * mm, PI / 2.0, 3.0 * PI / 2.0).unwrap(),
        ];
        make_face_from_wire(&make_wire(&e).unwrap(), true).unwrap()
    };
    let guides = [1.0, -1.0].map(|k: f64| make_line(DVec3::new(0.0, 10.0 * k, 0.0) * mm, DVec3::new(0.0, 10.0 * k, 30.0) * mm).unwrap());
    let s = loft_guided_curved(&stadium(10.0, 0.0), &stadium(20.0, 30.0), &guides, true).unwrap();
    assert!(s.is_valid());
    let side: Vec<Shape> = s.faces().into_iter().filter(|f| f.face_plane().ok().flatten().is_none()).collect();
    assert_eq!(side.len(), 1);
    let p = make_vertex(DVec3::new(15.0, -10.0, 15.0) * mm).unwrap();
    let d = distance(&p, &side[0]).unwrap().distance;
    assert!(d < 1e-7, "{} mm off the side", d / mm);
}

/// A curve projected along a direction onto a linear extrusion, the projection sweeping a plane square to the
/// extrusion: the section is the extruded curve itself in that plane (c2-hunting-knife's Split 3: live's Loft 3 guide is
/// the sketch's cubic, one Bézier; OCCT's projection gave a degree-5 approximation 1-2 µm off it).
#[test]
fn a_line_projected_onto_an_extruded_cubic_gives_the_cubic() {
    let poles = [DVec3::new(-50.0, 0.0, -19.0), DVec3::new(-80.0, 0.0, -18.6), DVec3::new(-105.0, 0.0, -12.2), DVec3::new(-114.3, 0.0, 3.0)];
    let cubic = BSplineCurveData { poles: poles.to_vec(), weights: None, knots: vec![0.0, 1.0], mults: vec![4, 4], degree: 3, periodic: false };
    let side = extrude(&make_bspline_edge(&cubic).unwrap(), DVec3::new(0.0, 4.0, 0.0)).unwrap().shape;
    let face = side.faces()[0].clone();
    let line = make_line(DVec3::new(-120.0, 1.75, 50.0), DVec3::new(-40.0, 1.75, 50.0)).unwrap();
    let projected = project_curves(&line, &face, Some(DVec3::NEG_Z)).unwrap();
    let edges = projected.edges();
    assert_eq!(edges.len(), 1);
    let got = edges[0].edge_bspline().unwrap();
    assert_eq!((got.degree, got.poles.len()), (3, 4), "one cubic Bézier");
    let mut want: Vec<DVec3> = poles.iter().map(|p| DVec3::new(p.x, 1.75, p.z)).collect();
    if got.poles[0].distance(want[0]) > 1e-6 {
        want.reverse();
    }
    for (g, w) in got.poles.iter().zip(&want) {
        assert!(g.distance(*w) < 1e-9, "{g} against {w}");
    }
}

/// `exit_now` ends the process with its code while another thread is still in the kernel (Booleans in a loop), its
/// stdout written: the app's quit (qa's QUIT-ABORT-DOCK, where exit()'s static destructors ran under such a thread).
/// The test runs itself as the child.
#[test]
fn exit_now_ends_the_process_while_a_thread_is_in_the_kernel() {
    if std::env::var_os("NOSHAPE_EXIT_NOW_CHILD").is_some() {
        std::thread::spawn(|| {
            loop {
                let a = make_box_corners(DVec3::ZERO, DVec3::ONE).unwrap();
                let b = make_box_corners(DVec3::splat(0.5), DVec3::splat(1.5)).unwrap();
                let _ = boolean(BooleanOp::Fuse, &[a], &[b], &BooleanOptions::default());
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        print!("child about to exit");
        exit_now(7);
    }
    // (The test's name in the merged test binary: its module path past the crate's.)
    let module = module_path!().split_once("::").map_or(module_path!(), |m| m.1);
    let name = format!("{module}::exit_now_ends_the_process_while_a_thread_is_in_the_kernel");
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name.as_str(), "--nocapture", "--test-threads=1"])
        .env("NOSHAPE_EXIT_NOW_CHILD", "1")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(7), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("child about to exit"));
}

/// A point on a cylinder face across the surface's seam (an arc from -30° to 30° extruded: its u runs past 2π) is on
/// the face whichever side of the seam (the projection's u lies in [0, 2π): c2-sm-tablet-stand's rolled wall matched no
/// piece of its sheet metal definition).
#[test]
fn a_cylinder_face_across_its_seam_contains_its_points() {
    let arc = make_arc(DVec3::ZERO, DVec3::Z, DVec3::X, 0.01, -std::f64::consts::FRAC_PI_6, std::f64::consts::FRAC_PI_6).unwrap();
    let face = extrude(&arc, DVec3::new(0.0, 0.0, 0.01)).unwrap().shape.faces().remove(0);
    for deg in [-20.0f64, -10.0, 10.0, 20.0] {
        let a = deg.to_radians();
        let p = DVec3::new(0.01 * a.cos(), 0.01 * a.sin(), 0.005);
        assert!(face.contains_point(p, 1e-7).unwrap(), "{deg}° off the face");
        assert!(face.project_point(p).unwrap().distance < 1e-9, "{deg}°");
    }
    let outside = DVec3::new(-0.01, 0.0, 0.005);
    assert!(!face.contains_point(outside, 1e-7).unwrap());
}

/// An edge through points on a cylinder (a rolling ball's contact curve on a tube, asm-suspension's Lower Control Arm
/// Fillet 3) lies on it between the points too, and splits the face along it.
#[test]
fn an_edge_through_points_on_a_cylinder_stays_on_it_and_splits_it() {
    let (r, h) = (0.0225, 0.04);
    let arc = make_arc(DVec3::ZERO, DVec3::Z, DVec3::X, r, 0.0, PI).unwrap();
    let side = extrude(&arc, DVec3::new(0.0, 0.0, h)).unwrap().shape.faces().remove(0);
    let at = |t: f64| {
        let a = (20.0 + 100.0 * t * t).to_radians();
        DVec3::new(r * a.cos(), r * a.sin(), h * t)
    };
    let pts: Vec<DVec3> = (0..=8).map(|i| at(i as f64 / 8.0)).collect();
    let e = make_edge_on(&side, &pts).unwrap();
    let i = e.edge_info().unwrap();
    assert!(i.start.distance(pts[0]) < 1e-9 && i.end.distance(pts[8]) < 1e-9, "{:?} {:?}", i.start, i.end);
    for k in 0..=64 {
        let p = e.point_at(i.first_param + (i.last_param - i.first_param) * k as f64 / 64.0).unwrap();
        let off = (p.x.hypot(p.y) - r).abs();
        assert!(off < 1e-8, "{off} m off the cylinder at {k}/64");
    }
    let s = split_on(&side, &[e], 1e-7).unwrap().shape;
    let s = s.fixed(1e-7).unwrap();
    assert!(s.is_valid());
    assert_eq!(s.faces().len(), 2);
    let areas: Vec<f64> = s.faces().iter().map(|f| f.area().unwrap()).collect();
    assert!(close(areas.iter().sum::<f64>(), PI * r * h, 1e-9), "{areas:?}");
    // (The piece toward x: the band between angle 0 and the curve, 20 + 100 t² degrees at height h t.)
    let band = r * h * (20f64.to_radians() + 100f64.to_radians() / 3.0);
    assert!(areas.iter().any(|a| close(*a, band, 1e-6)), "{areas:?} vs {band}");
}

/// A cylinder's side, two holes in it, with its top circle replaced by two half circles: the same face, its holes kept.
#[test]
fn a_face_with_an_edge_replaced_by_a_run_keeps_its_other_boundaries() {
    let (r, h) = (0.0225, 0.08);
    let tube = make_cylinder(&Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), r, h, 2.0 * PI).unwrap();
    // (Across y: clear of the seam at +x, two holes in the side.)
    let hole = make_cylinder(&Plane::new(DVec3::new(0.0, -0.05, h / 2.0), DVec3::Y, DVec3::Z), 0.005, 0.1, 2.0 * PI).unwrap();
    let part = cut(&tube, &hole).unwrap().shape;
    let side = part.faces().into_iter().find(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { radius, .. }) if close(radius, r, 1e-9))).unwrap();
    assert_eq!(side.wires().len(), 3);
    let top = side.edges().into_iter().find(|e| e.edge_info().is_ok_and(|i| i.closed && close(i.start.z, h, 1e-9))).unwrap();
    let p = |a: f64| DVec3::new(r * a.cos(), r * a.sin(), h);
    let halves = [make_arc_3pts(p(0.0), p(PI / 2.0), p(PI)).unwrap(), make_arc_3pts(p(PI), p(1.5 * PI), p(0.0)).unwrap()];
    let f = face_with_edge_replaced(&side, &top, &halves).unwrap();
    assert!(f.is_valid());
    assert_eq!((f.wires().len(), f.edges().len()), (3, side.edges().len() + 1), "side's edges {}", side.edges().len());
    assert!(close(f.area().unwrap(), side.area().unwrap(), 1e-9), "{} vs {}", f.area().unwrap(), side.area().unwrap());
}

/// c2-lora-enclosure-ribs' Battery Remove: the cut is valid (its main solid 178 faces), but merging its same-domain faces
/// left a plane of 28 edges invalid; the unify then keeps the cut as it came.
#[test]
fn a_unify_that_would_leave_an_invalid_face_keeps_the_shape() {
    let inputs = Shape::from_bin_bytes(include_bytes!("data/lora-battery-cut.bin")).unwrap();
    let parts = inputs.children();
    let (objects, tools) = (parts[0].children(), parts[1].children());
    let cut = boolean(BooleanOp::Cut, &objects, &tools, &BooleanOptions { parallel: true, ..Default::default() }).unwrap();
    assert!(cut.shape.is_valid());
    let u = unify_same_domain(&cut.shape).unwrap();
    assert!(u.shape.is_valid());
    assert!(close(volume_properties(&u.shape).unwrap().mass * 1e9, 13590.5611, 1e-6));
}

/// A sheet whose faces meet at sharp edges thickened to one side, its offsets meeting where they cross (live's
/// intersection join; c2-surf-workshop-split's Thicken 1): a 40 mm plate with a 10 mm square hole and 10 mm walls hung
/// from the hole's rim, 2 mm down. Concave at the rim (offsets trimmed), convex at the walls' corners (extended to
/// meet); the walls round the foot of the hole lie in one plane and are one face, as live's.
#[test]
fn a_sheet_with_sharp_edges_thickens_with_its_offsets_meeting() {
    let mm = 1e-3;
    let sq = |h: f64| -> Vec<Segment2d> {
        let c = [DVec2::new(-h, -h), DVec2::new(h, -h), DVec2::new(h, h), DVec2::new(-h, h)].map(|p| p * mm);
        (0..4).map(|k| Segment2d::Line { a: c[k], b: c[(k + 1) % 4] }).collect()
    };
    let mut faces = vec![make_face_2d(&Plane::xy(), &sq(20.0), &[sq(5.0)]).unwrap()];
    let corner = [DVec3::new(-5.0, -5.0, 0.0), DVec3::new(5.0, -5.0, 0.0), DVec3::new(5.0, 5.0, 0.0), DVec3::new(-5.0, 5.0, 0.0)].map(|p| p * mm);
    let down = DVec3::new(0.0, 0.0, -10.0 * mm);
    for k in 0..4 {
        let (a, b) = (corner[k], corner[(k + 1) % 4]);
        let pts = [a, b, b + down, a + down];
        let edges: Vec<Shape> = (0..4).map(|j| make_line(pts[j], pts[(j + 1) % 4]).unwrap()).collect();
        faces.push(make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap());
    }
    let sheet = sew(&faces, 1e-7, false).unwrap().shape;
    let top = sheet.faces().into_iter().find(|f| f.is_planar() && f.area().unwrap() > 1e-3).unwrap();
    let up = top.normal_at_point(DVec3::new(-15.0, -15.0, 0.0) * mm).unwrap().z > 0.0;
    let t = 2.0 * mm;
    let r = if up { thicken(&sheet, 0.0, t) } else { thicken(&sheet, t, 0.0) }.unwrap();
    assert!(r.shape.is_valid());
    // The plate 2 mm thick, and the walls' 14 mm square ring below it.
    let v = (1600.0 - 100.0) * 2.0 + (14.0 * 14.0 - 100.0) * 8.0;
    assert!(close(r.shape.volume().unwrap() * 1e9, v, 1e-6 * v), "{} mm3 vs {v}", r.shape.volume().unwrap() * 1e9);
    let bb = r.shape.bounding_box(false).unwrap();
    assert!(close(bb.min.z, -10.0 * mm, 1e-6) && close(bb.max.z, 0.0, 1e-6), "z {} .. {}", bb.min.z, bb.max.z);
    // Plate, hole walls, their offsets, the plate's four edge walls and the ring under the hole walls.
    assert_eq!(r.shape.faces().len(), 1 + 4 + 1 + 4 + 4 + 1);
}

/// A cut's volume change from the faces it touched only: the hole's volume taken off (most faces shared), nothing
/// for the part against itself, and the same as two whole volumes for a shape sharing no face.
#[test]
fn a_volume_change_counts_only_the_faces_not_shared() {
    let mm = 1e-3;
    let block = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 30.0, 10.0) * mm).unwrap();
    let pin = make_cylinder(&Plane::new(DVec3::new(10.0, 10.0, -1.0) * mm, DVec3::Z, DVec3::X), 3.0 * mm, 12.0 * mm, 2.0 * PI).unwrap();
    let cut = boolean(BooleanOp::Cut, std::slice::from_ref(&block), &[pin], &BooleanOptions::default()).unwrap().shape;
    let hole = PI * 9.0 * 10.0;
    assert!((volume_change(&block, &cut).unwrap() / (mm * mm * mm) + hole).abs() < 1e-6, "the hole taken off");
    assert!((volume_change(&cut, &block).unwrap() / (mm * mm * mm) - hole).abs() < 1e-6, "and put back");
    assert_eq!(volume_change(&cut, &cut).unwrap(), 0.0, "nothing against itself");
    let other = make_box_corners(DVec3::ZERO, DVec3::new(20.0, 30.0, 10.0) * mm).unwrap();
    assert!((volume_change(&block, &other).unwrap() / (mm * mm * mm) + 6000.0).abs() < 1e-6, "no shared face: the whole difference");
}

/// A surface loft between two open curves: its edges from end to end are lines, as live's (c2-surfaces-lofts-projected's
/// Loft 1 between two bridging curves: 2 LINE, 2 SPLINE edges; ours were 4 splines). T1.
#[test]
fn a_loft_between_two_open_curves_has_straight_lines_between_their_ends() {
    let mm = 1e-3;
    let bow = |z: f64| {
        let pts: Vec<DVec3> = (0..=6).map(|i| {
            let x = i as f64 / 6.0;
            DVec3::new(10.0 * mm + 9.3 * mm * x, -36.3 * mm - 4.06 * mm * (PI * x).sin(), z)
        }).collect();
        make_interpolated_spline(&pts, &[], false).unwrap()
    };
    let (a, b) = (bow(-12.5 * mm), bow(12.5 * mm));
    let len = a.edges()[0].length().unwrap();
    let sheet = loft(&[a, b], &LoftOptions { solid: false, ..Default::default() }).unwrap().shape;
    assert!(sheet.is_valid());
    let edges = sheet.edges();
    assert_eq!(edges.len(), 4);
    let lines: Vec<f64> = edges.iter().filter_map(|e| {
        let i = e.edge_info().unwrap();
        matches!(i.curve, Curve::Line { .. }).then(|| e.length().unwrap())
    }).collect();
    assert_eq!(lines.len(), 2, "lines between the ends");
    assert!(lines.iter().all(|l| (l - 25.0 * mm).abs() < 1e-9), "{lines:?}");
    let area = sheet.area().unwrap();
    assert!((area - len * 25.0 * mm).abs() < 1e-6 * area, "{} mm² vs {}", area * 1e6, len * 25.0 * mm * 1e6);
}

#[test]
fn a_solid_loft_between_two_hexagons_is_a_frustum_with_line_sides() {
    let mm = 1e-3;
    let hexagon = |r: f64, z: f64| {
        let p: Vec<DVec2> = (0..6).map(|i| DVec2::from_angle(PI / 3.0 * i as f64) * r).collect();
        let segs: Vec<_> = (0..6).map(|i| Segment2d::Line { a: p[i], b: p[(i + 1) % 6] }).collect();
        make_wire_2d(&Plane::xy().offset(z), &segs).unwrap()
    };
    let frustum = |r1: f64, r2: f64, h: f64| {
        let (a1, a2) = (1.5 * 3f64.sqrt() * r1 * r1, 1.5 * 3f64.sqrt() * r2 * r2);
        h / 3.0 * (a1 + a2 + (a1 * a2).sqrt())
    };
    let solid = |r1: f64, r2: f64| loft(&[hexagon(r1, 0.0), hexagon(r2, 30.0 * mm)], &LoftOptions { solid: true, ..Default::default() }).unwrap().shape;
    let (outer, inner) = (solid(20.0 * mm, 10.0 * mm), solid(18.0 * mm, 8.0 * mm));
    let wall = boolean(BooleanOp::Cut, std::slice::from_ref(&outer), std::slice::from_ref(&inner), &BooleanOptions::default()).unwrap().shape;
    let v = wall.volume().unwrap();
    let want = frustum(20.0 * mm, 10.0 * mm, 30.0 * mm) - frustum(18.0 * mm, 8.0 * mm, 30.0 * mm);
    assert!((v - want).abs() < 1e-6 * want, "wall {} mm³ for {}", v * 1e9, want * 1e9);
    for (s, r1, r2) in [(&outer, 20.0 * mm, 10.0 * mm), (&inner, 18.0 * mm, 8.0 * mm)] {
        assert!(s.is_valid());
        assert!(s.max_edge_tolerance() < 1e-6, "edge tolerance {}", s.max_edge_tolerance());
        let v = s.volume().unwrap();
        assert!((v - frustum(r1, r2, 30.0 * mm)).abs() < 1e-6 * v, "{} mm³", v * 1e9);
        let lines = s.edges().iter().filter(|e| matches!(e.edge_info().unwrap().curve, Curve::Line { .. })).count();
        assert_eq!(lines, 18, "sides and profiles");
    }
}

/// DRIVEN-PATTERN-SWEEP (c2-loft-guide-curves-driven-pattern's Sweep 1, live OK): a 0.001 in circle, 22° off square to
/// a 40 mm routing curve coiled 0.06 in high, sweeps into one tube (OCCT's approximation failed at the tolerance the
/// path's 3 mm size gave). Its volume is the circle's area square to the path times the path's length.
#[test]
fn a_thin_wire_coiled_on_a_routing_curve_sweeps() {
    let c = Shape::from_brep_bytes(include_bytes!("data/sweep/coiled-wire.brep")).unwrap();
    let parts = c.children();
    let (profile, path) = (parts[0].clone(), parts[parts.len() - 1].clone());
    let r = sweep(&profile, &path, &SweepOptions { solid: true, ..Default::default() }).unwrap().shape;
    assert_eq!(r.count(ShapeType::Solid), 1);
    assert!(r.is_valid());
    let (rad, len) = (0.0254e-3, path.edges().iter().map(|e| e.edge_info().unwrap().length).sum::<f64>());
    let e = &path.edges()[0];
    let i = e.edge_info().unwrap();
    let t = e.tangent_at(i.first_param).unwrap().normalize();
    let Ok(Surface::Plane(pl)) = profile.faces()[0].face_surface() else { panic!("a planar profile") };
    let n = pl.normal;
    let want = PI * rad * rad * len * t.dot(n).abs();
    let v = r.volume().unwrap();
    assert!((v - want).abs() < 0.01 * want, "{} mm3, want {}", v * 1e9, want * 1e9);
}

/// A face moved along its normal between neighbours that run along it (square planes, parallel cylinders) is the part
/// cut or fused with the face's slab: exact volume, one solid, and each face of the result one face's image (the
/// scissor-lift's Move faces, where OCCT's offset took up to 1 s, and its fallback came back null after the slab's
/// unify had changed the part's own edges).
#[test]
fn a_face_moved_between_square_neighbours_is_the_part_and_its_slab() {
    let check = |s: &Shape, face: &Shape, d: f64| {
        let (v0, a) = (s.volume().unwrap(), face.area().unwrap());
        let r = offset_faces(s, std::slice::from_ref(face), d).unwrap();
        assert!(r.shape.is_valid() && r.shape.count(ShapeType::Solid) == 1, "offset {d}: {:?} {} solids", r.shape.shape_type(), r.shape.count(ShapeType::Solid));
        assert!(close(r.shape.volume().unwrap(), v0 + a * d, 1e-9), "offset {d}: {} vs {}", r.shape.volume().unwrap(), v0 + a * d);
        let faces = r.shape.faces();
        assert_eq!(faces.len(), s.faces().len(), "offset {d}");
        let by_out = r.history.by_output();
        for i in 0..faces.len() as u32 {
            let from: std::collections::HashSet<u32> =
                by_out.get(&(ShapeType::Face, i)).into_iter().flatten().filter(|h| h.arg == 0 && h.input_kind == ShapeType::Face).map(|h| h.input_index).collect();
            assert_eq!(from.len(), 1, "offset {d}: face {i} from {from:?}");
        }
        // Each face generated from the one it was, as OCCT's offset (named "Offset of" it: asm-scissor-lift's Extrude 1).
        assert!(r.history.records.iter().filter(|h| h.input_kind == ShapeType::Face && h.output_kind == ShapeType::Face).all(|h| h.relation == Relation::Generated), "offset {d}");
    };
    let at_z = |s: &Shape, z: f64| s.faces().into_iter().find(|f| f.is_planar() && close(f.centroid().unwrap().z, z, 1e-9)).unwrap();
    // A block with a pocket: its top (a plane with a hole) up and down, the pocket's floor deeper and higher.
    let block = make_box(&Plane::xy(), 40.0, 30.0, 20.0).unwrap();
    let pocket = make_box_corners(DVec3::new(15.0, 10.0, 12.0), DVec3::new(25.0, 20.0, 21.0)).unwrap();
    let s = cut(&block, &pocket).unwrap().shape;
    for d in [4.0, -3.0] {
        check(&s, &at_z(&s, 20.0), d);
    }
    for d in [-3.0, 2.0] {
        check(&s, &at_z(&s, 12.0), d);
    }
    // Rounded corners and a hole: the top down (its slab cut away along the cylinders).
    let vertical: Vec<Shape> = block.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| (i.start.z - i.end.z).abs() > 1.0)).collect();
    let rounded = fillet(&block, &vertical, 5.0).unwrap().shape;
    let hole = make_cylinder(&Plane::new(DVec3::new(20.0, 15.0, -1.0), DVec3::Z, DVec3::X), 4.0, 22.0, 2.0 * PI).unwrap();
    let s = cut(&rounded, &hole).unwrap().shape;
    check(&s, &at_z(&s, 20.0), -3.0);
}
