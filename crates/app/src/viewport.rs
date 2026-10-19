//! 3D viewport element: shows frames from the shared render worker and turns
//! mouse input into camera navigation and pick events.
//!
//! Navigation follows Onshape exactly (see `navigation.rs` for the mapping
//! and docs/parity/navigation.md): with the default mouse controls,
//! right-drag rotates about the geometry under the cursor (Alt: constrained
//! rotate), Ctrl+right-drag or middle-drag pans, the wheel / two-finger
//! scroll / pinch zoom about the cursor, double middle click zooms to fit;
//! right-click opens the context menu; left-click selects and left-drag on
//! empty space box-selects (left→right = window, right→left = crossing).

use crate::gpu::Gpu;
use crate::navigation::{self as nav, Buttons, Inertia, Keys, Mode, ModeResolver, MouseControls, RotateDir, RotationSpeed, StepRotation, Zoom};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use glam::{DVec2, DVec3};
use noshape_render::{Camera, Frame, PickBuffer, PickRect, Projection, Scene, StandardView, Style, worker::RenderJob};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Point under the cursor for a ray (origin, direction), if any.
pub type OrbitPivotFn = Arc<dyn Fn(DVec3, DVec3) -> Option<DVec3> + Send + Sync>;

#[derive(Clone, Debug)]
pub enum ViewportEvent {
    Hover(Option<u32>),
    /// Left click (no drag). `pick` is the id under the cursor.
    Click { pick: Option<u32>, position: DVec2, modifiers: Modifiers },
    /// Right click without drag.
    ContextMenu { pick: Option<u32>, position: Point<Pixels> },
    /// Box selection finished; ids are unique.
    BoxSelect { ids: Vec<u32>, modifiers: Modifiers },
    /// Left-button press/drag/release while a tool captured the pointer.
    ToolDown { position: DVec2, modifiers: Modifiers, click_count: usize },
    ToolDrag { position: DVec2 },
    ToolUp { position: DVec2, modifiers: Modifiers },
    /// Pointer moved (viewport-local logical pixels), for tool previews.
    PointerMove { position: DVec2, modifiers: Modifiers },
    CameraChanged,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Drag {
    None,
    /// Button pressed, not yet moved beyond the click threshold.
    Pending(MouseButton),
    /// Rotating / panning / zooming (`Viewport::nav_mode`).
    Nav,
    Box,
    Tool,
}

/// What a pending pick query is for.
#[derive(Clone, Debug)]
enum PickPurpose {
    Hover,
    Click { position: DVec2, modifiers: Modifiers },
    ContextMenu { position: DVec2, window_position: Point<Pixels> },
    Box { a: DVec2, b: DVec2, crossing: bool, modifiers: Modifiers },
    /// Visible-depth samples for the rotate centre (nothing under the cursor).
    DepthPivot,
}

/// Half size (device pixels) of the pick rectangle read around the cursor:
/// small cursor moves are answered from the cached rectangle.
const PICK_HALF: i32 = 40;
/// Pick radius in logical pixels.
const PICK_RADIUS: f64 = 5.0;

struct CameraAnimation {
    from: Camera,
    to: Camera,
    start: Instant,
    duration: Duration,
}

pub struct Viewport {
    gpu: Gpu,
    target_id: u64,
    pub camera: Camera,
    scene: Arc<Scene>,
    pub style: Style,
    image: Option<Arc<RenderImage>>,
    /// Newest pick-buffer rectangle read back (around the cursor or a box),
    /// valid while `pick_epoch` is unchanged.
    pick_cache: Option<Arc<PickBuffer>>,
    /// Incremented whenever a frame that invalidates the pick buffer is submitted.
    pick_epoch: u64,
    next_pick_token: u64,
    /// Outstanding pick queries: token -> (epoch, purpose).
    pick_requests: HashMap<u64, (u64, PickPurpose, Instant)>,
    /// A hover query is outstanding / another one is wanted when it returns.
    hover_query: bool,
    hover_query_again: bool,
    /// The pointer is over the viewport (hover is cleared when it leaves).
    pointer_inside: bool,
    /// Submitted but not yet presented frames (seq, submit time), oldest first.
    in_flight: VecDeque<(u64, Instant)>,
    /// A deferred submit is scheduled for the end of the current update.
    submit_scheduled: bool,
    /// A timer re-checks a dirty frame that could not be submitted because
    /// the pipeline was full.
    retry_scheduled: bool,
    /// Camera and device size of the last submitted frame (a change
    /// invalidates the pick buffer even if the caller did not say so).
    last_submitted_view: Option<(Camera, u32, u32)>,
    /// When the viewport was last laid out in a window frame: frames are only
    /// submitted ahead of GPUI's render pass while it is on screen.
    last_layout: Option<Instant>,
    /// Last time the pick buffer was invalidated, and whether a settle
    /// timer for the hover refresh is running.
    pick_changed_at: Instant,
    hover_refresh_pending: bool,
    /// Viewport bounds in window coordinates (logical pixels).
    bounds: Bounds<Pixels>,
    scale: f32,
    seq: u64,
    last_presented: u64,
    dirty: bool,
    pick_dirty: bool,
    drag: Drag,
    press_pos: DVec2,
    last_pos: DVec2,
    /// Selected entities, drawn as a digit at the pointer (Onshape's
    /// selection-count cursor: 1-5, "5" for five or more). Set by the owner.
    pub selection_count: usize,
    /// The count cursor last set through the platform (`None` = the arrow),
    /// and whether the platform could (else the digit is drawn).
    cursor_applied: Option<Option<usize>>,
    cursor_platform: bool,
    hover: Option<u32>,
    /// When true, left-drag goes to the active tool instead of box select.
    pub tool_captures_left: bool,
    /// Zoom to window (W): the next left-drag box zooms instead of selecting.
    pub zoom_window: bool,
    /// Point under the cursor for a ray (origin, direction): right-drag
    /// rotates about it (Onshape rotates about the geometry under the cursor).
    pub orbit_pivot_fn: Option<OrbitPivotFn>,
    orbit_pivot: Option<DVec3>,
    /// Pressed buttons, the navigation drag's mode and its resolver (the
    /// mouse controls preference), and the modifiers it last saw.
    buttons: Buttons,
    nav_mode: Mode,
    nav_resolver: Option<ModeResolver>,
    nav_keys: Keys,
    /// Where a drag zoom zooms (the press point).
    zoom_anchor: DVec2,
    /// Depth-averaged rotate centre for a pick epoch (`None` inside: too few
    /// visible samples), the epoch a query is out for, and whether the
    /// running drag's centre is a stand-in waiting for that query.
    depth_pivot: Option<(u64, Option<DVec3>)>,
    depth_pivot_asked: Option<u64>,
    pivot_awaits_depth: bool,
    /// macOS: the pending right press is a Ctrl+left click converted by the
    /// platform (Chrome reports it as a left click with Ctrl, which never
    /// navigates: clicking still opens the context menu, dragging does nothing).
    ctrl_click_press: bool,
    /// Rotation speed, last rotate step time, and the post-release inertia
    /// with the time of its last step.
    rotation_speed: RotationSpeed,
    last_rotation: Option<Instant>,
    inertia: Option<(Inertia, Instant)>,
    /// Running keyboard / view cube rotation steps (they add up while a key
    /// is held) and the time they were last advanced.
    step_rotations: Vec<StepRotation>,
    step_clock: Option<Instant>,
    /// World-space manipulator handles (segments): pressing the left button
    /// near one routes it to the tool events even when `tool_captures_left`
    /// is false.
    /// Manipulator hit areas: world segments, or (with a length in pixels)
    /// a screen-sized glyph from the first point towards the second.
    pub manipulators: Vec<(DVec3, DVec3, f64)>,
    /// Pick ids that win ties in the pick radius (edges/points over faces).
    pub pick_priority: Arc<dyn Fn(u32) -> i32 + Send + Sync>,
    /// What zoom to fit frames (set by the owner, e.g. the visible parts);
    /// `None` frames the whole scene (default planes included).
    pub fit_bounds: Option<(DVec3, DVec3)>,
    /// Logical pixels at the left of the viewport covered by an overlay (the
    /// feature dialog): zoom to fit centers the model in the rest.
    pub fit_inset_left: f64,
    box_rect: Option<(DVec2, DVec2)>,
    anim: Option<CameraAnimation>,
    _frames: Task<()>,
    _picks: Task<()>,
    /// Frames presented so far.
    frames_presented: u64,
    /// `NOSHAPE_PERF`: when the last frame was presented (paint latency).
    presented_at: Option<Instant>,
    /// `NOSHAPE_PERF_ORBIT`: closed-loop orbit benchmark state.
    bench: Option<OrbitBench>,
}

/// Auto-orbit benchmark (`NOSHAPE_PERF=1 NOSHAPE_PERF_ORBIT=<frames>`,
/// optional `NOSHAPE_PERF_ORBIT_DELAY=<seconds>`, default 10): after the
/// delay, the camera is rotated every 8 ms (like a 125 Hz mouse during a
/// right-drag) until `<frames>` frames were presented; then fps and the
/// stage timings are logged.
struct OrbitBench {
    frames: u64,
    started: Option<Instant>,
    frames_at_start: u64,
    last_present: Option<Instant>,
    /// `NOSHAPE_PERF_BENCH=zoom`: scroll-zoom instead of orbiting.
    zoom: bool,
    ticks: u64,
    /// GPUI renders of the viewport (window re-renders) during the run.
    renders: u64,
}

static BENCH_CLAIMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn orbit_bench_from_env() -> Option<OrbitBench> {
    let frames: u64 = std::env::var("NOSHAPE_PERF_ORBIT").ok()?.parse().ok()?;
    if BENCH_CLAIMED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    Some(OrbitBench { frames, started: None, frames_at_start: 0, last_present: None, zoom: false, ticks: 0, renders: 0 })
}

impl EventEmitter<ViewportEvent> for Viewport {}

impl Viewport {
    /// Top-left of the viewport in window coordinates.
    pub fn origin(&self) -> Point<Pixels> {
        self.bounds.origin
    }

