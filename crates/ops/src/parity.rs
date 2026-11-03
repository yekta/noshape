//! The model parity suite's translator (docs/PARITY-SUITES.md §1): an
//! Onshape Part Studio's feature list (`GET …/features` JSON) rebuilt in a
//! noshape document through the ops registry, the CLI's path, one feature at
//! a time. A test tool, not a user feature. What it can't translate it
//! reports per feature with the reason.
//!
//! Queries name Onshape's own entities (`deterministicIds`), which mean
//! nothing here: each is matched by geometry, from the bodydetails live.jsonl
//! carries or `refs.jsonl`, else read from the query itself: default planes
//! and the Origin, a sketch's regions and entities, and Onshape's names for
//! what an Extrude or Revolve made (packed `qCompressed` queries decoded with
//! the FeatureScript host's unpacker). Units follow the document's.

use crate::{Ctx, registry};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::Path;

/// One referenced entity as live evaluated it (`refs.jsonl`), SI units.
#[derive(Clone, Debug, Default)]
pub struct LiveRef {
    pub kind: String,
    pub point: Option<[f64; 3]>,
    pub normal: Option<[f64; 3]>,
    pub direction: Option<[f64; 3]>,
    pub measure: Option<f64>,
    /// Surface or curve type in our query_entities words ("plane", "line"…).
    pub geometry: Option<String>,
}

/// What became of one Onshape feature.
#[derive(Clone, Debug, Default)]
pub struct Step {
    pub index: usize,
    pub name: String,
    pub feature_type: String,
    /// Our feature id, when one was made.
    pub ours: Option<String>,
    /// Our regeneration status of it ("Ok", "Error", …) and its message.
    pub status: Option<String>,
    pub message: Option<String>,
    /// Things not translated, each with its reason.
    pub problems: Vec<String>,
}

pub struct Translator<'a> {
    ctx: &'a mut Ctx,
    features: Vec<Value>,
    refs: HashMap<(usize, String), LiveRef>,
    /// Onshape feature id → ours.
    feature_ids: HashMap<String, String>,
    /// (Onshape sketch feature id, sketch entity id) → our entity id
    /// (points as "<id>.start" / ".end" / ".center").
    sketch_entities: HashMap<(String, String), String>,
    /// Onshape sketch frames by feature id (`sketches` from `GET …/sketches`):
    /// origin, x, y in metres.
    frames: HashMap<String, [[f64; 3]; 3]>,
    /// Free sketch points (Onshape sketch id, ours), in drawing order.
    sketch_points: Vec<(String, String)>,
    /// Onshape sketch curves by (sketch feature id, entity id), sampled as
    /// sketch-plane segments (metres), for picking regions.
    curves: HashMap<(String, String), Vec<[f64; 2]>>,
    /// Configuration inputs' default options (`configuration` of features.json).
    config_defaults: HashMap<String, String>,
    /// Edits the current sketch's constraints and dimensions made (to undo).
    sketch_edits: usize,
    /// The sketch before its constraints, and the first that moved it.
    sketch_before: Value,
    /// The open sketch's Onshape (x, y) → ours.
    sketch_map: SketchMap,
    /// Onshape's circles and arcs per (sketch, entity): centre and radius (m).
    circles: HashMap<(String, String), ([f64; 2], f64)>,
    /// Onshape's ellipses: centre, major direction, radii (m).
    ellipses: HashMap<(String, String), [f64; 6]>,
    /// Face pieces one selection list already took (each is picked once).
    picked: std::collections::HashSet<String>,
    first_move: Option<String>,
    /// Onshape Feature Studio element id → our Feature Studio tab id.
    studios: HashMap<String, String>,
    /// What isn't translated of the document itself (configuration inputs…).
    pub notes: Vec<String>,
}

fn f3(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

/// A point (m) as ops take it, with its unit: right in any workspace units.
fn mm(p: [f64; 3]) -> Value {
    json!(p.map(|c| format!("{} mm", c * 1000.0)))
}

/// Metres per unit of an ops reply's `units.length` ("mm", "in", …).
fn per_unit(units: &Value) -> f64 {
    match units["length"].as_str().unwrap_or("mm") {
        "m" => 1.0,
        "cm" => 1e-2,
        "in" => 0.0254,
        "ft" => 0.3048,
        "yd" => 0.9144,
        "um" => 1e-6,
        _ => 1e-3,
    }
}

/// The document's units from features.json's `units` (unit info, either
/// `{"length": "millimeter"}` or REST's `defaultUnits` list), in set_units' words.
fn doc_units(u: &Value) -> (Option<&'static str>, Option<&'static str>) {
    let mut length = u["length"].as_str();
    let mut angle = u["angle"].as_str();
    for d in u["defaultUnits"].as_array().into_iter().flatten() {
        match d["key"].as_str().map(str::to_uppercase).as_deref() {
            Some("LENGTH") => length = d["value"].as_str(),
            Some("ANGLE") => angle = d["value"].as_str(),
            _ => {}
        }
    }
    let l = length.and_then(|l| match l.to_lowercase().as_str() {
        "millimeter" | "mm" => Some("mm"),
        "centimeter" | "cm" => Some("cm"),
        "meter" | "m" => Some("m"),
        "inch" | "in" => Some("in"),
        "foot" | "ft" => Some("ft"),
        "yard" | "yd" => Some("yd"),
        "micrometer" | "um" => Some("um"),
        _ => None,
    });
    let a = angle.and_then(|a| match a.to_lowercase().as_str() {
        "degree" | "deg" => Some("deg"),
        "radian" | "rad" => Some("rad"),
        _ => None,
    });
    (l, a)
}

/// Parses `refs.jsonl` (one referenced id per line, see the module docs).
pub fn parse_refs(text: &str) -> HashMap<(usize, String), LiveRef> {
    let mut out = HashMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let (Some(k), Some(id)) = (v["index"].as_u64(), v["id"].as_str()) else { continue };
        let r = LiveRef { kind: v["kind"].as_str().unwrap_or("").to_string(), point: f3(&v["point"]), normal: f3(&v["normal"]), direction: f3(&v["direction"]), measure: v["measure"].as_f64(), geometry: our_type(v["surfaceType"].as_str().or(v["curveType"].as_str())) };
        out.insert((k as usize, id.to_string()), r);
    }
    out
}

