//! Solve time in sketches with long fit splines (qa's SKETCH-SOLVE-SPLINE-SLOW: fuzz seed 211, step 89, a broken
//! sketch with a 94-point fit spline).

use noshape_sketch::*;
use std::time::{Duration, Instant};

fn sk211() -> Sketch {
    let path = format!("{}/tests/fixtures/solve/sk211-step89.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A point Coincident on the spline, then a dimension, each re-fitted the spline once per input for every gradient:
/// 68 s and minutes (0.3–0.7 s with the fit's own gradients, dev build under load). Both stay over defined as before.
#[test]
fn sk211_constraints_on_a_94_point_fit_spline_solve_quickly() {
    let mut sk = sk211();
    sk.solve();
    let t = Instant::now();
    sk.add_constraint(ConstraintKind::Coincident { a: EntityId(257), b: EntityId(215) }).unwrap();
    let r = sk.solve();
    let coincident = t.elapsed();
    assert!(matches!(r.status, SolveStatus::Overconstrained { .. }), "{:?}", r.status);
    let t = Instant::now();
    sk.add_constraint(ConstraintKind::Distance { a: EntityId(137), b: EntityId(48), value: -0.010, mode: DistanceMode::default() }).unwrap();
    let r = sk.solve();
    let distance = t.elapsed();
    assert!(matches!(r.status, SolveStatus::Overconstrained { .. }), "{:?}", r.status);
    eprintln!("coincident {coincident:?}, distance {distance:?}");
    assert!(coincident < Duration::from_secs(20) && distance < Duration::from_secs(20), "coincident {coincident:?}, distance {distance:?}");
}

/// The end of qa's sketch fuzz (seed 211, 200 steps: 438 entities, five fit
/// splines, 1385 regions with centripetal splines). Its regions took 1.5 s
/// (dev build): each curve pair re-derived a long spline's box, size and
/// samples, and every half-edge its curve's size, each a tessellation; now
/// ~40 ms.
#[test]
fn fuzz_sketch_regions_quickly() {
    let path = format!("{}/tests/fixtures/solve/fz211-final.json", env!("CARGO_MANIFEST_DIR"));
    let sk: Sketch = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let t = Instant::now();
    let regions = sk.regions();
    let took = t.elapsed();
    eprintln!("{} regions in {took:?}", regions.len());
    assert_eq!(regions.len(), 1385);
    assert!(took < Duration::from_secs(1), "{took:?}");
}

/// Timings of the two broken spline sketches (PLAN 2.7, T2): a re-solve, one with the DOF analysis, regions, a
/// dimension change, and a 30-step point drag of qa's fuzz sketch; the sk211 constraints above.
/// `tools/tiers/perf.sh -p noshape-sketch --test all -- solve_time::broken_spline_sketch_timings --ignored --nocapture`
#[test]
#[ignore = "T2: timings"]
fn broken_spline_sketch_timings() {
    use noshape_sketch::SolveOptions;
    let path = format!("{}/tests/fixtures/solve/fz211-final.json", env!("CARGO_MANIFEST_DIR"));
    let sk: Sketch = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let med = |mut v: Vec<Duration>| {
        v.sort();
        (v[v.len() / 2].as_secs_f64() * 1e3, v[v.len() - 1].as_secs_f64() * 1e3)
    };
    let (mut plain, mut analyze, mut dim) = (Vec::new(), Vec::new(), Vec::new());
    let d = sk.constraints().find(|c| c.driving && c.kind.is_dimension() && c.kind.value().is_some_and(|v| v.abs() > 1e-9)).map(|c| (c.id, c.kind.value().unwrap()));
    for _ in 0..5 {
        let mut s = sk.clone();
        let t = Instant::now();
        s.solve_with(&SolveOptions { analyze: false, ..Default::default() });
        plain.push(t.elapsed());
        let mut s = sk.clone();
        let t = Instant::now();
        s.solve_with(&SolveOptions { analyze: true, ..Default::default() });
        analyze.push(t.elapsed());
        let mut s = sk.clone();
        let t = Instant::now();
        if let Some((id, v)) = d {
            let _ = s.set_dimension_value(id, v * 1.002);
        }
        s.solve();
        dim.push(t.elapsed());
    }
    let mut s = sk.clone();
    let p = s.entities().filter(|e| e.kind() == EntityKind::Line && !e.external).flat_map(|e| e.point_refs()).find(|p| s.point(*p).is_some()).unwrap();
    let from = s.point(p).unwrap();
    let t = Instant::now();
    let each = s.begin_drag(&[(DragHandle::Point(p), from)]);
    let begin = t.elapsed().as_secs_f64() * 1e3;
    let start = s.clone();
    let mut steps = Vec::new();
    for k in 1..=30 {
        if each {
            s = start.clone();
        }
        let t = Instant::now();
        s.drag(&[(DragHandle::Point(p), from + glam::DVec2::new(0.0002 * k as f64, 0.0001 * k as f64))]);
        steps.push(t.elapsed());
    }
    let f = |(m, w): (f64, f64)| format!("median {m:.2} ms, worst {w:.2} ms");
    println!("fz211: re-solve {}; with the DOF analysis {}; a dimension change {}; drag: begin {begin:.2} ms, a step {}", f(med(plain)), f(med(analyze)), f(med(dim)), f(med(steps)));
}
