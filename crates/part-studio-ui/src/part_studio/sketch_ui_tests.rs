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
    let session = crate::session::Session::create(&path, crate::user_units::new_document()).unwrap();
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
    keys(cx, window, &["3", "0", "enter", "2", "0", "enter"]);
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
                        ps.features.push(noshape_model::partstudio::Feature { id: FeatureId(id), type_id: ty.into(), name: name.into(), suppressed: false, params: Default::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None });
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
