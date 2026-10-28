//! Planar faces with many boundary nodes are meshed by the kernel's own
//! constrained Delaunay triangulation (mesh.cpp `PlanarCdtAlgo`): the mesh
//! must stay watertight with the faces around (the same boundary nodes),
//! turned like the face and Delaunay like BRepMesh's.
use noshape_kernel::*;
use std::collections::HashMap;
use std::f64::consts::TAU;

/// The 400 mm plate with 400 Ø8 holes (qa5 "big plate", PERF1), metres.
fn plate() -> Shape {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(0.4, 0.4, 0.01)).unwrap();
    let mut tools = Vec::new();
    for i in 0..20 {
        for j in 0..20 {
            let c = DVec3::new(0.01 + 0.02 * i as f64, 0.01 + 0.02 * j as f64, -0.001);
            tools.push(make_cylinder(&Plane::new(c, DVec3::Z, DVec3::X), 0.004, 0.012, TAU).unwrap());
        }
    }
    boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap().shape
}

/// A 2 mm-pitch comb of 1000 fins, 40 mm high, on a 10 mm base (the fins
/// pattern's side faces: one planar face of ~4000 nodes each side).
fn comb() -> Shape {
    let base = make_box_corners(DVec3::new(0.0, 0.0, -0.01), DVec3::new(2.0, 0.02, 0.0)).unwrap();
    let fins: Vec<Shape> = (0..1000).map(|k| make_box_corners(DVec3::new(0.002 * k as f64, 0.0, 0.0), DVec3::new(0.002 * k as f64 + 0.001, 0.02, 0.04)).unwrap()).collect();
    boolean(BooleanOp::Fuse, &[base], &fins, &BooleanOptions { unify: true, ..Default::default() }).unwrap().shape
}

fn area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f64 {
    let (a, b, c) = (DVec3::from(a.map(f64::from)), DVec3::from(b.map(f64::from)), DVec3::from(c.map(f64::from)));
    (b - a).cross(c - a).length() / 2.0
}

