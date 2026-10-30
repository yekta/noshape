//! noshape's own display mesher (crates/brep `mesh`, cpp/mesh_rust.cpp) against BRepMesh: the same shapes
//! meshed with it (faces it takes) and without it (a copy: new faces, so the mesh cache has none) give the same
//! faces (area, bounds, turned like the normals), the same edge polylines and a watertight mesh.
use noshape_kernel::*;
use std::collections::HashMap;
use std::f64::consts::TAU;

const MM: f64 = 1e-3;

fn mesh(s: &Shape, rust: bool) -> Tessellation {
    set_rust_mesh(rust);
    // The display's angle-based deflections (display.rs display_deflection).
    let t = tessellate(s, s.bounding_box(false).unwrap().diagonal(), 2.0 * TAU / 144.0, true).unwrap();
    set_rust_mesh(true);
    t
}

fn p(q: [f32; 3]) -> DVec3 {
    DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)
}

/// Per face: area (along the vertex normals), bounds.
fn faces(t: &Tessellation) -> Vec<(f64, DVec3, DVec3)> {
    t.faces
        .iter()
        .map(|r| {
            let tris = &t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize];
            let mut area = 0.0;
            for c in tris.chunks(3) {
                let (a, b, cc) = (p(t.positions[c[0] as usize]), p(t.positions[c[1] as usize]), p(t.positions[c[2] as usize]));
                let n = (b - a).cross(cc - a);
                let normal = p(t.normals[c[0] as usize]);
                // Turned like the normals.
                assert!(n.dot(normal) >= 0.0, "a triangle against its normal");
                area += n.length() / 2.0;
            }
            let pts = &t.positions[r.first_vertex as usize..(r.first_vertex + r.vertex_count) as usize];
            let lo = pts.iter().fold(DVec3::splat(f64::MAX), |m, &q| m.min(p(q)));
            let hi = pts.iter().fold(DVec3::splat(f64::MIN), |m, &q| m.max(p(q)));
            (area, lo, hi)
        })
        .collect()
}

