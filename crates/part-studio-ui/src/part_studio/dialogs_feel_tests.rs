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
        Row { id: "manipulators / patterns: none", live: "live2 pattern-hole-1118: Linear and Circular pattern draw no manipulator (no distance, count or angle arrow) with a part picked", gap: false, check: |cx| {
            expect((manipulator_for(cx, "linearPattern"), manipulator_for(cx, "circularPattern")), (false, false), "linear, circular")
        } },
        Row { id: "fields / Circular pattern's fields and defaults", live: "live2 pattern-hole-1118: Entities to pattern, Axis of pattern, Angle 360 deg, Instance count 4, Equal spacing on, Centered off, Skip instances off", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-circular-fields");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("circularPattern", w, cx))).unwrap();
            draw(cx, window);
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
            let want = ["Entities to pattern", "Axis of pattern", "Angle", "Instance count", "Equal spacing", "Centered", "Skip instances"];
            let (labels, values) = cx.update(|cx| {
                let d = dialog.read(cx);
                let p = &d.feature.params;
                let labels: Vec<&str> = d.ty.params.iter().filter(|s| s.is_visible(p) && want.contains(&s.label)).map(|s| s.label).collect();
                let text = |id: &str| d.input_text(id, cx).map(|(t, _)| t).unwrap_or_default();
                (labels, (text("angle"), text("instanceCount"), p.bool("equalSpace"), p.bool("isCentered"), p.bool("skipInstances")))
            });
            expect((labels, values), (want.to_vec(), ("360 deg".to_string(), "4".to_string(), true, false, false)), "(the fields in order, (Angle, Instance count, Equal spacing, Centered, Skip instances))")
        } },
        Row { id: "Hole / only sketch points light and take; a second click removes it", live: "live2 pattern-hole-1118: a part vertex or a face never lights and adds nothing; a sketch point lights, its click adds \"Vertex of …\" and fills Merge scope with Part 1; a click on it again removes it", gap: false, check: |cx| {
            let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
            let (window, view) = part_studio_from(cx, "dlg-hole-picks", Some(starts["hole-sketch"].clone()));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("hole", w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
            let pick_of = |cx: &mut TestAppContext, kind: noshape_scene::builder::PickKind| cx.update(|cx| view.read(cx).built.as_ref().and_then(|b| b.picks.ids_where(|t| t.kind == kind).next()));
            let event = |cx: &mut TestAppContext, ev: ViewportEvent| {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ev, w, cx);
                }))
                .unwrap();
                wait_until(cx, window, |cx| !view.read(cx).ui_busy());
                draw(cx, window);
            };
            let mut lit = Vec::new();
            for kind in [noshape_scene::builder::PickKind::Face, noshape_scene::builder::PickKind::Vertex, noshape_scene::builder::PickKind::SketchPoint] {
                let id = pick_of(cx, kind).ok_or(format!("no {kind:?} pick"))?;
                event(cx, ViewportEvent::Hover(Some(id)));
                lit.push(cx.update(|cx| !view.read(cx).highlighted_hover(cx).is_empty()));
            }
            let mut picked = Vec::new();
            for kind in [noshape_scene::builder::PickKind::Face, noshape_scene::builder::PickKind::Vertex, noshape_scene::builder::PickKind::SketchPoint, noshape_scene::builder::PickKind::SketchPoint] {
                let id = pick_of(cx, kind).ok_or(format!("no {kind:?} pick"))?;
                event(cx, ViewportEvent::Click { pick: Some(id), position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() });
                picked.push(cx.update(|cx| {
                    let p = &dialog.read(cx).feature.params;
                    (p.query("locations").len(), p.query("scope").len())
                }));
            }
            if let Some(last) = picked.last_mut() {
                last.1 = 0;
            }
            expect((lit, picked), (vec![false, false, true], vec![(0, 0), (0, 0), (1, 1), (0, 0)]), "(lit: a face, a part vertex, a sketch point), (locations, merge scope) after clicking a face, a vertex, the point; locations after the point again")
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
        Row { id: "manipulators / an arrow drag is the pointer projected on its line; Ctrl / Alt change nothing", live: "live manip-feel-1118: Depth is the pointer projected on the arrow's line (a sideways move changes nothing), Ctrl or Alt held change nothing (Shift drags unstepped), the arrow drawn dragged (one solid blue stroke)", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "dlg-arrow-projection");
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let (at, along, mm_per_px) = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, _) = vp.manipulators.first()?;
                    let (pa, pb) = (vp.project(a)?, vp.project(b)?);
                    let mid = (pa + pb) * 0.5;
                    let o = vp.origin();
                    Some((gpui_kit::point(o.x + px(mid.x as f32), o.y + px(mid.y as f32)), (pb - pa).normalize_or_zero(), (b - a).length() * 1000. / (pb - pa).length()))
                })
                .ok_or("no arrow")?;
            let to = |d: f64, side: f64| gpui_kit::point(at.x + px((along.x * d - along.y * side) as f32), at.y + px((along.y * d + along.x * side) as f32));
            let depth = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).input_text("depth", cx)).and_then(|(t, _)| t.split_whitespace().next()?.parse::<f64>().ok());
            let move_to = |cx: &mut TestAppContext, p, modifiers| {
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: p, pressed_button: Some(gpui_kit::MouseButton::Left), modifiers });
                draw(cx, window);
            };
            let start = depth(cx).ok_or("no Depth")?;
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
            vcx.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
            move_to(cx, to(20., 0.), Modifiers::default());
            move_to(cx, to(40., 0.), Modifiers::default());
            let plain = depth(cx).ok_or("no Depth")?;
            let dragged_look = cx.update(|cx| view.read(cx).manip_drag.is_some());
            move_to(cx, to(40., 25.), Modifiers::default());
            let sideways = depth(cx).ok_or("no Depth")?;
            move_to(cx, to(30., 0.), Modifiers::default());
            move_to(cx, to(40., 0.), Modifiers { control: true, ..Default::default() });
            let ctrl = depth(cx).ok_or("no Depth")?;
            move_to(cx, to(30., 0.), Modifiers::default());
            move_to(cx, to(40., 0.), Modifiers { alt: true, ..Default::default() });
            let alt = depth(cx).ok_or("no Depth")?;
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: to(40., 0.), modifiers: Modifiers::default(), click_count: 1 });
            draw(cx, window);
            let step = linear_step(mm_per_px * 0.001, 0.001) * 1000.;
            let projected = ((plain - start) - 40. * mm_per_px).abs() <= step * 0.5 + 1e-9;
            if !projected {
                return Err(format!("Depth {plain} mm after 40 px along the arrow from {start} mm: not {start} + 40 px × {mm_per_px:.4} mm/px on the {step} mm step"));
            }
            expect((dragged_look, sideways, ctrl, alt), (true, plain, plain, plain), "(the arrow drawn dragged, Depth after a 25 px sideways move, with Ctrl, with Alt; each against the plain drag's)")
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
        Row { id: "fields / a row hovered lights its entity", live: "its tooltip names the entity (\"Edge of Extrude 1\") to the chip's right; the selected orange outline turns light yellow-green (#f2ff80) while the row is hovered (live 121–181 ms after the move; ours in the next frame, no time passing); clears when the pointer leaves", gap: false, check: |cx| {
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
            crate::ui_script::clear_tooltip();
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: row.center(), pressed_button: None, modifiers: Modifiers::default() });
            draw(cx, window);
            let on = lit(cx);
            for _ in 0..50 {
                cx.executor().advance_clock(std::time::Duration::from_millis(10));
                draw(cx, window);
            }
            let tip = crate::ui_script::shown_tooltip().unwrap_or_default();
            if !tip.starts_with("Edge of Extrude 1") {
                return Err(format!("the row's tooltip {tip:?}"));
            }
            // To the chip's right (live2 ours-1118; FEEL-CHIP-TOOLTIP-PLACE), level with it, not under the pointer.
            let shown = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("placed-tooltip").ok_or("no tooltip drawn")?;
            if shown.left() < row.right() || shown.top() > row.bottom() || shown.bottom() < row.top() {
                return Err(format!("the tooltip at {shown:?}, the chip at {row:?}: not to its right"));
            }
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: row.center() + gpui_kit::point(px(0.), px(200.)), pressed_button: None, modifiers: Modifiers::default() });
            draw(cx, window);
            expect((before, on, lit(cx)), (false, true, false), "lit before, on the row, after leaving")
        } },
        Row { id: "fields / a row click leaves the selection as is", live: "clicking a row (not its x) keeps the field's entities; Delete and Backspace then do nothing (live2 ours-1118)", gap: false, check: |cx| {
            let (window, view, dialog, _) = fillet_on_an_edge(cx, "dlg-row-click")?;
            let path = cx.update(|cx| dialog.read(cx).active_query.clone()).ok_or("no active field")?;
            let row = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("chip-{path}-0").into_boxed_str())).ok_or("no row")?;
            let at = row.origin + gpui_kit::point(px(20.), row.size.height / 2.);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_click(at, Modifiers::default());
            draw(cx, window);
            let clicked = cx.update(|cx| dialog.read(cx).feature.params.query("entities").len());
            keys(cx, window, &["delete", "backspace"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let keyed = cx.update(|cx| (dialog.read(cx).feature.params.query("entities").len(), view.read(cx).edit.is_some()));
            expect((clicked, keyed), (1, (1, true)), "(entities after the click, (entities, dialog open) after Delete and Backspace)")
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
        Row { id: "fields / the wheel over a value field", live: "live2 field-keys-1118: unfocused, two notches over Depth change nothing; focused, a notch steps it 1 mm, the next the other way back", gap: false, check: |cx| {
            let (window, _view, dialog) = edit_extrude(cx, "dlg-wheel");
            let field = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("numf-depth").ok_or("no Depth field")?;
            let depth = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).feature.params.expr("depth").map(|e| e.text.clone()));
            let notch = |cx: &mut TestAppContext, dy: f32| {
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(gpui_kit::ScrollWheelEvent { position: field.center(), delta: gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., dy)), modifiers: Modifiers::default(), ..Default::default() });
                draw(cx, window);
            };
            let before = depth(cx);
            notch(cx, 1.);
            notch(cx, 1.);
            let unfocused = depth(cx);
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            draw(cx, window);
            let shown = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).input_text("depth", cx)).and_then(|(t, _)| t.split_whitespace().next()?.parse::<f64>().ok());
            let start = shown(cx).ok_or("no Depth text")?;
            notch(cx, 1.);
            let one = shown(cx).ok_or("no Depth text")?;
            notch(cx, -1.);
            let back = shown(cx).ok_or("no Depth text")?;
            expect((unfocused == before, ((one - start).abs() * 10.).round() / 10., back == start), (true, 1.0, true), "(unfocused unchanged, a focused notch's step, the next notch back)")
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
        Row { id: "preview / asked at the pick", live: "live 11-17 feature-dialogs: the preview is asked 113–330 ms after the pick (its round trip); ours in the frame of the pick", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-preview-asked");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let faces = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.bodies.first().map(|b| b.face_refs.len()));
            let (before_faces, before_seq) = (faces(cx), cx.update(|cx| view.read(cx).regen_seq));
            let pick = cx
                .update(|cx| {
                    let v = view.read(cx);
                    let e = v.display.bodies.first().and_then(|b| b.edge_refs.first().cloned())?;
                    v.built.as_ref().and_then(|b| b.picks.ids_for(&e).first().copied())
                })
                .ok_or("no edge pick")?;
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::Click { pick: Some(pick), position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() }, w, cx);
            }))
            .unwrap();
            let asked = cx.update(|cx| view.read(cx).regen_seq > before_seq && view.read(cx).pending.is_some());
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            expect((asked, before_faces, faces(cx)), (true, Some(6), Some(7)), "(a regeneration asked before any time passed, the block's faces before, the preview's)")
        } },
        Row { id: "preview / Linear pattern: a typed count waits for Enter", live: "live2 pattern-hole-1118: \"3\" typed into Instance count changes nothing for 3–4 s; Enter shows the preview (0.8–1.5 s on live's lane), the dialog stays open", gap: false, check: |cx| {
            use noshape_model::params::ParamValue;
            let (window, view) = block(cx, "dlg-pattern-count");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("linearPattern", w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let (part, edge) = cx
                .update(|cx| {
                    let b = view.read(cx).display.bodies.first()?.clone();
                    let along_x = b.edge_refs.iter().zip(&b.mesh.edges).find(|(_, pl)| matches!((pl.first(), pl.last()), (Some(a), Some(z)) if (a[0] - z[0]).abs() > 1e-3 && (a[1] - z[1]).abs() < 1e-6 && (a[2] - z[2]).abs() < 1e-6))?.0.clone();
                    Some((Reference::Body(b.id.clone()), along_x))
                })
                .ok_or("no part or x edge")?;
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
            dialog.update(cx, |d, cx| d.set_value("entities", ParamValue::Query(vec![part]), cx));
            dialog.update(cx, |d, cx| d.set_value("directionOne", ParamValue::Query(vec![edge]), cx));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let span = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.bounds().map(|(lo, hi)| ((hi - lo).max_element() * 1000.).round()));
            let two = span(cx);
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("instanceCount", w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["3"]);
            for _ in 0..150 {
                cx.executor().advance_clock(std::time::Duration::from_millis(20));
                draw(cx, window);
            }
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let typed = span(cx);
            keys(cx, window, &["enter"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let open = cx.update(|cx| view.read(cx).edit.is_some());
            expect((two, typed, span(cx), open), (Some(65.), Some(65.), Some(90.), true), "the parts' span (mm): 2 instances, 3 s after typing 3, after Enter; the dialog open")
        } },
        Row { id: "fields / after a field's menu item, one Esc cancels", live: "qa dialog-query-rclick-clear-all, fillet-chip-clear-missing: Entities to fillet › Clear all (or Clear missing references) leaves the keyboard on the page; one Esc closes the dialog (only Tab past its end leaves Esc dead)", gap: false, check: |cx| {
            let (window, view, dialog, _) = fillet_on_an_edge(cx, "dlg-menu-esc")?;
            // What a field's menu item does after its action (FeatureDialog::leave_keyboard_after_menu).
            dialog.update(cx, |d, cx| {
                d.keyboard_left = true;
                cx.emit(crate::feature_dialog::DialogEvent::LeaveFocus { forward: false });
            });
            draw(cx, window);
            keys(cx, window, &["escape"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            expect(cx.update(|cx| view.read(cx).edit.is_some()), false, "the dialog open after one Esc")
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
        Row { id: "Appearance / the part outlined while open, its new colour at once", live: "Part 1 row › Edit appearance…: the part's orange silhouette band, its faces untinted; a swatch: the part red on the next frame (38 ms), still outlined (live tools-1119 a1-appearance, a2-red; ours tinted it orange at 260 ms)", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-appearance-outline");
            let looks = |cx: &mut TestAppContext| cx.update(|cx| {
                let v = view.read(cx);
                let body = v.display.bodies.first().unwrap();
                let states = &v.viewport.read(cx).scene().states;
                let faces: Vec<u32> = body.face_refs.iter().flat_map(|f| v.built.as_ref().unwrap().picks.ids_for(f).to_vec()).collect();
                let all = |flag: u32| !faces.is_empty() && faces.iter().all(|&id| states.get(id as usize).is_some_and(|s| s & flag != 0));
                let any = |flag: u32| faces.iter().any(|&id| states.get(id as usize).is_some_and(|s| s & flag != 0));
                (all(noshape_render::scene::state::BAND), any(noshape_render::scene::state::SELECTED), body.color.map(|c| (c * 100.).round() / 100.))
            });
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                v.selection = vec![Reference::Body(b.clone())];
                v.edit_appearance(crate::part_studio::appearance::EditTarget::Parts(vec![b]), w, cx);
            }))
            .unwrap();
            draw(cx, window);
            let (open_band, open_tint, _) = looks(cx);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.set_edit_color([255, 0, 0], w, cx))).unwrap();
            draw(cx, window);
            let red = looks(cx);
            expect((open_band, open_tint, red), (true, false, (true, false, [1., 0., 0., 1.])), "(opened: the band, a tint), (a red swatch, before the regeneration: the band, a tint, the part's colour)")
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
        Row { id: "look / the first box active: blue frame on pale blue; empty and required, the title red", live: "live feature-dialogs-1117: a new Fillet opens with Entities to fillet active (border rgb(22,81,176), fill rgb(222,241,251)); with nothing picked the title is red", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-open-look");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("fillet", w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
            let active = cx.update(|cx| dialog.read(cx).active_query.clone());
            let red = cx.update(|cx| dialog.read(cx).status.as_ref().is_some_and(crate::feature_dialog::status_reads_red));
            let pal = crate::theme::light();
            expect((active.as_deref(), pal.accent, pal.query_active, red), (Some("entities"), gpui_kit::rgb(0x1651b0), gpui_kit::rgb(0xdef1fb), true), "(the active box, its frame, its fill, the title red)")
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

/// Routing curve's Point edit type Multiple (std routingCurve.fs addMultiPointsManipulator / multiTriadManipulator,
/// live2 audit-1119 missing-commands row 4): a click on a point toggles it in Selected points; the triad sits at the
/// selected points' centre and its X arrow moves each of them by the drag, the others stay.
#[gpui::test]
fn a_routing_curves_multiple_triad_moves_the_selected_points(cx: &mut TestAppContext) {
    use noshape_model::params::{ParamValue, ParamValues};
    let (window, view) = block(cx, "dlg-routing-multi");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("routingCurve", w, cx))).unwrap();
    draw(cx, window);
    let item = || {
        let mut it = ParamValues::default();
        it.set("referenceType", ParamValue::Enum("ORIGIN".into()));
        it
    };
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Routing curve dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.set_value("pointsEditType", ParamValue::Enum("MULTI".into()), cx);
        d.set_value("points", ParamValue::Array(vec![item(), item(), item()]), cx);
    }));
    draw(cx, window);
    // Typed in the points' fields (std's editing logic places a new point itself).
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| {
        d.set_expr_text("points.1.dx", "40 mm", w, cx);
        d.set_expr_text("points.2.dy", "40 mm", w, cx);
    }))
    .unwrap();
    draw(cx, window);
    let triad = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).routing_triad(cx)).expect("Multiple mode's manipulator");
    assert!(!triad(cx).shown, "no triad before a point is selected");
    let at = |cx: &mut TestAppContext, w: glam::DVec3| cx.update(|cx| view.read(cx).viewport.read(cx).project(w)).expect("on screen");
    let event = |cx: &mut TestAppContext, ev: ViewportEvent| {
        let t = triad(cx);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.triad_event(t, &ev, w, cx))).unwrap();
        draw(cx, window);
    };
    let click = |cx: &mut TestAppContext, w: glam::DVec3| {
        let position = at(cx, w);
        event(cx, ViewportEvent::ToolDown { position, modifiers: Modifiers::default(), click_count: 1 });
        event(cx, ViewportEvent::ToolUp { position, modifiers: Modifiers::default() });
    };
    let mm = 1e-3;
    let typed = cx.update(|cx| view.read(cx).routing_points(cx));
    assert!(typed.len() == 3 && typed[1].1.distance(glam::DVec3::new(40.0 * mm, 0.0, 0.0)) < 1e-9 && typed[2].1.distance(glam::DVec3::new(0.0, 40.0 * mm, 0.0)) < 1e-9, "the points typed: {typed:?}");
    click(cx, glam::DVec3::new(40.0 * mm, 0.0, 0.0));
    click(cx, glam::DVec3::new(0.0, 40.0 * mm, 0.0));
    let selected = cx.update(|cx| view.read(cx).routing_selected(cx));
    let t = triad(cx);
    assert_eq!(selected, vec![1, 2], "the two points clicked");
    assert!(t.shown && t.origin.distance(glam::DVec3::new(20.0 * mm, 20.0 * mm, 0.0)) < 1e-9, "the triad at their centre: {:?}", t.origin);
    // The X arrow's middle, dragged 30 px along it.
    let segs = cx.update(|cx| view.read(cx).triad_hit_segments(&t, cx));
    let (a, b) = (at(cx, segs[0].0), at(cx, segs[0].1));
    let (grab, dir) = ((a + b) * 0.5, (b - a).normalize());
    event(cx, ViewportEvent::ToolDown { position: grab, modifiers: Modifiers::default(), click_count: 1 });
    event(cx, ViewportEvent::ToolDrag { position: grab + dir * 30.0 });
    event(cx, ViewportEvent::ToolUp { position: grab + dir * 30.0, modifiers: Modifiers::default() });
    let pts = cx.update(|cx| view.read(cx).routing_points(cx));
    let moved = pts[1].1.x - 40.0 * mm;
    assert!(moved > 1.0 * mm, "point 1 moved along X: {pts:?}");
    assert!((pts[2].1.x - moved).abs() < 1e-9 && (pts[2].1.y - 40.0 * mm).abs() < 1e-9 && pts[1].1.y.abs() < 1e-9, "point 2 by the same X only: {pts:?}");
    assert!(pts[0].1.length() < 1e-9, "the unselected point stays: {pts:?}");
    // A second click on a selected point takes it out.
    click(cx, pts[1].1);
    assert_eq!(cx.update(|cx| view.read(cx).routing_selected(cx)), vec![2], "point 1 deselected");
}

