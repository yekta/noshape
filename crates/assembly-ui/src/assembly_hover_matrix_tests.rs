//! The hover × selection look matrix in an assembly (user 11-21, qa batch 4; live/psui/hover-matrix-1121 as2): one
//! instance of the 40 × 30 × 20 mm block with its Part Studio's Mate connector 1 on the corner (40, 30, 20), seen along
//! (-0.433, 0.75, -0.5) at 0.0506 mm/px (live after `f`), device pixel ratio 1, 80 px crops centred on the target.

use super::AssemblyView;
use super::feel_tests::{at_window, draw, frames, gpu_pointer, move_to, settle, wait_until};
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::scene_builder::PickKind;
use glam::{DVec2, DVec3};
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, Point, TestAppContext, WindowBounds, WindowOptions, px, size};

const VERTEX: [f64; 3] = [40., 0., 20.];
const EDGE: [f64; 3] = [10., 0., 20.];
const MID: [f64; 3] = [20., 0., 20.];
const FACE: [f64; 3] = [12., 12., 20.];
const MC: [f64; 3] = [40., 30., 20.];
/// "Another selected": the right face.
const OTHER: [f64; 3] = [40., 15., 10.];

struct Matrix {
    window: AnyWindowHandle,
    view: Entity<AssemblyView>,
}

/// Live's camera: orthographic, looking along (-0.433, 0.75, -0.5), 0.0506 mm per px.
fn live_camera(height_px: f64) -> noshape_render::Camera {
    let mut cam = noshape_render::Camera::default();
    let forward = DVec3::new(-0.433, 0.75, -0.5).normalize();
    let up = (DVec3::Z - forward * DVec3::Z.dot(forward)).normalize();
    cam.rotation = noshape_render::Camera::look_rotation(forward, up);
    cam.target = DVec3::new(0.02, 0.015, 0.01);
    cam.projection = noshape_render::Projection::Orthographic;
    cam.distance = 0.0506e-3 * height_px / (2.0 * (cam.fov_y * 0.5).tan());
    cam
}

/// qa's hm-asm start (the block, Sketch 2, Mate connector 1 on its corner, the part in the assembly) in a test window.
fn matrix(cx: &mut TestAppContext, name: &str) -> Matrix {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-matrix-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hm.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "mateConnector", "name": "Mate connector 1", "params": { "originQuery": ["vertex through [40, 30, 20]"] } }
    ], "assembly": { "instances": [{ "part": "Part 1" }] } });
    noshape_ops::ui_parity::build_start(&mut noshape_ops::Ctx::new(), &path, &start).unwrap();
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| t.name.ends_with(" asm")).expect("the start's assembly").id;
    let gpu = Gpu::headless_rendering().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| AssemblyView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            let front: gpui_kit::AnyView = cx.new(|_| gpui_kit::EmptyView).into();
            cx.new(|_| super::ui_tests::FrontHost { view: v, front })
        })
        .unwrap();
        (w, view.unwrap())
    });
    cx.update_window(window, |_, w, _| w.set_scale_factor(1.0)).unwrap();
    draw(cx, window);
    wait_until(cx, window, &view, |v, _| v.built.is_some() && v.result.occurrences.len() == 1);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            v.viewport.update(cx, |vp, cx| {
                vp.camera = live_camera(vp.size().y);
                vp.request_frame(cx);
            });
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    frames(cx, window, &view);
    Matrix { window, view }
}

impl Matrix {
    fn screen(&self, cx: &mut TestAppContext, mm: [f64; 3]) -> DVec2 {
        cx.update(|cx| self.view.read(cx).viewport.read(cx).project(DVec3::from(mm) * 1e-3).expect("on screen")).round()
    }

    fn park(&self, cx: &mut TestAppContext) {
        move_to(cx, self.window, &self.view, DVec2::new(30.0, 30.0));
    }

    /// The pointer on `p`, resting there as live's 1.2 s.
    fn hover(&self, cx: &mut TestAppContext, p: DVec2) {
        move_to(cx, self.window, &self.view, p + DVec2::new(0.0, 6.0));
        move_to(cx, self.window, &self.view, p);
        std::thread::sleep(std::time::Duration::from_millis(120));
        settle(cx, self.window, &self.view);
    }

    fn click(&self, cx: &mut TestAppContext, p: DVec2) {
        self.hover(cx, p);
        let w = at_window(cx, &self.view, p);
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_click(w, Modifiers::default());
        wait_until(cx, self.window, &self.view, |v, _| !v.ui.entity_sel.is_empty() || !v.selected.is_empty());
        settle(cx, self.window, &self.view);
    }

