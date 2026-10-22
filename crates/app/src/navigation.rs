//! View navigation exactly like Onshape's web client (its `webgl` bundle's
//! view controller, `bundle-raw/webgl.*.js`, plus the client constants in
//! `woolsthorpe.*.js`): which mouse button + modifier does what (the "Mouse
//! controls" preference and its presets), how far a wheel notch, a trackpad
//! scroll or a pinch zooms, rotation speed and inertia, and the keyboard
//! steps (arrows, Shift/Ctrl+arrows, Ctrl+Shift+arrows, Z / Shift+Z).
//!
//! Everything here is pure input-to-camera math so it can be unit tested;
//! `viewport.rs` (3D) and the drawing canvas (2D) feed it platform events.
//! The parity checklist is docs/parity/navigation.md.

use glam::{DQuat, DVec2, DVec3};
use gpui_kit::ScrollDelta;
use noshape_render::{Camera, Projection};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};

// ------------------------------------------------------------ constants

/// Arrow key step (`ROTATE_ANGLE`).
pub const ROTATE_ANGLE_DEG: f64 = 15.0;
/// Ctrl+arrow step (`PRECISION_ROTATE_ANGLE`).
pub const PRECISION_ROTATE_ANGLE_DEG: f64 = 5.0;
/// Shift+arrow step (`COARSE_ROTATE_ANGLE`).
pub const COARSE_ROTATE_ANGLE_DEG: f64 = 90.0;
/// Mouse rotation: full turns per viewport diagonal (`ROTATION_CYCLES_PER_SCREEN`).
pub const ROTATION_CYCLES_PER_SCREEN: f64 = 3.0;
/// Touch rotation (`POINTER_ROTATION_CYCLES_PER_SCREEN`).
pub const POINTER_ROTATION_CYCLES_PER_SCREEN: f64 = 2.0;
/// Rotation inertia decay per 60 Hz frame (`ROTATION_INERTIA`).
pub const ROTATION_INERTIA: f64 = 0.77;
/// Inertia only follows a drag whose last rotation is this recent.
pub const INERTIA_RELEASE_WINDOW: Duration = Duration::from_millis(50);
/// Ctrl+Shift+arrow pan (`DEFAULT_PAN_AMOUNT_PX`).
pub const DEFAULT_PAN_AMOUNT_PX: f64 = 150.0;
/// Z / Shift+Z zoom factor (`DEFAULT_ZOOM_FACTOR`).
pub const DEFAULT_ZOOM_FACTOR: f64 = 1.75;
/// Drag zoom: device pixels per zoom step (`DRAG_ZOOM_FACTOR`).
pub const DRAG_ZOOM_FACTOR: f64 = 10.0;
/// Zoom out stops once the model is this small on screen (`MIN_MODEL_PIXEL_SIZE`).
pub const MIN_MODEL_PIXEL_SIZE: f64 = 100.0;
/// Zoom in stops at this many pixels per metre (2^24).
pub const MAX_PIXELS_PER_METRE: f64 = 16_777_216.0;
/// Perspective eye distance limits, in scene diameters (`MIN_SCALE`, `MAX_SCALE`).
pub const PERSPECTIVE_MIN_SCALE: f64 = 0.05;
pub const PERSPECTIVE_MAX_SCALE: f64 = 50.0;
/// Constrained rotate levels the view first unless it is this close (radians,
/// `AXIS_ROTATION_SNAP_TOLERANCE`).
pub const AXIS_ROTATION_SNAP_TOLERANCE: f64 = 0.001;
/// `CAMERA_SNAP_ANIMATION_TIME_MS`, `CAMERA_SHORT_…`, `CAMERA_LONG_…`.
pub const CAMERA_SNAP_ANIMATION: Duration = Duration::from_millis(125);
pub const CAMERA_SHORT_ANIMATION: Duration = Duration::from_millis(250);
pub const CAMERA_LONG_ANIMATION: Duration = Duration::from_millis(500);
/// Drag distance (logical px) before a press becomes a drag.
pub const DRAG_THRESHOLD_PX: f64 = 3.0;

// ------------------------------------------------------------ platform

/// The platform's conventions for wheel deltas (Onshape scales its zoom by
/// the browser's `wheelDelta`, which each OS reports differently).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::Mac
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// Windows: lines scrolled per wheel notch (the system setting gpui applies
/// to `ScrollDelta::Lines`), so a notch can be recovered from the lines.
#[cfg(target_os = "windows")]
fn wheel_scroll_lines() -> f32 {
    use windows::Win32::UI::WindowsAndMessaging::{SPI_GETWHEELSCROLLLINES, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW};
    let mut lines: u32 = 3;
    // SAFETY: SPI_GETWHEELSCROLLLINES writes one u32 to the pointer.
    let ok = unsafe { SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, Some((&mut lines as *mut u32).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
    // WHEEL_PAGESCROLL (u32::MAX) scrolls a page per notch; gpui then reports
    // that huge number of lines, which still divides back to one notch.
    if ok.is_ok() && lines > 0 { lines as f32 } else { 3.0 }
}

#[cfg(not(target_os = "windows"))]
fn wheel_scroll_lines() -> f32 {
    3.0
}

/// The browser's `wheelDelta` (120 per notch, positive = wheel up / two
/// fingers moving toward "scroll up") for a gpui scroll delta, per platform:
/// * macOS: Chrome reports 120 per line, and 3 per precise (trackpad) pixel
///   (40 px per tick).
/// * Windows: gpui reports `notches × wheel scroll lines`; precision touchpad
///   pixels (DirectManipulation) are 100 px per notch.
/// * Linux: gpui reports 3 lines per notch (X11 and Wayland); Wayland touchpad
///   pixels are libinput units ×3, 15 units per notch.
pub fn wheel_delta(delta: ScrollDelta, platform: Platform) -> DVec2 {
    match (delta, platform) {
        (ScrollDelta::Lines(l), Platform::Mac) => DVec2::new(l.x as f64, l.y as f64) * 120.0,
        (ScrollDelta::Lines(l), Platform::Windows) => DVec2::new(l.x as f64, l.y as f64) * (120.0 / wheel_scroll_lines() as f64),
        (ScrollDelta::Lines(l), Platform::Linux) => DVec2::new(l.x as f64, l.y as f64) * 40.0,
        (ScrollDelta::Pixels(p), platform) => {
            let per_px = match platform {
                Platform::Mac => 3.0,
                Platform::Windows => 1.2,
                Platform::Linux => 120.0 / 45.0,
            };
            DVec2::new(f64::from(f32::from(p.x)), f64::from(f32::from(p.y))) * per_px
        }
    }
}

/// Onshape's zoom amount for a wheel event (`zoomOnMouseWheelScroll`):
/// positive zooms in. Every scroll zooms (mouse wheel, two-finger trackpad
/// scroll, its momentum, Ctrl+wheel); mostly horizontal scrolls are ignored.
/// 0.5 per notch, ×3 on Windows; "Reverse scroll wheel zoom direction" flips
/// it. Shift+wheel: gpui turns it horizontal on Linux, but Chrome on Linux
/// keeps it vertical and Onshape zooms (live: Shift+wheel = plain wheel), so
/// the swap is undone there; macOS (the OS) and Chrome on Windows make it
/// horizontal themselves, and Onshape ignores it.
pub fn wheel_zoom_amount(delta: ScrollDelta, platform: Platform, reverse: bool, shift: bool) -> Option<f64> {
    let mut w = wheel_delta(delta, platform);
    if shift && platform == Platform::Linux && w.y == 0.0 {
        w = DVec2::new(0.0, w.x);
    }
    if w.y == 0.0 || (w.x != 0.0 && w.x.abs() > w.y.abs()) {
        return None;
    }
    let mut n = 0.5 / 120.0;
    if platform == Platform::Windows {
        n *= 3.0;
    }
    let i = n * w.y;
    Some(if reverse { -i } else { i })
}

/// Scale of the visible region for a zoom amount (`2^(-i/6)`; < 1 zooms in).
pub fn zoom_scale(amount: f64) -> f64 {
    (-amount / 6.0).exp2()
}

/// How a zoom is specified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    /// Wheel / drag / keyboard amount (`zoomPoint`): positive zooms in.
    Amount(f64),
    /// Pinch: the fingers' distance ratio since the last event (`pinchZoom`);
    /// > 1 zooms in, and the model scales exactly with the fingers.
    Pinch(f64),
}

/// The pinch ratio for a gpui [`gpui_kit::PinchEvent`] delta (macOS
/// magnification, Windows DirectManipulation and Linux gesture scale deltas
/// are all `ratio - 1`).
pub fn pinch_ratio(delta: f32) -> f64 {
    (1.0 + delta as f64).max(0.05)
}

/// Pixel size (world units per logical pixel) at a world point.
pub fn pixel_size_at(camera: &Camera, p: DVec3, viewport: DVec2) -> f64 {
    let wpp = camera.world_per_pixel(viewport.y);
    match camera.projection {
        Projection::Orthographic => wpp,
        Projection::Perspective => {
            let depth = (p - camera.eye()).dot(camera.forward());
            wpp * (depth / camera.distance.max(1e-12)).max(1e-6)
        }
    }
}

/// Onshape's `canZoom`: zooming by `scale` (> 1 zooms out) is refused once the
/// model would be under 100 px across, or past 2^24 px per metre.
pub fn can_zoom(camera: &Camera, scale: f64, bounds: Option<(DVec3, DVec3)>, viewport: DVec2) -> bool {
    let Some((lo, hi)) = bounds else { return false };
    let diameter = (hi - lo).length();
    if diameter <= 0.0 {
        return false;
    }
    let px = pixel_size_at(camera, (lo + hi) * 0.5, viewport);
    !(scale >= 1.0 && diameter / px <= scale * MIN_MODEL_PIXEL_SIZE || scale <= 1.0 && 1.0 / px >= scale * MAX_PIXELS_PER_METRE)
}

/// Zoom about a viewport point (`zoomOnCanvasPoint` / `pinchZoom`): the
/// world point under `at` stays put. Orthographic views scale by
/// [`zoom_scale`] (or `1/ratio`); perspective eyes dolly along the ray by
/// `distance × i / 6` (or so the model follows the fingers), within
/// 0.05–50 scene diameters. Returns false when Onshape refuses the zoom.
pub fn zoom_at(camera: &mut Camera, zoom: Zoom, at: DVec2, viewport: DVec2, bounds: Option<(DVec3, DVec3)>) -> bool {
    let gate = match zoom {
        Zoom::Amount(i) => zoom_scale(i),
        Zoom::Pinch(t) => 1.0 / t,
    };
    if !gate.is_finite() || gate == 1.0 || !can_zoom(camera, gate, bounds, viewport) {
        return false;
    }
    match camera.projection {
        Projection::Orthographic => camera.zoom_at(gate, at, viewport),
        Projection::Perspective => {
            let d = camera.distance;
            let diameter = bounds.map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.0);
            let (dmin, dmax) = (diameter * PERSPECTIVE_MIN_SCALE, diameter * PERSPECTIVE_MAX_SCALE);
            // Length of the eye's move along the ray through `at`.
            let e = match zoom {
                Zoom::Amount(i) => d.max(dmin) * i / 6.0,
                Zoom::Pinch(t) => d - d / t,
            };
            let (_, dir) = camera.ray(at, viewport);
            let r = camera.forward().dot(dir).max(1e-6);
            // Forward part of the move, clamped like `dollyAlong`.
            let mut o = r * e;
            if d < dmin {
                o = o.min(0.0);
            } else if d - o < dmin {
                o = (dmin - d).max(0.0);
            } else if d - o > dmax {
                o = (d - dmax).min(0.0);
            }
            if o == 0.0 {
                return false;
            }
            // Moving the eye along the ray keeps the point under `at`; that is
            // `Camera::zoom_at` with the new distance.
            camera.zoom_at((d - o) / d, at, viewport);
        }
    }
    true
}

