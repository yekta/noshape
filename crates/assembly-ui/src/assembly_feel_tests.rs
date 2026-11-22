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
pub(super) fn gpu_pointer() -> Option<parking_lot::MutexGuard<'static, ()>> {
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

/// A fresh document at a unique scratch path, built by `make` through the agent API.
fn document(cx: &mut TestAppContext, name: &str, make: impl FnOnce(&mut Ctx)) -> std::path::PathBuf {
    cx.update(gpui_kit::init);
    crate::assembly_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-asm-feel-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("asm.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "corner_rectangle", "points": [[100, 0], [160, 20]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    make(&mut ctx);
    path
}

/// Assembly 1 of the document at `path` in a test window with a real renderer, evaluated with `occurrences`
/// occurrences, seen from the top with its parts filling the middle of the graphics.
fn open_from_top(cx: &mut TestAppContext, path: &std::path::Path, occurrences: usize) -> (AnyWindowHandle, Entity<AssemblyView>) {
    open_from_top_when(cx, path, move |v| v.result.occurrences.len() == occurrences && v.result.mates.iter().all(|m| m.frame_b.is_some()))
}

/// [`open_from_top`] once the assembly is built and `ready` holds.
fn open_from_top_when(cx: &mut TestAppContext, path: &std::path::Path, ready: impl Fn(&AssemblyView) -> bool) -> (AnyWindowHandle, Entity<AssemblyView>) {
    open_from_top_on(cx, path, false, ready)
}

/// [`open_from_top_when`], with the front layer (the left dialogs and the mate toolbar) drawn over the view when `front`.
fn open_from_top_on(cx: &mut TestAppContext, path: &std::path::Path, front: bool, ready: impl Fn(&AssemblyView) -> bool) -> (AnyWindowHandle, Entity<AssemblyView>) {
    let session = crate::session::Session::open(path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| t.name == "Assembly 1").unwrap().id;
    let gpu = Gpu::headless_rendering().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| AssemblyView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            let front: gpui_kit::AnyView = if front { v.update(cx, |v, cx| v.front_host(cx).0) } else { cx.new(|_| gpui_kit::EmptyView).into() };
            cx.new(|_| super::ui_tests::FrontHost { view: v, front })
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    wait_until(cx, window, &view, |v, _| v.built.is_some() && ready(v));
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
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
    frames(cx, window, &view);
    (window, view)
}

/// A 40 × 20 × 10 block (Part 1 <1>) and a 60 × 20 × 10 one (Part 2 <1>) side by side, Part 2 fastened by its top
/// face's centre to Part 1's (so its top face carries Fastened 1's glyph, shown), seen from the top.
fn mated_blocks(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<AssemblyView>) {
    let path = document(cx, name, |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
        let top = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
        let (a, b) = (top(ctx, "Part 1 <1>"), top(ctx, "Part 2 <1>"));
        call(ctx, "assembly_mate", json!({ "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 2 <1>", "entity": b }, "offset": { "x": "70 mm" } }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    // A new mate is hidden (live): Show mates on it.
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let id = v.result.mates[0].id;
            v.set_mates_hidden(vec![id], false, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, &view, |v, _| !v.ui.overlay_mates.is_empty());
    (window, view)
}

pub(super) fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
}

pub(super) fn frames(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>) {
    for _ in 0..5 {
        let vp = cx.update(|cx| view.read(cx).viewport.clone());
        cx.update(|cx| vp.update(cx, |v, cx| v.request_frame(cx)));
        cx.update(|cx| view.read(cx).gpu.deliver_frames());
        draw(cx, window);
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub(super) fn wait_until(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>, done: impl Fn(&AssemblyView, &gpui_kit::App) -> bool) {
    let t = std::time::Instant::now();
    while !cx.update(|cx| done(view.read(cx), cx)) && t.elapsed() < std::time::Duration::from_secs(60) {
        frames(cx, window, view);
    }
}

/// Lets the pointer's picks come back and the scene show them.
pub(super) fn settle(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>) {
    let t = std::time::Instant::now();
    frames(cx, window, view);
    while cx.update(|cx| {
        let vp = view.read(cx).viewport.read(cx);
        vp.pick_pending() || vp.hover_pending() || !vp.hover_settled()
    }) && t.elapsed() < std::time::Duration::from_secs(60)
    {
        frames(cx, window, view);
    }
    frames(cx, window, view);
}

/// A viewport-local point in window coordinates.
pub(super) fn at_window(cx: &mut TestAppContext, view: &Entity<AssemblyView>, at: DVec2) -> Point<gpui_kit::Pixels> {
    let origin = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
    gpui_kit::point(origin.x + px(at.x as f32), origin.y + px(at.y as f32))
}

pub(super) fn move_to(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>, at: DVec2) {
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
    // The press goes to what is hovered: wait for the glyph's hover pick to come back (on a loaded host the face's
    // from 24 px can still be the hover, and the press takes its instance).
    wait_until(cx, window, &view, |v, _| v.hover_overlay == Some(OverlayPick::Mate(0)));
    let w = at_window(cx, &view, centre + DVec2::new(3.0, 0.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    // The click's pick comes back on its own time (a loaded host): wait for its result, not for what is pending
    // right after the click (nothing yet, before its pick starts).
    wait_until(cx, window, &view, |v, _| v.selected_mate.is_some() || !v.selected.is_empty());
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
    // (As the app clears it: the scene and what a press does follow, with no hover in between.)
    let clear = |cx: &mut TestAppContext| cx.update(|cx| view.update(cx, |v, cx| {
        v.clear_selection();
        v.refresh_scene(false, cx);
    }));

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

/// Live feel cov-1120 A1 (qa FEEL-ASM-DRAG-DEAD-ZONE): a free instance pressed and dragged through the pointer's real
/// moves follows the whole move from the press, no dead zone (ours lost 2 px: 148 of 150).
#[gpui::test]
fn a_dragged_free_instance_takes_the_whole_move_from_the_press(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "drag-delta", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
        call(ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>", "Part 2 <1>"], "fixed": false }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    let centre = |cx: &mut TestAppContext| {
        let (lo, hi) = screen_box(cx, &view, "Part 2 <1>");
        (lo + hi) * 0.5
    };
    let from = centre(cx);
    drag(cx, window, &view, from, from + DVec2::new(100.0, 0.0));
    wait_until(cx, window, &view, |v, _| v.drag.is_none());
    let moved = centre(cx) - from;
    assert!((moved - DVec2::new(100.0, 0.0)).length() < 0.5, "a 100 px drag moves the part 100 px: {moved:?}");
}

/// Live behaviours "feel (3D): assembly" (qa FEEL-ASM-DRAG-LIT, feel/3d-asm/free-during-drag.png): a free part dragged by
/// its face shows no hover look for the whole drag; after the release the face under the pointer lights again.
#[gpui::test]
fn a_dragged_part_shows_no_hover_until_released(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "drag-unlit", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
        call(ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>", "Part 2 <1>"], "fixed": false }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    let (lo, hi) = screen_box(cx, &view, "Part 2 <1>");
    let from = (lo + hi) * 0.5;
    let lit = |cx: &mut TestAppContext| cx.update(|cx| {
        let scene = view.read(cx).viewport.read(cx).scene();
        scene.hover_outline
    });
    move_to(cx, window, &view, from);
    assert!(lit(cx), "the face under the pointer lit before the press");
    let w = at_window(cx, &view, from);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_down(w, MouseButton::Left, Modifiers::default());
    for k in 1..=6 {
        let p = at_window(cx, &view, from + DVec2::new(10.0 * f64::from(k), 0.0));
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(p, Some(MouseButton::Left), Modifiers::default());
        frames(cx, window, &view);
    }
    assert!(cx.update(|cx| view.read(cx).drag.as_ref().is_some_and(|d| d.started)), "dragging");
    assert!(!lit(cx), "nothing lit while the part is dragged");
    let w = at_window(cx, &view, from + DVec2::new(60.0, 0.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_up(w, MouseButton::Left, Modifiers::default());
    settle(cx, window, &view);
    wait_until(cx, window, &view, |v, _| v.drag.is_none());
    settle(cx, window, &view);
    assert!(lit(cx), "released: the face under the pointer lights again");
}

/// live2 asm-feel (3b): in a subassembly, hovering one part outlines only that part (the pre-selection is its face with
/// a two-level path) and a click selects that part's face inside the sub, the collapsed sub's row grey (related).
#[gpui::test]
fn a_part_inside_a_subassembly_hovers_and_is_taken_alone(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "sub-part", |ctx| {
        call(ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
        call(ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 2" }));
        call(ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Sub" }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    // Part 1's top face, in the middle of its image.
    let (centre, sub, part1) = cx.update(|cx| {
        let v = view.read(cx);
        let o = v.result.occurrences.iter().min_by(|a, b| {
            let x = |o: &noshape_model::assembly::eval::PartOccurrence| o.mesh.positions.iter().map(|p| o.transform.transform_point3(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).x).fold(f64::MAX, f64::min);
            x(a).total_cmp(&x(b))
        }).unwrap();
        let (mut lo, mut hi) = (glam::DVec3::splat(f64::MAX), glam::DVec3::splat(f64::MIN));
        for p in &o.mesh.positions {
            let w = o.transform.transform_point3(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
            (lo, hi) = (lo.min(w), hi.max(w));
        }
        let c = v.viewport.read(cx).project(glam::DVec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z)).unwrap();
        (c, o.top(), o.path.clone())
    });
    assert_eq!(part1.len(), 2, "a part inside the sub: a two-level path");
    move_to(cx, window, &view, centre);
    cx.update(|cx| {
        let v = view.read(cx);
        let h = v.hover.as_ref().expect("the face under the pointer");
        assert_eq!((h.kind, &h.path), (crate::scene_builder::PickKind::Face, &part1), "the pre-selection: that part's face, its two-level path");
        // asm-feel2 (c): the face's edges in the hover colour and that part's silhouette in #FFC20E, nothing filled.
        let scene = v.viewport.read(cx).scene();
        assert!(scene.hover_outline && scene.hover_outline_rgb == Some([0xff, 0xc2, 0x0e]), "the part's silhouette band, #FFC20E");
        let built = v.built.as_ref().unwrap();
        let outlined: Vec<u32> = (1..scene.states.len() as u32).filter(|id| scene.states[*id as usize] & noshape_render::state::OUTLINE != 0).collect();
        assert!(!outlined.is_empty() && outlined.iter().all(|id| built.picks.get(*id).is_some_and(|t| t.path == part1)), "only that part outlined");
    });
    let w = at_window(cx, &view, centre);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    settle(cx, window, &view);
    cx.update(|cx| {
        let v = view.read(cx);
        assert_eq!(v.ui.entity_sel.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(), vec![part1.clone()], "the click takes that part's face alone");
        assert_eq!(v.selected, vec![sub], "its sub in the selection");
        assert!(v.picked_inside(sub) && v.related_instances(cx).contains(&sub), "the sub's row related (grey), not selected");
    });
}

/// Dispatches `action` to the focused assembly view, as its key does.
fn press(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>, action: impl gpui_kit::Action) {
    let action: Box<dyn gpui_kit::Action> = Box::new(action);
    cx.update_window(window, |_, w, cx| {
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
        w.dispatch_action(action, cx);
    })
    .unwrap();
    frames(cx, window, view);
}

/// live2 shortcuts-1118 (keys pressed in live and ours): Shift+R's bar, W's prompt (Esc ends both mode and prompt),
/// N normal to the selected face, Shift+P hides the origin (and keeps it hidden), Ctrl+C without a toast, Ctrl+V's copy
/// at the pointer, unselected and staying there (live2 paste-1119; one undo removes it), Shift+S keeps the selection with
/// no banner, Shift+I takes the selection away.
#[gpui::test]
fn the_assembly_keys_act_as_live(cx: &mut TestAppContext) {
    use crate::actions::*;
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = mated_blocks(cx, "keys-1118");
    press(cx, window, &view, ViewHighQuality);
    assert!(cx.update(|cx| view.read(cx).ui.high_quality), "Shift+R: viewing in high quality (its bar)");
    press(cx, window, &view, ViewHighQuality);
    press(cx, window, &view, ZoomWindow);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.viewport.read(cx).zoom_window, "W arms the zoom window");
        assert_eq!(v.message.as_ref().map(|(e, m)| (*e, m.as_str())), Some((false, "Drag to create zoom bounding box.")), "W's blue prompt");
    });
    press(cx, window, &view, Cancel);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(!v.viewport.read(cx).zoom_window && v.message.is_none(), "Esc ends the mode and its prompt");
    });
    // N: the view turns normal to Part 1's -Y face, selected.
    let found = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let o = v.result.occurrences.iter().find(|o| v.assembly(cx).and_then(|a| a.instance(o.top())).is_some_and(|i| i.name == "Part 1 <1>")).unwrap();
            let (path, faces) = (o.path.clone(), o.face_refs.clone());
            faces.into_iter().any(|r| {
                v.ui.entity_sel = vec![(path.clone(), r)];
                v.selected_face_normal(cx).is_some_and(|(f, _)| f.y > 0.99)
            })
        })
    });
    assert!(found, "Part 1's -Y face");
    press(cx, window, &view, ViewNormal);
    for _ in 0..40 {
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        frames(cx, window, &view);
    }
    let fwd = cx.update(|cx| view.read(cx).viewport.read(cx).camera.forward());
    assert!(fwd.y > 0.999, "N: normal to the face (forward {fwd:?})");
    // Shift+P hides the origin; a second press keeps it hidden.
    press(cx, window, &view, HideConstruction);
    press(cx, window, &view, HideConstruction);
    assert!(cx.update(|cx| view.read(cx).ui.origin_hidden), "Shift+P hides the origin");
    // Ctrl+C: no toast; Ctrl+V: the copy at the pointer, not selected, dropped there at once.
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    move_to(cx, window, &view, (lo + hi) * 0.5);
    let part1 = cx.update(|cx| view.read(cx).assembly(cx).and_then(|a| a.instances.iter().find(|i| i.name == "Part 1 <1>")).unwrap().id);
    cx.update(|cx| view.update(cx, |v, _| v.selected = vec![part1]));
    press(cx, window, &view, Copy);
    assert!(cx.update(|cx| view.read(cx).message.is_none()), "Ctrl+C shows no toast");
    let count = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).assembly(cx).map_or(0, |a| a.instances.len()));
    let before = count(cx);
    press(cx, window, &view, Paste);
    assert_eq!(count(cx), before + 1, "the copy is in");
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.placing.is_none(), "dropped at once: it doesn't follow the pointer");
        let copy = v.assembly(cx).and_then(|a| a.instances.last()).unwrap().id;
        assert!(!v.selected.contains(&copy), "the copy isn't selected");
    });
    let (clo, chi) = screen_box(cx, &view, &cx.update(|cx| view.read(cx).assembly(cx).and_then(|a| a.instances.last()).unwrap().name.clone()));
    let at = (lo + hi) * 0.5;
    assert!(clo.x <= at.x && at.x <= chi.x && clo.y <= at.y && at.y <= chi.y, "under the pointer: {clo:?}..{chi:?} around {at:?}");
    // (Undo is the workspace's: the document's.)
    cx.update(|cx| view.read(cx).doc.clone().update(cx, |d, cx| d.undo(cx)));
    frames(cx, window, &view);
    assert_eq!(count(cx), before, "one undo removes it");
    // Shift+S: snap mode on, the selection kept, no message.
    cx.update(|cx| view.update(cx, |v, _| v.selected = vec![part1]));
    press(cx, window, &view, ToggleSnapMode);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.snap.is_some() && v.selected == vec![part1] && v.message.is_none(), "Shift+S keeps the selection, no message");
    });
    press(cx, window, &view, ToggleSnapMode);
    // Shift+I: the dialog, the selection gone.
    press(cx, window, &view, IsolateSelected);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.isolate.is_some() && v.selected.is_empty() && v.ui.entity_sel.is_empty(), "Shift+I isolates and clears the selection");
    });
}

