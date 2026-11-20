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

/// Live reach-px-1119 (D1 re-read from full-page shots): in Select a view lights and takes a click out to 13 px outside
/// its geometry, not at 14, at the fit (a reach in sheet mm, ours 3.45); beyond it nothing lights and a click takes
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
        for (px_off, lights) in [(-0.5, true), (0.5, false)] {
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

/// qa 11-19 walk: after a Table's ✓ its placement's alignment line and ✕ went on being drawn, the pointer elsewhere.
#[gpui::test]
fn a_tables_alignment_line_goes_with_its_check_mark(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::TableTool;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-table-infer");
    real_sheet(cx, window, &editor);
    let (frame, zoom) = cx.update(|cx| editor.update(cx, |e, cx| {
        e.set_tool(Tool::Table { kind: TableTool::General, view: None, origin: None }, cx);
        (e.current_sheet().size.frame_rect(), e.zoom)
    }));
    draw(cx, window);
    let mid = (frame[1] + frame[3]) / 2.0;
    click_at(cx, window, &editor, [frame[0] + 40.0, mid], Modifiers::default());
    // 3 px off the frame's left side: the table lines up with it.
    VisualTestContext::from_window(window, cx).simulate_mouse_move(at_window(cx, &editor, [frame[0] + 3.0 / zoom, mid]), None, Modifiers::default());
    draw(cx, window);
    assert!(cx.update(|cx| !editor.read(cx).infer_lines.is_empty()), "the table lines up with the frame while placed");
    window.update(cx, |_, w, cx| editor.update(cx, |e, cx| e.close_table_tool(true, w, cx))).unwrap();
    draw(cx, window);
    let e = cx.update(|cx| (editor.read(cx).infer_lines.len(), editor.read(cx).current_sheet().tables.len()));
    assert_eq!(e, (0, 1), "✓: the table placed, no alignment line left");
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

/// Live2 batch 1 (11-18 16:15): with the Dimension tool a circle's centre snaps from 9 px on the diagonal, not from 11
/// (a vertex stays 7 / 9).
#[gpui::test]
fn the_dimension_tool_snaps_to_a_circle_centre_within_9_px(cx: &mut TestAppContext) {
    use noshape_model::drawing::views::SnapKind;
    use noshape_scene::drawing_tools::DimTool;
    SUGGESTED.set(true);
    HOLED.set(true);
    let (window, editor) = drawing(cx, "feel-centre-snap");
    HOLED.set(false);
    real_sheet(cx, window, &editor);
    let (centre, mm_per_px) = cx.update(|cx| {
        let e = editor.read(cx);
        let built = e.built.as_ref().expect("built");
        let centre = built.placements.iter().find_map(|pl| e.geoms.get(&pl.id)?.snaps.iter().find(|s| s.kind == SnapKind::Center).map(|s| pl.to_sheet(s.p))).expect("a circle centre");
        let (a, b) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        (centre, 100.0 / (b[0] - a[0]).abs())
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)));
    draw(cx, window);
    let on_centre = |cx: &mut TestAppContext, d_px: f64| {
        let k = d_px * mm_per_px / std::f64::consts::SQRT_2;
        lit_at(cx, window, &editor, [centre[0] - k, centre[1] - k]);
        cx.update(|cx| editor.read(cx).hover_snap).is_some_and(|s| (s[0] - centre[0]).abs() < 1e-6 && (s[1] - centre[1]).abs() < 1e-6)
    };
    for d in [0.0, 5.0, 9.0] {
        assert!(on_centre(cx, d), "{d} px from the centre on the diagonal: snapped");
    }
    assert!(!on_centre(cx, 11.0), "11 px from the centre: no snap");
}

/// Live2 batch 1 (11-18 16:15): a note's leader lights the note to 8 px off its stroke, not at 10 (11-13 had 10 / 12).
#[gpui::test]
fn a_note_leader_lights_its_note_within_8_px(cx: &mut TestAppContext) {
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
    for (d, lit) in [(0.0, true), (6.0, true), (8.0, true), (10.0, false)] {
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

/// Live2 place-1118: placing a one-edge dimension (ANSI), its value follows the pointer along the line, inside the
/// extension lines too; against an extension line both arrows flip outside, pointing in; past an end the line is carried
/// out under the value, arrows inside.
#[gpui::test]
fn a_placed_dimensions_value_follows_the_pointer_and_crowds_its_arrows_out(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    use noshape_scene::drawing_tools::DimTool;
    SUGGESTED.set(true);
    ANSI.set(true);
    let (window, editor) = drawing(cx, "feel-dim-follow");
    real_sheet(cx, window, &editor);
    let (l, r, y) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let ends = |x: &noshape_model::drawing::views::ViewEdge| (pl.to_sheet(x.points[0]), pl.to_sheet(x.points[1]));
        let top = g.edges.iter().filter(|x| x.visible && x.points.len() == 2).map(ends).filter(|(a, b)| (a[1] - b[1]).abs() < 1e-6).max_by(|a, b| (a.0[1] + (a.1[0] - a.0[0]).abs() * 1e-3).total_cmp(&(b.0[1] + (b.1[0] - b.0[0]).abs() * 1e-3))).expect("a horizontal edge");
        (top.0[0].min(top.1[0]), top.0[0].max(top.1[0]), top.0[1])
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)));
    draw(cx, window);
    click_at(cx, window, &editor, [(l + r) / 2.0, y], Modifiers::default());
    // The value at x, the line 15 mm above the edge: (text along, the line's leftmost and rightmost x).
    let place = |cx: &mut TestAppContext, x: f64| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = [x, y + 15.0];
                e.update_preview(cx);
            })
        })
        .unwrap();
        draw(cx, window);
        cx.update(|cx| {
            let e = editor.read(cx);
            let a = e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet)).and_then(|s| s.annotations.last()).cloned().expect("the dimension previewed");
            let b = e.built.as_ref().expect("built");
            let pick = b.picks.iter().position(|t| *t == PickTarget::Annotation(a.id)).expect("drawn") as u32;
            let xs: Vec<f64> = b.geometry.lines.iter().filter(|q| q.pick == pick && q.points.len() == 2 && (q.points[0][1] - q.points[1][1]).abs() < 1e-9).flat_map(|q| [q.points[0][0], q.points[1][0]]).collect();
            (a.text_along, xs.iter().copied().fold(f64::MAX, f64::min), xs.iter().copied().fold(f64::MIN, f64::max))
        })
    };
    let mid = (l + r) / 2.0;
    let (along, lo, _) = place(cx, mid + 8.0);
    assert!(along.is_some_and(|s| (s - 8.0).abs() < 1e-6), "inside, clear of the ends: at the pointer, {along:?}");
    assert!((lo - l).abs() < 1e-6, "arrows inside: the line starts at the extension line, {lo} vs {l}");
    let (along, lo, _) = place(cx, l + 2.0);
    assert!(along.is_some_and(|s| (s - (l + 2.0 - mid)).abs() < 1e-6), "against the left end: at the pointer, {along:?}");
    assert!(lo < l - 1.0, "arrows flipped outside: the line runs past the extension line, {lo} vs {l}");
    let (along, lo, hi) = place(cx, r + 15.0);
    assert!(along.is_some_and(|s| (s - (r + 15.0 - mid)).abs() < 1e-6), "past the right end: at the pointer, {along:?}");
    assert!((lo - l).abs() < 1e-6 && hi > r + 5.0, "arrows inside, the line carried out under the value: {lo}..{hi} vs {l}..{r}");
}

/// Live2 place-1118: a projected view being placed is drawn grey rgb(159,159,159) in a 1 px rgb(154,165,175) frame;
/// the views already there keep their ink.
#[gpui::test]
fn a_projected_views_preview_is_grey_in_a_thin_frame(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-projected-ghost");
    real_sheet(cx, window, &editor);
    let (base, at) = cx.update(|cx| {
        let e = editor.read(cx);
        let s = e.current_sheet();
        let v = s.views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).expect("a base view");
        (v.id, [v.position[0] + 90.0, v.position[1]])
    });
    cx.update_window(window, |_, _, cx| {
        editor.update(cx, |e, cx| {
            e.set_tool(Tool::Projected { parent: Some(base) }, cx);
            e.cursor = at;
            e.update_preview(cx);
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update(|cx| {
        let e = editor.read(cx);
        let id = e.placing_view().expect("the projected view previewed");
        assert!(e.placement(id).is_some(), "its box framed");
        let b = e.built.as_ref().expect("built");
        let ghost = crate::drawing_view::ghost_picks(b, id);
        assert!(b.geometry.lines.iter().any(|l| ghost.contains(&l.pick)), "the preview has geometry");
        let lines = crate::drawing_scene::sheet_lines(b, e.zoom, &Default::default(), &[], (&ghost, crate::drawing_scene::GHOST_INK), false);
        let grey = lines.segments.iter().filter(|s| s.color == crate::drawing_scene::GHOST_INK).count();
        let ink = lines.segments.iter().filter(|s| s.color != crate::drawing_scene::GHOST_INK).count();
        assert!(grey > 0 && ink > 0, "the preview grey ({grey}), the other views as they are ({ink})");
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Select, cx)));
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).placing_view().is_none()), "no preview once the tool ends");
}

/// Live2 place-1118: Section view (Vertical): a click away from an inference point only picks the view (the cutting line
/// then follows the pointer), another such click does nothing, and a click on an edge's midpoint fixes the cut.
#[gpui::test]
fn a_vertical_sections_cut_is_fixed_only_on_an_inference_point(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    use noshape_model::drawing::views::EdgeGeom;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-section-infer");
    real_sheet(cx, window, &editor);
    let (inside, mid) = cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.id).expect("a base view");
        let pl = e.placement(v).expect("placed");
        let g = e.geoms.get(&v).cloned().expect("geometry");
        let m = g.model_edges.iter().find_map(|me| match me.geom {
            EdgeGeom::Line { a, b } => Some(pl.to_sheet([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])),
            _ => None,
        });
        ([(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0], m.expect("a straight edge"))
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Section { parent: None, p0: None, line: None, partial: false, jogs: vec![] }, cx)));
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).section_axis().is_some()), "Vertical by default");
    let state = |cx: &mut TestAppContext| cx.update(|cx| match &editor.read(cx).tool {
        Tool::Section { parent, p0, line, .. } => (parent.is_some(), p0.is_some(), line.is_some()),
        _ => (false, false, false),
    });
    click_at(cx, window, &editor, inside, Modifiers::default());
    assert_eq!(state(cx), (true, false, false), "away from an inference point: the view picked, no cut");
    click_at(cx, window, &editor, [inside[0] + 1.0, inside[1]], Modifiers::default());
    assert_eq!(state(cx), (true, false, false), "again away from one: nothing");
    click_at(cx, window, &editor, mid, Modifiers::default());
    assert_eq!(state(cx), (true, true, true), "on an edge's midpoint: the cut fixed");
}

