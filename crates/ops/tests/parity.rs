//! The model parity suite (docs/PARITY-SUITES.md §1): Onshape Part Studios
//! rebuilt feature by feature and compared with live after each feature.

use noshape_ops::Ctx;
use noshape_ops::parity::{Translator, differences, live_parts, our_parts, parse_refs, refs_from_bodies};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("noshape-parity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// One document's result: features matching live, and the first that doesn't.
struct ModelResult {
    name: String,
    features: usize,
    /// Features with a live.jsonl line.
    compared: usize,
    matching: usize,
    first: Option<String>,
    untranslated: Vec<String>,
    /// Full regeneration: ours (reopened) and live's, ms.
    regen_ms: Option<(f64, Option<f64>)>,
    /// Of it, opening the file and regenerating it (feature_list), ms; the
    /// rest is query_parts' measures.
    open_ms: f64,
    list_ms: f64,
    /// Our slowest features in it (name, ms).
    slowest: Vec<(String, f64)>,
}

/// Rebuilds `features` and compares with `live` (live.jsonl lines) after each feature.
fn run_model(name: &str, features: &Value, live: &[Value], refs: &str) -> ModelResult {
    let mut ctx = Ctx::new();
    let path = scratch(&format!("{name}.noshape"));
    let mut res = ModelResult { name: name.to_string(), features: 0, compared: 0, matching: 0, first: None, untranslated: Vec::new(), regen_ms: None, open_ms: 0.0, list_ms: 0.0, slowest: Vec::new() };
    // refs.jsonl wins over the geometry read from live's bodydetails.
    let mut all = refs_from_bodies(live);
    all.extend(parse_refs(refs));
    let mut t = match Translator::open(&mut ctx, &path, features, all) {
        Ok(t) => t,
        Err(e) => {
            res.first = Some(format!("not opened: {e}"));
            return res;
        }
    };
    res.features = t.len();
    res.untranslated.extend(t.notes.iter().map(|n| format!("document: {n}")));
    for k in 0..t.len() {
        let step = t.step(k);
        for p in &step.problems {
            res.untranslated.push(format!("{} ({}): {p}", step.name, step.feature_type));
        }
        // Not compared where live has no line (a dump of the end state only).
        let Some(line) = live.iter().find(|l| l["index"].as_u64() == Some(k as u64)) else { continue };
        res.compared += 1;
        let mut diff = Vec::new();
        let live_status = line["status"].as_str().unwrap_or("OK").to_uppercase();
        let ours = step.status.clone().unwrap_or_else(|| "not built".into()).to_uppercase();
        if ours != live_status {
            let why = step.message.as_deref().map(|m| format!(" ({m})")).unwrap_or_default();
            diff.push(format!("status {ours}{why}, live {live_status}"));
        }
        // Onshape's words: live's row tooltip ("Fillet 1 did not regenerate
        // properly: …") holds our message.
        if let Some(m) = line["message"].as_str().filter(|m| !m.is_empty())
            && !step.message.as_deref().is_some_and(|ours| !ours.is_empty() && m.contains(ours))
        {
            diff.push(format!("message {:?}, live {m:?}", step.message.as_deref().unwrap_or("")));
        }
        match our_parts(&mut t) {
            Ok(parts) => diff.extend(differences(&live_parts(line), &parts)),
            Err(e) => diff.push(format!("parts: {e}")),
        }
        if diff.is_empty() {
            res.matching += 1;
        } else if res.first.is_none() {
            res.first = Some(format!("feature {k} \"{}\" ({}): {}", step.name, step.feature_type, diff.join("; ")));
        }
    }
    drop(t);
    // Our full regeneration: the document reopened, its parts queried.
    let mut fresh = Ctx::new();
    let t0 = std::time::Instant::now();
    let run = |ctx: &mut Ctx, tool: &str, args: Value| noshape_ops::registry::by_tool_name(tool).is_some_and(|op| noshape_ops::registry::run(ctx, op, &args).is_ok());
    let opened = run(&mut fresh, "document_open", json!({ "path": path.to_str().unwrap() }));
    res.open_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let list = opened.then(|| noshape_ops::registry::by_tool_name("feature_list").map(|op| noshape_ops::registry::run(&mut fresh, op, &json!({})))).flatten();
    res.list_ms = t0.elapsed().as_secs_f64() * 1000.0 - res.open_ms;
    if opened && run(&mut fresh, "query_parts", json!({})) {
        res.regen_ms = Some((t0.elapsed().as_secs_f64() * 1000.0, live.iter().find_map(|l| l["full_regen_ms"].as_f64())));
        // Its slowest features (the Regeneration times panel's).
        if let Some(Ok(list)) = list {
            let mut times: Vec<(String, f64)> = list.value["features"].as_array().into_iter().flatten().filter_map(|f| Some((f["name"].as_str()?.to_string(), f["regen_ms"].as_f64()?))).collect();
            times.sort_by(|a, b| b.1.total_cmp(&a.1));
            res.slowest = times.into_iter().take(3).filter(|t| t.1 >= 1.0).collect();
        }
    }
    res
}

fn read_model(dir: &Path) -> Option<(Value, Vec<Value>, String)> {
    let features: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("features.json")).ok()?).ok()?;
    let live = std::fs::read_to_string(dir.join("live.jsonl")).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|l| l.get("index").is_some()).collect();
    let refs = std::fs::read_to_string(dir.join("refs.jsonl")).unwrap_or_default();
    Some((features, live, refs))
}