/// live2 shortcuts-1118 (H): Show mates mode shows the hovered instance's mates' glyphs, hidden ones too.
#[gpui::test]
fn show_mates_mode_shows_the_hovered_instances_mates(cx: &mut TestAppContext) {
    use crate::actions::*;
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = mated_blocks(cx, "keys-1118-h");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let id = v.result.mates[0].id;
            v.set_mates_hidden(vec![id], true, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, &view, |v, _| v.ui.overlay_mates.is_empty());
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    move_to(cx, window, &view, (lo + hi) * 0.5);
    assert!(cx.update(|cx| view.read(cx).ui.overlay_mates.is_empty()), "hidden: no glyph");
    press(cx, window, &view, ToggleShowMatesMode);
    wait_until(cx, window, &view, |v, _| !v.ui.overlay_mates.is_empty());
    assert!(cx.update(|cx| !view.read(cx).ui.overlay_mates.is_empty()), "H: the hovered part's mate glyph shows");
}

/// A mate row's double-click opens the mate's edit dialog; a single click only selects it (interaction-feel "Mate row
/// double-click").
#[gpui::test]
fn a_mate_rows_double_click_edits_the_mate(cx: &mut TestAppContext) {
    let (window, view) = mated_blocks(cx, "mate-dblclick");
    let id = cx.update(|cx| view.read(cx).result.mates[0].id);
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let at = vcx.debug_bounds("mate-row Fastened 1").expect("Fastened 1's row").center();
    vcx.simulate_click(at, Modifiers::default());
    draw(cx, window);
    cx.update(|cx| assert!(view.read(cx).mate.is_none() && view.read(cx).selected_mate == Some(id), "a click selects it, no dialog"));
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    for count in [1, 2] {
        vcx.simulate_event(gpui_kit::MouseDownEvent { position: at, modifiers: Modifiers::default(), button: gpui_kit::MouseButton::Left, click_count: count, first_mouse: false });
        vcx.simulate_event(gpui_kit::MouseUpEvent { position: at, modifiers: Modifiers::default(), button: gpui_kit::MouseButton::Left, click_count: count });
    }
    draw(cx, window);
    cx.update(|cx| assert_eq!(view.read(cx).mate.as_ref().and_then(|m| m.editing), Some(id), "a double-click opens Fastened 1's dialog"));
}