impl<'a> Translator<'a> {
    /// A new document at `path` for the Part Studio `doc` (features.json),
    /// with live's reference geometry `refs`.
    pub fn open(ctx: &'a mut Ctx, path: &Path, doc: &Value, refs: HashMap<(usize, String), LiveRef>) -> Result<Self, String> {
        let features = doc["features"].as_array().ok_or("features.json has no `features`")?.clone();
        let mut frames = HashMap::new();
        for s in doc["sketches"]["sketches"].as_array().into_iter().flatten() {
            // `sketchMatrix` (older API versions) or `transformMatrix` (live's fetch).
            let (Some(id), Some(m)) = (s["featureId"].as_str(), s["sketchMatrix"].as_array().or(s["transformMatrix"].as_array())) else { continue };
            let m: Vec<f64> = m.iter().filter_map(Value::as_f64).collect();
            if m.len() == 16 {
                // Row-major 4×4: columns x, y, normal, origin.
                frames.insert(id.to_string(), [[m[3], m[7], m[11]], [m[0], m[4], m[8]], [m[1], m[5], m[9]]]);
            }
        }
        let config_defaults = doc["configuration"]["configurationParameters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| Some((c["parameterId"].as_str()?.to_string(), c["defaultValue"].as_str()?.to_string())))
            .collect();
        let _ = std::fs::remove_file(path);
        let mut t = Self { ctx, features, refs, feature_ids: HashMap::new(), sketch_entities: HashMap::new(), sketch_points: Vec::new(), frames, curves: HashMap::new(), config_defaults, sketch_edits: 0, sketch_before: Value::Null, sketch_map: SketchMap::default(), circles: HashMap::new(), ellipses: HashMap::new(), picked: Default::default(), first_move: None, studios: HashMap::new(), notes: Vec::new() };
        t.call("document_create", json!({ "path": path.to_str().ok_or("path")? }))?;
        // Bare numbers in Onshape's expressions are in the document's units.
        let (length, angle) = doc_units(&doc["units"]);
        if length.is_some() || angle.is_some() {
            let mut a = json!({});
            if let Some(l) = length {
                a["length_unit"] = json!(l);
            }
            if let Some(g) = angle {
                a["angle_unit"] = json!(g);
            }
            t.call("document_set_units", a)?;
        }
        for c in doc["configuration"]["configurationParameters"].as_array().into_iter().flatten() {
            t.config_input(c);
        }
        for table in doc["variables"].as_array().into_iter().flatten().filter(|v| v["variableStudioReference"].is_object()) {
            t.variable_studio(table);
        }
        for (element, studio) in doc["featureStudios"].as_object().into_iter().flatten() {
            t.feature_studio(element, studio);
        }
        Ok(t)
    }

    /// A Feature Studio custom features come from (features.json
    /// `featureStudios`: element id → `{ "name", "source" }`): ours with the
    /// same FeatureScript, committed.
    fn feature_studio(&mut self, element: &str, studio: &Value) {
        let Some(source) = studio["source"].as_str().or(studio.as_str()) else { return };
        let name = studio["name"].as_str().unwrap_or("Feature Studio 1");
        let made = self.call("tab_create", json!({ "kind": "featurestudio", "name": name })).and_then(|r| r["tab"]["id"].as_str().map(str::to_string).ok_or_else(|| "no tab id".to_string()));
        let tab = match made {
            Ok(t) => t,
            Err(e) => {
                self.notes.push(format!("Feature Studio {name}: {e}"));
                return;
            }
        };
        if let Err(e) = self.call("featurestudio_edit", json!({ "tab": tab, "source": source })).and_then(|_| self.call("featurestudio_commit", json!({ "tab": tab }))) {
            self.notes.push(format!("Feature Studio {name}: {e}"));
        }
        self.studios.insert(element.to_string(), tab);
    }

    /// A Variable Studio the Part Studio's variables come from (live's
    /// `variables`, a table with a `variableStudioReference`): ours with the
    /// same variables, inserted into the Part Studio.
    fn variable_studio(&mut self, table: &Value) {
        let made = match self.call("variable_create_studio", json!({})) {
            Ok(r) => r,
            Err(e) => {
                self.notes.push(format!("Variable Studio: {e}"));
                return;
            }
        };
        let Some(studio) = made["created"]["id"].as_str().map(str::to_string) else { return };
        for v in table["variables"].as_array().into_iter().flatten() {
            let (Some(name), Some(value)) = (v["name"].as_str(), v["expression"].as_str().or(v["value"].as_str())) else { continue };
            let mut args = json!({ "tab": studio, "name": name.trim_start_matches('#'), "value": value });
            if let Some(kind) = v["type"].as_str() {
                args["kind"] = json!(kind);
            }
            if let Err(e) = self.call("variable_create", args) {
                self.notes.push(format!("Variable Studio variable {name}: {e}"));
            }
        }
    }

    /// A configuration input. Variables (#parameterId) as Variable inputs
    /// with their default and range; Lists and Checkboxes only through the
    /// default option their configured parameters take.
    fn config_input(&mut self, c: &Value) {
        let id = c["parameterId"].as_str().unwrap_or("");
        let ty = c["btType"].as_str().unwrap_or("");
        if !ty.starts_with("BTMConfigurationParameterQuantity") {
            let kind = ty.trim_start_matches("BTMConfigurationParameter").split('-').next().unwrap_or(ty);
            self.notes.push(format!("configuration input {id} ({kind}): only its default is built"));
            return;
        }
        let r = &c["rangeAndDefault"];
        let unit = match r["units"].as_str().unwrap_or("") {
            "millimeter" => "mm",
            "centimeter" => "cm",
            "meter" => "m",
            "inch" => "in",
            "foot" => "ft",
            "yard" => "yd",
            "degree" => "deg",
            "radian" => "rad",
            _ => "",
        };
        let q = |v: &Value| v.as_f64().map(|x| format!("{x} {unit}").trim().to_string());
        let mut args = json!({ "kind": "variable", "name": id, "variable_kind": c["quantityType"].as_str().unwrap_or("LENGTH") });
        if let Some(v) = q(&r["defaultValue"]) {
            args["value"] = json!(v);
        }
        if let (Some(lo), Some(hi)) = (q(&r["minValue"]), q(&r["maxValue"])) {
            args["range"] = json!([lo, hi]);
        }
        if let Err(e) = self.call("config_add_input", args) {
            self.notes.push(format!("configuration variable {id}: {e}"));
        }
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    pub fn call(&mut self, tool: &str, args: Value) -> Result<Value, String> {
        let op = registry::by_tool_name(tool).ok_or_else(|| format!("no tool {tool}"))?;
        registry::run(self.ctx, op, &args).map(|o| o.value).map_err(|e| e.to_json().to_string())
    }

    /// Rebuilds feature `k` (after features 0 … k−1).
    pub fn step(&mut self, k: usize) -> Step {
        let f = self.features[k].clone();
        let mut step = Step { index: k, name: f["name"].as_str().unwrap_or("").to_string(), feature_type: f["featureType"].as_str().unwrap_or("").to_string(), ..Default::default() };
        // Mate connectors made inside the feature's dialog (its sub-features):
        // ours has none in a feature, so each is a Mate connector before it.
        for sub in f["subFeatures"].as_array().into_iter().flatten().filter(|s| s["featureType"] == "mateConnector") {
            let mut problems = Vec::new();
            match self.feature(k, sub, &mut problems) {
                Ok((id, _, _)) => {
                    if let Some(oid) = sub["featureId"].as_str() {
                        self.feature_ids.insert(oid.to_string(), id);
                    }
                    step.problems.push(format!("its in-feature {} built as a feature before it (noshape has none inside a feature)", sub["featureType"].as_str().unwrap_or("?")));
                }
                Err(e) => step.problems.push(format!("its in-feature {}: not built: {e}", sub["featureType"].as_str().unwrap_or("?"))),
            }
            step.problems.extend(problems.into_iter().map(|p| format!("its in-feature {}: {p}", sub["featureType"].as_str().unwrap_or("?"))));
        }
        let made = if f["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketch")) { self.sketch(k, &f, &mut step.problems) } else { self.feature(k, &f, &mut step.problems) };
        match made {
            Ok((id, status, message)) => {
                if let Some(oid) = f["featureId"].as_str() {
                    self.feature_ids.insert(oid.to_string(), id.clone());
                }
                if f["suppressed"].as_bool() == Some(true)
                    && let Err(e) = self.call("feature_suppress", json!({ "features": [id.clone()], "suppressed": true }))
                {
                    step.problems.push(format!("suppressed: {e}"));
                }
                step.ours = Some(id);
                step.status = Some(status);
                step.message = message;
            }
            Err(e) => step.problems.insert(0, format!("not built: {e}")),
        }
        step
    }

    /// A non-sketch feature through feature_add, its parameters by name.
    fn feature(&mut self, k: usize, f: &Value, problems: &mut Vec<String>) -> Result<(String, String, Option<String>), String> {
        let ty = f["featureType"].as_str().ok_or("no featureType")?;
        // A custom feature: its namespace names the Feature Studio ("e<element id>::m<microversion>").
        let custom = f["namespace"].as_str().filter(|n| !n.is_empty()).map(|n| match n.strip_prefix('e').and_then(|r| r.split("::").next()).and_then(|e| self.studios.get(e)) {
            // Ours: `custom:<tab id>:<feature name>` (model-data custom::type_id).
            Some(tab) => Ok(format!("custom:{tab}:{ty}")),
            None => Err(format!("custom feature `{ty}` from a Feature Studio not in features.json ({n})")),
        });
        let owned;
        let ty = match custom {
            Some(Ok(t)) => {
                owned = t;
                owned.as_str()
            }
            Some(Err(e)) => return Err(e),
            None => ty,
        };
        let schema = self.call("feature_schema", json!({ "type": ty, "all": true })).map_err(|e| format!("feature type `{ty}` is not in noshape ({e})"))?;
        let known: Vec<String> = schema["params"]["properties"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
        let mut params = Map::new();
        let mut unknown = Vec::new();
        let given: Vec<&str> = f["parameters"].as_array().into_iter().flatten().filter_map(|p| p["parameterId"].as_str()).collect();
        let mut tolerances = 0;
        // A Hole before V2: its standard is today's table for its units and
        // style (std hole.fs picks it so).
        let legacy_table = (ty == "hole" && !given.contains(&"isV2")).then(|| {
            let value = |id: &str| f["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == id).and_then(|p| p["value"].as_str());
            match (value("unitsSystem") == Some("INCH"), value("style").is_none_or(|s| s == "SIMPLE")) {
                (true, true) => "ansiHoleTableEx",
                (true, false) => "ansiHoleTable",
                (false, true) => "isoHoleTableEx",
                (false, false) => "isoHoleTable",
            }
        });
        for p in f["parameters"].as_array().into_iter().flatten() {
            let Some(id) = p["parameterId"].as_str() else { continue };
            if INTERNAL.contains(&id) {
                continue;
            }
            // Its "Custom" standard (std's tappedOrClearanceHoleTable): no
            // table, whatever its units.
            if legacy_table.is_some() && id == "standardTappedOrClearance" && p["value"]["standard"] == "Custom" {
                for table in ["ansiHoleTableEx", "ansiHoleTable", "isoHoleTableEx", "isoHoleTable"] {
                    params.insert(table.into(), json!([]));
                }
                continue;
            }
            let renamed;
            let (p, id) = match legacy_table {
                Some(table) if id == "standardTappedOrClearance" && !given.contains(&table) => {
                    let mut q = p.clone();
                    q["parameterId"] = json!(table);
                    renamed = q;
                    (&renamed, table)
                }
                _ => (p, id),
            };
            // A legacy parameter beside its newer version (Hole `style` /
            // `styleV2`): the newer one counts.
            if ty == "hole" && (given.contains(&format!("{id}V2").as_str()) || given.contains(&format!("{id}V3").as_str())) {
                continue;
            }
            if !known.iter().any(|n| n == id) && ty == "hole" {
                // A Hole saved by an older std keeps its earlier parameters
                // beside the V3 ones; tolerances don't change the geometry.
                // (`locationSignatures`: the dialog's editing logic; `holeDepthMultiple`: a tolerance's label.)
                if ["Tolerance", "Precision", "holeClass", "toleranceMatching", "locationSignatures", "holeDepthMultiple"].iter().any(|w| id.contains(w)) {
                    tolerances += 1;
                    continue;
                }
                // The pre-V2 Hole's standards (std hole.fs: `!isV2` branch).
                if ["standardTappedOrClearance", "standardBlindInLast"].contains(&id) && given.contains(&"isV2") {
                    continue;
                }
                let stem = id.trim_end_matches("V2");
                if given.contains(&format!("{stem}V3").as_str()) || given.contains(&format!("{stem}V2").as_str()) && known.iter().any(|n| *n == format!("{stem}V2")) {
                    continue;
                }
            }
            match self.param(k, p) {
                Ok(None) => {}
                Ok(Some(v)) if !known.iter().any(|n| n == id) => {
                    // Kept for `upgrade`, which maps older ids onto ours.
                    params.insert(id.to_string(), v);
                    unknown.push(id.to_string());
                }
                Ok(Some(v)) => {
                    params.insert(id.to_string(), v);
                }
                Err(e) => problems.push(format!("{id}: {e}")),
            }
        }
        if tolerances > 0 {
            problems.push(format!("{tolerances} tolerance / precision fields not translated (no effect on the geometry)"));
        }
        upgrade(ty, &mut params, &mut unknown);
        if !unknown.is_empty() {
            problems.push(format!("parameters not in noshape's {ty}: {}", unknown.join(", ")));
        }
        // Parameters Onshape didn't store take the feature's defaults, as
        // Onshape regenerates them (not our dialog's choices: a Remove
        // extrude's side, a merge scope).
        // Except where std treats a missing one otherwise: no `defaultScope`
        // is "Merge with all" (boolean.fs fills it true).
        for (id, prop) in schema["params"]["properties"].as_object().into_iter().flatten() {
            if params.contains_key(id) || given.contains(&id.as_str()) {
                continue;
            }
            let missing = if id == "defaultScope" { Some(json!(true)) } else { prop.get("default").filter(|d| !d.is_null()).cloned() };
            if let Some(d) = missing {
                params.insert(id.clone(), d);
            }
        }
        let mut args = json!({ "type": ty, "params": params });
        // "#featureName": Onshape's placeholder for the default name.
        if let Some(n) = f["name"].as_str().filter(|n| !n.starts_with('#')) {
            args["name"] = json!(n);
        }
        // A parameter we reject: reported, and the feature added without it.
        let r = loop {
            match self.call("feature_add", args.clone()) {
                Ok(r) => break r,
                Err(e) => {
                    let v: Value = serde_json::from_str(&e).unwrap_or(Value::Null);
                    let Some(param) = v["param"].as_str().and_then(|p| p.strip_prefix("params.")).map(|p| p.split(['.', '[']).next().unwrap_or(p).to_string()) else { return Err(e) };
                    if args["params"].as_object_mut().and_then(|m| m.remove(&param)).is_none() {
                        return Err(e);
                    }
                    problems.push(format!("{param}: rejected ({})", v["message"].as_str().unwrap_or(&e)));
                }
            }
        };
        let st = &r["feature_status"];
        Ok((r["feature"].as_str().unwrap_or("").to_string(), st["status"].as_str().unwrap_or("").to_string(), st["message"].as_str().map(str::to_string)))
    }

    /// One Onshape parameter as feature_add takes it (`None`: nothing to set).
    fn param(&mut self, k: usize, p: &Value) -> Result<Option<Value>, String> {
        let ty = p["btType"].as_str().unwrap_or("");
        let kind = ty.split('-').next().unwrap_or(ty);
        Ok(match kind {
            "BTMParameterQuantity" => Some(json!(p["expression"].as_str().ok_or("no expression")?)),
            "BTMParameterNullableQuantity" if p["isNull"].as_bool() == Some(true) => None,
            "BTMParameterNullableQuantity" => Some(json!(p["expression"].as_str().ok_or("no expression")?)),
            "BTMParameterEnum" | "BTMParameterBoolean" | "BTMParameterString" => Some(p["value"].clone()),
            "BTMParameterQueryList" => {
                self.picked.clear();
                let mut ids = Vec::new();
                for q in p["queries"].as_array().into_iter().flatten() {
                    ids.extend(self.query(k, q)?);
                }
                (!ids.is_empty()).then(|| json!(ids))
            }
            "BTMParameterArray" => {
                let mut items = Vec::new();
                for it in p["items"].as_array().into_iter().flatten() {
                    let mut m = Map::new();
                    for ip in it["parameters"].as_array().into_iter().flatten() {
                        let Some(id) = ip["parameterId"].as_str() else { continue };
                        if let Some(v) = self.param(k, ip)? {
                            m.insert(id.to_string(), v);
                        }
                    }
                    items.push(Value::Object(m));
                }
                Some(json!(items))
            }
            // A configured parameter: the default configuration's value
            // (configurations themselves aren't translated yet).
            "BTMParameterConfigured" => {
                let values = p["values"].as_array().ok_or("configured parameter without values")?;
                let want = p["configurationParameterId"].as_str().and_then(|c| self.config_defaults.get(c)).map(String::as_str).unwrap_or("Default");
                let default = values.iter().find(|v| v["enumValue"] == want).or_else(|| values.first()).ok_or("configured parameter without values")?;
                return self.param(k, &default["value"]);
            }
            // A standard's lookup path, by level name: ours in level order.
            "BTMParameterLookupTablePath" => {
                let table = match p["parameterId"].as_str().unwrap_or("") {
                    "isoHoleTableEx" => "hole.isoEx",
                    "isoHoleTable" => "hole.iso",
                    "ansiHoleTableEx" => "hole.ansiEx",
                    "ansiHoleTable" => "hole.ansi",
                    "ansiThreadClass" | "ansiThreadClassV2" => "thread.ansiClass",
                    "isoThreadClass" | "isoThreadClassV2" => "thread.isoClass",
                    // A table we don't have: reported with the feature's unknown parameters.
                    _ => return Ok(Some(p["value"].clone())),
                };
                let by_level = p["value"].as_object().ok_or("lookup path without a value")?;
                let mut path: Vec<String> = Vec::new();
                while let Some(level) = noshape_model::features::hole_tables::table_level_key(table, &path) {
                    let Some(v) = by_level.get(&level).and_then(Value::as_str) else { break };
                    path.push(v.to_string());
                }
                if path.len() < by_level.len() {
                    return Err(format!("lookup path {} only partly in noshape's {table} ({})", Value::Object(by_level.clone()), path.join(" › ")));
                }
                Some(json!(path))
            }
            // A table from another element (Variable's lookup mode): none chosen is nothing to set.
            "BTMParameterReferenceTable" if p["elementLibraryData"]["elementLibraryId"].as_str().is_none_or(str::is_empty) => None,
            "BTMParameterFeatureList" => {
                let ids: Vec<String> = p["featureIds"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|id| self.feature_ids.get(id).map(|o| format!("feature:{o}")).ok_or_else(|| format!("feature {id} was not built"))).collect::<Result<_, _>>()?;
                Some(json!(ids))
            }
            _ => return Err(format!("parameter type {ty} not translated")),
        })
    }

    /// Our reference ids for one Onshape query.
    fn query(&mut self, k: usize, q: &Value) -> Result<Vec<String>, String> {
        let ids: Vec<String> = q["deterministicIds"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
        let text = q["queryString"].as_str().unwrap_or("");
        if !ids.is_empty() && ids.iter().all(|id| self.refs.contains_key(&(k, id.clone()))) {
            let found: Vec<String> = ids.iter().map(|id| self.by_geometry(k, id)).collect::<Result<_, _>>()?;
            // A swept edge a Boolean split: Onshape's query keeps every piece.
            let strings = packed_strings_of(text);
            if strings.iter().any(|s| s == "SPLIT") && strings.iter().any(|s| s == "SWEPT_EDGE") {
                return Ok(self.with_collinear_pieces(found));
            }
            return Ok(found);
        }
        if let Some(p) = default_plane(text) {
            return Ok(vec![p.to_string()]);
        }
        // "Created by" selections: a sketch's faces are its regions, a
        // feature's bodies its parts, else every face / edge it made.
        if q["btType"].as_str().is_some_and(|t| t.starts_with("BTMIndividualCreatedByQuery"))
            && let Some(fid) = q["featureId"].as_str()
        {
            let ours = self.feature_ids.get(fid).cloned().ok_or_else(|| format!("feature {fid} was not built"))?;
            let kind = q["entityType"].as_str().unwrap_or("BODY");
            if q["bodyType"] == "MATE_CONNECTOR" {
                return Ok(vec![format!("mc:{ours}.0")]);
            }
            if kind == "FACE" && let Ok(g) = self.call("sketch_get", json!({ "sketch": ours })) {
                return Ok(g["regions"].as_array().into_iter().flatten().filter_map(|r| r["id"].as_str().map(str::to_string)).collect());
            }
            // A sketch's wire edges: its curves (without construction ones
            // when filtered).
            if kind == "EDGE"
                && let Ok(g) = self.call("sketch_get", json!({ "sketch": ours }))
            {
                let no_construction = q["filterConstruction"] == true;
                return Ok(g["entities"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e["type"] != "point" && !(no_construction && e["construction"] == true))
                    .filter_map(|e| e["id"].as_str().map(str::to_string))
                    .collect());
            }
            let (list, field) = match kind {
                "EDGE" => ("edges", "id"),
                "VERTEX" => ("vertices", "id"),
                "FACE" => ("faces", "id"),
                _ => ("faces", "part"),
            };
            let r = self.call("query_entities", json!({ "kind": list, "created_by": ours, "limit": 10000 }))?;
            let mut ids: Vec<String> = Vec::new();
            for id in r[list].as_array().into_iter().flatten().filter_map(|e| e[field].as_str()) {
                if !ids.iter().any(|x| x == id) {
                    ids.push(id.to_string());
                }
            }
            if !ids.is_empty() {
                return Ok(ids);
            }
        }
        // The default planes' and the Origin's deterministic ids (fixed in
        // every Part Studio), also when the query text is empty.
        if let [id] = ids.as_slice()
            && let Some(p) = [("JDC", "plane:top"), ("JCC", "plane:front"), ("JEC", "plane:right"), ("IB", "origin")].iter().find(|(i, _)| i == id).map(|(_, p)| p)
        {
            return Ok(vec![p.to_string()]);
        }
        // A Plane feature's plane (`dummyQuery(id+"F….planeOp", …)`).
        if let Some(fid) = text.split("id+\"").nth(1).and_then(|r| r.split(".planeOp\"").next()).filter(|f| !f.contains('"'))
            && let Some(ours) = self.feature_ids.get(fid)
        {
            return Ok(vec![format!("fplane:{ours}")]);
        }
        // A Plane feature's plane, packed (`DUMMY` of "F….planeOp").
        if let Some(fid) = packed_strings_of(text).iter().find_map(|s| s.strip_suffix(".planeOp").map(str::to_string))
            && let Some(ours) = self.feature_ids.get(&fid)
        {
            return Ok(vec![format!("fplane:{ours}")]);
        }
        if let Some(r) = self.region_from_query(text)? {
            return Ok(vec![r]);
        }
        if let Some(e) = self.sketch_entity_from_query(text) {
            return Ok(vec![e]);
        }
        if let Some(r) = self.extrude_name_from_query(text) {
            return r.map(|e| vec![e]);
        }
        // sketchEntityQuery(id+"F….wireOp", EntityType.EDGE, "<entity>").
        if let Some(rest) = text.split("sketchEntityQuery(id+\"").nth(1)
            && let Some((sk, after)) = rest.split_once(".wireOp\"")
            && let Some(e) = after.split('"').nth(1)
            && let Some(ours) = self.sketch_entities.get(&(sk.to_string(), e.to_string()))
        {
            return Ok(vec![ours.clone()]);
        }
        // qCreatedBy(id + "F…", EntityType.BODY): the parts a feature made.
        if let Some(fid) = text.split("qCreatedBy(id + \"").nth(1).and_then(|r| r.split('"').next())
            && text.contains("EntityType.BODY")
        {
            let ours = self.feature_ids.get(fid).cloned().ok_or_else(|| format!("feature {fid} was not built"))?;
            let f = self.call("query_entities", json!({ "kind": "faces", "created_by": ours, "limit": 200 }))?;
            let mut parts: Vec<String> = f["faces"].as_array().into_iter().flatten().filter_map(|x| x["part"].as_str().map(str::to_string)).collect();
            parts.dedup();
            if !parts.is_empty() {
                return Ok(parts);
            }
        }
        // qCreatedBy(makeId("Top"), EntityType.FACE): a default plane; of a
        // Plane feature: its plane.
        if let Some(fid) = text.split("qCreatedBy(makeId(\"").nth(1).and_then(|r| r.split('"').next())
            && text.contains("EntityType.FACE")
        {
            if let Some(p) = default_plane(&format!("{fid}.planeOp")) {
                return Ok(vec![p.to_string()]);
            }
            if let Some(ours) = self.feature_ids.get(fid) {
                return Ok(vec![format!("fplane:{ours}")]);
            }
        }
        // qContainsPoint(…EntityType.EDGE / FACE / VERTEX…, vector(x, y, z) * unit).
        if let Some(rest) = text.split("qContainsPoint(").nth(1)
            && let Some(v) = rest.split("vector(").nth(1)
        {
            let (nums, unit) = v.split_once(')').unwrap_or((v, ""));
            let c: Vec<f64> = nums.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            let scale = if unit.contains("millimeter") { 1e-3 } else if unit.contains("inch") { 0.0254 } else if unit.contains("centimeter") { 1e-2 } else { 1.0 };
            let kind = ["EDGE", "FACE", "VERTEX"].into_iter().find(|k| rest.contains(&format!("EntityType.{k}"))).unwrap_or("EDGE");
            if c.len() == 3 {
                let r = LiveRef { kind: kind.into(), point: Some([c[0] * scale, c[1] * scale, c[2] * scale]), ..Default::default() };
                let key = (k, format!("point:{text}"));
                self.refs.insert(key.clone(), r);
                return self.by_geometry(k, &key.1).map(|e| vec![e]);
            }
        }
        // qAllModifiableSolidBodies(): every part.
        if text.contains("qAllModifiableSolidBodies()") && !text.contains("qOwnedByBody") {
            let parts = self.call("query_parts", json!({}))?;
            return Ok(parts["parts"].as_array().into_iter().flatten().filter_map(|p| p["id"].as_str().map(str::to_string)).collect());
        }
        // qCreatedBy(makeId("F…"), EntityType.VERTEX): a sketch's points.
        if let Some(sk) = text.split("qCreatedBy(makeId(\"").nth(1).and_then(|r| r.split('"').next())
            && text.contains("EntityType.VERTEX")
        {
            let points: Vec<String> = self.sketch_points.iter().filter(|(s, _)| s == sk).map(|(_, e)| e.clone()).collect();
            if !points.is_empty() {
                return Ok(points);
            }
        }
        // qSketchRegion(id + "F…", filterInnerLoops): the sketch's regions;
        // with `true` only those not inside another (a region whose curves
        // all bound another region with holes is one of those holes' insides).
        if let Some((sk, rest)) = text.split("qSketchRegion(id + \"").nth(1).and_then(|r| r.split_once('"')) {
            let ours = self.feature_ids.get(sk).cloned().ok_or_else(|| format!("sketch {sk} was not built"))?;
            let g = self.call("sketch_get", json!({ "sketch": ours }))?;
            // (Not the face's imprinted pieces: sketch faces, not regions.)
            let regions: Vec<&Value> = g["regions"].as_array().into_iter().flatten().filter(|r| r["imprinted"] != true).collect();
            let curves = |r: &Value| -> Vec<String> { r["boundary"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
            let outer_only = rest.trim_start_matches([',', ' ']).starts_with("true");
            let inner = |r: &Value| {
                let mine = curves(r);
                regions.iter().any(|o| o["id"] != r["id"] && o["holes"].as_u64().unwrap_or(0) > 0 && { let theirs = curves(o); mine.iter().all(|c| theirs.contains(c)) })
            };
            return Ok(regions.iter().filter(|r| !outer_only || !inner(r)).filter_map(|r| r["id"].as_str().map(str::to_string)).collect());
        }
        Err(format!("query not mapped (no live geometry for {}): {}", if ids.is_empty() { "it".to_string() } else { ids.join(", ") }, text.chars().take(80).collect::<String>()))
    }

    /// A sketch region query (Onshape's packed form: a FACE whose
    /// TOPOLOGY is the sketch entities bounding it): our region whose
    /// interior point lies inside those curves. `None`: not a region query.
    fn region_from_query(&mut self, text: &str) -> Result<Option<String>, String> {
        let Some(packed) = text.split("qCompressed(1.0,\"").nth(1).and_then(|r| r.split('"').next()) else { return Ok(None) };
        let Ok(tree) = noshape_model::custom::host::packed::unpack(packed) else { return Ok(None) };
        let mut strings = Vec::new();
        packed_strings(&tree, &mut strings);
        // A face the sketch imprints, told by the curves around it (a face
        // swept from sketch curves names them too, without the imprint).
        if !["SKETCH_ENTITY", "FACE", "TOPOLOGY", "IMPRINT"].iter().all(|w| strings.iter().any(|s| s == w)) {
            return Ok(None);
        }
        let Some(sk) = strings.iter().find_map(|s| s.strip_suffix(".wireOp")).map(str::to_string) else { return Ok(None) };
        let mut segments: Vec<[f64; 2]> = strings.iter().filter_map(|e| self.curves.get(&(sk.clone(), e.clone()))).flat_map(|poly| poly.windows(2).flat_map(|w| [w[0], w[1]])).collect();
        if segments.is_empty() {
            return Err(format!("region of sketch {sk}: its curves were not translated"));
        }
        let ours = self.feature_ids.get(&sk).cloned().ok_or_else(|| format!("sketch {sk} was not built"))?;
        let g = self.call("sketch_get", json!({ "sketch": ours }))?;
        let (o, x, y) = (f3(&g["frame"]["origin"]), f3(&g["frame"]["x"]), f3(&g["frame"]["y"]));
        let (Some(o), Some(x), Some(y)) = (o, x, y) else { return Err("sketch frame".into()) };
        let frame = self.frames.get(&sk).copied();
        // A piece of the face the sketch is on is named by the face's edges too: other sketches' curves they came from
        // ("<sketch>.wireOp" … sketchEntityId), into this sketch's plane.
        let mut derived = false;
        let mut face_edges: Vec<[f64; 2]> = Vec::new();
        if let Some([fo, fx, fy]) = frame {
            let mut of: Option<&str> = None;
            for w in strings.windows(2) {
                if let Some(other) = w[1].strip_suffix(".wireOp").filter(|_| w[0] == "operationId") {
                    of = Some(other);
                }
                let (Some(other), true) = (of.filter(|o| *o != sk), w[0] == "sketchEntityId") else { continue };
                let (Some(poly), Some([go, gx, gy])) = (self.curves.get(&(other.to_string(), w[1].clone())), self.frames.get(other)) else { continue };
                let to_here = |p: &[f64; 2]| {
                    let world: Vec<f64> = (0..3).map(|i| go[i] + p[0] * gx[i] + p[1] * gy[i] - fo[i]).collect();
                    [(0..3).map(|i| world[i] * fx[i]).sum::<f64>(), (0..3).map(|i| world[i] * fy[i]).sum::<f64>()]
                };
                let pieces: Vec<[f64; 2]> = poly.windows(2).flat_map(|p| [to_here(&p[0]), to_here(&p[1])]).collect();
                segments.extend(&pieces);
                face_edges.extend(pieces);
                derived = true;
            }
        }
        // Named by the sketch's own curves alone: the region its loops make,
        // whole, though the face's edges split it now (live: knob-tim's
        // Extrude 2 adds the whole circle over a hexagonal hole). Ours: its
        // key without the imprinted edges' segments (entity ids from 2^30).
        let whole = |id: &str| -> String {
            if derived {
                return id.to_string();
            }
            let Some((head, keys)) = id.rsplit_once(':') else { return id.to_string() };
            let kept: Vec<&str> = keys.split(',').enumerate().filter(|(i, k)| *i == 0 || k.parse::<u64>().is_ok_and(|v| v < 1 << 31)).map(|(_, k)| k).collect();
            format!("{head}:{}", kept.join(","))
        };
        // Onshape names the region by enough of its boundary curves to tell
        // it apart (TOPOLOGY): ours whose boundary has them all.
        let named: Vec<String> = strings.iter().filter_map(|e| self.sketch_entities.get(&(sk.clone(), e.clone())).cloned()).collect();
        let all: Vec<Value> = g["regions"].as_array().cloned().unwrap_or_default();
        let bounded: Vec<Value> = all.iter().filter(|r| !named.is_empty() && named.iter().all(|e| r["boundary"].as_array().is_some_and(|b| b.iter().any(|x| x == e)))).cloned().collect();
        if let [only] = bounded.as_slice() {
            return Ok(only["id"].as_str().map(whole));
        }
        // A piece of the face (named by face edges too): the pieces whose own
        // curves are exactly the named ones, the one nearest those face edges
        // (each side of a symmetric part names its own edges).
        if derived && !named.is_empty() {
            let own = |r: &Value| -> std::collections::BTreeSet<String> {
                r["boundary"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|e| e.rsplit(':').next().and_then(|n| n.parse::<u64>().ok()).is_some_and(|n| n < 1 << 30)).map(str::to_string).collect()
            };
            let want: std::collections::BTreeSet<String> = named.iter().cloned().collect();
            let exact: Vec<&Value> = bounded.iter().filter(|r| own(r) == want).collect();
            let k = per_unit(&g["units"]);
            let near = |r: &Value| -> f64 {
                let (u, v) = (r["inside"][0].as_f64().unwrap_or(0.0), r["inside"][1].as_f64().unwrap_or(0.0));
                let w: Vec<f64> = (0..3).map(|i| (o[i] + u * x[i] + v * y[i]) * k).collect();
                let p = match frame {
                    Some([fo, fx, fy]) => [(0..3).map(|i| (w[i] - fo[i]) * fx[i]).sum::<f64>(), (0..3).map(|i| (w[i] - fo[i]) * fy[i]).sum::<f64>()],
                    None => [u * k, v * k],
                };
                face_edges.chunks(2).map(|s| {
                    let (d, e) = ([s[1][0] - s[0][0], s[1][1] - s[0][1]], [p[0] - s[0][0], p[1] - s[0][1]]);
                    let t = ((e[0] * d[0] + e[1] * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-30)).clamp(0.0, 1.0);
                    (e[0] - t * d[0]).hypot(e[1] - t * d[1])
                }).fold(f64::INFINITY, f64::min)
            };
            // A mirrored pair names the same edges: each takes one.
            let fresh: Vec<&&Value> = exact.iter().filter(|r| r["id"].as_str().is_some_and(|id| !self.picked.contains(id))).collect();
            let pool = if fresh.is_empty() { exact.iter().collect() } else { fresh };
            if let Some(id) = pool.iter().min_by(|a, b| near(a).total_cmp(&near(b))).and_then(|r| r["id"].as_str()).map(str::to_string) {
                self.picked.insert(id.clone());
                return Ok(Some(id));
            }
        }
        // Several share them: the named curve's whole shape decides (a
        // rectangle's `.left` side alone is on both its inside and the region
        // beside it; live: bracket120's Extrude 2 takes the rectangle).
        if bounded.len() > 1 {
            let shapes: std::collections::BTreeSet<&str> = strings.iter().filter(|e| self.curves.contains_key(&(sk.clone(), (*e).clone()))).filter_map(|e| e.rsplit_once('.').map(|(p, _)| p)).collect();
            // Each side not named yet, once.
            let siblings = self.curves.iter().filter(|((s, id), _)| *s == sk && !strings.contains(id) && id.rsplit_once('.').is_some_and(|(p, _)| shapes.contains(p)));
            segments.extend(siblings.flat_map(|(_, poly)| poly.windows(2).flat_map(|w| [w[0], w[1]])));
        }
        // Else (several share them, or none known): the one inside those curves.
        let regions = if bounded.is_empty() { all } else { bounded };
        let mut best: Option<(f64, String)> = None;
        for r in &regions {
            let (u, v) = (r["inside"][0].as_f64().unwrap_or(0.0), r["inside"][1].as_f64().unwrap_or(0.0));
            // Ours (mm, our frame) → Onshape's sketch plane (m).
            let k = per_unit(&g["units"]);
            let w: Vec<f64> = (0..3).map(|i| (o[i] + u * x[i] + v * y[i]) * k).collect();
            let p = match frame {
                Some([fo, fx, fy]) => [(0..3).map(|i| (w[i] - fo[i]) * fx[i]).sum::<f64>(), (0..3).map(|i| (w[i] - fo[i]) * fy[i]).sum::<f64>()],
                None => [u * k, v * k],
            };
            let crossings = segments.chunks(2).filter(|s| (s[0][1] > p[1]) != (s[1][1] > p[1]) && p[0] < s[0][0] + (p[1] - s[0][1]) * (s[1][0] - s[0][0]) / (s[1][1] - s[0][1])).count();
            let area = r["area"].as_f64().unwrap_or(0.0);
            if crossings % 2 == 1
                && best.as_ref().is_none_or(|b| area > b.0)
                && let Some(id) = r["id"].as_str()
            {
                best = Some((area, id.to_string()));
            }
        }
        best.map(|b| Some(whole(&b.1))).ok_or_else(|| format!("no region of ours inside sketch {sk}'s curves {:?}", strings.iter().filter(|e| self.curves.contains_key(&(sk.clone(), (*e).clone()))).collect::<Vec<_>>()))
    }

    /// A sketch curve or point named by its Onshape id (packed
    /// `SKETCH_ENTITY` query of a sketch's own wire, not of a feature made
    /// from it).
    fn sketch_entity_from_query(&self, text: &str) -> Option<String> {
        let packed = text.split("qCompressed(1.0,\"").nth(1)?.split('"').next()?;
        let tree = noshape_model::custom::host::packed::unpack(packed).ok()?;
        let mut strings = Vec::new();
        packed_strings(&tree, &mut strings);
        // An imprint edge derived from the sketch's own entity is that entity
        // here (not an imprinted face: a region).
        let imprint = strings.iter().any(|s| s == "IMPRINT");
        if !strings.iter().any(|s| s == "SKETCH_ENTITY") || (imprint && strings.iter().any(|s| s == "FACE" || s == "TOPOLOGY")) {
            return None;
        }
        let mut ops = strings.iter().filter(|s| s.contains(".op") || s.ends_with("Op"));
        let sk = ops.next()?.strip_suffix(".wireOp")?.to_string();
        if ops.next().is_some() || (imprint && strings.iter().any(|s| s.ends_with(".imprint") && *s != format!("{sk}.imprint"))) {
            return None;
        }
        strings.iter().find_map(|e| self.sketch_entities.get(&(sk.clone(), e.clone())).cloned())
    }

    /// A face, edge or part an Extrude made, by Onshape's name for it
    /// (packed `CAP_FACE` / `SWEPT_FACE` / `CAP_EDGE` / `SWEPT_EDGE` of
    /// `….opExtrude` from sketch curves, `SWEPT_BODY`): ours of the same
    /// Extrude through the same sketch geometry. Used when live's geometry
    /// for the id is missing.
    fn extrude_name_from_query(&mut self, text: &str) -> Option<Result<String, String>> {
        use noshape_model::custom::host::packed::{Packed, unpack};
        let packed = text.split("qCompressed(1.0,\"").nth(1)?.split('"').next()?;
        let tree = unpack(packed).ok()?;
        fn find<'p>(p: &'p Packed, names: &[&str]) -> Option<&'p Vec<(Packed, Packed)>> {
            match p {
                Packed::Map(m) if m.iter().any(|(k, v)| matches!(k, Packed::Str(k) if k == "queryType") && matches!(v, Packed::Str(v) if names.contains(&v.as_str()))) => Some(m),
                Packed::Map(m) => m.iter().find_map(|(_, v)| find(v, names)),
                Packed::Array(a) => a.iter().find_map(|v| find(v, names)),
                Packed::Tagged(_, v) => find(v, names),
                _ => None,
            }
        }
        let names = ["CAP_FACE", "SWEPT_FACE", "CAP_EDGE", "SWEPT_EDGE", "SWEPT_BODY"];
        // Nested in another operation's name: only a Boolean keeps the entity
        // itself (a pattern's or mirror's COPY is another entity).
        if let Packed::Tagged(_, top) = &tree
            && let Packed::Map(tm) = top.as_ref()
            && !tm.iter().any(|(k, v)| matches!(k, Packed::Str(k) if k == "queryType") && matches!(v, Packed::Str(v) if names.contains(&v.as_str())))
        {
            let mut op = Vec::new();
            if let Some((_, v)) = tm.iter().find(|(k, _)| matches!(k, Packed::Str(k) if k == "operationId")) {
                packed_strings(v, &mut op);
            }
            if !op.iter().any(|o| o.ends_with(".opBoolean")) {
                return None;
            }
        }
        let m = find(&tree, &names)?;
        let get = |key: &str| m.iter().find(|(k, _)| matches!(k, Packed::Str(k) if k == key)).map(|(_, v)| v);
        let Some(Packed::Str(name)) = get("queryType") else { return None };
        let is_start = matches!(get("isStart"), Some(Packed::Bool(true)));
        let mut strings = Vec::new();
        packed_strings(&Packed::Map(m.clone()), &mut strings);
        if name == "SWEPT_BODY" {
            // Whatever made it (opExtrude, opRevolve, opThicken…): the feature's part.
            let op = strings.iter().find_map(|s| s.rsplit_once(".op").map(|(f, _)| f.to_string()))?;
            // The part our Extrude made (the part of its faces).
            let Some(ours) = self.feature_ids.get(&op).cloned() else { return Some(Err(format!("SWEPT_BODY: feature {op} was not built"))) };
            let r = self.call("query_entities", json!({ "kind": "faces", "created_by": ours, "limit": 1 }));
            return Some(r.and_then(|f| f["faces"][0]["part"].as_str().map(str::to_string).ok_or_else(|| format!("SWEPT_BODY of {op}: no part of ours"))));
        }
        let (op, revolve) = strings.iter().find_map(|s| s.strip_suffix(".opExtrude").map(|o| (o.to_string(), false)).or_else(|| s.strip_suffix(".opRevolve").map(|o| (o.to_string(), true))))?;
        let sk = strings.iter().find_map(|s| s.strip_suffix(".wireOp"))?.to_string();
        let curves: Vec<Vec<[f64; 2]>> = strings.iter().filter_map(|e| self.curves.get(&(sk.clone(), e.clone())).cloned()).collect();
        Some(if revolve { self.revolve_entity(name, &op, &sk, &curves) } else { self.extrude_entity(name, is_start, &op, &sk, &curves) })
    }

    /// A Revolve's face or edge by Onshape's name: the face swept by a
    /// profile curve contains it, the edge swept by a profile vertex is the
    /// circle through it (ours of the same Revolve nearest that point).
    fn revolve_entity(&mut self, name: &str, op: &str, sk: &str, curves: &[Vec<[f64; 2]>]) -> Result<String, String> {
        let ours = self.feature_ids.get(op).cloned().ok_or_else(|| format!("{name}: revolve {op} was not built"))?;
        let our_sketch = self.feature_ids.get(sk).cloned().ok_or_else(|| format!("{name}: sketch {sk} was not built"))?;
        let first = curves.first().ok_or_else(|| format!("{name} of {op}: its sketch curves were not translated"))?;
        let [o, x, y] = match self.frames.get(sk) {
            Some(f) => *f,
            None => {
                let g = self.call("sketch_get", json!({ "sketch": our_sketch }))?;
                let f = |k: &str| f3(&g["frame"][k]).ok_or("sketch frame");
                let o = f("origin")?;
                [o.map(|c| c * per_unit(&g["units"])), f("x")?, f("y")?]
            }
        };
        let world = |p: [f64; 2]| [o[0] + p[0] * x[0] + p[1] * y[0], o[1] + p[0] * x[1] + p[1] * y[1], o[2] + p[0] * x[2] + p[1] * y[2]];
        let (kind, at, curve) = match name {
            "SWEPT_EDGE" => {
                let shared = curves.get(1).and_then(|b| [first[0], first[first.len() - 1]].into_iter().find(|p| [b[0], b[b.len() - 1]].iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-9)));
                ("edges", world(shared.unwrap_or(first[0])), Some("circle"))
            }
            "SWEPT_FACE" => ("faces", world(if first.len() == 2 { [(first[0][0] + first[1][0]) / 2.0, (first[0][1] + first[1][1]) / 2.0] } else { first[first.len() / 2] }), None),
            _ => return Err(format!("{name} of a revolve not translated")),
        };
        let mut a = json!({ "kind": kind, "created_by": ours, "near": mm(at), "limit": 1 });
        if let Some(c) = curve {
            a["curve"] = json!(c);
        }
        let r = self.call("query_entities", a)?;
        r[kind][0]["id"].as_str().map(str::to_string).ok_or_else(|| format!("{name} of {op}: no entity of ours near {:?} mm", mm(at)))
    }

    fn extrude_entity(&mut self, name: &str, is_start: bool, op: &str, sk: &str, curves: &[Vec<[f64; 2]>]) -> Result<String, String> {
        let ours = self.feature_ids.get(op).cloned().ok_or_else(|| format!("{name}: extrude {op} was not built"))?;
        let our_sketch = self.feature_ids.get(sk).cloned().ok_or_else(|| format!("{name}: sketch {sk} was not built"))?;
        if curves.is_empty() {
            return Err(format!("{name} of {op}: its sketch curves were not translated"));
        }
        // Onshape's sketch frame (else ours, the same for default planes).
        let [o, x, y] = match self.frames.get(sk) {
            Some(f) => *f,
            None => {
                let g = self.call("sketch_get", json!({ "sketch": our_sketch }))?;
                let f = |k: &str| f3(&g["frame"][k]).ok_or("sketch frame");
                let o = f("origin")?;
                [o.map(|c| c * per_unit(&g["units"])), f("x")?, f("y")?]
            }
        };
        let n = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
        let world = |p: [f64; 2]| [o[0] + p[0] * x[0] + p[1] * y[0], o[1] + p[0] * x[1] + p[1] * y[1], o[2] + p[0] * x[2] + p[1] * y[2]];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        // Off the line through `p` along the normal (m), and along it.
        let lateral = |q: [f64; 3], p: [f64; 3]| {
            let d = sub(q, p);
            let a = dot(d, n);
            ((d[0] - a * n[0]).powi(2) + (d[1] - a * n[1]).powi(2) + (d[2] - a * n[2]).powi(2)).sqrt()
        };
        // A line is its two ends; arcs and splines are finely sampled.
        let mid = |c: &Vec<[f64; 2]>| if c.len() == 2 { [(c[0][0] + c[1][0]) / 2.0, (c[0][1] + c[1][1]) / 2.0] } else { c[c.len() / 2] };
        let (kind, key, at) = match name {
            // The edge swept from the vertex the two curves share.
            "SWEPT_EDGE" => {
                let shared = curves.first().and_then(|a| curves.get(1).and_then(|b| [a[0], a[a.len() - 1]].into_iter().find(|p| [b[0], b[b.len() - 1]].iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-9))));
                ("edges", "midpoint", world(shared.ok_or("SWEPT_EDGE: its curves share no vertex")?))
            }
            "SWEPT_FACE" => ("faces", "centroid", world(mid(&curves[0]))),
            "CAP_EDGE" => ("edges", "midpoint", world(mid(&curves[0]))),
            _ => ("faces", "centroid", world(mid(&curves[0]))),
        };
        let found = self.call("query_entities", json!({ "kind": kind, "created_by": ours, "limit": 200 }))?;
        let k = per_unit(&found["units"]);
        let cands: Vec<(String, [f64; 3])> = found[kind].as_array().into_iter().flatten().filter_map(|e| Some((e["id"].as_str()?.to_string(), f3(&e[key])?.map(|c| c * k)))).collect();
        // A whole circle's midpoint is its centre: it is over the sketch
        // curve when the curve's point is a radius from it.
        let rings: HashMap<String, f64> = found[kind].as_array().into_iter().flatten().filter(|e| e["closed"] == true).filter_map(|e| Some((e["id"].as_str()?.to_string(), e["radius"].as_f64()? * k))).collect();
        let lateral = |c: &(String, [f64; 3]), p: [f64; 3]| match rings.get(&c.0) {
            Some(r) => (lateral(c.1, p) - r).abs(),
            None => lateral(c.1, p),
        };
        let pick = match name {
            // On the line through the sketch point along the normal; swept
            // faces: the one whose centroid is nearest that line.
            "SWEPT_EDGE" | "SWEPT_FACE" => cands.iter().min_by(|a, b| lateral(a, at).total_cmp(&lateral(b, at))),
            // Caps: over the sketch curve, nearest (start) or farthest (end) along the extrude.
            _ => {
                let over: Vec<&(String, [f64; 3])> = cands.iter().filter(|c| lateral(c, at) < 1e-6 || name == "CAP_FACE").collect();
                let along = |c: &&(String, [f64; 3])| dot(sub(c.1, o), n).abs();
                let extreme = if is_start { over.iter().map(along).fold(f64::INFINITY, f64::min) } else { over.iter().map(along).fold(f64::NEG_INFINITY, f64::max) };
                let level: Vec<&(String, [f64; 3])> = over.into_iter().filter(|c| (along(c) - extreme).abs() < 1e-6).collect();
                // Several caps at that level (separate profiles): the one
                // over the named curves (its centroid inside them).
                let segments: Vec<[f64; 2]> = curves.iter().flat_map(|c| c.windows(2).flat_map(|w| [w[0], w[1]])).collect();
                let inside = |c: &&(String, [f64; 3])| {
                    let d = sub(c.1, o);
                    let p = [dot(d, x), dot(d, y)];
                    segments.chunks(2).filter(|s| (s[0][1] > p[1]) != (s[1][1] > p[1]) && p[0] < s[0][0] + (p[1] - s[0][1]) * (s[1][0] - s[0][0]) / (s[1][1] - s[0][1])).count() % 2 == 1
                };
                if name == "CAP_FACE" && level.len() > 1 { level.iter().copied().find(inside).or(level.first().copied()) } else { level.first().copied() }
            }
        };
        pick.map(|c| c.0.clone()).ok_or_else(|| format!("{name} of {op}: no entity of ours matches"))
    }

    /// The entity of ours at live's geometry of `id`.
    fn by_geometry(&mut self, k: usize, id: &str) -> Result<String, String> {
        let r = self.refs[&(k, id.to_string())].clone();
        let p = r.point.ok_or_else(|| format!("{id}: no point"))?;
        let pick = |v: &Value, key: &str| v[key].as_array().and_then(|a| a.first()).and_then(|e| e["id"].as_str()).map(str::to_string);
        match r.kind.as_str() {
            "PLANE" if p.iter().all(|c| c.abs() < 1e-9) => {
                let n = r.normal.ok_or_else(|| format!("{id}: plane without a normal"))?;
                let axis = (0..3).max_by(|a, b| n[*a].abs().total_cmp(&n[*b].abs())).unwrap_or(2);
                Ok(["plane:right", "plane:front", "plane:top"][axis].to_string())
            }
            "FACE" | "PLANE" => self.nearest("faces", &r).ok_or_else(|| format!("{id}: no face of ours at {:?} mm", mm(p))),
            "EDGE" => self.nearest("edges", &r).ok_or_else(|| format!("{id}: no edge of ours at {:?} mm", mm(p))),
            "VERTEX" => pick(&self.call("query_entities", json!({ "kind": "vertices", "at": [mm(p)] }))?, "vertices").ok_or_else(|| format!("{id}: no vertex of ours at {:?} mm", mm(p))),
            "BODY" => {
                let parts = self.call("query_parts", json!({}))?;
                let k = per_unit(&parts["units"]);
                let at = |v: &Value, i: usize| v.as_array().and_then(|a| a.get(i)).and_then(Value::as_f64).map_or(f64::NAN, |c| c * k);
                // The part whose box holds live's point, else the nearest box
                // (an earlier difference changed the part; that shows already).
                let off = |b: &Value| (0..3).map(|i| (at(&b["bounding_box"]["min"], i) - p[i]).max(p[i] - at(&b["bounding_box"]["max"], i)).max(0.0).powi(2)).sum::<f64>().sqrt();
                parts["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|b| !off(b).is_nan())
                    .min_by(|a, b| off(a).total_cmp(&off(b)))
                    .and_then(|b| b["id"].as_str().map(str::to_string))
                    .ok_or_else(|| format!("{id}: no part of ours"))
            }
            "SKETCH_REGION" => self.region_at(p, r.measure),
            other => Err(format!("{id}: reference kind {other} not mapped")),
        }
    }

    /// Our face or edge for live's: of its type, nearest live's point, the
    /// first whose area / length is live's (a face's point is its box
    /// centre, which may lie in a hole or on an axis), else the nearest.
    fn nearest(&mut self, kind: &str, r: &LiveRef) -> Option<String> {
        let p = r.point?;
        let mut a = json!({ "kind": kind, "near": mm(p), "limit": 8 });
        if let Some(t) = &r.geometry {
            a[if kind == "faces" { "surface" } else { "curve" }] = json!(t);
        }
        if let Some(n) = r.normal {
            a["normal"] = json!(n);
        }
        let mut reply = self.call("query_entities", a.clone()).ok()?;
        if reply[kind].as_array().is_none_or(Vec::is_empty) && r.normal.is_some() {
            // The normal's sense isn't always certain.
            a.as_object_mut().map(|m| m.remove("normal"));
            reply = self.call("query_entities", a).ok()?;
        }
        let found = reply[kind].as_array().cloned().unwrap_or_default();
        let k = per_unit(&reply["units"]);
        let (key, scale) = if kind == "faces" { ("area", k * k) } else { ("length", k) };
        let same = |e: &&Value| r.measure.zip(e[key].as_f64()).is_some_and(|(m, ours)| (ours * scale - m).abs() <= 1e-5 * m.abs() + 1e-12);
        found.iter().find(same).or(found.first()).and_then(|e| e["id"].as_str()).map(str::to_string)
    }

    /// The sketch region of ours with live's area (when given) nearest
    /// live's point, over every sketch built so far. (Our region `center`
    /// is a label point, not the centroid live gives: area decides.)
    fn region_at(&mut self, p: [f64; 3], area: Option<f64>) -> Result<String, String> {
        let sketches: Vec<String> = self.feature_ids.values().cloned().collect();
        let mut best: Option<(f64, String)> = None;
        for s in sketches {
            let Ok(g) = self.call("sketch_get", json!({ "sketch": s })) else { continue };
            let (o, x, y) = (f3(&g["frame"]["origin"]), f3(&g["frame"]["x"]), f3(&g["frame"]["y"]));
            let (Some(o), Some(x), Some(y)) = (o, x, y) else { continue };
            for r in g["regions"].as_array().into_iter().flatten() {
                let c = &r["center"];
                let (u, v) = (c[0].as_f64().unwrap_or(0.0), c[1].as_f64().unwrap_or(0.0));
                let k = per_unit(&g["units"]);
                let w: Vec<f64> = (0..3).map(|i| (o[i] + u * x[i] + v * y[i]) * k).collect();
                let d = (0..3).map(|i| (w[i] - p[i]).powi(2)).sum::<f64>().sqrt();
                if let Some(a) = area
                    && r["area"].as_f64().is_none_or(|ours| (ours * k * k - a).abs() > 1e-6 * a.max(1e-12))
                {
                    continue;
                }
                if best.as_ref().is_none_or(|b| d < b.0)
                    && let Some(id) = r["id"].as_str()
                {
                    best = Some((d, id.to_string()));
                }
            }
        }
        match best {
            Some((d, id)) if area.is_some() || d < 1e-3 => Ok(id),
            _ => Err(format!("no sketch region of ours at {:?} mm", mm(p))),
        }
    }

    /// A sketch: its plane, its curves at Onshape's solved positions (our
    /// inference adds the coincidences), then its dimensions.
    fn sketch(&mut self, k: usize, f: &Value, problems: &mut Vec<String>) -> Result<(String, String, Option<String>), String> {
        let oid = f["featureId"].as_str().unwrap_or("").to_string();
        let plane_q = f["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == "sketchPlane").ok_or("no sketchPlane")?;
        let plane = self.param(k, plane_q)?.and_then(|v| v.as_array().and_then(|a| a.first()).cloned()).ok_or("no sketch plane")?;
        let mut args = json!({ "plane": plane });
        if let Some(n) = f["name"].as_str().filter(|n| !n.starts_with('#')) {
            args["name"] = json!(n);
        }
        let made = self.call("sketch_create", args)?;
        let id = made["sketch"].as_str().ok_or("sketch_create gave no id")?.to_string();
        let frame = self.frames.get(&oid).copied();
        // Sketch (x, y) in metres → a sketch_draw point: model mm through
        // Onshape's frame when known, else our frame's mm.
        let at = |u: f64, v: f64| match frame {
            Some([o, x, y]) => mm([o[0] + u * x[0] + v * y[0], o[1] + u * x[1] + v * y[1], o[2] + u * x[2] + v * y[2]]),
            None => json!([format!("{} mm", u * 1000.0), format!("{} mm", v * 1000.0)]),
        };
        let mut shapes = Vec::new();
        let mut local = Vec::new();
        // Onshape's start and end of each curve (sketch metres): ours may
        // run the other way (an arc is counter-clockwise).
        let mut ends: Vec<Option<[[f64; 2]; 2]>> = Vec::new();
        for e in f["entities"].as_array().into_iter().flatten() {
            let eid = e["entityId"].as_str().unwrap_or("").to_string();
            let g = &e["geometry"];
            let construction = e["isConstruction"].as_bool() == Some(true);
            let gt = g["btType"].as_str().unwrap_or("");
            let num = |k: &str| g[k].as_f64().unwrap_or(0.0);
            if gt.starts_with("BTCurveGeometryLine") && (e["endParam"].as_f64().unwrap_or(0.0) - e["startParam"].as_f64().unwrap_or(0.0)).abs() < 1e-9 {
                problems.push(format!("sketch entity {eid}: a line of zero length, left out"));
                continue;
            }
            let shape = match gt.split('-').next().unwrap_or("") {
                "BTCurveGeometryLine" => {
                    let (t0, t1) = (e["startParam"].as_f64().unwrap_or(0.0), e["endParam"].as_f64().unwrap_or(0.0));
                    let (px, py, dx, dy) = (num("pntX"), num("pntY"), num("dirX"), num("dirY"));
                    Some(json!({ "tool": "line", "points": [at(px + t0 * dx, py + t0 * dy), at(px + t1 * dx, py + t1 * dy)], "construction": construction }))
                }
                "BTCurveGeometryCircle" => {
                    let (cx, cy, r, xd, yd) = (num("xCenter"), num("yCenter"), num("radius"), num("xDir"), num("yDir"));
                    self.circles.insert((oid.clone(), eid.clone()), ([cx, cy], r));
                    let full = e["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketchCurve-")) || e["startParam"].is_null();
                    if full {
                        Some(json!({ "tool": "circle", "points": [at(cx, cy), at(cx + r * xd, cy + r * yd)], "construction": construction }))
                    } else {
                        // Arc parameters are angles from xDir, clockwise when flagged.
                        let sense = if g["clockwise"].as_bool() == Some(true) { -1.0 } else { 1.0 };
                        let pt = |t: f64| {
                            let a = sense * t;
                            let (c, s) = (a.cos(), a.sin());
                            at(cx + r * (c * xd - s * yd), cy + r * (c * yd + s * xd))
                        };
                        let (t0, t1) = arc_span(e).unwrap_or((0.0, 0.0));
                        Some(json!({ "tool": "three_point_arc", "points": [pt(t0), pt(t1), pt((t0 + t1) / 2.0)], "construction": construction }))
                    }
                }
                "BTCurveGeometryEllipse" => {
                    let (cx, cy, a, b, xd, yd) = (num("xCenter"), num("yCenter"), g["majorRadius"].as_f64().or(g["radius"].as_f64()).unwrap_or(0.0), num("minorRadius"), num("xDir"), num("yDir"));
                    self.ellipses.insert((oid.clone(), eid.clone()), [cx, cy, xd, yd, a, b]);
                    let sense = if g["clockwise"].as_bool() == Some(true) { -1.0 } else { 1.0 };
                    let pt = |t: f64| {
                        let (c, s) = (t.cos(), sense * t.sin());
                        at(cx + a * c * xd - b * s * yd, cy + a * c * yd + b * s * xd)
                    };
                    match (e["startParam"].as_f64(), e["endParam"].as_f64()) {
                        (Some(t0), Some(t1)) if e["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketchCurveSegment")) => {
                            let (t0, t1) = if sense < 0.0 { (t1, t0) } else { (t0, t1) };
                            Some(json!({ "tool": "elliptical_arc", "points": [at(cx, cy), pt(0.0), pt(t0), pt(t1)], "construction": construction }))
                        }
                        _ => Some(json!({ "tool": "ellipse", "points": [at(cx, cy), pt(0.0), pt(std::f64::consts::FRAC_PI_2)], "construction": construction })),
                    }
                }
                "BTCurveGeometryInterpolatedSpline" => {
                    let flat: Vec<f64> = g["interpolationPoints"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
                    let derivs = ["startDerivativeX", "startDerivativeY", "endDerivativeX", "endDerivativeY"].iter().any(|k| num(k).abs() > 1e-12);
                    if derivs || g["isPeriodic"].as_bool() == Some(true) {
                        problems.push(format!("sketch entity {eid}: spline end derivatives / periodic not translated (fit points only)"));
                    }
                    (flat.len() >= 4).then(|| json!({ "tool": "spline", "points": flat.chunks(2).map(|c| at(c[0], c[1])).collect::<Vec<_>>(), "construction": construction }))
                }
                "BTCurveGeometryControlPointSpline" => {
                    let flat: Vec<f64> = g["controlPoints"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
                    let n = flat.len() / 2;
                    let bezier = g["degree"].as_u64() == Some(n.saturating_sub(1) as u64) && g["isRational"].as_bool() != Some(true);
                    if bezier { Some(json!({ "tool": "bezier", "points": flat.chunks(2).map(|c| at(c[0], c[1])).collect::<Vec<_>>(), "construction": construction })) } else { None }
                }
                _ if e["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketchPoint")) => Some(json!({ "tool": "point", "points": [at(e["x"].as_f64().unwrap_or(0.0), e["y"].as_f64().unwrap_or(0.0))], "construction": construction })),
                _ => None,
            };
            let poly = sampled(e);
            let end_points = poly.as_ref().filter(|p| p.len() >= 2 && !e["startParam"].is_null()).map(|p| [p[0], p[p.len() - 1]]);
            if let Some(poly) = poly {
                self.curves.insert((oid.clone(), eid.clone()), poly);
            }
            match shape {
                Some(s) => {
                    shapes.push(s);
                    local.push(eid);
                    ends.push(end_points);
                }
                None => problems.push(format!("sketch entity {eid}: {} not translated", if gt.is_empty() { e["btType"].as_str().unwrap_or("?") } else { gt })),
            }
        }
        // Curves first, then the free points, so each reply lists ours in
        // Onshape's order.
        let ours = self.call("sketch_get", json!({ "sketch": id }))?;
        let unit = per_unit(&ours["units"]);
        let v = |key: &str| -> [f64; 3] { std::array::from_fn(|i| ours["frame"][key][i].as_f64().unwrap_or(0.0)) };
        self.sketch_map = SketchMap { onshape: frame, ours: [v("origin").map(|c| c * unit), v("x"), v("y")], k: unit };
        let map = self.sketch_map;
        for points in [false, true] {
            let (batch, ids): (Vec<Value>, Vec<String>) = shapes.iter().zip(&local).filter(|(s, _)| (s["tool"] == "point") == points).map(|(s, e)| (s.clone(), e.clone())).unzip();
            let ends_of: HashMap<&String, [[f64; 2]; 2]> = local.iter().zip(&ends).filter_map(|(e, x)| Some((e, (*x)?))).collect();
            if batch.is_empty() {
                continue;
            }
            // One call for them all; if one shape fails, each on its own
            // (the others still drawn, the failing ones reported).
            let replies: Vec<(Vec<String>, Value)> = match self.call("sketch_draw", json!({ "sketch": id, "shapes": batch.clone() })) {
                Ok(r) => vec![(ids, r)],
                Err(_) => {
                    let mut out = Vec::new();
                    for (shape, eid) in batch.iter().zip(&ids) {
                        match self.call("sketch_draw", json!({ "sketch": id, "shapes": [shape] })) {
                            Ok(r) => out.push((vec![eid.clone()], r)),
                            Err(e) => problems.push(format!("sketch entity {eid}: not drawn ({e})")),
                        }
                    }
                    out
                }
            };
            for (ids, r) in &replies {
                let made: Vec<&Value> = r["created"].as_array().into_iter().flatten().filter(|c| (c["type"] == "point") == points).collect();
                for (eid, c) in ids.iter().zip(made) {
                    if let Some(cid) = c["id"].as_str() {
                        self.sketch_entities.insert((oid.clone(), eid.clone()), cid.to_string());
                        if points {
                            self.sketch_points.push((oid.clone(), cid.to_string()));
                        }
                    }
                    // Ours reversed: its start nearer Onshape's end.
                    let at = |key: &str| Some([c[key][0].as_f64()? * map.k, c[key][1].as_f64()? * map.k]);
                    let reversed = match (ends_of.get(eid), at("start")) {
                        (Some([s, e]), Some(p)) => {
                            let d = |q: [f64; 2]| (q[0] - p[0]).hypot(q[1] - p[1]);
                            d(map.local(*e)) < d(map.local(*s)) - 1e-9
                        }
                        _ => false,
                    };
                    let keys = if reversed { [("start", "end_point"), ("end", "start_point"), ("center", "center_point")] } else { [("start", "start_point"), ("end", "end_point"), ("center", "center_point")] };
                    for (end, key) in keys {
                        if let Some(pid) = c[key].as_str() {
                            self.sketch_entities.insert((oid.clone(), format!("{eid}.{end}")), pid.to_string());
                        }
                    }
                }
            }
        }
        // Constraints and dimensions keep the geometry where Onshape solved
        // it; any that move it (a solve that differs) are taken back.
        let before = self.call("sketch_get", json!({ "sketch": id }))?;
        let top = |t: &mut Self| t.ctx.doc(None).ok().and_then(|od| od.session.undo_tops().0);
        let mark = top(self);
        self.sketch_edits = 0;
        self.sketch_before = before.clone();
        self.first_move = None;
        self.constraints(k, &oid, &id, f, problems);
        self.dimensions(k, &oid, &id, f, problems);
        let after = self.call("sketch_get", json!({ "sketch": id }))?;
        let moved = sketch_moved(&before, &after);
        if moved > 1e-7 {
            // Undone back to the edit before them (deleting them would leave
            // the points moved; some calls make no edit, so not by count).
            let mut undone = 0;
            while top(self) != mark && undone <= self.sketch_edits && self.call("history_undo", json!({})).is_ok_and(|u| u["undone"] == true) {
                undone += 1;
            }
            let back = self.call("sketch_get", json!({ "sketch": id })).map(|g| sketch_moved(&before, &g)).unwrap_or(f64::NAN);
            let first = self.first_move.take().unwrap_or_default();
            problems.push(format!("its constraints and dimensions moved the geometry by up to {:.6} mm (first {first}): taken back ({undone} edits undone; {:.6} mm left)", moved * 1e3, back * 1e3));
        }
        let st = self.call("feature_list", json!({}))?;
        let row = st["features"].as_array().into_iter().flatten().find(|x| x["id"] == id).cloned().unwrap_or_default();
        Ok((id, row["status"].as_str().unwrap_or("Ok").to_string(), row["message"].as_str().map(str::to_string)))
    }

    /// The sketch's geometric constraints as ours (the geometry already
    /// satisfies them: they keep it so when variables or configurations
    /// change a dimension). Coincidences our inference merged into one point
    /// are skipped; the rest that fail or have no counterpart are counted.
    fn constraints(&mut self, k: usize, oid: &str, id: &str, f: &Value, problems: &mut Vec<String>) {
        let mut missed: std::collections::BTreeMap<String, usize> = Default::default();
        for c in f["constraints"].as_array().into_iter().flatten() {
            let ty = c["constraintType"].as_str().unwrap_or("");
            let ours = match ty {
                "COINCIDENT" => "coincident",
                "CONCENTRIC" => "concentric",
                "PARALLEL" => "parallel",
                "TANGENT" => "tangent",
                "HORIZONTAL" => "horizontal",
                "VERTICAL" => "vertical",
                "MIDPOINT" => "midpoint",
                "PERPENDICULAR" => "perpendicular",
                "EQUAL" => "equal",
                "SYMMETRIC" | "MIRROR" => "symmetric",
                "FIX" => "fix",
                "NORMAL" => "normal",
                "CURVATURE" => "curvature",
                // Dimensions: `dimensions`.
                "DISTANCE" | "LENGTH" | "DIAMETER" | "RADIUS" | "ANGLE" | "CENTERLINE_DIMENSION" | "MAJOR_DIAMETER" | "MINOR_DIAMETER" => continue,
                _ => {
                    *missed.entry(format!("{ty} (no counterpart)")).or_default() += 1;
                    continue;
                }
            };
            let par = |name: &str| c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).and_then(|p| p["value"].as_str().map(str::to_string)).filter(|v| !v.is_empty());
            // Ours takes the mirror line last, a midpoint's point last (between
            // two points: Onshape's centre-point rectangle).
            let order: &[&str] = match ours {
                "symmetric" => &["localFirst", "localSecond", "localMirror"],
                "midpoint" => &["localFirst", "localEntity1", "localSecond", "localEntity2", "localMidpoint"],
                _ => &["localFirst", "localSecond"],
            };
            let mut entities: Vec<String> = Vec::new();
            let mut external = false;
            for n in order {
                if let Some(v) = par(n) {
                    match self.sketch_entities.get(&(oid.to_string(), v)) {
                        Some(e) => entities.push(e.clone()),
                        None => external = true,
                    }
                }
            }
            for p in c["parameters"].as_array().into_iter().flatten().filter(|p| p["parameterId"].as_str().is_some_and(|n| n.starts_with("external"))) {
                match self.external_ref(k, p) {
                    Some(e) => entities.push(e),
                    None => external = true,
                }
            }
            if external {
                *missed.entry(format!("{ty} to external geometry")).or_default() += 1;
                continue;
            }
            if ours == "coincident" && entities.len() == 2 && entities[0] == entities[1] {
                continue;
            }
            let made = !entities.is_empty() && self.call("sketch_constrain", json!({ "sketch": id, "constraint": ours, "entities": entities })).is_ok();
            self.sketch_edits += usize::from(made);
            if made {
                self.check_moved(id, || format!("{ty} on {}", c["parameters"].as_array().into_iter().flatten().filter_map(|p| p["value"].as_str()).collect::<Vec<_>>().join(", ")));
            }
            if !made {
                *missed.entry(format!("{ty} rejected")).or_default() += 1;
            }
        }
        if !missed.is_empty() {
            problems.push(format!("constraints left to the solved geometry: {}", missed.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")));
        }
    }

    /// A constraint's or dimension's model geometry (`external…`): the
    /// Origin, or the one entity of ours its query maps to (the sketch takes
    /// it in as Use would).
    fn external_ref(&mut self, k: usize, p: &Value) -> Option<String> {
        let queries: Vec<Value> = p["queries"].as_array().cloned().unwrap_or_default();
        let text: String = queries.iter().filter_map(|q| q["queryString"].as_str()).collect();
        // The Origin: its deterministic id is "IB".
        let origin = queries.iter().any(|q| q["deterministicIds"].as_array().is_some_and(|ids| ids.len() == 1 && ids[0] == "IB"));
        if origin || text.contains("Origin") || text.contains("originOp") {
            return Some("origin".into());
        }
        let [q] = queries.as_slice() else { return None };
        match self.query(k, q) {
            Ok(ids) if ids.len() == 1 => ids.into_iter().next(),
            _ => None,
        }
    }

    /// Where an angle dimension's label goes (ours, metres) so it measures
    /// the quadrant Onshape's does: the one whose angle is the value, or
    /// else the one Onshape's label is in (`labelAngle` from the vertex,
    /// behind it when `labelDistance` is negative).
    fn angle_label(&self, oid: &str, c: &Value, value: Option<&str>) -> Option<[f64; 2]> {
        let par = |name: &str| c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name);
        let line = |name: &str| -> Option<([f64; 2], [f64; 2])> {
            let p = self.curves.get(&(oid.to_string(), par(name)?["value"].as_str()?.to_string()))?;
            let (a, b) = (p.first()?, p.last()?);
            let l = (b[0] - a[0]).hypot(b[1] - a[1]);
            (l > 1e-12).then(|| (*a, [(b[0] - a[0]) / l, (b[1] - a[1]) / l]))
        };
        let ((pa, da), (pb, db)) = (line("localFirst")?, line("localSecond")?);
        let den = da[0] * db[1] - da[1] * db[0];
        if den.abs() < 1e-9 {
            return None;
        }
        let t = ((pb[0] - pa[0]) * db[1] - (pb[1] - pa[1]) * db[0]) / den;
        let x = [pa[0] + t * da[0], pa[1] + t * da[1]];
        let between = (da[0] * db[0] + da[1] * db[1]).clamp(-1.0, 1.0).acos();
        let prefs = noshape_units::UnitPrefs::default();
        let want = value.and_then(|v| noshape_units::evaluate_as(v, noshape_units::Dim::ANGLE, &prefs, &noshape_units::EmptyScope).ok()).map(|q| q.value);
        let num = |name: &str| par(name).and_then(|p| p["expression"].as_str()).and_then(|e| e.split('*').next()?.trim().parse::<f64>().ok());
        let dir = match want {
            // Between da and db the angle is `between`; between da and −db its supplement.
            Some(w) if (w - between).abs() < 1e-6 => [da[0] + db[0], da[1] + db[1]],
            Some(w) if (w - (std::f64::consts::PI - between)).abs() < 1e-6 => [da[0] - db[0], da[1] - db[1]],
            _ => {
                let (a, d) = (num("labelAngle")?, num("labelDistance")?);
                [d * a.cos(), d * a.sin()]
            }
        };
        let n = dir[0].hypot(dir[1]);
        if n < 1e-12 {
            return None;
        }
        Some(self.sketch_map.local([x[0] + 0.01 * dir[0] / n, x[1] + 0.01 * dir[1] / n]))
    }

    /// A distance to a circle or arc measures to its centre, its near side
    /// or its far side: the one whose length is the value Onshape solved
    /// (its `halfSpace` sides aren't read).
    fn circle_mode(&self, oid: &str, c: &Value, value: Option<&str>) -> Option<&'static str> {
        let par = |name: &str| c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).and_then(|p| p["value"].as_str()).map(str::to_string);
        let want = noshape_units::evaluate_as(value?, noshape_units::Dim::LENGTH, &noshape_units::UnitPrefs::default(), &noshape_units::EmptyScope).ok()?.value;
        enum G {
            Point([f64; 2], f64),
            Line([f64; 2], [f64; 2]),
        }
        let geom = |e: String| -> Option<G> {
            if let Some((c, r)) = self.circles.get(&(oid.to_string(), e.clone())) {
                return Some(G::Point(*c, *r));
            }
            if let Some(p) = self.curves.get(&(oid.to_string(), e.clone())) {
                return Some(G::Line(*p.first()?, *p.last()?));
            }
            let (base, end) = e.rsplit_once('.')?;
            let key = (oid.to_string(), base.to_string());
            match end {
                "center" => self.circles.get(&key).map(|(c, _)| G::Point(*c, 0.0)),
                "start" => self.curves.get(&key).and_then(|p| p.first()).map(|p| G::Point(*p, 0.0)),
                "end" => self.curves.get(&key).and_then(|p| p.last()).map(|p| G::Point(*p, 0.0)),
                _ => None,
            }
        };
        let (a, b) = (geom(par("localFirst")?)?, geom(par("localSecond")?)?);
        let to_line = |p: [f64; 2], s: [f64; 2], e: [f64; 2]| {
            let (dx, dy) = (e[0] - s[0], e[1] - s[1]);
            ((p[0] - s[0]) * dy - (p[1] - s[1]) * dx).abs() / dx.hypot(dy).max(1e-12)
        };
        let (centres, radii) = match (a, b) {
            (G::Point(p, r), G::Point(q, t)) => ((p[0] - q[0]).hypot(p[1] - q[1]), r + t),
            (G::Point(p, r), G::Line(s, e)) | (G::Line(s, e), G::Point(p, r)) => (to_line(p, s, e), r),
            (G::Line(..), G::Line(..)) => return None,
        };
        if radii < 1e-12 {
            return None;
        }
        [("center", centres), ("minimum", centres - radii), ("maximum", centres + radii)].into_iter().min_by(|x, y| (x.1 - want).abs().total_cmp(&(y.1 - want).abs())).map(|m| m.0)
    }

    /// `edges` and the line edges on the same lines (the pieces a cut
    /// split them into).
    fn with_collinear_pieces(&mut self, edges: Vec<String>) -> Vec<String> {
        let Ok(all) = self.call("query_entities", json!({ "kind": "edges", "curve": "line", "limit": 100000 })) else { return edges };
        let all: Vec<&Value> = all["edges"].as_array().into_iter().flatten().collect();
        let v = |e: &Value, k: &str| -> Option<[f64; 3]> { Some([e[k][0].as_f64()?, e[k][1].as_f64()?, e[k][2].as_f64()?]) };
        let mut out = edges.clone();
        for id in &edges {
            let Some(line) = all.iter().find(|e| e["id"] == id.as_str()) else { continue };
            let (Some(p), Some(d)) = (v(line, "start"), v(line, "direction")) else { continue };
            for e in &all {
                let (Some(q), Some(f), Some(eid)) = (v(e, "start"), v(e, "direction"), e["id"].as_str()) else { continue };
                let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
                let norm = |a: [f64; 3]| a.iter().map(|c| c * c).sum::<f64>().sqrt();
                let off = cross([q[0] - p[0], q[1] - p[1], q[2] - p[2]], d);
                if norm(cross(d, f)) < 1e-9 && norm(off) < 1e-6 && !out.iter().any(|o| o == eid) {
                    out.push(eid.to_string());
                }
            }
        }
        out
    }

    /// The circle side (centre, near, far) of a distance dimension that
    /// leaves the sketch where it is; each tried one is undone.
    fn resting_mode(&mut self, id: &str, args: &Value) -> &'static str {
        let top = |t: &mut Self| t.ctx.doc(None).ok().and_then(|od| od.session.undo_tops().0);
        let Ok(before) = self.call("sketch_get", json!({ "sketch": id })) else { return "center" };
        for mode in ["center", "minimum", "maximum"] {
            let mark = top(self);
            let mut a = args.clone();
            a["mode"] = json!(mode);
            if self.call("sketch_dimension", a).is_err() {
                continue;
            }
            let moved = self.call("sketch_get", json!({ "sketch": id })).map(|g| sketch_moved(&before, &g)).unwrap_or(f64::INFINITY);
            for _ in 0..4 {
                if top(self) == mark || self.call("history_undo", json!({})).is_err() {
                    break;
                }
            }
            if moved <= 1e-7 {
                return mode;
            }
        }
        "center"
    }

