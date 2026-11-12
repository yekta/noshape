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

/// A file of this run's scratch directory. Tests run in parallel: a name
/// used by two tests would let one overwrite the other's file (flaky), so
/// that fails here, naming both.
fn scratch(name: &str) -> std::path::PathBuf {
    static OWNERS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, String>>> = std::sync::LazyLock::new(Default::default);
    let me = std::thread::current().name().unwrap_or("?").to_string();
    let owner = OWNERS.lock().unwrap().entry(name.to_string()).or_insert_with(|| me.clone()).clone();
    assert_eq!(owner, me, "scratch file `{name}` is used by two tests ({owner} and {me}): give each its own name");
    let dir = std::env::temp_dir().join(format!("noshape-ops-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    let _ = std::fs::remove_file(&p);
    p
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// An ellipse axis dimension reads Ø its axis (qa AGENT-ELLIPSE-DIM-VALUE:
/// it read Ø60 for a 30 mm axis), by `auto` or `diameter`.
#[test]
fn ellipse_axis_dimension_reads_its_diameter() {
    let mut ctx = Ctx::new();
    let path = scratch("ellipse-dim.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "ellipse", "points": [[0, 0], [20, 0], [0, 8]] }));
    let el = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "ellipse").unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sketch, "entities": [el], "value": "2 * 15 mm" }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }));
    let dims: Vec<&Value> = st["constraints"].as_array().unwrap().iter().filter(|c| c["type"] == "Ellipse radius").collect();
    assert_eq!(dims.len(), 1, "{st}");
    assert_eq!((dims[0]["value"].clone(), dims[0]["expression"].clone()), (json!("Ø30"), json!("2 * 15 mm")), "{st}");
    // `diameter` takes the other axis the same way.
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sketch, "kind": "diameter", "entities": [el], "label": [0, 12], "value": "12 mm" }));
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }));
    let values: Vec<String> = st["constraints"].as_array().unwrap().iter().filter(|c| c["type"] == "Ellipse radius").map(|c| c["value"].as_str().unwrap().to_string()).collect();
    assert_eq!(values, ["Ø30", "Ø12"], "{st}");
}

/// sketch_get `at`: only the regions holding a sketch point, or a model
/// point projected onto the sketch plane (Front: sketch y is model Z).
#[test]
fn sketch_regions_at_a_point() {
    let mut ctx = Ctx::new();
    let sketch = new_sketch(&mut ctx, "regions-at.noshape", "plane:front");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "circle", "points": [[0, 20], [5, 20]] }));
    let centers = |v: &Value| v["regions"].as_array().unwrap().iter().map(|r| (num(&r["center"][0]).round(), num(&r["center"][1]).round())).collect::<Vec<_>>();
    assert_eq!(centers(&call(&mut ctx, "sketch_get", json!({ "sketch": sketch }))).len(), 2);
    assert_eq!(centers(&call(&mut ctx, "sketch_get", json!({ "sketch": sketch, "at": [1, 19] }))), [(0.0, 20.0)]);
    assert_eq!(centers(&call(&mut ctx, "sketch_get", json!({ "sketch": sketch, "at": ["1 mm", "0 mm", "1 mm"] }))), [(0.0, 0.0)]);
    assert_eq!(centers(&call(&mut ctx, "sketch_get", json!({ "sketch": sketch, "at": [0, 0, 20] }))), [(0.0, 20.0)]);
    assert!(centers(&call(&mut ctx, "sketch_get", json!({ "sketch": sketch, "at": [10, 10] }))).is_empty());
    // Each region's outer box.
    let r = call(&mut ctx, "sketch_get", json!({ "sketch": sketch, "at": [1, 19] }));
    let b = &r["regions"][0]["bounds"];
    assert_eq!([num(&b["min"][0]), num(&b["min"][1]), num(&b["max"][0]), num(&b["max"][1])].map(|v| v.round()), [-5.0, 15.0, 5.0, 25.0], "{r}");
}

/// A shape whose clicks don't complete it is an error naming the clicks,
/// also when its one click snapped to the Origin (it drew nothing).
#[test]
fn an_incomplete_shape_is_an_error() {
    let mut ctx = Ctx::new();
    let path = scratch("incomplete-shape.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    for at in [[0, 0], [5, 5]] {
        let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "ellipse", "points": [at] })).expect_err("one click");
        assert!(e.message.contains("didn't complete an ellipse"), "{}", e.to_json());
    }
}

/// An angle dimension measures the quadrant its `label` is in, as the
/// Dimension tool's cursor picks it (the parity translator places Onshape's
/// 120° this way).
#[test]
fn angle_dimension_label_picks_the_quadrant() {
    let mut ctx = Ctx::new();
    let path = scratch("angle-quadrant.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "shapes": [{ "tool": "line", "points": [[0, 0], [50, 0]] }, { "tool": "line", "points": [[0, 10], [30, 50]] }] }));
    let lines: Vec<String> = r["created"].as_array().unwrap().iter().filter(|e| e["type"] == "line").map(|e| e["id"].as_str().unwrap().to_string()).collect();
    let angles = |ctx: &mut Ctx| -> Vec<String> {
        let st = call(ctx, "sketch_get", json!({ "sketch": sketch }));
        st["constraints"].as_array().unwrap().iter().filter(|c| c["type"] == "Angle").map(|c| c["value"].as_str().unwrap_or("").to_string()).collect()
    };
    // Lines at 0° and 53.13°: the obtuse quadrant is 126.87°.
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sketch, "kind": "angle", "entities": lines, "label": [-20, 10] }));
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sketch, "kind": "angle", "entities": lines, "label": [40, 10] }));
    let a = angles(&mut ctx);
    assert_eq!(a.len(), 2, "{a:?}");
    assert!(a[0].starts_with("126.87") && a[1].starts_with("53.13"), "{a:?}");
}

/// Live mode: what the open window shows (its camera, its selection) is
/// what ui_state and the window tools see, not what a call last set.
#[test]
fn window_snapshot_is_what_the_window_tools_see() {
    let mut ctx = Ctx::new();
    let path = scratch("window-snapshot.noshape");
    let info = call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let tab = info["tabs"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [20, 10]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": "Sketch 1" }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "5 mm" } }));
    let face = call(&mut ctx, "query_entities", json!({ "kind": "faces", "normal": [0, 0, 1], "limit": 1 }))["faces"][0]["id"].as_str().unwrap().to_string();
    let id = noshape_ops::doc::find_tab(ctx.doc(None).unwrap().doc(), &tab).unwrap();
    let mut snap = noshape_ops::window::WindowSnapshot { active_tab: Some(id), ..Default::default() };
    snap.cameras.insert(id, (noshape_render::Camera::default(), glam::DVec2::new(800.0, 500.0)));
    snap.selection.insert(id, vec![face.clone()]);
    snap.ui = Some(json!({ "dialog": { "title": "Extrude 1" } }));
    ctx.doc(None).unwrap().window.sync(snap);
    let st = call(&mut ctx, "ui_state", json!({}));
    assert_eq!(st["selection"][0]["selected"], json!([face]), "{st}");
    assert_eq!(st["cameras"].as_array().map(|c| c.len()), Some(1), "{st}");
    // The Part Studio's on-screen state as the window gave it.
    assert_eq!(st["window"]["dialog"]["title"], "Extrude 1", "{st}");
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
    // The rectangle's automatic constraints are the UI's (live's): Perpendicular, 2 Parallel, Horizontal;
    // the circle's centre is coincident with the Origin (live).
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }));
    let kinds: Vec<&str> = st["constraints"].as_array().unwrap().iter().map(|c| c["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["Perpendicular", "Parallel", "Parallel", "Horizontal", "Coincident"], "{kinds:?}");
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
    // Each feature's regeneration time, their sum as the Regeneration times panel's footer.
    let times: Vec<f64> = list["features"].as_array().unwrap().iter().map(|f| num(&f["regen_ms"])).collect();
    assert!(times.iter().all(|t| *t >= 0.0) && (times.iter().sum::<f64>() - num(&list["regen_ms"])).abs() < 0.5, "{list}");
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

    let e = try_call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 9" })).expect_err("no Part 9");
    assert!(e.message.starts_with("no part `Part 9` in the source Part Studio; its parts: "), "{}", e.to_json());
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
    // Type its value (the value box): the pin turns 30° about the hole's axis.
    call(&mut ctx, "assembly_mate_value", json!({ "mate": "Revolute 1", "dof": "rz", "value": "30 deg" }));
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Move : Revolute 1");
    let turned = call(&mut ctx, "assembly_list", json!({}))["instances"].as_array().unwrap().iter().find(|i| i["id"] == pin_i).unwrap()["rotation"].clone();
    assert!((num(&turned[2]).abs() - (15f64).to_radians().sin()).abs() < 1e-6, "{turned}");
    let e = try_call(&mut ctx, "assembly_mate_value", json!({ "mate": "Revolute 1", "dof": "x", "value": "3 mm" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("dof"), "{e:?}");
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
    // Simulation connection: the dialog's last checkbox, stored (Onshape
    // help: every mate but Pin slot, Parallel, Tangent and Width).
    assert_eq!(mj["simulation_connection"], false, "{mj}");
    call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "simulation_connection": true }));
    let list = call(&mut ctx, "assembly_list", json!({}));
    assert_eq!(list["mates"].as_array().unwrap().iter().find(|x| x["name"] == "Revolute 1").unwrap()["simulation_connection"], true);
    // Mate menu › Reset: its rotation back to 0, one edit.
    call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "reset": true }));
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Reset : Revolute 1");
    // Mate menu › Apply limit position › Min / Max Z angle limit: the pin turned to -30 / 45 deg, live's toast (2043).
    let pin_rotation = |ctx: &mut Ctx| {
        let list = call(ctx, "assembly_list", json!({}));
        let q = list["instances"].as_array().unwrap().iter().find(|i| i["id"] == pin_i).unwrap()["rotation"].clone();
        (0..4).map(|k| num(&q[k])).collect::<Vec<f64>>()
    };
    let r = call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "apply_limit": { "axis": "rz", "end": "min" } }));
    assert_eq!(r["message"], "Min Z angle limit has been applied to Revolute 1", "{r}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Apply limit position : Revolute 1");
    let q_min = pin_rotation(&mut ctx);
    let r = call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "apply_limit": { "end": "max" } }));
    assert_eq!(r["message"], "Max Z angle limit has been applied to Revolute 1", "{r}");
    let q_max = pin_rotation(&mut ctx);
    let dot: f64 = q_min.iter().zip(&q_max).map(|(a, b)| a * b).sum();
    let turned = 2.0 * dot.abs().min(1.0).acos();
    assert!((turned - 75f64.to_radians()).abs() < 1e-6, "min to max turned {} deg", turned.to_degrees());
    let err = try_call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "apply_limit": { "axis": "x", "end": "max" } })).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("apply_limit"), "{err:?}");
    assert_eq!(mj["type"], "revolute", "the type kept: {mj}");
    // Limits the mate type doesn't have are refused.
    let err = try_call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "limits": { "x": { "min": "0 mm" } } })).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("limits"), "{err:?}");
    // Another type: a default name follows it, as in the mate dialog (qa API-MATE-RETYPE-NAME).
    let r = call(&mut ctx, "assembly_mate", json!({ "mate": "Revolute 1", "type": "slider" }));
    assert_eq!(r["name"], "Slider 1", "{r}");
    call(&mut ctx, "assembly_mate", json!({ "mate": "Slider 1", "name": "Pin guide" }));
    let r = call(&mut ctx, "assembly_mate", json!({ "mate": "Pin guide", "type": "cylindrical" }));
    assert_eq!(r["name"], "Pin guide", "a custom name stays: {r}");
}

/// Tab › Show code (live dom/fs-showcode.txt): the Part Studio as FeatureScript, one block per feature, view only;
/// an Assembly has no Show code.
#[test]
fn show_code_lists_each_feature() {
    let path = scratch("show-code.noshape");
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [20, 10]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": "Sketch 1" }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "5 mm" } }));
    let code = call(&mut ctx, "tab_show_code", json!({ "tab": "Part Studio 1" }))["code"].as_str().unwrap().to_string();
    assert!(code.contains("const buildPrivate = definePartStudio(function(context is Context, configuration is map, lookup is function)"), "{code}");
    let sketch = code.find("annotation { \"Feature Name\" : \"Sketch 1\" }").expect("Sketch 1's block");
    let extrude = code.find("annotation { \"Feature Name\" : \"Extrude 1\" }").expect("Extrude 1's block");
    assert!(sketch < extrude, "in list order: {code}");
    assert!(code.contains("'expression' : \"5 mm\" }.value"), "the depth as live writes it: {code}");
    let e = try_call(&mut ctx, "tab_show_code", json!({ "tab": "Assembly 1" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tab"), "{e:?}");
}

/// Live's "fs pack src" Part Studio (dom/fs-packed-query.txt) built here: Show code reads as live's capture
/// (dom/fs-showcode.txt) with our ids for live's.
#[test]
fn show_code_reads_as_lives_capture() {
    let path = scratch("show-code-live.noshape");
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [40, 30]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "circle", "points": [[60, 15], [65, 15]] }));
    let regions = call(&mut ctx, "sketch_get", json!({ "sketch": "Sketch 1" }))["regions"].clone();
    let region = |at: f64| regions.as_array().unwrap().iter().find(|r| (r["area"].as_f64().unwrap_or(0.0) - at).abs() < 1.0).unwrap_or(&regions[0])["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region(1200.0)], "depth": "10 mm" } }));
    let edge = |ctx: &mut Ctx, at: [f64; 3]| call(ctx, "query_entities", json!({ "kind": "edges", "at": [at] }))["edges"][0]["id"].as_str().unwrap().to_string();
    let face = |ctx: &mut Ctx, at: [f64; 3]| call(ctx, "query_entities", json!({ "kind": "faces", "at": [at] }))["faces"][0]["id"].as_str().unwrap().to_string();
    let e = edge(&mut ctx, [0.0, 0.0, 5.0]);
    call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": [e], "radius": "3 mm" } }));
    let e = edge(&mut ctx, [20.0, 30.0, 10.0]);
    call(&mut ctx, "feature_add", json!({ "type": "chamfer", "params": { "entities": [e], "width": "2 mm" } }));
    call(&mut ctx, "feature_add", json!({ "type": "cPlane", "params": { "entities": ["plane:top"], "cplaneType": "OFFSET", "offset": "25 mm" } }));
    let top = face(&mut ctx, [20.0, 10.0, 10.0]);
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region(78.5)], "endBound": "UP_TO_SURFACE", "endBoundEntityFace": [top] } }));
    let (front, back) = (face(&mut ctx, [20.0, 0.0, 5.0]), face(&mut ctx, [20.0, 30.0, 4.0]));
    call(&mut ctx, "feature_add", json!({ "type": "cPlane", "params": { "entities": [front, back], "cplaneType": "MID_PLANE" } }));
    call(&mut ctx, "feature_add", json!({ "type": "cPlane", "params": { "entities": ["plane:top", "origin"], "cplaneType": "PLANE_POINT" } }));
    let code = call(&mut ctx, "tab_show_code", json!({ "tab": "Part Studio 1" }))["code"].as_str().unwrap().to_string();
    if let Ok(out) = std::env::var("SHOW_CODE_OUT") {
        std::fs::write(out, &code).unwrap();
    }
    // Live's sketch entities by their initial guess (the same geometry), its features by their order.
    const LIVE_GUESS: &str = "\"r0\" : [0.0, 0.0, 1.0, 0.0, 0.0, 0.04], \"r1\" : [0.04, 0.0, 0.0, 1.0, 0.0, 0.03], \"r2\" : [0.04, 0.03, -1.0, 0.0, 0.0, 0.04], \"r3\" : [0.0, 0.03, 0.0, -1.0, 0.0, 0.03], \"c1\" : [0.06, 0.015, 1.0, 0.0, 0.005, -1.0]";
    const LIVE_QUERIES: [&str; 9] = [
        "%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",
        "&1f3$eJydkFFPwjAUhf/M9VFCC2PwuHUFGoXhbX3AF7KtjTQGNrui2b+3DEM0mpj40vSec79ze3uTRvBwMq5bRZJQ0LYtDqV9PhXe1ses8EVCVvSno7rGyCmofJPf54ttuJqjt96aNhknlA1DWgnaOPtm9NzVh14pLk3dGU4L4NdCjoFnCx6QvW197brPeIY8USJfB6NujOsnC51SEDohkpDBBObl0yOtl9F+MzMpm+yG79aZvJEzeD1v1QdpkHdcseWOr5VQW2mgfTG+2l8eILSk4IYY4QRjnOKMVSH8Vg9isIfG2aPHSsYgVhsUa5WRy3o47s8rhCVWqNGcs8gXmdCgkxGS6G+O/pMb/cphxOLwsfOE8e/eB3T+mcU=",
        "%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS13$ORIGINAL_DEPENDENCYS9$originalsA2C0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS2$r0C0M5R4R5R6R7R8RaRbRcRdS2$r3R4R5R6R7R8CbA1S11.9$FyUE18XOUbMicpL_0opExtrudeRbSa$SWEPT_EDGE",
        "%B5$QueryM6S12$disambiguationDataA1M2S12$disambiguationTypeS13$ORIGINAL_DEPENDENCYS9$originalsA1C0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS2$r2R4R5R6R7S7$isStartFR8CbA1S11.9$FyUE18XOUbMicpL_0opExtrudeRbS8$CAP_EDGE",
        "%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY",
        "%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS8$TOPOLOGYS8$entitiesA1A2C0M5Sb$derivedFromC0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS2$c1R5R6R7R8R9CcA1S-d.7$imprintRcS7$IMPRINTD1R5C7S4$FACER7R8R9R12RcR13",
        "&1a0$eJx1kN9vgjAQx/+Z2+tCERAfoRRtJuCumMy9kFYabbYJKzUb//1EE18WH+8+ue+Pe0pDeD1rOxaRID60ZpBfyhzO0pnulEknE1L4/0k99lqQGVTIl7xM1k3GNqzMWEl3YgGdNQdzkp9DElCvCIUEfXLGjdNVKoHdBxEAy5ZMKDiawXX2toyBIktqXpUX0PXaXi15m/rA24QIQp4jyNX71u9W4XGz0CmNGu/HWF31F/fvqc5VqAXxwmq6alhZ83onNAwf2u2PtwC8FT5Yb0qIAYYY4RxjlKhwj1dEHiP/MZphQKNLsTyhbKJiDmYQTlqXY0zVLf8C8nHLSPxWbVVh9v268bqe/Tp7bjWq6QPJppkU/gAsoIUJ",
        "%B5$QueryM2S9$queryTypeB9$QueryTypeS5$UNIONSa$subqueriesA2C0M5S12$disambiguationDataA1M2S12$disambiguationTypeS13$ORIGINAL_DEPENDENCYS9$originalsA1C0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpR0Sd$SKETCH_ENTITYSe$sketchEntityIdS2$r0R7CaS4$FACER9RaRbCfA1S11.9$FyUE18XOUbMicpL_0opExtrudeR0Sa$SWEPT_FACEC0M5R3A1M2R4R5R6A1C0M5R7R8R9RaRbRdR0ReRfS2$r2R7R15R9RaRbR17R0R18",
        "%B5$QueryM2S9$queryTypeB9$QueryTypeS5$UNIONSa$subqueriesA2C0M4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpR0S5$DUMMYC0M4R3C6S6$VERTEXR5R6R7CbA1S6.7$OriginpointOpR0Ra",
    ];
    let guesses = |text: &str| -> Vec<(String, String)> { text.split("], ").map(|e| { let (id, v) = e.split_once(" : ").unwrap(); (id.trim_matches('"').to_string(), v.trim_end_matches(']').to_string()) }).collect() };
    let ours_guess = code.lines().find_map(|l| l.trim().strip_prefix("const initialGuess")).and_then(|l| l.split_once("= { ")).map(|(_, r)| r.trim_end_matches(" };").to_string()).unwrap();
    let ours = guesses(&ours_guess);
    let mut names: std::collections::HashMap<String, String> = guesses(LIVE_GUESS).into_iter().map(|(id, v)| (id, ours.iter().find(|(_, w)| *w == v).unwrap_or_else(|| panic!("no entity like live's {v} in {ours_guess}")).0.clone())).collect();
    let fids: Vec<String> = code.lines().filter_map(|l| l.trim().strip_prefix("features.").and_then(|r| r.split_once(" = function")).map(|(f, _)| f.to_string())).collect();
    names.insert("FbZU2oH5hP9eBC6_0".into(), fids[0].clone());
    names.insert("FyUE18XOUbMicpL_0".into(), fids[1].clone());
    use noshape_model::custom::host::packed::{Packed, unpack};
    fn rename(p: &Packed, names: &std::collections::HashMap<String, String>) -> Packed {
        match p {
            Packed::Str(s) => Packed::Str(s.split('.').map(|x| names.get(x).cloned().unwrap_or_else(|| x.to_string())).collect::<Vec<_>>().join(".")),
            Packed::Map(m) => Packed::Map(m.iter().map(|(k, v)| (rename(k, names), rename(v, names))).collect()),
            Packed::Array(a) => Packed::Array(a.iter().map(|v| rename(v, names)).collect()),
            Packed::Tagged(t, v) => Packed::Tagged(t.clone(), Box::new(rename(v, names))),
            other => other.clone(),
        }
    }
    let ours: Vec<&str> = code.split("qCompressed(1.0,\"").skip(1).map(|r| r.split('"').next().unwrap()).collect();
    assert_eq!(ours.len(), LIVE_QUERIES.len(), "{code}");
    for (i, (live, ours)) in LIVE_QUERIES.iter().zip(&ours).enumerate() {
        assert_eq!(unpack(ours).unwrap(), rename(&unpack(live).unwrap(), &names), "query {i}");
    }
    // Live's complete parameter lists (the REST-made Fillet and Chamfer carry only what was sent): each key in
    // order, and the enums as live writes them.
    const LIVE_PARAMS: [(&str, &str, &str, &str); 5] = [
        ("Extrude 1", "extrude", "domain,bodyType,operationType,surfaceOperationType,flatOperationType,entities,surfaceEntities,wallShape,midplane,thickness1,flipWall,thickness2,thickness,endBound,oppositeDirection,depth,endBoundEntityFace,endBoundEntityBody,endBoundEntityVertex,hasOffset,offsetDistance,offsetOppositeDirection,hasExtrudeDirection,extrudeDirection,startOffset,startOffsetBound,startOffsetDistance,startOffsetOppositeDirection,startOffsetEntity,symmetric,hasDraft,draftAngle,draftPullDirection,hasSecondDirection,secondDirectionBound,secondDirectionOppositeDirection,secondDirectionDepth,secondDirectionBoundEntityFace,secondDirectionBoundEntityBody,secondDirectionBoundEntityVertex,hasSecondDirectionOffset,secondDirectionOffsetDistance,secondDirectionOffsetOppositeDirection,hasSecondDirectionDraft,secondDirectionDraftAngle,secondDirectionDraftPullDirection,defaultScope,booleanScope,defaultSurfaceScope,booleanSurfaceScope", "OperationDomain.MODEL,ExtendedToolBodyType.SOLID,NewBodyOperationType.NEW,NewSurfaceOperationType.NEW,FlatOperationType.REMOVE,BoundingType.BLIND,StartOffsetType.BLIND,BoundingType.BLIND"),
        ("Plane 1", "cPlane", "entities,cplaneType,offset,angle,oppositeDirection,flipAlignment,flipNormal,width,height", "CPlaneType.OFFSET"),
        ("Extrude 2", "extrude", "domain,bodyType,operationType,surfaceOperationType,flatOperationType,entities,surfaceEntities,wallShape,midplane,thickness1,flipWall,thickness2,thickness,endBound,oppositeDirection,depth,endBoundEntityFace,endBoundEntityBody,endBoundEntityVertex,hasOffset,offsetDistance,offsetOppositeDirection,hasExtrudeDirection,extrudeDirection,startOffset,startOffsetBound,startOffsetDistance,startOffsetOppositeDirection,startOffsetEntity,symmetric,hasDraft,draftAngle,draftPullDirection,hasSecondDirection,secondDirectionBound,secondDirectionOppositeDirection,secondDirectionDepth,secondDirectionBoundEntityFace,secondDirectionBoundEntityBody,secondDirectionBoundEntityVertex,hasSecondDirectionOffset,secondDirectionOffsetDistance,secondDirectionOffsetOppositeDirection,hasSecondDirectionDraft,secondDirectionDraftAngle,secondDirectionDraftPullDirection,defaultScope,booleanScope,defaultSurfaceScope,booleanSurfaceScope", "OperationDomain.MODEL,ExtendedToolBodyType.SOLID,NewBodyOperationType.NEW,NewSurfaceOperationType.NEW,FlatOperationType.REMOVE,BoundingType.UP_TO_SURFACE,StartOffsetType.BLIND,BoundingType.BLIND"),
        ("Plane 2", "cPlane", "entities,cplaneType,offset,angle,oppositeDirection,flipAlignment,flipNormal,width,height", "CPlaneType.MID_PLANE"),
        ("Plane 3", "cPlane", "entities,cplaneType,offset,angle,oppositeDirection,flipAlignment,flipNormal,width,height", "CPlaneType.PLANE_POINT"),
    ];
    for (name, function, keys, enums) in LIVE_PARAMS {
        let head = format!("annotation {{ \"Feature Name\" : \"{name}\" }}");
        let line = code.lines().skip_while(|l| !l.contains(&head)).nth(1).unwrap_or_else(|| panic!("no {name}"));
        let body = line.split_once(&format!("{function}(context, id + ")).unwrap_or_else(|| panic!("{name}: {line}")).1;
        let ours_keys: Vec<&str> = body.split("\" : ").filter_map(|x| x.rsplit('"').next()).collect();
        let ours_keys = ours_keys[..ours_keys.len() - 1].join(",");
        assert_eq!(ours_keys, keys, "{name}'s parameters");
        let ours_enums: Vec<&str> = body.split(&[' ', ',', '{', '}'][..]).filter(|w| w.contains('.') && w.starts_with(|c: char| c.is_ascii_uppercase()) && !w.contains('(')).collect();
        assert_eq!(ours_enums.join(","), enums, "{name}'s enums");
    }
    // The sketch's entities and the rectangle's corners, as live writes them.
    const LIVE_SKETCH: [&str; 9] = [
        "skLineSegment(sketch, \"r0\", { \"construction\" : false, \"index\" : \"1\" });",
        "skLineSegment(sketch, \"r1\", { \"construction\" : false, \"index\" : \"2\" });",
        "skLineSegment(sketch, \"r2\", { \"construction\" : false, \"index\" : \"3\" });",
        "skLineSegment(sketch, \"r3\", { \"construction\" : false, \"index\" : \"4\" });",
        "skCircle(sketch, \"c1\", { \"construction\" : false, \"index\" : \"1\" });",
        "skConstraint(sketch, \"k0\", { \"constraintType\" : ConstraintType.COINCIDENT, \"index\" : \"1\", \"name\" : \"\", \"localFirst\" : \"r0.end\", \"localSecond\" : \"r1.start\" });",
        "skConstraint(sketch, \"k1\", { \"constraintType\" : ConstraintType.COINCIDENT, \"index\" : \"2\", \"name\" : \"\", \"localFirst\" : \"r1.end\", \"localSecond\" : \"r2.start\" });",
        "skConstraint(sketch, \"k2\", { \"constraintType\" : ConstraintType.COINCIDENT, \"index\" : \"3\", \"name\" : \"\", \"localFirst\" : \"r2.end\", \"localSecond\" : \"r3.start\" });",
        "skConstraint(sketch, \"k3\", { \"constraintType\" : ConstraintType.COINCIDENT, \"index\" : \"4\", \"name\" : \"\", \"localFirst\" : \"r3.end\", \"localSecond\" : \"r0.start\" });",
    ];
    for live in LIVE_SKETCH {
        let mut line = live.to_string();
        for (from, to) in &names {
            line = line.replace(&format!("\"{from}\""), &format!("\"{to}\"")).replace(&format!("\"{from}."), &format!("\"{to}."));
        }
        assert!(code.lines().any(|l| l.trim() == line), "{line}\n{code}");
    }
}

/// Two tabs of one name (two imports of one file): the name is an error that
/// gives both ids, never silently the first tab; each id works.
#[test]
fn a_shared_tab_name_asks_for_the_id() {
    let path = scratch("shared-tab-name.noshape");
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let a = call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "Bracket" }))["tab"]["id"].as_str().unwrap().to_string();
    let b = call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "Bracket" }))["tab"]["id"].as_str().unwrap().to_string();
    let e = try_call(&mut ctx, "query_parts", json!({ "tab": "Bracket" })).unwrap_err().to_json();
    let msg = e["message"].as_str().unwrap();
    assert_eq!(e["param"], "tab", "{e}");
    assert!(msg.contains("2 tabs are named `Bracket`") && msg.contains(&a) && msg.contains(&b), "{msg}");
    for id in [&a, &b] {
        call(&mut ctx, "query_parts", json!({ "tab": id }));
    }
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
    // A parameter the feature doesn't have is named, with the near one.
    let e = try_call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "dept", "input": size })).expect_err("no dept");
    assert!(e.message.starts_with("Extrude 1 has no parameter `dept` (did you mean `depth`?)"), "{}", e.to_json());
    call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "depth", "input": size }));
    call(&mut ctx, "config_set_value", json!({ "feature": ext, "param": "depth", "configuration": "Large", "value": "30 mm" }));
    assert!((volume(&mut ctx) - 40.0 * 20.0 * 10.0).abs() < 1e-3, "default configuration unchanged");
    let act = call(&mut ctx, "config_activate", json!({ "values": { "Size": "Large", "w": "50 mm" } }));
    assert!(act["regeneration"]["failed"].as_array().is_none_or(|f| f.is_empty()), "{act}");
    assert!((volume(&mut ctx) - 50.0 * 20.0 * 30.0).abs() < 1e-3, "{act}");
    assert!(act.get("note").is_none(), "{act}");
    // Mass properties follow the active configuration (qa4 CFG-MASS).
    let m = call(&mut ctx, "query_mass_properties", json!({}));
    assert!((num(&m["volume"]) - 50.0 * 20.0 * 30.0).abs() < 1e-3, "{m}");
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
    // The row menu (live 1717): Duplicate row puts "Copy of Large" right
    // under Large with its value; Set as default; Move up.
    let d = call(&mut ctx, "config_edit_option", json!({ "input": "Size", "option": "Large", "duplicate": true }));
    let copy = d["option"].as_str().unwrap().to_string();
    let list = call(&mut ctx, "config_list", json!({}));
    let rows = list["inputs"].as_array().unwrap().iter().find(|i| i["id"] == size).unwrap()["configurations"].clone();
    assert_eq!(rows[2]["name"], "Copy of Large", "{list}");
    let depth = list["configured"].as_array().unwrap().iter().find(|c| c["param"] == "depth").unwrap().clone();
    assert_eq!(depth["values"][&copy], "25 mm", "{list}");
    call(&mut ctx, "config_edit_option", json!({ "input": "Size", "option": copy, "set_default": true }));
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Part Studio 1 :: Set default configuration");
    call(&mut ctx, "config_edit_option", json!({ "input": "Size", "option": copy, "move": -1 }));
    let list = call(&mut ctx, "config_list", json!({}));
    let size_input = list["inputs"].as_array().unwrap().iter().find(|i| i["id"] == size).unwrap().clone();
    assert_eq!(size_input["default"], copy.as_str(), "{list}");
    assert_eq!(size_input["configurations"][1]["name"], "Copy of Large", "{list}");
    call(&mut ctx, "config_edit_option", json!({ "input": "Size", "option": default_id, "set_default": true }));
    call(&mut ctx, "config_edit_option", json!({ "input": "Size", "option": copy, "delete": true }));

    // A Checkbox suppressing the extrude.
    let c = call(&mut ctx, "config_add_input", json!({ "kind": "checkbox", "name": "Hide block", "default": false }));
    assert_eq!(c["input"], "Hide_block", "the id from the name, as live's: {c}");
    let cb = c["input"].as_str().unwrap().to_string();
    call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "suppressed", "input": cb }));
    call(&mut ctx, "config_set_value", json!({ "feature": ext, "param": "suppressed", "configuration": true, "value": true }));
    call(&mut ctx, "config_activate", json!({ "values": { "Hide block": true } }));
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 0);
    // One call per process (noshape-cli call): the reply says the choice ends with it.
    ctx.one_shot = true;
    let act = call(&mut ctx, "config_activate", json!({ "reset": true }));
    assert!(act["note"].as_str().is_some_and(|n| n.contains("`configuration`")), "{act}");
    ctx.one_shot = false;
    assert!((volume(&mut ctx) - 40.0 * 20.0 * 10.0).abs() < 1e-3);
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Part Studio 1 :: Edit configured value");
    // Unconfigure the depth: the column goes, the feature keeps its value.
    call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "depth", "unconfigure": true }));
    let list = call(&mut ctx, "config_list", json!({}));
    assert!(!list["configured"].as_array().unwrap().iter().any(|c| c["param"] == "depth"), "{list}");
    let e = try_call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "depth", "unconfigure": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("param"), "{e:?}");
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
    // document_create adds `.noshape`: a run whose process id an earlier one had would find its file.
    let _ = std::fs::remove_file(format!("{}.noshape", path.display()));
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
    call(&mut ctx, "sketch_extend", json!({ "sketch": sk, "points": [[-4.5, 6], [0, 8]] }));
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
    // Each fillet has its own radius dimension, no Equal (live, behaviours
    // "qa (2026-10-28) SKETCH-FILLET-STUCK-RADIUS: two corners in one fillet session").
    let st = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let cons = st["constraints"].as_array().unwrap();
    let radius: Vec<&Value> = cons.iter().filter(|c| c["type"] == "Radius").collect();
    assert_eq!(radius.len(), 2, "{cons:?}");
    assert!(radius.iter().all(|r| r["value"] == "R4"), "{radius:?}");
    assert_eq!(radius[0]["expression"], "4 mm");
    assert!(!cons.iter().any(|c| c["type"] == "Equal"));
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
    // The same face by a query; a query that matches several faces is refused.
    let q = call(&mut ctx, "sketch_create", json!({ "plane": { "query": { "kind": "faces", "normal": [0, 0, 1] } }, "name": "By query" }));
    assert!((num(&q["frame"]["origin"][2]) - 10.0).abs() < 1e-6, "{q}");
    call(&mut ctx, "feature_delete", json!({ "features": [q["sketch"].clone()] }));
    let e = try_call(&mut ctx, "sketch_create", json!({ "plane": { "query": { "kind": "faces", "surface": "plane" } } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("plane"), "{e:?}");
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
    // The used edges gone (Extrude 1 suppressed): live's warning on the sketch.
    let s4 = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top", "name": "Uses" }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_use", json!({ "sketch": s4, "references": [top] }));
    let r = call(&mut ctx, "feature_suppress", json!({ "features": ["Extrude 1"], "suppressed": true }));
    let uses = r["regeneration"]["features"].as_array().unwrap().iter().find(|f| f["name"] == "Uses").unwrap().clone();
    assert_eq!((uses["status"].as_str(), uses["message"].as_str()), (Some("Warning"), Some("Some external references are missing.")), "{uses}");
    // Its history entry names the feature (live ver-history-entry-per-edit).
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(last.ends_with(":: Suppress : Extrude 1"), "{last}");
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
    let merged = call(&mut ctx, "history_merge", json!({ "branch": "B1" }));
    assert_eq!(entities(&mut ctx, &sk, "circle").len(), 1);
    assert_eq!(merged["message"], "Merged B1 into Main.", "{merged}");
    // Unprotected, it takes edits again.
    let info = call(&mut ctx, "document_unprotect", json!({}));
    assert_eq!(info["protected"], false);
    // The notice's "Revert to before this merge" (live 2351): a Restore entry, the circle gone.
    call(&mut ctx, "history_restore", json!({ "to": merged["revert_to"] }));
    assert_eq!(entities(&mut ctx, &sk, "circle").len(), 0);
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(last.starts_with("Restore ") && last.contains(" back: "), "{last}");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[20, 0], [25, 0]] }));
}

/// ☰ › Workspace units… isn't an undo step (live doc-units-*: Undo stays off after ✓); the edit before it still is.
#[test]
fn workspace_units_are_not_an_undo_step() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "units-undo.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    call(&mut ctx, "document_set_units", json!({ "length_unit": "in" }));
    let r = call(&mut ctx, "history_undo", json!({}));
    assert_ne!(r["description"], "Change workspace units", "{r}");
    assert_eq!(call(&mut ctx, "document_info", json!({}))["units"]["length"], "in", "the units stay");
}

/// Help › Help and Help › FeatureScript documentation: the user guide, the standard library's modules and sources.
#[test]
fn help_and_the_featurescript_library_read_offline() {
    let mut ctx = Ctx::new();
    let h = call(&mut ctx, "app_user_guide", json!({}));
    assert!(h["guide"].as_str().is_some_and(|g| g.starts_with("# Noshape help")), "{h}");
    // A dialog's ? opens one section: its heading and text, up to the next.
    let c = call(&mut ctx, "app_user_guide", json!({ "section": "Compare" }));
    let text = c["guide"].as_str().unwrap();
    assert!(text.starts_with("### Compare") && !text.contains("### Merging"), "{c}");
    let e = try_call(&mut ctx, "app_user_guide", json!({ "section": "Nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("section"));
    let l = call(&mut ctx, "featurestudio_std", json!({}));
    assert!(l["modules"].as_array().is_some_and(|m| m.iter().any(|m| m == "geometry.fs")), "{l}");
    let m = call(&mut ctx, "featurestudio_std", json!({ "module": "common.fs" }));
    assert!(m["source"].as_str().is_some_and(|s| s.starts_with("FeatureScript")), "{m}");
    let e = try_call(&mut ctx, "featurestudio_std", json!({ "module": "nothing.fs" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("module"));
}

/// Help › System check: the system, its memory, the documents folder's disk and the graphics adapter.
#[test]
fn system_check_reports_the_machine() {
    let mut ctx = Ctx::new();
    let r = call(&mut ctx, "app_system_check", json!({}));
    assert_eq!(r["os"], std::env::consts::OS, "{r}");
    assert!(r["cpus"].as_u64().is_some_and(|n| n >= 1), "{r}");
    assert!(r["memory"]["total"].as_u64().is_some_and(|t| t > 0), "{r}");
    assert!(r["disk"]["total"].as_u64().is_some_and(|t| t > 0), "{r}");
    assert!(r["graphics"]["software"].is_boolean() && r["problems"].is_array(), "{r}");
}

/// A version row's Branch to create workspace…: the workspace starts at that version, not at the current state.
#[test]
fn a_workspace_branches_from_a_version() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "branch-from-version.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[20, 0], [25, 0]] }));
    call(&mut ctx, "history_create_branch", json!({ "name": "B1", "version": "V1" }));
    assert_eq!(entities(&mut ctx, &sk, "circle").len(), 1, "B1 starts at V1");
    let e = try_call(&mut ctx, "history_create_branch", json!({ "name": "B2", "version": "V9" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{}", e.to_json());
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

/// The Measure tool (query_measure): a face's area; a whole part among the entities gives no measurement, as Onshape
/// (live 2463: the readout and the Measure panel stay empty), with a note saying why.
#[test]
fn measure_takes_no_whole_part() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "measure-parts.noshape");
    let top = call(&mut ctx, "query_entities", json!({ "kind": "faces", "normal": [0, 0, 1], "limit": 1 }))["faces"][0]["id"].as_str().unwrap().to_string();
    let m = call(&mut ctx, "query_measure", json!({ "entities": [top] }));
    let area = m["measurements"].as_array().unwrap().iter().find_map(|x| x["value"]["area"].as_f64()).unwrap_or_else(|| panic!("{m}"));
    assert!((area - (80.0 * 40.0 - std::f64::consts::PI * 25.0)).abs() < 1e-3, "{m}");
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    for entities in [json!([part]), json!([part, top])] {
        let m = call(&mut ctx, "query_measure", json!({ "entities": entities }));
        assert_eq!(m["measurements"], json!([]), "{m}");
        assert!(m["note"].as_str().is_some_and(|n| n.contains("no whole part")), "{m}");
    }
}

/// query_entities `of`: what lies next to given entities (a face's edges,
/// two faces' common edge, an edge's faces, a corner's edges).
#[test]
fn entities_next_to_others() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "next-to");
    let face = |ctx: &mut Ctx, n: [i32; 3]| call(ctx, "query_entities", json!({ "kind": "faces", "normal": n }))["faces"][0]["id"].as_str().unwrap().to_string();
    let (top, side) = (face(&mut ctx, [0, 0, 1]), face(&mut ctx, [1, 0, 0]));
    let of = |ctx: &mut Ctx, kind: &str, ids: &[&str]| call(ctx, "query_entities", json!({ "kind": kind, "of": ids }))[kind].as_array().unwrap().clone();
    // The top: four sides and the hole's circle; its neighbours: the four sides and the bore.
    let edges = of(&mut ctx, "edges", &[&top]);
    assert_eq!(edges.len(), 5, "{edges:?}");
    assert!(edges.iter().all(|e| (num(&e["midpoint"][2]) - 20.0).abs() < 1e-9));
    assert_eq!(of(&mut ctx, "faces", &[&top]).len(), 5);
    // Top and +X side: their one common edge, at x 40, z 20, whose faces are those two.
    let common = of(&mut ctx, "edges", &[&top, &side]);
    assert_eq!(common.len(), 1, "{common:?}");
    assert!((num(&common[0]["midpoint"][0]) - 40.0).abs() < 1e-9 && (num(&common[0]["midpoint"][2]) - 20.0).abs() < 1e-9);
    let edge = common[0]["id"].as_str().unwrap().to_string();
    let mut faces: Vec<String> = of(&mut ctx, "faces", &[&edge]).iter().map(|f| f["id"].as_str().unwrap().to_string()).collect();
    faces.sort();
    let mut want = vec![top.clone(), side.clone()];
    want.sort();
    assert_eq!(faces, want);
    // A corner: three edges, three faces; the common edge's two ends.
    let corner = call(&mut ctx, "query_entities", json!({ "kind": "vertices", "at": [[40, 20, 20]] }))["vertices"][0]["id"].as_str().unwrap().to_string();
    assert_eq!((of(&mut ctx, "edges", &[&corner]).len(), of(&mut ctx, "faces", &[&corner]).len()), (3, 3));
    assert_eq!(of(&mut ctx, "vertices", &[&edge]).len(), 2);
    let e = try_call(&mut ctx, "query_entities", json!({ "kind": "edges", "of": ["Part 1"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("of[0]"), "{e:?}");
}

/// Parameters the dialog hides with the given values are ignored, as in the
/// dialog, and the reply says so (qa AGENT-HIDDEN-PARAMS-SILENT: an offset
/// given with a Blind end was dropped without a word).
#[test]
fn hidden_parameters_are_reported_as_ignored() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "ignored.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-40, -20], [40, 20]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    let ex = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm", "hasOffset": true, "offsetDistance": "5 mm" } }));
    assert_eq!(ex["feature_status"]["status"], "Ok", "{ex}");
    let ignored: Vec<&str> = ex["ignored"].as_array().unwrap_or_else(|| panic!("ignored: {ex}")).iter().map(|i| i["param"].as_str().unwrap()).collect();
    assert!(ignored.contains(&"hasOffset") && !ignored.contains(&"depth"), "{ex}");
    let why = ex["ignored"][0]["message"].as_str().unwrap();
    assert!(why.contains("ignored") && why.contains("endBound is"), "says what shows it: {why}");
    // Shown (and used) with an Up to end: nothing ignored.
    let ok = call(&mut ctx, "feature_edit", json!({ "feature": "Extrude 1", "params": { "endBound": "UP_TO_NEXT", "hasOffset": true } }));
    assert!(ok.get("ignored").is_none(), "{ok}");
    let back = call(&mut ctx, "feature_edit", json!({ "feature": "Extrude 1", "params": { "endBound": "BLIND", "hasOffset": true } }));
    assert_eq!(back["ignored"][0]["param"], "hasOffset", "{back}");
}

/// A Variable Studio variable renamed (user report): every Assembly that sees the studio follows too, its mates'
/// offsets, patterns' spacing and configured values (they kept `#w` and failed as undefined).
#[test]
fn variable_studio_rename_reaches_assemblies() {
    let mut ctx = Ctx::new();
    let path = scratch("vs-rename-asm.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "10 mm" } }));
    call(&mut ctx, "variable_create_studio", json!({}));
    call(&mut ctx, "variable_create", json!({ "tab": "Variable Studio 1", "name": "w", "value": "5 mm" }));
    let asm = json!("Assembly 1");
    call(&mut ctx, "assembly_insert", json!({ "tab": asm, "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": asm, "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_set_state", json!({ "tab": asm, "instances": ["Part 1 <1>"], "fixed": true }));
    let face = |ctx: &mut Ctx, instance: &str, z: f64| call(ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": instance, "kind": "faces", "normal": [0, 0, z] }))["faces"][0]["id"].clone();
    let (top, bottom) = (face(&mut ctx, "Part 1 <1>", 1.0), face(&mut ctx, "Part 1 <2>", -1.0));
    call(&mut ctx, "assembly_mate", json!({ "tab": asm, "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": top }, "b": { "instance": "Part 1 <2>", "entity": bottom }, "offset": { "z": "#w" } }));
    let edges = call(&mut ctx, "assembly_entities", json!({ "tab": asm, "instance": "Part 1 <1>", "kind": "edges" }));
    let x_edge = edges["edges"].as_array().unwrap().iter().find(|e| e["curve"] == "line" && num(&e["direction"][0]).abs() > 0.99).unwrap()["id"].clone();
    call(&mut ctx, "assembly_pattern", json!({ "tab": asm, "kind": "linear", "seeds": ["Part 1 <2>"], "direction": { "instance": "Part 1 <1>", "entity": x_edge }, "distance": "#w * 20" }));
    let input = call(&mut ctx, "config_add_input", json!({ "tab": asm, "kind": "list" }))["input"].as_str().unwrap().to_string();
    call(&mut ctx, "config_add_option", json!({ "tab": asm, "input": input, "name": "Far" }));
    call(&mut ctx, "config_configure", json!({ "tab": asm, "mate": "Fastened 1", "param": "offset_z", "input": input }));
    call(&mut ctx, "config_set_value", json!({ "tab": asm, "mate": "Fastened 1", "param": "offset_z", "configuration": "Far", "value": "#w * 2" }));

    let r = call(&mut ctx, "variable_set", json!({ "tab": "Variable Studio 1", "variable": "w", "new_name": "gap" }));
    assert_eq!(r["message"], "Renamed variable 'w' to 'gap' in 3 tabs in this workspace.", "{r}");
    let l = call(&mut ctx, "assembly_list", json!({ "tab": asm }));
    let mate = l["mates"].as_array().unwrap().iter().find(|m| m["name"] == "Fastened 1").unwrap();
    assert_eq!((mate["offset"]["z"].as_str(), mate["ok"].as_bool()), (Some("#gap"), Some(true)), "{mate}");
    assert_eq!(l["patterns"][0]["distance"], "#gap * 20", "{l}");
    let c = call(&mut ctx, "config_list", json!({ "tab": asm }));
    let configured = &c["configured"][0]["values"];
    assert!(configured.as_object().unwrap().values().any(|v| v == "#gap * 2"), "{c}");
    assert!(!c.to_string().contains("#w"), "no #w left: {c}");
}

/// Variable Studio tables in a Part Studio's Variable table: create, insert,
/// reorder (the later table's variable wins), remove, settings.
#[test]
fn variable_studio_tables_in_part_studios() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "vs-tables.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [10, 10]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    // The first studio is inserted into all Part Studios and Assemblies.
    let c = call(&mut ctx, "variable_create_studio", json!({}));
    assert_eq!(c["created"]["name"], "Variable Studio 1", "{c}");
    assert_eq!(c["tables"][0]["automatic"], true, "{c}");
    call(&mut ctx, "variable_create", json!({ "tab": "Variable Studio 1", "name": "w", "value": "30 mm" }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "#w" } }));
    let vol = |ctx: &mut Ctx| num(&call(ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    assert!((vol(&mut ctx) - 3000.0).abs() < 1e-6);
    // A rename follows in every tab that sees the studio (live vs-rename-variable); a taken name becomes `<name>_0`.
    let r = call(&mut ctx, "variable_set", json!({ "tab": "Variable Studio 1", "variable": "w", "new_name": "width" }));
    assert_eq!(r["message"], "Renamed variable 'w' to 'width' in 3 tabs in this workspace.", "{r}");
    assert!((vol(&mut ctx) - 3000.0).abs() < 1e-6, "the extrude's #w is #width now");
    call(&mut ctx, "variable_create", json!({ "tab": "Variable Studio 1", "name": "w", "value": "1 mm" }));
    let r = call(&mut ctx, "variable_set", json!({ "tab": "Variable Studio 1", "variable": "width", "new_name": "w" }));
    assert_eq!((r["message"].as_str(), r["variables"][0]["name"].as_str()), (Some("Changed some variable names to avoid name conflicts."), Some("#w_0")), "{r}");
    call(&mut ctx, "variable_delete", json!({ "tab": "Variable Studio 1", "variable": "w" }));
    call(&mut ctx, "variable_set", json!({ "tab": "Variable Studio 1", "variable": "w_0", "new_name": "w" }));
    assert!((vol(&mut ctx) - 3000.0).abs() < 1e-6);
    // A second studio is inserted explicitly; the later table wins.
    call(&mut ctx, "tab_create", json!({ "kind": "variablestudio", "name": "Sizes" }));
    call(&mut ctx, "variable_create", json!({ "tab": "Sizes", "name": "w", "value": "50 mm" }));
    let t = call(&mut ctx, "variable_insert_studio", json!({ "studio": "Sizes" }));
    assert_eq!(t["tables"].as_array().unwrap().iter().map(|x| x["studio"].as_str().unwrap()).collect::<Vec<_>>(), ["Variable Studio 1", "Sizes"], "{t}");
    assert!((vol(&mut ctx) - 5000.0).abs() < 1e-6);
    call(&mut ctx, "variable_move_studio", json!({ "studio": "Sizes", "by": -1 }));
    assert!((vol(&mut ctx) - 3000.0).abs() < 1e-6);
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Move table up"));
    let e = try_call(&mut ctx, "variable_move_studio", json!({ "studio": "Sizes", "by": -1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("by"), "{e:?}");
    // Remove the automatic one here: Sizes' w.
    let r = call(&mut ctx, "variable_remove_studio", json!({ "studio": "Variable Studio 1" }));
    assert_eq!(r["tables"].as_array().unwrap().len(), 1, "{r}");
    assert!((vol(&mut ctx) - 5000.0).abs() < 1e-6);
    let l = call(&mut ctx, "variable_list", json!({}));
    assert_eq!(l["studio_tables"][0]["studio"], "Sizes", "{l}");
    // Errors name the parameter.
    let e = try_call(&mut ctx, "variable_insert_studio", json!({ "studio": "Variable Studio 1", "variables": ["nope"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("variables[0]"), "{e:?}");
    let e = try_call(&mut ctx, "variable_insert_studio", json!({ "studio": "Part Studio 1" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("studio"), "{e:?}");
    let e = try_call(&mut ctx, "variable_insert_studio", json!({ "studio": "Sizes", "configuration": { "Size": "L" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("configuration.Size"), "{e:?}");
    // Only some variables.
    call(&mut ctx, "variable_create", json!({ "tab": "Variable Studio 1", "name": "h", "value": "5 mm" }));
    let t = call(&mut ctx, "variable_insert_studio", json!({ "studio": "Variable Studio 1", "variables": ["#h"] }));
    assert_eq!(t["tables"][1]["variables"], json!(["#h"]), "{t}");
    assert!((vol(&mut ctx) - 5000.0).abs() < 1e-6, "Variable Studio 1's w stays out");
    // The studio's "Insert into all" checkbox.
    let s = call(&mut ctx, "variable_studio_settings", json!({ "tab": "Sizes", "insert_into_all": true }));
    assert_eq!(s["insert_into_all"], true, "{s}");
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Insert into all Part Studios and Assemblies"));
    // Its table's column widths: an undoable edit kept in the document (live vs-column-resize).
    let s = call(&mut ctx, "variable_studio_settings", json!({ "tab": "Sizes", "column_widths": [140, 140, 200, 140] }));
    assert_eq!(s["column_widths"], json!([140.0, 140.0, 200.0, 140.0]), "{s}");
    assert_eq!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"], "Sizes :: Resize table column");
    call(&mut ctx, "history_undo", json!({ "tab": "Sizes" }));
    assert_eq!(call(&mut ctx, "variable_studio_settings", json!({ "tab": "Sizes" }))["column_widths"], json!([]));
    let e = try_call(&mut ctx, "variable_studio_settings", json!({ "tab": "Sizes", "column_widths": [10, 140, 140, 140] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("column_widths"), "{e:?}");
}

/// A Variable Studio's Configuration panel through the config ops: a List
/// input, a row, a configured Value, the configuration a Part Studio
/// inserts the studio in, the row menu and Unconfigure.
#[test]
fn variable_studio_configurations() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "vs-config.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [10, 10]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "variable_create_studio", json!({}));
    let vs = "Variable Studio 1";
    call(&mut ctx, "variable_create", json!({ "tab": vs, "name": "w", "value": "10 mm" }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "#w" } }));
    let vol = |ctx: &mut Ctx| num(&call(ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    assert!((vol(&mut ctx) - 1000.0).abs() < 1e-6);
    // Inserted into all: no configurations until that is unticked (live).
    let e = try_call(&mut ctx, "config_add_input", json!({ "tab": vs, "kind": "list" })).unwrap_err();
    assert!(e.message.contains("Insert into all"), "{e:?}");
    call(&mut ctx, "variable_studio_settings", json!({ "tab": vs, "insert_into_all": false }));
    call(&mut ctx, "variable_insert_studio", json!({ "studio": vs }));
    let i = call(&mut ctx, "config_add_input", json!({ "tab": vs, "kind": "list" }));
    assert!(i["input"].as_str().unwrap().starts_with("List_"), "{i}");
    assert_eq!(i["configuration"]["inputs"][0]["name"], "Configuration", "{i}");
    let o = call(&mut ctx, "config_add_option", json!({ "tab": vs, "input": "Configuration", "name": "Big" }));
    assert_eq!(o["option"], "Big", "{o}");
    call(&mut ctx, "config_configure", json!({ "tab": vs, "variable": "w", "input": "Configuration" }));
    let c = call(&mut ctx, "config_set_value", json!({ "tab": vs, "variable": "#w", "configuration": "Big", "value": "40 mm" }));
    assert_eq!(c["configuration"]["configured"][0]["values"]["Big"], "40 mm", "{c}");
    // A Part Studio inserts the studio in one configuration.
    call(&mut ctx, "variable_insert_studio", json!({ "studio": vs, "configuration": { "Configuration": "Big" } }));
    assert!((vol(&mut ctx) - 4000.0).abs() < 1e-6);
    let a = call(&mut ctx, "config_activate", json!({ "tab": vs, "values": { "Configuration": "Big" } }));
    assert_eq!(a["active"][i["input"].as_str().unwrap()], "Big", "{a}");
    call(&mut ctx, "variable_insert_studio", json!({ "studio": vs, "configuration": "default" }));
    assert!((vol(&mut ctx) - 1000.0).abs() < 1e-6);
    let t = call(&mut ctx, "variable_insert_studio", json!({ "studio": vs, "configuration": "last_viewed" }));
    assert!((vol(&mut ctx) - 4000.0).abs() < 1e-6, "{t}");
    // The row menu: Duplicate row; Copy table.
    let d = call(&mut ctx, "config_edit_option", json!({ "tab": vs, "input": "Configuration", "option": "Big", "duplicate": true }));
    assert_eq!(d["option"], "Big_copy", "{d}");
    let l = call(&mut ctx, "config_list", json!({ "tab": vs, "copy": true }));
    assert!(l["copied"][i["input"].as_str().unwrap()].as_str().unwrap().contains("Big copy\t40 mm"), "{l}");
    // Unconfigure; errors name the parameter.
    let u = call(&mut ctx, "config_configure", json!({ "tab": vs, "variable": "w", "unconfigure": true }));
    assert_eq!(u["configuration"]["configured"], json!([]), "{u}");
    let e = try_call(&mut ctx, "config_configure", json!({ "tab": vs, "input": "Configuration" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("variable"), "{e:?}");
    let e = try_call(&mut ctx, "config_configure", json!({ "variable": "w", "input": "Configuration", "param": "depth" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("variable"), "{e:?}");
    let e = try_call(&mut ctx, "config_add_input", json!({ "tab": vs, "kind": "checkbox" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"), "{e:?}");
    let cb = call(&mut ctx, "config_add_input", json!({ "tab": vs, "kind": "checkbox", "name": "Lite" }));
    assert_eq!(cb["input"], "Lite", "{cb}");
    // Live's Part Studio Checkbox dialog (CONFIG-INPUT-ID): the Name is
    // required, the id is made from it, Default is checked.
    let e = try_call(&mut ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "checkbox" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"), "{e:?}");
    let cb = call(&mut ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "checkbox", "name": "My flag" }));
    assert_eq!(cb["input"], "My_flag", "{cb}");
    let l = call(&mut ctx, "config_list", json!({ "tab": "Part Studio 1" }));
    let flag = l["inputs"].as_array().unwrap().iter().find(|i| i["id"] == "My_flag").cloned().unwrap();
    assert_eq!(flag["default"], true, "{flag}");
}

/// The Versions and history panel's Properties… and Publish FeatureScript….
#[test]
fn version_properties_and_publish() {
    let mut ctx = Ctx::new();
    new_sketch(&mut ctx, "versions.noshape", "plane:top");
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let v = call(&mut ctx, "history_version_properties", json!({ "version": "V1", "name": "Proto", "description": "First try" }));
    assert_eq!((v["name"].as_str(), v["description"].as_str()), (Some("Proto"), Some("First try")), "{v}");
    let l = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(l["versions"][0]["name"], "Proto", "{l}");
    // Nothing to publish: the unmet requirements.
    let e = try_call(&mut ctx, "history_publish", json!({ "version": "Proto" })).unwrap_err();
    assert!(e.message.contains("at least one custom feature"), "{e:?}");
    call(&mut ctx, "history_create_branch", json!({ "name": "exp" }));
    let b = call(&mut ctx, "history_version_properties", json!({ "branch": "exp", "description": "Thinner walls" }));
    assert_eq!((b["name"].as_str(), b["description"].as_str()), (Some("exp"), Some("Thinner walls")), "{b}");
    let e = try_call(&mut ctx, "history_version_properties", json!({ "version": "V9" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{e:?}");
}

/// IN-FEATURE-MATE-CONNECTOR (live 1851): a Hole placed on a mate connector made inside it (its "Sketch points to
/// place holes" field's "Select mate connectors" button, here at the top face's centroid) cuts the hole there and adds
/// no Mate connector feature; the connector is the Hole's own and goes when the Hole no longer names it.
#[test]
fn a_hole_on_an_in_feature_mate_connector() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "hole-in-feature-mc.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-20, -15], [20, 15]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let s2 = call(&mut ctx, "sketch_create", json!({ "plane": { "query": { "kind": "faces", "normal": [0, 0, 1] } } }))["sketch"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "point", "points": [[5, 0, 10]] }));
    let pt: Vec<Value> = p["created"].as_array().unwrap().iter().filter(|x| x["type"] == "point").map(|x| x["id"].clone()).collect();
    let top = call(&mut ctx, "query_entities", json!({ "kind": "faces", "normal": [0, 0, 1], "limit": 1 }))["faces"][0]["id"].as_str().unwrap().to_string();
    let mc = json!({ "mate_connector": { "originQuery": [top], "entityInferenceType": "CENTROID" } });
    let h = call(&mut ctx, "feature_add", json!({ "type": "hole", "params": { "locations": [mc], "isoHoleTableEx": ["Drilled", "5 mm"], "endStyleV2": "THROUGH" } }));
    assert_eq!(h["feature_status"]["status"], "Ok", "{h}");
    let names: Vec<String> = call(&mut ctx, "feature_list", json!({}))["features"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_string()).collect();
    assert!(!names.iter().any(|n| n.starts_with("Mate connector")), "{names:?}");
    assert_eq!(call(&mut ctx, "query_datums", json!({}))["mate_connectors"], json!([]));
    let g = call(&mut ctx, "feature_get", json!({ "feature": h["feature"] }));
    let subs = g["sub_features"].as_array().unwrap_or_else(|| panic!("{g}"));
    assert_eq!((subs.len(), subs[0]["type"].as_str(), subs[0]["params"]["isForSubFeature"].as_bool()), (1, Some("mateConnector"), Some(true)), "{g}");
    assert_eq!(g["params"]["locations"][0], json!(format!("mc:{}.0", subs[0]["id"].as_str().unwrap())), "{g}");
    // Through the 10 mm block at its centre: a 5 mm hole.
    let hole = std::f64::consts::PI * 2.5 * 2.5 * 10.0;
    let vol = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    assert!((vol - (12000.0 - hole)).abs() < 0.5, "{vol}");
    let c = call(&mut ctx, "query_entities", json!({ "kind": "faces", "surface": "cylinder" }));
    let at = &c["faces"][0]["axis"]["origin"];
    assert!(at[0].as_f64().is_some_and(|x| x.abs() < 1e-6) && at[1].as_f64().is_some_and(|y| y.abs() < 1e-6), "{c}");
    // The location moved to a sketch point: the connector goes with it.
    call(&mut ctx, "feature_edit", json!({ "feature": h["feature"], "params": { "locations": pt } }));
    let g = call(&mut ctx, "feature_get", json!({ "feature": h["feature"] }));
    assert!(g.get("sub_features").is_none(), "{g}");
}

/// An in-feature construction point (Onshape's "Edge point" sub-feature, live 2422: c2-hex-bookcase-derived's
/// "cross-section" Plane through a point a quarter along an edge): `{"point": {cPoint parameters}}` in a selection
/// makes it inside the feature, the plane goes through it, and no point stays in the Part Studio.
#[test]
fn a_plane_through_an_in_feature_edge_point() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "plane-in-feature-point.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-20, -15], [20, 15]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    // The top front edge, along x from -20 to 20 mm.
    let edges = call(&mut ctx, "query_entities", json!({ "kind": "edges", "direction": [1, 0, 0], "near": [0, -15, 10], "limit": 1 }));
    let edge = edges["edges"][0]["id"].as_str().unwrap().to_string();
    let point = json!({ "point": { "pointType": "EDGE_POINT", "entities": [edge], "parameter": "0.25" } });
    let p = call(&mut ctx, "feature_add", json!({ "type": "cPlane", "params": { "cplaneType": "PLANE_POINT", "entities": [point, "plane:right"] } }));
    assert_eq!(p["feature_status"]["status"], "Ok", "{p}");
    let g = call(&mut ctx, "feature_get", json!({ "feature": p["feature"] }));
    let subs = g["sub_features"].as_array().unwrap_or_else(|| panic!("{g}"));
    assert_eq!((subs.len(), subs[0]["type"].as_str()), (1, Some("cPoint")), "{g}");
    // Parallel to Right, a quarter along the edge from whichever end it starts: x = ±10 mm.
    let planes = call(&mut ctx, "query_datums", json!({}))["planes"].clone();
    let plane = planes.as_array().unwrap().iter().find(|x| x["id"].as_str().is_some_and(|i| i.starts_with("fplane:"))).unwrap_or_else(|| panic!("{planes}"));
    let x = num(&plane["frame"]["origin"][0]);
    assert!((x.abs() - 10.0).abs() < 1e-6, "{plane}");
    // The point was the Plane's own: none left in the Part Studio (the block's 8 vertices only).
    let v = call(&mut ctx, "query_entities", json!({ "kind": "vertices", "limit": 100 }));
    assert_eq!(v["vertices"].as_array().unwrap().len(), 8, "{v}");
}

/// A Hole's Standard through the API does what the dialog does: the Units
/// tab that shows it and the table's sizes in the fields not given.
#[test]
fn hole_standard_sets_the_table_sizes() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "hole-standard.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-20, -15], [20, 15]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let s2 = call(&mut ctx, "sketch_create", json!({ "plane": { "query": { "kind": "faces", "normal": [0, 0, 1] } } }))["sketch"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "point", "points": [[5, 0, 10]] }));
    let pt: Vec<Value> = p["created"].as_array().unwrap().iter().filter(|x| x["type"] == "point").map(|x| x["id"].clone()).collect();
    let h = call(&mut ctx, "feature_add", json!({ "type": "hole", "params": { "locations": pt, "isoHoleTableEx": ["Tapped", "Straight tap", "M6"], "endStyleV2": "BLIND", "holeDepthV3": "8 mm", "tappedDepthV3": "8 mm" } }));
    assert_eq!(h["feature_status"]["status"], "Ok", "{h}");
    let g = call(&mut ctx, "feature_get", json!({ "feature": "Hole 1" }));
    assert_eq!((g["params"]["unitsSystem"].as_str(), g["params"]["holeDiameterV3"].as_str(), g["params"]["majorDiameter"].as_str()), (Some("METRIC"), Some("5 mm"), Some("6 mm")), "{g}");
    // The whole path, each level's default below the size, as the dialog stores it.
    assert_eq!(g["params"]["isoHoleTableEx"], json!(["Tapped", "Straight tap", "M6", "1.00 mm (Coarse)", "None"]), "{g}");
    // The 5 mm drill, 8 mm deep, with its 118° tip.
    let r: f64 = 2.5;
    let drill = std::f64::consts::PI * r * r * (8.0 + r / 59f64.to_radians().tan() / 3.0);
    let vol = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    assert!((vol - (12000.0 - drill)).abs() < 0.5, "{vol} vs {}", 12000.0 - drill);
    // Edit to M8: its 6.8 mm drill; a size given with it wins.
    call(&mut ctx, "feature_edit", json!({ "feature": "Hole 1", "params": { "isoHoleTableEx": ["Tapped", "Straight tap", "M8"] } }));
    let g = call(&mut ctx, "feature_get", json!({ "feature": "Hole 1" }));
    assert_eq!(g["params"]["holeDiameterV3"], "6.8 mm", "{g}");
    call(&mut ctx, "feature_edit", json!({ "feature": "Hole 1", "params": { "isoHoleTableEx": ["Tapped", "Straight tap", "M6"], "holeDiameterV3": "5.2 mm" } }));
    let g = call(&mut ctx, "feature_get", json!({ "feature": "Hole 1" }));
    assert_eq!(g["params"]["holeDiameterV3"], "5.2 mm", "{g}");
    // The legacy `style` (hidden, ignored) points to `styleV2` (qa API-HOLE-STYLE);
    // feature_get's own values pass back unchanged.
    let e = try_call(&mut ctx, "feature_edit", json!({ "feature": "Hole 1", "params": { "style": "C_BORE" } })).unwrap_err();
    assert!(e.param.as_deref() == Some("params.style") && e.message.contains("styleV2"), "{e:?}");
    call(&mut ctx, "feature_edit", json!({ "feature": "Hole 1", "params": g["params"].clone() }));
}

/// The Reference manager's Update to latest: a Variable Studio inserted from another document's V1 is listed once that
/// document has a V2, and moved to it. ☰ › Update workspace… is the feature library's (always the latest here).
#[test]
fn update_references_moves_references_to_newer_versions() {
    let mut ctx = Ctx::new();
    let lib = scratch("update-ws-lib.noshape");
    call(&mut ctx, "document_create", json!({ "path": lib.to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "variable_studio", "name": "Sizes" }));
    call(&mut ctx, "variable_create", json!({ "tab": "Sizes", "name": "w", "value": "10 mm" }));
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let user = scratch("update-ws-user.noshape");
    call(&mut ctx, "document_create", json!({ "path": user.to_str().unwrap() }));
    call(&mut ctx, "variable_insert_studio", json!({ "studio": "Sizes", "document": lib.to_str().unwrap(), "version": "V1" }));
    let ws = call(&mut ctx, "document_update_workspace", json!({}));
    assert_eq!(ws["message"], "Workspace is already at the latest feature library version.", "{ws}");
    let none = call(&mut ctx, "document_update_references", json!({}));
    assert_eq!(none["outdated"], json!([]), "{none}");
    // The library moves on.
    call(&mut ctx, "variable_set", json!({ "doc": lib.to_str().unwrap(), "tab": "Sizes", "variable": "w", "value": "20 mm" }));
    call(&mut ctx, "history_create_version", json!({ "doc": lib.to_str().unwrap(), "name": "V2" }));
    let l = call(&mut ctx, "document_update_references", json!({ "doc": user.to_str().unwrap() }));
    assert_eq!((l["outdated"][0]["from"].as_str(), l["outdated"][0]["to"].as_str()), (Some("V1"), Some("V2")), "{l}");
    call(&mut ctx, "document_update_references", json!({ "doc": user.to_str().unwrap(), "apply": true }));
    let v = call(&mut ctx, "variable_list", json!({ "doc": user.to_str().unwrap() }));
    assert_eq!(v["studio_tables"][0]["version"], "V2", "{v}");
    assert!(v["visible_at_end"]["#w"].as_str().is_some_and(|w| w.starts_with("20")), "{v}");
}

/// The sheet metal table through the API: rows as the panel lists them, a
/// bend's radius edit adding a Modify joint, Move up, Convert to rip, and
/// the flat pattern's DXF. The whole table is checked after every step: the
/// rip leaves Bend B's wall a part of its own, and the table took whichever
/// part's data came first (by random part ids), so in about half the runs it
/// listed only that wall's rip, as "Joint A".
#[test]
fn sheet_metal_table_ops() {
    let mut ctx = Ctx::new();
    // Bends as "name radius angle k direction", joints as "name style", in the table's order.
    let table = |ctx: &mut Ctx| {
        let t = call(ctx, "sheetmetal_table", json!({}));
        let s = |v: &Value| v.as_str().unwrap_or("-").to_string();
        let bends: Vec<String> = t["bends"].as_array().map(|a| a.iter().map(|b| format!("{} {} {} {} {}", s(&b["name"]), s(&b["radius"]), s(&b["angle_deg"]), s(&b["k_factor"]), s(&b["direction"]))).collect()).unwrap_or_default();
        let joints: Vec<String> = t["joints"].as_array().map(|a| a.iter().map(|j| format!("{} {}", s(&j["name"]), s(&j["style"]))).collect()).unwrap_or_default();
        (bends, joints)
    };
    let sk = new_sketch(&mut ctx, "sheet-metal-table.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [60, 40]] }));
    call(&mut ctx, "feature_add", json!({ "type": "sheetMetalStart", "params": { "process": "THICKEN", "regions": [r["regions"][0]["id"]], "thickness": "2 mm", "radius": "2 mm" } }));
    assert_eq!(table(&mut ctx), (vec![], vec![]));
    let mut want: Vec<&str> = Vec::new();
    for (dir, near, row) in [([1, 0, 0], [30, 0, 2], "Bend A 2 90 0.45 Up"), ([0, 1, 0], [0, 20, 2], "Bend B 2 90 0.45 Up")] {
        let e = call(&mut ctx, "query_entities", json!({ "kind": "edges", "direction": dir, "near": near, "limit": 1 }))["edges"][0]["id"].clone();
        let f = call(&mut ctx, "feature_add", json!({ "type": "sheetMetalFlange", "params": { "edges": [e], "distance": "20 mm" } }));
        assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
        want.push(row);
        assert_eq!(table(&mut ctx), (want.iter().map(|x| x.to_string()).collect(), vec![]));
    }
    // A radius edit is a Modify joint of that bend.
    call(&mut ctx, "sheetmetal_edit_bend", json!({ "bend": "Bend A", "radius": 3 }));
    let list = call(&mut ctx, "feature_list", json!({}));
    let mj = list["features"].as_array().unwrap().iter().find(|f| f["type"] == "sheetMetalJoint").expect("a Modify joint").clone();
    let g = call(&mut ctx, "feature_get", json!({ "feature": mj["id"] }));
    assert_eq!((g["params"]["jointType"].as_str(), g["params"]["radius"].as_str(), g["params"]["useDefaultRadius"].as_bool()), (Some("BEND"), Some("3 mm"), Some(false)), "{g}");
    assert_eq!(table(&mut ctx), (vec!["Bend A 3 90 0.45 Up".to_string(), "Bend B 2 90 0.45 Up".to_string()], vec![]));
    // Move up: Bend B first.
    let m = call(&mut ctx, "sheetmetal_move_bend", json!({ "bend": "Bend B", "by": -1 }));
    assert_eq!(m["bends"][0]["name"], "Bend B", "{m}");
    let moved = (vec!["Bend B 2 90 0.45 Up".to_string(), "Bend A 3 90 0.45 Up".to_string()], vec![]);
    assert_eq!(table(&mut ctx), moved);
    let e = try_call(&mut ctx, "sheetmetal_move_bend", json!({ "bend": "Bend B", "by": -1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("by"), "{e:?}");
    assert_eq!(table(&mut ctx), moved);
    // The flat pattern's DXF with its two bend lines.
    let dxf = scratch("sheet-metal-flat.dxf");
    let x = call(&mut ctx, "sheetmetal_export_flat", json!({ "part": "Part 1", "file": dxf.to_str().unwrap() }));
    assert_eq!(x["bend_lines"], 2, "{x}");
    assert!(std::fs::read_to_string(&dxf).unwrap().contains("SHEETMETAL_BEND_LINES_"));
    // Convert to rip: a Modify joint set to Rip. Bend B's wall is then a
    // part of its own; the table is still the model's: Bend A and the rip.
    call(&mut ctx, "sheetmetal_convert_to_rip", json!({ "bend": "Bend B" }));
    let n = call(&mut ctx, "feature_list", json!({}))["features"].as_array().unwrap().iter().filter(|f| f["type"] == "sheetMetalJoint").count();
    assert_eq!(n, 2);
    assert_eq!(call(&mut ctx, "query_parts", json!({}))["parts"].as_array().unwrap().len(), 2);
    assert_eq!(table(&mut ctx), (vec!["Bend A 3 90 0.45 Up".to_string()], vec!["Joint B Edge joint".to_string()]));
}

/// An Other joints row's Style ▾ / Type ▾ (live "ui (10-24 a)" (5e, 5f)): a plate with one flange, its bend
/// converted to a rip; the style pick and then Bend change that rip's own Modify joint.
#[test]
fn sheet_metal_other_joint_ops() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "sheet-metal-joints.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [60, 40]] }));
    call(&mut ctx, "feature_add", json!({ "type": "sheetMetalStart", "params": { "process": "THICKEN", "regions": [r["regions"][0]["id"]], "thickness": "2 mm", "radius": "2 mm" } }));
    let e = call(&mut ctx, "query_entities", json!({ "kind": "edges", "direction": [1, 0, 0], "near": [30, 0, 2], "limit": 1 }))["edges"][0]["id"].clone();
    call(&mut ctx, "feature_add", json!({ "type": "sheetMetalFlange", "params": { "edges": [e], "distance": "20 mm" } }));
    call(&mut ctx, "sheetmetal_convert_to_rip", json!({ "bend": "Bend A" }));
    let joints = |ctx: &mut Ctx| call(ctx, "sheetmetal_table", json!({}));
    let t = joints(&mut ctx);
    assert_eq!((t["bends"].as_array().unwrap().len(), t["joints"][0]["name"].as_str(), t["joints"][0]["style"].as_str()), (0, Some("Joint A"), Some("Edge joint")), "{t}");
    call(&mut ctx, "sheetmetal_edit_joint", json!({ "joint": "Joint A", "style": "Butt joint - Direction 1" }));
    let t = joints(&mut ctx);
    assert_eq!(t["joints"][0]["style"], "Butt joint - Direction 1", "{t}");
    call(&mut ctx, "sheetmetal_edit_joint", json!({ "joint": "Joint A", "type": "Bend" }));
    let t = joints(&mut ctx);
    assert!(t["joints"].as_array().unwrap().is_empty() && t["bends"].as_array().unwrap().len() == 1, "back to a bend: {t}");
    let n = call(&mut ctx, "feature_list", json!({}))["features"].as_array().unwrap().iter().filter(|f| f["type"] == "sheetMetalJoint").count();
    assert_eq!(n, 1, "the same Modify joint");
}

/// A Part Studio created in context follows its assembly (live "dx
/// (2026-10-26, P2) Modeling in context"): the plate made thicker shows the
/// update dot, Update context moves the cover up, and the cover's own
/// instance is never part of its context.
#[test]
fn part_studio_created_in_context_updates() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "in-context-update.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-40, -25], [40, 25]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let plate = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": plate }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_set_state", json!({ "instances": [b], "fixed": true }));
    let ps = call(&mut ctx, "assembly_create_part_studio_in_context", json!({}))["part_studio"].as_str().unwrap().to_string();
    let top = call(&mut ctx, "query_entities", json!({ "tab": ps, "kind": "faces", "context": true, "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
    let s2 = call(&mut ctx, "sketch_create", json!({ "tab": ps, "plane": top.clone() }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_use", json!({ "tab": ps, "sketch": s2, "references": [top] }));
    let regions = call(&mut ctx, "sketch_get", json!({ "tab": ps, "sketch": s2 }))["regions"].clone();
    call(&mut ctx, "feature_add", json!({ "tab": ps, "type": "extrude", "params": { "entities": regions.as_array().unwrap().iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), "depth": "3 mm" } }));
    call(&mut ctx, "context_insert_in_assembly", json!({ "tab": ps, "parts": ["Part 1"] }));
    let z = |ctx: &mut Ctx| num(&call(ctx, "query_parts", json!({ "tab": ps }))["parts"][0]["bounding_box"]["min"][2]);
    assert!((z(&mut ctx) - 10.0).abs() < 1e-6);
    let up_to_date = |ctx: &mut Ctx| !call(ctx, "context_list", json!({ "tab": ps }))["contexts"][0]["out_of_date"].as_bool().unwrap();
    assert!(up_to_date(&mut ctx), "the cover's own instance doesn't put its context out of date");
    // The plate 15 mm: the dot, then Update context.
    call(&mut ctx, "feature_edit", json!({ "tab": "Part Studio 1", "feature": "Extrude 1", "params": { "depth": "15 mm" } }));
    assert!(!up_to_date(&mut ctx), "update available");
    let u = call(&mut ctx, "context_update", json!({ "tab": ps, "context": "Context 1" }));
    let names: Vec<&str> = u["contexts"][0]["instances"].as_array().unwrap().iter().map(|i| i["instance"].as_str().unwrap()).collect();
    assert_eq!(names, ["Part 1 <1>"], "{u}");
    assert!(up_to_date(&mut ctx));
    assert!((z(&mut ctx) - 15.0).abs() < 1e-6, "the cover follows the plate");
}

/// A Width mate through the API (api F19, fixed by skasm cdde1c06): the
/// block's two X faces centred between the base's.
#[test]
fn width_mate_centres_the_tab() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "width-mate.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-30, -10], [30, 10]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[100, -10], [120, 10]] }));
    let regions: Vec<Value> = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    let parts = call(&mut ctx, "query_parts", json!({}));
    let by_x = |lo: f64| parts["parts"].as_array().unwrap().iter().find(|p| (num(&p["bounding_box"]["min"][0]) - lo).abs() < 1e-6).unwrap()["id"].as_str().unwrap().to_string();
    let (base, tab) = (by_x(-30.0), by_x(100.0));
    let b = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": base }))["instance"].as_str().unwrap().to_string();
    let t = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": tab }))["instance"].as_str().unwrap().to_string();
    // As agents write it: one `instance`, the `state` by name.
    let st = call(&mut ctx, "assembly_set_state", json!({ "instance": b, "state": "fixed" }));
    assert!(st["instances"].as_array().unwrap().iter().any(|i| i["id"] == b.as_str() && i["fixed"] == true), "{st}");
    let face = |ctx: &mut Ctx, inst: &str, nx: f64| call(ctx, "assembly_entities", json!({ "instance": inst, "kind": "faces", "normal": [nx, 0, 0] }))["faces"][0]["id"].clone();
    let (t_pos, t_neg, b_pos, b_neg) = (face(&mut ctx, &t, 1.0), face(&mut ctx, &t, -1.0), face(&mut ctx, &b, 1.0), face(&mut ctx, &b, -1.0));
    let m = call(&mut ctx, "assembly_mate", json!({ "type": "width", "a": { "instance": t, "entity": t_pos }, "b": { "instance": b, "entity": b_pos }, "c": { "instance": b, "entity": b_neg }, "d": { "instance": t, "entity": t_neg } }));
    assert_eq!(m["mate_ok"], true, "{m}");
    // Centred: the tab's +X face at x = 10. Left: the two slides in the slot
    // plane and the turn about the width axis.
    let f = call(&mut ctx, "assembly_entities", json!({ "instance": t, "kind": "faces", "normal": [1, 0, 0] }));
    assert!((num(&f["faces"][0]["centroid"][0]) - 10.0).abs() < 1e-6, "{f}");
    assert_eq!(call(&mut ctx, "assembly_list", json!({}))["dof"], 3);
}

/// drawing_get lists the silhouettes of curved faces with the model edges,
/// and a callout anchors on one (qa4 DRW-SIL).
#[test]
fn drawing_lists_silhouettes() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "drawing-sil.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [10, 0]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "30 mm" } }));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    // The side views: the cylinder's sides are silhouettes (no model edge).
    let sil: Vec<Value> = g["views"].as_array().unwrap().iter().flat_map(|v| v["edges"].as_array().cloned().unwrap_or_default()).filter(|e| e["type"] == "silhouette").collect();
    assert!(sil.len() >= 2, "{g}");
    let s = &sil[0];
    let (a, b) = (&s["a"], &s["b"]);
    assert!(a.is_array() && b.is_array(), "a straight silhouette is a segment: {s}");
    let mid = [(num(&a[0]) + num(&b[0])) / 2.0, (num(&a[1]) + num(&b[1])) / 2.0];
    let c = call(&mut ctx, "drawing_callout", json!({ "drawing": tab, "anchor": mid, "position": [mid[0] + 20.0, mid[1] + 20.0] }));
    assert!(c["annotation"].is_u64(), "{c}");
    // Both silhouettes of one side view: a Ø dimension (help D2).
    let mids: Vec<[f64; 2]> = g["views"].as_array().unwrap().iter().map(|v| v["edges"].as_array().cloned().unwrap_or_default().into_iter().filter(|e| e["type"] == "silhouette" && e["a"].is_array()).map(|e| [(num(&e["a"][0]) + num(&e["b"][0])) / 2.0, (num(&e["a"][1]) + num(&e["b"][1])) / 2.0]).collect::<Vec<_>>()).find(|m| m.len() == 2).expect("a side view with two silhouettes");
    let place = [(mids[0][0] + mids[1][0]) / 2.0 + 3.0, (mids[0][1] + mids[1][1]) / 2.0 + 25.0];
    let dia = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": mids, "place": place }));
    assert_eq!(dia["text"], "Ø20", "{dia}");
    // The Diameter tool on one silhouette: the same (help D15).
    let one = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "diameter", "picks": [mids[0]], "place": [place[0], place[1] + 10.0] }));
    assert_eq!(one["text"], "Ø20", "{one}");
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
    // A view id that isn't there names the ones that are.
    let e = try_call(&mut ctx, "drawing_delete", json!({ "drawing": tab, "views": [99] })).expect_err("no view 99");
    assert_eq!(e.message, "no view 99 on Sheet1; its views: 1, 2, 3, 4", "{}", e.to_json());
}

/// The first view inserted into an empty drawing is scaled to fit (live
/// 1812: a 100 x 50 x 20 block's Front view 1:2 on ANSI A, a 10 x 10 x 5
/// one's 6:1), and gives the sheet that scale: the next view takes it.
#[test]
fn first_inserted_view_is_scaled_to_fit() {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("first_view_scale.noshape").to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
    let src = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

annotation { "Feature Type Name" : "Block" }
export const block = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "L" }
        isLength(definition.l, { (millimeter) : [1, 100, 1000] } as LengthBoundSpec);
        annotation { "Name" : "W" }
        isLength(definition.w, { (millimeter) : [1, 50, 1000] } as LengthBoundSpec);
        annotation { "Name" : "H" }
        isLength(definition.h, { (millimeter) : [1, 20, 1000] } as LengthBoundSpec);
    }
    {
        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(definition.l, definition.w, definition.h) });
    });
"#;
    call(&mut ctx, "featurestudio_edit", json!({ "source": src }));
    let c = call(&mut ctx, "featurestudio_commit", json!({}));
    let ty = c["feature_types"].as_array().unwrap().iter().find(|t| t["name"] == "Block").unwrap()["type"].as_str().unwrap().to_string();
    let small = call(&mut ctx, "tab_create", json!({ "kind": "partstudio" }))["tab"]["name"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "tab": "Part Studio 1", "type": ty, "params": { "l": "100 mm", "w": "50 mm", "h": "20 mm" } }));
    call(&mut ctx, "feature_add", json!({ "tab": small, "type": ty, "params": { "l": "10 mm", "w": "10 mm", "h": "5 mm" } }));
    for (studio, want) in [("Part Studio 1", 0.5), (small.as_str(), 6.0)] {
        let d = call(&mut ctx, "drawing_create", json!({ "template": "ANSI_A_INCH.dwt" }));
        let drawing = d["drawing"].as_str().unwrap().to_string();
        let v = call(&mut ctx, "drawing_insert_view", json!({ "drawing": drawing, "source": studio, "position": [100, 120] }));
        let scale = |r: &Value, id: &Value| num(&r["views"].as_array().unwrap().iter().find(|x| x["id"] == *id).unwrap()["scale"]);
        assert!((scale(&v, &v["view"]) - want).abs() < 1e-9, "{studio}: {v}");
        let w = call(&mut ctx, "drawing_insert_view", json!({ "drawing": drawing, "source": studio, "orientation": "top", "position": [100, 60] }));
        assert!((scale(&w, &w["view"]) - want).abs() < 1e-9, "{studio}, second view: {w}");
    }
}

/// A fit spline with Onshape's end derivatives (sketch_draw `start_derivative` /
/// `end_derivative`, as Onshape stores them): its end handles where live's are
/// (cf-curve-pattern-normal's Sketch 2: startHandle x 25.107, endHandle x -32.296 mm).
#[test]
fn a_spline_takes_end_derivatives() {
    let mut ctx = Ctx::new();
    let path = scratch("spline-ends.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let points = json!([["74.051873976 mm", "-70.884396716 mm"], [0, 0], ["-74.051873976 mm", "91.370642185 mm"]]);
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "spline", "points": points, "start_derivative": ["-304.11224264883985 mm", "0 mm"], "end_derivative": ["-242.21679513831001 mm", "0 mm"] }));
    let handles: Vec<[f64; 2]> = r["created"].as_array().unwrap().iter().filter(|e| e["type"] == "point").map(|e| [num(&e["at"][0]), num(&e["at"][1])]).filter(|p| p[0].abs() > 1e-6).collect();
    let near = |p: [f64; 2]| handles.iter().any(|h| (h[0] - p[0]).abs() < 1e-3 && (h[1] - p[1]).abs() < 1e-3);
    assert!(near([25.107119551, -70.884396716]) && near([-32.296054065, 91.370642185]), "{handles:?}");
    let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "spline", "points": points, "start_derivative": ["1 mm", "0 mm"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("end_derivative"), "{e:?}");
}

/// A Spline whose last click lands on its first point closes into a periodic spline (live: "clicking the first point
/// closes the spline"; cf-surface-pattern's Split 1 tool): `closed`, one curve, one region inside it.
#[test]
fn a_spline_closes_on_its_first_point() {
    let mut ctx = Ctx::new();
    let path = scratch("spline-closed.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "spline", "points": [[14, 33], [-58, 18], [-52, -26], [70, -32], [14, 33]] }));
    let splines = created(&r, "spline");
    assert_eq!(splines.len(), 1, "{r}");
    assert_eq!(splines[0]["closed"], true, "{r}");
    assert_eq!(splines[0]["point_ids"].as_array().map(Vec::len), Some(5), "the first point last too: {r}");
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    assert_eq!(g["regions"].as_array().map(Vec::len), Some(1), "{g}");
    // An open one (its last click elsewhere) isn't.
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "spline", "points": [[100, 0], [120, 20], [140, 0]] }));
    assert!(created(&r, "spline")[0].get("closed").is_none(), "{r}");
}

/// A spline's handle arms (Onshape's spline handles, the lines from its ends to the handle tips): listed by
/// sketch_get as lines of the spline, constrained and dimensioned as lines; deleting the spline takes them.
#[test]
fn a_splines_handle_arms_take_constraints_and_dimensions() {
    let mut ctx = Ctx::new();
    let path = scratch("spline-handle-arms.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "spline", "points": [[0, 0], [30, 20], [60, 0]] }, { "tool": "line", "points": [[0, -20], [40, -20]] }] }));
    let spline = created(&r, "spline")[0].clone();
    let line = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "line").unwrap()["id"].clone();
    let arms = spline["handle_arms"].as_array().cloned().unwrap_or_default();
    assert_eq!(arms.len(), 2, "{spline}");
    // `created` is what was drawn, one entry a shape (the parity translator pairs them with live's in order:
    // c2-impeller-loft's construction line had paired with an arm).
    let drawn: Vec<&str> = r["created"].as_array().unwrap().iter().filter(|e| e["type"] != "point").filter_map(|e| e["type"].as_str()).collect();
    assert_eq!(drawn, ["spline", "line"], "{r}");
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let entity = |g: &Value, id: &Value| g["entities"].as_array().unwrap().iter().find(|e| e["id"] == *id).cloned().unwrap();
    let start_arm = entity(&g, &arms[0]);
    assert_eq!((start_arm["type"].as_str(), start_arm["handle_of"].as_str(), start_arm["handle_end"].as_str(), start_arm["construction"].as_bool()), (Some("line"), spline["id"].as_str(), Some("start"), Some(true)), "{start_arm}");
    assert_eq!(entity(&g, &arms[1])["handle_end"], "end");
    // Horizontal, 15 long; the end arm at 30° to the line.
    call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "horizontal", "entities": [arms[0]] }));
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sk, "entities": [arms[0]], "value": "15 mm" }));
    call(&mut ctx, "sketch_dimension", json!({ "sketch": sk, "kind": "angle", "entities": [arms[1], line], "value": "30 deg" }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let ends = |e: &Value| -> (glam::DVec2, glam::DVec2) {
        let p = |k: &str| glam::DVec2::new(e[k][0].as_f64().unwrap(), e[k][1].as_f64().unwrap());
        (p("start"), p("end"))
    };
    let (a, b) = ends(&entity(&g, &arms[0]));
    assert!((a.y - b.y).abs() < 1e-6 && (a.distance(b) - 15.0).abs() < 1e-6, "{a} {b}");
    let (c, d) = ends(&entity(&g, &arms[1]));
    let (l0, l1) = ends(&entity(&g, &line));
    let t = (l1 - l0).angle_to(d - c).abs();
    assert!((t.min(std::f64::consts::PI - t) - 30f64.to_radians()).abs() < 1e-6, "{}", t.to_degrees());
    // The arm alone isn't deleted; with its spline it goes.
    let _ = try_call(&mut ctx, "sketch_delete", json!({ "sketch": sk, "entities": [arms[0]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    assert!(g["entities"].as_array().unwrap().iter().any(|e| e["id"] == arms[0]));
    call(&mut ctx, "sketch_delete", json!({ "sketch": sk, "entities": [spline["id"]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    assert!(!g["entities"].as_array().unwrap().iter().any(|e| e["id"] == arms[0] || e["id"] == arms[1]), "{g}");
}

/// A Bezier's control polygon is lines pole to pole that take constraints, as its arm is a spline's (live 2187: the
/// first segment of (0,0) (30,20) (60,0) selects alone and h makes it horizontal); a conic has none.
#[test]
fn a_beziers_control_polygon_takes_constraints() {
    let mut ctx = Ctx::new();
    let path = scratch("bezier-polygon.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "bezier", "points": [[0, 0], [30, 20], [60, 0]] }, { "tool": "conic", "points": [[0, -40], [60, -40], [30, -20]] }] }));
    let drawn: Vec<&str> = r["created"].as_array().unwrap().iter().filter(|e| e["type"] != "point").filter_map(|e| e["type"].as_str()).collect();
    assert_eq!(drawn, ["bspline", "conic"], "{r}");
    let bezier = created(&r, "bspline")[0].clone();
    let polygon = bezier["handle_arms"].as_array().cloned().unwrap_or_default();
    assert_eq!(polygon.len(), 2, "{bezier}");
    assert!(created(&r, "conic")[0].get("handle_arms").is_none());
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let entity = |g: &Value, id: &Value| g["entities"].as_array().unwrap().iter().find(|e| e["id"] == *id).cloned().unwrap();
    let first = entity(&g, &polygon[0]);
    assert_eq!((first["type"].as_str(), first["handle_of"].as_str(), first["polygon_index"].as_u64(), first["construction"].as_bool()), (Some("line"), bezier["id"].as_str(), Some(0), Some(true)), "{first}");
    call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "horizontal", "entities": [polygon[0]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let (a, b) = { let e = entity(&g, &polygon[0]); (e["start"][1].as_f64().unwrap(), e["end"][1].as_f64().unwrap()) };
    assert!((a - b).abs() < 1e-6, "{a} {b}");
    let bezier = g["entities"].as_array().unwrap().iter().find(|e| e["id"] == bezier["id"]).cloned().unwrap();
    assert!((bezier["control_points"][1][1].as_f64().unwrap() - bezier["control_points"][0][1].as_f64().unwrap()).abs() < 1e-6, "the second pole level with the first: {bezier}");
    call(&mut ctx, "sketch_delete", json!({ "sketch": sk, "entities": [bezier["id"]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    assert!(!g["entities"].as_array().unwrap().iter().any(|e| polygon.contains(&e["id"])), "{g}");
}

/// Live's template documents: ANSI A–D (no E), ISO A0–A4 and A4 Portrait,
/// JIS the same sizes; a JIS drawing's title block in Japanese, third angle,
/// its texts in Onshape-Standard-JapaneseFields (live tpl-JIS_A3.dxf).
#[test]
fn jis_templates_as_lives() {
    let mut ctx = Ctx::new();
    let path = scratch("jis-templates.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let t = call(&mut ctx, "drawing_templates", json!({}));
    let files: Vec<&str> = t["templates"].as_array().unwrap().iter().filter_map(|r| r["file"].as_str()).collect();
    for f in ["JIS_A0.dwt", "JIS_A4_Portrait.dwt", "ISO_A4_Portrait.dwt", "ANSI_D_MM.dwt"] {
        assert!(files.contains(&f), "{f}: {files:?}");
    }
    assert!(!files.iter().any(|f| f.starts_with("ANSI_E")), "{files:?}");
    let d = call(&mut ctx, "drawing_create", json!({ "template": "JIS_A3.dwt" }));
    let out = scratch("jis-a3.dxf");
    call(&mut ctx, "drawing_export", json!({ "drawing": d["drawing"], "format": "DXF", "file": out.to_str().unwrap(), "version": "2018" }));
    let dxf = std::fs::read_to_string(&out).unwrap();
    assert!(dxf.contains("第三角法") && dxf.contains("図面を拡大縮小しない"), "Japanese labels");
    assert!(!dxf.contains("THIRD ANGLE PROJECTION"));
    assert!(dxf.matches("Onshape-Standard-JapaneseFields").count() >= 44 + 20, "title block and zone texts in the Japanese style");
}

#[test]
fn drawing_views_and_dimensions() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing.noshape");
    let t = call(&mut ctx, "drawing_templates", json!({}));
    assert!(t["templates"].as_array().unwrap().iter().any(|r| r["name"] == "ISO A3"));
    // A template by its file name with or without `.dwt`; an unknown one lists them.
    let stem = call(&mut ctx, "drawing_create", json!({ "template": "ansi_a_mm" }));
    assert!(stem["drawing"].is_string(), "{stem}");
    let e = try_call(&mut ctx, "drawing_create", json!({ "template": "ANSI_Q" })).unwrap_err().to_json().to_string();
    assert!(e.contains("no template `ANSI_Q`") && e.contains("ISO_A3.dwt"), "{e}");
    call(&mut ctx, "tab_delete", json!({ "tab": stem["drawing"] }));
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
    // Shift+Q: the edge's midpoint and a corner (half the edge), two quadrant points (the diameter).
    let half = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": [mid, [num(&long["a"][0]), num(&long["a"][1])]], "place": [mid[0], mid[1] + 30.0], "mid_quad": true }));
    assert!(half["text"].as_str().unwrap().contains("40"), "{half}");
    let quads = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": [[cx + r, cy], [cx - r, cy]], "place": [cx, cy + 25.0], "mid_quad": true }));
    assert!(quads["text"].as_str().unwrap().contains("10"), "{quads}");
    // The dimension palette: prefix, precision, a symmetrical tolerance.
    let lin_id = lin["annotation"].as_u64().unwrap();
    let ed = call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": lin_id, "prefix": "2X ", "precision": 1, "tolerance": "symmetrical", "tolerance_value": 0.1 }));
    let text = ed["annotations"].as_array().unwrap().iter().find(|x| x["id"] == lin_id).unwrap()["text"].as_str().unwrap().to_string();
    // (ISO_A3.dwt: a decimal comma, live "30±0,1".)
    assert!(text.contains("2X") && text.contains("80") && text.contains("±0,1"), "{text}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert!(hist["history"][0]["description"].as_str().unwrap().ends_with(":: Edit dimension"), "{hist}");
    let ed = call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": lin_id, "value": "REF", "tolerance": "None" }));
    let text = ed["annotations"].as_array().unwrap().iter().find(|x| x["id"] == lin_id).unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains("REF") && !text.contains("±"), "{text}");
    // The override keeps its prefix (D39: the other fields stay).
    assert!(text.contains("2X REF"), "{text}");
    // Other dimensions take a typed value too, after their symbol (D39).
    let ed = call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": dia["annotation"], "value": "X" }));
    let text = ed["annotations"].as_array().unwrap().iter().find(|x| x["id"] == dia["annotation"]).unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains("ØX"), "{text}");
    call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": dia["annotation"], "value": "" }));
    // A callout on the linear dimension's text (qa CALLOUT-ON-DIMENSION):
    // its leader ends on the dimension; deleting the dimension deletes it.
    let ann = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["annotations"].as_array().unwrap().clone();
    // 10 mm along its dimension line from its text (the 40 mm dimension's
    // extension line runs through the text's x).
    let pos = ann(&mut ctx).iter().find(|a| a["id"] == lin_id).unwrap()["position"].clone();
    let at = json!([num(&pos[0]) + 10.0, num(&pos[1])]);
    let co = call(&mut ctx, "drawing_callout", json!({ "drawing": tab, "anchor": at, "position": [num(&at[0]) + 25.0, num(&at[1]) + 25.0], "text": "7" }));
    let cid = co["annotation"].as_u64().unwrap();
    let leader_end = ann(&mut ctx).iter().find(|a| a["id"] == cid).unwrap()["leaders"][0]["path"].as_array().unwrap().last().unwrap().clone();
    assert!((num(&leader_end[1]) - num(&at[1])).abs() < 5.0, "the leader ends on the dimension near its text: {leader_end} vs {at}");
    call(&mut ctx, "drawing_delete", json!({ "drawing": tab, "annotations": [lin_id] }));
    assert!(!ann(&mut ctx).iter().any(|a| a["id"] == cid), "the callout went with its dimension");
    // The dual row: this dimension's dual precision, units and Show/hide
    // units (live asbdd-9: Ø10 "[0.39 in]"; this ISO_A3.dwt drawing writes
    // its decimal comma).
    let dia_id = dia["annotation"].as_u64().unwrap();
    let ed = call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": dia_id, "dual": "dual", "dual_precision": 2, "dual_units": "Inches", "dual_show_units": true }));
    let text = ed["annotations"].as_array().unwrap().iter().find(|x| x["id"] == dia_id).unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains("[0,39 in]"), "{text}");
    let ed = call(&mut ctx, "drawing_edit_dimension", json!({ "drawing": tab, "annotation": dia_id, "dual_show_units": false }));
    let text = ed["annotations"].as_array().unwrap().iter().find(|x| x["id"] == dia_id).unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains("[0,39]"), "{text}");
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
    // Custom sheets with an empty range: live's error (drawing/92).
    let e = try_call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "DXF", "file": dxf.to_str().unwrap(), "sheets": " " })).unwrap_err();
    assert_eq!((e.param.as_deref(), e.message.as_str()), (Some("sheets"), "Sheet range cannot be empty"), "{}", e.to_json());
    // Release 14 (live's default) is written in the drawing's code page, not UTF-8.
    let bytes = std::fs::read(&dxf).unwrap();
    assert!(bytes.windows(8).any(|w| w == b"ENTITIES"));
    // Unset properties keep their "----" in a DXF (live 1857), not in a PDF.
    assert!(bytes.windows(4).any(|w| w == b"----"), "no placeholder in the DXF");
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

/// A STEP assembly with a subassembly (live request 1742 (A), live's own
/// file): Sub keeps its Assembly tab, instanced in the root one, all parts
/// from the one Part Studio; tabs Assembly 1, Sub, the parts.
#[test]
fn import_step_assembly_keeps_its_subassembly() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "import_nested.noshape");
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../kernel/tests/data/step/nested-asm.step");
    let r = call(&mut ctx, "import_files", json!({ "files": [file] }));
    let info = call(&mut ctx, "document_info", json!({}));
    let tabs: Vec<String> = info["tabs"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    let at = |n: &str| tabs.iter().position(|t| t.starts_with(n)).unwrap_or_else(|| panic!("{n} in {tabs:?}"));
    assert!(at("Assembly 1 (1)") < at("Sub") && at("Sub") < at("nested-asm parts"), "{tabs:?}");
    let names = |ctx: &mut Ctx, tab: &str| {
        let a = call(ctx, "assembly_list", json!({ "tab": tab }));
        let mut v: Vec<String> = a["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
        v.sort();
        v
    };
    let root = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Assembly" && t["name"] != "Sub").and_then(|t| t["name"].as_str()).unwrap().to_string();
    assert_eq!(names(&mut ctx, &root), ["Part 1 <1>", "Sub <1>"]);
    assert_eq!(names(&mut ctx, "Sub"), ["Part 2 <1>", "Part 3 <1>"]);
    assert_eq!(r["parts"].as_array().unwrap().len(), 3, "{r}");
}

/// Import files… › Split into multiple documents (live behaviours "dx 1742 (B)", live's own file): a folder named
/// after the file holding "Sub" (its Assembly, a Part Studio per part, CAD Imports, V1) and "nested-asm.step"
/// (Assembly 1, Part 1's Part Studio, CAD Imports, V1) whose Assembly 1 uses Sub's Assembly at Sub's V1; with
/// "Create one document per part or assembly" every part has its own document too.
#[test]
fn documents_import_splits_a_step_assembly_into_linked_documents() {
    let dir = scratch("docs-import-split");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let mut ctx = Ctx::new();
    ctx.documents = Some(dir.clone());
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../kernel/tests/data/step/nested-asm.step");
    let r = call(&mut ctx, "documents_import", json!({ "files": [file], "mode": "split" }));
    let folder = dir.join("nested-asm.step");
    assert_eq!(r["folder"], folder.display().to_string(), "{r}");
    let (sub, top) = (folder.join("Sub.noshape"), folder.join("nested-asm.step.noshape"));
    assert_eq!(r["documents"], json!([sub.display().to_string(), top.display().to_string()]), "{r}");
    assert_eq!(r["open"], top.display().to_string());
    let tabs = |ctx: &mut Ctx| -> Vec<String> { call(ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| t["name"].as_str().unwrap().to_string()).collect() };
    let instances = |ctx: &mut Ctx, tab: &str| -> Vec<(String, Value)> {
        let a = call(ctx, "assembly_list", json!({ "tab": tab }));
        a["instances"].as_array().unwrap().iter().map(|i| (i["name"].as_str().unwrap().to_string(), i["source"].clone())).collect()
    };
    let versions = |ctx: &mut Ctx| -> Vec<String> { call(ctx, "history_list", json!({}))["versions"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap().to_string()).collect() };
    // CAD Imports (live 2424 (5)): a Parasolid file per part a document has the Part Studio of, the STEP in the top one.
    let files = |ctx: &mut Ctx| -> Vec<String> {
        let mut f: Vec<String> = call(ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] == "File").map(|t| t["name"].as_str().unwrap().to_string()).collect();
        f.sort();
        f
    };
    call(&mut ctx, "document_open", json!({ "path": sub.to_str().unwrap() }));
    assert_eq!(tabs(&mut ctx), ["Sub", "Part 2", "Part 3"]);
    assert_eq!(files(&mut ctx), ["Part 2.x_t", "Part 3.x_t"]);
    let names: Vec<String> = instances(&mut ctx, "Sub").into_iter().map(|i| i.0).collect();
    assert_eq!(names, ["Part 2 <1>", "Part 3 <1>"]);
    assert_eq!(versions(&mut ctx), ["V1"]);
    call(&mut ctx, "document_open", json!({ "path": top.to_str().unwrap() }));
    assert_eq!(tabs(&mut ctx), ["Assembly 1", "Part 1"]);
    assert_eq!(files(&mut ctx), ["Part 1.x_t", "nested-asm.step"]);
    let top_instances = instances(&mut ctx, "Assembly 1");
    let names: Vec<&str> = top_instances.iter().map(|i| i.0.as_str()).collect();
    // Live 1742: "Sub <1>" then "Part 1 <1>" (the reverse of the file's order, as Import to a single document).
    assert_eq!(names, ["Sub <1>", "Part 1 <1>"]);
    assert_eq!(top_instances[0].1["version_of"]["document"], "Sub.noshape", "Sub <1> is Sub's document: {:?}", top_instances[0]);
    assert_eq!(versions(&mut ctx), ["V1"]);
    // Sub's parts come through its document: three occurrences in all.
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    assert_eq!(list["occurrences"].as_array().unwrap().len(), 3, "{list}");
    call(&mut ctx, "document_close", json!({}));
    call(&mut ctx, "document_close", json!({}));
    // Again with one document per part: the folder "nested-asm.step (1)", Part 1 to Part 3 each their own document.
    let r = call(&mut ctx, "documents_import", json!({ "files": [file], "mode": "per_part" }));
    let folder = dir.join("nested-asm.step (1)");
    let mut docs: Vec<String> = r["documents"].as_array().unwrap().iter().map(|d| std::path::Path::new(d.as_str().unwrap()).strip_prefix(&folder).unwrap().display().to_string()).collect();
    docs.sort();
    // (Live 2424 (4): "Part 1.x_t", after the Parasolid file each holds.)
    assert_eq!(docs, ["Part 1.x_t.noshape", "Part 2.x_t.noshape", "Part 3.x_t.noshape", "Sub.noshape", "nested-asm.step.noshape"], "{r}");
    call(&mut ctx, "document_open", json!({ "path": folder.join("Sub.noshape").to_str().unwrap() }));
    assert_eq!(tabs(&mut ctx), ["Sub"]);
    assert_eq!(files(&mut ctx), Vec::<String>::new(), "Sub holds only its assembly");
    let subs = instances(&mut ctx, "Sub");
    assert!(subs.iter().all(|i| i.1["version_of"]["part"].is_string()), "{subs:?}");
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "Sub" }));
    assert_eq!(list["occurrences"].as_array().unwrap().len(), 2, "{list}");
    // Not a STEP assembly, or a nonexistent file: named.
    let e = try_call(&mut ctx, "documents_import", json!({ "files": ["/tmp/nope.step"], "mode": "split" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("files[0]"));
    let e = try_call(&mut ctx, "documents_import", json!({ "files": [file], "mode": "split", "combine": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("combine"));
}

/// Split into multiple documents of a STEP with loose parts (live 2511 (1), live's own file): a folder named after
/// the file with one document "parts3.step": its Assembly "parts3" placing each part once, a Part Studio per part.
#[test]
fn documents_import_splits_loose_step_parts_into_one_document() {
    let dir = scratch("docs-import-split-parts");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let mut ctx = Ctx::new();
    ctx.documents = Some(dir.clone());
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../kernel/tests/data/step/parts3.step");
    let r = call(&mut ctx, "documents_import", json!({ "files": [file], "mode": "split" }));
    let folder = dir.join("parts3.step");
    assert_eq!(r["folder"], folder.display().to_string(), "{r}");
    let top = folder.join("parts3.step.noshape");
    assert_eq!(r["documents"], json!([top.display().to_string()]), "{r}");
    call(&mut ctx, "document_open", json!({ "path": top.to_str().unwrap() }));
    let mut tabs: Vec<String> = call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(tabs.remove(0), "parts3", "the Assembly first, named after the file");
    tabs.sort();
    assert_eq!(tabs, ["Part 1", "Part 2", "Part 3"]);
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "parts3" }));
    let mut names: Vec<String> = list["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    names.sort();
    assert_eq!(names, ["Part 1 <1>", "Part 2 <1>", "Part 3 <1>"], "{list}");
    call(&mut ctx, "document_close", json!({}));
}

/// Split into multiple documents of a Parasolid assembly (live 2511 (2), live's own file): as a STEP one, a folder
/// "asm2.x_t" with "Sub" (its Assembly and Part Studios) and "asm2.x_t" whose Assembly 1 uses Sub's at its V1.
#[test]
fn documents_import_splits_a_parasolid_assembly() {
    let dir = scratch("docs-import-split-xt");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let mut ctx = Ctx::new();
    ctx.documents = Some(dir.clone());
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../kernel/tests/data/xt-asm/asm2.x_t");
    let r = call(&mut ctx, "documents_import", json!({ "files": [file], "mode": "split" }));
    let folder = dir.join("asm2.x_t");
    let (sub, top) = (folder.join("Sub.noshape"), folder.join("asm2.x_t.noshape"));
    assert_eq!(r["documents"], json!([sub.display().to_string(), top.display().to_string()]), "{r}");
    let tabs = |ctx: &mut Ctx| -> Vec<String> { call(ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| t["name"].as_str().unwrap().to_string()).collect() };
    call(&mut ctx, "document_open", json!({ "path": sub.to_str().unwrap() }));
    let mut sub_tabs = tabs(&mut ctx);
    assert_eq!(sub_tabs.remove(0), "Sub");
    sub_tabs.sort();
    assert_eq!(sub_tabs, ["Part 2", "Part 3"]);
    call(&mut ctx, "document_open", json!({ "path": top.to_str().unwrap() }));
    assert_eq!(tabs(&mut ctx), ["Assembly 1", "Part 1"]);
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    let mut names: Vec<String> = list["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    names.sort();
    assert_eq!(names, ["Part 1 <1>", "Sub <1>"], "{list}");
    assert_eq!(list["occurrences"].as_array().unwrap().len(), 3, "{list}");
    call(&mut ctx, "document_close", json!({}));
    call(&mut ctx, "document_close", json!({}));
}

/// Split into multiple documents of an IGES assembly (live 2511 (3), live's own file): its subfigure is the
/// subassembly's document ("Assembly 1": IGES names none), and the top Assembly is named after the file ("asm2"; live's
/// shows the sender's product id).
#[test]
fn documents_import_splits_an_iges_assembly() {
    let dir = scratch("docs-import-split-igs");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let mut ctx = Ctx::new();
    ctx.documents = Some(dir.clone());
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../kernel/tests/data/iges/asm2.igs");
    let r = call(&mut ctx, "documents_import", json!({ "files": [file], "mode": "split" }));
    let folder = dir.join("asm2.igs");
    let (sub, top) = (folder.join("Assembly 1.noshape"), folder.join("asm2.igs.noshape"));
    assert_eq!(r["documents"], json!([sub.display().to_string(), top.display().to_string()]), "{r}");
    let tabs = |ctx: &mut Ctx| -> Vec<String> { call(ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| t["name"].as_str().unwrap().to_string()).collect() };
    call(&mut ctx, "document_open", json!({ "path": sub.to_str().unwrap() }));
    let mut sub_tabs = tabs(&mut ctx);
    assert_eq!(sub_tabs.remove(0), "Assembly 1");
    sub_tabs.sort();
    assert_eq!(sub_tabs, ["Part 2", "Part 3"]);
    call(&mut ctx, "document_open", json!({ "path": top.to_str().unwrap() }));
    assert_eq!(tabs(&mut ctx), ["asm2", "Part 1"]);
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "asm2" }));
    let mut names: Vec<String> = list["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    names.sort();
    assert_eq!(names, ["Assembly 1 <1>", "Part 1 <1>"], "{list}");
    assert_eq!(list["occurrences"].as_array().unwrap().len(), 3, "{list}");
    call(&mut ctx, "document_close", json!({}));
    call(&mut ctx, "document_close", json!({}));
}

/// An IGES subassembly placed twice (live's asm2.igs with a second instance 50 mm along x): one document for it, used
/// twice by the top Assembly at the file's placements.
#[test]
fn documents_import_splits_an_iges_subassembly_placed_twice() {
    let dir = scratch("docs-import-split-igs-twice");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let mut ctx = Ctx::new();
    ctx.documents = Some(dir.clone());
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../kernel/tests/data/iges/asm2-twice.igs");
    let r = call(&mut ctx, "documents_import", json!({ "files": [file], "mode": "split" }));
    let folder = dir.join("asm2-twice.igs");
    let (sub, top) = (folder.join("Assembly 1.noshape"), folder.join("asm2-twice.igs.noshape"));
    assert_eq!(r["documents"], json!([sub.display().to_string(), top.display().to_string()]), "{r}");
    call(&mut ctx, "document_open", json!({ "path": top.to_str().unwrap() }));
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "asm2-twice" }));
    let instances = list["instances"].as_array().unwrap();
    let mut names: Vec<&str> = instances.iter().map(|i| i["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Assembly 1 <1>", "Assembly 1 <2>", "Part 1 <1>"], "{list}");
    let mut xs: Vec<f64> = instances.iter().filter(|i| i["name"].as_str().unwrap().starts_with("Assembly 1")).map(|i| num(&i["translation"][0])).collect();
    xs.sort_by(f64::total_cmp);
    assert!((xs[0]).abs() < 1e-6 && (xs[1] - 50.).abs() < 1e-6, "the second placement 50 mm along x: {xs:?}");
    assert_eq!(list["occurrences"].as_array().unwrap().len(), 5, "{list}");
    call(&mut ctx, "document_close", json!({}));
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

/// A CAD file tab's Export… (live document/94): the file's parts in another format; a stored file is named after the
/// file without its extension; STEP isn't offered.
#[test]
fn a_step_file_tab_exports_to_parasolid() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "file-export.noshape");
    let step = scratch("fx-block.step");
    call(&mut ctx, "export_parts", json!({ "format": "STEP", "file": step.to_str().unwrap() }));
    call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
    let xt = scratch("fx-block.x_t");
    let r = call(&mut ctx, "export_parts", json!({ "tab": "fx-block.step", "format": "PARASOLID", "file": xt.to_str().unwrap() }));
    assert_eq!(r["parts"], 1, "{r}");
    assert!(xt.is_file() && r["bytes"].as_u64().unwrap() > 1000, "{r}");
    // Read back: the block's volume.
    let v0 = num(&call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"][0]["volume"]);
    let back = call(&mut ctx, "import_files", json!({ "files": [xt.to_str().unwrap()], "keep_files": false }));
    let tab = back["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").unwrap()["id"].as_str().unwrap().to_string();
    let v1 = num(&call(&mut ctx, "query_parts", json!({ "tab": tab }))["parts"][0]["volume"]);
    assert!((v1 - v0).abs() < 1.0, "exported volume {v1} vs {v0}");
    // (OBJ, a .zip as live writes it.)
    let r = call(&mut ctx, "export_parts", json!({ "tab": "fx-block.step", "format": "OBJ", "options": "store" }));
    assert!(r["stored_tab"].is_string(), "{r}");
    let names: Vec<String> = call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"fx-block.zip".to_string()), "{names:?}");
    let e = try_call(&mut ctx, "export_parts", json!({ "tab": "fx-block.step", "format": "STEP", "file": step.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("format"));
}

/// An IGES round trip keeps the part's name and colour (qa
/// IGES-IMPORT-NAME-SOLID: it came back as "SOLID", uncoloured).
#[test]
fn import_iges_round_trip_keeps_names_and_colours() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "import-igs.noshape");
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "part_set_appearance", json!({ "parts": [part], "color": "#e51919" }));
    let igs = scratch("block.igs");
    call(&mut ctx, "export_parts", json!({ "format": "IGES", "file": igs.to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [igs.to_str().unwrap()] }));
    assert_eq!(r["parts"].as_array().unwrap().len(), 1, "{r}");
    let back = &call(&mut ctx, "query_parts", json!({ "tab": "block" }))["parts"][0];
    assert_eq!(back["name"], "Part 1", "{back}");
    assert_eq!(back["color"], "#e51919", "{back}");
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
    // The row's menu (live 2292): "Face appearance 1" renamed, suppressed (kept with its colour), unsuppressed.
    assert_eq!(a["appearances"][0]["name"], "Face appearance 1", "{a}");
    let r = call(&mut ctx, "part_appearance_row", json!({ "index": 0, "name": "Red top", "suppressed": true }));
    assert_eq!((&r["appearances"][0]["name"], &r["appearances"][0]["suppressed"]), (&json!("Red top"), &json!(true)), "{r}");
    let e = try_call(&mut ctx, "part_appearance_row", json!({ "index": 3, "suppressed": false })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("index"), "{e:?}");
    let a = call(&mut ctx, "part_appearance_row", json!({ "index": 0, "suppressed": false }));
    assert!(a["appearances"][0]["name"] == "Red top" && a["appearances"][0].get("suppressed").is_none() && a["appearances"][0]["color"] == "#ff0000", "{a}");
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
    let e = try_call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 9" })).expect_err("no Part 9");
    assert!(e.message.starts_with("no part `Part 9` in the source Part Studio; its parts: "), "{}", e.to_json());
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
    let copy_id = copy.to_string();
    let at = |id: &str| list["instances"].as_array().unwrap().iter().find(|i| i["id"] == id).unwrap()["translation"].clone();
    assert!(((num(&at(copy)[0]) - num(&at(&pin_i)[0])).abs() - 60.0).abs() < 1e-6, "{list}");
    assert_eq!(call(&mut ctx, "assembly_interference", json!({}))["count"], 1);

    // Edit: three pins.
    let pat = call(&mut ctx, "assembly_pattern", json!({ "edit": "Linear pattern 1", "count": 3 }));
    assert_eq!(pat["instances"].as_array().unwrap().len(), 4, "{pat}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Edit : Linear pattern 1");
    // A second direction (centered: both ways) and a third (live 2204): 3 × 3 × 2 pins.
    let edge_along = |k: usize| edges["edges"].as_array().unwrap().iter().find(|e| e["curve"] == "line" && num(&e["direction"][k]).abs() > 0.99).unwrap()["id"].clone();
    let (y_edge, z_edge) = (edge_along(1), edge_along(2));
    let pat = call(&mut ctx, "assembly_pattern", json!({ "edit": "Linear pattern 1", "second": { "direction": { "instance": block_i, "entity": y_edge }, "distance": "50 mm", "count": 2, "centered": true }, "third": { "direction": { "instance": block_i, "entity": z_edge }, "distance": "40 mm", "count": 2 } }));
    assert_eq!(pat["instances"].as_array().unwrap().len(), 1 + 3 * 3 * 2, "{pat}");
    let e = try_call(&mut ctx, "assembly_pattern", json!({ "edit": "Linear pattern 1", "second": false })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("third"), "{e:?}");
    let pat = call(&mut ctx, "assembly_pattern", json!({ "edit": "Linear pattern 1", "second": false, "third": false }));
    assert_eq!(pat["instances"].as_array().unwrap().len(), 4, "{pat}");

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

    // A pattern's copy stands for its pattern in a group (live 1681): the
    // copy alone is enough.
    let g = call(&mut ctx, "assembly_group", json!({ "instances": [copy_id] }));
    assert_eq!(g["name"], "Group 1", "{g}");

    // Deleting the pattern takes its instances with it, and the group it
    // was all of.
    let d = call(&mut ctx, "assembly_delete", json!({ "mates": ["Linear pattern 1"] }));
    assert_eq!(d["instances"].as_array().unwrap().len(), 2, "{d}");
    let e = try_call(&mut ctx, "assembly_group", json!({ "instances": [pin_i, block_i], "edit": "Group 1" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("edit"), "{e:?}");
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
    let e = try_call(&mut ctx, "assembly_group", json!({ "instances": [] })).unwrap_err();
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
    // Drag Revolute 2 above Revolute 1 in the list.
    let r = call(&mut ctx, "assembly_reorder", json!({ "feature": "Revolute 2", "before": "Revolute 1" }));
    let order: Vec<&str> = r["mates"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).filter(|n| n.starts_with("Revolute")).collect();
    assert_eq!(order, ["Revolute 2", "Revolute 1"], "{r}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Reorder");
    let r = call(&mut ctx, "assembly_relation", json!({ "type": "gear", "mates": ["Revolute 1", "Revolute 2"], "ratio": 2 }));
    assert_eq!(r["name"], "Gear 1", "{r}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Insert : Gear relation");
    // Listed with its mates and state (qa API-ASM-RELATIONS).
    let g = &call(&mut ctx, "assembly_list", json!({}))["relations"][0];
    assert_eq!((g["name"].clone(), g["type"].clone(), g["ratio"].clone(), g["ok"].clone()), (json!("Gear 1"), json!("gear"), json!("2"), json!(true)), "{g}");
    assert_eq!(g["mates"], json!([m1["mate"], m2["mate"]]), "{g} {m1}");
    let e = try_call(&mut ctx, "assembly_relation", json!({ "type": "gear", "mates": ["Revolute 1", "Revolute 1"] })).unwrap_err();
    assert!(e.message.contains("two different mates"), "{e:?}");
    // A ratio that doesn't evaluate is refused, naming the field (it used to become 1 silently).
    let e = try_call(&mut ctx, "assembly_relation", json!({ "edit": "Gear 1", "type": "gear", "mates": ["Revolute 1", "Revolute 2"], "ratio": "#nowhere" })).unwrap_err().to_json();
    assert_eq!((e["param"].as_str(), e["message"].as_str()), (Some("ratio"), Some("Enter a valid expression.")), "{e}");

    // Rename, suppress, hide.
    call(&mut ctx, "assembly_rename", json!({ "instance": p2, "name": "Pin B" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Rename : Pin B");
    call(&mut ctx, "assembly_set_feature_state", json!({ "features": ["Gear 1"], "suppressed": true }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Suppress mate");
    call(&mut ctx, "assembly_set_feature_state", json!({ "features": ["Mate connector 1"], "hidden": true }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Hide mate connector");
    // A mate's Hide / Show (its row menu's), as the app labels it.
    call(&mut ctx, "assembly_set_feature_state", json!({ "features": ["Revolute 1"], "hidden": false }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Show");
    let shown = call(&mut ctx, "assembly_list", json!({}));
    assert_eq!(shown["mates"].as_array().unwrap().iter().find(|m| m["name"] == "Revolute 1").map(|m| m["hidden"].clone()), Some(json!(false)), "{shown}");

    // Replace the second block with a pin.
    let rp = call(&mut ctx, "assembly_replace", json!({ "instance": b2, "source": "Part Studio 1", "part": pin }));
    assert!(rp["message"].as_str().unwrap().starts_with("Replaced 1 instance(s)"), "{rp}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Replace instance : Part 2");
    // Several instances at once, with a sketch (the dialog's Instances to replace; Part Studio sketches).
    let rp = call(&mut ctx, "assembly_replace", json!({ "instances": [p1, p2], "source": "Part Studio 1", "sketch": "Pin" }));
    assert!(rp["message"].as_str().unwrap().starts_with("Replaced 2 instance(s)"), "{rp}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Replace instance : Pin");
    let sketches = rp["instances"].as_array().unwrap().iter().filter(|i| i["id"] == p1 || i["id"] == p2).filter(|i| i["source"]["sketch"].is_string()).count();
    assert_eq!(sketches, 2, "{rp}");
    let e = try_call(&mut ctx, "assembly_replace", json!({ "source": "Part Studio 1" })).unwrap_err();
    assert!(e.message.contains("instance"), "{e:?}");
    // Standard content (the dialog's Standard content tab).
    let rp = call(&mut ctx, "assembly_replace", json!({ "instance": p1, "component": "Hex socket head cap screw ISO 4762", "size": "M6" }));
    let src = rp["instances"].as_array().unwrap().iter().find(|i| i["id"] == p1).unwrap()["source"].clone();
    assert!(src["standard_content"].as_str().is_some_and(|n| n.contains("M6")), "{src}");
    assert!(last(&mut ctx).starts_with("Assembly 1 :: Replace instance : "), "{}", last(&mut ctx));
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
    // A studio importing another tab: its notices resolve the import, as
    // the editor's (the corpus's studios import each other by element).
    let lib = call(&mut ctx, "tab_create", json!({ "kind": "featurestudio", "name": "Lib" }))["tab"]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "featurestudio_edit", json!({ "tab": lib, "source": "FeatureScript 3095;\nexport const LIB_WIDTH = 7;\n" }));
    call(&mut ctx, "featurestudio_commit", json!({ "tab": lib }));
    let user = call(&mut ctx, "tab_create", json!({ "kind": "featurestudio", "name": "User" }))["tab"]["id"].as_str().unwrap().to_string();
    let src = format!("FeatureScript 3095;\nimport(path : \"{lib}\", version : \"\");\nexport const W = LIB_WIDTH + 1;\n");
    let r = call(&mut ctx, "featurestudio_edit", json!({ "tab": user, "source": src }));
    assert!(r["notices"].as_array().unwrap().iter().all(|n| n["severity"] != "error"), "{r}");
}

/// Frame's profile library (live D134) through the agent API: the standards and profiles, a profile's sizes and value,
/// and a Frame built with it; a wrong size names the parameter.
#[test]
fn a_frame_with_a_library_profile() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "frame-library.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[0, 0], [200, 0]] }));
    let line = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "line").unwrap_or_else(|| panic!("{r}"))["id"].clone();
    let l = call(&mut ctx, "feature_frame_profiles", json!({}));
    let iso = l["standards"].as_array().unwrap().iter().find(|s| s["standard"] == "ISO").unwrap_or_else(|| panic!("{l}"));
    assert!(iso["profiles"].as_array().unwrap().iter().any(|p| p == "Rectangular tube"), "{l}");
    let p = call(&mut ctx, "feature_frame_profiles", json!({ "standard": "ISO", "profile": "Rectangular tube", "size": "30mm x 30mm x 2mm" }));
    assert_eq!(p["default"], "20mm x 20mm x 2mm", "{p}");
    let e = try_call(&mut ctx, "feature_frame_profiles", json!({ "standard": "ISO", "profile": "Rectangular tube", "size": "31mm" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("size"), "{e:?}");
    let f = call(&mut ctx, "feature_add", json!({ "type": "frame", "params": { "profileSketch": p["profileSketch"], "selections": [line] } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    let v = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    assert!((v - (900.0 - 676.0) * 200.0).abs() < 1e-3, "volume {v}");
}

/// Frame › Select sketch › Current document (FRAME-PROFILE-OTHER-STUDIO, live D134): the profile is a sketch of
/// another Part Studio, `<tab>#<sketch>.<index>` as the panel writes it; it follows that sketch when it changes.
#[test]
fn a_frame_with_another_part_studios_sketch() {
    let mut ctx = Ctx::new();
    let profile = new_sketch(&mut ctx, "frame-other-studio.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": profile, "tool": "center_rectangle", "points": [[0, 0], [10, 5]] }));
    assert_eq!(r["regions"].as_array().map(Vec::len), Some(1), "{r}");
    let info = call(&mut ctx, "document_info", json!({}));
    let source = info["tabs"].as_array().unwrap().iter().find(|t| t["name"] == "Part Studio 1").unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "tab_create", json!({ "kind": "Part Studio", "name": "Path" }));
    let path = call(&mut ctx, "sketch_create", json!({ "tab": "Path", "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "tab": "Path", "sketch": path, "tool": "line", "points": [[0, 0], [200, 0]] }));
    let line = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "line").unwrap_or_else(|| panic!("{r}"))["id"].clone();
    let value = format!("{source}#{profile}.{}", u32::MAX);
    let f = call(&mut ctx, "feature_add", json!({ "tab": "Path", "type": "frame", "params": { "profileSketch": value, "selections": [line] } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    let volume = |ctx: &mut Ctx| num(&call(ctx, "query_parts", json!({ "tab": "Path" }))["parts"][0]["volume"]);
    let v = volume(&mut ctx);
    assert!((v - 20.0 * 10.0 * 200.0).abs() < 1e-3, "volume {v}");
    // A sketch that is gone: "Select profile." on the field.
    let gone = format!("{source}#F1234.{}", u32::MAX);
    let e = call(&mut ctx, "feature_edit", json!({ "tab": "Path", "feature": f["feature"], "params": { "profileSketch": gone } }));
    assert_eq!(e["feature_status"]["status"], "Error", "{e}");
}

/// An assembly instance of a Frame part whose profile is another Part Studio's sketch has its geometry (the assembly
/// regenerates the studio with its profile's source, as the Part Studio does; asm-pattern-std's frame parts).
#[test]
fn an_assembly_shows_a_frame_part_with_another_studios_profile() {
    let mut ctx = Ctx::new();
    let profile = new_sketch(&mut ctx, "asm-frame-other-studio.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": profile, "tool": "center_rectangle", "points": [[0, 0], [10, 5]] }));
    let info = call(&mut ctx, "document_info", json!({}));
    let source = info["tabs"].as_array().unwrap().iter().find(|t| t["name"] == "Part Studio 1").unwrap()["id"].as_str().unwrap().to_string();
    call(&mut ctx, "tab_create", json!({ "kind": "Part Studio", "name": "Path" }));
    let path = call(&mut ctx, "sketch_create", json!({ "tab": "Path", "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "tab": "Path", "sketch": path, "tool": "line", "points": [[0, 0], [200, 0]] }));
    let line = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "line").unwrap()["id"].clone();
    let f = call(&mut ctx, "feature_add", json!({ "tab": "Path", "type": "frame", "params": { "profileSketch": format!("{source}#{profile}.{}", u32::MAX), "selections": [line] } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    let inst = call(&mut ctx, "assembly_insert", json!({ "source": "Path" }))["instance"].as_str().unwrap().to_string();
    let faces = call(&mut ctx, "assembly_entities", json!({ "instance": inst, "kind": "faces" }));
    assert!(faces["faces"].as_array().is_some_and(|a| !a.is_empty()), "the frame part has no faces in the assembly: {faces}");
}

/// A new Frame's profile is 8020 › 10 Series › 1001 (live 2216: a fresh account's default), a profile with no Size;
/// feature_frame_profiles lists 8020's series as Types.
#[test]
fn a_new_frame_is_an_8020_1001() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "frame-8020.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[0, 0], [200, 0]] }));
    let line = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "line").unwrap_or_else(|| panic!("{r}"))["id"].clone();
    let l = call(&mut ctx, "feature_frame_profiles", json!({}));
    let s8020 = &l["standards"][0];
    assert_eq!((&s8020["standard"], &s8020["types"][0]["type"], &s8020["types"][0]["profiles"][0]), (&json!("8020"), &json!("10 Series"), &json!("1001")), "{l}");
    let p = call(&mut ctx, "feature_frame_profiles", json!({ "standard": "8020", "profile": "1001" }));
    assert_eq!((&p["sizes"], &p["profileSketch"]), (&json!([]), &json!("frame-library:8020/1001")), "{p}");
    let f = call(&mut ctx, "feature_add", json!({ "type": "frame", "params": { "selections": [line] } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
    // A 1" square less one T-slot and the bore, 200 mm long.
    let v = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    let area = (1.0 - (0.256 * 0.087 + 0.584 * 0.236) - std::f64::consts::PI * 0.1025 * 0.1025) * 645.16;
    assert!((v - area * 200.0).abs() < 1e-2, "volume {v} vs {}", area * 200.0);
}

/// Add custom features (live 2213): a Feature Studio's feature at another document's version, listed by
/// feature_types with `document` and `version`, works in a document without Feature Studios.
#[test]
fn a_custom_feature_from_another_documents_version() {
    let mut ctx = Ctx::new();
    let lib = scratch("custom-feature-lib.noshape");
    call(&mut ctx, "document_create", json!({ "path": lib.to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "featurestudio", "name": "Blocks" }));
    let src = r#"FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

annotation { "Feature Type Name" : "Slab" }
export const slab = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Width" }
        isLength(definition.width, { (millimeter) : [1, 30, 1000] } as LengthBoundSpec);
    }
    {
        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(definition.width, 20 * millimeter, 10 * millimeter) });
    });
"#;
    call(&mut ctx, "featurestudio_edit", json!({ "source": src }));
    call(&mut ctx, "featurestudio_commit", json!({}));
    let e = try_call(&mut ctx, "feature_types", json!({ "document": lib.to_str().unwrap(), "version": "V1" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "custom features come only from a version: {e:?}");
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let user = scratch("custom-feature-user.noshape");
    call(&mut ctx, "document_create", json!({ "path": user.to_str().unwrap() }));
    let l = call(&mut ctx, "feature_types", json!({ "document": lib.to_str().unwrap(), "version": "V1" }));
    assert_eq!(l["feature_studios"][0]["feature_studio"], "Blocks", "{l}");
    let f = &l["feature_studios"][0]["features"][0];
    assert_eq!(f["name"], "Slab", "{l}");
    let ty = f["type"].as_str().unwrap().to_string();
    assert!(ty.starts_with("custom:ext:"), "{ty}");
    let r = call(&mut ctx, "feature_add", json!({ "tab": "Part Studio 1", "type": ty, "params": { "width": "40 mm" } }));
    assert_eq!(r["feature_status"]["status"], "Ok", "{r}");
    let v = num(&call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"][0]["volume"]);
    assert!((v - 8000.0).abs() < 1e-3, "volume {v}");
}

/// Add custom features (psui c1cb5e34, live 2361 / 2213): a version's custom features onto the user's toolbar one by
/// one or a whole Feature Studio at once, kept alphabetical, live's toast for each one added, Remove taking one off.
#[test]
fn custom_features_on_the_toolbar() {
    let mut ctx = Ctx::new();
    let lib = scratch("custom-toolbar-lib.noshape");
    call(&mut ctx, "document_create", json!({ "path": lib.to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "featurestudio", "name": "Blocks" }));
    let feature = |name: &str, var: &str| format!(r#"
annotation {{ "Feature Type Name" : "{name}" }}
export const {var} = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {{
    }}
    {{
        fCuboid(context, id + "box", {{ "corner1" : vector(0, 0, 0) * millimeter, "corner2" : vector(10, 20, 10) * millimeter }});
    }});
"#);
    let src = format!("FeatureScript 3095;\nimport(path : \"onshape/std/geometry.fs\", version : \"3095.0\");\n{}{}", feature("Slab", "slab"), feature("Beam", "beam"));
    call(&mut ctx, "featurestudio_edit", json!({ "source": src }));
    call(&mut ctx, "featurestudio_commit", json!({}));
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let user = scratch("custom-toolbar-user.noshape");
    call(&mut ctx, "document_create", json!({ "path": user.to_str().unwrap() }));
    let l = call(&mut ctx, "feature_types", json!({ "document": lib.to_str().unwrap(), "version": "V1" }));
    let ty = |name: &str| l["feature_studios"][0]["features"].as_array().unwrap().iter().find(|f| f["name"] == name).unwrap()["type"].as_str().unwrap().to_string();
    let (slab, beam) = (ty("Slab"), ty("Beam"));
    let names = |r: &Value| r["toolbar"]["custom_features"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let r = call(&mut ctx, "ui_toolbar", json!({ "add_custom_features": [slab], "document": lib.to_str().unwrap(), "version": "V1" }));
    assert_eq!(r["messages"], json!(["Custom feature \"Slab\" was added to your toolbar from version V1."]), "{r}");
    // The Feature Studio's row: all of its features (only Beam is new), alphabetical.
    let r = call(&mut ctx, "ui_toolbar", json!({ "add_custom_features": [slab, beam], "document": lib.to_str().unwrap(), "version": "V1" }));
    assert_eq!(r["messages"], json!(["Custom feature \"Beam\" was added to your toolbar from version V1."]), "{r}");
    assert_eq!(names(&r), ["Beam", "Slab"], "{r}");
    // Account-wide: read back without a document, as every Part Studio does.
    assert_eq!(names(&call(&mut ctx, "ui_toolbar", json!({}))), ["Beam", "Slab"]);
    let r = call(&mut ctx, "ui_toolbar", json!({ "remove_custom_features": [slab] }));
    assert_eq!(names(&r), ["Beam"], "{r}");
    let e = try_call(&mut ctx, "ui_toolbar", json!({ "add_custom_features": ["extrude"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("add_custom_features[0]"), "{e:?}");
    call(&mut ctx, "ui_toolbar", json!({ "remove_custom_features": [beam] }));
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
        let mut path = e["file"].as_str().unwrap().to_string();
        assert!(std::path::Path::new(&path).extension().is_some(), "{f}: the format's extension is added: {e}");
        // OBJ downloads as a zip of the .obj and its .mtl (live): import the .obj.
        if *f == "OBJ" {
            assert!(path.ends_with(".zip"), "{e}");
            let dir = scratch(&format!("import_all_{i}_obj"));
            let _ = std::fs::create_dir_all(&dir);
            let names = noshape_kernel::extract_zip(std::path::Path::new(&path), &dir).unwrap();
            assert!(names.iter().any(|n| n.ends_with(".mtl")), "{names:?}");
            path = dir.join(names.iter().find(|n| n.ends_with(".obj")).unwrap()).to_string_lossy().to_string();
        }
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

/// Export Resolution (audit X2, live 1745): STL's chordal tolerance and
/// angular deviation drive the mesh (a finer one has more triangles); the
/// presets are absolute (Medium = 0.12 mm / 6.25°, Custom's first values);
/// STL's lengths are in its units.
#[test]
fn export_custom_resolution_drives_the_mesh() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "export_custom.noshape");
    let triangles = |ctx: &mut Ctx, name: &str, args: Value| {
        let f = scratch(name);
        let mut a = json!({ "format": "STL", "file": f.to_str().unwrap(), "resolution": "Custom" });
        a.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
        call(ctx, "export_parts", a);
        let b = std::fs::read(&f).unwrap();
        u32::from_le_bytes([b[80], b[81], b[82], b[83]])
    };
    let coarse = triangles(&mut ctx, "custom_coarse.stl", json!({ "chordal_tolerance": 1.0, "angular_deviation": 30 }));
    let fine = triangles(&mut ctx, "custom_fine.stl", json!({ "chordal_tolerance": 0.005, "angular_deviation": 2 }));
    assert!(fine > coarse * 2, "{fine} vs {coarse}");
    let medium = triangles(&mut ctx, "preset_medium.stl", json!({ "resolution": "Medium" }));
    assert_eq!(triangles(&mut ctx, "custom_default.stl", json!({})), medium, "Custom starts at Medium's values");
    // (Custom's fields are in the STL's units, metres by default: 0.12 mm in millimetres.)
    assert_eq!(triangles(&mut ctx, "custom_medium.stl", json!({ "units": "Millimeter", "chordal_tolerance": 0.12, "angular_deviation": 6.25 })), medium);
    assert!(triangles(&mut ctx, "preset_fine.stl", json!({ "resolution": "Fine" })) > medium);
    // 0.12 mm in inches (and the file in inches).
    assert_eq!(triangles(&mut ctx, "custom_inch.stl", json!({ "units": "Inch", "chordal_tolerance": 0.12 / 25.4, "angular_deviation": 6.25 })), medium);
    for bad in [-1.0, 0.0] {
        let e = try_call(&mut ctx, "export_parts", json!({ "format": "STL", "file": scratch("bad.stl").to_str().unwrap(), "resolution": "Custom", "chordal_tolerance": bad })).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("chordal_tolerance"), "{e:?}");
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
    // So is an OBJ export's zip (.obj + .mtl): live's MTL message (qa OBJ-ZIP-MSG).
    let z = call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "OBJ", "file": scratch("obj_zip").to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [z["file"].as_str().unwrap()] }));
    assert!(r["errors"][0].as_str().unwrap().ends_with("was not translated. Translation is not supported for MTL (.mtl) files."), "{z} {r}");
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
    // A selection the constraint can't take: live's words (2270), nothing added.
    let e = try_call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "parallel", "entities": [lines[0]["id"], corner] })).unwrap_err();
    assert_eq!(e.message, "A parallel constraint requires two lines.", "{e:?}");
    // A dimension without a value keeps the measured one (the value box accepted unchanged).
    let d = call(&mut ctx, "sketch_dimension", json!({ "sketch": sk, "kind": "length", "entities": [lines[0]["id"]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let dim = g["constraints"].as_array().unwrap().iter().find(|c| c["id"] == d["dimension"]).unwrap().clone();
    assert_eq!(dim["driving"], true, "{dim}");
    let shown = dim["value"].as_str().unwrap();
    assert!(shown.starts_with("30") || shown.starts_with("20"), "{dim}");
}

/// A constraint to the Origin references it (live: the external Origin, no
/// sketch entity): sketch_get lists no hidden point for it, and the
/// constraint names `origin` (skb, qa5 P1-4 data note).
#[test]
fn sketch_get_names_the_origin_not_a_hidden_point() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "origin_ref.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "line", "points": [[5, 5], [30, 20]] }));
    let line = r["created"].as_array().unwrap().iter().find(|e| e["type"] == "line").unwrap().clone();
    let start = line["start_point"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "coincident", "entities": [start, "origin"] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": sk }));
    let points = g["entities"].as_array().unwrap().iter().filter(|e| e["type"] == "point").count();
    assert_eq!(points, 2, "only the line's end points: {g}");
    let ents = g["entities"].as_array().unwrap().iter().map(|e| e["id"].clone()).collect::<Vec<_>>();
    let to_origin = g["constraints"].as_array().unwrap().iter().find(|c| c["entities"].as_array().unwrap().iter().any(|e| e == "origin")).unwrap_or_else(|| panic!("no constraint names the origin: {g}"));
    for e in to_origin["entities"].as_array().unwrap() {
        assert!(e == "origin" || ents.contains(e), "{to_origin}");
    }
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
    let ins = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": block }));
    assert_eq!(ins["name"], "Part 1 <1>", "the reply names the new instance: {ins}");
    let b = ins["instance"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": pin }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_set_state", json!({ "instances": [b], "fixed": true }));
    let cyl = |ctx: &mut Ctx, i: &str| call(ctx, "assembly_entities", json!({ "instance": i, "kind": "faces", "surface": "cylinder" }))["faces"][0]["id"].as_str().unwrap().to_string();
    let (hole, pf) = (cyl(&mut ctx, &b), cyl(&mut ctx, &p));
    call(&mut ctx, "assembly_mate", json!({ "type": "revolute", "a": { "instance": p, "entity": pf }, "b": { "instance": b, "entity": hole } }));
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();

    // Animate…: the dialog's defaults (a full turn in 300 steps); a quarter
    // of the run turns the pin 90°, and nothing is saved.
    let before = last(&mut ctx);
    let start = call(&mut ctx, "assembly_animate", json!({ "frame": 0 }));
    let an = call(&mut ctx, "assembly_animate", json!({ "mate": "Revolute 1", "frame": 75 }));
    assert_eq!((an["dofs"].clone(), an["dof"].clone(), an["start"].clone(), an["end"].clone(), an["steps"].clone()), (json!(["rz"]), json!("rz"), json!("0 deg"), json!("360 deg"), json!(300)), "{an}");
    assert!((num(&an["value"]) - 90.0).abs() < 1e-9, "{an}");
    let rot = |v: &Value, i: &str| v["instances"].as_array().unwrap().iter().find(|x| x["id"] == i).unwrap()["rotation"].clone();
    assert_ne!(rot(&start, &p), rot(&an, &p), "the pin turns: {an}");
    assert_eq!(rot(&start, &b), rot(&an, &b), "the fixed block stays");
    assert_eq!(last(&mut ctx), before, "animation is not saved");
    let e = try_call(&mut ctx, "assembly_animate", json!({ "dof": "tz" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("dof"), "{e:?}");
    let e = try_call(&mut ctx, "assembly_animate", json!({ "steps": 10, "frame": 11 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("frame"), "{e:?}");
    // The camera: Zoom to selection on the pin; View normal to a block face.
    let z = call(&mut ctx, "view_camera", json!({ "tab": "Assembly 1", "action": "zoom_selection", "refs": [p] }));
    assert_eq!(z["previous_views"], 1, "{z}");
    let face = call(&mut ctx, "assembly_entities", json!({ "instance": b, "kind": "faces", "surface": "plane" }))["faces"][0]["id"].as_str().unwrap().to_string();
    let n = call(&mut ctx, "view_camera", json!({ "tab": "Assembly 1", "action": "normal_to", "refs": [{ "instance": b, "entity": face }] }));
    let d = &n["view_direction"];
    assert!((0..3).any(|i| num(&d[i]).abs() > 1.0 - 1e-9), "normal to an axis-aligned face: {n}");
    // Display: isolate the pin, show the mates, snap mode.
    let dv = call(&mut ctx, "view_display", json!({ "tab": "Assembly 1", "isolate": [p], "show_mates": true, "snap_mode": true }));
    assert_eq!((dv["isolate"][0].clone(), dv["show_mates"].clone(), dv["snap_mode"].clone()), (json!(p), json!(true), json!(true)), "{dv}");
    let e = try_call(&mut ctx, "view_display", json!({ "tab": "Assembly 1", "highlight_boundary": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("highlight_boundary"), "{e:?}");
    // Selection: Select instances with same configuration; Clear selection.
    let s = call(&mut ctx, "ui_select", json!({ "tab": "Assembly 1", "action": "same_configuration", "refs": [p] }));
    assert_eq!(s["selected"], json!([b, p]), "both from Part Studio 1 in its default configuration: {s}");
    let s = call(&mut ctx, "ui_select", json!({ "tab": "Assembly 1", "action": "same_configuration", "refs": [p], "same_part": true }));
    assert_eq!(s["selected"], json!([p]), "{s}");
    let s = call(&mut ctx, "ui_select", json!({ "tab": "Assembly 1", "action": "add", "refs": [b] }));
    assert_eq!(s["selected"], json!([p, b]), "{s}");
    let s = call(&mut ctx, "ui_select", json!({ "tab": "Assembly 1", "action": "clear" }));
    assert_eq!(s["selected"], json!([]), "{s}");
    // Select other… through the window's camera: the pin under the view's
    // centre after Zoom to selection on it.
    call(&mut ctx, "view_camera", json!({ "tab": "Assembly 1", "action": "zoom_selection", "refs": [p] }));
    let o = call(&mut ctx, "ui_select", json!({ "tab": "Assembly 1", "action": "select_other", "at": [640, 400], "index": 0 }));
    assert!(o["under"].as_array().unwrap().contains(&json!(p)), "{o}");
    // Search tools in the assembly (live dom/assembly-tool-search.txt).
    let t = call(&mut ctx, "ui_search_tools", json!({ "tab": "Assembly 1", "text": "gear" }));
    assert_eq!(t["tools"], json!([{ "name": "Gear relation", "icon": "gear-relation", "keys": null }]), "{t}");
    // The lists: a mate row's Expand, Find in instance list, Expand all.
    let l = call(&mut ctx, "ui_list", json!({ "tab": "Assembly 1", "action": "expand", "items": ["Revolute 1"] }));
    assert_eq!(l["expanded_mates"], json!(["Revolute 1"]), "{l}");
    let e = try_call(&mut ctx, "ui_list", json!({ "tab": "Assembly 1", "action": "expand", "items": [b] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("items[0]"), "a part instance has no rows under it: {e:?}");
    let l = call(&mut ctx, "ui_list", json!({ "tab": "Assembly 1", "action": "find", "items": [p] }));
    assert_eq!(l["selected"], json!([p]), "{l}");
    let l = call(&mut ctx, "ui_list", json!({ "tab": "Assembly 1", "action": "collapse_all" }));
    assert_eq!(l["expanded_mates"], json!([]), "{l}");

    let e = try_call(&mut ctx, "assembly_named_position_drivers", json!({ "drivers": [{ "mate": "Revolute 1", "dof": "tz" }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("drivers[0].dof"));
    call(&mut ctx, "assembly_named_position_drivers", json!({ "drivers": [{ "mate": "Revolute 1", "dof": "rz" }] }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Driving mate selector");
    let np = call(&mut ctx, "assembly_named_position", json!({ "action": "add" }));
    assert_eq!(np["named_positions"][0]["name"], "Named position 1", "{np}");
    call(&mut ctx, "assembly_named_position", json!({ "action": "rename", "position": "1", "name": "Closed" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Rename named position : Closed");
    call(&mut ctx, "assembly_named_position", json!({ "action": "duplicate", "position": "Closed" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Duplicate named position : Closed Copy 1");
    let np = call(&mut ctx, "assembly_named_position", json!({ "action": "set_value", "position": "Closed Copy 1", "mate": "Revolute 1", "dof": "rz", "value": "90 deg" }));
    let v = np["named_positions"].as_array().unwrap().iter().find(|p| p["name"] == "Closed Copy 1").unwrap()["mate_values"][0]["rz_deg"].clone();
    assert!((num(&v) - 90.0).abs() < 1e-6, "{np}");
    call(&mut ctx, "assembly_named_position", json!({ "action": "apply", "position": "Closed Copy 1" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Apply named position : Closed Copy 1");
    call(&mut ctx, "assembly_named_position", json!({ "action": "move_up", "position": "Closed Copy 1" }));
    let np = call(&mut ctx, "assembly_named_position", json!({ "action": "delete", "position": "Closed" }));
    assert_eq!(np["named_positions"].as_array().unwrap().len(), 1, "{np}");

    // Exploded view: lift the pin 30 mm, then rename and delete.
    let ev = call(&mut ctx, "assembly_exploded_view", json!({ "action": "add" }));
    assert_eq!(ev["exploded_views"][0]["name"], "Explode 1", "{ev}");
    let st = call(&mut ctx, "assembly_explode_step", json!({ "view": "Explode 1", "instances": [p], "direction": [0, 0, 1], "distance": "30 mm" }));
    assert_eq!(st["steps"], 1, "{st}");
    // The reply says where the step puts the pin.
    assert!((num(&st["exploded"][0]["offset"][2]) - 30.0).abs() < 1e-6 && st["exploded"].as_array().unwrap().len() == 1, "{st}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Edit : Explode step");
    // Its explode lines start from a face of the pin (the dialog's "Edges, vertices, or faces"); an edit keeps them.
    let face = call(&mut ctx, "assembly_entities", json!({ "instance": p, "kind": "faces" }))["faces"][0]["id"].clone();
    call(&mut ctx, "assembly_explode_step", json!({ "view": "Explode 1", "step": 1, "instances": [p], "direction": [0, 0, 1], "distance": "30 mm", "line_entities": [{ "instance": p, "entity": face }] }));
    call(&mut ctx, "assembly_explode_step", json!({ "view": "Explode 1", "step": 1, "instances": [p], "direction": [0, 0, 1], "distance": "35 mm" }));
    let views = call(&mut ctx, "assembly_list", json!({}))["exploded_views"].clone();
    assert_eq!(views[0]["step_line_entities"], json!([1]), "{views}");
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "rename", "view": "Explode 1", "name": "Pin out" }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Rename : Pin out");
    // The view's menu: Hide all explode lines, Show all explode lines.
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "lines", "view": "Pin out", "on": false }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Hide all explode lines");
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "lines", "view": "Pin out", "on": true }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Show all explode lines");
    // Duplicate: a copy with its steps after it (live asm-explode-duplicate).
    let ev = call(&mut ctx, "assembly_exploded_view", json!({ "action": "duplicate", "view": "Pin out" }));
    let views = ev["exploded_views"].as_array().unwrap();
    assert_eq!(views.iter().map(|v| v["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Pin out", "Pin out Copy 1"], "{ev}");
    assert_eq!(views[0]["steps"], views[1]["steps"], "{ev}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Duplicate : Pin out Copy 1");
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "delete", "view": "Pin out Copy 1" }));
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
    // Section depth Up to entity with an Offset (live request 1737): only
    // with depth_to; a limit 2 mm past the hole's centre makes a valid view.
    let e = try_call(&mut ctx, "drawing_section_view", json!({ "drawing": tab, "parent": top_id, "point": [cx, cy], "position": [max_x + 60.0, cy - 80.0], "depth_offset": 2 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("depth_offset"), "{e:?}");
    let s2 = call(&mut ctx, "drawing_section_view", json!({ "drawing": tab, "parent": top_id, "point": [cx, cy], "position": [max_x + 60.0, cy - 80.0], "depth_to": [cx, cy], "depth_offset": 2 }));
    let sv2 = views_of(&mut ctx).into_iter().find(|v| v["id"] == s2["view"]).unwrap();
    assert!(sv2["error"].is_null(), "{sv2}");
    call(&mut ctx, "history_undo", json!({}));
    assert_eq!(sv["parent"], top_id, "{sv}");
    assert!(last(&mut ctx).ends_with(":: Section view"));
    // Edit hatch section...: one region restyled, then back (live asb 10-23 views (4)).
    assert!(sv["hatch_regions"].as_u64().is_some_and(|n| n >= 1), "{sv}");
    let corner = [num(&sv["bounds"]["min"][0]) + 0.5, num(&sv["bounds"]["min"][1]) + 0.5];
    call(&mut ctx, "drawing_edit_hatch", json!({ "drawing": tab, "view": sid, "at": corner, "pattern": "ANSI32 steel", "angle": 90, "color": "#ff0000" }));
    assert!(last(&mut ctx).ends_with(":: Edit hatch"));
    let sv = views_of(&mut ctx).into_iter().find(|v| v["id"] == sid).unwrap();
    assert_eq!(sv["section_hatches"][0]["pattern"], "ANSI32 steel", "{sv}");
    call(&mut ctx, "drawing_edit_hatch", json!({ "drawing": tab, "view": sid, "at": corner, "standard": "general", "pattern": "None" }));
    let sv = views_of(&mut ctx).into_iter().find(|v| v["id"] == sid).unwrap();
    assert_eq!(sv["section_hatches"].as_array().map(|a| a.len()), Some(1), "the same region: replaced {sv}");
    assert_eq!(sv["section_hatches"][0]["pattern"], "None");
    call(&mut ctx, "drawing_edit_hatch", json!({ "drawing": tab, "view": sid, "at": corner, "reset": true }));
    // Show/hide ▸ Show offset cut lines (section views, live (6)).
    call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": sid, "offset_cut_lines": true }));
    assert!(last(&mut ctx).ends_with(":: Offset cut lines"));
    assert_eq!(views_of(&mut ctx).into_iter().find(|v| v["id"] == sid).unwrap()["offset_cut_lines"], true);
    let e = try_call(&mut ctx, "drawing_view_display", json!({ "drawing": tab, "view": top_id, "offset_cut_lines": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("offset_cut_lines"), "{e:?}");
    assert!(views_of(&mut ctx).into_iter().find(|v| v["id"] == sid).unwrap()["section_hatches"].is_null());
    let e = try_call(&mut ctx, "drawing_edit_hatch", json!({ "drawing": tab, "view": sid, "at": [5, 5] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("at"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_edit_hatch", json!({ "drawing": tab, "view": sid, "at": corner, "pattern": "Plaid" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("pattern"), "{e:?}");

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
    let ec = call(&mut ctx, "drawing_centerline", json!({ "drawing": tab, "kind": "edge_to_edge", "picks": [mid(h0), mid(h1)] }));
    assert!(last(&mut ctx).ends_with(":: Edge to edge centerline"));
    // Its end grip shortens it (to the middle: half as long), its middle
    // snap moves it (help A51); a 2 point one's ends stay on their points.
    // Drawn 2.5 mm past each end (ISO, live 2230): the bounds include that.
    let ext = 2.5;
    let eid = ec["annotation"].as_u64().unwrap();
    let span = |ctx: &mut Ctx| {
        let g = call(ctx, "drawing_get", json!({ "drawing": tab }));
        let b = g["annotations"].as_array().unwrap().iter().find(|a| a["id"] == eid).unwrap()["bounds"].clone();
        ([num(&b["min"][0]), num(&b["min"][1])], [num(&b["max"][0]), num(&b["max"][1])])
    };
    let (lo, hi) = span(&mut ctx);
    let (c, l0) = ([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0], (hi[0] - lo[0]).hypot(hi[1] - lo[1]) - 2.0 * ext);
    call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": eid, "action": "centerline_end", "end": 1, "point": c }));
    assert!(last(&mut ctx).ends_with(":: Resize centerline"));
    let (lo, hi) = span(&mut ctx);
    assert!(((hi[0] - lo[0]).hypot(hi[1] - lo[1]) - (l0 / 2.0 + 2.0 * ext)).abs() < 1e-6, "{lo:?} {hi:?} {l0}");
    let m = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": eid, "action": "move_centerline", "point": [m[0] + 3.0, m[1] + 4.0] }));
    let (lo2, hi2) = span(&mut ctx);
    assert!((lo2[0] - lo[0] - 3.0).abs() < 1e-6 && (hi2[1] - hi[1] - 4.0).abs() < 1e-6, "{lo2:?} {hi2:?}");
    let e = try_call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": c2["annotation"], "action": "centerline_end", "end": 0, "point": c })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("end"), "{e:?}");
    // Snaps optional (help A50): a 2 point centerline from a spot inside the
    // part to 6 mm right of it; its first end is a snap point for the next.
    let (m0, m1) = (mid(h0), mid(h1));
    let f0 = [(m0[0] + m1[0]) / 2.0 + 1.3, (m0[1] + m1[1]) / 2.0 + 0.7];
    let f1 = [f0[0] + 6.0, f0[1]];
    let fc = call(&mut ctx, "drawing_centerline", json!({ "drawing": tab, "kind": "two_point", "picks": [f0, f1] }));
    let fid = fc["annotation"].as_u64().unwrap();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let b = g["annotations"].as_array().unwrap().iter().find(|a| a["id"] == fid).unwrap()["bounds"].clone();
    assert!((num(&b["min"][0]) - (f0[0] - ext)).abs() < 1e-6 && (num(&b["max"][0]) - (f1[0] + ext)).abs() < 1e-6 && (num(&b["min"][1]) - f0[1]).abs() < 1e-6, "{b} {f0:?} {f1:?}");
    let fc2 = call(&mut ctx, "drawing_centerline", json!({ "drawing": tab, "kind": "two_point", "picks": [[f0[0] + 0.2, f0[1] - 0.15], [f0[0], f0[1] + 9.0]] }));
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let b = g["annotations"].as_array().unwrap().iter().find(|a| a["id"] == fc2["annotation"]).unwrap()["bounds"].clone();
    assert!((num(&b["min"][0]) - f0[0]).abs() < 1e-6 && (num(&b["min"][1]) - (f0[1] - ext)).abs() < 1e-6, "snapped to the first one's end: {b}");
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
    // Further views: another view of the same part only (live request 1744).
    let e = try_call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "hole", "view": top_id, "origin": [cx, cy], "more_views": [{ "view": top_id, "origin": [cx, cy] }], "position": [20, 200] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("more_views[0]"), "{e:?}");
}

/// sketch_draw's inference takes a line to horizontal / vertical only when it is (c2-hunting-knife: the screen's 3°
/// snapped a 2.2° blade edge flat and constrained it).
#[test]
fn sketch_draw_keeps_a_nearly_horizontal_line_as_drawn() {
    let mut ctx = Ctx::new();
    let s = new_sketch(&mut ctx, "near_horizontal.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": s, "tool": "line", "points": [[71.330304287, -7.739502841], [0, -10.5]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": s, "tool": "line", "points": [[0, 20], [30, 20]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": s }));
    let lines: Vec<&Value> = g["entities"].as_array().unwrap().iter().filter(|e| e["type"] == "line").collect();
    assert_eq!(lines[0]["end"], json!([0.0, -10.5]), "{g}");
    let horizontal: Vec<&Value> = g["constraints"].as_array().unwrap().iter().filter(|c| c["type"] == "Horizontal").collect();
    assert_eq!(horizontal.len(), 1, "only the exactly horizontal line: {g}");
    assert_eq!(horizontal[0]["entities"], json!([lines[1]["id"]]), "{g}");
}

/// sketch_draw's bezier with `degree` / `knots`: a stored B-spline (Onshape's from a Use or an offset) on any number of
/// control points, past the tool's 16 clicks; knots that don't fit are named.
#[test]
fn sketch_draw_makes_a_stored_b_spline() {
    let mut ctx = Ctx::new();
    let s = new_sketch(&mut ctx, "bspline.noshape", "plane:top");
    let poles: Vec<Value> = (0..20).map(|i| json!([i as f64 * 5.0, if i % 2 == 0 { 0.0 } else { 4.0 }])).collect();
    // Clamped cubic, 20 poles: 24 knots, 17 spans.
    let mut knots = vec![0.0; 4];
    knots.extend((1..17).map(|i| i as f64 / 17.0));
    knots.extend([1.0; 4]);
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": s, "tool": "bezier", "points": poles, "degree": 3, "knots": knots }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": r["sketch"] }));
    let b = g["entities"].as_array().unwrap().iter().find(|e| e["type"] == "bspline").unwrap_or_else(|| panic!("{g}"));
    assert_eq!((b["degree"].clone(), b["control_points"].as_array().map(Vec::len), b["knots"].as_array().map(Vec::len)), (json!(3), Some(20), Some(24)), "{b}");
    assert_eq!(b["start_point"], b["point_ids"][0], "{b}");
    let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": s, "tool": "bezier", "points": [[0, 10], [5, 14], [10, 10], [15, 14]], "degree": 3, "knots": [0, 0, 0, 1, 1, 1] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("knots"), "{e:?}");
    let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": s, "tool": "line", "points": [[0, 20], [5, 20]], "degree": 1, "knots": [0, 0, 1, 1] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("knots"), "{e:?}");
}

/// Vertical takes an elliptical arc's major axis, and Coincident makes two elliptical arcs one ellipse (Onshape: a
/// trimmed ellipse's pieces; c2-rings-loft).
#[test]
fn elliptical_arcs_take_vertical_and_coincident() {
    let mut ctx = Ctx::new();
    let s = new_sketch(&mut ctx, "elliptical.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": s, "shapes": [
        { "tool": "elliptical_arc", "points": [[0, 0], [2, 10], [8, 0], [0, -10.2]] },
        { "tool": "elliptical_arc", "points": [[30, 0], [30, 12], [36, 0], [30, -12]] }
    ] }));
    let arcs: Vec<String> = r["created"].as_array().unwrap().iter().filter(|e| e["type"] == "elliptical_arc").map(|e| e["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(arcs.len(), 2, "{r}");
    call(&mut ctx, "sketch_constrain", json!({ "sketch": s, "constraint": "vertical", "entities": [arcs[0]] }));
    call(&mut ctx, "sketch_constrain", json!({ "sketch": s, "constraint": "coincident", "entities": [arcs[0], arcs[1]] }));
    let g = call(&mut ctx, "sketch_get", json!({ "sketch": s }));
    let e: Vec<&Value> = g["entities"].as_array().unwrap().iter().filter(|e| e["type"] == "elliptical_arc").collect();
    let num = |v: &Value| v.as_f64().unwrap();
    assert!(num(&e[0]["major"][0]).abs() < 1e-6, "vertical: {}", e[0]);
    for k in ["center", "major"] {
        for i in 0..2 {
            assert!((num(&e[0][k][i]) - num(&e[1][k][i])).abs() < 1e-6, "one ellipse: {} / {}", e[0], e[1]);
        }
    }
    assert!((num(&e[0]["minor_radius"]) - num(&e[1]["minor_radius"])).abs() < 1e-6, "{} / {}", e[0], e[1]);
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
    // `name` alone names the variable to change; `new_name` renames.
    let r = call(&mut ctx, "variable_set", json!({ "tab": "vs all", "name": "#n", "value": 4 }));
    assert!(r["variables"].as_array().unwrap().iter().any(|v| v["name"] == "#n" && v["expression"] == "4"), "{r}");
    call(&mut ctx, "variable_set", json!({ "tab": "vs all", "variable": "n", "new_name": "m" }));
    call(&mut ctx, "variable_set", json!({ "tab": "vs all", "variable": "m", "new_name": "n" }));
    let e = try_call(&mut ctx, "variable_set", json!({ "tab": "vs all", "value": 1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("variable"), "{e:?}");
    let r = call(&mut ctx, "variable_move", json!({ "tab": "vs all", "variable": "n", "by": -1 }));
    assert_eq!(r["variables"][1]["name"], "#n", "{r}");
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Move variable up"));
    let r = call(&mut ctx, "variable_delete", json!({ "tab": "vs all", "variable": "n" }));
    assert_eq!(r["variables"].as_array().unwrap().len(), 2, "{r}");
    // Errors name the parameter.
    let e = try_call(&mut ctx, "variable_create", json!({ "tab": "vs all", "name": "2x", "value": 1 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"), "{e:?}");
    // A taken name becomes `<name>_0`, with live's toast.
    let r = call(&mut ctx, "variable_create", json!({ "tab": "vs all", "name": "half", "value": 1 }));
    assert_eq!((r["renamed"].as_str(), r["message"].as_str()), (Some("half_0"), Some("Renamed some new variables to avoid name conflicts.")), "{r}");
    call(&mut ctx, "variable_delete", json!({ "tab": "vs all", "variable": "half_0" }));
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
    // A hole table of that view (live 1983): TAG, X / Y LOC, SIZE, QTY; an
    // extruded cut is no Hole feature's hole, so live lists none (behaviours
    // "no Hole features": the header only).
    let ht = call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "hole", "view": top["id"], "origin": p(&lines[0], "a"), "position": [20, 120] }));
    let hid = ht["table"].clone();
    let tables = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["tables"].clone();
    let t = tables.as_array().unwrap().iter().find(|t| t["id"] == hid).cloned().unwrap();
    assert_eq!(t["header"], json!(["TAG", "X LOC", "Y LOC", "SIZE", "QTY"]), "{t}");
    assert_eq!(t["rows"], json!([]), "{t}");

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
    // The base clicked just inside an edge, within its highlight (5 px at
    // the API zoom; qa4 R5v3-S11-a): measured from
    // the edge, not the click: the hole centre at 40, the far edge at 80.
    let verticals: Vec<Value> = top["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line" && (num(&e["a"][0]) - num(&e["b"][0])).abs() < 1e-6).cloned().collect();
    let left = verticals.iter().min_by(|x, y| num(&x["a"][0]).total_cmp(&num(&y["a"][0]))).unwrap();
    let right = verticals.iter().max_by(|x, y| num(&x["a"][0]).total_cmp(&num(&y["a"][0]))).unwrap();
    let (lx, ly) = (num(&left["a"][0]), (num(&left["a"][1]) + num(&left["b"][1])) / 2.0);
    let base = [lx + 0.2, ly];
    let o2 = call(&mut ctx, "drawing_ordinate", json!({ "drawing": tab, "origin": base, "side": [base[0], base[1] + 40.0], "points": [[cx, cy], p(right, "a")] }));
    let text = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["annotations"].as_array().unwrap().iter().find(|a| a["id"] == o2["annotation"]).unwrap()["text"].as_str().unwrap().to_string();
    let values: Vec<f64> = text.split_whitespace().filter_map(|w| w.replace(',', ".").parse().ok()).collect();
    for want in [40.0, 80.0] {
        assert!(values.iter().any(|v| (v - want).abs() < 1e-9), "{want} in {text:?}");
    }

    // Max/min: the hole's far side to the edge's end.
    let mm = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "max_min", "picks": [[cx + r, cy], a0], "place": [cx, cy - 25.0] }));
    assert_eq!(kind_of(&mut ctx, mm["annotation"].as_u64().unwrap()), "MaxMinDimension");
    assert!(last(&mut ctx).ends_with(":: Maximum / minimum dimension"), "{}", last(&mut ctx));

    let cm = call(&mut ctx, "drawing_centermark", json!({ "drawing": tab, "holes": [[cx - r, cy]] }));
    assert_eq!(kind_of(&mut ctx, cm["annotations"][0].as_u64().unwrap()), "Centermark");
    assert!(last(&mut ctx).ends_with(":: Centermark"));
    // Radial on the centermark (its arm inside the hole, away from the rim):
    // the circle it marks (help D13).
    let rd = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "radius", "picks": [[cx - r + 0.6 * r, cy]], "place": [cx - r - 15.0, cy + 15.0] }));
    assert_eq!(kind_of(&mut ctx, rd["annotation"].as_u64().unwrap()), "RadialDimension");

    let nt = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "BREAK ALL EDGES", "position": [40, 40], "leader": a0 }));
    assert_eq!(kind_of(&mut ctx, nt["annotation"].as_u64().unwrap()), "Note");
    assert!(last(&mut ctx).ends_with(":: Note"));
    // A leader to a point along an edge (not a snap point), as a click on
    // the edge (live "Note leader"; audit A1).
    let along = [a0[0] + (b0[0] - a0[0]) / 3.0, a0[1] + (b0[1] - a0[1]) / 3.0];
    let ne = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "ON EDGE", "position": [40, 50], "leader": along }));
    assert_eq!(kind_of(&mut ctx, ne["annotation"].as_u64().unwrap()), "Note");
    assert!(try_call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "AIR", "position": [40, 60], "leader": [5.0, 5.0] })).is_err(), "nothing there");

    let g = call(&mut ctx, "drawing_gdt", json!({ "drawing": tab, "position": [60, 60], "rows": [{ "symbol": "position", "before": "Ø", "tolerance": "0.1", "datums": ["A", "B"] }] }));
    assert_eq!(kind_of(&mut ctx, g["annotation"].as_u64().unwrap()), "Gdt");
    assert!(last(&mut ctx).ends_with(":: Geometric tolerance"));
    // With `anchor`: its leader goes to the geometry, as clicking it first does.
    let ga = call(&mut ctx, "drawing_gdt", json!({ "drawing": tab, "position": [70, 70], "anchor": a0, "rows": [{ "symbol": "flatness", "tolerance": "0.05" }] }));
    let gid = ga["annotation"].clone();
    let ann = ga["annotations"].as_array().unwrap().iter().find(|x| x["id"] == gid).unwrap().clone();
    assert!(ann.to_string().contains("leader"), "{ann}");
    let e = try_call(&mut ctx, "drawing_gdt", json!({ "drawing": tab, "position": [60, 60], "rows": [{ "symbol": "wobble" }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("rows[0].symbol"), "{e:?}");

    let sf = call(&mut ctx, "drawing_surface_finish", json!({ "drawing": tab, "position": quarter(&lines[0]) }));
    assert_eq!(kind_of(&mut ctx, sf["annotation"].as_u64().unwrap()), "SurfaceFinish");
    assert!(last(&mut ctx).ends_with(":: Surface finish"));
    // On an edge it stands on it: Flip turns it to the other side (live A45);
    // a free symbol has no Flip.
    call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": sf["annotation"], "action": "flip" }));
    assert!(last(&mut ctx).ends_with(":: Flip"), "{}", last(&mut ctx));
    let free = call(&mut ctx, "drawing_surface_finish", json!({ "drawing": tab, "position": [20, 270] }))["annotation"].clone();
    assert!(try_call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": free, "action": "flip" })).is_err());
    // The whole dialog (live request 1628): type, cells, lay; a bad type is refused.
    let iso = call(&mut ctx, "drawing_surface_finish", json!({ "drawing": tab, "position": [60, 270], "value": "Ra 1.6", "spec": { "kind": "IsoRemovalRequired", "left": ["a", "b", "c"], "right": ["R2", "R3", "R4"], "lay": "Angular", "parens": true } }));
    assert_eq!(kind_of(&mut ctx, iso["annotation"].as_u64().unwrap()), "SurfaceFinish");
    let e = try_call(&mut ctx, "drawing_surface_finish", json!({ "drawing": tab, "position": [80, 270], "spec": { "kind": "Shiny" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("spec"), "{e:?}");
    let w = call(&mut ctx, "drawing_weld", json!({ "drawing": tab, "target": a0, "position": [a0[0] - 20.0, a0[1] + 20.0], "spec": { "arrow": { "joint": "Fillet", "size": "6" }, "all_around": true } }));
    assert_eq!(kind_of(&mut ctx, w["annotation"].as_u64().unwrap()), "Weld");
    assert!(last(&mut ctx).ends_with(":: Weld symbol"));
    let e = try_call(&mut ctx, "drawing_weld", json!({ "drawing": tab, "target": a0, "position": [10, 10], "spec": { "arrow": { "joint": "Glue" } } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("spec"), "{e:?}");
    // Edge condition symbol (live 1955): on an edge, a row's sign and value, All around, ( ).
    let ec = call(&mut ctx, "drawing_edge_condition", json!({ "drawing": tab, "target": a0, "position": [a0[0] + 15.0, a0[1] + 15.0], "rows": [{ "sign": "+", "value": "0.3 mm" }], "all_around": true, "parens": true }));
    let ecid = ec["annotation"].clone();
    assert_eq!(kind_of(&mut ctx, ecid.as_u64().unwrap()), "EdgeCondition");
    assert!(last(&mut ctx).ends_with(":: Edge condition symbol"));
    // Drawn in the drawing's length format (this ISO drawing: "+0,3").
    let ann = ec["annotations"].as_array().unwrap().iter().find(|x| x["id"] == ecid).unwrap().clone();
    assert_eq!(ann["text"], "+0,3", "{ann}");
    let e = try_call(&mut ctx, "drawing_edge_condition", json!({ "drawing": tab, "target": a0, "position": [10, 10], "rows": [{ "sign": "*" }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("rows"), "{e:?}");

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
    let did = dt["annotation"].as_u64().unwrap();
    assert_eq!(kind_of(&mut ctx, did), "Datum");
    assert!(last(&mut ctx).ends_with(":: Datum"));
    // On the view's edge it follows the view's scale (qa DATUM-SCALE-DETACHED).
    let bounds = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["annotations"].as_array().unwrap().iter().find(|a| a["id"] == did).unwrap()["bounds"].clone();
    let before = bounds(&mut ctx);
    call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "action": "properties", "set": { "scale": "1:2" } }));
    let after = bounds(&mut ctx);
    assert!((num(&after["min"][0]) - num(&before["min"][0])).abs() + (num(&after["min"][1]) - num(&before["min"][1])).abs() > 1.0, "the datum moved with its edge: {before} → {after}");
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

/// A title block field's cell is a note edited in place, with the formats
/// locked too (live qa4 round 5 s11 (4)): the TITLE cell's middle field
/// replaced by typed text, the other two fields kept.
#[test]
fn drawing_title_block_fields() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_title.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let title = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["title_fields"].as_array().unwrap().iter().find(|f| f["field"] == "Title").cloned().expect("a Title cell");
    let t = title(&mut ctx);
    let cell: Vec<f64> = t["cell"].as_array().unwrap().iter().map(num).collect();
    assert!(cell[2] - cell[0] > 20.0 && cell[3] - cell[1] > 5.0, "{cell:?}");
    // The cell's left line is a format line of its own (M1): Move to, then Delete.
    let on = json!([cell[0], (cell[1] + cell[3]) / 2.0]);
    call(&mut ctx, "drawing_format_line", json!({ "drawing": tab, "point": on, "layer": "Border zones" }));
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Move to"));
    call(&mut ctx, "drawing_format_line", json!({ "drawing": tab, "point": on, "delete": true }));
    let edits = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["format_edits"].clone();
    assert_eq!(edits.as_array().map(|a| a.len()), Some(1), "{edits}");
    assert_eq!((edits[0]["deleted"].clone(), edits[0]["layer"].clone()), (json!(true), json!("Border zones")));
    let e = try_call(&mut ctx, "drawing_format_line", json!({ "drawing": tab, "point": [140, 108], "delete": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("point"), "no format line mid-sheet {e:?}");
    let value = t["value"].as_str().unwrap().to_string();
    let lines: Vec<&str> = value.split('\n').collect();
    assert_eq!(lines.len(), 3, "three property fields: {value}");
    let typed = format!("{}\nQA4 PLATE\n{}", lines[0], lines[2]);
    call(&mut ctx, "drawing_title_field", json!({ "drawing": tab, "field": "Title", "text": typed }));
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Edit title block"));
    let t = title(&mut ctx);
    assert_eq!(t["value"].as_str().unwrap(), typed);
    assert!(t["shown"].as_array().unwrap().iter().any(|l| l == "QA4 PLATE"), "{t}");
    // Explode note (the field's menu): the field becomes a note of its own in its cell.
    call(&mut ctx, "drawing_title_field", json!({ "drawing": tab, "field": "Title", "explode": true }));
    assert!(call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().ends_with(":: Explode note"));
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
    let note = g["annotations"].as_array().unwrap().iter().find(|a| a["text"].as_str().is_some_and(|t| t.contains("QA4 PLATE"))).cloned().expect("the field's note");
    let p = [num(&note["position"][0]), num(&note["position"][1])];
    assert!(p[0] > cell[0] && p[0] < cell[2] && p[1] > cell[1] && p[1] < cell[3], "{note} in {cell:?}");
    // The format no longer draws it: the note does.
    assert!(!g["title_fields"].as_array().unwrap().iter().any(|f| f["field"] == "Title"), "{}", g["title_fields"]);
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
    // Blind 25 mm by default (live).
    let d0 = num(&v["broken_out"][0]["depth_mm"]);
    assert!((d0 - 25.0).abs() < 1e-9, "{v}");
    // Edit it to a 3 mm blind depth.
    call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": a["id"], "points": tri(a), "depth": "3 mm", "index": 0 }));
    let v = views_of(&mut ctx).into_iter().find(|v| v["id"] == a["id"]).unwrap();
    assert_eq!(v["broken_out"].as_array().unwrap().len(), 1, "{v}");
    assert!((num(&v["broken_out"][0]["depth_mm"]) - 3.0).abs() < 1e-9, "{v}");
    let e = try_call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": a["id"], "points": tri(a), "depth": 0 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("depth"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": a["id"], "points": tri(a), "index": 4 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("index"), "{e:?}");
    // Offset goes with Up to entity only.
    let e = try_call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": a["id"], "points": tri(a), "offset": "2 mm" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("offset"), "{e:?}");
    // Not on section, detail or aligned section views (help broken_out_section_view).
    let (ax, ay) = ((num(&a["bounds"]["min"][0]) + num(&a["bounds"]["max"][0])) / 2.0, (num(&a["bounds"]["min"][1]) + num(&a["bounds"]["max"][1])) / 2.0);
    let det = call(&mut ctx, "drawing_detail_view", json!({ "drawing": tab, "parent": a["id"], "center": [ax, ay], "radius": 6, "position": [350, 250] }));
    let dv = det["views"].as_array().unwrap().last().unwrap().clone();
    let e = try_call(&mut ctx, "drawing_broken_out_section", json!({ "drawing": tab, "view": dv["id"], "points": tri(&dv) })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"), "{e:?}");
    // Spline and polygon crops of the other view.
    call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "spline", "points": tri(b) }));
    assert!(last(&mut ctx).ends_with(":: Crop view"));
    assert_eq!(views_of(&mut ctx).into_iter().find(|v| v["id"] == b["id"]).unwrap()["crop"], "spline");
    call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "polygon", "points": tri(b) }));
    assert_eq!(views_of(&mut ctx).into_iter().find(|v| v["id"] == b["id"]).unwrap()["crop"], "polygon");
    // An existing closed sketch curve (no shape pressed / Esc, live 1404): picked on its curve, not inside it.
    let (bx, by) = ((num(&b["bounds"]["min"][0]) + num(&b["bounds"]["max"][0])) / 2.0, (num(&b["bounds"]["min"][1]) + num(&b["bounds"]["max"][1])) / 2.0);
    // The picked curve leaves the sheet with the view made from it (live2 drw-crop-existing-profile,
    // drw-detail-existing-profile).
    let sketch_n = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["sketch_entities"].as_u64().unwrap_or(0);
    let n_before = sketch_n(&mut ctx);
    call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "circle", "picks": [[bx, by], [bx + 4.0, by]] }));
    let e = try_call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "existing", "at": [bx, by] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("at"), "{e:?}");
    call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "existing", "at": [bx + 4.0, by] }));
    assert_eq!(views_of(&mut ctx).into_iter().find(|v| v["id"] == b["id"]).unwrap()["crop"], "polygon");
    assert_eq!(sketch_n(&mut ctx), n_before, "the crop's circle is off the sheet");
    call(&mut ctx, "drawing_sketch", json!({ "drawing": tab, "tool": "circle", "picks": [[bx, by], [bx + 4.0, by]] }));
    let r = call(&mut ctx, "drawing_detail_view", json!({ "drawing": tab, "parent": b["id"], "profile": "existing", "at": [bx, by - 4.0], "position": [380, 250] }));
    assert!(r["view"].is_u64() || r["views"].is_array(), "{r}");
    assert_eq!(sketch_n(&mut ctx), n_before, "the detail's circle is off the sheet");
    let e = try_call(&mut ctx, "drawing_crop_view", json!({ "drawing": tab, "view": b["id"], "shape": "polygon", "points": [[1, 1], [2, 2]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("points"), "{e:?}");
    // Remove crop / Remove section keep their boundary as a sketch entity
    // (live asb 10-23 views (3)).
    let n0 = sketch_n(&mut ctx);
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": b["id"], "action": "remove_crop" }));
    assert!(last(&mut ctx).ends_with(":: Remove crop"));
    assert!(views_of(&mut ctx).into_iter().find(|v| v["id"] == b["id"]).unwrap()["crop"].is_null());
    let n1 = sketch_n(&mut ctx);
    assert!(n1 > n0, "the polygon stays: {n0} -> {n1}");
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": a["id"], "action": "remove_broken_out", "index": 0 }));
    assert!(last(&mut ctx).ends_with(":: Remove section"));
    assert!(views_of(&mut ctx).into_iter().find(|v| v["id"] == a["id"]).unwrap()["broken_out"].as_array().is_none_or(|x| x.is_empty()));
    // A closed spline through the profile's 3 points (live2 drw-broken-out-remove), not lines.
    assert_eq!(sketch_n(&mut ctx) - n1, 4, "the spline and its 3 points stay");
    let e = try_call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": b["id"], "action": "remove_crop" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"), "{e:?}");
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
    // Its TEXT "FOO" (at 1, 4; 0.75 high) as an editable note.
    assert_eq!(r["notes"], 1, "{r}");
    let notes = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    assert!(notes.to_string().contains("FOO"), "{notes}");
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

/// drawing_annotation_edit on leaders (live drw2 10-21 g, (6)): Add node /
/// Remove node / Add leader from a node / Delete on a callout's leaders, Add
/// leader, Move by grip and Remove leaders on a note, as the leader menus do.
#[test]
fn drawing_leader_nodes_and_branches() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_leaders.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let top = g["views"].as_array().unwrap().iter().find(|v| v["edges"].as_array().is_some_and(|es| es.iter().any(|e| e["type"] == "circle"))).unwrap().clone();
    let line = top["edges"].as_array().unwrap().iter().find(|e| e["type"] == "line" && (num(&e["a"][1]) - num(&e["b"][1])).abs() < 1e-6).unwrap().clone();
    let quarter = [num(&line["a"][0]) * 0.75 + num(&line["b"][0]) * 0.25, num(&line["a"][1]) * 0.75 + num(&line["b"][1]) * 0.25];
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let leaders_of = |ctx: &mut Ctx, id: u64| -> Vec<Value> {
        let g = call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
        g["annotations"].as_array().unwrap().iter().find(|a| a["id"] == id).and_then(|a| a["leaders"].as_array().cloned()).unwrap_or_default()
    };
    let p2 = |v: &Value| [num(&v[0]), num(&v[1])];
    let edit = |ctx: &mut Ctx, args: Value| {
        let mut a = json!({ "drawing": tab });
        a.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
        call(ctx, "drawing_annotation_edit", a)
    };

    let c = call(&mut ctx, "drawing_callout", json!({ "drawing": tab, "anchor": quarter, "position": [40, 250], "text": "7" }))["annotation"].as_u64().unwrap();
    let l = leaders_of(&mut ctx, c);
    assert_eq!((l.len(), l[0]["leader"].as_u64()), (1, Some(0)), "{l:?}");
    // Add node halfway along the leader to the anchor (on the line).
    let path = l[0]["path"].as_array().unwrap();
    let (s0, s1) = (p2(&path[0]), p2(&path[1]));
    let mid = [(s0[0] + s1[0]) / 2.0, (s0[1] + s1[1]) / 2.0];
    edit(&mut ctx, json!({ "annotation": c, "action": "add_node", "leader": 0, "point": mid }));
    assert!(last(&mut ctx).ends_with(":: Add node"));
    let l = leaders_of(&mut ctx, c);
    assert_eq!(l[0]["nodes"].as_u64(), Some(1));
    // The path runs border → node → anchor.
    let node = p2(&l[0]["path"][1]);
    assert!((node[0] - mid[0]).abs() < 1e-3 && (node[1] - mid[1]).abs() < 1e-3, "{node:?}");
    // Add leader from that node: the branch starts there.
    edit(&mut ctx, json!({ "annotation": c, "action": "add_leader", "from_node": [0, 0], "point": [120, 270] }));
    assert!(last(&mut ctx).ends_with(":: Add leader"));
    let l = leaders_of(&mut ctx, c);
    assert_eq!(l.len(), 2);
    let b0 = p2(&l[1]["path"][0]);
    assert!((b0[0] - node[0]).abs() < 1e-3 && (b0[1] - node[1]).abs() < 1e-3, "{b0:?}");
    // Remove node: the branch starts at the border again.
    edit(&mut ctx, json!({ "annotation": c, "action": "remove_node", "leader": 0, "node": 0 }));
    assert!(last(&mut ctx).ends_with(":: Remove node"));
    let l = leaders_of(&mut ctx, c);
    assert_eq!(l[0]["nodes"].as_u64(), Some(0));
    assert!((p2(&l[1]["path"][0])[0] - node[0]).abs() > 1e-3);
    // Delete the leader to the anchor: the callout keeps its added one.
    edit(&mut ctx, json!({ "annotation": c, "action": "delete_leader", "leader": 0 }));
    assert!(last(&mut ctx).ends_with(":: Delete leader"));
    let l = leaders_of(&mut ctx, c);
    assert!(l[0]["path"].is_null() && l[1]["path"].is_array(), "{l:?}");

    // A note: Add leader branches from its landing's bend; Move by grip; Remove leaders.
    let a0 = [num(&line["a"][0]), num(&line["a"][1])];
    let n = call(&mut ctx, "drawing_note", json!({ "drawing": tab, "text": "BREAK EDGES", "position": [300, 250], "leader": a0 }))["annotation"].as_u64().unwrap();
    edit(&mut ctx, json!({ "annotation": n, "action": "add_leader", "point": [350, 200] }));
    let l = leaders_of(&mut ctx, n);
    assert_eq!(l.len(), 2);
    assert_eq!(l[1]["path"][0], l[0]["path"][1], "from the bend");
    // The second leader's end into empty sheet, then onto the edge's end.
    edit(&mut ctx, json!({ "annotation": n, "action": "move_leader", "leader": 1, "point": [14, 150] }));
    assert!(last(&mut ctx).ends_with(":: Move by grip"));
    let l = leaders_of(&mut ctx, n);
    let end = |l: &Vec<Value>, i: usize| p2(l[i]["path"].as_array().unwrap().last().unwrap());
    assert!((end(&l, 1)[0] - 14.0).abs() < 1e-3 && (end(&l, 1)[1] - 150.0).abs() < 1e-3, "{l:?}");
    edit(&mut ctx, json!({ "annotation": n, "action": "move_leader", "leader": 1, "point": a0 }));
    let l = leaders_of(&mut ctx, n);
    assert!((end(&l, 1)[0] - a0[0]).abs() < 0.5 && (end(&l, 1)[1] - a0[1]).abs() < 0.5, "attached to the vertex: {l:?}");
    edit(&mut ctx, json!({ "annotation": n, "action": "remove_leaders" }));
    assert!(last(&mut ctx).ends_with(":: Remove leaders"));
    assert!(leaders_of(&mut ctx, n).is_empty());

    // Errors name the argument.
    let e = try_call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": c, "action": "delete_leader", "leader": 5 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("leader"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": tab, "annotation": c, "action": "remove_node", "leader": 1, "node": 0 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("node"), "{e:?}");
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
    // Reorder from 5, with the dialog's own shape and border size.
    call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "add", "annotations": [ids[1], ids[0]], "number": 5, "shape": "hexagon", "border_size": 3 }));
    assert_eq!(numbers(&mut ctx), vec![json!(6), json!(5), Value::Null]);
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }));
    let first = g["annotations"].as_array().unwrap().iter().find(|x| x["id"] == ids[0]).unwrap().clone();
    assert_eq!((first["inspection_shape"].clone(), first["inspection_border_size"].clone()), (json!("Hexagon"), json!(3)), "{first}");
    let e = try_call(&mut ctx, "drawing_inspection", json!({ "drawing": tab, "action": "add", "annotations": [ids[2]], "border_size": 6 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("border_size"), "{e:?}");
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
    // Projection angle: the views swap rows (live 2313): ISO A3 is first angle, the Top below the Front.
    assert_eq!(r["properties"]["projection"], "first_angle", "{r}");
    // The base view and its projected child in the same column (the Top).
    let col = |r: &Value| -> (f64, f64) {
        let vs = r["views"].as_array().unwrap();
        let base = vs.iter().find(|v| v["kind"] == "base").unwrap_or_else(|| panic!("{r}"));
        let child = vs.iter().find(|v| v["parent"] == base["id"] && (v["position"][0].as_f64().unwrap() - base["position"][0].as_f64().unwrap()).abs() < 1e-6).unwrap_or_else(|| panic!("{r}"));
        (base["position"][1].as_f64().unwrap(), child["position"][1].as_f64().unwrap())
    };
    let (front, top) = col(&r);
    assert!(top < front, "first angle: the Top below the Front");
    let r = call(&mut ctx, "drawing_properties", json!({ "drawing": tab, "set": { "projection": "third_angle" } }));
    assert_eq!(r["properties"]["projection"], "third_angle", "{r}");
    let (front3, top3) = col(&r);
    assert!(top3 > front3 && (front3 - top).abs() < 1e-6, "third angle: the rows swap, the Top above: {r}");
    call(&mut ctx, "drawing_properties", json!({ "drawing": tab, "set": { "projection": "first_angle" } }));
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

/// Files that aren't CAD (Markdown, CSV, a picture) import as file tabs,
/// one edit each, as the app's + › Import… (Onshape: imported for viewing).
#[test]
fn import_files_stores_other_files_as_file_tabs() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "import_other.noshape");
    let md = scratch("notes.md");
    std::fs::write(&md, "# Notes\n\nThe plate is **6061**.\n").unwrap();
    let csv = scratch("bom.csv");
    std::fs::write(&csv, "Part,Qty\nPlate,1\n").unwrap();
    let r = call(&mut ctx, "import_files", json!({ "files": [md.to_str().unwrap(), csv.to_str().unwrap()] }));
    let tabs: Vec<(&str, &str)> = r["tabs"].as_array().unwrap().iter().map(|t| (t["name"].as_str().unwrap(), t["kind"].as_str().unwrap())).collect();
    assert_eq!(tabs, [("notes.md", "File"), ("bom.csv", "File")], "{r}");
    assert_eq!(r["first"], r["tabs"][1]["id"], "the window shows the last one");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 2 }));
    assert_eq!(hist["history"][0]["description"], "Import bom.csv", "{hist}");
    assert_eq!(hist["history"][1]["description"], "Import notes.md", "{hist}");
    // With a CAD file: its Part Studio too, the other file stored as well.
    let step = scratch("import_other.step");
    call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP", "file": step.to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap(), md.to_str().unwrap()] }));
    let kinds: Vec<&str> = r["tabs"].as_array().unwrap().iter().map(|t| t["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"Part Studio") && kinds.iter().filter(|k| **k == "File").count() == 2, "the stored .md and the kept .step: {r}");
}

/// A PDF file tab's pages render (render_file_page; Onshape views PDFs in
/// their tab): a drawing stored as PDF, its one ISO A3 page at 1 px per point.
#[test]
fn a_pdf_file_tab_renders_its_pages() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "pdf_tab.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "drawing_export", json!({ "drawing": tab, "format": "PDF", "options": "store" }));
    let pdf = r["stored_tab"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
    let file = scratch("pdf_tab_page.png");
    let p = call(&mut ctx, "render_file_page", json!({ "tab": pdf, "scale": 1.0, "file": file.to_str().unwrap() }));
    assert_eq!(p["pages"], 1, "{p}");
    let bytes = std::fs::read(&file).unwrap();
    let mut r = png::Decoder::new(std::io::Cursor::new(&bytes)).read_info().unwrap();
    let info = r.info().clone();
    // A3 landscape: 420 × 297 mm = 1190.55 × 841.89 points (whole pixels).
    assert_eq!((info.width, info.height), (1190, 841), "{p}");
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    r.next_frame(&mut buf).unwrap();
    let px: Vec<&[u8]> = buf.chunks(info.color_type.samples()).map(|c| &c[..3]).collect();
    // White paper with the border, title block and views drawn on it.
    let dark = px.iter().filter(|c| c.iter().all(|&v| v < 128)).count();
    let white = px.iter().filter(|c| c.iter().all(|&v| v > 240)).count();
    assert!(white > px.len() / 2 && dark > 2000, "{dark} dark, {white} white of {} pixels", px.len());
    let e = try_call(&mut ctx, "render_file_page", json!({ "tab": pdf, "page": 2 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("page"), "{e:?}");
    let e = try_call(&mut ctx, "render_file_page", json!({ "tab": "Part Studio 1" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tab"), "{e:?}");
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
    // On a folder: every feature in it; Remove expression clears them.
    call(&mut ctx, "feature_add_folder", json!({ "features": ["Sketch 1", "Extrude 1"], "name": "Base" }));
    call(&mut ctx, "feature_dynamic_suppression", json!({ "folder": "Base", "unsuppressed_if": "2 mm < 1 mm" }));
    assert_eq!(call(&mut ctx, "feature_get", json!({ "feature": "Extrude 1" }))["suppression_expression"], "2 mm < 1 mm");
    call(&mut ctx, "feature_dynamic_suppression", json!({ "folder": "Base" }));
    assert!(call(&mut ctx, "feature_get", json!({ "feature": "Extrude 1" })).get("suppression_expression").is_none());
    // The list's folders and filter (ui_list).
    let l = call(&mut ctx, "ui_list", json!({ "action": "collapse_all" }));
    assert!(l["folders"].as_array().unwrap().iter().all(|f| f["open"] == false), "{l}");
    let l = call(&mut ctx, "ui_list", json!({ "action": "expand", "items": ["Base"] }));
    assert_eq!(l["folders"].as_array().unwrap().iter().find(|f| f["name"] == "Base").unwrap()["open"], true, "{l}");
    let l = call(&mut ctx, "ui_list", json!({ "action": "filter", "text": "extrude" }));
    assert_eq!(l["shown_features"], json!(["Extrude 1"]), "{l}");
    let l = call(&mut ctx, "ui_list", json!({ "action": "filter", "text": "" }));
    assert!(l.get("shown_features").is_none(), "{l}");
    assert_eq!(try_call(&mut ctx, "ui_list", json!({ "action": "expand", "items": ["Nope"] })).unwrap_err().param.as_deref(), Some("items[0]"));
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
    // Live (request 1582): the move takes no undo step and the assembly's Undo is off after it.
    let u = call(&mut ctx, "history_undo", json!({ "tab": "Assembly 1" }));
    assert_eq!((u["undone"].clone(), u["can_undo"].clone()), (json!(false), json!(false)), "{u}");
    assert_eq!(count(&m), 2, "the two moved, one subassembly instance: {m}");
    let sub = m["new_instance"]["id"].as_str().unwrap().to_string();
    let tabs = call(&mut ctx, "document_info", json!({}));
    assert!(tabs["tabs"].as_array().unwrap().iter().any(|t| t["name"] == "Assembly 2"), "{tabs}");
    let l = call(&mut ctx, "assembly_lock", json!({ "instance": sub, "lock": "current" }));
    assert_eq!(l["instances"].as_array().unwrap().iter().find(|i| i["id"] == sub).unwrap()["lock"], "current");
    assert!(try_call(&mut ctx, "assembly_lock", json!({ "instance": b, "lock": "current" })).is_err(), "not a subassembly");
    let e = try_call(&mut ctx, "assembly_lock", json!({ "instance": sub, "lock": "Open" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("lock"));
    // Add to named positions… (live 2027): a column of the parent's table; "Named position 1" made for it; each
    // position picks one of the subassembly's own; applying it locks the subassembly there.
    call(&mut ctx, "assembly_named_position", json!({ "tab": "Assembly 2", "action": "add", "name": "Open" }));
    let ap = call(&mut ctx, "assembly_add_to_named_positions", json!({ "tab": "Assembly 1", "instance": sub }));
    assert!(ap["toast"].as_str().unwrap().ends_with(" added to named positions"), "{ap}");
    assert_eq!(ap["named_positions"], json!(["Named position 1"]), "{ap}");
    assert!(try_call(&mut ctx, "assembly_add_to_named_positions", json!({ "tab": "Assembly 1", "instance": b })).is_err(), "not a subassembly");
    let e = try_call(&mut ctx, "assembly_named_position", json!({ "tab": "Assembly 1", "action": "set_sub", "position": "Named position 1", "instance": sub, "sub_position": "Shut" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("sub_position"), "{e:?}");
    call(&mut ctx, "assembly_named_position", json!({ "tab": "Assembly 1", "action": "set_sub", "position": "Named position 1", "instance": sub, "sub_position": "Open" }));
    call(&mut ctx, "assembly_lock", json!({ "tab": "Assembly 1", "instance": sub, "lock": "current" }));
    let applied = call(&mut ctx, "assembly_named_position", json!({ "tab": "Assembly 1", "action": "apply", "position": "Named position 1" }));
    assert!(applied["named_positions"].as_array().is_some_and(|v| v.len() == 1), "{applied}");
    let st = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    assert_eq!(st["instances"].as_array().unwrap().iter().find(|i| i["id"] == sub).unwrap()["lock"], "Open", "{st}");
    let dv = call(&mut ctx, "assembly_dissolve_subassembly", json!({ "instances": [sub] }));
    assert_eq!(count(&dv), 3, "{dv}");
    assert!(last(&mut ctx).ends_with("Dissolve subassembly"));
    // No undo step for the dissolve either (live); ours keeps Undo off for the
    // edits before the move (whole-tab undo would bring the moved instances back).
    let u = call(&mut ctx, "history_undo", json!({ "tab": "Assembly 1" }));
    assert_eq!(u["undone"], json!(false), "{u}");
    let cs = call(&mut ctx, "assembly_create_subassembly", json!({}));
    assert_eq!(count(&cs), 4);
    // Nor for Create empty subassembly (live asm-menu-create-empty-subassembly: Undo stays off).
    let u = call(&mut ctx, "history_undo", json!({ "tab": "Assembly 1" }));
    assert_eq!(u["undone"], json!(false), "{u}");
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

/// An instance whose name has a slash (inch standard content, "… 1/4-20 x …") is found by its name, not read as a
/// subassembly path.
#[test]
fn an_instance_named_with_a_slash_is_found_by_name() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "std-slash.noshape");
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }));
    let s = call(&mut ctx, "assembly_insert_standard", json!({ "component": "Socket button head cap screw", "size": "1/4-20" }));
    let id = s["inserted"][0].as_str().unwrap();
    let name = s["instances"].as_array().unwrap().iter().find(|i| i["id"] == id).and_then(|i| i["name"].as_str()).unwrap().to_string();
    assert!(name.contains('/'), "{name}");
    let r = call(&mut ctx, "assembly_set_state", json!({ "instances": [name], "fixed": true }));
    assert!(r["instances"].as_array().unwrap().iter().any(|i| i["id"] == id && i["fixed"] == true), "{r}");
}

/// STANDARD-CONTENT-PROPS-OP: a standard content configuration's Part number and Description (the Insert panel's
/// pencil, the instance's Properties…), kept for this computer: set by instance, read back by the component.
#[test]
fn standard_content_part_number_and_description() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "std-props.noshape");
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }));
    let s = call(&mut ctx, "assembly_insert_standard", json!({ "component": "Socket button head cap screw", "size": "1/4-20" }));
    let id = s["inserted"][0].as_str().unwrap().to_string();
    let p = call(&mut ctx, "assembly_standard_properties", json!({ "instance": id }));
    let generated = p["generated_description"].as_str().unwrap().to_string();
    assert!(!generated.is_empty() && p["description"] == generated.as_str() && p["part_number"] == "", "{p}");
    let p = call(&mut ctx, "assembly_standard_properties", json!({ "instance": id, "part_number": "STD-0042", "description": "Button screw, zinc" }));
    assert_eq!((p["part_number"].as_str(), p["description"].as_str(), p["own_description"].as_bool()), (Some("STD-0042"), Some("Button screw, zinc"), Some(true)), "{p}");
    // The same configuration named by its component: the same values (they belong to it, not to the instance).
    let q = call(&mut ctx, "assembly_standard_properties", json!({ "component": "Socket button head cap screw", "size": "1/4-20" }));
    assert_eq!((q["part_number"].as_str(), q["description"].as_str()), (Some("STD-0042"), Some("Button screw, zinc")), "{q}");
    // Another size has none of its own.
    let other = call(&mut ctx, "assembly_standard_properties", json!({ "component": "Socket button head cap screw", "size": "#10-24" }));
    assert_eq!(other["part_number"], "", "{other}");
    // null: the generated description again.
    let r = call(&mut ctx, "assembly_standard_properties", json!({ "instance": id, "description": null }));
    assert_eq!((r["description"].as_str(), r["own_description"].as_bool()), (Some(generated.as_str()), Some(false)), "{r}");
    let e = try_call(&mut ctx, "assembly_standard_properties", json!({ "instance": "Part Studio 1 <1>" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("instance"), "{e:?}");
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
    // Child views inherit Show annotations from model (live 3102).
    assert_eq!(det["views"].as_array().unwrap().last().unwrap()["model_annotations"], false, "{det}");
    let pr = call(&mut ctx, "drawing_projected_view", json!({ "drawing": tab, "parent": base["id"], "position": [100, 260] }));
    assert_eq!(pr["views"].as_array().unwrap().last().unwrap()["model_annotations"], false, "{pr}");
    // A circle detail: move and resize its circle, its label onto a leader
    // and back (the circle's grips and menu, live asb 10-23 views (2)).
    let cd = call(&mut ctx, "drawing_detail_view", json!({ "drawing": tab, "parent": base["id"], "center": [100, 150], "radius": 8, "position": [330, 260] }));
    let did = cd["views"].as_array().unwrap().last().unwrap()["id"].clone();
    let detail = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"].as_array().unwrap().iter().find(|v| v["id"] == did).unwrap()["detail"].clone();
    call(&mut ctx, "drawing_edit_detail", json!({ "drawing": tab, "view": did, "center": [104, 151], "radius": 12 }));
    let dj = detail(&mut ctx);
    assert!((num(&dj["center"][0]) - 104.0).abs() < 1e-3 && (num(&dj["center"][1]) - 151.0).abs() < 1e-3 && (num(&dj["radius"]) - 12.0).abs() < 1e-3, "{dj}");
    assert!(dj["label_at"].is_null());
    call(&mut ctx, "drawing_edit_detail", json!({ "drawing": tab, "view": did, "leader": true }));
    assert!(detail(&mut ctx)["label_at"].is_array());
    call(&mut ctx, "drawing_edit_detail", json!({ "drawing": tab, "view": did, "label_at": [130, 180] }));
    assert_eq!(detail(&mut ctx)["label_at"], json!([130.0, 180.0]));
    call(&mut ctx, "drawing_edit_detail", json!({ "drawing": tab, "view": did, "leader": false }));
    assert!(detail(&mut ctx)["label_at"].is_null());
    // Edit detail's options.
    let ed = call(&mut ctx, "drawing_edit_detail", json!({ "drawing": tab, "view": did, "scale": 3, "label": "Z" }));
    let dv = ed["views"].as_array().unwrap().iter().find(|v| v["id"] == did).unwrap().clone();
    assert!((num(&dv["scale"]) - 3.0).abs() < 1e-9, "{dv}");
    // View properties › Prefix / Letter / Suffix (live asbv-97: the
    // drawing's "DETAIL" until changed); "\n" starts a label line.
    let p = call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": did }));
    assert_eq!((&p["properties"]["prefix"], &p["properties"]["label"], &p["properties"]["suffix"]), (&json!("DETAIL"), &json!("Z"), &json!("")), "{p}");
    let label = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"].as_array().unwrap().iter().find(|v| v["id"] == did).unwrap()["label_text"].clone();
    assert_eq!(label(&mut ctx), json!(["DETAIL Z", "SCALE 3:1"]));
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": did, "set": { "prefix": "VIEW", "suffix": "(ROTATED)\nSEE NOTE 2" } }));
    assert_eq!(label(&mut ctx), json!(["VIEW Z (ROTATED)", "SEE NOTE 2", "SCALE 3:1"]));
    // ☐ View label / ☐ Scale label hide what they name (qa VIEW-LABEL-TOGGLE).
    let p = call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": did }));
    assert_eq!((&p["properties"]["view_label"], &p["properties"]["scale_label"]), (&json!(true), &json!(true)), "{p}");
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": did, "set": { "view_label": false } }));
    assert_eq!(label(&mut ctx), json!(["SCALE 3:1"]));
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": did, "set": { "scale_label": false } }));
    assert!(label(&mut ctx).is_null());
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": did, "set": { "view_label": true, "scale_label": true } }));
    assert_eq!(label(&mut ctx), json!(["VIEW Z (ROTATED)", "SEE NOTE 2", "SCALE 3:1"]));
    let e = try_call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": base["id"], "set": { "prefix": "X" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.prefix"), "{e:?}");
    // A typed scale is kept as typed and labels the view while it differs
    // from the sheet's (live request 1645); inheriting again hides it.
    let base_label = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"].as_array().unwrap().iter().find(|v| v["id"] == base["id"]).unwrap()["label_text"].clone();
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": base["id"], "set": { "scale": "2:4" } }));
    assert_eq!(base_label(&mut ctx), json!(["SCALE 2:4"]));
    let p = call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": base["id"] }));
    assert_eq!(p["properties"]["scale"], json!("2:4"), "{p}");
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": base["id"], "set": { "scale": null } }));
    assert!(base_label(&mut ctx).is_null());
    call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "action": "properties", "set": { "scale": "0.5" } }));
    let sp = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "action": "properties" }));
    assert_eq!(sp["properties"]["scale"], json!("0.5:1"), "{sp}");
    let e = try_call(&mut ctx, "drawing_edit_detail", json!({ "drawing": tab, "view": base["id"], "radius": 5 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_detail_view", json!({ "drawing": tab, "parent": base["id"], "profile": "polygon", "points": [[90, 140], [110, 160]], "position": [200, 260] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("points"));
    let e = try_call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "named_view": "Nope", "position": [200, 60] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("named_view"));
    let e = try_call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "source": "Part Studio 1", "simplification": "auto", "simplification_value": "1 mm", "position": [200, 60] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("simplification_value"));
}

/// Tab folders as the tab bar makes them: create (nested), move a tab in,
/// Move to parent folder, reorder, unpack; the document lists them.
#[test]
fn tab_folders() {
    let mut ctx = Ctx::new();
    let path = scratch("folders.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let f = call(&mut ctx, "tab_folder_create", json!({}));
    assert_eq!(f["folder"]["name"], "Folder 1", "{f}");
    call(&mut ctx, "tab_folder_create", json!({ "name": "B", "folder": "Folder 1" }));
    call(&mut ctx, "tab_move", json!({ "tab": "Part Studio 1", "folder": "B" }));
    let tab_folder = |ctx: &mut Ctx, name: &str| {
        let d = call(ctx, "document_info", json!({}));
        d["tabs"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap()["folder"].clone()
    };
    assert_eq!(tab_folder(&mut ctx, "Part Studio 1"), "B");
    let d = call(&mut ctx, "document_info", json!({}));
    let b = d["folders"].as_array().unwrap().iter().find(|f| f["name"] == "B").unwrap().clone();
    assert_eq!(b["folder"], "Folder 1", "{d}");
    // Move to parent folder: B → Folder 1, then out to the root.
    call(&mut ctx, "tab_move", json!({ "tab": "Part Studio 1", "to_parent": true }));
    assert_eq!(tab_folder(&mut ctx, "Part Studio 1"), "Folder 1");
    call(&mut ctx, "tab_move", json!({ "tab": "Part Studio 1", "to_parent": true }));
    assert!(tab_folder(&mut ctx, "Part Studio 1").is_null());
    let e = try_call(&mut ctx, "tab_move", json!({ "tab": "Part Studio 1", "to_parent": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("to_parent"));
    // A folder can't go into itself.
    let e = try_call(&mut ctx, "tab_move", json!({ "tab": "Folder 1", "folder": "B" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("folder"));
    // Before a tab inside a folder: it joins that folder in front of it.
    call(&mut ctx, "tab_move", json!({ "tab": "Assembly 1", "folder": "B" }));
    call(&mut ctx, "tab_move", json!({ "tab": "Part Studio 1", "before": "Assembly 1" }));
    assert_eq!(tab_folder(&mut ctx, "Part Studio 1"), "B");
    // Unpack: its tabs go to the parent.
    call(&mut ctx, "tab_folder_unpack", json!({ "folder": "B" }));
    assert_eq!(tab_folder(&mut ctx, "Part Studio 1"), "Folder 1");
    assert_eq!(tab_folder(&mut ctx, "Assembly 1"), "Folder 1");
    let d = call(&mut ctx, "document_info", json!({}));
    assert_eq!(d["folders"].as_array().unwrap().len(), 1, "{d}");
}

/// Tab › Move to document…: into a new document (named, next to this one), then into an existing one; the tabs
/// leave this document; the last tab can't go.
#[test]
fn tab_move_to_document() {
    let mut ctx = Ctx::new();
    let path = scratch("move-src.noshape");
    let other = scratch("move-other.noshape");
    call(&mut ctx, "document_create", json!({ "path": other.to_str().unwrap() }));
    call(&mut ctx, "document_close", json!({}));
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(&mut ctx, "variable_create_studio", json!({}));
    let names = |v: &Value| v["tabs"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let r = call(&mut ctx, "tab_move_to_document", json!({ "tabs": ["Variable Studio 1"], "name": "move-new" }));
    assert!(!names(&r).contains(&"Variable Studio 1".to_string()), "{r}");
    let new = std::path::PathBuf::from(r["document"].as_str().unwrap());
    assert_eq!(new.file_name().unwrap(), "move-new.noshape", "{r}");
    assert_eq!(new.parent(), path.parent());
    // Into the existing document: Assembly 1 there becomes "Assembly 2".
    let r = call(&mut ctx, "tab_move_to_document", json!({ "tabs": ["Assembly 1"], "document": other.to_str().unwrap() }));
    assert_eq!(names(&r), ["Part Studio 1"], "{r}");
    let e = try_call(&mut ctx, "tab_move_to_document", json!({ "tabs": ["Part Studio 1"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tabs"));
    let e = try_call(&mut ctx, "tab_move_to_document", json!({ "tabs": ["Nope"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tabs"));
    let o = call(&mut ctx, "document_open", json!({ "path": other.to_str().unwrap() }));
    assert_eq!(names(&o), ["Part Studio 1", "Assembly 1", "Assembly 2"], "{o}");
    let n = call(&mut ctx, "document_open", json!({ "path": new.to_str().unwrap() }));
    assert_eq!(names(&n), ["Variable Studio 1"], "{n}");
    assert_eq!(n["name"], "move-new", "{n}");
    let _ = std::fs::remove_file(&new);
    // A Part Studio's referenced tabs (its Variable Studio) go with it (live 2323), unless `referenced` is false.
    let refs = scratch("move-refs.noshape");
    call(&mut ctx, "document_create", json!({ "path": refs.to_str().unwrap() }));
    call(&mut ctx, "variable_create_studio", json!({}));
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "Part Studio 2" }));
    let r = call(&mut ctx, "tab_move_to_document", json!({ "tabs": ["Part Studio 2"], "name": "move-refs-to", "referenced": false }));
    assert_eq!(r["moved"], json!(["Part Studio 2"]), "{r}");
    let _ = std::fs::remove_file(r["document"].as_str().unwrap());
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "Part Studio 2" }));
    let r = call(&mut ctx, "tab_move_to_document", json!({ "tabs": ["Part Studio 2"], "name": "move-refs-to" }));
    assert_eq!((r["moved"].clone(), r["message"].as_str()), (json!(["Part Studio 2", "Variable Studio 1"]), Some("Part Studio 2, Variable Studio 1 have been moved to move-refs-to.")), "{r}");
    let _ = std::fs::remove_file(r["document"].as_str().unwrap());
}

/// The documents page through its ops: folders, labels, rename, move, copy, details and the trash, each as the page
/// does it (labels and the recent list follow a document).
#[test]
fn documents_page() {
    let dir = scratch("docs-page");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let mut ctx = Ctx::new();
    ctx.documents = Some(dir.clone());
    call(&mut ctx, "document_create", json!({ "path": dir.join("Bracket.noshape").to_str().unwrap() }));
    call(&mut ctx, "document_close", json!({}));
    let f = call(&mut ctx, "documents_create_folder", json!({}));
    assert!(f["created"].as_str().unwrap().ends_with("Untitled folder"), "{f}");
    call(&mut ctx, "documents_create_folder", json!({ "name": "Fixtures" }));
    let e = try_call(&mut ctx, "documents_create_folder", json!({ "name": "Fixtures" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"), "{e:?}");
    let l = call(&mut ctx, "documents_list", json!({}));
    assert_eq!((l["folders"].as_array().unwrap().len(), l["documents"][0]["name"].as_str()), (2, Some("Bracket")), "{l}");
    let z = call(&mut ctx, "documents_list", json!({ "sort": "name_desc" }));
    assert_eq!(z["folders"][0]["name"], "Untitled folder", "{z}");
    // A label, then the document renamed and moved: the label follows.
    call(&mut ctx, "documents_label", json!({ "label": "Brackets", "create": true, "path": "Bracket" }));
    let r = call(&mut ctx, "documents_rename", json!({ "path": "Bracket", "name": "Bracket v2" }));
    let renamed = r["path"].as_str().unwrap().to_string();
    assert!(renamed.ends_with("Bracket v2.noshape"), "{r}");
    let m = call(&mut ctx, "documents_move", json!({ "path": renamed, "to": "Fixtures" }));
    let moved = m["path"].as_str().unwrap().to_string();
    assert!(moved.contains("Fixtures"), "{m}");
    let l = call(&mut ctx, "documents_list", json!({ "label": "Brackets" }));
    assert_eq!(l["documents"][0]["path"], moved.as_str(), "{l}");
    // Copy workspace…, Details.
    let c = call(&mut ctx, "documents_copy", json!({ "path": moved }));
    assert_eq!(c["name"], "Bracket v2 - Copy", "{c}");
    let d = call(&mut ctx, "documents_details", json!({ "path": moved, "description": "Wall bracket" }));
    assert_eq!(d["description"], "Wall bracket", "{d}");
    // The trash: send, restore, send again and delete, a folder and Empty trash.
    let copy = c["copy"].as_str().unwrap().to_string();
    let t = call(&mut ctx, "documents_trash", json!({ "path": copy }));
    assert!(t["message"].as_str().unwrap().ends_with("has been sent to the trash and will be automatically deleted after 30 days."), "{t}");
    assert!(!std::path::Path::new(&copy).exists());
    let r = call(&mut ctx, "documents_restore", json!({ "item": "Bracket v2 - Copy" }));
    assert_eq!(r["message"], "Bracket v2 - Copy has been restored.", "{r}");
    assert!(std::path::Path::new(&copy).exists());
    call(&mut ctx, "documents_trash", json!({ "path": copy }));
    call(&mut ctx, "documents_delete", json!({ "item": "Bracket v2 - Copy" }));
    let e = try_call(&mut ctx, "documents_restore", json!({ "item": "Bracket v2 - Copy" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("item"), "{e:?}");
    call(&mut ctx, "documents_trash", json!({ "path": "Untitled folder" }));
    let l = call(&mut ctx, "documents_list", json!({ "place": "trash" }));
    assert!(l["items"].as_array().unwrap().iter().any(|i| i["name"] == "Untitled folder" && i["folder"] == true), "{l}");
    call(&mut ctx, "documents_empty_trash", json!({}));
    assert!(call(&mut ctx, "documents_list", json!({ "place": "trash" }))["items"].as_array().unwrap().is_empty());
    // An open document can't be trashed.
    call(&mut ctx, "document_open", json!({ "path": moved }));
    let e = try_call(&mut ctx, "documents_trash", json!({ "path": moved })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("path"), "{e:?}");
    call(&mut ctx, "document_close", json!({}));
    let _ = std::fs::remove_dir_all(&dir);
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
    // Through the hole's axis (a cylinder face, as the dialog takes it).
    let hole = call(&mut ctx, "query_entities", json!({ "kind": "faces", "surface": "cylinder" }))["faces"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "render_image", json!({ "width": 200, "height": 150, "section": [{ "plane": hole }] }));
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
    assert_eq!(again["info"], json!("No inputs pasted. Existing inputs have the same internal FeatureScript ID."), "live's words: {again}");
    // A Derived feature of Part Studio 1 follows Holder's Size.
    // By tab name (stored as the tab's id); not a Part Studio: refused (qa4 CFG-MASS).
    let e = try_call(&mut ctx, "feature_add", json!({ "tab": "Holder", "type": "importDerived", "params": { "partStudio": "Assembly 1" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("params.partStudio"), "{e:?}");
    let d = call(&mut ctx, "feature_add", json!({ "tab": "Holder", "type": "importDerived", "params": { "partStudio": "Part Studio 1" } }));
    assert_eq!(d["feature_status"]["status"], "Ok", "{d}");
    let g = call(&mut ctx, "feature_get", json!({ "tab": "Holder", "feature": d["feature"].clone() }));
    assert_eq!(g["params"]["partStudio"], ps1.as_str(), "{g}");
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
    let lite = call(&mut ctx, "config_add_input", json!({ "kind": "checkbox", "name": "Lite", "default": false }))["input"].as_str().unwrap().to_string();
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
    // An edge dimension turns with the view and keeps its value (qa
    // DWG-ROTATE-VIEW-DIMS-ZERO; live V44).
    let fv0 = get(&mut ctx, true)["views"].as_array().unwrap().iter().find(|v| v["id"] == free).cloned().unwrap();
    let top = fv0["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line" && (num(&e["a"][1]) - num(&e["b"][1])).abs() < 1e-6).max_by(|x, y| (num(&x["a"][1]) + num(&x["b"][0]) - num(&x["a"][0])).total_cmp(&(num(&y["a"][1]) + num(&y["b"][0]) - num(&y["a"][0])))).cloned().unwrap();
    let tm = [(num(&top["a"][0]) + num(&top["b"][0])) / 2.0, num(&top["a"][1])];
    let dim = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": [tm], "place": [tm[0], tm[1] + 15.0] }));
    let dim_text = |ctx: &mut Ctx| get(ctx, false)["annotations"].as_array().unwrap().iter().find(|x| x["id"] == dim["annotation"]).unwrap()["text"].clone();
    let before = dim_text(&mut ctx);
    assert_ne!(before, json!("0"), "{dim}");
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": free, "action": "rotate", "by": 90 }));
    assert_eq!(last(&mut ctx, 1), ["Rotate view"]);
    assert!((num(&view(&mut ctx, &free)["rotation"]) - 90.0).abs() < 1e-9);
    assert_eq!(dim_text(&mut ctx), before, "the edge dimension keeps its value");
    call(&mut ctx, "drawing_delete", json!({ "drawing": tab, "annotations": [dim["annotation"]] }));
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
    // Not aligned with its parent any more, it turns on its own (live: Rotate
    // view on views not aligned to a parent); a view with children doesn't.
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": child, "action": "rotate", "by": 15 }));
    assert!((num(&view(&mut ctx, &child)["rotation"]) - 15.0).abs() < 1e-9);
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": child, "action": "rotate", "to": 0 }));
    let e = try_call(&mut ctx, "drawing_view_edit", json!({ "drawing": tab, "view": base, "action": "rotate", "by": 15 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"), "{e:?}");
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
    // A new scale ticks Scale label even without a view label (live 1603 (d)).
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free, "set": { "view_label": false, "scale_label": false } }));
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free, "set": { "scale": "1:4" } }));
    let p = call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free }));
    assert_eq!(p["properties"]["scale_label"], json!(true), "{p}");
    call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free, "set": { "scale": "1:2", "view_label": true } }));
    let e = try_call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": free, "set": { "bogus": 1 } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.bogus"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_view_properties", json!({ "drawing": tab, "view": base, "set": { "rotation": 45 } })).unwrap_err();
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
    assert_eq!(sheets(&r), ["Sheet1", "Sheet1 Copy 1", "Details"], "{r}");
    assert_eq!(r["views"].as_array().unwrap().len(), d["views"].as_array().unwrap().len(), "the copy has the views: {r}");
    let r = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Details", "action": "move", "to": 1 }));
    assert_eq!(last(&mut ctx), "Move sheet");
    assert_eq!(sheets(&r), ["Details", "Sheet1", "Sheet1 Copy 1"], "{r}");
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
    assert_eq!(p["properties"]["reference"], Value::Null, "{p}");
    let p = call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties", "set": { "reference": "Unassigned" } }));
    assert_eq!(last(&mut ctx), "Sheet reference");
    assert_eq!(p["properties"]["reference"], "Unassigned", "{p}");
    let e = try_call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties", "set": { "reference": "Part 1" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.reference"), "{e:?}");
    call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "sheet": "Sheet1", "action": "properties", "set": { "reference": null } }));
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
    let e = edit(&mut ctx, json!({ "action": "show_location", "on": false })).unwrap_err();
    assert!(e.message.contains("hole tables"), "{e:?}");
    let e = edit(&mut ctx, json!({ "action": "merge_right", "row": 1, "column": 2 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("action"), "{e:?}");

    // A BOM table: type and order; no cell formats.
    let bt = call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "bom", "view": base, "position": [20, 280] }));
    // Its top right corner at x 20: off the sheet, and the reply says so.
    assert!(bt["warning"].as_str().is_some_and(|w| w.contains("TopRight")), "{bt}");
    let bid = bt["table"].clone();
    let t0 = table(&mut ctx, &bid);
    assert_eq!(t0["row_names"].as_array().map(|l| l.len()), t0["rows"].as_array().map(|l| l.len()), "a part name per row: {t0}");
    call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "bom_type", "to": "structured" }));
    assert_eq!(last(&mut ctx), "BOM type");
    call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "order", "to": "bottom_to_top" }));
    assert_eq!(last(&mut ctx), "BOM order");
    let t = table(&mut ctx, &bid);
    assert_eq!((t["structured"].clone(), t["bottom_to_top"].clone()), (json!(true), json!(true)), "{t}");
    assert_eq!(t["bom_type"], json!("structured_multi_level"), "{t}");
    // Structured - Top level (live: the dialog's third type), by the menu and by the properties.
    call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": bid, "action": "bom_type", "to": "structured_top_level" }));
    assert_eq!(table(&mut ctx, &bid)["bom_type"], json!("structured_top_level"));
    call(&mut ctx, "drawing_table_properties", json!({ "drawing": tab, "table": bid, "bom_type": "flattened" }));
    assert_eq!(table(&mut ctx, &bid)["bom_type"], json!("flattened"));
    let e = try_call(&mut ctx, "drawing_table_properties", json!({ "drawing": tab, "table": bid, "bom_type": "nested" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("bom_type"), "{e:?}");
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

/// Insert › Other documents through the API (dx's Split into multiple
/// documents builds on it): another document's Assembly, with the parts of
/// its own Part Studio, as one subassembly pinned to V1, and one part of
/// that document's Part Studio.
#[test]
fn assembly_inserts_another_documents_assembly_at_a_version() {
    let mut ctx = Ctx::new();
    two_part_assembly(&mut ctx, "other-doc-asm-lib.noshape");
    let lib = std::env::temp_dir().join(format!("noshape-ops-e2e-{}", std::process::id())).join("other-doc-asm-lib.noshape");
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let top = scratch("other-doc-asm-top.noshape");
    call(&mut ctx, "document_create", json!({ "path": top.to_str().unwrap() }));
    let e = try_call(&mut ctx, "assembly_insert", json!({ "source": "Assembly 1", "document": lib.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{e:?}");
    let e = try_call(&mut ctx, "assembly_insert", json!({ "source": "Assembly 1", "document": lib.to_str().unwrap(), "version": "V9" })).unwrap_err();
    assert!(e.param.as_deref() == Some("version") && e.message.contains("V1"), "{e:?}");
    let r = call(&mut ctx, "assembly_insert", json!({ "source": "Assembly 1", "document": lib.to_str().unwrap(), "version": "V1" }));
    assert_eq!(r["name"], "Assembly 1 <1>", "{r}");
    let sub = r["instances"].as_array().unwrap().iter().find(|i| i["id"] == r["instance"]).unwrap().clone();
    assert_eq!(sub["source"]["version_of"]["document"], "other-doc-asm-lib.noshape", "{sub}");
    assert!(sub["source"]["version_of"]["part"].is_null(), "{sub}");
    // The subassembly brings the other document's two parts.
    let bom = call(&mut ctx, "assembly_bom", json!({ "type": "flattened" }));
    let text = bom.to_string();
    assert!(text.contains("Part 1") && text.contains("Part 2"), "{bom}");
    // One part of the other document's Part Studio.
    let p = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2", "document": lib.to_str().unwrap(), "version": "V1" }));
    let pin = p["instances"].as_array().unwrap().iter().find(|i| i["id"] == p["instance"]).unwrap().clone();
    assert!(pin["source"]["version_of"]["part"].is_string(), "{pin}");
    let faces = call(&mut ctx, "assembly_entities", json!({ "instance": p["instance"], "kind": "faces" }));
    assert!(!faces["faces"].as_array().unwrap().is_empty(), "{faces}");
    let hist = call(&mut ctx, "history_list", json!({ "limit": 1 }));
    assert_eq!(hist["history"][0]["description"], "Assembly 1 :: Insert : Part Studio 1 (other document)", "{hist}");
}

/// Properties… of another document's part (live 2464): saved into the version the instance uses, so every use of
/// that version shows them, while the source's workspace keeps its own; Name and Not revision managed stay as they are.
#[test]
fn a_linked_parts_properties_save_into_its_version() {
    let mut ctx = Ctx::new();
    two_part_assembly(&mut ctx, "linked-props-lib.noshape");
    let lib = std::env::temp_dir().join(format!("noshape-ops-e2e-{}", std::process::id())).join("linked-props-lib.noshape");
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let top = scratch("linked-props-top.noshape");
    call(&mut ctx, "document_create", json!({ "path": top.to_str().unwrap() }));
    let own = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }));
    let e = try_call(&mut ctx, "assembly_linked_part_properties", json!({ "instance": own["instance"], "part_number": "X" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("instance"), "{e:?}");
    let p = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2", "document": lib.to_str().unwrap(), "version": "V1" }));
    let r = call(&mut ctx, "assembly_linked_part_properties", json!({ "instance": p["instance"], "part_number": "Q2464", "vendor": "Acme" }));
    assert_eq!(r["properties"]["part_number"], "Q2464", "{r}");
    assert_eq!(r["properties"]["name"], "Part 2", "{r}");
    let e = try_call(&mut ctx, "assembly_linked_part_properties", json!({ "instance": p["instance"], "not_revision_managed": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("not_revision_managed"), "{e:?}");
    // A second instance of that version shows them too.
    let q = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2", "document": lib.to_str().unwrap(), "version": "V1" }));
    let r = call(&mut ctx, "assembly_linked_part_properties", json!({ "instance": q["instance"] }));
    assert_eq!((r["properties"]["part_number"].as_str(), r["properties"]["vendor"].as_str()), (Some("Q2464"), Some("Acme")), "{r}");
    // Release… on it (live 2464, as a paid plan has it): the part at V1, linked to that version, its number there.
    let rel = call(&mut ctx, "release_create", json!({ "tab": "Assembly 1", "instance": p["instance"], "name": "R1" }));
    assert_eq!(rel["items"].as_array().map(Vec::len), Some(1), "{rel}");
    assert_eq!((rel["items"][0]["name"].as_str(), rel["items"][0]["part_number"].as_str(), rel["items"][0]["linked"]["version"].as_str()), (Some("Part 2"), Some("Q2464"), Some("V1")), "{rel}");
    // The source's workspace keeps its own (none).
    call(&mut ctx, "document_open", json!({ "path": lib.to_str().unwrap() }));
    let a = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2" }));
    let bom = call(&mut ctx, "assembly_bom", json!({ "type": "flattened" }));
    assert!(!bom.to_string().contains("Q2464"), "{a} {bom}");
}

/// Live 2468: a sheet metal part's flat lies where its fixed face (the first wall) puts it, here the plate's region on
/// Top, thickened up: z 0…2 with the plate kept at x 0…60, y 0…40 and the flange unrolled beside it. A sketch on its top
/// face is on the flat (z 2, normal +z) and shows only in the flat view; the flat is in no whole-part query.
#[test]
fn a_sketch_on_the_sheet_metal_flat() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "sheet-metal-flat-sketch.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [60, 40]] }));
    call(&mut ctx, "feature_add", json!({ "type": "sheetMetalStart", "params": { "process": "THICKEN", "regions": [r["regions"][0]["id"]], "thickness": "2 mm", "radius": "2 mm" } }));
    let e = call(&mut ctx, "query_entities", json!({ "kind": "edges", "direction": [1, 0, 0], "near": [30, 0, 2], "limit": 1 }))["edges"][0]["id"].clone();
    call(&mut ctx, "feature_add", json!({ "type": "sheetMetalFlange", "params": { "edges": [e], "distance": "20 mm" } }));
    let parts = call(&mut ctx, "query_parts", json!({}));
    assert_eq!(parts["parts"].as_array().map(Vec::len), Some(1), "the flat is no part: {parts}");
    let t = call(&mut ctx, "sheetmetal_table", json!({}));
    let faces = t["flat_faces"].as_array().expect("flat faces").clone();
    let num = |v: &Value| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64()).unwrap_or(f64::NAN);
    let top = faces.iter().filter(|f| num(&f["normal"][2]) > 0.999).max_by(|a, b| num(&a["area"]).total_cmp(&num(&b["area"]))).expect("a top face").clone();
    assert_eq!(top["part"], "Flat pattern of Part 1", "{top}");
    assert!((num(&top["offset"]) - 2.0).abs() < 1e-6, "the top face at z 2: {top}");
    let s = call(&mut ctx, "sketch_create", json!({ "plane": top["id"] }));
    assert_eq!(s["feature_status"]["status"], "Ok", "{s}");
    assert_eq!(s["on_flat"], true, "{s}");
    assert!((num(&s["frame"]["origin"][2]) - 2.0).abs() < 1e-6 && num(&s["frame"]["normal"][2]) > 0.999, "{s}");
}

/// Live 2464: Edit in context on another document's part stores the context in that document's Part Studio (its
/// Main); the banner's Create version and update in assembly versions that document and moves the instance there; a
/// pinned reference offers no Edit in context.
#[test]
fn edit_in_context_on_another_documents_part() {
    let mut ctx = Ctx::new();
    two_part_assembly(&mut ctx, "ctx-other-lib.noshape");
    let lib = std::env::temp_dir().join(format!("noshape-ops-e2e-{}", std::process::id())).join("ctx-other-lib.noshape");
    let libs = lib.to_str().unwrap().to_string();
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let top = scratch("ctx-other-top.noshape");
    let tops = top.to_str().unwrap().to_string();
    call(&mut ctx, "document_create", json!({ "path": tops }));
    let p = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2", "document": libs, "version": "V1" }));
    let inst = p["instance"].clone();
    let other = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1", "document": libs, "version": "V1" }))["instance"].clone();
    let r = call(&mut ctx, "assembly_edit_in_context", json!({ "doc": tops, "instance": inst }));
    assert_eq!(r["part_studio"], "Part Studio 1", "{r}");
    assert!(r["document"].as_str().is_some_and(|d| d.ends_with("ctx-other-lib.noshape")), "{r}");
    let listed = call(&mut ctx, "context_list", json!({ "doc": libs, "tab": "Part Studio 1" }));
    assert_eq!(listed["contexts"].as_array().map(Vec::len), Some(1), "the context is the other document's: {listed}");
    let mv_of = |ctx: &mut Ctx, id: &Value| call(ctx, "assembly_list", json!({ "doc": tops }))["instances"].as_array().unwrap().iter().find(|i| i["id"] == *id).unwrap()["source"]["version_of"]["microversion"].clone();
    let mv = |ctx: &mut Ctx| mv_of(ctx, &inst);
    let before = mv(&mut ctx);
    let u = call(&mut ctx, "assembly_version_and_update", json!({ "doc": tops, "instance": inst }));
    assert_eq!(u["version"], "V2", "{u}");
    // (Live 2475: its description; only this instance moves.)
    let lv = call(&mut ctx, "history_list", json!({ "doc": libs }))["versions"].as_array().unwrap().iter().find(|v| v["name"] == "V2").cloned().unwrap();
    assert_eq!(lv["description"], "Update version for edit in context", "{lv}");
    assert_eq!(mv_of(&mut ctx, &other), before, "Part 1 <1> (the same source) stays at V1");
    assert_ne!(mv(&mut ctx), before, "the instance moved to V2");
    let last = call(&mut ctx, "history_list", json!({ "doc": tops, "limit": 1 }))["history"][0]["description"].clone();
    assert_eq!(last, "Assembly 1 :: Update to V2");
    call(&mut ctx, "assembly_pin_reference", json!({ "doc": tops, "instance": inst }));
    let e = try_call(&mut ctx, "assembly_edit_in_context", json!({ "doc": tops, "instance": inst })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("instance"), "{e:?}");
}

/// A callout on a part's silhouette is that part's, not the part of an edge
/// a little way off (qa BALLOON-WRONG-ITEM: a click on a pin's silhouette
/// near the plate's edge showed the plate's item).
#[test]
fn callout_on_a_silhouette_takes_its_part() {
    let mut ctx = Ctx::new();
    let (_, pin) = two_part_assembly(&mut ctx, "callout_silhouette.noshape");
    // The pin stands on the plate.
    call(&mut ctx, "assembly_transform", json!({ "instance": pin, "translation": [-50, 10, 20] }));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Assembly 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let iso = g["views"].as_array().unwrap().iter().find(|v| v["edges"].as_array().is_some_and(|es| es.iter().any(|e| e["type"] == "silhouette"))).cloned().expect("a view with silhouettes");
    let segs = |e: &Value| -> Vec<([f64; 2], [f64; 2])> {
        let p = |v: &Value| [num(&v[0]), num(&v[1])];
        match e["type"].as_str() {
            Some("line" | "silhouette") => vec![(p(&e["a"]), p(&e["b"]))],
            _ => e["points"].as_array().map(|ps| ps.windows(2).map(|w| (p(&w[0]), p(&w[1]))).collect()).unwrap_or_default(),
        }
    };
    let dist = |q: [f64; 2], (a, b): ([f64; 2], [f64; 2])| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let t = (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-12)).clamp(0.0, 1.0);
        ((a[0] + t * dx - q[0]).powi(2) + (a[1] + t * dy - q[1]).powi(2)).sqrt()
    };
    let edges = iso["edges"].as_array().unwrap();
    let near = |q: [f64; 2], kind: &str| edges.iter().filter(|e| e["type"] == kind).flat_map(&segs).map(|s| dist(q, s)).fold(f64::INFINITY, f64::min);
    // A point of a silhouette 0.12–0.28 mm from a (plate) line: within the
    // pick tolerance of both, on the pin.
    let at = edges
        .iter()
        .filter(|e| e["type"] == "silhouette")
        .flat_map(&segs)
        .flat_map(|(a, b)| (0..=400).map(move |k| [a[0] + (b[0] - a[0]) * k as f64 / 400.0, a[1] + (b[1] - a[1]) * k as f64 / 400.0]))
        .find(|&q| (0.12..0.28).contains(&near(q, "line")) && near(q, "curve") > 0.3)
        .expect("a silhouette point near a plate edge");
    let c = call(&mut ctx, "drawing_callout", json!({ "drawing": tab, "anchor": at, "position": [at[0] + 30.0, at[1] + 30.0], "text": "{part.name}" }));
    let text = c["annotations"].as_array().unwrap().iter().find(|a| a["id"] == c["annotation"]).unwrap()["text"].clone();
    assert_eq!(text, "Part 2", "the pin, at {at:?}");
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
    // The banner's Hide/show instances (view_display context_hidden).
    let pin_name = c["instances"][0]["instance"].as_str().unwrap().to_string();
    let d = call(&mut ctx, "view_display", json!({ "tab": "Part Studio 1", "context_hidden": [pin_name] }));
    assert_eq!(d["context_hidden"], json!([pin]), "{d}");
    let d = call(&mut ctx, "view_display", json!({ "tab": "Part Studio 1", "context_hidden": [] }));
    assert_eq!(d["context_hidden"], json!([]), "{d}");
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
    assert_eq!(cs[0]["follow_position"], "Current assembly position", "{cs:?}");

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

/// qa5 CTX-USE: Use on the context face a sketch lies on projects its
/// edges (live: the face's outer edges), as on an ordinary face.
#[test]
fn use_on_the_context_face_a_sketch_lies_on() {
    let mut ctx = Ctx::new();
    let (block, _pin) = two_part_assembly(&mut ctx, "ctx_use.noshape");
    call(&mut ctx, "assembly_edit_in_context", json!({ "instance": block }));
    let faces = call(&mut ctx, "query_entities", json!({ "tab": "Part Studio 1", "kind": "faces", "context": true, "surface": "plane", "normal": [0, 0, 1] }));
    let top = faces["faces"][0]["id"].as_str().unwrap_or_else(|| panic!("{faces}")).to_string();
    let sk = call(&mut ctx, "sketch_create", json!({ "tab": "Part Studio 1", "plane": top, "name": "On pin" }))["sketch"].as_str().unwrap().to_string();
    let r = try_call(&mut ctx, "sketch_use", json!({ "tab": "Part Studio 1", "sketch": sk, "references": [top] }));
    assert!(r.is_ok(), "{r:?}");
    let got = call(&mut ctx, "sketch_get", json!({ "tab": "Part Studio 1", "sketch": sk }));
    assert!(!got["entities"].as_array().map(|e| e.is_empty()).unwrap_or(true), "{got}");
}

/// The annotation menus: jogs, break, leaders and note nodes, foreshorten,
/// groups, align centermark, stacked callouts, add to an ordinate
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

    // A linear dimension: jog, break.
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

    // Radius: foreshorten (live: only with its text inside the arc) and back.
    let outside = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "radius", "picks": [[cx + r, cy]], "place": [cx + r + 15.0, cy + 15.0] }))["annotation"].clone();
    let e = edit(&mut ctx, &outside, json!({ "action": "foreshorten", "point": [cx + r + 30.0, cy] })).unwrap_err();
    assert!(e.message.contains("inside the arc"), "{e:?}");
    let rad = call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "kind": "radius", "picks": [[cx + r, cy]], "place": [cx + r * 0.4, cy] }))["annotation"].clone();
    let e = edit(&mut ctx, &rad, json!({ "action": "foreshorten" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("point"), "{e:?}");
    let jog = [cx - r * 3.0, cy + 4.0];
    edit(&mut ctx, &rad, json!({ "action": "foreshorten", "point": jog })).unwrap();
    assert_eq!(last(&mut ctx), "Foreshorten");
    assert_eq!(ann(&mut ctx, &rad)["kind"], "ForeshortenedRadial");
    edit(&mut ctx, &rad, json!({ "action": "remove_foreshorten" })).unwrap();
    assert_eq!(last(&mut ctx), "Remove foreshorten");
    let a = ann(&mut ctx, &rad);
    assert_eq!(a["kind"], "RadialDimension");
    // The text on the straight leader, level with where the jog ended.
    let at = [num(&a["position"][0]), num(&a["position"][1])];
    assert!((at[0] - jog[0]).abs() < 0.5, "{at:?} {jog:?}");

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
    // Configure appearance: a row for every configuration of the first list.
    let cfg = call(&mut ctx, "config_list", json!({ "tab": "Part Studio 1" }));
    let options = cfg["inputs"].as_array().unwrap().iter().find(|i| i["kind"] == "list").map_or(1, |i| i["configurations"].as_array().unwrap().len());
    let t = call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": part, "action": "configure_property", "property": "Appearance" }));
    assert_eq!(t["table"]["properties"], json!(["Appearance"]), "{t}");
    assert_eq!(t["table"]["rows"].as_array().unwrap().len(), options, "{t}");
    call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": part, "action": "delete_table" }));

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
    // Dragging the letter box away (and back).
    let id = r["annotations"][0]["id"].clone();
    let r = call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "move", "annotation": id, "offset": [40, -12.5] }));
    assert_eq!(last(&mut ctx), "Move annotation");
    assert_eq!(r["annotations"][0]["offset"], json!([40.0, -12.5]), "{r}");
    let r = call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "move", "annotation": id, "offset": [0, 0] }));
    assert!(r["annotations"][0].get("offset").is_none(), "{r}");
    let e = try_call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "move", "annotation": 99, "offset": [1, 1] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("annotation"), "{e:?}");
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

/// An Assembly exported as Parasolid keeps its structure (qa
/// ASM-PARASOLID-FLAT; live's x_t of an assembly re-imports as an Assembly
/// and a "<file> parts" Part Studio): each part once, placed by its instances.
#[test]
fn parasolid_assembly_export_and_import_round_trip() {
    let mut ctx = Ctx::new();
    let (_, pin) = two_part_assembly(&mut ctx, "xt_asm.noshape");
    call(&mut ctx, "assembly_transform", json!({ "instance": pin, "translation": [0, 0, 30], "axis": [0, 0, 1], "angle": "90 deg" }));
    let list = call(&mut ctx, "assembly_list", json!({}));
    let before = list["instances"].as_array().unwrap().iter().find(|i| i["id"] == pin).unwrap().clone();
    let file = scratch("xt_asm_out.x_t");
    let e = call(&mut ctx, "export_parts", json!({ "tab": "Assembly 1", "format": "PARASOLID", "file": file.to_str().unwrap() }));
    assert_eq!(e["parts"], 2, "{e}");
    let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
    let tabs: Vec<(String, String)> = r["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| (t["name"].as_str().unwrap().to_string(), t["kind"].as_str().unwrap().to_string())).collect();
    let asm = tabs.iter().find(|t| t.1 == "Assembly").unwrap_or_else(|| panic!("an Assembly: {r}")).0.clone();
    assert!(tabs.contains(&("xt_asm_out parts".to_string(), "Part Studio".to_string())), "{r}");
    // Named as in the assembly (qa ASM-XT-ROUNDTRIP-NAMES).
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "xt_asm_out parts" }));
    let mut names: Vec<&str> = parts["parts"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Part 1", "Part 2"], "{parts}");
    let inst = call(&mut ctx, "assembly_list", json!({ "tab": asm }))["instances"].as_array().unwrap().clone();
    assert_eq!(inst.len(), 2, "{inst:?}");
    let moved = |i: &Value| (0..3).all(|k| (num(&i["translation"][k]) - num(&before["translation"][k])).abs() < 1e-6);
    assert!(inst.iter().any(moved), "the pin where the assembly had it: {inst:?} vs {before}");
    // Named after their parts (ASM-XT-ROUNDTRIP-NAMES: "<1>", "<2>").
    let mut names: Vec<&str> = inst.iter().map(|i| i["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Part 1 <1>", "Part 2 <1>"], "{inst:?}");
}

/// An Assembly exported as IGES keeps its structure as live's does (1755:
/// each part once as a subfigure, placed by its instances; it re-imports as
/// an Assembly and a "<file> parts" Part Studio).
#[test]
fn iges_assembly_export_and_import_round_trip() {
    let mut ctx = Ctx::new();
    let (_, pin) = two_part_assembly(&mut ctx, "igs_asm.noshape");
    let pin_part = call(&mut ctx, "query_parts", json!({}))["parts"][1]["id"].clone();
    let pin2 = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": pin_part }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_transform", json!({ "instance": pin2, "translation": [0, 0, 30], "axis": [0, 0, 1], "angle": "90 deg" }));
    let list = call(&mut ctx, "assembly_list", json!({}));
    let before: Vec<Value> = list["instances"].as_array().unwrap().iter().filter(|i| i["id"] == pin || i["id"] == pin2).cloned().collect();
    let file = scratch("igs_asm_out.igs");
    let e = call(&mut ctx, "export_parts", json!({ "tab": "Assembly 1", "format": "IGES", "file": file.to_str().unwrap() }));
    assert_eq!(e["parts"], 3, "{e}");
    let text = std::fs::read_to_string(&file).unwrap();
    let count = |t: &str| text.lines().filter(|l| &l[72..73] == "D" && l[..8].trim() == t).count() / 2;
    assert_eq!((count("186"), count("308"), count("408")), (2, 2, 3), "each part once, each instance a 408");
    let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
    let tabs: Vec<(String, String)> = r["tabs"].as_array().unwrap().iter().filter(|t| t["kind"] != "File").map(|t| (t["name"].as_str().unwrap().to_string(), t["kind"].as_str().unwrap().to_string())).collect();
    let asm = tabs.iter().find(|t| t.1 == "Assembly").unwrap_or_else(|| panic!("an Assembly: {r}")).0.clone();
    assert!(tabs.contains(&("igs_asm_out parts".to_string(), "Part Studio".to_string())), "{r}");
    let parts = call(&mut ctx, "query_parts", json!({ "tab": "igs_asm_out parts" }));
    let mut names: Vec<&str> = parts["parts"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Part 1", "Part 2"], "{parts}");
    let inst = call(&mut ctx, "assembly_list", json!({ "tab": asm }))["instances"].as_array().unwrap().clone();
    assert_eq!(inst.len(), 3, "{inst:?}");
    for b in &before {
        let at = |i: &Value| (0..3).all(|k| (num(&i["translation"][k]) - num(&b["translation"][k])).abs() < 1e-6);
        assert!(inst.iter().any(at), "a pin where the assembly had it: {inst:?} vs {b}");
    }
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
    // The base's thickness: the bottom line (y 0) to the top of the base (y 6).
    let line = |from: [f64; 2], to: [f64; 2]| s["created"].as_array().unwrap().iter().find(|e| e["type"] == "line" && e["start"] == json!(from) && e["end"] == json!(to)).unwrap_or_else(|| panic!("no line {from:?}-{to:?}: {s}"))["id"].clone();
    let d = call(&mut ctx, "sketch_dimension", json!({ "sketch": "Sketch 1", "kind": "distance", "entities": [line([0.0, 0.0], [50.0, 0.0]), line([44.0, 6.0], [0.0, 6.0])], "value": "6 mm" }));
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
    // Views copied in another document or workspace are refused (live V53).
    let mut other: Value = serde_json::from_str(c["clipboard"].as_str().unwrap()).unwrap();
    let origin = other["origin"].clone();
    assert!(origin[0].as_str().unwrap().ends_with("drawing_more.noshape"), "{origin}");
    for (o, m) in [(json!(["/elsewhere.noshape", origin[1]]), "Views could not be copied between documents."), (json!([origin[0], "another"]), "Views could not be copied between workspaces.")] {
        other["origin"] = o;
        let e = try_call(&mut ctx, "drawing_paste", json!({ "drawing": tab, "position": [100, 70], "clipboard": other.to_string() })).unwrap_err();
        assert!(format!("{e:?}").contains(m), "{e:?}");
    }

    // Change configuration… on the reference: every base view of it (live S6).
    call(&mut ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "list", "name": "Size" }));
    let large = call(&mut ctx, "config_add_option", json!({ "tab": "Part Studio 1", "input": "Size", "name": "Large" }))["option"].clone();
    call(&mut ctx, "drawing_change_configuration", json!({ "drawing": tab, "view": second, "configuration": { "Size": "Large" } }));
    assert!(last(&mut ctx).ends_with(":: Change configuration"));
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let bases: Vec<&Value> = g["views"].as_array().unwrap().iter().filter(|v| v["kind"] == "base").collect();
    assert!(bases.len() >= 3, "{g}");
    for v in bases {
        assert_eq!(v["configuration"].as_object().map(|o| o.values().cloned().collect::<Vec<_>>()), Some(vec![large.clone()]), "{v}");
    }

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
    // Paste as table from a spreadsheet's HTML: merged cells and column
    // widths come along (drawing.md TB12).
    let html = r#"<table><colgroup><col width="100"><col width="60"></colgroup><tr style="height:30px"><td colspan="2" style="font-weight:bold">Parts</td></tr><tr style="height:21px"><td>Bolt</td><td>4</td></tr></table>"#;
    let ph = call(&mut ctx, "drawing_paste_table", json!({ "drawing": tab, "html": html, "text": "Parts\nBolt\t4", "position": [60, 280] }));
    let hid = ph["table"].clone();
    let h = ph["tables"].as_array().unwrap().iter().find(|t| t["id"] == hid).unwrap().clone();
    assert_eq!(h["rows"], json!([["Parts", ""], ["Bolt", "4"]]), "{h}");
    assert_eq!(h["merges"], json!([{ "row": 0, "column": 0, "rows": 1, "columns": 2 }]), "{h}");
    let widths: Vec<f64> = h["column_widths"].as_array().unwrap().iter().map(num).collect();
    // Live 1727: px as mm, 1:1.
    assert!((widths[0] - 100.0).abs() < 0.01 && (widths[1] - 60.0).abs() < 0.01, "{h}");
    let e = try_call(&mut ctx, "drawing_paste_table", json!({ "drawing": tab, "html": "<p>x</p>", "position": [60, 280] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("html"), "{e:?}");
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
    // variable_set by `name` (as variable_create names it), then `new_name`.
    call(&mut ctx, "variable_set", json!({ "name": "w", "value": "25 mm" }));
    call(&mut ctx, "variable_set", json!({ "variable": "#w", "new_name": "width" }));
    let v = call(&mut ctx, "variable_list", json!({}));
    assert!(v.to_string().contains("#width") && v.to_string().contains("25"), "{v}");
}

/// The line tool's Line/tangent arc switch (Shift+A while chaining): the
/// next segment is a tangent arc, then the chain goes on with lines.
#[test]
fn line_tangent_arc_switch() {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("tangent_arc_switch.noshape").to_str().unwrap() }));
    call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "line", "points": [[0, 0], [20, 0], [30, 10], [30, 30]], "tangent_arc_switch": [2] }));
    let kinds: Vec<&str> = d["created"].as_array().unwrap().iter().map(|e| e["type"].as_str().unwrap()).filter(|t| *t != "point").collect();
    assert_eq!(kinds, ["line", "arc", "line"], "{d}");
    let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "circle", "points": [[50, 0], [55, 0]], "tangent_arc_switch": [1] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tangent_arc_switch"), "{e:?}");
}

/// The graphics area's camera through the view menus (view_camera), read
/// back by ui_state and drawn by render_image `view: "window"`.
#[test]
fn view_camera_menus() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "view_camera.noshape");
    let cam = |ctx: &mut Ctx, args: Value| call(ctx, "view_camera", args);
    let c = cam(&mut ctx, json!({ "action": "get" }));
    assert_eq!((c["standard_view"].clone(), c["previous_views"].clone()), (json!("trimetric"), json!(0)), "a graphics area opens in Trimetric: {c}");
    let iso = cam(&mut ctx, json!({ "action": "view" }));
    assert_eq!((iso["standard_view"].clone(), iso["previous_views"].clone()), (json!("isometric"), json!(1)), "{iso}");
    let h = num(&iso["visible_height"]);
    let zin = cam(&mut ctx, json!({ "action": "zoom_in" }));
    assert!((num(&zin["visible_height"]) - h / 1.75).abs() < 1e-6 * h, "Z zooms by 1.75: {zin}");
    let back = cam(&mut ctx, json!({ "action": "previous" }));
    assert!((num(&back["visible_height"]) - h).abs() < 1e-9 * h, "{back}");
    // View normal to the Top plane; again: from the other side.
    let n = cam(&mut ctx, json!({ "action": "normal_to", "refs": ["plane:top"] }));
    assert!((num(&n["view_direction"][2]) + 1.0).abs() < 1e-9, "{n}");
    let n = cam(&mut ctx, json!({ "action": "normal_to", "refs": ["plane:top"] }));
    assert!((num(&n["view_direction"][2]) - 1.0).abs() < 1e-9, "{n}");
    // Live: the back of Top keeps Y up, X to the left; the cube shows Bottom.
    assert_eq!((n["standard_view"].clone(), n["up"][1].as_f64().map(f64::round)), (json!("bottom"), Some(1.0)), "{n}");
    // Align view with vertical axis: an X edge points up the screen.
    let xedge = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "line" }))["edges"].as_array().unwrap().iter().find(|e| num(&e["direction"][0]).abs() > 0.99).unwrap()["id"].as_str().unwrap().to_string();
    let al = cam(&mut ctx, json!({ "action": "align_vertical", "refs": [xedge] }));
    assert!(num(&al["up"][0]).abs() > 1.0 - 1e-9, "{al}");
    // Zoom to window: a box a quarter of the 800 px height.
    let before = num(&al["visible_height"]);
    let w = cam(&mut ctx, json!({ "action": "zoom_window", "box": [[540, 300], [740, 500]] }));
    assert!((num(&w["visible_height"]) - before / 4.0).abs() < 1e-6 * before, "{w}");
    let e = try_call(&mut ctx, "view_camera", json!({ "action": "zoom_window", "box": [[10, 10], [12, 12]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("box"), "{e:?}");
    // Zoom to selection frames the part closer than Zoom to fit (the planes too).
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    let z = cam(&mut ctx, json!({ "action": "zoom_selection", "refs": [part] }));
    let f = cam(&mut ctx, json!({ "action": "zoom_fit" }));
    assert!(num(&z["visible_height"]) < num(&f["visible_height"]), "{z} {f}");
    let p = cam(&mut ctx, json!({ "action": "perspective", "on": true }));
    assert_eq!(p["projection"], "perspective", "{p}");
    let ui = call(&mut ctx, "ui_state", json!({}));
    assert_eq!(ui["cameras"][0]["projection"], "perspective", "{ui}");
    let img = call(&mut ctx, "render_image", json!({ "view": "window", "width": 64, "height": 40 }));
    assert_eq!(img["view"], "window", "{img}");
    // Without a size: the graphics area's (1280 × 800); one side keeps its aspect.
    let img = call(&mut ctx, "render_image", json!({ "view": "window" }));
    assert_eq!((img["width"].as_u64(), img["height"].as_u64()), (Some(1280), Some(800)), "{img}");
    let img = call(&mut ctx, "render_image", json!({ "view": "window", "width": 320 }));
    assert_eq!((img["width"].as_u64(), img["height"].as_u64()), (Some(320), Some(200)), "{img}");
    // A drawing: Zoom in doubles the zoom; Zoom to fit restores it.
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let fit = cam(&mut ctx, json!({ "tab": tab, "action": "zoom_fit" }));
    let zi = cam(&mut ctx, json!({ "tab": tab, "action": "zoom_in" }));
    assert!((num(&zi["zoom"]) - 2.0 * num(&fit["zoom"])).abs() < 1e-9, "{zi}");
    let again = cam(&mut ctx, json!({ "tab": tab, "action": "zoom_sheet" }));
    assert_eq!(again["zoom"], fit["zoom"], "{again}");
}

/// Display settings kept by the window (view_display): isolate,
/// transparent, boundary edges; render_image `view: "window"` uses them.
#[test]
fn view_display_settings() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "view_display.noshape");
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    let d = call(&mut ctx, "view_display", json!({ "isolate": ["Part 1"] }));
    assert_eq!((d["isolate"].clone(), d["transparent"].clone()), (json!([part]), Value::Null), "{d}");
    let d = call(&mut ctx, "view_display", json!({ "transparent": [part], "highlight_boundary": true }));
    assert_eq!((d["isolate"].clone(), d["transparent"].clone(), d["highlight_boundary"].clone()), (Value::Null, json!([part]), json!(true)), "{d}");
    let e = try_call(&mut ctx, "view_display", json!({ "show_mates": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("show_mates"), "an Assembly's: {e:?}");
    let e = try_call(&mut ctx, "view_display", json!({ "isolate": ["nope"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("isolate[0]"), "{e:?}");
    let ui = call(&mut ctx, "ui_state", json!({}));
    assert_eq!(ui["display"][0]["transparent"], json!([part]), "{ui}");
    call(&mut ctx, "view_camera", json!({ "action": "zoom_fit" }));
    call(&mut ctx, "render_image", json!({ "view": "window", "width": 64, "height": 40 }));
    let d = call(&mut ctx, "view_display", json!({ "exit_isolate": true }));
    assert_eq!(d["transparent"], Value::Null, "{d}");
}

/// The window's selection (ui_select): set, Select ▸ related, Select
/// other… under a point, Clear selection; Zoom to selection uses it.
#[test]
fn ui_select_related_and_select_other() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "ui_select.noshape");
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    let s = call(&mut ctx, "ui_select", json!({ "action": "set", "refs": [part] }));
    assert_eq!(s["selected"], json!([part]), "{s}");
    let z = call(&mut ctx, "view_camera", json!({ "action": "zoom_selection" }));
    assert_eq!(z["previous_views"], 1, "zooms to the selection: {z}");
    // Select ▸ All faces of part, from one face.
    let face = call(&mut ctx, "query_entities", json!({ "kind": "faces" }))["faces"][0]["id"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "ui_select", json!({ "action": "related", "refs": [face], "criterion": "All faces of part" }));
    assert!(r["selected"].as_array().unwrap().len() >= 7, "a block with a hole: {r}");
    // Select other… looking down at the block 20 mm right of its centre:
    // the top face, then the bottom face.
    let n = call(&mut ctx, "view_camera", json!({ "action": "normal_to", "refs": ["plane:top"] }));
    let ppm = 800.0 / num(&n["visible_height"]);
    let at = json!([640.0 + 20.0 * ppm, 400.0]);
    let o = call(&mut ctx, "ui_select", json!({ "action": "select_other", "at": at }));
    assert_eq!(o["under"].as_array().unwrap().len(), 2, "{o}");
    let o = call(&mut ctx, "ui_select", json!({ "action": "select_other", "at": at, "index": 1 }));
    assert_eq!(o["selected"], json!([o["under"][1].clone()]), "{o}");
    // Create selection… adds what it finds to the selection.
    let cs = call(&mut ctx, "ui_select", json!({ "action": "create_selection", "refs": [face], "criterion": "Tangent connected faces" }));
    let sel = cs["selected"].as_array().unwrap();
    assert!(sel.len() >= 2 && sel.contains(&o["under"][1]) && sel.contains(&json!(face)), "{cs}");
    let c = call(&mut ctx, "ui_select", json!({ "action": "clear" }));
    assert_eq!(c["selected"], json!([]), "{c}");
    // A drawing's MBD status: no model datums here.
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let m = call(&mut ctx, "drawing_mbd", json!({ "drawing": d["drawing"] }));
    assert_eq!(m["rows"], json!([]), "{m}");
    assert_eq!(try_call(&mut ctx, "drawing_mbd", json!({ "drawing": d["drawing"], "show": "Datum A" })).unwrap_err().param.as_deref(), Some("show"));
    let e = try_call(&mut ctx, "view_camera", json!({ "action": "zoom_selection" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("refs"), "nothing selected: {e:?}");
}

/// Search tools… (ui_search_tools): Onshape's ranking of the Part Studio's
/// tools, with their feature types and keys.
#[test]
fn ui_search_tools_ranks_like_onshape() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "search_tools.noshape");
    let t = call(&mut ctx, "ui_search_tools", json!({ "text": "fil" }));
    let names: Vec<&str> = t["tools"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(&names[..2], ["Fillet", "Fill"], "{t}");
    assert_eq!((t["tools"][0]["type"].clone(), t["tools"][0]["keys"].clone()), (json!("fillet"), json!("shift-f")), "{t}");
    assert!(call(&mut ctx, "ui_search_tools", json!({ "text": "" }))["tools"].as_array().unwrap().is_empty());
    // In a sketch: the sketch tools only, by their plain names (live qa5-ts-sketch-*).
    let s = call(&mut ctx, "ui_search_tools", json!({ "text": "chamfer", "sketch": true }));
    assert_eq!(s["tools"], json!([{ "name": "Chamfer", "keys": null }]), "{s}");
    let s = call(&mut ctx, "ui_search_tools", json!({ "text": "fillet", "sketch": true }));
    assert_eq!(s["tools"][0], json!({ "name": "Fillet", "keys": "Shift+F" }), "{s}");
}

/// Clear missing references (feature_clear_missing): a fillet whose edges'
/// extrude is suppressed loses them in one edit.
#[test]
fn clear_missing_references() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "clear_missing.noshape");
    let edge = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "line" }))["edges"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": [edge], "radius": "2 mm" } }));
    let e = try_call(&mut ctx, "feature_clear_missing", json!({ "feature": "Fillet 1" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("feature"), "nothing missing yet: {e:?}");
    call(&mut ctx, "feature_suppress", json!({ "features": ["Extrude 1"], "suppressed": true }));
    let c = call(&mut ctx, "feature_clear_missing", json!({ "feature": "Fillet 1" }));
    assert_eq!(c["removed"], 1, "{c}");
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(last.ends_with(":: Edit : Fillet 1"), "{last}");
}

/// Benchmark 2's fixes (P1 workflows through the API): selections as
/// queries, a feature pattern from its features, configured queries,
/// ops_list by a group's longer name, a short feature_schema.
#[test]
fn agent_benchmark_p1_fixes() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "bench_p1.noshape");
    // The four vertical edges by a query, no ids copied.
    let fi = call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": { "query": { "kind": "edges", "direction": [0, 0, 1], "curve": "line" } }, "radius": "3 mm" } }));
    assert_eq!(fi["feature_status"]["status"], "Ok", "{fi}");
    let g = call(&mut ctx, "feature_get", json!({ "feature": "Fillet 1" }));
    let q = call(&mut ctx, "query_entities", json!({ "kind": "edges", "direction": [0, 0, 1], "curve": "line" }));
    // The four corners (seams aren't entities).
    assert_eq!(g["params"]["entities"].as_array().unwrap().len(), 4, "{g} {q}");
    let e = try_call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": { "query": { "kind": "edges", "curve": "ellipse" } }, "radius": "1 mm" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("params.entities.query"), "{e:?}");
    // `near` ranks by the distance to the face, not its centroid: a point on
    // the top face near a corner finds the top face, not the closer-centred side.
    let n = call(&mut ctx, "query_entities", json!({ "kind": "faces", "near": [36, 16, 20], "limit": 1 }));
    assert!((num(&n["faces"][0]["centroid"][2]) - 20.0).abs() < 1e-6, "{n}");
    // A feature pattern from `entities` that are features.
    let x = call(&mut ctx, "query_entities", json!({ "kind": "edges", "direction": [1, 0, 0], "curve": "line" }))["edges"][0]["id"].clone();
    let ex = call(&mut ctx, "feature_list", json!({}))["features"].as_array().unwrap().iter().find(|f| f["name"] == "Extrude 1").unwrap()["id"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "feature_add", json!({ "type": "linearPattern", "params": { "entities": [format!("feature:{ex}")], "directionOne": [x], "distance": "100 mm", "instanceCount": 2 } }));
    assert_eq!(p["feature_status"]["status"], "Ok", "{p}");
    let g = call(&mut ctx, "feature_get", json!({ "feature": "Linear pattern 1" }));
    assert_eq!(g["params"]["patternType"], "FEATURE", "{g}");
    // Configured queries: by input and option names.
    call(&mut ctx, "config_add_input", json!({ "kind": "list", "name": "Size" }));
    call(&mut ctx, "config_add_option", json!({ "input": "Size", "name": "Thick" }));
    call(&mut ctx, "config_configure", json!({ "feature": "Extrude 1", "param": "depth", "input": "Size" }));
    call(&mut ctx, "config_set_value", json!({ "feature": "Extrude 1", "param": "depth", "configuration": "Thick", "value": "40 mm" }));
    let vol = |v: &Value| v["parts"].as_array().unwrap().iter().map(|p| num(&p["volume"])).sum::<f64>();
    let thin = call(&mut ctx, "query_parts", json!({}));
    let thick = call(&mut ctx, "query_parts", json!({ "configuration": { "Size": "Thick" } }));
    assert!((vol(&thick) - 2.0 * vol(&thin)).abs() < 0.02 * vol(&thin), "twice the depth: {thin} {thick}");
    let m = call(&mut ctx, "query_mass_properties", json!({ "configuration": { "Size": "Thick" } }));
    assert!(m.to_string().contains("volume"), "{m}");
    // ops_list: a group by its longer name; an unknown one names the groups.
    let l = call(&mut ctx, "ops_list", json!({ "group": "configuration" }));
    assert!(l["operations"].as_array().unwrap().iter().any(|o| o["tool"] == "config_add_input"), "{l}");
    let e = try_call(&mut ctx, "ops_list", json!({ "group": "nope" })).unwrap_err();
    assert!(e.to_json().to_string().contains("config"), "{e:?}");
    let s = call(&mut ctx, "ops_list", json!({ "search": "pattern assembly" }));
    assert!(s["operations"].as_array().unwrap().iter().all(|o| o["tool"].as_str().unwrap().starts_with("assembly")), "{s}");
    // An unknown tool's error names the close ones; one `part` for set_material.
    let e = try_call(&mut ctx, "ops_describe", json!({ "tools": ["variable_edit"] })).unwrap_err();
    assert!(e.message.contains("variable_set"), "{e:?}");
    // Ranked by the name's words, the rarer first.
    for (wrong, first) in [("mate_add", "assembly_mate"), ("version_create", "history_create_version")] {
        let e = try_call(&mut ctx, "ops_describe", json!({ "tools": [wrong] })).unwrap_err();
        assert!(e.message.contains(&format!("did you mean: {first},")), "{wrong}: {}", e.message);
    }
    let m = call(&mut ctx, "part_set_material", json!({ "part": "Part 1", "material": "Aluminum - 6061" }));
    assert!(!m.to_string().contains("error"), "{m}");
    // feature_schema: what the dialog shows; the rest by id.
    let h = call(&mut ctx, "feature_schema", json!({ "type": "hole" }));
    assert!(h["params"]["properties"].get("holeDiameterV3").is_some() && h["params"]["properties"].get("cBoreDiameterV3").is_none(), "{h}");
    assert!(h["more_when_shown"].as_array().unwrap().contains(&json!("cBoreDiameterV3")), "{h}");
    let all = call(&mut ctx, "feature_schema", json!({ "type": "hole", "all": true }));
    assert!(all["params"]["properties"].get("cBoreDiameterV3").is_some());
}

/// A STEP assembly whose two parts share a name imports with distinct
/// instance names ("Part 1 <1>", "Part 1 <2>"), as instance names count.
#[test]
fn step_assembly_same_named_parts_get_distinct_instances() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "step_names.noshape");
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio" }));
    let sk = call(&mut ctx, "sketch_create", json!({ "tab": "Part Studio 2", "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "sketch_draw", json!({ "tab": "Part Studio 2", "sketch": sk, "tool": "circle", "points": [[0, 0], [5, 0]] }));
    call(&mut ctx, "feature_add", json!({ "tab": "Part Studio 2", "type": "extrude", "params": { "entities": [r["regions"][0]["id"]], "depth": "30 mm" } }));
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
    let pin = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 2", "part": "Part 1" }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_transform", json!({ "instance": pin, "translation": [60, 0, 0] }));
    let step = scratch("distinct_instances.step");
    call(&mut ctx, "export_parts", json!({ "tab": "Assembly 1", "format": "STEP", "file": step.to_str().unwrap() }));
    call(&mut ctx, "document_create", json!({ "path": scratch("same_names_import.noshape").to_str().unwrap() }));
    let imp = call(&mut ctx, "import_files", json!({ "files": [step.to_str().unwrap()] }));
    let asm = imp["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Assembly").unwrap()["name"].as_str().unwrap().to_string();
    // The import is the window's tab: tools without `tab` work on it.
    let l = call(&mut ctx, "assembly_list", json!({}));
    assert_eq!(call(&mut ctx, "ui_state", json!({}))["active_tab"]["name"], json!(asm));
    let mut names: Vec<&str> = l["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Part 1 <1>", "Part 1 <2>"], "{l}");
}

/// Edits the app makes that the API lacked (the audit of its history texts):
/// the instance list's drops (Reorder instances, Add to folder, Move to
/// subassembly), Reorder : Explode steps, the Mirror strategy (Derived
/// copies from the "-Mirrored" Part Studio) and an image's Move to sheet /
/// Image borders.
#[test]
fn instance_drops_explode_reorder_mirror_strategy_and_image_menu() {
    let mut ctx = Ctx::new();
    let (b, p) = two_part_assembly(&mut ctx, "drops_mirror.noshape");
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let names = |v: &Value| v["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    // Reorder: the pin dropped above the block.
    let before = names(&call(&mut ctx, "assembly_list", json!({})));
    let r = call(&mut ctx, "assembly_move_instance", json!({ "instances": [p], "before": b }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Reorder instances");
    assert_eq!(names(&r), vec![before[1].clone(), before[0].clone()], "{r}");
    let r = call(&mut ctx, "assembly_move_instance", json!({ "instances": [p], "before": "end" }));
    assert_eq!(names(&r), before, "{r}");
    let e = try_call(&mut ctx, "assembly_move_instance", json!({ "instances": [p], "before": b, "folder": "x" })).unwrap_err();
    assert!(e.message.contains("one of"), "{e:?}");
    // Add to folder: a folder holding the block, then the pin dropped on it.
    let f = call(&mut ctx, "assembly_add_folder", json!({ "instances": [b], "name": "Base" }));
    let r = call(&mut ctx, "assembly_move_instance", json!({ "instances": [p], "folder": f["folder"].to_string() }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Add to folder");
    assert_eq!(r["folders"][0]["instances"].as_array().unwrap().len(), 2, "{r}");
    // Move to subassembly: the pin dropped onto an empty subassembly.
    let cs = call(&mut ctx, "assembly_create_subassembly", json!({}));
    let sub = cs["new_instance"]["id"].as_str().unwrap().to_string();
    let r = call(&mut ctx, "assembly_move_instance", json!({ "instances": [p], "subassembly": sub }));
    assert_eq!(last(&mut ctx), "Move to subassembly");
    assert_eq!(r["moved"].as_array().unwrap().len(), 1, "{r}");
    assert!(!r["instances"].as_array().unwrap().iter().any(|i| i["id"] == p), "{r}");
    let e = try_call(&mut ctx, "assembly_move_instance", json!({ "instances": [sub], "subassembly": sub })).unwrap_err();
    assert!(e.message.contains("cyclical"), "{e:?}");
    let e = try_call(&mut ctx, "assembly_move_instance", json!({ "instances": [b], "subassembly": b })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("subassembly"), "{e:?}");

    // Reorder : Explode steps.
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "add" }));
    call(&mut ctx, "assembly_explode_step", json!({ "view": "Explode 1", "instances": [b], "direction": [0, 0, 1], "distance": "10 mm" }));
    call(&mut ctx, "assembly_explode_step", json!({ "view": "Explode 1", "instances": [sub], "direction": [1, 0, 0], "distance": "20 mm" }));
    let ev = call(&mut ctx, "assembly_exploded_view", json!({ "action": "move", "view": "Explode 1", "step": 2, "to": 1 }));
    assert_eq!(last(&mut ctx), "Assembly 1 :: Reorder : Explode steps");
    assert_eq!(ev["exploded_views"][0]["step_instances"], json!([[sub], [b]]), "{ev}");
    let e = try_call(&mut ctx, "assembly_exploded_view", json!({ "action": "move", "view": "Explode 1", "step": 1, "to": 3 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("to"));

    // Mirror strategy: the block mirrored across its +X face, Derived.
    let faces = call(&mut ctx, "assembly_entities", json!({ "instance": b, "kind": "faces", "normal": [1, 0, 0] }));
    let face = faces["faces"][0]["id"].as_str().unwrap().to_string();
    let m = call(&mut ctx, "assembly_pattern", json!({ "kind": "mirror", "seeds": [b], "direction": { "instance": b, "entity": face }, "strategy": [{ "instance": b, "action": "derived", "placement": "origin_xy" }] }));
    let h = call(&mut ctx, "history_list", json!({ "limit": 2 }))["history"].as_array().unwrap().iter().map(|h| h["description"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(h[0].ends_with(":: Insert : Mirror 1") && h[1] == "Create mirrored Part Studio", "{h:?}");
    let tabs = call(&mut ctx, "document_info", json!({}))["tabs"].clone();
    assert!(tabs.as_array().unwrap().iter().any(|t| t["name"] == "Part Studio 1-Mirrored"), "{tabs}");
    let pat = m["patterns"].as_array().unwrap().iter().find(|x| x["name"] == "Mirror 1").unwrap().clone();
    assert_eq!(pat["strategy"], json!([{ "instance": b, "action": "derived", "placement": "origin_xy" }]), "{pat}");
    assert_eq!(m["created"].as_array().unwrap().len(), 1, "{m}");
    // Back to Transform: no second Part Studio.
    let m = call(&mut ctx, "assembly_pattern", json!({ "edit": "Mirror 1", "strategy": [{ "instance": b, "action": "transform", "placement": "auto" }] }));
    let pat = m["patterns"].as_array().unwrap().iter().find(|x| x["name"] == "Mirror 1").unwrap().clone();
    assert_eq!(pat["strategy"][0]["action"], "transform", "{pat}");
    assert_eq!(last(&mut ctx), "Assembly 1 :: Edit : Mirror 1");
    let e = try_call(&mut ctx, "assembly_pattern", json!({ "edit": "Mirror 1", "strategy": [{ "instance": sub }] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("strategy[0].instance"));

    // Image menu: Move to sheet, Image borders.
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    call(&mut ctx, "drawing_sheet", json!({ "drawing": tab, "action": "add" }));
    let png = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/icons/noshape-256.png");
    let r = call(&mut ctx, "drawing_insert_image", json!({ "drawing": tab, "sheet": 1, "path": png.to_str().unwrap(), "position": [20, 20] }));
    let img = r["image"].clone();
    let r = call(&mut ctx, "drawing_image", json!({ "drawing": tab, "sheet": 1, "image": img, "move_to": 2 }));
    assert!(last(&mut ctx).ends_with(":: Move image to sheet"));
    assert_eq!(r["images"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(r["image"], 1, "{r}");
    let s1 = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "sheet": 1 }));
    assert!(s1["images"].as_array().unwrap().is_empty(), "{s1}");
    call(&mut ctx, "drawing_image", json!({ "drawing": tab, "sheet": 2, "image": 1, "borders": true }));
    assert!(last(&mut ctx).ends_with(":: Image borders"));
    let pr = call(&mut ctx, "drawing_properties", json!({ "drawing": tab }));
    assert_eq!(pr["properties"]["image_borders"], true, "{pr}");
    let e = try_call(&mut ctx, "drawing_image", json!({ "drawing": tab, "sheet": 2, "image": 1, "move_to": 2 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("move_to"));
}

/// A Material Library tab's Update… / Export… through the API: its
/// materials then assign like the Onshape library's.
#[test]
fn material_library_update_and_export() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "matlib.noshape");
    let t = call(&mut ctx, "tab_create", json!({ "kind": "materiallibrary" }));
    let tab = t["tab"]["name"].as_str().or(t["name"].as_str()).unwrap_or("Material Library 1").to_string();
    let r = call(&mut ctx, "material_library_update", json!({ "tab": tab, "csv": "Category,Name,Density [kg/m^3],Supplier\nMetals,Brass C360,8500,Acme\nPlastics,\"PEEK, natural\",1300,\n" }));
    assert_eq!(r["materials"], 2, "{r}");
    let h = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(h.ends_with(":: Update material library"), "{h}");
    let l = call(&mut ctx, "material_list", json!({ "search": "peek" }));
    let mine: Vec<&Value> = l["materials"].as_array().unwrap().iter().filter(|m| m["library"] == tab.as_str()).collect();
    assert!(mine.len() == 1 && mine[0]["name"] == "PEEK, natural" && mine[0]["density_kg_m3"] == 1300.0, "{l}");
    let parts = call(&mut ctx, "query_parts", json!({}));
    let part = parts["parts"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "part_set_material", json!({ "tab": "Part Studio 1", "part": part, "material": "Brass C360", "library": tab }));
    let e = call(&mut ctx, "material_library_export", json!({ "tab": tab }));
    assert_eq!(e["csv"], "Category,Name,Density [kg/m^3],Supplier\nMetals,Brass C360,8500,Acme\nPlastics,\"PEEK, natural\",1300,\n", "{e}");
    let err = try_call(&mut ctx, "material_library_update", json!({ "tab": tab, "csv": "Name\nX\n" })).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("csv"), "{err:?}");
    let err = try_call(&mut ctx, "material_library_update", json!({ "tab": "Part Studio 1", "csv": "x" })).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("tab"), "{err:?}");
}

/// Live "dx (2026-10-27, P2 Derived appearance)": a derived part takes the
/// target studio's next default colour (grey beside its own blue-grey part),
/// unless the source part has an explicit appearance (red stays red).
#[test]
fn derived_parts_carry_explicit_appearances_only() {
    let mut ctx = Ctx::new();
    let path = scratch("derived_colour.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let block = |ctx: &mut Ctx, tab: &str, x: f64| {
        let sk = call(ctx, "sketch_create", json!({ "tab": tab, "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
        call(ctx, "sketch_draw", json!({ "tab": tab, "sketch": sk, "tool": "corner_rectangle", "points": [[x, 0], [x + 30.0, 20]] }));
        let r = call(ctx, "sketch_get", json!({ "tab": tab, "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
        call(ctx, "feature_add", json!({ "tab": tab, "type": "extrude", "params": { "entities": [r], "depth": "10 mm", "operationType": "NEW" } }));
    };
    block(&mut ctx, "Part Studio 1", 0.0);
    let b = call(&mut ctx, "tab_create", json!({ "kind": "partstudio" }))["tab"]["name"].as_str().unwrap().to_string();
    block(&mut ctx, &b, 50.0);
    call(&mut ctx, "feature_add", json!({ "tab": b, "type": "importDerived", "params": { "partStudio": "Part Studio 1" } }));
    let colours = |ctx: &mut Ctx| call(ctx, "query_parts", json!({ "tab": b }))["parts"].as_array().unwrap().iter().map(|p| p["color"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(colours(&mut ctx), ["#9dcfed", "#a5a5a5"], "its own part, then the derived one in the next default colour");
    let a = call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "part_set_appearance", json!({ "tab": "Part Studio 1", "parts": [a], "color": "#ff0000" }));
    assert_eq!(colours(&mut ctx), ["#9dcfed", "#ff0000"], "an explicit appearance comes along");
}

/// The Versions and history panel's search ▾ through history_list: by tab,
/// tab kind, author and time, and text in the description.
#[test]
fn history_list_filters_as_the_history_panel() {
    let mut ctx = Ctx::new();
    let (_b, _p) = two_part_assembly(&mut ctx, "history_filter.noshape");
    call(&mut ctx, "variable_create", json!({ "tab": "Part Studio 1", "name": "w", "value": "5 mm" }));
    let all = call(&mut ctx, "history_list", json!({ "limit": 100 }))["history"].as_array().unwrap().len();
    let descs = |r: &Value| r["history"].as_array().unwrap().iter().map(|h| h["description"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let asm = descs(&call(&mut ctx, "history_list", json!({ "tab": "Assembly 1", "limit": 100 })));
    assert!(!asm.is_empty() && asm.iter().all(|d| d.starts_with("Assembly 1 :: ")), "{asm:?}");
    let ps = descs(&call(&mut ctx, "history_list", json!({ "kinds": ["partstudio"], "limit": 100 })));
    assert!(ps.iter().any(|d| d.ends_with("Create variable")) && !ps.iter().any(|d| d.starts_with("Assembly 1 :: ")), "{ps:?}");
    assert!(ps.len() + asm.len() <= all);
    let found = descs(&call(&mut ctx, "history_list", json!({ "search": "ASSEMBLY 1 :: INSERT", "limit": 100 })));
    assert!(found.len() >= 2 && found.iter().all(|d| d.starts_with("Assembly 1 :: Insert")), "{found:?}");
    // This process wrote everything: "Not me" lists nothing, "Only me" all.
    assert!(descs(&call(&mut ctx, "history_list", json!({ "modified_by": "not_me" }))).is_empty());
    let first = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["time"].as_str().unwrap().parse::<i64>().unwrap();
    assert_eq!(descs(&call(&mut ctx, "history_list", json!({ "from": first + 1 }))).len(), 0);
    assert!(!descs(&call(&mut ctx, "history_list", json!({ "to": first }))).is_empty());
    let e = try_call(&mut ctx, "history_list", json!({ "kinds": ["sketch"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("kinds[0]"));
}

/// ☰ › Print… (live studios 2349): Download image is the framed view at about 300 dpi, Print its PDF page; the paper
/// and orientation come back next time. GPU-less machines skip the rendering.
#[test]
fn render_print_frames_the_view_on_paper() {
    let mut ctx = Ctx::new();
    let _ = two_part_assembly(&mut ctx, "print.noshape");
    let png = scratch("print-a6.png");
    match try_call(&mut ctx, "render_print", json!({ "paper": "A6", "orientation": "Portrait", "file": png.to_str().unwrap() })) {
        Ok(v) => {
            // A6 is 105 × 148 mm: 1239 × 1746 px.
            assert_eq!((v["width"].as_u64(), v["height"].as_u64(), v["output"].as_str()), (Some(1239), Some(1746), Some("png")), "{v}");
            let bytes = std::fs::read(&png).unwrap();
            assert_eq!(&bytes[1..4], b"PNG");
            assert_eq!((u32::from_be_bytes(bytes[16..20].try_into().unwrap()), u32::from_be_bytes(bytes[20..24].try_into().unwrap())), (1239, 1746));
            // Remembered: the next print without a paper is A6 Portrait again; Print's page is a PDF.
            let pdf = scratch("print-asm.pdf");
            let v = call(&mut ctx, "render_print", json!({ "tab": "Assembly 1", "orientation": "Landscape", "output": "pdf", "file": pdf.to_str().unwrap() }));
            assert_eq!((v["paper"].as_str(), v["orientation"].as_str(), v["width"].as_u64()), (Some("A6"), Some("Landscape"), Some(1746)), "{v}");
            assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
        }
        Err(e) if e.code == "render_failed" => eprintln!("render skipped: {}", e.message),
        Err(e) => panic!("render_print: {}", e.to_json()),
    }
    let e = try_call(&mut ctx, "render_print", json!({ "paper": "B5", "file": png.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("paper"));
    let e = try_call(&mut ctx, "render_print", json!({ "output": "svg", "file": png.to_str().unwrap() })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("output"));
}

/// Restore to Main… as live's dialog (studios 2351): each changed tab kept or restored, the entry
/// `Restore <m>m <s>s back: "<the restored point's change>"`.
#[test]
fn history_restore_keeps_the_picked_tabs() {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("restore_keep.noshape").to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "variablestudio", "name": "A" }));
    call(&mut ctx, "tab_create", json!({ "kind": "variablestudio", "name": "B" }));
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    call(&mut ctx, "variable_create", json!({ "tab": "A", "name": "a", "value": 1 }));
    call(&mut ctx, "variable_create", json!({ "tab": "B", "name": "b", "value": 2 }));
    call(&mut ctx, "history_restore", json!({ "to": "V1", "keep": ["B"] }));
    let count = |ctx: &mut Ctx, tab: &str| call(ctx, "variable_list", json!({ "tab": tab }))["variables"].as_array().map_or(0, Vec::len);
    assert_eq!((count(&mut ctx, "A"), count(&mut ctx, "B")), (0, 1));
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert!(last.starts_with("Restore ") && last.contains("s back: \""), "{last}");
    // Nothing to undo after a Restore (live ver-restore-undo).
    let u = call(&mut ctx, "history_undo", json!({}));
    assert_eq!((u["undone"].as_bool(), u["can_undo"].as_bool()), (Some(false), Some(false)), "{u}");
    call(&mut ctx, "history_restore", json!({ "to": "V1" }));
    assert_eq!(count(&mut ctx, "B"), 0);
    let e = try_call(&mut ctx, "history_restore", json!({ "to": "V1", "keep": ["nope"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("keep"));
}

/// ui_list on a Variable Studio: its Configurations panel's Expand all /
/// Collapse all, and Collapse all tables (also from a Part Studio's
/// Variable table panel).
#[test]
fn ui_list_variable_studio_display() {
    let mut ctx = Ctx::new();
    let path = scratch("ui_list_vs.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let vs = call(&mut ctx, "tab_create", json!({ "kind": "variablestudio" }))["tab"]["name"].as_str().unwrap().to_string();
    call(&mut ctx, "variable_studio_settings", json!({ "tab": vs, "insert_into_all": false }));
    call(&mut ctx, "config_add_input", json!({ "tab": vs, "kind": "list" }));
    let r = call(&mut ctx, "ui_list", json!({ "tab": vs, "action": "collapse_all" }));
    assert_eq!(r["inputs_open"], false, "{r}");
    let r = call(&mut ctx, "ui_list", json!({ "tab": vs, "action": "expand_all" }));
    assert_eq!(r["inputs_open"], true, "{r}");
    call(&mut ctx, "ui_list", json!({ "tab": vs, "action": "collapse_tables" }));
    call(&mut ctx, "ui_list", json!({ "tab": "Part Studio 1", "action": "collapse_tables" }));
    let e = try_call(&mut ctx, "ui_list", json!({ "tab": vs, "action": "filter", "text": "x" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("action"));
}

/// Release… through the API as the Release candidate dialog: part numbers,
/// revisions A then B, a version per release, the dialog's checks, and the
/// Revision history.
#[test]
fn release_parts_and_revision_history() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "release.noshape");
    let e = try_call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R1" })).unwrap_err();
    assert!(e.message.contains("needs attention"), "no part number: {e:?}");
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R1", "notes": "first", "part_numbers": { "Part 1": "PRT-0001" } }));
    assert_eq!(r["items"][0]["revision"], "A", "{r}");
    assert_eq!(r["items"][0]["part_number"], "PRT-0001", "{r}");
    let parts = call(&mut ctx, "query_parts", json!({}));
    assert_eq!(parts["parts"][0]["part_number"], "PRT-0001", "{parts}");
    let h = call(&mut ctx, "history_list", json!({ "limit": 3 }));
    assert!(h["versions"].as_array().unwrap().iter().any(|v| v["name"] == "R1"), "{h}");
    // The part number stays; the next release is revision B.
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R2" }));
    assert_eq!(r["items"][0]["revision"], "B", "{r}");
    let e = try_call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R3", "revisions": { "Part 1": "A" } })).unwrap_err();
    assert!(e.message.contains("Part 1"), "a revision lower than the released B: {e:?}");
    let rh = call(&mut ctx, "release_revision_history", json!({ "part_number": "PRT-0001" }));
    let revs = rh["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 2, "{rh}");
    assert!(revs[0]["revision"] == "B" && revs[0]["obsolete"] == false && revs[0]["version"] == "R2", "{rh}");
    // A stays released unless Company settings obsolete it automatically.
    assert!(revs[1]["revision"] == "A" && revs[1]["version"] == "R1", "{rh}");
    let all = call(&mut ctx, "release_revision_history", json!({}));
    assert_eq!(all["packages"].as_array().unwrap().len(), 2, "{all}");
}

/// The Release candidate dialog's drafts and row actions through the API (Onshape help "Typical Release Workflow"):
/// Save draft keeps a candidate without its checks, Release drafts lists it, Review Release releases it (and the draft
/// goes), the State column's Revert keeps the released revision, Delete draft.
#[test]
fn release_drafts_revert_and_generate() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "release-drafts.noshape");
    // Saved without a part number: a draft has no checks but its name.
    let d = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "D1", "notes": "draft", "draft": true }));
    assert_eq!(d["draft"], "D1", "{d}");
    let list = call(&mut ctx, "release_drafts", json!({}));
    assert_eq!(list["drafts"].as_array().unwrap().len(), 1, "{list}");
    assert_eq!(list["drafts"][0]["items"][0]["name"], "Part 1", "{list}");
    // Reopened and released with a part number: revision A, and the draft is gone.
    let r = call(&mut ctx, "release_create", json!({ "from_draft": "D1", "part_numbers": { "Part 1": "PRT-0007" } }));
    assert_eq!(r["release"], "D1", "{r}");
    assert_eq!(r["items"][0]["revision"], "A", "{r}");
    assert!(call(&mut ctx, "release_drafts", json!({}))["drafts"].as_array().unwrap().is_empty());
    // Revert: the item keeps its released revision A in the next candidate.
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R2", "draft": true, "revert": ["Part 1"] }));
    assert_eq!(r["items"][0]["revision"], "A", "reverted to the last released revision: {r}");
    let e = try_call(&mut ctx, "release_draft_delete", json!({ "name": "nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"));
    call(&mut ctx, "release_draft_delete", json!({ "name": "R2" }));
    assert!(call(&mut ctx, "release_drafts", json!({}))["drafts"].as_array().unwrap().is_empty());
    // Nothing named: `tab` or `from_draft` is needed; generating needs a sequential numbering scheme.
    let e = try_call(&mut ctx, "release_create", json!({ "name": "R3" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tab"));
    let e = try_call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R3", "generate_part_numbers": true, "revert": [] })).err();
    assert!(e.as_ref().is_none_or(|e| e.param.as_deref() == Some("generate_part_numbers")), "{e:?}");
}

/// The approval workflow (Onshape help "Typical Release Workflow"; the local user is the only person offered): with an
/// approver, Submit makes the candidate's version and leaves it Pending; Approve releases it at that version; Reject
/// keeps it, Rejected.
#[test]
fn release_submit_approve_and_reject() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "release-approve.noshape");
    let me = noshape_store::local_user_name();
    let e = try_call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R1", "part_numbers": { "Part 1": "PRT-0100" }, "approvers": ["Somebody Else"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("approvers[0]"), "{e:?}");
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R1", "part_numbers": { "Part 1": "PRT-0100" }, "approvers": [me], "observers": [me] }));
    assert_eq!(r["state"], "Pending", "{r}");
    // The candidate's version is made when it is submitted.
    let h = call(&mut ctx, "history_list", json!({ "limit": 3 }));
    assert!(h["versions"].as_array().unwrap().iter().any(|v| v["name"] == "R1"), "{h}");
    let list = call(&mut ctx, "release_drafts", json!({}));
    assert_eq!(list["candidates"][0]["state"], "Pending", "{list}");
    assert_eq!(list["candidates"][0]["approvers"][0], me.as_str(), "{list}");
    assert!(try_call(&mut ctx, "release_revision_history", json!({ "part_number": "PRT-0100" })).is_err(), "not released while pending");
    let a = call(&mut ctx, "release_approve", json!({ "name": "R1" }));
    assert_eq!(a["state"], "Released", "{a}");
    assert_eq!(a["approved_by"], me.as_str(), "Approved by …: {a}");
    assert_eq!(a["version"], r["version"], "released at the version made at Submit: {a}");
    let rh = call(&mut ctx, "release_revision_history", json!({ "part_number": "PRT-0100" }));
    assert_eq!(rh["revisions"][0]["revision"], "A", "{rh}");
    // A second candidate, rejected: it stays, Rejected, and can't be approved.
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R2", "approvers": [me] }));
    assert_eq!(r["state"], "Pending", "{r}");
    assert_eq!(call(&mut ctx, "release_reject", json!({ "name": "R2" }))["state"], "Rejected");
    let list = call(&mut ctx, "release_drafts", json!({}));
    assert_eq!(list["candidates"][0]["state"], "Rejected", "{list}");
    assert_eq!(list["candidates"][0]["decided_by"], me.as_str(), "Rejected by …: {list}");
    let e = try_call(&mut ctx, "release_approve", json!({ "name": "R2" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("name"));
    // Comments, then Discard: it stays, Discarded.
    call(&mut ctx, "release_comment", json!({ "name": "R2", "text": "Wrong hole size." }));
    call(&mut ctx, "release_discard", json!({ "name": "R2" }));
    let list = call(&mut ctx, "release_drafts", json!({}));
    assert_eq!(list["candidates"][0]["state"], "Discarded", "{list}");
    assert_eq!(list["candidates"][0]["comments"][0]["text"], "Wrong hole size.", "{list}");
    assert_eq!(try_call(&mut ctx, "release_discard", json!({ "name": "R2" })).unwrap_err().param.as_deref(), Some("name"));
    // Without approvers, Release releases at once.
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "R3" }));
    assert_eq!(r["state"], "Released", "{r}");
}

/// Include associated drawings leaves out a drawing made from another workspace's state (Onshape help "Typical Release
/// Workflow": "drawings referencing a version from another workspace are left out").
#[test]
fn release_leaves_out_a_drawing_made_from_another_workspace() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "release-assoc.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let drawing = d["drawing"].as_str().unwrap().to_string();
    let kinds = |r: &Value| r["items"].as_array().unwrap().iter().map(|i| i["type"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "D1", "draft": true, "include_drawings": true }));
    assert!(kinds(&r).iter().any(|k| k == "Drawing"), "the drawing is associated: {r}");
    // A state made on another workspace (B1), then the drawing in Main marked as made from it.
    let main = call(&mut ctx, "document_info", json!({}))["branch"].as_str().unwrap().to_string();
    call(&mut ctx, "history_create_branch", json!({ "name": "B1" }));
    call(&mut ctx, "variable_create", json!({ "name": "w", "value": "3 mm" }));
    let b1_head = ctx.doc(None).unwrap().session.head().unwrap().to_string();
    call(&mut ctx, "history_switch_branch", json!({ "branch": main }));
    {
        let od = ctx.doc(None).unwrap();
        let id = noshape_ops::doc::find_tab(od.doc(), &drawing).unwrap();
        if let Some(noshape_model::TabContent::Drawing(dr)) = od.session.doc.tab_mut(id).map(|t| &mut t.content) {
            dr.sources_at = Some(b1_head);
        }
        od.session.commit_all("test: drawing made on B1").unwrap();
    }
    let r = call(&mut ctx, "release_create", json!({ "tab": "Part Studio 1", "name": "D2", "draft": true, "include_drawings": true }));
    assert!(!kinds(&r).iter().any(|k| k == "Drawing"), "made from B1's state, left out: {r}");
    assert_eq!(r["drawings_left_out"].as_array().map(Vec::len), Some(1), "{r}");
}

/// qa DWG-DIM-TWO-EDGES: Line to line on two parallel view edges clicked at
/// their middles (where snap points and other targets sit on top) dimensions
/// the distance between them.
#[test]
fn drawing_line_to_line_on_two_edges() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "dim_two_edges.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let dw = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": dw, "edges": true }));
    let view = g["views"].as_array().unwrap().iter().find(|v| v["orientation"] == "front").unwrap().clone();
    let mut vertical: Vec<Value> = view["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line" && (num(&e["a"][0]) - num(&e["b"][0])).abs() < 1e-6).cloned().collect();
    vertical.sort_by(|a, b| num(&a["a"][0]).total_cmp(&num(&b["a"][0])));
    let (l, r) = (vertical.first().unwrap(), vertical.last().unwrap());
    let mid = |e: &Value| json!([num(&e["a"][0]), (num(&e["a"][1]) + num(&e["b"][1])) / 2.0]);
    let width = num(&r["a"][0]) - num(&l["a"][0]);
    let out = call(&mut ctx, "drawing_dimension", json!({ "drawing": dw, "kind": "line_to_line", "picks": [mid(l), mid(r)], "place": [num(&l["a"][0]) + width / 2.0, num(&l["b"][1]).max(num(&l["a"][1])) + 15.0] }));
    let text = out["annotations"].as_array().unwrap().last().unwrap()["text"].as_str().unwrap().to_string();
    let scale = num(&view["scale"]);
    assert!((text.parse::<f64>().unwrap() - width / scale).abs() < 0.01, "{text} vs {} (sheet {width}, scale {scale})", width / scale);
}

/// qa API-ASM-CONFIG-NAMES: an assembly instance's configuration given by
/// input and option names ({"Size": "Large"}) is resolved to their ids,
/// as config_activate does, so the instance is that configuration.
#[test]
fn assembly_configuration_by_names() {
    let mut ctx = Ctx::new();
    let path = scratch("asm_config_names.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    let ext = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }))["feature"].as_str().unwrap().to_string();
    let size = call(&mut ctx, "config_add_input", json!({ "kind": "list", "name": "Size" }))["input"].as_str().unwrap().to_string();
    assert_ne!(size, "Size", "the input's id differs from its name");
    call(&mut ctx, "config_add_option", json!({ "input": "Size", "name": "Large" }));
    // A parameter the feature doesn't have is named, with the near one.
    let e = try_call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "dept", "input": size })).expect_err("no dept");
    assert!(e.message.starts_with("Extrude 1 has no parameter `dept` (did you mean `depth`?)"), "{}", e.to_json());
    call(&mut ctx, "config_configure", json!({ "feature": ext, "param": "depth", "input": size }));
    call(&mut ctx, "config_set_value", json!({ "feature": ext, "param": "depth", "configuration": "Large", "value": "30 mm" }));
    let part = call(&mut ctx, "query_parts", json!({}))["parts"][0]["id"].as_str().unwrap().to_string();
    let i = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": part, "configuration": { "Size": "Large" } }))["instance"].as_str().unwrap().to_string();
    let top = |ctx: &mut Ctx, inst: &str| call(ctx, "assembly_entities", json!({ "instance": inst, "kind": "faces", "normal": [0, 0, 1] }))["faces"].as_array().unwrap().iter().map(|f| num(&f["centroid"][2])).fold(f64::MIN, f64::max);
    assert!((top(&mut ctx, &i) - 30.0).abs() < 1e-6, "inserted in Large");
    call(&mut ctx, "assembly_set_configuration", json!({ "instance": i, "configuration": { "Size": "Default" } }));
    assert!((top(&mut ctx, &i) - 10.0).abs() < 1e-6, "set back to Default");
    let e = try_call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": part, "configuration": { "Size": "Huge" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("configuration.Size"), "{e:?}");
}

/// Drawing edits the API lacked (the audit of the app's edit texts): a
/// linear dimension's orientation and Flip arrows, a section view's
/// Cutting line angle.
#[test]
fn drawing_orientation_flip_and_cutting_line_angle() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "dim_orient.noshape");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let dw = d["drawing"].as_str().unwrap().to_string();
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": dw, "edges": true }));
    let views = g["views"].as_array().unwrap().clone();
    let front = views.iter().find(|v| v["orientation"] == "front").unwrap();
    let mut vertical: Vec<Value> = front["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line" && (num(&e["a"][0]) - num(&e["b"][0])).abs() < 1e-6).cloned().collect();
    vertical.sort_by(|a, b| num(&a["a"][0]).total_cmp(&num(&b["a"][0])));
    let (l, r) = (vertical.first().unwrap(), vertical.last().unwrap());
    let mid = |e: &Value| json!([num(&e["a"][0]), (num(&e["a"][1]) + num(&e["b"][1])) / 2.0]);
    let out = call(&mut ctx, "drawing_dimension", json!({ "drawing": dw, "picks": [mid(l), mid(r)], "place": [(num(&l["a"][0]) + num(&r["a"][0])) / 2.0, num(&l["a"][1]).max(num(&l["b"][1])) + 15.0] }));
    let dim = out["annotations"].as_array().unwrap().last().unwrap()["id"].as_u64().unwrap();
    call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": dw, "annotation": dim, "action": "orientation", "value": "aligned" }));
    assert!(last(&mut ctx).ends_with(":: Dimension orientation"));
    call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": dw, "annotation": dim, "action": "flip_arrows" }));
    assert!(last(&mut ctx).ends_with(":: Flip arrows"));
    let e = try_call(&mut ctx, "drawing_annotation_edit", json!({ "drawing": dw, "annotation": dim, "action": "arc_condition", "end": 0, "value": "minimum" })).unwrap_err();
    assert!(e.message.contains("maximum / minimum"), "{e:?}");
    // A section through the hole, its cutting line turned to 45°.
    let top = views.iter().find(|v| v["edges"].as_array().unwrap().iter().any(|e| e["type"] == "circle")).unwrap();
    let circle = top["edges"].as_array().unwrap().iter().find(|e| e["type"] == "circle").unwrap();
    let s = call(&mut ctx, "drawing_section_view", json!({ "drawing": dw, "parent": top["id"], "point": circle["center"], "position": [num(&top["bounds"]["max"][0]) + 60.0, num(&circle["center"][1])] }));
    let sid = s["view"].as_u64().unwrap();
    call(&mut ctx, "drawing_view_edit", json!({ "drawing": dw, "view": sid, "action": "cutting_line_angle", "to": 45 }));
    assert!(last(&mut ctx).ends_with(":: Cutting line angle"));
    let e = try_call(&mut ctx, "drawing_view_edit", json!({ "drawing": dw, "view": top["id"], "action": "cutting_line_angle", "to": 45 })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("view"), "{e:?}");
}

/// Snap mode through the API: the pin's bottom face onto the block's top
/// face, face to face, fastened (one edit, as the window's).
#[test]
fn assembly_snap_fastens_face_to_face() {
    let mut ctx = Ctx::new();
    let (b, p) = two_part_assembly(&mut ctx, "snap.noshape");
    let face = |ctx: &mut Ctx, inst: &str, n: [f64; 3]| call(ctx, "assembly_entities", json!({ "instance": inst, "kind": "faces", "surface": "plane", "normal": n }))["faces"][0].clone();
    let (pin_bottom, block_top) = (face(&mut ctx, &p, [0.0, 0.0, -1.0]), face(&mut ctx, &b, [0.0, 0.0, 1.0]));
    let r = call(&mut ctx, "assembly_snap", json!({ "a": { "instance": p, "entity": pin_bottom["id"] }, "b": { "instance": b, "entity": block_top["id"] } }));
    assert!(r["mate"].as_str().is_some_and(|m| m.starts_with("mate:")), "{r}");
    let h = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert_eq!(h, "Assembly 1 :: Insert : Fastened (snap)");
    let mates = r["mates"].as_array().unwrap();
    assert!(mates.iter().any(|m| m["name"] == "Fastened 1" && m["type"] == "fastened"), "{r}");
    let now = face(&mut ctx, &p, [0.0, 0.0, -1.0]);
    assert!((num(&now["centroid"][2]) - num(&block_top["centroid"][2])).abs() < 1e-6, "face to face: {now} vs {block_top}");
    let e = try_call(&mut ctx, "assembly_snap", json!({ "a": { "instance": p, "entity": pin_bottom["id"] }, "b": { "instance": p, "entity": pin_bottom["id"] } })).unwrap_err();
    assert!(e.message.contains("different instance"), "{e:?}");
}

/// qa SURF-EXTRUDE-SKETCH-EDGES: a Surface extrude of a sketch picked whole
/// (its feature-list row: "Edges of Sketch 1", a `sketch:` reference)
/// extrudes all its curves, as picking the circle itself does (live).
#[test]
fn surface_extrude_of_a_whole_sketch() {
    let mut ctx = Ctx::new();
    let path = scratch("surface-sketch.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "circle", "points": [[0, 0], [10, 0]] }));
    let ex = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "bodyType": "SURFACE", "surfaceEntities": [format!("sketch:{sketch}")], "depth": "20 mm" } }));
    assert_eq!(ex["feature_status"]["status"], "Ok", "{ex}");
    let parts = call(&mut ctx, "query_parts", json!({}));
    let bodies = parts["parts"].as_array().unwrap();
    assert_eq!(bodies.len(), 1, "{parts}");
    assert!(bodies[0]["kind"].as_str().unwrap().contains("Surface"), "{parts}");
    // A Ø20 × 20 tube: 2π·10·20 mm².
    let area = num(&bodies[0]["surface_area"]);
    assert!((area - 2.0 * std::f64::consts::PI * 200.0).abs() < 1e-6, "{area}");
}

/// The title block's DWG NO. / REV / TITLE / DRAWN date as live maps them
/// (behaviours "dx title block mappings"): the drawing's own Part number
/// and Revision, the reference part's Title 1–3, the day it was made.
#[test]
fn title_block_maps_the_drawings_own_properties() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "title_map.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
    let r = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].clone();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [r], "depth": "10 mm" } }));
    call(&mut ctx, "part_set_properties", json!({ "part": "Part 1", "part_number": "P-100", "revision": "B" }));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ANSI_A_MM.dwt", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let field = |ctx: &mut Ctx, f: &str| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["title_fields"].as_array().unwrap().iter().find(|x| x["field"] == f).map(|x| x["shown"].as_array().map(|l| l.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default()).unwrap_or_default();
    // Not the part's number / revision.
    assert_ne!(field(&mut ctx, "Part number"), "P-100");
    assert_eq!(field(&mut ctx, "Revision"), "-");
    let p = call(&mut ctx, "drawing_element_properties", json!({ "drawing": tab }));
    assert!(p["properties"]["created"].is_string(), "{p}");
    call(&mut ctx, "drawing_element_properties", json!({ "drawing": tab, "set": { "part_number": "D-200", "revision": "C" } }));
    assert_eq!(field(&mut ctx, "Part number"), "D-200");
    assert_eq!(field(&mut ctx, "Revision"), "C");
    let e = try_call(&mut ctx, "drawing_element_properties", json!({ "drawing": tab, "set": { "bogus": "x" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.bogus"), "{e:?}");
    // The Properties dialog's drawing fields (live doc-tab-properties-drawing): Drawn by, Approver, the dates.
    let p = call(&mut ctx, "drawing_element_properties", json!({ "drawing": tab, "set": { "drawn_by": "A. Drafter", "approver": "B. Checker", "approved": "2026-11-09" } }));
    assert_eq!((p["properties"]["drawn_by"].clone(), p["properties"]["approver"].clone(), p["properties"]["approved"].clone()), (json!("A. Drafter"), json!("B. Checker"), json!("2026-11-09")), "{p}");
    let e = try_call(&mut ctx, "drawing_element_properties", json!({ "drawing": tab, "set": { "approved": "soon" } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("set.approved"), "{e:?}");
}

/// Live ("(7) Dragging feature rows"): a fillet moved above the Extrude its edge comes from fails with its own
/// empty-selection message and "1 missing selection"; the feature list says how many selections are missing.
#[test]
fn a_fillet_moved_above_its_extrude_reports_its_missing_selection() {
    let mut ctx = Ctx::new();
    let path = scratch("missing-selection.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let edge = call(&mut ctx, "query_entities", json!({ "kind": "edges" }))["edges"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": [edge], "radius": "2 mm" } }));
    call(&mut ctx, "feature_reorder", json!({ "features": ["Fillet 1"], "to": 1 }));
    let list = call(&mut ctx, "feature_list", json!({}));
    let fillet = list["features"].as_array().unwrap().iter().find(|f| f["name"] == "Fillet 1").unwrap();
    assert_eq!(fillet["status"], "Error", "{list}");
    assert_eq!(fillet["message"], "Select edges or faces to fillet.");
    assert_eq!(fillet["missing_selections"], 1);
    assert_eq!(list["failed"][0]["missing_selections"], 1, "{list}");
}

/// A hole table's SIZE is the Hole feature's callout, written once for a
/// size with its QTY (live behaviours 1983: "Ø.250 ↧.472", QTY 3, merged).
#[test]
fn hole_table_sizes_are_the_hole_callouts() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "hole_table.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [80, 50]] }));
    let r = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].clone();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [r], "depth": "10 mm" } }));
    let top = call(&mut ctx, "query_entities", json!({ "kind": "faces", "at": [[40, 25, 10]] }))["faces"][0]["id"].clone();
    let s2 = call(&mut ctx, "sketch_create", json!({ "plane": top }))["sketch"].clone();
    let p = call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "point", "points": [[15, 15, 10], [65, 15, 10], [40, 35, 10]] }));
    let pts: Vec<Value> = p["created"].as_array().unwrap().iter().map(|x| x["id"].clone()).collect();
    let hole = call(&mut ctx, "feature_add", json!({ "type": "hole", "params": { "locations": pts, "holeDiameterV3": "6.6 mm", "endStyleV2": "THROUGH" } }));
    assert_eq!(hole["regeneration"]["failed"], json!([]), "{hole}");
    // R2 rounds on the block's vertical edges: convex, not holes (qa HOLE-TABLE-FILLETS).
    let edges = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "line" }));
    let vertical: Vec<Value> = edges["edges"].as_array().unwrap().iter().filter(|e| num(&e["direction"][2]).abs() > 0.99 && (num(&e["start"][0]) < 1.0 || num(&e["start"][0]) > 79.0)).map(|e| e["id"].clone()).collect();
    assert_eq!(vertical.len(), 4, "{edges}");
    let fi = call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": vertical, "radius": "2 mm" } }));
    assert_eq!(fi["feature_status"]["status"], "Ok", "{fi}");
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
    let view = g["views"].as_array().unwrap().iter().find(|v| v["edges"].as_array().is_some_and(|es| es.iter().filter(|e| e["type"] == "circle").count() >= 3)).cloned().expect("the view showing the holes");
    let corner = view["edges"].as_array().unwrap().iter().find(|e| e["type"] == "line").map(|e| [num(&e["a"][0]), num(&e["a"][1])]).unwrap();
    let ht = call(&mut ctx, "drawing_view_table", json!({ "drawing": tab, "kind": "hole", "view": view["id"], "origin": corner, "position": [20, 120] }));
    let t = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["tables"].as_array().unwrap().iter().find(|t| t["id"] == ht["table"]).cloned().unwrap();
    let rows = t["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{t}");
    // (ISO_A3.dwt: a decimal comma.)
    assert!(rows[0][3].as_str().is_some_and(|s| s.starts_with('⌀') && s.contains("6,6") && s.contains("THRU")), "{t}");
    assert_eq!(rows[0][4], "3", "{t}");
    assert_eq!((rows[1][3].as_str(), rows[2][4].as_str()), (Some(""), Some("")), "merged: {t}");
    // Split hole table below A1 (help drawing_hole_table): the second part
    // shows the group's SIZE / QTY too.
    let sp = call(&mut ctx, "drawing_table_edit", json!({ "drawing": tab, "table": ht["table"], "action": "split", "row": 1 }));
    let tables = call(&mut ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["tables"].clone();
    let part = tables.as_array().unwrap().iter().find(|t| t["id"] == sp["table"]).cloned().unwrap();
    let rows = part["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{part}");
    assert!(rows[0][3].as_str().is_some_and(|s| s.contains("6,6")) && rows[0][4] == "3", "{part}");
}


/// MASS-MC-FIELD: the reference mate connector is named also when the part has no material (no mass).
#[test]
fn mass_properties_name_the_reference_connector_without_a_material() {
    let mut ctx = Ctx::new();
    let path = scratch("mass-reference.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let vertex = call(&mut ctx, "query_entities", json!({ "kind": "vertices" }))["vertices"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "mateConnector", "params": { "originQuery": [vertex] } }));
    let mc = call(&mut ctx, "query_datums", json!({}));
    let mc = mc["mate_connectors"][0]["id"].as_str().unwrap_or_else(|| panic!("{mc}")).to_string();
    let m = call(&mut ctx, "query_mass_properties", json!({ "reference": mc }));
    assert_eq!(m["reference"], "Mate connector 1", "{m}");
}

/// Live 1959: a part with no material and a mass override of 2 kg has the volume's centroid and the inertia of that
/// mass spread uniformly (the 40 × 30 × 10 block: Lxx 367.437 lb mm² = 166.667 kg mm², Lyy 283.333, Lzz 416.667).
#[test]
fn a_mass_override_without_a_material_spreads_the_mass_uniformly() {
    let mut ctx = Ctx::new();
    let path = scratch("mass-override-no-material.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "corner_rectangle", "points": [[0, 0], [40, 30]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    call(&mut ctx, "part_set_properties", json!({ "part": "Part 1", "mass_override": 2.0 }));
    let m = call(&mut ctx, "query_mass_properties", json!({ "parts": ["Part 1"] }));
    assert_eq!(m["mass_kg"], 2.0, "{m}");
    let c: Vec<f64> = m["center_of_mass"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert!((c[0] - 20.0).abs() < 1e-6 && (c[1] - 15.0).abs() < 1e-6 && (c[2] - 5.0).abs() < 1e-6, "{m}");
    let i = &m["inertia"];
    let near = |v: &serde_json::Value, want: f64| (v.as_f64().unwrap() - want).abs() < 1e-3;
    assert!(near(&i[0][0], 166.667) && near(&i[1][1], 283.333) && near(&i[2][2], 416.667) && near(&i[0][1], 0.0), "{m}");
}

/// This machine's setup through the API (Help › Install command line tool…
/// and Connect to coding agents…): the agents listed with their state, the
/// command line tool linked into a folder, bad agent ids named. No agent's
/// configuration is changed here (crates/ops/tests/install.rs does that
/// against fake homes).
#[test]
fn agents_and_command_line_tool_ops() {
    let mut ctx = Ctx::new();
    let st = call(&mut ctx, "agents_status", json!({ "command": "/opt/noshape/noshape" }));
    let ids: Vec<&str> = st["agents"].as_array().unwrap().iter().map(|a| a["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["claude_code", "codex", "claude_desktop"], "{st}");
    assert_eq!(st["command"], "/opt/noshape/noshape");
    let e = try_call(&mut ctx, "agents_connect", json!({ "agents": ["cursor"] })).unwrap_err();
    assert_eq!((e.code, e.param.as_deref()), ("invalid_argument", Some("agents")), "{}", e.to_json());
    let e = try_call(&mut ctx, "agents_disconnect", json!({ "agents": "claude_code,nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("agents"));
    #[cfg(unix)]
    {
        let dir = scratch("cli-install-bin");
        let target = scratch("cli-install-target");
        std::fs::write(&target, "").unwrap();
        let r = call(&mut ctx, "cli_install", json!({ "dir": dir.to_str().unwrap(), "command": target.to_str().unwrap() }));
        assert_eq!(r["path"], dir.join("noshape").to_str().unwrap(), "{r}");
        assert_eq!(std::fs::read_link(dir.join("noshape")).unwrap(), target);
        assert_eq!(r["on_path"], false);
    }
}

/// Help › About through the API: the version, the copyright, Open CASCADE's notice, and the notices file the
/// dialog's "Open source software components" link opens (OCCT's LGPL among them); `notices` adds their text.
#[test]
fn about_names_the_copyright_and_the_notices() {
    let mut ctx = Ctx::new();
    let r = call(&mut ctx, "app_about", json!({}));
    assert_eq!((r["name"].as_str(), r["copyright"].as_str()), (Some("Noshape"), Some("© 2026 Yekta")), "{r}");
    assert!(!r["version"].as_str().unwrap().is_empty(), "{r}");
    assert!(r["occt"].as_str().unwrap().contains("GNU Lesser General Public License version 2.1"), "{r}");
    assert!(r.get("notices_text").is_none(), "the text only on request");
    let text = std::fs::read_to_string(r["notices_file"].as_str().unwrap()).unwrap();
    assert_eq!(text, noshape_ops::about::NOTICES);
    assert!(text.contains("Open CASCADE exception") && text.contains("GNU LESSER GENERAL PUBLIC LICENSE"));
    let r = call(&mut ctx, "app_about", json!({ "notices": true }));
    assert_eq!(r["notices_text"].as_str(), Some(text.as_str()));
    let e = try_call(&mut ctx, "app_about", json!({ "notices": "yes" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("notices"), "{}", e.to_json());
}

/// MBD dimensions (drawing.md CR18): a Model definitions dimension between
/// two holes' cylinders measures their axes' distance, and a drawing of the
/// part shows it, with its tolerance, in the view facing its plane.
#[test]
fn mbd_dimension_shows_in_the_drawing() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "mbd-dim.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-30, -20], [30, 20]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[-15, 0], [-11, 0]] }));
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[15, 0], [19, 0]] }));
    let regions = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"].clone();
    let outer = regions.as_array().unwrap().iter().max_by(|a, b| num(&a["area"]).total_cmp(&num(&b["area"]))).unwrap()["id"].clone();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [outer], "depth": "20 mm" } }));
    let cyl = call(&mut ctx, "query_entities", json!({ "kind": "faces", "surface": "cylinder" }))["faces"].clone();
    let ids: Vec<Value> = cyl.as_array().unwrap().iter().map(|f| f["id"].clone()).collect();
    assert_eq!(ids.len(), 2, "{cyl}");
    let r = call(&mut ctx, "part_annotation", json!({ "action": "add_dimension", "entities": ids, "plane": "top", "position": [0, 10, 20], "tolerance": { "type": "Symmetrical", "upper": 0.1 } }));
    let d = &r["annotations"][0]["kind"]["dimension"];
    assert!((num(&d["value"]) - 30.0).abs() < 1e-6, "{r}");
    // Its plane (top) faces the Top view only.
    let dr = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": true }));
    let name = dr["drawing"].as_str().unwrap().to_string();
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": name }));
    let dims: Vec<&Value> = g["annotations"].as_array().unwrap().iter().filter(|a| a["kind"] == "LinearDimension").collect();
    assert_eq!(dims.len(), 1, "{}", g["annotations"]);
    assert!(dims[0]["text"].as_str().unwrap().starts_with("30±0"), "{}", dims[0]);
    // STEP AP242 carries it as semantic PMI (audit X3), with a datum on the
    // top face, as OCCT's XDE reads it back.
    let top = call(&mut ctx, "query_entities", json!({ "tab": "Part Studio 1", "kind": "faces", "surface": "plane", "normal": [0, 0, 1] }))["faces"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "part_annotation", json!({ "tab": "Part Studio 1", "action": "add_datum", "face": top }));
    let step = scratch("mbd-pmi.step");
    call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP", "file": step.to_str().unwrap() }));
    let pmi = noshape_kernel::read_step_pmi(&step).unwrap();
    // Between the holes' axes: a centre distance (type 0), as XDE reads
    // live's own file (live 1746).
    assert!(pmi.iter().any(|l| l == "dim 0 30 tol 0.1 0.1 faces 1+1"), "{pmi:?}");
    let text = std::fs::read_to_string(&step).unwrap();
    assert!(text.contains("=DATUM_FEATURE('Datum_feature A',") && text.contains("=DATUM('Datum A','Datum A',"), "the datum");
    assert!(text.contains("=DIMENSIONAL_LOCATION('linear distance centre',") && text.contains("=SHAPE_ASPECT('AXIS_DISTANCE."), "{text}");
    // Drawn as live draws it: a callout of tessellated text, arrows and lines.
    assert_eq!(text.matches("=DRAUGHTING_CALLOUT(").count(), 2, "the dimension and the datum");
    assert!(text.contains("=TESSELLATED_ANNOTATION_OCCURRENCE('linear dimension',"));
    // A custom text height scales only the text (live: ×1.25 at 5 mm).
    let tall = scratch("mbd-pmi-5.step");
    call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP", "file": tall.to_str().unwrap(), "mbd_text_height": 5 }));
    assert_ne!(std::fs::read_to_string(&tall).unwrap(), text);
    let e = try_call(&mut ctx, "export_parts", json!({ "tab": "Part Studio 1", "format": "STEP", "file": tall.to_str().unwrap(), "mbd_text_height": 60 })).unwrap_err();
    assert_eq!(e.message, "Valid annotations text height in range from 0.0001 to 50 is required.", "{e:?}");
    let top = g["views"].as_array().unwrap().iter().find(|v| v["orientation"] == "top").expect("a Top view");
    assert_eq!(dims[0]["view"], top["id"], "{g}");
    // A second drawing without views: a Top view inserted later gets it.
    let dr = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
    let name = dr["drawing"].as_str().unwrap().to_string();
    call(&mut ctx, "drawing_insert_view", json!({ "drawing": name, "source": "Part Studio 1", "orientation": "top", "position": [200, 150] }));
    let g = call(&mut ctx, "drawing_get", json!({ "drawing": name }));
    assert_eq!(g["annotations"].as_array().unwrap().iter().filter(|a| a["kind"] == "LinearDimension").count(), 1, "{}", g["annotations"]);
}


/// Format coverage (docs/parity/formats.md): one part exported to every
/// export format, then each file imported back where noshape reads that
/// format; prints the matrix (format, bytes, parts and volume back) and
/// holds B-rep formats to the volume, meshes to 3 %.
#[test]
fn every_export_format_imports_back() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "formats.noshape");
    let v0 = num(&call(&mut ctx, "query_parts", json!({}))["parts"][0]["volume"]);
    let formats = ["PARASOLID", "ACIS", "STEP", "IGES", "PVZ", "JT", "RHINO", "GLTF", "GLB", "OBJ", "3MF", "STL"];
    let mut rows = Vec::new();
    for f in formats {
        let file = scratch(&format!("formats-{}", f.to_lowercase()));
        let r = match try_call(&mut ctx, "export_parts", json!({ "format": f, "file": file.to_str().unwrap() })) {
            Ok(r) => r,
            Err(e) => {
                rows.push(format!("{f}: export unavailable ({})", e.message));
                continue;
            }
        };
        let mut out = std::path::PathBuf::from(r["file"].as_str().unwrap());
        // OBJ downloads a zip of the .obj and its .mtl (as live, which
        // doesn't translate that zip): the .obj is what imports.
        if out.extension().is_some_and(|e| e == "zip") {
            let dir = scratch(&format!("formats-{}-unzip", f.to_lowercase()));
            std::fs::create_dir_all(&dir).unwrap();
            let names = noshape_kernel::extract_zip(&out, &dir).unwrap();
            out = dir.join(names.iter().find(|n| n.ends_with(".obj")).expect("the .obj"));
        }
        let mut ictx = Ctx::new();
        call(&mut ictx, "document_create", json!({ "path": scratch(&format!("formats-in-{}.noshape", f.to_lowercase())).to_str().unwrap() }));
        // STL carries no unit: written in metres (live's default), read as metres.
        let mut args = json!({ "files": [out.to_str().unwrap()] });
        if f == "STL" {
            args["mesh_unit"] = json!("Meter");
        }
        match try_call(&mut ictx, "import_files", args) {
            Ok(i) => {
                let studio = i["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").map(|t| t["name"].as_str().unwrap().to_string());
                let (n, v) = match studio {
                    Some(s) => {
                        let q = call(&mut ictx, "query_parts", json!({ "tab": s }));
                        let parts = q["parts"].as_array().unwrap().clone();
                        (parts.len(), parts.iter().map(|p| p["volume"].as_f64().unwrap_or(0.0)).sum::<f64>())
                    }
                    None => (0, 0.0),
                };
                let mesh = matches!(f, "PVZ" | "GLTF" | "GLB" | "OBJ" | "3MF" | "STL");
                rows.push(format!("{f}: {} bytes, back {n} part(s), volume {:.4} of {v0:.4}{}", r["bytes"], v, if mesh { " (mesh)" } else { "" }));
                assert!(n >= 1, "{f}: no part back");
                let tol = if mesh { 0.03 } else { 1e-6 };
                assert!((v - v0).abs() <= v0 * tol, "{f}: volume {v} vs {v0}");
            }
            Err(e) => rows.push(format!("{f}: {} bytes, not importable ({})", r["bytes"], e.message)),
        }
    }
    eprintln!("format matrix:\n  {}", rows.join("\n  "));
}

/// A mistyped name gets the list and the nearest option (agent usability):
/// arguments, enum values, feature parameters, drawing tools, formats.
#[test]
fn mistyped_names_get_the_options_and_a_suggestion() {
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch("typos.noshape").to_str().unwrap() }));
    let e = try_call(&mut ctx, "query_entities", json!({ "kind": "face" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("kind"));
    assert!(e.message.contains("`face` is not one of: faces, edges, vertices (did you mean `faces`?)"), "{}", e.message);
    let e = try_call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "depht": "10 mm" } })).unwrap_err();
    assert!(e.message.starts_with("unknown parameter `depht` (did you mean `depth`?)"), "{}", e.message);
    let e = try_call(&mut ctx, "query_entities", json!({ "kinds": "faces" })).unwrap_err();
    assert!(e.message.starts_with("unknown argument (did you mean `kind`?)"), "{}", e.message);
    call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
    let e = try_call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "shapes": [{ "tool": "cirle", "points": [[0, 0], [5, 0]] }] })).unwrap_err();
    assert!(e.message.starts_with("`cirle` is not a drawing tool (did you mean `circle`?); tools: line,"), "{}", e.message);
    let e = try_call(&mut ctx, "export_parts", json!({ "format": "STP", "file": scratch("typo.stp").to_str().unwrap() })).unwrap_err();
    assert!(e.message.starts_with("`STP` is not one of: PARASOLID, ACIS, STEP,") && e.message.ends_with("(did you mean `STEP`?)"), "{}", e.message);
}

/// A hole seen end-on gets a centermark though its edges aren't circles (a
/// radial hole through a round boss, its edges saddle curves; live 1857:
/// pipeflange-steven's Right view): the boss's mark in the Top view and the
/// hole's in the Front view, six lines each.
#[test]
fn a_radial_hole_gets_its_centermark() {
    let mut ctx = Ctx::new();
    let sketch = new_sketch(&mut ctx, "radial-hole.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "circle", "points": [[0, 0], [20, 0]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "40 mm" } }));
    let hole = call(&mut ctx, "sketch_create", json!({ "plane": "plane:front" }))["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "sketch": hole, "tool": "circle", "points": [[0, 20, 0], [0, 20, 3]] }));
    let disc = call(&mut ctx, "sketch_get", json!({ "sketch": hole }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [disc], "operationType": "REMOVE", "endBound": "THROUGH_ALL", "symmetric": true } }));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "suggested_views": true }));
    let dxf = scratch("radial-hole.dxf");
    call(&mut ctx, "drawing_export", json!({ "drawing": d["drawing"], "format": "DXF", "file": dxf.to_str().unwrap() }));
    let text = String::from_utf8_lossy(&std::fs::read(&dxf).unwrap()).to_string();
    // (code, value) pairs: LINE entities on CENTERMARKS.
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let (mut kind, mut marks) = ("", 0);
    for p in lines.chunks(2).filter(|p| p.len() == 2) {
        match p[0] {
            "0" => kind = p[1],
            "8" if kind == "LINE" && p[1] == "CENTERMARKS" => marks += 1,
            _ => {}
        }
    }
    assert_eq!(marks, 12, "two marks of six lines");
}

/// Suggested views of a 50 mm part reopened from its file are 1:1 on ANSI A
/// (live 1857: pipeflange-steven, 50 × 50 × 28 mm; ours took 1:2 from the
/// fast box, enlarged by its curved faces' poles).
#[test]
fn suggested_views_scale_to_the_exact_extent() {
    let mut ctx = Ctx::new();
    let sketch = new_sketch(&mut ctx, "exact-extent.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "circle", "points": [[0, 0], [25, 0]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sketch }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "28 mm" } }));
    let edges: Vec<String> = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "circle" }))["edges"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().to_string()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": edges, "radius": "5 mm" } }));
    // Reopened: the shapes as the file keeps them.
    let mut ctx = Ctx::new();
    let path = std::env::temp_dir().join(format!("noshape-ops-e2e-{}", std::process::id())).join("exact-extent.noshape");
    call(&mut ctx, "document_open", json!({ "path": path.to_str().unwrap() }));
    let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ANSI_A_INCH.dwt", "suggested_views": true }));
    let scales: Vec<f64> = d["views"].as_array().unwrap().iter().map(|v| num(&v["scale"])).collect();
    assert_eq!(scales, [1.0; 4], "{d}");
}

/// A part of a flexible subassembly instance moved in its parent (assembly_transform with the occurrence path, as the
/// parity translator places live's per-instance positions): that instance's part moves, the subassembly's other
/// instance and the subassembly itself don't; a locked instance's parts and deeper paths are refused by name.
#[test]
fn a_flexible_subassembly_instances_part_moves_in_its_parent_only() {
    let mut ctx = Ctx::new();
    let path = scratch("nested-move.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Sub" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Sub" }));
    call(&mut ctx, "assembly_transform", json!({ "tab": "Assembly 1", "instance": "Sub <2>", "translation": ["0 mm", "100 mm", "0 mm"] }));
    call(&mut ctx, "assembly_transform", json!({ "tab": "Assembly 1", "instance": "Sub <2> > Part 1 <1>", "translation": ["30 mm", "120 mm", "5 mm"], "axis": [0, 0, 1], "angle": "90 deg" }));
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    let at = |p: &str| -> Vec<f64> { list["occurrences"].as_array().unwrap().iter().find(|o| o["path"] == p).map(|o| (0..3).map(|i| num(&o["translation"][i])).collect()).unwrap_or_else(|| panic!("no occurrence {p}: {list}")) };
    let near = |a: &[f64], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6);
    assert!(near(&at("Sub <2> > Part 1 <1>"), [30.0, 120.0, 5.0]), "moved: {:?}", at("Sub <2> > Part 1 <1>"));
    assert!(near(&at("Sub <1> > Part 1 <1>"), [0.0, 0.0, 0.0]), "the other instance stays: {:?}", at("Sub <1> > Part 1 <1>"));
    let sub = call(&mut ctx, "assembly_list", json!({ "tab": "Sub" }));
    assert!(near(&(0..3).map(|i| num(&sub["instances"][0]["translation"][i])).collect::<Vec<_>>(), [0.0, 0.0, 0.0]), "the subassembly itself stays: {sub}");
    let e = try_call(&mut ctx, "assembly_transform", json!({ "tab": "Assembly 1", "instance": "Sub <1> > Part 1 <1> > Part 2 <1>", "translation": [0, 0, 0] })).unwrap_err().to_json();
    assert_eq!(e["param"], "instance", "{e}");
    // One level up, the positions come along: Assembly 1 inside another assembly.
    call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Top" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Top", "source": "Assembly 1" }));
    let top = call(&mut ctx, "assembly_list", json!({ "tab": "Top" }));
    let at = |p: &str| -> Vec<f64> { top["occurrences"].as_array().unwrap().iter().find(|o| o["path"] == p).map(|o| (0..3).map(|i| num(&o["translation"][i])).collect()).unwrap_or_else(|| panic!("no occurrence {p}: {top}")) };
    assert!(near(&at("Assembly 1 <1> > Sub <2> > Part 1 <1>"), [30.0, 120.0, 5.0]), "moved in Top: {:?}", at("Assembly 1 <1> > Sub <2> > Part 1 <1>"));
    assert!(near(&at("Assembly 1 <1> > Sub <1> > Part 1 <1>"), [0.0, 0.0, 0.0]), "the other instance in Top: {:?}", at("Assembly 1 <1> > Sub <1> > Part 1 <1>"));
}

/// A mate to a part inside a subassembly, as the app makes one: the part named by its occurrence path
/// ("Sub <1> > Part 1 <1>", Onshape's notation) in assembly_entities, assembly_connectors and assembly_mate. The top
/// block's top face Fastened to the nested block's bottom face lifts the subassembly onto it.
#[test]
fn a_mate_reaches_a_part_inside_a_subassembly() {
    let mut ctx = Ctx::new();
    let path = scratch("nested-mate.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "circle", "points": [[70, 10], [75, 10]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 2" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Sub" }));
    call(&mut ctx, "assembly_set_state", json!({ "tab": "Assembly 1", "instances": ["Part 1 <1>"], "fixed": true }));
    let face = |ctx: &mut Ctx, instance: &str, z: f64| call(ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": instance, "kind": "faces", "normal": [0, 0, z] }))["faces"][0].clone();
    let top = face(&mut ctx, "Part 1 <1>", 1.0)["id"].as_str().unwrap().to_string();
    let nested = "Sub <1> > Part 1 <1>";
    let bottom = face(&mut ctx, nested, -1.0);
    // Only the nested block's faces: its bottom at z 0, as inserted.
    assert!(num(&bottom["centroid"][2]).abs() < 1e-6 && num(&bottom["centroid"][0]) < 40.0, "{bottom}");
    let cands = call(&mut ctx, "assembly_connectors", json!({ "tab": "Assembly 1", "instance": nested, "entity": bottom["id"] }));
    assert_eq!(cands["connectors"][0]["instance"], nested, "{cands}");
    let m = call(&mut ctx, "assembly_mate", json!({ "tab": "Assembly 1", "type": "fastened", "a": { "instance": nested, "entity": bottom["id"] }, "b": { "instance": "Part 1 <1>", "entity": top } }));
    assert_eq!(m["mate_ok"], true, "{m}");
    let lifted = face(&mut ctx, nested, -1.0);
    assert!((num(&lifted["centroid"][2]) - 10.0).abs() < 1e-6, "the nested block's bottom on the top block: {lifted}");
    // assembly_list's occurrences: the nested ones by path, where their Part Studio coordinates are (lifted 10 mm).
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    let occ = |p: &str| list["occurrences"].as_array().unwrap().iter().find(|o| o["path"] == p).cloned().unwrap_or_else(|| panic!("no occurrence {p}: {list}"));
    assert!((num(&occ(nested)["translation"][2]) - 10.0).abs() < 1e-6, "{}", occ(nested));
    assert!(num(&occ("Part 1 <1>")["translation"][2]).abs() < 1e-6);
    assert_eq!(list["occurrences"].as_array().unwrap().len(), 3, "{list}");
    let e = try_call(&mut ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": "Part 1 <1> > Part 2 <1>", "kind": "faces" })).unwrap_err().to_json();
    assert_eq!(e["param"], "instance", "{e}");
}

/// A drawing view showing an exploded view keeps showing it when an earlier one is deleted, and a named position
/// when the list is reordered (the view refers to them by position: they're re-pointed in the same undo step).
#[test]
fn a_drawing_view_keeps_its_exploded_view_and_named_position() {
    let mut ctx = Ctx::new();
    let path = scratch("drawing-explode-ref.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm" } }));
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
    for _ in 0..2 {
        call(&mut ctx, "assembly_exploded_view", json!({ "action": "add" }));
        call(&mut ctx, "assembly_named_position", json!({ "action": "add" }));
    }
    let made = call(&mut ctx, "drawing_create", json!({ "source": "Assembly 1", "template": "ISO A3" }));
    let drawing = made["drawing"].as_str().unwrap().to_string();
    call(&mut ctx, "drawing_insert_view", json!({ "drawing": drawing, "source": "Assembly 1", "position": [100, 120], "explode_position": "Explode 2" }));
    call(&mut ctx, "drawing_insert_view", json!({ "drawing": drawing, "source": "Assembly 1", "position": [250, 120], "explode_position": "Named position 1" }));
    let views = |ctx: &mut Ctx| -> Vec<(Option<usize>, Option<usize>)> {
        let doc = ctx.doc(path.to_str()).unwrap().doc().clone();
        doc.tabs.iter().filter_map(|t| match &t.content { noshape_model::TabContent::Drawing(d) => Some(d), _ => None }).flat_map(|d| d.sheets.iter().flat_map(|s| s.views.iter())).map(|v| (v.explode, v.named_position)).collect()
    };
    assert_eq!(views(&mut ctx), vec![(Some(1), None), (None, Some(0))]);
    call(&mut ctx, "assembly_exploded_view", json!({ "action": "delete", "view": "Explode 1" }));
    call(&mut ctx, "assembly_named_position", json!({ "action": "move_down", "position": "Named position 1" }));
    assert_eq!(views(&mut ctx), vec![(Some(0), None), (None, Some(1))]);
    // One undo step each: the view follows back.
    call(&mut ctx, "history_undo", json!({}));
    assert_eq!(views(&mut ctx), vec![(Some(0), None), (None, Some(0))]);
}

/// The BOM in configurations: a part's configured Part number per configuration (two instances, Small and Large, are
/// two rows with their own numbers), and a configured subassembly (its second part suppressed in "Small") walked in each
/// instance's configuration (two rows, two parts and one).
#[test]
fn the_bom_follows_configurations() {
    let mut ctx = Ctx::new();
    let path = scratch("bom-configurations.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm" } }));
    // Part Studio 1: Size (Small, Large); Part 1's Part number configured.
    call(&mut ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "list", "name": "Size" }));
    call(&mut ctx, "config_edit_option", json!({ "tab": "Part Studio 1", "input": "Size", "option": "Default", "name": "Small" }));
    call(&mut ctx, "config_add_option", json!({ "tab": "Part Studio 1", "input": "Size", "name": "Large" }));
    let cfg = call(&mut ctx, "config_list", json!({ "tab": "Part Studio 1" }));
    let input = cfg["inputs"][0]["id"].as_str().unwrap().to_string();
    let option = |name: &str| cfg["inputs"][0]["configurations"].as_array().unwrap().iter().find(|o| o["name"] == name).map(|o| o["id"].as_str().unwrap().to_string()).unwrap_or_else(|| panic!("{cfg}"));
    let (small, large) = (option("Small"), option("Large"));
    call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": "Part 1", "action": "add_property", "property": "Part number" }));
    call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": "Part 1", "action": "set", "property": "Part number", "value": "PN-S", "configuration": { input.clone(): small } }));
    call(&mut ctx, "part_configured_properties", json!({ "tab": "Part Studio 1", "part": "Part 1", "action": "set", "property": "Part number", "value": "PN-L", "configuration": { input.clone(): large.clone() } }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_set_configuration", json!({ "tab": "Assembly 1", "instance": "Part 1 <2>", "configuration": { input.clone(): large } }));
    let bom = call(&mut ctx, "assembly_bom", json!({ "tab": "Assembly 1", "type": "flattened", "columns": ["Item", "Quantity", "Part number"] }));
    let numbers: Vec<(String, u64)> = bom["rows"].as_array().unwrap().iter().map(|r| (r["cells"]["Part number"].as_str().unwrap_or_default().to_string(), r["quantity"].as_u64().unwrap())).collect();
    assert_eq!(numbers, vec![("PN-S".to_string(), 1), ("PN-L".to_string(), 1)], "{bom}");

    // Sub: two parts, the second suppressed in its configuration "Small" (Variant: Full, Small).
    call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "config_add_input", json!({ "tab": "Sub", "kind": "list", "name": "Variant" }));
    call(&mut ctx, "config_edit_option", json!({ "tab": "Sub", "input": "Variant", "option": "Default", "name": "Full" }));
    call(&mut ctx, "config_add_option", json!({ "tab": "Sub", "input": "Variant", "name": "Small" }));
    call(&mut ctx, "config_configure", json!({ "tab": "Sub", "instance": "Part 1 <2>", "param": "suppressed", "input": "Variant" }));
    call(&mut ctx, "config_set_value", json!({ "tab": "Sub", "instance": "Part 1 <2>", "param": "suppressed", "configuration": "Small", "value": true }));
    let sub_cfg = call(&mut ctx, "config_list", json!({ "tab": "Sub" }));
    let variant = sub_cfg["inputs"][0]["id"].as_str().unwrap().to_string();
    let sub_small = sub_cfg["inputs"][0]["configurations"].as_array().unwrap().iter().find(|o| o["name"] == "Small").map(|o| o["id"].as_str().unwrap().to_string()).unwrap_or_else(|| panic!("{sub_cfg}"));
    call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Top" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Top", "source": "Sub" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Top", "source": "Sub" }));
    call(&mut ctx, "assembly_set_configuration", json!({ "tab": "Top", "instance": "Sub <2>", "configuration": { variant: sub_small } }));
    let bom = call(&mut ctx, "assembly_bom", json!({ "tab": "Top", "type": "structured", "columns": ["Item", "Quantity"] }));
    let rows: Vec<(u64, String, u64)> = bom["rows"].as_array().unwrap().iter().map(|r| (r["depth"].as_u64().unwrap(), r["name"].as_str().unwrap().to_string(), r["quantity"].as_u64().unwrap())).collect();
    assert_eq!(rows, vec![(0, "Sub".to_string(), 1), (1, "Part 1".to_string(), 2), (0, "Sub".to_string(), 1), (1, "Part 1".to_string(), 1)], "{bom}");
}

/// Live 2458: a configuration variable's `#` is its FeatureScript id, not its shown name. Rename changes only the name
/// ("size (#len)"), so its uses keep working, in a Part Studio and an Assembly alike; only Edit FeatureScript IDs
/// changes the id, and that breaks them (they are not rewritten); an Assembly's ids can't be edited.
#[test]
fn renaming_a_configuration_variable_keeps_its_uses() {
    let mut ctx = Ctx::new();
    let path = scratch("config-var-rename.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let len = call(&mut ctx, "config_add_input", json!({ "kind": "variable", "name": "len", "value": "10 mm" }))["input"].as_str().unwrap().to_string();
    assert_eq!(len, "len");
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "#len", "operationType": "NEW" } }));
    let volume = |ctx: &mut Ctx| num(&call(ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"][0]["volume"]);
    assert!((volume(&mut ctx) - 8000.0).abs() < 1e-6);
    call(&mut ctx, "config_edit_input", json!({ "input": "len", "name": "size" }));
    let l = call(&mut ctx, "config_list", json!({}));
    let input = l["inputs"].as_array().unwrap().iter().find(|i| i["id"] == "len").unwrap_or_else(|| panic!("{l}"));
    assert_eq!(input["name"], "size", "{l}");
    assert!((volume(&mut ctx) - 8000.0).abs() < 1e-6, "#len still resolves after the rename");
    let f = call(&mut ctx, "feature_list", json!({ "tab": "Part Studio 1" }));
    assert_eq!(f["features"].as_array().unwrap().iter().find(|x| x["name"] == "Extrude 1").unwrap()["status"], "Ok", "{f}");
    // The id changed: Extrude 1 keeps `#len` and fails.
    call(&mut ctx, "config_edit_ids", json!({ "input": "len", "id": "size" }));
    let f = call(&mut ctx, "feature_list", json!({ "tab": "Part Studio 1" }));
    let ext = f["features"].as_array().unwrap().iter().find(|x| x["name"] == "Extrude 1").unwrap().clone();
    assert_eq!(ext["status"], "Error", "{ext}");
    let fe = call(&mut ctx, "feature_get", json!({ "tab": "Part Studio 1", "feature": "Extrude 1" }));
    assert!(fe.to_string().contains("#len"), "the use isn't rewritten: {fe}");

    // An Assembly: a variable `gap` used by a mate offset, renamed `space`.
    call(&mut ctx, "config_edit_ids", json!({ "input": "size", "id": "len" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_set_state", json!({ "tab": "Assembly 1", "instances": ["Part 1 <1>"], "fixed": true }));
    let gap = call(&mut ctx, "config_add_input", json!({ "tab": "Assembly 1", "kind": "variable", "name": "gap", "value": "5 mm" }))["input"].as_str().unwrap().to_string();
    assert_eq!(gap, "gap", "its id from its name");
    let face = |ctx: &mut Ctx, instance: &str, z: f64| call(ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": instance, "kind": "faces", "normal": [0, 0, z] }))["faces"][0].clone();
    let (top, bottom) = (face(&mut ctx, "Part 1 <1>", 1.0)["id"].clone(), face(&mut ctx, "Part 1 <2>", -1.0)["id"].clone());
    call(&mut ctx, "assembly_mate", json!({ "tab": "Assembly 1", "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": top }, "b": { "instance": "Part 1 <2>", "entity": bottom }, "offset": { "z": "#gap" } }));
    let lift = |ctx: &mut Ctx| num(&face(ctx, "Part 1 <2>", -1.0)["centroid"][2]) - 10.0;
    assert!((lift(&mut ctx).abs() - 5.0).abs() < 1e-6, "{}", lift(&mut ctx));
    let edges = call(&mut ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": "Part 1 <1>", "kind": "edges" }));
    let x_edge = edges["edges"].as_array().unwrap().iter().find(|e| e["curve"] == "line" && num(&e["direction"][0]).abs() > 0.99).unwrap()["id"].clone();
    call(&mut ctx, "assembly_pattern", json!({ "tab": "Assembly 1", "kind": "linear", "seeds": ["Part 1 <2>"], "direction": { "instance": "Part 1 <1>", "entity": x_edge }, "distance": "#gap * 4" }));
    let patterned_x = |ctx: &mut Ctx| {
        let l = call(ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
        l["instances"].as_array().unwrap().iter().map(|i| num(&i["translation"][0]).abs()).fold(0.0, f64::max)
    };
    assert!((patterned_x(&mut ctx) - 20.0).abs() < 1e-6, "{}", patterned_x(&mut ctx));
    call(&mut ctx, "config_edit_input", json!({ "tab": "Assembly 1", "input": "gap", "name": "space" }));
    let l = call(&mut ctx, "config_list", json!({ "tab": "Assembly 1" }));
    assert_eq!(l["inputs"][0]["name"], "space", "{l}");
    let m = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    assert_eq!(m["mates"][0]["ok"], true, "the mate's #gap still resolves: {m}");
    assert!((lift(&mut ctx).abs() - 5.0).abs() < 1e-6, "{}", lift(&mut ctx));
    assert_eq!(m["patterns"][0]["distance"], "#gap * 4", "{m}");
    assert!((patterned_x(&mut ctx) - 20.0).abs() < 1e-6, "the pattern's #gap still resolves: {}", patterned_x(&mut ctx));
    let e = try_call(&mut ctx, "config_edit_ids", json!({ "tab": "Assembly 1", "input": "gap", "id": "space" })).unwrap_err();
    assert!(e.param.as_deref() == Some("tab") && e.message.contains("no Edit FeatureScript IDs"), "an Assembly's ids can't be edited: {e:?}");
}

/// ASM-CONFIG-VARIABLE-NOOP: an assembly's configuration variable is a `#name` for mates, in each configuration.
#[test]
fn an_assembly_configuration_variable_drives_a_mate_offset() {
    let mut ctx = Ctx::new();
    let path = scratch("asm-config-variable.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "10 mm", "operationType": "NEW" } }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1" }));
    call(&mut ctx, "assembly_set_state", json!({ "tab": "Assembly 1", "instances": ["Part Studio 1 <1>"], "fixed": true }));
    let i = call(&mut ctx, "config_add_input", json!({ "tab": "Assembly 1", "kind": "variable", "name": "gap", "value": "5 mm" }));
    let input = i["input"].as_str().unwrap().to_string();
    let face = |ctx: &mut Ctx, instance: &str, z: f64| call(ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": instance, "kind": "faces", "normal": [0, 0, z] }))["faces"][0].clone();
    let top = face(&mut ctx, "Part Studio 1 <1>", 1.0)["id"].clone();
    let bottom = face(&mut ctx, "Part Studio 1 <2>", -1.0)["id"].clone();
    let m = call(&mut ctx, "assembly_mate", json!({ "tab": "Assembly 1", "type": "fastened", "a": { "instance": "Part Studio 1 <1>", "entity": top }, "b": { "instance": "Part Studio 1 <2>", "entity": bottom }, "offset": { "z": "#gap" } }));
    assert_eq!(m["mate_ok"], true, "{m}");
    // The moved block's bottom: 10 mm up on the fixed block's top, then the offset.
    let gap = |ctx: &mut Ctx| (num(&face(ctx, "Part Studio 1 <2>", -1.0)["centroid"][2]) - 10.0).abs();
    assert!((gap(&mut ctx) - 5.0).abs() < 1e-6, "the default 5 mm: {}", gap(&mut ctx));
    call(&mut ctx, "config_activate", json!({ "tab": "Assembly 1", "values": { input.clone(): "12 mm" } }));
    assert!((gap(&mut ctx) - 12.0).abs() < 1e-6, "12 mm: {}", gap(&mut ctx));
    call(&mut ctx, "config_activate", json!({ "tab": "Assembly 1", "reset": true }));
    assert!((gap(&mut ctx) - 5.0).abs() < 1e-6, "back to the default: {}", gap(&mut ctx));
    // The second connector moved 4 mm along its own Z (the mate dialog's connector Move): the part shifts 4 mm more.
    call(&mut ctx, "assembly_delete", json!({ "tab": "Assembly 1", "mates": ["Fastened 1"] }));
    let m = call(&mut ctx, "assembly_mate", json!({ "tab": "Assembly 1", "type": "fastened", "a": { "instance": "Part Studio 1 <1>", "entity": top }, "b": { "instance": "Part Studio 1 <2>", "entity": bottom, "move": { "z": "4 mm" } }, "offset": { "z": "#gap" } }));
    assert_eq!(m["mate_ok"], true, "{m}");
    assert!(((gap(&mut ctx) - 5.0).abs() - 4.0).abs() < 1e-6, "moved 4 mm: {}", gap(&mut ctx));
    let e = try_call(&mut ctx, "assembly_mate", json!({ "tab": "Assembly 1", "type": "fastened", "a": { "instance": "Part Studio 1 <1>", "entity": top }, "b": { "instance": "Part Studio 1 <2>", "entity": bottom, "move": { "rotation_axis": "w" } } })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("b.move.rotation_axis"), "{e:?}");
}

/// ASM-API-GAPS: the Properties dialog's Exclude from all BOMs, Unit of measure and Not revision managed through
/// part_set_properties, as the BOM reads them.
#[test]
fn part_properties_reach_the_bom() {
    let mut ctx = Ctx::new();
    let path = scratch("part-props-bom.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "circle", "points": [[70, 10], [75, 10]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    call(&mut ctx, "part_set_properties", json!({ "tab": "Part Studio 1", "part": "Part 1", "unit_of_measure": "kilogram", "not_revision_managed": true }));
    call(&mut ctx, "part_set_properties", json!({ "tab": "Part Studio 1", "part": "Part 2", "exclude_from_bom": true }));
    let e = try_call(&mut ctx, "part_set_properties", json!({ "tab": "Part Studio 1", "part": "Part 1", "unit_of_measure": "furlong" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("unit_of_measure"));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 2" }));
    let bom = call(&mut ctx, "assembly_bom", json!({ "tab": "Assembly 1", "type": "flattened", "columns": ["Item", "Quantity", "Name", "Unit of measure", "Not revision managed"] }));
    let rows = bom["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "Part 2 is excluded from all BOMs: {bom}");
    assert_eq!(rows[0]["cells"]["Name"], "Part 1", "{bom}");
    assert_eq!(rows[0]["cells"]["Unit of measure"], "Kilogram", "{bom}");
    assert_eq!(rows[0]["cells"]["Not revision managed"], "true", "{bom}");
}

/// ASM-API-GAPS: an assembly's Properties… (its name, part number, Subassembly BOM behavior, Exclude from all BOMs),
/// as the parent's BOM reads them.
#[test]
fn assembly_properties_set_the_subassemblys_bom_row() {
    let mut ctx = Ctx::new();
    let path = scratch("asm-props.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "10 mm" } }));
    call(&mut ctx, "tab_create", json!({ "kind": "assembly", "name": "Sub" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Sub", "source": "Part Studio 1" }));
    let p = call(&mut ctx, "assembly_properties", json!({ "tab": "Sub", "name": "Frame", "part_number": "FR-1", "bom_behavior": "Show Assembly only" }));
    assert_eq!(p["properties"]["part_number"], "FR-1", "{p}");
    assert_eq!(p["properties"]["state"], "In progress", "{p}");
    let last = call(&mut ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].clone();
    assert_eq!(last, "Properties : Frame");
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Frame" }));
    let bom = call(&mut ctx, "assembly_bom", json!({ "tab": "Assembly 1", "type": "structured", "columns": ["Item", "Quantity", "Name", "Part number"] }));
    let rows: Vec<(String, String)> = bom["rows"].as_array().unwrap().iter().map(|r| (r["name"].as_str().unwrap().to_string(), r["cells"]["Part number"].as_str().unwrap_or_default().to_string())).collect();
    assert_eq!(rows, vec![("Frame".to_string(), "FR-1".to_string())], "Show Assembly only: no part rows under it: {bom}");
    call(&mut ctx, "assembly_properties", json!({ "tab": "Frame", "exclude_from_bom": true }));
    let bom = call(&mut ctx, "assembly_bom", json!({ "tab": "Assembly 1", "type": "structured" }));
    assert!(bom["rows"].as_array().unwrap().is_empty(), "excluded: {bom}");
    let e = try_call(&mut ctx, "assembly_properties", json!({ "tab": "Frame", "bom_behavior": "nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("bom_behavior"));
}

/// ASM-API-GAPS: Measure and Mass and section properties in an assembly.
#[test]
fn assembly_measure_and_mass_properties() {
    let mut ctx = Ctx::new();
    let path = scratch("asm-measure-mass.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }] }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"]], "depth": "10 mm" } }));
    call(&mut ctx, "part_set_properties", json!({ "tab": "Part Studio 1", "part": "Part 1", "mass_override": 2.0 }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_set_state", json!({ "tab": "Assembly 1", "instances": ["Part 1 <1>"], "fixed": true }));
    let face = |ctx: &mut Ctx, instance: &str, z: f64| call(ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": instance, "kind": "faces", "normal": [0, 0, z] }))["faces"][0]["id"].clone();
    let (top, bottom) = (face(&mut ctx, "Part 1 <1>", 1.0), face(&mut ctx, "Part 1 <2>", -1.0));
    call(&mut ctx, "assembly_mate", json!({ "tab": "Assembly 1", "type": "fastened", "a": { "instance": "Part 1 <1>", "entity": top }, "b": { "instance": "Part 1 <2>", "entity": bottom }, "offset": { "z": "5 mm" } }));
    let m = call(&mut ctx, "assembly_measure", json!({ "tab": "Assembly 1", "entities": [{ "instance": "Part 1 <1>", "entity": top }] }));
    assert!((num(&m["measurements"][0]["value"]["area"]) - 800.0).abs() < 1e-6, "{m}");
    let m = call(&mut ctx, "assembly_measure", json!({ "tab": "Assembly 1", "entities": [{ "instance": "Part 1 <1>", "entity": top }, { "instance": "Part 1 <2>", "entity": bottom }] }));
    let gap = m["measurements"].as_array().unwrap().iter().find_map(|x| x["value"]["distance"].as_f64()).unwrap_or_else(|| panic!("{m}"));
    assert!((gap.abs() - 5.0).abs() < 1e-6, "the fastened offset: {m}");
    let e = try_call(&mut ctx, "assembly_measure", json!({ "tab": "Assembly 1", "entities": [{ "instance": "Part 1 <1>", "entity": "face:nope" }] })).unwrap_err();
    assert!(e.param.as_deref().is_some_and(|p| p.starts_with("entities[0]")), "{e:?}");
    // Two 2 kg blocks (mass overrides), centered between them; then the assembly's own override.
    let mp = call(&mut ctx, "assembly_mass_properties", json!({ "tab": "Assembly 1" }));
    assert!((num(&mp["mass_kg"]) - 4.0).abs() < 1e-9, "{mp}");
    assert!((num(&mp["volume"]) - 16000.0).abs() < 1e-6, "{mp}");
    let one = call(&mut ctx, "assembly_mass_properties", json!({ "tab": "Assembly 1", "instances": ["Part 1 <1>"] }));
    assert!((num(&one["mass_kg"]) - 2.0).abs() < 1e-9, "{one}");
    call(&mut ctx, "assembly_properties", json!({ "tab": "Assembly 1", "mass_override": 7.5 }));
    let mp = call(&mut ctx, "assembly_mass_properties", json!({ "tab": "Assembly 1" }));
    assert!((num(&mp["mass_kg"]) - 7.5).abs() < 1e-9 && mp["mass_overridden"] == true, "{mp}");
}

/// ASM-API-GAPS: the Reference manager through the API: Update all references moves an other-document instance
/// to its latest version, Selective update to a named one, Change to version pins this document's own parts.
#[test]
fn assembly_update_references_moves_versions() {
    let mut ctx = Ctx::new();
    two_part_assembly(&mut ctx, "update-refs-lib.noshape");
    let lib = std::env::temp_dir().join(format!("noshape-ops-e2e-{}", std::process::id())).join("update-refs-lib.noshape");
    let libs = lib.to_str().unwrap().to_string();
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let top = scratch("update-refs-top.noshape");
    call(&mut ctx, "document_create", json!({ "path": top.to_str().unwrap() }));
    let tops = top.to_str().unwrap().to_string();
    let r = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 2", "document": libs, "version": "V1" }));
    let inst = r["instance"].clone();
    let mv = |ctx: &mut Ctx| call(ctx, "assembly_list", json!({ "doc": tops }))["instances"].as_array().unwrap().iter().find(|i| i["id"] == inst).unwrap()["source"]["version_of"]["microversion"].clone();
    let v1 = mv(&mut ctx);
    // Nothing newer yet.
    let u = call(&mut ctx, "assembly_update_references", json!({ "doc": tops }));
    assert_eq!(u["updated"], 0, "{u}");
    call(&mut ctx, "sketch_create", json!({ "doc": libs, "plane": "plane:front", "name": "Later" }));
    call(&mut ctx, "history_create_version", json!({ "doc": libs, "name": "V2" }));
    let u = call(&mut ctx, "assembly_update_references", json!({ "doc": tops }));
    assert_eq!(u["updated"], 1, "{u}");
    assert_ne!(mv(&mut ctx), v1, "moved to V2");
    let last = call(&mut ctx, "history_list", json!({ "doc": tops, "limit": 1 }))["history"][0]["description"].clone();
    assert_eq!(last, "Assembly 1 :: Update to V2");
    let u = call(&mut ctx, "assembly_update_references", json!({ "doc": tops, "document": "update-refs-lib", "version": "V1" }));
    assert_eq!(u["updated"], 1, "{u}");
    assert_eq!(mv(&mut ctx), v1, "back to V1");
    let e = try_call(&mut ctx, "assembly_update_references", json!({ "doc": tops, "version": "V9" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{e:?}");
    // Pin reference (live 2464): updates leave it on V1 until Unpin reference.
    let p = call(&mut ctx, "assembly_pin_reference", json!({ "doc": tops, "instance": inst }));
    assert_eq!(p["pinned"], true, "{p}");
    let listed = call(&mut ctx, "assembly_list", json!({ "doc": tops }))["instances"].as_array().unwrap().iter().find(|i| i["id"] == inst).unwrap()["reference_pinned"].clone();
    assert_eq!(listed, true);
    let u = call(&mut ctx, "assembly_update_references", json!({ "doc": tops }));
    assert_eq!((u["updated"].clone(), mv(&mut ctx)), (json!(0), v1.clone()), "pinned stays on V1: {u}");
    call(&mut ctx, "assembly_pin_reference", json!({ "doc": tops, "instance": inst, "pinned": false }));
    let u = call(&mut ctx, "assembly_update_references", json!({ "doc": tops }));
    assert_eq!(u["updated"], 1, "unpinned, it moves: {u}");
}

/// ASM-STD-AUTOSIZE-HOLE (live 2204): Auto-size into a tapped Hole sizes by its thread: M6 coarse (Ø5 tap drill)
/// gives M6, not M5; a fine M6x0.75 finds no size of the coarse-only ISO 4762, with live's message.
#[test]
fn standard_content_auto_sizes_by_a_tapped_holes_thread() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "std-tapped.noshape", "plane:top");
    call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[-20, -15], [20, 15]] }));
    let region = call(&mut ctx, "sketch_get", json!({ "sketch": sk }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let s2 = call(&mut ctx, "sketch_create", json!({ "plane": { "query": { "kind": "faces", "normal": [0, 0, 1] } } }))["sketch"].as_str().unwrap().to_string();
    let p = call(&mut ctx, "sketch_draw", json!({ "sketch": s2, "tool": "point", "points": [[5, 0, 10]] }));
    let pt: Vec<Value> = p["created"].as_array().unwrap().iter().filter(|x| x["type"] == "point").map(|x| x["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "hole", "params": { "locations": pt, "isoHoleTableEx": ["Tapped", "Straight tap", "M6"], "endStyleV2": "THROUGH" } }));
    let block = call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1" }))["instance"].as_str().unwrap().to_string();
    call(&mut ctx, "assembly_set_state", json!({ "instances": [block], "fixed": true }));
    let hole = |ctx: &mut Ctx| call(ctx, "assembly_entities", json!({ "instance": block, "kind": "faces", "surface": "cylinder" }))["faces"][0]["id"].clone();
    let h = hole(&mut ctx);
    let s = call(&mut ctx, "assembly_insert_standard", json!({ "component": "Hex socket head cap screw ISO 4762", "holes": [{ "instance": block, "entity": h }], "auto_size": true }));
    assert_eq!(s["spec"]["size"], "M6", "the tapped hole's thread, not its Ø5 drill: {s}");
    call(&mut ctx, "feature_edit", json!({ "tab": "Part Studio 1", "feature": "Hole 1", "params": { "isoHoleTableEx": ["Tapped", "Straight tap", "M6", "0.75 mm (Fine)"] } }));
    let h = hole(&mut ctx);
    let e = try_call(&mut ctx, "assembly_insert_standard", json!({ "component": "Hex socket head cap screw ISO 4762", "holes": [{ "instance": block, "entity": h }], "auto_size": true })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("auto_size"), "{e:?}");
    assert!(e.message.starts_with("Unable to find matching size for selection."), "{e:?}");
}

/// ASM-CONFIGURE-PATTERNS (live 2209): a pattern's Instance count and a mate connector's X translation configured;
/// Long has five pattern instances (its copies placed by the pattern, no new mates) and the connector moved.
#[test]
fn assembly_patterns_and_connectors_are_configured() {
    let mut ctx = Ctx::new();
    let path = scratch("asm-config-patterns.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "circle", "points": [[70, 10], [75, 10]] }] }));
    let regions: Vec<Value> = d["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": regions, "depth": "10 mm", "operationType": "NEW" } }));
    let block = call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 1" }))["instance"].clone();
    let pin = call(&mut ctx, "assembly_insert", json!({ "tab": "Assembly 1", "source": "Part Studio 1", "part": "Part 2" }))["instance"].clone();
    call(&mut ctx, "assembly_set_state", json!({ "tab": "Assembly 1", "instances": [block], "fixed": true }));
    let edges = call(&mut ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": block, "kind": "edges" }));
    let x_edge = edges["edges"].as_array().unwrap().iter().find(|e| e["curve"] == "line" && num(&e["direction"][0]).abs() > 0.99).unwrap()["id"].clone();
    call(&mut ctx, "assembly_pattern", json!({ "tab": "Assembly 1", "kind": "linear", "seeds": [pin], "direction": { "instance": block, "entity": x_edge }, "distance": "60 mm" }));
    let top = call(&mut ctx, "assembly_entities", json!({ "tab": "Assembly 1", "instance": block, "kind": "faces", "normal": [0, 0, 1] }))["faces"][0]["id"].clone();
    call(&mut ctx, "assembly_mate_connector", json!({ "tab": "Assembly 1", "on": { "instance": block, "entity": top }, "offset": { "x": "10 mm" } }));
    let i = call(&mut ctx, "config_add_input", json!({ "tab": "Assembly 1", "kind": "list" }));
    let input = i["input"].as_str().unwrap().to_string();
    call(&mut ctx, "config_add_option", json!({ "tab": "Assembly 1", "input": input, "name": "Long" }));
    call(&mut ctx, "config_configure", json!({ "tab": "Assembly 1", "pattern": "Linear pattern 1", "param": "count", "input": input }));
    let last = |ctx: &mut Ctx| call(ctx, "history_list", json!({ "limit": 1 }))["history"][0]["description"].as_str().unwrap().to_string();
    assert_eq!(last(&mut ctx), "Assembly 1 :: Configure Linear pattern 1");
    call(&mut ctx, "config_set_value", json!({ "tab": "Assembly 1", "pattern": "Linear pattern 1", "param": "count", "configuration": "Long", "value": "5" }));
    call(&mut ctx, "config_configure", json!({ "tab": "Assembly 1", "connector": "Mate connector 1", "param": "translation_x", "input": input }));
    call(&mut ctx, "config_set_value", json!({ "tab": "Assembly 1", "connector": "Mate connector 1", "param": "translation_x", "configuration": "Long", "value": "25 mm" }));
    let e = try_call(&mut ctx, "config_configure", json!({ "tab": "Assembly 1", "pattern": "Linear pattern 1", "param": "angle", "input": input })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("param"), "{e:?}");
    let l = call(&mut ctx, "config_list", json!({ "tab": "Assembly 1" }));
    assert!(l["configured"].as_array().unwrap().iter().any(|c| c["pattern"].is_string() && c["param"] == "count"), "{l}");
    let pins = |ctx: &mut Ctx| call(ctx, "assembly_list", json!({ "tab": "Assembly 1" }))["instances"].as_array().unwrap().len() - 1;
    assert_eq!(pins(&mut ctx), 2);
    call(&mut ctx, "config_activate", json!({ "tab": "Assembly 1", "values": { input.clone(): "Long" } }));
    assert_eq!(pins(&mut ctx), 5, "Long: five pins");
    let list = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }));
    assert_eq!(list["mates"].as_array().unwrap().len(), 0, "{list}");
    call(&mut ctx, "config_activate", json!({ "tab": "Assembly 1", "reset": true }));
    assert_eq!(pins(&mut ctx), 2);
}

/// document_create puts a document where the app lists documents unless given a folder (user 11-07: an agent's
/// document was in the documents list but not in the folder the page names): no path, a new "Untitled <n>" there; a
/// bare name, that name there (".noshape" added); a full path, that path. The reply's path is always absolute.
#[test]
fn document_create_defaults_to_the_documents_folder() {
    let documents = std::env::temp_dir().join(format!("noshape-ops-e2e-documents-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&documents);
    let mut ctx = Ctx::new();
    ctx.documents = Some(documents.clone());
    let made = call(&mut ctx, "document_create", json!({}));
    assert_eq!(std::path::Path::new(made["path"].as_str().unwrap()), documents.join("Untitled 1.noshape"), "{made}");
    let made = call(&mut ctx, "document_create", json!({}));
    assert_eq!(std::path::Path::new(made["path"].as_str().unwrap()), documents.join("Untitled 2.noshape"), "{made}");
    let made = call(&mut ctx, "document_create", json!({ "path": "AppleWatchDock" }));
    assert_eq!(std::path::Path::new(made["path"].as_str().unwrap()), documents.join("AppleWatchDock.noshape"), "{made}");
    assert!(documents.join("AppleWatchDock.noshape").exists());
    let elsewhere = scratch("document-create-elsewhere.noshape");
    let made = call(&mut ctx, "document_create", json!({ "path": elsewhere.to_str().unwrap() }));
    assert_eq!(std::path::Path::new(made["path"].as_str().unwrap()), elsewhere, "{made}");
    assert!(std::path::Path::new(made["path"].as_str().unwrap()).is_absolute());
    drop(ctx);
    let _ = std::fs::remove_dir_all(&documents);
}

/// qa's agent session v0.4.36: Symmetric about "origin" (a point) is refused, saying what it takes and how to centre a
/// shape on the origin instead.
#[test]
fn symmetric_about_a_point_says_what_it_takes() {
    let mut ctx = Ctx::new();
    let sk = new_sketch(&mut ctx, "symmetric-origin.noshape", "plane:top");
    let r = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "shapes": [{ "tool": "corner_rectangle", "points": [[-30, -20], [30, 20]] }] }));
    let pts: Vec<Value> = r["created"].as_array().unwrap().iter().filter(|e| e["type"] == "point").map(|e| e["id"].clone()).collect();
    let e = try_call(&mut ctx, "sketch_constrain", json!({ "sketch": sk, "constraint": "symmetric", "entities": [pts[0], pts[2], "origin"] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("entities"), "{e:?}");
    assert!(e.message.contains("about a line") && e.message.contains("horizontal_distance"), "{e:?}");
}

/// A drawing view of another document's version (live 2303): Insert view › Other documents at V1; once that document
/// has a V2 the reference is behind (drawing_update_references lists it, document_update_references too); Pin
/// reference keeps it at V1 (no notice, Update leaves it); unpinned, Update moves its views to V2.
#[test]
fn drawing_view_of_another_documents_version() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "drawing_other_doc_lib.noshape");
    let lib = call(&mut ctx, "document_info", json!({}))["path"].as_str().unwrap().to_string();
    call(&mut ctx, "history_create_version", json!({ "name": "V1" }));
    let user = scratch("drawing_other_doc_user.noshape");
    let _ = std::fs::remove_file(format!("{}.noshape", user.display()));
    call(&mut ctx, "document_create", json!({ "path": user.to_str().unwrap() }));
    let d = call(&mut ctx, "drawing_create", json!({ "template": "ISO A3" }));
    let tab = d["drawing"].as_str().unwrap().to_string();
    let e = try_call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "document": lib, "source": "Part Studio 1", "position": [150, 150] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{e:?}");
    let e = try_call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "document": lib, "version": "V9", "source": "Part Studio 1", "position": [150, 150] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("version"), "{e:?}");
    let r = call(&mut ctx, "drawing_insert_view", json!({ "drawing": tab, "document": lib, "version": "V1", "source": "Part Studio 1", "position": [150, 150] }));
    let view = r["view"].as_u64().unwrap();
    let get = |ctx: &mut Ctx| call(ctx, "drawing_get", json!({ "drawing": tab, "edges": false }))["views"][0].clone();
    let v = get(&mut ctx);
    assert!(v["version"].as_str().is_some_and(|n| n.ends_with(" V1")), "{v}");
    let up = call(&mut ctx, "drawing_update_references", json!({ "drawing": tab }));
    assert_eq!((up["updated"].clone(), up["outdated"].clone()), (json!([]), json!([])), "nothing newer yet: {up}");
    // The other document moves on.
    call(&mut ctx, "feature_edit", json!({ "doc": lib, "feature": "Extrude 1", "params": { "depth": "30 mm" } }));
    call(&mut ctx, "history_create_version", json!({ "doc": lib, "name": "V2" }));
    let l = call(&mut ctx, "document_update_references", json!({ "doc": user.to_str().unwrap() }));
    assert_eq!((l["outdated"][0]["from"].as_str(), l["outdated"][0]["to"].as_str()), (Some("V1"), Some("V2")), "{l}");
    // Pin reference: kept at V1, no longer listed.
    call(&mut ctx, "drawing_pin_reference", json!({ "doc": user.to_str().unwrap(), "drawing": tab, "view": view }));
    assert_eq!(get(&mut ctx)["reference_pinned"], true);
    let l = call(&mut ctx, "document_update_references", json!({ "doc": user.to_str().unwrap() }));
    assert_eq!(l["outdated"], json!([]), "pinned: {l}");
    let up = call(&mut ctx, "drawing_update_references", json!({ "drawing": tab }));
    assert_eq!(up["updated"], json!([]), "{up}");
    // Unpinned: Update all moves it to V2.
    call(&mut ctx, "drawing_pin_reference", json!({ "drawing": tab, "view": view, "pin": false }));
    let up = call(&mut ctx, "drawing_update_references", json!({ "drawing": tab }));
    assert_eq!((up["updated"][0]["from"].as_str(), up["updated"][0]["to"].as_str()), (Some("V1"), Some("V2")), "{up}");
    assert_eq!(up["outdated"], json!([]), "{up}");
    assert!(get(&mut ctx)["version"].as_str().is_some_and(|n| n.ends_with(" V2")));
    // Back to V1 by name (Selective update), then a reference in this document can't be pinned.
    let up = call(&mut ctx, "drawing_update_references", json!({ "drawing": tab, "version": "V1" }));
    assert_eq!(up["updated"][0]["to"], "V1", "{up}");
    let e = try_call(&mut ctx, "drawing_update_references", json!({ "drawing": tab, "document": "nope" })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("document"), "{e:?}");
    let sw = call(&mut ctx, "drawing_switch_to_reference", json!({ "drawing": tab, "view": view }));
    assert_eq!(sw["version"], "V1", "{sw}");
}

/// STL presets as Onshape's facetter takes them (live 2319: its Medium cuts clamp-hms' 7.5 mm arcs into 0.085 mm
/// segments, a sag of 0.12 µm): the 5 mm hole's rim on the top face in about 2π·5 / (0.085·√(5/7.5)) ≈ 453 segments at
/// Medium, √2 more at Fine, √2 fewer at Coarse (lead 11-08: the same file sizes and smoothness as Onshape's).
#[test]
fn stl_presets_facet_as_onshapes() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "stl_presets.noshape");
    let rim = |ctx: &mut Ctx, resolution: &str| -> usize {
        let file = scratch(&format!("stl_presets_{resolution}.stl"));
        call(ctx, "export_parts", json!({ "format": "STL", "file": file.to_str().unwrap(), "units": "Millimeter", "resolution": resolution }));
        let b = std::fs::read(&file).unwrap();
        let n = u32::from_le_bytes(b[80..84].try_into().unwrap()) as usize;
        let mut pts = std::collections::HashSet::new();
        for i in 0..n {
            for k in 0..3 {
                let o = 84 + i * 50 + 12 + k * 12;
                let f = |j: usize| f32::from_le_bytes(b[o + 4 * j..o + 4 * j + 4].try_into().unwrap());
                let (x, y, z) = (f(0), f(1), f(2));
                if (z - 20.0).abs() < 1e-3 && ((x * x + y * y).sqrt() - 5.0).abs() < 1e-3 {
                    pts.insert((x.to_bits(), y.to_bits()));
                }
            }
        }
        pts.len()
    };
    let medium = rim(&mut ctx, "medium");
    assert!((medium as f64 - 453.0).abs() < 45.0, "Medium: {medium} rim points, Onshape's ≈ 453");
    let (coarse, fine) = (rim(&mut ctx, "coarse"), rim(&mut ctx, "fine"));
    let r = |a: usize, b: usize| a as f64 / b as f64;
    assert!((r(fine, medium) - 2f64.sqrt()).abs() < 0.15 && (r(medium, coarse) - 2f64.sqrt()).abs() < 0.15, "coarse {coarse}, medium {medium}, fine {fine}");
}

/// An STL's fillet torus on live's grid (live 2335, knob-jason's fillets): across the profile and round the axis the
/// fewest equal steps whose chords sag by at most half the chord tolerance. The hole's R 2 rim fillet (tube centre
/// R 7 at z 18) at Coarse: 72 steps over its 90°, 537 round (live's rule; BRepMesh alone gave about half).
#[test]
fn stl_fillet_torus_on_onshapes_grid() {
    let mut ctx = Ctx::new();
    block_with_hole(&mut ctx, "stl_torus.noshape");
    let circles = call(&mut ctx, "query_entities", json!({ "kind": "edges", "curve": "circle" }))["edges"].as_array().unwrap().clone();
    let rim = circles.iter().find(|e| (num(&e["midpoint"][2]) - 20.0).abs() < 1e-6).unwrap()["id"].as_str().unwrap().to_string();
    let fi = call(&mut ctx, "feature_add", json!({ "type": "fillet", "params": { "entities": [rim], "radius": "2 mm" } }));
    assert_eq!(fi["feature_status"]["status"], "Ok", "{fi}");
    let file = scratch("stl_torus_coarse.stl");
    call(&mut ctx, "export_parts", json!({ "format": "STL", "file": file.to_str().unwrap(), "units": "Millimeter", "resolution": "coarse" }));
    let b = std::fs::read(&file).unwrap();
    let n = u32::from_le_bytes(b[80..84].try_into().unwrap()) as usize;
    // The torus's inner nodes by their angle round the tube (degrees, 90 at the top, 180 at the bore), in rings
    // (gaps over 0.1°; the seam's own nodes alone between them).
    let mut nodes: std::collections::HashSet<(u32, u32, u32)> = Default::default();
    let mut phis: Vec<f64> = Vec::new();
    for i in 0..n {
        for k in 0..3 {
            let o = 84 + i * 50 + 12 + k * 12;
            let f = |j: usize| f32::from_le_bytes(b[o + 4 * j..o + 4 * j + 4].try_into().unwrap());
            let (x, y, z) = (f(0) as f64, f(1) as f64, f(2) as f64);
            let rho = x.hypot(y);
            let phi = (z - 18.0).atan2(rho - 7.0).to_degrees();
            if ((rho - 7.0).hypot(z - 18.0) - 2.0).abs() < 1e-3 && phi > 90.2 && phi < 179.8 && nodes.insert((f(0).to_bits(), f(1).to_bits(), f(2).to_bits())) {
                phis.push(phi);
            }
        }
    }
    phis.sort_by(f64::total_cmp);
    let mut rings: Vec<usize> = Vec::new();
    for (k, phi) in phis.iter().enumerate() {
        if k == 0 || phi - phis[k - 1] > 0.1 {
            rings.push(0);
        }
        *rings.last_mut().unwrap() += 1;
    }
    rings.retain(|&r| r > 10);
    assert_eq!(rings.len(), 71, "rings inside the profile (live's rule: 72 steps): {rings:?}");
    let mid = rings[rings.len() / 2];
    assert!((536..=537).contains(&mid), "{mid} nodes round a middle ring, live's rule 537 steps: {rings:?}");
}

/// Live vs-ps-table-create-studio: a later Variable Studio created from a Part Studio's Variable table is inserted
/// there as that Part Studio's own edit, so its Undo takes the insertion back (the new tab itself, a tab edit, stays).
#[test]
fn a_studio_created_from_a_part_studios_table_is_undone_there() {
    let mut ctx = Ctx::new();
    let path = scratch("vs-create-undo.noshape");
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(&mut ctx, "variable_create_studio", json!({}));
    let c = call(&mut ctx, "variable_create_studio", json!({ "tab": "Part Studio 1" }));
    assert_eq!(c["created"]["name"], "Variable Studio 2", "{c}");
    let titles = |v: &Value| v.as_array().map(|a| a.iter().map(|t| t["studio"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>()).unwrap_or_default();
    assert!(titles(&c["tables"]).contains(&"Variable Studio 2".to_string()), "inserted: {c}");
    let u = call(&mut ctx, "history_undo", json!({ "tab": "Part Studio 1" }));
    assert_eq!(u["description"], "Part Studio 1 :: Insert Variable Studio", "{u}");
    let t = call(&mut ctx, "variable_list", json!({ "tab": "Part Studio 1" }));
    assert!(!titles(&t["studio_tables"]).contains(&"Variable Studio 2".to_string()), "taken back: {t}");
    assert!(call(&mut ctx, "document_info", json!({}))["tabs"].as_array().unwrap().iter().any(|t| t["name"] == "Variable Studio 2"), "the tab stays");
}
