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

/// The user's setup with a circle (Sketch 1 on Top, default iso, the planes behind): on its rim, 5 px inside and 5 px
/// outside it, the circle lights (a curve's 7 px reach, live2 hov10); 12 px outside nothing of the sketch does; on its
/// centre the centre point lights (a point beats the region it sits in).
#[gpui::test]
fn a_sketch_circle_and_its_centre_light_over_the_planes(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-circle", Tool::Circle);
    click(cx, window, &view, (575., 350.));
    click(cx, window, &view, (665., 350.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (feature, circle, centre, c, rim) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().unwrap();
        let data = v.sketch_data(cx).unwrap();
        let vp = v.viewport.read(cx);
        let circle = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Circle).unwrap().id;
        let centre = data.sketch.entity(circle).unwrap().center().unwrap();
        let Some(noshape_sketch::Curve2::Circle { center, radius }) = data.sketch.curve(circle) else { panic!("a circle") };
        let screen = |q: DVec2| vp.project(ed.frame.to_world(q)).unwrap();
        let rim: Vec<DVec2> = (0..8).map(|k| screen(center + DVec2::from_angle(k as f64 * 45f64.to_radians()) * radius)).collect();
        (ed.feature, circle.0, centre.0, screen(center), rim)
    });
    let pointer = Pointer::new(cx, window, &view);
    let is = |r: &Option<Reference>, id: u32| matches!(r, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && *entity == id);
    let mut wrong = Vec::new();
    for at in &rim {
        let out = (*at - c).normalize();
        for (d, lit) in [(0.0, true), (-5.0, true), (5.0, true), (12.0, false)] {
            pointer.move_to(cx, &view, *at + out * d);
            let got = hovered(cx, &view);
            if is(&got, circle) != lit {
                wrong.push(format!("{d:+} px off the rim at ({:.0}, {:.0}): lit {got:?}", at.x, at.y));
            }
            if !lit && matches!(&got, Some(Reference::SketchEntity { sketch, .. }) if *sketch == feature) {
                wrong.push(format!("{d:+} px off the rim: the sketch still lit {got:?}"));
            }
        }
    }
    pointer.move_to(cx, &view, c);
    let got = hovered(cx, &view);
    if !is(&got, centre) {
        wrong.push(format!("on the centre: lit {got:?}, not the centre point"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// qa P0 USER-HOVER-STAYS-LIT: on a slow GPU the highlight trailed the pointer by several picks. Off a lit line fast
/// to a plane, the pick at rest was asked before the frame with the line's un-lighting went out, so it came back "not
/// current" and was asked again (each ~300 ms there). Latest wins: the first answer after the pointer rests already
/// lights what is under it.
#[gpui::test]
fn the_first_pick_at_rest_lights_what_is_under_the_pointer(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-first-pick", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let lines = lines_on_screen(cx, &view);
    let (_, a, b) = *lines.iter().max_by(|x, y| (x.1.y + x.2.y).total_cmp(&(y.1.y + y.2.y))).unwrap();
    let on = (a + b) * 0.5;
    let pointer = Pointer::new(cx, window, &view);
    // A spot below the rectangle where a plane lights, reached slowly.
    let spot = [140.0, 180.0, 220.0, 100.0]
        .iter()
        .flat_map(|d| [DVec2::new(on.x - d, on.y + d), DVec2::new(on.x + d, on.y + d)])
        .find(|s| {
            pointer.move_to(cx, &view, *s);
            matches!(hovered(cx, &view), Some(Reference::Plane(_)))
        })
        .expect("a plane below the sketch");
    let want = hovered(cx, &view);
    pointer.move_to(cx, &view, on);
    assert!(matches!(hovered(cx, &view), Some(Reference::SketchEntity { .. })), "the line lit");
    // Off fast: no frame drawn, no pick answered on the way.
    for k in 1..=12 {
        let at = pointer.window_point(cx, on + (spot - on) * (k as f64 / 12.0));
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(at, None, Modifiers::default());
    }
    // The pause: the pick at rest is asked; then the frame with the line un-lit goes out; then the GPU answers.
    cx.executor().advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    draw(cx, window);
    cx.update(|cx| view.read(cx).gpu.deliver_frames());
    draw(cx, window);
    assert_eq!(hovered(cx, &view), want, "the first answer at rest lights the plane under the pointer");
}
