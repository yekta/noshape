//! The hover × selection look matrix (user 11-21, DECISIONS "hover and selection looks"): each target in each state is
//! drawn as live's crop of the same cell (live/psui/hover-matrix-1121: a 40 × 30 × 20 mm block, default iso,
//! 0.2845 mm/px, device pixel ratio 1, 80 px crops centred on the target).

use super::feel::{Pointer, gpu_pointer};
use super::*;
use glam::DVec3;

/// Live's camera for the matrix: orthographic, looking along (-0.433, 0.75, -0.5), 0.2845 mm per px.
fn live_camera(height_px: f64) -> noshape_render::Camera {
    let mut cam = noshape_render::Camera::default();
    let forward = DVec3::new(-0.433, 0.75, -0.5).normalize();
    let up = (DVec3::Z - forward * DVec3::Z.dot(forward)).normalize();
    cam.rotation = noshape_render::Camera::look_rotation(forward, up);
    cam.target = DVec3::new(0.02, 0.015, 0.01);
    cam.projection = noshape_render::Projection::Orthographic;
    // world_per_pixel = 2 · distance · tan(fov / 2) / height.
    cam.distance = 0.2845e-3 * height_px / (2.0 * (cam.fov_y * 0.5).tan());
    cam
}

pub(super) struct Matrix {
    pub window: AnyWindowHandle,
    pub view: Entity<PartStudioView>,
    pub pointer: Pointer,
}

/// Live's matrix document in a rendering test window at a device pixel ratio of 1, with live's camera.
pub(super) fn matrix_studio(cx: &mut TestAppContext, name: &str) -> Matrix {
    matrix_studio_with(cx, name, false)
}

/// [`matrix_studio`], with batch 3's Mate connector 1 on the block's top back right corner when `connector`.
pub(super) fn matrix_studio_with(cx: &mut TestAppContext, name: &str, connector: bool) -> Matrix {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        // Batch 2's: a point, a line, an arc from 0.3 to 2.8 rad and a circle.
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [
            { "point": [[60, 25]] },
            { "line": [[60, 5], [90, 5]] },
            { "center_point_arc": [[75, 15], [75. + 8. * 0.3f64.cos(), 15. + 8. * 0.3f64.sin()], [75. + 8. * 2.8f64.cos(), 15. + 8. * 2.8f64.sin()]] },
            { "circle": [[100, 20], [106, 20]] }
        ] }
    ] });
    let mut start = start;
    if connector {
        let mc = serde_json::json!({ "type": "mateConnector", "name": "Mate connector 1", "params": { "originQuery": ["vertex through [40, 30, 20]"] } });
        start["features"].as_array_mut().expect("features").push(mc);
    }
    matrix_studio_from(cx, name, start)
}

/// A rendering test window on `start` at a device pixel ratio of 1, with the matrix's live camera.
pub(super) fn matrix_studio_from(cx: &mut TestAppContext, name: &str, start: serde_json::Value) -> Matrix {
    RENDERING.set(true);
    let (window, view) = part_studio_from(cx, name, Some(start));
    cx.update_window(window, |_, w, _| w.set_scale_factor(1.0)).unwrap();
    draw(cx, window);
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().any(|b| b.kind == noshape_model::display::BodyKind::Solid));
    cx.update(|cx| {
        let vp = view.read(cx).viewport.clone();
        vp.update(cx, |vp, cx| {
            vp.camera = live_camera(vp.size().y);
            vp.request_frame(cx);
        });
    });
    let pointer = Pointer::new(cx, window, &view);
    pointer.frames(cx, &view);
    Matrix { window, view, pointer }
}

impl Matrix {
    pub fn screen(&self, cx: &mut TestAppContext, mm: [f64; 3]) -> DVec2 {
        self.exact(cx, mm).round()
    }

    /// Where a model point shows, unrounded (where the ui-parity runner puts the pointer).
    pub fn exact(&self, cx: &mut TestAppContext, mm: [f64; 3]) -> DVec2 {
        cx.update(|cx| self.view.read(cx).viewport.read(cx).project(DVec3::from(mm) * 1e-3).expect("on screen"))
    }

    /// Nothing selected, the pointer parked over the background.
    pub fn reset(&self, cx: &mut TestAppContext) {
        self.view.update(cx, |v, cx| {
            v.selection.clear();
            if let Some(ed) = v.sketch.as_mut() {
                ed.selection.clear();
                ed.selected_regions.clear();
                ed.selected_constraints.clear();
                ed.model_selection.clear();
            }
            v.refresh_scene(false, cx);
        });
        self.park(cx);
    }

    /// Sketch 2 opened for editing (live: its row double-clicked), live's camera kept.
    pub fn edit_sketch(&self, cx: &mut TestAppContext) {
        let id = cx.update(|cx| self.view.read(cx).display.sketches.iter().find(|s| s.name == "Sketch 2").map(|s| s.feature).expect("Sketch 2"));
        cx.update_window(self.window, |_, w, cx| self.view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
        wait_until(cx, self.window, |cx| self.view.read(cx).sketch.is_some() && !self.view.read(cx).ui_busy());
        cx.update(|cx| {
            let vp = self.view.read(cx).viewport.clone();
            vp.update(cx, |vp, cx| {
                vp.camera = live_camera(vp.size().y);
                vp.request_frame(cx);
            });
        });
        self.pointer.frames(cx, &self.view);
    }

    pub fn park(&self, cx: &mut TestAppContext) {
        self.pointer.move_to(cx, &self.view, DVec2::new(30.0, 30.0));
    }

    /// The frame as drawn now, 80 × 80 px centred on `at` (row-major RGB).
    pub fn crop(&self, cx: &mut TestAppContext, at: DVec2) -> Vec<[u8; 3]> {
        let (w, h, px) = self.frame(cx);
        let (x0, y0) = (at.x as i32 - 40, at.y as i32 - 40);
        let mut out = Vec::with_capacity(6400);
        for y in y0..y0 + 80 {
            for x in x0..x0 + 80 {
                let (cx_, cy_) = (x.clamp(0, w as i32 - 1), y.clamp(0, h as i32 - 1));
                out.push(px[(cy_ * w as i32 + cx_) as usize]);
            }
        }
        out
    }

    /// The whole frame as drawn now: width, height and its pixels (row-major RGB).
    pub fn frame(&self, cx: &mut TestAppContext) -> (u32, u32, Vec<[u8; 3]>) {
        self.pointer.frames(cx, &self.view);
        let (scene, camera, style, size, geometry) = cx.update(|cx| {
            let vp = self.view.read(cx).viewport.read(cx);
            let (_, scene, camera, style) = vp.print_source();
            (scene, camera, style, vp.size(), vp.camera_geometry())
        });
        let (w, h) = (size.x as u32, size.y as u32);
        let mut r = noshape_render::Renderer::new().expect("a renderer");
        // Mate connector glyphs and manipulators are built per frame from the camera.
        r.set_camera_geometry(noshape_render::renderer::SYNC_TARGET, geometry);
        let f = r.render(noshape_render::RenderParams { scene: &scene, camera: &camera, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).expect("a frame");
        (f.width, f.height, f.bgra.as_chunks::<4>().0.iter().map(|p| [p[2], p[1], p[0]]).collect())
    }
}

/// Writes a crop as a binary PPM (scratch comparisons against live's crops; NOSHAPE_HOVER_MATRIX_DUMP names the folder).
fn dump(name: &str, crop: &[[u8; 3]]) {
    let Ok(dir) = std::env::var("NOSHAPE_HOVER_MATRIX_DUMP") else { return };
    let mut bytes = b"P6\n80 80\n255\n".to_vec();
    bytes.extend(crop.iter().flatten());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ppm")), bytes);
}

/// Batch 1, Part Studio: vertex, edge, edge midpoint, face and the Top plane in each state, crops for live's.
#[gpui::test]
fn part_studio_hover_matrix_crops(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "hover-matrix-ps");
    let targets: [(&str, [f64; 3]); 5] = [("vertex", [40., 0., 20.]), ("edge", [10., 0., 20.]), ("mid", [20., 0., 20.]), ("face", [12., 12., 20.]), ("plane", [-45., -45., 0.])];
    let other = m.screen(cx, [40., 15., 10.]);
    for (name, at) in targets {
        let p = m.screen(cx, at);
        m.reset(cx);
        dump(&format!("{name}-none"), &m.crop(cx, p));
        m.pointer.move_to(cx, &m.view, p);
        if name == "mid" {
            std::thread::sleep(std::time::Duration::from_millis(120));
            m.pointer.move_to(cx, &m.view, p);
        }
        dump(&format!("{name}-hovered"), &m.crop(cx, p));
        m.pointer.click(cx, &m.view, p);
        m.park(cx);
        dump(&format!("{name}-selected"), &m.crop(cx, p));
        m.pointer.move_to(cx, &m.view, p);
        dump(&format!("{name}-selected-hovered"), &m.crop(cx, p));
        m.reset(cx);
        m.pointer.move_to(cx, &m.view, other);
        m.pointer.click(cx, &m.view, other);
        m.park(cx);
        m.pointer.move_to(cx, &m.view, p);
        dump(&format!("{name}-hovered-other-selected"), &m.crop(cx, p));
    }
    // The midpoint arrived at along the edge, plain and selected.
    let (mid, along) = (m.screen(cx, [20., 0., 20.]), m.screen(cx, [14., 0., 20.]));
    for selected in [false, true] {
        m.reset(cx);
        if selected {
            m.pointer.move_to(cx, &m.view, along);
            m.pointer.click(cx, &m.view, along);
        }
        m.pointer.move_to(cx, &m.view, along);
        for k in 1..=6 {
            m.pointer.move_to(cx, &m.view, along + (mid - along) * (k as f64 / 6.0));
        }
        dump(if selected { "mid-arrived-on-selected-edge" } else { "mid-arrived-along-edge" }, &m.crop(cx, mid));
    }
    // Batch 2: Sketch 2's entities in the Part Studio (ps/sketch-*) and in sketch edit (sk/*); the other selection is
    // the circle's edge.
    let other = m.screen(cx, SKETCH_OTHER);
    for edit in [false, true] {
        if edit {
            m.reset(cx);
            m.edit_sketch(cx);
        }
        for (name, at) in SKETCH_TARGETS {
            let name = if edit { format!("sk-{name}") } else { format!("sketch-{name}") };
            let p = m.screen(cx, at);
            m.reset(cx);
            dump(&format!("{name}-none"), &m.crop(cx, p));
            approach(cx, &m, p);
            dump(&format!("{name}-hovered"), &m.crop(cx, p));
            m.pointer.click(cx, &m.view, p);
            m.park(cx);
            dump(&format!("{name}-selected"), &m.crop(cx, p));
            approach(cx, &m, p);
            dump(&format!("{name}-selected-hovered"), &m.crop(cx, p));
            m.reset(cx);
            m.pointer.move_to(cx, &m.view, other);
            m.pointer.click(cx, &m.view, other);
            m.park(cx);
            approach(cx, &m, p);
            dump(&format!("{name}-hovered-other-selected"), &m.crop(cx, p));
        }
    }
    // The Line tool from (45, -10) along x: its length written in the sketch plane (live sk/inference-*-at).
    m.reset(cx);
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    let (from, to) = (m.screen(cx, [45., -10., 0.]), m.screen(cx, [60., -10., 0.]));
    m.pointer.move_to(cx, &m.view, from);
    m.pointer.click(cx, &m.view, from);
    for k in 1..=10 {
        m.pointer.move_to(cx, &m.view, from + (to - from) * (k as f64 / 10.0));
    }
    dump("sk-line-tool-length", &m.crop(cx, (from + to) * 0.5));
}

/// Batch 3 (live b3/): the origin at the block's corner, Mate connector 1, and Fillet 1's selection field (radius 1.3 mm,
/// as live's), each in its states; crops for live's.
#[gpui::test]
fn part_studio_hover_matrix_b3_crops(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio_with(cx, "hover-matrix-b3", true);
    wait_until(cx, m.window, |cx| !m.view.read(cx).display.connectors.is_empty());
    let other = m.screen(cx, [40., 15., 10.]);
    let hovered = |cx: &mut TestAppContext, m: &Matrix| cx.update(|cx| format!("{:?}", m.view.read(cx).hover));
    for (name, at) in [("ps-origin", [0., 0., 0.]), ("ps-mc", [40., 30., 20.])] {
        let p = m.screen(cx, at);
        m.reset(cx);
        dump(&format!("{name}-none"), &m.crop(cx, p));
        approach(cx, &m, p);
        eprintln!("b3 {name} hovered: {}", hovered(cx, &m));
        dump(&format!("{name}-hovered"), &m.crop(cx, p));
        m.pointer.click(cx, &m.view, p);
        m.park(cx);
        dump(&format!("{name}-selected"), &m.crop(cx, p));
        approach(cx, &m, p);
        dump(&format!("{name}-selected-hovered"), &m.crop(cx, p));
        m.reset(cx);
        m.pointer.move_to(cx, &m.view, other);
        m.pointer.click(cx, &m.view, other);
        m.park(cx);
        approach(cx, &m, p);
        dump(&format!("{name}-hovered-other-selected"), &m.crop(cx, p));
    }
    let open_fillet = |cx: &mut TestAppContext, m: &Matrix| {
        m.reset(cx);
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
        draw(cx, m.window);
        let dialog = cx.update(|cx| m.view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).expect("the Fillet dialog");
        cx.update_window(m.window, |_, w, cx| dialog.update(cx, |d, cx| d.set_expr_text("radius", "1.3 mm", w, cx))).unwrap();
        m.park(cx);
    };
    let close = |cx: &mut TestAppContext, m: &Matrix| {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
        wait_until(cx, m.window, |cx| m.view.read(cx).edit.is_none() && !m.view.read(cx).ui_busy());
    };
    let edge = m.screen(cx, [10., 0., 20.]);
    for (name, at) in [("dlg-edge", [10., 0., 20.]), ("dlg-mid", [20., 0., 20.]), ("dlg-vertex", [40., 0., 20.]), ("dlg-face", [12., 12., 20.])] {
        let p = m.screen(cx, at);
        open_fillet(cx, &m);
        dump(&format!("{name}-none"), &m.crop(cx, p));
        approach(cx, &m, p);
        eprintln!("b3 {name} hovered: {}", hovered(cx, &m));
        dump(&format!("{name}-hovered"), &m.crop(cx, p));
        close(cx, &m);
    }
    open_fillet(cx, &m);
    m.pointer.move_to(cx, &m.view, edge);
    m.pointer.click(cx, &m.view, edge);
    m.park(cx);
    dump("dlg-edge-selected", &m.crop(cx, edge));
    approach(cx, &m, edge);
    dump("dlg-edge-selected-hovered", &m.crop(cx, edge));
    let face = m.screen(cx, [12., 12., 20.]);
    approach(cx, &m, face);
    dump("dlg-face-hovered-other-selected", &m.crop(cx, face));
    close(cx, &m);
}

/// Sketch 2's targets (live index.md batch 2): its point, the line off its middle, the line's midpoint, the arc at 1.5 rad
/// (1.4 px from its midpoint) and the circle's region.
const SKETCH_TARGETS: [(&str, [f64; 3]); 5] = [("point", [60., 25., 0.]), ("line", [66., 5., 0.]), ("linemid", [75., 5., 0.]), ("arc", [75.5659, 22.9800, 0.]), ("region", [100., 20., 0.])];
/// The circle's edge, the other selection of batch 2.
const SKETCH_OTHER: [f64; 3] = [100., 26., 0.];

/// The pointer comes onto `p` in 2 px steps from 20 px straight below, as a hand does, and rests there.
fn approach(cx: &mut TestAppContext, m: &Matrix, p: DVec2) {
    for k in 0..=10 {
        m.pointer.move_to(cx, &m.view, p + DVec2::new(0.0, 20.0 - 2.0 * k as f64));
    }
    std::thread::sleep(std::time::Duration::from_millis(120));
    m.pointer.move_to(cx, &m.view, p);
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Nothing hovered or selected.
    Rest,
    Hovered,
    Selected,
    SelectedHovered,
    HoveredOtherSelected,
    /// Hovered with this point's entity selected (live ps-owner-matrix-1124: its owner or a part of it).
    HoveredWith([f64; 3]),
    /// The pointer comes along the edge from 6 mm away onto its midpoint.
    Arrived,
    ArrivedOnSelectedEdge,
    /// The pointer comes from 40 px away in 2 px steps, at this angle (degrees) to the edge's screen direction (live
    /// "Midpoint approach paths": 0 is along the edge from its far end's side).
    Path(f64),
}

/// A matrix cell drawn: the crop round the target in that state, in the Part Studio.
fn cell(cx: &mut TestAppContext, target: [f64; 3], state: State) -> Vec<[u8; 3]> {
    cell_in(cx, false, target, state)
}

/// [`cell`] with Sketch 2 being edited.
fn sketch_cell(cx: &mut TestAppContext, target: [f64; 3], state: State) -> Vec<[u8; 3]> {
    cell_in(cx, true, target, state)
}

fn cell_in(cx: &mut TestAppContext, edit: bool, target: [f64; 3], state: State) -> Vec<[u8; 3]> {
    let m = matrix_studio(cx, "hover-matrix-cell");
    if edit {
        m.edit_sketch(cx);
    }
    let p = m.screen(cx, target);
    let rest = |cx: &mut TestAppContext| {
        // The pointer rests on the target.
        std::thread::sleep(std::time::Duration::from_millis(120));
        m.pointer.move_to(cx, &m.view, p);
    };
    match state {
        State::Rest => m.park(cx),
        State::Hovered => {
            m.pointer.move_to(cx, &m.view, p);
            rest(cx);
        }
        State::Selected | State::SelectedHovered => {
            m.pointer.move_to(cx, &m.view, p);
            rest(cx);
            m.pointer.click(cx, &m.view, p);
            m.park(cx);
            if state == State::SelectedHovered {
                m.pointer.move_to(cx, &m.view, p);
            }
        }
        State::HoveredOtherSelected => {
            let other = m.screen(cx, if target[0] > 50. { SKETCH_OTHER } else { [40., 15., 10.] });
            m.pointer.move_to(cx, &m.view, other);
            m.pointer.click(cx, &m.view, other);
            m.park(cx);
            m.pointer.move_to(cx, &m.view, p);
            rest(cx);
        }
        State::HoveredWith(at) => {
            let q = m.screen(cx, at);
            m.pointer.move_to(cx, &m.view, q);
            rest(cx);
            m.pointer.move_to(cx, &m.view, q);
            m.pointer.click(cx, &m.view, q);
            m.park(cx);
            m.pointer.move_to(cx, &m.view, p);
            rest(cx);
        }
        State::Arrived | State::ArrivedOnSelectedEdge => {
            let along = m.screen(cx, [target[0] - 6., target[1], target[2]]);
            m.pointer.move_to(cx, &m.view, along);
            if state == State::ArrivedOnSelectedEdge {
                m.pointer.click(cx, &m.view, along);
            }
            for k in 1..=6 {
                m.pointer.move_to(cx, &m.view, along + (p - along) * (k as f64 / 6.0));
            }
        }
        State::Path(deg) => {
            let d = (m.screen(cx, [target[0] + 10., target[1], target[2]]) - p).normalize();
            let r = deg.to_radians();
            let from = DVec2::new(d.x * r.cos() - d.y * r.sin(), d.x * r.sin() + d.y * r.cos()) * 40.0;
            for k in 0..=20 {
                m.pointer.move_to(cx, &m.view, (p + from * (1.0 - k as f64 / 20.0)).round());
            }
            rest(cx);
        }
    }
    m.crop(cx, p)
}

const VERTEX: [f64; 3] = [40., 0., 20.];
const EDGE: [f64; 3] = [10., 0., 20.];
const MID: [f64; 3] = [20., 0., 20.];
const FACE: [f64; 3] = [12., 12., 20.];
const SK_POINT: [f64; 3] = [60., 25., 0.];
const SK_CENTRE: [f64; 3] = [100., 20., 0.];
const SK_LINE: [f64; 3] = [66., 5., 0.];
const SK_LINEMID: [f64; 3] = [75., 5., 0.];
const SK_LINEEND: [f64; 3] = [90., 5., 0.];
const SK_ARC: [f64; 3] = [75.5659, 22.98, 0.];

/// Pixels of the crop within `r` px of its centre (the pointer's hotspot).
fn near_centre(crop: &[[u8; 3]], r: f64) -> impl Iterator<Item = [u8; 3]> + '_ {
    crop.iter().enumerate().filter(move |(i, _)| DVec2::new((i % 80) as f64 - 40.0, (i / 80) as f64 - 40.0).length() <= r).map(|(_, c)| *c)
}

fn close(c: [u8; 3], rgb: u32, tol: i32) -> bool {
    let w = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    c.iter().zip(w).all(|(a, b)| (*a as i32 - b as i32).abs() <= tol)
}

/// Whether a colour of live's (within 24 per channel) is drawn within `r` px of the target.
fn shows(crop: &[[u8; 3]], rgb: u32, r: f64) -> bool {
    near_centre(crop, r).any(|c| close(c, rgb, 24))
}

/// The share of the crop's pixels from `r0` to `r1` px off its centre close to `rgb` (a disc's size: all of its rim).
fn ring_of(crop: &[[u8; 3]], rgb: u32, r0: f64, r1: f64) -> f64 {
    let off = |i: usize| DVec2::new((i % 80) as f64 - 40.0, (i / 80) as f64 - 40.0).length();
    let ring: Vec<[u8; 3]> = crop.iter().enumerate().filter(|(i, _)| (r0..=r1).contains(&off(*i))).map(|(_, c)| *c).collect();
    ring.iter().filter(|c| close(**c, rgb, 24)).count() as f64 / ring.len().max(1) as f64
}

/// The edge's #ffc685 hover band past an 11 px point marker (7 to 12 px out; the marker's own disc is near its colour).
fn band_beside(crop: &[[u8; 3]]) -> bool {
    band_beside_of(crop, 0xffc685)
}

/// A band of `rgb` past an 11 px point marker.
fn band_beside_of(crop: &[[u8; 3]], rgb: u32) -> bool {
    crop.iter().enumerate().any(|(i, c)| (7.0..=12.0).contains(&DVec2::new((i % 80) as f64 - 40.0, (i / 80) as f64 - 40.0).length()) && close(*c, rgb, 16))
}

/// Near-black pixels within `r` px: a dark ring or edge line (the black edge runs on past an 11 px point marker).
fn dark(crop: &[[u8; 3]], r: f64) -> usize {
    near_centre(crop, r).filter(|c| c.iter().all(|&v| v < 60)).count()
}

/// One test per cell of live's batches (live/psui/hover-matrix-1121 index.md, ps/*.png, sk/*.png): the colours live draws at
/// the target in that state, and those it doesn't.
macro_rules! cells {
    () => {};
    ($name:ident: $target:expr, $state:expr => edit |$crop:ident| $check:block $($rest:tt)*) => {
        #[gpui::test]
        fn $name(cx: &mut TestAppContext) {
            let Some(_gpu) = gpu_pointer() else { return };
            let $crop = sketch_cell(cx, $target, $state);
            dump(concat!("cell-", stringify!($name)), &$crop);
            $check
        }
        cells! { $($rest)* }
    };
    ($name:ident: $target:expr, $state:expr => |$crop:ident| $check:block $($rest:tt)*) => {
        #[gpui::test]
        fn $name(cx: &mut TestAppContext) {
            let Some(_gpu) = gpu_pointer() else { return };
            let $crop = cell(cx, $target, $state);
            dump(concat!("cell-", stringify!($name)), &$crop);
            $check
        }
        cells! { $($rest)* }
    };
}

