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

/// Live2 feel/drawing.json F4: a dragged view lands exactly where it is released, with no snap to another view's
/// centre line at 12, 9, 5 or 3 px from it (and no guide line). The isometric view is dragged level with Top's centre.
#[gpui::test]
fn a_dragged_view_lands_where_released_with_no_alignment_snap(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-view-align");
    real_sheet(cx, window, &editor);
    let (iso, top_y) = cx.update(|cx| {
        let s = editor.read(cx).current_sheet();
        let iso = s.views.iter().find(|v| matches!(v.kind, ViewKind::Projected { direction: Some(d), .. } if d[0] != 0.0 && d[1] != 0.0)).expect("the isometric view");
        let top = s.views.iter().filter(|v| matches!(v.kind, ViewKind::Projected { direction: Some(d), .. } if d[0] == 0.0)).max_by(|a, b| a.position[1].total_cmp(&b.position[1])).expect("Top");
        (iso.id, top.position[1])
    });
    let mm_per_px = cx.update(|cx| {
        let e = editor.read(cx);
        let (a, b) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        100.0 / (b[0] - a[0]).abs()
    });
    // Drags the view's centre to sheet height `y` (from inside its geometry, in steps).
    let drag_to = |cx: &mut TestAppContext, y: f64| {
        cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
        cx.executor().advance_clock(std::time::Duration::from_millis(600));
        let from = cx.update(|cx| editor.read(cx).current_sheet().view(iso).map(|v| v.position)).expect("the view");
        let grab = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.iter().find(|p| p.id == iso).copied())).expect("placed");
        let g = [(grab.min[0] + grab.max[0]) / 2.0, (grab.min[1] + grab.max[1]) / 2.0];
        let to = [g[0], g[1] + (y - from[1])];
        let p = at_window(cx, &editor, g);
        let mut v = VisualTestContext::from_window(window, cx);
        v.simulate_mouse_move(p, None, Modifiers::default());
        v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
        for k in 1..=6 {
            let t = f64::from(k) / 6.0;
            let m = at_window(cx, &editor, [g[0] + (to[0] - g[0]) * t, g[1] + (to[1] - g[1]) * t]);
            VisualTestContext::from_window(window, cx).simulate_mouse_move(m, Some(MouseButton::Left), Modifiers::default());
        }
        let q = at_window(cx, &editor, to);
        VisualTestContext::from_window(window, cx).simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
        draw(cx, window);
        cx.update(|cx| editor.read(cx).current_sheet().view(iso).map(|v| v.position)).expect("the view")
    };
    // Down well clear of Top's line first (the view starts level with it: a 3 px drag would be a click).
    drag_to(cx, top_y - 40.0 * mm_per_px);
    for off_px in [12.0, 9.0, 5.0, 3.0, 1.0] {
        let after = drag_to(cx, top_y + off_px * mm_per_px);
        assert!(((after[1] - top_y) / mm_per_px - off_px).abs() < 0.6, "{off_px} px from Top's centre line: landed {:.2} px from it", (after[1] - top_y) / mm_per_px);
        drag_to(cx, top_y - 40.0 * mm_per_px);
    }
}

/// Live2 feel/drawing.json D3: the Dimension tool snaps to a view's vertex from 7 px (not 9; ours 8) and marks it;
/// Select puts no snap marker on it.
#[gpui::test]
fn the_dimension_tool_snaps_to_a_vertex_within_its_reach_and_select_marks_none(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::{DimTool, Tool};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-snap-mark");
    real_sheet(cx, window, &editor);
    // A view's vertex with no other snap point near it: the corner farthest down-left.
    let (corner, mm_per_px) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let corner = g.snaps.iter().map(|s| pl.to_sheet(s.p)).min_by(|a, b| (a[0] + a[1]).total_cmp(&(b[0] + b[1]))).expect("a vertex");
        let (a, b) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        (corner, 100.0 / (b[0] - a[0]).abs())
    });
    let snapped = |cx: &mut TestAppContext, d_px: f64| {
        // Straight out from the corner, away from the view.
        let at = [corner[0] - d_px * mm_per_px, corner[1]];
        lit_at(cx, window, &editor, at);
        cx.update(|cx| editor.read(cx).hover_snap)
    };
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)));
    draw(cx, window);
    for d in [0.0, 4.0, 7.0] {
        let s = snapped(cx, d).unwrap_or_else(|| panic!("{d} px from the vertex: snapped"));
        assert!((s[0] - corner[0]).abs() < 1e-6 && (s[1] - corner[1]).abs() < 1e-6, "{d} px: onto the vertex, {s:?} vs {corner:?}");
    }
    assert!(snapped(cx, 10.0).is_none(), "10 px from the vertex: no snap");
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Select, cx)));
    draw(cx, window);
    assert!(snapped(cx, 2.0).is_none(), "Select: no snap marker");
}

