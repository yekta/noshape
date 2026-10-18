//! Orbit camera with orthographic and perspective projections.
//!
//! World convention matches Onshape: Z is up, the Top plane is XY, the Front
//! plane is XZ (normal -Y points toward the viewer) and the Right plane is YZ.

use glam::{DMat4, DQuat, DVec2, DVec3, Mat4};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    Orthographic,
    Perspective,
}

/// Standard named views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardView {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    Isometric,
    Dimetric,
    Trimetric,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Point the camera orbits around.
    pub target: DVec3,
    /// Rotation taking camera space (looking down -Z, Y up) to world space.
    pub rotation: DQuat,
    /// Distance from the target to the eye (perspective) and the half-height
    /// of the visible region (orthographic) are both derived from this.
    pub distance: f64,
    pub projection: Projection,
    /// Vertical field of view in radians (perspective only).
    pub fov_y: f64,
}

impl Default for Camera {
    fn default() -> Self {
        let mut c = Self {
            target: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            distance: 0.25,
            projection: Projection::Orthographic,
            fov_y: 30f64.to_radians(),
        };
        c.set_view(StandardView::Isometric);
        c
    }
}

impl Camera {
    pub fn forward(&self) -> DVec3 {
        self.rotation * DVec3::NEG_Z
    }
    pub fn up(&self) -> DVec3 {
        self.rotation * DVec3::Y
    }
    pub fn right(&self) -> DVec3 {
        self.rotation * DVec3::X
    }
    pub fn eye(&self) -> DVec3 {
        self.target - self.forward() * self.distance
    }

    /// Half-height of the view volume at the target, in world units.
    pub fn half_height(&self) -> f64 {
        self.distance * (self.fov_y * 0.5).tan()
    }

    /// World units per pixel at the target depth for a viewport of `height_px`.
    pub fn world_per_pixel(&self, height_px: f64) -> f64 {
        2.0 * self.half_height() / height_px.max(1.0)
    }

    pub fn rotation_for(view: StandardView) -> DQuat {
        // Build from forward/up vectors (forward = direction the camera looks).
        let (fwd, up) = match view {
            StandardView::Front => (DVec3::Y, DVec3::Z),
            StandardView::Back => (DVec3::NEG_Y, DVec3::Z),
            StandardView::Left => (DVec3::X, DVec3::Z),
            StandardView::Right => (DVec3::NEG_X, DVec3::Z),
            StandardView::Top => (DVec3::NEG_Z, DVec3::Y),
            StandardView::Bottom => (DVec3::Z, DVec3::NEG_Y),
            // Onshape's isometric (measured live): the camera sits 30° to the
            // right of the front view and 30° above the ground plane.
            StandardView::Isometric => (DVec3::new(-0.4330127, 0.75, -0.5).normalize(), DVec3::Z),
            StandardView::Dimetric => (DVec3::new(-1.0, 1.0, -0.5).normalize(), DVec3::Z),
            StandardView::Trimetric => (DVec3::new(-0.8, 1.0, -0.6).normalize(), DVec3::Z),
        };
        Self::look_rotation(fwd, up)
    }

    /// Rotation whose -Z axis is `forward` and whose Y axis is as close to `up` as possible.
    pub fn look_rotation(forward: DVec3, up: DVec3) -> DQuat {
        let f = forward.normalize();
        let mut r = f.cross(up);
        if r.length_squared() < 1e-12 {
            r = f.any_orthonormal_vector();
        }
        let r = r.normalize();
        let u = r.cross(f);
        DQuat::from_mat3(&glam::DMat3::from_cols(r, u, -f)).normalize()
    }

    pub fn set_view(&mut self, view: StandardView) {
        self.rotation = Self::rotation_for(view);
    }

