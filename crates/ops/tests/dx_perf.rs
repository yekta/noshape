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
/// count, total volume, names). `NOSHAPE_BIG_PARTS` changes the count (rows of 40).
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
        let mut names: Vec<String> = parts.iter().map(|p| p["name"].as_str().unwrap_or("").to_string()).collect();
        names.sort();
        (parts.len(), parts.iter().map(|p| p["volume"].as_f64().unwrap_or(0.0)).sum::<f64>(), names)
    };
    let (count, v0, names0) = volume(&mut ctx, None);
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
        let (got, v1, names1) = volume(&mut ictx, Some(&studio));
        report(&format!("big: import {n} parts {ext} -> {got} parts, volume {:.4} of {:.4}", v1, v0), vec![ms]);
        // IGES carries surfaces: its parts may come in as sheets.
        if ext != "igs" {
            assert_eq!(got, n, "{ext}");
            assert!((v1 - v0).abs() < v0 * 1e-6, "{ext}: volume {v1} vs {v0}");
            assert!(names1 == names0, "{ext}: the parts' names ({:?}… vs {:?}…)", &names1[..3], &names0[..3]);
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

/// Many copies of a heavy part in a flat file (user 11-02, mesh reuse): the
/// NIST assembly's bigger cap screw (threads) as a part pattern
/// (`NOSHAPE_HEAVY_ROWS` rows of 5, default 5), exported as STEP and
/// imported back; times each step (NOSHAPE_IMPORT_TIMING=1 for the
/// import's stages).
#[test]
#[ignore]
fn timed_many_heavy_parts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf");
    let dir = std::env::temp_dir().join(format!("noshape-dx-heavy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": dir.join("src.noshape").to_str().unwrap() }));
    let r = call(&mut ctx, "import_files", json!({ "files": [root.join("nist-mtc-crada-assembly.step").to_str().unwrap()] }));
    let studio = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").unwrap()["name"].as_str().unwrap().to_string();
    let parts = call(&mut ctx, "query_parts", json!({ "tab": studio }))["parts"].as_array().unwrap().clone();
    let screw = parts.iter().find(|p| p["name"].as_str().unwrap().starts_with("91274A141")).expect("the screw").clone();
    let rows: usize = std::env::var("NOSHAPE_HEAVY_ROWS").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let n = 5 * rows;
    let t = Instant::now();
    let p = call(&mut ctx, "feature_add", json!({ "tab": studio, "type": "linearPattern", "params": {
        "patternType": "PART", "entities": [screw["id"]],
        "directionOne": ["plane:right"], "distance": "30 mm", "instanceCount": "5",
        "directionTwo": ["plane:front"], "distanceTwo": "30 mm", "instanceCountTwo": rows.to_string(), "hasSecondDir": true,
        "operationType": "NEW" } }));
    assert_eq!(p["feature_status"]["status"], "Ok", "{p}");
    report(&format!("heavy: pattern of {n} screws"), vec![t.elapsed().as_secs_f64() * 1000.0]);
    let file = dir.join("screws.step");
    // The screw and the copies the pattern added.
    let before: Vec<&Value> = parts.iter().map(|p| &p["id"]).collect();
    let copies: Vec<String> = call(&mut ctx, "query_parts", json!({ "tab": studio }))["parts"].as_array().unwrap().iter().filter(|p| p["id"] == screw["id"] || !before.contains(&&p["id"])).map(|p| p["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(copies.len(), n);
    let t = Instant::now();
    call(&mut ctx, "export_parts", json!({ "tab": studio, "format": "STEP", "parts": copies, "file": file.to_str().unwrap() }));
    report(&format!("heavy: export {n} screws step"), vec![t.elapsed().as_secs_f64() * 1000.0]);
    let mut ictx = Ctx::new();
    call(&mut ictx, "document_create", json!({ "path": dir.join("in.noshape").to_str().unwrap() }));
    let t = Instant::now();
    let r = call(&mut ictx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
    let got = r["parts"].as_array().map_or(0, |p| p.len());
    report(&format!("heavy: import {n} screws step ({} MB) -> {got} parts", std::fs::metadata(&file).map(|m| m.len() / 1_000_000).unwrap_or(0)), vec![t.elapsed().as_secs_f64() * 1000.0]);
    assert_eq!(got, n);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live's own big files (request 1755, live/samples/dx-big in
/// `NOSHAPE_LIVE_BIG`): its 1000-part / 2000-instance assembly as STEP,
/// Parasolid and IGES, the 1000-part Part Studio as STEP, and NIST's largest
/// PMI test parts. Each imported into a new document: time, parts, distinct
/// names, colours, total volume and instances. Live: 33.5 / 12.9 / 69.3 /
/// 29.3 s to "Post-processing done"; 1000 parts, 1000 names, 900 + 100
/// colours, 396562.770315 mm³, 2000 instances; NIST parts 12–14 s.
#[test]
#[ignore]
fn timed_live_big_samples() {
    let Some(dir) = std::env::var_os("NOSHAPE_LIVE_BIG").map(std::path::PathBuf::from) else { return };
    let scratch = std::env::temp_dir().join(format!("noshape-dx-livebig-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let files = ["dx-big-asm-ap242.step", "dx-big-asm.x_t", "dx-big-asm.igs", "dx-big-parts-ap242.step", "nist/nist_ftc_08_asme1_ap242-e2.stp", "nist/nist_ftc_09_asme1_ap242-e1.stp", "nist/nist_stc_09_asme1_ap242-e3.stp"];
    for (k, f) in files.iter().enumerate() {
        let file = dir.join(f);
        if !file.exists() {
            continue;
        }
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": scratch.join(format!("in-{k}.noshape")).to_str().unwrap() }));
        let t = Instant::now();
        let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let tab = |kind: &str| r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == kind).map(|t| t["name"].as_str().unwrap().to_string());
        let studio = tab("Part Studio").expect("a Part Studio");
        let parts = call(&mut ctx, "query_parts", json!({ "tab": studio }))["parts"].as_array().unwrap().clone();
        let solids: Vec<&Value> = parts.iter().filter(|p| p["kind"] == "Solid").collect();
        let names: std::collections::HashSet<&str> = solids.iter().filter_map(|p| p["name"].as_str()).collect();
        let mut colours: std::collections::BTreeMap<String, usize> = Default::default();
        for p in &solids {
            *colours.entry(p["color"].as_str().unwrap_or("?").to_string()).or_default() += 1;
        }
        // Live names them "B<row>-<col>" (IGES: not "B17-29(0)").
        if f.starts_with("dx-big") {
            let live_name = |n: &&str| n.strip_prefix('B').and_then(|r| r.split_once('-')).is_some_and(|(a, b)| a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok());
            let odd: Vec<&&str> = names.iter().filter(|n| !live_name(n)).collect();
            assert!(odd.is_empty(), "{f}: {} not as live, e.g. {:?}", odd.len(), &odd[..odd.len().min(5)]);
        }
        let volume: f64 = solids.iter().map(|p| p["volume"].as_f64().unwrap_or(0.0)).sum();
        let instances = tab("Assembly").map_or(0, |a| call(&mut ctx, "assembly_list", json!({ "tab": a }))["instances"].as_array().unwrap().len());
        report(&format!("live big: {f}: {} solids ({} names), colours {colours:?}, {volume:.6} mm3, {instances} instances", solids.len(), names.len()), vec![ms]);
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// A 2-point dimension between the ends of a straight edge of a view (sheet
/// mm; at a small scale a click on an edge is ambiguous, live picked a
/// vertex), the longest that takes one, placed 8 mm beside it.
fn dimension_longest_line(ctx: &mut Ctx, drawing: &str, view: i64) -> f64 {
    let g = call(ctx, "drawing_get", json!({ "drawing": drawing }));
    let v = g["views"].as_array().unwrap().iter().find(|v| v["id"] == view).unwrap().clone();
    let xy = |p: &Value| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
    let mut lines: Vec<((f64, f64), (f64, f64))> = v["edges"].as_array().unwrap().iter().filter(|e| e["type"] == "line").map(|e| (xy(&e["a"]), xy(&e["b"]))).collect();
    let len = |(a, b): &((f64, f64), (f64, f64))| (b.0 - a.0).hypot(b.1 - a.1);
    lines.sort_by(|x, y| len(y).total_cmp(&len(x)));
    let op = registry::by_tool_name("drawing_dimension").unwrap();
    for l @ (a, b) in lines.iter().take(20) {
        let (mx, my, n) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, len(l));
        let (nx, ny) = ((a.1 - b.1) / n, (b.0 - a.0) / n);
        let t = Instant::now();
        if registry::run(ctx, op, &json!({ "drawing": drawing, "kind": "point_to_point", "picks": [[a.0, a.1], [b.0, b.1]], "place": [mx + 8.0 * nx, my + 8.0 * ny] })).is_ok() {
            return t.elapsed().as_secs_f64() * 1000.0;
        }
    }
    panic!("no line of view {view} takes a dimension");
}

/// Drawings of big models against live (user 11-02; live request "dx big
/// drawings"): live's 2000-instance assembly (its STEP in `NOSHAPE_LIVE_BIG`,
/// imported) with the suggested views, a BOM and a dimension; the same
/// 1000-part Part Studio built from live's own custom feature (Big1755
/// parts, 25 × 40) with the suggested views, then Columns 40 → 39 and the
/// drawing's Update; NIST's largest part (FTC 08) with the suggested views.
#[test]
#[ignore]
fn timed_big_drawings() {
    let scratch = std::env::temp_dir().join(format!("noshape-dx-bigdrw-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let live = std::env::var_os("NOSHAPE_LIVE_BIG").map(std::path::PathBuf::from);
    let views = |d: &Value| d["views"].as_array().map_or(0, |v| v.len());
    let timed = |ctx: &mut Ctx, what: &str, tool: &str, args: Value| {
        let t = Instant::now();
        let r = call(ctx, tool, args);
        report(what, vec![t.elapsed().as_secs_f64() * 1000.0]);
        r
    };
    if let Some(file) = live.as_ref().map(|d| d.join("dx-big-asm-ap242.step")).filter(|f| f.exists()) {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": scratch.join("asm.noshape").to_str().unwrap() }));
        let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
        let asm = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Assembly").unwrap()["name"].as_str().unwrap().to_string();
        let d = timed(&mut ctx, "big drawing: 2000-instance assembly, suggested views", "drawing_create", json!({ "source": asm, "template": "ANSI_A_INCH.dwt", "suggested_views": true }));
        assert_eq!(views(&d), 4, "{d}");
        let drawing = d["drawing"].as_str().unwrap().to_string();
        // The app shows the polygonal views first (then the exact ones).
        {
            use noshape_model::drawing::views;
            let doc = ctx.doc(None).unwrap().doc().clone();
            let tab = doc.tabs.iter().find(|t| t.name == drawing).unwrap().id;
            let dr = noshape_ops::drawing::drawing_of(&doc, tab).unwrap().clone();
            let sources = views::regenerate_sources(&doc, &dr);
            let t = Instant::now();
            views::compute_sheet_views(&dr, &dr.sheets[0], &sources, &views::ViewCache::new(), false);
            report("big drawing: assembly, polygonal views (app's first pass)", vec![t.elapsed().as_secs_f64() * 1000.0]);
        }
        let front = d["views"][0]["id"].as_i64().unwrap();
        let iso = d["views"][3]["id"].as_i64().unwrap();
        let b = timed(&mut ctx, "big drawing: assembly BOM", "drawing_view_table", json!({ "drawing": drawing, "kind": "bom", "view": iso, "position": [270, 210] }));
        eprintln!("  BOM: {}", b.to_string().chars().take(200).collect::<String>());
        report("big drawing: assembly dimension", vec![dimension_longest_line(&mut ctx, &drawing, front)]);
        timed(&mut ctx, "big drawing: assembly drawing_get", "drawing_get", json!({ "drawing": drawing, "edges": false }));
        for (format, ext) in [("PDF", "pdf"), ("DWG", "dwg"), ("DXF", "dxf"), ("PNG", "png")] {
            let file = scratch.join(format!("asm.{ext}"));
            timed(&mut ctx, &format!("big drawing: assembly export {format}"), "drawing_export", json!({ "drawing": drawing, "format": format, "file": file.to_str().unwrap() }));
            eprintln!("  {} KB", std::fs::metadata(&file).map(|m| m.len() / 1000).unwrap_or(0));
        }
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": scratch.join("parts.noshape").to_str().unwrap() }));
        call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
        let e = call(&mut ctx, "featurestudio_edit", json!({ "source": BIG1755 }));
        assert!(e["notices"].as_array().unwrap().iter().all(|n| n["severity"] != "error"), "{e}");
        let c = call(&mut ctx, "featurestudio_commit", json!({}));
        let ty = c["feature_types"].as_array().unwrap().iter().find(|t| t["name"] == "Big1755 parts").unwrap()["type"].as_str().unwrap().to_string();
        let f = timed(&mut ctx, "big drawing: 1000 parts regenerate", "feature_add", json!({ "tab": "Part Studio 1", "type": ty, "params": { "rows": 25, "cols": 40 } }));
        assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
        let fid = f["feature"].as_str().or(f["id"].as_str()).unwrap_or_else(|| panic!("{f}")).to_string();
        assert_eq!(call(&mut ctx, "query_parts", json!({ "tab": "Part Studio 1" }))["parts"].as_array().unwrap().len(), 1000);
        let d = timed(&mut ctx, "big drawing: 1000-part Part Studio, suggested views", "drawing_create", json!({ "source": "Part Studio 1", "template": "ANSI_A_INCH.dwt", "suggested_views": true }));
        assert_eq!(views(&d), 4, "{d}");
        let drawing = d["drawing"].as_str().unwrap().to_string();
        report("big drawing: Part Studio dimension", vec![dimension_longest_line(&mut ctx, &drawing, d["views"][0]["id"].as_i64().unwrap())]);
        timed(&mut ctx, "big drawing: Columns 40 -> 39 regenerate", "feature_edit", json!({ "tab": "Part Studio 1", "feature": fid, "params": { "cols": 39 } }));
        timed(&mut ctx, "big drawing: Part Studio drawing Update", "drawing_update", json!({ "drawing": drawing }));
    }
    if let Some(file) = live.as_ref().map(|d| d.join("nist/nist_ftc_08_asme1_ap242-e2.stp")).filter(|f| f.exists()) {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": scratch.join("nist.noshape").to_str().unwrap() }));
        let r = call(&mut ctx, "import_files", json!({ "files": [file.to_str().unwrap()] }));
        let studio = r["tabs"].as_array().unwrap().iter().find(|t| t["kind"] == "Part Studio").unwrap()["name"].as_str().unwrap().to_string();
        let d = timed(&mut ctx, "big drawing: NIST FTC 08 part, suggested views", "drawing_create", json!({ "source": studio, "template": "ANSI_A_INCH.dwt", "suggested_views": true }));
        assert_eq!(views(&d), 4, "{d}");
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// Live's "Big1755 parts" (behaviours request 1755): 25 × 40 blocks, holes,
/// fillets, names and colours.
const BIG1755: &str = r#"FeatureScript 3095;
import(path : "onshape/std/common.fs", version : "3095.0");

annotation { "Feature Type Name" : "Big1755 parts" }
export const big1755Parts = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Rows" }
        isInteger(definition.rows, { (unitless) : [1, 25, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Columns" }
        isInteger(definition.cols, { (unitless) : [1, 40, 100] } as IntegerBoundSpec);
    }
    {
        for (var r = 0; r < definition.rows; r += 1)
        {
            for (var c = 0; c < definition.cols; c += 1)
            {
                const k = r * definition.cols + c;
                const bid = id + ("b" ~ k);
                const x0 = c * 12;
                const y0 = r * 12;
                const h = 4 + (k % 5) + 0.25 * (r % 4);
                fCuboid(context, bid, { "corner1" : vector(x0, y0, 0) * millimeter, "corner2" : vector(x0 + 8, y0 + 8, h) * millimeter });
                const body = qCreatedBy(bid, EntityType.BODY);
                if (k % 10 == 3)
                {
                    fCylinder(context, id + ("h" ~ k), { "topCenter" : vector(x0 + 4, y0 + 4, h + 1) * millimeter, "bottomCenter" : vector(x0 + 4, y0 + 4, -1) * millimeter, "radius" : 2 * millimeter });
                    opBoolean(context, id + ("s" ~ k), { "tools" : qCreatedBy(id + ("h" ~ k), EntityType.BODY), "targets" : body, "operationType" : BooleanOperationType.SUBTRACTION });
                }
                else if (k % 10 == 7)
                {
                    opFillet(context, id + ("f" ~ k), { "entities" : qParallelEdges(qOwnedByBody(body, EntityType.EDGE), Z_DIRECTION), "radius" : 1.5 * millimeter });
                }
                setProperty(context, { "entities" : body, "propertyType" : PropertyType.NAME, "value" : "B" ~ r ~ "-" ~ c });
                if (k % 10 == 5)
                {
                    setProperty(context, { "entities" : body, "propertyType" : PropertyType.APPEARANCE, "value" : color(0.9, 0.1, 0.1) });
                }
            }
        }
    });
"#;
