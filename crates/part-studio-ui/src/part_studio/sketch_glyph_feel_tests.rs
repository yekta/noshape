//! A line's constraint glyphs after the pointer leaves it (qa 11-17, the user's flow on c330a760): they linger briefly
//! so the pointer can reach them, then go, with no further pointer move to redraw them away.

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
    pointer.move_to(cx, &view, on + DVec2::new(0.0, 160.0));
    let lingering = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().glyph_entity.map(|(e, _)| e));
    assert_eq!(lingering, Some(line), "the left line's glyphs linger");
    let redraws = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = redraws.clone();
    let _watch = cx.update(|cx| cx.observe(&view, move |_, _| counted.set(counted.get() + 1)));
    // (The linger is measured on the wall clock; the redraw's timer on the test executor's.)
    std::thread::sleep(GLYPH_LINGER);
    cx.executor().advance_clock(GLYPH_LINGER + std::time::Duration::from_millis(10));
    cx.run_until_parked();
    assert!(redraws.get() > 0, "a redraw when the lingering glyphs are due to go");
}
