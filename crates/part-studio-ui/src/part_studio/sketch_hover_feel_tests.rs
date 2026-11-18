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

/// Live2 hover-p0 (ours-1118, qa FEEL-SKETCH-REACH-OUTSIDE) and live "feel (3D) retaken 11-18": outside sketch mode a
/// shown sketch's line lights from 7 px (8: not) and stays lit to 10 px on the way out (12: not), a point to 9 px, and
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
    // On a line's middle, resting past the midpoint's dwell: still the line, no tool being open (live2 rr/ln.png).
    let (id, a, b) = lines[0];
    let mid = (a + b) * 0.5;
    pointer.move_to(cx, &view, mid + (b - a).perp().normalize() * 3.0);
    pointer.move_to(cx, &view, mid);
    std::thread::sleep(crate::part_studio::behaviors::MIDPOINT_DWELL + std::time::Duration::from_millis(100));
    pointer.frames(cx, &view);
    let got = hovered(cx, &view);
    if !is_line(&got, id) {
        wrong.push(format!("resting on line {id}'s middle: lit {got:?}, not the line"));
    }
    let corner = lines[0].1;
    let out = (corner - centre).normalize();
    for (d, want) in [(6.0, true), (8.0, true), (11.0, false)] {
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

/// The presented frame's sRGB pixels (device px, BGRA in the image) and its width, after the scene settles.
fn settled_frame(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>) -> (Vec<u8>, usize) {
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    let pointer = Pointer::new(cx, window, view);
    pointer.frames(cx, view);
    cx.update(|cx| vp.update(cx, |v, _| v.record_frames(true)));
    let t = std::time::Instant::now();
    while cx.update(|cx| vp.read(cx).recorded_frames().len()) < 2 && t.elapsed() < std::time::Duration::from_secs(15) {
        cx.update(|cx| vp.update(cx, |v, cx| v.request_frame(cx)));
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
