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
}

/// Rebuilds `features` and compares with `live` (live.jsonl lines) after each feature.
fn run_model(name: &str, features: &Value, live: &[Value], refs: &str) -> ModelResult {
    let mut ctx = Ctx::new();
    let path = scratch(&format!("{name}.noshape"));
    let mut res = ModelResult { name: name.to_string(), features: 0, compared: 0, matching: 0, first: None, untranslated: Vec::new() };
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
            diff.push(format!("status {ours}, live {live_status}"));
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
            index.push(json!({ "name": name, "document": s["document"], "workspace": s["workspace"], "element": s["element"], "url": s["url"] }));
        }
        let Some((features, live, refs)) = read_model(&dir) else {
            println!("{name}: unreadable features.json");
            continue;
        };
        let r = run_model(&name, &features, &live, &refs);
        println!("{}: {}/{} features match ({} of {} compared){}", r.name, r.matching, r.compared, r.compared, r.features, r.first.as_ref().map(|f| format!("; first difference: {f}")).unwrap_or_else(|| "; matches to the end".into()));
        for u in &r.untranslated {
            println!("  untranslated: {u}");
        }
        results.push(r);
    }
    let docs = results.iter().filter(|r| r.first.is_none() && r.compared > 0).count();
    let (matching, total) = results.iter().fold((0, 0), |(m, t), r| (m + r.matching, t + r.compared));
    println!("parity_models score: {docs}/{} documents match to the end, {matching}/{total} features match", results.len());
    let report: Vec<Value> = results.iter().map(|r| json!({ "name": r.name, "features": r.features, "compared": r.compared, "matching": r.matching, "first_difference": r.first, "untranslated": r.untranslated })).collect();
    let _ = std::fs::write(root.with_file_name("report.json"), serde_json::to_string_pretty(&json!({ "documents_matching": docs, "documents": results.len(), "features_matching": matching, "features": total, "models": report })).unwrap());
    if std::env::var_os("NOSHAPE_PARITY_INDEX").is_some() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity-models/index.json");
        std::fs::write(file, serde_json::to_string_pretty(&index).unwrap() + "\n").unwrap();
    }
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
    assert!(r.untranslated.iter().any(|u| u.contains("geometric constraints left to inference")), "{:?}", r.untranslated);
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
