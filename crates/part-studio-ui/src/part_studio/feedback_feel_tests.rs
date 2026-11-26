//! The feedback matrix (docs/parity/interaction-feel.md "Feedback"): live2's feel/feedback.json rows for the Part Studio
//! (a failing or warning feature's row and tooltip). A row ours doesn't match yet is a gap; the test fails when a
//! row's status moves either way.

use super::*;
use gpui_kit::{MouseMoveEvent, point};

const NONE: Modifiers = Modifiers { shift: false, control: false, alt: false, platform: false, function: false };

fn start(cx: &mut TestAppContext, name: &str, json: serde_json::Value) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = part_studio_from(cx, name, Some(json));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    (window, view)
}

/// The tooltip shown 500 ms after the pointer comes onto `row`.
pub(super) fn row_tooltip(cx: &mut TestAppContext, window: AnyWindowHandle, row: &str) -> Option<String> {
    let b = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("feature:{row}").into_boxed_str()))?;
    let at = point(b.origin.x + px(40.), b.center().y);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_event(MouseMoveEvent { position: point(px(900.), px(600.)), pressed_button: None, modifiers: NONE });
    draw(cx, window);
    crate::ui_script::clear_tooltip();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
    for _ in 0..50 {
        cx.executor().advance_clock(std::time::Duration::from_millis(10));
        draw(cx, window);
    }
    crate::ui_script::shown_tooltip()
}

/// The tooltip shown 500 ms after the pointer comes onto the element `selector`.
fn hover_tooltip(cx: &mut TestAppContext, window: AnyWindowHandle, selector: &'static str) -> Option<String> {
    draw(cx, window);
    let b = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(selector)?;
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(MouseMoveEvent { position: point(px(900.), px(600.)), pressed_button: None, modifiers: NONE });
    draw(cx, window);
    crate::ui_script::clear_tooltip();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_event(MouseMoveEvent { position: point(b.origin.x + px(20.), b.center().y), pressed_button: None, modifiers: NONE });
    for _ in 0..50 {
        cx.executor().advance_clock(std::time::Duration::from_millis(10));
        draw(cx, window);
    }
    crate::ui_script::shown_tooltip()
}

fn feature_id(cx: &mut TestAppContext, view: &Entity<PartStudioView>, name: &str) -> noshape_model::FeatureId {
    cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == name).unwrap().id)
}

/// live2's warning: Fillet 1 on two edges, one made by Chamfer 1, moved above Chamfer 1 (it keeps one edge). The
/// kept edge is away from the chamfer, which then has nothing to fail on.
fn warning_start() -> serde_json::Value {
    serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
        { "type": "chamfer", "name": "Chamfer 1", "params": { "entities": ["edge through [40, 15, 10]"], "width": "2 mm" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 30, 0]", "edge through [38, 15, 10]"], "radius": "1 mm" } },
        { "op": "reorder", "feature": "Fillet 1", "before": "Chamfer 1" }
    ] })
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

