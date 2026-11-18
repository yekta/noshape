//! Drawing interaction feel through real pointer events (docs/parity/interaction-feel.md "Drawings"; user P0 11-17):
//! what lights at live's reach, what a click takes, and when a press becomes a drag.

use super::*;
use gpui_kit::{Focusable as _, MouseButton, VisualTestContext};
use noshape_scene::drawing_tools::{Sel, Tool};

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
fn a_dragged_view_lands_where_released_with_no_alignment_snap_or_line(cx: &mut TestAppContext) {
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
            // No dashed alignment lines at any distance either (live2 drw-feel-1118: none from 60 px to 0).
            draw(cx, window);
            assert!(cx.update(|cx| editor.read(cx).infer_lines.is_empty()), "no alignment line while a view is dragged");
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
    use noshape_scene::drawing_tools::DimTool;
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

/// Live drw/dact-aa-d3, behaviours "drawing Dimension tool": with the Dimension tool a view's edge lights alone, in
/// orange, 2 px solid and antialiased as every highlighted drawing line (live dh0, dsel3); Select keeps tan.
#[gpui::test]
fn the_dimension_tool_lights_a_hovered_edge_alone_in_orange(cx: &mut TestAppContext) {
    use crate::drawing_view::{DRAWING_HOVER, DRAWING_SELECTED};
    use noshape_render::state;
    use noshape_scene::drawing_tools::{DimTool, Tool};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-tool-edge");
    real_sheet(cx, window, &editor);
    // The middle of a view's longest visible straight edge.
    let mid = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let len = |p: &[[f64; 2]]| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]);
        let edge = g.edges.iter().filter(|x| x.visible && x.points.len() == 2).max_by(|a, b| len(&a.points).total_cmp(&len(&b.points))).expect("a straight edge");
        let (a, b) = (pl.to_sheet(edge.points[0]), pl.to_sheet(edge.points[1]));
        [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
    });
    let look = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let e = editor.read(cx);
            (e.build_scene().states.iter().filter(|s| **s & state::HOVER != 0).count(), e.render_style())
        })
    };
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)));
    draw(cx, window);
    let lit = lit_at(cx, window, &editor, mid);
    assert!(matches!(lit, Some(Sel::Edge(..))), "the Dimension tool lights the edge: {lit:?}");
    let (hovered, style) = look(cx);
    assert_eq!(hovered, 1, "the edge alone, not its view");
    assert_eq!(style.hover_color, crate::theme::lin(DRAWING_SELECTED), "in orange");
    assert_eq!(style.lit_line_px, 3.0, "2 px solid with an antialiased pixel each side");
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Select, cx)));
    draw(cx, window);
    let lit = lit_at(cx, window, &editor, mid);
    assert!(view_of(&lit).is_some(), "Select: the edge stands for its view: {lit:?}");
    let (hovered, style) = look(cx);
    assert_eq!((hovered, style.hover_color), (0, crate::theme::lin(DRAWING_HOVER)), "Select: the view's tan frame, its edges as they are");
}

/// Live drawing-sysvars.json: a tool picks within Graebert's PICKBOX, 7 px (ours had 5): the Dimension tool lights an
/// edge 6 px off it, not 9 px off.
#[gpui::test]
fn a_tool_picks_an_edge_within_7_px(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::DimTool;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-pickbox");
    real_sheet(cx, window, &editor);
    let (mid, normal) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let len = |p: &[[f64; 2]]| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]);
        let edge = g.edges.iter().filter(|x| x.visible && x.points.len() == 2).max_by(|a, b| len(&a.points).total_cmp(&len(&b.points))).expect("a straight edge");
        let (a, b) = (pl.to_sheet(edge.points[0]), pl.to_sheet(edge.points[1]));
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], [-(b[1] - a[1]) / l, (b[0] - a[0]) / l])
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)));
    draw(cx, window);
    let edge = lit_at(cx, window, &editor, mid);
    assert!(matches!(edge, Some(Sel::Edge(..))), "on the edge: {edge:?}");
    let off = |cx: &mut TestAppContext, px: f64| {
        let mm = cx.update(|cx| editor.read(cx).tol_mm(px));
        [mid[0] + normal[0] * mm, mid[1] + normal[1] * mm]
    };
    for (px, lights) in [(6.0, true), (9.0, false)] {
        for sign in [1.0, -1.0] {
            let at = off(cx, sign * px);
            let lit = lit_at(cx, window, &editor, at);
            assert_eq!(lit == edge, lights, "{} px off the edge: lit {lit:?}", sign * px);
        }
    }
}