/// Live2 place-1118: a projected view is the orthographic one within 22.5° of the parent's axes, locked to its row
/// (only the pointer's x moves it), and the isometric past that, on the pointer: dx 173 px: dy 60 locked, 80 iso; dx
/// 373 px: 140 locked, 160 iso.
#[gpui::test]
fn a_projected_view_locks_within_22_5_degrees_and_turns_isometric_past(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-projected-lock");
    real_sheet(cx, window, &editor);
    let (base, centre, mm_per_px) = cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).expect("a base view");
        let (a, b) = (e.sheet_to_local([0.0, 0.0]), e.sheet_to_local([100.0, 0.0]));
        (v.id, v.position, 100.0 / (b[0] - a[0]).abs())
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Projected { parent: Some(base) }, cx)));
    let at = |cx: &mut TestAppContext, dx: f64, dy: f64| {
        let p = [centre[0] + dx * mm_per_px, centre[1] + dy * mm_per_px];
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = p;
                e.update_preview(cx);
            })
        })
        .unwrap();
        let v = cx.update(|cx| editor.read(cx).pending_view()).expect("the view previewed");
        let Some(dir) = (match v.kind {
            ViewKind::Projected { direction, .. } => direction,
            _ => None,
        }) else {
            panic!("a projected view")
        };
        (dir[0] != 0.0 && dir[1] != 0.0, v.position, p)
    };
    for (dx, dy, iso) in [(173.0, 60.0, false), (173.0, 80.0, true), (373.0, 140.0, false), (373.0, 160.0, true)] {
        let (got_iso, pos, p) = at(cx, dx, dy);
        assert_eq!(got_iso, iso, "dx {dx} px, dy {dy} px: isometric {got_iso}");
        if iso {
            assert!((pos[0] - p[0]).abs() < 1e-6 && (pos[1] - p[1]).abs() < 1e-6, "the isometric on the pointer: {pos:?} vs {p:?}");
        } else {
            assert!((pos[1] - centre[1]).abs() < 1e-6 && (pos[0] - p[0]).abs() < 1e-6, "locked to the parent's row, x on the pointer: {pos:?}");
        }
    }
}

/// Live2 place-1118: a detail view being placed is a grey preview with no label, its profile not yet on the parent,
/// where the overlay keeps the plain solid circle.
#[gpui::test]
fn a_detail_views_preview_is_grey_with_the_parents_circle_plain(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-detail-ghost");
    real_sheet(cx, window, &editor);
    let (base, centre) = cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).expect("a base view");
        (v.id, v.position)
    });
    cx.update_window(window, |_, _, cx| {
        editor.update(cx, |e, cx| {
            e.set_tool(Tool::Detail { parent: Some(base), center: Some([0.0, 0.0]), radius: Some(0.005) }, cx);
            e.cursor = [centre[0] + 80.0, centre[1]];
            e.update_preview(cx);
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update(|cx| {
        let e = editor.read(cx);
        let id = e.placing_view().expect("the detail previewed");
        let v = e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet)).and_then(|s| s.view(id)).cloned().expect("in the preview");
        assert!(v.hide_label && v.detail_hide_profile, "no label, no profile on the parent yet");
        assert!(e.overlay.tinted.iter().any(|(pts, rgb, _)| pts.len() > 60 && *rgb == [0, 0, 0, 255]), "the parent's plain solid circle");
    });
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
    // The hole table's X direction pick is the same PICKBOX (getIntersected for every tool), it had 5 px.
    let view = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.first().map(|p| p.id))).expect("a view");
    for (px, picks) in [(6.0, true), (9.0, false)] {
        let at = off(cx, px);
        let dir = cx.update(|cx| {
            editor.update(cx, |e, _| {
                e.cursor = at;
                e.straight_edge_dir_near(view)
            })
        });
        assert_eq!(dir.is_some(), picks, "X direction {px} px off the edge");
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

/// Live2 hatch-list-1118: the Pattern list opens 1 px under its field, 2 px wider, 30 px rows, as tall as its rows up to
/// 341 px and then scrolls (ANSI's 9 rows 272 px, General's 31 at 341).
#[gpui::test]
fn the_hatch_pattern_list_opens_under_its_field_and_scrolls_past_341_px(cx: &mut TestAppContext) {
    for (standard, height) in [(0, 272.), (2, 341.)] {
        let (window, editor) = drawing(cx, &format!("feel-hatch-list-{standard}"));
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_hatch_dialog(None, w, cx))).unwrap();
        cx.update(|cx| {
            editor.update(cx, |e, cx| {
                if let Some(d) = &mut e.hatch_dlg {
                    d.standard = standard;
                }
                cx.notify();
            })
        });
        draw(cx, window);
        let field = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("hd-pattern-field").expect("the Pattern field");
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(field.center(), Modifiers::default());
        draw(cx, window);
        // (The bounds inside its 1 px border.)
        let list = cx.update(|cx| crate::os_menu::open_bounds(cx)).expect("the list open").dilate(px(1.));
        assert!((f32::from(list.top() - field.bottom()) - 1.).abs() < 0.5 && (list.left() - field.left()).abs() < px(0.5), "1 px under the field: {list:?} / {field:?}");
        assert!((f32::from(list.size.width) - 190.).abs() < 0.5, "190 px wide: {list:?}");
        assert!((f32::from(list.size.height) - height).abs() < 0.5, "standard {standard}: {height} px tall: {list:?}");
    }
}

/// Live2 toolbar-help-1118: a toolbar tool's help shows about 3.1 s after the pointer rests on it, 400 px wide at the
/// editor's left edge just under the toolbar, and goes when the pointer leaves; Undo has none.
#[gpui::test]
fn a_toolbar_tools_help_shows_after_3_s_under_the_toolbar(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "feel-tool-help");
    let shown = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("tool-help");
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.tool_hover("Projected view (p)", true, w, cx))).unwrap();
    // Its title tooltip at 450 ms (live 0.45–0.69 s, median 0.47), read with the help's text as live's recorder does.
    let read = |cx: &mut TestAppContext| {
        crate::ui_script::clear_tooltip();
        cx.update_window(window, |_, w, _| w.refresh()).unwrap();
        draw(cx, window);
        crate::ui_script::shown_tooltip()
    };
    cx.executor().advance_clock(std::time::Duration::from_millis(440));
    assert_eq!(read(cx), None, "no tooltip at 440 ms");
    // The editor redraws when it shows (read on its next frame, not on some later one).
    let notified = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = notified.clone();
    let _watch = cx.update(|cx| cx.observe(&editor, move |_, _| seen.set(true)));
    cx.executor().advance_clock(std::time::Duration::from_millis(20));
    cx.run_until_parked();
    assert!(notified.get(), "redrawn at 450 ms");
    assert!(read(cx).is_some_and(|t| t.starts_with("Projected view (p) / Projected view (p) ")), "the tooltip at 460 ms");
    cx.executor().advance_clock(std::time::Duration::from_millis(2440));
    draw(cx, window);
    assert!(shown(cx).is_none(), "not yet at 2.9 s");
    cx.executor().advance_clock(std::time::Duration::from_millis(300));
    draw(cx, window);
    // (The editor fills the test window.)
    let b = shown(cx).expect("shown by 3.2 s");
    assert_eq!((b.left(), b.top(), b.size.width), (px(0.), px(crate::ui::TOOLBAR_H + 1.), px(400.)), "{b:?}");
    let h = crate::drawing_help::help("Projected view (p)").expect("Projected view's help");
    assert_eq!(h.steps.len(), 3, "live's three steps");
    assert_eq!(crate::ui_script::shown_tooltip(), Some(format!("Projected view (p) / Projected view (p) {} {}", h.intro, h.steps.join(" "))));
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.tool_hover("Projected view (p)", false, w, cx))).unwrap();
    draw(cx, window);
    assert!(shown(cx).is_none(), "gone when the pointer leaves");
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.tool_hover("Undo (ctrl+z)", true, w, cx))).unwrap();
    cx.executor().advance_clock(std::time::Duration::from_millis(4000));
    draw(cx, window);
    assert!(shown(cx).is_none(), "Undo has none");
}

/// Live2 drw-detail-id-profile-tips (s7–s19): Edit detail… reads no tool and lights the detail's circle in its parent;
/// the profile chip's × asks for a closed profile to pick ("Select closed profile", "No closed profile selected.", no
/// tool, the parent no longer lit), and Esc then closes the panel.
#[gpui::test]
fn edit_detail_clear_profile_picks_a_region_and_esc_closes(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let front = g["views"].as_array().and_then(|v| v.iter().find(|v| v["orientation"] == "front").cloned()).expect("Front");
        call(ctx, "drawing_detail_view", json!({ "drawing": dw, "parent": front["id"], "center": [0, 0], "radius": 8, "position": [300, 200] }));
    }));
    let (window, editor) = drawing(cx, "feel-detail-edit-clear");
    let (detail, parent) = cx.update(|cx| {
        let s = editor.read(cx).current_sheet();
        s.views.iter().find_map(|v| match v.kind { noshape_model::drawing::ViewKind::Detail { parent, .. } => Some((v.id, parent)), _ => None }).expect("the detail")
    });
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_detail_edit(detail, w, cx))).unwrap();
    assert!(cx.update(|cx| editor.read(cx).detail_dlg.as_ref().is_some_and(|d| d.edit == Some(detail) && d.view == Some(parent))), "the panel open on the detail");
    draw(cx, window);
    let after = cx.update(|cx| editor.read(cx).detail_dlg.as_ref().map(|d| (d.edit, d.view, d.pick_region)));
    assert_eq!(after, Some((Some(detail), Some(parent), false)), "still open after a frame");
    let state = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).ui_state(cx));
    let parent_name = cx.update(|cx| editor.read(cx).ui_view_name(parent));
    let s = state(cx);
    assert_eq!(s["drawing"]["tool"], Value::Null, "no tool while editing");
    assert!(s["drawing"]["selection"].as_array().is_some_and(|a| a.iter().any(|x| x["kind"] == "EDGE" && x["owner"] == parent_name.as_str())), "the parent lit: {s}");
    press(cx, window, "dd-profile-clear");
    let s = state(cx);
    assert_eq!(s["drawing"]["tool"], Value::Null, "still no tool");
    assert_eq!(s["drawing"]["selection"], json!([]), "the parent no longer lit");
    assert!(cx.update(|cx| editor.read(cx).detail_dlg.as_ref().is_some_and(|d| d.pick_region)));
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).detail_dlg.is_none()), "Esc closes the panel");
}

