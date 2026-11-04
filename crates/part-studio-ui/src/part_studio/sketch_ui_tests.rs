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
            edit(&mut d);
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
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.show_dimensions(sketch, w, cx))).unwrap();
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

/// core-workflow 8.1.15 / 8.6.4: one Esc cancels the appearance editor, its hex field focused (live request 1663 (a)).
#[gpui::test]
fn escape_cancels_the_appearance_editor(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "appearance-esc");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(super::super::appearance::EditTarget::Faces(vec![], None), w, cx))).unwrap();
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
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_appearance(super::super::appearance::EditTarget::Faces(vec![], None), w, cx))).unwrap();
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
