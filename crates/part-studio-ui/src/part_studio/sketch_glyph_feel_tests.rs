//! A line's constraint glyphs after the pointer leaves it (qa 11-17, the user's flow on c330a760): they linger briefly
//! so the pointer can reach them, then go, with no further pointer move to redraw them away; a glyph's hover reach.

use super::feel::{Pointer, gpu_pointer};
use super::*;
use crate::part_studio::sketch::GLYPH_LINGER;

#[gpui::test]
fn a_left_lines_glyphs_go_when_the_linger_ends(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "glyph-linger", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // The bottom line's middle on screen.
    let (line, on) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let vp = v.viewport.read(cx);
        data.sketch
            .entities()
            .filter_map(|e| match data.sketch.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => Some((e.id, vp.project(ed.frame.to_world((a + b) * 0.5))?)),
                _ => None,
            })
            .max_by(|x, y| x.1.y.total_cmp(&y.1.y))
            .expect("a line")
    });
    let pointer = Pointer::new(cx, window, &view);
    pointer.move_to(cx, &view, on);
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover), Some(line), "the line lit");
    // Off the line with one pointer event and no time passing; then the linger on the test clock alone.
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_move(pointer.window_point(cx, on + DVec2::new(0.0, 160.0)), None, Modifiers::default());
    draw(cx, window);
    let lingering = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = view.read(cx);
        v.ui.glyph_linger_task.as_ref().and(v.sketch.as_ref().unwrap().glyph_entity.map(|(e, _)| e))
    });
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover), None, "the line let go");
    assert_eq!(lingering(cx), Some(line), "the left line's glyphs linger");
    let redraws = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = redraws.clone();
    let _watch = cx.update(|cx| cx.observe(&view, move |_, _| counted.set(counted.get() + 1)));
    cx.executor().advance_clock(GLYPH_LINGER - std::time::Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(lingering(cx), Some(line), "still there 100 ms before the linger ends");
    cx.executor().advance_clock(std::time::Duration::from_millis(110));
    cx.run_until_parked();
    assert_eq!(lingering(cx), None, "gone when it ends");
    assert!(redraws.get() > 0, "a redraw when the lingering glyphs are due to go");
}

