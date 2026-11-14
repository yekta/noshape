//! The fields matrix (docs/parity/interaction-feel.md "Fields"): live2's feel/fields.json "numericFields" rows (what a
//! number field shows after Enter, blurred and refocused, invalid and negative input, the arrow keys and the wheel,
//! the autocomplete list) on the block start's Extrude 1. A row ours doesn't match yet is a gap; the test fails when a
//! row's status moves either way.

use super::*;

/// Extrude 1 of the block opened for editing; its window, view and dialog.
fn edit_extrude(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>, Entity<crate::feature_dialog::FeatureDialog>) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, name, Some(starts["block"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let id = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Extrude 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(id, w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Extrude dialog").dialog.clone());
    (window, view, dialog)
}

/// A field of the dialog as its state reads it (its shown text, "ok" / "error"); "Draft angle" is the unlabelled
/// number on Draft's row.
fn field(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, label: &str) -> Option<(String, String)> {
    draw(cx, window);
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let fields = state["dialog"]["fields"].as_array()?.clone();
    let f = match label {
        "Draft angle" => {
            let at = fields.iter().position(|f| f["label"] == "Draft")?;
            fields[at + 1..].iter().find(|f| f["kind"] == "number")?.clone()
        }
        _ => fields.iter().find(|f| f["label"] == label)?.clone(),
    };
    Some((f["text"].as_str()?.to_string(), f["state"].as_str()?.to_string()))
}

/// `path`'s field focused with its text selected, `typed` typed over it, then `then` pressed.
fn type_in(cx: &mut TestAppContext, window: AnyWindowHandle, dialog: &Entity<crate::feature_dialog::FeatureDialog>, path: &str, typed: &str, then: &[&str]) {
    cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected(path, w, cx))).unwrap();
    draw(cx, window);
    let chars: Vec<String> = typed.chars().map(|c| if c == ' ' { "space".to_string() } else { c.to_string() }).collect();
    let mut all: Vec<&str> = chars.iter().map(String::as_str).collect();
    all.extend_from_slice(then);
    keys(cx, window, &all);
}

