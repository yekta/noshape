//! A dimension's label dragged with the real pointer (live2 behaviours "Part 2b, constraints from a selection and
//! dimension label drags"): no tool, nothing selected first; a press on the label and a slow drag keep the text centred
//! on the pointer at every step, with no snap, and the dimension never enters the selection.

use super::feel::{Pointer, gpu_pointer};
use super::*;

/// The sketch's one dimension label, its centre in viewport px.
fn label_at(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Option<DVec2> {
    cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx)?;
        v.placed_dimension_labels(&data, cx).into_iter().next().map(|d| DVec2::new(d.at.0 as f64, d.at.1 as f64))
    })
}

#[gpui::test]
fn a_dimension_label_follows_the_pointer_without_a_selection(cx: &mut TestAppContext) {
    // (The labels' hit areas come with the viewport's drawn frame.)
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = line_then_dimension_tool(cx, "label-drag");
    // The line's length: the line, then the label's place below it, then Enter in the value box.
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 440.));
    keys(cx, window, &["enter"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let start = label_at(cx, &view).expect("the length's label");
    let pointer = Pointer::new(cx, window, &view);
    pointer.move_to(cx, &view, start);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_move(pointer.window_point(cx, start), None, Modifiers::default());
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(pointer.window_point(cx, start), gpui_kit::MouseButton::Left, Modifiers::default());
    draw(cx, window);
    let mut off = Vec::new();
    // Live's slow drag: 40 px across the line, 60 px along it, then back to 2 px from the press (live sketch-drag-1125:
    // a drag starts by 4 px from the press; from then on the label goes wherever the pointer goes).
    let out: Vec<DVec2> = (1..=10).map(|i| DVec2::new(0.0, 4.0 * i as f64)).chain((1..=10).map(|i| DVec2::new(-6.0 * i as f64, 40.0))).collect();
    let back: Vec<DVec2> = (1..=10).map(|i| DVec2::new(-60.0, 40.0) * (1.0 - i as f64 / 10.0) + DVec2::new(0.0, 2.0) * (i as f64 / 10.0)).collect();
    for d in out.into_iter().chain(back) {
        let at = start + d;
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_move(pointer.window_point(cx, at), Some(gpui_kit::MouseButton::Left), Modifiers::default());
        pointer.frames(cx, &view);
        let got = label_at(cx, &view).expect("the label");
        let want = at;
        if (got - want).length() > 1.5 {
            off.push(format!("pointer at {d:?} from the press: label {:.1} px off ({got:?} vs {want:?})", (got - want).length()));
        }
    }
    let end = start + DVec2::new(0.0, 2.0);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_up(pointer.window_point(cx, end), gpui_kit::MouseButton::Left, Modifiers::default());
    draw(cx, window);
    let (selected, constraints) = cx.update(|cx| {
        let ed = view.read(cx).sketch.as_ref().unwrap();
        (ed.selection.len(), ed.selected_constraints.len())
    });
    assert!(off.is_empty(), "the text centred on the pointer at every step:\n{}", off.join("\n"));
    assert_eq!((selected, constraints), (0, 0), "a label drag selects nothing");
}

/// Live2 Part 2 (behaviours "USER P0, 11-17 12:2x"): with no tool, the pointer on a dimension's label lights it tan,
/// a click selects it (orange), a double-click opens its value box.
#[gpui::test]
fn a_dimension_label_hovers_tan_selects_on_a_click_and_opens_on_a_double_click(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = line_then_dimension_tool(cx, "label-click");
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 440.));
    keys(cx, window, &["enter"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let at = label_at(cx, &view).expect("the length's label");
    let pointer = Pointer::new(cx, window, &view);
    pointer.move_to(cx, &view, at + DVec2::new(0.0, 60.0));
    pointer.move_to(cx, &view, at);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_move(pointer.window_point(cx, at), None, Modifiers::default());
    draw(cx, window);
    let hovered = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover_constraint);
    assert!(hovered.is_some(), "the label lit (tan) under the pointer");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_click(pointer.window_point(cx, at), Modifiers::default());
    draw(cx, window);
    let selected = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().selected_constraints.clone());
    assert_eq!(selected.to_vec(), vec![hovered.unwrap()], "a click selects the dimension (orange)");
    let p = pointer.window_point(cx, at);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    for count in [1, 2] {
        vcx.simulate_event(gpui_kit::MouseDownEvent { position: p, modifiers: Modifiers::default(), button: gpui_kit::MouseButton::Left, click_count: count, first_mouse: false });
        vcx.simulate_event(gpui_kit::MouseUpEvent { position: p, modifiers: Modifiers::default(), button: gpui_kit::MouseButton::Left, click_count: count });
    }
    draw(cx, window);
    let editing = cx.update(|cx| view.read(cx).dim_input.as_ref().map(|(c, _, _)| *c));
    assert_eq!(editing, hovered, "a double-click opens its value box");
    // (Live dimbox-1118, recheck b5b664aa: its value selected and lit blue; ours paints a selection only while the
    // field has the keyboard.)
    let lit = cx.update_window(window, |_, w, cx| view.read(cx).dim_input.as_ref().is_some_and(|(_, st, _)| st.read(cx).focus_handle(cx).is_focused(w) && !st.read(cx).selected_range().is_empty())).unwrap();
    assert!(lit, "the box has the keyboard, its value selected");
}

/// Live sk-hover-matrix-1123 (TL / TD columns): with the Line or Dimension tool on a dimension doesn't light under the
/// pointer (what is behind it does); with no tool it does.
#[gpui::test]
fn a_dimension_label_takes_no_hover_with_line_or_dimension_on(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = line_then_dimension_tool(cx, "label-tools");
    click(cx, window, &view, (570., 380.));
    click(cx, window, &view, (570., 440.));
    keys(cx, window, &["enter"]);
    let pointer = Pointer::new(cx, window, &view);
    let mut got = Vec::new();
    for tool in [Tool::Select, Tool::Line, Tool::Dimension] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(tool, w, cx))).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let at = label_at(cx, &view).expect("the length's label");
        pointer.move_to(cx, &view, at + DVec2::new(0.0, 30.0));
        pointer.move_to(cx, &view, at);
        pointer.frames(cx, &view);
        let lit = cx.update(|cx| (view.read(cx).viewport.read(cx).label_at(at).is_some(), view.read(cx).sketch.as_ref().is_some_and(|ed| ed.hover_constraint.is_some())));
        got.push((tool, lit));
    }
    assert_eq!(got, vec![(Tool::Select, (true, true)), (Tool::Line, (false, false)), (Tool::Dimension, (false, false))], "tool, (label hit, dimension lit)");
}
