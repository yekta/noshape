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
    // The rectangle's automatic constraints are the UI's (live's): Perpendicular, 2 Parallel, Horizontal.
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }));
    let kinds: Vec<&str> = st["constraints"].as_array().unwrap().iter().map(|c| c["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["Perpendicular", "Parallel", "Parallel", "Horizontal"], "{kinds:?}");
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
    // Edit it as the mate dialog does: limits, an offset, the secondary axis.
    let e = call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "limits": { "rz": { "min": "-30 deg", "max": "45 deg" } }, "offset": { "z": "5 mm" }, "reorient_secondary": 1 }));
    assert_eq!(e["mate"], m["mate"], "{e}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Edit : Revolute 1");
    let list = call(&mut ctx, "assembly_list", json!({}));
    let mj = list["mates"].as_array().unwrap().iter().find(|x| x["name"] == "Revolute 1").unwrap().clone();
    assert_eq!(mj["limits"]["rz"], json!({ "min": "-30 deg", "max": "45 deg" }), "{mj}");
    assert_eq!(mj["offset"]["z"], "5 mm", "{mj}");
    assert_eq!(mj["reorient_secondary"], 1, "{mj}");
    // Mate menu › Reset: its rotation back to 0, one edit.
    call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "reset": true }));
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Reset : Revolute 1");
    assert_eq!(mj["type"], "revolute", "the type kept: {mj}");
    // Limits the mate type doesn't have are refused.
    let err = try_call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "limits": { "x": { "min": "0 mm" } } })).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("limits"), "{err:?}");
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

/// A new drawing knows the document state its views come from, so a model
/// change puts it out of date before it was ever edited.
#[test]
fn new_drawing_goes_out_of_date() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing-stale.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
    assert_eq!(g["out_of_date"], false, "{g}");
    call(&mut ctx, "feature_edit", json!({ "tab": "Part Studio 1", "feature": "Extrude 1", "params": { "depth": "30 mm" } }));
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
    assert_eq!(g["out_of_date"], true, "{g}");
}

#[test]
fn drawing_views_and_dimensions() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing.noshape");
    let t = call(&mut ctx, "drawing_templates", json!({}));
    assert!(t["templates"].as_array().unwrap().iter().any(|r| r["name"] == "ISO A3"));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
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
    // The dimension palette: prefix, precision, a symmetrical tolerance.
    let lin_id = lin["annotation"].as_u64().unwrap();
    let ed = call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": lin_id, "prefix": "2X ", "precision": 1, "tolerance": "symmetrical", "tolerance_value": 0.1 }));
    let text = ed["annotations"].as_array().unwrap().iter().find(|x| x["id"] == lin_id).unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains("2X") && text.contains("80") && text.contains("±0.1"), "{text}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert!(hist["history"][0]["description"].as_str().unwrap().ends_with(":: Edit dimension"), "{hist}");
    let ed = call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": lin_id, "value": "REF", "tolerance": "None" }));
    let text = ed["annotations"].as_array().unwrap().iter().find(|x| x["id"] == lin_id).unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains("REF") && !text.contains("±"), "{text}");
    let e = try_call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": dia["annotation"], "value": "X" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("value"));
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
    // ⋯ › Copy table: the clipboard text, tab-separated.
    let copy = call(&mut ctx, "assembly_bom", json!({ "copy": true }));
    assert!(copy["copied"].as_str().unwrap().starts_with("Item\tQuantity"), "{copy}");
    assert!(copy["message"].as_str().unwrap().ends_with("BOM table copied to clipboard."), "{copy}");
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
    // The view's menu: Hide all explode lines, Show all explode lines.
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "lines", "view": "Pin out", "on": false }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Hide all explode lines");
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "lines", "view": "Pin out", "on": true }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Show all explode lines");
    let ev = call(&mut ctx, "assembly_exploded_view", json!({ "action": "delete", "view": "Pin out" }));
    assert_eq!(ev["exploded_views"].as_array().unwrap().len(), 0, "{ev}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Delete : Pin out");
}

#[test]
fn drawing_section_detail_views_and_annotations() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_tools.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
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
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
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

    let rt = call(&mut ctx, "drawing_revision_table", json!({ "drawing": tab, "position": [400, 290], "visible_rows": 3 }));
    assert!(rt["table"].as_u64().is_some(), "{rt}");
    assert!(last(&mut ctx).ends_with(":: Revision table"));
    let rc = call(&mut ctx, "drawing_revision_callout", json!({ "drawing": tab, "position": [100, 100] }));
    assert_eq!(kind_of(&mut ctx, rc["annotation"].as_u64().unwrap()), "RevisionCallout");
    assert!(last(&mut ctx).ends_with(":: Revision callout"));

    let gt = call(&mut ctx, "drawing_table", json!({ "drawing": tab, "position": [200, 60], "rows": 2, "columns": 3, "header_row": true }));
    let gid = gt["table"].as_u64().unwrap();
    assert!(last(&mut ctx).ends_with(":: Table"));
    call(&mut ctx, "drawing_table_cell", json!({ "drawing": tab, "table": gid, "row": 0, "column": 1, "text": "QTY" }));
    assert!(last(&mut ctx).ends_with(":: Edit cell"));
    let e = try_call(&mut ctx, "drawing_table_cell", json!({ "drawing": tab, "table": rt["table"], "row": 0, "column": 0, "text": "x" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("table"), "{e:?}");

    let dt = call(&mut ctx, "drawing_datum", json!({ "drawing": tab, "anchor": quarter(&lines[1]), "position": [quarter(&lines[1])[0], quarter(&lines[1])[1] + 15.0] }));
    assert_eq!(kind_of(&mut ctx, dt["annotation"].as_u64().unwrap()), "Datum");
    assert!(last(&mut ctx).ends_with(":: Datum"));
}

#[test]
fn sketch_fillet_radius_default_is_r5_then_sticky() {
    // Live: R5 on a fresh page, then the last radius used in that tab.
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "fillet_sticky.noshape", "plane:top");
    let rect = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [40, 30]] }));
    let corner = |x: f64, y: f64| id_of(created(&rect, "point").into_iter().find(|p| near(&p["at"], x, y)).unwrap_or_else(|| panic!("no corner {x},{y}: {rect}")));
    let radius = |f: &Value| {
        let a = created(f, "arc")[0];
        ((num(&a["start"][0]) - num(&a["center"][0])).powi(2) + (num(&a["start"][1]) - num(&a["center"][1])).powi(2)).sqrt()
    };
    let f = call(&mut ctx, "sketch_fillet", json!({ "sketch": sk, "entities": [corner(40.0, 0.0)] }));
    assert!((radius(&f) - 5.0).abs() < 1e-6, "{f}");
    let f = call(&mut ctx, "sketch_fillet", json!({ "sketch": sk, "entities": [corner(40.0, 30.0)], "radius": "3 mm" }));
    assert!((radius(&f) - 3.0).abs() < 1e-6, "{f}");
    // A new sketch in the same Part Studio: the last radius sticks.
    let sk2 = call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }))["sketch"].as_str().unwrap().to_string();
    let rect2 = call(&mut ctx, "sketch_draw", json!({ "sketch": sk2, "tool": "corner_rectangle", "points": [[0, 0], [40, 30]] }));
    let c = id_of(created(&rect2, "point").into_iter().find(|p| near(&p["at"], 40.0, 0.0)).unwrap());
    let f = call(&mut ctx, "sketch_fillet", json!({ "sketch": sk2, "entities": [c] }));
    assert!((radius(&f) - 3.0).abs() < 1e-6, "{f}");
}

#[test]
fn drawing_jogged_and_aligned_sections() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_sections.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let (top, circle) = g["views"].as_array().unwrap().iter().find_map(|v| v["edges"].as_array().and_then(|es| es.iter().find(|e| e["type"] == "circle").map(|c| (v.clone(), c.clone())))).unwrap();
    let top_id = top["id"].as_u64().unwrap();
    let (cx, cy) = (num(&circle["center"][0]), num(&circle["center"][1]));
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let views_of = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"].as_array().unwrap().clone();
    // Jogged section: across to the hole center, then up; aligned section about the hole.
    let min_x = num(&top["bounds"]["min"][0]);
    let jg = call(&mut ctx, "drawing_jogged_section_view", json!({ "drawing": tab, "parent": top_id, "points": [[min_x + 3.0, cy - 5.0], [cx, cy - 5.0], [cx, cy + 5.0]], "position": [cx, cy - 80.0] }));
    let jv = views_of(&mut ctx).into_iter().find(|v| v["id"] == jg["view"]).unwrap();
    assert_eq!(jv["kind"], "section", "{jv}");
    assert!(last(&mut ctx).ends_with(":: Section view"));
    let al = call(&mut ctx, "drawing_aligned_section_view", json!({ "drawing": tab, "parent": top_id, "center": [cx, cy], "a": [cx + 10.0, cy], "b": [cx, cy + 10.0], "position": [cx + 120.0, cy - 60.0] }));
    let av = views_of(&mut ctx).into_iter().find(|v| v["id"] == al["view"]).unwrap();
    assert_eq!(av["kind"], "aligned_section", "{av}");
    assert!(last(&mut ctx).ends_with(":: Aligned section view"));

}

#[test]
fn drawing_out_of_date_until_update() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_update.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    // A drawing edit stamps the state its views are made from.
    call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "A", "position": [30, 30] }));
    // The tallest view's height on the sheet (a side view shows the 20 mm thickness).
    let height = |ctx: &mut Ctx| {
        let g = call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
        let h = g["views"].as_array().unwrap().iter().map(|v| num(&v["bounds"]["max"][1]) - num(&v["bounds"]["min"][1])).fold(0.0f64, f64::max);
        (h, g["out_of_date"].as_bool().unwrap())
    };
    let (h0, ood) = height(&mut ctx);
    assert!(!ood);
    // Change the model: the views keep their geometry, the drawing is out of date.
    call(&mut ctx, "feature_edit", json!({ "tab": "Part Studio 1", "feature": "Extrude 1", "params": { "depth": "40 mm" } }));
    let (h1, ood) = height(&mut ctx);
    assert!(ood, "out of date after the model changed");
    assert!((h1 - h0).abs() < 1e-6, "views keep their geometry until Update: {h0} -> {h1}");
    // A drawing edit while out of date keeps the old state.
    call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "B", "position": [30, 40] }));
    assert!(height(&mut ctx).1, "still out of date");
    call(&mut ctx, "drawing_update", json!({ "drawing": tab }));
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Update"));
    let (h2, ood) = height(&mut ctx);
    assert!(!ood);
    assert!(h2 > h0 + 1.0, "updated views: {h0} -> {h2}");
}

#[test]
fn drawing_broken_out_sections_and_profile_crops() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_broken_out.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let views_of = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"].as_array().unwrap().clone();
    let vs = views_of(&mut ctx);
    let (a, b) = (&vs[0], &vs[1]);
    // A triangle inside a view's bounds, around its middle.
    let tri = |v: &Value| {
        let (x0, y0, x1, y1) = (num(&v["bounds"]["min"][0]), num(&v["bounds"]["min"][1]), num(&v["bounds"]["max"][0]), num(&v["bounds"]["max"][1]));
        let (cx, cy, w, h) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 4.0, (y1 - y0) / 4.0);
        json!([[cx - w, cy - h], [cx + w, cy - h], [cx, cy + h]])
    };
    // Broken-out: the view under the first point, the dialog's default depth.
    let r = call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "points": tri(a) }));
    assert_eq!(r["view"], a["id"], "{r}");
    assert!(last(&mut ctx).ends_with(":: Broken-out section"));
    let v = views_of(&mut ctx).into_iter().find(|v| v["id"] == a["id"]).unwrap();
    let d0 = num(&v["broken_out"][0]["depth_mm"]);
    assert!(d0 > 0.0, "{v}");
    // Edit it to a 3 mm blind depth.
    call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": a["id"], "points": tri(a), "depth": "3 mm", "index": 0 }));
    let v = views_of(&mut ctx).into_iter().find(|v| v["id"] == a["id"]).unwrap();
    assert_eq!(v["broken_out"].as_array().unwrap().len(), 1, "{v}");
    assert!((num(&v["broken_out"][0]["depth_mm"]) - 3.0).abs() < 1e-9, "{v}");
    let e = try_call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": a["id"], "points": tri(a), "depth": 0 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("depth"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": a["id"], "points": tri(a), "index": 4 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("index"), "{e:?}");
    // Spline and polygon crops of the other view.
    call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "spline", "points": tri(b) }));
    assert!(last(&mut ctx).ends_with(":: Crop view"));
    assert_eq!(views_of(&mut ctx).into_iter().find(|v| v["id"] == b["id"]).unwrap()["crop"], "spline");
    call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "polygon", "points": tri(b) }));
    assert_eq!(views_of(&mut ctx).into_iter().find(|v| v["id"] == b["id"]).unwrap()["crop"], "polygon");
    let e = try_call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "polygon", "points": [[1, 1], [2, 2]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("points"), "{e:?}");
}

#[test]
fn drawing_hatch_regions() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_hatch.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    // Inside the top view's outline, off the hole: the face region.
    let (top, circle) = g["views"].as_array().unwrap().iter().find_map(|v| v["edges"].as_array().and_then(|es| es.iter().find(|e| e["type"] == "circle").map(|c| (v.clone(), c.clone())))).unwrap();
    let (x0, y0) = (num(&top["bounds"]["min"][0]), num(&top["bounds"]["min"][1]));
    let (cx, cy) = (num(&circle["center"][0]), num(&circle["center"][1]));
    let inside = [x0 + (cx - x0) * 0.3, y0 + (cy - y0) * 0.3];
    let h = call(&mut ctx, "drawing_hatch_region", json!({ "drawing": tab, "points": [inside], "standard": "iso", "pattern": "ISO02W100", "scale": 2 }));
    assert!(last(&mut ctx).ends_with(":: Hatch region"));
    let id = h["hatches"][0].as_u64().unwrap();
    let hs = h["hatches"].clone();
    assert_eq!(hs.as_array().unwrap().len(), 1, "{h}");
    let got = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
    let hj = &got["hatches"][0];
    assert_eq!(hj["view"], top["id"], "{hj}");
    assert_eq!(hj["standard"], "iso", "{hj}");
    assert!(hj["pattern"].as_str().unwrap().starts_with("ISO02W100"), "{hj}");
    assert_eq!(num(&hj["scale"]), 2.0, "{hj}");
    // Edit: the General tab (scale back to 1), a red color.
    call(&mut ctx, "drawing_hatch_region", json!({ "drawing": tab, "hatch": id, "standard": "general", "color": [255, 0, 0] }));
    assert!(last(&mut ctx).ends_with(":: Edit hatch"));
    let got = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
    assert_eq!(got["hatches"].as_array().unwrap().len(), 1, "{got}");
    assert_eq!(got["hatches"][0]["standard"], "general");
    assert_eq!(got["hatches"][0]["color"], json!([255, 0, 0]));
    assert_eq!(num(&got["hatches"][0]["scale"]), 1.0);
    // Off every profile, and a bad pattern name.
    let e = try_call(&mut ctx, "drawing_hatch_region", json!({ "drawing": tab, "points": [[5, 5]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("points[0]"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_hatch_region", json!({ "drawing": tab, "points": [inside], "pattern": "nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("pattern"), "{e:?}");
}

#[test]
fn drawing_custom_table_from_a_feature_studio() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_custom_table.noshape");
    call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
    let src = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");
annotation { "Table Type Name" : "Part faces" }
export const partFaces = defineTable(function(context is Context, definition is map) returns Table
    precondition
    {
    }
    {
        const columns = [tableColumnDefinition("name", "Part"), tableColumnDefinition("faces", "Faces")];
        var rows = [];
        for (var body in evaluateQuery(context, qAllModifiableSolidBodies()))
            rows = append(rows, tableRow({ "name" : getProperty(context, { "entity" : body, "propertyType" : PropertyType.NAME }), "faces" : size(evaluateQuery(context, qOwnedByBody(body, EntityType.FACE))) }, body));
        return table("Faces", columns, rows);
    });
"#;
    call(&mut ctx, "featurestudio_edit", json!({ "source": src }));
    call(&mut ctx, "featurestudio_commit", json!({}));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    // By display name; the Part Studio the drawing shows.
    let r = call(&mut ctx, "drawing_custom_table", json!({ "drawing": tab, "table": "Part faces", "position": [400, 280] }));
    assert!(last(&mut ctx).ends_with(":: Custom table"));
    let id = r["table"].as_u64().unwrap();
    let t = r["tables"].as_array().unwrap().iter().find(|t| t["id"] == id).unwrap().clone();
    assert_eq!(t["kind"], "Custom", "{t}");
    assert_eq!(t["header"], json!(["Part", "Faces"]), "{t}");
    // The block with a hole: 6 planar faces and the hole's.
    assert!(t["rows"][0][1].as_str().unwrap().parse::<u32>().unwrap() >= 7, "{t}");
    // Properties: bottom to top, another corner.
    call(&mut ctx, "drawing_custom_table", json!({ "drawing": tab, "id": id, "bottom_to_top": true, "corner": "bottom_left" }));
    assert!(last(&mut ctx).ends_with(":: Custom table properties"));
    let e = try_call(&mut ctx, "drawing_custom_table", json!({ "drawing": tab, "table": "nope", "position": [10, 10] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("table"), "{e:?}");
    assert!(e.message.contains("Part faces"), "{e:?}");
    // Its properties in drawing_table_properties too; its Feature Studio (this document's).
    call(&mut ctx, "drawing_table_properties", json!({ "drawing": tab, "table": id, "bottom_to_top": false }));
    assert!(last(&mut ctx).ends_with(":: Custom table properties"));
    let f = call(&mut ctx, "drawing_open_feature_studio", json!({ "drawing": tab, "table": id }));
    assert_eq!((f["linked"].clone(), f["target"]["kind"].clone()), (json!(false), json!("Feature Studio")), "{f}");
    let s = call(&mut ctx, "drawing_switch_to_reference", json!({ "drawing": tab, "table": id }));
    assert_eq!(s["target"]["name"], "Part Studio 1", "{s}");
}

#[test]
fn drawing_insert_image_and_cad() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_insert.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let hist = |ctx: &mut Ctx, n: usize| call(ctx, "history_list", json!({ "limit": n }))["history"].as_array().unwrap().iter().map(|h| h["description"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // A local square PNG: imported as a file tab, then placed 30 mm high.
    let png = root.join("packaging/icons/noshape-256.png");
    let r = call(&mut ctx, "drawing_insert_image", json!({ "drawing": tab, "path": png.to_str().unwrap(), "position": [20, 20] }));
    let h = hist(&mut ctx, 2);
    assert!(h[0].ends_with(":: Insert image") && h[1].contains("Import noshape-256.png"), "{h:?}");
    let img = &r["images"][0];
    assert_eq!(img["id"], r["image"], "{r}");
    assert_eq!(img["size"], json!([30.0, 30.0]), "{img}");
    // From the file tab now in the document, dragged 50 mm wide (aspect kept).
    let r = call(&mut ctx, "drawing_insert_image", json!({ "drawing": tab, "file": "noshape-256.png", "position": [100, 20], "to": [150, 30] }));
    assert_eq!(r["images"][1]["size"], json!([50.0, 50.0]), "{r}");
    let e = try_call(&mut ctx, "drawing_insert_image", json!({ "drawing": tab, "file": "Part Studio 1", "position": [10, 10] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("file"), "{e:?}");
    // A DXF as sheet sketch geometry.
    let dxf = root.join("crates/sketch/tests/fixtures/dwg/r2000_entities-2d.dxf");
    let r = call(&mut ctx, "drawing_insert_cad", json!({ "drawing": tab, "path": dxf.to_str().unwrap() }));
    assert!(hist(&mut ctx, 1)[0].ends_with(":: Insert DXF or DWG"));
    let n = r["entities"].as_u64().unwrap();
    assert!(n > 0, "{r}");
    assert!(r["sketch_entities"].as_u64().unwrap() >= n, "{r}");
    let e = try_call(&mut ctx, "drawing_insert_cad", json!({ "drawing": tab, "path": png.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("path"), "{e:?}");
}

#[test]
fn drawing_sketch_tools() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_sketch.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    // A closed triangle: the last click lands on the first point (no new point).
    let r = call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "line", "picks": [[30, 250], [60, 250], [45, 270], [30, 250]] }));
    assert!(last(&mut ctx).ends_with(":: Line"));
    assert_eq!(r["created"], 6, "3 points, 3 lines: {r}");
    let r = call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "circle", "picks": [[100, 250], [110, 250]] }));
    assert!(last(&mut ctx).ends_with(":: Center point circle"));
    assert!(r["created"].as_i64().unwrap() >= 1, "{r}");
    call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "point", "picks": [[150, 250]] }));
    assert!(last(&mut ctx).ends_with(":: Sketch point"));
    let r = call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "spline", "picks": [[30, 200], [50, 210], [70, 200], [90, 210]] }));
    assert!(last(&mut ctx).ends_with(":: Spline"));
    let n = r["sketch_entities"].as_u64().unwrap();
    let r = call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "spline_point", "picks": [[60, 205]] }));
    assert!(last(&mut ctx).ends_with(":: Spline point"));
    assert!(r["sketch_entities"].as_u64().unwrap() > n, "{r}");
    let e = try_call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "spline_point", "picks": [[200, 100]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("picks[0]"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "circle", "picks": [[1, 1]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("picks"), "{e:?}");
    // Trim the lone point away.
    let before = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["sketch_entities"].as_u64().unwrap();
    let r = call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "trim", "picks": [[150, 250]] }));
    assert!(last(&mut ctx).ends_with(":: Trim"));
    assert_eq!(r["sketch_entities"].as_u64().unwrap(), before - 1, "{r}");
}