/// Interaction-feel "Mate row hover": a hovered mate row lights both its parts and shows its glyph in the frame it
/// arrives; off the row both go (the flyout's rows: `a_mate_rows_flyout_shows_the_type_alone_until_a_parameter_is_set`).
#[gpui::test]
fn a_hovered_mate_row_lights_both_parts_and_its_glyph(cx: &mut TestAppContext) {
    let (window, view) = mated_blocks(cx, "mate-row-hover");
    cx.update(|cx| view.update(cx, |v, cx| {
        v.show_mates = false;
        v.refresh_scene(false, cx);
    }));
    draw(cx, window);
    let id = cx.update(|cx| view.read(cx).result.mates[0].id);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        let (scene, built) = (v.viewport.read(cx).scene(), v.built.as_ref().unwrap());
        let a = v.assembly(cx).unwrap();
        let mut names: Vec<String> = a.instances.iter().filter(|i| built.picks.faces_of(i.id).iter().any(|f| scene.states[*f as usize] & noshape_render::state::ACCENT != 0)).map(|i| i.name.clone()).collect();
        names.sort();
        (names, v.ui.overlay_mates.contains(&id))
    });
    assert_eq!(lit(cx), (vec![], false), "at rest, mates hidden: nothing lit, no glyph");
    let at = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mate-row Fastened 1").expect("Fastened 1's row").center();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(at, None, Modifiers::default());
    draw(cx, window);
    assert_eq!(lit(cx), (vec!["Part 1 <1>".to_string(), "Part 2 <1>".to_string()], true), "hovered: both parts and the glyph");
    let off = at_window(cx, &view, DVec2::new(20.0, 20.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(off, None, Modifiers::default());
    draw(cx, window);
    assert_eq!(lit(cx), (vec![], false), "off the row: gone");
}

/// Interaction-feel "Esc / empty graphics" (assembly instance list): a part clicked in the graphics stays selected
/// through Esc; a click on the empty graphics clears it.
#[gpui::test]
fn esc_keeps_the_selection_and_an_empty_graphics_click_clears_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = mated_blocks(cx, "esc-empty");
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    let on_part = (lo + hi) * 0.5;
    move_to(cx, window, &view, on_part);
    wait_until(cx, window, &view, |v, _| v.hover.is_some());
    let w = at_window(cx, &view, on_part);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    wait_until(cx, window, &view, |v, _| !v.selected.is_empty() || !v.ui.entity_sel.is_empty());
    settle(cx, window, &view);
    let taken = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).selected.len() + view.read(cx).ui.entity_sel.len());
    assert!(taken(cx) > 0, "the click took the part");
    press(cx, window, &view, crate::actions::Cancel);
    assert!(taken(cx) > 0, "Esc keeps the selection");
    let empty = DVec2::new(20.0, 20.0);
    move_to(cx, window, &view, empty);
    let w = at_window(cx, &view, empty);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    wait_until(cx, window, &view, |v, _| v.selected.is_empty() && v.ui.entity_sel.is_empty());
    settle(cx, window, &view);
    assert_eq!(taken(cx), 0, "an empty graphics click clears it");
}

/// Live selother-ball-1119: Select other (`) with the pointer over the background off a part's silhouette edge lists
/// "Edge of Part 1 <1>" and the part out to 8 px, the face behind that edge and the part from 9 to 11 px, and nothing
/// from 12 px.
#[gpui::test]
fn select_other_reaches_an_edge_to_8_px_and_its_face_to_11(cx: &mut TestAppContext) {
    use crate::scene_builder::PickKind;
    use super::behaviors::SelectOtherRow;
    const SELECT_OTHER_REACH: f64 = 14.0;
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = mated_blocks(cx, "selother-reach");
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    let part1 = cx.update(|cx| view.read(cx).assembly(cx).unwrap().instances.iter().find(|i| i.name == "Part 1 <1>").unwrap().id);
    // (Clear of the 8 / 11 px boundaries: an edge's pick pixels lie a pixel nearer or farther on another GPU.)
    for (off, want) in [(6.5, Some(PickKind::Edge)), (10., Some(PickKind::Face)), (13.5, None)] {
        move_to(cx, window, &view, DVec2::new(lo.x - off, (lo.y + hi.y) / 2.));
        // (The pick rectangle around the pointer in, as after a pause.)
        wait_until(cx, window, &view, |v, cx| v.viewport.read(cx).pick_at_pointer_within(SELECT_OTHER_REACH, &|_| false).is_some());
        cx.update(|cx| view.update(cx, |v, cx| {
            v.ui.select_other = None;
            v.select_other(false, cx);
        }));
        cx.update(|cx| {
            let rows = &view.read(cx).ui.select_other.as_ref().unwrap().0;
            let kind = rows.iter().find_map(|r| match r {
                SelectOtherRow::Entity(t) => Some(t.kind),
                SelectOtherRow::Instance(_) => None,
            });
            assert_eq!(kind, want, "{off} px off the edge");
            let has_part = rows.iter().any(|r| matches!(r, SelectOtherRow::Instance(id) if *id == part1));
            assert_eq!(has_part, want.is_some(), "{off} px: Part 1 <1> listed with its entity");
        });
    }
}

