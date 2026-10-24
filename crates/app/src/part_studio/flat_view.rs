//! The flat view of the Sheet metal table and flat view panel (live "qa5 sm"
//! (8), qa5/sm-091, sm-104; dark/dk-973): its own graphics area under the
//! tables. The flat pattern is a shaded solid in the world's XY plane (the
//! reference side up at z 0, as REST's isFlattenedBody), seen from Top; the
//! bend lines are dark centre lines; a View cube without the camera menu;
//! Measure and Analysis tools at the bottom right.

use super::view_cube::{CubeCell, CubeHooks, view_cube};
use crate::gpu::Gpu;
use crate::viewport::{Viewport, ViewportEvent};
use gpui_kit::*;
use noshape_model::sheetmetal::FlatPattern;
use noshape_render::{Drawable, Instance, LineData, LineSegment, Scene, StandardView};
use std::sync::Arc;

pub struct FlatView {
    gpu: Gpu,
    pub viewport: Entity<Viewport>,
    cube_hover: Option<CubeCell>,
    cube_origin: Option<Point<Pixels>>,
    /// The pattern shown (by identity) and its highlighted bend: a change
    /// rebuilds the scene.
    shown: Option<(usize, Option<usize>)>,
    /// Keep the Top view fitted while the view's size settles (until the
    /// user moves the camera), and the size it was last fitted at.
    pending_fit: bool,
    fit_size: glam::DVec2,
    /// The camera our last fit set (a change from it is the user's).
    fitted: Option<noshape_render::Camera>,
    _sub: Subscription,
}

/// What the flat view asks of its Part Studio.
pub enum FlatViewEvent {
    /// Right-click at this window position, on the flat or not.
    ContextMenu(Point<Pixels>, bool),
}

impl EventEmitter<FlatViewEvent> for FlatView {}

impl FlatView {
    pub fn new(gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewport = cx.new(|cx| Viewport::new(gpu.clone(), window, cx));
        let sub = cx.subscribe(&viewport, |this: &mut Self, _, ev: &ViewportEvent, cx| match ev {
            ViewportEvent::ContextMenu { position, pick } => cx.emit(FlatViewEvent::ContextMenu(*position, pick.is_some())),
            // The user zoomed, panned or turned the flat: keep their view.
            ViewportEvent::CameraChanged if this.fitted.is_some_and(|c| c != this.viewport.read(cx).camera) => this.pending_fit = false,
            _ => {}
        });
        Self { gpu, viewport, cube_hover: None, cube_origin: None, shown: None, pending_fit: false, fit_size: glam::DVec2::ZERO, fitted: None, _sub: sub }
    }

    /// Show `flat` in `color` (linear RGBA, the part's). The first pattern
    /// fits the Top view; later ones keep the camera (live: editing the
    /// model keeps the flat view's zoom).
    pub fn show(&mut self, flat: Option<&Arc<FlatPattern>>, color: [f32; 4], highlight: Option<usize>, cx: &mut Context<Self>) {
        let key = flat.map(|f| (Arc::as_ptr(f) as usize, highlight));
        if key == self.shown {
            return;
        }
        self.pending_fit |= self.shown.is_none();
        self.shown = key;
        let scene = flat.map(|f| flat_scene(f, color, highlight, &self.gpu)).unwrap_or_default();
        self.viewport.update(cx, |vp, cx| vp.set_scene(Arc::new(scene), true, cx));
        cx.notify();
    }

    pub fn isometric(&mut self, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| vp.animate_to_view(StandardView::Isometric, cx));
    }

    pub fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
    }

    fn cube_click(&mut self, cell: CubeCell, cx: &mut Context<Self>) {
        let rot = cell.rotation();
        self.viewport.update(cx, |vp, cx| vp.animate_to_rotation_fit(rot, cx));
    }
}

/// The flat solid and its bend lines.
fn flat_scene(flat: &FlatPattern, color: [f32; 4], highlight: Option<usize>, gpu: &Gpu) -> Scene {
    let Some(display) = noshape_model::regen::flat_view_display(flat, color) else { return Scene::default() };
    let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
    let mut scene = crate::scene_builder::build(&display, &opts, &mut Default::default(), gpu.uploader()).scene;
    // Live (sm-105, 4x): dark centre lines through the bend zones, over the
    // flat's top (the reference side, z 0).
    let mut lines = LineData::default();
    for (a, b) in flat.flat_view_bend_lines() {
        lines.push_polyline(&[a.as_vec3().to_array(), b.as_vec3().to_array()], [0x33, 0x33, 0x33, 0xff], 0, 1.0, LineSegment::DASH_DOT);
    }
    if !lines.segments.is_empty() {
        scene.instances.push(Instance::new(Drawable::Lines(gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().layer(12));
    }
    // The selected bend's strip (its developed length wide) over the top.
    for l in flat.bend_lines.iter().filter(|l| Some(l.bend) == highlight) {
        let d = (l.b - l.a).normalize_or_zero();
        let n = glam::DVec2::new(-d.y, d.x) * (l.allowance / 2.0);
        let z = 1e-5;
        let p = |q: glam::DVec2| [q.x as f32, q.y as f32, z];
        let positions = [p(l.a - n), p(l.b - n), p(l.b + n), p(l.a + n)];
        let mesh = gpu.uploader().upload_mesh_parts(&positions, &[[0.0, 0.0, 1.0]; 4], &[0; 4], &[0, 1, 2, 0, 2, 3]);
        scene.instances.push(Instance::new(Drawable::Mesh(mesh)).with_color(noshape_scene::palette::lin(noshape_scene::palette::viewport::SELECTION)).unpickable().layer(11));
    }
    scene
}

impl Render for FlatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hover = self.cube_hover;
        self.viewport.update(cx, |vp, cx| vp.set_view_cube(Some(hover), cx));
        let hooks = CubeHooks {
            state: |this: &mut Self| (&mut this.cube_hover, &mut this.cube_origin),
            rotation: |this: &Self, cx| this.viewport.read(cx).camera.rotation,
            click: Self::cube_click,
            rotate: |this, dx, dy, cx| {
                let (dir, deg) = crate::navigation::step_from_degrees(dx, dy);
                this.viewport.update(cx, |vp, cx| vp.rotate_step(dir, deg, cx));
            },
            roll: |this, deg, cx| {
                let dir = if deg > 0.0 { crate::navigation::RotateDir::CounterClockwise } else { crate::navigation::RotateDir::Clockwise };
                this.viewport.update(cx, |vp, cx| vp.rotate_step(dir, deg.abs(), cx));
            },
            placed: |this, at, cx| this.viewport.update(cx, |vp, cx| vp.set_view_cube_origin(at, cx)),
        };
        let size = self.viewport.read(cx).size();
        if self.pending_fit && size.x > 0.0 && size.y > 0.0 && size != self.fit_size {
            self.fit_size = size;
            self.fitted = Some(self.viewport.update(cx, |vp, cx| {
                vp.camera.rotation = noshape_render::Camera::rotation_for(StandardView::Top);
                vp.zoom_to_fit(false, cx);
                vp.camera
            }));
        } else if self.pending_fit && size.x <= 0.0 {
            // Not laid out yet: look again next frame.
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        }
        // No camera menu under this cube (live sm-104).
        let cube = view_cube(div().into_any_element(), hooks, size, cx);
        div()
            .size_full()
            .relative()
            .child(self.viewport.clone())
            .child(div().absolute().top(px(15.)).right(px(15.)).child(cube))
    }
}