/// Live2 drw-centermark (behaviours "studios (11-17, P0 drawing feel)"): a hole's centre mark lights from its circle, its
/// centre or an arm (its view not framed), its circle light blue, its grips shown; hovering the view alone shows none;
/// with the view selected no grips, an orange dot at its centre and the circle the view's orange (live2 drw-tools-1119
/// centermark-sel); a press on the circle drags the view.
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
        let l = looks(cx);
        assert!(!l.grips.is_empty() && l.circles == circle, "{what}: its grips, its circle light blue ({l:?})");
        assert!(frames(cx).is_empty(), "{what}: the view not framed");
    }
    let blank = [c[0] + reach + 12.0 * mm_per_px, c[1] + reach + 12.0 * mm_per_px];
    assert_eq!(view_of(&lit_at(cx, window, &editor, blank)), Some(view), "the view's blank inside lights the view");
    assert_eq!(looks(cx), Default::default(), "the view hovered: no grips");
    assert_eq!(click_at(cx, window, &editor, on_arm, Modifiers::default()), vec![mark], "a click on an arm takes the centre mark");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    let l = looks(cx);
    assert!(!l.grips.is_empty() && l.circles == circle, "selected: grips, the circle light blue");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    assert_eq!(click_at(cx, window, &editor, on_circle, Modifiers::default()), vec![mark], "a click on the circle takes the centre mark too");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    assert_eq!(click_at(cx, window, &editor, blank, Modifiers::default()), vec![Sel::View(view)], "a click inside takes the view");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    let l = looks(cx);
    assert!(l.grips.is_empty() && !l.dots.is_empty() && l.circles.is_empty(), "the view selected: no grips, the centre dot, the circle the view's orange ({l:?})");
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

/// Live2 place-1118: Auxiliary view: the view being placed is a grey preview in the thin frame with no letters and no
/// label, its fold line midway between the parent's edge and the preview's frame; with the pointer over the parent the
/// preview sits on it.
#[gpui::test]
fn an_auxiliary_views_preview_is_grey_framed_without_letters_and_shows_over_its_parent(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-aux-ghost");
    real_sheet(cx, window, &editor);
    let (base, centre, edge) = cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).expect("a base view");
        let g = e.geoms.get(&v.id).cloned().expect("geometry");
        (v.id, v.position, [g.max[0], (g.min[1] + g.max[1]) / 2.0])
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Auxiliary { parent: None }, cx)));
    draw(cx, window);
    let place = |cx: &mut TestAppContext, at: [f64; 2]| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                if let Some(d) = &mut e.aux_dlg {
                    d.anchor = Some(edge);
                }
                e.tool = Tool::Auxiliary { parent: Some((base, [0.0, 1.0])) };
                e.cursor = at;
                e.update_preview(cx);
            })
        })
        .unwrap();
        draw(cx, window);
    };
    // Twice: the second preview takes the first one's box for its fold line.
    place(cx, [centre[0] + 90.0, centre[1]]);
    place(cx, [centre[0] + 90.0, centre[1]]);
    cx.update(|cx| {
        let e = editor.read(cx);
        let id = e.placing_view().expect("the auxiliary view previewed");
        let v = e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet)).and_then(|s| s.view(id)).cloned().expect("in the preview");
        let a = v.aux_ref.clone().expect("its reference indicator");
        assert!(v.hide_label && a.letter.is_empty(), "no label, no letters");
        let (pl, ppl) = (e.placement(id).expect("framed"), e.placement(base).expect("the parent"));
        let (edge_x, frame_x) = (ppl.to_sheet(edge)[0], pl.min[0] - e.tol_mm(8.0));
        let off = a.offset.expect("the fold line placed");
        assert!((edge_x + off - (edge_x + frame_x) / 2.0).abs() < 0.05, "midway: edge {edge_x}, frame {frame_x}, line at {}", edge_x + off);
    });
    place(cx, centre);
    assert!(cx.update(|cx| editor.read(cx).placing_view().is_some()), "over the parent the preview shows");
}

/// Live2 place-1118: Break view: a zigzag break line at the pointer over the view (none off it), each click places
/// one, and after the second the view shows broken at once, before ✓.
#[gpui::test]
fn a_break_line_follows_the_pointer_and_two_clicks_break_the_view_at_once(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-break-preview");
    real_sheet(cx, window, &editor);
    let (base, pl) = cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.id).expect("a base view");
        (v, e.placement(v).expect("placed"))
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Break { parent: None, first: None, vertical: true }, cx)));
    draw(cx, window);
    let x = |t: f64| [pl.min[0] + (pl.max[0] - pl.min[0]) * t, (pl.min[1] + pl.max[1]) / 2.0];
    let zigzags = |cx: &mut TestAppContext, at: [f64; 2]| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = at;
                e.update_preview(cx);
            })
        })
        .unwrap();
        cx.update(|cx| editor.read(cx).overlay.tinted.iter().filter(|(pts, rgb, _)| pts.len() == 6 && *rgb == [0, 0, 0, 255]).count())
    };
    assert_eq!(zigzags(cx, x(0.3)), 1, "a zigzag at the pointer over the view");
    assert_eq!(zigzags(cx, [pl.max[0] + 30.0, pl.max[1] + 30.0]), 0, "none off it");
    click_sheet(cx, window, &editor, x(0.3));
    assert_eq!(zigzags(cx, x(0.7)), 2, "the first one placed and one at the pointer");
    click_sheet(cx, window, &editor, x(0.7));
    cx.update(|cx| {
        let e = editor.read(cx);
        assert!(e.drawing_sheet().view(base).is_some_and(|v| v.breaks.is_empty()), "not applied before ✓");
        let shown = e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet)).and_then(|s| s.view(base)).map(|v| v.breaks.len());
        assert_eq!(shown, Some(1), "the view broken in the preview");
        assert_eq!(e.placement(base).map(|p| p.nbreaks), Some(1), "and drawn broken");
    });
}

/// Live2 place-1118 (pin-block-asm-drawing): placing a BOM table, its fixed corner (top right) snaps to the border
/// frame's corner with a hollow orange square from 5 and 8.6 px, not 12.8; the table under the pointer is not lit (live
/// draws it in black lines).
#[gpui::test]
fn a_placed_bom_tables_corner_snaps_to_the_frames_corner_within_10_px(cx: &mut TestAppContext) {
    use crate::drawing_scene::SnapMark;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-bom-corner");
    real_sheet(cx, window, &editor);
    let corner = cx.update(|cx| {
        editor.update(cx, |e, _| {
            e.sources.values_mut().for_each(|s| s.assembly = true);
            e.tool = Tool::Table { kind: noshape_scene::drawing_tools::TableTool::Bom, view: None, origin: None };
            e.sync_table_dlg();
            let [_, _, x1, y1] = e.current_sheet().size.frame_rect();
            [x1, y1]
        })
    });
    draw(cx, window);
    for (px_off, snaps) in [(5.0, true), (8.6, true), (12.8, false)] {
        let mm = cx.update(|cx| editor.read(cx).tol_mm(px_off)) / std::f64::consts::SQRT_2;
        let at = at_window(cx, &editor, [corner[0] - mm, corner[1] - mm]);
        VisualTestContext::from_window(window, cx).simulate_mouse_move(at, None, Modifiers::default());
        draw(cx, window);
        cx.update(|cx| {
            let e = editor.read(cx);
            assert!(e.table_dlg_preview().is_some(), "the table follows the pointer");
            let snapped = (e.cursor[0] - corner[0]).hypot(e.cursor[1] - corner[1]) < 1e-9;
            assert_eq!(snapped, snaps, "{px_off} px off the corner: at {:?}", e.cursor);
            let mark = e.overlay.marks.iter().any(|(p, k, _)| *k == SnapMark::Box && *p == corner);
            assert_eq!(mark, snaps, "{px_off} px: the orange square");
            let lit = e.hover.and_then(|h| e.built.as_ref().and_then(|b| b.picks.get(h as usize)).cloned());
            assert!(!matches!(lit, Some(noshape_model::drawing::build::PickTarget::Table(_))), "the table being placed is not lit: {lit:?}");
        });
    }
}

/// Live2 place-1118: Crop view (Spline): the profile being drawn is black and open through the points to the pointer,
/// with a red ✕ and a pink inference line while the pointer is in line with the first point; it closes onto the first point at 5 and 3 px, not at 8,
/// and a click there ends it.
#[gpui::test]
fn a_crop_spline_is_drawn_open_and_closes_within_6_px_of_its_first_point(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-crop-spline");
    real_sheet(cx, window, &editor);
    let pl = cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.id).expect("a base view");
        e.placement(v).expect("placed")
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.start_crop(cx)));
    draw(cx, window);
    let at = |u: f64, v: f64| [pl.min[0] + (pl.max[0] - pl.min[0]) * u, pl.min[1] + (pl.max[1] - pl.min[1]) * v];
    let first = at(0.3, 0.6);
    for p in [first, at(0.7, 0.65), at(0.6, 0.3)] {
        click_sheet(cx, window, &editor, p);
    }
    let look = |cx: &mut TestAppContext, p: [f64; 2]| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = p;
                e.hover_snap = None;
                e.update_preview(cx);
            })
        })
        .unwrap();
        cx.update(|cx| {
            let e = editor.read(cx);
            let curve = e.overlay.tinted.iter().find(|(_, rgb, _)| *rgb == [0, 0, 0, 255]).map(|(pts, _, _)| pts.clone()).expect("the black profile");
            let (a, b) = (curve[0], curve[curve.len() - 1]);
            ((a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9, !e.overlay.crosses.is_empty() && !e.overlay.infer.is_empty())
        })
    };
    let px = |cx: &mut TestAppContext, d: f64| {
        let mm = cx.update(|cx| editor.read(cx).tol_mm(d));
        [first[0] + mm, first[1]]
    };
    let far = at(0.4, 0.2);
    assert_eq!(look(cx, far), (false, false), "open to the pointer; out of line with the first point: no ✕, no line");
    // In line with the first point within 8 px (live2 wake-1118: 0–8 yes, 12 no): the line from it to a ✕ where the
    // pointer snaps onto it.
    for (d, inline) in [(8.0, true), (12.0, false)] {
        let mm = cx.update(|cx| editor.read(cx).tol_mm(d));
        let p = [far[0], first[1] + mm];
        assert_eq!(look(cx, p).1, inline, "{d} px off the first point's y");
        let end = cx.update(|cx| editor.read(cx).overlay.tinted.iter().find(|(_, rgb, _)| *rgb == [0, 0, 0, 255]).and_then(|(pts, _, _)| pts.last().copied()));
        assert_eq!(end.is_some_and(|q| (q[1] - first[1]).abs() < 1e-4), inline, "{d} px: the profile ends on the line: {end:?} for {first:?}");
    }
    for (d, closed) in [(12.0, false), (8.0, false), (5.0, true), (3.0, true)] {
        let p = px(cx, d);
        assert_eq!(look(cx, p).0, closed, "{d} px from the first point");
    }
    let p = px(cx, 3.0);
    click_sheet(cx, window, &editor, p);
    assert!(cx.update(|cx| editor.read(cx).crop_dlg.as_ref().is_some_and(|d| d.pending.is_some())), "the click on the first point ends it");
}