/// Live sel-feel (P0): a double-click on a part edge in the graphics selects it, then the second click deselects it;
/// no chain or loop is added.
#[gpui::test]
fn a_double_click_on_an_edge_selects_then_deselects_it(cx: &mut TestAppContext) {
    let (window, view) = block(cx, "dbl-edge");
    draw(cx, window);
    let (edge, pick) = cx.update(|cx| {
        let v = view.read(cx);
        let e = v.display.bodies.first().and_then(|b| b.edge_refs.first().cloned()).expect("an edge");
        let p = v.built.as_ref().and_then(|b| b.picks.ids_for(&e).first().copied()).expect("its pick");
        (e, p)
    });
    let clock = std::rc::Rc::new(std::cell::Cell::new(std::time::Instant::now()));
    let c = clock.clone();
    cx.update(|cx| view.read(cx).viewport.clone().update(cx, |vp, _| vp.set_clock(move || c.get())));
    let click = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::Click { pick: Some(pick), position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
        cx.update(|cx| view.read(cx).selection.clone())
    };
    let first = click(cx);
    clock.set(clock.get() + std::time::Duration::from_millis(200));
    let second = click(cx);
    assert_eq!((first, second), (vec![edge], Vec::new()), "the first click selects the edge alone, the second (a double-click) deselects it");
}

