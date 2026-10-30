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
use noshape_model::display::StudioDisplay;
use noshape_model::reference::Reference;
use noshape_model::sheetmetal::FlatPattern;
use noshape_render::{Drawable, Instance, LineData, LineSegment, Scene, StandardView};
use glam::DVec3;
use std::sync::Arc;

pub struct FlatView {
    gpu: Gpu,
    pub viewport: Entity<Viewport>,
    cube_hover: Option<CubeCell>,
    cube_origin: Option<Point<Pixels>>,
    /// The pattern shown (by identity) and its highlighted bend: a change
    /// rebuilds the scene.
    shown: Option<(usize, Option<usize>, Option<usize>)>,
    /// The pattern shown and its highlighted (table-selected) bend.
    flat: Option<(Arc<FlatPattern>, Option<usize>)>,
    /// The flat's display, its built scene and the overlays (bend lines,
    /// highlights), and the face under the pointer.
    parts: Option<(StudioDisplay, crate::scene_builder::BuiltScene, Vec<Instance>)>,
    hover: Option<Reference>,
    /// Each flat face's side: the reference side (the top, z 0), the back,
    /// or `None` (an edge face); and the side selected.
    sides: Vec<Option<bool>>,
    selected: Option<bool>,
    /// The bend line selected (index into the pattern's bend lines).
    selected_bend: Option<usize>,
    /// The open sketch's frame on the flat, its name's top-left and in-plane
    /// x / y directions, and the name.
    sketch_frame: Option<SketchFrame>,
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
    /// Right-click at this window position: on the flat's face, on a bend
    /// line (or anywhere while one is selected: live request 1605), or not.
    ContextMenu(Point<Pixels>, FlatPick),
    /// The flat's face selected (live request 1605: picking it selects the
    /// model face): its reference side (`true`), its back, or nothing.
    Selected(Option<bool>),
    /// A bend line selected (the model's bend index), or none.
    BendSelected(Option<usize>),
}

/// An open sketch's box on the flat (live request 1605: New sketch… on the
/// flat's face shows the plane's frame, named, in the flat view).
#[derive(Clone, Debug, PartialEq)]
pub struct SketchFrame {
    pub corners: [DVec3; 4],
    pub name_at: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub name: String,
}

/// What a right-click in the flat view is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatPick {
    Empty,
    Face,
    Bend(usize),
}

impl EventEmitter<FlatViewEvent> for FlatView {}

impl FlatView {
    pub fn new(gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewport = cx.new(|cx| Viewport::new(gpu.clone(), window, cx));
        let sub = cx.subscribe(&viewport, |this: &mut Self, _, ev: &ViewportEvent, cx| match ev {
            // Live (4a): the face under the pointer outlined in the
            // pre-select colour.
            ViewportEvent::Hover(id) => {
                let r = id.and_then(|id| this.parts.as_ref()?.1.picks.get(id).map(|t| t.reference.clone())).filter(|r| matches!(r, Reference::Face(_)));
                if r != this.hover {
                    this.hover = r;
                    this.compose(false, cx);
                }
            }
            ViewportEvent::ContextMenu { position, pick } => {
                let o = this.viewport.read(cx).origin();
                let local = glam::DVec2::new(f64::from(f32::from(position.x - o.x)), f64::from(f32::from(position.y - o.y)));
                let what = match (this.bend_at(local, cx).or(this.selected_bend), pick) {
                    (Some(b), _) => FlatPick::Bend(b),
                    (None, Some(_)) => FlatPick::Face,
                    (None, None) => FlatPick::Empty,
                };
                cx.emit(FlatViewEvent::ContextMenu(*position, what));
            }
            ViewportEvent::Click { pick, position, .. } => {
                // A bend line (live: it highlights, the face doesn't), else the face.
                if let Some(b) = this.bend_at(*position, cx) {
                    this.selected_bend = if this.selected_bend == Some(b) { None } else { Some(b) };
                    this.selected = None;
                    this.compose(false, cx);
                    cx.emit(FlatViewEvent::Selected(None));
                    cx.emit(FlatViewEvent::BendSelected(this.selected_bend.and_then(|l| this.bend_of_line(l))));
                    return;
                }
                if this.selected_bend.take().is_some() {
                    cx.emit(FlatViewEvent::BendSelected(None));
                }
                let side = pick.and_then(|id| this.face_side(id));
                this.select(if side.is_some() && side == this.selected { None } else { side }, cx);
                this.compose(false, cx);
                cx.emit(FlatViewEvent::Selected(this.selected));
            }
            // The user zoomed, panned or turned the flat: keep their view.
            ViewportEvent::CameraChanged if this.fitted.is_some_and(|c| c != this.viewport.read(cx).camera) => this.pending_fit = false,
            _ => {}
        });
        Self { gpu, viewport, cube_hover: None, cube_origin: None, shown: None, flat: None, parts: None, hover: None, sides: Vec::new(), selected: None, selected_bend: None, sketch_frame: None, pending_fit: false, fit_size: glam::DVec2::ZERO, fitted: None, _sub: sub }
    }

