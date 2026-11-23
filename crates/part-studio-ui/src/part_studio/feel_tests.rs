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

    pub(super) fn settle(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>) {
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

    /// Right-clicks there and lets the menu's pick come back.
    pub(super) fn right_click(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>, at: DVec2) {
        let position = self.window_point(cx, at);
        let mut vcx = gpui_kit::VisualTestContext::from_window(self.window, cx);
        vcx.simulate_mouse_down(position, gpui_kit::MouseButton::Right, Modifiers::default());
        vcx.simulate_mouse_up(position, gpui_kit::MouseButton::Right, Modifiers::default());
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
        // Live reach-px-1119 (full-page shots): an edge lights from 5 px over the background, 7 over a face, slanted
        // or not, with no hold (lit on the way back to the same distance). Which side is which: a face lit far off (a
        // plane behind counts as the background).
        let side_reach = |cx: &mut TestAppContext, side: f64| {
            pointer.move_to(cx, &view, on + across * (side * 14.0));
            if cx.update(|cx| matches!(view.read(cx).hover, Some(Reference::Face(_)))) { 7.0 } else { 5.0 }
        };
        let reaches = [(-1.0, side_reach(cx, -1.0)), (1.0, side_reach(cx, 1.0))];
        let steps = reaches.into_iter().flat_map(|(side, reach)| {
            [(14.0, false), (reach + 1.0, false), (reach - 0.5, true), (reach + 1.0, false)].map(|(d, lit)| (side * d, lit))
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
/// release still opens the context menu within 4 device px (not from 5); a left press moved 6 px within its first 50 ms is
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
        // Device px (live's CONTEXT_MENU_DISTANCE_THRESHOLD has no `_PX`: 2 CSS px at a ratio of 2).
        for (dx, menu) in [(3.0, true), (4.0, true), (6.0, false)] {
            cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.close_context_menu(cx))).unwrap();
            pointer.frames(cx, &view);
            let before = cx.update(|cx| pointer.vp.read(cx).camera);
            right(&mut vcx, dx / f64::from(scale));
            pointer.settle(cx, &view);
            assert_ne!(cx.update(|cx| pointer.vp.read(cx).camera), before, "scale {scale}: a {dx} device px right drag orbits");
            assert_eq!(cx.update(|cx| view.read(cx).context_menu.is_some()), menu, "scale {scale}: the menu after a {dx} device px right drag");
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
        // Live sel-gestures-1122: a plain box and a Shift box add, the same box again changes nothing (no toggle), a Ctrl
        // box removes.
        let rect = (a - m, b + m);
        let boxed = |cx: &mut TestAppContext, modifiers: Modifiers| {
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::BoxSelect { ids: Vec::new(), modifiers, crossing: false, rect }, w, cx);
                })
            })
            .unwrap();
            draw(cx, window);
            cx.update(|cx| view.read(cx).selection.len())
        };
        view.update(cx, |v, _| v.selection.clear());
        let got = [
            boxed(cx, Modifiers::default()),
            boxed(cx, Modifiers::default()),
            boxed(cx, Modifiers { control: true, ..Default::default() }),
            boxed(cx, Modifiers { shift: true, ..Default::default() }),
        ];
        assert_eq!(got, [18, 18, 0, 18], "scale {scale}: selected after a box, the same box, a Ctrl box, a Shift box");
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

