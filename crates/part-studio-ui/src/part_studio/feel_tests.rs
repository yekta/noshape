//! Interaction feel through the real pointer and the GPU's own pick (docs/parity/interaction-feel.md): what lights
//! and what a click takes, at a device pixel ratio of 1 and 2. They run on every build under the GPU lock, and skip
//! (saying so) where there is no adapter or its pick doesn't come back.

use super::*;

/// The GPU lock, once a GPU's pick is known to come back: a point drawn at the frame's centre picks there. None (and
/// why) where it doesn't: CI's Linux runner has no adapter, macOS CI's readback came out white (run 42333748082).
pub(super) fn gpu_pointer() -> Option<parking_lot::MutexGuard<'static, ()>> {
    let lock = noshape_ops::doc::gpu_lock();
    if let Err(e) = Gpu::headless_rendering() {
        eprintln!("feel: GPU pointer tests skipped: {e}");
        return None;
    }
    use noshape_render::{Camera, Drawable, Instance, PointData, PointSprite, RenderParams, Renderer, Scene, StandardView, Style};
    let mut r = Renderer::new().ok()?;
    let points = PointData { points: vec![PointSprite { position: [0.0; 3], color: [0, 0, 255, 255], pick_id: 7, size: 6.0, style: 2, _pad: 0 }] };
    let scene = Scene { instances: vec![Instance::new(Drawable::Points(r.upload_points(&points))).per_primitive_color()], ..Default::default() };
    let mut cam = Camera::default();
    cam.set_view(StandardView::Top);
    cam.fit_sphere(glam::DVec3::ZERO, 0.03, 1.0);
    let ok = r
        .render(RenderParams { scene: &scene, camera: &cam, width: 64, height: 64, device_pixel_ratio: 1.0, style: &Style::default(), pick: true })
        .ok()
        .and_then(|f| f.pick)
        .is_some_and(|p| p.pick(32, 32, 3, |id| if id == 7 { 1 } else { 0 }) == Some(7));
    if !ok {
        eprintln!("feel: GPU pointer tests skipped: this GPU's pick doesn't come back");
        return None;
    }
    Some(lock)
}

/// The pointer in a test window's viewport, by viewport-local CSS px.
pub(super) struct Pointer {
    window: AnyWindowHandle,
    pub(super) vp: Entity<crate::viewport::Viewport>,
}

impl Pointer {
    pub(super) fn new(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>) -> Self {
        let vp = cx.update(|cx| view.read(cx).viewport.clone());
        Self { window, vp }
    }

    pub(super) fn window_point(&self, cx: &mut TestAppContext, at: DVec2) -> Point<gpui_kit::Pixels> {
        let origin = cx.update(|cx| self.vp.read(cx).origin());
        gpui_kit::point(origin.x + px(at.x as f32), origin.y + px(at.y as f32))
    }