/// Live2 ann-1118: the 2 point centerline frames a view's vertex in the Note tool's hollow orange square, lit 9 px off it
/// on the diagonal and not at 10; the Dimension tool keeps its disc and 8 px.
#[gpui::test]
fn the_two_point_centerline_frames_a_vertex_from_9_px(cx: &mut TestAppContext) {
    use crate::drawing_scene::SnapMark;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-centerline-snap");
    real_sheet(cx, window, &editor);
    let corner = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        g.snaps.iter().map(|s| pl.to_sheet(s.p)).min_by(|a, b| (a[0] + a[1]).total_cmp(&(b[0] + b[1]))).expect("a vertex")
    });
    let mark_at = |cx: &mut TestAppContext, tool: Tool, px: f64| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.tool = tool;
                let mm = e.tol_mm(px) * std::f64::consts::FRAC_1_SQRT_2;
                e.cursor = [corner[0] - mm, corner[1] - mm];
                e.update_preview(cx);
            })
        })
        .unwrap();
        cx.update(|cx| editor.read(cx).overlay.marks.iter().find(|(p, _, _)| (p[0] - corner[0]).hypot(p[1] - corner[1]) < 1e-9).map(|m| m.1))
    };
    let centerline = Tool::Centerline { first: None };
    assert_eq!(mark_at(cx, centerline.clone(), 9.0), Some(SnapMark::Box), "9 px: the square");
    assert_eq!(mark_at(cx, centerline, 10.0), None, "10 px: none");
    let dim = Tool::Dimension { kind: noshape_scene::drawing_tools::DimTool::Linear, picks: vec![] };
    assert_eq!(mark_at(cx, dim.clone(), 7.0), Some(SnapMark::Disc), "the Dimension tool's disc");
    assert_eq!(mark_at(cx, dim, 9.0), None, "within its 8 px only");
}

/// A base view's longest horizontal straight edge on the sheet (its ends) and the way out of the view from it.
fn horizontal_edge(cx: &mut TestAppContext, editor: &Entity<DrawingEditor>) -> ([f64; 2], [f64; 2], f64) {
    use noshape_model::drawing::ViewKind;
    use noshape_model::drawing::views::EdgeGeom;
    cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.id).expect("a base view");
        let pl = e.placement(v).expect("placed");
        let g = e.geoms.get(&v).cloned().expect("geometry");
        let (a, b) = g
            .model_edges
            .iter()
            .filter_map(|me| match me.geom {
                EdgeGeom::Line { a, b } => Some((pl.to_sheet(a), pl.to_sheet(b))),
                _ => None,
            })
            .filter(|(a, b)| (a[1] - b[1]).abs() < 1e-6)
            .max_by(|x, y| (x.1[0] - x.0[0]).abs().total_cmp(&(y.1[0] - y.0[0]).abs()))
            .expect("a horizontal edge");
        let (a, b) = if a[0] < b[0] { (a, b) } else { (b, a) };
        let out = if a[1] > (pl.min[1] + pl.max[1]) / 2.0 { 1.0 } else { -1.0 };
        (a, b, out)
    })
}

/// Live2 ann-1118: the Datum tool lights the hovered edge; after the edge click the box rides a leader square to the
/// edge, its foot under the pointer and kept within the edge's ends (no rubber band).
#[gpui::test]
fn a_datums_leader_stays_square_to_its_edge_within_the_edges_ends(cx: &mut TestAppContext) {
    use noshape_model::drawing::AnnotationKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-datum-square");
    real_sheet(cx, window, &editor);
    let (a, b, out) = horizontal_edge(cx, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Datum { anchor: None }, cx)));
    draw(cx, window);
    let mid = [(a[0] + b[0]) / 2.0, a[1]];
    assert!(matches!(lit_at(cx, window, &editor, mid), Some(Sel::Edge(..))), "the hovered edge lit");
    click_sheet(cx, window, &editor, mid);
    let datum_at = |cx: &mut TestAppContext, p: [f64; 2]| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = p;
                e.update_preview(cx);
            })
        })
        .unwrap();
        cx.update(|cx| {
            let e = editor.read(cx);
            assert!(e.overlay.lines.is_empty(), "no rubber band");
            let ann = e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet)).and_then(|s| s.annotations.last()).cloned().expect("the datum previewed");
            let AnnotationKind::Datum { anchor, .. } = ann.kind else { panic!("a datum: {:?}", ann.kind) };
            (anchor, ann.position)
        })
    };
    let near = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-6;
    let over = [a[0] + (b[0] - a[0]) * 0.3, a[1] + out * 20.0];
    let (foot, box_at) = datum_at(cx, over);
    assert!(near(foot, [over[0], a[1]]) && near(box_at, over), "the foot under the pointer: {foot:?} / {box_at:?}");
    let past = [b[0] + 30.0, a[1] + out * 15.0];
    let (foot, box_at) = datum_at(cx, past);
    assert!(near(foot, b) && near(box_at, [b[0], past[1]]), "past the end: the edge's end, square to it: {foot:?} / {box_at:?}");
}

/// Live2 ann-1118: the Surface finish tool previews the symbol at the pointer, standing on the hovered edge; a click
/// places it at once and the next preview follows the pointer.
#[gpui::test]
fn the_surface_finish_symbol_follows_the_pointer_before_and_after_a_click(cx: &mut TestAppContext) {
    use noshape_model::drawing::AnnotationKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-sf-preview");
    real_sheet(cx, window, &editor);
    let (a, b, out) = horizontal_edge(cx, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::SurfaceFinish, cx)));
    draw(cx, window);
    let shown = |cx: &mut TestAppContext, p: [f64; 2]| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = p;
                e.update_hover(cx);
                e.update_preview(cx);
            })
        })
        .unwrap();
        cx.update(|cx| {
            let e = editor.read(cx);
            let have = e.drawing_sheet().annotations.len();
            let s = e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet))?;
            (s.annotations.len() == have + 1).then(|| s.annotations.last().cloned()).flatten().and_then(|x| match x.kind {
                AnnotationKind::SurfaceFinish { on, .. } => Some((x.position, on.is_some())),
                _ => None,
            })
        })
    };
    let mid = [(a[0] + b[0]) / 2.0, a[1]];
    assert_eq!(shown(cx, mid), Some((mid, true)), "on the hovered edge, standing on it");
    click_sheet(cx, window, &editor, mid);
    let placed = cx.update(|cx| editor.read(cx).drawing_sheet().annotations.iter().filter(|x| matches!(x.kind, AnnotationKind::SurfaceFinish { .. })).count());
    assert_eq!(placed, 1, "placed at once");
    let free = [mid[0] + 25.0, a[1] + out * 25.0];
    assert_eq!(shown(cx, free).map(|s| s.0), Some(free), "the next symbol at the pointer");
}

/// Live2 gtol-1119 (as the 11-07 take; ann-1118's leader didn't reproduce): a click on the lit edge places a ⌖ frame
/// right there, no leader, moving with the view; the next click places a second frame; both stay.
#[gpui::test]
fn a_geometric_tolerance_clicked_on_an_edge_is_placed_there_and_the_next_click_places_another(cx: &mut TestAppContext) {
    use noshape_model::drawing::AnnotationKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-gdt-edge");
    real_sheet(cx, window, &editor);
    let (a, b, out) = horizontal_edge(cx, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Gdt, cx)));
    draw(cx, window);
    let gdts = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).drawing_sheet().annotations.iter().filter(|x| matches!(x.kind, AnnotationKind::Gdt { .. })).cloned().collect::<Vec<_>>());
    let mid = [(a[0] + b[0]) / 2.0, a[1]];
    click_sheet(cx, window, &editor, mid);
    let placed = gdts(cx);
    assert!(matches!(placed.as_slice(), [g] if matches!(&g.kind, AnnotationKind::Gdt { leaders, .. } if leaders.is_empty()) && g.view_group.is_some()), "one frame on the edge, no leader, with its view: {placed:?}");
    assert!((placed[0].position[0] - mid[0]).hypot(placed[0].position[1] - mid[1]) < 0.05, "at the click: {:?} for {mid:?}", placed[0].position);
    assert!(cx.update(|cx| editor.read(cx).note_leader.is_none()), "no leader anchored");
    let above = [mid[0], a[1] + out * 8.0];
    click_sheet(cx, window, &editor, above);
    assert_eq!(gdts(cx).len(), 2, "the second click places a second frame");
}

