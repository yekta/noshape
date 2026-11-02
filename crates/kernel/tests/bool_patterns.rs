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

/// The B94.6 knurl's patterned cut is taken by noshape's Boolean alone, as recorded (a decline fails here instead of
/// running OCCT: 2afca924 made the grooves' faces on one surface coincident, read a point past a neighbour's patch as
/// on its edge and declined, and the knurl ran OCCT's path, 16 s → 138 s, unseen until vibe3's hourly times, 11-01).
#[test]
fn the_b94_knurl_is_cut_by_noshapes_boolean() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/patterns");
    let c = k::bool_corpus::load(&dir, "d14b48a8ddf91647").expect("the recorded knurl");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined the knurl: {why} ({e})"));
    assert_eq!(r.shape.count(k::ShapeType::Face), c.summary.faces);
    assert!(r.shape.is_valid());
}

/// A fuse whose tools' edges end inside a face (12 slabs on a plate, recorded): noshape's Boolean takes it, the
/// edges in the result's faces as OCCT's builder leaves them (INTERNAL), its faces and edges as OCCT's.
#[test]
fn edges_ending_inside_a_face_stay_in_it() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "5fc0970a688d0fb9").expect("the recorded fuse");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    assert_eq!((r.shape.count(k::ShapeType::Face), r.shape.count(k::ShapeType::Edge)), (c.summary.faces, c.summary.edges));
    assert!(r.shape.is_valid());
    let inner = r.shape.sub_shapes(k::ShapeType::Face).iter().flat_map(|f| f.children()).flat_map(|w| w.children()).filter(|e| matches!(e.orientation(), k::Orientation::Internal)).count();
    assert!(inner > 0, "no INTERNAL edge");
}

/// A sheet metal enclosure's fuse (qa4 S1: the flanges, bends and hem, recorded): noshape's Boolean's result valid and
/// OCCT's volume. A bend on a trimmed left-handed cylinder came out the other way round: invalid, 7 mm³ more
/// (RUST-SM-CUT-VOLUME).
#[test]
fn a_sheet_metal_enclosure_fuse_as_occt() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "02215ef9a303de5c").expect("the recorded fuse");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid() && (v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "valid {}, volume {v} against OCCT's {}", r.shape.is_valid(), c.summary.volume);
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), [c.summary.faces, c.summary.edges, c.summary.vertices]);
}

/// An argument with an edge inside a face (INTERNAL, recorded cut): noshape's Boolean takes it (the edge kept inside the
/// face's piece it lies in, splitting nothing; here in a piece the cut removes), as OCCT's: faces, edges, vertices.
#[test]
fn an_input_edge_inside_a_face_stays_in_it() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "818552c996311f5e").expect("the recorded cut");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), [c.summary.faces, c.summary.edges, c.summary.vertices]);
    assert!(r.shape.is_valid());
    let internal = |s: &k::Shape| s.sub_shapes(k::ShapeType::Face).iter().flat_map(|f| f.children()).flat_map(|w| w.children()).any(|e| matches!(e.orientation(), k::Orientation::Internal));
    assert!(c.objects.iter().chain(&c.tools).any(internal), "no INTERNAL edge in the arguments");
}

/// A fuse whose plane-cylinder sections OCCT gives round the other way (its ellipses' axes against ours, recorded):
/// noshape's Boolean takes it on OCCT's curves, valid, OCCT's volume and counts.
#[test]
fn an_ellipse_round_the_other_way() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "1310d7e980382436").expect("the recorded fuse");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid() && (v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "valid {}, volume {v} against OCCT's {}", r.shape.is_valid(), c.summary.volume);
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), [c.summary.faces, c.summary.edges, c.summary.vertices]);
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

/// qa MIRROR-HANG-402 (live request 1629): a Part mirror (across Top), Add with Merge with all, over 11 overlapping
/// copies of a 38-face free-form fuzz part: live one solid of 243267.537 mm³ in ~2 s. OCCT's one fuse of all 12 ran
/// ~220 s and failed (BOPAlgo_AlertBuilderFailed); fused two at a time in a balanced tree it is live's solid.
#[test]
#[ignore = "T2: qa's STEP outside the repo (onshape-research/live/samples/qa-hang402-11parts.step), ~30 s"]
fn overlapping_free_form_parts_merge_like_onshape() {
    let path = std::path::Path::new("/root/onshape-research/live/samples/qa-hang402-11parts.step");
    let _ = k::set_working_scale(1000.0);
    let parts: Vec<k::Shape> = k::read_step(path, k::LengthUnit::Meter).unwrap().into_iter().map(|p| p.shape).collect();
    assert_eq!(parts.len(), 11);
    let pick = parts.iter().position(|p| (p.bounding_box(false).unwrap().min.x + 0.052).abs() < 5e-4).unwrap();
    let mirrored = parts[pick].transformed(&k::Transform::Mirror(k::Plane::from_normal(DVec3::ZERO, DVec3::Z))).unwrap();
    let t0 = std::time::Instant::now();
    let r = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&mirrored), &parts, &k::BooleanOptions::default()).unwrap();
    let v = k::volume_properties(&r.shape).unwrap().mass * 1e9;
    eprintln!("{:?} ({}): {v:.6} mm³, {} solids, {} faces", t0.elapsed(), k::last_boolean_path(), r.shape.count(k::ShapeType::Solid), r.shape.count(k::ShapeType::Face));
    assert_eq!(r.shape.count(k::ShapeType::Solid), 1);
    assert!((v - 243267.537419).abs() < 0.01, "{v}");
    // Live: 15 faces once the regen's unify merges them.
    let u = k::unify_same_domain(&r.shape).unwrap().shape;
    assert_eq!(u.count(k::ShapeType::Face), 15);
}

