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

/// A fill pattern's holes: a plate cut by 40 x 40 cylinders through it, its first column notches in its side (so not
/// the holes' own path) (cf-fill-pattern's 1739: its faces' pieces asked against each cylinder's coplanar caps one
/// pair at a time, 7.9 s; and OCCT's check of its faces with 1739 wires each, 0.8 s).
#[test]
#[ignore = "T2: timed pattern Booleans"]
fn a_fill_pattern_of_holes_in_a_plate() {
    let plate = k::make_box_corners(DVec3::ZERO, DVec3::new(0.2, 0.2, 0.004)).unwrap();
    let hole = k::make_cylinder(&z_frame(DVec3::new(0.0, 0.0025, 0.0)), 0.0015, 0.004, TAU).unwrap();
    let holes: Vec<k::Shape> = (0..1600).map(|i| k::transform(&hole, &k::Transform::Translate(DVec3::new(0.005 * (i % 40) as f64, 0.005 * (i / 40) as f64, 0.0)), true).unwrap().shape).collect();
    bench("fill pattern of 1600 holes", k::BooleanOp::Cut, &plate, &holes, &k::BooleanOptions::default());
}

/// A face cut by disks on its plane (a fill pattern's region less its instances, its 1735 disks: OCCT's 0.9 s, ours
/// 0.16 s): the face's pieces on no disk, as OCCT's, history too: the face's untouched edges and vertices Kept by 25
/// disks, Modified by 72 (64 or more: OCCT's path cuts scaled copies of the inputs).
#[test]
fn a_face_cut_by_disks_on_its_plane_as_occt() {
    let _ = k::set_working_scale(1000.0);
    let top = |s: &k::Shape, z: f64| s.faces().into_iter().find(|f| (f.centroid().unwrap().z - z).abs() < 1e-9).expect("its top");
    let face = top(&k::make_box_corners(DVec3::ZERO, DVec3::new(0.06, 0.04, 0.01)).unwrap(), 0.01);
    let near = |a: &std::collections::BTreeMap<String, Vec<Vec<f64>>>, b: &std::collections::BTreeMap<String, Vec<Vec<f64>>>| {
        a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| v.len() == w.len() && v.iter().all(|x| w.iter().any(|y| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| (p - q).abs() <= 1e-9)))))
    };
    for (nx, ny, r) in [(5, 5, 0.002), (9, 8, 0.001)] {
        let disks: Vec<k::Shape> = (0..nx * ny).map(|i| top(&k::make_cylinder(&z_frame(DVec3::new(0.06 * ((i % nx) as f64 + 0.5) / nx as f64, 0.04 * ((i / nx) as f64 + 0.5) / ny as f64, 0.0)), r, 0.01, TAU).unwrap(), 0.01)).collect();
        let run = |rust: bool| {
            k::set_rust_boolean(Some(rust));
            let r = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&face), &disks, &k::BooleanOptions::default()).expect("the cut");
            let path = k::last_boolean_path();
            k::set_rust_boolean(None);
            (r, path)
        };
        let ((ours, path), (occt, _)) = (run(true), run(false));
        assert_eq!(path, "rust", "{}", k::last_rust_decline());
        assert!((ours.shape.area().unwrap() - occt.shape.area().unwrap()).abs() <= 1e-12);
        assert_eq!(ours.shape.count(k::ShapeType::Edge), occt.shape.count(k::ShapeType::Edge));
        assert!(near(&placed_records(&ours), &placed_records(&occt)), "{} disks, history: ours {:?}\nOCCT's {:?}", disks.len(), placed_records(&ours).keys().filter(|k| k.starts_with("arg0")).collect::<Vec<_>>(), placed_records(&occt).keys().filter(|k| k.starts_with("arg0")).collect::<Vec<_>>());
    }
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
/// ~220 s and failed (BOPAlgo_AlertBuilderFailed); fused two at a time in a balanced tree it is live's solid. The
/// STEP's parts in its order: its 38-face body shifted along x (exact copies) and its 3-face body.
#[test]
#[ignore = "T2: ~30 s"]
fn overlapping_free_form_parts_merge_like_onshape() {
    let _ = k::set_working_scale(1000.0);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/mirror/hang402-parts.brep");
    let bodies = k::read_brep(&path).unwrap().solids();
    let parts: Vec<k::Shape> = [Some(18.0), Some(16.0), Some(14.0), Some(12.0), Some(10.0), None, Some(32.0), Some(24.0), Some(16.0), Some(8.0), Some(0.0)]
        .into_iter()
        .map(|dx| match dx {
            Some(dx) => bodies[0].translated(DVec3::new(dx * 1e-3, 0.0, 0.0)).unwrap(),
            None => bodies[1].clone(),
        })
        .collect();
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

/// noshape's Boolean alone on a recorded knurl: valid, OCCT's counts, its volume within `rel` (OCCT's sections are
/// approximations).
fn knurl_as_recorded(name: &str, rel: f64) {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, name).expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid() && (v - c.summary.volume).abs() <= rel * c.summary.volume, "valid {}, volume {v} against OCCT's {}", r.shape.is_valid(), c.summary.volume);
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), [c.summary.faces, c.summary.edges, c.summary.vertices]);
}

/// The Ø25 × 24 CP12 45° / 30° knurl (recorded, 74 grooves): two split points of a section a micrometre apart across
/// it, both within its reach, are one vertex (an 11 nm piece of it joined them: a wire crossing itself, an invalid
/// result past the 2000 faces the build checks).
#[test]
#[ignore = "T2: a 74-groove knurl, its 2592 faces checked"]
fn a_25_by_24_knurl() {
    knurl_as_recorded("3c8b721535de27b6", 1e-8);
}

/// The Ø25 × 25 CP20 45° / 45° knurl (recorded, 124 grooves): section pieces made straight between their vertices
/// get their curves on the faces by projection (the section's curves, by its own parameters, were put on the line's:
/// cylinder pieces open on the cylinder, the result invalid and 0.6 % of its volume off).
#[test]
#[ignore = "T2: a 124-groove knurl, its 12464 faces checked"]
fn a_25_by_25_knurl() {
    knurl_as_recorded("ae71f76b6e894374", 1e-8);
}

/// A document's cuts by a pattern of 8 cutters (recorded, parity): noshape's Boolean declines the first late (two edges
/// leave a vertex one way, found while placing its pieces), so in its feature type's scope the next cut goes straight
/// to OCCT's; OCCT's results both.
#[test]
fn a_late_decline_sends_the_like_straight_to_occt() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let (a, b) = (k::bool_corpus::load(&dir, "71dac1decbc5653c").expect("a case"), k::bool_corpus::load(&dir, "f901b748bdc19fac").expect("a case"));
    let run = |c: &k::bool_corpus::Case| {
        let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("the cut");
        let v = r.shape.volume().unwrap();
        assert!((v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "volume {v} against OCCT's {}", c.summary.volume);
        (k::last_boolean_path(), k::last_rust_decline())
    };
    let memo = std::sync::Arc::new(k::DeclineMemo::default());
    memo.scope("pattern", || {
        assert_eq!(run(&a).1, "two edges leave a vertex in one direction");
        assert_eq!(run(&b).1, "not tried (an earlier path took it)");
    });
    // (Another feature type, or none: tried.)
    memo.scope("another", || assert_eq!(run(&b).1, "two edges leave a vertex in one direction"));
}

