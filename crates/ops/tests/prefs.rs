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
    // Dark mode (the account menu's View in dark mode beta), kept beside the Preferences page's other settings.
    std::fs::write(dir.join("preferences.json"), r#"{"language":"de"}"#).unwrap();
    assert_eq!(call(&mut ctx, "ui_preferences", json!({})).unwrap()["dark_mode"], false);
    assert_eq!(call(&mut ctx, "ui_preferences", json!({ "dark_mode": true })).unwrap()["dark_mode"], true);
    let kept: Value = serde_json::from_slice(&std::fs::read(dir.join("preferences.json")).unwrap()).unwrap();
    assert_eq!(kept, json!({ "language": "de", "darkMode": true }));
    let e = call(&mut ctx, "ui_preferences", json!({ "dark_mode": "yes" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("dark_mode"), "{e:?}");

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

    custom_properties(&mut ctx, &dir);
    export_defaults(&mut ctx, &dir);
    libraries(&mut ctx, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder's Set library ▸: kept per user, one folder per library, unset with `clear`.
fn libraries(ctx: &mut Ctx, dir: &std::path::Path) {
    let profiles = dir.join("Profiles");
    std::fs::create_dir_all(&profiles).unwrap();
    let l = call(ctx, "documents_set_library", json!({ "library": "My frame profiles library", "folder": profiles.to_str().unwrap() })).unwrap();
    assert_eq!(l["libraries"]["frame_profiles"], profiles.to_str().unwrap(), "{l}");
    let l = call(&mut Ctx::new(), "documents_set_library", json!({ "library": "images", "folder": profiles.to_str().unwrap() })).unwrap();
    assert_eq!(l["libraries"].as_object().unwrap().len(), 2, "{l}");
    let l = call(ctx, "documents_set_library", json!({ "library": "images", "clear": true })).unwrap();
    assert!(l["libraries"]["images"].is_null(), "{l}");
    let e = call(ctx, "documents_set_library", json!({ "library": "nope", "folder": profiles.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("library"), "{e:?}");
    let e = call(ctx, "documents_set_library", json!({ "library": "images", "folder": dir.join("none").to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("folder"), "{e:?}");
}

/// The Export dialog's remembered options are export_parts' defaults (live exportSolidOptions): STL as text in
/// mm after the dialog last exported so; an option given wins.
fn export_defaults(ctx: &mut Ctx, dir: &std::path::Path) {
    noshape_ops::export::SolidExportPrefs { stl_text: true, stl_units: 4, ..Default::default() }.save();
    let stl = dir.join("remembered.stl");
    call(ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STL", "file": stl.to_str().unwrap() })).unwrap();
    let text = std::fs::read_to_string(&stl).expect("a text STL");
    let max_x = text.lines().filter_map(|l| l.trim().strip_prefix("vertex ")).filter_map(|v| v.split_whitespace().next()?.parse::<f64>().ok()).fold(0.0_f64, |m, x| m.max(x.abs()));
    assert!((max_x - 20.0).abs() < 1e-6, "in mm: {max_x}");
    call(ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STL", "stl_text": false, "file": stl.to_str().unwrap() })).unwrap();
    let bytes = std::fs::read(&stl).unwrap();
    let n = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 84 + 50 * n, "binary as asked");
}

/// Company settings › Custom properties (drawing.md A15): definitions kept
/// per user, values on parts, `{part.custom.<name>}` in a drawing note.
fn custom_properties(ctx: &mut Ctx, dir: &std::path::Path) {
    let l = call(ctx, "company_custom_properties", json!({})).unwrap();
    assert_eq!(l["custom_properties"], json!([]), "{l}");
    call(ctx, "company_custom_properties", json!({ "action": "create", "name": "Finish code", "type": "enum", "values": ["A1", "B2"], "applies": ["part", "drawing"] })).unwrap();
    call(ctx, "company_custom_properties", json!({ "action": "create", "name": "Weight class", "type": "integer" })).unwrap();
    let e = call(ctx, "company_custom_properties", json!({ "action": "create", "name": "finish CODE" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"), "names are unique: {e:?}");
    let l = call(&mut Ctx::new(), "company_custom_properties", json!({})).unwrap();
    assert_eq!(l["custom_properties"][0]["values"], json!(["A1", "B2"]), "kept: {l}");
    // A part's values, checked against the type.
    let path = dir.join("cp.noshape");
    call(ctx, "document_create", json!({ "path": path.to_str().unwrap() })).unwrap();
    let sk = call(ctx, "sketch_create", json!({ "plane": "plane:top" })).unwrap()["sketch"].as_str().unwrap().to_string();
    call(ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-20, -15], [20, 15]] })).unwrap();
    let region = call(ctx, "sketch_get", json!({ "sketch": sk })).unwrap()["regions"][0]["id"].as_str().unwrap().to_string();
    call(ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } })).unwrap();
    let e = call(ctx, "part_set_properties", json!({ "part": "Part 1", "custom": { "Weight class": "heavy" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("custom"), "{e:?}");
    let e = call(ctx, "part_set_properties", json!({ "part": "Part 1", "custom": { "Nope": "x" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("custom"), "{e:?}");
    call(ctx, "part_set_properties", json!({ "part": "Part 1", "custom": { "finish code": "b2", "Weight class": 3 } })).unwrap();
    // A drawing note shows the sheet part's value.
    let d = call(ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true })).unwrap();
    let name = d["drawing"].as_str().unwrap().to_string();
    call(ctx, "drawing_note", json!({ "drawing": name, "text": "FINISH {part.custom.Finish code}", "position": [60, 60] })).unwrap();
    let g = call(ctx, "drawing_get", json!({ "drawing": name })).unwrap();
    assert!(g["annotations"].as_array().unwrap().iter().any(|a| a["text"] == "FINISH B2"), "{}", g["annotations"]);
}
