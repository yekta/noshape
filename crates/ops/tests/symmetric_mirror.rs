//! Symmetric modelling: half a part, Part mirror onto itself with Add (qa's halfmir / halfmir2 / mirhole, bool
//! 10-29). Faces on the mirror plane merge with their images; the Rust Boolean's results went through OCCT's
//! unify differently from OCCT's own once (a split hole wall, a pocket wall turned over: wrong volume). With the
//! Rust Boolean off (NOSHAPE_BOOL_RUST=0) the half boss and the half hole fail: OCCT's fuse of a reflected half
//! turns a face over (core's HALF-MIRROR-HOLE-INVERTED).

use noshape_ops::registry;
use noshape_ops::Ctx;
use serde_json::{Value, json};
use std::f64::consts::PI;

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).map(|o| o.value).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json()))
}

fn new(ctx: &mut Ctx, name: &str) {
    let dir = std::env::temp_dir().join(format!("noshape-ops-mirror-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.noshape"));
    let _ = std::fs::remove_file(&p);
    call(ctx, "document_create", json!({ "path": p.to_str().unwrap(), "length_unit": "mm" }));
}

/// The regions of a sketch on `plane` with `shapes`.
fn sketch(ctx: &mut Ctx, plane: &str, shapes: Value) -> Vec<Value> {
    let r = call(ctx, "sketch_create", json!({ "plane": plane }));
    let s = r["sketch"].as_str().unwrap().to_string();
    call(ctx, "sketch_draw", json!({ "sketch": s, "shapes": shapes }));
    call(ctx, "sketch_get", json!({ "sketch": s }))["regions"].as_array().unwrap().clone()
}

fn extrude(ctx: &mut Ctx, region: &Value, params: Value) {
    let mut p = json!({ "entities": [region["id"]] });
    p.as_object_mut().unwrap().extend(params.as_object().unwrap().clone());
    let r = call(ctx, "feature_add", json!({ "type": "extrude", "params": p }));
    assert_eq!(r["feature_status"]["status"], "Ok", "{r}");
}

/// Mirrors the one part across `plane` with Add: its (volume, faces), and the areas of its cylinder faces.
fn mirror_add(ctx: &mut Ctx, plane: &str) -> ((f64, u64), Vec<f64>) {
    let b = call(ctx, "query_parts", json!({}))["parts"][0].clone();
    let id = b.get("id").or(b.get("body")).unwrap().clone();
    let r = call(ctx, "feature_add", json!({ "type": "mirror", "params": { "patternType": "PART", "operationType": "ADD", "entities": [id], "mirrorPlane": [plane], "booleanScope": [id] } }));
    assert_eq!(r["feature_status"]["status"], "Ok", "{r}");
    let parts = call(ctx, "query_parts", json!({}))["parts"].as_array().unwrap().clone();
    assert_eq!(parts.len(), 1, "{parts:?}");
    let faces = call(ctx, "query_entities", json!({ "kind": "faces" }))["faces"].as_array().unwrap().clone();
    let neg: Vec<f64> = faces.iter().filter_map(|f| f["area"].as_f64()).filter(|&a| a < 0.0).collect();
    assert!(neg.is_empty(), "faces of negative area {neg:?}");
    let mut cyl: Vec<f64> = faces.iter().filter(|f| f.get("surface").or(f.get("type")) == Some(&json!("cylinder"))).filter_map(|f| f["area"].as_f64()).collect();
    cyl.sort_by(|a, b| a.total_cmp(b));
    ((parts[0]["volume"].as_f64().unwrap(), parts[0]["faces"].as_u64().unwrap()), cyl)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-3
}

/// A 20 × 20 × 10 half plate at x ≥ 0 on Top.
fn half_plate(ctx: &mut Ctx, name: &str) {
    new(ctx, name);
    let r = sketch(ctx, "plane:top", json!([{ "tool": "corner_rectangle", "points": [[0, 0], [20, 20]] }]));
    extrude(ctx, &r[0], json!({ "depth": "10 mm" }));
}