/// Onshape's `adjustCameraToValidZoom` for keyboard zoom and fits: instead
/// of refusing, clamp the zoom into the valid range (about the view centre).
pub fn clamp_zoom(camera: &mut Camera, bounds: Option<(DVec3, DVec3)>, viewport: DVec2) {
    let Some((lo, hi)) = bounds else { return };
    let diameter = (hi - lo).length();
    if diameter <= 0.0 {
        return;
    }
    let px = pixel_size_at(camera, (lo + hi) * 0.5, viewport);
    let s = if diameter / px <= MIN_MODEL_PIXEL_SIZE {
        diameter / px / MIN_MODEL_PIXEL_SIZE
    } else if 1.0 / px >= MAX_PIXELS_PER_METRE {
        1.0 / (MAX_PIXELS_PER_METRE * px)
    } else {
        return;
    };
    camera.distance *= s;
}

// ------------------------------------------------------------ rotation

/// Rotate the camera by `angle` (radians) about a world `axis` through
/// `pivot` (Onshape's `camera.rotateAbout`; positive turns the model the way
/// a drag toward −x / −y does).
pub fn rotate_about(camera: &mut Camera, axis: DVec3, angle: f64, pivot: DVec3) {
    if angle == 0.0 || axis.length_squared() < 1e-24 {
        return;
    }
    let q = DQuat::from_axis_angle(axis.normalize(), angle);
    camera.rotation = (q * camera.rotation).normalize();
    camera.target = pivot + q * (camera.target - pivot);
}

/// Radians per pointer pixel of drag (`2π × cycles / canvas diagonal`).
/// Onshape scales both the drag (`onDrag(t, i·dpr, n·dpr)`) and the canvas
/// size by its device pixel ratio, so per logical pixel it is this at any
/// ratio.
pub fn rotation_per_px(viewport: DVec2, touch: bool) -> f64 {
    let cycles = if touch { POINTER_ROTATION_CYCLES_PER_SCREEN } else { ROTATION_CYCLES_PER_SCREEN };
    std::f64::consts::TAU * cycles / viewport.length().max(1.0)
}

/// Free 3D rotate for a drag `delta` (logical px, y down): about the camera's
/// up axis, then its right axis, through `pivot`.
pub fn drag_rotate(camera: &mut Camera, delta: DVec2, viewport: DVec2, pivot: DVec3, touch: bool) {
    let r = rotation_per_px(viewport, touch);
    rotate_about(camera, camera.up(), -delta.x * r, pivot);
    rotate_about(camera, camera.right(), -delta.y * r, pivot);
}

/// Constrained rotate's up axis (`axisRotateUpAxis`: world Z).
pub const AXIS_ROTATE_UP: DVec3 = DVec3::Z;

/// How far the camera is rolled from a "floor down" view: the angle that
/// levels its right vector (radians), and the axis to level it about.
pub fn roll_to_level(camera: &Camera) -> (f64, DVec3) {
    let right = camera.right();
    let e = right.dot(AXIS_ROTATE_UP).clamp(-1.0, 1.0).asin();
    (e, right.cross(AXIS_ROTATE_UP))
}

/// Constrained rotate step for a drag `delta` once the view is level:
/// horizontal moves turn about world Z (sign follows the camera's up so it
/// never feels inverted), vertical moves pitch about the camera's right.
pub fn drag_axis_rotate(camera: &mut Camera, delta: DVec2, viewport: DVec2, pivot: DVec3) {
    let t = rotation_per_px(viewport, false);
    let s = camera.up().z.signum();
    let s = if s == 0.0 { 1.0 } else { s };
    rotate_about(camera, AXIS_ROTATE_UP, -delta.x * s * t, pivot);
    rotate_about(camera, camera.right(), -delta.y * t, pivot);
}

/// The camera levelled to the nearest floor-down view (Alt animates there
/// before constrained rotating), and how long that animation takes; `None`
/// when it is already level within the tolerance (the caller levels it
/// in place then).
pub fn level_view(camera: &Camera, pivot: DVec3) -> (Camera, Option<Duration>) {
    let (e, axis) = roll_to_level(camera);
    let mut to = *camera;
    rotate_about(&mut to, axis, -e, pivot);
    if e.abs() < AXIS_ROTATION_SNAP_TOLERANCE {
        (to, None)
    } else if e.abs() > std::f64::consts::FRAC_PI_4 {
        (to, Some(CAMERA_SHORT_ANIMATION))
    } else {
        (to, Some(CAMERA_SNAP_ANIMATION))
    }
}

/// Rotation speed tracking and the post-release inertia (`rotationSpeedX/Y`,
/// `performStopRotationAnimation`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RotationSpeed(pub DVec2);

impl RotationSpeed {
    /// Each drag step averages into the speed.
    pub fn push(&mut self, delta: DVec2) {
        self.0 = (self.0 + delta) * 0.5;
    }
}

/// Inertia after a rotate drag: decays by [`ROTATION_INERTIA`] per 60 Hz
/// frame (time based) and stops below a pixel per step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inertia {
    pub speed: DVec2,
    pub factor: f64,
    pub pivot: DVec3,
}

impl Inertia {
    /// The rotation (logical px, like a drag delta) for `elapsed` since the
    /// last step, or `None` when the inertia has died out.
    pub fn step(&mut self, elapsed: Duration) -> Option<DVec2> {
        self.factor *= ROTATION_INERTIA.powf(elapsed.as_secs_f64() * 60.0);
        let d = self.speed * self.factor;
        (d.x.abs() > 1.0 || d.y.abs() > 1.0).then_some(d)
    }
}