/// Live2 feel-1417 (Show constraints off): a hovered line's glyph row is placed once, ~40 px off the line by the spot
/// where the hover started (not the line's middle), and stays there while the pointer slides along the line; moving
/// straight off the line from that spot reaches its first glyph: the glyph lights and the line stays lit as its partner.
#[gpui::test]
fn a_hovered_lines_glyphs_wait_by_the_pointer(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "glyph-row-at-pointer", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (750., 420.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // The longest line on screen, a quarter along it, its first glyph's constraint and the row's count.
    let (line, on, first, count) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let vp = v.viewport.read(cx);
        let (id, a, b) = data
            .sketch
            .entities()
            .filter_map(|e| match data.sketch.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => Some((e.id, vp.project(ed.frame.to_world(a))?, vp.project(ed.frame.to_world(b))?)),
                _ => None,
            })
            .max_by(|x, y| (x.2 - x.1).length().total_cmp(&(y.2 - y.1).length()))
            .expect("a line");
        let row: Vec<_> = ed.constraint_glyphs(&data).into_iter().filter(|g| g.entity == id && !g.on_point && g.offset.is_none()).collect();
        (id, a + (b - a) * 0.25, row.first().expect("the line's glyphs").constraint, row.len())
    });
    let pointer = Pointer::new(cx, window, &view);
    pointer.move_to(cx, &view, on);
    assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover), Some(line), "the line lit");
    // The row's first glyph as drawn (window px): by the pointer's spot on the line, not the line's middle.
    let spot = pointer.window_point(cx, on);
    let selector: &'static str = Box::leak(format!("glyph-{}-{}", first.0, line.0).into_boxed_str());
    let glyph = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(selector).expect("the line's first glyph shown").center();
    let (dx, dy) = (f32::from(glyph.x - spot.x), f32::from(glyph.y - spot.y));
    assert!(dx.abs() <= 11.0 * count as f32 + 1.0 && (30.0..50.0).contains(&dy.abs()), "the row by the pointer: first glyph {dx:.0}, {dy:.0} px from the pointer's spot");
    // Placed once where the hover starts: sliding 60 px along the line leaves it there (live2 feel-1417 follow-up).
    let (a, b) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().unwrap();
        let data = v.sketch_data(cx).unwrap();
        let vp = v.viewport.read(cx);
        let Some(noshape_sketch::Curve2::Line { a, b }) = data.sketch.curve(line) else { unreachable!() };
        (vp.project(ed.frame.to_world(a)).unwrap(), vp.project(ed.frame.to_world(b)).unwrap())
    });
    let along = (b - a).normalize();
    for i in 1..=6 {
        pointer.move_to(cx, &view, on + along * (10.0 * i as f64));
    }
    let moved = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(selector).expect("the row still shown").center();
    assert!(f32::from(moved.x - glyph.x).abs() < 0.5 && f32::from(moved.y - glyph.y).abs() < 0.5, "the row stays put while the pointer slides along the line: {glyph:?} -> {moved:?}");
    pointer.move_to(cx, &view, on);
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        let at = gpui_kit::point(spot.x + (glyph.x - spot.x) * t, spot.y + (glyph.y - spot.y) * t);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_move(at, None, Modifiers::default());
        // (Drawn, no time passing: the glyphs' linger runs on the test clock.)
        draw(cx, window);
    }
    let (hovered, lit) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().unwrap();
        let data = v.sketch_data(cx).unwrap();
        (ed.hover_constraint, ed.picked(Some(&data)).contains(&line))
    });
    assert_eq!(hovered, Some(first), "the pointer reached the row's first glyph, by its spot on the line");
    assert!(lit, "the line stays lit as the hovered glyph's partner");
    // Live2 sktools-1118: lit to 14 px from the tile's centre (4 px past its 20 px edge), off at 15–16 when jumped to; a
    // slide holds it to 16. Straight out from the line, away from the row's other glyphs: a slide 13, 15, 16, 17, then
    // away and a jump back to 15.
    let (ox, oy) = (f32::from(glyph.x - spot.x), f32::from(glyph.y - spot.y));
    let n = (ox * ox + oy * oy).sqrt();
    let reach: Vec<(f32, Option<noshape_sketch::ConstraintId>)> = [13.0f32, 15.0, 16.0, 17.0, 60.0, 15.0]
        .into_iter()
        .map(|d| {
            let at = gpui_kit::point(glyph.x + px(ox / n * d), glyph.y + px(oy / n * d));
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_mouse_move(at, None, Modifiers::default());
            draw(cx, window);
            (d, cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover_constraint))
        })
        .collect();
    assert_eq!(reach, vec![(13.0, Some(first)), (15.0, Some(first)), (16.0, Some(first)), (17.0, None), (60.0, None), (15.0, None)], "lit at 13 px, held by the slide to 16, not jumped to at 15");
}

