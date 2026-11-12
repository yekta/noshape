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
/// region by index), "edge through [20, 0, 10]" (also face / vertex; mm),
/// "Sketch 1 edges" (all its curves: a Frame's path).
fn entity(ctx: &mut Ctx, name: &str) -> Result<Value, String> {
    if let Some(sketch) = name.strip_suffix(" edges") {
        let got = call(ctx, "sketch_get", json!({ "sketch": sketch }))?;
        let ids: Vec<Value> = got["entities"].as_array().into_iter().flatten().filter(|e| !matches!(e["type"].as_str(), Some("point")) && e["construction"] != true).filter_map(|e| e["id"].as_str().map(|id| json!(id))).collect();
        if ids.is_empty() {
            return Err(format!("`{name}`: {sketch} has no curves"));
        }
        return Ok(Value::Array(ids));
    }
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
        Value::Array(items) if items.iter().all(Value::is_string) && items.iter().any(|s| s.as_str().is_some_and(|s| s.contains(" region ") || s.contains(" through ") || s.ends_with(" edges"))) => {
            // ("Sketch 1 edges" stands for several.)
            let mut out = Vec::new();
            for s in items {
                match entity(ctx, s.as_str().unwrap_or_default())? {
                    Value::Array(many) => out.extend(many),
                    one => out.push(one),
                }
            }
            Ok(Value::Array(out))
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
    add_features(ctx, &start["features"])?;
    // The Part Studio's configuration inputs (README "### start"): a List's rows, the first its default.
    for (k, input) in start["configuration"].as_array().into_iter().flatten().enumerate() {
        let what = |e: String| format!("configuration input {k}: {e}");
        if input["type"] != "list" {
            return Err(what("only List inputs".into()));
        }
        let name = input["name"].as_str().ok_or_else(|| what("no name".into()))?;
        call(ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "list", "name": name })).map_err(what)?;
        for (r, row) in input["rows"].as_array().into_iter().flatten().filter_map(Value::as_str).enumerate() {
            if r == 0 {
                call(ctx, "config_edit_option", json!({ "tab": "Part Studio 1", "input": name, "option": "Default", "name": row })).map_err(what)?;
            } else {
                call(ctx, "config_add_option", json!({ "tab": "Part Studio 1", "input": name, "name": row })).map_err(what)?;
            }
        }
    }
    // Live's runner names an assembly after the start ("start pin-block asm").
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("start");
    let asm_tab = format!("start {stem} asm");
    if let Some(asm) = start.get("assembly") {
        build_assembly(ctx, asm)?;
        call(ctx, "tab_rename", json!({ "tab": "Assembly 1", "name": asm_tab }))?;
    }
    // A Feature Studio (README "### start": a custom table's defineTable, a custom feature), committed.
    if let Some(fs) = start.get("feature_studio") {
        let name = fs["name"].as_str().unwrap_or("Feature Studio 1");
        let source = fs["source"].as_str().ok_or("feature_studio: its source")?;
        call(ctx, "tab_create", json!({ "kind": "featurestudio", "name": name }))?;
        call(ctx, "featurestudio_edit", json!({ "tab": name, "source": source }))?;
        call(ctx, "featurestudio_commit", json!({ "tab": name }))?;
    }
    // A whole document's tabs after Part Studio 1 and Assembly 1 (README "Documents, tabs and preferences"): tabs by
    // kind and name, a folder with the tabs in it.
    for t in start["tabs"].as_array().into_iter().flatten() {
        build_tab(ctx, t, None)?;
    }
    // Files (README "### start" `files`): each imported as a file tab named after it, in order; a relative name is in
    // `files_dir` (the runner's tests/ui-parity/files).
    let files_dir = start["files_dir"].as_str().map(std::path::PathBuf::from);
    for f in start["files"].as_array().into_iter().flatten() {
        let name = f.as_str().ok_or_else(|| format!("a start file that isn't a name: {f}"))?;
        let path = files_dir.as_ref().map_or_else(|| std::path::PathBuf::from(name), |d| d.join(name));
        // A CAD file stays a file tab as uploaded (README `files`: no translation; its own Import… translates it).
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if crate::import::IMPORT_EXTS.contains(&ext.as_str()) {
            let bytes = std::fs::read(&path).map_err(|e| format!("start file {name}: {e}"))?;
            let od = ctx.doc(None).map_err(|e| format!("start file {name}: {e}"))?;
            od.session.edit_document(&format!("Import {name}"), |d| crate::import::add_file_tab(d, noshape_model::file_tab::FileTab::new(name, &bytes, noshape_model::file_tab::now()), None, None)).map_err(|e| format!("start file {name}: {e}"))?;
            continue;
        }
        call(ctx, "import_files", json!({ "files": [path] })).map_err(|e| format!("start file {name}: {e}"))?;
    }
    // A linked document (README "Variable Studio and Versions" `linked`): built from its own start beside this one, its
    // tab inserted into this document's assembly at one of its versions (a reference to another document's version).
    if let Some(l) = start.get("linked") {
        let name = l["name"].as_str().ok_or("linked: its name")?;
        let source = path.with_file_name(format!("{name}.noshape"));
        build_start(&mut Ctx::new(), &source, &l["start"]).map_err(|e| format!("linked {name}: {e}"))?;
        let ins = &l["insert"];
        let mut args = json!({ "tab": ins["assembly"].as_str().unwrap_or("Assembly 1"), "source": ins["source"].as_str().unwrap_or("Part Studio 1"), "document": source.to_str().ok_or("path")?, "version": ins["version"].as_str().ok_or("linked: insert's version")? });
        // One part of it (live2's start: "Part 1 <1>"), else the whole Part Studio.
        if let Some(part) = ins["part"].as_str() {
            args["part"] = json!(part);
        }
        call(ctx, "assembly_insert", args).map_err(|e| format!("linked {name}: {e}"))?;
    }
    // (The run opens on Part Studio 1, where the versions' features go too.)
    let has = |key: &str| start[key].as_array().is_some_and(|t| !t.is_empty());
    if has("tabs") || has("files") || start.get("feature_studio").is_some() || start.get("linked").is_some() {
        call(ctx, "tab_activate", json!({ "tab": "Part Studio 1" }))?;
    }
    // `"drawing": {"first": true}`: the drawing before the versions and workspaces, each of which may note it.
    if start["drawing"]["first"].as_bool().unwrap_or(false) {
        build_drawing(ctx, start, &asm_tab)?;
    }
    // Versions (README "Versions and history"): each made after what comes before it, then its own `features` in Main.
    for v in start["versions"].as_array().into_iter().flatten() {
        let name = v["name"].as_str().ok_or_else(|| format!("a start version without a name: {v}"))?;
        call(ctx, "history_create_version", json!({ "name": name })).map_err(|e| format!("start version {name}: {e}"))?;
        add_features(ctx, &v["features"])?;
    }
    // Workspaces (README "Variable Studio and Versions"): each branched from Main's head with its own `features`, the
    // run back on Main.
    let branches = start["branches"].as_array().cloned().unwrap_or_default();
    for b in &branches {
        let name = b["name"].as_str().ok_or_else(|| format!("a start workspace without a name: {b}"))?;
        call(ctx, "history_switch_branch", json!({ "branch": "Main" })).map_err(|e| format!("start workspace {name}: {e}"))?;
        call(ctx, "history_create_branch", json!({ "name": name })).map_err(|e| format!("start workspace {name}: {e}"))?;
        add_features(ctx, &b["features"])?;
        add_drawing_notes(ctx, start, &b["drawing_notes"])?;
    }
    if !branches.is_empty() {
        call(ctx, "history_switch_branch", json!({ "branch": "Main" }))?;
    }
    add_drawing_notes(ctx, start, &start["drawing_notes"])?;
    // A drawing made first: the run opens on the Part Studio, as live's runner opens a whole document.
    if start["drawing"]["first"].as_bool().unwrap_or(false) {
        call(ctx, "tab_activate", json!({ "tab": "Part Studio 1" }))?;
    }
    if !start["drawing"]["first"].as_bool().unwrap_or(false) {
        build_drawing(ctx, start, &asm_tab)?;
    }
    Ok(())
}

