//! The user's preferences through the agent API (a test config directory:
//! its own process, as it sets NOSHAPE_CONFIG_DIR).

use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Result<Value, noshape_ops::OpError> {
    registry::run(ctx, registry::by_tool_name(tool).expect("tool"), &args).map(|o| o.value)
}

#[test]
fn graphics_preferences_toolbar_and_shortcuts_read_set_and_kept() {
    let dir = std::env::temp_dir().join(format!("noshape-prefs-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: this test binary runs this one test; nothing else reads the environment.
    unsafe { std::env::set_var("NOSHAPE_CONFIG_DIR", &dir) };
    let mut ctx = Ctx::new();
    let g = call(&mut ctx, "ui_preferences", json!({})).unwrap();
    assert_eq!((g["graphics"]["fov_deg"].clone(), g["graphics"]["specular"].clone()), (json!(45.0), json!("#999999")), "Onshape's defaults: {g}");
    let s = call(&mut ctx, "ui_preferences", json!({ "graphics": { "fov_deg": 60, "sketch_constrained": "#202020", "construction": [5, 5, 20, 5] } })).unwrap();
    assert_eq!(s["graphics"]["fov_deg"], 60.0, "{s}");
    assert_eq!(s["shown_in_app"], false, "{s}");
    // Saved per user: a new session reads them back.
    let again = call(&mut Ctx::new(), "ui_preferences", json!({})).unwrap();
    assert_eq!((again["graphics"]["fov_deg"].clone(), again["graphics"]["sketch_constrained"].clone()), (json!(60.0), json!("#202020")), "{again}");
    // The dialog's ranges.
    let e = call(&mut ctx, "ui_preferences", json!({ "graphics": { "fov_deg": 200 } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("graphics.fov_deg"), "{e:?}");
    let e = call(&mut ctx, "ui_preferences", json!({ "graphics": { "nope": 1 } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("graphics.nope"), "{e:?}");

    // The toolbar: Customize toolbar… and the S shortcut toolbar.
    let t = call(&mut ctx, "ui_toolbar", json!({})).unwrap();
    assert_eq!(t["shortcut_tools"].as_array().unwrap().len(), 10, "Onshape's ten: {t}");
    call(&mut ctx, "ui_toolbar", json!({ "set": { "removed": ["helix"], "shortcut_tools": ["sketch", "extrude", "fillet"], "hide_search": true } })).unwrap();
    let t = call(&mut Ctx::new(), "ui_toolbar", json!({})).unwrap();
    assert_eq!((t["toolbar"]["removed"].clone(), t["shortcut_tools"].clone(), t["toolbar"]["hide_search"].clone()), (json!(["helix"]), json!(["sketch", "extrude", "fillet"]), json!(true)), "{t}");
    let e = call(&mut ctx, "ui_toolbar", json!({ "set": { "shortcut_tools": ["nope"] } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.shortcut_tools"), "{e:?}");
    // The assembly toolbar (an Assembly tab in a document).
    call(&mut ctx, "document_create", json!({ "path": dir.join("tb.noshape").to_str().unwrap() })).unwrap();
    call(&mut ctx, "tab_create", json!({ "kind": "assembly" })).unwrap();
    let t = call(&mut ctx, "ui_toolbar", json!({ "tab": "Assembly 1" })).unwrap();
    assert_eq!(t["shortcut_tools"].as_array().unwrap().len(), 11, "Onshape's eleven: {t}");
    call(&mut ctx, "ui_toolbar", json!({ "tab": "Assembly 1", "set": { "removed": ["tb-gear"], "order": [1, 0], "hide_search": true } })).unwrap();
    let t = call(&mut Ctx::new(), "ui_toolbar", json!({ "doc": dir.join("tb.noshape").to_str().unwrap(), "tab": "Assembly 1" })).unwrap();
    assert_eq!((t["toolbar"]["removed"].clone(), t["sets"][0]["index"].clone(), t["toolbar"]["hide_search"].clone()), (json!(["tb-gear"]), json!(1), json!(true)), "{t}");
    assert!(!t["sets"].as_array().unwrap().iter().any(|s| s["tools"].as_array().unwrap().iter().any(|x| x["id"] == "tb-gear")), "{t}");
    let e = call(&mut ctx, "ui_toolbar", json!({ "tab": "Assembly 1", "set": { "removed": ["helix"] } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.removed"), "a Part Studio tool: {e:?}");

    // Help › Keyboard shortcuts: Onshape's keys with the user's changes.
    std::fs::write(dir.join("keyboard-shortcuts.json"), r#"{"sketch:Center point circle*": {"key": "shift-c"}, "general:Flip primary axis": {"disabled": true}}"#).unwrap();
    let k = call(&mut ctx, "ui_shortcuts", json!({ "tab": "Sketch" })).unwrap();
    let rows = k["tabs"][0]["rows"].as_array().unwrap();
    let circle = rows.iter().find(|r| r["name"] == "Center point circle*").expect("circle");
    assert_eq!((circle["keys"].clone(), circle["customized"].clone()), (json!("shift-c"), json!(true)), "{k}");
    let flip = call(&mut ctx, "ui_shortcuts", json!({ "search": "flip prim" })).unwrap();
    assert_eq!(flip["tabs"], json!([{ "tab": "General", "rows": [{ "name": "Flip primary axis", "keys": "a", "id": "general:Flip primary axis", "customized": true, "disabled": true }] }]), "{flip}");
    let help = call(&mut ctx, "ui_shortcuts", json!({ "search": "shift+/" })).unwrap();
    assert!(help["tabs"][0]["rows"].as_array().unwrap().iter().any(|r| r["name"] == "Keyboard shortcuts"), "{help}");
    let locked = call(&mut ctx, "ui_shortcuts", json!({ "tab": "General", "search": "accept command" })).unwrap();
    assert_eq!(locked["tabs"][0]["rows"][0]["locked"], true, "{locked}");
    let _ = std::fs::remove_dir_all(&dir);
}
