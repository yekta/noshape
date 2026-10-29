//! noshape's own Boolean (crates/brep) against OCCT over the Boolean corpus: each case it accepts must give
//! OCCT's recorded result (validity, counts, volume to 1e-9, the history), and it is timed against OCCT run
//! here. Prints the cases it takes, why it declines the others, and the times by size.
//!
//! `NOSHAPE_BOOL_CORPUS=/root/core-scratch/bool-corpus cargo test -p noshape-kernel --test all
//! brep_replay::brep_corpus_replay -- --ignored --nocapture` (NOSHAPE_BREP_DIFF=1 prints each mismatch's
//! history records that differ; NOSHAPE_BREP_ONLY=<name> runs one case; NOSHAPE_BREP_SHOW_OCCT=<name>,... prints
//! OCCT's result and history records for those cases, taken or not; NOSHAPE_BREP_VS_OCCT=1 compares with OCCT run
//! here instead of the recorded result: for a corpus recorded with this Boolean on; NOSHAPE_BREP_ALWAYS=<dir> writes
//! every taken case's two results there).

use noshape_kernel::bool_corpus::{cases, load, Summary};
use noshape_kernel::*;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

/// Each history record as text, with its output's place (micrometres; sorted).
fn records(r: &OpResult) -> Vec<(String, [i64; 3])> {
    let q = |x: f64| (x * 1e6).round() as i64;
    let mut v: Vec<(String, [i64; 3])> = r
        .history
        .records
        .iter()
        .map(|h| {
            let p = r
                .shape
                .sub_shape(h.output_kind, h.output_index as usize)
                .ok()
                .map(|x| match h.output_kind {
                    ShapeType::Face => x.centroid().unwrap_or_default(),
                    ShapeType::Edge => x.edge_info().ok().and_then(|i| x.point_at(0.5 * (i.first_param + i.last_param)).ok()).unwrap_or_default(),
                    ShapeType::Vertex => x.vertex_point().unwrap_or_default(),
                    _ => DVec3::ZERO,
                })
                .unwrap_or_default();
            (format!("arg{} {:?}{} {:?} {:?}", h.arg, h.input_kind, h.input_index, h.relation, h.output_kind), [q(p.x), q(p.y), q(p.z)])
        })
        .collect();
    v.sort();
    v
}