    /// The frame as drawn now (the scene and its screen-size glyphs), 80 × 80 px centred on `at` (row-major RGB).
    fn crop(&self, cx: &mut TestAppContext, at: DVec2) -> Vec<[u8; 3]> {
        frames(cx, self.window, &self.view);
        let (scene, camera, style, size, geometry) = cx.update(|cx| {
            let v = self.view.read(cx);
            let vp = v.viewport.read(cx);
            let (_, scene, camera, style) = vp.print_source();
            (scene, camera, style, vp.size(), v.ui.camera_geometry.clone())
        });
        let (w, h) = (size.x as u32, size.y as u32);
        let mut r = noshape_render::Renderer::new().expect("a renderer");
        let view = noshape_render::camera_geometry::View { camera, size };
        let mut scene = (*scene).clone();
        let up = r.uploader();
        scene.instances.extend(geometry.iter().flat_map(|g| g.build(&view, &up)));
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

#[derive(Clone, Copy, PartialEq)]
enum State {
    Hovered,
    Selected,
    SelectedHovered,
    HoveredOtherSelected,
}

/// A cell drawn: the crop round `target` in `state` (qa's gen-as.py steps).
fn cell(cx: &mut TestAppContext, name: &str, target: [f64; 3], state: State) -> Vec<[u8; 3]> {
    let m = matrix(cx, name);
    let p = m.screen(cx, target);
    match state {
        State::Hovered => {
            m.park(cx);
            m.hover(cx, p);
        }
        State::Selected | State::SelectedHovered => {
            m.click(cx, p);
            m.park(cx);
            if state == State::SelectedHovered {
                m.hover(cx, p);
            }
        }
        State::HoveredOtherSelected => {
            let other = m.screen(cx, OTHER);
            m.click(cx, other);
            m.park(cx);
            m.hover(cx, p);
        }
    }
    let crop = m.crop(cx, p);
    dump(name, &crop);
    crop
}

/// Writes a crop as a binary PPM (comparisons against live's crops; NOSHAPE_HOVER_MATRIX_DUMP names the folder).
fn dump(name: &str, crop: &[[u8; 3]]) {
    let Ok(dir) = std::env::var("NOSHAPE_HOVER_MATRIX_DUMP") else { return };
    let mut bytes = b"P6\n80 80\n255\n".to_vec();
    bytes.extend(crop.iter().flatten());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(std::path::Path::new(&dir).join(format!("as-{name}.ppm")), bytes);
}

/// The crop's pixels from `r0` to `r1` px off its centre (the pointer's hotspot).
fn between(crop: &[[u8; 3]], r0: f64, r1: f64) -> impl Iterator<Item = [u8; 3]> + '_ {
    crop.iter().enumerate().filter(move |(i, _)| (r0..=r1).contains(&DVec2::new((i % 80) as f64 - 40.0, (i / 80) as f64 - 40.0).length())).map(|(_, c)| *c)
}

fn close(c: [u8; 3], rgb: u32, tol: i32) -> bool {
    let w = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    c.iter().zip(w).all(|(a, b)| (*a as i32 - b as i32).abs() <= tol)
}

/// Whether a colour of live's (within 24 per channel) is drawn within `r` px of the target.
fn shows(crop: &[[u8; 3]], rgb: u32, r: f64) -> bool {
    between(crop, 0.0, r).any(|c| close(c, rgb, 24))
}

/// The move manipulator's centre as live's: a ring (~12 to ~18 px across) whose middle shows `rgb` near white (its
/// core), and nothing near white or the hover blue inside it (the pick shows through; the part's light blue #8cb9d4
/// is no hover blue).
fn white_ring(crop: &[[u8; 3]]) -> bool {
    between(crop, 6.0, 8.0).any(|c| close(c, 0xf5f5f6, 16)) && !between(crop, 0.0, 4.0).any(|c| close(c, 0xf5f5f6, 30) || close(c, 0x6087bd, 12))
}

/// The centre hovered: the same ring blue (#6087bd..#85a3d0), its middle still showing the pick.
fn blue_ring(crop: &[[u8; 3]]) -> bool {
    between(crop, 6.0, 8.0).any(|c| close(c, 0x85a3d0, 24))
        && !between(crop, 5.5, 9.0).any(|c| close(c, 0x3e94ff, 30) || close(c, 0xf5f5f6, 16))
        && !between(crop, 0.0, 4.0).any(|c| close(c, 0xf5f5f6, 30) || close(c, 0x6087bd, 12))
}

macro_rules! cells {
    () => {};
    ($name:ident: $target:expr, $state:expr => |$crop:ident| $check:block $($rest:tt)*) => {
        #[gpui::test]
        fn $name(cx: &mut TestAppContext) {
            let Some(_gpu) = gpu_pointer() else { return };
            let $crop = cell(cx, stringify!($name), $target, $state);
            $check
        }
        cells! { $($rest)* }
    };
}

cells! {
    // A vertex: the Part Studio's ~11 px see-through disc (#f0c28c..#f8c58a); selected #f9a912 inside the move
    // manipulator's hollow ring, which turns blue under the pointer.
    vertex_hovered: VERTEX, State::Hovered => |c| {
        assert!(shows(&c, 0xf6c285, 5.0) && between(&c, 3.5, 5.5).any(|p| close(p, 0xf6c285, 24)), "an 11 px disc");
    }
    vertex_selected: VERTEX, State::Selected => |c| {
        assert!(shows(&c, 0xf6a612, 4.0) && white_ring(&c));
    }
    vertex_selected_hovered: VERTEX, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xf6a612, 4.0) && blue_ring(&c));
    }
    vertex_hovered_other_selected: VERTEX, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xf6c285, 5.0));
    }
    // An edge: the 5 px #ffc685 band; selected #ffa90a through the ring.
    edge_hovered: EDGE, State::Hovered => |c| {
        assert!(shows(&c, 0xffc685, 3.0) && !shows(&c, 0xffa90a, 3.0));
    }
    edge_selected: EDGE, State::Selected => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && white_ring(&c));
    }
    edge_selected_hovered: EDGE, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && blue_ring(&c));
    }
    edge_hovered_other_selected: EDGE, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xffc685, 3.0));
    }
    // The edge's midpoint: the edge band only, no midpoint marker in an assembly.
    mid_hovered: MID, State::Hovered => |c| {
        assert!(shows(&c, 0xffc685, 3.0) && !shows(&c, 0xffd8ac, 2.0));
    }
    mid_selected: MID, State::Selected => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && white_ring(&c));
    }
    mid_selected_hovered: MID, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xffa90a, 3.0) && blue_ring(&c));
    }
    mid_hovered_other_selected: MID, State::HoveredOtherSelected => |c| {
        assert!(shows(&c, 0xffc685, 3.0) && !shows(&c, 0xffd8ac, 2.0));
    }
    // A face: no change inside it hovered; selected #e5a222 through the ring.
    face_hovered: FACE, State::Hovered => |c| {
        assert!(!shows(&c, 0xffc685, 12.0) && !shows(&c, 0xe5a222, 12.0));
    }
    face_selected: FACE, State::Selected => |c| {
        assert!(shows(&c, 0xe5a222, 3.0) && white_ring(&c));
    }
    face_selected_hovered: FACE, State::SelectedHovered => |c| {
        assert!(shows(&c, 0xe5a222, 3.0) && blue_ring(&c));
    }
    face_hovered_other_selected: FACE, State::HoveredOtherSelected => |c| {
        assert!(!shows(&c, 0xffc685, 12.0) && !shows(&c, 0xe5a222, 12.0));
    }
    // The part's own Mate connector 1: drawn at rest, hovered its disc #ffe79f, selected #ffe187 with the ring; either
    // way its part outlined in tan (#dab46b..#e9c177), not the hover's #ffc20e (qa FEEL-ASM-MC-LOOK).
    mc_hovered: MC, State::Hovered => |c| {
        assert!(shows(&c, 0xffe79f, 14.0) && shows(&c, 0xe2ba70, 40.0) && !shows(&c, 0xffc20e, 40.0));
    }
    mc_selected: MC, State::Selected => |c| {
        assert!(shows(&c, 0xffe187, 14.0) && between(&c, 6.0, 8.0).any(|p| close(p, 0xf5f5f6, 16)) && shows(&c, 0xe2ba70, 40.0));
    }
    // Hovered, the move ring over it stays one #6087bd with the connector's own blue in its middle (live
    // as-mc-selhover: 144 px of #0101d0; qa 7e58aaae8: the edge's #3a6ba8 rims had covered it, 245 px).
    mc_selected_hovered: MC, State::SelectedHovered => |c| {
        let ring = between(&c, 6.0, 8.0).filter(|p| close(*p, 0x6087bd, 16)).count();
        let (rims, connector) = (c.iter().filter(|p| close(**p, 0x3a6ba8, 8)).count(), c.iter().filter(|p| close(**p, 0x0101d0, 30)).count());
        assert!(shows(&c, 0xffe187, 14.0), "the selected connector's disc");
        // Ours 60 ring px, 0 rim px, 58 connector px; with the edge's ring 0 ring px.
        assert!(ring >= 30 && rims < 10 && connector >= 40, "{ring} ring px, {rims} rim px, {connector} connector px");
    }
    mc_hovered_other_selected: MC, State::HoveredOtherSelected => |c| {
        // No selected instance's #ffa033 band (as the silhouette test measures it: a blended pixel where the
        // other's #ffa90a meets the disc is no band).
        assert!(shows(&c, 0xffe79f, 14.0) && between(&c, 0.0, 40.0).filter(|p| close(*p, 0xffa033, 12)).count() < 2);
    }
}