cells! {
    // A vertex: an 11 px see-through disc, light orange hovered (#f6c285), #f6a612 selected; hovered once selected
    // it stays the selected disc.
    vertex_hovered: VERTEX, State::Hovered => |c| {
        assert!(shows(&c, 0xf6c285, 6.0) && !shows(&c, 0xf6a612, 6.0));
    }
    vertex_selected: VERTEX, State::Selected => |c| {
        assert!(shows(&c, 0xf6a612, 6.0) && !shows(&c, 0xf6c285, 6.0));
    }
    vertex_selected_hovered: VERTEX, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xf6a612, 6.0) && !shows(&c, 0xf6c285, 6.0));
    }
    // (The selected right face's #ffa90a boundary runs through the vertex.)
    vertex_hovered_other_selected: VERTEX, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xf6c285, 6.0));
    }
    // An edge: a 5 px band, #ffc685 hovered with the midpoint's dark marker, #ffa90a selected.
    edge_hovered: EDGE, State::Hovered => |c| {
        assert!(shows(&c, 0xffc685, 3.0) && !shows(&c, 0xffa90a, 3.0));
    }
    edge_selected: EDGE, State::Selected => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && !shows(&c, 0xffc685, 3.0));
    }
    edge_selected_hovered: EDGE, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && !shows(&c, 0xffc685, 3.0));
    }
    edge_hovered_other_selected: EDGE, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xffc685, 3.0) && !shows(&c, 0xffa90a, 3.0));
    }
    // The midpoint landed on, whatever the approach: only the point (light-orange ring, centre #ffd8ac), the edge black
    // (live mid-paths, the user's shot hover-point-onshape.png; a single-event jump isn't a hand's).
    mid_hovered: MID, State::Hovered => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0, "{:?}", near_centre(&c, 2.0).collect::<Vec<_>>());
    }
    mid_hovered_other_selected: MID, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    // Selected: the yellow ring (rim #f6be15, centre #ffd65b), the edge plain.
    mid_selected: MID, State::Selected => |c| {
        assert!(shows(&c, 0xf6be15, 7.0) && shows(&c, 0xffd65b, 2.0) && !shows(&c, 0xffc685, 9.0));
    }
    // Selected and hovered: the hovered point's light-orange ring (rim #f6c285, centre #ffd8ac), the edge plain, no dark ring.
    mid_selected_hovered: MID, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && dark(&c, 4.5) == 0, "{:?}", near_centre(&c, 2.0).collect::<Vec<_>>());
    }
    // Arrived along the edge: the point alone (light-orange ring), no band, no dark ring.
    mid_arrived_along_edge: MID, State::Arrived => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && dark(&c, 4.5) == 0);
    }
    // Any path onto it, whatever the angle, lights only the point, the edge black (live mid-paths).
    mid_path_along_edge: MID, State::Path(0.0) => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    mid_path_along_edge_far_side: MID, State::Path(180.0) => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    mid_path_90: MID, State::Path(90.0) => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    mid_path_minus_90: MID, State::Path(-90.0) => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    mid_path_45: MID, State::Path(45.0) => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    mid_path_135: MID, State::Path(135.0) => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    // On a selected edge: a solid orange centre (#ffb32e) over the #ffa90a band, no dark outline.
    mid_arrived_on_selected_edge: MID, State::ArrivedOnSelectedEdge => |c| {
        assert!(shows(&c, 0xffb32e, 2.0) && shows(&c, 0xffa90a, 9.0) && dark(&c, 4.5) == 0);
    }
    // A face: hovered, only its 2 px boundary (#ffc685), the fill as it was; selected, fill #e4aa38.
    face_hovered: FACE, State::Hovered => |c| {
        assert!(!shows(&c, 0xffc685, 20.0) && !shows(&c, 0xe4aa38, 20.0));
        assert!(c.iter().any(|&p| close(p, 0xffc685, 24)), "its boundary in the crop");
    }
    face_selected: FACE, State::Selected => |c| {
        assert!(near_centre(&c, 20.0).all(|p| close(p, 0xe4aa38, 24)));
    }
    face_selected_hovered: FACE, State::SelectedHovered => |c| {
        assert!(near_centre(&c, 20.0).all(|p| close(p, 0xe4aa38, 24)));
    }
    face_hovered_other_selected: FACE, State::HoveredOtherSelected => |c| {
        assert!(!shows(&c, 0xffc685, 20.0) && c.iter().any(|&p| close(p, 0xffc685, 24)));
    }
    // Batch 2, a shown sketch's line in the Part Studio: the part edge's band, its grey line showing through the middle
    // (#f2c694 hovered, #f2b034 selected), not a dark one.
    sketch_line_hovered: SK_LINE, State::Hovered => |c| {
        assert!(shows(&c, 0xffc685, 3.0) && shows(&c, 0xf2c694, 1.5) && !shows(&c, 0xd2a674, 1.5) && !shows(&c, 0xffa90a, 3.0), "{:?}", near_centre(&c, 1.5).collect::<Vec<_>>());
    }
    sketch_line_selected: SK_LINE, State::Selected => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && shows(&c, 0xf2b034, 1.5) && !shows(&c, 0xd29015, 1.5), "{:?}", near_centre(&c, 1.5).collect::<Vec<_>>());
    }
    sketch_line_selected_hovered: SK_LINE, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && shows(&c, 0xf2b034, 1.5));
    }
    sketch_line_hovered_other_selected: SK_LINE, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xffc685, 3.0) && shows(&c, 0xf2c694, 1.5) && !shows(&c, 0xffa90a, 3.0));
    }
    // Its midpoint, and the arc's middle, as a part edge's: hovered only the point's light-orange ring; a click selects
    // the midpoint (yellow ring #f7c01d, centre #ffd65b), hovered then the light-orange ring.
    sketch_linemid_hovered: SK_LINEMID, State::Hovered => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    sketch_linemid_selected: SK_LINEMID, State::Selected => |c| {
        assert!(shows(&c, 0xf7c01d, 7.0) && shows(&c, 0xffd65b, 2.0) && !band_beside_of(&c, 0xffa90a) && !band_beside(&c));
    }
    sketch_linemid_selected_hovered: SK_LINEMID, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside_of(&c, 0xffa90a) && dark(&c, 4.5) == 0);
    }
    sketch_linemid_hovered_other_selected: SK_LINEMID, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    sketch_linemid_path_along: SK_LINEMID, State::Path(0.0) => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 2.0) && !band_beside(&c) && dark(&c, 4.5) == 0);
    }
    sketch_arc_hovered: SK_ARC, State::Hovered => |c| {
        assert!(shows(&c, 0xf6c285, 7.0) && shows(&c, 0xffd8ac, 3.0) && dark(&c, 4.5) == 0);
    }
    sketch_arc_selected: SK_ARC, State::Selected => |c| {
        assert!(shows(&c, 0xf7c01d, 7.0) && shows(&c, 0xffd65b, 3.0) && !band_beside_of(&c, 0xffa90a));
    }
    // A sketch point (standalone, a grey ring with a white middle at rest): lit, an 11 px disc of the state colour with
    // the ring and middle showing through (centre #ffd8ac hovered, #ffc458 selected); hovered once selected it stays
    // selected. In sketch edit the same over its blue ring.
    sketch_point_hovered: SK_POINT, State::Hovered => |c| {
        assert!(shows(&c, 0xffd8ac, 1.5) && shows(&c, 0xffc685, 5.5) && shows(&c, 0xe6bd8d, 4.0) && !shows(&c, 0xffffff, 2.0), "{:?}", near_centre(&c, 5.5).collect::<Vec<_>>());
    }
    sketch_point_selected: SK_POINT, State::Selected => |c| {
        assert!(shows(&c, 0xffc458, 1.5) && shows(&c, 0xffab14, 5.5) && !shows(&c, 0xffd8ac, 3.0), "{:?}", near_centre(&c, 5.5).collect::<Vec<_>>());
    }
    sketch_point_selected_hovered: SK_POINT, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xffc458, 1.5) && shows(&c, 0xffab14, 5.5) && !shows(&c, 0xffd8ac, 3.0));
    }
    sketch_point_hovered_other_selected: SK_POINT, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xffd8ac, 1.5) && shows(&c, 0xffc685, 5.5) && !shows(&c, 0xffffff, 2.0));
    }
    sk_point_hovered: SK_POINT, State::Hovered => edit |c| {
        assert!(shows(&c, 0xffd8ac, 1.5) && shows(&c, 0xffc685, 5.5) && !shows(&c, 0xffffff, 2.0), "{:?}", near_centre(&c, 5.5).collect::<Vec<_>>());
    }
    sk_point_selected: SK_POINT, State::Selected => edit |c| {
        assert!(shows(&c, 0xffc558, 1.5) && shows(&c, 0xffab14, 5.5) && !shows(&c, 0xffffff, 2.0), "{:?}", near_centre(&c, 5.5).collect::<Vec<_>>());
    }
    sk_point_selected_hovered: SK_POINT, State::SelectedHovered => edit |c| {
        assert!(shows(&c, 0xffc558, 1.5) && !shows(&c, 0xffd8ac, 3.0));
    }
    // A circle's centre in the Part Studio: a ~4 px grey dot at rest; lit an 11 px disc, the dot faintly through it.
    sketch_region_none: SK_CENTRE, State::Rest => |c| {
        assert!(shows(&c, 0x999a9b, 1.5), "{:?}", near_centre(&c, 3.0).collect::<Vec<_>>());
    }
    sketch_region_hovered: SK_CENTRE, State::Hovered => |c| {
        assert!(shows(&c, 0xffc685, 5.0) && shows(&c, 0xf4c287, 1.5) && !shows(&c, 0xffa90a, 5.0), "{:?}", near_centre(&c, 5.0).collect::<Vec<_>>());
    }
    sketch_region_selected: SK_CENTRE, State::Selected => |c| {
        assert!(shows(&c, 0xffa90a, 5.0) && shows(&c, 0xf4a719, 1.5) && !shows(&c, 0xffc685, 5.0), "{:?}", near_centre(&c, 5.0).collect::<Vec<_>>());
    }
    sketch_region_selected_hovered: SK_CENTRE, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xffa90a, 5.0) && !shows(&c, 0xffc685, 5.0));
    }
    // A line's free end in the Part Studio (a 4 px grey dot at rest): lit an 11 px disc as a centre's (live workflow 12:
    // another sketch's end selected under the pointer ~12 px across).
    sketch_lineend_hovered: SK_LINEEND, State::Hovered => |c| {
        assert!(ring_of(&c, 0xffc685, 4.0, 5.0) >= 0.75, "{:?}", near_centre(&c, 5.0).collect::<Vec<_>>());
    }
    // Its size, ~12 px: orange to 5 px out (live's shot is in a sketch, its colour there not yet compared).
    sketch_lineend_selected_hovered: SK_LINEEND, State::SelectedHovered => |c| {
        let off = |i: usize| DVec2::new((i % 80) as f64 - 40.0, (i / 80) as f64 - 40.0).length();
        let ring: Vec<[u8; 3]> = c.iter().enumerate().filter(|(i, _)| (4.0..=5.0).contains(&off(*i))).map(|(_, p)| *p).collect();
        let orange = ring.iter().filter(|p| p[0] >= 240 && (160..=215).contains(&p[1]) && p[2] <= 170).count();
        assert!(orange * 5 >= ring.len() * 4, "{:?}", near_centre(&c, 5.0).collect::<Vec<_>>());
    }
    // Sketch edit: the band over the blue line, which shows through its middle (#c89c96 hovered, #c88536 selected); no
    // midpoint marker; a click on the line's middle takes the line.
    sk_line_hovered: SK_LINE, State::Hovered => edit |c| {
        assert!(shows(&c, 0xffc685, 3.0) && shows(&c, 0xc89c96, 1.5) && !shows(&c, 0xca9f6d, 1.5), "{:?}", near_centre(&c, 1.5).collect::<Vec<_>>());
    }
    sk_line_selected: SK_LINE, State::Selected => edit |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && shows(&c, 0xc88536, 1.5) && !shows(&c, 0xca890d, 1.5), "{:?}", near_centre(&c, 1.5).collect::<Vec<_>>());
    }
    sk_line_selected_hovered: SK_LINE, State::SelectedHovered => edit |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && shows(&c, 0xc88536, 1.5));
    }
    sk_line_hovered_other_selected: SK_LINE, State::HoveredOtherSelected => edit |c| {
        assert!(shows(&c, 0xffc685, 3.0) && shows(&c, 0xc89c96, 1.5) && !shows(&c, 0xffa90a, 3.0));
    }
    sk_linemid_hovered: SK_LINEMID, State::Hovered => edit |c| {
        assert!(shows(&c, 0xffc685, 3.0) && shows(&c, 0xc89c96, 1.5) && dark(&c, 6.0) == 0 && !shows(&c, 0xc49867, 6.0));
    }
    sk_linemid_selected: SK_LINEMID, State::Selected => edit |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && shows(&c, 0xc88536, 1.5) && !shows(&c, 0xc4960b, 6.0));
    }
    sk_arc_hovered: SK_ARC, State::Hovered => edit |c| {
        assert!(shows(&c, 0xffc685, 3.0) && shows(&c, 0xc89c96, 2.0) && dark(&c, 6.0) == 0);
    }
    sk_arc_selected: SK_ARC, State::Selected => edit |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && shows(&c, 0xc88536, 2.0));
    }
}

/// A batch 3 cell (live b3/): the matrix with Mate connector 1, Fillet 1 open at live's 1.3 mm when `dialog`, `target` in
/// `state` (the other selection: the right face, in the Fillet field the top front edge); the matrix and its crop.
fn b3_cell(cx: &mut TestAppContext, dialog: bool, target: [f64; 3], state: State) -> (Matrix, Vec<[u8; 3]>) {
    let m = matrix_studio_with(cx, "hover-matrix-b3-cell", true);
    wait_until(cx, m.window, |cx| !m.view.read(cx).display.connectors.is_empty());
    if dialog {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
        draw(cx, m.window);
        let d = cx.update(|cx| m.view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).expect("the Fillet dialog");
        cx.update_window(m.window, |_, w, cx| d.update(cx, |d, cx| d.set_expr_text("radius", "1.3 mm", w, cx))).unwrap();
        m.park(cx);
    }
    let p = m.exact(cx, target);
    match state {
        State::Hovered => approach(cx, &m, p),
        State::Selected | State::SelectedHovered => {
            approach(cx, &m, p);
            m.pointer.click(cx, &m.view, p);
            m.park(cx);
            if state == State::SelectedHovered {
                approach(cx, &m, p);
            }
        }
        State::HoveredOtherSelected => {
            let other = m.screen(cx, if dialog { EDGE } else { [40., 15., 10.] });
            m.pointer.move_to(cx, &m.view, other);
            m.pointer.click(cx, &m.view, other);
            m.park(cx);
            approach(cx, &m, p);
        }
        _ => m.park(cx),
    }
    let crop = m.crop(cx, p);
    (m, crop)
}

/// Whether `rgb` (within 24, 2 px either way: a selected band's pure rows are 1 px) runs along the crop from its centre in
/// direction `d` (screen px), 6 to 20 px out.
fn runs_along(crop: &[[u8; 3]], d: DVec2, rgb: u32) -> bool {
    let d = d.normalize();
    (6..=20).filter(|&k| {
        let q = DVec2::new(40.0, 40.0) + d * k as f64;
        (-2..=2).flat_map(|dy| (-2..=2).map(move |dx| (dx, dy))).any(|(dx, dy)| {
            let (x, y) = (q.x.round() as i32 + dx, q.y.round() as i32 + dy);
            (0..80).contains(&x) && (0..80).contains(&y) && close(crop[(y * 80 + x) as usize], rgb, 24)
        })
    })
    .count()
        >= 10
}

/// The screen direction of a world step `d` (mm) at `at`.
fn screen_dir(cx: &mut TestAppContext, m: &Matrix, at: [f64; 3], d: [f64; 3]) -> DVec2 {
    m.screen(cx, [at[0] + d[0], at[1] + d[1], at[2] + d[2]]) - m.screen(cx, at)
}

const ORIGIN: [f64; 3] = [0., 0., 0.];
const CONNECTOR: [f64; 3] = [40., 30., 20.];

// The origin at the block's corner (live b3/ps-origin-*): the +x edge through it lights and takes the click (5 px
// #ffc685 band, #ffa90a selected); the vertical edge stays black.
#[gpui::test]
fn b3_origin_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, false, ORIGIN, State::Hovered);
    dump("cell-b3_origin_hovered", &c);
    let (x, z) = (screen_dir(cx, &m, ORIGIN, [5., 0., 0.]), screen_dir(cx, &m, ORIGIN, [0., 0., 5.]));
    assert!(runs_along(&c, x, 0xffc685) && !runs_along(&c, z, 0xffc685));
}

#[gpui::test]
fn b3_origin_selected(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, false, ORIGIN, State::Selected);
    dump("cell-b3_origin_selected", &c);
    let (x, z) = (screen_dir(cx, &m, ORIGIN, [5., 0., 0.]), screen_dir(cx, &m, ORIGIN, [0., 0., 5.]));
    assert!(runs_along(&c, x, 0xffa90a) && !runs_along(&c, z, 0xffa90a));
}

#[gpui::test]
fn b3_origin_selected_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, false, ORIGIN, State::SelectedHovered);
    dump("cell-b3_origin_selected_hovered", &c);
    let (x, z) = (screen_dir(cx, &m, ORIGIN, [5., 0., 0.]), screen_dir(cx, &m, ORIGIN, [0., 0., 5.]));
    assert!(runs_along(&c, x, 0xffa90a) && !runs_along(&c, z, 0xffa90a) && !runs_along(&c, z, 0xffc685));
}

#[gpui::test]
fn b3_origin_hovered_other_selected(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, false, ORIGIN, State::HoveredOtherSelected);
    dump("cell-b3_origin_hovered_other_selected", &c);
    let (x, z) = (screen_dir(cx, &m, ORIGIN, [5., 0., 0.]), screen_dir(cx, &m, ORIGIN, [0., 0., 5.]));
    assert!(runs_along(&c, x, 0xffc685) && !runs_along(&c, z, 0xffc685));
}

/// Crisp navy rim pixels (#002847 within 12): live's rim is soft, its darkest #1e4461..#26465b.
fn crisp_navy(crop: &[[u8; 3]]) -> usize {
    crop.iter().filter(|c| close(**c, 0x002847, 12)).count()
}

/// Dark navy pixels as qa's hover-matrix score classes them (hue 185..240, saturation >= 0.45, value 0.12..0.45): live's
/// soft connector rim has 41 (b3/ps-mc-hover), ours had 77 at 1.3 px.
fn navy_class(crop: &[[u8; 3]]) -> usize {
    crop.iter().filter(|&&[r, g, b]| {
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        let (v, d) = (f32::from(mx) / 255.0, f32::from(mx - mn));
        if mx == 0 || d == 0.0 || mx != b { return false; }
        let h = 60.0 * ((f32::from(r) - f32::from(g)) / d + 4.0);
        (185.0..=240.0).contains(&h) && d / f32::from(mx) >= 0.45 && (0.12..0.45).contains(&v)
    }).count()
}

// Mate connector 1 (live b3/ps-mc-*): hovered, the disc's wedge #f4df99 over the background and its ring #c2c187 over
// the face, a soft navy rim, the owner's edges yellow-green #e1ec87; selected #f6da83 / #ccc173 and the owner's faces
// as they are.
#[gpui::test]
fn b3_connector_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_m, c) = b3_cell(cx, false, CONNECTOR, State::Hovered);
    dump("cell-b3_connector_hovered", &c);
    assert!(shows(&c, 0xf4df99, 16.0) && shows(&c, 0xe1ec87, 30.0) && crisp_navy(&c) == 0, "{} crisp navy px", crisp_navy(&c));
    // Its origin vertex lit under the glyph: a ~10 px #f1fe81 disc round the corner (live ps-mc-hover: 39 px of it
    // within 5 px of the centre; the owner's three edges meeting there alone gave ours 18).
    let lit = near_centre(&c, 5.0).filter(|p| close(*p, 0xf1fe81, 16)).count();
    assert!(lit >= 30, "{lit} lit origin px");
    // The blue z-axis drawn over that disc down to the centre (qa FEEL-MC-AXIS-UNDER-DISC: live 17 px of it show, ours
    // 11 under the disc).
    let blue: Vec<usize> = (0..=42).filter(|y| (34..=46).any(|x| close(c[y * 80 + x], 0x0000d0, 60))).collect();
    assert!(blue.len() >= 15 && blue.last().is_some_and(|&y| y >= 38), "z-axis rows {blue:?} (live 17, down to the centre)");
    // Its top 20 px above the centre (live; ours 16 at 18 px long), the rim soft (live 41 navy px).
    assert!(blue.first().is_some_and(|&y| y <= 21), "z-axis rows {blue:?} (live from row 20)");
    let navy = navy_class(&c);
    assert!((25..=60).contains(&navy), "{navy} navy rim px (live 41)");
}

#[gpui::test]
fn b3_connector_selected(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_m, c) = b3_cell(cx, false, CONNECTOR, State::Selected);
    dump("cell-b3_connector_selected", &c);
    // (Our old owner tint: the whole top face #bed5a8, ~1000 px; the owner edges' blend with the face gives a few.)
    assert!(shows(&c, 0xccc173, 16.0) && count(&c, 0xbed5a8) < 300 && shows(&c, 0x99c0d7, 40.0) && crisp_navy(&c) == 0);
    let lit = near_centre(&c, 5.0).filter(|p| close(*p, 0xf1fe81, 16)).count();
    assert!(lit >= 30, "{lit} lit origin px (live ps-mc-selected 39)");
}

#[gpui::test]
fn b3_connector_selected_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_m, c) = b3_cell(cx, false, CONNECTOR, State::SelectedHovered);
    dump("cell-b3_connector_selected_hovered", &c);
    assert!(shows(&c, 0xccc173, 16.0) && count(&c, 0xbed5a8) < 300);
}

#[gpui::test]
fn b3_connector_hovered_other_selected(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_m, c) = b3_cell(cx, false, CONNECTOR, State::HoveredOtherSelected);
    dump("cell-b3_connector_hovered_other_selected", &c);
    // The selected right face is one of the owner's: its fill plain while the connector is hovered (live
    // ps-mc-otherhover-r, #d9a235 before and after, #8db0c5 on the connector).
    assert!(shows(&c, 0xf4df99, 16.0) && crisp_navy(&c) == 0 && count(&c, 0xd9a235) < 30, "{} selected fill px", count(&c, 0xd9a235));
}

/// Pixels of `rgb` (within 24) anywhere in the crop.
fn count(crop: &[[u8; 3]], rgb: u32) -> usize {
    crop.iter().filter(|c| close(**c, rgb, 24)).count()
}

// Fillet 1's field (live b3/dlg-*): the plain hover look over the dialog's preview (the 5 px #ffc685 band, its middle
// rows darker where the edge shows through, no midpoint marker; a face's #ffc685 boundary, never the muted #b28a5d);
// a corner lights the top front edge.
#[gpui::test]
fn b3_field_edge_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_m, c) = b3_cell(cx, true, EDGE, State::Hovered);
    dump("cell-b3_field_edge_hovered", &c);
    assert!(shows(&c, 0xffc685, 3.0) && dark(&c, 6.0) < 4);
}

#[gpui::test]
fn b3_field_midpoint_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_m, c) = b3_cell(cx, true, MID, State::Hovered);
    dump("cell-b3_field_midpoint_hovered", &c);
    assert!(shows(&c, 0xffc685, 3.0) && dark(&c, 6.0) < 4);
}

#[gpui::test]
fn b3_field_vertex_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, true, VERTEX, State::Hovered);
    dump("cell-b3_field_vertex_hovered", &c);
    let (x, z) = (screen_dir(cx, &m, VERTEX, [-5., 0., 0.]), screen_dir(cx, &m, VERTEX, [0., 0., -5.]));
    assert!(runs_along(&c, x, 0xffc685) && !runs_along(&c, z, 0xffc685));
}

