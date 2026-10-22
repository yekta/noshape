//! Documents for pixel crops of the viewport (ignored; `NOSHAPE_FIXTURE_OUT`
//! = the .noshape to write). `sketch_labels_fixture`: a Part Studio whose
//! Sketch 1 has every kind of dimension label at rest: plain values, an
//! expression (`ƒ` marker), a variable (`ƒx`), a diameter, for comparing
//! the labels drawn in the frame (`NOSHAPE_FRAME_SKETCH_LABELS`) with GPUI's.

use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

#[test]
#[ignore]
fn sketch_labels_fixture() {
    let out = std::env::var("NOSHAPE_FIXTURE_OUT").expect("NOSHAPE_FIXTURE_OUT");
    let _ = std::fs::remove_file(&out);
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": out }));
    let tab = "Part Studio 1";
    let v = call(&mut ctx, "feature_add", json!({ "tab": tab, "type": "assignVariable", "params": { "name": "h", "variableType": "LENGTH", "lengthValue": "30 mm" } }));
    assert_eq!(v["feature_status"]["status"], "Ok", "{v}");
    let sk = call(&mut ctx, "sketch_create", json!({ "tab": tab, "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    call(&mut ctx, "sketch_draw", json!({ "tab": tab, "sketch": sketch, "tool": "corner_rectangle", "points": [[0, 0], [60, 30]] }));
    call(&mut ctx, "sketch_draw", json!({ "tab": tab, "sketch": sketch, "tool": "circle", "points": [[30, 15], [36, 15]] }));
    let state = call(&mut ctx, "sketch_get", json!({ "tab": tab, "sketch": sketch }));
    let entities = state["entities"].as_array().unwrap();
    let line = |horizontal: bool, at: f64| {
        entities
            .iter()
            .find(|e| {
                e["type"] == "line" && {
                    let (s, t) = (&e["start"], &e["end"]);
                    let (sx, sy, tx, ty) = (s[0].as_f64().unwrap(), s[1].as_f64().unwrap(), t[0].as_f64().unwrap(), t[1].as_f64().unwrap());
                    if horizontal { (sy - ty).abs() < 1e-6 && (sy - at).abs() < 1e-6 } else { (sx - tx).abs() < 1e-6 && (sx - at).abs() < 1e-6 }
                }
            })
            .unwrap_or_else(|| panic!("line {horizontal} {at}"))["id"]
            .clone()
    };
    let circle = entities.iter().find(|e| e["type"] == "circle").unwrap()["id"].clone();
    let dim = |ctx: &mut Ctx, entities: Value, kind: &str, value: &str, label: [f64; 2]| {
        let d = call(ctx, "sketch_dimension", json!({ "tab": tab, "sketch": sketch, "entities": entities, "kind": kind, "value": value, "label": label }));
        assert!(d.get("dimension").is_some(), "{d}");
    };
    // Plain, expression (ƒ), variable (ƒx), diameter.
    dim(&mut ctx, json!([line(true, 0.0)]), "length", "60 mm", [30.0, -8.0]);
    dim(&mut ctx, json!([line(false, 0.0)]), "length", "#h", [-8.0, 15.0]);
    dim(&mut ctx, json!([circle]), "diameter", "10 mm + 2 mm", [42.0, 24.0]);
    dim(&mut ctx, json!([line(false, 60.0), circle]), "distance", "30 mm", [45.0, 6.0]);
    let r = call(&mut ctx, "sketch_get", json!({ "tab": tab, "sketch": sketch }));
    eprintln!("{}", serde_json::to_string_pretty(&r["constraints"]).unwrap());
}