/// Live2 routing-multi (11-19): in Multiple the triad is the full one (rings at the tips, a centre ball); a ring
/// turns only the triad, 5° near a multiple, the points staying where they are; the X arrow then moves the selected
/// points along the turned X (10.2 mm after 45° about Y: +7.2125 X, −7.2125 Z); an Edit reference puts the triad on its
/// vertex, world-aligned, and moves keep the points' own references (only dx / dy / dz change).
#[gpui::test]
fn a_routing_curves_multiple_ring_turns_only_the_triad_and_its_arrows_follow_the_turn(cx: &mut TestAppContext) {
    use noshape_model::params::{ParamValue, ParamValues};
    let (window, view) = block(cx, "dlg-routing-ring");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("routingCurve", w, cx))).unwrap();
    draw(cx, window);
    let item = || {
        let mut it = ParamValues::default();
        it.set("referenceType", ParamValue::Enum("ORIGIN".into()));
        it
    };
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Routing curve dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.set_value("pointsEditType", ParamValue::Enum("MULTI".into()), cx);
        d.set_value("points", ParamValue::Array(vec![item(), item(), item()]), cx);
    }));
    // (std's editing logic for the new points first: it would put back offsets typed before it ran.)
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| {
        d.set_expr_text("points.1.dx", "40 mm", w, cx);
        d.set_expr_text("points.2.dy", "40 mm", w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let triad = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).routing_triad(cx)).expect("Multiple mode's manipulator");
    let at = |cx: &mut TestAppContext, w: glam::DVec3| cx.update(|cx| view.read(cx).viewport.read(cx).project(w)).expect("on screen");
    let event = |cx: &mut TestAppContext, ev: ViewportEvent| {
        let t = triad(cx);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.triad_event(t, &ev, w, cx))).unwrap();
        draw(cx, window);
    };
    let typed = cx.update(|cx| view.read(cx).routing_points(cx));
    assert!(typed.len() == 3 && typed[1].1.distance(glam::DVec3::new(0.04, 0.0, 0.0)) < 1e-9, "the points typed: {typed:?}");
    let drag = |cx: &mut TestAppContext, from: glam::DVec2, to: glam::DVec2| {
        event(cx, ViewportEvent::ToolDown { position: from, modifiers: Modifiers::default(), click_count: 1 });
        event(cx, ViewportEvent::ToolDrag { position: from + (to - from) * 0.5 });
        event(cx, ViewportEvent::ToolDrag { position: to });
        event(cx, ViewportEvent::ToolUp { position: to, modifiers: Modifiers::default() });
    };
    let mm = 1e-3;
    for p in [glam::DVec3::new(40.0 * mm, 0.0, 0.0), glam::DVec3::new(0.0, 40.0 * mm, 0.0)] {
        let position = at(cx, p);
        event(cx, ViewportEvent::ToolDown { position, modifiers: Modifiers::default(), click_count: 1 });
        event(cx, ViewportEvent::ToolUp { position, modifiers: Modifiers::default() });
    }
    let t = triad(cx);
    assert!(t.full && t.shown && t.origin.distance(glam::DVec3::new(20.0 * mm, 20.0 * mm, 0.0)) < 1e-9, "the full triad at the two points' centre: {t:?}");
    let before = cx.update(|cx| view.read(cx).routing_points(cx));
    // The Z tip's ring (its tick's middle) turned 45° about Y: the ring's centre carried round the triad's origin.
    let segs = cx.update(|cx| view.read(cx).triad_hit_segments(&t, cx));
    let ring = (segs[8].0 + segs[8].1) * 0.5;
    let turned = t.origin + glam::DQuat::from_axis_angle(glam::DVec3::Y, 44.0f64.to_radians()) * (ring - t.origin);
    let (from, to) = (at(cx, ring), at(cx, turned));
    drag(cx, from, to);
    let t = triad(cx);
    let x = glam::DVec3::new(1.0, 0.0, -1.0).normalize();
    assert!(t.axes[0].distance(x) < 1e-9 && t.axes[1].distance(glam::DVec3::Y) < 1e-9, "the triad turned 45° about Y (44° snapped): {:?}", t.axes);
    assert_eq!(cx.update(|cx| view.read(cx).routing_points(cx)), before, "the points stay where they were");
    // The turned X arrow's middle dragged 40 px along it: both selected points move along it on the 0.1 mm step.
    let segs = cx.update(|cx| view.read(cx).triad_hit_segments(&t, cx));
    let (a, b) = (at(cx, segs[0].0), at(cx, segs[0].1));
    let grab = (a + b) * 0.5;
    drag(cx, grab, grab + (b - a).normalize() * 40.0);
    let pts = cx.update(|cx| view.read(cx).routing_points(cx));
    let moved = pts[1].1 - before[1].1;
    let along = moved.dot(x);
    assert!(along > 1.0 * mm && (along / (0.1 * mm) - (along / (0.1 * mm)).round()).abs() < 1e-4 && moved.distance(x * along) < 1e-8, "point 1 moved along the turned X on the 0.1 mm step: {moved:?}");
    assert!((pts[2].1 - before[2].1).distance(moved) < 1e-9 && pts[0].1 == before[0].1, "point 2 by the same, point 0 stays: {pts:?}");
    // Its value box: the move, selected; "10" + Enter makes it 10 mm in all, the box staying (live2 e7).
    let shown = cx.update(|cx| view.read(cx).ui.triad_box.as_ref().map(|b| (b.drag.value, b.input.is_some())));
    assert!(shown.is_some_and(|(v, field)| (v - along).abs() < 1e-9 && field), "the box after the drag: {shown:?}, moved {along}");
    keys(cx, window, &["1", "0", "enter"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let pts = cx.update(|cx| view.read(cx).routing_points(cx));
    let moved = pts[1].1 - before[1].1;
    assert!(moved.distance(x * 10.0 * mm) < 1e-8 && (pts[2].1 - before[2].1).distance(moved) < 1e-8, "10 mm along the turned X in all: {moved:?}");
    let kept = cx.update(|cx| view.read(cx).ui.triad_box.as_ref().map(|b| (b.drag.value, b.input.is_some())));
    assert!(kept.is_some_and(|(v, field)| (v - 10.0 * mm).abs() < 1e-12 && field), "the box stays with 10 mm: {kept:?}");
    // An Edit reference: the triad on that vertex, world-aligned; an X drag moves the points along world X only.
    let vertex = cx.update(|cx| view.read(cx).display.bodies.first().and_then(|b| b.vertex_refs.first().cloned())).expect("a block vertex");
    let vertex_at = cx.update(|cx| view.read(cx).vertex_position(&vertex)).expect("its position");
    cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("multiEditReference", ParamValue::Query(vec![vertex]), cx)));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let t = triad(cx);
    assert!(t.origin.distance(vertex_at) < 1e-9 && t.axes == [glam::DVec3::X, glam::DVec3::Y, glam::DVec3::Z], "on the reference vertex, world-aligned: {t:?}");
    assert!(cx.update(|cx| view.read(cx).ui.triad_box.is_none()), "another Edit reference drops the value box");
    let before = cx.update(|cx| view.read(cx).routing_points(cx));
    let segs = cx.update(|cx| view.read(cx).triad_hit_segments(&t, cx));
    let (a, b) = (at(cx, segs[0].0), at(cx, segs[0].1));
    let grab = (a + b) * 0.5;
    drag(cx, grab, grab + (b - a).normalize() * 40.0);
    let pts = cx.update(|cx| view.read(cx).routing_points(cx));
    let moved = pts[1].1 - before[1].1;
    let t = triad(cx);
    assert!(moved.x > 1.0 * mm && moved.y.abs() < 1e-9 && moved.z.abs() < 1e-9 && (pts[2].1 - before[2].1).distance(moved) < 1e-9, "both points along world X: {moved:?}");
    assert!(t.origin.distance(vertex_at + moved) < 1e-9, "the triad moved with them from the vertex: {:?}", t.origin);
    let kinds = cx.update(|cx| dialog.read(cx).feature.params.array("points").iter().map(|it| it.enum_("referenceType").to_string()).collect::<Vec<_>>());
    assert_eq!(kinds, vec!["ORIGIN"; 3], "the points keep their own reference");
}