#[gpui::test]
fn b3_field_face_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_m, c) = b3_cell(cx, true, FACE, State::Hovered);
    dump("cell-b3_field_face_hovered", &c);
    assert!(count(&c, 0xffc685) > 200 && count(&c, 0xb28a5d) < 20, "{} #ffc685, {} #b28a5d", count(&c, 0xffc685), count(&c, 0xb28a5d));
}

/// White pixels of the radius arrow down-right of the edge's midpoint (live b3w: it points into the material from the
/// band), and above the band where ours stood upright.
fn arrow_px(cx: &mut TestAppContext, m: &Matrix, crop: &[[u8; 3]]) -> (usize, usize) {
    let mid = m.screen(cx, MID) - m.screen(cx, EDGE) + DVec2::new(40.0, 40.0);
    let into = screen_dir(cx, m, MID, [0., 0.707, -0.707]).normalize();
    let (mut ahead, mut behind) = (0, 0);
    for (i, c) in crop.iter().enumerate() {
        let t = (DVec2::new((i % 80) as f64, (i / 80) as f64) - mid).dot(into);
        if c.iter().all(|&v| v > 235) && (DVec2::new((i % 80) as f64, (i / 80) as f64) - mid).length() < 26.0 {
            if t > 2.0 { ahead += 1 } else if t < -3.0 { behind += 1 }
        }
    }
    (ahead, behind)
}

#[gpui::test]
fn b3_field_edge_picked(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, true, EDGE, State::Selected);
    dump("cell-b3_field_edge_picked", &c);
    let (ahead, behind) = arrow_px(cx, &m, &c);
    assert!(shows(&c, 0xffa90a, 3.0) && ahead >= 8 && behind == 0, "arrow px ahead {ahead}, behind {behind}");
    // The selected edge's 5 px band over the round (live dlg-edge-selected: #FFA90A at both tangent lines, #DDA027..
    // #FFAF25 between); ours was a 3 px line with the face between it and the tangent lines.
    let along = screen_dir(cx, &m, EDGE, [5., 0., 0.]);
    let run = amber_across(&c, along);
    assert!((4..=6).contains(&run), "{run} amber px across the picked edge (live 4, ours was 3)");
}

/// Amber pixels (#FFA90A over anything: r ≥ #D0, g #98..#B8, b ≤ #40) on the line through the crop's centre across
/// `along`, within 6 px.
fn amber_across(crop: &[[u8; 3]], along: DVec2) -> usize {
    let n = DVec2::new(-along.y, along.x).normalize();
    (-6..=6)
        .filter(|k| {
            let q = DVec2::new(40.0, 40.0) + n * f64::from(*k);
            let [r, g, b] = crop[q.y.round() as usize * 80 + q.x.round() as usize];
            r >= 0xd0 && (0x98..=0xb8).contains(&g) && b <= 0x40
        })
        .count()
}

#[gpui::test]
fn b3_field_edge_picked_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, true, EDGE, State::SelectedHovered);
    dump("cell-b3_field_edge_picked_hovered", &c);
    let (ahead, behind) = arrow_px(cx, &m, &c);
    assert!(shows(&c, 0xffa90a, 3.0) && !shows(&c, 0xffc685, 3.0) && ahead >= 8 && behind == 0, "arrow px ahead {ahead}, behind {behind}");
}

#[gpui::test]
fn b3_field_face_hovered_edge_picked(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (m, c) = b3_cell(cx, true, FACE, State::HoveredOtherSelected);
    dump("cell-b3_field_face_hovered_edge_picked", &c);
    let edge = m.screen(cx, EDGE) - m.screen(cx, FACE) + DVec2::new(40.0, 40.0);
    let picked = (edge.y as usize - 2..=edge.y as usize + 2).any(|y| close(c[y * 80 + edge.x as usize], 0xffa90a, 24));
    // (#b28a5d: the muted boundary gave ~80; the preview's tangent line beside the band blends to a few, live 9.
    // #ffc685: the face's own boundary only, 125 since the preview-made edges stay plain.)
    assert!(picked && count(&c, 0xffc685) > 100 && count(&c, 0xb28a5d) < 50, "{} #ffc685, {} #b28a5d", count(&c, 0xffc685), count(&c, 0xb28a5d));
    // The band over the hovered face's boundary line beside it: its core #ffaf25, 78 % over #ffc685 (qa
    // FEEL-FILLET-FACE-OVER-PICKED: ours the core #dd9f26 over the face with the 1 px line over the band).
    let column: Vec<[u8; 3]> = (edge.y as usize - 4..=edge.y as usize + 4).map(|y| c[y * 80 + edge.x as usize]).collect();
    assert!(column.iter().any(|p| close(*p, 0xffaf25, 10)) && !column.iter().any(|p| close(*p, 0xdd9f26, 10)), "across the band {column:02x?}");
    // No #ffc685 along the picked edge: the preview's tangent line 3 px over the band stays dark (live
    // hover-matrix-recheck-1123 cell 4: 0 changed px within ±7 px; the face's other edges lit).
    let near: Vec<[u8; 3]> = (edge.y as usize - 7..=edge.y as usize + 7).flat_map(|y| (edge.x as usize - 6..=edge.x as usize + 6).map(move |x| (x, y))).map(|(x, y)| c[y * 80 + x]).collect();
    assert_eq!(near.iter().filter(|p| close(**p, 0xffc685, 12)).count(), 0, "#ffc685 within 7 px of the picked edge");
}

// Select other on the top face (live uip/select-other-row-click-1..4, qa FEEL-SELECT-OTHER-LOOK): no point is drawn where
// it was asked, also once the pointer has left the click zone.
#[gpui::test]
fn select_other_draws_no_point_where_it_was_asked(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "select-other-no-point");
    let p = m.screen(cx, FACE);
    approach(cx, &m, p);
    m.view.update(cx, |v, cx| v.select_other(false, cx));
    draw(cx, m.window);
    assert!(cx.update(|cx| m.view.read(cx).ui.select_other.is_some()), "the Select other list is open");
    m.pointer.move_to(cx, &m.view, p + DVec2::new(60.0, 60.0));
    let c = m.crop(cx, p);
    dump("cell-select_other_no_point", &c);
    assert_eq!(count(&c, 0xffc20e), 0, "#ffc20e px at the asked point");
}

// A shown sketch's line right-clicked on its lit midpoint in the Part Studio: the line's menu at its midpoint, not a
// model point's (live ctx-menus-1117 sketch2-line-graphics, Add comment left out; qa MENU-SKETCH-IN-PS).
#[gpui::test]
fn a_sketch_lines_lit_midpoint_right_clicked_gives_the_lines_menu(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "sketch-linemid-menu");
    let p = m.screen(cx, SK_LINEMID);
    approach(cx, &m, p);
    assert!(cx.update(|cx| matches!(m.view.read(cx).hover, Some(Reference::Implicit { .. }))), "the midpoint lit");
    m.pointer.right_click(cx, &m.view, p);
    let rows = cx.update(|cx| m.view.read(cx).context_menu.as_ref().map(|(_, menu, _)| menu.read(cx).labels()).unwrap_or_default());
    let live = ["Edit Sketch 2…", "Show dimensions", "Show dependencies of Sketch 2…", "———", "Hide", "Hide other sketches", "Hide all sketches", "———", "Create Drawing of Sketch 2…", "———", "Select", "Select other…", "———", "Zoom to fit", "———", "Delete Sketch 2", "———", "Edit sketch appearance…"];
    assert_eq!(rows, live);
}

// Sketch 2 (on Top) edited, the block's top face right-clicked (live's on its own 10 mm block): Section view… and View normal to, no Curve/surface
// analysis… (live ctx-menus-1117 sk-model-face, Add comment left out; qa MENU-SK-MODEL-ENTITY).
#[gpui::test]
fn a_model_face_right_clicked_in_a_sketch_has_lives_menu(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "sk-model-face-menu");
    m.edit_sketch(cx);
    let p = m.screen(cx, FACE);
    approach(cx, &m, p);
    let lit = cx.update(|cx| m.view.read(cx).hover.clone());
    assert!(matches!(lit, Some(Reference::Face(_))), "the face lit: {lit:?} at {p:?}, the midpoint at {:?}", m.screen(cx, MID));
    m.pointer.right_click(cx, &m.view, p);
    let rows: Vec<String> = cx.update(|cx| m.view.read(cx).context_menu.as_ref().map(|(_, menu, _)| menu.read(cx).labels()).unwrap_or_default());
    let rows: Vec<&str> = rows.iter().map(String::as_str).filter(|l| *l != "———").collect();
    let live = ["Confirm Sketch 2", "Use", "Hide Part 1", "Isolate…", "Make transparent…", "Section view…", "Clear selection", "Select", "Select other…", "Zoom to fit", "Zoom to selection", "View normal to", "View normal to sketch plane"];
    assert_eq!(rows, live);
}

// Sketch 2 (on Top) edited, no tool: a part edge's midpoint lights as in the Part Studio, its menu the part's items
// only; off it the edge's adds Use, Curve/surface analysis…, Zoom to selection and Align view (live psui walk 5 p1a / p1b,
// Add comment left out).
#[gpui::test]
fn a_model_edges_midpoint_in_a_sketch_lights_and_has_lives_menu(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "sk-model-mid-menu");
    m.edit_sketch(cx);
    let menu = |cx: &mut TestAppContext, at: [f64; 3]| -> (Option<Reference>, Vec<String>) {
        let p = m.screen(cx, at);
        approach(cx, &m, p);
        let lit = cx.update(|cx| m.view.read(cx).hover.clone());
        m.pointer.right_click(cx, &m.view, p);
        let rows = cx.update(|cx| m.view.read(cx).context_menu.as_ref().map(|(_, menu, _)| menu.read(cx).labels()).unwrap_or_default());
        m.view.update(cx, |v, cx| v.close_context_menu(cx));
        (lit, rows.into_iter().filter(|l| l != "———").collect())
    };
    let (lit, rows) = menu(cx, MID);
    assert!(matches!(lit, Some(Reference::Implicit { .. })), "the midpoint lit: {lit:?}");
    assert_eq!(rows, ["Confirm Sketch 2", "Hide Part 1", "Isolate…", "Make transparent…", "Clear selection", "Select", "Select other…", "Zoom to fit", "View normal to sketch plane"]);
    let (lit, rows) = menu(cx, EDGE);
    assert!(matches!(lit, Some(Reference::Edge(_))), "the edge lit: {lit:?}");
    assert_eq!(rows, ["Confirm Sketch 2", "Use", "Hide Part 1", "Curve/surface analysis…", "Isolate…", "Make transparent…", "Clear selection", "Select", "Select other…", "Zoom to fit", "Zoom to selection", "View normal to sketch plane", "Align view with vertical axis"]);
}

// Live workflow 05 step 4 (qa FEEL-MENU-EDGE-STAYS-LIT): in a sketch a part edge right-clicked stays lit while its menu is
// open, the pointer gone onto the menu (ours dropped it); "Hide Part 1" has the eye-slash (MENU-HIDE-PART-ICON).
#[gpui::test]
fn a_model_edge_right_clicked_in_a_sketch_stays_lit_under_its_menu(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "sk-model-edge-menu-lit");
    m.edit_sketch(cx);
    let p = m.screen(cx, EDGE);
    approach(cx, &m, p);
    let edge = cx.update(|cx| m.view.read(cx).hover.clone()).filter(|r| matches!(r, Reference::Edge(_))).expect("the edge lit");
    m.pointer.right_click(cx, &m.view, p);
    m.pointer.move_to(cx, &m.view, p + DVec2::new(60.0, 40.0));
    draw(cx, m.window);
    let (lit, icon) = cx.update(|cx| {
        let v = m.view.read(cx);
        let icon = v.context_menu.as_ref().and_then(|(_, menu, _)| menu.read(cx).named_icon("Hide Part 1").map(|n| n.to_string()));
        (v.highlighted_hover(cx), icon)
    });
    assert_eq!((lit, icon.as_deref()), (vec![edge], Some("eye-off")), "(lit with the menu open, Hide Part 1's icon)");
}

// The Part Studio, the top face selected: the hand path to an edge's midpoint lights it, the click adds it to the face,
// and the right-click gives the part's menu ending in "Add appearance to face…" (live psui walk 5 p2a-p2d).
#[gpui::test]
fn a_midpoint_clicked_with_a_face_selected_joins_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "ps-mid-with-face");
    let f = m.screen(cx, FACE);
    approach(cx, &m, f);
    m.pointer.click(cx, &m.view, f);
    let p = m.screen(cx, MID);
    approach(cx, &m, p);
    m.pointer.click(cx, &m.view, p);
    let sel = cx.update(|cx| m.view.read(cx).selection.clone());
    assert!(sel.len() == 2 && matches!(sel[0], Reference::Face(_)) && matches!(sel[1], Reference::Implicit { .. }), "the face and the midpoint: {sel:?}");
    m.pointer.right_click(cx, &m.view, p);
    let rows: Vec<String> = cx.update(|cx| m.view.read(cx).context_menu.as_ref().map(|(_, menu, _)| menu.read(cx).labels()).unwrap_or_default());
    let rows: Vec<&str> = rows.iter().map(String::as_str).filter(|l| *l != "———").collect();
    assert_eq!(rows.first().copied(), Some("Edit Extrude 1…"), "{rows:?}");
    assert_eq!(rows.last().copied(), Some("Add appearance to face…"), "{rows:?}");
}

// Sketch 2 (on Top) edited, a right-click on Top's square where nothing else is (live qa walk 5 b-rclick-75-40,
// ctx-menus-1117 sk-empty and sk-tool-line-active, Add comment left out; qa MENU-SK-SKETCH-PLANE): unlit at rest, it
// gives the Top plane's menu and is lit while that is open; with the Line tool, Escape line first.
#[gpui::test]
fn the_sketchs_own_plane_right_clicked_gives_its_menu(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "sk-own-plane-menu");
    m.edit_sketch(cx);
    let p = m.screen(cx, [75., 40., 0.]);
    let plane = ["Confirm Sketch 2", "Hide Top plane", "Hide other planes", "Hide all planes", "Section view…", "Clear selection", "Select", "Select other…", "Zoom to fit", "Zoom to selection", "View normal to", "View normal to sketch plane"];
    let mut wrong = Vec::new();
    for (tool, first) in [(Tool::Select, None), (Tool::Line, Some("Escape line"))] {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.activate_tool(tool, w, cx))).unwrap();
        approach(cx, &m, p);
        if let Some(h) = cx.update(|cx| m.view.read(cx).hover.clone()) {
            wrong.push(format!("{tool:?}: {h:?} lit at rest"));
        }
        m.pointer.right_click(cx, &m.view, p);
        let rows: Vec<String> = cx.update(|cx| m.view.read(cx).context_menu.as_ref().map(|(_, menu, _)| menu.read(cx).labels()).unwrap_or_default());
        let rows: Vec<&str> = rows.iter().map(String::as_str).filter(|l| *l != "———").collect();
        let want: Vec<&str> = first.into_iter().chain(plane).collect();
        if rows != want {
            wrong.push(format!("{tool:?}: {rows:?}"));
        }
        let lit = cx.update(|cx| m.view.read(cx).ui.menu_target.clone());
        if lit != Some(Reference::Plane(DatumPlane::Top)) {
            wrong.push(format!("{tool:?}: {lit:?} lit under the menu"));
        }
        // Drawn lit (live workflow 40, qa FEEL-SKETCH-RCLICK-PLANE-LIT: the plane's outline orange under the menu).
        let edge = cx.update(|cx| {
            let v = m.view.read(cx);
            let vp = v.viewport.read(cx);
            let pl = v.display.planes.iter().find(|q| q.reference == Reference::Plane(DatumPlane::Top))?;
            [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)].into_iter().filter_map(|(u, w)| vp.project(pl.corner(u, w))).find(|s| s.x > 60.0 && s.y > 60.0 && s.x < vp.size().x - 60.0 && s.y < vp.size().y - 60.0)
        });
        let Some(edge) = edge else {
            wrong.push(format!("{tool:?}: no edge of the Top plane on screen"));
            continue;
        };
        let lit_px = count(&m.crop(cx, edge), 0xffc685);
        m.view.update(cx, |v, cx| v.close_context_menu(cx));
        m.park(cx);
        let rest_px = count(&m.crop(cx, edge), 0xffc685);
        if lit_px < 40 || rest_px > 0 {
            wrong.push(format!("{tool:?}: the plane's border not drawn lit under the menu ({lit_px} #ffc685 px open, {rest_px} closed)"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// A new Extrude on the block's top face, its 25 mm preview over the block (live walk6-2 preview-hover, qa
// FEEL-PREVIEW-HOVER-LIT): the preview's front face lights nothing where the block wasn't; the block's own front face
// below lights.
#[gpui::test]
fn an_extrudes_preview_face_lights_nothing(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "preview-hover");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    let p = m.screen(cx, FACE);
    approach(cx, &m, p);
    m.pointer.click(cx, &m.view, p);
    wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
    let hover_at = |cx: &mut TestAppContext, mm: [f64; 3]| {
        m.park(cx);
        let q = m.screen(cx, mm);
        approach(cx, &m, q);
        cx.update(|cx| m.view.read(cx).hover.clone())
    };
    let on_preview = hover_at(cx, [20., 0., 32.]);
    assert_eq!(on_preview, None, "on the preview's front face");
    let on_block = hover_at(cx, [20., 0., 8.]);
    assert!(matches!(on_block, Some(Reference::Face(_))), "on the block's front face: {on_block:?}");
}

// A Fillet's own preview isn't pickable (live 10-27 PREVIEW-SELF-PICK, behaviours "qa (2026-10-27, P2)"): with the
// top-front edge at 5 mm, the rounded band's upper half hovers the original top face, its lower half the original front
// face, and a click adds "Face of Extrude 1": no "deleted feature" chip, the field and title not red, ✓ enabled.
#[gpui::test]
fn a_fillets_preview_band_picks_the_faces_it_was_built_on(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "preview-self-pick");
    draw(cx, m.window);
    let hover_at = |cx: &mut TestAppContext, mm: [f64; 3]| {
        m.park(cx);
        let q = m.screen(cx, mm);
        approach(cx, &m, q);
        cx.update(|cx| m.view.read(cx).hover.clone())
    };
    let top = hover_at(cx, FACE).expect("the top face");
    let front = hover_at(cx, [20., 0., 8.]).expect("the front face");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    draw(cx, m.window);
    let p = m.exact(cx, EDGE);
    approach(cx, &m, p);
    m.pointer.click(cx, &m.view, p);
    let d = cx.update(|cx| m.view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).expect("the Fillet dialog");
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("radius", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("5 mm")), cx)));
    wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
    // The band: centre (y 5, z 15), radius 5; 5° and 80° down from its top tangent line (clear of the picked edge drawn
    // through its middle) and of the radius arrow at the edge's midpoint (x 20).
    let (a, b) = (5f64.to_radians(), 80f64.to_radians());
    let upper = [32., 5. - 5. * a.sin(), 15. + 5. * a.cos()];
    let lower = [32., 5. - 5. * b.sin(), 15. + 5. * b.cos()];
    assert_eq!(hover_at(cx, upper).as_ref(), Some(&top), "the band's upper half");
    assert_eq!(hover_at(cx, lower).as_ref(), Some(&front), "the band's lower half");
    let q = m.screen(cx, upper);
    approach(cx, &m, q);
    m.pointer.click(cx, &m.view, q);
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    draw(cx, m.window);
    let s = cx.update_window(m.window, |_, w, cx| m.view.read(cx).ui_state(w, cx)).unwrap();
    let field = &s["dialog"]["fields"].as_array().expect("fields")[0];
    let items: Vec<&str> = field["items"].as_array().expect("chips").iter().filter_map(|i| i.as_str()).collect();
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(items[1], "Face of Extrude 1", "{items:?}");
    assert_eq!((field["state"].as_str(), s["dialog"]["titleState"].as_str(), s["dialog"]["ok"].as_str()), (Some("ok"), Some("ok"), Some("enabled")));
}

// Delete part's preview (live 10-27 delete-preview, behaviours "psui (10-27) Delete part preview"): at the slider's
// rest, 0 and 1 the only orange (255,192,76) is the part's silhouette; its inner edges never are. The fill is a pale
// ghost at rest (top (198,208,230)), the normal part at 0 and gone at 1.
#[gpui::test]
fn delete_parts_preview_outlines_only_the_silhouette(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "delete-preview-look");
    draw(cx, m.window);
    m.park(cx);
    let (outline, inner, top) = (m.exact(cx, [20., 0., 0.]).round(), m.exact(cx, [20., 0., 20.]).round(), m.exact(cx, [20., 15., 20.]).round());
    let at = |f: &(u32, u32, Vec<[u8; 3]>), p: DVec2| f.2[(p.y as u32 * f.0 + p.x as u32) as usize];
    let orange = |f: &(u32, u32, Vec<[u8; 3]>), p: DVec2, r: i32| {
        (-r..=r).flat_map(|y| (-r..=r).map(move |x| p + DVec2::new(f64::from(x), f64::from(y)))).filter(|&q| close(at(f, q), 0xffc04c, 24)).count()
    };
    let part_top = at(&m.frame(cx), top);
    let body = cx.update(|cx| m.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 1").map(|b| b.id.clone()).expect("Part 1"));
    m.view.update(cx, |v, _| v.selection = vec![Reference::Body(body.clone())]);
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("deleteBodies", w, cx))).unwrap();
    wait_until(cx, m.window, |cx| {
        let v = m.view.read(cx);
        !v.ui_busy() && v.ui.preview_before.is_some() && v.ui.removed_outline.as_ref().is_some_and(|(_, o)| o.is_some())
    });
    let mut wrong = Vec::new();
    for (name, t) in [("rest", None), ("0", Some(0.0)), ("1", Some(1.0))] {
        if let Some(t) = t {
            m.view.update(cx, |v, cx| {
                v.ui.preview_opacity = t;
                v.refresh_scene(false, cx);
            });
        }
        let f = m.frame(cx);
        let (o, i, fill) = (orange(&f, outline, 4), orange(&f, inner, 3), at(&f, top));
        if o < 3 || i > 0 {
            wrong.push(format!("{name}: {o} orange px on the silhouette (want ≥ 3), {i} on the front top edge (want 0)"));
        }
        let want = match name {
            "rest" => close(fill, 0xc6d0e6, 24),
            "0" => fill.iter().zip(part_top).all(|(a, b)| a.abs_diff(b) <= 12),
            _ => fill.iter().zip(part_top).any(|(a, b)| a.abs_diff(b) > 40),
        };
        if !want {
            wrong.push(format!("{name}: the top reads {fill:?} (the part's own {part_top:?})"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// Shell 1 with the block's top face in Faces to remove, its centre clicked again (live walk6-3 shell-reclick, qa
// FEEL-SHELL-RECLICK-REMOVED-FACE): resting there lights nothing new, the click takes the face out of the field.
#[gpui::test]
fn a_shells_removed_face_clicked_again_leaves_the_field(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "shell-reclick");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("shell", w, cx))).unwrap();
    let p = m.screen(cx, FACE);
    approach(cx, &m, p);
    m.pointer.click(cx, &m.view, p);
    let faces = |cx: &mut TestAppContext| cx.update(|cx| m.view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.params.query("entities").len()));
    assert_eq!(faces(cx), Some(1), "the top face in the field");
    wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
    m.park(cx);
    let parked = m.crop(cx, p);
    approach(cx, &m, p);
    let rest = m.crop(cx, p);
    let changed = parked.iter().zip(&rest).filter(|(a, b)| !close(**a, u32::from_be_bytes([0, b[0], b[1], b[2]]), 8)).count();
    assert!(changed < 20, "{changed} px changed resting on the removed face");
    m.pointer.click(cx, &m.view, p);
    assert_eq!(faces(cx), Some(0), "the field after the click");
}