/// The start's drawing (README "### Drawings"): Create Drawing of a part, the run opening on its tab ("Part 1 Drawing 1").
fn build_drawing(ctx: &mut Ctx, start: &Value, asm_tab: &str) -> Result<(), String> {
    let Some(d) = start.get("drawing") else { return Ok(()) };
    let template = d["template"].as_str().unwrap_or("ANSI_A_INCH.dwt");
    let suggested = d["views"].as_str() == Some("suggested");
    // A part's drawing, or (`"assembly": true`) the start's assembly's.
    // `"part": null`: Create Drawing of Part Studio 1 (all its parts).
    let made = if d["assembly"] == true {
        call(ctx, "drawing_create", json!({ "source": asm_tab, "template": template, "suggested_views": suggested }))
    } else if d.get("part").is_some_and(Value::is_null) {
        call(ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": template, "suggested_views": suggested }))
    } else {
        call(ctx, "drawing_create", json!({ "source": "Part Studio 1", "part": d["part"].as_str().unwrap_or("Part 1"), "template": template, "suggested_views": suggested }))
    };
    made.map_err(|e| format!("start drawing: {e}"))?;
    Ok(())
}

/// The start drawing's name, as `build_drawing` makes it.
fn drawing_name(start: &Value) -> String {
    let d = &start["drawing"];
    if d.get("part").is_some_and(Value::is_null) { "Part Studio 1 Drawing 1".to_string() } else { format!("{} Drawing 1", d["part"].as_str().unwrap_or("Part 1")) }
}

/// `"drawing_notes": [{"text", "position": [x, y]}]` (sheet mm) on the start's drawing, in the workspace the run is in.
fn add_drawing_notes(ctx: &mut Ctx, start: &Value, notes: &Value) -> Result<(), String> {
    for n in notes.as_array().into_iter().flatten() {
        call(ctx, "drawing_note", json!({ "drawing": drawing_name(start), "text": n["text"], "position": n["position"] })).map_err(|e| format!("start drawing note: {e}"))?;
    }
    Ok(())
}

/// A start's tab (`{"kind": "partstudio" | "assembly" | "variablestudio" | "featurestudio" | "folder", "name"}`, a
/// folder's own `tabs` inside it), at the end of the tab bar or of `folder`.
fn build_tab(ctx: &mut Ctx, t: &Value, folder: Option<&str>) -> Result<(), String> {
    let name = t["name"].as_str().ok_or_else(|| format!("a start tab without a name: {t}"))?;
    let what = |e: String| format!("start tab {name}: {e}");
    match t["kind"].as_str().unwrap_or("partstudio") {
        "folder" => {
            let mut args = json!({ "name": name });
            if let Some(f) = folder {
                args["folder"] = json!(f);
            }
            call(ctx, "tab_folder_create", args).map_err(what)?;
            for inner in t["tabs"].as_array().into_iter().flatten() {
                build_tab(ctx, inner, Some(name))?;
            }
        }
        kind => {
            call(ctx, "tab_create", json!({ "kind": kind, "name": name })).map_err(what)?;
            // A Variable Studio's variables: `{"name", "value", "kind"?, "description"?}`.
            for v in t["variables"].as_array().into_iter().flatten() {
                let mut args = v.clone();
                args["tab"] = json!(name);
                call(ctx, "variable_create", args).map_err(what)?;
            }
            if let Some(f) = folder {
                call(ctx, "tab_move", json!({ "tab": name, "folder": f })).map_err(what)?;
            }
        }
    }
    Ok(())
}

/// A start's features (README "### start"), in Part Studio 1 at the end of its list.
fn add_features(ctx: &mut Ctx, features: &Value) -> Result<(), String> {
    for (k, f) in features.as_array().into_iter().flatten().enumerate() {
        let what = |e: String| format!("feature {k}: {e}");
        if f["op"] == "reorder" {
            let to = feature_index(ctx, f["before"].as_str().unwrap_or_default()).map_err(what)?;
            call(ctx, "feature_reorder", json!({ "features": [f["feature"]], "to": to })).map_err(what)?;
            continue;
        }
        let ty = f["type"].as_str().ok_or_else(|| what("no type".into()))?;
        // Named as it is made: a default name stays a default one, as in live's start documents (a paste's next name).
        if ty == "newSketch" {
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
        } else {
            let mut params = serde_json::Map::new();
            for (p, v) in f["params"].as_object().into_iter().flatten() {
                params.insert(p.clone(), resolve(ctx, v).map_err(what)?);
            }
            let mut args = json!({ "type": ty, "params": params });
            if let Some(n) = f["name"].as_str() {
                args["name"] = json!(n);
            }
            call(ctx, "feature_add", args).map_err(what)?;
        }
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
        // As live's REST builder makes it: no primaryAxisAlignment (spec default true: flipped, plain flip icon, live
        // 2423) unless `flip` (false), and a REST connector whose Z points the other way (live 2412: pin-block's pin
        // stays upright, the second connector's matedCS Z (0, 0, -1) with the mate flipped).
        let mut b = side(ctx, &m["b"]).map_err(what)?;
        b["flip"] = json!(true);
        let mut args = json!({ "type": m["type"].as_str().unwrap_or("fastened").to_lowercase(), "a": side(ctx, &m["a"]).map_err(what)?, "b": b, "flip_primary": m["flip"].as_bool() != Some(true) });
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
            { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "20 mm", "operationType": "NEW" } },
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
        // Live's REST-made Revolute 1: Flip primary axis on (its plain icon), the pin upright.
        assert_eq!(list["mates"][0]["flip_primary"], true, "{list}");
        let r: Vec<f64> = pin["rotation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert!(r[3].abs() > 1.0 - 1e-9, "{pin}");
        // "Sketch 1 region 0" is the square with the hole (live's block; our first region), not the hole's disc.
        let mp = call(&mut ctx, "query_mass_properties", json!({ "tab": "Part Studio 1", "parts": ["Part 1"] })).unwrap();
        let v = 40.0 * 40.0 * 20.0 - std::f64::consts::PI * 25.0 * 20.0;
        assert!((mp["volume"].as_f64().unwrap() - v).abs() < 1.0, "{mp}");
        call(&mut ctx, "tab_activate", json!({ "tab": "start pin asm" })).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A whole document's start (README "Documents, tabs and preferences"): Part Studio 1 and Assembly 1, then the
    /// start's tabs in order, a folder holding its own.
    #[test]
    fn a_whole_document_start_builds() {
        let start = json!({ "units": "mm", "features": [], "tabs": [
            { "kind": "partstudio", "name": "Part Studio 2" },
            { "kind": "variablestudio", "name": "Variable Studio 1" },
            { "kind": "featurestudio", "name": "Feature Studio 1" },
            { "kind": "folder", "name": "Folder 1", "tabs": [{ "kind": "partstudio", "name": "Part Studio 3" }] }
        ] });
        let dir = std::env::temp_dir().join(format!("noshape-uip-tabs-{}", std::process::id()));
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &dir.join("tabs.noshape"), &start).unwrap();
        let info = call(&mut ctx, "document_info", json!({})).unwrap();
        let tab = |name: &str| info["tabs"].as_array().unwrap().iter().find(|t| t["name"] == name).cloned().unwrap_or_else(|| panic!("no {name} in {info}"));
        let names: Vec<&str> = info["tabs"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
        let at = |n: &str| names.iter().position(|x| *x == n).unwrap_or_else(|| panic!("no {n} in {names:?}"));
        assert!(at("Part Studio 1") < at("Assembly 1") && at("Assembly 1") < at("Part Studio 2") && at("Part Studio 2") < at("Variable Studio 1") && at("Variable Studio 1") < at("Feature Studio 1"), "{names:?}");
        assert_eq!(tab("Variable Studio 1")["kind"], "Variable Studio", "{info}");
        assert_eq!(tab("Part Studio 3")["folder"], "Folder 1", "{info}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A start's workspaces (`branches`): doc-branches' B1 is Main's head and its own hole, the run on Main.
    #[test]
    fn a_start_with_a_workspace_builds() {
        let starts: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui-parity/starts.json")).unwrap()).unwrap();
        let dir = std::env::temp_dir().join(format!("noshape-uip-branches-{}", std::process::id()));
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &dir.join("branches.noshape"), &starts["doc-branches"]).unwrap();
        let names = |ctx: &mut Ctx| -> Vec<String> { call(ctx, "feature_list", json!({})).unwrap()["features"].as_array().unwrap().iter().filter_map(|f| f["name"].as_str().map(str::to_string)).collect() };
        let main = names(&mut ctx);
        assert!(main.iter().any(|n| n == "Extrude 2") && !main.iter().any(|n| n == "Extrude 3"), "Main: {main:?}");
        call(&mut ctx, "history_switch_branch", json!({ "branch": "B1" })).unwrap();
        let b1 = names(&mut ctx);
        assert_eq!(b1.last().map(String::as_str), Some("Extrude 3"), "B1: {b1:?}");
        assert_eq!(b1[..b1.len() - 2], main[..], "B1 from Main's head: {b1:?}");
        // Its hole goes through Part 1, as live's builder makes it (the direction given: std's edit logic flips a Remove on
        // a plane away from the part behind it, ver-switch-workspace-edit's Extrude 3 then cut nothing and had no
        // appearance item).
        let volume = call(&mut ctx, "query_mass_properties", json!({ "parts": ["Part 1"] })).unwrap()["volume"].as_f64().unwrap();
        assert!((volume - (12000.0 - std::f64::consts::PI * 9.0 * 10.0)).abs() < 1.0, "B1's Part 1 with its Ø6 hole: {volume} mm³");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A start's Feature Studio (README "### start"): block-drawing-table's custom table goes on its drawing.
    #[test]
    fn a_feature_studio_start_builds() {
        let starts: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui-parity/starts.json")).unwrap()).unwrap();
        let dir = std::env::temp_dir().join(format!("noshape-uip-fs-{}", std::process::id()));
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &dir.join("table.noshape"), &starts["block-drawing-table"]).unwrap();
        let r = call(&mut ctx, "drawing_custom_table", json!({ "drawing": "Part 1 Drawing 1", "table": "Part faces", "position": [150, 150] }));
        assert!(r.is_ok(), "{r:?}");
        // As the app opens it: from the saved file.
        let mut again = Ctx::new();
        call(&mut again, "document_open", json!({ "path": dir.join("table.noshape").to_str().unwrap() })).unwrap();
        let r = call(&mut again, "drawing_custom_table", json!({ "drawing": "Part 1 Drawing 1", "table": "Part faces", "position": [150, 150] }));
        assert!(r.is_ok(), "reopened: {r:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A frame start: Frame 1 along Sketch 1's two lines, a cut list, and a drawing of the whole Part Studio whose
    /// Cut list table lists both segments.
    #[test]
    fn a_frame_start_builds() {
        let starts: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui-parity/starts.json")).unwrap()).unwrap();
        let dir = std::env::temp_dir().join(format!("noshape-uip-frame-{}", std::process::id()));
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &dir.join("frame.noshape"), &starts["frame-drawing"]).unwrap();
        let fs = call(&mut ctx, "feature_list", json!({ "tab": "Part Studio 1" })).unwrap();
        let bad: Vec<&Value> = fs["features"].as_array().unwrap().iter().filter(|f| f["status"].as_str().is_some_and(|s| s != "Ok")).collect();
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(fs["features"].as_array().unwrap().iter().filter(|f| f["status"] == "Ok").count(), 3, "{fs}");
        let d = call(&mut ctx, "drawing_get", json!({ "drawing": "Part Studio 1 Drawing 1", "edges": false })).unwrap();
        let view = d["views"][0]["id"].clone();
        let r = call(&mut ctx, "drawing_view_table", json!({ "drawing": "Part Studio 1 Drawing 1", "kind": "cut_list", "view": view, "position": [150, 150] }));
        assert!(r.is_ok(), "{r:?}");
        // Insert › Select a composite part (live2 2278): the Cut list feature's open composite, "Composite part 1" (2299).
        let r = call(&mut ctx, "drawing_view_table", json!({ "drawing": "Part Studio 1 Drawing 1", "kind": "cut_list", "composite": { "studio": "Part Studio 1", "part": "Composite part 1" }, "position": [150, 250] }));
        assert!(r.is_ok(), "{r:?}");
        let d = call(&mut ctx, "drawing_get", json!({ "drawing": "Part Studio 1 Drawing 1", "edges": false })).unwrap();
        let t = d["tables"].as_array().unwrap().iter().find(|t| t["composite"] == "Composite part 1").unwrap_or_else(|| panic!("{}", d["tables"]));
        assert_eq!(t["rows"].as_array().map(|r| r.len()), d["tables"][0]["rows"].as_array().map(|r| r.len()), "the same frames as the view's: {t}");
        let e = call(&mut ctx, "drawing_view_table", json!({ "drawing": "Part Studio 1 Drawing 1", "kind": "cut_list", "composite": { "studio": "Part Studio 1", "part": "Part 1" }, "position": [150, 250] })).unwrap_err();
        assert!(e.contains("is not a composite part"), "{e}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A start's files (README "### start"): block-drawing-files' image and DXF are file tabs its drawing inserts.
    #[test]
    fn a_files_start_builds() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui-parity");
        let starts: Value = serde_json::from_str(&std::fs::read_to_string(format!("{root}/starts.json")).unwrap()).unwrap();
        let mut start = starts["block-drawing-files"].clone();
        start["files_dir"] = json!(format!("{root}/files"));
        let dir = std::env::temp_dir().join(format!("noshape-uip-files-{}", std::process::id()));
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &dir.join("files.noshape"), &start).unwrap();
        let r = call(&mut ctx, "drawing_insert_image", json!({ "drawing": "Part 1 Drawing 1", "file": "dx-image.png", "position": [40, 40], "to": [88, 64] }));
        assert!(r.is_ok(), "{r:?}");
        let r = call(&mut ctx, "drawing_insert_cad", json!({ "drawing": "Part 1 Drawing 1", "file": "dx-plate.dxf" })).unwrap();
        assert_eq!(r["entities"], 5, "{r}");
        let _ = std::fs::remove_dir_all(&dir);
        // doc-files: its five file tabs (CSV, JSON, TXT, PDF and a PNG), the run on Part Studio 1.
        let mut start = starts["doc-files"].clone();
        start["files_dir"] = json!(format!("{root}/files"));
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &dir.join("doc-files.noshape"), &start).unwrap();
        let info = call(&mut ctx, "document_info", json!({})).unwrap();
        let tabs: Vec<&str> = info["tabs"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
        for f in ["dx-image.png", "uip-table.csv", "uip-data.json", "uip-notes.txt", "uip-page.pdf"] {
            assert!(tabs.contains(&f), "{f} in {tabs:?}");
        }
        let ui = call(&mut ctx, "ui_state", json!({})).unwrap();
        assert_eq!(ui["active_tab"]["name"], "Part Studio 1", "{}", ui["active_tab"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A linked start (doc-linked): "uip Source" built beside it, its Part 1 in Assembly 1 at V1 (live2's "Part 1 <1>").
    #[test]
    fn a_linked_start_builds() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui-parity");
        let starts: Value = serde_json::from_str(&std::fs::read_to_string(format!("{root}/starts.json")).unwrap()).unwrap();
        let dir = std::env::temp_dir().join(format!("noshape-uip-linked-{}", std::process::id()));
        let mut ctx = Ctx::new();
        build_start(&mut ctx, &dir.join("linked.noshape"), &starts["doc-linked"]).unwrap();
        assert!(dir.join("uip Source.noshape").exists());
        let list = call(&mut ctx, "assembly_list", json!({ "tab": "Assembly 1" })).unwrap();
        assert_eq!(list["instances"].as_array().map(Vec::len), Some(1), "{list}");
        assert_eq!(list["instances"][0]["name"], "Part 1 <1>", "{list}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
