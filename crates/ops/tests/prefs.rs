//! The user's preferences through the agent API (a test config directory:
//! its own process, as it sets NOSHAPE_CONFIG_DIR).

use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Result<Value, noshape_ops::OpError> {
    registry::run(ctx, registry::by_tool_name(tool).expect("tool"), &args).map(|o| o.value)
}

#[test]
fn graphics_preferences_and_toolbar_read_set_and_kept() {
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
    let _ = std::fs::remove_dir_all(&dir);
}
