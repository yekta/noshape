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
    forms_library(&mut ctx, &dir);
    frames_library(&mut ctx, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder's Set library ▸: kept per user, one folder per library, unset with `clear`.
fn libraries(ctx: &mut Ctx, dir: &std::path::Path) {
    let profiles = dir.join("Profiles");
    std::fs::create_dir_all(&profiles).unwrap();
    // An empty folder is no library (live 2508).
    let e = call(ctx, "documents_set_library", json!({ "library": "frame_profiles", "folder": profiles.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.message, "Failed to update the library\n\nFolder \"Profiles\" is empty.");
    call(&mut Ctx::new(), "document_create", json!({ "path": profiles.join("profile doc.noshape").to_str().unwrap() })).unwrap();
    let l = call(ctx, "documents_set_library", json!({ "library": "My frame profiles library", "folder": profiles.to_str().unwrap() })).unwrap();
    assert_eq!(l["libraries"]["frame_profiles"], profiles.to_str().unwrap(), "{l}");
    // My images needs a versioned image in each document.
    let e = call(&mut Ctx::new(), "documents_set_library", json!({ "library": "images", "folder": profiles.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.message, "Failed to update the library\n\nDocument \"profile doc\" has 0 versioned images. One versioned image is required.");
    let l = call(&mut Ctx::new(), "documents_set_library", json!({ "library": "tolerances", "folder": profiles.to_str().unwrap() })).unwrap();
    assert_eq!(l["libraries"].as_object().unwrap().len(), 2, "{l}");
    let l = call(ctx, "documents_set_library", json!({ "library": "tolerances", "clear": true })).unwrap();
    assert!(l["libraries"]["tolerances"].is_null(), "{l}");
    let e = call(ctx, "documents_set_library", json!({ "library": "nope", "folder": profiles.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("library"), "{e:?}");
    let e = call(ctx, "documents_set_library", json!({ "library": "images", "folder": dir.join("none").to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("folder"), "{e:?}");
}

/// A form document: a Part Studio driven by a length configuration variable "thickness" (its 10 × 10 mm block's height), its block marked positive by a
/// Form Tag, versioned "V1" (the library reads a document at its latest version).
fn form_document(path: &std::path::Path) {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() })).unwrap();
    call(&mut ctx, "config_add_input", json!({ "kind": "variable", "name": "thickness", "value": "3 mm" })).unwrap();
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" })).unwrap()["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [10, 10]] })).unwrap();
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk })).unwrap()["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "#thickness" } })).unwrap();
    let part = call(&mut ctx, "query_parts", json!({})).unwrap()["parts"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "tag", "params": { "tagPurpose": "FORM", "positivePart": [part] } })).unwrap();
    call(&mut ctx, "history_create_version", json!({ "name": "V1" })).unwrap();
}

/// A Form whose Part Studio is a library form (its `ext:` value) stamps its tool, in the sheet's thickness.
fn library_form_on_a_plate(path: &std::path::Path, form: &str) {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() })).unwrap();
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" })).unwrap()["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [60, 40]] })).unwrap();
    call(&mut ctx, "feature_add", json!({ "type": "sheetMetalStart", "params": { "process": "THICKEN", "regions": [r["regions"][0]["id"]], "thickness": "1 mm", "radius": "1 mm" } })).unwrap();
    let volume = |ctx: &mut Ctx| call(ctx, "query_parts", json!({})).unwrap()["parts"][0]["volume"].as_f64().unwrap();
    let before = volume(&mut ctx);
    let top = call(&mut ctx, "query_entities", json!({ "kind": "faces", "normal": [0, 0, 1], "limit": 1 })).unwrap()["faces"][0]["id"].as_str().unwrap().to_string();
    let s2 = call(&mut ctx, "sketch_create", json!({ "plane": top })).unwrap()["sketch"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "point", "points": [[30, 20, 1]] })).unwrap();
    let pt: Vec<Value> = p["created"].as_array().unwrap().iter().filter(|x| x["type"] == "point").map(|x| x["id"].clone()).collect();
    let f = call(&mut ctx, "feature_add", json!({ "type": "sheetMetalFormed", "params": { "formPartStudio": form, "locations": pt, "targetFaces": [top] } })).unwrap();
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    assert!((volume(&mut ctx) - before - 100.0).abs() < 1e-6, "the form's block, as thick as the sheet (1 mm), not its own 3 mm: {before} -> {}", volume(&mut ctx));
}

