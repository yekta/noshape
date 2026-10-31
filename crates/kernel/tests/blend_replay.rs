//! The blend corpus (noshape_kernel::blend_corpus): each recorded fillet / chamfer again, against what was
//! recorded (counts, validity, volume to 1e-9, the history by where its outputs lie).
//! Replay: `NOSHAPE_BLEND_CORPUS=<dir> cargo test --release -p noshape-kernel --test all blend_replay -- --ignored --nocapture`
//! (NOSHAPE_BLEND_RUST=0: OCCT's blends only).

use noshape_kernel::blend_corpus;
use std::path::Path;

/// Replays every case of `dir`: (cases, differences, a timing line).
fn replay(dir: &Path) -> (usize, Vec<String>, String) {
    // Cases where noshape's blend is live's and OCCT's isn't (`<dir>/live`, a name a line, why after a space: a
    // chamfer as wide as the corner fillets, live request 1646): not failed, counted.
    let live: Vec<String> = std::fs::read_to_string(dir.join("live")).unwrap_or_default().lines().filter_map(|l| l.split_whitespace().next()).map(str::to_string).collect();
    let mut diffs = Vec::new();
    let mut paths: std::collections::BTreeMap<String, usize> = Default::default();
    let (mut n, mut was_ms, mut now_ms) = (0, 0.0, 0.0);
    for name in blend_corpus::cases(dir) {
        let Some(case) = blend_corpus::load(dir, &name) else { continue };
        n += 1;
        noshape_kernel::reset_blend_decline();
        if std::env::var_os("NOSHAPE_BLEND_VERBOSE").is_some() {
            eprintln!("case {name}");
        }
        let t = std::time::Instant::now();
        let r = blend_corpus::run(&case);
        let why = noshape_kernel::last_blend_decline();
        let post = noshape_kernel::last_rust_decline();
        let path = if why.is_empty() && post.is_empty() { "rust".to_string() } else if why.is_empty() { format!("built, then: {post}") } else { why };
        *paths.entry(path.clone()).or_insert(0) += 1;
        if std::env::var("NOSHAPE_BLEND_LIST").is_ok_and(|l| path.contains(&l)) {
            println!("{name} [{path}] {:?} {} edges, {} faces", case.blend, case.edges.len(), case.shape.count(noshape_kernel::ShapeType::Face));
        }
        let ms = t.elapsed().as_secs_f64() * 1e3;
        was_ms += case.ms;
        now_ms += ms;
        match r {
            Ok(res) => {
                let mut d = noshape_kernel::bool_corpus::Summary::of(&res).differences(&case.summary);
                // A history that differs by its hash only: the records again against OCCT's own, their places to
                // 1e-7 (a fillet corner's exact ellipse against OCCT's approximation of it).
                // (And the volume to 1e-7: OCCT's corners are approximations, its volume off the exact one by up
                // to ~5e-9 where noshape's is exact to 1e-12, plate-with-hole-and-frame 21788e8f.)
                let near = |d: &Vec<String>| d.iter().all(|x| x == "history differs" || x.starts_with("volume "));
                let vol_close = (noshape_kernel::bool_corpus::Summary::of(&res).volume - case.summary.volume).abs() <= 1e-7 * case.summary.volume.abs();
                if !d.is_empty() && near(&d) && vol_close {
                    noshape_kernel::set_rust_blend(Some(false));
                    let occt = blend_corpus::run(&case);
                    noshape_kernel::set_rust_blend(None);
                    if let Ok(o) = occt {
                        if same_records(&o, &res, false) {
                            d.clear();
                            *paths.entry("rust (as OCCT to 1e-7: its corners approximated)".into()).or_insert(0) += 1;
                        } else if retried(&o, &case) && same_records(&o, &res, true) {
                            d.clear();
                            *paths.entry("rust (as OCCT to 1e-6, its history through its scaled retry)".into()).or_insert(0) += 1;
                        }
                    }
                }
                if !d.is_empty() && live.contains(&name) {
                    d.clear();
                    *paths.entry("rust (live's, OCCT's differs)".into()).or_insert(0) += 1;
                }
                if !d.is_empty() {
                    diffs.push(format!("{name} [{path}] ({:?}, {} edges): {}", case.blend, case.edges.len(), d.join(", ")));
                }
            }
            Err(e) => diffs.push(format!("{name} [{path}] ({:?}): failed: {e}", case.blend)),
        }
    }
    let paths: Vec<String> = paths.iter().map(|(p, k)| format!("  {k:4} {p}")).collect();
    (n, diffs, format!("{n} cases, recorded {was_ms:.0} ms, now {now_ms:.0} ms; paths:\n{}", paths.join("\n")))
}