/// The presented frame's pixels (BGRA, device px), its width and the window's scale.
fn frame_pixels(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<AssemblyView>) -> (Vec<u8>, usize) {
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    frames(cx, window, view);
    cx.update(|cx| vp.update(cx, |v, _| v.record_frames(true)));
    let t = std::time::Instant::now();
    while cx.update(|cx| vp.read(cx).recorded_frames().len()) < 2 && t.elapsed() < std::time::Duration::from_secs(15) {
        frames(cx, window, view);
    }
    let img = cx.update(|cx| vp.read(cx).recorded_frames()).last().cloned().expect("a presented frame");
    cx.update(|cx| vp.update(cx, |v, _| v.record_frames(false)));
    let w = img.size(0).width.0 as usize;
    (img.as_bytes(0).map(|b| b.to_vec()).unwrap_or_default(), w)
}

/// Live2 asm-dialogs-1119 hz: with the mate dialog open, a hovered face's corner dots are filled white (the disc lost
/// to the face's depth: hollow rings); after the first pick the connector's instance is outlined and its face selected.
#[gpui::test]
fn a_mate_dialogs_dots_are_white_and_the_first_pick_outlines_its_instance(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "mate-dots", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let (lo, hi) = v.result.bounds().unwrap();
            v.viewport.update(cx, |vp, cx| {
                let s = vp.size();
                vp.camera.set_view(noshape_render::StandardView::Isometric);
                vp.camera.fit_sphere((lo + hi) * 0.5, (hi - lo).length() * 0.6, s.x / s.y);
                vp.request_frame(cx);
            });
            v.start_mate(noshape_model::assembly::MateType::Fastened, w, cx);
        })
    })
    .unwrap();
    settle(cx, window, &view);
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    let on_top = (lo + hi) * 0.5 - DVec2::new(0.0, (hi.y - lo.y) * 0.2);
    move_to(cx, window, &view, on_top);
    wait_until(cx, window, &view, |v, _| v.infer.is_some());
    settle(cx, window, &view);
    // The top face's corner nearest the top of the screen (+Z up in the isometric view): its dot's centre.
    let corner = cx.update(|cx| {
        let v = view.read(cx);
        let id = v.assembly(cx).unwrap().instances.iter().find(|i| i.name == "Part 1 <1>").unwrap().id;
        let o = v.result.occurrences.iter().find(|o| o.top() == id).unwrap();
        let vp = v.viewport.read(cx);
        o.mesh.positions.iter().map(|p| o.transform.transform_point3(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))).filter_map(|p| vp.project(p)).min_by(|a, b| a.y.total_cmp(&b.y)).unwrap()
    });
    let scale = cx.update_window(window, |_, w, _| f64::from(w.scale_factor())).unwrap();
    let (px, w) = frame_pixels(cx, window, &view);
    let at = |p: DVec2| {
        let (x, y) = ((p.x * scale).round() as usize, (p.y * scale).round() as usize);
        let i = (y * w + x) * 4;
        [px[i + 2], px[i + 1], px[i]]
    };
    let c = at(corner);
    assert!(c.iter().all(|v| *v >= 235), "the corner dot filled white: rgb {c:?}");
    // The face outlined once: its edges the 2 device px outline, not each a hovered edge's 5 px band.
    cx.update(|cx| {
        let v = view.read(cx);
        let (scene, built) = (v.viewport.read(cx).scene(), v.built.as_ref().unwrap());
        let lit: Vec<u32> = (1..scene.states.len() as u32).filter(|id| scene.states[*id as usize] & noshape_render::state::HOVER != 0 && built.picks.get(*id).is_some_and(|t| t.kind == crate::scene_builder::PickKind::Edge)).collect();
        let banded = lit.iter().filter(|id| scene.states[**id as usize] & noshape_render::state::OUTLINE == 0).count();
        assert!(lit.len() >= 4 && banded == 0, "the face's {} edges outlined, {banded} of them banded", lit.len());
    });
    let click = at_window(cx, &view, on_top);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(click, Modifiers::default());
    settle(cx, window, &view);
    assert!(cx.update(|cx| view.read(cx).mate.as_ref().is_some_and(|m| m.dialog.read(cx).connectors().len() == 1)), "the first connector picked");
    move_to(cx, window, &view, on_top + DVec2::new(400.0, 200.0));
    settle(cx, window, &view);
    assert!(cx.update(|cx| view.read(cx).viewport.read(cx).scene().hover_outline), "its instance outlined");
    // Live2 recheck-1119 asm2 (2): the picked face stays filled yellow with the pointer away (its connector at 20 %).
    cx.update(|cx| {
        let v = view.read(cx);
        let (scene, built) = (v.viewport.read(cx).scene(), v.built.as_ref().unwrap());
        let picked = noshape_render::state::SELECTED | noshape_render::state::PICKED;
        let selected = (1..scene.states.len() as u32).filter(|id| scene.states[*id as usize] & picked == picked && built.picks.get(*id).is_some_and(|t| t.kind == crate::scene_builder::PickKind::Face)).count();
        assert_eq!(selected, 1, "the picked face filled the picks' yellow (live feel cov-1120 A2), not a selection's amber");
    });
}

