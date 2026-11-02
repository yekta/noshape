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
            let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
            runs.push(t.elapsed().as_secs_f64() * 1000.0);
            // As live: an Assembly and a Part Studio of the 8 distinct parts.
            assert!(r["tabs"].as_array().unwrap().iter().any(|t| t["kind"] == "Assembly"), "{r}");
            parts = r["parts"].as_array().map(|p| p.len()).unwrap_or(0);
            // At live's size: the box is 152.4 × 101.6 × 73 mm, the largest
            // part (qa XT-ASM-SCALE: x_t parts came in 1000× too small).
            let studio = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").expect("the parts' Part Studio")["id"].clone();
            let q = call(&mut ctx, "query_parts", json!({ "tab": studio }));
            let extent = |p: &Value| (0..3).map(|k| p["bounding_box"]["max"][k].as_f64().unwrap() - p["bounding_box"]["min"][k].as_f64().unwrap()).fold(0.0, f64::max);
            let biggest = q["parts"].as_array().unwrap().iter().map(extent).fold(0.0, f64::max);
            assert!((biggest - 152.4).abs() < 0.5, "{ext}: the largest part is {biggest} mm long, not 152.4");
            let _ = std::fs::remove_dir_all(&dir);
        }
        assert_eq!(parts, 8);
        report(&format!("NIST import {ext} ({parts} parts)"), runs);
    }
}

/// Big files (user 11-02): 1000 distinct parts (a 40 × 25 part pattern of
/// Ø4 × 10 mm pins) exported as STEP, Parasolid and IGES, then each file
/// imported into a new document; times both ways and checks fidelity (part
/// count, total volume). `NOSHAPE_BIG_PARTS` changes the count (rows of 40).
#[test]
#[ignore]
fn timed_big_part_imports() {
    let rows: u32 = std::env::var("NOSHAPE_BIG_PARTS").ok().and_then(|v| v.parse().ok()).map_or(25, |n: u32| n.div_ceil(40).max(1));
    let n = 40 * rows as usize;
    let dir = std::env::temp_dir().join(format!("noshape-dx-big-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": dir.join("src.noshape").to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let region = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [2, 0]] }))["regions"][0]["id"].as_str().unwrap().to_string();
    let seed = call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }))["feature"].as_str().unwrap().to_string();
    let t = Instant::now();
    let p = call(&mut ctx, "feature_add", json!({ "type": "linearPattern", "params": {
        "patternType": "PART", "entities": [format!("body:{seed}.0")],
        "directionOne": ["plane:right"], "distance": "6 mm", "instanceCount": "40",
        "directionTwo": ["plane:front"], "distanceTwo": "6 mm", "instanceCountTwo": rows.to_string(), "hasSecondDir": true,
        "operationType": "NEW" } }));
    assert_eq!(p["feature_status"]["status"], "Ok", "{p}");
    report(&format!("big: pattern of {n} parts"), vec![t.elapsed().as_secs_f64() * 1000.0]);
    let volume = |ctx: &mut Ctx, tab: Option<&str>| {
        let q = call(ctx, "query_parts", match tab { Some(t) => json!({ "tab": t }), None => json!({}) });
        let parts = q["parts"].as_array().unwrap().clone();
        (parts.len(), parts.iter().map(|p| p["volume"].as_f64().unwrap_or(0.0)).sum::<f64>())
    };
    let (count, v0) = volume(&mut ctx, None);
    assert_eq!(count, n);
    for (format, ext) in [("STEP", "step"), ("PARASOLID", "x_t"), ("IGES", "igs")] {
        let file = dir.join(format!("big.{ext}"));
        let t = Instant::now();
        call(&mut ctx, "export_parts", json!({ "format": format, "file": file.to_str().unwrap() }));
        report(&format!("big: export {n} parts {ext} ({} MB)", std::fs::metadata(&file).map(|m| m.len() / 1_000_000).unwrap_or(0)), vec![t.elapsed().as_secs_f64() * 1000.0]);
        let mut ictx = Ctx::new();
        call(&mut ictx, "document_create", json!({ "path": dir.join(format!("in-{ext}.noshape")).to_str().unwrap() }));
        let t = Instant::now();
        let r = call(&mut ictx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let studio = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").map(|t| t["name"].as_str().unwrap().to_string()).expect("a Part Studio");
        let (got, v1) = volume(&mut ictx, Some(&studio));
        report(&format!("big: import {n} parts {ext} -> {got} parts, volume {:.4} of {:.4}", v1, v0), vec![ms]);
        // IGES carries surfaces: its parts may come in as sheets.
        if ext != "igs" {
            assert_eq!(got, n, "{ext}");
            assert!((v1 - v0).abs() < v0 * 1e-6, "{ext}: volume {v1} vs {v0}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Big assemblies (user 11-02): `tests/perf/pins-assembly-1000.noshape`
/// (1000 instances of one pin) exported as STEP, Parasolid and IGES, each
/// file imported into a new document; times both ways and checks fidelity
/// (STEP / Parasolid: an Assembly of 1000 instances of the one part, named
/// after it; IGES: 1000 parts, flat).
#[test]
#[ignore]
fn timed_big_assembly_round_trip() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/pins-assembly-1000.noshape");
    let dir = std::env::temp_dir().join(format!("noshape-dx-bigasm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pins.noshape");
    std::fs::copy(&src, &path).unwrap();
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_open", json!({ "path": path.to_str().unwrap() }));
    let n = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" }))["instances"].as_array().unwrap().len();
    assert_eq!(n, 1000);
    for (format, ext) in [("STEP", "step"), ("PARASOLID", "x_t"), ("IGES", "igs")] {
        let file = dir.join(format!("pins.{ext}"));
        let t = Instant::now();
        call(&mut ctx, "export_parts", json!({ "tab": "Assembly 1", "format": format, "file": file.to_str().unwrap() }));
        report(&format!("big asm: export {n} instances {ext} ({} KB)", std::fs::metadata(&file).map(|m| m.len() / 1000).unwrap_or(0)), vec![t.elapsed().as_secs_f64() * 1000.0]);
        let mut ictx = Ctx::new();
        call(&mut ictx, "document_create", json!({ "path": dir.join(format!("in-{ext}.noshape")).to_str().unwrap() }));
        let t = Instant::now();
        let r = call(&mut ictx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let tab = |kind: &str| r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == kind).map(|t| t["name"].as_str().unwrap().to_string());
        let studio = tab("Part Studio").expect("a Part Studio");
        let parts = call(&mut ictx, "query_parts", json!({ "tab": studio }))["parts"].as_array().unwrap().clone();
        let instances = tab("Assembly").map_or(0, |a| call(&mut ictx, "assembly_list", json!({ "tab": a }))["instances"].as_array().unwrap().len());
        report(&format!("big asm: import {n} instances {ext} -> {} part(s), {instances} instances", parts.len()), vec![ms]);
        if ext == "igs" {
            assert_eq!(parts.len(), n, "IGES is flat");
        } else {
            assert_eq!((parts.len(), instances), (1, n), "{ext}");
            assert_eq!(parts[0]["name"], "Part 1", "{ext}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
