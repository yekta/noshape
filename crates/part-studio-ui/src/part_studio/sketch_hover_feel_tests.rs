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



/// Live2's rows (behaviours "USER P0, 11-17 12:2x", hov10-clean; live "psui recheck 2" fc11d): in sketch mode a line
/// reaches 7 px into its region and live's 5 px pick square outside it (4 px lit, 5 not), a point 10 px and beats its
/// lines, the region beats the planes behind it, and the sketch's own plane (Top) never hovers.
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
        // Outside: live's 5 px pick square round the pointer (4.9 here), farther across a slanted line.
        let square = 4.9 * (n.x.abs() + n.y.abs());
        for (d, want) in [(4.0, true), (square + 1.0, false), (-7.0, true), (-9.0, false)] {
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

/// Live2 hover-p0 (ours-1118, qa FEEL-SKETCH-REACH-OUTSIDE) and live "feel (3D) retaken 11-18": outside sketch mode a
/// shown sketch's line lights from 7 px (8: not) and stays lit to 10 px on the way out (12: not), a point to 9 px (qa
/// 11-18 on f09f8eca: 10 lit, live's 9: 8.5 px here), and
/// 8 px off a corner along its diagonal the corner point lights, not its lines, drawn as a disc.
#[gpui::test]
fn a_shown_sketchs_line_and_point_reach_outside_sketch_mode_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "shown-sketch-reach", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    let lines = lines_on_screen(cx, &view);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.finish_sketch(w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let centre = lines.iter().fold(DVec2::ZERO, |c, (_, a, b)| c + *a + *b) / (2.0 * lines.len() as f64);
    let is_line = |r: &Option<Reference>, id: u32| matches!(r, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && *entity == id);
    let mut wrong = Vec::new();
    for (id, a, b) in &lines {
        // (Off the midpoint, which turns the hover into the line's midpoint.)
        let mid = *a + (*b - *a) * 0.3;
        let mut n = (*b - *a).perp().normalize();
        if n.dot((*a + *b) * 0.5 - centre) < 0.0 {
            n = -n;
        }
        for (d, want) in [(14.0, false), (8.0, false), (7.0, true), (0.0, true), (-6.0, true), (9.0, true), (10.0, true), (12.0, false), (-14.0, false), (-8.0, false), (-7.0, true)] {
            pointer.move_to(cx, &view, mid + n * d);
            let got = hovered(cx, &view);
            if is_line(&got, *id) != want {
                wrong.push(format!("line {id} at {d:+} px: lit {got:?}"));
            }
        }
    }
    // On a line's middle: its midpoint, as a part edge's (live hover-matrix-1121 ps/sketch-linemid-*).
    let (id, a, b) = lines[0];
    let mid = (a + b) * 0.5;
    pointer.move_to(cx, &view, mid + (b - a).perp().normalize() * 3.0);
    pointer.move_to(cx, &view, mid);
    pointer.frames(cx, &view);
    let got = hovered(cx, &view);
    if !matches!(&got, Some(Reference::Implicit { on, inference: noshape_model::reference::Inference::Midpoint }) if is_line(&Some((**on).clone()), id)) {
        wrong.push(format!("on line {id}'s middle: lit {got:?}, not its midpoint"));
    }
    let corner = lines[0].1;
    let out = (corner - centre).normalize();
    for (d, want) in [(6.0, true), (8.0, true), (10.0, false), (11.0, false)] {
        pointer.move_to(cx, &view, corner + out * d);
        let got = hovered(cx, &view);
        let point = cx.update(|cx| {
            let v = view.read(cx);
            let shown = v.display.sketches.iter().find(|s| s.feature == feature);
            matches!(&got, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && shown.is_some_and(|s| s.points.iter().any(|q| q.entity == *entity)))
        });
        if point != want {
            wrong.push(format!("{d} px off a corner: lit {got:?}"));
        }
        // (Lit, its disc: live2 11-18 pt.png, ~10 px light orange; at rest a corner isn't drawn.)
        if want && cx.update(|cx| view.read(cx).midpoint_overlay(cx).is_empty()) {
            wrong.push(format!("{d} px off a corner: no disc"));
        }
    }
    // Live hover-pick-1120 row 2: moving along a line toward its end the end point takes over 8 px before it (the line
    // back at 9); ours 7 (the point's square): 6 px in the point, 10 px in the line.
    let along = (b - a).normalize();
    for (d, point) in [(10.0, false), (6.0, true)] {
        pointer.move_to(cx, &view, a + along * d);
        let got = hovered(cx, &view);
        let is_point = cx.update(|cx| {
            let shown = view.read(cx).display.sketches.iter().find(|s| s.feature == feature);
            matches!(&got, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && shown.is_some_and(|s| s.points.iter().any(|q| q.entity == *entity)))
        });
        let lit = if point { is_point } else { is_line(&got, id) };
        if !lit {
            wrong.push(format!("{d} px along line {id} from its end: lit {got:?}, want the {}", if point { "point" } else { "line" }));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The user's setup with a circle (Sketch 1 on Top, default iso, the planes behind): on its rim, 5 px inside (a curve's
/// 7 px reach into its region, live2 hov10) and 4 px outside it (live's 5 px pick square, fc11d), the circle lights;
/// 12 px outside nothing of the sketch does; on its centre the centre point lights (a point beats the region it sits in).
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
        for (d, lit) in [(0.0, true), (-5.0, true), (4.0, true), (12.0, false)] {
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
    // The GPU's answer on its own thread, waited for without the test clock moving (lavapipe under load: seconds).
    let t = std::time::Instant::now();
    loop {
        cx.update(|cx| view.read(cx).gpu.deliver_frames());
        draw(cx, window);
        if hovered(cx, &view).is_some() || t.elapsed() > std::time::Duration::from_secs(30) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(hovered(cx, &view), want, "the first answer at rest lights the plane under the pointer");
}

/// Heavy Part Studio feel (live drift-1120: on software GL a move off clears in 5-7 ms): with frames read back, a
/// hover cleared back to a frame drawn before shows that frame again without drawing, pixel for pixel, and picks answer
/// for it (back on the line, the line lights again). Reuse is opt-in (`NOSHAPE_FRAME_CACHE=1`) until a click after a
/// frame shown again draws on time.
#[gpui::test]
fn a_cleared_hover_shows_the_frame_drawn_before_without_drawing(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-reuse", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (_, a, b) = lines_on_screen(cx, &view)[0];
    let on = (a + b) * 0.5;
    let pointer = Pointer::new(cx, window, &view);
    let off = [(-300.0, -200.0), (300.0, -200.0), (-300.0, 200.0), (300.0, 200.0), (0.0, -250.0)]
        .iter()
        .map(|(x, y)| on + DVec2::new(*x, *y))
        .find(|s| {
            pointer.move_to(cx, &view, *s);
            hovered(cx, &view).is_none()
        })
        .expect("a spot where nothing lights");
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    cx.update(|cx| vp.update(cx, |v, _| v.set_reuse_frames(true)));
    let (before, _) = settled_frame(cx, window, &view);
    let reused = cx.update(|cx| vp.read(cx).frames_reused());
    pointer.move_to(cx, &view, on);
    assert!(matches!(hovered(cx, &view), Some(Reference::SketchEntity { .. })), "the line lit");
    let (lit, _) = settled_frame(cx, window, &view);
    assert!(lit != before, "the lit line drawn");
    pointer.move_to(cx, &view, off);
    assert_eq!(hovered(cx, &view), None, "nothing lit off the line");
    let (after, w) = settled_frame(cx, window, &view);
    let reused = cx.update(|cx| vp.read(cx).frames_reused()) - reused;
    assert!(reused > 0, "the cleared frame shown again (none reused)");
    assert!(after == before, "the frame shown again is the one drawn before the hover: {}, lit {:?}", frame_diff(&before, &after, w), hovered(cx, &view));
    pointer.move_to(cx, &view, on);
    assert!(matches!(hovered(cx, &view), Some(Reference::SketchEntity { .. })), "back on the line, it lights again");
}

/// qa USER-HOVER-STAYS-LIT (v0.4.57, the user's flow): from each line of the rectangle the pointer sweeps off in 12
/// quick steps (16 ms apart) to a spot well outside it, crossing the rectangle on the way, and rests; nothing of the
/// sketch stays lit (the region the sweep crossed included), and back on the line the line lights again.
#[gpui::test]
fn a_quick_sweep_off_a_line_leaves_nothing_lit(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-sweep", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    let lines = lines_on_screen(cx, &view);
    let pointer = Pointer::new(cx, window, &view);
    let centre = lines.iter().fold(DVec2::ZERO, |c, (_, a, b)| c + *a + *b) / (2.0 * lines.len() as f64);
    let lit_sketch = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let region = v.sketch.as_ref().and_then(|s| s.hover_region.clone());
            let ours = matches!(&v.hover, Some(Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. }) if *sketch == feature);
            (ours || region.is_some()).then(|| format!("{:?} / region {region:?}", v.hover))
        })
    };
    let mut wrong = Vec::new();
    for (id, a, b) in &lines {
        let mid = (*a + *b) * 0.5;
        pointer.move_to(cx, &view, mid);
        // Off through the rectangle to 250 px beyond its centre, the other side.
        let far = centre + (centre - mid).normalize() * 250.0;
        for k in 1..=12 {
            let at = mid + (far - mid) * (k as f64 / 12.0);
            let win = pointer.window_point(cx, at);
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
            cx.update(|cx| view.read(cx).gpu.deliver_frames());
            draw(cx, window);
            cx.executor().advance_clock(std::time::Duration::from_millis(16));
        }
        pointer.move_to(cx, &view, far);
        pointer.frames(cx, &view);
        if let Some(lit) = lit_sketch(cx) {
            wrong.push(format!("line {id}: after the sweep off it, still lit: {lit}"));
        }
        pointer.move_to(cx, &view, mid);
        let got = hovered(cx, &view);
        if !matches!(&got, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && entity == id) {
            wrong.push(format!("line {id}: back on it, lit {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// skasm 11-18 (the user's hat15b, a 1440 × 900 window, its 145-feature list scrolled to the end, a sketch edited, n,
/// 14 wheel steps in): a line lit with the pointer ~10 px left of it. In windows of several sizes, the list at its top
/// and scrolled to the end: the viewport's origin is where its element is laid out, and the pointer on a line, placed
/// from that element, lights the line.
#[gpui::test]
fn the_pointer_lights_the_line_under_it_whatever_the_window_and_list(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let mut features = vec![serde_json::json!({ "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] })];
    for n in 1..=144 {
        features.push(serde_json::json!({ "type": "cPlane", "name": format!("Plane {n}"), "params": { "entities": ["plane:top"], "cplaneType": "OFFSET", "offset": format!("{n} mm") } }));
    }
    let mut wrong = Vec::new();
    for (w, h) in [(1440.0f32, 900.0f32), (1919.0, 1000.0), (1100.0, 700.0)] {
        for scrolled in [false, true] {
            let (window, view) = part_studio_from(cx, "pointer-origin", Some(serde_json::json!({ "units": "mm", "features": features })));
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_resize(gpui_kit::size(px(w), px(h)));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            if scrolled {
                cx.update(|cx| view.read(cx).ui.list_scroll.set_offset(gpui_kit::point(px(0.), px(-100_000.))));
                draw(cx, window);
            }
            let (window, view) = sketch_in(cx, window, view, Tool::CornerRect);
            click(cx, window, &view, (300., 200.));
            click(cx, window, &view, (420., 330.));
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
            keys(cx, window, &["n"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).viewport.read(cx).animation().is_none());
            let pointer = Pointer::new(cx, window, &view);
            pointer.frames(cx, &view);
            let laid = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("viewport-area").expect("the viewport's area").origin;
            let origin = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
            if (f32::from(origin.x - laid.x)).abs() > 0.5 || (f32::from(origin.y - laid.y)).abs() > 0.5 {
                wrong.push(format!("{w}x{h} scrolled {scrolled}: viewport origin {origin:?}, laid out at {laid:?}"));
            }
            let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
            for (id, a, b) in lines_on_screen(cx, &view).into_iter().filter(|(_, a, b)| (a.x - b.x).abs() < 1.0) {
                let on = (a + b) * 0.5;
                let at = gpui_kit::point(laid.x + px(on.x as f32), laid.y + px(on.y as f32));
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(at, None, Modifiers::default());
                let t = std::time::Instant::now();
                while !cx.update(|cx| view.read(cx).viewport.read(cx).hover_settled()) && t.elapsed() < std::time::Duration::from_secs(15) {
                    pointer.frames(cx, &view);
                }
                pointer.frames(cx, &view);
                let got = hovered(cx, &view);
                if got != Some(Reference::SketchEntity { sketch: feature, entity: id }) {
                    wrong.push(format!("{w}x{h} scrolled {scrolled}: on vertical line {id} at ({:.0}, {:.0}): lit {got:?}", on.x, on.y));
                }
            }
            cx.update_window(window, |_, w, _| w.remove_window()).unwrap();
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// How two frames of width `w` (BGRA) differ: pixels, their bounds and the largest channel step.
fn frame_diff(a: &[u8], b: &[u8], w: usize) -> String {
    if a.len() != b.len() {
        return format!("sizes {} and {}", a.len(), b.len());
    }
    let (mut n, mut max, mut lo, mut hi) = (0, 0, (usize::MAX, usize::MAX), (0, 0));
    for (i, (p, q)) in a.chunks(4).zip(b.chunks(4)).enumerate() {
        let d = p.iter().zip(q).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        if d == 0 {
            continue;
        }
        let (x, y) = (i % w.max(1), i / w.max(1));
        (n, max, lo, hi) = (n + 1, max.max(d), (lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y)));
    }
    format!("{n} px differ (up to {max}) in {lo:?}..{hi:?}")
}

/// The presented frame's sRGB pixels (device px, BGRA in the image) and its width, after the scene settles.
pub(super) fn settled_frame(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>) -> (Vec<u8>, usize) {
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    let pointer = Pointer::new(cx, window, view);
    pointer.frames(cx, view);
    cx.update(|cx| vp.update(cx, |v, _| v.record_frames(true)));
    let t = std::time::Instant::now();
    while cx.update(|cx| vp.read(cx).recorded_frames().len()) < 2 && t.elapsed() < std::time::Duration::from_secs(15) {
        // Asked for only once the last is back (a GPU slower than 10 ms a frame would always have one in flight).
        if !cx.update(|cx| vp.read(cx).frame_in_flight()) {
            cx.update(|cx| vp.update(cx, |v, cx| v.request_frame(cx)));
        }
        cx.update(|cx| view.read(cx).gpu.deliver_frames());
        draw(cx, window);
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let img = cx.update(|cx| vp.read(cx).recorded_frames()).last().cloned().expect("a presented frame");
    cx.update(|cx| vp.update(cx, |v, _| v.record_frames(false)));
    let (w, h) = (img.size(0).width.0 as usize, img.size(0).height.0 as usize);
    let px = img.as_bytes(0).map(|b| b.to_vec()).unwrap_or_default();
    if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let png = noshape_scene::png::encode_png(px.clone(), w as u32, h as u32, w as u32, h as u32).unwrap();
        std::fs::write(format!("{dir}/sketch-hl-{n}.png"), png).unwrap();
    }
    (px, w)
}

/// live2 hl-pixels-1118 (sketch mode, an active rectangle, normal to it), at 1× and 2×: a line ≈ 2 px blue (4 device px
/// at 2×); hovered a 5 px band (9 device px) holding an opaque #FFC685 rim; selected the same band in #FFA90A; a
/// point hovered an ~11–12 px disc with an #FFC685 rim, selected #FFA90A. (`NOSHAPE_TEST_OUT=<dir>` keeps the frames.)
#[gpui::test]
fn sketch_lines_and_points_light_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    const HOVER: [u8; 3] = [0xFF, 0xC6, 0x85];
    const SELECT: [u8; 3] = [0xFF, 0xA9, 0x0A];
    let mut wrong = Vec::new();
    for scale in [1.0f32, 2.0] {
        let (window, view) = sketch_with(cx, &format!("sketch-hl-{scale}"), Tool::CornerRect);
        cx.update_window(window, |_, w, _| w.set_scale_factor(scale)).unwrap();
        click(cx, window, &view, (500., 300.));
        click(cx, window, &view, (650., 400.));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
        // Normal to the sketch: its lines horizontal and vertical on screen.
        keys(cx, window, &["n"]);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).viewport.read(cx).animation().is_none());
        let lines = lines_on_screen(cx, &view);
        let (_, a, b) = *lines.iter().max_by(|x, y| (x.1.y + x.2.y).total_cmp(&(y.1.y + y.2.y))).unwrap();
        let mid = (a + b) * 0.5;
        let corner = if a.x < b.x { a } else { b };
        let (s, k) = (scale as f64, scale as usize);
        // sRGB pixels through `at`: down a column, or along a row.
        let cut = |px: &[u8], w: usize, at: DVec2, horizontal: bool| -> Vec<[u8; 3]> {
            let (cx0, cy0) = ((at.x * s).round() as i64, (at.y * s).round() as i64);
            let n = (12.0 * s) as i64;
            (-n..=n)
                .map(|d| {
                    let (x, y) = if horizontal { (cx0 + d, cy0) } else { (cx0, cy0 + d) };
                    let o = (y as usize * w + x as usize) * 4;
                    [px[o + 2], px[o + 1], px[o]]
                })
                .collect()
        };
        let show = |c: &[[u8; 3]]| c.iter().map(|c| format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])).collect::<Vec<_>>().join(" ");
        let away = mid + DVec2::new(40.0, 120.0);
        let pointer = Pointer::new(cx, window, &view);
        pointer.move_to(cx, &view, away);
        let (px, w) = settled_frame(cx, window, &view);
        let rest = cut(&px, w, mid + DVec2::new(20.0, 0.0), false);
        let blue = rest.iter().filter(|c| c[0] < 0x80 && c[2] > 0xC0).count();
        if !(k..=2 * k + 1).contains(&blue) {
            wrong.push(format!("{scale}x line at rest: {blue} blue px: {}", show(&rest)));
        }
        let band = |col: &[[u8; 3]], colour: [u8; 3], what: &str, wrong: &mut Vec<String>| {
            let lit: Vec<usize> = (0..col.len()).filter(|&i| col[i] != rest[i]).collect();
            let width = lit.last().zip(lit.first()).map_or(0, |(b, a)| b - a + 1);
            if !(4 * k..=6 * k + 1).contains(&width) || !col.contains(&colour) {
                wrong.push(format!("{scale}x line {what}: a {width} px band: {}", show(col)));
            }
        };
        pointer.move_to(cx, &view, mid);
        let (px, w) = settled_frame(cx, window, &view);
        band(&cut(&px, w, mid + DVec2::new(20.0, 0.0), false), HOVER, "hovered", &mut wrong);
        pointer.click(cx, &view, mid);
        pointer.move_to(cx, &view, away);
        let (px, w) = settled_frame(cx, window, &view);
        band(&cut(&px, w, mid + DVec2::new(20.0, 0.0), false), SELECT, "selected", &mut wrong);
        // The point's disc: from its rim on the left (away from the lines) to the one on the right.
        let disc = |row: &[[u8; 3]], colour: [u8; 3], what: &str, wrong: &mut Vec<String>| {
            let rim: Vec<usize> = (0..row.len()).filter(|&i| row[i] == colour).collect();
            let width = rim.last().zip(rim.first()).map_or(0, |(b, a)| b - a + 1) as f64 / s;
            if !(9.5..=12.5).contains(&width) {
                wrong.push(format!("{scale}x point {what}: a {width:.1} px disc: {}", show(row)));
            }
        };
        pointer.move_to(cx, &view, corner);
        let (px, w) = settled_frame(cx, window, &view);
        disc(&cut(&px, w, corner, true), HOVER, "hovered", &mut wrong);
        pointer.click(cx, &view, corner);
        pointer.move_to(cx, &view, away);
        let (px, w) = settled_frame(cx, window, &view);
        disc(&cut(&px, w, corner, true), SELECT, "selected", &mut wrong);
        cx.update_window(window, |_, w, _| w.remove_window()).unwrap();
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// qa 11-18 SELFEEL-DBLCLICK-FACE-SKETCH (live2 11-17 sel-feel row 3), through the real pointer and the GPU's pick: a
/// part's top face double-clicked draws its feature's hidden sketch at the part's base (its curves in the scene) while
/// the feature and the sketch are related.
#[gpui::test]
fn a_face_double_clicked_draws_its_sketch_through_the_pointer(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    // qa's set-up: the two-blocks start, Part 2's top (Extrude 2 of Sketch 2).
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "dblclick-gpu", Some(starts["two-blocks"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 2);
    let pointer = Pointer::new(cx, window, &view);
    let at = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).nth(1).unwrap();
        let top_z = b.mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        let top: Vec<glam::DVec3> = b.mesh.positions.iter().filter(|p| (p[2] - top_z).abs() < 1e-7).map(|p| glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).collect();
        let c = top.iter().copied().sum::<glam::DVec3>() / top.len() as f64;
        v.viewport.read(cx).project(c).unwrap()
    });
    pointer.move_to(cx, &view, at);
    let win = pointer.window_point(cx, at);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    for n in [1, 2] {
        vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: win, modifiers: Modifiers::default(), click_count: n, first_mouse: false });
        vcx.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: win, modifiers: Modifiers::default(), click_count: n });
    }
    pointer.frames(cx, &view);
    pointer.move_to(cx, &view, at + DVec2::new(2.0, 0.0));
    let (related, shown, drawn) = cx.update(|cx| {
        let v = view.read(cx);
        let drawn = v.built.as_ref().is_some_and(|b| (1..20_000).filter_map(|i| b.picks.get(i)).any(|t| matches!(t.reference, Reference::SketchEntity { .. })));
        (v.related_features(cx).len(), v.sketches_shown_for_now().len(), drawn)
    });
    // Live feel/3d dblclick-face.png: drawn through the part, blue with its corner dots, as an edited sketch looks.
    let through = cx.update(|cx| {
        let sketch = view.read(cx).sketches_shown_for_now();
        view.update(cx, |v, cx| v.scene_options(cx).through_sketches.iter().filter(|f| sketch.contains(f)).count())
    });
    assert_eq!((related, shown, drawn, through), (2, 1, true, 1), "(related rows, sketches shown for now, its curves in the scene, drawn through the parts)");
}

/// live2 hover-time-1118: every move picks at once only while a sketch creation tool is active (live's
/// `alwaysPreHighlightOnMouseMove`: Line, Dimension); in a sketch with no tool, and outside a sketch, a fast move's pick
/// waits for the pause (the sketch's 270–300 ms with no tool vs ~30 ms with Dimension).
#[gpui::test]
fn only_a_sketch_tool_picks_on_every_move(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "pick-on-move", Tool::Select);
    let at_once = |cx: &mut TestAppContext, tool: Option<Tool>| {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                match tool {
                    Some(t) => v.activate_tool(t, w, cx),
                    None => v.leave_sketch(cx),
                }
                v.push_tool_cursor(cx);
            })
        })
        .unwrap();
        cx.update(|cx| view.read(cx).viewport.read(cx).pick_on_move)
    };
    let got = [at_once(cx, Some(Tool::Line)), at_once(cx, Some(Tool::Dimension)), at_once(cx, Some(Tool::Select)), at_once(cx, None)];
    assert_eq!(got, [true, true, false, false], "(Line, Dimension, no tool, outside the sketch): every move picks at once");
}