/// Interaction-feel "Toolbar's place" and "Toolbar's grip" (live2 REQUESTS 18:20, place-1118): the mate toolbar's top
/// left stands 12 px right of and 14 px below the second pick, near the graphics' left edge too; a drag on its grip
/// moves it with the pointer, it stays where released, and Escape twice still cancels the mate.
#[gpui::test]
fn the_mate_toolbar_stands_by_the_second_pick_and_its_grip_moves_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "mate-toolbar", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    });
    let (window, view) = open_from_top_on(cx, &path, true, |v| v.result.occurrences.len() == 2);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            // The parts in the graphics' left third, so the second pick is near its left edge.
            let (lo, hi) = v.result.bounds().unwrap();
            v.viewport.update(cx, |vp, cx| {
                let s = vp.size();
                vp.camera.fit_sphere((lo + hi) * 0.5 + glam::DVec3::X * (hi.x - lo.x) * 1.2, (hi - lo).length() * 1.2, s.x / s.y);
                vp.request_frame(cx);
            });
            v.start_mate(noshape_model::assembly::MateType::Fastened, w, cx);
        })
    })
    .unwrap();
    settle(cx, window, &view);
    let pick = |cx: &mut TestAppContext, name: &str| {
        let (lo, hi) = screen_box(cx, &view, name);
        let at = (lo + hi) * 0.5;
        move_to(cx, window, &view, at);
        let w = at_window(cx, &view, at);
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
        settle(cx, window, &view);
        at
    };
    pick(cx, "Part 2 <1>");
    let second = pick(cx, "Part 1 <1>");
    wait_until(cx, window, &view, |v, cx| v.mate.as_ref().is_some_and(|m| m.hud.is_some() && m.dialog.read(cx).connectors_complete()));
    draw(cx, window);
    let origin = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
    assert!(second.x + 12.0 < f64::from(f32::from(origin.x)), "the second pick {second:?} nearer the graphics' left edge than its window x {:?}", origin.x);
    let hud = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mate-hud").expect("the mate toolbar").origin;
    let placed = hud(cx);
    let want = at_window(cx, &view, second + DVec2::new(12.0, 14.0));
    assert!((placed.x - want.x).abs() <= px(1.) && (placed.y - want.y).abs() <= px(1.), "the toolbar at {placed:?}, 12 right of and 14 below the pick: {want:?}");
    // Its grip: dragged (+150, +100), it follows and stays.
    let grip = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mate-hud-drag").expect("the toolbar's grip").center();
    let by = gpui_kit::point(px(150.), px(100.));
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(grip, MouseButton::Left, Modifiers::default());
    for k in 1..=10 {
        let t = k as f32 / 10.0;
        vcx.simulate_mouse_move(gpui_kit::point(grip.x + by.x * t, grip.y + by.y * t), Some(MouseButton::Left), Modifiers::default());
    }
    vcx.simulate_mouse_up(grip + by, MouseButton::Left, Modifiers::default());
    draw(cx, window);
    let moved = hud(cx);
    assert!((moved.x - placed.x - by.x).abs() <= px(1.) && (moved.y - placed.y - by.y).abs() <= px(1.), "the toolbar moved with the grip: {placed:?} → {moved:?}");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_move(grip + by + gpui_kit::point(px(200.), px(0.)), None, Modifiers::default());
    draw(cx, window);
    assert_eq!(hud(cx), moved, "it stays where released");
    for _ in 0..2 {
        press(cx, window, &view, crate::actions::Cancel);
    }
    assert!(cx.update(|cx| view.read(cx).mate.is_none() && view.read(cx).result.mates.is_empty()), "Escape twice cancels the mate");
}

/// Live reach-1118 (feel 3D): a mate connector lights 22–23 px across and 14–15 px down from its origin on a top
/// face's centre in the isometric view, 22 across and 16–21 down on a vertex, and wins over the vertex throughout.
#[gpui::test]
fn a_mate_connector_reaches_as_live_and_wins_over_its_vertex(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "mc-reach", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        let top = call(ctx, "assembly_entities", json!({ "instance": "Part 1 <1>", "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
        let corner = call(ctx, "assembly_entities", json!({ "instance": "Part 1 <1>", "kind": "vertices", "at": [[40, 0, 10]] }))["vertices"][0]["id"].clone();
        call(ctx, "assembly_mate_connector", json!({ "on": { "instance": "Part 1 <1>", "entity": top, "inference": "center" } }));
        call(ctx, "assembly_mate_connector", json!({ "on": { "instance": "Part 1 <1>", "entity": corner, "inference": "vertex" } }));
    });
    let (window, view) = open_from_top(cx, &path, 1);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (lo, hi) = v.result.bounds().unwrap();
            v.viewport.update(cx, |vp, cx| {
                let s = vp.size();
                vp.camera.set_view(noshape_render::StandardView::Isometric);
                vp.camera.fit_sphere((lo + hi) * 0.5, (hi - lo).length() * 0.8, s.x / s.y);
                vp.request_frame(cx);
            });
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    settle(cx, window, &view);
    assert_eq!(cx.update(|cx| view.read(cx).overlay_connectors.len()), 2, "both connectors shown");
    let origins: Vec<DVec2> = cx.update(|cx| {
        let v = view.read(cx);
        let ids = v.overlay_connectors.clone();
        ids.iter().map(|o| {
            let super::OverlayConnector::Assembly(id) = o else { panic!("an assembly connector") };
            v.result.connectors.iter().find(|c| c.id == *id).and_then(|c| c.frame).map(|f| v.viewport.read(cx).project(f.origin).unwrap()).unwrap()
        }).collect()
    });
    // Our reach (scanned in 1 px steps): 22 right, 23 left, 15 up and down on both; live's 16–21 down on a vertex is a
    // Part Studio's hold (an assembly has none, live reach-1118).
    for (i, o) in origins.iter().enumerate() {
        let reach = [(DVec2::X, 22.0, 25.0), (-DVec2::X, 22.0, 25.0), (DVec2::Y, 14.0, 18.0), (-DVec2::Y, 14.0, 18.0)];
        for (dir, lit, past) in reach {
            for d in [0.0, 4.0, 8.0, lit] {
                move_to(cx, window, &view, *o + dir * d);
                let (ov, hover) = cx.update(|cx| (view.read(cx).hover_overlay, view.read(cx).hover.as_ref().map(|t| t.kind)));
                assert_eq!((ov, hover), (Some(OverlayPick::Connector(i)), None), "connector {i}, {d} px along {dir:?}: the connector, not what is under it");
            }
            move_to(cx, window, &view, *o + dir * past);
            assert_ne!(cx.update(|cx| view.read(cx).hover_overlay), Some(OverlayPick::Connector(i)), "connector {i}, {past} px along {dir:?}: past its reach");
        }
    }
}

/// Live asm-views2-1118: dragging a section view's arrow moves its plane in 0.1 mm steps, its value in a box at the
/// arrow and its caps the tan hatched preview on every instance while dragged; after release the box stays, focused,
/// its text selected.
#[gpui::test]
fn a_section_arrows_drag_steps_by_a_tenth_of_a_mm_and_leaves_its_value_box_focused(cx: &mut TestAppContext) {
    use gpui_kit::Focusable as _;
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "section-arrow", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let (lo, hi) = v.result.bounds().unwrap();
            v.viewport.update(cx, |vp, cx| {
                let s = vp.size();
                vp.camera.set_view(noshape_render::StandardView::Isometric);
                vp.camera.fit_sphere((lo + hi) * 0.5, (hi - lo).length() * 0.8, s.x / s.y);
                vp.request_frame(cx);
            });
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    settle(cx, window, &view);
    // Section view on Part 1's top face: the plane through its centroid, the arrow along its normal.
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    move_to(cx, window, &view, (lo + hi) * 0.5 - DVec2::new(0.0, (hi.y - lo.y) * 0.2));
    assert_eq!(cx.update(|cx| view.read(cx).hover.as_ref().map(|t| t.kind)), Some(crate::scene_builder::PickKind::Face), "Part 1's top face hovered");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_section(w, cx))).unwrap();
    settle(cx, window, &view);
    // The arrow's head (60–85 px from the base along the plane's direction) and its direction on screen.
    let (head, along) = cx.update(|cx| {
        let v = view.read(cx);
        let s = v.ui.section.as_ref().unwrap();
        let pl = &s.planes[s.dialog.as_ref().unwrap().active];
        let vp = v.viewport.read(cx);
        let wpp = vp.camera.world_per_pixel(vp.size().y.max(1.0));
        let (b, h) = (vp.project(pl.base()).unwrap(), vp.project(pl.base() + pl.dir() * 72.0 * wpp).unwrap());
        (h, (h - b).normalize())
    });
    move_to(cx, window, &view, head);
    assert_eq!(cx.update(|cx| view.read(cx).hover_overlay), Some(OverlayPick::Section(crate::assembly_scene::SectionHandle::Arrow)), "the arrow hovered");
    let down = at_window(cx, &view, head);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_down(down, MouseButton::Left, Modifiers::default());
    for k in 1..=6 {
        let p = at_window(cx, &view, head - along * (5.0 * f64::from(k)));
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(p, Some(MouseButton::Left), Modifiers::default());
        frames(cx, window, &view);
        cx.update_window(window, |_, _, cx| {
            view.update(cx, |v, cx| {
                let d = v.ui.section_drag.clone().expect("dragging the arrow");
                if !d.moved {
                    return;
                }
                let tenths = d.value * 1e4;
                assert!((tenths - tenths.round()).abs() < 1e-6, "{k}: the plane moved in 0.1 mm steps: {} mm", d.value * 1e3);
                assert!(v.render_section_value(cx).is_some(), "{k}: the value box at the arrow");
                let active = v.ui.section.as_ref().unwrap().dialog.as_ref().unwrap().active;
                assert!(v.viewport.read(cx).scene().clip_selected[active], "{k}: the caps the tan hatched preview");
            })
        })
        .unwrap();
    }
    let value = cx.update(|cx| view.read(cx).ui.section_drag.as_ref().map(|d| d.value)).expect("still dragging");
    assert!(value < -1e-3, "a 30 px drag against the arrow moved the plane back: {} mm", value * 1e3);
    let up = at_window(cx, &view, head - along * 30.0);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_up(up, MouseButton::Left, Modifiers::default());
    settle(cx, window, &view);
    cx.update_window(window, |_, w, cx| {
        let v = view.read(cx);
        assert!(v.ui.section_drag.is_none(), "the drag ended");
        let sv = v.ui.section_value.as_ref().expect("the value box stays after release");
        let st = sv.input.read(cx).state().clone();
        let st = st.read(cx);
        let text = st.value().to_string();
        assert_eq!(text, format!("{:.1} mm", value * 1e3), "the dragged value, one decimal");
        assert!(st.focus_handle(cx).is_focused(w), "the box focused");
        assert_eq!(st.selected_range().len(), text.len(), "its text selected");
        let active = v.ui.section.as_ref().unwrap().dialog.as_ref().unwrap().active;
        assert!(v.viewport.read(cx).scene().clip_selected[active], "the caps tan while the dialog is open (live: the part's colour after ✓)");
    })
    .unwrap();
}