/// OCCT's blend went through its retry at the working scale (its first try failed or was invalid): the
/// history through the scaled copy has every input face Modified, the untouched ones too, and no solid.
fn retried(o: &noshape_kernel::OpResult, case: &blend_corpus::Case) -> bool {
    use noshape_kernel::{Relation, ShapeType};
    let faces = case.shape.count(ShapeType::Face);
    let modified = o.history.records.iter().filter(|h| h.input_kind == ShapeType::Face && h.relation == Relation::Modified).map(|h| h.input_index).collect::<std::collections::BTreeSet<_>>();
    modified.len() == faces && !o.history.records.iter().any(|h| h.input_kind == ShapeType::Solid)
}

/// The two results' history records the same, each output's place (centroid / midpoint / point, area /
/// length) to 1e-7 of the part's size (`made`: only the faces generated and the edges' pieces, as a retried
/// OCCT blend's history keeps them, to 1e-6: its corners at the working scale are coarser).
fn same_records(a: &noshape_kernel::OpResult, b: &noshape_kernel::OpResult, made: bool) -> bool {
    use noshape_kernel::ShapeType;
    let size = a.shape.bounding_box(false).map(|x| x.diagonal()).unwrap_or(1.0).max(1e-9);
    let place = |r: &noshape_kernel::OpResult, kind: ShapeType, i: u32| -> [f64; 4] {
        let Ok(x) = r.shape.sub_shape(kind, i as usize) else { return [f64::NAN; 4] };
        let (p, m) = match kind {
            ShapeType::Face => (x.centroid().unwrap_or_default(), x.area().unwrap_or(0.0) / size),
            ShapeType::Edge => (x.edge_info().ok().and_then(|e| x.point_at(0.5 * (e.first_param + e.last_param)).ok()).unwrap_or_default(), x.length().unwrap_or(0.0)),
            ShapeType::Vertex => (x.vertex_point().unwrap_or_default(), 0.0),
            _ => (Default::default(), 0.0),
        };
        [p.x, p.y, p.z, m]
    };
    // (arg, input kind, index, relation, output kind) and the output's place.
    type Rec = ((u32, u8, u32, u8, u8), [f64; 4]);
    let recs = |r: &noshape_kernel::OpResult| -> Vec<Rec> {
        let keep = |h: &&noshape_kernel::HistoryRecord| !made || h.relation == noshape_kernel::Relation::Generated || (h.relation == noshape_kernel::Relation::Modified && h.input_kind == ShapeType::Edge);
        let mut v: Vec<_> = r.history.records.iter().filter(keep).map(|h| ((h.arg, h.input_kind as u8, h.input_index, h.relation as u8, h.output_kind as u8), place(r, h.output_kind, h.output_index))).collect();
        v.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal)));
        v
    };
    let (ra, rb) = (recs(a), recs(b));
    ra.len() == rb.len()
        && ra.iter().zip(&rb).all(|(x, y)| x.0 == y.0 && x.1.iter().zip(&y.1).all(|(p, q)| (p.is_nan() && q.is_nan()) || (p - q).abs() <= if made { 1e-6 } else { 1e-7 } * size))
}

