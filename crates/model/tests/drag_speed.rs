//! Drag-step speed (wall clock, so `--ignored`; for perf.yml, release build):
//! live answers each drag step of a 19-cube Planar stack in ≈21 ms whatever
//! the stack's size (qa4 DRAG-RUNAWAY, live's confirmation). Here one step is
//! the solver's whole drag solve (`solve_with`, no DOF analysis, as the app
//! runs it), median of 50 pointer moves.
//!
//! `cargo test --release -p noshape-model --test drag_speed -- --ignored --nocapture`

use glam::{DQuat, DVec3};
use noshape_model::assembly::MateType;
use noshape_model::assembly::solver::{DragTarget, Frame, SolverBody, SolverMate, solve_with};
use std::time::Instant;

const MM: f64 = 1e-3;

fn body(origin: DVec3, fixed: bool) -> SolverBody {
    SolverBody { pose: Frame::new(origin, DQuat::IDENTITY), fixed }
}

/// Median milliseconds of `steps` drags of `dragged`'s point `lp`, each
/// `step` further from where it started.
fn median_ms(bodies: &mut [SolverBody], mates: &[SolverMate], dragged: usize, lp: DVec3, step: DVec3, steps: usize) -> f64 {
    let start = bodies[dragged].pose.transform_point(lp);
    let mut ms: Vec<f64> = (1..=steps)
        .map(|k| {
            let drags = [DragTarget { body: dragged, local_point: lp, target: start + step * k as f64 }];
            let t = Instant::now();
            solve_with(bodies, mates, &[], &drags, false);
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    ms[ms.len() / 2]
}

/// Plate (fixed) + `n` 20 mm cubes, each Planar-mated on the one below.
fn planar_stack(n: usize) -> (Vec<SolverBody>, Vec<SolverMate>) {
    let mut bodies = vec![body(DVec3::ZERO, true)];
    bodies.extend((0..n).map(|i| body(DVec3::new(0.0, 0.0, (10.0 + 20.0 * i as f64) * MM), false)));
    let top = |z: f64| Frame::from_axes(DVec3::new(10.0 * MM, 10.0 * MM, z * MM), DVec3::Z, DVec3::X);
    let bot = Frame::from_axes(DVec3::new(10.0 * MM, 10.0 * MM, 0.0), DVec3::NEG_Z, DVec3::X);
    let mates = (1..=n).map(|i| SolverMate::new(MateType::Planar, i, bot, i - 1, if i == 1 { top(10.0) } else { top(20.0) })).collect();
    (bodies, mates)
}

#[test]
#[ignore]
fn planar_stack_drag_step_is_as_fast_as_live() {
    for n in [10, 19] {
        let (mut bodies, mates) = planar_stack(n);
        solve_with(&mut bodies, &mates, &[], &[], false);
        let lp = DVec3::new(10.0 * MM, 10.0 * MM, 20.0 * MM);
        // A view-plane drag: in-plane and out of it (the usual pointer case).
        let ms = median_ms(&mut bodies, &mates, n, lp, DVec3::new(1.0, 0.5, 0.4) * MM, 50);
        println!("planar stack of {n}: {ms:.2} ms per drag step");
        assert!(ms < 21.0, "{n} cubes: {ms:.2} ms per drag step (live ≈21 ms)");
    }
}

#[test]
#[ignore]
fn revolute_chain_drag_step_time() {
    // 20 links chained by revolutes (the whole chain moves): reported only.
    let n = 20;
    let mut bodies: Vec<SolverBody> = (0..n).map(|i| body(DVec3::new(44.0 * i as f64 * MM, 0.0, 5.0 * i as f64 * MM), i == 0)).collect();
    let top = Frame::from_axes(DVec3::new(52.0 * MM, 6.0 * MM, 5.0 * MM), DVec3::Z, DVec3::X);
    let bot = Frame::from_axes(DVec3::new(8.0 * MM, 6.0 * MM, 0.0), DVec3::NEG_Z, DVec3::X);
    let mates: Vec<SolverMate> = (1..n).map(|i| SolverMate::new(MateType::Revolute, i, bot, i - 1, top)).collect();
    solve_with(&mut bodies, &mates, &[], &[], false);
    let ms = median_ms(&mut bodies, &mates, n - 1, DVec3::new(30.0 * MM, 6.0 * MM, 0.0), DVec3::new(-1.0, 0.5, 0.0) * MM, 50);
    println!("revolute chain of {n}: {ms:.2} ms per drag step");
}