/// Every triangle edge shared by exactly two triangles once equal positions
/// are welded: no cracks or T-junctions between faces.
fn assert_watertight(m: &Tessellation) {
    let mut id: HashMap<[u32; 3], u32> = HashMap::new();
    let weld: Vec<u32> = m
        .positions
        .iter()
        .map(|p| {
            let n = id.len() as u32;
            *id.entry(p.map(f32::to_bits)).or_insert(n)
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

/// The planar faces' triangles turned like the face; the large ones are
/// Delaunay (no neighbour's far vertex inside a triangle's circumcircle).
fn assert_planar_faces(s: &Shape, m: &Tessellation) {
    let mut big = 0;
    for (i, f) in s.faces().iter().enumerate() {
        if !matches!(f.face_surface(), Ok(Surface::Plane(_))) {
            continue;
        }
        let r = &m.faces[i];
        let tris = &m.indices[r.first_index as usize..(r.first_index + r.index_count) as usize];
        // Every triangle turned like the face (with the watertight check: no
        // folds or overlaps, so the triangles cover the face's polygon once).
        let normal = DVec3::from(m.normals[tris[0] as usize].map(f64::from));
        let flipped = tris
            .chunks(3)
            .filter(|t| {
                let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
                let n = (DVec3::from(b.map(f64::from)) - DVec3::from(a.map(f64::from))).cross(DVec3::from(c.map(f64::from)) - DVec3::from(a.map(f64::from)));
                area(a, b, c) > 0.0 && n.dot(normal) <= 0.0
            })
            .count();
        assert_eq!(flipped, 0, "face {i}: triangles turned against the face");
        if tris.len() / 3 < 1000 {
            continue;
        }
        big += 1;
        let p = |k: u32| DVec3::from(m.positions[k as usize].map(f64::from));
        let mut opposite: HashMap<(u32, u32), u32> = HashMap::new();
        let mut bad = 0;
        let mut worst = f64::MIN;
        for t in tris.chunks(3) {
            for k in 0..3 {
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                if let Some(&d) = opposite.get(&(b, a)) {
                    // Delaunay: the two angles facing the edge sum to at most
                    // pi. Positions are f32 (~3e-8 m at 0.4 m against 0.2 mm
                    // rim edges: ~2e-4 rad of noise); a triangulation that
                    // isn't Delaunay is off by far more.
                    let angle = |o: DVec3, x: DVec3, y: DVec3| (x - o).angle_between(y - o);
                    let excess = angle(p(c), p(a), p(b)) + angle(p(d), p(b), p(a)) - std::f64::consts::PI;
                    worst = worst.max(excess);
                    if excess > 1e-2 {
                        bad += 1;
                    }
                } else {
                    opposite.insert((a, b), c);
                }
            }
        }
        eprintln!("face {i}: {} triangles, largest Delaunay excess {worst:.2e} rad", tris.len() / 3);
        assert_eq!(bad, 0, "face {i}: {bad} edges not Delaunay (worst {worst:.3} rad)");
    }
    assert!(big >= 2, "the large planar faces: {big}");
}

#[test]
#[ignore = "T2: 33 s mesh stress"]
fn planar_faces_with_many_holes_mesh_watertight_and_delaunay() {
    for (what, s) in [("plate", plate()), ("comb", comb())] {
        // The display's settings: angular Auto (~140 segments a circle), chord by the angle.
        let t0 = std::time::Instant::now();
        let m = tessellate(&s, s.bounding_box(false).unwrap().diagonal(), 0.0898, true).unwrap();
        eprintln!("{what}: {} triangles in {:?}", m.triangle_count(), t0.elapsed());
        assert_watertight(&m);
        assert_planar_faces(&s, &m);
    }
}

/// A boundary segment a-b with a vertex in line behind a (as the 400-fin
/// plate's front face once a fin is fused under one: the fin's sides in line
/// with the fin's above). The kernel's triangulation took that vertex for one
/// on the segment and refused the face, and BRepMesh took 0.3 s over the
/// plate's. Here a = (0, 0) and b = (0, -5) have nodes in line behind them as
/// their nearest (so Delaunay edges), and two spikes beside a-b's middle
/// keep a-b from being one.
#[test]
fn a_vertex_in_line_behind_a_boundary_segment_keeps_the_planar_triangulation() {
    for flip in [1.0, -1.0] {
        let p = |x: f64, y: f64| DVec3::new(flip * x, y, 0.0) * 1e-3;
        let mut pts = vec![p(0.0, 0.0), p(0.0, -5.0), p(10.0, -5.0), p(10.0, -2.6), p(0.3, -2.5), p(10.0, -2.4), p(10.0, 10.0), p(1.0, 10.0), p(0.0, 1.0), p(-1.0, 10.0), p(-10.0, 10.0), p(-10.0, -10.0), p(0.0, -6.0), p(-0.3, -2.5)];
        if flip < 0.0 {
            pts.reverse();
        }
        let face = make_face_from_wire(&make_polyline(&pts, true).unwrap(), true).unwrap();
        let m = tessellate(&face, face.bounding_box(false).unwrap().diagonal(), 0.0898, true).unwrap();
        assert_eq!(m.planar_fallbacks, 0, "flip {flip}: the face went to BRepMesh");
        assert_eq!(m.triangle_count(), pts.len() - 2);
    }
}

/// Meshing the biggest face of a part, `NOSHAPE_BENCH_N` times (perf; run
/// with NOSHAPE_MESH_CACHE=0). NOSHAPE_BENCH_CASE=<dir>/<name> (a corpus case:
/// its first object).
#[test]
#[ignore = "bench: NOSHAPE_BENCH_CASE=/root/core-scratch/bool-one/a2121c5b456e7188 NOSHAPE_MESH_CACHE=0"]
fn mesh_big_face_bench() {
    let Ok(case) = std::env::var("NOSHAPE_BENCH_CASE") else { return };
    let path = std::path::Path::new(&case);
    let c = noshape_kernel::bool_corpus::load(path.parent().unwrap(), path.file_name().unwrap().to_str().unwrap()).expect("the case");
    let _ = set_working_scale(1000.0);
    let face = c.objects[0].faces().into_iter().max_by_key(|f| f.count(ShapeType::Edge)).unwrap();
    let n: usize = std::env::var("NOSHAPE_BENCH_N").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let diag = c.objects[0].bounding_box(false).unwrap().diagonal();
    let mut ms = Vec::new();
    let mut sum = 0u64;
    for _ in 0..n {
        let t = std::time::Instant::now();
        let m = tessellate(&face, diag * 1e-3, 0.09, false).unwrap();
        ms.push(t.elapsed().as_secs_f64() * 1e3);
        assert_eq!(m.planar_fallbacks, 0);
        // What came out, to compare builds.
        sum = m.indices.iter().map(|&i| i as u64).chain(m.positions.iter().flatten().map(|p| p.to_bits() as u64)).fold(0xcbf2_9ce4_8422_2325, |h, x| (h ^ x).wrapping_mul(0x100_0000_01b3));
    }
    ms.sort_by(f64::total_cmp);
    eprintln!("the {}-edge face: median {:.1} ms, best {:.1} ms, triangles' hash {sum:016x}", face.count(ShapeType::Edge), ms[ms.len() / 2], ms[0]);
}
