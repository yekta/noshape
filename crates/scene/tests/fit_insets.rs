//! Zoom to fit with covered strips at both sides (camera_ops::fit_camera_to_box_insets).

use glam::{DVec2, DVec3};
use noshape_scene::camera_ops::fit_camera_to_box_insets;

/// SKETCH-F-FIT: a flat 40 x 20 box far from the origin, fitted with 244 px
/// covered at the left (the dialog) and 40 at the right (the panel strip),
/// lands between the two, centred there.
#[test]
fn a_fit_keeps_clear_of_both_insets() {
    let mut cam = noshape_render::Camera::default();
    let (lo, hi) = (DVec3::new(0.2, 0.2, 0.0), DVec3::new(0.24, 0.22, 0.0));
    let size = DVec2::new(1097.0, 890.0);
    fit_camera_to_box_insets(&mut cam, lo, hi, size, 244.0, 40.0);
    let xs: Vec<f64> = (0..4)
        .map(|i| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, 0.0))
        .map(|c| cam.project(c, size, 1.0).expect("in front").x)
        .collect();
    let (x0, x1) = (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    assert!(x0 > 244.0 && x1 < size.x - 40.0, "{x0} .. {x1}");
    assert!(((x0 + x1) * 0.5 - (244.0 + (size.x - 40.0)) * 0.5).abs() < 1.0, "centred between the insets: {x0} .. {x1}");
}
