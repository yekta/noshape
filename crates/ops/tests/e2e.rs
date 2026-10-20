//! End to end: a part built only through the agent API's operations (the
//! registry the CLI and MCP server use), checked by its geometry.

use noshape_ops::registry;
use noshape_ops::{Ctx, OpError};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    try_call(ctx, tool, args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json()))
}

fn try_call(ctx: &mut Ctx, tool: &str, args: Value) -> Result<Value, OpError> {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).map(|o| o.value)
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("noshape-ops-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    let _ = std::fs::remove_file(&p);
    p
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

#[test]
fn sketch_extrude_fillet_pattern_export_render() {
    let mut ctx = Ctx::new();
    let path = scratch("plate.noshape");
    let info = call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    assert_eq!(info["tabs"][0]["name"], "Part Studio 1");

    // Sketch: a 50 x 30 rectangle with a Ø10 hole, dimensioned.
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    assert_eq!(sk["name"], "Sketch 1");
    let rect = call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "corner_rectangle", "points": [[-25, -15], [25, 15]] }));
    let lines: Vec<String> = rect["created"].as_array().unwrap().iter().filter(|e| e["type"] == "line").map(|e| e["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(lines.len(), 4, "{rect}");
    let circle = call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "circle", "points": [[0, 0], [4, 0]] }));
    let circle_id = circle["created"].as_array().unwrap().iter().find(|e| e["type"] == "circle").unwrap()["id"].as_str().unwrap().to_string();
    // The rectangle's automatic constraints are the UI's: horizontal/vertical sides.
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }));
    let kinds: Vec<&str> = st["constraints"].as_array().unwrap().iter().map(|c| c["type"].as_str().unwrap()).collect();
    assert!(kinds.iter().filter(|k| **k == "Horizontal").count() >= 2 && kinds.iter().filter(|k| **k == "Vertical").count() >= 2, "{kinds:?}");
    // Ø10 by a diameter dimension; the bottom line 50 mm via an expression.
    let d = call(&mut ctx, "sketch_dimension", json!({ "sketch": sketch, "kind": "diameter", "entities": [circle_id], "value": "10 mm" }));
    assert!(d["dimension"].as_str().unwrap().starts_with("dim:"));
    let bottom = rect["created"].as_array().unwrap().iter().find(|e| e["type"] == "line" && num(&e["start"][1]) < -14.0 && num(&e["end"][1]) < -14.0).unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sketch, "kind": "length", "entities": [bottom], "value": "2 * 25 mm" }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }));
    let regions = st["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 2, "plate and hole regions: {st}");
    let plate = regions.iter().find(|r| r["holes"] == 1).unwrap()["id"].as_str().unwrap().to_string();
    let circle_dims: Vec<&Value> = st["constraints"].as_array().unwrap().iter().filter(|c| c["type"] == "Diameter").collect();
    assert_eq!(circle_dims[0]["value"], "Ø10", "{:?}", circle_dims);

    // Errors name the parameter.
    let e = try_call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "depth": "ten" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("params.depth"), "{}", e.to_json());
    let e = try_call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "endBound": "NOPE" } })).unwrap_err();
    assert!(e.message.contains("BLIND"), "{}", e.to_json());

    // Extrude the plate region 10 mm.
    let ex = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [plate], "depth": "10 mm" } }));
    assert_eq!(ex["feature_status"]["status"], "Ok", "{ex}");
    assert_eq!(ex["name"], "Extrude 1");
    let parts = call(&mut ctx, "query_parts", json!({}));
    assert_eq!(parts["parts"].as_array().unwrap().len(), 1);
    let v0 = num(&parts["parts"][0]["volume"]);
    let expect0 = 50.0 * 30.0 * 10.0 - std::f64::consts::PI * 25.0 * 10.0;
    assert!((v0 - expect0).abs() < 1.0, "volume {v0} vs {expect0}");
    assert_eq!(parts["parts"][0]["faces"], 7);

    // Fillet the four vertical outer edges 3 mm.
    let edges = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "line" }));
    let vertical: Vec<String> = edges["edges"].as_array().unwrap().iter().filter(|e| num(&e["direction"][2]).abs() > 0.99 && num(&e["start"][0]).abs() > 24.0).map(|e| e["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(vertical.len(), 4, "{edges}");
    let fi = call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": vertical, "radius": "3 mm" } }));
    assert_eq!(fi["feature_status"]["status"], "Ok", "{fi}");
    let parts = call(&mut ctx, "query_parts", json!({}));
    let v1 = num(&parts["parts"][0]["volume"]);
    let corner = 4.0 * (9.0 - std::f64::consts::PI * 9.0 / 4.0) * 10.0;
    assert!((v1 - (expect0 - corner)).abs() < 1.0, "filleted volume {v1}");
    assert_eq!(parts["parts"][0]["faces"], 11);

    // Pattern the part 3x along X, 60 mm apart.
    let part = parts["parts"][0]["id"].as_str().unwrap().to_string();
    let xedge = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "line" }))["edges"].as_array().unwrap().iter().find(|e| num(&e["direction"][0]).abs() > 0.99).unwrap()["id"].as_str().unwrap().to_string();
    let pa = call(&mut ctx, "feature_add", json!({ "type": "linearPattern", "params": { "patternType": "PART", "entities": [part], "directionOne": [xedge], "distance": "60 mm", "instanceCount": 3 } }));
    assert_eq!(pa["feature_status"]["status"], "Ok", "{pa}");
    let parts = call(&mut ctx, "query_parts", json!({}));
    assert_eq!(parts["parts"].as_array().unwrap().len(), 3, "{parts}");
    let mass = call(&mut ctx, "query_mass_properties", json!({}));
    assert!((num(&mass["volume"]) - 3.0 * v1).abs() < 3.0, "{mass}");

    // Feature list and history are the UI's.
    let list = call(&mut ctx, "feature_list", json!({}));
    let names: Vec<&str> = list["features"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Sketch 1", "Extrude 1", "Fillet 1", "Linear pattern 1"]);
    let hist = call(&mut ctx, "history_list", json!({ "limit": 3 }));
    assert_eq!(hist["history"][0]["description"], "Part Studio 1 :: Insert feature : Linear pattern 1");

    // Export STEP and STL; read the STEP back.
    let step = scratch("plate.step");
    let r = call(&mut ctx, "export_parts", json!({ "format": "STEP", "file": step.to_str().unwrap() }));
    assert_eq!(r["parts"], 3);
    let back = noshape_kernel::read_step(&step, noshape_kernel::LengthUnit::Meter).expect("read STEP");
    let vol: f64 = back.iter().map(|s| s.shape.volume().unwrap()).sum::<f64>() * 1e9;
    assert!((vol - 3.0 * v1).abs() < 5.0, "STEP volume {vol}");
    let stl = scratch("plate.stl");
    let r = call(&mut ctx, "export_parts", json!({ "format": "STL", "file": stl.to_str().unwrap() }));
    assert!(num(&r["bytes"]) > 1000.0);

    // Undo removes the pattern (one microversion), redo brings it back.
    call(&mut ctx, "history_undo", json!({}));
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 1);
    call(&mut ctx, "history_redo", json!({}));
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 3);

    // The file holds it all: a fresh session sees the same model.
    let mut ctx2 = Ctx::new();
    let parts2 = call(&mut ctx2, "query_parts", json!({ "doc": path.to_str().unwrap() }));
    assert_eq!(parts2["parts"].as_array().unwrap().len(), 3);

    // Render (skipped without any GPU adapter).
    let png = scratch("plate.png");
    match try_call(&mut ctx, "render_image", json!({ "view": "isometric", "width": 320, "height": 240, "file": png.to_str().unwrap() })) {
        Ok(v) => {
            assert!(num(&v["bytes"]) > 1000.0);
            let bytes = std::fs::read(&png).unwrap();
            assert_eq!(&bytes[1..4], b"PNG");
        }
        Err(e) if e.code == "render_failed" => eprintln!("render skipped: {}", e.message),
        Err(e) => panic!("render: {}", e.to_json()),
    }
}