/// Live triad-depth-1122 e1 (qa FEEL-ASM-MANIPULATOR-RING): a selected edge's #ffa90a band draws over the move
/// manipulator, hiding the X shaft that lies along the edge (no shaft core on the edge's line past the centre ring:
/// before, the shaft's white and grey strokes ran over the band).
#[gpui::test]
fn edge_band_over_triad(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix(cx, "edge_band_over_triad");
    let p = m.screen(cx, EDGE);
    m.click(cx, p);
    m.park(cx);
    let along = (m.screen(cx, [EDGE[0] + 5.0, EDGE[1], EDGE[2]]) - p).normalize();
    let crop = m.crop(cx, p);
    dump("edge_band_over_triad", &crop);
    let at = |d: f64| crop[((40.0 + along.y * d).round() as usize) * 80 + (40.0 + along.x * d).round() as usize];
    let line: Vec<[u8; 3]> = (11..36).flat_map(|d| [at(d as f64), at(-(d as f64))]).collect();
    // The band's 78 % core lets the shaft's white tint it, as live's (246,179,54); the shaft's own white never shows.
    assert!(line.iter().all(|c| c[0] >= 190 && c[2] <= 70 && !close(*c, 0xf7f7f8, 40)), "the band along the edge, over the shaft: {line:?}");
}

