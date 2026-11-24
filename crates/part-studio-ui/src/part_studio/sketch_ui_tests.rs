//! UI keystroke tests of sketch mode on GPUI's test platform (docs/TESTS.md
//! "UI keystroke tests"): the real Part Studio view in a test window, the GPU
//! a no-op stand-in, clicks as the viewport's tool events, keys through GPUI.

use super::super::PartStudioView;
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::sketch_mode::{ConstraintTool, Tool};
use crate::viewport::ViewportEvent;
use glam::DVec2;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Focusable as _, IntoElement, Keystroke, Modifiers, InteractiveElement as _, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};
use noshape_model::reference::{DatumPlane, Reference};

/// The app's workspace as far as keys go: the "Workspace" key context the
/// default bindings (Esc → Cancel, …) need above the view.
struct Workspace(Entity<PartStudioView>);

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().key_context("Workspace").size_full().child(self.0.clone())
    }
}

thread_local! {
    /// The next test window's viewport draws real frames (`Gpu::headless_rendering`).
    static RENDERING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A fresh document's Part Studio in a test window, drawn once.
fn part_studio(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    part_studio_from(cx, name, None)
}

struct KeysBound;
impl gpui_kit::Global for KeysBound {}

/// A Part Studio built from a ui-parity start (tests/ui-parity/starts.json's form), or a fresh one.
fn part_studio_from(cx: &mut TestAppContext, name: &str, start: Option<serde_json::Value>) -> (AnyWindowHandle, Entity<PartStudioView>) {
    part_studio_with(cx, name, |path| match start {
        Some(start) => {
            noshape_ops::ui_parity::build_start(&mut noshape_ops::Ctx::new(), path, &start).unwrap();
            crate::session::Session::open(path).unwrap()
        }
        None => crate::session::Session::create(path, crate::user_units::new_document()).unwrap(),
    })
}

/// A Part Studio of the document `make` writes and opens at the path it is given (a scratch file of the test's own).
fn part_studio_with(cx: &mut TestAppContext, name: &str, make: impl FnOnce(&std::path::Path) -> crate::session::Session) -> (AnyWindowHandle, Entity<PartStudioView>) {
    // Once per App, as the app does: bound again, a key's action ran once per window opened (a matrix's Enter in a
    // field applied, then accepted).
    cx.update(|cx| {
        if cx.has_global::<KeysBound>() {
            return;
        }
        cx.set_global(KeysBound);
        gpui_kit::init(cx);
        // The app's default key bindings (Esc → Cancel, tool keys), without the user's customizations.
        cx.bind_keys(crate::actions::default_key_bindings());
        // The value box's own keys (the app binds them once per process).
        cx.bind_keys(super::entry_key_bindings());
    });
    crate::part_studio_hooks::register_for_tests();
    // A file of its own each time (docs/TESTS.md): on Windows an earlier window's session still holds the last one
    // open, so it can't be removed and the start would refuse to build over it.
    static OPENED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("noshape-ui-{name}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    let session = make(&path);
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let gpu = if RENDERING.get() { Gpu::headless_rendering().unwrap() } else { Gpu::headless().unwrap() };
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        // As every app window: its menus (a list row's right-click) draw in it.
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    (window, view)
}

/// A fresh document's Part Studio in a test window, sketching on Top with `tool`.
fn sketch_with(cx: &mut TestAppContext, name: &str, tool: Tool) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = part_studio(cx, name);
    sketch_in(cx, window, view, tool)
}

/// A new sketch on Top in the test window's Part Studio, with `tool`.
fn sketch_in(cx: &mut TestAppContext, window: AnyWindowHandle, view: Entity<PartStudioView>, tool: Tool) -> (AnyWindowHandle, Entity<PartStudioView>) {
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![Reference::Plane(DatumPlane::Top)];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    // The window active, as the user's is (focus events need it); the
    // sketch editor comes up with the next frame.
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    sketch_in_tool(cx, window, view, tool)
}

/// `tool` in the open sketch, the canvas focused.
fn sketch_in_tool(cx: &mut TestAppContext, window: AnyWindowHandle, view: Entity<PartStudioView>, tool: Tool) -> (AnyWindowHandle, Entity<PartStudioView>) {
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| v.activate_tool(tool, w, cx));
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    (window, view)
}

/// A frame (layout, paint: a focused input registers its text handler).
fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Draws until `done` holds (a regeneration on the background threads), at most 30 s (a loaded land server takes
/// seconds; it returns as soon as `done` holds).
fn wait_until(cx: &mut TestAppContext, window: AnyWindowHandle, done: impl Fn(&gpui_kit::App) -> bool) {
    let t = std::time::Instant::now();
    while !cx.update(|cx| done(cx)) && t.elapsed() < std::time::Duration::from_secs(120) {
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        draw(cx, window);
    }
}

/// A click of the tool at viewport pixel `at` (what the viewport sends for a
/// press and release without a drag).
fn click(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, at: (f64, f64)) {
    let at = DVec2::new(at.0, at.1);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            for ev in [
                ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() },
                ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 },
                ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() },
            ] {
                v.on_viewport_event(&vp, &ev, w, cx);
            }
        });
    })
    .unwrap();
    draw(cx, window);
}

/// Keys as the user types them (a character key carries its text), a frame
/// after each.
fn keys(cx: &mut TestAppContext, window: AnyWindowHandle, keys: &[&str]) {
    for k in keys {
        let mut ks = Keystroke::parse(k).unwrap();
        if k.chars().count() == 1 {
            ks.key_char = Some(k.to_string());
        } else if *k == "space" {
            ks.key_char = Some(" ".into());
        }
        cx.update_window(window, |_, w, cx| {
            w.dispatch_keystroke(ks, cx);
        })
        .unwrap();
        draw(cx, window);
    }
}

/// The sketch's dimension values (metres), sorted.
fn dimensions(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<f64> {
    cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).expect("sketching");
        let mut v: Vec<f64> = data.sketch.constraints().filter(|c| c.driving).filter_map(|c| c.value()).collect();
        v.sort_by(f64::total_cmp);
        v
    })
}

fn near(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

/// OTF-FIRST-KEY: Line, click, click, `5` `0` Enter: the value box takes the
/// typed digits from the first key, and Enter makes a 50 mm dimension (the
/// first digit used to come up selected, so `0` replaced it).
#[gpui::test]
fn line_value_box_takes_the_first_key(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "line", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.05]), "{:?}", dimensions(cx, &view));
}

/// Live2 11-17 value-box: mid-chain a digit opens the value box on the last finished segment (never the rubber band);
/// Enter dimensions it and the chain goes on from its moved end; a digit after that, or Tab with no box, opens
/// nothing; Esc in the box closes it with no dimension, the tool on; a canvas click drops the value and places the next
/// point; "1abc" Enter keeps the box open; a circle's box only once the circle is finished.
#[gpui::test]
fn value_boxes_mid_chain_as_lives(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "value-box-chain", Tool::Line);
    let open = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).ui_ref().is_some_and(|u| u.otf_input.is_some()));
    let lines = |cx: &mut TestAppContext| cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        let mut v: Vec<(noshape_sketch::EntityId, f64)> = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).filter_map(|e| Some((e.id, d.sketch.curve(e.id)?.length()))).collect();
        v.sort_by_key(|x| x.0);
        v
    });
    let lengths = |cx: &mut TestAppContext| cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        d.sketch.constraints().filter_map(|c| match c.kind { noshape_sketch::ConstraintKind::Length { curve, value } => Some((curve, value)), _ => None }).collect::<Vec<_>>()
    });
    let pointer = |cx: &mut TestAppContext, at: (f64, f64)| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: DVec2::new(at.0, at.1), modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
    };
    click(cx, window, &view, (400., 400.));
    click(cx, window, &view, (520., 400.));
    click(cx, window, &view, (520., 320.));
    pointer(cx, (560., 300.));
    keys(cx, window, &["tab"]);
    assert!(!open(cx), "Tab with no box: nothing");
    keys(cx, window, &["2", "5"]);
    assert!(open(cx), "a digit opens the box");
    keys(cx, window, &["enter"]);
    let segs = lines(cx);
    assert_eq!(segs.len(), 2, "two segments, the rubber band none: {segs:?}");
    let dims = lengths(cx);
    assert!(dims.len() == 1 && dims[0].0 == segs[1].0 && (dims[0].1 - 0.025).abs() < 1e-9, "LENGTH 25 mm on the last finished segment: {dims:?} {segs:?}");
    assert!((segs[1].1 - 0.025).abs() < 1e-9, "the segment resized: {segs:?}");
    assert!(cx.update(|cx| view.read(cx).sketch.as_ref().is_some_and(|ed| ed.tool == Tool::Line)), "the tool on");
    keys(cx, window, &["3"]);
    assert!(!open(cx), "a digit after Enter opens nothing");
    // Esc in an open box, then a canvas click with one open (one Esc ends Line: the tool armed again).
    let line_tool = |cx: &mut TestAppContext| {
        keys(cx, window, &["escape"]);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
        draw(cx, window);
    };
    line_tool(cx);
    click(cx, window, &view, (400., 500.));
    click(cx, window, &view, (520., 500.));
    keys(cx, window, &["7"]);
    assert!(open(cx), "a new segment's box opens");
    keys(cx, window, &["escape"]);
    assert!(!open(cx) && lengths(cx).len() == 1, "Esc: box gone, no dimension");
    assert!(cx.update(|cx| view.read(cx).sketch.as_ref().is_some_and(|ed| ed.tool == Tool::Line)), "Esc in the box keeps the tool");
    line_tool(cx);
    click(cx, window, &view, (400., 560.));
    click(cx, window, &view, (520., 560.));
    let n = lines(cx).len();
    keys(cx, window, &["7"]);
    click(cx, window, &view, (520., 620.));
    assert_eq!((open(cx), lengths(cx).len(), lines(cx).len()), (false, 1, n + 1), "a canvas click drops the value and places the next point (box, lengths, lines)");
    keys(cx, window, &["8"]);
    keys(cx, window, &["a", "b", "c", "enter"]);
    let invalid = cx.update(|cx| (view.read(cx).ui_ref().is_some_and(|u| u.otf_invalid), view.read(cx).message.is_none()));
    assert!(open(cx) && lengths(cx).len() == 1 && invalid == (true, true), "\"8abc\" Enter: the box stays, drawn invalid, no dimension, no message: {invalid:?}");
}

/// SKETCH-UNDO-REDO-STATE (live 11-02, qa's t-skundo): the sketch toolbar's
/// Undo and Redo follow the sketch's own history: both off on entering it;
/// after a line Undo on, Redo off; after undoing it Undo off, Redo on; after
/// redoing it Undo on, Redo off.
#[gpui::test]
fn sketch_undo_and_redo_follow_its_history(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "sk-undo", Tool::Line);
    let state = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).draft_history_state());
    assert_eq!(state(cx), (false, false));
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape"]);
    assert_eq!(state(cx), (true, false));
    let step = |cx: &mut TestAppContext, redo: bool| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.draft_undo(redo, w, cx))).unwrap();
        draw(cx, window);
    };
    step(cx, false);
    assert_eq!(state(cx), (false, true));
    step(cx, true);
    assert_eq!(state(cx), (true, false));
}

/// Live 1853: a sketch started with no plane, Top then picked: picking the
/// plane is an undo step (Undo on), and Undo past the line un-picks it (the
/// plane field empty again, out of the sketch editor).
#[gpui::test]
fn picking_the_sketch_plane_is_an_undo_step(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "sk-undo-plane");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection.clear();
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    let state = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).draft_history_state());
    assert_eq!(state(cx), (false, false));
    cx.update_window(window, |_, _, cx| {
        let dialog = view.read(cx).edit.as_ref().unwrap().dialog.clone();
        dialog.update(cx, |d, cx| d.add_pick(Reference::Plane(DatumPlane::Top), cx));
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some());
    assert_eq!(state(cx), (true, false));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    draw(cx, window);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape"]);
    assert_eq!(state(cx), (true, false));
    let undo = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.draft_undo(false, w, cx))).unwrap();
        draw(cx, window);
    };
    undo(cx);
    assert_eq!(state(cx), (true, true));
    undo(cx);
    assert_eq!(state(cx), (false, true));
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none());
    assert!(cx.update(|cx| view.read(cx).sketch.is_none()), "the plane is un-picked");
}

/// Live 1853 (c): a plane selected before Sketch isn't an undo step.
#[gpui::test]
fn a_preselected_sketch_plane_is_not_an_undo_step(cx: &mut TestAppContext) {
    let (_window, view) = sketch_with(cx, "sk-undo-presel", Tool::Line);
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (false, false));
}

/// Live (sketch-open feel, interaction-feel "Top selected then Shift+S"): a face selected before Sketch opens it on
/// that face, its Sketch plane field "Face of Extrude 1".
#[gpui::test]
fn a_preselected_face_names_the_sketch_plane_field_by_its_feature(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "sk-plane-face", Some(starts["block"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![v.display.bodies[0].face_refs[0].clone()];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some());
    let items = cx.update_window(window, |_, w, cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).ui_state(w, cx)["fields"][0]["items"].clone())).unwrap();
    assert_eq!(items, Some(serde_json::json!(["Face of Extrude 1"])));
}

/// Live (sketch-entry-1118, interaction-feel "Drawing on a face"): a rectangle drawn on a face is there, solved, in the
/// closing click's frame (live shows a light-blue preview for 0.58 s while its server solves), a corner under that
/// click (the first may snap to the face's edge).
#[gpui::test]
fn a_rectangle_drawn_on_a_face_is_solved_in_the_closing_clicks_frame(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "sk-face-rect", Some(starts["block"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![v.display.bodies[0].face_refs[0].clone()];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some());
    let (window, view) = sketch_in_tool(cx, window, view, Tool::CornerRect);
    let frame = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().frame);
    assert!(frame.origin.z.abs() > 1e-6 || frame.normal.z.abs() < 0.5, "the sketch is on the face, not Top: {frame:?}");
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    let corners = cx.update(|cx| {
        let v = view.read(cx);
        let (vp, data) = (v.viewport.read(cx), v.sketch_data(cx).unwrap());
        let lines = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).count();
        let points: Vec<DVec2> = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).filter_map(|e| data.sketch.point(e.id)).map(|q| vp.project(frame.to_world(q)).unwrap()).collect();
        (lines, points, v.pending.is_none())
    });
    assert_eq!(corners.0, 4, "four lines");
    assert!(corners.2, "nothing waits on a regeneration");
    let at = DVec2::new(650., 400.);
    assert!(corners.1.iter().any(|p| p.distance(at) < 1.0), "a corner under the closing click: {:?}", corners.1);
}

/// Live (sketch-open feel, interaction-feel "A new sketch changes nothing on screen"): a new sketch on Top beside the
/// block hides, greys and isolates nothing, and the view stays where it was.
#[gpui::test]
fn a_new_sketch_changes_nothing_on_screen(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "sk-open-still", Some(starts["block"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    draw(cx, window);
    let shown = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        (v.viewport.read(cx).camera, v.display.bodies.iter().map(|b| (b.hidden, b.color)).collect::<Vec<_>>(), v.ui.isolate.is_some())
    });
    let before = shown(cx);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![Reference::Plane(DatumPlane::Top)];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some());
    draw(cx, window);
    assert_eq!(shown(cx), before);
}

/// The Origin's hidden stand-in (made when a line's first click snaps to the
/// origin) isn't an undo step: live's Undo stays off after that click and an
/// Escape; a drawn line still is a step.
#[gpui::test]
fn the_origins_hidden_point_is_not_an_undo_step(cx: &mut TestAppContext) {
    use noshape_model::params::ParamValue;
    let (_window, view) = sketch_with(cx, "sk-undo-origin", Tool::Line);
    let f = cx.update(|cx| view.read(cx).edit.as_ref().unwrap().dialog.read(cx).feature.clone());
    let mut history = super::super::DraftHistory::new(&f);
    let with = |f: &noshape_model::Feature, edit: &dyn Fn(&mut noshape_model::sketch_feature::SketchData)| {
        let mut g = f.clone();
        if let Some(ParamValue::Sketch(d)) = g.params.get("sketch").cloned() {
            let mut d = d;
            edit(std::sync::Arc::make_mut(&mut d));
            g.params.set("sketch", ParamValue::Sketch(d));
        }
        g
    };
    let hidden = with(&f, &|d| {
        crate::sketch_mode::origin_point(&mut d.sketch);
    });
    assert!(!history.checkpoint(&hidden));
    assert_eq!(history.can(), (false, false));
    let line = with(&hidden, &|d| {
        d.sketch.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.03, 0.0));
    });
    assert!(history.checkpoint(&line));
    assert_eq!(history.can(), (true, false));
}

/// `=#len` in a Line's value box (a Variable `len` = 42 mm before the
/// sketch): Enter makes a 42 mm dimension driven by the variable.
#[gpui::test]
fn line_value_box_takes_a_variable(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "line-var");
    cx.update(|cx| {
        let (doc, tab) = { let v = view.read(cx); (v.doc.clone(), v.tab) };
        doc.update(cx, |d, cx| {
            let units = d.session.doc.settings.units;
            d.edit_studio(tab, "Variable", cx, |ps| {
                let mut f = ps.new_feature("assignVariable", &units).unwrap();
                f.params.set("name", noshape_model::ParamValue::Text("len".into()));
                f.params.set("variableType", noshape_model::ParamValue::Enum("LENGTH".into()));
                f.params.set("lengthValue", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("42 mm")));
                ps.features.push(f);
            });
        });
    });
    draw(cx, window);
    let (window, view) = sketch_in(cx, window, view, Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["=", "#", "l", "e", "n", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.042]), "{:?}", dimensions(cx, &view));
}

/// OTF-FIRST-KEY: corner rectangle, `30` Enter `20` Enter: 30 × 20 (the
/// width's first key used to be lost, and `20` went to the width).
#[gpui::test]
fn rectangle_value_boxes_take_width_then_height(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "rect", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["3", "0", "enter"]);
    let focus = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)["focus"]["kind"].clone()).unwrap();
    assert_eq!(focus, "none", "the height box waits on the canvas until typed into (live)");
    keys(cx, window, &["2", "0", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.02, 0.03]), "{:?}", dimensions(cx, &view));
}

/// SKETCH-F-FIT (live request 1583): F in a sketch frames what is shown
/// (planes, origin, the sketch) as after ✓, over the whole canvas: the Sketch
/// dialog keeps no inset.
#[gpui::test]
fn zoom_fit_in_a_sketch_frames_as_outside(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "fit", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["3", "0", "enter", "2", "0", "enter"]);
    let (lo, hi) = cx.update(|cx| view.read(cx).zoom_fit_bounds()).unwrap();
    assert!(hi.x - lo.x > 0.1 && lo.x <= 0.0 && hi.x >= 0.0, "planes and origin in: {lo} {hi}");
    assert_eq!(cx.update(|cx| view.read(cx).fit_inset_left()), 0.0);
}

/// Tab in a rectangle's value box (live, behaviours "skasm (2026-10-28)
/// sketch value boxes: Tab"): inside the typed width it applies nothing and
/// stays, the value selected with its unit, so 30 Tab 20 Enter makes the
/// width 20 and no height; with nothing typed it makes the height box the
/// active one.
#[gpui::test]
fn rectangle_value_boxes_tab_as_live(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "rect-tab", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["3", "0", "tab", "2", "0", "enter"]);
    assert_eq!(dimension_kinds(cx, &view), vec![(true, 0.02)], "the width 20, no height");
    let (window, view) = sketch_with(cx, "rect-tab2", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["tab", "2", "0", "enter"]);
    assert_eq!(dimension_kinds(cx, &view), vec![(false, 0.02)], "the height 20");
}

/// SKETCH-ENTER-NO-PLANE (live request 1532): a new sketch with no plane
/// yet ignores Enter and Esc (it was committed red / cancelled); only ✕ or a
/// plane and ✓ leave it.
#[gpui::test]
fn enter_and_esc_leave_a_planeless_sketch_open(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "noplane");
    let features = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).map(|s| s.features.len()));
    let before = features(cx);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection.clear();
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "Enter keeps the dialog open");
    assert_eq!(features(cx), before, "nothing committed");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "Esc too");
}

/// The driving dimensions as (a width: a horizontal line's length or the
/// distance between vertical lines; value).
fn dimension_kinds(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<(bool, f64)> {
    cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).expect("sketching");
        let sk = &data.sketch;
        let dir = |e: noshape_sketch::EntityId| match sk.curve(e) {
            Some(noshape_sketch::Curve2::Line { a, b }) => Some(b - a),
            _ => None,
        };
        let width = |es: &[noshape_sketch::EntityId]| match es {
            [e] => dir(*e).is_some_and(|d| d.y.abs() < 1e-9),
            _ => es.iter().all(|e| dir(*e).is_some_and(|d| d.x.abs() < 1e-9)),
        };
        sk.constraints()
            .filter(|c| c.driving)
            .filter_map(|c| Some((width(&c.kind.entities()), (c.value()? * 1e6).round() / 1e6)))
            .collect()
    })
}

/// A press, a drag in `steps` and a release of the tool, viewport pixels.
fn drag(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, from: (f64, f64), to: (f64, f64), steps: usize) {
    let p = |t: f64| DVec2::new(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    let mut evs = vec![ViewportEvent::PointerMove { position: p(0.0), modifiers: Modifiers::default() }, ViewportEvent::ToolDown { position: p(0.0), modifiers: Modifiers::default(), click_count: 1 }];
    evs.extend((1..=steps).map(|k| ViewportEvent::ToolDrag { position: p(k as f64 / steps as f64) }));
    evs.push(ViewportEvent::ToolUp { position: p(1.0), modifiers: Modifiers::default() });
    for ev in evs {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ev, w, cx);
            });
        })
        .unwrap();
        draw(cx, window);
    }
}

/// Conflicting dimensions in one part of a sketch don't stop a drag in
/// another: a point placed beside a line with two different lengths drags
/// with the pointer.
#[gpui::test]
fn a_point_drags_beside_conflicting_dimensions(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "conflict-drag", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Point, w, cx))).unwrap();
    draw(cx, window);
    click(cx, window, &view, (800., 300.));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.with_sketch(cx, |_, data| {
                let line = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
                data.sketch.add_constraint(noshape_sketch::ConstraintKind::Length { curve: line, value: 0.01 }).unwrap();
                data.sketch.add_constraint_ext(noshape_sketch::ConstraintKind::Length { curve: line, value: 0.02 }, true).unwrap();
                data.sketch.solve();
            });
            v.activate_tool(Tool::Select, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    let point = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let data = view.read(cx).sketch_data(cx).unwrap();
            let p = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).max().unwrap();
            data.sketch.point(p).unwrap()
        })
    };
    let before = point(cx);
    drag(cx, window, &view, (800., 300.), (860., 330.), 6);
    let moved = point(cx) - before;
    assert!(moved.length() > 1e-4, "the point stayed: {moved:?}");
}

/// DIM-AC-ENTER / EXPR-PARSE-JUNK: in a dimension's value box, `1 in` opens
/// the unit list, and one Enter commits 1 in (25.4 mm) and closes the box, as
/// live; the `26` typed next goes nowhere near it (two Enters were needed, and
/// the `26` landed in the box: 126 in).
#[gpui::test]
fn dimension_box_enter_commits_with_the_unit_list_open(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "dim-ac", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter", "escape", "escape"]);
    let cid = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| c.value().is_some()).unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_dim_edit(cid, w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["1", "space", "i", "n"]);
    assert!(cx.update(|cx| view.read(cx).ui_ref().is_some_and(|u| u.dim_completion.is_some())), "the unit list is open");
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "one Enter closes the box");
    keys(cx, window, &["2", "6"]);
    assert!(near(&dimensions(cx, &view), &[0.0254]), "{:?}", dimensions(cx, &view));
}

/// A Part Studio with Variable `wall_thickness` = 4 mm (the user's VAR-AUTOCOMPLETE-ENTER).
fn studio_with_wall_thickness(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = part_studio(cx, name);
    cx.update(|cx| {
        let (doc, tab) = { let v = view.read(cx); (v.doc.clone(), v.tab) };
        doc.update(cx, |d, cx| {
            let units = d.session.doc.settings.units;
            d.edit_studio(tab, "Variable", cx, |ps| {
                let mut f = ps.new_feature("assignVariable", &units).unwrap();
                f.params.set("name", noshape_model::ParamValue::Text("wall_thickness".into()));
                f.params.set("variableType", noshape_model::ParamValue::Enum("LENGTH".into()));
                f.params.set("lengthValue", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("4 mm")));
                ps.features.push(f);
            });
        });
    });
    draw(cx, window);
    (window, view)
}

/// A Part Studio with length Variables `name` = `value` (in feature order).
fn studio_with_variables(cx: &mut TestAppContext, name: &str, vars: &[(&str, &str)]) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = part_studio(cx, name);
    cx.update(|cx| {
        let (doc, tab) = { let v = view.read(cx); (v.doc.clone(), v.tab) };
        doc.update(cx, |d, cx| {
            let units = d.session.doc.settings.units;
            d.edit_studio(tab, "Variable", cx, |ps| {
                for (n, v) in vars {
                    let mut f = ps.new_feature("assignVariable", &units).unwrap();
                    f.params.set("name", noshape_model::ParamValue::Text((*n).into()));
                    f.params.set("variableType", noshape_model::ParamValue::Enum("LENGTH".into()));
                    f.params.set("lengthValue", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new(*v)));
                    ps.features.push(f);
                }
            });
        });
    });
    draw(cx, window);
    (window, view)
}

const FAN_VARS: [(&str, &str); 4] = [("fan_control_board_depth", "2.4 mm"), ("fan_control_board_wall_length", "12 mm"), ("fan_clearance", "1 mm"), ("width", "30 mm")];

/// The user's hat15 report (11-13): "fancontrol" typed in a sketch dimension's box lists #fan_control_board_depth
/// and #fan_control_board_wall_length (live behaviours "Variable autocomplete in expression fields": word prefixes,
/// `#` optional); Enter inserts the highlighted one, the next Enter commits it.
#[gpui::test]
fn dimension_box_lists_variables_by_their_word_prefixes(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "dim-fan", &FAN_VARS);
    let (window, view) = sketch_in(cx, window, view, Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter", "escape", "escape"]);
    let cid = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| c.value().is_some()).unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_dim_edit(cid, w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["f", "a", "n", "c", "o", "n", "t", "r", "o", "l"]);
    let rows = cx.update(|cx| view.read(cx).ui_ref().and_then(|u| u.dim_completion.as_ref()).map(|c| c.ui_state(None)["rows"].clone()));
    assert_eq!(rows, Some(serde_json::json!(["#fan_control_board_depth", "#fan_control_board_wall_length"])));
    // Live wf37 step 25: the list hangs from the 100 px value field, right edges lined up, so it reaches left of it.
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let (f, l) = (vcx.debug_bounds("sk-dim-field").expect("the value field"), vcx.debug_bounds("var-complete").expect("the list"));
    assert_eq!((l.top() - f.bottom(), l.right() - f.right(), l.left() < f.left()), (px(0.), px(0.), true), "(list top − field bottom, right − right, starts left of the field): field {f:?}, list {l:?}");
    keys(cx, window, &["down", "down", "up", "enter"]);
    let box_text = cx.update(|cx| view.read(cx).dim_input.as_ref().map(|d| d.1.read(cx).value().to_string()));
    assert_eq!(box_text.as_deref(), Some("#fan_control_board_depth"), "↑ / ↓ don't wrap; the typed word replaced");
    keys(cx, window, &["enter"]);
    assert!(near(&dimensions(cx, &view), &[0.0024]), "{:?}", dimensions(cx, &view));
}

/// The same list in a feature dialog's number field: "fcb" in Extrude's Depth, Tab inserts, closes the list and keeps the
/// keyboard in the field.
#[gpui::test]
fn dialog_field_lists_variables_by_their_word_prefixes(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "dlg-fan", &FAN_VARS);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    wait_until(cx, window, |cx| view.read(cx).display.variables.contains_key("fan_control_board_depth"));
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["f", "c", "b"]);
    let rows = cx.update(|cx| dialog.read(cx).ui_list().map(|l| l["rows"].clone()));
    assert_eq!(rows, Some(serde_json::json!(["#fan_control_board_depth", "#fan_control_board_wall_length"])));
    keys(cx, window, &["down", "tab"]);
    assert_eq!(dialog.update(cx, |d, cx| d.field_text("depth", cx)).as_deref(), Some("#fan_control_board_wall_length"));
    // Live (green e503ca2f, fields): Tab closes the list and the keyboard stays in the field.
    assert!(cx.update(|cx| dialog.read(cx).ui_list().is_none()), "the list closed");
    keys(cx, window, &["*", "2"]);
    assert_eq!(dialog.update(cx, |d, cx| d.field_text("depth", cx)).as_deref(), Some("#fan_control_board_wall_length*2"), "typed on in the field");
}

/// VAR-AUTOCOMPLETE-ENTER (live 11-02): `#wall` + Enter in a dimension's value box inserts the highlighted
/// `#wall_thickness` and the box stays; the next Enter commits it.
#[gpui::test]
fn dimension_box_enter_inserts_the_highlighted_variable(cx: &mut TestAppContext) {
    let (window, view) = studio_with_wall_thickness(cx, "dim-wall");
    let (window, view) = sketch_in(cx, window, view, Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter", "escape", "escape"]);
    let cid = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| c.value().is_some()).unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_dim_edit(cid, w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["#", "w", "a", "l", "l", "enter"]);
    let box_text = cx.update(|cx| view.read(cx).dim_input.as_ref().map(|d| d.1.read(cx).value().to_string()));
    assert_eq!(box_text.as_deref(), Some("#wall_thickness"), "inserted, the box open");
    assert!(near(&dimensions(cx, &view), &[0.05]), "not committed yet: {:?}", dimensions(cx, &view));
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "the second Enter commits");
    assert!(near(&dimensions(cx, &view), &[0.004]), "{:?}", dimensions(cx, &view));
}

/// VAR-AUTOCOMPLETE-ENTER (live 11-02): `#wall` + Enter in Extrude's Depth inserts `#wall_thickness`, the cursor
/// after it (not selected), the dialog open; the next Enter accepts it.
#[gpui::test]
fn dialog_field_enter_inserts_the_highlighted_variable(cx: &mut TestAppContext) {
    let (window, view) = studio_with_wall_thickness(cx, "dlg-wall");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    // Opened before the regeneration defining the variable finished: the dialog's variables follow it.
    wait_until(cx, window, |cx| view.read(cx).display.variables.contains_key("wall_thickness"));
    assert!(dialog.read_with(cx, |d, _| d.variables.contains_key("wall_thickness")), "the dialog sees the variable");
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["#", "w", "a", "l", "l", "enter"]);
    assert_eq!(dialog.update(cx, |d, cx| d.field_text("depth", cx)).as_deref(), Some("#wall_thickness"));
    assert_eq!(dialog.update(cx, |d, cx| d.field_selection("depth", cx)), Some(15..15), "the cursor after the insertion");
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "the dialog stays");
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (false, false), "inserted, not applied (live ui-parity extrude-depth-variable-enter-twice)");
    assert!(cx.update(|cx| dialog.read(cx).ui_list().is_none()), "no list for the inserted text");
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "the next Enter applies it, the dialog stays");
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (true, false));
    assert_eq!(dialog.update(cx, |d, cx| d.field_selection("depth", cx)), Some(0..15), "selected");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // (A person's next Enter: not in the same instant, which is dropped.)
    cx.executor().advance_clock(std::time::Duration::from_millis(200));
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_none()), "the third accepts");
}

/// The parity suite's state (tests/ui-parity/README.md) reads the open dialog, its focused field, the autofill
/// list and the feature list as shown.
#[gpui::test]
fn the_parity_state_reads_dialog_list_and_features(cx: &mut TestAppContext) {
    let (window, view) = studio_with_wall_thickness(cx, "uip-state");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).display.variables.contains_key("wall_thickness"));
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["#", "w", "a"]);
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    assert_eq!(state["focus"]["label"], "Depth", "{}", state["focus"]);
    assert_eq!(state["focus"]["text"], "#wa");
    assert_eq!(state["focus"]["selection"], serde_json::json!([3, 3]));
    assert_eq!(state["list"]["rows"], serde_json::json!(["#wall_thickness", "New variable…"]));
    assert_eq!(state["list"]["highlighted"], 0);
    assert_eq!(state["dialog"]["title"], "Extrude 1");
    let tabs: Vec<&str> = state["dialog"]["tabs"].as_array().unwrap().iter().map(|t| t["label"].as_str().unwrap()).collect();
    assert_eq!(&tabs[..3], ["Solid", "Surface", "Thin"]);
    let depth = state["dialog"]["fields"].as_array().unwrap().iter().find(|f| f["label"] == "Depth").expect("Depth listed").clone();
    assert_eq!((depth["kind"].as_str(), depth["text"].as_str(), depth["focused"].as_bool()), (Some("number"), Some("#wa"), Some(true)));
    let rows: Vec<&str> = state["features"]["rows"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect();
    // The new Extrude's row while its dialog is open, above the bar (live).
    assert_eq!(rows, ["#wall_thickness = 4 mm", "Extrude 1"]);
    assert_eq!(state["features"]["rollbackAfter"], "Extrude 1");
}

/// Live (ui-parity feature-menu-edit-expression): Dynamic suppression's "Unsuppressed if" box lists what a number
/// field does: "tr" gives the row "true"; Enter inserts it, the next Enter applies it; Esc with the list open
/// closes only the list.
#[gpui::test]
fn dynamic_suppression_box_completes_true(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "dyn-complete", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_dynamic_suppression(id, w, cx))).unwrap();
    draw(cx, window);
    let rows = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).ui.cell_completion.as_ref().map(|c| c.entries.iter().map(|e| e.insertion().0).collect::<Vec<_>>()));
    keys(cx, window, &["t", "r"]);
    assert_eq!(rows(cx), Some(vec!["true".to_string()]), "the constant listed");
    keys(cx, window, &["escape"]);
    assert_eq!(rows(cx), None, "Esc closes the list");
    assert!(cx.update(|cx| view.read(cx).ui.dyn_suppression.is_some()), "…and leaves the box open");
    keys(cx, window, &["u"]);
    keys(cx, window, &["enter"]);
    let text = cx.update(|cx| view.read(cx).ui.dyn_suppression.as_ref().map(|d| d.1.read(cx).value().to_string()));
    assert_eq!(text.as_deref(), Some("true"), "Enter inserted the row");
    keys(cx, window, &["enter"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let expr = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.id == id).unwrap().suppression_expr.clone());
    assert_eq!(expr.as_deref(), Some("true"), "the next Enter applies it");
}

/// Live sketch-entry-1118: editing a sketch leaves a hidden later sketch hidden (nothing comes back while editing).
#[gpui::test]
fn a_hidden_later_sketch_stays_hidden_while_editing(cx: &mut TestAppContext) {
    let doc = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Front", "shapes": [{ "corner_rectangle": [[0, 0], [10, 10]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "hidden-later-sketch", Some(doc));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = |cx: &mut TestAppContext, name: &str| cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == name).unwrap().id);
    let (s1, s2) = (id(cx, "Sketch 1"), id(cx, "Sketch 2"));
    assert_eq!(cx.update(|cx| view.read(cx).display.sketches.iter().find(|s| s.feature == s2).map(|s| s.hidden)), Some(false), "Sketch 2 shown first");
    cx.update(|cx| view.update(cx, |v, cx| v.set_visibility(&[Reference::Feature(s2)], true, cx)));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let hidden = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.sketches.iter().find(|s| s.feature == s2).is_none_or(|s| s.hidden));
    assert!(hidden(cx), "Sketch 2 hidden");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(s1, w, cx))).unwrap();
    draw(cx, window);
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    assert!(hidden(cx), "Sketch 2 still hidden while Sketch 1 is edited");
}

/// Live list-1119 (p6-failed-menu.png): a feature failing because one it uses is suppressed reads red on its own icon,
/// no ⚠ for the selections it lost (Fillet 1 with Extrude 1 suppressed).
#[gpui::test]
fn a_feature_failing_under_a_suppressed_one_has_no_warning_badge(cx: &mut TestAppContext) {
    let doc = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "failed-under-suppressed", Some(doc));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = |cx: &mut TestAppContext, name: &str| cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == name).unwrap().id);
    let (extrude, fillet) = (id(cx, "Extrude 1"), id(cx, "Fillet 1"));
    cx.update(|cx| view.update(cx, |v, cx| v.set_suppressed(vec![extrude], true, cx)));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.feature_state(fillet).is_some_and(|s| s.status != noshape_model::display::FeatureStatus::Ok));
    let (status, missing, badge) = cx.update(|cx| {
        let v = view.read(cx);
        let f = v.studio(cx).unwrap().feature(fillet).unwrap().clone();
        let st = v.display.feature_state(fillet);
        (st.map(|s| s.status), st.map_or(0, |s| s.missing), v.missing_selections(&f, st, cx))
    });
    assert_eq!((status, missing > 0, badge), (Some(noshape_model::display::FeatureStatus::Error), true, 0), "(Fillet 1, it lost its edge, the selections its ⚠ counts)");
}

/// Live (ui-parity extrude-enter-commits): `20` Enter applies "20 mm" and keeps the dialog; the next Enter accepts it.
#[gpui::test]
fn enter_applies_then_the_next_enter_accepts(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "enter-enter", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["2", "0", "enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "the first Enter applies");
    // Live2 feature-dialogs-1119: an Enter in the same instant is dropped; from a 150 ms gap it commits.
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "a second Enter at once is dropped");
    keys(cx, window, &["2", "5", "enter"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.executor().advance_clock(std::time::Duration::from_millis(200));
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_none()), "the next Enter 200 ms on accepts");
}

/// Live errors-1123 (Hole depth 0, a flange length 0, a fillet radius 0): Enter on a value out of range commits
/// nothing, however often; the title keeps the last valid value and reads "…properly:  A parameter value is not
/// valid.", the row's tip while the dialog is open too.
#[gpui::test]
fn enter_on_a_value_out_of_range_commits_nothing(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "enter-out-of-range", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Fillet 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Fillet dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("radius", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["0", "enter"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    for _ in 0..2 {
        cx.executor().advance_clock(std::time::Duration::from_millis(200));
        keys(cx, window, &["enter"]);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        assert!(cx.update(|cx| view.read(cx).edit.is_some()), "Enter on radius 0 keeps the dialog open");
    }
    let (radius, status) = cx.update(|cx| {
        let d = dialog.read(cx);
        (d.row_feature().params.expr("radius").map(|e| e.text.clone()), d.title_status())
    });
    assert_eq!(radius.as_deref(), Some("2 mm"), "the title keeps the last valid radius");
    let tips = cx.update(|cx| (dialog.read(cx).range_message("radius", "0 mm"), dialog.read(cx).range_message("radius", "1000000 m")));
    assert_eq!(tips.0, "Enter a value between 0.0100 and 500000.  Current value is 0.", "the field's tip");
    assert!(tips.1.ends_with("Current value is 1000000000."), "the value whole: {}", tips.1);
    assert!(status.as_ref().is_some_and(|s| s.0 == noshape_model::display::FeatureStatus::Error && s.1.ends_with(" A parameter value is not valid.")), "{status:?}");
    // ✓ greyed and the state's title red (live errors-1123 shell-zero-n-dlg).
    let state = cx.update_window(window, |_, w, cx| dialog.read(cx).ui_state(w, cx)).unwrap();
    assert_eq!((state["ok"].as_str(), state["titleState"].as_str()), (Some("disabled"), Some("error")), "✓ and title with radius 0");
    // The row's tip while open: one space after "properly:" (live errors-1123 lp-count0).
    let row = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("feature:Fillet 1").expect("the Fillet 1 row");
    crate::ui_script::clear_tooltip();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(gpui_kit::point(row.origin.x + px(40.), row.center().y), None, Modifiers::default());
    for _ in 0..50 {
        cx.executor().advance_clock(std::time::Duration::from_millis(10));
        draw(cx, window);
    }
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Fillet 1 did not regenerate properly: A parameter value is not valid."), "the open row's tip");
    // The field's range tip 2 px right of the field, top-aligned, not under the pointer (live err-tip-placement-1123:
    // a box's tip 2 px right of it; errors-1123 fillet 0 at x 412, the field 56 px short of the dialog's edge).
    let (field, dlg) = {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        (vcx.debug_bounds("numf-radius").expect("the radius field"), vcx.debug_bounds("feature-dialog").expect("the dialog"))
    };
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(gpui_kit::point(field.origin.x + px(10.), field.center().y), None, Modifiers::default());
    for _ in 0..50 {
        cx.executor().advance_clock(std::time::Duration::from_millis(10));
        draw(cx, window);
    }
    let tip = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("placed-tooltip").expect("the range tip, placed");
    assert!((tip.origin.x - field.right() - px(2.)).abs() <= px(1.) && (tip.origin.y - field.origin.y).abs() <= px(2.), "tip {tip:?}, field {field:?}, dialog right {:?}", dlg.right());
    keys(cx, window, &["escape"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let depth = cx.update(|cx| view.read(cx).studio(cx).unwrap().feature(id).unwrap().params.expr("radius").map(|e| e.text.clone()));
    assert_eq!(depth.as_deref(), Some("2 mm"), "nothing committed");
}

/// Live (ui-parity extrude-ok-button, behaviours request 1849): a click on ✓ while Depth holds typed text only
/// applies it ("20 mm"); the dialog stays and the next ✓ accepts.
#[gpui::test]
fn a_check_click_with_typed_text_only_applies_it(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "ok-applies", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["2", "0"]);
    let ok = |cx: &mut TestAppContext| {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let b = vcx.debug_bounds("dialog-ok").expect("the ✓");
        vcx.simulate_click(b.center(), Modifiers::default());
        draw(cx, window);
    };
    ok(cx);
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "the dialog stays");
    assert_eq!(dialog.update(cx, |d, cx| d.field_text("depth", cx)).as_deref(), Some("20 mm"));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    ok(cx);
    assert!(cx.update(|cx| view.read(cx).edit.is_none()), "the second ✓ accepts");
}

/// Live 2213: ⊞ opens Add custom features on Other documents (its filters); Escape closes it (live 2361,
/// custom-features-menu-empty), and so does its ✕.
#[gpui::test]
fn add_custom_features_closes_on_escape_and_its_close_button(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "add-custom-features");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_add_custom_features(w, cx))).unwrap();
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).ui.add_features.is_some()), "the panel opened");
    let filters = cx.update(|cx| view.read(cx).ui.add_features.as_ref().map(|a| a.panel.read(cx).shows_document_filters()));
    assert_eq!(filters, Some(true), "on Other documents, its filters");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).ui.add_features.is_none()), "Escape closes it");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_add_custom_features(w, cx))).unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let b = vcx.debug_bounds("sel-close").expect("its ✕");
    vcx.simulate_click(b.center(), Modifiers::default());
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).ui.add_features.is_none()), "✕ closes it");
}

/// Live loft-surface-thin-tabs: Loft's Surface tab swaps Profiles for its same-named twin (the wire profiles'); the
/// tab click leaves nothing focused, as from any tab.
#[gpui::test]
fn a_tab_that_swaps_a_field_for_its_twin_leaves_the_keyboard_off(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "zz-loft");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("loft", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("Loft").dialog.clone());
    draw(cx, window);
    let f0 = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)["focus"].clone()).unwrap();
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| { let fh = gpui::Focusable::focus_handle(d, cx); w.focus(&fh, cx); d.keyboard_left = true; d.set_value("bodyType", noshape_model::ParamValue::Enum("SURFACE".into()), cx); })).unwrap();
    draw(cx, window);
    let f1 = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)["focus"].clone()).unwrap();
    assert_eq!((f0["kind"].as_str(), f0["label"].as_str()), (Some("field"), Some("Profiles")));
    assert_eq!(f1["kind"].as_str(), Some("none"), "{f1}");
}

/// Live analysis-zebra / -reflection / -connection: a preselected face stays selected while the tool is open; Esc
/// leaves it selected after Zebra stripes and Reflection, clears it after Connection.
#[gpui::test]
fn closing_zebra_keeps_the_selection_closing_connection_clears_it(cx: &mut TestAppContext) {
    use crate::part_studio::analysis_tools::ToolKind;
    let (window, view, top, _) = block_part(cx, "analysis-close-selection");
    for (kind, kept) in [(ToolKind::Zebra, true), (ToolKind::Reflection, true), (ToolKind::Connection, false)] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| { v.selection = vec![top.clone()]; v.toggle_analysis_tool(kind, w, cx); })).unwrap();
        draw(cx, window);
        assert_eq!(cx.update(|cx| view.read(cx).selection.len()), 1, "{kind:?} open");
        view.update(cx, |v, cx| { v.close_analysis_tool(cx); });
        draw(cx, window);
        assert_eq!(cx.update(|cx| view.read(cx).selection.len()), usize::from(kept), "{kind:?} closed");
    }
}

/// Live analysis-curve-surface (live2 11-15): Curve/surface analysis from the toolbar with a face selected opens with
/// its field empty and nothing selected; from a context menu it takes the right-clicked entity, which stays selected.
#[gpui::test]
fn curve_analysis_from_the_toolbar_drops_the_preselection(cx: &mut TestAppContext) {
    use crate::part_studio::analysis_tools::ToolKind;
    let (window, view, top, _) = block_part(cx, "curve-toolbar");
    let open = |cx: &mut TestAppContext, menu: bool| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            v.close_analysis_tool(cx);
            v.selection = vec![top.clone()];
            v.ui.analysis_menu_target = menu;
            v.toggle_analysis_tool(ToolKind::CurveSurface, w, cx);
        }))
        .unwrap();
        draw(cx, window);
        cx.update(|cx| { let v = view.read(cx); (v.selection.len(), v.ui.analysis_tool.as_ref().map(|t| t.selection.len())) })
    };
    assert_eq!(open(cx, false), (0, Some(0)), "toolbar: (selected, in the field)");
    assert_eq!(open(cx, true), (1, Some(1)), "context menu");
}

/// Live edge-menu-curve-analysis: the open Curve/surface analysis panel is recorded as live's recorder reads it (its
/// field, then each option in order, the U / V counts after their checkboxes, "Comb size" last).
#[gpui::test]
fn the_curve_analysis_panel_is_recorded(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "curve-panel-state");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| { v.selection = vec![top.clone()]; v.ui.analysis_menu_target = true; v.toggle_analysis_tool(crate::part_studio::analysis_tools::ToolKind::CurveSurface, w, cx); })).unwrap();
    draw(cx, window);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let d = &st["dialog"];
    assert_eq!(d["title"].as_str(), Some("Curve/surface analysis"), "{d}");
    let text: Vec<&str> = d["text"].as_array().unwrap().iter().filter_map(|t| t.as_str()).collect();
    assert_eq!((text.first().copied(), text.last().copied()), (Some("Edges and faces to analyze"), Some("Comb size")), "{text:?}");
    assert!(text.windows(2).any(|w| w == ["U curves", "8"]), "{text:?}");
}

/// Live 3096: the Create selection panel opened from a field's button sits 4 px right of the dialog, its top 64 px
/// under the button's (Mirror › Face mirror: button top 141, panel 205).
#[gpui::test]
fn the_create_selection_panel_opens_under_its_button(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "cs-panel-place");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("mirror", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Mirror dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("patternType", noshape_model::ParamValue::Enum("FACE".into()), cx)));
    draw(cx, window);
    wait_until(cx, window, |cx| dialog.read(cx).cs_buttons.borrow().contains_key("faces"));
    cx.update(|cx| dialog.update(cx, |_, cx| cx.emit(crate::feature_dialog::DialogEvent::CreateSelection("faces".into()))));
    draw(cx, window);
    let (button_top, panel_top, origin_y) = cx.update(|cx| {
        let v = view.read(cx);
        let b = dialog.read(cx).cs_buttons.borrow().get("faces").copied().expect("the button drawn");
        (f32::from(b.top()), f32::from(v.ui.create_selection.as_ref().expect("the panel").pos.y), f32::from(v.viewport.read(cx).origin().y))
    });
    assert_eq!(panel_top + origin_y - button_top, 64.0, "64 px under the button");
}

/// Live revolve-add-tab: the empty field a new feature's error blames stops reading red at its first pick, before the
/// preview regenerates (the state recorded straight after a pick had it red under load).
#[gpui::test]
fn a_blamed_empty_field_is_not_red_once_picked(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "picked-not-red");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("revolve", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Revolve dialog").dialog.clone());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && dialog.read(cx).field_error("entities").is_some());
    assert!(cx.update(|cx| dialog.read(cx).field_error("entities").is_some()), "empty: red");
    cx.update(|cx| dialog.update(cx, |d, cx| { d.add_pick(top.clone(), cx); }));
    assert!(cx.update(|cx| dialog.read(cx).field_error("entities").is_none()), "picked: not red, before any regeneration");
}

/// Live2 feature-dialogs-1119 (rv.png): Revolve with its face picked and no axis yet fails asking for the axis, and the
/// face stays drawn as picked, not in the failing red.
/// qa ERR-SM-BAD-SELECTION-RED (live errors-1123 sheet-metal fl-joint2): a Flange on a joint edge of Flange 1's bend
/// fails "Cannot create flange on an edge that is a joint…"; with its dialog open the joint edge is drawn red.
#[gpui::test]
fn a_flange_on_a_joint_edge_draws_the_edge_red(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "sheetMetalStart", "name": "Sheet metal model 1", "params": { "process": "THICKEN", "regions": ["Sketch 1 region 0"], "thickness": "2 mm", "radius": "2 mm" } },
        { "type": "sheetMetalFlange", "name": "Flange 1", "params": { "edges": ["edge through [20, 0, 2]"] } },
        { "type": "sheetMetalFlange", "name": "Flange 2", "params": { "edges": ["edge through [60, 25, 2]"] } },
        { "type": "sheetMetalFlange", "name": "Flange 3", "params": { "edges": ["edge through [29, 2, 2]"] } },
    ] });
    let (window, view) = part_studio_from(cx, "sm-flange-joint-red", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let flange = cx.update(|cx| view.read(cx).studio(cx).and_then(|st| st.features.iter().find(|f| f.name == "Flange 3").map(|f| f.id))).expect("Flange 3");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(flange, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).status.is_some()));
    draw(cx, window);
    let (status, missing, invalid, edges, red) = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.edit.as_ref().expect("the Flange dialog").dialog.read(cx);
        let edges: Vec<Reference> = d.feature.params.query("edges").to_vec();
        let states = &v.viewport.read(cx).scene().states;
        let red = edges.iter().all(|r| v.built.as_ref().is_some_and(|b| { let ids = b.picks.ids_for(r); !ids.is_empty() && ids.iter().all(|&id| states.get(id as usize).is_some_and(|s| s & noshape_render::scene::state::ERROR != 0)) }));
        (d.status.clone(), d.only_missing_input(), d.invalid_input, edges.len(), red)
    });
    assert!(status.as_ref().is_some_and(|(s, m)| *s == noshape_model::display::FeatureStatus::Error && m.contains("is a joint")), "{status:?}");
    assert!(edges == 1 && red, "the joint edge red: edges {edges}, red {red}, only missing {missing}, invalid input {invalid}");
}

#[gpui::test]
fn a_dialog_only_missing_an_input_draws_its_picks_unred(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "missing-input-not-red");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("revolve", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Revolve dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| { d.add_pick(top.clone(), cx); }));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.schedule_regen(w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && dialog.read(cx).status.is_some());
    draw(cx, window);
    let (missing, red) = cx.update(|cx| {
        let v = view.read(cx);
        let states = &v.viewport.read(cx).scene().states;
        let red = v.built.as_ref().is_some_and(|b| b.picks.ids_for(&top).iter().any(|&id| states.get(id as usize).is_some_and(|s| s & noshape_render::scene::state::ERROR != 0)));
        (dialog.read(cx).only_missing_input(), red)
    });
    assert_eq!((missing, red), (true, false), "(the error only asks for the axis, the face drawn red)");
}

/// Live (live2 11-15, boundary-surface-new-edit-reedit): Boundary surface's U profiles take two picks (bsurf.fs
/// "MaxNumberOfPicks" : 2); the second makes V profiles the active field.
#[gpui::test]
fn boundary_surface_u_profiles_full_pass_the_selection_on(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "bsurf-advance");
    let edges = cx.update(|cx| {
        let v = view.read(cx);
        let b = &v.context.as_ref().expect("the part").bodies[0];
        let top = b.shape.bounding_box(false).map(|bb| bb.max.z).unwrap_or(0.0);
        let on_top: Vec<usize> = b.shape.edges().iter().enumerate().filter(|(_, e)| e.edge_info().is_ok_and(|i| (i.start.z - top).abs() < 1e-7 && (i.end.z - top).abs() < 1e-7)).map(|(i, _)| i).collect();
        on_top.iter().map(|&i| Reference::Edge((*b.edges[i]).clone())).collect::<Vec<_>>()
    });
    assert!(edges.len() >= 2, "the block's top edges");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("boundarySurface", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Boundary surface dialog").dialog.clone());
    let active = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).active_query.clone());
    assert_eq!(active(cx).as_deref(), Some("uProfilesArray"));
    cx.update(|cx| dialog.update(cx, |d, cx| { d.add_pick(edges[0].clone(), cx); }));
    assert_eq!(active(cx).as_deref(), Some("uProfilesArray"), "one pick: U stays active");
    cx.update(|cx| dialog.update(cx, |d, cx| { d.add_pick(edges[2.min(edges.len() - 1)].clone(), cx); }));
    assert_eq!(active(cx).as_deref(), Some("vProfilesArray"), "two picks: V profiles active");
}

/// Live (ui-parity mirror-face-create-selection): a face picked in the open Create selection panel lights up the
/// feature that made it in the list.
#[gpui::test]
fn a_create_selection_pick_lights_up_its_feature(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "cs-related");
    let maker = top.depends_on().expect("the face's feature");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("mirror", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Mirror dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("patternType", noshape_model::ParamValue::Enum("FACE".into()), cx)));
    draw(cx, window);
    cx.update(|cx| dialog.update(cx, |_, cx| cx.emit(crate::feature_dialog::DialogEvent::CreateSelection("faces".into()))));
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).ui.create_selection.is_some()), "the panel open");
    assert!(!cx.update(|cx| view.read(cx).related_features(cx).contains(&maker)));
    view.update(cx, |v, cx| v.cs_pick(&top, noshape_scene::builder::PickKind::Face, cx));
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).related_features(cx).contains(&maker)), "the picked face's feature lit");
    // Add selection: the panel's Selection field takes the keyboard (live mirror-face-create-selection).
    let add = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("cs-add").expect("Add selection drawn").center();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(add, Modifiers::default());
    draw(cx, window);
    let focused = cx.update_window(window, |_, w, cx| view.read(cx).ui.create_selection.as_ref().is_some_and(|c| c.focus.is_focused(w))).unwrap();
    assert!(focused, "the Selection field has the keyboard");
    // Another field of the dialog activated: the panel closes, the field takes the next pick.
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.user_focus_query("mirrorPlane");
        cx.emit(crate::feature_dialog::DialogEvent::QueryFocused);
    }));
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).ui.create_selection.is_none()), "the panel closed");
}

/// Live (ui-parity extrude-remove-tab): Extrude opened on a part's face takes Add by itself, and that isn't an undo
/// step (Undo off); a change after it is, and undoing it keeps Add.
#[gpui::test]
fn what_the_dialog_fills_in_on_opening_is_no_undo_step(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "open-no-undo");
    view.update(cx, |v, _| v.selection = vec![top.clone()]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    let op = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).feature.params.enum_("operationType").to_string());
    wait_until(cx, window, |cx| dialog.read(cx).feature.params.enum_("operationType") == "ADD" && !view.read(cx).ui_busy());
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (false, false), "nothing to undo on opening");
    cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("symmetric", noshape_model::ParamValue::Bool(true), cx)));
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (true, false), "a change is a step");
    keys(cx, window, &["secondary-z"]);
    assert_eq!(op(cx), "ADD", "undo goes back to the opened dialog");
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (false, true));
}

/// Live 1990 (1.13): with the keyboard on a dialog control (a checkbox just clicked), Ctrl+Z does nothing; with it
/// elsewhere, it steps the dialog's last edit.
#[gpui::test]
fn undo_does_nothing_while_a_dialog_control_has_the_keyboard(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "undo-on-control");
    view.update(cx, |v, _| v.selection = vec![top.clone()]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("symmetric", noshape_model::ParamValue::Bool(true), cx)));
    draw(cx, window);
    let symmetric = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).feature.params.bool("symmetric"));
    cx.update_window(window, |_, w, cx| {
        let h = dialog.read(cx).tab_stop_handle("cb:symmetric").expect("the Symmetric checkbox");
        w.focus(&h, cx);
    }).unwrap();
    keys(cx, window, &["secondary-z"]);
    assert!(symmetric(cx), "the checkbox has the keyboard: nothing undone");
    cx.update_window(window, |_, w, cx| {
        let h = dialog.read(cx).focus_handle(cx);
        w.focus(&h, cx);
    }).unwrap();
    keys(cx, window, &["secondary-z"]);
    assert!(!symmetric(cx), "the keyboard elsewhere: the dialog's edit undone");
}

/// A feature row click redraws only the rows whose state changed (the clicked one): the others, and the parts list's
/// rows, keep their cached views (speed: the lists' re-render was ~18 ms of a click's frame on a big studio).
#[gpui::test]
fn a_row_click_redraws_only_the_rows_that_change(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "cPlane", "name": "Plane 1", "params": { "entities": ["plane:top"], "offset": "5 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "row-cache", Some(block));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let rows = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).row_cache.iter().map(|(id, (_, e))| (*id, e.entity_id())).collect::<std::collections::HashMap<_, _>>());
    let before = rows(cx);
    let parts = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).part_row_cache.values().map(|(_, e)| e.entity_id()).collect::<Vec<_>>());
    let parts_before = parts(cx);
    assert_eq!(parts_before.len(), 1, "the part's row is cached");
    let id = |name: &str, cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == name).unwrap().id);
    let (plane, extrude) = (id("Plane 1", cx), id("Extrude 1", cx));
    assert!(before.contains_key(&plane) && before.contains_key(&extrude), "rows cached: {before:?}");
    view.update(cx, |v, cx| v.click_feature(plane, false, cx));
    draw(cx, window);
    let after = rows(cx);
    assert_ne!(after[&plane], before[&plane], "the clicked row is drawn again");
    assert_eq!(after[&extrude], before[&extrude], "an unchanged row keeps its cached view");
    assert_eq!(parts(cx), parts_before, "the part (not made by Plane 1) keeps its row");
    assert!(cx.update(|cx| view.read(cx).selected_features == vec![plane]));
}

/// Cached feature rows keep their height in a list taller than its panel (they shrank to nothing and were drawn over
/// each other in cf-straight-spline's 63 features), and stay in place and cached when a dialog opens and previews.
#[gpui::test]
fn cached_rows_stack_in_a_long_list(cx: &mut TestAppContext) {
    let planes: Vec<serde_json::Value> = (1..=120).map(|i| serde_json::json!({ "type": "cPlane", "name": format!("Plane {i}"), "params": { "entities": ["plane:top"], "offset": format!("{i} mm") } })).collect();
    let (window, view) = part_studio_from(cx, "row-stack", Some(serde_json::json!({ "units": "mm", "features": planes })));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let ids: Vec<_> = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().map(|f| f.id).collect());
    let tops = |cx: &mut TestAppContext| {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        ids.iter().filter_map(|id| vcx.debug_bounds(Box::leak(format!("feature-row-{id:?}").into_boxed_str())).map(|b| (*id, f32::from(b.origin.y)))).collect::<Vec<_>>()
    };
    let shown = tops(cx);
    assert!(shown.len() >= 5, "the rows in view are cached: {shown:?}");
    for w in shown.windows(2) {
        assert!((w[1].1 - w[0].1 - crate::ui::ROW_H).abs() < 0.5, "one row below the other: {shown:?}");
    }
    let cached = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).row_cache.iter().map(|(id, (_, e))| (*id, e.entity_id())).collect::<std::collections::HashMap<_, _>>());
    let before = cached(cx);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(ids[119], w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let after = cached(cx);
    let kept = before.iter().filter(|(id, e)| after.get(id) == Some(e)).count();
    assert!(kept + 1 >= before.len(), "the other rows keep their cached views through the dialog's preview: {kept} of {}", before.len());
    let now = tops(cx);
    assert!(now.len() >= 5, "{now:?}");
    for (id, y) in &now {
        assert_eq!(shown.iter().find(|(i, _)| i == id).map(|r| r.1), Some(*y), "the other rows stay put: {now:?}");
    }
}

/// Live (behaviours request 1839): Enter on a selected feature row opens nothing, and ↑ after the row click doesn't
/// move the rollback bar; once the bar is clicked ↑ steps it.
#[gpui::test]
fn row_keys_leave_the_feature_and_the_bar(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "row-keys", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.click_feature(id, false, cx);
        w.focus(&v.ui.list_focus, cx);
    }))
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["enter", "up"]);
    let rollback = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).unwrap().rollback);
    assert!(cx.update(|cx| view.read(cx).edit.is_none()), "no dialog");
    assert_eq!(rollback(cx), None, "the bar stays at the end");
    // A click on the bar (its on_click).
    view.update(cx, |v, _| v.ui.rollback_keyboard = Some(v.selected_features.clone()));
    keys(cx, window, &["up"]);
    assert_eq!(rollback(cx), Some(1), "above Extrude 1");
    // The sketch the bar now sits under shows, though it is hidden (live: behaviours request 1844).
    let sketch = cx.update(|cx| view.read(cx).studio(cx).unwrap().features[0].clone());
    // (Its auto-hide counts only consumers above the bar: live rollback-menus 11-21, its row's menu has Hide.)
    assert!(!sketch.hidden, "the extruded sketch shown with its Extrude rolled back");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.sketches.iter().any(|s| s.feature == sketch.id));
    assert!(cx.update(|cx| view.read(cx).display.sketches.iter().all(|s| !s.hidden)), "drawn");
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    assert_eq!(state["features"]["rows"][0]["hidden"], false, "{}", state["features"]);
}

/// Live (ui-parity new-extrude-then-other-tool, new-variable-enter-then-dblclick-other): another tool, or a double
/// click on another feature, while a new feature's dialog is open keeps that feature as ✓ would.
#[gpui::test]
fn another_tool_keeps_the_open_feature(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "other-tool", Some(block));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let names = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().map(|f| f.name.clone()).collect::<Vec<_>>());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("assignVariable", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Variable dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("name", noshape_model::ParamValue::Text("v1".into()), cx)));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let extrude = cx.update(|cx| view.read(cx).studio(cx).unwrap().features[1].id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(extrude, w, cx))).unwrap();
    assert_eq!(names(cx).len(), 3, "the variable kept: {:?}", names(cx));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("cPlane", w, cx))).unwrap();
    assert_eq!(cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.type_id.clone())).as_deref(), Some("cPlane"));
    assert_eq!(names(cx).len(), 3, "the unchanged Extrude 1 as it was: {:?}", names(cx));
}

/// Live (ui-parity shortcut-plane-shift-enter): Shift+Enter in Depth with "20" typed applies it and keeps the
/// dialog; the next Shift+Enter accepts and opens the next one.
#[gpui::test]
fn shift_enter_with_typed_text_applies_it_first(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "shift-enter", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["2", "0", "shift-enter"]);
    assert_eq!(cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.id)), Some(id), "the dialog stays");
    assert_eq!(dialog.update(cx, |d, cx| d.field_text("depth", cx)).as_deref(), Some("20 mm"));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.executor().advance_clock(std::time::Duration::from_millis(200));
    keys(cx, window, &["shift-enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.as_ref().is_some_and(|e| e.is_new)), "accepted, a new Extrude open");
}

/// Live (ui-parity variable-edit-focus): an edited Variable opens with the keyboard in Value, its text selected.
#[gpui::test]
fn an_edited_variable_opens_in_its_value(cx: &mut TestAppContext) {
    let (window, view) = studio_with_wall_thickness(cx, "var-edit-focus");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.type_id == "assignVariable").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    assert_eq!((state["focus"]["label"].as_str(), state["focus"]["all"].as_bool()), (Some("Value"), Some(true)), "{}", state["focus"]);
}

/// Live 11-03 (ui-parity feature-row-copy-paste; psui's probe in its own tab: Onshape handles neither key on a
/// feature row and has no feature paste): Ctrl+C on a selected feature row, then Ctrl+V, adds nothing.
#[gpui::test]
fn ctrl_c_ctrl_v_on_a_feature_row_add_nothing(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "row-copy", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features[1].id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.click_feature(id, false, cx);
        w.focus(&v.ui.list_focus, cx);
    }))
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["ctrl-c", "ctrl-v"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!(cx.update(|cx| view.read(cx).studio(cx).unwrap().features.len()), 2, "nothing pasted");
}

/// Live 1863 (ui-parity shortcut-shift-p-planes): Shift+P hides the origin, planes and sketches as an undo step; a
/// second press isn't a toggle; Ctrl+Z brings back exactly what was hidden (a sketch hidden before stays hidden).
/// (Live records the second press as an empty step of its own; our store keeps no step that changes nothing.)
#[gpui::test]
fn shift_p_hides_construction_one_undo_step_a_press(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "shift-p", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let shown = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.planes.iter().filter(|p| !p.hidden).count());
    let can_undo = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).doc.read(cx).can_undo());
    assert_eq!((shown(cx), can_undo(cx)), (3, false));
    let shift_p = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.hide_construction(w, cx))).unwrap();
    shift_p(cx);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!((shown(cx), can_undo(cx)), (0, true), "hidden, a step");
    shift_p(cx);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!(shown(cx), 0, "not a toggle");
    let undo = |cx: &mut TestAppContext| {
        cx.update(|cx| view.read(cx).doc.clone()).update(cx, |d, cx| d.undo(cx));
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    };
    undo(cx);
    assert_eq!(shown(cx), 3, "the planes back");
    assert!(cx.update(|cx| view.read(cx).studio(cx).unwrap().features[0].hidden), "the extruded sketch stays hidden");
}

/// Live (ui-parity extrude-depth-variable-escape, 11-03): Esc with the autofill list open closes the list only;
/// `#wall` and the focus stay in Depth (ours reverted the field and left it).
#[gpui::test]
fn escape_closes_only_the_autofill_list(cx: &mut TestAppContext) {
    let (window, view) = studio_with_wall_thickness(cx, "esc-list");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| view.read(cx).display.variables.contains_key("wall_thickness"));
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["#", "w", "a", "l", "l"]);
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    assert!(state["list"].is_object(), "the list is open");
    keys(cx, window, &["escape"]);
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    assert!(state["list"].is_null(), "{}", state["list"]);
    assert_eq!((state["focus"]["label"].as_str(), state["focus"]["text"].as_str()), (Some("Depth"), Some("#wall")), "{}", state["focus"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_some()), "the dialog stays");
}

/// Live (ui-parity number-field-bare-number, 11-03): `15` + Enter in Depth commits "15 mm", selected, and no
/// autofill list opens for its unit.
#[gpui::test]
fn a_committed_number_opens_no_list(cx: &mut TestAppContext) {
    let (window, view) = studio_with_wall_thickness(cx, "commit-no-list");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["1", "5", "enter"]);
    draw(cx, window);
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    assert_eq!(state["focus"]["text"], "15 mm", "{}", state["focus"]);
    assert_eq!(state["focus"]["all"], true);
    assert!(state["list"].is_null(), "{}", state["list"]);
}

/// VAR-AUTOCOMPLETE-ENTER (live behaviours "Focus, click and selection"): the first click into a dialog's number
/// field selects all its text, so typing replaces it; the second click puts the caret.
#[gpui::test]
fn a_first_click_into_a_number_field_selects_all(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "click-select");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    // The window active, as the user's is (focus events need it).
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let b = vcx.debug_bounds("numf-depth").expect("the Depth field");
    let at = gpui_kit::point(b.origin.x + b.size.width - gpui_kit::px(30.), b.center().y);
    vcx.simulate_click(at, Modifiers::default());
    draw(cx, window);
    let text = dialog.update(cx, |d, cx| d.field_text("depth", cx)).unwrap();
    assert_eq!(dialog.update(cx, |d, cx| d.field_selection("depth", cx)), Some(0..text.len()), "the first click selects {text:?}");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_click(at, Modifiers::default());
    draw(cx, window);
    let sel = dialog.update(cx, |d, cx| d.field_selection("depth", cx)).unwrap();
    assert!(sel.is_empty(), "the second click puts the caret: {sel:?}");
}

/// A line drawn and its value box dismissed (Esc), then the Dimension tool.
fn line_then_dimension_tool(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = sketch_with(cx, name, Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape", "escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    draw(cx, window);
    (window, view)
}

/// Live session-1118 2d (retaken 3 of 3): in the iso view a rectangle's bottom line picked with the Dimension tool and a
/// placement click 8 mm under it places the dimension and opens its box.
#[gpui::test]
fn a_dimension_placed_under_a_line_in_iso_opens_its_box(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "dim-iso-place", Tool::CornerRect);
    let slant = cx.update(|cx| { let v = view.read(cx); v.viewport.read(cx).camera.forward().dot(v.sketch.as_ref().unwrap().frame.normal).abs() });
    assert!(slant < 0.9, "the view is iso, not normal to Top: {slant}");
    let at = |cx: &mut TestAppContext, x: f64, y: f64| { let p = on_screen(cx, &view, DVec2::new(x, y)); (p.x, p.y) };
    let (a, b) = (at(cx, 0.01, 0.008), at(cx, 0.09, 0.058));
    click(cx, window, &view, a);
    click(cx, window, &view, b);
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    draw(cx, window);
    let (line, place) = (at(cx, 0.05, 0.008), at(cx, 0.05, 0.0));
    click(cx, window, &view, line);
    click(cx, window, &view, place);
    let dims = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().filter(|c| c.kind.is_dimension()).count());
    let text = cx.update(|cx| view.read(cx).dim_input.as_ref().map(|(_, st, _)| st.read(cx).value().to_string()));
    assert_eq!((dims, text.as_deref()), (1, Some("80 mm")), "placed, its box open with the width");
}

/// Live (ui-parity sketch-dimension-variable, qa SKETCH-DIM-PLACE-CLICK-SELECTS): on Top a rectangle's bottom line lies
/// in the Front plane; with it picked, a placement click 8 mm under it, over the Front plane, places its dimension (the
/// box open for "#width"), not a second pick of the plane the line lies in, which doesn't light while placing. The pick
/// alone reads its Length, as live's.
#[gpui::test]
fn a_dimension_placed_over_the_plane_its_line_lies_in_opens_its_box(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "dim-place-own-trace", Tool::CornerRect);
    let at = |cx: &mut TestAppContext, x: f64, y: f64| { let p = on_screen(cx, &view, DVec2::new(x, y)); (p.x, p.y) };
    let (a, b) = (at(cx, 0.06, 0.0), at(cx, 0.09, 0.02));
    click(cx, window, &view, a);
    click(cx, window, &view, b);
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    draw(cx, window);
    let (line, place) = (at(cx, 0.075, 0.0), at(cx, 0.075, -0.008));
    click(cx, window, &view, line);
    let units = cx.update(|cx| view.read(cx).doc.read(cx).session.doc.settings.units);
    let readout = cx.update(|cx| { let v = view.read(cx); v.sketch.as_ref().unwrap().selection_readout(&v.sketch_data(cx).unwrap(), &units) });
    assert_eq!(readout.iter().map(|(l, t)| (l.as_str(), t.as_str())).collect::<Vec<_>>(), [("Length", "30.000 mm")], "the picked line's readout, as live's");
    // The Front plane under the pointer, as the GPU's pick gives it there.
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let (vp, at) = (v.viewport.clone(), DVec2::new(place.0, place.1));
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
            v.hover = Some(Reference::Plane(DatumPlane::Front));
            // Placing over it, the plane doesn't light (live qa walk 1 w3-placing, qa FEEL-PLANE-LIT-WHILE-DIMENSIONING).
            assert!(v.highlighted_hover(cx).is_empty(), "the Front plane lit while the dimension is placed over it");
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    let dims = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().filter(|c| c.kind.is_dimension()).count());
    let text = cx.update(|cx| view.read(cx).dim_input.as_ref().map(|(_, st, _)| st.read(cx).value().to_string()));
    assert_eq!((dims, text.as_deref()), (1, Some("30 mm")), "placed, its box open with the width, the plane not picked");
}

/// qa P0 FEEL-DIM-PICK-READOUT (live cov2-dim-orient, R3): the Dimension tool's picks fill the bottom-right readout,
/// "Point" after the first, "Min dist" after the second, kept while the pointer goes to place it; gone once placed.
#[gpui::test]
fn the_dimension_tools_picks_fill_the_readout(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "dim-readout");
    let readout = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let (data, units) = (v.sketch_data(cx).unwrap(), v.doc.read(cx).session.doc.settings.units);
            v.sketch.as_ref().unwrap().selection_readout(&data, &units).into_iter().map(|(label, _)| label).collect::<Vec<_>>()
        })
    };
    let mut seen = Vec::new();
    click(cx, window, &view, (500., 400.));
    seen.push(readout(cx));
    click(cx, window, &view, (640., 360.));
    seen.push(readout(cx));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        let vp = v.viewport.clone();
        v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: DVec2::new(560., 300.), modifiers: Modifiers::default() }, w, cx);
    }))
    .unwrap();
    draw(cx, window);
    seen.push(readout(cx));
    click(cx, window, &view, (560., 300.));
    seen.push(readout(cx));
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(seen, vec![s(&["Point"]), s(&["Min dist"]), s(&["Min dist"]), Vec::new()], "the readout after the first pick, the second, moving to place it, placed");
}

/// Live feel round 2 R3 (qa FEEL-DIM-TWO-POINTS-ZERO): a line from the origin to (30, 20); the Dimension tool's quick
/// clicks on its two ends, the hover still the line from where the pointer was, take the ends, and the dimension placed
/// above-left reads their horizontal distance (was the line and its own end: "0 mm").
#[gpui::test]
fn a_dimension_between_a_lines_ends_clicked_quickly_reads_their_distance(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "dim-two-points", Tool::Line);
    let at = |cx: &mut TestAppContext, x: f64, y: f64| { let p = on_screen(cx, &view, DVec2::new(x, y)); (p.x, p.y) };
    let (a, b, place) = (at(cx, 0.0, 0.0), at(cx, 0.03, 0.02), at(cx, 0.005, 0.025));
    click(cx, window, &view, a);
    click(cx, window, &view, b);
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    draw(cx, window);
    let line = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.sketch_data(cx).unwrap();
        let e = d.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
        Reference::SketchEntity { sketch: v.sketch.as_ref().unwrap().feature, entity: e.0 }
    });
    for p in [a, b] {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let (vp, at) = (v.viewport.clone(), DVec2::new(p.0, p.1));
                v.hover = Some(line.clone());
                v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
                v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() }, w, cx);
            });
        })
        .unwrap();
        draw(cx, window);
    }
    let picks = cx.update(|cx| { let v = view.read(cx); let d = v.sketch_data(cx).unwrap(); v.sketch.as_ref().unwrap().tool_picks().iter().map(|e| d.sketch.kind(*e)).collect::<Vec<_>>() });
    assert_eq!(picks, [Some(noshape_sketch::EntityKind::Point); 2], "both ends picked, not the line");
    let origin = cx.update(|cx| { let v = view.read(cx); let d = v.sketch_data(cx).unwrap(); v.sketch.as_ref().unwrap().tool_picks().iter().any(|e| crate::sketch_mode::is_origin(&d.sketch, *e)) });
    assert!(!origin, "the line's start, not the Origin under it (live sk-1 dim-hv-two-points)");
    // (The ends where the clicks' pixels put them, about 30 mm apart across: live's 29.548 mm.)
    let across = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        let (a, b) = d.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).and_then(|e| d.sketch.endpoints(e.id)).unwrap();
        (d.sketch.point(b).unwrap().x - d.sketch.point(a).unwrap().x).abs() * 1e3
    });
    click(cx, window, &view, place);
    let text = cx.update(|cx| view.read(cx).dim_input.as_ref().map(|(_, st, _)| st.read(cx).value().to_string()));
    let value: f64 = text.as_deref().and_then(|t| t.strip_suffix(" mm")).and_then(|t| t.parse().ok()).unwrap_or(0.0);
    assert!((value - across).abs() < 1e-3 && (across - 30.0).abs() < 1.0, "the ends' horizontal distance {across} mm: {text:?}");
}

/// Live2 11-17 sketch-dim-sel: with the Dimension tool a pick previews the dimension at once (the line kept selected);
/// the placement click creates it and opens its box, the measured value with its unit pre-selected; Esc in the box
/// keeps the dimension, the tool on; "1abc" Enter keeps the box (invalid), the first Esc restores the measured text
/// with the box open, the second closes it; Esc after a pick drops it and ends the tool; Delete on a rectangle side
/// takes the line and every constraint on it, the rest staying.
#[gpui::test]
fn the_dimension_tool_flow_as_lives(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "dim-flow", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (650., 420.));
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    draw(cx, window);
    let dims = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().filter(|c| c.kind.is_dimension()).count());
    let tool = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().map(|ed| ed.tool));
    let pointer = |cx: &mut TestAppContext, at: (f64, f64)| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: DVec2::new(at.0, at.1), modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
    };
    // The sides' middles on screen: top, left, bottom, right.
    let mids: Vec<(f64, f64)> = {
        let m = cx.update(|cx| {
            let d = view.read(cx).sketch_data(cx).unwrap();
            let ms: Vec<DVec2> = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).filter_map(|e| { let (a, b) = d.sketch.endpoints(e.id)?; Some((d.sketch.point(a)? + d.sketch.point(b)?) * 0.5) }).collect();
            let by = |f: &dyn Fn(&DVec2) -> f64| *ms.iter().max_by(|a, b| f(a).total_cmp(&f(b))).unwrap();
            [by(&|p| p.y), by(&|p| -p.x), by(&|p| -p.y), by(&|p| p.x)]
        });
        m.iter().map(|q| { let p = on_screen(cx, &view, *q); (p.x, p.y) }).collect()
    };
    let off = |p: (f64, f64), dx: f64, dy: f64| (p.0 + dx, p.1 + dy);
    let input = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).dim_input.as_ref().map(|(_, st, _)| { let s = st.read(cx); (s.value().to_string(), s.selected_range()) }));
    // A pick, the preview following the pointer, the placement.
    click(cx, window, &view, mids[0]);
    pointer(cx, off(mids[0], 0., -40.));
    let preview = cx.update(|cx| { let v = view.read(cx); v.sketch.as_ref().unwrap().dimension_preview(&v.sketch_data(cx).unwrap()).is_some() });
    let state = cx.update(|cx| { let v = view.read(cx); let ed = v.sketch.as_ref().unwrap(); (ed.tool, ed.tool_picks(), ed.selection.clone(), v.sketch_data(cx).unwrap().sketch.entities().count()) });
    assert!(preview && dims(cx) == 0, "the pick previews (nothing added yet): {state:?}");
    click(cx, window, &view, off(mids[0], 0., -40.));
    let (text, sel) = input(cx).expect("the placed dimension's box");
    assert!(dims(cx) == 1 && text.ends_with(" mm") && sel == (0..text.len()), "created at once, its box the measured value pre-selected: {text:?} {sel:?}");
    keys(cx, window, &["escape"]);
    assert!(input(cx).is_none() && dims(cx) == 1 && tool(cx) == Some(Tool::Dimension), "Esc in the box: the dimension kept, the tool on");
    // "1abc" Enter, Esc, Esc.
    click(cx, window, &view, mids[1]);
    pointer(cx, off(mids[1], -40., 0.));
    click(cx, window, &view, off(mids[1], -40., 0.));
    let measured = input(cx).expect("the second box").0;
    keys(cx, window, &["1", "a", "b", "c", "enter"]);
    let got = (input(cx), cx.update(|cx| view.read(cx).ui.dim_invalid), cx.update(|cx| view.read(cx).message.clone()));
    assert!(got.0.as_ref().is_some_and(|t| t.0 == "1abc") && got.1, "\"1abc\" Enter: the box stays, invalid: {got:?}");
    keys(cx, window, &["escape"]);
    assert_eq!(input(cx).map(|t| t.0), Some(measured.clone()), "the first Esc restores the measured text, the box open");
    keys(cx, window, &["escape"]);
    assert!(input(cx).is_none() && dims(cx) == 2 && tool(cx) == Some(Tool::Dimension), "the second Esc closes it, the dimension kept");
    // Esc after a pick: dropped, the tool off.
    click(cx, window, &view, mids[2]);
    keys(cx, window, &["escape"]);
    assert!(dims(cx) == 2 && tool(cx) == Some(Tool::Select), "Esc before placing: nothing added, the tool off: {:?}", tool(cx));
    // Delete on the right side: it and every constraint on it go.
    let (right, before, on_it) = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.sketch_data(cx).unwrap();
        let right = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).max_by(|a, b| {
            let mx = |e: &noshape_sketch::Entity| d.sketch.curve(e.id).map_or(0.0, |c| c.point(0.5).x);
            mx(a).total_cmp(&mx(b))
        }).unwrap().id;
        let (pa, pb) = d.sketch.endpoints(right).unwrap();
        // (The corners stay with the other sides, and so do the constraints on them.)
        let _ = (pa, pb);
        let on = d.sketch.constraints().filter(|c| c.kind.entities().contains(&right)).count();
        (right, d.sketch.constraints().count(), on)
    });
    click(cx, window, &view, mids[3]);
    keys(cx, window, &["delete"]);
    let (gone, after) = cx.update(|cx| { let d = view.read(cx).sketch_data(cx).unwrap(); (d.sketch.entity(right).is_none(), d.sketch.constraints().count()) });
    assert!(gone && after == before - on_it, "Delete: the line and its {on_it} constraints go, the rest stays ({before} -> {after})");
}

/// qa 11-17 SKSNAP-REACH (live 11-17 sketch-snap-1117 row 1): another sketch's point takes a line's end from 6–7 px
/// along x (its 15 px square), with no hover of it (the GPU's point pick reaches 5 px).
#[gpui::test]
fn another_sketchs_point_snaps_from_its_square(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "other-sketch-point", Tool::Point);
    click(cx, window, &view, (700., 250.));
    keys(cx, window, &["escape"]);
    let q = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).filter_map(|e| d.sketch.point(e.id)).last().unwrap()
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).sketch.is_none());
    let (window, view) = sketch_in(cx, window, view, Tool::Line);
    click(cx, window, &view, (450., 400.));
    let wpp = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().world_per_px);
    let at = |cx: &mut TestAppContext, dx: f64| {
        let p = on_screen(cx, &view, q + DVec2::new(dx * wpp, 0.0));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: p, modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
        cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().inference.as_ref().is_some_and(|i| (i.position - q).length() < 1e-7))
    };
    assert!(!at(cx, 12.0), "12 px off: free");
    // Live 6-7 px (qa 11-18 SKSNAP-REACH): 7 px lights coming from outside.
    assert!(at(cx, 7.0), "7 px along x: on the point");
    assert!(!at(cx, 12.0), "12 px off: free again");
    assert!(at(cx, 6.5), "6.5 px along x: on the point");
    // Live 11-18 use-glyph: the click makes one Coincident to the point and uses nothing: no use glyph, the point not
    // drawn in this sketch.
    let p = on_screen(cx, &view, q + DVec2::new(6.5 * wpp, 0.0));
    click(cx, window, &view, (p.x, p.y));
    let (coincident, used, uses) = cx.update(|cx| {
        let v = view.read(cx);
        let (d, ed) = (v.sketch_data(cx).unwrap(), v.sketch.as_ref().unwrap());
        let ext = |e: noshape_sketch::EntityId| d.sketch.entity(e).is_some_and(|x| x.external);
        let coincident = d.sketch.constraints().filter(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Coincident { a, b } if ext(a) || ext(b))).count();
        let used = d.sketch.entities().filter(|e| e.external && !e.reference_only && e.kind() == noshape_sketch::EntityKind::Point).count();
        (coincident, used, ed.constraint_glyphs(&d).iter().filter(|g| g.icon == "use").count())
    });
    assert_eq!((coincident, used, uses), (1, 0, 0), "(a Coincident to the point, used points, use glyphs)");
}

/// lead 11-18 (the user's hat15b, Sketch 1: its used point stored 1.07 mm off the vertex it names): a model vertex on the
/// sketch plane snaps to the vertex's own point, not the display mesh's f32 copy of it (34.749999642 for 34.75 mm), and
/// the point the click stores is that point.
#[gpui::test]
fn a_model_vertex_snaps_to_its_exact_point(cx: &mut TestAppContext) {
    let (window, view, _top, _bottom) = block_part(cx, "model-vertex-exact");
    let (_window, view) = sketch_in(cx, window, view, Tool::Line);
    // A corner of the block on the sketch plane, its exact point and its name.
    let (r, exact) = cx.update(|cx| {
        let v = view.read(cx);
        let frame = v.sketch.as_ref().unwrap().frame;
        let c = v.context.as_ref().unwrap();
        let b = c.bodies.iter().find(|b| b.vertices.len() == 8).expect("the block");
        b.vertices
            .iter()
            .enumerate()
            .filter_map(|(i, n)| Some((Reference::Vertex((**n).clone()), b.shape.sub_shape(noshape_kernel::ShapeType::Vertex, i).ok()?.vertex_point().ok()?)))
            .find(|(_, w)| (frame.to_world(frame.to_local(*w)) - *w).length() < 1e-9)
            .map(|(r, w)| (r, frame.to_local(w)))
            .expect("a corner on the plane")
    });
    let snap = cx.update(|cx| {
        let v = view.read(cx);
        let wpp = v.sketch.as_ref().unwrap().world_per_px;
        v.model_snap_of(&r, exact + DVec2::new(3.0 * wpp, 2.0 * wpp), v.viewport.read(cx))
    });
    let Some(crate::sketch_mode::ExternalSnap::Vertex { position, .. }) = snap else { panic!("a vertex snap: {snap:?}") };
    assert!((position - exact).length() < 1e-12, "the vertex's own point: {position} vs {exact}");
}

/// qa 11-17 SKSNAP-MODEL-EDGE (live 11-17 sketch-snap-1117 rows 1, 3): in a new sketch on Top under a block, the
/// block's edge on the plane takes a line's end 2 px off it (COINCIDENT with the edge used) and offers its midpoint at
/// its middle, with no hover of it (in a normal view the block's top edge is drawn over it and is the one lit).
#[gpui::test]
fn a_model_edge_on_the_sketch_plane_snaps_unhovered(cx: &mut TestAppContext) {
    let (window, view, _top, _bottom) = block_part(cx, "model-edge-snap");
    let (window, view) = sketch_in(cx, window, view, Tool::Line);
    let edge = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().unwrap();
        super::model_straight_edges(&v.display, &ed.frame).into_iter().max_by(|a, b| ((a.1 + a.2).x).total_cmp(&(b.1 + b.2).x)).map(|(_, a, z)| (a, z)).expect("an edge on the plane")
    });
    let (a, z) = edge;
    let pointer = |cx: &mut TestAppContext, q: DVec2| {
        let at = on_screen(cx, &view, q);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
        at
    };
    let inference = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().inference.as_ref().map(|i| (i.kind, i.position)));
    let wpp = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().world_per_px);
    // (Clear of the level's 7 px band at the middle, as live's start: a level crossing the edge there is taken instead.)
    let start = on_screen(cx, &view, (a + z) * 0.5 + DVec2::new(0.02, 20.0 * wpp));
    click(cx, window, &view, (start.x, start.y));
    let mid = (a + z) * 0.5;
    pointer(cx, mid + DVec2::new(3.0 * wpp, 0.0));
    let at_mid = inference(cx);
    assert!(at_mid.is_some_and(|(k, q)| k == noshape_sketch::InferenceKind::Midpoint && (q - mid).length() < 1e-9), "the edge's middle: its midpoint, {at_mid:?}");
    let off = a + (z - a) * 0.3 + DVec2::new(2.0 * wpp, 0.0);
    let at = pointer(cx, off);
    let on = inference(cx);
    assert!(on.is_some_and(|(_, q)| (q.x - a.x).abs() < 1e-7), "2 px off the edge: on it, {on:?}");
    click(cx, window, &view, (at.x, at.y));
    let (end, coincident) = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        let line = d.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.external).map(|e| e.id).unwrap();
        let (_, b) = d.sketch.endpoints(line).unwrap();
        let used: Vec<noshape_sketch::EntityId> = d.sketch.entities().filter(|e| e.external).map(|e| e.id).collect();
        (d.sketch.point(b).unwrap(), d.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Coincident { a: p, b: q } if (p == b && used.contains(&q)) || (q == b && used.contains(&p)))))
    });
    // (Within the edge's f32 mesh precision.)
    assert!((end.x - a.x).abs() < 1e-7 && coincident, "the end on the edge (x {}), coincident with it: {end:?}, {coincident}", a.x);
}

/// User P0 11-12 (hat15b, 5th report): a line's end over a point, hovered, then clicked: the end is the point.
/// Live (behaviours "sketch snapping and hover"): a point takes the end from 8–9 px (a line from 7); here 7.8 px.
fn a_line_ends_on_the_hovered_point(cx: &mut TestAppContext, scale: f32) {
    let (window, view) = sketch_with(cx, &format!("hover-snap-{scale}"), Tool::Point);
    cx.update_window(window, |_, w, _| w.set_scale_factor(scale)).unwrap();
    draw(cx, window);
    // (Off the origin's horizontal and vertical, which it would snap to.)
    click(cx, window, &view, (700., 250.));
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    draw(cx, window);
    let found = |cx: &gpui_kit::App| {
        let v = view.read(cx);
        let data = v.sketch_data(cx)?;
        // (The placed point: the first is the origin's hidden one.)
        let point = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).last()?.id;
        let r = Reference::SketchEntity { sketch: v.sketch.as_ref()?.feature, entity: point.0 };
        Some((point, v.built.as_ref()?.picks.ids_for(&r).first().copied()?))
    };
    wait_until(cx, window, |cx| found(cx).is_some());
    let (point, pick) = cx.update(|cx| found(cx)).expect("the point is pickable");
    click(cx, window, &view, (450., 170.));
    let at = DVec2::new(706., 255.);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            vp.update(cx, |vp, _| vp.place_pointer(at));
            for ev in [
                ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() },
                // The GPU's hover pick, after the move.
                ViewportEvent::Hover(Some(pick)),
                ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 },
                ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() },
            ] {
                v.on_viewport_event(&vp, &ev, w, cx);
            }
        });
    })
    .unwrap();
    draw(cx, window);
    let (ends, at_point, coincident) = cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).unwrap();
        let ends = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).filter_map(|e| e.endpoints()).collect::<Vec<_>>();
        let end = ends.first().map(|e| e.1);
        let at_point = end.and_then(|e| Some((data.sketch.point(e)? - data.sketch.point(point)?).length()));
        let coincident = end.is_some_and(|e| e == point || data.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Coincident { a, b } if (a, b) == (e, point) || (a, b) == (point, e))));
        (ends, at_point, coincident)
    });
    assert_eq!(ends.len(), 1, "one line: {ends:?}");
    assert!(coincident && at_point.is_some_and(|d| d < 1e-12), "the line ends on the hovered point (scale {scale}): {at_point:?} off, coincident {coincident}");
}

#[gpui::test]
fn a_line_ends_on_the_hovered_point_at_scale_1(cx: &mut TestAppContext) {
    a_line_ends_on_the_hovered_point(cx, 1.0);
}

#[gpui::test]
fn a_line_ends_on_the_hovered_point_at_scale_2(cx: &mut TestAppContext) {
    a_line_ends_on_the_hovered_point(cx, 2.0);
}

/// Live (behaviours "sketch snapping and hover", 11-12): a point dragged to 7 px off a line lands on it, coincident;
/// at 9 px it stays where it is dropped.
#[gpui::test]
fn a_dragged_point_lands_on_a_line_within_7_px(cx: &mut TestAppContext) {
    for (off, lands) in [(7.0, true), (9.0, false)] {
        let (window, view) = sketch_with(cx, &format!("drag-onto-line-{off}"), Tool::Line);
        click(cx, window, &view, (450., 150.));
        click(cx, window, &view, (750., 150.));
        keys(cx, window, &["escape", "escape"]);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Point, w, cx))).unwrap();
        draw(cx, window);
        click(cx, window, &view, (620., 250.));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
        draw(cx, window);
        drag(cx, window, &view, (620., 250.), (620., 150. + off), 8);
        let (on_line, coincident, dbg) = cx.update(|cx| {
            let data = view.read(cx).sketch_data(cx).unwrap();
            let line = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
            let point = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).max().unwrap();
            let q = data.sketch.point(point).unwrap();
            let on_line = data.sketch.curve(line).unwrap().closest(q).2 < 1e-12;
            let coincident = data.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Coincident { a, b } if (a, b) == (point, line) || (a, b) == (line, point)));
            (on_line, coincident, format!("{q:?} {:?} {:?}", data.sketch.curve(line), view.read(cx).sketch.as_ref().map(|e| e.world_per_px)))
        });
        assert_eq!((on_line, coincident), (lands, lands), "a point dropped {off} px off the line: {dbg}");
    }
}

/// Live2 "feel (sketch)" (3): in a sketch with no tool, Ctrl+click on an entity opens the sketch's context menu (as a
/// right-click) and doesn't toggle the selection.
#[gpui::test]
fn ctrl_click_in_a_sketch_opens_its_context_menu(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "ctrl-click-menu", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (700., 400.));
    keys(cx, window, &["escape", "escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    let at = DVec2::new(600., 400.);
    let ctrl = Modifiers { control: true, ..Default::default() };
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: ctrl }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: ctrl, click_count: 1 }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: at, modifiers: ctrl }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).context_menu.is_some()), "the sketch's context menu opened");
    assert!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().selection.is_empty()), "the selection didn't toggle");
}

/// ui-parity sketch-offset under load (vibe6 11-13): the GPU's pick of a line is narrower than live's sketch reach,
/// so with the pointer near a line but nothing of the sketch in the GPU's pick, nothing lit and a quick click took the
/// line while a later one didn't. What a click there takes (the line, within reach) lights, and the click selects it.
#[gpui::test]
fn a_line_in_reach_lights_and_is_clicked_when_the_gpu_pick_misses_it(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "reach-not-gpu", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (700., 400.));
    keys(cx, window, &["escape", "escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    let (line, at) = cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        let ed = v.sketch.as_ref().unwrap();
        let line = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
        let Some(noshape_sketch::Curve2::Line { a, b }) = data.sketch.curve(line) else { panic!("a line") };
        let vp = v.viewport.read(cx);
        let (sa, sb) = (vp.project(ed.frame.to_world(a)).unwrap(), vp.project(ed.frame.to_world(b)).unwrap());
        // (Within live's 5 px pick square round the pointer: fc11d lit at 4 px, not 5.)
        (line, (sa + sb) * 0.5 + (sb - sa).perp().normalize() * 4.0)
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            vp.update(cx, |vp, _| vp.place_pointer(at));
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::Hover(None), w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover), Some(line), "the line in reach lights");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().selection.clone()), vec![line], "the click selects it");
}

/// The user's recording (11-17, lead-scratch/vid/hover): in a sketch on Top, the pointer coming onto the sketch's line
/// in front of the Front plane lit the plane, and the light went stale. The GPU's pick stayed the plane, so no new
/// hover came and what is in reach under the pointer wasn't asked again. Each move asks again: the line lights, and
/// the plane once the pointer is off the line.
#[gpui::test]
fn a_line_lights_over_the_plane_the_gpu_pick_keeps(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "line-over-plane", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (700., 400.));
    keys(cx, window, &["escape", "escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    let (line, on, off, plane) = cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        let ed = v.sketch.as_ref().unwrap();
        let line = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
        let Some(noshape_sketch::Curve2::Line { a, b }) = data.sketch.curve(line) else { panic!("a line") };
        let vp = v.viewport.read(cx);
        let (sa, sb) = (vp.project(ed.frame.to_world(a)).unwrap(), vp.project(ed.frame.to_world(b)).unwrap());
        let n = (sb - sa).perp().normalize();
        let plane = v.built.as_ref().unwrap().picks.ids_for(&Reference::Plane(DatumPlane::Front)).first().copied().expect("the Front plane's pick");
        (line, (sa + sb) * 0.5 + n * 3.0, (sa + sb) * 0.5 + n * 60.0, plane)
    });
    let (end, by_end, line_pick) = cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        let ed = v.sketch.as_ref().unwrap();
        let Some(noshape_sketch::Curve2::Line { b, .. }) = data.sketch.curve(line) else { panic!("a line") };
        let end = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Point && data.sketch.point(e.id).is_some_and(|q| (q - b).length() < 1e-9)).unwrap().id;
        let vp = v.viewport.read(cx);
        let at = vp.project(ed.frame.to_world(data.sketch.point(end).unwrap())).unwrap();
        let picks = &v.built.as_ref().unwrap().picks;
        (end, at + DVec2::new(-2.0, 4.0), picks.ids_for(&Reference::SketchEntity { sketch: ed.feature, entity: line.0 }).first().copied().unwrap())
    });
    let move_to = |cx: &mut TestAppContext, at: DVec2, gpu: Option<Option<u32>>| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                vp.update(cx, |vp, _| vp.place_pointer(at));
                v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
                if let Some(id) = gpu {
                    v.on_viewport_event(&vp, &ViewportEvent::Hover(id), w, cx);
                }
            });
        })
        .unwrap();
        draw(cx, window);
    };
    let lit = |cx: &mut TestAppContext| cx.update(|cx| (view.read(cx).sketch.as_ref().unwrap().hover, view.read(cx).hover.clone()));
    move_to(cx, off, Some(Some(plane)));
    assert_eq!(lit(cx), (None, Some(Reference::Plane(DatumPlane::Front))), "off the line: the plane");
    move_to(cx, on, None);
    assert_eq!(lit(cx).0, Some(line), "onto the line, the GPU's pick still the plane: the line lights");
    move_to(cx, off, None);
    assert_eq!(lit(cx), (None, Some(Reference::Plane(DatumPlane::Front))), "off it again: the plane, not a stale line");
    // Beside the line's end, the GPU's pick the line: the end point in its reach wins (live: a point beats its lines).
    move_to(cx, by_end, Some(Some(line_pick)));
    assert_eq!(lit(cx).0, Some(end), "by the end: the point");
}

/// Live (live2 11-17 tools-planes): with a drawing tool active, before the first click and mid-line, no plane
/// pre-highlights (ours lit the Front plane orange under a line being drawn).
#[gpui::test]
fn no_plane_lights_while_a_line_is_drawn(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "no-plane-mid-line", Tool::Line);
    draw(cx, window);
    let plane = cx.update(|cx| view.read(cx).built.as_ref().unwrap().picks.ids_for(&Reference::Plane(DatumPlane::Front)).first().copied().expect("the Front plane's pick"));
    let hover = |cx: &mut TestAppContext, at: (f64, f64)| {
        let at = DVec2::new(at.0, at.1);
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                vp.update(cx, |vp, _| vp.place_pointer(at));
                v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
                v.on_viewport_event(&vp, &ViewportEvent::Hover(Some(plane)), w, cx);
            });
        })
        .unwrap();
        draw(cx, window);
        cx.update(|cx| view.read(cx).hover.clone())
    };
    assert_eq!(hover(cx, (800., 300.)), None, "before the first click");
    click(cx, window, &view, (500., 400.));
    assert_eq!(hover(cx, (800., 300.)), None, "mid-line");
}

/// ui-parity sketch-constraint-refusals step 58 (qa 11-12): a click on a small circle's rim, its centre 11 px away. Our
/// pick pass reaches a point from about 12 px and points win, so the GPU's hover was the centre; held to live's reach
/// (a point 10 px) the circle under the pointer is what lights and what the click selects, as live.
#[gpui::test]
fn a_circles_rim_lights_and_selects_the_circle_not_its_centre_beyond_reach(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "rim-not-centre", Tool::Circle);
    click(cx, window, &view, (600., 400.));
    click(cx, window, &view, (611., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    let ids = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx)?;
        let feature = v.sketch.as_ref()?.feature;
        let circle = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Circle)?.id;
        let centre = data.sketch.entity(circle)?.center()?;
        let pick = v.built.as_ref()?.picks.ids_for(&Reference::SketchEntity { sketch: feature, entity: centre.0 }).first().copied()?;
        Some((circle, pick))
    });
    wait_until(cx, window, |_| true);
    let (circle, centre_pick) = ids(cx).expect("the circle and its centre's pick");
    // The rim point farthest from the centre on screen (the view is isometric): beyond a point's 10 px reach.
    let (at, off) = cx.update(|cx| {
        let v = view.read(cx);
        let (vp, frame) = (v.viewport.read(cx), v.sketch.as_ref().unwrap().frame);
        let Some(noshape_sketch::Curve2::Circle { center, radius }) = v.sketch_data(cx).unwrap().sketch.curve(circle) else { panic!("a circle") };
        // (A pointer pixel stands for its centre: the point under it is half a pixel on.)
        let screen = |q: DVec2| vp.project(frame.to_world(q)).unwrap() - DVec2::splat(0.5);
        let c = screen(center);
        (0..72).map(|k| screen(center + DVec2::from_angle(k as f64 * 5f64.to_radians()) * radius)).map(|s| (s, (s - c).length())).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap()
    });
    assert!(off > 10.5, "the rim {off} px from the centre");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            vp.update(cx, |vp, _| vp.place_pointer(at));
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::Hover(Some(centre_pick)), w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover), Some(circle), "the circle lights, not its centre");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().selection.clone()), vec![circle], "the click selects the circle");
}


/// User 11-12 (hat15b, 6th report): an open sketch that is under defined shows the feature list's under-defined
/// badge on its row, as once it is closed.
#[gpui::test]
fn an_open_under_defined_sketch_has_the_rows_badge(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "open-underdefined");
    // The sketch's edits superseded entering it's background run: nothing stays pending (it kept the working
    // overlay up until the sketch closed).
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert!(cx.update(|cx| view.read(cx).pending.is_none()), "nothing pending in the open sketch");
    let under = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        let id = v.sketch.as_ref().unwrap().feature;
        v.display.feature_state(id).map(|s| (s.status, s.underdefined))
    });
    assert_eq!(under(cx), Some((noshape_model::display::FeatureStatus::Ok, true)), "the open sketch's state");
    // Its line's end dragged (the user's flow): still under defined, open and once closed.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    drag(cx, window, &view, (640., 360.), (660., 330.), 6);
    assert_eq!(under(cx), Some((noshape_model::display::FeatureStatus::Ok, true)), "after a drag");
    let id = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).edit.is_none() && view.read(cx).display.feature_state(id).is_some_and(|s| s.status != noshape_model::display::FeatureStatus::RolledBack));
    assert_eq!(cx.update(|cx| view.read(cx).display.feature_state(id).map(|s| s.underdefined)), Some(true), "closed");
}

/// qa session-1118 (live): with no tool, a rectangle's corner selected, then a click on the sketch's Origin: both
/// selected, and i makes the corner coincident with it (ours: the Origin click cleared the selection).
#[gpui::test]
fn a_corner_and_the_origin_selected_take_a_coincident(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "origin-coincident", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    let corner = cx.update(|cx| {
        let sk = view.read(cx).sketch_data(cx).unwrap().sketch.clone();
        let l = sk.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).map(|e| e.id).unwrap();
        sk.endpoints(l).unwrap().0
    });
    let origin = cx.update(|cx| view.read(cx).viewport.read(cx).project(glam::DVec3::ZERO).unwrap());
    cx.update_window(window, |_, _, cx| view.update(cx, |v, _| {
        v.sketch.as_mut().unwrap().selection = vec![corner];
        // (The GPU's pick under the pointer there.)
        v.hover = Some(Reference::Origin);
    }))
    .unwrap();
    click(cx, window, &view, (origin.x, origin.y));
    let (sel, model) = cx.update(|cx| {
        let ed = view.read(cx).sketch.as_ref().unwrap();
        (ed.selection.clone(), ed.model_selection.clone())
    });
    assert_eq!((sel, model), (vec![corner], vec![Reference::Origin]), "both selected");
    keys(cx, window, &["i"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let at = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.point(corner).unwrap());
    assert!(at.length() < 1e-9, "the corner on the Origin: {at}");
}

/// Live2 sketch-state (11-17): a free line's sketch is under defined (the row's badge, the line and its ends blue);
/// Fix on the line holds the line (black) but not its ends (blue, free along it), the badge stays; a HORIZONTAL on
/// that slanted line can't be solved: the row red ("Sketch could not be solved."), no badge, the bubble, the line
/// where it was; deleting it goes back to the fixed state, deleting the Fix to the free one; all with the sketch open.
#[gpui::test]
fn an_open_sketchs_state_follows_its_constraints(cx: &mut TestAppContext) {
    use noshape_model::display::FeatureStatus;
    use noshape_sketch::ConstraintState::{FullyConstrained as Full, UnderConstrained as Under};
    let (window, view) = line_then_dimension_tool(cx, "open-states");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    let (line, ends, at) = cx.update(|cx| {
        let sk = view.read(cx).sketch_data(cx).unwrap().sketch.clone();
        let l = sk.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).map(|e| e.id).unwrap();
        let (s, e) = sk.endpoints(l).unwrap();
        (l, [s, e], sk.curve(l))
    });
    let row = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        let id = v.sketch.as_ref().unwrap().feature;
        v.display.feature_state(id).map(|s| (s.status, s.underdefined, s.message.clone())).unwrap()
    });
    let states = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        let id = v.sketch.as_ref().unwrap().feature;
        let r = v.context.as_ref().and_then(|c| c.sketches.iter().find(|s| s.feature == id)).map(|s| s.report.clone()).unwrap();
        (r.state(line), ends.map(|e| r.state(e)))
    });
    let settle = |cx: &mut TestAppContext| {
        draw(cx, window);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    };
    let constrain = |cx: &mut TestAppContext, c: ConstraintTool| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.sketch.as_mut().unwrap().selection = vec![line];
                v.constraint(c, w, cx);
            })
        })
        .unwrap();
        settle(cx);
    };
    let delete = |cx: &mut TestAppContext, horizontal: bool| {
        let cid = cx.update(|cx| {
            view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Horizontal { .. }) == horizontal).map(|c| c.id).unwrap()
        });
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let ed = v.sketch.as_mut().unwrap();
                ed.selection.clear();
                ed.selected_constraints = vec![cid];
                v.sketch_delete(w, cx);
            })
        })
        .unwrap();
        settle(cx);
    };
    let free = |cx: &mut TestAppContext, what: &str| {
        assert_eq!((row(cx).0, row(cx).1), (FeatureStatus::Ok, true), "{what}: the badge");
        assert_eq!(states(cx), (Some(Under), [Some(Under); 2]), "{what}: all blue");
    };
    let fixed = |cx: &mut TestAppContext, what: &str| {
        assert_eq!((row(cx).0, row(cx).1), (FeatureStatus::Ok, true), "{what}: the badge stays");
        assert_eq!(states(cx), (Some(Full), [Some(Under); 2]), "{what}: the line black, its ends blue");
    };
    free(cx, "free");
    constrain(cx, ConstraintTool::Fix);
    fixed(cx, "fixed");
    constrain(cx, ConstraintTool::Horizontal);
    let r = row(cx);
    assert!(r.0 == FeatureStatus::Error && !r.1 && r.2.as_deref() == Some("Sketch could not be solved."), "conflict: {r:?}");
    let bubble = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).message.as_ref().map(|m| m.1.clone()));
    assert_eq!(bubble(cx).as_deref(), Some("Sketch could not be solved."), "the bubble");
    let now = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.curve(line));
    assert_eq!(now, at, "the line stays where it was");
    // Accepted and opened again: the bubble shows as it opens (live errors-1118's Sketch HV), over later regenerations.
    let id = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
    assert_eq!(bubble(cx), None, "it goes with the sketch");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    settle(cx);
    assert_eq!(bubble(cx).as_deref(), Some("Sketch could not be solved."), "the bubble as it opens");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    settle(cx);
    delete(cx, true);
    fixed(cx, "the conflict deleted");
    assert_eq!(bubble(cx), None, "the bubble goes");
    delete(cx, false);
    free(cx, "the Fix deleted");
}

/// Parity sketch.md 7.3: the Dimension tool's pick and placement clicks open
/// the value box, which takes the typed value from the first key; Enter
/// applies it.
#[gpui::test]
fn dimension_tool_click_click_type_enter(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "dim-tool");
    assert!(dimensions(cx, &view).is_empty());
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 330.));
    assert!(cx.update(|cx| view.read(cx).dim_input.is_some()), "placement opens the value box");
    keys(cx, window, &["2", "5", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.025]), "{:?}", dimensions(cx, &view));
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()));
}

/// Parity sketch.md 7.16a: Esc on an untouched new box closes it at once
/// and the dimension keeps its measured value; reopened and changed, the
/// first Esc reverts the text and keeps the box, the second closes it.
#[gpui::test]
fn dimension_box_escape_as_live(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "dim-esc");
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 330.));
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "an untouched box closes at once");
    // Live (feel round 2 R10): the box's Esc keeps the Dimension tool, the next one leaves it.
    let tool = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().map(|s| s.tool));
    assert_eq!(tool(cx), Some(Tool::Dimension));
    keys(cx, window, &["escape"]);
    assert_eq!(tool(cx), Some(Tool::Select));
    let measured = dimensions(cx, &view);
    assert_eq!(measured.len(), 1, "the dimension stays");
    let cid = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| c.value().is_some()).unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_dim_edit(cid, w, cx))).unwrap();
    draw(cx, window);
    let box_text = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).dim_input.as_ref().map(|d| d.1.read(cx).value().to_string()));
    let before = box_text(cx);
    keys(cx, window, &["7"]);
    assert_eq!(box_text(cx).as_deref(), Some("7"), "typing replaces the value");
    keys(cx, window, &["escape"]);
    // (The measured text depends on the viewport's scale, so compare with it
    // rather than with the typed key.)
    let text = box_text(cx);
    assert!(text.is_some() && text == before, "the first Esc reverts and keeps the box: {text:?} vs {before:?}");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "the second Esc closes it");
    assert!(near(&dimensions(cx, &view), &measured), "{:?}", dimensions(cx, &view));
}

/// Live sketch-edit-feel-1122 S1: the dimension box applies only on Enter. A click on empty canvas keeps it open,
/// unfocused, its text formatted ("41" → "41 mm"); Tab formats it too and keeps it focused; neither applies it.
#[gpui::test]
fn dimension_box_applies_only_on_enter(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "dim-enter-only");
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 330.));
    keys(cx, window, &["escape"]);
    let measured = dimensions(cx, &view);
    let cid = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().find(|c| c.value().is_some()).unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_dim_edit(cid, w, cx))).unwrap();
    draw(cx, window);
    let state = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| view.read(cx).dim_input.as_ref().map(|d| (d.1.read(cx).value().to_string(), d.1.read(cx).focus_handle(cx).is_focused(w)))).unwrap()
    };
    keys(cx, window, &["4", "1"]);
    click(cx, window, &view, (200., 150.));
    assert_eq!(state(cx), Some(("41 mm".into(), false)), "a click outside: open, unfocused, formatted");
    assert!(near(&dimensions(cx, &view), &measured), "nothing applied: {:?}", dimensions(cx, &view));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| if let Some((_, st, _)) = &v.dim_input { st.update(cx, |s, cx| s.focus(w, cx)) })).unwrap();
    draw(cx, window);
    keys(cx, window, &["secondary-a", "4", "2", "tab"]);
    assert_eq!(state(cx), Some(("42 mm".into(), true)), "Tab: formatted, still focused");
    assert!(near(&dimensions(cx, &view), &measured), "nothing applied: {:?}", dimensions(cx, &view));
    keys(cx, window, &["enter"]);
    assert!(near(&dimensions(cx, &view), &[0.042]), "Enter applies it: {:?}", dimensions(cx, &view));
}

/// The number of sides typed before the polygon's rim click (live: digits set
/// the side count while the polygon tool waits for its circumference); the
/// click after the rim's commits it.
#[gpui::test]
fn polygon_sides_typed_before_the_rim_click(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "polygon", Tool::Polygon);
    click(cx, window, &view, (600., 400.));
    keys(cx, window, &["8"]);
    click(cx, window, &view, (680., 400.));
    click(cx, window, &view, (680., 400.));
    let lines = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.construction).count());
    assert_eq!(lines, 8);
}

/// Enter in a Line chain (live): the last segment's boxed length becomes a
/// driving dimension at its measured value.
#[gpui::test]
fn enter_in_a_line_chain_dimensions_the_last_segment(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "line-chain", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    click(cx, window, &view, (700., 450.));
    let last = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).map(|e| e.id).max().and_then(|l| d.sketch.curve(l)).map(|c| c.length()).unwrap()
    });
    keys(cx, window, &["enter"]);
    assert!(near(&dimensions(cx, &view), &[last]), "{:?} vs {last}", dimensions(cx, &view));
}

/// BOX-DIALOG-PANIC: a box selection with a feature dialog open went into the
/// dialog's field while reading the dialog inside its own update (a panic).
#[gpui::test]
fn box_select_with_a_dialog_open(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "box-dialog");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    draw(cx, window);
    // Every pick id the drawn scene can have (the default planes, the origin).
    let ids: Vec<u32> = (1..64).collect();
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            assert!(v.edit.is_some(), "the Fillet dialog is open");
            v.box_select(&ids, false, cx);
            v.box_select(&ids, true, cx);
        })
    })
    .unwrap();
    draw(cx, window);
}

/// A number an editing logic sets shows in its field (std's Tab logic set
/// Subtraction offset 0.02 mm and the field still read "0 mm").
#[gpui::test]
fn a_value_set_by_editing_logic_shows_in_its_field(cx: &mut TestAppContext) {
    use noshape_model::params::ParamValue;
    let (window, view) = part_studio(cx, "logic-text");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    draw(cx, window);
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    assert!(dialog.update(cx, |d, cx| d.field_text("depth", cx)).is_some(), "the Depth field is drawn");
    dialog.update(cx, |d, cx| d.set_value_logic("depth", ParamValue::Expr(noshape_units::ExprValue::new("0.02 mm")), cx));
    draw(cx, window);
    assert_eq!(dialog.update(cx, |d, cx| d.field_text("depth", cx)).as_deref(), Some("0.02 mm"));
}

/// SECTION-DIALOG-STACK: starting a feature with the Section view dialog open
/// closes that dialog as with ✓ and keeps the section (live 10-27).
#[gpui::test]
fn a_feature_started_over_the_section_dialog_accepts_it(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "section-stack");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.section_through(Reference::Plane(DatumPlane::Front), w, cx);
            assert!(v.ui.section_dialog.is_some());
            v.start_feature("extrude", w, cx);
            assert!(v.ui.section_dialog.is_none(), "the Section view dialog closed");
            assert!(!v.ui.section.is_empty(), "the section stays on");
            assert!(v.edit.is_some(), "the Extrude dialog is open");
        })
    })
    .unwrap();
}

/// Live 11-02: a section plane keeps its number when another is removed ("Section plane 2 (Front)" alone), a new one
/// takes the lowest number free.
#[gpui::test]
fn section_planes_keep_their_numbers(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "section-numbers");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let numbers = |v: &PartStudioView| v.ui.section.iter().map(|p| p.number).collect::<Vec<_>>();
            v.section_through(Reference::Plane(DatumPlane::Top), w, cx);
            assert!(v.section_pick(&Reference::Plane(DatumPlane::Front), w, cx));
            assert_eq!(numbers(v), [1, 2]);
            assert!(v.section_pick(&Reference::Plane(DatumPlane::Top), w, cx), "Top again removes it");
            assert_eq!(numbers(v), [2]);
            assert!(v.section_pick(&Reference::Plane(DatumPlane::Top), w, cx));
            assert!(v.section_pick(&Reference::Plane(DatumPlane::Right), w, cx));
            assert_eq!(numbers(v), [2, 1, 3]);
        })
    })
    .unwrap();
}

/// Live2 sketch-entry-1118: with "Orient normal to sketch on edit" on, editing Sketch 1 from iso turns the view to its
/// normal; ✗ leaves the view there (it doesn't go back to iso).
#[gpui::test]
fn orient_on_edit_turns_to_the_sketch_and_exit_keeps_the_view(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "orient-on-edit", Some(block));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // The viewport's animation clock, moved with the test's (a new view change is ignored while one runs, live).
    let clock = std::rc::Rc::new(std::cell::Cell::new(std::time::Instant::now()));
    let c = clock.clone();
    cx.update(|cx| view.read(cx).viewport.clone().update(cx, |vp, _| vp.set_clock(move || c.get())));
    let settle = |cx: &mut TestAppContext| {
        for _ in 0..20 {
            clock.set(clock.get() + std::time::Duration::from_millis(100));
            cx.executor().advance_clock(std::time::Duration::from_millis(100));
            draw(cx, window);
        }
    };
    view.update(cx, |v, cx| {
        v.view(noshape_render::StandardView::Isometric, cx);
        v.ui.orient_on_edit = true;
    });
    settle(cx);
    // Where the view is going (the viewport animates on its own clock): the animation's end, else the camera.
    let forward = |cx: &mut TestAppContext| cx.update(|cx| {
        let vp = view.read(cx).viewport.read(cx);
        vp.animation().map_or(vp.camera, |(to, _)| to).forward()
    });
    let iso = forward(cx);
    let sketch = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Sketch 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(sketch, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).sketch.is_some());
    settle(cx);
    let editing = forward(cx);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).sketch.is_none());
    settle(cx);
    let after = forward(cx);
    assert!(iso.distance(glam::DVec3::NEG_Z) > 0.5, "iso to start: {iso:?}");
    assert!(editing.distance(glam::DVec3::NEG_Z) < 1e-6, "normal to Top while editing: {editing:?}");
    assert!(after.distance(glam::DVec3::NEG_Z) < 1e-6, "still normal after ✗: {after:?}");
}

/// Live2 view-tools-1118: a section turned off comes back with the next Shift+X, its plane and offset as they were.
#[gpui::test]
fn shift_x_after_turning_off_restores_the_last_section(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "section-restore");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection.clear();
            v.section_through(Reference::Plane(DatumPlane::Front), w, cx);
            v.ui.section[0].offset = 0.0123;
            v.end_section_dialog(true, w, cx);
            v.turn_off_section(w, cx);
            assert!(v.ui.section.is_empty(), "turned off");
            v.toggle_section(w, cx);
            let back: Vec<(String, f64)> = v.ui.section.iter().map(|p| (p.label.clone(), p.offset)).collect();
            assert_eq!(back.len(), 1, "the plane back: {back:?}");
            assert!((back[0].1 - 0.0123).abs() < 1e-12 && v.ui.section_dialog.is_some(), "its offset, the dialog open: {back:?}");
        })
    })
    .unwrap();
}

/// Live cap-plane-1119: the cap is tan (drawn selected) only while its plane has just been picked in the open dialog;
/// after ✓ and when the section is reopened for edit it is the part colour; a plane picked then turns tan again.
#[gpui::test]
fn a_section_cap_is_tan_only_while_its_plane_is_just_picked(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "section-cap-tan");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection.clear();
            v.section_through(Reference::Plane(DatumPlane::Front), w, cx);
            let picked = v.opts.section_selected;
            v.end_section_dialog(true, w, cx);
            let after_ok = v.opts.section_selected;
            v.edit_section(w, cx);
            let reopened = v.opts.section_selected;
            assert!(v.section_pick(&Reference::Plane(DatumPlane::Right), w, cx));
            let right = v.opts.section_selected;
            assert_eq!((picked, after_ok, reopened, right), (Some(0), None, None, Some(1)), "(Front picked, after ✓, reopened, Right picked)");
        })
    })
    .unwrap();
}

/// Live 11-02: every section plane's manipulator is shown while the dialog is open, and dragging another plane's
/// arrow moves that plane.
#[gpui::test]
fn another_section_planes_arrow_drags_it(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "section-others", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.finish_sketch(w, cx);
            v.section_through(Reference::Plane(DatumPlane::Top), w, cx);
            assert!(v.section_pick(&Reference::Plane(DatumPlane::Right), w, cx));
        })
    })
    .unwrap();
    draw(cx, window);
    let (from, to) = view.update(cx, |v, cx| {
        assert_eq!(v.ui.section_dialog.as_ref().map(|d| d.active), Some(1), "Right is the active plane");
        let others = v.section_other_manips();
        assert_eq!(others.len(), 1, "Top's manipulator is shown too");
        let a = others[0].1.arrow;
        let vp = v.viewport.read(cx);
        let tip = a.base + a.dir * a.length;
        let (p0, p1) = (vp.project(tip).unwrap(), vp.project(tip + vp.camera.right() * 1e-3).unwrap());
        let wpp = 1e-3 / (p1 - p0).length();
        let (from, to) = (vp.project(tip + a.dir * 45.0 * wpp).unwrap(), vp.project(tip + a.dir * 75.0 * wpp).unwrap());
        ((from.x, from.y), (to.x, to.y))
    });
    drag(cx, window, &view, from, to, 4);
    view.update(cx, |v, _| {
        assert_eq!(v.ui.section_dialog.as_ref().map(|d| d.active), Some(0), "Top is now the active plane");
        assert!(v.ui.section[0].offset > 0.0, "Top moved up: {}", v.ui.section[0].offset);
        assert_eq!(v.ui.section[1].offset, 0.0, "Right stayed");
    });
    // Live view-tools-1118: a value box at the arrow while dragging, in 0.1 mm steps from the drag's start; typing in
    // it moves the plane.
    let (moved, boxed) = cx.update(|cx| {
        let v = view.read(cx);
        let moved = v.ui.section[0].offset - v.ui.section_drag0.unwrap_or(0.0);
        (moved, v.ui.section_offset_edit.as_ref().map(|(i, _)| i.read(cx).value().to_string()))
    });
    assert!(((moved / 1e-4) - (moved / 1e-4).round()).abs() < 1e-6, "0.1 mm steps: moved {moved}");
    let units = cx.update(|cx| view.read(cx).doc.read(cx).session.doc.settings.units);
    let shown = boxed.as_deref().and_then(|t| noshape_units::evaluate_as(t, noshape_units::Dim::LENGTH, &units, &noshape_units::EmptyScope).ok()).map(|q| q.value);
    assert!(shown.is_some_and(|d| (d - moved).abs() < 1e-9), "the value box {boxed:?} for {moved}");
    keys(cx, window, &["5", "enter"]);
    let typed = cx.update(|cx| {
        let v = view.read(cx);
        v.ui.section[0].offset - v.ui.section_drag0.unwrap_or(0.0)
    });
    assert!((typed - 0.005).abs() < 1e-9, "\"5\" + Enter: 5 mm from the drag's start, was {moved}, now {typed}");
}

/// Interaction 6.2 (live qa4-r5-s9-5b / 5f): each rotate handle of the Section view manipulator lights alone along
/// its axis line, and the glyph's exact centre, where the two cross, is on no handle.
#[gpui::test]
fn section_rotate_handles_and_their_dead_centre(cx: &mut TestAppContext) {
    use super::super::section_manip::SectionHandle;
    // A rectangle sketch gives the model its bounds (the manipulator stands on them).
    let (window, view) = sketch_with(cx, "section-glyph", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.finish_sketch(w, cx);
            v.section_through(Reference::Plane(DatumPlane::Top), w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    view.update(cx, |v, cx| {
        let s = v.section_manip().expect("the section manipulator");
        let vp = v.viewport.read(cx);
        let tip = s.arrow.base + s.arrow.dir * s.arrow.length;
        let (p0, p1) = (vp.project(tip).unwrap(), vp.project(tip + vp.camera.right() * 1e-3).unwrap());
        let wpp = 1e-3 / (p1 - p0).length();
        let centre = s.ring_center(wpp);
        let at = |p: glam::DVec3| vp.project(p).unwrap();
        assert_eq!(v.section_handle_at(&s, at(centre), cx), None, "the centre is on no handle");
        for k in 0..2 {
            assert_eq!(v.section_handle_at(&s, at(centre + s.axes[k] * 16.0 * wpp), cx), Some(SectionHandle::Ring(k)), "along axis {k}");
        }
    });
}

/// qa FEEL-SECTION-ARROW-FOOT (live manip 10-28, side-on): the Section view's arrow is longer than Extrude's: its
/// shaft runs from the base ring to the head's base 66 px from the plane, the apex ~98 px out, the rings past it.
#[gpui::test]
fn the_section_arrow_reaches_about_100_px_from_its_plane(cx: &mut TestAppContext) {
    use super::super::section_manip::SectionHandle;
    let (window, view) = sketch_with(cx, "section-arrow-len", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.finish_sketch(w, cx);
            v.section_through(Reference::Plane(DatumPlane::Top), w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    view.update(cx, |v, cx| {
        let s = v.section_manip().expect("the section manipulator");
        let vp = v.viewport.read(cx);
        let tip = s.arrow.base + s.arrow.dir * s.arrow.length;
        let (p0, p1) = (vp.project(tip).unwrap(), vp.project(tip + vp.camera.right() * 1e-3).unwrap());
        let wpp = 1e-3 / (p1 - p0).length();
        let at = |px: f64| vp.project(tip + s.arrow.dir * px * wpp).unwrap();
        for px in [30.0, 60.0, 90.0] {
            assert_eq!(v.section_handle_at(&s, at(px), cx), Some(SectionHandle::Arrow), "{px} px up the arrow");
        }
        let rings = (s.ring_center(wpp) - tip).length() / wpp;
        assert!((100.0..=112.0).contains(&rings), "the rings' centre {rings:.1} px from the plane (live ~109)");
    });
}

/// MC-SELECT-NO-READOUT: a mate connector selected alone reads "Point:" at its
/// origin (live "psui (2026-10-27, P2) interaction 3.4"); with something else
/// selected too the readout measures the selection as before.
#[gpui::test]
fn a_selected_mate_connector_reads_its_origin(cx: &mut TestAppContext) {
    use noshape_model::display::ConnectorDisplay;
    use noshape_model::reference::FeatureId;
    let (window, view) = part_studio(cx, "mc-readout");
    let mc = Reference::MateConnector { feature: FeatureId(7), index: 0 };
    let origin = glam::DVec3::new(0.02, 0.01, 0.01);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, _| {
            std::sync::Arc::make_mut(&mut v.display).connectors.push(ConnectorDisplay {
                reference: mc.clone(),
                name: "Mate connector 1".into(),
                origin,
                x: glam::DVec3::X,
                z: glam::DVec3::Z,
            });
            assert_eq!(v.readout_connector_origin(), None, "nothing selected");
            v.selection = vec![mc.clone()];
            assert_eq!(v.readout_connector_origin(), Some(origin));
            v.selection.push(Reference::Plane(DatumPlane::Top));
            assert_eq!(v.readout_connector_origin(), None, "a connector with a plane is a measurement of both");
        })
    })
    .unwrap();
}

/// HOLE-TABLE-BUTTON (live "qa (2026-10-28, P3) Part Studio Hole table panel with no holes"): with no hole the
/// Hole table panel doesn't exist (an open one closes), and a suppressed Hole lists as "Hole".
#[gpui::test]
fn no_hole_table_without_a_hole(cx: &mut TestAppContext) {
    use super::super::config_panel::RightPanel;
    let (window, view) = part_studio(cx, "hole-table");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, _| {
            assert!(!v.has_holes());
            v.right_panel = Some(RightPanel::HoleTable);
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            assert_eq!(v.right_panel, None, "the Hole table closed: no hole");
            let units = noshape_units::UnitPrefs::default();
            let mut f = v.studio(cx).unwrap().new_feature("hole", &units).unwrap();
            assert_ne!(v.shown_feature_name(&f, cx), "Hole", "an unsuppressed Hole is named by its callout");
            f.suppressed = true;
            assert_eq!(v.shown_feature_name(&f, cx), "Hole");
        })
    })
    .unwrap();
}

/// PREFS-SELECT-LIST-DETACH: an open `<select>` list closes when the wheel turns outside it (Chrome); inside, it
/// stays.
#[gpui::test]
fn a_select_list_closes_on_a_wheel_outside_it(cx: &mut TestAppContext) {
    use crate::os_menu::{OsMenu, OsMenuItem};
    struct Page;
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }
    cx.update(gpui_kit::init);
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1000.), px(800.)) })), ..Default::default() };
    let (window, _) = cx.update(|cx| crate::os_menu::open_window(options, cx, |_, cx| cx.new(|_| Page))).unwrap();
    cx.update_window(window, |_, w, cx| {
        // Options from a list (not menu labels of the app: the ops menu coverage scans literal ones).
        let options = ["one", "two"].map(String::from);
        let menu = OsMenu::build(w, cx, |m, _, _| options.into_iter().fold(m.native_select(170.), |m, o| m.item(OsMenuItem::new(o))));
        crate::os_menu::open_anchored(gpui_kit::point(px(100.), px(100.)), gpui_kit::Anchor::TopLeft, menu, w, cx);
    })
    .unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let wheel = |x: f32, y: f32| gpui_kit::ScrollWheelEvent { position: gpui_kit::point(px(x), px(y)), delta: gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., -3.)), ..Default::default() };
    vcx.simulate_event(wheel(150., 110.));
    assert!(vcx.update(|_, cx| crate::os_menu::is_open(cx)), "a wheel over the list keeps it");
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_event(wheel(600., 500.));
    assert!(!vcx.update(|_, cx| crate::os_menu::is_open(cx)), "a wheel outside closes it");
}

/// core-workflow 8.1.4 / 8.2.4: a closed sketch's Show dimensions draws each dimension's value in the sketch plane
/// (its glyph outlines among the viewport's in-plane fills), not only its lines and arrows.
#[gpui::test]
fn shown_dimensions_draw_their_values(cx: &mut TestAppContext) {
    let (window, view) = line_then_dimension_tool(cx, "shown-dims");
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 330.));
    keys(cx, window, &["2", "5", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.025]), "{:?}", dimensions(cx, &view));
    let sketch = cx.update(|cx| view.read(cx).sketch.as_ref().map(|s| s.feature)).expect("sketching");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.finish_sketch(w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.show_dimensions(sketch, None, w, cx))).unwrap();
    draw(cx, window);
    let (labels, fills) = cx.update(|cx| {
        let v = view.read(cx);
        (v.shown_dims_geometry(cx).map(|(l, _)| l.into_iter().map(|l| l.text).collect::<Vec<_>>()), v.shown_dims_fills(cx))
    });
    assert_eq!(labels, Some(vec!["25".to_string()]), "the dimension's label");
    assert!(fills.iter().any(|f| f.contours.len() >= 2), "the label's digits as glyph outlines: {:?}", fills.iter().map(|f| f.contours.len()).collect::<Vec<_>>());
}

/// core-workflow 8.4.1: Top › Rename, Esc, then the focus elsewhere (a context menu taking it) records nothing: the
/// field's late blur used to rename the plane to its own name (an undo step and a regeneration under the open menu,
/// whose click it ate).
#[gpui::test]
fn an_escaped_plane_rename_records_nothing(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "plane-rename");
    let top = Reference::Plane(DatumPlane::Top);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_default_rename(top.clone(), w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| {
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    let (names, undo) = cx.update(|cx| {
        let v = view.read(cx);
        (v.studio(cx).map(|s| s.default_names.clone()).unwrap_or_default(), v.doc.read(cx).session.can_undo())
    });
    assert!(names.is_empty(), "no new name: {names:?}");
    assert!(!undo, "nothing to undo in the fresh document");
}

/// core-workflow 8.5.2: the same for a folder's Rename (Esc, then the focus elsewhere): nothing recorded.
#[gpui::test]
fn an_escaped_folder_rename_records_nothing(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "folder-rename");
    let folder = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Add folder", cx, |ps| {
                    noshape_ops::studio::add_folder(ps, &[], "F1");
                })
            });
            v.studio(cx).and_then(|s| s.folders.first().map(|f| f.id)).expect("a folder")
        })
    });
    let undo_before = cx.update(|cx| view.read(cx).doc.read(cx).session.last_redone().map(str::to_string));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_folder_rename(folder, w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| {
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    let (name, undo) = cx.update(|cx| {
        let v = view.read(cx);
        (v.studio(cx).and_then(|s| s.folder(folder).map(|f| f.name.clone())), v.doc.read(cx).session.last_redone().map(str::to_string))
    });
    assert_eq!(name.as_deref(), Some("F1"));
    assert_eq!(undo, undo_before, "no rename recorded");
}

/// Live 11-02 (qa4's "Plane z10"): p hides every plane, a Plane feature's with the default ones, then shows them all.
#[gpui::test]
fn p_toggles_every_plane(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "p-every-plane");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Add plane", cx, |ps| {
                    let mut f = ps.new_feature("cPlane", &noshape_units::UnitPrefs::default()).expect("Plane");
                    f.params.set("entities", noshape_model::ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
                    f.params.set("offset", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("10 mm")));
                    ps.features.push(f);
                })
            });
        })
    });
    wait_until(cx, window, |cx| view.read(cx).display.planes.len() == 4);
    let shown = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.planes.iter().filter(|p| !p.hidden).count());
    assert_eq!(shown(cx), 4, "the default planes and Plane z10");
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_default_planes(cx)));
    wait_until(cx, window, |cx| view.read(cx).display.planes.iter().all(|p| p.hidden));
    assert_eq!(shown(cx), 0, "p hid every plane");
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_default_planes(cx)));
    wait_until(cx, window, |cx| view.read(cx).display.planes.iter().all(|p| !p.hidden));
    assert_eq!(shown(cx), 4, "p again showed them all");
}

/// core-workflow 8.1.15 / 8.6.4: one Esc cancels the appearance editor, its hex field focused (a part's; live request
/// 1663 (a)).
#[gpui::test]
fn escape_cancels_the_appearance_editor(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "appearance-esc");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(super::super::appearance::EditTarget::Parts(vec![]), w, cx))).unwrap();
    draw(cx, window);
    let hex_focused = cx.update_window(window, |_, w, cx| view.read(cx).ui.appearance_edit.as_ref().is_some_and(|e| e.hex.read(cx).focus_handle(cx).is_focused(w))).unwrap();
    assert!(hex_focused, "the editor opens with its hex field focused");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).ui.appearance_edit.is_none()), "Esc closed the editor");
}

/// Live (behaviours "Appearance editor R / G / B / A / hex"): Tab goes A → # → R → G → B, a field emptied shows its
/// value again when left, Enter accepts and closes the editor.
#[gpui::test]
fn the_appearance_editors_fields_tab_and_enter_as_live(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "appearance-tab");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(super::super::appearance::EditTarget::Parts(vec![]), w, cx))).unwrap();
    draw(cx, window);
    let focused = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| {
            let e = view.read(cx).ui.appearance_edit.as_ref().unwrap();
            [&e.alpha, &e.hex, &e.channels[0], &e.channels[1], &e.channels[2]].iter().position(|f| f.read(cx).focus_handle(cx).is_focused(w))
        })
        .unwrap()
    };
    assert_eq!(focused(cx), Some(1), "the hex field first");
    keys(cx, window, &["tab"]);
    assert_eq!(focused(cx), Some(2), "Tab: R");
    keys(cx, window, &["1", "2", "tab"]);
    assert_eq!(focused(cx), Some(3), "Tab: G");
    keys(cx, window, &["tab", "tab"]);
    assert_eq!(focused(cx), Some(0), "after B, A");
    keys(cx, window, &["shift-tab", "shift-tab", "shift-tab"]);
    assert_eq!(focused(cx), Some(2), "Shift+Tab back to R");
    keys(cx, window, &["backspace", "backspace", "tab"]);
    let r = cx.update(|cx| {
        let e = view.read(cx).ui.appearance_edit.as_ref().unwrap();
        (e.rgb[0], e.channels[0].read(cx).value().to_string())
    });
    assert_eq!(r, (12, "12".to_string()), "R emptied keeps 12");
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).ui.appearance_edit.is_none()), "Enter closed the editor");
}

/// Live workflow 28 (qa APPEARANCE-RGB-FIELD-SCROLL): R selected whole, 200 typed, Tab: the left field shows its
/// start, "200", not the end it scrolled to while typing ("00").
#[gpui::test]
fn a_left_colour_field_shows_the_start_of_its_value(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "appearance-r-scroll");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(super::super::appearance::EditTarget::Parts(vec![]), w, cx))).unwrap();
    draw(cx, window);
    let r_scroll = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).ui.appearance_edit.as_ref().unwrap().channels[0].read(cx).scroll_offset().x);
    keys(cx, window, &["tab", "secondary-a", "2", "0", "0"]);
    draw(cx, window);
    let typing = r_scroll(cx);
    keys(cx, window, &["tab"]);
    draw(cx, window);
    let left = r_scroll(cx);
    let r = cx.update(|cx| view.read(cx).ui.appearance_edit.as_ref().unwrap().rgb[0]);
    assert_eq!((r, typing < gpui::px(0.), left), (200, true, gpui::px(0.)), "(R, scrolled while typing, offset when left)");
}

/// Live2 mass-appear-1118, the Mixer tab: a press alone in the saturation / value square changes nothing; a drag sets
/// the colour (the hex follows); a click on the hue strip takes its hue and keeps saturation and value.
#[gpui::test]
fn the_appearance_mixer_changes_on_a_drag_not_a_press(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "appearance-mixer");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(super::super::appearance::EditTarget::Parts(vec![]), w, cx))).unwrap();
    cx.update(|cx| view.update(cx, |v, cx| {
        v.ui.appearance_edit.as_mut().unwrap().mixer = true;
        cx.notify();
    }));
    draw(cx, window);
    let state = |cx: &mut TestAppContext| cx.update(|cx| {
        let e = view.read(cx).ui.appearance_edit.as_ref().unwrap();
        (e.rgb, e.hex.read(cx).value().to_string(), e.mix.s, e.mix.v)
    });
    let bounds = |cx: &mut TestAppContext, sel: &'static str| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(sel).expect(sel);
    let sq = bounds(cx, "mixer-square");
    let at = |b: gpui_kit::Bounds<gpui_kit::Pixels>, fx: f32, fy: f32| gpui_kit::point(b.left() + b.size.width * fx, b.top() + b.size.height * fy);
    let start = state(cx);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at(sq, 0.5, 0.5), modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
    draw(cx, window);
    let pressed = state(cx);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at(sq, 0.75, 0.25), pressed_button: Some(gpui_kit::MouseButton::Left), modifiers: Modifiers::default() });
    vcx.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: at(sq, 0.75, 0.25), modifiers: Modifiers::default(), click_count: 1 });
    draw(cx, window);
    let dragged = state(cx);
    let strip = bounds(cx, "mixer-strip");
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at(strip, 0.5, 1.0 / 3.0), Modifiers::default());
    draw(cx, window);
    let hued = state(cx);
    let hex = |rgb: [u8; 3]| format!("{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
    assert_eq!(pressed.0, start.0, "a press alone changes nothing");
    assert!(dragged.0 != start.0 && dragged.1.to_lowercase().contains(&hex(dragged.0)), "the drag sets the colour and the hex: {dragged:?}");
    assert!(hued.0 != dragged.0 && (hued.2, hued.3) == (dragged.2, dragged.3), "a hue click keeps saturation and value: {dragged:?} -> {hued:?}");
}

/// Live (ui-parity appearance-face-row-remove): the appearance editor commits as you type, so Undo turns on with the
/// first change; ✓ makes the session one undo step, and ✕ puts the appearances back with no step.
#[gpui::test]
fn the_appearance_editor_commits_as_you_type(cx: &mut TestAppContext) {
    use super::super::appearance::EditTarget;
    use noshape_model::reference::FeatureId;
    let (window, view) = part_studio(cx, "appearance-live");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Add features", cx, |ps| {
                    ps.features.push(noshape_model::partstudio::Feature { id: FeatureId(9002), type_id: "extrude".into(), name: "Extrude 1".into(), suppressed: false, params: Default::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None, sub_features: Vec::new() });
                })
            });
        })
    });
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    let state = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let d = v.doc.read(cx);
            let colors: Vec<[f32; 4]> = v.studio(cx).map(|s| s.appearances.iter().map(|a| a.color).collect()).unwrap_or_default();
            (colors.len(), colors.first().map(|c| c[0]), d.session.undo_len(), d.can_undo())
        })
    };
    let open = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(EditTarget::Feature(FeatureId(9002), None), w, cx))).unwrap();
    let set = |cx: &mut TestAppContext, rgb: [u8; 3]| cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.set_edit_color(rgb, w, cx))).unwrap();
    let (_, _, steps, _) = state(cx);
    open(cx);
    set(cx, [200, 0, 0]);
    let (n, r, now, undo) = state(cx);
    assert_eq!((n, now, undo), (1, steps, true), "the first change is in the document at once, Undo on, no step yet");
    assert!(r.is_some_and(|r| (r - super::super::appearance::srgb_to_lin(200)).abs() < 1e-6), "{r:?}");
    set(cx, [10, 0, 0]);
    cx.update(|cx| view.update(cx, |v, cx| v.accept_appearance(cx)));
    let (n, _, after, _) = state(cx);
    assert_eq!((n, after), (1, steps + 1), "✓: one undo step for the session");
    open(cx);
    set(cx, [90, 0, 0]);
    cx.update(|cx| view.update(cx, |v, cx| v.cancel_appearance(cx)));
    let (n, r, after_cancel, _) = state(cx);
    assert_eq!((n, after_cancel), (1, steps + 1), "✕: no new step");
    assert!(r.is_some_and(|r| (r - super::super::appearance::srgb_to_lin(10)).abs() < 1e-6), "✕ put the colour back: {r:?}");
}

/// core-workflow 8.2.11: Extrude 1 › Add appearance to feature… opens "Face appearance 1" with "Extrude 1" in its
/// Faces or features field, black, the focus on the body; Sketch 1's editor is "Appearance" at #999 (live 1663).
#[gpui::test]
fn a_features_appearance_opens_as_live(cx: &mut TestAppContext) {
    use super::super::appearance::EditTarget;
    use noshape_model::reference::FeatureId;
    let (window, view) = part_studio(cx, "appearance-feature");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Add features", cx, |ps| {
                    for (id, ty, name) in [(9001, "sketch", "Sketch 1"), (9002, "extrude", "Extrude 1")] {
                        ps.features.push(noshape_model::partstudio::Feature { id: FeatureId(id), type_id: ty.into(), name: name.into(), suppressed: false, params: Default::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None, sub_features: Vec::new() });
                    }
                })
            });
        })
    });
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    let open = |cx: &mut TestAppContext, target: EditTarget| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(target, w, cx))).unwrap();
        draw(cx, window);
        cx.update_window(window, |_, w, cx| {
            let v = view.read(cx);
            let e = v.ui.appearance_edit.as_ref().expect("the editor");
            let (title, faces) = v.appearance_heading(&e.target, cx);
            (title.to_string(), faces, e.rgb, e.hex.read(cx).focus_handle(cx).is_focused(w))
        })
        .unwrap()
    };
    assert_eq!(open(cx, EditTarget::Feature(FeatureId(9002), None)), ("Face appearance 1".into(), Some("Extrude 1".into()), [0, 0, 0], false));
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).ui.appearance_edit.is_none()), "Esc cancelled the face appearance");
    assert_eq!(open(cx, EditTarget::Feature(FeatureId(9001), None)), ("Appearance".into(), None, [0x99, 0x99, 0x99], true));
    keys(cx, window, &["escape"]);
    // The panel's Add face appearance on no faces: black, ✓ off (live appearance-1123 q1-panel-add-face-appearance-click).
    let (rgb, ok) = (open(cx, EditTarget::Faces(Vec::new(), None)).2, cx.update(|cx| view.read(cx).ui.appearance_edit.as_ref().map(|e| e.acceptable())));
    assert_eq!((rgb, ok), ([0, 0, 0], Some(false)), "(colour, ✓ on) with no faces");
}

/// workflow-keyboard-mouse "p" (live request 1673): p sets the default planes' own visibility: with Top shown and
/// Front / Right hidden it hides Top too, then shows all three.
#[gpui::test]
fn p_hides_the_default_planes_then_shows_them(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "p-planes");
    let planes = [DatumPlane::Top, DatumPlane::Front, DatumPlane::Right].map(Reference::Plane);
    cx.update(|cx| view.update(cx, |v, cx| v.set_visibility(&planes[1..], true, cx)));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    cx.update_window(window, |_, w, cx| {
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    let hidden = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).map(|s| planes.iter().filter(|p| s.hidden_defaults.contains(p)).count()).unwrap_or(0));
    assert_eq!(hidden(cx), 2);
    keys(cx, window, &["p"]);
    assert_eq!(hidden(cx), 3, "a shown plane: p hides all three");
    keys(cx, window, &["p"]);
    assert_eq!(hidden(cx), 0, "all hidden: p shows all three");
}

/// User P1 (10-31): Esc on a dialog whose preview is still regenerating shows the document as it was before the edit
/// at once (the same display, no dialog, no spinner, in the same update), and the cancelled run's late result never
/// replaces it.
#[gpui::test]
fn cancel_shows_the_document_before_the_edit_at_once(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "cancel-at-once");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    let before = cx.update(|cx| view.read(cx).display.clone());
    assert!(cx.update(|cx| view.read(cx).pending.is_none()), "settled before the edit");
    // The dialog opens and its preview regeneration starts; the executor doesn't run it before the Esc (a slow one).
    let after_esc = cx
        .update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| v.start_feature("extrude", w, cx));
            assert!(view.read(cx).edit.is_some() && view.read(cx).pending.is_some(), "the preview regenerating");
            let focus = view.read(cx).focus.clone();
            w.focus(&focus, cx);
            w.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            let v = view.read(cx);
            (v.edit.is_some(), v.pending.is_some(), std::sync::Arc::ptr_eq(&v.display, &before))
        })
        .unwrap();
    assert_eq!(after_esc, (false, false, true), "(dialog open, spinner, other display) right after Esc");
    // Whatever the cancelled run still delivers is dropped.
    draw(cx, window);
    let settled = cx.update(|cx| {
        let v = view.read(cx);
        (v.pending.is_some(), std::sync::Arc::ptr_eq(&v.display, &before))
    });
    assert_eq!(settled, (false, true), "no late result, no spinner");
}

/// User P1 (11-01): a dialog opened while an earlier edit still regenerates cancels at once too: Esc shows the last
/// finished result (no dialog, the new feature not drawn) in the same update; neither run's late result is shown, and
/// the earlier edit's document then regenerates afresh (its new variable drawn, the cancelled extrude never).
#[gpui::test]
fn cancel_during_an_earlier_regeneration_shows_the_last_result(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "cancel-earlier");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    let before = cx.update(|cx| view.read(cx).display.clone());
    let after_esc = cx
        .update_window(window, |_, w, cx| {
            // A committed edit (a Variable) starts regenerating; before it finishes, a dialog opens and is cancelled.
            let var = view.update(cx, |v, cx| {
                let (tab, units) = (v.tab, v.doc.read(cx).session.doc.settings.units);
                let var = v.doc.update(cx, |doc, cx| {
                    let mut id = None;
                    doc.edit_studio(tab, "Create variable", cx, |ps| id = noshape_model::variable_table::create(ps, "w", "LENGTH", "1 mm", "", None, &units).ok());
                    id.expect("a variable")
                });
                v.start_feature("extrude", w, cx);
                var
            });
            let extrude = view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.id).expect("the dialog");
            assert!(view.read(cx).pending.is_some(), "both regenerating");
            let focus = view.read(cx).focus.clone();
            w.focus(&focus, cx);
            w.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            let v = view.read(cx);
            ((v.edit.is_some(), std::sync::Arc::ptr_eq(&v.display, &before)), var, extrude)
        })
        .unwrap();
    let (state, var, extrude) = after_esc;
    assert_eq!(state, (false, true), "(dialog open, other display) right after Esc");
    // (The fresh run coalesces with the burst of requests: its 12 ms wait.)
    cx.executor().advance_clock(std::time::Duration::from_millis(50));
    draw(cx, window);
    let settled = cx.update(|cx| {
        let v = view.read(cx);
        (v.pending.is_some(), v.display.feature_state(var).is_some(), v.display.feature_state(extrude).is_some())
    });
    assert_eq!(settled, (false, true, false), "(spinner, variable drawn, cancelled extrude drawn) once settled");
}

/// A feature dialog of `type_id` (its defaults, `set` applied) in the test window's app, with no field active yet.
fn dialog_of(cx: &mut TestAppContext, view: &Entity<PartStudioView>, type_id: &str, set: impl FnOnce(&mut noshape_model::params::ParamValues)) -> Entity<crate::feature_dialog::FeatureDialog> {
    cx.update(|cx| {
        let v = view.read(cx);
        let units = v.doc.read(cx).session.doc.settings.units;
        let mut f = v.studio(cx).expect("studio").new_feature(type_id, &units).expect("feature type");
        set(&mut f.params);
        let labeler: crate::feature_dialog::RefLabeler = std::sync::Arc::new(|r: &Reference| format!("{r:?}"));
        cx.new(|cx| crate::feature_dialog::FeatureDialog::new(f, units, labeler, cx))
    })
}

fn pick(cx: &mut TestAppContext, d: &Entity<crate::feature_dialog::FeatureDialog>, r: Reference) {
    cx.update(|cx| d.update(cx, |d, cx| d.add_pick(r, cx)));
}

fn active(cx: &mut TestAppContext, d: &Entity<crate::feature_dialog::FeatureDialog>) -> Option<String> {
    cx.update(|cx| d.read(cx).active_query.clone())
}

fn focus(cx: &mut TestAppContext, d: &Entity<crate::feature_dialog::FeatureDialog>, field: &str) {
    cx.update(|cx| d.update(cx, |d, _| d.user_focus_query(field)));
}

fn picks(cx: &mut TestAppContext, d: &Entity<crate::feature_dialog::FeatureDialog>, field: &str) -> Vec<Reference> {
    cx.update(|cx| d.read(cx).feature.params.query(field).to_vec())
}

const P: [Reference; 3] = [Reference::Plane(DatumPlane::Top), Reference::Plane(DatumPlane::Front), Reference::Plane(DatumPlane::Right)];

/// Auto-advance (user 10-31; live behaviours "psui (2026-10-31, PRIORITY) selection-field auto-advance", request
/// 1650): Mate connector's Origin entity (≤1) passes the focus over Realign, Move and Owner entity to Select owner
/// entity, filled already (rule 3); that one, full, replaces and moves on to Attach to; the last field keeps the
/// focus and replaces (rule 5).
#[gpui::test]
fn mate_connector_fields_advance_over_their_checkboxes(cx: &mut TestAppContext) {
    let (_, view) = part_studio(cx, "advance-mc");
    let d = dialog_of(cx, &view, "mateConnector", |p| {
        p.set("attachmentOption", noshape_model::params::ParamValue::Enum("TO_SELECTION".into()));
        p.set("ownerPart", noshape_model::params::ParamValue::Query(vec![P[2].clone()]));
    });
    focus(cx, &d, "originQuery");
    pick(cx, &d, P[0].clone());
    assert_eq!(active(cx, &d).as_deref(), Some("ownerPart"), "Origin entity full: on to the (filled) owner");
    pick(cx, &d, P[1].clone());
    assert_eq!(picks(cx, &d, "ownerPart"), vec![P[1].clone()], "a full one-pick field replaces");
    assert_eq!(active(cx, &d).as_deref(), Some("attachTo"));
    pick(cx, &d, P[0].clone());
    pick(cx, &d, P[2].clone());
    assert_eq!(active(cx, &d).as_deref(), Some("attachTo"), "the last field keeps the focus");
    assert_eq!(picks(cx, &d, "attachTo"), vec![P[2].clone()]);
}

/// Sheet metal Bend: Bend line (≤1) ⇒ Sheet metal face to bend (≤1) over Hold opposite side; back in the full
/// Bend line (a click), a new pick replaces and moves on again (rule 4).
#[gpui::test]
fn bend_line_advances_and_a_full_one_replaces_and_advances_again(cx: &mut TestAppContext) {
    let (_, view) = part_studio(cx, "advance-bend");
    let d = dialog_of(cx, &view, "sheetMetalBend", |_| {});
    focus(cx, &d, "bendReference");
    pick(cx, &d, P[0].clone());
    assert_eq!(active(cx, &d).as_deref(), Some("face"));
    focus(cx, &d, "bendReference");
    pick(cx, &d, P[1].clone());
    assert_eq!(picks(cx, &d, "bendReference"), vec![P[1].clone()]);
    assert_eq!(active(cx, &d).as_deref(), Some("face"), "refilled: on again");
}

/// The printable knurl (the user's custom feature, tests/fs/knurl_b94.fs): Start and end edges (MaxNumberOfPicks
/// 2) keeps the focus after one edge and jumps over its six controls to Targets after the second. Rule 6: after a
/// chip's ×, refilling to two doesn't advance; once the user clicked Targets and back, the next pick (a full field:
/// the oldest goes) advances again. A third edge into the full field drops the oldest (rule 4).
#[gpui::test]
fn knurl_edges_take_two_then_targets_and_rearm_after_a_removal(cx: &mut TestAppContext) {
    let (_, view) = part_studio(cx, "advance-knurl");
    let mut doc = noshape_model::Document::new();
    let fs = doc.add_tab("Feature Studio 1", noshape_model::TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(include_str!("../../../model/tests/fs/knurl_b94.fs"))));
    let _ = noshape_model::custom::DocumentScripts::from_document(&doc);
    let d = dialog_of(cx, &view, &noshape_model::custom::type_id(fs, "myFeature"), |_| {});
    focus(cx, &d, "edges");
    pick(cx, &d, P[0].clone());
    assert_eq!(active(cx, &d).as_deref(), Some("edges"), "one of two");
    pick(cx, &d, P[1].clone());
    assert_eq!(active(cx, &d).as_deref(), Some("targets"), "two: on to Targets");
    // A third edge into the full field: the oldest goes, and it moves on.
    focus(cx, &d, "edges");
    pick(cx, &d, P[2].clone());
    assert_eq!(picks(cx, &d, "edges"), vec![P[1].clone(), P[2].clone()]);
    assert_eq!(active(cx, &d).as_deref(), Some("targets"));
    // × on the first chip (Targets active): Start and end edges takes the focus; refilled, it stays.
    cx.update(|cx| d.update(cx, |d, _| d.remove_picks("edges", &[0])));
    assert_eq!(active(cx, &d).as_deref(), Some("edges"));
    pick(cx, &d, P[0].clone());
    assert_eq!(active(cx, &d).as_deref(), Some("edges"), "after a removal the refill doesn't advance");
    // Leave and come back: re-armed.
    focus(cx, &d, "targets");
    focus(cx, &d, "edges");
    pick(cx, &d, P[1].clone());
    assert_eq!(active(cx, &d).as_deref(), Some("targets"));
}

/// Revolve's angle handle snaps as live (psb 10-24 rev, 1 px steps: 45 → 47.5365 → 50 → 52.2405 → 52.8758 → 55 →
/// 57.3507 → 57.9137 → 60 → 62.1854 → 62.5070 → 62.9932 → 65): to a multiple of 5° within 2° of it.
#[test]
fn revolve_angles_snap_to_five_degrees_as_live() {
    use super::super::manipulators::snap_degrees;
    for (dragged, shown) in [(47.5365, 47.5365), (48.1, 50.0), (52.2405, 52.2405), (52.8758, 52.8758), (53.2, 55.0), (62.9932, 62.9932), (63.5, 65.0), (359.0, 360.0), (365.0, 360.0)] {
        assert!((snap_degrees(dragged, 200.) - shown).abs() < 1e-9, "{dragged} → {}", snap_degrees(dragged, 200.));
    }
}

/// Extrude's Second end position (std addExtrudeManipulator, live psb 10-24 ex2): a second arrow at the second cap,
/// hit as the second handle; dragging it sets only the second end's Depth (the first stays). Through all's direction
/// arrow flips on a click (live 11-01). Shell and Chamfer show no manipulator (std adds none; live psb 10-24).
#[gpui::test]
fn extrude_second_end_has_its_own_arrow(cx: &mut TestAppContext) {
    use super::super::manipulators::{Handle, Manip};
    let (window, view) = sketch_with(cx, "second-end", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.finish_sketch(w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let sk = v.display.sketches.first().expect("the sketch");
            v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().expect("a region").key.clone() }];
            v.start_feature("extrude", w, cx);
            let d = v.edit.as_ref().expect("the dialog").dialog.clone();
            d.update(cx, |d, _| {
                d.feature.params.set("hasSecondDirection", noshape_model::ParamValue::Bool(true));
                d.feature.params.set("secondDirectionBound", noshape_model::ParamValue::Enum("BLIND".into()));
                d.feature.params.set("secondDirectionDepth", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("10 mm")));
            });
        })
    })
    .unwrap();
    draw(cx, window);
    let (at, to) = view.update(cx, |v, cx| {
        v.sync_manipulator(cx);
        let Some(Manip::Handles([Some(Handle::Arrow(a)), Some(Handle::Arrow(b)), ..])) = v.current_manip(cx) else { panic!("two arrows") };
        assert!(b.secondary && a.dir.dot(b.dir) < 0.0, "the second end's double chevron, the other way");
        let vp = v.viewport.read(cx);
        let tip = b.base + b.dir * b.length;
        let (p0, p1) = (vp.project(tip).unwrap(), vp.project(tip + b.dir * 1e-3).unwrap());
        let along = (p1 - p0).normalize();
        let at = p0 + along * 20.0;
        assert_eq!(vp.manipulator_index_at(at), Some(1), "the second arrow's glyph");
        let to = at + along * 40.0;
        ((at.x, at.y), (to.x, to.y))
    });
    let depths = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let p = &v.edit.as_ref().expect("still editing").dialog.read(cx).feature.params;
            (p.expr("depth").map(|x| x.text.clone()), p.expr("secondDirectionDepth").map(|x| x.text.clone()))
        })
    };
    let (first, _) = depths(cx);
    drag(cx, window, &view, at, to, 4);
    let (depth, second) = depths(cx);
    assert_eq!(depth, first, "the first depth unchanged");
    let second = second.expect("second depth");
    assert!(second != "10 mm" && second.ends_with("mm"), "the second depth dragged: {second}");
    // Through all: a direction arrow on the region; a click on it flips the direction (live 11-01).
    let flipped = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).edit.as_ref().expect("editing").dialog.read(cx).feature.params.bool("oppositeDirection"));
    cx.update(|cx| {
        let d = view.read(cx).edit.as_ref().expect("editing").dialog.clone();
        d.update(cx, |d, _| d.feature.params.set("endBound", noshape_model::ParamValue::Enum("THROUGH_ALL".into())));
    });
    draw(cx, window);
    let before = flipped(cx);
    let at = view.update(cx, |v, cx| {
        v.sync_manipulator(cx);
        let Some(Manip::Handles([Some(Handle::Arrow(a)), ..])) = v.current_manip(cx) else { panic!("the direction arrow") };
        assert!(a.param.is_empty(), "nothing to drag");
        let vp = v.viewport.read(cx);
        let (p0, p1) = (vp.project(a.base).unwrap(), vp.project(a.base + a.dir * 1e-3).unwrap());
        let at = p0 + (p1 - p0).normalize() * 30.0;
        (at.x, at.y)
    });
    click(cx, window, &view, at);
    assert_eq!(flipped(cx), !before, "a click flips it");
    for ty in ["shell", "chamfer"] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            v.cancel_edit(w, cx);
            v.start_feature(ty, w, cx);
        }))
        .unwrap();
        draw(cx, window);
        assert!(cx.update(|cx| view.read(cx).current_manip(cx).is_none()), "{ty} has no manipulator");
    }
}

/// A part from a corner rectangle extruded (the dialog's preview and the commit waited for), its top and bottom faces.
fn block_part(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>, Reference, Reference) {
    let (window, view) = sketch_with(cx, name, Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.finish_sketch(w, cx);
    }))
    .unwrap();
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let sk = v.display.sketches.first().expect("the sketch");
            v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().expect("a region").key.clone() }];
            v.start_feature("extrude", w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    // ✓ once the preview has regenerated (a regeneration without an error makes the dialog dirty).
    wait_until(cx, window, |cx| view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).can_submit()));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    // The commit's regeneration (it waits out a burst's 12 ms first): the preview's context already has the part, so
    // wait for the consumed sketch to hide too (a loaded land server's box test still saw it).
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        v.context.as_ref().is_some_and(|c| !c.bodies.is_empty()) && v.display.sketches.iter().all(|s| s.hidden) && v.display.bodies.iter().any(|b| b.kind == noshape_model::display::BodyKind::Solid)
    });
    // The part's flat faces at its top and bottom.
    let (top, bottom) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let flat_at = |want_top: bool| {
            let zs: Vec<(Reference, f32, f32)> = b
                .face_refs
                .iter()
                .enumerate()
                .filter_map(|(i, r)| {
                    let &(first, count) = b.mesh.face_ranges.get(i)?;
                    let z: Vec<f32> = b.mesh.indices[first as usize..(first + count) as usize].iter().map(|&k| b.mesh.positions[k as usize][2]).collect();
                    let (lo, hi) = z.iter().fold((f32::MAX, f32::MIN), |(a, c), x| (a.min(*x), c.max(*x)));
                    ((hi - lo).abs() < 1e-7).then(|| (r.clone(), lo, hi))
                })
                .collect();
            zs.into_iter().max_by(|a, b| if want_top { a.1.total_cmp(&b.1) } else { b.1.total_cmp(&a.1) }).expect("a flat face").0
        };
        (flat_at(true), flat_at(false))
    });
    (window, view, top, bottom)
}

/// Preview audit (live 11-01): a Shell's removed face shows as it was before the feature, not on the rim the shell
/// leaves (which keeps the face's name): the open Shell dialog counts the picked top face as changed, a face it
/// leaves alone (the bottom) as not.
#[gpui::test]
fn a_shelled_face_is_highlighted_as_before(cx: &mut TestAppContext) {
    let (window, view, top, bottom) = block_part(cx, "shell-pick");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_feature("shell", w, cx);
            let d = v.edit.as_ref().expect("the Shell dialog").dialog.clone();
            d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(vec![top.clone()]), cx));
        })
    })
    .unwrap();
    // The preview over the model before it.
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        v.ui.preview_before.is_some() && v.edit.as_ref().and_then(|e| v.display.feature_state(e.dialog.read(cx).feature.id)).is_some_and(|s| s.status == noshape_model::display::FeatureStatus::Ok)
    });
    let state = cx.update(|cx| {
        let v = view.read(cx);
        let id = v.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
        (v.ui.preview_before.is_some(), id.and_then(|id| v.display.feature_state(id)).map(|s| (s.status, s.message.clone())), v.edit.as_ref().map(|e| e.dialog.read(cx).feature.params.query("entities").to_vec()))
    });
    assert!(state.0 && matches!(state.1, Some((noshape_model::display::FeatureStatus::Ok, _))), "the shell previewed over the model before it: {state:?}");
    let changed = cx.update(|cx| view.read(cx).changed_picks(&[top.clone(), bottom.clone()]));
    assert_eq!(changed, vec![top], "the removed top face, not the bottom");
    // Live 11-01: the removed face filled khaki at half (the opening and the rim) under its outline.
    let drawn = cx.update(|cx| view.read(cx).ui.consumed_lines.as_ref().map(|c| c.5.iter().map(|i| (matches!(i.drawable, noshape_render::Drawable::Mesh(_)), i.color[3])).collect::<Vec<_>>()));
    // (The outline twice: as drawn, and its hidden-only clone, so the face's edges show whole: qa FEEL-BEND-FACE-PICKED-LOOK.)
    assert_eq!(drawn, Some(vec![(true, 0.47), (false, 1.0), (false, 1.0)]), "the removed face's fill, then its outline");
}

/// Live2 feature-dialogs-1119 (Lo-extrude-open.png): a shown sketch's closed rectangle is filled light grey at rest, as
/// while editing it (ours drew its outline only).
#[gpui::test]
fn a_shown_sketchs_closed_loop_is_filled_at_rest(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "sketch-fill-at-rest", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
    draw(cx, window);
    let fills = cx.update(|cx| {
        let v = view.read(cx);
        v.viewport.read(cx).scene().instances.iter().filter(|i| matches!(i.drawable, noshape_render::Drawable::Mesh(_)) && !i.pickable && (i.color[3] - 0.33).abs() < 1e-6).count()
    });
    assert_eq!(fills, 1, "the rectangle's fill at rest");
}

/// Live behaviours "psui exited sketch dots and fill" (c-1, qa SKETCH-ON-FACE-EXITED-LOOK): a rectangle sketched on a
/// part's face is not filled at rest, its lines over the face's own colour (on a plane it is, above).
#[gpui::test]
fn a_shown_sketchs_closed_loop_on_a_face_is_not_filled_at_rest(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "sk-face-rest-fill", Some(starts["block"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![v.display.bodies[0].face_refs[0].clone()];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some());
    let (window, view) = sketch_in_tool(cx, window, view, Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
    draw(cx, window);
    let (on_face, fills) = cx.update(|cx| {
        let v = view.read(cx);
        let on_face = v.display.sketches.iter().filter(|s| !s.hidden).map(|s| s.on_face).collect::<Vec<_>>();
        (on_face, v.viewport.read(cx).scene().instances.iter().filter(|i| matches!(i.drawable, noshape_render::Drawable::Mesh(_)) && !i.pickable && (i.color[3] - 0.33).abs() < 1e-6).count())
    });
    assert_eq!((on_face, fills), (vec![true], 0), "(the shown sketches on a face, rest fills)");
}

/// Live2 feature-dialogs-1119 (rcmp2.png): Extrude's picked top face under its preview takes a warm tint, filled at
/// half under its orange outline (ours had the outline only, the face blue).
#[gpui::test]
fn an_extrudes_picked_face_is_tinted_under_its_preview(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "extrude-face-tint");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_feature("extrude", w, cx);
            let d = v.edit.as_ref().expect("the Extrude dialog").dialog.clone();
            d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(vec![top.clone()]), cx));
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        v.ui.preview_before.is_some() && v.edit.as_ref().and_then(|e| v.display.feature_state(e.dialog.read(cx).feature.id)).is_some_and(|s| s.status == noshape_model::display::FeatureStatus::Ok)
    });
    draw(cx, window);
    let drawn = cx.update(|cx| view.read(cx).ui.consumed_lines.as_ref().map(|c| c.5.iter().map(|i| (matches!(i.drawable, noshape_render::Drawable::Mesh(_)), i.color[3])).collect::<Vec<_>>()));
    // (The outline as drawn and its hidden-only clone, as a Bend's filled face: qa FEEL-BEND-FACE-PICKED-LOOK.)
    assert_eq!(drawn, Some(vec![(true, 0.5), (false, 1.0), (false, 1.0)]), "the picked face's tint, then its outline");
}

/// Live manip-feel-1118 m6, 1122 transform-x-drag: a Transform that moves a part leaves its old place light grey and
/// see-through with no edges, and shows the moved part as it is, opaque in its colour (qa FEEL-TRANSFORM-TRIAD-LOOK).
#[gpui::test]
fn a_transforms_old_place_is_a_light_grey_ghost(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "transform-ghost");
    let body = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part").id.clone());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_feature("transform", w, cx);
            let d = v.edit.as_ref().expect("the Transform dialog").dialog.clone();
            d.update(cx, |d, cx| {
                d.set_value("entities", noshape_model::ParamValue::Query(vec![Reference::Body(body.clone())]), cx);
                d.set_value("transformType", noshape_model::ParamValue::Enum("TRANSLATION_3D".into()), cx);
                d.set_value("dx", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("30 mm")), cx);
            });
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui.transform_ghost.is_empty());
    let shown = cx.update(|cx| view.read(cx).display_for_scene_inner());
    let lin = crate::theme::lin;
    let near = |c: [f32; 4], want: [f32; 4], a: f32| (0..3).all(|i| (c[i] - want[i]).abs() < 1e-4) && (c[3] - a).abs() < 1e-4;
    let ghost = shown.bodies.iter().find(|b| b.id.index & (1 << 31) != 0).expect("the old place's ghost");
    assert!(near(ghost.color, lin(0xc9ced2), 0.6) && ghost.mesh.edges.is_empty(), "the ghost light grey at 0.6 with no edges: {:?}", ghost.color);
    let moved = shown.bodies.iter().find(|b| b.id == body).expect("the moved part");
    let own = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.id == body).map(|b| b.color)).expect("the part");
    assert!(moved.color == own && moved.color[3] == 1.0 && !moved.mesh.edges.is_empty(), "the moved part in its own colour, opaque, with its edges: {:?} (its own {own:?})", moved.color);
}

/// Live manip-feel-1122 (qa TRANSFORM-ROTATE-EDGE-SENSE): a Rotate about one of the moved part's own edges keeps that
/// axis edge lit orange; the old place shows no other edge.
#[gpui::test]
fn a_transforms_axis_edge_on_the_moved_part_stays_lit(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "transform-axis-lit");
    let (body, axis) = cx.update(|cx| {
        let b = view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part").clone();
        let vertical = |pl: &Vec<[f32; 3]>| pl.len() >= 2 && (pl[0][0] - pl[pl.len() - 1][0]).abs() < 1e-6 && (pl[0][1] - pl[pl.len() - 1][1]).abs() < 1e-6;
        let e = b.edge_refs.iter().zip(&b.mesh.edges).find(|(_, pl)| vertical(pl)).map(|(r, _)| r.clone()).expect("a vertical edge");
        (b.id.clone(), e)
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_feature("transform", w, cx);
            let d = v.edit.as_ref().expect("the Transform dialog").dialog.clone();
            d.update(cx, |d, cx| {
                d.set_value("entities", noshape_model::ParamValue::Query(vec![Reference::Body(body.clone())]), cx);
                d.set_value("transformType", noshape_model::ParamValue::Enum("ROTATION".into()), cx);
                d.set_value("transformAxis", noshape_model::ParamValue::Query(vec![axis.clone()]), cx);
            });
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui.transform_ghost.is_empty() && !view.read(cx).ui_busy());
    draw(cx, window);
    let shown = cx.update(|cx| view.read(cx).display_for_scene_inner());
    let ghost = shown.bodies.iter().find(|b| b.id.index & (1 << 31) != 0).expect("the old place's ghost");
    let lit = cx.update(|cx| {
        let v = view.read(cx);
        let scene = v.viewport.read(cx).scene();
        v.built.as_ref().map(|b| b.picks.ids_for(&axis).iter().any(|&id| scene.states.get(id as usize).is_some_and(|st| st & noshape_render::state::SELECTED != 0)))
    });
    assert_eq!((ghost.edge_refs.clone(), ghost.mesh.edges.len(), lit), (vec![axis.clone()], 1, Some(true)), "(the ghost's edges, drawn, the axis edge selected)");
}

/// Live shortcuts-ps-1118: Shift+T (Make transparent) with Part 1 selected makes it transparent and clears the
/// selection, as Isolate does.
#[gpui::test]
fn make_transparent_clears_the_selection(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "transparent-clears");
    let body = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part").id.clone());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![Reference::Body(body.clone())];
            v.isolate(crate::part_studio::behaviors::IsolateMode::Transparent, w, cx);
        })
    })
    .unwrap();
    let (selection, transparent) = cx.update(|cx| {
        let v = view.read(cx);
        (v.selection.clone(), v.ui.isolate.as_ref().map(|i| i.bodies.clone()))
    });
    assert_eq!((selection, transparent), (Vec::new(), Some(vec![body])), "(the selection, the parts made transparent)");
}

/// Live2 ps-feel2-1118: Shift+I with Part 1 selected isolates it and clears the selection; Esc closes the dialog and
/// ends isolate.
#[gpui::test]
fn shift_i_isolates_the_selected_part_and_esc_ends_it(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "shift-i");
    let body = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part").id.clone());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, _| v.selection = vec![Reference::Body(body.clone())]);
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["shift-i"]);
    let state = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        (v.ui.isolate.as_ref().map(|i| (i.mode == crate::part_studio::behaviors::IsolateMode::Isolate, i.bodies.clone())), v.selection.len())
    });
    assert_eq!(state(cx), (Some((true, vec![body.clone()])), 0), "(isolating Part 1, selected after)");
    // Live (248, 79): the graphics area's top left, 3 px down, as the feature dialogs; 220 × 231 with Show internal
    // geometry (live asm/isolate-1122 ps-panel).
    draw(cx, window);
    let panel = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("ps-isolate").expect("the Isolate dialog");
    let origin = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
    let f = |p: gpui_kit::Pixels| f32::from(p).round();
    assert_eq!((f(panel.origin.x - origin.x), f(panel.origin.y - origin.y), f(panel.size.width), f(panel.size.height)), (0., 3., 220., 231.), "(x, y) in the graphics, its size");
    keys(cx, window, &["escape"]);
    assert_eq!(state(cx).0, None, "Esc ends isolate");
}

/// Live keys-1118: Make transparent's slider reopens where it was left; Isolate's is its own (live asm/isolate-1122: 30 %
/// in a new account).
#[gpui::test]
fn the_transparency_slider_is_remembered_per_dialog(cx: &mut TestAppContext) {
    use crate::part_studio::behaviors::IsolateMode;
    let (window, view, _, _) = block_part(cx, "transparency-kept");
    let body = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part").id.clone());
    let open = |cx: &mut TestAppContext, mode: IsolateMode, set: Option<f32>| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.selection = vec![Reference::Body(body.clone())];
                v.isolate(mode, w, cx);
                let shown = v.ui.isolate.as_ref().map(|i| i.opacity);
                if let (Some(f), Some(iso)) = (set, v.ui.isolate.as_mut()) {
                    iso.opacity = f;
                }
                v.exit_isolate(w, cx);
                shown
            })
        })
        .unwrap()
    };
    assert_eq!(open(cx, IsolateMode::Transparent, Some(0.3)), Some(0.0), "first open");
    assert_eq!(open(cx, IsolateMode::Transparent, None), Some(0.3), "Make transparent again");
    assert_eq!(open(cx, IsolateMode::Isolate, None), Some(crate::isolate_ui::KNOB_START), "Isolate");
}

/// Live workflow 02 step 29 (qa FEEL-EXTRUDE-ARROW-HOVER-LOOK): a 50 × 25 rectangle's depth arrow stands at its centre
/// (25, 12.5), where live's pointer finds it.
#[gpui::test]
fn an_extrudes_arrow_stands_at_its_regions_centre(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [50, 25]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "arrow-centre", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.sketches.first().is_some_and(|s| !s.regions.is_empty()));
    let region = cx.update(|cx| {
        let sk = &view.read(cx).display.sketches[0];
        Reference::SketchRegion { sketch: sk.feature, key: sk.regions[0].key.clone() }
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(vec![region]), cx)));
    let base = cx.update(|cx| view.read(cx).current_arrow(cx).map(|a| a.base * 1e3));
    assert!(base.is_some_and(|b| b.distance(glam::DVec3::new(25.0, 12.5, 0.0)) < 0.01), "the arrow's base (mm): {base:?}");
}

/// Live workflow 15 (qa EXTRUDE-ARROW-GONE-WITH-VARIABLE): Depth "#depth", a Variable Studio's 25 mm, keeps the depth
/// arrow, 25 mm long.
#[gpui::test]
fn an_extrude_depth_from_a_variable_studio_keeps_its_arrow(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "depth-variable-arrow");
    cx.update(|cx| {
        let doc = view.read(cx).doc.clone();
        doc.update(cx, |d, _| {
            let depth = noshape_model::variables::VariableDef { name: "depth".into(), expr: noshape_units::ExprValue::new("25 mm"), kind: Some(noshape_model::variables::VarKind::Length), description: String::new(), description_expr: false, import: None, other_values: Default::default() };
            let vs = noshape_model::variables::VariableStudio { variables: vec![depth], insert_into_all: true, ..Default::default() };
            d.session.doc.add_tab("Variable Studio 1", noshape_model::document::TabContent::VariableStudio(vs));
        })
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(vec![top]), cx)));
    let arrow = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).current_arrow(cx).map(|a| (a.param, a.length)));
    assert!(arrow(cx).is_some_and(|(p, _)| p == "depth"), "the face's depth arrow: {:?}", arrow(cx));
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("depth", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("#depth")), cx)));
    let a = arrow(cx);
    assert!(a.is_some_and(|(p, l)| p == "depth" && (l - 0.025).abs() < 1e-9), "#depth: {a:?}");
}

/// Fillet's drag arrow as std edgeBlendCommon.fs addFilletControlManipulator: on a block's top edge (a 90° convex edge)
/// its tip where the round crosses the faces' bisector (offset radius × (1/cos 45° − 1)), into the material; Width
/// drives the width (offset × |n₀ − n₁| = √2 per width), a conic section its own radius; none for a variable radius.
#[gpui::test]
fn fillet_arrow_as_std(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "fillet-arrow");
    let edge = cx.update(|cx| {
        let v = view.read(cx);
        let b = &v.context.as_ref().expect("the part").bodies[0];
        let top = b.shape.bounding_box(false).map(|bb| bb.max.z).unwrap_or(0.0);
        let i = b.shape.edges().iter().position(|e| e.edge_info().is_ok_and(|i| (i.start.z - top).abs() < 1e-7 && (i.end.z - top).abs() < 1e-7)).expect("a top edge");
        Reference::Edge((*b.edges[i]).clone())
    });
    let arrow = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).param_arrow(cx).map(|a| (a.param, a.scale, a.dir, a.length)));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let set = |cx: &mut TestAppContext, k: &str, v: noshape_model::ParamValue| cx.update(|cx| d.update(cx, |d, cx| d.set_value(k, v, cx)));
    set(cx, "entities", noshape_model::ParamValue::Query(vec![edge]));
    let ratio = std::f64::consts::SQRT_2 - 1.0;
    let a = arrow(cx).expect("the radius arrow");
    assert!(a.0 == "radius" && (a.1 - ratio).abs() < 1e-9 && a.2.z < -0.5, "{a:?}");
    set(cx, "crossSection", noshape_model::ParamValue::Enum("CONIC".into()));
    assert!(arrow(cx).is_some_and(|a| a.0 == "nonCircularRadius" && (a.1 - ratio).abs() < 1e-9), "conic: {:?}", arrow(cx));
    set(cx, "crossSection", noshape_model::ParamValue::Enum("CIRCULAR".into()));
    set(cx, "blendControlType", noshape_model::ParamValue::Enum("WIDTH".into()));
    assert!(arrow(cx).is_some_and(|a| a.0 == "width" && (a.1 - ratio / std::f64::consts::SQRT_2).abs() < 1e-9), "width: {:?}", arrow(cx));
    set(cx, "blendControlType", noshape_model::ParamValue::Enum("RADIUS".into()));
    set(cx, "isVariable", noshape_model::ParamValue::Bool(true));
    assert!(arrow(cx).is_none(), "a variable radius: no arrow");
}

/// Projected curve's direction flip (audit-part-studio M11; projectCurves.fs addDropCurveManipulator): Curve to face
/// with a Direction, the Top plane: a flip arrow at its origin along +Z, along −Z with Opposite direction; none for
/// Normal to target.
#[gpui::test]
fn projected_curve_direction_flip_as_std(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "proj-flip");
    let arrow = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).param_arrow(cx).map(|a| (a.base, a.dir, a.flip)));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("projectCurves", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let set = |cx: &mut TestAppContext, k: &str, v: noshape_model::ParamValue| cx.update(|cx| d.update(cx, |d, cx| d.set_value(k, v, cx)));
    set(cx, "curveProjectionType", noshape_model::ParamValue::Enum("CURVE_TO_FACE".into()));
    set(cx, "projectionType", noshape_model::ParamValue::Enum("DIRECTION".into()));
    set(cx, "directionQuery", noshape_model::ParamValue::Query(vec![P[0].clone()]));
    let a = arrow(cx).expect("a flip arrow");
    assert!(a.0.length() < 1e-12 && a.1.z > 0.999 && a.2 == Some("oppositeDirection"), "{a:?}");
    set(cx, "oppositeDirection", noshape_model::ParamValue::Bool(true));
    assert!(arrow(cx).is_some_and(|a| a.1.z < -0.999), "flipped");
    set(cx, "projectionType", noshape_model::ParamValue::Enum("NORMAL_TO_TARGET".into()));
    assert!(arrow(cx).is_none());
}

/// Body draft's pull direction flip (audit-part-studio M11; bodyDraft.fs createManipulators), pulled along Top's
/// normal: with the Top plane as the parting object, at its origin along +Z (−Z flipped); with the block's top face,
/// at the face's middle; Draft on self on the top face, at its point nearest the Top plane (z of the face).
#[gpui::test]
fn body_draft_flip_as_std(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "body-draft-flip");
    let arrow = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).param_arrow(cx).map(|a| (a.base, a.dir, a.flip)));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("bodyDraft", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let set = |cx: &mut TestAppContext, k: &str, v: noshape_model::ParamValue| cx.update(|cx| d.update(cx, |d, cx| d.set_value(k, v, cx)));
    set(cx, "pullDirection", noshape_model::ParamValue::Query(vec![P[0].clone()]));
    set(cx, "partingObject", noshape_model::ParamValue::Query(vec![P[0].clone()]));
    let a = arrow(cx).expect("a flip arrow");
    assert!(a.0.length() < 1e-12 && a.1.z > 0.999 && a.2 == Some("flipPullDirection"), "{a:?}");
    set(cx, "flipPullDirection", noshape_model::ParamValue::Bool(true));
    assert!(arrow(cx).is_some_and(|a| a.1.z < -0.999), "flipped");
    // The top face's middle: the middle of its box.
    let (lo, hi) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let fi = b.face_refs.iter().position(|r| *r == top).expect("the top face");
        let &(first, count) = &b.mesh.face_ranges[fi];
        b.mesh.indices[first as usize..(first + count) as usize].iter().map(|&k| glam::DVec3::from(b.mesh.positions[k as usize].map(|x| x as f64))).fold((glam::DVec3::MAX, glam::DVec3::MIN), |(l, h), q| (l.min(q), h.max(q)))
    });
    set(cx, "partingObject", noshape_model::ParamValue::Query(vec![top.clone()]));
    let a = arrow(cx).expect("a flip arrow");
    assert!((a.0 - (lo + hi) * 0.5).length() < 1e-6, "{a:?} against {:?}", (lo + hi) * 0.5);
    set(cx, "draftOnSelf", noshape_model::ParamValue::Bool(true));
    set(cx, "selectionType", noshape_model::ParamValue::Enum("FACES".into()));
    set(cx, "faces", noshape_model::ParamValue::Query(vec![top]));
    let a = arrow(cx).expect("a flip arrow");
    assert!((a.0.z - hi.z).abs() < 1e-6, "{a:?}");
}

/// A Part Studio with rib variables in its Variable table (the user's 11-02 table): rib_diameter 1.2 mm,
/// rib_empty_space 0.4 mm, rib_count 3, depth = (#rib_diameter + #rib_empty_space) * #rib_count; the panel open.
fn rib_variables(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>, Vec<noshape_model::FeatureId>) {
    let (window, view) = part_studio(cx, name);
    let ids = cx
        .update_window(window, |_, _, cx| {
            view.update(cx, |v, cx| {
                let units = v.doc.read(cx).session.doc.settings.units;
                let mut ids = Vec::new();
                v.edit_config("Add variable", cx, |ps| {
                    for (n, k, val) in [("rib_diameter", "LENGTH", "1.2 mm"), ("rib_empty_space", "LENGTH", "0.4 mm"), ("rib_count", "NUMBER", "3"), ("depth", "LENGTH", "(#rib_diameter + #rib_empty_space) * #rib_count")] {
                        ids.push(noshape_model::variable_table::create(ps, n, k, val, "", None, &units).unwrap());
                    }
                });
                v.config_dirty(cx);
                v.right_panel = Some(super::super::config_panel::RightPanel::Variables);
                cx.notify();
                ids
            })
        })
        .unwrap();
    wait_until(cx, window, |cx| view.read(cx).display.variables.len() == 4);
    draw(cx, window);
    (window, view, ids)
}

fn var_values(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<(String, String)> {
    cx.update(|cx| view.read(cx).studio(cx).map(noshape_model::variable_table::rows).unwrap_or_default().into_iter().map(|r| (r.name, r.value)).collect())
}

/// Live 11-02: a Value holding an expression shows its result while not edited ("4.8 mm" for depth), the
/// expression while edited (a click on the cell).
#[gpui::test]
fn a_variable_expression_shows_its_result_until_edited(cx: &mut TestAppContext) {
    let (window, view, _) = rib_variables(cx, "vt-result");
    let depth = cx.update(|cx| view.read(cx).display.variables.get("depth").copied()).expect("depth evaluated");
    let units = noshape_units::UnitPrefs::default();
    assert_eq!(super::super::config_panel::shown_value(depth, &units), "4.8 mm");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let b = vcx.debug_bounds("pvar-result-3").expect("depth's result over its cell");
    assert!(vcx.debug_bounds("pvar-result-0").is_none(), "a plain value shows as typed");
    vcx.simulate_click(b.center(), Modifiers::default());
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    assert!(vcx.debug_bounds("pvar-result-3").is_none(), "editing: the expression");
}

/// Live fv-1121 dark: an edited Value cell greys its column's header (#d7d7d7 light, #333333 dark) and shows its
/// value in a chip over the row above, as a Variable Studio's.
#[gpui::test]
fn an_edited_value_greys_its_header_and_shows_its_value_above(cx: &mut TestAppContext) {
    let (window, view, _) = rib_variables(cx, "vt-focus-head");
    let column = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| view.read(cx).focused_var_column(w, cx)).unwrap();
    assert_eq!(column(cx), None);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    assert!(vcx.debug_bounds("pvar-old-value").is_none(), "no chip at rest");
    let cell = vcx.debug_bounds("pvar-result-3").expect("depth's result over its cell");
    vcx.simulate_click(cell.center(), Modifiers::default());
    draw(cx, window);
    assert_eq!(column(cx), Some(2), "the Value column");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let chip = vcx.debug_bounds("pvar-old-value").expect("the value's chip");
    assert!(chip.bottom() <= cell.top() && chip.top() >= cell.top() - px(34.), "over the row above: {chip:?} {cell:?}");
    assert_eq!(crate::theme::p().table_header_focus, gpui_kit::rgb(0xd7d7d7));
}

/// The user's 11-02: `#ri` in a Value cell lists the variables above it that match, with their values; ↓ and Enter
/// insert one and commit the cell (live 11-02); Esc closes the list.
#[gpui::test]
fn a_value_cell_completes_a_variable(cx: &mut TestAppContext) {
    let (window, view, ids) = rib_variables(cx, "vt-complete");
    let key = format!("pvar-value-{}", ids[3]);
    let st = cx.update(|cx| view.read(cx).panel_inputs.get(&key).map(|e| e.0.clone())).expect("depth's value input");
    cx.update_window(window, |_, w, cx| {
        st.update(cx, |s, cx| {
            s.focus(w, cx);
            s.set_value("", w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["#", "r", "i"]);
    let entries = cx.update(|cx| view.read(cx).ui.cell_completion.as_ref().map(|c| c.entries.iter().map(|e| e.insertion().0).collect::<Vec<_>>()));
    let entries = entries.expect("the list");
    assert_eq!(entries.len(), 3, "the three rib variables: {entries:?}");
    assert!(entries.iter().all(|e| e.starts_with("#rib_")), "{entries:?}");
    let k = entries.iter().position(|e| e == "#rib_diameter").expect("rib_diameter listed");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).ui.cell_completion.is_none()), "Esc closes it");
    keys(cx, window, &["backspace", "i"]);
    for _ in 0..k {
        keys(cx, window, &["down"]);
    }
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).ui.cell_completion.is_none()));
    assert_eq!(var_values(cx, &view)[3], ("depth".to_string(), "#rib_diameter".to_string()), "inserted and committed");
}

/// Live 11-02: a header's right edge drags its column's width (all tables'), at least 40 px.
#[gpui::test]
fn variable_table_columns_resize(cx: &mut TestAppContext) {
    let (window, _view, _) = rib_variables(cx, "vt-resize");
    let before = crate::ui::var_col_widths();
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let h = vcx.debug_bounds("pvar-col-2").expect("the Value header's handle");
    let drag = |vcx: &mut gpui_kit::VisualTestContext, from: Point<gpui_kit::Pixels>, dx: f32| {
        vcx.simulate_mouse_down(from, gpui_kit::MouseButton::Left, Modifiers::default());
        for k in 1..=4 {
            vcx.simulate_mouse_move(gpui_kit::point(from.x + px(dx * k as f32 / 4.), from.y), gpui_kit::MouseButton::Left, Modifiers::default());
        }
        vcx.simulate_mouse_up(gpui_kit::point(from.x + px(dx), from.y), gpui_kit::MouseButton::Left, Modifiers::default());
    };
    drag(&mut vcx, h.center(), 60.);
    draw(cx, window);
    let after = crate::ui::var_col_widths();
    assert!((after[2] - before[2] - 60.).abs() <= 3., "{before:?} -> {after:?}");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let h = vcx.debug_bounds("pvar-col-0").expect("the Name header's handle");
    drag(&mut vcx, h.center(), -300.);
    draw(cx, window);
    assert_eq!(crate::ui::var_col_widths()[0], crate::ui::VAR_COL_MIN, "no narrower than 40 px");
}

/// Live 11-02: a row dragged by its Name cell moves its Variable feature (depth dropped over rib_diameter's
/// upper half goes first, and fails: its variables come after it).
#[gpui::test]
fn a_variable_row_drags_to_reorder(cx: &mut TestAppContext) {
    let (window, view, ids) = rib_variables(cx, "vt-drag");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let from = vcx.debug_bounds("pvar-name-drag-3").expect("depth's name").center();
    let to = vcx.debug_bounds("pvar-name-drag-0").expect("rib_diameter's name");
    let to = gpui_kit::point(to.center().x, to.top() + px(5.));
    vcx.simulate_mouse_down(from, gpui_kit::MouseButton::Left, Modifiers::default());
    for k in 1..=6 {
        let t = k as f32 / 6.;
        vcx.simulate_mouse_move(gpui_kit::point(from.x, from.y + (to.y - from.y) * t), gpui_kit::MouseButton::Left, Modifiers::default());
    }
    vcx.simulate_mouse_up(to, gpui_kit::MouseButton::Left, Modifiers::default());
    draw(cx, window);
    let names: Vec<String> = var_values(cx, &view).into_iter().map(|v| v.0).collect();
    assert_eq!(names, ["depth", "rib_diameter", "rib_empty_space", "rib_count"]);
    // The press that started the drag left no name focused with its text selected.
    let key = format!("pvar-name-{}", ids[3]);
    let focused = cx.update_window(window, |_, w, cx| view.read(cx).panel_inputs.get(&key).is_some_and(|e| e.0.read(cx).focus_handle(cx).is_focused(w))).unwrap();
    assert!(!focused, "the dragged row's name isn't left focused");
    wait_until(cx, window, |cx| view.read(cx).display.feature_state(ids[3]).is_some_and(|s| s.status == noshape_model::display::FeatureStatus::Error));
}

/// Frame trim (Face)'s keep-side flip (audit-part-studio M11; frameTrim.fs addKeepSideManipulator): with the Top
/// plane as the tool and the block (standing on it) to trim, at the plane's point nearest the part (z 0) along +Z,
/// along −Z with Opposite side.
#[gpui::test]
fn frame_trim_flip_as_std(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "frame-trim-flip");
    let part = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone()))).expect("the part");
    let arrow = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).param_arrow(cx).map(|a| (a.base, a.dir, a.flip)));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("frameTrim", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let set = |cx: &mut TestAppContext, k: &str, v: noshape_model::ParamValue| cx.update(|cx| d.update(cx, |d, cx| d.set_value(k, v, cx)));
    set(cx, "trimType", noshape_model::ParamValue::Enum("FACE".into()));
    set(cx, "targets", noshape_model::ParamValue::Query(vec![part]));
    set(cx, "tool", noshape_model::ParamValue::Query(vec![P[0].clone()]));
    let a = arrow(cx).expect("a flip arrow");
    assert!(a.0.z.abs() < 1e-9 && a.1.z > 0.999 && a.2 == Some("flipTrim"), "{a:?}");
    set(cx, "flipTrim", noshape_model::ParamValue::Bool(true));
    assert!(arrow(cx).is_some_and(|a| a.1.z < -0.999), "flipped");
}

/// Live 1714 (dom/psui-audit-rmb-graphics-curve.txt): a curve's graphics menu: Edit Composite curve 1… · Edit
/// Sketch 1… · Show dimensions · Show dependencies… | Hide Curve 1 · Isolate… · Make transparent… | Create Drawing
/// of Curve 1… | Select ▸ · Select other… | Zoom to fit | Delete Composite curve 1 (no Fillet / Chamfer, analysis,
/// Copy, Export, Zoom to selection, appearance or material).
#[gpui::test]
fn a_curves_graphics_menu_as_live(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "curve-menu", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    draw(cx, window);
    let line = cx.update(|cx| {
        let sk = view.read(cx).display.sketches.first().expect("the sketch").clone();
        Reference::SketchEntity { sketch: sk.feature, entity: sk.curves.first().expect("the line").entity }
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.start_feature("compositeCurve", w, cx);
            let d = v.edit.as_ref().expect("the dialog").dialog.clone();
            d.update(cx, |d, cx| d.set_value("edges", noshape_model::ParamValue::Query(vec![line.clone()]), cx));
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).can_submit()));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().any(|b| b.kind == noshape_model::display::BodyKind::Wire));
    let edge = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Wire).and_then(|b| b.edge_refs.first().cloned())).expect("the curve's edge");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| v.open_context_menu(Some(noshape_scene::builder::PickTarget { reference: edge.clone(), kind: noshape_scene::builder::PickKind::Edge, anchor: None }), gpui_kit::point(px(600.), px(400.)), w, cx))
    })
    .unwrap();
    draw(cx, window);
    let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
    let want = [
        "Edit Composite curve 1…", "Edit Sketch 1…", "Show dimensions", "Show dependencies of Composite curve 1…", "———",
        "Hide Curve 1", "Isolate…", "Make transparent…", "———", "Create Drawing of Curve 1…", "———",
        "Select", "Select other…", "———", "Zoom to fit", "———", "Delete Composite curve 1",
    ];
    assert_eq!(labels, want);
}

/// Live (ui-parity fillet-asymmetric-toggle): an edge clicked at its midpoint (an implicit point on it) lights up the
/// feature that made the edge, as the edge itself does.
#[gpui::test]
fn an_edge_midpoint_lights_up_its_feature(cx: &mut TestAppContext) {
    let (_window, view, _top, _) = block_part(cx, "edge-mid-related");
    let (edge, maker) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let e = b.edge_refs.first().expect("an edge").clone();
        let Reference::Edge(n) = &e else { panic!("an edge: {e:?}") };
        (e.clone(), noshape_ops::studio::edge_creator(n, v.studio(cx).expect("the studio")).expect("its feature"))
    });
    view.update(cx, |v, _| v.selection = vec![Reference::Implicit { on: Box::new(edge), inference: noshape_model::reference::Inference::Midpoint }]);
    assert!(cx.update(|cx| view.read(cx).related_features(cx).contains(&maker)));
}

/// A part edge's midpoint hovered (live hover-matrix-1121 mid-paths, user 11-21 shots hover-point*): only the point
/// lights (never the edge), as the hovered point marker (an 11 px disc, a darker ring, a light centre; solid orange
/// #ffb32e on a selected edge), on the edge or off it on the face beside it.
#[gpui::test]
fn a_hovered_edge_midpoint_lights_only_the_point(cx: &mut TestAppContext) {
    let (_window, view, top, _) = block_part(cx, "edge-mid-rest");
    let (edge, on_edge, on_face) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let e = b.edge_refs.iter().find(|e| v.edge_midpoint(e).is_some()).expect("an edge with ends").clone();
        let picks = &v.built.as_ref().expect("built").picks;
        let target = |r: &Reference| picks.ids_for(r).first().and_then(|id| picks.get(*id).cloned()).expect("its pick");
        (e.clone(), target(&e), target(&top))
    });
    let look = |cx: &mut TestAppContext, pick: &crate::scene_builder::PickTarget, selected: bool| {
        view.update(cx, |v, cx| {
            v.selection = if selected { vec![edge.clone()] } else { Vec::new() };
            v.ui.hover_pick = Some(pick.clone());
            v.hover = Some(Reference::Implicit { on: Box::new(edge.clone()), inference: noshape_model::reference::Inference::Midpoint });
            let lit = v.highlighted_hover(cx);
            let marker: Vec<(u32, f32, bool)> = v.midpoint_sprites(cx).iter().map(|p| (u32::from_be_bytes([0, p.color[0], p.color[1], p.color[2]]), p.size, p.style & noshape_render::PointSprite::RING != 0)).collect();
            (lit, marker)
        })
    };
    let [disc, ring, centre] = crate::theme::viewport::POINT_HOVER;
    for (pick, at) in [(&on_edge, "on the edge"), (&on_face, "on the face beside it")] {
        let (lit, marker) = look(cx, pick, false);
        assert!(!lit.contains(&edge), "{at}: the edge stays dark ({lit:?})");
        assert_eq!(marker, vec![(disc, 11.0, false), (ring, 8.0, true), (centre, 5.0, false)], "{at}: the hovered point marker");
    }
    let (lit, marker) = look(cx, &on_edge, true);
    assert!(!lit.contains(&edge), "on a selected edge: not lit by the hover ({lit:?})");
    assert_eq!(marker.last(), Some(&(0xffb32e, 5.0, false)), "on a selected edge: a solid orange centre ({marker:?})");
}

/// A new Extrude on a part's face previews as Add from the first preview: it waits for std's editing logic (which
/// turns New into Add) instead of showing the New result, a separate part, until then (ui-parity extrude-remove-tab).
#[gpui::test]
fn a_new_extrude_on_a_face_never_previews_as_new(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "no-new-flash");
    view.update(cx, |v, _| v.selection = vec![top.clone()]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Extrude dialog").dialog.clone());
    let mut most = 0;
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_secs(30) {
        most = most.max(cx.update(|cx| view.read(cx).display.bodies.len()));
        if cx.update(|cx| dialog.read(cx).feature.params.enum_("operationType") == "ADD" && !view.read(cx).ui_busy()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        draw(cx, window);
    }
    assert_eq!(cx.update(|cx| (dialog.read(cx).feature.params.enum_("operationType").to_string(), view.read(cx).display.bodies.len())), ("ADD".to_string(), 1));
    assert_eq!(most, 1, "no New preview with a second part on the way");
}

/// A selected face's boundary: its four edges outlined (found from its pick id, scene builder `face_at`), no other
/// edge of the part.
#[gpui::test]
fn a_selected_faces_edges_are_outlined(cx: &mut TestAppContext) {
    let (_window, view, top, _) = block_part(cx, "face-outline");
    let (outlined, expected) = cx.update(|cx| {
        let v = view.read(cx);
        let built = v.built.as_ref().expect("a scene");
        let scene = crate::scene_builder::with_highlights(built, &v.display, std::slice::from_ref(&top), None, &[], Vec::new(), Default::default(), v.gpu.uploader());
        let edges: Vec<u32> = (1..scene.states.len() as u32).filter(|&id| built.picks.get(id).is_some_and(|t| t.kind == crate::scene_builder::PickKind::Edge)).collect();
        let outlined: Vec<u32> = edges.iter().copied().filter(|&id| scene.states[id as usize] & noshape_render::scene::state::OUTLINE != 0).collect();
        let expected: Vec<u32> = crate::scene_builder::face_edges(&v.display, &top).iter().flat_map(|e| built.picks.ids_for(e).to_vec()).collect();
        (outlined, expected)
    });
    let mut expected = expected;
    expected.sort_unstable();
    assert_eq!(outlined.len(), 4, "the top face's four edges: {outlined:?}");
    assert_eq!(outlined, expected);
}

/// A plane picked in an open dialog's field takes a light amber (live feel/mirror 33: 20 % of the selection colour
/// over its own fill), a plane selected without a dialog the solid amber (live selection/01).
#[gpui::test]
fn a_plane_picked_in_a_dialog_is_lighter_than_a_selected_plane(cx: &mut TestAppContext) {
    let (_window, view, _, _) = block_part(cx, "plane-picked-fill");
    let fills = cx.update(|cx| {
        let v = view.read(cx);
        let built = v.built.as_ref().expect("a scene");
        let right = v.display.planes.iter().find(|p| p.name == "Right").expect("the Right plane").reference.clone();
        [false, true].map(|dialog| {
            let scene = crate::scene_builder::with_highlights(built, &v.display, std::slice::from_ref(&right), None, &[], Vec::new(), crate::scene_builder::Highlighting { dialog, ..Default::default() }, v.gpu.uploader());
            scene.instances.iter().map(|i| i.color[3]).filter(|a| [crate::theme::viewport::PLANE_SELECTED_ALPHA, crate::theme::viewport::PLANE_PICKED_ALPHA].contains(a)).collect::<Vec<f32>>()
        })
    });
    assert_eq!(fills, [vec![crate::theme::viewport::PLANE_SELECTED_ALPHA], vec![crate::theme::viewport::PLANE_PICKED_ALPHA]]);
}

/// Live 1993 (b): Edit healthy moment on a broken chip (the Fillet moved above its Extrude) opens the Repair panel at
/// the last moment the pick held, the one before the move.
#[gpui::test]
fn edit_healthy_moment_opens_the_moment_before_the_break(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "healthy-moment", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let tab = cx.update(|cx| view.read(cx).tab);
    let (fillet, edge) = cx.update(|cx| {
        let f = view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Fillet 1").unwrap().clone();
        (f.id, f.params.query("entities")[0].clone())
    });
    view.update(cx, |v, cx| {
        v.doc.update(cx, |doc, cx| {
            doc.edit_studio(tab, "Move Fillet 1", cx, |ps| {
                let f = ps.features.remove(2);
                ps.features.insert(1, f);
                Some(())
            })
        });
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    view.update(cx, |v, cx| v.edit_healthy_moment(fillet, edge, cx));
    wait_until(cx, window, |cx| crate::repair_mode::is_open(&view.read(cx).doc, cx));
    assert!(cx.update(|cx| crate::repair_mode::is_open(&view.read(cx).doc, cx)), "the Repair panel at the moment before the move");
}

/// Live 1993 (c): the Extrude a Fillet's edge came from deleted, its chip reads "Missing Edge", in the error state.
#[gpui::test]
fn a_chip_whose_creator_is_deleted_reads_missing_edge(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "deleted-creator-chip", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let tab = cx.update(|cx| view.read(cx).tab);
    view.update(cx, |v, cx| {
        v.doc.update(cx, |doc, cx| {
            doc.edit_studio(tab, "Delete Extrude 1", cx, |ps| {
                ps.features.retain(|f| f.name != "Extrude 1");
                Some(())
            })
        });
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let fillet = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Fillet 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(fillet, w, cx))).unwrap();
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Fillet dialog").dialog.clone());
    let field = cx.update_window(window, |_, w, cx| {
        let state = dialog.read(cx).ui_state(w, cx);
        state["fields"].as_array().unwrap().iter().find(|f| f["kind"] == "query").cloned().unwrap()
    }).unwrap();
    assert_eq!((field["items"].clone(), field["state"].clone()), (serde_json::json!(["Missing Edge"]), serde_json::json!("error")));
}

/// Live 1990 (5.15): a Fillet moved above the Extrude whose edge it takes: its chip is that edge made by a later
/// feature (grey italic, not missing).
#[gpui::test]
fn a_chip_made_by_a_later_feature(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "later-chip", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let fillet = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Fillet 1").unwrap().id);
    let open = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(fillet, w, cx))).unwrap();
        let d = view.update(cx, |v, _| v.edit.as_ref().expect("the Fillet dialog").dialog.clone());
        cx.update(|cx| {
            let d = d.read(cx);
            let edge = d.feature.params.query("entities")[0].clone();
            ((d.later)(&edge), d.ref_missing(&edge))
        })
    };
    assert_eq!(open(cx), (false, false), "in place, its edge is an earlier feature's");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
    let tab = cx.update(|cx| view.read(cx).tab);
    view.update(cx, |v, cx| {
        v.doc.update(cx, |doc, cx| {
            doc.edit_studio(tab, "Move Fillet 1", cx, |ps| {
                let f = ps.features.remove(2);
                ps.features.insert(1, f);
                Some(())
            })
        });
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!(open(cx), (true, false), "above the Extrude: made by a later feature, not missing");
}

/// Live (ui-parity measure-fillet-face): an edge between an Extrude face and a Fillet face is Fillet 1's (the edge
/// exists since the later of its faces): selected, it lights up Fillet 1, not Extrude 1.
#[gpui::test]
fn a_fillets_tangent_edge_lights_up_the_fillet(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "fillet-tangent-edge", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (edge, extrude, fillet) = cx.update(|cx| {
        let v = view.read(cx);
        let id = |name: &str| v.studio(cx).unwrap().features.iter().find(|f| f.name == name).unwrap().id;
        let (extrude, fillet) = (id("Extrude 1"), id("Fillet 1"));
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let edge = b
            .edge_refs
            .iter()
            .find(|e| match e {
                Reference::Edge(noshape_model::reference::TopoName::Edge { faces, .. }) => {
                    let made: Vec<_> = faces.iter().filter_map(|f| f.creator()).collect();
                    made.contains(&extrude) && made.contains(&fillet)
                }
                _ => false,
            })
            .expect("a tangent edge of the fillet")
            .clone();
        (edge, extrude, fillet)
    });
    view.update(cx, |v, _| v.selection = vec![edge]);
    let related = cx.update(|cx| view.read(cx).related_features(cx));
    assert!(related.contains(&fillet) && !related.contains(&extrude), "{related:?}");
}

/// Live (ui-parity draft-on-side-face): a face picked in the graphics fills Draft's Neutral plane and the next field,
/// Entities to draft, becomes active with the keyboard (the press in the graphics had taken it from the dialog).
#[gpui::test]
fn a_pick_that_moves_on_gives_the_next_field_the_keyboard(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "pick-advance-focus");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("draft", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Draft dialog").dialog.clone());
    let focus = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)["focus"].clone()).unwrap();
    assert_eq!(focus(cx)["label"], "Neutral plane");
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.keyboard_left = true;
        d.add_pick(top, cx);
    }));
    draw(cx, window);
    assert_eq!((focus(cx)["kind"].clone(), focus(cx)["label"].clone()), ("field".into(), "Entities to draft".into()));
}

/// Live 1954: the menu of an edge's midpoint point (the pointer rested on it) is a corner's without Delete part,
/// material and appearance; the edge itself still has Fillet… and Chamfer….
#[gpui::test]
fn an_edge_midpoint_has_the_points_menu(cx: &mut TestAppContext) {
    let (window, view, _top, _) = block_part(cx, "midpoint-menu");
    let edge = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).and_then(|b| b.edge_refs.first().cloned())).expect("an edge");
    let menu = |cx: &mut TestAppContext, r: Reference, kind| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(Some(noshape_scene::builder::PickTarget { reference: r, kind, anchor: None }), gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
        draw(cx, window);
        cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default())
    };
    let of_edge = menu(cx, edge.clone(), noshape_scene::builder::PickKind::Edge);
    assert!(of_edge.iter().any(|l| l == "Fillet…") && of_edge.iter().any(|l| l == "Delete Part 1…"), "{of_edge:?}");
    let point = Reference::Implicit { on: Box::new(edge), inference: noshape_model::reference::Inference::Midpoint };
    let of_point = menu(cx, point, noshape_scene::builder::PickKind::Vertex);
    for gone in ["Fillet…", "Chamfer…", "Zoom to selection", "Delete Part 1…", "Assign material for Part 1…", "Edit appearance for Part 1…"] {
        assert!(!of_point.iter().any(|l| l == gone), "{gone} in {of_point:?}");
    }
    assert!(of_point.iter().any(|l| l == "Delete Extrude 1") && of_point.iter().any(|l| l == "Edit Extrude 1…"), "{of_point:?}");
}

/// Live multi-part-menu-1122 (qa FEEL-PS-MULTI-PART-MENU): the tops of two parts selected, the menu on either top is the
/// two parts' (live's 26 rows less the cloud's Add comment and its separator), none greyed; on Part 1's unselected front
/// face only the face count grows.
#[gpui::test]
fn two_parts_faces_give_the_two_parts_menu(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 40]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "corner_rectangle": [[60, 0], [80, 20]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }
    ] });
    let (window, view) = part_studio_from(cx, "two-parts-menu", Some(start));
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count() == 2);
    // A part's face whose every vertex has `axis` at `value`.
    let face = |cx: &mut TestAppContext, part: usize, axis: usize, value: f32| {
        cx.update(|cx| {
            let v = view.read(cx);
            let b = v.display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).nth(part).expect("the part");
            b.face_refs
                .iter()
                .enumerate()
                .find(|(fi, _)| b.mesh.face_ranges.get(*fi).is_some_and(|&(first, count)| b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| (b.mesh.positions[k as usize][axis] - value).abs() < 1e-6)))
                .map(|(_, r)| r.clone())
                .expect("the face")
        })
    };
    let (top1, top2, front1) = (face(cx, 0, 2, 0.02), face(cx, 1, 2, 0.01), face(cx, 0, 1, 0.0));
    view.update(cx, |v, cx| {
        v.selection = vec![top1.clone(), top2.clone()];
        v.refresh_scene(false, cx);
    });
    let menu = |cx: &mut TestAppContext, r: Reference| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(Some(noshape_scene::builder::PickTarget { reference: r, kind: noshape_scene::builder::PickKind::Face, anchor: None }), gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
        draw(cx, window);
        let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
        view.update(cx, |v, cx| v.close_context_menu(cx));
        labels
    };
    let live = |faces: usize| {
        let s = "Hide 2 parts|Isolate…|Make transparent…|Section view…|Check interference…|———|Copy 2 items|———|Export…|———|Clear selection|Select|Select other…|———|Zoom to fit|Zoom to selection|View normal to|———|Delete 2 parts…|Delete 2 features|———|Assign material for 2 parts…|Edit appearance for 2 parts…";
        let mut rows: Vec<String> = s.split('|').map(String::from).collect();
        rows.push(format!("Add appearance to {faces} faces…"));
        rows
    };
    assert_eq!(menu(cx, top1.clone()), live(2), "on Part 1's selected top");
    assert_eq!(menu(cx, top2.clone()), live(2), "on Part 2's selected top");
    assert_eq!(menu(cx, front1), live(3), "on Part 1's unselected front");
    assert_eq!(cx.update(|cx| view.read(cx).selection.len()), 2, "the selection stays at the two tops");
    // Part 1's row and Part 2's top (live appearance-1123 q3-part-plus-face-menu): the two parts' menu too.
    let part1 = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.face_refs.contains(&top1)).map(|b| b.id.clone()).expect("Part 1"));
    view.update(cx, |v, cx| {
        v.selection = vec![Reference::Body(part1), top2.clone()];
        v.refresh_scene(false, cx);
    });
    let rows = menu(cx, top2);
    let wanted = ["Hide 2 parts", "Copy 2 items", "Delete 2 parts…", "Edit appearance for 2 parts…"];
    assert!(wanted.iter().all(|w| rows.iter().any(|r| r == w)), "a part row and a face: {rows:?}");
}

/// Live appearance-1123 (qa FEEL-APPEAR-PANEL-FA-ROWS): each face appearance sits under its part in the Appearances
/// panel, a feature's named "Face appearance N" too, with its "N selections" count.
#[gpui::test]
fn face_appearances_sit_under_their_parts(cx: &mut TestAppContext) {
    use noshape_model::partstudio::{AppearanceTarget, FaceAppearance};
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 40]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "corner_rectangle": [[60, 0], [80, 20]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }
    ] });
    let (window, view) = part_studio_from(cx, "fa-rows", Some(start));
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count() == 2);
    let (face1, extrude2) = cx.update(|cx| {
        let v = view.read(cx);
        let s = v.studio(cx).unwrap();
        let e1 = s.features.iter().find(|f| f.name == "Extrude 1").unwrap().id;
        let face = v.display.bodies.iter().flat_map(|b| b.face_refs.iter()).find(|r| r.depends_on() == Some(e1)).cloned().unwrap();
        (face, s.features.iter().find(|f| f.name == "Extrude 2").unwrap().id)
    });
    view.update(cx, |v, cx| {
        let tab = v.tab;
        v.doc.update(cx, |doc, cx| {
            doc.edit_studio(tab, "Add appearances", cx, |ps| {
                // Part 2's first, so the panel's order isn't the list's.
                ps.appearances.push(FaceAppearance::new(AppearanceTarget::Feature(extrude2), [0.0, 1.0, 0.0, 1.0]));
                ps.appearances.push(FaceAppearance::new(AppearanceTarget::Faces(vec![face1]), [0.0, 0.0, 1.0, 1.0]));
            })
        });
        v.right_panel = Some(super::super::config_panel::RightPanel::Appearance);
        cx.notify();
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let top = |cx: &mut TestAppContext, label: &str| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("ap-row:{label}").into_boxed_str())).map(|b| f32::from(b.top()));
    let order: Vec<Option<f32>> = ["Part 1", "Face appearance 2", "Part 2", "Face appearance 1"].iter().map(|l| top(cx, l)).collect();
    assert!(order.iter().all(Option::is_some) && order.windows(2).all(|w| w[0] < w[1]), "Part 1, its face appearance, Part 2, Extrude 2's: {order:?}");
}

/// Live core/appear-subtract-1124: a Subtract uses up the tool body whose Extrude has a feature appearance; the panel
/// lists only Part 1, no row for the appearance (its faces are gone; the cut faces are the Boolean's).
#[gpui::test]
fn a_used_up_tools_feature_appearance_has_no_row(cx: &mut TestAppContext) {
    use noshape_model::partstudio::{AppearanceTarget, FaceAppearance};
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 40]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "corner_rectangle": [[30, 10], [60, 30]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
        { "type": "booleanBodies", "name": "Boolean 1", "params": { "operationType": "SUBTRACTION", "targets": ["part 0"], "tools": ["part 1"] } }
    ] });
    let (window, view) = part_studio_from(cx, "fa-used-up", Some(start));
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count() == 1);
    let extrude2 = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 2").unwrap().id);
    view.update(cx, |v, cx| {
        let tab = v.tab;
        v.doc.update(cx, |doc, cx| {
            doc.edit_studio(tab, "Add appearance", cx, |ps| ps.appearances.push(FaceAppearance::new(AppearanceTarget::Feature(extrude2), [0.37, 0.65, 0.24, 1.0])))
        });
        v.right_panel = Some(super::super::config_panel::RightPanel::Appearance);
        cx.notify();
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let row = |cx: &mut TestAppContext, label: &str| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("ap-row:{label}").into_boxed_str())).is_some();
    assert_eq!((row(cx, "Part 1"), row(cx, "Face appearance 1")), (true, false));
}

/// Live2 nav-feel-1119: "Where used for Part 1…" carries its icon (a part with a ?) in the face, edge and vertex menus.
#[gpui::test]
fn where_used_has_its_icon(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "where-used-icon");
    let (edge, vertex) = cx.update(|cx| {
        let b = view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).cloned().expect("the part");
        (b.edge_refs[0].clone(), b.vertex_refs[0].clone())
    });
    let mut icons = Vec::new();
    for (r, kind) in [(top, noshape_scene::builder::PickKind::Face), (edge, noshape_scene::builder::PickKind::Edge), (vertex, noshape_scene::builder::PickKind::Vertex)] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(Some(noshape_scene::builder::PickTarget { reference: r, kind, anchor: None }), gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
        draw(cx, window);
        icons.push(cx.update(|cx| view.read(cx).context_menu.as_ref().and_then(|(_, m, _)| m.read(cx).named_icon("Where used for Part 1…")).map(|n| n.to_string())));
        view.update(cx, |v, cx| v.close_context_menu(cx));
    }
    assert_eq!(icons, vec![Some("menu-where-used".to_string()); 3], "face, edge, vertex");
}

/// Live2 nav-feel-1119 L3-line: a shown sketch's line right-clicked a quarter along has Copy sketch entities, Curve/surface
/// analysis…, Select other… alone, Zoom to selection and Align view; on its midpoint (within 4 px) none of those but
/// Select ▸ with Select other…; both have Create Drawing and the hide rows.
#[gpui::test]
fn a_sketch_lines_menu_and_its_midpoints(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "line-midpoint-menu", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.finish_sketch(w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let (sketch, entity, a, b) = cx.update(|cx| {
        let s = view.read(cx).display.sketches.first().cloned().expect("the sketch");
        let c = s.curves.iter().find(|c| c.points.len() == 2).cloned().expect("a line");
        (s.feature, c.entity, c.points[0], c.points[1])
    });
    let menu = |cx: &mut TestAppContext, t: f64| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let at = v.viewport.read(cx).project(glam::DVec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2])).lerp(glam::DVec3::new(f64::from(b[0]), f64::from(b[1]), f64::from(b[2])), t)).expect("on screen");
            v.viewport.update(cx, |vp, _| vp.place_pointer(at));
            let target = noshape_scene::builder::PickTarget { reference: Reference::SketchEntity { sketch, entity }, kind: noshape_scene::builder::PickKind::SketchCurve, anchor: None };
            v.open_context_menu(Some(target), gpui_kit::point(px(at.x as f32), px(at.y as f32)), w, cx);
        }))
        .unwrap();
        draw(cx, window);
        let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
        view.update(cx, |v, cx| v.close_context_menu(cx));
        labels
    };
    let has = |rows: &[String], l: &str| rows.iter().any(|r| r.starts_with(l));
    let marks = |rows: &[String]| ["Copy sketch entities", "Curve/surface analysis…", "Zoom to selection", "Align view", "Select other…", "Create Drawing", "Hide all sketches"].map(|l| has(rows, l)).to_vec();
    let line = menu(cx, 0.25);
    let mid = menu(cx, 0.5);
    let select = |rows: &[String]| rows.iter().any(|r| r == "Select");
    assert_eq!((marks(&line), select(&line)), (vec![true; 7], false), "a quarter along: {line:?}");
    assert_eq!((marks(&mid), select(&mid)), (vec![false, false, false, false, true, true, true], true), "the midpoint: {mid:?}");
}

/// Live ctx-menus-1117 (face-selected, two-faces, face-and-edge; Add comment left out, cloud): with the face clicked
/// selected the menu gains Clear selection above Select; two faces count in "Add appearance to 2 faces…" and drop the
/// one planar face's sketch / plane / Extrude / Revolve rows and Export as DXF/DWG; a face and an edge drop View normal
/// to, Delete Part 1… and the part's material / appearance rows.
#[gpui::test]
fn graphics_menus_over_a_selection_as_live(cx: &mut TestAppContext) {
    let (window, view, top, bottom) = block_part(cx, "menus-selected");
    let edge = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).and_then(|b| b.edge_refs.first().cloned())).expect("an edge");
    let menu = |cx: &mut TestAppContext, sel: Vec<Reference>, at: Reference, kind| {
        view.update(cx, |v, _| v.selection = sel);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(Some(noshape_scene::builder::PickTarget { reference: at, kind, anchor: None }), gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
        draw(cx, window);
        let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
        view.update(cx, |v, cx| v.close_context_menu(cx));
        labels.into_iter().filter(|l| l != "———").collect::<Vec<_>>()
    };
    let live = |rows: &str| rows.split(" · ").map(str::to_string).collect::<Vec<_>>();
    let tail = "Copy Part 1 · Create Drawing of Part 1… · Export… · Where used for Part 1… · Clear selection · Select · Select other… · Zoom to fit · Zoom to selection";
    let face = menu(cx, vec![top.clone()], top.clone(), noshape_scene::builder::PickKind::Face);
    assert_eq!(face, live("Edit Extrude 1… · Edit Sketch 1… · Show dimensions · Show dependencies of Extrude 1… · New sketch… · Offset plane… · Extrude… · Revolve… · Hide Part 1 · Isolate… · Make transparent… · Section view… · Copy Part 1 · Create Drawing of Part 1… · Export as DXF/DWG… · Export… · Where used for Part 1… · Clear selection · Select · Select other… · Zoom to fit · Zoom to selection · View normal to · Delete Part 1… · Delete Extrude 1 · Assign material for Part 1… · Edit appearance for Part 1… · Add appearance to face…"), "the face, selected");
    let two = menu(cx, vec![top.clone(), bottom.clone()], bottom.clone(), noshape_scene::builder::PickKind::Face);
    assert_eq!(two, live(&format!("Edit Extrude 1… · Edit Sketch 1… · Show dimensions · Show dependencies of Extrude 1… · Hide Part 1 · Isolate… · Make transparent… · Section view… · {tail} · View normal to · Delete Part 1… · Delete Extrude 1 · Assign material for Part 1… · Edit appearance for Part 1… · Add appearance to 2 faces…")), "two faces");
    let mixed = menu(cx, vec![top.clone(), edge.clone()], edge, noshape_scene::builder::PickKind::Edge);
    assert_eq!(mixed, live(&format!("Edit Extrude 1… · Edit Sketch 1… · Show dimensions · Show dependencies of Extrude 1… · Hide Part 1 · Isolate… · Make transparent… · Section view… · {tail} · Delete Extrude 1 · Add appearance to face…")), "a face and an edge");
    // part-selected-graphics: Part 1's row selected, its top face right-clicked: no creation tools, no DXF.
    let body = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone()))).expect("the part");
    let part = menu(cx, vec![body], top.clone(), noshape_scene::builder::PickKind::Face);
    assert_eq!(part, live(&format!("Edit Extrude 1… · Edit Sketch 1… · Show dimensions · Show dependencies of Extrude 1… · Hide Part 1 · Isolate… · Make transparent… · Section view… · {tail} · View normal to · Delete Part 1… · Delete Extrude 1 · Assign material for Part 1… · Edit appearance for Part 1… · Add appearance to face…")), "a face with the part selected");
    // vertex: nothing selected.
    let vertex = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).and_then(|b| b.vertex_refs.first().cloned())).expect("a vertex");
    let corner = menu(cx, vec![], vertex, noshape_scene::builder::PickKind::Vertex);
    assert_eq!(corner, live("Edit Extrude 1… · Edit Sketch 1… · Show dimensions · Show dependencies of Extrude 1… · Hide Part 1 · Isolate… · Make transparent… · Copy Part 1 · Create Drawing of Part 1… · Export… · Where used for Part 1… · Select · Select other… · Zoom to fit · Delete Part 1… · Delete Extrude 1 · Assign material for Part 1… · Edit appearance for Part 1…"), "a vertex");
}

/// IN-FEATURE-MATE-CONNECTOR (live 1851): with "Sketch points to place holes"' mate connector button on, a face
/// picked makes a mate connector inside the Hole: chip "Mate connector", the hole at it, no Mate connector feature.
#[gpui::test]
fn a_hole_placed_on_an_in_feature_mate_connector(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "hole-in-feature-mc");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("hole", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Hole dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.mc_mode = Some("locations".into());
        d.active_query = Some("locations".into());
        d.add_pick(top, cx);
    }));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (subs, chips) = cx.update_window(window, |_, w, cx| {
        let d = dialog.read(cx);
        let state = d.ui_state(w, cx);
        let chips = state["fields"].as_array().unwrap().iter().find(|f| f["label"] == "Sketch points to place holes").map(|f| f["items"].clone()).unwrap_or_default();
        (d.feature.sub_features.len(), chips)
    }).unwrap();
    assert_eq!((subs, chips), (1, serde_json::json!(["Mate connector"])));
    // Its chip's icon opens its editor; a change there is the Hole's sub-feature's.
    let sub = cx.update(|cx| dialog.read(cx).feature.sub_features[0].id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_sub_editor(sub, w, cx))).unwrap();
    let nested = cx.update(|cx| view.read(cx).ui.sub_dialog.as_ref().map(|s| s.0.clone()).expect("the Mate connector editor"));
    cx.update(|cx| nested.update(cx, |d, cx| d.set_value("realign", noshape_model::ParamValue::Bool(true), cx)));
    draw(cx, window);
    assert!(cx.update(|cx| dialog.read(cx).feature.sub_features[0].params.bool("realign")));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (features, hole_ok) = cx.update(|cx| {
        let v = view.read(cx);
        let s = v.studio(cx).unwrap();
        let hole = s.features.iter().find(|f| f.type_id == "hole").expect("the Hole");
        (s.features.iter().map(|f| f.type_id.clone()).collect::<Vec<_>>(), v.display.feature_state(hole.id).is_some_and(|st| st.status == noshape_model::display::FeatureStatus::Ok) && hole.sub_features.len() == 1)
    });
    assert!(hole_ok && !features.iter().any(|t| t == "mateConnector"), "{features:?}");
}

/// qa SKETCH-ON-IN-FEATURE-MC (c2-sheet-metal-forms' Sketch 3): the Sketch plane field's mate connector button on, a
/// face picked makes a mate connector inside the sketch, and the sketch opens on it (the face's centroid, its normal);
/// no Mate connector feature is added.
#[gpui::test]
fn a_sketch_on_its_own_in_feature_mate_connector(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "sketch-in-feature-mc");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("sketch", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Sketch dialog").dialog.clone());
    assert!(cx.update(|cx| dialog.update(cx, |d, cx| {
        d.active_query = Some("sketchPlane".into());
        d.toggle_mc_mode(cx)
    })), "the Sketch plane field takes mate connectors (ctrl+m)");
    cx.update(|cx| dialog.update(cx, |d, cx| d.add_pick(top.clone(), cx)));
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    let (subs, chips) = cx.update_window(window, |_, w, cx| {
        let d = dialog.read(cx);
        let state = d.ui_state(w, cx);
        let chips = state["fields"].as_array().unwrap().iter().find(|f| f["label"] == "Sketch plane").map(|f| f["items"].clone()).unwrap_or_default();
        (d.feature.sub_features.len(), chips)
    }).unwrap();
    assert_eq!((subs, chips), (1, serde_json::json!(["Mate connector"])));
    // The top face's centre and normal (the block's top at z = its highest mesh point).
    let (frame, z_top) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        (v.sketch.as_ref().map(|s| s.frame), b.mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max))
    });
    let frame = frame.expect("the sketch editor");
    assert!((frame.origin.z - f64::from(z_top)).abs() < 1e-6 && (frame.normal.z - 1.0).abs() < 1e-9, "{frame:?} on z {z_top}");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
    let (types, ok) = cx.update(|cx| {
        let v = view.read(cx);
        let s = v.studio(cx).unwrap();
        let sk = s.features.iter().rfind(|f| f.type_id == "sketch").expect("the new sketch");
        (s.features.iter().map(|f| f.type_id.clone()).collect::<Vec<_>>(), sk.sub_features.len() == 1 && v.display.feature_state(sk.id).is_some_and(|st| st.status == noshape_model::display::FeatureStatus::Ok))
    });
    assert!(ok && !types.iter().any(|t| t == "mateConnector"), "{types:?}");
}

/// Live (feel/3d.json "double_click_face", 3d/dblclick-face.png): a part face double-clicked shows its feature's
/// sketch (consumed, so hidden) until the selection changes; the two clicks select then deselect.
#[gpui::test]
fn a_face_double_clicked_shows_its_features_sketch(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "dblclick-face");
    let at = std::rc::Rc::new(std::cell::Cell::new(std::time::Instant::now()));
    let clock = at.clone();
    cx.update(|cx| view.read(cx).viewport.clone().update(cx, |v, _| v.set_clock(move || clock.get())));
    let pick = cx.update(|cx| view.read(cx).built.as_ref().and_then(|b| b.picks.ids_for(&top).first().copied()));
    let shown = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display_for_scene().sketches.iter().filter(|s| !s.hidden).count());
    let click_at = |cx: &mut TestAppContext, pick: Option<u32>| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::Click { pick, position: DVec2::new(575., 350.), modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
    };
    let before = shown(cx);
    click_at(cx, pick);
    at.set(at.get() + std::time::Duration::from_millis(200));
    click_at(cx, pick);
    let (after, selected) = (shown(cx), cx.update(|cx| view.read(cx).selection.len()));
    // (Drawn: the scene has its curves, qa 11-18 SELFEEL-DBLCLICK-FACE-SKETCH on green: related but not drawn.)
    let drawn = cx.update(|cx| view.read(cx).built.as_ref().is_some_and(|b| (1..20_000).filter_map(|i| b.picks.get(i)).any(|t| matches!(t.reference, Reference::SketchEntity { .. }))));
    assert!(drawn, "the double-clicked face's sketch in the scene");
    let related = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).related_features(cx).len());
    let lit = related(cx);
    at.set(at.get() + std::time::Duration::from_millis(800));
    click_at(cx, pick);
    assert_eq!((before, after, selected, lit, shown(cx)), (0, 1, 0, 2, 0), "sketches shown before, after the double-click (selection, related rows), after the next click");
    // Live2 11-17 sel-feel 2: after the double-click an empty click clears the related rows and the sketch.
    at.set(at.get() + std::time::Duration::from_millis(200));
    click_at(cx, pick);
    assert_eq!((shown(cx), related(cx)), (1, 2), "double-clicked again");
    at.set(at.get() + std::time::Duration::from_millis(800));
    click_at(cx, None);
    assert_eq!((shown(cx), related(cx), cx.update(|cx| view.read(cx).selection.len())), (0, 0, 0), "after an empty click: no sketch, no related row");
}


/// Live (analysis-deviation; behaviours "psui (Deviation analysis panel, in full)"): a face preselected opens Deviation
/// analysis on its Surface tab, the face as the Source, read-only Max / Min, "Max and min location" on, "Absolute
/// values" off, then Scale; reopened over an edge it opens on Curve with both chains empty.
#[gpui::test]
fn deviation_opens_on_surface_with_a_face_preselected(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "deviation-face");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.selection = vec![top.clone()];
        v.toggle_analysis_tool(crate::part_studio::analysis_tools::ToolKind::Deviation, w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let panel = cx.update(|cx| view.read(cx).analysis_panel_state(cx)).expect("the Deviation panel");
    let text: Vec<String> = panel["text"].as_array().unwrap().iter().filter_map(|t| t.as_str().map(str::to_string)).collect();
    let want = ["Curve", "Surface", "Source", "Face of Extrude 1", "×", "To measure", "Max", "Min", "[x] Max and min location", "Max and min location", "[ ] Absolute values", "Absolute values", "Scale"];
    assert_eq!(text, want.map(String::from).to_vec());
    // Closed, then opened again over an edge: Curve (the tab isn't remembered), the edge not taken.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.toggle_analysis_tool(crate::part_studio::analysis_tools::ToolKind::Deviation, w, cx);
        v.selection = v.display.bodies[0].edge_refs.iter().take(1).cloned().collect();
        v.toggle_analysis_tool(crate::part_studio::analysis_tools::ToolKind::Deviation, w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let panel = cx.update(|cx| view.read(cx).analysis_panel_state(cx)).expect("the Deviation panel");
    let text: Vec<String> = panel["text"].as_array().unwrap().iter().filter_map(|t| t.as_str().map(str::to_string)).collect();
    assert_eq!(text, ["Curve", "Surface", "Chain 1", "Chain 2", "Max", "Min", "Scale"].map(String::from).to_vec());
}

/// Live (behaviours "psui (Deviation analysis panel, in full)", the overlap start): Part 1's top as the Source and Part
/// 2's as To measure read Max 11.18 mm (Part 2's far corners from Part 1's edge), Min 5 mm.
#[gpui::test]
fn surface_deviation_measures_to_measure_against_the_source(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "deviation-overlap", Some(starts["overlap"].clone()));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 2);
    // The top face of each part (all its triangles at the part's top height).
    let top = |cx: &mut TestAppContext, body: usize, z: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            v.display.bodies[body].face_refs.iter().find(|f| {
                let t = v.face_triangles(f);
                !t.is_empty() && t.iter().flatten().all(|p| (p.z - z).abs() < 1e-7)
            })
            .cloned()
        })
        .expect("the top face")
    };
    let (source, measured) = (top(cx, 0, 0.010), top(cx, 1, 0.015));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.selection = vec![source.clone()];
        v.toggle_analysis_tool(crate::part_studio::analysis_tools::ToolKind::Deviation, w, cx);
        if let Some(t) = &mut v.ui.analysis_tool {
            t.selection2 = vec![measured.clone()];
        }
        v.recompute_analysis_tool(cx);
    }))
    .unwrap();
    draw(cx, window);
    let panel = cx.update(|cx| view.read(cx).analysis_panel_state(cx)).expect("the Deviation panel");
    let value = |label: &str| panel["fields"].as_array().unwrap().iter().find(|f| f["label"] == label).and_then(|f| f["value"].as_str().map(str::to_string));
    assert_eq!((value("Max"), value("Min")), (Some("11.18 mm".to_string()), Some("5 mm".to_string())));
    // The colour map on To measure (Viridis): its vertices over Part 1 at the low end, the far corners at the top.
    let (low, high) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let colors = v.vertex_colors(cx).expect("vertex colours");
            let b = &v.display.bodies[1];
            let c = colors.get(&b.id).expect("Part 2's colours").clone();
            let at = |x: f32, y: f32| (0..b.mesh.positions.len()).find(|&i| (b.mesh.positions[i][0] - x).abs() < 1e-6 && (b.mesh.positions[i][1] - y).abs() < 1e-6 && (b.mesh.positions[i][2] - 0.015).abs() < 1e-6 && c[i][3] == 255).map(|i| c[i]);
            (at(0.030, 0.010), at(0.050, 0.020))
        })
    });
    assert_eq!((low, high), (Some([0x44, 0x01, 0x54, 255]), Some([0xfd, 0xe7, 0x25, 255])), "Viridis' ends at 5 and 11.18 mm");
    // Swapped, Part 1's top lies under Part 2's: signed, it reads negative (Max -5 mm); Absolute values drops the sign.
    let swapped = |cx: &mut TestAppContext, absolute: bool| {
        cx.update(|cx| view.update(cx, |v, cx| {
            if let Some(t) = &mut v.ui.analysis_tool {
                t.selection = vec![measured.clone()];
                t.selection2 = vec![source.clone()];
                t.options.absolute = absolute;
            }
            v.recompute_analysis_tool(cx);
        }));
        draw(cx, window);
        let panel = cx.update(|cx| view.read(cx).analysis_panel_state(cx)).expect("the Deviation panel");
        let value = |label: &str| panel["fields"].as_array().unwrap().iter().find(|f| f["label"] == label).and_then(|f| f["value"].as_str().map(str::to_string));
        (value("Max"), value("Min").is_some_and(|m| m.starts_with('-')))
    };
    assert_eq!((swapped(cx, false), swapped(cx, true).1), ((Some("-5 mm".to_string()), true), false), "signed, then absolute");
}

/// Live2 (11-14, feel/hidden-part-selection.png; hl-pixels-1118): a part selected whole has its outline drawn through
/// the parts in front (the renderer's band over its image's mask, depth ignored: `Scene::body_outline`); nothing
/// selected, none.
#[gpui::test]
fn a_selected_part_outlines_through_parts_in_front(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "outline-through");
    let through = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).scene().body_outline);
    let before = through(cx);
    cx.update(|cx| view.update(cx, |v, cx| {
        let body = v.display.bodies[0].id.clone();
        v.selection = vec![Reference::Body(body)];
        v.refresh_scene(false, cx);
    }));
    draw(cx, window);
    assert_eq!((before, through(cx)), (false, true), "outline through: nothing selected, the part selected");
}

/// Live workflow 11 (qa SKETCH-LINE-ON-FACE-HIDDEN): a shown sketch's line under a plate's top face is drawn dashed
/// where the plate hides it (one dashed copy of its curves, drawn only behind the parts); the consumed sketch, hidden,
/// has none.
#[gpui::test]
fn a_shown_sketch_behind_a_part_is_dashed(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "2 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "line": [[30, -10], [30, 50]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "sketch-dashed-behind", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.sketches.iter().any(|s| !s.hidden));
    draw(cx, window);
    let dashed = cx.update(|cx| view.read(cx).viewport.read(cx).scene().instances.iter().filter(|i| i.hidden_only && matches!(i.drawable, noshape_render::Drawable::Lines(_))).count());
    assert_eq!(dashed, 1, "Sketch 2's dashed copy only");
}

/// Live workflow 19 (qa FEEL-QUERY-PART-PICK-FILL): a part picked into a dialog's field (Mirror's Entities) keeps its own
/// colour, only its outline drawn (through the parts in front); selected outside a dialog it takes the part amber.
#[gpui::test]
fn a_part_in_a_dialog_field_is_outlined_not_tinted(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "part-pick-outline");
    let look = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let body = &v.display.bodies[0];
            let id = v.built.as_ref().and_then(|b| b.picks.ids_for(&body.face_refs[0]).first().copied()).expect("a face's pick");
            let s = v.viewport.read(cx).scene();
            let st = s.states.get(id as usize).copied().unwrap_or(0);
            (st & noshape_render::state::BODY != 0, s.body_outline)
        })
    };
    let body = cx.update(|cx| view.read(cx).display.bodies[0].id.clone());
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection = vec![Reference::Body(body.clone())];
        v.refresh_scene(false, cx);
    }));
    draw(cx, window);
    let selected = look(cx);
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection.clear();
        v.refresh_scene(false, cx);
    }));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("mirror", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(vec![Reference::Body(body.clone())]), cx)));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    assert_eq!((selected, look(cx)), ((true, true), (false, true)), "((tinted, outlined) selected, in Mirror's Entities)");
}

/// Live workflows 2, 11, 12 and 21 (qa FEEL-HELIX-FACE-PICKED-LOOK): a face in a dialog's field keeps its shading under
/// a light tint (TINT), its boundary orange; selected outside a dialog it takes the amber (no TINT).
#[gpui::test]
fn a_face_in_a_dialog_field_is_lightly_tinted(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "face-pick-tint");
    let look = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let id = v.built.as_ref().and_then(|b| b.picks.ids_for(&top).first().copied()).expect("the top face's pick");
            let st = v.viewport.read(cx).scene().states.get(id as usize).copied().unwrap_or(0);
            (st & noshape_render::state::SELECTED != 0, st & noshape_render::state::TINT != 0)
        })
    };
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection = vec![top.clone()];
        v.refresh_scene(false, cx);
    }));
    draw(cx, window);
    let selected = look(cx);
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection.clear();
        v.refresh_scene(false, cx);
    }));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("cPlane", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(vec![top.clone()]), cx)));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    assert_eq!((selected, look(cx)), ((true, false), (true, true)), "((selected, tinted) outside a dialog, in Plane's Entities)");
}

/// Live workflow 38 step 5 (qa FEEL-QUERY-PART-PICK-FILL): both overlapping parts in Boolean's Tools, the union
/// previewed: no face takes the part amber, the parts outlined.
#[gpui::test]
fn parts_in_booleans_tools_stay_untinted_under_the_preview(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "bool-tools-untinted", Some(starts["overlap"].clone()));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 2);
    let bodies: Vec<Reference> = cx.update(|cx| view.read(cx).display.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("booleanBodies", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("tools", noshape_model::ParamValue::Query(bodies), cx)));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let (amber, outlined) = cx.update(|cx| {
        let v = view.read(cx);
        let built = v.built.as_ref().expect("the scene");
        let s = v.viewport.read(cx).scene();
        let amber = v.display.bodies.iter().flat_map(|b| b.face_refs.iter()).flat_map(|f| built.picks.ids_for(f).iter().copied()).filter(|&id| s.states.get(id as usize).is_some_and(|st| st & noshape_render::state::BODY != 0)).count();
        (amber, s.body_outline)
    });
    assert_eq!((amber, outlined), (0, true), "(faces in the part amber, outlined)");
}

/// Live workflow 38 (qa SPLIT-KEEP-TOOLS-AUTO): std's splitEditLogic: one face picked as Split's tool ticks Keep tools
/// (else "Faces selected as split tools must be kept.").
#[gpui::test]
fn a_face_as_splits_tool_ticks_keep_tools(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "split-keep-tools");
    let body = cx.update(|cx| Reference::Body(view.read(cx).display.bodies[0].id.clone()));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("splitPart", w, cx))).unwrap();
    let d = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("targets", noshape_model::ParamValue::Query(vec![body]), cx)));
    cx.update(|cx| d.update(cx, |d, cx| d.set_value("tool", noshape_model::ParamValue::Query(vec![top]), cx)));
    let keep = |cx: &mut TestAppContext| cx.update(|cx| d.read(cx).feature.params.bool("keepTools"));
    for _ in 0..500 {
        if keep(cx) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        draw(cx, window);
    }
    assert!(keep(cx), "Keep tools ticked by the face");
}

/// Live 1990 (3.18): a double-click on a shown sketch's line in the graphics doesn't open the sketch; the first click
/// selects the line, the second deselects it.
#[gpui::test]
fn a_double_click_on_a_sketch_line_does_not_open_the_sketch(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "dblclick-sketch-line", Tool::Line);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 300.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).edit.is_none() && view.read(cx).display.sketches.first().is_some_and(|s| !s.curves.is_empty()));
    let pick = cx.update(|cx| {
        let v = view.read(cx);
        let sk = v.display.sketches.first().unwrap();
        let line = Reference::SketchEntity { sketch: sk.feature, entity: sk.curves[0].entity };
        v.built.as_ref().and_then(|b| b.picks.ids_for(&line).first().copied())
    });
    assert!(pick.is_some(), "the line is pickable");
    for _ in 0..2 {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::Click { pick, position: DVec2::new(575., 300.), modifiers: Modifiers::default() }, w, cx);
        })).unwrap();
        draw(cx, window);
    }
    assert!(cx.update(|cx| view.read(cx).edit.is_none() && view.read(cx).selection.is_empty()));
}

/// MC-INFERENCE-HOVER (live behaviours 1327, psui/hole-mc-1851/4): in a Mate connector's Origin entity, the hovered
/// top face offers its inference points; with the pointer at a corner a click picks that corner, stored as live does
/// (the face, POINT, the vertex), so the connector sits at the corner with its Z the face normal.
#[gpui::test]
fn a_mate_connector_picked_at_a_hovered_faces_corner(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "mc-inference-corner");
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("mateConnector", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let corner = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let i = b.face_refs.iter().position(|r| *r == top).expect("the top face");
        let (first, count) = b.mesh.face_ranges[i];
        let p = b.mesh.indices[first as usize..(first + count) as usize].iter().map(|&k| b.mesh.positions[k as usize]).max_by(|a, c| (a[0] + a[1]).total_cmp(&(c[0] + c[1]))).unwrap();
        glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.ui.pointer = v.viewport.read(cx).project(corner);
        v.ui.hover_pick = Some(noshape_scene::builder::PickTarget { reference: top.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None });
        v.update_inference(false, w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).inference_choice().is_some());
    let chosen = cx.update(|cx| view.read(cx).inference_choice().map(|p| p.frame.origin)).unwrap();
    assert!(chosen.distance(corner) < 1e-6, "{chosen} for the corner {corner}");
    view.update(cx, |v, cx| v.pick(noshape_scene::builder::PickTarget { reference: top.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None }, cx));
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Mate connector dialog").dialog.clone());
    let (origin, inference, secondary) = cx.update(|cx| {
        let p = &dialog.read(cx).feature.params;
        (p.query("originQuery").to_vec(), p.enum_("entityInferenceType").to_string(), p.query("secondaryOriginQuery").to_vec())
    });
    assert_eq!((origin, inference.as_str()), (vec![top.clone()], "POINT"));
    assert!(matches!(secondary.as_slice(), [Reference::Vertex(_)]), "{secondary:?}");
    // Live fc-1120 (7): the chip names the vertex, the connector attached to its owner.
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !dialog.read(cx).feature.params.query("ownerPart").is_empty());
    let (chips, attach) = cx.update(|cx| {
        let d = dialog.read(cx);
        (d.chip_labels("originQuery"), d.feature.params.enum_("attachmentOption").to_string())
    });
    assert_eq!((chips, attach.as_str()), (vec!["Vertex of Extrude 1".to_string()], "TO_OWNER"), "(the origin's chip, Attachment)");
    // Live mc-infer-1122: after its one pick the feature infers no more (the face hovered again shows no points), and
    // the owner part is what the dialog shows lit (its #ffc04c outline).
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.ui.hover_pick = Some(noshape_scene::builder::PickTarget { reference: top.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None });
        v.update_inference(false, w, cx);
    }))
    .unwrap();
    let (applies, marks, active) = cx.update(|cx| {
        let v = view.read(cx);
        (v.inference_applies(cx), v.inference_marks().is_some(), dialog.read(cx).active_refs())
    });
    assert!(!applies && !marks, "inference after the pick: applies {applies}, marks {marks}");
    assert!(matches!(active.as_slice(), [Reference::Body(_)]), "the lit picks after the pick: {active:?} (live: the owner part)");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).display.connectors.is_empty());
    let (o, z) = cx.update(|cx| view.read(cx).display.connectors.first().map(|c| (c.origin, c.z))).unwrap();
    assert!(o.distance(corner) < 1e-6 && z.distance(glam::DVec3::Z) < 1e-9, "{o} {z}");
}

/// Live fc-1120 (8) (and psui 10-28 2.23): a part see-through by its appearance is hovered and clicked as any other;
/// with Alt the pick goes through it to the part behind.
#[gpui::test]
fn alt_picks_through_a_see_through_part(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[-100, -100], [100, 100]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [20, 20]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "30 mm", "operationType": "NEW" } },
    ] });
    let (window, view) = part_studio_with(cx, "alt-through", |path| {
        let mut ctx = noshape_ops::Ctx::new();
        noshape_ops::ui_parity::build_start(&mut ctx, path, &start).unwrap();
        let op = noshape_ops::registry::by_tool_name("part_set_appearance").expect("part_set_appearance");
        noshape_ops::registry::run(&mut ctx, op, &serde_json::json!({ "parts": ["Part 2"], "color": "#e51919", "transparency": 0.5 })).unwrap();
        crate::session::Session::open(path).unwrap()
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count() == 2);
    // The tall part's top face and where its middle is on screen.
    let (tall, top, at) = cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        v.display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).find_map(|b| {
            b.face_refs.iter().zip(&b.mesh.face_ranges).find_map(|(r, &(first, count))| {
                let idx = &b.mesh.indices[first as usize..(first + count) as usize];
                let c = idx.iter().map(|&k| { let q = b.mesh.positions[k as usize]; glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64) }).sum::<glam::DVec3>() / idx.len() as f64;
                ((c.z - 0.030).abs() < 1e-6).then(|| Some((b.id.clone(), r.clone(), vp.project(c)?))).flatten()
            })
        })
    }).expect("the tall part's top face on screen");
    assert!(cx.update(|cx| view.read(cx).display.body(&tall).is_some_and(|b| b.color[3] < 0.99)), "Part 2 see-through");
    let t = noshape_scene::builder::PickTarget { reference: top.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None };
    let through = |alt: bool, cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            v.pick_through_transparent(t.clone(), at, alt, cx).map(|p| (v.body_of(&p.reference) == Some(tall.clone()), p.kind))
        })
    };
    let hovered = |alt: bool, cx: &mut TestAppContext| {
        cx.update(|cx| view.update(cx, |v, cx| {
            v.ui.alt_held = alt;
            v.ui.pointer = Some(at);
            v.ui.hover_raw = Some(t.clone());
            v.rehover(cx);
            v.ui.hover_pick.as_ref().map(|p| v.body_of(&p.reference) == Some(tall.clone()))
        }))
    };
    use noshape_scene::builder::PickKind::Face;
    let got = (through(false, cx), hovered(false, cx), through(true, cx), hovered(true, cx));
    assert_eq!(got, (Some((true, Face)), Some(true), Some((false, Face)), Some(false)), "(click, hover; with Alt: click, hover) on the see-through part, true while it takes the pick");
}

/// Live fc-1120 (5): a new Fillet, Tangent propagation on, one edge of a tangent chain clicked: only that edge is picked
/// and listed; the chain shows in the preview alone (more faces than that edge's fillet), which Tangent propagation off
/// shrinks to the edge.
#[gpui::test]
fn a_fillet_lists_only_the_clicked_edge_of_a_tangent_chain(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 20]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [40, 20, 5]"], "radius": "5 mm" } },
    ] });
    let mut doc = None;
    let (window, view) = part_studio_with(cx, "fillet-chain", |path| {
        doc = Some(path.to_path_buf());
        noshape_ops::ui_parity::build_start(&mut noshape_ops::Ctx::new(), path, &start).unwrap();
        crate::session::Session::open(path).unwrap()
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let op = noshape_ops::registry::by_tool_name("query_entities").expect("query_entities");
    let got = noshape_ops::registry::run(&mut noshape_ops::Ctx::new(), op, &serde_json::json!({ "doc": doc.unwrap().to_str(), "kind": "edges", "at": [[20, 20, 10]] })).unwrap().value;
    let edge = got["edges"][0]["id"].as_str().and_then(noshape_ops::refs::parse_str).expect("the top edge along y = 20");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    view.update(cx, |v, cx| v.pick(noshape_scene::builder::PickTarget { reference: edge.clone(), kind: noshape_scene::builder::PickKind::Edge, anchor: None }, cx));
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Fillet dialog").dialog.clone());
    let faces = |cx: &mut TestAppContext| {
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).ui.preview_before.is_some());
        cx.update(|cx| view.read(cx).display.bodies.iter().map(|b| b.face_refs.len()).sum::<usize>())
    };
    let chain = faces(cx);
    let listed = cx.update(|cx| (dialog.read(cx).feature.params.query("entities").to_vec(), dialog.read(cx).chip_labels("entities")));
    cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("tangentPropagation", noshape_model::params::ParamValue::Bool(false), cx)));
    let one = faces(cx);
    assert_eq!(listed, (vec![edge], vec!["Edge of Extrude 1".to_string()]), "(the picks, their chips)");
    assert!(chain > one, "the chain's fillet in the preview ({chain} faces), the edge's alone with Tangent propagation off ({one})");
}

/// Live 1987: a committed Hole's remembered values open the next new Hole; its Units / Style tabs (no units) in every
/// document, its diameter only in documents of the same length unit.
#[gpui::test]
fn remembered_hole_values_follow_the_document_units(cx: &mut TestAppContext) {
    let (_window, view, _top, _) = block_part(cx, "remembered-hole");
    let (mm, inch) = {
        let mm = noshape_units::UnitPrefs::default();
        let mut inch = mm;
        inch.length = noshape_units::LengthUnit::Inch;
        (mm, inch)
    };
    let mut hole = cx.update(|cx| view.read(cx).studio(cx).unwrap().new_feature("hole", &mm).unwrap());
    hole.params.set("styleV2", noshape_model::ParamValue::Enum("C_BORE".into()));
    hole.params.set("holeDiameterV3", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("7 mm")));
    view.update(cx, |v, _| v.remember_values(&hole, &mm));
    let reopened = |units: &noshape_units::UnitPrefs, cx: &mut TestAppContext| {
        let mut f = cx.update(|cx| view.read(cx).studio(cx).unwrap().new_feature("hole", units).unwrap());
        cx.update(|cx| view.read(cx).apply_remembered(&mut f, units));
        (f.params.enum_("styleV2").to_string(), f.params.expr("holeDiameterV3").map(|e| e.text.clone()).unwrap_or_default())
    };
    assert_eq!(reopened(&mm, cx), ("C_BORE".to_string(), "7 mm".to_string()));
    let (style, dia) = reopened(&inch, cx);
    assert_eq!(style, "C_BORE");
    assert_ne!(dia, "7 mm", "the mm diameter isn't an inch document's");
}

/// User P0 11-17 (live's answer, behaviours "feature dialogs remembering the previous value"): a value with a #variable
/// is never remembered, the earlier memory stays ("9 mm + #wall_thickness" opened red in a new document); plain values
/// and variable-free expressions are kept as typed; metric documents share one memory, inch ones another.
#[gpui::test]
fn a_remembered_value_never_carries_a_variable(cx: &mut TestAppContext) {
    let (_window, view) = part_studio(cx, "remembered-expression");
    let mm = noshape_units::UnitPrefs::default();
    let cm = noshape_units::UnitPrefs { length: noshape_units::LengthUnit::Centimeter, ..mm };
    let inch = noshape_units::UnitPrefs { length: noshape_units::LengthUnit::Inch, ..mm };
    let new_fillet = |units: &noshape_units::UnitPrefs, cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).unwrap().new_feature("fillet", units).unwrap());
    let commit = |radius: &str, cx: &mut TestAppContext| {
        let mut f = new_fillet(&mm, cx);
        f.params.set("radius", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new(radius)));
        view.update(cx, |v, _| v.remember_values(&f, &mm));
    };
    let opens = |units: &noshape_units::UnitPrefs, cx: &mut TestAppContext| {
        let mut g = new_fillet(units, cx);
        cx.update(|cx| view.read(cx).apply_remembered(&mut g, units));
        g.params.expr("radius").map(|e| e.text.clone()).unwrap()
    };
    let default = opens(&mm, cx);
    // A store an older build wrote (lengths per unit, "length:Millimeter"): a #variable in it reads as nothing, the
    // default opens; a plain value is the metric memory (in a cm document too), not the inch one.
    let older = |radius: &str, cx: &mut TestAppContext| {
        view.update(cx, |v, _| {
            let old: noshape_model::params::ParamValues = noshape_model::params::ParamValues([("radius".to_string(), noshape_model::ParamValue::Expr(noshape_units::ExprValue::new(radius)))].into_iter().collect());
            v.ui.remembered.insert("fillet".into(), [("length:Millimeter".to_string(), old.0.into_iter().collect())].into_iter().collect());
        })
    };
    older("9 mm + #wall_thickness", cx);
    assert_eq!(opens(&mm, cx), default, "a stored value with a variable is ignored");
    older("3 mm", cx);
    assert_eq!((opens(&mm, cx), opens(&cm, cx)), ("3 mm".to_string(), "3 mm".to_string()), "an older store's mm memory is the metric one");
    assert_ne!(opens(&inch, cx), "3 mm", "not the inch one");
    view.update(cx, |v, _| v.ui.remembered.clear());
    commit("9 mm + #wall_thickness", cx);
    assert_eq!(opens(&mm, cx), default, "nothing with a variable is remembered");
    commit("1 mm + 1 mm", cx);
    assert_eq!(opens(&mm, cx), "1 mm + 1 mm", "a variable-free expression as typed");
    commit("2 mm + #v", cx);
    assert_eq!(opens(&mm, cx), "1 mm + 1 mm", "the earlier memory stays");
    assert_eq!(opens(&cm, cx), "1 mm + 1 mm", "one memory for metric documents");
    assert_ne!(opens(&inch, cx), "1 mm + 1 mm", "inch documents keep their own");
}

/// User P0 11-17 (live, "feature dialogs remembering the previous value"): a Fillet carries its Radius and Measurement to
/// the next one, not Tangent propagation or Asymmetric; an Extrude's Depth is never remembered (25 mm again); a
/// cancelled dialog leaves the memory as it was (only ✓ writes it).
#[gpui::test]
fn only_radius_and_measurement_carry_and_a_cancel_remembers_nothing(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "remembered-which");
    let mm = noshape_units::UnitPrefs::default();
    let new = |ty: &str, cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).unwrap().new_feature(ty, &mm).unwrap());
    let opens = |ty: &str, cx: &mut TestAppContext| {
        let mut f = new(ty, cx);
        cx.update(|cx| view.read(cx).apply_remembered(&mut f, &mm));
        f
    };
    view.update(cx, |v, _| v.ui.remembered.clear());
    let defaults = (opens("fillet", cx), opens("extrude", cx));
    let mut f = new("fillet", cx);
    f.params.set("radius", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("3.5 mm")));
    f.params.set("blendControlType", noshape_model::ParamValue::Enum("WIDTH".into()));
    f.params.set("tangentPropagation", noshape_model::ParamValue::Bool(false));
    f.params.set("isAsymmetric", noshape_model::ParamValue::Bool(true));
    let mut e = new("extrude", cx);
    e.params.set("depth", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("7 mm")));
    view.update(cx, |v, _| {
        v.remember_values(&f, &mm);
        v.remember_values(&e, &mm);
    });
    let fillet = opens("fillet", cx);
    let carried = (fillet.params.expr("radius").map(|x| x.text.clone()), fillet.params.enum_("blendControlType").to_string(), fillet.params.bool("tangentPropagation"), fillet.params.bool("isAsymmetric"));
    let default_tangent = defaults.0.params.bool("tangentPropagation");
    assert_eq!(carried, (Some("3.5 mm".to_string()), "WIDTH".to_string(), default_tangent, false), "(Radius, Measurement, Tangent propagation, Asymmetric) on the next Fillet");
    let depth = |f: &noshape_model::partstudio::Feature| f.params.expr("depth").map(|x| x.text.clone());
    assert_eq!(depth(&opens("extrude", cx)), depth(&defaults.1), "an Extrude's Depth isn't remembered");
    // A Fillet opened, its Radius typed, then cancelled: the next one opens with the committed 3.5 mm.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).expect("the Fillet dialog");
    dialog.update(cx, |d, _| d.feature.params.set("radius", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("4 mm"))));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
    assert_eq!(opens("fillet", cx).params.expr("radius").map(|x| x.text.clone()), Some("3.5 mm".to_string()), "a cancelled dialog isn't remembered");
    // A Fillet committed with nothing to fillet (it fails): its 6 mm is remembered all the same.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).expect("the Fillet dialog");
    dialog.update(cx, |d, cx| d.set_value("radius", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("6 mm")), cx));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).edit.is_none());
    let failed = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().any(|f| f.type_id == "fillet"));
    assert!(failed, "the failing Fillet committed");
    assert_eq!(opens("fillet", cx).params.expr("radius").map(|x| x.text.clone()), Some("6 mm".to_string()), "a committed Fillet that fails is remembered");
}

/// A view with a cached slice holding a nested cached row, and a focusable element outside both: counts their renders.
struct CacheHost {
    renders: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, usize>>>,
    list: Option<Entity<crate::cached_slice::Slice<CacheHost>>>,
    row: Option<Entity<crate::cached_slice::Slice<CacheHost>>>,
    /// What the list shows ([`crate::cached_slice::Slice::keyed`]).
    version: u64,
    other: gpui_kit::FocusHandle,
}

impl Render for CacheHost {
    fn render(&mut self, _: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        use crate::cached_slice::Slice;
        let me = cx.entity();
        let bump = |r: &std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, usize>>>, k| *r.borrow_mut().entry(k).or_default() += 1;
        bump(&self.renders, "host");
        if self.row.is_none() {
            self.row = Some(Slice::unobserved(&me, move |this: &mut Self, _, _| {
                bump(&this.renders, "row");
                div().size_full().debug_selector(|| "cached-row".into()).child("row").into_any_element()
            }, cx));
            self.list = Some(Slice::new(&me, move |this: &mut Self, _, _| {
                bump(&this.renders, "list");
                let row = this.row.clone().unwrap().cached(gpui_kit::StyleRefinement::default().w_full().h(px(20.)).flex_none());
                div().size_full().flex().flex_col().child(div().h(px(30.)).flex_none()).child(row).into_any_element()
            }, cx));
            Slice::keyed(self.list.as_ref().unwrap(), |this: &Self, _| this.version, cx);
        }
        // Read, so the window keeps tracking them (the app's slices are made inside a cached view's render).
        for e in [&self.list, &self.row].into_iter().flatten() {
            e.read(cx);
        }
        let style = || gpui_kit::StyleRefinement::default().w_full().h(px(100.)).flex_none();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.list.clone().unwrap().cached(style()))
            .child(div().id("other").track_focus(&self.other).size(px(10.)))
    }
}

/// GPUI's view cache (vendor gpui-pre NOSHAPE PATCHes, agent psui): a cached row nested in a cached list stays cached
/// when the list is redrawn the frame after one that reused the list whole, and a focus change elsewhere redraws
/// neither (upstream refreshed the whole window: a Part Studio's ~70 cached rows drawn again on every dialog open
/// and close). The reused row is painted where it was.
#[gpui::test]
fn cached_views_survive_a_reused_parent_and_a_focus_change(cx: &mut TestAppContext) {
    let renders: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, usize>>> = Default::default();
    let r2 = renders.clone();
    let window = cx.add_window(move |_, cx| CacheHost { renders: r2, list: None, row: None, version: 0, other: cx.focus_handle() });
    let any: AnyWindowHandle = window.into();
    let count = |k: &str| renders.borrow().get(k).copied().unwrap_or(0);
    // Settle: the first frames, and the list's key read on its first parent notify.
    draw(cx, any);
    window.update(cx, |_, _, cx| cx.notify()).unwrap();
    draw(cx, any);
    let row_at = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(any, cx).debug_bounds("cached-row");
    let first = row_at(cx).expect("the row is painted");
    let (list0, row0) = (count("list"), count("row"));
    // The host redraws, reusing the list (and the row in it) whole.
    window.update(cx, |_, _, cx| cx.notify()).unwrap();
    draw(cx, any);
    assert_eq!((count("list") - list0, count("row") - row0), (0, 0), "the list is reused");
    // What the list shows changes: it is drawn again, the row (not dirty) reused.
    window.update(cx, |h, _, cx| {
        h.version += 1;
        cx.notify();
    })
    .unwrap();
    draw(cx, any);
    assert_eq!((count("list") - list0, count("row") - row0), (1, 0), "the row stays cached under its redrawn list");
    assert_eq!(row_at(cx), Some(first), "and is painted in place");
    // Focus moves to an element outside the slices.
    window.update(cx, |h, w, cx| w.focus(&h.other.clone(), cx)).unwrap();
    draw(cx, any);
    assert_eq!((count("list") - list0, count("row") - row0), (1, 0), "a focus change elsewhere redraws no cached view");
    assert_eq!(row_at(cx), Some(first));
}

/// A dialog's preview builds its scene off the UI thread (`apply_display_async`): it lands once built, and never
/// over a display applied after it.
#[gpui::test]
fn a_background_scene_build_never_lands_over_a_newer_display(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "scene-async");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let full = cx.update(|cx| view.read(cx).display.clone());
    let empty = std::sync::Arc::new(noshape_model::display::StudioDisplay::defaults());
    let top_picks = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).built.as_ref().map_or(0, |b| b.picks.ids_for(&top).len()));
    assert_eq!(top_picks(cx), 1);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.apply_display_async(empty.clone(), w, cx);
            v.apply_display(full.clone(), w, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!(top_picks(cx), 1, "the older build was dropped");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.apply_display_async(empty.clone(), w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!(top_picks(cx), 0, "a build alone lands");
}

/// Opening a select changes nothing outside its list (live: the Configurations box's Size select keeps its field at
/// [74,112,161,26] and the panel unmoved while open; the field shows the empty search input): the box, its title and
/// buttons, the field and the feature list keep their bounds.
#[gpui::test]
fn opening_the_configuration_select_moves_nothing(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "cfg-select", Some(block));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    view.update(cx, |v, cx| v.add_config_input("List", cx));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let (input, select) = cx.update(|cx| {
        let v = view.read(cx);
        let (id, (s, _)) = v.ui.cfg_selects.iter().next().expect("the List input's select");
        (id.clone(), s.clone())
    });
    let names = ["cfg-box", "cfgbox-title", "cfgbox-split", "feature-list"];
    let field = format!("os-select:cfgsel-{input}");
    let bounds = |cx: &mut TestAppContext| {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let mut all: Vec<(String, Option<Bounds<gpui_kit::Pixels>>)> = names.iter().map(|n| (n.to_string(), vcx.debug_bounds(n))).collect();
        all.push((field.clone(), vcx.debug_bounds(Box::leak(field.clone().into_boxed_str()))));
        all
    };
    let closed = bounds(cx);
    assert!(closed.iter().all(|(_, b)| b.is_some()), "{closed:?}");
    let at = closed.last().unwrap().1.unwrap().center();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::default());
    draw(cx, window);
    assert!(cx.update(|cx| select.read(cx).is_open()), "the list opened");
    let open = bounds(cx);
    for ((name, a), (_, b)) in closed.iter().zip(&open) {
        assert_eq!(a, b, "{name} moved when the select opened");
    }
}

/// Live2 ps-feel2-1118 (cfg): the feature list's configuration box takes Down + Return: the next row becomes the
/// configuration, the box closes, the feature list stays as it was.
#[gpui::test]
fn the_configuration_box_switches_by_down_and_return(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "cfg-switch-keys", Some(block));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let input = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                noshape_ops::config::add_option(ps, &input, "A".into());
            });
            input
        })
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let rows: Vec<String> = cx.update(|cx| match view.read(cx).studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == input)).map(|i| i.kind.clone()) {
        Some(noshape_model::configuration::ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
        _ => Vec::new(),
    });
    let features = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).map(|s| s.features.iter().map(|f| f.name.clone()).collect::<Vec<_>>()));
    let before = features(cx);
    let select = cx.update(|cx| view.read(cx).ui.cfg_selects.get(&input).map(|(s, _)| s.clone()).expect("the box"));
    let field = format!("os-select:cfgsel-{input}");
    let at = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(field.into_boxed_str())).expect("the box drawn").center();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::default());
    draw(cx, window);
    assert!(cx.update(|cx| select.read(cx).is_open()), "the list opened");
    keys(cx, window, &["down", "enter"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let now = cx.update(|cx| view.read(cx).config_values.0.get(&input).cloned());
    assert_eq!((now.as_deref(), cx.update(|cx| select.read(cx).is_open())), (rows.get(1).map(String::as_str), false), "(the configuration, the box open)");
    assert_eq!(features(cx), before, "the feature list as it was");
}

/// Cached list rows span the list (the hover highlight from edge to edge), with the eye at the row's right end
/// (live): a part row was sized to its content, the eye right after the name.
#[gpui::test]
fn list_rows_span_the_list_with_the_eye_at_the_right(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "row-width", Some(block));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let (part, sketch) = cx.update(|cx| {
        let v = view.read(cx);
        (v.display.bodies[0].id.clone(), v.studio(cx).unwrap().features.iter().find(|f| f.type_id == "sketch").unwrap().id)
    });
    assert!(cx.update(|cx| view.read(cx).part_row_cache.contains_key(&part)), "the part row is cached");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let mut at = |s: String| vcx.debug_bounds(Box::leak(s.into_boxed_str())).unwrap_or_else(|| panic!("no bounds"));
    let list = at("feature-list".into());
    let (part_row, part_eye) = (at(format!("part-row-{part}")), at(format!("part-eye-{part}")));
    let (sketch_row, sketch_eye) = (at(format!("feature-row-{sketch:?}")), at(format!("feat-eye-{sketch:?}")));
    for (name, row, eye) in [("part", part_row, part_eye), ("sketch", sketch_row, sketch_eye)] {
        assert!((row.left() - list.left()).abs() < px(1.) && (row.right() - list.right()).abs() < px(20.), "{name} row {row:?} spans the list {list:?}");
        assert!(row.right() - eye.right() < px(20.), "{name} eye {eye:?} at the right of {row:?}");
    }
}

/// The Export dialog of a test window: `open` it, `press` an element by id, `pick` a list's option as the user does
/// (open the list, click the option).
struct ExportDialog {
    window: AnyWindowHandle,
    exported: std::rc::Rc<std::cell::RefCell<Vec<noshape_ops::export::ExportSettings>>>,
}

/// The export options a test remembers, in its own scratch file: one run's test binaries share the settings directory
/// (a parallel test's export or an agent API export would change them under it).
struct OwnExportPrefs(std::path::PathBuf);

impl OwnExportPrefs {
    fn new(test: &str) -> Self {
        let p = std::env::temp_dir().join(format!("psui-export-prefs-{test}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&p);
        noshape_ops::export::SolidExportPrefs::use_file(Some(p.clone()));
        Self(p)
    }
}

impl Drop for OwnExportPrefs {
    fn drop(&mut self) {
        noshape_ops::export::SolidExportPrefs::use_file(None);
        let _ = std::fs::remove_file(&self.0);
    }
}

impl ExportDialog {
    fn open(&self, cx: &mut TestAppContext, assembly: bool) {
        let exported = self.exported.clone();
        let run: super::super::export::RunExport = std::rc::Rc::new(move |s, _, _, _| exported.borrow_mut().push(s));
        cx.update_window(self.window, |_, w, cx| super::super::export::open_export_modal("Part Studio 1".into(), assembly, run, w, cx)).unwrap();
        draw(cx, self.window);
    }

    fn bounds(&self, cx: &mut TestAppContext, id: &'static str) -> Option<Bounds<gpui_kit::Pixels>> {
        gpui_kit::VisualTestContext::from_window(self.window, cx).debug_bounds(id)
    }

    fn press(&self, cx: &mut TestAppContext, id: &'static str) {
        let mut vcx = gpui_kit::VisualTestContext::from_window(self.window, cx);
        let b = vcx.debug_bounds(id).unwrap_or_else(|| panic!("no {id}"));
        vcx.simulate_click(b.center(), Modifiers::default());
        draw(cx, self.window);
    }

    fn pick(&self, cx: &mut TestAppContext, id: &'static str, label: &str) {
        self.press(cx, id);
        let rows = cx.update(|cx| crate::os_menu::open_labels(cx));
        assert!(rows.iter().any(|r| r == label), "{label} in {id}'s {rows:?}");
        self.press(cx, Box::leak(format!("menu:{label}").into_boxed_str()));
        assert!(!cx.update(|cx| crate::os_menu::is_open(cx)), "{label} picked");
    }

    /// Export with what the dialog shows: the settings it exported with.
    fn export(&self, cx: &mut TestAppContext) -> noshape_ops::export::ExportSettings {
        self.press(cx, "modal-ok");
        self.exported.borrow_mut().pop().expect("Export ran")
    }
}

/// Live (the client's saveExportOptions / getExportSolidOptions, user setting exportSolidOptions): Export opens on the
/// last export's format and options, each format keeping its own (STEP's version stays across a switch to Parasolid
/// and back, Parasolid's is still the latest); Cancel keeps nothing.
#[gpui::test]
fn export_dialog_opens_with_the_last_exports_choices(cx: &mut TestAppContext) {
    use noshape_ops::export::PartFormat;
    struct Page;
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }
    cx.update(gpui_kit::init);
    crate::part_studio_hooks::register_for_tests();
    // The app's window layers: dialogs and the lists opened from them.
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) })), ..Default::default() };
    let (window, _) = cx.update(|cx| crate::os_menu::open_window(options, cx, |_, cx| cx.new(|_| Page))).unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    let _prefs = OwnExportPrefs::new("last-choices");
    let d = ExportDialog { window, exported: Default::default() };
    d.open(cx, false);
    let first = d.export(cx);
    // (The units held are STEP's, unseen until its custom units are ticked: Centimeter, live qexport.)
    assert_eq!((first.format, first.latest, first.resolution, first.units), (PartFormat::Parasolid, true, 1, 0), "live's first Export: Parasolid, latest version");

    d.open(cx, false);
    d.pick(cx, "ex-format", "STEP");
    let tops: Vec<f32> = ["ex-latest", "ex-custom-units", "ex-tol", "ex-mbd", "ex-individual", "ex-hidden"].into_iter().map(|id| f32::from(d.bounds(cx, id).unwrap_or_else(|| panic!("no {id}")).origin.y)).collect();
    let gaps: Vec<f32> = [(0, 1), (1, 2), (2, 3), (4, 5)].into_iter().map(|(a, b)| tops[b] - tops[a]).collect();
    assert_eq!(gaps, vec![31., 31., 31., 39.], "STEP's checkboxes under Version 31 px apart, under Options 39 (live wf39)");
    d.press(cx, "ex-tol");
    d.press(cx, "ex-latest");
    d.pick(cx, "ex-version", "AP214");
    assert!(d.bounds(cx, "ex-tol").is_none(), "AP242 only (live 2035)");
    d.press(cx, "ex-yup");
    d.pick(cx, "ex-format", "PARASOLID");
    d.pick(cx, "ex-format", "STEP");
    let step = d.export(cx);
    assert_eq!((step.format, step.version, step.latest, step.derived_tolerances, step.y_up), (PartFormat::Step, 1, false, true, true), "STEP's own choices kept across the switch");

    d.open(cx, false);
    let again = d.export(cx);
    assert_eq!((again.format, again.version, again.latest, again.derived_tolerances, again.y_up), (PartFormat::Step, 1, false, true, true), "reopened on STEP AP214 with its options");

    d.open(cx, false);
    d.pick(cx, "ex-format", "PARASOLID");
    let parasolid = d.export(cx);
    assert!(parasolid.latest && parasolid.y_up, "Parasolid keeps its own version, Y up is shared");

    d.open(cx, false);
    d.pick(cx, "ex-format", "STL");
    d.press(cx, "ex-yup");
    d.press(cx, "modal-cancel");
    assert!(d.exported.borrow().is_empty(), "Cancel exports nothing");
    d.open(cx, false);
    let after_cancel = d.export(cx);
    assert_eq!((after_cancel.format, after_cancel.y_up), (PartFormat::Parasolid, true), "Cancel saved nothing");

    // The Assembly Export shares them: Parasolid, Y up.
    d.open(cx, true);
    let asm = d.export(cx);
    assert_eq!((asm.format, asm.y_up), (PartFormat::Parasolid, true));
}

/// Live import-export-1118: the Export dialog opens with the focus on the dialog, not in File name (a key typed
/// changes no name); Esc closes it and exports nothing; Enter in File name exports at once, with the name typed.
#[gpui::test]
fn export_dialog_keys_as_live(cx: &mut TestAppContext) {
    struct Page;
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }
    cx.update(gpui_kit::init);
    crate::part_studio_hooks::register_for_tests();
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) })), ..Default::default() };
    let (window, _) = cx.update(|cx| crate::os_menu::open_window(options, cx, |_, cx| cx.new(|_| Page))).unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    let _prefs = OwnExportPrefs::new("keys");
    let names = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let open = |cx: &mut TestAppContext| {
        let names = names.clone();
        let run: super::super::export::RunExport = std::rc::Rc::new(move |_, name, _, _| names.borrow_mut().push(name));
        cx.update_window(window, |_, w, cx| super::super::export::open_export_modal("Part Studio 1".into(), false, run, w, cx)).unwrap();
        draw(cx, window);
    };
    let shown = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| gpui_kit::component::WindowExt::has_active_dialog(w, cx)).unwrap();
    open(cx);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("x");
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let ok = vcx.debug_bounds("modal-ok").expect("Export");
    vcx.simulate_click(ok.center(), Modifiers::default());
    draw(cx, window);
    cx.run_until_parked();
    assert_eq!(names.borrow_mut().drain(..).collect::<Vec<_>>(), ["Part Studio 1".to_string()], "the key typed on opening went to no field");
    open(cx);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    draw(cx, window);
    assert!(!shown(cx) && names.borrow().is_empty(), "Esc closed the dialog, nothing exported");
    open(cx);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let field = vcx.debug_bounds("ex-file").expect("File name");
    vcx.simulate_click(field.center(), Modifiers::default());
    vcx.simulate_keystrokes("end 2 enter");
    draw(cx, window);
    assert!(!shown(cx), "Enter in File name closed the dialog");
    cx.run_until_parked();
    assert_eq!(names.borrow().as_slice(), ["Part Studio 12".to_string()], "exported at once, with the name typed");
}

/// Opening a feature's dialog redraws only its toolbar button (now active); the other tools keep their cached views
/// (the whole toolbar was drawn again: ~6-10 ms of the open's frame on a big studio).
#[gpui::test]
fn opening_a_dialog_redraws_only_its_toolbar_button(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "tool-cache");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let buttons = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).tool_cache.iter().map(|(id, (k, e))| (*id, (*k, e.entity_id()))).collect::<std::collections::HashMap<_, _>>());
    let before = buttons(cx);
    assert!(before.contains_key("extrude") && before.contains_key("revolve"), "{:?}", before.keys());
    assert_eq!(before["extrude"].0, (false, false));
    view.update(cx, |v, _| v.selection = vec![top.clone()]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let after = buttons(cx);
    assert_eq!(after["extrude"].0, (true, false), "the open tool shows active");
    assert_ne!(after["extrude"].1, before["extrude"].1, "and is drawn again");
    assert_eq!(after["revolve"], before["revolve"], "another tool keeps its cached view");
}

/// User P1 (11-05): the public Displacement map (a custom feature with editing logic; its preview a sheet beside the
/// part, "Surfaces (1)") on a cylinder, its preview listed, a value changed and Esc before the new preview and the
/// editing logic are in: the model before the dialog at once (no sheet listed, no spinner), and so it stays once the
/// cancelled work's results arrive.
#[gpui::test]
fn cancel_a_displacement_map_after_its_preview_is_listed(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "cancel-displacement");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    let cylinder = r#"FeatureScript 3044;
import(path : "onshape/std/common.fs", version : "3044.0");
annotation { "Feature Type Name" : "Test cylinder" }
export const testCylinder = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        fCylinder(context, id + "c", { "bottomCenter" : vector(0, 0, 0) * meter, "topCenter" : vector(0, 0, 60) * millimeter, "radius" : 25 * millimeter });
    });
"#;
    let csv: String = (0..24).map(|i| (0..24).map(|j| format!("{}", ((((i + 2 * j) as f64) * 0.7).sin() * 127.5 + 127.5).round() as i32)).collect::<Vec<_>>().join(",") + "\n").collect();
    let (disp, table) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            let units = v.doc.read(cx).session.doc.settings.units;
            let mut ids = None;
            v.doc.update(cx, |doc, cx| {
                doc.edit_document("Add tabs", cx, |d| {
                    let disp = d.add_tab("Displacement map", noshape_model::TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(include_str!("../../../model/tests/fs/displacement_map.fs"))));
                    let cyl = d.add_tab("Cylinder", noshape_model::TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(cylinder)));
                    let table = d.add_tab("leather.csv", noshape_model::TabContent::File(noshape_model::file_tab::FileTab::new("leather.csv", csv.as_bytes(), 0)));
                    ids = Some((disp, cyl, table));
                });
                let (_, cyl, _) = ids.expect("the tabs");
                // The Feature Studios compiled: their feature types known.
                let _ = doc.custom_features();
                doc.edit_studio(tab, "Add cylinder", cx, |ps| {
                    let f = ps.new_feature(&noshape_model::custom::type_id(cyl, "testCylinder"), &units).expect("the cylinder feature");
                    ps.features.push(f);
                });
            });
            let (disp, _, table) = ids.expect("the tabs");
            (disp, table)
        })
    });
    wait_until(cx, window, |cx| view.read(cx).pending.is_none() && view.read(cx).display.bodies.len() == 1);
    let before = cx.update(|cx| view.read(cx).display.clone());
    assert_eq!(before.bodies.len(), 1, "the cylinder");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let ctx = v.context.clone().expect("the cylinder's context");
            let b = &ctx.bodies[0];
            let side = (0..b.faces.len())
                .find(|&i| matches!(b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).and_then(|f| f.face_surface()), Ok(noshape_kernel::Surface::Cylinder { .. })))
                .expect("the side face");
            let face = (*b.faces[side]).clone();
            v.start_feature(&noshape_model::custom::type_id(disp, "displacementMap"), w, cx);
            let d = v.edit.as_ref().expect("the dialog").dialog.clone();
            d.update(cx, |d, _| {
                let p = &mut d.feature.params;
                p.set("imageTable", noshape_model::ParamValue::Text(table.to_string()));
                p.set("face", noshape_model::ParamValue::Query(vec![Reference::Face(face)]));
                p.set("degree", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("5")));
                p.set("blackValue", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("0 mm")));
                p.set("whiteValue", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("1 mm")));
            });
            v.schedule_regen(w, cx);
        })
    })
    .unwrap();
    let listed = |v: &PartStudioView| v.display.bodies.iter().map(|b| super::super::part_section(b)).collect::<Vec<_>>();
    wait_until(cx, window, |cx| view.read(cx).display.bodies.len() == 2);
    assert_eq!(cx.update(|cx| listed(view.read(cx))), ["parts", "surfaces"], "the preview's sheet listed");
    // A value changed (std's editing logic and a new preview start), then Esc before either is in.
    cx.update(|cx| {
        let d = view.read(cx).edit.as_ref().expect("the dialog").dialog.clone();
        d.update(cx, |d, cx| d.set_value("degree", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("4")), cx));
    });
    let after_esc = cx
        .update_window(window, |_, w, cx| {
            assert!(view.read(cx).pending.is_some() && view.read(cx).ui.std_edit_task.is_some(), "the editing logic and the new preview running");
            let focus = view.read(cx).focus.clone();
            w.focus(&focus, cx);
            w.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            let v = view.read(cx);
            (v.edit.is_some(), v.pending.is_some(), listed(v), std::sync::Arc::ptr_eq(&v.display, &before))
        })
        .unwrap();
    assert_eq!(after_esc, (false, false, vec!["parts"], true), "(dialog open, spinner, listed, other model) right after Esc");
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let settled = cx.update(|cx| {
        let v = view.read(cx);
        (v.pending.is_some(), listed(v), std::sync::Arc::ptr_eq(&v.display, &before))
    });
    assert_eq!(settled, (false, vec!["parts"], true), "(spinner, listed, other model) once the late results are in");
}

/// A Part Studio with one committed part (a 150 × 100 px rectangle on Top, extruded), settled.
fn one_part(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = sketch_with(cx, name, Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).pending.is_none() && !view.read(cx).display.sketches.is_empty());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let sk = v.display.sketches.first().expect("the sketch").clone();
            v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().expect("a region").key.clone() }];
            v.start_feature("extrude", w, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).display.bodies.len() == 1);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_open_dialog(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).pending.is_none() && view.read(cx).edit.is_none() && view.read(cx).display.bodies.len() == 1);
    assert_eq!(cx.update(|cx| view.read(cx).display.bodies.len()), 1, "Part 1");
    (window, view)
}

/// The Regenerator held on another thread (a cancelled preview in a kernel step that doesn't stop) until the
/// returned sender is dropped.
fn hold_regenerator(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> std::sync::mpsc::Sender<()> {
    let regen = cx.update(|cx| view.read(cx).regen.clone());
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let _held = regen.lock();
        let _ = held_tx.send(());
        let _ = release_rx.recv();
    });
    held_rx.recv().unwrap();
    release_tx
}

/// Frames for `ms` of real time (background work runs meanwhile).
fn frames_for(cx: &mut TestAppContext, window: AnyWindowHandle, ms: u64) {
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(ms) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        draw(cx, window);
    }
}

/// TICKETS P0 HEAVY-ASM-SOFTWARE-FRAME (1): a window redraw that changes nothing in the 3D view asks for no GPU frame (on
/// a software renderer a big assembly's frame took ~47 s): the studio re-rendered, the pointer resting on a list button
/// for its tooltip, then over the feature list's empty end.
#[gpui::test]
fn ui_only_redraws_submit_no_3d_frame(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "ui-only-frames");
    frames_for(cx, window, 300);
    let submitted = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).frames_requested());
    let before = submitted(cx);
    let mut after = Vec::new();
    for _ in 0..5 {
        view.update(cx, |_, cx| cx.notify());
        draw(cx, window);
    }
    after.push(submitted(cx));
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let button = vcx.debug_bounds("button:New folder").expect("the New folder button").center();
    let list = vcx.debug_bounds("feature-list").expect("the feature list");
    vcx.simulate_event(gpui_kit::MouseMoveEvent { position: button, pressed_button: None, modifiers: Modifiers::default() });
    frames_for(cx, window, 600);
    after.push(submitted(cx));
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_event(gpui_kit::MouseMoveEvent { position: gpui_kit::point(list.center().x, list.bottom() - px(4.)), pressed_button: None, modifiers: Modifiers::default() });
    frames_for(cx, window, 300);
    after.push(submitted(cx));
    // A real change still asks: the top face selected.
    view.update(cx, |v, cx| {
        v.selection = vec![top];
        v.refresh_scene(false, cx);
    });
    after.push(submitted(cx));
    assert_eq!((&after[..3], after[3] > before), (&vec![before; 3][..], true), "frames asked for after: 5 re-renders, the button's tooltip, the list's empty end; a face selected (before: {before})");
}

/// User P1 (11-05): a new part's dialog whose preview is slow (its regeneration not in when Esc comes, then held in a
/// kernel step that doesn't stop), cancelled: the parts list shows the model before the dialog and the spinner is off
/// at once and while the cancelled work still runs, and both stay so once it ends.
#[gpui::test]
fn cancel_a_slow_preview_shows_the_model_before_it_at_once_and_after(cx: &mut TestAppContext) {
    let (window, view) = one_part(cx, "cancel-slow-preview");
    let before = cx.update(|cx| view.read(cx).display.clone());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let sk = v.display.sketches.first().expect("the sketch").clone();
            v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().expect("a region").key.clone() }];
            v.start_feature("extrude", w, cx);
            let d = v.edit.as_ref().expect("the dialog").dialog.clone();
            d.update(cx, |d, _| d.feature.params.set("operationType", noshape_model::ParamValue::Enum("NEW".into())));
            v.schedule_regen(w, cx);
        })
    })
    .unwrap();
    let release = hold_regenerator(cx, &view);
    let state = |v: &PartStudioView| (v.edit.is_some(), v.pending.is_some(), v.display.bodies.len(), std::sync::Arc::ptr_eq(&v.display, &before));
    let after_esc = cx
        .update_window(window, |_, w, cx| {
            assert!(view.read(cx).pending.is_some(), "the preview regenerating");
            let focus = view.read(cx).focus.clone();
            w.focus(&focus, cx);
            w.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            state(view.read(cx))
        })
        .unwrap();
    assert_eq!(after_esc, (false, false, 1, true), "(dialog open, spinner, parts, other model) right after Esc");
    frames_for(cx, window, 1500);
    assert_eq!(cx.update(|cx| state(view.read(cx))), (false, false, 1, true), "while the cancelled work still runs");
    drop(release);
    frames_for(cx, window, 1000);
    assert_eq!(cx.update(|cx| state(view.read(cx))), (false, false, 1, true), "once it has ended");
}

/// Live (spinner, live's client code 11-18): one working spinner, shown 900 ms after a regeneration starts and none if it
/// ends sooner; a 50 × 50 box centred 40 px above the graphics' bottom.
#[gpui::test]
fn the_working_spinner_shows_900_ms_into_a_regeneration_centred_low(cx: &mut TestAppContext) {
    let (window, view) = one_part(cx, "pending-spinner");
    let spinner = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("pending-spinner");
    let start = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let sk = v.display.sketches.first().expect("the sketch").clone();
                v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().expect("a region").key.clone() }];
                v.start_feature("extrude", w, cx);
            })
        })
        .unwrap();
        (std::time::Instant::now(), cx.executor().now())
    };
    // The spinner's delay runs on the executor's clock: frames_for advances it with the wall clock, the slow case below
    // by itself (a slow runner's frames can't put it past 900 ms).
    let sim = |cx: &mut TestAppContext, since: std::time::Instant| cx.executor().now().saturating_duration_since(since);
    // A quick preview: no spinner at any frame (when the host ran it within the 900 ms).
    let (t, t_sim) = start(cx);
    let mut seen = false;
    while cx.update(|cx| view.read(cx).pending.is_some()) && t.elapsed() < std::time::Duration::from_secs(30) {
        frames_for(cx, window, 20);
        seen |= spinner(cx).is_some();
    }
    if sim(cx, t_sim) < super::super::pending::OVERLAY_DELAY {
        assert!(!seen, "a spinner for a regeneration done in {:?}", sim(cx, t_sim));
    }
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).pending.is_none() && view.read(cx).edit.is_none());
    // A slow one (a regeneration not ending; a held Regenerator would stall the test executor): nothing until 900 ms,
    // then the spinner until it ends.
    let seq = u64::MAX;
    let t = cx.executor().now();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.begin_pending(seq, w, cx))).unwrap();
    while sim(cx, t) < std::time::Duration::from_millis(880) {
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        draw(cx, window);
        assert!(spinner(cx).is_none(), "a spinner at {:?}", sim(cx, t));
    }
    cx.executor().advance_clock(std::time::Duration::from_millis(40));
    draw(cx, window);
    let b = spinner(cx).expect("the spinner past 900 ms");
    let area = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("viewport-area").expect("the graphics");
    let f = |p: gpui_kit::Pixels| f32::from(p);
    assert_eq!((f(b.size.width), f(b.size.height)), (50., 50.), "the box");
    assert!((f(b.center().x) - f(area.center().x)).abs() < 1., "centred: {b:?} in {area:?}");
    assert!((f(area.bottom()) - f(b.bottom()) - 40.).abs() < 1., "40 px above the bottom: {b:?} in {area:?}");
    view.update(cx, |v, cx| v.end_pending(seq, cx));
    draw(cx, window);
    assert!(spinner(cx).is_none(), "the spinner gone with the result");
}

/// Live cfg-1118: switching configurations greys nothing (no row, the old model drawn as it is) and the working
/// spinner (the_working_spinner_…) comes only past 900 ms.
#[gpui::test]
fn switching_configurations_greys_nothing_and_spins_only_past_900_ms(cx: &mut TestAppContext) {
    let (window, view) = one_part(cx, "cfg-switch-spinner");
    let input = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let id = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "extrude")).map(|f| f.id).expect("Extrude 1");
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                let b = noshape_ops::config::add_option(ps, &input, "B".into()).expect("row B");
                noshape_ops::config::configure_parameter(ps, id, "depth", &input);
                noshape_ops::config::set_configured_text(ps, 0, &b, "40 mm".into());
            });
            input
        })
    });
    wait_until(cx, window, |cx| view.read(cx).pending.is_none());
    let spinner = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("pending-spinner").is_some();
    let greyed = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        v.studio(cx).map_or(0, |s| s.features.len().saturating_sub(v.row_ctx.rollback))
    });
    let before = cx.update(|cx| view.read(cx).display.clone());
    let (t, t_sim) = (std::time::Instant::now(), cx.executor().now());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.set_config_value(&input, "B".into(), w, cx))).unwrap();
    let same = |cx: &mut TestAppContext| cx.update(|cx| std::sync::Arc::ptr_eq(&view.read(cx).display, &before));
    assert_eq!((cx.update(|cx| view.read(cx).pending.is_some()), greyed(cx), same(cx)), (true, 0, true), "(regenerating, grey rows, the old model) at once");
    while cx.update(|cx| view.read(cx).pending.is_some()) && t.elapsed() < std::time::Duration::from_secs(30) {
        frames_for(cx, window, 20);
        assert_eq!(greyed(cx), 0, "grey rows at {:?}", t.elapsed());
        // The spinner's delay runs on the executor's clock (frames_for advances it with the wall clock).
        let at = cx.executor().now().saturating_duration_since(t_sim);
        if at < super::super::pending::OVERLAY_DELAY {
            assert!(!spinner(cx), "a spinner at {at:?}");
        }
    }
    draw(cx, window);
    let changed = !same(cx);
    assert_eq!((spinner(cx), greyed(cx), changed), (false, 0, true), "(spinner, grey rows, B drawn) once regenerated");
}

/// Live2 ours-1118 prefs: with "Display parts list properties" on, each part's row carries its state pill ("In progress"
/// while none is set); off, none.
#[gpui::test]
fn parts_list_properties_put_the_state_pill_on_each_part(cx: &mut TestAppContext) {
    let (window, view) = one_part(cx, "parts-props");
    let part = cx.update(|cx| view.read(cx).display.bodies.first().expect("Part 1").id.clone());
    let pill = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("part-state-{part}").into_boxed_str())).is_some();
    let state = cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.part_properties.get(&part)).and_then(|p| p.meta.state.clone()));
    assert_eq!((pill(cx), state), (false, None), "(pill, state) with the preference off");
    view.update(cx, |v, cx| {
        v.ui.show_part_props = true;
        cx.notify();
    });
    draw(cx, window);
    assert!(pill(cx), "the state pill (\"In progress\") with the preference on");
}

/// Live2 ours-1118 view: Perspective on / off and a render mode (Translucent, Shaded) change the picture in one step,
/// with no camera animation; the perspective's field of view is 45°.
#[gpui::test]
fn perspective_and_render_modes_switch_in_one_step(cx: &mut TestAppContext) {
    let (window, view) = one_part(cx, "view-switch");
    let state = |v: &PartStudioView, cx: &gpui_kit::App| {
        let vp = v.viewport.read(cx);
        (vp.camera.projection, vp.animating(), vp.scene().no_occlusion)
    };
    let fov = cx.update(|cx| view.read(cx).viewport.read(cx).camera.fov_y.to_degrees());
    assert!((fov - 45.0).abs() < 1e-9, "the field of view {fov}°");
    cx.update_window(window, |_, w, cx| {
        w.activate_window();
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    // Window::dispatch_action is deferred to the end of the update: the state read in the next one, no frame between.
    let toggle = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| w.dispatch_action(Box::new(crate::actions::ToggleProjection), cx)).unwrap();
        cx.update(|cx| state(view.read(cx), cx))
    };
    assert_eq!(toggle(cx), (noshape_render::Projection::Perspective, false, false), "(projection, animating, see-through) right after Perspective");
    let translucent = cx
        .update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| v.set_display_mode(crate::scene_builder::DisplayMode::Translucent, w, cx));
            state(view.read(cx), cx)
        })
        .unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.set_display_mode(crate::scene_builder::DisplayMode::ShadedWithEdges, w, cx))).unwrap();
    let modes = (translucent, toggle(cx));
    assert_eq!(
        modes,
        ((noshape_render::Projection::Perspective, false, true), (noshape_render::Projection::Orthographic, false, false)),
        "(projection, animating, see-through) right after Translucent, then Shaded with edges and Perspective off"
    );
}

/// User P1 (11-05), the user's sequence: the part deleted (its regeneration not shown yet), a new dialog opened at
/// once, its preview slow (held in a kernel step), cancelled. The model before the dialog shows at once, then the
/// document as it is (no part) while the cancelled step still runs: no spinner waiting for it, and its late result
/// changes nothing.
#[gpui::test]
fn cancel_a_slow_preview_opened_before_the_last_edit_was_shown(cx: &mut TestAppContext) {
    let (window, view) = one_part(cx, "cancel-slow-after-edit");
    let release = hold_regenerator(cx, &view);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            let extrude = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "extrude").map(|f| f.id)).expect("Extrude 1");
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Delete Extrude 1", cx, |ps| ps.features.retain(|f| f.id != extrude));
            });
        })
    })
    .unwrap();
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            assert_eq!(v.display.bodies.len(), 1, "the deletion not shown yet");
            let sk = v.display.sketches.first().expect("the sketch").clone();
            v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions.first().expect("a region").key.clone() }];
            v.start_feature("extrude", w, cx);
        })
    })
    .unwrap();
    let after_esc = cx
        .update_window(window, |_, w, cx| {
            let focus = view.read(cx).focus.clone();
            w.focus(&focus, cx);
            w.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            let v = view.read(cx);
            (v.edit.is_some(), v.pending.is_some(), v.display.bodies.len())
        })
        .unwrap();
    assert_eq!(after_esc, (false, false, 1), "(dialog open, spinner, parts) right after Esc: the model before the dialog");
    wait_until(cx, window, |cx| view.read(cx).pending.is_none() && view.read(cx).display.bodies.is_empty());
    let state = |v: &PartStudioView| (v.pending.is_some(), v.display.bodies.len());
    assert_eq!(cx.update(|cx| state(view.read(cx))), (false, 0), "the document as it is while the cancelled step still runs");
    drop(release);
    frames_for(cx, window, 1000);
    assert_eq!(cx.update(|cx| state(view.read(cx))), (false, 0), "once its late result is in");
}

/// Live (ui-parity sketch-empty-rclick-menu): in a sketch on Top, a right-click where the Front plane is under the
/// pointer gives the plane's rows; the sketch's own plane gives its own (live qa walk 5 b-rclick-75-40).
#[gpui::test]
fn a_plane_behind_the_sketch_gets_its_menu(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "sketch-plane-menu", Tool::Select);
    let labels_over = |cx: &mut TestAppContext, plane: DatumPlane| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.hover = Some(Reference::Plane(plane));
                v.open_sketch_context_menu(gpui_kit::point(px(600.), px(400.)), None, w, cx);
            })
        })
        .unwrap();
        draw(cx, window);
        let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
        cx.update(|cx| view.update(cx, |v, cx| v.close_context_menu(cx)));
        labels
    };
    let want = [
        "Confirm Sketch 1", "———", "Hide Front plane", "Hide other planes", "Hide all planes", "Section view…", "———",
        "Clear selection", "Select", "Select other…", "———", "Zoom to fit", "Zoom to selection", "View normal to",
        "View normal to sketch plane",
    ];
    assert_eq!(labels_over(cx, DatumPlane::Front), want);
    let own = labels_over(cx, DatumPlane::Top);
    assert_eq!(own[2], "Hide Top plane", "{own:?}");
}

/// Live 11-17 ctx-menus-1117 (menus-sk.json): the sketch's right-click menus on a line, on its end, on a line and its
/// end selected, with Line drawing (Add comment, a cloud item, left out). Hide other / all sketches with no other sketch
/// shown (qa MENU-SK-HIDE-SKETCHES).
#[gpui::test]
fn sketch_entity_menus_as_lives(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "sketch-entity-menus", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape"]);
    let (feature, line, end) = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.sketch_data(cx).unwrap();
        let l = d.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
        (v.sketch.as_ref().unwrap().feature, l, d.sketch.endpoints(l).unwrap().1)
    });
    let menu = |cx: &mut TestAppContext, over: noshape_sketch::EntityId, selected: Vec<noshape_sketch::EntityId>, tool: Tool| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.activate_tool(tool, w, cx);
                if let Some(ed) = &mut v.sketch {
                    ed.selection = selected.clone();
                }
                v.hover = Some(Reference::SketchEntity { sketch: feature, entity: over.0 });
                v.open_sketch_context_menu(gpui_kit::point(px(600.), px(400.)), None, w, cx);
            })
        })
        .unwrap();
        draw(cx, window);
        let labels: Vec<String> = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
        cx.update(|cx| view.update(cx, |v, cx| v.close_context_menu(cx)));
        labels.into_iter().filter(|l| l != "———").collect::<Vec<_>>()
    };
    let line_menu = ["Confirm Sketch 1", "Copy sketch entities", "Transform sketch entities", "Construction", "Extrude…", "Revolve…", "Hide other sketches", "Hide all sketches", "Curve/surface analysis…", "Clear selection", "Select", "Select other…", "Zoom to fit", "Zoom to selection", "View normal to sketch plane", "Align view with vertical axis", "Delete sketch entity"];
    let mut wrong = Vec::new();
    for (what, got, want) in [
        ("a line", menu(cx, line, vec![], Tool::Select), line_menu.to_vec()),
        ("a selected line", menu(cx, line, vec![line], Tool::Select), line_menu.to_vec()),
        ("a line's end", menu(cx, end, vec![], Tool::Select), vec!["Confirm Sketch 1", "Hide other sketches", "Hide all sketches", "Clear selection", "Select", "Select other…", "Zoom to fit", "View normal to sketch plane", "Delete sketch entity"]),
        ("a line and its end", menu(cx, line, vec![line, end], Tool::Select), vec!["Confirm Sketch 1", "Copy sketch entities", "Transform sketch entities", "Hide other sketches", "Hide all sketches", "Curve/surface analysis…", "Clear selection", "Select", "Select other…", "Zoom to fit", "Zoom to selection", "View normal to sketch plane", "Delete 2 sketch entities"]),
    ] {
        if got != want.iter().map(|s| s.to_string()).collect::<Vec<_>>() {
            wrong.push(format!("{what}: ours {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Lead 11-06, the display cache: a regenerated display comes back from the cache as it was; shown in its place the
/// view is view-only (a dialog asked for waits, the user told at once, "Regenerating…" in the list), and the real
/// regeneration replaces it with the same meshes and opens the dialog.
#[gpui::test]
fn a_cached_display_stands_in_until_the_identical_real_one_lands(cx: &mut TestAppContext) {
    let (window, view, top, _) = block_part(cx, "display-cache");
    // The block's regeneration done, its refined display too (it would end the stand-in below).
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert!(!cx.update(|cx| view.read(cx).ui_busy()), "the block's regeneration finished");
    let real = cx.update(|cx| view.read(cx).display.clone());
    type Meshes = Vec<(String, Vec<[f32; 3]>, Vec<u32>, Vec<(u32, u32)>, usize)>;
    let meshes = |d: &noshape_model::display::StudioDisplay| -> Meshes {
        d.bodies.iter().map(|b| (b.name.clone(), b.mesh.positions.clone(), b.mesh.indices.clone(), b.mesh.face_ranges.clone(), b.mesh.edges.len())).collect()
    };
    assert!(!real.bodies.is_empty(), "the block's part");
    let dir = std::env::temp_dir().join(format!("noshape-display-cache-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::super::display_cache::store(&super::super::display_cache::Shelf::dir(&dir), "k", &real).unwrap();
    let cached = super::super::display_cache::load(&super::super::display_cache::Shelf::dir(&dir), "k").expect("the cached display");
    assert_eq!(meshes(&cached), meshes(&real), "cached as it was");
    assert_eq!((cached.planes.len(), cached.sketches.len()), (real.planes.len(), real.sketches.len()));
    let _ = std::fs::remove_dir_all(&dir);
    // Reopened: the cached display stands in.
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.ui.real_display_in = false;
            v.show_cached_display("k".into(), cached, w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).ui.cached_view && view.read(cx).render_regenerating_row().is_some()), "view-only, Regenerating… shown");
    // Lead 11-21 (fast open step 4): a click on the part selects its face at once, with no waiting.
    cx.update(|cx| view.update(cx, |v, _| {
        v.selection.clear();
        v.message = None;
    }));
    let lit = cx
        .update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let id = v.built.as_ref().and_then(|b| b.picks.ids_for(&top).first().copied()).expect("the top face's pick");
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::Hover(Some(id)), w, cx);
                let lit = v.hover.clone();
                v.on_viewport_event(&vp, &ViewportEvent::Click { pick: Some(id), position: DVec2::new(575., 350.), modifiers: Modifiers::default() }, w, cx);
                lit
            })
        })
        .unwrap();
    draw(cx, window);
    let (picked, said) = cx.update(|cx| (view.read(cx).selection.clone(), view.read(cx).message.clone()));
    assert_eq!(lit, Some(top.clone()), "the top face lit under the pointer");
    assert_eq!(picked, vec![top.clone()], "the top face selected on the cached model ({said:?})");
    assert!(said.is_none(), "no waiting for a click");
    cx.update(|cx| view.update(cx, |v, _| v.selection.clear()));
    // A dialog waits, with feedback at once.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    draw(cx, window);
    let (opened, said) = cx.update(|cx| (view.read(cx).edit.is_some(), view.read(cx).message.as_ref().map(|(_, m)| m.clone())));
    assert!(!opened, "no dialog on the cached model");
    assert_eq!(said.as_deref(), Some(crate::display_cache::WAITING));
    // The real regeneration lands: the same meshes, the dialog opens.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.schedule_regen(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).edit.is_some());
    let after = cx.update(|cx| view.read(cx).display.clone());
    assert!(!cx.update(|cx| view.read(cx).ui.cached_view), "the model's own again");
    assert_eq!(meshes(&after), meshes(&real), "the regenerated display is the cached one");
}

/// Lead 11-07, the display cache frame by frame (T1: no GPU, the scenes the frames are drawn from): from the first
/// scene that draws the cached display, every scene the viewport is given draws the model's own frame (scene, camera,
/// full detail), through the real regeneration's arrival and after it: no blank frame, no flash.
#[gpui::test]
fn the_swap_from_the_cached_display_to_the_real_one_changes_no_scene(cx: &mut TestAppContext) {
    // Uploads hash what they hold from the start, the real display's too.
    noshape_render::scene::keep_content_keys(true);
    let (window, view, _top, _) = block_part(cx, "display-cache-scenes");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert!(!cx.update(|cx| view.read(cx).ui_busy()), "the block's regeneration finished");
    let real = cx.update(|cx| view.read(cx).display.clone());
    let dir = std::env::temp_dir().join(format!("noshape-display-cache-scenes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::super::display_cache::store(&super::super::display_cache::Shelf::dir(&dir), "k", &real).unwrap();
    let cached = super::super::display_cache::load(&super::super::display_cache::Shelf::dir(&dir), "k").expect("the cached display");
    let _ = std::fs::remove_dir_all(&dir);
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    let model = cx.update(|cx| vp.read(cx).frame_signature());
    cx.update(|cx| vp.update(cx, |v, _| v.record_scenes(true)));
    // Reopened: nothing drawn yet.
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.ui.real_display_in = false;
            v.apply_display(std::sync::Arc::new(noshape_model::display::StudioDisplay::default()), w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    let blank = cx.update(|cx| vp.read(cx).frame_signature());
    assert_ne!(blank, model, "the empty view differs from the model");
    // The cached display (its scene built off the UI thread), then the real regeneration.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.show_cached_display("k".into(), cached, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).ui.cached_view);
    assert_eq!(cx.update(|cx| vp.read(cx).frame_signature()), model, "the cached display draws the model's frame");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.schedule_regen(w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui.cached_view && !view.read(cx).ui_busy());
    assert!(cx.update(|cx| !view.read(cx).ui.cached_view && !view.read(cx).ui_busy()), "the real display landed");
    let scenes = cx.update(|cx| vp.read(cx).recorded_scenes());
    let first = scenes.iter().position(|s| *s != blank).expect("a scene of the cached model");
    for (i, s) in scenes.iter().enumerate().skip(first) {
        assert_eq!(*s, model, "scene {i} of {} isn't the model's frame", scenes.len());
    }
    assert_eq!(cx.update(|cx| vp.read(cx).frame_signature()), model, "the regenerated display draws the model's frame");
}

/// Lead 11-07, the display cache frame by frame: from the first frame that draws the cached display, every presented
/// frame is the model's own picture, through the real regeneration's arrival and after it (no blank frame, no flash).
#[gpui::test]
#[ignore = "T2: GPU pixels (macOS CI's readback came out white, run 42333748082); T1: ..._changes_no_scene"]
fn the_swap_from_the_cached_display_to_the_real_one_changes_no_frame(cx: &mut TestAppContext) {
    let _gpu = noshape_ops::doc::gpu_lock();
    // Rendering needs a GPU adapter; CI's Linux runner has none.
    if let Err(e) = Gpu::headless_rendering() {
        return eprintln!("frames skipped: {e}");
    }
    RENDERING.set(true);
    let (window, view, _top, _) = block_part(cx, "display-cache-frames");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert!(!cx.update(|cx| view.read(cx).ui_busy()), "the block's regeneration finished");
    let real = cx.update(|cx| view.read(cx).display.clone());
    let dir = std::env::temp_dir().join(format!("noshape-display-cache-frames-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::super::display_cache::store(&super::super::display_cache::Shelf::dir(&dir), "k", &real).unwrap();
    let cached = super::super::display_cache::load(&super::super::display_cache::Shelf::dir(&dir), "k").expect("the cached display");
    let _ = std::fs::remove_dir_all(&dir);
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    let pixels = |i: &std::sync::Arc<gpui_kit::RenderImage>| i.as_bytes(0).map(|b| b.to_vec()).unwrap_or_default();
    // The picture of the regenerated model, settled.
    let settled = |cx: &mut TestAppContext| {
        cx.update(|cx| vp.update(cx, |v, _| v.record_frames(true)));
        // A software GPU's first frames compile their pipelines: up to seconds.
        let t = std::time::Instant::now();
        // Two full-detail frames (not navigation's reduced ones: the sketch's camera move ends first).
        while (cx.update(|cx| vp.read(cx).recorded_frames().len()) < 2 || cx.update(|cx| vp.read(cx).interactive())) && t.elapsed() < std::time::Duration::from_secs(15) {
            if cx.update(|cx| vp.read(cx).interactive()) {
                cx.update(|cx| vp.update(cx, |v, _| v.record_frames(true)));
            }
            cx.update(|cx| vp.update(cx, |v, cx| v.request_frame(cx)));
            cx.update(|cx| view.read(cx).gpu.deliver_frames());
            draw(cx, window);
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        cx.update(|cx| vp.read(cx).recorded_frames()).last().map(pixels).expect("a presented frame")
    };
    let model = settled(cx);
    // Reopened: nothing drawn yet.
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.ui.real_display_in = false;
            v.apply_display(std::sync::Arc::new(noshape_model::display::StudioDisplay::default()), w, cx);
        })
    })
    .unwrap();
    let blank = settled(cx);
    assert_ne!(blank, model, "the empty view differs from the model");
    // The cached display, then the real regeneration, every presented frame recorded.
    cx.update(|cx| vp.update(cx, |v, _| v.record_frames(true)));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.show_cached_display("k".into(), cached, w, cx))).unwrap();
    // Frames drawn and handed over until `done` (at most 15 s).
    let pump = |cx: &mut TestAppContext, done: &dyn Fn(&gpui_kit::App) -> bool| {
        let t = std::time::Instant::now();
        while !cx.update(|cx| done(cx)) && t.elapsed() < std::time::Duration::from_secs(15) {
            cx.update(|cx| vp.update(cx, |v, cx| v.request_frame(cx)));
            cx.update(|cx| view.read(cx).gpu.deliver_frames());
            draw(cx, window);
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    pump(cx, &|cx| vp.read(cx).recorded_frames().iter().any(|f| pixels(f) != blank));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.schedule_regen(w, cx))).unwrap();
    pump(cx, &|cx| !view.read(cx).ui.cached_view);
    let at_swap = cx.update(|cx| vp.read(cx).recorded_frames().len());
    // Later frames (each asked for, as the pointer or a highlight does).
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_secs(15) {
        if cx.update(|cx| vp.read(cx).recorded_frames().len()) >= at_swap + 3 {
            break;
        }
        cx.update(|cx| vp.update(cx, |v, cx| v.request_frame(cx)));
        cx.update(|cx| view.read(cx).gpu.deliver_frames());
        draw(cx, window);
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let frames: Vec<Vec<u8>> = cx.update(|cx| vp.read(cx).recorded_frames()).iter().map(pixels).collect();
    let first = frames.iter().position(|f| *f != blank).expect("a frame of the cached model");
    for (i, f) in frames.iter().enumerate().skip(first) {
        assert!(*f == model, "frame {i} of {} (the swap after frame {at_swap}) isn't the model's picture", frames.len());
    }
    assert!(frames.len() >= at_swap + 3, "frames after the real display landed");
}


/// User P0 11-08: a spline saved before handle arms (the 11-06 entities) is given them on entering its sketch, in the
/// dialog's copy only; the shown display, regenerated without them, drew none and the overlay left them to the scene:
/// no arm lines until another regeneration. The overlay draws the arms the shown display doesn't, the entry
/// regenerates so the scene has them, and giving them isn't an undo step.
#[gpui::test]
fn a_spline_from_before_handle_arms_shows_its_arms_on_entering_its_sketch(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "spline-arms", Some(block));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // Its spline as saved before arms: the tips, no arm lines.
    let id = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            let id = v.studio(cx).unwrap().features.iter().find(|f| f.name == "Sketch 1").unwrap().id;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Spline", cx, |ps| {
                    let f = ps.features.iter_mut().find(|f| f.id == id).unwrap();
                    let Some(noshape_model::params::ParamValue::Sketch(mut arc)) = f.params.get("sketch").cloned() else { panic!("a sketch") };
                    let d = std::sync::Arc::make_mut(&mut arc);
                    let s = d.sketch.add_fit_spline_with_handles(&[glam::DVec2::new(0.0, 0.05), glam::DVec2::new(0.02, 0.07), glam::DVec2::new(0.04, 0.05)]).unwrap();
                    // (Arms go only with their spline: the saved form without them.)
                    let mut json = serde_json::to_value(&d.sketch).unwrap();
                    if let Some(serde_json::Value::Object(es)) = json.get_mut("entities") {
                        es.retain(|_, e| e.get("handle_of").is_none_or(serde_json::Value::is_null));
                    }
                    d.sketch = serde_json::from_value(json).unwrap();
                    assert!(d.sketch.handle_arms(s).iter().all(Option::is_none));
                    f.params.set("sketch", noshape_model::params::ParamValue::Sketch(arc));
                })
            });
            id
        })
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    // Every arm the dialog's sketch has is drawn: by the scene (the shown display has it) or by the overlay.
    let drawn = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().expect("the sketch editor");
            let data = v.sketch_data(cx).expect("its data");
            let arms: Vec<_> = data.sketch.entities().filter(|e| e.handle_of.is_some()).map(|e| e.id).collect();
            let in_display: Vec<_> = v.display.sketches.iter().filter(|s| s.feature == id).flat_map(|s| s.curves.iter().filter(|c| c.handle).map(|c| noshape_sketch::EntityId(c.entity))).collect();
            let scene = ed.scene_arms.clone().expect("the scene's arms");
            assert!(scene.iter().all(|a| in_display.contains(a)), "the scene draws only the display's arms");
            (arms.len(), arms.iter().filter(|a| in_display.contains(a)).count())
        })
    };
    let (arms, _) = drawn(cx);
    assert_eq!(arms, 2, "given on entry");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!(drawn(cx), (2, 2), "regenerated: the scene draws both");
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (false, false), "not an undo step");
}

/// psui / skasm 11-18 (the user's hat15b, Sketch 1): a sketch whose stored geometry its regeneration solves elsewhere (an
/// upstream change moved it 1.1 mm) is edited where it is shown: the dialog's draft takes the solve on entering, so
/// hover, snaps and drags act on the drawn lines (its line lit with the pointer 30 px beside it, a click on it took the
/// region). Not an undo step.
#[gpui::test]
fn an_opened_sketch_is_edited_where_its_solve_shows_it(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "line": [[0, 0], [40, 10]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "solved-draft", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // Stored as drawn, with a Horizontal its line doesn't hold yet (as a sketch whose upstream moved).
    let (id, line) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            let id = v.studio(cx).unwrap().features.iter().find(|f| f.name == "Sketch 1").unwrap().id;
            let mut line = None;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Horizontal", cx, |ps| {
                    let f = ps.features.iter_mut().find(|f| f.id == id).unwrap();
                    let Some(noshape_model::params::ParamValue::Sketch(mut arc)) = f.params.get("sketch").cloned() else { panic!("a sketch") };
                    let d = std::sync::Arc::make_mut(&mut arc);
                    let l = d.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
                    d.sketch.add_constraint(noshape_sketch::ConstraintKind::Horizontal { a: l, b: None }).unwrap();
                    line = Some(l);
                    f.params.set("sketch", noshape_model::params::ParamValue::Sketch(arc));
                })
            });
            (id, line.unwrap())
        })
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let ends = |sk: &noshape_sketch::Sketch| match sk.curve(line) {
        Some(noshape_sketch::Curve2::Line { a, b }) => (a, b),
        _ => panic!("the line"),
    };
    let stored = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.studio(cx).unwrap().features.iter().find(|f| f.id == id).and_then(|f| match f.params.get("sketch") {
            Some(noshape_model::params::ParamValue::Sketch(d)) => Some(ends(&d.sketch)),
            _ => None,
        });
        d.unwrap()
    });
    assert!((stored.1.y - stored.0.y).abs() > 1e-3, "stored slanted: {stored:?}");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).sketch.as_ref().is_some_and(|ed| ed.draft_solved));
    let (draft, solved) = cx.update(|cx| {
        let v = view.read(cx);
        let solved = v.context.as_ref().and_then(|c| c.sketches.iter().find(|s| s.feature == id)).map(|s| ends(&s.sketch)).unwrap();
        (ends(&v.sketch_data(cx).unwrap().sketch), solved)
    });
    assert!((solved.1.y - solved.0.y).abs() < 1e-9, "the solve levels it: {solved:?}");
    assert!(draft.0.distance(solved.0) < 1e-12 && draft.1.distance(solved.1) < 1e-12, "the draft is the solve: {draft:?} vs {solved:?}");
    assert_eq!(cx.update(|cx| view.read(cx).draft_history_state()), (false, false), "not an undo step");
}

/// User P0 (lead 11-09, help "Custom features"): a feature clicked in Add custom features joins the toolbar with
/// live's toast and shows as added; clicked again it comes off (no second entry); a Feature Studio's row adds all its
/// features; the list is alphabetical and every open Part Studio follows it (an account setting).
#[gpui::test]
fn add_custom_features_toggles_adds_all_and_sorts(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "custom-toggle");
    let source = r#"FeatureScript 3044;
import(path : "onshape/std/common.fs", version : "3044.0");
annotation { "Feature Type Name" : "Zeta probe" }
export const zetaProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
    });
annotation { "Feature Type Name" : "alpha probe" }
export const alphaProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
    });
"#;
    let (zeta, alpha) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut fs = None;
            v.doc.update(cx, |doc, cx| {
                doc.edit_document("Add a Feature Studio", cx, |d| fs = Some(d.add_tab("Probes", noshape_model::TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(source)))));
                let _ = doc.custom_features();
            });
            let fs = fs.expect("the studio");
            (noshape_model::custom::type_id(fs, "zetaProbe"), noshape_model::custom::type_id(fs, "alphaProbe"))
        })
    });
    let mine = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).ui.toolbar.custom_features.iter().map(|f| f.name.clone()).collect::<Vec<_>>());
    cx.update(|cx| view.update(cx, |v, _| v.ui.toolbar.custom_features.clear()));
    // A click adds it, with live's toast (fs-ps-acf-click-adds).
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_my_feature(&zeta, Some("version V1".into()), cx)));
    assert_eq!(mine(cx), ["Zeta probe"]);
    let toast = cx.update(|cx| view.read(cx).message.clone().map(|(_, m)| m));
    assert_eq!(toast.as_deref(), Some("Custom feature \"Zeta probe\" was added to your toolbar from version V1."));
    // Again: off the toolbar, its toast gone; not a second entry.
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_my_feature(&zeta, Some("version V1".into()), cx)));
    assert!(mine(cx).is_empty(), "the second click takes it off");
    assert!(cx.update(|cx| view.read(cx).message.is_none()));
    // The studio's row: both, alphabetical (help: "Onshape automatically lists custom features alphabetically").
    cx.update(|cx| view.update(cx, |v, cx| v.add_all_my_features(&[zeta.clone(), alpha.clone()], None, cx)));
    assert_eq!(mine(cx), ["alpha probe", "Zeta probe"]);
    // All on: the row takes them all off.
    cx.update(|cx| view.update(cx, |v, cx| v.add_all_my_features(&[zeta.clone(), alpha.clone()], None, cx)));
    assert!(mine(cx).is_empty());
    // Account-wide: another Part Studio view reloads the saved list when it draws.
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_my_feature(&alpha, None, cx)));
    cx.update(|cx| view.update(cx, |v, cx| {
        v.ui.my_custom_rev = 0;
        v.ui.toolbar.custom_features.clear();
        v.reload_my_features_if_changed(cx);
    }));
    assert_eq!(mine(cx), ["alpha probe"], "the saved list, reloaded");
    // The panel shows it as added.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_add_custom_features(w, cx))).unwrap();
    let added = cx.update(|cx| view.read(cx).ui.add_features.as_ref().map(|a| a.panel.read(cx).added().contains(&alpha)));
    assert_eq!(added, Some(true));
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_my_feature(&alpha, None, cx)));
}

/// Help "Custom features": a custom feature in the Feature list that isn't on the user's toolbar (removed, or a
/// shared document's) is offered by "Linked custom features in this Part Studio" ▾; on the toolbar, it isn't.
#[gpui::test]
fn linked_custom_features_offer_a_used_feature_not_on_the_toolbar(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "custom-linked");
    noshape_model::custom::set_external_tab_loader(noshape_ops::derived::external_tab);
    // Another document with the feature, at its V1.
    let dir = std::env::temp_dir().join(format!("noshape-ui-custom-linked-src-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join("probes.noshape");
    let _ = std::fs::remove_file(&src_path);
    let source = "FeatureScript 3044;\nimport(path : \"onshape/std/common.fs\", version : \"3044.0\");\nannotation { \"Feature Type Name\" : \"Linked probe\" }\nexport const linkedProbe = defineFeature(function(context is Context, id is Id, definition is map)\n    precondition\n    {\n    }\n    {\n    });\n";
    let mut doc = noshape_model::Document::new();
    let fs = doc.add_tab("Probes", noshape_model::TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(source)));
    let mut session = noshape_ops::session::Session::create(&src_path, doc).unwrap();
    let mv = session.head().unwrap();
    session.create_version("V1", "").unwrap();
    drop(session);
    let key = noshape_ops::derived::ExternalRef { path: src_path.canonicalize().unwrap(), microversion: mv, tab: fs.to_string() }.key();
    assert!(noshape_model::custom::external_studio(&key).is_some(), "the other document's studio compiles");
    let type_id = noshape_model::custom::studio_type_id(&key, "linkedProbe");
    // Used in this Part Studio, not on the toolbar.
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.ui.toolbar.custom_features.clear();
            let (tab, units) = (v.tab, v.doc.read(cx).session.doc.settings.units);
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Insert the linked probe", cx, |ps| {
                    let f = ps.new_feature(&type_id, &units).expect("the linked feature's type");
                    ps.features.push(f);
                });
            });
        })
    });
    draw(cx, window);
    let linked = cx.update(|cx| view.update(cx, |v, cx| v.linked_custom_tools(cx).is_some()));
    assert!(linked, "offered while not on the toolbar");
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_my_feature(&type_id, None, cx)));
    let linked = cx.update(|cx| view.update(cx, |v, cx| v.linked_custom_tools(cx).is_some()));
    assert!(!linked, "on the toolbar: not linked any more");
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_my_feature(&type_id, None, cx)));
}

/// Delete part's preview outlines the part it removes (live 10-27), computed off the UI thread and kept: a refresh
/// (a hover) reuses it instead of computing it again (20-28 ms each on c2-bottle-split's Split 1).
#[gpui::test]
fn a_removed_parts_outline_is_computed_once_off_the_ui_thread(cx: &mut TestAppContext) {
    let (window, view, _top, _) = block_part(cx, "removed-outline");
    let body = cx.update(|cx| view.read(cx).display.bodies.first().map(|b| b.id.clone()).expect("the block"));
    view.update(cx, |v, _| v.selection = vec![Reference::Body(body.clone())]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("deleteBodies", w, cx))).unwrap();
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        v.pending.is_none() && v.ui.preview_before.is_some() && v.display.body(&body).is_none()
    });
    wait_until(cx, window, |cx| view.read(cx).ui.removed_outline.as_ref().is_some_and(|(_, o)| o.is_some()));
    let key = cx.update(|cx| view.read(cx).ui.removed_outline.as_ref().map(|(k, _)| k.clone()));
    // A refresh keeps it: no second computation (the slot doesn't go back to computing).
    cx.update(|cx| view.update(cx, |v, cx| v.refresh_scene(false, cx)));
    let after = cx.update(|cx| view.read(cx).ui.removed_outline.as_ref().map(|(k, o)| (k.clone(), o.is_some())));
    assert_eq!(after, key.map(|k| (k, true)));
}

/// User P0 (lead 11-10): selecting two big imported parts froze the app for 51 s while the readout measured them on
/// the UI thread. Whole parts aren't measured (live 2463), and the rest is a background task keyed by the selection:
/// the frame that shows the selection draws without it (no readout yet), and the readout, the Measure panel and its
/// lines take it when it arrives. Two many-faced prisms stand in for the user's parts.
#[gpui::test]
fn a_selections_measurement_doesnt_hold_up_the_frame(cx: &mut TestAppContext) {
    // A 96-sided star prism and a copy 50 mm along X: many faces each.
    let star = |cx0: f64| -> Vec<serde_json::Value> {
        let pts: Vec<[f64; 2]> = (0..96).map(|i| {
            let a = i as f64 * std::f64::consts::TAU / 96.0;
            let r = if i % 2 == 0 { 25.0 } else { 21.0 };
            [cx0 + r * a.cos(), r * a.sin()]
        }).collect();
        (0..96).map(|i| serde_json::json!({ "line": [pts[i], pts[(i + 1) % 96]] })).collect()
    };
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": star(0.0) },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "30 mm", "operationType": "NEW" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": star(50.0) },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "30 mm", "operationType": "NEW" } }
    ] });
    let (window, view) = part_studio_from(cx, "measure-off-thread", Some(start));
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count() == 2);
    let bodies: Vec<Reference> = cx.update(|cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone())).collect());
    assert_eq!(bodies.len(), 2, "the two prisms");
    // Two whole parts: nothing to measure, no task (live 2463).
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection = bodies.clone();
        cx.notify();
    }));
    draw(cx, window);
    cx.update(|cx| assert!(view.read(cx).measure_cache.is_none(), "whole parts aren't measured"));
    // A face of each: measured off the UI thread.
    let parts: Vec<Reference> = cx.update(|cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| b.face_refs[0].clone()).collect());
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection = parts.clone();
        cx.notify();
    }));
    // One frame, the background threads held: it draws with the measurement pending, not computed in it.
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.update(|cx| {
        let c = view.read(cx).measure_cache.as_ref().expect("the measurement started by the frame");
        assert!(c.selection == parts && c.results.is_none(), "measured on the UI thread");
    });
    // The task runs; the readout takes its result.
    draw(cx, window);
    cx.update(|cx| {
        let c = view.read(cx).measure_cache.as_ref().unwrap();
        let r = c.results.as_ref().expect("the measurement arrived");
        assert!(r.iter().any(|(l, _)| l == "Min dist"), "two parts' distance rows: {:?}", r.iter().map(|(l, _)| l).collect::<Vec<_>>());
    });
    // Another selection drops the old one's result; its own comes on.
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection = vec![parts[0].clone()];
        cx.notify();
    }));
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.update(|cx| assert!(view.read(cx).measure_cache.as_ref().is_some_and(|c| c.selection.len() == 1 && c.results.is_none())));
    draw(cx, window);
    cx.update(|cx| assert!(view.read(cx).measure_cache.as_ref().is_some_and(|c| c.selection.len() == 1 && c.results.is_some())));
}

/// Mass properties (lead 11-10, after the measure P0): the parts' mass properties are worked out off the UI thread,
/// once per parts and model, not in every frame of the dialog: the frame that opens it draws with them pending.
#[gpui::test]
fn mass_properties_dont_hold_up_the_frame(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "mass-off-thread");
    let part = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone()))).expect("the part");
    cx.update(|cx| view.update(cx, |v, cx| {
        v.selection = vec![part.clone()];
        v.show_mass = true;
        cx.notify();
    }));
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.update(|cx| assert!(view.read(cx).ui.mass_job.as_ref().is_some_and(|j| j.value.is_none()), "worked out in the frame"));
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        let mp = v.ui.mass_job.as_ref().and_then(|j| j.value.clone()).and_then(|(mp, _)| mp).expect("the mass properties arrived");
        assert!(mp.volume > 0.0, "{mp:?}");
    });
    // Another frame doesn't start it again.
    let before = cx.update(|cx| view.read(cx).ui.mass_job.as_ref().map(|j| j.key.clone()));
    draw(cx, window);
    cx.update(|cx| assert!(view.read(cx).ui.mass_job.as_ref().is_some_and(|j| Some(j.key.clone()) == before && j.value.is_some())));
}

/// Show center of mass (lead 11-10, the synchronous calls list): worked out off the UI thread, the marker comes when it
/// is known; a second click while it is under way takes it back.
#[gpui::test]
fn center_of_mass_is_worked_out_off_the_click(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "com-off-thread");
    let body = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| b.id.clone())).expect("the part");
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_center_of_mass(&body, cx)));
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.com.is_none() && v.ui.com_task.is_some(), "worked out on the click");
    });
    draw(cx, window);
    cx.update(|cx| assert!(view.read(cx).ui.com.as_ref().is_some_and(|(b, _)| *b == body), "the marker"));
    cx.update(|cx| view.update(cx, |v, cx| v.toggle_center_of_mass(&body, cx)));
    cx.update(|cx| assert!(view.read(cx).ui.com.is_none() && view.read(cx).ui.com_task.is_none(), "hidden"));
    // Shown and hidden again before it arrives: nothing shows.
    cx.update(|cx| view.update(cx, |v, cx| {
        v.toggle_center_of_mass(&body, cx);
        v.toggle_center_of_mass(&body, cx);
    }));
    draw(cx, window);
    cx.update(|cx| assert!(view.read(cx).ui.com.is_none(), "taken back"));
}

/// The curvature map (lead 11-10): its colours over every mesh are worked out off the UI thread; the plain shading
/// shows until they arrive, then the scene takes them.
#[gpui::test]
fn the_curvature_map_is_worked_out_off_the_frame(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "curvature-off-thread");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_analysis(crate::part_studio::analysis::Analysis::Curvature, w, cx))).unwrap();
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.curvature_job.is_some() && v.opts.vertex_colors.is_none(), "plain shading while it is worked out");
    });
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        assert!(v.ui.curvature_job.is_none() && v.opts.vertex_colors.is_some(), "the curvature colours in the scene");
    });
}

/// Live 2468: New sketch… on a face of the flat sketches inside the flat view: the flat view shows the sketch (not
/// the parts), takes the tool's clicks, and the sketch ends on the flat (shown only there).
#[gpui::test]
fn a_sketch_on_the_flat_is_drawn_in_the_flat_view(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    cx.update(|cx| cx.bind_keys(super::entry_key_bindings()));
    crate::part_studio_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-ui-flat-sketch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    {
        use serde_json::json;
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [60, 40]] }));
        call(&mut ctx, "feature_add", json!({ "type": "sheetMetalStart", "params": { "process": "THICKEN", "regions": [r["regions"][0]["id"]], "thickness": "2 mm", "radius": "2 mm" } }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        // As every app window: its menus (a list row's right-click) draw in it.
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    wait_until(cx, window, |cx| view.read(cx).context.as_ref().is_some_and(|c| !c.sheet_metal.is_empty()));
    // The Sheet metal table and flat view, its flat built.
    cx.update(|cx| view.update(cx, |v, cx| {
        v.right_panel = Some(crate::part_studio::config_panel::RightPanel::SheetMetal);
        cx.notify();
    }));
    wait_until(cx, window, |cx| view.read(cx).flat_view.is_some() && view.read(cx).flat_cache.as_ref().is_some_and(|c| c.parts.iter().any(|p| p.1.is_ok())));
    // The flat's top face (its largest face facing +Z).
    let (part, top) = cx.update(|cx| {
        let v = view.read(cx);
        let ctx = v.context.as_ref().unwrap();
        let part = ctx.bodies.iter().find(|b| ctx.sheet_metal.contains_key(&b.id)).unwrap().id.clone();
        let solid = noshape_model::sheetmetal::flat_pattern_for(ctx, &part).unwrap().flat_view_solid().unwrap();
        let top = solid.faces().iter().enumerate().filter(|(_, f)| f.face_plane().ok().flatten().is_some_and(|p| p.unit_normal().z > 0.999)).max_by(|a, b| a.1.area().unwrap_or(0.0).total_cmp(&b.1.area().unwrap_or(0.0))).unwrap().0;
        // It shows the plate's top face, as seen from Top (Flip direction up off: the model's up is +Z; qa
        // session-sm-1119, live's flat view): the face the main view lights for it.
        let shown = noshape_model::sheetmetal::flat_face_counterpart(ctx, &noshape_model::sheetmetal::flat_face_name(&part, top)).expect("the part's face");
        let Reference::Face(n) = &shown else { panic!("{shown:?}") };
        let body = ctx.body(&part).unwrap();
        let fi = *body.find_faces(n).first().expect("on the part");
        let pl = body.shape.faces()[fi].face_plane().unwrap().unwrap();
        assert!(pl.unit_normal().z > 0.999 && (pl.origin.z - 2e-3).abs() < 1e-9, "{pl:?}");
        (part, top)
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![Reference::Face(noshape_model::sheetmetal::flat_face_name(&part, top))];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some());
    let fvp = cx.update(|cx| view.read(cx).flat_sketch_vp(cx)).expect("sketching in the flat view");
    assert!(cx.update(|cx| view.read(cx).flat_view.as_ref().unwrap().read(cx).sketching()), "the flat view shows the sketch");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    draw(cx, window);
    // Two clicks in the flat view at the flat's (10, 10) and (40, 30) mm: a line there.
    let at = |cx: &mut TestAppContext, x: f64, y: f64| cx.update(|cx| fvp.read(cx).project(glam::DVec3::new(x * 1e-3, y * 1e-3, 0.0))).expect("on screen");
    for (x, y) in [(10.0, 10.0), (40.0, 30.0)] {
        let p = at(cx, x, y);
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.active_vp = Some(fvp.clone());
                for ev in [
                    ViewportEvent::PointerMove { position: p, modifiers: Modifiers::default() },
                    ViewportEvent::ToolDown { position: p, modifiers: Modifiers::default(), click_count: 1 },
                    ViewportEvent::ToolUp { position: p, modifiers: Modifiers::default() },
                ] {
                    v.on_viewport_event(&fvp, &ev, w, cx);
                }
                v.active_vp = None;
            });
        })
        .unwrap();
        draw(cx, window);
    }
    let data = cx.update(|cx| view.read(cx).sketch_data(cx)).expect("the sketch");
    let line = data.sketch.entities().filter_map(|e| match data.sketch.curve(e.id) {
        Some(noshape_sketch::Curve2::Line { a, b }) => Some((a, b)),
        _ => None,
    }).next().expect("a line");
    let near = |p: DVec2, q: (f64, f64)| (p - DVec2::new(q.0 * 1e-3, q.1 * 1e-3)).length() < 0.5e-3;
    let frame = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().frame);
    let w = |p: DVec2| { let q = frame.to_world(p); DVec2::new(q.x, q.y) };
    assert!(near(w(line.0), (10.0, 10.0)) && near(w(line.1), (40.0, 30.0)), "{:?} {:?}", w(line.0), w(line.1));
}

/// A flat sketch's region preselected, Extrude opens as the flat's (live q-flat-ops-1119): "Extrude 1" with only
/// Add | Remove and the region (no depth, end type or creation type).
#[gpui::test]
fn extrude_on_a_flat_region_opens_the_flat_form(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    crate::part_studio_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-ui-flat-extrude-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    {
        use serde_json::json;
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [60, 40]] }));
        call(&mut ctx, "feature_add", json!({ "type": "sheetMetalStart", "params": { "process": "THICKEN", "regions": [r["regions"][0]["id"]], "thickness": "2 mm", "radius": "2 mm" } }));
        let faces = call(&mut ctx, "sheetmetal_table", json!({}))["flat_faces"].as_array().unwrap().clone();
        let num = |v: &serde_json::Value| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64()).unwrap_or(f64::NAN);
        let top = faces.iter().filter(|f| num(&f["normal"][2]) > 0.999).max_by(|a, b| num(&a["area"]).total_cmp(&num(&b["area"]))).unwrap().clone();
        let s2 = call(&mut ctx, "sketch_create", json!({ "plane": top["id"] }))["sketch"].clone();
        call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "corner_rectangle", "points": [[-5, -5], [5, 5]] }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    wait_until(cx, window, |cx| view.read(cx).context.as_ref().is_some_and(|c| !c.flat_sketches.is_empty()));
    let region = cx.update(|cx| {
        let v = view.read(cx);
        let sketch = *v.context.as_ref().unwrap().flat_sketches.keys().next().unwrap();
        let sk = v.display.sketches.iter().find(|s| s.feature == sketch).expect("the flat sketch shown");
        Reference::SketchRegion { sketch, key: sk.regions[0].key.clone() }
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![region.clone()];
            v.start_feature("extrude", w, cx);
        });
    })
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).edit.is_some());
    let (domain, shown) = cx.update(|cx| {
        let d = view.read(cx).edit.as_ref().unwrap().dialog.read(cx);
        let p = &d.feature.params;
        (p.enum_("domain").to_string(), d.ty.params.iter().filter(|s| s.is_visible(p)).map(|s| s.id).collect::<Vec<_>>())
    });
    assert_eq!((domain.as_str(), shown), ("FLAT", vec!["flatOperationType", "entities"]));
}

/// Every kind of plane right-clicked in the graphics, unselected and selected: Top, Front and Right and a Plane
/// feature's. The menu is built while the view is being updated: the default planes' read the view for their selection
/// there and crashed the app (v0.4.46, 2347771e). Selected, it adds Clear selection (live PLANE-MENU-SELECTED). A default
/// plane's is the viewport's, not its row's (live ctx-menus-1117 plane-top-graphics, qa MENU-PLANE-GRAPHICS).
#[gpui::test]
fn every_kind_of_plane_right_clicked_in_the_graphics_opens_its_menu(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "plane-graphics-menu");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Add plane", cx, |ps| {
                    let mut f = ps.new_feature("cPlane", &noshape_units::UnitPrefs::default()).expect("Plane");
                    f.params.set("entities", noshape_model::ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
                    f.params.set("offset", noshape_model::ParamValue::Expr(noshape_units::ExprValue::new("10 mm")));
                    ps.features.push(f);
                })
            });
        })
    });
    wait_until(cx, window, |cx| view.read(cx).display.planes.len() == 4);
    let feature_plane = cx.update(|cx| view.read(cx).display.planes.iter().map(|p| p.reference.clone()).find(|r| matches!(r, Reference::FeaturePlane(_)))).expect("Plane 1");
    let menu = |cx: &mut TestAppContext, r: &Reference| {
        let t = noshape_scene::builder::PickTarget { reference: r.clone(), kind: noshape_scene::builder::PickKind::Plane, anchor: None };
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_context_menu(Some(t), gpui_kit::point(px(600.), px(400.)), w, cx))).unwrap();
        draw(cx, window);
        cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default())
    };
    let planes = [Reference::Plane(DatumPlane::Top), Reference::Plane(DatumPlane::Front), Reference::Plane(DatumPlane::Right), feature_plane];
    for r in &planes {
        for selected in [false, true] {
            cx.update_window(window, |_, _, cx| view.update(cx, |v, _| v.selection = if selected { vec![r.clone()] } else { Vec::new() })).unwrap();
            let rows = menu(cx, r);
            assert!(rows.iter().any(|l| l == "New sketch…") && rows.iter().any(|l| l == "Offset plane…"), "{r:?} selected {selected}: {rows:?}");
            if let Reference::Plane(p) = r {
                assert_eq!(rows.iter().any(|l| l == "Clear selection"), selected, "{r:?} selected {selected}: {rows:?}");
                let name = format!("{p:?}");
                let mut want = vec!["New sketch…", "Offset plane…", "Size to Part Studio", "———", "Hide {name} plane", "Hide other planes", "Hide all planes", "Section view…", "———"];
                if selected {
                    want.push("Clear selection");
                }
                want.extend(["Select", "Select other…", "———", "Zoom to fit", "Zoom to selection", "View normal to"]);
                let want: Vec<String> = want.iter().map(|l| l.replace("{name}", &name)).collect();
                assert_eq!(rows, want, "{r:?} selected {selected}");
            }
        }
    }
}

#[path = "feel_tests.rs"]
mod feel;
#[path = "hover_matrix_tests.rs"]
mod hover_matrix;
#[path = "snap_matrix_tests.rs"]
mod snaps;
#[path = "list_feel_tests.rs"]
mod list;
#[path = "measure_feel_tests.rs"]
mod measure;
#[path = "nav_feel_tests.rs"]
mod nav;
#[path = "chrome_feel_tests.rs"]
mod chrome;
#[path = "toolbar_highlight_tests.rs"]
mod toolbar_highlight;
#[path = "sm_feel_tests.rs"]
mod sm;
#[path = "sketchtools_feel_tests.rs"]
mod sketchtools;
#[path = "editing_feel_tests.rs"]
mod editing;
#[path = "dialogs_feel_tests.rs"]
mod dialogs;
#[path = "latency_tests.rs"]
mod latency;
#[path = "fields_feel_tests.rs"]
mod fields;
#[path = "feedback_feel_tests.rs"]
mod feedback;
#[path = "sketch_hover_feel_tests.rs"]
mod sketch_hover;
#[path = "sketch_label_feel_tests.rs"]
mod sketch_label;
#[path = "sketch_glyph_feel_tests.rs"]
mod sketch_glyph;
#[path = "sketch_box_feel_tests.rs"]
mod sketch_box;
#[path = "sketch_gesture_feel_tests.rs"]
mod sketch_gesture;
#[path = "export_feel_tests.rs"]
mod export_feel;

/// Qa SKT-USE-HOVER (live sketch-tools-1123 use-convert, sk23 use-0-hover-vertex): seen from above, the Use tool on a
/// top corner of the block offers an edge of the top face running across the screen, never the upright edge seen end on
/// (it projects to a point: nothing lit).
#[gpui::test]
fn use_on_a_corner_from_above_offers_an_edge_across_the_screen(cx: &mut TestAppContext) {
    let (window, view, _, _, a, _) = sketch_on_top_face(cx, "use-corner-edge", Tool::Use);
    cx.update(|cx| view.update(cx, |v, cx| v.viewport.update(cx, |vp, cx| {
        vp.camera.set_view(noshape_render::StandardView::Top);
        vp.request_frame(cx);
    })));
    draw(cx, window);
    let (edge, span) = cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        let corner = vp.project(v.sketch.as_ref().unwrap().frame.to_world(a)).expect("the corner on screen");
        let edge = v.model_edge_at(corner, cx).expect("an edge offered at the corner");
        let body = v.display.bodies.iter().find(|b| b.edge_refs.contains(&edge)).unwrap();
        let pl = &body.mesh.edges[body.edge_refs.iter().position(|e| *e == edge).unwrap()];
        let ends = [pl[0], *pl.last().unwrap()].map(|p| vp.project(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).unwrap());
        (edge, (ends[1] - ends[0]).abs())
    });
    assert!(span.max_element() > 20.0 && span.x >= span.y, "{edge:?}: on screen {span:?}, an edge running across it (live: the edge along x)");
}

/// A sketch on the block's top face, with `tool`; and one of that face's edges: its reference, its pick id and its two
/// ends in the sketch plane.
fn sketch_on_top_face(cx: &mut TestAppContext, name: &str, tool: Tool) -> (AnyWindowHandle, Entity<PartStudioView>, Reference, u32, DVec2, DVec2) {
    let (window, view, top, _) = block_part(cx, name);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![top.clone()];
            v.start_feature("sketch", w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(tool, w, cx))).unwrap();
    draw(cx, window);
    let (edge, pick, a, b) = cx.update(|cx| {
        let v = view.read(cx);
        let frame = v.sketch.as_ref().unwrap().frame;
        let body = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let top_z = body.mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        let (r, pl) = body.edge_refs.iter().zip(&body.mesh.edges).filter(|(_, pl)| pl.iter().all(|p| (p[2] - top_z).abs() < 1e-7)).max_by(|x, y| {
            let len = |pl: &Vec<[f32; 3]>| (glam::Vec3::from(pl[0]) - glam::Vec3::from(*pl.last().unwrap())).length();
            len(x.1).total_cmp(&len(y.1))
        }).unwrap();
        let local = |p: &[f32; 3]| frame.to_local(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
        let pick = v.built.as_ref().unwrap().picks.ids_for(r).first().copied().expect("the edge is pickable");
        (r.clone(), pick, local(&pl[0]), local(pl.last().unwrap()))
    });
    (window, view, edge, pick, a, b)
}

/// The block's centre on the sketch plane (its first corner sits on the origin, so the origin can't tell an edge's inside).
fn block_centre(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> DVec2 {
    cx.update(|cx| {
        let v = view.read(cx);
        let body = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let sum = body.mesh.positions.iter().fold(glam::DVec3::ZERO, |s, p| s + glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
        v.sketch.as_ref().unwrap().frame.to_local(sum / body.mesh.positions.len() as f64)
    })
}

/// A sketch-plane point on screen (viewport-local).
fn on_screen(cx: &mut TestAppContext, view: &Entity<PartStudioView>, q: DVec2) -> DVec2 {
    cx.update(|cx| {
        let v = view.read(cx);
        v.viewport.read(cx).project(v.sketch.as_ref().unwrap().frame.to_world(q)).unwrap()
    })
}

/// User P0 11-13 (hat15, Sketch 1 on a face): a line from the end of a short segment toward the face's model edge, the
/// edge lit and the perpendicular glyph shown: the click left the end short of the edge (the perpendicular beat the
/// edge). The end lands on the edge, coincident with it (its used curve), and keeps the direction inferred (live2: a
/// direction and on-a-line both).
#[gpui::test]
fn a_line_ending_on_a_lit_model_edge_lands_on_it(cx: &mut TestAppContext) {
    let (window, view, _edge, pick, a, b) = sketch_on_top_face(cx, "line-onto-model-edge", Tool::Line);
    let along = (b - a).normalize();
    let inward = {
        let centre = block_centre(cx, &view);
        let n = along.perp();
        if (centre - a).dot(n) > 0.0 { n } else { -n }
    };
    let wpp = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().world_per_px);
    // (30 px along from the edge's midpoint, which would take the end as a point.)
    let mid = (a + b) * 0.5 + along * 30.0 * wpp;
    // A short segment parallel to the edge, 40 px inside it.
    let (s1, s0) = (mid + inward * 40.0 * wpp - along * 30.0 * wpp, mid + inward * 40.0 * wpp);
    let (p1, p0) = (on_screen(cx, &view, s1), on_screen(cx, &view, s0));
    click(cx, window, &view, (p1.x, p1.y));
    click(cx, window, &view, (p0.x, p0.y));
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    draw(cx, window);
    // From its end straight toward the edge, the pointer 2 px past it on the edge's far side and 2 px along it.
    let target = on_screen(cx, &view, s0 - inward * 42.0 * wpp + along * 2.0 * wpp);
    click(cx, window, &view, (p0.x, p0.y));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            vp.update(cx, |vp, _| vp.place_pointer(target));
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: target, modifiers: Modifiers::default() }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::Hover(Some(pick)), w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: target, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: target, modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    let (off, on_edge, perpendicular) = cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).unwrap();
        let line = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.external && !e.construction).map(|e| e.id).max().unwrap();
        let (_, end) = data.sketch.endpoints(line).unwrap();
        let q = data.sketch.point(end).unwrap();
        let off = (q - a).perp_dot(along).abs();
        let on_edge = data.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Coincident { a: x, b: y } if (x == end && data.sketch.entity(y).is_some_and(|e| e.external)) || (y == end && data.sketch.entity(x).is_some_and(|e| e.external))));
        // (The block's edges lie along the sketch's axes: the direction inferred there is Horizontal / Vertical.)
        let perpendicular = data.sketch.constraints().any(|c| match c.kind {
            noshape_sketch::ConstraintKind::Perpendicular { a: x, b: y } => x == line || y == line,
            noshape_sketch::ConstraintKind::Horizontal { a: x, b: None } | noshape_sketch::ConstraintKind::Vertical { a: x, b: None } => x == line,
            _ => false,
        });
        (off, on_edge, perpendicular)
    });
    // (Against the display mesh's f32 edge.)
    assert!(off < 1e-6, "the end is on the edge: {off} m off");
    assert!(on_edge, "coincident with the edge's used curve");
    assert!(perpendicular, "the direction inferred kept");
}

/// Live workflow 05 step 11 and lenbox-1122 E (behaviours "skasm length box side"): a line drawn level toward a lit
/// model edge, the pointer on the edge's midpoint 2 px off the level: live shows horizontal and vertical (no diamond) and
/// ends the line on the level straight across from the midpoint, HORIZONTAL and VERTICAL to the edge's midpoint, not
/// at the midpoint.
#[gpui::test]
fn a_level_line_onto_a_model_edges_midpoint_ends_where_the_level_meets_it(cx: &mut TestAppContext) {
    let (window, view, _edge, pick, a, b) = sketch_on_top_face(cx, "level-onto-edge-midpoint", Tool::Line);
    let along = (b - a).normalize();
    let inward = {
        let centre = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().frame.to_local(glam::DVec3::ZERO));
        let n = along.perp();
        if (centre - a).dot(n) > 0.0 { n } else { -n }
    };
    let wpp = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().world_per_px);
    let mid = (a + b) * 0.5;
    let s0 = mid + inward * 60.0 * wpp + along * 2.0 * wpp;
    let (p0, target) = (on_screen(cx, &view, s0), on_screen(cx, &view, mid));
    click(cx, window, &view, (p0.x, p0.y));
    let icons = cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            vp.update(cx, |vp, _| vp.place_pointer(target));
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: target, modifiers: Modifiers::default() }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::Hover(Some(pick)), w, cx);
            v.sketch.as_ref().unwrap().snap_icons()
        })
    })
    .unwrap();
    assert!(icons.len() == 2 && icons.contains(&"horizontal") && icons.contains(&"vertical"), "the level and the midpoint's alignment: {icons:?}");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: target, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: target, modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    let (start, end, midpoint, aligned) = cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).unwrap();
        let line = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.external && !e.construction).map(|e| e.id).max().unwrap();
        let (start, end) = data.sketch.endpoints(line).unwrap();
        let midpoint = data.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Midpoint { point, .. } if point == end));
        let mids: Vec<_> = data.sketch.constraints().filter_map(|c| match c.kind { noshape_sketch::ConstraintKind::Midpoint { point, curve } if data.sketch.entity(curve).is_some_and(|e| e.external) => Some(point), _ => None }).collect();
        let aligned = data.sketch.constraints().any(|c| match c.kind {
            noshape_sketch::ConstraintKind::Vertical { a, b: Some(b) } | noshape_sketch::ConstraintKind::Horizontal { a, b: Some(b) } => (a == end && mids.contains(&b)) || (b == end && mids.contains(&a)),
            _ => false,
        });
        (data.sketch.point(start).unwrap(), data.sketch.point(end).unwrap(), midpoint, aligned)
    });
    // (Against the display mesh's f32 edge; the start where its click landed.)
    assert!((end - a).perp_dot(along).abs() < 1e-6 && (end - start).dot(along).abs() < 1e-9, "where the level meets the edge: {start:?} -> {end:?}");
    assert!(!midpoint && (end - mid).length() > wpp, "not the edge's midpoint: {end:?}, midpoint {mid:?}");
    assert!(aligned, "aligned with a point at the edge's midpoint");
}

/// User P0 11-14 (hat15, a chain on a face): the next segment square to the previous one, ending where that square
/// crosses the lit model edge, takes both (Perpendicular to the previous segment, coincident with the edge's used
/// curve) and shows both glyphs by the pointer, the direction's first (live: a compound shows each glyph). The
/// previous segment at 30 deg to the edge, so no horizontal or vertical competes.
#[gpui::test]
fn a_square_to_the_last_segment_crossing_a_lit_model_edge_takes_and_shows_both(cx: &mut TestAppContext) {
    let (window, view, _edge, pick, a, b) = sketch_on_top_face(cx, "square-onto-model-edge", Tool::Line);
    let along = (b - a).normalize();
    let inward = {
        let centre = block_centre(cx, &view);
        let n = along.perp();
        if (centre - a).dot(n) > 0.0 { n } else { -n }
    };
    let wpp = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().world_per_px);
    let (c30, s30) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let u = along * c30 + inward * s30;
    let s0 = (a + b) * 0.5 + along * 40.0 * wpp + inward * 50.0 * wpp;
    let s1 = s0 - u * 60.0 * wpp;
    // Square to it, toward the edge, from its end: where it crosses the edge.
    let sq = if u.perp().dot(inward) < 0.0 { u.perp() } else { -u.perp() };
    let crossing = s0 + sq * ((a - s0).dot(inward) / sq.dot(inward));
    let (p1, p0) = (on_screen(cx, &view, s1), on_screen(cx, &view, s0));
    click(cx, window, &view, (p1.x, p1.y));
    click(cx, window, &view, (p0.x, p0.y));
    // 2 px past the crossing along the edge and 2 px beyond it.
    let target = on_screen(cx, &view, crossing + along * 2.0 * wpp - inward * 2.0 * wpp);
    let first = cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).unwrap();
        data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.external && !e.construction).map(|e| e.id).max().unwrap()
    });
    let icons = cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            vp.update(cx, |vp, _| vp.place_pointer(target));
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: target, modifiers: Modifiers::default() }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::Hover(Some(pick)), w, cx);
            v.sketch.as_ref().unwrap().snap_icons()
        })
    })
    .unwrap();
    assert_eq!(icons, vec!["perpendicular", "coincident"], "both glyphs by the pointer");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: target, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
            v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: target, modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    let (off, on_edge, square) = cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).unwrap();
        let line = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.external && !e.construction).map(|e| e.id).max().unwrap();
        let (_, end) = data.sketch.endpoints(line).unwrap();
        let q = data.sketch.point(end).unwrap();
        let on_edge = data.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Coincident { a: x, b: y } if (x == end && data.sketch.entity(y).is_some_and(|e| e.external)) || (y == end && data.sketch.entity(x).is_some_and(|e| e.external))));
        let square = data.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Perpendicular { a: x, b: y } if (x == line && y == first) || (x == first && y == line)));
        ((q - a).perp_dot(along).abs(), on_edge, square)
    });
    // (Against the display mesh's f32 edge; the solver may nudge the free previous segment to keep both.)
    assert!(off < 1e-6, "the end is on the edge: {off} m off");
    assert!(on_edge, "coincident with the edge's used curve");
    assert!(square, "perpendicular to the previous segment");
}

/// The same user P0: an existing point dragged onto a model edge of the sketch's face, released 3 px off it, lands
/// on it, coincident with its used curve (live2 "feel (sketch)" (2): onto a line from 7 px).
#[gpui::test]
fn a_point_dragged_onto_a_model_edge_lands_on_it(cx: &mut TestAppContext) {
    let (window, view, _edge, _pick, a, b) = sketch_on_top_face(cx, "drag-onto-model-edge", Tool::Point);
    let along = (b - a).normalize();
    let inward = {
        let centre = block_centre(cx, &view);
        let n = along.perp();
        if (centre - a).dot(n) > 0.0 { n } else { -n }
    };
    let wpp = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().world_per_px);
    // (30 px along from the edge's midpoint.)
    let start = (a + b) * 0.5 + along * 30.0 * wpp + inward * 40.0 * wpp;
    let s = on_screen(cx, &view, start);
    click(cx, window, &view, (s.x, s.y));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    draw(cx, window);
    let to = on_screen(cx, &view, start - inward * 37.0 * wpp);
    drag(cx, window, &view, (s.x, s.y), (to.x, to.y), 8);
    let (off, on_edge) = cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).unwrap();
        let pt = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point && !e.external).map(|e| e.id).max().unwrap();
        let q = data.sketch.point(pt).unwrap();
        let on_edge = data.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Coincident { a: x, b: y } if (x == pt && data.sketch.entity(y).is_some_and(|e| e.external)) || (y == pt && data.sketch.entity(x).is_some_and(|e| e.external))));
        ((q - a).perp_dot(along).abs(), on_edge)
    });
    assert!(on_edge, "coincident with the edge's used curve");
    assert!(off < 1e-6, "on the edge: {off} m off");
}

/// Live2 config.json (configureParameter): a field configured in an open dialog (right-click › Configure) adds its
/// column, but the dialog must be committed: Esc takes the configured column away again (a Variable's value here).
#[gpui::test]
fn escape_on_a_dialog_takes_its_configured_field_away(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-esc", &[("width", "30 mm")]);
    view.update(cx, |v, cx| v.add_config_input("List", cx));
    let id = cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "assignVariable")).map(|f| f.id)).expect("the variable");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let configured = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).map(|s| s.configuration.configured.len()).unwrap_or(0));
    view.update(cx, |v, cx| v.configure_dialog_param("lengthValue", true, cx));
    assert_eq!(configured(cx), 1, "Configure adds the column at once");
    let toast = cx.update(|cx| view.read(cx).message.clone().map(|(s, t)| (s == noshape_model::display::FeatureStatus::Info, t)));
    assert!(toast.as_ref().is_some_and(|(info, t)| *info && t.ends_with(" added to Configuration")), "the toast \"<feature> <parameter> added to <input>\" (the first List is \"Configuration\"): {toast:?}");
    // Its × closes it, nothing else changed.
    draw(cx, window);
    let x = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("bubble-x").expect("the toast's ×");
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(x.center(), Modifiers::default());
    draw(cx, window);
    assert_eq!((cx.update(|cx| view.read(cx).message.is_some()), configured(cx)), (false, 1), "(the toast shown, configured) after its ×");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
    draw(cx, window);
    assert_eq!(configured(cx), 0, "Esc takes it away");
    assert!(cx.update(|cx| view.read(cx).studio(cx).is_some_and(|s| s.configuration.inputs.len() == 1)), "the input made before the dialog stays");
    // In Configure features mode (its dialog has only ✕) a click configures for good: Esc keeps it.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    view.update(cx, |v, cx| {
        let dialog = v.edit.as_ref().expect("the dialog").dialog.clone();
        dialog.update(cx, |d, _| d.configure_mode = true);
        v.configure_dialog_param("lengthValue", true, cx);
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
    draw(cx, window);
    assert_eq!(configured(cx), 1, "kept in Configure features mode");
}

/// Qa CONFIG-FILTER-HELP-STILL-POINTER (live workflows-1123 06 s2-s4): the pointer resting on the Configurations panel's
/// filter box shows "Filter inputs" beside it, then the supported filters 400 px wide, centred 3 px under it.
#[gpui::test]
fn the_config_filter_box_shows_its_help_under_a_still_pointer(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-filter-help", &[("width", "30 mm")]);
    view.update(cx, |v, cx| {
        v.edit_config("Configure", cx, |ps| {
            noshape_ops::config::add_input(ps, "List");
        });
        v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
        cx.notify();
    });
    draw(cx, window);
    let at = |cx: &mut TestAppContext, sel: &'static str| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(sel);
    let step = |cx: &mut TestAppContext, ms: u64| {
        for _ in 0..ms / 10 {
            cx.executor().advance_clock(std::time::Duration::from_millis(10));
            draw(cx, window);
        }
    };
    let b = at(cx, "cfg-filter-box").expect("the filter box");
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(gpui_kit::point(px(20.), px(300.)), None, gpui_kit::Modifiers::default());
    step(cx, 1000);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(b.center(), None, gpui_kit::Modifiers::default());
    step(cx, 700);
    let short = at(cx, "rich-tooltip").expect("the Filter inputs tooltip");
    step(cx, 2000);
    let long = at(cx, "rich-tooltip").expect("the supported filters");
    let win = cx.update_window(window, |_, w, _| f32::from(w.viewport_size().width)).unwrap();
    let near = |x: gpui_kit::Pixels, want: f32| (f32::from(x) - want).abs() < 1.;
    let left = (f32::from(b.center().x) - 200.).min(win - 402.).max(2.);
    let row = (
        near(short.left(), f32::from(b.right()) + 3.),
        near(short.center().y, f32::from(b.center().y)),
        near(long.size.width, 400.),
        near(long.left(), left),
        near(long.top(), f32::from(b.bottom()) + 3.),
        f32::from(long.size.height) > 150.,
    );
    assert_eq!(row, (true, true, true, true, true, true), "(short beside the box, centred on it; long 400 px, centred, 3 px under, tall) box {b:?}, short {short:?}, long {long:?}");
}

/// Qa CONFIG-FILTER-HELP-STILL-POINTER (live workflows-1123 06 s2-s4): Configure Part Studio clicked, the pointer left
/// where it was (now on the filter box): the supported filters show and stay while the new input's name is typed and
/// Enter pressed.
#[gpui::test]
fn the_config_filter_help_stays_while_the_new_inputs_name_is_typed(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-filter-help-typing", &[("width", "30 mm")]);
    view.update(cx, |v, cx| {
        v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
        cx.notify();
    });
    draw(cx, window);
    let at = |cx: &mut TestAppContext, sel: &'static str| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(sel);
    let step = |cx: &mut TestAppContext, ms: u64| {
        for _ in 0..ms / 10 {
            cx.executor().advance_clock(std::time::Duration::from_millis(10));
            draw(cx, window);
        }
    };
    let button = at(cx, "cfg-configure").expect("Configure Part Studio").center();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(button, None, gpui_kit::Modifiers::default());
    step(cx, 300);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(button, gpui_kit::Modifiers::default());
    step(cx, 2700);
    let shown = |cx: &mut TestAppContext| at(cx, "rich-tooltip").map(|b| f32::from(b.size.width));
    let after_click = shown(cx);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_input("Large");
    step(cx, 500);
    let typed = shown(cx);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("enter");
    step(cx, 500);
    let entered = shown(cx);
    assert_eq!((after_click, typed, entered), (Some(400.), Some(400.), Some(400.)), "the supported filters after the click, while typing, after Enter");
}

/// Live2 config.json (tableKeys): in the Configurations table, Enter on an edited value commits it and moves the
/// selection down one row (the cell below framed).
#[gpui::test]
fn enter_in_a_configured_value_moves_down_a_row(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-enter", &[("width", "30 mm")]);
    let (rows, input) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let id = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "assignVariable")).map(|f| f.id).expect("the variable");
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                noshape_ops::config::add_option(ps, &input, "A".into());
                noshape_ops::config::configure_parameter(ps, id, "lengthValue", &input);
            });
            v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
            let rows: Vec<String> = match v.studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == input)).map(|i| &i.kind) {
                Some(noshape_model::configuration::ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
                _ => Vec::new(),
            };
            (rows, input)
        })
    });
    assert_eq!(rows.len(), 2, "Default and A in {input}");
    draw(cx, window);
    view.update(cx, |v, cx| {
        v.ui.cfg_editing = Some(format!("cp-0-{}", rows[0]));
        v.ui.cfg_editing_focus = true;
        cx.notify();
    });
    draw(cx, window);
    keys(cx, window, &["4", "0", "enter"]);
    let (value, framed) = cx.update(|cx| {
        let v = view.read(cx);
        let value = v.studio(cx).and_then(|s| s.configuration.configured.first()).and_then(|c| c.values.get(&rows[0]).cloned());
        (value, v.ui.cfg_framed.as_ref().map(|(k, _)| k.clone()))
    });
    assert!(value.map(|p| format!("{p:?}")).is_some_and(|t| t.contains("40 mm")), "committed as 40 mm");
    assert_eq!(framed, Some(format!("cp-0-{}", rows[1])), "the cell below framed");
}

/// Live workflow 14 (qa CONFIG-NAME-FOCUS-BOX): the add row's Name input taking the keys has the blue focus box, and
/// after a name + Enter the next empty Name row has it.
#[gpui::test]
fn the_configuration_add_rows_name_has_the_focus_box(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-name-focus", &[("width", "30 mm")]);
    let input = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| input = noshape_ops::config::add_input(ps, "List"));
            v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
            input
        })
    });
    let selector: &'static str = Box::leak(format!("cfg-new-focus-{input}").into_boxed_str());
    let boxed = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(selector).is_some();
    draw(cx, window);
    let before = boxed(cx);
    view.update(cx, |v, cx| {
        v.ui.cfg_focus_new_row = Some(input.clone());
        cx.notify();
    });
    draw(cx, window);
    let focused = boxed(cx);
    keys(cx, window, &["l", "a", "r", "g", "e", "enter"]);
    draw(cx, window);
    draw(cx, window);
    assert_eq!((before, focused, boxed(cx)), (false, true, true), "(the box: at rest, the add row focused, after a name + Enter)");
}

/// Live2 ps-feel2-1118: a List's rows by keys. A name + Tab in the add row adds the row and frames the first row's
/// name; typing on a framed name replaces it and Return commits and frames the row below; Up / Down move the frame;
/// Down from the last row gives the add row the keyboard.
#[gpui::test]
fn configuration_names_take_spreadsheet_keys(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-names", &[("width", "30 mm")]);
    let input = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                noshape_ops::config::add_option(ps, &input, "A".into());
            });
            v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
            input
        })
    });
    let rows = |cx: &mut TestAppContext| -> Vec<(String, String)> {
        cx.update(|cx| match view.read(cx).studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == input)).map(|i| &i.kind) {
            Some(noshape_model::configuration::ConfigInputKind::List { options, .. }) => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
            _ => Vec::new(),
        })
    };
    let framed = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).ui.cfg_framed.as_ref().map(|(k, _)| k.clone()));
    let name_key = |rk: &str| format!("opt-{input}-{rk}");
    draw(cx, window);
    view.update(cx, |v, cx| {
        v.ui.cfg_focus_new_row = Some(input.clone());
        cx.notify();
    });
    draw(cx, window);
    keys(cx, window, &["b", "tab"]);
    draw(cx, window);
    let after_tab = (rows(cx).iter().map(|r| r.1.clone()).collect::<Vec<_>>(), framed(cx));
    let first = rows(cx)[0].0.clone();
    keys(cx, window, &["c", "enter"]);
    draw(cx, window);
    let r = rows(cx);
    let after_return = (r[0].1.clone(), framed(cx));
    keys(cx, window, &["down"]);
    draw(cx, window);
    let after_down = framed(cx);
    keys(cx, window, &["up"]);
    draw(cx, window);
    let after_up = framed(cx);
    keys(cx, window, &["down", "down"]);
    draw(cx, window);
    let past_last = framed(cx);
    assert_eq!(
        (after_tab, after_return, after_down, after_up, past_last),
        (
            (vec!["Default".to_string(), "A".into(), "b".into()], Some(name_key(&first))),
            ("c".to_string(), Some(name_key(&r[1].0))),
            Some(name_key(&r[2].0)),
            Some(name_key(&r[1].0)),
            None,
        ),
        "(rows and frame after name + Tab, first name and frame after typing + Return, the frame after Down, Up, Down twice)"
    );
}

/// Live2 cfg-1118: a second configuration named like the first is accepted (option id "A0", name "A"); a value it can't
/// take ("abc") is kept with nothing failing while another configuration is active, and fails its feature once it is
/// switched to.
#[gpui::test]
fn a_duplicate_configuration_name_and_a_bad_value_wait_for_their_row(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-dup-bad", &[("width", "30 mm")]);
    let (input, second) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let id = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "assignVariable")).map(|f| f.id).expect("the variable");
            let (mut input, mut second) = (String::new(), None);
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                noshape_ops::config::add_option(ps, &input, "A".into());
                second = noshape_ops::config::add_option(ps, &input, "A".into());
                noshape_ops::config::configure_parameter(ps, id, "lengthValue", &input);
            });
            (input, second.expect("a second A"))
        })
    });
    assert_eq!(second, "A0", "the duplicate's option id");
    cx.update(|cx| view.update(cx, |v, cx| v.edit_config("Configure", cx, |ps| noshape_ops::config::set_configured_text(ps, 0, &second, "abc".into()))));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let status = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        let id = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "assignVariable")).map(|f| f.id).unwrap();
        v.display.feature_state(id).map(|s| s.status)
    });
    assert_ne!(status(cx), Some(noshape_model::display::FeatureStatus::Error), "abc kept for A0, Default active: nothing fails");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.set_config_value(&input, second.clone(), w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.feature_state(view.read(cx).studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "assignVariable")).map(|f| f.id).unwrap()).is_some_and(|s| s.status == noshape_model::display::FeatureStatus::Error));
    assert_eq!(status(cx), Some(noshape_model::display::FeatureStatus::Error), "switched to A0: the variable fails");
}

/// Live2 cfg-1118 (qa CFGFEEL-BAD-CELL-PINK): a value that fails is silent until its row is active, then its cell is
/// pink.
#[gpui::test]
fn a_bad_configured_value_is_pink_only_on_the_active_row(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-bad", &[("width", "30 mm")]);
    let (rows, input) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let id = v.studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "assignVariable")).map(|f| f.id).expect("the variable");
            let mut input = String::new();
            let mut a = None;
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                a = noshape_ops::config::add_option(ps, &input, "A".into());
                noshape_ops::config::configure_parameter(ps, id, "lengthValue", &input);
                noshape_ops::config::set_configured_text(ps, 0, a.as_deref().unwrap_or_default(), "abc".into());
            });
            v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
            (a.into_iter().collect::<Vec<_>>(), input)
        })
    });
    draw(cx, window);
    let bad = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("cfg-bad cp-0-{}", rows[0]).into_boxed_str())).is_some();
    assert!(!bad(cx), "silent while the row isn't active");
    view.update(cx, |v, cx| {
        v.config_values.0.insert(input.clone(), rows[0].clone());
        cx.notify();
    });
    draw(cx, window);
    assert!(bad(cx), "pink once its row is active");
}

/// A new Remove extrude of a circle on Top over a block below Top (psui's start hole-sketch): Remove turns the arrow
/// into the block (std canSetBooleanFlip), also when the next pick comes before the logic's run for Remove is back,
/// and the hole goes through. `settle`: wait for the dialog between the picks.
fn remove_extrude_faces(cx: &mut TestAppContext, name: &str, settle: bool, picks: &[(&str, noshape_model::params::ParamValue)]) -> (bool, usize) {
    let (flipped, faces, _) = remove_extrude_result(cx, name, settle, picks);
    (flipped, faces)
}

/// [`remove_extrude_faces`] and the new extrude's status and message.
fn remove_extrude_result(cx: &mut TestAppContext, name: &str, settle: bool, picks: &[(&str, noshape_model::params::ParamValue)]) -> (bool, usize, Option<(noshape_model::display::FeatureStatus, Option<String>)>) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm", "oppositeDirection": true } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[20, 15], [23, 15]] }] }
    ] });
    let (window, view) = part_studio_from(cx, name, Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let sk = v.display.sketches.iter().find(|s| s.regions.len() == 1 && v.studio(cx).unwrap().feature(s.feature).is_some_and(|f| f.name == "Sketch 2")).expect("Sketch 2");
            v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions[0].key.clone() }];
            v.start_feature("extrude", w, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    for (k, v) in picks {
        let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("editing").dialog.clone());
        dialog.update(cx, |d, cx| d.set_value(k, v.clone(), cx));
        if settle {
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        }
    }
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).can_submit()));
    let flipped = cx.update(|cx| view.read(cx).edit.as_ref().expect("editing").dialog.read(cx).feature.params.bool("oppositeDirection"));
    let failed = cx.update(|cx| view.read(cx).tool_failed());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).edit.is_none());
    let faces = cx.update(|cx| view.read(cx).display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the block").face_refs.len());
    let state = cx.update(|cx| {
        let v = view.read(cx);
        let last = v.studio(cx).unwrap().features.last().map(|f| f.id)?;
        v.display.feature_state(last).map(|s| (s.status, s.message.clone()))
    });
    let missed = matches!(state, Some((noshape_model::display::FeatureStatus::Info | noshape_model::display::FeatureStatus::Error, _)));
    assert_eq!(failed, missed, "the preview draws its regions by their outline alone exactly when the tool fails: {state:?}");
    (flipped, faces, state)
}

#[gpui::test]
fn a_remove_extrude_on_top_cuts_the_block_below(cx: &mut TestAppContext) {
    use noshape_model::params::ParamValue;
    let remove = ("operationType", ParamValue::Enum("REMOVE".into()));
    let through = ("endBound", ParamValue::Enum("THROUGH_ALL".into()));
    assert_eq!(remove_extrude_faces(cx, "rm-through", true, &[remove.clone(), through.clone()]), (true, 7), "Remove, then Through all");
}

#[gpui::test]
fn a_remove_extrude_picked_fast_still_cuts_the_block_below(cx: &mut TestAppContext) {
    use noshape_model::params::ParamValue;
    let remove = ("operationType", ParamValue::Enum("REMOVE".into()));
    let through = ("endBound", ParamValue::Enum("THROUGH_ALL".into()));
    assert_eq!(remove_extrude_faces(cx, "rm-through-fast", false, &[remove, through]), (true, 7), "Through all picked before Remove's logic is back");
}

#[gpui::test]
fn a_flipped_remove_extrude_points_away_from_the_block(cx: &mut TestAppContext) {
    use noshape_model::params::ParamValue;
    let remove = ("operationType", ParamValue::Enum("REMOVE".into()));
    let flip = ("oppositeDirection", ParamValue::Bool(false));
    assert!(!remove_extrude_faces(cx, "rm-flip", true, &[remove, flip]).0, "the user's flip after Remove's");
}

/// Live (REQUESTS 2979): Remove, Through all or Blind 10 mm, then one flip: the tool stands above the block, cuts
/// nothing, and the extrude commits as an info, "Selected tools and targets do not intersect.".
#[gpui::test]
fn a_flipped_remove_extrude_missing_the_block_is_an_info(cx: &mut TestAppContext) {
    use noshape_model::display::FeatureStatus;
    use noshape_model::params::ParamValue;
    let remove = ("operationType", ParamValue::Enum("REMOVE".into()));
    let flip = ("oppositeDirection", ParamValue::Bool(false));
    let info = Some((FeatureStatus::Info, Some("Selected tools and targets do not intersect.".to_string())));
    for (name, bound) in [("rm-flip-through", ("endBound", ParamValue::Enum("THROUGH_ALL".into()))), ("rm-flip-blind", ("depth", ParamValue::Expr(noshape_units::ExprValue::new("10 mm"))))] {
        let (flipped, faces, state) = remove_extrude_result(cx, name, true, &[remove.clone(), bound, flip.clone()]);
        assert_eq!((flipped, faces, state), (false, 6, info.clone()), "{name}: (flipped, the block's faces, the extrude's status)");
    }
}

/// Live errors-1123 extrude remove-miss: a Remove beside the block shows the blue "do not intersect" bubble with Merge
/// with all on, and with an empty merge scope only its error ("No merge scope selected."), no bubble.
#[gpui::test]
fn a_remove_missing_with_no_merge_scope_shows_only_its_error(cx: &mut TestAppContext) {
    use noshape_model::display::FeatureStatus;
    use noshape_model::params::ParamValue;
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm", "oppositeDirection": true } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[80, 15], [83, 15]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "rm-miss-noscope", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let sk = v.display.sketches.iter().find(|s| s.regions.len() == 1 && v.studio(cx).unwrap().feature(s.feature).is_some_and(|f| f.name == "Sketch 2")).expect("Sketch 2");
            v.selection = vec![Reference::SketchRegion { sketch: sk.feature, key: sk.regions[0].key.clone() }];
            v.start_feature("extrude", w, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("editing").dialog.clone());
    let bubble = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).message.as_ref().map(|(_, m)| m.clone()));
    let state = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let fid = v.edit.as_ref()?.dialog.read(cx).feature.id;
            v.display.feature_state(fid).map(|s| (s.status, s.message.clone().unwrap_or_default()))
        })
    };
    dialog.update(cx, |d, cx| d.set_value("operationType", ParamValue::Enum("REMOVE".into()), cx));
    dialog.update(cx, |d, cx| d.set_value("defaultScope", ParamValue::Bool(true), cx));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).message.is_some());
    assert_eq!(bubble(cx).as_deref(), Some("Selected tools and targets do not intersect."), "Merge with all on: the info bubble");
    dialog.update(cx, |d, cx| d.set_value("defaultScope", ParamValue::Bool(false), cx));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).message.is_none());
    assert_eq!(state(cx), Some((FeatureStatus::Error, "No merge scope selected.".to_string())));
    assert_eq!(bubble(cx), None, "an empty merge scope: the error only");
}

/// Live2 feedback.json (documentLoad): an opening studio shows "Loading studio data…" until a result is in and
/// "Loading graphics…" while its scene builds; nothing in the first moment of a quick open, nor once it is drawn.
#[gpui::test]
fn an_opening_studio_says_what_it_is_loading(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "loading");
    wait_until(cx, window, |cx| view.read(cx).ui.loaded);
    assert_eq!(cx.update(|cx| view.read(cx).loading_message()), None, "drawn: nothing");
    view.update(cx, |v, _| {
        v.ui.loaded = false;
        v.ui.opened_at = std::time::Instant::now();
    });
    assert_eq!(cx.update(|cx| view.read(cx).loading_message()), None, "a quick open shows nothing at first");
    view.update(cx, |v, _| v.ui.opened_at = std::time::Instant::now() - std::time::Duration::from_secs(1));
    assert_eq!(cx.update(|cx| view.read(cx).loading_message()), Some("Loading studio data…"));
}

/// live ctx-menus-1117: on every graphics menu that names a part (here a part edge's), "Where used for Part 1…" sits
/// in its own group right after the Create Drawing / Export… group.
#[gpui::test]
fn where_used_for_the_part_has_its_own_group_after_export(cx: &mut TestAppContext) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "where-used-menu", Some(starts["block"].clone()));
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().any(|b| b.kind == noshape_model::display::BodyKind::Solid) && !view.read(cx).ui_busy());
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let edge = v.display.bodies.first().and_then(|b| b.edge_refs.first().cloned()).expect("an edge");
            let target = v.built.as_ref().and_then(|b| b.picks.ids_for(&edge).first().and_then(|id| b.picks.get(*id)).cloned());
            v.open_context_menu(target, gpui_kit::point(px(600.), px(400.)), w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    let rows = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels())).expect("the menu");
    let at = rows.iter().position(|r| r == "Where used for Part 1…").expect("Where used for Part 1…");
    assert_eq!((rows[at - 2].as_str(), rows[at - 1].as_str(), rows.get(at + 1).map(String::as_str)), ("Export…", "———", Some("———")), "{rows:?}");
}

/// Live cfg-1118: Configurations docks at the right, 712 px with its 6 px gutter, and the graphics shrink by it (live's
/// 1920 px window).
#[gpui::test]
fn the_configurations_panel_docks_712_px_and_the_graphics_shrink(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "cfg-panel-width");
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_resize(gpui_kit::size(px(1920.), px(1080.)));
    draw(cx, window);
    let graphics = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).viewport.read(cx).size().x);
    let before = graphics(cx);
    cx.update(|cx| view.update(cx, |v, cx| {
        v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
        cx.notify();
    }));
    draw(cx, window);
    draw(cx, window);
    let panel = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("right-panel").expect("the panel drawn");
    let after = graphics(cx);
    let w = f64::from(f32::from(panel.size.width));
    assert!((w - 718.0).abs() < 1.0, "the panel 712 px and its gutter: {w}");
    assert!((before - after - w).abs() < 2.0, "the graphics shrink by the panel: {before} -> {after}, the panel {w}");
}

/// Live2 cfg-1118 (qa CFGFEEL-ROW-CLICK-SWITCHES): a plain click on a configuration row doesn't switch to it; only the
/// row menu's Switch to and the feature list's box do.
#[gpui::test]
fn a_click_on_a_configuration_row_doesnt_switch_to_it(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-row-click", &[("width", "30 mm")]);
    let (rows, input) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                noshape_ops::config::add_option(ps, &input, "A".into());
            });
            v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
            let rows: Vec<String> = match v.studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == input)).map(|i| &i.kind) {
                Some(noshape_model::configuration::ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
                _ => Vec::new(),
            };
            (rows, input)
        })
    });
    draw(cx, window);
    let before = cx.update(|cx| view.read(cx).config_values.0.get(&input).cloned());
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let b = vcx.debug_bounds(Box::leak(format!("cfg-row {input} {}", rows[1]).into_boxed_str())).expect("row A drawn");
    vcx.simulate_click(b.center(), Modifiers::default());
    draw(cx, window);
    let after = cx.update(|cx| view.read(cx).config_values.0.get(&input).cloned());
    assert_ne!(after.as_deref(), Some(rows[1].as_str()), "the click didn't switch to A");
    assert_eq!(after, before);
}

/// Live2 ps-feel2-1118: a configuration row's menu: the current row has no Switch to and no Delete row, the default row
/// no Set as default, the first no Move up, the last no Move down.
#[gpui::test]
fn a_configuration_rows_menu_offers_what_live_offers(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-row-menu", &[("width", "30 mm")]);
    let (rows, input) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| {
                input = noshape_ops::config::add_input(ps, "List");
                noshape_ops::config::add_option(ps, &input, "A".into());
                noshape_ops::config::add_option(ps, &input, "B".into());
            });
            v.right_panel = Some(crate::part_studio::config_panel::RightPanel::Configurations);
            let rows: Vec<String> = match v.studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == input)).map(|i| &i.kind) {
                Some(noshape_model::configuration::ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
                _ => Vec::new(),
            };
            (rows, input)
        })
    });
    assert_eq!(rows.len(), 3, "Default, A, B");
    draw(cx, window);
    let mut menus = Vec::new();
    for r in &rows {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let b = vcx.debug_bounds(Box::leak(format!("cfg-row {input} {r}").into_boxed_str())).expect("the row drawn");
        vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Right, position: b.center(), modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
        vcx.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Right, position: b.center(), modifiers: Modifiers::default(), click_count: 1 });
        draw(cx, window);
        menus.push(cx.update(|cx| crate::os_menu::open_labels(cx)).into_iter().filter(|s| s != "———").collect::<Vec<_>>());
        keys(cx, window, &["escape"]);
    }
    let want: Vec<Vec<String>> = [
        vec!["Duplicate row", "Move down"],
        vec!["Switch to \"A\"", "Set as default", "Duplicate row", "Move up", "Move down", "Delete row"],
        vec!["Switch to \"B\"", "Set as default", "Duplicate row", "Move up", "Delete row"],
    ]
    .iter()
    .map(|m| m.iter().map(|s| s.to_string()).collect())
    .collect();
    assert_eq!(menus, want, "the menus of Default (current, default, first), A and B (last)");
}

const PROBE_FS: &str = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

export enum ProbeMode
{
    annotation { "Name" : "First mode" }
    FIRST,
    annotation { "Name" : "Second mode" }
    SECOND
}

export enum ProbeSide
{
    annotation { "Name" : "Left" }
    LEFT,
    annotation { "Name" : "Right" }
    RIGHT
}

const probeTable = {
        "name" : "size", "displayName" : "Size",
        "entries" : {
            "Small" : { "name" : "grade", "displayName" : "Grade", "entries" : { "A" : 1, "B" : 2 } },
            "Large" : { "name" : "grade", "displayName" : "Grade", "entries" : { "A" : 3, "B" : 4 } }
        }
    };

annotation { "Feature Type Name" : "Probe all", "Feature Type Description" : "live2 probe" }
export const probeAll = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Faces", "Filter" : EntityType.FACE, "MaxNumberOfPicks" : 2 }
        definition.faces is Query;
        annotation { "Name" : "Mode" }
        definition.mode is ProbeMode;
        annotation { "Name" : "Side", "UIHint" : UIHint.HORIZONTAL_ENUM }
        definition.side is ProbeSide;
        annotation { "Name" : "Length" }
        isLength(definition.len, LENGTH_BOUNDS);
        annotation { "Name" : "Angle" }
        isAngle(definition.ang, ANGLE_360_BOUNDS);
        annotation { "Name" : "Count" }
        isInteger(definition.count, POSITIVE_COUNT_BOUNDS);
        annotation { "Name" : "Ratio" }
        isReal(definition.ratio, POSITIVE_REAL_BOUNDS);
        annotation { "Name" : "Label" }
        definition.label is string;
        annotation { "Name" : "Flip", "UIHint" : UIHint.OPPOSITE_DIRECTION }
        definition.flip is boolean;
        annotation { "Name" : "Fail on purpose" }
        definition.fail is boolean;
        annotation { "Name" : "More options" }
        definition.more is boolean;
        if (definition.more)
        {
            annotation { "Name" : "Extra length", "UIHint" : UIHint.REMEMBER_PREVIOUS_VALUE }
            isLength(definition.extra, LENGTH_BOUNDS);
        }
        annotation { "Name" : "Size", "Lookup Table" : probeTable }
        definition.size is LookupTablePath;
    }
    {
        if (definition.fail)
            throw regenError("Probe failure on purpose", ["len"]);
    });
"#;

/// A Part Studio with Feature Studio "Probe" (`PROBE_FS`) and the dialog of its "Probe all" open.
fn probe_dialog(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = part_studio(cx, name);
    let fs = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut id = None;
            v.doc.update(cx, |doc, cx| {
                doc.edit_document("Add tabs", cx, |d| id = Some(d.add_tab("Probe", noshape_model::TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(PROBE_FS)))));
                let _ = doc.custom_features();
            });
            id.expect("the tab")
        })
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature(&noshape_model::custom::type_id(fs, "probeAll"), w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    (window, view)
}

/// Live2 custom-feature-1118 (b1, b3, b4): a custom feature's generated dialog: a plain enum is a select without a
/// label, HORIZONTAL_ENUM tabs, defaults from the bounds, the OPPOSITE_DIRECTION boolean a flip, a lookup table one
/// select per level; ticked, the conditional field shows under its checkbox; a regenError naming `len` turns the title
/// red and Length pink, its message only in the title's tooltip.
#[gpui::test]
fn a_custom_features_dialog_is_generated_and_fails_as_live(cx: &mut TestAppContext) {
    let (window, view) = probe_dialog(cx, "cf-probe");
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let state = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| dialog.read(cx).ui_state(w, cx)).unwrap();
    let fields = |s: &serde_json::Value| -> Vec<(String, String, String)> {
        s["fields"].as_array().into_iter().flatten().map(|f| (f["label"].as_str().unwrap_or("-").to_string(), f["text"].as_str().unwrap_or_default().to_string(), f["state"].as_str().unwrap_or_default().to_string())).collect()
    };
    let s = state(cx);
    assert_eq!(s["tabs"].as_array().map(|t| t.iter().map(|t| t["label"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>()), Some(vec!["Left".to_string(), "Right".to_string()]));
    let labels: Vec<String> = fields(&s).into_iter().map(|f| format!("{} {}", f.0, f.1).trim().to_string()).collect();
    assert_eq!(labels, ["Faces", "- First mode", "Length 25 mm", "Angle 30 deg", "Count 2", "Ratio 1", "Label", "toggle", "Fail on purpose", "More options", "Size Small", "Grade A"]);
    dialog.update(cx, |d, cx| {
        d.set_value("more", noshape_model::ParamValue::Bool(true), cx);
        d.set_value("fail", noshape_model::ParamValue::Bool(true), cx);
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.schedule_regen(w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && dialog.read(cx).status.is_some());
    draw(cx, window);
    let s = state(cx);
    let f = fields(&s);
    assert_eq!(s["titleState"], "error", "the title red");
    assert!(f.iter().any(|f| f.0 == "Length" && f.2 == "error"), "Length pink: {f:?}");
    let more = f.iter().position(|f| f.0 == "More options").expect("More options");
    assert_eq!(f.get(more + 1).map(|f| f.0.as_str()), Some("Extra length"), "under its checkbox");
    let title = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("dialog-title").expect("the title");
    crate::ui_script::clear_tooltip();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(gpui_kit::point(title.origin.x + px(20.), title.center().y), None, Modifiers::default());
    for _ in 0..50 {
        cx.executor().advance_clock(std::time::Duration::from_millis(10));
        draw(cx, window);
    }
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Probe all 1 did not regenerate properly: Probe failure on purpose A parameter value is not valid."));
}

/// Live workflow 13 step 10 (qa CUSTOM-FEATURE-DIALOG-HELP): a custom feature's dialog has no "?" help; a built-in's has.
/// Live errors-1123 lp-feat-off: an error naming a checkbox (Reapply features off: "Could not create all instances…")
/// reads as it is; one naming a value field appends " A parameter value is not valid.".
#[gpui::test]
fn an_error_naming_a_checkbox_adds_no_value_suffix(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "checkbox-error");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("linearPattern", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Linear pattern dialog").dialog.clone());
    let msg = "Could not create all instances, some instances failed to regenerate.";
    let title = |cx: &mut TestAppContext, field: &str| {
        dialog.update(cx, |d, cx| {
            d.set_status(Some((noshape_model::display::FeatureStatus::Error, msg.to_string())), cx);
            d.set_faulty(vec![field.to_string()], false, Vec::new(), cx);
            d.title_status().map(|s| s.1)
        })
    };
    assert_eq!(title(cx, "fullFeaturePattern").as_deref(), Some(msg), "a checkbox: the message alone");
    assert_eq!(title(cx, "distance"), Some(format!("{msg} A parameter value is not valid.")), "a value field: the suffix");
}

/// Live errors-1123 loft empty: an error naming a list of selections (Loft's Profiles) reads as it is.
#[gpui::test]
fn an_error_naming_a_list_of_selections_adds_no_value_suffix(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "array-error");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("loft", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Loft dialog").dialog.clone());
    let msg = "Select profiles in order from start to finish.";
    let title = dialog.update(cx, |d, cx| {
        d.set_status(Some((noshape_model::display::FeatureStatus::Error, msg.to_string())), cx);
        d.set_faulty(vec!["sheetProfilesArray".to_string()], false, Vec::new(), cx);
        d.title_status().map(|s| s.1)
    });
    assert_eq!(title.as_deref(), Some(msg));
}

/// Live errors-1123 loft guides-empty: a guide entry without its pick reads "No selections".
#[gpui::test]
fn an_empty_guide_entry_reads_no_selections(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "empty-guide");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("loft", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Loft dialog").dialog.clone());
    dialog.update(cx, |d, _| {
        d.feature.params.set("addGuides", noshape_model::ParamValue::Bool(true));
        d.feature.params.set("guidesArray", noshape_model::ParamValue::Array(vec![Default::default()]));
    });
    let state = cx.update_window(window, |_, w, cx| dialog.read(cx).ui_state(w, cx)).unwrap().to_string();
    assert!(state.contains("Guides No selections"), "{state}");
}

/// Live errors-1123: an error that blames none of the inputs (draft face-parallel, a fillet too big on a picked face)
/// leaves every entry dark; one that blames an input (a chamfer or fillet too big's edge) draws that entry red.
#[gpui::test]
fn only_the_inputs_an_error_blames_draw_their_entries_red(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "blamed-entry", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let fillet = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Fillet 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(fillet, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Fillet dialog").dialog.clone());
    let red = |cx: &mut TestAppContext, blame: bool| {
        dialog.update(cx, |d, cx| {
            d.set_status(Some((noshape_model::display::FeatureStatus::Error, "Failed to fillet selections.".to_string())), cx);
            let edge = d.feature.params.query("entities")[0].clone();
            d.set_faulty(Vec::new(), false, if blame { vec![edge.clone()] } else { Vec::new() }, cx);
            d.entry_failing("entities", &edge)
        })
    };
    assert_eq!((red(cx, false), red(cx, true)), (false, true), "(blaming nothing, blaming the edge)");
}

/// Live errors-1123 fl-joint2: an error naming a selection field draws its entry red ("Edge of Flange 1"); one naming
/// another field leaves it dark.
#[gpui::test]
fn an_error_naming_a_selection_field_draws_its_entry_red(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "faulty-entry", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let fillet = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Fillet 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(fillet, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Fillet dialog").dialog.clone());
    let red = |cx: &mut TestAppContext, field: &str| {
        dialog.update(cx, |d, cx| {
            d.set_status(Some((noshape_model::display::FeatureStatus::Error, "Failed to fillet.".to_string())), cx);
            d.set_faulty(vec![field.to_string()], false, Vec::new(), cx);
            let edge = d.feature.params.query("entities")[0].clone();
            d.entry_failing("entities", &edge)
        })
    };
    assert_eq!((red(cx, "entities"), red(cx, "radius")), (true, false), "(the error names the edges, the radius)");
}

#[gpui::test]
fn a_custom_features_dialog_has_no_help(cx: &mut TestAppContext) {
    let (window, _view) = probe_dialog(cx, "cf-probe-help");
    assert!(gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("dlg-help").is_none(), "no help in a custom feature's dialog");
    let (window, view) = part_studio(cx, "builtin-help");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    assert!(gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("dlg-help").is_some(), "Extrude's dialog has its help");
}

/// Live2 custom-feature-1118 (Feature Studio edits): a committed edit is picked up by the document at once (the next
/// dialog and regeneration use it), but an open dialog keeps its labels: a renamed parameter isn't shown in it.
#[gpui::test]
fn an_open_custom_feature_dialog_keeps_its_labels_after_a_commit(cx: &mut TestAppContext) {
    let (window, view) = probe_dialog(cx, "cf-probe-commit");
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    let labels = |cx: &mut TestAppContext| -> Vec<String> {
        let s = cx.update_window(window, |_, w, cx| dialog.read(cx).ui_state(w, cx)).unwrap();
        s["fields"].as_array().into_iter().flatten().filter_map(|f| f["label"].as_str().map(str::to_string)).collect()
    };
    assert!(labels(cx).iter().any(|l| l == "Length"), "{:?}", labels(cx));
    let fs = cx.update(|cx| view.read(cx).doc.read(cx).session.doc.tabs.iter().find(|t| t.name == "Probe").map(|t| t.id)).expect("the Probe tab");
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            v.doc.update(cx, |doc, cx| {
                doc.edit_tab(fs, "Commit Feature Studio", cx, |c| {
                    if let noshape_model::TabContent::FeatureStudio(f) = c {
                        f.save(PROBE_FS.replace("\"Name\" : \"Length\"", "\"Name\" : \"Length renamed\""));
                        f.commit();
                    }
                });
                let _ = doc.custom_features();
            });
        })
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let ty = noshape_model::features::get(&noshape_model::custom::type_id(fs, "probeAll")).expect("the committed type");
    assert!(ty.params.iter().any(|p| p.label == "Length renamed"), "the document has the edit");
    let shown = labels(cx);
    assert!(shown.iter().any(|l| l == "Length") && !shown.iter().any(|l| l == "Length renamed"), "the open dialog unchanged: {shown:?}");
}

/// Live2 custom-feature-1118 (e2, e4): a custom feature from another document's V1 once that document has a V2: the
/// row's blue update indicator ("Change version of linked document") and, in its menu, Update linked document… and
/// Open linked document.
#[gpui::test]
fn a_linked_feature_behind_a_newer_version_shows_the_update_indicator(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("noshape-ui-linked-lib-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lib = dir.join("lib.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", lib.display()));
    }
    let lib_s = lib.to_str().unwrap().to_string();
    let mut ctx = noshape_ops::Ctx::new();
    let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
    call("document_create", serde_json::json!({ "path": lib_s }));
    call("tab_create", serde_json::json!({ "kind": "featurestudio", "name": "Probe" }));
    call("featurestudio_edit", serde_json::json!({ "tab": "Probe", "source": PROBE_FS }));
    call("featurestudio_commit", serde_json::json!({ "tab": "Probe" }));
    call("history_create_version", serde_json::json!({ "name": "V1" }));
    let types = call("feature_types", serde_json::json!({ "document": lib_s, "version": "V1" }));
    let ty = types["feature_studios"][0]["features"][0]["type"].as_str().expect("Probe all").to_string();
    call("featurestudio_edit", serde_json::json!({ "tab": "Probe", "source": PROBE_FS.replace("live2 probe", "probe, edited") }));
    call("featurestudio_commit", serde_json::json!({ "tab": "Probe" }));
    call("history_create_version", serde_json::json!({ "name": "V2" }));
    let (window, view) = part_studio_with(cx, "linked-update", |path| {
        let session = crate::session::Session::create(path, crate::user_units::new_document()).unwrap();
        drop(session);
        let mut ctx = noshape_ops::Ctx::new();
        let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call("document_open", serde_json::json!({ "path": path.to_str().unwrap() }));
        call("feature_add", serde_json::json!({ "tab": "Part Studio 1", "type": ty }));
        drop(ctx);
        crate::session::Session::open(path).unwrap()
    });
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui.outdated_linked.is_empty());
    draw(cx, window);
    let id = cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.features.last().map(|f| f.id))).expect("Probe all 1");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    assert!(vcx.debug_bounds(Box::leak(format!("feat-update-{id:?}").into_boxed_str())).is_some(), "the update indicator");
    let row = vcx.debug_bounds(Box::leak(format!("feature-row-{id:?}").into_boxed_str())).expect("the row");
    let at = gpui_kit::point(row.origin.x + px(40.), row.center().y);
    vcx.simulate_mouse_down(at, gpui_kit::MouseButton::Right, Modifiers::default());
    vcx.simulate_mouse_up(at, gpui_kit::MouseButton::Right, Modifiers::default());
    draw(cx, window);
    let labels = cx.update(|cx| crate::os_menu::open_labels(cx));
    assert!(labels.iter().any(|l| l == "Update linked document…") && labels.iter().any(|l| l == "Open linked document"), "{labels:?}");
}


/// Live errors-1118 (Configurations): a Variable input's value out of range ("500" for 1–100 mm) or not a value ("abc")
/// stays in the box, the unit added, pink with a red underline and no message, and the model keeps the last good value;
/// a good one applies.
#[gpui::test]
fn the_configurations_box_refuses_a_value_out_of_range_or_not_a_value(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-refused", &[("width", "30 mm")]);
    let input = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let mut input = String::new();
            v.edit_config("Configure", cx, |ps| input = noshape_ops::config::add_variable_input(ps, "Len", "LENGTH", "10 mm", Some(("1 mm".into(), "100 mm".into()))));
            input
        })
    });
    draw(cx, window);
    let key = format!("cfgbox-val-{input}");
    let enter = |cx: &mut TestAppContext, text: &str| {
        cx.update_window(window, |_, w, cx| {
            let st = view.read(cx).panel_inputs.get(&key).map(|(st, _)| st.clone()).expect("the box's Len field");
            st.update(cx, |s, cx| {
                s.focus(w, cx);
                s.set_value(text.to_string(), w, cx);
            });
        })
        .unwrap();
        draw(cx, window);
        keys(cx, window, &["enter"]);
        draw(cx, window);
        cx.update(|cx| {
            let v = view.read(cx);
            let shown = v.panel_inputs.get(&key).map(|(st, _)| st.read(cx).value().to_string()).unwrap_or_default();
            (shown, v.config_values.0.get(&input).cloned())
        })
    };
    assert_eq!(enter(cx, "500"), ("500 mm".to_string(), None), "out of range: kept with its unit, not applied");
    assert_eq!(enter(cx, "abc"), ("abc".to_string(), None), "not a value: kept, not applied");
    assert_eq!(enter(cx, "20").1.as_deref(), Some("20 mm"), "a good value applies");
}

/// Live errors-1118 (panel.png, panel-len-only.png): the Configurations box stands as tall with two inputs as with one,
/// the second row cut off (it scrolls).
#[gpui::test]
fn the_configurations_box_stays_one_input_tall_with_two(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-box-two", &[("width", "30 mm")]);
    let box_h = |cx: &mut TestAppContext| {
        draw(cx, window);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.debug_bounds("cfg-box").expect("the box drawn").size.height
    };
    cx.update(|cx| view.update(cx, |v, cx| v.edit_config("Configure", cx, |ps| {
        noshape_ops::config::add_variable_input(ps, "Len", "LENGTH", "10 mm", None);
    })));
    let one = box_h(cx);
    let mut list = String::new();
    cx.update(|cx| view.update(cx, |v, cx| v.edit_config("Configure", cx, |ps| {
        list = noshape_ops::config::add_input(ps, "List");
    })));
    let two = box_h(cx);
    assert!((one - two).abs() < px(0.5), "one input {one:?}, two {two:?}");
    // Live cfglabel-1120: a 45 px pane under the title; its rows 2 px in and 25 px short of the box's right edge
    // (Chrome's scrollbar, 5 px each side of it; qa psui5: rows 42..220, the border at 245), the split button 10 px.
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let pane = vcx.debug_bounds("cfg-box").expect("the box drawn");
    let label = vcx.debug_bounds(Box::leak(format!("cfgbox-label-{list}").into_boxed_str())).expect("the List's label");
    let field = vcx.debug_bounds(Box::leak(format!("cfgbox-field-{list}").into_boxed_str())).expect("the List's field");
    assert!((label.left() - pane.left() - px(2.)).abs() < px(0.5), "rows 2 px in: {:?} in {:?}", label, pane);
    assert!((pane.right() - field.right() - px(25.)).abs() < px(0.5), "rows 25 px short (the scrollbar): {:?} in {:?}", field, pane);
    let split = vcx.debug_bounds("cfgbox-split").expect("the split button");
    assert!((pane.right() - split.right() - px(10.)).abs() < px(0.5), "the split button 10 px from the border: {:?} in {:?}", split, pane);
    assert!(cx.update(|cx| view.read(cx).ui.cfg_box_overflow.get()), "the second input scrolls");
}

/// Live cfglabel-1120: the Configurations box has no label column: a label is its text + 5 px each side, the field takes
/// the rest of the row; a Variable's field keeps 100 px, its long label cut to what is left.
#[gpui::test]
fn the_configurations_box_labels_take_their_texts_width(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-labels", &[("width", "30 mm")]);
    let (size, long) = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let (mut size, mut long) = (String::new(), String::new());
            v.edit_config("Configure", cx, |ps| {
                size = noshape_ops::config::add_input(ps, "List");
                noshape_ops::config::rename_input(ps, &size, "Size".into());
                long = noshape_ops::config::add_variable_input(ps, "A much longer input name", "LENGTH", "25 mm", None);
            });
            v.ui.cfg_box_h = Some(200.);
            (size, long)
        })
    });
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let mut b = |k: &str| vcx.debug_bounds(Box::leak(k.to_string().into_boxed_str())).unwrap_or_else(|| panic!("{k} drawn"));
    let (sl, sf) = (b(&format!("cfgbox-label-{size}")), b(&format!("cfgbox-field-{size}")));
    let (ll, lf) = (b(&format!("cfgbox-label-{long}")), b(&format!("cfgbox-field-{long}")));
    let w = |r: Bounds<gpui_kit::Pixels>| f32::from(r.size.width);
    assert!((20.0..50.0).contains(&w(sl)), "Size's label {:.1} px: its text + 10 (live 31.9), no 80 px column", w(sl));
    assert!((sf.left() - sl.right()).abs() < px(0.5), "Size's field starts at its label's end");
    assert!((w(lf) - 100.).abs() < 0.5, "the Variable's field keeps 100 px: {:.1}", w(lf));
    assert!((lf.left() - ll.right()).abs() < px(0.5) && (w(ll) + w(lf) - (w(sl) + w(sf))).abs() < 0.5, "the long label takes the rest of the row");
}

/// Live's sketch keys (interaction-feel shortcuts-sk-1118): Ctrl+M does nothing; ` with nothing under the pointer
/// opens Select other "(No selections found)"; Shift+X opens Section view over the sketch, its Esc leaving the sketch
/// open; Shift+N makes the sketch dialog's name an edit box, Enter naming the sketch; Shift+V closes the sketch (as
/// ✓) and opens Named views.
#[gpui::test]
fn sketch_keys_as_lives(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "sk-keys", Tool::Select);
    let open = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.is_some());
    keys(cx, window, &["ctrl-m"]);
    assert!(open(cx), "Ctrl+M leaves the sketch open");
    assert_eq!(cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.type_id.clone())), Some("sketch".to_string()), "no Mate connector");
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: DVec2::new(900.0, 150.0), modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["`"]);
    assert_eq!(cx.update(|cx| view.read(cx).ui.select_other.as_ref().map(|s| s.hits.len())), Some(0), "the empty Select other box");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).ui.select_other.is_none()) && open(cx), "Esc closes the box, the sketch stays");
    keys(cx, window, &["shift-x"]);
    assert!(cx.update(|cx| view.read(cx).ui.section_dialog.is_some()) && open(cx), "Section view over the sketch");
    keys(cx, window, &["escape"]);
    assert!(cx.update(|cx| view.read(cx).ui.section_dialog.is_none()) && open(cx), "its Esc leaves the sketch open");
    keys(cx, window, &["shift-n"]);
    assert!(cx.update(|cx| view.read(cx).edit.as_ref().is_some_and(|e| e.dialog.read(cx).title_editing())), "the name is an edit box");
    keys(cx, window, &["B", "o", "x", "enter"]);
    assert_eq!(cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.name.clone())), Some("Box".to_string()));
    assert!(open(cx), "Enter in the name leaves the sketch open");
    keys(cx, window, &["shift-v"]);
    assert!(!open(cx) && cx.update(|cx| view.read(cx).named_views.is_some()), "Shift+V: the sketch closed, Named views open");
    let names: Vec<String> = cx.update(|cx| view.read(cx).studio(cx).map(|s| s.features.iter().map(|f| f.name.clone()).collect()).unwrap_or_default());
    assert!(names.iter().any(|n| n == "Box"), "the sketch kept as Box: {names:?}");
}

/// Live walk7 7-saved (qa FEEL-NAMED-VIEW-AFTER-SAVE): a view saved with Enter in Named views is then the picked one
/// (the select reads it, ⊗ on), the name field empty for the next.
#[gpui::test]
fn a_saved_named_view_is_the_picked_one(cx: &mut TestAppContext) {
    let (window, view) = part_studio_from(cx, "named-view-saved", None);
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_named_views(w, cx))).unwrap();
    draw(cx, window);
    assert!(cx.update(|cx| view.read(cx).named_views.is_some()), "Named views open");
    keys(cx, window, &["w", "7", "enter"]);
    let (picked, typed) = cx.update(|cx| {
        let v = view.read(cx);
        (v.named_view_picked.clone(), v.named_views.as_ref().map(|(st, _)| st.read(cx).value().to_string()))
    });
    assert_eq!((picked.as_deref(), typed.as_deref()), (Some("w7"), Some("")), "the select reads w7, the field empty");
}

/// Live shortcuts-sk-1118 / sketch-readout-1119: Shift+C in a sketch with no tool on selects the analysed line in the
/// sketch (orange) and the bottom right reads its Length.
#[gpui::test]
fn curve_analysis_in_a_sketch_selects_the_analysed_line(cx: &mut TestAppContext) {
    use crate::part_studio::analysis_tools::ToolKind;
    let (window, view) = sketch_with(cx, "curve-in-sketch", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape", "escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        if let Some(ed) = &mut v.sketch {
            ed.selection.clear();
        }
        v.toggle_analysis_tool(ToolKind::CurveSurface, w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let (field, readout) = cx.update(|cx| {
        let v = view.read(cx);
        let units = v.doc.read(cx).session.doc.settings.units;
        let readout = v.sketch_data(cx).zip(v.sketch.as_ref()).map(|(data, ed)| ed.selection_readout(&data, &units)).unwrap_or_default();
        (v.ui.analysis_tool.as_ref().map(|t| t.selection.len()), readout)
    });
    assert_eq!(field, Some(1), "the line in the field");
    assert_eq!(readout.first().map(|r| r.0.as_str()), Some("Length"), "{readout:?}");
}

/// Live tools-1119 sc-2 / sc-3: in a rectangle's sketch with one line selected, Shift+C analyses that line only: one
/// curve in the field, only it selected, the Length readout kept.
#[gpui::test]
fn curve_analysis_in_a_sketch_takes_the_selected_line_only(cx: &mut TestAppContext) {
    use crate::part_studio::analysis_tools::ToolKind;
    let (window, view) = sketch_with(cx, "curve-in-sketch-one", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["escape", "escape"]);
    let line = cx.update(|cx| {
        let v = view.read(cx);
        let sk = v.sketch.as_ref().map(|s| s.feature).expect("editing");
        let sd = v.display.sketches.iter().find(|s| s.feature == sk).expect("its display");
        assert_eq!(sd.curves.len(), 4, "the rectangle's lines");
        noshape_sketch::EntityId(sd.curves[0].entity)
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        if let Some(ed) = &mut v.sketch {
            ed.selection = vec![line];
        }
        v.toggle_analysis_tool(ToolKind::CurveSurface, w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let (field, selected, readout) = cx.update(|cx| {
        let v = view.read(cx);
        let units = v.doc.read(cx).session.doc.settings.units;
        let readout = v.sketch_data(cx).zip(v.sketch.as_ref()).map(|(data, ed)| ed.selection_readout(&data, &units)).unwrap_or_default();
        (v.ui.analysis_tool.as_ref().map(|t| t.selection.len()), v.sketch.as_ref().map(|s| s.selection.clone()), readout)
    });
    assert_eq!((field, selected), (Some(1), Some(vec![line])), "(curves in the field, the sketch's selection)");
    assert_eq!(readout.first().map(|r| r.0.as_str()), Some("Length"), "{readout:?}");
}

/// Live2 session-studio-1119 (shot 31): Add configuration input › Checkbox opens the "Checkbox input" dialog (Name,
/// Default ticked; ✓ off until named) instead of adding "Checkbox 2" at once; "Rounded" + Enter adds "Rounded
/// (Default: true)".
#[gpui::test]
fn add_configuration_input_checkbox_opens_its_dialog(cx: &mut TestAppContext) {
    let (window, view) = studio_with_variables(cx, "cfg-checkbox-dialog", &[("width", "30 mm")]);
    let inputs = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).map(|s| s.configuration.inputs.clone()).unwrap_or_default());
    let before = inputs(cx).len();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_checkbox_input(w, cx))).unwrap();
    draw(cx, window);
    assert_eq!(inputs(cx).len(), before, "nothing added yet");
    cx.update(|cx| {
        let v = view.read(cx);
        let d = v.ui.var_input.as_ref().expect("the dialog");
        assert_eq!(d.checkbox, Some(true), "Default ticked");
    });
    cx.update_window(window, |_, w, cx| {
        let st = view.read(cx).ui.var_input.as_ref().map(|d| d.name.clone()).expect("its Name");
        st.update(cx, |s, cx| s.set_value("Rounded".to_string(), w, cx));
    })
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["enter"]);
    draw(cx, window);
    let all = inputs(cx);
    let added = all.last().expect("an input");
    assert_eq!(all.len(), before + 1);
    assert_eq!(added.name, "Rounded");
    assert!(matches!(added.kind, noshape_model::configuration::ConfigInputKind::Checkbox { default: true }), "{:?}", added.kind);
    assert!(cx.update(|cx| view.read(cx).ui.var_input.is_none()), "closed");
}

/// Live (ui-parity sketch-vertical-key, live2's camera retake): in the isometric view a line from the origin turns
/// vertical only while the pointer is within 7.5 px on screen of the plumb line through its start: 9 px off (the
/// recording's (0, 0) to (3, 30) on Top) adds Coincident alone, 6 px off makes it Vertical; the pointer coming there
/// in 8 moves through the reach (live's recorder) doesn't hold it.
#[gpui::test]
fn a_lines_vertical_reach_is_measured_on_the_screen_in_the_app(cx: &mut TestAppContext) {
    for (off, vertical, moves) in [(9.0, false, 0), (6.0, true, 0), (9.0, false, 8)] {
        let (window, view) = sketch_with(cx, "vertical-reach", Tool::Line);
        let (o, ex, ey) = cx.update(|cx| view.read(cx).sketch.as_ref().and_then(|ed| ed.screen_map)).expect("a screen map");
        let to_px = |p: DVec2| glam::DMat2::from_cols(ex, ey).inverse() * (p - o);
        let start = to_px(DVec2::ZERO);
        let up = (to_px(DVec2::new(0.0, 0.03)) - start).normalize();
        let end = start + up * 120.0 + up.perp() * off;
        click(cx, window, &view, (start.x, start.y));
        for k in 1..moves {
            let at = start + (end - start) * (k as f64 / moves as f64);
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
                });
            })
            .unwrap();
            draw(cx, window);
        }
        click(cx, window, &view, (end.x, end.y));
        let got = cx.update(|cx| view.read(cx).sketch_data(cx).expect("sketching").sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Vertical { .. })));
        assert_eq!(got, vertical, "{off} px off plumb on screen, {moves} moves");
        cx.update_window(window, |_, w, _| w.remove_window()).unwrap();
    }
}

/// qa STALE-SKETCH-HOVER (b): a line's end lit by its reach (no GPU pick there) goes dark with the one move that takes
/// the pointer away into empty space, not a move later.
#[gpui::test]
fn a_reach_lit_end_goes_dark_with_the_move_away(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "stale-end-hover", Tool::Line);
    click(cx, window, &view, (300.0, 200.0));
    click(cx, window, &view, (400.0, 200.0));
    keys(cx, window, &["escape"]);
    let origin = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
    let real_move = |cx: &mut TestAppContext, x: f32, y: f32| {
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(origin + gpui_kit::point(px(x), px(y)), None, Modifiers::default());
        draw(cx, window);
    };
    real_move(cx, 403.0, 202.0);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::Hover(None), w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().and_then(|ed| ed.hover));
    assert!(lit(cx).is_some(), "the end lit within its reach");
    real_move(cx, 600.0, 350.0);
    assert_eq!(lit(cx), None, "dark after the one move away");
}

/// Live rb-followup 11-18 (feel rows "The bar is #b8b8b8 away from the pointer, #c8d9e2 hovered"): the rollback bar is
/// grey, light blue-grey while the pointer is on it and grey again once it leaves.
#[gpui::test]
fn the_rollback_bar_is_grey_and_light_blue_grey_while_hovered(cx: &mut TestAppContext) {
    let (window, view) = one_part(cx, "rb-hover");
    draw(cx, window);
    let grey = crate::theme::p().rollback;
    assert_eq!(grey, gpui_kit::rgb(0xb8b8b8), "the bar's grey");
    let lit = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        crate::part_studio::rollback_bar_colour(false, v.ui.rollback_hovered.is_some(), grey)
    });
    let to = |cx: &mut TestAppContext, at: Point<gpui_kit::Pixels>| {
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
        draw(cx, window);
    };
    let bar = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("rollback").expect("the rollback bar");
    to(cx, gpui_kit::point(px(900.), px(600.)));
    let away = lit(cx);
    to(cx, bar.center());
    let on = lit(cx);
    to(cx, bar.center() + gpui_kit::point(px(0.), px(120.)));
    assert_eq!((away, on, lit(cx)), (grey, gpui_kit::rgb(0xc8d9e2), grey), "the bar away, under the pointer, after it left");
}

/// Live (ps-list-1119 "a hole edge hovered in the graphics"): a hovered edge lights in the graphics and tints no row of
/// the feature list or the Parts list.
#[gpui::test]
fn a_hovered_edge_tints_no_list_row(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "edge-hover-rows");
    view.update(cx, |v, _| v.selection.clear());
    let edge = cx
        .update(|cx| view.read(cx).built.as_ref().and_then(|b| b.picks.ids_where(|t| t.kind == noshape_scene::builder::PickKind::Edge).next()))
        .expect("an edge pick");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        let vp = v.viewport.clone();
        v.on_viewport_event(&vp, &ViewportEvent::Hover(Some(edge)), w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let state = cx.update(|cx| {
        let v = view.read(cx);
        let rows = v.display.bodies.iter().map(|b| v.part_row_flags(b, cx)).filter(|f| *f != (false, false)).count();
        (v.highlighted_hover(cx).is_empty(), v.related_features(cx).len(), v.selected_features.len(), v.ui.list_hover, rows)
    });
    assert_eq!(state, (false, 0, 0, None, 0), "(nothing lit, related rows, selected rows, list hover, tinted part rows)");
}

/// Live workflow 10 (qa FEEL-ROW-HOVER-IN-OWN-DIALOG): Extrude 1 double-clicked to edit, the pointer still on its row,
/// lights only the region its dialog picked: the row of the feature being edited pre-highlights nothing (ours outlined
/// the whole extrude); another row hovered still does.
#[gpui::test]
fn the_edited_features_own_row_hovered_lights_nothing(cx: &mut TestAppContext) {
    let (window, view, _, _) = block_part(cx, "own-row-hover");
    let (sketch, extrude) = cx.update(|cx| {
        let st = view.read(cx).studio(cx).expect("the studio");
        let id = |ty: &str| st.features.iter().find(|f| f.type_id == ty).map(|f| f.id).expect(ty);
        (id("sketch"), id("extrude"))
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(extrude, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).edit.is_some());
    let lit = |cx: &mut TestAppContext, row: noshape_model::reference::FeatureId| {
        view.update(cx, |v, _| v.ui.list_hover = Some(row));
        draw(cx, window);
        cx.update(|cx| view.read(cx).highlighted_hover(cx).len())
    };
    let (own, other) = (lit(cx, extrude), lit(cx, sketch));
    assert!(own == 0 && other > 0, "{own} entities lit by Extrude 1's own row, {other} by Sketch 1's");
}

/// Live 1.223 (rn-1222-1223 st4.json, sticky-h-*.png; TABLES-STICKY-HEADER-HSCROLL): the Cut list is live's table
/// ("Frame" members' Item | Qty | Standard | Description | Length | Angle 1 | Angle 2); scrolling the panel, the 34 px
/// column header stops at the panel's top while the rows pass under it, and is back under the title once scrolled back.
#[gpui::test]
fn a_cut_list_header_sticks_to_the_panel_top(cx: &mut TestAppContext) {
    // 24 unconnected members of different lengths: as many rows, taller than the panel.
    let shapes: Vec<serde_json::Value> = (0..24).map(|i| serde_json::json!({ "line": [[0, 40 * i], [20 + 5 * i, 40 * i]] })).collect();
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": shapes },
        { "type": "frame", "name": "Frame 1", "params": { "selections": ["Sketch 1 edges"] } }
    ] });
    let (window, view) = part_studio_from(cx, "cut-list-sticky", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).has_frames());
    view.update(cx, |v, cx| {
        v.right_panel = Some(crate::part_studio::config_panel::RightPanel::CutList);
        cx.notify();
    });
    draw(cx, window);
    let head = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("table-head:Cut list").map(|b| f32::from(b.top()));
    let panel_top = |cx: &mut TestAppContext| cx.update(|cx| f32::from(view.read(cx).ui.ct.panel_scroll.bounds().top()));
    let scroll = |cx: &mut TestAppContext, y: f32| {
        // A frame clamps the offset to the content (as a wheel scroll does at once), the next renders from it.
        cx.update(|cx| view.read(cx).ui.ct.panel_scroll.set_offset(gpui::point(px(0.), px(-y))));
        draw(cx, window);
        draw(cx, window);
    };
    let rows = cx.update(|cx| view.read(cx).context.as_ref().map(|c| noshape_model::regen::features::frames::cut_list(c, None).len()));
    let at_rest = head(cx).expect("the cut list's column header");
    let top = panel_top(cx);
    scroll(cx, 300.);
    let scrolled = head(cx).expect("the header while scrolled");
    let (offset, max) = cx.update(|cx| {
        let s = &view.read(cx).ui.ct.panel_scroll;
        (f32::from(s.offset().y), f32::from(s.max_offset().y))
    });
    let bar = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("table-hscroll:Cut list");
    let panel = cx.update(|cx| view.read(cx).ui.ct.panel_scroll.bounds());
    let laid = cx.update(|cx| view.read(cx).ui.ct.grid_tops.borrow().clone());
    scroll(cx, 0.);
    let back = head(cx).expect("the header scrolled back");
    let near = |a: f32, b: f32| (a - b).abs() < 1.5;
    // Narrower than the table: the sideways bar sits on the panel's bottom while the table's own bottom is out of
    // view, and right under the table once scrolled to its end.
    let wide = crate::ui::right_panel_w();
    crate::ui::set_right_panel_w(300.);
    scroll(cx, 0.);
    let bar_at = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("table-hscroll:Cut list").map(|b| f32::from(b.bottom()));
    let grid_bottom = |cx: &mut TestAppContext| {
        let (offset, laid) = cx.update(|cx| (f32::from(view.read(cx).ui.ct.panel_scroll.offset().y), view.read(cx).ui.ct.grid_tops.borrow().get("Cut list").copied()));
        laid.map(|(t, h)| t + offset + h)
    };
    let pinned = bar_at(cx);
    let panel_bottom = cx.update(|cx| f32::from(view.read(cx).ui.ct.panel_scroll.bounds().bottom()));
    scroll(cx, 10_000.);
    let (under, end) = (bar_at(cx), grid_bottom(cx));
    crate::ui::set_right_panel_w(wide);
    assert_eq!(
        (pinned.map(|b| near(b, panel_bottom)), under.zip(end).map(|(b, e)| near(b, e + 16.))),
        (Some(true), Some(true)),
        "(the bar on the panel's bottom, then under the table): {pinned:?} vs {panel_bottom}, {under:?} vs {end:?} + 16"
    );
    assert_eq!(
        (rows, at_rest > top + 30., near(scrolled, top), near(back, at_rest)),
        (Some(24), true, true, true),
        "(cut list rows, header under the title at rest, at the panel's top scrolled 300 px, back in place): at rest {at_rest}, scrolled {scrolled}, back {back}, panel top {top}; scrolled to {offset} of {max}, panel {panel:?}, sideways bar {bar:?}, grids {laid:?}"
    );
}

/// Live cutlist-wheel (qa CUTLIST-WHEEL-SIDEWAYS, 24 rows, the table wider than the Tables panel): a plain wheel over
/// the rows or the header scrolls the panel down, never the table sideways; a sideways wheel (Shift+wheel on every OS)
/// scrolls the table and the panel stays put; a wheel over the footer strip below scrolls nothing.
#[gpui::test]
fn a_cut_list_wheel_scrolls_the_panel_and_a_sideways_wheel_the_table(cx: &mut TestAppContext) {
    let shapes: Vec<serde_json::Value> = (0..24).map(|i| serde_json::json!({ "line": [[0, 40 * i], [20 + 5 * i, 40 * i]] })).collect();
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": shapes },
        { "type": "frame", "name": "Frame 1", "params": { "selections": ["Sketch 1 edges"] } }
    ] });
    let (window, view) = part_studio_from(cx, "cut-list-wheel", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).has_frames());
    let wide = crate::ui::right_panel_w();
    crate::ui::set_right_panel_w(300.);
    view.update(cx, |v, cx| {
        v.right_panel = Some(crate::part_studio::config_panel::RightPanel::CutList);
        cx.notify();
    });
    draw(cx, window);
    draw(cx, window);
    let offsets = |cx: &mut TestAppContext| cx.update(|cx| {
        let ct = &view.read(cx).ui.ct;
        let table = ct.grid_scrolls.borrow().get("Cut list").map_or(0., |s| f32::from(s.offset().x));
        (f32::from(ct.panel_scroll.offset().y) < 0., table < 0.)
    });
    let reset = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let ct = &view.read(cx).ui.ct;
            ct.panel_scroll.set_offset(gpui::point(px(0.), px(0.)));
            if let Some(s) = ct.grid_scrolls.borrow().get("Cut list") {
                s.set_offset(gpui::point(px(0.), px(0.)));
            }
        });
        draw(cx, window);
    };
    let wheel = |cx: &mut TestAppContext, at: gpui::Point<gpui::Pixels>, x: f32, y: f32| {
        // (The wheel goes to what the last frame's hit test put under the pointer.)
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
        draw(cx, window);
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::ScrollWheelEvent { position: at, delta: gpui_kit::ScrollDelta::Lines(gpui_kit::point(x, y)), ..Default::default() });
        draw(cx, window);
        offsets(cx)
    };
    let head = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("table-head:Cut list").expect("the column header");
    // The header runs past the panel's right edge: aim inside the panel.
    let head_at = gpui::point(head.left() + px(100.), head.center().y);
    let row = head_at + gpui::point(px(0.), px(60.));
    let footer = cx.update(|cx| {
        let b = view.read(cx).ui.ct.panel_scroll.bounds();
        gpui::point(b.center().x, b.bottom() + px(20.))
    });
    let scrolls = cx.update(|cx| view.read(cx).ui.ct.grid_scrolls.borrow().get("Cut list").map(|s| f32::from(s.max_offset().x)));
    let mut got = Vec::new();
    for (at, x, y) in [(row, 0., -3.), (head_at, 0., -3.), (row, -3., 0.), (footer, 0., -3.), (footer, -3., 0.)] {
        reset(cx);
        got.push(wheel(cx, at, x, y));
    }
    crate::ui::set_right_panel_w(wide);
    assert!(scrolls.is_some_and(|m| m > 50.), "the table wider than the panel: sideways range {scrolls:?}");
    assert_eq!(
        got,
        vec![(true, false), (true, false), (false, true), (false, false), (false, false)],
        "(panel scrolled, table scrolled sideways) after a wheel over a row, the header, a sideways wheel over a row, a wheel and a sideways wheel over the footer"
    );
}

/// The S toolbar in a sketch (live behaviours "S key", sketch/38; qa S-TOOLBAR-BORDER): 175 × 62 with its 1 px border,
/// left at the pointer + 10, top at the pointer − 30.
#[gpui::test]
fn the_s_toolbar_is_175_by_62_at_the_pointer(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "s-toolbar", Tool::Select);
    view.update(cx, |v, cx| {
        v.ui.pointer = Some(DVec2::new(300.0, 200.0));
        v.toggle_mini_toolbar(cx);
    });
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let b = vcx.debug_bounds("mini-toolbar").expect("the S toolbar");
    let vp = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
    let (x, y) = (f32::from(b.origin.x - vp.x), f32::from(b.origin.y - vp.y));
    assert!((w - 175.0).abs() < 0.6 && (h - 62.0).abs() < 0.6 && (x - 310.0).abs() < 0.6 && (y - 170.0).abs() < 0.6, "{w} × {h} at ({x}, {y}) in the viewport");
}

/// Qa 11-22 (SKETCH-USE-VERTEX-OTHER-SKETCH, live workflows-1122 12 s9–12): in a sketch on Right, Select on another
/// sketch's line end, then u: live projects it at once into a point (ours only switched the Use tool on, no point).
#[gpui::test]
fn u_on_another_sketchs_vertex_projects_it(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "line": [[-10, -20], [10, -20]] }] },
    ] });
    let (window, view) = part_studio_from(cx, "use-other-vertex", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.sketches.is_empty());
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![Reference::Plane(DatumPlane::Right)];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    let (window, view) = sketch_in_tool(cx, window, view, Tool::Select);
    let end = cx.update(|cx| view.read(cx).viewport.read(cx).project(glam::DVec3::new(-0.010, -0.020, 0.0))).expect("the line end on screen");
    click(cx, window, &view, (end.x, end.y));
    keys(cx, window, &["u"]);
    let points = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).count());
    assert_eq!(points, 1, "the line end projected into a point");
}

/// Live's workflow scenarios (qa UIS-WORLD-TARGET): `{"world": [x, y, z]}` is a model point in mm, on empty space or a
/// manipulator, placed as a vertex's.
#[gpui::test]
fn a_ui_script_world_target_is_a_model_point(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "uis-world");
    draw(cx, window);
    let at = |cx: &mut TestAppContext, t: serde_json::Value| cx.update(|cx| view.read(cx).ui_target_point(&t, cx));
    let world = at(cx, serde_json::json!({ "world": [10, 0, 0] }));
    let vertex = at(cx, serde_json::json!({ "vertex": [10, 0, 0] }));
    assert!(world.is_ok() && world == vertex, "world {world:?}, vertex {vertex:?}");
}

/// UIS-STATE-GEOMETRY: the parity state lists each sketch entity's points in sketch mm and a fit spline's end
/// derivatives, so a step that moves a point or a handle compares as a difference, not as the same kinds.
#[gpui::test]
fn the_parity_state_lists_sketch_geometry(cx: &mut TestAppContext) {
    let block = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] }
    ] });
    let (window, view) = part_studio_from(cx, "uip-geometry", Some(block));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let id = cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            let id = v.studio(cx).unwrap().features.iter().find(|f| f.name == "Sketch 1").unwrap().id;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Spline", cx, |ps| {
                    let f = ps.features.iter_mut().find(|f| f.id == id).unwrap();
                    let Some(noshape_model::params::ParamValue::Sketch(mut arc)) = f.params.get("sketch").cloned() else { panic!("a sketch") };
                    let d = std::sync::Arc::make_mut(&mut arc);
                    d.sketch.add_fit_spline_with_handles(&[glam::DVec2::new(0.0, 0.05), glam::DVec2::new(0.02, 0.07), glam::DVec2::new(0.04, 0.05)]).unwrap();
                    f.params.set("sketch", noshape_model::params::ParamValue::Sketch(arc));
                })
            });
            id
        })
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let geometry = state["sketch"]["geometry"].as_array().cloned().unwrap_or_default();
    let ends = |g: &serde_json::Value| {
        let mut p: Vec<(i64, i64)> = g["points"].as_array().unwrap().iter().map(|p| (p[0].as_f64().unwrap().round() as i64, p[1].as_f64().unwrap().round() as i64)).collect();
        p.sort();
        p
    };
    let mut lines: Vec<Vec<(i64, i64)>> = geometry.iter().filter(|g| g["kind"] == "line").map(ends).collect();
    lines.sort();
    assert_eq!(lines, [vec![(0, 0), (0, 30)], vec![(0, 0), (40, 0)], vec![(0, 30), (40, 30)], vec![(40, 0), (40, 30)]], "{geometry:?}");
    let spline = geometry.iter().find(|g| g["kind"] == "spline").expect("the spline listed");
    assert_eq!(spline["points"], serde_json::json!([[0.0, 50.0], [20.0, 70.0], [40.0, 50.0]]));
    let d = spline["derivatives"].as_array().expect("its end derivatives");
    assert!(d.len() == 2 && d.iter().all(|v| v.as_array().is_some_and(|v| v.len() == 2)), "{spline}");
    assert!(!geometry.iter().any(|g| g["kind"] == "construction line"), "its handle arms go with the spline: {geometry:?}");
}

/// User 11-23 (v0.4.67): the Offset tool's value box drew no background (the typed text floated over the sketch
/// with only its blue line) and a Tab typed into it showed as a black missing-glyph box. Live: a white box with its
/// border; letters turn it red; Tab types nothing. Checked on the painted quads: an opaque white fill under the
/// whole box, a 1 px caret.
#[gpui::test]
fn the_offset_value_box_is_an_opaque_white_box_with_a_thin_caret(cx: &mut TestAppContext) {
    use gpui_kit::EntityInputHandler as _;
    let (window, view) = sketch_with(cx, "offset-box", Tool::Line);
    click(cx, window, &view, (400., 300.));
    click(cx, window, &view, (500., 300.));
    keys(cx, window, &["escape", "escape"]);
    let (window, view) = sketch_in_tool(cx, window, view, Tool::Offset);
    click(cx, window, &view, (450., 300.));
    assert!(cx.update(|cx| view.read(cx).sketch.as_ref().is_some_and(|ed| ed.offset_pending.is_some())), "the offset preview");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_offset_input(w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["a", "s", "d", "f"]);
    assert!(cx.update(|cx| view.read(cx).ui_ref().is_some_and(|u| u.offset_invalid)), "letters: the box red");
    let field = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("sk-offset-field").expect("the offset box drawn");
    let (scale, quads) = cx.update_window(window, |_, w, _| (w.scale_factor(), w.painted_quads())).unwrap();
    let (l, t, r, b) = (f32::from(field.left()) * scale, f32::from(field.top()) * scale, f32::from(field.right()) * scale, f32::from(field.bottom()) * scale);
    let inside = |q: &gpui_kit::Quad| q.bounds.origin.x.0 >= l - 0.5 && q.bounds.origin.y.0 >= t - 0.5 && q.bounds.origin.x.0 + q.bounds.size.width.0 <= r + 0.5 && q.bounds.origin.y.0 + q.bounds.size.height.0 <= b + 0.5;
    let fill = quads.iter().filter(|q| inside(q)).filter(|q| q.bounds.size.width.0 >= (r - l) * 0.9 && q.bounds.size.height.0 >= (b - t) * 0.9).filter_map(|q| q.background.as_solid()).find(|c| c.a >= 0.999);
    assert!(fill.is_some_and(|c| c.l >= 0.9), "an opaque light fill under the whole box: {fill:?}");
    let tall = |q: &&gpui_kit::Quad| q.bounds.size.height.0 >= 8.0 * scale && q.bounds.size.width.0 < (r - l) * 0.5;
    let marks: Vec<f32> = quads.iter().filter(|q| inside(q)).filter(tall).filter(|q| q.background.as_solid().is_some_and(|c| c.a > 0.5)).map(|q| q.bounds.size.width.0 / scale).collect();
    let all: Vec<String> = quads.iter().filter(|q| inside(q)).map(|q| format!("{:.1}x{:.1}@{:.1},{:.1} {:?}", q.bounds.size.width.0, q.bounds.size.height.0, q.bounds.origin.x.0, q.bounds.origin.y.0, q.background.as_solid())).collect();
    assert!(!marks.is_empty() && marks.iter().all(|w| *w <= 1.01), "only a thin caret inside the box, no block: widths {marks:?}; box {l},{t}-{r},{b} x{scale}: {all:?}");
    let mut tab = Keystroke::parse("tab").unwrap();
    tab.key_char = Some("\t".into());
    cx.update_window(window, |_, w, cx| w.dispatch_keystroke(tab, cx)).unwrap();
    draw(cx, window);
    // (macOS inserts a key's text itself when no binding takes it.)
    let st = cx.update(|cx| view.read(cx).ui_ref().and_then(|u| u.offset_input.as_ref().map(|(st, _)| st.clone()))).expect("the offset box open");
    cx.update_window(window, |_, w, cx| st.update(cx, |s, cx| s.replace_text_in_range(None, "\t", w, cx))).unwrap();
    draw(cx, window);
    assert_eq!(cx.update(|cx| st.read(cx).value().to_string()), "asdf", "Tab types nothing");
}

/// qa UIS-DIM-LABEL-TARGET: `{"dimension": …}` is a sketch dimension's label, by its shown text or its value, so a
/// session can double-click it to edit.
#[gpui::test]
fn a_ui_script_dimension_target_is_its_label(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "uis-dim", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter", "escape", "escape"]);
    draw(cx, window);
    let at = |cx: &mut TestAppContext, t: serde_json::Value| cx.update(|cx| view.read(cx).ui_target_point(&t, cx));
    let by_text = at(cx, serde_json::json!({ "dimension": "50 mm" }));
    let by_value = at(cx, serde_json::json!({ "dimension": "50" }));
    assert!(by_text.is_ok() && by_text == by_value, "text {by_text:?}, value {by_value:?}");
    let label = cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        let cid = data.sketch.constraints().find(|c| c.value().is_some()).unwrap().id;
        let (_, pos, ..) = v.sketch.as_ref().unwrap().dimension_labels(&data, &v.doc.read(cx).session.doc.settings.units).into_iter().find(|l| l.0 == cid).unwrap();
        let vp = v.viewport.read(cx);
        let s = vp.project(v.sketch.as_ref().unwrap().frame.to_world(pos)).unwrap();
        vp.origin() + gpui::point(gpui::px(s.x as f32), gpui::px(s.y as f32))
    });
    assert_eq!(by_text.unwrap(), label);
    assert!(at(cx, serde_json::json!({ "dimension": "51" })).is_err(), "no 51 shown");
}

/// Live click-empty-plane-1123 (qa FEEL-CLICK-EMPTY-SELECTS-PLANE): in a sketch, a click on empty space inside the
/// sketch's own plane selects nothing: the selected point is cleared and the plane is not selected.
#[gpui::test]
fn a_click_on_empty_sketch_space_selects_nothing(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "click-empty-plane", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["escape", "escape"]);
    let end = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        let l = d.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
        d.sketch.endpoints(l).unwrap().1
    });
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.activate_tool(Tool::Select, w, cx);
            if let Some(ed) = &mut v.sketch {
                ed.selection = vec![end];
            }
            // The own plane under the pointer, as a GPU hover pick would give it.
            v.hover = Some(Reference::Plane(DatumPlane::Top));
        })
    })
    .unwrap();
    click(cx, window, &view, (300., 250.));
    let after = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().unwrap();
        (ed.selection.clone(), ed.model_selection.clone())
    });
    assert_eq!(after, (vec![], vec![]), "(sketch selection, model selection) after the click");
}

/// Live shortcuts-1123: F1 in a sketch opens help on the tool that is on, or the last one used (Concentric right after
/// Shift+O), and on the sketch tools before any.
#[gpui::test]
fn f1_in_a_sketch_opens_help_on_the_current_or_last_tool(cx: &mut TestAppContext) {
    static OPENED: parking_lot::Mutex<Vec<String>> = parking_lot::Mutex::new(Vec::new());
    noshape_app_core::help::set_opener(|section, _| OPENED.lock().push(section.to_string()));
    let (window, _view) = sketch_with(cx, "f1-help", Tool::Select);
    keys(cx, window, &["f1", "l", "f1", "escape", "f1", "shift-o", "escape", "f1"]);
    assert_eq!(*OPENED.lock(), ["Sketch tools", "Line", "Line", "Concentric"]);
}

/// Live sketch-tools-1123 text.md: with a new text's dialog open a canvas click starts and commits nothing (the
/// dialog stays, its field focused again); Enter types a new line; one Esc closes the dialog, the tool off, nothing
/// kept.
#[gpui::test]
fn the_text_dialog_closes_with_one_esc_and_survives_a_canvas_click(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "text-keys", Tool::Text);
    click(cx, window, &view, (400., 300.));
    click(cx, window, &view, (500., 340.));
    let state = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().map(|ed| (ed.text_dialog_open(), ed.tool)));
    assert_eq!(state(cx), Some((true, Tool::Text)), "the dialog open");
    click(cx, window, &view, (300., 200.));
    assert_eq!(state(cx), Some((true, Tool::Text)), "a canvas click keeps it");
    let focused = cx.update_window(window, |_, w, cx| view.read(cx).ui_ref().and_then(|u| u.text_area.as_ref().map(|t| t.0.clone())).is_some_and(|st| st.read(cx).focus_handle(cx).is_focused(w))).unwrap();
    assert!(focused, "the field focused again");
    keys(cx, window, &["a", "b", "enter", "c"]);
    assert_eq!(state(cx), Some((true, Tool::Text)), "Enter doesn't commit");
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().map(|ed| ed.text_string.clone())).as_deref(), Some("ab\nc"), "Enter types a new line");
    keys(cx, window, &["escape"]);
    assert_eq!(state(cx), Some((false, Tool::Select)), "one Esc: closed, the tool off");
    assert!(cx.update(|cx| view.read(cx).sketch_data(cx).is_some_and(|d| d.texts.is_empty())), "nothing kept");
}

/// Live sketch-tools-1123 chamfer.md (qa SKT-CHAMFER-BOX-B): Enter on the first distance's box opens the second,
/// focused, so the next keys type its value (4, then 6).
#[gpui::test]
fn a_chamfers_second_box_takes_the_keys_after_the_first(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "chamfer-box-b", Tool::Line);
    click(cx, window, &view, (400., 300.));
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (500., 400.));
    keys(cx, window, &["escape", "escape"]);
    let (window, view) = sketch_in_tool(cx, window, view, Tool::Chamfer);
    click(cx, window, &view, (500., 300.));
    keys(cx, window, &["enter", "4", "enter"]);
    assert!(cx.update(|cx| view.read(cx).dim_input.is_some()), "the second distance's box open");
    keys(cx, window, &["6", "enter"]);
    let mut values: Vec<i64> = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        d.sketch.constraints().filter(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Length { curve, .. } if d.sketch.entity(curve).is_some_and(|e| e.construction))).filter_map(|c| c.kind.value()).map(|x| (x * 1e6).round() as i64).collect()
    });
    values.sort();
    assert_eq!(values, vec![4000, 6000], "the chamfer's distances (µm)");
    assert!(cx.update(|cx| view.read(cx).dim_input.is_none()), "the second Enter closes the box");
}