/// Interaction-feel "Suppressed mate" (live assembly/198; live2 fields.json mateList.suppressed): a suppressed mate
/// stays listed, grey (inactive) italic struck through, its tooltip "Fastened 1 (Suppressed)".
#[gpui::test]
fn a_suppressed_mate_row_is_grey_and_its_tooltip_says_suppressed(cx: &mut TestAppContext) {
    let (window, view) = mated_blocks(cx, "mate-suppressed");
    cx.update(|cx| view.update(cx, |v, cx| {
        v.edit("Suppress", cx, |a| a.mates[0].suppressed = true);
    }));
    draw(cx, window);
    assert!(gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mate-row Fastened 1").is_some(), "still listed");
    cx.update(|cx| {
        let v = view.read(cx);
        let m = v.assembly(cx).unwrap().mates[0].clone();
        assert_eq!(v.mate_tooltip(&m).lines().next(), Some("Fastened 1 (Suppressed)"));
    });
    assert_eq!(super::feature_row_color(true, true, false), crate::theme::p().text_faint, "grey as an inactive row");
}

/// Interaction-feel "Failed mate's marks" (live2 errors-1118/1119): a Fastened mate between two fixed parts that it
/// can't hold fails; its tooltip reads "Fastened 1 has error: …" and each fixed part's icon carries the ⚠ badge
/// ("More than one part is fixed").
#[gpui::test]
fn a_failed_mate_between_fixed_parts_is_marked(cx: &mut TestAppContext) {
    let path = document(cx, "mate-failed", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
        let top = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
        let (a, b) = (top(ctx, "Part 1 <1>"), top(ctx, "Part 2 <1>"));
        call(ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>", "Part 2 <1>"], "fixed": true }));
        call(ctx, "assembly_mate", json!({ "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": a }, "b": { "instance": "Part 2 <1>", "entity": b }, "offset": { "x": "30 mm" } }));
    });
    let (window, view) = open_from_top_when(cx, &path, |v| v.result.mates.first().is_some_and(|s| !s.ok));
    cx.update(|cx| {
        let v = view.read(cx);
        let m = v.assembly(cx).unwrap().mates[0].clone();
        let tip = v.mate_tooltip(&m);
        assert!(tip.starts_with("Fastened 1 has error: "), "{tip}");
    });
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    for name in ["Part 1 <1>", "Part 2 <1>"] {
        assert!(vcx.debug_bounds(format!("fixed {name}").leak()).is_some(), "{name}'s fixed icon");
        assert!(vcx.debug_bounds(format!("fixed-warning {name}").leak()).is_some(), "{name}'s ⚠");
    }
}

/// Interaction-feel "Graphics → list (2)": a part hovered in the graphics area lights nothing in the instance list
/// (no hovered or related row) and opens no flyout.
#[gpui::test]
fn a_part_hovered_in_the_graphics_lights_nothing_in_the_list(cx: &mut TestAppContext) {
    let (window, view) = mated_blocks(cx, "graphics-to-list");
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    crate::ui_script::clear_tooltip();
    move_to(cx, window, &view, (lo + hi) * 0.5);
    wait_until(cx, window, &view, |v, _| v.hover.is_some());
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.hover.is_some(), "the part is hovered");
        assert!(v.ui.list_hover.is_none() && v.related_instances(cx).is_empty(), "no row lit");
    });
    assert_eq!(crate::ui_script::shown_tooltip(), None, "no flyout");
}

/// Interaction-feel "Insert preview": an inserted instance follows the pointer, selected (the solid orange) and drawn
/// see-through for its hidden edges; a click drops it there, deselected, and the pointer moves on without it.
#[gpui::test]
fn an_inserted_instance_follows_the_pointer_and_a_click_drops_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "insert-preview", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    let pin = cx.update(|cx| view.read(cx).assembly(cx).and_then(|a| a.instances.iter().find(|i| i.name == "Part 2 <1>")).unwrap().id);
    cx.update(|cx| view.update(cx, |v, cx| v.start_placing(vec![pin], cx)));
    // Its box centre on screen (it is placed with that centre on the pointer's ray over empty space).
    let centre = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let (lo, hi) = v.result.instance_bounds(pin).unwrap();
            v.viewport.read(cx).project((lo + hi) * 0.5).unwrap()
        })
    };
    let (a, b) = (DVec2::new(70.0, 70.0), DVec2::new(150.0, 100.0));
    for at in [a, b] {
        move_to(cx, window, &view, at);
        wait_until(cx, window, &view, |v, cx| v.viewport.read(cx).project({
            let (lo, hi) = v.result.instance_bounds(pin).unwrap();
            (lo + hi) * 0.5
        }).is_some_and(|c| (c - at).length() < 1.0));
        let c = centre(cx);
        assert!((c - at).length() < 1.0, "on the pointer at {at:?}: {c:?}");
        cx.update(|cx| {
            let v = view.read(cx);
            assert_eq!(v.placing.as_deref(), Some(&[pin][..]), "still following");
            assert_eq!(v.selected, vec![pin], "selected: drawn orange");
            assert!(v.scene_options(cx).xray_tops.contains(&pin), "its hidden edges show through");
        });
    }
    let w = at_window(cx, &view, b);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    wait_until(cx, window, &view, |v, _| v.placing.is_none());
    settle(cx, window, &view);
    let dropped = centre(cx);
    assert!((dropped - b).length() < 1.0, "dropped where it was: {dropped:?}");
    cx.update(|cx| assert!(!view.read(cx).selected.contains(&pin), "deselected once placed"));
    move_to(cx, window, &view, a);
    let after = centre(cx);
    assert!((after - dropped).length() < 1e-6, "it stays when the pointer moves on: {after:?}");
}

