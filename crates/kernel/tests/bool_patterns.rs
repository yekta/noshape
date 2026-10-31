//! Booleans with patterned tools (rotated or translated copies of one tool), timed: noshape's way (whatever
//! op_boolean does) against OCCT's alone on the same inputs, each case's faces and volume the same both ways.
//! T2 (seconds to minutes each); `tools/tiers/perf.sh -n 1 -p noshape-kernel --test all -- bool_patterns::
//! --ignored --nocapture --test-threads=1` on vibe3. NOSHAPE_KERNEL_PROFILE=1 adds each case's kernel and OCCT steps.

use glam::DVec3;
use noshape_kernel as k;
use std::f64::consts::TAU;
use std::time::Instant;

/// Cuts (or fuses) `tools` from `object` both ways; prints and compares.
fn bench(name: &str, op: k::BooleanOp, object: &k::Shape, tools: &[k::Shape], opts: &k::BooleanOptions) {
    bench_within(name, op, object, tools, opts, 1e-9);
}

/// `bench`, the volumes the same to `volume_tol` relative.
fn bench_within(name: &str, op: k::BooleanOp, object: &k::Shape, tools: &[k::Shape], opts: &k::BooleanOptions, volume_tol: f64) {
    let run = |rust: Option<bool>| {
        k::set_rust_boolean(rust);
        let t = Instant::now();
        let r = k::boolean(op, std::slice::from_ref(object), tools, opts).unwrap_or_else(|e| panic!("{name}: {e}"));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let path = k::last_boolean_path();
        // (Where noshape's Boolean declined: why.)
        let path = if path != "rust" && rust.is_none() && !k::last_rust_decline().is_empty() { format!("{path}: {}", k::last_rust_decline()) } else { path };
        k::set_rust_boolean(None);
        let v: f64 = r.shape.volume().unwrap_or(0.0);
        (ms, path, v, r.shape.count(k::ShapeType::Face), r.shape.is_valid())
    };
    let ours = run(None);
    k::kernel_profile_mark(&format!("{name} (ours)"));
    // (NOSHAPE_PATTERNS_OURS=1: noshape's way alone, not compared.)
    if std::env::var_os("NOSHAPE_PATTERNS_OURS").is_some() {
        eprintln!("pattern {name}: {:.1} ms ({}); {:.6e} m³, {} faces, valid {}", ours.0, ours.1, ours.2, ours.3, ours.4);
        return;
    }
    let occt = run(Some(false));
    k::kernel_profile_mark(&format!("{name} (OCCT)"));
    eprintln!("pattern {name}: {:.1} ms ({}), OCCT {:.1} ms ({:.1}x); {:.6e} m³, {} faces, valid {}; {} tools (OCCT's: {:.6e} m³, {} faces, valid {})", ours.0, ours.1, occt.0, occt.0 / ours.0, ours.2, ours.3, ours.4, tools.len(), occt.2, occt.3, occt.4);
    // (Where OCCT's result isn't valid, ours must be: not the same as it.)
    assert!(ours.4 || !occt.4, "{name}: not valid");
    if occt.4 {
        assert_eq!(ours.3, occt.3, "{name}: faces");
        assert!((ours.2 - occt.2).abs() <= volume_tol * occt.2.abs(), "{name}: volume {} against OCCT's {}", ours.2, occt.2);
    }
}

fn rotated(tool: &k::Shape, axis_origin: DVec3, axis: DVec3, n: usize) -> Vec<k::Shape> {
    (0..n).map(|i| k::transform(tool, &k::Transform::Rotate { origin: axis_origin, axis, angle: TAU * i as f64 / n as f64 }, true).unwrap().shape).collect()
}

fn z_frame(o: DVec3) -> k::Plane {
    k::Plane::new(o, DVec3::Z, DVec3::X)
}

/// 24 through holes on a circle in a plate.
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn holes_on_a_circle_in_a_plate() {
    let plate = k::make_box_corners(DVec3::new(-0.05, -0.05, 0.0), DVec3::new(0.05, 0.05, 0.01)).unwrap();
    let hole = k::make_cylinder(&z_frame(DVec3::new(0.035, 0.0, -0.001)), 0.003, 0.012, TAU).unwrap();
    bench("holes on a circle", k::BooleanOp::Cut, &plate, &rotated(&hole, DVec3::ZERO, DVec3::Z, 24), &k::BooleanOptions::default());
}