/// Live2 routing-multi ball-a (11-19): Multiple's centre ball is a free move that snaps to the hovered face's
/// inference points; on the block's +X face centre the triad lands there and takes the face's frame
/// (multiRotationMatrix "[[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]"), the selected points move with it,
/// translated, and multiDx/Dy/Dz is its place from the origin; no value box.
#[gpui::test]
fn a_routing_curves_multiple_ball_snaps_to_a_faces_inference_point(cx: &mut TestAppContext) {
    use noshape_model::params::{ParamValue, ParamValues};
    let (window, view) = block(cx, "dlg-routing-ball");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("routingCurve", w, cx))).unwrap();
    draw(cx, window);
    let item = || {
        let mut it = ParamValues::default();
        it.set("referenceType", ParamValue::Enum("ORIGIN".into()));
        it
    };
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Routing curve dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.set_value("pointsEditType", ParamValue::Enum("MULTI".into()), cx);
        d.set_value("points", ParamValue::Array(vec![item(), item(), item()]), cx);
    }));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| {
        d.set_expr_text("points.1.dx", "40 mm", w, cx);
        d.set_expr_text("points.2.dy", "40 mm", w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let mm = 1e-3;
    let at = |cx: &mut TestAppContext, w: glam::DVec3| cx.update(|cx| view.read(cx).viewport.read(cx).project(w)).expect("on screen");
    let event = |cx: &mut TestAppContext, ev: ViewportEvent| {
        let t = cx.update(|cx| view.read(cx).routing_triad(cx)).expect("Multiple mode's manipulator");
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.triad_event(t, &ev, w, cx))).unwrap();
        draw(cx, window);
    };
    for p in [glam::DVec3::new(40.0 * mm, 0.0, 0.0), glam::DVec3::new(0.0, 40.0 * mm, 0.0)] {
        let position = at(cx, p);
        event(cx, ViewportEvent::ToolDown { position, modifiers: Modifiers::default(), click_count: 1 });
        event(cx, ViewportEvent::ToolUp { position, modifiers: Modifiers::default() });
    }
    let start = cx.update(|cx| view.read(cx).routing_triad(cx)).expect("the triad").origin;
    let before = cx.update(|cx| view.read(cx).routing_points(cx));
    // The block's +X face (every corner at the largest x) and its centre.
    let (face, centre) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.first().expect("the block");
        let corners = |i: usize| {
            let (first, count) = b.mesh.face_ranges[i];
            b.mesh.indices[first as usize..(first + count) as usize].iter().map(|&k| b.mesh.positions[k as usize]).map(|p| glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).collect::<Vec<_>>()
        };
        let max_x = (0..b.face_refs.len()).flat_map(corners).map(|p| p.x).fold(f64::MIN, f64::max);
        let i = (0..b.face_refs.len()).find(|&i| corners(i).iter().all(|p| (p.x - max_x).abs() < 1e-9)).expect("the +X face");
        let ps = corners(i);
        let (lo, hi) = ps.iter().fold((glam::DVec3::MAX, glam::DVec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        // (The mesh is f32; the block's sizes are whole mm: its exact centre is the mesh's to the micrometre.)
        (b.face_refs[i].clone(), ((lo + hi) * 0.5e6).round() * 1e-6)
    });
    let ball = at(cx, start);
    event(cx, ViewportEvent::ToolDown { position: ball, modifiers: Modifiers::default(), click_count: 1 });
    assert!(cx.update(|cx| view.read(cx).triad_ball_dragged() && view.read(cx).viewport.read(cx).tool_hover), "the ball grabbed, the graphics still picked");
    let target = at(cx, centre);
    view.update(cx, |v, cx| {
        v.ui.pointer = Some(target);
        v.ui.hover_pick = Some(noshape_scene::builder::PickTarget { reference: face.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None });
        v.update_inference(false, cx);
    });
    wait_until(cx, window, |cx| view.read(cx).inference_choice().is_some());
    let snapped = cx.update(|cx| view.read(cx).inference_choice().map(|p| p.frame.origin)).expect("a snap point");
    assert!(snapped.distance(centre) < 1e-6, "the face centre chosen: {snapped} for {centre}");
    event(cx, ViewportEvent::ToolDrag { position: target });
    let pts = cx.update(|cx| view.read(cx).routing_points(cx));
    let moved = centre - start;
    assert!((pts[1].1 - before[1].1).distance(moved) < 1e-8 && (pts[2].1 - before[2].1).distance(moved) < 1e-8 && pts[0].1 == before[0].1, "the selected points translated with the triad during the drag: {pts:?}, by {moved}");
    event(cx, ViewportEvent::ToolUp { position: target, modifiers: Modifiers::default() });
    let t = cx.update(|cx| view.read(cx).routing_triad(cx)).expect("the triad");
    assert!(t.origin.distance(centre) < 1e-8 && t.axes[2].distance(glam::DVec3::X) < 1e-9, "on the face centre, its Z the face normal: {t:?}");
    let (matrix, d) = cx.update(|cx| {
        let p = &dialog.read(cx).feature.params;
        let num = |id: &str| p.expr(id).map(|x| x.text.clone()).unwrap_or_default();
        (p.get("multiRotationMatrix").cloned(), [num("multiDx"), num("multiDy"), num("multiDz")])
    });
    assert_eq!(matrix, Some(ParamValue::Text("[[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]".into())), "live's stored frame");
    let units = cx.update(|cx| view.read(cx).doc.read(cx).session.doc.settings.units);
    let values: Vec<f64> = d.iter().map(|t| noshape_units::evaluate_as(t, noshape_units::Dim::LENGTH, &units, &noshape_units::EmptyScope).map_or(f64::NAN, |q| q.value)).collect();
    assert!((0..3).all(|i| (values[i] - centre[i]).abs() < 1e-12), "multiDx/Dy/Dz: the triad's place from the origin: {d:?}, centre {centre}");
    assert!(cx.update(|cx| view.read(cx).ui.triad_box.is_none() && !view.read(cx).viewport.read(cx).tool_hover && view.read(cx).inference_choice().is_none()), "no value box; picking and the points end with the drag");
}

