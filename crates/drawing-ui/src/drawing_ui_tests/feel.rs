//! Drawing interaction feel through real pointer events (docs/parity/interaction-feel.md "Drawings"; user P0 11-17):
//! what lights at live's reach, what a click takes, and when a press becomes a drag.

use super::*;
use gpui_kit::{MouseButton, VisualTestContext};
use noshape_scene::drawing_tools::Sel;

/// The window point of sheet point `s` (mm).
fn at_window(cx: &mut TestAppContext, editor: &Entity<DrawingEditor>, s: [f64; 2]) -> Point<gpui_kit::Pixels> {
    cx.update(|cx| {
        let e = editor.read(cx);
        let l = e.sheet_to_local(s);
        e.bounds.origin + Point::new(px(l[0] as f32), px(l[1] as f32))
    })
}

/// The pointer moved to `s`: what lights there.
fn lit_at(cx: &mut TestAppContext, window: AnyWindowHandle, editor: &Entity<DrawingEditor>, s: [f64; 2]) -> Option<Sel> {
    let p = at_window(cx, editor, s);
    VisualTestContext::from_window(window, cx).simulate_mouse_move(p, None, Modifiers::default());
    draw(cx, window);
    cx.update(|cx| {
        let e = editor.read(cx);
        e.hover.and_then(|h| e.sel_of_pick(h))
    })
}

/// A click at `s` (each one apart from the last: never the second of a double-click): the selection after it.
fn click_at(cx: &mut TestAppContext, window: AnyWindowHandle, editor: &Entity<DrawingEditor>, s: [f64; 2], mods: Modifiers) -> Vec<Sel> {
    let p = at_window(cx, editor, s);
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(p, None, mods);
    v.simulate_click(p, mods);
    draw(cx, window);
    cx.update(|cx| editor.read(cx).selection.clone())
}

/// A view as Sel (its edges stand for it in Select).
fn view_of(s: &Option<Sel>) -> Option<noshape_model::drawing::ViewId> {
    match s {
        Some(Sel::View(v) | Sel::Edge(v, _)) => Some(*v),
        _ => None,
    }
}

/// Live2 feel/drawing.json D1: in Select a view lights from 14 px (zoom A) / 16 px (zoom B) outside its geometry, a
/// reach constant in sheet mm (ours 3.6 mm), and a click there takes it; beyond it nothing lights and a click takes
/// nothing.
#[gpui::test]
fn a_view_lights_and_is_taken_within_its_reach_in_sheet_mm(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-view-reach");
    real_sheet(cx, window, &editor);
    let reach = noshape_scene::drawing_tools::VIEW_REACH_MM;
    // The topmost view, approached from the free sheet above it (the left panel covers the sheet's left margin).
    let pl = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.iter().max_by(|a, b| a.max[1].total_cmp(&b.max[1])).copied())).expect("a view");
    let x = (pl.min[0] + pl.max[0]) / 2.0;
    for zoom_by in [1.0, 2.5] {
        // (Zoomed about the approach, which stays where it is on screen.)
        let zoom = cx.update(|cx| editor.update(cx, |e, cx| {
            let l = e.sheet_to_local([x, pl.max[1] + reach]);
            e.zoom_by(zoom_by, [l[0], l[1]], cx);
            e.zoom
        }));
        draw(cx, window);
        for (px_off, lights) in [(-2.0, true), (2.0, false)] {
            let s = [x, pl.max[1] + reach + px_off / zoom];
            let lit = lit_at(cx, window, &editor, s);
            assert_eq!(view_of(&lit) == Some(pl.id), lights, "zoom {zoom}: {px_off} px past the reach, lit {lit:?}");
            cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
            let sel = click_at(cx, window, &editor, s, Modifiers::default());
            assert_eq!(sel.iter().any(|x| view_of(&Some(*x)) == Some(pl.id)), lights, "zoom {zoom}: {px_off} px past the reach, the click takes what lit: {sel:?}");
        }
    }
}

