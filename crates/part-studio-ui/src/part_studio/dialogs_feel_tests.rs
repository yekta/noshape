//! The dialogs matrix (docs/parity/interaction-feel.md "Dialogs"): live2's feel/dialogs.json rows (which features have
//! a manipulator, an arrow's grab reach, the drag steps, the Properties modal's keyboard) on the block start. A row ours
//! doesn't match yet is a gap; the test fails when a row's status moves either way.

use super::*;
use crate::part_studio::manipulators::{linear_step, snap_degrees};
use gpui_kit::component::WindowExt as _;

fn block(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, name, Some(starts["block"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    (window, view)
}

/// The block's top face picked and `feature` opened on it; whether a manipulator shows.
fn manipulator_for(cx: &mut TestAppContext, feature: &str) -> bool {
    let (window, view) = block(cx, &format!("dlg-{feature}"));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let b = v.display.bodies.first().cloned();
            v.selection = b.map(|b| b.face_refs.into_iter().take(1).collect()).unwrap_or_default();
            v.start_feature(feature, w, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    cx.update(|cx| view.read(cx).current_manip(cx).is_some())
}

/// Extrude 1 of the block opened for editing; its dialog.
fn edit_extrude(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>, Entity<crate::feature_dialog::FeatureDialog>) {
    let (window, view) = block(cx, name);
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Extrude dialog").dialog.clone());
    (window, view, dialog)
}

/// Fillet opened on the block with one edge picked into its field; the edge's pick id.
fn fillet_on_an_edge(cx: &mut TestAppContext, name: &str) -> Result<(AnyWindowHandle, Entity<PartStudioView>, Entity<crate::feature_dialog::FeatureDialog>, u32), String> {
    let (window, view) = block(cx, name);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
    draw(cx, window);
    let (edge, pick) = cx.update(|cx| {
        let v = view.read(cx);
        let e = v.display.bodies.first().and_then(|b| b.edge_refs.first().cloned());
        let p = e.as_ref().and_then(|e| v.built.as_ref().and_then(|b| b.picks.ids_for(e).first().copied()));
        (e, p)
    });
    let (edge, pick) = (edge.ok_or("no edge")?, pick.ok_or("no pick")?);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        let vp = v.viewport.clone();
        v.on_viewport_event(&vp, &ViewportEvent::Click { pick: Some(pick), position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() }, w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    if !cx.update(|cx| dialog.read(cx).feature.params.query("entities").contains(&edge)) {
        return Err("the edge didn't go into the field".into());
    }
    Ok((window, view, dialog, pick))
}

fn expect<T: PartialEq + std::fmt::Debug>(got: T, want: T, what: &str) -> Result<(), String> {
    if got == want { Ok(()) } else { Err(format!("{what}: ours {got:?}, live {want:?}")) }
}

type Check = fn(&mut TestAppContext) -> Result<(), String>;

struct Row {
    id: &'static str,
    live: &'static str,
    check: Check,
    gap: bool,
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "manipulators / which features", live: "Extrude: one arrow; Chamfer, Shell: none", gap: false, check: |cx| {
            expect((manipulator_for(cx, "extrude"), manipulator_for(cx, "chamfer"), manipulator_for(cx, "shell")), (true, false, false), "extrude, chamfer, shell")
        } },
        Row { id: "manipulators / an arrow's grab reach", live: "within 14 px across (16 no), from 4 px behind the base to the tip; nothing past the tip", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-reach");
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let b = v.display.bodies.first().cloned();
                    v.selection = b.map(|b| b.face_refs.into_iter().take(1).collect()).unwrap_or_default();
                    v.start_feature("extrude", w, cx);
                })
            })
            .unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let (a, b) = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, glyph) = vp.manipulators.first()?;
                    let (a, b) = (vp.project(a)?, vp.project(b)?);
                    Some((a, if glyph > 0.0 { a + (b - a).normalize_or_zero() * glyph } else { b }))
                })
                .ok_or("no arrow")?;
            let (dir, len) = ((b - a).normalize(), (b - a).length());
            let n = dir.perp();
            let hit = |cx: &mut TestAppContext, p: glam::DVec2| cx.update(|cx| vp.read(cx).manipulator_index_at(p).is_some());
            let got = [
                hit(cx, a + dir * (len * 0.5) + n * 13.5),
                hit(cx, a + dir * (len * 0.5) + n * 16.0),
                hit(cx, a - dir * 3.5),
                hit(cx, a - dir * 6.0),
                hit(cx, b + dir * 3.0),
            ];
            expect(got, [true, false, true, false, false], "13.5 across, 16 across, 3.5 behind, 6 behind, 3 past the tip")
        } },
        Row { id: "manipulators / a click on Extrude's arrow flips it", live: "a plain click flips the direction (offset 27 -> -27, the field still 27 mm) and regenerates", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-arrow-click");
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let at = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, _) = vp.manipulators.first()?;
                    let mid = (vp.project(a)? + vp.project(b)?) * 0.5;
                    let o = vp.origin();
                    Some(gpui_kit::point(o.x + px(mid.x as f32), o.y + px(mid.y as f32)))
                })
                .ok_or("no arrow")?;
            let state = |cx: &mut TestAppContext| cx.update(|cx| {
                let f = &dialog.read(cx).feature;
                (f.params.bool("oppositeDirection"), f.params.expr("depth").map(|e| e.text.clone()))
            });
            let before = state(cx);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
            vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
            vcx.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1 });
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let after = state(cx);
            expect((after.0 != before.0, after.1 == before.1, cx.update(|cx| view.read(cx).edit.is_some())), (true, true, true), "flipped, depth kept, dialog open")
        } },
        Row { id: "manipulators / typing during the arrow's drag wins over later moves", live: "live2 feature-dialogs-1119 m1: \"40\" typed with the button held replaces Depth; further moves leave it; the release keeps 40", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-arrow-type");
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let (at, along) = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, _) = vp.manipulators.first()?;
                    let (a, b) = (vp.project(a)?, vp.project(b)?);
                    let mid = (a + b) * 0.5;
                    let o = vp.origin();
                    Some((gpui_kit::point(o.x + px(mid.x as f32), o.y + px(mid.y as f32)), (b - a).normalize_or_zero()))
                })
                .ok_or("no arrow")?;
            let to = |k: f64| gpui_kit::point(at.x + px((along.x * 8.0 * k) as f32), at.y + px((along.y * 8.0 * k) as f32));
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
            vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
            for k in 1..=3 {
                vcx.simulate_event(gpui_kit::MouseMoveEvent { position: to(k as f64), pressed_button: Some(gpui_kit::MouseButton::Left), modifiers: Modifiers::default() });
            }
            draw(cx, window);
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("4 0");
            draw(cx, window);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            for k in 4..=6 {
                vcx.simulate_event(gpui_kit::MouseMoveEvent { position: to(k as f64), pressed_button: Some(gpui_kit::MouseButton::Left), modifiers: Modifiers::default() });
            }
            vcx.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: to(6.0), modifiers: Modifiers::default(), click_count: 1 });
            draw(cx, window);
            expect(cx.update(|cx| dialog.read(cx).input_text("depth", cx)).map(|(t, _)| t), Some("40".to_string()), "Depth after typing 40 mid-drag, moving on and releasing")
        } },
        Row { id: "manipulators / Esc with the button held cancels the dialog", live: "live manip-feel-1118 (5): Escape while dragging the depth arrow closes the whole dialog, the feature as it was", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-arrow-esc");
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let (at, along) = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, _) = vp.manipulators.first()?;
                    let (a, b) = (vp.project(a)?, vp.project(b)?);
                    let mid = (a + b) * 0.5;
                    let o = vp.origin();
                    Some((gpui_kit::point(o.x + px(mid.x as f32), o.y + px(mid.y as f32)), (b - a).normalize_or_zero()))
                })
                .ok_or("no arrow")?;
            let depth = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.features.iter().find(|f| f.name == "Extrude 1").and_then(|f| f.params.expr("depth").map(|e| e.text.clone()))));
            let before = depth(cx);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
            vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
            for k in 1..=6 {
                let p = gpui_kit::point(at.x + px((along.x * 8.0 * k as f64) as f32), at.y + px((along.y * 8.0 * k as f64) as f32));
                vcx.simulate_event(gpui_kit::MouseMoveEvent { position: p, pressed_button: Some(gpui_kit::MouseButton::Left), modifiers: Modifiers::default() });
            }
            draw(cx, window);
            let dragged = cx.update(|cx| dialog.read(cx).feature.params.expr("depth").map(|e| e.text.clone())) != before;
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
            draw(cx, window);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let open = cx.update(|cx| view.read(cx).edit.is_some());
            expect((dragged, open, depth(cx) == before), (true, false, true), "(the drag changed Depth, the dialog open after Esc, Extrude 1 as before)")
        } },
        Row { id: "manipulators / Fillet's handle drags unstepped", live: "live manip-1118b: from R5, 5 px moves give 23.41787, 28.97056, 41.52447 mm (not the arrows' 0.1 mm steps); toward the edge no less than 0.02414 mm", gap: false, check: |cx| {
            let (window, view, dialog, _) = fillet_on_an_edge(cx, "dlg-fillet-drag")?;
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let (at, along) = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, _) = vp.manipulators.first()?;
                    let (a, b) = (vp.project(a)?, vp.project(b)?);
                    let o = vp.origin();
                    Some((gpui_kit::point(o.x + px(a.x as f32), o.y + px(a.y as f32)), (b - a).normalize_or_zero()))
                })
                .ok_or("no fillet handle")?;
            let radius = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).input_text("radius", cx)).map(|(t, _)| t).unwrap_or_default();
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
            vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
            let mut seen = Vec::new();
            for k in 1..=4 {
                let p = gpui_kit::point(at.x + px((along.x * 7.0 * k as f64) as f32), at.y + px((along.y * 7.0 * k as f64) as f32));
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: p, pressed_button: Some(gpui_kit::MouseButton::Left), modifiers: Modifiers::default() });
                draw(cx, window);
                seen.push(radius(cx));
            }
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1 });
            draw(cx, window);
            let mm = |t: &String| t.trim_end_matches(" mm").parse::<f64>().ok();
            let off_grid = seen.iter().filter_map(mm).filter(|v| ((v * 10.0) - (v * 10.0).round()).abs() > 1e-6).count();
            expect(off_grid > 0, true, &format!("a radius off the 0.1 mm grid while dragging: {seen:?}"))
        } },
        Row { id: "manipulators / an arrow drag lights nothing; Depth has the keyboard, its text selected", live: "live manip-feel-1118 (1, 2): nothing lights under the pointer while the arrow is dragged; Depth focused with its whole text selected, rewritten each update, and so after the release", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-arrow-keys");
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let (at, along) = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, _) = vp.manipulators.first()?;
                    let (a, b) = (vp.project(a)?, vp.project(b)?);
                    let mid = (a + b) * 0.5;
                    let o = vp.origin();
                    Some((gpui_kit::point(o.x + px(mid.x as f32), o.y + px(mid.y as f32)), (b - a).normalize_or_zero()))
                })
                .ok_or("no arrow")?;
            let state = |cx: &mut TestAppContext| {
                let focused = cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap();
                let (text, selected) = cx.update(|cx| dialog.read(cx).input_text("depth", cx)).unwrap_or_default();
                (focused, !text.is_empty() && selected == text, cx.update(|cx| view.read(cx).hover.clone()))
            };
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
            vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
            let mut during = Vec::new();
            let mut last = at;
            for k in 1..=6 {
                last = gpui_kit::point(at.x + px((along.x * 10.0 * k as f64) as f32), at.y + px((along.y * 10.0 * k as f64) as f32));
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: last, pressed_button: Some(gpui_kit::MouseButton::Left), modifiers: Modifiers::default() });
                draw(cx, window);
                during.push(state(cx));
            }
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: last, modifiers: Modifiers::default(), click_count: 1 });
            draw(cx, window);
            let after = state(cx);
            let want = (Some("in:depth".to_string()), true, None);
            expect((during, after), (vec![want.clone(); 6], want), "(focused, Depth's whole text selected, lit) at each drag move, after the release")
        } },
        Row { id: "drag / the length step follows the zoom", live: "10^(round(log10(100 px in the unit)) − 2): 0.1 mm at 0.25 mm/px, 1 mm at 0.414", gap: false, check: |_| {
            let mm = 0.001;
            expect(((linear_step(0.25 * mm, mm) / mm * 1e6).round() / 1e6, (linear_step(0.414 * mm, mm) / mm * 1e6).round() / 1e6), (0.1, 1.0), "steps (mm)")
        } },
        Row { id: "drag / the angle step", live: "5° within 2° (radius ≤ 573 px), free in between: 42.4154 stays, 43.5 → 45", gap: false, check: |_| {
            expect((snap_degrees(42.4154, 300.), snap_degrees(43.5, 300.)), (42.4154, 45.0), "angles")
        } },
        Row { id: "fields / a selected entity clicked again leaves the field", live: "[Face of Extrude 2] -> [] -> [Face of Extrude 2]", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-toggle");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
            draw(cx, window);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let edge = cx.update(|cx| view.read(cx).display.bodies.first().and_then(|b| b.edge_refs.first().cloned())).ok_or("no edge")?;
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().unwrap().dialog.clone());
            let mut counts = Vec::new();
            for _ in 0..3 {
                // Each click on the edge's pick as the scene has it once the last click's preview is built (its picks
                // are rebuilt with it).
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                    let pick = v.built.as_ref().and_then(|b| b.picks.ids_for(&edge).first().copied());
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::Click { pick, position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() }, w, cx);
                }))
                .unwrap();
                wait_until(cx, window, |cx| !view.read(cx).ui_busy());
                draw(cx, window);
                counts.push(cx.update(|cx| dialog.read(cx).feature.params.query("entities").iter().filter(|r| **r == edge).count()));
            }
            expect(counts, vec![1, 0, 1], "the edge in the field after each click")
        } },
        Row { id: "fields / a row hovered lights its entity", live: "the selected orange outline turns light yellow-green (#f2ff80) while the row is hovered; clears when the pointer leaves", gap: false, check: |cx| {
            let (window, view, dialog, _) = fillet_on_an_edge(cx, "dlg-row-hover")?;
            let edge = cx.update(|cx| dialog.read(cx).feature.params.query("entities").first().cloned()).ok_or("no edge")?;
            // The preview consumes the edge: it is drawn by the overlay of consumed picks, else by its pick's flag.
            let lit = |cx: &mut TestAppContext| cx.update(|cx| {
                let v = view.read(cx);
                let flagged = v.built.as_ref().is_some_and(|b| b.picks.ids_for(&edge).iter().any(|&id| v.viewport.read(cx).scene().states.get(id as usize).is_some_and(|s| s & noshape_render::scene::state::QUERY != 0)));
                flagged || v.ui.consumed_lines.as_ref().is_some_and(|c| c.2.contains(&edge))
            });
            let path = cx.update(|cx| dialog.read(cx).active_query.clone()).ok_or("no active field")?;
            let row = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("chip-{path}-0").into_boxed_str())).ok_or("no row")?;
            let before = lit(cx);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: row.center(), pressed_button: None, modifiers: Modifiers::default() });
            draw(cx, window);
            let on = lit(cx);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: row.center() + gpui_kit::point(px(0.), px(200.)), pressed_button: None, modifiers: Modifiers::default() });
            draw(cx, window);
            expect((before, on, lit(cx)), (false, true, false), "lit before, on the row, after leaving")
        } },
        Row { id: "fields / a row click leaves the selection as is", live: "clicking a row (not its x) keeps the field's entities", gap: false, check: |cx| {
            let (window, _view, dialog, _) = fillet_on_an_edge(cx, "dlg-row-click")?;
            let path = cx.update(|cx| dialog.read(cx).active_query.clone()).ok_or("no active field")?;
            let row = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("chip-{path}-0").into_boxed_str())).ok_or("no row")?;
            let at = row.origin + gpui_kit::point(px(20.), row.size.height / 2.);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_click(at, Modifiers::default());
            draw(cx, window);
            expect(cx.update(|cx| dialog.read(cx).feature.params.query("entities").len()), 1, "entities after the click")
        } },
        Row { id: "fields / a chip's x: its tooltip, a click removes the chip", live: "the x hovered: tooltip \"Delete Edge of Extrude 1\"; a click removes that chip (live2 ours-1118, qa CHIP-X-TOOLTIP)", gap: false, check: |cx| {
            let (window, _view, dialog, _) = fillet_on_an_edge(cx, "dlg-chip-x")?;
            let path = cx.update(|cx| dialog.read(cx).active_query.clone()).ok_or("no active field")?;
            let x = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("chipx-{path}-0").into_boxed_str())).ok_or("no x")?;
            crate::ui_script::clear_tooltip();
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(x.center(), None, Modifiers::default());
            for _ in 0..50 {
                cx.executor().advance_clock(std::time::Duration::from_millis(10));
                draw(cx, window);
            }
            let tip = crate::ui_script::shown_tooltip().unwrap_or_default();
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(x.center(), Modifiers::default());
            draw(cx, window);
            let left = cx.update(|cx| dialog.read(cx).feature.params.query("entities").len());
            expect((tip.starts_with("Delete Edge of "), left), (true, 0), &format!("the x's tooltip {tip:?}, entities after its click"))
        } },
        Row { id: "fields / a focused length field's step keys", live: "live2 field-keys-1118 (Extrude Depth 10): Up 11, Down 10, Ctrl+Up 10.1, Shift+Up 20.1, Alt+Up 21.1 (as plain), PageUp nothing; the text selected throughout", gap: false, check: |cx| {
            let (window, _view, dialog) = edit_extrude(cx, "dlg-step-keys");
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            let value = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).input_text("depth", cx)).and_then(|(t, sel)| Some((t.split_whitespace().next()?.parse::<f64>().ok()?, sel == t)));
            let (start, _) = value(cx).ok_or("no Depth")?;
            let mut got = Vec::new();
            for k in ["up", "down", "ctrl-up", "shift-up", "alt-up", "pageup"] {
                keys(cx, window, &[k]);
                let (v, selected) = value(cx).ok_or("no Depth")?;
                got.push(((v - start) * 10.0).round() / 10.0);
                if !selected {
                    return Err(format!("after {k} the text isn't selected"));
                }
            }
            expect(got, vec![1.0, 0.0, 0.1, 10.1, 11.1, 11.1], "Depth's steps from the start (Up, Down, Ctrl+Up, Shift+Up, Alt+Up, PageUp)")
        } },
        Row { id: "fields / Tab from Radius", live: "Radius's ± button, then the checkboxes in order; Tab skips the selection fields and the dropdowns", gap: false, check: |cx| {
            let (window, _view, dialog, _) = fillet_on_an_edge(cx, "dlg-tab")?;
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("radius", w, cx))).unwrap();
            draw(cx, window);
            let mut order = Vec::new();
            for _ in 0..6 {
                keys(cx, window, &["tab"]);
                order.push(cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap().unwrap_or_default());
            }
            let want = ["tol:radius", "cb:isAsymmetric", "cb:isPartial", "cb:isVariable", "cb:allowEdgeOverflow", "cb:smoothCorners"];
            expect(order, want.map(String::from).to_vec(), "focus after each Tab")
        } },
        Row { id: "fields / Tab with the unit list open, then ✓", live: "typing '5 mm' opens the list (mm / max / min); Tab takes the entry and applies the field, the keyboard stays; ✓ then commits (behaviours: 'Tab, then ✓ commits')", gap: false, check: |cx| {
            let (window, view, dialog, _) = fillet_on_an_edge(cx, "dlg-tab-list")?;
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("radius", w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["6", "space", "m", "m"]);
            let listed = cx.update(|cx| dialog.read(cx).ui_list().is_some());
            keys(cx, window, &["tab"]);
            draw(cx, window);
            let after = cx.update_window(window, |_, w, cx| (dialog.read(cx).ui_list().is_some(), dialog.read(cx).focused_control(w, cx))).unwrap();
            let ok = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("dialog-ok").ok_or("no ✓")?.center();
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(ok, Modifiers::default());
            draw(cx, window);
            wait_until(cx, window, |cx| view.read(cx).edit.is_none());
            let closed = cx.update(|cx| view.read(cx).edit.is_none());
            expect((listed, after, closed), (true, (false, Some("in:radius".to_string())), true), "the list while typing; (the list, the keyboard) after Tab; the dialog closed by one ✓")
        } },
        Row { id: "preview / not while typing; Tab sends it", live: "no preview request while typing (none for 3 s a key); Enter, Tab or leaving the field sends it", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-typing");
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let height = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.bounds().map(|(lo, hi)| ((hi.z - lo.z) * 1000.).round()));
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["4", "0"]);
            for _ in 0..150 {
                cx.executor().advance_clock(std::time::Duration::from_millis(20));
                draw(cx, window);
            }
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let typed = height(cx);
            keys(cx, window, &["tab"]);
            for _ in 0..25 {
                cx.executor().advance_clock(std::time::Duration::from_millis(20));
                draw(cx, window);
            }
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            expect((typed, height(cx)), (Some(10.), Some(40.)), "the body's height (mm) 3 s after typing 40, after Tab")
        } },
        Row { id: "fields / Esc in a value field", live: "the first Esc discards the typed value and leaves the field, the dialog stays; a second Esc cancels", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-esc");
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["3", "escape"]);
            let after_one = (cx.update(|cx| view.read(cx).edit.is_some()), cx.update(|cx| dialog.read(cx).feature.params.expr("depth").map(|e| e.text.clone())));
            keys(cx, window, &["escape"]);
            let after_two = cx.update(|cx| view.read(cx).edit.is_some());
            expect((after_one, after_two), ((true, Some("10 mm".to_string())), false), "(open, depth) after one Esc, open after two")
        } },
        Row { id: "fields / Enter with the keyboard outside a field commits", live: "Enter with the focus outside a field commits the dialog", gap: false, check: |cx| {
            let (window, view, _dialog) = edit_extrude(cx, "dlg-enter");
            cx.update_window(window, |_, w, cx| {
                let focus = view.read(cx).focus.clone();
                w.focus(&focus, cx);
            })
            .unwrap();
            keys(cx, window, &["enter"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            expect(cx.update(|cx| view.read(cx).edit.is_some()), false, "dialog open")
        } },
        Row { id: "Derived / Select Part Studio panel: Esc and Enter", live: "with the panel open, the first Esc closes only the panel, the second cancels; the first Enter closes only the panel (live docs-import-feel-1118)", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-derived-keys");
            let panel_open = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).ref_picker.is_some()));
            let open_panel = |cx: &mut TestAppContext| -> Result<(), String> {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("importDerived", w, cx))).unwrap();
                wait_until(cx, window, |cx| !view.read(cx).ui_busy());
                draw(cx, window);
                let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
                let field = vcx.debug_bounds("ref-partStudio").ok_or("no Part Studio field")?;
                vcx.simulate_click(field.center(), Modifiers::default());
                draw(cx, window);
                Ok(())
            };
            open_panel(cx)?;
            let opened = panel_open(cx);
            keys(cx, window, &["escape"]);
            let after_one = panel_open(cx);
            keys(cx, window, &["escape"]);
            let after_two = panel_open(cx);
            open_panel(cx)?;
            keys(cx, window, &["enter"]);
            let after_enter = panel_open(cx);
            expect((opened, after_one, after_two, after_enter), (Some(true), Some(false), None, Some(false)), "panel open, after Esc, after a second Esc (None: no dialog), after Enter")
        } },
        Row { id: "Properties / Enter and Esc do nothing", live: "Enter in a text field neither applies nor closes; Esc doesn't close (clean or changed)", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-props-keys");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.open_part_meta(b, w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let form = crate::properties_dialog::open_form().ok_or("no modal")?;
            cx.update_window(window, |_, w, cx| form.update(cx, |f, cx| f.focus_field("Vendor", w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["v", "enter"]);
            let open_after_enter = crate::properties_dialog::open_form().is_some() && cx.update_window(window, |_, w, cx| w.has_active_dialog(cx)).unwrap();
            keys(cx, window, &["escape"]);
            let open_after_esc = cx.update_window(window, |_, w, cx| w.has_active_dialog(cx)).unwrap();
            expect((open_after_enter, open_after_esc), (true, true), "open after Enter, after Esc")
        } },
        Row { id: "Properties / Tab order", live: "Name, Description, Part number, Revision, Vendor, Project, Product line, Title 1-3, Not revision managed, Exclude from all BOMs (selects and read-only fields skipped)", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-props-tab");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.open_part_meta(b, w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let form = crate::properties_dialog::open_form().ok_or("no modal")?;
            cx.update_window(window, |_, w, cx| form.update(cx, |f, cx| f.focus_field("Name", w, cx))).unwrap();
            draw(cx, window);
            let mut order = Vec::new();
            for _ in 0..11 {
                keys(cx, window, &["tab"]);
                order.push(cx.update_window(window, |_, w, cx| form.read(cx).focused_field(w, cx)).unwrap().unwrap_or("?"));
            }
            cx.update_window(window, |_, w, cx| w.close_dialog(cx)).unwrap();
            expect(order, vec!["Description", "Part number", "Revision", "Vendor", "Project", "Product line", "Title 1", "Title 2", "Title 3", "Not revision managed", "Exclude from all BOMs"], "the field after each Tab from Name")
        } },
        Row { id: "Appearance / its panel on opening", live: "220 × 400 at the graphics area's bottom left; the hex field focused, its text selected, uppercase without '#'; A with two decimals (1.00)", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-appearance");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.edit_appearance(crate::part_studio::appearance::EditTarget::Parts(vec![b]), w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            let panel = vcx.debug_bounds("appearance-editor").ok_or("no panel")?;
            let area = cx.update(|cx| { let vp = view.read(cx).viewport.read(cx); (vp.origin(), vp.size()) });
            let (hex, alpha, focused, selected) = cx.update_window(window, |_, w, cx| {
                let e = view.read(cx).ui.appearance_edit.as_ref().unwrap();
                let h = e.hex.read(cx);
                (h.value().to_string(), e.alpha.read(cx).value().to_string(), h.focus_handle(cx).is_focused(w), h.selected_range().len() == h.value().len())
            })
            .unwrap();
            let left = (f32::from(panel.origin.x) - f32::from(area.0.x)).round();
            let bottom = (f32::from(area.0.y) + area.1.y as f32 - f32::from(panel.bottom())).round();
            let size = (f32::from(panel.size.width).round(), f32::from(panel.size.height).round());
            expect(
                (size, (left, bottom), hex.len() == 6 && hex == hex.to_uppercase() && !hex.starts_with('#'), alpha.as_str(), focused, selected),
                ((220., 400.), (0., 0.), true, "1.00", true, true),
                "size, (left, bottom) in the graphics, hex uppercase without #, A, hex focused, its text selected",
            )
        } },
        Row { id: "Material / its panel on opening", live: "280 × 368 at the graphics area's bottom; 'Material' with ✓ / ✗, tabs Library | Custom, 'Onshape Material Library', '- None -', 'No material assigned'", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-material-panel");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.open_material_panel(vec![b], w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let panel = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("material-panel").ok_or("no panel")?;
            let area = cx.update(|cx| { let vp = view.read(cx).viewport.read(cx); (vp.origin(), vp.size()) });
            let bottom = (f32::from(area.0.y) + area.1.y as f32 - f32::from(panel.bottom())).round();
            let picked = cx.update(|cx| view.read(cx).ui.material_panel.as_ref().and_then(|m| m.picked_name()));
            expect(((f32::from(panel.size.width).round(), f32::from(panel.size.height).round()), bottom, picked), ((280., 368.), 0., None), "size, bottom in the graphics, the material picked")
        } },
        Row { id: "Material / the search and Esc", live: "the list filters as you type ('steel' left 38 rows: live's read counted the highlighted first row twice), the first highlighted; Esc closes the list only, a second Esc the dialog", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-material");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.open_material_panel(vec![b], w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let at = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mat-picker").ok_or("no picker")?.center();
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::default());
            draw(cx, window);
            keys(cx, window, &["s", "t", "e", "e", "l"]);
            let (names, active) = cx.update(|cx| (view.read(cx).material_row_names(cx), view.read(cx).ui.material_panel.as_ref().map(|m| m.active)));
            let rows = names.len();
            keys(cx, window, &["escape"]);
            let after_one = cx.update(|cx| view.read(cx).ui.material_panel.as_ref().map(|m| m.is_list_open()));
            keys(cx, window, &["escape"]);
            let after_two = cx.update(|cx| view.read(cx).ui.material_panel.is_some());
            expect((rows, active, after_one, after_two), (38, Some(0), Some(false), false), "rows for 'steel', the active row, the list after one Esc (panel open), the panel after two")
        } },
        Row { id: "Material / a pick keeps the keyboard in the search", live: "material-search-acetal-ok, material-library-pick-abs-cancel: after Enter (or a click) on a row the list closes, the search input keeps the keyboard, its text empty; typing opens the list again", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-material-pick");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.open_material_panel(vec![b], w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let at = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mat-picker").ok_or("no picker")?.center();
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::default());
            draw(cx, window);
            keys(cx, window, &["a", "c", "e", "t", "a", "l", "enter"]);
            draw(cx, window);
            let state = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| {
                let m = view.read(cx).ui.material_panel.as_ref().unwrap();
                let s = m.search.read(cx);
                (m.picked_name(), m.is_list_open(), s.focus_handle(cx).is_focused(w), s.value().to_string())
            })
            .unwrap();
            let picked = state(cx);
            keys(cx, window, &["s"]);
            draw(cx, window);
            let typed = state(cx).1;
            expect((picked, typed), ((Some("Acetal (Delrin)".to_string()), false, true, String::new()), true), "(the material, the list open, the search focused, its text) after Enter; the list after typing")
        } },
        Row { id: "Material / Down and Enter pick the next match, no preview, Esc discards", live: "'alum': Down moves from Alumina Oxide to Aluminum, Enter picks it; the part's look stays; Esc closes the dialog unapplied (live2 parts-list-1118 f2-f4)", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-material-down");
            let color = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.bodies.first().map(|b| b.color));
            let before = color(cx);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.open_material_panel(vec![b], w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let at = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("mat-picker").ok_or("no picker")?.center();
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::default());
            draw(cx, window);
            keys(cx, window, &["a", "l", "u", "m"]);
            let first = cx.update(|cx| view.read(cx).material_row_names(cx).first().cloned());
            keys(cx, window, &["down", "enter"]);
            draw(cx, window);
            let picked = cx.update(|cx| view.read(cx).ui.material_panel.as_ref().and_then(|m| m.picked_name()));
            let look = color(cx) == before;
            keys(cx, window, &["escape"]);
            let after = cx.update(|cx| {
                let v = view.read(cx);
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                (v.ui.material_panel.is_some(), v.studio(cx).and_then(|s| s.part_properties.get(&b)).and_then(|p| p.material.clone()))
            });
            expect((first, picked, look, after), (Some("Alumina Oxide".to_string()), Some("Aluminum".to_string()), true, (false, None)), "(first match, picked, the part's look kept, (panel open, material) after Esc)")
        } },
        Row { id: "Appearance / a face's: a feature-style dialog that recolours the face at once, no feature row", live: "Add appearance to face… opens 'Face appearance 1', 220 x 401 in the feature dialogs' place, chip 'Face of Extrude 1'; a swatch recolours only that face at once; ✓ keeps it; the feature list stays as it was (live2 parts-list-1118 e1-e4)", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-face-appearance");
            let features = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).map(|s| s.features.len()));
            let before = features(cx);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let face = v.display.bodies[0].face_refs[0].clone();
                v.selection = vec![face.clone()];
                v.edit_appearance(crate::part_studio::appearance::EditTarget::Faces(vec![face], None), w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let panel = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("appearance-editor").ok_or("no dialog")?;
            let origin = cx.update(|cx| view.read(cx).viewport.read(cx).origin());
            let place = (panel.origin - origin) == crate::part_studio::behaviors::feature_dialog_pos();
            let size = (f32::from(panel.size.width).round(), f32::from(panel.size.height).round());
            let state = cx.update(|cx| view.read(cx).appearance_dialog_state(cx)).ok_or("no state")?;
            let (title, chips) = (state["title"].as_str().map(str::to_string), state["fields"][0]["items"].clone());
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.set_edit_color([255, 0, 0], w, cx))).unwrap();
            draw(cx, window);
            // The first face's vertices red, another face's left to the part colour (alpha 0).
            let colors = cx.update(|cx| view.update(cx, |v, cx| {
                let b = &v.display.bodies[0];
                let (first, other) = (b.mesh.face_ranges[0].0 as usize, b.mesh.face_ranges[1].0 as usize);
                let (i0, i1) = (b.mesh.indices[first] as usize, b.mesh.indices[other] as usize);
                let id = b.id.clone();
                v.vertex_colors(cx).and_then(|m| m.get(&id).map(|c| (c[i0], c[i1][3])))
            }));
            cx.update(|cx| view.update(cx, |v, cx| v.accept_appearance(cx)));
            draw(cx, window);
            let after = (features(cx), cx.update(|cx| view.read(cx).studio(cx).map(|s| s.appearances.len())), cx.update(|cx| view.read(cx).ui.appearance_edit.is_some()));
            expect(
                (place, size, title, chips, colors, after),
                (true, (220., 401.), Some("Face appearance 1".to_string()), serde_json::json!(["Face of Extrude 1"]), Some(([255, 0, 0, 255], 0)), (before, Some(1), false)),
                "(in the feature dialogs' place, size, title, chips, (the face's colour, another face's alpha) after a red swatch, (features, appearances, open) after ✓)",
            )
        } },
        Row { id: "keys / on opening the first selection field has the keyboard", live: "a new Extrude, a new Fillet and Extrude 1 opened for edit: 'Faces and sketch regions to extrude' / 'Entities to fillet' focused, no number field (live2 dialog-keys-1118)", gap: false, check: |cx| {
            let mut got = Vec::new();
            for feature in ["extrude", "fillet"] {
                let (window, view) = block(cx, &format!("dlg-open-focus-{feature}"));
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature(feature, w, cx))).unwrap();
                draw(cx, window);
                let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
                got.push(cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap());
            }
            let (window, _view, dialog) = edit_extrude(cx, "dlg-open-focus-edit");
            got.push(cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap());
            expect(got, vec![Some("q:entities".to_string()); 3], "the focused control: new Extrude, new Fillet, Extrude 1 edited")
        } },
        Row { id: "keys / Tab from Depth, then out of the dialog; Shift+Tab back", live: "Depth -> its ± -> Direction, Starting offset, Symmetric, Draft, Second end position -> the help link -> out of the dialog; End type, the selection field, the tabs and ✓ / ✗ skipped; Shift+Tab walks it back (live2 dialog-keys-1118 b1)", gap: false, check: |cx| {
            let (window, _view, dialog) = edit_extrude(cx, "dlg-tab-depth");
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            let focused = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap();
            let mut forward = Vec::new();
            for _ in 0..8 {
                keys(cx, window, &["tab"]);
                forward.push(focused(cx));
            }
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            for _ in 0..7 {
                keys(cx, window, &["tab"]);
            }
            let mut back = Vec::new();
            for _ in 0..7 {
                keys(cx, window, &["shift-tab"]);
                back.push(focused(cx));
            }
            let stops = ["tol:depth", "cb:hasExtrudeDirection", "cb:startOffset", "cb:symmetric", "cb:hasDraft", "cb:hasSecondDirection", "help"];
            let want_forward: Vec<Option<String>> = stops.iter().map(|s| Some(s.to_string())).chain([None]).collect();
            let want_back: Vec<Option<String>> = stops.iter().rev().skip(1).map(|s| Some(s.to_string())).chain([Some("in:depth".to_string())]).collect();
            expect((forward, back), (want_forward, want_back), "(the focus after each Tab from Depth, after each Shift+Tab from the help link)")
        } },
        Row { id: "keys / Tab orders of Chamfer, Shell, Hole, Revolve, Linear and Circular pattern", live: "from the first number field, to out of the dialog: Chamfer Distance -> ± -> Tangent propagation -> help; Shell thickness -> its flip arrow -> help; Hole's size -> ± -> Start plane -> Termination (its dropdowns are stops) -> flip -> depth -> ± -> tip angle -> help; Revolve (Full revolve off) angle -> ± -> Symmetric -> Second end position -> help; Linear Distance -> Instance count -> flip -> Centered -> Second direction -> Skip instances -> help; Circular Angle -> Instance count -> flip -> Equal spacing -> Centered -> Skip instances -> help (live dialogs-1118 batches 1-3)", gap: false, check: |cx| {
            // (Feature, checkboxes set first, the field Tab starts from, the stops after it.)
            type Case = (&'static str, &'static [(&'static str, bool)], &'static str, &'static [&'static str]);
            let cases: [Case; 6] = [
                ("chamfer", &[], "width", &["tol:width", "cb:tangentPropagation"]),
                ("shell", &[], "thickness", &["flip:oppositeDirection"]),
                ("hole", &[], "holeDiameterV3", &["tol:holeDiameterV3", "sel:startStyle", "sel:endStyleV2", "flip:oppositeDirection", "in:holeDepthV3", "tol:holeDepthV3", "sel:tipAngleStyle"]),
                ("revolve", &[("fullRevolve", false)], "angle", &["tol:angle", "cb:symmetric", "cb:hasStartBound"]),
                ("linearPattern", &[], "distance", &["in:instanceCount", "flip:oppositeDirection", "cb:isCentered", "cb:hasSecondDir", "cb:skipInstances"]),
                ("circularPattern", &[], "angle", &["in:instanceCount", "flip:oppositeDirection", "cb:equalSpace", "cb:isCentered", "cb:skipInstances"]),
            ];
            let (mut got, mut want) = (Vec::new(), Vec::new());
            for (feature, bools, field, stops) in cases {
                let (window, view) = block(cx, &format!("dlg-tab-{feature}"));
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature(feature, w, cx))).unwrap();
                draw(cx, window);
                let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
                cx.update(|cx| dialog.update(cx, |d, cx| bools.iter().for_each(|(p, b)| d.set_value(p, noshape_model::params::ParamValue::Bool(*b), cx))));
                draw(cx, window);
                cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected(field, w, cx))).unwrap();
                draw(cx, window);
                let mut walk = Vec::new();
                for _ in 0..stops.len() + 2 {
                    keys(cx, window, &["tab"]);
                    walk.push(cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap().unwrap_or_else(|| "out".into()));
                }
                got.push((feature, walk));
                want.push((feature, stops.iter().map(|s| s.to_string()).chain(["help".into(), "out".into()]).collect::<Vec<String>>()));
            }
            expect(got, want, "(feature, the focus after each Tab from its first number field)")
        } },
        Row { id: "keys / Esc after Tab has left the dialog does nothing", live: "Tab past the help link takes the keyboard out of the dialog (the page, then its header): Esc twice leaves the dialog open; a click on empty graphics, then Esc, cancels it (live dialogs-1118 batch 3)", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-esc-tab-out");
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            for _ in 0..8 {
                keys(cx, window, &["tab"]);
            }
            let out = cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap();
            keys(cx, window, &["escape"]);
            keys(cx, window, &["escape"]);
            let open = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).edit.is_some());
            let after_tab = open(cx);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::Click { pick: None, position: glam::DVec2::new(40., 40.), modifiers: Modifiers::default() }, w, cx);
            }))
            .unwrap();
            draw(cx, window);
            keys(cx, window, &["escape"]);
            expect((out, after_tab, open(cx)), (None, true, false), "(the focus after Tab past the help link, open after Esc ×2, open after a graphics click and Esc)")
        } },
        Row { id: "keys / Space toggles a focused checkbox", live: "Symmetric focused by Tab: Space on, Space off, the preview following (live2 dialog-keys-1118 c1)", gap: false, check: |cx| {
            let (window, _view, dialog) = edit_extrude(cx, "dlg-space");
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            for _ in 0..8 {
                if cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap().as_deref() == Some("cb:symmetric") {
                    break;
                }
                keys(cx, window, &["tab"]);
            }
            let symmetric = |cx: &mut TestAppContext| cx.update(|cx| matches!(dialog.read(cx).feature.params.get("symmetric"), Some(noshape_model::params::ParamValue::Bool(true))));
            // GPUI clicks a focused element on Space's release.
            let space = |cx: &mut TestAppContext| {
                keys(cx, window, &["space"]);
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::KeyUpEvent { keystroke: gpui_kit::Keystroke::parse("space").unwrap() });
                draw(cx, window);
            };
            space(cx);
            let on = symmetric(cx);
            space(cx);
            expect((on, symmetric(cx)), (true, false), "(Symmetric after one Space, after two)")
        } },
        Row { id: "keys / a dropdown: its search box focused, Down moves, Esc changes nothing", live: "End type clicked: the list with a focused search box; Down, Down: Blind -> Up to next -> Up to face highlighted; Esc closes it, Blind kept (live2 dialog-keys-1118 c2-c3)", gap: false, check: |cx| {
            let (window, _view, dialog) = edit_extrude(cx, "dlg-dropdown");
            let at = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("os-select:sel-endBound").ok_or("no End type")?.center();
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(at, Modifiers::default());
            draw(cx, window);
            let select = cx.update(|cx| dialog.read(cx).select("endBound")).ok_or("no select")?;
            let state = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| select.read(cx).open_state(w, cx).map(|(l, f)| (l.to_string(), f))).unwrap();
            let opened = state(cx);
            keys(cx, window, &["down", "down"]);
            let moved = state(cx);
            keys(cx, window, &["escape"]);
            let after = (state(cx), cx.update(|cx| dialog.read(cx).feature.params.enum_("endBound").to_string()));
            expect((opened, moved, after), (Some(("Blind".to_string(), true)), Some(("Up to face".to_string(), true)), (None, "BLIND".to_string())), "(highlighted, search focused) opened, after Down Down; (open, End type) after Esc")
        } },
        Row { id: "keys / Ctrl+Enter in a value field is Enter", live: "the dialog stays open with the value applied, as plain Enter (live2 dialog-keys-1118)", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-ctrl-enter");
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["1", "2", "ctrl-enter"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let depth = cx.update(|cx| dialog.read(cx).feature.params.expr("depth").map(|e| e.text.clone()));
            expect((cx.update(|cx| view.read(cx).edit.is_some()), depth), (true, Some("12 mm".to_string())), "(dialog open, Depth)")
        } },
        Row { id: "fields / an empty click changes nothing", live: "a click on empty background leaves the box's entries and the dialog as they are (live feature-dialogs-1117)", gap: false, check: |cx| {
            let (window, view, dialog, _) = fillet_on_an_edge(cx, "dlg-empty-click")?;
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::Click { pick: None, position: glam::DVec2::new(40., 40.), modifiers: Modifiers::default() }, w, cx);
            }))
            .unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let entries = cx.update(|cx| dialog.read(cx).feature.params.query("entities").len());
            expect((entries, cx.update(|cx| view.read(cx).edit.is_some())), (1, true), "entries, dialog open")
        } },
        Row { id: "fields / Radius focused, an edge click goes into the box", live: "with Radius's text focused the Entities box stays active: an edge click adds to it and takes the keyboard out of Radius (live feature-dialogs-1117)", gap: false, check: |cx| {
            let (window, view, dialog, _) = fillet_on_an_edge(cx, "dlg-radius-click")?;
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("radius", w, cx))).unwrap();
            draw(cx, window);
            let in_radius = cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap();
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let other = v.display.bodies.first().and_then(|b| b.edge_refs.get(3).cloned());
                let pick = other.and_then(|e| v.built.as_ref().and_then(|b| b.picks.ids_for(&e).first().copied()));
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::Click { pick, position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() }, w, cx);
            }))
            .unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let entries = cx.update(|cx| dialog.read(cx).feature.params.query("entities").len());
            let after = cx.update_window(window, |_, w, cx| dialog.read(cx).focused_control(w, cx)).unwrap();
            expect((entries, after == in_radius), (2, false), "entries, still in Radius")
        } },
        Row { id: "Boolean / Subtract: picks stay in Tools", live: "after switching to Subtract, Tools stays active: Part 2, then Part 1 both land in Tools; no auto-advance to Targets (live feature-dialogs-1117)", gap: false, check: |cx| {
            let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
            let (window, view) = part_studio_from(cx, "dlg-boolean-subtract", Some(starts["two-blocks"].clone()));
            wait_until(cx, window, |cx| view.read(cx).display.bodies.len() == 2 && !view.read(cx).ui_busy());
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("booleanBodies", w, cx))).unwrap();
            wait_until(cx, window, |cx| view.read(cx).edit.is_some() && !view.read(cx).ui_busy());
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().unwrap().dialog.clone());
            dialog.update(cx, |d, cx| d.set_value("operationType", noshape_model::ParamValue::Enum("SUBTRACTION".into()), cx));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            for i in [1, 0] {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                    let face = v.display.bodies.get(i).and_then(|b| b.face_refs.first().cloned());
                    let pick = face.and_then(|f| v.built.as_ref().and_then(|b| b.picks.ids_for(&f).first().copied()));
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::Click { pick, position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() }, w, cx);
                }))
                .unwrap();
                wait_until(cx, window, |cx| !view.read(cx).ui_busy());
                draw(cx, window);
            }
            let (tools, targets) = cx.update(|cx| {
                let d = dialog.read(cx);
                (d.feature.params.query("tools").len(), d.feature.params.query("targets").len())
            });
            expect((tools, targets), (2, 0), "Tools, Targets")
        } },
        Row { id: "Properties / no field has the keyboard on opening", live: "no field focused; typing does nothing until a field is clicked", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-props");
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                    v.open_part_meta(b, w, cx);
                })
            })
            .unwrap();
            draw(cx, window);
            let form = crate::properties_dialog::open_form().ok_or("no modal")?;
            let (field, own) = cx.update_window(window, |_, w, cx| (form.read(cx).a_field_has_focus(w, cx), form.read(cx).has_focus(w))).unwrap();
            expect((field, own), (false, true), "a field focused, the modal focused")
        } },
    ]
}