    /// Show `flat` in `color` (linear RGBA, the part's). The first pattern
    /// fits the Top view; later ones keep the camera (live: editing the
    /// model keeps the flat view's zoom).
    pub fn show(&mut self, flat: Option<&Arc<FlatPattern>>, color: [f32; 4], highlight: Option<usize>, hovered: Option<usize>, cx: &mut Context<Self>) {
        let key = flat.map(|f| (Arc::as_ptr(f) as usize, highlight, hovered));
        if key == self.shown {
            return;
        }
        self.pending_fit |= self.shown.is_none();
        self.shown = key;
        self.flat = flat.map(|f| (f.clone(), highlight));
        self.parts = flat.and_then(|f| flat_parts(f, color, highlight, hovered, &self.gpu));
        self.sides = flat.and_then(|f| f.flat_view_solid()).map(|s| s.faces().iter().map(|f| f.face_plane().ok().flatten().map(|p| p.unit_normal().z)).map(|z| z.filter(|z| z.abs() > 0.5).map(|z| z > 0.0)).collect()).unwrap_or_default();
        self.hover = None;
        self.compose(true, cx);
        cx.notify();
    }

    /// The open sketch's frame on the flat, or none.
    pub fn show_sketch_frame(&mut self, frame: Option<SketchFrame>, cx: &mut Context<Self>) {
        if frame != self.sketch_frame {
            self.sketch_frame = frame;
            self.compose(false, cx);
        }
    }