fn assert_watertight(m: &Tessellation) {
    let mut id: HashMap<[u32; 3], u32> = HashMap::new();
    let weld: Vec<u32> = m
        .positions
        .iter()
        .map(|q| {
            let n = id.len() as u32;
            *id.entry(q.map(f32::to_bits)).or_insert(n)
        })
        .collect();
    let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
    for t in m.indices.chunks(3) {
        for k in 0..3 {
            let (a, b) = (weld[t[k] as usize], weld[t[(k + 1) % 3] as usize]);
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    let open = edges.values().filter(|&&n| n != 2).count();
    assert_eq!(open, 0, "{open} of {} edges not shared by two triangles", edges.len());
}

/// Normals (PSUI-CONE-SHADING, a cone's tip shaded as a blob): every node's normal is the face's own outward
/// one there and BRepMesh's at the same point (an apex's nodes, the same point three times or more, aside:
/// theirs differ by side line); every triangle's vertex normals are within 7° of the face's normal at its middle
/// (an apex's too: one axial normal there was 59° off a 31° cone's).
fn assert_normals(s: &Shape, rust: &Tessellation, occt: &Tessellation) {
    let faces = s.faces();
    let nodes = |t: &Tessellation, i: usize| -> Vec<([u32; 3], DVec3, DVec3)> {
        let r = &t.faces[i];
        (r.first_vertex..r.first_vertex + r.vertex_count).map(|k| (t.positions[k as usize].map(f32::to_bits), p(t.positions[k as usize]), p(t.normals[k as usize]).normalize())).collect()
    };
    for (i, f) in faces.iter().enumerate() {
        let (ours, theirs) = (nodes(rust, i), nodes(occt, i));
        let mut count: HashMap<[u32; 3], usize> = HashMap::new();
        for (b, _, _) in &ours {
            *count.entry(*b).or_insert(0) += 1;
        }
        let at: HashMap<[u32; 3], DVec3> = theirs.iter().map(|(b, _, n)| (*b, *n)).collect();
        for (b, q, n) in ours.iter().filter(|(b, _, _)| count[b] < 3) {
            // (None at an apex itself.)
            let Ok(exact) = f.normal_at_point(*q) else { continue };
            assert!(n.dot(exact) > 1.0 - 1e-5, "face {i}: the normal at {q} is {:.3}° off the surface's", n.angle_between(exact).to_degrees());
            if let Some(m) = at.get(b) {
                assert!(n.dot(*m) > 1.0 - 1e-5, "face {i}: the normal at {q} is {:.3}° off BRepMesh's", n.angle_between(*m).to_degrees());
            }
        }
        let r = &rust.faces[i];
        for c in rust.indices[r.first_index as usize..(r.first_index + r.index_count) as usize].chunks(3) {
            let mid = c.iter().map(|&k| p(rust.positions[k as usize])).sum::<DVec3>() / 3.0;
            let Ok(exact) = f.normal_at_point(mid) else { continue };
            for &k in c {
                let n = p(rust.normals[k as usize]).normalize();
                assert!(n.angle_between(exact) < 7f64.to_radians(), "face {i}: a triangle's normal at {} is {:.1}° off the surface's at its middle", p(rust.positions[k as usize]), n.angle_between(exact).to_degrees());
            }
        }
    }
}

/// `s` meshed both ways: the same faces and edges, watertight; returns how many faces the Rust mesher took.
fn same_as_brepmesh(s: &Shape) -> usize {
    let rust = mesh(s, true);
    let copy = transform(s, &Transform::Translate(DVec3::ZERO), true).unwrap().shape;
    let occt = mesh(&copy, false);
    assert_eq!(occt.rust_faces, 0);
    assert_eq!(rust.faces.len(), occt.faces.len());
    let tol = s.bounding_box(false).unwrap().diagonal() * 1e-6;
    for (i, (a, b)) in faces(&rust).into_iter().zip(faces(&occt)).enumerate() {
        // Within the polygons' own error: a 144-gon's area is 1 − 8e-5 of its circle's (BRepMesh triangulates a
        // cone between the same rims a little differently); a sphere's is short in both directions, each up to
        // (π / 144)² ≈ 5e-4 between its rows (BRepMesh's 7.4e-4 under, ours 4e-4).
        assert!((a.0 - b.0).abs() <= 1e-3 * b.0.max(MM * MM), "face {i}: area {} against BRepMesh's {}", a.0, b.0);
        // Its rows where BRepMesh has none, a doubly curved face's extremes differ by up to a polygon's sag (the
        // sphere's equator: ours on it, BRepMesh's 4.6e-4 of the radius inside).
        let btol = tol.max(1e-3 * (b.2 - b.1).max_element());
        assert!((a.1 - b.1).abs().max_element() <= btol && (a.2 - b.2).abs().max_element() <= btol, "face {i}: bounds {:?} {:?} against {:?} {:?}", a.1, a.2, b.1, b.2);
    }
    assert_eq!(rust.edges.len(), occt.edges.len());
    for i in 0..rust.edges.len() {
        let (a, b) = (rust.edge_polyline(i), occt.edge_polyline(i));
        assert_eq!(a.len(), b.len(), "edge {i}: points");
        assert!(a.iter().zip(b).all(|(x, y)| (p(*x) - p(*y)).length() <= tol), "edge {i}: points differ");
    }
    assert_watertight(&rust);
    assert_normals(s, &rust, &occt);
    rust.rust_faces
}

/// A plate with 1 mm fins across it, fused and unified (the fin plate in small): planes and lines only.
fn fins(n: usize) -> Shape {
    let len = (2 * n + 4) as f64;
    let base = make_box_corners(DVec3::ZERO, DVec3::new(len, 20.0, 5.0) * MM).unwrap();
    let tools: Vec<Shape> = (0..n).map(|i| make_box_corners(DVec3::new(2.0 + 2.0 * i as f64, 0.0, 5.0) * MM, DVec3::new(3.0 + 2.0 * i as f64, 20.0, 9.0) * MM).unwrap()).collect();
    boolean(BooleanOp::Fuse, &[base], &tools, &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape
}

#[test]
fn planes_bounded_by_lines_mesh_as_brepmesh_meshes_them() {
    let s = fins(30);
    let n = s.faces().len();
    assert_eq!(same_as_brepmesh(&s), n, "every face of the fin plate");
}

#[test]
fn holes_mesh_as_brepmesh_meshes_them() {
    // A plate with four through holes: planes with circles, cylinders with seams.
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 5.0) * MM).unwrap();
    let holes: Vec<Shape> = (0..4).map(|i| make_cylinder(&Plane::new(DVec3::new(5.0 + 10.0 * i as f64, 10.0, -1.0) * MM, DVec3::Z, DVec3::X), 2.0 * MM, 7.0 * MM, TAU).unwrap()).collect();
    let s = boolean(BooleanOp::Cut, &[plate], &holes, &BooleanOptions::default()).unwrap().shape;
    assert_eq!(same_as_brepmesh(&s), s.faces().len(), "every face");
}

#[test]
fn a_countersink_meshes_as_brepmesh_meshes_it() {
    // A hole with a 90° countersink: a cone between two circles.
    let plate = make_box_corners(DVec3::new(-10.0, -10.0, 0.0) * MM, DVec3::new(10.0, 10.0, 5.0) * MM).unwrap();
    let hole = make_cylinder(&Plane::new(DVec3::new(0.0, 0.0, -1.0) * MM, DVec3::Z, DVec3::X), 2.0 * MM, 7.0 * MM, TAU).unwrap();
    let sink = make_cone(&Plane::new(DVec3::new(0.0, 0.0, 3.0) * MM, DVec3::Z, DVec3::X), 1.0 * MM, 4.0 * MM, 3.0 * MM, TAU).unwrap();
    let s = boolean(BooleanOp::Cut, &[plate], &[hole, sink], &BooleanOptions::default()).unwrap().shape;
    assert_eq!(same_as_brepmesh(&s), s.faces().len(), "every face");
}

#[test]
fn fillets_round_a_boss_mesh_as_brepmesh_meshes_them() {
    // A boss on a plate, its foot and its top edge filleted: a concave and a convex torus between the plate, the
    // boss's side and its top.
    let plate = make_box_corners(DVec3::new(-10.0, -10.0, 0.0) * MM, DVec3::new(10.0, 10.0, 5.0) * MM).unwrap();
    let boss = make_cylinder(&Plane::new(DVec3::new(0.0, 0.0, 5.0) * MM, DVec3::Z, DVec3::X), 4.0 * MM, 4.0 * MM, TAU).unwrap();
    let s = boolean(BooleanOp::Fuse, &[plate], &[boss], &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape;
    let round: Vec<Shape> = s.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| i.closed && !i.degenerated && i.start.z > 4.9 * MM)).collect();
    assert_eq!(round.len(), 2);
    let s = fillet(&s, &round, 1.0 * MM).unwrap().shape;
    assert_eq!(same_as_brepmesh(&s), s.faces().len(), "every face");
}

