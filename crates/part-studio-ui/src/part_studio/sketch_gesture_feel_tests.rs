//! Selection gestures in a sketch (live psui/sel-gestures-1122, the sketch column: rectangle -50..-20 × -40..-10 and a
//! circle on Top, iso): a window over the rectangle's right part takes only the line wholly inside; a box adds to a
//! selected circle (window: + 4 lines, crossing: + the 3 lines it touches) and a Ctrl window takes them out again; ` over
//! a line lists the line first, then what lies behind it.

use super::feel::{Pointer, gpu_pointer};
use super::*;

type Line = (u32, DVec2, DVec2);

/// The open sketch's lines (id, both ends on screen) and its circle (id, a point on it on screen).
fn shapes(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> (Vec<Line>, Option<(u32, DVec2)>) {
    cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let vp = v.viewport.read(cx);
        let mut lines = Vec::new();
        let mut circle = None;
        for e in data.sketch.entities() {
            match data.sketch.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => lines.extend(vp.project(ed.frame.to_world(a)).zip(vp.project(ed.frame.to_world(b))).map(|(a, b)| (e.id.0, a, b))),
                Some(noshape_sketch::Curve2::Circle { center, radius }) => circle = vp.project(ed.frame.to_world(center - DVec2::new(radius, 0.0))).map(|p| (e.id.0, p)),
                _ => {}
            }
        }
        (lines, circle)
    })
}

fn drag(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, pointer: &Pointer, from: DVec2, to: DVec2, keys: Modifiers) {
    pointer.move_to(cx, view, from);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(pointer.window_point(cx, from), gpui_kit::MouseButton::Left, keys);
    draw(cx, window);
    for i in 1..=12 {
        let at = from + (to - from) * (i as f64 / 12.0);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_move(pointer.window_point(cx, at), Some(gpui_kit::MouseButton::Left), keys);
        pointer.frames(cx, view);
    }
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_up(pointer.window_point(cx, to), gpui_kit::MouseButton::Left, keys);
    draw(cx, window);
}

fn selected(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<u32> {
    let mut ids: Vec<u32> = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().selection.iter().map(|e| e.0).collect());
    ids.sort();
    ids
}

fn select_only(cx: &mut TestAppContext, view: &Entity<PartStudioView>, ids: &[u32]) {
    view.update(cx, |v, _| {
        if let Some(ed) = &mut v.sketch {
            ed.selection = ids.iter().map(|i| noshape_sketch::EntityId(*i)).collect();
        }
    });
}