/// Live2 ours-1118 pattern (p1audit skip-02..04): with Skip instances on, "Instances to skip" takes the instances' rings:
/// the one under the pointer lights, a click skips its instance (the item (1, 0), the preview one part fewer) and a
/// second click on it keeps it again; the seed's dot takes nothing.
#[gpui::test]
fn a_skip_ring_lights_and_a_click_skips_its_instance_then_keeps_it(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm", "operationType": "NEW" } },
        { "type": "linearPattern", "name": "Linear pattern 1", "params": { "patternType": "PART", "entities": ["part 0"], "directionOne": ["edge through [20, 0, 0]"],
            "distance": "60 mm", "instanceCount": "3", "operationType": "NEW", "skipInstances": true } }
    ] });
    let (window, view) = part_studio_from(cx, "skip-rings", Some(start));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 3);
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Linear pattern 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.edit_feature(id, w, cx);
        v.zoom_fit(cx);
    }))
    .unwrap();
    // (Settled first: the dialog opens on its first query, Entities.) "Instances to skip" clicked: its field takes the rings.
    draw(cx, window);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the pattern's dialog").dialog.clone());
    // As the field's click: it becomes active and the keyboard leaves Entities' Tab stop for the dialog.
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| {
        d.user_focus_query("skippedInstances");
        w.focus(&gpui::Focusable::focus_handle(d, cx), cx);
        cx.notify();
    }))
    .unwrap();
    draw(cx, window);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).toggle_points(cx).is_some_and(|p| p.len() == 3));
    draw(cx, window);
    let shown = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.edit.as_ref().expect("the pattern's dialog").dialog.read(cx);
        (v.toggle_points(cx).map(<[_]>::len), d.active_query.clone(), v.display.feature_state(d.feature.id).map(|s| s.toggle_points.len()))
    });
    assert_eq!(shown.0, Some(3), "the rings once drawn, Instances to skip active: (rings, active field, state's points) {shown:?}");
    let at = |cx: &mut TestAppContext, index: (i64, i64)| cx.update(|cx| {
        let v = view.read(cx);
        let t = v.toggle_points(cx).unwrap().iter().find(|t| t.index == index).cloned().expect("the instance's point");
        (v.viewport.read(cx).project(glam::DVec3::from_array(t.pos)).expect("on screen"), t.seed)
    });
    let send = |cx: &mut TestAppContext, ev: ViewportEvent| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ev, w, cx);
        }))
        .unwrap();
        draw(cx, window);
    };
    let skipped = |cx: &mut TestAppContext| cx.update(|cx| {
        let d = dialog.read(cx);
        d.feature.params.array("skippedInstances").iter().map(|it| (it.expr("index1").map(|e| e.text.clone()), it.expr("index2").map(|e| e.text.clone()))).collect::<Vec<_>>()
    });
    let parts = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.bodies.len());
    let ((seed, is_seed), (ring, _)) = (at(cx, (0, 0)), at(cx, (1, 0)));
    assert!(is_seed, "the seed's point");
    // Over the seed's dot nothing lights; over a ring (3 px off its centre) it does.
    send(cx, ViewportEvent::PointerMove { position: seed, modifiers: Modifiers::default() });
    assert_eq!(cx.update(|cx| view.read(cx).ui.toggle_hover), None, "the seed's dot lit");
    send(cx, ViewportEvent::PointerMove { position: ring + glam::DVec2::new(3., 0.), modifiers: Modifiers::default() });
    assert_eq!(cx.update(|cx| view.read(cx).ui.toggle_hover), Some((1, 0)), "the ring under the pointer lights");
    send(cx, ViewportEvent::Click { pick: None, position: ring, modifiers: Modifiers::default() });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!((skipped(cx), parts(cx)), (vec![(Some("1".to_string()), Some("0".to_string()))], 2), "(skipped, parts) after the click");
    let rings = cx.update(|cx| {
        let v = view.read(cx);
        let d = v.edit.as_ref().expect("the dialog").dialog.read(cx);
        (d.toggle_points_field(), v.display.feature_state(d.feature.id).map(|s| s.toggle_points.iter().filter(|t| t.selected).count()))
    });
    assert_eq!(rings, (Some("skippedInstances"), Some(1)), "(the field taking rings, skipped rings) after the click");
    let (ring, _) = at(cx, (1, 0));
    send(cx, ViewportEvent::Click { pick: None, position: ring, modifiers: Modifiers::default() });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!((skipped(cx), parts(cx)), (vec![], 3), "(skipped, parts) after the second click");
    send(cx, ViewportEvent::Click { pick: None, position: seed, modifiers: Modifiers::default() });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!(skipped(cx), vec![], "a click on the seed's dot skips nothing");
}