/// My sheet metal forms (live 2497): every Part Studio of every document in the folder and its subfolders, at its
/// latest version, must be a form, or the library isn't made; Update builds it again (V2), clear unsets it.
fn forms_library(ctx: &mut Ctx, dir: &std::path::Path) {
    let forms = dir.join("uip forms");
    std::fs::create_dir_all(forms.join("uip forms sub")).unwrap();
    form_document(&forms.join("uip form A.noshape"));
    form_document(&forms.join("uip forms sub").join("uip form B.noshape"));
    let plain = forms.join("uip plain.noshape");
    call(&mut Ctx::new(), "document_create", json!({ "path": plain.to_str().unwrap() })).unwrap();
    assert_eq!(call(ctx, "documents_forms_library", json!({})).unwrap()["forms_library"], Value::Null);

    let set = json!({ "library": "sheet_metal_forms", "folder": forms.to_str().unwrap() });
    let e = call(ctx, "documents_set_library", set.clone()).unwrap_err();
    let element = "Element \"Part Studio 1\" in document \"uip plain\" at its latest version \"Start\" failed validation";
    assert_eq!(e.message, format!("Failed to update the library\n\n{element}: \"The Part Studio must be driven by a length configuration variable with FeatureScript ID \"thickness\"\". Fix and create a new version.\n\n{element}: \"The Part Studio should have at least one solid body marked as positive or negative\". Fix and create a new version."));
    assert_eq!(call(ctx, "documents_forms_library", json!({})).unwrap()["forms_library"], Value::Null, "a failed set makes no library");

    std::fs::remove_file(&plain).unwrap();
    let l = call(ctx, "documents_set_library", set).unwrap();
    assert_eq!(l["libraries"]["sheet_metal_forms"], forms.to_str().unwrap(), "{l}");
    let lib = call(ctx, "documents_forms_library", json!({})).unwrap()["forms_library"].clone();
    assert_eq!(lib["name"], format!("{} sheet metal forms", noshape_store::local_user_name()), "{lib}");
    assert_eq!(lib["version"], "V1", "{lib}");
    let docs = lib["documents"].as_array().unwrap();
    let shape: Vec<(Value, Value, Value, usize)> = docs.iter().map(|d| (d["folders"].clone(), d["name"].clone(), d["version"].clone(), d["forms"].as_array().unwrap().len())).collect();
    assert_eq!(shape, vec![(json!([]), json!("uip form A"), json!("V1"), 1), (json!(["uip forms sub"]), json!("uip form B"), json!("V1"), 1)], "{lib}");
    assert_eq!(docs[0]["forms"][0]["name"], "Part Studio 1", "{lib}");
    assert!(docs[0]["forms"][0]["value"].as_str().unwrap().starts_with("ext:"), "a form is its Part Studio at the library's version: {lib}");
    library_form_on_a_plate(&dir.join("plate.noshape"), docs[0]["forms"][0]["value"].as_str().unwrap());

    // Nothing changed: "already up to date" (live 2508), the library as it was; a new version of a document builds V2.
    let u = call(ctx, "documents_set_library", json!({ "library": "sheet_metal_forms", "update": true })).unwrap();
    assert_eq!((u["up_to_date"].clone(), u["forms_library"]["version"].clone()), (json!(true), json!("V1")), "{u}");
    let mut a = Ctx::new();
    call(&mut a, "document_open", json!({ "path": forms.join("uip form A.noshape").to_str().unwrap() })).unwrap();
    call(&mut a, "document_set_units", json!({ "length_unit": "in" })).unwrap();
    call(&mut a, "history_create_version", json!({ "name": "V2" })).unwrap();
    drop(a);
    let u = call(ctx, "documents_set_library", json!({ "library": "sheet_metal_forms", "update": true })).unwrap();
    assert_eq!((u["up_to_date"].clone(), u["forms_library"]["version"].clone(), u["forms_library"]["documents"][0]["version"].clone()), (json!(false), json!("V2"), json!("V2")), "{u}");
    call(ctx, "documents_set_library", json!({ "library": "sheet_metal_forms", "clear": true })).unwrap();
    assert_eq!(call(ctx, "documents_forms_library", json!({})).unwrap()["forms_library"], Value::Null);
    let e = call(ctx, "documents_set_library", json!({ "library": "sheet_metal_forms", "update": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("update"), "{e:?}");
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

/// A frame profile document: Part Studio 1's sketch "Profile", a 25.4 mm square round the origin, marked by a Frame
/// Tag when `tagged`, versioned "V1" (live 2530's "uip frame A" / "uip frame plain").
fn frame_document(path: &std::path::Path, tagged: bool) {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() })).unwrap();
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" })).unwrap()["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-12.7, -12.7], [12.7, 12.7]] })).unwrap();
    if tagged {
        call(&mut ctx, "feature_add", json!({ "type": "tag", "params": { "tagPurpose": "FRAME", "sketch": [format!("sketch:{sk}")] } })).unwrap();
    }
    call(&mut ctx, "history_create_version", json!({ "name": "V1" })).unwrap();
}

