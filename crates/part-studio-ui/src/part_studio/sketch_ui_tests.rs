//! UI keystroke tests of sketch mode on GPUI's test platform (docs/TESTS.md
//! "UI keystroke tests"): the real Part Studio view in a test window, the GPU
//! a no-op stand-in, clicks as the viewport's tool events, keys through GPUI.

use super::super::PartStudioView;
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::sketch_mode::Tool;
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

/// A Part Studio built from a ui-parity start (tests/ui-parity/starts.json's form), or a fresh one.
fn part_studio_from(cx: &mut TestAppContext, name: &str, start: Option<serde_json::Value>) -> (AnyWindowHandle, Entity<PartStudioView>) {
    cx.update(gpui_kit::init);
    // The app's default key bindings (Esc → Cancel, tool keys), without the
    // user's customizations.
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    // The value box's own keys (the app binds them once per process).
    cx.update(|cx| cx.bind_keys(super::entry_key_bindings()));
    crate::part_studio_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    let session = match start {
        Some(start) => {
            noshape_ops::ui_parity::build_start(&mut noshape_ops::Ctx::new(), &path, &start).unwrap();
            crate::session::Session::open(&path).unwrap()
        }
        None => crate::session::Session::create(&path, crate::user_units::new_document()).unwrap(),
    };
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let gpu = if RENDERING.get() { Gpu::headless_rendering().unwrap() } else { Gpu::headless().unwrap() };
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
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
    while !cx.update(|cx| done(cx)) && t.elapsed() < std::time::Duration::from_secs(30) {
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
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    keys(cx, window, &["enter"]);
    assert!(cx.update(|cx| view.read(cx).edit.is_none()), "the second Enter accepts");
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
    assert!(sketch.hidden, "the extruded sketch starts hidden");
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
        let (from, to) = (vp.project(tip + a.dir * 10.0 * wpp).unwrap(), vp.project(tip + a.dir * 40.0 * wpp).unwrap());
        ((from.x, from.y), (to.x, to.y))
    });
    drag(cx, window, &view, from, to, 4);
    view.update(cx, |v, _| {
        assert_eq!(v.ui.section_dialog.as_ref().map(|d| d.active), Some(0), "Top is now the active plane");
        assert!(v.ui.section[0].offset > 0.0, "Top moved up: {}", v.ui.section[0].offset);
        assert_eq!(v.ui.section[1].offset, 0.0, "Right stayed");
    });
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
        assert!((snap_degrees(dragged) - shown).abs() < 1e-9, "{dragged} → {}", snap_degrees(dragged));
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
    // The commit's regeneration (it waits out a burst's 12 ms first).
    wait_until(cx, window, |cx| view.read(cx).context.as_ref().is_some_and(|c| !c.bodies.is_empty()));
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
    let drawn = cx.update(|cx| view.read(cx).ui.consumed_lines.as_ref().map(|c| c.4.iter().map(|i| (matches!(i.drawable, noshape_render::Drawable::Mesh(_)), i.color[3])).collect::<Vec<_>>()));
    assert_eq!(drawn, Some(vec![(true, 0.47), (false, 1.0)]), "the removed face's fill, then its outline");
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
        let scene = crate::scene_builder::with_highlights(built, &v.display, std::slice::from_ref(&top), None, &[], Vec::new(), false, v.gpu.uploader());
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
    view.update(cx, |v, cx| {
        v.ui.pointer = v.viewport.read(cx).project(corner);
        v.ui.hover_pick = Some(noshape_scene::builder::PickTarget { reference: top.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None });
        v.update_inference(false, cx);
    });
    wait_until(cx, window, |cx| view.read(cx).inference_choice().is_some());
    let chosen = cx.update(|cx| view.read(cx).inference_choice().map(|p| p.frame.origin)).unwrap();
    assert!(chosen.distance(corner) < 1e-6, "{chosen} for the corner {corner}");
    view.update(cx, |v, cx| v.pick(noshape_scene::builder::PickTarget { reference: top.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None }, cx));
    let dialog = view.update(cx, |v, _| v.edit.as_ref().expect("the Mate connector dialog").dialog.clone());
    let (origin, inference, secondary) = cx.update(|cx| {
        let p = &dialog.read(cx).feature.params;
        (p.query("originQuery").to_vec(), p.enum_("entityInferenceType").to_string(), p.query("secondaryOriginQuery").to_vec())
    });
    assert_eq!((origin, inference.as_str()), (vec![top], "POINT"));
    assert!(matches!(secondary.as_slice(), [Reference::Vertex(_)]), "{secondary:?}");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).display.connectors.is_empty());
    let (o, z) = cx.update(|cx| view.read(cx).display.connectors.first().map(|c| (c.origin, c.z))).unwrap();
    assert!(o.distance(corner) < 1e-6 && z.distance(glam::DVec3::Z) < 1e-9, "{o} {z}");
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
    // One run's test binaries share the settings directory (tools/tiers): the remembered options go with the test,
    // or later exports through the agent API would take them.
    struct Forget(std::path::PathBuf);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _forget = Forget(noshape_scene::config_dir().join("export_solid.json"));
    let _ = std::fs::remove_file(noshape_scene::config_dir().join("export_solid.json"));
    let d = ExportDialog { window, exported: Default::default() };
    d.open(cx, false);
    let first = d.export(cx);
    // (The units held are STEP's, unseen until its custom units are ticked: Centimeter, live qexport.)
    assert_eq!((first.format, first.latest, first.resolution, first.units), (PartFormat::Parasolid, true, 1, 0), "live's first Export: Parasolid, latest version");

    d.open(cx, false);
    d.pick(cx, "ex-format", "STEP");
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
/// pointer gives the plane's rows; the sketch's own plane is empty space.
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
    assert!(!own.iter().any(|l| l.starts_with("Hide ")), "{own:?}");
}

