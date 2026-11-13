//! Interaction feel through the real pointer and the GPU's own pick (docs/parity/interaction-feel.md): what lights
//! and what a click takes, at a device pixel ratio of 1 and 2. They run on every build under the GPU lock, and skip
//! (saying so) where there is no adapter or its pick doesn't come back.

use super::*;

/// The GPU lock, once a GPU's pick is known to come back: a point drawn at the frame's centre picks there. None (and
/// why) where it doesn't: CI's Linux runner has no adapter, macOS CI's readback came out white (run 42333748082).
fn gpu_pointer() -> Option<parking_lot::MutexGuard<'static, ()>> {
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
struct Pointer {
    window: AnyWindowHandle,
    vp: Entity<crate::viewport::Viewport>,
}

impl Pointer {
    fn new(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>) -> Self {
        let vp = cx.update(|cx| view.read(cx).viewport.clone());
        Self { window, vp }
    }

    fn window_point(&self, cx: &mut TestAppContext, at: DVec2) -> Point<gpui_kit::Pixels> {
        let origin = cx.update(|cx| self.vp.read(cx).origin());
        gpui_kit::point(origin.x + px(at.x as f32), origin.y + px(at.y as f32))
    }

    /// Moves there and waits for the GPU's hover pick of it to show (frames delivered meanwhile).
    fn move_to(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>, at: DVec2) {
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
        // A few frames more: the scene shows the new hover, so the next pick holds a lit line (a person's pointer
        // takes many frames to get anywhere).
        self.frames(cx, view);
    }

    fn frames(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>) {
        for _ in 0..5 {
            cx.update(|cx| self.vp.update(cx, |v, cx| v.request_frame(cx)));
            cx.update(|cx| view.read(cx).gpu.deliver_frames());
            draw(cx, self.window);
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Clicks there and lets the click's pick come back (a hover just changed is picked again first).
    fn click(&self, cx: &mut TestAppContext, view: &Entity<PartStudioView>, at: DVec2) {
        let win = self.window_point(cx, at);
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_click(win, Modifiers::default());
        self.frames(cx, view);
    }
}

/// The test window at a device pixel ratio, redrawn.
fn at_scale(cx: &mut TestAppContext, window: AnyWindowHandle, scale: f32) {
    cx.update_window(window, |_, w, _| w.set_scale_factor(scale)).unwrap();
    draw(cx, window);
}

/// Outside sketch mode a click acts on exactly what is lit (lead 11-12, after the sketch reach fix), and an edge lights
/// as live's (live/feel/3d.json "hover_reach_px", a level edge: lights at 6–7 px, stays lit to 9–10) on either side,
/// over the background and over its own face. The pointer moves across a part edge a quarter along it, toward it and
/// back out on each side, and the click selects what lights there.
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
            [(2.0 * hold, false), (reach + 1.5, false), (reach - 1.0, true), (hold - 1.5, true), (hold + 1.5, false)].map(|(d, lit)| (side * d, lit))
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
/// GPU's own hover pick, what lights is what the click takes, within live's sketch reach (a curve 6 px): on the line's
/// drawn centre and 4 px beside it the line is lit and a click starts its dimension; 8 px beside it (live: 5 mm off a
/// line at sketch-offset-key's zoom is a miss) nothing is lit or taken.
#[gpui::test]
fn the_dimension_tool_takes_the_hovered_line(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    for scale in [1.0, 2.0] {
        for (off, lit) in [(0.0, true), (4.0, true), (8.0, false)] {
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
/// a click, moved 3 px later a box.
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
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let right = |vcx: &mut gpui_kit::VisualTestContext, dx: f64| {
            let (from, to) = (pointer.window_point(vcx, at), pointer.window_point(vcx, at + DVec2::new(dx, 0.0)));
            vcx.simulate_mouse_down(from, gpui_kit::MouseButton::Right, Modifiers::default());
            vcx.simulate_mouse_move(to, gpui_kit::MouseButton::Right, Modifiers::default());
            vcx.simulate_mouse_up(to, gpui_kit::MouseButton::Right, Modifiers::default());
        };
        for (dx, menu) in [(3.0, true), (4.0, true), (6.0, false)] {
            cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.close_context_menu(cx))).unwrap();
            pointer.frames(cx, &view);
            let before = cx.update(|cx| pointer.vp.read(cx).camera);
            right(&mut vcx, dx);
            pointer.frames(cx, &view);
            assert_ne!(cx.update(|cx| pointer.vp.read(cx).camera), before, "scale {scale}: a {dx} px right drag orbits");
            assert_eq!(cx.update(|cx| view.read(cx).context_menu.is_some()), menu, "scale {scale}: the menu after a {dx} px right drag");
        }
        cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.close_context_menu(cx))).unwrap();
        pointer.frames(cx, &view);
        // Left: 6 px at once is still a click (it selects what is under the pointer); 3 px after 60 ms is a box.
        let left = |vcx: &mut gpui_kit::VisualTestContext, dx: f64, wait: u64| {
            let (from, to) = (pointer.window_point(vcx, at), pointer.window_point(vcx, at + DVec2::new(dx, 0.0)));
            vcx.simulate_mouse_down(from, gpui_kit::MouseButton::Left, Modifiers::default());
            std::thread::sleep(std::time::Duration::from_millis(wait));
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
