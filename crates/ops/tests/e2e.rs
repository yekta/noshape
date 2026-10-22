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

/// A pin in a block: two parts inserted into an assembly, the block fixed,
/// the pin mated (Revolute) into the block's hole.
#[test]
fn assembly_with_mates() {
    let mut ctx = Ctx::new();
    let path = scratch("asm.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    // Block 40 x 40 x 20 with a Ø10 through hole.
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Block" }));
    let s1 = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s1, "tool": "corner_rectangle", "points": [[-20, -20], [20, 20]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": s1, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s1 }));
    let plate = st["regions"].as_array().unwrap().iter().find(|r| r["holes"] == 1).unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [plate], "depth": "20 mm" } }));
    // Pin Ø10 x 50, off to the side.
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Pin" }));
    let s2 = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "circle", "points": [[60, 0], [65, 0]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s2 }));
    let disc = st["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [disc], "depth": "50 mm", "operationType": "NEW" } }));
    let parts = call(&mut ctx, "query_parts", json!({}));
    assert_eq!(parts["parts"].as_array().unwrap().len(), 2, "{parts}");
    let (block, pin) = (parts["parts"][0]["id"].as_str().unwrap().to_string(), parts["parts"][1]["id"].as_str().unwrap().to_string());

    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": block }));
    let block_i = b["instance"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": pin }));
    let pin_i = p["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_set_state", json!({ "instances": [block_i], "fixed": true }));

    // The hole's cylindrical face and the pin's cylindrical face.
    let hole = call(&mut ctx, "assembly_entities", json!({ "instance": block_i, "kind": "faces", "surface": "cylinder" }));
    let hole_face = hole["faces"][0]["id"].as_str().unwrap().to_string();
    let pinf = call(&mut ctx, "assembly_entities", json!({ "instance": pin_i, "kind": "faces", "surface": "cylinder" }));
    let pin_face = pinf["faces"][0]["id"].as_str().unwrap().to_string();
    let m = call(&mut ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": pin_i, "entity": pin_face }, "b": { "instance": block_i, "entity": hole_face } }));
    assert_eq!(m["mate_ok"], true, "{m}");
    assert_eq!(m["name"], "Revolute 1");
    // The pin moved onto the hole's axis (x = y = 0) and keeps one rotation.
    let list = call(&mut ctx, "assembly_list", json!({}));
    let pin_now = list["instances"].as_array().unwrap().iter().find(|i| i["id"] == pin_i).unwrap().clone();
    let pin_faces = call(&mut ctx, "assembly_entities", json!({ "instance": pin_i, "kind": "faces", "surface": "cylinder" }));
    let axis = &pin_faces["faces"][0]["axis"]["origin"];
    assert!(num(&axis[0]).abs() < 1e-3 && num(&axis[1]).abs() < 1e-3, "pin axis {axis}, instance {pin_now}");
    assert_eq!(list["dof"], 1, "{list}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Insert : Revolute 1");
}

/// A configured block: a List input sets the extrude depth, a Checkbox
/// suppresses the hole, a configuration Variable drives the width.
#[test]
fn configurations() {
    let mut ctx = Ctx::new();
    let path = scratch("config.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let v = call(&mut ctx, "config_add_input", json!({ "kind": "variable", "name": "w", "value": "40 mm", "range": ["10 mm", "100 mm"] }));
    let w = v["input"].as_str().unwrap().to_string();
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let s1 = sk["sketch"].as_str().unwrap().to_string();
    let rect = call(&mut ctx, "sketch_draw", json!({ "sketch": s1, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
    let bottom = rect["created"].as_array().unwrap().iter().find(|e| e["type"] == "line" && num(&e["start"][1]).abs() < 1e-6 && num(&e["end"][1]).abs() < 1e-6).unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_dimension", json!({ "sketch": s1, "kind": "length", "entities": [bottom], "value": "#w" }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s1 }));
    let region = st["regions"][0]["id"].as_str().unwrap().to_string();
    let ext = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let ext = ext["feature"].as_str().unwrap().to_string();
    let volume = |ctx: &mut Ctx| num(&call(ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    assert!((volume(&mut ctx) - 40.0 * 20.0 * 10.0).abs() < 1e-3);

    // List "Size": Default (10 mm) and Large (30 mm).
    let l = call(&mut ctx, "config_add_input", json!({ "kind": "list", "name": "Size" }));
    let size = l["input"].as_str().unwrap().to_string();
    call(&mut ctx, "config_add_option", json!({ "input": "Size", "name": "Large" }));
    call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "depth", "input": size }));
    call(&mut ctx, "config_set_value", json!({ "feature": ext, "param": "depth", "configuration": "Large", "value": "30 mm" }));
    assert!((volume(&mut ctx) - 40.0 * 20.0 * 10.0).abs() < 1e-3, "default configuration unchanged");
    let act = call(&mut ctx, "config_activate", json!({ "values": { "Size": "Large", "w": "50 mm" } }));
    assert!(act["regeneration"]["failed"].as_array().is_none_or(|f| f.is_empty()), "{act}");
    assert!((volume(&mut ctx) - 50.0 * 20.0 * 30.0).abs() < 1e-3, "{act}");
    // Out of range: the Configurations box's message; nothing changes.
    let e = try_call(&mut ctx, "config_activate", json!({ "values": { "w": "500 mm" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("values.w"));
    assert!(e.message.contains("between 10 and 100"), "{}", e.message);
    // Editing the feature in the Large configuration edits Large's value.
    call(&mut ctx, "feature_edit", json!({ "feature": ext, "params": { "depth": "25 mm" } }));
    assert!((volume(&mut ctx) - 50.0 * 20.0 * 25.0).abs() < 1e-3);
    let list = call(&mut ctx, "config_list", json!({}));
    let depth = list["configured"].as_array().unwrap().iter().find(|c| c["param"] == "depth").unwrap().clone();
    let default_id = list["inputs"].as_array().unwrap().iter().find(|i| i["id"] == size).unwrap()["configurations"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(depth["values"][&default_id], "10 mm", "{list}");
    assert_eq!(list["active"][&w], "50 mm", "{list}");

    // A Checkbox suppressing the extrude.
    let c = call(&mut ctx, "config_add_input", json!({ "kind": "checkbox", "name": "Hide block" }));
    let cb = c["input"].as_str().unwrap().to_string();
    call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "suppressed", "input": cb }));
    call(&mut ctx, "config_set_value", json!({ "feature": ext, "param": "suppressed", "configuration": true, "value": true }));
    call(&mut ctx, "config_activate", json!({ "values": { "Hide block": true } }));
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 0);
    call(&mut ctx, "config_activate", json!({ "reset": true }));
    assert!((volume(&mut ctx) - 40.0 * 20.0 * 10.0).abs() < 1e-3);
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Part Studio 1 :: Edit configured value");
}

// ------------------------------------------------------------ sketch edit tools

fn created<'a>(v: &'a Value, ty: &str) -> Vec<&'a Value> {
    v["created"].as_array().unwrap_or_else(|| panic!("no created: {v}")).iter().filter(|e| e["type"] == ty).collect()
}

fn id_of(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn near(a: &Value, x: f64, y: f64) -> bool {
    (num(&a[0]) - x).abs() < 1e-6 && (num(&a[1]) - y).abs() < 1e-6
}

/// Entities of a sketch, by type.
fn entities(ctx: &mut Ctx, sketch: &str, ty: &str) -> Vec<Value> {
    call(ctx, "sketch_get", json!({ "sketch": sketch }))["entities"].as_array().unwrap().iter().filter(|e| e["type"] == ty).cloned().collect()
}

fn new_sketch(ctx: &mut Ctx, file: &str, plane: &str) -> String {
    let path = scratch(file);
    call(ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(ctx, "sketch_create", json!({ "plane": plane }))["sketch"].as_str().unwrap().to_string()
}

#[test]
fn sketch_trim_extend_split() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "trim.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[-10, 0], [10, 0]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[0, -10], [0, 10]] }));
    // Trim the right half of the horizontal line at the crossing.
    call(&mut ctx, "sketch_trim", json!({ "sketch": sk, "points": [[5, 0]] }));
    let lines = entities(&mut ctx, &sk, "line");
    assert!(lines.iter().any(|l| (near(&l["start"], -10.0, 0.0) && near(&l["end"], 0.0, 0.0)) || (near(&l["end"], -10.0, 0.0) && near(&l["start"], 0.0, 0.0))), "{lines:?}");
    assert!(!lines.iter().any(|l| near(&l["start"], 10.0, 0.0) || near(&l["end"], 10.0, 0.0)), "{lines:?}");
    // Split the vertical line at y = 5: two pieces.
    call(&mut ctx, "sketch_split", json!({ "sketch": sk, "points": [[0, 5]] }));
    let vertical = entities(&mut ctx, &sk, "line").into_iter().filter(|l| num(&l["start"][0]).abs() < 1e-9 && num(&l["end"][0]).abs() < 1e-9).count();
    assert_eq!(vertical, 2);
    // Extend a short horizontal line to the vertical one.
    let c = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[-10, 6], [-4, 6]] }));
    let c = id_of(created(&c, "line")[0]);
    call(&mut ctx, "sketch_extend", json!({ "sketch": sk, "points": [[-4.5, 6]], "entities": [c] }));
    let ext = entities(&mut ctx, &sk, "line").into_iter().find(|l| l["id"] == c.as_str()).unwrap();
    assert!(near(&ext["end"], 0.0, 6.0) || near(&ext["start"], 0.0, 6.0), "{ext}");
    // A drag path trims what it crosses: a lone line is deleted.
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[5, -5], [5, 5]] }));
    let d = id_of(created(&d, "line")[0]);
    call(&mut ctx, "sketch_trim", json!({ "sketch": sk, "path": [[3, 3], [7, 3]] }));
    assert!(!entities(&mut ctx, &sk, "line").iter().any(|l| l["id"] == d.as_str()));
    // One edit per call, described as the UI does.
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Part Studio 1 :: Edit : Sketch 1");
    // Errors: a click on nothing.
    let e = try_call(&mut ctx, "sketch_trim", json!({ "sketch": sk, "points": [[50, 50]] })).unwrap_err();
    assert_eq!(e.code, "invalid_argument");
}