/// Live2 feel/drawing.json D1: a dimension's line lights from 6 px, not 8 (ours 7), its text from 8 px outside the
/// text's box, not 10 (ours 9); a click there selects the dimension.
#[gpui::test]
fn a_dimensions_line_and_text_light_at_lives_reach(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "feel-dim-reach");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.kind.is_dimension()).map(|a| a.id)).expect("a dimension");
    // Its dimension line (the longest straight line of its picks) a sixth along, away from the text, and its text's
    // box top.
    let (line, text_top, text_mid_x, zoom) = cx.update(|cx| {
        let e = editor.read(cx);
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(Sel::Annotation(id)));
        let len = |l: &noshape_model::drawing::sheet::Polyline2| (l.points[1][0] - l.points[0][0]).hypot(l.points[1][1] - l.points[0][1]);
        let l = b.geometry.lines.iter().filter(|l| picks.contains(&l.pick) && l.points.len() == 2 && (l.points[1][1] - l.points[0][1]).abs() < 1e-6).max_by(|a, b| len(a).total_cmp(&len(b))).expect("a level dimension line").clone();
        let t = b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("its text");
        let q = noshape_model::drawing::font::text_box(t);
        let top = q.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        let mid = q.iter().map(|p| p[0]).sum::<f64>() / 4.0;
        (l, top, mid, e.zoom)
    });
    let (a, b) = (line.points[0], line.points[1]);
    let on = [a[0] + (b[0] - a[0]) / 6.0, a[1]];
    // Below the line: away from the text above it.
    for (off, lights) in [(6.0, true), (8.0, false)] {
        let s = [on[0], on[1] - off / zoom];
        let lit = lit_at(cx, window, &editor, s);
        assert_eq!(lit == Some(Sel::Annotation(id)), lights, "{off} px from the dimension line: lit {lit:?}");
    }
    for (off, lights) in [(8.0, true), (10.0, false)] {
        let s = [text_mid_x, text_top + off / zoom];
        let lit = lit_at(cx, window, &editor, s);
        assert_eq!(lit == Some(Sel::Annotation(id)), lights, "{off} px above the text's box: lit {lit:?}");
    }
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    let sel = click_at(cx, window, &editor, [on[0], on[1] - 6.0 / zoom], Modifiers::default());
    assert_eq!(sel, vec![Sel::Annotation(id)], "the click takes the lit dimension");
}

/// Live2 feel/drawing.json D6: a click, a Ctrl + click and a Shift + click each toggle what they land on: add it when
/// unselected, take it out when selected; the others stay.
#[gpui::test]
fn click_ctrl_click_and_shift_click_toggle(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-toggle");
    real_sheet(cx, window, &editor);
    let pls = cx.update(|cx| editor.read(cx).built.as_ref().map(|b| b.placements.clone()).unwrap_or_default());
    let mid = |pl: &noshape_model::drawing::build::ViewPlacement| [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
    let (a, b) = (pls[0], pls[1]);
    let ctrl = Modifiers::secondary_key();
    let shift = Modifiers { shift: true, ..Default::default() };
    for mods in [Modifiers::default(), ctrl, shift] {
        cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
        let views = |sel: Vec<Sel>| sel.iter().filter_map(|s| view_of(&Some(*s))).collect::<Vec<_>>();
        assert_eq!(views(click_at(cx, window, &editor, mid(&a), mods)), vec![a.id], "{mods:?}: the first view added");
        assert_eq!(views(click_at(cx, window, &editor, mid(&b), mods)), vec![a.id, b.id], "{mods:?}: the second added");
        assert_eq!(views(click_at(cx, window, &editor, mid(&a), mods)), vec![b.id], "{mods:?}: the first taken out again");
    }
}

/// Live2 feel/drawing.json D5: a press on a view becomes a drag past 4 px (5 px moves it, 4 px is a click that selects
/// it, the view where it was).
#[gpui::test]
fn a_view_moves_from_5_px_and_4_px_is_a_click(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-drag");
    real_sheet(cx, window, &editor);
    let pl = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.first().copied())).expect("a view");
    let start = [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
    for (dx, moves) in [(4.0, false), (5.0, true)] {
        cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
        cx.executor().advance_clock(std::time::Duration::from_millis(600));
        let before = cx.update(|cx| editor.read(cx).current_sheet().view(pl.id).map(|v| v.position)).expect("the view");
        let p = at_window(cx, &editor, start);
        let q = p + Point::new(px(dx), px(0.));
        let mut v = VisualTestContext::from_window(window, cx);
        v.simulate_mouse_move(p, None, Modifiers::default());
        v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
        v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
        v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
        draw(cx, window);
        let (after, sel) = cx.update(|cx| (editor.read(cx).current_sheet().view(pl.id).map(|v| v.position).expect("the view"), editor.read(cx).selection.clone()));
        assert_eq!(after != before, moves, "{dx} px: moved {before:?} -> {after:?}");
        if !moves {
            assert!(sel.iter().any(|s| view_of(&Some(*s)) == Some(pl.id)), "{dx} px: a click, the view selected: {sel:?}");
        }
        if moves {
            cx.update(|cx| editor.update(cx, |e, cx| e.doc.update(cx, |d, cx| d.undo(cx))));
            draw(cx, window);
        }
    }
}