/// My frame profiles library (live 2530): an untagged Part Studio doesn't stop it; every Part Studio is a profile
/// (its tagged sketch, else its first), the subfolder's too; a Frame built with one sweeps that square; unset, there's
/// none.
fn frames_library(ctx: &mut Ctx, dir: &std::path::Path) {
    let frames = dir.join("uip frames");
    std::fs::create_dir_all(frames.join("uip frames sub")).unwrap();
    frame_document(&frames.join("uip frame A.noshape"), true);
    frame_document(&frames.join("uip frame plain.noshape"), false);
    frame_document(&frames.join("uip frames sub").join("uip frame B.noshape"), true);
    let l = call(ctx, "documents_set_library", json!({ "library": "frame_profiles", "folder": frames.to_str().unwrap() })).unwrap();
    assert_eq!(l["frames_library"]["name"], format!("{} frame profiles", noshape_store::local_user_name()), "{l}");
    let lib = call(ctx, "documents_frames_library", json!({})).unwrap()["frames_library"].clone();
    let docs = lib["documents"].as_array().unwrap();
    let shape: Vec<(Value, Value)> = docs.iter().map(|d| (d["folders"].clone(), d["name"].clone())).collect();
    assert_eq!(shape, vec![(json!([]), json!("uip frame A")), (json!([]), json!("uip frame plain")), (json!(["uip frames sub"]), json!("uip frame B"))], "{lib}");
    assert!(docs.iter().all(|d| d["forms"][0]["value"].as_str().is_some_and(|v| v.starts_with("ext:") && v.contains("#F"))), "each Part Studio's profile sketch: {lib}");
    // A Frame along a 200 mm line with the library's first profile: the 25.4 mm square swept.
    let mut user = Ctx::new();
    call(&mut user, "document_create", json!({ "path": dir.join("uses frames.noshape").to_str().unwrap() })).unwrap();
    let sk = call(&mut user, "sketch_create", json!({ "plane": "plane:front" })).unwrap()["sketch"].as_str().unwrap().to_string();
    let r = call(&mut user, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[0, 0], [200, 0]] })).unwrap();
    let line = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "line").unwrap()["id"].clone();
    for d in docs {
        let f = call(&mut user, "feature_add", json!({ "type": "frame", "params": { "profileSketch": d["forms"][0]["value"], "selections": [line.clone()] } })).unwrap();
        assert_eq!(f["feature_status"]["status"], "Ok", "{}: {f}", d["name"]);
        let v = call(&mut user, "query_parts", json!({})).unwrap()["parts"].as_array().unwrap().last().unwrap()["volume"].as_f64().unwrap();
        assert!((v - 25.4 * 25.4 * 200.0).abs() < 1.0, "{}: volume {v}", d["name"]);
        call(&mut user, "feature_delete", json!({ "features": [f["feature"].clone()] })).unwrap();
    }
    call(ctx, "documents_set_library", json!({ "library": "frame_profiles", "clear": true })).unwrap();
    assert_eq!(call(ctx, "documents_frames_library", json!({})).unwrap()["frames_library"], Value::Null);
}

