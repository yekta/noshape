//! Variable Studio editing feel through real keystrokes (live vs-fs-feel-1117; qa VSFEEL-*, user P0 11-17).

use super::{VariableStudioView, foreign_length, invalid_name};
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Keystroke, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};
use serde_json::json;

struct Host(Entity<VariableStudioView>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().flex().flex_col().child(self.0.clone())
    }
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
}

fn keys(cx: &mut TestAppContext, window: AnyWindowHandle, keys: &[&str]) {
    for key in keys {
        cx.update_window(window, |_, w, cx| {
            let mut ks = Keystroke::parse(key).unwrap();
            if key.len() == 1 {
                ks.key_char = Some((*key).into());
            }
            w.dispatch_keystroke(ks, cx);
        })
        .unwrap();
        draw(cx, window);
    }
}

/// A Variable Studio with one variable `w`, its add row's Name focused.
fn studio(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<VariableStudioView>) {
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-variables-feel-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("variables.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = noshape_ops::Ctx::new();
        let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call("document_create", json!({ "path": path.to_str().unwrap() }));
        let tab = call("tab_create", json!({ "kind": "variablestudio" }));
        let tab = tab["tab"]["name"].as_str().unwrap().to_string();
        call("variable_create", json!({ "tab": tab, "name": "w", "value": "10 mm" }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::VariableStudio(_))).unwrap().id;
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| VariableStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Host(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        let st = view.read(cx).inputs.get("new").map(|(st, _)| st.clone()).expect("the add row");
        st.update(cx, |s, cx| s.focus(w, cx));
    })
    .unwrap();
    draw(cx, window);
    (window, view)
}

fn add_row_text(cx: &mut TestAppContext, view: &Entity<VariableStudioView>) -> String {
    cx.update(|cx| view.read(cx).inputs.get("new").map(|(st, _)| st.read(cx).value().to_string()).unwrap_or_default())
}

fn names(cx: &mut TestAppContext, view: &Entity<VariableStudioView>) -> Vec<String> {
    cx.update(|cx| view.read(cx).studio(cx).map(|s| s.variables.iter().map(|v| v.name.clone()).collect()).unwrap_or_default())
}

/// Live fsvs-feel-1118 (lead 11-18): an invalid name isn't marked while it is typed; Enter adds nothing, keeps the text,
/// and 526 ms later the cell turns pink; typing on or Esc (which throws the text away) unmarks it.
#[gpui::test]
fn an_invalid_name_turns_pink_526_ms_after_enter_not_while_typed(cx: &mut TestAppContext) {
    for (typed, bad) in [("10", true), ("1x", true), ("10 +", true), ("w2", false), ("#w2", false), ("", false)] {
        assert_eq!(invalid_name(typed), bad, "{typed:?}");
    }
    let (window, view) = studio(cx, "name-pink");
    let marked = |cx: &mut TestAppContext| {
        let text = add_row_text(cx, &view);
        cx.update(|cx| view.read(cx).name_error.clone().is_some_and(|t| t == text))
    };
    keys(cx, window, &["1", "2"]);
    assert_eq!(add_row_text(cx, &view), "12");
    assert!(!marked(cx), "not while typed");
    keys(cx, window, &["enter"]);
    assert_eq!(names(cx, &view), ["w"], "Enter adds nothing");
    assert_eq!(add_row_text(cx, &view), "12", "the text stays in the cell");
    cx.executor().advance_clock(std::time::Duration::from_millis(400));
    draw(cx, window);
    assert!(!marked(cx), "400 ms after Enter: not yet");
    cx.executor().advance_clock(std::time::Duration::from_millis(200));
    draw(cx, window);
    assert!(marked(cx), "600 ms after Enter: pink");
    keys(cx, window, &["escape"]);
    assert_eq!(add_row_text(cx, &view), "", "Esc throws the text away");
    assert!(!marked(cx));
    assert_eq!(names(cx, &view), ["w"]);
}

/// Live vs-fs-feel-1117 (qa VSFEEL-UNITS): a length written in another unit shows in the document's: "2 in" in a
/// millimetre document reads 50.8 mm; one in the document's unit, an angle or an expression shows as it is.
#[test]
fn a_length_in_another_unit_shows_in_the_documents() {
    let mm = noshape_units::UnitPrefs::default();
    for (text, foreign) in [("2 in", true), ("1 m", true), ("10 mm", false), ("10", false), ("10 deg", false), ("#w * 2", false), ("2 in + 1 mm", false)] {
        assert_eq!(foreign_length(text, &mm), foreign, "{text:?}");
    }
    let q = noshape_units::evaluate("2 in", &noshape_units::EmptyScope).expect("2 in");
    assert_eq!(noshape_units::format_quantity(q, &mm), "50.8 mm");
    let inch = noshape_units::UnitPrefs { length: noshape_units::LengthUnit::Inch, ..mm };
    assert!(!foreign_length("2 in", &inch) && foreign_length("10 mm", &inch));
}