/// Live2 errors-1118: a view whose part no longer resolves goes blank (no "No parts to show" box) and the sheet shows
/// Onshape's blue info notice "Failed to resolve part.".
#[gpui::test]
fn a_view_whose_part_is_gone_goes_blank_with_failed_to_resolve_part(cx: &mut TestAppContext) {
    use noshape_model::drawing::ViewKind;
    use noshape_model::reference::{BodyId, FeatureId};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-part-gone");
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let id = e.drawing.sheets[e.sheet].views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.id).expect("a base view");
            let sheet = e.sheet;
            e.edit("Lose the part", cx, |d| {
                if let Some(ViewKind::Base { part, .. }) = d.sheets[sheet].view_mut(id).map(|v| &mut v.kind) {
                    *part = Some(BodyId { feature: FeatureId(u64::MAX - 7), index: 0 });
                }
            });
            e.sources.clear();
            e.ensure_sources(true, cx);
            id
        })
    });
    for _ in 0..600 {
        if cx.update(|cx| editor.read(cx).toast.is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    for _ in 0..100 {
        draw(cx, window);
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
    }
    cx.update(|cx| {
        let e = editor.read(cx);
        assert_eq!(e.toast.as_deref(), Some("Failed to resolve part."), "the info notice");
        let b = e.built.as_ref().expect("built");
        let view_pick = b.picks.iter().position(|t| *t == noshape_model::drawing::build::PickTarget::View(id)).map(|i| i as u32);
        assert!(!b.geometry.texts.iter().any(|t| Some(t.pick) == view_pick), "no text in the blank view");
    });
}

/// Live2 errors-1118: a diameter dimension whose circle is lost keeps its value, red-orange, with the red-orange dot at
/// the lost circle's centre (not at its leader's end on the old circle).
#[gpui::test]
fn a_lost_diameters_dot_sits_at_the_circles_centre(cx: &mut TestAppContext) {
    use noshape_model::drawing::{Annotation, AnnotationKind, AnnotationRef, PointRef, ViewKind};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-lost-diameter");
    real_sheet(cx, window, &editor);
    let (view, centre) = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let view = e.drawing.sheets[e.sheet].views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.id).expect("a base view");
            let g = e.geoms.get(&view).cloned().expect("geometry");
            let mut gone = g.snaps.iter().find_map(|s| s.point.clone()).expect("a snap point");
            let (PointRef::Vertex { body, .. } | PointRef::Center { body, .. } | PointRef::Midpoint { body, .. } | PointRef::Quadrant { body, .. } | PointRef::Intersection { body, .. } | PointRef::Thread { body, .. }) = &mut gone;
            body.index += 1000;
            let center = [(g.min[0] + g.max[0]) / 2.0, (g.min[1] + g.max[1]) / 2.0];
            let sheet = e.sheet;
            e.edit("Lost diameter", cx, |d| {
                let s = &mut d.sheets[sheet];
                let id = s.next_annotation_id();
                s.annotations.push(Annotation {
                    id,
                    kind: AnnotationKind::RadialDimension { view, center, radius: 0.002, diameter: true, angle: 0.8, dim_line: false, knee: None },
                    refs: vec![AnnotationRef { slot: 0, point: gone }],
                    ..Default::default()
                });
            });
            (view, center)
        })
    });
    draw(cx, window);
    cx.update(|cx| {
        let e = editor.read(cx);
        let want = e.placement(view).expect("placed").to_sheet(centre);
        let dots = e.lost_dots();
        assert!(matches!(dots.as_slice(), [(p, rgb)] if (p[0] - want[0]).hypot(p[1] - want[1]) < 1e-9 && *rgb == [221, 82, 40, 255]), "at the centre: {dots:?} for {want:?}");
    });
}

/// qa session-1118: Insert view's continuation (Projected view with the new view its parent) places along the parent's
/// axes only: a diagonal click places nothing, where a P-started projected view goes isometric (live2 place-1118).
#[gpui::test]
fn insert_views_continuation_places_no_diagonal_view(cx: &mut TestAppContext) {
    SUGGESTED.set(false);
    let (window, editor) = drawing(cx, "feel-insert-continuation");
    let studio = cx.update(|cx| editor.read(cx).doc.read(cx).session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).map(|t| t.id).expect("the Part Studio"));
    cx.update(|cx| editor.update(cx, |e, cx| e.choose_source(studio, None, cx)));
    for _ in 0..600 {
        if cx.update(|cx| editor.read(cx).pending_view().is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let base = [150.0, 190.0];
    click_sheet(cx, window, &editor, base);
    let pending = |cx: &mut TestAppContext, at: [f64; 2]| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = at;
                e.update_preview(cx);
            })
        })
        .unwrap();
        cx.update(|cx| editor.read(cx).pending_view().is_some())
    };
    assert!(cx.update(|cx| matches!(editor.read(cx).tool, Tool::Projected { parent: Some(_) })), "the continuation");
    assert!(pending(cx, [base[0], base[1] - 90.0]), "below: a projected view");
    assert!(!pending(cx, [base[0] + 150.0, base[1] - 95.0]), "a diagonal: nothing");
    // The same with P: isometric past 22.5°.
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| {
        let parent = e.drawing_sheet().views.first().map(|v| v.id);
        e.set_tool(Tool::Select, cx);
        e.set_tool(Tool::Projected { parent }, cx);
    }));
    assert!(pending(cx, [base[0] + 150.0, base[1] - 95.0]), "P-started: the isometric view");
}

/// Live2 shortcuts-1118: Shift+arrows move the sheet as Ctrl+Shift+arrows (a tenth of the canvas width: Shift+Down
/// down, Shift+Left left); W shows no bubble; Shift+N with a view selected and the Sheets panel closed does nothing;
/// `[` opens the Measure panel at the top left and clears the selection, no bubble; Space clears the selection;
/// Ctrl+S opens the Sheets panel over the sheet, which stays where it is.
#[gpui::test]
fn drawing_keys_as_lives_shortcut_sweep(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-shortcuts-1118");
    real_sheet(cx, window, &editor);
    let act = |cx: &mut TestAppContext, a: Box<dyn gpui_kit::Action>| {
        cx.update_window(window, |_, w, cx| {
            let focus = editor.read(cx).focus.clone();
            w.focus(&focus, cx);
            w.dispatch_action(a, cx);
        })
        .unwrap();
        draw(cx, window);
    };
    // The sheet's origin in canvas pixels.
    let on_screen = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let e = editor.read(cx);
            let s = e.size_px();
            [s[0] / 2.0 - e.center[0] * e.zoom, s[1] / 2.0 + e.center[1] * e.zoom]
        })
    };
    let step = cx.update(|cx| editor.read(cx).size_px()[0] * 0.1);
    let p0 = on_screen(cx);
    act(cx, Box::new(crate::actions::RotateDown90));
    let p1 = on_screen(cx);
    assert!((p1[1] - p0[1] - step).abs() < 1e-6 && (p1[0] - p0[0]).abs() < 1e-6, "Shift+Down: down {step} px: {p0:?} → {p1:?}");
    act(cx, Box::new(crate::actions::RotateLeft90));
    let p2 = on_screen(cx);
    assert!((p1[0] - p2[0] - step).abs() < 1e-6, "Shift+Left: left {step} px: {p1:?} → {p2:?}");
    act(cx, Box::new(crate::actions::ZoomWindow));
    assert!(cx.update(|cx| editor.read(cx).zoom_window.is_some() && editor.read(cx).status.is_none()), "W: armed, no bubble");
    act(cx, Box::new(crate::actions::Cancel));
    let view = cx.update(|cx| editor.read(cx).current_sheet().views.first().map(|v| v.id)).expect("a view");
    let select = |cx: &mut TestAppContext| cx.update(|cx| editor.update(cx, |e, _| e.selection = vec![Sel::View(view)]));
    select(cx);
    act(cx, Box::new(crate::actions::RenameSelected));
    assert!(cx.update(|cx| editor.read(cx).view_props.is_none() && editor.read(cx).row_rename.is_none()), "Shift+N, panel closed: nothing");
    act(cx, Box::new(crate::actions::MeasureTool));
    cx.update(|cx| {
        let e = editor.read(cx);
        assert!(e.measure.is_some() && e.selection.is_empty() && e.status.is_none(), "[: Measure on, selection cleared, no bubble");
    });
    let panel = VisualTestContext::from_window(window, cx).debug_bounds("dr-measure-panel").expect("the Measure panel");
    let canvas = cx.update(|cx| editor.read(cx).bounds);
    assert!((panel.left() - canvas.left()).abs() < px(0.5) && (panel.top() - canvas.top()).abs() < px(0.5), "top left: {panel:?} in {canvas:?}");
    // Its readout (live2 wake-1118 mu-mm-edge-*.png): a 40 mm edge on a millimetre drawing reads "40.00 mm", "1.57 in"
    // in Inches; Show dual unit adds "[1.575 in]" to either; Feet reads a foot as "1.00 ft"; a line lists as "Line 1".
    cx.update(|cx| {
        editor.update(cx, |e, _| {
            let units_mm = e.drawing.units_mm;
            e.drawing.units_mm = true;
            assert_eq!(e.measure_text(0.04), "40.00 mm");
            e.measure_unit = Some(0);
            assert_eq!(e.measure_text(0.04), "1.57 in");
            e.measure_dual = true;
            assert_eq!(e.measure_text(0.04), "1.57 in [1.575 in]");
            e.measure_unit = None;
            assert_eq!(e.measure_text(0.04), "40.00 mm [1.575 in]");
            (e.measure_unit, e.measure_dual) = (Some(3), false);
            assert_eq!(e.measure_text(0.3048), "1.00 ft");
            (e.measure_unit, e.drawing.units_mm) = (None, units_mm);
            let end = |x: f64| noshape_scene::drawing_tools::PickedPoint { view: noshape_model::drawing::ViewId(1), p: [x, 0.0], r: None };
            let picks = e.measure.replace(vec![crate::drawing_keys::MeasurePick::Line(end(0.0), end(0.04))]);
            assert_eq!(e.measure_pick_names(), ["Line 1"]);
            e.measure = picks;
        })
    });
    act(cx, Box::new(crate::actions::MeasureTool));
    select(cx);
    act(cx, Box::new(crate::actions::ClearSelection));
    assert!(cx.update(|cx| editor.read(cx).selection.is_empty()), "Space clears the selection");
    let before = on_screen(cx);
    cx.update(|cx| editor.update(cx, |e, cx| e.toggle_sheets_panel(cx)));
    draw(cx, window);
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).sheets_open), "Ctrl+S: the Sheets panel");
    let after = on_screen(cx);
    assert!((after[0] - before[0]).abs() < 1e-6 && (after[1] - before[1]).abs() < 1e-6, "the sheet stays: {before:?} → {after:?}");
    assert_eq!(cx.update(|cx| editor.read(cx).bounds), canvas, "the canvas keeps its size");
}

