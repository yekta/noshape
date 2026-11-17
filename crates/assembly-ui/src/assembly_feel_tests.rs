//! Assembly interaction feel through the real pointer and the GPU's own pick (docs/parity/interaction-feel.md, live2
//! asm-feel 11-17): what lights and what a click or a box takes. They run under the GPU lock, and skip (saying so)
//! where there is no adapter or its pick doesn't come back.

use super::{AssemblyView, OverlayPick};
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use glam::DVec2;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
}

/// The GPU lock, once a GPU's pick is known to come back (psui's part-studio feel tests' check).
fn gpu_pointer() -> Option<parking_lot::MutexGuard<'static, ()>> {
    let lock = noshape_ops::doc::gpu_lock();
    if let Err(e) = Gpu::headless_rendering() {
        eprintln!("asm feel: GPU pointer tests skipped: {e}");
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
        eprintln!("asm feel: GPU pointer tests skipped: this GPU's pick doesn't come back");
        return None;
    }
    Some(lock)
}

/// A 40 × 20 × 10 block (Part 1 <1>) and a 60 × 20 × 10 one (Part 2 <1>) side by side, Part 2 fastened by its top
/// face's centre to Part 1's (so its top face carries Fastened 1's glyph), in a test window with a real renderer, seen
/// from the top.
fn mated_blocks(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<AssemblyView>) {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-feel-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("asm.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "corner_rectangle", "points": [[100, 0], [160, 20]] }] }));
        let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
        let top = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
        let (a, b) = (top(&mut ctx, "Part 1 <1>"), top(&mut ctx, "Part 2 <1>"));
        call(&mut ctx, "assembly_mate", json!({ "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 2 <1>", "entity": b }, "offset": { "x": "70 mm" } }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::Assembly(_))).unwrap().id;
    let gpu = Gpu::headless_rendering().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| AssemblyView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            v
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    wait_until(cx, window, &view, |v, _| v.built.is_some() && v.result.occurrences.len() == 2 && v.result.mates.first().is_some_and(|m| m.frame_b.is_some()));
    // Seen from the top, the two blocks filling the middle of the graphics.
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            // A new mate is hidden (live): Show mates on it.
            let id = v.result.mates[0].id;
            v.set_mates_hidden(vec![id], false, cx);
            let (lo, hi) = v.result.bounds().unwrap();
            v.viewport.update(cx, |vp, cx| {
                let s = vp.size();
                vp.camera.set_view(noshape_render::StandardView::Top);
                vp.camera.fit_sphere((lo + hi) * 0.5, (hi - lo).length() * 0.6, s.x / s.y);
                vp.request_frame(cx);
            });
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, &view, |v, _| !v.ui.overlay_mates.is_empty());
    (window, view)
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
}