/// Every document under /root/parity/models (NOSHAPE_PARITY_MODELS; one
/// name with NOSHAPE_PARITY_ONLY): the first feature that differs per
/// document, what wasn't translated, and the score. Writes report.json next
/// to the models; NOSHAPE_PARITY_INDEX=1 rewrites tests/parity-models/index.json
/// from their source.json. The models are on this machine only: run it here
/// (NOSHAPE_RB_LOCAL=1 with tools/tiers/rb.sh), or copy them and point
/// NOSHAPE_PARITY_MODELS at the copy.
#[test]
#[ignore = "T2: live's models from /root/parity/models (other people's work, never in the repo)"]
fn parity_models() {
    let root = PathBuf::from(std::env::var("NOSHAPE_PARITY_MODELS").unwrap_or_else(|_| "/root/parity/models".into()));
    let only = std::env::var("NOSHAPE_PARITY_ONLY").ok();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("features.json").is_file()).collect()).unwrap_or_default();
    dirs.sort();
    let mut results = Vec::new();
    let mut index = Vec::new();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        if only.as_ref().is_some_and(|o| *o != name) {
            continue;
        }
        if let Ok(s) = std::fs::read_to_string(dir.join("source.json")).map(|t| serde_json::from_str::<Value>(&t).unwrap_or(Value::Null)) {
            // Live's source.json: live's copy (fetched from) and the public original.
            let c = &s["copy"];
            index.push(json!({ "name": name, "document": c["did"], "workspace": c["wid"], "element": c["eid"], "url": c["url"], "title": s["title"], "original": s["original"]["url"] }));
        }
        // Live's fetch skipped or didn't finish it: nothing to compare.
        if !dir.join("live.jsonl").is_file() {
            let why = ["SKIPPED.txt", "INCOMPLETE.txt"].iter().find_map(|f| std::fs::read_to_string(dir.join(f)).ok()).unwrap_or_default();
            println!("{name}: no live.jsonl, not scored ({})", why.trim());
            // NOSHAPE_PARITY_UNSCORED=1: still translated, for what isn't.
            if std::env::var_os("NOSHAPE_PARITY_UNSCORED").is_some()
                && let Some((features, _, refs)) = read_model(&dir)
            {
                for u in run_model(&name, &features, &[], &refs).untranslated {
                    println!("  untranslated: {u}");
                }
            }
            continue;
        }
        let Some((features, live, refs)) = read_model(&dir) else {
            println!("{name}: unreadable features.json");
            continue;
        };
        let r = run_model(&name, &features, &live, &refs);
        let speed = match r.regen_ms {
            Some((ours, Some(l))) => format!("; reopened and regenerated in {ours:.0} ms (open {:.0} ms, regenerate {:.0} ms; live full regeneration {l:.0} ms)", r.open_ms, r.list_ms),
            Some((ours, None)) => format!("; reopened and regenerated in {ours:.0} ms (open {:.0} ms, regenerate {:.0} ms)", r.open_ms, r.list_ms),
            None => String::new(),
        };
        let speed = if r.slowest.is_empty() { speed } else { format!("{speed} (slowest: {})", r.slowest.iter().map(|(n, ms)| format!("{n} {ms:.0} ms")).collect::<Vec<_>>().join(", ")) };
        println!("{}: {}/{} features match ({} of {} compared){}{speed}", r.name, r.matching, r.compared, r.compared, r.features, r.first.as_ref().map(|f| format!("; first difference: {f}")).unwrap_or_else(|| "; matches to the end".into()));
        for u in &r.untranslated {
            println!("  untranslated: {u}");
        }
        results.push(r);
    }
    let docs = results.iter().filter(|r| r.first.is_none() && r.compared > 0).count();
    let (matching, total) = results.iter().fold((0, 0), |(m, t), r| (m + r.matching, t + r.compared));
    println!("parity_models score: {docs}/{} documents match to the end, {matching}/{total} features match", results.len());
    // The causes: first differences by feature type and what differed.
    let mut causes: std::collections::BTreeMap<String, usize> = Default::default();
    for f in results.iter().filter_map(|r| r.first.as_deref()) {
        let ty = f.split(" (").nth(1).and_then(|r| r.split(')').next()).unwrap_or("?");
        // Its first difference without part names, numbers as #, live's side dropped.
        let first = f.split_once("): ").map(|(_, d)| d).unwrap_or("").split("; ").next().unwrap_or("");
        let first = first.split(", live").next().unwrap_or(first);
        let first = match first.split_once("\": ") {
            Some((_, rest)) if first.starts_with('"') => rest,
            _ => first,
        };
        let mut what = String::new();
        for c in first.chars() {
            if c.is_ascii_digit() {
                if !what.ends_with('#') {
                    what.push('#');
                }
            } else if !(c == '.' && what.ends_with('#')) {
                what.push(c);
            }
        }
        *causes.entry(format!("{ty}: {what}")).or_default() += 1;
    }
    let mut causes: Vec<(String, usize)> = causes.into_iter().collect();
    causes.sort_by_key(|c| std::cmp::Reverse(c.1));
    for (c, n) in causes.iter().take(5) {
        println!("  cause: {n} × {c}");
    }
    println!("{}", score_row(docs, results.len(), matching, total, &causes));
    let report: Vec<Value> = results.iter().map(|r| json!({ "name": r.name, "features": r.features, "compared": r.compared, "matching": r.matching, "regen_ms": r.regen_ms.map(|(o, l)| json!({ "ours": o, "live": l })), "slowest": r.slowest, "first_difference": r.first, "untranslated": r.untranslated })).collect();
    let _ = std::fs::write(root.with_file_name("report.json"), serde_json::to_string_pretty(&json!({ "documents_matching": docs, "documents": results.len(), "features_matching": matching, "features": total, "causes": causes.iter().map(|(c, n)| json!({ "cause": c, "documents": n })).collect::<Vec<_>>(), "models": report })).unwrap());
    if std::env::var_os("NOSHAPE_PARITY_INDEX").is_some() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity-models/index.json");
        std::fs::write(file, serde_json::to_string_pretty(&index).unwrap() + "\n").unwrap();
    }
}