#[test]
fn a_cone_to_its_apex_and_a_sphere_mesh_as_brepmesh_meshes_them() {
    // An apex and two poles: degenerate edges, one node each.
    let cone = make_cone(&Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 3.0 * MM, 0.0, 5.0 * MM, TAU).unwrap();
    assert_eq!(same_as_brepmesh(&cone), cone.faces().len(), "the cone and its base");
    let ball = make_sphere(&Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 4.0 * MM, TAU).unwrap();
    assert_eq!(same_as_brepmesh(&ball), 1, "the sphere");
}

#[test]
fn faces_on_curves_brepmesh_splits_stay_brepmesh_and_the_mesh_stays_watertight() {
    // A hole drilled at an angle: its rims are ellipses (BRepMesh's), so the hole and the faces it cuts are
    // BRepMesh's; the plate's sides are ours.
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 5.0) * MM).unwrap();
    let axis = DVec3::new(0.3, 0.0, 1.0).normalize();
    let hole = make_cylinder(&Plane::new(DVec3::new(20.0, 10.0, -5.0) * MM, axis, DVec3::Y), 2.0 * MM, 15.0 * MM, TAU).unwrap();
    let s = boolean(BooleanOp::Cut, &[plate], &[hole], &BooleanOptions::default()).unwrap().shape;
    assert_eq!(same_as_brepmesh(&s), 4, "the four sides");
}