/// qa REGION-PICK-SKETCH-SHOWN (live session-1118 step 5): with Extrude's "Faces and sketch regions" field active, a
/// small shown circle's region lights on its centre and between the centre and the rim; the field takes no sketch
/// points or curves, so their reach outside sketch mode doesn't apply.
#[gpui::test]
fn a_small_circles_region_lights_in_a_region_field(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "region-field-small-circle", Tool::Circle);
    click(cx, window, &view, (575., 350.));
    click(cx, window, &view, (593., 350.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (feature, c) = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().unwrap();
        let data = v.sketch_data(cx).unwrap();
        let circle = data.sketch.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Circle).unwrap().id;
        let Some(noshape_sketch::Curve2::Circle { center, .. }) = data.sketch.curve(circle) else { panic!("a circle") };
        (ed.feature, v.viewport.read(cx).project(ed.frame.to_world(center)).unwrap())
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.finish_sketch(w, cx);
        v.selection.clear();
        v.start_feature("extrude", w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    for off in [DVec2::ZERO, DVec2::new(5.0, 0.0), DVec2::new(0.0, -7.0)] {
        pointer.move_to(cx, &view, c + off);
        let got = hovered(cx, &view);
        if !matches!(&got, Some(Reference::SketchRegion { sketch, .. }) if *sketch == feature) {
            wrong.push(format!("{off:?} off the centre: lit {got:?}, not the region"));
        }
    }
    // The click puts the region in Extrude's field.
    pointer.click(cx, &view, c + DVec2::new(5.0, 0.0));
    let picked = cx.update(|cx| match view.read(cx).edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.get("entities").cloned()) {
        Some(noshape_model::params::ParamValue::Query(refs)) => refs,
        _ => Vec::new(),
    });
    if !matches!(picked.as_slice(), [Reference::SketchRegion { sketch, .. }] if *sketch == feature) {
        wrong.push(format!("the click put {picked:?} in the field, not the region"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// qa FLOW-REGION-UNDER-FACE-PICK (enclosure flow cell 61; live behaviours.md: a sketch region on a part's face wins
/// over the face under it): an open box's rim sketched over with a larger rectangle; in Extrude the rim's region
/// lights and is picked, not the rim face under it, and so is the ring beside the box once the preview covers it.
#[gpui::test]
fn a_sketch_region_on_a_rim_face_wins_over_the_face(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = part_studio_with(cx, "region-over-rim", |path| {
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| {
            let op = noshape_ops::registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
            noshape_ops::registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
        };
        // (The ops' session closed before the window opens the file.)
        {
            let ctx = &mut noshape_ops::Ctx::new();
            call(ctx, "document_create", serde_json::json!({ "path": path, "length_unit": "mm" }));
            let s1 = call(ctx, "sketch_create", serde_json::json!({ "plane": "plane:top" }))["sketch"].clone();
            let d = call(ctx, "sketch_draw", serde_json::json!({ "sketch": s1, "tool": "corner_rectangle", "points": [[0, 0], [60, 40]] }));
            call(ctx, "feature_add", serde_json::json!({ "type": "extrude", "params": { "entities": d["regions"].as_array().unwrap().iter().map(|r| r.get("id").unwrap_or(r).clone()).collect::<Vec<_>>(), "endBound": "BLIND", "depth": "30 mm", "operationType": "NEW" } }));
            call(ctx, "feature_add", serde_json::json!({ "type": "fillet", "params": { "entities": { "query": { "kind": "edges", "at": [[0, 0, 15], [60, 0, 15], [60, 40, 15], [0, 40, 15]] } }, "radius": "5 mm" } }));
            call(ctx, "feature_add", serde_json::json!({ "type": "shell", "params": { "entities": { "query": { "kind": "faces", "at": [[30, 20, 30]] } }, "thickness": "2 mm" } }));
            let rim = call(ctx, "query_entities", serde_json::json!({ "kind": "faces", "at": [[1, 20, 30]] }))["faces"][0]["id"].clone();
            let s2 = call(ctx, "sketch_create", serde_json::json!({ "plane": rim }))["sketch"].clone();
            call(ctx, "sketch_draw", serde_json::json!({ "sketch": s2, "tool": "corner_rectangle", "points": [[-6, -6, 30], [66, 46, 30]] }));
        }
        crate::session::Session::open(path).unwrap()
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.sketches.len() == 2 && !view.read(cx).display.bodies.is_empty());
    let sketch = cx.update(|cx| view.read(cx).display.sketches.iter().find(|s| s.name == "Sketch 2").expect("Sketch 2").feature);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    let on_screen = |cx: &mut TestAppContext, p: [f64; 3]| cx.update(|cx| view.read(cx).viewport.read(cx).project(glam::DVec3::from_array(p) * 0.001)).expect("on screen");
    for (what, p) in [("the rim", [1., 20., 30.]), ("the rim's far side", [59., 20., 30.]), ("the opening", [30., 20., 30.]), ("the outer ring", [-1., 20., 30.])] {
        let at = on_screen(cx, p);
        pointer.move_to(cx, &view, at);
        let got = hovered(cx, &view);
        if !matches!(&got, Some(Reference::SketchRegion { sketch: s, .. }) if *s == sketch) {
            wrong.push(format!("{what}: lit {got:?}, not a region of Sketch 2"));
        }
    }
    // As qa's flow: the opening first, then the rim, then the ring beside the box (over the preview, no part face behind).
    for p in [[30., 20., 30.], [1., 20., 30.], [-1., 20., 30.]] {
        let at = on_screen(cx, p);
        pointer.move_to(cx, &view, at);
        pointer.move_to(cx, &view, at + DVec2::new(1.0, 0.0));
        pointer.move_to(cx, &view, at);
        let got = hovered(cx, &view);
        if !matches!(&got, Some(Reference::SketchRegion { sketch: s, .. }) if *s == sketch) {
            wrong.push(format!("{p:?} in Extrude: lit {got:?}, not a region of Sketch 2"));
        }
        pointer.click(cx, &view, at);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let rim = on_screen(cx, [1., 20., 30.]);
        pointer.move_to(cx, &view, rim + DVec2::new(2.0, 0.0));
        pointer.move_to(cx, &view, rim);
    }
    let picked = cx.update(|cx| match view.read(cx).edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.get("entities").cloned()) {
        Some(noshape_model::params::ParamValue::Query(refs)) => refs,
        _ => Vec::new(),
    });
    if !(picked.len() == 3 && picked.iter().all(|r| matches!(r, Reference::SketchRegion { sketch: s, .. } if *s == sketch))) {
        wrong.push(format!("the clicks on the opening, the rim and the outer ring put {picked:?} in the field, not three regions"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// What of `feature` is lit: the hover, the sketch editor's own hover (sketch mode) and a shown point's disc.
fn sketch_lit(cx: &mut TestAppContext, view: &Entity<PartStudioView>, feature: noshape_model::reference::FeatureId) -> Vec<String> {
    cx.update(|cx| {
        let v = view.read(cx);
        let mut lit = Vec::new();
        if let Some(h) = v.hover.as_ref().filter(|h| matches!(h, Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } if *sketch == feature)) {
            lit.push(format!("hover {h:?}"));
        }
        if let Some(e) = v.sketch.as_ref().and_then(|ed| ed.hover) {
            lit.push(format!("the editor's hover {e:?}"));
        }
        if v.sketch.is_none() && !v.midpoint_overlay(cx).is_empty() {
            lit.push("a point's disc".into());
        }
        lit
    })
}

/// qa P0 STALE-SKETCH-HOVER (b), in sketch mode: a line drawn, Esc, ONE move to empty space: its end lit under the
/// pointer goes dark with that move (it stayed lit until a second move).
#[gpui::test]
fn a_lines_lit_end_goes_dark_with_one_move_away(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "stale-end-gpu", Tool::Line);
    let pointer = Pointer::new(cx, window, &view);
    pointer.click(cx, &view, DVec2::new(400., 300.));
    pointer.move_to(cx, &view, DVec2::new(520., 300.));
    pointer.click(cx, &view, DVec2::new(520., 300.));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    let mut wrong = Vec::new();
    // Resting on the end, the line's chain going on (the pointer as qa's: still there after the click), then Esc.
    pointer.move_to(cx, &view, DVec2::new(521., 301.));
    eprintln!("stale-end: in Line on the end: lit {:?}", sketch_lit(cx, &view, feature));
    keys(cx, window, &["escape"]);
    pointer.frames(cx, &view);
    let tool = cx.update(|cx| view.read(cx).sketch.as_ref().map(|ed| ed.tool));
    eprintln!("stale-end: after Esc the tool {tool:?}, lit {:?}", sketch_lit(cx, &view, feature));
    pointer.move_to(cx, &view, DVec2::new(700., 480.));
    let lit = sketch_lit(cx, &view, feature);
    if !lit.is_empty() {
        wrong.push(format!("Esc on the end, one move away: still lit {lit:?}"));
    }
    // Lit in Select 2 px off the end, one move away.
    pointer.move_to(cx, &view, DVec2::new(522., 301.));
    if sketch_lit(cx, &view, feature).is_empty() {
        wrong.push("2 px off the end in Select: not lit".into());
    }
    pointer.move_to(cx, &view, DVec2::new(700., 480.));
    let lit = sketch_lit(cx, &view, feature);
    if !lit.is_empty() {
        wrong.push(format!("lit in Select, one move away: still lit {lit:?}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// qa P0 STALE-SKETCH-HOVER (b) and (a), outside sketch mode: a circle's centre point lit, then one move off (and a view
/// key under the resting pointer): nothing of the sketch stays lit; the circle drawn with its rim click last, ✓, then
/// moves away: its region's outline doesn't stay.
#[gpui::test]
fn a_shown_circles_centre_and_region_go_dark_with_the_move_away(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "stale-centre-gpu", Tool::Circle);
    let pointer = Pointer::new(cx, window, &view);
    pointer.click(cx, &view, DVec2::new(575., 350.));
    pointer.click(cx, &view, DVec2::new(665., 350.));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.finish_sketch(w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
    pointer.frames(cx, &view);
    let mut wrong = Vec::new();
    for (k, at) in [(700., 500.), (300., 200.), (720., 150.)].into_iter().enumerate() {
        pointer.move_to(cx, &view, DVec2::new(at.0, at.1));
        let lit = sketch_lit(cx, &view, feature);
        if !lit.is_empty() {
            wrong.push(format!("after ✓ with the pointer on the rim, move {} away: lit {lit:?}", k + 1));
        }
    }
    let centre = cx.update(|cx| {
        let v = view.read(cx);
        let s = v.display.sketches.iter().find(|s| s.feature == feature).unwrap();
        let vp = v.viewport.read(cx);
        s.points.iter().filter_map(|q| vp.project(glam::DVec3::new(q.position[0] as f64, q.position[1] as f64, q.position[2] as f64))).min_by(|a, b| a.distance(DVec2::new(575., 350.)).total_cmp(&b.distance(DVec2::new(575., 350.)))).unwrap()
    });
    pointer.move_to(cx, &view, centre + DVec2::new(3., 2.));
    if sketch_lit(cx, &view, feature).is_empty() {
        wrong.push("3 px off the centre: the centre point not lit".into());
    }
    pointer.move_to(cx, &view, centre + DVec2::new(150., 140.));
    let lit = sketch_lit(cx, &view, feature);
    if !lit.is_empty() {
        wrong.push(format!("one move off the centre: lit {lit:?}"));
    }
    // 7 px off: the point by its reach, the GPU's pick there the region, also once the view has moved.
    pointer.move_to(cx, &view, centre + DVec2::new(7., 0.));
    if !sketch_lit(cx, &view, feature).iter().any(|l| l.contains("SketchEntity")) {
        wrong.push("7 px off the centre: the centre point not lit".into());
    }
    keys(cx, window, &["shift"]);
    cx.update(|cx| view.update(cx, |v, cx| v.view(noshape_render::StandardView::Top, cx)));
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_secs(15) {
        pointer.frames(cx, &view);
        if cx.update(|cx| !pointer.vp.read(cx).animating() && pointer.vp.read(cx).hover_settled()) {
            break;
        }
    }
    // (The view's re-pick waits for 60 ms of quiet, then the GPU answers.)
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(1500) {
        pointer.frames(cx, &view);
    }
    let (away, at) = cx.update(|cx| {
        let v = view.read(cx);
        let s = v.display.sketches.iter().find(|s| s.feature == feature).unwrap();
        let vp = v.viewport.read(cx);
        let at = vp.pointer_position();
        (s.points.iter().filter_map(|q| vp.project(glam::DVec3::new(q.position[0] as f64, q.position[1] as f64, q.position[2] as f64))).all(|q| q.distance(at) > 12.0), at)
    });
    let lit: Vec<String> = sketch_lit(cx, &view, feature).into_iter().filter(|l| !l.contains("SketchRegion")).collect();
    if away && !lit.is_empty() {
        wrong.push(format!("the view key moved the centre from under the resting pointer ({:.0}, {:.0}): lit {lit:?}", at.x, at.y));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// qa P0 STALE-SKETCH-HOVER (c) and (a): a sketch closed (✕, or ✓ from its dialog) with an end lit under the pointer
/// takes its lit look with it: no point's disc drawn where the cancelled sketch's end was, nothing lit after the pointer
/// comes back from the dialog's button over empty space.
#[gpui::test]
fn a_closed_sketchs_lit_end_goes_with_it(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let mut wrong = Vec::new();
    for cancel in [true, false] {
        let (window, view) = sketch_with(cx, if cancel { "closed-lit-x" } else { "closed-lit-ok" }, Tool::Line);
        let pointer = Pointer::new(cx, window, &view);
        pointer.click(cx, &view, DVec2::new(400., 300.));
        pointer.move_to(cx, &view, DVec2::new(520., 300.));
        pointer.click(cx, &view, DVec2::new(520., 300.));
        keys(cx, window, &["escape"]);
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
        pointer.move_to(cx, &view, DVec2::new(522., 301.));
        if sketch_lit(cx, &view, feature).is_empty() {
            wrong.push(format!("cancel {cancel}: the end not lit 2 px off it"));
        }
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            if cancel {
                v.cancel_edit(w, cx);
            } else {
                v.finish_sketch(w, cx);
            }
        }))
        .unwrap();
        wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && !view.read(cx).ui_busy());
        pointer.frames(cx, &view);
        let lit = sketch_lit(cx, &view, feature);
        if cancel && !lit.is_empty() {
            wrong.push(format!("cancelled, the pointer still where the end was: lit {lit:?}"));
        }
        // (Where the dialog's button is: outside the graphics.)
        pointer.move_to(cx, &view, DVec2::new(-40., -40.));
        let lit = sketch_lit(cx, &view, feature);
        if !lit.is_empty() {
            wrong.push(format!("cancel {cancel}: closed, the pointer on its button: still lit {lit:?}"));
        }
        pointer.move_to(cx, &view, DVec2::new(700., 480.));
        let lit = sketch_lit(cx, &view, feature);
        if !lit.is_empty() {
            wrong.push(format!("cancel {cancel}: back over empty space: still lit {lit:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live sk2-1121 dim-circle-rims (default iso, ~3.2 px a mm): the Dimension tool lights a 5 mm circle round the Origin on
/// its rim either side, not the Origin at its centre, before and after a dimension between its rim and another circle's.
#[gpui::test]
fn a_small_circle_round_the_origin_lights_on_its_rim(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-rims", Tool::Circle);
    cx.update(|cx| view.update(cx, |v, _| v.sketch.as_mut().unwrap().set_show_constraints(true)));
    let screen = |cx: &mut TestAppContext, x: f64, y: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().unwrap();
            v.viewport.read(cx).project(ed.frame.to_world(DVec2::new(x, y) * 1e-3)).unwrap()
        })
    };
    for (c, r) in [((0., 0.), (5., 0.)), ((30., 0.), (38., 0.))] {
        let (a, b) = (screen(cx, c.0, c.1), screen(cx, r.0, r.1));
        click(cx, window, &view, (a.x, a.y));
        click(cx, window, &view, (b.x, b.y));
    }
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (feature, circle) = cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        let c = data.sketch.entities().find(|e| matches!(data.sketch.curve(e.id), Some(noshape_sketch::Curve2::Circle { center, .. }) if center.length() < 1e-9)).unwrap().id;
        (v.sketch.as_ref().unwrap().feature, c.0)
    });
    let pointer = Pointer::new(cx, window, &view);
    let is_circle = |r: &Option<Reference>| matches!(r, Some(Reference::SketchEntity { sketch, entity }) if *sketch == feature && *entity == circle);
    let mut wrong = Vec::new();
    let mut rims = |cx: &mut TestAppContext, when: &str| {
        for x in [5., -5.] {
            let at = screen(cx, x, 0.);
            pointer.move_to(cx, &view, at);
            let got = hovered(cx, &view);
            if !is_circle(&got) {
                let o = screen(cx, 0., 0.);
                wrong.push(format!("{when}: at ({x}, 0) mm, {:.1} px from the centre, lit {got:?}", (at - o).length()));
            }
        }
    };
    rims(cx, "before a dimension");
    for (x, y) in [(5., 0.), (22., 0.), (15., -10.)] {
        let at = screen(cx, x, y);
        pointer.move_to(cx, &view, at);
        pointer.click(cx, &view, at);
    }
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("enter escape d");
    draw(cx, window);
    rims(cx, "after one");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live plane-behind-1121 (sk2-1121 dim-ellipse-axes): with the Dimension tool's ellipse picked no plane's face lights
/// (the Front plane's lights at (5, 25) mm before), a plane's outline still does, and a click at (5, 25) on nothing
/// dimensions the major axis (a label above the horizontal ellipse).
#[gpui::test]
fn an_ellipse_picked_for_a_dimension_takes_no_plane_face(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-ellipse-planes", Tool::Ellipse);
    let screen = |cx: &mut TestAppContext, x: f64, y: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().unwrap();
            v.viewport.read(cx).project(ed.frame.to_world(DVec2::new(x, y) * 1e-3)).unwrap()
        })
    };
    for (x, y) in [(0., 0.), (30., 0.), (0., 15.)] {
        let a = screen(cx, x, y);
        click(cx, window, &view, (a.x, a.y));
    }
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let plane = |r: &Option<Reference>| matches!(r, Some(Reference::Plane(_)));
    let above = screen(cx, 5., 25.);
    pointer.move_to(cx, &view, above);
    let before = hovered(cx, &view);
    assert!(plane(&before), "nothing picked: a plane's face lights at (5, 25) mm, lit {before:?}");
    // Just inside the Right plane's +y outline (75 mm out), 40 mm above the sketch plane.
    let edge = cx.update(|cx| view.read(cx).viewport.read(cx).project(glam::DVec3::new(0., 0.074, 0.04)).unwrap());
    pointer.move_to(cx, &view, edge);
    let edge_before = hovered(cx, &view);
    let rim = screen(cx, 0., 15.);
    pointer.move_to(cx, &view, rim);
    pointer.click(cx, &view, rim);
    let mut wrong = Vec::new();
    pointer.move_to(cx, &view, above);
    let got = hovered(cx, &view);
    if plane(&got) {
        wrong.push(format!("the ellipse picked: at (5, 25) mm lit {got:?}"));
    }
    pointer.move_to(cx, &view, edge);
    let got = hovered(cx, &view);
    if got != Some(Reference::Plane(DatumPlane::Right)) {
        wrong.push(format!("the ellipse picked: by the Right plane's outline lit {got:?} (before the pick {edge_before:?})"));
    }
    // Live's (40, 5) mm lies by the Right plane's +y outline: that lights it.
    let by_edge = screen(cx, 40., 5.);
    pointer.move_to(cx, &view, by_edge);
    let got = hovered(cx, &view);
    let (outline_d, vp_size) = cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        let c = [(0., 0.075, -0.075), (0., 0.075, 0.075)].map(|(x, y, z)| vp.project(glam::DVec3::new(x, y, z)).unwrap());
        let t = ((by_edge - c[0]).dot(c[1] - c[0]) / (c[1] - c[0]).length_squared()).clamp(0., 1.);
        ((c[0] + (c[1] - c[0]) * t).distance(by_edge), vp.pointer_position())
    });
    if got != Some(Reference::Plane(DatumPlane::Right)) {
        wrong.push(format!("the ellipse picked: at (40, 5) mm ({outline_d:.1} px from the Right plane's +y outline, pointer {vp_size:?}) lit {got:?}"));
    }
    pointer.move_to(cx, &view, above);
    pointer.click(cx, &view, above);
    let made = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.constraints().filter_map(|c| match c.kind { noshape_sketch::ConstraintKind::EllipseRadius { major, .. } => Some(major), _ => None }).collect::<Vec<_>>());
    if made != vec![true] {
        wrong.push(format!("a click at (5, 25) mm: ellipse dimensions {made:?}, want the major"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live dimplane-1122 (behaviours "psui plane while dimensioning", qa FEEL-PLANE-LIT-WHILE-DIMENSIONING): with the
/// Dimension tool's line picked a plane lights only with the pointer on its outline (~2 px), never over its inside;
/// with nothing picked, over its face (any plane in front there).
#[gpui::test]
fn a_line_picked_for_a_dimension_lights_a_plane_only_on_its_outline(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-dim-plane-outline", Tool::Line);
    let screen = |cx: &mut TestAppContext, x: f64, y: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().unwrap();
            v.viewport.read(cx).project(ed.frame.to_world(DVec2::new(x, y) * 1e-3)).unwrap()
        })
    };
    for (x, y) in [(0., 10.), (30., 10.)] {
        let a = screen(cx, x, y);
        click(cx, window, &view, (a.x, a.y));
    }
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).highlighted_hover(cx));
    let right = vec![Reference::Plane(DatumPlane::Right)];
    // Inside the Right plane, 40 mm above the sketch plane; and 1 px inside its +y outline at that height.
    let (inside, outline) = cx.update(|cx| {
        let vp = view.read(cx).viewport.read(cx);
        let p = |y: f64| vp.project(glam::DVec3::new(0., y, 0.04)).unwrap();
        let (o, c) = (p(0.075), p(0.0));
        (p(0.04), o + (c - o).normalize() * 1.0)
    });
    pointer.move_to(cx, &view, inside);
    assert!(matches!(lit(cx).as_slice(), [Reference::Plane(_)]), "nothing picked: a plane lights over its face, lit {:?}", lit(cx));
    let on_line = screen(cx, 15., 10.);
    pointer.move_to(cx, &view, on_line);
    pointer.click(cx, &view, on_line);
    pointer.move_to(cx, &view, inside);
    assert!(lit(cx).is_empty(), "the line picked: inside the Right plane nothing lights, lit {:?}", lit(cx));
    pointer.move_to(cx, &view, outline);
    assert_eq!(lit(cx), right, "the line picked: on the Right plane's outline it lights");
}

/// Live ui-parity sketch-fully-defined (11-25 retake; behaviours "psui plane while dimensioning" (3)): a 30 x 20
/// rectangle's right side picked at (30, 10), the click at (38, 10) is on the Right plane's outline and picks it (the
/// line and the plane selected, no dimension placed, no value box).
#[gpui::test]
fn a_click_on_the_right_planes_outline_picks_it_for_the_dimension(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-dim-outline-pick", Tool::CornerRect);
    let screen = |cx: &mut TestAppContext, x: f64, y: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().unwrap();
            v.viewport.read(cx).project(ed.frame.to_world(DVec2::new(x, y) * 1e-3)).unwrap()
        })
    };
    for (x, y) in [(0., 0.), (30., 20.)] {
        let a = screen(cx, x, y);
        click(cx, window, &view, (a.x, a.y));
    }
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let side = screen(cx, 30., 10.);
    pointer.move_to(cx, &view, side);
    pointer.click(cx, &view, side);
    let at = screen(cx, 38., 10.);
    let outline = cx.update(|cx| {
        let v = view.read(cx);
        v.display.planes.iter().find(|q| q.reference == Reference::Plane(DatumPlane::Right)).map(|q| crate::part_studio::sketch::plane_outline_px(q, at, v.viewport.read(cx)))
    });
    pointer.move_to(cx, &view, at);
    let lit = cx.update(|cx| view.read(cx).highlighted_hover(cx));
    pointer.click(cx, &view, at);
    let got = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.sketch_data(cx).unwrap();
        (v.sketch.as_ref().unwrap().tool_picks().len(), d.sketch.constraints().filter(|c| c.kind.is_dimension()).count(), v.dim_input.is_some())
    });
    assert!(got == (2, 0, false), "(model picks, dimensions, box) {got:?}; lit {lit:?}, {outline:?} px off the outline");
}