#[gpui::test]
fn the_dialogs_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let only = std::env::var("DIALOG_ROW").ok();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in rows.iter().filter(|r| only.as_ref().is_none_or(|o| r.id.contains(o.as_str()))) {
        let res = (r.check)(cx);
        let ok = res.is_ok();
        report.push(format!("{} {} (live: {}){}", if ok { "=" } else { "≠" }, r.id, r.live, res.err().map(|e| format!("\n    {e}")).unwrap_or_default()));
        if ok == r.gap {
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("dialogs matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}

/// Live2 manip-feel-1118 m6: Transform › Translate by XYZ's triad sits at the centre of the parts' bounding box (std's
/// transformCopy.fs findCenter, box3dCenter), not on the top face (ours was ~24 px too high on the block).
#[gpui::test]
fn the_translate_triad_sits_at_the_parts_box_centre(cx: &mut TestAppContext) {
    let (window, view) = block(cx, "dlg-triad");
    let (part, lo, hi) = cx.update(|cx| {
        let b = view.read(cx).display.bodies.first().cloned().expect("the block");
        let lo = b.mesh.positions.iter().fold(glam::DVec3::splat(f64::MAX), |m, p| m.min(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)));
        let hi = b.mesh.positions.iter().fold(glam::DVec3::splat(f64::MIN), |m, p| m.max(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)));
        (Reference::Body(b.id.clone()), lo, hi)
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("transform", w, cx))).unwrap();
    draw(cx, window);
    cx.update(|cx| view.update(cx, |v, cx| {
        if let Some(e) = &v.edit {
            e.dialog.update(cx, |d, _| {
                d.feature.params.set("transformType", noshape_model::params::ParamValue::Enum("TRANSLATION_3D".into()));
                d.feature.params.set("entities", noshape_model::params::ParamValue::Query(vec![part.clone()]));
            });
        }
    }));
    draw(cx, window);
    let origin = cx.update(|cx| view.read(cx).current_triad(cx).map(|t| t.origin)).expect("the triad");
    let centre = (lo + hi) * 0.5;
    assert!(origin.distance(centre) < 1e-6, "triad at {origin:?}, the part's box centre {centre:?}");
}

/// Live2 pattern-hole-1118 y (qa): Hole with no point: the title's tooltip ends "[Hole] did not regenerate properly: No
/// hole points selected." (an error naming only a selection field doesn't append "A parameter value is not valid.").
#[gpui::test]
fn a_hole_without_points_says_only_that(cx: &mut TestAppContext) {
    let (window, view) = block(cx, "dlg-hole-none");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("hole", w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Hole dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.schedule_regen(w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && dialog.read(cx).status.is_some());
    draw(cx, window);
    let title = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("dialog-title").expect("the title");
    crate::ui_script::clear_tooltip();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(gpui_kit::point(title.origin.x + px(20.), title.center().y), None, Modifiers::default());
    for _ in 0..50 {
        cx.executor().advance_clock(std::time::Duration::from_millis(10));
        draw(cx, window);
    }
    let tip = crate::ui_script::shown_tooltip().unwrap_or_default();
    assert!(tip.ends_with("did not regenerate properly: No hole points selected."), "the title's tooltip: {tip:?}");
}