/// The same records, places within 2 µm (a centroid rounded the other way).
fn same_records(a: &[(String, [i64; 3])], b: &[(String, [i64; 3])]) -> bool {
    let mut used = vec![false; b.len()];
    a.len() == b.len()
        && a.iter().all(|(k, p)| {
            let hit = b.iter().enumerate().position(|(j, (l, o))| !used[j] && k == l && (0..3).all(|i| (p[i] - o[i]).abs() <= 2));
            hit.map(|j| used[j] = true).is_some()
        })
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// (cases, accepted, mismatches, report).
fn replay(dir: &Path) -> (usize, usize, Vec<String>, String) {
    let _ = set_working_scale(1000.0);
    let only = std::env::var("NOSHAPE_BREP_ONLY").ok();
    let diff = std::env::var_os("NOSHAPE_BREP_DIFF").is_some();
    let unstable: Vec<String> = std::fs::read_to_string(dir.join("unstable")).map(|t| t.lines().map(|l| l.trim().to_string()).collect()).unwrap_or_default();
    let mut bad = Vec::new();
    let mut report = String::new();
    let mut declines: BTreeMap<String, usize> = BTreeMap::new();
    // By size bucket: (rust ms, occt ms) of each accepted case.
    let mut times: [Vec<(f64, f64)>; 3] = Default::default();
    let (mut n, mut accepted) = (0, 0);
    for name in cases(dir) {
        if only.as_ref().is_some_and(|o| *o != name) || unstable.contains(&name) {
            continue;
        }
        let Some(c) = load(dir, &name) else { continue };
        if std::env::var("NOSHAPE_BREP_MIN_FACES").ok().and_then(|v| v.parse::<usize>().ok()).is_some_and(|m| c.target_faces < m) {
            continue;
        }
        n += 1;
        // Declines fail at once (no OCCT run).
        set_rust_boolean_only(true);
        let t = Instant::now();
        let r = boolean(c.op, &c.objects, &c.tools, &c.opts);
        let mut ms = t.elapsed().as_secs_f64() * 1e3;
        let path = last_boolean_path();
        let why = last_rust_decline();
        set_rust_boolean_only(false);
        if let Ok(names) = std::env::var("NOSHAPE_BREP_SHOW_OCCT")
            && names.split(',').any(|x| x == name)
        {
            set_rust_boolean(Some(false));
            if let Ok(o) = boolean(c.op, &c.objects, &c.tools, &c.opts) {
                eprintln!("OCCT {name}: {} solids {} faces {} edges {} vertices", o.shape.count(ShapeType::Solid), o.shape.count(ShapeType::Face), o.shape.count(ShapeType::Edge), o.shape.count(ShapeType::Vertex));
                for x in records(&o) {
                    eprintln!("  {} {:?}", x.0, x.1);
                }
            }
            set_rust_boolean(None);
        }
        // (Failed without a reason: an error inside it, which the app would show, not a decline.)
        if let (true, Err(e)) = (path != "rust" && why.is_empty(), &r) {
            bad.push(format!("{name}: failed: {e}"));
            continue;
        }
        if path != "rust" {
            if std::env::var("NOSHAPE_BREP_WHY").is_ok_and(|w| why.contains(&w)) {
                report += &format!("{name} declined: {why}\n");
            }
            *declines.entry(why).or_default() += 1;
            continue;
        }
        accepted += 1;
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                bad.push(format!("{name}: failed: {e}"));
                continue;
            }
        };
        // Again, warm (the first call loads the inputs' maps).
        set_rust_boolean(Some(true));
        kernel_profile_mark("before");
        let t = Instant::now();
        let _ = boolean(c.op, &c.objects, &c.tools, &c.opts);
        kernel_profile_mark(&name);
        ms = ms.min(t.elapsed().as_secs_f64() * 1e3);
        set_rust_boolean(Some(false));
        let t = Instant::now();
        let o = boolean(c.op, &c.objects, &c.tools, &c.opts);
        let occt_ms = t.elapsed().as_secs_f64() * 1e3;
        set_rust_boolean(None);
        // (A corpus recorded with this Boolean on: against OCCT run here.)
        let mut d = match (&o, std::env::var_os("NOSHAPE_BREP_VS_OCCT")) {
            (Ok(o), Some(_)) => Summary::of(&r).differences(&Summary::of(o)),
            _ => Summary::of(&r).differences(&c.summary),
        };
        // The history's hash rounds centroids: compare with OCCT's records within 2 µm.
        if d == ["history differs"] && std::env::var_os("NOSHAPE_BREP_EXACT").is_none() && o.as_ref().is_ok_and(|o| same_records(&records(&r), &records(o))) {
            d.clear();
        }
        // What the model does next with a result (merging faces left on one surface) must come out the same.
        if d.is_empty()
            && let Ok(o) = &o
        {
            // (Faces, edges, validity; its volume to 1e-9.)
            let unified = |x: &Shape| unify_same_domain(x).map(|u| (u.shape.count(ShapeType::Face), u.shape.count(ShapeType::Edge), u.shape.is_valid(), u.shape.volume().unwrap_or(0.0))).ok();
            let alike = |a: &Option<(usize, usize, bool, f64)>, b: &Option<(usize, usize, bool, f64)>| match (a, b) {
                (Some(a), Some(b)) => (a.0, a.1, a.2) == (b.0, b.1, b.2) && (a.3 - b.3).abs() <= 1e-9 * a.3.abs().max(b.3.abs()),
                _ => a.is_none() && b.is_none(),
            };
            if let Some(dump) = std::env::var_os("NOSHAPE_BREP_ALWAYS") {
                let _ = std::fs::write(Path::new(&dump).join(format!("{name}-rust.brep")), r.shape.to_brep_bytes().unwrap_or_default());
                let _ = std::fs::write(Path::new(&dump).join(format!("{name}-occt.brep")), o.shape.to_brep_bytes().unwrap_or_default());
            }
            let (ur, uo) = (unified(&r.shape), unified(&o.shape));
            if !alike(&ur, &uo) {
                d.push(format!("unified differently: {ur:?} vs OCCT {uo:?}"));
            }
        }
        if !d.is_empty() {
            let mut line = format!("{name} ({:?}, {} tools, recorded path {}): {}", c.op, c.tools.len(), c.path, d.join(", "));
            if let Some(dump) = std::env::var_os("NOSHAPE_BREP_DUMP") {
                let _ = std::fs::write(Path::new(&dump).join(format!("{name}-rust.brep")), r.shape.to_brep_bytes().unwrap_or_default());
                if let Ok(o) = &o {
                    let _ = std::fs::write(Path::new(&dump).join(format!("{name}-occt.brep")), o.shape.to_brep_bytes().unwrap_or_default());
                }
            }
            if std::env::var_os("NOSHAPE_BREP_FACES").is_some()
                && let Ok(o) = &o
            {
                for (who, x) in [("rust", &r), ("occt", o)] {
                    for f in x.shape.sub_shapes(ShapeType::Face) {
                        let c = f.centroid().unwrap_or_default();
                        line += &format!("\n    {who} face area {:.6e} at ({:.5} {:.5} {:.5}) valid {} {:?}", f.area().unwrap_or(0.0), c.x, c.y, c.z, f.is_valid(), f.orientation());
                        if !f.is_valid() || who == "occt" {
                            for w in f.children() {
                                for e in w.children() {
                                    let i = e.edge_info();
                                    line += &format!("\n        {:?} {:?}", e.orientation(), i.map(|i| (i.start, i.end, i.first_param, i.last_param)));
                                }
                            }
                        }
                    }
                }
            }
            if let (Ok(at), Ok(o)) = (std::env::var("NOSHAPE_BREP_AT"), &o) {
                for (who, x) in [("rust", records(&r)), ("occt", records(o))] {
                    for rec in x.iter().filter(|x| format!("{:?}", x.1) == at) {
                        line += &format!("\n    {who} at: {} {:?}", rec.0, rec.1);
                    }
                }
            }
            if diff && let Ok(o) = &o {
                let (mine, theirs) = (records(&r), records(o));
                for x in mine.iter().filter(|x| !theirs.contains(x)) {
                    line += &format!("\n    rust only: {} {:?}", x.0, x.1);
                }
                for x in theirs.iter().filter(|x| !mine.contains(x)) {
                    line += &format!("\n    occt only: {} {:?}", x.0, x.1);
                }
            }
            bad.push(line);
            continue;
        }
        let faces: usize = c.objects.iter().chain(&c.tools).map(|x| x.count(ShapeType::Face)).sum();
        let b = if faces < 50 { 0 } else if faces <= 1000 { 1 } else { 2 };
        times[b].push((ms, occt_ms));
        report += &format!("{name} {:?} faces {faces} tools {}: {ms:.2} ms, OCCT {occt_ms:.2} ms ({:.1}x)\n", c.op, c.tools.len(), occt_ms / ms.max(1e-6));
    }
    report += &format!("{accepted} of {n} cases taken, {} differ from OCCT\n", bad.len());
    for (why, k) in &declines {
        report += &format!("  declined {k}: {why}\n");
    }
    for (what, t) in ["<50 input faces", "50-1000", ">1000"].iter().zip(&times) {
        let mut r: Vec<f64> = t.iter().map(|x| x.0).collect();
        let mut o: Vec<f64> = t.iter().map(|x| x.1).collect();
        let (sr, so): (f64, f64) = (r.iter().sum(), o.iter().sum());
        report += &format!(
            "{what}: {} cases, median {:.2} ms (OCCT {:.2} ms), total {sr:.0} ms (OCCT {so:.0} ms, {:.1}x)\n",
            t.len(),
            median(&mut r),
            median(&mut o),
            so / sr.max(1e-6)
        );
    }
    (n, accepted, bad, report)
}