fn frames(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>) {
    for _ in 0..5 {
        let vp = cx.update(|cx| view.read(cx).viewport.clone());
        cx.update(|cx| vp.update(cx, |v, cx| v.request_frame(cx)));
        cx.update(|cx| view.read(cx).gpu.deliver_frames());
        draw(cx, window);
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn wait_until(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>, done: impl Fn(&AssemblyView, &gpui_kit::App) -> bool) {
    let t = std::time::Instant::now();
    while !cx.update(|cx| done(view.read(cx), cx)) && t.elapsed() < std::time::Duration::from_secs(60) {
        frames(cx, window, view);
    }
}

/// Lets the pointer's picks come back and the scene show them.
fn settle(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>) {
    let t = std::time::Instant::now();
    frames(cx, window, view);
    while cx.update(|cx| {
        let vp = view.read(cx).viewport.read(cx);
        vp.pick_pending() || vp.hover_pending() || !vp.hover_settled()
    }) && t.elapsed() < std::time::Duration::from_secs(15)
    {
        frames(cx, window, view);
    }
    frames(cx, window, view);
}

/// A viewport-local point in window coordinates.
fn at_window(cx: &mut TestAppContext, view: &Entity<AssemblyView>, at: DVec2) -> Point<gpui_kit::Pixels> {
    let origin = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
    gpui_kit::point(origin.x + px(at.x as f32), origin.y + px(at.y as f32))
}

fn move_to(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>, at: DVec2) {
    let w = at_window(cx, view, at);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(w, None, Modifiers::default());
    settle(cx, window, view);
}

/// A press at `from`, a drag through to `to` in steps and the release there.
fn drag(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>, from: DVec2, to: DVec2) {
    move_to(cx, window, view, from);
    let w = at_window(cx, view, from);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_down(w, MouseButton::Left, Modifiers::default());
    for k in 1..=8 {
        let p = at_window(cx, view, from + (to - from) * (f64::from(k) / 8.0));
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(p, Some(MouseButton::Left), Modifiers::default());
        frames(cx, window, view);
    }
    let w = at_window(cx, view, to);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_up(w, MouseButton::Left, Modifiers::default());
    settle(cx, window, view);
}

/// An instance's box on screen.
fn screen_box(cx: &mut TestAppContext, view: &Entity<AssemblyView>, name: &str) -> (DVec2, DVec2) {
    cx.update(|cx| {
        let v = view.read(cx);
        let id = v.assembly(cx).and_then(|a| a.instances.iter().find(|i| i.name == name)).unwrap().id;
        let vp = v.viewport.read(cx);
        let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
        for o in v.result.occurrences.iter().filter(|o| o.top() == id) {
            for p in &o.mesh.positions {
                let s = vp.project(o.transform.transform_point3(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))).unwrap();
                (lo, hi) = (lo.min(s), hi.max(s));
            }
        }
        (lo, hi)
    })
}

/// live2 asm-feel2 (b), asm-feel (4): a mate glyph (~26 px across face-on) takes the hover from the face behind it up
/// to ~20 px from its centre (the glyph plus 6–7 px), the face past that; a click on it selects the mate (its
/// instances' rows related), not an instance.
#[gpui::test]
fn a_mate_glyph_wins_the_hover_within_its_reach_and_its_click_selects_the_mate(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = mated_blocks(cx, "glyph-reach");
    let centre = cx.update(|cx| {
        let v = view.read(cx);
        let f = v.result.mates[0].frame_b.unwrap();
        v.viewport.read(cx).project(f.origin).unwrap()
    });
    assert_eq!(cx.update(|cx| view.read(cx).ui.overlay_mates.len()), 1, "Fastened 1's glyph shown");
    for d in [0.0, 10.0, 19.0] {
        move_to(cx, window, &view, centre + DVec2::new(d, 0.0));
        let (ov, hover) = cx.update(|cx| (view.read(cx).hover_overlay, view.read(cx).hover.as_ref().map(|t| t.kind)));
        assert_eq!((ov, hover), (Some(OverlayPick::Mate(0)), None), "{d} px from the glyph's centre: the glyph");
    }
    move_to(cx, window, &view, centre + DVec2::new(24.0, 0.0));
    let (ov, hover) = cx.update(|cx| (view.read(cx).hover_overlay, view.read(cx).hover.as_ref().map(|t| t.kind)));
    assert_eq!((ov, hover), (None, Some(crate::scene_builder::PickKind::Face)), "24 px from the glyph's centre: the face");

    move_to(cx, window, &view, centre + DVec2::new(3.0, 0.0));
    let w = at_window(cx, &view, centre + DVec2::new(3.0, 0.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    settle(cx, window, &view);
    cx.update(|cx| {
        let v = view.read(cx);
        assert_eq!(v.selected_mate, Some(v.result.mates[0].id), "the glyph's click selects its mate");
        assert!(v.selected.is_empty(), "and no instance");
        assert_eq!(v.related_instances(cx).len(), 2, "both mated instances' rows related");
    });
}

/// live2 asm-feel (5): a box takes whole instances, never faces: left to right (a window) over half a part takes
/// nothing and around it the instance; right to left (crossing) over half a part takes the whole instance.
#[gpui::test]
fn a_box_takes_whole_instances_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = mated_blocks(cx, "box");
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    let (above, below) = (lo.y - 30.0, hi.y + 30.0);
    let mid = (lo.x + hi.x) * 0.5;
    let selected = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        let a = v.assembly(cx).unwrap();
        v.selected.iter().map(|id| a.instances.iter().find(|i| i.id == *id).unwrap().name.clone()).collect::<Vec<_>>()
    });
    let clear = |cx: &mut TestAppContext| cx.update(|cx| view.update(cx, |v, _| v.clear_selection()));

    drag(cx, window, &view, DVec2::new(lo.x - 30.0, above), DVec2::new(mid, below));
    assert!(selected(cx).is_empty(), "a window over half a part takes nothing");
    drag(cx, window, &view, DVec2::new(lo.x - 30.0, above), DVec2::new(hi.x + 20.0, below));
    assert_eq!(selected(cx), vec!["Part 1 <1>".to_string()], "a window around the part takes the instance");
    clear(cx);
    drag(cx, window, &view, DVec2::new(mid, below), DVec2::new(lo.x - 30.0, above));
    assert_eq!(selected(cx), vec!["Part 1 <1>".to_string()], "a crossing box over half a part takes the whole instance");
    let faces = cx.update(|cx| view.read(cx).hover.is_some() || view.read(cx).selected_mate.is_some());
    assert!(!faces, "no face or mate taken by a box");
}
