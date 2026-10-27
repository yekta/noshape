//! dx's speed targets (docs/PLAN.md 2.7: at least 2× faster than live;
//! ignored, run in release with `--ignored --nocapture`; perf.yml `model`
//! job): creating a drawing of the qa5 bracket with the four standard views,
//! and importing the NIST MTC CRADA assembly from STEP and Parasolid. Each
//! prints `timed <what>: <median> ms (runs …)`; live's times are in
//! behaviours.md "dx speed: drawing creation and NIST import".

use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};
use std::time::Instant;

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn report(what: &str, runs: Vec<f64>) -> f64 {
    let m = median(runs.clone());
    eprintln!("timed {what}: {m:.1} ms (runs {})", runs.iter().map(|r| format!("{r:.1}")).collect::<Vec<_>>().join(", "));
    m
}

fn regions(ctx: &mut Ctx, sketch: &str) -> Vec<Value> {
    let mut rs = call(ctx, "sketch_get", json!({ "sketch": sketch }))["regions"].as_array().cloned().unwrap_or_default();
    rs.sort_by(|a, b| a["area"].as_f64().unwrap_or(0.0).total_cmp(&b["area"].as_f64().unwrap_or(0.0)));
    rs.into_iter().map(|r| r["id"].clone()).collect()
}

fn sketch(ctx: &mut Ctx, plane: &str, draws: &[(&str, Value)]) -> String {
    let s = call(ctx, "sketch_create", json!({ "plane": plane }))["sketch"].as_str().unwrap().to_string();
    for (tool, points) in draws {
        call(ctx, "sketch_draw", json!({ "sketch": s, "tool": tool, "points": points }));
    }
    s
}

/// Edges whose midpoint is `p` (mm).
fn edges_at(ctx: &mut Ctx, p: [f64; 3]) -> Vec<Value> {
    let es = call(ctx, "query_entities", json!({ "kind": "edges", "limit": 400 }))["edges"].as_array().cloned().unwrap_or_default();
    es.into_iter()
        .filter(|e| e["midpoint"].as_array().is_some_and(|m| m.len() == 3 && m.iter().zip(p).all(|(a, b)| (a.as_f64().unwrap_or(f64::NAN) - b).abs() < 1e-3)))
        .map(|e| e["id"].clone())
        .collect()
}

fn feature(ctx: &mut Ctx, ty: &str, params: Value) -> String {
    let r = call(ctx, "feature_add", json!({ "type": ty, "params": params }));
    assert_eq!(r["feature_status"]["status"], "Ok", "{ty}: {r}");
    r["feature_status"]["id"].as_str().unwrap().to_string()
}

/// The qa5 bracket (docs/parity/workflow-bracket.md; live "qa5 bracket"),
/// as qa5's mkbracket.py builds it: 67191.3275 mm³.
fn bracket(ctx: &mut Ctx, path: &str) {
    call(ctx, "document_create", json!({ "path": path }));
    let s = sketch(ctx, "plane:top", &[("corner_rectangle", json!([[0, 0], [80, 50]]))]);
    let r = regions(ctx, &s);
    feature(ctx, "extrude", json!({ "entities": r, "depth": "8 mm" }));
    let s = sketch(ctx, "plane:top", &[("corner_rectangle", json!([[0, 0], [80, 8]]))]);
    let r = regions(ctx, &s);
    feature(ctx, "extrude", json!({ "entities": r, "depth": "60 mm", "operationType": "ADD" }));
    let s = sketch(ctx, "plane:top", &[("circle", json!([[20, 30], [24, 30]])), ("circle", json!([[60, 30], [64, 30]]))]);
    let r: Vec<Value> = regions(ctx, &s).into_iter().take(2).collect();
    // Up into the part (std's edit logic points a Remove on the Top plane away from it).
    feature(ctx, "extrude", json!({ "entities": r, "operationType": "REMOVE", "endBound": "THROUGH_ALL", "oppositeDirection": false }));
    let mut f = edges_at(ctx, [0.0, 50.0, 4.0]);
    f.extend(edges_at(ctx, [80.0, 50.0, 4.0]));
    feature(ctx, "fillet", json!({ "entities": f, "radius": "5 mm" }));
    let c = edges_at(ctx, [40.0, 0.0, 60.0]);
    feature(ctx, "chamfer", json!({ "entities": c, "width": "1 mm" }));
    let s = sketch(ctx, "plane:front", &[("circle", json!([[15, 45], [18, 45]]))]);
    let r: Vec<Value> = regions(ctx, &s).into_iter().take(1).collect();
    let hole = feature(ctx, "extrude", json!({ "entities": r, "operationType": "REMOVE", "endBound": "THROUGH_ALL", "oppositeDirection": true }));
    let dir = edges_at(ctx, [40.0, 50.0, 0.0]);
    let pattern = json!({ "patternType": "FEATURE", "instanceFunction": [format!("feature:{hole}")], "directionOne": dir, "distance": "25 mm", "instanceCount": 3 });
    // The edge may point off the wall (live fails the same way): then its Opposite direction.
    if call(ctx, "feature_add", json!({ "type": "linearPattern", "params": pattern }))["feature_status"]["status"] != "Ok" {
        call(ctx, "history_undo", json!({}));
        let mut p = pattern.clone();
        p["oppositeDirection"] = json!(true);
        feature(ctx, "linearPattern", p);
    }
    let s = sketch(ctx, "plane:right", &[("line", json!([[8, 8], [30, 8]])), ("line", json!([[30, 8], [8, 40]])), ("line", json!([[8, 40], [8, 8]]))]);
    let r: Vec<Value> = regions(ctx, &s).into_iter().take(1).collect();
    let gusset = feature(ctx, "extrude", json!({ "entities": r, "depth": "5 mm", "operationType": "ADD" }));
    let pl = feature(ctx, "cPlane", json!({ "entities": ["plane:right"], "offset": "40 mm" }));
    feature(ctx, "mirror", json!({ "patternType": "FEATURE", "instanceFunction": [format!("feature:{gusset}")], "mirrorPlane": [format!("fplane:{pl}")] }));
    let parts = call(ctx, "query_parts", json!({}));
    let v = parts["parts"][0]["volume"].as_f64().unwrap();
    assert!((v - 67191.3275).abs() < 1e-3, "the bracket: {v} mm³ (live 67191.3275)");
}