    /// Notes the first constraint or dimension that moved the sketch.
    fn check_moved(&mut self, id: &str, what: impl FnOnce() -> String) {
        if self.first_move.is_some() {
            return;
        }
        let Ok(now) = self.call("sketch_get", json!({ "sketch": id })) else { return };
        let d = sketch_moved(&self.sketch_before, &now);
        if d > 1e-7 {
            self.first_move = Some(format!("{} by {:.6} mm", what(), d * 1e3));
        }
    }

    /// The sketch's dimensions with Onshape's expressions (so variables and
    /// configurations drive them).
    fn dimensions(&mut self, k: usize, oid: &str, id: &str, f: &Value, problems: &mut Vec<String>) {
        let mut external: std::collections::BTreeMap<String, usize> = Default::default();
        for c in f["constraints"].as_array().into_iter().flatten() {
            let ty = c["constraintType"].as_str().unwrap_or("");
            let par0 = |name: &str| c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).and_then(|p| p["value"].as_str().map(str::to_string));
            let kind = match ty {
                "DISTANCE" => match par0("direction").as_deref() {
                    Some("HORIZONTAL") => "horizontal_distance",
                    Some("VERTICAL") => "vertical_distance",
                    _ => "distance",
                },
                "CENTERLINE_DIMENSION" => "centerline",
                "LENGTH" => "length",
                "DIAMETER" => "diameter",
                "RADIUS" => "radius",
                // An ellipse's axis: ours by where the label is.
                "MAJOR_DIAMETER" | "MINOR_DIAMETER" => "diameter",
                "ANGLE" => "angle",
                _ => continue,
            };
            let par = |name: &str| c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).cloned();
            let value = ["length", "angle"].iter().find_map(|n| par(n)).and_then(|p| p["expression"].as_str().map(str::to_string));
            let mut entities = Vec::new();
            let mut missing = false;
            for n in ["localFirst", "externalFirst", "localSecond", "externalSecond", "localEntity1", "localEntity2"] {
                if n.starts_with("external") {
                    let Some(ext) = par(n) else { continue };
                    match self.external_ref(k, &ext) {
                        Some(e) => entities.push(e),
                        None => missing = true,
                    }
                    continue;
                }
                if let Some(v) = par(n).and_then(|p| p["value"].as_str().map(str::to_string)).filter(|v| !v.is_empty()) {
                    match self.sketch_entities.get(&(oid.to_string(), v.clone())) {
                        Some(e) => entities.push(e.clone()),
                        None => problems.push(format!("{ty} dimension: sketch entity {v} not translated")),
                    }
                }
            }
            // To model geometry: left to the solved geometry, as constraints are.
            if missing {
                *external.entry(ty.to_string()).or_default() += 1;
                continue;
            }
            if entities.is_empty() {
                continue;
            }
            let mut args = json!({ "sketch": id, "entities": entities, "kind": kind });
            if kind == "distance"
                && let Some(mode) = self.circle_mode(oid, c, value.as_deref())
            {
                args["mode"] = json!(mode);
            }
            if matches!(ty, "MAJOR_DIAMETER" | "MINOR_DIAMETER")
                && let Some([cx, cy, xd, yd, a, b]) = par0("localFirst").and_then(|e| self.ellipses.get(&(oid.to_string(), e)).copied())
            {
                // Beyond the axis' end.
                let (d, r) = if ty == "MAJOR_DIAMETER" { ([xd, yd], a) } else { ([-yd, xd], b) };
                let l = self.sketch_map.local([cx + d[0] * (r + 0.005), cy + d[1] * (r + 0.005)]);
                args["label"] = json!(l.map(|x| format!("{} mm", x * 1e3)));
            }
            if kind == "angle"
                && let Some(l) = self.angle_label(oid, c, value.as_deref())
            {
                args["label"] = json!(l.map(|x| format!("{} mm", x * 1e3)));
            }
            if let Some(v) = value {
                args["value"] = json!(v);
            }
            let driven = c["parameters"].as_array().into_iter().flatten().any(|p| p["parameterId"] == "driven" && p["value"] == true);
            // A distance to a circle whose side isn't known from Onshape's
            // geometry (the other end is model geometry): the side that
            // keeps the sketch where it is.
            let circle = ["localFirst", "localSecond"].iter().filter_map(|n| par0(n)).any(|e| self.circles.contains_key(&(oid.to_string(), e)));
            if kind == "distance" && circle && args.get("mode").is_none() {
                args["mode"] = json!(self.resting_mode(id, &args));
            }
            match self.call("sketch_dimension", args) {
                Ok(r) => {
                    self.sketch_edits += 1;
                    // Onshape's driven (reference) dimension; and a driving one
                    // that over-defines the sketch stays driving, as stored
                    // (ours added it driven, as the Dimension tool does).
                    let over = r.get("warning").is_some();
                    if driven != over
                        && let Some(d) = r["dimension"].as_str()
                        && self.call("sketch_dimension_options", json!({ "sketch": id, "dimension": d, "driving": !driven })).is_ok()
                    {
                        self.sketch_edits += 1;
                    }
                    self.check_moved(id, || format!("{ty} {kind} {} on {}", c["parameters"].as_array().into_iter().flatten().find_map(|p| p["expression"].as_str()).unwrap_or(""), ["localFirst", "localSecond", "localEntity1", "localEntity2"].iter().filter_map(|n| par0(n)).filter(|v| !v.is_empty()).collect::<Vec<_>>().join(", ")));
                }
                Err(e) => problems.push(format!("{ty} dimension on {}: {e}", ["localFirst", "localSecond", "localEntity1", "localEntity2"].iter().filter_map(|n| par0(n)).filter(|v| !v.is_empty()).collect::<Vec<_>>().join(", "))),
            }
        }
        if !external.is_empty() {
            problems.push(format!("dimensions left to the solved geometry: {}", external.iter().map(|(k, n)| format!("{n} {k} to external geometry")).collect::<Vec<_>>().join(", ")));
        }
    }
}