/// Parity documents' fuses of flat sheets whose tools share edges (recorded): OCCT's route grouped the tools as apart
/// (their meshes' check misses flat faces sharing an edge) and left them as they were against each other, 47 edges
/// after merging faces where noshape's Boolean, one fuse of them all, gave 16. Now not grouped: either noshape's result
/// is OCCT's or it declines.
#[test]
fn sheet_fuses_with_touching_tools_as_occts_or_declined() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    for name in ["ac1b1ab0f9cd1895", "342606d0dfa32eef"] {
        let c = k::bool_corpus::load(&dir, name).expect("a case");
        k::set_rust_boolean(Some(false));
        let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's fuse");
        k::set_rust_boolean_only(true);
        let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
        let path = k::last_boolean_path();
        k::set_rust_boolean_only(false);
        k::set_rust_boolean(None);
        if path != "rust" {
            continue;
        }
        let r = r.expect("noshape's fuse");
        let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
        assert_eq!(counts(&r.shape), counts(&o.shape), "{name}: noshape's fuse took it, unlike OCCT's");
        assert_eq!(k::bool_corpus::Summary::of(&r).history, k::bool_corpus::Summary::of(&o).history, "{name}: history unlike OCCT's");
    }
}

/// cf-straight-spline's first cut by a pattern of 8 spline cutters (recorded): noshape's Boolean takes it (it declined
/// as invalid: an edge's range a rounding past its curve on a face), OCCT's volume.
#[test]
fn a_straight_spline_cut_taken() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "171341672da7c4f9").expect("a case");
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("the cut");
    assert_eq!(k::last_boolean_path(), "rust", "declined: {}", k::last_rust_decline());
    let v = r.shape.volume().unwrap();
    assert!(r.shape.is_valid() && (v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "volume {v} against OCCT's {}", c.summary.volume);
}

/// cf-planetary-gears' cut of a gear's 42 tooth gaps (recorded, one tool of 254 faces): OCCT's result, and no slower
/// than twice OCCT's time here (the point deep inside the gear's face took 35 distances to each of its 294 curves per
/// probe curve: 7 s a face, 12.8 s against OCCT's 0.5 s).
#[test]
#[ignore = "T2: timed against OCCT"]
fn a_gears_tooth_gaps_cut_as_fast_as_occts() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "8420b23460cb82ee").expect("the recorded case");
    let timed = |rust: bool| {
        k::set_rust_boolean(Some(rust));
        let t = std::time::Instant::now();
        let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
        let (ms, path) = (t.elapsed().as_secs_f64() * 1e3, k::last_boolean_path());
        k::set_rust_boolean(None);
        (r.expect("the cut"), ms, path)
    };
    let _ = timed(true);
    let (ours, ms, path) = timed(true);
    let (_, occt_ms, _) = timed(false);
    assert_eq!(path, "rust");
    let v = ours.shape.volume().unwrap();
    assert!(ours.shape.is_valid() && (v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "volume {v} against OCCT's {}", c.summary.volume);
    assert_eq!(ours.shape.count(k::ShapeType::Face), c.summary.faces);
    assert!(ms <= 2.0 * occt_ms, "{ms:.0} ms, OCCT {occt_ms:.0} ms");
}

/// The Ø25 × 10 CP30 45° / 30° knurl (recorded, 186 grooves): where the cylinder's sections with two grooves' flanks
/// cross (exactly), the flanks' own section, 99 times slower at its end, passes 40 nm off, 17 um before its end; its
/// foot there (the end the nearest sample, Newton's step out of the range) read as its end, 3.8 um off, so it wasn't
/// split there and a flank's strip outside the cylinder joined its piece inside ("a non-manifold result").
#[test]
#[ignore = "T2: a 186-groove knurl, its 17762 faces"]
fn a_25_by_10_knurl_at_30_tpi() {
    knurl_as_recorded("cdf1714d75779e72", 1e-7);
}

/// The Ø25 × 71 CP12 45° / 30° knurl (recorded, 74 grooves): two mirrored grooves' flanks meet in a 32 um curve at the
/// cylinder's end (from their bottom edges' crossing to their ridges') beside the long one OCCT's section gives; it
/// is asked again from between their edges' crossings no curve ends at.
#[test]
#[ignore = "T2: a 74-groove knurl, its 7254 faces"]
fn a_25_by_71_knurl() {
    knurl_as_recorded("5c6ea633a4c1e420", 1e-7);
}

/// The Ø10 × 71 CP12 80° / 30° knurl (recorded, 30 grooves): a ray from a tooth's top crosses a groove's flank at the
/// corner of a patch an eighth of the flank long, which Newton's steps from the patch's middle leave for the
/// surface's continuation (the tooth's top read as inside the groove).
#[test]
#[ignore = "T2: a 30-groove knurl, 13 s"]
fn a_10_by_71_knurl() {
    knurl_as_recorded("613d362cd8436f28", 1e-7);
}

/// cf-spur-gear's cut of 20 tooth gaps from the blank (recorded): each gap's B-spline face lies on the blank's
/// cylinder, which its seam splits where the face isn't; those coincident pieces, all dropped, need no partners (it
/// declined "coincident pieces differ": OCCT's 4.2 s instead of 0.25 s).
#[test]
fn a_gear_blanks_seam_under_a_tooth_gaps_face() {
    as_recorded("9bcf777667a133d5");
}

/// prize-wheel-tim's Revolve 1 (recorded: a Ø9.8 sphere knob fused onto the wheel's top): OCCT's result. A ray's
/// crossing with the sphere came out at u 3e-5 below 0, outside its face until taken a whole turn over; the sliver of
/// the wheel's side under the sphere's section was kept ("a non-manifold result" after ~90 ms, then OCCT's 15 ms).
#[test]
fn a_sphere_knob_fused_onto_a_wheel_as_occt() {
    as_recorded("b2a1c0a5aff95e9f");
}

/// prize-wheel-tim's fuse of three tools with flats at x ±1.2 mm into the hub under its sphere knob (recorded): OCCT's
/// result. The sphere's sections by the flats lie outside the sphere's face: the two faces' parts of a section were
/// apart, and an edge was made backwards between their ends (a 0.2 mm arc splitting a flat; "no point inside a face
/// piece" after ~17 ms, then OCCT's).
#[test]
fn tools_under_a_sphere_knob_fused_as_occt() {
    as_recorded("1b25a77bc927a450");
}

/// cf-fix-pcb's fuse of imported boards (recorded, five of its 51 tools: two of them the same board): their flat faces
/// are B-spline surfaces, read as the planes they are, so the boards' faces lying on each other are found as planes'
/// (one face's piece on the other's was "a piece's point on another solid's face"; OCCT's tree of fuses, 4.3 s).
#[test]
fn boards_with_b_spline_faces_fused_as_occt() {
    as_recorded("042819da42a6ddf5");
}