/// Live2 drw-inspection-table-iso-scrolled: in an ISO drawing a linear dimension's value (drawn above its line) is
/// placed with its centre on the pointer, the line under it.
#[gpui::test]
fn an_iso_dimensions_value_lands_centred_on_the_pointer(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    use noshape_scene::drawing_tools::DimTool;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-iso-value");
    real_sheet(cx, window, &editor);
    assert!(cx.update(|cx| !editor.read(cx).drawing.is_ansi()), "an ISO drawing");
    let (mid, normal) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let len = |p: &[[f64; 2]]| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]);
        // The lowest long horizontal edge of the first view: the value placed under it, clear of other views.
        let edge = g.edges.iter().filter(|x| x.visible && x.points.len() == 2 && (x.points[0][1] - x.points[1][1]).abs() < 1e-9 && len(&[pl.to_sheet(x.points[0]), pl.to_sheet(x.points[1])]) > 5.0).min_by(|a, b| pl.to_sheet(a.points[0])[1].total_cmp(&pl.to_sheet(b.points[0])[1])).expect("a horizontal edge");
        let (a, b) = (pl.to_sheet(edge.points[0]), pl.to_sheet(edge.points[1]));
        ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], [0.0, -1.0])
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)));
    draw(cx, window);
    click_at(cx, window, &editor, mid, Modifiers::default());
    let tool = cx.update(|cx| format!("{:?}", editor.read(cx).tool));
    assert!(tool.contains("Line"), "the edge picked: {tool}");
    let at = [mid[0], mid[1] + normal[1] * 8.0];
    click_at(cx, window, &editor, at, Modifiers::default());
    let centre = cx.update(|cx| {
        let e = editor.read(cx);
        let sheet = e.current_sheet();
        let id = sheet.annotations.iter().find(|a| a.kind.is_dimension()).expect("the dimension placed").id;
        let b = e.built.as_ref().expect("built");
        let pts: Vec<[f64; 2]> = b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(PickTarget::Annotation(x)) if *x == id)).flat_map(noshape_model::drawing::font::text_box).collect();
        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
        (lo + hi) / 2.0
    });
    assert!((centre - at[1]).abs() < 0.05, "the value's centre {centre:.2} on the pointer {:.2}", at[1]);
}

/// Live tables-1118 (h-stack; behaviours 1482): a hovered table keeps its ink black (no hover tint on its lines or
/// text); its look is the grey rows, the hovered row or cell darker, and a tan frame.
#[gpui::test]
fn a_hovered_table_keeps_its_ink_and_shades_the_cell_under_the_pointer(cx: &mut TestAppContext) {
    use noshape_render::state;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-table-hover");
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let id = e.current_sheet().tables.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            e.edit_sheet("Table", cx, |s| {
                s.tables.push(noshape_model::drawing::Table::new(id, noshape_model::drawing::TableKind::General { cells: vec![vec!["a".into(), "b".into()], vec!["c".into(), "d".into()]], title: None, header: false }, [60.0, 200.0]));
            });
            id
        })
    });
    draw(cx, window);
    let (centre, picks) = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        let l = b.table_layout(id).expect("the table laid out");
        let centre = [(l.xs[0] + l.xs[1]) / 2.0, (l.ys[0] + l.ys[1]) / 2.0];
        let picks: Vec<u32> = b.picks.iter().enumerate().filter(|(_, p)| matches!(p, noshape_model::drawing::build::PickTarget::Table(t) if *t == id)).map(|(i, _)| i as u32).collect();
        (centre, picks)
    });
    let lit = lit_at(cx, window, &editor, centre);
    assert_eq!(lit, Some(Sel::Table(id)), "the table is under the pointer");
    let (tinted, tables) = cx.update(|cx| {
        let e = editor.read(cx);
        let states = e.build_scene().states;
        (picks.iter().any(|p| states.get(*p as usize).is_some_and(|s| s & (state::HOVER | state::SELECTED) != 0)), e.lit_tables())
    });
    assert!(!tinted, "its ink keeps its colour");
    assert_eq!(tables, vec![(id, Some((0, 0)))], "the table and the cell under the pointer are shaded");
}