/// A cut by 24 grooves crossing in pairs whose floors lie on one plane facing one way (recorded): noshape's Boolean
/// keeps one of the coinciding floor pieces, as OCCT's same-domain faces (it dropped both: a non-manifold result),
/// valid, OCCT's volume and counts.
#[test]
fn crossing_grooves_with_one_floor() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "50004ac95f2ebada").expect("the recorded cut");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid() && (v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "valid {}, volume {v} against OCCT's {}", r.shape.is_valid(), c.summary.volume);
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), [c.summary.faces, c.summary.edges, c.summary.vertices]);
}

/// noshape's Boolean alone on a recorded case: valid, OCCT's volume and counts.
fn as_recorded(name: &str) {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, name).expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid() && (v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "valid {}, volume {v} against OCCT's {}", r.shape.is_valid(), c.summary.volume);
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), [c.summary.faces, c.summary.edges, c.summary.vertices]);
}

/// A common keeping a piece of an argument's half circle from its start (recorded): the piece's range from the
/// edge's own start (its angle a rounding below it went a turn on: "an edge outside its face's range").
#[test]
fn a_piece_of_an_arc_from_its_start() {
    as_recorded("2e30111c66d9ea88");
}

/// A common through a fillet's torus whose closed edge (a whole circle) is cut in two (recorded): each half's ends on
/// the torus at their own ends of the circle's curve there (both halves were mapped onto one: "two edges leave a
/// vertex in one direction").
#[test]
fn halves_of_a_closed_edge_on_a_torus() {
    as_recorded("9f434299c9a0048f");
}

/// noshape's Boolean alone on a recorded case (recorded with it on: its summary is ours then), its result valid and
/// against OCCT's on the same inputs now: the same counts, its volume within `rel`.
fn valid_as_occt(name: &str, rel: f64) {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, name).expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    k::set_rust_boolean(Some(false));
    let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's");
    k::set_rust_boolean(None);
    assert!(r.shape.is_valid(), "noshape's result invalid");
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), counts(&o.shape));
    let (v, w) = (r.shape.volume().unwrap(), o.shape.volume().unwrap());
    assert!((v - w).abs() <= rel * w, "volume {v} against OCCT's {w}");
}

/// Two knurl grooves of opposite hands crossing 44 µm above the cylinder's end face, where both tools' caps lie (the
/// user's Ø25 × 25 B94.6 knurl at 45° / 45°, its tools 28 and 61; recorded): their flanks meet in 94 µm curves from
/// the ridges' crossing to the bottom edges', which OCCT's face/face misses; asked again from between where the
/// faces' edges cross, they are found (none: "a non-manifold result"). OCCT's cut of the two removes nothing here,
/// so the result is against each groove's own cut: the two remove their volumes but for where they cross.
#[test]
fn grooves_crossing_just_above_the_end() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "cd1c58431ab4a62c").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    let one: Vec<f64> = c.tools.iter().map(|t| k::boolean(c.op, &c.objects, std::slice::from_ref(t), &c.opts).expect("one groove's cut").shape.volume().unwrap()).collect();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v0 = c.objects[0].volume().unwrap();
    let want = v0 - one.iter().map(|v| v0 - v).sum::<f64>();
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid());
    // (Where they cross: a sliver of ~1e-4 mm³.)
    assert!(v >= want && v - want <= 1e-3 * 1e-9, "volume {v}, the two grooves' own {want}");
    assert_eq!(r.shape.count(k::ShapeType::Face), 9);
}

/// Two grooves of the Ø25 × 34 knurl at 45° / 45° (its tools 30 and 51; recorded) crossing 37 µm above the end face,
/// mirror images there: their flanks' sections from the bottom edges' crossing to the end plane are 70 µm from the
/// ones at the crossing height, and were taken for one place (merged within ten chords' reach) and not asked for
/// ("a non-manifold result"). OCCT's cut of the two leaves 29 mm³ here; against each groove's own cut and their
/// overlap in the cylinder by OCCT ((object ∩ one) ∩ the other: 0.088501 mm³).
#[test]
fn grooves_crossing_37_um_above_the_end() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "ddaa5365e6d5f85e").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    let one: Vec<f64> = c.tools.iter().map(|t| k::boolean(c.op, &c.objects, std::slice::from_ref(t), &c.opts).expect("one groove's cut").shape.volume().unwrap()).collect();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v0 = c.objects[0].volume().unwrap();
    let want = v0 - one.iter().map(|v| v0 - v).sum::<f64>() + 0.088501e-9;
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid());
    // (OCCT's overlap to 1e-6 mm³, the single cuts ours: within 1e-4 mm³, 6e-9 of the part.)
    assert!((v - want).abs() <= 1e-4 * 1e-9, "volume {v}, the grooves' own and their overlap {want}");
}