/// Interaction-feel "Short press": a press and release on a free part without moving is a click; it takes the face
/// under the pointer, and the triad stands at the click point; a drag of the triad's centre ball moves the triad, not
/// the part.
#[gpui::test]
fn a_short_press_selects_the_face_and_the_triads_ball_moves_only_the_triad(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "short-press", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    let part = cx.update(|cx| view.read(cx).assembly(cx).and_then(|a| a.instances.iter().find(|i| i.name == "Part 1 <1>")).unwrap().id);
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    // Off the face's centre: the triad goes to the click point (live triad-click-1120, asm 3087's Move to origin).
    let on_part = (lo + hi) * 0.5 + DVec2::new((hi.x - lo.x) * 0.25, 0.0);
    move_to(cx, window, &view, on_part);
    wait_until(cx, window, &view, |v, _| v.hover.is_some());
    let w = at_window(cx, &view, on_part);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    wait_until(cx, window, &view, |v, _| !v.ui.entity_sel.is_empty());
    settle(cx, window, &view);
    let triad = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let f = v.manipulator(cx).expect("a triad");
            v.viewport.read(cx).project(f.origin).unwrap()
        })
    };
    cx.update(|cx| {
        let v = view.read(cx);
        assert_eq!(v.ui.entity_sel.len(), 1, "one entity taken");
        let (path, r) = &v.ui.entity_sel[0];
        assert_eq!(path, &vec![part], "on Part 1 <1>");
        let o = v.result.occurrences.iter().find(|o| o.top() == part).unwrap();
        assert!(o.face_refs.contains(r), "a face: {r:?}");
        assert!(v.drag.as_ref().is_none_or(|d| !d.started), "no drag");
    });
    let at = triad(cx);
    assert!((at - on_part).length() < 2.0, "the triad at the click point: {at:?} for {on_part:?}");
    let before = cx.update(|cx| view.read(cx).result.instance_bounds(part).unwrap());
    // Live triad-snap-1121: the release leaves the triad where the ball is: at the pointer, or on the inference point
    // it snapped to over a face.
    let to = at + DVec2::new(60.0, 0.0);
    drag(cx, window, &view, at, to);
    let moved = triad(cx);
    let ball = cx.update(|cx| {
        let v = view.read(cx);
        v.triad_snap(to, cx).map_or(to, |s| v.viewport.read(cx).project(s.marks[s.chosen].origin).unwrap())
    });
    assert!((moved - ball).length() < 2.0, "the triad followed the ball: {moved:?} for {ball:?} (pointer {to:?})");
    let after = cx.update(|cx| view.read(cx).result.instance_bounds(part).unwrap());
    assert!((after.0 - before.0).length() < 1e-9 && (after.1 - before.1).length() < 1e-9, "the part stayed");
}

/// Live triad-snap-1121: the triad's centre ball dragged over a face sits on its inference point nearest the pointer's
/// spot in 3D, however far on screen, the triad's arrows unturned; with that point under the pointer (within the ball's
/// ~8 px ring), and over the background, it follows the pointer; released snapped, the triad stays on the point.
#[gpui::test]
fn the_triads_ball_snaps_over_a_face_and_the_release_leaves_it_on_the_point(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let path = document(cx, "ball-snap", |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    let (lo, hi) = screen_box(cx, &view, "Part 1 <1>");
    let on_part = (lo + hi) * 0.5 + DVec2::new((hi.x - lo.x) * 0.25, 0.0);
    move_to(cx, window, &view, on_part);
    wait_until(cx, window, &view, |v, _| v.hover.is_some());
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at_window(cx, &view, on_part), Modifiers::default());
    wait_until(cx, window, &view, |v, _| !v.ui.entity_sel.is_empty());
    settle(cx, window, &view);
    let triad = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).manipulator(cx).expect("a triad"));
    let start = triad(cx);
    let at = cx.update(|cx| view.read(cx).viewport.read(cx).project(start.origin).unwrap());
    // Part 2's top face: its centre, and a spot 30 % of the way from there to a corner (the centre still nearest).
    let (centre, spot) = cx.update(|cx| {
        let v = view.read(cx);
        let id = v.assembly(cx).and_then(|a| a.instances.iter().find(|i| i.name == "Part 2 <1>")).unwrap().id;
        let (lo, hi) = v.result.instance_bounds(id).unwrap();
        let centre = glam::DVec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z);
        let vp = v.viewport.read(cx);
        let (c, k) = (vp.project(centre).unwrap(), vp.project(glam::DVec3::new(lo.x, lo.y, hi.z)).unwrap());
        (centre, c + (k - c) * 0.3)
    });
    let centre_px = cx.update(|cx| view.read(cx).viewport.read(cx).project(centre).unwrap());
    assert!((spot - centre_px).length() > 20.0, "the spot well off the centre on screen: {spot:?} {centre_px:?}");
    // (On a loaded host the parts' inference points can come after the open: wait for them under the spot.)
    wait_until(cx, window, &view, |v, cx| v.triad_snap(spot, cx).is_some());
    assert!(cx.update(|cx| view.read(cx).triad_snap(spot, cx).is_some()), "the face's points under the spot");
    let w = at_window(cx, &view, at);
    // (The press takes the handle the GPU pick last hovered: on a loaded host that comes frames after the move.)
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(w, None, Modifiers::default());
    wait_until(cx, window, &view, |v, _| v.manip_hover == Some(crate::assembly_scene::ManipHandle::Center));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_down(w, MouseButton::Left, Modifiers::default());
    let go =|cx: &mut TestAppContext, from: DVec2, to: DVec2| {
        for k in 1..=8 {
            let p = at_window(cx, &view, from + (to - from) * (f64::from(k) / 8.0));
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(p, Some(MouseButton::Left), Modifiers::default());
            frames(cx, window, &view);
        }
    };
    let ball = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let f = v.manipulator(cx).unwrap();
            (f, v.viewport.read(cx).project(f.origin).unwrap(), v.drag.as_ref().is_some_and(|d| d.ball_snap.is_some()))
        })
    };
    // (Each check waits for the move to be taken, the pointer resting there and stirring a pixel: a move to where the
    // pointer already is is dropped.)
    let until = |cx: &mut TestAppContext, step: &str, to: DVec2, snapped: bool| {
        let t = std::time::Instant::now();
        let mut k = 0.0;
        while !cx.update(|cx| view.read(cx).drag.as_ref().is_some_and(|d| d.ball_snap.is_some() == snapped)) {
            if t.elapsed() >= std::time::Duration::from_secs(60) {
                // What the snap sees there now: the ray's hit, the chosen mark and its distance on screen, the meshes.
                let why = cx.update(|cx| {
                    let v = view.read(cx);
                    let hit = v.raycast_hit(to, &[], cx).map(|(h, s)| (h.path, h.reference, s));
                    let snap = v.triad_snap(to, cx).map(|s| (s.chosen, s.marks.len(), s.marks[s.chosen].origin));
                    let meshes: Vec<usize> = v.result.occurrences.iter().map(|o| o.mesh.indices.len()).collect();
                    format!("hit {hit:?}; snap {snap:?}; meshes {meshes:?}; drag {:?}", v.drag.as_ref().map(|d| (d.ball_snap.is_some(), d.started, d.ball_at)))
                });
                panic!("{step}: snapped {snapped} at {to:?}: {:?}; {why}", ball(cx).0.origin);
            }
            k = 1.0 - k;
            let p = at_window(cx, &view, to + DVec2::new(k, 0.0));
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(p, Some(MouseButton::Left), Modifiers::default());
            frames(cx, window, &view);
        }
    };
    go(cx, at, spot);
    until(cx, "check 1", spot, true);
    let (f, _, snapped) = ball(cx);
    assert!(snapped && (f.origin - centre).length() < 1e-6, "on the face's centre: {:?} for {centre:?}", f.origin);
    assert!((f.rotation.dot(start.rotation)).abs() > 1.0 - 1e-9, "the triad unturned");
    go(cx, spot, centre_px);
    until(cx, "check 2", centre_px, false);
    let (_, shown, snapped) = ball(cx);
    assert!(!snapped && (shown - centre_px).length() < 2.0, "the point under the pointer: the ball follows it ({shown:?})");
    go(cx, centre_px, spot);
    until(cx, "check 3", spot, true);
    assert!(ball(cx).2, "snapped again off the ring");
    let background = DVec2::new(40.0, 40.0);
    go(cx, spot, background);
    until(cx, "check 4", background, false);
    let (_, shown, snapped) = ball(cx);
    assert!(!snapped && (shown - background).length() < 2.0, "none over the background ({shown:?})");
    go(cx, background, spot);
    until(cx, "check 5", spot, true);
    assert!(ball(cx).2);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_up(at_window(cx, &view, spot), MouseButton::Left, Modifiers::default());
    settle(cx, window, &view);
    let (f, _, _) = ball(cx);
    assert!((f.origin - centre).length() < 1e-6, "released snapped, the triad on the face's centre: {:?} for {centre:?}", f.origin);
    assert!((f.rotation.dot(start.rotation)).abs() > 1.0 - 1e-9, "its arrows as before the drag");
}