/// One docs/parity/SCORE.md row, to paste as is: UTC date, the build
/// (NOSHAPE_PARITY_BUILD, else this checkout's commit), models and features
/// matched, the scenarios column left to psui's runner, the five biggest
/// causes (qa adds their tickets).
fn score_row(docs: usize, total_docs: usize, matching: usize, features: usize, causes: &[(String, usize)]) -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let build = std::env::var("NOSHAPE_PARITY_BUILD").ok().or_else(|| {
        let out = std::process::Command::new("git").args(["rev-parse", "--short=8", "HEAD"]).current_dir(env!("CARGO_MANIFEST_DIR")).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    });
    let top: Vec<String> = causes.iter().take(5).map(|(c, n)| format!("{n}× {c}")).collect();
    format!("SCORE.md row: | {year}-{month:02}-{day:02} | {} | {docs}/{total_docs} | {matching}/{features} | – | {} |", build.unwrap_or_else(|| "?".into()), if top.is_empty() { "–".into() } else { top.join("; ") })
}

/// A small Part Studio as Onshape's REST gives it: a 60 × 40 rectangle with
/// a Ø8 hole on Top, extruded 20 mm (all the sketch's regions but the
/// hole's, by geometry), a 3 mm fillet on one vertical edge, and live's
/// state after each feature.
fn block_model() -> (Value, Vec<Value>, String) {
    let line = |id: &str, (x0, y0): (f64, f64), (x1, y1): (f64, f64)| {
        let l = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
        json!({ "btType": "BTMSketchCurveSegment-155", "entityId": id, "isConstruction": false, "startParam": 0.0, "endParam": l,
            "geometry": { "btType": "BTCurveGeometryLine-117", "pntX": x0, "pntY": y0, "dirX": (x1 - x0) / l, "dirY": (y1 - y0) / l } })
    };
    let q = |v: &str| json!({ "btType": "BTMParameterQuantity-147", "parameterId": "", "expression": v });
    let mut depth = q("20 mm");
    depth["parameterId"] = json!("depth");
    let mut radius = q("3 mm");
    radius["parameterId"] = json!("radius");
    let mut dia = q("8 mm");
    dia["parameterId"] = json!("length");
    let features = json!({ "features": [
        { "btType": "BTMSketch-151", "featureType": "newSketch", "featureId": "FS1", "name": "Sketch 1",
          "parameters": [{ "btType": "BTMParameterQueryList-148", "parameterId": "sketchPlane", "queries": [{ "btType": "BTMIndividualQuery-138", "deterministicIds": ["JDC"], "queryString": "query=qCompressed(1.0,\"%B5$QueryM4S…TopplaneOp…\",id);" }] }],
          "entities": [line("r0", (0.0, 0.0), (0.06, 0.0)), line("r1", (0.06, 0.0), (0.06, 0.04)), line("r2", (0.06, 0.04), (0.0, 0.04)), line("r3", (0.0, 0.04), (0.0, 0.0)),
                       { "btType": "BTMSketchCurve-4", "entityId": "c1", "isConstruction": false, "geometry": { "btType": "BTCurveGeometryCircle-115", "radius": 0.004, "xCenter": 0.015, "yCenter": 0.02, "xDir": 1.0, "yDir": 0.0, "clockwise": false } }],
          "constraints": [{ "btType": "BTMSketchConstraint-2", "constraintType": "DIAMETER", "parameters": [{ "btType": "BTMParameterString-149", "parameterId": "localFirst", "value": "c1" }, dia] },
                          { "btType": "BTMSketchConstraint-2", "constraintType": "COINCIDENT", "parameters": [] }] },
        { "btType": "BTMFeature-134", "featureType": "extrude", "featureId": "FE1", "name": "Extrude 1", "parameters": [
            { "btType": "BTMParameterEnum-145", "parameterId": "bodyType", "value": "SOLID" },
            { "btType": "BTMParameterEnum-145", "parameterId": "domain", "value": "MODEL" },
            { "btType": "BTMParameterQueryList-148", "parameterId": "entities", "queries": [{ "btType": "BTMIndividualSketchRegionQuery-140", "deterministicIds": ["JGC"], "featureId": "FS1" }] },
            { "btType": "BTMParameterEnum-145", "parameterId": "endBound", "value": "BLIND" }, depth] },
        { "btType": "BTMFeature-134", "featureType": "fillet", "featureId": "FF1", "name": "Fillet 1", "parameters": [
            { "btType": "BTMParameterQueryList-148", "parameterId": "entities", "queries": [{ "btType": "BTMIndividualQuery-138", "deterministicIds": ["JHt"] }] }, radius] }
    ] });
    let refs = [
        json!({ "index": 0, "id": "JDC", "kind": "PLANE", "point": [0.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0] }),
        json!({ "index": 1, "id": "JGC", "kind": "SKETCH_REGION", "point": [0.0303212, 0.02, 0.0], "measure": 0.00234973452 }),
        json!({ "index": 2, "id": "JHt", "kind": "EDGE", "point": [0.06, 0.04, 0.01], "direction": [0.0, 0.0, 1.0] }),
    ]
    .iter()
    .map(|v| v.to_string())
    .collect::<Vec<_>>()
    .join("\n");
    let block = 0.06 * 0.04 * 0.02 - std::f64::consts::PI * 0.004 * 0.004 * 0.02;
    let fillet_off = 0.003 * 0.003 * (1.0 - std::f64::consts::FRAC_PI_4) * 0.02;
    let part = |v: f64, f: u64, e: u64, vx: u64| json!([{ "name": "Part 1", "bodyType": "SOLID", "faces": f, "edges": e, "vertices": vx, "volume": v, "area": null, "bbox": { "min": [0.0, 0.0, 0.0], "max": [0.06, 0.04, 0.02] } }]);
    let live = vec![
        json!({ "index": 0, "status": "OK", "parts": [] }),
        json!({ "index": 1, "status": "OK", "parts": part(block, 7, 14, 8) }),
        json!({ "index": 2, "status": "OK", "parts": part(block - fillet_off, 8, 17, 10) }),
    ];
    (features, live, refs)
}