/// Lead 11-06, the display cache: a regenerated display comes back from the cache as it was; shown in its place the
/// view is view-only (a dialog asked for waits, the user told at once, "Regenerating…" in the list), and the real
/// regeneration replaces it with the same meshes and opens the dialog.
#[gpui::test]
fn a_cached_display_stands_in_until_the_identical_real_one_lands(cx: &mut TestAppContext) {
    let (window, view, _top, _) = block_part(cx, "display-cache");
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
    super::super::display_cache::store(&dir, "k", &real).unwrap();
    let cached = super::super::display_cache::load(&dir, "k").expect("the cached display");
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
    super::super::display_cache::store(&dir, "k", &real).unwrap();
    let cached = super::super::display_cache::load(&dir, "k").expect("the cached display");
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
    super::super::display_cache::store(&dir, "k", &real).unwrap();
    let cached = super::super::display_cache::load(&dir, "k").expect("the cached display");
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
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
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
        // It shows the plate's sketch side, the face on Top (the flat is the plate turned over: live 2468, the sketch
        // side faces +Z): the face the main view lights for it.
        let shown = noshape_model::sheetmetal::flat_face_counterpart(ctx, &noshape_model::sheetmetal::flat_face_name(&part, top)).expect("the part's face");
        let Reference::Face(n) = &shown else { panic!("{shown:?}") };
        let body = ctx.body(&part).unwrap();
        let fi = *body.find_faces(n).first().expect("on the part");
        let pl = body.shape.faces()[fi].face_plane().unwrap().unwrap();
        assert!(pl.unit_normal().z < -0.999 && pl.origin.z.abs() < 1e-9, "{pl:?}");
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
    let at = |cx: &mut TestAppContext, x: f64, y: f64| cx.update(|cx| fvp.read(cx).project(glam::DVec3::new(x * 1e-3, y * 1e-3, 2e-3))).expect("on screen");
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