    pub fn new(gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (target_id, rx, pick_rx) = gpu.register_target_with_picks();
        let frames = cx.spawn_in(window, async move |this, cx| {
            while let Ok((seq, result, _)) = rx.recv().await {
                let ok = this.update_in(cx, |this: &mut Viewport, window, cx| {
                    this.present(seq, result, window, cx);
                });
                if ok.is_err() {
                    break;
                }
            }
        });
        let picks = cx.spawn(async move |this, cx| {
            while let Ok((token, result)) = pick_rx.recv().await {
                if this.update(cx, |this: &mut Viewport, cx| this.on_pick(token, result, cx)).is_err() {
                    break;
                }
            }
        });
        let bench = orbit_bench_from_env();
        if bench.is_some() {
            let delay = std::env::var("NOSHAPE_PERF_ORBIT_DELAY").ok().and_then(|s| s.parse::<f64>().ok()).unwrap_or(10.0);
            cx.spawn(async move |this, cx| {
                let wait = (delay - noshape_render::perf::process_start().elapsed().as_secs_f64()).max(0.0);
                cx.background_executor().timer(Duration::from_secs_f64(wait)).await;
                loop {
                    cx.background_executor().timer(Duration::from_millis(8)).await;
                    match this.update(cx, |this: &mut Viewport, cx| this.bench_tick(cx)) {
                        Ok(true) => {}
                        _ => break,
                    }
                }
            })
            .detach();
        }
        Self {
            gpu,
            target_id,
            camera: Camera::default(),
            scene: Arc::new(Scene::default()),
            style: Style::default(),
            image: None,
            pick_cache: None,
            pick_epoch: 0,
            next_pick_token: 1,
            pick_requests: HashMap::new(),
            hover_query: false,
            hover_query_again: false,
            pointer_inside: false,
            in_flight: VecDeque::new(),
            submit_scheduled: false,
            retry_scheduled: false,
            last_submitted_view: None,
            last_layout: None,
            pick_changed_at: Instant::now(),
            hover_refresh_pending: false,
            bounds: Bounds::default(),
            scale: 1.0,
            seq: 0,
            last_presented: 0,
            dirty: true,
            pick_dirty: true,
            drag: Drag::None,
            press_pos: DVec2::ZERO,
            last_pos: DVec2::ZERO,
            selection_count: 0,
            cursor_applied: None,
            cursor_platform: true,
            hover: None,
            tool_captures_left: false,
            zoom_window: false,
            orbit_pivot_fn: None,
            orbit_pivot: None,
            buttons: Buttons::default(),
            nav_mode: Mode::None,
            nav_resolver: None,
            nav_keys: Keys::default(),
            zoom_anchor: DVec2::ZERO,
            depth_pivot: None,
            depth_pivot_asked: None,
            pivot_awaits_depth: false,
            ctrl_click_press: false,
            rotation_speed: RotationSpeed::default(),
            last_rotation: None,
            inertia: None,
            step_rotations: Vec::new(),
            step_clock: None,
            manipulators: Vec::new(),
            pick_priority: Arc::new(|_| 0),
            fit_bounds: None,
            fit_inset_left: 0.0,
            box_rect: None,
            anim: None,
            _frames: frames,
            _picks: picks,
            frames_presented: 0,
            presented_at: None,
            bench,
        }
    }