#[test]
fn drawing_format_painter_copies_content() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_painter.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let a = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "FROM", "position": [30, 30] }))["annotation"].clone();
    let b = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "TO", "position": [30, 60] }))["annotation"].clone();
    let text = |ctx: &mut Ctx, id: &Value| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["annotations"].as_array().unwrap().iter().find(|x| x["id"] == *id).unwrap()["text"].clone();
    // Style only (the default) keeps the text.
    call(&mut ctx, "drawing_format_painter", json!({ "drawing": tab, "source": a, "targets": [b] }));
    assert_eq!(text(&mut ctx, &b), "TO");
    call(&mut ctx, "drawing_format_painter", json!({ "drawing": tab, "source": a, "targets": [b], "content": true }));
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Format painter"));
    assert_eq!(text(&mut ctx, &b), "FROM");
    let e = try_call(&mut ctx, "drawing_format_painter", json!({ "drawing": tab, "source": a, "targets": [999] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("targets[0]"), "{e:?}");
}

#[test]
fn drawing_inspection_items() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_inspection.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let ids: Vec<Value> = ["A", "B", "C"].iter().enumerate().map(|(i, t)| call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": t, "position": [30, 30 + 15 * i] }))["annotation"].clone()).collect();
    let numbers = |ctx: &mut Ctx| -> Vec<Value> {
        let g = call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
        ids.iter().map(|id| g["annotations"].as_array().unwrap().iter().find(|x| x["id"] == *id).unwrap()["inspection"].clone()).collect()
    };
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "add", "annotations": [ids[0], ids[1]] }));
    assert!(last(&mut ctx).ends_with(":: Inspection item"));
    assert_eq!(numbers(&mut ctx), vec![json!(1), json!(2), Value::Null]);
    // Reorder from 5.
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "add", "annotations": [ids[1], ids[0]], "number": 5 }));
    assert_eq!(numbers(&mut ctx), vec![json!(6), json!(5), Value::Null]);
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "edit", "id": ids[0], "number": 9 }));
    assert!(last(&mut ctx).ends_with(":: Edit inspection item"));
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "move", "id": ids[0], "to": ids[2] }));
    assert_eq!(numbers(&mut ctx), vec![Value::Null, json!(5), json!(9)]);
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "remove", "annotations": [ids[1]] }));
    assert_eq!(numbers(&mut ctx), vec![Value::Null, Value::Null, json!(9)]);
    // Automatic numbers dimensions, GD&T, surface finishes... (not notes).
    let sf = call(&mut ctx, "drawing_surface_finish", json!({ "drawing": tab, "position": [200, 30] }))["annotation"].clone();
    let g = call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "automatic" }));
    assert!(last(&mut ctx).ends_with(":: Inspection items"));
    assert_eq!(g["annotations"].as_array().unwrap().iter().find(|x| x["id"] == sf).unwrap()["inspection"], 10, "{g}");
    assert_eq!(numbers(&mut ctx), vec![Value::Null, Value::Null, json!(9)]);
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "hide" }));
    assert!(last(&mut ctx).ends_with(":: Hide inspection items"));
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "shape", "shape": "flag" }));
    assert!(last(&mut ctx).ends_with(":: Inspection border shape"));
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "remove_all" }));
    assert_eq!(numbers(&mut ctx), vec![Value::Null, Value::Null, Value::Null]);
    let e = try_call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "edit", "id": ids[0], "number": 2 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("id"), "{e:?}");
}

#[test]
fn drawing_properties_get_and_set() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_props.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "drawing_properties", json!({ "drawing": tab }))["properties"].clone();
    assert!(p["inspection_text_height"].is_number(), "{p}");
    let r = call(&mut ctx, "drawing_properties", json!({ "drawing": tab, "set": { "inspection_text_height": 3.5, "decimal_comma": true } }));
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Drawing properties"));
    assert_eq!(r["properties"]["inspection_text_height"], 3.5, "{r}");
    assert_eq!(r["properties"]["decimal_comma"], true, "{r}");
    // Locked: nothing but the lock changes.
    call(&mut ctx, "drawing_properties", json!({ "drawing": tab, "set": { "locked": true } }));
    let r = call(&mut ctx, "drawing_properties", json!({ "drawing": tab, "set": { "inspection_text_height": 5.0 } }));
    assert_eq!(r["properties"]["inspection_text_height"], 3.5, "{r}");
    let e = try_call(&mut ctx, "drawing_properties", json!({ "drawing": tab, "set": { "nope": 1 } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set"), "{e:?}");
}

#[test]
fn drawing_auxiliary_view_in_line_with_a_sketch_line() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_aux_inline.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let v = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"][0].clone();
    let (x0, y0, x1, y1) = (num(&v["bounds"]["min"][0]), num(&v["bounds"]["min"][1]), num(&v["bounds"]["max"][0]), num(&v["bounds"]["max"][1]));
    // A 45° sketch line across the view's middle (off its corners).
    let (cx, cy, h) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0).min(y1 - y0) / 4.0);
    call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "line", "picks": [[cx - h * 0.7, cy - h * 0.6], [cx + h * 0.7, cy + h * 0.8]] }));
    let e = try_call(&mut ctx, "drawing_auxiliary_view", json!({ "drawing": tab, "mode": "in_line", "edge": [x0 - 30.0, y0 - 30.0], "position": [0, 0] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("edge"), "{e:?}");
    let mid = [cx, cy + h * 0.1];
    let r = call(&mut ctx, "drawing_auxiliary_view", json!({ "drawing": tab, "mode": "in_line", "edge": mid, "position": [mid[0] + 80.0, mid[1] - 80.0] }));
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Auxiliary view"));
    let id = r["view"].clone();
    assert_eq!(r["views"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap()["kind"], "auxiliary", "{r}");
}

/// Live samples (outside the repo; skipped when missing).
fn live_sample(name: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new("/root/onshape-research/live/samples").join(name);
    p.is_file().then_some(p)
}

#[test]
fn pvz_of_several_parts_imports_as_part_studio_and_assembly() {
    let Some(pvz) = live_sample("pvz-shapes.pvz") else { return };
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("pvz_import.noshape").to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [pvz.to_str().unwrap()] }));
    // Live (qa5 zip import): Assembly "RootAssembly" then Part Studio "pvz-shapes parts".
    let mut tabs: Vec<(String, String)> = r["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| (t["name"].as_str().unwrap().to_string(), t["kind"].as_str().unwrap().to_string())).collect();
    tabs.sort();
    assert_eq!(tabs, vec![("RootAssembly".to_string(), "Assembly".to_string()), ("pvz-shapes parts".to_string(), "Part Studio".to_string())], "{r}");
    let order: Vec<String> = call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    let (a, s) = (order.iter().position(|t| t == "RootAssembly").unwrap(), order.iter().position(|t| t == "pvz-shapes parts").unwrap());
    assert!(a < s, "the Assembly tab first: {order:?}");
    // Parts named as in the file, in live's order, mesh volumes as live's.
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "pvz-shapes parts" }));
    let solids: Vec<(String, f64)> = parts["parts"].as_array().unwrap().iter().filter(|p| p["kind"] == "Solid").map(|p| (p["name"].as_str().unwrap().to_string(), num(&p["volume"]))).collect();
    let names: Vec<&str> = solids.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(names, ["Cylinder", "Torus", "Cone"], "{parts}");
    for (got, live) in solids.iter().map(|s| s.1).zip([9407.77811303131, 1770.239206699627, 1569.176626930149]) {
        assert!((got - live).abs() < 1e-3, "{got} vs {live}");
    }
    assert!(parts["parts"].as_array().unwrap().iter().any(|p| p["kind"] == "Surface"), "the Loft's open mesh: {parts}");
    // The Assembly: an instance per solid (the composite isn't instanced).
    let asm = call(&mut ctx, "assembly_list", json!({ "tab": "RootAssembly" }));
    let mut inst: Vec<String> = asm["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    inst.sort();
    assert_eq!(inst, ["Cone <1>", "Cylinder <1>", "Torus <1>"], "{asm}");
}

#[test]
fn pvz_of_one_part_keeps_its_name() {
    let Some(pvz) = live_sample("pvz-block.pvz") else { return };
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("pvz_import1.noshape").to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [pvz.to_str().unwrap()] }));
    let kinds: Vec<&str> = r["tabs"].as_array().unwrap().iter().map(|t| t["kind"].as_str().unwrap()).collect();
    assert!(!kinds.contains(&"Assembly"), "{r}");
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "pvz-block" }));
    assert_eq!(parts["parts"][0]["name"], "Part 1", "{parts}");
    assert!((num(&parts["parts"][0]["volume"]) - 7999.999463558208).abs() < 1e-3, "{parts}");
}

/// qa4 IMP1: the parts of one Import feature never share face names (picking
/// one part highlighted faces of the other).
#[test]
fn imported_parts_have_their_own_face_names() {
    let Some(step) = live_sample("g48-two.step") else { return };
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("imp1.noshape").to_str().unwrap() }));
    call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
    let faces = call(&mut ctx, "query_entities", json!({ "tab": "g48-two", "kind": "faces" }));
    let list = faces["faces"].as_array().unwrap();
    let parts: std::collections::BTreeSet<&str> = list.iter().map(|f| f["part"].as_str().unwrap()).collect();
    assert_eq!(parts.len(), 2, "{faces}");
    let mut ids: Vec<&str> = list.iter().map(|f| f["id"].as_str().unwrap()).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "a face id names faces of both parts: {faces}");
}

/// Live ("api (10-23) import parity"): a second import of a file gives its
/// Part Studio the next free " (n)" and keeps the file tab's name.
#[test]
fn second_import_of_a_file_is_numbered_like_onshape() {
    let Some(step) = live_sample("g25-b.step") else { return };
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("reimport.noshape").to_str().unwrap() }));
    call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
    call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
    let tabs: Vec<(String, String)> = call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().map(|t| (t["name"].as_str().unwrap().to_string(), t["kind"].as_str().unwrap().to_string())).collect();
    let studios: Vec<&str> = tabs.iter().filter(|t| t.1 == "Part Studio" && t.0.starts_with("g25-b")).map(|t| t.0.as_str()).collect();
    assert_eq!(studios, ["g25-b", "g25-b (1)"], "{tabs:?}");
    assert_eq!(tabs.iter().filter(|t| t.1 == "File" && t.0 == "g25-b.step").count(), 2, "{tabs:?}");
}

/// Live: pvz-rot-asm.pvz's instances: <1> turned 90° about z and moved 50 mm
/// along x, <2> in place.
#[test]
fn pvz_instances_keep_their_placements() {
    let Some(pvz) = live_sample("pvz-rot-asm.pvz") else { return };
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("pvz_rot.noshape").to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [pvz.to_str().unwrap()] }));
    let asm = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Assembly").unwrap()["name"].as_str().unwrap().to_string();
    assert_eq!(asm, "pvz rot asm", "{r}");
    let list = call(&mut ctx, "assembly_list", json!({ "tab": asm }));
    let inst = list["instances"].as_array().unwrap();
    let find = |n: &str| inst.iter().find(|i| i["name"] == n).unwrap_or_else(|| panic!("{n}: {list}"));
    let (a, b) = (find("Part 1 <1>"), find("Part 1 <2>"));
    let t = |i: &Value| [num(&i["translation"][0]), num(&i["translation"][1]), num(&i["translation"][2])];
    assert!((t(a)[0] - 50.0).abs() < 1e-6 && t(a)[1].abs() < 1e-6, "{a}");
    assert!(t(b).iter().all(|x| x.abs() < 1e-6), "{b}");
    // 90° about z: quaternion (0, 0, ±√½, √½).
    let q = &a["rotation"];
    assert!((num(&q[2]).abs() - 0.5f64.sqrt()).abs() < 1e-6, "{a}");
    // Imported again: the root's raw name, then that numbered (live).
    let again = |ctx: &mut Ctx| -> String {
        let r = call(ctx, "import_files", json!({ "files": [pvz.to_str().unwrap()] }));
        r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Assembly").unwrap()["name"].as_str().unwrap().to_string()
    };
    assert_eq!(again(&mut ctx), "pvz rot asm_54");
    assert_eq!(again(&mut ctx), "pvz rot asm_54 (1)");
    // pvz-assembly.pvz: <1> in place, <2> 50 mm along x.
    let Some(pvz) = live_sample("pvz-assembly.pvz") else { return };
    let r = call(&mut ctx, "import_files", json!({ "files": [pvz.to_str().unwrap()] }));
    let asm = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Assembly").unwrap()["name"].as_str().unwrap().to_string();
    let list = call(&mut ctx, "assembly_list", json!({ "tab": asm }));
    let inst = list["instances"].as_array().unwrap();
    let find = |n: &str| inst.iter().find(|i| i["name"] == n).unwrap_or_else(|| panic!("{n}: {list}"));
    assert!(t(find("Part 1 <1>")).iter().all(|x| x.abs() < 1e-6), "{list}");
    assert!((t(find("Part 1 <2>"))[0] - 50.0).abs() < 1e-6, "{list}");
}

/// Export Options › Store file in a new tab (live: the file tab "<name>.<ext>"
/// directly after the exported tab).
#[test]
fn export_store_file_in_a_new_tab() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "export_store.noshape");
    let order = |ctx: &mut Ctx| -> Vec<String> { call(ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect() };
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "PARASOLID", "options": "store" }));
    assert!(r.get("file").is_none() && r["stored_tab"].is_string(), "{r}");
    assert_eq!(last(&mut ctx), "Store Part Studio 1.x_t");
    let o = order(&mut ctx);
    let ps = o.iter().position(|t| t == "Part Studio 1").unwrap();
    assert_eq!(o[ps + 1], "Part Studio 1.x_t", "{o:?}");
    // Download and store: the file too.
    let file = scratch("stored.step");
    let r = call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP", "file": file.to_str().unwrap(), "options": "download_and_store" }));
    assert!(file.is_file(), "{r}");
    assert!(order(&mut ctx).contains(&"stored.step".to_string()));
    // A drawing's export.
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let name = call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().find(|t| t["id"] == tab.as_str() || t["name"] == tab.as_str()).unwrap()["name"].as_str().unwrap().to_string();
    call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "PDF", "options": "store" }));
    let o = order(&mut ctx);
    let di = o.iter().position(|t| *t == name).unwrap();
    assert_eq!(o[di + 1], format!("{name}.pdf"), "{o:?}");
    let e = try_call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("file"), "{e:?}");
}

/// Rhino (EXP1): export and import back with the same volume (builds with
/// openNURBS; without it the export says so).
#[test]
fn rhino_export_round_trip() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "rhino_rt.noshape");
    let want = num(&call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"][0]["volume"]);
    let file = scratch("rhino_rt.3dm");
    match try_call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "RHINO", "file": file.to_str().unwrap() })) {
        Ok(_) => {}
        Err(e) => {
            assert!(e.message.contains("openNURBS"), "{e:?}");
            return;
        }
    }
    let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
    let studio = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").unwrap()["name"].as_str().unwrap().to_string();
    let got = num(&call(&mut ctx, "query_parts", json!({ "tab": studio }))["parts"][0]["volume"]);
    assert!((got - want).abs() < 2e-5 * want, "{got} vs {want}");
}

/// Export Drawing through the API with the modal's options: raster formats,
/// versions, the inspection list.
#[test]
fn drawing_export_formats_and_options() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_export_all.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let head = |p: &std::path::Path, n: usize| std::fs::read(p).unwrap()[..n].to_vec();
    for (format, magic) in [("PNG", &[0x89u8, b'P', b'N', b'G'][..]), ("JPEG", &[0xffu8, 0xd8, 0xff][..]), ("TIFF", &[b'I', b'I', 42, 0][..])] {
        let f = scratch(&format!("sheet.{}", format.to_lowercase()));
        call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": format, "file": f.to_str().unwrap() }));
        assert_eq!(head(&f, magic.len()), magic, "{format}");
    }
    let dxf = scratch("sheet.dxf");
    call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "DXF", "version": "2018", "file": dxf.to_str().unwrap(), "text": "explode_all", "units": "Inch" }));
    assert!(std::fs::read_to_string(&dxf).unwrap().contains("AC1032"));
    let e = try_call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "PDF", "version": "9.9", "file": "/tmp/x.pdf" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{e:?}");
    // The inspection list: a CSV with the panel's columns.
    call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "A", "position": [30, 30] }));
    let csv = scratch("insp.csv");
    call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "Inspection list", "file": csv.to_str().unwrap() }));
    assert!(!std::fs::read_to_string(&csv).unwrap().is_empty());
}

