//! The interaction parity suite's start documents (tests/ui-parity/README.md):
//! a `starts.json` entry (a feature list in FeatureScript terms) built into a
//! new document through the ops registry, as live builds it over REST. A test
//! tool, not a user feature.

use crate::{Ctx, registry};
use serde_json::{Value, json};
use std::path::Path;

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Result<Value, String> {
    let op = registry::by_tool_name(tool).ok_or_else(|| format!("no tool {tool}"))?;
    registry::run(ctx, op, &args).map(|o| o.value).map_err(|e| e.to_json().to_string())
}

/// A selection as the start names it: "Sketch 1 region 0" (the sketch's
/// region by index), "edge through [20, 0, 10]" (also face / vertex; mm).
fn entity(ctx: &mut Ctx, name: &str) -> Result<Value, String> {
    if let Some((kind, at)) = name.split_once(" through ") {
        let at: Value = serde_json::from_str(at).map_err(|e| format!("`{name}`: {e}"))?;
        let kind = match kind {
            "edge" => "edges",
            "face" => "faces",
            "vertex" => "vertices",
            _ => return Err(format!("`{name}`: not an edge, face or vertex")),
        };
        return Ok(json!({ "query": { "kind": kind, "at": [at] } }));
    }
    let (sketch, index) = name.rsplit_once(" region ").ok_or_else(|| format!("`{name}`: not \"<sketch> region <n>\" or \"<kind> through [x, y, z]\""))?;
    let index: usize = index.parse().map_err(|_| format!("`{name}`: the region index"))?;
    let got = call(ctx, "sketch_get", json!({ "sketch": sketch }))?;
    got["regions"].get(index).and_then(|r| r["id"].as_str()).map(|id| json!(id)).ok_or_else(|| format!("`{name}`: {sketch} has {} regions", got["regions"].as_array().map_or(0, Vec::len)))
}

/// Selections named in a feature's params, resolved to ours.
fn resolve(ctx: &mut Ctx, v: &Value) -> Result<Value, String> {
    match v {
        Value::Array(items) if items.iter().all(Value::is_string) && items.iter().any(|s| s.as_str().is_some_and(|s| s.contains(" region ") || s.contains(" through "))) => {
            items.iter().map(|s| entity(ctx, s.as_str().unwrap_or_default())).collect::<Result<Vec<_>, _>>().map(Value::Array)
        }
        _ => Ok(v.clone()),
    }
}

fn feature_index(ctx: &mut Ctx, name: &str) -> Result<usize, String> {
    let list = call(ctx, "feature_list", json!({}))?;
    list["features"].as_array().and_then(|fs| fs.iter().position(|f| f["name"] == name)).ok_or_else(|| format!("no feature {name}"))
}