/// Live vs-fs-feel-1117 v2-06 (qa VSFEEL): a value cell's tooltip ("10 mm", or "Error regenerating.") is centred over
/// the cell, 4 px above it on the column header, not at the pointer.
#[gpui::test]
fn a_value_cells_tooltip_sits_over_the_cell(cx: &mut TestAppContext) {
    let (window, _view) = studio(cx, "value-tip");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let cell = vcx.debug_bounds("vs-val-0").expect("the value cell");
    vcx.simulate_mouse_move(gpui_kit::point(cell.left() + px(10.), cell.center().y), None, gpui_kit::Modifiers::default());
    for _ in 0..4 {
        vcx.executor().advance_clock(std::time::Duration::from_millis(300));
        draw(&mut vcx, window);
    }
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("10 mm"));
    let tip = vcx.debug_bounds("placed-tooltip").expect("the tooltip");
    assert!((tip.bottom() - (cell.top() - px(4.))).abs() < px(1.), "4 px above the cell: {tip:?} {cell:?}");
    assert!((tip.center().x - cell.center().x).abs() < px(1.), "centred on it, not on the pointer: {tip:?} {cell:?}");
}

/// Live behaviours "feel coverage (11-20)" V2: while a value cell is edited, its old value sits in a grey chip over the
/// row above (26 px, 2 px above the cell, its right 46 px in from the cell's), whatever is typed; gone after the edit.
#[gpui::test]
fn an_edited_value_shows_its_old_value_in_a_chip(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "old-value");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let cell = vcx.debug_bounds("vs-val-0").expect("the value cell");
    assert!(vcx.debug_bounds("vs-old-value").is_none(), "no chip at rest");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_cell_edit("e0", None, w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["#"]);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let chip = vcx.debug_bounds("vs-old-value").expect("the chip while editing");
    assert!((chip.size.height - px(26.)).abs() < px(0.5) && (chip.bottom() - (cell.top() - px(2.))).abs() < px(1.), "{chip:?} {cell:?}");
    assert!((chip.right() - (cell.right() - px(46.))).abs() < px(1.5), "{chip:?} {cell:?}");
    keys(cx, window, &["escape", "escape"]);
    assert!(gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("vs-old-value").is_none(), "gone after the edit");
}

/// Live2 errors-1118: Enter on a value that fails ("1 +") keeps that cell selected with its red border; a good value
/// moves down a row as before.
#[gpui::test]
fn enter_on_a_failing_value_keeps_its_cell_selected(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "enter-failing");
    keys(cx, window, &["h", "enter"]);
    assert_eq!(names(cx, &view), ["w", "h"], "a second row");
    let enter_value = |cx: &mut TestAppContext, text: &str| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.start_cell_edit("e0", None, w, cx);
                let st = v.inputs.get("e0").map(|(st, _)| st.clone()).expect("the value cell");
                st.update(cx, |s, cx| s.set_value(text.to_string(), w, cx));
            })
        })
        .unwrap();
        draw(cx, window);
        keys(cx, window, &["enter"]);
        cx.update(|cx| view.read(cx).cell_selected.clone())
    };
    assert_eq!(enter_value(cx, "1 +").as_deref(), Some("e0"), "the failing cell stays selected");
    assert_eq!(enter_value(cx, "12 mm").as_deref(), Some("e1"), "a good value moves down a row");
}

/// Live2 session-studio-1119 (step 2): a name typed in the add row then Tab adds the variable (Length, 0) as Enter does
/// and the keyboard stays in the next empty Name, so the next typing starts a new name (ours kept "depth" and made
/// "depth20").
#[gpui::test]
fn tab_after_a_new_name_adds_the_variable(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "tab-adds");
    cx.update(super::bind_keys);
    keys(cx, window, &["d", "tab"]);
    assert_eq!(names(cx, &view), ["w", "d"], "Tab added d");
    keys(cx, window, &["h", "enter"]);
    assert_eq!(names(cx, &view), ["w", "d", "h"], "the next typing was a new name");
}