/// Context menus: Copy sketch → Paste sketch on another plane; Export as
/// DXF/DWG… of a sketch and of a planar face; the Sketch dialog's Disable
/// imprinting.
#[test]
fn sketch_paste_export_and_imprinting() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "sketch_paste.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let src = call(&mut ctx, "sketch_get", json!({ "sketch": "Sketch 1" }));
    let p = call(&mut ctx, "sketch_paste", json!({ "sketch": "Sketch 1", "plane": "plane:front" }));
    assert!(last(&mut ctx).ends_with(":: Paste sketch"));
    assert_eq!(p["entities"].as_array().unwrap().len(), src["entities"].as_array().unwrap().len(), "{p}");
    assert_ne!(p["sketch"], src["sketch"], "a new sketch: {p}");
    // Its plane is Front: the frame's normal is ±y.
    let n = &p["frame"]["normal"];
    assert!((num(&n[1]).abs() - 1.0).abs() < 1e-9, "{p}");
    let dxf = scratch("sketch1.dxf");
    let r = call(&mut ctx, "sketch_export", json!({ "sketch": "Sketch 1", "file": dxf.to_str().unwrap() }));
    assert!(num(&r["curves"]) >= 5.0, "rectangle + circle: {r}");
    assert!(std::fs::read_to_string(&dxf).unwrap().contains("LINE"));
    let svg = scratch("sketch1.svg");
    call(&mut ctx, "sketch_export", json!({ "sketch": "Sketch 1", "file": svg.to_str().unwrap(), "format": "svg" }));
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<svg"));
    // A planar face of the part.
    let faces = call(&mut ctx, "query_entities", json!({ "kind": "faces" }));
    let top = faces["faces"].as_array().unwrap().iter().find(|f| f["surface"] == "plane" && num(&f["normal"][2]) > 0.99).unwrap()["id"].as_str().unwrap().to_string();
    let fd = scratch("face.dxf");
    let r = call(&mut ctx, "sketch_export", json!({ "face": top, "file": fd.to_str().unwrap() }));
    assert!(num(&r["curves"]) >= 5.0, "outline + hole: {r}");
    // Disable imprinting on a new sketch.
    let s = call(&mut ctx, "sketch_create", json!({ "plane": "plane:right", "disable_imprinting": true }));
    let f = call(&mut ctx, "feature_get", json!({ "feature": s["sketch"] }));
    assert_eq!(f["params"]["disableImprinting"], true, "{f}");
}

#[test]
fn feature_folders_dependencies_and_dynamic_suppression() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "feature_folders.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let f = call(&mut ctx, "feature_add_folder", json!({ "features": ["Sketch 1", "Extrude 1"] }));
    assert_eq!(f["name"], "New folder", "the dialog's default: {f}");
    assert_eq!(f["features"].as_array().unwrap().len(), 2, "{f}");
    assert!(last(&mut ctx).ends_with(":: Add folder"));
    call(&mut ctx, "feature_rename_folder", json!({ "folder": "New folder", "name": "Base" }));
    assert!(last(&mut ctx).ends_with(":: Rename folder : Base"));
    let l = call(&mut ctx, "feature_list", json!({}));
    assert_eq!(l["folders"][0]["name"], "Base", "{l}");
    let d = call(&mut ctx, "feature_dependencies", json!({ "feature": "Extrude 1" }));
    assert_eq!(d["parents"][0]["name"], "Sketch 1", "{d}");
    assert!(d["children"].as_array().unwrap().is_empty(), "{d}");
    let d = call(&mut ctx, "feature_dependencies", json!({ "feature": "Sketch 1" }));
    assert_eq!(d["children"][0]["name"], "Extrude 1", "{d}");
    call(&mut ctx, "feature_unpack_folder", json!({ "folder": "Base" }));
    assert!(last(&mut ctx).ends_with(":: Remove folder"));
    assert!(call(&mut ctx, "feature_list", json!({}))["folders"].as_array().unwrap().is_empty());
    assert_eq!(try_call(&mut ctx, "feature_unpack_folder", json!({ "folder": "Base" })).unwrap_err().param.as_deref(), Some("folder"));
    // An empty folder (the list header's New folder).
    let e = call(&mut ctx, "feature_add_folder", json!({ "name": "Later" }));
    assert!(e["features"].as_array().unwrap().is_empty(), "{e}");
    // Dynamic suppression: suppressed while the expression is false.
    call(&mut ctx, "feature_dynamic_suppression", json!({ "feature": "Extrude 1", "unsuppressed_if": "2 mm < 1 mm" }));
    assert!(last(&mut ctx).ends_with(":: Dynamic suppression"));
    let g = call(&mut ctx, "feature_get", json!({ "feature": "Extrude 1" }));
    assert_eq!(g["suppression_expression"], "2 mm < 1 mm", "{g}");
    assert!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().is_empty(), "Extrude 1 is suppressed");
    // Unsuppress drops the expression (the menu's Unsuppress).
    call(&mut ctx, "feature_suppress", json!({ "features": ["Extrude 1"], "suppressed": false }));
    let g = call(&mut ctx, "feature_get", json!({ "feature": "Extrude 1" }));
    assert!(g.get("suppression_expression").is_none(), "{g}");
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 1);
}

#[test]
fn create_drawing_dialog_options() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "create_drawing_options.noshape");
    // The dialog's defaults: ANSI_A_INCH.dwt, No views.
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1" }));
    assert_eq!(d["drawing"], "Part Studio 1 Drawing 1", "{d}");
    assert!((num(&d["size_mm"][0]) - 279.4).abs() < 0.01 && (num(&d["size_mm"][1]) - 215.9).abs() < 0.01, "ANSI A: {d}");
    assert!(d["views"].as_array().unwrap().is_empty(), "No views: {d}");
    // Custom template: ISO A4 Portrait.
    let c = call(&mut ctx, "drawing_create", json!({ "custom_template": { "standard": "ISO", "size": "A4 Portrait", "projection": "third_angle", "title_block": false, "language": "German" } }));
    assert!((num(&c["size_mm"][0]) - 210.0).abs() < 0.01 && (num(&c["size_mm"][1]) - 297.0).abs() < 0.01, "{c}");
    let e = try_call(&mut ctx, "drawing_create", json!({ "custom_template": { "standard": "ISO", "size": "B" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("custom_template.size"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_create", json!({ "custom_template": { "horizontal_zones": 3 } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("custom_template.horizontal_zones"), "{e:?}");
    // Create Drawing of Sketch 1.
    let s = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "sketch": "Sketch 1", "suggested_views": true }));
    assert_eq!(s["drawing"], "Sketch 1 Drawing 1", "{s}");
    assert!(!s["views"].as_array().unwrap().is_empty(), "{s}");
    assert_eq!(try_call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "sketch": "Extrude 1" })).unwrap_err().param.as_deref(), Some("sketch"));
}

/// Live's raster and SVG files (behaviours "api (10-24) drawing raster
/// export"): 300 px per inch of sheet, opaque white RGB, two colours, all
/// black; PNG pHYs 3937 px/m, JPEG 4:2:0 at 100 dpi, TIFF uncompressed;
/// SVG text as outlines.
#[test]
fn drawing_raster_export_matches_live() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "raster-export.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ANSI_A_INCH.dwt", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "RASTER TEST NOTE 123", "position": [40, 40] }));
    let files: Vec<std::path::PathBuf> = ["png", "jpg", "tif", "svg"].iter().map(|e| scratch(&format!("raster.{e}"))).collect();
    let path = |ext: &str| files.iter().find(|f| f.extension().is_some_and(|x| x == ext)).unwrap().clone();
    for (format, ext) in [("PNG", "png"), ("JPEG", "jpg"), ("TIFF", "tif"), ("SVG", "svg")] {
        call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": format, "file": path(ext).to_str().unwrap() }));
    }
    let png = std::fs::read(path("png")).unwrap();
    let mut r = png::Decoder::new(std::io::Cursor::new(&png)).read_info().unwrap();
    let info = r.info().clone();
    assert_eq!((info.width, info.height), (3300, 2550), "ANSI A at 300 px/in");
    assert_eq!(info.color_type, png::ColorType::Rgb, "no alpha");
    assert_eq!(info.pixel_dims.map(|p| (p.xppu, p.yppu)), Some((3937, 3937)));
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    r.next_frame(&mut buf).unwrap();
    let colours: std::collections::HashSet<[u8; 3]> = buf.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    assert_eq!(colours, [[255, 255, 255], [0, 0, 0]].into_iter().collect(), "white and black only (no antialiasing, no colour)");
    let jpg = std::fs::read(path("jpg")).unwrap();
    // JFIF APP0: units 1 (inch), density 100 x 100.
    assert_eq!(&jpg[6..11], b"JFIF\0");
    assert_eq!(&jpg[13..18], &[1, 0, 100, 0, 100]);
    // Baseline SOF0: Y sampled 2 x 2, Cb and Cr 1 x 1 (4:2:0).
    let sof = jpg.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    assert_eq!((jpg[sof + 11], jpg[sof + 14], jpg[sof + 17]), (0x22, 0x11, 0x11));
    let tif = std::fs::read(path("tif")).unwrap();
    assert!(tif.len() >= 3300 * 2550 * 3, "uncompressed RGB: {}", tif.len());
    let svg = std::fs::read_to_string(path("svg")).unwrap();
    assert!(!svg.contains("<text"), "text as outlines");
    for attr in ["stroke=\"#", "fill=\"#"] {
        for (i, _) in svg.match_indices(attr) {
            let c = &svg[i + attr.len()..i + attr.len() + 6];
            assert!(c.eq_ignore_ascii_case("000000") || c.eq_ignore_ascii_case("ffffff"), "all black: {attr}{c}");
        }
    }
}

/// The instance and mate menus: copy and paste, folders, subassemblies,
/// lock, instance properties, dynamic suppression, Change configuration,
/// inserting a sketch.
#[test]
fn assembly_instance_menu() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "asm-menu.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let count = |l: &Value| l["instances"].as_array().unwrap().len();
    let a = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }))["instance"].as_str().unwrap().to_string();
    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }))["instance"].as_str().unwrap().to_string();
    // Copy and paste, moved 100 mm along x.
    let c = call(&mut ctx, "assembly_copy", json!({ "instances": [a], "offset": ["100 mm", 0, 0] }));
    assert_eq!(c["pasted"].as_array().unwrap().len(), 1, "{c}");
    assert_eq!(count(&c), 3);
    assert!(last(&mut ctx).ends_with(":: Paste Part Studio 1 <1>"), "{}", last(&mut ctx));
    let pasted = c["pasted"][0]["id"].as_str().unwrap().to_string();
    let p = c["instances"].as_array().unwrap().iter().find(|i| i["id"] == pasted).unwrap().clone();
    assert!((num(&p["translation"][0]) - 100.0).abs() < 1e-6, "{p}");
    // Folders: add, rename (assembly_rename), unpack.
    let f = call(&mut ctx, "assembly_add_folder", json!({ "instances": [a, b], "name": "Pair" }));
    assert_eq!(f["folders"][0]["name"], "Pair", "{f}");
    assert_eq!(f["folders"][0]["instances"].as_array().unwrap().len(), 2, "{f}");
    assert!(last(&mut ctx).ends_with(":: Add selection to folder"));
    let r = call(&mut ctx, "assembly_rename", json!({ "feature": "Pair", "name": "Duo" }));
    assert_eq!(r["folders"][0]["name"], "Duo", "{r}");
    let u = call(&mut ctx, "assembly_unpack_folder", json!({ "folder": "Duo" }));
    assert!(u["folders"].as_array().unwrap().is_empty(), "{u}");
    // Instance properties and tessellation.
    let ip = call(&mut ctx, "assembly_instance_properties", json!({ "instances": [b], "reference_designator": "R1", "best_tessellation": true }));
    let bj = ip["instances"].as_array().unwrap().iter().find(|i| i["id"] == b).unwrap().clone();
    assert_eq!((bj["reference_designator"].clone(), bj["best_tessellation"].clone()), (json!("R1"), json!(true)), "{bj}");
    // Dynamic suppression.
    let d = call(&mut ctx, "assembly_dynamic_suppression", json!({ "instance": b, "unsuppressed_if": "2 mm < 1 mm" }));
    assert_eq!(d["instances"].as_array().unwrap().iter().find(|i| i["id"] == b).unwrap()["unsuppressed_if"], "2 mm < 1 mm");
    call(&mut ctx, "assembly_dynamic_suppression", json!({ "instance": b }));
    // Move to new subassembly, lock it, dissolve it.
    let m = call(&mut ctx, "assembly_move_to_subassembly", json!({ "instances": [a, pasted] }));
    assert!(last(&mut ctx).ends_with("Move to new subassembly"), "{}", last(&mut ctx));
    assert_eq!(count(&m), 2, "the two moved, one subassembly instance: {m}");
    let sub = m["new_instance"]["id"].as_str().unwrap().to_string();
    let tabs = call(&mut ctx, "document_info", json!({}));
    assert!(tabs["tabs"].as_array().unwrap().iter().any(|t| t["name"] == "Assembly 2"), "{tabs}");
    let l = call(&mut ctx, "assembly_lock", json!({ "instance": sub, "lock": "current" }));
    assert_eq!(l["instances"].as_array().unwrap().iter().find(|i| i["id"] == sub).unwrap()["lock"], "current");
    assert!(try_call(&mut ctx, "assembly_lock", json!({ "instance": b, "lock": "current" })).is_err(), "not a subassembly");
    let e = try_call(&mut ctx, "assembly_lock", json!({ "instance": sub, "lock": "Open" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("lock"));
    let dv = call(&mut ctx, "assembly_dissolve_subassembly", json!({ "instances": [sub] }));
    assert_eq!(count(&dv), 3, "{dv}");
    assert!(last(&mut ctx).ends_with("Dissolve subassembly"));
    let cs = call(&mut ctx, "assembly_create_subassembly", json!({}));
    assert_eq!(count(&cs), 4);
    // Insert a sketch.
    let s = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "sketch": "Sketch 1" }));
    assert!(last(&mut ctx).ends_with(":: Insert : Sketch 1"));
    assert_eq!(count(&s), 5);
    // Change configuration of an instance of a configured Part Studio.
    let input = call(&mut ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "list", "name": "Size" }))["input"].as_str().unwrap().to_string();
    let big = call(&mut ctx, "config_add_option", json!({ "tab": "Part Studio 1", "input": input, "name": "Big" }))["option"].as_str().unwrap().to_string();
    let sc = call(&mut ctx, "assembly_set_configuration", json!({ "instance": b, "configuration": { input.clone(): big.clone() } }));
    assert_eq!(sc["instances"].as_array().unwrap().iter().find(|i| i["id"] == b).unwrap()["configuration"][&input], json!(big));
    assert!(last(&mut ctx).contains(":: Change configuration : "));
}

/// Standard content: browse the catalogue, insert a screw auto-sized into a
/// hole (fastened there), a washer under it, edit its length.
#[test]
fn standard_content_into_a_hole() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "std-content.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let st = call(&mut ctx, "assembly_standard_content", json!({}));
    assert!(st["standards"].as_array().unwrap().iter().any(|s| s == "ISO"), "{st}");
    let c = call(&mut ctx, "assembly_standard_content", json!({ "standard": "ISO", "category": "Bolts & screws", "class": "Socket head screws", "component": "Hex socket head cap screw ISO 4762" }));
    assert!(c["sizes"].as_array().unwrap().iter().any(|s| s == "M10"), "{c}");
    assert!(!c["lengths"].as_array().unwrap().is_empty(), "{c}");
    let block = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_set_state", json!({ "instances": [block], "fixed": true }));
    let hole = call(&mut ctx, "assembly_entities", json!({ "instance": block, "kind": "faces", "surface": "cylinder" }))["faces"][0]["id"].as_str().unwrap().to_string();
    let s = call(&mut ctx, "assembly_insert_standard", json!({ "component": "Hex socket head cap screw ISO 4762", "holes": [{ "instance": block, "entity": hole }], "auto_size": true }));
    assert_eq!(s["spec"]["size"], "M10", "auto-size rounds down to the Ø10 hole: {s}");
    assert_eq!(s["inserted"].as_array().unwrap().len(), 1, "{s}");
    let screw = s["inserted"][0].as_str().unwrap().to_string();
    assert!(last(&mut ctx).starts_with("Assembly 1 :: Insert : "), "{}", last(&mut ctx));
    let fastened: Vec<&Value> = s["mates"].as_array().unwrap().iter().filter(|m| m["type"] == "fastened").collect();
    assert_eq!(fastened.len(), 1, "{s}");
    assert_eq!(fastened[0]["ok"], true, "{s}");
    // Edit its length.
    let lengths = call(&mut ctx, "assembly_standard_content", json!({ "standard": "ISO", "category": "Bolts & screws", "class": "Socket head screws", "component": "Hex socket head cap screw ISO 4762", "size": "M10" }))["lengths"].clone();
    let l = lengths.as_array().unwrap().last().unwrap().as_str().unwrap().to_string();
    let e = call(&mut ctx, "assembly_edit_standard", json!({ "instances": [screw], "length": l }));
    assert_eq!(e["spec"]["length"], json!(l), "{e}");
    assert!(last(&mut ctx).contains(":: Edit standard content : "));
    // Bad values name the field.
    let err = try_call(&mut ctx, "assembly_insert_standard", json!({ "component": "Hex socket head cap screw ISO 4762", "size": "M99" })).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("size"));
}

/// Assembly configurations: a List input suppressing an instance in one
/// configuration (Configure suppression), the active configuration.
#[test]
fn assembly_configurations() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "asm-config.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }));
    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }))["instance"].as_str().unwrap().to_string();
    let i = call(&mut ctx, "config_add_input", json!({ "tab": "Assembly 1", "kind": "list" }));
    let input = i["input"].as_str().unwrap().to_string();
    assert_eq!(i["configuration"]["inputs"][0]["name"], "Configuration", "{i}");
    let lite = call(&mut ctx, "config_add_option", json!({ "tab": "Assembly 1", "input": input, "name": "Lite" }))["option"].as_str().unwrap().to_string();
    call(&mut ctx, "config_configure", json!({ "tab": "Assembly 1", "instance": b, "param": "suppressed", "input": input }));
    assert!(last(&mut ctx).ends_with(":: Configure suppression : Part Studio 1 <2>"), "{}", last(&mut ctx));
    let v = call(&mut ctx, "config_set_value", json!({ "tab": "Assembly 1", "instance": b, "param": "suppressed", "configuration": "Lite", "value": true }));
    assert_eq!(v["configuration"]["configured"][0]["values"][&lite], true, "{v}");
    let suppressed = |ctx: &mut Ctx| call(ctx, "assembly_list", json!({}))["instances"].as_array().unwrap().iter().find(|x| x["id"] == b).unwrap()["suppressed"].clone();
    assert_eq!(suppressed(&mut ctx), false);
    call(&mut ctx, "config_activate", json!({ "tab": "Assembly 1", "values": { input.clone(): "Lite" } }));
    assert_eq!(suppressed(&mut ctx), true, "suppressed in Lite");
    call(&mut ctx, "config_activate", json!({ "tab": "Assembly 1", "reset": true }));
    assert_eq!(suppressed(&mut ctx), false);
    // Instances configure only their suppression.
    let e = try_call(&mut ctx, "config_configure", json!({ "tab": "Assembly 1", "instance": b, "param": "offset", "input": input })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("param"));
    let l = call(&mut ctx, "config_list", json!({ "tab": "Assembly 1" }));
    assert_eq!(l["configured"][0]["param"], "suppressed", "{l}");
}

