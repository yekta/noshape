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

/// Live vsm-1 v10 (qa FEEL-VS-CHIP-ON-CLICK): a value cell's chip stays when the cell is clicked with the pointer kept
/// there.
#[gpui::test]
fn a_value_cells_tooltip_stays_on_a_click(cx: &mut TestAppContext) {
    let (window, _view) = studio(cx, "value-tip-click");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let cell = vcx.debug_bounds("vs-val-0").expect("the value cell");
    let at = gpui_kit::point(cell.left() + px(10.), cell.center().y);
    vcx.simulate_mouse_move(at, None, gpui_kit::Modifiers::default());
    for _ in 0..4 {
        vcx.executor().advance_clock(std::time::Duration::from_millis(300));
        draw(&mut vcx, window);
    }
    assert!(vcx.debug_bounds("placed-tooltip").is_some(), "shown on hover");
    vcx.simulate_click(at, gpui_kit::Modifiers::default());
    for _ in 0..3 {
        vcx.executor().advance_clock(std::time::Duration::from_millis(300));
        draw(&mut vcx, window);
    }
    assert!(vcx.debug_bounds("vs-val-0").is_some());
    assert!(vcx.debug_bounds("placed-tooltip").is_some(), "kept after the click");
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

/// A press and release at `at` as the `count`-th click.
fn click_at(cx: &mut TestAppContext, window: AnyWindowHandle, at: Point<gpui_kit::Pixels>, count: usize) {
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let none = gpui_kit::Modifiers::default();
    vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: none, click_count: count, first_mouse: false });
    vcx.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: none, click_count: count });
    draw(cx, window);
}

/// Live feel cov-1120 V1: a double-click on a Value cell edits it with its whole text selected; Esc reverts and keeps
/// the cell selected; an edit is committed by a click on another cell; a second single click on the selected cell
/// doesn't edit.
#[gpui::test]
fn a_double_clicked_value_edits_all_selected_esc_reverts_a_click_away_commits(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "value-dblclick");
    // (An active window: a click away blurs the editor, which commits it.)
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    keys(cx, window, &["h", "enter"]);
    let cell = |cx: &mut TestAppContext, i: usize| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(["vs-val-0", "vs-val-1"][i]).expect("the value cell").center();
    let state = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let input = v.inputs.get("e0").map(|(st, _)| (st.read(cx).value().to_string(), st.read(cx).selected_range()));
            (v.cell_editing.clone(), v.cell_selected.clone(), input)
        })
    };
    let w = cell(cx, 0);
    click_at(cx, window, w, 1);
    click_at(cx, window, w, 2);
    assert_eq!(state(cx), (Some("e0".into()), Some("e0".into()), Some(("10 mm".into(), 0..5))), "(editing, selected, (text, selection)) after the double-click");
    keys(cx, window, &["2"]);
    keys(cx, window, &["escape"]);
    let (editing, selected, input) = state(cx);
    assert_eq!((editing, selected, input.map(|i| i.0)), (None, Some("e0".into()), Some("10 mm".into())), "(editing, selected, text) after Esc");
    // (h's edit, the row below: the unit list opened under it covers no cell above.)
    let text = |cx: &mut TestAppContext, i: usize| cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.variables.get(i).map(|v| v.expr.text.clone())));
    let h_was = text(cx, 1).expect("h's value");
    let h = cell(cx, 1);
    click_at(cx, window, h, 1);
    click_at(cx, window, h, 2);
    keys(cx, window, &["end", " ", "+", " ", "1", " ", "m", "m"]);
    let w = cell(cx, 0);
    click_at(cx, window, w, 1);
    assert_eq!((text(cx, 1), state(cx).0), (Some(format!("{h_was} + 1 mm")), None), "(h, editing) after a click on w's value");
    cx.executor().advance_clock(std::time::Duration::from_millis(900));
    let w = cell(cx, 0);
    click_at(cx, window, w, 1);
    assert_eq!(state(cx).0, None, "a second single click on the selected cell edits it");
}