    /// The bend line within 5 px of a viewport-local point.
    fn bend_at(&self, p: glam::DVec2, cx: &App) -> Option<usize> {
        let (f, _) = self.flat.as_ref()?;
        let vp = self.viewport.read(cx);
        f.flat_view_bend_lines()
            .iter()
            .enumerate()
            .filter_map(|(i, (a, b))| {
                let (a, b) = (vp.project(*a)?, vp.project(*b)?);
                let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-12)).clamp(0.0, 1.0);
                Some((i, (a + (b - a) * t).distance(p)))
            })
            .filter(|x| x.1 <= 5.0)
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|x| x.0)
    }

    /// The model's bend (index into its bends) of bend line `line`.
    pub fn bend_of_line(&self, line: usize) -> Option<usize> {
        self.flat.as_ref()?.0.bend_lines.get(line).map(|l| l.bend)
    }

    /// Select bend line `line` (or none), the face unselected.
    pub fn select_bend(&mut self, line: Option<usize>, cx: &mut Context<Self>) {
        self.selected_bend = line;
        self.selected = None;
        self.compose(false, cx);
    }

    /// Align view with vertical axis: the selected bend line upright.
    pub fn align_bend_vertical(&mut self, bend: usize, cx: &mut Context<Self>) {
        let Some((a, b)) = self.flat.as_ref().and_then(|(f, _)| f.flat_view_bend_lines().get(bend).copied()) else { return };
        let Some((fwd, up)) = noshape_scene::camera_ops::align_vertical(&self.viewport.read(cx).camera, (b - a).normalize_or_zero()) else { return };
        self.viewport.update(cx, |vp, cx| vp.animate_to_direction(fwd, up, cx));
    }

    /// The side of the flat face a pick id is on (`None`: not a top / back face).
    fn face_side(&self, id: u32) -> Option<bool> {
        let r = self.parts.as_ref()?.1.picks.get(id)?.reference.clone();
        let Reference::Face(n) = r else { return None };
        let noshape_model::reference::TopoName::Face { role: noshape_model::reference::Role::New(i), .. } = &n else { return None };
        self.sides.get(*i as usize).copied().flatten()
    }

    /// Select the flat's reference-side (`true`) or back face, or nothing.
    pub fn select(&mut self, side: Option<bool>, cx: &mut Context<Self>) {
        if side != self.selected {
            self.selected = side;
            self.compose(false, cx);
        }
    }

    /// The scene: the flat with the hovered face outlined, and the overlays.
    fn compose(&mut self, geometry_changed: bool, cx: &mut Context<Self>) {
        let selected: Vec<Reference> = self.selected.and_then(|side| self.sides.iter().position(|s| *s == Some(side))).map(|i| Reference::Face(noshape_model::reference::TopoName::face(noshape_model::reference::FeatureId(0), noshape_model::reference::Role::New(i as u32)))).into_iter().collect();
        let mut extra_now = self.parts.as_ref().map(|p| p.2.clone()).unwrap_or_default();
        // The selected bend line in the selection colour (live: orange).
        if let Some((a, b)) = self.selected_bend.and_then(|i| self.flat.as_ref().and_then(|(f, _)| f.flat_view_bend_lines().get(i).copied())) {
            let mut lines = LineData::default();
            let [_, r, g, b8] = noshape_scene::palette::viewport::SELECTION.to_be_bytes();
            lines.push_polyline(&[a.as_vec3().to_array(), b.as_vec3().to_array()], [r, g, b8, 0xff], 0, 3.0, LineSegment::ON_TOP);
            extra_now.push(Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().layer(14));
        }
        // The open sketch's frame and name, in the 3D view's sketch box colour.
        let [_, r, g, b8] = noshape_scene::sketch_mode::SKETCH_BOX_COLOR.to_be_bytes();
        let mut fills = Vec::new();
        if let Some(sf) = &self.sketch_frame {
            let mut lines = LineData::default();
            let mut ring: Vec<[f32; 3]> = sf.corners.iter().map(|c| c.as_vec3().to_array()).collect();
            ring.push(ring[0]);
            lines.push_polyline(&ring, [r, g, b8, 0xff], 0, 1.0, LineSegment::ON_TOP);
            extra_now.push(Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().layer(14));
            // The name at its top-left, its height a twentieth of the frame's.
            let asc = 0.045 * sf.corners[0].distance(sf.corners[3]);
            let mut contours = Vec::new();
            super::in_plane_contours(&sf.name, sf.name_at, sf.x, sf.y, asc, &mut contours);
            let c = gpui_kit::Rgba::from(gpui_kit::Hsla::from(gpui_kit::rgb(noshape_scene::sketch_mode::SKETCH_BOX_COLOR)));
            fills.push(noshape_render::overlay::WorldFill { contours: Arc::new(contours), color: [c.r, c.g, c.b, c.a] });
        }
        self.viewport.update(cx, |vp, cx| vp.set_plane_labels(fills, cx));
        let scene = match &self.parts {
            Some((display, built, _)) => crate::scene_builder::with_highlights(built, display, &selected, self.hover.as_ref(), &[], extra_now, false, self.gpu.uploader()),
            None => Scene::default(),
        };
        self.viewport.update(cx, |vp, cx| vp.set_scene(Arc::new(scene), geometry_changed, cx));
    }

    pub fn top(&mut self, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| vp.animate_to_view(StandardView::Top, cx));
    }

    pub fn isometric(&mut self, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| vp.animate_to_view(StandardView::Isometric, cx));
    }

    pub fn zoom_to_fit(&mut self, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
    }

    /// Zoom to selection: the bend selected in the table (its strip), else
    /// the whole flat.
    pub fn zoom_to_selection(&mut self, cx: &mut Context<Self>) {
        let strip = self.flat.as_ref().and_then(|(f, bend)| {
            let l = match self.selected_bend {
                Some(i) => f.bend_lines.get(i)?,
                None => f.bend_lines.iter().find(|l| Some(l.bend) == *bend)?,
            };
            let d = (l.b - l.a).normalize_or_zero();
            let n = glam::DVec2::new(-d.y, d.x) * (l.allowance / 2.0);
            let pts = [l.a - n, l.a + n, l.b - n, l.b + n];
            let (lo, hi) = pts.iter().fold((glam::DVec2::splat(f64::INFINITY), glam::DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
            Some((lo.extend(-f.thickness), hi.extend(0.0)))
        });
        match strip {
            Some((lo, hi)) => self.viewport.update(cx, |vp, cx| vp.zoom_to_bounds(lo, hi, cx)),
            None => self.zoom_to_fit(cx),
        }
    }

    fn cube_click(&mut self, cell: CubeCell, cx: &mut Context<Self>) {
        let rot = cell.rotation();
        self.viewport.update(cx, |vp, cx| vp.animate_to_rotation_fit(rot, cx));
    }
}

/// The flat's display and scene, and its overlays: the bend lines, the
/// selected bend's strip and the hovered row's outline.
fn flat_parts(flat: &FlatPattern, color: [f32; 4], highlight: Option<usize>, hovered: Option<usize>, gpu: &Gpu) -> Option<(StudioDisplay, crate::scene_builder::BuiltScene, Vec<Instance>)> {
    let display = noshape_model::regen::flat_view_display(flat, color)?;
    let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
    let built = crate::scene_builder::build(&display, &opts, &mut Default::default(), gpu.uploader());
    let mut extra: Vec<Instance> = Vec::new();
    // Live (sm-105, 4x): dark centre lines through the bend zones, over the
    // flat's top (the reference side, z 0).
    let mut lines = LineData::default();
    for (a, b) in flat.flat_view_bend_lines() {
        lines.push_polyline(&[a.as_vec3().to_array(), b.as_vec3().to_array()], [0x33, 0x33, 0x33, 0xff], 0, 1.0, LineSegment::DASH_DOT);
    }
    if !lines.segments.is_empty() {
        extra.push(Instance::new(Drawable::Lines(gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().layer(12));
    }
    // The hovered row's bend zone outlined in the pre-select colour
    // #FFC685 (live ui-sm4b-2: about 3 px).
    let mut outline = LineData::default();
    for l in flat.bend_lines.iter().filter(|l| Some(l.bend) == hovered) {
        let d = (l.b - l.a).normalize_or_zero();
        let n = glam::DVec2::new(-d.y, d.x) * (l.allowance / 2.0);
        let p = |q: glam::DVec2| [q.x as f32, q.y as f32, 2e-5];
        outline.push_polyline(&[p(l.a - n), p(l.b - n), p(l.b + n), p(l.a + n), p(l.a - n)], [0xff, 0xc6, 0x85, 0xff], 0, 3.0, LineSegment::ON_TOP);
    }
    if !outline.segments.is_empty() {
        extra.push(Instance::new(Drawable::Lines(gpu.uploader().upload_lines(&outline))).per_primitive_color().unpickable().layer(13));
    }
    // The selected bend's strip (its developed length wide) over the top.
    for l in flat.bend_lines.iter().filter(|l| Some(l.bend) == highlight) {
        let d = (l.b - l.a).normalize_or_zero();
        let n = glam::DVec2::new(-d.y, d.x) * (l.allowance / 2.0);
        let z = 1e-5;
        let p = |q: glam::DVec2| [q.x as f32, q.y as f32, z];
        let positions = [p(l.a - n), p(l.b - n), p(l.b + n), p(l.a + n)];
        let mesh = gpu.uploader().upload_mesh_parts(&positions, &[[0.0, 0.0, 1.0]; 4], &[0; 4], &[0, 1, 2, 0, 2, 3]);
        extra.push(Instance::new(Drawable::Mesh(mesh)).with_color(noshape_scene::palette::lin(noshape_scene::palette::viewport::SELECTION)).unpickable().layer(11));
    }
    Some((display, built, extra))
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
