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
}
