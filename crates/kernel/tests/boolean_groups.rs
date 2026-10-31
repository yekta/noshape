//! A cut with many tools gathers the tools that don't touch each other into
//! one argument each group (boolean.cpp group_disjoint); tools that cross
//! must stay apart, and the result is the same solid.

use noshape_kernel::*;
use std::f64::consts::PI;

#[test]
fn cut_with_many_tools_some_crossing() {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(100.0, 100.0, 10.0)).unwrap();
    // 12 holes Ø6 through the plate, apart from each other and from the bars.
    let r = 3.0;
    let mut tools: Vec<Shape> = Vec::new();
    for i in 0..4 {
        for j in 0..3 {
            let c = DVec3::new(15.0 + 22.0 * i as f64, 15.0 + 70.0 * (j as f64 / 2.0), -1.0);
            if (c.y - 50.0).abs() < 1.0 {
                continue;
            }
            tools.push(make_cylinder(&Plane::new(c, DVec3::Z, DVec3::X), r, 12.0, 2.0 * PI).unwrap());
        }
    }
    for i in 0..4 {
        tools.push(make_cylinder(&Plane::new(DVec3::new(15.0 + 22.0 * i as f64, 95.0, -1.0), DVec3::Z, DVec3::X), r, 12.0, 2.0 * PI).unwrap());
    }
    let holes = tools.len();
    assert_eq!(holes, 12);
    // Two through slots crossing each other in the middle (a plus).
    tools.push(make_box_corners(DVec3::new(20.0, 48.0, -1.0), DVec3::new(80.0, 52.0, 11.0)).unwrap());
    tools.push(make_box_corners(DVec3::new(48.0, 30.0, -1.0), DVec3::new(52.0, 70.0, 11.0)).unwrap());
    // OCCT's build from the faces (patch 0001's CutByFaces: 14 tools, the slots one group of crossing ones):
    // the plate's solid Modified as in the usual build, not deleted (bool's replays compare the two).
    set_rust_boolean(Some(false));
    let by_faces = boolean(BooleanOp::Cut, std::slice::from_ref(&plate), &tools, &BooleanOptions::default());
    set_rust_boolean(None);
    let h = by_faces.unwrap().history;
    let solid = |rel: Relation| h.records.iter().any(|x| x.arg == 0 && x.input_kind == ShapeType::Solid && x.input_index == 0 && x.relation == rel);
    assert!(solid(Relation::Modified) && !solid(Relation::Deleted), "{:?}", h.records.iter().filter(|x| x.input_kind == ShapeType::Solid).collect::<Vec<_>>());
    let res = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap().shape;
    let want = 100.0 * 100.0 * 10.0 - holes as f64 * PI * r * r * 10.0 - (60.0 * 4.0 + 40.0 * 4.0 - 4.0 * 4.0) * 10.0;
    let v = res.volume().unwrap();
    assert!((v - want).abs() < 1e-6 * want, "volume {v}, want {want}");
    assert_eq!(res.count(ShapeType::Solid), 1);
    assert!(res.is_valid());
}

/// Holes straight through a plate are built directly (boolean.cpp
/// cut_through_holes; big-plate's 400 cost ~1.7 s of CPU in OCCT): the same
/// solid as a cut, each tool's side the hole's wall, its caps gone.
#[test]
fn holes_through_a_plate_built_directly() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(160.0, 160.0, 10.0) * mm).unwrap();
    let r = 4.0 * mm;
    let mut tools: Vec<Shape> = Vec::new();
    for i in 0..8 {
        for j in 0..8 {
            let c = DVec3::new(10.0 + 20.0 * i as f64, 10.0 + 20.0 * j as f64, -5.0) * mm;
            tools.push(make_cylinder(&Plane::new(c, DVec3::Z, DVec3::X), r, 20.0 * mm, 2.0 * PI).unwrap());
        }
    }
    let t = std::time::Instant::now();
    let res = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap();
    eprintln!("64 holes in {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let s = &res.shape;
    assert!(s.is_valid());
    let want = (160.0 * 160.0 - 64.0 * PI * 16.0) * 10.0 * 1e-9;
    assert!((s.volume().unwrap() - want).abs() < 1e-12, "{} vs {want}", s.volume().unwrap());
    assert_eq!(s.faces().len(), 6 + 64);
    for (k, tool) in tools.iter().enumerate() {
        let side = tool.faces().iter().position(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { .. }))).unwrap() as u32;
        let walls = res.history.modified(k as u32 + 1, ShapeType::Face, side);
        assert_eq!(walls.len(), 1, "tool {k}: {:?}", res.history.of_input(k as u32 + 1, ShapeType::Face, side).collect::<Vec<_>>());
    }
}