// The line the Line tool's snap lands on (live sk/inference-on-line-at, qa FEEL-SKETCH-BAND-MIDDLE inf-on-line): lit as
// hovered, the blue line through the band's middle (live #c79a95), not an opaque #ecb78a / #ffc685 line.
#[gpui::test]
fn the_line_a_snap_lands_on_is_lit_as_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "inference-on-line");
    m.edit_sketch(cx);
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    let start = m.screen(cx, [45., -10., 0.]);
    m.pointer.move_to(cx, &m.view, start);
    m.pointer.click(cx, &m.view, start);
    let p = m.screen(cx, SK_LINE);
    approach(cx, &m, p);
    let lit = cx.update(|cx| m.view.read(cx).sketch.as_ref().is_some_and(|ed| !ed.inference_lit(&m.view.read(cx).sketch_data(cx).expect("sketch data").sketch).is_empty()));
    assert!(lit, "the snap lands on the line");
    let c = m.crop(cx, p);
    dump("cell-inference_on_line", &c);
    // Along the line 8 to 14 mm on from the pointer, clear of the rubber band and the cursor's glyphs.
    let (mut through, mut opaque) = (0, 0);
    for k in 0..=12 {
        let q = m.screen(cx, [SK_LINE[0] + 8.0 + 0.5 * k as f64, 5., 0.]) - p + DVec2::new(40.0, 40.0);
        let col = (q.y as usize - 1..=q.y as usize + 1).map(|y| c[y * 80 + q.x as usize]);
        through += col.clone().any(|c| close(c, 0xc79a95, 20)) as usize;
        opaque += col.filter(|c| close(*c, 0xecb78a, 12)).count();
    }
    assert!(through >= 8 && opaque == 0, "{through} of 13 samples blue through, {opaque} px #ecb78a");
}

// Live hover-matrix-recheck-1123 cells 1-3: the Line tool's end snapped onto a point, a midpoint or a line lies under the
// lit target (the point's disc #C3B89C, the band #D6BD95 at the pointer); no solid #449CCD dot on top.
#[gpui::test]
fn the_line_end_dot_lies_under_the_lit_target(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    for (name, target) in [("point", SK_POINT), ("midpoint", SK_LINEMID), ("line", SK_LINE)] {
        let m = matrix_studio(cx, &format!("inference-end-dot-{name}"));
        m.edit_sketch(cx);
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
        let start = m.screen(cx, [45., -10., 0.]);
        m.pointer.move_to(cx, &m.view, start);
        m.pointer.click(cx, &m.view, start);
        let p = m.screen(cx, target);
        approach(cx, &m, p);
        let c = m.crop(cx, p);
        dump(&format!("cell-inference_end_dot_{name}"), &c);
        let blue = near_centre(&c, 2.0).filter(|q| close(*q, 0x449ccd, 30)).count();
        let under = near_centre(&c, 1.0).filter(|q| close(*q, if name == "point" { 0xc3b89c } else { 0xd6bd95 }, 30)).count();
        assert!(blue == 0 && under >= 1, "{name}: {blue} #449ccd px on top, {under} px of the dot under the lit target");
    }
}

const SK_LINESTART: [f64; 3] = [60., 5., 0.];

fn sketch_blue(c: [u8; 3]) -> bool {
    c[2] >= 180 && c[0] <= 90 && c[1] <= 90
}

// Combined states in sketch edit (live skasm/hm-sk-1122 c1a): a selected line's band covers its own unlit end point (the
// blue line through its middle, #c78436, no blue dot on top); the end hovered, its light-orange disc (rim #fec788) lies
// over the band.
#[gpui::test]
fn sk_a_selected_lines_band_covers_its_end_and_the_hovered_end_lies_over_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "linesel-hover-end");
    m.edit_sketch(cx);
    let line = m.screen(cx, SK_LINE);
    m.pointer.move_to(cx, &m.view, line);
    m.pointer.click(cx, &m.view, line);
    m.park(cx);
    let end = m.screen(cx, SK_LINESTART);
    let c = m.crop(cx, end);
    dump("cell-sk_c1a_before", &c);
    assert!(shows(&c, 0xc88536, 1.0) && !near_centre(&c, 1.0).any(sketch_blue), "{:?}", near_centre(&c, 2.0).collect::<Vec<_>>());
    approach(cx, &m, end);
    let c = m.crop(cx, end);
    dump("cell-sk_c1a", &c);
    assert!(shows(&c, 0xfec788, 5.5) && !shows(&c, 0xc88536, 1.0) && band_beside_of(&c, 0xffa90a), "{:?}", near_centre(&c, 5.5).collect::<Vec<_>>());
}

// Line or Circle on, before the first click, over a line (live skasm/hm-sk-1122 c4a, c4d): the line's band with the blue
// through it and the coincident glyph; no preview dot at the pointer.
#[gpui::test]
fn sk_a_drawing_tool_over_a_line_lights_it_without_a_preview_dot(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "tool-hover-line");
    m.edit_sketch(cx);
    let p = m.screen(cx, SK_LINE);
    for tool in [Tool::Line, Tool::Circle] {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.activate_tool(tool, w, cx))).unwrap();
        approach(cx, &m, p);
        let c = m.crop(cx, p);
        dump("cell-sk_c4", &c);
        assert!(shows(&c, 0xffc685, 3.0) && shows(&c, 0xc89c96, 1.5) && !shows(&c, 0x449ccd, 4.0), "{:?}: {:?}", tool, near_centre(&c, 4.0).collect::<Vec<_>>());
        m.park(cx);
    }
}


// Fillet's radius arrow stays at the edge's midpoint wherever the edge was picked (live qa walk 6 (5), walk6-m5: picks at
// (40, 5, 10) and (40, 20, 10) give pixel-identical frames; std edgeBlendCommon.fs places it at the edge's parameter 0.5).
#[gpui::test]
fn the_fillet_arrow_stays_at_the_edges_midpoint_wherever_it_was_picked(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "fillet-arrow-anchor");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    draw(cx, m.window);
    m.park(cx);
    let pick = |cx: &mut TestAppContext, at: [f64; 3]| {
        let p = m.exact(cx, at);
        approach(cx, &m, p);
        m.pointer.click(cx, &m.view, p);
        m.park(cx);
        cx.update(|cx| m.view.read(cx).param_arrow(cx).map(|a| a.base))
    };
    let near = pick(cx, [40., 5., 20.]).expect("the radius arrow, picked near the front end");
    let d = cx.update(|cx| m.view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).expect("the Fillet dialog");
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(Vec::new()), cx)));
    draw(cx, m.window);
    let far = pick(cx, [40., 25., 20.]).expect("the radius arrow, picked near the back end");
    let mid = DVec3::new(0.04, 0.015, 0.02);
    assert!(near.distance(mid) < 1e-9 && far.distance(mid) < 1e-9, "{near:?} / {far:?}, the midpoint {mid:?}");
}

// Live workflow 07 (qa FEEL-PATTERN-FAIL-EDGE-RED, PATTERN-FAIL-PREVIEW): a Feature pattern of the boss whose instance
// lands off the block fails; its picked Direction edge keeps the selection orange and the instance it couldn't make is
// drawn as a red see-through body at its place (ours drew the edge #842228 and no instance).
#[gpui::test]
fn a_failing_feature_patterns_direction_stays_orange_and_its_instance_shows_red(cx: &mut TestAppContext) {
    use noshape_model::params::ParamValue;
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[-20, -15], [20, 15]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[10, 0], [14, 0]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "ADD", "defaultScope": true } },
    ] });
    let (window, view) = part_studio_from(cx, "pattern-fail-look", Some(start));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let (boss, edge) = cx.update(|cx| {
        let v = view.read(cx);
        let boss = v.studio(cx).and_then(|st| st.features.iter().find(|f| f.name == "Extrude 2").map(|f| f.id)).expect("Extrude 2");
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let front_top = |a: &[f32; 3]| (a[1] + 0.015).abs() < 1e-6 && (a[2] - 0.010).abs() < 1e-6;
        let edge = b.edge_refs.iter().zip(&b.mesh.edges).find(|(_, pl)| matches!((pl.first(), pl.last()), (Some(a), Some(z)) if front_top(a) && front_top(z) && (a[0] - z[0]).abs() > 0.039));
        (boss, edge.expect("the block's front top edge").0.clone())
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("linearPattern", w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Linear pattern dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.set_value("patternType", ParamValue::Enum("FEATURE".into()), cx);
        d.set_value("instanceFunction", ParamValue::Query(vec![Reference::Feature(boss)]), cx);
        d.user_focus_query("directionOne");
        d.add_pick(edge.clone(), cx);
    }));
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        let d = dialog.read(cx);
        !v.ui_busy() && d.feature.params.query("directionOne").len() == 1 && matches!(d.status, Some((noshape_model::display::FeatureStatus::Error, _)))
    });
    let state = cx.update(|cx| {
        let v = view.read(cx);
        let id = dialog.read(cx).feature.id;
        v.display.feature_state(id).map(|s| (s.message.clone(), s.faulty.clone(), s.error_mesh.is_some()))
    });
    let (px, w) = super::sketch_hover::settled_frame(cx, window, &view);
    let scale = w as f64 / cx.update(|cx| view.read(cx).viewport.read(cx).size().x).round();
    let rgb = |x: i32, y: i32| -> Option<[i32; 3]> {
        let o = (y.max(0) as usize * w + x.max(0) as usize) * 4;
        px.get(o..o + 3).map(|b| [b[2] as i32, b[1] as i32, b[0] as i32])
    };
    let screen = |cx: &mut TestAppContext, p: DVec3| cx.update(|cx| view.read(cx).viewport.read(cx).project(p)).map(|s| ((s.x * scale).round() as i32, (s.y * scale).round() as i32));
    const SELECT: [i32; 3] = [0xFF, 0xA9, 0x0A];
    let mut off = Vec::new();
    for k in 1..10 {
        let Some((x, y)) = screen(cx, DVec3::new(-0.020 + 0.004 * k as f64, -0.015, 0.010)) else { continue };
        let r = (2.0 * scale).ceil() as i32;
        let near = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter_map(|(dx, dy)| rgb(x + dx, y + dy)).any(|c| (0..3).all(|i| (c[i] - SELECT[i]).abs() <= 24));
        if !near {
            off.push(format!("x {} mm: {:?}", -20 + 4 * k, rgb(x, y)));
        }
    }
    // The instance 25 mm on: a disc of 8 mm at (35, 0), 0..20 mm high. Red where it's seen over the white background.
    let red = (0..=8)
        .filter_map(|k| screen(cx, DVec3::new(0.035, 0.0, 0.0025 * k as f64)))
        .filter(|&(x, y)| rgb(x, y).is_some_and(|c| c[0] > c[1] + 50 && c[0] > c[2] + 40))
        .count();
    assert!(off.is_empty() && red >= 5, "the Direction edge not orange at {off:?}; {red} of 9 samples red up the instance's axis; the pattern's state {state:?}");
}

// Section view through Right (live workflow 28, qa FEEL-SECTION-PLANE-LABEL-LOOK): the planes' names stay their solid
// blue (live (39, 94, 181)), never the pale mauve ours drew (138, 117, 129).
#[gpui::test]
fn plane_names_keep_their_blue_in_a_section_view(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "section-plane-names");
    let at = cx.update(|cx| {
        let v = m.view.read(cx);
        let pl = v.display.planes.iter().find(|q| q.reference == Reference::Plane(DatumPlane::Top))?;
        // The name is 18.4 px to its ascender at any zoom (builder PLANE_NAME_ASC_PX): about its middle.
        let vp = v.viewport.read(cx);
        let px = noshape_scene::camera_ops::pixel_size_at(&vp.camera, pl.corner(-1.0, 1.0), vp.size());
        vp.project(pl.corner(-1.0, 1.0) + pl.frame.x * (22.0 * px) - pl.frame.y * (9.0 * px))
    });
    let at = at.expect("Top's name on screen").round();
    let blue = |c: &[[u8; 3]]| c.iter().filter(|p| close(**p, 0x275eb5, 48)).count();
    let plain = blue(&m.crop(cx, at));
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| {
        v.selection.clear();
        v.section_through(Reference::Plane(DatumPlane::Right), w, cx);
        v.end_section_dialog(true, w, cx);
    })).unwrap();
    m.park(cx);
    let crop = m.crop(cx, at);
    let sectioned = blue(&crop);
    let mauve = crop.iter().filter(|p| close(**p, 0x8a7581, 20)).count();
    assert!(plain > 20 && sectioned * 10 >= plain * 8 && mauve < 5, "Top's name: {plain} blue px plain, {sectioned} in the section view, {mauve} mauve");
}

// A plane's name keeps its size on screen whatever the zoom (live client: PLANE_TEXT_HEIGHT_PX 20; nav-feel-1122 cube Top,
// qa FEEL-PLANE-LABEL-NORMAL-VIEW: ours grew with the plane). Live circmid-1698 0-top-view: "Top" inks 18 rows, T's top to
// p's tail.
#[gpui::test]
fn a_plane_name_keeps_its_screen_size_at_any_zoom(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "plane-name-size");
    let mut rows = Vec::new();
    for wpp in [0.2845e-3, 0.1e-3] {
        let at = cx.update(|cx| {
            let pl = m.view.read(cx).display.planes.iter().find(|q| q.reference == Reference::Plane(DatumPlane::Top)).cloned().expect("Top");
            let vp = m.view.read(cx).viewport.clone();
            vp.update(cx, |vp, cx| {
                let size = vp.size();
                let corner = pl.corner(-1.0, 1.0);
                vp.camera.rotation = noshape_render::Camera::look_rotation(-DVec3::Z, DVec3::Y);
                vp.camera.distance = wpp * size.y / (2.0 * (vp.camera.fov_y * 0.5).tan());
                // The corner at (200, 150) from the top left.
                vp.camera.target = corner + DVec3::X * ((size.x / 2.0 - 200.0) * wpp) - DVec3::Y * ((size.y / 2.0 - 150.0) * wpp);
                vp.request_frame(cx);
                vp.project(corner).expect("the corner on screen")
            })
        });
        m.pointer.frames(cx, &m.view);
        let crop = m.crop(cx, at.round() + DVec2::new(30.0, 30.0));
        let ink: Vec<usize> = (0..80).filter(|y| (0..80).any(|x| close(crop[y * 80 + x], 0x275eb5, 48))).collect();
        let (Some(lo), Some(hi)) = (ink.first(), ink.last()) else { panic!("no name at {wpp} m/px") };
        rows.push(hi - lo + 1);
    }
    assert!(rows.iter().all(|r| (16..=20).contains(r)) && rows[0].abs_diff(rows[1]) <= 1, "Top's name {rows:?} px tall at 0.28 and 0.1 mm/px; live 18 at any zoom");
}