/// PLAN 2.7 dx: Create drawing of the bracket with its standard views
/// (Front, Top, Right, Isometric: the dialog's suggested views), from the
/// dialog's OK to every view computed (`drawing_create` builds the sheet:
/// hidden-line views, tangent edges, projections). The Part Studio is
/// already regenerated, as when the user right-clicks its tab.
#[test]
#[ignore]
fn timed_bracket_drawing_four_views() {
    let dir = std::env::temp_dir().join(format!("noshape-dx-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ctx = Ctx::new();
    bracket(&mut ctx, dir.join("bracket.noshape").to_str().unwrap());
    let mut runs = Vec::new();
    let mut one = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ANSI_A_INCH.dwt", "suggested_views": true }));
        runs.push(t.elapsed().as_secs_f64() * 1000.0);
        let views = d["views"].as_array().map(|v| v.len()).unwrap_or(0);
        assert_eq!(views, 4, "{d}");
        let drawing = d["drawing"].as_str().unwrap().to_string();
        let t = Instant::now();
        call(&mut ctx, "drawing_insert_view", json!({ "drawing": drawing, "source": "Part Studio 1", "orientation": "front", "position": [60, 60] }));
        one.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    report("bracket drawing, 4 standard views", runs);
    report("bracket drawing, insert one Front view", one);
    let _ = std::fs::remove_dir_all(&dir);
}

/// PLAN 2.7 dx: import of the NIST MTC CRADA assembly from STEP and from
/// Parasolid (live's own exports of it: tests/perf/nist-mtc-crada-assembly.step
/// / .x_t), from Import to the parts regenerated and tessellated for display.
#[test]
#[ignore]
fn timed_nist_import() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    for ext in ["step", "x_t"] {
        let file = root.join(format!("nist-mtc-crada-assembly.{ext}"));
        if !file.exists() {
            eprintln!("timed NIST import {ext}: skipped, no {}", file.display());
            continue;
        }
        let mut runs = Vec::new();
        let mut parts = 0;
        for i in 0..3 {
            let dir = std::env::temp_dir().join(format!("noshape-dx-perf-{}-{ext}-{i}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let mut ctx = Ctx::new();
            call(&mut ctx, "document_create", json!({ "path": dir.join("nist.noshape").to_str().unwrap() }));
            let t = Instant::now();
            call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
            let q = call(&mut ctx, "query_parts", json!({}));
            runs.push(t.elapsed().as_secs_f64() * 1000.0);
            parts = q["parts"].as_array().map(|p| p.len()).unwrap_or(0);
            let _ = std::fs::remove_dir_all(&dir);
        }
        assert!(parts > 0);
        report(&format!("NIST import {ext} ({parts} parts)"), runs);
    }
}