/// A boss standing on a face is fused directly (boolean.cpp fuse_on_face):
/// the face gets the boss's outline as an inner loop; the same solid as the
/// boolean's. Off the face's edge it is fused as usual.
#[test]
fn a_boss_on_a_face_fused_directly() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(100.0, 60.0, 10.0) * mm).unwrap();
    let boss = make_cylinder(&Plane::new(DVec3::new(30.0, 30.0, 10.0) * mm, DVec3::Z, DVec3::X), 5.0 * mm, 8.0 * mm, 2.0 * PI).unwrap();
    let t = std::time::Instant::now();
    let r = boolean(BooleanOp::Fuse, std::slice::from_ref(&plate), std::slice::from_ref(&boss), &BooleanOptions::default()).unwrap();
    eprintln!("boss in {:.2} ms", t.elapsed().as_secs_f64() * 1e3);
    assert!(r.shape.is_valid());
    let want = (100.0 * 60.0 * 10.0 + PI * 25.0 * 8.0) * 1e-9;
    assert!((r.shape.volume().unwrap() - want).abs() < 1e-13, "{}", r.shape.volume().unwrap());
    assert_eq!(r.shape.faces().len(), 6 + 2);
    let top = plate.faces().iter().position(|f| f.centroid().unwrap().z > 9.9 * mm).unwrap() as u32;
    assert_eq!(r.history.modified(0, ShapeType::Face, top).len(), 1);
    let bottom_cap = boss.faces().iter().position(|f| f.centroid().unwrap().z < 10.1 * mm && matches!(f.face_surface(), Ok(Surface::Plane(_)))).unwrap() as u32;
    assert!(r.history.is_deleted(1, ShapeType::Face, bottom_cap));
    // Over the plate's edge: the boolean's (the boss's foot is split).
    let edge = make_cylinder(&Plane::new(DVec3::new(98.0, 30.0, 10.0) * mm, DVec3::Z, DVec3::X), 5.0 * mm, 8.0 * mm, 2.0 * PI).unwrap();
    let r = boolean(BooleanOp::Fuse, &[plate], &[edge], &BooleanOptions::default()).unwrap();
    assert!(r.shape.is_valid());
    let v = r.shape.volume().unwrap();
    assert!((v - (100.0 * 60.0 * 10.0 + PI * 25.0 * 8.0) * 1e-9).abs() < 1e-12, "{v}");
}

/// A fin fused flush under a plate of 100 fins goes the local way
/// (boolean.cpp local_boolean: only the faces it reaches and their
/// neighbours through OCCT) and gives OCCT's whole-part result: the same
/// solid, faces and history (the corpus replay compares all of it).
#[test]
fn a_small_fuse_on_a_big_part_is_done_locally() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let n = 100;
    let len = (2 * n + 10) as f64;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(len, 20.0, 10.0) * mm).unwrap();
    let fins: Vec<Shape> = (0..n).map(|i| make_box_corners(DVec3::new(2.0 + 2.0 * i as f64, 0.0, 10.0) * mm, DVec3::new(3.0 + 2.0 * i as f64, 20.0, 15.0) * mm).unwrap()).collect();
    let part = boolean(BooleanOp::Fuse, &[plate], &fins, &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape;
    assert!(part.count(ShapeType::Face) >= 200, "{}", part.count(ShapeType::Face));
    let fin = make_box_corners(DVec3::new(2.0, 0.0, -5.0) * mm, DVec3::new(3.0, 20.0, 0.0) * mm).unwrap();
    let local = boolean(BooleanOp::Fuse, std::slice::from_ref(&part), std::slice::from_ref(&fin), &BooleanOptions::default()).unwrap();
    assert_eq!(last_boolean_path(), "local");
    assert!(local.shape.is_valid());
    let want = (len * 20.0 * 10.0 + (n + 1) as f64 * 20.0 * 5.0) * 1e-9;
    assert!((local.shape.volume().unwrap() - want).abs() < 1e-9 * want, "{}", local.shape.volume().unwrap());
    // The whole Boolean's result, through OCCT (a fuzzy value keeps it off the local path; noshape's own
    // Boolean switched off, which would take it).
    set_rust_boolean(Some(false));
    let whole = boolean(BooleanOp::Fuse, &[part], &[fin], &BooleanOptions { fuzzy: 1e-12, ..Default::default() }).unwrap();
    set_rust_boolean(None);
    assert_eq!(last_boolean_path(), "occt");
    assert_eq!(noshape_kernel::bool_corpus::Summary::of(&local).differences(&noshape_kernel::bool_corpus::Summary::of(&whole)), Vec::<String>::new());
}