/// The Insert view panel's options: a sketch view, simplification, model
/// annotations, sketch appearances.
#[test]
fn drawing_insert_view_options() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "insert-view-options.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let v = call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "position": [100, 150], "simplification": "absolute", "simplification_value": "2 mm", "model_annotations": false, "sketch_appearances": true }));
    let base = v["views"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(base["model_annotations"], false, "{base}");
    assert_eq!(base["sketch_appearances"], true, "{base}");
    assert!(base["simplification"].as_str().unwrap().starts_with("Absolute("), "{base}");
    let s = call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "sketch": "Sketch 1", "position": [300, 150] }));
    let sk = s["views"].as_array().unwrap().last().unwrap().clone();
    assert!(sk["sketch"].is_string(), "a sketch view: {sk}");
    // A detail view with a rectangle profile.
    let det = call(&mut ctx, "drawing_detail_view", json!({ "drawing": tab, "parent": base["id"], "profile": "rectangle", "points": [[90, 140], [110, 160]], "position": [200, 260] }));
    assert_eq!(det["views"].as_array().unwrap().last().unwrap()["kind"], "detail", "{det}");
    let e = try_call(&mut ctx, "drawing_detail_view", json!({ "drawing": tab, "parent": base["id"], "profile": "polygon", "points": [[90, 140], [110, 160]], "position": [200, 260] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("points"));
    let e = try_call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "named_view": "Nope", "position": [200, 60] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("named_view"));
    let e = try_call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "simplification": "auto", "simplification_value": "1 mm", "position": [200, 60] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("simplification_value"));
}

/// feature_add makes the dialog's own choices for fields not given: an
/// Extrude from a sketch on a part's face adds to that part (merge scope
/// the part); Remove there cuts into it.
#[test]
fn feature_add_makes_the_dialogs_choices() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "dialog-logic.noshape");
    let faces = call(&mut ctx, "query_entities", json!({ "kind": "faces" }));
    let top = faces["faces"].as_array().unwrap().iter().find(|f| f["surface"] == "plane" && num(&f["normal"][2]) > 0.99).unwrap()["id"].as_str().unwrap().to_string();
    let volume = |ctx: &mut Ctx| num(&call(ctx, "query_mass_properties", json!({}))["volume"]);
    let v0 = volume(&mut ctx);
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": top }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[25, 0], [30, 0]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    let ex = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "5 mm" } }));
    let f = call(&mut ctx, "feature_get", json!({ "feature": ex["feature"] }));
    assert_eq!(f["params"]["operationType"], "ADD", "{f}");
    assert_eq!(f["params"]["booleanScope"].as_array().map(|a| a.len()), Some(1), "{f}");
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 1, "added to the block");
    let v1 = volume(&mut ctx);
    assert!((v1 - v0 - std::f64::consts::PI * 25.0 * 5.0).abs() < 0.5, "{v0} -> {v1}");
    // Remove from the same face: points into the part.
    let sk2 = call(&mut ctx, "sketch_create", json!({ "plane": top }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk2, "tool": "circle", "points": [[-25, 0], [-30, 0]] }));
    let r2 = call(&mut ctx, "sketch_get", json!({ "sketch": sk2 }))["regions"][0]["id"].as_str().unwrap().to_string();
    let cut = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [r2], "depth": "5 mm", "operationType": "REMOVE" } }));
    assert_eq!(cut["feature_status"]["status"], "Ok", "{cut}");
    let g = call(&mut ctx, "feature_get", json!({ "feature": cut["feature"] }));
    assert_eq!(g["params"]["oppositeDirection"], true, "into the part: {g}");
    let v2 = volume(&mut ctx);
    assert!((v1 - v2 - std::f64::consts::PI * 25.0 * 5.0).abs() < 0.5, "{v1} -> {v2}");
    // A field the caller gives is kept.
    let sk3 = call(&mut ctx, "sketch_create", json!({ "plane": top }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk3, "tool": "circle", "points": [[0, 12], [0, 15]] }));
    let r3 = call(&mut ctx, "sketch_get", json!({ "sketch": sk3 }))["regions"][0]["id"].as_str().unwrap().to_string();
    let n = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [r3], "depth": "5 mm", "operationType": "NEW" } }));
    assert_eq!(call(&mut ctx, "feature_get", json!({ "feature": n["feature"] }))["params"]["operationType"], "NEW");
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 2);
}

/// The sketch dimension's menu: driven / driving, tolerance and precision
/// (a diameter; line lengths take none, as the menu), configure /
/// unconfigure.
#[test]
fn sketch_dimension_menu() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "dim-menu.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    let circles = entities(&mut ctx, &sk, "circle");
    let d = call(&mut ctx, "sketch_dimension", json!({ "sketch": sk, "entities": [circles[0]["id"]], "value": "10 mm" }));
    let dim = d["dimension"].as_str().map(str::to_string).unwrap_or_else(|| d["constraints"].as_array().unwrap().iter().rev().find(|c| c["id"].as_str().unwrap().starts_with("dim:")).unwrap()["id"].as_str().unwrap().to_string());
    let find = |v: &Value| v["constraints"].as_array().unwrap().iter().find(|c| c["id"] == dim.as_str()).unwrap().clone();
    let t = call(&mut ctx, "sketch_dimension_options", json!({ "sketch": sk, "dimension": dim, "tolerance": "symmetrical", "upper": "0.1 mm", "precision": 2 }));
    let c = find(&t);
    assert_eq!(c["tolerance"]["type"], "symmetrical", "{c}");
    assert_eq!(c["tolerance"]["precision"], 2, "{c}");
    let t = call(&mut ctx, "sketch_dimension_options", json!({ "sketch": sk, "dimension": dim, "driving": false }));
    assert_eq!(find(&t)["driving"], false);
    call(&mut ctx, "sketch_dimension_options", json!({ "sketch": sk, "dimension": dim, "driving": true }));
    // Configure with a List input, then give the second configuration 60 mm.
    let input = call(&mut ctx, "config_add_input", json!({ "kind": "list", "name": "Size" }))["input"].as_str().unwrap().to_string();
    let big = call(&mut ctx, "config_add_option", json!({ "input": input, "name": "Big" }))["option"].as_str().unwrap().to_string();
    let c = call(&mut ctx, "sketch_dimension_options", json!({ "sketch": sk, "dimension": dim, "configure": "Size" }));
    let key = c["configured_param"].as_str().unwrap().to_string();
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(last.ends_with(":: Configure dimension"), "{last}");
    call(&mut ctx, "config_set_value", json!({ "feature": sk, "param": key, "configuration": big, "value": "16 mm" }));
    let l = call(&mut ctx, "config_list", json!({}));
    let col = l["configured"].as_array().unwrap().iter().find(|c| c["param"] == key.as_str()).unwrap().clone();
    assert_eq!(col["values"][&big], "16 mm", "{col}");
    call(&mut ctx, "sketch_dimension_options", json!({ "sketch": sk, "dimension": dim, "unconfigure": true }));
    assert!(call(&mut ctx, "config_list", json!({}))["configured"].as_array().unwrap().iter().all(|c| c["param"] != key.as_str()));
    let e = try_call(&mut ctx, "sketch_dimension_options", json!({ "sketch": sk, "dimension": dim })).unwrap_err();
    assert!(e.message.contains("nothing to change"), "{e:?}");
}

/// Part Studio menus: copy / paste sketch entities, Size to Part Studio,
/// a section view render, named views.
#[test]
fn part_studio_menu_ops() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "ps-menus.noshape");
    // Copy the hole circle into a new sketch on Front, moved 10 mm.
    let circle = entities(&mut ctx, "Sketch 1", "circle")[0]["id"].as_str().unwrap().to_string();
    let front = call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }))["sketch"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "sketch_copy_entities", json!({ "sketch": "Sketch 1", "entities": [circle], "to": front, "offset": [10, 0] }));
    assert!(p["created"].as_array().unwrap().iter().any(|e| e["type"] == "circle"), "{p}");
    let c = entities(&mut ctx, &front, "circle");
    assert!(near(&c[0]["center"], 10.0, 0.0), "{c:?}");
    // In the same sketch.
    let same = call(&mut ctx, "sketch_copy_entities", json!({ "sketch": "Sketch 1", "entities": [circle], "offset": [0, 12] }));
    assert_eq!(same["sketch"], call(&mut ctx, "feature_get", json!({ "feature": "Sketch 1" }))["id"]);
    // Size to Part Studio.
    let s = call(&mut ctx, "feature_size_plane", json!({ "plane": "plane:top" }));
    assert!(num(&s["half_width"]) > 40.0 && num(&s["half_height"]) > 20.0, "{s}");
    assert!(try_call(&mut ctx, "feature_size_plane", json!({ "plane": "face:nope" })).is_err());
    // A section through the hole, and a named view keeping it.
    let img = call(&mut ctx, "render_image", json!({ "view": "isometric", "width": 200, "height": 150, "section": [{ "plane": "plane:front" }] }));
    assert!(num(&img["bytes"]) > 100.0, "{img}");
    let e = try_call(&mut ctx, "render_image", json!({ "section": [{ "plane": "entity:nope" }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("section[0].plane"));
    let v = call(&mut ctx, "render_named_view", json!({ "name": "Cut", "view": "front", "section": [{ "plane": "plane:right", "flip": true }] }));
    assert_eq!(v["saved"], "Cut");
    call(&mut ctx, "render_image", json!({ "view": "Cut", "width": 200, "height": 150 }));
    call(&mut ctx, "render_named_view", json!({ "name": "Cut", "delete": true }));
    assert!(try_call(&mut ctx, "render_image", json!({ "view": "Cut" })).is_err());
}

/// render_image of an assembly: what the Assembly view shows, with a section.
#[test]
fn render_an_assembly() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "asm-render.noshape");
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }));
    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_transform", json!({ "instance": b, "translation": ["100 mm", 0, 0] }));
    let file = scratch("asm.png");
    let r = call(&mut ctx, "render_image", json!({ "tab": "Assembly 1", "view": "isometric", "width": 320, "height": 240, "file": file.to_str().unwrap() }));
    assert_eq!(r["instances"], 2, "{r}");
    assert_eq!(&std::fs::read(&file).unwrap()[1..4], b"PNG");
    let s = call(&mut ctx, "render_image", json!({ "tab": "Assembly 1", "view": "front", "width": 320, "height": 240, "section": [{ "plane": "plane:front" }], "section_exclude": [b] }));
    assert!(num(&s["bytes"]) > 100.0, "{s}");
    let e = try_call(&mut ctx, "render_image", json!({ "tab": "Assembly 1", "section": [{ "plane": "face:x" }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("section[0].plane"));
    let e = try_call(&mut ctx, "render_image", json!({ "tab": "Assembly 1", "view": "My view" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"));
}

/// Thickness analysis and the curvature color map as numbers.
#[test]
fn thickness_and_curvature_analysis() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "thickness.noshape");
    let t = call(&mut ctx, "analysis_thickness", json!({ "method": "ray", "below": "15 mm", "points": 3 }));
    let p = &t["parts"][0];
    // A 80 x 40 x 20 block: rays through it measure 20 mm (top / bottom) up to 80.
    assert!(num(&p["min"]) > 1.0 && num(&p["min"]) < 21.0, "{t}");
    assert!(num(&p["max"]) >= 19.9, "{t}");
    assert!(num(&p["below_share"]) > 0.0, "{t}");
    assert!(p["thinnest"].as_array().unwrap().len() <= 3, "{t}");
    let e = try_call(&mut ctx, "analysis_thickness", json!({ "method": "x" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("method"));
    // The Ø10 hole: its smallest radius is 5 mm.
    let c = call(&mut ctx, "analysis_curvature_map", json!({ "kind": "min_radius", "points": 1 }));
    let p = &c["parts"][0];
    assert!((num(&p["min"]) - 5.0).abs() < 0.5, "{c}");
    assert!(num(&p["flat_share"]) > 0.5, "mostly flat: {c}");
}

/// The Configurations panel's inputs between Part Studios: copy / paste,
/// input propagation to a Derived feature, FeatureScript IDs, visibility.
#[test]
fn configuration_panel_inputs() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "config-panel.noshape");
    call(&mut ctx, "config_add_input", json!({ "kind": "list", "name": "Size" }));
    call(&mut ctx, "config_add_option", json!({ "input": "Size", "name": "Large" }));
    let holder = call(&mut ctx, "tab_create", json!({ "kind": "Part Studio", "name": "Holder" }));
    let info = call(&mut ctx, "document_info", json!({}));
    let ps1 = info["tabs"].as_array().unwrap().iter().find(|t| t["name"] == "Part Studio 1").unwrap()["id"].as_str().unwrap().to_string();
    let _ = holder;
    let c = call(&mut ctx, "config_copy_inputs", json!({ "from": "Part Studio 1", "tab": "Holder" }));
    assert_eq!(c["pasted"], json!(["Size"]), "{c}");
    // Again: nothing new to paste.
    let again = call(&mut ctx, "config_copy_inputs", json!({ "from": "Part Studio 1", "tab": "Holder" }));
    assert_eq!(again["pasted"], json!([]));
    // A Derived feature of Part Studio 1 follows Holder's Size.
    call(&mut ctx, "feature_add", json!({ "tab": "Holder", "type": "importDerived", "params": { "partStudio": ps1 } }));
    let p = call(&mut ctx, "config_propagate", json!({ "tab": "Holder", "input": "Size" }));
    assert_eq!(p["candidates"][0]["propagated"], true, "{p}");
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(last.ends_with(":: Input propagation"), "{last}");
    // FeatureScript IDs.
    let e = call(&mut ctx, "config_edit_ids", json!({ "input": "Size", "id": "size_class", "options": { "Large": "large" } }));
    let inputs = e["configuration"]["inputs"].as_array().unwrap().clone();
    let size = inputs.iter().find(|i| i["name"] == "Size").unwrap();
    assert_eq!(size["id"], "size_class", "{size}");
    assert!(size["configurations"].as_array().unwrap().iter().any(|o| o["id"] == "large"), "{size}");
    assert_eq!(try_call(&mut ctx, "config_edit_ids", json!({ "input": "Size", "id": "2bad" })).unwrap_err().param.as_deref(), Some("id"));
    // Visibility: Size shows only with Lite unchecked.
    let lite = call(&mut ctx, "config_add_input", json!({ "kind": "checkbox", "name": "Lite" }))["input"].as_str().unwrap().to_string();
    let e = try_call(&mut ctx, "config_visibility", json!({ "input": "Size", "visibility": { "conditions": [{ "input": lite, "test": { "Is": ["false"] } }] } })).unwrap_err();
    assert!(e.message.contains("not above"), "{e:?}");
    call(&mut ctx, "config_reorder_inputs", json!({ "input": lite, "to": "Size" }));
    let v = call(&mut ctx, "config_visibility", json!({ "input": "Size", "visibility": { "conditions": [{ "input": lite, "test": { "Is": ["false"] } }], "option_rules": [] } }));
    let size = v["configuration"]["inputs"].as_array().unwrap().iter().find(|i| i["name"] == "Size").unwrap().clone();
    assert!(size["visible_when"].is_object(), "{size}");
    let v = call(&mut ctx, "config_visibility", json!({ "input": "Size", "visibility": null }));
    assert!(v["configuration"]["inputs"].as_array().unwrap().iter().find(|i| i["name"] == "Size").unwrap().get("visible_when").is_none());
    assert_eq!(try_call(&mut ctx, "config_visibility", json!({ "input": "Size", "visibility": { "conditions": [{ "input": "nope", "test": { "Is": ["x"] } }] } })).unwrap_err().param.as_deref(), Some("visibility"));
}

/// The view context menu: Show/hide, orientation, tangent edges, rotate,
/// order, align, alignment, View properties, Show/hide dialogs, copy, move
/// to sheet and Delete, each with the editor's edit text.
#[test]
fn drawing_view_menu_edits() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_view_menu.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx, n: usize| call(ctx, "history_list", json!({ "limit": n }))["history"].as_array().unwrap().iter().map(|h| h["description"].as_str().unwrap().rsplit(":: ").next().unwrap().to_string()).collect::<Vec<_>>();
    let get = |ctx: &mut Ctx, edges: bool| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": edges }));
    let view = |ctx: &mut Ctx, id: &Value| get(ctx, false)["views"].as_array().unwrap().iter().find(|v| v["id"] == *id).cloned().unwrap_or(Value::Null);
    let g = get(&mut ctx, false);
    let vs = g["views"].as_array().unwrap().clone();
    let base = vs.iter().find(|v| v["kind"] == "base").unwrap()["id"].clone();
    let child = vs.iter().find(|v| v["kind"] == "projected").unwrap()["id"].clone();
    // A view of its own (no parent, no children), free to turn.
    let f = call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "position": [330, 230] }));
    let free = f["views"].as_array().unwrap().last().unwrap()["id"].clone();

    // Display: one edit per choice, in order.
    call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": base, "tangent_edges": "phantom", "hidden_lines": true, "threads": false }));
    assert_eq!(last(&mut ctx, 3), ["Threads", "Hidden lines", "Tangent edges"]);
    let b = view(&mut ctx, &base);
    assert_eq!(b["tangent_edges"], "phantom", "{b}");
    assert_eq!(b["style"], "HiddenLinesVisible", "{b}");
    call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": free, "orientation": "isometric" }));
    assert_eq!(last(&mut ctx, 1), ["View orientation"]);
    assert_eq!(view(&mut ctx, &free)["orientation"], "isometric");
    let e = try_call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": free, "orientation": "Nope" })).unwrap_err();
    assert!(e.message.contains("isometric"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": child, "orientation": "top" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("orientation"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": base, "bend_lines": false })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("bend_lines"), "{e:?}");
    call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": free, "orientation": "front" }));

    // Rotate: by, to, then to a straight edge.
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "rotate", "by": 90 }));
    assert_eq!(last(&mut ctx, 1), ["Rotate view"]);
    assert!((num(&view(&mut ctx, &free)["rotation"]) - 90.0).abs() < 1e-9);
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "rotate", "to": 30 }));
    let fv = get(&mut ctx, true)["views"].as_array().unwrap().iter().find(|v| v["id"] == free).cloned().unwrap();
    assert!((num(&fv["rotation"]) - 30.0).abs() < 1e-9, "{fv}");
    let line = fv["edges"].as_array().unwrap().iter().find(|e| e["type"] == "line").cloned().unwrap();
    let mid = [(num(&line["a"][0]) + num(&line["b"][0])) / 2.0, (num(&line["a"][1]) + num(&line["b"][1])) / 2.0];
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "rotate_to_edge", "edge": mid, "direction": "horizontal" }));
    assert_eq!(last(&mut ctx, 1), ["Rotate view horizontally"]);
    let rot = view(&mut ctx, &free)["rotation"].as_f64().unwrap_or(0.0);
    assert!((rot / 90.0 - (rot / 90.0).round()).abs() < 1e-6, "{rot}");
    let e = try_call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": base, "action": "rotate_to_edge", "edge": mid, "direction": "vertical" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"), "{e:?}");
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "rotate", "to": 0 }));
    assert!(view(&mut ctx, &free)["rotation"].is_null());

    // Order and align.
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "order", "to": "back" }));
    assert_eq!(last(&mut ctx, 1), ["Send to back"]);
    assert_eq!(get(&mut ctx, false)["views"][0]["id"], free);
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "align", "with": base, "direction": "vertical" }));
    assert_eq!(last(&mut ctx, 1), ["Align views"]);
    assert!((num(&view(&mut ctx, &free)["position"][0]) - num(&view(&mut ctx, &base)["position"][0])).abs() < 1e-9);

    // Alignment with the parent.
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": child, "action": "suppress_alignment" }));
    assert_eq!(last(&mut ctx, 1), ["Suppress alignment with parent"]);
    assert_eq!(view(&mut ctx, &child)["unaligned"], true);
    let e = try_call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": base, "action": "suppress_alignment" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": base, "action": "remove_crop" })).unwrap_err();
    assert!(e.message.contains("not cropped"), "{e:?}");

    // View properties: read, then name, scale and label.
    let p = call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free }));
    assert_eq!(p["properties"]["name"], "Front", "{p}");
    assert_eq!(p["properties"]["orientation"], "front", "{p}");
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free, "set": { "name": "Main", "scale": "1:2", "view_label": true, "tangent_edges": "hidden" } }));
    assert_eq!(last(&mut ctx, 1), ["View properties"]);
    let fv = view(&mut ctx, &free);
    assert_eq!(fv["name"], "Main", "{fv}");
    assert_eq!(fv["label"], "MAIN", "{fv}");
    assert!((num(&fv["scale"]) - 0.5).abs() < 1e-12, "{fv}");
    assert_eq!(fv["tangent_edges"], "hidden", "{fv}");
    let e = try_call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free, "set": { "bogus": 1 } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.bogus"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": child, "set": { "rotation": 45 } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.rotation"), "{e:?}");

    // Show/hide parts and edges (an edge hidden, then shown again).
    call(&mut ctx, "drawing_view_show_hide", json!({ "drawing": tab, "view": free, "hidden_parts": ["Part 1"] }));
    assert_eq!(last(&mut ctx, 1), ["Show/hide parts"]);
    assert_eq!(view(&mut ctx, &free)["hidden_parts"], 1);
    let e = try_call(&mut ctx, "drawing_view_show_hide", json!({ "drawing": tab, "view": free, "hidden_parts": ["Nope"] })).unwrap_err();
    assert!(e.message.contains("Part 1"), "{e:?}");
    call(&mut ctx, "drawing_view_show_hide", json!({ "drawing": tab, "view": free, "hidden_parts": [] }));
    let fv = get(&mut ctx, true)["views"].as_array().unwrap().iter().find(|v| v["id"] == free).cloned().unwrap();
    let line = fv["edges"].as_array().unwrap().iter().find(|e| e["type"] == "line").cloned().unwrap();
    let mid = [(num(&line["a"][0]) + num(&line["b"][0])) / 2.0, (num(&line["a"][1]) + num(&line["b"][1])) / 2.0];
    call(&mut ctx, "drawing_view_show_hide", json!({ "drawing": tab, "view": free, "toggle_edges": [mid] }));
    assert_eq!(last(&mut ctx, 1), ["Show/hide edges"]);
    assert_eq!(view(&mut ctx, &free)["hidden_edges"], 1);
    call(&mut ctx, "drawing_view_show_hide", json!({ "drawing": tab, "view": free, "toggle_edges": [mid] }));
    assert!(view(&mut ctx, &free)["hidden_edges"].is_null());

    // Copy / paste, then Delete the copy.
    let n = get(&mut ctx, false)["views"].as_array().unwrap().len();
    let c = call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "copy", "at": [330, 80] }));
    assert_eq!(last(&mut ctx, 1), ["Paste views"]);
    let pasted = c["pasted"][0].clone();
    assert_eq!(get(&mut ctx, false)["views"].as_array().unwrap().len(), n + 1);
    call(&mut ctx, "drawing_delete", json!({ "drawing": tab, "views": [pasted] }));
    assert_eq!(last(&mut ctx, 1), ["Delete"]);
    assert_eq!(get(&mut ctx, false)["views"].as_array().unwrap().len(), n);
    let e = try_call(&mut ctx, "drawing_delete", json!({ "drawing": tab, "views": [pasted] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("views[0]"), "{e:?}");

    // Move to a new sheet.
    let m = call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "move_to_sheet" }));
    assert_eq!(last(&mut ctx, 1), ["Move to sheet"]);
    assert_eq!(m["sheets"].as_array().unwrap().len(), 2, "{m}");
    assert_eq!(m["sheet"], 2, "{m}");
    let first = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "sheet": 1, "edges": false }));
    assert_eq!(first["views"].as_array().unwrap().len(), n - 1, "{first}");
}