/// An Onshape sketch's frame and ours: Onshape's sketch (x, y) in metres
/// to ours in metres (the same when Onshape's frame isn't known).
#[derive(Clone, Copy, Default)]
struct SketchMap {
    onshape: Option<[[f64; 3]; 3]>,
    ours: [[f64; 3]; 3],
    /// Metres per unit of the sketch ops' replies.
    k: f64,
}

impl SketchMap {
    fn local(&self, [u, w]: [f64; 2]) -> [f64; 2] {
        let Some([fo, fx, fy]) = self.onshape else { return [u, w] };
        let [o, x, y] = self.ours;
        let p: [f64; 3] = std::array::from_fn(|i| fo[i] + u * fx[i] + w * fy[i] - o[i]);
        [(0..3).map(|i| p[i] * x[i]).sum(), (0..3).map(|i| p[i] * y[i]).sum()]
    }
}

/// How far (m) any sketch point moved between two `sketch_get` replies.
fn sketch_moved(before: &Value, after: &Value) -> f64 {
    let points = |g: &Value| -> HashMap<String, Vec<f64>> {
        let k = per_unit(&g["units"]);
        g["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let coords: Vec<f64> = ["at", "start", "end", "center"].iter().filter_map(|key| e[*key].as_array()).flatten().filter_map(Value::as_f64).map(|c| c * k).collect();
                Some((e["id"].as_str()?.to_string(), coords))
            })
            .collect()
    };
    let (b, a) = (points(before), points(after));
    b.iter().filter_map(|(id, p)| a.get(id).map(|q| p.iter().zip(q).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max))).fold(0.0, f64::max)
}