/// Live2 wake-1118: placing a geometric tolerance, a view's vertex the pointer passed over wakes (a red ✕ stays on it);
/// from then on the frame lines up with it, a dotted line and the point held on its x, from 8 px off and not at 12. A
/// vertex not woken lines up nothing.
#[gpui::test]
fn a_gtol_lines_up_with_a_woken_vertex_within_8_px(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-gdt-wake");
    real_sheet(cx, window, &editor);
    let (_, b, out) = horizontal_edge(cx, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Gdt, cx)));
    draw(cx, window);
    let zoom = cx.update(|cx| editor.read(cx).zoom);
    let hover = |cx: &mut TestAppContext, s: [f64; 2]| {
        let p = at_window(cx, &editor, s);
        VisualTestContext::from_window(window, cx).simulate_mouse_move(p, None, Modifiers::default());
        draw(cx, window);
        cx.update(|cx| {
            let e = editor.read(cx);
            (e.infer_lines.iter().any(|(from, _)| (from[0] - b[0]).abs() < 1e-9), (e.cursor[0] - b[0]).abs() < 1e-9)
        })
    };
    let off = |d: f64| [b[0] + d / zoom, b[1] + out * 25.0];
    assert_eq!(hover(cx, off(6.0)), (false, false), "not woken: nothing lines up");
    hover(cx, b);
    cx.update(|cx| {
        let e = editor.read(cx);
        assert!(e.woken_points.iter().any(|w| (w[0] - b[0]).hypot(w[1] - b[1]) < 1e-9), "hovered: woken {:?}", e.woken_points);
    });
    for (d, lines_up) in [(1.0, true), (8.0, true), (12.0, false)] {
        assert_eq!(hover(cx, off(d)), (lines_up, lines_up), "{d} px off the woken vertex's x");
        let crossed = cx.update(|cx| editor.read(cx).overlay.crosses.iter().any(|(c, _)| (c[0] - b[0]).hypot(c[1] - b[1]) < 1e-9));
        assert!(crossed, "{d} px: the woken vertex keeps its ✕");
    }
}

/// Live2 errors-1119-asmdrw: Insert view's picker with a search that matches nothing reads "No parts or sketches match
/// current search." (bold 12 px #333); a search that matches shows its rows, no message.
#[gpui::test]
fn insert_views_search_with_no_match_says_so(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "feel-insert-no-match");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.open_insert_chooser(cx)));
    draw(cx, window);
    draw(cx, window);
    let search = |cx: &mut TestAppContext, text: &str| {
        cx.update_window(window, |_, w, cx| {
            let st = editor.read(cx).chooser.search.clone().expect("the picker's search");
            st.update(cx, |s, cx| s.set_value(text.to_string(), w, cx));
        })
        .unwrap();
        draw(cx, window);
        VisualTestContext::from_window(window, cx).debug_bounds("dr-ch-no-match").is_some()
    };
    assert!(search(cx, "zzzz-none"), "no match: the message");
    assert!(!search(cx, ""), "no search: no message");
}

/// Live2 errors-1119-asmdrw: the Dimension tool on a circle seen edge-on (a line in the view) gives a linear dimension
/// across it, not a diameter with a leader; a circle seen face-on keeps its diameter.
#[gpui::test]
fn dimensioning_a_circle_seen_edge_on_measures_it_as_a_line(cx: &mut TestAppContext) {
    use noshape_model::drawing::{build::PickTarget, views::EdgeGeom};
    use noshape_scene::drawing_tools::Pick;
    SUGGESTED.set(true);
    // A pin standing beside the block: its top circle is edge-on in Front.
    SETUP.set(Some(|ctx, dw| {
        let sk = call(ctx, "sketch_create", json!({ "tab": "Part Studio 1", "plane": "plane:top" }))["sketch"].clone();
        call(ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[70, 10], [75, 10]] }));
        let region = call(ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].clone();
        call(ctx, "feature_add", json!({ "tab": "Part Studio 1", "type": "extrude", "params": { "entities": [region], "depth": "15 mm", "operationType": "NEW" } }));
        call(ctx, "drawing_update", json!({ "drawing": dw }));
    }));
    let (window, editor) = drawing(cx, "feel-dim-edge-on-circle");
    real_sheet(cx, window, &editor);
    // A picked edge whose model edge is a circle, edge-on and face-on.
    let find = |cx: &mut TestAppContext, face_on: bool| {
        cx.update(|cx| {
            let e = editor.read(cx);
            let built = e.built.as_ref()?;
            built.picks.iter().enumerate().find_map(|(i, t)| {
                let PickTarget::Edge { view, edge } = t else { return None };
                let g = e.geoms.get(view)?;
                let me = g.model_edges.get(g.edges.get(*edge as usize)?.model? as usize)?;
                match me.geom {
                    EdgeGeom::Circle { face_on: f, radius, .. } if f == face_on && (face_on || me.conic.is_some()) => Some((i as u32, radius)),
                    _ => None,
                }
            })
        })
    };
    let pick_with = |cx: &mut TestAppContext, hover: u32| {
        cx.update(|cx| {
            editor.update(cx, |e, _| {
                e.tool = Tool::Dimension { kind: noshape_scene::drawing_tools::DimTool::Linear, picks: vec![] };
                e.hover = Some(hover);
                let pk = e.pk();
                (pk.circle_pick(e.cursor).filter(|p| matches!(p, Pick::Circle { .. }) && pk.hovered_edge().is_some_and(|(_, me)| matches!(me.geom, EdgeGeom::Circle { face_on: true, .. }))), pk.line_pick())
            })
        })
    };
    let (edge_on, radius) = find(cx, false).expect("an edge-on circle in a view");
    let (circle, line) = pick_with(cx, edge_on);
    assert!(circle.is_none(), "no diameter: {circle:?}");
    let Some(Pick::Line { a, b }) = line else { panic!("a line: {line:?}") };
    assert!(((b.p[0] - a.p[0]).hypot(b.p[1] - a.p[1]) - 2.0 * radius).abs() < 1e-9, "across its diameter");
    if let Some((face_on, _)) = find(cx, true) {
        assert!(matches!(pick_with(cx, face_on).0, Some(Pick::Circle { .. })), "face-on: its diameter");
    }
}

/// Live2 drw-tools-1119 (p2p.png, dt.png): the 2 point dimension tool on a vertex shows the disc alone (its edge not
/// lit), wakes no "+" marks along the edges without Shift+Q and no ✕ on the picks; the picked points keep their discs and
/// the value is bold while placing; once placed the dimension stays selected, bold; dragging it in Select draws it plain.
#[gpui::test]
fn the_dimension_tool_marks_its_picks_as_live_and_keeps_the_placed_one_selected(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    use noshape_render::state;
    use noshape_scene::drawing_tools::{DimTool, SnapMark};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-dim-p2p");
    real_sheet(cx, window, &editor);
    // Two vertices of the first view: its lower-left and lower-right corners; the middle of its bottom edge.
    let (a, b) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let pts: Vec<[f64; 2]> = g.snaps.iter().map(|s| pl.to_sheet(s.p)).collect();
        let a = pts.iter().copied().min_by(|p, q| (p[0] + p[1]).total_cmp(&(q[0] + q[1]))).expect("a vertex");
        let b = pts.iter().copied().filter(|p| (p[1] - a[1]).abs() < 1e-6 && p[0] > a[0] + 1.0).max_by(|p, q| p[0].total_cmp(&q[0])).expect("a vertex level with it");
        (a, b)
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Point2, picks: vec![] }, cx)));
    draw(cx, window);
    lit_at(cx, window, &editor, [(a[0] + b[0]) / 2.0, a[1]]);
    assert!(cx.update(|cx| editor.read(cx).woken_plus.is_empty()), "no + marks without Shift+Q");
    lit_at(cx, window, &editor, a);
    let (snap, lit) = cx.update(|cx| {
        let e = editor.read(cx);
        (e.hover_snap, e.build_scene().states.iter().filter(|s| **s & state::HOVER != 0).count())
    });
    assert!(snap.is_some() && lit == 0, "on the vertex: the disc, its edge not lit ({snap:?}, {lit} lit)");
    let picks = |cx: &mut TestAppContext| cx.update(|cx| match &editor.read(cx).tool {
        Tool::Dimension { picks, .. } => picks.len(),
        _ => 99,
    });
    click_at(cx, window, &editor, a, Modifiers::default());
    assert_eq!(picks(cx), 1, "the first vertex picked");
    click_at(cx, window, &editor, b, Modifiers::default());
    assert_eq!(picks(cx), 2, "the second vertex picked ({a:?}, {b:?})");
    lit_at(cx, window, &editor, [(a[0] + b[0]) / 2.0, a[1] - 12.0]);
    let bold = |cx: &mut TestAppContext, id: Option<u64>| {
        cx.update(|cx| {
            let e = editor.read(cx);
            let bt = e.built.as_ref().expect("built");
            let id = id.or_else(|| e.preview.as_ref().and_then(|d| d.sheets.get(e.sheet)).and_then(|s| s.annotations.last()).map(|a| a.id)).expect("a dimension");
            let texts: Vec<usize> = (0..bt.geometry.texts.len()).filter(|&i| bt.picks.get(bt.geometry.texts[i].pick as usize) == Some(&PickTarget::Annotation(id))).collect();
            !texts.is_empty() && texts.iter().all(|&i| bt.geometry.text_font(i).is_some_and(|f| f.bold))
        })
    };
    cx.update(|cx| {
        let e = editor.read(cx);
        assert!(e.overlay.crosses.is_empty(), "no ✕ on the picks");
        for p in [a, b] {
            assert!(e.overlay.marks.iter().any(|(m, k, _)| *k == SnapMark::Disc && (m[0] - p[0]).hypot(m[1] - p[1]) < 1e-6), "a disc on {p:?}");
        }
    });
    assert!(bold(cx, None), "the value bold while placing");
    let count = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).drawing.sheets[editor.read(cx).sheet].annotations.len());
    let before = count(cx);
    let sel = click_at(cx, window, &editor, [(a[0] + b[0]) / 2.0, a[1] - 12.0], Modifiers::default());
    assert_eq!(picks(cx), 0, "placed: the tool ready for the next");
    let placed = cx.update(|cx| editor.read(cx).current_sheet().annotations.last().map(|x| x.id)).expect("placed");
    assert_eq!(count(cx), before + 1, "one dimension placed");
    assert_eq!(sel, vec![Sel::Annotation(placed)], "placed: selected");
    assert!(bold(cx, Some(placed)), "placed: bold");
    // Select: a press-drag on its text draws it plain, no orange, no tan.
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Select, cx)));
    draw(cx, window);
    let text_at = cx.update(|cx| {
        let e = editor.read(cx);
        let bt = e.built.as_ref().expect("built");
        bt.geometry.texts.iter().find(|t| bt.picks.get(t.pick as usize) == Some(&PickTarget::Annotation(placed))).map(|t| t.pos).expect("its text")
    });
    click_at(cx, window, &editor, text_at, Modifiers::default());
    let p = at_window(cx, &editor, text_at);
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(p, None, Modifiers::default());
    v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
    v.simulate_mouse_move(p + Point::new(px(40.), px(-16.)), Some(MouseButton::Left), Modifiers::default());
    draw(cx, window);
    let lit = cx.update(|cx| {
        let e = editor.read(cx);
        assert_eq!(e.dragging_annotation_id(), Some(placed), "dragging the dimension");
        e.build_scene().states.iter().filter(|s| **s & (state::HOVER | state::SELECTED) != 0).count()
    });
    assert_eq!(lit, 0, "dragged: drawn plain");
    VisualTestContext::from_window(window, cx).simulate_mouse_up(p + Point::new(px(40.), px(-16.)), MouseButton::Left, Modifiers::default());
}