/// cf-spur-gear's Boolean 1 (recorded: a 186-face gear fused with one tool): OCCT's result. A flank face's angles run
/// over its cylinder's seam angle (3.4 .. 9.1) in the input already; its piece kept them and was declined as across
/// the seam ("a piece across its surface's seam angle" after the whole Boolean, then OCCT's 0.45 s).
#[test]
fn a_gear_fused_as_occt() {
    as_recorded("cef7972794e2ae06");
}

/// cf-fix-pcb's glued fuse of embossed letters onto the board: one letter's trim by the board (recorded: an e-like
/// glyph standing on the board's top face): OCCT's result. A ray from the board's piece under the letter's counter
/// crossed the counter's wall, a 3 mm extruded face over u 0 .. 1, where Newton's steps stall at rounding (3e-14 in u)
/// without counting as converged: the crossing unseen, the piece taken as inside the letter ("a non-manifold result").
#[test]
fn a_letter_on_a_board_cut_as_occt() {
    as_recorded("63881743af8b072f");
}

/// motor-mount-jack's Extrude 3 (recorded: a boss fused onto the tube with four elliptic slots through it): OCCT's
/// result. Each slot is a hole in the tube's cylinders: the region bounded by its loop alone is outside the face, no
/// point deep inside it sought (86 ms against OCCT's 31 before, with the slot walls' ray patches).
#[test]
fn a_tube_with_slots_fused_as_occt() {
    as_recorded("e104843a9d579bfb");
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

/// noshape's Boolean alone on a recorded case, its result valid and against OCCT's on the same inputs now: the same
/// counts, its volume within `rel`.
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
    // (A fuzz part's volume can be negative: a solid turned inside out, both alike.)
    assert!((v - w).abs() <= rel * w.abs(), "volume {v} against OCCT's {w}");
}

/// A valve body's conical seat cut in two by a tool (c2-valve-config, recorded): merging faces after ours leaves the
/// two cone pieces one face as after OCCT's (its unify depends on the faces' order; the cone's pieces are tried in
/// others, as a cylinder's), 37 faces.
#[test]
fn a_cone_cut_in_two_merges_as_after_occts() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "378e9493f981dca8").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    k::set_rust_boolean(Some(false));
    let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's");
    k::set_rust_boolean(None);
    let merged = |s: &k::Shape| {
        let u = k::unify_same_domain(s).expect("merged").shape;
        (u.count(k::ShapeType::Face), u.count(k::ShapeType::Edge))
    };
    assert_eq!(merged(&r.shape), merged(&o.shape));
    assert_eq!(merged(&r.shape).0, 37);
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

/// Two coplanar sheets overlapping in a strip, one's arc tangent to the other's edge at a vertex 0.4 µm off the
/// tangent point (recorded, fuzz seed 1195): the two leave the vertex 2e-5 apart in angle, which ordered them the
/// wrong way round it and lost the face's main piece ("coincident pieces differ"); within a micrometre over their
/// radii they leave it as one, ordered by how they turn.
#[test]
fn an_arc_tangent_to_an_edge_at_a_vertex() {
    valid_as_occt("c8f89e0180653cee", 1e-9);
}

/// Two knurl grooves crossing at the cylinder's seam 0.8 mm above its end (recorded: the Ø25 × 24 knurl at 45° /
/// 30°, its tools 13 and 50): sections crossing within their tolerance of a section's end, the crossings a
/// micrometre apart one vertex ("an invalid result" before).
#[test]
fn grooves_crossing_at_the_seam() {
    valid_as_occt("13187a9d1dd709bc", 1e-9);
}

/// A Shell common whose part has a face of nominal tolerance 1e-5 (its real one 1e-7) and many vertices and edges of
/// 1e-4 (recorded, Shell's fuzz): the tolerant face checked alone against the other argument (nothing in its grey
/// band), the vertices and edges by their own rule ("a tolerant input": every tolerant sub-shape was checked, and
/// past 64 of them declined).
#[test]
fn a_part_with_a_tolerant_face_and_many_tolerant_vertices() {
    valid_as_occt("611959b22ab84eae", 1e-9);
}

/// A drafted plane a few degrees off a hole's axis cutting its cylinder (recorded, fuzz seed 1194's draft): OCCT's
/// long ellipse, taken down to |n · a| = 1e-3 now ("a plane nearly along a cylinder" below 0.25 before).
#[test]
fn a_plane_a_few_degrees_off_a_cylinders_axis() {
    valid_as_occt("a9a9f9b663091ab2", 1e-9);
}

/// Two coplanar sheets, a vertex of one of tolerance 1e-4 lying 1e-4 from the other's edge (recorded, fuzz seed
/// 1195's mirror): on that edge by the sum of the two's tolerances, while the edges' crossing there, a new point,
/// wasn't merged into it (the sum with a new point's tolerance, its curves', is that reach too): "coincident pieces
/// differ".
#[test]
fn a_tolerant_vertex_at_its_reach_from_an_edge() {
    valid_as_occt("760ca2f4c60b4f9a", 1e-9);
}

/// Two sheets on one torus, its axis one way for one and the other way for the other (recorded, fuzz seed 1203's
/// mirror asking whether two fillet faces overlap): taken as two coplanar sheets are ("sheets that meet").
#[test]
fn a_common_of_two_sheets_on_one_torus() {
    valid_as_occt("1fcd43055b140300", 1e-9);
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

/// Shell's slab fuse (recorded): a part's vertex where its hole's or fillet's arc ends lies inside coincident edges of
/// two slabs. OCCT's history says Deleted for it (its copy, made for the edges, merges again with a section's vertex
/// there, and PrepareHistory follows one same-domain link only); between planes it says Modified. Ours as OCCT's:
/// each record (input, relation, output kind) as many times.
#[test]
fn a_vertex_at_an_arc_inside_two_slabs_edges_is_deleted_as_occt() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "a18c862ab40aa82b").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    k::set_rust_boolean(Some(false));
    let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's");
    k::set_rust_boolean(None);
    let counts = |x: &k::OpResult| placed_records(x).into_iter().map(|(key, v)| (key, v.len())).collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(counts(&r), counts(&o));
}

/// Shell's slab common on a part with a cone cut by a plane (recorded): its edges on the hyperbola and the face
/// extruded from it taken as cubic B-splines through the conic at its own parameters (it declined "a conic edge",
/// then "an extruded face"); valid, OCCT's counts and volume.
#[test]
fn hyperbola_edges_and_a_face_extruded_from_one() {
    valid_as_occt("6abd9f7169b73894", 1e-9);
}

/// A Shell's common (qa's cli-shell-gpf, Shell 2; recorded) with a vertex 44 um off its face's plane, its tolerance
/// 0.12 mm: declined to OCCT (taken with the merged vertex at the first one's point it came out 3.3e-7 off OCCT's in
/// volume, and the Shell failed: RUST-SHELL-GPF-REGRESS; at OCCT's merged vertex's point, the slab's, the part's face
/// and the slab's split differently).
#[test]
fn a_tolerant_vertex_off_its_faces_plane_declines() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "54630d70bb1b6e23").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    assert!(r.is_err() && why == "coincident pieces differ", "taken, or declined otherwise: {why}");
}

