// fs temporary: time api benchmark F4 (extrude REMOVE through all on an imported bracket). Not committed.
use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};
use std::time::Instant;

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    let t = Instant::now();
    let v = registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value;
    eprintln!("CALL {tool}: {:?}", t.elapsed());
    v
}

#[test]
fn tmp_speed_imported_cut() {
    let dir = "/root/fs-scratch/spd";
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_open", json!({ "path": format!("{dir}/bracket.noshape") }));
    call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP", "file": format!("{dir}/bracket.step") }));
    let _ = std::fs::remove_file(format!("{dir}/imported.noshape"));
    call(&mut ctx, "document_create", json!({ "path": format!("{dir}/imported.noshape") }));
    let imp = call(&mut ctx, "import_files", json!({ "files": [format!("{dir}/bracket.step")] }));
    eprintln!("import {}", imp.to_string().chars().take(300).collect::<String>());
    let f = call(&mut ctx, "query_entities", json!({ "tab": "bracket", "kind": "faces", "normal": [0, 0, 1], "near": [0, 30, 12], "limit": 1 }));
    let face = f["faces"][0]["id"].as_str().unwrap().to_string();
    let sk = call(&mut ctx, "sketch_create", json!({ "tab": "bracket", "plane": face, "name": "Cut" }));
    eprintln!("sketch {sk}");
    call(&mut ctx, "sketch_draw", json!({ "tab": "bracket", "sketch": "Cut", "tool": "circle", "points": [[0, 20, 12], [5, 20, 12]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "tab": "bracket", "sketch": "Cut" }));
    let region = g["regions"][0]["id"].as_str().unwrap().to_string();
    for k in 0..2 {
        let t = Instant::now();
        let r = call(&mut ctx, "feature_add", json!({ "tab": "bracket", "type": "extrude", "params": { "entities": [region.clone()], "operationType": "REMOVE", "endBound": "THROUGH_ALL" } }));
        eprintln!("ADD {k}: {:?} {}", t.elapsed(), r.to_string().chars().take(200).collect::<String>());
        let id = r["feature"].as_str().unwrap().to_string();
        call(&mut ctx, "feature_delete", json!({ "tab": "bracket", "features": [id] }));
    }
}
