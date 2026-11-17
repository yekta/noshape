//! Box select in sketch mode, iso, the default planes behind the sketch (live2 "USER P0, 11-17 12:2x" Part 2, "Box
//! select in iso over the planes"): a press on a plane and a drag draws the box (the plane doesn't drag); left → right
//! round the whole rectangle takes its 4 lines (no region, no planes, no points); right → left over its right half
//! takes the bottom, right and top lines (not the left).

use super::feel::{Pointer, gpu_pointer};
use super::*;

/// The open sketch's lines: id and both ends on screen (viewport px).
fn lines(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<(u32, DVec2, DVec2)> {
    cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let vp = v.viewport.read(cx);
        data.sketch
            .entities()
            .filter_map(|e| match data.sketch.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => Some((e.id.0, vp.project(ed.frame.to_world(a))?, vp.project(ed.frame.to_world(b))?)),
                _ => None,
            })
            .collect()
    })
}

fn drag(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, pointer: &Pointer, from: DVec2, to: DVec2) {
    pointer.move_to(cx, view, from);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(pointer.window_point(cx, from), gpui_kit::MouseButton::Left, Modifiers::default());
    draw(cx, window);
    for i in 1..=12 {
        let at = from + (to - from) * (i as f64 / 12.0);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_move(pointer.window_point(cx, at), Some(gpui_kit::MouseButton::Left), Modifiers::default());
        pointer.frames(cx, view);
    }
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_up(pointer.window_point(cx, to), gpui_kit::MouseButton::Left, Modifiers::default());
    draw(cx, window);
}

fn selected(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> (Vec<u32>, Vec<Reference>) {
    cx.update(|cx| {
        let v = view.read(cx);
        let mut lines: Vec<u32> = v.sketch.as_ref().unwrap().selection.iter().map(|e| e.0).collect();
        lines.sort();
        (lines, v.selection.clone())
    })
}

/// (Not Esc: live's Esc leaves a sketch selection as it is, behaviours "Clicking empty space clears the selection".)
fn clear(cx: &mut TestAppContext, view: &Entity<PartStudioView>) {
    view.update(cx, |v, _| {
        if let Some(ed) = &mut v.sketch {
            ed.selection.clear();
        }
    });
}

#[gpui::test]
fn a_box_in_iso_over_the_planes_takes_lines_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-box-iso", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let all = lines(cx, &view);
    assert_eq!(all.len(), 4, "the rectangle's four lines");
    let ends: Vec<DVec2> = all.iter().flat_map(|(_, a, b)| [*a, *b]).collect();
    let lo = ends.iter().fold(DVec2::splat(f64::MAX), |m, p| m.min(*p));
    let hi = ends.iter().fold(DVec2::splat(f64::MIN), |m, p| m.max(*p));
    // The line ending furthest left (the short left edge in iso): a crossing box starting just right of it takes the
    // other three, as live's over the rectangle's right part.
    let right_end = |l: &(u32, DVec2, DVec2)| l.1.x.max(l.2.x);
    let (left, start_x) = all.iter().map(|l| (l.0, right_end(l) + 5.0)).min_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
    assert!(all.iter().filter(|l| l.0 != left).all(|l| right_end(l) > start_x + 10.0), "the other lines reach past the box's start");
    let mut want_right: Vec<u32> = all.iter().map(|l| l.0).filter(|id| *id != left).collect();
    want_right.sort();
    let mut want_all: Vec<u32> = all.iter().map(|l| l.0).collect();
    want_all.sort();
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    // Left → right, a window round the whole rectangle, from over the planes.
    drag(cx, window, &view, &pointer, lo - DVec2::splat(25.0), hi + DVec2::splat(25.0));
    let (got, model) = selected(cx, &view);
    if got != want_all || !model.is_empty() {
        wrong.push(format!("window round it: lines {got:?} (want {want_all:?}), else {model:?}"));
    }
    clear(cx, &view);
    // Right → left, crossing over the rectangle's right part.
    drag(cx, window, &view, &pointer, hi + DVec2::splat(25.0), DVec2::new(start_x, lo.y - 25.0));
    let (got, model) = selected(cx, &view);
    if got != want_right || !model.is_empty() {
        wrong.push(format!("crossing over its right part from x {start_x:.0}: lines {got:?} (want {want_right:?}), else {model:?}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live2 "feel (sketch)" (behaviours 39910): with no tool a click on a line adds it (A, then B: both), a click on a
/// selected line removes it, Shift+click toggles the same way, and Ctrl+click opens the sketch's context menu without
/// toggling; in iso with the planes behind, through the real pointer.
#[gpui::test]
fn sketch_clicks_toggle_and_ctrl_opens_the_menu_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-click-mods", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let all = lines(cx, &view);
    let (a, b) = (&all[0], &all[1]);
    let pointer = Pointer::new(cx, window, &view);
    let press = |cx: &mut TestAppContext, at: DVec2, modifiers: Modifiers| {
        pointer.move_to(cx, &view, at);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_click(pointer.window_point(cx, at), modifiers);
        draw(cx, window);
        selected(cx, &view).0
    };
    let sorted = |mut v: Vec<u32>| {
        v.sort();
        v
    };
    let (on_a, on_b) = ((a.1 + a.2) * 0.5, (b.1 + b.2) * 0.5);
    let shift = Modifiers { shift: true, ..Default::default() };
    let ctrl = Modifiers { control: true, ..Default::default() };
    let mut wrong = Vec::new();
    let mut row = |what: &str, got: Vec<u32>, want: Vec<u32>| {
        if got != want {
            wrong.push(format!("{what}: {got:?}, want {want:?}"));
        }
    };
    row("click A", press(cx, on_a, Modifiers::default()), vec![a.0]);
    row("then click B", press(cx, on_b, Modifiers::default()), sorted(vec![a.0, b.0]));
    row("click A again", press(cx, on_a, Modifiers::default()), vec![b.0]);
    row("Shift+click A", press(cx, on_a, shift), sorted(vec![a.0, b.0]));
    row("Shift+click B", press(cx, on_b, shift), vec![a.0]);
    row("Ctrl+click B", press(cx, on_b, ctrl), vec![a.0]);
    assert!(cx.update(|cx| view.read(cx).context_menu.is_some()), "Ctrl+click opened the sketch's context menu");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