/// The Sheets panel menu and Sheet properties.
#[test]
fn drawing_sheet_menu_and_properties() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_sheets.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().rsplit(":: ").next().unwrap().to_string();
    let sheets = |r: &Value| r["sheets"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect::<Vec<_>>();
    let r = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "action": "add" }));
    assert_eq!(last(&mut ctx), "Add sheet");
    assert_eq!(sheets(&r), ["Sheet1", "Sheet2"], "{r}");
    assert_eq!(r["sheet"], 2);
    call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": 2, "action": "rename", "name": "Details" }));
    assert_eq!(last(&mut ctx), "Rename sheet");
    let r = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "duplicate" }));
    assert_eq!(last(&mut ctx), "Duplicate sheet");
    assert_eq!(sheets(&r), ["Sheet1", "Sheet1 copy", "Details"], "{r}");
    assert_eq!(r["views"].as_array().unwrap().len(), d["views"].as_array().unwrap().len(), "the copy has the views: {r}");
    let r = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Details", "action": "move", "to": 1 }));
    assert_eq!(last(&mut ctx), "Move sheet");
    assert_eq!(sheets(&r), ["Details", "Sheet1", "Sheet1 copy"], "{r}");
    let r = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Details", "action": "copy" }));
    assert_eq!(last(&mut ctx), "Paste sheet");
    assert_eq!(sheets(&r).last().unwrap(), "Details (1)");
    let r = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Details (1)", "action": "delete" }));
    assert_eq!(last(&mut ctx), "Delete sheet");
    assert_eq!(sheets(&r).len(), 3);
    let e = try_call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "action": "move", "to": 9 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("to"), "{e:?}");

    // Sheet properties: read, then scale, size, Custom with zones.
    let p = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties" }));
    assert_eq!(p["properties"]["size"], "A3", "{p}");
    let p = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties", "set": { "scale": "1:2", "size": "A2" } }));
    assert_eq!(last(&mut ctx), "Sheet size");
    assert_eq!(p["properties"]["scale"], "1:2", "{p}");
    assert_eq!(p["properties"]["size"], "A2", "{p}");
    let e = try_call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties", "set": { "size": "B" } })).unwrap_err();
    assert!(e.message.contains("A4 Portrait"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties", "set": { "zones": [4, 4] } })).unwrap_err();
    assert!(e.message.contains("Custom"), "{e:?}");
    let p = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties", "set": { "size": "Custom", "width": 500, "zones": [8, 4], "border": false } }));
    assert_eq!(last(&mut ctx), "Sheet border");
    let pr = &p["properties"];
    assert_eq!(pr["size"], "Custom", "{p}");
    assert_eq!(pr["width"], 500.0, "{p}");
    assert_eq!(pr["height"], 420.0, "{p}");
    assert_eq!(pr["zones"], json!([8, 4]), "{p}");
    assert_eq!(pr["border"], false, "{p}");
}

/// The table menus on a general table and a BOM table.
#[test]
fn drawing_table_menu_edits() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_table_menu.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let base = d["views"][0]["id"].clone();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().rsplit(":: ").next().unwrap().to_string();
    let gt = call(&mut ctx, "drawing_table", json!({ "drawing": tab, "position": [200, 60], "rows": 3, "columns": 3, "header_row": true }));
    let gid = gt["table"].clone();
    let table = |ctx: &mut Ctx, id: &Value| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["tables"].as_array().unwrap().iter().find(|t| t["id"] == *id).cloned().unwrap();
    let edit = |ctx: &mut Ctx, args: Value| {
        let mut a = json!({ "drawing": tab, "table": gid });
        a.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
        try_call(ctx, "drawing_table_edit", a)
    };
    let widths = |t: &Value| t["column_widths"].as_array().unwrap().iter().map(num).collect::<Vec<f64>>();

    edit(&mut ctx, json!({ "action": "merge_right", "row": 1, "column": 0 })).unwrap();
    assert_eq!(last(&mut ctx), "Merge cells");
    assert_eq!(table(&mut ctx, &gid)["merges"], json!([{ "row": 1, "column": 0, "rows": 1, "columns": 2 }]));
    edit(&mut ctx, json!({ "action": "unmerge", "row": 1, "column": 1 })).unwrap();
    assert_eq!(last(&mut ctx), "Unmerge");
    assert!(table(&mut ctx, &gid)["merges"].is_null());
    edit(&mut ctx, json!({ "action": "merge", "row": 1, "column": 0, "to_row": 2, "to_column": 1 })).unwrap();
    assert_eq!(last(&mut ctx), "All");
    assert_eq!(table(&mut ctx, &gid)["merges"][0]["rows"], 2);
    edit(&mut ctx, json!({ "action": "unmerge", "row": 1, "column": 0 })).unwrap();
    let e = edit(&mut ctx, json!({ "action": "merge", "row": 1, "column": 0 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("action"), "{e:?}");

    edit(&mut ctx, json!({ "action": "insert", "row": 0, "column": 2, "at": "column_right" })).unwrap();
    assert_eq!(last(&mut ctx), "Column Right");
    assert_eq!(widths(&table(&mut ctx, &gid)).len(), 4);
    edit(&mut ctx, json!({ "action": "remove", "row": 0, "column": 3, "what": "columns" })).unwrap();
    assert_eq!(last(&mut ctx), "Columns");
    assert_eq!(widths(&table(&mut ctx, &gid)).len(), 3);
    let rows0 = table(&mut ctx, &gid)["rows"].as_array().unwrap().len();
    edit(&mut ctx, json!({ "action": "duplicate", "row": 1, "column": 0, "what": "rows" })).unwrap();
    assert_eq!(last(&mut ctx), "Rows");
    assert_eq!(table(&mut ctx, &gid)["rows"].as_array().unwrap().len(), rows0 + 1);

    edit(&mut ctx, json!({ "action": "resize", "column": 0, "width": 40 })).unwrap();
    assert_eq!(last(&mut ctx), "Resize cell");
    assert!((widths(&table(&mut ctx, &gid))[0] - 40.0).abs() < 1e-9);
    edit(&mut ctx, json!({ "action": "columns_equally" })).unwrap();
    assert_eq!(last(&mut ctx), "Size columns equally");
    let w = widths(&table(&mut ctx, &gid));
    assert!(w.iter().all(|x| (x - w[0]).abs() < 1e-6), "{w:?}");
    edit(&mut ctx, json!({ "action": "format_cell", "row": 0, "column": 0, "scope": "row", "align": "left", "bold": true })).unwrap();
    assert_eq!(last(&mut ctx), "Format cells");

    let before = table(&mut ctx, &gid)["bounds"].clone();
    edit(&mut ctx, json!({ "action": "fixed_corner", "to": "bottom_left" })).unwrap();
    assert_eq!(last(&mut ctx), "Table fixed corner");
    let t = table(&mut ctx, &gid);
    assert_eq!(t["corner"], "BottomLeft", "{t}");
    for k in ["min", "max"] {
        for i in 0..2 {
            assert!((num(&t["bounds"][k][i]) - num(&before[k][i])).abs() < 1e-6, "{t} {before}");
        }
    }
    edit(&mut ctx, json!({ "action": "text_height", "to": 5 })).unwrap();
    assert_eq!(last(&mut ctx), "Table text height");
    assert_eq!(table(&mut ctx, &gid)["text_height"], 5.0);
    let e = edit(&mut ctx, json!({ "action": "text_height", "to": 4 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("to"), "{e:?}");
    edit(&mut ctx, json!({ "action": "fit" })).unwrap();
    assert_eq!(last(&mut ctx), "Fit table");
    let e = edit(&mut ctx, json!({ "action": "flip_x" })).unwrap_err();
    assert!(e.message.contains("hole tables"), "{e:?}");
    let e = edit(&mut ctx, json!({ "action": "merge_right", "row": 1, "column": 2 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("action"), "{e:?}");

    // A BOM table: type and order; no cell formats.
    let bt = call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "bom", "view": base, "position": [20, 280] }));
    let bid = bt["table"].clone();
    call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "bom_type", "to": "structured" }));
    assert_eq!(last(&mut ctx), "BOM type");
    call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "order", "to": "bottom_to_top" }));
    assert_eq!(last(&mut ctx), "BOM order");
    let t = table(&mut ctx, &bid);
    assert_eq!((t["structured"].clone(), t["bottom_to_top"].clone()), (json!(true), json!(true)), "{t}");
    let e = try_call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "format_cell", "row": 0, "column": 0 })).unwrap_err();
    assert!(e.message.contains("BOM"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "split", "row": 1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("row"), "{e:?}");
}

/// A Part Studio with two parts (a block with a hole and a pin) inserted
/// in an assembly ("Assembly 1"): their instance ids.
fn two_part_assembly(ctx: &mut Ctx, file: &str) -> (String, String) {
    let path = scratch(file);
    call(ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Block" }))["sketch"].as_str().unwrap().to_string();
    call(ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-20, -20], [20, 20]] }));
    call(ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    let st = call(ctx, "sketch_get", json!({ "sketch": sk }));
    let plate = st["regions"].as_array().unwrap().iter().find(|r| r["holes"] == 1).unwrap()["id"].as_str().unwrap().to_string();
    call(ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [plate], "depth": "20 mm" } }));
    let sk = call(ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Pin" }))["sketch"].as_str().unwrap().to_string();
    call(ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[60, 0], [65, 0]] }));
    let disc = call(ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [disc], "depth": "50 mm", "operationType": "NEW" } }));
    let parts = call(ctx, "query_parts", json!({}));
    let ids: Vec<String> = parts["parts"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids.len(), 2, "{parts}");
    let b = call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": ids[0] }))["instance"].as_str().unwrap().to_string();
    let p = call(ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": ids[1] }))["instance"].as_str().unwrap().to_string();
    (b, p)
}

/// Split BOM table below / Unsplit table on a two-item BOM.
#[test]
fn drawing_bom_split_and_unsplit() {
    let mut ctx = Ctx::new();
    two_part_assembly(&mut ctx, "drawing_bom_split.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Assembly 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let base = d["views"].as_array().unwrap().iter().find(|v| v["kind"] == "base").unwrap()["id"].clone();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().rsplit(":: ").next().unwrap().to_string();
    let tables = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["tables"].as_array().unwrap().clone();
    let bt = call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "bom", "view": base, "position": [20, 280] }));
    let bid = bt["table"].clone();
    let t = tables(&mut ctx).into_iter().find(|t| t["id"] == bid).unwrap();
    assert_eq!(t["rows"].as_array().unwrap().len(), 2, "two items: {t}");
    // The last data row can't be split off below itself.
    let e = try_call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "split", "row": 2 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("row"), "{e:?}");
    // Below the first item (displayed row 1; the header is row 0).
    let r = call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "split", "row": 1 }));
    assert_eq!(last(&mut ctx), "Split table");
    let second = r["table"].clone();
    assert_ne!(second, bid);
    let ts = tables(&mut ctx);
    let (a, b) = (ts.iter().find(|t| t["id"] == bid).unwrap(), ts.iter().find(|t| t["id"] == second).unwrap());
    assert_eq!(a["part_rows"], json!([0, 1]), "{a}");
    assert_eq!(b["part_rows"], json!([1, 2]), "{b}");
    assert_eq!((a["rows"].as_array().unwrap().len(), b["rows"].as_array().unwrap().len()), (1, 1), "{a} {b}");
    // Beside the first part, not on it.
    assert!(num(&b["bounds"]["min"][0]) >= num(&a["bounds"]["max"][0]) || num(&b["bounds"]["max"][0]) <= num(&a["bounds"]["min"][0]), "{a} {b}");
    // The first part can't be unsplit; the second goes back.
    let e = try_call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "unsplit" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("action"), "{e:?}");
    call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": second, "action": "unsplit" }));
    assert_eq!(last(&mut ctx), "Unsplit table");
    let ts = tables(&mut ctx);
    assert_eq!(ts.len(), 1, "{ts:?}");
    assert!(ts[0]["part_rows"].is_null(), "{ts:?}");
    assert_eq!(ts[0]["rows"].as_array().unwrap().len(), 2);
}