    /// Replace the scene (geometry or highlight state changed).
    pub fn set_scene(&mut self, scene: Arc<Scene>, geometry_changed: bool, cx: &mut Context<Self>) {
        if !geometry_changed && (Arc::ptr_eq(&scene, &self.scene) || scene.same_as(&self.scene)) {
            // Nothing visible changed (e.g. a refresh with the same highlights).
            return;
        }
        self.scene = scene;
        if geometry_changed {
            self.pick_dirty = true;
        }
        self.request_frame(cx);
    }

    /// Size of the viewport in logical pixels.
    pub fn size(&self) -> DVec2 {
        DVec2::new(f64::from(f32::from(self.bounds.size.width)), f64::from(f32::from(self.bounds.size.height)))
    }

    /// World-space ray through a viewport-local position.
    pub fn ray(&self, local: DVec2) -> (DVec3, DVec3) {
        self.camera.ray(local, self.size())
    }

    /// Whether a viewport-local position is on a manipulator handle.
    pub fn manipulator_at(&self, p: DVec2) -> bool {
        self.manipulators.iter().any(|&(a, b, glyph_px)| {
            let (Some(a), Some(mut b)) = (self.project(a), self.project(b)) else { return false };
            // A screen-sized glyph: `glyph_px` long from `a` towards `b`.
            if glyph_px > 0.0 {
                b = a + (b - a).normalize_or_zero() * glyph_px;
            }
            let d = b - a;
            let u = ((p - a).dot(d) / d.length_squared().max(1e-9)).clamp(0.0, 1.0);
            (a + d * u - p).length() < 8.0 || (b - p).length() < 14.0
        })
    }

    /// Project a world point to viewport-local logical pixels.
    pub fn project(&self, p: DVec3) -> Option<DVec2> {
        self.camera.project(p, self.size(), self.scene_radius())
    }

    fn scene_radius(&self) -> f64 {
        self.scene
            .bounds()
            .map(|(lo, hi)| (hi - lo).length() as f64 * 0.5 + ((lo + hi) * 0.5).as_dvec3().distance(self.camera.target))
            .unwrap_or(1.0)
    }

    pub fn request_redraw(&mut self, cx: &mut Context<Self>) {
        self.request_frame(cx);
    }