/// Feel row "Constraint glyphs" (live sketch/28): with Show constraints on, a line's glyphs are 20 px boxes on a 22 px
/// pitch in one row, the row centred 20.5 px off the line's middle, square to the line.
#[gpui::test]
fn a_lines_glyph_row_is_20_px_boxes_22_px_apart_20_5_px_off_its_middle(cx: &mut TestAppContext) {
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "glyph-row-layout", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (750., 420.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update(|cx| view.update(cx, |v, cx| {
        v.sketch.as_mut().expect("sketch mode").set_show_constraints(true);
        cx.notify();
    }));
    draw(cx, window);
    // Each line's middle and direction on screen (window px) and its row's glyphs.
    let lines: Vec<(noshape_sketch::EntityId, DVec2, DVec2, Vec<noshape_sketch::ConstraintId>)> = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let vp = v.viewport.read(cx);
        let glyphs = ed.constraint_glyphs(&data);
        data.sketch
            .entities()
            .filter_map(|e| match data.sketch.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => {
                    let (a, b) = (vp.project(ed.frame.to_world(a))?, vp.project(ed.frame.to_world(b))?);
                    let row = glyphs.iter().filter(|g| g.entity == e.id && !g.on_point && g.offset.is_none()).map(|g| g.constraint).collect();
                    Some((e.id, (a + b) * 0.5, (b - a).normalize(), row))
                }
                _ => None,
            })
            .collect()
    });
    assert_eq!(lines.len(), 4, "the rectangle's lines");
    let rows: Vec<usize> = lines.iter().map(|l| l.3.len()).collect();
    assert!(rows.iter().filter(|&&n| n > 0).count() >= 2 && rows.iter().any(|&n| n >= 2), "glyphs per line {rows:?}: two rows shown, one of 2+");
    let pointer_origin = {
        let p = Pointer::new(cx, window, &view);
        p.window_point(cx, DVec2::ZERO)
    };
    let mut wrong = Vec::new();
    for (line, mid, dir, row) in &lines {
        if row.is_empty() {
            continue;
        }
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let boxes: Vec<_> = row
            .iter()
            .map(|c| {
                let selector: &'static str = Box::leak(format!("glyph-{}-{}", c.0, line.0).into_boxed_str());
                vcx.debug_bounds(selector).expect("the glyph drawn")
            })
            .collect();
        let centres: Vec<DVec2> = boxes
            .iter()
            .map(|b| DVec2::new(f64::from(f32::from(b.center().x - pointer_origin.x)), f64::from(f32::from(b.center().y - pointer_origin.y))))
            .collect();
        for b in &boxes {
            // The bounds hold the tile and its clear hover margin.
            let (w, h) = (f32::from(b.size.width) - 2.0 * super::super::GLYPH_MARGIN, f32::from(b.size.height) - 2.0 * super::super::GLYPH_MARGIN);
            if (w - 20.0).abs() > 0.5 || (h - 20.0).abs() > 0.5 {
                wrong.push(format!("line {line:?}: a {w} × {h} box, want 20 × 20"));
            }
        }
        let mut xs: Vec<f64> = centres.iter().map(|c| c.x).collect();
        xs.sort_by(f64::total_cmp);
        if xs.windows(2).any(|w| (w[1] - w[0] - 22.0).abs() > 0.5) || centres.iter().any(|c| (c.y - centres[0].y).abs() > 0.5) {
            wrong.push(format!("line {line:?}: centres {centres:?}, want one row 22 px apart"));
        }
        let off = centres.iter().sum::<DVec2>() / centres.len() as f64 - *mid;
        if (off.length() - 20.5).abs() > 1.0 || off.normalize().dot(*dir).abs() > 0.05 {
            wrong.push(format!("line {line:?}: the row's centre {off:?} from its middle, want 20.5 px square to it"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live fc-1120 (2) (psui recheck 02:44): while a rectangle's corner is dragged no glyph shows, Show constraints on, and
/// the regions lose their grey fill; both come back on release (the fill too: qa FEEL-SKETCH-REGION-FILL-AFTER-DRAG).
#[gpui::test]
fn glyphs_and_the_region_fill_go_while_geometry_is_dragged(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "glyph-drag-hidden", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (750., 420.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update(|cx| view.update(cx, |v, cx| {
        v.sketch.as_mut().expect("sketch mode").set_show_constraints(true);
        cx.notify();
    }));
    let shown = |cx: &mut TestAppContext| {
        draw(cx, window);
        let selectors: Vec<&'static str> = cx.update(|cx| {
            let v = view.read(cx);
            let (ed, data) = (v.sketch.as_ref().expect("sketch mode"), v.sketch_data(cx).expect("sketch data"));
            ed.constraint_glyphs(&data).iter().map(|g| &*Box::leak(format!("glyph-{}-{}", g.constraint.0, g.entity.0).into_boxed_str())).collect()
        });
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let glyphs = selectors.into_iter().filter(|s| vcx.debug_bounds(s).is_some()).count();
        // The regions' grey fill in the scene drawn (an invisible pick-only mesh while dragging).
        let filled = cx.update(|cx| {
            let scene = view.read(cx).viewport.read(cx).print_source().1;
            scene.instances.iter().any(|i| i.layer == 9 && !i.pick_only && i.color[3] > 0.0 && matches!(i.drawable, noshape_render::Drawable::Mesh(_)))
        });
        (glyphs, !filled)
    };
    let at_rest = shown(cx);
    let event = |ev: ViewportEvent, cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ev, w, cx);
        }))
        .unwrap();
    };
    let corner = DVec2::new(750., 420.);
    event(ViewportEvent::PointerMove { position: corner, modifiers: Modifiers::default() }, cx);
    event(ViewportEvent::ToolDown { position: corner, modifiers: Modifiers::default(), click_count: 1 }, cx);
    for k in 1..=4 {
        event(ViewportEvent::ToolDrag { position: corner + DVec2::new(5.0, 3.0) * k as f64 }, cx);
    }
    let dragging = shown(cx);
    event(ViewportEvent::ToolUp { position: corner + DVec2::new(20.0, 12.0), modifiers: Modifiers::default() }, cx);
    let released = shown(cx);
    assert!(at_rest.0 > 0 && !at_rest.1, "at rest: {at_rest:?} (glyphs shown, regions unfilled)");
    assert_eq!(dragging, (0, true), "(glyphs shown, regions unfilled) while the corner is dragged");
    assert_eq!(released, (at_rest.0, false), "(glyphs shown, regions unfilled) after the release");
}