/// classify_point on a part of 200+ faces goes by rays (geom.cpp
/// RayClassifier): points inside, outside, on faces and on edges of a plate
/// with 100 fins, against the union of its boxes.
#[test]
fn points_against_a_big_part_classified_by_rays() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let n = 100;
    let len = (2 * n + 10) as f64;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(len, 20.0, 10.0) * mm).unwrap();
    let fins: Vec<Shape> = (0..n).map(|i| make_box_corners(DVec3::new(2.0 + 2.0 * i as f64, 0.0, 10.0) * mm, DVec3::new(3.0 + 2.0 * i as f64, 20.0, 15.0) * mm).unwrap()).collect();
    let part = boolean(BooleanOp::Fuse, &[plate], &fins, &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape;
    assert!(part.count(ShapeType::Face) >= 200);
    // In the closed union of the boxes (mm).
    let closed = |p: DVec3| {
        let inb = |a: DVec3, b: DVec3| p.cmpge(a).all() && p.cmple(b).all();
        inb(DVec3::ZERO, DVec3::new(len, 20.0, 10.0)) || (0..n).any(|i| inb(DVec3::new(2.0 + 2.0 * i as f64, 0.0, 10.0), DVec3::new(3.0 + 2.0 * i as f64, 20.0, 15.0)))
    };
    let truth = |p: DVec3| {
        if !closed(p) {
            return PointClass::Outside;
        }
        let e = 1e-3;
        let all_in = (0..27).all(|k| closed(p + DVec3::new((k % 3) as f64 - 1.0, ((k / 3) % 3) as f64 - 1.0, (k / 9) as f64 - 1.0) * e));
        if all_in { PointClass::Inside } else { PointClass::OnBoundary }
    };
    let xs: Vec<f64> = (-2..=60).map(|k| k as f64 * 0.5).chain([len - 0.5, len, len + 0.5]).collect();
    let mut wrong = Vec::new();
    for &x in &xs {
        for y in [-1.0, 0.0, 7.3, 20.0, 21.0] {
            for z in [-1.0, 0.0, 4.1, 10.0, 12.5, 15.0, 16.0] {
                let p = DVec3::new(x, y, z);
                let got = part.classify_point(p * mm, 1e-7).unwrap();
                if got != truth(p) {
                    wrong.push(format!("{p}: {got:?}, want {:?}", truth(p)));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} wrong:\n{}", wrong.len(), wrong.join("\n"));
}

/// The unify after a local Boolean on a part the unify found nothing in
/// looks only where the Boolean changed the part (boolean.cpp `UnifyScope`):
/// the same result as the whole unify, in fuses whose faces merge into the
/// part's big faces and whose edges merge into chains, and in cuts.
#[test]
fn the_unify_after_a_local_boolean_matches_the_whole_one() {
    use noshape_kernel::bool_corpus::Summary;
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let n = 100;
    let len = (2 * n + 10) as f64;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(len, 20.0, 10.0) * mm).unwrap();
    let fins: Vec<Shape> = (0..n).map(|i| make_box_corners(DVec3::new(2.0 + 2.0 * i as f64, 0.0, 10.0) * mm, DVec3::new(3.0 + 2.0 * i as f64, 20.0, 15.0) * mm).unwrap()).collect();
    let part = boolean(BooleanOp::Fuse, &[plate], &fins, &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape;
    // As the regeneration does after every Boolean: a unify, which finds
    // nothing more; the next one knows.
    let part = unify_same_domain(&part).unwrap().shape;
    let _ = unify_same_domain(&part).unwrap();
    assert_eq!(last_unify_path(), "known");
    let cases = [
        ("a fin under the plate", BooleanOp::Fuse, DVec3::new(2.0, 0.0, -5.0), DVec3::new(3.0, 20.0, 0.0)),
        ("the gap between two fins filled", BooleanOp::Fuse, DVec3::new(3.0, 0.0, 10.0), DVec3::new(4.0, 20.0, 15.0)),
        ("the plate made longer, flush", BooleanOp::Fuse, DVec3::new(len, 0.0, 0.0), DVec3::new(len + 5.0, 20.0, 10.0)),
        ("a fin on the end, flush", BooleanOp::Fuse, DVec3::new(len - 1.0, 0.0, 10.0), DVec3::new(len, 20.0, 15.0)),
        ("a notch in a fin", BooleanOp::Cut, DVec3::new(1.5, 5.0, 12.0), DVec3::new(3.5, 8.0, 16.0)),
        ("a pocket in the bottom", BooleanOp::Cut, DVec3::new(20.0, 5.0, -1.0), DVec3::new(30.0, 15.0, 4.0)),
    ];
    for (what, op, a, b) in cases {
        let tool = make_box_corners(a * mm, b * mm).unwrap();
        let r = boolean(op, std::slice::from_ref(&part), &[tool], &BooleanOptions::default()).unwrap();
        assert_eq!(last_boolean_path(), "local", "{what}");
        let scoped = unify_same_domain(&r.shape).unwrap();
        assert_eq!(last_unify_path(), "scoped", "{what}");
        let whole = unify_same_domain_whole(&r.shape).unwrap();
        assert_eq!(Summary::of(&scoped).differences(&Summary::of(&whole)), Vec::<String>::new(), "{what}");
        assert!(scoped.shape.count(ShapeType::Face) <= r.shape.count(ShapeType::Face), "{what}");
        let _ = unify_same_domain(&scoped.shape).unwrap();
        assert_eq!(last_unify_path(), "known", "{what}");
    }
}

/// The fin fused under the 400-fin plate (corpus case a2121c5b, Extrude 3 of
/// fins-400), as the regeneration runs it: the Boolean, the unify, the
/// naming's maps, `NOSHAPE_BENCH_N` times (default 20) for `perf record`.
/// NOSHAPE_BENCH_CASE=<dir>/<name>.
#[test]
#[ignore = "bench: NOSHAPE_BENCH_CASE=/root/core-scratch/bool-one/a2121c5b456e7188"]
fn local_fuse_bench() {
    let Ok(case) = std::env::var("NOSHAPE_BENCH_CASE") else { return };
    let path = std::path::Path::new(&case);
    let c = noshape_kernel::bool_corpus::load(path.parent().unwrap(), path.file_name().unwrap().to_str().unwrap()).expect("the case");
    let _ = set_working_scale(1000.0);
    let n: usize = std::env::var("NOSHAPE_BENCH_N").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let mut ms = Vec::new();
    for _ in 0..n {
        // A fresh target each round, known to have nothing to merge (as a
        // regenerated part is).
        let target = Shape::from_brep_bytes(&c.objects[0].to_brep_bytes().unwrap()).unwrap();
        let target = unify_same_domain(&target).unwrap().shape;
        let _ = target.ancestor_map(ShapeType::Edge, ShapeType::Face).unwrap();
        let _ = target.ancestor_map(ShapeType::Vertex, ShapeType::Face).unwrap();
        // The auto scope's classification made the face boxes.
        let _ = target.classify_point(DVec3::ZERO, 1e-9);
        let t = std::time::Instant::now();
        let r = boolean(c.op, std::slice::from_ref(&target), &c.tools, &c.opts).unwrap();
        let u = unify_same_domain(&r.shape).unwrap();
        let _ = u.shape.ancestor_map(ShapeType::Edge, ShapeType::Face).unwrap();
        let _ = u.shape.ancestor_map(ShapeType::Vertex, ShapeType::Face).unwrap();
        ms.push(t.elapsed().as_secs_f64() * 1e3);
    }
    ms.sort_by(f64::total_cmp);
    eprintln!("local fuse + unify + maps: median {:.1} ms, best {:.1} ms ({} runs, path {}, unify {})", ms[ms.len() / 2], ms[0], n, last_boolean_path(), last_unify_path());
}


/// Display meshes of faces a Boolean left alone come from the kernel's mesh
/// cache (mesh.cpp `MeshSet`); only the new faces are meshed: the same
/// triangles, normals and edge polylines as meshing a fresh copy whole, for a
/// fuse (straight shared edges: partial) and a round hole (circular edges
/// shared with kept faces: meshed whole).
#[test]
fn display_meshes_of_kept_faces_are_reused() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let n = 100;
    let len = (2 * n + 10) as f64;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(len, 20.0, 10.0) * mm).unwrap();
    let fins: Vec<Shape> = (0..n).map(|i| make_box_corners(DVec3::new(2.0 + 2.0 * i as f64, 0.0, 10.0) * mm, DVec3::new(3.0 + 2.0 * i as f64, 20.0, 15.0) * mm).unwrap()).collect();
    let part = boolean(BooleanOp::Fuse, &[plate], &fins, &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape;
    let part = unify_same_domain(&part).unwrap().shape;
    let (lin, ang) = (0.2 * mm, 0.3);
    let _ = tessellate(&part, lin, ang, false).unwrap();
    let fin = make_box_corners(DVec3::new(2.0, 0.0, -5.0) * mm, DVec3::new(3.0, 20.0, 0.0) * mm).unwrap();
    let hole = make_cylinder(&Plane::new(DVec3::new(50.5, 10.0, -1.0) * mm, DVec3::Z, DVec3::X), 3.0 * mm, 12.0 * mm, 2.0 * PI).unwrap();
    for (what, op, tool) in [("a fin fused under the plate", BooleanOp::Fuse, fin), ("a round hole", BooleanOp::Cut, hole)] {
        let r = boolean(op, std::slice::from_ref(&part), &[tool], &BooleanOptions::default()).unwrap().shape;
        let t = std::time::Instant::now();
        let cached = tessellate(&r, lin, ang, false).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let fresh = Shape::from_brep_bytes(&r.to_brep_bytes().unwrap()).unwrap();
        let t = std::time::Instant::now();
        let whole = tessellate(&fresh, lin, ang, false).unwrap();
        eprintln!("{what}: {ms:.1} ms with the cache, {:.1} ms whole", t.elapsed().as_secs_f64() * 1e3);
        assert_eq!(cached.triangle_count(), whole.triangle_count(), "{what}");
        assert_eq!(cached.indices, whole.indices, "{what}");
        assert_eq!(cached.faces.len(), whole.faces.len(), "{what}");
        assert_eq!(cached.edge_points.len(), whole.edge_points.len(), "{what}");
        let near = |a: &[[f32; 3]], b: &[[f32; 3]]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-6));
        assert!(near(&cached.positions, &whole.positions), "{what}: positions");
        assert!(near(&cached.normals, &whole.normals), "{what}: normals");
        assert!(near(&cached.edge_points, &whole.edge_points), "{what}: edge points");
    }
}