/// Live2 feel/drawing.json D2: a centerline laid on a view's edge lights the view there and a click takes the view; a
/// centerline inside the view (3 px from any edge) lights and is taken itself.
#[gpui::test]
fn a_centerline_on_a_view_edge_gives_way_to_the_view(cx: &mut TestAppContext) {
    use noshape_model::drawing::{Annotation, AnnotationKind};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-centerline-edge");
    real_sheet(cx, window, &editor);
    // The first view's longest straight edge (view coordinates), and a line across the view's middle.
    let (view, on_edge, inside, zoom) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().and_then(|b| b.placements.first().copied()).expect("a view");
        let g = e.geoms.get(&pl.id).expect("its geometry");
        let len = |p: &[[f64; 2]]| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]);
        let edge = g.edges.iter().filter(|x| x.visible && x.points.len() == 2).max_by(|a, b| len(&a.points).total_cmp(&len(&b.points))).expect("a straight edge");
        let (a, b) = (edge.points[0], edge.points[1]);
        let lerp = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        let lo = g.edges.iter().flat_map(|x| x.points.iter()).fold([f64::MAX; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
        let hi = g.edges.iter().flat_map(|x| x.points.iter()).fold([f64::MIN; 2], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
        let mid_y = (lo[1] + hi[1]) / 2.0;
        (pl.id, (lerp(0.2), lerp(0.8)), ([lo[0] + (hi[0] - lo[0]) * 0.2, mid_y], [lo[0] + (hi[0] - lo[0]) * 0.8, mid_y]), e.zoom)
    });
    let (id_edge, id_inside) = cx.update(|cx| editor.update(cx, |e, cx| {
        let first = e.current_sheet().next_annotation_id();
        e.edit_sheet("Centerline", cx, |s| {
            s.annotations.push(Annotation { id: first, kind: AnnotationKind::Centerline { view, a: on_edge.0, b: on_edge.1 }, ..Default::default() });
            s.annotations.push(Annotation { id: first + 1, kind: AnnotationKind::Centerline { view, a: inside.0, b: inside.1 }, ..Default::default() });
        });
        (first, first + 1)
    }));
    real_sheet(cx, window, &editor);
    let sheet_of = |cx: &mut TestAppContext, p: [f64; 2]| cx.update(|cx| editor.read(cx).placement(view).map(|pl| pl.to_sheet(p))).expect("placed");
    let mid = |l: ([f64; 2], [f64; 2])| [(l.0[0] + l.1[0]) / 2.0, (l.0[1] + l.1[1]) / 2.0];
    let at_edge = sheet_of(cx, mid(on_edge));
    let lit = lit_at(cx, window, &editor, at_edge);
    assert_eq!(view_of(&lit), Some(view), "on the edge: the view lights, not centerline {id_edge}: {lit:?}");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    let sel = click_at(cx, window, &editor, at_edge, Modifiers::default());
    assert!(sel.iter().any(|s| view_of(&Some(*s)) == Some(view)) && !sel.contains(&Sel::Annotation(id_edge)), "on the edge the click takes the view: {sel:?}");
    let at_inside = sheet_of(cx, mid(inside));
    let lit = lit_at(cx, window, &editor, [at_inside[0], at_inside[1] + 1.0 / zoom]);
    assert_eq!(lit, Some(Sel::Annotation(id_inside)), "inside the view: the centerline");
}