#[test]
fn a_patterns_instances_share_one_mesh() {
    // A body and a moved instance of it (the same faces at another place).
    let s = fins(5);
    let moved = transform(&s, &Transform::Translate(DVec3::new(0.0, 30.0, 0.0) * MM), false).unwrap().shape;
    let both = Shape::compound(&[s.clone(), moved]);
    assert_eq!(same_as_brepmesh(&both), 2 * s.faces().len());
}

/// This thread's CPU time in ms (Linux; 0 elsewhere): the bench's measure on a busy machine.
fn cpu_ms() -> f64 {
    std::fs::read_to_string("/proc/thread-self/schedstat").ok().and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0) / 1e6
}

/// The 400-fin plate's display mesh, ours against BRepMesh (fresh copies, so the mesh cache serves nothing,
/// with their maps made, as a regenerated part has them): median of 5 each, wall and CPU time. `cargo test --release -p noshape-kernel --test all rust_mesh::bench -- --ignored --nocapture`;
/// NOSHAPE_BENCH_OURS=1: ours only, 40 copies (under `perf record`).
#[test]
#[ignore = "bench"]
fn bench_fin_plate_display_mesh() {
    let s = fins(400);
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    let times = |rust: bool| {
        let (wall, cpu): (Vec<f64>, Vec<f64>) = (0..5)
            .map(|_| {
                let copy = transform(&s, &Transform::Translate(DVec3::ZERO), true).unwrap().shape;
                // Its maps, as the regeneration's naming leaves them on a part.
                let _ = copy.ancestor_map(ShapeType::Edge, ShapeType::Face).unwrap();
                let (t, c) = (std::time::Instant::now(), cpu_ms());
                kernel_profile_mark("before");
                let m = mesh(&copy, rust);
                kernel_profile_mark(if rust { "ours" } else { "BRepMesh" });
                assert_eq!(m.rust_faces > 0, rust);
                (t.elapsed().as_secs_f64() * 1e3, cpu_ms() - c)
            })
            .unzip();
        (median(wall), median(cpu))
    };
    if std::env::var_os("NOSHAPE_BENCH_OURS").is_some() {
        // Ours only, many rounds on one set of copies' worth of work (under `perf record`).
        let copies: Vec<Shape> = (0..40).map(|_| transform(&s, &Transform::Translate(DVec3::ZERO), true).unwrap().shape).collect();
        for c in &copies {
            let _ = c.ancestor_map(ShapeType::Edge, ShapeType::Face).unwrap();
        }
        // A profiler attaches here (`perf record -p`), past the setup.
        eprintln!("fin plate: meshing in 1 s");
        std::thread::sleep(std::time::Duration::from_secs(1));
        let t = std::time::Instant::now();
        for c in &copies {
            mesh(c, true);
        }
        eprintln!("fin plate, ours: {:.1} ms each", t.elapsed().as_secs_f64() * 1e3 / 40.0);
        return;
    }
    let (ours, occt) = (times(true), times(false));
    eprintln!(
        "fin plate ({} faces) display mesh: ours {:.1} ms (CPU {:.1}), BRepMesh {:.1} ms (CPU {:.1}): {:.1}× ({:.1}× CPU)",
        s.faces().len(),
        ours.0,
        ours.1,
        occt.0,
        occt.1,
        occt.0 / ours.0,
        occt.1 / ours.1
    );
}