/// Modeling in context: Edit in context, context geometry referenced by a
/// feature, Update / Rename / Exit / Show / Delete, Set as primary
/// instance, and Create Part Studio in context with Insert and go to
/// assembly.
#[test]
fn edit_in_context_and_part_studio_in_context() {
    let mut ctx = Ctx::new();
    let (block, pin) = two_part_assembly(&mut ctx, "in_context.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().rsplit(":: ").next().unwrap().to_string();
    let contexts = |ctx: &mut Ctx, tab: &str| call(ctx, "context_list", json!({ "tab": tab }))["contexts"].as_array().unwrap().clone();

    let r = call(&mut ctx, "assembly_edit_in_context", json!({ "instance": block }));
    assert_eq!(r["part_studio"], "Part Studio 1", "{r}");
    let c = &r["context"];
    assert_eq!((c["name"].clone(), c["shown"].clone(), c["referenced"].clone()), (json!("Context 1"), json!(true), json!(false)), "{r}");
    assert_eq!(c["instances"].as_array().unwrap().len(), 1, "the pin around the block: {r}");
    // The pin's top face, from the context.
    let faces = call(&mut ctx, "query_entities", json!({ "tab": "Part Studio 1", "kind": "faces", "context": true, "surface": "plane", "normal": [0, 0, 1] }));
    let top = faces["faces"][0]["id"].as_str().unwrap_or_else(|| panic!("{faces}")).to_string();
    let e = try_call(&mut ctx, "query_entities", json!({ "tab": "Part Studio 1", "kind": "faces", "context": true, "part": "Nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("part"), "{e:?}");
    // A sketch on it references the context: it persists.
    call(&mut ctx, "sketch_create", json!({ "tab": "Part Studio 1", "plane": top, "name": "On pin" }));
    let cs = contexts(&mut ctx, "Part Studio 1");
    assert_eq!(cs[0]["referenced"], true, "{cs:?}");
    assert_eq!(cs[0]["out_of_date"], false, "{cs:?}");

    // Moving the pin makes an update available.
    call(&mut ctx, "assembly_transform", json!({ "instance": pin, "translation": [0, 0, 15] }));
    assert_eq!(contexts(&mut ctx, "Part Studio 1")[0]["out_of_date"], true);
    call(&mut ctx, "context_update", json!({ "tab": "Part Studio 1", "context": "Context 1" }));
    assert_eq!(last(&mut ctx), "Update context : Context 1");
    assert_eq!(contexts(&mut ctx, "Part Studio 1")[0]["out_of_date"], false);
    call(&mut ctx, "context_rename", json!({ "tab": "Part Studio 1", "context": "Context 1", "name": "Housing" }));
    assert_eq!(last(&mut ctx), "Rename context");

    // Exit keeps a referenced context; show / None.
    let cs = call(&mut ctx, "context_exit", json!({ "tab": "Part Studio 1" }))["contexts"].as_array().unwrap().clone();
    assert_eq!((cs.len(), cs[0]["shown"].clone()), (1, json!(false)), "{cs:?}");
    let e = try_call(&mut ctx, "query_entities", json!({ "tab": "Part Studio 1", "kind": "faces", "context": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("context"), "{e:?}");
    assert_eq!(call(&mut ctx, "context_show", json!({ "tab": "Part Studio 1", "context": "Housing" }))["contexts"][0]["shown"], true);
    assert_eq!(call(&mut ctx, "context_show", json!({ "tab": "Part Studio 1", "context": null }))["contexts"][0]["shown"], false);

    // Edit in context again: Edit Housing, or a new context.
    let r = call(&mut ctx, "assembly_edit_in_context", json!({ "instance": block, "context": "Housing" }));
    assert_eq!(r["context"]["name"], "Housing", "{r}");
    let e = try_call(&mut ctx, "assembly_edit_in_context", json!({ "instance": block, "context": "Nope" })).unwrap_err();
    assert!(e.message.contains("Housing"), "{e:?}");
    call(&mut ctx, "assembly_update_context", json!({ "instance": block, "context": "Housing" }));
    assert_eq!(last(&mut ctx), "Update context : Housing");

    // Another instance of the block becomes the primary one.
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }));
    let block_part = parts["parts"][0]["id"].as_str().unwrap().to_string();
    let b2 = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": block_part }))["instance"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "assembly_set_primary_instance", json!({ "instance": b2, "context": "Housing" }));
    let b2_name = call(&mut ctx, "assembly_list", json!({}))["instances"].as_array().unwrap().iter().find(|i| i["id"] == b2).unwrap()["name"].clone();
    assert_eq!(r["context"]["primary"], b2_name, "{r}");
    let e = try_call(&mut ctx, "assembly_set_primary_instance", json!({ "instance": pin, "context": "Housing" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("instance"), "{e:?}");

    // Create Part Studio in context, model there, Insert and go to assembly.
    let e = try_call(&mut ctx, "assembly_create_part_studio_in_context", json!({ "origin": "Nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("origin"), "{e:?}");
    let r = call(&mut ctx, "assembly_create_part_studio_in_context", json!({}));
    // The new tab (undoable), then its context shown (not undoable), as in the app.
    let h = call(&mut ctx, "history_list", json!({ "limit": 2 }))["history"].as_array().unwrap().iter().map(|h| h["description"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(h, ["Edit in context", "Create Part Studio in context"], "{h:?}");
    let studio = r["part_studio"].as_str().unwrap().to_string();
    assert_eq!(studio, "Part Studio 2", "{r}");
    assert_eq!((r["context"]["origin"].clone(), r["context"]["instances"].as_array().unwrap().len()), (json!(true), 3), "{r}");
    let sk = call(&mut ctx, "sketch_create", json!({ "tab": studio, "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "tab": studio, "sketch": sk, "tool": "circle", "points": [[0, 0], [8, 0]] }));
    let disc = call(&mut ctx, "sketch_get", json!({ "tab": studio, "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "tab": studio, "type": "extrude", "params": { "entities": [disc], "depth": "5 mm" } }));
    let n = call(&mut ctx, "assembly_list", json!({}))["instances"].as_array().unwrap().len();
    let e = try_call(&mut ctx, "context_insert_in_assembly", json!({ "tab": studio })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("parts"), "{e:?}");
    let r = call(&mut ctx, "context_insert_in_assembly", json!({ "tab": studio, "parts": ["Part 1"] }));
    assert_eq!(last(&mut ctx), "Insert and go to assembly");
    assert_eq!(r["instances"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(call(&mut ctx, "assembly_list", json!({}))["instances"].as_array().unwrap().len(), n + 1);
    call(&mut ctx, "context_show", json!({ "tab": "Part Studio 1", "context": "Housing" }));
    let e = try_call(&mut ctx, "context_insert_in_assembly", json!({ "tab": "Part Studio 1", "parts": ["Part 1"] })).unwrap_err();
    assert!(e.message.contains("not created in context"), "{e:?}");

    call(&mut ctx, "context_delete", json!({ "tab": "Part Studio 1", "context": "Housing" }));
    assert_eq!(last(&mut ctx), "Delete context");
    assert!(contexts(&mut ctx, "Part Studio 1").is_empty());
}

/// The annotation menus: jogs, break, leaders and note nodes, foreshorten,
/// groups, detach, align centermark, stacked callouts, add to an ordinate
/// group, and Adjust linestyle.
#[test]
fn drawing_annotation_menu_edits() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_annotation_menu.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let (top, circle) = g["views"].as_array().unwrap().iter().find_map(|v| v["edges"].as_array().and_then(|es| es.iter().find(|e| e["type"] == "circle").map(|c| (v.clone(), c.clone())))).unwrap();
    let top_id = top["id"].clone();
    let (cx, cy, r) = (num(&circle["center"][0]), num(&circle["center"][1]), num(&circle["radius"]));
    let lines: Vec<Value> = top["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line").cloned().collect();
    let horizontal: Vec<&Value> = lines.iter().filter(|e| (num(&e["a"][1]) - num(&e["b"][1])).abs() < 1e-6).collect();
    let vertical: Vec<&Value> = lines.iter().filter(|e| (num(&e["a"][0]) - num(&e["b"][0])).abs() < 1e-6).collect();
    let p = |e: &Value, k: &str| [num(&e[k][0]), num(&e[k][1])];
    let quarter = |e: &Value| [num(&e["a"][0]) * 0.75 + num(&e["b"][0]) * 0.25, num(&e["a"][1]) * 0.75 + num(&e["b"][1]) * 0.25];
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().rsplit(":: ").next().unwrap().to_string();
    let ann = |ctx: &mut Ctx, id: &Value| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["annotations"].as_array().unwrap().iter().find(|a| a["id"] == *id).cloned().unwrap();
    let edit = |ctx: &mut Ctx, id: &Value, args: Value| {
        let mut a = json!({ "drawing": tab, "annotation": id });
        a.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
        try_call(ctx, "drawing_annotation_edit", a)
    };

    // Edge picks first, before annotations lie over the edges.
    // Adjust linestyle on an edge of the top view, then revert.
    let view = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"].as_array().unwrap().iter().find(|v| v["id"] == top_id).cloned().unwrap();
    call(&mut ctx, "drawing_view_linestyle", json!({ "drawing": tab, "view": top_id, "edges": [quarter(vertical[0])], "linestyle": "phantom", "thickness": 0.5 }));
    assert_eq!(last(&mut ctx), "Adjust linestyle");
    assert_eq!(view(&mut ctx)["edge_styles"], 1);
    let e = try_call(&mut ctx, "drawing_view_linestyle", json!({ "drawing": tab, "view": top_id, "edges": [quarter(vertical[0])], "thickness": 0.33 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("thickness"), "{e:?}");
    call(&mut ctx, "drawing_view_linestyle", json!({ "drawing": tab, "view": top_id, "edges": [quarter(vertical[0])], "revert": true }));
    assert!(view(&mut ctx)["edge_styles"].is_null());
    // Align centermark with a vertical edge.
    let cm = call(&mut ctx, "drawing_centermark", json!({ "drawing": tab, "holes": [[cx - r, cy]] }))["annotations"][0].clone();
    edit(&mut ctx, &cm, json!({ "action": "align_centermark", "point": quarter(vertical[0]) })).unwrap();
    assert_eq!(last(&mut ctx), "Align centermark");
    assert!((num(&ann(&mut ctx, &cm)["angle"]).abs() - 90.0).abs() < 1e-6);

    // A linear dimension: jog, break, detach.
    let h0 = horizontal[0];
    // Placed outside the part (away from the other horizontal edge).
    let out = if num(&h0["a"][1]) < num(&horizontal[1]["a"][1]) { -20.0 } else { 20.0 };
    let dim = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": [quarter(h0)], "place": [quarter(h0)[0], quarter(h0)[1] + out] }))["annotation"].clone();
    assert_eq!(ann(&mut ctx, &dim)["kind"], "LinearDimension");
    edit(&mut ctx, &dim, json!({ "action": "add_jog", "line": 0 })).unwrap();
    assert_eq!(last(&mut ctx), "Add jog");
    assert_eq!(ann(&mut ctx, &dim)["jogs"], json!([0]));
    let e = edit(&mut ctx, &dim, json!({ "action": "add_jog", "line": 0 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("action"), "{e:?}");
    edit(&mut ctx, &dim, json!({ "action": "remove_jog", "line": 0 })).unwrap();
    assert_eq!(last(&mut ctx), "Remove jog");
    assert!(ann(&mut ctx, &dim)["jogs"].is_null());
    edit(&mut ctx, &dim, json!({ "action": "break", "on": false })).unwrap();
    assert_eq!(last(&mut ctx), "Unbreak dimension");
    assert_eq!(ann(&mut ctx, &dim)["unbroken"], true);
    assert_eq!(ann(&mut ctx, &dim)["attached"], true);
    edit(&mut ctx, &dim, json!({ "action": "detach" })).unwrap();
    assert_eq!(last(&mut ctx), "Detach");
    assert!(ann(&mut ctx, &dim)["attached"].is_null());

    // A note with a leader: another leader, a node on it, then removals.
    let (a0, b0) = (p(h0, "a"), p(h0, "b"));
    let note = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "BREAK ALL EDGES", "position": [40, 40], "leader": a0 }))["annotation"].clone();
    edit(&mut ctx, &note, json!({ "action": "add_leader", "point": b0 })).unwrap();
    assert_eq!(last(&mut ctx), "Add leader");
    let n = ann(&mut ctx, &note);
    assert_eq!(n["leaders"].as_array().unwrap().len(), 2, "{n}");
    // The middle of the leader's last segment (to its point).
    let path = n["leaders"][1]["path"].as_array().unwrap().clone();
    let (q0, q1) = (&path[path.len() - 2], &path[path.len() - 1]);
    let mid = [(num(&q0[0]) + num(&q1[0])) / 2.0, (num(&q0[1]) + num(&q1[1])) / 2.0];
    // 5 mm off the line, square to it.
    let (dx, dy) = (num(&q1[0]) - num(&q0[0]), num(&q1[1]) - num(&q0[1]));
    let len = dx.hypot(dy);
    let off = [mid[0] - dy / len * 5.0, mid[1] + dx / len * 5.0];
    let e = edit(&mut ctx, &note, json!({ "action": "add_node", "leader": 1, "point": off })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("point"), "{e:?}");
    edit(&mut ctx, &note, json!({ "action": "add_node", "leader": 1, "point": mid })).unwrap();
    assert_eq!(last(&mut ctx), "Add node");
    assert_eq!(ann(&mut ctx, &note)["leaders"][1]["nodes"], 1);
    edit(&mut ctx, &note, json!({ "action": "remove_node", "leader": 1, "node": 0 })).unwrap();
    assert_eq!(last(&mut ctx), "Remove node");
    assert_eq!(ann(&mut ctx, &note)["leaders"][1]["nodes"], 0);
    edit(&mut ctx, &note, json!({ "action": "delete_leader", "leader": 1 })).unwrap();
    assert_eq!(last(&mut ctx), "Delete leader");
    assert_eq!(ann(&mut ctx, &note)["leaders"].as_array().unwrap().len(), 1);

    // Groups: with a view, and annotations together.
    let note2 = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "NOTE 2", "position": [40, 60] }))["annotation"].clone();
    edit(&mut ctx, &note, json!({ "action": "group_annotation", "with": [note2] })).unwrap();
    assert_eq!(last(&mut ctx), "Group annotation");
    let (g1, g2) = (ann(&mut ctx, &note)["annotation_group"].clone(), ann(&mut ctx, &note2)["annotation_group"].clone());
    assert!(g1.is_u64() && g1 == g2, "{g1} {g2}");
    edit(&mut ctx, &note, json!({ "action": "remove_annotation_from_group", "with": [note2] })).unwrap();
    assert!(ann(&mut ctx, &note2)["annotation_group"].is_null());
    edit(&mut ctx, &note2, json!({ "action": "group", "view": top_id })).unwrap();
    assert_eq!(last(&mut ctx), "Group");
    assert_eq!(ann(&mut ctx, &note2)["view_group"], top_id);
    edit(&mut ctx, &note2, json!({ "action": "remove_from_group" })).unwrap();
    assert_eq!(last(&mut ctx), "Remove from group");
    assert!(ann(&mut ctx, &note2)["view_group"].is_null());
    edit(&mut ctx, &note, json!({ "action": "remove_leaders" })).unwrap();
    assert_eq!(last(&mut ctx), "Remove leaders");
    assert!(ann(&mut ctx, &note)["leaders"].is_null());
    let e = edit(&mut ctx, &note, json!({ "action": "remove_leaders" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("action"), "{e:?}");

    // A GD&T frame gets a leader; it has no jogs.
    let gdt = call(&mut ctx, "drawing_gdt", json!({ "drawing": tab, "position": [60, 80], "rows": [{ "symbol": "position", "tolerance": "0.1" }] }))["annotation"].clone();
    let before = ann(&mut ctx, &gdt)["leaders"].as_array().map_or(0, Vec::len);
    edit(&mut ctx, &gdt, json!({ "action": "add_leader", "point": a0 })).unwrap();
    assert_eq!(ann(&mut ctx, &gdt)["leaders"].as_array().map_or(0, Vec::len), before + 1);
    let e = edit(&mut ctx, &gdt, json!({ "action": "add_jog", "line": 0 })).unwrap_err();
    assert!(e.message.contains("linear dimensions"), "{e:?}");

    // Radius: foreshorten and back.
    let rad = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "radius", "picks": [[cx + r, cy]], "place": [cx + r + 15.0, cy + 15.0] }))["annotation"].clone();
    edit(&mut ctx, &rad, json!({ "action": "foreshorten" })).unwrap();
    assert_eq!(last(&mut ctx), "Foreshorten");
    assert_eq!(ann(&mut ctx, &rad)["kind"], "ForeshortenedRadial");
    edit(&mut ctx, &rad, json!({ "action": "remove_foreshorten" })).unwrap();
    assert_eq!(ann(&mut ctx, &rad)["kind"], "RadialDimension");

    // Stacked callouts need a BOM table.
    let co = call(&mut ctx, "drawing_callout", json!({ "drawing": tab, "anchor": quarter(h0), "position": [40, 250] }))["annotation"].clone();
    let e = edit(&mut ctx, &co, json!({ "action": "stacked_callouts", "parts": ["Part 1"] })).unwrap_err();
    assert!(e.message.contains("BOM"), "{e:?}");
    call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "bom", "view": top_id, "position": [20, 280] }));
    edit(&mut ctx, &co, json!({ "action": "stacked_callouts", "parts": ["Part 1"] })).unwrap();
    assert_eq!(last(&mut ctx), "Add stacked callouts");
    assert_eq!(ann(&mut ctx, &co)["stacked"], 1);
    let e = edit(&mut ctx, &co, json!({ "action": "stacked_callouts", "parts": ["Nope"] })).unwrap_err();
    assert!(e.message.contains("Part 1"), "{e:?}");

    // Add to ordinate group.
    let o = call(&mut ctx, "drawing_ordinate", json!({ "drawing": tab, "origin": a0, "side": [a0[0], a0[1] - 15.0], "points": [b0] }))["annotation"].clone();
    call(&mut ctx, "drawing_ordinate", json!({ "drawing": tab, "group": o, "points": [p(horizontal[1], "b")] }));
    assert_eq!(last(&mut ctx), "Add to ordinate group");
    let e = try_call(&mut ctx, "drawing_ordinate", json!({ "drawing": tab, "group": o, "origin": a0, "points": [b0] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("group"), "{e:?}");
    // Reset ordinate: nothing to move back, no edit (as the editor).
    let r = call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": o, "action": "reset_ordinate" }));
    assert!(r["message"].as_str().unwrap().starts_with("Nothing to reset"), "{r}");
    assert_eq!(last(&mut ctx), "Add to ordinate group");
}

/// P1 menu gaps: display states, the assembly configuration panel's input
/// menus, Change to version, configured part properties and Part Studio
/// datum annotations.
#[test]
fn assembly_and_part_studio_menu_gaps() {
    let mut ctx = Ctx::new();
    let (block, pin) = two_part_assembly(&mut ctx, "menu_gaps.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().rsplit(":: ").next().unwrap().to_string();
    let name_of = |ctx: &mut Ctx, id: &str| call(ctx, "assembly_list", json!({}))["instances"].as_array().unwrap().iter().find(|i| i["id"] == id).unwrap()["name"].as_str().unwrap().to_string();
    let pin_name = name_of(&mut ctx, &pin);

    // Display states.
    call(&mut ctx, "assembly_set_state", json!({ "instances": [pin], "hidden": true }));
    let r = call(&mut ctx, "assembly_display_state", json!({ "action": "add", "name": "Pin hidden" }));
    assert_eq!(last(&mut ctx), "Add display state : Pin hidden");
    assert_eq!(r["display_states"][0]["hidden_instances"], json!([pin_name]), "{r}");
    let e = try_call(&mut ctx, "assembly_display_state", json!({ "action": "add", "name": "Pin hidden" })).unwrap_err();
    assert!(e.message.contains("already a display state"), "{e:?}");
    call(&mut ctx, "assembly_set_state", json!({ "instances": [pin], "hidden": false }));
    let r = call(&mut ctx, "assembly_display_state", json!({ "action": "apply", "state": "Pin hidden" }));
    assert_eq!(last(&mut ctx), "Apply display state : Pin hidden");
    assert_eq!(r["hidden_instances"], json!([pin_name]), "{r}");
    call(&mut ctx, "assembly_display_state", json!({ "action": "rename", "state": "1", "name": "No pin" }));
    assert_eq!(last(&mut ctx), "Rename display state : No pin");
    let r = call(&mut ctx, "assembly_display_state", json!({ "action": "remove", "state": "No pin" }));
    assert_eq!(last(&mut ctx), "Remove display state");
    assert!(r["display_states"].as_array().unwrap().is_empty());

    // Configuration inputs: duplicate, visibility, copy into the Part Studio, propagate.
    let list = call(&mut ctx, "config_add_input", json!({ "tab": "Assembly 1", "kind": "list" }))["input"].as_str().unwrap().to_string();
    let lite = call(&mut ctx, "config_add_option", json!({ "tab": "Assembly 1", "input": list, "name": "Lite" }))["option"].as_str().unwrap().to_string();
    let check = call(&mut ctx, "config_add_input", json!({ "tab": "Assembly 1", "kind": "checkbox", "name": "Holes" }))["input"].as_str().unwrap().to_string();
    let d = call(&mut ctx, "config_duplicate_input", json!({ "tab": "Assembly 1", "input": "Holes" }));
    assert_eq!(last(&mut ctx), "Copy configuration input");
    assert!(d["configuration"]["inputs"].as_array().unwrap().iter().any(|i| i["name"] == "Holes copy"), "{d}");
    let e = try_call(&mut ctx, "config_duplicate_input", json!({ "tab": "Part Studio 1", "input": "x" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tab"), "{e:?}");
    call(&mut ctx, "config_visibility", json!({ "tab": "Assembly 1", "input": check, "visibility": { "conditions": [{ "input": list, "test": { "Is": [lite] } }], "option_rules": [] } }));
    assert_eq!(last(&mut ctx), "Edit visibility conditions");
    let l = call(&mut ctx, "config_list", json!({ "tab": "Assembly 1" }));
    assert!(l["inputs"].as_array().unwrap().iter().find(|i| i["id"] == check).unwrap()["visible_when"].is_object(), "{l}");
    let e = try_call(&mut ctx, "config_visibility", json!({ "tab": "Assembly 1", "input": list, "visibility": { "conditions": [{ "input": check, "test": { "Is": ["true"] } }], "option_rules": [] } })).unwrap_err();
    assert!(e.message.contains("not above"), "{e:?}");
    let c = call(&mut ctx, "config_copy_inputs", json!({ "from": "Assembly 1", "tab": "Part Studio 1" }));
    assert_eq!(c["pasted"].as_array().unwrap().len(), 3, "{c}");
    let p = call(&mut ctx, "config_propagate", json!({ "tab": "Assembly 1", "input": list }));
    assert_eq!(last(&mut ctx), "Input propagation");
    let cands = p["candidates"].as_array().unwrap();
    assert_eq!(cands.len(), 2, "{p}");
    assert!(cands.iter().all(|c| c["propagated"] == true), "{p}");
    let e = try_call(&mut ctx, "config_propagate", json!({ "tab": "Assembly 1", "input": list, "features": ["x"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("features"), "{e:?}");

    // Change to version: the block shows V1, then V2; it keeps its geometry.
    let e = try_call(&mut ctx, "assembly_change_to_version", json!({ "version": "V1" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{e:?}");
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    call(&mut ctx, "assembly_change_to_version", json!({ "version": "V1", "instances": [block] }));
    assert_eq!(last(&mut ctx), "Change to version : V1");
    let src = |ctx: &mut Ctx| call(ctx, "assembly_list", json!({}))["instances"].as_array().unwrap().iter().find(|i| i["id"] == block).unwrap()["source"].clone();
    assert!(src(&mut ctx)["version_of"].is_object(), "{}", src(&mut ctx));
    let faces = call(&mut ctx, "assembly_entities", json!({ "instance": block, "kind": "faces" }));
    assert!(!faces["faces"].as_array().unwrap().is_empty(), "the versioned block keeps its geometry: {faces}");
    call(&mut ctx, "history_create_version", json!({ "name": "V2" }));
    call(&mut ctx, "assembly_change_to_version", json!({ "version": "V2", "instances": [block] }));
    assert_eq!(last(&mut ctx), "Update to V2");

    // Configured part properties.
    let part = call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"][0]["id"].as_str().unwrap().to_string();
    let t = call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": part, "action": "add_property", "property": "Description" }));
    assert_eq!(last(&mut ctx), "Add configured property");
    assert_eq!(t["table"]["properties"], json!(["Description"]), "{t}");
    let t = call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": part, "action": "set", "property": "Description", "value": "Long" }));
    assert_eq!(last(&mut ctx), "Edit configured properties");
    assert_eq!(t["table"]["rows"][0]["values"]["Description"], "Long", "{t}");
    let t = call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": part, "action": "delete_column", "property": "Description" }));
    assert_eq!(last(&mut ctx), "Delete configured property");
    assert!(t["table"]["properties"].as_array().unwrap().is_empty(), "{t}");
    call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": part, "action": "delete_table" }));
    assert_eq!(last(&mut ctx), "Delete configured properties table");
    let e = try_call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": part, "action": "delete_table" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("part"), "{e:?}");

    // Datum annotations.
    let top = call(&mut ctx, "query_entities", json!({ "tab": "Part Studio 1", "kind": "faces", "surface": "plane", "normal": [0, 0, 1] }))["faces"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "add_datum", "face": top }));
    assert_eq!(last(&mut ctx), "Datum");
    let r = call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "add_datum", "face": top }));
    let letters: Vec<Value> = r["annotations"].as_array().unwrap().iter().map(|a| a["kind"]["datum"].clone()).collect();
    assert_eq!(letters, [json!("A"), json!("B")], "{r}");
    let r = call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "delete", "annotation": 1 }));
    assert_eq!(last(&mut ctx), "Delete annotation");
    assert_eq!(r["annotations"].as_array().unwrap().len(), 1, "{r}");
    let e = try_call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "add_datum", "face": "plane:top" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("face"), "{e:?}");
}

/// Flex PCB model and Finish flex PCB model are feature_add types.
#[test]
fn flex_pcb_model_features() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "flex_pcb.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [60, 20]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "1 mm" } }));
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    let s = call(&mut ctx, "feature_add", json!({ "type": "pcbStart", "params": { "process": "CONVERT", "partToConvert": [part] } }));
    assert_eq!(s["feature_status"]["status"], "Ok", "{s}");
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    let f = call(&mut ctx, "feature_add", json!({ "type": "pcbEnd", "params": { "unfoldType": "UNFOLD_NONE", "pcbPart": [part] } }));
    // Finishing reports Onshape's note, an Info, not an error.
    let st = &f["feature_status"];
    assert!(st["status"] == "Ok" || (st["status"] == "Info" && st["message"].as_str().is_some_and(|m| m.contains("flat pattern"))), "{f}");
}

/// render_image's view menu options and analysis displays.
#[test]
fn render_image_view_options() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "render_options.noshape");
    let img = |ctx: &mut Ctx, extra: Value| {
        let mut a = json!({ "width": 160, "height": 120, "view": "isometric" });
        a.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let r = call(ctx, "render_image", a);
        assert!(num(&r["bytes"]) > 100.0, "{r}");
        r
    };
    let png = |ctx: &mut Ctx, extra: Value| -> Vec<u8> {
        let file = scratch("render_options.png");
        let mut a = extra;
        a["file"] = json!(file.to_str().unwrap());
        img(ctx, a);
        std::fs::read(&file).unwrap()
    };
    for o in [
        json!({ "perspective": true }),
        json!({ "tangent_edges": "phantom" }),
        json!({ "tangent_edges": "hidden", "hidden_edges": true }),
        json!({ "boundary_edges": true }),
        json!({ "isolate": ["Part 1"] }),
        json!({ "transparent": ["Part 1"] }),
        json!({ "high_quality": true }),
        json!({ "reflection": {} }),
        json!({ "reflection": { "image": "Nightfall", "rotation": 30, "blur": true, "show_edges": true } }),
    ] {
        img(&mut ctx, o);
    }
    let r = img(&mut ctx, json!({ "normal_to": "plane:front" }));
    assert_eq!(r["view"], "normal_to");
    // Zebra stripes change the picture.
    assert_ne!(png(&mut ctx, json!({})), png(&mut ctx, json!({ "zebra": { "stripes": 20 } })));
    for (o, param) in [
        (json!({ "tangent_edges": "dashed" }), "tangent_edges"),
        (json!({ "isolate": ["Nope"] }), "isolate[0]"),
        (json!({ "normal_to": "plane:nope" }), "normal_to"),
        (json!({ "reflection": { "image": "/nonexistent/env.png" } }), "reflection.image"),
        (json!({ "zebra": {}, "reflection": {} }), "reflection"),
        (json!({ "zebra": { "stripes": 0 } }), "zebra.stripes"),
    ] {
        let mut a = o.clone();
        a["width"] = json!(64);
        a["height"] = json!(64);
        let e = try_call(&mut ctx, "render_image", a).unwrap_err();
        assert_eq!(e.param.as_deref(), Some(param), "{o}: {e:?}");
    }
    // Assemblies: the same options on instances; no normal_to / boundary edges.
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }));
    let r = call(&mut ctx, "render_image", json!({ "tab": "Assembly 1", "width": 160, "height": 120, "perspective": true, "zebra": {}, "isolate": ["Part Studio 1 <1>"] }));
    assert!(num(&r["bytes"]) > 100.0, "{r}");
    let e = try_call(&mut ctx, "render_image", json!({ "tab": "Assembly 1", "normal_to": "plane:top" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("normal_to"), "{e:?}");
}

/// Draft analysis and Flatten surfaces on the block with a hole.
#[test]
fn draft_and_flatten_analysis() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "draft_flatten.noshape");
    // Pulled along +Z: the top toward the pull, the bottom away, walls and hole steep.
    let d = call(&mut ctx, "analysis_draft", json!({ "pull": "plane:top" }));
    let faces = d["parts"][0]["faces"].as_array().unwrap().clone();
    let count = |faces: &[Value], c: &str| faces.iter().filter(|f| f["class"] == c).count();
    assert_eq!((count(&faces, "side_1"), count(&faces, "side_2")), (1, 1), "{d}");
    assert_eq!(count(&faces, "steep"), faces.len() - 2, "{d}");
    let top = faces.iter().find(|f| f["class"] == "side_1").unwrap();
    assert!((num(&top["draft_deg"][0]) - 90.0).abs() < 1e-3, "{top}");
    let flipped = call(&mut ctx, "analysis_draft", json!({ "pull": "plane:top", "flip": true }));
    let ff = flipped["parts"][0]["faces"].as_array().unwrap();
    assert_eq!(ff.iter().find(|f| f["face"] == top["face"]).unwrap()["class"], "side_2", "{flipped}");
    let e = try_call(&mut ctx, "analysis_draft", json!({ "pull": "plane:top", "angle": "100 deg" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("angle"), "{e:?}");

    // Flatten a side wall (already flat: no stretch) and the hole's wall
    // (opened along its seam). Faces with holes: qa5-routing FLAT-HOLE.
    let wall = call(&mut ctx, "query_entities", json!({ "kind": "faces", "surface": "plane", "normal": [0, -1, 0] }))["faces"][0]["id"].clone();
    let f = call(&mut ctx, "analysis_flatten", json!({ "faces": [wall] }));
    assert!((num(&f["area_3d"]) - 1600.0).abs() < 1e-3, "{f}");
    assert!((num(&f["area_flat"]) - 1600.0).abs() < 1e-3, "{f}");
    let size = [num(&f["flat_size"][0]), num(&f["flat_size"][1])];
    assert!((size[0].min(size[1]) - 20.0).abs() < 1e-3 && (size[0].max(size[1]) - 80.0).abs() < 1e-3, "{f}");
    // The hole's wall (one face or two halves).
    let hole: Vec<Value> = call(&mut ctx, "query_entities", json!({ "kind": "faces", "surface": "cylinder" }))["faces"].as_array().unwrap().iter().map(|f| f["id"].clone()).collect();
    let dxf = scratch("flat_hole.dxf");
    let f = call(&mut ctx, "analysis_flatten", json!({ "faces": hole, "export": { "format": "DXF", "file": dxf.to_str().unwrap() } }));
    let want = 2.0 * std::f64::consts::PI * 5.0 * 20.0;
    assert!((num(&f["area_flat"]) - want).abs() / want < 0.02, "{f}");
    assert!(std::fs::read_to_string(&dxf).unwrap().contains("SECTION"));
    let e = try_call(&mut ctx, "analysis_flatten", json!({ "faces": ["plane:top"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("faces[0]"), "{e:?}");
}

/// An Assembly exported as STEP keeps the assembly (root product, each part
/// once, placed by its instances), and importing it gives back an Assembly
/// and a "<file> parts" Part Studio with the parts in their own coordinates
/// (live qa4 r4 s5).
#[test]
fn step_assembly_export_and_import_round_trip() {
    let mut ctx = Ctx::new();
    let (_, pin) = two_part_assembly(&mut ctx, "step_asm.noshape");
    call(&mut ctx, "assembly_transform", json!({ "instance": pin, "translation": [0, 0, 30], "axis": [0, 0, 1], "angle": "90 deg" }));
    let placed = |ctx: &mut Ctx, tab: &str, name: &str| call(ctx, "assembly_list", json!({ "tab": tab }))["instances"].as_array().unwrap().iter().find(|i| i["name"] == name).unwrap_or_else(|| panic!("no {name} in {tab}")).clone();
    let list = call(&mut ctx, "assembly_list", json!({}));
    let name_of = |id: &str| list["instances"].as_array().unwrap().iter().find(|i| i["id"] == id).unwrap()["name"].as_str().unwrap().to_string();
    let pin_name = name_of(&pin);
    let pin_before = placed(&mut ctx, "Assembly 1", &pin_name);

    let file = scratch("step_asm_out.step");
    let e = call(&mut ctx, "export_parts", json!({ "tab": "Assembly 1", "format": "STEP", "file": file.to_str().unwrap() }));
    assert_eq!(e["parts"], 2, "{e}");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("PRODUCT('Assembly 1','Assembly 1','Assembly 1'"), "the root product");
    assert_eq!(text.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE(").count(), 2);
    assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count() + text.matches("BREP_WITH_VOIDS(").count(), 2, "each part once");

    let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
    let tabs: Vec<(String, String)> = r["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| (t["name"].as_str().unwrap().to_string(), t["kind"].as_str().unwrap().to_string())).collect();
    assert_eq!(tabs.len(), 2, "{r}");
    assert!(tabs.contains(&("Assembly 1 (1)".to_string(), "Assembly".to_string())), "{r}");
    assert!(tabs.contains(&("step_asm_out parts".to_string(), "Part Studio".to_string())), "{r}");
    // The parts in the reverse of the file's order, in their own coordinates.
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "step_asm_out parts" }));
    let names: Vec<&str> = parts["parts"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Part 2", "Part 1"], "{parts}");
    let pin_part = &parts["parts"][0];
    assert!((num(&pin_part["bounding_box"]["min"][0]) - 55.0).abs() < 1e-3 && (num(&pin_part["bounding_box"]["max"][2]) - 50.0).abs() < 1e-3, "the pin where its Part Studio has it: {pin_part}");
    // The instances where the assembly had them.
    let pin_after = placed(&mut ctx, "Assembly 1 (1)", "Part 2 <1>");
    for i in 0..3 {
        assert!((num(&pin_after["translation"][i]) - num(&pin_before["translation"][i])).abs() < 1e-6, "{pin_after} vs {pin_before}");
    }
    for i in 0..4 {
        assert!((num(&pin_after["rotation"][i]).abs() - num(&pin_before["rotation"][i]).abs()).abs() < 1e-6, "{pin_after} vs {pin_before}");
    }
    let block_after = placed(&mut ctx, "Assembly 1 (1)", "Part 1 <1>");
    assert!(block_after["translation"].as_array().unwrap().iter().all(|v| num(v).abs() < 1e-6), "{block_after}");
}

/// Live's own STEP assembly (qa4 r4 s5): the tabs, parts and placements live makes.
#[test]
fn step_assembly_import_matches_live() {
    let Some(step) = live_sample("qa4-s5-asm.step") else { return };
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("qa4_s5.noshape").to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
    let mut tabs: Vec<String> = r["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| t["name"].as_str().unwrap().to_string()).collect();
    tabs.sort();
    // " (1)": the document already has an Assembly 1.
    assert_eq!(tabs, ["Assembly 1 (1)", "qa4-s5-asm parts"], "{r}");
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "qa4-s5-asm parts" }));
    let names: Vec<&str> = parts["parts"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Part 2", "Part 3", "Part 1"], "{parts}");
    // Each part at its own part position (live).
    let bbox = |n: &str| {
        let p = parts["parts"].as_array().unwrap().iter().find(|p| p["name"] == n).unwrap();
        (0..3).map(|i| num(&p["bounding_box"]["min"][i])).chain((0..3).map(|i| num(&p["bounding_box"]["max"][i]))).collect::<Vec<f64>>()
    };
    for (n, want) in [("Part 1", [0.0, 0.0, 0.0, 100.0, 60.0, 10.0]), ("Part 2", [195.0, -5.0, 0.0, 205.0, 5.0, 40.0]), ("Part 3", [300.0, 0.0, 0.0, 320.0, 20.0, 20.0])] {
        let got = bbox(n);
        assert!(got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3), "{n}: {got:?}");
    }
    // The assembly places them as the file does: Part 3 at (-210, 0, 10) from its part position.
    let asm = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1 (1)" }));
    let p3 = asm["instances"].as_array().unwrap().iter().find(|i| i["name"] == "Part 3 <1>").unwrap_or_else(|| panic!("{asm}"));
    let t: Vec<f64> = (0..3).map(|i| num(&p3["translation"][i])).collect();
    assert!((t[0] + 210.0).abs() < 1e-6 && t[1].abs() < 1e-6 && (t[2] - 10.0).abs() < 1e-6, "{p3}");
    assert_eq!(asm["instances"].as_array().unwrap().len(), 3, "{asm}");
}

/// Two different parts with the same name (common in STEP files: "Part",
/// "Body", "SOLID"): each imported instance gets its own part's geometry,
/// not the first part of that name.
#[test]
fn step_assembly_parts_sharing_a_name_keep_their_geometry() {
    let mut ctx = Ctx::new();
    let (block, pin) = two_part_assembly(&mut ctx, "step_same_names.noshape");
    for p in call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"].as_array().unwrap().clone() {
        call(&mut ctx, "part_rename", json!({ "tab": "Part Studio 1", "part": p["id"], "name": "Part" }));
    }
    call(&mut ctx, "assembly_transform", json!({ "instance": pin, "translation": [0, 0, 30] }));
    let before = call(&mut ctx, "assembly_list", json!({}));
    let at = |list: &Value, id: &str| -> Vec<f64> { (0..3).map(|i| num(&list["instances"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap()["translation"][i])).collect() };
    let (block_at, pin_at) = (at(&before, &block), at(&before, &pin));

    let file = scratch("same_names.step");
    call(&mut ctx, "export_parts", json!({ "tab": "Assembly 1", "format": "STEP", "file": file.to_str().unwrap() }));
    call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "same_names parts" }));
    let list = parts["parts"].as_array().unwrap().clone();
    assert_eq!(list.len(), 2, "{parts}");
    assert!(list.iter().all(|p| p["name"] == "Part"), "both keep the file's name: {parts}");
    // Each instance's part: the pin's is the Ø10 cylinder at x 55..65, the block's the 40 mm block.
    let asm = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1 (1)" }));
    let instances = asm["instances"].as_array().unwrap().clone();
    assert_eq!(instances.len(), 2, "{asm}");
    let part_of = |i: &Value| list.iter().find(|p| p["id"] == i["source"]["part"]).unwrap_or_else(|| panic!("{i}")).clone();
    let near = |i: &Value, t: &[f64]| (0..3).all(|k| (num(&i["translation"][k]) - t[k]).abs() < 1e-6);
    let pin_i = instances.iter().find(|i| near(i, &pin_at)).unwrap_or_else(|| panic!("no instance at the pin's place: {asm}"));
    let block_i = instances.iter().find(|i| near(i, &block_at)).unwrap_or_else(|| panic!("no instance at the block's place: {asm}"));
    let (pp, bp) = (part_of(pin_i), part_of(block_i));
    assert_ne!(pp["id"], bp["id"], "two different parts: {asm}");
    assert!((num(&pp["bounding_box"]["min"][0]) - 55.0).abs() < 1e-3 && (num(&pp["bounding_box"]["max"][0]) - 65.0).abs() < 1e-3, "the pin instance shows the pin: {pp}");
    assert!((num(&bp["bounding_box"]["min"][0]) + 20.0).abs() < 1e-3 && (num(&bp["bounding_box"]["max"][0]) - 20.0).abs() < 1e-3, "the block instance shows the block: {bp}");
}

/// The parts and instances of an imported STEP assembly in live's order
/// (behaviours.md "api (10-25) STEP assembly part order"): occurrences
/// walked from the file's last, each product one part at its first
/// occurrence, a product used twice one part with two instances.
#[test]
fn step_assembly_import_order_matches_live() {
    for (file, want_parts, want_instances) in [
        ("stepord-ABCD.step", &["C", "B", "A", "D"][..], &["C <1>", "B <1>", "A <1>", "D <1>"][..]),
        ("stepord-ABBCD.step", &["B", "A", "C", "D"][..], &["B <1>", "A <1>", "C <1>", "D <1>", "B <2>"][..]),
    ] {
        let Some(step) = live_sample(file) else { return };
        let stem = file.trim_end_matches(".step");
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": scratch(&format!("{stem}.noshape")).to_str().unwrap() }));
        let r = call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
        let parts = call(&mut ctx, "query_parts", json!({ "tab": format!("{stem} parts") }));
        let names: Vec<&str> = parts["parts"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(names, want_parts, "{file}: {parts}");
        let root = format!("stepord {}", stem.trim_start_matches("stepord-"));
        let asm = call(&mut ctx, "assembly_list", json!({ "tab": root }));
        let instances: Vec<&str> = asm["instances"].as_array().unwrap_or_else(|| panic!("{r}")).iter().map(|i| i["name"].as_str().unwrap()).collect();
        assert_eq!(instances, want_instances, "{file}: {asm}");
        // Both B instances show the one B part.
        let b: Vec<&Value> = asm["instances"].as_array().unwrap().iter().filter(|i| i["name"].as_str().unwrap().starts_with("B <")).collect();
        assert!(b.iter().all(|i| i["source"]["part"] == b[0]["source"]["part"]), "{file}: {asm}");
    }
}

/// The agent-usability pass (AGENT_API.md "Lessons from agent runs"): an L
/// bracket built the way an agent does, using the helps that came out of it:
/// `at` and `direction` picks, model points on a turned face sketch, sketch
/// edits reporting failures, undo naming the edit, tidy numbers, feature type
/// search and projected view orientations.
#[test]
fn agent_usability_pass() {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("agent_pass.noshape").to_str().unwrap() }));
    call(&mut ctx, "sketch_create", json!({ "plane": "plane:right" }));
    let s = call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "line", "points": [[0, 0], [50, 0], [50, 40], [44, 40], [44, 6], [0, 6], [0, 0]] }));
    let region = s["regions"][0]["id"].clone();
    assert_eq!(s["failed"], json!([]), "{s}");
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "80 mm" } }));

    // Three different edges in one call, in the points' order.
    let e = call(&mut ctx, "query_entities", json!({ "kind": "edges", "at": [[0, 0, 3], [80, 0, 3], [40, 44, 6]] }));
    let mids: Vec<Value> = e["edges"].as_array().unwrap().iter().map(|x| x["midpoint"].clone()).collect();
    assert_eq!(mids, [json!([0.0, 0.0, 3.0]), json!([80.0, 0.0, 3.0]), json!([40.0, 44.0, 6.0])], "{e}");
    // A point on a face picks it; one on the edge two faces share, the face whose centroid is closer.
    let f = call(&mut ctx, "query_entities", json!({ "kind": "faces", "at": [[40, 25, 6], [40, 0, 6]] }));
    assert_eq!(f["faces"][0]["normal"], json!([0.0, 0.0, 1.0]), "{f}");
    assert_eq!(f["faces"][1]["normal"], json!([0.0, -1.0, 0.0]), "{f}");
    // Vertical edges only.
    let v = call(&mut ctx, "query_entities", json!({ "kind": "edges", "direction": [0, 0, 1] }));
    let list = v["edges"].as_array().unwrap();
    assert_eq!(list.len(), 6, "{v}");
    assert!(list.iter().all(|x| num(&x["direction"][2]).abs() == 1.0), "{v}");
    // No floating-point noise in replies.
    assert!(!v.to_string().contains("-0.0"), "{v}");

    // Hole points given as model positions on the base's top face, whatever its sketch axes.
    let top = call(&mut ctx, "query_entities", json!({ "kind": "faces", "at": [[40, 22, 6]] }))["faces"][0]["id"].clone();
    call(&mut ctx, "sketch_create", json!({ "plane": top }));
    let p = call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 2", "tool": "point", "points": [[10, 10, 6], [70, 10, 6], [10, 34, 6], [70, 34, 6]] }));
    let pts: Vec<Value> = p["created"].as_array().unwrap().iter().map(|x| x["id"].clone()).collect();
    assert_eq!(pts.len(), 4, "{p}");
    let fr = &p["frame"];
    let world = |at: &Value| (0..3).map(|i| num(&fr["origin"][i]) + num(&at[0]) * num(&fr["x"][i]) + num(&at[1]) * num(&fr["y"][i])).collect::<Vec<f64>>();
    let first = world(&p["created"][0]["at"]);
    assert!((first[0] - 10.0).abs() < 1e-9 && (first[1] - 10.0).abs() < 1e-9 && (first[2] - 6.0).abs() < 1e-9, "{p}");
    let hole = call(&mut ctx, "feature_add", json!({ "type": "hole", "params": { "locations": pts, "holeDiameterV3": "6.6 mm", "endStyleV2": "THROUGH" } }));
    assert_eq!(hole["regeneration"]["failed"], json!([]), "{hole}");
    let vol = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    assert!((vol - (40320.0 - 4.0 * std::f64::consts::PI * 3.3 * 3.3 * 6.0)).abs() < 1e-3, "{vol}");

    // Undo names the edit it undid; redo puts it back.
    let u = call(&mut ctx, "history_undo", json!({}));
    assert!(u["description"].as_str().unwrap().ends_with("Hole 1"), "{u}");
    let r = call(&mut ctx, "history_redo", json!({}));
    assert!(r["description"].as_str().unwrap().ends_with("Hole 1"), "{r}");

    // Sketch edits report the features that fail after them (none here: the hole follows its face).
    let d = call(&mut ctx, "sketch_dimension", json!({ "sketch": "Sketch 1", "kind": "distance", "entities": [s["created"][2]["id"], s["created"][10]["id"]], "value": "6 mm" }));
    let dim = d["dimension"].clone();
    let ok = call(&mut ctx, "sketch_set_dimension", json!({ "sketch": "Sketch 1", "dimension": dim, "value": "8 mm" }));
    assert_eq!(ok["failed"], json!([]), "{ok}");

    // Finding a feature type by words.
    let t = call(&mut ctx, "feature_types", json!({ "search": "move face" }));
    assert_eq!(t["feature_types"].as_array().unwrap().iter().map(|x| x["type"].as_str().unwrap()).collect::<Vec<_>>(), ["moveFace"], "{t}");

    // Parts by the name the parts list shows, in the part tools and drawings too.
    call(&mut ctx, "part_rename", json!({ "part": "Part 1", "name": "Bracket" }));
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"][0]["name"], "Bracket");
    call(&mut ctx, "part_set_properties", json!({ "part": "bracket", "part_number": "BR-1" }));

    // Projected views name what they look along.
    call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": "Part Studio 1 Drawing 1" }));
    let projected: Vec<&Value> = g["views"].as_array().unwrap().iter().filter(|v| v["kind"] == "projected").collect();
    assert!(!projected.is_empty() && projected.iter().all(|v| v["orientation"].is_string()), "{g}");
}