/// Shell's slab fuse (recorded) with tolerant vertices off their faces' planes within their own tolerances (up to
/// 1 mm taken, as OCCT's): valid, OCCT's counts and volume.
#[test]
fn a_tolerant_vertex_off_its_faces_plane() {
    valid_as_occt("952c2efc54541e86", 1e-9);
}

/// Shell's slab common (recorded): two of the slab's vertices (tolerance 0.1 mm) 0.14 mm apart and the part's point
/// within both, one vertex as OCCT's (its vertices interfering in a chain merge as one), the slab's edge between the
/// two OCCT's micro edge, deleted (it declined "a point near two vertices"); valid, OCCT's counts and volume.
#[test]
fn a_group_of_vertices_one_vertex() {
    valid_as_occt("d6d166ba28b54cd2", 1e-9);
}

/// Shell's fuse of a part with 47 slabs (recorded; free-form faces among them: op_boolean's tree of fuses two at a
/// time): OCCT's path keeps every tool's volume, the union's 6.1536e-5 m³ (the tree's leaves were group_disjoint's
/// compounds of tools apart, and OCCT's fuses of those lost a slab's: 5.755e-5).
#[test]
fn occts_tree_of_fuses_keeps_every_tool() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "bf976715f0288163").expect("the recorded case");
    k::set_rust_boolean(Some(false));
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's Boolean");
    k::set_rust_boolean(None);
    let v = r.shape.volume().unwrap();
    assert!((v - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "volume {v} against the union's {}", c.summary.volume);
}

/// Holes through a boss in a mirrored plate, fused (recorded): each hole's wall is one cylinder in faces of both
/// inputs whose seams are half a turn apart, the plate's halves left-handed. noshape's Boolean takes them (it declined
/// "their seams apart"), and the model's unify gives each wall one face, ours and OCCT's alike, as live (1767: the
/// second hole with a tube in it, 12 faces; OCCT's own unify left 14).
#[test]
fn holes_through_a_boss_in_a_mirrored_plate_unify_as_live() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    for (name, faces) in [("38b2cd00e7f8e88c", 9), ("fd7ef1286d0cf8d5", 12)] {
        let c = k::bool_corpus::load(&dir, name).expect("the recorded case");
        k::set_rust_boolean_only(true);
        let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
        let why = k::last_rust_decline();
        k::set_rust_boolean_only(false);
        let r = r.unwrap_or_else(|e| panic!("{name}: noshape's Boolean declined: {why} ({e})"));
        k::set_rust_boolean(Some(false));
        let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's");
        k::set_rust_boolean(None);
        for (who, x) in [("ours", &r), ("OCCT's", &o)] {
            let u = k::unify_same_domain(&x.shape).expect("unified").shape;
            assert!(u.is_valid(), "{name}: {who} unified invalid");
            assert_eq!(u.count(k::ShapeType::Face), faces, "{name}: {who} unified");
            assert!((u.volume().unwrap() - c.summary.volume).abs() <= 1e-9 * c.summary.volume, "{name}: {who} volume");
        }
    }
}

/// Shell's fuse of 35 slabs (6e9bc680), our result (tests/data/unify): a part of a cylinder's wall in faces of two
/// inputs whose frames differ, one loop round them. The model's unify gives OCCT's counts (14 faces, 30 edges), valid
/// (it stopped at 15 faces, 39 edges on ours).
#[test]
fn a_part_of_a_wall_in_two_inputs_faces_unifies_as_occts() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify/half-wall-in-two-inputs.brep");
    let ours = k::read_brep(&path).expect("our result");
    let u = k::unify_same_domain(&ours).expect("unified").shape;
    assert!(u.is_valid(), "unified invalid");
    assert_eq!((u.count(k::ShapeType::Face), u.count(k::ShapeType::Edge)), (14, 30));
}

/// Shell's fuse of 38 slabs (37e806ac), our result (tests/data/unify): faces on one cylinder whose frames differ, a
/// B-spline edge of their boundary a rounding past its curve's end. The model's unify doesn't fail (the merge's
/// projection of that edge threw: unify_same_domain failed), valid, the volume kept.
#[test]
fn a_b_spline_edge_past_its_curves_end_unifies() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify/b-spline-edge-past-its-curve.brep");
    let ours = k::read_brep(&path).expect("our result");
    let u = k::unify_same_domain(&ours).expect("unified").shape;
    assert!(u.is_valid(), "unified invalid");
    let (v, w) = (u.volume().unwrap(), ours.volume().unwrap());
    assert!((v - w).abs() <= 1e-9 * w.abs(), "volume {v} against {w}");
}

/// Shell's fuse of 38 slabs (1b121fc3), our result (tests/data/unify): walls on one cylinder in faces of one frame
/// that USD leaves apart (its result depending on the faces' order and wires' starts). Merged after it as those whose
/// frames differ: OCCT's counts (32 faces, 77 edges), valid (it stopped at 33 / 80 on ours).
#[test]
fn walls_usd_leaves_apart_are_merged_after_it() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify/walls-usd-leaves-apart.brep");
    let ours = k::read_brep(&path).expect("our result");
    let u = k::unify_same_domain(&ours).expect("unified").shape;
    assert!(u.is_valid(), "unified invalid");
    assert_eq!((u.count(k::ShapeType::Face), u.count(k::ShapeType::Edge)), (32, 77));
}

/// c2-sm-louver's Linear pattern 3 fuse (bool's trace, 11-06), its result (tests/data/unify): 1382 faces, invalid (ten
/// overlapping copies of the panel from an upstream sheet-metal difference). Our unify took 124-156 s on it: the
/// cylinder-patch merge checked the whole shape for each of ~300 pairs (an invalid input never passes), and its three
/// reordered tries did it again. Now ~1.2 s; the bound is generous (any machine), the old cost far past it.
#[test]
fn an_invalid_fuse_result_unifies_in_bounded_time() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify/louver-fuse-invalid.brep");
    let ours = k::read_brep(&path).expect("the fuse's result");
    let t = std::time::Instant::now();
    let u = k::unify_same_domain(&ours).expect("unified").shape;
    let secs = t.elapsed().as_secs_f64();
    assert_eq!((ours.count(k::ShapeType::Face), u.count(k::ShapeType::Face)), (1382, 1173), "USD's merges as before");
    assert!(secs < 30.0, "unify took {secs:.1} s");
}