    /// Moves there and waits for the GPU's hover pick of it to show (frames delivered meanwhile).
    pub(super) fn move_to(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>, at: DVec2) {
        let win = self.window_point(cx, at);
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_mouse_move(win, None, Modifiers::default());
        let t = std::time::Instant::now();
        // (A jump is a fast move: the hover waits for the pointer to pause, as live's pre-highlight.)
        while !cx.update(|cx| self.vp.read(cx).pick_under_pointer().is_some() && self.vp.read(cx).hover_settled()) && t.elapsed() < std::time::Duration::from_secs(15) {
            cx.update(|cx| self.vp.update(cx, |v, cx| v.request_frame(cx)));
            cx.update(|cx| view.read(cx).gpu.deliver_frames());
            draw(cx, self.window);
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // Until the scene shows the new hover and the pick is drawn again with it, so the next pick holds a lit line (a
        // person's pointer takes many frames to get anywhere; a loaded machine builds the scene slowly).
        self.settle(cx, view);
    }

    fn settle(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>) {
        let t = std::time::Instant::now();
        self.frames(cx, view);
        while cx.update(|cx| view.read(cx).ui.scene_task.is_some() || self.vp.read(cx).pick_pending() || self.vp.read(cx).hover_pending()) && t.elapsed() < std::time::Duration::from_secs(15) {
            self.frames(cx, view);
        }
    }

    pub(super) fn frames(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>) {
        for _ in 0..5 {
            cx.update(|cx| self.vp.update(cx, |v, cx| v.request_frame(cx)));
            cx.update(|cx| view.read(cx).gpu.deliver_frames());
            draw(cx, self.window);
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Clicks there and lets the click's pick come back (a hover just changed is picked again first; a loaded
    /// software GPU can take a while).
    pub(super) fn click(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>, at: DVec2) {
        let win = self.window_point(cx, at);
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_click(win, Modifiers::default());
        self.settle(cx, view);
    }
}

/// The test window at a device pixel ratio, redrawn.
fn at_scale(cx: &mut TestAppContext, window: AnyWindowHandle, scale: f32) {
    cx.update_window(window, |_, w, _| w.set_scale_factor(scale)).unwrap();
    draw(cx, window);
}

/// Outside sketch mode a click acts on exactly what is lit (lead 11-12, after the sketch reach fix), and an edge lights
/// as live's (live/feel/3d.json "hover_reach_px", a level edge: lights at 6–7 px) on either side, over the background
/// and over its own face, and lets go past its hold. The pointer moves across a part edge a quarter along it, toward
/// it and back out on each side, and the click selects what lights there. (The hold to 9–10 px: offscreen
/// `our_3d_pick_reach_for_edges_and_vertices`.)
#[gpui::test]
fn a_click_beside_an_edge_selects_what_is_lit(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    for scale in [1.0, 2.0] {
        let (window, view, _top, _) = block_part(cx, &format!("click-is-hover-{scale}"));
        at_scale(cx, window, scale);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let pointer = Pointer::new(cx, window, &view);
        // The top face's longest edge on screen (all four are seen from the iso view), a point on it and the direction
        // across it.
        let (edge, on, across) = cx.update(|cx| {
            let v = view.read(cx);
            let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
            let vp = v.viewport.read(cx);
            let s = |p: &[f32; 3]| vp.project(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
            let top_z = b.mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
            b.edge_refs
                .iter()
                .zip(&b.mesh.edges)
                .filter(|(_, pl)| pl.iter().all(|p| (p[2] - top_z).abs() < 1e-7))
                .filter_map(|(r, pl)| {
                    let (a, z) = (s(pl.first()?)?, s(pl.last()?)?);
                    // (A quarter along: by its middle a rest lights the edge's midpoint target, live 1954.)
                    Some((r.clone(), a + (z - a) * 0.25, (z - a).perp().normalize(), (z - a).length()))
                })
                .max_by(|a, b| a.3.total_cmp(&b.3))
                .map(|(r, m, n, _)| (r, m, n))
                .unwrap()
        });
        // The pick square reaches a slanted edge further than a level one: 5 px × (|nx| + |ny|) to its pick band (1.5
        // px each side at rest, 4.5 lit).
        let square = 5.0 * (across.x.abs() + across.y.abs());
        let (reach, hold) = (square + 1.5, square + 4.5);
        let steps = [-1.0, 1.0f64].into_iter().flat_map(|side| {
            [(2.0 * hold, false), (reach + 1.5, false), (reach - 1.0, true), (hold + 1.5, false)].map(|(d, lit)| (side * d, lit))
        });
        for (off, lights) in steps {
            cx.update_window(window, |_, _, cx| {
                view.update(cx, |v, cx| {
                    // (Each click on its own: not the second of a double-click on the plane behind.)
                    v.selection.clear();
                    v.ui.last_click = None;
                    v.rebuild_scene(cx);
                })
            })
            .unwrap();
            pointer.frames(cx, &view);
            let at = on + across * off;
            pointer.move_to(cx, &view, at);
            let lit = cx.update(|cx| view.read(cx).hover.clone());
            pointer.click(cx, &view, at);
            let selected = cx.update(|cx| view.read(cx).selection.clone());
            assert_eq!(selected, lit.clone().into_iter().collect::<Vec<_>>(), "scale {scale}, {off} px off the edge: the click selects what was lit");
            assert_eq!(lit.as_ref() == Some(&edge), lights, "scale {scale}, {off} px off the edge: lit {lit:?}");
        }
    }
}

/// User P0 11-12 (hat15b, 7th report): the Dimension tool's hover lit a line but the click started nothing. With the
/// GPU's own hover pick, what lights is what the click takes, within live's Dimension reach (live2's catalogue: a line
/// from 8 px, not 9): on the line's drawn centre, 4 and 8 px beside it the line is lit and a click starts its
/// dimension; 10 px beside it nothing is lit or taken.
#[gpui::test]
fn the_dimension_tool_takes_the_hovered_line(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    for scale in [1.0, 2.0] {
        for (off, lit) in [(0.0, true), (4.0, true), (7.5, true), (10.0, false)] {
            let (window, view) = line_then_dimension_tool(cx, &format!("dim-hover-{scale}-{off}"));
            at_scale(cx, window, scale);
            let pointer = Pointer::new(cx, window, &view);
            let (line, centre) = cx.update(|cx| {
                let v = view.read(cx);
                let data = v.sketch_data(cx).unwrap();
                let ed = v.sketch.as_ref().unwrap();
                let line = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
                let Some(noshape_sketch::Curve2::Line { a, b }) = data.sketch.curve(line) else { panic!("a line") };
                (line, v.viewport.read(cx).project(ed.frame.to_world((a + b) * 0.5)).unwrap())
            });
            // Off the line across it (it runs from (500, 400) to (640, 360) on screen).
            let at = centre + DVec2::new(40., 140.).normalize() * off;
            pointer.move_to(cx, &view, at);
            assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().is_some_and(|ed| ed.hover == Some(line))), lit, "scale {scale}: the line lit {off} px off it");
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
                    v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() }, w, cx);
                });
            })
            .unwrap();
            draw(cx, window);
            let picks = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().tool_picks());
            assert_eq!(picks.contains(&line), lit, "scale {scale}: the click {off} px off takes the line: {picks:?}");
        }
    }
}