/// The whole corpus (outside the repo).
#[test]
#[ignore = "T2: the Boolean corpus (/root/core-scratch/bool-corpus, NOSHAPE_BOOL_CORPUS)"]
fn brep_corpus_replay() {
    let dir = std::env::var("NOSHAPE_BOOL_CORPUS").unwrap_or_else(|_| "/root/core-scratch/bool-corpus".into());
    let (n, _, bad, report) = replay(Path::new(&dir));
    eprintln!("{report}");
    assert!(bad.is_empty(), "{} of {n} cases differ:\n{}", bad.len(), bad.join("\n"));
}

/// Small planar cases from the corpus (tests/data/brep): noshape's own Boolean takes every one, as OCCT.
#[test]
fn brep_small_cases_match_occt() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/brep");
    let (n, accepted, bad, report) = replay(&dir);
    eprintln!("{report}");
    assert!(n >= 20, "{n} cases in {}", dir.display());
    assert!(bad.is_empty(), "{} of {n} cases differ:\n{}", bad.len(), bad.join("\n"));
    assert_eq!(accepted, n, "cases declined:\n{report}");
}

/// The same result, faces in the same order with the same history, run alone and on 8 and 16 threads at once
/// on the very same input shapes (their OcShapes and kept bodies shared): every small case. The switches it
/// sets (`set_rust_boolean_only`) are each thread's own. (10-28: input edges the Boolean gave curves on new
/// faces' surfaces were changed under other threads' reads; it no longer changes any input.)
#[test]
fn brep_results_are_the_same_on_any_thread() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/brep");
    for name in cases(&dir) {
        let c = load(&dir, &name).expect("a case");
        let run = || {
            set_rust_boolean_only(true);
            let r = boolean(c.op, &c.objects, &c.tools, &c.opts).unwrap_or_else(|e| panic!("{name}: noshape's Boolean declined: {} ({e})", last_rust_decline()));
            set_rust_boolean_only(false);
            let faces: Vec<DVec3> = r.shape.sub_shapes(ShapeType::Face).iter().map(|f| f.centroid().unwrap_or_default()).collect();
            (faces, r.history.records)
        };
        let alone = run();
        for n in [8, 16] {
            let all: Vec<_> = std::thread::scope(|s| {
                let hs: Vec<_> = (0..n).map(|_| s.spawn(run)).collect();
                hs.into_iter().map(|h| h.join().expect("a thread")).collect()
            });
            for r in all {
                assert_eq!(r, alone, "{name} on {n} threads");
            }
        }
    }
}

