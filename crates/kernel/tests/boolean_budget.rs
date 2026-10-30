//! A boolean stops with an error when it needs more memory than its budget
//! (lead 10-25: the user's Diamond knurl took 17 GB and ran on). Its own
//! test binary: the budget (NOSHAPE_BOOLEAN_MEMORY_MB) is read once.

use glam::DVec3;
use noshape_kernel as k;

#[test]
fn boolean_over_its_memory_budget_stops_with_an_error() {
    // SAFETY: the only test of this binary, set before any boolean runs.
    // Parallel booleans (as written: serial, the default since
    // BOOL-THREAD-CRASH, stays under 64 MB on this knurl).
    unsafe {
        std::env::set_var("NOSHAPE_BOOLEAN_MEMORY_MB", "64");
        std::env::set_var("NOSHAPE_BOP_PARALLEL", "1");
    }
    let _ = k::set_working_scale(1000.0);
    // (OCCT's pave filler's budget: noshape's Boolean takes this knurl in a few MB, so off here.)
    k::set_rust_boolean(Some(false));
    let frame = k::Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
    // A small cut stays well inside the budget.
    let block = k::make_box_corners(DVec3::ZERO, DVec3::splat(0.02)).unwrap();
    let pin = k::make_box_corners(DVec3::splat(0.005), DVec3::new(0.01, 0.01, 0.03)).unwrap();
    assert!(k::boolean(k::BooleanOp::Cut, &[block], &[pin], &k::BooleanOptions::default()).is_ok());
    // A knurl: 8 helical grooves and their mirror images, crossing (about
    // 0.5 GB without a budget).
    let (r, h) = (0.025, 0.04);
    let pitch = 2.0 * r * std::f64::consts::PI * 60f64.to_radians().tan();
    let cyl = k::make_cylinder(&frame, r, h, std::f64::consts::TAU).unwrap();
    let helix = k::make_helix(&frame, r, pitch, h, false, 0.0).unwrap();
    let (depth, half) = (0.0008, 0.0008 * 40f64.to_radians().tan() / 30f64.to_radians().cos());
    let p = |t: f64, d: f64| DVec3::new(r + d, t, 0.0);
    let face = k::make_face_from_wire(&k::make_polyline(&[p(half, 0.0), p(0.0, depth), p(-half, 0.0), p(0.0, -depth)], true).unwrap(), true).unwrap();
    let tool = k::sweep(&face, &helix, &k::SweepOptions::default()).unwrap().shape;
    let n = 8;
    let mut tools: Vec<k::Shape> = (0..n).map(|i| k::transform(&tool, &k::Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::Z, angle: std::f64::consts::TAU * i as f64 / n as f64 }, true).unwrap().shape).collect();
    let mirror = k::Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X);
    let mirrored: Vec<k::Shape> = tools.iter().map(|t| k::transform(t, &k::Transform::Mirror(mirror), true).unwrap().shape).collect();
    tools.extend(mirrored);
    let err = k::boolean(k::BooleanOp::Cut, &[cyl], &tools, &k::BooleanOptions::default()).expect_err("the knurl should run out of a 64 MB budget");
    assert!(err.to_string().contains("MB of memory"), "{err}");
}
