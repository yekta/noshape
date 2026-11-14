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
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in &rows {
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