/// A result's body kept for the next Boolean (no conversion) gives what converting it again gives: each
/// small case's result cut and fused with its tools moved aside, from the kept body and from the same shape
/// wrapped anew (nothing kept).
#[test]
fn brep_kept_bodies_give_the_same_result() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/brep");
    let mut n = 0;
    for name in cases(&dir) {
        let Some(c) = load(&dir, &name) else { continue };
        set_rust_boolean_only(true);
        let Ok(r) = boolean(c.op, &c.objects, &c.tools, &c.opts) else {
            set_rust_boolean_only(false);
            continue;
        };
        let fresh_shape = r.shape.oriented(r.shape.orientation()).expect("a new wrapper");
        // Tools apart from the ones the result shares vertices with.
        let tools: Vec<Shape> = c.tools.iter().map(|t| t.transformed(&Transform::Translate(DVec3::new(3e-4, 2e-4, 0.0))).expect("moved")).collect();
        for op in [BooleanOp::Cut, BooleanOp::Fuse] {
            let kept = boolean(op, std::slice::from_ref(&r.shape), &tools, &c.opts).map(|x| Summary::of(&x));
            let fresh = boolean(op, std::slice::from_ref(&fresh_shape), &tools, &c.opts).map(|x| Summary::of(&x));
            assert_eq!(kept.is_ok(), fresh.is_ok(), "{name} {op:?}");
            if let (Ok(a), Ok(b)) = (kept, fresh) {
                assert!(a.differences(&b).is_empty(), "{name} {op:?}: {:?}", a.differences(&b));
                n += 1;
            }
        }
        set_rust_boolean_only(false);
    }
    assert!(n >= 20, "{n} second Booleans");
}