/// 24 radial holes into a cylinder's side.
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn radial_holes_in_a_cylinder() {
    let cyl = k::make_cylinder(&z_frame(DVec3::ZERO), 0.025, 0.05, TAU).unwrap();
    let hole = k::make_cylinder(&k::Plane::new(DVec3::new(0.015, 0.0, 0.025), DVec3::X, DVec3::Y), 0.002, 0.015, TAU).unwrap();
    bench("radial holes", k::BooleanOp::Cut, &cyl, &rotated(&hole, DVec3::ZERO, DVec3::Z, 24), &k::BooleanOptions::default());
}

/// 12 radial holes into a cylinder: the pattern's rotation found, one period's places, crossings, pieces and
/// classification for the rest; as OCCT's.
#[test]
fn radial_holes_by_one_period() {
    let cyl = k::make_cylinder(&z_frame(DVec3::ZERO), 0.025, 0.05, TAU).unwrap();
    let hole = k::make_cylinder(&k::Plane::new(DVec3::new(0.015, 0.0, 0.025), DVec3::X, DVec3::Y), 0.002, 0.015, TAU).unwrap();
    bench("radial holes, 12", k::BooleanOp::Cut, &cyl, &rotated(&hole, DVec3::ZERO, DVec3::Z, 12), &k::BooleanOptions::default());
}

/// 400 fins (a 20 × 20 grid of thin boxes) fused onto a plate's top, each reaching into it.
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn four_hundred_fins_on_a_plate() {
    let plate = k::make_box_corners(DVec3::ZERO, DVec3::new(0.2, 0.2, 0.01)).unwrap();
    let fin = k::make_box_corners(DVec3::new(0.004, 0.002, 0.008), DVec3::new(0.006, 0.008, 0.03)).unwrap();
    let tools: Vec<k::Shape> = (0..400).map(|i| k::transform(&fin, &k::Transform::Translate(DVec3::new(0.01 * (i % 20) as f64, 0.01 * (i / 20) as f64, 0.0)), true).unwrap().shape).collect();
    bench("400 fins", k::BooleanOp::Fuse, &plate, &tools, &k::BooleanOptions::default());
}

/// A 10 × 10 grid of pockets in a plate's top.
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn a_grid_of_pockets() {
    let plate = k::make_box_corners(DVec3::ZERO, DVec3::new(0.2, 0.2, 0.02)).unwrap();
    let pocket = k::make_box_corners(DVec3::new(0.005, 0.005, 0.014), DVec3::new(0.015, 0.015, 0.021)).unwrap();
    let tools: Vec<k::Shape> = (0..100).map(|i| k::transform(&pocket, &k::Transform::Translate(DVec3::new(0.02 * (i % 10) as f64, 0.02 * (i / 10) as f64, 0.0)), true).unwrap().shape).collect();
    bench("pocket grid", k::BooleanOp::Cut, &plate, &tools, &k::BooleanOptions::default());
}

/// 40 slots around a disc's rim (gear-like teeth between them).
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn slots_around_a_disc() {
    let disc = k::make_cylinder(&z_frame(DVec3::ZERO), 0.03, 0.01, TAU).unwrap();
    let slot = k::make_box_corners(DVec3::new(0.025, -0.0015, -0.001), DVec3::new(0.032, 0.0015, 0.011)).unwrap();
    bench("gear slots", k::BooleanOp::Cut, &disc, &rotated(&slot, DVec3::ZERO, DVec3::Z, 40), &k::BooleanOptions::default());
}

/// A V-groove swept along a helix round a cylinder, `n` of them by rotation, and with `mirrored` their mirror
/// images too (a diamond knurl's two families).
fn helical(n: usize, mirrored: bool) -> (k::Shape, Vec<k::Shape>) {
    let frame = z_frame(DVec3::ZERO);
    let (r, h) = (0.025, 0.04);
    let pitch = 2.0 * r * std::f64::consts::PI * 60f64.to_radians().tan();
    let cyl = k::make_cylinder(&frame, r, h, TAU).unwrap();
    let helix = k::make_helix(&frame, r, pitch, h, false, 0.0).unwrap();
    let (depth, half) = (0.0008, 0.0008 * 40f64.to_radians().tan() / 30f64.to_radians().cos());
    let p = |t: f64, d: f64| DVec3::new(r + d, t, 0.0);
    let face = k::make_face_from_wire(&k::make_polyline(&[p(half, 0.0), p(0.0, depth), p(-half, 0.0), p(0.0, -depth)], true).unwrap(), true).unwrap();
    let tool = k::sweep(&face, &helix, &k::SweepOptions::default()).unwrap().shape;
    let mut tools = rotated(&tool, DVec3::ZERO, DVec3::Z, n);
    if mirrored {
        let mirror = k::Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X);
        let m: Vec<k::Shape> = tools.iter().map(|t| k::transform(t, &k::Transform::Mirror(mirror), true).unwrap().shape).collect();
        tools.extend(m);
    }
    (cyl, tools)
}

