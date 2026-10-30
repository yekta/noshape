//! The blend corpus (noshape_kernel::blend_corpus): each recorded fillet / chamfer again, against what was
//! recorded (counts, validity, volume to 1e-9, the history by where its outputs lie).
//! Replay: `NOSHAPE_BLEND_CORPUS=<dir> cargo test --release -p noshape-kernel --test all blend_replay -- --ignored --nocapture`
//! (NOSHAPE_BLEND_RUST=0: OCCT's blends only).

use noshape_kernel::blend_corpus;
use std::path::Path;

/// Replays every case of `dir`: (cases, differences, a timing line).
fn replay(dir: &Path) -> (usize, Vec<String>, String) {
    let mut diffs = Vec::new();
    let mut paths: std::collections::BTreeMap<String, usize> = Default::default();
    let (mut n, mut was_ms, mut now_ms) = (0, 0.0, 0.0);
    for name in blend_corpus::cases(dir) {
        let Some(case) = blend_corpus::load(dir, &name) else { continue };
        n += 1;
        noshape_kernel::reset_blend_decline();
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
                let d = noshape_kernel::bool_corpus::Summary::of(&res).differences(&case.summary);
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