/// Live cferr-bomclk-1118 (b): one click on a BOM cell selects that cell (blue) and opens the floating Cell panel for
/// its formats, the table still selected; a double-click is the same, the cell not typed in.
#[gpui::test]
fn a_click_on_a_bom_cell_opens_the_cell_panel_and_a_double_click_adds_nothing(cx: &mut TestAppContext) {
    use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-bom-cell");
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let id = e.current_sheet().tables.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            let view = e.current_sheet().views[0].id;
            let kind = noshape_model::drawing::TableKind::Bom { view, structured: false, top_level: false, bottom_to_top: false, overrides: Vec::new(), default_template: true, source: None };
            e.edit_sheet("BOM table", cx, |s| s.tables.push(noshape_model::drawing::Table::new(id, kind, [60.0, 200.0])));
            id
        })
    });
    draw(cx, window);
    let cell = cx.update(|cx| {
        let l = editor.read(cx).built.as_ref().expect("built").table_layout(id).expect("the BOM laid out");
        let first = l.title as usize + 1;
        [(l.xs[0] + l.xs[1]) / 2.0, (l.ys[first] + l.ys[first + 1]) / 2.0]
    });
    let at = at_window(cx, &editor, cell);
    let press = |cx: &mut TestAppContext, counts: std::ops::RangeInclusive<usize>| {
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| {
            e.on_mouse_move(&MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }, w, cx);
            for n in counts {
                e.on_mouse_down(&MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: n, first_mouse: false }, w, cx);
                e.on_mouse_up(&MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: n }, w, cx);
            }
        }))
        .unwrap();
        draw(cx, window);
        cx.update(|cx| {
            let e = editor.read(cx);
            (e.cell_sel.map(|c| (c.table, c.a, c.b)), e.selection.clone(), e.cell_panel.is_some(), e.panel.is_none())
        })
    };
    let one = press(cx, 1..=1);
    assert_eq!(one, (Some((id, (1, 0), (1, 0))), vec![Sel::Table(id)], true, true), "one click: the cell, the table, the Cell panel, no editor");
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    assert_eq!(press(cx, 1..=2), one, "a double-click: the same");
}

/// Live2 bomsel-1118 (tables-1118 d3): the BOM table tool places the table on one click (no ✓); the panel closes and
/// the table stays selected.
#[gpui::test]
fn the_bom_tool_places_on_one_click_and_keeps_the_table_selected(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-bom-place");
    real_sheet(cx, window, &editor);
    let empty = empty_sheet_point(cx, window, &editor);
    let placed = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            // (The drawing's Part Studio stands in for an assembly: the tool lists assemblies only.)
            e.sources.values_mut().for_each(|s| s.assembly = true);
            e.tool = Tool::Table { kind: noshape_scene::drawing_tools::TableTool::Bom, view: None, origin: None };
            e.sync_table_dlg();
            assert!(e.table_dlg.is_some(), "the BOM table panel is open");
            e.cursor = empty;
            assert!(e.table_dlg_click(cx), "the click is the tool's");
            let id = e.current_sheet().tables.iter().map(|t| t.id).max().expect("a table placed");
            (id, e.selection.clone(), e.table_dlg.is_none(), matches!(e.tool, Tool::Select))
        })
    });
    assert_eq!(placed.1, vec![Sel::Table(placed.0)], "the placed table is selected");
    assert!(placed.2 && placed.3, "the panel closed and Select is back");
    let after_esc = cx.update(|cx| editor.update(cx, |e, cx| {
        e.cancel(cx);
        e.selection.clone()
    }));
    assert_eq!(after_esc, vec![Sel::Table(placed.0)], "Esc keeps it selected");
}

/// Live keys-1118 (hov-measure): the drawing's bottom right has a Measure button, "Measure ([)"; it turns Measure on
/// and off as `[` does.
#[gpui::test]
fn the_measure_button_turns_measure_on_and_off(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "feel-measure-button");
    real_sheet(cx, window, &editor);
    let on = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).measure.is_some());
    assert!(!on(cx));
    super::press(cx, window, "dr-measure");
    assert!(on(cx), "pressed: Measure on");
    super::press(cx, window, "dr-measure");
    assert!(!on(cx), "again: off");
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