/// qa CYL-UNIFY-OPPOSITE: one circle extruded both ways from its plane, fused
/// and unified: one cylinder face over the whole length (live: 3 faces, one
/// 753.98 mm² cylinder), each half's face mapped onto it.
#[test]
fn opposite_extrudes_of_a_circle_unify_into_one_cylinder() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let c = make_circle(DVec3::new(0.0, 0.0, 5.0 * mm), DVec3::Y, 6.0 * mm).unwrap();
    let f = make_face_from_wire(&make_wire(&[c]).unwrap(), true).unwrap();
    let a = extrude(&f, DVec3::new(0.0, -10.0 * mm, 0.0)).unwrap().shape;
    let b = extrude(&f, DVec3::new(0.0, 10.0 * mm, 0.0)).unwrap().shape;
    let fused = fuse(&a, &b).unwrap().shape;
    let v = fused.volume().unwrap();
    for (what, r) in [("unify", unify_same_domain(&fused).unwrap()), ("whole", unify_same_domain_whole(&fused).unwrap())] {
        let s = &r.shape;
        let cyl: Vec<f64> = s.faces().iter().filter(|x| matches!(x.face_surface(), Ok(Surface::Cylinder { .. }))).map(|x| x.area().unwrap() * 1e6).collect();
        assert_eq!(s.count(ShapeType::Face), 3, "{what}: {cyl:?}");
        assert!(cyl.len() == 1 && (cyl[0] - 240.0 * std::f64::consts::PI).abs() < 1e-6, "{what}: {cyl:?}");
        assert!(s.is_valid() && (s.volume().unwrap() - v).abs() < 1e-9 * v, "{what}");
        let out_cyl = s.faces().iter().position(|x| matches!(x.face_surface(), Ok(Surface::Cylinder { .. }))).unwrap() as u32;
        for (i, x) in fused.faces().iter().enumerate() {
            let images = r.history.modified(0, ShapeType::Face, i as u32);
            assert_eq!(images.len(), 1, "{what}: face {i}");
            if matches!(x.face_surface(), Ok(Surface::Cylinder { .. })) {
                assert_eq!(images[0], out_cyl, "{what}: face {i}");
            }
        }
    }
}

