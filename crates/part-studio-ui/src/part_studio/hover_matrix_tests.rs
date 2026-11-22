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
    // (#b28a5d: the muted boundary gave ~80; the preview's tangent line beside the band blends to a few, live 9.)
    assert!(picked && count(&c, 0xffc685) > 150 && count(&c, 0xb28a5d) < 50, "{} #ffc685, {} #b28a5d", count(&c, 0xffc685), count(&c, 0xb28a5d));
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
        let asc = 0.031 * 2.0 * pl.half_height;
        v.viewport.read(cx).project(pl.corner(-1.0, 1.0) + pl.frame.x * (1.2 * asc) - pl.frame.y * (0.5 * asc))
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
    // p04: the Top plane's row: its outline.
    set(cx, &|v| v.hover = Some(Reference::Plane(DatumPlane::Top)));
    let edge = cx.update(|cx| {
        let v = m.view.read(cx);
        let vp = v.viewport.read(cx);
        let pl = v.display.planes.iter().find(|q| q.reference == Reference::Plane(DatumPlane::Top))?;
        [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)].into_iter().filter_map(|(u, w)| vp.project(pl.corner(u, w))).find(|s| s.x > 10.0 && s.y > 10.0 && s.x < vp.size().x - 10.0 && s.y < vp.size().y - 10.0)
    });
    match edge {
        Some(edge) if near(&m.frame(cx), edge.round(), 4, 0xffc685, 40) >= 3 => {}
        Some(_) => wrong.push("p04 the Top plane's row: its outline not lit".to_string()),
        None => wrong.push("p04: no edge of the Top plane on screen".to_string()),
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