// Section view through Right on the bracket (live workflow 28, qa FEEL-SECTION-PLANE-LABEL-LOOK): the cut part's edges
// stay 1 px; ours outlined the opened wall and the part in ~3 px dark bands (the back faces' silhouette, which the cut
// leaves nothing in front of).
#[gpui::test]
fn a_cut_parts_edges_stay_thin_in_a_section_view(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let m = matrix_studio_from(cx, "section-thin-edges", starts["bracket"].clone());
    // The wall's front face purple (workflow 28 steps 18-21: Add appearance to face, C890FF).
    let p = m.exact(cx, [-10., 25., 20.]).round();
    approach(cx, &m, p);
    let face = cx.update(|cx| m.view.read(cx).hover.clone()).expect("the wall's face");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| {
        v.edit_appearance(crate::part_studio::appearance::EditTarget::Faces(vec![face], None), w, cx);
        v.set_edit_color([200, 144, 255], w, cx);
    })).unwrap();
    cx.update(|cx| m.view.update(cx, |v, cx| v.accept_appearance(cx)));
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| {
        v.selection.clear();
        v.section_through(Reference::Plane(DatumPlane::Right), w, cx);
        v.end_section_dialog(true, w, cx);
    })).unwrap();
    m.park(cx);
    // Each dark pixel's thickness (its shorter run across or down), the 75th percentile: the purple face's top edge and
    // the cap's edge.
    let p75 = |crop: &[[u8; 3]]| {
        let dark = |x: i32, y: i32| (0..80).contains(&x) && (0..80).contains(&y) && crop[(y * 80 + x) as usize].iter().all(|c| *c < 80);
        let run = |x: i32, y: i32, dx: i32, dy: i32| (1..).take_while(|k| dark(x + k * dx, y + k * dy)).count() + (1..).take_while(|k| dark(x - k * dx, y - k * dy)).count() + 1;
        let mut thick: Vec<usize> = (0..80).flat_map(|y| (0..80).map(move |x| (x, y))).filter(|&(x, y)| dark(x, y)).map(|(x, y)| run(x, y, 1, 0).min(run(x, y, 0, 1))).collect();
        thick.sort_unstable();
        (thick.len(), thick.get(thick.len() * 3 / 4).copied().unwrap_or(0))
    };
    let mut wrong = Vec::new();
    for (name, at) in [("the purple face's top edge", [-10., 25., 30.]), ("the cap's edge", [0., 25., 30.])] {
        let p = m.exact(cx, at).round();
        let (n, t) = p75(&m.crop(cx, p));
        if n <= 20 || t > 2 {
            wrong.push(format!("{name}: {n} dark px, thickness p75 {t}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// The bracket (ui-parity start), Sketch 3's rib line in front of the base's top face (live workflow 36 steps 1 / 27, qa
// FEEL-PICK-SKETCH-LINE-THROUGH-FACE): a click 8 px beside the line takes the face (live's Area 1000 mm²), 6 px the line
// (live reach-px-1119: over a face an edge takes 6 px in, the face 8).
#[gpui::test]
fn a_click_8_px_beside_a_sketch_line_over_a_face_takes_the_face(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let m = matrix_studio_from(cx, "bracket-rib-pick", starts["bracket"].clone());
    let (a, b, f) = (m.exact(cx, [0., 5., 5.]), m.exact(cx, [0., 25., 25.]), m.exact(cx, [0., 12., 5.]));
    let along = (b - a).normalize();
    let foot = a + along * (f - a).dot(along);
    let out = (f - foot).normalize();
    let mut wrong = Vec::new();
    for (px, line) in [(8.0, false), (9.0, false), (6.0, true)] {
        let p = (foot + out * px).round();
        approach(cx, &m, p);
        m.pointer.click(cx, &m.view, p);
        let sel = cx.update(|cx| m.view.read(cx).selection.clone());
        let took_line = matches!(sel.as_slice(), [Reference::SketchEntity { .. }]);
        let took_face = matches!(sel.as_slice(), [Reference::Face(_)]);
        if (line && !took_line) || (!line && !took_face) {
            wrong.push(format!("{px} px beside the line: {sel:?}"));
        }
        m.reset(cx);
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// Live hover-list-1122 p01 (list × viewport matrix): Extrude 1's row hovered lights Part 1 as its part row does: the
// #ffc685 outline round its image, its inner edges plain dark.
#[gpui::test]
fn a_feature_rows_hover_outlines_the_part_it_made(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let m = matrix_studio_from(cx, "row-hover-outline", starts["overlap"].clone());
    let f = cx.update(|cx| m.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 1").map(|b| b.id.feature).expect("Part 1"));
    m.park(cx);
    m.view.update(cx, |v, cx| {
        v.ui.list_hover = Some(f);
        v.refresh_scene(false, cx);
    });
    let lit = |p: &[u8; 3]| close(*p, 0xffc685, 40);
    let mut got = Vec::new();
    // The front bottom edge is its outline, the front top edge between two of its faces.
    for (name, at) in [("outline", [20., 0., 0.]), ("inner edge", [20., 0., 10.])] {
        let p = m.exact(cx, at).round();
        let crop = m.crop(cx, p);
        got.push((name, (36..45).flat_map(|y| (36..45).map(move |x| (x, y))).filter(|&(x, y)| lit(&crop[y * 80 + x])).count()));
    }
    assert!(got[0].1 >= 3 && got[1].1 == 0, "lit px near (outline, inner edge): {got:?}");
}

// Live workflow 19 (overlap; qa FEEL-QUERY-PART-PICK-FILL): a part in a dialog's field carries live's orange band round
// its whole image, through the part in front: Mirror's Part 2 (step 2; ours drew none under the preview), Subtract's
// Tools Part 2 (step 10; ours stopped at Part 1) and its Targets Part 1 (step 12; ours none).
#[gpui::test]
fn a_part_in_a_field_is_banded_through_the_part_in_front(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let m = matrix_studio_from(cx, "field-part-band", starts["overlap"].clone());
    let parts: Vec<Reference> = cx.update(|cx| m.view.read(cx).display.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect());
    assert_eq!(parts.len(), 2, "Part 1 and Part 2");
    let band = |p: &[u8; 3]| close(*p, 0xffc04c, 40);
    // Part 2's corner (30, 10, 0) lies inside Part 1.
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("mirror", w, cx))).unwrap();
    let d = cx.update(|cx| m.view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(vec![parts[1].clone()]), cx)));
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    m.park(cx);
    let at = m.exact(cx, [30., 10., 0.]).round();
    let crop = m.crop(cx, at);
    let near = (36..45).flat_map(|y| (36..45).map(move |x| (x, y))).filter(|&(x, y)| band(&crop[y * 80 + x])).count();
    let mut wrong = Vec::new();
    if near < 3 {
        wrong.push(format!("Mirror's Part 2: {near} band px at its corner, {} in the crop", crop.iter().filter(|p| band(p)).count()));
    }
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| {
        v.cancel_edit(w, cx);
        v.start_feature("booleanBodies", w, cx);
    }))
    .unwrap();
    let d = cx.update(|cx| m.view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("operationType", noshape_model::ParamValue::Enum("SUBTRACTION".into()), cx)));
    // Part 1's corner (40, 0, 0) at the front.
    for (step, field, part, corner) in [("10 Tools Part 2", "tools", 1, [30., 10., 0.]), ("12 Targets Part 1", "targets", 0, [40., 0., 0.])] {
        // The field clicked first (its list takes the keyboard, as a click does), then the part.
        cx.update_window(m.window, |_, w, cx| d.update(cx, |d, cx| {
            d.user_focus_query(field);
            if let Some(h) = d.tab_stop_handle(&format!("q:{field}")) {
                w.focus(&h, cx);
            }
            d.set_value(field, noshape_model::ParamValue::Query(vec![parts[part].clone()]), cx);
        }))
        .unwrap();
        wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
        m.park(cx);
        let at = m.exact(cx, corner).round();
        let crop = m.crop(cx, at);
        let near = (36..45).flat_map(|y| (36..45).map(move |x| (x, y))).filter(|&(x, y)| band(&crop[y * 80 + x])).count();
        if near < 3 {
            let why = cx.update(|cx| {
                let v = m.view.read(cx);
                let s = v.viewport.read(cx).scene();
                let banded = s.states.iter().filter(|st| *st & noshape_render::state::BAND != 0).count();
                (v.display.bodies.iter().map(|b| b.id.clone()).collect::<Vec<_>>(), s.body_outline, banded, v.ui.removed_outline.as_ref().map(|(_, o)| o.is_some()), d.read(cx).active_query.clone(), d.read(cx).status.clone())
            });
            wrong.push(format!("step {step}: {near} band px at its corner, {} in the crop; (bodies, body_outline, BAND states, removed outline, active field, status) {why:?}", crop.iter().filter(|p| band(p)).count()));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// Live's list × viewport matrix, Part Studio (hover-list-1122, user P0): its document (Part 1 block, Part 2 cylinder,
// Part 3 plate), rows hovered and selected, what the viewport shows. The list's own look is list_feel_tests'.
#[gpui::test]
fn the_list_viewport_matrix_matches_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[60, 15], [68, 15]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "15 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 3", "plane": "Top", "shapes": [{ "corner_rectangle": [[80, 0], [100, 20]] }] },
        { "type": "extrude", "name": "Extrude 3", "params": { "entities": ["Sketch 3 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }
    ] });
    let m = matrix_studio_from(cx, "list-viewport", start);
    let solids = |cx: &gpui_kit::App| m.view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count();
    wait_until(cx, m.window, |cx| solids(cx) == 3 && !m.view.read(cx).ui_busy());
    cx.update(|cx| {
        let vp = m.view.read(cx).viewport.clone();
        vp.update(cx, |vp, cx| {
            vp.camera.target = DVec3::new(0.05, 0.015, 0.01);
            vp.request_frame(cx);
        });
    });
    m.park(cx);
    let feature = |cx: &mut TestAppContext, name: &str| cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == name).map(|f| f.id).expect(name));
    let part = |cx: &mut TestAppContext, name: &str| cx.update(|cx| m.view.read(cx).display.bodies.iter().find(|b| b.name == name).map(|b| Reference::Body(b.id.clone())).expect(name));
    let set = |cx: &mut TestAppContext, f: &dyn Fn(&mut PartStudioView)| {
        m.view.update(cx, |v, cx| {
            f(v);
            v.refresh_scene(false, cx);
        });
    };
    // Pixels near `at` (a square of radius r) within `tol` of a colour; pixels two frames differ in.
    let near = |frame: &(u32, u32, Vec<[u8; 3]>), at: DVec2, r: i32, rgb: u32, tol: i32| -> usize {
        let (w, h, px) = frame;
        let (x0, y0) = (at.x as i32, at.y as i32);
        (y0 - r..=y0 + r).flat_map(|y| (x0 - r..=x0 + r).map(move |x| (x, y))).filter(|&(x, y)| x >= 0 && y >= 0 && x < *w as i32 && y < *h as i32 && close(px[(y * *w as i32 + x) as usize], rgb, tol)).count()
    };
    let changed = |a: &(u32, u32, Vec<[u8; 3]>), b: &(u32, u32, Vec<[u8; 3]>)| a.2.iter().zip(&b.2).filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 8)).count();
    // Part 1's front bottom edge is on its silhouette, its front top edge between two of its faces.
    let p1_outline = m.exact(cx, [20., 0., 0.]).round();
    let p1_inner = m.exact(cx, [20., 0., 20.]).round();
    let p1_top = m.exact(cx, [20., 15., 20.]).round();
    let p2_outline = m.exact(cx, [60., 7., 0.]).round();
    let p2_top_outline = m.exact(cx, [60., 7., 15.]).round();
    let p3_top = m.exact(cx, [90., 10., 10.]).round();
    let (e1, e2, e3, s2, s3) = (feature(cx, "Extrude 1"), feature(cx, "Extrude 2"), feature(cx, "Extrude 3"), feature(cx, "Sketch 2"), feature(cx, "Sketch 3"));
    let (part1, part2, part3) = (part(cx, "Part 1"), part(cx, "Part 2"), part(cx, "Part 3"));
    let top_face = cx.update(|cx| {
        let v = m.view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.name == "Part 1").expect("Part 1");
        let on_top = |&(first, count): &(u32, u32)| b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| (b.mesh.positions[k as usize][2] - 0.02).abs() < 1e-6);
        b.face_refs.iter().zip(&b.mesh.face_ranges).find(|(_, r)| on_top(r)).map(|(r, _)| r.clone()).expect("Part 1's top face")
    });
    let mut wrong = Vec::new();

    // p01: Extrude 1's row: Part 1's silhouette #ffc685, its inner edges plain.
    set(cx, &|v| v.ui.list_hover = Some(e1));
    let f = m.frame(cx);
    let (out, inner) = (near(&f, p1_outline, 4, 0xffc685, 40), near(&f, p1_inner, 3, 0xffc685, 40));
    if out < 3 || inner > 0 {
        wrong.push(format!("p01 Extrude 1's row: {out} hover px at Part 1's outline (want ≥ 3), {inner} at its inner edge (want 0)"));
    }
    set(cx, &|v| v.ui.list_hover = None);
    // p03: Part 2's row: its silhouette.
    let p2 = part2.clone();
    set(cx, &move |v| v.hover = Some(p2.clone()));
    let n = near(&m.frame(cx), p2_outline, 4, 0xffc685, 40);
    if n < 3 {
        wrong.push(format!("p03 Part 2's row: {n} hover px at its outline"));
    }
    set(cx, &|v| v.hover = None);
    // p02: Sketch 2's row (consumed, hidden): its circle lights, Part 2 doesn't.
    set(cx, &|v| v.ui.list_hover = Some(s2));
    let f02 = m.frame(cx);
    let (circle, top) = (near(&f02, p2_outline, 4, 0xffc685, 40), near(&f02, p2_top_outline, 3, 0xffc685, 40));
    if circle < 3 || top > 0 {
        let c = f02.2[(p2_outline.y as u32 * f02.0 + p2_outline.x as u32) as usize];
        let wide = near(&f02, p2_outline, 6, 0xffc685, 60);
        wrong.push(format!("p02 hidden Sketch 2's row: {circle} hover px on its circle (want ≥ 3; {wide} within 6 px / 60, at it {c:02x?}), {top} on Part 2's top outline (want 0)"));
    }
    set(cx, &|v| v.ui.list_hover = None);
    // p04: the Top plane's row: its outline. The pointer off the graphics, on the list's empty space, as over the row
    // (left in the graphics, the viewport's own hover pick took the row's hover back: dx's land 11-23 02:13).
    let below = cx.update_window(m.window, |_, w, _| w.viewport_size()).unwrap();
    gpui_kit::VisualTestContext::from_window(m.window, cx).simulate_mouse_move(gpui_kit::point(px(60.), below.height - px(60.)), None, gpui_kit::Modifiers::default());
    m.pointer.frames(cx, &m.view);
    set(cx, &|v| v.hover = Some(Reference::Plane(DatumPlane::Top)));
    // Points along its four edges on screen: the viewport's size (and so which edges parts hide) differs by platform.
    let (size, edge) = cx.update(|cx| {
        let v = m.view.read(cx);
        let vp = v.viewport.read(cx);
        let Some(pl) = v.display.planes.iter().find(|q| q.reference == Reference::Plane(DatumPlane::Top)) else { return (vp.size(), Vec::new()) };
        let along = [-0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75];
        let uw = along.iter().flat_map(|&t| [(t, -1.0), (t, 1.0), (-1.0, t), (1.0, t)]);
        (vp.size(), uw.filter_map(|(u, w)| vp.project(pl.corner(u, w))).filter(|s| s.x > 10.0 && s.y > 10.0 && s.x < vp.size().x - 10.0 && s.y < vp.size().y - 10.0).collect::<Vec<_>>())
    });
    let f04 = m.frame(cx);
    let lit = edge.iter().map(|e| near(&f04, e.round(), 4, 0xffc685, 40)).collect::<Vec<_>>();
    if !lit.iter().any(|&n| n >= 3) {
        let hover = cx.update(|cx| m.view.read(cx).hover.clone());
        wrong.push(format!("p04 the Top plane's row: its outline not lit (viewport {size:?}, hover {hover:?}; hover px at {} points along its edges: {lit:?})", edge.len()));
    }
    set(cx, &|v| v.hover = None);

    // p05: Part 1's top face selected; p06-p09: either part's rows hovered draw the part's silhouette, the face stays.
    let tf = top_face.clone();
    set(cx, &move |v| v.selection = vec![tf.clone()]);
    let n = near(&m.frame(cx), p1_top, 2, 0xe4aa38, 30);
    if n < 20 {
        wrong.push(format!("p05 Part 1's top face selected: {n} #e4aa38 px"));
    }
    for (cell, row, feature, outline) in [("p06 Extrude 2", None, Some(e2), p2_outline), ("p07 Extrude 1", None, Some(e1), p1_outline), ("p08 Part 1", Some(part1.clone()), None, p1_outline), ("p09 Part 2", Some(part2.clone()), None, p2_outline)] {
        set(cx, &move |v| (v.hover, v.ui.list_hover) = (row.clone(), feature));
        let f = m.frame(cx);
        let (out, face) = (near(&f, outline, 4, 0xffc685, 40), near(&f, p1_top, 2, 0xe4aa38, 30));
        if out < 3 || face < 20 {
            wrong.push(format!("{cell}'s row with the face selected: {out} hover px at its part's outline (want ≥ 3), {face} selected px on the face (want ≥ 20)"));
        }
    }
    set(cx, &|v| (v.hover, v.ui.list_hover) = (None, None));
    set(cx, &|v| v.selection.clear());
    // p10-p12: geometry hovered lights no row.
    for (cell, at) in [("p10 a face of Part 2", m.exact(cx, [60., 15., 15.]).round()), ("p11 an edge of Part 1", p1_inner), ("p12 a face of Part 3", p3_top)] {
        m.pointer.move_to(cx, &m.view, at);
        let (lit, row) = cx.update(|cx| (m.view.read(cx).hover.clone(), m.view.read(cx).row_hovered_feature(cx)));
        if lit.is_none() || row.is_some() {
            wrong.push(format!("{cell} hovered: lit {lit:?}, row lit {row:?} (live: no row)"));
        }
    }
    m.park(cx);

    // p13: Extrude 1's row selected: Part 1 in a selected face's amber, one #ffa90a silhouette, inner edges plain.
    set(cx, &|v| v.selected_features = vec![e1]);
    m.frame(cx);
    let f13 = m.frame(cx);
    let top = near(&f13, p1_top, 2, 0xe4aa38, 30);
    let (out, inner) = (near(&f13, p1_outline, 4, 0xffa90a, 30), near(&f13, p1_inner, 3, 0xffa90a, 30));
    if top < 20 || out < 3 || inner > 0 {
        let c = f13.2[(p1_top.y as u32 * f13.0 + p1_top.x as u32) as usize];
        wrong.push(format!("p13 Extrude 1 selected: top face {c:02x?} (live #e4aa38), {out} #ffa90a px at the outline (want ≥ 3), {inner} at the inner edge (want 0)"));
    }
    // p14: hovering the selected row changes nothing.
    set(cx, &|v| v.ui.list_hover = Some(e1));
    let f14 = m.frame(cx);
    let n = changed(&f13, &f14);
    if n > 0 {
        let accent = cx.update(|cx| m.view.read(cx).highlighted_hover(cx));
        set(cx, &|v| v.ui.list_hover = None);
        let back = changed(&f13, &m.frame(cx));
        wrong.push(format!("p14 Extrude 1 selected, its row hovered: {n} px changed (live none); hovered refs {accent:?}, {back} px changed with the hover gone"));
    }
    // p15: another row hovered: its part's silhouette.
    set(cx, &|v| v.ui.list_hover = Some(e2));
    let n = near(&m.frame(cx), p2_outline, 4, 0xffc685, 40);
    if n < 3 {
        wrong.push(format!("p15 Extrude 1 selected, Extrude 2's row hovered: {n} hover px at Part 2's outline"));
    }
    set(cx, &|v| v.ui.list_hover = None);
    // p16: nor a face of the part it made.
    m.pointer.move_to(cx, &m.view, p1_top);
    let n = changed(&f13, &m.frame(cx));
    if n > 0 {
        wrong.push(format!("p16 Extrude 1 selected, Part 1's top face hovered: {n} px changed (live none)"));
    }
    m.park(cx);

    // p17: Part 1's row selected: the lighter part amber, the #ffc04c band.
    let p1 = part1.clone();
    set(cx, &move |v| {
        v.selected_features.clear();
        v.selection = vec![p1.clone()];
    });
    let f17 = m.frame(cx);
    let (top, out) = (near(&f17, p1_top, 2, 0xf0bb58, 24), near(&f17, p1_outline, 4, 0xffc04c, 30));
    if top < 20 || out < 3 {
        let c = f17.2[(p1_top.y as u32 * f17.0 + p1_top.x as u32) as usize];
        wrong.push(format!("p17 Part 1 selected: top face {c:02x?} (live #f0bb58), {out} #ffc04c px at the outline (want ≥ 3)"));
    }
    // p18, p19: its row hovered, its face hovered: no change.
    let p1 = part1.clone();
    set(cx, &move |v| v.hover = Some(p1.clone()));
    let n = changed(&f17, &m.frame(cx));
    if n > 0 {
        wrong.push(format!("p18 Part 1 selected, its row hovered: {n} px changed (live none)"));
    }
    set(cx, &|v| v.hover = None);
    m.pointer.move_to(cx, &m.view, p1_top);
    let n = changed(&f17, &m.frame(cx));
    if n > 0 {
        wrong.push(format!("p19 Part 1 selected, its top face hovered: {n} px changed (live none)"));
    }
    m.park(cx);
    // p19b: Part 2's row: its silhouette.
    let p2 = part2.clone();
    set(cx, &move |v| v.hover = Some(p2.clone()));
    let n = near(&m.frame(cx), p2_outline, 4, 0xffc685, 40);
    if n < 3 {
        wrong.push(format!("p19b Part 1 selected, Part 2's row hovered: {n} hover px at its outline"));
    }
    set(cx, &|v| v.hover = None);
    set(cx, &|v| v.selection.clear());

    // p20-p22: Extrude 1 and Extrude 3 selected: both parts amber; Extrude 2's row its silhouette, Extrude 3's nothing.
    set(cx, &|v| v.selected_features = vec![e1, e3]);
    m.frame(cx);
    let f20 = m.frame(cx);
    let (one, three) = (near(&f20, p1_top, 2, 0xe4aa38, 30), near(&f20, p3_top, 2, 0xda961b, 32));
    if one < 20 || three < 20 {
        wrong.push(format!("p20 Extrude 1 + 3 selected: {one} #e4aa38 px on Part 1's top, {three} #da961b px on Part 3's (want ≥ 20)"));
    }
    set(cx, &|v| v.ui.list_hover = Some(e2));
    let n = near(&m.frame(cx), p2_outline, 4, 0xffc685, 40);
    if n < 3 {
        wrong.push(format!("p21 Extrude 2's row: {n} hover px at Part 2's outline"));
    }
    set(cx, &|v| v.ui.list_hover = Some(e3));
    let n = changed(&f20, &m.frame(cx));
    if n > 0 {
        wrong.push(format!("p22 Extrude 3 selected, its row hovered: {n} px changed (live none)"));
    }
    set(cx, &|v| v.ui.list_hover = None);
    // p23, p24: Part 1 and Part 3 selected: both the lighter amber; Part 2's row its silhouette.
    let (p1, p3) = (part1.clone(), part3.clone());
    set(cx, &move |v| {
        v.selected_features.clear();
        v.selection = vec![p1.clone(), p3.clone()];
    });
    let f23 = m.frame(cx);
    let (one, three) = (near(&f23, p1_top, 2, 0xf0bb58, 24), near(&f23, p3_top, 2, 0xf0bb58, 32));
    if one < 20 || three < 20 {
        wrong.push(format!("p23 Part 1 + 3 selected: {one} / {three} #f0bb58 px on their tops (want ≥ 20)"));
    }
    let p2 = part2.clone();
    set(cx, &move |v| v.hover = Some(p2.clone()));
    let n = near(&m.frame(cx), p2_outline, 4, 0xffc685, 40);
    if n < 3 {
        wrong.push(format!("p24 Part 2's row: {n} hover px at its outline"));
    }
    set(cx, &|v| v.hover = None);
    set(cx, &|v| v.selection.clear());

    // p25-p27: Extrude 2 suppressed: its row lights nothing; Sketch 2, shown again, lights its circle.
    m.view.update(cx, |v, cx| v.set_suppressed(vec![e2], true, cx));
    wait_until(cx, m.window, |cx| solids(cx) == 2 && !m.view.read(cx).ui_busy());
    m.park(cx);
    let f25 = m.frame(cx);
    set(cx, &|v| v.ui.list_hover = Some(e2));
    let n = changed(&f25, &m.frame(cx));
    if n > 0 {
        wrong.push(format!("p26 suppressed Extrude 2's row hovered: {n} px changed (live none)"));
    }
    set(cx, &|v| v.ui.list_hover = Some(s2));
    let n = near(&m.frame(cx), p2_outline, 4, 0xffc685, 40);
    if n < 3 {
        wrong.push(format!("p27 Sketch 2's row (its Extrude suppressed): {n} hover px on its circle"));
    }
    set(cx, &|v| v.ui.list_hover = None);
    m.view.update(cx, |v, cx| v.set_suppressed(vec![e2], false, cx));
    wait_until(cx, m.window, |cx| solids(cx) == 3 && !m.view.read(cx).ui_busy());

    // p28-p31: rolled back to after Extrude 2: the rows below the bar light nothing, Extrude 1's still its part.
    let at = cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().position(|f| f.id == e2).expect("Extrude 2"));
    m.view.update(cx, |v, cx| v.set_rollback(Some(at + 1), cx));
    wait_until(cx, m.window, |cx| solids(cx) == 2 && !m.view.read(cx).ui_busy());
    m.park(cx);
    let f28 = m.frame(cx);
    for (cell, row) in [("p29 Extrude 3", e3), ("p30 Sketch 3", s3)] {
        set(cx, &move |v| v.ui.list_hover = Some(row));
        let n = changed(&f28, &m.frame(cx));
        if n > 0 {
            wrong.push(format!("{cell}'s row below the rollback bar hovered: {n} px changed (live none)"));
        }
    }
    set(cx, &|v| v.ui.list_hover = Some(e1));
    let n = near(&m.frame(cx), p1_outline, 4, 0xffc685, 40);
    if n < 3 {
        wrong.push(format!("p31 Extrude 1's row above the bar: {n} hover px at Part 1's outline"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// Live mc-infer-1122 reach-ps-corner d=6..0 (qa FEEL-MC-VERTEX-FROM-FACE): in the Mate connector feature a block's corner
// reached across its top face keeps the connector glyph on the vertex; hovered straight from empty space it has none.
#[gpui::test]
fn a_vertex_reached_from_its_face_keeps_the_connector_glyph(cx: &mut TestAppContext) {
    use noshape_scene::builder::{PickKind as K, PickTarget as T};
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } }
    ] });
    let m = matrix_studio_from(cx, "mc-vertex-glyph", start);
    wait_until(cx, m.window, |cx| !m.view.read(cx).display.bodies.is_empty() && !m.view.read(cx).ui_busy());
    let (top, corner) = cx.update(|cx| {
        let b = &m.view.read(cx).display.bodies[0];
        let on = |&(first, count): &(u32, u32)| b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| (b.mesh.positions[k as usize][2] - 0.02).abs() < 1e-6);
        let top = b.face_refs.iter().zip(&b.mesh.face_ranges).find(|(_, r)| on(r)).map(|(r, _)| r.clone()).expect("the top face");
        let k = b.mesh.vertices.iter().position(|p| (p[0] - 0.04).abs() < 1e-6 && (p[1] - 0.03).abs() < 1e-6 && (p[2] - 0.02).abs() < 1e-6).expect("the corner");
        (top, b.vertex_refs[k].clone())
    });
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("mateConnector", w, cx))).unwrap();
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    let at = m.exact(cx, [40., 30., 20.]);
    let hover = |cx: &mut TestAppContext, on: Option<(&Reference, K)>| {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| {
            v.ui.pointer = Some(at);
            v.ui.hover_raw = on.map(|(r, kind)| T { reference: r.clone(), kind, anchor: None });
            v.rehover(cx);
            v.update_inference(false, w, cx);
        }))
        .unwrap();
        if on.is_some() {
            wait_until(cx, m.window, |cx| m.view.read(cx).inference_marks().is_some());
        }
        cx.update(|cx| m.view.read(cx).inference_glyph_shown())
    };
    assert!(!hover(cx, Some((&corner, K::Vertex))), "straight from empty space: no glyph");
    hover(cx, None);
    assert!(hover(cx, Some((&top, K::Face))), "on the face");
    assert!(hover(cx, Some((&corner, K::Vertex))), "reached across the face: the glyph kept on the vertex");
    // Live mc-infer-1122 r6 ps-frontc (qa FEEL-MC-PS-ORIGIN-CORNER-MARKER): the front face's corner mark on the Origin
    // covers the Origin's glyph; the top face's marks don't reach it.
    let front = cx.update(|cx| {
        let b = &m.view.read(cx).display.bodies[0];
        let on = |&(first, count): &(u32, u32)| b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| b.mesh.positions[k as usize][1].abs() < 1e-6);
        b.face_refs.iter().zip(&b.mesh.face_ranges).find(|(_, r)| on(r)).map(|(r, _)| r.clone()).expect("the front face")
    });
    hover(cx, Some((&top, K::Face)));
    assert!(!cx.update(|cx| m.view.read(cx).origin_marked(cx)), "the top face: the Origin shown");
    hover(cx, Some((&front, K::Face)));
    assert!(cx.update(|cx| m.view.read(cx).origin_marked(cx)), "the front face: its corner mark on the Origin");
    assert_eq!(cx.update(|cx| m.view.read(cx).built.as_ref().map(|b| b.origin_glyphs.len())), Some(2), "the Origin's two glyph instances");
}