/// Live sketch-tool walk 3 (qa FEEL-HOVER-GLYPHS-SECOND-ENTITY), Show constraints off: each hovered edge of a rectangle
/// shows a glyph for every constraint it takes part in, also those drawn on the other line with Show constraints on (the
/// top ⊥ ⫽ —, the right edge its own).
#[gpui::test]
fn a_hovered_line_shows_all_its_constraints(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "glyph-second-entity", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (750., 420.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // Each edge and the constraints with a glyph it takes part in.
    let edges = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let glyphs = ed.constraint_glyphs(&data);
        data.sketch
            .entities()
            .filter(|e| matches!(data.sketch.curve(e.id), Some(noshape_sketch::Curve2::Line { .. })))
            .map(|e| {
                let mine: Vec<_> = glyphs.iter().filter(|g| !g.on_point && data.sketch.constraint(g.constraint).is_some_and(|c| c.kind.entities().contains(&e.id))).map(|g| g.constraint).collect();
                (e.id, mine)
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(edges.len(), 4, "a rectangle");
    assert!(edges.iter().any(|(line, mine)| cx.update(|cx| {
        let glyphs = view.read(cx).sketch.as_ref().unwrap().constraint_glyphs(&view.read(cx).sketch_data(cx).unwrap());
        mine.iter().any(|c| !glyphs.iter().any(|g| g.constraint == *c && g.entity == *line))
    })), "some edge's constraint is drawn on another line with Show constraints on");
    let pointer = Pointer::new(cx, window, &view);
    for (line, mine) in edges {
        pointer.move_to(cx, &view, DVec2::new(30.0, 30.0));
        cx.executor().advance_clock(GLYPH_LINGER + std::time::Duration::from_millis(50));
        cx.run_until_parked();
        // (Read after the clock moved: the new sketch's view turns normal to its plane meanwhile.)
        let on = along_line(cx, &view, line);
        pointer.move_to(cx, &view, on);
        assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover), Some(line), "the line lit");
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let missing: Vec<_> = mine.iter().filter(|c| vcx.debug_bounds(Box::leak(format!("glyph-{}-{}", c.0, line.0).into_boxed_str())).is_none()).collect();
        assert!(missing.is_empty(), "line {line:?}: constraints {missing:?} of {mine:?} not shown");
    }
}

/// Live sketch/11 (qa FEEL-HOVER-GLYPHS-IN-TOOL), Show constraints off: with the Dimension, Extend or Offset tool active
/// a hovered rectangle edge shows only its band, no glyphs.
#[gpui::test]
fn a_tool_hides_a_hovered_lines_glyphs(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "glyph-in-tool", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (750., 420.));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // The top edge (highest in the sketch) and its glyphs' selectors.
    let (line, selectors) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let id = data
            .sketch
            .entities()
            .filter_map(|e| match data.sketch.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => Some((e.id, a.y + b.y)),
                _ => None,
            })
            .max_by(|x, y| x.1.total_cmp(&y.1))
            .expect("a line")
            .0;
        let sel: Vec<&'static str> = ed.focus_glyphs(&data, ed.constraint_glyphs(&data), &[id]).iter().filter(|g| g.entity == id).map(|g| &*Box::leak(format!("glyph-{}-{}", g.constraint.0, g.entity.0).into_boxed_str())).collect();
        (id, sel)
    });
    assert!(!selectors.is_empty(), "the top edge has constraints");
    let pointer = Pointer::new(cx, window, &view);
    for tool in [Tool::Select, Tool::Dimension, Tool::Extend, Tool::Offset] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(tool, w, cx))).unwrap();
        pointer.move_to(cx, &view, DVec2::new(30.0, 30.0));
        cx.executor().advance_clock(GLYPH_LINGER + std::time::Duration::from_millis(50));
        cx.run_until_parked();
        let on = along_line(cx, &view, line);
        pointer.move_to(cx, &view, on);
        assert_eq!(cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover), Some(line), "{tool:?}: the line lit");
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let shown = selectors.iter().filter(|s| vcx.debug_bounds(s).is_some()).count();
        if tool == Tool::Select {
            assert_eq!(shown, selectors.len(), "no tool: the hovered line's glyphs");
        } else {
            assert_eq!(shown, 0, "{tool:?}: no glyphs on the hovered line");
        }
    }
}