fn starts() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap()
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "error / the row's tooltip", live: "'<Feature> did not regenerate properly: <message>', then '1 missing selection'", gap: false, check: |cx| {
            let (window, _view) = start(cx, "fb-error", starts()["block-broken"].clone());
            let tip = row_tooltip(cx, window, "Fillet 1").ok_or("no tooltip")?;
            let (first, second) = tip.split_once('\n').ok_or(format!("one line: {tip:?}"))?;
            expect((first.starts_with("Fillet 1 did not regenerate properly: "), second), (true, "1 missing selection"), &tip)
        } },
        Row { id: "error / the row and the Features header", live: "the row an error (rgb(170,37,45) name, red icon, warning overlay); the header's failures icon", gap: false, check: |cx| {
            let (window, view) = start(cx, "fb-error-row", starts()["block-broken"].clone());
            let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
            let row = state["features"]["rows"].as_array().and_then(|r| r.iter().find(|r| r["name"] == "Fillet 1")).cloned().ok_or("no row")?;
            let header_icon = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("feature-errors").is_some();
            expect((row["state"].as_str(), header_icon), (Some("error"), true), "(the row's state, the header's failures icon)")
        } },
        Row { id: "error / its dialog", live: "Sketch 1 deleted under Extrude 1: the title red, the lost region a red 'Missing Face' entry, ✓ not disabled (an edge whose feature is only below reads 'Edge of Extrude 1': fillet-chip-edit-healthy-moment)", gap: false, check: |cx| {
            let (window, view) = start(cx, "fb-error-dialog", starts()["block"].clone());
            let ids: Vec<_> = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().map(|f| (f.name.clone(), f.id)).collect());
            let id = |n: &str| ids.iter().find(|(m, _)| m == n).unwrap().1;
            cx.update(|cx| view.update(cx, |v, cx| v.delete_features(vec![id("Sketch 1")], cx)));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id("Extrude 1"), w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
            let d = &state["dialog"];
            let query = d["fields"].as_array().and_then(|f| f.iter().find(|f| f["kind"] == "query")).cloned().ok_or("no selection field")?;
            let items: Vec<String> = query["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string)).collect();
            expect((d["titleState"].as_str(), items, query["state"].as_str(), d["ok"].as_str() == Some("disabled")), (Some("error"), vec!["Missing Face".to_string()], Some("error"), false), "title, items, field, ✓ disabled")
        } },
        Row { id: "warning / the row's tooltip", live: "'Fillet 1', then '1 missing selection' (no 'did not regenerate properly')", gap: false, check: |cx| {
            let (window, _view) = start(cx, "fb-warning", warning_start());
            let tip = row_tooltip(cx, window, "Fillet 1").ok_or("no tooltip")?;
            expect(tip.as_str(), "Fillet 1\n1 missing selection", "tooltip")
        } },
        Row { id: "warning / the row and the Features header", live: "the name stays rgb(51,51,51), the icon gets the warning overlay; no failures icon after 'Features (n)'", gap: false, check: |cx| {
            let (window, view) = start(cx, "fb-warning-row", warning_start());
            let id = feature_id(cx, &view, "Fillet 1");
            let (status, message) = cx.update(|cx| view.read(cx).display.feature_state(id).map(|s| (s.status, s.message.clone()))).ok_or("no state")?;
            let header_icon = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("feature-errors").is_some();
            expect((status, crate::part_studio::row_reads_red(status, message.as_deref()), header_icon), (noshape_model::display::FeatureStatus::Warning, false, false), "(status, name red, header's failures icon)")
        } },
        Row { id: "error / a pick its field refuses", live: "the boss's cylinder as Part mirror's plane: the row's tooltip 'Mirror 1 did not regenerate properly: Select a mirror plane.' then '1 missing selection'; in the dialog the plane's entry a red 'Missing Face of Extrude 2' (live errors-1123 mi-cylplane)", gap: false, check: |cx| {
            let mut json = starts()["boss-block"].clone();
            json["features"].as_array_mut().unwrap().push(serde_json::json!({ "type": "mirror", "name": "Mirror 1", "params": { "patternType": "PART", "entities": ["part 0"], "mirrorPlane": ["face through [14, 0, 15]"] } }));
            let (window, view) = start(cx, "fb-refused-pick", json);
            let tip = row_tooltip(cx, window, "Mirror 1").ok_or("no tooltip")?;
            let id = feature_id(cx, &view, "Mirror 1");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
            let plane = state["dialog"]["fields"].as_array().and_then(|f| f.iter().find(|f| f["label"] == "Mirror plane")).cloned().ok_or("no Mirror plane field")?;
            let items: Vec<String> = plane["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string)).collect();
            expect((tip.as_str(), items, plane["state"].as_str()), ("Mirror 1 did not regenerate properly: Select a mirror plane.\n1 missing selection", vec!["Missing Face of Extrude 2".to_string()], Some("error")), "(tooltip, the plane's entries, its state)")
        } },
        Row { id: "warning / its dialog", live: "the title stays black with no message, no bubble or toast; the lost selection a grey italic 'Edge of Chamfer 1', the kept one normal", gap: false, check: |cx| {
            let (window, view) = start(cx, "fb-warning-dialog", warning_start());
            let id = feature_id(cx, &view, "Fillet 1");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
            let d = &state["dialog"];
            let query = d["fields"].as_array().and_then(|f| f.iter().find(|f| f["kind"] == "query")).cloned().ok_or("no selection field")?;
            let items: Vec<String> = query["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string)).collect();
            let bubble = cx.update(|cx| view.read(cx).message.clone());
            expect((d["titleState"].as_str(), items.iter().any(|i| i == "Edge of Chamfer 1"), bubble), (Some("ok"), true, None), "(title, the lost edge listed, a bubble)")
        } },
        Row { id: "error / the red title's tooltip", live: "hovering the red title shows '<Feature> did not regenerate properly: <message>'", gap: false, check: |cx| {
            let (window, view) = start(cx, "fb-error-title", starts()["block"].clone());
            let (sketch, extrude) = (feature_id(cx, &view, "Sketch 1"), feature_id(cx, &view, "Extrude 1"));
            cx.update(|cx| view.update(cx, |v, cx| v.delete_features(vec![sketch], cx)));
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(extrude, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let tip = hover_tooltip(cx, window, "dialog-title").ok_or("no tooltip")?;
            expect(tip.starts_with("Extrude 1 did not regenerate properly: "), true, &tip)
        } },
        Row { id: "fillet / R100 on a vertical edge builds", live: "behaviours 'blend (2986)': the block's vertical-edge fillet builds at every radius up to R100 (599.887 mm³ less, 5 faces), no error", gap: false, check: |cx| {
            let json = serde_json::json!({ "units": "mm", "features": [
                { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
                { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
                { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [0, 0, 5]"], "radius": "100 mm" } }
            ] });
            let (_window, view) = start(cx, "fb-vertical-r100", json);
            let id = feature_id(cx, &view, "Fillet 1");
            let (status, faces) = cx.update(|cx| {
                let v = view.read(cx);
                (v.display.feature_state(id).map(|s| s.status), v.display.bodies.iter().map(|b| b.face_refs.len()).collect::<Vec<_>>())
            });
            expect((status, faces), (Some(noshape_model::display::FeatureStatus::Ok), vec![5]), "Fillet 1's status, the faces")
        } },
        Row { id: "error / a failed preview", live: "Fillet 100 mm on a block edge: no preview body, the title red, the field's entry 'Edge of Extrude 1' red (the field not marked); tooltip '… did not regenerate properly: Failed to fillet selections.'", gap: false, check: |cx| {
            let json = serde_json::json!({ "units": "mm", "features": [
                { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
                { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
                { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } }
            ] });
            let (window, view) = start(cx, "fb-fail-preview", json);
            let id = feature_id(cx, &view, "Fillet 1");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("radius", w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["1", "0", "0", "enter"]);
            // The preview's run (debounced), then its answer.
            for _ in 0..50 {
                cx.executor().advance_clock(std::time::Duration::from_millis(20));
                draw(cx, window);
            }
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
            let d = &state["dialog"];
            let query = d["fields"].as_array().and_then(|f| f.iter().find(|f| f["kind"] == "query")).cloned().ok_or("no selection field")?;
            let entry_red = cx.update(|cx| {
                let dlg = dialog.read(cx);
                dlg.feature.params.query("entities").iter().all(|r| dlg.entry_failing("entities", r))
            });
            let tip = hover_tooltip(cx, window, "dialog-title").unwrap_or_default();
            // (Live's recorder reads the field itself "ok": fillet-too-big; the entry is what turns red.)
            expect((d["titleState"].as_str(), query["state"].as_str(), entry_red, tip.as_str()), (Some("error"), Some("ok"), true, "Fillet 1 did not regenerate properly: Failed to fillet selections."), "(title, field, its entry red, title's tooltip)")
        } },
        Row { id: "error / a fillet too big shows the round it tried", live: "live errors-1123 fillet too-big (R20 on the block's top face): the attempted fillet drawn in magenta (255, 0, 255) strips, the face's picks kept orange, the radius manipulator shown", gap: false, check: |cx| {
            let json = serde_json::json!({ "units": "mm", "features": [
                { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
                { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
                { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["face through [20, 15, 10]"], "radius": "20 mm" } }
            ] });
            let (window, view) = start(cx, "fb-fail-attempt", json);
            let id = feature_id(cx, &view, "Fillet 1");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let got = cx.update(|cx| {
                let v = view.read(cx);
                let magenta = v.display.feature_state(id).is_some_and(|s| s.debug.colored.iter().any(|c| matches!(c, noshape_model::display::ColoredDebug::Solid(_, [255, 0, 255]))));
                let states = &v.viewport.read(cx).scene().states;
                let picks = v.edit.as_ref().map(|e| e.dialog.read(cx).feature.params.query("entities").to_vec()).unwrap_or_default();
                let red = v.built.as_ref().is_some_and(|b| picks.iter().flat_map(|r| b.picks.ids_for(r)).any(|&id| states.get(id as usize).is_some_and(|s| s & noshape_render::scene::state::ERROR != 0)));
                (magenta, picks.len(), red, v.current_manip(cx).is_some())
            });
            expect(got, (true, 1, false, true), "(the attempt in magenta, the picks, any drawn red, the radius manipulator)")
        } },
        Row { id: "error / an edge a chamfer dragged above consumed: its chip Missing", live: "workflow 33 steps 23-26 (qa FEAT-MISSING-REF-CHIP): Fillet 1's chip reads 'Missing Edge of Extrude 1' in red; its menu Clear all / Clear missing references / Edit healthy moment", gap: false, check: |cx| {
            let json = serde_json::json!({ "units": "mm", "features": [
                { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
                { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
                { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [40, 15, 10]"], "radius": "1 mm" } },
                { "type": "chamfer", "name": "Chamfer 1", "params": { "entities": ["edge through [0, 15, 10]"], "width": "2 mm" } },
                { "op": "reorder", "feature": "Chamfer 1", "before": "Fillet 1" }
            ] });
            let (window, view) = start(cx, "fb-missing-chip", json);
            let id = feature_id(cx, &view, "Fillet 1");
            // Chamfer 1, now above Fillet 1, edited to take Fillet 1's edge.
            let chamfer = feature_id(cx, &view, "Chamfer 1");
            let edge = cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.features.iter().find(|f| f.id == id)).map(|f| f.params.query("entities").to_vec())).ok_or("no Fillet 1")?;
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(chamfer, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let chamfer_dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no Chamfer dialog")?;
            cx.update(|cx| chamfer_dialog.update(cx, |d, cx| d.set_value("entities", noshape_model::ParamValue::Query(edge), cx)));
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_open_dialog(w, cx))).unwrap();
            wait_until(cx, window, |cx| { let v = view.read(cx); v.edit.is_none() && !v.ui_busy() });
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
            let (labels, missing) = cx.update(|cx| {
                let d = dialog.read(cx);
                (d.chip_labels("entities"), d.feature.params.query("entities").iter().map(|r| d.ref_missing(r)).collect::<Vec<_>>())
            });
            let regen = cx.update(|cx| view.read(cx).display.feature_state(id).map(|s| (s.status, s.message.clone(), s.missing_refs.len())));
            expect((labels, missing), (vec!["Missing Edge of Extrude 1".to_string()], vec![true]), &format!("Fillet 1's chip, missing (red, Clear missing references); its regeneration {regen:?}"))
        } },
        Row { id: "error / inputs refused", live: "Thicken on a block's top face with Thickness 1 and 2 both 0 (live part-studio 2059, L19): 'Invalid input selections.', the title red, the face's entry dark and the face orange (selected), no field marked (qa ERROR-CHIP-RED-NO-FIELD)", gap: false, check: |cx| {
            let json = serde_json::json!({ "units": "mm", "features": [
                { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
                { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
                { "type": "thicken", "name": "Thicken 1", "params": { "entities": ["face through [20, 15, 10]"], "thickness1": "0 mm", "thickness2": "0 mm" } }
            ] });
            let (window, view) = start(cx, "fb-thicken-zero", json);
            let id = feature_id(cx, &view, "Thicken 1");
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let dialog = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.clone())).ok_or("no dialog")?;
            let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
            let d = &state["dialog"];
            let (message, entry_red, picked) = cx.update(|cx| {
                let dlg = dialog.read(cx);
                let picked = dlg.feature.params.query("entities");
                (dlg.status.as_ref().map(|s| s.1.clone()), picked.iter().any(|r| dlg.entry_failing("entities", r)), picked.len())
            });
            let marked: Vec<String> = d["fields"].as_array().map(|f| f.iter().filter(|f| f["state"] != "ok").filter_map(|f| f["label"].as_str().map(str::to_string)).collect()).unwrap_or_default();
            expect((d["titleState"].as_str(), message.as_deref(), picked, entry_red, marked), (Some("error"), Some("Invalid input selections."), 1, false, Vec::<String>::new()), "(title, message, faces picked, an entry red, fields marked)")
        } },
    ]
}

#[gpui::test]
fn the_feedback_matrix_matches_live(cx: &mut TestAppContext) {
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
    eprintln!("feedback matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}

/// qa HIST-TOAST-DROPPED-ON-RCLICK (live workflows-1123 19: the Merge toast still up 58 s after the merge, the user
/// clicking ≤ 11 s apart; 2351: left alone, gone at ≈ 35 s): a notice with a link stays its time from the last input
/// in the window, and closes that long after it.
#[gpui::test]
fn a_linked_notice_stays_while_the_user_works(cx: &mut TestAppContext) {
    let (window, view) = part_studio_from(cx, "feedback-merge-toast", None);
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let doc = cx.update(|cx| view.read(cx).doc.clone());
    let msg = "Merged B1 into Main.".to_string();
    let shown = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).message.as_ref().is_some_and(|(_, m)| *m == msg));
    let wait = |cx: &mut TestAppContext, secs: u64| {
        cx.executor().advance_clock(std::time::Duration::from_secs(secs));
        draw(cx, window);
    };
    let input = |cx: &mut TestAppContext| cx.update(|cx| {
        let now = cx.background_executor().now();
        doc.update(cx, |d, _| d.note_input(now));
    });
    cx.update(|cx| doc.update(cx, |d, cx| d.info_with_link(msg.clone(), "Revert to before this merge", 35, |_, _| {}, cx)));
    draw(cx, window);
    assert!(shown(cx), "the toast");
    for _ in 0..6 {
        wait(cx, 10);
        input(cx);
    }
    wait(cx, 10);
    assert!(shown(cx), "70 s on, the last input 10 s ago: still up");
    wait(cx, 26);
    assert!(!shown(cx), "35 s after the last input: gone");
    cx.update(|cx| doc.update(cx, |d, cx| d.info_with_link(msg.clone(), "Revert to before this merge", 35, |_, _| {}, cx)));
    draw(cx, window);
    wait(cx, 34);
    assert!(shown(cx), "left alone: up at 34 s");
    wait(cx, 2);
    assert!(!shown(cx), "left alone: gone at 36 s");
}

/// Live 11-12 "sketch snapping and hover" (state rows): an under-defined sketch's row tooltip is "Sketch 1 is not fully
/// defined".
#[gpui::test]
fn an_under_defined_sketchs_row_says_so(cx: &mut TestAppContext) {
    let (window, _view) = start(cx, "fb-underdefined", serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
    ] }));
    assert_eq!(row_tooltip(cx, window, "Sketch 1").as_deref(), Some("Sketch 1 is not fully defined"));
}

/// Live qa/mistakes-1125 (behaviours "First-hour mistakes", batches 1 and 2) on the Block 40 × 30 × 20 mm: each mistake's
/// row (error red or an ok-looking info) and its exact tooltip, and the parts left.
type Mistake = (&'static str, Vec<serde_json::Value>, &'static str, &'static str, &'static str, usize);

/// Live qa/mistakes-1125's cases on the Block 40 × 30 × 20 mm: (name, the features, the row, its state, its tooltip, the
/// parts left).
fn mistakes() -> Vec<Mistake> {
    let block = || vec![
        serde_json::json!({ "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] }),
        serde_json::json!({ "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } }),
    ];
    // A second block of its own, apart from the first.
    let apart = |at: f64| vec![
        serde_json::json!({ "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "corner_rectangle": [[at, 0], [at + 10.0, 10]] }] }),
        serde_json::json!({ "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }),
    ];
    let mut edges = Vec::new();
    for z in [0.0, 20.0] {
        edges.extend([[20.0, 0.0, z], [20.0, 30.0, z], [0.0, 15.0, z], [40.0, 15.0, z]]);
    }
    edges.extend([[0.0, 0.0, 10.0], [40.0, 0.0, 10.0], [0.0, 30.0, 10.0], [40.0, 30.0, 10.0]]);
    let all_edges: Vec<String> = edges.iter().map(|e| format!("edge through [{}, {}, {}]", e[0], e[1], e[2])).collect();
    let cases: Vec<Mistake> = vec![
        ("fillet", vec![serde_json::json!({ "type": "fillet", "name": "Fillet 1", "params": { "entities": all_edges, "radius": "12 mm" } })],
            "Fillet 1", "error", "Fillet 1 did not regenerate properly: Failed to fillet selections.", 1),
        ("chamfer", vec![serde_json::json!({ "type": "chamfer", "name": "Chamfer 1", "params": { "entities": all_edges, "width": "12 mm" } })],
            "Chamfer 1", "error", "Chamfer 1 did not regenerate properly: Failed to chamfer selections.", 1),
        ("shell", vec![serde_json::json!({ "type": "shell", "name": "Shell 1", "params": { "entities": ["face through [20, 15, 0]"], "thickness": "16 mm" } })],
            "Shell 1", "error", "Shell 1 did not regenerate properly: Could not shell part with selections.", 1),
        ("subtract-apart", [apart(60.0), vec![serde_json::json!({ "type": "booleanBodies", "name": "Boolean 1", "params": { "operationType": "SUBTRACTION", "targets": ["part 0"], "tools": ["part 1"] } })]].concat(),
            "Boolean 1", "ok", "Boolean 1", 1),
        ("union-apart", [apart(60.0), vec![serde_json::json!({ "type": "booleanBodies", "name": "Boolean 1", "params": { "operationType": "UNION", "tools": ["part 0", "part 1"] } })]].concat(),
            "Boolean 1", "ok", "Boolean 1", 2),
        ("mirror-no-plane", vec![serde_json::json!({ "type": "mirror", "name": "Mirror 1", "params": { "patternType": "PART", "entities": ["part 0"] } })],
            "Mirror 1", "error", "Mirror 1 did not regenerate properly: Select a mirror plane.", 1),
        ("union-consumed", [apart(30.0), vec![
            serde_json::json!({ "type": "booleanBodies", "name": "Boolean 1", "params": { "operationType": "UNION", "tools": ["part 0", "part 1"] } }),
            serde_json::json!({ "type": "booleanBodies", "name": "Boolean 2", "params": { "operationType": "UNION", "tools": ["part 0"] } }),
        ]].concat(), "Boolean 2", "error", "Boolean 2 did not regenerate properly: Need at least two parts or surfaces for a Boolean operation.", 1),
        ("extrude-open", vec![
            // (Closed here, opened by `start_mistake`: its region stored, then gone.)
            serde_json::json!({ "type": "newSketch", "name": "Open U", "plane": "Top", "shapes": [{ "line": [[50, 0], [50, 20]] }, { "line": [[50, 0], [70, 0]] }, { "line": [[70, 0], [70, 20]] }, { "line": [[50, 20], [70, 20]] }] }),
            serde_json::json!({ "type": "extrude", "name": "Extrude open", "params": { "entities": ["Open U region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }),
        ], "Extrude open", "error", "Extrude open did not regenerate properly: Select face or sketch region to extrude.\n1 missing selection", 1),
    ];
    cases.into_iter().map(|(n, extra, row, state, tip, parts)| (n, [block(), extra].concat(), row, state, tip, parts)).collect()
}

/// A mistake's studio; the open Extrude's sketch loses its closing line (y 20 mm) after its region was picked.
fn start_mistake(cx: &mut TestAppContext, name: &str, features: Vec<serde_json::Value>) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = start(cx, &format!("mistake-{name}"), serde_json::json!({ "units": "mm", "features": features }));
    if name != "extrude-open" {
        return (window, view);
    }
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            let tab = v.tab;
            let id = v.studio(cx).unwrap().features.iter().find(|f| f.name == "Open U").unwrap().id;
            v.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Open the U", cx, |ps| {
                    let f = ps.features.iter_mut().find(|f| f.id == id).unwrap();
                    let Some(noshape_model::params::ParamValue::Sketch(mut arc)) = f.params.get("sketch").cloned() else { panic!("a sketch") };
                    let d = std::sync::Arc::make_mut(&mut arc);
                    let top = d.sketch.entities().find(|e| match e.geometry {
                        noshape_sketch::Geometry::Line { start, end } => [start, end].iter().all(|p| d.sketch.point(*p).is_some_and(|p| (p.y - 0.02).abs() < 1e-9)),
                        _ => false,
                    }).unwrap().id;
                    d.sketch.remove_entity(top);
                    f.params.set("sketch", noshape_model::params::ParamValue::Sketch(arc));
                })
            });
        })
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    (window, view)
}

#[gpui::test]
fn first_hour_mistakes_rows_and_tooltips_as_live(cx: &mut TestAppContext) {
    let cases = mistakes();
    let mut diffs = Vec::new();
    for (name, features, row, state, tip, parts) in cases {
        let (window, view) = start_mistake(cx, name, features);
        let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
        let got_state = st["features"]["rows"].as_array().and_then(|r| r.iter().find(|r| r["name"] == row)).and_then(|r| r["state"].as_str()).map(str::to_string);
        let got_tip = row_tooltip(cx, window, row);
        let got_parts = cx.update(|cx| view.read(cx).display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count());
        if (got_state.as_deref(), got_tip.as_deref(), got_parts) != (Some(state), Some(tip), parts) {
            diffs.push(format!("{name}: ours ({got_state:?}, {got_tip:?}, {got_parts} parts), live ({state:?}, {tip:?}, {parts} parts)"));
        }
    }
    assert!(diffs.is_empty(), "{}", diffs.join("\n"));
}

/// Live qa/mistakes-1125: each mistake's edit dialog: the title red (only Booleans' info stays black), the message only in
/// the title's tooltip, ✓ enabled; Shell's face and the open Extrude's region chips red, Mirror's empty plane field
/// red; a new Hole with no point: ✓ disabled, its point field the active one, not red.
#[gpui::test]
fn first_hour_mistakes_dialogs_as_live(cx: &mut TestAppContext) {
    // (the case, its title red, the query fields drawn red, the chips drawn red)
    let want: &[(&str, bool, &[&str], &[&str])] = &[
        ("fillet", true, &[], &[]),
        ("chamfer", true, &[], &[]),
        ("shell", true, &[], &["Face of Extrude 1"]),
        ("subtract-apart", false, &[], &[]),
        ("union-apart", false, &[], &[]),
        ("mirror-no-plane", true, &["Mirror plane"], &[]),
        // (Live's REST pick was the whole sketch, "Missing Faces of Open U"; ours its one region.)
        ("extrude-open", true, &[], &["Missing Face of Open U"]),
    ];
    let mut diffs = Vec::new();
    for (name, features, row, ..) in mistakes() {
        let Some((_, red_title, red_fields, red_chips)) = want.iter().find(|w| w.0 == name) else { continue };
        let (window, view) = start_mistake(cx, name, features);
        let id = feature_id(cx, &view, row);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        draw(cx, window);
        let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
        let d = &st["dialog"];
        let queries: Vec<&serde_json::Value> = d["fields"].as_array().into_iter().flatten().filter(|f| f["kind"] == "query").collect();
        // (Drawn red: invalid and not the active field.)
        let red: Vec<String> = queries.iter().filter(|f| f["state"] == "error" && f["active"] != true).filter_map(|f| f["label"].as_str().map(str::to_string)).collect();
        let chips: Vec<String> = queries.iter().flat_map(|f| f["redItems"].as_array().cloned().unwrap_or_default()).filter_map(|v| v.as_str().map(str::to_string)).collect();
        let got = (d["titleState"].as_str() == Some("error"), red, chips, d["ok"].as_str() == Some("disabled"));
        let live = (*red_title, red_fields.iter().map(|s| s.to_string()).collect::<Vec<_>>(), red_chips.iter().map(|s| s.to_string()).collect::<Vec<_>>(), false);
        if got != live {
            diffs.push(format!("{name}: ours (title red, red fields, red chips, ✓ off) {got:?}, live {live:?}; fields {}", d["fields"]));
        }
    }
    // A new Hole with nothing selected.
    let (window, view) = start(cx, "mistake-dlg-hole", serde_json::json!({ "units": "mm", "features": mistakes()[0].1[..2].to_vec() }));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("hole", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let d = &st["dialog"];
    let red: Vec<String> = d["fields"].as_array().into_iter().flatten().filter(|f| f["kind"] == "query" && f["state"] == "error" && f["active"] != true).filter_map(|f| f["label"].as_str().map(str::to_string)).collect();
    let tip = hover_tooltip(cx, window, "dialog-title");
    let got = (d["titleState"].as_str() == Some("error"), red, d["ok"].as_str() == Some("disabled"), tip);
    // (Live's "Ø 6.35 mm THRU" was that account's remembered End; std's default is Blind 12 mm: behaviours "Hole" dialog.)
    let live = (true, Vec::<String>::new(), true, Some("Ø 6.35 mm ↧ 12 mm [Hole] did not regenerate properly: No hole points selected.".to_string()));
    if got != live {
        diffs.push(format!("hole: ours {got:?}, live {live:?}; fields {}", d["fields"]));
    }
    assert!(diffs.is_empty(), "{}", diffs.join("\n"));
}

/// Live errors-1123 too-big: a Fillet of two edges, one failing alone (a hole's rim on an R8 fillet, picked off its seam), opened: its entry
/// red, the other's not. The blame comes after the dialog opens, off regeneration (P0 11-26: a rerun per edge cost
/// c2-impeller-loft's failed Fillet 52 s); regeneration alone blames neither.
#[gpui::test]
fn a_failed_fillets_dialog_reds_the_edge_that_fails_alone(cx: &mut TestAppContext) {
    let features = serde_json::json!([
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [30, 0, 20]"], "radius": "8 mm" } },
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[30, 4], [33, 4]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "40 mm", "symmetric": true, "operationType": "REMOVE" } },
        { "type": "fillet", "name": "Fillet 2", "params": { "entities": ["edge through [30, 7, 19.93725]", "edge through [60, 40, 10]"], "radius": "1 mm" } },
    ]);
    let (window, view) = start(cx, "mistake-blame", serde_json::json!({ "units": "mm", "features": features }));
    let id = feature_id(cx, &view, "Fillet 2");
    let at_regen = cx.update(|cx| view.read(cx).display.feature_state(id).map(|s| (s.status, s.error_refs.len())));
    assert_eq!(at_regen, Some((noshape_model::display::FeatureStatus::Error, 0)), "the failed Fillet, no blame at regeneration");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).blend_blame.is_some());
    draw(cx, window);
    let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let field = st["dialog"]["fields"].as_array().into_iter().flatten().find(|f| f["kind"] == "query").cloned().unwrap_or_default();
    let blamed = cx.update(|cx| view.read(cx).blend_blame.as_ref().map(|b| b.2.len()));
    assert!(field["redItems"].as_array().map(Vec::len) == Some(1) && blamed == Some(1), "one entry red: {field}; blamed {blamed:?}");
}

/// Live mistakes-1125 batch 2: the failing Fillet (12 edges R12) fixed in its dialog (Radius 2 mm, Enter: the dialog stays
/// open, its title black; ✓) is OK; Ctrl+Z brings the error back with the dialog closed, Ctrl+Y the fix, Ctrl+Z the error.
#[gpui::test]
fn undoing_a_fixed_fillet_brings_its_error_back(cx: &mut TestAppContext) {
    let (name, features, row, ..) = mistakes().into_iter().find(|m| m.0 == "fillet").unwrap();
    let (window, view) = start_mistake(cx, name, features);
    let id = feature_id(cx, &view, row);
    let row_state = |cx: &mut TestAppContext| {
        let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
        st["features"]["rows"].as_array().and_then(|r| r.iter().find(|r| r["name"] == row)).and_then(|r| r["state"].as_str()).unwrap_or("none").to_string()
    };
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the dialog").dialog.clone());
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("radius", w, cx))).unwrap();
    draw(cx, window);
    keys(cx, window, &["2", "enter"]);
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let after_enter = (cx.update(|cx| view.read(cx).edit.is_some()), st["dialog"]["titleState"].as_str().map(str::to_string));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_edit(false, w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let mut states = vec![row_state(cx)];
    for undo in [true, false, true] {
        cx.update(|cx| view.update(cx, |v, cx| v.doc.update(cx, |d, cx| if undo { d.undo(cx) } else { d.redo(cx) })));
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        draw(cx, window);
        states.push(row_state(cx));
    }
    let closed = cx.update(|cx| view.read(cx).edit.is_none());
    assert_eq!((after_enter, states, closed), ((true, Some("ok".to_string())), vec!["ok".to_string(), "error".into(), "ok".into(), "error".into()], true));
}

/// Live mistakes-1125 batch 2: a studio with only a surface exports (the full Export dialog); one with only a sketch has
/// "Selected tab contains no supported geometry for export.".
#[gpui::test]
fn a_surface_exports_and_a_sketch_alone_has_no_geometry(cx: &mut TestAppContext) {
    let u = serde_json::json!({ "type": "newSketch", "name": "Open U", "plane": "Top", "shapes": [{ "line": [[0, 0], [0, 20]] }, { "line": [[0, 0], [40, 0]] }, { "line": [[40, 0], [40, 20]] }] });
    let surf = serde_json::json!({ "type": "extrude", "name": "Surface 1", "params": { "bodyType": "SURFACE", "surfaceOperationType": "NEW", "surfaceEntities": ["Open U edges"], "endBound": "BLIND", "depth": "30 mm" } });
    let mut got = Vec::new();
    for (name, features) in [("surf-only", vec![u.clone(), surf]), ("sketch-only", vec![u])] {
        let (window, view) = start(cx, &format!("export-{name}"), serde_json::json!({ "units": "mm", "features": features }));
        let st = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
        got.push((cx.update(|cx| view.read(cx).has_export_geometry()), st["features"]["rows"].to_string()));
    }
    assert_eq!(got.iter().map(|g| g.0).collect::<Vec<_>>(), vec![true, false], "(surface only, sketch only) exportable: {got:?}");
}

/// Live qa/mistakes-1125: a rectangle dimensioned 40 x 20 with a third length on the opposite side, conflicting (30) or
/// only redundant (40): the row red, its tooltip "<name> did not regenerate properly: Sketch could not be solved.";
/// opened, the sketch's title red and ✓ enabled. Live overdefined-downstream-1125: an Extrude of it (hiding it) keeps
/// building from the last solved rectangle (8000 mm³), a new one of its region too (12000 mm³), and the tooltip ends
/// "… Sketch could not be solved. (Hidden)".
#[gpui::test]
fn an_over_defined_sketchs_row_and_dialog_as_live(cx: &mut TestAppContext) {
    use noshape_model::display::FeatureStatus;
    for (name, top) in [("Sketch conflict", "30 mm"), ("Sketch redundant", "40 mm")] {
        let extruded = name == "Sketch redundant";
        let mut start = serde_json::json!({ "units": "mm", "features": [
            { "type": "newSketch", "name": name, "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 20]] }] },
        ] });
        if extruded {
            start["features"].as_array_mut().unwrap().push(serde_json::json!({ "type": "extrude", "name": "Extrude 1", "params": { "entities": [format!("{name} region 0")], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }));
        }
        let (window, view) = part_studio_with(cx, &format!("fb-overdefined-{}", &name[7..]), |path| {
            let mut ctx = noshape_ops::Ctx::new();
            noshape_ops::ui_parity::build_start(&mut ctx, path, &start).unwrap();
            let mut call = |tool: &str, args: serde_json::Value| {
                let op = noshape_ops::registry::by_tool_name(tool).unwrap();
                noshape_ops::registry::run(&mut ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {}", e.to_json())).value
            };
            let g = call("sketch_get", serde_json::json!({ "sketch": name }));
            let line = |a: [f64; 2], b: [f64; 2]| {
                let at = |p: &serde_json::Value, q: [f64; 2]| (p[0].as_f64().unwrap() - q[0]).abs() < 1e-6 && (p[1].as_f64().unwrap() - q[1]).abs() < 1e-6;
                g["entities"].as_array().unwrap().iter().filter(|e| e["type"] == "line")
                    .find(|e| (at(&e["start"], a) && at(&e["end"], b)) || (at(&e["start"], b) && at(&e["end"], a)))
                    .unwrap_or_else(|| panic!("no line {a:?}-{b:?}"))["id"].clone()
            };
            let (bottom, left, upper) = (line([0., 0.], [40., 0.]), line([0., 0.], [0., 20.]), line([0., 20.], [40., 20.]));
            call("sketch_dimension", serde_json::json!({ "sketch": name, "entities": [bottom], "value": "40 mm", "label": [20, -8] }));
            call("sketch_dimension", serde_json::json!({ "sketch": name, "entities": [left], "value": "20 mm", "label": [-8, 10] }));
            call("sketch_dimension", serde_json::json!({ "sketch": name, "entities": [upper], "value": top, "label": [20, 28] }));
            if extruded {
                let volume = |call: &mut dyn FnMut(&str, serde_json::Value) -> serde_json::Value| (call("query_parts", serde_json::json!({}))["parts"].as_array().unwrap().iter().filter_map(|p| p["volume"].as_f64()).sum::<f64>() * 10.0).round() / 10.0;
                assert_eq!(volume(&mut call), 8000.0, "Extrude 1 keeps the last solved rectangle");
                let regions: Vec<serde_json::Value> = call("sketch_get", serde_json::json!({ "sketch": name }))["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
                assert_eq!(regions.len(), 1, "its one region");
                call("feature_add", serde_json::json!({ "type": "extrude", "params": { "entities": regions, "endBound": "BLIND", "depth": "5 mm", "oppositeDirection": true, "operationType": "NEW" } }));
                assert_eq!(volume(&mut call), 12000.0, "Extrude 2 of its region builds");
            }
            crate::session::Session::open(path).unwrap()
        });
        cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        let id = feature_id(cx, &view, name);
        let status = cx.update(|cx| view.read(cx).display.feature_state(id).map(|s| (s.status, s.message.clone())));
        assert_eq!(status, Some((FeatureStatus::Error, Some("Sketch could not be solved.".to_string()))), "{name}: the row red");
        let tip = row_tooltip(cx, window, name);
        let hidden = if extruded { " (Hidden)" } else { "" };
        assert_eq!(tip, Some(format!("{name} did not regenerate properly: Sketch could not be solved.{hidden}")), "{name}: the row's tooltip");
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        draw(cx, window);
        let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
        let d = &state["dialog"];
        assert_eq!((d["titleState"].as_str(), d["ok"].as_str()), (Some("error"), Some("enabled")), "{name}: the sketch's title red, ✓ enabled: {d}");
    }
}