// Live mc-infer-1122, the Part Studio Mate connector feature (Part 1 block, Part 2 cylinder): Shift keeps the hovered face
// (the glyph moves among its points; off it, its centroid; released over the cylinder, the cylinder's top); after its one
// pick the feature infers no more, the placed connector's disc pale #d2e3ed, Part 1 outlined #ffc04c, and Part 2
// hovered outlined whole #ffc685 with no glyph.
#[gpui::test]
fn the_mate_connector_feature_infers_as_live(cx: &mut TestAppContext) {
    use noshape_scene::builder::{PickKind as K, PickTarget as T};
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[60, 15], [68, 15]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "15 mm", "operationType": "NEW" } }
    ] });
    let m = matrix_studio_from(cx, "mc-infer", start);
    let solids = |cx: &gpui_kit::App| m.view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count();
    wait_until(cx, m.window, |cx| solids(cx) == 2 && !m.view.read(cx).ui_busy());
    cx.update(|cx| {
        let vp = m.view.read(cx).viewport.clone();
        vp.update(cx, |vp, cx| {
            vp.camera.target = DVec3::new(0.035, 0.015, 0.01);
            vp.request_frame(cx);
        });
    });
    m.park(cx);
    let top_of = |cx: &mut TestAppContext, part: &str, z: f32| cx.update(|cx| {
        let v = m.view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.name == part).expect(part);
        let on = |&(first, count): &(u32, u32)| b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| (b.mesh.positions[k as usize][2] - z).abs() < 1e-6);
        b.face_refs.iter().zip(&b.mesh.face_ranges).find(|(_, r)| on(r)).map(|(r, _)| r.clone()).expect("its top face")
    });
    let (top, cyl_top) = (top_of(cx, "Part 1", 0.02), top_of(cx, "Part 2", 0.015));
    let part2 = cx.update(|cx| m.view.read(cx).display.bodies.iter().find(|b| b.name == "Part 2").map(|b| Reference::Body(b.id.clone())).unwrap());
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("mateConnector", w, cx))).unwrap();
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    let near_corner = m.exact(cx, [37., 3., 20.]);
    let (at_top, at_cyl) = (m.exact(cx, [20., 15., 20.]), m.exact(cx, [60., 15., 15.]));
    let centroid = DVec3::new(0.02, 0.015, 0.02);
    let mut wrong = Vec::new();
    // Hover (pointer, the pick under it, Shift) and what the inference chose, once its points came.
    let hover = |cx: &mut TestAppContext, at: DVec2, face: &Reference, shift: bool| {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| {
            v.ui.pointer = Some(at);
            v.ui.hover_raw = Some(T { reference: face.clone(), kind: K::Face, anchor: None });
            v.rehover(cx);
            v.update_inference(shift, w, cx);
            v.refresh_scene(false, cx);
        }))
        .unwrap();
        wait_until(cx, m.window, |cx| m.view.read(cx).inference_choice().is_some());
        cx.update(|cx| {
            let v = m.view.read(cx);
            (v.inference_choice().map(|p| p.frame.origin * 1e3), v.hover.clone())
        })
    };
    let (corner, _) = hover(cx, near_corner, &top, false);
    if corner.is_none_or(|c| c.distance(DVec3::new(40., 0., 20.)) > 1e-3) {
        wrong.push(format!("near the corner: {corner:?} (live the corner)"));
    }
    let (at_centre, _) = hover(cx, at_top, &top, true);
    if at_centre.is_none_or(|c| c.distance(centroid * 1e3) > 1e-3) {
        wrong.push(format!("Shift, at the centroid: {at_centre:?}"));
    }
    let (on_cyl, lit) = hover(cx, at_cyl, &cyl_top, true);
    if on_cyl.is_none_or(|c| c.distance(centroid * 1e3) > 1e-3) || lit.as_ref() != Some(&top) {
        wrong.push(format!("Shift held over the cylinder: glyph {on_cyl:?} (live the block top's centroid), lit {lit:?} (live the block's top)"));
    }
    let (released, lit) = hover(cx, at_cyl, &cyl_top, false);
    if released.is_none_or(|c| c.distance(DVec3::new(60., 15., 15.)) > 1e-3) || lit.as_ref() != Some(&cyl_top) {
        wrong.push(format!("Shift released over the cylinder: glyph {released:?} (live its top's centre), lit {lit:?}"));
    }
    // The pick at the block top's centroid.
    hover(cx, at_top, &top, false);
    m.view.update(cx, |v, cx| v.pick(T { reference: top.clone(), kind: K::Face, anchor: None }, cx));
    let dialog = cx.update(|cx| m.view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy() && !dialog.read(cx).feature.params.query("ownerPart").is_empty());
    m.park(cx);
    let f = m.frame(cx);
    let near = |frame: &(u32, u32, Vec<[u8; 3]>), at: DVec2, r: i32, rgb: u32, tol: i32| -> usize {
        let (w, h, px) = frame;
        let (x0, y0) = (at.x as i32, at.y as i32);
        (y0 - r..=y0 + r).flat_map(|y| (x0 - r..=x0 + r).map(move |x| (x, y))).filter(|&(x, y)| x >= 0 && y >= 0 && x < *w as i32 && y < *h as i32 && close(px[(y * *w as i32 + x) as usize], rgb, tol)).count()
    };
    let p1_outline = m.exact(cx, [20., 0., 0.]).round();
    let outline = near(&f, p1_outline, 4, 0xffc04c, 30);
    if outline < 3 {
        wrong.push(format!("after the pick: {outline} #ffc04c px at Part 1's outline (live its owner outline)"));
    }
    let centre = m.exact(cx, [20., 15., 20.]);
    let wedge = ((m.exact(cx, [21., 15., 20.]) - centre).normalize() + (m.exact(cx, [20., 16., 20.]) - centre).normalize()).normalize();
    let disc_at = (centre + wedge * 15.0).round();
    let (pale, tan) = (near(&f, disc_at, 3, 0xd2e3ed, 14), near(&f, disc_at, 3, 0xc2c187, 20));
    if pale < 10 || tan > 0 {
        let c = f.2[(disc_at.y as u32 * f.0 + disc_at.x as u32) as usize];
        wrong.push(format!("the placed connector's disc: {pale} #d2e3ed px, {tan} #c2c187 (at its middle {c:02x?})"));
    }
    // Part 1's corner again: no inference; Part 2 hovered: the whole part outlined, no glyph.
    m.pointer.move_to(cx, &m.view, near_corner.round());
    let marks = cx.update(|cx| m.view.read(cx).inference_marks().is_some());
    if marks {
        wrong.push("after the pick, Part 1's corner hovered: inference marks drawn (live none)".into());
    }
    m.pointer.move_to(cx, &m.view, at_cyl.round());
    let (lit, marks) = cx.update(|cx| (m.view.read(cx).highlighted_hover(cx), m.view.read(cx).inference_marks().is_some()));
    let filter = cx.update(|cx| dialog.read(cx).active_filter().map(|(q, f, _)| (q.to_string(), f)));
    let f = m.frame(cx);
    let (bottom, rim) = (m.exact(cx, [60., 7., 0.]).round(), m.exact(cx, [52., 15., 15.]).round());
    let (side, cap) = (near(&f, bottom, 4, 0xffc685, 40), near(&f, rim, 4, 0xffc685, 40));
    if lit != vec![part2.clone()] || marks || side < 3 || cap < 3 {
        wrong.push(format!("after the pick, Part 2's top hovered: lit {lit:?} (live Part 2), marks {marks}, {side} #ffc685 px at its bottom outline, {cap} at its top; field {filter:?}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live sel-see-through-1122: the block's bottom face alone selected (hidden in the iso view): the fills stay as they
/// were (no see-through amber), its boundary drawn orange over the part, so its back edge shows through the top.
#[gpui::test]
fn a_hidden_selected_faces_outline_shows_through_the_part(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "selected-see-through");
    let bottom = cx.update(|cx| {
        let v = m.view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the block");
        b.face_refs
            .iter()
            .enumerate()
            .find(|(fi, _)| b.mesh.face_ranges.get(*fi).is_some_and(|&(first, count)| b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| b.mesh.positions[k as usize][2].abs() < 1e-6)))
            .map(|(_, r)| r.clone())
            .expect("the bottom face")
    });
    m.park(cx);
    let orange = |c: [u8; 3]| c[0] > 225 && (130..=200).contains(&c[1]) && c[2] < 100;
    let back = m.screen(cx, [20., 30., 0.]);
    let top = m.screen(cx, [30., 8., 20.]);
    let (w, _, before) = m.frame(cx);
    let at = |px: &[[u8; 3]], p: DVec2| px[(p.y as u32 * w + p.x as u32) as usize];
    let crop_before = m.crop(cx, back);
    m.view.update(cx, |v, cx| {
        v.selection = vec![bottom.clone()];
        v.refresh_scene(false, cx);
    });
    let (_, _, after) = m.frame(cx);
    let crop_after = m.crop(cx, back);
    let (n0, n1) = (crop_before.iter().filter(|c| orange(**c)).count(), crop_after.iter().filter(|c| orange(**c)).count());
    let (t0, t1) = (at(&before, top), at(&after, top));
    let same = t0.iter().zip(t1).all(|(&a, b)| (i32::from(a) - i32::from(b)).abs() <= 3);
    assert!(n0 == 0 && n1 >= 40 && same, "orange px round the hidden back edge: {n0} before, {n1} selected (live: its line through the top); the top's fill {t0:?} → {t1:?} (live: unchanged)");
}

/// Qa FEEL-HOVER-DISC-AFTER-COMMIT (workflows-1123 03 steps 12-14): Enter commits the dialog with the pointer still on an
/// edge's midpoint; live lights nothing there until the pointer moves (ours kept the midpoint's pale disc).
#[gpui::test]
fn nothing_lights_under_the_still_pointer_after_enter(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "hover-held-after-enter");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    draw(cx, m.window);
    let p = m.exact(cx, EDGE);
    approach(cx, &m, p);
    m.pointer.click(cx, &m.view, p);
    let mid = m.exact(cx, MID);
    approach(cx, &m, mid);
    let before = cx.update(|cx| m.view.read(cx).hover.clone());
    assert!(before.is_some(), "the midpoint lit before Enter");
    gpui_kit::VisualTestContext::from_window(m.window, cx).simulate_keystrokes("enter");
    wait_until(cx, m.window, |cx| m.view.read(cx).edit.is_none() && !m.view.read(cx).ui_busy());
    for _ in 0..4 {
        m.pointer.frames(cx, &m.view);
    }
    let after = cx.update(|cx| m.view.read(cx).hover.clone());
    assert!(after.is_none(), "lit under the still pointer after Enter: {after:?}");
    approach(cx, &m, mid + DVec2::new(0.0, -3.0));
    assert!(cx.update(|cx| m.view.read(cx).hover.is_some()), "nothing lit once the pointer moved");
}

// Extrude 1 edited (live manip-feel-1122 edit-body-shade, qa FEEL-EDIT-BODY-SHADE): the top under the end cap reads live's
// flat (132,147,155) within 9 (the cap's blue grey once over the preview mix; #595959 read (131,140,147) and
// (138,148,154); drawn with the frame too, (118,127,133)).
#[gpui::test]
fn an_edited_extrudes_end_cap_greys_its_top_once(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "edit-body-shade");
    let id = cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").map(|f| f.id)).expect("Extrude 1");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
    m.park(cx);
    let tops: Vec<DVec2> = [[20., 15., 20.], [30., 8., 20.]].iter().map(|p| m.screen(cx, *p)).collect();
    let (w, _, px) = m.frame(cx);
    let at: Vec<[u8; 3]> = tops.iter().map(|p| px[(p.y as u32 * w + p.x as u32) as usize]).collect();
    assert!(at.iter().all(|c| close(*c, 0x84929b, 9)), "the top under the end cap {at:?}, live (132,146,155)");
}

/// Extrude 1 edited, no drag (live psui/edit-body-slider-1124, 5x5 medians, qa FEEL-EDIT-BODY-SHADE): the preview's sides
/// where the sight line misses the picked region, the region seen through them and the top under the end cap. Live's preview is 0.7 x the block
/// in its own color at its rest shade (the region's tint over it) + 0.3 x the model before (edit-body-slider-1124).
#[gpui::test]
fn an_edited_extrudes_region_shows_through_its_sides_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    // Live's document: the block alone.
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } }
    ] });
    let m = matrix_studio_from(cx, "edit-body-region", block);
    let id = cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").map(|f| f.id)).expect("Extrude 1");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
    m.park(cx);
    // (Not the right face's y = 5 column, over the datum planes, darker in ours at slider 0: FEEL-EDIT-BODY-SHADE. The
    // region through the right face: live's 9-point median, off its outline.)
    let live: [([f64; 3], u32); 12] = [
        ([5., 0., 10.], 0x8da0ab),
        ([5., 0., 18.], 0x8da0ab),
        ([20., 0., 3.], 0xa3a085),
        ([35., 0., 10.], 0xa3a085),
        ([40., 12., 4.], 0xb9baa3),
        ([40., 20., 5.], 0xb9baa3),
        ([40., 15., 18.], 0xabc5d5),
        ([40., 25., 10.], 0xabc5d5),
        ([40., 5., 18.], 0xa1bed0),
        ([30., 8., 20.], 0x84939c),
        ([5., 5., 20.], 0x84939c),
        ([35., 25., 20.], 0x84939c),
    ];
    let (w, _, px) = m.frame(cx);
    let mut wrong = Vec::new();
    for (p, rgb) in live {
        let s = m.screen(cx, p);
        let c = px[(s.y as u32 * w + s.x as u32) as usize];
        if !close(c, rgb, 6) {
            wrong.push(format!("{p:?}: ours {c:?}, live #{rgb:06x}"));
        }
    }
    // The region's outline through the body, its hidden far edges included: live #ffa90a about 3 px wide.
    let hidden = (1..10).map(|k| [4. * k as f64, 30., 0.]).chain((1..8).map(|k| [0., 4. * k as f64, 0.]));
    for p in hidden {
        let s = m.screen(cx, p);
        let near = (-1..=1).flat_map(|i| (-1..=1).map(move |j| (i, j))).any(|(i, j)| close(px[((s.y as i32 + j) as u32 * w + (s.x as i32 + i) as u32) as usize], 0xffa90a, 6));
        if !near {
            wrong.push(format!("{p:?}: no #ffa90a outline within 1 px"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}

/// A selected edge of a selected face is one solid #ffa90a band (live sel-gestures-1122 ps-box-crossing-result: the
/// face's boundary covers the band's core); alone it keeps its darker core (live hover-matrix-1121 edge-selected
/// (207,141,18)).
#[gpui::test]
fn a_selected_edge_of_a_selected_face_is_solid(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "hover-matrix-solid-band");
    let (edge, face) = (m.screen(cx, EDGE), m.screen(cx, FACE));
    let core = |c: &[[u8; 3]]| near_centre(c, 1.0).map(|p| p[0]).min().unwrap_or(0);
    m.pointer.move_to(cx, &m.view, edge);
    m.pointer.click(cx, &m.view, edge);
    m.park(cx);
    let alone = m.crop(cx, edge);
    assert!(core(&alone) < 225, "alone: its darker core ({:?})", near_centre(&alone, 1.0).collect::<Vec<_>>());
    m.pointer.move_to(cx, &m.view, face);
    m.pointer.click(cx, &m.view, face);
    m.park(cx);
    assert_eq!(cx.update(|cx| m.view.read(cx).selection.len()), 2, "the edge and its face selected");
    let both = m.crop(cx, edge);
    dump("edge-of-selected-face", &both);
    assert!(near_centre(&both, 1.5).all(|p| close(p, 0xffa90a, 16)), "one solid band ({:?})", near_centre(&both, 1.5).collect::<Vec<_>>());
}

/// Selected faces keep one hue on every side (live sel-gestures-1122 ps-box-crossing-result): the front (184,136,45),
/// the top (228,170,56).
#[gpui::test]
fn a_selected_front_face_is_the_tops_hue_darker(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "hover-matrix-front-fill");
    let (front, top) = (m.screen(cx, [20., 0., 10.]), m.screen(cx, FACE));
    for p in [front, top] {
        m.pointer.move_to(cx, &m.view, p);
        m.pointer.click(cx, &m.view, p);
    }
    m.park(cx);
    let (w, _, px) = m.frame(cx);
    let at = |p: DVec2| px[(p.y as u32 * w + p.x as u32) as usize];
    let (f, t) = (at(front), at(top));
    assert!(close(f, 0xb8882d, 6) && close(t, 0xe4aa38, 6), "front {f:?} (live (184,136,45)), top {t:?} (live (228,170,56))");
}

/// qa ERR-SM-BAD-SELECTION-RED (live errors-1123 sheet-metal fl-joint2): with a Flange on a joint edge of Flange 1's
/// bend open, the joint edge is drawn red on the part (#bd3039), not only flagged.
#[gpui::test]
fn a_flange_on_a_joint_edge_draws_red_pixels(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "sheetMetalStart", "name": "Sheet metal model 1", "params": { "process": "THICKEN", "regions": ["Sketch 1 region 0"], "thickness": "2 mm", "radius": "2 mm" } },
        { "type": "sheetMetalFlange", "name": "Flange 1", "params": { "edges": ["edge through [20, 0, 2]"] } },
        { "type": "sheetMetalFlange", "name": "Flange 2", "params": { "edges": ["edge through [60, 25, 2]"] } },
        { "type": "sheetMetalFlange", "name": "Flange 3", "params": { "edges": ["edge through [29, 2, 2]"] } },
    ] });
    let m = matrix_studio_from(cx, "sm-flange-joint-red-px", start);
    // From above: from the matrix's front view Flange 1's wall hides the joint edge.
    cx.update(|cx| {
        let vp = m.view.read(cx).viewport.clone();
        vp.update(cx, |vp, cx| {
            vp.camera.rotation = noshape_render::Camera::look_rotation(DVec3::new(0., 0., -1.), DVec3::Y);
            vp.camera.target = DVec3::new(0.03, 0.02, 0.0);
            vp.request_frame(cx);
        });
    });
    m.park(cx);
    let red = |cx: &mut TestAppContext| m.frame(cx).2.iter().filter(|c| close(**c, 0xbd3039, 40)).count();
    let before = red(cx);
    let flange = cx.update(|cx| m.view.read(cx).studio(cx).and_then(|st| st.features.iter().find(|f| f.name == "Flange 3").map(|f| f.id))).expect("Flange 3");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(flange, w, cx))).unwrap();
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy() && m.view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).status.is_some()));
    m.park(cx);
    let after = red(cx);
    assert!(after >= before + 20, "red px: {before} closed, {after} with the dialog open");
}

/// FEEL-EDIT-BODY-SHADE study: Extrude 1 edited, the frame's main colours (live manip-feel-1122 extrude-released: top
/// (132,144,156), front (168,196,212), the region through the front (184,184,164), through the left (164,160,132))
/// (cargo test -p noshape-part-studio-ui --lib print_edit_body_shade -- --ignored --nocapture).
#[gpui::test]
#[ignore = "T2: prints an edited Extrude's colours"]
fn print_edit_body_shade(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "edit-body-shade-study");
    let id = cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").map(|f| f.id)).expect("Extrude 1");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
    m.park(cx);
    let (w, _, px) = m.frame(cx);
    for (name, p) in [("top", [20., 15., 20.]), ("front", [20., 0., 10.]), ("front low", [20., 0., 2.]), ("right low", [40., 15., 2.]), ("bottom centre", [20., 15., 0.])] {
        let q = m.screen(cx, p);
        println!("== {name} {:?}", px[(q.y as u32 * w + q.x as u32) as usize]);
    }
    let mut count: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
    for p in px.iter().filter(|p| !(p[0] > 225 && p[1] > 225 && p[2] > 225)) {
        *count.entry([p[0] / 4 * 4, p[1] / 4 * 4, p[2] / 4 * 4]).or_default() += 1;
    }
    let mut top: Vec<_> = count.into_iter().collect();
    top.sort_by_key(|t| std::cmp::Reverse(t.1));
    for (c, n) in top.into_iter().take(12) {
        println!("  [{c:?}] {n}");
    }
}

/// ERR-TOOL-BODY-COLOUR: a failing tool body is matte dark red, no grey highlight (live errors-1123: an extrude
/// missing the parts, its sides (130,62,67); a revolve's ring (135,63,69)); its main colour within 10.
#[gpui::test]
fn a_failing_tool_body_is_lives_matte_dark_red(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let base = serde_json::json!([
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }
    ]);
    let cases = [
        ("extrude add-miss", 0x823e43, serde_json::json!([
            { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[70, 15], [78, 15]] }] },
            { "type": "extrude", "name": "Tool", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "ADD", "defaultScope": false, "booleanScope": [] } }
        ])),
        ("revolve remove-miss", 0x873f45, serde_json::json!([
            { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "corner_rectangle": [[60, 5], [70, 15]] }] },
            { "type": "newSketch", "name": "Sketch 4", "plane": "Top", "shapes": [{ "line": [[50, -5], [50, 25]] }] },
            { "type": "revolve", "name": "Tool", "params": { "entities": ["Sketch 2 region 0"], "axis": ["Sketch 4 edges"], "operationType": "REMOVE", "defaultScope": false, "booleanScope": [] } }
        ])),
    ];
    for (name, live, extra) in cases {
        let mut features = base.as_array().unwrap().clone();
        features.extend(extra.as_array().unwrap().iter().cloned());
        let m = matrix_studio_from(cx, "failed-tool-study", serde_json::json!({ "units": "mm", "features": features }));
        let id = cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Tool").map(|f| f.id)).expect("the tool feature");
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
        wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
        m.park(cx);
        let failed = cx.update(|cx| m.view.read(cx).tool_failed());
        let (_, _, px) = m.frame(cx);
        let mut count: std::collections::HashMap<[u8; 3], usize> = std::collections::HashMap::new();
        for p in px.iter().filter(|p| p[0] as i32 - p[1] as i32 > 12 && p[0] < 215 && p[0] as i32 - p[2] as i32 > 8) {
            *count.entry([p[0] / 3 * 3, p[1] / 3 * 3, p[2] / 3 * 3]).or_default() += 1;
        }
        let mut top: Vec<_> = count.into_iter().collect();
        top.sort_by_key(|t| std::cmp::Reverse(t.1));
        let main = top.first().map(|t| t.0).unwrap_or_default();
        assert!(failed && close(main, live, 10), "{name}: failed {failed}, its main colour {main:?}, live #{live:06x} ({:?})", &top[..top.len().min(6)]);
    }
}

/// FEEL-TRANSFORM-TRIAD-LOOK (qa manipnav r6): Translate by XYZ's hovered X arrow is one solid blue stroke over the part
/// (live), not broken every ~3 px by the part's edges.
#[gpui::test]
fn a_hovered_triad_arrow_is_solid_blue_over_the_part(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "triad-solid");
    let part = cx.update(|cx| m.view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone()))).expect("the block");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("transform", w, cx))).unwrap();
    draw(cx, m.window);
    cx.update(|cx| m.view.update(cx, |v, cx| {
        if let Some(e) = &v.edit {
            e.dialog.update(cx, |d, _| {
                d.feature.params.set("transformType", noshape_model::params::ParamValue::Enum("TRANSLATION_3D".into()));
                d.feature.params.set("entities", noshape_model::params::ParamValue::Query(vec![part.clone()]));
            });
        }
    }));
    draw(cx, m.window);
    // The default view (the shaft slanted, as qa's manipnav m10), then from the top-front edge's side at the block's
    // centre: the X shaft (from the centre) along that edge on screen, the edge in front of it (qa manipnav r6).
    for edge_view in [false, true] {
        if edge_view {
            cx.update(|cx| {
                let vp = m.view.read(cx).viewport.clone();
                vp.update(cx, |vp, cx| {
                    vp.camera.rotation = noshape_render::Camera::look_rotation(DVec3::new(0., 15., -10.).normalize(), DVec3::Z);
                    vp.camera.target = DVec3::new(0.02, 0.015, 0.01);
                    vp.request_frame(cx);
                });
            });
            draw(cx, m.window);
        }
        let (a, b) = cx.update(|cx| {
            let v = m.view.read(cx);
            let t = v.current_triad(cx).expect("the triad");
            let s = v.triad_hit_segments(&t, cx)[0];
            (s.0, s.1)
        });
        let at = |cx: &mut TestAppContext, p: DVec3| m.exact(cx, [p.x * 1000.0, p.y * 1000.0, p.z * 1000.0]);
        let (sa, sb) = (at(cx, a), at(cx, b));
        m.pointer.move_to(cx, &m.view, sa.lerp(sb, 0.4).round());
        assert_eq!(cx.update(|cx| m.view.read(cx).ui.triad_hover), Some(0), "the X arrow hovered (edge view {edge_view})");
        let (w, _, px) = m.frame(cx);
        // At every step along the stroke, across it a pixel mostly the light core #97B1D9 over the #3F6FAC outline (live's
        // shaft is antialiased across its width); qa r6-r10 saw the core in steps, the outline between them, 13 gaps
        // in 56 px.
        let (outline, core) = (DVec3::new(63., 111., 172.), DVec3::new(151., 177., 217.));
        let n = (sb - sa).length().floor() as usize;
        let across = (sb - sa).normalize().perp();
        let rgb = |p: DVec2| {
            let c = px[(p.y as u32 * w + p.x as u32) as usize];
            DVec3::new(c[0] as f64, c[1] as f64, c[2] as f64)
        };
        let share: Vec<(DVec2, f64)> = (n / 10..n / 2).map(|k| sa.lerp(sb, k as f64 / n as f64)).map(|q| {
            // Only pixels on the outline-core blend (not the light part or background beside the stroke).
            let best = (-2..=2).map(|k| {
                let c = rgb(q + across * (k as f64 * 0.5)) - outline;
                let t = c.dot(core - outline) / (core - outline).length_squared();
                if (c - (core - outline) * t).length() < 20.0 { t } else { 0.0 }
            }).fold(0.0, f64::max);
            (q, best)
        }).collect();
        let gaps = share.iter().filter(|s| s.1 < 0.6).count();
        assert!(gaps == 0, "edge view {edge_view}: {gaps} of {} centre-line pixels not the core: {share:?}", share.len());
        // At every step along the shaft, beside the core a pixel of the full outline (live's rims ~1.3 px; qa 8641c8b2a:
        // ours were 0.75 px, never a full outline pixel and lighter every ~7 columns).
        let rim: Vec<(f64, f64)> = share.iter().map(|&(q, _)| {
            let side = |sign: f64| (3..=9).map(|k| rgb(q + across * sign * (k as f64 * 0.25)).distance(outline)).fold(f64::MAX, f64::min);
            (side(1.0), side(-1.0))
        }).collect();
        let faint = rim.iter().filter(|r| r.0.min(r.1) > 24.0).count();
        assert!(faint == 0, "edge view {edge_view}: {faint} of {} steps without a full outline pixel: {rim:?}", rim.len());
    }
}