/// DRAFT-NONDETERMINISTIC (qa's cli-draft-hang2): the face deletion in a Draft's fallback (faces 5, 14, 15 of the
/// dumped shape) timed: past its 10 s CPU budget here, under it on vibe5, so the Draft's result depended on the machine.
#[test]
#[ignore = "T2 probe: timing"]
fn draft_fallback_face_deletion_probe() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/draft-defeature-slow.brep");
    let s = k::read_brep(&path).expect("the shape");
    let faces: Vec<k::Shape> = [5usize, 14, 15].iter().map(|&i| s.sub_shape(k::ShapeType::Face, i).unwrap()).collect();
    let t = std::time::Instant::now();
    let r = k::delete_faces(&s, &faces);
    println!("delete faces: {:?} in {:.2} s", r.as_ref().map(|r| r.shape.count(k::ShapeType::Face)).map_err(|e| e.to_string()), t.elapsed().as_secs_f64());
}

/// Shell's common with hyperbola edges (14218a58), our result (tests/data/unify): walls on one cylinder USD leaves
/// apart, merged after it, and USD's joining of their boundary's edges then left the shape invalid. That merge is
/// dropped then: valid, OCCT's counts (42 faces, 100 edges).
#[test]
fn a_merge_after_usd_left_invalid_is_dropped() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify/hyperbola-edges-walls.brep");
    let ours = k::read_brep(&path).expect("our result");
    let u = k::unify_same_domain(&ours).expect("unified").shape;
    assert!(u.is_valid(), "unified invalid");
    assert_eq!((u.count(k::ShapeType::Face), u.count(k::ShapeType::Edge)), (42, 100));
}

/// qa's cli-shell-gpf part fused with its mirror (11ed9bd7), our result (tests/data/unify): each Ø12 hole's wall in
/// three faces of the two inputs (frames apart). The model's unify gives each one face, OCCT's counts (33 faces;
/// live 1767: a hole's wall one face, mirrored or not); the merge's volume check is on the faces it changes (the whole
/// solid's integral was 2.9e-7 off with itself and refused it: 37 faces).
#[test]
fn a_part_fused_with_its_mirror_unifies_its_holes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify/part-fused-with-its-mirror.brep");
    let ours = k::read_brep(&path).expect("our result");
    let u = k::unify_same_domain(&ours).expect("unified").shape;
    assert!(u.is_valid(), "unified invalid");
    assert_eq!(u.count(k::ShapeType::Face), 33);
}

/// A cylinder cut by a box whose side runs along its axis but for a 1e-9 rad tilt (Shell's slabs of a part's walls:
/// |n · a| 2.4e-9): along it within a tenth of the tolerance over the face, its two rulings (it declined "a plane
/// nearly along a cylinder"; OCCT's ellipse there is r / |n · a| long). Taken, valid, OCCT's volume and counts.
#[test]
fn a_plane_along_a_cylinder_but_for_rounding() {
    let _ = k::set_working_scale(1000.0);
    let cyl = k::make_cylinder(&z_frame(DVec3::ZERO), 0.005, 0.02, TAU).unwrap();
    let block = k::make_box_corners(DVec3::new(0.002, -0.01, -0.005), DVec3::new(0.01, 0.01, 0.025)).unwrap();
    let block = k::transform(&block, &k::Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::Y, angle: 1e-9 }, true).unwrap().shape;
    let opts = k::BooleanOptions::default();
    k::set_rust_boolean_only(true);
    let r = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&cyl), std::slice::from_ref(&block), &opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    k::set_rust_boolean(Some(false));
    let o = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&cyl), std::slice::from_ref(&block), &opts).expect("OCCT's");
    k::set_rust_boolean(None);
    assert!(r.shape.is_valid());
    assert_eq!(r.shape.count(k::ShapeType::Face), o.shape.count(k::ShapeType::Face));
    let (v, w) = (r.shape.volume().unwrap(), o.shape.volume().unwrap());
    assert!((v - w).abs() <= 1e-9 * w, "volume {v} against OCCT's {w}");
}

/// Shell's slab common on a part with a vertex inside a face (INTERNAL, on no wire; recorded): left out, as OCCT's
/// result keeps none in the faces it splits either (it declined "a vertex in a face"); valid, OCCT's counts and volume.
#[test]
fn a_vertex_inside_a_face() {
    valid_as_occt("a7bdd15ba4f8613a", 1e-9);
}

/// A Shell slab's common (recorded) with a thin piece whose middle is within the Boolean's tolerance of its face's
/// boundary but clear of the nearest edges' own: in, as OCCT's (it declined "a piece's point near its face's
/// boundary"). Valid, OCCT's counts and volume.
#[test]
fn a_sliver_between_exact_edges_of_a_tolerant_slab() {
    valid_as_occt("d0e9929845ef66b4", 1e-9);
}

/// Shell's slab common with a plane at an angle to a cone steeper than its rulings (recorded): a hyperbola, its axes
/// from where the plane meets the two rulings in the plane of the axis and its normal, drawn on the cone point by
/// point (it declined "a plane at an angle to a cone"); valid, OCCT's counts and volume.
#[test]
fn a_plane_steeper_than_a_cones_rulings() {
    valid_as_occt("45794f15a71f879f", 1e-9);
}

/// A cone tilted 0.2 rad cut by a box whose side is a plane steeper than its rulings exactly through its apex: where the
/// plane meets the two rulings in the plane of the axis and its normal is the apex for both (0 along each; it
/// declined "a plane at an angle to a cone"); its two rulings there. Taken, valid, OCCT's volume and counts.
#[test]
fn a_steep_plane_exactly_through_a_cones_apex() {
    let _ = k::set_working_scale(1000.0);
    let cone = k::make_cone(&z_frame(DVec3::ZERO), 0.0, 0.005, 0.01, TAU).unwrap();
    let cone = k::transform(&cone, &k::Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::Y, angle: 0.2 }, true).unwrap().shape;
    let block = k::make_box_corners(DVec3::new(0.0, -0.01, -0.01), DVec3::new(0.02, 0.01, 0.02)).unwrap();
    let opts = k::BooleanOptions::default();
    k::set_rust_boolean_only(true);
    let r = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&cone), std::slice::from_ref(&block), &opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    k::set_rust_boolean(Some(false));
    let o = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&cone), std::slice::from_ref(&block), &opts).expect("OCCT's");
    k::set_rust_boolean(None);
    assert!(r.shape.is_valid());
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), counts(&o.shape));
    let (v, w) = (r.shape.volume().unwrap(), o.shape.volume().unwrap());
    assert!((v - w).abs() <= 1e-9 * w, "volume {v} against OCCT's {w}");
}

/// A cut of a threaded rod (recorded): its cylinder's faces between the thread's turns go round between rulings at one
/// angle (helical strips, no seam), th0 there with the rulings its loop runs up on the far side (it declined "a
/// cylinder face without curved edges"); a piece's end a tolerance round the axis past a turn is not across the
/// seam (3e-7 rad on Ø9.4). Valid, OCCT's counts and volume.
#[test]
fn a_threaded_rods_helical_faces() {
    valid_as_occt("308a667d2f24d313", 1e-9);
}

/// Shell's slab common on a part with countersinks (cone apexes) and tilted holes (ellipse edges) (recorded): each
/// ellipse edge takes its own minor radius (the body's minor radii skipped the apex edges, so an ellipse after one read
/// another edge's: 0 there made it "a degenerate edge between two vertices", else a wrong ellipse); valid, OCCT's
/// counts and volume.
#[test]
fn ellipse_edges_after_a_cones_apex() {
    valid_as_occt("b200c5bfa89fcb3d", 1e-9);
}

