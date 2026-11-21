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
    RENDERING.set(true);
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
        cx.update(|cx| self.view.read(cx).viewport.read(cx).project(DVec3::from(mm) * 1e-3).expect("on screen")).round()
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
        self.pointer.frames(cx, &self.view);
        let (scene, camera, style, size) = cx.update(|cx| {
            let vp = self.view.read(cx).viewport.read(cx);
            let (_, scene, camera, style) = vp.print_source();
            (scene, camera, style, vp.size())
        });
        let (w, h) = (size.x as u32, size.y as u32);
        let mut r = noshape_render::Renderer::new().expect("a renderer");
        let f = r.render(noshape_render::RenderParams { scene: &scene, camera: &camera, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).expect("a frame");
        let (x0, y0) = (at.x as i32 - 40, at.y as i32 - 40);
        let mut out = Vec::with_capacity(6400);
        for y in y0..y0 + 80 {
            for x in x0..x0 + 80 {
                let (cx_, cy_) = (x.clamp(0, f.width as i32 - 1), y.clamp(0, f.height as i32 - 1));
                let i = ((cy_ * f.width as i32 + cx_) * 4) as usize;
                out.push([f.bgra[i + 2], f.bgra[i + 1], f.bgra[i]]);
            }
        }
        out
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
