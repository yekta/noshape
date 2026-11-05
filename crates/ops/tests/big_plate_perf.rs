//! Timing probe for qa5 PERF1/PERF2 (ignored; run with `--ignored
//! --nocapture`): the 400 × 400 × 10 mm plate with a 20 × 20 grid of Ø8 mm
//! holes (Extrude Remove Through all), built through the agent API like
//! qa5's mkbig2.py. Prints the wall time of each call, then a regeneration
//! from scratch split into feature evaluation (`regen_ms`, per feature)
//! and display (tessellation and the rest), and the plate's tessellation
//! alone. Live: the Remove 617 ms in all (Extrude 2 491 ms).

use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};
use std::time::Instant;

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

fn timed(ctx: &mut Ctx, what: &str, tool: &str, args: Value) -> Value {
    let t = Instant::now();
    let v = call(ctx, tool, args);
    eprintln!("{what}: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    v
}

/// The plate in a new document at `path` (each step timed).
fn big_plate(ctx: &mut Ctx, path: &str) {
    call(ctx, "document_create", json!({ "path": path }));
    let tab = "Part Studio 1";
    let sk = call(ctx, "sketch_create", json!({ "tab": tab, "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    let st = call(ctx, "sketch_draw", json!({ "tab": tab, "sketch": sketch, "tool": "corner_rectangle", "points": [[0, 0], [400, 400]] }));
    let region = st["regions"][0]["id"].as_str().unwrap().to_string();
    timed(ctx, "Extrude 1 (plate)", "feature_add", json!({ "tab": tab, "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    let sk = call(ctx, "sketch_create", json!({ "tab": tab, "plane": "plane:top" }));
    let sketch = sk["sketch"].as_str().unwrap().to_string();
    let t = Instant::now();
    let mut last = Value::Null;
    for i in 0..20 {
        for j in 0..20 {
            let (x, y) = (10 + 20 * i, 10 + 20 * j);
            last = call(ctx, "sketch_draw", json!({ "tab": tab, "sketch": sketch, "tool": "circle", "points": [[x, y], [x + 4, y]] }));
        }
    }
    eprintln!("400 circles: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    let regions: Vec<Value> = last["regions"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    eprintln!("regions: {}", regions.len());
    let r = timed(ctx, "Extrude 2 (Remove, Through all)", "feature_add", json!({ "tab": tab, "type": "extrude", "params": { "entities": regions, "operationType": "REMOVE", "defaultScope": true, "endBound": "THROUGH_ALL" } }));
    assert_eq!(r["feature_status"]["status"], "Ok", "{r}");
}

/// The plate as a benchmark fixture (`tests/perf/big-plate.noshape`,
/// perf.yml `fixture`). `NOSHAPE_FIXTURE_OUT=<path>` writes it.
#[test]
#[ignore]
fn write_big_plate_fixture() {
    let out = std::env::var("NOSHAPE_FIXTURE_OUT").expect("NOSHAPE_FIXTURE_OUT");
    let _ = std::fs::remove_file(&out);
    big_plate(&mut Ctx::new(), &out);
}

#[test]
#[ignore]
fn big_plate_timing() {
    let dir = std::env::temp_dir().join(format!("noshape-big-plate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plate.noshape");
    let mut ctx = Ctx::new();
    big_plate(&mut ctx, path.to_str().unwrap());
    let tab = "Part Studio 1";

    // From scratch: feature evaluation vs display.
    let mut fresh = noshape_model::regen::Regenerator::new();
    let od = ctx.doc(path.to_str()).unwrap();
    let tabid = noshape_ops::doc::find_tab(od.doc(), tab).unwrap();
    let noshape_model::document::TabContent::PartStudio(ps) = &od.doc().tab(tabid).unwrap().content else { panic!("not a Part Studio") };
    let t = Instant::now();
    let out = fresh.regenerate(ps, &Default::default(), &od.doc().settings.units, &noshape_model::regen::RegenOptions::default());
    let total = t.elapsed().as_secs_f64() * 1000.0;
    let d = &out.display;
    let feats: Vec<String> = d.features.iter().map(|f| format!("{:.0}", f.time_ms)).collect();
    let tris: usize = d.bodies.iter().map(|b| b.mesh.indices.len() / 3).sum();
    eprintln!("regenerate from scratch: {total:.0} ms; features {:.0} ms [{}]; the rest (display) {:.0} ms; {} bodies, {tris} triangles", d.regen_ms, feats.join(", "), total - d.regen_ms as f64, d.bodies.len());
    for b in &out.context.bodies {
        let t = Instant::now();
        let m = noshape_model::regen::tessellate_shape(&b.shape, 1.0).unwrap();
        eprintln!("tessellate {} alone: {:.0} ms, {} triangles", b.name, t.elapsed().as_secs_f64() * 1000.0, m.indices.len() / 3);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Profiling aid: the chamfered plate's body tessellated five times
/// (`perf record` this test to see where display meshing goes).
#[test]
#[ignore]
fn big_plate_tessellate_profile() {
    let dir = std::env::temp_dir().join(format!("noshape-big-plate-prof-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plate.noshape");
    let mut ctx = Ctx::new();
    big_plate(&mut ctx, path.to_str().unwrap());
    let od = ctx.doc(path.to_str()).unwrap();
    let tabid = noshape_ops::doc::find_tab(od.doc(), "Part Studio 1").unwrap();
    let r = od.regenerate(tabid, None).unwrap();
    let shape = r.context.bodies[0].shape.clone();
    eprintln!("PROFILE START");
    for _ in 0..5 {
        let t = Instant::now();
        let m = noshape_model::regen::tessellate_shape(&shape, 1.0).unwrap();
        eprintln!("tessellate: {:.0} ms, {} triangles", t.elapsed().as_secs_f64() * 1000.0, m.indices.len() / 3);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