/// A Ø10 × 10 diamond knurl, 12 TPI 45° / 45° (recorded; 30 grooves): split points of a curve nearer along it than
/// the tolerance are one vertex, as OCCT's pave (two crossings a micrometre apart across a section joined by an 11 nm
/// piece of it: "an invalid result"). Valid, and the volume of OCCT's cut groove by groove (6.77883e-7 m³; its one
/// cut of all of them is invalid, 1.16e-6).
#[test]
fn a_ten_millimetre_knurl() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "5099e897b3caf1fa").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    assert!(r.shape.is_valid());
    let v = r.shape.volume().unwrap();
    assert!((v - 6.77883072e-7).abs() <= 1e-5 * 6.77883072e-7, "volume {v}");
}

/// cf-straight-spline's SAE Straight-Sided Spline feature (recorded): a shaft cut by four slots and their run-outs
/// with the model's many-tools options, unified as the model does; faces and shown edges / vertices.
fn spline_shaft(n: u32) -> (k::Shape, (usize, usize, usize)) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let target = k::read_brep(&dir.join(format!("spline-shaft-{n}-target.brep"))).expect("the shaft");
    let tools = k::read_brep(&dir.join(format!("spline-shaft-{n}-tools.brep"))).expect("the cutters").solids();
    let opts = k::BooleanOptions { glue: k::Glue::ShiftChecked, obb: true, parallel: true, ..Default::default() };
    let r = k::boolean(k::BooleanOp::Cut, &[target], &tools, &opts).expect("cut");
    let s = k::unify_same_domain(&r.shape).expect("unified").shape;
    let t = s.shown_topology().expect("shown topology");
    let counts = (s.count(k::ShapeType::Face), t.edges.iter().filter(|&&x| x).count(), t.vertices.iter().filter(|&&x| x).count());
    (s, counts)
}

/// The first shaft (live: Part 1 31 / 77 / 48, 38747.8297 mm³): the run-out across the shaft cylinder's seam was two
/// edges (the seam's vertex between them; live's faces have no seams): the seam moved to a run-out's vertex.
#[test]
fn a_spline_shafts_run_out_across_its_seam_is_one_edge() {
    let (s, counts) = spline_shaft(1);
    assert!(s.is_valid());
    assert_eq!(counts, (31, 77, 48));
    let v = s.volume().unwrap() * 1e9;
    assert!((v / 38747.8297 - 1.0).abs() < 1e-6, "{v}");
}

/// prize-wheel-tim's knob (live 20 / 49 / 32): a Ø3.76 pin with a slot 2.4 wide and 0.2 deep across its top. Where
/// the cylinder's seam is, it splits an arc of the top (one edge too many); it moves to a slot corner, whose short
/// edge up the slot's wall runs on from the new seam, and the arc is whole. Either seam: 13 edges, 8 vertices shown.
#[test]
fn a_pins_seam_moves_to_a_slot_corner_with_an_edge_along_it() {
    let mm = 1e-3;
    for x_dir in [DVec3::X, DVec3::Y] {
        let pin = k::make_cylinder(&k::Plane::new(DVec3::ZERO, DVec3::Z, x_dir), 1.88 * mm, 20.0 * mm, TAU).unwrap();
        let slot = k::make_box_corners(DVec3::new(-1.2, -3.0, 19.8) * mm, DVec3::new(1.2, 3.0, 21.0) * mm).unwrap();
        let cut = k::cut(&pin, &slot).unwrap().shape;
        let u = k::unify_same_domain(&cut).unwrap().shape;
        assert!(u.is_valid());
        let shown = u.shown_topology().unwrap();
        let counts = (u.count(k::ShapeType::Face), shown.edges.iter().filter(|e| **e).count(), shown.vertices.iter().filter(|v| **v).count());
        assert_eq!(counts, (7, 13, 8), "seam along {x_dir}");
        let v = u.volume().unwrap();
        let want = std::f64::consts::PI * 1.88 * 1.88 * 20.0 * mm.powi(3) - cut_slot_volume(1.88, 1.2, 0.2) * mm.powi(3);
        assert!((v / want - 1.0).abs() < 1e-6, "{v} vs {want}");
    }
}

/// The volume a slot `2 w` wide and `d` deep takes off the top of a pin of radius `r` (the circle's chord strip).
fn cut_slot_volume(r: f64, w: f64, d: f64) -> f64 {
    let half = (r * r - w * w).sqrt();
    2.0 * (w * half + r * r * (w / r).asin()) * d
}

/// The second shaft (live: Part 2 31 / 77 / 48, 52687.29 mm³): each slot's flat side and its run-out's cone touch
/// along the profile edge, and OCCT's cut by all eight kept the cone's half inside the slot (an edge on three faces:
/// "would result in non-manifold body", where live builds it): cut by the tools fused instead.
#[test]
fn a_spline_shafts_run_outs_tangent_to_its_slots_cut_manifold() {
    let (s, counts) = spline_shaft(2);
    assert!(s.is_valid());
    let ef = s.edge_faces().unwrap();
    assert!(ef.iter().all(|f| {
        let mut f = f.clone();
        f.sort_unstable();
        f.dedup();
        f.len() <= 2
    }));
    assert_eq!(counts, (31, 77, 48));
    let v = s.volume().unwrap() * 1e9;
    assert!((v / 52687.29 - 1.0).abs() < 1e-6, "{v}");
}

/// motor-mount-jack's Extrude 3 cut (recorded before its unify): a tube with a slot of two ellipses and lines. The
/// seam move took the ellipses' tangent lines for a seam's split and hopped from vertex to vertex: 3.4 s (17 ms
/// before). Nothing to move: quick, and the shape as it was.
#[test]
fn a_slot_of_ellipses_and_tangent_lines_keeps_its_seam_quickly() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify/motor-mount-slots.brep");
    let s = k::read_brep(&path).expect("the cut");
    let t = Instant::now();
    let u = k::unify_same_domain(&s).expect("unified").shape;
    let ms = t.elapsed().as_secs_f64() * 1e3;
    assert!(u.is_valid());
    assert!(ms < 1000.0, "{ms:.0} ms");
    assert_eq!((u.count(k::ShapeType::Face), u.count(k::ShapeType::Edge)), (s.count(k::ShapeType::Face), s.count(k::ShapeType::Edge)));
}