#[test]
fn sketch_fillet_chamfer_offset() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "fillet.noshape", "plane:top");
    let rect = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
    let corner = |x: f64, y: f64| id_of(created(&rect, "point").into_iter().find(|p| near(&p["at"], x, y)).unwrap_or_else(|| panic!("no corner {x},{y}: {rect}")));
    let (c1, c2, c3) = (corner(40.0, 0.0), corner(40.0, 20.0), corner(0.0, 20.0));
    let f = call(&mut ctx, "sketch_fillet", json!({ "sketch": sk, "entities": [c1, c2], "radius": "4 mm" }));
    assert_eq!(created(&f, "arc").len(), 2, "{f}");
    // The first fillet is dimensioned, the second equal to it (one session).
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let cons = st["constraints"].as_array().unwrap();
    let radius: Vec<&Value> = cons.iter().filter(|c| c["type"] == "Radius").collect();
    assert_eq!(radius.len(), 1, "{cons:?}");
    assert_eq!(radius[0]["expression"], "4 mm");
    assert!(cons.iter().any(|c| c["type"] == "Equal"));
    let ch = call(&mut ctx, "sketch_chamfer", json!({ "sketch": sk, "entities": [c3], "distance": 3 }));
    assert_eq!(created(&ch, "line").len(), 1, "{ch}");
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let area = num(&st["regions"][0]["area"]);
    let expect = 800.0 - 2.0 * (16.0 - 4.0 * std::f64::consts::PI) - 4.5;
    assert!((area - expect).abs() < 1e-3, "area {area} vs {expect}");
    // Offset a circle outward 2 mm (toward a point outside it).
    let circle = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[60, 10], [65, 10]] }));
    let circle = id_of(created(&circle, "circle")[0]);
    let o = call(&mut ctx, "sketch_offset", json!({ "sketch": sk, "entities": [circle], "distance": "2 mm", "toward": [60, 30] }));
    let r = num(&created(&o, "circle")[0]["radius"]);
    assert!((r - 7.0).abs() < 1e-6, "{o}");
    let o = call(&mut ctx, "sketch_offset", json!({ "sketch": sk, "entities": [circle], "distance": "2 mm", "toward": [60, 10] }));
    assert!((num(&created(&o, "circle")[0]["radius"]) - 3.0).abs() < 1e-6, "{o}");
    // A point given as a curve to fillet with no partner is an explicit error.
    let line = id_of(created(&rect, "line")[0]);
    let e = try_call(&mut ctx, "sketch_fillet", json!({ "sketch": sk, "entities": [line] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("entities"), "{}", e.to_json());
}

#[test]
fn sketch_mirror_patterns_transform() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "pattern.noshape", "plane:top");
    let axis = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[0, -20], [0, 20]], "construction": true }));
    let axis = id_of(created(&axis, "line")[0]);
    let c1 = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[10, 0], [12, 0]] }));
    let c1 = id_of(created(&c1, "circle")[0]);
    let m = call(&mut ctx, "sketch_mirror", json!({ "sketch": sk, "entities": [c1], "axis": axis }));
    assert!(near(&created(&m, "circle")[0]["center"], -10.0, 0.0), "{m}");
    let lp = call(&mut ctx, "sketch_linear_pattern", json!({ "sketch": sk, "entities": [c1], "count": 3, "distance": "10 mm", "angle": "90 deg" }));
    let made = created(&lp, "circle");
    assert_eq!(made.len(), 2, "{lp}");
    assert!(made.iter().any(|c| near(&c["center"], 10.0, 20.0)), "{lp}");
    let pid = lp["pattern"].as_u64().unwrap();
    // Edit the pattern: 4 instances.
    call(&mut ctx, "sketch_linear_pattern", json!({ "sketch": sk, "pattern": pid, "count": 4 }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    assert_eq!(st["patterns"][0]["count"], 4, "{}", st["patterns"]);
    assert!(entities(&mut ctx, &sk, "circle").iter().any(|c| near(&c["center"], 10.0, 30.0)));
    // Circular pattern of a circle about the origin: 4 instances.
    let c2 = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[30, 0], [31, 0]] }));
    let c2 = id_of(created(&c2, "circle")[0]);
    let cp = call(&mut ctx, "sketch_circular_pattern", json!({ "sketch": sk, "entities": [c2], "count": 4, "center": "origin" }));
    let made = created(&cp, "circle");
    assert_eq!(made.len(), 3, "{cp}");
    assert!(made.iter().any(|c| near(&c["center"], -30.0, 0.0)) && made.iter().any(|c| near(&c["center"], 0.0, 30.0)), "{cp}");
    // Transform: move a circle 5 mm along X.
    let c3 = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[50, -40], [51, -40]] }));
    let c3 = id_of(created(&c3, "circle")[0]);
    call(&mut ctx, "sketch_transform", json!({ "sketch": sk, "entities": [c3], "translate": [5, 0] }));
    let moved = entities(&mut ctx, &sk, "circle").into_iter().find(|c| c["id"] == c3.as_str()).unwrap();
    assert!(near(&moved["center"], 55.0, -40.0), "{moved}");
}

#[test]
fn sketch_use_intersection_and_model_references() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "use.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-10, -10], [10, 10]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let top = call(&mut ctx, "query_entities", json!({ "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].as_str().unwrap().to_string();
    // Use the top face: its four edges, linked to the model.
    let s2 = call(&mut ctx, "sketch_create", json!({ "plane": top.clone() }))["sketch"].as_str().unwrap().to_string();
    let used = call(&mut ctx, "sketch_use", json!({ "sketch": s2, "references": [top.clone()] }));
    let lines = created(&used, "line");
    assert_eq!(lines.len(), 4, "{used}");
    assert!(lines.iter().all(|l| l["external"] == true), "{used}");
    // The Front plane crosses the top face along a line at Z = 10.
    let s3 = call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }))["sketch"].as_str().unwrap().to_string();
    let inter = call(&mut ctx, "sketch_intersection", json!({ "sketch": s3, "face": top }));
    let l = created(&inter, "line");
    assert_eq!(l.len(), 1, "{inter}");
    assert!((num(&l[0]["start"][1]) - 10.0).abs() < 1e-6 && (num(&l[0]["end"][1]) - 10.0).abs() < 1e-6, "{inter}");
    // A dimension to a model edge: the edge's projection stands in.
    let c = call(&mut ctx, "sketch_draw", json!({ "sketch": s3, "tool": "circle", "points": [[0, 20], [2, 20]] }));
    let center = c["created"].as_array().unwrap().iter().find(|e| e["type"] == "circle").unwrap()["center_point"].as_str().unwrap().to_string();
    let edges = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "line" }));
    let edge = edges["edges"].as_array().unwrap().iter().find(|e| num(&e["direction"][0]).abs() > 0.99 && (num(&e["start"][2]) - 10.0).abs() < 1e-6).unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_dimension", json!({ "sketch": s3, "kind": "distance", "entities": [center, edge], "value": "15 mm" }));
    let circle = entities(&mut ctx, &s3, "circle").into_iter().next().unwrap();
    assert!((num(&circle["center"][1]) - 25.0).abs() < 1e-6, "{circle}");
}