/// 40 % along line `line` on screen, as the view is now (a rectangle's short side is ~38 px: clear of its corners'
/// reach).
fn along_line(cx: &mut TestAppContext, view: &Entity<PartStudioView>, line: noshape_sketch::EntityId) -> DVec2 {
    cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let Some(noshape_sketch::Curve2::Line { a, b }) = data.sketch.curve(line) else { panic!("a line") };
        v.viewport.read(cx).project(ed.frame.to_world(a + (b - a) * 0.4)).expect("on screen")
    })
}

/// Live (behaviours "skasm point glyph placement", qa SKETCH-SELECTED-GLYPH-OFFSET): Show constraints off, a selected
/// corner's glyph sits straight above it, its centre ~40 px up and +4..+9 px right, in screen space; with Show
/// constraints on it keeps its box up-left of the point (behaviours 22938).
#[gpui::test]
fn a_selected_points_glyph_sits_above_it(cx: &mut TestAppContext) {
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "glyph-point-above", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (750., 420.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    // A corner (a point with one Coincident glyph) selected, and where it is in viewport px.
    let (corner, at) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let glyphs = ed.constraint_glyphs(&data);
        let corner = data.sketch.entities().map(|e| e.id).find(|id| glyphs.iter().filter(|g| g.entity == *id && g.on_point).count() == 1).expect("a corner with one glyph");
        let p = data.sketch.point(corner).unwrap();
        (corner, v.viewport.read(cx).project(ed.frame.to_world(p)).unwrap())
    });
    let boxes = |cx: &mut TestAppContext, show: bool| {
        cx.update(|cx| view.update(cx, |v, cx| {
            let ed = v.sketch.as_mut().expect("sketch mode");
            ed.set_show_constraints(show);
            ed.selection = vec![corner];
            cx.notify();
        }));
        draw(cx, window);
        cx.update(|cx| view.read(cx).ui.glyph_boxes.iter().filter(|b| b.1 == corner).map(|b| DVec2::new(f64::from(b.2) + 10.0, f64::from(b.3) + 10.0) - at).collect::<Vec<_>>())
    };
    let off = boxes(cx, false);
    assert_eq!(off.len(), 1, "{off:?}");
    assert!((3.0..=9.0).contains(&off[0].x) && (off[0].y + 40.0).abs() <= 1.5, "centred above the point: {off:?}");
    let on = boxes(cx, true);
    assert!(on.iter().any(|c| (c.x + 17.0).abs() <= 0.5 && (c.y + 16.0).abs() <= 0.5), "up-left with Show constraints on: {on:?}");
}