/// How long after a camera change frames count as interactive
/// (`setUserIsInteractingWithTimeout`).
pub const INTERACTIVE_WINDOW: Duration = Duration::from_millis(500);
/// Below this interactive frame rate Onshape reduces detail and drops the
/// post-rotation inertia (`DETAIL_REDUCTION_THRESHOLD_FPS`).
pub const DETAIL_REDUCTION_THRESHOLD_FPS: f64 = 20.0;
/// Interactive frames between two frame-interval timings (`timingFrequency`).
const TIMING_FREQUENCY: u32 = 10;
/// Timings averaged (`addTiming` keeps the last 5).
const TIMINGS_KEPT: usize = 5;

/// The last [`TIMINGS_KEPT`] timings, their mean (`averageTime`) and how
/// many were added since the last reset (`timingCount`).
#[derive(Clone, Debug, Default)]
struct Timings {
    kept: std::collections::VecDeque<f64>,
    count: u32,
}

impl Timings {
    fn add(&mut self, ms: f64) {
        self.kept.push_back(ms);
        if self.kept.len() > TIMINGS_KEPT {
            self.kept.pop_front();
        }
        self.count += 1;
    }

    fn average(&self) -> Option<f64> {
        (!self.kept.is_empty()).then(|| self.kept.iter().sum::<f64>() / self.kept.len() as f64)
    }
}

/// Onshape's frame timer for interactive frames (`FrameTimer`, frame type
/// INTERACTIVE): every frame's draw time (`drawLoopDetails`), and every
/// [`TIMING_FREQUENCY`] frames the slower of two consecutive frame
/// intervals, measured with forced draws (`frameDetails`); each averaged
/// over the last [`TIMINGS_KEPT`].
#[derive(Clone, Debug, Default)]
pub struct FrameTimer {
    draw: Timings,
    frame: Timings,
    since_timing: u32,
    /// A frame-interval timing in progress: its last draw start and the
    /// intervals so far.
    timing: Option<(Instant, Vec<f64>)>,
}

impl FrameTimer {
    /// An interactive frame starts drawing (`onDrawStart`). True while a
    /// frame-interval timing runs: the caller draws the next frame at once
    /// (`forceRequestDraw`).
    pub fn draw_start(&mut self, at: Instant) -> bool {
        match &mut self.timing {
            Some((start, intervals)) => {
                intervals.push((at - *start).as_secs_f64() * 1000.0);
                if intervals.len() < 2 {
                    *start = at;
                    return true;
                }
                let slower = intervals[0].max(intervals[1]);
                self.frame.add(slower);
                self.since_timing = 0;
                self.timing = None;
                false
            }
            None => {
                self.since_timing += 1;
                if self.since_timing >= TIMING_FREQUENCY {
                    self.timing = Some((at, Vec::new()));
                    return true;
                }
                false
            }
        }
    }

    /// An interactive frame took `draw` from its start (`onDrawEnd`).
    pub fn draw_end(&mut self, draw: Duration) {
        self.draw.add(draw.as_secs_f64() * 1000.0);
    }

    /// Frames slower than [`DETAIL_REDUCTION_THRESHOLD_FPS`] on average.
    fn too_slow(average: Option<f64>) -> bool {
        average.is_some_and(|a| a > 1000.0 / DETAIL_REDUCTION_THRESHOLD_FPS)
    }

    /// `isInteractiveFramerateBelowThreshold`.
    pub fn below_threshold(&self) -> bool {
        Self::too_slow(self.draw.average()) || Self::too_slow(self.frame.average())
    }
}

/// Above this interactive frame rate detail comes back
/// (`DETAIL_ENHANCEMENT_THRESHOLD_FPS`).
pub const DETAIL_ENHANCEMENT_THRESHOLD_FPS: f64 = 30.0;
/// Lowest-LOD body share steps (`LOW_LEVEL_BODY_PROPORTION_STEP`,
/// `MAX_LOW_LEVEL_BODY_PROPORTION`).
const LOW_LEVEL_BODY_PROPORTION_STEP: f64 = 0.1;
const MAX_LOW_LEVEL_BODY_PROPORTION: f64 = 0.9;

/// Onshape's interactive detail settings (DetailManager's per-viewer
/// settings): what interactive frames draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetailSettings {
    pub edge_silhouettes: bool,
    pub section_edges: bool,
    pub ssao: bool,
    pub backface_silhouettes: bool,
    /// The share of bodies drawn at their lowest level of detail.
    pub low_level_body_proportion: f64,
}

impl Default for DetailSettings {
    fn default() -> Self {
        Self { edge_silhouettes: true, section_edges: true, ssao: true, backface_silhouettes: true, low_level_body_proportion: 0.0 }
    }
}

impl DetailSettings {
    /// How many [`DetailSettings::decrease`] steps from full detail (for the
    /// perf logs).
    pub fn steps(&self) -> u32 {
        let off = |on: bool| u32::from(!on);
        off(self.edge_silhouettes || self.section_edges) + off(self.ssao) + off(self.backface_silhouettes) + (self.low_level_body_proportion / LOW_LEVEL_BODY_PROPORTION_STEP).round() as u32
    }

    /// `decreaseDetail`: edge silhouettes and section edges, then ambient
    /// occlusion, then backface silhouettes, then more bodies at their
    /// lowest LOD. Whether anything changed.
    pub fn decrease(&mut self) -> bool {
        if self.edge_silhouettes || self.section_edges {
            self.edge_silhouettes = false;
            self.section_edges = false;
        } else if self.ssao {
            self.ssao = false;
        } else if self.backface_silhouettes {
            self.backface_silhouettes = false;
        } else if self.low_level_body_proportion < MAX_LOW_LEVEL_BODY_PROPORTION {
            self.low_level_body_proportion = (self.low_level_body_proportion + LOW_LEVEL_BODY_PROPORTION_STEP).min(MAX_LOW_LEVEL_BODY_PROPORTION);
        } else {
            return false;
        }
        true
    }

    /// `increaseDetail`, the other way round (`ssao` / `silhouettes`: the
    /// platform allows them interactively).
    pub fn increase(&mut self, ssao: bool, silhouettes: bool) -> bool {
        if self.low_level_body_proportion > 0.0 {
            self.low_level_body_proportion = (self.low_level_body_proportion - LOW_LEVEL_BODY_PROPORTION_STEP).max(0.0);
        } else if !self.backface_silhouettes && silhouettes {
            self.backface_silhouettes = true;
        } else if !self.ssao && ssao {
            self.ssao = true;
        } else if !self.edge_silhouettes && silhouettes {
            self.edge_silhouettes = true;
        } else if !self.section_edges {
            self.section_edges = true;
        } else {
            return false;
        }
        true
    }
}

/// Onshape's DetailManager for interactive frames
/// (`updateInteractiveSettings`, run for every interactive frame): slow
/// draws step detail down at once (and restart the draw timings); every
/// second frame-interval timing, slow frames step it down and fast ones
/// (over [`DETAIL_ENHANCEMENT_THRESHOLD_FPS`]) step it back up.
#[derive(Clone, Debug, Default)]
pub struct DetailManager {
    pub interactive: DetailSettings,
    last_acted: Option<u32>,
}

impl DetailManager {
    pub fn update(&mut self, timer: &mut FrameTimer) {
        let count = timer.frame.count;
        let Some(last) = self.last_acted else {
            self.last_acted = Some(count);
            return;
        };
        if FrameTimer::too_slow(timer.draw.average()) {
            self.interactive.decrease();
            timer.draw = Timings::default();
        } else if count.saturating_sub(last) >= 2 {
            let frames = timer.frame.average();
            if FrameTimer::too_slow(frames) {
                self.interactive.decrease();
                self.last_acted = Some(count);
            } else if frames.is_some_and(|a| a < 1000.0 / DETAIL_ENHANCEMENT_THRESHOLD_FPS) {
                self.interactive.increase(true, true);
                self.last_acted = Some(count);
            }
        }
    }
}

/// Visible-depth samples per viewport (`DEPTH_SAMPLES_PER_VIEWPORT`).
pub const DEPTH_SAMPLES_PER_VIEWPORT: u32 = 2000;
/// Fewer hits than this and the depth average is not used
/// (`MIN_SAMPLES_TO_USE_FOR_DEPTH_AVERAGED_ROTATION_CENTER`).
pub const MIN_DEPTH_SAMPLES: usize = 4;