/// The strings of a query's packed form (`qCompressed(1.0, "…", id)`), if any.
fn packed_strings_of(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(packed) = text.split("qCompressed(1.0,\"").nth(1).and_then(|r| r.split('"').next())
        && let Ok(tree) = noshape_model::custom::host::packed::unpack(packed)
    {
        packed_strings(&tree, &mut out);
    }
    out
}

/// Every string in a packed query.
fn packed_strings(p: &noshape_model::custom::host::packed::Packed, out: &mut Vec<String>) {
    use noshape_model::custom::host::packed::Packed;
    match p {
        Packed::Str(s) => out.push(s.clone()),
        Packed::Map(m) => m.iter().for_each(|(k, v)| {
            packed_strings(k, out);
            packed_strings(v, out);
        }),
        Packed::Array(a) => a.iter().for_each(|v| packed_strings(v, out)),
        Packed::Tagged(_, v) => packed_strings(v, out),
        Packed::Num(_) | Packed::Bool(_) => {}
    }
}

/// An arc's parameters, the end past the start: Onshape's may wrap (an arc
/// from π to 0 runs through 3π/2).
fn arc_span(e: &Value) -> Option<(f64, f64)> {
    let (a, b) = (e["startParam"].as_f64()?, e["endParam"].as_f64()?);
    let tau = std::f64::consts::TAU;
    Some((a, if b < a { b + tau * ((a - b) / tau).ceil() } else { b }))
}