/// Build start `start` (a `starts.json` value) into a new document at `path`.
pub fn build_start(ctx: &mut Ctx, path: &Path, start: &Value) -> Result<(), String> {
    let _ = std::fs::remove_file(path);
    call(ctx, "document_create", json!({ "path": path.to_str().ok_or("path")?, "length_unit": start["units"].as_str().unwrap_or("mm") }))?;
    for (k, f) in start["features"].as_array().into_iter().flatten().enumerate() {
        let what = |e: String| format!("feature {k}: {e}");
        if f["op"] == "reorder" {
            let to = feature_index(ctx, f["before"].as_str().unwrap_or_default()).map_err(what)?;
            call(ctx, "feature_reorder", json!({ "features": [f["feature"]], "to": to })).map_err(what)?;
            continue;
        }
        let ty = f["type"].as_str().ok_or_else(|| what("no type".into()))?;
        let made = if ty == "newSketch" {
            let plane = format!("plane:{}", f["plane"].as_str().unwrap_or("Top").to_lowercase());
            let mut args = json!({ "plane": plane });
            if let Some(n) = f["name"].as_str() {
                args["name"] = json!(n);
            }
            let made = call(ctx, "sketch_create", args).map_err(what)?;
            let id = made["sketch"].clone();
            let shapes: Vec<Value> = f["shapes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_object)
                .flat_map(|m| m.iter().map(|(tool, points)| json!({ "tool": tool, "points": points })))
                .collect();
            if !shapes.is_empty() {
                call(ctx, "sketch_draw", json!({ "sketch": id, "shapes": shapes })).map_err(what)?;
            }
            id
        } else {
            let mut params = serde_json::Map::new();
            for (p, v) in f["params"].as_object().into_iter().flatten() {
                params.insert(p.clone(), resolve(ctx, v).map_err(what)?);
            }
            call(ctx, "feature_add", json!({ "type": ty, "params": params })).map_err(what)?["feature"].clone()
        };
        if let Some(name) = f["name"].as_str() {
            call(ctx, "feature_rename", json!({ "feature": made, "name": name })).map_err(what)?;
        }
    }
    if let Some(asm) = start.get("assembly") {
        build_assembly(ctx, asm)?;
        // Live's runner names it after the start ("start pin-block asm").
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("start");
        call(ctx, "tab_rename", json!({ "tab": "Assembly 1", "name": format!("start {stem} asm") }))?;
    }
    // A drawing of a part (README "### Drawings"): Create Drawing of it, the run opening on its tab ("Part 1 Drawing 1").
    if let Some(d) = start.get("drawing") {
        let part = d["part"].as_str().unwrap_or("Part 1");
        let made = call(ctx, "drawing_create", json!({ "source": "Part Studio 1", "part": part, "template": d["template"].as_str().unwrap_or("ANSI_A_INCH.dwt"), "suggested_views": d["views"].as_str() == Some("suggested") }));
        made.map_err(|e| format!("start drawing: {e}"))?;
    }
    Ok(())
}

/// A start's `assembly` (after the Part Studio's features): its instances
/// (`part`, optional `fixed` and `at` [x, y, z] in the document's unit) and
/// its mates (`type`, `name`, sides `a` / `b` as an instance, a point on the
/// face, and an inference).
fn build_assembly(ctx: &mut Ctx, asm: &Value) -> Result<(), String> {
    for (k, i) in asm["instances"].as_array().into_iter().flatten().enumerate() {
        let what = |e: String| format!("assembly instance {k}: {e}");
        let part = i["part"].as_str().ok_or_else(|| what("no part".into()))?;
        let made = call(ctx, "assembly_insert", json!({ "source": i["studio"].as_str().unwrap_or("Part Studio 1"), "part": part })).map_err(what)?;
        let name = made["instances"].get(0).and_then(|x| x["name"].as_str()).or_else(|| made["name"].as_str()).map(str::to_string).ok_or_else(|| what(format!("no instance in {made}")))?;
        if let Some(at) = i.get("at") {
            call(ctx, "assembly_transform", json!({ "instance": name, "translation": at })).map_err(what)?;
        }
        if i["fixed"].as_bool() == Some(true) {
            call(ctx, "assembly_set_state", json!({ "instances": [name], "fixed": true })).map_err(what)?;
        }
    }
    for (k, m) in asm["mates"].as_array().into_iter().flatten().enumerate() {
        let what = |e: String| format!("assembly mate {k}: {e}");
        let side = |ctx: &mut Ctx, s: &Value| -> Result<Value, String> {
            let instance = s["instance"].as_str().ok_or("a side without an instance")?;
            let faces = call(ctx, "assembly_entities", json!({ "instance": instance, "kind": "faces", "at": [s["face"].clone()] }))?;
            let id = faces["faces"].get(0).and_then(|f| f["id"].as_str()).ok_or_else(|| format!("no face of {instance} at {}", s["face"]))?;
            Ok(json!({ "instance": instance, "entity": id, "inference": s["inference"].as_str().unwrap_or("center") }))
        };
        let mut args = json!({ "type": m["type"].as_str().unwrap_or("fastened").to_lowercase(), "a": side(ctx, &m["a"]).map_err(what)?, "b": side(ctx, &m["b"]).map_err(what)? });
        if let Some(n) = m["name"].as_str() {
            args["name"] = json!(n);
        }
        call(ctx, "assembly_mate", args).map_err(what)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// qa's "vars" start: two variables and a 40 × 30 × 10 box, every feature regenerated.
    #[test]
    fn the_vars_start_builds() {
        let start = json!({ "units": "mm", "features": [
            { "type": "assignVariable", "params": { "mode": "ASSIGNED", "variableType": "LENGTH", "name": "wall_thickness", "lengthValue": "2 mm" } },
            { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
            { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } },
            { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [20, 0, 10]"], "radius": "2 mm" } },
            { "op": "reorder", "feature": "Fillet 1", "before": "Extrude 1" }
        ] });
        let dir = std::env::temp_dir().join(format!("noshape-uip-{}", std::process::id()));
        let path = dir.join("vars.noshape");
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &path, &start).unwrap();
        let list = call(&mut ctx, "feature_list", json!({})).unwrap();
        let rows: Vec<(String, String)> = list["features"].as_array().unwrap().iter().map(|f| (f["name"].as_str().unwrap_or("").to_string(), f["status"].as_str().unwrap_or("").to_string())).collect();
        let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(names, ["Variable 1", "Sketch 1", "Fillet 1", "Extrude 1"], "{rows:?}");
        assert!(rows.iter().filter(|r| r.0 != "Fillet 1").all(|r| r.1 == "Ok"), "{rows:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A start with an assembly (live 1907's pin in a block): the block fixed,
    /// the pin Revolute'd into its hole on mid-axis connectors.
    #[test]
    fn an_assembly_start_builds() {
        let start = json!({ "units": "mm", "features": [
            { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[-20, -20], [20, 20]] }, { "circle": [[0, 0], [5, 0]] }] },
            { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 1"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
            { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[60, 0], [65, 0]] }] },
            { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "30 mm", "operationType": "NEW" } }
        ], "assembly": {
            "instances": [{ "part": "Part 1", "fixed": true }, { "part": "Part 2" }],
            "mates": [{ "type": "Revolute", "name": "Revolute 1", "a": { "instance": "Part 2 <1>", "face": [65, 0, 15], "inference": "mid_axis" }, "b": { "instance": "Part 1 <1>", "face": [5, 0, 10], "inference": "mid_axis" } }]
        } });
        let dir = std::env::temp_dir().join(format!("noshape-uip-asm-{}", std::process::id()));
        let path = dir.join("pin.noshape");
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &path, &start).unwrap();
        let list = call(&mut ctx, "assembly_list", json!({})).unwrap();
        let pin = list["instances"].as_array().unwrap().iter().find(|i| i["name"] == "Part 2 <1>").unwrap().clone();
        let t: Vec<f64> = pin["translation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert!((t[0] + 60.0).abs() < 1e-6 && t[1].abs() < 1e-6 && (t[2] + 5.0).abs() < 1e-6, "{pin}");
        assert_eq!(list["mates"][0]["name"], "Revolute 1", "{list}");
        call(&mut ctx, "tab_activate", json!({ "tab": "start pin asm" })).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