/// A cylinder through a box at an angle to its faces (sections on planes at an angle to its axis: ellipses, their
/// curves on the cylinder OCCT's own), through the middle, near a side and short of the faces: noshape's own
/// Boolean takes each, as OCCT (a plane nearly along the axis declines).
#[test]
fn brep_tilted_holes_match_occt() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let mut taken = 0;
    for deg in [10.0f64, 30.0, 60.0] {
        for (dx, dy, len) in [(0.0, 0.0, 40.0), (3.0, 2.0, 40.0), (0.0, 7.0, 40.0), (0.0, 0.0, 12.0)] {
            for op in [BooleanOp::Fuse, BooleanOp::Cut, BooleanOp::Common] {
                let b = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0) * mm).unwrap();
                let a = deg.to_radians();
                let d = DVec3::new(0.0, -a.sin(), a.cos());
                let base = DVec3::new(20.0 + dx, 10.0 + dy, 5.0) * mm - d * (len / 2.0) * mm;
                let c = make_cylinder(&Plane::from_normal(base, d), 5.0 * mm, len * mm, std::f64::consts::TAU).unwrap();
                let name = format!("{deg}° ({dx}, {dy}) length {len} {op:?}");
                set_rust_boolean_only(true);
                let r = boolean(op, std::slice::from_ref(&b), std::slice::from_ref(&c), &BooleanOptions::default());
                let (path, why) = (last_boolean_path(), last_rust_decline());
                set_rust_boolean_only(false);
                set_rust_boolean(Some(false));
                let o = boolean(op, &[b], &[c], &BooleanOptions::default()).unwrap();
                set_rust_boolean(None);
                if path != "rust" {
                    // (Only the plane nearly along the axis: 10° with the cylinder near a side.)
                    assert_eq!(why, "a plane nearly along a cylinder", "{name}");
                    continue;
                }
                taken += 1;
                let d = Summary::of(&r.unwrap()).differences(&Summary::of(&o));
                assert!(d.is_empty(), "{name}: {d:?}");
            }
        }
    }
    assert!(taken >= 30, "{taken} taken");
}

/// Cylinders of one radius whose axes meet (cross-drilled holes, a boss across a bar): OCCT's two ellipses, its
/// curves on both cylinders; noshape's own Boolean takes them as OCCT, at right angles and slanted.
#[test]
fn brep_cross_drilled_holes_match_occt() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let mut taken = 0;
    let mut why_not = Vec::new();
    for deg in [90.0f64, 60.0] {
        for (len, r2) in [(40.0, 4.0), (14.0, 4.0), (40.0, 3.0)] {
            for op in [BooleanOp::Fuse, BooleanOp::Cut, BooleanOp::Common] {
                let b = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0) * mm).unwrap();
                let down = make_cylinder(&Plane::from_normal(DVec3::new(20.0, 10.0, -5.0) * mm, DVec3::Z), 4.0 * mm, 20.0 * mm, std::f64::consts::TAU).unwrap();
                let a = deg.to_radians();
                let d = DVec3::new(a.sin(), 0.0, a.cos());
                let base = DVec3::new(20.0, 10.0, 5.0) * mm - d * (len / 2.0) * mm;
                let across = make_cylinder(&Plane::from_normal(base, d), r2 * mm, len * mm, std::f64::consts::TAU).unwrap();
                let name = format!("{deg}° length {len} r {r2} {op:?}");
                let tools = [down, across];
                set_rust_boolean_only(true);
                let r = boolean(op, std::slice::from_ref(&b), &tools, &BooleanOptions::default());
                let (path, why) = (last_boolean_path(), last_rust_decline());
                set_rust_boolean_only(false);
                set_rust_boolean(Some(false));
                let o = boolean(op, &[b], &tools, &BooleanOptions::default());
                set_rust_boolean(None);
                if path != "rust" {
                    why_not.push(format!("{name}: {why}"));
                    continue;
                }
                taken += 1;
                let d = Summary::of(&r.unwrap()).differences(&Summary::of(&o.unwrap()));
                assert!(d.is_empty(), "{name}: {d:?}");
            }
        }
    }
    // (Of other radii a curve of the fourth order, a common with several tools, the slanted long one's ends
    // inside a face: declined.)
    assert!(taken >= 6, "{taken} taken; declined: {why_not:#?}");
}