/// Live sketch-entity-copy-paste: a line right-clicked and copied from its menu, a right-click where its glyph row was
/// opens the plain sketch menu, Paste second: the menu ended the line's hover, so the row was gone by the Copy click
/// (ours kept the hover under the menu, and the row lingered from the pointer's return onto the glyph).
#[gpui::test]
fn a_right_click_where_a_copied_lines_glyphs_were_opens_the_sketch_menu(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-copy-paste-menu", Tool::Line);
    // The scenario's scale: 0.275 mm a px.
    cx.update(|cx| view.read(cx).viewport.clone()).update(cx, |vp, cx| {
        let h = vp.size().y;
        vp.camera.distance = 0.275365e-3 * h * 0.5 / (vp.camera.fov_y * 0.5).tan();
        vp.request_frame(cx);
    });
    draw(cx, window);
    let screen = |cx: &mut TestAppContext, x: f64, y: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().unwrap();
            v.viewport.read(cx).project(ed.frame.to_world(DVec2::new(x, y) * 1e-3)).unwrap()
        })
    };
    for (x, y) in [(0., 0.), (30., 0.)] {
        let a = screen(cx, x, y);
        click(cx, window, &view, (a.x, a.y));
    }
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape escape");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    // Near the line's end, so the way from the menu to its glyph row crosses no line.
    let on_line = screen(cx, 27., 0.);
    pointer.move_to(cx, &view, on_line);
    pointer.right_click(cx, &view, on_line);
    let glyph = cx.update(|cx| view.read(cx).ui.glyph_boxes.first().map(|b| DVec2::new(f64::from(b.2) + 10.0, f64::from(b.3) + 10.0))).expect("the line's glyph row");
    let target = pointer.window_point(cx, glyph);
    let item = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("menu:Copy sketch entities").expect("Copy sketch entities").center();
    // As the scenario runner: three moves to each target, a rest, then the click.
    let mut from = pointer.window_point(cx, on_line);
    for to in [item, target] {
        for k in 1..=3 {
            let t = k as f32 / 3.0;
            let p = gpui_kit::point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(p, None, Modifiers::default());
            pointer.frames(cx, &view);
        }
        if to == item {
            cx.executor().advance_clock(std::time::Duration::from_millis(700));
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(item, Modifiers::default());
            pointer.settle(cx, &view);
        }
        from = to;
    }
    let held = cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref();
        format!("{:?}; glyphs {:?} for the pointer at {glyph:?}; hover {:?} / {:?}, linger {:?} at {:?}, row entity {:?}", ed.and_then(|ed| ed.hover_constraint), v.ui.glyph_boxes, ed.and_then(|ed| ed.hover), v.hover, v.ui.glyph_linger, cx.background_executor().now(), ed.and_then(|ed| ed.glyph_entity))
    });
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(target, gpui_kit::MouseButton::Right, Modifiers::default());
    vcx.simulate_mouse_up(target, gpui_kit::MouseButton::Right, Modifiers::default());
    pointer.settle(cx, &view);
    let rows: Vec<String> = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default()).into_iter().filter(|l| l != "———").collect();
    assert!(rows.get(1).map(String::as_str) == Some("Paste sketch entities") && !rows.iter().any(|r| r.starts_with("Delete")), "{rows:?}; glyph hover {held}");
}