/// Live2 pattern-hole-1118 (P0): Mirror of the part across one of its side faces: the plane box reads "Face of
/// Extrude 1", the preview comes, and the mirror touching its seed turns the operation to Add with the part as the merge
/// scope (one part after ✓).
#[gpui::test]
fn a_mirror_across_a_side_face_adds_into_its_seed(cx: &mut TestAppContext) {
    let (window, view) = block(cx, "dlg-mirror-side");
    let (part, side) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let at_x = |i: usize| {
            let Some(&(first, count)) = b.mesh.face_ranges.get(i) else { return false };
            b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| (b.mesh.positions[k as usize][0] - 0.04).abs() < 1e-6)
        };
        let side = b.face_refs.iter().enumerate().find(|(i, _)| at_x(*i)).map(|(_, r)| r.clone()).expect("the face at x = 40 mm");
        (Reference::Body(b.id.clone()), side)
    });
    view.update(cx, |v, _| v.selection = vec![part.clone()]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("mirror", w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Mirror dialog").dialog.clone());
    assert_eq!(cx.update(|cx| dialog.read(cx).feature.params.query("entities").to_vec()), vec![part.clone()], "the selected part is the seed");
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.user_focus_query("mirrorPlane");
        d.add_pick(side.clone(), cx);
    }));
    let label = cx.update(|cx| view.read(cx).labeler(cx)(&side));
    assert_eq!(label, "Face of Extrude 1", "the plane box's chip");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && dialog.read(cx).feature.params.enum_("operationType") == "ADD");
    let state = cx.update(|cx| {
        let p = &dialog.read(cx).feature.params;
        (p.enum_("operationType").to_string(), p.bool("defaultScope"), p.query("booleanScope").to_vec())
    });
    assert_eq!(state, ("ADD".to_string(), false, vec![part]), "(operation, Merge with all, merge scope) once previewed");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).edit.is_none());
    assert_eq!(cx.update(|cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count()), 1, "one part after ✓");
}