/// Live2 sel-feel (11-17 22:12): while a box is dragged across a part nothing lights under the pointer, its faces and
/// edges included; on release what the box takes is selected at once (live's window box round a block: 18).
#[gpui::test]
fn a_box_drag_lights_nothing_and_selects_at_release(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _top, _) = block_part(cx, "box-drag-hover");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (lo, hi) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let vp = v.viewport.read(cx);
        let s: Vec<DVec2> = b.mesh.positions.iter().filter_map(|q| vp.project(glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64))).collect();
        (s.iter().fold(DVec2::splat(f64::MAX), |m, p| m.min(*p)), s.iter().fold(DVec2::splat(f64::MIN), |m, p| m.max(*p)))
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    // The part lights at rest under the pointer first (the test can see a hover).
    pointer.move_to(cx, &view, (lo + hi) * 0.5);
    assert!(lit(cx).is_some(), "the part's middle lights at rest");
    // A start beside the part where nothing lights.
    let start = [DVec2::new(lo.x - 40.0, lo.y - 40.0), DVec2::new(hi.x + 40.0, lo.y - 40.0), DVec2::new(lo.x - 40.0, hi.y + 40.0), DVec2::new(hi.x + 40.0, hi.y + 40.0)]
        .into_iter()
        .find(|s| {
            pointer.move_to(cx, &view, *s);
            lit(cx).is_none()
        })
        .expect("a spot beside the part where nothing lights");
    let end = DVec2::new(if start.x < lo.x { hi.x + 40.0 } else { lo.x - 40.0 }, if start.y < lo.y { hi.y + 40.0 } else { lo.y - 40.0 });
    let at = pointer.window_point(cx, start);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
    // A person's press lasts past the 50 ms a left press needs to become a drag (the viewport's clock: the wall's): the
    // first move is the drag's (it moved straight on, so the drag began at the second move, which may light by the
    // first-move rule: main red 18:4x).
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut wrong = Vec::new();
    for i in 1..=12 {
        let p = start + (end - start) * (i as f64 / 12.0);
        let win = pointer.window_point(cx, p);
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, Some(gpui_kit::MouseButton::Left), Modifiers::default());
        // (A person's slow drag: frames and picks come back between moves.)
        pointer.frames(cx, &view);
        // (The first move may light what it ends over, the box waiting a move: live's first-move rule, the test above.)
        let got = lit(cx);
        if got.is_some() && i > 1 {
            wrong.push(format!("box drag step {i} at {p:?}: lit {got:?}"));
        }
    }
    let win = pointer.window_point(cx, end);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: win, modifiers: Modifiers::default(), click_count: 1 });
    // At the release itself, no GPU readback first (live recheck 11-18: ours showed it 4–5 frames after the box went).
    let n = cx.update(|cx| view.read(cx).selection.len());
    if n != 18 {
        wrong.push(format!("released: {n} selected at once, want live's 18 (6 faces, 12 edges)"));
    }
    // The box stays until the frame with the selection is up, then goes (live recheck 5c922906: ours showed frames
    // with neither).
    let shown = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).box_shown());
    if !shown(cx) {
        wrong.push("released: the box went before a frame with the selection".into());
    }
    let t = std::time::Instant::now();
    while shown(cx) && t.elapsed() < std::time::Duration::from_secs(15) {
        pointer.frames(cx, &view);
    }
    if shown(cx) {
        wrong.push("the box stayed after frames with the selection".into());
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live2 hover-p0 (11-17 12:35): a plane lights over its whole inside and up to 6 px out past its edge (8 px out,
/// nothing: ours reaches further across a slanted edge, printed). Checked on an outer edge of each default plane in
/// the iso view, where no other plane is near, coming in from outside.
#[gpui::test]
fn a_plane_lights_6_px_past_its_edge(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = part_studio(cx, "plane-edge-reach");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let quads: Vec<(Reference, [DVec2; 4])> = cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        v.display.planes.iter().filter(|p| !p.hidden).filter_map(|p| Some((p.reference.clone(), [vp.project(p.corner(-1., -1.))?, vp.project(p.corner(1., -1.))?, vp.project(p.corner(1., 1.))?, vp.project(p.corner(-1., 1.))?]))).collect()
    });
    assert_eq!(quads.len(), 3, "the three default planes shown");
    let inside = |q: &[DVec2; 4], p: DVec2| {
        let s: Vec<f64> = (0..4).map(|k| (q[(k + 1) % 4] - q[k]).perp_dot(p - q[k])).collect();
        s.iter().all(|x| *x >= 0.0) || s.iter().all(|x| *x <= 0.0)
    };
    let seg_dist = |a: DVec2, b: DVec2, p: DVec2| {
        let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
        (a + (b - a) * t - p).length()
    };
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    for (plane, q) in &quads {
        let centre = q.iter().sum::<DVec2>() / 4.0;
        // A spot on an outer edge whose 20 px outside touches no other plane and is in the viewport.
        let size = cx.update(|cx| pointer.vp.read(cx).size());
        let spot = (0..4).flat_map(|k| [0.3, 0.5, 0.7].map(|t| (k, t))).find_map(|(k, t)| {
            let (a, b) = (q[k], q[(k + 1) % 4]);
            let on = a + (b - a) * t;
            let mut n = (b - a).perp().normalize();
            if n.dot(on - centre) < 0.0 {
                n = -n;
            }
            let far = on + n * 20.0;
            let clear = quads.iter().all(|(r, o)| {
                let edges = (0..4).map(|j| seg_dist(o[j], o[(j + 1) % 4], far)).fold(f64::MAX, f64::min);
                (r == plane || (!inside(o, on + n * 4.0) && !inside(o, on - n * 20.0))) && !inside(o, far) && edges > 15.0
            });
            (clear && far.x > 20.0 && far.y > 20.0 && far.x < size.x - 20.0 && far.y < size.y - 20.0).then_some((on, n))
        });
        let Some((on, n)) = spot else {
            wrong.push(format!("{plane:?}: no outer edge clear of the other planes"));
            continue;
        };
        // Live reach-px-1119: lit at 6 px off a level edge (7: not), at 7.2 off iso's slanted one (8.05: not).
        for (d, want) in [(20.0, false), (9.0, false), (6.0, true), (-20.0, true)] {
            pointer.move_to(cx, &view, on + n * d);
            let lit = cx.update(|cx| view.read(cx).hover.clone());
            if (lit.as_ref() == Some(plane)) != want {
                wrong.push(format!("{plane:?}: {d} px out lit {lit:?}, want {}", if want { "the plane" } else { "nothing" }));
            }
        }
        let reach = (6..=14).rev().find(|d| {
            pointer.move_to(cx, &view, on + n * 30.0);
            pointer.move_to(cx, &view, on + n * f64::from(*d));
            cx.update(|cx| view.read(cx).hover.as_ref() == Some(plane))
        });
        eprintln!("plane reach {plane:?}: lit to {reach:?} px out across an edge at {:.0}°", n.y.atan2(n.x).to_degrees());
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live2 sel-feel (11-17, box-e/ltr-from-plane; qa FEEL-PLANE-FIRST-MOVE): a press on empty space whose first move ends
/// over a default plane lights the plane and draws no box, nor does a 1 px nudge; the next move draws it.
#[gpui::test]
fn a_box_whose_first_move_ends_on_a_plane_waits_a_move(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = part_studio(cx, "box-first-move-plane");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // Spots near each shown plane's corners (where no other plane is over them: the one lit at rest).
    let spots: Vec<(Reference, DVec2)> = cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        v.display.planes.iter().filter(|p| !p.hidden).flat_map(|p| [(-0.8, -0.8), (0.8, -0.8), (0.8, 0.8), (-0.8, 0.8)].map(|(u, v)| (p.reference.clone(), p.corner(u, v)))).filter_map(|(r, w)| Some((r, vp.project(w)?))).collect()
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let (plane, centre) = spots
        .into_iter()
        .find(|(plane, at)| {
            pointer.move_to(cx, &view, *at);
            lit(cx).as_ref() == Some(plane)
        })
        .expect("a plane's corner that lights it at rest");
    let empty = DVec2::new(15.0, 15.0);
    pointer.move_to(cx, &view, empty);
    assert!(lit(cx).is_none(), "nothing lights at the viewport's corner");
    let press = pointer.window_point(cx, empty);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: press, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
    // (Past the press's hold time, as a person's drag.)
    std::thread::sleep(std::time::Duration::from_millis(250));
    let drag_to = |cx: &mut TestAppContext, at: DVec2| {
        let win = pointer.window_point(cx, at);
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, Some(gpui_kit::MouseButton::Left), Modifiers::default());
        pointer.frames(cx, &view);
        (lit(cx), cx.update(|cx| pointer.vp.read(cx).box_drag()))
    };
    let first = {
        let at = drag_to(cx, centre);
        // (The first move's pick comes back from the GPU's thread: wait for what it decides, a light or a box.)
        let t = std::time::Instant::now();
        let mut at = at;
        while at == (None, false) && t.elapsed() < std::time::Duration::from_secs(15) {
            pointer.frames(cx, &view);
            at = (lit(cx), cx.update(|cx| pointer.vp.read(cx).box_drag()));
        }
        at
    };
    let nudged = drag_to(cx, centre + DVec2::new(1.0, 0.0));
    let next = drag_to(cx, centre + DVec2::new(20.0, 10.0));
    let up = pointer.window_point(cx, centre + DVec2::new(20.0, 10.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: up, modifiers: Modifiers::default(), click_count: 1 });
    assert_eq!(
        (first, nudged.1, next.1),
        ((Some(plane), false), false, true),
        "((lit, box) at the first move onto the plane, box after a 1 px nudge, box after the next move)"
    );
}

/// Live box-hover-1122 (qa FEEL-PS-BOX-DRAG-HOVER): a press on a lit plane starts a box; that plane stays lit through the
/// whole drag, nothing else lights (the other planes crossed included), and it goes out at the release, unselected.
#[gpui::test]
fn a_box_from_a_plane_keeps_that_plane_lit_to_the_release(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = part_studio(cx, "box-from-plane");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let spots: Vec<(Reference, DVec2)> = cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        v.display.planes.iter().filter(|p| !p.hidden).flat_map(|p| [(-0.8, -0.8), (0.8, -0.8), (0.8, 0.8), (-0.8, 0.8)].map(|(u, v)| (p.reference.clone(), p.corner(u, v)))).filter_map(|(r, w)| Some((r, vp.project(w)?))).collect()
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let (plane, start) = spots
        .into_iter()
        .find(|(plane, at)| {
            pointer.move_to(cx, &view, *at);
            lit(cx).as_ref() == Some(plane)
        })
        .expect("a plane's corner that lights it at rest");
    // Across the view's middle, over the other planes.
    let size = cx.update(|cx| pointer.vp.read(cx).size());
    let end = DVec2::new(size.x - start.x, size.y - start.y);
    let press = pointer.window_point(cx, start);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: press, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
    std::thread::sleep(std::time::Duration::from_millis(250));
    let mut wrong = Vec::new();
    let mut boxed = false;
    for i in 1..=12 {
        let p = start + (end - start) * (i as f64 / 12.0);
        let win = pointer.window_point(cx, p);
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, Some(gpui_kit::MouseButton::Left), Modifiers::default());
        pointer.frames(cx, &view);
        boxed |= cx.update(|cx| pointer.vp.read(cx).box_drag());
        let got = lit(cx);
        if got.as_ref() != Some(&plane) {
            wrong.push(format!("box drag step {i} at {p:?}: lit {got:?}, live the pressed plane {plane:?}"));
        }
    }
    if !boxed {
        wrong.push("a press on a plane drew no box".into());
    }
    let up = pointer.window_point(cx, end);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: up, modifiers: Modifiers::default(), click_count: 1 });
    pointer.frames(cx, &view);
    if let Some(got) = lit(cx) {
        wrong.push(format!("released: {got:?} still lit"));
    }
    if cx.update(|cx| view.read(cx).selection.contains(&plane)) {
        wrong.push("released: the plane selected".into());
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live hover-pick-1120 (11-17 20:41): a mate connector on a face wins over the face across its drawn disc and triad:
/// lit 22 px out to either side, 13 px up or down, in the iso view.
#[gpui::test]
fn a_mate_connector_wins_over_its_face_across_its_disc(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, top, _) = block_part(cx, "connector-reach");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.selection = vec![top.clone()];
        v.start_feature("mateConnector", w, cx);
    }))
    .unwrap();
    draw(cx, window);
    wait_until(cx, window, |cx| view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).can_submit()));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).edit.is_none() && !view.read(cx).display.connectors.is_empty());
    draw(cx, window);
    let o = cx.update(|cx| {
        let v = view.read(cx);
        v.viewport.read(cx).project(v.display.connectors[0].origin).unwrap()
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext, at: DVec2| {
        pointer.move_to(cx, &view, o + DVec2::new(0.0, 60.0));
        pointer.move_to(cx, &view, at);
        cx.update(|cx| view.read(cx).hover.clone())
    };
    let mut wrong = Vec::new();
    for (d, want) in [(DVec2::new(22.0, 0.0), true), (DVec2::new(-22.0, 0.0), true), (DVec2::new(0.0, 13.0), true), (DVec2::new(0.0, -13.0), true), (DVec2::new(27.0, 0.0), false), (DVec2::new(0.0, 18.0), false)] {
        let got = lit(cx, o + d);
        if matches!(got, Some(Reference::MateConnector { .. })) != want {
            wrong.push(format!("{d:?} from the connector: lit {got:?}, want {}", if want { "the connector" } else { "the face" }));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live shortcuts-ps-1118: after a view key (Shift+1 Front) whatever comes under the resting pointer lights once the
/// view has moved; the pointer doesn't need to move.
#[gpui::test]
fn a_view_key_lights_what_comes_under_the_resting_pointer(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _, _) = block_part(cx, "view-key-hover");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    // The part's middle on screen, in the iso view and (fit again) in Front.
    let mid = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let (lo, hi) = b.bounds.unwrap();
        v.viewport.read(cx).project((lo + hi) * 0.5).unwrap()
    });
    pointer.move_to(cx, &view, mid);
    assert!(cx.update(|cx| view.read(cx).hover.is_some()), "the part lights under the pointer before the key");
    // A key (GPUI drops the hover at any key by default), then the view change it asks for.
    keys(cx, window, &["shift"]);
    cx.update(|cx| view.update(cx, |v, cx| v.view(noshape_render::StandardView::Front, cx)));
    // The view moves (its animation), then what is under the pointer is picked again.
    let t = std::time::Instant::now();
    let mut lit_after = None;
    while t.elapsed() < std::time::Duration::from_secs(15) {
        pointer.frames(cx, &view);
        let (moving, settled, lit) = cx.update(|cx| (pointer.vp.read(cx).animating(), pointer.vp.read(cx).hover_settled(), view.read(cx).hover.clone()));
        if !moving && settled && lit.is_some() {
            lit_after = lit;
            break;
        }
    }
    assert!(lit_after.is_some(), "nothing lit under the resting pointer after the view key");
}

/// Live workflows 16 and 17 (qa FEEL-HOVER-AFTER-MODAL): a modal closed over the graphics leaves nothing lit under the
/// still pointer; the first move lights what is under it.
#[gpui::test]
fn a_modal_closed_over_the_graphics_lights_nothing_until_the_pointer_moves(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _, _) = block_part(cx, "hover-after-modal");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let mid = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let (lo, hi) = b.bounds.unwrap();
        v.viewport.read(cx).project((lo + hi) * 0.5).unwrap()
    });
    pointer.move_to(cx, &view, mid);
    assert!(cx.update(|cx| view.read(cx).hover.is_some()), "the part lights under the pointer before the modal");
    let run: crate::part_studio::export::RunExport = std::rc::Rc::new(|_, _, _, _| {});
    cx.update_window(window, |_, w, cx| crate::part_studio::export::open_export_modal("Part Studio 1".into(), false, run, w, cx)).unwrap();
    pointer.frames(cx, &view);
    assert!(cx.update(|cx| view.read(cx).hover.is_none()), "the modal covers the graphics");
    keys(cx, window, &["escape"]);
    let shown = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| gpui_kit::component::WindowExt::has_active_dialog(w, cx)).unwrap();
    assert!(!shown(cx), "Esc closes Export");
    for _ in 0..20 {
        pointer.frames(cx, &view);
    }
    assert!(cx.update(|cx| view.read(cx).hover.is_none()), "nothing lit under the still pointer after the modal closed");
    pointer.move_to(cx, &view, mid + DVec2::new(1.0, 0.0));
    assert!(cx.update(|cx| view.read(cx).hover.is_some()), "the first move lights the part");
}