/// The keyboard off the dialog's fields (a click on the graphics' empty space blurs them, live).
fn blur(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>) {
    cx.update_window(window, |_, w, cx| {
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
}

fn depth(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>) -> String {
    field(cx, window, view, "Depth").map(|f| f.0).unwrap_or_default()
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
        Row { id: "Enter: the expression, its unit added", live: "'10+5' -> '(10+5) mm', '10.123456' -> '10.123456 mm', '1 in' stays", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-enter");
            let mut got = Vec::new();
            for typed in ["10+5", "10.123456", "1 in"] {
                type_in(cx, window, &dialog, "depth", typed, &["enter"]);
                got.push(depth(cx, window, &view));
            }
            expect(got, vec!["(10+5) mm".to_string(), "10.123456 mm".into(), "1 in".into()], "Depth after Enter")
        } },
        Row { id: "blurred: the value in the document unit, 3 decimals", live: "'(10+5) mm' -> '15 mm', '1 in' -> '25.4 mm', '10.123456' -> '10.123 mm'", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-blur");
            let mut got = Vec::new();
            for typed in ["10+5", "1 in", "10.123456"] {
                type_in(cx, window, &dialog, "depth", typed, &["enter"]);
                blur(cx, window, &view);
                got.push(depth(cx, window, &view));
            }
            expect(got, vec!["15 mm".to_string(), "25.4 mm".into(), "10.123 mm".into()], "Depth blurred")
        } },
        Row { id: "refocused: the expression again", live: "the field shows the typed expression ('(10+5) mm'): the value shown blurred is display only", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-refocus");
            type_in(cx, window, &dialog, "depth", "10+5", &["enter"]);
            blur(cx, window, &view);
            cx.update_window(window, |_, w, cx| dialog.update(cx, |d, cx| d.focus_field_selected("depth", w, cx))).unwrap();
            expect(depth(cx, window, &view), "(10+5) mm".to_string(), "Depth refocused")
        } },
        Row { id: "invalid: on Enter, kept, ✓ off", live: "'abc' + Enter: the field pink (invalid), the text kept, the title red, ✓ disabled", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-invalid");
            type_in(cx, window, &dialog, "depth", "abc", &["enter"]);
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            let f = field(cx, window, &view, "Depth").ok_or("no Depth")?;
            let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
            let d = &state["dialog"];
            expect((f.0.as_str(), f.1.as_str(), d["titleState"].as_str(), d["ok"].as_str()), ("abc", "error", Some("error"), Some("disabled")), "(text, field, title, ✓)")
        } },
        Row { id: "negative length: accepted", live: "'-5' in Depth: '-5 mm', not red (the extrude goes the other way)", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-negative");
            type_in(cx, window, &dialog, "depth", "-5", &["enter"]);
            let f = field(cx, window, &view, "Depth").ok_or("no Depth")?;
            expect((f.0.as_str(), f.1.as_str()), ("-5 mm", "ok"), "Depth")
        } },
        Row { id: "angle: unitless gets deg; negative refused", live: "'5' in Draft angle -> '5 deg'; '-5' -> '-5 deg' invalid", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-angle");
            cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("hasDraft", noshape_model::params::ParamValue::Bool(true), cx)));
            draw(cx, window);
            type_in(cx, window, &dialog, "draftAngle", "5", &["enter"]);
            let five = field(cx, window, &view, "Draft angle").ok_or("no Draft angle")?;
            type_in(cx, window, &dialog, "draftAngle", "-5", &["enter"]);
            let minus = field(cx, window, &view, "Draft angle").ok_or("no Draft angle")?;
            expect((five.0.as_str(), minus.0.as_str(), minus.1.as_str()), ("5 deg", "-5 deg", "error"), "'5', then '-5' (its state)")
        } },
        Row { id: "arrow keys step the value", live: "Up +1 unit, Down −1, Shift+Up +10, Alt+Up +1 (10 mm -> 11 mm …), the text selected after each", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-arrows");
            let mut got = Vec::new();
            for key in ["up", "down", "shift-up", "alt-up"] {
                type_in(cx, window, &dialog, "depth", "10", &["enter", key]);
                got.push(depth(cx, window, &view));
            }
            expect(got, vec!["11 mm".to_string(), "9 mm".into(), "20 mm".into(), "11 mm".into()], "Depth after Up, Down, Shift+Up, Alt+Up from 10")
        } },
        Row { id: "autocomplete: units and functions by what's typed", live: "Depth '1 m': m, max( x , y ), min( x , y ), mm; '1 i': in, isInteger( x ); '#': New variable…; Draft angle '5 m': max, min", gap: false, check: |cx| {
            let (window, _view, dialog) = edit_extrude(cx, "fl-complete");
            let list = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).ui_list()).map(|l| l["detail"].as_array().map(|r| r.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default()).unwrap_or_default();
            type_in(cx, window, &dialog, "depth", "1 m", &[]);
            let m = list(cx);
            type_in(cx, window, &dialog, "depth", "1 i", &[]);
            let i = list(cx);
            expect((m, i), (vec!["m".to_string(), "max( x , y )".into(), "min( x , y )".into(), "mm".into()], vec!["in".to_string(), "isInteger( x )".into()]), "'1 m', '1 i'")
        } },
        Row { id: "autocomplete: Esc closes the list only", live: "Esc closes the menu; the dialog stays open", gap: false, check: |cx| {
            let (window, view, dialog) = edit_extrude(cx, "fl-complete-esc");
            type_in(cx, window, &dialog, "depth", "1 m", &["escape"]);
            let open = cx.update(|cx| dialog.read(cx).ui_list().is_some());
            expect((open, cx.update(|cx| view.read(cx).edit.is_some())), (false, true), "(list open, dialog open)")
        } },
    ]
}

#[gpui::test]
fn the_fields_matrix_matches_live(cx: &mut TestAppContext) {
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
    eprintln!("fields matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}