/// Live drw-styles-dimension s26-30 (behaviours "qa5 ADV4": Wt spinboxes ignore Esc): a value typed in the Styles
/// panel's Text gap is set on Enter, the field keeps the focus, and Esc after it leaves the dimension selected; leaving
/// the field after that adds no second undo step.
#[gpui::test]
fn a_styles_text_gap_is_set_on_enter_and_esc_after_it_keeps_the_dimension_selected(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "feel-styles-gap");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.kind.is_dimension()).map(|a| a.id)).expect("a dimension");
    cx.update(|cx| {
        editor.update(cx, |e, cx| {
            e.selection = vec![Sel::Annotation(id)];
            e.right_panel = Some(crate::drawing_panels::RightPanel::Styles);
            cx.notify();
        })
    });
    draw(cx, window);
    let field = cx.update(|cx| editor.read(cx).styles_gap[0].as_ref().map(|g| g.1.clone())).expect("the Text gap field");
    cx.update_window(window, |_, w, cx| w.focus(&field.focus_handle(cx), cx)).unwrap();
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_keystrokes("secondary-a");
    v.simulate_input("0.2");
    v.simulate_keystrokes("enter");
    draw(cx, window);
    let gap = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| a.style.as_ref()).and_then(|s| s.text_gap));
    let unit = cx.update(|cx| noshape_model::drawing::format::length_units(&editor.read(cx).drawing, &noshape_model::drawing::DimFormat::default()).meters() * 1000.0);
    assert!(gap(cx).is_some_and(|g| (g - 0.2 * unit).abs() < 1e-9), "Enter sets the gap: {:?}", gap(cx));
    assert!(cx.update_window(window, |_, w, cx| field.focus_handle(cx).is_focused(w)).unwrap(), "the field keeps the focus");
    VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    draw(cx, window);
    assert_eq!(cx.update(|cx| editor.read(cx).selection.clone()), vec![Sel::Annotation(id)], "Esc keeps the dimension selected");
    cx.update_window(window, |_, w, cx| {
        let f = editor.read(cx).focus.clone();
        w.focus(&f, cx)
    })
    .unwrap();
    draw(cx, window);
    cx.update(|cx| editor.update(cx, |e, cx| e.doc.update(cx, |d, cx| d.undo(cx))));
    draw(cx, window);
    assert_eq!(gap(cx), None, "one undo takes the gap back: leaving the field added no step");
}

/// Live2 drw-hatch-panel-tips s14 (behaviours "studios (11-17, low)"): in Hatch region with Corner rectangle on, the
/// first Esc ends the profile tool and the panel stays; the second closes it.
#[gpui::test]
fn esc_in_hatch_region_ends_its_profile_tool_first_then_closes_the_panel(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "feel-hatch-esc");
    cx.update_window(window, |_, w, cx| {
        editor.update(cx, |e, cx| {
            e.open_hatch_dialog(None, w, cx);
            e.set_tool(Tool::Sketch { tool: crate::drawing_sketch::SkTool::CornerRect, pts: vec![], from: None }, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    let state = |cx: &mut TestAppContext| cx.update(|cx| (editor.read(cx).hatch_dlg.is_some(), matches!(editor.read(cx).tool, Tool::Sketch { .. })));
    assert_eq!(state(cx), (true, true), "Hatch region open, Corner rectangle on");
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    assert_eq!(state(cx), (true, false), "the first Esc ends the tool, the panel stays");
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    assert_eq!(state(cx), (false, false), "the second closes the panel");
}

/// Live2 drw-centermark (behaviours "studios (11-17, P0 drawing feel)"): a hole's centre mark lights from its circle, its
/// centre or an arm (its view not framed), its circle light blue, its grips shown; hovering the view alone shows none;
/// with the view selected the grips show and the circle is the view's orange; a press on the circle drags the view.
#[gpui::test]
fn a_holes_centre_mark_lights_from_its_circle_and_a_press_there_drags_the_view(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    SUGGESTED.set(true);
    HOLED.set(true);
    let (window, editor) = drawing(cx, "feel-centermark");
    HOLED.set(false);
    real_sheet(cx, window, &editor);
    let (view, key, c, r, reach, mm_per_px, circle) = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        let (view, key) = b.picks.iter().find_map(|t| if let PickTarget::AutoCentermark { view, center } = t { Some((*view, *center)) } else { None }).expect("the hole's automatic centre mark");
        let pl = b.placement(view).copied().expect("its view");
        let c = pl.to_sheet([key[0] as f64 / 1e9, key[1] as f64 / 1e9]);
        let circle = e.pk().centermark_circle_picks(view, key);
        let l = b.geometry.lines.iter().find(|l| circle.contains(&l.pick)).expect("the hole's circle");
        let r = (l.points[0][0] - c[0]).hypot(l.points[0][1] - c[1]);
        let reach = b.geometry.grips.iter().filter(|g| b.picks.get(g.pick as usize) == Some(&PickTarget::AutoCentermark { view, center: key })).map(|g| (g.pos[0] - c[0]).hypot(g.pos[1] - c[1])).fold(0.0, f64::max);
        let (a, z) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        (view, key, c, r, reach, 100.0 / (z[0] - a[0]).abs(), circle)
    });
    assert!(reach > r, "grips at the arm ends, past the circle: {reach} vs {r}");
    let mark = Sel::AutoCentermark(view, key);
    let looks = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).centermark_looks());
    let frames = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).view_frames());
    let d = std::f64::consts::FRAC_1_SQRT_2;
    let on_circle = [c[0] + r * d, c[1] + r * d];
    let on_arm = [c[0] + (r + reach) / 2.0, c[1]];
    for (at, what) in [(on_circle, "its circle"), (c, "its centre"), (on_arm, "an arm")] {
        assert_eq!(lit_at(cx, window, &editor, at), Some(mark), "{what} lights the centre mark");
        let (grips, blue) = looks(cx);
        assert!(!grips.is_empty() && blue == circle, "{what}: its grips, its circle light blue ({grips:?}, {blue:?})");
        assert!(frames(cx).is_empty(), "{what}: the view not framed");
    }
    let blank = [c[0] + reach + 12.0 * mm_per_px, c[1] + reach + 12.0 * mm_per_px];
    assert_eq!(view_of(&lit_at(cx, window, &editor, blank)), Some(view), "the view's blank inside lights the view");
    assert_eq!(looks(cx), (vec![], vec![]), "the view hovered: no grips");
    assert_eq!(click_at(cx, window, &editor, on_arm, Modifiers::default()), vec![mark], "a click on an arm takes the centre mark");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    let (grips, blue) = looks(cx);
    assert!(!grips.is_empty() && blue == circle, "selected: grips, the circle light blue");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    assert_eq!(click_at(cx, window, &editor, on_circle, Modifiers::default()), vec![mark], "a click on the circle takes the centre mark too");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    assert_eq!(click_at(cx, window, &editor, blank, Modifiers::default()), vec![Sel::View(view)], "a click inside takes the view");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    let (grips, blue) = looks(cx);
    assert!(!grips.is_empty() && blue.is_empty(), "the view selected: its centre mark's grips, the circle the view's orange");
    let lit_arms = cx.update(|cx| {
        let e = editor.read(cx);
        let scene = e.build_scene();
        e.picks_of(mark).iter().filter(|p| scene.states.get(**p as usize).is_some_and(|s| *s & (noshape_render::state::SELECTED | noshape_render::state::HOVER) != 0)).count()
    });
    assert_eq!(lit_arms, 0, "the view selected: its centre mark's arms as they are");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    let position = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).current_sheet().view(view).map(|v| v.position).expect("the view"));
    let before = position(cx);
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    let (p, q) = (at_window(cx, &editor, on_circle), at_window(cx, &editor, [on_circle[0] + 20.0 * mm_per_px, on_circle[1]]));
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(p, None, Modifiers::default());
    v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
    v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
    v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
    draw(cx, window);
    let after = position(cx);
    assert!((after[0] - before[0] - 20.0 * mm_per_px).abs() < 0.01 && (after[1] - before[1]).abs() < 0.01, "a press on the circle drags the view: {before:?} -> {after:?}");
}

