//! Live behaviours "qa4 round 5 s3 mate flip" (qa4 MATE-FLIP-AXIAL): a fixed
//! 100×60×10 plate (z 0..10) and a Ø10×40 pin at (80, 0, 0) mm; the pin's
//! top-face connector is mated to the plate's top-face connector.
//! - Revolute, Cylindrical and Planar by default align the Z axes: the pin
//!   hangs through the plate, t (0, 0, −30), Z (0, 0, 1).
//! - Flip primary axis turns it over: t (0, 0, 50), Z (0, 0, −1).
//! - The pin laid on its side first (its Z perpendicular to the plate's)
//!   gets the default result too.

use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

fn studio(ctx: &mut Ctx, tab: &str, draw: Value, depth: &str) {
    let sk = call(ctx, "sketch_create", json!({ "plane": "plane:top", "tab": tab }))["sketch"].as_str().unwrap().to_string();
    let mut d = draw;
    d["sketch"] = json!(sk);
    d["tab"] = json!(tab);
    call(ctx, "sketch_draw", d);
    let region = call(ctx, "sketch_get", json!({ "sketch": sk, "tab": tab }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": depth }, "tab": tab }));
}

/// The face of `inst` whose world normal is `n`.
fn face(ctx: &mut Ctx, inst: &str, n: [f64; 3]) -> String {
    let faces = call(ctx, "assembly_entities", json!({ "instance": inst, "kind": "faces" }));
    let f = faces["faces"].as_array().unwrap().iter().find(|f| (0..3).all(|k| (f["normal"][k].as_f64().unwrap_or(9.0) - n[k]).abs() < 1e-6)).expect("face");
    f["id"].as_str().unwrap().to_string()
}

/// The pin's translation (mm) and Z axis after a mate.
fn mate(kind: &str, flip: Option<bool>, side: bool) -> ([f64; 3], [f64; 3]) {
    let dir = std::env::temp_dir().join(format!("noshape-mate-flip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{kind}-{flip:?}-{side}.noshape"));
    let _ = std::fs::remove_file(&path);
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    studio(&mut ctx, "Part Studio 1", json!({ "tool": "corner_rectangle", "points": [[-50, -30], [50, 30]] }), "10 mm");
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "pin" }));
    studio(&mut ctx, "pin", json!({ "tool": "circle", "points": [[0, 0], [5, 0]] }), "40 mm");
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "source": "pin", "part": "Part 1" }));
    call(&mut ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>"], "fixed": true }));
    let mut t = json!({ "instance": "Part 1 <2>", "translation": [80, 0, 0] });
    if side {
        t["axis"] = json!([1, 0, 0]);
        t["angle"] = json!("90 deg");
    }
    call(&mut ctx, "assembly_transform", t);
    let a = face(&mut ctx, "Part 1 <2>", if side { [0.0, -1.0, 0.0] } else { [0.0, 0.0, 1.0] });
    let b = face(&mut ctx, "Part 1 <1>", [0.0, 0.0, 1.0]);
    let mut args = json!({ "type": kind, "a": { "instance": "Part 1 <2>", "entity": a, "inference": "centroid" }, "b": { "instance": "Part 1 <1>", "entity": b, "inference": "centroid" } });
    if let Some(f) = flip {
        args["flip_primary"] = json!(f);
    }
    call(&mut ctx, "assembly_mate", args);
    let list = call(&mut ctx, "assembly_list", json!({}));
    let pin = list["instances"].as_array().unwrap().iter().find(|i| i["name"] == "Part 1 <2>").unwrap().clone();
    let t: Vec<f64> = pin["translation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let q: Vec<f64> = pin["rotation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let zax = [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)];
    let _ = std::fs::remove_file(&path);
    ([t[0], t[1], t[2]], zax)
}

fn near(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < tol)
}

#[test]
fn axial_mates_default_align_and_flip_turns_over() {
    for kind in ["revolute", "cylindrical", "planar"] {
        // The CLI's flip_primary is the model's: true = Z axes aligned.
        let (t, z) = mate(kind, None, false);
        assert!(near(t, [0.0, 0.0, -30.0], 1e-2) && near(z, [0.0, 0.0, 1.0], 1e-6), "{kind} default: t {t:?} Z {z:?}");
        let (t, z) = mate(kind, Some(false), false);
        assert!(near(t, [0.0, 0.0, 50.0], 1e-2) && near(z, [0.0, 0.0, -1.0], 1e-6), "{kind} flipped: t {t:?} Z {z:?}");
    }
    let (t, z) = mate("revolute", None, true);
    assert!(near(t, [0.0, 0.0, -30.0], 1e-2) && near(z, [0.0, 0.0, 1.0], 1e-6), "revolute from the side: t {t:?} Z {z:?}");
}