/// Live workflow 29 step 7 (qa FEEL-PLANE-LIT-AFTER-MENU): the pointer went from the feature list onto a cover over the
/// graphics (Top row's menu) without crossing them; the cover closed by a click or key leaves nothing lit under it.
#[gpui::test]
fn a_cover_reached_from_the_list_lights_nothing_after_it_closes(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _, _) = block_part(cx, "hover-after-list-cover");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let mid = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let (lo, hi) = b.bounds.unwrap();
        v.viewport.read(cx).project((lo + hi) * 0.5).unwrap()
    });
    pointer.move_to(cx, &view, mid);
    assert!(cx.update(|cx| view.read(cx).hover.is_some()), "the part lights under the pointer");
    let list = pointer.window_point(cx, DVec2::new(-60.0, mid.y));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(list, None, Modifiers::default());
    pointer.frames(cx, &view);
    assert!(cx.update(|cx| view.read(cx).hover.is_none()), "nothing lit with the pointer over the list");
    let run: crate::part_studio::export::RunExport = std::rc::Rc::new(|_, _, _, _| {});
    cx.update_window(window, |_, w, cx| crate::part_studio::export::open_export_modal("Part Studio 1".into(), false, run, w, cx)).unwrap();
    pointer.frames(cx, &view);
    let over = pointer.window_point(cx, mid);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(over, None, Modifiers::default());
    pointer.frames(cx, &view);
    keys(cx, window, &["escape"]);
    for _ in 0..20 {
        pointer.frames(cx, &view);
    }
    assert!(cx.update(|cx| view.read(cx).hover.is_none()), "nothing lit under the still pointer after the cover closed");
    pointer.move_to(cx, &view, mid + DVec2::new(1.0, 0.0));
    assert!(cx.update(|cx| view.read(cx).hover.is_some()), "the first move lights the part");
}