/// Live fv-1121 vs1-click / vs3-enter: the column header over the focused cell turns grey, and stays grey while it's
/// edited; no header is grey once nothing in the table has the keyboard.
#[gpui::test]
fn the_focused_cells_column_header_turns_grey(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "header-focus");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    let active = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| view.read(cx).active_column(w)).unwrap();
    assert_eq!(active(cx), None, "nothing focused");
    let at = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("vs-val-0").expect("the value cell").center();
    click_at(cx, window, at, 1);
    assert_eq!(active(cx), Some(2), "a click on w's value: the Value header");
    keys(cx, window, &["enter"]);
    assert_eq!(cx.update(|cx| view.read(cx).cell_editing.clone()), Some("e0".into()), "Enter edits it");
    assert_eq!(active(cx), Some(2), "still the Value header while edited");
    keys(cx, window, &["escape", "left"]);
    assert_eq!(active(cx), Some(1), "the left arrow: the Variable type header");
    assert_eq!(crate::theme::p().table_header_focus, gpui_kit::rgb(0xd7d7d7), "live's grey");
}

/// Live shortcuts-1123 vsfoc: a letter typed in a focused Value cell starts its edit with the list open ("a": abs(x)).
#[gpui::test]
fn a_letter_in_a_focused_value_cell_opens_the_list(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "value-letter");
    keys(cx, window, &["h", "enter"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_cell_edit("e0", Some("a".into()), w, cx))).unwrap();
    draw(cx, window);
    let got = cx.update(|cx| {
        let v = view.read(cx);
        let abs = v.completion.as_ref().is_some_and(|c| c.entries.iter().any(|e| matches!(e, crate::var_complete::Entry::Function("abs", _))));
        (v.cell_editing.clone(), abs)
    });
    assert_eq!(got, (Some("e0".to_string()), true), "editing, abs( listed");
}

/// Live feel cov-1120 V2: "#" in a Value cell lists only the variables above its row, with no "New variable…".
#[gpui::test]
fn the_value_cells_hash_list_holds_the_variables_above_only(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "value-hash");
    keys(cx, window, &["h", "enter"]);
    keys(cx, window, &["d", "enter"]);
    let list = |cx: &mut TestAppContext, key: &str| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_cell_edit(key, None, w, cx))).unwrap();
        draw(cx, window);
        keys(cx, window, &["backspace", "#"]);
        let got = cx.update(|cx| {
            view.read(cx).completion.as_ref().map(|c| {
                let vars: Vec<String> = c.entries.iter().filter_map(|e| match e {
                    crate::var_complete::Entry::Variable(n) => Some(n.clone()),
                    _ => None,
                }).collect();
                // (Live's take, "#a" over "#b", doesn't tell the table's order from the alphabet's.)
                let mut vars = vars;
                vars.sort();
                (vars, c.new_variable.clone())
            })
        });
        keys(cx, window, &["escape"]);
        got
    };
    assert_eq!(list(cx, "e1"), Some((vec!["w".to_string()], None)), "h's row: (variables, New variable…)");
    assert_eq!(list(cx, "e2"), Some((vec!["h".to_string(), "w".to_string()], None)), "d's row: (variables, New variable…)");
}

/// qa workflow 15 (VS-NAME-FOCUS-BOX): the add row's empty Name cell is outlined blue while it has the focus, after
/// the click and again after Enter adds the variable.
#[gpui::test]
fn the_focused_add_row_name_is_outlined(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "name-outline");
    let outlined = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| view.read(cx).add_name_focused(w, cx)).unwrap();
    assert!(outlined(cx), "focused: outlined");
    keys(cx, window, &["d", "enter"]);
    assert_eq!(names(cx, &view), ["w", "d"], "Enter adds it");
    assert!(outlined(cx), "after Enter: outlined again");
}