/// Live as2-edge-selhover_f (qa FEEL-ASM-RING-OVER-BAND): the hovered centre ring lies over the selected edge's band,
/// its blue where it crosses the edge (before, the band ran over it in orange).
#[gpui::test]
fn hovered_centre_ring_over_band(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix(cx, "hovered_centre_ring_over_band");
    let p = m.screen(cx, EDGE);
    m.click(cx, p);
    m.hover(cx, p);
    let along = (m.screen(cx, [EDGE[0] + 5.0, EDGE[1], EDGE[2]]) - p).normalize();
    let crop = m.crop(cx, p);
    dump("hovered_centre_ring_over_band", &crop);
    let at = |d: f64| crop[((40.0 + along.y * d).round() as usize) * 80 + (40.0 + along.x * d).round() as usize];
    let ring: Vec<[u8; 3]> = [-8.0, -7.0, -6.0, 6.0, 7.0, 8.0].into_iter().map(at).collect();
    let blue = |cs: &[[u8; 3]]| cs.iter().any(|c| c[2] > c[0] + 40);
    assert!(blue(&ring[..3]) && blue(&ring[3..]), "the blue ring over the band on both sides: {ring:?}");
    // A hollow blue stroke as live's (qa FEEL-ASM-MOVE-RING-RIMS): #3a6ba8 rims round a #99b2da core (live 17 / 16 px
    // within 8 in this crop, ours 120 / 16: our rims less blended at their edges; before, one flat #6087bd).
    let (rims, core) = (crop.iter().filter(|c| close(**c, 0x3a6ba8, 8)).count(), crop.iter().filter(|c| close(**c, 0x99b2da, 8)).count());
    assert!(rims >= 10 && core >= 10, "{rims} rim px, {core} core px (live 17 / 16)");
}