/// Live2 feel/drawing.json D5: a press on a note becomes a drag only past 7 px (8 px moves it, 6 px is a click that
/// selects it).
#[gpui::test]
fn a_note_moves_from_8_px_and_6_px_is_a_click(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
        call(ctx, "drawing_note", json!({ "drawing": dw, "text": "BREAK ALL SHARP EDGES", "position": [30, 40] }));
    }));
    let (window, editor) = drawing(cx, "feel-note-drag");
    real_sheet(cx, window, &editor);
    let (id, at) = cx.update(|cx| {
        let e = editor.read(cx);
        let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Note { .. })).expect("the note");
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(Sel::Annotation(a.id)));
        let t = b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("its text");
        let q = noshape_model::drawing::font::text_box(t);
        (a.id, [q.iter().map(|p| p[0]).sum::<f64>() / 4.0, q.iter().map(|p| p[1]).sum::<f64>() / 4.0])
    });
    for (dx, moves) in [(6.0, false), (8.0, true)] {
        cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
        cx.executor().advance_clock(std::time::Duration::from_millis(600));
        let before = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.id == id).map(|a| a.position)).expect("the note");
        let p = at_window(cx, &editor, at);
        let q = p + Point::new(px(dx), px(0.));
        let mut v = VisualTestContext::from_window(window, cx);
        v.simulate_mouse_move(p, None, Modifiers::default());
        v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
        v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
        v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
        draw(cx, window);
        let (after, sel) = cx.update(|cx| (editor.read(cx).current_sheet().annotations.iter().find(|a| a.id == id).map(|a| a.position).expect("the note"), editor.read(cx).selection.clone()));
        assert_eq!(after != before, moves, "{dx} px: moved {before:?} -> {after:?}");
        if !moves {
            assert!(sel.contains(&Sel::Annotation(id)), "{dx} px: a click, the note selected: {sel:?}");
        } else {
            cx.update(|cx| editor.update(cx, |e, cx| e.doc.update(cx, |d, cx| d.undo(cx))));
            draw(cx, window);
        }
    }
}

/// Live2 feel/drawing.json D4: placing a note, the pointer lines up with another annotation from 7 px, not 9 (ours
/// 8): the pink dotted line shows and the point jumps onto the alignment.
#[gpui::test]
fn placing_a_note_lines_up_with_another_within_8_px(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
        call(ctx, "drawing_note", json!({ "drawing": dw, "text": "NOTE", "position": [30, 40] }));
    }));
    let (window, editor) = drawing(cx, "feel-infer");
    real_sheet(cx, window, &editor);
    let (note, zoom) = cx.update(|cx| editor.update(cx, |e, _| {
        e.tool = noshape_scene::drawing_tools::Tool::Note;
        let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Note { .. })).expect("the note");
        (a.position, e.zoom)
    }));
    // Straight above the note, 60 mm up: off to the side by 7 and by 9 px.
    for (off, lines_up) in [(7.0, true), (9.0, false)] {
        let s = [note[0] + off / zoom, note[1] + 60.0];
        let p = at_window(cx, &editor, s);
        VisualTestContext::from_window(window, cx).simulate_mouse_move(p, None, Modifiers::default());
        draw(cx, window);
        let (shown, x) = cx.update(|cx| {
            let e = editor.read(cx);
            (e.infer_lines.iter().any(|(from, _)| (from[0] - note[0]).abs() < 1e-9), e.cursor[0])
        });
        assert_eq!(shown, lines_up, "{off} px off the note's x: the alignment line");
        assert_eq!((x - note[0]).abs() < 1e-9, lines_up, "{off} px off: the point on the note's x ({x} for {})", note[0]);
    }
}