    /// Orbit around the target by pixel deltas (turntable style around world Z,
    /// like Onshape's default right-drag rotation).
    pub fn orbit(&mut self, delta_px: DVec2, viewport_height: f64) {
        let speed = std::f64::consts::PI / viewport_height.max(1.0) * 1.2;
        let yaw = DQuat::from_axis_angle(DVec3::Z, -delta_px.x * speed);
        let pitch = DQuat::from_axis_angle(self.right(), -delta_px.y * speed);
        self.rotation = (yaw * pitch * self.rotation).normalize();
    }

    /// Free trackball rotation around the camera's own axes.
    pub fn orbit_free(&mut self, delta_px: DVec2, viewport_height: f64) {
        let speed = std::f64::consts::PI / viewport_height.max(1.0) * 1.2;
        let yaw = DQuat::from_axis_angle(self.up(), -delta_px.x * speed);
        let pitch = DQuat::from_axis_angle(self.right(), -delta_px.y * speed);
        self.rotation = (yaw * pitch * self.rotation).normalize();
    }

    /// Orbit around an arbitrary pivot point (e.g. the point under the cursor).
    pub fn orbit_around(&mut self, pivot: DVec3, delta_px: DVec2, viewport_height: f64) {
        let before = self.rotation;
        self.orbit(delta_px, viewport_height);
        let delta = self.rotation * before.inverse();
        self.target = pivot + delta * (self.target - pivot);
    }

    /// Pan so the scene follows the cursor.
    pub fn pan(&mut self, delta_px: DVec2, viewport_height: f64) {
        let wpp = self.world_per_pixel(viewport_height);
        self.target += (-self.right() * delta_px.x + self.up() * delta_px.y) * wpp;
    }

    /// Zoom by `factor` (< 1 zooms in) keeping the world point under `cursor_px`
    /// fixed on screen.
    pub fn zoom_at(&mut self, factor: f64, cursor_px: DVec2, viewport: DVec2) {
        let factor = factor.clamp(0.01, 100.0);
        let before = self.unproject_on_target_plane(cursor_px, viewport);
        self.distance = (self.distance * factor).clamp(1e-7, 1e7);
        let after = self.unproject_on_target_plane(cursor_px, viewport);
        self.target += before - after;
    }

    /// Point on the plane through the target facing the camera under `cursor_px`.
    pub fn unproject_on_target_plane(&self, cursor_px: DVec2, viewport: DVec2) -> DVec3 {
        let ndc = DVec2::new(
            cursor_px.x / viewport.x.max(1.0) * 2.0 - 1.0,
            1.0 - cursor_px.y / viewport.y.max(1.0) * 2.0,
        );
        let aspect = viewport.x / viewport.y.max(1.0);
        let hh = self.half_height();
        self.target + self.right() * ndc.x * hh * aspect + self.up() * ndc.y * hh
    }

    /// Ray (origin, direction) through a pixel.
    pub fn ray(&self, cursor_px: DVec2, viewport: DVec2) -> (DVec3, DVec3) {
        let p = self.unproject_on_target_plane(cursor_px, viewport);
        match self.projection {
            Projection::Orthographic => (p - self.forward() * self.distance * 1000.0, self.forward()),
            Projection::Perspective => {
                let eye = self.eye();
                (eye, (p - eye).normalize())
            }
        }
    }

    /// Fit a bounding sphere into view.
    pub fn fit_sphere(&mut self, center: DVec3, radius: f64, aspect: f64) {
        let radius = radius.max(1e-6);
        self.target = center;
        let half = (self.fov_y * 0.5).tan();
        let needed_half_height = if aspect < 1.0 { radius / aspect } else { radius } * 1.15;
        self.distance = needed_half_height / half;
    }