/// An Onshape sketch curve as a polyline in its sketch plane (metres).
fn sampled(e: &Value) -> Option<Vec<[f64; 2]>> {
    let g = &e["geometry"];
    let num = |k: &str| g[k].as_f64().unwrap_or(0.0);
    match g["btType"].as_str()?.split('-').next()? {
        "BTCurveGeometryLine" => {
            let (t0, t1) = (e["startParam"].as_f64()?, e["endParam"].as_f64()?);
            let (px, py, dx, dy) = (num("pntX"), num("pntY"), num("dirX"), num("dirY"));
            Some(vec![[px + t0 * dx, py + t0 * dy], [px + t1 * dx, py + t1 * dy]])
        }
        "BTCurveGeometryCircle" => {
            let (cx, cy, r, xd, yd) = (num("xCenter"), num("yCenter"), num("radius"), num("xDir"), num("yDir"));
            let sense = if g["clockwise"].as_bool() == Some(true) { -1.0 } else { 1.0 };
            let (t0, t1) = arc_span(e).unwrap_or((0.0, std::f64::consts::TAU));
            Some(
                (0..=64)
                    .map(|i| {
                        let a = sense * (t0 + (t1 - t0) * i as f64 / 64.0);
                        let (c, s) = (a.cos(), a.sin());
                        [cx + r * (c * xd - s * yd), cy + r * (c * yd + s * xd)]
                    })
                    .collect(),
            )
        }
        "BTCurveGeometryEllipse" => {
            let (cx, cy, a, b, xd, yd) = (num("xCenter"), num("yCenter"), g["majorRadius"].as_f64().or(g["radius"].as_f64()).unwrap_or(0.0), num("minorRadius"), num("xDir"), num("yDir"));
            let sense = if g["clockwise"].as_bool() == Some(true) { -1.0 } else { 1.0 };
            let (t0, t1) = arc_span(e).unwrap_or((0.0, std::f64::consts::TAU));
            Some(
                (0..=64)
                    .map(|i| {
                        let t = t0 + (t1 - t0) * i as f64 / 64.0;
                        let (c, s) = (t.cos(), sense * t.sin());
                        [cx + a * c * xd - b * s * yd, cy + a * c * yd + b * s * xd]
                    })
                    .collect(),
            )
        }
        // Splines through their fit or control points: enough to tell which
        // side of them a region lies.
        "BTCurveGeometryInterpolatedSpline" | "BTCurveGeometryControlPointSpline" => {
            let key = if g["interpolationPoints"].is_array() { "interpolationPoints" } else { "controlPoints" };
            let flat: Vec<f64> = g[key].as_array()?.iter().filter_map(Value::as_f64).collect();
            (flat.len() >= 4).then(|| flat.chunks(2).map(|c| [c[0], c[1]]).collect())
        }
        _ => None,
    }
}