    /// Mark the frame dirty and submit it at the end of the current update
    /// (coalescing several changes) if the pipeline has room; otherwise it is
    /// submitted when the next frame is presented.
    ///
    /// While the viewport is on screen this does not invalidate the GPUI
    /// window: nothing visible changes until the new frame is presented
    /// (which notifies), so camera moves do not re-render the whole UI.
    ///
    /// Always deferred: GPUI ignores `notify` during a window draw (the view
    /// is marked dirty but no redraw is scheduled), so notifying here could
    /// leave a stale frame on screen until the mouse moves.
    fn request_frame(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        if !self.submit_scheduled {
            self.submit_scheduled = true;
            let this = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = this.update(cx, |this, cx| {
                    this.submit_scheduled = false;
                    let on_screen = this.last_layout.is_some_and(|t| t.elapsed() < Duration::from_millis(500));
                    if on_screen {
                        this.submit_if_ready(cx);
                    } else if this.dirty {
                        // Not drawn lately (idle window or hidden tab): `render`
                        // submits once the viewport is laid out again.
                        cx.notify();
                    }
                });
            });
        }
    }

    fn submit_if_ready(&mut self, cx: &mut Context<Self>) {
        if !self.dirty {
            return;
        }
        // A frame that never comes back must not block the viewport forever.
        while self.in_flight.front().is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(3)) {
            self.in_flight.pop_front();
        }
        if self.in_flight.len() < self.gpu.frames_in_flight() {
            self.submit(cx);
        } else if !self.retry_scheduled {
            // Normally the next presented frame submits this one; re-check
            // in case that frame is lost.
            self.retry_scheduled = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                let _ = this.update(cx, |this: &mut Viewport, cx| {
                    this.retry_scheduled = false;
                    this.submit_if_ready(cx);
                });
            })
            .detach();
        }
    }

    pub fn set_projection(&mut self, p: Projection, cx: &mut Context<Self>) {
        self.camera.projection = p;
        self.camera_changed(cx);
    }

    /// Animate to a standard view (keeps target, fits nothing).
    pub fn animate_to_view(&mut self, view: StandardView, cx: &mut Context<Self>) {
        let mut to = self.camera;
        to.set_view(view);
        self.animate_to(to, cx);
    }

    /// Animate to an arbitrary camera orientation looking along `forward`.
    pub fn animate_to_direction(&mut self, forward: DVec3, up: DVec3, cx: &mut Context<Self>) {
        let mut to = self.camera;
        to.rotation = Camera::look_rotation(forward, up);
        self.animate_to(to, cx);
    }

    pub fn animate_to(&mut self, to: Camera, cx: &mut Context<Self>) {
        // Live (behaviours.md "N"): Onshape's view changes take about half a
        // second (`CAMERA_LONG_ANIMATION_TIME_MS`).
        self.animate_to_with(to, nav::CAMERA_LONG_ANIMATION, cx);
    }

    /// Animate to `to` over `duration` with Onshape's easing (smootherstep).
    pub fn animate_to_with(&mut self, to: Camera, duration: Duration, cx: &mut Context<Self>) {
        self.inertia = None;
        self.anim = Some(CameraAnimation { from: self.camera, to, start: Instant::now(), duration });
        self.dirty = true;
        cx.notify();
    }

    /// Zoom to fit [`Self::fit_bounds`] (the whole scene when unset), like
    /// Onshape: parts fill the view, default planes only count when there is
    /// nothing else, and an overlay at the left is kept clear.
    pub fn zoom_to_fit(&mut self, animate: bool, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.fit_bounds.or_else(|| self.scene.bounds().map(|(lo, hi)| (lo.as_dvec3(), hi.as_dvec3()))) else { return };
        let size = self.size();
        let mut to = self.camera;
        fit_camera_to_box(&mut to, lo, hi, size, self.fit_inset_left);
        if animate {
            self.animate_to(to, cx);
        } else {
            self.camera = to;
            self.camera_changed(cx);
        }
    }

    /// Zoom so the box (viewport-local corners) fills the view.
    fn zoom_to_box(&mut self, a: DVec2, b: DVec2, cx: &mut Context<Self>) {
        let size = self.size();
        let (w, h) = ((a.x - b.x).abs(), (a.y - b.y).abs());
        if w < 4.0 || h < 4.0 || size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        let mut to = self.camera;
        // Bring the box center to the view center, then zoom about it.
        to.pan(size * 0.5 - (a + b) * 0.5, size.y);
        to.zoom_at((w / size.x).max(h / size.y), size * 0.5, size);
        self.animate_to(to, cx);
    }

    fn camera_changed(&mut self, cx: &mut Context<Self>) {
        self.pick_dirty = true;
        self.pick_changed_at = Instant::now();
        cx.emit(ViewportEvent::CameraChanged);
        self.request_frame(cx);
    }

    fn local(&self, p: Point<Pixels>) -> DVec2 {
        DVec2::new(
            f64::from(f32::from(p.x - self.bounds.origin.x)),
            f64::from(f32::from(p.y - self.bounds.origin.y)),
        )
    }

    fn device_px(&self, local: DVec2) -> (i32, i32) {
        let s = self.scale as f64;
        ((local.x * s) as i32, (local.y * s) as i32)
    }

    /// Pick id at a viewport-local position from the cached pick rectangle:
    /// `None` when the cache does not cover the position (ask the GPU).
    fn pick_at(&self, local: DVec2) -> Option<Option<u32>> {
        let pick = self.pick_cache.as_ref()?;
        let (x, y) = self.device_px(local);
        let r = (PICK_RADIUS * self.scale as f64) as i32;
        if !pick.covers(x, y, r) {
            return None;
        }
        let prio = self.pick_priority.clone();
        Some(pick.pick(x, y, r, |id| prio(id)))
    }

    /// Ask the render thread for a pick rectangle.
    fn request_pick(&mut self, rect: PickRect, purpose: PickPurpose) {
        let token = self.next_pick_token;
        self.next_pick_token += 1;
        self.pick_requests.insert(token, (self.pick_epoch, purpose, Instant::now()));
        self.gpu.pick(self.target_id, token, rect);
    }

    /// Refresh the hover id at the last pointer position from the GPU (one
    /// query in flight at a time).
    fn request_hover(&mut self) {
        if self.drag == Drag::Nav || self.anim.is_some() || !self.pointer_inside {
            return;
        }
        if self.hover_query {
            self.hover_query_again = true;
            return;
        }
        self.hover_query = true;
        let (x, y) = self.device_px(self.last_pos);
        let half = PICK_HALF.max((PICK_RADIUS * self.scale as f64) as i32 + 1);
        self.request_pick(PickRect::around(x, y, half), PickPurpose::Hover);
    }

    fn whole_frame_rect(&self) -> PickRect {
        let s = self.size() * self.scale as f64;
        PickRect { x: 0, y: 0, width: s.x.ceil() as u32 + 1, height: s.y.ceil() as u32 + 1, depth_samples: 0 }
    }

    fn on_pick(&mut self, token: u64, result: Result<Arc<PickBuffer>, String>, cx: &mut Context<Self>) {
        let Some((epoch, purpose, asked)) = self.pick_requests.remove(&token) else { return };
        noshape_render::perf::record("v.pick_latency", asked.elapsed());
        let buf = match result {
            Ok(b) => Some(b),
            Err(e) => {
                log::warn!("pick failed: {e}");
                None
            }
        };
        let current = epoch == self.pick_epoch;
        if current
            && let Some(b) = &buf
            && !matches!(purpose, PickPurpose::DepthPivot)
        {
            self.pick_cache = Some(b.clone());
        }
        let r = (PICK_RADIUS * self.scale as f64) as i32;
        let prio = self.pick_priority.clone();
        let pick_in = |b: &Option<Arc<PickBuffer>>, p: DVec2, s: f64| -> Option<u32> {
            b.as_ref()?.pick((p.x * s) as i32, (p.y * s) as i32, r, |id| prio(id))
        };
        let s = self.scale as f64;
        match purpose {
            PickPurpose::Hover => {
                self.hover_query = false;
                if !self.pointer_inside {
                    self.hover_query_again = false;
                } else if current {
                    match self.pick_at(self.last_pos) {
                        Some(id) => self.set_hover(id, cx),
                        // The pointer moved out of the rectangle meanwhile.
                        None => self.hover_query_again = true,
                    }
                } else {
                    self.hover_query_again = true;
                }
                if std::mem::take(&mut self.hover_query_again) {
                    self.request_hover();
                }
            }
            PickPurpose::Click { position, modifiers } => {
                let pick = pick_in(&buf, position, s);
                cx.emit(ViewportEvent::Click { pick, position, modifiers });
            }
            PickPurpose::ContextMenu { position, window_position } => {
                let pick = pick_in(&buf, position, s);
                cx.emit(ViewportEvent::ContextMenu { pick, position: window_position });
            }
            PickPurpose::Box { a, b, crossing, modifiers } => {
                let ids = match &buf {
                    Some(buf) => box_ids(buf, a, b, s, crossing),
                    None => Vec::new(),
                };
                cx.emit(ViewportEvent::BoxSelect { ids, modifiers });
            }
            PickPurpose::DepthPivot => {
                let frame = buf.as_ref().map(|b| (b.frame_width, b.frame_height)).unwrap_or_default();
                let pivot = buf.as_ref().and_then(|b| nav::depth_average_pivot(&b.samples, frame, b.view_proj?));
                if self.depth_pivot_asked == Some(epoch) {
                    self.depth_pivot_asked = None;
                }
                if current {
                    self.depth_pivot = Some((epoch, pivot));
                }
                // A drag that started meanwhile picks the centre up now (the
                // samples show the view at the press; a world point stays valid).
                if std::mem::take(&mut self.pivot_awaits_depth)
                    && self.drag == Drag::Nav
                    && let Some(p) = pivot
                {
                    self.orbit_pivot = Some(p);
                    if let Some((inertia, _)) = &mut self.inertia {
                        inertia.pivot = p;
                    }
                }
            }
        }
    }

    fn set_hover(&mut self, id: Option<u32>, cx: &mut Context<Self>) {
        if self.hover != id {
            self.hover = id;
            cx.emit(ViewportEvent::Hover(id));
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let size = self.bounds.size;
        let scale = self.scale;
        let w = (f32::from(size.width) * scale).round() as u32;
        let h = (f32::from(size.height) * scale).round() as u32;
        if w < 2 || h < 2 {
            return;
        }
        // The pick buffer is only rendered when a pick query arrives, so it is
        // simply invalidated here (also during navigation). Any camera or
        // size change invalidates it, also when the caller only asked for a
        // redraw (e.g. keyboard zoom).
        let view = (self.camera, w, h);
        let pick = std::mem::take(&mut self.pick_dirty) | (self.last_submitted_view != Some(view));
        self.last_submitted_view = Some(view);
        if pick {
            self.pick_epoch += 1;
            self.pick_cache = None;
            self.schedule_hover_refresh(cx);
        }
        self.seq += 1;
        self.in_flight.push_back((self.seq, Instant::now()));
        self.gpu.submit(RenderJob {
            target: self.target_id,
            scene: self.scene.clone(),
            camera: self.camera,
            width: w,
            height: h,
            device_pixel_ratio: scale,
            style: self.style,
            pick,
            seq: self.seq,
        });
        self.dirty = false;
    }

    /// After the pick buffer was invalidated (geometry or camera changed),
    /// update the hover once things settle (not on every zoom step).
    fn schedule_hover_refresh(&mut self, cx: &mut Context<Self>) {
        const SETTLE: Duration = Duration::from_millis(60);
        self.pick_changed_at = Instant::now();
        if self.hover_refresh_pending || self.drag == Drag::Nav || self.anim.is_some() {
            return;
        }
        self.hover_refresh_pending = true;
        cx.spawn(async move |this, cx| {
            let mut wait = SETTLE;
            loop {
                cx.background_executor().timer(wait).await;
                let again = this.update(cx, |this: &mut Viewport, _| {
                    let quiet = this.pick_changed_at.elapsed();
                    if quiet < SETTLE {
                        return Some(SETTLE - quiet);
                    }
                    this.hover_refresh_pending = false;
                    if this.pick_cache.is_none() {
                        this.request_hover();
                    }
                    None
                });
                match again {
                    Ok(Some(w)) => wait = w,
                    _ => break,
                }
            }
        })
        .detach();
    }

    fn present(&mut self, seq: u64, result: Result<(Frame, Arc<RenderImage>), String>, window: &mut Window, cx: &mut Context<Self>) {
        // Frames up to `seq` are done (older ones may have been coalesced away).
        while self.in_flight.front().is_some_and(|(s, _)| *s <= seq) {
            let (s, t) = self.in_flight.pop_front().expect("front");
            if s == seq {
                noshape_render::perf::record("v.submit_to_present", t.elapsed());
            }
        }
        if seq < self.last_presented {
            self.submit_if_ready(cx);
            return;
        }
        self.last_presented = seq;
        match result {
            Ok((frame, image)) => {
                if noshape_render::perf::enabled() {
                    static FIRST: std::sync::Once = std::sync::Once::new();
                    FIRST.call_once(|| noshape_render::perf::milestone("first viewport frame presented"));
                    noshape_render::perf::record("v.frame_to_present", frame.rendered_at.elapsed());
                    self.presented_at = Some(Instant::now());
                }
                if let Some(old) = self.image.replace(image) {
                    let _ = window.drop_image(old);
                }
                self.frames_presented += 1;
            }
            Err(e) => log::error!("render failed: {e}"),
        }
        self.bench_presented();
        self.submit_if_ready(cx);
        cx.notify();
    }

    /// Record presentation stats of the running orbit benchmark.
    fn bench_presented(&mut self) {
        let Some(b) = &mut self.bench else { return };
        if b.started.is_none() {
            return;
        }
        let now = Instant::now();
        if let Some(t) = b.last_present.replace(now) {
            noshape_render::perf::record("v.frame_interval", now - t);
        }
    }

    /// One simulated mouse-move of the orbit benchmark; false when finished.
    fn bench_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(b) = &mut self.bench else { return false };
        if b.started.is_none() {
            if self.scene.instances.is_empty() || self.bounds.size.width <= px(1.0) {
                return true;
            }
            log::info!(target: "perf", "orbit bench: start ({} frames, {} instances, {:.0}x{:.0} logical px, scale {})", b.frames, self.scene.instances.len(), f32::from(self.bounds.size.width), f32::from(self.bounds.size.height), self.scale);
            noshape_render::perf::reset();
            b.started = Some(Instant::now());
            b.frames_at_start = self.frames_presented;
            b.zoom = std::env::var("NOSHAPE_PERF_BENCH").is_ok_and(|m| m == "zoom");
            if !b.zoom {
                self.drag = Drag::Nav;
            }
        }
        let presented = self.frames_presented - b.frames_at_start;
        if presented >= b.frames {
            let el = b.started.map(|t| t.elapsed().as_secs_f64()).unwrap_or(1.0);
            log::info!(target: "perf", "orbit bench: {presented} frames in {:.0} ms = {:.1} fps", el * 1000.0, presented as f64 / el);
            log::info!(target: "perf", "orbit bench: {} window renders for {presented} presented frames", b.renders);
            noshape_render::perf::flush();
            self.bench = None;
            self.drag = Drag::None;
            self.pick_dirty = true;
            self.dirty = true;
            cx.notify();
            return false;
        }
        b.ticks += 1;
        if b.zoom {
            // Scroll-wheel zoom in and out around the viewport centre.
            let factor = if (b.ticks / 15) % 2 == 0 { 1.04 } else { 1.0 / 1.04 };
            let size = self.size();
            self.camera.zoom_at(factor, size * 0.5, size);
        } else {
            let h = self.size().y;
            self.camera.orbit(DVec2::new(3.0, 0.75), h);
        }
        self.camera_changed(cx);
        true
    }

    // ------------------------------------------------------------ input

    /// The bounds Onshape's zoom limits are measured on (the scene).
    fn zoom_bounds(&self) -> Option<(DVec3, DVec3)> {
        self.scene.bounds().map(|(lo, hi)| (lo.as_dvec3(), hi.as_dvec3())).or(self.fit_bounds)
    }

    /// Rotate / pan centre for a drag starting at `at` (Onshape's
    /// `getRotateCenterForCanvasLocation`): the geometry under the cursor,
    /// else the model's centre when it fits in the view, else the point on
    /// the cursor ray at the model's depth.
    fn pivot_at(&mut self, at: DVec2) -> DVec3 {
        let (o, d) = self.ray(at);
        if let Some(p) = self.orbit_pivot_fn.as_ref().and_then(|f| f(o, d)) {
            return p;
        }
        // Visible-depth average (asked for at the press).
        match self.depth_pivot {
            Some((e, Some(p))) if e == self.pick_epoch => return p,
            Some((e, None)) if e == self.pick_epoch => {}
            _ => self.pivot_awaits_depth = self.depth_pivot_asked.is_some(),
        }
        let Some((lo, hi)) = self.fit_bounds.or_else(|| self.zoom_bounds()) else { return self.camera.target };
        let center = (lo + hi) * 0.5;
        let size = self.size();
        let slack = size * 0.1;
        let fits = (0..8).all(|i| {
            let c = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
            self.project(c).is_some_and(|p| p.x >= -slack.x && p.y >= -slack.y && p.x <= size.x + slack.x && p.y <= size.y + slack.y)
        });
        if fits {
            return center;
        }
        let min = 0.01 * (hi - lo).length();
        o + d * d.dot(center - o).max(min)
    }

    /// At a navigation press with no geometry under the cursor, ask the
    /// render thread for visible-depth samples (Onshape's rotate centre falls
    /// back to their average); answered before most drags start.
    fn request_depth_pivot(&mut self, at: DVec2) {
        if self.depth_pivot.is_some_and(|(e, _)| e == self.pick_epoch) || self.depth_pivot_asked == Some(self.pick_epoch) {
            return;
        }
        let (o, d) = self.ray(at);
        if self.orbit_pivot_fn.as_ref().and_then(|f| f(o, d)).is_some() {
            return;
        }
        self.depth_pivot_asked = Some(self.pick_epoch);
        self.request_pick(PickRect::depth_samples(nav::DEPTH_SAMPLES_PER_VIEWPORT), PickPurpose::DepthPivot);
    }

    /// Switch the navigation drag to `mode` (`updateManipulationMode`).
    fn set_nav_mode(&mut self, mode: Mode, at: DVec2) {
        let prev = std::mem::replace(&mut self.nav_mode, mode);
        if prev == mode {
            return;
        }
        if nav::needs_new_pivot(prev, mode) {
            let perspective_pan = mode == Mode::Pan && self.camera.projection == noshape_render::Projection::Perspective;
            self.orbit_pivot = (mode != Mode::Pan || perspective_pan).then(|| self.pivot_at(at));
        }
        if mode == Mode::Rotate {
            self.last_rotation = Some(Instant::now());
            self.rotation_speed = RotationSpeed::default();
        }
    }

    /// Re-resolve the mode when the buttons or a bound modifier changed.
    fn update_nav_mode(&mut self, modifiers: Modifiers, at: DVec2) {
        let Some(resolver) = &self.nav_resolver else { return };
        let keys = resolver.input_key(modifiers.into());
        let mode = resolver.resolve(self.buttons, keys, self.nav_mode);
        self.nav_keys = keys;
        self.set_nav_mode(mode, at);
    }

    /// End a navigation drag: a rotation still moving at release keeps
    /// turning (inertia).
    fn end_nav(&mut self, cx: &mut Context<Self>) {
        if self.nav_mode == Mode::Rotate
            && self.last_rotation.is_some_and(|t| t.elapsed() < nav::INERTIA_RELEASE_WINDOW)
            && let Some(pivot) = self.orbit_pivot
        {
            self.inertia = Some((Inertia { speed: self.rotation_speed.0, factor: 1.0, pivot }, Instant::now()));
        } else {
            self.orbit_pivot = None;
        }
        self.nav_mode = Mode::None;
        self.nav_resolver = None;
        self.drag = Drag::None;
        cx.emit(ViewportEvent::CameraChanged);
        // Settled: refresh the hover (renders the pick buffer lazily).
        self.request_hover();
        cx.notify();
    }

    /// Apply one drag step of the current navigation mode.
    fn nav_drag(&mut self, delta: DVec2, cx: &mut Context<Self>) {
        let size = self.size();
        let pivot = self.orbit_pivot.unwrap_or(self.camera.target);
        match self.nav_mode {
            Mode::None => return,
            Mode::Rotate => {
                nav::drag_rotate(&mut self.camera, delta, size, pivot, false);
                self.rotation_speed.push(delta);
                self.last_rotation = Some(Instant::now());
            }
            Mode::AxisRotate => {
                // The view is first animated to the nearest floor-down view
                // (drags wait for that), then turns about world Z.
                if self.anim.is_some() {
                    return;
                }
                match nav::level_view(&self.camera, pivot) {
                    (level, None) => {
                        self.camera = level;
                        nav::drag_axis_rotate(&mut self.camera, delta, size, pivot);
                    }
                    (level, Some(duration)) => {
                        self.animate_to_with(level, duration, cx);
                        return;
                    }
                }
            }
            Mode::Pan => nav::pan(&mut self.camera, delta, size, self.orbit_pivot),
            Mode::Zoom => {
                // `zoomOnDrag`: device pixels / DRAG_ZOOM_FACTOR, up zooms in.
                let amount = -delta.y * self.scale as f64 / nav::DRAG_ZOOM_FACTOR;
                let bounds = self.zoom_bounds();
                if amount.abs() < 1e-12 || !nav::zoom_at(&mut self.camera, Zoom::Amount(amount), self.zoom_anchor, size, bounds) {
                    return;
                }
            }
        }
        self.camera_changed(cx);
    }

    fn on_mouse_down(&mut self, e: &MouseDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        self.buttons.set(e.button, true);
        cx.stop_propagation();
        if self.drag == Drag::Nav {
            // Another button joined the drag.
            self.update_nav_mode(e.modifiers, p);
            return;
        }
        self.press_pos = p;
        self.last_pos = p;
        match e.button {
            MouseButton::Left if self.tool_captures_left || self.manipulator_at(p) => {
                self.drag = Drag::Tool;
                cx.emit(ViewportEvent::ToolDown { position: p, modifiers: e.modifiers, click_count: e.click_count });
            }
            // Onshape: double-click the middle button to zoom to fit.
            MouseButton::Middle if e.click_count >= 2 => {
                self.drag = Drag::None;
                self.zoom_to_fit(true, cx);
            }
            MouseButton::Left | MouseButton::Right | MouseButton::Middle => {
                self.ctrl_click_press = e.button == MouseButton::Right && converted_ctrl_click();
                if e.button != MouseButton::Left && !self.ctrl_click_press {
                    self.request_depth_pivot(p);
                }
                self.drag = Drag::Pending(e.button);
            }
            _ => {}
        }
    }

    fn on_hover_change(&mut self, hovered: &bool, _window: &mut Window, cx: &mut Context<Self>) {
        self.pointer_inside = *hovered;
        if !*hovered {
            self.hover_query_again = false;
            self.set_hover(None, cx);
        }
    }

    fn on_mouse_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        // Moves outside the bounds arrive while a button is held (drags).
        self.pointer_inside = self.bounds.contains(&e.position);
        let delta = p - self.last_pos;
        self.last_pos = p;
        if self.selection_count > 0 {
            cx.notify();
        }
        // A release we did not see (outside the window): the drag is over.
        if e.pressed_button.is_none() && self.buttons.any() {
            self.buttons = Buttons::default();
            if self.drag == Drag::Nav {
                self.end_nav(cx);
                return;
            }
        }
        if let Drag::Pending(button) = self.drag
            && p.distance(self.press_pos) > nav::DRAG_THRESHOLD_PX
        {
            // `onDragStart`: what this button + modifier combination does.
            let resolver = ModeResolver::new(MouseControls::current(), nav::Space::ThreeD);
            let keys = resolver.input_key(e.modifiers.into());
            let mode = if self.ctrl_click_press { Mode::None } else { resolver.resolve(self.buttons, keys, Mode::None) };
            if mode != Mode::None {
                self.drag = Drag::Nav;
                self.anim = None;
                self.inertia = None;
                self.nav_resolver = Some(resolver);
                self.nav_keys = keys;
                self.zoom_anchor = self.press_pos;
                self.pivot_awaits_depth = false;
                self.set_nav_mode(mode, self.press_pos);
            } else {
                // Dragging from geometry with no tool is a box select too
                // (Onshape only drags geometry inside sketches/assemblies);
                // other unbound drags do nothing (and open no menu).
                self.drag = if button == MouseButton::Left { Drag::Box } else { Drag::None };
            }
        }
        match self.drag {
            Drag::Nav => {
                if let Some(resolver) = &self.nav_resolver
                    && resolver.input_key(e.modifiers.into()) != self.nav_keys
                {
                    self.update_nav_mode(e.modifiers, p);
                }
                self.nav_drag(delta, cx);
            }
            Drag::Box => {
                self.box_rect = Some((self.press_pos, p));
                cx.notify();
            }
            Drag::Tool => {
                cx.emit(ViewportEvent::ToolDrag { position: p });
            }
            Drag::None | Drag::Pending(_) => {
                match self.pick_at(p) {
                    Some(id) => self.set_hover(id, cx),
                    None => self.request_hover(),
                }
                cx.emit(ViewportEvent::PointerMove { position: p, modifiers: e.modifiers });
            }
        }
        let _ = window;
    }

    fn on_mouse_up(&mut self, e: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        self.buttons.set(e.button, false);
        if self.drag == Drag::Nav {
            if self.buttons.any() {
                self.update_nav_mode(e.modifiers, p);
                if self.nav_mode != Mode::None {
                    return;
                }
            }
            self.end_nav(cx);
            return;
        }
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Pending(MouseButton::Left) => match self.pick_at(p) {
                Some(pick) => cx.emit(ViewportEvent::Click { pick, position: p, modifiers: e.modifiers }),
                None => {
                    let (x, y) = self.device_px(p);
                    self.request_pick(PickRect::around(x, y, PICK_HALF), PickPurpose::Click { position: p, modifiers: e.modifiers });
                }
            },
            Drag::Pending(MouseButton::Right) => match self.pick_at(p) {
                Some(pick) => cx.emit(ViewportEvent::ContextMenu { pick, position: e.position }),
                None => {
                    let (x, y) = self.device_px(p);
                    self.request_pick(PickRect::around(x, y, PICK_HALF), PickPurpose::ContextMenu { position: p, window_position: e.position });
                }
            },
            Drag::Box => {
                if let Some((a, b)) = self.box_rect.take() {
                    if std::mem::take(&mut self.zoom_window) {
                        self.zoom_to_box(a, b, cx);
                        cx.notify();
                        return;
                    }
                    let crossing = b.x < a.x;
                    // Crossing selection only needs the box; window selection
                    // also needs what is outside it.
                    let rect = if crossing {
                        let s = self.scale as f64;
                        let (x0, y0) = ((a.x.min(b.x) * s).floor() as i32, (a.y.min(b.y) * s).floor() as i32);
                        let (x1, y1) = ((a.x.max(b.x) * s).ceil() as i32, (a.y.max(b.y) * s).ceil() as i32);
                        PickRect { x: x0, y: y0, width: (x1 - x0).max(0) as u32 + 1, height: (y1 - y0).max(0) as u32 + 1, depth_samples: 0 }
                    } else {
                        self.whole_frame_rect()
                    };
                    self.request_pick(rect, PickPurpose::Box { a, b, crossing, modifiers: e.modifiers });
                }
            }
            Drag::Tool => cx.emit(ViewportEvent::ToolUp { position: p, modifiers: e.modifiers }),
            _ => {}
        }
        cx.notify();
    }

    /// A button released outside the viewport: finish a drag that started in
    /// it (a press that never moved is dropped).
    fn on_mouse_up_out(&mut self, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        match self.drag {
            Drag::Nav | Drag::Box | Drag::Tool => self.on_mouse_up(e, window, cx),
            _ => {
                self.buttons.set(e.button, false);
                if !self.buttons.any() {
                    self.drag = Drag::None;
                }
            }
        }
    }

    /// Mouse wheel, two-finger trackpad scroll (and its momentum): zoom about
    /// the cursor, never pan (Onshape's `zoomOnMouseWheelScroll`).
    fn on_scroll(&mut self, e: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if self.drag == Drag::Box {
            return;
        }
        let reverse = MouseControls::current().reverse_scroll;
        let Some(amount) = nav::wheel_zoom_amount(e.delta, nav::Platform::current(), reverse, e.modifiers.shift) else { return };
        let (p, size, bounds) = (self.local(e.position), self.size(), self.zoom_bounds());
        if nav::zoom_at(&mut self.camera, Zoom::Amount(amount), p, size, bounds) {
            self.anim = None;
            self.camera_changed(cx);
        }
    }

    /// Trackpad pinch (macOS magnify, Windows precision touchpad, Linux
    /// gestures): the model scales with the fingers about the pinch centre.
    fn on_pinch(&mut self, e: &PinchEvent, _window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if self.drag == Drag::Box || e.delta == 0.0 {
            return;
        }
        let (p, size, bounds) = (self.local(e.position), self.size(), self.zoom_bounds());
        if nav::zoom_at(&mut self.camera, Zoom::Pinch(nav::pinch_ratio(e.delta)), p, size, bounds) {
            self.anim = None;
            self.camera_changed(cx);
        }
    }

    // ------------------------------------------------------------ keyboard

    /// Arrow keys and the view cube arrows (`rotateInDirection`): turn the
    /// view `degrees` over 250 ms about the camera target. Steps run side by
    /// side, so a held key (key repeat) keeps the view turning.
    pub fn rotate_step(&mut self, dir: RotateDir, degrees: f64, cx: &mut Context<Self>) {
        let (axis, angle) = nav::rotate_step_axis(&self.camera, dir, degrees);
        if self.step_rotations.is_empty() {
            self.step_clock = Some(Instant::now());
        }
        self.step_rotations.push(StepRotation::new(axis, angle, self.camera.target));
        self.dirty = true;
        cx.notify();
    }

    /// Ctrl+Shift+arrows (`animatePan`): the model moves 150 px toward the
    /// arrow (`dx`, `dy` = ±1). Ignored while the camera animates.
    pub fn pan_step(&mut self, dx: f64, dy: f64, cx: &mut Context<Self>) {
        if self.anim.is_some() {
            return;
        }
        let center = self.zoom_bounds().map(|(lo, hi)| (lo + hi) * 0.5).unwrap_or(self.camera.target);
        let to = nav::pan_step(&self.camera, dx, dy, self.size(), center);
        self.animate_to_with(to, nav::CAMERA_SHORT_ANIMATION, cx);
    }

    /// Shift+Z / Z (`zoomIn` / `zoomOut`): ×1.75 about the view centre,
    /// animated. Ignored while the camera animates.
    pub fn zoom_step(&mut self, zoom_in: bool, cx: &mut Context<Self>) {
        if self.anim.is_some() {
            return;
        }
        let to = nav::zoom_step(&self.camera, zoom_in, self.zoom_bounds(), self.size());
        self.animate_to_with(to, nav::CAMERA_SHORT_ANIMATION, cx);
    }
}