/// with_call_budget: a section that takes longer than its budget fails with
/// "section took too long" (std's collision checks in editing logic give up
/// on such a pair, qa MIRROR-HANG-402); without one it runs to the end. The
/// budget is so small that the first check of the deadline (the section's
/// progress indicator) stops it, whatever the machine's speed.
#[test]
fn a_section_under_a_call_budget_gives_up() {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(100.0, 100.0, 10.0)).unwrap();
    let mut tools = Vec::new();
    for i in 0..20 {
        for j in 0..20 {
            tools.push(make_cylinder(&Plane::new(DVec3::new(2.5 + 5.0 * i as f64, 2.5 + 5.0 * j as f64, -1.0), DVec3::Z, DVec3::X), 1.5, 12.0, 2.0 * PI).unwrap());
        }
    }
    let holes = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap().shape;
    let tilted = make_box_corners(DVec3::new(-10.0, -10.0, 4.0), DVec3::new(110.0, 110.0, 6.0)).unwrap();
    let tilted = tilted.transformed(&Transform::Rotate { origin: DVec3::new(50.0, 50.0, 5.0), axis: DVec3::new(1.0, 1.0, 0.0), angle: 0.05 }).unwrap();
    let t = std::time::Instant::now();
    let r = with_call_budget(1e-9, || section(&holes, &tilted));
    let e = r.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(e.contains("took too long"), "{e:?} after {:?}", t.elapsed());
    let s = section(&holes, &tilted).unwrap().shape;
    assert!(s.count(ShapeType::Edge) > 400);
}

