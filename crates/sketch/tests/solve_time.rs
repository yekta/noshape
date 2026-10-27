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