/// Onshape's surface / curve type names in query_entities' words.
fn our_type(t: Option<&str>) -> Option<String> {
    let t = t?.to_lowercase();
    ["plane", "cylinder", "cone", "sphere", "torus", "line", "circle", "ellipse"].contains(&t.as_str()).then_some(t)
}

/// Live's referenced geometry from the raw `bodydetails` bodies a
/// live.jsonl line carries (`bodies`): line k describes the state feature
/// k + 1 references. Faces get a point on their surface near their box
/// centre (planes: the projection; others: the box centre), edges their
/// midpoint, vertices their point.
pub fn refs_from_bodies(lines: &[Value]) -> HashMap<(usize, String), LiveRef> {
    let v3 = |v: &Value| Some([v["x"].as_f64()?, v["y"].as_f64()?, v["z"].as_f64()?]);
    let mut out = HashMap::new();
    for line in lines {
        let (Some(k), Some(bodies)) = (line["index"].as_u64(), line["bodies"].as_array()) else { continue };
        let at = k as usize + 1;
        for b in bodies {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for f in b["faces"].as_array().into_iter().flatten() {
                let (Some(mn), Some(mx)) = (v3(&f["box"]["minCorner"]), v3(&f["box"]["maxCorner"])) else { continue };
                for i in 0..3 {
                    lo[i] = lo[i].min(mn[i]);
                    hi[i] = hi[i].max(mx[i]);
                }
                let c = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
                let s = &f["surface"];
                let mut r = LiveRef { kind: "FACE".into(), point: Some(c), measure: f["area"].as_f64(), geometry: our_type(s["type"].as_str()), ..Default::default() };
                if s["type"] == "PLANE"
                    && let (Some(o), Some(n)) = (v3(&s["origin"]), v3(&s["normal"]))
                {
                    let d: f64 = (0..3).map(|i| (c[i] - o[i]) * n[i]).sum();
                    r.point = Some([c[0] - d * n[0], c[1] - d * n[1], c[2] - d * n[2]]);
                    let flip = if f["orientation"].as_bool() == Some(true) { 1.0 } else { -1.0 } * if s["isOrientedWithFace"].as_bool() == Some(false) { -1.0 } else { 1.0 };
                    r.normal = Some(n.map(|x| x * flip));
                }
                if let Some(id) = f["id"].as_str() {
                    out.insert((at, id.to_string()), r);
                }
            }
            for e in b["edges"].as_array().into_iter().flatten() {
                if let (Some(id), Some(p)) = (e["id"].as_str(), v3(&e["geometry"]["midPoint"])) {
                    out.insert((at, id.to_string()), LiveRef { kind: "EDGE".into(), point: Some(p), measure: e["geometry"]["length"].as_f64(), geometry: our_type(e["curve"]["type"].as_str()), ..Default::default() });
                }
            }
            for v in b["vertices"].as_array().into_iter().flatten() {
                if let (Some(id), Some(p)) = (v["id"].as_str(), v3(&v["point"])) {
                    out.insert((at, id.to_string()), LiveRef { kind: "VERTEX".into(), point: Some(p), ..Default::default() });
                }
            }
            if let Some(id) = b["id"].as_str()
                && lo[0].is_finite()
            {
                out.insert((at, id.to_string()), LiveRef { kind: "BODY".into(), point: Some([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0]), ..Default::default() });
            }
        }
    }
    out
}

