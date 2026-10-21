//! qa5 D13: a drawing's DRAWN name (the user who created it) is saved with
//! the drawing and survives closing and reopening the document, whoever
//! opens it later.

use noshape_model::document::TabContent;
use noshape_model::drawing::build::{SheetInfo, render_sheet_with};
use noshape_model::drawing::views::{ViewCache, regenerate_sources};
use noshape_ops::registry;
use noshape_ops::{Ctx, session::Session};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

#[test]
fn drawn_by_survives_save_and_reopen() {
    let dir = std::env::temp_dir().join(format!("noshape-drawn-by-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("drawn.noshape");
    let _ = std::fs::remove_file(&path);

    noshape_store::set_default_author(Some("Alice Drafter".into()));
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-20, -10], [20, 10]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let region = st["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "5 mm" } }));
    call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ANSI A" }));
    drop(ctx);

    // Reopened by someone else: DRAWN still shows the creator.
    noshape_store::set_default_author(Some("Bob Viewer".into()));
    let session = Session::open(&path).unwrap();
    let drawing = session.doc.tabs.iter().find_map(|t| if let TabContent::Drawing(d) = &t.content { Some(d.clone()) } else { None }).expect("drawing tab");
    assert_eq!(drawing.drawn_by.as_deref(), Some("Alice Drafter"));
    let sources = regenerate_sources(&session.doc, &drawing);
    let info = SheetInfo { user_name: "Bob Viewer".into(), ..Default::default() };
    let built = render_sheet_with(&drawing, 0, &sources, &ViewCache::new(), false, &info).unwrap();
    let texts: Vec<&str> = built.geometry.texts.iter().map(|t| t.text.as_str()).collect();
    assert!(texts.contains(&"ALICE DRAFTER"), "{texts:?}");
    assert!(!texts.contains(&"BOB VIEWER"), "{texts:?}");
    noshape_store::set_default_author(None);
}