/// Live/feel/3d.json "drag_thresholds" through the real pointer: a right drag orbits from its first pixel and its
/// release still opens the context menu within 4 px (not from 5); a left press moved 6 px within its first 50 ms is
/// a click, moved 3 px later a box. The press rules read a clock the test sets (T1: no wall-clock time).
#[gpui::test]
fn presses_orbit_box_and_open_the_menu_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    for scale in [1.0, 2.0] {
        let (window, view, _top, _) = block_part(cx, &format!("presses-{scale}"));
        at_scale(cx, window, scale);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let pointer = Pointer::new(cx, window, &view);
        // The part's middle on screen.
        let at = cx.update(|cx| {
            let v = view.read(cx);
            let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
            let c = b.mesh.positions.iter().fold(glam::DVec3::ZERO, |c, p| c + glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)) / b.mesh.positions.len() as f64;
            v.viewport.read(cx).project(c).unwrap()
        });
        pointer.move_to(cx, &view, at);
        let clock = std::rc::Rc::new(std::cell::Cell::new(std::time::Instant::now()));
        let c = clock.clone();
        cx.update(|cx| pointer.vp.update(cx, |vp, _| vp.set_clock(move || c.get())));
        let tick = |ms: u64| clock.set(clock.get() + std::time::Duration::from_millis(ms));
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        // A right press moved `dx` px and released 100 ms later.
        let right = |vcx: &mut gpui_kit::VisualTestContext, dx: f64| {
            let (from, to) = (pointer.window_point(vcx, at), pointer.window_point(vcx, at + DVec2::new(dx, 0.0)));
            vcx.simulate_mouse_down(from, gpui_kit::MouseButton::Right, Modifiers::default());
            tick(50);
            vcx.simulate_mouse_move(to, gpui_kit::MouseButton::Right, Modifiers::default());
            tick(50);
            vcx.simulate_mouse_up(to, gpui_kit::MouseButton::Right, Modifiers::default());
        };
        for (dx, menu) in [(3.0, true), (4.0, true), (6.0, false)] {
            cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.close_context_menu(cx))).unwrap();
            pointer.frames(cx, &view);
            let before = cx.update(|cx| pointer.vp.read(cx).camera);
            right(&mut vcx, dx);
            pointer.settle(cx, &view);
            assert_ne!(cx.update(|cx| pointer.vp.read(cx).camera), before, "scale {scale}: a {dx} px right drag orbits");
            assert_eq!(cx.update(|cx| view.read(cx).context_menu.is_some()), menu, "scale {scale}: the menu after a {dx} px right drag");
        }
        cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.close_context_menu(cx))).unwrap();
        pointer.frames(cx, &view);
        // Left: 6 px at once is still a click (it selects what is under the pointer); 3 px after 60 ms is a box.
        let left = |vcx: &mut gpui_kit::VisualTestContext, dx: f64, wait: u64| {
            let (from, to) = (pointer.window_point(vcx, at), pointer.window_point(vcx, at + DVec2::new(dx, 0.0)));
            vcx.simulate_mouse_down(from, gpui_kit::MouseButton::Left, Modifiers::default());
            tick(wait);
            vcx.simulate_mouse_move(to, gpui_kit::MouseButton::Left, Modifiers::default());
            let boxed = vcx.update(|_, cx| pointer.vp.read(cx).box_drag());
            vcx.simulate_mouse_up(to, gpui_kit::MouseButton::Left, Modifiers::default());
            boxed
        };
        pointer.move_to(cx, &view, at);
        assert!(!left(&mut vcx, 6.0, 0), "scale {scale}: 6 px within 50 ms stays a click");
        pointer.frames(cx, &view);
        assert!(cx.update(|cx| !view.read(cx).selection.is_empty()), "scale {scale}: the click selected what is under the pointer");
        assert!(left(&mut vcx, 3.0, 60), "scale {scale}: 3 px after 60 ms is a box");
    }
}