    /// Fit an axis-aligned box as seen from the current orientation: its
    /// projected extent fills the view (with a small margin), which frames
    /// parts much tighter than [`Self::fit_sphere`]. In perspective the eye
    /// backs off by the box's depth so its near side fits too.
    pub fn fit_box(&mut self, lo: DVec3, hi: DVec3, aspect: f64) {
        let (r, u, f) = (self.right(), self.up(), self.forward());
        let (mut x0, mut x1, mut y0, mut y1, mut z0, mut z1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for i in 0..8 {
            let c = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
            let (x, y, z) = (c.dot(r), c.dot(u), c.dot(f));
            (x0, x1, y0, y1, z0, z1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y), z0.min(z), z1.max(z));
        }
        if !(x0 <= x1 && y0 <= y1) {
            return;
        }
        self.target = r * ((x0 + x1) * 0.5) + u * ((y0 + y1) * 0.5) + f * ((z0 + z1) * 0.5);
        let half = ((y1 - y0) * 0.5).max((x1 - x0) * 0.5 / aspect.max(1e-6)).max(1e-6) * 1.12;
        self.distance = half / (self.fov_y * 0.5).tan();
        if self.projection == Projection::Perspective {
            self.distance += (z1 - z0) * 0.5;
        }
    }

    pub fn view_matrix(&self) -> DMat4 {
        look_at_rh(self.eye(), self.target, self.up())
    }

    /// Projection using reverse-Z (near maps to 1, far to 0) for precision.
    /// `scene_radius` bounds the scene around the target for ortho depth range.
    pub fn projection_matrix(&self, aspect: f64, scene_radius: f64) -> DMat4 {
        let aspect = aspect.max(1e-6);
        match self.projection {
            Projection::Perspective => {
                let near = (self.distance * 1e-3).max(1e-7);
                perspective_infinite_reverse_rh(self.fov_y, aspect, near)
            }
            Projection::Orthographic => {
                let hh = self.half_height();
                let hw = hh * aspect;
                let depth = (scene_radius.max(self.distance) * 4.0).max(1e-3);
                // Eye sits `distance` in front of target; cover the scene on both sides.
                let near = self.distance - depth;
                let far = self.distance + depth;
                // Reverse-Z: swap near and far.
                orthographic_rh(-hw, hw, -hh, hh, far, near)
            }
        }
    }

    pub fn view_proj_f32(&self, aspect: f64, scene_radius: f64) -> Mat4 {
        (self.projection_matrix(aspect, scene_radius) * self.view_matrix()).as_mat4()
    }

    /// Project a world point to pixel coordinates; returns None when behind the camera.
    pub fn project(&self, p: DVec3, viewport: DVec2, scene_radius: f64) -> Option<DVec2> {
        let vp = self.projection_matrix(viewport.x / viewport.y.max(1.0), scene_radius) * self.view_matrix();
        let clip = vp * p.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(DVec2::new((ndc.x + 1.0) * 0.5 * viewport.x, (1.0 - ndc.y) * 0.5 * viewport.y))
    }

    /// Interpolate between two cameras (for animated view changes).
    pub fn lerp(&self, other: &Camera, t: f64) -> Camera {
        let t = t.clamp(0.0, 1.0);
        let s = t * t * (3.0 - 2.0 * t);
        Camera {
            target: self.target.lerp(other.target, s),
            rotation: self.rotation.slerp(other.rotation, s),
            distance: self.distance * (other.distance / self.distance).powf(s),
            projection: other.projection,
            fov_y: other.fov_y,
        }
    }
}