/// A plane along a cone's axis, off it (a slab's wall by a chamfer's cone, a pocket's side by a countersink):
/// OCCT's hyperbola, its curves on the plane and the cone; noshape's own Boolean takes them as OCCT.
#[test]
fn brep_hyperbolas_match_occt() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let mut taken = 0;
    let mut why_not = Vec::new();
    for (y1, r1, r2) in [(9.0, 6.0, 2.0), (11.5, 6.0, 2.0), (13.0, 6.0, 2.0), (6.0, 2.0, 6.0), (14.0, 2.0, 6.0)] {
        for op in [BooleanOp::Fuse, BooleanOp::Cut, BooleanOp::Common] {
            let b = make_box_corners(DVec3::new(5.0, 0.0, 0.0) * mm, DVec3::new(35.0, y1, 10.0) * mm).unwrap();
            let cone = make_cone(&Plane::from_normal(DVec3::new(20.0, 10.0, 2.0) * mm, DVec3::Z), r1 * mm, r2 * mm, 6.0 * mm, std::f64::consts::TAU).unwrap();
            let name = format!("y {y1} r {r1}..{r2} {op:?}");
            set_rust_boolean_only(true);
            let r = boolean(op, std::slice::from_ref(&b), std::slice::from_ref(&cone), &BooleanOptions::default());
            let (path, why) = (last_boolean_path(), last_rust_decline());
            set_rust_boolean_only(false);
            set_rust_boolean(Some(false));
            let o = boolean(op, &[b], &[cone], &BooleanOptions::default());
            set_rust_boolean(None);
            if path != "rust" {
                why_not.push(format!("{name}: {why}"));
                continue;
            }
            taken += 1;
            let d = Summary::of(&r.unwrap()).differences(&Summary::of(&o.unwrap()));
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }
    // (The common of the narrow box and the widening cone: two solids with a void, declined.)
    assert!(taken >= 14, "{taken} taken; declined: {why_not:#?}");
}

/// qa's RUST-MIRROR-NOTCH (P0): a notched cylinder, its notch (r2, 5 deep up from its bottom) cut by noshape's own
/// Boolean (the app's, recorded), fused with its mirror across x = 0, which fills the notch, then unified: the full
/// cylinder in 3 faces. (ShapeUpgrade_UnifySameDomain's result depended on where the cut's seamed wire starts, OCCT's
/// own order there only by luck, and it changed its input in place; the unify now checks the volume it keeps.)
#[test]
fn a_notched_cylinder_fused_with_its_mirror_unifies_whole() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/brep");
    let c = load(&dir, "ad14557ce79005e1").unwrap();
    set_rust_boolean_only(true);
    let r = boolean(c.op, &c.objects, &c.tools, &c.opts).unwrap();
    let path = last_boolean_path();
    set_rust_boolean_only(false);
    assert_eq!(path, "rust");
    let m = r.shape.transformed(&Transform::Mirror(Plane::from_normal(DVec3::ZERO, DVec3::X))).unwrap();
    let f = boolean(BooleanOp::Fuse, std::slice::from_ref(&r.shape), std::slice::from_ref(&m), &BooleanOptions::default()).unwrap();
    let u = unify_same_domain(&f.shape).unwrap();
    set_rust_boolean(None);
    let full = std::f64::consts::PI * 25.0 * 10.0 * 1e-9;
    for (what, s) in [("unified", &u.shape), ("fused, after the unify", &f.shape)] {
        let v = volume_properties(s).unwrap().mass;
        assert!((v - full).abs() < 1e-9 * full, "{what}: {v}");
    }
    assert_eq!(u.shape.count(ShapeType::Face), 3);
}