/// A piece a few tolerances across (recorded: the Ø25 × 43 knurl at 45° / 45°, its tools 17 and 55): their end caps'
/// corners overlap in a 3 µm diamond on the end plane, whose first point was 0.64 µm from the other cap's edge ("a
/// piece's point near a coincident face's boundary"); a point farther in. OCCT's volume (it splits the cylinder's
/// face at its seam where ours keeps one face, so not its counts).
#[test]
fn a_piece_a_few_tolerances_across() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "83c6823917494739").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    k::set_rust_boolean(Some(false));
    let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's");
    k::set_rust_boolean(None);
    let (v, w) = (r.shape.volume().unwrap(), o.shape.volume().unwrap());
    assert!(r.shape.is_valid() && (v - w).abs() <= 1e-9 * w, "valid {}, volume {v} against OCCT's {w}", r.shape.is_valid());
}

/// A ray crossing a groove's twisted helical flank twice in one of its Bézier pieces (recorded: the Ø10 × 25 knurl
/// at 80° / 30°, its tools 3 and 16): the crossings' search stopped at the first, the count came out odd and a
/// sliver of the cylinder by the end face was taken as inside a groove ("a non-manifold result"). Every start in
/// such a piece now, and the nearest crossing's side has to agree with the count.
#[test]
fn a_ray_crossing_a_twisted_flank_twice() {
    valid_as_occt("5074542e7423ed66", 1e-8);
}

/// Two sheets on one cylinder (a hole's wall in two pieces; recorded, fuzz seed 1192's mirror: model-core asks
/// whether faces overlap by their common): taken as two coplanar sheets are ("sheets that meet").
#[test]
fn a_common_of_two_sheets_on_one_cylinder() {
    valid_as_occt("0dd37c751e8ee2b3", 1e-9);
}

/// Slivers a micrometre wide between two end caps whose edges nearly meet for 330 µm (recorded: the Ø10 × 25 knurl
/// at 45° / 30°, CP30, its tools 21 and 58): every point of one is in the grey band of the other cap's boundary ("a
/// piece's point near a coincident face's boundary"); on the cap's side or not by a curve of it that is a piece of
/// the cap's edge. OCCT's cut of the two keeps one groove; against each groove's own cut and their overlap in the
/// cylinder by OCCT (0.004729 mm³).
#[test]
fn slivers_between_two_caps() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "92dea237e4b5764e").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    let one: Vec<f64> = c.tools.iter().map(|t| k::boolean(c.op, &c.objects, std::slice::from_ref(t), &c.opts).expect("one groove's cut").shape.volume().unwrap()).collect();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v0 = c.objects[0].volume().unwrap();
    let want = v0 - one.iter().map(|v| v0 - v).sum::<f64>() + 0.004729e-9;
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid());
    assert!((v - want).abs() <= 1e-4 * 1e-9, "volume {v}, the grooves' own and their overlap {want}");
}

/// Knurl grooves whose end caps are a hair off the cylinder's end planes (recorded: the Ø100 × 10 knurl at 45° / 30°,
/// its tools 1 and 146, caps 2.3e-8 and 2.8e-5 rad off, within 2.4e-8 of the planes): past the parallel test they
/// weren't coplanar, and the end faces' pieces over the caps were taken as on them by the rays ("a piece's point on
/// another solid's face"); the caps are put on the end planes.
#[test]
fn caps_a_hair_off_the_end_planes() {
    valid_as_occt("9adf79ad7434d5d3", 1e-9);
}

/// A mirror fuse whose face has a hole touching its outer boundary at one vertex (fuzz seed 1144): a wire each side of
/// that vertex, as OCCT's builder makes them (one wire through it twice was invalid: BadOrientationOfSubshape).
#[test]
fn a_hole_touching_the_boundary_is_its_own_wire() {
    valid_as_occt("e02980a2afe11ac3", 1e-9);
}

/// A Shell's common whose new edge on a slightly tilted plane runs through a tolerant vertex off it (fuzz seed 1144):
/// declined to OCCT, whose section runs on the planes' exact intersection (ours, its tolerance raised, was valid but
/// 0.036 % off in volume, and a later sheet metal extrude crashed on it).
#[test]
fn an_edge_through_a_tolerant_vertex_off_a_plane_declines() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "b997c4c389ff7abe").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    assert!(r.is_err() && why == "an edge off a plane past its tolerance", "taken, or declined otherwise: {why}");
}

/// Two coplanar faces overlapping, their common (model-core's face-overlap test; recorded): the object's pieces on the
/// tool's face, as OCCT's (it declined "sheets that meet").
#[test]
fn a_common_of_two_overlapping_coplanar_faces() {
    as_recorded("0b6e553b104c603b");
}