/// Live2 sheet-fit (A3-start, B-start; 11-15): a drawing opens at its Zoom to fit (camera_ops `a_sheet_fits_as_live`:
/// the paper about 47 px short of a 973 px canvas, 14.75 px below its centre); zoomed and panned, Zoom to fit puts it
/// back where it opened.
#[gpui::test]
fn a_drawing_opens_fitted_and_zoom_to_fit_puts_it_back(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "feel-fit");
    real_sheet(cx, window, &editor);
    let camera = |cx: &mut TestAppContext| cx.update(|cx| { let e = editor.read(cx); (e.zoom, e.center) });
    let opened = camera(cx);
    let fit = cx.update(|cx| { let e = editor.read(cx); noshape_scene::camera_ops::drawing_fit_sheet(e.size_px(), e.current_sheet()) });
    assert_eq!(opened, fit, "opened at the sheet's fit");
    cx.update(|cx| editor.update(cx, |e, cx| {
        e.zoom_by(2.5, [300.0, 200.0], cx);
        e.pan_step(1.0, -1.0, cx);
    }));
    draw(cx, window);
    assert_ne!(camera(cx), opened, "zoomed and panned");
    cx.update(|cx| editor.update(cx, |e, cx| e.zoom_to_fit(cx)));
    draw(cx, window);
    assert_eq!(camera(cx), opened, "Zoom to fit: back where it opened");
}