/// Live sketch-fully-defined: with a side picked for a dimension, a click 1.5 px outside the Right plane's outline, over no
/// plane's face, picks the plane (the length placed before).
#[gpui::test]
fn a_click_just_outside_the_right_planes_outline_picks_it_for_the_dimension(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-dim-outline-outside", Tool::CornerRect);
    let screen = |cx: &mut TestAppContext, x: f64, y: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().unwrap();
            v.viewport.read(cx).project(ed.frame.to_world(DVec2::new(x, y) * 1e-3)).unwrap()
        })
    };
    for (x, y) in [(0., 0.), (30., 20.)] {
        let a = screen(cx, x, y);
        click(cx, window, &view, (a.x, a.y));
    }
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    let side = screen(cx, 30., 10.);
    pointer.move_to(cx, &view, side);
    pointer.click(cx, &view, side);
    let near = screen(cx, 38., 10.);
    let at = cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        let q = v.display.planes.iter().find(|q| q.reference == Reference::Plane(DatumPlane::Right)).unwrap();
        let c = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(u, w)| vp.project(q.corner(u, w)).unwrap());
        let mid = vp.project(q.corner(0.0, 0.0)).unwrap();
        (0..4)
            .map(|k| {
                let (a, b) = (c[k], c[(k + 1) % 4]);
                let foot = a + (b - a) * ((near - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
                let n = DVec2::new(a.y - b.y, b.x - a.x).normalize();
                foot + n * 1.5 * (foot - mid).dot(n).signum()
            })
            .min_by(|x, y| x.distance(near).total_cmp(&y.distance(near)))
            .unwrap()
    });
    pointer.move_to(cx, &view, at);
    let lit = cx.update(|cx| view.read(cx).highlighted_hover(cx));
    pointer.click(cx, &view, at);
    let got = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.sketch_data(cx).unwrap();
        (v.sketch.as_ref().unwrap().tool_picks().len(), d.sketch.constraints().filter(|c| c.kind.is_dimension()).count(), v.dim_input.is_some())
    });
    assert!(got == (2, 0, false), "(model picks, dimensions, box) {got:?}; lit {lit:?} at {at:?}");
}

