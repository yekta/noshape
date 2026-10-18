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
use noshape_render::{Camera, Frame, PickBuffer, Projection, Scene, StandardView, Style, worker::RenderJob};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum ViewportEvent {
    Hover(Option<u32>),
    /// Left click (no drag). `pick` is the id under the cursor.
    Click { pick: Option<u32>, position: DVec2, modifiers: Modifiers, click_count: usize },
    /// Right click without drag.
    ContextMenu { pick: Option<u32>, position: Point<Pixels> },
    /// Box selection finished; ids are unique.
    BoxSelect { ids: Vec<u32>, crossing: bool, modifiers: Modifiers },
    /// Left-button press/drag/release while a tool captured the pointer.
    ToolDown { position: DVec2, modifiers: Modifiers, click_count: usize },
    ToolDrag { position: DVec2, modifiers: Modifiers },
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
    pick: Option<Arc<PickBuffer>>,
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
    /// World-space manipulator handles (segments): pressing the left button
    /// near one routes it to the tool events even when `tool_captures_left`
    /// is false.
    pub manipulators: Vec<(DVec3, DVec3)>,
    /// Pick ids that win ties in the pick radius (edges/points over faces).
    pub pick_priority: Arc<dyn Fn(u32) -> i32 + Send + Sync>,
    box_rect: Option<(DVec2, DVec2)>,
    anim: Option<CameraAnimation>,
    _frames: Task<()>,
    stats: FrameStats,
}

#[derive(Default, Clone, Copy)]
pub struct FrameStats {
    pub last_frame_ms: f32,
    pub frames: u64,
}

impl EventEmitter<ViewportEvent> for Viewport {}

