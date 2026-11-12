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
    }

    fn click(&self, cx: &mut TestAppContext, at: DVec2) {
        let win = self.window_point(cx, at);
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_click(win, Modifiers::default());
        draw(cx, self.window);
    }
}

/// The test window at a device pixel ratio, redrawn.
fn at_scale(cx: &mut TestAppContext, window: AnyWindowHandle, scale: f32) {
    cx.update_window(window, |_, w, _| w.set_scale_factor(scale)).unwrap();
    draw(cx, window);
}

/// Outside sketch mode a click acts on exactly what is lit (lead 11-12, after the sketch reach fix): the pointer 5 to
/// 12 px either side of a part edge (a quarter along it), the click selects what lights there. The edge lights from
/// 5 px off it on both sides, not from 12; today it reaches 9 px over the background but only 5 over its own face
/// (interaction-feel.md "Edge hover reach"; live's is being measured).
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
        for off in [-12.0, -9.0, -5.0, 5.0, 6.0, 12.0f64] {
            cx.update_window(window, |_, _, cx| view.update(cx, |v, _| v.selection.clear())).unwrap();
            let at = on + across * off;
            pointer.move_to(cx, &view, at);
            let lit = cx.update(|cx| view.read(cx).hover.clone());
            pointer.click(cx, at);
            let selected = cx.update(|cx| view.read(cx).selection.clone());
            assert_eq!(selected, lit.clone().into_iter().collect::<Vec<_>>(), "scale {scale}, {off} px off the edge: the click selects what was lit");
            if off.abs() <= 5.0 || off.abs() >= 12.0 {
                assert_eq!(lit.as_ref() == Some(&edge), off.abs() <= 5.0, "scale {scale}, {off} px off the edge: lit {lit:?}");
            }
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