/// Live2 pattern-hole-1118 (P0): a new Linear pattern of the part shows no instance until its Direction is picked; the
/// edge picked, the preview has the instance with nothing else done.
#[gpui::test]
fn a_linear_patterns_instance_previews_once_its_direction_is_picked(cx: &mut TestAppContext) {
    let (window, view) = block(cx, "dlg-lp-direction");
    let (part, edge) = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let along_x = b.edge_refs.iter().zip(&b.mesh.edges).find(|(_, pl)| matches!((pl.first(), pl.last()), (Some(a), Some(z)) if (a[0] - z[0]).abs() > 1e-3 && (a[1] - z[1]).abs() < 1e-6 && (a[2] - z[2]).abs() < 1e-6));
        (Reference::Body(b.id.clone()), along_x.expect("an edge along x").0.clone())
    });
    // The model's extent in mm: 40 for the block, 65 with the instance 25 mm along x.
    let span = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display.bounds().map(|(lo, hi)| ((hi - lo).max_element() * 1000.).round()));
    view.update(cx, |v, _| v.selection = vec![part.clone()]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("linearPattern", w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Linear pattern dialog").dialog.clone());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert_eq!((cx.update(|cx| dialog.read(cx).feature.params.query("entities").to_vec()), span(cx)), (vec![part], Some(40.)), "(the seed, the extent) with no direction");
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.user_focus_query("directionOne");
        d.add_pick(edge, cx);
    }));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    assert_eq!(span(cx), Some(65.), "the instance previewed 25 mm along the edge after the Direction pick");
    // Translucent: the preview is drawn blended over the part as it was (at the Preview slider's rest), so the new
    // instance shows see-through.
    let mix = cx.update(|cx| view.read(cx).viewport.read(cx).scene().preview_mix);
    assert_eq!(mix, Some(crate::feature_dialog::PREVIEW_REST), "the preview blended at rest over the scene before it");
}