/// macOS: whether the newest right press is a Ctrl+left click the platform
/// converted (vendor/gpui-pre-macos).
#[cfg(target_os = "macos")]
fn converted_ctrl_click() -> bool {
    gpui_macos::last_right_press_was_ctrl_click()
}

#[cfg(not(target_os = "macos"))]
fn converted_ctrl_click() -> bool {
    false
}

/// Distinct ids for a box selection from a pick rectangle: crossing selects
/// everything visible inside the box, window selection only what is visible
/// inside and nowhere outside it.
fn box_ids(buf: &PickBuffer, a: DVec2, b: DVec2, s: f64, crossing: bool) -> Vec<u32> {
    let (x0, x1) = ((a.x.min(b.x) * s) as i64, (a.x.max(b.x) * s) as i64);
    let (y0, y1) = ((a.y.min(b.y) * s) as i64, (a.y.max(b.y) * s) as i64);
    if crossing {
        return buf.ids_in(x0, y0, x1, y1);
    }
    let mut inside = ahash::AHashSet::new();
    let mut outside = ahash::AHashSet::new();
    buf.for_each(|x, y, id| {
        if x >= x0 && x < x1 && y >= y0 && y < y1 {
            inside.insert(id);
        } else {
            outside.insert(id);
        }
    });
    let mut v: Vec<u32> = inside.difference(&outside).copied().collect();
    v.sort_unstable();
    v
}