/// Live2 submenu-diag-1119 (qa face-rclick-select-fillet-faces): a click beside an open context menu only closes it:
/// the plane under it isn't selected (live: selection 0); the next click there selects the plane.
#[gpui::test]
fn a_click_beside_an_open_context_menu_only_closes_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _top, _) = block_part(cx, "menu-click-closes");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let face_at = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let vp = v.viewport.read(cx);
        let top_z = b.mesh.positions.iter().map(|q| q[2]).fold(f32::MIN, f32::max);
        let tops: Vec<DVec2> = b.mesh.positions.iter().filter(|q| (q[2] - top_z).abs() < 1e-7).filter_map(|q| vp.project(glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64))).collect();
        tops.iter().sum::<DVec2>() / tops.len() as f64
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let plane_spot = [DVec2::new(face_at.x - 220.0, face_at.y + 120.0), DVec2::new(face_at.x + 220.0, face_at.y + 120.0), DVec2::new(face_at.x, face_at.y + 200.0)]
        .into_iter()
        .find(|s| {
            pointer.move_to(cx, &view, *s);
            matches!(lit(cx), Some(Reference::Plane(_)))
        })
        .expect("a plane beside the part");
    pointer.move_to(cx, &view, face_at);
    let at = pointer.window_point(cx, face_at);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(at, gpui_kit::MouseButton::Right, Modifiers::default());
    vcx.simulate_mouse_up(at, gpui_kit::MouseButton::Right, Modifiers::default());
    pointer.settle(cx, &view);
    let opened = cx.update(|cx| view.read(cx).context_menu.is_some());
    pointer.click(cx, &view, plane_spot);
    let after = cx.update(|cx| (view.read(cx).context_menu.is_some(), view.read(cx).selection.clone()));
    pointer.click(cx, &view, plane_spot);
    let second = cx.update(|cx| view.read(cx).selection.clone());
    assert!(opened && after == (false, Vec::new()), "the face's menu opened ({opened}); after a click beside it (open, selected): {after:?}");
    assert!(matches!(second.as_slice(), [Reference::Plane(_)]), "the next click selects the plane: {second:?}");
}

