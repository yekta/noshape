//! Live qa/hobbyist-1125 "Phone stand" (qa FLOW-VAR-UNDERDEFINED-SKETCH-MOVES): a stand profile on Front drawn with
//! the line tool (its H/V and coincidents, the first point on the origin) and ONE dimension, the base's thickness #t
//! between l0 and l4, a sketch fillet R8 at (10, 5), extruded 35 mm. #t 5 -> 4 moves the whole lip down with l4 (l3
//! keeps its 10 mm, l1 shortens): 44230.708 -> 41430.708 mm3.

use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

fn line_between(sketch: &Value, a: [f64; 2], b: [f64; 2]) -> Value {
    let near = |p: &Value, q: [f64; 2]| (p[0].as_f64().unwrap() - q[0]).abs() < 1e-6 && (p[1].as_f64().unwrap() - q[1]).abs() < 1e-6;
    let lines = sketch["entities"].as_array().unwrap().iter().filter(|e| e["type"] == "line");
    lines
        .clone()
        .find(|e| (near(&e["start"], a) && near(&e["end"], b)) || (near(&e["start"], b) && near(&e["end"], a)))
        .unwrap_or_else(|| panic!("no line {a:?}-{b:?}"))["id"]
        .clone()
}

#[test]
fn a_thinner_base_moves_the_lip_down_with_it() {
    let dir = std::env::temp_dir().join(format!("noshape-stand-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stand.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap(), "length_unit": "mm" }));
    call(&mut ctx, "feature_add", json!({ "type": "assignVariable", "params": { "mode": "ASSIGNED", "variableType": "LENGTH", "name": "t", "lengthValue": "5 mm" } }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }))["sketch"].clone();
    let points = [[0, 0], [90, 0], [90, 15], [85, 15], [85, 5], [10, 5], [10, 80], [0, 80], [0, 0]];
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": points }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let (l0, l4) = (line_between(&g, [0.0, 0.0], [90.0, 0.0]), line_between(&g, [85.0, 5.0], [10.0, 5.0]));
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sk, "entities": [l0, l4], "value": "#t", "label": [50, -10] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let corner = g["entities"].as_array().unwrap().iter().find(|e| e["type"] == "point" && [&e["at"], &e["position"]].iter().any(|p| p[0] == 10.0 && p[1] == 5.0)).map(|e| e["id"].clone());
    call(&mut ctx, "sketch_fillet", json!({ "sketch": sk, "entities": [corner.expect("the (10, 5) corner")], "radius": "8 mm" }));
    let regions: Vec<Value> = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "endBound": "BLIND", "depth": "35 mm", "operationType": "NEW" } }));
    let volume = |ctx: &mut Ctx| call(ctx, "query_parts", json!({}))["parts"][0]["volume"].as_f64().unwrap();
    assert!((volume(&mut ctx) - 44230.708).abs() < 0.01, "#t 5: {}", volume(&mut ctx));
    call(&mut ctx, "feature_edit", json!({ "feature": "Variable 1", "params": { "lengthValue": "4 mm" } }));
    let v = volume(&mut ctx);
    assert!((v - 41430.708).abs() < 0.01, "#t 4: {v} (live 41430.708; 41605.708 = the lip left at y 15)");
}