/// Live2 pattern-hole-1118 (P0): the Hole's style tabs: Counterbore adds its diameter and depth rows (10 mm, 5 mm),
/// Countersink its diameter and angle (10 mm, 90 deg); the Metric tab makes the diameter 5 mm.
#[gpui::test]
fn the_hole_tabs_bring_their_rows_and_values(cx: &mut TestAppContext) {
    use noshape_model::params::ParamValue;
    let (window, view) = block(cx, "dlg-hole-tabs");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("hole", w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Hole dialog").dialog.clone());
    // (The Hole's value rows are drawn with an icon, no label: live's recorder reads them as "".) The value rows' texts
    // in order.
    let values = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| {
            let state = dialog.read(cx).ui_state(w, cx);
            state["fields"].as_array().into_iter().flatten().filter(|f| f["kind"] == "number").filter_map(|f| f["text"].as_str().map(str::to_string)).collect::<Vec<_>>()
        })
        .unwrap()
    };
    let tab = |cx: &mut TestAppContext, id: &'static str, value: &'static str| {
        cx.update(|cx| dialog.update(cx, |d, cx| d.set_value(id, ParamValue::Enum(value.into()), cx)));
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        draw(cx, window);
    };
    let has = |v: &[String], pair: [&str; 2]| v.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1]);
    let simple = values(cx);
    tab(cx, "styleV2", "C_BORE");
    let cbore = values(cx);
    assert!(cbore.len() == simple.len() + 2 && has(&cbore, ["10 mm", "5 mm"]), "Counterbore adds its diameter 10 mm and depth 5 mm rows: {simple:?} -> {cbore:?}");
    tab(cx, "styleV2", "C_SINK");
    let csink = values(cx);
    assert!(csink.len() == simple.len() + 2 && has(&csink, ["10 mm", "90 deg"]), "Countersink adds its diameter 10 mm and angle 90 deg rows: {simple:?} -> {csink:?}");
    tab(cx, "styleV2", "SIMPLE");
    assert_eq!(values(cx), simple, "Simple again: its rows as before");
    tab(cx, "unitsSystem", "METRIC");
    let metric = values(cx);
    assert_eq!(metric.first().map(String::as_str), Some("5 mm"), "Metric's diameter: {metric:?}");
}

/// Live2 pattern-hole-1118 (P0): a Hole's depth typed changes nothing (no regeneration, the preview as it was) until
/// Enter; Enter takes it and the preview regenerates, the dialog still open.
#[gpui::test]
fn a_typed_hole_depth_waits_for_enter(cx: &mut TestAppContext) {
    let (window, view) = block(cx, "dlg-hole-depth");
    let top = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        let at_top = |i: usize| {
            let Some(&(first, count)) = b.mesh.face_ranges.get(i) else { return false };
            b.mesh.indices[first as usize..(first + count) as usize].iter().all(|&k| (b.mesh.positions[k as usize][2] - 0.01).abs() < 1e-6)
        };
        b.face_refs.iter().enumerate().find(|(i, _)| at_top(*i)).map(|(_, r)| r.clone()).expect("the top face")
    });
    view.update(cx, |v, _| v.selection.clear());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("hole", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Hole dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, cx| {
        d.mc_mode = Some("locations".into());
        d.active_query = Some("locations".into());
        d.add_pick(top, cx);
    }));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let depth = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).feature.params.expr("holeDepthV3").map(|e| e.text.clone()));
    let shown = cx.update(|cx| view.read(cx).display.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("holeDepthV3", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["5"]);
    for _ in 0..150 {
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        draw(cx, window);
        let quiet = cx.update(|cx| view.read(cx).pending.is_none() && std::sync::Arc::ptr_eq(&view.read(cx).display, &shown));
        assert!(quiet, "a regeneration before Enter");
    }
    keys(cx, window, &["enter"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let taken = depth(cx).is_some_and(|t| t.starts_with('5'));
    let after = cx.update(|cx| (taken, !std::sync::Arc::ptr_eq(&view.read(cx).display, &shown), view.read(cx).edit.is_some()));
    assert_eq!(after, (true, true, true), "(the depth taken, the preview regenerated, the dialog open) after Enter");
}

/// Live2 qv-fillets (11-17): the Query variable dialog shows no count: with a face selected it opens with the face in
/// its box and no "n items" / "(n)" anywhere in its fields.
#[gpui::test]
fn the_query_variable_dialog_shows_no_count(cx: &mut TestAppContext) {
    let (window, view) = block(cx, "dlg-qv-count");
    let face = cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
        b.face_refs.first().cloned().expect("a face")
    });
    view.update(cx, |v, _| v.selection = vec![face]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("queryVariable", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Query variable dialog").dialog.clone());
    let state = cx.update_window(window, |_, w, cx| dialog.read(cx).ui_state(w, cx)).unwrap();
    let fields = state["fields"].as_array().cloned().unwrap_or_default();
    let picked: usize = fields.iter().filter(|f| f["kind"] == "query").map(|f| f["items"].as_array().map_or(0, Vec::len)).sum();
    assert_eq!(picked, 1, "the selected face in the dialog's box: {fields:?}");
    let texts: Vec<String> = fields
        .iter()
        .flat_map(|f| [f["label"].as_str(), f["text"].as_str().filter(|_| f["kind"] != "number")].into_iter().flatten().map(str::to_string).chain(f["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string))))
        .collect();
    let counted = |t: &String| {
        let words: Vec<&str> = t.split_whitespace().collect();
        let number = |w: &str| w.trim_matches(|c| c == '(' || c == ')').parse::<u32>().is_ok();
        words.first().is_some_and(|w| number(w) && words.len() > 1 && !w.starts_with('(')) || words.last().is_some_and(|w| w.starts_with('(') && number(w))
    };
    assert_eq!(texts.iter().filter(|t| counted(t)).collect::<Vec<_>>(), Vec::<&String>::new(), "no count in the dialog: {texts:?}");
    assert!(state.get("count").is_none(), "no count in the dialog's state");
}