#[test]
fn translates_a_sketch_extrude_and_fillet() {
    let (features, mut live, refs) = block_model();
    // Area is not given here: only what is given is compared.
    for l in &mut live {
        for p in l["parts"].as_array_mut().into_iter().flatten() {
            p.as_object_mut().unwrap().remove("area");
        }
    }
    let r = run_model("block", &features, &live, &refs);
    assert_eq!((r.matching, r.features), (3, 3), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
    // What isn't translated is said, per feature.
    // The COINCIDENT without entities is counted, not dropped.
    assert!(r.untranslated.iter().any(|u| u.contains("constraints left to the solved geometry: 1 COINCIDENT rejected")), "{:?}", r.untranslated);
}

/// Without live's geometry a query is reported, not guessed.
#[test]
fn an_unmapped_query_is_reported() {
    let (features, live, _) = block_model();
    let r = run_model("block-norefs", &features, &live, "");
    assert!(r.untranslated.iter().any(|u| u.starts_with("Fillet 1 (fillet): entities: query not mapped (no live geometry for JHt)")), "{:?}", r.untranslated);
    assert_eq!(r.matching, 1, "the sketch on Top still builds (its plane read from the query): {:?}", r.first);
}

/// A region query in Onshape's packed form (live Extrude 1 of
/// dom/fs-pack-src: the FACE whose TOPOLOGY is sketch lines r0…r3) picks the
/// rectangle, not the circle beside it, without live's geometry.
/// Live's dom/fs-pack-src Sketch 1 (a 40 × 30 rectangle on Top, a Ø10
/// circle beside it) and Extrude 1 (10 mm of the rectangle, by its packed
/// region query), with `more` features after them.
fn pack_src_model(more: Vec<Value>) -> Value {
    let line = |id: &str, p: (f64, f64), d: (f64, f64), l: f64| json!({ "btType": "BTMSketchCurveSegment-155", "entityId": id, "startParam": 0.0, "endParam": l, "geometry": { "btType": "BTCurveGeometryLine-117", "pntX": p.0, "pntY": p.1, "dirX": d.0, "dirY": d.1 } });
    let packed = r#"query=qCompressed(1.0,"&1f3$eJydkFFPwjAUhf/M9VFCC2PwuHUFGoXhbX3AF7KtjTQGNrui2b+3DEM0mpj40vSec79ze3uTRvBwMq5bRZJQ0LYtDqV9PhXe1ses8EVCVvSno7rGyCmofJPf54ttuJqjt96aNhknlA1DWgnaOPtm9NzVh14pLk3dGU4L4NdCjoFnCx6QvW197brPeIY8USJfB6NujOsnC51SEDohkpDBBObl0yOtl9F+MzMpm+yG79aZvJEzeD1v1QdpkHdcseWOr5VQW2mgfTG+2l8eILSk4IYY4QRjnOKMVSH8Vg9isIfG2aPHSsYgVhsUa5WRy3o47s8rhCVWqNGcs8gXmdCgkxGS6G+O/pMb/cphxOLwsfOE8e/eB3T+mcU=",id);"#;
    let mut features = json!({ "features": [
        { "btType": "BTMSketch-151", "featureType": "newSketch", "featureId": "FbZU2oH5hP9eBC6_0", "name": "Sketch 1",
          "parameters": [{ "btType": "BTMParameterQueryList-148", "parameterId": "sketchPlane", "queries": [{ "btType": "BTMIndividualQuery-138", "queryString": "query=qCompressed(1.0,\"%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY\",id);" }] }],
          "entities": [line("r0", (0.0, 0.0), (1.0, 0.0), 0.04), line("r1", (0.04, 0.0), (0.0, 1.0), 0.03), line("r2", (0.04, 0.03), (-1.0, 0.0), 0.04), line("r3", (0.0, 0.03), (0.0, -1.0), 0.03),
                       { "btType": "BTMSketchCurve-4", "entityId": "c1", "geometry": { "btType": "BTCurveGeometryCircle-115", "radius": 0.005, "xCenter": 0.06, "yCenter": 0.015, "xDir": 1.0, "yDir": 0.0, "clockwise": false } }],
          "constraints": [] },
        { "btType": "BTMFeature-134", "featureType": "extrude", "featureId": "FyUE18XOUbMicpL_0", "name": "Extrude 1", "parameters": [
            { "btType": "BTMParameterQueryList-148", "parameterId": "entities", "queries": [{ "btType": "BTMIndividualQuery-138", "deterministicIds": ["JGC"], "queryString": packed }] },
            { "btType": "BTMParameterQuantity-147", "parameterId": "depth", "expression": "10 mm" }] }
    ] });
    features["features"].as_array_mut().unwrap().extend(more);
    features
}

#[test]
fn a_region_query_is_read_from_onshapes_packed_form() {
    let features = pack_src_model(Vec::new());
    let live = vec![json!({ "index": 1, "status": "OK", "parts": [{ "faces": 6, "edges": 12, "vertices": 8, "volume": 0.04 * 0.03 * 0.01, "bbox": { "min": [0.0, 0.0, 0.0], "max": [0.04, 0.03, 0.01] } }] })];
    let r = run_model("packed-region", &features, &live, "");
    assert_eq!((r.matching, r.compared), (1, 1), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// An ellipse (Onshape's BTCurveGeometryEllipse) extruded: π·a·b·h.
#[test]
fn an_ellipse_is_rebuilt() {
    let features = json!({ "features": [
        { "btType": "BTMSketch-151", "featureType": "newSketch", "featureId": "FS", "name": "Sketch 1",
          "parameters": [{ "btType": "BTMParameterQueryList-148", "parameterId": "sketchPlane", "queries": [{ "btType": "BTMIndividualQuery-138", "queryString": "query=dummyQuery(id+\"Front.planeOp\",EntityType.FACE);" }] }],
          "entities": [{ "btType": "BTMSketchCurve-4", "entityId": "e1", "geometry": { "btType": "BTCurveGeometryEllipse-1189", "majorRadius": 0.02, "minorRadius": 0.01, "xCenter": 0.0, "yCenter": 0.0, "xDir": 1.0, "yDir": 0.0, "clockwise": false } }],
          "constraints": [] },
        { "btType": "BTMFeature-134", "featureType": "extrude", "featureId": "FE", "name": "Extrude 1", "parameters": [
            { "btType": "BTMParameterQueryList-148", "parameterId": "entities", "queries": [{ "btType": "BTMIndividualSketchRegionQuery-140", "queryString": "query = qSketchRegion(id + \"FS\", true);" }] },
            { "btType": "BTMParameterQuantity-147", "parameterId": "depth", "expression": "10 mm" }] }
    ] });
    let live = vec![json!({ "index": 1, "status": "OK", "parts": [{ "faces": 3, "edges": 2, "vertices": 0, "volume": std::f64::consts::PI * 0.02 * 0.01 * 0.01 }] })];
    let r = run_model("ellipse", &features, &live, "");
    assert_eq!((r.matching, r.compared), (1, 1), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// The fillet's edge placed from the bodydetails live's line after the
/// extrude carries (no refs.jsonl line for it): of the four vertical edges,
/// the one at (60, 40).
#[test]
fn an_edge_is_placed_from_live_bodydetails() {
    let (features, mut live, refs) = block_model();
    let refs: String = refs.lines().filter(|l| !l.contains("JHt")).collect::<Vec<_>>().join("\n");
    let v = |x: f64, y: f64, z: f64| json!({ "x": x, "y": y, "z": z });
    let edge = |id: &str, x: f64, y: f64| json!({ "id": id, "curve": { "type": "LINE" }, "geometry": { "midPoint": v(x, y, 0.01), "length": 0.02 } });
    live[1]["bodies"] = json!([{ "id": "JHD", "type": "SOLID", "faces": [], "vertices": [], "edges": [edge("JHB", 0.0, 0.0), edge("JHN", 0.06, 0.0), edge("JHt", 0.06, 0.04), edge("JHp", 0.0, 0.04)] }]);
    for l in &mut live {
        for p in l["parts"].as_array_mut().into_iter().flatten() {
            p.as_object_mut().unwrap().remove("area");
        }
    }
    let r = run_model("block-bodies", &features, &live, &refs);
    assert_eq!((r.matching, r.compared), (3, 3), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// Without live's geometry, Onshape's own names for an Extrude's edges are
/// read (live dom/fs-pack-src): Fillet 1's SWEPT_EDGE from r0 / r3 (the
/// vertical edge at the origin), Chamfer 1's end CAP_EDGE of r2 (the top
/// edge at y = 30).
#[test]
fn extrude_edges_are_read_from_onshapes_names() {
    let q = |id: &str, text: &str| json!({ "btType": "BTMParameterQueryList-148", "parameterId": id, "queries": [{ "btType": "BTMIndividualQuery-138", "deterministicIds": ["JXX"], "queryString": text }] });
    let n = |id: &str, v: &str| json!({ "btType": "BTMParameterQuantity-147", "parameterId": id, "expression": v });
    let fillet = r#"query=qCompressed(1.0,"%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS13$ORIGINAL_DEPENDENCYS9$originalsA2C0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS2$r0C0M5R4R5R6R7R8RaRbRcRdS2$r3R4R5R6R7R8CbA1S11.9$FyUE18XOUbMicpL_0opExtrudeRbSa$SWEPT_EDGE",id);"#;
    let chamfer = r#"query=qCompressed(1.0,"%B5$QueryM6S12$disambiguationDataA1M2S12$disambiguationTypeS13$ORIGINAL_DEPENDENCYS9$originalsA1C0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS2$r2R4R5R6R7S7$isStartFR8CbA1S11.9$FyUE18XOUbMicpL_0opExtrudeRbS8$CAP_EDGE",id);"#;
    let features = pack_src_model(vec![
        json!({ "btType": "BTMFeature-134", "featureType": "fillet", "featureId": "FpTbfjg0cxfx7cp_1", "name": "Fillet 1", "parameters": [q("entities", fillet), n("radius", "3 mm")] }),
        json!({ "btType": "BTMFeature-134", "featureType": "chamfer", "featureId": "FOsbB0EugxlexG1_1", "name": "Chamfer 1", "parameters": [q("entities", chamfer), n("width", "2 mm")] }),
    ]);
    let mm3 = 1e-9;
    let fillet_off = (9.0 - 9.0 * std::f64::consts::FRAC_PI_4) * 10.0 * mm3;
    let live = vec![
        json!({ "index": 2, "status": "OK", "parts": [{ "faces": 7, "edges": 15, "vertices": 10, "volume": 12000.0 * mm3 - fillet_off }] }),
        json!({ "index": 3, "status": "OK", "parts": [{ "faces": 8, "edges": 18, "vertices": 12, "volume": 12000.0 * mm3 - fillet_off - 80.0 * mm3, "bbox": { "min": [0.0, 0.0, 0.0], "max": [0.04, 0.03, 0.01] } }] }),
    ];
    let r = run_model("packed-names", &features, &live, "");
    assert_eq!((r.matching, r.compared), (2, 2), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// An inch document (features.json `units`): a bare number is inches, so the
/// fillet's "0.11811023622" is the same 3 mm, and every measure is compared
/// in SI whatever our workspace units.
#[test]
fn an_inch_documents_bare_numbers_are_inches() {
    let (mut features, mut live, refs) = block_model();
    features["units"] = json!({ "length": "inch", "angle": "degree" });
    for f in features["features"].as_array_mut().unwrap() {
        for p in f["parameters"].as_array_mut().into_iter().flatten() {
            if p["parameterId"] == "radius" {
                p["expression"] = json!("0.11811023622");
            }
        }
    }
    for l in &mut live {
        for p in l["parts"].as_array_mut().into_iter().flatten() {
            p.as_object_mut().unwrap().remove("area");
        }
    }
    let r = run_model("block-inch", &features, &live, &refs);
    assert_eq!((r.matching, r.compared), (3, 3), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// The comparison: counts exact; volume, area and centroid within live's
/// band (REST's low-accuracy values) or 1e-6; anything else is named.
#[test]
fn differences_use_lives_bands() {
    use noshape_ops::parity::PartState;
    let part = |volume: f64, faces: u64| PartState { name: "Part 1".into(), body_type: "solid".into(), faces, edges: 12, vertices: 8, volume: Some(volume), area: None, volume_band: None, area_band: None, bbox: None, centroid: Some([0.01, 0.0, 0.0]), ..Default::default() };
    let mut live = part(1.0e-6, 6);
    live.volume_band = Some([0.999e-6, 1.001e-6]);
    live.centroid = Some([0.0100005, 0.0, 0.0]);
    live.centroid_band = Some([[0.0099, -1e-4, -1e-4], [0.0101, 1e-4, 1e-4]]);
    assert!(differences(&[live.clone()], &[part(1.0005e-6, 6)]).is_empty(), "inside live's bands");
    let d = differences(&[live.clone()], &[part(1.002e-6, 7)]);
    assert!(d.iter().any(|x| x.contains("7/12/8 faces/edges/vertices, live 6/12/8")) && d.iter().any(|x| x.contains("volume")), "{d:?}");
    assert_eq!(differences(&[live.clone()], &[]), ["0 parts, live 1"]);
    // The same counts but other geometry: a fillet face live makes a cylinder.
    let kinds = |pairs: &[(&str, usize)]| Some(pairs.iter().map(|(k, n)| (k.to_string(), *n)).collect());
    let (mut a, mut b) = (live.clone(), part(1.0e-6, 6));
    a.face_types = kinds(&[("plane", 5), ("cylinder", 1)]);
    b.face_types = kinds(&[("plane", 5), ("other", 1)]);
    assert_eq!(differences(&[a], &[b]), ["\"Part 1\": faces 1 other, 5 plane, live 1 cylinder, 5 plane"]);
}

/// Variables from a Variable Studio (live's `variables` with a studio
/// reference): ours makes the studio, so the Extrude's "#d" is 20 mm.
#[test]
fn variable_studio_variables_are_rebuilt() {
    let (mut features, mut live, refs) = block_model();
    features["variables"] = json!([{ "variableStudioReference": { "elementId": "e1" }, "variables": [{ "name": "d", "type": "LENGTH", "expression": "20 mm" }] }]);
    for f in features["features"].as_array_mut().unwrap() {
        for p in f["parameters"].as_array_mut().into_iter().flatten() {
            if p["parameterId"] == "depth" {
                p["expression"] = json!("#d");
            }
        }
    }
    for l in &mut live {
        for p in l["parts"].as_array_mut().into_iter().flatten() {
            p.as_object_mut().unwrap().remove("area");
        }
    }
    let r = run_model("block-vs", &features, &live, &refs);
    assert_eq!((r.matching, r.compared), (3, 3), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// A custom feature: its Feature Studio's source (features.json
/// `featureStudios`) becomes ours, and the feature (namespace "e<element>::m…")
/// runs from it: a 10 mm cube.
#[test]
fn a_custom_feature_runs_from_its_feature_studio() {
    let source = r#"FeatureScript 3095;
import(path : "onshape/std/common.fs", version : "3095.0");

annotation { "Feature Type Name" : "Cube" }
export const cube = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Size" }
        isLength(definition.size, LENGTH_BOUNDS);
    }
    {
        fCuboid(context, id + "cuboid", { "corner1" : vector(0, 0, 0) * meter, "corner2" : vector(1, 1, 1) * definition.size });
    });
"#;
    let features = json!({
        "featureStudios": { "0123": { "name": "Cubes", "source": source } },
        "features": [{ "btType": "BTMFeature-134", "featureType": "cube", "namespace": "e0123::m4567", "featureId": "FC", "name": "Cube 1",
            "parameters": [{ "btType": "BTMParameterQuantity-147", "parameterId": "size", "expression": "10 mm" }] }]
    });
    let live = vec![json!({ "index": 0, "status": "OK", "parts": [{ "faces": 6, "edges": 12, "vertices": 8, "volume": 1e-6 }] })];
    let r = run_model("custom-cube", &features, &live, "");
    assert_eq!((r.matching, r.compared), (1, 1), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// `NOSHAPE_PACKED_DIR=<dir>`: every packed query (`qCompressed(1.0, "…", id)`)
/// in the feature lists under `dir` decodes (the translator reads regions and
/// Onshape's names from them).
#[test]
#[ignore = "T2: needs NOSHAPE_PACKED_DIR (feature lists from live, not in the repo)"]
fn packed_queries_decode() {
    let Some(dir) = std::env::var_os("NOSHAPE_PACKED_DIR") else { return };
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) if s.contains("qCompressed(1.0,\"") => out.extend(s.split("qCompressed(1.0,\"").skip(1).filter_map(|r| r.split('"').next()).map(str::to_string)),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(m) => m.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut files = vec![PathBuf::from(dir)];
    let (mut total, mut failed) = (0, Vec::new());
    while let Some(p) = files.pop() {
        if p.is_dir() {
            files.extend(std::fs::read_dir(&p).unwrap().filter_map(|e| e.ok().map(|e| e.path())));
            continue;
        }
        // Feature lists only (not excerpts of FeatureScript source).
        if !p.file_name().is_some_and(|n| n.to_string_lossy().contains("features")) {
            continue;
        }
        let Ok(v) = std::fs::read_to_string(&p).map(|t| serde_json::from_str::<Value>(&t).unwrap_or(Value::Null)) else { continue };
        let mut packed = Vec::new();
        walk(&v, &mut packed);
        for q in packed {
            total += 1;
            if let Err(e) = noshape_model::custom::host::packed::unpack(&q) {
                failed.push(format!("{}: {e}: {}", p.display(), q.chars().take(60).collect::<String>()));
            }
        }
    }
    println!("packed queries: {total}, failed {}", failed.len());
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Onshape's constraints keep a sketch where Onshape solved it: a clockwise
/// arc (ours runs the other way, its ends mapped by position) tangent to two
/// lines and dimensioned, a centre-point rectangle's MIDPOINT between two of
/// its corners, an arc whose end parameter wraps past 2π (π → 0, the lower
/// half) and a distance to a circle's far side. Nothing moves, nothing is
/// taken back, and the wrapping arc's half disc lies below its chord.
#[test]
fn onshapes_constraints_dont_move_the_sketch() {
    let line = |id: &str, p: [f64; 2], d: [f64; 2], t: [f64; 2]| json!({ "btType": "BTMSketchCurveSegment-155", "entityId": id, "startPointId": format!("{id}.start"), "endPointId": format!("{id}.end"), "startParam": t[0], "endParam": t[1], "geometry": { "btType": "BTCurveGeometryLine-117", "pntX": p[0], "pntY": p[1], "dirX": d[0], "dirY": d[1] } });
    let arc = |id: &str, c: [f64; 2], r: f64, cw: bool, t: [f64; 2]| json!({ "btType": "BTMSketchCurveSegment-155", "entityId": id, "startPointId": format!("{id}.start"), "endPointId": format!("{id}.end"), "centerId": format!("{id}.center"), "startParam": t[0], "endParam": t[1], "geometry": { "btType": "BTCurveGeometryCircle-115", "xCenter": c[0], "yCenter": c[1], "radius": r, "xDir": 1.0, "yDir": 0.0, "clockwise": cw } });
    let s = |id: &str, v: &str| json!({ "btType": "BTMParameterString-149", "parameterId": id, "value": v });
    let q = |id: &str, e: &str| json!({ "btType": "BTMParameterQuantity-147", "parameterId": id, "expression": e });
    let c = |ty: &str, params: Vec<Value>| json!({ "btType": "BTMSketchConstraint-2", "constraintType": ty, "parameters": params });
    let pi = std::f64::consts::PI;
    let features = json!({ "features": [
        { "btType": "BTMSketch-151", "featureType": "newSketch", "featureId": "FS", "name": "Sketch 1",
          "parameters": [{ "btType": "BTMParameterQueryList-148", "parameterId": "sketchPlane", "queries": [{ "btType": "BTMIndividualQuery-138", "queryString": "query=dummyQuery(id+\"Top.planeOp\",EntityType.FACE);" }] }],
          "entities": [
              arc("a", [-0.05, 0.0], 0.007, true, [-pi / 2.0, pi / 2.0]),
              line("t", [-0.05, 0.007], [-1.0, 0.0], [0.0, 0.02]),
              line("l", [-0.07, 0.007], [0.0, -1.0], [0.0, 0.014]),
              line("b", [-0.07, -0.007], [1.0, 0.0], [0.0, 0.02]),
              arc("w", [0.02, 0.0], 0.005, false, [pi, 0.0]),
              line("chord", [0.025, 0.0], [-1.0, 0.0], [0.0, 0.01]),
              line("r.top", [0.05, 0.01], [1.0, 0.0], [-0.01, 0.01]),
              line("r.right", [0.06, 0.0], [0.0, -1.0], [-0.01, 0.01]),
              line("r.bottom", [0.05, -0.01], [1.0, 0.0], [-0.01, 0.01]),
              line("r.left", [0.04, 0.0], [0.0, 1.0], [-0.01, 0.01]),
              { "btType": "BTMSketchPoint-158", "entityId": "r.middle", "isConstruction": true, "x": 0.05, "y": 0.0 }
          ],
          "constraints": [
              c("TANGENT", vec![s("localFirst", "a"), s("localSecond", "t")]),
              c("TANGENT", vec![s("localFirst", "a"), s("localSecond", "b")]),
              c("COINCIDENT", vec![s("localFirst", "a.start"), s("localSecond", "t.start")]),
              c("COINCIDENT", vec![s("localFirst", "b.end"), s("localSecond", "a.end")]),
              c("HORIZONTAL", vec![s("localFirst", "t")]),
              c("VERTICAL", vec![s("localFirst", "l")]),
              c("RADIUS", vec![s("localFirst", "a"), q("length", "7 mm")]),
              c("MIDPOINT", vec![s("localMidpoint", "r.middle"), s("localEntity1", "r.top.start"), s("localEntity2", "r.bottom.end")]),
              c("DISTANCE", vec![s("localFirst", "a"), s("localSecond", "l"), s("direction", "MINIMUM"), q("length", "27 mm")])
          ] }
    ] });
    let mut ctx = Ctx::new();
    let path = scratch("constraints-stay.noshape");
    let mut t = Translator::open(&mut ctx, &path, &features, Default::default()).unwrap();
    let step = t.step(0);
    assert!(step.problems.is_empty(), "{:?}", step.problems);
    let g = t.call("sketch_get", json!({ "sketch": step.ours.unwrap() })).unwrap();
    // The wrapping arc's region: a half disc (39.27 mm²) centred below y = 0.
    let half = g["regions"].as_array().unwrap().iter().find(|r| (r["area"].as_f64().unwrap() - 0.5 * pi * 25.0).abs() < 1e-3).unwrap_or_else(|| panic!("{}", g["regions"]));
    assert!(half["center"][1].as_f64().unwrap() < 0.0, "{half}");
}

/// An Extrude saved before Symmetric was a checkbox (endBound SYMMETRIC):
/// the block 20 mm deep about the sketch plane.
#[test]
fn an_old_symmetric_extrude_is_read() {
    let (mut features, mut live, refs) = block_model();
    features["features"].as_array_mut().unwrap().truncate(2);
    for p in features["features"][1]["parameters"].as_array_mut().unwrap() {
        if p["parameterId"] == "endBound" {
            p["value"] = json!("SYMMETRIC");
        }
    }
    live.truncate(2);
    for p in live[1]["parts"].as_array_mut().unwrap() {
        p.as_object_mut().unwrap().remove("area");
        p["bbox"] = json!({ "min": [0.0, 0.0, -0.01], "max": [0.06, 0.04, 0.01] });
    }
    let r = run_model("old-symmetric", &features, &live, &refs);
    assert_eq!((r.matching, r.features), (2, 2), "first difference: {:?}; untranslated: {:?}", r.first, r.untranslated);
}

/// An assembly's sources: a second Part Studio translated into a new tab of
/// the same document (`open_in`); each tab has its own part, the first is
/// untouched.
#[test]
fn a_second_part_studio_goes_into_its_own_tab() {
    let (features, live, refs) = block_model();
    let mut all = refs_from_bodies(&live);
    all.extend(parse_refs(&refs));
    let mut ctx = Ctx::new();
    let path = scratch("two-studios.noshape");
    {
        let mut t = Translator::open(&mut ctx, &path, &features, all.clone()).unwrap();
        for k in 0..t.len() {
            assert!(t.step(k).ours.is_some());
        }
    }
    {
        let mut t = Translator::open_in(&mut ctx, "Bracket", &features, all).unwrap();
        for k in 0..t.len() {
            let s = t.step(k);
            assert!(s.ours.is_some(), "{}: {:?}", s.name, s.problems);
        }
    }
    let run = |ctx: &mut Ctx, tab: &str| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name("query_parts").unwrap(), &json!({ "tab": tab })).unwrap().value;
    for tab in ["Part Studio 1", "Bracket"] {
        let p = run(&mut ctx, tab);
        assert_eq!(p["parts"].as_array().map(Vec::len), Some(1), "{tab}: {p}");
        assert_eq!((p["parts"][0]["faces"].as_u64(), p["parts"][0]["edges"].as_u64()), (Some(8), Some(17)), "{tab}: {p}");
    }
}