/// Onshape's `getRotateCenterBasedOnWindowDepths`: the average of the
/// visible samples' frame position and depth, unprojected with the matrix
/// they were rendered with (`frame` = device pixels).
pub fn depth_average_pivot(samples: &[noshape_render::DepthSample], frame: (u32, u32), view_proj: glam::Mat4) -> Option<DVec3> {
    if samples.len() < MIN_DEPTH_SAMPLES || frame.0 == 0 || frame.1 == 0 {
        return None;
    }
    let n = samples.len() as f64;
    let (x, y, z) = samples.iter().fold((0.0, 0.0, 0.0), |a, s| (a.0 + s.x as f64 / n, a.1 + s.y as f64 / n, a.2 + s.depth as f64 / n));
    let ndc = glam::DVec4::new(x / frame.0 as f64 * 2.0 - 1.0, 1.0 - y / frame.1 as f64 * 2.0, z, 1.0);
    let w = view_proj.as_dmat4().inverse() * ndc;
    (w.w.abs() > 1e-300).then(|| w.truncate() / w.w)
}

// ------------------------------------------------------------ pan

/// Pan so the world point under the cursor follows it. Perspective views pan
/// in the plane through `pivot` (the geometry the drag started on).
pub fn pan(camera: &mut Camera, delta: DVec2, viewport: DVec2, pivot: Option<DVec3>) {
    let k = match (camera.projection, pivot) {
        (Projection::Perspective, Some(p)) => {
            let depth = (p - camera.eye()).dot(camera.forward());
            (depth / camera.distance.max(1e-12)).max(1e-6)
        }
        _ => 1.0,
    };
    camera.pan(delta * k, viewport.y);
}

// ------------------------------------------------------------ keyboard

/// Arrow-key / view cube rotation directions (`rotateInDirection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotateDir {
    Left,
    Right,
    Up,
    Down,
    Clockwise,
    CounterClockwise,
}

/// The world axis and signed angle (radians) of a rotation step: left/right
/// about the camera's up, up/down about its right, (counter)clockwise about
/// the line of sight. Same handedness as a drag toward that side.
pub fn rotate_step_axis(camera: &Camera, dir: RotateDir, degrees: f64) -> (DVec3, f64) {
    let t = degrees.to_radians();
    let (up, forward) = (camera.up(), camera.forward());
    let side = forward.cross(up);
    match dir {
        RotateDir::Left => (up, t),
        RotateDir::Right => (up, -t),
        RotateDir::Up => (side, t),
        RotateDir::Down => (side, -t),
        RotateDir::Clockwise => (forward, -t),
        RotateDir::CounterClockwise => (forward, t),
    }
}

/// The step for a signed (dx, dy) in degrees, as the view cube and the
/// arrow-key handlers express it (−x = left, −y = up).
pub fn step_from_degrees(dx: f64, dy: f64) -> (RotateDir, f64) {
    if dx != 0.0 {
        (if dx < 0.0 { RotateDir::Left } else { RotateDir::Right }, dx.abs())
    } else {
        (if dy < 0.0 { RotateDir::Up } else { RotateDir::Down }, dy.abs())
    }
}

/// One running keyboard rotation (`animateRotation`): linear over 250 ms,
/// applied incrementally so steps from a held key add up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepRotation {
    pub axis: DVec3,
    pub angle: f64,
    pub pivot: DVec3,
    pub elapsed: Duration,
    pub applied: f64,
}

impl StepRotation {
    pub fn new(axis: DVec3, angle: f64, pivot: DVec3) -> Self {
        Self { axis, angle, pivot, elapsed: Duration::ZERO, applied: 0.0 }
    }

    /// Advance by `dt` and apply the new part to `camera`; false once done.
    pub fn advance(&mut self, camera: &mut Camera, dt: Duration) -> bool {
        self.elapsed += dt;
        let dur = CAMERA_SHORT_ANIMATION.as_secs_f64();
        let a = self.elapsed.as_secs_f64().min(dur) / dur * self.angle;
        rotate_about(camera, self.axis, a - self.applied, self.pivot);
        self.applied = a;
        self.elapsed < CAMERA_SHORT_ANIMATION
    }
}

/// Camera after Ctrl+Shift+arrow (`animatePan`): the model moves
/// [`DEFAULT_PAN_AMOUNT_PX`] toward the arrow, measured at the scene centre.
pub fn pan_step(camera: &Camera, dx: f64, dy: f64, viewport: DVec2, scene_center: DVec3) -> Camera {
    let mut to = *camera;
    let k = pixel_size_at(camera, scene_center, viewport) / camera.world_per_pixel(viewport.y).max(1e-300);
    to.pan(DVec2::new(dx, dy) * DEFAULT_PAN_AMOUNT_PX * k, viewport.y);
    to
}

/// Camera after Z / Shift+Z (`zoomOut` / `zoomIn`): ×1.75 about the view centre.
pub fn zoom_step(camera: &Camera, zoom_in: bool, bounds: Option<(DVec3, DVec3)>, viewport: DVec2) -> Camera {
    let mut to = *camera;
    to.distance *= if zoom_in { 1.0 / DEFAULT_ZOOM_FACTOR } else { DEFAULT_ZOOM_FACTOR };
    clamp_zoom(&mut to, bounds, viewport);
    to
}

/// Onshape's camera animation easing (smootherstep).
pub fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (6.0 * t - 15.0) + 10.0)
}

// ------------------------------------------------------------ mouse controls

/// What a drag does (`ViewManipulationMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    None,
    Rotate,
    /// Constrained rotate (about world Z, view kept floor down).
    AxisRotate,
    Pan,
    Zoom,
}

/// Graphics area kind: Part Studios and Assemblies are 3D; drawings are 2D
/// (pan and zoom only, with their own bindings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    ThreeD,
    Drawing,
}

/// "View settings" presets of Preferences › Mouse controls, in Onshape's order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Preset {
    #[default]
    Onshape,
    Solidworks,
    Nx,
    Creo,
    Autocad,
}

impl Preset {
    pub const ALL: [Preset; 5] = [Preset::Onshape, Preset::Solidworks, Preset::Nx, Preset::Creo, Preset::Autocad];

    /// The dropdown label.
    pub fn label(self) -> &'static str {
        match self {
            Preset::Onshape => "Onshape (default)",
            Preset::Solidworks => "SOLIDWORKS",
            Preset::Nx => "NX 10",
            Preset::Creo => "Creo",
            Preset::Autocad => "AutoCAD",
        }
    }

    /// The table rows (Rotate, Constrained rotate, Pan, Zoom): 3D and 2D text.
    pub fn table(self) -> [(&'static str, &'static str, &'static str); 4] {
        const NA: &str = "Not applicable";
        match self {
            Preset::Onshape => [
                ("Rotate", "Right mouse button drag", NA),
                ("Constrained rotate", "Right mouse button drag", NA),
                ("Pan", "Ctrl + Right mouse button drag or \nMiddle mouse button drag", "Ctrl + Right mouse button drag or \nMiddle mouse button drag or \nRight mouse button drag or \nCtrl + Middle mouse button drag"),
                ("Zoom", "Scroll wheel in/out", "Scroll wheel in/out"),
            ],
            Preset::Solidworks => [
                ("Rotate", "Middle mouse button drag", NA),
                ("Constrained rotate", "Middle mouse button drag", NA),
                ("Pan", "Ctrl + Middle mouse button drag", "Middle mouse button drag or \nCtrl + Middle mouse button drag"),
                ("Zoom", "Scroll wheel in/out or \nShift + Middle mouse button drag", "Scroll wheel in/out or \nShift + Middle mouse button drag"),
            ],
            Preset::Nx => [
                ("Rotate", "Middle mouse button drag", NA),
                ("Constrained rotate", "Middle mouse button drag", NA),
                ("Pan", "Shift + Middle mouse button drag", "Shift + Middle mouse button drag"),
                ("Zoom", "Scroll wheel in/out or \nCtrl + Middle mouse button drag", "Scroll wheel in/out or \nCtrl + Middle mouse button drag"),
            ],
            Preset::Creo => [
                ("Rotate", "Middle mouse button drag", NA),
                ("Constrained rotate", "Middle mouse button drag", NA),
                ("Pan", "Shift + Middle mouse button drag", "Middle mouse button drag"),
                ("Zoom", "Scroll wheel in/out or \nCtrl + Middle mouse button drag", "Scroll wheel in/out or \nCtrl + Middle mouse button drag"),
            ],
            Preset::Autocad => [
                ("Rotate", "Shift + Middle mouse button drag", NA),
                ("Constrained rotate", "Shift + Middle mouse button drag", NA),
                ("Pan", "Middle mouse button drag", "Middle mouse button drag"),
                ("Zoom", "Scroll wheel in/out", "Scroll wheel in/out"),
            ],
        }
    }
}