#[test]
#[ignore = "T2: the blend corpus (/root/core-scratch/blend-corpus, NOSHAPE_BLEND_CORPUS)"]
fn blend_corpus_replay() {
    let dir = std::env::var("NOSHAPE_BLEND_CORPUS").unwrap_or_else(|_| "/root/core-scratch/blend-corpus".into());
    let (n, diffs, timing) = replay(Path::new(&dir));
    println!("{timing}");
    for d in &diffs {
        println!("{d}");
    }
    assert!(n == 0 || diffs.is_empty(), "{} of {n} cases differ", diffs.len());
}

/// One case both ways, its history records side by side (those only in one):
/// `NOSHAPE_BLEND_CASE=<name> cargo test -p noshape-kernel --test all blend_replay::blend_case_diff -- --ignored --nocapture`.
#[test]
#[ignore = "T2 tool: NOSHAPE_BLEND_CASE"]
fn blend_case_diff() {
    use noshape_kernel::{set_rust_blend, OpResult, ShapeType};
    let Ok(name) = std::env::var("NOSHAPE_BLEND_CASE") else { return };
    let dir = std::env::var("NOSHAPE_BLEND_CORPUS").unwrap_or_else(|_| "/root/core-scratch/blend-corpus".into());
    let case = blend_corpus::load(Path::new(&dir), &name).expect("case");
    let records = |r: &OpResult| -> Vec<String> {
        let q = |x: f64| (x * 1e6).round() as i64;
        let place = |kind: ShapeType, i: u32| -> String {
            let Ok(x) = r.shape.sub_shape(kind, i as usize) else { return "-".into() };
            match kind {
                ShapeType::Face => {
                    let c = x.centroid().unwrap_or_default();
                    format!("face ({} {} {}) area {}", q(c.x), q(c.y), q(c.z), q(x.area().unwrap_or(0.0) * 1e3))
                }
                ShapeType::Edge => {
                    let m = x.edge_info().ok().and_then(|i| x.point_at(0.5 * (i.first_param + i.last_param)).ok()).unwrap_or_default();
                    format!("edge mid ({} {} {}) len {}", q(m.x), q(m.y), q(m.z), q(x.length().unwrap_or(0.0)))
                }
                ShapeType::Vertex => {
                    let p = x.vertex_point().unwrap_or_default();
                    format!("vertex ({} {} {})", q(p.x), q(p.y), q(p.z))
                }
                _ => String::new(),
            }
        };
        let mut v: Vec<String> = r.history.records.iter().map(|h| format!("{:?} {} {:?} -> {:?} {}", h.input_kind, h.input_index, h.relation, h.output_kind, place(h.output_kind, h.output_index))).collect();
        v.sort();
        v
    };
    let time = |on: bool| {
        set_rust_blend(Some(on));
        let _ = blend_corpus::run(&case);
        let t = std::time::Instant::now();
        let n = 5;
        for _ in 0..n {
            let _ = blend_corpus::run(&case);
        }
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    };
    for (i, e) in case.edges.iter().enumerate() {
        let x = e.edge_info().unwrap();
        println!("blended edge {i}: {:?} {:.6?} -> {:.6?} ({:?})", x.curve, x.start, x.end, case.blend);
    }
    println!("time: OCCT {:.2} ms, ours {:.2} ms", time(false), time(true));
    set_rust_blend(Some(false));
    let occt = blend_corpus::run(&case).expect("OCCT's");
    set_rust_blend(Some(true));
    noshape_kernel::reset_blend_decline();
    let ours = blend_corpus::run(&case);
    set_rust_blend(None);
    println!("ours: {:?} ({})", ours.as_ref().map(|r| r.shape.count(ShapeType::Face)), noshape_kernel::last_blend_decline());
    let Ok(ours) = ours else { return };
    if std::env::var_os("NOSHAPE_BLEND_FACES").is_some() {
        for (what, r) in [("OCCT", &occt), ("ours", &ours)] {
            println!("{what}: volume {:.12e}", r.shape.volume().unwrap_or(0.0));
            for (i, f) in r.shape.faces().iter().enumerate() {
                let c = f.centroid().unwrap_or_default();
                println!("  {what} F{i} {:?} area {:.12e} centroid ({:.9} {:.9} {:.9})", f.face_surface().map(|s| format!("{s:?}").chars().take(60).collect::<String>()), f.area().unwrap_or(0.0), c.x, c.y, c.z);
                if std::env::var_os("NOSHAPE_BLEND_FACES").is_some_and(|v| v == "2") {
                    println!("     uv {:?}", f.uv_bounds());
                    for e in f.edges() {
                        let i = e.edge_info().unwrap();
                        println!("     edge {:?} {:.6?} -> {:.6?} t {:.6}..{:.6}", i.curve, i.start, i.end, i.first_param, i.last_param);
                    }
                }
            }
        }
    }
    let (a, b) = (records(&occt), records(&ours));
    if std::env::var_os("NOSHAPE_BLEND_HISTORY").is_some() {
        for x in &a {
            println!("OCCT: {x}");
        }
        for (i, e) in case.shape.edges().iter().enumerate() {
            let x = e.edge_info().unwrap();
            println!("input edge {i}: {:.6?} -> {:.6?}", x.start, x.end);
        }
    }
    for x in a.iter().filter(|x| !b.contains(x)) {
        println!("OCCT only: {x}");
    }
    for x in b.iter().filter(|x| !a.contains(x)) {
        println!("ours only: {x}");
    }
}