/// Live2 feel/drawing.json D1: a note's leader lights the note from 10 px off its stroke, not from 12.
#[gpui::test]
fn a_note_leader_lights_its_note_within_10_px(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
        // The leader to a view's lowest-left vertex, the note down and left of it on the empty sheet.
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let num = |v: &Value| v.as_f64().unwrap_or(0.0);
        let mut corner = [f64::MAX, f64::MAX];
        for v in g["views"].as_array().cloned().unwrap_or_default() {
            for e in v["edges"].as_array().cloned().unwrap_or_default().iter().filter(|e| e["type"] == "line") {
                for k in ["a", "b"] {
                    let p = [num(&e[k][0]), num(&e[k][1])];
                    if p[0] + p[1] < corner[0] + corner[1] {
                        corner = p;
                    }
                }
            }
        }
        let at = [(corner[0] - 40.0).max(25.0), (corner[1] - 40.0).max(25.0)];
        call(ctx, "drawing_note", json!({ "drawing": dw, "text": "DEBURR", "position": at, "leader": corner }));
    }));
    let (window, editor) = drawing(cx, "feel-leader-reach");
    real_sheet(cx, window, &editor);
    let (id, mid, n, mm_per_px) = cx.update(|cx| {
        let e = editor.read(cx);
        let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Note { .. })).expect("the note");
        let paths = noshape_scene::drawing_tools::annotation_edit::leader_paths(e.built.as_deref(), a);
        let pts = &paths.first().expect("its leader").1.points;
        let (p, q) = (pts[pts.len() - 2], pts[pts.len() - 1]);
        let l = (q[0] - p[0]).hypot(q[1] - p[1]);
        let (a2, b2) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        (a.id, [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], [-(q[1] - p[1]) / l, (q[0] - p[0]) / l], 100.0 / (b2[0] - a2[0]).abs())
    });
    for (d, lit) in [(0.0, true), (6.0, true), (10.0, true), (12.0, false)] {
        for side in [1.0, -1.0] {
            let at = [mid[0] + side * n[0] * d * mm_per_px, mid[1] + side * n[1] * d * mm_per_px];
            let got = lit_at(cx, window, &editor, at);
            assert_eq!(got == Some(Sel::Annotation(id)), lit, "{d} px off the leader ({side}): {got:?}");
        }
    }
}

/// Live2 feel/drawing.json "colours", D1, live 2068: a view under the pointer gets a tan rgb(204,178,146) frame ~8 px
/// outside its geometry, its edges as they are; a click turns its edges orange rgb(204,146,92) (and frames it in that
/// orange once the pointer leaves); the pointer gone, nothing is tan.
#[gpui::test]
fn a_hovered_view_is_framed_in_tan_and_a_click_turns_its_edges_orange(cx: &mut TestAppContext) {
    use noshape_render::state;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-colours");
    real_sheet(cx, window, &editor);
    assert_eq!((crate::drawing_view::DRAWING_HOVER, crate::drawing_view::DRAWING_SELECTED), (0xccb292, 0xcc925c), "live's tan and orange");
    let pl = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.first().copied())).expect("a view");
    let inside = [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
    let flags = |cx: &mut TestAppContext, flag: u32| {
        cx.update(|cx| {
            let e = editor.read(cx);
            let scene = e.build_scene();
            let picks = e.picks_of(Sel::View(pl.id));
            (picks.iter().filter(|p| scene.states.get(**p as usize).is_some_and(|s| s & flag != 0)).count(), picks.len())
        })
    };
    let lit = lit_at(cx, window, &editor, inside);
    assert_eq!(view_of(&lit), Some(pl.id), "the view lights: {lit:?}");
    let (hovered, all) = flags(cx, state::HOVER);
    assert!(all > 0 && hovered == 0, "the hovered view's edges stay as they are (its frame shows): {hovered} of {all}");
    let frames = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).view_frames());
    assert_eq!(frames(cx), vec![(pl.id, crate::drawing_view::DRAWING_HOVER)], "the hovered view framed in tan");
    let sel = click_at(cx, window, &editor, inside, Modifiers::default());
    assert!(sel.iter().any(|s| view_of(&Some(*s)) == Some(pl.id)), "the click selects the view: {sel:?}");
    let (selected, all) = flags(cx, state::SELECTED);
    assert_eq!(selected, all, "every edge of the view in the selection colour");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    assert_eq!(flags(cx, state::HOVER).0, 0, "off the view: nothing lit");
    assert_eq!(frames(cx), vec![(pl.id, crate::drawing_view::DRAWING_SELECTED)], "off it, the selected view framed in orange");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    assert!(frames(cx).is_empty(), "nothing selected or hovered: no frame");
}

/// Live2 feel/drawing.json D5: a dimension's text moves from 5 px of drag (4 px is a click, which selects it); no hold
/// needed.
#[gpui::test]
fn a_dimension_moves_from_5_px_and_4_px_is_a_click(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "feel-dim-drag");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.kind.is_dimension()).map(|a| a.id)).expect("a dimension");
    let at = cx.update(|cx| {
        let e = editor.read(cx);
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(Sel::Annotation(id)));
        let t = b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("its text");
        let q = noshape_model::drawing::font::text_box(t);
        [q.iter().map(|p| p[0]).sum::<f64>() / 4.0, q.iter().map(|p| p[1]).sum::<f64>() / 4.0]
    });
    let state = |cx: &mut TestAppContext| cx.update(|cx| format!("{:?}", editor.read(cx).current_sheet().annotations.iter().find(|a| a.id == id).expect("the dimension")));
    for (dx, moves) in [(4.0, false), (5.0, true)] {
        cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
        cx.executor().advance_clock(std::time::Duration::from_millis(600));
        let before = state(cx);
        let p = at_window(cx, &editor, at);
        let q = p + Point::new(px(0.), px(-dx));
        let mut v = VisualTestContext::from_window(window, cx);
        v.simulate_mouse_move(p, None, Modifiers::default());
        v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
        v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
        v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
        draw(cx, window);
        let (after, sel) = (state(cx), cx.update(|cx| editor.read(cx).selection.clone()));
        assert_eq!(after != before, moves, "{dx} px: moved");
        if moves {
            cx.update(|cx| editor.update(cx, |e, cx| e.doc.update(cx, |d, cx| d.undo(cx))));
            draw(cx, window);
        } else {
            assert_eq!(sel, vec![Sel::Annotation(id)], "{dx} px: a click, the dimension selected");
        }
    }
}