/// qa HOVER-LEAVES-WINDOW (11-18): a face lit as the pointer leaves the window goes dark, also when its pick was still
/// on its way, and nothing lights again while the pointer is out.
#[gpui::test]
fn a_lit_face_goes_dark_when_the_pointer_leaves_the_window(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, top, _) = block_part(cx, "hover-leaves-window");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let centre = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let i = b.face_refs.iter().position(|r| *r == top).expect("the top face");
        let (first, count) = b.mesh.face_ranges[i];
        let idx = &b.mesh.indices[first as usize..(first + count) as usize];
        let c = idx.iter().map(|&k| { let q = b.mesh.positions[k as usize]; glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64) }).sum::<glam::DVec3>() / idx.len() as f64;
        v.viewport.read(cx).project(c).expect("the top face on screen")
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
    let leave = |cx: &mut TestAppContext| {
        let y = pointer.window_point(cx, centre).y;
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseExitEvent { position: gpui_kit::point(px(-2.), y), pressed_button: None, modifiers: Modifiers::default() });
        // 400 ms of frames: a pick still on its way comes back meanwhile.
        for _ in 0..4 {
            pointer.frames(cx, &view);
        }
    };
    pointer.move_to(cx, &view, centre);
    assert_eq!(lit(cx), Some(top.clone()), "the top face lit under the pointer");
    leave(cx);
    assert_eq!(lit(cx), None, "the pointer out of the window: nothing lit");
    // Onto the face and straight out: its pick still on its way as the pointer leaves.
    pointer.move_to(cx, &view, centre);
    let win = pointer.window_point(cx, centre + DVec2::new(12.0, 0.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
    leave(cx);
    assert_eq!(lit(cx), None, "out of the window with a pick on its way: nothing lit");
}

/// Live drift-1120: a fast move off a lit face clears it on that move (redrawn 5–7 ms after it), not after the pause
/// the next pick waits for.
#[gpui::test]
fn a_fast_move_off_a_lit_face_clears_it_on_that_move(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, top, _) = block_part(cx, "hover-fast-off");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let centre = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let i = b.face_refs.iter().position(|r| *r == top).expect("the top face");
        let (first, count) = b.mesh.face_ranges[i];
        let idx = &b.mesh.indices[first as usize..(first + count) as usize];
        let c = idx.iter().map(|&k| { let q = b.mesh.positions[k as usize]; glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64) }).sum::<glam::DVec3>() / idx.len() as f64;
        v.viewport.read(cx).project(c).expect("the top face on screen")
    });
    let pointer = Pointer::new(cx, window, &view);
    pointer.move_to(cx, &view, centre);
    assert_eq!(cx.update(|cx| view.read(cx).hover.clone()), Some(top.clone()), "the top face lit under the pointer");
    // A 1 px move on the face, then at once 200 px up off the part: one fast move.
    let move_to = |cx: &mut TestAppContext, at: DVec2| {
        let win = pointer.window_point(cx, at);
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
    };
    move_to(cx, centre + DVec2::new(1.0, 0.0));
    move_to(cx, DVec2::new(centre.x, (centre.y - 200.0).max(4.0)));
    let after = cx.update(|cx| (view.read(cx).hover.clone(), view.read(cx).viewport.read(cx).hover_settled()));
    assert_eq!(after.0, None, "the face dark on the move off, before any pause or frame (settled: {})", after.1);
}