/// 12 helical grooves (a 12-start thread's).
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn helical_grooves() {
    let (cyl, tools) = helical(12, false);
    bench("helical grooves", k::BooleanOp::Cut, &cyl, &tools, &k::BooleanOptions::default());
}

/// 16 helical grooves each way, crossing (a small diamond knurl).
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn crossing_helical_grooves() {
    let (cyl, tools) = helical(16, true);
    bench("crossing helical grooves", k::BooleanOp::Cut, &cyl, &tools, &k::BooleanOptions::default());
}

/// The B94.6 diamond knurl's cut as the Feature Studio feature makes it (core's
/// custom_features::knurl_feature_b94_on_a_cylinder): 148 grooves, recorded (tests/data/patterns).
#[test]
#[ignore = "T2: timed pattern Booleans (minutes)"]
fn the_b94_diamond_knurl() {
    // (OCCT's pave filler grows ~4–8 GB on it: past the default budget on a small machine.) Its steps too (perf.sh
    // can't set the environment; read at the kernel's first call: run it alone).
    // SAFETY: read by each Boolean of this thread; these tests run one at a time (--test-threads=1).
    unsafe {
        std::env::set_var("NOSHAPE_BOOLEAN_MEMORY_MB", "16000");
        if std::env::var_os("NOSHAPE_KERNEL_PROFILE").is_none() {
            std::env::set_var("NOSHAPE_KERNEL_PROFILE", "1");
        }
    }
    // At the model's working scale (millimetres), as the feature runs.
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/patterns");
    let c = k::bool_corpus::load(&dir, "d14b48a8ddf91647").expect("the recorded knurl");
    // (The volume to 1e-7: the_b94_knurl_by_noshapes_boolean.)
    bench_within("B94.6 diamond knurl", c.op, &c.objects[0], &c.tools, &c.opts, 1e-7);
}

/// A result's history records, each output by where it lies (a face by its centroid and area, an edge by its ends
/// and length, a vertex by its point: none by a curve's parameters), grouped by what they say.
fn placed_records(r: &k::OpResult) -> std::collections::BTreeMap<String, Vec<Vec<f64>>> {
    let mut out: std::collections::BTreeMap<String, Vec<Vec<f64>>> = Default::default();
    for h in &r.history.records {
        let x = r.shape.sub_shape(h.output_kind, h.output_index as usize).ok();
        let at = match (h.output_kind, &x) {
            (k::ShapeType::Face, Some(x)) => {
                let c = x.centroid().unwrap_or_default();
                vec![c.x, c.y, c.z, x.area().unwrap_or(0.0)]
            }
            (k::ShapeType::Edge, Some(x)) => {
                let i = x.edge_info().unwrap();
                vec![i.start.x, i.start.y, i.start.z, i.end.x, i.end.y, i.end.z, x.length().unwrap_or(0.0)]
            }
            (k::ShapeType::Vertex, Some(x)) => x.vertex_point().unwrap_or_default().to_array().to_vec(),
            _ => Vec::new(),
        };
        out.entry(format!("arg{} {:?}{} {:?} {:?}", h.arg, h.input_kind, h.input_index, h.relation, h.output_kind)).or_default().push(at);
    }
    out
}

