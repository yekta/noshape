//! Sketch hover through the real pointer and the GPU's own pick, in the user's setup (lead 11-17, user P0): a new Part
//! Studio, Sketch 1 on Top, a rectangle, the default iso view with the default planes shown behind it. The open sketch's
//! entities win over planes and faces, and the hover follows the pointer: on a line the line lights, off it nothing of
//! the sketch stays lit.

use super::feel::{Pointer, gpu_pointer};
use super::*;

/// The open sketch's lines, each with its two ends on screen (viewport px).
fn lines_on_screen(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<(u32, DVec2, DVec2)> {
    cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let vp = v.viewport.read(cx);
        data.sketch
            .entities()
            .map(|e| e.id)
            .filter_map(|id| match data.sketch.curve(id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => Some((id.0, vp.project(ed.frame.to_world(a))?, vp.project(ed.frame.to_world(b))?)),
                _ => None,
            })
            .collect()
    })
}

fn hovered(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Option<Reference> {
    cx.update(|cx| view.read(cx).hover.clone())
}

#[gpui::test]
fn a_sketch_line_lights_over_the_planes_behind_it_and_lets_go(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-planes", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    let lines = lines_on_screen(cx, &view);
    assert_eq!(lines.len(), 4, "the rectangle's four lines");
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    for (id, a, b) in &lines {
        for t in [0.25, 0.5, 0.75] {
            let on = *a + (*b - *a) * t;
            pointer.move_to(cx, &view, on);
            let want = Reference::SketchEntity { sketch: feature, entity: *id };
            let got = hovered(cx, &view);
            if got.as_ref() != Some(&want) {
                wrong.push(format!("on line {id} at {t} ({:.0}, {:.0}): lit {got:?}", on.x, on.y));
            }
            // Far off the sketch (below it, over the background or a plane): nothing of the sketch stays lit.
            let off = DVec2::new(on.x, on.y + 160.0);
            pointer.move_to(cx, &view, off);
            let got = hovered(cx, &view);
            if matches!(got, Some(Reference::SketchEntity { sketch, .. }) if sketch == feature) {
                wrong.push(format!("160 px below line {id} at {t}: still lit {got:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{} of 24 hovers wrong:\n{}", wrong.len(), wrong.join("\n"));
}