/// Free-form faces reach noshape's own Boolean as NURBS: the thread case's B-spline faces (degree 1 × 10, 290
/// poles), their edges' B-spline curves and curves on the faces, decoded as OCCT has them (points on the
/// surfaces at their (u, v), edge curves through their vertices, curves on faces onto the edges).
#[test]
fn brep_free_form_faces_arrive_as_occt_has_them() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/brep-freeform");
    let c = load(&dir, "8dc3e8d4da1ab5af").unwrap();
    let tool = &c.tools[0];
    let body = brep_body(tool).unwrap();
    assert!(!body.nurbs_surfaces.is_empty() && !body.pcurves.is_empty());
    let faces = tool.sub_shapes(ShapeType::Face);
    for (f, face) in body.faces.iter().enumerate() {
        let noshape_brep::Surface::Nurbs { index } = face.surface else { continue };
        let s = &body.nurbs_surfaces[index as usize];
        let [u0, u1, v0, v1] = faces[f].uv_bounds().unwrap();
        for k in 0..=8 {
            let (u, v) = (u0 + (u1 - u0) * k as f64 / 8.0, v0 + (v1 - v0) * (8 - k) as f64 / 8.0);
            let want = faces[f].point_at_uv(u, v).unwrap();
            assert!((s.at(u, v) - want).length() < 1e-12, "face {f} at ({u}, {v}): {} vs {want}", s.at(u, v));
        }
    }
    for e in &body.edges {
        if let noshape_brep::Curve::Nurbs { index, t0, t1 } = e.curve {
            let c = &body.nurbs_curves[index as usize];
            assert!((c.at(t0) - body.vertices[e.v[0] as usize]).length() < 1e-7 && (c.at(t1) - body.vertices[e.v[1] as usize]).length() < 1e-7);
        }
    }
    for p in &body.pcurves {
        let noshape_brep::Surface::Nurbs { index } = body.faces[p.face as usize].surface else { panic!("a curve on a face not NURBS") };
        let (s, c) = (&body.nurbs_surfaces[index as usize], &body.nurbs_curves[p.curve as usize]);
        let e = body.edges[p.edge as usize];
        for (t, v) in [(p.t0, e.v[0]), (p.t1, e.v[1])] {
            let uv = c.at(t);
            assert!((s.at(uv.x, uv.y) - body.vertices[v as usize]).length() < 1e-6, "edge {} on face {}", p.edge, p.face);
        }
    }
}

/// Free-form cases (tests/data/brep-freeform: a helical thread's B-spline faces against a cylinder): noshape's own
/// Boolean takes them with OCCT's own sections of the free-form faces and everything else its own, as OCCT. (Not
/// yet: the thread fused with a rod its helical edges lie on, a common it finds non-manifold.)
#[test]
fn brep_free_form_cases_match_occt() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/brep-freeform");
    let (n, accepted, bad, report) = replay(&dir);
    eprintln!("{report}");
    assert!(bad.is_empty(), "{} of {n} cases differ:\n{}", bad.len(), bad.join("\n"));
    assert!(accepted >= 1, "{accepted} of {n} taken:\n{report}");
}

