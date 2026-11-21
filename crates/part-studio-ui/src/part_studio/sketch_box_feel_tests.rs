//! Box select in sketch mode, iso, the default planes behind the sketch (live2 "USER P0, 11-17 12:2x" Part 2, "Box
//! select in iso over the planes"): a press on a plane and a drag draws the box (the plane doesn't drag); left → right
//! round the whole rectangle takes its 4 lines (no region, no planes, no points); right → left over its right half
//! takes the bottom, right and top lines (not the left).

use super::feel::{Pointer, gpu_pointer};
use super::*;

/// The open sketch's lines: id and both ends on screen (viewport px).
fn lines(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<(u32, DVec2, DVec2)> {
    cx.update(|cx| {
        let v = view.read(cx);
        let ed = v.sketch.as_ref().expect("sketch mode");
        let data = v.sketch_data(cx).expect("sketch data");
        let vp = v.viewport.read(cx);
        data.sketch
            .entities()
            .filter_map(|e| match data.sketch.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => Some((e.id.0, vp.project(ed.frame.to_world(a))?, vp.project(ed.frame.to_world(b))?)),
                _ => None,
            })
            .collect()
    })
}

fn drag(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, pointer: &Pointer, from: DVec2, to: DVec2) {
    pointer.move_to(cx, view, from);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(pointer.window_point(cx, from), gpui_kit::MouseButton::Left, Modifiers::default());
    draw(cx, window);
    for i in 1..=12 {
        let at = from + (to - from) * (i as f64 / 12.0);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_move(pointer.window_point(cx, at), Some(gpui_kit::MouseButton::Left), Modifiers::default());
        pointer.frames(cx, view);
    }
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_up(pointer.window_point(cx, to), gpui_kit::MouseButton::Left, Modifiers::default());
    draw(cx, window);
}

