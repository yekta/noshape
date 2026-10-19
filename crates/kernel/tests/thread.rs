//! Helical thread teeth (FeatureScript "Y Threads": a tooth profile swept along
//! a helix, clipped to the threaded region and added to the turned-down core).

use glam::DVec3;
use noshape_kernel::*;

const MM: f64 = 1e-3;

/// The default Y Threads geometry on a Ø10 x 20 shaft: core r 4.7, a six-sided
/// tooth (r 4.55..5, 0.8 wide at the root, 0.2 flat crest) swept 22 turns
/// down from z 20.5, clipped to z 0..20 and unioned with the core.
fn teeth_and_core() -> (Shape, Shape, Shape) {
    let z = |p: DVec3| Plane::from_normal(p, DVec3::Z);
    let core = make_cylinder(&z(DVec3::ZERO), 4.7 * MM, 20.0 * MM, std::f64::consts::TAU).unwrap();
    // Helix: axis -z through the origin, start at (5, 0, 20.5), right-handed travel down.
    let frame = Plane::new(DVec3::new(0.0, 0.0, 20.5) * MM, -DVec3::Z, DVec3::X);
    let helix = make_helix(&frame, 5.0 * MM, 1.0 * MM, 22.0 * MM, true, 0.0).unwrap();
    // Tooth profile in the plane y = 0 (x radial, z axial).
    let pts = [(4.55, 20.1), (4.55, 20.9), (4.7, 20.9), (5.0, 20.6), (5.0, 20.4), (4.7, 20.1)];
    let p = |(x, zz): (f64, f64)| DVec3::new(x, 0.0, zz) * MM;
    let edges: Vec<Shape> = (0..pts.len()).map(|i| make_line(p(pts[i]), p(pts[(i + 1) % pts.len()])).unwrap()).collect();
    let face = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    // Default frame: OCCT's corrected Frenet fails on a helix, `sweep` falls back to the Frenet frame.
    let teeth = sweep(&face, &helix, &SweepOptions { solid: true, ..Default::default() }).unwrap().shape;
    assert!(teeth.is_valid());
    let clip = make_cylinder(&z(DVec3::ZERO), 6.3 * MM, 20.0 * MM, std::f64::consts::TAU).unwrap();
    (core, teeth, clip)
}

/// The tooth stays in the axial plane (screw motion), so by Pappus each turn of
/// the clipped teeth outside the core adds 2π ∫ r dA over the profile part beyond
/// r 4.7: the trapezoid 0.8 → 0.2 wide from r 4.7 to 5 (∫ r dA = 0.723 mm³).
#[test]
fn helical_teeth_add_to_the_core() {
    let (core, teeth, clip) = teeth_and_core();
    let clipped = common(&teeth, &clip).unwrap().shape;
    assert!(clipped.is_valid());
    let all = fuse(&core, &clipped).unwrap().shape;
    assert!(all.is_valid());
    assert_eq!(all.count(ShapeType::Solid), 1);
    let v = volume_properties(&all).unwrap().mass / MM.powi(3);
    let expected = std::f64::consts::PI * 4.7f64.powi(2) * 20.0 + 20.0 * std::f64::consts::TAU * 0.723;
    assert!((v - expected).abs() < 0.01, "{v} vs {expected}");
}