/// The B94.6 knurl's cut through noshape's Boolean (its grooves' end caps share the cylinder's end planes and each
/// other's: a piece on the object and a tool, or on two tools, taken), as OCCT's: faces, edges, vertices, validity,
/// every history record (each output where OCCT's is, within 2 µm; an edge's ends within 20 µm, below), the volume
/// to 1e-7 (OCCT computes each groove's sections apart, ours turns one period's: OCCT's edges lie on ours within
/// 2e-8 m, their curves parameterized otherwise). OCCT unscaled (NOSHAPE_BOOL_NO_SCALE): op_boolean's scaled path
/// for 64+ tools makes the vertices it keeps anew, its history then saying Modified (the cylinder's seam vertex).
#[test]
#[ignore = "T2: the knurl's cut both ways (minutes)"]
fn the_b94_knurl_by_noshapes_boolean() {
    // SAFETY: read by each Boolean of this thread; these tests run one at a time (--test-threads=1).
    unsafe {
        std::env::set_var("NOSHAPE_BOOLEAN_MEMORY_MB", "16000");
    }
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/patterns");
    let c = k::bool_corpus::load(&dir, "d14b48a8ddf91647").expect("the recorded knurl");
    let ours = k::boolean(c.op, &c.objects, &c.tools, &c.opts).unwrap();
    assert_eq!(k::last_boolean_path(), "rust", "declined: {}", k::last_rust_decline());
    k::set_rust_boolean(Some(false));
    // SAFETY: as above.
    unsafe { std::env::set_var("NOSHAPE_BOOL_NO_SCALE", "1") };
    let occt = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    // SAFETY: as above.
    unsafe { std::env::remove_var("NOSHAPE_BOOL_NO_SCALE") };
    k::set_rust_boolean(None);
    let occt = occt.unwrap();
    for kind in [k::ShapeType::Solid, k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex] {
        assert_eq!(ours.shape.count(kind), occt.shape.count(kind), "{kind:?}");
    }
    assert!(ours.shape.is_valid() && occt.shape.is_valid());
    let (v, w) = (ours.shape.volume().unwrap(), occt.shape.volume().unwrap());
    assert!((v - w).abs() <= 1e-7 * w, "volume {v} against OCCT's {w}");
    let (a, b) = (placed_records(&ours), placed_records(&occt));
    let mut differ = Vec::new();
    for key in a.keys().chain(b.keys()).collect::<std::collections::BTreeSet<_>>() {
        let (xs, ys) = (a.get(key).cloned().unwrap_or_default(), b.get(key).cloned().unwrap_or_default());
        let mut used = vec![false; ys.len()];
        for x in &xs {
            // (An edge's ends within 20 µm: where the two families of grooves cross on the pattern's mirror planes
            // their curves meet nearly tangent, and OCCT's crossing lies 11 µm along them from ours (ours on the
            // plane, OCCT's 2e-9 m off it); its length, and everything else, within 2 µm.)
            let close = |y: &[f64]| x.iter().zip(y).enumerate().all(|(i, (p, q))| (p - q).abs() <= if x.len() == 7 && i < 6 { 2e-5 } else { 2e-6 });
            // (Its ends either way round.)
            let near = |y: &Vec<f64>| y.len() == x.len() && (close(y) || (y.len() == 7 && close(&[&y[3..6], &y[0..3], &y[6..]].concat())));
            match (0..ys.len()).find(|&j| !used[j] && near(&ys[j])) {
                Some(j) => used[j] = true,
                None => differ.push(format!("ours only: {key} {x:?}")),
            }
        }
        differ.extend((0..ys.len()).filter(|&j| !used[j]).map(|j| format!("OCCT's only: {key} {:?}", ys[j])));
    }
    assert!(differ.is_empty(), "{} records differ:\n{}", differ.len(), differ[..differ.len().min(40)].join("\n"));
}

/// Tools that are rigid copies are found from their geometry (noshape_brep::pattern): a circle of holes is one
/// family turning by 15°; the B94.6 knurl's 148 grooves two families of 74 (the pattern and its mirror), each
/// turning by 360° / 74 about the cylinder's axis.
#[test]
fn copies_among_patterned_tools() {
    use noshape_brep::pattern::families;
    let hole = k::make_cylinder(&z_frame(DVec3::new(0.035, 0.0, -0.001)), 0.003, 0.012, TAU).unwrap();
    let holes: Vec<_> = rotated(&hole, DVec3::ZERO, DVec3::Z, 24).iter().map(|t| k::brep_body(t).unwrap()).collect();
    let fams = families(&holes.iter().collect::<Vec<_>>(), 1e-7);
    assert_eq!(fams.len(), 1);
    let (_, axis, angle) = fams[0][1].1.rotation().expect("a rotation");
    assert!(axis.cross(DVec3::Z).length() < 1e-9 && (angle - TAU / 24.0).abs() < 1e-9, "{axis} {angle}");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/patterns");
    let c = k::bool_corpus::load(&dir, "d14b48a8ddf91647").expect("the recorded knurl");
    let grooves: Vec<_> = c.tools.iter().map(|t| k::brep_body(t).unwrap()).collect();
    let fams = families(&grooves.iter().collect::<Vec<_>>(), 1e-7);
    assert_eq!(fams.iter().map(|f| f.len()).collect::<Vec<_>>(), [74, 74]);
    for f in &fams {
        let (_, axis, angle) = f[1].1.rotation().expect("a rotation");
        assert!(axis.cross(DVec3::Z).length() < 1e-9, "{axis}");
        let turns = angle / (TAU / 74.0);
        assert!((turns - turns.round()).abs() < 1e-6, "{angle}");
    }
}