/// A fast move's hover waits 5 × the average pick (live's `averagePickDuration`): the pick's own time, not a pick's
/// wait behind a frame the render thread draws first (a heavy model's software frame held the wait at its 250 ms cap).
#[gpui::test]
fn a_pick_held_up_behind_a_frame_leaves_the_hover_wait(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _, _) = block_part(cx, "hover-wait-own-pick");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let size = cx.update(|cx| view.read(cx).viewport.read(cx).size());
    let at = DVec2::new(size.x * 0.2, size.y * 0.2);
    pointer.move_to(cx, &view, at);
    let avg = |cx: &mut TestAppContext| cx.update(|cx| pointer.vp.read(cx).average_pick_ms());
    // Frames and picks delivered as they finish, none asked: the next frame is drawn at once, a pick asked alone.
    let drain = |cx: &mut TestAppContext| {
        let t = std::time::Instant::now();
        while cx.update(|cx| pointer.vp.read(cx).frame_in_flight() || pointer.vp.read(cx).pick_pending()) && t.elapsed() < std::time::Duration::from_secs(15) {
            cx.update(|cx| view.read(cx).gpu.deliver_frames());
            draw(cx, window);
        }
    };
    drain(cx);
    // A slow move away (past the pixels picked round the pointer): its pick, asked with no frame in flight, is timed.
    cx.executor().advance_clock(std::time::Duration::from_secs(5));
    let win = pointer.window_point(cx, DVec2::new(size.x * 0.6, size.y * 0.6));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
    drain(cx);
    assert!(avg(cx) >= 0.0, "a pick asked alone timed");
    cx.executor().advance_clock(std::time::Duration::from_secs(5));
    let win = pointer.window_point(cx, at);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
    drain(cx);
    let before = avg(cx);
    // A frame on its way (the camera turned; headless frames wait for `deliver_frames`), then a slow 3 px move (5 s after
    // the last) whose pick waits for that frame 10 s.
    cx.executor().advance_clock(std::time::Duration::from_secs(5));
    cx.update(|cx| {
        pointer.vp.update(cx, |v, cx| {
            v.camera.orbit(DVec2::new(20.0, 0.0), size.y);
            v.request_frame(cx);
        })
    });
    draw(cx, window);
    assert!(cx.update(|cx| pointer.vp.read(cx).frame_in_flight()), "the turned view's frame on its way");
    let win = pointer.window_point(cx, at + DVec2::new(3.0, 0.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
    assert!(cx.update(|cx| pointer.vp.read(cx).pick_pending()), "the move's pick asked behind the frame");
    cx.executor().advance_clock(std::time::Duration::from_secs(10));
    pointer.frames(cx, &view);
    let t = std::time::Instant::now();
    while cx.update(|cx| pointer.vp.read(cx).pick_pending()) && t.elapsed() < std::time::Duration::from_secs(15) {
        pointer.frames(cx, &view);
    }
    // One 10 s pick in the average would put it past 2 s (0.8 old + 0.2 new).
    assert!(avg(cx) < before + 1000.0, "the 10 s behind the frame isn't the pick's: average {before} -> {} ms", avg(cx));
}

/// A hover after the window rested (no frame drawn for seconds) sends its frame to the render thread when its pick
/// comes back, not through a redraw of the whole window first (on a software adapter that redraw took the CPU the
/// frame needed: 100-180 ms of each heavy-document hover, trace diag1 11-20).
#[gpui::test]
fn a_hover_after_a_rest_submits_its_frame_without_a_window_redraw(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, _, _) = block_part(cx, "hover-after-rest");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let size = cx.update(|cx| view.read(cx).viewport.read(cx).size());
    let part = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let c = b.mesh.positions.iter().fold(glam::DVec3::ZERO, |c, p| c + glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)) / b.mesh.positions.len() as f64;
        v.viewport.read(cx).project(c).unwrap()
    });
    pointer.move_to(cx, &view, DVec2::new(size.x * 0.05, size.y * 0.95));
    let busy = |cx: &mut TestAppContext| cx.update(|cx| pointer.vp.read(cx).frame_in_flight() || pointer.vp.read(cx).pick_pending());
    let t = std::time::Instant::now();
    while busy(cx) && t.elapsed() < std::time::Duration::from_secs(15) {
        cx.update(|cx| view.read(cx).gpu.deliver_frames());
        draw(cx, window);
    }
    let lit_before = cx.update(|cx| view.read(cx).hover.clone());
    // Two seconds with no window frame, on the viewport's clock and the executor's (a slow move: picked at once).
    let clock = std::rc::Rc::new(std::cell::Cell::new(cx.update(|cx| pointer.vp.read(cx).now()) + std::time::Duration::from_secs(2)));
    let c = clock.clone();
    cx.update(|cx| pointer.vp.update(cx, |vp, _| vp.set_clock(move || c.get())));
    cx.executor().advance_clock(std::time::Duration::from_secs(2));
    let laid_out = cx.update(|cx| pointer.vp.read(cx).layout_count());
    let win = pointer.window_point(cx, part);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
    // Picks answered with no window drawn meanwhile; frames stay held (one presented would redraw the window, a slow
    // runner's earlier frame finishing then).
    let t = std::time::Instant::now();
    while cx.update(|cx| pointer.vp.read(cx).pick_pending()) && t.elapsed() < std::time::Duration::from_secs(15) {
        cx.update(|cx| view.read(cx).gpu.deliver_picks());
        cx.run_until_parked();
    }
    let lit = cx.update(|cx| view.read(cx).hover.clone());
    assert!(lit.is_some() && lit != lit_before, "the part lit under the pointer: {lit_before:?} -> {lit:?}");
    assert!(cx.update(|cx| pointer.vp.read(cx).frame_in_flight()), "the lit part's frame on its way");
    assert_eq!(cx.update(|cx| pointer.vp.read(cx).layout_count()), laid_out, "no window redraw before the frame");
}