/// qa SKETCH-DIM-PLACE-PICKS-PLANE (workflow 01 steps 15-20, live: a linear dimension of the side): with the pointer
/// last over the Front plane's face (lit, nothing picked yet), a quick click on the rectangle's right side and then beside
/// it places the side's length; the stale hover's plane, off its outline, isn't picked (a 90° angle to Front before).
#[gpui::test]
fn a_quick_placement_click_after_a_plane_hover_places_the_length(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-hover-dim-quick-place", Tool::CornerRect);
    let screen = |cx: &mut TestAppContext, x: f64, y: f64| {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().unwrap();
            v.viewport.read(cx).project(ed.frame.to_world(DVec2::new(x, y) * 1e-3)).unwrap()
        })
    };
    for (x, y) in [(0., 0.), (40., 30.)] {
        let a = screen(cx, x, y);
        click(cx, window, &view, (a.x, a.y));
    }
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Dimension, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pointer = Pointer::new(cx, window, &view);
    // Inside the Front plane's face, 40 mm above the sketch.
    let before = cx.update(|cx| view.read(cx).viewport.read(cx).project(glam::DVec3::new(0.02, 0.0, 0.04)).unwrap());
    pointer.move_to(cx, &view, before);
    assert_eq!(hovered(cx, &view), Some(Reference::Plane(DatumPlane::Front)), "nothing picked: the Front plane lit under the pointer");
    for (x, y) in [(40., 15.), (48., 15.)] {
        let a = screen(cx, x, y);
        click(cx, window, &view, (a.x, a.y));
    }
    keys(cx, window, &["2", "5", "enter"]);
    let right = cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).expect("sketch data");
        data.sketch.entities().find_map(|e| match data.sketch.curve(e.id) {
            Some(noshape_sketch::Curve2::Line { a, b }) if a.x > 0.03 && b.x > 0.03 => Some((b - a).length()),
            _ => None,
        })
    });
    let right = right.expect("the rectangle's right side");
    assert!((right - 0.025).abs() < 1e-9, "the right side dimensioned to 25 mm, it is {:.4} mm", right * 1e3);
}

