//! Geometry that depends on the camera: screen-sized handles and heads that
//! face the viewer (the Part Studio's drag arrows, triads). The owner hands
//! the renderer plain data ([`CameraGeometry`]); the render thread builds the
//! instances for each frame from that frame's camera, so they follow every
//! camera move without the UI rebuilding the scene (or re-rendering).

use crate::camera::Camera;
use crate::renderer::Uploader;
use crate::scene::Instance;
use glam::{DVec2, DVec3};

/// The view a frame is rendered for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub camera: Camera,
    /// Viewport size in logical pixels.
    pub size: DVec2,
}

impl View {
    /// A world point in logical pixels from the viewport's top-left (as
    /// `Viewport::project`); `None` behind the camera.
    pub fn project(&self, p: DVec3) -> Option<DVec2> {
        // The scene radius only sets the depth range; x and y don't use it.
        self.camera.project(p, self.size, 1.0)
    }

    /// World length of one screen pixel at `p`, measured along the view's
    /// right axis.
    pub fn world_per_px(&self, p: DVec3) -> Option<f64> {
        let right = self.camera.right();
        let (p0, p1) = (self.project(p)?, self.project(p + right * 1e-3)?);
        Some(1e-3 / (p1 - p0).length().max(1e-9))
    }

    /// The viewing direction (from the eye towards the target).
    pub fn direction(&self) -> DVec3 {
        (self.camera.target - self.camera.eye()).normalize_or_zero()
    }
}

/// Camera-dependent geometry built on the render thread for every frame.
pub trait CameraGeometry: Send + Sync {
    fn build(&self, view: &View, up: &Uploader) -> Vec<Instance>;
}