/// SEAM-MOVER-UNIFY-VOLUME (bool's corpus4 0662365385035891, a fuse): noshape's and OCCT's results are one part (their
/// volumes equal to 1e-15), and unified they must stay one: the unify moves each one's cylinder seam to another vertex,
/// and the volume integral read the wall whose u ran on [5.30, 11.58] 8.6e-6 off. Both unified agree to 1e-9, and
/// either unified twice gives the same volume, bit for bit.
#[test]
fn equal_results_unify_to_equal_volumes() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/unify");
    let c = k::bool_corpus::load(&dir, "seam-mover-0662365385035891").expect("the case");
    let run = |rust: bool| {
        k::set_rust_boolean(Some(rust));
        let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("the fuse").shape;
        k::set_rust_boolean(None);
        r
    };
    let unified_volume = |s: &k::Shape| k::unify_same_domain(s).expect("unified").shape.volume().unwrap();
    let (ours, occt) = (run(true), run(false));
    let (vo, vc) = (unified_volume(&ours), unified_volume(&occt));
    assert!((vo / vc - 1.0).abs() < 1e-9, "ours unified {vo}, OCCT's {vc}");
    assert_eq!(unified_volume(&ours).to_bits(), vo.to_bits());
    assert_eq!(unified_volume(&occt).to_bits(), vc.to_bits());
}

/// A plate with two holes split by a planar face across it (Split part; cf-fix-pcb's Fix PCB splits its board so):
/// noshape's Boolean (the plate's common and cut with the half-space) as OCCT's Splitter: two valid solids of its
/// volumes, the plate's faces Modified, kept or gone alike, the plane face into the cut faces.
#[test]
fn a_plate_split_by_a_plane_as_occts_splitter() {
    let _ = k::set_working_scale(1000.0);
    let opts = k::BooleanOptions::default();
    let plate = k::make_box_corners(DVec3::ZERO, DVec3::new(0.04, 0.02, 0.006)).unwrap();
    let hole = |x: f64| k::make_cylinder(&z_frame(DVec3::new(x, 0.01, -0.001)), 0.003, 0.008, TAU).unwrap();
    let plate = k::boolean(k::BooleanOp::Cut, &[plate], &[hole(0.01), hole(0.03)], &opts).unwrap().shape;
    let slab = k::make_box_corners(DVec3::new(-0.01, -0.01, 0.002), DVec3::new(0.05, 0.03, 0.003)).unwrap();
    let tool = (0..slab.count(k::ShapeType::Face))
        .map(|i| slab.sub_shape(k::ShapeType::Face, i).unwrap())
        .find(|f| (f.centroid().unwrap().z - 0.002).abs() < 1e-9)
        .expect("the slab's bottom face");
    let ours = k::split(std::slice::from_ref(&plate), std::slice::from_ref(&tool), &opts).unwrap();
    k::set_rust_boolean(Some(false));
    let occt = k::split(std::slice::from_ref(&plate), std::slice::from_ref(&tool), &opts).unwrap();
    k::set_rust_boolean(None);
    assert!(ours.shape.is_valid());
    let vols = |s: &k::Shape| {
        let mut v: Vec<f64> = s.sub_shapes(k::ShapeType::Solid).iter().map(|x| x.volume().unwrap()).collect();
        v.sort_by(f64::total_cmp);
        v
    };
    let (a, b) = (vols(&ours.shape), vols(&occt.shape));
    assert_eq!(a.len(), 2);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() <= 1e-9 * y, "volume {x} against OCCT's {y}");
    }
    // (Which input faces each says Modified / Deleted; OCCT shares a cut face between the two solids, ours gives
    // each its own, so the images' counts differ there.)
    let rel = |r: &k::OpResult| {
        let mut v: Vec<(u32, u32, String)> = r.history.records.iter().filter(|h| h.input_kind == k::ShapeType::Face).map(|h| (h.arg, h.input_index, format!("{:?}", h.relation))).collect();
        v.sort();
        v.dedup();
        v
    };
    assert_eq!(rel(&ours), rel(&occt));
}

/// c2-sm-louver's Mirror 2 (from skasm): the mirrored half's bend-end cutter is inside out (its volume below zero, a
/// mirror's image not turned back). Ours declines it first ("an inside-out tool", no section computed), not after
/// 0.35 s of work ("no clean ray"); OCCT then cuts it as it is.
#[test]
fn an_inside_out_tool_declined_first() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let read = |n: &str| k::Shape::from_brep_bytes(&std::fs::read(dir.join(n)).unwrap()).unwrap();
    let (bend, cutter) = (read("louver-mirror2-bend.brep"), read("louver-mirror2-cutter.brep"));
    assert!(cutter.volume().unwrap() < 0.0);
    k::set_rust_boolean_only(true);
    let r = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&bend), std::slice::from_ref(&cutter), &k::BooleanOptions::default());
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    assert!(r.is_err());
    assert_eq!(why, "an inside-out tool");
    let o = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&bend), std::slice::from_ref(&cutter), &k::BooleanOptions::default()).expect("OCCT's");
    assert!(o.shape.is_valid());
}

/// A part cut by 12 copies of a filleted tool (parity56 1abdab7d, recorded): its tools' tori and spheres taken as
/// copies of one another made ours give 123 edges where OCCT gives 117; found as copies only by their cylinders,
/// cones, extrusions and revolutions, the result is OCCT's (ours declines it).
#[test]
fn copies_of_a_filleted_tool_cut_as_occt() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "1abdab7d91d58819").expect("the recorded case");
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("the cut");
    k::set_rust_boolean(Some(false));
    let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's");
    k::set_rust_boolean(None);
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&r.shape), counts(&o.shape));
    let (v, w) = (r.shape.volume().unwrap(), o.shape.volume().unwrap());
    assert!((v - w).abs() <= 1e-9 * w.abs(), "volume {v} against OCCT's {w}");
}

/// A plate cut by two long rods through it 20 mm apart, tilted to cross 100 mm below it (big cutters of a pattern
/// meeting past the part, as c2-valve-config's handles): ours one rod at a time, the same as OCCT's cut by both (its
/// faces, volume, and which input faces it says Modified, kept or Deleted).
#[test]
fn rods_meeting_below_a_plate_cut_one_at_a_time_as_occt() {
    let _ = k::set_working_scale(1000.0);
    let plate = k::make_box_corners(DVec3::ZERO, DVec3::new(0.04, 0.02, 0.01)).unwrap();
    let rod = |x: f64, lean: f64| {
        let a = DVec3::new(lean, 0.0, 1.0).normalize();
        let c = DVec3::new(x, 0.01, 0.005);
        k::make_cylinder(&k::Plane::new(c - a * 0.15, a, DVec3::Y), 0.003, 0.3, TAU).unwrap()
    };
    let tools = [rod(0.01, -0.1), rod(0.03, 0.1)];
    let opts = k::BooleanOptions::default();
    k::set_rust_boolean_only(true);
    let ours = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&plate), &tools, &opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let ours = ours.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})"));
    k::set_rust_boolean(Some(false));
    let occt = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&plate), &tools, &opts).unwrap();
    k::set_rust_boolean(None);
    assert!(ours.shape.is_valid());
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&ours.shape), counts(&occt.shape));
    let (v, w) = (ours.shape.volume().unwrap(), occt.shape.volume().unwrap());
    assert!((v - w).abs() <= 1e-9 * w, "volume {v} against OCCT's {w}");
    let rel = |r: &k::OpResult| {
        let mut v: Vec<(u32, u32, String)> = r.history.records.iter().filter(|h| h.input_kind == k::ShapeType::Face).map(|h| (h.arg, h.input_index, format!("{:?}", h.relation))).collect();
        v.sort();
        v.dedup();
        v
    };
    assert_eq!(rel(&ours), rel(&occt));
}