/// FEEL-TRANSFORM-TRIAD-LOOK (qa manipnav r6, live manip-feel-1122): while Translate by XYZ's X arrow is dragged, live
/// keeps the part's orange outline at its first place (the light grey ghost, #C2CDD5) and draws the moved part opaque,
/// its top #99C0D7 (live transform-x-drag: 7821 top px, 2418 ghost, 1384 orange); ours drew no outline (the outline mask
/// skipped see-through parts).
#[gpui::test]
fn a_dragged_triad_keeps_the_outline_and_an_opaque_part(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "triad-drag-look");
    let part = cx.update(|cx| m.view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone()))).expect("the block");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("transform", w, cx))).unwrap();
    draw(cx, m.window);
    cx.update(|cx| m.view.update(cx, |v, cx| {
        if let Some(e) = &v.edit {
            e.dialog.update(cx, |d, _| {
                d.feature.params.set("transformType", noshape_model::params::ParamValue::Enum("TRANSLATION_3D".into()));
                d.feature.params.set("entities", noshape_model::params::ParamValue::Query(vec![part.clone()]));
            });
        }
    }));
    draw(cx, m.window);
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    let (a, b) = cx.update(|cx| {
        let v = m.view.read(cx);
        let t = v.current_triad(cx).expect("the triad");
        let s = v.triad_hit_segments(&t, cx)[0];
        (s.0, s.1)
    });
    let at = |cx: &mut TestAppContext, p: DVec3| m.exact(cx, [p.x * 1000.0, p.y * 1000.0, p.z * 1000.0]);
    let (sa, sb) = (at(cx, a), at(cx, b));
    let grab = sa.lerp(sb, 0.4).round();
    m.pointer.move_to(cx, &m.view, grab);
    let count = |px: &[[u8; 3]]| {
        let near = |c: u32, t: i32| px.iter().filter(|p| close(**p, c, t)).count();
        let orange = px.iter().filter(|p| p[0] > 220 && (120..200).contains(&p[1]) && p[2] < 90).count();
        (near(0x99c0d7, 12), near(0xc2cdd5, 8), orange)
    };
    let rest = count(&m.frame(cx).2);
    let mut vcx = gpui_kit::VisualTestContext::from_window(m.window, cx);
    let dir = (sb - sa).normalize();
    let p0 = m.pointer.window_point(&mut vcx, grab);
    vcx.simulate_mouse_down(p0, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::default());
    for k in 1..=6 {
        let p = m.pointer.window_point(&mut vcx, grab + dir * (10.0 * k as f64));
        vcx.simulate_mouse_move(p, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::default());
        m.pointer.frames(cx, &m.view);
    }
    for _ in 0..20 {
        m.pointer.frames(cx, &m.view);
    }
    let dragged = count(&m.frame(cx).2);
    println!("triad drag (live top #99C0D7 px, ghost #C2CDD5 px, orange px): at rest {rest:?}, dragged {dragged:?}");
    assert!(dragged.0 > 3000 && dragged.1 > 300 && dragged.2 * 2 > rest.2, "dragged: the moved top opaque, the ghost, its outline kept (live (7821, 2418, 1384)): {dragged:?} (at rest {rest:?})");
    let mut vcx = gpui_kit::VisualTestContext::from_window(m.window, cx);
    let end = m.pointer.window_point(&mut vcx, grab + dir * 60.0);
    vcx.simulate_mouse_up(end, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::default());
}

/// The pixels across a lit edge's band: the column through `at` (crop px), 4 px each way.
fn across(crop: &[[u8; 3]], at: DVec2) -> Vec<[u8; 3]> {
    (at.y as usize - 4..=at.y as usize + 4).map(|y| crop[y * 80 + at.x as usize]).collect()
}

// Live ps-owner-matrix-1124: an edge hovered while its own face is selected draws its hover band with the face's
// selected line under its core, #FFC06A all through (no dark edge line: the plain hover's core reads #D8B080..#C8A070).
#[gpui::test]
fn an_edge_hovered_on_its_selected_face_has_no_dark_core(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let c = cell(cx, EDGE, State::HoveredWith(FACE));
    let col = across(&c, DVec2::new(40.0, 40.0));
    assert!(col.iter().any(|p| close(*p, 0xffc06a, 8)) && !col.iter().any(|p| close(*p, 0xffa90a, 12) || (p[0] < 0xd8 && p[2] < 0x80)), "across the band {col:02x?}");
}

// A face hovered with one of its edges selected: that edge's band flat #F8A820, the face's hover line under its core
// (at rest #D09018..#C88810 with the darker line through).
#[gpui::test]
fn a_selected_edge_on_its_hovered_face_is_a_flat_band(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let c = cell(cx, FACE, State::HoveredWith(EDGE));
    let m = matrix_studio(cx, "hover-matrix-owner");
    let col = across(&c, m.screen(cx, EDGE) - m.screen(cx, FACE) + DVec2::new(40.0, 40.0));
    assert!(col.iter().any(|p| close(*p, 0xf8a820, 8)) && !col.iter().any(|p| p[0] < 0xe8 && p[2] < 0x30), "across the band {col:02x?}");
}

/// qa ERR-SM-BAD-SELECTION-RED (live errors-1123 sheet-metal fl-joint2 vp): from the default view, where Flange 1's
/// wall hides the joint edge, live draws it red over the wall (its red line inside the wall's face, a few px above the
/// wall's bottom outline), as a picked edge is drawn through the part.
#[gpui::test]
fn a_failing_flanges_joint_edge_is_red_through_the_wall(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "sheetMetalStart", "name": "Sheet metal model 1", "params": { "process": "THICKEN", "regions": ["Sketch 1 region 0"], "thickness": "2 mm", "radius": "2 mm" } },
        { "type": "sheetMetalFlange", "name": "Flange 1", "params": { "edges": ["edge through [20, 0, 2]"] } },
        { "type": "sheetMetalFlange", "name": "Flange 2", "params": { "edges": ["edge through [60, 25, 2]"] } },
        { "type": "sheetMetalFlange", "name": "Flange 3", "params": { "edges": ["edge through [29, 2, 2]"] } },
    ] });
    let m = matrix_studio_from(cx, "sm-flange-joint-red-wall", start);
    m.park(cx);
    let red = |cx: &mut TestAppContext| m.frame(cx).2.iter().filter(|c| close(**c, 0xbd3039, 40)).count();
    let before = red(cx);
    let flange = cx.update(|cx| m.view.read(cx).studio(cx).and_then(|st| st.features.iter().find(|f| f.name == "Flange 3").map(|f| f.id))).expect("Flange 3");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(flange, w, cx))).unwrap();
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy() && m.view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).status.is_some()));
    m.park(cx);
    let after = red(cx);
    assert!(after >= before + 40, "red px: {before} closed, {after} with the dialog open");
}

/// Qa SKETCH-MIRROR-LINE-LOOK (live sketch-tools-1123 mirror-pattern.md): Mirror's picked line is pale yellow-green
/// (242,255,128) with (215,226,150) edges, 2 px, under the parked pointer; qa measured ours darker (189,199,100).
#[gpui::test]
fn the_picked_mirror_line_is_live_pale_yellow_green(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "hover-matrix-mirror-line");
    m.edit_sketch(cx);
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.activate_tool(Tool::Mirror, w, cx))).unwrap();
    let p = m.screen(cx, SK_LINE);
    m.pointer.move_to(cx, &m.view, p);
    m.pointer.click(cx, &m.view, p);
    m.park(cx);
    assert!(cx.update(|cx| m.view.read(cx).sketch.as_ref().and_then(|ed| ed.shown_mirror_line()).is_some()), "the line picked as the mirror line");
    let c = m.crop(cx, p);
    let column: Vec<[u8; 3]> = (30..51).map(|y| c[y * 80 + 40]).collect();
    let lit = column.iter().filter(|px| px[1] > 180 && px[1] >= px[0] && (px[1] as i32 - px[2] as i32) > 60).count();
    assert!(column.iter().any(|px| close(*px, 0xf2ff80, 16)) && (1..=3).contains(&lit), "rows 30..50 across the line: {column:?}");
}

/// Live mc-reach-1124 (FEEL-MC-PS-MARKER-REACH (2)): the pointer on the block's top-front edge, from its midpoint toward
/// the corner: the Mate connector stays at the midpoint to 26 px of the 56 px half, then goes to the corner end (Z along
/// the edge) from 27 px.
#[gpui::test]
fn a_mate_connector_along_an_edge_goes_to_its_nearer_end_past_half_way(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } }
    ] });
    let m = matrix_studio_from(cx, "mc-edge-half", start);
    wait_until(cx, m.window, |cx| !m.view.read(cx).display.bodies.is_empty() && !m.view.read(cx).ui_busy());
    m.park(cx);
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("mateConnector", w, cx))).unwrap();
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    let (mid, corner) = (m.exact(cx, [20., 0., 20.]), m.exact(cx, [40., 0., 20.]));
    let mut got = Vec::new();
    for f in [0.2, 0.4, 0.6, 0.8] {
        m.pointer.move_to(cx, &m.view, (mid + (corner - mid) * f).round());
        wait_until(cx, m.window, |cx| m.view.read(cx).inference_choice().is_some());
        let at = cx.update(|cx| m.view.read(cx).inference_choice().map(|p| (p.frame.origin * 1e3).round()));
        got.push((f, at));
    }
    let want = |f: f64| if f < 0.5 { DVec3::new(20., 0., 20.) } else { DVec3::new(40., 0., 20.) };
    assert!(got.iter().all(|(f, at)| *at == Some(want(*f))), "fraction of the half, glyph at (mm): {got:?}; {:.1} px half", mid.distance(corner));
}

/// Along the line through the crop's centre in direction `d`, 34 px either way: the steps with `rgb` within 2 px across
/// it, and the bare steps (no orange and no blue within 2 px: a dash's gap).
fn dashes(crop: &[[u8; 3]], d: DVec2, rgb: u32) -> (usize, usize) {
    let d = d.normalize();
    let n = DVec2::new(-d.y, d.x);
    let across = |k: i32| {
        (-2..=2).filter_map(move |j| {
            let q = DVec2::new(40.0, 40.0) + d * k as f64 + n * j as f64;
            let (x, y) = (q.x.round() as i32, q.y.round() as i32);
            ((0..80).contains(&x) && (0..80).contains(&y)).then(|| crop[(y * 80 + x) as usize])
        })
    };
    let lit = (-34..=34).filter(|&k| across(k).any(|c| close(c, rgb, 24))).count();
    let bare = (-34..=34).filter(|&k| across(k).all(|c| !(c[0] >= 0xd0 && c[2] <= 0xb0) && !sketch_blue(c))).count();
    (lit, bare)
}

// A construction line in sketch edit (live sk-hover-matrix-1123 CL-N / CL-SS, raw/mx): hovered, its dashes take the
// #ffc685 band and its gaps stay bare; selected, the dashes are #ffa90a and stay so under the pointer (no hover tint).
#[gpui::test]
fn sk_a_construction_lines_dashes_take_the_hover_and_selected_bands(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "construction-hover");
    m.edit_sketch(cx);
    let p = m.screen(cx, SK_LINE);
    m.pointer.move_to(cx, &m.view, p);
    m.pointer.click(cx, &m.view, p);
    cx.update_window(m.window, |_, w, cx| {
        m.view.update(cx, |v, cx| {
            v.toggle_construction(w, cx);
            if let Some(ed) = &mut v.sketch {
                ed.selection.clear();
            }
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    m.park(cx);
    let d = screen_dir(cx, &m, SK_LINE, [1., 0., 0.]);
    approach(cx, &m, p);
    let (lit, bare) = dashes(&m.crop(cx, p), d, 0xffc685);
    assert!(lit >= 20 && bare >= 3, "hovered: {lit} lit, {bare} bare steps of 69");
    m.pointer.click(cx, &m.view, p);
    m.park(cx);
    let selected = m.crop(cx, p);
    let (lit, bare) = dashes(&selected, d, 0xffa90a);
    assert!(lit >= 20 && bare >= 3, "selected: {lit} lit, {bare} bare steps of 69");
    approach(cx, &m, p);
    let both = m.crop(cx, p);
    // Within 3 px of the line, 34 px either way: no pixel changes under the pointer.
    let n = DVec2::new(-d.y, d.x).normalize();
    let changed: Vec<(i32, [u8; 3], [u8; 3])> = (-34..=34)
        .flat_map(|k| (-3..=3).map(move |j| (k, j)))
        .filter_map(|(k, j)| {
            let q = DVec2::new(40.0, 40.0) + d.normalize() * k as f64 + n * j as f64;
            let i = (q.y.round() as usize) * 80 + q.x.round() as usize;
            selected[i].iter().zip(both[i]).any(|(a, b)| a.abs_diff(b) > 8).then_some((k, selected[i], both[i]))
        })
        .collect();
    assert!(changed.is_empty(), "selected, then hovered too: {} px changed, {:02x?}", changed.len(), &changed[..changed.len().min(4)]);
}

// A feature-list row hovered while a sketch is open (live sk-hover-matrix-1123, "Sketch 2" / "Block" rows): nothing is
// drawn in the graphics area (outside the sketch, Extrude 1's row outlines its part).
#[gpui::test]
fn sk_a_feature_rows_hover_draws_nothing_while_a_sketch_is_open(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "row-hover-in-sketch");
    m.edit_sketch(cx);
    m.park(cx);
    let ids: Vec<(String, noshape_model::reference::FeatureId)> = cx.update(|cx| m.view.read(cx).studio(cx).map(|st| st.features.iter().filter(|f| f.name != "Sketch 2").map(|f| (f.name.clone(), f.id)).collect()).unwrap_or_default());
    assert!(!ids.is_empty(), "Sketch 1 and Extrude 1");
    m.pointer.frames(cx, &m.view);
    let still = m.frame(cx);
    for (name, f) in ids {
        m.view.update(cx, |v, cx| {
            v.ui.list_hover = Some(f);
            v.refresh_scene(false, cx);
        });
        m.pointer.frames(cx, &m.view);
        let now = m.frame(cx);
        let changed = still.2.iter().zip(&now.2).filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 8)).count();
        assert_eq!(changed, 0, "{name}'s row hovered changed {changed} px");
        m.view.update(cx, |v, cx| {
            v.ui.list_hover = None;
            v.refresh_scene(false, cx);
        });
        m.pointer.frames(cx, &m.view);
    }
}

// Select other (`) on a sketch line between two regions (live sk-leftovers-1124): Edge, then a Face row for the region on
// each side of it.
#[gpui::test]
fn sk_select_other_on_a_line_between_two_regions_lists_both(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [
            { "corner_rectangle": [[60, 0], [100, 20]] },
            { "line": [[80, 0], [80, 20]] }
        ] }
    ] });
    let m = matrix_studio_from(cx, "select-other-two-regions", start);
    m.edit_sketch(cx);
    let p = m.screen(cx, [80., 10., 0.]);
    approach(cx, &m, p);
    cx.update_window(m.window, |_, _, cx| m.view.update(cx, |v, cx| v.select_other(false, cx))).unwrap();
    let rows: Vec<String> = cx.update(|cx| {
        let v = m.view.read(cx);
        let label = v.labeler(cx);
        v.ui.select_other.as_ref().map(|s| s.hits.iter().map(|r| label(r)).collect()).unwrap_or_default()
    });
    assert_eq!(rows.first().map(String::as_str), Some("Edge of Sketch 2"), "{rows:?}");
    assert_eq!(rows.iter().filter(|r| *r == "Face of Sketch 2").count(), 2, "a region each side: {rows:?}");
}


/// The darkest pixel within `r` px of the crop's centre (a 1 px line's core).
fn darkest(crop: &[[u8; 3]], r: f64) -> [u8; 3] {
    near_centre(crop, r).min_by_key(|c| c.iter().map(|&v| v as u32).sum::<u32>()).unwrap_or([255; 3])
}

// A dimension's line in sketch edit (live sk-hover-matrix-1123 DL-N / DL-SS, raw/mx): black, hovered #9c7611 at its core;
// selected #6f5610, the same under the pointer (no hover tint).
#[gpui::test]
fn sk_a_dimension_line_is_lives_gold_hovered_and_selected(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "dimension-line-hover");
    m.edit_sketch(cx);
    let p = m.screen(cx, SK_LINE);
    m.pointer.move_to(cx, &m.view, p);
    let line = cx.update(|cx| m.view.read(cx).sketch.as_ref().and_then(|ed| ed.hover)).expect("the line under the pointer");
    m.park(cx);
    cx.update_window(m.window, |_, w, cx| {
        m.view.update(cx, |v, cx| {
            v.with_sketch(cx, |_, data| {
                crate::sketch_mode::add_dimension(data, noshape_sketch::ConstraintKind::Length { curve: line, value: 0.03 }, Some(glam::DVec2::new(0.075, -0.012)), None)
            });
            v.regen_sync(w, cx);
        })
    })
    .unwrap();
    m.pointer.frames(cx, &m.view);
    // A quarter along the dimension line (its text sits in the middle; the extension lines run along y), in mm on the
    // Top plane.
    let segs = cx.update(|cx| {
        let v = m.view.read(cx);
        let (Some(ed), Some(data)) = (v.sketch.as_ref(), v.sketch_data(cx)) else { return Vec::new() };
        ed.dimension_graphics(&data).into_iter().flat_map(|g| g.segments).collect::<Vec<_>>()
    });
    let seg = segs.iter().filter(|s| (s[1] - s[0]).x.abs() > 4.0 * (s[1] - s[0]).y.abs()).max_by(|a, b| (a[1] - a[0]).length().total_cmp(&(b[1] - b[0]).length())).copied().unwrap_or_else(|| panic!("the dimension line: {segs:?}"));
    let mid = (seg[0] + (seg[1] - seg[0]) * 0.25) * 1000.0;
    let at = m.screen(cx, [mid.x, mid.y, 0.]);
    let before = darkest(&m.crop(cx, at), 3.0);
    approach(cx, &m, at);
    let hovered = darkest(&m.crop(cx, at), 3.0);
    assert!(cx.update(|cx| m.view.read(cx).sketch.as_ref().is_some_and(|ed| ed.hover_constraint.is_some())), "the dimension hovered");
    m.pointer.click(cx, &m.view, at);
    m.park(cx);
    let selected = darkest(&m.crop(cx, at), 3.0);
    approach(cx, &m, at);
    let both = darkest(&m.crop(cx, at), 3.0);
    // An extension line lights it too (live DE-N #aa8b38).
    cx.update_window(m.window, |_, _, cx| {
        m.view.update(cx, |v, cx| {
            if let Some(ed) = &mut v.sketch {
                ed.selected_constraints.clear();
            }
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    m.park(cx);
    let ext = segs.iter().filter(|s| (s[1] - s[0]).y.abs() > 4.0 * (s[1] - s[0]).x.abs()).max_by(|a, b| (a[1] - a[0]).length().total_cmp(&(b[1] - b[0]).length())).copied().expect("an extension line");
    let e = (ext[0] + (ext[1] - ext[0]) * 0.6) * 1000.0;
    let at_ext = m.screen(cx, [e.x, e.y, 0.]);
    approach(cx, &m, at_ext);
    let ext_lit = darkest(&m.crop(cx, at_ext), 3.0);
    let ext_hovered = cx.update(|cx| m.view.read(cx).sketch.as_ref().and_then(|ed| ed.hover_constraint));
    let got = [before, hovered, selected, both];
    let want = [0x000000, 0x9c7611, 0x6f5610, 0x6f5610];
    assert!(got.iter().zip(want).all(|(c, w)| close(*c, w, 8)), "line core before, hovered, selected, both: {got:02x?} at {mid:?} mm, {segs:?}");
    assert!(ext_hovered.is_some() && ext_lit[0] > 0x80 && ext_lit[0] > ext_lit[1] && ext_lit[1] > ext_lit[2], "the extension line hovered: {ext_hovered:?} {ext_lit:02x?}");
}

/// Sketch 2's targets of live sk-hover-matrix-1124: its point, line end, line, arc, circle, circle region, circle centre
/// and arc centre.
const SK_1124: [(&str, [f64; 3]); 8] = [
    ("P", [60., 25., 0.]),
    ("LE", [90., 5., 0.]),
    ("LN", [68., 5., 0.]),
    ("AR", [75.5659, 22.98, 0.]),
    ("CI", [100., 14., 0.]),
    ("RG", [103., 22., 0.]),
    ("CC", [100., 20., 0.]),
    ("AC", [75., 15., 0.]),
];

fn sk_1124(name: &str) -> [f64; 3] {
    SK_1124.iter().find(|t| t.0 == name).map(|t| t.1).expect("a target")
}

/// Pixels of two crops more than 8 apart in a channel.
fn changed(a: &[[u8; 3]], b: &[[u8; 3]]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| (*p as i32 - *q as i32).abs() > 8)).count()
}

/// The crop round `target`, the pointer parked, then come onto it (after `select` was clicked, when given).
fn rest_and_lit(cx: &mut TestAppContext, m: &Matrix, select: Option<[f64; 3]>, target: [f64; 3]) -> (Vec<[u8; 3]>, Vec<[u8; 3]>) {
    m.reset(cx);
    if let Some(s) = select {
        let q = m.screen(cx, s);
        approach(cx, m, q);
        m.pointer.click(cx, &m.view, q);
        m.park(cx);
    }
    let p = m.screen(cx, target);
    let rest = m.crop(cx, p);
    approach(cx, m, p);
    (rest, m.crop(cx, p))
}

// Live sk-hover-matrix-1124: what is selected doesn't change when the pointer comes onto it (P, LE, CI, RG, CC: 0 px
// changed; LN, AC, AR: nothing on the target), in sketch edit and on the committed sketch in the Part Studio.
#[gpui::test]
fn sk_hovering_whats_selected_changes_nothing(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "selected-then-hovered");
    let mut bad = Vec::new();
    for edit in [false, true] {
        if edit {
            m.edit_sketch(cx);
        }
        for (name, t) in SK_1124 {
            // (In the Part Studio a sketch's line, arc and arc centre aren't in live's cells.)
            if !edit && matches!(name, "LN" | "AR" | "AC" | "LE") {
                continue;
            }
            let (sel, lit) = rest_and_lit(cx, &m, Some(t), t);
            let n = changed(&sel, &lit);
            if n > 4 {
                bad.push(format!("{}{name}: {n} px", if edit { "sketch edit " } else { "Part Studio " }));
            }
        }
    }
    assert!(bad.is_empty(), "selected then hovered changed: {bad:?}");
}