/// Live vs-rowdrag-1122: a row drags from 6 px (not 5) as a pill with its full name 10 px right of the pointer and 5 px
/// below; Esc drops the drag (the release moves nothing); released over the first row's top, the row goes first.
#[gpui::test]
fn a_row_drags_from_6_px_as_a_named_pill_and_esc_cancels(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, MouseButton};
    let (window, view) = studio(cx, "row-drag");
    keys(cx, window, &["d", "enter"]);
    assert_eq!(names(cx, &view), ["w", "d"]);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let (first, last) = (vcx.debug_bounds("vs-val-0").expect("row w"), vcx.debug_bounds("vs-val-1").expect("row d"));
    let start = gpui_kit::point(last.left() - px(30.), last.center().y);
    let top = gpui_kit::point(start.x, first.top() + px(2.));
    let drag = |vcx: &mut gpui_kit::VisualTestContext, esc: bool| {
        vcx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        vcx.simulate_mouse_move(start + gpui_kit::point(px(0.), px(-5.)), Some(MouseButton::Left), Modifiers::default());
        vcx.run_until_parked();
        assert!(vcx.debug_bounds("vs-row-ghost").is_none(), "5 px: no drag yet");
        vcx.simulate_mouse_move(start + gpui_kit::point(px(0.), px(-6.)), Some(MouseButton::Left), Modifiers::default());
        vcx.simulate_mouse_move(top, Some(MouseButton::Left), Modifiers::default());
        vcx.run_until_parked();
        let ghost = vcx.debug_bounds("vs-row-ghost").expect("the ghost from 6 px");
        assert!((ghost.left() - (top.x + px(10.))).abs() < px(1.) && (ghost.top() - (top.y + px(5.))).abs() < px(1.), "beside the pointer: {ghost:?} {top:?}");
        assert!((ghost.size.height - px(31.)).abs() < px(0.5), "31 px high: {ghost:?}");
        if esc {
            vcx.simulate_keystrokes("escape");
            assert!(vcx.debug_bounds("vs-row-ghost").is_none(), "Esc: the ghost gone");
        }
        vcx.simulate_mouse_up(top, MouseButton::Left, Modifiers::default());
        vcx.run_until_parked();
    };
    drag(&mut vcx, true);
    assert_eq!(names(cx, &view), ["w", "d"], "after Esc the release moves nothing");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.executor().advance_clock(std::time::Duration::from_millis(600));
    drag(&mut vcx, false);
    assert_eq!(names(cx, &view), ["d", "w"], "dropped over the first row's top");
}

/// Live drw-matrix-1121 vsm-1, shortcuts-1123, fv-1121: a click selects one cell; a Shift- or Ctrl-click on another
/// moves the selection there (no range, no multi-select); Esc keeps it; Space starts its edit as Enter does; Tab
/// commits and moves right, Shift+Tab left.
#[gpui::test]
fn a_cell_selection_moves_on_shift_or_ctrl_click_and_space_edits(cx: &mut TestAppContext) {
    let (window, view) = studio(cx, "cell-select-keys");
    cx.update(super::bind_keys);
    keys(cx, window, &["d", "enter"]);
    assert_eq!(names(cx, &view), ["w", "d"]);
    let selected = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).cell_selected.clone());
    let editing = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).cell_editing.clone());
    let cell = |cx: &mut TestAppContext, i: usize| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("vs-val-{i}").into_boxed_str())).expect("the value cell").center();
    let (v0, v1) = (cell(cx, 0), cell(cx, 1));
    click_at(cx, window, v0, 1);
    assert_eq!(selected(cx), Some("e0".into()), "a click selects w's value");
    for m in [gpui_kit::Modifiers::shift(), gpui_kit::Modifiers::secondary_key()] {
        let to = if selected(cx).as_deref() == Some("e0") { v1 } else { v0 };
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(to, m);
        draw(cx, window);
        assert_eq!(selected(cx), Some(if to == v1 { "e1".into() } else { "e0".into() }), "{m:?}-click moves the selection");
    }
    assert_eq!(selected(cx), Some("e0".into()));
    keys(cx, window, &["escape"]);
    assert_eq!((selected(cx), editing(cx)), (Some("e0".into()), None), "Esc keeps the selected cell");
    keys(cx, window, &["space"]);
    assert_eq!(editing(cx), Some("e0".into()), "Space edits it");
    keys(cx, window, &["tab"]);
    assert_eq!((editing(cx), selected(cx)), (None, Some("d0".into())), "Tab commits and moves right");
    keys(cx, window, &["shift-tab"]);
    assert_eq!(selected(cx), Some("e0".into()), "Shift+Tab moves left");
}
