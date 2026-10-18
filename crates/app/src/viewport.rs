//! 3D viewport element: shows frames from the shared render worker and turns
//! mouse input into camera navigation and pick events.
//!
//! Navigation (Onshape conventions):
//! * right-drag: rotate (turntable about world Z), right-click: context menu
//! * middle-drag or Ctrl+right-drag: pan
//! * scroll: zoom at cursor
//! * left-click: select, left-drag on empty space: box select
//!   (left→right = window, right→left = crossing)

use crate::gpu::Gpu;
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use glam::{DVec2, DVec3};
use noshape_render::{Camera, Frame, PickBuffer, PickRect, Projection, Scene, StandardView, Style, worker::RenderJob};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    Rotate,
    Pan,
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
    hover: Option<u32>,
    /// When true, left-drag goes to the active tool instead of box select.
    pub tool_captures_left: bool,
    /// Zoom to window (W): the next left-drag box zooms instead of selecting.
    pub zoom_window: bool,
    /// Point under the cursor for a ray (origin, direction): right-drag
    /// rotates about it (Onshape rotates about the geometry under the cursor).
    pub orbit_pivot_fn: Option<Arc<dyn Fn(DVec3, DVec3) -> Option<DVec3> + Send + Sync>>,
    orbit_pivot: Option<DVec3>,
    /// Last orbit step and when (for rotation inertia), and the inertia step.
    last_orbit: Option<(DVec2, Instant)>,
    inertia: Option<DVec2>,
    /// World-space manipulator handles (segments): pressing the left button
    /// near one routes it to the tool events even when `tool_captures_left`
    /// is false.
    pub manipulators: Vec<(DVec3, DVec3)>,
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
            hover: None,
            tool_captures_left: false,
            zoom_window: false,
            orbit_pivot_fn: None,
            orbit_pivot: None,
            last_orbit: None,
            inertia: None,
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
    /// Whether the pointer is over the viewport.
    pub fn pointer_inside(&self) -> bool {
        self.pointer_inside
    }

    pub fn size(&self) -> DVec2 {
        DVec2::new(f64::from(f32::from(self.bounds.size.width)), f64::from(f32::from(self.bounds.size.height)))
    }

    /// World-space ray through a viewport-local position.
    pub fn ray(&self, local: DVec2) -> (DVec3, DVec3) {
        self.camera.ray(local, self.size())
    }

    /// Whether a viewport-local position is on a manipulator handle.
    pub fn manipulator_at(&self, p: DVec2) -> bool {
        self.manipulators.iter().any(|&(a, b)| {
            let (Some(a), Some(b)) = (self.project(a), self.project(b)) else { return false };
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
        self.anim = Some(CameraAnimation { from: self.camera, to, start: Instant::now(), duration: Duration::from_millis(300) });
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
        if matches!(self.drag, Drag::Rotate | Drag::Pan) || self.anim.is_some() || !self.pointer_inside {
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
        PickRect { x: 0, y: 0, width: s.x.ceil() as u32 + 1, height: s.y.ceil() as u32 + 1 }
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
        if current && let Some(b) = &buf {
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
        if self.hover_refresh_pending || matches!(self.drag, Drag::Rotate | Drag::Pan) || self.anim.is_some() {
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
                self.drag = Drag::Rotate;
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

    fn on_mouse_down(&mut self, e: &MouseDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        self.press_pos = p;
        self.last_pos = p;
        self.anim = None;
        self.inertia = None;
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
                self.drag = Drag::Pending(e.button);
            }
            _ => {}
        }
        cx.stop_propagation();
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
        let height = self.size().y;
        if let Drag::Pending(button) = self.drag
            && p.distance(self.press_pos) > 3.0
        {
            self.drag = match button {
                MouseButton::Right if e.modifiers.control || e.modifiers.shift => Drag::Pan,
                MouseButton::Right if self.orbit_pivot_fn.is_some() => {
                    let (o, d) = self.ray(self.press_pos);
                    self.orbit_pivot = self.orbit_pivot_fn.as_ref().and_then(|f| f(o, d));
                    Drag::Rotate
                }
                MouseButton::Right => Drag::Rotate,
                MouseButton::Middle => Drag::Pan,
                // Dragging from geometry with no tool is a box select too
                // (Onshape only drags geometry inside sketches/assemblies).
                MouseButton::Left => Drag::Box,
                _ => Drag::None,
            };
        }
        match self.drag {
            Drag::Rotate => {
                match self.orbit_pivot {
                    Some(pivot) => self.camera.orbit_around(pivot, delta, height),
                    None => self.camera.orbit(delta, height),
                }
                self.last_orbit = Some((delta, Instant::now()));
                self.camera_changed(cx);
            }
            Drag::Pan => {
                self.camera.pan(delta, height);
                self.camera_changed(cx);
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
                        PickRect { x: x0, y: y0, width: (x1 - x0).max(0) as u32 + 1, height: (y1 - y0).max(0) as u32 + 1 }
                    } else {
                        self.whole_frame_rect()
                    };
                    self.request_pick(rect, PickPurpose::Box { a, b, crossing, modifiers: e.modifiers });
                }
            }
            Drag::Tool => cx.emit(ViewportEvent::ToolUp { position: p, modifiers: e.modifiers }),
            Drag::Rotate | Drag::Pan => {
                // Rotation inertia: a flick keeps the view turning briefly.
                if drag == Drag::Rotate
                    && let Some((d, t)) = self.last_orbit.take()
                    && t.elapsed() < Duration::from_millis(60)
                    && d.length() > 2.0
                {
                    self.inertia = Some(d);
                    cx.notify();
                } else {
                    self.orbit_pivot = None;
                }
                // Settled: refresh the hover (renders the pick buffer lazily).
                self.request_hover();
            }
            _ => {}
        }
        cx.notify();
    }

    fn on_scroll(&mut self, e: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        let dy = match e.delta {
            ScrollDelta::Lines(l) => l.y as f64 * 40.0,
            ScrollDelta::Pixels(px) => f64::from(f32::from(px.y)),
        };
        if dy.abs() < 0.01 {
            return;
        }
        if let ScrollDelta::Pixels(px) = e.delta
            && e.modifiers.shift
        {
            let d = DVec2::new(f64::from(f32::from(px.x)), f64::from(f32::from(px.y)));
            self.camera.pan(-d, self.size().y);
        } else {
            let factor = (-dy * 0.0025).exp();
            self.camera.zoom_at(factor, p, self.size());
        }
        self.anim = None;
        self.camera_changed(cx);
    }
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
            self.camera = anim.from.lerp(&anim.to, t);
            self.dirty = true;
            self.pick_dirty = true;
            if t >= 1.0 {
                self.anim = None;
                cx.emit(ViewportEvent::CameraChanged);
            } else {
                window.request_animation_frame();
            }
        }
        // Rotation inertia (decays ×0.77 per frame, Onshape ROTATION_INERTIA).
        if let Some(d) = self.inertia {
            let h = self.size().y;
            match self.orbit_pivot {
                Some(pivot) => self.camera.orbit_around(pivot, d, h),
                None => self.camera.orbit(d, h),
            }
            let next = d * 0.77;
            self.inertia = (next.length() > 0.3).then_some(next);
            if self.inertia.is_none() {
                self.orbit_pivot = None;
                cx.emit(ViewportEvent::CameraChanged);
            }
            self.dirty = true;
            self.pick_dirty = true;
            window.request_animation_frame();
        }
        self.scale = window.scale_factor();
        self.submit_if_ready(cx);

        let entity = cx.entity();
        let box_rect = self.box_rect;
        let image = self.image.clone();
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
            .on_scroll_wheel(cx.listener(Self::on_scroll))
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
