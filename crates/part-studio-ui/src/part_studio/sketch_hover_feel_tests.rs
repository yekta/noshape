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



/// Live2's rows (behaviours "USER P0, 11-17 12:2x", hov10-clean): in sketch mode a line reaches 7 px either side, a
/// point 10 px and beats its lines, the region beats the planes behind it, and the sketch's own plane (Top) never
/// hovers.
#[gpui::test]
fn sketch_hover_reach_and_priority_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-reach", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    let lines = lines_on_screen(cx, &view);
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    let centre = lines.iter().fold(DVec2::ZERO, |c, (_, a, b)| c + *a + *b) / (2.0 * lines.len() as f64);
    let is_line = |r: &Option<Reference>, id: u32| matches!(r, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && *entity == id);
    let is_top = |r: &Option<Reference>| matches!(r, Some(Reference::Plane(DatumPlane::Top)));
    for (id, a, b) in &lines {
        let mid = (*a + *b) * 0.5;
        // Outward from the rectangle's centre on screen.
        let mut n = (*b - *a).perp().normalize();
        if n.dot(mid - centre) < 0.0 {
            n = -n;
        }
        for (d, want) in [(6.0, true), (-6.0, true), (9.0, false), (-9.0, false)] {
            pointer.move_to(cx, &view, mid + n * d);
            let got = hovered(cx, &view);
            if is_line(&got, *id) != want {
                wrong.push(format!("line {id} at {d:+} px: lit {got:?}"));
            }
            if is_top(&got) {
                wrong.push(format!("line {id} at {d:+} px: the sketch's own plane lit"));
            }
        }
    }
    // A corner 8 px out along the diagonal: the point, not its lines.
    let corner = lines[0].1;
    pointer.move_to(cx, &view, corner + (corner - centre).normalize() * 8.0);
    let got = hovered(cx, &view);
    let is_point = cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        matches!(&got, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && data.sketch.point(noshape_sketch::EntityId(*entity)).is_some())
    });
    if !is_point {
        wrong.push(format!("8 px off a corner: lit {got:?}, not the point"));
    }
    // Inside the rectangle, away from its lines: the region.
    pointer.move_to(cx, &view, centre);
    let got = hovered(cx, &view);
    if !matches!(got, Some(Reference::SketchRegion { sketch, .. }) if sketch == feature) {
        wrong.push(format!("inside the rectangle: lit {got:?}, not the region"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