/// Live2 rad-note plus-*.png (11-19 08:30): after a Dimension pick on an edge (not on hover, not after a vertex pick)
/// every dimension of the sheet shows orange + marks: a linear one at its value's centre and at its dimension line's end
/// on the second extension line; the placement snaps to them.
#[gpui::test]
fn a_dimension_pick_marks_every_dimensions_value_and_far_end(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    use noshape_model::drawing::sheet::GripKind;
    use noshape_scene::drawing_tools::DimTool;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-dim-plus");
    real_sheet(cx, window, &editor);
    let (a, b) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let pts: Vec<[f64; 2]> = g.snaps.iter().map(|s| pl.to_sheet(s.p)).collect();
        let a = pts.iter().copied().min_by(|p, q| (p[0] + p[1]).total_cmp(&(q[0] + q[1]))).expect("a vertex");
        let b = pts.iter().copied().filter(|p| (p[1] - a[1]).abs() < 1e-6 && p[0] > a[0] + 1.0).max_by(|p, q| p[0].total_cmp(&q[0])).expect("a vertex level with it");
        (a, b)
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Point2, picks: vec![] }, cx)));
    draw(cx, window);
    click_at(cx, window, &editor, a, Modifiers::default());
    click_at(cx, window, &editor, b, Modifiers::default());
    click_at(cx, window, &editor, [(a[0] + b[0]) / 2.0, a[1] - 12.0], Modifiers::default());
    let placed = cx.update(|cx| editor.read(cx).current_sheet().annotations.last().map(|x| x.id)).expect("placed");
    let far_end = cx.update(|cx| {
        let e = editor.read(cx);
        let bt = e.built.as_ref().expect("built");
        bt.geometry.grips.iter().filter(|g| bt.picks.get(g.pick as usize) == Some(&PickTarget::Annotation(placed)) && g.kind == GripKind::End).nth(1).map(|g| g.pos).expect("its far end")
    });
    let marks = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).dimension_pick_marks());
    let edge = [(a[0] * 3.0 + b[0]) / 4.0, a[1]];
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)));
    draw(cx, window);
    lit_at(cx, window, &editor, edge);
    assert!(marks(cx).is_empty(), "none on hover");
    click_at(cx, window, &editor, edge, Modifiers::default());
    let m = marks(cx);
    assert_eq!(m.len(), 2, "the linear dimension's value and far end: {m:?}");
    assert!(m.iter().any(|p| (p[0] - far_end[0]).hypot(p[1] - far_end[1]) < 1e-9), "one at its far end {far_end:?}: {m:?}");
    // The placement snaps onto a mark within reach.
    let near = [far_end[0] + cx.update(|cx| editor.read(cx).tol_mm(2.0)), far_end[1] - cx.update(|cx| editor.read(cx).tol_mm(2.0))];
    lit_at(cx, window, &editor, near);
    let at = cx.update(|cx| editor.read(cx).cursor);
    assert!((at[0] - far_end[0]).hypot(at[1] - far_end[1]) < 1e-9, "the pointer held on the mark: {at:?} vs {far_end:?}");
    // A vertex pick shows none.
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Dimension { kind: DimTool::Point2, picks: vec![] }, cx)));
    draw(cx, window);
    click_at(cx, window, &editor, a, Modifiers::default());
    assert!(marks(cx).is_empty(), "none after a vertex pick");
}

/// Live2 drw-tools-1119 (sl.png), recheck-1119: a section's cutting line hovered in Select frames the section view light
/// blue, tan with its arrows and letters and grips; a click on its body selects the line itself; a press-drag there
/// moves nothing and selects the parent view, which keeps the line's grips while under the pointer.
#[gpui::test]
fn a_cutting_line_hovered_frames_its_section_blue_and_its_body_does_not_drag(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    use noshape_model::drawing::views::EdgeGeom;
    use noshape_model::drawing::ViewKind;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-cut-line");
    real_sheet(cx, window, &editor);
    let (base, inside, mid, right) = cx.update(|cx| {
        let e = editor.read(cx);
        let v = e.current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.id).expect("a base view");
        let pl = e.placement(v).expect("placed");
        let g = e.geoms.get(&v).cloned().expect("geometry");
        let m = g.model_edges.iter().find_map(|me| match me.geom {
            EdgeGeom::Line { a, b } if (a[1] - b[1]).abs() < 1e-9 => Some(pl.to_sheet([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])),
            _ => None,
        });
        (v, [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0], m.expect("a horizontal edge"), pl.max[0])
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Section { parent: None, p0: None, line: None, partial: false, jogs: vec![] }, cx)));
    draw(cx, window);
    click_at(cx, window, &editor, inside, Modifiers::default());
    click_at(cx, window, &editor, mid, Modifiers::default());
    click_at(cx, window, &editor, [right + 60.0, inside[1]], Modifiers::default());
    let section = cx.update(|cx| editor.read(cx).current_sheet().views.iter().find(|v| matches!(v.kind, ViewKind::Section { .. })).map(|v| v.id)).expect("the section placed");
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Select, cx)));
    draw(cx, window);
    // A point on the cutting line's longest stroke, a quarter along (clear of its cut point).
    let on_line = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        let l = b.geometry.lines.iter().filter(|l| b.picks.get(l.pick as usize) == Some(&PickTarget::SectionLine(section))).max_by(|l, m| {
            let len = |p: &[[f64; 2]]| p.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>();
            len(&l.points).total_cmp(&len(&m.points))
        });
        let l = l.expect("the cutting line drawn");
        let (p, q) = (l.points[0], l.points[l.points.len() - 1]);
        [p[0] + (q[0] - p[0]) * 0.25, p[1] + (q[1] - p[1]) * 0.25]
    });
    lit_at(cx, window, &editor, on_line);
    let frames = cx.update(|cx| {
        let e = editor.read(cx);
        assert_eq!(e.hovered_cut_line(), Some(section), "the cutting line under the pointer");
        e.view_frames()
    });
    assert_eq!(frames, vec![(section, 0x90cef1)], "its section framed light blue");
    // Live2 recheck-1119: tan grips at both ends of each arrow, a light-blue one on the line where it follows the
    // view, the arrows and letters tan; with the parent selected the grips alone.
    const BLUE: [u8; 4] = [0x90, 0xce, 0xf1, 255];
    let looks = |cx: &mut TestAppContext| cx.update(|cx| {
        let l = editor.read(cx).cut_line_looks();
        (l.lit.len(), l.grips.iter().filter(|g| g.1 != BLUE).count(), l.grips.iter().filter(|g| g.1 == BLUE).map(|g| g.0).collect::<Vec<_>>(), l.ink.indices.len())
    });
    let (lit, tan, blue, ink) = looks(cx);
    assert!(lit > 0 && ink > 0, "the line, its arrows and letters tan");
    assert_eq!(tan, 4, "a tan grip at both ends of each arrow");
    assert!(blue.len() == 1 && (blue[0][0] - mid[0]).abs() < 1e-6, "one light-blue grip on the point it follows: {blue:?} (line at x {})", mid[0]);
    // A plain click on its body selects the line itself, drawn as hovered, the pointer gone (centermark-sel case 2).
    assert_eq!(click_at(cx, window, &editor, on_line, Modifiers::default()), vec![Sel::CutLine(section)], "a click selects the cutting line");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    let (lit, tan, blue, ink) = looks(cx);
    assert!(lit > 0 && ink > 0 && tan == 4 && blue.len() == 1, "selected: tan with its grips and anchor ({lit}, {tan}, {blue:?}, {ink})");
    cx.update(|cx| editor.update(cx, |e, _| e.selection.clear()));
    lit_at(cx, window, &editor, on_line);
    let points = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).current_sheet().section_points(section));
    let before = points(cx);
    let p = at_window(cx, &editor, on_line);
    let q = p + Point::new(px(-40.), px(0.));
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_down(p, MouseButton::Left, Modifiers::default());
    v.simulate_mouse_move(q, Some(MouseButton::Left), Modifiers::default());
    v.simulate_mouse_up(q, MouseButton::Left, Modifiers::default());
    draw(cx, window);
    assert_eq!(points(cx), before, "the cut didn't move");
    assert_eq!(cx.update(|cx| editor.read(cx).selection.clone()), vec![Sel::View(base)], "the parent view selected");
    // Live2 drw-tools-1119 centermark-sel: with the pointer over the selected parent its cutting line's grips alone,
    // none once the pointer leaves it.
    lit_at(cx, window, &editor, [(inside[0] + right) / 2.0, inside[1]]);
    let (lit, tan, blue, ink) = looks(cx);
    assert_eq!((lit, tan, blue.len(), ink), (0, 4, 1, 0), "the parent selected, the pointer over it: the cutting line's grips alone");
    lit_at(cx, window, &editor, [5.0, 5.0]);
    assert_eq!(looks(cx).1, 0, "the pointer away: no grips");
}