impl Viewport {
    pub fn new(gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (target_id, rx) = gpu.register_target();
        let frames = cx.spawn_in(window, async move |this, cx| {
            while let Ok((seq, result, render_ms)) = rx.recv().await {
                let ok = this.update_in(cx, |this: &mut Viewport, window, cx| {
                    this.present(seq, result, render_ms, window, cx);
                });
                if ok.is_err() {
                    break;
                }
            }
        });
        Self {
            gpu,
            target_id,
            camera: Camera::default(),
            scene: Arc::new(Scene::default()),
            style: Style::default(),
            image: None,
            pick: None,
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
            manipulators: Vec::new(),
            pick_priority: Arc::new(|_| 0),
            box_rect: None,
            anim: None,
            _frames: frames,
            stats: FrameStats::default(),
        }
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn scene(&self) -> &Arc<Scene> {
        &self.scene
    }

    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    /// Replace the scene (geometry or highlight state changed).
    pub fn set_scene(&mut self, scene: Arc<Scene>, geometry_changed: bool, cx: &mut Context<Self>) {
        self.scene = scene;
        self.dirty = true;
        if geometry_changed {
            self.pick_dirty = true;
        }
        cx.notify();
    }

    pub fn hovered(&self) -> Option<u32> {
        self.hover
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
        self.dirty = true;
        cx.notify();
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

    /// Zoom to fit the current scene.
    pub fn zoom_to_fit(&mut self, animate: bool, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.scene.bounds() else { return };
        let center = ((lo + hi) * 0.5).as_dvec3();
        let radius = ((hi - lo).length() as f64 * 0.5).max(1e-4);
        let size = self.size();
        let aspect = if size.y > 0.0 { size.x / size.y } else { 1.0 };
        let mut to = self.camera;
        to.fit_sphere(center, radius, aspect);
        if animate {
            self.animate_to(to, cx);
        } else {
            self.camera = to;
            self.camera_changed(cx);
        }
    }

    fn camera_changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        self.pick_dirty = true;
        cx.emit(ViewportEvent::CameraChanged);
        cx.notify();
    }

    fn local(&self, p: Point<Pixels>) -> DVec2 {
        DVec2::new(
            f64::from(f32::from(p.x - self.bounds.origin.x)),
            f64::from(f32::from(p.y - self.bounds.origin.y)),
        )
    }

    fn pick_at(&self, local: DVec2) -> Option<u32> {
        let pick = self.pick.as_ref()?;
        let s = self.scale as f64;
        let prio = self.pick_priority.clone();
        pick.pick((local.x * s) as i32, (local.y * s) as i32, (5.0 * s) as i32, |id| prio(id))
    }

    fn ids_in_rect(&self, a: DVec2, b: DVec2) -> Vec<u32> {
        let Some(pick) = self.pick.as_ref() else { return vec![] };
        let s = self.scale as f64;
        let (x0, x1) = ((a.x.min(b.x) * s) as i64, (a.x.max(b.x) * s) as i64);
        let (y0, y1) = ((a.y.min(b.y) * s) as i64, (a.y.max(b.y) * s) as i64);
        let (w, h) = (pick.width as i64, pick.height as i64);
        let mut seen = ahash::AHashSet::new();
        for y in y0.max(0)..y1.min(h) {
            for x in x0.max(0)..x1.min(w) {
                let id = pick.ids[(y * w + x) as usize];
                if id != 0 {
                    seen.insert(id);
                }
            }
        }
        let mut v: Vec<u32> = seen.into_iter().collect();
        v.sort_unstable();
        v
    }

    /// Ids visible anywhere in the viewport (for window selection we need ids
    /// that are entirely inside: visible inside and not visible outside).
    fn ids_fully_inside(&self, a: DVec2, b: DVec2) -> Vec<u32> {
        let Some(pick) = self.pick.as_ref() else { return vec![] };
        let s = self.scale as f64;
        let (x0, x1) = ((a.x.min(b.x) * s) as i64, (a.x.max(b.x) * s) as i64);
        let (y0, y1) = ((a.y.min(b.y) * s) as i64, (a.y.max(b.y) * s) as i64);
        let (w, h) = (pick.width as i64, pick.height as i64);
        let mut inside = ahash::AHashSet::new();
        let mut outside = ahash::AHashSet::new();
        for y in 0..h {
            for x in 0..w {
                let id = pick.ids[(y * w + x) as usize];
                if id == 0 {
                    continue;
                }
                if x >= x0 && x < x1 && y >= y0 && y < y1 {
                    inside.insert(id);
                } else {
                    outside.insert(id);
                }
            }
        }
        let mut v: Vec<u32> = inside.difference(&outside).copied().collect();
        v.sort_unstable();
        v
    }

    fn set_hover(&mut self, id: Option<u32>, cx: &mut Context<Self>) {
        if self.hover != id {
            self.hover = id;
            cx.emit(ViewportEvent::Hover(id));
        }
    }

    fn submit(&mut self, window: &mut Window) {
        let size = self.bounds.size;
        let scale = window.scale_factor();
        self.scale = scale;
        let w = (f32::from(size.width) * scale).round() as u32;
        let h = (f32::from(size.height) * scale).round() as u32;
        if w < 2 || h < 2 {
            return;
        }
        // During continuous navigation skip the pick pass; it is rendered once
        // the interaction settles.
        let navigating = matches!(self.drag, Drag::Rotate | Drag::Pan) || self.anim.is_some();
        let pick = self.pick_dirty && !navigating;
        if pick {
            self.pick_dirty = false;
        }
        self.seq += 1;
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

    fn present(&mut self, seq: u64, result: Result<(Frame, Arc<RenderImage>), String>, render_ms: f32, window: &mut Window, cx: &mut Context<Self>) {
        if seq < self.last_presented {
            return;
        }
        self.last_presented = seq;
        match result {
            Ok((frame, image)) => {
                if let Some(p) = frame.pick {
                    self.pick = Some(p);
                    let pos = self.last_pos;
                    let id = self.pick_at(pos);
                    self.set_hover(id, cx);
                }
                if let Some(old) = self.image.replace(image) {
                    let _ = window.drop_image(old);
                }
                self.stats.last_frame_ms = render_ms;
                self.stats.frames += 1;
            }
            Err(e) => log::error!("render failed: {e}"),
        }
        cx.notify();
    }

    // ------------------------------------------------------------ input

    fn on_mouse_down(&mut self, e: &MouseDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        self.press_pos = p;
        self.last_pos = p;
        self.anim = None;
        match e.button {
            MouseButton::Left if self.tool_captures_left || self.manipulator_at(p) => {
                self.drag = Drag::Tool;
                cx.emit(ViewportEvent::ToolDown { position: p, modifiers: e.modifiers, click_count: e.click_count });
            }
            MouseButton::Left | MouseButton::Right | MouseButton::Middle => {
                self.drag = Drag::Pending(e.button);
            }
            _ => {}
        }
        cx.stop_propagation();
    }

    fn on_mouse_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        let delta = p - self.last_pos;
        self.last_pos = p;
        let height = self.size().y;
        if let Drag::Pending(button) = self.drag {
            if p.distance(self.press_pos) > 3.0 {
                self.drag = match button {
                    MouseButton::Right if e.modifiers.control || e.modifiers.shift => Drag::Pan,
                    MouseButton::Right => Drag::Rotate,
                    MouseButton::Middle => Drag::Pan,
                    MouseButton::Left => {
                        if self.pick_at(self.press_pos).is_some() && !e.modifiers.shift {
                            // Dragging from geometry with no tool: treat as box select too
                            // (Onshape only drags geometry inside sketches/assemblies).
                            Drag::Box
                        } else {
                            Drag::Box
                        }
                    }
                    _ => Drag::None,
                };
            }
        }
        match self.drag {
            Drag::Rotate => {
                self.camera.orbit(delta, height);
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
                cx.emit(ViewportEvent::ToolDrag { position: p, modifiers: e.modifiers });
            }
            Drag::None | Drag::Pending(_) => {
                let id = self.pick_at(p);
                self.set_hover(id, cx);
                cx.emit(ViewportEvent::PointerMove { position: p, modifiers: e.modifiers });
            }
        }
        let _ = window;
    }

    fn on_mouse_up(&mut self, e: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let p = self.local(e.position);
        match self.drag {
            Drag::Pending(MouseButton::Left) => {
                let pick = self.pick_at(p);
                cx.emit(ViewportEvent::Click { pick, position: p, modifiers: e.modifiers, click_count: e.click_count });
            }
            Drag::Pending(MouseButton::Right) => {
                let pick = self.pick_at(p);
                cx.emit(ViewportEvent::ContextMenu { pick, position: e.position });
            }
            Drag::Box => {
                if let Some((a, b)) = self.box_rect.take() {
                    let crossing = b.x < a.x;
                    let ids = if crossing { self.ids_in_rect(a, b) } else { self.ids_fully_inside(a, b) };
                    cx.emit(ViewportEvent::BoxSelect { ids, crossing, modifiers: e.modifiers });
                }
            }
            Drag::Tool => cx.emit(ViewportEvent::ToolUp { position: p, modifiers: e.modifiers }),
            Drag::Rotate | Drag::Pan => {
                // Settle: render the pick buffer now.
                self.pick_dirty = true;
                self.dirty = true;
            }
            _ => {}
        }
        self.drag = Drag::None;
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

impl Render for Viewport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Advance camera animation.
        if let Some(anim) = &self.anim {
            let t = anim.start.elapsed().as_secs_f64() / anim.duration.as_secs_f64();
            self.camera = anim.from.lerp(&anim.to, t);
            self.dirty = true;
            if t >= 1.0 {
                self.anim = None;
                self.pick_dirty = true;
                cx.emit(ViewportEvent::CameraChanged);
            } else {
                window.request_animation_frame();
            }
        }
        if self.dirty {
            self.submit(window);
        }

        let entity = cx.entity();
        let box_rect = self.box_rect;
        let image = self.image.clone();
        div()
            .id("viewport")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(0x16191f))
            .child(
                canvas(
                    move |bounds, _window, cx| {
                        entity.update(cx, |this, cx| {
                            if this.bounds != bounds {
                                this.bounds = bounds;
                                this.dirty = true;
                                this.pick_dirty = true;
                                cx.notify();
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
                let color = if crossing { rgb(0x4ade80) } else { rgb(0x4cc9f0) };
                div()
                    .absolute()
                    .left(px(x0))
                    .top(px(y0))
                    .w(px(w))
                    .h(px(h))
                    .border_1()
                    .when(crossing, |d| d.border_dashed())
                    .border_color(color)
                    .bg(color.opacity(0.08))
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
    }
}