/// The fin plate's display mesh by step (the kernel profile, wall time) over three fresh copies: for
/// tools/tiers/perf.sh, which can't pass the environment. `cargo test --release -p noshape-kernel --test all
/// rust_mesh::bench_profile_fin -- --ignored --nocapture --test-threads=1` (alone: the profile switch is read once).
#[test]
#[ignore = "bench"]
fn bench_profile_fin_plate_display_mesh() {
    // SAFETY: set before any kernel call of this test process (run alone), read by the kernel once.
    unsafe { std::env::set_var("NOSHAPE_KERNEL_PROFILE", "1") };
    let s = fins(400);
    let copies: Vec<Shape> = (0..3).map(|_| transform(&s, &Transform::Translate(DVec3::ZERO), true).unwrap().shape).collect();
    for c in &copies {
        let _ = c.ancestor_map(ShapeType::Edge, ShapeType::Face).unwrap();
    }
    kernel_profile_mark("setup");
    let t = std::time::Instant::now();
    for c in &copies {
        mesh(c, true);
    }
    eprintln!("fin plate, ours: {:.1} ms each (profiled)", t.elapsed().as_secs_f64() * 1e3 / 3.0);
    kernel_profile_mark("ours, three copies");
}

/// This process's CPU time in ms, its threads' that ended included (Linux, 10 ms ticks; 0 elsewhere).
fn process_cpu_ms() -> f64 {
    std::fs::read_to_string("/proc/self/stat").ok().and_then(|s| {
        let f: Vec<&str> = s.rsplit_once(')')?.1.split_whitespace().collect();
        Some((f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?) * 10.0)
    }).unwrap_or(0.0)
}

/// The filleted boss (live-30's bosses: two tori of ~5000 nodes and their neighbours) meshed 40 times each way,
/// fresh copies: wall and process CPU per mesh, all threads counted. `cargo test --release -p noshape-kernel
/// --test all rust_mesh::bench_filleted -- --ignored --nocapture`; NOSHAPE_BENCH_OURS=1: ours only, ten rounds
/// (under `perf record`).
#[test]
#[ignore = "bench"]
fn bench_filleted_boss_display_mesh() {
    let plate = make_box_corners(DVec3::new(-10.0, -10.0, 0.0) * MM, DVec3::new(10.0, 10.0, 5.0) * MM).unwrap();
    let boss = make_cylinder(&Plane::new(DVec3::new(0.0, 0.0, 5.0) * MM, DVec3::Z, DVec3::X), 4.0 * MM, 4.0 * MM, TAU).unwrap();
    let s = boolean(BooleanOp::Fuse, &[plate], &[boss], &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape;
    let round: Vec<Shape> = s.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| i.closed && !i.degenerated && i.start.z > 4.9 * MM)).collect();
    let s = fillet(&s, &round, 1.0 * MM).unwrap().shape;
    let times = |rust: bool| {
        let copies: Vec<Shape> = (0..40).map(|_| transform(&s, &Transform::Translate(DVec3::ZERO), true).unwrap().shape).collect();
        let (t, c) = (std::time::Instant::now(), process_cpu_ms());
        let nodes: usize = copies.iter().map(|copy| mesh(copy, rust).positions.len()).sum();
        (t.elapsed().as_secs_f64() * 1e3 / 40.0, (process_cpu_ms() - c) / 40.0, nodes / 40)
    };
    if std::env::var_os("NOSHAPE_BENCH_OURS").is_some() {
        for _ in 0..10 {
            times(true);
        }
        return;
    }
    let (ours, occt) = (times(true), times(false));
    eprintln!("filleted boss ({} nodes) display mesh: ours {:.2} ms (CPU {:.2}), BRepMesh {:.2} ms (CPU {:.2}): {:.1}× ({:.1}× CPU)", ours.2, ours.0, ours.1, occt.0, occt.1, occt.0 / ours.0, occt.1 / ours.1);
}

/// A big shape's box (its faces boxed in parallel, kernel geom.cpp) is the box of its faces, each boxed alone.
#[test]
fn a_big_shapes_box_is_its_faces_boxes() {
    let s = fins(80);
    assert!(s.faces().len() >= 256, "the parallel path");
    let b = s.bounding_box(false).unwrap();
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for f in s.faces() {
        let fb = f.bounding_box(false).unwrap();
        lo = lo.min(fb.min);
        hi = hi.max(fb.max);
    }
    // Within the faces' tolerance gaps (one box keeps the largest gap all round).
    assert!((b.min - lo).abs().max_element() < 1e-6 && (b.max - hi).abs().max_element() < 1e-6, "{b:?} against {lo} {hi}");
}