#[test]
fn sketch_text_and_imports() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "text.noshape", "plane:top");
    let t = call(&mut ctx, "sketch_text", json!({ "sketch": sk, "text": "AB", "corner": [0, 0], "height": "5 mm" }));
    assert!(!t["created"].as_array().unwrap().is_empty(), "{t}");
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    assert_eq!(st["texts"][0]["text"], "AB", "{st}");
    let line = st["texts"][0]["box_lines"][0].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_text", json!({ "sketch": sk, "edit": line, "text": "ABC", "bold": true }));
    assert_eq!(call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["texts"][0]["text"], "ABC");
    // Insert DXF: two lines, a circle, an arc and a polyline in millimeters.
    let dxf = scratch("sample.dxf");
    std::fs::write(&dxf, "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nLINE\n8\n0\n10\n10\n20\n0\n11\n10\n21\n5\n0\nCIRCLE\n8\n0\n10\n5\n20\n5\n40\n2\n0\nARC\n10\n0\n20\n0\n40\n3\n50\n0\n51\n90\n0\nENDSEC\n0\nEOF\n").unwrap();
    let d = call(&mut ctx, "sketch_insert_dxf", json!({ "sketch": sk, "file": dxf.to_str().unwrap(), "use_origin": true }));
    let circles = created(&d, "circle");
    assert_eq!(circles.len(), 1, "{d}");
    assert!(near(&circles[0]["center"], 5.0, 5.0) && (num(&circles[0]["radius"]) - 2.0).abs() < 1e-9, "{d}");
    // Insert image: a 4 x 2 px PNG keeps its 2:1 aspect.
    let png = noshape_scene::png::encode_png(vec![255; 4 * 2 * 4], 4, 2, 4, 2).unwrap();
    let img = scratch("pic.png");
    std::fs::write(&img, png).unwrap();
    call(&mut ctx, "sketch_insert_image", json!({ "sketch": sk, "file": img.to_str().unwrap(), "points": [[0, -20], [8, -16]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    assert_eq!(st["images"][0]["name"], "pic.png", "{st}");
    let e = try_call(&mut ctx, "sketch_insert_image", json!({ "sketch": sk, "file": "/nonexistent.png", "points": [[0, 0], [1, 1]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("file"));
}

#[test]
fn workspace_protection() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "protect.noshape", "plane:top");
    let main = call(&mut ctx, "document_info", json!({}))["branch"].as_str().unwrap().to_string();
    let info = call(&mut ctx, "document_protect", json!({}));
    assert_eq!(info["protected"], true, "{info}");
    // Model edits are refused with the `protected` code and Onshape's reason.
    let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] })).unwrap_err();
    assert_eq!(e.code, "protected", "{}", e.to_json());
    assert!(e.message.contains("merge them into this one"), "{}", e.message);
    assert_eq!(try_call(&mut ctx, "tab_create", json!({ "kind": "partstudio" })).unwrap_err().code, "protected");
    assert_eq!(try_call(&mut ctx, "history_undo", json!({})).unwrap_err().code, "protected");
    assert_eq!(try_call(&mut ctx, "document_set_units", json!({ "length_unit": "in" })).unwrap_err().code, "protected");
    // Versions and branches are allowed; the branch takes edits.
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    call(&mut ctx, "history_create_branch", json!({ "name": "B1" }));
    assert_eq!(call(&mut ctx, "document_info", json!({}))["protected"], false);
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    // Merging into the protected workspace is allowed.
    call(&mut ctx, "history_switch_branch", json!({ "branch": main }));
    assert_eq!(call(&mut ctx, "document_info", json!({}))["protected"], true);
    call(&mut ctx, "history_merge", json!({ "branch": "B1" }));
    assert_eq!(entities(&mut ctx, &sk, "circle").len(), 1);
    // Unprotected, it takes edits again.
    let info = call(&mut ctx, "document_unprotect", json!({}));
    assert_eq!(info["protected"], false);
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[20, 0], [25, 0]] }));
}

// ------------------------------------------------------------ drawings

/// A block 80 x 40 x 20 with a Ø10 through hole at its center.
fn block_with_hole(ctx: &mut Ctx, file: &str) {
    let sk = new_sketch(ctx, file, "plane:top");
    call(ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-40, -20], [40, 20]] }));
    call(ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    let st = call(ctx, "sketch_get", json!({ "sketch": sk }));
    let plate = st["regions"].as_array().unwrap().iter().find(|r| r["holes"] == 1).unwrap()["id"].as_str().unwrap().to_string();
    let ex = call(ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [plate], "depth": "20 mm" } }));
    assert_eq!(ex["feature_status"]["status"], "Ok", "{ex}");
}

#[test]
fn drawing_views_and_dimensions() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing.noshape");
    let t = call(&mut ctx, "drawing_templates", json!({}));
    assert!(t["templates"].as_array().unwrap().iter().any(|r| r["name"] == "ISO A3"));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    assert!(tab.ends_with("Drawing 1"), "{d}");
    let views = d["views"].as_array().unwrap();
    assert!(views.len() >= 3, "suggested views: {d}");
    // The view that shows the hole as a circle (the top view).
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let (view, circle) = g["views"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|v| v["edges"].as_array().and_then(|es| es.iter().find(|e| e["type"] == "circle").map(|c| (v.clone(), c.clone()))))
        .unwrap_or_else(|| panic!("no view shows the hole: {g}"));
    let (cx, cy, r) = (num(&circle["center"][0]), num(&circle["center"][1]), num(&circle["radius"]));
    let dia = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "diameter", "picks": [[cx + r, cy]], "place": [cx + r + 15.0, cy + 15.0] }));
    assert!(dia["text"].as_str().unwrap().contains("10"), "{dia}");
    // The smart Dimension on the 80 mm edge of the same view.
    let long = view["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "line")
        .max_by(|a, b| {
            let l = |e: &Value| ((num(&e["b"][0]) - num(&e["a"][0])).powi(2) + (num(&e["b"][1]) - num(&e["a"][1])).powi(2)).sqrt();
            l(a).total_cmp(&l(b))
        })
        .unwrap()
        .clone();
    let mid = [(num(&long["a"][0]) + num(&long["b"][0])) / 2.0, (num(&long["a"][1]) + num(&long["b"][1])) / 2.0];
    let lin = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": [mid], "place": [mid[0], mid[1] + 20.0] }));
    assert!(lin["text"].as_str().unwrap().contains("80"), "{lin}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert!(hist["history"][0]["description"].as_str().unwrap().ends_with(":: Linear dimension"), "{hist}");
    // An isometric base view, and a view projected from it.
    let iv = call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "orientation": "isometric", "position": [330, 230], "scale": 1.0 }));
    let base = iv["view"].as_u64().unwrap();
    let pv = call(&mut ctx, "drawing_projected_view", json!({ "drawing": tab, "parent": base, "position": [330, 120] }));
    let pv_id = pv["view"].as_u64().unwrap();
    let after = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
    let pvj = after["views"].as_array().unwrap().iter().find(|v| v["id"] == pv_id).unwrap().clone();
    assert_eq!(pvj["parent"], base, "{pvj}");
    // Errors: a click on nothing.
    let e = try_call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": [[5, 5]], "place": [10, 10] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("picks[0]"), "{}", e.to_json());
    // Export: one PDF page, and DXF.
    let pdf = scratch("drawing.pdf");
    let x = call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "PDF", "file": pdf.to_str().unwrap() }));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"), "{x}");
    let dxf = scratch("drawing.dxf");
    call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "DXF", "file": dxf.to_str().unwrap() }));
    // Release 14 (live's default) is written in the drawing's code page, not UTF-8.
    assert!(std::fs::read(&dxf).unwrap().windows(8).any(|w| w == b"ENTITIES"));
}

