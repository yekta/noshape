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
fn row_tooltip(cx: &mut TestAppContext, window: AnyWindowHandle, row: &str) -> Option<String> {
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
        // Ours: Fillet 100 mm on the 40 × 30 × 10 block's top edge regenerates "Ok" into a body with no faces (20 mm
        // fails as live: fillet-too-big); core's fillet, reported 11-13.
        Row { id: "error / a failed preview", live: "Fillet 100 mm on a block edge: no preview body, the title red, the field's entry 'Edge of Extrude 1' red; tooltip '… did not regenerate properly: Failed to fillet selections.'", gap: true, check: |cx| {
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
            let tip = hover_tooltip(cx, window, "dialog-title").unwrap_or_default();
            expect((d["titleState"].as_str(), query["state"].as_str(), tip.as_str()), (Some("error"), Some("error"), "Fillet 1 did not regenerate properly: Failed to fillet selections."), "(title, field, title's tooltip)")
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
