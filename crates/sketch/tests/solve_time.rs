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
/// splines, 1397 regions). Its regions took 1.5 s (dev build): each curve
/// pair re-derived a long spline's box, size and samples, and every
/// half-edge its curve's size, each a tessellation; now ~40 ms.
#[test]
fn fuzz_sketch_regions_quickly() {
    let path = format!("{}/tests/fixtures/solve/fz211-final.json", env!("CARGO_MANIFEST_DIR"));
    let sk: Sketch = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let t = Instant::now();
    let regions = sk.regions();
    let took = t.elapsed();
    eprintln!("{} regions in {took:?}", regions.len());
    assert_eq!(regions.len(), 1397);
    assert!(took < Duration::from_secs(1), "{took:?}");
}