/// Preferences › Mouse controls (Onshape's `mouseActions`, `viewMappingId`,
/// `axisRotationLock`, `usePenAsMouse`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MouseControls {
    pub preset: Preset,
    /// "Reverse scroll wheel zoom direction".
    pub reverse_scroll: bool,
    /// "Set default rotation behavior to constrained rotate" (Alt then gives
    /// free rotation).
    pub constrained_rotate: bool,
    /// "Use pen input as mouse".
    pub pen_as_mouse: bool,
}

static MOUSE_CONTROLS: RwLock<Option<MouseControls>> = RwLock::new(None);

fn mouse_controls_path() -> std::path::PathBuf {
    crate::files::config_dir().join("mouse-controls.json")
}

impl MouseControls {
    /// The saved settings (read once, then cached).
    pub fn current() -> Self {
        if let Some(c) = *MOUSE_CONTROLS.read().unwrap_or_else(|e| e.into_inner()) {
            return c;
        }
        let c: Self = std::fs::read_to_string(mouse_controls_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        *MOUSE_CONTROLS.write().unwrap_or_else(|e| e.into_inner()) = Some(c);
        c.apply_to_platform();
        c
    }

    /// "Use pen input as mouse": the Windows platform turns pen contacts into
    /// touches unless it is set (vendor/gpui-pre-windows).
    fn apply_to_platform(self) {
        #[cfg(target_os = "windows")]
        gpui_windows::set_pen_as_mouse(self.pen_as_mouse);
    }

    /// "Save mouse controls".
    pub fn save(self) {
        *MOUSE_CONTROLS.write().unwrap_or_else(|e| e.into_inner()) = Some(self);
        self.apply_to_platform();
        if let Ok(t) = serde_json::to_string_pretty(&self) {
            let _ = std::fs::write(mouse_controls_path(), t);
        }
    }
}

/// Pressed mouse buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons {
    pub left: bool,
    pub middle: bool,
    pub right: bool,
}

impl Buttons {
    pub fn any(self) -> bool {
        self.left || self.middle || self.right
    }

    pub fn set(&mut self, button: gpui_kit::MouseButton, down: bool) {
        match button {
            gpui_kit::MouseButton::Left => self.left = down,
            gpui_kit::MouseButton::Middle => self.middle = down,
            gpui_kit::MouseButton::Right => self.right = down,
            _ => {}
        }
    }
}

/// Modifier keys that take part in bindings (Ctrl is the Control key on
/// every platform, as in the browser).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl From<gpui_kit::Modifiers> for Keys {
    fn from(m: gpui_kit::Modifiers) -> Self {
        Keys { ctrl: m.control, shift: m.shift, alt: m.alt }
    }
}

// Onshape's mouse/key flags (`mouseKeyFlags`): the left button has none.
const MMB: u8 = 1;
const RMB: u8 = 2;
const SHIFT: u8 = 4;
const CTRL: u8 = 8;
const ALT: u8 = 16;

/// The bindings (`viewManipulationMouseKeyMapping`) for a preset and space;
/// constrained-rotate-by-default swaps Rotate and Constrained rotate.
fn bindings(c: MouseControls, space: Space) -> Vec<(u8, Mode)> {
    let (rot, axis) = if c.constrained_rotate { (Mode::AxisRotate, Mode::Rotate) } else { (Mode::Rotate, Mode::AxisRotate) };
    match (space, c.preset) {
        (Space::ThreeD, Preset::Onshape) => vec![(RMB, rot), (RMB | ALT, axis), (RMB | CTRL, Mode::Pan), (MMB, Mode::Pan)],
        (Space::ThreeD, Preset::Solidworks) => vec![(MMB, rot), (MMB | ALT, axis), (MMB | CTRL, Mode::Pan), (MMB | SHIFT, Mode::Zoom)],
        (Space::ThreeD, Preset::Nx | Preset::Creo) => vec![(MMB, rot), (MMB | ALT, axis), (MMB | SHIFT, Mode::Pan), (MMB | CTRL, Mode::Zoom)],
        (Space::ThreeD, Preset::Autocad) => vec![(MMB | SHIFT, rot), (MMB | SHIFT | ALT, axis), (MMB, Mode::Pan)],
        (Space::Drawing, Preset::Onshape) => vec![(RMB | CTRL, Mode::Pan), (MMB, Mode::Pan), (RMB, Mode::Pan), (MMB | CTRL, Mode::Pan)],
        (Space::Drawing, Preset::Solidworks) => vec![(MMB, Mode::Pan), (MMB | CTRL, Mode::Pan), (MMB | SHIFT, Mode::Zoom)],
        (Space::Drawing, Preset::Nx) => vec![(MMB | SHIFT, Mode::Pan), (MMB | CTRL, Mode::Zoom)],
        (Space::Drawing, Preset::Creo) => vec![(MMB, Mode::Pan), (MMB | CTRL, Mode::Zoom)],
        (Space::Drawing, Preset::Autocad) => vec![(MMB, Mode::Pan)],
    }
}

/// Resolves button + modifier state into a [`Mode`] like Onshape's
/// `updateManipulationMode`: an exact binding wins; otherwise the "closest"
/// one (a binding whose buttons and keys are all pressed, preferring a mode
/// other than the current one). Modifiers no binding uses are ignored, so in
/// the default preset Shift+right-drag still rotates.
#[derive(Clone, Debug)]
pub struct ModeResolver {
    bindings: Vec<(u8, Mode)>,
    uses: u8,
}

impl ModeResolver {
    pub fn new(controls: MouseControls, space: Space) -> Self {
        let bindings = bindings(controls, space);
        let uses = bindings.iter().fold(0, |a, (id, _)| a | (id & (SHIFT | CTRL | ALT)));
        Self { bindings, uses }
    }

    fn id(&self, buttons: Buttons, keys: Keys) -> u8 {
        let mut id = 0;
        if buttons.middle {
            id |= MMB;
        }
        if buttons.right {
            id |= RMB;
        }
        if keys.shift && self.uses & SHIFT != 0 {
            id |= SHIFT;
        }
        if keys.ctrl && self.uses & CTRL != 0 {
            id |= CTRL;
        }
        if keys.alt && self.uses & ALT != 0 {
            id |= ALT;
        }
        id
    }

    fn exact(&self, id: u8) -> Mode {
        self.bindings.iter().find(|(b, _)| *b == id).map(|(_, m)| *m).unwrap_or_default()
    }

    /// `getClosestMode`: among modes enabled by the pressed combination (one
    /// of their bindings is a subset of it) other than `exclude`, the one with
    /// the highest binding id.
    fn closest(&self, id: u8, exclude: Mode) -> Mode {
        let enabled = |mode: Mode| self.bindings.iter().any(|(b, m)| *m == mode && id & b == *b);
        let mut best = (0, Mode::None);
        for &(b, m) in &self.bindings {
            if m != exclude && enabled(m) && b > best.0 {
                best = (b, m);
            }
        }
        best.1
    }

    /// The mode for the input state, given the mode so far.
    pub fn resolve(&self, buttons: Buttons, keys: Keys, current: Mode) -> Mode {
        let id = self.id(buttons, keys);
        if id == 0 {
            return Mode::None;
        }
        match self.exact(id) {
            Mode::None => match self.closest(id, current) {
                Mode::None => self.closest(id, Mode::None),
                m => m,
            },
            m => m,
        }
    }

    /// Whether a modifier change can change the mode (`usesShiftKey` etc.).
    pub fn input_key(&self, keys: Keys) -> Keys {
        Keys { ctrl: keys.ctrl && self.uses & CTRL != 0, shift: keys.shift && self.uses & SHIFT != 0, alt: keys.alt && self.uses & ALT != 0 }
    }
}