/// Box select as live's counts on a block (live/feel/3d.json "box_select"), from above: a window round the whole block
/// takes its 6 faces and 12 edges (18: the hidden ones too, no vertices, no part); round its right half (the right
/// end face, edge-on) 5; a crossing box over that half 13 (the 5 faces and 8 edges it touches). No GPU: what the box
/// takes comes from the shown geometry.
#[gpui::test]
fn box_select_takes_what_live_takes(cx: &mut TestAppContext) {
    let (window, view, _top, _) = block_part(cx, "box-counts");
    for scale in [1.0f32, 2.0] {
        at_scale(cx, window, scale);
        let (lo, hi) = cx.update_window(window, |_, _, cx| {
            view.update(cx, |v, cx| {
                let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
                let p = |q: &[f32; 3]| glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64);
                let lo = b.mesh.positions.iter().fold(glam::DVec3::splat(f64::MAX), |a, q| a.min(p(q)));
                let hi = b.mesh.positions.iter().fold(glam::DVec3::splat(f64::MIN), |a, q| a.max(p(q)));
                v.viewport.update(cx, |vp, cx| {
                    let size = vp.size();
                    vp.camera.set_view(noshape_render::StandardView::Top);
                    vp.camera.fit_sphere((lo + hi) * 0.5, (hi - lo).length() * 0.6, size.x / size.y);
                    vp.request_frame(cx);
                });
                (lo, hi)
            })
        })
        .unwrap();
        draw(cx, window);
        let (a, b) = cx.update(|cx| {
            let vp = view.read(cx).viewport.read(cx);
            let (a, b) = (vp.project(lo).unwrap(), vp.project(hi).unwrap());
            (a.min(b), a.max(b))
        });
        let mid_x = (a.x + b.x) * 0.5;
        let m = DVec2::splat(15.0);
        // (Row, the box's corners, crossing, live's count.)
        let rows = [
            ("window round the block", (a - m, b + m), false, 18),
            ("window round its right half", (DVec2::new(mid_x + 20.0, a.y - 15.0), b + m), false, 5),
            ("crossing over its right half", (DVec2::new(mid_x + 20.0, a.y - 15.0), b + m), true, 13),
        ];
        for (name, corners, crossing, live) in rows {
            let n = cx.update(|cx| view.read(cx).box_ids(corners, crossing, cx).len());
            assert_eq!(n, live, "scale {scale}: {name}");
        }
    }
}