/// Part 2 <1> clicked at its top face's centre in live's view of triad-click-1120 (Trimetric at 17.0 px/mm: Part 2's
/// 20 mm along X 305 px, its 10 mm height 148 px), pressed at live's press point off the triad's centre and dragged
/// 30 px right in steps: the handle hovered at the press, the box's texts during the drag and after it, the triad's
/// centre and the part's transform before and after.
fn triad_drag_as_live(cx: &mut TestAppContext, name: &str, press: DVec2, handle: crate::assembly_scene::ManipHandle) -> (Vec<String>, String, glam::DVec3, glam::DMat4, glam::DMat4) {
    let path = document(cx, name, |ctx| {
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    });
    let (window, view) = open_from_top(cx, &path, 2);
    let part = cx.update(|cx| view.read(cx).assembly(cx).and_then(|a| a.instances.iter().find(|i| i.name == "Part 2 <1>")).unwrap().id);
    let (lo, hi) = cx.update(|cx| view.read(cx).result.instance_bounds(part).unwrap());
    let top = glam::DVec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            v.viewport.update(cx, |vp, cx| {
                let h = vp.size().y;
                vp.camera.set_view(noshape_render::StandardView::Trimetric);
                vp.camera.target = top;
                vp.camera.distance = h / (2.0 * 17.0e3 * (vp.camera.fov_y * 0.5).tan());
                vp.request_frame(cx);
            });
            v.refresh_scene(false, cx);
        })
    })
    .unwrap();
    settle(cx, window, &view);
    let c = cx.update(|cx| view.read(cx).viewport.read(cx).project(top).unwrap());
    move_to(cx, window, &view, c);
    wait_until(cx, window, &view, |v, _| v.hover.is_some());
    let w = at_window(cx, &view, c);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(w, Modifiers::default());
    wait_until(cx, window, &view, |v, _| !v.ui.entity_sel.is_empty());
    settle(cx, window, &view);
    let pose = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).result.occurrences.iter().find(|o| o.top() == part).unwrap().transform);
    let before = pose(cx);
    let centre = cx.update(|cx| view.read(cx).manipulator(cx).expect("a triad").origin);
    let from = cx.update(|cx| view.read(cx).viewport.read(cx).project(centre).unwrap()) + press;
    move_to(cx, window, &view, from);
    wait_until(cx, window, &view, |v, _| v.manip_hover.is_some());
    assert_eq!(cx.update(|cx| view.read(cx).manip_hover), Some(handle), "live's press point on {handle:?}");
    let down = at_window(cx, &view, from);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_down(down, MouseButton::Left, Modifiers::default());
    let mut during = Vec::new();
    for k in 1..=6 {
        let p = at_window(cx, &view, from + DVec2::new(5.0 * f64::from(k), 0.0));
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(p, Some(MouseButton::Left), Modifiers::default());
        frames(cx, window, &view);
        let text = cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.render_manip_value(cx).and(v.manip_drag_text(cx)))).unwrap();
        during.extend(text);
    }
    let up = at_window(cx, &view, from + DVec2::new(30.0, 0.0));
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_up(up, MouseButton::Left, Modifiers::default());
    settle(cx, window, &view);
    let after = cx.update(|cx| {
        let v = view.read(cx);
        let st = v.manip_value.as_ref().expect("the value box stays after release").input.read(cx).state().clone();
        st.read(cx).value().to_string()
    });
    (during, after, centre, before, pose(cx))
}

/// Live triad-click-1120 (3): pressed on the X arrow 52 px off the centre and dragged 30 px right, the part moves
/// 1.9 mm along X (0.1 mm steps at this zoom), unturned, with a "1.9 mm" box during the drag and after.
#[gpui::test]
fn the_triads_x_arrow_drags_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (during, after, _, before, end) = triad_drag_as_live(cx, "triad-x", DVec2::new(50.2, 14.5), crate::assembly_scene::ManipHandle::Axis(0));
    assert!(!during.is_empty() && during.last().is_some_and(|t| t == "1.9 mm"), "the box during the drag: {during:?}");
    assert_eq!(after, "1.9 mm", "the box after release");
    let moved = end.w_axis.truncate() - before.w_axis.truncate();
    assert!((moved - glam::DVec3::new(1.9e-3, 0.0, 0.0)).length() < 1e-9, "1.9 mm along X: {moved:?}");
    assert!(end.x_axis.abs_diff_eq(before.x_axis, 1e-12) && end.z_axis.abs_diff_eq(before.z_axis, 1e-12), "unturned");
}

/// Live triad-click-1120 (3b): pressed on the ring at the Z arrow's tip (91 px above the centre) and dragged 30 px
/// right, the part turns 15.0° about world Y through the triad's centre (16.8° dragged, 5° steps), with a "15 deg" box
/// during the drag and after.
#[gpui::test]
fn the_triads_z_ring_turns_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (during, after, centre, before, end) = triad_drag_as_live(cx, "triad-ring", DVec2::new(0.0, -91.3), crate::assembly_scene::ManipHandle::Ring(2));
    assert!(!during.is_empty() && during.last().is_some_and(|t| t == "15 deg"), "the box during the drag: {during:?}");
    assert_eq!(after, "15 deg", "the box after release");
    let turn = end * before.inverse();
    let (s, c) = 15f64.to_radians().sin_cos();
    assert!(turn.transform_vector3(glam::DVec3::Z).abs_diff_eq(glam::DVec3::new(s, 0.0, c), 1e-9) && turn.transform_vector3(glam::DVec3::Y).abs_diff_eq(glam::DVec3::Y, 1e-9), "15° about world Y: {turn:?}");
    assert!(turn.transform_point3(centre).abs_diff_eq(centre, 1e-9), "about the triad's centre {centre:?}: {turn:?}");
}