/// Right-handed view matrix.
pub fn look_at_rh(eye: DVec3, center: DVec3, up: DVec3) -> DMat4 {
    let f = (center - eye).normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    DMat4::from_cols(
        glam::DVec4::new(s.x, u.x, -f.x, 0.0),
        glam::DVec4::new(s.y, u.y, -f.y, 0.0),
        glam::DVec4::new(s.z, u.z, -f.z, 0.0),
        glam::DVec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Right-handed infinite perspective with reverse Z (depth 1 at `near`, 0 at infinity).
pub fn perspective_infinite_reverse_rh(fov_y: f64, aspect: f64, near: f64) -> DMat4 {
    let f = 1.0 / (0.5 * fov_y).tan();
    DMat4::from_cols(
        glam::DVec4::new(f / aspect, 0.0, 0.0, 0.0),
        glam::DVec4::new(0.0, f, 0.0, 0.0),
        glam::DVec4::new(0.0, 0.0, 0.0, -1.0),
        glam::DVec4::new(0.0, 0.0, near, 0.0),
    )
}

/// Right-handed orthographic projection mapping view depth `near..far` to 0..1.
/// Pass `near > far` (swapped) for reverse Z.
pub fn orthographic_rh(left: f64, right: f64, bottom: f64, top: f64, near: f64, far: f64) -> DMat4 {
    let rcp_w = 1.0 / (right - left);
    let rcp_h = 1.0 / (top - bottom);
    let r = 1.0 / (near - far);
    DMat4::from_cols(
        glam::DVec4::new(2.0 * rcp_w, 0.0, 0.0, 0.0),
        glam::DVec4::new(0.0, 2.0 * rcp_h, 0.0, 0.0),
        glam::DVec4::new(0.0, 0.0, r, 0.0),
        glam::DVec4::new(-(left + right) * rcp_w, -(top + bottom) * rcp_h, r * near, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_box_frames_the_projected_extent() {
        let mut c = Camera::default();
        c.set_view(StandardView::Front);
        // A wide, flat box: 100 x 10 x 20 mm, seen from the front (X right, Z up).
        let (lo, hi) = (DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.1, 0.01, 0.02));
        c.fit_box(lo, hi, 2.0);
        let vp = DVec2::new(800.0, 400.0);
        // Centered and filling the width with a margin.
        let a = c.project(DVec3::new(0.0, 0.0, 0.01), vp, 1.0).unwrap();
        let b = c.project(DVec3::new(0.1, 0.0, 0.01), vp, 1.0).unwrap();
        assert!((a.x + b.x - 800.0).abs() < 1e-6 && (a.y - 200.0).abs() < 1e-6, "{a} {b}");
        let fill = (b.x - a.x) / 800.0;
        assert!(fill > 0.85 && fill < 0.95, "fill {fill}");
        // Much tighter than the bounding sphere.
        let mut s = c;
        s.fit_sphere((lo + hi) * 0.5, (hi - lo).length() * 0.5, 2.0);
        assert!(c.distance < s.distance * 0.6);
        // Perspective: the near side still fits.
        c.projection = Projection::Perspective;
        c.set_view(StandardView::Isometric);
        c.fit_box(lo, hi, 2.0);
        for i in 0..8 {
            let p = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
            let q = c.project(p, vp, 1.0).unwrap();
            assert!(q.x >= 0.0 && q.x <= 800.0 && q.y >= 0.0 && q.y <= 400.0, "corner {i} at {q}");
        }
    }

    #[test]
    fn standard_views_look_along_expected_axes() {
        let mut c = Camera::default();
        c.set_view(StandardView::Top);
        assert!((c.forward() - DVec3::NEG_Z).length() < 1e-9);
        assert!((c.up() - DVec3::Y).length() < 1e-9);
        c.set_view(StandardView::Front);
        assert!((c.forward() - DVec3::Y).length() < 1e-9);
        assert!((c.up() - DVec3::Z).length() < 1e-9);
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let mut c = Camera::default();
        let vp = DVec2::new(800.0, 600.0);
        let cursor = DVec2::new(100.0, 50.0);
        let before = c.unproject_on_target_plane(cursor, vp);
        c.zoom_at(0.5, cursor, vp);
        let after = c.unproject_on_target_plane(cursor, vp);
        assert!((before - after).length() < 1e-9);
    }

    #[test]
    fn project_round_trip() {
        let c = Camera::default();
        let vp = DVec2::new(800.0, 600.0);
        let cursor = DVec2::new(300.0, 200.0);
        let p = c.unproject_on_target_plane(cursor, vp);
        let back = c.project(p, vp, 1.0).unwrap();
        assert!((back - cursor).length() < 1e-6);
    }
}