impl Drop for Viewport {
    fn drop(&mut self) {
        self.gpu.unregister_target(self.target_id);
    }
}

impl Render for Viewport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(b) = &mut self.bench
            && b.started.is_some()
        {
            b.renders += 1;
        }
        // Advance camera animation.
        if let Some(anim) = &self.anim {
            let t = anim.start.elapsed().as_secs_f64() / anim.duration.as_secs_f64();
            self.camera = anim.from.interpolate(&anim.to, nav::ease(t));
            self.dirty = true;
            self.pick_dirty = true;
            if t >= 1.0 {
                self.anim = None;
                cx.emit(ViewportEvent::CameraChanged);
            } else {
                window.request_animation_frame();
            }
        }
        // Rotation inertia (Onshape's post-rotation animation: ×0.77 per
        // 60 Hz frame until under a pixel per step).
        if let Some((mut inertia, last)) = self.inertia.take() {
            let now = Instant::now();
            match inertia.step(now - last) {
                Some(d) => {
                    let size = self.size();
                    nav::drag_rotate(&mut self.camera, d, size, inertia.pivot, false);
                    self.inertia = Some((inertia, now));
                    window.request_animation_frame();
                }
                None => {
                    self.orbit_pivot = None;
                    cx.emit(ViewportEvent::CameraChanged);
                }
            }
            self.dirty = true;
            self.pick_dirty = true;
        }
        // Keyboard / view cube rotation steps.
        if !self.step_rotations.is_empty() {
            let now = Instant::now();
            let dt = self.step_clock.map(|t| now - t).unwrap_or_default();
            self.step_clock = Some(now);
            let camera = &mut self.camera;
            self.step_rotations.retain_mut(|s| s.advance(camera, dt));
            if self.step_rotations.is_empty() {
                self.step_clock = None;
                cx.emit(ViewportEvent::CameraChanged);
            } else {
                window.request_animation_frame();
            }
            self.dirty = true;
            self.pick_dirty = true;
        }
        self.scale = window.scale_factor();
        self.submit_if_ready(cx);

        let entity = cx.entity();
        let box_rect = self.box_rect;
        let image = self.image.clone();
        self.sync_cursor(window, cx);
        div()
            .id("viewport")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(crate::theme::p().app_bg)
            .child(
                canvas(
                    move |bounds, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.last_layout = Some(Instant::now());
                            if let Some(t) = this.presented_at.take() {
                                noshape_render::perf::record("v.present_to_paint", t.elapsed());
                            }
                            if this.bounds != bounds {
                                this.bounds = bounds;
                                this.pick_dirty = true;
                                this.request_frame(cx);
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .children(image.map(|img| gpui_kit::img(img).absolute().top_0().left_0().size_full()))
            .children(box_rect.map(|(a, b)| {
                let crossing = b.x < a.x;
                let (x0, y0) = (a.x.min(b.x) as f32, a.y.min(b.y) as f32);
                let (w, h) = ((a.x - b.x).abs() as f32, (a.y - b.y).abs() as f32);
                // Onshape box select: window #789ad1 / #d0dcef, crossing #ffdc74 / #fff3cf.
                let color = if crossing { rgb(0xffdc74) } else { rgb(0x789ad1) };
                div()
                    .absolute()
                    .left(px(x0))
                    .top(px(y0))
                    .w(px(w))
                    .h(px(h))
                    .border_1()
                    .when(crossing, |d| d.border_dashed())
                    .border_color(color)
                    .bg(if crossing { rgb(0xfff3cf) } else { rgb(0xd0dcef) }.opacity(0.5))
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_hover(cx.listener(Self::on_hover_change))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up_out))
            .on_mouse_up_out(MouseButton::Right, cx.listener(Self::on_mouse_up_out))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(Self::on_mouse_up_out))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_pinch(cx.listener(Self::on_pinch))
    }
}

impl Viewport {
    /// Onshape's selection-count cursor over the graphics area (cursor.rs).
    fn sync_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let want = (self.selection_count > 0 && self.pointer_inside && matches!(self.drag, Drag::None | Drag::Pending(_))).then_some(self.selection_count.min(5));
        if self.cursor_applied == Some(want) || (want.is_none() && self.cursor_applied.is_none_or(|a| a.is_none())) {
            return;
        }
        self.cursor_platform = crate::cursor::set(window, want, cx);
        self.cursor_applied = Some(want);
        if want.is_some() && self.cursor_platform {
            // GPUI sets its own cursor at the end of a frame in which the
            // hovered style changed; set ours again once that frame is out.
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.cursor_applied == Some(want) {
                        crate::cursor::set(window, want, cx);
                    }
                });
            })
            .detach();
        }
    }
}

/// Fit `camera` to a world box for a viewport of `size` logical pixels whose
/// left `inset` pixels are covered (ignored when it would leave too little).
pub fn fit_camera_to_box(camera: &mut Camera, lo: DVec3, hi: DVec3, size: DVec2, inset: f64) {
    let inset = if size.x - inset >= size.x * 0.45 { inset.max(0.0) } else { 0.0 };
    let width = (size.x - inset).max(1.0);
    let aspect = if size.y > 0.0 { width / size.y } else { 1.0 };
    camera.fit_box(lo, hi, aspect);
    if inset > 0.0 && size.y > 0.0 {
        // Move the model right, into the middle of the uncovered part.
        camera.pan(DVec2::new(inset * 0.5, 0.0), size.y);
    }
}