/// Whether switching from `prev` to `next` picks a new rotate/pan centre
/// (`updateManipulationMode`: not when only toggling between free and
/// constrained rotation).
pub fn needs_new_pivot(prev: Mode, next: Mode) -> bool {
    matches!(next, Mode::Rotate | Mode::AxisRotate | Mode::Pan) && !matches!((prev, next), (Mode::AxisRotate, Mode::Rotate) | (Mode::Rotate, Mode::AxisRotate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{point, px};

    fn b(middle: bool, right: bool) -> Buttons {
        Buttons { left: false, middle, right }
    }
    fn k(ctrl: bool, shift: bool, alt: bool) -> Keys {
        Keys { ctrl, shift, alt }
    }
    fn onshape() -> ModeResolver {
        ModeResolver::new(MouseControls::default(), Space::ThreeD)
    }

    #[test]
    fn onshape_default_bindings() {
        let r = onshape();
        assert_eq!(r.resolve(b(false, true), k(false, false, false), Mode::None), Mode::Rotate);
        assert_eq!(r.resolve(b(false, true), k(false, false, true), Mode::None), Mode::AxisRotate);
        assert_eq!(r.resolve(b(false, true), k(true, false, false), Mode::None), Mode::Pan);
        assert_eq!(r.resolve(b(true, false), k(false, false, false), Mode::None), Mode::Pan);
        // Ctrl+middle: no exact binding, middle alone pans.
        assert_eq!(r.resolve(b(true, false), k(true, false, false), Mode::None), Mode::Pan);
        // Shift is not used by any binding: Shift+right still rotates.
        assert_eq!(r.resolve(b(false, true), k(false, true, false), Mode::None), Mode::Rotate);
        // The left button never navigates (it selects).
        assert_eq!(r.resolve(Buttons { left: true, ..Default::default() }, k(true, true, true), Mode::None), Mode::None);
        assert_eq!(r.resolve(Buttons::default(), k(false, false, false), Mode::Rotate), Mode::None);
    }

    #[test]
    fn modifier_change_mid_drag_switches_mode() {
        let r = onshape();
        // Right-drag rotating, then Ctrl pressed: pan; Ctrl released: rotate.
        let m = r.resolve(b(false, true), k(false, false, false), Mode::None);
        let m = r.resolve(b(false, true), k(true, false, false), m);
        assert_eq!(m, Mode::Pan);
        assert_eq!(r.resolve(b(false, true), k(false, false, false), m), Mode::Rotate);
        // Ctrl+Alt+right: no exact binding; the closest other than the
        // current mode with the highest id is Alt+right (18).
        assert_eq!(r.resolve(b(false, true), k(true, false, true), Mode::Pan), Mode::AxisRotate);
    }

    #[test]
    fn constrained_rotate_default_swaps_alt() {
        let c = MouseControls { constrained_rotate: true, ..Default::default() };
        let r = ModeResolver::new(c, Space::ThreeD);
        assert_eq!(r.resolve(b(false, true), k(false, false, false), Mode::None), Mode::AxisRotate);
        assert_eq!(r.resolve(b(false, true), k(false, false, true), Mode::None), Mode::Rotate);
    }

    #[test]
    fn presets_follow_their_tables() {
        let r = |p, s| ModeResolver::new(MouseControls { preset: p, ..Default::default() }, s);
        let sw = r(Preset::Solidworks, Space::ThreeD);
        assert_eq!(sw.resolve(b(true, false), k(false, false, false), Mode::None), Mode::Rotate);
        assert_eq!(sw.resolve(b(true, false), k(true, false, false), Mode::None), Mode::Pan);
        assert_eq!(sw.resolve(b(true, false), k(false, true, false), Mode::None), Mode::Zoom);
        assert_eq!(sw.resolve(b(false, true), k(false, false, false), Mode::None), Mode::None);
        let nx = r(Preset::Nx, Space::ThreeD);
        assert_eq!(nx.resolve(b(true, false), k(false, true, false), Mode::None), Mode::Pan);
        assert_eq!(nx.resolve(b(true, false), k(true, false, false), Mode::None), Mode::Zoom);
        let acad = r(Preset::Autocad, Space::ThreeD);
        assert_eq!(acad.resolve(b(true, false), k(false, false, false), Mode::None), Mode::Pan);
        assert_eq!(acad.resolve(b(true, false), k(false, true, false), Mode::None), Mode::Rotate);
        assert_eq!(acad.resolve(b(true, false), k(false, true, true), Mode::None), Mode::AxisRotate);
        // Drawings: never rotate; Onshape pans with right or middle, with or without Ctrl.
        let d = r(Preset::Onshape, Space::Drawing);
        for (m, rt, ctrl) in [(false, true, false), (false, true, true), (true, false, false), (true, false, true)] {
            assert_eq!(d.resolve(b(m, rt), k(ctrl, false, false), Mode::None), Mode::Pan);
        }
        let creo = r(Preset::Creo, Space::Drawing);
        assert_eq!(creo.resolve(b(true, false), k(false, false, false), Mode::None), Mode::Pan);
        assert_eq!(creo.resolve(b(true, false), k(true, false, false), Mode::None), Mode::Zoom);
    }

    #[test]
    fn wheel_notch_zoom_per_platform() {
        let notch = |lines: f32| ScrollDelta::Lines(point(0.0, lines));
        // macOS: one line = one notch = 0.5.
        assert_eq!(wheel_zoom_amount(notch(1.0), Platform::Mac, false, false), Some(0.5));
        // Linux: gpui reports 3 lines per notch.
        assert_eq!(wheel_zoom_amount(notch(3.0), Platform::Linux, false, false), Some(0.5));
        assert_eq!(wheel_zoom_amount(notch(-3.0), Platform::Linux, false, false), Some(-0.5));
        // Windows: ×3.
        assert!((wheel_zoom_amount(notch(wheel_scroll_lines()), Platform::Windows, false, false).unwrap() - 1.5).abs() < 1e-9);
        // A notch scales the view by 2^(-1/12) (≈ 5.6 %).
        assert!((zoom_scale(0.5) - 0.943_874).abs() < 1e-6);
        // Reverse preference.
        assert_eq!(wheel_zoom_amount(notch(1.0), Platform::Mac, true, false), Some(-0.5));
    }

    #[test]
    fn trackpad_scroll_zooms_and_horizontal_is_ignored() {
        let px2 = |x: f32, y: f32| ScrollDelta::Pixels(point(px(x), px(y)));
        // macOS trackpad: 40 px = one notch.
        assert!((wheel_zoom_amount(px2(0.0, 40.0), Platform::Mac, false, false).unwrap() - 0.5).abs() < 1e-9);
        // Momentum events are ordinary deltas: small ones zoom a little.
        let tiny = wheel_zoom_amount(px2(0.0, 0.5), Platform::Mac, false, false).unwrap();
        assert!(tiny > 0.0 && tiny < 0.01);
        // Mostly horizontal (Shift+wheel on every OS, sideways swipes): nothing.
        assert_eq!(wheel_zoom_amount(px2(10.0, 3.0), Platform::Mac, false, false), None);
        assert_eq!(wheel_zoom_amount(ScrollDelta::Lines(point(3.0, 0.0)), Platform::Linux, false, false), None);
        assert_eq!(wheel_zoom_amount(px2(0.0, 0.0), Platform::Windows, false, false), None);
        // Shift+wheel: zooms on Linux (gpui's horizontal swap undone), not
        // on Windows / macOS where the browser sees a horizontal scroll.
        let shifted = ScrollDelta::Lines(point(3.0, 0.0));
        assert_eq!(wheel_zoom_amount(shifted, Platform::Linux, false, true), Some(0.5));
        assert_eq!(wheel_zoom_amount(shifted, Platform::Windows, false, true), None);
        assert_eq!(wheel_zoom_amount(ScrollDelta::Lines(point(1.0, 0.0)), Platform::Mac, false, true), None);
        // Diagonal but mostly vertical still zooms.
        assert!(wheel_zoom_amount(px2(3.0, 10.0), Platform::Mac, false, false).is_some());
    }

    fn bounds() -> Option<(DVec3, DVec3)> {
        Some((DVec3::splat(-0.05), DVec3::splat(0.05)))
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let vp = DVec2::new(1600.0, 1000.0);
        let at = DVec2::new(1200.0, 300.0);
        for projection in [Projection::Orthographic, Projection::Perspective] {
            let mut c = Camera { projection, ..Camera::default() };
            let before = c.unproject_on_target_plane(at, vp);
            let (_, dir_before) = c.ray(at, vp);
            assert!(zoom_at(&mut c, Zoom::Amount(0.5), at, vp, bounds()));
            let (o, dir) = c.ray(at, vp);
            // The world point is still on the ray through the cursor.
            let to_point = (before - o).normalize();
            match projection {
                Projection::Orthographic => assert!((c.unproject_on_target_plane(at, vp) - before).length() < 1e-9),
                Projection::Perspective => assert!(to_point.dot(dir) > 1.0 - 1e-9 && dir.dot(dir_before) > 1.0 - 1e-9),
            }
        }
    }

    #[test]
    fn ortho_zoom_amounts_and_pinch_follow_fingers() {
        let vp = DVec2::new(1600.0, 1000.0);
        let mut c = Camera::default();
        let d = c.distance;
        zoom_at(&mut c, Zoom::Amount(0.5), vp * 0.5, vp, bounds());
        assert!((c.distance / d - zoom_scale(0.5)).abs() < 1e-12);
        // Pinch: the fingers spread by 10 % → the model is 10 % bigger.
        let d = c.distance;
        zoom_at(&mut c, Zoom::Pinch(pinch_ratio(0.1)), DVec2::new(300.0, 700.0), vp, bounds());
        assert!((d / c.distance - 1.1).abs() < 1e-6);
        // A full pinch in and out returns to the same zoom (no drift).
        let d0 = c.distance;
        for delta in [0.05f32, 0.05, -0.0952381, 0.2, -1.0 / 6.0] {
            zoom_at(&mut c, Zoom::Pinch(pinch_ratio(delta)), DVec2::new(300.0, 700.0), vp, bounds());
        }
        let fingers = 1.05 * 1.05 * (1.0 - 0.0952381) * 1.2 * (1.0 - 1.0 / 6.0);
        assert!((d0 / c.distance - fingers).abs() < 1e-6);
    }

    #[test]
    fn perspective_zoom_dollies_by_a_sixth_of_the_distance() {
        let vp = DVec2::new(1600.0, 1000.0);
        let mut c = Camera { projection: Projection::Perspective, ..Camera::default() };
        let d = c.distance;
        // At the view centre the ray is the line of sight: d' = d (1 - i/6).
        zoom_at(&mut c, Zoom::Amount(1.5), vp * 0.5, vp, bounds());
        assert!((c.distance - d * (1.0 - 0.25)).abs() < 1e-12);
        // The eye never gets closer than 0.05 scene diameters.
        for _ in 0..200 {
            zoom_at(&mut c, Zoom::Amount(1.5), vp * 0.5, vp, bounds());
        }
        let diameter = (DVec3::splat(0.1)).length();
        assert!(c.distance >= diameter * PERSPECTIVE_MIN_SCALE - 1e-12);
    }

    #[test]
    fn zoom_limits_like_can_zoom() {
        let vp = DVec2::new(1600.0, 1000.0);
        let mut c = Camera::default();
        // Zoom out until the model is under 100 px: further zoom out refused.
        let mut n = 0;
        while zoom_at(&mut c, Zoom::Amount(-1.5), vp * 0.5, vp, bounds()) {
            n += 1;
            assert!(n < 1000);
        }
        let diameter = DVec3::splat(0.1).length();
        let px_size = pixel_size_at(&c, DVec3::ZERO, vp);
        assert!(diameter / px_size <= zoom_scale(-1.5) * MIN_MODEL_PIXEL_SIZE + 1e-6);
        // Zooming back in still works.
        assert!(zoom_at(&mut c, Zoom::Amount(1.5), vp * 0.5, vp, bounds()));
        // No bounds (empty scene): Onshape does not zoom.
        assert!(!zoom_at(&mut c, Zoom::Amount(1.5), vp * 0.5, vp, None));
    }

    #[test]
    fn drag_rotate_rate_matches_live() {
        // Onshape (webgl `onDrag`): 2π · ROTATION_CYCLES_PER_SCREEN (3) /
        // the canvas diagonal per pixel, drag and canvas both in device
        // pixels (so the ratio cancels). Live at 1919×1079 (canvas
        // 1673×973): a 100 px drag turns 55.80° (qa5 KB4: ≈0.55°/px). The
        // graphics area is the same size in a window of the same size
        // (1440×900: 1194×794 in both), where 100 px turn 75.3°.
        for (vp, want) in [(DVec2::new(1673.0, 973.0), 55.80), (DVec2::new(1194.0, 794.0), 75.32)] {
            let mut c = Camera::default();
            let start = c.rotation;
            let pivot = c.target;
            drag_rotate(&mut c, DVec2::new(100.0, 0.0), vp, pivot, false);
            let deg = c.rotation.angle_between(start).to_degrees();
            assert!((deg - want).abs() < 0.05, "{vp:?}: 100 px turned {deg:.2}°, Onshape {want}°");
        }
    }

    #[test]
    fn drag_rotate_speed_is_three_turns_per_diagonal() {
        let vp = DVec2::new(1600.0, 1000.0);
        let mut c = Camera::default();
        let start = c.rotation;
        // A third of the diagonal horizontally = one full turn about up.
        let d = vp.length() / 3.0;
        let steps = 100;
        for _ in 0..steps {
            let pivot = c.target;
            drag_rotate(&mut c, DVec2::new(d / steps as f64, 0.0), vp, pivot, false);
        }
        assert!(c.rotation.dot(start).abs() > 1.0 - 1e-9, "{:?} vs {start:?}", c.rotation);
        // Dragging right turns the model right: the camera moves to its left
        // (the eye goes toward -right).
        let mut c = Camera::default();
        let eye = c.eye();
        let right = c.right();
        let pivot = c.target;
        drag_rotate(&mut c, DVec2::new(20.0, 0.0), vp, pivot, false);
        assert!((c.eye() - eye).dot(right) < 0.0);
        // Dragging down tilts the top toward the viewer: the eye rises.
        let mut c = Camera::default();
        let (eye, up) = (c.eye(), c.up());
        let pivot = c.target;
        drag_rotate(&mut c, DVec2::new(0.0, 20.0), vp, pivot, false);
        assert!((c.eye() - eye).dot(up) > 0.0);
    }

    #[test]
    fn rotation_keeps_the_pivot_fixed_on_screen() {
        let vp = DVec2::new(1600.0, 1000.0);
        let mut c = Camera::default();
        let pivot = DVec3::new(0.03, -0.02, 0.01);
        let before = c.project(pivot, vp, 1.0).unwrap();
        drag_rotate(&mut c, DVec2::new(37.0, -12.0), vp, pivot, false);
        let after = c.project(pivot, vp, 1.0).unwrap();
        assert!((before - after).length() < 1e-6);
    }

    #[test]
    fn frame_timer_follows_onshape() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        // 60 fps, 5 ms draws: not below 20 fps.
        let mut fast = FrameTimer::default();
        for i in 0..40 {
            fast.draw_start(ms(i * 16));
            fast.draw_end(Duration::from_millis(5));
        }
        assert!(!fast.below_threshold());
        // Draws over 50 ms on average (the last five): below.
        let mut slow_draw = FrameTimer::default();
        for d in [10, 60, 60, 60, 60, 60] {
            slow_draw.draw_end(Duration::from_millis(d));
        }
        assert!(slow_draw.below_threshold());
        // Quick draws but frames 70 ms apart: the interval timing (every
        // tenth frame, the slower of the next two intervals) is below.
        let mut slow_frames = FrameTimer::default();
        let mut forced = 0;
        for i in 0..12 {
            forced += slow_frames.draw_start(ms(i * 70)) as u32;
            slow_frames.draw_end(Duration::from_millis(5));
        }
        assert!(slow_frames.below_threshold());
        // Frame 10 starts the timing, frame 11 is the forced second draw.
        assert_eq!(forced, 2);
        // No timing before the tenth frame.
        let mut early = FrameTimer::default();
        for i in 0..9 {
            early.draw_start(ms(i * 70));
        }
        assert!(!early.below_threshold());
    }

    #[test]
    fn detail_step_count_follows_decrease() {
        let mut d = DetailSettings::default();
        assert_eq!(d.steps(), 0);
        let mut last = 0;
        while d.decrease() {
            let n = d.steps();
            assert!(n == last || n == last + 1, "{last} -> {n}");
            last = n;
        }
        // Silhouettes/section edges, AO, backface silhouettes, 9 body steps
        // (the share reaches 0.9 by 0.8999999999999999, one more decrease).
        assert_eq!(last, 12);
    }

    #[test]
    fn detail_steps_down_and_back_like_onshape() {
        // decreaseDetail's order, then the floor.
        let mut d = DetailSettings::default();
        assert!(d.decrease() && !d.edge_silhouettes && !d.section_edges && d.ssao);
        assert!(d.decrease() && !d.ssao && d.backface_silhouettes);
        assert!(d.decrease() && !d.backface_silhouettes && d.low_level_body_proportion == 0.0);
        for _ in 0..20 {
            d.decrease();
        }
        assert!((d.low_level_body_proportion - 0.9).abs() < 1e-9 && !d.decrease());
        // increaseDetail: LOD first, then backface silhouettes, SSAO, edge
        // silhouettes, section edges.
        // (0.9 less nine 0.1 steps is ~1e-17 in doubles, as in Onshape's
        // JavaScript: one more step to 0.)
        let mut steps = 0;
        while d.low_level_body_proportion > 0.0 {
            assert!(d.increase(true, true));
            steps += 1;
        }
        assert!((9..=10).contains(&steps) && !d.backface_silhouettes);
        assert!(d.increase(true, true) && d.backface_silhouettes && !d.ssao);
        assert!(d.increase(true, true) && d.ssao && !d.edge_silhouettes);
        assert!(d.increase(true, true) && d.edge_silhouettes && !d.section_edges);
        assert!(d.increase(true, true) && d == DetailSettings::default() && !d.increase(true, true));

        // The manager: slow draws step down at once; frame intervals act
        // every second timing (a timing every tenth frame).
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut m = DetailManager::default();
        let mut timer = FrameTimer::default();
        m.update(&mut timer);
        timer.draw_end(Duration::from_millis(80));
        m.update(&mut timer);
        assert!(!m.interactive.edge_silhouettes && m.interactive.ssao, "one step down on a slow draw");
        // 70 ms frames with quick draws: two interval timings, one more step.
        let mut at = 0;
        for _ in 0..24 {
            timer.draw_start(ms(at));
            timer.draw_end(Duration::from_millis(5));
            m.update(&mut timer);
            at += 70;
        }
        assert!(!m.interactive.ssao, "slow frames step down again");
        // Fast frames bring detail back.
        for _ in 0..200 {
            timer.draw_start(ms(at));
            timer.draw_end(Duration::from_millis(2));
            m.update(&mut timer);
            at += 10;
        }
        assert_eq!(m.interactive, DetailSettings::default());
    }

    #[test]
    fn drag_pan_keeps_the_grabbed_point_under_the_cursor() {
        // Onshape `pan`: the rays through the old and new pointer positions
        // meet the view-normal plane through the pan center; the camera
        // moves by their difference, so that point follows the pointer
        // exactly, in both projections and off the view axis.
        let vp = DVec2::new(1194.0, 794.0);
        for projection in [Projection::Orthographic, Projection::Perspective] {
            let mut c = Camera { projection, ..Camera::default() };
            let pivot = DVec3::new(0.03, -0.02, 0.01);
            let before = c.project(pivot, vp, 1.0).unwrap();
            let delta = DVec2::new(37.0, -12.0);
            pan(&mut c, delta, vp, Some(pivot));
            let after = c.project(pivot, vp, 1.0).unwrap();
            assert!((after - before - delta).length() < 1e-6, "{projection:?}: moved {:?}, pointer {delta:?}", after - before);
        }
    }

    #[test]
    fn keyboard_steps_match_a_drag_toward_the_arrow() {
        let vp = DVec2::new(1600.0, 1000.0);
        for (dir, drag) in [(RotateDir::Left, DVec2::new(-1.0, 0.0)), (RotateDir::Right, DVec2::new(1.0, 0.0)), (RotateDir::Up, DVec2::new(0.0, -1.0)), (RotateDir::Down, DVec2::new(0.0, 1.0))] {
            let mut by_key = Camera::default();
            let (axis, angle) = rotate_step_axis(&by_key, dir, 1.0);
            let pivot = by_key.target;
            rotate_about(&mut by_key, axis, angle, pivot);
            let mut by_drag = Camera::default();
            let r = rotation_per_px(vp, false);
            let pivot = by_drag.target;
            drag_rotate(&mut by_drag, drag * 1f64.to_radians() / r, vp, pivot, false);
            assert!(by_key.rotation.dot(by_drag.rotation).abs() > 1.0 - 1e-9, "{dir:?}");
        }
    }

    #[test]
    fn held_arrow_steps_add_up() {
        // Key repeat every 33 ms: each 15° step runs its full 250 ms alongside
        // the others, so after the last one finishes the view turned 15° × n.
        let mut c = Camera::default();
        let start = c.rotation;
        let mut running: Vec<StepRotation> = Vec::new();
        let frame = Duration::from_millis(16);
        let mut t = Duration::ZERO;
        let mut next_key = Duration::ZERO;
        let mut presses = 0;
        while presses < 6 || !running.is_empty() {
            if presses < 6 && t >= next_key {
                let (axis, angle) = rotate_step_axis(&c, RotateDir::Left, ROTATE_ANGLE_DEG);
                running.push(StepRotation::new(axis, angle, c.target));
                presses += 1;
                next_key += Duration::from_millis(33);
            }
            running.retain_mut(|s| s.advance(&mut c, frame));
            t += frame;
        }
        let turned = (c.rotation * start.inverse()).to_axis_angle().1;
        assert!((turned.to_degrees() - 90.0).abs() < 1e-6, "{}", turned.to_degrees());
    }

    #[test]
    fn inertia_decays_like_onshape() {
        let mut s = RotationSpeed::default();
        for _ in 0..10 {
            s.push(DVec2::new(20.0, 0.0));
        }
        let mut i = Inertia { speed: s.0, factor: 1.0, pivot: DVec3::ZERO };
        let first = i.step(Duration::from_secs_f64(1.0 / 60.0)).unwrap();
        assert!((first.x - s.0.x * 0.77).abs() < 1e-6);
        let mut frames = 1;
        while i.step(Duration::from_secs_f64(1.0 / 60.0)).is_some() {
            frames += 1;
        }
        // 20 px/step × 0.77^n ≤ 1 → 12 frames.
        assert_eq!(frames, 11);
    }

    #[test]
    fn constrained_rotate_levels_then_turns_about_z() {
        let vp = DVec2::new(1600.0, 1000.0);
        let mut c = Camera::default();
        // Roll the view 30° about the line of sight.
        let f = c.forward();
        let pivot = c.target;
        rotate_about(&mut c, f, 30f64.to_radians(), pivot);
        let (to, anim) = level_view(&c, c.target);
        assert_eq!(anim, Some(CAMERA_SNAP_ANIMATION));
        assert!(to.right().z.abs() < 1e-9);
        let mut c = to;
        let pitch_before = c.forward().z;
        let pivot = c.target;
        drag_axis_rotate(&mut c, DVec2::new(50.0, 0.0), vp, pivot);
        // Horizontal drags keep it level and keep the pitch.
        assert!(c.right().z.abs() < 1e-9);
        assert!((c.forward().z - pitch_before).abs() < 1e-9);
        assert_eq!(level_view(&c, c.target).1, None);
    }

    #[test]
    fn keyboard_pan_and_zoom_steps() {
        let vp = DVec2::new(1600.0, 1000.0);
        let c = Camera::default();
        // Ctrl+Shift+Left: the model moves 150 px left.
        let to = pan_step(&c, -1.0, 0.0, vp, DVec3::ZERO);
        let p0 = c.project(DVec3::ZERO, vp, 1.0).unwrap();
        let p1 = to.project(DVec3::ZERO, vp, 1.0).unwrap();
        assert!((p1 - p0 - DVec2::new(-150.0, 0.0)).length() < 1e-6);
        // Ctrl+Shift+Up: up.
        let to = pan_step(&c, 0.0, -1.0, vp, DVec3::ZERO);
        let p1 = to.project(DVec3::ZERO, vp, 1.0).unwrap();
        assert!((p1 - p0 - DVec2::new(0.0, -150.0)).length() < 1e-6);
        // Shift+Z zooms in 1.75×, Z out.
        assert!((zoom_step(&c, true, bounds(), vp).distance - c.distance / 1.75).abs() < 1e-12);
        assert!((zoom_step(&c, false, bounds(), vp).distance - c.distance * 1.75).abs() < 1e-12);
        // Easing ends at the ends.
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert!((ease(0.5) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn depth_average_unprojects_the_mean_sample() {
        use noshape_render::DepthSample;
        let vp = DVec2::new(400.0, 300.0);
        let cam = Camera::default();
        let view_proj = cam.view_proj_f32(vp.x / vp.y, 1.0);
        // Samples of two world points: the pivot is their midpoint (ortho is affine).
        let sample = |p: DVec3| {
            let c = view_proj.as_dmat4() * p.extend(1.0);
            let n = c.truncate() / c.w;
            DepthSample { x: ((n.x + 1.0) * 0.5 * vp.x) as f32, y: ((1.0 - n.y) * 0.5 * vp.y) as f32, depth: n.z as f32, id: 1 }
        };
        let (a, b) = (DVec3::new(0.01, 0.0, 0.0), DVec3::new(-0.01, 0.02, 0.005));
        let s = [sample(a), sample(a), sample(b), sample(b)];
        let p = depth_average_pivot(&s, (400, 300), view_proj).unwrap();
        assert!((p - (a + b) * 0.5).length() < 1e-5, "{p:?}");
        // Under four hits: not used.
        assert!(depth_average_pivot(&s[..3], (400, 300), view_proj).is_none());
    }

    #[test]
    fn mouse_controls_round_trip() {
        let c = MouseControls { preset: Preset::Creo, reverse_scroll: true, constrained_rotate: true, pen_as_mouse: false };
        let t = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<MouseControls>(&t).unwrap(), c);
        assert_eq!(serde_json::from_str::<MouseControls>("{}").unwrap(), MouseControls::default());
        assert_eq!(Preset::ALL.map(Preset::label), ["Onshape (default)", "SOLIDWORKS", "NX 10", "Creo", "AutoCAD"]);
    }
}