/// Onshape's own bookkeeping, not dialog parameters: the FeatureScript
/// version a feature was made with, the Fillet dialog's "defaults changed"
/// flag, a pattern's internal switch.
const INTERNAL: [&str; 3] = ["asVersion", "defaultsChanged", "computeTransformsWithoutBuiltin"];

/// Older features' parameters as today's std (and ours) has them, as
/// Onshape upgrades them: Revolve's `revolveType`, Extrude's `SYMMETRIC`
/// end, a pre-V2 Hole's, a Variable's `value`.
fn upgrade(ty: &str, params: &mut Map<String, Value>, unknown: &mut Vec<String>) {
    let mut done = |id: &str, params: &mut Map<String, Value>| {
        unknown.retain(|u| u != id);
        params.remove(id)
    };
    match ty {
        "revolve" => {
            if let Some(t) = done("revolveType", params) {
                let t = t.as_str().unwrap_or("FULL");
                params.insert("fullRevolve".into(), json!(t == "FULL"));
                params.insert("symmetric".into(), json!(t == "SYMMETRIC"));
                if t == "TWO_DIRECTIONS" {
                    params.insert("hasStartBound".into(), json!(true));
                }
            }
        }
        // Older Extrudes: Symmetric was an end type.
        "extrude" => {
            if params.get("endBound").is_some_and(|b| b == "SYMMETRIC") {
                params.insert("endBound".into(), json!("BLIND"));
                params.insert("symmetric".into(), json!(true));
            }
        }
        // Holes before V2 (2019): unsuffixed names, the start as a flag.
        "hole" => {
            for (old, new) in [("endStyle", "endStyleV2"), ("holeDiameter", "holeDiameterV3"), ("cBoreDiameter", "cBoreDiameterV3"), ("cBoreDepth", "cBoreDepthV3"), ("cSinkDiameter", "cSinkDiameterV3"), ("cSinkAngle", "cSinkAngleV3"), ("tapDrillDiameter", "tapDrillDiameterV3"), ("holeDepth", "holeDepthV3"), ("tappedDepth", "tappedDepthV3")] {
                if !params.contains_key(new)
                    && let Some(v) = done(old, params)
                {
                    params.insert(new.into(), v);
                }
            }
            if !params.contains_key("styleV2")
                && let Some(v) = params.get("style").cloned()
            {
                params.insert("styleV2".into(), v);
            }
            done("standardBlindInLast", params);
            if !params.contains_key("isV2") {
                params.insert("isV2".into(), json!(false));
            }
            if let Some(v) = done("startFromSketch", params) {
                params.insert("startStyle".into(), json!(if v == true { "SKETCH" } else { "PART" }));
            }
        }
        "assignVariable" => {
            if let Some(v) = done("value", params) {
                let key = match params.get("variableType").and_then(Value::as_str) {
                    Some("ANGLE") => "angleValue",
                    Some("NUMBER") => "numberValue",
                    Some("ANY") => "anyValue",
                    _ => "lengthValue",
                };
                params.entry(key).or_insert(v);
            }
        }
        _ => {}
    }
    for u in unknown.iter() {
        params.remove(u);
    }
}

/// Onshape's default planes in a query string (`dummyQuery(id + "Top.planeOp"…)`,
/// the compressed form's `TopplaneOp`).
fn default_plane(q: &str) -> Option<&'static str> {
    [("Top", "plane:top"), ("Front", "plane:front"), ("Right", "plane:right")].into_iter().find(|(n, _)| q.contains(&format!("{n}.planeOp")) || q.contains(&format!("{n}planeOp"))).map(|(_, p)| p)
}

/// One part's measures, SI (m, m², m³), from live.jsonl or ours.
#[derive(Clone, Debug, Default)]
pub struct PartState {
    pub name: String,
    pub body_type: String,
    pub faces: u64,
    pub edges: u64,
    pub vertices: u64,
    pub volume: Option<f64>,
    pub area: Option<f64>,
    pub volume_band: Option<[f64; 2]>,
    pub area_band: Option<[f64; 2]>,
    pub bbox: Option<[[f64; 3]; 2]>,
    pub centroid: Option<[f64; 3]>,
    /// Live's centroid band ([min, max] per axis), when given.
    pub centroid_band: Option<[[f64; 3]; 2]>,
    /// Faces and edges by geometry (plane, cylinder…; line, circle…; the
    /// rest "other"), when known (live: its bodydetails).
    pub face_types: Option<std::collections::BTreeMap<String, usize>>,
    pub edge_types: Option<std::collections::BTreeMap<String, usize>>,
}

/// A face's or edge's geometry as the parity suite counts it (Onshape's
/// type names or ours): plane, cylinder, cone, sphere, torus; line, circle,
/// ellipse; else other.
fn geometry_kind(t: &str) -> String {
    let t = t.to_lowercase();
    if ["plane", "cylinder", "cone", "sphere", "torus", "line", "circle", "ellipse"].contains(&t.as_str()) { t } else { "other".into() }
}

/// Live's parts of one live.jsonl line.
pub fn live_parts(line: &Value) -> Vec<PartState> {
    let band = |v: &Value| v.as_array().and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
    // Its bodydetails body (by part id): faces and edges by geometry.
    let types = |p: &Value, list: &str, field: &str| {
        let body = line["bodies"].as_array()?.iter().find(|b| b["id"] == p["partId"])?;
        let mut m = std::collections::BTreeMap::new();
        for x in body[list].as_array()? {
            *m.entry(geometry_kind(x[field]["type"].as_str().unwrap_or(""))).or_default() += 1;
        }
        Some(m)
    };
    line["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| PartState {
            name: p["name"].as_str().unwrap_or("").to_string(),
            body_type: p["bodyType"].as_str().unwrap_or("").to_lowercase(),
            faces: p["faces"].as_u64().unwrap_or(0),
            edges: p["edges"].as_u64().unwrap_or(0),
            vertices: p["vertices"].as_u64().unwrap_or(0),
            volume: p["volume"].as_f64(),
            area: p["area"].as_f64(),
            volume_band: band(&p["volume_band"]),
            area_band: band(&p["area_band"]),
            bbox: f3(&p["bbox"]["min"]).zip(f3(&p["bbox"]["max"])).map(|(a, b)| [a, b]),
            centroid: f3(&p["centroid"]),
            centroid_band: p["centroid_band"].as_array().and_then(|b| Some([f3(b.first()?)?, f3(b.get(1)?)?])),
            face_types: types(p, "faces", "surface"),
            edge_types: types(p, "edges", "curve"),
        })
        .collect()
}

/// Our parts now, SI.
pub fn our_parts(t: &mut Translator) -> Result<Vec<PartState>, String> {
    let r = t.call("query_parts", json!({}))?;
    // Faces and edges by geometry, per part.
    let mut types = |list: &str, field: &str| -> HashMap<String, std::collections::BTreeMap<String, usize>> {
        let mut out: HashMap<String, std::collections::BTreeMap<String, usize>> = HashMap::new();
        if let Ok(e) = t.call("query_entities", json!({ "kind": list, "limit": 100000 })) {
            for x in e[list].as_array().into_iter().flatten() {
                *out.entry(x["part"].as_str().unwrap_or("").to_string()).or_default().entry(geometry_kind(x[field].as_str().unwrap_or(""))).or_default() += 1;
            }
        }
        out
    };
    let (faces, edges) = (types("faces", "surface"), types("edges", "curve"));
    let k = per_unit(&r["units"]);
    let at = |v: &Value| f3(v).map(|p| p.map(|c| c * k));
    Ok(r["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| PartState {
            name: p["name"].as_str().unwrap_or("").to_string(),
            body_type: match p["kind"].as_str().unwrap_or("") {
                "Solid" => "solid",
                "Surface" => "sheet",
                "Curve" | "Wire" => "wire",
                o => o,
            }
            .to_string(),
            faces: p["faces"].as_u64().unwrap_or(0),
            edges: p["edges"].as_u64().unwrap_or(0),
            vertices: p["vertices"].as_u64().unwrap_or(0),
            volume: p["volume"].as_f64().map(|v| v * k * k * k),
            area: p["surface_area"].as_f64().map(|a| a * k * k),
            volume_band: None,
            area_band: None,
            bbox: at(&p["bounding_box"]["min"]).zip(at(&p["bounding_box"]["max"])).map(|(a, b)| [a, b]),
            centroid: at(&p["centroid"]),
            centroid_band: None,
            face_types: p["id"].as_str().map(|id| faces.get(id).cloned().unwrap_or_default()),
            edge_types: p["id"].as_str().map(|id| edges.get(id).cloned().unwrap_or_default()),
        })
        .collect())
}

/// How ours differs from live after a feature (empty: it matches). Parts
/// are paired by volume and box; counts exact, volume and area within live's
/// band or 1e-6 relative, centroids and boxes within 1e-6 m.
pub fn differences(live: &[PartState], ours: &[PartState]) -> Vec<String> {
    let mut out = Vec::new();
    if live.len() != ours.len() {
        out.push(format!("{} parts, live {}", ours.len(), live.len()));
    }
    let key = |p: &PartState| (p.volume.unwrap_or(0.0), p.bbox.map(|b| b[0][0] + b[0][1] + b[0][2]).unwrap_or(0.0));
    let mut free: Vec<&PartState> = ours.iter().collect();
    for l in live {
        let Some(i) = (0..free.len()).min_by(|a, b| {
            let d = |p: &PartState| ((key(p).0 - key(l).0).abs() / key(l).0.abs().max(1e-12), (key(p).1 - key(l).1).abs());
            d(free[*a]).partial_cmp(&d(free[*b])).unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            continue;
        };
        let o = free.remove(i);
        let who = if l.name.is_empty() { "a part".to_string() } else { format!("\"{}\"", l.name) };
        if !l.body_type.is_empty() && o.body_type != l.body_type {
            out.push(format!("{who}: a {} body, live {}", o.body_type, l.body_type));
        }
        if (o.faces, o.edges, o.vertices) != (l.faces, l.edges, l.vertices) {
            out.push(format!("{who}: {}/{}/{} faces/edges/vertices, live {}/{}/{}", o.faces, o.edges, o.vertices, l.faces, l.edges, l.vertices));
        } else {
            // The same counts: the same kinds of faces and edges.
            let show = |m: &std::collections::BTreeMap<String, usize>| m.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ");
            if let (Some(a), Some(b)) = (&o.face_types, &l.face_types)
                && a != b
            {
                out.push(format!("{who}: faces {}, live {}", show(a), show(b)));
            }
            if let (Some(a), Some(b)) = (&o.edge_types, &l.edge_types)
                && a != b
            {
                out.push(format!("{who}: edges {}, live {}", show(a), show(b)));
            }
        }
        // Only what live gives is compared.
        let within = |ours: Option<f64>, live: f64, band: Option<[f64; 2]>| {
            let Some(ours) = ours else { return false };
            let tol = 1e-6 * live.abs().max(1e-12);
            band.is_some_and(|[lo, hi]| ours >= lo - tol && ours <= hi + tol) || (ours - live).abs() <= tol
        };
        if let Some(v) = l.volume
            && !within(o.volume, v, l.volume_band)
        {
            out.push(format!("{who}: volume {:.6} mm³, live {:.6}", o.volume.unwrap_or(f64::NAN) * 1e9, v * 1e9));
        }
        if let Some(a) = l.area
            && !within(o.area, a, l.area_band)
        {
            out.push(format!("{who}: area {:.6} mm², live {:.6}", o.area.unwrap_or(f64::NAN) * 1e6, a * 1e6));
        }
        if let (Some(a), Some(b)) = (o.centroid, l.centroid)
            && (0..3).any(|j| (a[j] - b[j]).abs() > 1e-6 && !l.centroid_band.is_some_and(|[lo, hi]| a[j] >= lo[j] - 1e-9 && a[j] <= hi[j] + 1e-9))
        {
            out.push(format!("{who}: centroid {:?} mm, live {:?}", a.map(|c| c * 1e3), b.map(|c| c * 1e3)));
        }
        if let (Some(a), Some(b)) = (o.bbox, l.bbox)
            && (0..2).any(|i| (0..3).any(|j| (a[i][j] - b[i][j]).abs() > 1e-6))
        {
            out.push(format!("{who}: box {:?} mm, live {:?}", a.map(|p| p.map(|c| c * 1e3)), b.map(|p| p.map(|c| c * 1e3))));
        }
    }
    out
}