// Live sk-hover-matrix-1124: a centre point lights as it does with nothing selected when its circle or arc is (CC-CS 108
// = CC-N, AC-AS 286 = AC-N); a region lights as unselected with a line selected (RG-LS 422 = RG-N).
#[gpui::test]
fn sk_centres_and_regions_light_alike_whatever_else_is_selected(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "centre-with-curve-selected");
    m.edit_sketch(cx);
    let mut bad = Vec::new();
    for (sel, target, r) in [("CI", "CC", 8.0), ("AR", "AC", 8.0), ("LN", "RG", 40.0)] {
        let (_, alone) = rest_and_lit(cx, &m, None, sk_1124(target));
        let (_, with) = rest_and_lit(cx, &m, Some(sk_1124(sel)), sk_1124(target));
        let near = |c: &[[u8; 3]]| near_centre(c, r).collect::<Vec<_>>();
        let n = changed(&near(&alone), &near(&with));
        if n > 4 {
            bad.push(format!("{target} with {sel} selected: {n} px differ"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

// Live line-linger-1124: with the Line tool a hovered line end or circle centre stays lit (acquired) while the pointer
// leaves along its horizontal or vertical, and goes when it leaves another way; a free point never stays.
#[gpui::test]
fn sk_the_line_tool_keeps_an_acquired_point_lit_along_its_levels(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "acquired-point");
    m.edit_sketch(cx);
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    let disc = |a: &[[u8; 3]], b: &[[u8; 3]]| near_centre(a, 5.0).zip(near_centre(b, 5.0)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| (*p as i32 - *q as i32).abs() > 8)).count();
    let mut got = Vec::new();
    for (name, away, kept) in [("CC", [0., -15.], true), ("CC", [15., 0.], true), ("CC", [6., 6.], false), ("LE", [0., -8.], true), ("LE", [6., -6.], false), ("P", [0., -8.], false)] {
        let empty = m.screen(cx, [60., -20., 0.]);
        m.pointer.move_to(cx, &m.view, empty);
        m.park(cx);
        assert!(cx.update(|cx| m.view.read(cx).sketch.as_ref().and_then(|ed| ed.acquired)).is_none(), "parked, {got:#?}");
        let t = sk_1124(name);
        let p = m.screen(cx, t);
        let rest = m.crop(cx, p);
        approach(cx, &m, p);
        let on = cx.update(|cx| m.view.read(cx).sketch.as_ref().and_then(|ed| ed.acquired)).is_some();
        for k in 1..=8 {
            let f = k as f64 / 8.0;
            let q = m.screen(cx, [t[0] + away[0] * f, t[1] + away[1] * f, 0.]);
            m.pointer.move_to(cx, &m.view, q);
        }
        m.pointer.frames(cx, &m.view);
        let held = cx.update(|cx| m.view.read(cx).sketch.as_ref().and_then(|ed| ed.acquired)).is_some();
        let hovers = cx.update(|cx| {
            let v = m.view.read(cx);
            format!("{:?} / {:?}", v.hover, v.sketch.as_ref().map(|ed| (ed.hover, ed.hover_region.clone())))
        });
        let lit = disc(&rest, &m.crop(cx, p));
        let ok = held == kept && (lit >= 20) == kept && on == (name != "P");
        got.push(format!("{name} {away:?}: acquired {on}, held {held}, {lit} px lit, hover {hovers}{}", if ok { "" } else { " ✗" }));
    }
    assert!(got.iter().all(|g| !g.ends_with('✗')), "{got:#?}");
}

// A vertex hovered with its edge selected: the 11 px hover disc, the selected band orange (#F8A818) over a quarter of
// it (28 px), from the centre towards the edge; ours drew the disc over every line.
#[gpui::test]
fn a_hovered_vertex_on_its_selected_edge_keeps_the_band_over_its_disc(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let in_disc = |c: &[[u8; 3]]| (34..=46usize).flat_map(|y| (34..=46usize).map(move |x| (x, y))).filter(|&(x, y)| (x as f64 - 40.0).hypot(y as f64 - 40.0) <= 5.5 && close(c[y * 80 + x], 0xf8a818, 14)).count();
    let (with, plain) = (in_disc(&cell(cx, VERTEX, State::HoveredWith(EDGE))), in_disc(&cell(cx, VERTEX, State::Hovered)));
    println!("band orange in the disc: edge selected {with}, plain hover {plain} (live 28, 0)");
    assert!((15..=45).contains(&with) && plain <= 3, "band orange in the hover disc: with its edge selected {with} (live 28), plain {plain}");
}

/// Live errors-1123: an Extrude of depth 0 draws its circle region pale red with a whole red (189, 48, 57) outline,
/// edited or created; no grey corner dot along the circle (a tessellated circle has no corners).
#[gpui::test]
fn a_depth_0_extrudes_circle_region_has_a_red_outline(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    for creating in [false, true] {
        let mut feats = vec![
            serde_json::json!({ "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] }),
            serde_json::json!({ "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }),
            serde_json::json!({ "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[70, 15], [78, 15]] }] }),
        ];
        if !creating {
            feats.push(serde_json::json!({ "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "0 mm", "operationType": "NEW" } }));
        }
        let m = matrix_studio_from(cx, if creating { "depth0-new" } else { "depth0-edit" }, serde_json::json!({ "units": "mm", "features": feats }));
        if creating {
            cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
            let c = m.screen(cx, [70., 15., 0.]);
            m.pointer.move_to(cx, &m.view, c);
            m.pointer.click(cx, &m.view, c);
            wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
            cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| {
                if let Some(e) = &v.edit {
                    e.dialog.update(cx, |d, cx| d.set_expr_text("depth", "0 mm", w, cx));
                }
            })).unwrap();
        } else {
            let id = cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 2").map(|f| f.id)).expect("Extrude 2");
            cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
        }
        wait_until(cx, m.window, |cx| m.view.read(cx).edit.is_some() && !m.view.read(cx).ui_busy());
        m.park(cx);
        let (w, _, px) = m.frame(cx);
        let (a, b) = (m.screen(cx, [76., 15., 0.]), m.screen(cx, [80., 15., 0.]));
        let row: Vec<[u8; 3]> = (a.x as u32..=b.x as u32).map(|x| px[(a.y as u32 * w + x) as usize]).collect();
        let pale = px.iter().filter(|c| close(**c, 0xd3b4ba, 16)).count();
        assert!(pale > 100 && row.iter().any(|p| close(*p, 0xbd3039, 16)) && !row.iter().any(|p| p[0] == p[1] && p[1] == p[2] && p[0] < 0xc0), "creating {creating}: pale {pale}, across the outline {row:02x?}");
    }
}

// A construction circle is dashed like a construction line (live construction-dash-1124: 4 / 6 / 30 / 6 screen px, laid
// out from the curve's start), not solid where each of its short segments started the pattern again.
#[gpui::test]
fn sk_a_construction_circle_is_dashed(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "construction-circle");
    m.edit_sketch(cx);
    let rim = m.screen(cx, [106., 20., 0.]);
    m.pointer.move_to(cx, &m.view, rim);
    m.pointer.click(cx, &m.view, rim);
    cx.update_window(m.window, |_, w, cx| {
        m.view.update(cx, |v, cx| {
            v.toggle_construction(w, cx);
            if let Some(ed) = &mut v.sketch {
                ed.selection.clear();
            }
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    // Off the circle through an empty spot of the sketch, so nothing stays hovered.
    let empty = m.screen(cx, [60., -20., 0.]);
    m.pointer.move_to(cx, &m.view, empty);
    m.park(cx);
    m.pointer.frames(cx, &m.view);
    let centre = m.screen(cx, SK_CENTRE);
    let c = m.crop(cx, centre);
    let steps = 120;
    let bare: Vec<bool> = (0..steps)
        .map(|i| {
            let a = i as f64 / steps as f64 * std::f64::consts::TAU;
            let q = m.screen(cx, [100. + 6. * a.cos(), 20. + 6. * a.sin(), 0.]) - centre + DVec2::new(40.0, 40.0);
            (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).all(|(dx, dy)| {
                let (x, y) = (q.x.round() as i32 + dx, q.y.round() as i32 + dy);
                !(0..80).contains(&x) || !(0..80).contains(&y) || !sketch_blue(c[(y * 80 + x) as usize])
            })
        })
        .collect();
    // The gaps: runs of bare samples round the rim (2 per 46 px period, ~5.5 on its ~126 px).
    let gaps = (0..steps).filter(|i| bare[*i] && !bare[(i + steps - 1) % steps]).count();
    assert!((4..=7).contains(&gaps), "{gaps} gaps round the rim, {} of {steps} samples bare, r {:.0} px", bare.iter().filter(|b| **b).count(), (rim - centre).length());
}

/// Display modes study (live display-modes-1125 (1)): live's block (40×30×20, a Ø10 hole through, 3 mm fillets on the
/// vertical edges) seen Isometric at 0.290 mm/px; prints each state's pixels at live's sample points
/// (cargo test -p noshape-part-studio-ui --lib print_display_mode_pixels -- --ignored --nocapture).
#[gpui::test]
#[ignore = "T2: prints the display modes' pixels"]
fn print_display_mode_pixels(cx: &mut TestAppContext) {
    use crate::scene_builder::{DisplayMode as D, TangentEdges as T};
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Block", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[20, 15], [25, 15]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "40 mm", "symmetric": true, "operationType": "REMOVE" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [0, 0, 10]", "edge through [40, 0, 10]", "edge through [40, 30, 10]", "edge through [0, 30, 10]"], "radius": "3 mm" } },
    ] });
    let m = matrix_studio_from(cx, "display-mode-pixels", start);
    let failed = cx.update(|cx| m.view.read(cx).display.features.iter().filter_map(|f| f.message.clone()).collect::<Vec<_>>());
    assert!(failed.is_empty(), "the block regenerates: {failed:?}");
    cx.update(|cx| {
        let vp = m.view.read(cx).viewport.clone();
        vp.update(cx, |vp, cx| {
            let forward = DVec3::new(-1., 1., -1.).normalize();
            let up = (DVec3::Z - forward * DVec3::Z.dot(forward)).normalize();
            vp.camera.rotation = noshape_render::Camera::look_rotation(forward, up);
            vp.camera.target = DVec3::new(0.02, 0.015, 0.01);
            vp.camera.distance = 0.290e-3 * vp.size().y / (2.0 * (vp.camera.fov_y * 0.5).tan());
            vp.request_frame(cx);
        });
    });
    let states = [
        ("s1 shaded+edges", D::ShadedWithEdges, false, T::Visible),
        ("s2 shaded", D::Shaded, false, T::Visible),
        ("s3 unshaded", D::HiddenLine, false, T::Visible),
        ("s4 translucent", D::Translucent, false, T::Visible),
        ("s5 s1+hidden", D::ShadedWithEdges, true, T::Visible),
        ("s6 s3+hidden", D::HiddenLine, true, T::Visible),
        ("s7 s1+phantom", D::ShadedWithEdges, false, T::Phantom),
        ("s8 s1+removed", D::ShadedWithEdges, false, T::Removed),
    ];
    let hex = |p: [u8; 3]| format!("{:02x}{:02x}{:02x}", p[0], p[1], p[2]);
    for (name, mode, hidden, tangent) in states {
        cx.update_window(m.window, |_, w, cx| {
            m.view.update(cx, |v, cx| {
                v.opts.hidden_edges_visible = hidden;
                v.opts.tangent_edges = tangent;
                v.set_display_mode(mode, w, cx);
            })
        })
        .unwrap();
        m.park(cx);
        let (w, h, px) = m.frame(cx);
        let at = |x: i64, y: i64| px[(y.clamp(0, h as i64 - 1) * w as i64 + x.clamp(0, w as i64 - 1)) as usize];
        let mut row = Vec::new();
        for (n, p) in [("a", [31., 23., 20.]), ("b", [20., 0., 10.]), ("c", [40., 15., 10.]), ("f", [37., 0., 10.]), ("e1", [20., 10., 0.]), ("e2", [0., 27., 10.]), ("rim", [20., 10., 20.])] {
            let q = m.screen(cx, p);
            let (x, y) = (q.x as i64, q.y as i64);
            let dark = (-3..=3).flat_map(|i| (-3..=3).map(move |j| (i, j))).map(|(i, j)| at(x + i, y + j)).min_by_key(|c| c.iter().map(|&v| v as u32).sum::<u32>()).unwrap_or_default();
            row.push(format!("{n}=#{}/{}", hex(at(x, y)), hex(dark)));
        }
        row.push(format!("g=#{}", hex(at(40, h as i64 / 2))));
        let q = m.screen(cx, [20., 0., 0.]);
        let col: Vec<String> = (-6..=6).map(|j| hex(at(q.x as i64, q.y as i64 + j))).collect();
        let q = m.screen(cx, [37., 0., 10.]);
        let tan: Vec<String> = (-4..=4).map(|i| hex(at(q.x as i64 + i, q.y as i64))).collect();
        println!("== {name}: {} | sil {} | tangent row {}", row.join(" "), col.join(","), tan.join(","));
        // Live edge-ink-1125's edges: 9 px across each, along the screen normal (nearest px), and its column.
        for (n, p, d) in [("a", [20., 0., 20.], [1., 0., 0.]), ("b", [20., 0., 0.], [1., 0., 0.]), ("p30", [40., 15., 20.], [0., 1., 0.]), ("c1", [0.88, 0.88, 10.], [0., 0., 1.]), ("c2", [39.12, 29.12, 10.], [0., 0., 1.]), ("d1", [37., 0., 10.], [0., 0., 1.]), ("d2", [40., 3., 10.], [0., 0., 1.])] {
            let q = m.screen(cx, p);
            let e = m.screen(cx, [p[0] + d[0], p[1] + d[1], p[2] + d[2]]) - q;
            let e = e / e.length();
            let nrm = glam::DVec2::new(-e.y, e.x);
            let perp: Vec<String> = (-4..=4).map(|i| { let s = q + nrm * i as f64; hex(at(s.x.round() as i64, s.y.round() as i64)) }).collect();
            let colm: Vec<String> = (-4..=4).map(|j| hex(at(q.x.round() as i64, q.y.round() as i64 + j))).collect();
            println!("ink {name} {n} ({:.1},{:.1}) {:.0}deg perp {} col {}", q.x, q.y, e.y.atan2(e.x).to_degrees(), perp.join(","), colm.join(","));
        }
    }
}

/// Live manip-ring-1125 (isometric at 0.3234 mm/px, three setups and two zooms): Move face's Rotate head is a stem
/// 24.5 px up from the radius line and a hollow head to 40 px, 17 px across, white in grey as the arrows; the radius
/// line runs 15.5 px past the stem; hovered, the line lights with the head.
#[gpui::test]
fn move_faces_ring_head_is_drawn_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "20 mm", "operationType": "NEW" } },
        { "type": "moveFace", "name": "Move face 1", "params": { "moveFaces": ["face through [15, 15, 20]"], "moveFaceType": "ROTATE", "axis": ["edge through [20, 0, 20]"], "angle": "0 deg" } }
    ] });
    let m = matrix_studio_from(cx, "hm-moveface-ring", start);
    cx.update(|cx| {
        let vp = m.view.read(cx).viewport.clone();
        vp.update(cx, |vp, cx| {
            let forward = DVec3::new(-1.0, 1.0, -1.0).normalize();
            vp.camera.rotation = noshape_render::Camera::look_rotation(forward, (DVec3::Z - forward * DVec3::Z.dot(forward)).normalize());
            vp.camera.distance = 0.3234e-3 * vp.size().y / (2.0 * (vp.camera.fov_y * 0.5).tan());
            vp.request_frame(cx);
        });
    });
    let id = cx.update(|cx| m.view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Move face 1").unwrap().id);
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    m.park(cx);
    let (axis, foot) = {
        let foot = cx.update(|cx| m.view.read(cx).viewport.read(cx).manipulators[0].0) * 1e3;
        (m.exact(cx, [foot.x, 0.0, 20.0]), m.exact(cx, foot.to_array()))
    };
    let (w, h, px) = m.frame(cx);
    // The glyph's white core (the background, faces and edges around it are not white).
    let white = |p: DVec2| {
        (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (p.x.round() as i64 + dx, p.y.round() as i64 + dy))).any(|(x, y)| {
            (0..w as i64).contains(&x) && (0..h as i64).contains(&y) && px[(y * w as i64 + x) as usize].iter().all(|&c| c >= 0xf0)
        })
    };
    let reach = |from: DVec2, dir: DVec2| (1..80).take_while(|&k| white(from + dir * k as f64)).last().unwrap_or(0) as f64;
    let radial = (foot - axis).normalize();
    let past = reach(foot, radial);
    // (The head is hollow: its tip is the last white up the stem's line.)
    let tip = (1..60).rfind(|&k| white(foot - DVec2::Y * k as f64)).unwrap_or(0) as f64;
    let base = foot.y - 24.5;
    let across = reach(DVec2::new(foot.x, base), DVec2::X) + reach(DVec2::new(foot.x, base), -DVec2::X);
    assert!((past - 15.5).abs() <= 2.0, "the radius line {past} px past the stem (live 15.5)");
    assert!((tip - 40.0).abs() <= 2.0, "the head's tip {tip} px up from the line (live 40)");
    assert!((across - 17.0).abs() <= 2.5, "the head's base {across} px across (live 17)");
    // Hovered, the head and the radius line light blue together.
    let lit = |(w, _, px): (u32, u32, Vec<[u8; 3]>)| {
        [foot - DVec2::Y * 10.0, axis.lerp(foot, 0.5)].map(|p| {
            let c = px[(p.y.round() as i64 * w as i64 + p.x.round() as i64) as usize];
            i32::from(c[2]) - i32::from(c[0]) > 40
        })
    };
    assert_eq!(lit((w, h, px)), [false, false], "idle, the stem and the line white in grey");
    m.pointer.move_to(cx, &m.view, foot - DVec2::Y * 20.0);
    assert_eq!(lit(m.frame(cx)), [true, true], "hovered, the stem and the line light");
}

/// Live hover-combined-1125 (P0, a selection field's or a sketch's picks hovered): what a dialog's field holds or a
/// sketch has selected changes nothing when hovered (Fillet's picked face, Plane's picked vertex, a model vertex
/// selected in a sketch: 0 px); a model edge selected in a sketch changes a little (52 px against 415 unselected); a
/// part selected before Fillet opens is let go, its field empty.
#[gpui::test]
fn combined_hover_picked_or_selected_changes_nothing(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "combined-1125");
    let mut bad = Vec::new();
    let open = |cx: &mut TestAppContext, ty: &str| {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature(ty, w, cx))).unwrap();
        wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    };
    let close_dialog = |cx: &mut TestAppContext| {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
        wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    };
    for (ty, name, target) in [("fillet", "1-fillet-TF-picked", FACE), ("cPlane", "2-plane-V-picked", VERTEX)] {
        open(cx, ty);
        let p = m.screen(cx, target);
        approach(cx, &m, p);
        m.pointer.click(cx, &m.view, p);
        wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
        m.park(cx);
        let rest = m.crop(cx, p);
        approach(cx, &m, p);
        let n = changed(&rest, &m.crop(cx, p));
        if n > 4 {
            bad.push(format!("{name}: {n} px"));
        }
        close_dialog(cx);
    }
    // 4-partsel-fillet-TF: Part 1 selected, then Fillet.
    let part = cx.update(|cx| m.view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone()))).expect("the block");
    m.view.update(cx, |v, _| v.selection = vec![part]);
    open(cx, "fillet");
    let (sel, field) = cx.update(|cx| {
        let v = m.view.read(cx);
        (v.selection.len(), v.edit.as_ref().map_or(0, |e| e.dialog.read(cx).feature.params.query("entities").len()))
    });
    if (sel, field) != (0, 0) {
        bad.push(format!("4-partsel-fillet: selection {sel}, field {field}"));
    }
    close_dialog(cx);
    // 5-sk: in a sketch, a model vertex / edge selected then hovered.
    m.edit_sketch(cx);
    for (name, target, most) in [("5-sk-V2-sel", [0., 0., 20.], 4), ("5-sk-E1-sel", EDGE, 120)] {
        let (plain, unselected) = rest_and_lit(cx, &m, None, target);
        let (sel, lit) = rest_and_lit(cx, &m, Some(target), target);
        let picked = cx.update(|cx| m.view.read(cx).sketch.as_ref().map(|ed| ed.model_selection.clone()).unwrap_or_default());
        let hovered = cx.update(|cx| m.view.read(cx).hover.clone());
        if picked.len() != 1 || hovered.as_ref() != picked.first() {
            bad.push(format!("{name}: selected {picked:?}, lit {hovered:?}"));
        }
        let n = changed(&sel, &lit);
        if n > most {
            bad.push(format!("{name}: {n} px (unselected {} px)", changed(&plain, &unselected)));
        }
    }
    assert!(bad.is_empty(), "picked or selected then hovered: {bad:?}");
}

/// Live hover-combined-1125 cells 7 (P0, hovering in a Shell's or a Fillet's preview): with Shell's top face removed, the
/// top front edge lights the front face (Shell takes faces), and the shelled wall's inner top edge the removed top face,
/// changing nothing; Fillet's tangent edge on top lights the top face, never an edge of the preview.
#[gpui::test]
fn combined_hover_in_shell_and_fillet_previews_takes_the_model_before_them(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "combined-1125-7");
    let mut bad = Vec::new();
    let hovered = |cx: &mut TestAppContext, at: [f64; 3]| {
        let p = m.screen(cx, at);
        approach(cx, &m, p);
        cx.update(|cx| m.view.read(cx).hover.clone())
    };
    let top = hovered(cx, FACE);
    assert!(matches!(top, Some(Reference::Face(_))), "the top face: {top:?}");
    m.park(cx);
    for ty in ["shell", "fillet"] {
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_feature(ty, w, cx))).unwrap();
        wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
        let pick = m.screen(cx, if ty == "shell" { FACE } else { EDGE });
        approach(cx, &m, pick);
        m.pointer.click(cx, &m.view, pick);
        wait_until(cx, m.window, |cx| m.view.read(cx).ui.preview_before.is_some() && !m.view.read(cx).ui_busy());
        m.park(cx);
        if ty == "shell" {
            let lit = hovered(cx, EDGE);
            if !matches!(lit, Some(Reference::Face(_))) || lit == top {
                bad.push(format!("7-shell-E1: {lit:?}"));
            }
            m.park(cx);
            let inner = [12., 2., 20.];
            let p = m.screen(cx, inner);
            let rest = m.crop(cx, p);
            let lit = hovered(cx, inner);
            let n = changed(&rest, &m.crop(cx, p));
            // (Live's hover is the removed face; ours lights nothing new on a face the field holds: the same pixels.)
            if (lit.is_some() && lit != top) || n > 4 {
                bad.push(format!("7-shell-EI: {lit:?}, {n} px"));
            }
        } else {
            let r = cx.update(|cx| m.view.read(cx).edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.expr("radius").and_then(|x| x.cached.map(|q| q.value)))).expect("the radius") * 1000.0;
            let lit = hovered(cx, [12., r, 20.]);
            if lit != top {
                bad.push(format!("7-fillet-FT (r {r} mm): {lit:?}"));
            }
        }
        m.park(cx);
        cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
        wait_until(cx, m.window, |cx| !m.view.read(cx).ui_busy());
    }
    assert!(bad.is_empty(), "hovered in a preview: {bad:?}");
}

/// Live hover-combined-1125 5-sk-V (P0): in a sketch, its point drawn on a model vertex takes the hover, not the vertex
/// (live JKI at 240 ms), and clicked it is the sketch point selected, unchanged when hovered again (5-sk-V-sel: 0 px).
#[gpui::test]
fn combined_hover_a_sketch_point_on_a_model_vertex_takes_the_hover(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "point": [[60, 25]] }, { "point": [[40, 0]] }] }
    ] });
    let m = matrix_studio_from(cx, "combined-1125-5v", start);
    m.edit_sketch(cx);
    let v = [40., 0., 0.];
    let (rest, lit) = rest_and_lit(cx, &m, None, v);
    let (point, model) = cx.update(|cx| {
        let view = m.view.read(cx);
        (view.sketch.as_ref().and_then(|ed| ed.hover), view.hover.clone())
    });
    assert!(point.is_some() && !matches!(model, Some(Reference::Vertex(_))), "the sketch point lit, not the vertex: {point:?} {model:?}");
    assert!(changed(&rest, &lit) > 20, "the point lights");
    let (sel, again) = rest_and_lit(cx, &m, Some(v), v);
    let selected = cx.update(|cx| m.view.read(cx).sketch.as_ref().map(|ed| (ed.selection.clone(), ed.model_selection.len())));
    assert!(selected.as_ref().is_some_and(|(s, n)| s.len() == 1 && *n == 0), "the sketch point selected: {selected:?}");
    assert!(changed(&sel, &again) <= 4, "selected then hovered: {} px", changed(&sel, &again));
}

/// Live ctx-menus-1117 sk-line (Sketch 2 edited, Sketch 1 under Extrude 1): a sketch line's menu has Hide other sketches
/// and Hide all sketches when the Part Studio has another sketch, shown or not (qa MENU-SK-HIDE-SKETCHES).
#[gpui::test]
fn sk_a_lines_menu_hides_other_sketches(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix_studio(cx, "sk-line-menu-hides");
    m.edit_sketch(cx);
    let p = m.screen(cx, SK_LINE);
    approach(cx, &m, p);
    m.pointer.right_click(cx, &m.view, p);
    let rows = cx.update(|cx| m.view.read(cx).context_menu.as_ref().map(|(_, menu, _)| menu.read(cx).labels()).unwrap_or_default());
    assert!(rows.iter().any(|r| r == "Hide other sketches") && rows.iter().any(|r| r == "Hide all sketches"), "{rows:?}");
}