/// Live2 drw-window-resize: at its Zoom to fit the sheet is fitted again on a window resize; zoomed and panned it
/// keeps its width across the canvas (1879 -> 1400 px canvas: the sheet 1300 -> 969 px) and its place along it, and a
/// resize back restores it exactly.
#[gpui::test]
fn a_resized_window_refits_a_fitted_sheet_and_scales_a_zoomed_one_with_its_width(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "feel-resize");
    real_sheet(cx, window, &editor);
    let resize = |cx: &mut TestAppContext, w: f32, h: f32| {
        VisualTestContext::from_window(window, cx).simulate_resize(size(px(w), px(h)));
        draw(cx, window);
    };
    let state = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let e = editor.read(cx);
            let b = e.built.as_ref().expect("built");
            let (l, r) = (e.sheet_to_local([0.0, 0.0])[0], e.sheet_to_local([b.geometry.width, 0.0])[0]);
            let fit = noshape_scene::camera_ops::drawing_fit_sheet(e.size_px(), e.current_sheet());
            ((e.zoom, e.center), fit, r - l, (l + r) / 2.0 / e.size_px()[0], e.size_px()[0])
        })
    };
    resize(cx, 1919.0, 992.0);
    let (camera, fit, ..) = state(cx);
    assert_eq!(camera, fit, "at its fit: fitted again to the new canvas");
    resize(cx, 1400.0, 760.0);
    let (camera, fit, ..) = state(cx);
    assert_eq!(camera, fit, "and again");
    resize(cx, 1919.0, 992.0);
    cx.update(|cx| editor.update(cx, |e, cx| {
        e.zoom_by(1.7, [500.0, 300.0], cx);
        e.pan_step(1.0, 0.0, cx);
    }));
    draw(cx, window);
    let (camera, _, wide, place, canvas) = state(cx);
    resize(cx, 1400.0, 760.0);
    let (_, fit, wide2, place2, canvas2) = state(cx);
    assert!(canvas2 < canvas, "a narrower canvas");
    assert!((wide2 / wide - canvas2 / canvas).abs() < 1e-9, "the sheet scales with the canvas width: {wide} -> {wide2} px over {canvas} -> {canvas2}");
    assert!((place2 - place).abs() < 1e-9, "its place along the canvas kept: {place} -> {place2}");
    assert_ne!(state(cx).0, fit, "not refitted");
    resize(cx, 1919.0, 992.0);
    let (back, ..) = state(cx);
    assert!((back.0 - camera.0).abs() < 1e-9 && back.1 == camera.1, "resized back: as it was ({camera:?} vs {back:?})");
}

/// A press at `p` dragged to `q` (sheet mm), then released.
fn drag_sheet(cx: &mut TestAppContext, window: AnyWindowHandle, editor: &Entity<DrawingEditor>, p: [f64; 2], q: [f64; 2], mid: impl FnOnce(&mut TestAppContext)) {
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    let (a, b) = (at_window(cx, editor, p), at_window(cx, editor, q));
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(a, None, Modifiers::default());
    v.simulate_mouse_down(a, MouseButton::Left, Modifiers::default());
    v.simulate_mouse_move(b, Some(MouseButton::Left), Modifiers::default());
    draw(cx, window);
    mid(cx);
    VisualTestContext::from_window(window, cx).simulate_mouse_up(b, MouseButton::Left, Modifiers::default());
    draw(cx, window);
}

/// The automatic centre mark's centre and its +x arm end (sheet mm).
fn auto_mark(cx: &mut TestAppContext, editor: &Entity<DrawingEditor>) -> ([f64; 2], [f64; 2]) {
    use noshape_model::drawing::{build::PickTarget, sheet::GripKind};
    cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        let pick = b.picks.iter().position(|t| matches!(t, PickTarget::AutoCentermark { .. })).expect("an automatic centre mark") as u32;
        let grips: Vec<_> = b.geometry.grips.iter().filter(|g| g.pick == pick).collect();
        let c = grips.iter().find(|g| g.kind == GripKind::Origin).expect("its centre").pos;
        let end = grips.iter().filter(|g| g.kind == GripKind::Jog).max_by(|a, b| a.pos[0].total_cmp(&b.pos[0])).expect("its arms").pos;
        (c, end)
    })
}

/// Live2 drw-centermark follow-up: a grip dragged from hover; an automatic mark's arms all grow, following the pointer,
/// and never get shorter than the default (a red ⊘, nothing changes); a Single mark's shorten below it too.
#[gpui::test]
fn a_centre_marks_grips_drag_from_hover_and_an_automatic_one_never_gets_shorter(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    HOLED.set(true);
    let (window, editor) = drawing(cx, "feel-cm-drag");
    HOLED.set(false);
    real_sheet(cx, window, &editor);
    let mm_per_px = cx.update(|cx| {
        let e = editor.read(cx);
        let (a, b) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        100.0 / (b[0] - a[0]).abs()
    });
    let (c, end) = auto_mark(cx, &editor);
    let base = end[0] - c[0];
    lit_at(cx, window, &editor, end);
    drag_sheet(cx, window, &editor, end, [end[0] + 30.0 * mm_per_px, end[1]], |_| {});
    let (_, longer) = auto_mark(cx, &editor);
    assert!((longer[0] - c[0] - (base + 30.0 * mm_per_px)).abs() < 0.5 * mm_per_px, "the arms follow the pointer out: {base} -> {}", longer[0] - c[0]);
    lit_at(cx, window, &editor, [5.0, 5.0]);
    let (_, end) = auto_mark(cx, &editor);
    lit_at(cx, window, &editor, end);
    let refused = std::cell::Cell::new(false);
    drag_sheet(cx, window, &editor, end, [c[0] + base - 9.0 * mm_per_px, end[1]], |cx| {
        refused.set(cx.update(|cx| matches!(editor.read(cx).drag, noshape_scene::drawing_tools::Drag::AutoCentermarkArm { refused: true, .. })));
    });
    assert!(refused.get(), "short of the default: refused (the red ⊘)");
    assert_eq!(auto_mark(cx, &editor).1, end, "nothing changed");
}