fn selected(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> (Vec<u32>, Vec<Reference>) {
    cx.update(|cx| {
        let v = view.read(cx);
        let mut lines: Vec<u32> = v.sketch.as_ref().unwrap().selection.iter().map(|e| e.0).collect();
        lines.sort();
        (lines, v.selection.clone())
    })
}

/// (Not Esc: live's Esc leaves a sketch selection as it is, behaviours "Clicking empty space clears the selection".)
fn clear(cx: &mut TestAppContext, view: &Entity<PartStudioView>) {
    view.update(cx, |v, _| {
        if let Some(ed) = &mut v.sketch {
            ed.selection.clear();
        }
    });
}

#[gpui::test]
fn a_box_in_iso_over_the_planes_takes_lines_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-box-iso", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let all = lines(cx, &view);
    assert_eq!(all.len(), 4, "the rectangle's four lines");
    let ends: Vec<DVec2> = all.iter().flat_map(|(_, a, b)| [*a, *b]).collect();
    let lo = ends.iter().fold(DVec2::splat(f64::MAX), |m, p| m.min(*p));
    let hi = ends.iter().fold(DVec2::splat(f64::MIN), |m, p| m.max(*p));
    // The line ending furthest left (the short left edge in iso): a crossing box starting just right of it takes the
    // other three, as live's over the rectangle's right part.
    let right_end = |l: &(u32, DVec2, DVec2)| l.1.x.max(l.2.x);
    let (left, start_x) = all.iter().map(|l| (l.0, right_end(l) + 5.0)).min_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
    assert!(all.iter().filter(|l| l.0 != left).all(|l| right_end(l) > start_x + 10.0), "the other lines reach past the box's start");
    let mut want_right: Vec<u32> = all.iter().map(|l| l.0).filter(|id| *id != left).collect();
    want_right.sort();
    let mut want_all: Vec<u32> = all.iter().map(|l| l.0).collect();
    want_all.sort();
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    // Left → right, a window round the whole rectangle, from over the planes.
    drag(cx, window, &view, &pointer, lo - DVec2::splat(25.0), hi + DVec2::splat(25.0));
    let (got, model) = selected(cx, &view);
    if got != want_all || !model.is_empty() {
        wrong.push(format!("window round it: lines {got:?} (want {want_all:?}), else {model:?}"));
    }
    clear(cx, &view);
    // Right → left, crossing over the rectangle's right part.
    drag(cx, window, &view, &pointer, hi + DVec2::splat(25.0), DVec2::new(start_x, lo.y - 25.0));
    let (got, model) = selected(cx, &view);
    if got != want_right || !model.is_empty() {
        wrong.push(format!("crossing over its right part from x {start_x:.0}: lines {got:?} (want {want_right:?}), else {model:?}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live2 "feel (sketch)" (behaviours 39910): with no tool a click on a line adds it (A, then B: both), a click on a
/// selected line removes it, Shift+click toggles the same way, and Ctrl+click opens the sketch's context menu without
/// toggling; in iso with the planes behind, through the real pointer.
#[gpui::test]
fn sketch_clicks_toggle_and_ctrl_opens_the_menu_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-click-mods", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let all = lines(cx, &view);
    let (a, b) = (&all[0], &all[1]);
    let pointer = Pointer::new(cx, window, &view);
    let press = |cx: &mut TestAppContext, at: DVec2, modifiers: Modifiers| {
        pointer.move_to(cx, &view, at);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_click(pointer.window_point(cx, at), modifiers);
        draw(cx, window);
        selected(cx, &view).0
    };
    let sorted = |mut v: Vec<u32>| {
        v.sort();
        v
    };
    let (on_a, on_b) = ((a.1 + a.2) * 0.5, (b.1 + b.2) * 0.5);
    let shift = Modifiers { shift: true, ..Default::default() };
    let ctrl = Modifiers { control: true, ..Default::default() };
    let mut wrong = Vec::new();
    let mut row = |what: &str, got: Vec<u32>, want: Vec<u32>| {
        if got != want {
            wrong.push(format!("{what}: {got:?}, want {want:?}"));
        }
    };
    row("click A", press(cx, on_a, Modifiers::default()), vec![a.0]);
    row("then click B", press(cx, on_b, Modifiers::default()), sorted(vec![a.0, b.0]));
    row("click A again", press(cx, on_a, Modifiers::default()), vec![b.0]);
    row("Shift+click A", press(cx, on_a, shift), sorted(vec![a.0, b.0]));
    row("Shift+click B", press(cx, on_b, shift), vec![a.0]);
    row("Ctrl+click B", press(cx, on_b, ctrl), vec![a.0]);
    assert!(cx.update(|cx| view.read(cx).context_menu.is_some()), "Ctrl+click opened the sketch's context menu");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live2 Part 2 (iso, planes behind): with no tool a line's midpoint hovers the line (midpoints snap only while a tool
/// draws); a glyph click selects its constraint and a second glyph click replaces it; a click on a plane selects the
/// plane (it isn't empty space).
#[gpui::test]
fn midpoint_glyph_and_plane_clicks_in_a_sketch_as_live(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-glyph-plane-clicks", Tool::CornerRect);
    click(cx, window, &view, (450., 300.));
    click(cx, window, &view, (750., 420.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let feature = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    let all = lines(cx, &view);
    let (line, a, b) = *all.iter().max_by(|x, y| (x.2 - x.1).length().total_cmp(&(y.2 - y.1).length())).unwrap();
    let pointer = Pointer::new(cx, window, &view);
    let mut wrong = Vec::new();
    pointer.move_to(cx, &view, (a + b) * 0.5);
    let got = cx.update(|cx| view.read(cx).hover.clone());
    if got != Some(Reference::SketchEntity { sketch: feature, entity: line }) {
        wrong.push(format!("a line's midpoint, no tool: lit {got:?}, not the line"));
    }
    // The glyph row (Show constraints off: shown while its line is hovered) of the line with the most glyphs, two
    // glyphs clicked in turn.
    let rows: Vec<(u32, DVec2, DVec2, Vec<noshape_sketch::ConstraintId>)> = cx.update(|cx| {
        let v = view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        let glyphs = v.sketch.as_ref().unwrap().constraint_glyphs(&data);
        all.iter().map(|(id, a, b)| (*id, *a, *b, glyphs.iter().filter(|g| g.entity.0 == *id && !g.on_point && g.offset.is_none()).map(|g| g.constraint).collect())).collect()
    });
    let (line, a, b, row) = rows.into_iter().max_by_key(|r| r.3.len()).unwrap();
    assert!(row.len() >= 2, "a line with two glyphs or more: {row:?}");
    pointer.move_to(cx, &view, a + (b - a) * 0.3);
    let spot = pointer.window_point(cx, a + (b - a) * 0.3);
    let mut at = spot;
    for (k, cid) in row.iter().take(2).enumerate() {
        let selector: &'static str = Box::leak(format!("glyph-{}-{}", cid.0, line).into_boxed_str());
        let Some(to) = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(selector).map(|b| b.center()) else {
            let state = cx.update(|cx| {
                let ed = view.read(cx).sketch.as_ref().unwrap();
                format!("hover {:?}, hover_constraint {:?}, glyph_entity {:?}, selected {:?}", ed.hover, ed.hover_constraint, ed.glyph_entity.map(|g| g.0), ed.selected_constraints)
            });
            panic!("glyph {} of line {line} not shown: {state}", k + 1);
        };
        for i in 1..=6 {
            let t = i as f32 / 6.0;
            let p = gpui_kit::point(at.x + (to.x - at.x) * t, at.y + (to.y - at.y) * t);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_mouse_move(p, None, Modifiers::default());
            // (Drawn, no time passing: the glyphs' linger runs on the test clock.)
            draw(cx, window);
        }
        // (Where it is drawn now, under the pointer: the row may settle as the hover moves onto it.)
        let to = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(selector).map_or(to, |b| b.center());
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_move(to, None, Modifiers::default());
        draw(cx, window);
        at = to;
        let under = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().hover_constraint);
        if under != Some(*cid) {
            let state = cx.update(|cx| {
                let ed = view.read(cx).sketch.as_ref().unwrap();
                format!("hover {:?}, glyph_entity {:?}, row_at {:?}, selected {:?}", ed.hover, ed.glyph_entity.map(|g| (g.0, g.1.elapsed())), ed.glyph_row_at, ed.selected_constraints)
            });
            let rows: Vec<String> = row.iter().map(|c| {
                let sel: &'static str = Box::leak(format!("glyph-{}-{}", c.0, line).into_boxed_str());
                format!("{c:?} {:?}", gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(sel))
            }).collect();
            wrong.push(format!("glyph {} under the pointer at {to:?}: {under:?}, want {cid:?}; {state}; row {rows:?}", k + 1));
        }
        // Over a glyph nothing else lights (live2 hover-p0), whatever line runs under the row.
        let lit = cx.update(|cx| (view.read(cx).hover.clone(), view.read(cx).sketch.as_ref().unwrap().hover));
        if lit != (None, None) {
            wrong.push(format!("glyph {} hovered: also lit {lit:?}", k + 1));
        }
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_click(to, Modifiers::default());
        draw(cx, window);
        let selected = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().selected_constraints.to_vec());
        if selected != vec![*cid] {
            wrong.push(format!("glyph click {}: selected {selected:?}, want [{cid:?}]", k + 1));
        }
    }
    // A plane in view beside the sketch: the first spot below and right of it where a plane lights.
    let lo = all.iter().flat_map(|l| [l.1, l.2]).fold(DVec2::splat(f64::MAX), |m, p| m.min(p));
    let hi = all.iter().flat_map(|l| [l.1, l.2]).fold(DVec2::splat(f64::MIN), |m, p| m.max(p));
    let spots = [DVec2::new(hi.x + 60.0, hi.y + 40.0), DVec2::new(lo.x - 60.0, hi.y + 40.0), DVec2::new((lo.x + hi.x) * 0.5, hi.y + 80.0), DVec2::new(hi.x + 120.0, lo.y)];
    let plane = spots.iter().find_map(|s| {
        pointer.move_to(cx, &view, *s);
        cx.update(|cx| view.read(cx).hover.clone()).filter(|r| matches!(r, Reference::Plane(_))).map(|r| (*s, r))
    });
    let (spot, plane) = plane.expect("a plane lit beside the sketch");
    pointer.click(cx, &view, spot);
    let selected = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().model_selection.clone());
    if !selected.contains(&plane) {
        wrong.push(format!("a plane clicked in sketch mode: selection {selected:?}, want {plane:?}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Live2 "feel (sketch)" (interaction-feel "Clearing; double-click"): editing a sketch with no tool, Escape keeps the
/// selection and a click on empty space clears it; a double-click on a rectangle's line selects it then deselects it,
/// taking none of the other lines (no chain); through the real pointer.
#[gpui::test]
fn sketch_escape_keeps_an_empty_click_clears_and_a_double_click_toggles_twice(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = sketch_with(cx, "sketch-clear-dbl", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let a = lines(cx, &view)[0];
    let on_a = (a.1 + a.2) * 0.5;
    let pointer = Pointer::new(cx, window, &view);
    let press = |cx: &mut TestAppContext, at: DVec2, click_count: usize| {
        pointer.move_to(cx, &view, at);
        let win = pointer.window_point(cx, at);
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: win, modifiers: Modifiers::default(), click_count, first_mouse: false });
        vcx.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: win, modifiers: Modifiers::default(), click_count });
        draw(cx, window);
        selected(cx, &view).0
    };
    let mut got = vec![("click A", press(cx, on_a, 1))];
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    draw(cx, window);
    got.push(("Escape", selected(cx, &view).0));
    // Empty space: a spot where nothing lights (the Sketch dialog covers the viewport's top-left corner).
    let size = cx.update(|cx| pointer.vp.read(cx).size());
    let empty = [(0.5, 0.92), (0.1, 0.92), (0.9, 0.92), (0.1, 0.5), (0.5, 0.08)].into_iter().map(|(u, v)| DVec2::new(size.x * u, size.y * v)).find(|&at| {
        pointer.move_to(cx, &view, at);
        cx.update(|cx| view.read(cx).hover.is_none())
    });
    let empty = empty.expect("an empty spot in the viewport");
    got.push(("click empty space", press(cx, empty, 1)));
    got.push(("double-click A, first click", press(cx, on_a, 1)));
    got.push(("double-click A, second click", press(cx, on_a, 2)));
    let still_editing = cx.update(|cx| view.read(cx).sketch_data(cx).is_some());
    let want = vec![("click A", vec![a.0]), ("Escape", vec![a.0]), ("click empty space", vec![]), ("double-click A, first click", vec![a.0]), ("double-click A, second click", vec![])];
    assert_eq!((got, still_editing), (want, true), "(selection after each step, still editing the sketch)");
}

/// Live (interaction-feel "Nothing recoloured under the pointer mid-draw"): a sketch line under the pointer lights with
/// no tool; once a line tool's first click is down the same line under the pointer isn't recoloured (the snap shows).
#[gpui::test]
fn a_sketch_line_under_the_pointer_mid_draw_is_not_recoloured(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "mid-draw-no-recolour", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let line = lines(cx, &view)[0].0;
    let lit = |cx: &mut TestAppContext| cx.update(|cx| {
        view.update(cx, |v, _| {
            let sketch = v.sketch.as_ref().expect("sketch mode").feature;
            v.hover = Some(Reference::SketchEntity { sketch, entity: line });
        });
        !view.read(cx).highlighted_hover(cx).is_empty()
    });
    let at_rest = lit(cx);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    let tool_on = lit(cx);
    click(cx, window, &view, (400., 500.));
    let mid_draw = (cx.update(|cx| view.read(cx).sketch.as_ref().is_some_and(|ed| ed.mid_draw())), lit(cx));
    assert_eq!((at_rest, tool_on, mid_draw), (true, true, (true, false)), "(lit with no tool, with the Line tool before its first click, (mid-draw, lit) after it)");
}

/// Live hover-matrix-1121 sk/inference-on-point-at, inference-mid-of-line-at: mid-draw what the snap lands on lights,
/// a corner itself, the line for its midpoint or a point on it.
#[gpui::test]
fn mid_draw_the_snapped_point_or_the_line_of_a_snapped_midpoint_lights(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "mid-draw-snap-lit", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let (line, a, b) = lines(cx, &view)[0];
    click(cx, window, &view, (380., 520.));
    let lit_at = |cx: &mut TestAppContext, at: DVec2| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            vp.update(cx, |vp, _| vp.place_pointer(at));
            v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
        cx.update(|cx| {
            let v = view.read(cx);
            let kind = v.sketch.as_ref().unwrap().inference.as_ref().map(|i| i.kind);
            (kind, v.highlighted_hover(cx))
        })
    };
    let sketch = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().feature);
    let (_, lit) = lit_at(cx, (a + b) * 0.5);
    assert_eq!(lit, vec![Reference::SketchEntity { sketch, entity: line }], "on the line's middle: the line lit");
    let (kind, lit) = lit_at(cx, a);
    let corner = matches!(lit.as_slice(), [Reference::SketchEntity { entity, .. }] if *entity != line);
    assert!(kind == Some(noshape_sketch::inference::InferenceKind::Endpoint) && corner, "on a corner: the corner lit ({kind:?}, {lit:?})");
}

/// Live2 sketch-snaps (interaction-feel "Inference glyph"): with the Line tool's first click down, a single snap's
/// 20 px glyph box is centred ≈ 30 px right of and 30 below the pointer (live2's ±2 px).
#[gpui::test]
fn a_snap_glyph_sits_30_px_right_of_and_below_the_pointer(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "snap-glyph-place", Tool::Line);
    // Level in the sketch's plane (the camera isn't normal to it): a horizontal snap.
    let wpp = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().world_per_px);
    let a = DVec2::new(-60.0, 40.0) * wpp;
    let p = on_screen(cx, &view, a);
    click(cx, window, &view, (p.x, p.y));
    let target = on_screen(cx, &view, a + DVec2::new(150.0, 0.0) * wpp);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        let vp = v.viewport.clone();
        vp.update(cx, |vp, _| vp.place_pointer(target));
        v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: target, modifiers: Modifiers::default() }, w, cx);
        v.on_viewport_event(&vp, &ViewportEvent::Hover(None), w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let icons = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().snap_icons());
    assert_eq!(icons, vec!["horizontal"], "one snap glyph by the pointer");
    let pointer = Pointer::new(cx, window, &view);
    let at = pointer.window_point(cx, target);
    let glyph = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("snap-glyphs").expect("the glyphs drawn");
    let off = (f32::from(glyph.center().x - at.x), f32::from(glyph.center().y - at.y));
    assert!((off.0 - 30.0).abs() <= 2.0 && (off.1 - 30.0).abs() <= 2.0, "the glyph box centred {off:?} from the pointer, live ≈ (30, 30)");
}