/// The last drawing menu items: Replicate annotations, Copy / Paste, Paste
/// as table, table properties, Switch to reference and Replace, each with
/// the editor's edit text.
#[test]
fn drawing_replicate_copy_paste_replace_and_navigation() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_more.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let top_v = g["views"].as_array().unwrap().iter().find(|v| v["orientation"] == "top").unwrap_or_else(|| panic!("{g}")).clone();
    let top = top_v["id"].as_u64().unwrap();
    let hole = top_v["edges"].as_array().unwrap().iter().find(|e| e["type"] == "circle").unwrap_or_else(|| panic!("{top_v}")).clone();
    let (cx, cy, r) = (num(&hole["center"][0]), num(&hole["center"][1]), num(&hole["radius"]));
    call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "diameter", "picks": [[cx + r, cy]], "place": [cx + r + 15.0, cy + 15.0] }));

    // Replicate the Ø onto a second top view: it shows the same hole.
    let second = call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "orientation": "top", "position": [330, 90] }))["view"].as_u64().unwrap();
    let rep = call(&mut ctx, "drawing_replicate", json!({ "drawing": tab, "from": top, "to": second }));
    assert_eq!(rep["message"], "Successfully replicated 1/1 annotations", "{rep}");
    assert!(last(&mut ctx).ends_with(":: Replicate annotations"));
    // Once there, it isn't copied again.
    let again = call(&mut ctx, "drawing_replicate", json!({ "drawing": tab, "from": top, "to": second }));
    assert_eq!((again["replicated"].clone(), again["message"].clone()), (json!([]), json!("No annotations replicated")), "{again}");
    let e = try_call(&mut ctx, "drawing_replicate", json!({ "drawing": tab, "from": top, "to": second, "region": [[0, 0], [1, 1]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("at"), "{e:?}");

    // Copy the top view (with its Ø) and paste it.
    let c = call(&mut ctx, "drawing_copy", json!({ "drawing": tab, "views": [top] }));
    assert_eq!((c["copied"]["views"].clone(), c["message"].clone()), (json!(1), json!("Copied 1 view")), "{c}");
    assert!(c["copied"]["annotations"].as_u64().unwrap() >= 1, "{c}");
    let p = call(&mut ctx, "drawing_paste", json!({ "drawing": tab, "position": [100, 70] }));
    assert_eq!(p["pasted"]["views"].as_array().unwrap().len(), 1, "{p}");
    assert!(!p["pasted"]["annotations"].as_array().unwrap().is_empty(), "{p}");
    assert!(last(&mut ctx).ends_with(":: Paste views"));

    // Paste as table, then Table properties… (fixed corner, kept in place).
    let pt = call(&mut ctx, "drawing_paste_table", json!({ "drawing": tab, "text": "Item\tQty\nBolt\t4", "position": [20, 280] }));
    assert!(last(&mut ctx).ends_with(":: Paste as table"));
    let tid = pt["table"].clone();
    let t = pt["tables"].as_array().unwrap().iter().find(|t| t["id"] == tid).unwrap().clone();
    assert_eq!(t["corner"], "TopLeft", "{t}");
    let p2 = call(&mut ctx, "drawing_table_properties", json!({ "drawing": tab, "table": tid, "corner": "bottom_left" }));
    assert!(last(&mut ctx).ends_with(":: Table fixed corner"));
    let t2 = p2["tables"].as_array().unwrap().iter().find(|t| t["id"] == tid).unwrap().clone();
    assert_eq!(t2["corner"], "BottomLeft", "{t2}");
    for k in ["min", "max"] {
        for i in 0..2 {
            assert!((num(&t2["bounds"][k][i]) - num(&t["bounds"][k][i])).abs() < 1e-6, "the table stays where it is: {t} {t2}");
        }
    }
    let e = try_call(&mut ctx, "drawing_table_properties", json!({ "drawing": tab, "table": tid, "structured": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("structured"), "{e:?}");
    // Revision table properties.
    let rid = call(&mut ctx, "drawing_revision_table", json!({ "drawing": tab, "position": [400, 290] }))["table"].clone();
    call(&mut ctx, "drawing_table_properties", json!({ "drawing": tab, "table": rid, "visible_rows": 8, "newest_top": true }));
    assert!(last(&mut ctx).ends_with(":: Revision table properties"));
    let e = try_call(&mut ctx, "drawing_table_properties", json!({ "drawing": tab, "table": rid, "visible_rows": 0 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("visible_rows"), "{e:?}");

    // Switch to reference: the view's Part Studio (no app open: nothing to switch).
    let s = call(&mut ctx, "drawing_switch_to_reference", json!({ "drawing": tab, "view": top }));
    assert_eq!((s["target"]["name"].clone(), s["target"]["kind"].clone(), s["shown_in_app"].clone()), (json!("Part Studio 1"), json!("Part Studio"), json!(false)), "{s}");
    // Replace…: every base view of Part Studio 1 shows the copy.
    let dup = call(&mut ctx, "tab_duplicate", json!({ "tab": "Part Studio 1" }))["tab"]["name"].as_str().unwrap().to_string();
    call(&mut ctx, "drawing_replace", json!({ "drawing": tab, "view": top, "source": dup }));
    assert!(last(&mut ctx).ends_with(":: Replace"));
    for v in [top, second] {
        let s = call(&mut ctx, "drawing_switch_to_reference", json!({ "drawing": tab, "view": v }));
        assert_eq!(s["target"]["name"], dup.as_str(), "{s}");
    }
    let e = try_call(&mut ctx, "drawing_open_feature_studio", json!({ "drawing": tab, "table": rid })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("table"), "{e:?}");
}

/// The window's navigation (UI-only sweep, batch 1): the tab shown, the Tab
/// manager, instance and in-context navigation, viewing / comparing /
/// repairing history, the drawing's sheets. Each is read back in ui_state.
#[test]
fn window_navigation() {
    let mut ctx = Ctx::new();
    let (block, _pin) = two_part_assembly(&mut ctx, "window_nav.noshape");
    let state = |ctx: &mut Ctx| call(ctx, "ui_state", json!({}));
    let shown = |ctx: &mut Ctx| state(ctx)["active_tab"]["name"].as_str().unwrap().to_string();
    assert_eq!(shown(&mut ctx), "Part Studio 1");
    let t = call(&mut ctx, "tab_activate", json!({ "tab": "Assembly 1" }));
    assert_eq!((t["tab"]["kind"].clone(), t["shown_in_app"].clone()), (json!("Assembly"), json!(false)), "{t}");
    assert_eq!(shown(&mut ctx), "Assembly 1");

    // Tab manager: search, type filter, close.
    let m = call(&mut ctx, "tab_manager", json!({ "search": "assem" }));
    assert_eq!(m["rows"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Assembly 1"], "{m}");
    assert_eq!(state(&mut ctx)["tab_manager"], json!({ "open": true, "search": "assem" }));
    let m = call(&mut ctx, "tab_manager", json!({ "kinds": ["drawing"], "open": false }));
    assert!(m["rows"].as_array().unwrap().is_empty(), "{m}");
    assert_eq!(state(&mut ctx)["tab_manager"]["open"], false);
    let e = try_call(&mut ctx, "tab_manager", json!({ "kinds": ["nope"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("kinds[0]"), "{e:?}");

    // Instance menu › Switch to Part Studio.
    let s = call(&mut ctx, "assembly_switch_to_tab", json!({ "instance": block }));
    assert_eq!((s["target"]["name"].clone(), s["part"]["name"].clone()), (json!("Part Studio 1"), json!("Part 1")), "{s}");
    assert_eq!(shown(&mut ctx), "Part Studio 1");
    let e = try_call(&mut ctx, "assembly_open_linked_document", json!({ "instance": block })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("instance"), "{e:?}");
    let e = try_call(&mut ctx, "context_go_to_assembly", json!({})).unwrap_err();
    assert!(e.message.contains("context"), "{e:?}");

    // History: compare with a version, view it read-only, repair from it.
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    call(&mut ctx, "feature_rename", json!({ "feature": "Extrude 1", "name": "Plate" }));
    let c = call(&mut ctx, "history_compare", json!({ "target": "V1" }));
    let ps = c["tabs"].as_array().unwrap().iter().find(|t| t["name"] == "Part Studio 1").unwrap().clone();
    assert_eq!(ps["status"], "differs", "{c}");
    assert!(ps["features"].as_array().unwrap().iter().any(|f| f["status"] == "modified"), "{ps}");
    assert_eq!(c["target"], "V1", "{c}");
    assert_eq!(state(&mut ctx)["compare"]["target"], "V1");
    call(&mut ctx, "history_compare", json!({ "stop": true }));
    assert!(state(&mut ctx)["compare"].is_null());
    let v = call(&mut ctx, "history_view", json!({ "to": "V1" }));
    assert_eq!(v["label"], "V1", "{v}");
    assert!(v["tabs"].as_array().unwrap().iter().any(|t| t["features"].as_array().is_some_and(|f| f.iter().any(|f| f["name"] == "Extrude 1"))), "{v}");
    let e = try_call(&mut ctx, "feature_rename", json!({ "feature": "Extrude 1", "name": "X" })).unwrap_err();
    assert!(e.message.contains("read-only"), "{e:?}");
    assert!(state(&mut ctx)["viewing"].is_string());
    call(&mut ctx, "history_view", json!({ "to": "workspace" }));
    assert!(state(&mut ctx)["viewing"].is_null());
    let r = call(&mut ctx, "history_repair_view", json!({ "at": "V1" }));
    assert_eq!(r["label"], "V1", "{r}");
    assert_eq!(state(&mut ctx)["repair"]["label"], "V1");
    call(&mut ctx, "history_repair_view", json!({ "close": true }));
    assert!(state(&mut ctx)["repair"].is_null());

    // A drawing's sheets: the panel, then First / Next / Last sheet.
    let tab = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }))["drawing"].as_str().unwrap().to_string();
    call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "action": "add" }));
    let p = call(&mut ctx, "drawing_sheets_panel", json!({ "drawing": tab }));
    let sheets = p["sheets"].as_array().unwrap();
    assert_eq!(sheets.len(), 2, "{p}");
    assert!(!sheets[0]["views"].as_array().unwrap().is_empty(), "{p}");
    assert_eq!(state(&mut ctx)["sheets_panel_open"][0]["name"], tab.as_str());
    let first = call(&mut ctx, "drawing_switch_sheet", json!({ "drawing": tab, "step": "first" }));
    assert_eq!(first["shown_sheet"], 1, "{first}");
    let next = call(&mut ctx, "drawing_switch_sheet", json!({ "drawing": tab, "step": "next" }));
    assert_eq!(next["shown_sheet"], 2, "{next}");
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(last.ends_with("Switch sheet"), "{last}");
    assert_eq!(call(&mut ctx, "drawing_switch_sheet", json!({ "drawing": tab, "step": "last" }))["shown_sheet"], 2);
    let p = call(&mut ctx, "drawing_sheets_panel", json!({ "drawing": tab, "open": false }));
    assert_eq!(p["sheets"][1]["shown"], true, "{p}");
    assert!(state(&mut ctx)["sheets_panel_open"].as_array().unwrap().is_empty());
}

/// The Insert dialog's configuration menu (Reset to default, Set to last
/// viewed) and the Variables panel's Copy table.
#[test]
fn insert_configuration_menu_and_variable_table_copy() {
    let mut ctx = Ctx::new();
    two_part_assembly(&mut ctx, "insert_config_menu.noshape");
    for c in ["default", "last_viewed"] {
        let r = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2", "configuration": c }));
        assert!(r["instance"].is_string(), "{c}: {r}");
    }
    let e = try_call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2", "configuration": "nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("configuration"), "{e:?}");
    call(&mut ctx, "variable_create", json!({ "name": "w", "value": "20 mm", "description": "Width" }));
    let v = call(&mut ctx, "variable_list", json!({ "copy": true }));
    let text = v["copied"].as_str().unwrap();
    assert!(text.starts_with("Name\tVariable type\tValue\tDescription\n"), "{v}");
    assert!(text.lines().any(|l| l.starts_with("w\tLength\t") && l.ends_with("\tWidth")), "{v}");
}