/// The pixels across a point marker's middle row, drawn alone at a device pixel ratio of 2 (as the user's shots), from
/// its centre out to `reach` px.
fn marker_row(sprites: &[noshape_render::PointSprite], reach: i32) -> Vec<[u8; 3]> {
    use noshape_render::{Camera, Drawable, Instance, PointData, RenderParams, Renderer, Scene, StandardView, Style};
    let mut r = Renderer::new().expect("a renderer");
    let points = PointData { points: sprites.iter().map(|p| noshape_render::PointSprite { position: [0.0; 3], ..*p }).collect() };
    let scene = Scene { instances: vec![Instance::new(Drawable::Points(r.upload_points(&points))).per_primitive_color()], ..Default::default() };
    let mut cam = Camera::default();
    cam.set_view(StandardView::Front);
    cam.fit_sphere(glam::DVec3::ZERO, 0.03, 1.0);
    let size = 96u32;
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: size, height: size, device_pixel_ratio: 2.0, style: &Style::default(), pick: false }).expect("a frame");
    let (w, c) = (f.width as i32, (f.width / 2) as i32);
    let mid = (f.height / 2) as i32;
    (0..=reach).map(|dx| {
        let i = ((mid * w + c + dx) * 4) as usize;
        [f.bgra[i + 2], f.bgra[i + 1], f.bgra[i]]
    }).collect()
}