/// Live triad-depth-1122 t1 (hover matrix batch 4: "#ffa033 once selected"): a face selected selects its instance too,
/// the part keeping its own colour inside a 2 px #ffa033 band over its dark silhouette line (here along the right
/// face's back edge); where it runs along the selected face, the face's own #ffa90a boundary (live (255,169,10) ×3 on
/// the top's back edge). Before, no outline and no boundary.
#[gpui::test]
fn entity_selected_instance_outlined(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix(cx, "entity_selected_instance_outlined");
    let side = m.screen(cx, [40., 30., 10.]);
    let back = m.screen(cx, [20., 30., 20.]);
    let before = m.crop(cx, side);
    let dark = |c: &[u8; 3]| c.iter().all(|&v| v < 60);
    assert!(!shows(&before, 0xffa033, 3.0) && between(&before, 0.0, 3.0).any(|c| dark(&c)), "no outline at rest, the dark silhouette line");
    let p = m.screen(cx, FACE);
    m.click(cx, p);
    m.park(cx);
    let crop = m.crop(cx, side);
    dump("entity_selected_instance_outlined", &crop);
    assert!(between(&crop, 0.0, 3.0).filter(|c| close(*c, 0xffa033, 12)).count() >= 2, "the silhouette #ffa033");
    // Over the silhouette line, as live's (255,160,51) ×2 where the dark line was.
    assert!(!between(&crop, 0.0, 3.0).any(|c| dark(&c)), "the dark line under the band");
    assert!(!shows(&crop, 0xffc04c, 3.0), "not a whole part's #ffc04c");
    let crop = m.crop(cx, back);
    dump("entity_selected_instance_outlined_face", &crop);
    assert!(between(&crop, 0.0, 3.0).filter(|c| close(*c, 0xffa90a, 12)).count() >= 2, "the face's boundary over it");
}

/// Live b5-fastened-vertex (qa FEEL-ASM-MATE-VERTEX): Fastened's first field active, the pointer on the corner
/// (40, 0, 20): the vertex's ~11 px light orange disc (#f8c58a) with the white corner mark of the inferred connector
/// over it, no connector glyph (no triad's blue Z, no ring).
#[gpui::test]
fn fastened_vertex_hovered(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let m = matrix(cx, "fastened-vertex");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_mate(noshape_model::assembly::MateType::Fastened, w, cx))).unwrap();
    settle(cx, m.window, &m.view);
    let p = m.screen(cx, VERTEX);
    m.park(cx);
    m.hover(cx, p);
    let c = m.crop(cx, p);
    dump("fastened_vertex_hovered", &c);
    let hovered = cx.update(|cx| m.view.read(cx).hover.as_ref().map(|h| (h.kind, h.reference.clone())));
    assert!(matches!(hovered, Some((PickKind::Vertex, _))), "the vertex hovered, from its edge 6 px below (the inference dots never hide a pick): {hovered:?}");
    assert!(cx.update(|cx| m.view.read(cx).infer_choice.is_some()), "the vertex's point chosen");
    assert!(between(&c, 0.0, 1.5).all(|p| close(p, 0xffffff, 16)), "the white corner mark at the vertex");
    assert!([-4, 4].iter().all(|dx| close(c[40 * 80 + (40 + dx) as usize], 0xffffff, 16)), "the mark ~9 px wide, over the disc");
    assert!(between(&c, 4.5, 6.5).any(|p| close(p, 0xf8c58a, 20)), "inside the vertex's light orange disc");
    assert!(!between(&c, 0.0, 30.0).any(|p| p[2] > 180 && p[0] < 60 && p[1] < 60), "no connector triad (its blue Z)");
}

/// Interaction-feel "Combined states in an assembly" (live REQUESTS 07:02 (3) to confirm): with Fastened open, Mate
/// connector 1 takes the hover from its corner's inference dot (its glyph lit, no inferred point); a plain corner infers.
#[gpui::test]
fn combined_hover_mate_connector_in_a_mate_dialog(cx: &mut TestAppContext) {
    let m = matrix(cx, "mc-in-mate");
    cx.update_window(m.window, |_, w, cx| m.view.update(cx, |v, cx| v.start_mate(noshape_model::assembly::MateType::Fastened, w, cx))).unwrap();
    let (mc, corner) = (m.screen(cx, MC), m.screen(cx, VERTEX));
    m.park(cx);
    m.hover(cx, mc);
    assert!(cx.update(|cx| m.view.read(cx).hover_overlay.is_some() && m.view.read(cx).infer.is_none()), "the connector's glyph lit, no inference");
    m.hover(cx, corner);
    assert!(cx.update(|cx| m.view.read(cx).infer.is_some()), "a plain corner infers");
}
