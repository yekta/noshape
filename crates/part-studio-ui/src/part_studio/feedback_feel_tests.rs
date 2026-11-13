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
        Row { id: "warning / the row's tooltip", live: "'Fillet 1', then '1 missing selection' (no 'did not regenerate properly')", gap: false, check: |cx| {
            let json = serde_json::json!({ "units": "mm", "features": [
                { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
                { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
                { "type": "chamfer", "name": "Chamfer 1", "params": { "entities": ["edge through [0, 15, 10]"], "width": "2 mm" } },
                { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]", "edge through [2, 15, 10]"], "radius": "1 mm" } },
                { "op": "reorder", "feature": "Fillet 1", "before": "Chamfer 1" }
            ] });
            let (window, _view) = start(cx, "fb-warning", json);
            let tip = row_tooltip(cx, window, "Fillet 1").ok_or("no tooltip")?;
            expect(tip.as_str(), "Fillet 1\n1 missing selection", "tooltip")
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