/// A cone (a countersink's frustum) cut by a plane at an angle to its axis, through it off the axis: an ellipse
/// whose centre is off the axis; noshape's own Boolean takes them as OCCT.
#[test]
fn brep_cones_cut_at_an_angle_match_occt() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let mut taken = 0;
    let mut why_not = Vec::new();
    for (deg, off) in [(20.0f64, 0.0), (20.0, 1.0), (40.0, 0.5)] {
        let cone = make_cone(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 5.0 * mm, 2.0 * mm, 10.0 * mm, std::f64::consts::TAU).unwrap();
        let a = deg.to_radians();
        let d = DVec3::new(a.sin(), 0.0, a.cos());
        // A big box whose top, tilted, crosses the cone at 5 mm.
        let top = DVec3::new(off, 0.0, 5.0) * mm;
        let block = make_box_corners(DVec3::new(-20.0, -20.0, -40.0) * mm, DVec3::ZERO).unwrap();
        let tool = block.transformed(&Transform::frame_to_frame(&Plane::from_normal(DVec3::ZERO, DVec3::Z), &Plane::from_normal(top, d))).unwrap();
        for op in [BooleanOp::Fuse, BooleanOp::Cut, BooleanOp::Common] {
            let name = format!("{deg}° {off} mm {op:?}");
            set_rust_boolean_only(true);
            let r = boolean(op, std::slice::from_ref(&cone), std::slice::from_ref(&tool), &BooleanOptions::default());
            let (path, why) = (last_boolean_path(), last_rust_decline());
            set_rust_boolean_only(false);
            set_rust_boolean(Some(false));
            let o = boolean(op, std::slice::from_ref(&cone), std::slice::from_ref(&tool), &BooleanOptions::default());
            set_rust_boolean(None);
            if path != "rust" {
                why_not.push(format!("{name}: {why}"));
                continue;
            }
            taken += 1;
            let d = Summary::of(&r.unwrap()).differences(&Summary::of(&o.unwrap()));
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }
    eprintln!("{taken} taken; declined: {why_not:#?}");
    assert!(taken >= 9, "{taken} taken; declined: {why_not:#?}");
}

/// Booleans on a part that already has a tilted hole (its rims ellipses, the argument's own): a pocket next to
/// it, a slot across it, a second tilted hole; noshape's own Boolean takes them as OCCT.
#[test]
fn brep_parts_with_tilted_holes_match_occt() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let mut taken = 0;
    let mut why_not = Vec::new();
    for deg in [20.0f64, 45.0] {
        let plate = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0) * mm).unwrap();
        let a = deg.to_radians();
        let d = DVec3::new(0.0, -a.sin(), a.cos());
        let hole = make_cylinder(&Plane::from_normal(DVec3::new(15.0, 10.0, 5.0) * mm - d * 20.0 * mm, d), 4.0 * mm, 40.0 * mm, std::f64::consts::TAU).unwrap();
        set_rust_boolean(Some(false));
        let part = boolean(BooleanOp::Cut, &[plate], &[hole], &BooleanOptions::default()).unwrap().shape;
        set_rust_boolean(None);
        let tools = [
            ("pocket", make_box_corners(DVec3::new(25.0, 5.0, 6.0) * mm, DVec3::new(35.0, 15.0, 11.0) * mm).unwrap()),
            ("slot", make_box_corners(DVec3::new(5.0, 8.0, 7.0) * mm, DVec3::new(30.0, 12.0, 11.0) * mm).unwrap()),
            ("boss", make_box_corners(DVec3::new(5.0, 2.0, 10.0) * mm, DVec3::new(12.0, 18.0, 14.0) * mm).unwrap()),
            ("hole", make_cylinder(&Plane::from_normal(DVec3::new(30.0, 10.0, 5.0) * mm - d * 20.0 * mm, d), 3.0 * mm, 40.0 * mm, std::f64::consts::TAU).unwrap()),
        ];
        for (what, tool) in tools {
            for op in [BooleanOp::Fuse, BooleanOp::Cut, BooleanOp::Common] {
                let name = format!("{deg}° {what} {op:?}");
                set_rust_boolean_only(true);
                let r = boolean(op, std::slice::from_ref(&part), std::slice::from_ref(&tool), &BooleanOptions::default());
                let (path, why) = (last_boolean_path(), last_rust_decline());
                set_rust_boolean_only(false);
                set_rust_boolean(Some(false));
                let o = boolean(op, std::slice::from_ref(&part), std::slice::from_ref(&tool), &BooleanOptions::default());
                set_rust_boolean(None);
                if path != "rust" {
                    why_not.push(format!("{name}: {why}"));
                    continue;
                }
                taken += 1;
                let d = Summary::of(&r.unwrap()).differences(&Summary::of(&o.unwrap()));
                assert!(d.is_empty(), "{name}: {d:?}");
            }
        }
    }
    // (Not yet: the 45° slot, whose section ends inside a face; commons that come out empty.)
    assert!(taken >= 19, "{taken} taken; declined: {why_not:#?}");
}