/// User P0 11-17, out of sketch mode: a part in the default iso view with the default planes behind and through it.
/// Under the real pointer every face's middle lights the part (never a plane behind it), a top corner 5 px
/// out to the side lights its vertex (live/feel/3d.json: a vertex lights at 7 px level), and beside the part a plane
/// lights.
#[gpui::test]
fn part_faces_and_vertices_light_over_the_planes_behind(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _top, _) = block_part(cx, "3d-over-planes");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // Each face's middle and the part's corners on screen.
    let (faces, corners) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let vp = v.viewport.read(cx);
        let p = |q: &[f32; 3]| glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64);
        let faces: Vec<(Reference, DVec2)> = b
            .face_refs
            .iter()
            .zip(&b.mesh.face_ranges)
            .filter_map(|(r, (first, count))| {
                let idx = &b.mesh.indices[*first as usize..(*first + *count) as usize];
                let c = idx.iter().map(|&k| p(&b.mesh.positions[k as usize])).sum::<glam::DVec3>() / idx.len() as f64;
                Some((r.clone(), vp.project(c)?))
            })
            .collect();
        let top_z = b.mesh.positions.iter().map(|q| q[2]).fold(f32::MIN, f32::max);
        let mut corners: Vec<DVec2> = b.mesh.positions.iter().filter(|q| (q[2] - top_z).abs() < 1e-7).filter_map(|q| vp.project(p(q))).collect();
        corners.dedup_by(|a, b| (*a - *b).length() < 0.5);
        (faces, corners)
    });
    assert_eq!(faces.len(), 6, "the block's six faces");
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let mut wrong = Vec::new();
    for (face, at) in &faces {
        pointer.move_to(cx, &view, *at);
        let got = lit(cx);
        // (A hidden face's middle may fall by a seen edge, which lights: the part, never a plane behind it.)
        if !matches!(got, Some(Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_))) {
            wrong.push(format!("the middle of {face:?} ({:.0}, {:.0}): lit {got:?}, not the part", at.x, at.y));
        }
    }
    // The leftmost and rightmost top corners, 5 px further out.
    let centre = corners.iter().sum::<DVec2>() / corners.len() as f64;
    for corner in [corners.iter().min_by(|a, b| a.x.total_cmp(&b.x)), corners.iter().max_by(|a, b| a.x.total_cmp(&b.x))].into_iter().flatten() {
        let at = *corner + DVec2::new(5.0 * (corner.x - centre.x).signum(), 0.0);
        pointer.move_to(cx, &view, at);
        let got = lit(cx);
        if !matches!(got, Some(Reference::Vertex(_))) {
            wrong.push(format!("5 px beside the corner ({:.0}, {:.0}): lit {got:?}, not its vertex", corner.x, corner.y));
        }
    }
    // Beside the part: some spot lights a plane.
    let (lo, hi) = corners.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(l, h), c| (l.min(*c), h.max(*c)));
    let spots = [DVec2::new(lo.x - 50.0, hi.y + 60.0), DVec2::new(hi.x + 50.0, hi.y + 60.0), DVec2::new(lo.x - 80.0, lo.y), DVec2::new(hi.x + 80.0, lo.y), DVec2::new((lo.x + hi.x) * 0.5, hi.y + 120.0)];
    let plane = spots.iter().any(|s| {
        pointer.move_to(cx, &view, *s);
        matches!(lit(cx), Some(Reference::Plane(_)))
    });
    if !plane {
        wrong.push("no plane lit beside the part".into());
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// live psui/hover-pick-1120 (lead 20:28, P0): a block wholly behind the Front plane in the default iso view. Through
/// the transparent plane the block lights under the real pointer, never the plane: every face's middle; its
/// top-left edge (a silhouette with only the plane beyond) 4 px out and 3 px in (live 5–6 / 4); 14 px out the plane,
/// which lights only where nothing is behind it, and 14 px in the face.
#[gpui::test]
fn a_block_behind_the_front_plane_lights_through_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let doc = serde_json::json!({"units": "mm", "features": [
        {"type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{"corner_rectangle": [[10, 10], [50, 40]]}]},
        {"type": "extrude", "name": "Extrude 1", "params": {"entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW"}}
    ]});
    let (window, view) = part_studio_from(cx, "behind-front-plane", Some(doc));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.iter().any(|b| b.kind == noshape_model::display::BodyKind::Solid));
    draw(cx, window);
    let (faces, edge) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let vp = v.viewport.read(cx);
        let p = |q: &[f32; 3]| glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64);
        let faces: Vec<DVec2> = b
            .mesh
            .face_ranges
            .iter()
            .map(|(first, count)| &b.mesh.indices[*first as usize..(*first + *count) as usize])
            .filter_map(|idx| vp.project(idx.iter().map(|&k| p(&b.mesh.positions[k as usize])).sum::<glam::DVec3>() / idx.len() as f64))
            .collect();
        // The top face's left edge (x 10, z 10 mm, along y; a silhouette with only the plane beyond it in iso): a spot on
        // screen a third along it and the screen direction away from the block.
        let (r, pl) = b.edge_refs.iter().zip(&b.mesh.edges).find(|(_, pl)| pl.iter().all(|q| (q[0] - 0.01).abs() < 1e-6 && (q[2] - 0.01).abs() < 1e-6)).expect("the edge at x 10, z 10");
        let (a, z) = (vp.project(p(&pl[0])).unwrap(), vp.project(p(pl.last().unwrap())).unwrap());
        // (Not its middle: a rest there lights the edge's midpoint target, live 1954.)
        let mid = a + (z - a) / 3.0;
        let centre = vp.project(glam::DVec3::new(0.03, 0.025, 0.005)).unwrap();
        let mut out = (z - a).perp().normalize();
        if out.dot(mid - centre) < 0.0 {
            out = -out;
        }
        (faces, (r.clone(), mid, out))
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let mut wrong = Vec::new();
    for at in &faces {
        pointer.move_to(cx, &view, *at);
        let got = lit(cx);
        if !matches!(got, Some(Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_))) {
            wrong.push(format!("a face's middle ({:.0}, {:.0}): lit {got:?}, not the block", at.x, at.y));
        }
    }
    let (edge_ref, mid, out) = edge;
    for (d, want) in [(14.0, "plane"), (4.0, "edge"), (-3.0, "edge"), (-14.0, "face")] {
        // (Each from afar, as a fresh approach: a lit edge holds further.)
        pointer.move_to(cx, &view, mid + out * 60.0);
        pointer.move_to(cx, &view, mid + out * d);
        let got = lit(cx);
        let ok = match want {
            "plane" => matches!(got, Some(Reference::Plane(_))),
            "edge" => got.as_ref() == Some(&edge_ref),
            _ => matches!(got, Some(Reference::Face(_))),
        };
        if !ok {
            wrong.push(format!("{d:+} px off the top-left edge: lit {got:?}, want the {want}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// qa SELFEEL-PART-ROW (11-17, green 592b0fc9; live2 sel-feel row 5): through the Parts list with real pointer
/// events, Part 1's row hovered, then Part 2's, then a click on it: Part 2 is selected and lit as hovered, Part 1's
/// hover outline doesn't stay.
#[gpui::test]
fn a_part_row_clicked_after_passing_another_leaves_no_stale_hover(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "part-row-hover", Some(starts["two-blocks"].clone()));
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count() == 2 && !view.read(cx).ui_busy());
    draw(cx, window);
    let ids: Vec<_> = cx.update(|cx| view.read(cx).display.bodies.iter().map(|b| b.id.clone()).collect());
    let row = |cx: &mut TestAppContext, id: &noshape_model::reference::BodyId| {
        let sel: &'static str = Box::leak(format!("part-row-{id}").into_boxed_str());
        gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(sel).expect("the part's row").center()
    };
    let (r1, r2) = (row(cx, &ids[0]), row(cx, &ids[1]));
    let mv = |cx: &mut TestAppContext, at: Point<gpui_kit::Pixels>| {
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(at, None, Modifiers::default());
        draw(cx, window);
    };
    let hover = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let (p1, p2) = (Reference::Body(ids[0].clone()), Reference::Body(ids[1].clone()));
    mv(cx, r1);
    assert_eq!(hover(cx), Some(p1.clone()), "Part 1's row lights Part 1");
    for k in 1..=4 {
        mv(cx, gpui_kit::point(r1.x, r1.y + (r2.y - r1.y) * (k as f32 / 4.0)));
    }
    assert_eq!(hover(cx), Some(p2.clone()), "on Part 2's row, Part 2 lit and Part 1 not");
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(r2, Modifiers::default());
    draw(cx, window);
    let selection = cx.update(|cx| view.read(cx).selection.clone());
    assert_eq!(selection, vec![p2.clone()], "the click selects Part 2");
    assert_ne!(hover(cx), Some(p1), "Part 1's outline doesn't stay");
}

/// live psui/feature-dialogs-1117 (lead 11-17 ~22:40, P0): while a dialog's first box is active, hover is filtered by
/// what the box takes, under the real pointer in iso with the planes shown. Extrude: an edge lights the face beside it
/// (never the edge); Fillet / Chamfer: a vertex lights an edge at it; Boolean: a face lights the whole part; a plane's
/// empty spot lights nothing in any of them.
#[gpui::test]
fn a_dialogs_active_box_filters_what_lights(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _top, _) = block_part(cx, "dialog-hover-filter");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // The top face's longest edge a third along, a top corner, the top face's middle and an empty spot over a plane.
    let (edge_at, corner, face_at) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let vp = v.viewport.read(cx);
        let p = |q: &[f32; 3]| glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64);
        let top_z = b.mesh.positions.iter().map(|q| q[2]).fold(f32::MIN, f32::max);
        let (a, z) = b
            .mesh
            .edges
            .iter()
            .filter(|pl| pl.iter().all(|q| (q[2] - top_z).abs() < 1e-7))
            .filter_map(|pl| Some((vp.project(p(pl.first()?))?, vp.project(p(pl.last()?))?)))
            .max_by(|x, y| (x.1 - x.0).length().total_cmp(&(y.1 - y.0).length()))
            .unwrap();
        let tops: Vec<DVec2> = b.mesh.positions.iter().filter(|q| (q[2] - top_z).abs() < 1e-7).filter_map(|q| vp.project(p(q))).collect();
        let centre = tops.iter().sum::<DVec2>() / tops.len() as f64;
        (a + (z - a) / 3.0, a, centre)
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let plane_spot = [DVec2::new(face_at.x - 220.0, face_at.y + 120.0), DVec2::new(face_at.x + 220.0, face_at.y + 120.0), DVec2::new(face_at.x, face_at.y + 200.0)]
        .into_iter()
        .find(|s| {
            pointer.move_to(cx, &view, *s);
            matches!(lit(cx), Some(Reference::Plane(_)))
        })
        .expect("a plane beside the part, no dialog");
    let mut wrong = Vec::new();
    for (ty, at, want) in [
        ("extrude", edge_at, "face"),
        ("fillet", corner, "edge"),
        ("chamfer", corner, "edge"),
        ("booleanBodies", face_at, "part"),
    ] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature(ty, w, cx))).unwrap();
        wait_until(cx, window, |cx| view.read(cx).edit.is_some() && !view.read(cx).ui_busy());
        pointer.move_to(cx, &view, at);
        // (What is drawn lit: a face under a parts box stands for its part.)
        let got = cx.update(|cx| view.read(cx).highlighted_hover(cx));
        let ok = match want {
            "face" => matches!(got.as_slice(), [Reference::Face(_)]),
            "edge" => matches!(got.as_slice(), [Reference::Edge(_)]),
            _ => matches!(got.as_slice(), [Reference::Body(_)]),
        };
        if !ok {
            wrong.push(format!("{ty}: lit {got:?}, want a {want}"));
        }
        pointer.move_to(cx, &view, plane_spot);
        if let Some(got) = lit(cx) {
            wrong.push(format!("{ty}: a plane's spot lit {got:?}, want nothing"));
        }
        keys(cx, window, &["escape", "escape"]);
        wait_until(cx, window, |cx| view.read(cx).edit.is_none());
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