/// Two rods crossing inside a plate (and past it): not one at a time (the second meets the first's walls), the cut by
/// both at once, as OCCT's.
#[test]
fn rods_crossing_inside_a_plate_cut_at_once_as_occt() {
    let _ = k::set_working_scale(1000.0);
    let plate = k::make_box_corners(DVec3::ZERO, DVec3::new(0.04, 0.02, 0.01)).unwrap();
    let rod = |lean: f64| {
        let a = DVec3::new(lean, 0.0, 1.0).normalize();
        let c = DVec3::new(0.02, 0.01, 0.005);
        k::make_cylinder(&k::Plane::new(c - a * 0.15, a, DVec3::Y), 0.003, 0.3, TAU).unwrap()
    };
    let tools = [rod(-0.5), rod(0.5)];
    let opts = k::BooleanOptions::default();
    let ours = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&plate), &tools, &opts).unwrap();
    k::set_rust_boolean(Some(false));
    let occt = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&plate), &tools, &opts).unwrap();
    k::set_rust_boolean(None);
    assert!(ours.shape.is_valid());
    let counts = |s: &k::Shape| [k::ShapeType::Face, k::ShapeType::Edge, k::ShapeType::Vertex].map(|t| s.count(t));
    assert_eq!(counts(&ours.shape), counts(&occt.shape));
    let (v, w) = (ours.shape.volume().unwrap(), occt.shape.volume().unwrap());
    assert!((v - w).abs() <= 1e-9 * w, "volume {v} against OCCT's {w}");
}

/// Two solids meeting on one planar face each, the same outline (a part split on a plane and fused again: cf-part-
/// draft's drafted gear halves on z 0): the two shells sewn along it, without a Boolean; OCCT's counts (no unify) and
/// the sum of the volumes. Shifted half off the other's face, they aren't glued. (Planes and cylinders go to
/// noshape's Boolean first: switched off here, the glued fuse comes before OCCT's.)
#[test]
fn cylinders_meeting_on_a_whole_face_fuse_glued() {
    k::set_rust_boolean(Some(false));
    let (r, h) = (0.01, 0.02);
    let up = k::make_cylinder(&z_frame(DVec3::ZERO), r, h, TAU).unwrap();
    let down = k::make_cylinder(&z_frame(DVec3::new(0.0, 0.0, -h)), r, h, TAU).unwrap();
    let f = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&up), &[down], &k::BooleanOptions::default()).unwrap();
    assert_eq!(k::last_boolean_path(), "glued");
    assert!(f.shape.is_valid());
    assert_eq!((f.shape.count(k::ShapeType::Solid), f.shape.count(k::ShapeType::Face)), (1, 4));
    let v = f.shape.volume().unwrap();
    assert!((v / (2.0 * std::f64::consts::PI * r * r * h) - 1.0).abs() < 1e-9, "{v}");
    let aside = k::make_cylinder(&z_frame(DVec3::new(r, 0.0, -h)), r, h, TAU).unwrap();
    let g = k::boolean(k::BooleanOp::Fuse, &[up], &[aside], &k::BooleanOptions::default()).unwrap();
    assert_ne!(k::last_boolean_path(), "glued");
    assert!(g.shape.is_valid());
    k::set_rust_boolean(None);
}

/// c2-valve-config's Extrude 2 (recorded): a valve body cut by a handle whose end plane crosses the seat cone's axis
/// past its apex, and whose face's section with the body's chamfer cone (OCCT's, of 0.206 mm) ends 0.2065 mm off where
/// it leaves the cone's rim: no section on the other nappe, the section's end the rim's vertex (OCCT's bound pave);
/// taken, as OCCT's (it declined "a plane through a cone's apex", then "an invalid result" from a 0.2 mm sliver).
#[test]
fn a_valve_handle_cut_past_a_cones_apex_as_occt() {
    valid_as_occt("81493b18e2677a24", 1e-9);
}

/// c2-valve-config's Circular pattern 1 (recorded): the valve body cut by 7 handle tools at once. Where the -90°
/// handle's countersink meets the 0.036 mm chamfer strip over the flat at y = -70 mm, live keeps two vertices on each
/// side, joined by a 0.0712 mm edge (REQUESTS 2210: (±2.9371, -70, 0.0356) and (±2.9318, -69.9386, 0) mm); OCCT joins
/// them on one side only, an edge and a vertex fewer. Ours is live's: OCCT's faces and volume, one edge and one
/// vertex more, the short edge on both sides (the gate's recorded exception).
#[test]
fn a_valve_handles_pattern_keeps_both_short_edges_as_live() {
    let _ = k::set_working_scale(1000.0);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/booleans");
    let c = k::bool_corpus::load(&dir, "42cf72780eec3242").expect("the recorded case");
    k::set_rust_boolean_only(true);
    let r = k::boolean(c.op, &c.objects, &c.tools, &c.opts);
    let why = k::last_rust_decline();
    k::set_rust_boolean_only(false);
    let r = r.unwrap_or_else(|e| panic!("noshape's Boolean declined: {why} ({e})")).shape;
    k::set_rust_boolean(Some(false));
    let o = k::boolean(c.op, &c.objects, &c.tools, &c.opts).expect("OCCT's").shape;
    k::set_rust_boolean(None);
    assert!(r.is_valid());
    assert_eq!(r.count(k::ShapeType::Face), o.count(k::ShapeType::Face));
    assert_eq!((r.count(k::ShapeType::Edge), r.count(k::ShapeType::Vertex)), (o.count(k::ShapeType::Edge) + 1, o.count(k::ShapeType::Vertex) + 1));
    let (v, w) = (r.volume().unwrap(), o.volume().unwrap());
    assert!((v - w).abs() <= 1e-8 * w, "volume {v} against OCCT's {w}");
    for x in [-1.0, 1.0] {
        let at = DVec3::new(x * 2.93442e-3, -69.96929e-3, 0.01778e-3);
        let short = r.edges().into_iter().any(|e| (e.length().unwrap() - 0.07117e-3).abs() < 0.5e-6 && e.edge_info().is_ok_and(|i| ((i.start + i.end) * 0.5 - at).length() < 5e-6));
        assert!(short, "no 0.0712 mm edge at x {}", x * 2.934);
    }

/// cf-snap-hook's Snap Hook 2 (recorded): a hook fused to a body whose rounds are arcs turned about an axis (tori
/// OCCT keeps as surfaces of revolution): the arcs as B-splines at their own parameters, as an extrusion's, so the
/// revolutions are noshape's free-form faces and the fuse ours, as OCCT's (it declined "a face of revolution"; OCCT
/// 0.96 s, ours ~11 ms).
#[test]
fn a_fuse_with_revolved_arcs_as_occt() {
    valid_as_occt("4a77bf0cde101133", 1e-9);
}