/// The half region (x > 0) of a circle of radius `r` at (0, 10) on the plate's top face.
fn half_disk_on_top(ctx: &mut Ctx, r: f64) -> Value {
    let faces = call(ctx, "query_entities", json!({ "kind": "faces", "surface": "plane", "normal": [0, 0, 1] }))["faces"].as_array().unwrap().clone();
    let top = faces[0]["id"].as_str().unwrap().to_string();
    let regions = sketch(ctx, &top, json!([{ "tool": "circle", "points": [[0, 10], [r, 10]] }, { "tool": "line", "points": [[0, 0], [0, 20]] }]));
    regions.into_iter().find(|g| (g["area"].as_f64().unwrap() - PI * r * r / 2.0).abs() < 0.5 && g["center"][0].as_f64().unwrap() > 0.0).expect("the half disk")
}

#[test]
fn a_half_boss_on_the_mirror_plane() {
    let mut ctx = Ctx::new();
    half_plate(&mut ctx, "half-boss");
    let half = half_disk_on_top(&mut ctx, 6.0);
    extrude(&mut ctx, &half, json!({ "depth": "10 mm", "operationType": "ADD", "defaultScope": true }));
    let ((v, _), _) = mirror_add(&mut ctx, "plane:right");
    assert!(near(v, 2.0 * (4000.0 + PI * 36.0 * 10.0 / 2.0)), "{v}");
}

#[test]
fn a_blind_half_pocket_on_the_mirror_plane() {
    let mut ctx = Ctx::new();
    half_plate(&mut ctx, "half-pocket");
    let half = half_disk_on_top(&mut ctx, 5.0);
    extrude(&mut ctx, &half, json!({ "depth": "5 mm", "operationType": "REMOVE", "defaultScope": true, "oppositeDirection": true }));
    let ((v, _), _) = mirror_add(&mut ctx, "plane:right");
    assert!(near(v, 2.0 * (4000.0 - PI * 25.0 * 5.0 / 2.0)), "{v}");
}

#[test]
fn a_half_hole_through_on_the_mirror_plane() {
    let mut ctx = Ctx::new();
    new(&mut ctx, "half-hole");
    let r = sketch(&mut ctx, "plane:top", json!([{ "tool": "corner_rectangle", "points": [[0, 0], [20, 20]] }, { "tool": "circle", "points": [[0, 10], [5, 10]] }]));
    let plate = r.iter().max_by(|a, b| a["area"].as_f64().unwrap().total_cmp(&b["area"].as_f64().unwrap())).unwrap().clone();
    extrude(&mut ctx, &plate, json!({ "depth": "10 mm" }));
    let ((v, faces), cyl) = mirror_add(&mut ctx, "plane:right");
    assert!(near(v, 2.0 * (4000.0 - PI * 25.0 * 10.0 / 2.0)), "{v}");
    // One hole wall.
    assert_eq!((faces, cyl.len()), (7, 1), "{cyl:?}");
}

#[test]
fn a_plate_with_a_hole_mirrored_across_its_bottom() {
    let mut ctx = Ctx::new();
    new(&mut ctx, "mirhole");
    let r = sketch(&mut ctx, "plane:top", json!([{ "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }, { "tool": "circle", "points": [[20, 10], [25, 10]] }]));
    let plate = r.iter().max_by(|a, b| a["area"].as_f64().unwrap().total_cmp(&b["area"].as_f64().unwrap())).unwrap().clone();
    extrude(&mut ctx, &plate, json!({ "depth": "10 mm" }));
    let ((v, faces), cyl) = mirror_add(&mut ctx, "plane:top");
    assert!(near(v, 2.0 * (800.0 - PI * 25.0) * 10.0), "{v}");
    // The hole wall one face across the mirror plane (RUST-MIRROR-EXTRA-FACE).
    assert_eq!((faces, cyl.len()), (7, 1), "{cyl:?}");
    assert!(near(cyl[0], 2.0 * PI * 5.0 * 20.0), "{cyl:?}");
}