/// Live2 drw-centermark follow-up (3): a Single mark's grip dragged from hover shortens it below its default too.
#[gpui::test]
fn a_single_centre_marks_grip_shortens_it_from_hover(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    HOLED.set(true);
    SETUP.set(Some(|ctx, dw| {
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let (view, circle) = g["views"].as_array().unwrap().iter().find_map(|v| v["edges"].as_array().unwrap().iter().find(|e| e["type"] == "circle").map(|e| (v["id"].clone(), e.clone()))).expect("the hole end-on");
        call(ctx, "drawing_view_display", json!({ "drawing": dw, "view": view, "auto_centermarks": false }));
        let (c, r) = ([circle["center"][0].as_f64().unwrap(), circle["center"][1].as_f64().unwrap()], circle["radius"].as_f64().unwrap());
        call(ctx, "drawing_centermark", json!({ "drawing": dw, "holes": [[c[0] + r, c[1]]] }));
    }));
    let (window, editor) = drawing(cx, "feel-cm-single");
    HOLED.set(false);
    real_sheet(cx, window, &editor);
    let arm = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let e = editor.read(cx);
            let id = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Centermark { .. })).expect("the Single mark").id;
            let pk = e.pk();
            let (c, dirs, lens) = pk.centermark_arms(&pk.resolved_annotation(id).expect("resolved")).expect("its arms");
            (c, [c[0] + dirs[0][0] * lens[0], c[1] + dirs[0][1] * lens[0]], lens[0])
        })
    };
    let mm_per_px = cx.update(|cx| {
        let e = editor.read(cx);
        let (a, b) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        100.0 / (b[0] - a[0]).abs()
    });
    let (c, end, before) = arm(cx);
    lit_at(cx, window, &editor, end);
    let to = [end[0] - (end[0] - c[0]) / before * 10.0 * mm_per_px, end[1] - (end[1] - c[1]) / before * 10.0 * mm_per_px];
    drag_sheet(cx, window, &editor, end, to, |_| {});
    let (_, _, after) = arm(cx);
    assert!((before - after - 10.0 * mm_per_px).abs() < 0.5 * mm_per_px, "10 px inward from hover: shorter than its default ({before} -> {after})");
}

/// Live2 drw-leader-add-node s10: a right-click on a note's leader selects the note and its menu takes the pointer, so
/// the leader shows selected (orange), not hovered (tan).
#[gpui::test]
fn a_right_click_on_a_leader_selects_its_note_and_drops_the_hover(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
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
        call(ctx, "drawing_note", json!({ "drawing": dw, "text": "TEMP", "position": at, "leader": corner }));
    }));
    let (window, editor) = drawing(cx, "feel-leader-rclick");
    real_sheet(cx, window, &editor);
    let (id, mid) = cx.update(|cx| {
        let e = editor.read(cx);
        let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Note { .. })).expect("the note");
        let (_, path) = e.leader_paths_of(a).into_iter().next().expect("its leader");
        let (p, q) = (path.bend(), *path.points.last().expect("the leader's end"));
        (a.id, [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5])
    });
    assert_eq!(lit_at(cx, window, &editor, mid), Some(Sel::Annotation(id)), "the leader lights its note");
    let p = at_window(cx, &editor, mid);
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_down(p, MouseButton::Right, Modifiers::default());
    v.simulate_mouse_up(p, MouseButton::Right, Modifiers::default());
    draw(cx, window);
    let (sel, hover) = cx.update(|cx| (editor.read(cx).selection.clone(), editor.read(cx).hover));
    assert_eq!(sel, vec![Sel::Annotation(id)], "the note selected");
    assert_eq!(hover, None, "the menu has the pointer: nothing hovered on the sheet");
}