/// A rectangle and, left of it, a circle; the Select tool.
fn rectangle_and_circle(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = sketch_with(cx, name, Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Circle, w, cx))).unwrap();
    // (Off the Origin's level: no inferred Origin point.)
    click(cx, window, &view, (330., 215.));
    click(cx, window, &view, (360., 215.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    (window, view)
}

#[gpui::test]
fn sketch_boxes_take_whats_inside_and_add_to_the_selection_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = rectangle_and_circle(cx, "sketch-gesture-boxes");
    let (lines, circle) = shapes(cx, &view);
    assert_eq!(lines.len(), 4, "the rectangle's four lines");
    let (circle, _) = circle.expect("the circle");
    let ends: Vec<DVec2> = lines.iter().flat_map(|(_, a, b)| [*a, *b]).collect();
    let lo = ends.iter().fold(DVec2::splat(f64::MAX), |m, p| m.min(*p));
    let hi = ends.iter().fold(DVec2::splat(f64::MIN), |m, p| m.max(*p));
    let left_end = |l: &(u32, DVec2, DVec2)| l.1.x.min(l.2.x);
    let right_end = |l: &(u32, DVec2, DVec2)| l.1.x.max(l.2.x);
    // The line starting furthest right: a window from just left of it holds only that one.
    let (right, window_x) = lines.iter().map(|l| (l.0, left_end(l) - 5.0)).max_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
    assert!(lines.iter().filter(|l| l.0 != right).all(|l| left_end(l) < window_x - 10.0), "the other lines start left of the window");
    // The line ending furthest left: a crossing from just right of it touches the other three.
    let (left, crossing_x) = lines.iter().map(|l| (l.0, right_end(l) + 5.0)).min_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
    let with = |mut ids: Vec<u32>, more: &[u32]| {
        ids.extend(more);
        ids.sort();
        ids
    };
    let all: Vec<u32> = lines.iter().map(|l| l.0).collect();
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    let mut row = |what: &str, got: Vec<u32>, want: Vec<u32>| {
        if got != want {
            wrong.push(format!("{what}: {got:?}, want {want:?}"));
        }
    };
    let (top, bottom) = (lo.y - 25.0, hi.y + 25.0);
    select_only(cx, &view, &[]);
    drag(cx, window, &view, &pointer, DVec2::new(window_x, top), DVec2::new(hi.x + 25.0, bottom), Modifiers::default());
    row("window over the right part", selected(cx, &view), vec![right]);
    select_only(cx, &view, &[circle]);
    drag(cx, window, &view, &pointer, lo - DVec2::splat(25.0), hi + DVec2::splat(25.0), Modifiers::default());
    row("window round it, the circle selected", selected(cx, &view), with(all.clone(), &[circle]));
    let ctrl = Modifiers { control: true, ..Default::default() };
    drag(cx, window, &view, &pointer, lo - DVec2::splat(25.0), DVec2::new(hi.x + 25.0, bottom), ctrl);
    row("Ctrl window round the rectangle, all selected", selected(cx, &view), vec![circle]);
    select_only(cx, &view, &[circle]);
    drag(cx, window, &view, &pointer, DVec2::new(hi.x + 25.0, top), DVec2::new(crossing_x, bottom), Modifiers::default());
    row("crossing over the right part, the circle selected", selected(cx, &view), with(all.iter().copied().filter(|i| *i != left).collect(), &[circle]));
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live (16-backtick): ` over a rectangle's line in a sketch lists "Edge of Sketch 3" first, then what lies behind it
/// along the ray ("Right plane", "Face of Sketch 3" twice, "Front plane").
#[gpui::test]
fn backtick_over_a_sketch_line_lists_the_line_first(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = rectangle_and_circle(cx, "sketch-gesture-backtick");
    let (lines, _) = shapes(cx, &view);
    let l = lines.iter().max_by(|x, y| (x.1.x + x.2.x).total_cmp(&(y.1.x + y.2.x))).unwrap();
    let pointer = Pointer::new(cx, window, &view);
    let at = (l.1 + l.2) * 0.5;
    pointer.move_to(cx, &view, at);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
        });
    })
    .unwrap();
    draw(cx, window);
    keys(cx, window, &["`"]);
    let open = cx.update(|cx| view.read(cx).ui.select_other.is_some());
    let rows: Vec<String> = cx.update(|cx| {
        let v = view.read(cx);
        let label = v.labeler(cx);
        v.ui.select_other.as_ref().map(|s| s.hits.iter().map(|r| label(r)).collect()).unwrap_or_default()
    });
    assert_eq!(rows.first().map(String::as_str), Some("Edge of Sketch 1"), "the line first (panel open {open}): {rows:?}");
    assert!(rows.iter().any(|r| r == "Face of Sketch 1"), "the rectangle's region behind it: {rows:?}");
}

/// Live (sk-16): a rectangle's line clicked, then ` with the pointer still on it: the Select other panel by the pointer,
/// the line first.
#[gpui::test]
fn backtick_on_a_clicked_sketch_line_opens_select_other_by_the_pointer(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = rectangle_and_circle(cx, "sketch-gesture-backtick-clicked");
    let (lines, _) = shapes(cx, &view);
    let l = lines.iter().max_by(|x, y| (x.1.x + x.2.x).total_cmp(&(y.1.x + y.2.x))).unwrap();
    let pointer = Pointer::new(cx, window, &view);
    let at = (l.1 + l.2) * 0.5;
    pointer.move_to(cx, &view, at);
    pointer.click(cx, &view, at);
    assert_eq!(selected(cx, &view), vec![l.0], "the line clicked");
    keys(cx, window, &["`"]);
    let (anchor, rows) = cx.update(|cx| {
        let v = view.read(cx);
        let label = v.labeler(cx);
        (v.ui.select_other.as_ref().map(|s| s.at), v.ui.select_other.as_ref().map(|s| s.hits.iter().map(|r| label(r)).collect::<Vec<String>>()).unwrap_or_default())
    });
    let anchor = anchor.expect("Select other open");
    assert!(anchor.distance(at) < 3.0, "by the pointer {at:?}: {anchor:?}");
    assert_eq!(rows.first().map(String::as_str), Some("Edge of Sketch 1"), "the line first: {rows:?}");
}