/// User 11-21 (hover-point / hover-point-on-selected-edge shots, 2x): an edge midpoint arrived at along the edge is an
/// opaque orange marker with no dark outline: a light centre (#ffd8ac; on a selected edge solid orange #ffb32e) about 3 px out, a
/// darker ring at about 5 px, the orange disc at 9-10 px, nothing past 12 px.
#[gpui::test]
fn a_hovered_edge_midpoints_marker_has_lives_pixels(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (_window, view, _top, _) = block_part(cx, "edge-mid-pixels");
    let edge = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        b.edge_refs.iter().find(|e| v.edge_midpoint(e).is_some()).expect("an edge with ends").clone()
    });
    // Live's row through the marker's centre, px 0..=12 out (hover-point-onshape.png, the left half mirrored).
    let live_plain: [[u8; 3]; 13] = [
        [255, 216, 172], [255, 216, 172], [255, 216, 172], [249, 210, 165], [201, 160, 114], [192, 149, 100], [203, 158, 106],
        [217, 169, 114], [237, 186, 128], [247, 193, 132], [249, 194, 132], [157, 140, 120], [115, 115, 115],
    ];
    let mut wrong = Vec::new();
    for selected in [false, true] {
        let sprites = view.update(cx, |v, cx| {
            v.selection = if selected { vec![edge.clone()] } else { Vec::new() };
            v.hover = Some(Reference::Implicit { on: Box::new(edge.clone()), inference: noshape_model::reference::Inference::Midpoint });
            v.midpoint_sprites(cx)
        });
        let row = marker_row(&sprites, 12);
        eprintln!("marker row (selected {selected}): {row:?}");
        let near = |a: [u8; 3], b: [u8; 3], tol: i32| a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= tol);
        let centre = if selected { [255, 179, 46] } else { live_plain[0] };
        for (dx, want, tol) in [(0, centre, 12), (1, centre, 12), (5, live_plain[5], 30), (9, live_plain[9], 24)] {
            if !near(row[dx], want, tol) {
                wrong.push(format!("selected {selected}: {dx} px out {:?}, live {want:?}", row[dx]));
            }
        }
        if let Some((dx, p)) = row.iter().enumerate().find(|(_, p)| p.iter().all(|c| *c < 60)) {
            wrong.push(format!("selected {selected}: a dark outline pixel {p:?} {dx} px out (live has none)"));
        }
        if near(row[12], live_plain[9], 30) {
            wrong.push(format!("selected {selected}: still orange 12 px out {:?}", row[12]));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live walk3-1121 (6a), qa FEEL-TANGENT-ARC-AT-ORIGIN: with the real pointer and the GPU's picks, a line ended on the
/// Origin, then Tangent arc clicked there and at (15, 12) mm: the arc is made, Tangent to the line (qa's 11-22 take saw
/// a dashed straight preview and no arc; not reproduced here).
#[gpui::test]
fn a_tangent_arc_from_a_line_end_on_the_origin_with_the_pointer(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "tarc-origin-pointer", Tool::Line);
    let px = |cx: &mut TestAppContext, p: DVec2| -> DVec2 {
        cx.update(|cx| {
            let v = view.read(cx);
            let ed = v.sketch.as_ref().expect("sketch mode");
            v.viewport.read(cx).project(ed.frame.to_world(p)).expect("on screen")
        })
    };
    let pointer = Pointer::new(cx, window, &view);
    let origin = px(cx, DVec2::ZERO);
    let start = px(cx, DVec2::new(-0.03, 0.0));
    click(cx, window, &view, (start.x, start.y));
    click(cx, window, &view, (origin.x, origin.y));
    keys(cx, window, &["escape"]);
    let lines = cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).count()));
    assert_eq!(lines, 1, "the line to the origin");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::TangentArc, w, cx))).unwrap();
    pointer.click(cx, &view, origin);
    let end = px(cx, DVec2::new(0.015, 0.012));
    pointer.move_to(cx, &view, (origin + end) * 0.5);
    pointer.click(cx, &view, end);
    let (arcs, tangent, msg) = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.sketch_data(cx).expect("sketch data");
        let arcs = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Arc).count();
        let tangent = d.sketch.constraints().any(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Tangent { .. }));
        let kinds: Vec<_> = d.sketch.constraints().map(|c| format!("{:?}", c.kind)).chain(d.sketch.entities().filter_map(|e| d.sketch.curve(e.id)).map(|c| format!("{c:?}"))).collect();
        (arcs, tangent, (v.sketch.as_ref().and_then(|ed| ed.message.clone()), kinds))
    });
    assert_eq!(arcs, 1, "the arc made ({msg:?}; line from {start:?} to the origin at {origin:?})");
    assert!(tangent, "Tangent to the line");
}