#[test]
fn analysis_tools() {
    let mut ctx = Ctx::new();
    // A 20 x 20 x 10 block and a Ø10 x 30 cylinder through it (two parts).
    let sk = new_sketch(&mut ctx, "analysis.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "center_rectangle", "points": [[0, 0], [10, 10]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let s2 = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    let disk = call(&mut ctx, "sketch_get", json!({ "sketch": s2 }))["regions"][0]["id"].as_str().unwrap().to_string();
    let ex = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [disk], "depth": "30 mm", "operationType": "NEW" } }));
    assert_eq!(ex["feature_status"]["status"], "Ok", "{ex}");
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 2);
    // Interference: the cylinder inside the block, π·5²·10.
    let i = call(&mut ctx, "analysis_interference", json!({}));
    let list = i["interferences"].as_array().unwrap();
    assert_eq!(list.len(), 1, "{i}");
    let v = num(&list[0]["volume"]);
    assert!((v - std::f64::consts::PI * 25.0 * 10.0).abs() < 1.0, "overlap {v}");
    // Curvature: the cylinder's circular edges have radius 5.
    let circles = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "circle" }));
    let edge = circles["edges"][0]["id"].as_str().unwrap().to_string();
    let c = call(&mut ctx, "analysis_curvature", json!({ "entities": [edge] }));
    assert!((num(&c["min_radius"]["radius"]) - 5.0).abs() < 0.05, "{c}");
    // Dihedral: a vertical block edge is 90°.
    let lines = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "line" }));
    let vertical = lines["edges"].as_array().unwrap().iter().find(|e| num(&e["direction"][2]).abs() > 0.99).unwrap()["id"].as_str().unwrap().to_string();
    let d = call(&mut ctx, "analysis_dihedral", json!({ "entities": [vertical] }));
    assert!((num(&d["min_deg"]) - 90.0).abs() < 0.5 && (num(&d["max_deg"]) - 90.0).abs() < 0.5, "{d}");
}