/// The corpus's cases by shape (for choosing what noshape's blend takes next): per decline reason, the edges'
/// curves and their two faces' surfaces. `cargo test -p noshape-kernel --test all blend_replay::blend_corpus_shapes -- --ignored --nocapture`
#[test]
#[ignore = "T2 tool: the blend corpus by shape"]
fn blend_corpus_shapes() {
    use noshape_brep::{Curve, Surface};
    let dir = std::env::var("NOSHAPE_BLEND_CORPUS").unwrap_or_else(|_| "/root/core-scratch/blend-corpus".into());
    let mut tally: std::collections::BTreeMap<String, usize> = Default::default();
    noshape_kernel::set_rust_blend(Some(true));
    for name in blend_corpus::cases(Path::new(&dir)) {
        let Some(case) = blend_corpus::load(Path::new(&dir), &name) else { continue };
        noshape_kernel::reset_blend_decline();
        let _ = blend_corpus::run(&case);
        let why = noshape_kernel::last_blend_decline();
        let Ok(body) = noshape_kernel::brep_body(&case.shape) else {
            *tally.entry(format!("{why} | no body")).or_insert(0) += 1;
            continue;
        };
        let mut faces_of = vec![Vec::new(); body.edges.len()];
        for (f, face) in body.faces.iter().enumerate() {
            for lp in &face.loops {
                for u in lp {
                    faces_of[u.edge as usize].push(f);
                }
            }
        }
        let sk = |s: &Surface| match s {
            Surface::Plane { .. } => "plane",
            Surface::Cylinder { .. } => "cyl",
            Surface::Cone { .. } => "cone",
            Surface::Torus { .. } => "torus",
            Surface::Sphere { .. } => "sphere",
            _ => "free",
        };
        let mut kinds: Vec<String> = case
            .edges
            .iter()
            .filter_map(|e| case.shape.index_of(e, noshape_kernel::ShapeType::Edge))
            .map(|i| {
                let c = match body.edges[i].curve {
                    Curve::Line => "line",
                    Curve::Circle { .. } => "arc",
                    _ => "other",
                };
                let mut fs: Vec<&str> = faces_of[i].iter().map(|&f| sk(&body.faces[f].surface)).collect();
                fs.sort();
                format!("{c}({})", fs.join("/"))
            })
            .collect();
        kinds.sort();
        kinds.dedup();
        let n = case.edges.len();
        let bucket = if n == 1 { "1".to_string() } else if n <= 4 { "2-4".into() } else { "5+".into() };
        *tally.entry(format!("{why} | {bucket} edges | {}", kinds.join(" "))).or_insert(0) += 1;
    }
    noshape_kernel::set_rust_blend(None);
    let mut v: Vec<_> = tally.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (k, n) in v {
        println!("{n:4} {k}");
    }
}
