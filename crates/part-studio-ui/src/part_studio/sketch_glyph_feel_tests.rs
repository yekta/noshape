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