#[test]
fn import_step_round_trip() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "import.noshape");
    let v0 = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    let step = scratch("block.step");
    call(&mut ctx, "export_parts", json!({ "format": "STEP", "file": step.to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
    let tabs = r["tabs"].as_array().unwrap();
    assert!(tabs.iter().any(|t| t["name"] == "block" && t["kind"] == "Part Studio"), "{r}");
    assert!(tabs.iter().any(|t| t["name"] == "block.step"), "the file tab in CAD Imports: {r}");
    assert_eq!(r["parts"].as_array().unwrap().len(), 1, "{r}");
    let v1 = num(&call(&mut ctx, "query_parts", json!({ "tab": "block" }))["parts"][0]["volume"]);
    assert!((v1 - v0).abs() < 1.0, "imported volume {v1} vs {v0}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Import block.step");
    // Errors name the file.
    let e = try_call(&mut ctx, "import_files", json!({ "files": ["/tmp/nope.xyz"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("files[0]"));
}

#[test]
fn materials_and_appearances() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "material.noshape");
    let lib = call(&mut ctx, "material_list", json!({ "search": "6061" }));
    let al = lib["materials"].as_array().unwrap().first().unwrap_or_else(|| panic!("no 6061: {lib}")).clone();
    let name = al["name"].as_str().unwrap().to_string();
    let density = num(&al["density_kg_m3"]);
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "part_set_material", json!({ "parts": [part.clone()], "material": name.clone() }));
    // Mass = volume x density.
    let m = call(&mut ctx, "query_mass_properties", json!({}));
    let vol_m3 = num(&m["volume"]) * 1e-9;
    assert!((num(&m["mass_kg"]) - vol_m3 * density).abs() < 1e-5, "{m}");
    // Colors: the part, and a face of it.
    call(&mut ctx, "part_set_appearance", json!({ "parts": [part.clone()], "color": "#3b61b4", "transparency": 0.25 }));
    let top = call(&mut ctx, "query_entities", json!({ "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "part_set_appearance", json!({ "faces": [top], "color": [255, 0, 0] }));
    let a = call(&mut ctx, "part_appearances", json!({}));
    assert_eq!(a["parts"][0]["material"], name.as_str());
    assert_eq!(a["parts"][0]["appearance"]["color"], "#3b61b4", "{a}");
    assert_eq!(a["parts"][0]["appearance"]["transparency"], 0.25);
    assert_eq!(a["appearances"][0]["color"], "#ff0000", "{a}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Part Studio 1 :: Edit appearance");
    let after = call(&mut ctx, "part_remove_appearance", json!({ "parts": [part], "appearances": [0] }));
    assert!(after["appearances"].as_array().unwrap().is_empty() && after["parts"][0].get("appearance").is_none(), "{after}");
}

/// A block with a pin through it: a linear pattern of the pin along a
/// block edge (insert, edit, delete), the BOM and interference detection.
#[test]
fn assembly_patterns_bom_interference() {
    let mut ctx = Ctx::new();
    let path = scratch("asm_pattern.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Block" }));
    let s1 = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s1, "tool": "corner_rectangle", "points": [[-20, -20], [20, 20]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s1 }));
    let r = st["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [r], "depth": "20 mm" } }));
    // A Ø10 x 50 pin standing in the block (x 5..15).
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Pin" }));
    let s2 = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "circle", "points": [[10, 0], [15, 0]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s2 }));
    let disc = st["regions"].as_array().unwrap().iter().find(|r| num(&r["area"]) < 100.0).unwrap_or(&st["regions"][0])["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [disc], "depth": "50 mm", "operationType": "NEW" } }));
    let parts = call(&mut ctx, "query_parts", json!({}));
    assert_eq!(parts["parts"].as_array().unwrap().len(), 2, "{parts}");
    let (block, pin) = (parts["parts"][0]["id"].as_str().unwrap().to_string(), parts["parts"][1]["id"].as_str().unwrap().to_string());
    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": block }));
    let block_i = b["instance"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": pin }));
    let pin_i = p["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_set_state", json!({ "instances": [block_i], "fixed": true }));

    // The pin overlaps the block: π·5²·20 mm³.
    let i = call(&mut ctx, "assembly_interference", json!({}));
    assert_eq!(i["count"], 1, "{i}");
    assert!((num(&i["interferences"][0]["volume"]) - std::f64::consts::PI * 25.0 * 20.0).abs() < 1.0, "{i}");

    // Two pins along an X edge of the block, 60 mm apart.
    let edges = call(&mut ctx, "assembly_entities", json!({ "instance": block_i, "kind": "edges" }));
    let x_edge = edges["edges"].as_array().unwrap().iter().find(|e| e["curve"] == "line" && num(&e["direction"][0]).abs() > 0.99).unwrap()["id"].as_str().unwrap().to_string();
    let pat = call(&mut ctx, "assembly_pattern", json!({ "kind": "linear", "seeds": [pin_i], "direction": { "instance": block_i, "entity": x_edge }, "distance": "60 mm" }));
    assert_eq!(pat["name"], "Linear pattern 1", "{pat}");
    assert_eq!(pat["created"].as_array().unwrap().len(), 1, "{pat}");
    assert_eq!(pat["instances"].as_array().unwrap().len(), 3);
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Insert : Linear pattern 1");
    // The copy sits 60 mm from the seed along X, clear of the block.
    let list = call(&mut ctx, "assembly_list", json!({}));
    let copy = pat["created"][0].as_str().unwrap();
    let at = |id: &str| list["instances"].as_array().unwrap().iter().find(|i| i["id"] == id).unwrap()["translation"].clone();
    assert!(((num(&at(copy)[0]) - num(&at(&pin_i)[0])).abs() - 60.0).abs() < 1e-6, "{list}");
    assert_eq!(call(&mut ctx, "assembly_interference", json!({}))["count"], 1);

    // Edit: three pins.
    let pat = call(&mut ctx, "assembly_pattern", json!({ "edit": "Linear pattern 1", "count": 3 }));
    assert_eq!(pat["instances"].as_array().unwrap().len(), 4, "{pat}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Edit : Linear pattern 1");

    // BOM: one block, three pins.
    let csv = scratch("asm_pattern_bom.csv");
    let bom = call(&mut ctx, "assembly_bom", json!({ "csv": csv.to_str().unwrap() }));
    let qty: Vec<u64> = bom["rows"].as_array().unwrap().iter().map(|r| r["quantity"].as_u64().unwrap()).collect();
    assert_eq!(qty, vec![1, 3], "{bom}");
    assert!(std::fs::read_to_string(&csv).unwrap().starts_with("Item,Quantity"));
    let s = call(&mut ctx, "assembly_bom_settings", json!({ "columns": ["Item", "Quantity", "Material", "Part number", "Description"] }));
    assert_eq!(s["columns"], json!(["Item", "Quantity", "Material", "Part number", "Description"]), "{s}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 3 }));
    let descs: Vec<&str> = hist["history"].as_array().unwrap().iter().map(|h| h["description"].as_str().unwrap()).collect();
    assert!(descs.contains(&"Assembly 1 :: Add column Material"), "{descs:?}");

    // Deleting the pattern takes its instances with it.
    let d = call(&mut ctx, "assembly_delete", json!({ "mates": ["Linear pattern 1"] }));
    assert_eq!(d["instances"].as_array().unwrap().len(), 2, "{d}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Delete : Linear pattern 1");
}

/// Groups, a mate connector used by a mate, a gear relation, rename,
/// suppress / hide, and replace instance.
#[test]
fn assembly_groups_relations_connectors() {
    let mut ctx = Ctx::new();
    let path = scratch("asm_features.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Block" }));
    let s1 = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s1, "tool": "corner_rectangle", "points": [[-20, -20], [20, 20]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": s1, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s1 }));
    let plate = st["regions"].as_array().unwrap().iter().find(|r| r["holes"] == 1).unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [plate], "depth": "20 mm" } }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Pin" }));
    let s2 = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "circle", "points": [[60, 0], [65, 0]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s2 }));
    let disc = st["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [disc], "depth": "50 mm", "operationType": "NEW" } }));
    let parts = call(&mut ctx, "query_parts", json!({}));
    let (block, pin) = (parts["parts"][0]["id"].as_str().unwrap().to_string(), parts["parts"][1]["id"].as_str().unwrap().to_string());
    let ins = |ctx: &mut Ctx, part: &str| call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": part }))["instance"].as_str().unwrap().to_string();
    let (b1, b2, p1, p2) = (ins(&mut ctx, &block), ins(&mut ctx, &block), ins(&mut ctx, &pin), ins(&mut ctx, &pin));
    call(&mut ctx, "assembly_set_state", json!({ "instances": [b1], "fixed": true }));
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();

    // Group the two blocks.
    let g = call(&mut ctx, "assembly_group", json!({ "instances": [b1, b2] }));
    assert_eq!(g["name"], "Group 1", "{g}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Insert : Group");
    let e = try_call(&mut ctx, "assembly_group", json!({ "instances": [b1] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("instances"));

    // Pin 1 turns in the hole; pin 2 turns about a mate connector on the block's top face.
    let cyl = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "surface": "cylinder" }))["faces"][0]["id"].as_str().unwrap().to_string();
    let (hole, pin1_face, pin2_face) = (cyl(&mut ctx, &b1), cyl(&mut ctx, &p1), cyl(&mut ctx, &p2));
    let m1 = call(&mut ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": p1, "entity": pin1_face }, "b": { "instance": b1, "entity": hole } }));
    assert_eq!(m1["mate_ok"], true, "{m1}");
    let faces = call(&mut ctx, "assembly_entities", json!({ "instance": b1, "kind": "faces" }));
    let top = faces["faces"].as_array().unwrap().iter().find(|f| f["surface"] == "plane" && num(&f["normal"][2]) > 0.99).unwrap()["id"].as_str().unwrap().to_string();
    let mc = call(&mut ctx, "assembly_mate_connector", json!({ "on": { "instance": b1, "entity": top }, "offset": { "x": "10 mm" } }));
    assert_eq!(mc["name"], "Mate connector 1", "{mc}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Insert : Mate connector 1");
    let m2 = call(&mut ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": p2, "entity": pin2_face }, "b": { "connector": "Mate connector 1" } }));
    assert_eq!(m2["mate_ok"], true, "{m2}");

    // A gear relation between the two revolutes.
    let r = call(&mut ctx, "assembly_relation", json!({ "type": "gear", "mates": ["Revolute 1", "Revolute 2"], "ratio": 2 }));
    assert_eq!(r["name"], "Gear 1", "{r}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Insert : Gear relation");
    let e = try_call(&mut ctx, "assembly_relation", json!({ "type": "gear", "mates": ["Revolute 1", "Revolute 1"] })).unwrap_err();
    assert!(e.message.contains("two different mates"), "{e:?}");

    // Rename, suppress, hide.
    call(&mut ctx, "assembly_rename", json!({ "instance": p2, "name": "Pin B" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Rename : Pin B");
    call(&mut ctx, "assembly_set_feature_state", json!({ "features": ["Gear 1"], "suppressed": true }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Suppress mate");
    call(&mut ctx, "assembly_set_feature_state", json!({ "features": ["Mate connector 1"], "hidden": true }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Hide mate connector");

    // Replace the second block with a pin.
    let rp = call(&mut ctx, "assembly_replace", json!({ "instance": b2, "source": "Part Studio 1", "part": pin }));
    assert!(rp["message"].as_str().unwrap().starts_with("Replaced 1 instance(s)"), "{rp}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Replace instance : Part 2");
}

/// A custom feature written in a Feature Studio, committed, then used in a
/// Part Studio.
#[test]
fn feature_studio_custom_feature() {
    let mut ctx = Ctx::new();
    let path = scratch("featurestudio.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
    let src = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

annotation { "Feature Type Name" : "Block" }
export const block = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Width" }
        isLength(definition.width, { (millimeter) : [1, 30, 1000] } as LengthBoundSpec);
    }
    {
        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(definition.width, 20 * millimeter, 10 * millimeter) });
    });
"#;
    let e = call(&mut ctx, "featurestudio_edit", json!({ "source": src }));
    assert_eq!(e["uncommitted"], true, "{e}");
    assert!(e["notices"].as_array().unwrap().iter().all(|n| n["severity"] != "error"), "{e}");
    assert_eq!(e["features"][0]["display_name"], "Block", "{e}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert!(hist["history"][0]["description"].as_str().unwrap().ends_with("Edit Feature Studio"), "{hist}");
    let c = call(&mut ctx, "featurestudio_commit", json!({}));
    assert_eq!(c["committed"], 1, "{c}");
    let ty = c["feature_types"].as_array().unwrap().iter().find(|t| t["name"] == "Block").unwrap_or_else(|| panic!("{c}"))["type"].as_str().unwrap().to_string();
    let f = call(&mut ctx, "feature_add", json!({ "tab": "Part Studio 1", "type": ty, "params": { "width": "40 mm" } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    let v = num(&call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"][0]["volume"]);
    assert!((v - 8000.0).abs() < 1e-3, "volume {v}");
    // A syntax error is reported with its line.
    let bad = call(&mut ctx, "featurestudio_edit", json!({ "source": "FeatureScript 3095;\nexport const x = ;\n" }));
    let err = bad["notices"].as_array().unwrap().iter().find(|n| n["severity"] == "error").unwrap_or_else(|| panic!("{bad}")).clone();
    assert_eq!(err["line"], 2, "{bad}");
}

/// Every format the Import dialog takes that we can write: the block is
/// exported in each and imported back. B-rep formats keep the volume;
/// mesh formats come back as one mesh part.
#[test]
fn import_every_format() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "import_all.noshape");
    let v0 = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    // (export format, volume kept)
    // (Rhino needs openNURBS in the build; the app's CI builds have it.)
    let formats = [("Parasolid", true), ("ACIS", true), ("STEP", true), ("IGES", true), ("JT", true), ("PVZ", false), ("glTF", false), ("GLB", false), ("OBJ", false), ("3MF", false), ("STL", false)];
    for (i, (f, brep)) in formats.iter().enumerate() {
        let file = scratch(&format!("import_all_{i}"));
        // STL has no units: written and read in millimeters.
        let e = call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": f, "file": file.to_str().unwrap(), "units": "Millimeter" }));
        let path = e["file"].as_str().unwrap().to_string();
        assert!(std::path::Path::new(&path).extension().is_some(), "{f}: the format's extension is added: {e}");
        // STL in millimeters; OBJ is written (and read by default) in meters.
        let mut args = json!({ "files": [path.clone()] });
        if *f == "STL" {
            args["mesh_unit"] = json!("Millimeter");
        }
        let r = call(&mut ctx, "import_files", args);
        let studio = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").unwrap_or_else(|| panic!("{f}: no Part Studio: {r}"))["name"].as_str().unwrap().to_string();
        assert!(r["errors"].is_null(), "{f}: {r}");
        assert_eq!(r["parts"].as_array().unwrap().len(), 1, "{f}: {r}");
        // B-rep: the same solid; meshes: the tessellated solid (within 0.1 %).
        let v = num(&call(&mut ctx, "query_parts", json!({ "tab": studio }))["parts"][0]["volume"]);
        let tol = if *brep { 1e-6 } else { 1e-3 };
        assert!((v - v0).abs() < v0 * tol, "{f}: imported volume {v} vs {v0}");
        let name = std::path::Path::new(&path).file_name().unwrap().to_str().unwrap().to_string();
        assert!(r["tabs"].as_array().unwrap().iter().any(|t| t["name"] == name.as_str()), "{f}: the file tab: {r}");
    }
}

/// Import options: combine into one Part Studio, no file tabs, mesh units;
/// and the errors name the file.
#[test]
fn import_options_and_errors() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "import_opts.noshape");
    let step = scratch("opts_block.step");
    let stl = scratch("opts_block.stl");
    call(&mut ctx, "export_parts", json!({ "format": "STEP", "file": step.to_str().unwrap() }));
    call(&mut ctx, "export_parts", json!({ "format": "STL", "file": stl.to_str().unwrap(), "units": "Millimeter" }));
    let before = call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().len();
    let r = call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap(), stl.to_str().unwrap()], "combine": true, "keep_files": false, "mesh_unit": "Millimeter" }));
    let tabs = r["tabs"].as_array().unwrap();
    assert_eq!(tabs.len(), 1, "combine, no file tabs: {r}");
    assert_eq!(r["parts"].as_array().unwrap().len(), 2, "{r}");
    assert_eq!(call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().len(), before + 1);
    // The STL in millimeters has the block's size.
    let parts = call(&mut ctx, "query_parts", json!({ "tab": tabs[0]["name"].as_str().unwrap() }));
    assert!(parts["parts"].as_array().unwrap().iter().all(|p| { let b = &p["bounding_box"]; b.is_null() || (num(&b["max"][0]) - num(&b["min"][0]) - 80.0).abs() < 0.1 }), "{parts}");
    // A zip without a native root assembly, an unknown type, a missing file.
    // Two parts exported as individual files: a zip of STEP files, no native assembly.
    let sk = call(&mut ctx, "sketch_create", json!({ "tab": "Part Studio 1", "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "tab": "Part Studio 1", "sketch": sk, "tool": "circle", "points": [[80, 0], [85, 0]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "tab": "Part Studio 1", "sketch": sk }));
    let disc = st["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "tab": "Part Studio 1", "type": "extrude", "params": { "entities": [disc], "depth": "10 mm", "operationType": "NEW" } }));
    let z = call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP", "file": scratch("not_native").to_str().unwrap(), "individual": true }));
    let zip = std::path::PathBuf::from(z["file"].as_str().unwrap());
    assert_eq!(zip.extension().and_then(|e| e.to_str()), Some("zip"), "{z}");
    // Onshape keeps such a zip as a file tab and reports it as not translated.
    let r = call(&mut ctx, "import_files", json!({ "files": [zip.to_str().unwrap()] }));
    assert!(r["errors"][0].as_str().unwrap().contains("was not translated"), "{r}");
    let e = try_call(&mut ctx, "import_files", json!({ "files": ["/tmp/nope.xyz"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("files[0]"));
    let e = try_call(&mut ctx, "import_files", json!({ "files": ["/tmp/definitely-missing.step"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("files[0]"));
}

/// Coincident on a corner drawn on the origin (and the origin), or on the
/// point two lines share: already coincident, nothing added, no edit.
#[test]
fn coincident_on_one_point_is_a_no_op() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "coincident.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [30, 20]] }));
    let lines: Vec<&Value> = r["created"].as_array().unwrap().iter().filter(|e| e["type"] == "line").collect();
    let corner = lines[0]["start_point"].as_str().unwrap().to_string();
    let n0 = call(&mut ctx, "history_list", json!({}))["history"].as_array().unwrap().len();
    let v = call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "coincident", "entities": [corner, "origin"] }));
    assert_eq!(v["added"], json!([]), "{v}");
    assert!(v["message"].as_str().unwrap().contains("already coincident"), "{v}");
    let shared = lines[3]["end_point"].as_str().unwrap().to_string();
    let v = call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "coincident", "entities": [corner, shared] }));
    assert_eq!(v["added"], json!([]), "{v}");
    assert_eq!(call(&mut ctx, "history_list", json!({}))["history"].as_array().unwrap().len(), n0, "no edit");
    // Other duplicates are one selected entity, as in the UI.
    let e = try_call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "equal", "entities": [lines[0]["id"], lines[0]["id"]] })).unwrap_err();
    assert!(e.message.contains("Select 2 entities"), "{e:?}");
    // A dimension without a value keeps the measured one (the value box accepted unchanged).
    let d = call(&mut ctx, "sketch_dimension", json!({ "sketch": sk, "kind": "length", "entities": [lines[0]["id"]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let dim = g["constraints"].as_array().unwrap().iter().find(|c| c["id"] == d["dimension"]).unwrap().clone();
    assert_eq!(dim["driving"], true, "{dim}");
    let shown = dim["value"].as_str().unwrap();
    assert!(shown.starts_with("30") || shown.starts_with("20"), "{dim}");
}

/// Named positions driven by a revolute mate, and an exploded view.
#[test]
fn assembly_named_positions_and_exploded_views() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "asm_positions.noshape");
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Pin" }));
    let s2 = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "circle", "points": [[60, 0], [65, 0]] }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": s2 }));
    let disc = st["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [disc], "depth": "50 mm", "operationType": "NEW" } }));
    let parts = call(&mut ctx, "query_parts", json!({}));
    let (block, pin) = (parts["parts"][0]["id"].as_str().unwrap().to_string(), parts["parts"][1]["id"].as_str().unwrap().to_string());
    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": block }))["instance"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": pin }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_set_state", json!({ "instances": [b], "fixed": true }));
    let cyl = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "surface": "cylinder" }))["faces"][0]["id"].as_str().unwrap().to_string();
    let (hole, pf) = (cyl(&mut ctx, &b), cyl(&mut ctx, &p));
    call(&mut ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": p, "entity": pf }, "b": { "instance": b, "entity": hole } }));
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();

    let e = try_call(&mut ctx, "assembly_named_position_drivers", json!({ "drivers": [{ "mate": "Revolute 1", "dof": "tz" }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("drivers[0].dof"));
    call(&mut ctx, "assembly_named_position_drivers", json!({ "drivers": [{ "mate": "Revolute 1", "dof": "rz" }] }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Driving mate selector");
    let np = call(&mut ctx, "assembly_named_position", json!({ "action": "add" }));
    assert_eq!(np["named_positions"][0]["name"], "Named position 1", "{np}");
    call(&mut ctx, "assembly_named_position", json!({ "action": "rename", "position": "1", "name": "Closed" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Rename named position : Closed");
    call(&mut ctx, "assembly_named_position", json!({ "action": "duplicate", "position": "Closed" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Duplicate named position : Closed (1)");
    let np = call(&mut ctx, "assembly_named_position", json!({ "action": "set_value", "position": "Closed (1)", "mate": "Revolute 1", "dof": "rz", "value": "90 deg" }));
    let v = np["named_positions"].as_array().unwrap().iter().find(|p| p["name"] == "Closed (1)").unwrap()["mate_values"][0]["rz_deg"].clone();
    assert!((num(&v) - 90.0).abs() < 1e-6, "{np}");
    call(&mut ctx, "assembly_named_position", json!({ "action": "apply", "position": "Closed (1)" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Apply named position : Closed (1)");
    call(&mut ctx, "assembly_named_position", json!({ "action": "move_up", "position": "Closed (1)" }));
    let np = call(&mut ctx, "assembly_named_position", json!({ "action": "delete", "position": "Closed" }));
    assert_eq!(np["named_positions"].as_array().unwrap().len(), 1, "{np}");

    // Exploded view: lift the pin 30 mm, then rename and delete.
    let ev = call(&mut ctx, "assembly_exploded_view", json!({ "action": "add" }));
    assert_eq!(ev["exploded_views"][0]["name"], "Explode 1", "{ev}");
    let st = call(&mut ctx, "assembly_explode_step", json!({ "view": "Explode 1", "instances": [p], "direction": [0, 0, 1], "distance": "30 mm" }));
    assert_eq!(st["steps"], 1, "{st}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Edit : Explode step");
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "rename", "view": "Explode 1", "name": "Pin out" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Rename : Pin out");
    let ev = call(&mut ctx, "assembly_exploded_view", json!({ "action": "delete", "view": "Pin out" }));
    assert_eq!(ev["exploded_views"].as_array().unwrap().len(), 0, "{ev}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Delete : Pin out");
}

#[test]
fn drawing_section_detail_views_and_annotations() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_tools.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    // The top view: it shows the hole as a circle.
    let (top, circle) = g["views"].as_array().unwrap().iter().find_map(|v| v["edges"].as_array().and_then(|es| es.iter().find(|e| e["type"] == "circle").map(|c| (v.clone(), c.clone())))).unwrap();
    let top_id = top["id"].as_u64().unwrap();
    let (cx, cy, r) = (num(&circle["center"][0]), num(&circle["center"][1]), num(&circle["radius"]));
    let lines: Vec<Value> = top["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line").cloned().collect();
    let p = |e: &Value, k: &str| [num(&e[k][0]), num(&e[k][1])];
    // A quarter along an edge: off the section's cutting line through the hole.
    let mid = |e: &Value| [num(&e["a"][0]) * 0.75 + num(&e["b"][0]) * 0.25, num(&e["a"][1]) * 0.75 + num(&e["b"][1]) * 0.25];
    let horizontal: Vec<&Value> = lines.iter().filter(|e| (num(&e["a"][1]) - num(&e["b"][1])).abs() < 1e-6).collect();
    let vertical: Vec<&Value> = lines.iter().filter(|e| (num(&e["a"][0]) - num(&e["b"][0])).abs() < 1e-6).collect();
    assert!(horizontal.len() >= 2 && !vertical.is_empty(), "{top}");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let views_of = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"].as_array().unwrap().clone();

    // Section A-A through the hole, vertical cutting line (the dialog's default).
    let max_x = num(&top["bounds"]["max"][0]);
    let s = call(&mut ctx, "drawing_section_view", json!({ "drawing": tab, "parent": top_id, "point": [cx, cy], "position": [max_x + 60.0, cy] }));
    let sid = s["view"].as_u64().unwrap();
    let sv = views_of(&mut ctx).into_iter().find(|v| v["id"] == sid).unwrap();
    assert_eq!(sv["kind"], "section", "{sv}");
    assert_eq!(sv["parent"], top_id, "{sv}");
    assert!(last(&mut ctx).ends_with(":: Section view"));

    // Detail view of the hole, twice the parent's scale.
    let dv = call(&mut ctx, "drawing_detail_view", json!({ "drawing": tab, "parent": top_id, "center": [cx, cy], "radius": r * 1.5, "position": [380, 60] }));
    let did = dv["view"].as_u64().unwrap();
    let dj = views_of(&mut ctx).into_iter().find(|v| v["id"] == did).unwrap();
    assert_eq!(dj["kind"], "detail", "{dj}");
    assert!((num(&dj["scale"]) - 2.0 * num(&top["scale"])).abs() < 1e-9, "{dj}");
    assert!(last(&mut ctx).ends_with(":: Detail view"));
    // No breaks on detail views (Onshape's message).
    let e = try_call(&mut ctx, "drawing_break_view", json!({ "drawing": tab, "view": did, "from": [370, 60], "to": [390, 60] })).unwrap_err();
    assert!(e.message.contains("Break views can't be created from detail views"), "{e:?}");

    // Auxiliary view normal to a straight edge of the top view.
    let edge = mid(horizontal[0]);
    let below = if edge[1] < cy { -40.0 } else { 40.0 };
    let av = call(&mut ctx, "drawing_auxiliary_view", json!({ "drawing": tab, "edge": edge, "position": [edge[0], edge[1] + below] }));
    let aid = av["view"].as_u64().unwrap();
    assert_eq!(views_of(&mut ctx).into_iter().find(|v| v["id"] == aid).unwrap()["kind"], "auxiliary");
    assert!(last(&mut ctx).ends_with(":: Auxiliary view"));

    // Centerlines (the 2 point one diagonal, off the edges clicked next), virtual sharp.
    let (h0, h1) = (horizontal[0], horizontal[1]);
    let c2 = call(&mut ctx, "drawing_centerline", json!({ "drawing": tab, "kind": "two_point", "picks": [p(h0, "a"), p(h1, "b")] }));
    assert!(c2["annotation"].as_u64().is_some());
    assert!(last(&mut ctx).ends_with(":: Centerline"));
    call(&mut ctx, "drawing_centerline", json!({ "drawing": tab, "kind": "edge_to_edge", "picks": [mid(h0), mid(h1)] }));
    assert!(last(&mut ctx).ends_with(":: Edge to edge centerline"));
    call(&mut ctx, "drawing_virtual_sharp", json!({ "drawing": tab, "picks": [mid(h0), mid(vertical[0])] }));
    assert!(last(&mut ctx).ends_with(":: Virtual sharp"));
    let e = try_call(&mut ctx, "drawing_virtual_sharp", json!({ "drawing": tab, "picks": [mid(h0), mid(h1)] })).unwrap_err();
    assert!(e.message.contains("The edges are parallel"), "{e:?}");

    // Break and crop the top view, then a BOM table.
    let (min_x, mid_y) = (num(&top["bounds"]["min"][0]), cy);
    let w = max_x - min_x;
    call(&mut ctx, "drawing_break_view", json!({ "drawing": tab, "view": top_id, "from": [min_x + w * 0.1, mid_y], "to": [min_x + w * 0.2, mid_y] }));
    assert!(last(&mut ctx).ends_with(":: Break view"));
    // Another projected view (the top view has a break now: no crops on break views).
    let other = views_of(&mut ctx).into_iter().find(|v| v["kind"] == "projected" && v["id"] != top_id).unwrap();
    let (ox, oy) = (num(&other["position"][0]), num(&other["position"][1]));
    call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": other["id"], "shape": "circle", "from": [ox, oy], "to": [ox + 10.0, oy] }));
    assert!(last(&mut ctx).ends_with(":: Crop view"));
    let t = call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "bom", "view": top_id, "position": [20, 280] }));
    assert!(t["table"].as_u64().is_some(), "{t}");
    assert!(last(&mut ctx).ends_with(":: BOM table"));
    let e = try_call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "hole", "view": top_id, "position": [20, 200] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("origin"), "{e:?}");
}

#[test]
fn sketch_draw_many_shapes_in_one_edit() {
    let mut ctx = Ctx::new();
    // Separate calls and one `shapes` call give the same sketch (inference too:
    // the line starts on the rectangle's corner).
    let shapes = json!([
        { "tool": "corner_rectangle", "points": [[0, 0], [30, 20]] },
        { "tool": "line", "points": [[30, 20], [50, 40]] },
        { "tool": "circle", "points": [[10, 10], [13, 10]], "construction": true },
    ]);
    let a = new_sketch(&mut ctx, "many.noshape", "plane:top");
    for sh in shapes.as_array().unwrap() {
        let mut args = sh.clone();
        args["sketch"] = json!(a);
        call(&mut ctx, "sketch_draw", args);
    }
    let b = call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }))["sketch"].as_str().unwrap().to_string();
    let n0 = call(&mut ctx, "history_list", json!({}))["history"].as_array().unwrap().len();
    call(&mut ctx, "sketch_draw", json!({ "sketch": b, "shapes": shapes }));
    assert_eq!(call(&mut ctx, "history_list", json!({}))["history"].as_array().unwrap().len(), n0 + 1, "one edit");
    let (ga, gb) = (call(&mut ctx, "sketch_get", json!({ "sketch": a })), call(&mut ctx, "sketch_get", json!({ "sketch": b })));
    let kinds = |g: &Value, k: &str| {
        let mut v: Vec<String> = g[k].as_array().unwrap().iter().map(|e| format!("{}{}", e["type"], e["construction"])).collect();
        v.sort();
        v
    };
    assert_eq!(kinds(&ga, "entities"), kinds(&gb, "entities"));
    assert_eq!(kinds(&ga, "constraints"), kinds(&gb, "constraints"));
    // 400 circles at once (perf: one at a time took 50 s).
    let c = call(&mut ctx, "sketch_create", json!({ "plane": "plane:right" }))["sketch"].as_str().unwrap().to_string();
    let grid: Vec<Value> = (0..400).map(|i| json!({ "tool": "circle", "points": [[(i % 20) as f64 * 10.0, (i / 20) as f64 * 10.0], [(i % 20) as f64 * 10.0 + 3.0, (i / 20) as f64 * 10.0]] })).collect();
    let t0 = std::time::Instant::now();
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": c, "shapes": grid }));
    assert_eq!(r["created"].as_array().unwrap().iter().filter(|e| e["type"] == "circle").count(), 400, "{}", t0.elapsed().as_secs_f64());
    // An error names the shape.
    let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": c, "shapes": [{ "tool": "circle", "points": [[0, 0], [1, 0]] }, { "tool": "nope", "points": [[0, 0]] }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("shapes[1].tool"), "{e:?}");
}

#[test]
fn variable_studio_table() {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("vs.noshape").to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "variablestudio", "name": "vs all" }));
    let n0 = call(&mut ctx, "history_list", json!({}))["history"].as_array().unwrap().len();
    // Add: the table's last row, then its cells (one edit each).
    let r = call(&mut ctx, "variable_create", json!({ "tab": "vs all", "name": "width", "value": 40, "description": "Plate width" }));
    let w = r["variables"][0].clone();
    assert_eq!((w["name"].as_str(), w["type"].as_str(), w["expression"].as_str(), w["description"].as_str()), (Some("#width"), Some("Length"), Some("40 mm"), Some("Plate width")), "{r}");
    let hist = call(&mut ctx, "history_list", json!({}))["history"].as_array().unwrap().clone();
    assert_eq!(hist.len(), n0 + 3, "Add variable, Edit variable, Edit variable description");
    call(&mut ctx, "variable_create", json!({ "tab": "vs all", "name": "half", "value": "#width / 2" }));
    call(&mut ctx, "variable_create", json!({ "tab": "vs all", "name": "count", "kind": "NUMBER", "value": 3 }));
    let l = call(&mut ctx, "variable_list", json!({ "tab": "vs all" }));
    let rows = l["variables"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{l}");
    assert!(rows[1]["value"].as_str().unwrap().starts_with("20"), "{l}");
    assert_eq!((rows[2]["type"].as_str(), rows[2]["expression"].as_str()), (Some("Number"), Some("3")), "{l}");
    // Edit, rename (expressions keep their text, as the table), reorder, delete.
    call(&mut ctx, "variable_set", json!({ "tab": "vs all", "variable": "#width", "value": "60 mm", "kind": "LENGTH" }));
    let l = call(&mut ctx, "variable_list", json!({ "tab": "vs all" }));
    assert!(l["variables"][1]["value"].as_str().unwrap().starts_with("30"), "{l}");
    call(&mut ctx, "variable_set", json!({ "tab": "vs all", "variable": "count", "name": "n" }));
    let r = call(&mut ctx, "variable_move", json!({ "tab": "vs all", "variable": "n", "by": -1 }));
    assert_eq!(r["variables"][1]["name"], "#n", "{r}");
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Move variable up"));
    let r = call(&mut ctx, "variable_delete", json!({ "tab": "vs all", "variable": "n" }));
    assert_eq!(r["variables"].as_array().unwrap().len(), 2, "{r}");
    // Errors name the parameter.
    let e = try_call(&mut ctx, "variable_create", json!({ "tab": "vs all", "name": "2x", "value": 1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"), "{e:?}");
    let e = try_call(&mut ctx, "variable_create", json!({ "tab": "vs all", "name": "half", "value": 1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"), "{e:?}");
    let e = try_call(&mut ctx, "variable_set", json!({ "tab": "vs all", "variable": "nope", "value": 1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("variable"), "{e:?}");
}

#[test]
fn drawing_callouts_ordinates_and_datums() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_annot.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let (top, circle) = g["views"].as_array().unwrap().iter().find_map(|v| v["edges"].as_array().and_then(|es| es.iter().find(|e| e["type"] == "circle").map(|c| (v.clone(), c.clone())))).unwrap();
    let (cx, cy, r) = (num(&circle["center"][0]), num(&circle["center"][1]), num(&circle["radius"]));
    let lines: Vec<Value> = top["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line" && (num(&e["a"][1]) - num(&e["b"][1])).abs() < 1e-6).cloned().collect();
    let p = |e: &Value, k: &str| [num(&e[k][0]), num(&e[k][1])];
    let quarter = |e: &Value| [num(&e["a"][0]) * 0.75 + num(&e["b"][0]) * 0.25, num(&e["a"][1]) * 0.75 + num(&e["b"][1]) * 0.25];
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let kind_of = |ctx: &mut Ctx, id: u64| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["annotations"].as_array().unwrap().iter().find(|a| a["id"] == id).map(|a| a["kind"].as_str().unwrap_or("").to_string()).unwrap_or_default();

    let h = call(&mut ctx, "drawing_hole_callout", json!({ "drawing": tab, "hole": [cx + r, cy], "position": [cx + r + 20.0, cy + 20.0] }));
    assert!(last(&mut ctx).ends_with(":: Hole callout"));
    assert_eq!(kind_of(&mut ctx, h["annotation"].as_u64().unwrap()), "HoleCallout");
    let e = try_call(&mut ctx, "drawing_hole_callout", json!({ "drawing": tab, "hole": [5, 5], "position": [20, 20] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("hole"), "{e:?}");

    let c = call(&mut ctx, "drawing_callout", json!({ "drawing": tab, "anchor": quarter(&lines[0]), "position": [40, 250], "text": "7", "border": "hexagon" }));
    assert_eq!(kind_of(&mut ctx, c["annotation"].as_u64().unwrap()), "Balloon");
    assert!(last(&mut ctx).ends_with(":: Callout"));
    call(&mut ctx, "drawing_callout", json!({ "drawing": tab, "position": [60, 250], "text": "free" }));
    assert!(last(&mut ctx).ends_with(":: Callout"));

    // Ordinates from the edge's left end, values along x.
    let (a0, b0) = (p(&lines[0], "a"), p(&lines[0], "b"));
    let o = call(&mut ctx, "drawing_ordinate", json!({ "drawing": tab, "origin": a0, "side": [a0[0], a0[1] - 15.0], "points": [b0, p(&lines[1], "b")] }));
    assert_eq!(kind_of(&mut ctx, o["annotation"].as_u64().unwrap()), "OrdinateGroup");
    assert!(last(&mut ctx).ends_with(":: Add to ordinate group"));

    // Max/min: the hole's far side to the edge's end.
    let mm = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "max_min", "picks": [[cx + r, cy], a0], "place": [cx, cy - 25.0] }));
    assert_eq!(kind_of(&mut ctx, mm["annotation"].as_u64().unwrap()), "MaxMinDimension");
    assert!(last(&mut ctx).ends_with(":: Maximum / minimum dimension"), "{}", last(&mut ctx));

    let cm = call(&mut ctx, "drawing_centermark", json!({ "drawing": tab, "holes": [[cx - r, cy]] }));
    assert_eq!(kind_of(&mut ctx, cm["annotations"][0].as_u64().unwrap()), "Centermark");
    assert!(last(&mut ctx).ends_with(":: Centermark"));

    let nt = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "BREAK ALL EDGES", "position": [40, 40], "leader": a0 }));
    assert_eq!(kind_of(&mut ctx, nt["annotation"].as_u64().unwrap()), "Note");
    assert!(last(&mut ctx).ends_with(":: Note"));

    let g = call(&mut ctx, "drawing_gdt", json!({ "drawing": tab, "position": [60, 60], "rows": [{ "symbol": "position", "before": "Ø", "tolerance": "0.1", "datums": ["A", "B"] }] }));
    assert_eq!(kind_of(&mut ctx, g["annotation"].as_u64().unwrap()), "Gdt");
    assert!(last(&mut ctx).ends_with(":: Geometric tolerance"));
    let e = try_call(&mut ctx, "drawing_gdt", json!({ "drawing": tab, "position": [60, 60], "rows": [{ "symbol": "wobble" }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("rows[0].symbol"), "{e:?}");

    let sf = call(&mut ctx, "drawing_surface_finish", json!({ "drawing": tab, "position": quarter(&lines[0]) }));
    assert_eq!(kind_of(&mut ctx, sf["annotation"].as_u64().unwrap()), "SurfaceFinish");
    assert!(last(&mut ctx).ends_with(":: Surface finish"));
    let w = call(&mut ctx, "drawing_weld", json!({ "drawing": tab, "target": a0, "position": [a0[0] - 20.0, a0[1] + 20.0], "spec": { "arrow": { "joint": "Fillet", "size": "6" }, "all_around": true } }));
    assert_eq!(kind_of(&mut ctx, w["annotation"].as_u64().unwrap()), "Weld");
    assert!(last(&mut ctx).ends_with(":: Weld symbol"));
    let e = try_call(&mut ctx, "drawing_weld", json!({ "drawing": tab, "target": a0, "position": [10, 10], "spec": { "arrow": { "joint": "Glue" } } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("spec"), "{e:?}");

    let dt = call(&mut ctx, "drawing_datum", json!({ "drawing": tab, "anchor": quarter(&lines[1]), "position": [quarter(&lines[1])[0], quarter(&lines[1])[1] + 15.0] }));
    assert_eq!(kind_of(&mut ctx, dt["annotation"].as_u64().unwrap()), "Datum");
    assert!(last(&mut ctx).ends_with(":: Datum"));
}