/// Live drawing-feel-1118 (drag2, C2): while a view is dragged only it moves, the views aligned with it follow on
/// release; Esc while dragging leaves the preview until the release, which puts the view back.
#[gpui::test]
fn a_dragged_view_moves_alone_until_release_and_esc_puts_it_back(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-drag-alone");
    real_sheet(cx, window, &editor);
    // The base view and a projected view aligned with it.
    let (base, child, start) = cx.update(|cx| {
        let e = editor.read(cx);
        let s = e.current_sheet();
        // A view whose drag takes another along (across both axes).
        let (base, child) = s
            .views
            .iter()
            .find_map(|v| noshape_model::drawing::dragged_view_positions(s, v.id, [30.0, 30.0]).into_iter().find(|(x, _)| *x != v.id).map(|(x, _)| (v.id, x)))
            .expect("a view with an aligned one");
        let pl = e.built.as_ref().and_then(|b| b.placements.iter().find(|p| p.id == base).copied()).expect("its parent placed");
        (base, child, [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0])
    });
    let pos = |cx: &mut TestAppContext, id| cx.update(|cx| editor.read(cx).current_sheet().view(id).map(|v| v.position)).expect("the view");
    let shown = |cx: &mut TestAppContext, id| cx.update(|cx| {
        let e = editor.read(cx);
        e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet)).and_then(|s| s.view(id)).map(|v| v.position)
    });
    let (b0, c0) = (pos(cx, base), pos(cx, child));
    let p = at_window(cx, &editor, start);
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(p, None, Modifiers::default());
    v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
    // Across both axes, so whichever way the child is aligned it would follow.
    let q = p + Point::new(px(60.), px(60.));
    v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
    draw(cx, window);
    assert_ne!(shown(cx, base), Some(b0), "the dragged view follows the pointer");
    assert_eq!(shown(cx, child).unwrap_or(c0), c0, "the aligned view stays while the button is held");
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
    draw(cx, window);
    assert_ne!(pos(cx, base), b0);
    assert_ne!(pos(cx, child), c0, "the aligned view follows on release");
    assert!(cx.update(|cx| editor.read(cx).selection.is_empty()), "the dragged view ends unselected");
    cx.update(|cx| editor.update(cx, |e, cx| e.doc.update(cx, |d, cx| d.undo(cx))));
    draw(cx, window);
    assert_eq!((pos(cx, base), pos(cx, child)), (b0, c0), "one undo puts both back");
    // Esc while dragging.
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(p, None, Modifiers::default());
    v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
    v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
    // Esc is the workspace's Cancel action (the app's key context), which runs `cancel`.
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    draw(cx, window);
    assert_ne!(shown(cx, base), Some(b0), "the preview stays while the button is held");
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
    draw(cx, window);
    assert_eq!((pos(cx, base), pos(cx, child)), (b0, c0), "the release puts it back");
    assert_eq!(shown(cx, base), None, "no preview left");
}

/// Live drawing-feel-1118 (F): Esc while a box is dragged does nothing and the release still selects; a box over empty
/// sheet keeps what was selected.
#[gpui::test]
fn esc_mid_box_does_nothing_and_an_empty_box_keeps_the_selection(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-box-esc");
    real_sheet(cx, window, &editor);
    let pl = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.first().copied())).expect("a view");
    let pad = 12.0;
    let (a, b) = ([pl.min[0] - pad, pl.min[1] - pad], [pl.max[0] + pad, pl.max[1] + pad]);
    let drag_box = |cx: &mut TestAppContext, a: [f64; 2], b: [f64; 2], esc: bool| {
        cx.executor().advance_clock(std::time::Duration::from_millis(600));
        let (p, q) = (at_window(cx, &editor, a), at_window(cx, &editor, b));
        let mut v = VisualTestContext::from_window(window, cx);
        v.simulate_mouse_move(p, None, Modifiers::default());
        v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
        v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
        if esc {
            cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
        }
        let mut v = VisualTestContext::from_window(window, cx);
        v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
        draw(cx, window);
        cx.update(|cx| editor.read(cx).selection.clone())
    };
    // Another view selected first: Esc mid-box doesn't clear it.
    let other = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.iter().find(|p| p.id != pl.id).map(|p| p.id))).expect("a second view");
    cx.update(|cx| editor.update(cx, |e, _| e.selection = vec![Sel::View(other)]));
    let sel = drag_box(cx, a, b, true);
    assert!(sel.iter().any(|s| view_of(&Some(*s)) == Some(pl.id)), "Esc mid-box: the release still takes the view: {sel:?}");
    assert!(sel.contains(&Sel::View(other)), "Esc mid-box clears nothing: {sel:?}");
    // A box well away from every view, on empty sheet.
    let far = cx.update(|cx| {
        let e = editor.read(cx);
        let ps = &e.built.as_ref().expect("built").placements;
        let top = ps.iter().map(|p| p.max[1]).fold(f64::MIN, f64::max);
        let left = ps.iter().map(|p| p.min[0]).fold(f64::MAX, f64::min);
        ([left, top + 8.0], [left + 10.0, top + 14.0])
    });
    let kept = drag_box(cx, far.0, far.1, false);
    assert_eq!(kept, sel, "an empty box keeps the selection");
}