/// with_blend_budget: a fillet longer than its budget fails with "fillet took
/// too long" on every platform (off Linux the budget is a deadline OCCT patch
/// 0007's hook stops the fillet builder at; BUDGET-OFF-LINUX), and without a
/// short budget it builds. The budget is so small that the hook's first
/// check stops it, whatever the machine's speed.
#[test]
fn a_fillet_under_a_short_budget_gives_up() {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(100.0, 100.0, 10.0)).unwrap();
    let mut tools = Vec::new();
    for i in 0..10 {
        for j in 0..10 {
            tools.push(make_cylinder(&Plane::new(DVec3::new(5.0 + 10.0 * i as f64, 5.0 + 10.0 * j as f64, -1.0), DVec3::Z, DVec3::X), 3.0, 12.0, 2.0 * PI).unwrap());
        }
    }
    let plate = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap().shape;
    // The holes' top circles.
    let edges: Vec<Shape> = plate.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| matches!(i.curve, Curve::Circle { .. }) && e.point_at(i.first_param).is_ok_and(|p| p.z > 9.9))).collect();
    assert_eq!(edges.len(), 100);
    // (OCCT's fillet: noshape's own takes these rims without a budget.)
    set_rust_blend(Some(false));
    let t = std::time::Instant::now();
    let r = with_blend_budget(1e-9, || fillet(&plate, &edges, 1.0));
    set_rust_blend(None);
    let e = r.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(e.contains("took too long"), "{e:?} after {:?}", t.elapsed());
    let full = fillet(&plate, &edges, 1.0).unwrap().shape;
    assert_eq!(full.count(ShapeType::Solid), 1);
}