/// Live 3d.json (interaction-feel "Click", "Ctrl+click") and sel-gestures-1122: in 3D with no tool a click adds what is
/// lit, a click on a selected face removes it, a click on empty space clears, Shift+click there keeps the selection,
/// Ctrl+click opens the menu of what is under the pointer without selecting it, Shift+click on a face toggles it,
/// Escape keeps the selection and Space clears it, a press held 0.5 s or more selects nothing (longpress-1122); through
/// the real pointer.
#[gpui::test]
fn clicks_in_3d_add_remove_clear_and_ctrl_opens_the_menu(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view, top, _) = block_part(cx, "click-mods-3d");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let centres: Vec<(Reference, DVec2)> = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let vp = v.viewport.read(cx);
        b.face_refs
            .iter()
            .zip(&b.mesh.face_ranges)
            .filter_map(|(r, &(first, count))| {
                let idx = &b.mesh.indices[first as usize..(first + count) as usize];
                let c = idx.iter().map(|&k| { let q = b.mesh.positions[k as usize]; glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64) }).sum::<glam::DVec3>() / idx.len() as f64;
                Some((r.clone(), vp.project(c)?))
            })
            .collect()
    });
    let pointer = Pointer::new(cx, window, &view);
    let lit_at = |cx: &mut TestAppContext, at: DVec2| {
        pointer.move_to(cx, &view, at);
        cx.update(|cx| view.read(cx).hover.clone())
    };
    let on_top = centres.iter().find(|(r, _)| *r == top).expect("the top face on screen").1;
    let (side, on_side) = centres.iter().filter(|(r, _)| *r != top).find(|(r, at)| lit_at(cx, *at).as_ref() == Some(r)).cloned().expect("a side face lit at its centre");
    let empty = DVec2::new(15.0, 15.0);
    assert_eq!(lit_at(cx, empty), None, "nothing lights at the viewport's corner");
    let press = |cx: &mut TestAppContext, at: DVec2, modifiers: Modifiers| {
        pointer.move_to(cx, &view, at);
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(pointer.window_point(cx, at), modifiers);
        // A click the pick cache doesn't cover waits for the GPU's pick.
        pointer.settle(cx, &view);
        cx.update(|cx| view.read(cx).selection.clone())
    };
    let shift = Modifiers { shift: true, ..Default::default() };
    let ctrl = Modifiers { control: true, ..Default::default() };
    let mut wrong = Vec::new();
    let mut row = |what: &str, got: Vec<Reference>, want: &[&Reference]| {
        if got.len() != want.len() || !want.iter().all(|w| got.contains(w)) {
            wrong.push(format!("{what}: {got:?}, want {want:?}"));
        }
    };
    row("click the top", press(cx, on_top, Modifiers::default()), &[&top]);
    row("then click a side", press(cx, on_side, Modifiers::default()), &[&top, &side]);
    row("click the top again", press(cx, on_top, Modifiers::default()), &[&side]);
    row("click empty space", press(cx, empty, Modifiers::default()), &[]);
    row("click the top", press(cx, on_top, Modifiers::default()), &[&top]);
    row("Shift+click empty space", press(cx, empty, shift), &[&top]);
    row("Ctrl+click the side", press(cx, on_side, ctrl), &[&top]);
    assert!(cx.update(|cx| view.read(cx).context_menu.is_some()), "Ctrl+click opened the context menu");
    // Live sel-gestures-1122: Escape closes the menu and keeps the selection, also with two; Shift+click toggles; Space
    // clears.
    keys(cx, window, &["escape"]);
    pointer.settle(cx, &view);
    let selected = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).selection.clone());
    row("Escape over the menu", selected(cx), &[&top]);
    row("click the side", press(cx, on_side, Modifiers::default()), &[&top, &side]);
    keys(cx, window, &["escape"]);
    row("Escape with two selected", selected(cx), &[&top, &side]);
    row("Shift+click the top", press(cx, on_top, shift), &[&side]);
    row("Shift+click the top again", press(cx, on_top, shift), &[&top, &side]);
    keys(cx, window, &["space"]);
    row("Space", selected(cx), &[]);
    // Live longpress-1122: a release selects only when held under ~0.5 s (≤504 ms selects, ≥533 ms doesn't).
    let clock = std::rc::Rc::new(std::cell::Cell::new(cx.update(|cx| pointer.vp.read(cx).now())));
    let c = clock.clone();
    cx.update(|cx| pointer.vp.update(cx, |vp, _| vp.set_clock(move || c.get())));
    let hold = |cx: &mut TestAppContext, at: DVec2, ms: u64| {
        pointer.move_to(cx, &view, at);
        let win = pointer.window_point(cx, at);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_down(win, gpui_kit::MouseButton::Left, Modifiers::default());
        clock.set(clock.get() + std::time::Duration::from_millis(ms));
        vcx.simulate_mouse_up(win, gpui_kit::MouseButton::Left, Modifiers::default());
        pointer.settle(cx, &view);
        cx.update(|cx| view.read(cx).selection.clone())
    };
    row("hold the top 460 ms", hold(cx, on_top, 460), &[&top]);
    row("hold the side 600 ms", hold(cx, on_side, 600), &[&top]);
    row("hold the top 3 s", hold(cx, on_top, 3000), &[&top]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