/// Live2 recheck-1119 callout: the Callout tool lights an edge from the other tools' reach (live 6 px from the edge, not
/// 7; the 11-19 "5 vs 4" row measured from a 1 px shifted edge): lit at 6 px, not at 7.
#[gpui::test]
fn the_callout_tool_lights_an_edge_within_the_pickbox(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-callout-reach");
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
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Balloon { anchor: None }, cx)));
    draw(cx, window);
    let edge = lit_at(cx, window, &editor, mid);
    assert!(matches!(edge, Some(Sel::Edge(..))), "on the edge: {edge:?}");
    for (px, lights) in [(6.0, true), (7.0, false)] {
        for sign in [1.0, -1.0] {
            let mm = cx.update(|cx| editor.read(cx).tol_mm(px));
            let lit = lit_at(cx, window, &editor, [mid[0] + normal[0] * mm * sign, mid[1] + normal[1] * mm * sign]);
            assert_eq!(lit == edge, lights, "{} px off the edge: lit {lit:?}", sign * px);
        }
    }
}

/// ui-parity drw-copy-annotation-drag (qa 11-19 regression): Alt+drag on a selected note, pressed on its end where its
/// side grip is, leaves a copy and lands the dragged one where the pointer is released (live: 40.4 mm along; ours
/// resized the note from its grip).
#[gpui::test]
fn an_alt_drag_copies_a_note_and_lands_it_where_released(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, dw| {
        call(ctx, "drawing_note", json!({ "drawing": dw, "text": "TEMP", "position": [60, 40] }));
    }));
    let (window, editor) = drawing(cx, "feel-note-alt-drag");
    real_sheet(cx, window, &editor);
    let at = cx.update(|cx| {
        let e = editor.read(cx);
        let a = e.current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Note { .. })).expect("the note");
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(Sel::Annotation(a.id)));
        let t = b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("its text");
        let q = noshape_model::drawing::font::text_box(t);
        [q.iter().map(|p| p[0]).sum::<f64>() / 4.0, q.iter().map(|p| p[1]).sum::<f64>() / 4.0]
    });
    let before: Vec<[f64; 2]> = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().map(|a| a.position).collect());
    // Selected, as after the take's Esc (a note stays selected).
    cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let id = e.current_sheet().annotations[0].id;
            e.selection = vec![Sel::Annotation(id)];
            cx.notify();
        })
    });
    draw(cx, window);
    // Pressed on its left end, where the selected note's side grip is (the take's press, at the note's anchor).
    let at = cx.update(|cx| editor.read(cx).note_grips().and_then(|(_, g)| g.into_iter().find(|g| g.0 == 0)).map(|g| g.1)).unwrap_or(at);
    let alt = Modifiers { alt: true, ..Default::default() };
    let p = at_window(cx, &editor, at);
    let q = at_window(cx, &editor, [at[0] + 40.0, at[1]]);
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(p, None, alt);
    v.simulate_mouse_down(p, MouseButton::Left, alt);
    for k in 1..=10 {
        let m = p + (q - p) * (k as f32 / 10.);
        VisualTestContext::from_window(window, cx).simulate_mouse_move(m, Some(MouseButton::Left), alt);
        draw(cx, window);
    }
    VisualTestContext::from_window(window, cx).simulate_mouse_up(q, MouseButton::Left, alt);
    draw(cx, window);
    let mut after: Vec<[f64; 2]> = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().map(|a| a.position).collect());
    after.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(after.len(), 2, "a copy: {before:?} -> {after:?}");
    assert!((after[0][0] - before[0][0]).abs() < 1e-6, "one stays: {before:?} -> {after:?}");
    assert!((after[1][0] - before[0][0] - 40.0).abs() < 1.0, "the other 40 mm along: {before:?} -> {after:?}");
}

/// Live measure-1119: while measuring, a click on empty sheet keeps the picks, and a pick in another view than the
/// first one's is ignored (no row, no message); a second pick in the first view is added.
#[gpui::test]
fn measure_keeps_its_picks_on_an_empty_click_and_ignores_another_view(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-measure-views");
    real_sheet(cx, window, &editor);
    let corners = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        b.placements.iter().take(2).map(|pl| {
            let g = e.geoms.get(&pl.id).expect("geometry");
            (pl.to_sheet(g.min), pl.to_sheet(g.max), pl.to_sheet([g.min[0], g.max[1]]))
        }).collect::<Vec<_>>()
    });
    assert_eq!(corners.len(), 2, "two views");
    cx.update(|cx| editor.update(cx, |e, cx| e.toggle_measure(cx)));
    draw(cx, window);
    let picks = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).measure_pick_names());
    click_at(cx, window, &editor, corners[0].0, Modifiers::default());
    assert_eq!(picks(cx), ["Point 1"], "a corner of the first view");
    click_at(cx, window, &editor, [8.0, 8.0], Modifiers::default());
    assert_eq!(picks(cx), ["Point 1"], "an empty click keeps it");
    click_at(cx, window, &editor, corners[1].0, Modifiers::default());
    assert_eq!(picks(cx), ["Point 1"], "a pick in another view is ignored");
    click_at(cx, window, &editor, corners[0].2, Modifiers::default());
    assert_eq!(picks(cx), ["Point 1", "Point 2"], "another corner of the first view");
}

/// Live studios place-1118 (ann-1118 mnote.png): N › click › type "AB" › a click on empty sheet commits "AB" and the
/// Note tool goes on, its "NOTE" preview at that click (no second editor); Esc then ends the tool and adds nothing.
#[gpui::test]
fn a_click_away_from_a_typed_note_commits_it_and_the_note_tool_goes_on(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "feel-note-commit");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Note, cx)));
    draw(cx, window);
    // (The second click below the note: the Note panel floats above it.)
    let (a, b) = ([30.0, 120.0], [30.0, 50.0]);
    click_at(cx, window, &editor, a, Modifiers::default());
    assert!(cx.update(|cx| matches!(editor.read(cx).panel, Some(crate::drawing_view::Panel::Note { .. }))), "the note editor opens");
    VisualTestContext::from_window(window, cx).simulate_keystrokes("a b");
    draw(cx, window);
    let typed = cx.update(|cx| match &editor.read(cx).panel {
        Some(crate::drawing_view::Panel::Note { input, .. }) => Some(input.read(cx).value().to_string()),
        _ => None,
    });
    assert_eq!(typed.as_deref(), Some("ab"), "typed into the note");
    let notes = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let e = editor.read(cx);
            e.drawing_sheet().annotations.iter().filter_map(|x| match &x.kind {
                noshape_model::drawing::AnnotationKind::Note { text, .. } => Some(text.clone()),
                _ => None,
            }).collect::<Vec<_>>()
        })
    };
    let before = notes(cx);
    click_at(cx, window, &editor, b, Modifiers::default());
    let after = notes(cx);
    assert_eq!(after.len(), before.len() + 1, "{after:?}");
    assert_eq!(after.last().map(String::as_str), Some("ab"));
    let (open, note_tool, at, preview) = cx.update(|cx| {
        let e = editor.read(cx);
        let preview = e.current_sheet().annotations.iter().any(|x| matches!(&x.kind, noshape_model::drawing::AnnotationKind::Note { text, .. } if text == "NOTE"));
        (e.panel.is_some(), matches!(e.tool, Tool::Note) && e.note_leader.is_none(), e.cursor, preview)
    });
    assert!(!open && note_tool, "no second editor; the Note tool goes on");
    assert!(preview, "the \"NOTE\" preview drawn");
    assert!((at[0] - b[0]).abs() < 0.5 && (at[1] - b[1]).abs() < 0.5, "its preview at the click: {at:?}");
    // (Esc's binding needs the workspace's key context: its action, as the other tests.)
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    draw(cx, window);
    assert!(cx.update(|cx| matches!(editor.read(cx).tool, Tool::Select)), "Esc ends the tool");
    assert_eq!(notes(cx), after, "Esc adds nothing");
}

/// Live drw-hcallout-fit 14: a panel over a view hides it from the pointer: nothing under the panel lights (ours lit
/// the view through the palette); off the panel the view lights again.
#[gpui::test]
fn nothing_lights_under_a_panel(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-under-panel");
    real_sheet(cx, window, &editor);
    let pl = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.first().copied())).expect("a view");
    let c = [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
    assert_eq!(view_of(&lit_at(cx, window, &editor, c)), Some(pl.id), "the view lights with no panel");
    // Move to sheet opens where its menu was: just above and left of the view's centre, so the dialog covers it.
    cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let l = e.sheet_to_local(c);
            e.menu_at = [l[0] - 20.0, l[1] - 15.0];
            e.open_move_sheet(crate::drawing_move_sheet::MoveTarget::View(pl.id), cx);
        })
    });
    draw(cx, window);
    assert_eq!(lit_at(cx, window, &editor, c), None, "nothing lights under the dialog");
    cx.update(|cx| {
        editor.update(cx, |e, cx| {
            e.move_sheet = None;
            cx.notify();
        })
    });
    draw(cx, window);
    assert_eq!(view_of(&lit_at(cx, window, &editor, c)), Some(pl.id), "the view lights again once it closes");
}

/// qa DRW-DIALOG-FIRST-FOCUS (live drawing/1047, 1037): View properties opens with View orientation focused and Move
/// to sheet with Sheet focused (a 2 px blue underline); closing one gives the sheet its focus back.
#[gpui::test]
fn view_dialogs_open_with_their_first_select_focused(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "feel-first-focus");
    real_sheet(cx, window, &editor);
    let view = cx.update(|cx| editor.read(cx).current_sheet().views.first().map(|v| v.id)).expect("a view");
    let state = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| {
            let e = editor.read(cx);
            (e.first_lit, e.first_field.is_focused(w), e.focus.is_focused(w))
        })
        .unwrap()
    };
    draw(cx, window);
    assert_eq!(state(cx), (false, false, true), "the sheet has the focus");
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_view_props(view, w, cx))).unwrap();
    draw(cx, window);
    draw(cx, window);
    assert_eq!(state(cx), (true, true, false), "View orientation focused");
    cx.update(|cx| editor.update(cx, |e, cx| {
        e.view_props = None;
        cx.notify();
    }));
    draw(cx, window);
    draw(cx, window);
    assert_eq!(state(cx), (false, false, true), "closed: the sheet's focus again");
    cx.update(|cx| editor.update(cx, |e, cx| e.open_move_sheet(crate::drawing_move_sheet::MoveTarget::View(view), cx)));
    draw(cx, window);
    draw(cx, window);
    assert_eq!(state(cx), (true, true, false), "Move to sheet's Sheet focused");
}
