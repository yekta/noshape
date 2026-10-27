//! Boolean corpus replay (docs/BOOLEAN-SPEEDUP.md phase 0): every recorded
//! case (`noshape_kernel::bool_corpus`) run again through the current code and
//! compared with what was recorded: validity, counts, volume (1e-9 relative)
//! and the history. Prints each case's time against the recorded one and the
//! totals by size (faces of the objects).
//!
//! Record: `NOSHAPE_RECORD_BOOLEANS=/root/core-scratch/bool-corpus <any regeneration>`.
//! Replay: `NOSHAPE_BOOL_CORPUS=<dir> cargo test --release -p noshape-kernel --test all bool_replay::bool_corpus_replay -- --ignored --nocapture`
//! (NOSHAPE_BOOL_<NAME>=0 switches a fast path off; NOSHAPE_BOOL_REBASE=1 takes
//! this run's results as the expected ones, before a change is measured).

use noshape_kernel::bool_corpus::{cases, load, Summary};
use noshape_kernel::*;
use std::path::Path;

/// (cases, mismatches, report).
fn replay(dir: &Path, only_faces_below: Option<usize>) -> (usize, Vec<String>, String) {
    let _ = set_working_scale(1000.0);
    let names = cases(dir);
    // Cases OCCT itself doesn't reproduce from run to run (`<dir>/unstable`,
    // one name a line): reported, not failed.
    let unstable: Vec<String> = std::fs::read_to_string(dir.join("unstable")).map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default();
    let mut bad = Vec::new();
    let mut report = String::new();
    // (bucket, recorded ms, now ms, cases)
    let mut buckets = [("<50 input faces", 0.0, 0.0, 0usize), ("50-1000", 0.0, 0.0, 0), (">1000", 0.0, 0.0, 0)];
    let mut n = 0;
    for name in &names {
        let Some(c) = load(dir, name) else {
            bad.push(format!("{name}: can't be loaded"));
            continue;
        };
        // Size: the faces of every input (a pattern's 1000 tools are a big case).
        let faces: usize = c.objects.iter().chain(&c.tools).map(|x| x.count(ShapeType::Face)).sum();
        if only_faces_below.is_some_and(|m| faces >= m) {
            continue;
        }
        n += 1;
        let t = std::time::Instant::now();
        let r = boolean(c.op, &c.objects, &c.tools, &c.opts);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let path = last_boolean_path();
        match r {
            Ok(r) if std::env::var_os("NOSHAPE_BOOL_REBASE").is_some() => {
                let _ = noshape_kernel::bool_corpus::rebase(dir, name, &Summary::of(&r));
            }
            Ok(r) => {
                let d = Summary::of(&r).differences(&c.summary);
                if !d.is_empty() && unstable.contains(name) {
                    report += &format!("{name}: differs (listed unstable): {}\n", d.join(", "));
                } else if !d.is_empty() {
                    bad.push(format!("{name} ({:?}, {} tools, path {path}, was {}): {}", c.op, c.tools.len(), c.path, d.join(", ")));
                }
            }
            Err(e) => bad.push(format!("{name}: failed: {e}")),
        }
        let b = if faces < 50 { 0 } else if faces <= 1000 { 1 } else { 2 };
        buckets[b].1 += c.ms;
        buckets[b].2 += ms;
        buckets[b].3 += 1;
        report += &format!("{name} {:?} faces {faces} tools {}: {:.1} ms (recorded {:.1}, {:.2}x) {path}\n", c.op, c.tools.len(), ms, c.ms, c.ms / ms.max(1e-6));
    }
    for (what, was, now, k) in buckets {
        report += &format!("{what}: {k} cases, {now:.0} ms (recorded {was:.0} ms, {:.2}x)\n", was / now.max(1e-6));
    }
    (n, bad, report)
}

/// The whole corpus (outside the repo).
#[test]
#[ignore = "T2: the Boolean corpus (/root/core-scratch/bool-corpus, NOSHAPE_BOOL_CORPUS)"]
fn bool_corpus_replay() {
    let dir = std::env::var("NOSHAPE_BOOL_CORPUS").unwrap_or_else(|_| "/root/core-scratch/bool-corpus".into());
    // NOSHAPE_BOOL_MAX_FACES=n: only the cases with fewer input faces.
    let max = std::env::var("NOSHAPE_BOOL_MAX_FACES").ok().and_then(|v| v.parse().ok());
    let (n, bad, report) = replay(Path::new(&dir), max);
    eprintln!("{report}");
    assert!(n > 0, "no cases in {dir}");
    assert!(bad.is_empty(), "{} of {n} cases differ:\n{}", bad.len(), bad.join("\n"));
}

/// The small cases kept in the repo (tests/data/bool): every path, ~30 cases.
#[test]
fn bool_corpus_small_cases() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/bool");
    let (n, bad, report) = replay(&dir, None);
    eprintln!("{report}");
    assert!(n >= 20, "{n} cases in {}", dir.display());
    assert!(bad.is_empty(), "{} of {n} cases differ:\n{}", bad.len(), bad.join("\n"));
}

/// The same result serial and in parallel (docs/BOOLEAN-SPEEDUP.md rule 2):
/// a fuse of 8 tools (OCCT intersects their pairs in parallel), its faces in
/// the same order with the same history.
#[test]
fn parallel_booleans_give_the_same_result() {
    let _ = set_working_scale(1000.0);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/bool-determinism");
    let c = load(&dir, "a7a3cae1df220044").expect("the case");
    let run = |parallel: bool| {
        set_parallel_booleans(Some(parallel));
        let r = boolean(c.op, &c.objects, &c.tools, &c.opts);
        set_parallel_booleans(None);
        let r = r.unwrap();
        let order: Vec<(i64, i64, i64, i64)> = r
            .shape
            .faces()
            .iter()
            .map(|f| {
                let p = f.centroid().unwrap() * 1e9;
                (p.x.round() as i64, p.y.round() as i64, p.z.round() as i64, (f.area().unwrap() * 1e15).round() as i64)
            })
            .collect();
        (order, Summary::of(&r))
    };
    let serial = run(false);
    for _ in 0..3 {
        assert_eq!(run(true), serial, "parallel differs from serial");
    }
}
