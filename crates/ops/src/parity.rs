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
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// How a translated selection names an in-feature mate connector until its feature is added.
const SUB_FEATURE: &str = "subfeature:";

/// A sketch entity live made with Use (PROJECTED): our model reference for it, the shape to draw instead, live's
/// ends (a curve) or place (a point).
struct Use {
    eid: String,
    reference: String,
    shape: Value,
    ends: Option<[[f64; 2]; 2]>,
    point: Option<[f64; 2]>,
}

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
    /// A body's box (min, max).
    pub bbox: Option<[[f64; 3]; 2]>,
    /// Of a sketch's wire body or regions (live's `"type": "SKETCH"` bodies): a query's text, when there, places it
    /// better (the sketch entity it names), so it goes first.
    pub sketch: bool,
    /// The live part a face, edge or vertex is on (two parts can have one there: cf-fill-pattern's sheet metal
    /// part and the solid both have the hole its Fill pattern patterns).
    pub body: Option<String>,
    /// The feature parameter live evaluated it for (refs.jsonl `parameter`): a query live's fetch gave no ids for
    /// (Flange 6's edges in c2-sm-louver) is these.
    pub parameter: Option<String>,
    /// Its point is its plane's origin (live's evPlane, refs.jsonl `origin`), not a point on the face.
    pub plane_origin: bool,
    /// Of live's sketch bodies file (`sketches.json`, live 2267): a sketch region as its sketch made it (a per-feature
    /// line's may be a piece a later feature imprinted).
    pub sketch_file: bool,
    /// Live's sketch feature whose region it is (the sketch bodies file).
    pub sketch_of: Option<String>,
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
    /// A custom feature (Onshape's `namespace` names its Feature Studio).
    pub custom: bool,
    /// A sketch whose frame (origin, x, normal) isn't live's.
    pub frame: Option<String>,
}

pub struct Translator<'a> {
    ctx: &'a mut Ctx,
    features: Vec<Value>,
    refs: HashMap<(usize, String), LiveRef>,
    /// Onshape feature id → ours.
    feature_ids: HashMap<String, String>,
    /// In-feature mate connectors translated (live's sub-feature id → our parameters), placed in their feature.
    sub_params: HashMap<String, Map<String, Value>>,
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
    /// Construction curves among them.
    construction: std::collections::HashSet<(String, String)>,
    /// Configuration inputs' default options (`configuration` of features.json).
    config_defaults: HashMap<String, String>,
    /// Edits the current sketch's constraints and dimensions made (to undo).
    sketch_edits: usize,
    /// A spline's end handle (Onshape's `<spline>.startHandle` / `.endHandle` construction lines): our spline's end point
    /// and handle tip.
    handle_lines: HashMap<(String, String), [String; 2]>,
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
    /// The Part Studio tab translated into (`open_in`); calls that take a
    /// `tab` get it.
    tab: Option<String>,
    /// Part Studios Derived features take parts from (features.json `derivedSources`: element id → `{ doc, live }`),
    /// and those built: element id → (our tab id, live's part id → ours).
    derived_sources: Map<String, Value>,
    derived_built: HashMap<String, (String, HashMap<String, String>)>,
    first_move: Option<String>,
    /// Onshape Feature Studio element id → our Feature Studio tab id.
    studios: HashMap<String, String>,
    /// What isn't translated of the document itself (configuration inputs…).
    pub notes: Vec<String>,
    /// Custom features' failures, apart from geometry: a Feature Studio
    /// that doesn't compile ("<studio>: failed to compile: …").
    pub custom_failures: Vec<String>,
    /// Queries of a parameter left out while its others translated (the
    /// feature's problems).
    partial: Vec<String>,
    /// Live's solved sketch geometry (`sketches` geomEntities) by (sketch
    /// feature id, entity id).
    solved: HashMap<(String, String), Value>,
    /// Sketch entities (sketch, live id) ours made with Use: their PROJECTED constraint is that.
    used: std::collections::HashSet<(String, String)>,
    /// The image tab standing in for icons live hasn't fetched.
    placeholder: Option<String>,
    /// Each Feature Studio's pinned std and the one our host serves for it.
    pub std_pins: Vec<(String, u32, u32)>,
    /// The Part Studio's final part ids (`partOrder`, an assembly corpus' studio without live's geometry): an id-only
    /// query of those is a part query.
    part_order: Vec<String>,
    /// Live's final parts' volume bands (`partVolumes`, m³; an assembly corpus' studio): an id-only region query is
    /// the regions whose extrude makes one of them.
    part_volumes: Vec<[f64; 2]>,
    /// The document's other Part Studios built before this one (an assembly corpus): element id → our tab id and
    /// their features' ids (live's → ours), for a Frame's profile sketch in another studio.
    pub other_studios: HashMap<String, (String, HashMap<String, String>)>,
    /// Their configurations' defaults (element → input id → value).
    pub other_defaults: HashMap<String, HashMap<String, Value>>,
    /// Our sketches' sketch_get within the current step (a studio's every region query read every sketch).
    sketch_cache: HashMap<String, Value>,
    /// Import features' files (features.json `foreignFiles`: foreign id →
    /// `{ "path", "placed" }`, placed: live's export of the Part Studio).
    foreign: HashMap<String, Value>,
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
        // A face may come as its plane's `origin` and its `area` (live 2290); a sketch's region as its `centroid`
        // and area, a sketch's line as its `mid` and `length` (live 2313, `bodyType` SKETCH).
        let point = f3(&v["point"]).or_else(|| f3(&v["origin"])).or_else(|| f3(&v["centroid"])).or_else(|| f3(&v["mid"]));
        let bbox = v["box"].as_array().and_then(|b| Some([f3(b.first()?)?, f3(b.get(1)?)?]));
        let r = LiveRef { kind: v["kind"].as_str().unwrap_or("").to_string(), point, normal: f3(&v["normal"]), direction: f3(&v["direction"]), measure: v["measure"].as_f64().or(v["area"].as_f64()).or(v["length"].as_f64()), geometry: our_type(v["surfaceType"].as_str().or(v["curveType"].as_str())), bbox, sketch: v["bodyType"] == "SKETCH", body: None, parameter: v["parameter"].as_str().map(str::to_string), plane_origin: v["point"].is_null() && !v["origin"].is_null(), sketch_file: false, sketch_of: v["sketchFeatureId"].as_str().map(str::to_string) };
        out.insert((k as usize, id.to_string()), r);
    }
    out
}

impl<'a> Translator<'a> {
    /// A new document at `path` for the Part Studio `doc` (features.json),
    /// with live's reference geometry `refs`.
    pub fn open(ctx: &'a mut Ctx, path: &Path, doc: &Value, refs: HashMap<(usize, String), LiveRef>) -> Result<Self, String> {
        let _ = std::fs::remove_file(path);
        let mut t = Self::new(ctx, doc, refs)?;
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
        t.studio_setup(doc);
        Ok(t)
    }

    /// Another Part Studio of the document open in `ctx` (an assembly's
    /// sources): a new tab `name` the features go into (the document's units
    /// are the first studio's).
    pub fn open_in(ctx: &'a mut Ctx, name: &str, doc: &Value, refs: HashMap<(usize, String), LiveRef>) -> Result<Self, String> {
        let mut t = Self::new(ctx, doc, refs)?;
        let made = t.call("tab_create", json!({ "kind": "Part Studio", "name": name }))?;
        t.tab = Some(made["tab"]["id"].as_str().ok_or("tab_create gave no tab id")?.to_string());
        t.studio_setup(doc);
        Ok(t)
    }

    /// Configuration inputs, Variable Studios and Feature Studios of `doc`.
    fn studio_setup(&mut self, doc: &Value) {
        for c in doc["configuration"]["configurationParameters"].as_array().into_iter().flatten() {
            self.config_input(c);
        }
        for table in doc["variables"].as_array().into_iter().flatten().filter(|v| v["variableStudioReference"].is_object()) {
            self.variable_studio(table);
        }
        // Files the studios import (icons, CSVs), as file tabs under their names.
        for (element, file) in doc["fileElements"].as_object().into_iter().flatten() {
            if let Err(e) = self.file_element(element, file) {
                self.notes.push(format!("file {}: {e}", file["name"].as_str().unwrap_or(element)));
            }
        }
        // Every tab first: a studio imports another by its element id
        // ("<eid>", or "<did>/<vid>/<eid>" in another document), ours by tab id.
        let studios: Vec<(String, Value)> = doc["featureStudios"].as_object().into_iter().flatten().map(|(e, s)| (e.clone(), s.clone())).collect();
        for (element, studio) in &studios {
            self.feature_studio_tab(element, studio);
        }
        for (element, studio) in &studios {
            self.feature_studio(element, studio);
        }
        for (element, studio) in &studios {
            self.feature_studio_check(element, studio);
        }
    }

    fn new(ctx: &'a mut Ctx, doc: &Value, refs: HashMap<(usize, String), LiveRef>) -> Result<Self, String> {
        let features = doc["features"].as_array().ok_or("features.json has no `features`")?.clone();
        let mut frames = HashMap::new();
        let mut solved = HashMap::new();
        for s in doc["sketches"]["sketches"].as_array().into_iter().flatten() {
            if let Some(id) = s["featureId"].as_str() {
                for g in s["geomEntities"].as_array().into_iter().flatten() {
                    if let Some(e) = g["id"].as_str() {
                        solved.insert((id.to_string(), e.to_string()), g.clone());
                    }
                }
            }
            // `sketchMatrix` (older API versions) or `transformMatrix` (live's fetch).
            let (Some(id), Some(m)) = (s["featureId"].as_str(), s["sketchMatrix"].as_array().or(s["transformMatrix"].as_array())) else { continue };
            let m: Vec<f64> = m.iter().filter_map(Value::as_f64).collect();
            if m.len() == 16 {
                // Row-major 4×4: columns x, y, normal, origin.
                frames.insert(id.to_string(), [[m[3], m[7], m[11]], [m[0], m[4], m[8]], [m[1], m[5], m[9]]]);
            }
        }
        let foreign = doc["foreignFiles"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
        let config_defaults = doc["configuration"]["configurationParameters"]
            .as_array()
            .into_iter()
            .flatten()
            // (A boolean configuration's default is a JSON boolean.)
            .filter_map(|c| Some((c["parameterId"].as_str()?.to_string(), c["defaultValue"].as_str().map(str::to_string).or_else(|| c["defaultValue"].as_bool().map(|b| b.to_string()))?)))
            .collect();
        Ok(Self { ctx, features, refs, feature_ids: HashMap::new(), sub_params: HashMap::new(), sketch_entities: HashMap::new(), sketch_points: Vec::new(), frames, curves: HashMap::new(), construction: Default::default(), config_defaults, sketch_edits: 0, handle_lines: HashMap::new(), sketch_before: Value::Null, sketch_map: SketchMap::default(), circles: HashMap::new(), ellipses: HashMap::new(), picked: Default::default(), tab: None, first_move: None, studios: HashMap::new(), notes: Vec::new(), custom_failures: Vec::new(), partial: Vec::new(), solved, used: Default::default(), placeholder: None, std_pins: Vec::new(), foreign, derived_sources: doc["derivedSources"].as_object().cloned().unwrap_or_default(), derived_built: HashMap::new(), part_order: doc["partOrder"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect(), part_volumes: doc["partVolumes"].as_array().into_iter().flatten().filter_map(|v| Some([v[0].as_f64()?, v[1].as_f64()?])).collect(), other_studios: HashMap::new(), other_defaults: HashMap::new(), sketch_cache: HashMap::new() })
    }

    /// A Feature Studio custom features come from (features.json
    /// `featureStudios`: element id → `{ "name", "source" }`): its tab.
    fn feature_studio_tab(&mut self, element: &str, studio: &Value) {
        let name = studio["name"].as_str().unwrap_or("Feature Studio 1");
        match self.call("tab_create", json!({ "kind": "featurestudio", "name": name })).and_then(|r| r["tab"]["id"].as_str().map(str::to_string).ok_or_else(|| "no tab id".to_string())) {
            Ok(tab) => {
                self.studios.insert(element.to_string(), tab);
            }
            Err(e) => self.notes.push(format!("Feature Studio {name}: {e}")),
        }
    }

    /// Its FeatureScript, imports of the document's other studios pointed
    /// at their tabs, committed.
    fn feature_studio(&mut self, element: &str, studio: &Value) {
        let (Some(source), Some(tab)) = (studio["source"].as_str().or(studio.as_str()), self.studios.get(element).cloned()) else { return };
        let name = studio["name"].as_str().unwrap_or("Feature Studio 1");
        let mut source = source.to_string();
        // A studio fetched at a version (`<eid>@<version>`, live's file name)
        // first: the imports of that element at that version.
        let mut others: Vec<(&String, &String)> = self.studios.iter().collect();
        others.sort_by_key(|(e, _)| (!e.contains('@'), e.to_string()));
        for (other, ours) in others {
            if let Some((e, v)) = other.split_once('@') {
                for path in [format!("\"{e}\""), format!("/{e}\"")] {
                    let mut from = 0;
                    while let Some(at) = source[from..].find(&path).map(|i| i + from) {
                        let close = at + path.len();
                        let statement = &source[close..source[close..].find(')').map_or(source.len(), |i| i + close)];
                        if !statement.contains(&format!("\"{v}\"")) {
                            from = close;
                            continue;
                        }
                        let open = source[..at + 1].rfind('"').unwrap_or(at);
                        source.replace_range(open..close, &format!("\"{ours}\""));
                        from = open + ours.len() + 2;
                    }
                }
                continue;
            }
            for path in [format!("\"{other}\""), format!("/{other}\"")] {
                if let Some(at) = source.find(&path) {
                    // The whole path string: from its opening quote.
                    let open = source[..at + 1].rfind('"').unwrap_or(at);
                    let close = at + path.len();
                    source.replace_range(open..close, &format!("\"{ours}\""));
                }
            }
        }
        // An icon or image the studio imports (used only as `ns::BLOB_DATA`
        // in its annotations, no effect on geometry) that live hasn't
        // fetched: a placeholder image tab.
        for (ns, path) in blob_imports(&source) {
            let element = path.rsplit('/').next().unwrap_or(&path).to_string();
            // Fetched (its path already ours) or a studio of the document.
            if self.studios.contains_key(&element) || self.studios.values().any(|t| *t == element) {
                continue;
            }
            let Some(image) = self.placeholder_image() else { break };
            source = source.replacen(&format!("{ns}::import(path : \"{path}\""), &format!("{ns}::import(path : \"{image}\""), 1);
            self.notes.push(format!("Feature Studio {name}: its image {element} (`{ns}::BLOB_DATA`) not fetched: a placeholder"));
        }
        if let Err(e) = self.call("featurestudio_edit", json!({ "tab": tab, "source": source })).and_then(|_| self.call("featurestudio_commit", json!({ "tab": tab }))) {
            self.notes.push(format!("Feature Studio {name}: {e}"));
        }
        // Its pinned std (`FeatureScript N;`), loaded as our host serves it.
        if let Some(pin) = source.split("FeatureScript").nth(1).and_then(|r| r.trim_start().split(';').next()).and_then(|v| v.trim().parse::<u32>().ok()) {
            self.std_pins.push((name.to_string(), pin, noshape_featurescript::std_version_for(pin)));
        }
    }

    /// A 1 × 1 image tab for icons and images live hasn't fetched (made once).
    /// A file element (live's `<eid>.blob`) imported as a file tab under its
    /// name; studios' imports of the element then point at it.
    fn file_element(&mut self, element: &str, file: &Value) -> Result<(), String> {
        let (Some(path), Some(name)) = (file["path"].as_str(), file["name"].as_str()) else { return Err("no path or name".into()) };
        let dir = std::env::temp_dir().join(format!("noshape-parity-files-{}-{element}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let named = dir.join(name);
        std::fs::copy(path, &named).map_err(|e| e.to_string())?;
        let r = self.call("import_files", json!({ "files": [named.to_string_lossy()] }))?;
        let tab = r["first"].as_str().ok_or("the import made no tab")?.to_string();
        self.studios.insert(element.to_string(), tab);
        Ok(())
    }

    fn placeholder_image(&mut self) -> Option<String> {
        if let Some(t) = &self.placeholder {
            return Some(t.clone());
        }
        const PNG: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00,
            0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let path = std::env::temp_dir().join(format!("noshape-parity-icon-{}.png", std::process::id()));
        std::fs::write(&path, PNG).ok()?;
        let r = self.call("import_files", json!({ "files": [path.to_str()?] })).ok()?;
        let tab = r["first"].as_str()?.to_string();
        self.placeholder = Some(tab.clone());
        Some(tab)
    }

    /// What the editor's notices call errors, once every studio is in: it
    /// doesn't compile.
    fn feature_studio_check(&mut self, element: &str, studio: &Value) {
        let Some(tab) = self.studios.get(element).cloned() else { return };
        let name = studio["name"].as_str().unwrap_or("Feature Studio 1");
        if let Ok(g) = self.call("featurestudio_get", json!({ "tab": tab })) {
            let errors: Vec<String> = g["notices"].as_array().into_iter().flatten().filter(|n| n["severity"] == "error").map(|n| format!("{} (line {})", n["message"].as_str().unwrap_or("?"), n["line"])).collect();
            if !errors.is_empty() {
                self.custom_failures.push(format!("{name}: failed to compile: {}", errors.iter().take(3).cloned().collect::<Vec<_>>().join("; ")));
            }
        }
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

    pub fn call(&mut self, tool: &str, mut args: Value) -> Result<Value, String> {
        let op = registry::by_tool_name(tool).ok_or_else(|| format!("no tool {tool}"))?;
        if let (Some(tab), Some(a)) = (&self.tab, args.as_object_mut())
            && op.fields.iter().any(|f| f.name == "tab")
            && !a.contains_key("tab")
        {
            a.insert("tab".into(), json!(tab));
        }
        registry::run(self.ctx, op, &args).map(|o| o.value).map_err(|e| e.to_json().to_string())
    }

    /// Our feature for live's feature id.
    pub fn ours(&self, live: &str) -> Option<&str> {
        self.feature_ids.get(live).map(String::as_str)
    }

    /// Our entities for an Onshape query on the finished Part Studio (an
    /// assembly's mate connector names a part's edge or face this way).
    pub fn resolve(&mut self, q: &Value) -> Result<Vec<String>, String> {
        let k = self.features.len();
        self.query(k, q)
    }

    /// The entity whose implicit connector of `inference` (Onshape's
    /// EntityInferenceType) lies at `origin` (part coordinates, metres): an
    /// assembly's connector found from live's frame when its query isn't read.
    pub fn connector_entity(&mut self, inference: &str, origin: [f64; 3]) -> Result<String, String> {
        let (kind, key) = match inference {
            "CENTER" | "TOP_AXIS_POINT" | "MID_AXIS_POINT" | "BOTTOM_AXIS_POINT" => ("edges", "center"),
            "MID_POINT" => ("edges", "midpoint"),
            "POINT" => ("vertices", "position"),
            _ => ("faces", "centroid"),
        };
        let r = self.call("query_entities", json!({ "kind": kind, "near": mm(origin), "limit": 50 }))?;
        let k = per_unit(&r["units"]);
        let at = |e: &Value| (0..3).map(|i| e[key][i].as_f64().map_or(f64::INFINITY, |c| (c * k - origin[i]).abs())).fold(0.0, f64::max);
        r[kind]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| at(e) <= 1e-6)
            .find_map(|e| e["id"].as_str().map(str::to_string))
            .ok_or_else(|| format!("no {} of ours with its {key} at {:?} mm", &kind[..kind.len() - 1], origin.map(|c| c * 1e3)))
    }

    /// Places an instance of the assembly tab `assembly` where live has its occurrence:
    /// `transform` row-major 4×4, metres (the assembly definition's
    /// `occurrences[].transform`).
    pub fn place(&mut self, assembly: &str, instance: &str, transform: &[f64]) -> Result<(), String> {
        let [m0, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, ..] = transform else { return Err(format!("transform of {instance}: 16 numbers")) };
        let rot = glam::DMat3::from_cols(glam::DVec3::new(*m0, *m4, *m8), glam::DVec3::new(*m1, *m5, *m9), glam::DVec3::new(*m2, *m6, *m10));
        let (axis, angle) = glam::DQuat::from_mat3(&rot).to_axis_angle();
        let mut args = json!({ "tab": assembly, "instance": instance, "translation": mm([*m3, *m7, *m11]) });
        if angle.abs() > 1e-12 {
            args["axis"] = json!([axis.x, axis.y, axis.z]);
            args["angle"] = json!(format!("{} rad", angle));
        }
        self.call("assembly_transform", args).map(|_| ())
    }

    /// Rebuilds feature `k` (after features 0 … k−1).
    pub fn step(&mut self, k: usize) -> Step {
        self.sketch_cache.clear();
        let f = self.features[k].clone();
        let mut step = Step { index: k, name: f["name"].as_str().unwrap_or("").to_string(), feature_type: f["featureType"].as_str().unwrap_or("").to_string(), ..Default::default() };
        step.custom = f["namespace"].as_str().is_some_and(|n| !n.is_empty());
        // Mate connectors made inside the feature's dialog (its sub-features): translated here, they go into the
        // feature's selections that name them (`{"mate_connector": …}`, ours in-feature too).
        for sub in f["subFeatures"].as_array().into_iter().flatten().filter(|s| s["featureType"] == "mateConnector") {
            let mut problems = Vec::new();
            match self.feature_params(k, sub, &mut problems) {
                Ok((_, params)) => {
                    if let Some(oid) = sub["featureId"].as_str() {
                        self.sub_params.insert(oid.to_string(), params);
                    }
                }
                Err(e) => step.problems.push(format!("its in-feature {}: not translated: {e}", sub["featureType"].as_str().unwrap_or("?"))),
            }
            step.problems.extend(problems.into_iter().map(|p| format!("its in-feature {}: {p}", sub["featureType"].as_str().unwrap_or("?"))));
        }
        let made = if f["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketch")) {
            self.sketch(k, &f, &mut step.problems)
        } else if f["featureType"] == "importForeign" {
            self.import_foreign(&f)
        } else {
            self.feature(k, &f, &mut step.problems)
        };
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
                if f["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketch")) {
                    step.frame = self.frame_difference(&f, &id);
                }
                step.ours = Some(id);
                step.status = Some(status);
                step.message = message;
            }
            Err(e) => step.problems.insert(0, format!("not built: {e}")),
        }
        step
    }

    /// An Import feature: live's file imported as the Import dialog does (a
    /// Part Studio of its own, its Import feature first), the features
    /// after it then built in that Part Studio.
    fn import_foreign(&mut self, f: &Value) -> Result<(String, String, Option<String>), String> {
        let param = |id: &str| f["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == id).cloned().unwrap_or(Value::Null);
        // The file: `foreignId`, else (an assembly corpus' studio) the blob element of `blobData`'s namespace
        // ("e<element>::m<microversion>").
        let blob = param("blobData")["namespace"].as_str().and_then(|n| n.strip_prefix('e')).and_then(|n| n.split("::").next()).map(str::to_string);
        let id = param("foreignId")["foreignId"].as_str().map(str::to_string).or(blob).ok_or("no foreignId")?;
        let file = self.foreign.get(&id).cloned().ok_or_else(|| format!("its file {id} not fetched"))?;
        let path = file["path"].as_str().ok_or_else(|| format!("its file {id}: no path"))?;
        if !self.feature_ids.is_empty() {
            return Err("an Import after other features (ours imports into a Part Studio of its own)".into());
        }
        let y_up = file["placed"] != true && param("yAxisIsUp")["value"].as_bool() == Some(true);
        // `flatten` (an assembly's instances as placed parts of one Part Studio, live c2-gamma-imported): ours is
        // the dialog's single Part Studio.
        let flatten = param("flatten")["value"].as_bool() == Some(true);
        let r = self.call("import_files", json!({ "files": [path], "y_up": y_up, "keep_files": false, "combine": flatten }))?;
        // (A native assembly zip also makes an Assembly: the Import feature is its Part Studio's.)
        let studio = r["tabs"].as_array().into_iter().flatten().find(|t| t["kind"] == "Part Studio").and_then(|t| t["id"].as_str());
        self.tab = Some(studio.or(r["first"].as_str()).ok_or("the import made no Part Studio")?.to_string());
        let list = self.call("feature_list", json!({}))?;
        let first = &list["features"][0];
        let ours = first["id"].as_str().ok_or("the import's Part Studio has no feature")?.to_string();
        Ok((ours, first["status"].as_str().unwrap_or("Ok").to_string(), first["message"].as_str().map(str::to_string)))
    }

    /// Our sketch's frame against live's (`sketches`): origin within 1e-9 m,
    /// x and normal within 1e-9.
    fn frame_difference(&mut self, f: &Value, ours: &str) -> Option<String> {
        let [lo, lx, ly] = *self.frames.get(f["featureId"].as_str()?)?;
        let g = self.call("sketch_get", json!({ "sketch": ours })).ok()?;
        let k = per_unit(&g["units"]);
        let (o, x, n) = (f3(&g["frame"]["origin"])?.map(|c| c * k), f3(&g["frame"]["x"])?, f3(&g["frame"]["normal"])?);
        let ln = [lx[1] * ly[2] - lx[2] * ly[1], lx[2] * ly[0] - lx[0] * ly[2], lx[0] * ly[1] - lx[1] * ly[0]];
        let off = |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max);
        let r = |v: [f64; 3]| v.map(|c| (c * 1e6).round() / 1e6 + 0.0);
        (off(o, lo) > 1e-9 || off(x, lx) > 1e-9 || off(n, ln) > 1e-9).then(|| {
            format!("sketch frame origin {:?} mm, x {:?}, normal {:?}; live {:?} mm, {:?}, {:?}", r(o.map(|c| c * 1e3)), r(x), r(n), r(lo.map(|c| c * 1e3)), r(lx), r(ln))
        })
    }

    /// A non-sketch feature through feature_add, its parameters by name.
    /// Feature `f` (live's JSON) translated: our type and parameters.
    fn feature_params(&mut self, k: usize, f: &Value, problems: &mut Vec<String>) -> Result<(String, Map<String, Value>), String> {
        let ty = f["featureType"].as_str().ok_or("no featureType")?;
        // A custom feature: its namespace names the Feature Studio ("e<element id>::m<microversion>").
        // "e<element>::m<microversion>", or another document's version
        // "d<document>::v<version>::e<element>::m<microversion>".
        // The studio at the microversion the feature was made with
        // (studios/<eid>@<microversion>.fs) when live fetched it, else its head.
        let element = |n: &str| n.split("::").find_map(|p| p.strip_prefix('e')).map(str::to_string);
        let version = |n: &str| n.split("::").find_map(|p| p.strip_prefix('m')).map(str::to_string);
        let studio = |n: &str| element(n).and_then(|e| version(n).and_then(|m| self.studios.get(&format!("{e}@{m}"))).or_else(|| self.studios.get(&e)));
        let custom = f["namespace"].as_str().filter(|n| !n.is_empty()).map(|n| match studio(n) {
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
            // (asVersion where noshape keeps it: the Loft, whose start rule changed between versions.)
            if INTERNAL.contains(&id) && !(id == "asVersion" && known.iter().any(|n| n == id)) {
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
                Ok(Some(v)) => match lookup_path_in_level_order(ty, id, v) {
                    Ok(v) => {
                        params.insert(id.to_string(), v);
                    }
                    Err(e) => problems.push(format!("{id}: {e}")),
                },
                Err(e) => problems.push(format!("{id}: {e}")),
            }
        }
        problems.append(&mut self.partial);
        if tolerances > 0 {
            problems.push(format!("{tolerances} tolerance / precision fields not translated (no effect on the geometry)"));
        }
        // A connector on the face a sketch's circle imprints (ours: the
        // sketch region): on the circle itself, the same centre and normal.
        if ty == "mateConnector"
            && let Some(Value::Array(ids)) = params.get("originQuery").cloned()
        {
            let on: Vec<Value> = ids.iter().map(|id| id.as_str().filter(|i| i.starts_with("region:")).and_then(|r| self.region_curve(r)).map(Value::from).unwrap_or_else(|| id.clone())).collect();
            params.insert("originQuery".into(), Value::Array(on));
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
        Ok((ty.to_string(), params))
    }

    /// Placeholders of in-feature mate connectors in a translated selection, replaced by the connector's parameters.
    fn place_sub_features(&self, v: &mut Value) {
        match v {
            Value::String(id) => {
                if let Some(params) = id.strip_prefix(SUB_FEATURE).and_then(|oid| self.sub_params.get(oid)) {
                    *v = json!({ "mate_connector": params });
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|x| self.place_sub_features(x)),
            Value::Object(o) => o.values_mut().for_each(|x| self.place_sub_features(x)),
            _ => {}
        }
    }

    /// Feature `f` added after features 0 … k−1: its id, status and message.
    fn feature(&mut self, k: usize, f: &Value, problems: &mut Vec<String>) -> Result<(String, String, Option<String>), String> {
        let (ty, mut params) = if f["featureType"] == "importDerived" { self.derived_params(f, problems)? } else { self.feature_params(k, f, problems)? };
        // Its in-feature mate connectors, by their placeholders in its selections.
        for v in params.values_mut() {
            self.place_sub_features(v);
        }
        let ty = ty.as_str();
        let mut args = json!({ "type": ty, "params": params });
        // "#featureName": Onshape's placeholder for the default name.
        if let Some(n) = f["name"].as_str().filter(|n| !n.starts_with('#')) {
            args["name"] = json!(n);
        }
        // A parameter we reject: reported, and the feature added without it.
        let r = loop {
            // NOSHAPE_PARITY_TRACE=1: each feature as sent (to tell our
            // translation's changes from the evaluation's).
            if std::env::var_os("NOSHAPE_PARITY_TRACE").is_some() {
                eprintln!("TRACE feature_add {args}");
            }
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

    /// A Derived feature's parameters: its source Part Studio (`buildFunction`'s element, fetched beside the
    /// document) built in a tab of ours on first use, its parts ours of that tab's (live's part ids at the source's
    /// end state).
    fn derived_params(&mut self, f: &Value, problems: &mut Vec<String>) -> Result<(String, Map<String, Value>), String> {
        let param = |id: &str| f["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == id).cloned().unwrap_or(Value::Null);
        // (The new Derived, `newUI`: a `partStudio` reference with its own `partQuery`; the old one: `buildFunction` and
        // `parts`.)
        let studio = param("partStudio");
        let build = if studio.is_null() { param("buildFunction") } else { studio.clone() };
        let namespace = build["namespace"].as_str().unwrap_or("");
        // No source at all: live's ERROR "Select entities from another Part Studio." (c2-fan-blade-sweep's source's
        // Derived 1), ours the same.
        if namespace.is_empty() {
            return Ok(("importDerived".into(), Map::new()));
        }
        let element = namespace.split("::").find_map(|s| s.strip_prefix('e')).ok_or_else(|| format!("Derived: no source element in {namespace:?}"))?.to_string();
        if !self.derived_built.contains_key(&element) {
            let built = self.derived_source(&element, problems)?;
            self.derived_built.insert(element.clone(), built);
        }
        let (tab, parts) = &self.derived_built[&element];
        let mut ours = Vec::new();
        let part_query = if studio.is_null() { param("parts") } else { studio["partQuery"].clone() };
        for q in part_query["queries"].as_array().into_iter().flatten() {
            for id in q["deterministicIds"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                ours.push(parts.get(id).cloned().ok_or_else(|| format!("Derived: the source's part {id} not built"))?);
            }
        }
        let mut params = Map::new();
        params.insert("partStudio".into(), json!(tab));
        params.insert("parts".into(), json!(ours));
        Ok(("importDerived".into(), params))
    }

    /// The Derived source `element` translated into a new Part Studio tab of ours: its tab and live's part ids
    /// (at its end) → ours.
    fn derived_source(&mut self, element: &str, problems: &mut Vec<String>) -> Result<(String, HashMap<String, String>), String> {
        let src = self.derived_sources.get(element).cloned().ok_or_else(|| format!("Derived: Part Studio {element} of another document not fetched"))?;
        let lines: Vec<Value> = src["live"].as_array().cloned().unwrap_or_default();
        // Ours stays in its own tab whatever tab the new one becomes.
        if self.tab.is_none() {
            let info = self.call("document_info", json!({}))?;
            self.tab = info["tabs"].as_array().into_iter().flatten().find(|t| t["kind"] == "Part Studio").and_then(|t| t["id"].as_str()).map(str::to_string);
        }
        let name = src["doc"]["name"].as_str().map(str::to_string).unwrap_or_else(|| format!("Derived source {element}"));
        // Its Feature Studios ours already has (the same element of the same document, e.g. at the version both use)
        // are those tabs: a studio fetched alone may import others only the document has.
        let mut doc = src["doc"].clone();
        if let Some(studios) = doc["featureStudios"].as_object_mut() {
            studios.retain(|e, _| !self.studios.keys().any(|k| k == e || k.starts_with(&format!("{e}@"))));
        }
        let parent_studios = self.studios.clone();
        let mut refs = refs_from_bodies(&lines);
        refs.extend(parse_refs(src["refs"].as_str().unwrap_or("")));
        let mut t = Translator::open_in(&mut *self.ctx, &name, &doc, refs)?;
        for (e, tab) in parent_studios {
            t.studios.entry(e).or_insert(tab);
        }
        for k in 0..t.len() {
            let step = t.step(k);
            if step.ours.is_none() {
                problems.push(format!("its source's {} not built: {}", step.name, step.problems.join("; ")));
            } else {
                problems.extend(step.problems.iter().map(|p| format!("its source's {}: {p}", step.name)));
            }
        }
        let tab = t.tab.clone().ok_or("Derived: the source's tab")?;
        let last = lines.iter().rfind(|l| l.get("index").is_some()).cloned().unwrap_or(Value::Null);
        let mut parts = HashMap::new();
        // By live's geometry for the part, else by its name (ours are named as Onshape's).
        let named = t.call("query_parts", json!({}))?;
        for p in last["parts"].as_array().into_iter().flatten() {
            let Some(id) = p["partId"].as_str() else { continue };
            let by_name = || named["parts"].as_array().into_iter().flatten().find(|o| o["name"] == p["name"]).and_then(|o| o["id"].as_str()).map(str::to_string);
            match t.resolve(&json!({ "btType": "BTMIndividualQuery-138", "deterministicIds": [id] })).ok().as_deref() {
                Some([one]) => {
                    parts.insert(id.to_string(), one.clone());
                }
                _ => {
                    if let Some(one) = by_name() {
                        parts.insert(id.to_string(), one);
                    }
                }
            }
        }
        problems.extend(t.notes.iter().map(|n| format!("its source: {n}")));
        self.custom_failures.extend(t.custom_failures.iter().cloned());
        Ok((tab, parts))
    }

    /// Sketch entity `e` of sketch `oid` moved to live's solved geometry
    /// (lines, circles, arcs, points and texts; `None`: no solve given or no change).
    fn solved_entity(&self, oid: &str, e: &Value) -> Option<Value> {
        let id = e["entityId"].as_str()?;
        let g = self.solved.get(&(oid.to_string(), id.to_string()))?;
        let p2 = |v: &Value| Some([v[0].as_f64()?, v[1].as_f64()?]);
        let mut out = e.clone();
        let gt = e["geometry"]["btType"].as_str().unwrap_or("");
        match g["entityType"].as_str()? {
            "lineSegment" if gt.starts_with("BTCurveGeometryLine") => {
                // Its ends by their point records: the segment's own may run the other way (c2-sm-louver's 0.9 µm
                // line, stored with its end parameter below its start).
                let end = |which: &str| self.solved.get(&(oid.to_string(), format!("{id}.{which}"))).and_then(|p| p2(&p["point"]));
                let (a, b) = match (end("start"), end("end")) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (p2(&g["startPoint"])?, p2(&g["endPoint"])?),
                };
                let l = (b[0] - a[0]).hypot(b[1] - a[1]);
                if l < 1e-12 {
                    return None;
                }
                out["geometry"]["pntX"] = json!(a[0]);
                out["geometry"]["pntY"] = json!(a[1]);
                out["geometry"]["dirX"] = json!((b[0] - a[0]) / l);
                out["geometry"]["dirY"] = json!((b[1] - a[1]) / l);
                out["startParam"] = json!(0.0);
                out["endParam"] = json!(l);
            }
            "circle" | "arc" if gt.starts_with("BTCurveGeometryCircle") => {
                let c = p2(&g["center"])?;
                out["geometry"]["xCenter"] = json!(c[0]);
                out["geometry"]["yCenter"] = json!(c[1]);
                out["geometry"]["radius"] = g["radius"].clone();
                // An arc's ends as angles from its xDir, in its sense.
                if g["entityType"] == "arc" {
                    let (xd, yd) = (e["geometry"]["xDir"].as_f64().unwrap_or(1.0), e["geometry"]["yDir"].as_f64().unwrap_or(0.0));
                    let sense = if e["geometry"]["clockwise"].as_bool() == Some(true) { -1.0 } else { 1.0 };
                    let angle = |p: [f64; 2]| {
                        let (u, v) = (p[0] - c[0], p[1] - c[1]);
                        sense * (u * -yd + v * xd).atan2(u * xd + v * yd)
                    };
                    let (t0, mut t1) = (angle(p2(&g["startPoint"])?), angle(p2(&g["endPoint"])?));
                    while t1 <= t0 + 1e-12 {
                        t1 += std::f64::consts::TAU;
                    }
                    out["startParam"] = json!(t0);
                    out["endParam"] = json!(t1);
                }
            }
            "point" if e["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketchPoint")) => {
                let p = p2(&g["point"])?;
                out["x"] = json!(p[0]);
                out["y"] = json!(p[1]);
            }
            // A text's box where its constraints put it (c2-valve-config's "6" centred by a MIDPOINT; the stored
            // baseline is where it was typed).
            "sketchText" if e["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketchTextEntity")) => {
                let (start, dir) = (p2(&g["startPoint"])?, p2(&g["direction"])?);
                out["baselineStartX"] = json!(start[0]);
                out["baselineStartY"] = json!(start[1]);
                out["baselineDirectionX"] = json!(dir[0]);
                out["baselineDirectionY"] = json!(dir[1]);
                if let Some(ascent) = g["textAscent"].as_f64() {
                    out["ascent"] = json!(ascent);
                }
            }
            _ => return None,
        }
        Some(out)
    }

    /// Live's Use of model geometry (a PROJECTED entity) as Use makes it, when ours lands where live's is; else
    /// drawn as the other entities.
    fn use_projected(&mut self, oid: &str, id: &str, u: Use, problems: &mut Vec<String>) {
        let map = self.sketch_map;
        let top = |t: &mut Self| t.ctx.doc(None).ok().and_then(|od| od.session.undo_tops().0);
        let mark = top(self);
        let point = u.shape["tool"] == "point";
        let made = self.call("sketch_use", json!({ "sketch": id, "references": [u.reference] })).ok();
        let ours = made.as_ref().and_then(|r| r["created"].as_array()?.iter().find(|c| (c["type"] == "point") == point).cloned());
        let near = |c: &Value, key: &str, q: [f64; 2]| {
            let q = map.local(q);
            c[key][0].as_f64().zip(c[key][1].as_f64()).is_some_and(|(x, y)| (x * map.k - q[0]).hypot(y * map.k - q[1]) < 1e-7)
        };
        let lands = ours.as_ref().is_some_and(|c| match (u.point, u.ends) {
            (Some(p), _) => near(c, "at", p),
            (None, Some([a, b])) => (near(c, "start", a) && near(c, "end", b)) || (near(c, "start", b) && near(c, "end", a)),
            (None, None) => true,
        });
        if let (true, Some(c)) = (lands, ours) {
            self.map_drawn(oid, &u.eid, &c, point, u.ends);
            self.used.insert((oid.to_string(), u.eid));
            return;
        }
        if made.is_some() && top(self) != mark {
            let _ = self.call("history_undo", json!({}));
        }
        match self.call("sketch_draw", json!({ "sketch": id, "shapes": [u.shape] })) {
            Ok(r) => {
                if let Some(c) = r["created"].as_array().and_then(|l| l.iter().find(|c| (c["type"] == "point") == point)) {
                    self.map_drawn(oid, &u.eid, c, point, u.ends);
                }
            }
            Err(e) => problems.push(format!("sketch entity {}: not drawn ({e})", u.eid)),
        }
    }

    /// Sketch `oid`'s entity `eid` is our `c` (a sketch_draw or sketch_use reply's entity): its id, its ends (ours
    /// may run the other way: its start nearer Onshape's `ends` end), centre and a spline's handles.
    fn map_drawn(&mut self, oid: &str, eid: &str, c: &Value, point: bool, ends: Option<[[f64; 2]; 2]>) {
        let map = self.sketch_map;
        let oid = oid.to_string();
        if let Some(cid) = c["id"].as_str() {
            self.sketch_entities.insert((oid.clone(), eid.to_string()), cid.to_string());
            if point {
                self.sketch_points.push((oid.clone(), cid.to_string()));
            }
        }
        let at = |key: &str| Some([c[key][0].as_f64()? * map.k, c[key][1].as_f64()? * map.k]);
        let reversed = match (ends, at("start")) {
            (Some([s, e]), Some(p)) => {
                let d = |q: [f64; 2]| (q[0] - p[0]).hypot(q[1] - p[1]);
                d(map.local(e)) < d(map.local(s)) - 1e-9
            }
            _ => false,
        };
        // A spline's handles: their tips, and the lines from its ends.
        if let (Some([h0, h1]), Some(pts)) = (c["handles"].as_array().and_then(|h| Some([h.first()?.as_str()?.to_string(), h.get(1)?.as_str()?.to_string()])), c["point_ids"].as_array()) {
            let (first, last) = (pts.first().and_then(Value::as_str).unwrap_or_default().to_string(), pts.last().and_then(Value::as_str).unwrap_or_default().to_string());
            let arms: Vec<Option<String>> = c["handle_arms"].as_array().map(|a| a.iter().map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
            for (k, (end, tip, at)) in [("startHandle", h0, first), ("endHandle", h1, last)].into_iter().enumerate() {
                self.sketch_entities.insert((oid.clone(), format!("{eid}.{end}.start")), at.clone());
                self.sketch_entities.insert((oid.clone(), format!("{eid}.{end}.end")), tip.clone());
                // Live's handle line itself: our arm (a line taking its constraints and dimensions).
                if let Some(Some(arm)) = arms.get(k) {
                    self.sketch_entities.insert((oid.clone(), format!("{eid}.{end}")), arm.clone());
                }
                self.handle_lines.insert((oid.clone(), format!("{eid}.{end}")), [at, tip]);
            }
        }
        // A Bezier's control polygon: live's `<id>.cppolygon.<n>` lines, pole to pole.
        if let (Some(arms), Some(pts)) = (c["handle_arms"].as_array().filter(|_| c["type"] == "bspline"), c["point_ids"].as_array()) {
            for (n, arm) in arms.iter().enumerate() {
                let name = format!("{eid}.cppolygon.{n}");
                for (end, pole) in [("start", pts.get(n)), ("end", pts.get(n + 1))] {
                    if let Some(pole) = pole.and_then(Value::as_str) {
                        self.sketch_entities.insert((oid.clone(), format!("{name}.{end}")), pole.to_string());
                    }
                }
                if let Some(arm) = arm.as_str() {
                    self.sketch_entities.insert((oid.clone(), name), arm.to_string());
                }
            }
        }
        let keys = if reversed { [("start", "end_point"), ("end", "start_point"), ("center", "center_point")] } else { [("start", "start_point"), ("end", "end_point"), ("center", "center_point")] };
        for (end, key) in keys {
            if let Some(pid) = c[key].as_str() {
                self.sketch_entities.insert((oid.clone(), format!("{eid}.{end}")), pid.to_string());
            }
        }
    }

    /// Our plane (a default plane, a Plane feature's or a planar face) where
    /// live's frame of sketch `oid` lies: its origin on it, normals parallel.
    fn plane_at_frame(&mut self, oid: &str) -> Option<String> {
        let [o, x, y] = *self.frames.get(oid)?;
        let n = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
        let on = |po: [f64; 3], pn: [f64; 3]| {
            let parallel = (pn[0] * n[0] + pn[1] * n[1] + pn[2] * n[2]).abs() > 1.0 - 1e-9;
            parallel && ((o[0] - po[0]) * pn[0] + (o[1] - po[1]) * pn[1] + (o[2] - po[2]) * pn[2]).abs() < 1e-9
        };
        // The default planes and Plane features.
        if let Ok(d) = self.call("query_datums", json!({})) {
            let k = per_unit(&d["units"]);
            let found = d["planes"].as_array().into_iter().flatten().find(|p| f3(&p["frame"]["origin"]).zip(f3(&p["frame"]["normal"])).is_some_and(|(po, pn)| on(po.map(|c| c * k), pn)));
            if let Some(id) = found.and_then(|p| p["id"].as_str()) {
                return Some(id.to_string());
            }
        }
        let faces = self.call("query_entities", json!({ "kind": "faces", "surface": "plane", "normal": n, "near": mm(o), "limit": 20 })).ok()?;
        let k = per_unit(&faces["units"]);
        faces["faces"].as_array()?.iter().find(|f| f3(&f["centroid"]).is_some_and(|c| on(c.map(|v| v * k), n))).and_then(|f| f["id"].as_str().map(str::to_string))
    }

    /// The regions of the last sketch before feature `k` when `queries` are
    /// as many id-only queries as it has regions.
    fn last_sketch_regions(&mut self, k: usize, queries: &[Value]) -> Option<Vec<String>> {
        if queries.is_empty() || queries.iter().any(|q| q["queryString"].as_str().is_some_and(|t| !t.trim().is_empty()) || q["deterministicIds"].as_array().is_none_or(|a| a.is_empty())) {
            return None;
        }
        let ids: usize = queries.iter().map(|q| q["deterministicIds"].as_array().map_or(0, Vec::len)).sum();
        let sketch = self.features[..k].iter().rev().find(|f| f["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketch")))?;
        let ours = self.feature_ids.get(sketch["featureId"].as_str()?)?.clone();
        let g = self.call("sketch_get", json!({ "sketch": ours })).ok()?;
        let regions: Vec<String> = g["regions"].as_array()?.iter().filter(|r| r["imprinted"] != true).filter_map(|r| r["id"].as_str().map(str::to_string)).collect();
        if regions.len() == ids {
            return Some(regions);
        }
        if let Some(r) = self.regions_of_new_bodies(k, &ours, &g, ids, true) {
            return Some(r);
        }
        // Guessed only without live's geometry (an assembly corpus' studio), for a feature that takes regions.
        let kind = self.features.get(k).and_then(|f| f["featureType"].as_str()).unwrap_or("");
        if self.part_order.is_empty() || !["extrude", "revolve"].contains(&kind) {
            return None;
        }
        self.regions_by_volume(k, &g, ids).or_else(|| outer_region(&g, ids))
    }

    /// Without live's geometry: the `ids` regions of sketch `g` whose blind extrude (feature `k`, a new solid) has the
    /// volume of one of live's final parts, when only one choice does.
    fn regions_by_volume(&self, k: usize, g: &Value, ids: usize) -> Option<Vec<String>> {
        let params = self.features.get(k)?["parameters"].as_array()?;
        let get = |id: &str| params.iter().find(|p| p["parameterId"] == id);
        let value = |id: &str| get(id).and_then(|p| p["value"].as_str());
        if self.part_volumes.is_empty() || value("operationType") != Some("NEW") || value("bodyType").is_some_and(|b| b != "SOLID") || get("hasSecondDirection").is_some_and(|p| p["value"] == true) {
            return None;
        }
        if !matches!(value("endBound"), Some("BLIND" | "SYMMETRIC")) {
            return None;
        }
        let depth = noshape_units::evaluate_as(get("depth")?["expression"].as_str()?, noshape_units::Dim::LENGTH, &noshape_units::UnitPrefs::default(), &noshape_units::EmptyScope).ok()?.value;
        let unit = per_unit(&g["units"]);
        let regions: Vec<(&str, f64)> = g["regions"].as_array()?.iter().filter(|r| r["imprinted"] != true).filter_map(|r| Some((r["id"].as_str()?, r["area"].as_f64()? * unit * unit))).collect();
        if ids == 0 || ids > regions.len() || regions.len() > 16 {
            return None;
        }
        let fits = |v: f64| self.part_volumes.iter().any(|&[lo, hi]| v >= lo - 1e-6 * hi.abs() && v <= hi + 1e-6 * hi.abs());
        let mut found: Vec<Vec<String>> = Vec::new();
        for mask in 0u32..1 << regions.len() {
            if mask.count_ones() as usize != ids {
                continue;
            }
            let picked: Vec<&(&str, f64)> = regions.iter().enumerate().filter(|(i, _)| mask >> i & 1 == 1).map(|(_, r)| r).collect();
            if fits(picked.iter().map(|r| r.1).sum::<f64>() * depth) {
                found.push(picked.iter().map(|r| r.0.to_string()).collect());
            }
        }
        (found.len() == 1).then(|| found.remove(0))
    }

    /// Fewer regions named than the sketch has: the ones a part feature `k`
    /// made (live's line after it; or the faces it added to a part) covers, seen along the sketch's normal (an
    /// extruded part's footprint): the region with the part's box, else the
    /// regions inside that box which together fill it (a board split by a
    /// line).
    /// (`caps`: also by the cap a region added to a part made.)
    fn regions_of_new_bodies(&mut self, k: usize, _sketch: &str, g: &Value, ids: usize, caps: bool) -> Option<Vec<String>> {
        let before: std::collections::HashSet<&String> = self.refs.keys().filter(|(j, _)| *j == k).map(|(_, id)| id).collect();
        // (By id: the same order every run.)
        let made = |kind: &str| -> Vec<[[f64; 3]; 2]> {
            let mut m: Vec<(&String, [[f64; 3]; 2])> = self.refs.iter().filter(|((j, id), r)| *j == k + 1 && r.kind == kind && !before.contains(id)).filter_map(|((_, id), r)| Some((id, r.bbox?))).collect();
            m.sort_by(|a, b| a.0.cmp(b.0));
            m.into_iter().map(|(_, b)| b).collect()
        };
        let (new, faces) = (made("BODY"), made("FACE"));
        let unit = per_unit(&g["units"]);
        let (o, x, y) = (f3(&g["frame"]["origin"])?.map(|c| c * unit), f3(&g["frame"]["x"])?, f3(&g["frame"]["y"])?);
        let n = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let corners = |[lo, hi]: [[f64; 3]; 2]| (0..8).map(move |i| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }]);
        // A box seen along the sketch's normal, in the sketch plane.
        let footprint = |b: [[f64; 3]; 2]| -> Box2 {
            corners(b).map(|p| [dot([p[0] - o[0], p[1] - o[1], p[2] - o[2]], x), dot([p[0] - o[0], p[1] - o[1], p[2] - o[2]], y)]).fold(([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]), |(a, b), p| ([a[0].min(p[0]), a[1].min(p[1])], [b[0].max(p[0]), b[1].max(p[1])]))
        };
        let bounds = |r: &Value| -> Option<Box2> {
            let v = |a: &Value, i: usize| a[i].as_f64().map(|c| c * unit);
            let (lo, hi) = (&r["bounds"]["min"], &r["bounds"]["max"]);
            Some(([v(lo, 0)?, v(lo, 1)?], [v(hi, 0)?, v(hi, 1)?]))
        };
        let regions: Vec<(&str, Box2)> = g["regions"].as_array()?.iter().filter(|r| r["imprinted"] != true).filter_map(|r| Some((r["id"].as_str()?, bounds(r)?))).collect();
        // Live's box may come from the tessellation (the UI fetch): a gear's
        // is short of its tips.
        let tol = |(min, max): Box2| 1e-6_f64.max(1e-3 * (max[0] - min[0]).hypot(max[1] - min[1]));
        let off = |(min, max): Box2, (rl, rh): &Box2| (0..2).map(|i| (rl[i] - min[i]).abs().max((rh[i] - max[i]).abs())).fold(0.0, f64::max);
        // The region with that box (a hole's disc inside the same circle has a smaller one).
        let closest = |f: Box2| regions.iter().filter(|(_, b)| off(f, b) < tol(f)).min_by(|a, b| off(f, &a.1).total_cmp(&off(f, &b.1))).map(|(id, _)| id.to_string());
        // One region added to a part (no new body): the cap it made, a new
        // face flat along the sketch's normal.
        if new.is_empty() && ids == 1 && caps {
            let flat = |b: &[[f64; 3]; 2]| {
                let along: Vec<f64> = corners(*b).map(|p| dot(p, n)).collect();
                along.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - along.iter().cloned().fold(f64::INFINITY, f64::min) < 1e-7
            };
            let caps: Vec<Box2> = faces.iter().filter(|b| flat(b)).map(|b| footprint(*b)).collect();
            // Else the one region holding the cap (clipped where it merged
            // into the part).
            let holding = |f: &Box2| -> Option<String> {
                let t = tol(*f);
                match regions.iter().filter(|(_, (rl, rh))| (0..2).all(|i| rl[i] < f.0[i] + t && rh[i] > f.1[i] - t)).collect::<Vec<_>>().as_slice() {
                    [(id, _)] => Some(id.to_string()),
                    _ => None,
                }
            };
            return caps.iter().find_map(|f| closest(*f)).or_else(|| caps.iter().find_map(holding)).map(|id| vec![id]);
        }
        if new.is_empty() || new.len() > ids {
            return None;
        }
        let mut out = Vec::new();
        for b in new {
            let f = footprint(b);
            if let Some(id) = closest(f) {
                out.push(id);
                continue;
            }
            // Regions inside the box that together fill it (a board a line splits).
            let t = tol(f);
            let inside: Vec<&(&str, Box2)> = regions.iter().filter(|(_, (rl, rh))| (0..2).all(|i| rl[i] > f.0[i] - t && rh[i] < f.1[i] + t)).collect();
            let union = inside.iter().fold(([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]), |(a, b), (_, (rl, rh))| ([a[0].min(rl[0]), a[1].min(rl[1])], [b[0].max(rh[0]), b[1].max(rh[1])]));
            if inside.len() < 2 || off(f, &union) >= t {
                return None;
            }
            out.extend(inside.iter().map(|(id, _)| id.to_string()));
        }
        (out.len() == ids).then_some(out)
    }

    /// The regions of one sketch that the parts feature `k` made cover (by
    /// their boxes), when they are other regions than `ids`.
    fn regions_by_bodies(&mut self, k: usize, ids: &[String]) -> Option<Vec<String>> {
        let sketch = ids.first()?.strip_prefix("region:")?.split(':').next()?.to_string();
        if ids.iter().any(|i| i.strip_prefix("region:").and_then(|r| r.split(':').next()) != Some(sketch.as_str())) {
            return None;
        }
        let g = self.call("sketch_get", json!({ "sketch": sketch })).ok()?;
        let found = self.regions_of_new_bodies(k, &sketch, &g, ids.len(), false)?;
        // The same regions, though the face's edges split them (keys from 2^30).
        let own = |id: &String| -> String {
            let Some((head, keys)) = id.rsplit_once(':') else { return id.clone() };
            format!("{head}:{}", keys.split(',').filter(|k| k.parse::<u64>().is_ok_and(|v| v < 1 << 30)).collect::<Vec<_>>().join(","))
        };
        let set = |v: &[String]| v.iter().map(own).collect::<std::collections::BTreeSet<_>>();
        (set(&found) != set(ids)).then_some(found)
    }

    /// The part of ours (among `ours`) that our copy of the feature which made live's part `id` made, when that
    /// feature made one. Onshape gives out ids in creation order ([`onshape_counter`]): the maker is the feature that
    /// made one of our parts before the first feature naming a later id, with the latest references below it.
    fn part_by_creator(&self, k: usize, id: &str, ours: &[String]) -> Option<String> {
        let value = onshape_counter(id)?;
        let refs = |f: &Value| -> Vec<u32> {
            let mut out = Vec::new();
            let mut stack = vec![f];
            while let Some(v) = stack.pop() {
                match v {
                    Value::Object(m) => {
                        if let Some(a) = m.get("deterministicIds").and_then(Value::as_array) {
                            out.extend(a.iter().filter_map(Value::as_str).filter_map(onshape_counter));
                        }
                        stack.extend(m.values());
                    }
                    Value::Array(a) => stack.extend(a),
                    _ => {}
                }
            }
            out
        };
        let made = |f: &Value| -> Vec<&String> {
            let Some(theirs) = f["featureId"].as_str().and_then(|o| self.feature_ids.get(o)) else { return Vec::new() };
            let head = format!("body:{theirs}.");
            ours.iter().filter(|p| p.starts_with(&head)).collect()
        };
        // Made before the first feature that names a later id, by the feature whose own references come last below it.
        let upto = k.min(self.features.len());
        let before = self.features[..upto].iter().position(|f| refs(f).iter().any(|r| *r > value)).unwrap_or(upto);
        let mut candidates: Vec<(u32, &Value)> = self.features[..before].iter().filter(|f| !made(f).is_empty()).filter_map(|f| refs(f).into_iter().max().map(|m| (m, f))).filter(|(m, _)| *m < value).collect();
        candidates.sort_by_key(|(m, _)| std::cmp::Reverse(*m));
        match candidates.as_slice() {
            [(m, maker), rest @ ..] if rest.first().is_none_or(|(n, _)| n < m) => match made(maker).as_slice() {
                [one] => Some(one.to_string()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Our solid parts, when `queries` are id-only and name as many (no live geometry to tell them apart); else each
    /// by the feature that made it.
    fn parts_by_count(&mut self, k: usize, queries: &[Value]) -> Option<Vec<String>> {
        if self.part_order.is_empty() || queries.iter().any(|q| q["queryString"].as_str().is_some_and(|t| !t.trim().is_empty())) {
            return None;
        }
        let ids: Vec<String> = queries.iter().flat_map(|q| q["deterministicIds"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string))).collect();
        let ours = self.call("query_parts", json!({})).ok()?;
        let all: Vec<String> = ours["parts"].as_array()?.iter().filter(|p| p["kind"] == "Solid").filter_map(|p| p["id"].as_str().map(str::to_string)).collect();
        if ids.is_empty() || all.len() == ids.len() {
            return (!ids.is_empty()).then_some(all);
        }
        let by_creator: Vec<String> = ids.iter().filter_map(|id| self.part_by_creator(k, id, &all)).collect();
        (by_creator.len() == ids.len() && by_creator.iter().collect::<std::collections::HashSet<_>>().len() == ids.len()).then_some(by_creator)
    }

    /// A revolve's axis named by id alone: the last sketch's one
    /// construction line (the usual centreline), if it has exactly one.
    fn last_sketch_axis(&mut self, k: usize, queries: &[Value]) -> Option<Vec<String>> {
        if queries.len() != 1 || queries[0]["queryString"].as_str().is_some_and(|t| !t.trim().is_empty()) {
            return None;
        }
        let sketch = self.features[..k].iter().rev().find(|f| f["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketch")))?;
        let ours = self.feature_ids.get(sketch["featureId"].as_str()?)?.clone();
        let g = self.call("sketch_get", json!({ "sketch": ours })).ok()?;
        let entities = g["entities"].as_array()?;
        let lines: Vec<String> = entities.iter().filter(|e| e["type"] == "line" && e["construction"] == true && e.get("handle_of").is_none()).filter_map(|e| e["id"].as_str().map(str::to_string)).collect();
        if lines.len() == 1 {
            return Some(lines);
        }
        if !lines.is_empty() || self.part_order.is_empty() {
            return None;
        }
        // No centreline (an assembly corpus' studio, no live geometry): the profile's longest side with all of the
        // profile on one side of it (a screw's flat along its axis).
        let pt = |v: &Value| Some([v[0].as_f64()?, v[1].as_f64()?]);
        let mut points: Vec<[f64; 2]> = Vec::new();
        for e in entities.iter().filter(|e| e["construction"] != true) {
            match e["type"].as_str() {
                Some("line") => points.extend([pt(&e["start"])?, pt(&e["end"])?]),
                Some("arc") => {
                    let (c, a, b) = (pt(&e["center"])?, pt(&e["start"])?, pt(&e["end"])?);
                    let r = (a[0] - c[0]).hypot(a[1] - c[1]);
                    let (t0, mut t1) = ((a[1] - c[1]).atan2(a[0] - c[0]), (b[1] - c[1]).atan2(b[0] - c[0]));
                    if t1 <= t0 {
                        t1 += std::f64::consts::TAU;
                    }
                    points.extend((0..=16).map(|i| t0 + (t1 - t0) * i as f64 / 16.0).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]));
                }
                Some("point") => {}
                _ => return None,
            }
        }
        let tol = 1e-9 * points.iter().map(|p| p[0].abs().max(p[1].abs())).fold(1.0, f64::max);
        let mut sides: Vec<(f64, String)> = entities
            .iter()
            .filter(|e| e["type"] == "line" && e["construction"] != true)
            .filter_map(|e| {
                let (a, b) = (pt(&e["start"])?, pt(&e["end"])?);
                let len = (b[0] - a[0]).hypot(b[1] - a[1]);
                let side = |p: &[f64; 2]| ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) / len;
                let one_side = points.iter().all(|p| side(p) > -tol) || points.iter().all(|p| side(p) < tol);
                (len > 0.0 && one_side).then(|| (len, e["id"].as_str().unwrap_or("").to_string()))
            })
            .collect();
        sides.sort_by(|a, b| b.0.total_cmp(&a.0));
        match sides.as_slice() {
            [(l, id), rest @ ..] if rest.first().is_none_or(|(m, _)| *m < 0.99 * l) => Some(vec![id.clone()]),
            _ => None,
        }
    }

    /// The one curve bounding a sketch region, if one does.
    fn region_curve(&mut self, region: &str) -> Option<String> {
        let sketch = region.strip_prefix("region:")?.split(':').next()?;
        let g = self.call("sketch_get", json!({ "sketch": sketch })).ok()?;
        let r = g["regions"].as_array()?.iter().find(|r| r["id"] == region)?;
        match r["boundary"].as_array()?.as_slice() {
            [one] => one.as_str().map(str::to_string),
            _ => None,
        }
    }

    /// One Onshape parameter as feature_add takes it (`None`: nothing to set).
    fn param(&mut self, k: usize, p: &Value) -> Result<Option<Value>, String> {
        let ty = p["btType"].as_str().unwrap_or("");
        let kind = ty.split('-').next().unwrap_or(ty);
        Ok(match kind {
            "BTMParameterQuantity" => Some(json!(p["expression"].as_str().ok_or("no expression")?)),
            // With MBD tolerance fields (no effect on the geometry).
            "BTMParameterTolerantQuantity" => {
                let toleranced = p["toleranceParameters"].as_array().into_iter().flatten().any(|t| t["parameterId"] == "toleranceType" && t["value"] != "DEFAULT" && t["value"] != "NONE");
                if toleranced {
                    self.notes.push(format!("{}: its tolerance not translated (no effect on the geometry)", p["parameterId"].as_str().unwrap_or("")));
                }
                Some(json!(p["expression"].as_str().ok_or("no expression")?))
            }
            "BTMParameterNullableQuantity" if p["isNull"].as_bool() == Some(true) => None,
            "BTMParameterNullableQuantity" => Some(json!(p["expression"].as_str().ok_or("no expression")?)),
            "BTMParameterEnum" | "BTMParameterBoolean" | "BTMParameterString" => Some(p["value"].clone()),
            "BTMParameterQueryList" => {
                self.picked.clear();
                let mut queries: Vec<Value> = p["queries"].as_array().cloned().unwrap_or_default();
                // A query live's fetch names no ids for, evaluated by live for this parameter (refs.jsonl): its ids.
                let pid = p["parameterId"].as_str().unwrap_or("");
                let mut evaluated: Vec<String> = self.refs.iter().filter(|((i, _), r)| *i == k && r.parameter.as_deref() == Some(pid)).map(|((_, id), _)| id.clone()).collect();
                evaluated.sort();
                if !evaluated.is_empty() {
                    for q in queries.iter_mut().filter(|q| q["deterministicIds"].as_array().is_none_or(|a| a.is_empty())) {
                        *q = json!({ "btType": q["btType"], "deterministicIds": evaluated, "queryString": "" });
                    }
                }
                let mut ids = Vec::new();
                let mut missed = Vec::new();
                for q in &queries {
                    match self.query(k, q) {
                        Ok(found) => ids.extend(found),
                        Err(e) => missed.push(match q["deterministicIds"].as_array().filter(|a| !a.is_empty()) {
                            Some(d) => format!("{} ({})", e, d.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")),
                            None => e,
                        }),
                    }
                }
                let regions_param = ["entities", "surfaceEntities", "regions", "faceToAdd"].contains(&p["parameterId"].as_str().unwrap_or(""));
                let Some(first) = missed.first().cloned() else {
                    // Regions by their curves, checked against the parts the
                    // feature made (live's line after it): its boxes win when
                    // they tell other regions (clamp-guillem's band around a
                    // rectangle names the rectangle's sides too).
                    // (Not when live's own regions placed every id: its sketch bodies, live 2267. A body's box covers
                    // regions it doesn't use: asm-suspension's Knuckle around its centre disc.)
                    let exact = queries.iter().all(|q| {
                        let qids: Vec<&str> = q["deterministicIds"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                        !qids.is_empty() && qids.iter().all(|id| (0..=k).any(|j| self.refs.get(&(j, id.to_string())).is_some_and(|r| r.sketch_file && r.kind == "SKETCH_REGION")))
                    });
                    if regions_param
                        && !exact
                        && let Some(better) = self.regions_by_bodies(k, &ids)
                    {
                        return Ok(Some(json!(better)));
                    }
                    return Ok((!ids.is_empty()).then(|| json!(ids)));
                };
                // Regions named by id alone (older documents' queries carry no
                // text, and live's bodies have no sketch regions): as many as
                // the last sketch has, they are its regions.
                if ["entities", "surfaceEntities", "regions", "faceToAdd"].contains(&p["parameterId"].as_str().unwrap_or(""))
                    && let Some(r) = self.last_sketch_regions(k, &queries)
                {
                    return Ok(Some(json!(r)));
                }
                if p["parameterId"] == "axis"
                    && ids.is_empty()
                    && let Some(a) = self.last_sketch_axis(k, &queries)
                {
                    return Ok(Some(json!(a)));
                }
                // A merge scope of part ids alone, without live's geometry (an assembly corpus' studio): as many parts as
                // we have then are ours.
                if p["parameterId"] == "booleanScope"
                    && ids.is_empty()
                    && let Some(all) = self.parts_by_count(k, &queries)
                {
                    return Ok(Some(json!(all)));
                }
                if ids.is_empty() {
                    return Err(first);
                }
                // The others as found: a query that finds nothing adds nothing
                // (in Onshape too), said per query.
                let param = p["parameterId"].as_str().unwrap_or("").to_string();
                self.partial.extend(missed.into_iter().map(|e| format!("{param}: one of its queries: {e}")));
                Some(json!(ids))
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
                let default = values
                    .iter()
                    .find(|v| v["enumValue"] == want || v["booleanValue"].as_bool().is_some_and(|b| b.to_string() == want))
                    .or_else(|| values.first())
                    .ok_or("configured parameter without values")?;
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
            // A Frame's profile sketch in another Part Studio of the document: ours `<tab id>#<sketch>.4294967295`
            // (psui's Select sketch › Current document), that studio built first.
            "BTMParameterReferencePartStudio" if p["parameterId"] == "profileSketch" => Some(json!(self.studio_sketch(p)?)),
            _ => return Err(format!("parameter type {ty} not translated")),
        })
    }

    /// [`Self::param`]'s `profileSketch` of another Part Studio: its element from the namespace ("e<element>::m…"),
    /// its sketch from the query's feature. That studio is built at its default configuration: another one is reported.
    fn studio_sketch(&self, p: &Value) -> Result<String, String> {
        let ns = p["namespace"].as_str().unwrap_or("");
        let element = ns.split("::").find_map(|x| x.strip_prefix('e')).ok_or_else(|| format!("profile studio `{ns}`: no element"))?;
        let (tab, ids) = self.other_studios.get(element).ok_or_else(|| format!("profile studio {element} not built"))?;
        let q = &p["partQuery"]["queries"][0];
        let fid = q["featureId"].as_str().ok_or("profile sketch: no feature in its query")?;
        let sketch = ids.get(fid).ok_or_else(|| format!("profile sketch {fid} was not built"))?;
        if p["configuration"].as_array().is_some_and(|c| !c.is_empty()) && !self.other_studio_default(element, &p["configuration"]) {
            return Err("profile studio: a configuration other than its default".into());
        }
        Ok(format!("{tab}#{sketch}.{}", u32::MAX))
    }

    /// Whether `config` (live's parameter list) is the source studio's default (its studio JSON in `other_defaults`).
    fn other_studio_default(&self, element: &str, config: &Value) -> bool {
        let Some(defaults) = self.other_defaults.get(element) else { return false };
        config.as_array().into_iter().flatten().all(|c| c["parameterId"].as_str().is_some_and(|id| defaults.get(id) == Some(&c["value"])))
    }

    /// Our reference ids for one Onshape query.
    /// A query's entities of ours. Ids that only a sketch body places (live's sketch wire bodies and regions) go
    /// after the query's text: the sketch entity it names places them better than the geometry (a revolve's axis
    /// line, a region among its neighbours).
    fn query(&mut self, k: usize, q: &Value) -> Result<Vec<String>, String> {
        let ids: Vec<&str> = q["deterministicIds"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let sketch_only = !ids.is_empty() && ids.iter().all(|id| (0..=k).any(|j| self.refs.get(&(j, id.to_string())).is_some_and(|r| r.sketch)));
        // Sketch regions with live's own geometry (its sketch bodies file: area and box, live 2267): the text's regions
        // while their area is live's for these ids, else the ids' (a region query's text names all of its sketch's
        // regions, qSketchRegion(…, true); asm-suspension's Extrude 1 ring, whose text names the disc inside it).
        let live_area: Option<f64> = ids
            .iter()
            .map(|id| (0..=k).rev().find_map(|j| self.refs.get(&(j, id.to_string())).filter(|r| r.kind == "SKETCH_REGION" && r.sketch_file).and_then(|r| r.measure)))
            .sum();
        if sketch_only && let Some(want) = live_area {
            let text = self.query_text_first(k, q);
            if let Ok(found) = &text
                && self.regions_area(found).is_some_and(|a| (a - want).abs() <= 1e-3 * want)
            {
                return text;
            }
            if let Ok(found) = self.query_with(k, q, true)
                && !found.is_empty()
            {
                return Ok(found);
            }
            return text;
        }
        self.query_text_first(k, q)
    }

    /// [`Self::query`] for ids only a sketch body places: the query's text first, then its ids.
    fn query_text_first(&mut self, k: usize, q: &Value) -> Result<Vec<String>, String> {
        let ids: Vec<&str> = q["deterministicIds"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let sketch_only = !ids.is_empty() && ids.iter().all(|id| (0..=k).any(|j| self.refs.get(&(j, id.to_string())).is_some_and(|r| r.sketch)));
        if sketch_only && !q["queryString"].as_str().unwrap_or("").is_empty()
            && let Ok(found) = self.query_with(k, q, false)
            && !found.is_empty()
            && !self.names_stale_curves(q["queryString"].as_str().unwrap_or(""))
        {
            return Ok(found);
        }
        // A sketch entity named in the text is that entity, before anything lying where live's id is (c2-rings-loft's
        // Revolve 1 axis: the Axis sketch's line, not a construction point of the profile's sketch on it).
        if let Some(e) = q["queryString"].as_str().and_then(|t| self.sketch_entity_from_query(t)) {
            return Ok(vec![e]);
        }
        self.query_with(k, q, true)
    }

    /// Our sketch `id`'s sketch_get (cached within a step); None for a feature that isn't a sketch.
    fn sketch_geometry(&mut self, id: &str) -> Option<Value> {
        if let Some(g) = self.sketch_cache.get(id) {
            return (!g.is_null()).then(|| g.clone());
        }
        let g = self.call("sketch_get", json!({ "sketch": id })).unwrap_or(Value::Null);
        self.sketch_cache.insert(id.to_string(), g.clone());
        (!g.is_null()).then_some(g)
    }

    /// The total area (m²) of our sketch regions `ids` ("region:<sketch>:<keys>"), when all are regions.
    fn regions_area(&mut self, ids: &[String]) -> Option<f64> {
        let mut total = 0.0;
        for id in ids {
            let sketch = id.strip_prefix("region:")?.split(':').next()?.to_string();
            let g = self.sketch_geometry(&sketch)?;
            let k = per_unit(&g["units"]);
            let r = g["regions"].as_array()?.iter().find(|r| r["id"] == id.as_str())?;
            total += r["area"].as_f64()? * k * k;
        }
        (!ids.is_empty()).then_some(total)
    }

    /// Whether a query's text names sketch curves its sketch doesn't have (written when the sketch held others: c2-duct-
    /// multipart's Extrude 3 names Sketch 5's side triangles by curves since redrawn), so its ids place it better.
    fn names_stale_curves(&self, text: &str) -> bool {
        let Some(packed) = text.split("qCompressed(1.0,\"").nth(1).and_then(|t| t.split('"').next()) else { return false };
        let Ok(tree) = noshape_model::custom::host::packed::unpack(packed) else { return false };
        let mut strings = Vec::new();
        packed_strings(&tree, &mut strings);
        let mut sketch: Option<String> = None;
        let mut stale = false;
        for w in strings.windows(2) {
            if let Some(sk) = w[0].strip_suffix(".wireOp") {
                sketch = Some(sk.to_string());
            }
            if w[0] == "sketchEntityId"
                && let Some(sk) = sketch.as_ref().filter(|sk| self.feature_ids.contains_key(*sk))
                && self.curve_of(sk, &w[1]).is_none()
                && !self.sketch_entities.contains_key(&(sk.clone(), w[1].clone()))
            {
                stale = true;
            }
        }
        stale
    }

    fn query_with(&mut self, k: usize, q: &Value, use_ids: bool) -> Result<Vec<String>, String> {
        // A mate connector by the feature that made it (a Hole's in-feature
        // one): its id may name a face too (prize-wheel-tim).
        if q["btType"].as_str().is_some_and(|t| t.starts_with("BTMIndividualCreatedByQuery"))
            && q["bodyType"] == "MATE_CONNECTOR"
            && let Some(f) = q["featureId"].as_str()
        {
            if self.sub_params.contains_key(f) {
                return Ok(vec![format!("{SUB_FEATURE}{f}")]);
            }
            if let Some(ours) = self.feature_ids.get(f) {
                return Ok(vec![format!("mc:{ours}.0")]);
            }
        }
        // A sketch's vertices (Point pattern's locations): its curves' ends
        // (construction ones too) and its points, one per position.
        if q["btType"].as_str().is_some_and(|t| t.starts_with("BTMIndividualSketchUniqueVerticesQuery"))
            && let Some(ours) = q["featureId"].as_str().and_then(|f| self.feature_ids.get(f)).cloned()
        {
            let g = self.call("sketch_get", json!({ "sketch": ours }))?;
            let entities: Vec<&Value> = g["entities"].as_array().into_iter().flatten().collect();
            let ends: Vec<&str> = entities.iter().flat_map(|e| [&e["start_point"], &e["end_point"]]).filter_map(Value::as_str).collect();
            let centres: Vec<&str> = entities.iter().filter_map(|e| e["center_point"].as_str()).collect();
            let mut seen: Vec<[f64; 2]> = Vec::new();
            let mut out = Vec::new();
            for e in entities.iter().filter(|e| e["type"] == "point") {
                let Some(id) = e["id"].as_str() else { continue };
                let end = ends.contains(&id);
                if !end && (centres.contains(&id) || e["construction"] == true) {
                    continue;
                }
                let Some(at) = e["at"][0].as_f64().zip(e["at"][1].as_f64()).map(|(u, v)| [u, v]) else { continue };
                if !seen.iter().any(|p| (p[0] - at[0]).hypot(p[1] - at[1]) < 1e-9) {
                    seen.push(at);
                    out.push(id.to_string());
                }
            }
            return Ok(out);
        }
        let ids: Vec<String> = q["deterministicIds"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
        let text = q["queryString"].as_str().unwrap_or("");
        if ids.is_empty() && text.is_empty() && q["btType"].as_str().is_some_and(|t| t.starts_with("BTMIndividualQuery-")) {
            return Err("an empty query in live's fetch (no ids, no query)".into());
        }
        // A sketch region query without its text (live's UI fetch): the
        // regions of its sketch, as Onshape writes it.
        let written;
        let mut named = None;
        let text = match (text.is_empty(), q["btType"].as_str().is_some_and(|t| t.starts_with("BTMIndividualSketchRegionQuery")), q["featureId"].as_str()) {
            (true, true, Some(sk)) => {
                written = format!("query = qSketchRegion(id + \"{sk}\", {});", q["filterInnerLoops"] == true);
                named = (!ids.is_empty()).then_some(ids.len());
                written.as_str()
            }
            _ => text,
        };
        // Not placed just before feature k (live's line after a sketch has no
        // bodies): where an earlier line has them all (an id names one entity).
        let k = if !ids.is_empty() && !ids.iter().all(|id| self.refs.contains_key(&(k, id.clone()))) {
            (0..k).rev().find(|j| ids.iter().all(|id| self.refs.contains_key(&(*j, id.clone())))).unwrap_or(k)
        } else {
            k
        };
        if use_ids && !ids.is_empty() && ids.iter().all(|id| self.refs.contains_key(&(k, id.clone()))) {
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
        // Part ids alone, without live's geometry (an assembly corpus' studio): when they can only be ours: our one
        // part, or as many ids as we have parts (ranks among the final parts shift once a part merges away).
        if !ids.is_empty() && ids.iter().all(|id| self.part_order.contains(id)) {
            let ours = self.call("query_parts", json!({}))?;
            let all: Vec<String> = ours["parts"].as_array().into_iter().flatten().filter(|p| p["kind"] == "Solid").filter_map(|p| p["id"].as_str().map(str::to_string)).collect();
            if all.len() == ids.len() {
                return Ok(all);
            }
            // Else each by the feature that made it (`part_by_creator`).
            let by_creator: Vec<String> = ids.iter().filter_map(|id| self.part_by_creator(k, id, &all)).collect();
            if by_creator.len() == ids.len() && by_creator.iter().collect::<std::collections::HashSet<_>>().len() == ids.len() {
                return Ok(by_creator);
            }
        }
        // A Plane feature's plane (`dummyQuery(id+"F….planeOp", …)`).
        if let Some(fid) = text.split("id+\"").nth(1).and_then(|r| r.split(".planeOp\"").next()).filter(|f| !f.contains('"'))
            && let Some(ours) = self.feature_ids.get(fid)
        {
            return Ok(vec![format!("fplane:{ours}")]);
        }
        // The Origin, packed (`DUMMY` of "Origin.pointOp": c2-housing-draft's Bohrung 1, its in-feature mate connector).
        if ids.is_empty() && packed_strings_of(text).iter().any(|s| s == "Origin.pointOp") {
            return Ok(vec!["origin".into()]);
        }
        // A Plane feature's plane, packed (`DUMMY` of "F….planeOp").
        if let Some(fid) = packed_strings_of(text).iter().find_map(|s| s.strip_suffix(".planeOp").map(str::to_string))
            && let Some(ours) = self.feature_ids.get(&fid)
        {
            return Ok(vec![format!("fplane:{ours}")]);
        }
        // A Mate connector feature's connector (`dummyQuery(id+"F….mateConnectorOp", …)`), written or packed.
        if let Some(fid) = text.split("id+\"").nth(1).and_then(|r| r.split(".mateConnectorOp\"").next()).filter(|f| !f.contains('"')).map(str::to_string).or_else(|| packed_strings_of(text).iter().find_map(|s| s.strip_suffix(".mateConnectorOp").map(str::to_string)))
            && let Some(ours) = self.feature_ids.get(&fid)
        {
            return Ok(vec![format!("mc:{ours}.0")]);
        }
        if let Some(r) = self.blended_face_from_query(text) {
            return r.map(|e| vec![e]);
        }
        if let Some(r) = self.blend_vertex_from_query(text) {
            return r.map(|e| vec![e]);
        }
        if let Some(r) = self.imprinted_vertex_from_query(text) {
            return r.map(|e| vec![e]);
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
        if let Some(r) = self.hole_face_from_query(text) {
            return r.map(|e| vec![e]);
        }
        if let Some(r) = self.fit_spline_vertex_from_query(text) {
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
        // qContainsPoint(…EntityType.EDGE / FACE / VERTEX…, vector(x, y, z) * unit),
        // or of qSketchRegion(makeId("F…")): the sketch's region holding the point.
        if let Some(rest) = text.split("qContainsPoint(").nth(1)
            && let Some(v) = rest.split("vector(").nth(1)
        {
            let (nums, unit) = v.split_once(')').unwrap_or((v, ""));
            let c: Vec<f64> = nums.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            let scale = if unit.contains("millimeter") { 1e-3 } else if unit.contains("inch") { 0.0254 } else if unit.contains("centimeter") { 1e-2 } else { 1.0 };
            if let Some(sk) = rest.strip_prefix("qSketchRegion(makeId(\"").and_then(|r| r.split('"').next())
                && c.len() == 3
            {
                let ours = self.feature_ids.get(sk).cloned().ok_or_else(|| format!("sketch {sk} was not built"))?;
                let g = self.call("sketch_get", json!({ "sketch": ours, "at": mm([c[0] * scale, c[1] * scale, c[2] * scale]) }))?;
                let found: Vec<String> = g["regions"].as_array().into_iter().flatten().filter(|r| r["imprinted"] != true).filter_map(|r| r["id"].as_str().map(str::to_string)).collect();
                return if found.is_empty() { Err(format!("no region of sketch {sk} holds live's point")) } else { Ok(found) };
            }
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
        // with `true` none inside another's hole (its outer loop is a hole of
        // another region; live: bearing-block-takumi's rings in the bore). The
        // sketch's own curves decide: a piece of a ring that the face's edge
        // split off is no inner loop (live motor-mount-jack's Extrude 3), a
        // piece of an inner region is.
        // (Written `id + "F…"` or `id+"F…"`.)
        fn region_of(t: &str) -> Option<(&str, &str)> {
            t.split("qSketchRegion(").skip(1).find_map(|r| r.trim_start().strip_prefix("id")?.trim_start().strip_prefix('+')?.trim_start().strip_prefix('"')?.split_once('"'))
        }
        if let Some((sk, rest)) = region_of(text) {
            let ours = self.feature_ids.get(sk).cloned().ok_or_else(|| format!("sketch {sk} was not built"))?;
            let g = self.call("sketch_get", json!({ "sketch": ours }))?;
            // (Not the face's imprinted pieces: sketch faces, not regions.)
            let regions: Vec<&Value> = g["regions"].as_array().into_iter().flatten().filter(|r| r["imprinted"] != true).collect();
            let curves = |r: &Value| -> Vec<String> { r["boundary"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
            let outer_only = rest.trim_start_matches([',', ' ']).starts_with("true");
            let outer = |r: &Value| -> Vec<String> { r["outer"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
            let own = |c: &String| c.rsplit(':').next().and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| !noshape_model::regen::features::sketch::is_imprint_entity(n));
            let in_hole_of = |r: &Value, o: &Value, mine: &[String]| {
                let theirs = outer(o);
                let holes: Vec<String> = curves(o).into_iter().filter(|c| !theirs.contains(c)).collect();
                o["id"] != r["id"] && !mine.is_empty() && mine.iter().all(|c| holes.contains(c))
            };
            let inner_by_own = |r: &Value| {
                let mine: Vec<String> = outer(r).into_iter().filter(own).collect();
                regions.iter().any(|o| in_hole_of(r, o, &mine))
            };
            let inner = |r: &Value| {
                let mine = outer(r);
                if mine.iter().any(own) {
                    return inner_by_own(r);
                }
                regions.iter().any(|o| inner_by_own(o) && in_hole_of(r, o, &mine))
            };
            let found: Vec<String> = regions.iter().filter(|r| !outer_only || !inner(r)).filter_map(|r| r["id"].as_str().map(str::to_string)).collect();
            // Some of the sketch's regions named by id (one id each): which
            // is left to the bodies they made.
            // (Live naming more regions than ours has, without live's geometry: all of ours, which cover live's when
            // its sketch splits them finer. asm 11-07, assembly corpus studios.)
            if let Some(n) = named.filter(|n| *n < found.len()) {
                return Err(format!("query not mapped (no live geometry for {n} of the sketch's {} regions)", found.len()));
            }
            return Ok(found);
        }
        Err(format!("query not mapped (no live geometry for {}): {}", if ids.is_empty() { "it".to_string() } else { ids.join(", ") }, text.chars().take(80).collect::<String>()))
    }

    /// Where an [`EdgeSide`] points in our sketch (its coordinates): None where its curves aren't ours, aren't lines,
    /// circles or arcs, or don't meet at one point.
    fn edge_side_probe(&self, sk: &str, g: &Value, q: &EdgeSide) -> Option<glam::DVec2> {
        use glam::DVec2;
        #[derive(Clone, Copy)]
        enum C {
            Line(DVec2, DVec2),
            /// Centre, radius and an arc's start angle and sweep (anticlockwise).
            Circle(DVec2, f64, Option<(f64, f64)>),
        }
        let pt = |v: &Value| Some(DVec2::new(v[0].as_f64()?, v[1].as_f64()?));
        let curve = |live: &str| -> Option<C> {
            let ours = self.sketch_entities.get(&(sk.to_string(), live.to_string()))?;
            let e = g["entities"].as_array()?.iter().find(|e| e["id"] == ours.as_str())?;
            match e["type"].as_str()? {
                "line" => Some(C::Line(pt(&e["start"])?, pt(&e["end"])?)),
                "circle" => Some(C::Circle(pt(&e["center"])?, e["radius"].as_f64()?, None)),
                "arc" => {
                    let c = pt(&e["center"])?;
                    let (a, b) = (pt(&e["start"])? - c, pt(&e["end"])? - c);
                    let a0 = a.y.atan2(a.x);
                    let sweep = (b.y.atan2(b.x) - a0).rem_euclid(std::f64::consts::TAU);
                    Some(C::Circle(c, a.length(), Some((a0, if sweep == 0.0 { std::f64::consts::TAU } else { sweep }))))
                }
                _ => None,
            }
        };
        let size = |c: C| match c {
            C::Line(a, b) => a.distance(b),
            C::Circle(_, r, _) => r,
        };
        let tangent = |c: C, p: DVec2| match c {
            C::Line(a, b) => (b - a).normalize_or_zero(),
            C::Circle(o, _, _) => (p - o).perp().normalize_or_zero(),
        };
        let edge = curve(&q.edge)?;
        let tol = 1e-6 * size(edge).max(1.0);
        let on = |c: C, p: DVec2| match c {
            C::Line(a, b) => {
                let d = b - a;
                (p - (a + d * ((p - a).dot(d) / d.length_squared().max(1e-300)).clamp(0.0, 1.0))).length() <= tol
            }
            C::Circle(o, r, arc) => (p.distance(o) - r).abs() <= tol && arc.is_none_or(|(a0, sweep)| ((p - o).to_angle() - a0).rem_euclid(std::f64::consts::TAU) <= sweep + tol / r || ((p - o).to_angle() - a0).rem_euclid(std::f64::consts::TAU) >= std::f64::consts::TAU - tol / r),
        };
        // Where two curves meet (a tangent touch once).
        let meet = |a: C, b: C| -> Vec<DVec2> {
            let mut pts = match (a, b) {
                (C::Line(p, q), C::Line(u, v)) => {
                    let (d, e) = (q - p, v - u);
                    let den = d.perp_dot(e);
                    if den.abs() < 1e-300 { Vec::new() } else { vec![p + d * ((u - p).perp_dot(e) / den)] }
                }
                (C::Line(p, q), C::Circle(o, r, _)) | (C::Circle(o, r, _), C::Line(p, q)) => {
                    let d = (q - p).normalize_or_zero();
                    let t0 = (o - p).dot(d);
                    let h2 = r * r - (p + d * t0 - o).length_squared();
                    if h2 < -tol * r { Vec::new() } else { let h = h2.max(0.0).sqrt(); vec![p + d * (t0 - h), p + d * (t0 + h)] }
                }
                (C::Circle(o1, r1, _), C::Circle(o2, r2, _)) => {
                    let dist = o1.distance(o2);
                    if dist < 1e-300 { Vec::new() } else {
                        let a = (dist * dist + r1 * r1 - r2 * r2) / (2.0 * dist);
                        let h2 = r1 * r1 - a * a;
                        let (m, n) = (o1 + (o2 - o1) * (a / dist), (o2 - o1).perp() / dist);
                        if h2 < -tol * r1 { Vec::new() } else { let h = h2.max(0.0).sqrt(); vec![m - n * h, m + n * h] }
                    }
                }
            };
            pts.retain(|p| on(a, *p) && on(b, *p));
            pts.dedup_by(|x, y| x.distance(*y) <= tol);
            pts
        };
        // (A twentieth of the edge along it, on it; to the side by half the other curve's distance there, at most a
        // twentieth of that: between the two where they touch (a tangent line's cusp beside a circle), and clear of
        // the regions' tessellation, 1e-5 of their size.)
        let step = 0.05 * size(edge);
        let along = |p: DVec2, by: f64| match edge {
            C::Line(a, b) => p + (b - a).normalize_or_zero() * by,
            C::Circle(o, r, _) => o + DVec2::from_angle(by / r).rotate(p - o),
        };
        let distance = |c: C, p: DVec2| match c {
            C::Line(a, b) => {
                let d = b - a;
                (p - (a + d * ((p - a).dot(d) / d.length_squared().max(1e-300)).clamp(0.0, 1.0))).length()
            }
            C::Circle(o, r, _) => (p.distance(o) - r).abs(),
        };
        let (at, other) = match &q.vertex {
            Some(([a, b], flag)) => {
                let (ca, cb) = (curve(a)?, curve(b)?);
                let [v] = meet(ca, cb)[..] else { return None };
                if !on(edge, v) {
                    return None;
                }
                let other = if *a == q.edge { cb } else { ca };
                (along(v, if *flag < 0.0 { step } else { -step }), Some(other))
            }
            None => (
                match edge {
                    C::Line(a, b) => (a + b) * 0.5,
                    C::Circle(o, r, arc) => o + DVec2::from_angle(arc.map_or(0.0, |(a0, sweep)| a0 + 0.5 * sweep)) * r,
                },
                None,
            ),
        };
        let off = other.map_or(0.05 * step, |c| (0.5 * distance(c, at)).min(0.05 * step));
        Some(at + tangent(edge, at).perp() * (off * q.side.signum()))
    }

    /// A sketch region query (Onshape's packed form: a FACE whose
    /// TOPOLOGY is the sketch entities bounding it): our region whose
    /// interior point lies inside those curves. `None`: not a region query.
    fn region_from_query(&mut self, text: &str) -> Result<Option<String>, String> {
        let mut own_edges: Vec<String> = Vec::new();
        let (sk, strings) = match text.split("qCompressed(1.0,\"").nth(1).and_then(|r| r.split('"').next()) {
            Some(packed) => {
                let Ok(tree) = noshape_model::custom::host::packed::unpack(packed) else { return Ok(None) };
                let mut strings = Vec::new();
                packed_strings(&tree, &mut strings);
                own_edges = packed_face_edges(&tree);
                // The sketch whose imprint the face is (the query's own
                // operation), else the first sketch named.
                let sk = root_operation(&tree).and_then(|o| o.strip_suffix(".imprint").map(str::to_string)).or_else(|| strings.iter().find_map(|s| s.strip_suffix(".wireOp")).map(str::to_string));
                (sk, strings)
            }
            None => imprint_query_words(text),
        };
        // A face the sketch imprints, told by the curves around it (a face
        // swept from sketch curves names them too, without the imprint).
        if !["SKETCH_ENTITY", "FACE", "TOPOLOGY", "IMPRINT"].iter().all(|w| strings.iter().any(|s| s == w)) {
            return Ok(None);
        }
        let Some(sk) = sk else { return Ok(None) };
        let mut segments: Vec<[f64; 2]> = strings.iter().filter_map(|e| self.curve_of(&sk, e)).flat_map(|poly| poly.windows(2).flat_map(|w| [w[0], w[1]]).collect::<Vec<_>>()).collect();
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
        if segments.is_empty() {
            return Err(format!("region of sketch {sk}: its curves were not translated"));
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
        // The region bounded by exactly the face's own edges (the curves its vertices meet are named too, one level
        // down: c2-sm-exhaust-clamp's small circles touching a line and a ring, their disks, not the slivers beside).
        let own: std::collections::BTreeSet<String> = own_edges.iter().filter_map(|e| self.sketch_entities.get(&(sk.clone(), e.clone())).cloned()).collect();
        if !own.is_empty() && own.len() == own_edges.iter().collect::<std::collections::BTreeSet<_>>().len() && !derived {
            let exact: Vec<&Value> = all.iter().filter(|r| r["boundary"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect::<std::collections::BTreeSet<_>>() == own).collect();
            if let [only] = exact.as_slice() {
                return Ok(only["id"].as_str().map(whole));
            }
        }
        let mut bounded: Vec<Value> = all.iter().filter(|r| !named.is_empty() && named.iter().all(|e| r["boundary"].as_array().is_some_and(|b| b.iter().any(|x| x == e)))).cloned().collect();
        if let [only] = bounded.as_slice() {
            return Ok(only["id"].as_str().map(whole));
        }
        // None has them all: Onshape names a curve that only meets the
        // region's edge too (a tangent line's vertex on it; learning-center-ivan's
        // lobes). The region whose own curves are all named and meet every
        // other named one, the most of them.
        if bounded.is_empty() && !derived && !named.is_empty() {
            let own = |r: &Value| -> Vec<String> { r["boundary"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect() };
            let entity = |id: &str| g["entities"].as_array().into_iter().flatten().find(|e| e["id"] == id).cloned().unwrap_or(Value::Null);
            let tol = 1e-6 / per_unit(&g["units"]);
            let meets_all = |b: &[String]| named.iter().filter(|n| !b.contains(n)).all(|n| b.iter().any(|e| curves_meet(&entity(n), &entity(e), tol)));
            let within: Vec<(usize, &Value)> = all.iter().map(|r| (own(r), r)).filter(|(b, _)| !b.is_empty() && b.iter().all(|e| named.contains(e)) && meets_all(b)).map(|(b, r)| (b.iter().collect::<std::collections::BTreeSet<_>>().len(), r)).collect();
            let most = within.iter().map(|(n, _)| *n).max().unwrap_or(0);
            if let [(_, only)] = within.iter().filter(|(n, _)| *n == most).collect::<Vec<_>>().as_slice() {
                return Ok(only["id"].as_str().map(whole));
            }
        }
        // None has them all, and they don't close round one (a face's region named by curves of which ours bound it
        // with only some, others lying along its neighbours' edges: c2-sm-louver's R8BC): the one region bounded by
        // the most of them.
        if bounded.is_empty() && !named.is_empty() {
            let count = |r: &Value| named.iter().filter(|e| r["boundary"].as_array().is_some_and(|b| b.iter().any(|x| x == e.as_str()))).count();
            let most = all.iter().map(count).max().unwrap_or(0);
            if let [only] = all.iter().filter(|r| most > 0 && count(r) == most).collect::<Vec<_>>().as_slice() {
                return Ok(only["id"].as_str().map(whole));
            }
        }
        // A text query's face by its edge and side (a piece of a curve from where two curves meet, `INTERSECT`; or the
        // whole curve): the region at a point a hair along that piece from the vertex (-1: the piece starting there, +1:
        // ending there) and a hair to the face's side (+1 its left, as a loop runs; a circle's inside).
        if bounded.len() != 1 && !derived
            && let Some(q) = edge_side_query(text)
            && let Some(probe) = self.edge_side_probe(&sk, &g, &q)
        {
            let k = per_unit(&g["units"]);
            let at: [f64; 3] = std::array::from_fn(|i| (o[i] + probe.x * x[i] + probe.y * y[i]) * k);
            let found = self.call("sketch_get", json!({ "sketch": ours, "at": mm(at) }))?;
            // (A sketch on a face: its pieces of the face where no region of its own curves holds the point, cf-lighten's
            // pockets.)
            let at_point: Vec<&Value> = found["regions"].as_array().into_iter().flatten().collect();
            let own: Vec<&str> = at_point.iter().filter(|r| r["imprinted"] != true).filter_map(|r| r["id"].as_str()).collect();
            if let [only] = own.as_slice() {
                return Ok(Some(whole(only)));
            }
            if let ([], [piece]) = (own.as_slice(), at_point.as_slice())
                && let Some(id) = piece["id"].as_str()
            {
                return Ok(Some(id.to_string()));
            }
        }
        // The face's edge on a circle or arc and the face's side of it (`…}),-1.0]])]`, the query's last orientation):
        // -1 the regions outside that circle, +1 those inside (cf-lighten's rings around its holes, the link beside them).
        if bounded.len() > 1 && !derived
            && let Some(curve) = face_edge_entity(text).and_then(|e| self.sketch_entities.get(&(sk.clone(), e)).cloned())
            && let Some(e) = g["entities"].as_array().into_iter().flatten().find(|e| e["id"] == curve.as_str() && (e["type"] == "circle" || e["type"] == "arc"))
            && let Some(side) = text.rfind("]])]").and_then(|end| text[..end].rsplit_once(',')).and_then(|(_, n)| n.trim().parse::<f64>().ok())
        {
            let p = |v: &Value| Some(glam::DVec2::new(v[0].as_f64()?, v[1].as_f64()?));
            let center = p(&e["center"]);
            let radius = e["radius"].as_f64().or_else(|| Some(p(&e["start"])?.distance(center?)));
            if let (Some(c), Some(r)) = (center, radius) {
                let kept: Vec<&Value> = bounded.iter().filter(|g| p(&g["inside"]).is_some_and(|q| (q.distance(c) < r) == (side > 0.0))).collect();
                if let [only] = kept.as_slice() {
                    return Ok(only["id"].as_str().map(whole));
                }
                if !kept.is_empty() && kept.len() < bounded.len() {
                    bounded = kept.into_iter().cloned().collect();
                }
            }
        }
        // One line named and no region bounded by it: it lies along another curve that bounds the region instead
        // (two rectangles' sides on one line, cf-box-dovetail-joint's corners). The region beside it, where only one
        // side has one.
        if bounded.is_empty() && !derived && let [line] = named.as_slice() {
            let entity = g["entities"].as_array().into_iter().flatten().find(|e| e["id"] == line.as_str() && e["type"] == "line");
            let end = |e: &Value, k: &str| Some(glam::DVec2::new(e[k][0].as_f64()?, e[k][1].as_f64()?));
            if let Some((a, b)) = entity.and_then(|e| Some((end(e, "start")?, end(e, "end")?))) {
                let (mid, side) = ((a + b) * 0.5, (b - a).perp().normalize_or_zero() * (1e-3 * a.distance(b)).min(0.01));
                let k = per_unit(&g["units"]);
                let mut beside: Vec<String> = Vec::new();
                for q in [mid + side, mid - side] {
                    let at: [f64; 3] = std::array::from_fn(|i| (o[i] + q.x * x[i] + q.y * y[i]) * k);
                    let found = self.call("sketch_get", json!({ "sketch": ours, "at": mm(at) }))?;
                    beside.extend(found["regions"].as_array().into_iter().flatten().filter(|r| r["imprinted"] != true).filter_map(|r| r["id"].as_str().map(str::to_string)));
                }
                beside.dedup();
                if let [only] = beside.as_slice() {
                    return Ok(Some(whole(only)));
                }
            }
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
        // Several share them: the one bounded by exactly those (a circle cut
        // by two chords: its side piece, not the middle; stef-radijator).
        if bounded.len() > 1 && !derived {
            let want: std::collections::BTreeSet<&str> = named.iter().map(String::as_str).collect();
            let exact: Vec<&Value> = bounded.iter().filter(|r| r["boundary"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<std::collections::BTreeSet<_>>() == want).collect();
            if let [only] = exact.as_slice() {
                return Ok(only["id"].as_str().map(whole));
            }
        }
        // Several share them: the named curve's whole shape decides (a
        // rectangle's `.left` side alone is on both its inside and the region
        // beside it; live: bracket120's Extrude 2 takes the rectangle).
        if bounded.len() > 1 {
            // (A shape's curves share the id before the first dot: a
            // rectangle's `.left`, a polygon's `.seed` and `.pattern.0.1.0`
            // sides; clamp-guillem's hexagons.)
            let shape = |e: &str| e.split_once('.').map(|(p, _)| p.to_string());
            let shapes: std::collections::BTreeSet<String> = strings.iter().filter(|e| self.curves.contains_key(&(sk.clone(), (*e).clone()))).filter_map(|e| shape(e)).collect();
            // Each side not named yet, once; not its construction curves (a
            // polygon's circle).
            let construction = |id: &String| self.construction.contains(&(sk.clone(), id.clone()));
            let siblings = self.curves.iter().filter(|((s, id), _)| *s == sk && !strings.contains(id) && !construction(id) && shape(id).is_some_and(|p| shapes.contains(&p)));
            segments.extend(siblings.flat_map(|(_, poly)| poly.windows(2).flat_map(|w| [w[0], w[1]])));
        }
        // Else (several share them, or none known): the one inside those curves.
        let several = bounded.len() > 1;
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
        // (None inside them, several bounded by them: queries naming the same curves from either side of their
        // meeting take one each (cf-lighten's ring, split in two by a tangent line and an arc).)
        if best.is_none() && several && !derived
            && let Some(id) = regions.iter().filter_map(|r| r["id"].as_str()).find(|id| !self.picked.contains(*id)).map(str::to_string)
        {
            self.picked.insert(id.clone());
            return Ok(Some(whole(&id)));
        }
        best.map(|b| Some(whole(&b.1))).ok_or_else(|| format!("no region of ours inside sketch {sk}'s curves {:?}", strings.iter().filter(|e| self.curves.contains_key(&(sk.clone(), (*e).clone()))).collect::<Vec<_>>()))
    }

    /// A vertex a Split imprinted onto a face where two curves of a sketch meet (packed `opSplitFace` `IMPRINT_ON_FACE`,
    /// EntityType.VERTEX, derived from the sketch's curves; c2-hunting-knife's Bridging curve sides): ours nearest the
    /// line through that meeting point along the sketch's normal. `None`: not such a query.
    fn imprinted_vertex_from_query(&mut self, text: &str) -> Option<Result<String, String>> {
        if !text.contains("qCompressed(") {
            return None;
        }
        let strings = packed_strings_of(text);
        if !strings.iter().any(|s| s == "IMPRINT_ON_FACE") || strings.last().map(String::as_str) != Some("IMPRINT_ON_FACE") || !strings.iter().any(|s| s == "VERTEX") {
            return None;
        }
        // Each sketch's curves the query names (each named with its sketch), the sketch named last first: the vertex's
        // own come after the face's.
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        let mut current: Option<String> = None;
        for w in strings.windows(2) {
            if let Some(sk) = w[1].strip_suffix(".wireOp").filter(|_| w[0] == "operationId") {
                current = Some(sk.to_string());
            } else if w[0] == "sketchEntityId"
                && let Some(sk) = &current
            {
                let ids = match groups.iter().position(|(g, _)| g == sk) {
                    Some(i) => {
                        let g = groups.remove(i);
                        groups.push(g);
                        &mut groups.last_mut().expect("just pushed").1
                    }
                    None => {
                        groups.push((sk.clone(), Vec::new()));
                        &mut groups.last_mut().expect("just pushed").1
                    }
                };
                ids.push(w[1].clone());
            }
        }
        let ends = |c: &Vec<[f64; 2]>| [c[0], c[c.len() - 1]];
        let meeting = groups.iter().rev().find_map(|(sk, ids)| {
            let curves: Vec<Vec<[f64; 2]>> = ids.iter().filter_map(|e| self.curve_of(sk, e)).filter(|c| c.len() >= 2).collect();
            curves.iter().enumerate().find_map(|(i, a)| {
                curves[i + 1..].iter().find_map(|b| ends(a).into_iter().find(|p| ends(b).iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-7)))
            }).map(|p| (sk.clone(), p))
        });
        let Some((sk, corner)) = meeting else { return Some(Err("IMPRINT_ON_FACE vertex: no two of its sketch curves meet".into())) };
        let Some([o, x, y]) = self.frames.get(&sk).copied() else { return Some(Err(format!("IMPRINT_ON_FACE vertex: sketch {sk}'s frame"))) };
        let at: [f64; 3] = std::array::from_fn(|i| o[i] + x[i] * corner[0] + y[i] * corner[1]);
        let n = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
        let found = match self.call("query_entities", json!({ "kind": "vertices" })) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };
        let unit = per_unit(&found["units"]);
        let best = found["vertices"].as_array().into_iter().flatten().filter_map(|v| {
            let p = f3(&v["position"])?;
            let w: [f64; 3] = std::array::from_fn(|i| p[i] * unit - at[i]);
            let t = w[0] * n[0] + w[1] * n[1] + w[2] * n[2];
            let off: f64 = (0..3).map(|i| (w[i] - t * n[i]).powi(2)).sum::<f64>().sqrt();
            Some((off, t.abs(), v["id"].as_str()?.to_string()))
        }).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        Some(match best {
            Some((off, _, id)) if off < 1e-6 => Ok(id),
            Some((off, _, _)) => Err(format!("IMPRINT_ON_FACE vertex: none of ours on sketch {sk}'s point {:?} mm along its normal (nearest {:.4} mm off)", mm(at), off * 1e3)),
            None => Err("IMPRINT_ON_FACE vertex: no vertices of ours".into()),
        })
    }

    /// A fillet's vertex by Onshape's written name (`makeQuery(id+"F….opFillet","BLEND_VERTEX",EntityType.VERTEX,
    /// {"blendedFrom":[an Extrude's swept edge, …],"blendedInto":[two of its faces]})`, cf-snap-hook's Plane 1): ours
    /// where those two faces meet, nearest the line the swept edge ran along. `None`: not such a query.
    fn blend_vertex_from_query(&mut self, text: &str) -> Option<Result<String, String>> {
        if !text.contains("\"BLEND_VERTEX\",EntityType.VERTEX") {
            return None;
        }
        let query = inline_query_vars(text)?;
        let list = |key: &str| top_level_items(query.split(&format!("\"{key}\":[")).nth(1)?);
        let (Some(into), Some(from)) = (list("blendedInto"), list("blendedFrom")) else { return Some(Err("BLEND_VERTEX: no blendedFrom / blendedInto".into())) };
        let mut faces = Vec::new();
        for f in &into {
            match self.written_extrude_name(f) {
                Some(Ok(id)) => faces.push(id),
                Some(Err(e)) => return Some(Err(format!("BLEND_VERTEX: {e}"))),
                None => return Some(Err("BLEND_VERTEX: a face it lies on is not an Extrude's".into())),
            }
        }
        let Some(edge) = from.iter().find(|e| e.contains("\"SWEPT_EDGE\"")) else { return Some(Err("BLEND_VERTEX: not blended from a swept edge".into())) };
        let mut sk = None;
        let mut curves = Vec::new();
        for q in edge.split("sketchEntityQuery(id+\"").skip(1) {
            let Some((s, after)) = q.split_once(".wireOp\"") else { continue };
            curves.extend(after.split('"').nth(1).and_then(|e| self.curve_of(s, e)));
            sk = Some(s.to_string());
        }
        let (Some(sk), [a, b]) = (sk, curves.as_slice()) else { return Some(Err(format!("BLEND_VERTEX: {} of its swept edge's 2 curves translated", curves.len()))) };
        let ends = |c: &Vec<[f64; 2]>| [c[0], c[c.len() - 1]];
        let Some(corner) = ends(a).into_iter().find(|p| ends(b).iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-7)) else { return Some(Err("BLEND_VERTEX: its swept edge's curves don't meet".into())) };
        let Some([o, x, y]) = self.frames.get(&sk).copied() else { return Some(Err(format!("BLEND_VERTEX: sketch {sk}'s frame"))) };
        let at: [f64; 3] = std::array::from_fn(|i| o[i] + x[i] * corner[0] + y[i] * corner[1]);
        let n = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
        let found = match self.call("query_entities", json!({ "kind": "vertices", "of": faces })) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };
        let unit = per_unit(&found["units"]);
        let best = found["vertices"].as_array().into_iter().flatten().filter_map(|v| {
            let p = f3(&v["position"])?;
            let w: [f64; 3] = std::array::from_fn(|i| p[i] * unit - at[i]);
            let t = w[0] * n[0] + w[1] * n[1] + w[2] * n[2];
            let off: f64 = (0..3).map(|i| (w[i] - t * n[i]).powi(2)).sum::<f64>().sqrt();
            Some((off, v["id"].as_str()?.to_string()))
        }).min_by(|a, b| a.0.total_cmp(&b.0));
        Some(best.map(|(_, id)| id).ok_or_else(|| "BLEND_VERTEX: its two faces share no vertex of ours".to_string()))
    }

    /// The fillet face a packed query names `blendedFrom` an Extrude's swept edge at the corner of two sketch curves
    /// (a Hole's in-feature mate connector placed at the CENTER of a rounded corner, clamp-hms): ours, the cylinder
    /// whose axis runs along the extrude next to that corner. `None`: not such a query.
    fn blended_face_from_query(&mut self, text: &str) -> Option<Result<String, String>> {
        use noshape_model::custom::host::packed::Packed;
        let packed = text.split("qCompressed(1.0,\"").nth(1)?.split('"').next()?;
        let tree = noshape_model::custom::host::packed::unpack(packed).ok()?;
        let Packed::Tagged(_, top) = &tree else { return None };
        let Packed::Map(tm) = top.as_ref() else { return None };
        let (_, from) = tm.iter().find(|(k, _)| matches!(k, Packed::Str(k) if k == "blendedFrom"))?;
        let mut strings = Vec::new();
        packed_strings(from, &mut strings);
        if !strings.iter().any(|s| s == "SWEPT_EDGE") || !strings.iter().any(|s| s.ends_with(".opExtrude")) {
            return None;
        }
        let sk = strings.iter().find_map(|s| s.strip_suffix(".wireOp"))?.to_string();
        let ids: Vec<String> = strings.windows(2).filter(|w| w[0] == "sketchEntityId").map(|w| w[1].clone()).collect();
        let curves: Vec<Vec<[f64; 2]>> = ids.iter().filter_map(|e| self.curve_of(&sk, e)).collect();
        let [a, b] = curves.as_slice() else { return Some(Err(format!("blended edge of sketch {sk}: {} of its 2 curves translated", curves.len()))) };
        let ends = |c: &Vec<[f64; 2]>| [c[0], c[c.len() - 1]];
        let near = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-7;
        let Some(corner) = ends(a).into_iter().find(|p| ends(b).into_iter().any(|q| near(*p, q))) else { return Some(Err(format!("blended edge of sketch {sk}: its curves don't meet"))) };
        let Some([o, x, y]) = self.frames.get(&sk).copied() else { return Some(Err(format!("blended edge: sketch {sk}'s frame"))) };
        let at: [f64; 3] = std::array::from_fn(|i| o[i] + x[i] * corner[0] + y[i] * corner[1]);
        let n = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
        let found = match self.call("query_entities", json!({ "kind": "faces", "surface": "cylinder", "near": mm(at), "limit": 12 })) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };
        // The cylinder along the extrude whose axis is nearest the corner line (a fillet's: r√2 from a right corner).
        let unit = per_unit(&found["units"]);
        let best = found["faces"].as_array().into_iter().flatten().filter_map(|f| {
            let (Some(p), Some(d)) = (f3(&f["axis"]["origin"]), f3(&f["axis"]["direction"])) else { return None };
            let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
            if along.abs() < 1.0 - 1e-6 {
                return None;
            }
            // Distance between the two parallel lines (the axis, in the document's units, and the corner line, in m).
            let w: [f64; 3] = std::array::from_fn(|i| p[i] * unit - at[i]);
            let t = w[0] * n[0] + w[1] * n[1] + w[2] * n[2];
            let off: f64 = (0..3).map(|i| (w[i] - t * n[i]).powi(2)).sum::<f64>().sqrt();
            Some((off, f["id"].as_str()?.to_string()))
        }).min_by(|a, b| a.0.total_cmp(&b.0));
        Some(best.map(|(_, id)| id).ok_or_else(|| format!("blended edge of sketch {sk}: no rounded corner of ours there")))
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
    /// (packed `CAP_FACE` / `SWEPT_FACE` / `CAP_EDGE` / `SWEPT_EDGE` /
    /// `CAP_VERTEX` of `….opExtrude` from sketch curves, `SWEPT_BODY`): ours of the same
    /// Extrude through the same sketch geometry. Used when live's geometry
    /// for the id is missing.
    fn extrude_name_from_query(&mut self, text: &str) -> Option<Result<String, String>> {
        if let Some(r) = self.written_extrude_name(text) {
            return Some(r);
        }
        let packed = text.split("qCompressed(1.0,\"").nth(1)?.split('"').next()?;
        let tree = noshape_model::custom::host::packed::unpack(packed).ok()?;
        self.extrude_name_from_tree(&tree)
    }

    fn extrude_name_from_tree(&mut self, tree: &noshape_model::custom::host::packed::Packed) -> Option<Result<String, String>> {
        use noshape_model::custom::host::packed::Packed;
        // An edge a Boolean made where two named faces meet (`INTERSECT`
        // of their names, exercise-evana's Fillet 1): their common edge.
        let top = match tree {
            Packed::Tagged(_, t) => t.as_ref(),
            t => t,
        };
        if let Packed::Map(tm) = top {
            let get = |key: &str| tm.iter().find(|(k, _)| matches!(k, Packed::Str(k) if k == key)).map(|(_, v)| v);
            if matches!(get("queryType"), Some(Packed::Str(t)) if t == "INTERSECT")
                && let Some(Packed::Array(sides)) = get("derivedFrom")
                && sides.len() == 2
            {
                let mut faces = Vec::new();
                for side in sides {
                    match self.extrude_name_from_tree(side)? {
                        Ok(f) => faces.push(f),
                        Err(e) => return Some(Err(format!("INTERSECT: {e}"))),
                    }
                }
                return Some(self.common_edge(&faces[0], &faces[1]));
            }
        }
        fn find<'p>(p: &'p Packed, names: &[&str]) -> Option<&'p Vec<(Packed, Packed)>> {
            match p {
                Packed::Map(m) if m.iter().any(|(k, v)| matches!(k, Packed::Str(k) if k == "queryType") && matches!(v, Packed::Str(v) if names.contains(&v.as_str()))) => Some(m),
                Packed::Map(m) => m.iter().find_map(|(_, v)| find(v, names)),
                Packed::Array(a) => a.iter().find_map(|v| find(v, names)),
                Packed::Tagged(_, v) => find(v, names),
                _ => None,
            }
        }
        let names = ["CAP_FACE", "SWEPT_FACE", "CAP_EDGE", "SWEPT_EDGE", "CAP_VERTEX", "SWEPT_BODY"];
        // Nested in another operation's name: only a Boolean keeps the entity
        // itself (a pattern's or mirror's COPY is another entity).
        if let Packed::Tagged(_, top) = tree
            && let Packed::Map(tm) = top.as_ref()
            && !tm.iter().any(|(k, v)| matches!(k, Packed::Str(k) if k == "queryType") && matches!(v, Packed::Str(v) if names.contains(&v.as_str())))
        {
            let mut op = Vec::new();
            if let Some((_, v)) = tm.iter().find(|(k, _)| matches!(k, Packed::Str(k) if k == "operationId")) {
                packed_strings(v, &mut op);
            }
            // (A sketch's imprint keeps it too, split: cf-transform-pattern's connector on a cap edge a sketch imprints.)
            if !op.iter().any(|o| o.ends_with(".opBoolean") || o.ends_with(".imprint")) {
                return None;
            }
        }
        let m = find(tree, &names)?;
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
        let mut curves: Vec<Vec<[f64; 2]>> = Vec::new();
        for c in strings.iter().filter_map(|e| self.curve_of(&sk, e)) {
            // (A query names a curve again in its derivedFrom.)
            if !curves.contains(&c) {
                curves.push(c);
            }
        }
        // Which of the curves' crossings an edge swept from one is (ORDER).
        let crossing = packed_order(&Packed::Map(m.clone()));
        Some(if revolve { self.revolve_entity(name, &op, &sk, &curves) } else { self.extrude_entity(name, is_start, crossing, &op, &sk, &curves) })
    }

    /// An end of a curve a custom feature fit (`VERTEX` created by `<feature>.….opFitSpline`, ORDER 0 its start, 1 its
    /// end: cf-extrude-vertex's Symmetric on the end of Extruded vertex along line): our vertex there.
    fn fit_spline_vertex_from_query(&mut self, text: &str) -> Option<Result<String, String>> {
        // Written (`dummyQuery(id+"F….opFitSpline",EntityType.VERTEX,0.0)`: cf-3d-spline's Plane 1) or packed.
        let written = text.split("dummyQuery(id+\"").nth(1).and_then(|r| {
            let (op, rest) = r.split_once("\",EntityType.VERTEX,")?;
            let n: f64 = rest.split(')').next()?.trim().parse().ok()?;
            Some((op.strip_suffix(".opFitSpline")?.split('.').next()?.to_string(), n as usize))
        });
        let (feature, order) = match written {
            Some(w) => w,
            None => {
                let strings = packed_strings_of(text);
                if !strings.iter().any(|s| s == "VERTEX") {
                    return None;
                }
                let feature = strings.iter().find(|s| s.ends_with(".opFitSpline")).and_then(|s| s.split('.').next())?.to_string();
                let packed = text.split("qCompressed(1.0,\"").nth(1).and_then(|t| t.split('"').next())?;
                (feature, packed_order(&noshape_model::custom::host::packed::unpack(packed).ok()?)?)
            }
        };
        let Some(ours) = self.feature_ids.get(&feature).cloned() else { return Some(Err(format!("opFitSpline vertex: feature {feature} was not built"))) };
        let found = (|| -> Result<String, String> {
            let edges = self.call("query_entities", json!({ "kind": "edges", "created_by": ours, "limit": 2 }))?;
            let [edge] = edges["edges"].as_array().map(Vec::as_slice).unwrap_or_default() else { return Err(format!("opFitSpline vertex: {feature} made no single curve of ours")) };
            let at = f3(&edge[if order == 0 { "start" } else { "end" }]).ok_or("opFitSpline vertex: the curve has no ends")?;
            let vertices = self.call("query_entities", json!({ "kind": "vertices", "limit": 5000 }))?;
            vertices["vertices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| Some((v["id"].as_str()?, f3(&v["position"])?)))
                .find(|(_, p)| (0..3).all(|i| (p[i] - at[i]).abs() < 1e-6))
                .map(|(id, _)| id.to_string())
                .ok_or_else(|| format!("opFitSpline vertex: no vertex of ours at the end {order} of {feature}'s curve"))
        })();
        Some(found)
    }

    /// A Hole's face by Onshape's name (`HOLE_FACE` of `….opHole`): its
    /// `tip`, the face of ours the Hole made farthest down its bore.
    fn hole_face_from_query(&mut self, text: &str) -> Option<Result<String, String>> {
        let strings = packed_strings_of(text);
        if !strings.iter().any(|s| s == "HOLE_FACE") {
            return None;
        }
        let op = strings.iter().find(|s| s.ends_with(".opHole")).and_then(|s| s.split(".op").next())?;
        let name = strings.iter().position(|s| s == "faceName").and_then(|i| strings.get(i + 1))?.clone();
        let Some(ours) = self.feature_ids.get(op).cloned() else { return Some(Err(format!("HOLE_FACE: hole {op} was not built"))) };
        if name != "tip" {
            return Some(Err(format!("HOLE_FACE {name} not translated")));
        }
        let faces = match self.call("query_entities", json!({ "kind": "faces", "created_by": ours, "limit": 200 })) {
            Ok(f) => f["faces"].as_array().cloned().unwrap_or_default(),
            Err(e) => return Some(Err(e)),
        };
        // The bore: the narrowest cylinder; the tip: the other face farthest along its axis.
        let bore = faces.iter().filter(|f| f["surface"] == "cylinder").min_by(|a, b| a["radius"].as_f64().unwrap_or(f64::MAX).total_cmp(&b["radius"].as_f64().unwrap_or(f64::MAX)))?;
        let (c, d) = (f3(&bore["centroid"])?, f3(&bore["axis"]["direction"])?);
        let along = |f: &Value| f3(&f["centroid"]).map_or(0.0, |p| (0..3).map(|i| (p[i] - c[i]) * d[i]).sum::<f64>().abs());
        let tip = faces.iter().filter(|f| f["surface"] != "cylinder").max_by(|a, b| along(a).total_cmp(&along(b)))?;
        tip["id"].as_str().map(|id| Ok(id.to_string()))
    }

    /// The written-out form (`makeQuery(id + "F….opExtrude", "CAP_FACE",
    /// EntityType.FACE, {… sketchEntityQuery(id + "<sketch>.wireOp", …,
    /// "<entity>") …, "isStart" : false})`; learning-center-devon).
    fn written_extrude_name(&mut self, text: &str) -> Option<Result<String, String>> {
        let rest = text.trim_start_matches("query=").trim_start().strip_prefix("makeQuery(id+\"")?;
        let (op, rest) = rest.split_once('"')?;
        let name = rest.split('"').nth(1)?;
        if !["CAP_FACE", "SWEPT_FACE", "CAP_EDGE", "SWEPT_EDGE", "CAP_VERTEX"].contains(&name) {
            return None;
        }
        let (op, revolve) = match (op.strip_suffix(".opExtrude"), op.strip_suffix(".opRevolve")) {
            (Some(o), _) => (o, false),
            (_, Some(o)) => (o, true),
            _ => return None,
        };
        let mut sk = None;
        let mut curves = Vec::new();
        for q in text.split("sketchEntityQuery(id+\"").skip(1) {
            let (s, after) = q.split_once(".wireOp\"")?;
            let e = after.split('"').nth(1)?;
            sk = Some(s.to_string());
            curves.extend(self.curve_of(s, e));
        }
        let sk = sk?;
        let is_start = text.contains("\"isStart\":true");
        Some(if revolve { self.revolve_entity(name, op, &sk, &curves) } else { self.extrude_entity(name, is_start, None, op, &sk, &curves) })
    }

    /// The edge between faces `a` and `b`; else (a cut split one of them,
    /// the name picked the other piece) an edge of one whose other face lies
    /// in the other's plane.
    fn common_edge(&mut self, a: &str, b: &str) -> Result<String, String> {
        let r = self.call("query_entities", json!({ "kind": "edges", "of": [a, b], "limit": 1 }))?;
        if let Some(e) = r["edges"][0]["id"].as_str() {
            return Ok(e.to_string());
        }
        let all = self.call("query_entities", json!({ "kind": "faces", "limit": 100000 }))?;
        let face = |id: &str| all["faces"].as_array().into_iter().flatten().find(|f| f["id"] == id).cloned().unwrap_or(Value::Null);
        let tol = 1e-6 / per_unit(&all["units"]);
        let on = |f: &Value, g: &Value| -> bool {
            let (Some(n), Some(m), Some(p), Some(q)) = (f3(&f["normal"]), f3(&g["normal"]), f3(&f["centroid"]), f3(&g["centroid"])) else { return false };
            let along: f64 = (0..3).map(|i| (p[i] - q[i]) * m[i]).sum();
            (0..3).all(|i| (n[i] - m[i]).abs() < 1e-9) && along.abs() < tol
        };
        for (x, y) in [(a, b), (b, a)] {
            let target = face(x);
            let edges = self.call("query_entities", json!({ "kind": "edges", "of": [y] }))?;
            for e in edges["edges"].as_array().into_iter().flatten() {
                let Some(id) = e["id"].as_str() else { continue };
                let sides = self.call("query_entities", json!({ "kind": "faces", "of": [id] }))?;
                if sides["faces"].as_array().into_iter().flatten().any(|f| f["id"] != y && on(f, &target)) {
                    return Ok(id.to_string());
                }
            }
        }
        Err("INTERSECT: its two faces have no common edge of ours".into())
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
                let shared = curves.get(1).and_then(|b| [first[0], first[first.len() - 1]].into_iter().find(|p| [b[0], b[b.len() - 1]].iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-7)));
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

    /// Sketch `sk`'s curve `id` as drawn; a pattern instance id the sketch doesn't have
    /// (`<seed>.<group>.<i>.<k>`, stef-radijator's `TNHaprdwY81W.1.18.0` of an 18-instance circular pattern, named
    /// by Fillet 1 though the pattern lists instances 0-17) is instance i mod n of the pattern listing its siblings
    /// (the seed for i = n), as live resolves it (its Fillet rounds the edge at the seed's crossing).
    fn curve_of(&self, sk: &str, id: &str) -> Option<Vec<[f64; 2]>> {
        if let Some(c) = self.curves.get(&(sk.to_string(), id.to_string())) {
            return Some(c.clone());
        }
        let sketch = self.features.iter().find(|f| f["featureId"] == sk)?;
        self.curves.get(&(sk.to_string(), pattern_alias(sketch, id)?)).cloned()
    }

    fn extrude_entity(&mut self, name: &str, is_start: bool, crossing: Option<usize>, op: &str, sk: &str, curves: &[Vec<[f64; 2]>]) -> Result<String, String> {
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
        // A cap's vertex: over the corner the two curves share, on the part the extrude made, the nearest
        // (start) or farthest (end) along the extrude (cf-transform-pattern's Transform pattern 2).
        if name == "CAP_VERTEX" {
            let (a, b) = (curves.first().ok_or("CAP_VERTEX: no curves")?, curves.get(1).ok_or("CAP_VERTEX: one curve")?);
            // The end they share, else one's end on the other (c2-duct-multipart's Plane 4: a diagonal ending on a
            // line's middle), else where they cross.
            let on = |p: [f64; 2], c: &[[f64; 2]]| {
                c.windows(2).any(|w| {
                    let (d, e) = ([w[1][0] - w[0][0], w[1][1] - w[0][1]], [p[0] - w[0][0], p[1] - w[0][1]]);
                    let t = ((e[0] * d[0] + e[1] * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-30)).clamp(0.0, 1.0);
                    (e[0] - t * d[0]).hypot(e[1] - t * d[1]) < 1e-7
                })
            };
            let ends = |c: &Vec<[f64; 2]>| [c[0], c[c.len() - 1]];
            let corner = ends(a)
                .into_iter()
                .find(|p| ends(b).iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-7))
                .or_else(|| ends(a).into_iter().find(|p| on(*p, b)))
                .or_else(|| ends(b).into_iter().find(|p| on(*p, a)))
                .or_else(|| crossings(a, b).first().copied())
                .ok_or("CAP_VERTEX: its curves share no vertex")?;
            let at = world(corner);
            let made = self.call("query_entities", json!({ "kind": "faces", "created_by": ours, "limit": 200 }))?;
            let parts: Vec<String> = made["faces"].as_array().into_iter().flatten().filter_map(|f| f["part"].as_str().map(str::to_string)).collect();
            let found = self.call("query_entities", json!({ "kind": "vertices", "limit": 100000 }))?;
            let k = per_unit(&found["units"]);
            let on: Vec<(String, f64)> = found["vertices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| v["part"].as_str().is_some_and(|p| parts.iter().any(|x| x == p)))
                .filter_map(|v| Some((v["id"].as_str()?.to_string(), f3(&v["position"])?.map(|c| c * k))))
                .filter(|(_, q)| lateral(*q, at) < 1e-6)
                .map(|(id, q)| (id, dot(sub(q, o), n).abs()))
                .collect();
            let pick = if is_start { on.iter().min_by(|x, y| x.1.total_cmp(&y.1)) } else { on.iter().max_by(|x, y| x.1.total_cmp(&y.1)) };
            return pick.map(|x| x.0.clone()).ok_or_else(|| format!("{name} of {op}: no vertex of ours over {:?} mm", mm(at)));
        }
        let (kind, key, at) = match name {
            // The edge swept from the vertex the two curves share, else from
            // the `crossing`th point where they cross (an imprinted vertex).
            "SWEPT_EDGE" => {
                let shared = curves.first().and_then(|a| curves.get(1).and_then(|b| [a[0], a[a.len() - 1]].into_iter().find(|p| [b[0], b[b.len() - 1]].iter().any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-7))));
                let crossed = || curves.first().zip(curves.get(1)).and_then(|(a, b)| crossings(a, b).get(crossing.unwrap_or(0)).copied());
                ("edges", "midpoint", world(shared.or_else(crossed).ok_or("SWEPT_EDGE: its curves share no vertex")?))
            }
            "SWEPT_FACE" => ("faces", "centroid", world(mid(&curves[0]))),
            "CAP_EDGE" => ("edges", "midpoint", world(mid(&curves[0]))),
            _ => ("faces", "centroid", world(mid(&curves[0]))),
        };
        let found = self.call("query_entities", json!({ "kind": kind, "created_by": ours, "limit": 200 }))?;
        let k = per_unit(&found["units"]);
        // A cap of a planar profile faces along the extrude (a side face a later feature grew up past the cap is not
        // one: clamp-hms's y = 33 side under its hump).
        let capped = |e: &Value| name != "CAP_FACE" || f3(&e["normal"]).is_some_and(|m| dot(m, n).abs() > 1.0 - 1e-6);
        let cands: Vec<(String, [f64; 3])> = found[kind].as_array().into_iter().flatten().filter(|e| capped(e)).filter_map(|e| Some((e["id"].as_str()?.to_string(), f3(&e[key])?.map(|c| c * k)))).collect();
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
            // A face given as its plane (live's evPlane): ours on that plane, the nearest in area.
            "FACE" if r.plane_origin && let Some(f) = self.face_on_plane(&r) => Ok(f),
            // A sketch's region (refs.jsonl `bodyType` SKETCH): ours there, of its area.
            "FACE" if r.sketch => self.region_at(p, r.measure, r.sketch_of.as_deref()),
            "FACE" | "PLANE" => {
                let part = self.part_of(k, &r);
                self.nearest("faces", &r, part.as_deref()).ok_or_else(|| format!("{id}: no face of ours at {:?} mm", mm(p)))
            }
            // A sketch's line: that line of ours (a part's edge of its length may lie beside it).
            "EDGE" if r.sketch && let Some(line) = self.sketch_line_at(p, r.measure) => Ok(line),
            "EDGE" => {
                let part = self.part_of(k, &r);
                self.nearest("edges", &r, part.as_deref()).ok_or_else(|| format!("{id}: no edge of ours at {:?} mm", mm(p)))
            }
            "VERTEX" => {
                // A part's vertex there, else a sketch's point (a sketch's
                // wire body in live's line).
                let r = self.call("query_entities", json!({ "kind": "vertices", "at": [mm(p)] }))?;
                let k = per_unit(&r["units"]);
                let near = r["vertices"][0]["position"].as_array().is_some_and(|c| (0..3).all(|i| (c[i].as_f64().unwrap_or(f64::NAN) * k - p[i]).abs() < 1e-6));
                match pick(&r, "vertices").filter(|_| near) {
                    Some(v) => Ok(v),
                    None => self.sketch_point_at(p).or_else(|| pick(&r, "vertices")).ok_or_else(|| format!("{id}: no vertex of ours at {:?} mm", mm(p))),
                }
            }
            "BODY" => {
                let parts = self.call("query_parts", json!({}))?;
                let k = per_unit(&parts["units"]);
                let at = |v: &Value, i: usize| v.as_array().and_then(|a| a.get(i)).and_then(Value::as_f64).map_or(f64::NAN, |c| c * k);
                // The part whose box holds live's point, else the nearest box
                // (an earlier difference changed the part; that shows already).
                // With live's box known: the part whose box is nearest it (a
                // pattern's copies lie inside the box of the part they ring).
                let off = |b: &Value| match r.bbox {
                    Some([lo, hi]) => (0..3).map(|i| (at(&b["bounding_box"]["min"], i) - lo[i]).powi(2) + (at(&b["bounding_box"]["max"], i) - hi[i]).powi(2)).sum::<f64>().sqrt(),
                    None => (0..3).map(|i| (at(&b["bounding_box"]["min"], i) - p[i]).max(p[i] - at(&b["bounding_box"]["max"], i)).max(0.0).powi(2)).sum::<f64>().sqrt(),
                };
                let all: Vec<&Value> = parts["parts"].as_array().into_iter().flatten().filter(|b| !off(b).is_nan()).collect();
                let same_kind: Vec<&Value> = all.iter().copied().filter(|b| r.geometry.as_deref().is_some_and(|g| b["kind"] == g)).collect();
                if same_kind.is_empty() { all } else { same_kind }
                    .into_iter()
                    .min_by(|a, b| off(a).total_cmp(&off(b)))
                    .and_then(|b| b["id"].as_str().map(str::to_string))
                    .ok_or_else(|| format!("{id}: no part of ours"))
            }
            "SKETCH_REGION" => self.region_at(p, r.measure, r.sketch_of.as_deref()),
            other => Err(format!("{id}: reference kind {other} not mapped")),
        }
    }

    /// A point of one of our sketches at `p` (model, m), within 1e-6 m.
    fn sketch_point_at(&mut self, p: [f64; 3]) -> Option<String> {
        let sketches: Vec<String> = self.feature_ids.values().cloned().collect();
        for s in sketches {
            let Some(g) = self.sketch_geometry(&s) else { continue };
            let k = per_unit(&g["units"]);
            let (Some(o), Some(x), Some(y)) = (f3(&g["frame"]["origin"]), f3(&g["frame"]["x"]), f3(&g["frame"]["y"])) else { continue };
            for e in g["entities"].as_array().into_iter().flatten().filter(|e| e["type"] == "point") {
                let (Some(u), Some(v)) = (e["at"][0].as_f64(), e["at"][1].as_f64()) else { continue };
                let w: Vec<f64> = (0..3).map(|i| (o[i] + u * x[i] + v * y[i]) * k).collect();
                if (0..3).all(|i| (w[i] - p[i]).abs() < 1e-6) {
                    return e["id"].as_str().map(str::to_string);
                }
            }
        }
        None
    }

    /// A line of one of our sketches with its midpoint at `mid` (model, m)
    /// and live's length, within 1e-6 m.
    fn sketch_line_at(&mut self, mid: [f64; 3], length: Option<f64>) -> Option<String> {
        let sketches: Vec<String> = self.feature_ids.values().cloned().collect();
        for s in sketches {
            let Some(g) = self.sketch_geometry(&s) else { continue };
            let k = per_unit(&g["units"]);
            let (Some(o), Some(x), Some(y)) = (f3(&g["frame"]["origin"]), f3(&g["frame"]["x"]), f3(&g["frame"]["y"])) else { continue };
            let entities = g["entities"].as_array().cloned().unwrap_or_default();
            let at = |id: &Value| -> Option<[f64; 3]> {
                let e = entities.iter().find(|e| e["id"] == *id)?;
                let (u, v) = (e["at"][0].as_f64()?, e["at"][1].as_f64()?);
                Some([0, 1, 2].map(|i| (o[i] + u * x[i] + v * y[i]) * k))
            };
            for e in entities.iter().filter(|e| e["type"] == "line") {
                let (Some(a), Some(b)) = (at(&e["start_point"]), at(&e["end_point"])) else { continue };
                let on = (0..3).all(|i| ((a[i] + b[i]) / 2.0 - mid[i]).abs() < 1e-6);
                let long = length.is_none_or(|l| ((0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f64>().sqrt() - l).abs() < 1e-6);
                if on && long {
                    return e["id"].as_str().map(str::to_string);
                }
            }
        }
        None
    }

    /// Our face or edge for live's: of its type, nearest live's point, the
    /// first whose area / length is live's (a face's point is its box
    /// centre, which may lie in a hole or on an axis), then the first within
    /// 0.2 % of it (live's compact fetch sums tessellation facets: a cylinder's
    /// area 0.04 % low), else the nearest.
    /// Our part for the live part `r` is on, when live's line says (by its box, as a BODY reference).
    fn part_of(&mut self, k: usize, r: &LiveRef) -> Option<String> {
        let body = r.body.clone()?;
        if !self.refs.contains_key(&(k, body.clone())) {
            return None;
        }
        self.by_geometry(k, &body).ok()
    }

    /// `part`: the part of ours live's is on, which decides between equally good ones (two parts with a hole in
    /// one place: cf-fill-pattern's sheet metal part and its solid).
    /// The planar face of ours on live's plane (`r.point` its origin, `r.normal`), the nearest to live's area: a plane's
    /// origin can lie anywhere on it, far from the face (c2-sm-louver's Sketch 9 plane S2E6G, live 2290).
    fn face_on_plane(&mut self, r: &LiveRef) -> Option<String> {
        let (o, n) = (r.point?, r.normal?);
        let reply = self.call("query_entities", json!({ "kind": "faces", "surface": "plane", "limit": 100000 })).ok()?;
        let k = per_unit(&reply["units"]);
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let on = |e: &&Value| {
            let (Some(c), Some(fn_)) = (f3(&e["centroid"]), f3(&e["normal"])) else { return false };
            let c = c.map(|x| x * k);
            dot(fn_, n).abs() > 1.0 - 1e-6 && dot([c[0] - o[0], c[1] - o[1], c[2] - o[2]], n).abs() < 1e-7
        };
        let area = |e: &&Value| e["area"].as_f64().map_or(f64::INFINITY, |a| r.measure.map_or(0.0, |m| (a * k * k - m).abs()));
        reply["faces"].as_array()?.iter().filter(on).min_by(|a, b| area(a).total_cmp(&area(b))).and_then(|e| e["id"].as_str().map(str::to_string))
    }

    fn nearest(&mut self, kind: &str, r: &LiveRef, part: Option<&str>) -> Option<String> {
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
            reply = self.call("query_entities", a.clone()).ok()?;
        }
        let found = reply[kind].as_array().cloned().unwrap_or_default();
        let k = per_unit(&reply["units"]);
        let (key, scale) = if kind == "faces" { ("area", k * k) } else { ("length", k) };
        let within = |tol: f64| move |e: &&Value| r.measure.zip(e[key].as_f64()).is_some_and(|(m, ours)| (ours * scale - m).abs() <= tol * m.abs() + 1e-12);
        let id = |e: &Value| e["id"].as_str().map(str::to_string);
        let exact: Vec<&Value> = found.iter().filter(within(1e-5)).collect();
        if let Some(e) = exact.iter().find(|e| part.is_some_and(|p| e["part"] == p)).or(exact.first()) {
            return id(e);
        }
        // Within the tolerance, the one whose centroid is nearest live's point
        // (a disc and a cylinder can have one area); none of the nearest (a
        // point on an axis is nearer the end faces than a wide cylinder): more
        // of them.
        // (On live's part first: a Thicken's cap and the surface it kept are one face apart, one centroid, and
        // live's display-mesh area, 1.6e-4 off, is exact for neither: cf-surface-pattern's Split 1.)
        let off = |e: &&Value| f3(&e["centroid"]).map_or(f64::INFINITY, |c| (0..3).map(|i| (c[i] * k - p[i]).powi(2)).sum::<f64>());
        let on_part = |e: &&Value| part.is_some_and(|q| e["part"] == q);
        let best = |list: &[Value]| -> Option<String> {
            let near: Vec<&Value> = list.iter().filter(within(2e-3)).collect();
            let theirs: Vec<&Value> = near.iter().copied().filter(on_part).collect();
            if theirs.is_empty() { near } else { theirs }.into_iter().min_by(|x, y| off(x).total_cmp(&off(y))).and_then(id)
        };
        if let Some(e) = best(&found) {
            return Some(e);
        }
        if r.measure.is_some() && found.len() >= 8 {
            a["limit"] = json!(64);
            let more = self.call("query_entities", a.clone()).ok()?;
            if let Some(e) = best(more[kind].as_array().map(Vec::as_slice).unwrap_or(&[])) {
                return Some(e);
            }
        }
        // Live's areas are its display mesh's: a sliver's 3 % off (c2-hunting-knife's spine wedge, exactly 31.892 mm²
        // here and by integration, live 32.868): within 5 %, the nearest in measure (live's part first), before the
        // nearest in space (that wedge's point lies on the curve between it and its 197 mm² neighbour).
        if let Some(m) = r.measure.filter(|m| *m > 0.0) {
            let rel = |e: &&Value| e[key].as_f64().map_or(f64::INFINITY, |ours| (ours * scale - m).abs() / m);
            let close: Vec<&Value> = found.iter().filter(|e| rel(e) <= 5e-2).collect();
            let theirs: Vec<&Value> = close.iter().copied().filter(on_part).collect();
            if let Some(e) = if theirs.is_empty() { close } else { theirs }.into_iter().min_by(|x, y| rel(x).total_cmp(&rel(y))) {
                return id(e);
            }
        }
        found.first().and_then(id)
    }

    /// The sketch region of ours with live's area (when given) nearest
    /// live's point, over every sketch built so far. (Our region `center`
    /// is a label point, not the centroid live gives: area decides.)
    fn region_at(&mut self, p: [f64; 3], area: Option<f64>, sketch_of: Option<&str>) -> Result<String, String> {
        // Its own sketch when live's file names it, else every one of ours.
        let sketches: Vec<String> = match sketch_of.and_then(|f| self.feature_ids.get(f)) {
            Some(ours) => vec![ours.clone()],
            None => self.feature_ids.values().cloned().collect(),
        };
        let mut best: Option<(f64, String)> = None;
        for s in sketches {
            let Some(g) = self.sketch_geometry(&s) else { continue };
            let (o, x, y) = (f3(&g["frame"]["origin"]), f3(&g["frame"]["x"]), f3(&g["frame"]["y"]));
            let (Some(o), Some(x), Some(y)) = (o, x, y) else { continue };
            for r in g["regions"].as_array().into_iter().flatten() {
                let c = &r["center"];
                let (u, v) = (c[0].as_f64().unwrap_or(0.0), c[1].as_f64().unwrap_or(0.0));
                let k = per_unit(&g["units"]);
                let w: Vec<f64> = (0..3).map(|i| (o[i] + u * x[i] + v * y[i]) * k).collect();
                let d = (0..3).map(|i| (w[i] - p[i]).powi(2)).sum::<f64>().sqrt();
                // (1e-3: live's area may come from the region's tessellation.)
                if let Some(a) = area
                    && r["area"].as_f64().is_none_or(|ours| (ours * k * k - a).abs() > 1e-3 * a.max(1e-12))
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
        // A plane named by id alone (a Plane feature's, older documents):
        // ours that lies where live's sketch frame does.
        let plane = match self.param(k, plane_q) {
            Ok(v) => v.and_then(|v| v.as_array().and_then(|a| a.first()).cloned()).ok_or("no sketch plane")?,
            Err(e) => self.plane_at_frame(&oid).map(Value::from).ok_or(e)?,
        };
        let mut args = json!({ "plane": plane });
        if let Some(n) = f["name"].as_str().filter(|n| !n.starts_with('#')) {
            args["name"] = json!(n);
        }
        let made = self.call("sketch_create", args)?;
        let id = made["sketch"].as_str().ok_or("sketch_create gave no id")?.to_string();
        let frame = self.frames.get(&oid).copied();
        // A direction in Onshape's sketch (x, y) → ours ([x, y] mm): through
        // the model, onto our sketch's axes.
        let our_axes = (f3(&made["frame"]["x"]), f3(&made["frame"]["y"]));
        let along = |u: f64, v: f64| -> Value {
            let (a, b) = match (frame, our_axes) {
                (Some([_, x, y]), (Some(ox), Some(oy))) => {
                    let w: [f64; 3] = std::array::from_fn(|i| u * x[i] + v * y[i]);
                    ((0..3).map(|i| w[i] * ox[i]).sum::<f64>(), (0..3).map(|i| w[i] * oy[i]).sum::<f64>())
                }
                _ => (u, v),
            };
            json!([format!("{} mm", a * 1000.0), format!("{} mm", b * 1000.0)])
        };
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
        let mut texts: Vec<Value> = Vec::new();
        // Live's Use (PROJECTED): the entity's model geometry, used after the drawing as live's sketch uses it.
        let projected: HashMap<String, Value> = f["constraints"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["constraintType"] == "PROJECTED")
            .filter_map(|c| {
                let params = c["parameters"].as_array()?;
                let local = params.iter().find(|p| p["parameterId"] == "localFirst")?["value"].as_str()?.to_string();
                Some((local, params.iter().find(|p| p["parameterId"] == "externalSecond")?.clone()))
            })
            .collect();
        let mut uses: Vec<Use> = Vec::new();
        for e in f["entities"].as_array().into_iter().flatten() {
            // Where live's solve put it (`sketches` geomEntities): stored
            // geometry can lag its variables (cf-parameter-pattern).
            let solved = self.solved_entity(&oid, e);
            let e = solved.as_ref().unwrap_or(e);
            let eid = e["entityId"].as_str().unwrap_or("").to_string();
            let g = &e["geometry"];
            let construction = e["isConstruction"].as_bool() == Some(true);
            if construction {
                self.construction.insert((oid.clone(), eid.clone()));
            }
            let gt = g["btType"].as_str().unwrap_or("");
            let num = |k: &str| g[k].as_f64().unwrap_or(0.0);
            // Text: placed after the curves (sketch_text).
            if e["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketchTextEntity")) {
                texts.push(e.clone());
                continue;
            }
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
                            // The tool's third click sets the minor radius by its distance across the major axis and the
                            // start by its angle: the click with both, live's b and its start's angle (on the axis: the
                            // start itself).
                            let (c0, s0) = (t0.cos(), sense * t0.sin());
                            let third = if s0.abs() > 1e-6 {
                                let (k, m) = (a * c0 / s0.abs(), b * s0.signum());
                                json!(at(cx + k * xd - m * yd, cy + k * yd + m * xd))
                            } else {
                                pt(t0)
                            };
                            Some(json!({ "tool": "elliptical_arc", "points": [at(cx, cy), pt(0.0), third, pt(t1)], "construction": construction }))
                        }
                        _ => Some(json!({ "tool": "ellipse", "points": [at(cx, cy), pt(0.0), pt(std::f64::consts::FRAC_PI_2)], "construction": construction })),
                    }
                }
                // Onshape's points are start, control, end; ours are clicked start, end, control. Its rho: the RHO
                // constraint (`constraints`) sets the Rho dimension our conic gets.
                "BTCurveGeometryConic" => {
                    let p: Vec<f64> = g["points"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
                    (p.len() == 6).then(|| json!({ "tool": "conic", "points": [at(p[0], p[1]), at(p[4], p[5]), at(p[2], p[3])], "construction": construction }))
                }
                "BTCurveGeometryInterpolatedSpline" => {
                    let flat: Vec<f64> = g["interpolationPoints"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
                    let derivs = ["startDerivativeX", "startDerivativeY", "endDerivativeX", "endDerivativeY"].iter().any(|k| num(k).abs() > 1e-12);
                    // Periodic (closed): drawn as the Spline tool closes one, its first point clicked again.
                    let periodic = g["isPeriodic"].as_bool() == Some(true);
                    let closing = if periodic { flat.get(..2).unwrap_or_default().to_vec() } else { Vec::new() };
                    (flat.len() >= 4).then(|| {
                        let mut s = json!({ "tool": "spline", "points": flat.chunks(2).chain(closing.chunks(2)).map(|c| at(c[0], c[1])).collect::<Vec<_>>(), "construction": construction });
                        // Onshape's end derivatives (its handles).
                        if derivs && !periodic {
                            s["start_derivative"] = along(num("startDerivativeX"), num("startDerivativeY"));
                            s["end_derivative"] = along(num("endDerivativeX"), num("endDerivativeY"));
                        }
                        s
                    })
                }
                // A stored B-spline (a Use's, an offset's: BTCurveGeometrySpline; a control-point spline): its poles,
                // degree, knots and weights.
                "BTCurveGeometryControlPointSpline" | "BTCurveGeometrySpline" => {
                    let flat: Vec<f64> = g["controlPoints"].as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
                    let n = flat.len() / 2;
                    let nums = |k: &str| g[k].as_array().map(|l| l.iter().filter_map(Value::as_f64).collect::<Vec<f64>>());
                    let mut shape = json!({ "tool": "bezier", "points": flat.chunks(2).map(|c| at(c[0], c[1])).collect::<Vec<_>>(), "construction": construction });
                    let degree = g["degree"].as_u64().unwrap_or(0) as usize;
                    let knots = nums("knots").filter(|k| k.len() == n + degree + 1);
                    if g["isPeriodic"].as_bool() == Some(true) || n < 2 || degree == 0 {
                        None
                    } else if let Some(knots) = knots {
                        shape["degree"] = json!(degree);
                        shape["knots"] = json!(knots);
                        if g["isRational"].as_bool() == Some(true) {
                            shape["weights"] = json!(nums("weights").unwrap_or_default());
                        }
                        Some(shape)
                    } else {
                        (degree == n - 1 && g["isRational"].as_bool() != Some(true)).then_some(shape)
                    }
                }
                _ if e["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketchPoint")) => Some(json!({ "tool": "point", "points": [at(e["x"].as_f64().unwrap_or(0.0), e["y"].as_f64().unwrap_or(0.0))], "construction": construction })),
                _ => None,
            };
            let poly = sampled(e);
            let end_points = poly.as_ref().filter(|p| p.len() >= 2 && !e["startParam"].is_null()).map(|p| [p[0], p[p.len() - 1]]);
            if let Some(poly) = poly {
                self.curves.insert((oid.clone(), eid.clone()), poly);
            }
            let used = shape.as_ref().and_then(|_| projected.get(&eid)).and_then(|p| self.external_ref(k, p));
            match shape {
                Some(s) if used.is_some() => {
                    let point = (s["tool"] == "point").then(|| [e["x"].as_f64().unwrap_or(0.0), e["y"].as_f64().unwrap_or(0.0)]);
                    uses.push(Use { eid, reference: used.unwrap_or_default(), shape: s, ends: end_points, point });
                }
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
                // (Not a spline's handle arms: they come with it, not drawn for live's entities.)
                let made: Vec<&Value> = r["created"].as_array().into_iter().flatten().filter(|c| (c["type"] == "point") == points && c.get("handle_of").is_none()).collect();
                for (eid, c) in ids.iter().zip(made) {
                    self.map_drawn(&oid, eid, c, points, ends_of.get(eid).copied());
                }
            }
        }
        for u in uses {
            self.use_projected(&oid, &id, u, problems);
        }
        for t in &texts {
            if let Err(e) = self.sketch_text(&oid, &id, t, map) {
                problems.push(format!("sketch text {}: {e}", t["entityId"].as_str().unwrap_or("?")));
            }
        }
        self.unsnap(&oid, &id, map);
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

    /// A text entity as the Text tool makes it: its first baseline's start,
    /// the ascent as height, the font file's family and style, mirrored as
    /// live's; turned when its baseline isn't along our x.
    fn sketch_text(&mut self, oid: &str, id: &str, t: &Value, map: SketchMap) -> Result<(), String> {
        let param = |name: &str| t["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).map(|p| p["value"].clone()).unwrap_or(Value::Null);
        let num = |k: &str| t[k].as_f64().ok_or_else(|| format!("no {k}"));
        let (x, y, dx, dy, ascent) = (num("baselineStartX")?, num("baselineStartY")?, num("baselineDirectionX")?, num("baselineDirectionY")?, num("ascent")?);
        let (start, ahead) = (map.local([x, y]), map.local([x + dx, y + dy]));
        let mm = |v: f64| format!("{} mm", v * 1e3);
        // "OpenSans-BoldItalic.ttf": family "Open Sans", bold, italic.
        let file = param("fontName").as_str().unwrap_or("").trim_end_matches(".ttf").to_string();
        let (family, style) = file.split_once('-').unwrap_or((file.as_str(), "Regular"));
        let mut name = String::new();
        for (i, c) in family.chars().enumerate() {
            if i > 0 && c.is_uppercase() && family.chars().nth(i - 1).is_some_and(char::is_lowercase) {
                name.push(' ');
            }
            name.push(c);
        }
        // The box's height is Onshape's `ascent` (0.72 of the font size).
        // Its baseline vector is the advances plus ~40 font units (39-40 of
        // 2048 in cf-fix-pcb's and cf-straight-spline's Open Sans), so only
        // without an ascent is the height what lays the text out that wide.
        // A text converted to an expression is a quantity parameter with its expression (c2-valve-config's
        // `toString(roundToPrecision(#espesor_material/mm, 0))`).
        let expression = t["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == "text").and_then(|p| p["expression"].as_str()).filter(|x| !x.trim().is_empty()).map(str::to_string);
        let text = param("text").as_str().unwrap_or("").to_string();
        let font = if name.is_empty() { noshape_sketch::text::SKETCH_DEFAULT_FONT.to_string() } else { name.clone() };
        let (bold, italic) = (style.contains("Bold"), style.contains("Italic"));
        let advance = noshape_sketch::text::layout_sketch_text(&text, &font, bold, italic).width;
        let live_width = dx.hypot(dy);
        let height = if ascent > 1e-12 {
            ascent
        } else if advance > 1e-9 && live_width > 1e-9 {
            live_width / advance
        } else {
            0.0
        };
        let mut args = json!({ "sketch": id, "text": text, "corner": [mm(start[0]), mm(start[1])], "height": mm(height),
            "bold": style.contains("Bold"), "italic": style.contains("Italic"), "flip_horizontal": param("mirrorHorizontal") == true, "flip_vertical": param("mirrorVertical") == true });
        if !name.is_empty() {
            args["font"] = json!(name);
        }
        if let Some(expression) = expression {
            args["text"] = json!(expression);
            args["expression"] = json!(true);
        }
        let made = self.call("sketch_text", args)?;
        // Its box's baseline and left side by live's names (ours: bottom, right, top, left; the bottom on the
        // baseline), for the constraints and dimensions on them (c2-valve-config's height of 4.5 mm, its left side's
        // midpoint). Not the right and top: their place is the text's width, which our font's advances give a hair
        // off live's (c2-hex-bookcase-derived's MIDPOINT on a right side moved it 0.0125 mm).
        let lines: Vec<String> = made["created"].as_array().into_iter().flatten().filter(|c| c["type"] == "line").filter_map(|c| c["id"].as_str().map(str::to_string)).collect();
        if let (Some(tid), [bottom, _, _, left]) = (t["entityId"].as_str(), lines.as_slice()) {
            for (side, line) in [("baseline", bottom), ("left", left)] {
                self.sketch_entities.insert((oid.to_string(), format!("{tid}.sketch_text.{side}")), line.clone());
            }
        }
        let angle = (ahead[1] - start[1]).atan2(ahead[0] - start[0]);
        if angle.abs() > 1e-9 {
            let created: Vec<String> = made["created"].as_array().into_iter().flatten().filter_map(|c| c["id"].as_str().map(str::to_string)).collect();
            self.call("sketch_transform", json!({ "sketch": id, "entities": created, "rotate": format!("{angle} rad"), "center": [mm(start[0]), mm(start[1])] }))?;
        }
        Ok(())
    }

    /// Our line tool's Horizontal / Vertical inference snaps a line live
    /// solved a hair off the axis (exercise-anthony's patterned hexagon,
    /// 0.03°): those constraints go and the line's ends back to live's solve.
    fn unsnap(&mut self, oid: &str, id: &str, map: SketchMap) {
        let Ok(g) = self.call("sketch_get", json!({ "sketch": id })) else { return };
        let k = per_unit(&g["units"]);
        let live_of: HashMap<&str, &str> = self.sketch_entities.iter().filter(|((o, _), _)| o == oid).map(|((_, e), c)| (c.as_str(), e.as_str())).collect();
        let p2 = |v: &Value| Some([v[0].as_f64()?, v[1].as_f64()?]);
        let mut snapped = Vec::new();
        let mut moves = Vec::new();
        for c in g["constraints"].as_array().into_iter().flatten() {
            let axis = match c["type"].as_str() {
                Some("Horizontal") => 1,
                Some("Vertical") => 0,
                _ => continue,
            };
            let Some([line]) = c["entities"].as_array().map(Vec::as_slice) else { continue };
            let Some(eid) = line.as_str().and_then(|l| live_of.get(l)) else { continue };
            let Some(solve) = self.solved.get(&(oid.to_string(), eid.to_string())).filter(|s| s["entityType"] == "lineSegment") else { continue };
            let (Some(a), Some(b)) = (p2(&solve["startPoint"]), p2(&solve["endPoint"])) else { continue };
            let (a, b) = (map.local(a), map.local(b));
            if (b[axis] - a[axis]).abs() <= 1e-9 {
                continue;
            }
            snapped.push(c["id"].clone());
            for (end, at) in [("start", a), ("end", b)] {
                if let Some(pid) = self.sketch_entities.get(&(oid.to_string(), format!("{eid}.{end}"))) {
                    moves.push((pid.clone(), at));
                }
            }
        }
        if snapped.is_empty() || self.call("sketch_delete", json!({ "sketch": id, "constraints": snapped })).is_err() {
            return;
        }
        let Ok(g) = self.call("sketch_get", json!({ "sketch": id })) else { return };
        for (pid, at) in moves {
            let Some(now) = g["entities"].as_array().into_iter().flatten().find(|e| e["id"] == pid.as_str()).and_then(|e| p2(&e["at"])) else { continue };
            let d = [at[0] / k - now[0], at[1] / k - now[1]];
            if d[0].hypot(d[1]) * k > 1e-12 {
                let _ = self.call("sketch_transform", json!({ "sketch": id, "entities": [pid], "translate": d }));
            }
        }
    }

    /// Live's OFFSET (`localSecondOffset` the copy of `localSecond`, tied to the first pair `localMaster` /
    /// `localOffset`): the pair already drawn linked as our Offset tool links its copies.
    fn sketch_offset(&mut self, oid: &str, id: &str, c: &Value) -> Result<(), String> {
        let ours = |name: &str| -> Result<String, String> {
            let v = c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).and_then(|p| p["value"].as_str()).ok_or_else(|| format!("no {name}"))?;
            self.sketch_entities.get(&(oid.to_string(), v.to_string())).cloned().ok_or_else(|| format!("{v} not translated"))
        };
        if c["parameters"].as_array().into_iter().flatten().any(|p| p["parameterId"] == "externalMaster") {
            return Err("to external geometry".into());
        }
        let (ms, mc) = (ours("localMaster")?, ours("localOffset")?);
        // (A lone pair: its own master.)
        let has = |name: &str| c["parameters"].as_array().into_iter().flatten().any(|p| p["parameterId"] == name);
        let (source, copy) = if has("localSecond") { (ours("localSecond")?, ours("localSecondOffset")?) } else { (ms.clone(), mc.clone()) };
        let mut args = json!({ "sketch": id, "source": source, "copy": copy, "master_source": ms, "master_copy": mc });
        if let Some(tab) = &self.tab {
            args["tab"] = json!(tab);
        }
        crate::ops::link_sketch_offset(self.ctx, &args).map_err(|e| e.message)?;
        self.sketch_edits += 1;
        Ok(())
    }

    /// A sketch pattern (live's CIRCULAR_PATTERN: `localInstance<j>,<k>` the instance k's entity j, the seeds k = 0,
    /// about `localPivot`; LINEAR_PATTERN: `localInstance<j>,<a>,<b>` on a grid): the instances already drawn linked as
    /// our pattern tool links its copies.
    fn sketch_pattern(&mut self, oid: &str, id: &str, c: &Value) -> Result<(), String> {
        let params = c["parameters"].as_array().ok_or("no parameters")?;
        let mut cells: Vec<(Vec<usize>, String)> = Vec::new();
        for p in params {
            let Some(at) = p["parameterId"].as_str().and_then(|n| n.strip_prefix("localInstance")) else { continue };
            let idx: Vec<usize> = at.split(',').map(|x| x.parse::<usize>()).collect::<Result<_, _>>().map_err(|_| "an instance index")?;
            let v = p["value"].as_str().ok_or("an instance entity")?;
            // (An instance deleted in live: its entities' ids empty, the instance left out.)
            if v.is_empty() {
                continue;
            }
            let ours = self.sketch_entities.get(&(oid.to_string(), v.to_string())).ok_or_else(|| format!("{v} not translated"))?;
            cells.push((idx, ours.clone()));
        }
        let size = |d: usize| cells.iter().map(|(i, _)| i.get(d).copied().unwrap_or(0) + 1).max().unwrap_or(0);
        let (nj, linear) = (size(0), c["constraintType"] == "LINEAR_PATTERN");
        let entity = |i: &[usize]| cells.iter().find(|(x, _)| x == i).map(|(_, e)| json!(e));
        let mut args = json!({ "sketch": id });
        if let Some(tab) = &self.tab {
            args["tab"] = json!(tab);
        }
        if linear {
            let grid: Vec<Vec<Vec<Value>>> = (0..size(1)).map(|a| (0..size(2)).map(|b| (0..nj).filter_map(|j| entity(&[j, a, b])).collect()).collect()).collect();
            args["grid"] = json!(grid);
        } else {
            let instances: Vec<Vec<Value>> = (0..size(1)).map(|k| (0..nj).filter_map(|j| entity(&[j, k])).collect::<Vec<_>>()).filter(|g| g.len() == nj).collect();
            let pivot = params.iter().find(|p| p["parameterId"] == "localPivot").and_then(|p| p["value"].as_str()).ok_or("its pivot is model geometry")?;
            // (A circle as the pivot: its centre.)
            let center = self.sketch_entities.get(&(oid.to_string(), format!("{pivot}.center"))).or_else(|| self.sketch_entities.get(&(oid.to_string(), pivot.to_string()))).ok_or("its pivot not translated")?;
            args["circular"] = json!(instances);
            args["center"] = json!(center);
            args["full"] = json!(!params.iter().any(|p| p["parameterId"] == "openPattern" && p["value"] == true));
        }
        crate::ops::link_sketch_pattern(self.ctx, &args).map_err(|e| e.message)?;
        self.sketch_edits += 1;
        Ok(())
    }

    /// A conic's RHO: our conic's Rho dimension (made with it) set to live's value.
    fn conic_rho(&mut self, oid: &str, id: &str, c: &Value) -> Result<(), String> {
        let par = |name: &str| c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).cloned().unwrap_or(Value::Null);
        let conic = par("localFirst")["value"].as_str().and_then(|v| self.sketch_entities.get(&(oid.to_string(), v.to_string()))).cloned().ok_or("its conic not translated")?;
        let rho = par("rho");
        let value = rho["expression"].as_str().map(str::to_string).or_else(|| rho["value"].as_f64().map(|v| v.to_string())).ok_or("no rho")?;
        let ours = self.call("sketch_get", json!({ "sketch": id }))?;
        let dim = ours["constraints"].as_array().into_iter().flatten().find(|d| d["type"] == "Rho" && d["entities"].as_array().is_some_and(|e| e.iter().any(|x| x == conic.as_str()))).and_then(|d| d["id"].as_str().map(str::to_string)).ok_or("our conic has no Rho dimension")?;
        self.call("sketch_set_dimension", json!({ "sketch": id, "dimension": dim, "value": value }))?;
        self.sketch_edits += 1;
        Ok(())
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
                // A point at an ellipse's axis end: inferred when ours was drawn there (kept), else on the ellipse.
                "QUADRANT" => "quadrant",
                // Dimensions: `dimensions`.
                "DISTANCE" | "LENGTH" | "DIAMETER" | "RADIUS" | "ANGLE" | "CENTERLINE_DIMENSION" | "MAJOR_DIAMETER" | "MINOR_DIAMETER" => continue,
                "OFFSET" => {
                    if let Err(e) = self.sketch_offset(oid, id, c) {
                        *missed.entry(format!("{ty} ({e})")).or_default() += 1;
                    }
                    continue;
                }
                "CIRCULAR_PATTERN" | "LINEAR_PATTERN" => {
                    if let Err(e) = self.sketch_pattern(oid, id, c) {
                        *missed.entry(format!("{ty} ({e})")).or_default() += 1;
                    }
                    continue;
                }
                // Ours made with Use (`use_projected`), else drawn where live's solve put it.
                "PROJECTED" => {
                    let local = c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == "localFirst").and_then(|p| p["value"].as_str());
                    if !local.is_some_and(|l| self.used.contains(&(oid.to_string(), l.to_string()))) {
                        *missed.entry(format!("{ty} (not used: its model geometry not found)")).or_default() += 1;
                    }
                    continue;
                }
                "RHO" => {
                    if let Err(e) = self.conic_rho(oid, id, c) {
                        problems.push(format!("RHO on {}: {e}", c["entityId"].as_str().unwrap_or("?")));
                    }
                    continue;
                }
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
                if ty == "COINCIDENT" {
                    self.pin_external_vertex(k, oid, c);
                }
                // An external reference live's sketch no longer finds (its query empty): ours to nothing too, at
                // the local point, so the sketch warns as live's.
                let gone = c["parameters"].as_array().into_iter().flatten().filter(|p| p["parameterId"].as_str().is_some_and(|n| n.starts_with("external"))).any(|p| {
                    p["queries"].as_array().is_some_and(|qs| !qs.is_empty() && qs.iter().all(|q| q["queryString"].as_str().unwrap_or("").is_empty() && q["deterministicIds"].as_array().is_none_or(|ids| ids.iter().all(|i| i.as_str().is_none_or(str::is_empty)))))
                });
                if gone && let Some(point) = entities.first() {
                    let mut args = json!({ "sketch": id, "entity": point });
                    if let Some(tab) = &self.tab {
                        args["tab"] = json!(tab);
                    }
                    if crate::ops::pin_to_missing_reference(self.ctx, &args).is_ok() {
                        continue;
                    }
                }
                *missed.entry(format!("{ty} to external geometry")).or_default() += 1;
                continue;
            }
            if ours == "coincident" && entities.len() == 2 && entities[0] == entities[1] {
                continue;
            }
            if ours == "quadrant" {
                let have = entities.len() == 2
                    && self.call("sketch_get", json!({ "sketch": id })).is_ok_and(|g| {
                        g["constraints"].as_array().into_iter().flatten().any(|q| q["type"] == "Quadrant" && entities.iter().all(|e| q["entities"].as_array().is_some_and(|l| l.iter().any(|x| x == e.as_str()))))
                    });
                if have {
                    continue;
                }
            }
            let ours = if ours == "quadrant" { "coincident" } else { ours };
            // A point mirrored onto itself (ours merged the pair: a point on the mirror line) lies on the line.
            let (ours, entities) = if ours == "symmetric" && entities.len() == 3 && entities[0] == entities[1] { ("coincident", vec![entities[0].clone(), entities[2].clone()]) } else { (ours, entities) };
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

    /// A vertex live gives no geometry for (a sketch's point in ids-only
    /// lines), placed where sketch `oid` makes one of its points coincident
    /// with it (live's solve), for later queries naming it.
    fn pin_external_vertex(&mut self, k: usize, oid: &str, c: &Value) {
        let params = c["parameters"].as_array().into_iter().flatten();
        let mut local = None;
        let mut id = None;
        for p in params {
            let n = p["parameterId"].as_str().unwrap_or("");
            if n.starts_with("local") {
                local = p["value"].as_str().filter(|v| !v.is_empty());
            } else if n.starts_with("external")
                && let [q] = p["queries"].as_array().map(Vec::as_slice).unwrap_or_default()
                && let Some([one]) = q["deterministicIds"].as_array().map(Vec::as_slice)
            {
                id = one.as_str();
            }
        }
        let (Some(local), Some(id)) = (local, id) else { return };
        if self.refs.keys().any(|(_, i)| i == id) {
            return;
        }
        let Some(&[o, x, y]) = self.frames.get(oid) else { return };
        let Some([u, v]) = self.solved.get(&(oid.to_string(), local.to_string())).and_then(|g| Some([g["point"][0].as_f64()?, g["point"][1].as_f64()?])) else { return };
        let at = [0, 1, 2].map(|i| o[i] + u * x[i] + v * y[i]);
        self.refs.insert((k, id.to_string()), LiveRef { kind: "VERTEX".into(), point: Some(at), ..Default::default() });
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
            let mut handle: Option<[String; 2]> = None;
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
                        None => match self.handle_lines.get(&(oid.to_string(), v.clone())) {
                            Some(line) => handle = Some(line.clone()),
                            None => problems.push(format!("{ty} dimension: sketch entity {v} not translated")),
                        },
                    }
                }
            }
            // On a spline's handle line: its length the distance from the spline's end to the tip; an angle to it
            // has no counterpart (ours has no handle line).
            let mut kind = kind;
            if let Some([at, tip]) = handle {
                if kind != "length" || !entities.is_empty() {
                    *external.entry(format!("{ty} on a spline handle (no counterpart)")).or_default() += 1;
                    continue;
                }
                entities = vec![at, tip];
                kind = "distance";
            }
            // To model geometry: left to the solved geometry, as constraints are.
            if missing {
                *external.entry(format!("{ty} to external geometry")).or_default() += 1;
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
            problems.push(format!("dimensions left to the solved geometry: {}", external.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")));
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

/// The written-out form of a sketch region query (`makeQuery(id +
/// "<sketch>.imprint", "IMPRINT", EntityType.FACE, { "disambiguationData" :
/// [topologyDisambiguation([[makeQuery(… "derivedFrom" :
/// sketchEntityQuery(id + "<sketch>.wireOp", EntityType.EDGE, "<entity>")
/// …`; learning-center-devon) as the words its packed form gives: its sketch
/// and the curves it names.
fn imprint_query_words(text: &str) -> (Option<String>, Vec<String>) {
    let Some(sk) = text.split("makeQuery(id+\"").nth(1).and_then(|r| r.split('"').next()).and_then(|o| o.strip_suffix(".imprint")) else { return (None, Vec::new()) };
    if !text.contains("\"IMPRINT\",EntityType.FACE") || !text.contains("topologyDisambiguation") {
        return (None, Vec::new());
    }
    let mut words: Vec<String> = ["SKETCH_ENTITY", "FACE", "TOPOLOGY", "IMPRINT"].map(String::from).to_vec();
    for entity in text.split("sketchEntityQuery(").skip(1) {
        let mut quoted = entity.split('"').skip(1).step_by(2);
        let (Some(op), Some(e)) = (quoted.next(), quoted.next()) else { continue };
        words.extend(["operationId".to_string(), op.to_string(), "sketchEntityId".to_string(), e.to_string()]);
    }
    (Some(sk.to_string()), words)
}

/// Where polylines `a` and `b` cross, in order along `a`.
fn crossings(a: &[[f64; 2]], b: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<(f64, [f64; 2])> = Vec::new();
    for (i, p) in a.windows(2).enumerate() {
        for q in b.windows(2) {
            let (d, e) = ([p[1][0] - p[0][0], p[1][1] - p[0][1]], [q[1][0] - q[0][0], q[1][1] - q[0][1]]);
            let den = d[0] * e[1] - d[1] * e[0];
            if den.abs() < 1e-30 {
                continue;
            }
            let w = [q[0][0] - p[0][0], q[0][1] - p[0][1]];
            let (t, u) = ((w[0] * e[1] - w[1] * e[0]) / den, (w[0] * d[1] - w[1] * d[0]) / den);
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                let at = [p[0][0] + t * d[0], p[0][1] + t * d[1]];
                if out.iter().all(|(_, o)| (o[0] - at[0]).hypot(o[1] - at[1]) > 1e-9) {
                    out.push((i as f64 + t, at));
                }
            }
        }
    }
    out.sort_by(|x, y| x.0.total_cmp(&y.0));
    out.into_iter().map(|(_, p)| p).collect()
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
/// A custom feature's lookup path as live gives it (level key → option: `{"standard": "ANSI", "fit": "Free", …}`), as
/// ours takes it: the options in the order of its table's levels (the table its Feature Studio defines), a level live
/// leaves out at its default. Other values as they are.
fn lookup_path_in_level_order(ty: &str, id: &str, v: Value) -> Result<Value, String> {
    let Value::Object(by_level) = &v else { return Ok(v) };
    let table = noshape_model::features::get(ty).and_then(|t| t.params.iter().find(|s| s.id == id)).and_then(|s| match s.kind {
        noshape_model::params::ParamKind::Lookup { table } => Some(table),
        _ => None,
    });
    let Some(table) = table else { return Ok(v) };
    // (A level live leaves out is at its default: T-Slot Joint's nut shape and type.)
    use noshape_model::features::hole_tables::{table_level_default, table_level_key};
    let mut path: Vec<String> = Vec::new();
    let mut used = 0;
    while let Some(level) = table_level_key(table, &path) {
        let option = match by_level.get(&level).and_then(Value::as_str) {
            Some(o) => {
                used += 1;
                o.to_string()
            }
            None => match table_level_default(table, &path) {
                Some(d) => d,
                None => break,
            },
        };
        path.push(option);
    }
    if used < by_level.len() {
        return Err(format!("lookup path {v} only partly in its table ({})", path.join(" › ")));
    }
    Ok(json!(path))
}

/// A packed query's ORDER disambiguation (`{ "disambiguationType" : "ORDER", "order" : n }`, anywhere in it).
fn packed_order(p: &noshape_model::custom::host::packed::Packed) -> Option<usize> {
    use noshape_model::custom::host::packed::Packed;
    match p {
        Packed::Map(m) if m.iter().any(|(k, v)| matches!((k, v), (Packed::Str(k), Packed::Str(v)) if k == "disambiguationType" && v == "ORDER")) => m.iter().find_map(|(k, v)| match (k, v) {
            (Packed::Str(k), Packed::Num(n)) if k == "order" => Some(*n as usize),
            _ => None,
        }),
        Packed::Map(m) => m.iter().find_map(|(_, v)| packed_order(v)),
        Packed::Array(a) => a.iter().find_map(packed_order),
        Packed::Tagged(_, v) => packed_order(v),
        _ => None,
    }
}

/// A written query with its `var subQk=…;` definitions put in where they are used: `{var subQ0=A;query=f(subQ0);}`
/// → `f(A)`.
fn inline_query_vars(text: &str) -> Option<String> {
    let body = text.trim().trim_start_matches('{').trim_end_matches('}');
    let mut vars: Vec<(String, String)> = Vec::new();
    let mut query = None;
    for part in body.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        if let Some((name, value)) = part.strip_prefix("var ").and_then(|v| v.split_once('=')) {
            vars.push((name.trim().to_string(), value.to_string()));
        } else if let Some(q) = part.strip_prefix("query=") {
            query = Some(q.to_string());
        }
    }
    let mut query = query?;
    // (subQ10 before subQ1.)
    vars.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    for (name, value) in &vars {
        query = query.replace(name.as_str(), value);
    }
    Some(query)
}

/// The items of a written list up to its closing `]`, split at its own commas: `a(1,2),b]…` → [`a(1,2)`, `b`].
fn top_level_items(rest: &str) -> Option<Vec<String>> {
    let (mut depth, mut start, mut quoted) = (0i32, 0, false);
    let mut items = Vec::new();
    for (i, c) in rest.char_indices() {
        match c {
            '"' => quoted = !quoted,
            _ if quoted => {}
            '(' | '[' | '{' => depth += 1,
            ')' | '}' => depth -= 1,
            ']' if depth == 0 => {
                items.push(rest[start..i].trim().to_string());
                return Some(items.into_iter().filter(|s| !s.is_empty()).collect());
            }
            ']' => depth -= 1,
            ',' if depth == 0 => {
                items.push(rest[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

fn packed_strings_of(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(packed) = text.split("qCompressed(1.0,\"").nth(1).and_then(|r| r.split('"').next())
        && let Ok(tree) = noshape_model::custom::host::packed::unpack(packed)
    {
        packed_strings(&tree, &mut out);
    }
    out
}

/// The namespaced imports (`ns::import(path : "…", …)`) a FeatureScript
/// source uses only as `ns::BLOB_DATA` (icons, description images).
fn blob_imports(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (at, _) in source.match_indices("::import(path : \"") {
        let ns: String = source[..at].chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<Vec<_>>().into_iter().rev().collect();
        let rest = &source[at + "::import(path : \"".len()..];
        let Some(path) = rest.split('"').next() else { continue };
        let uses = source.matches(&format!("{ns}::")).count();
        let blobs = source.matches(&format!("{ns}::BLOB_DATA")).count();
        if !ns.is_empty() && uses == blobs + 1 && blobs > 0 {
            out.push((ns, path.to_string()));
        }
    }
    out
}

/// A packed query's own operation id (`operationId` of its top map).
fn root_operation(p: &noshape_model::custom::host::packed::Packed) -> Option<String> {
    use noshape_model::custom::host::packed::Packed;
    let Packed::Tagged(_, inner) = p else { return None };
    let Packed::Map(m) = inner.as_ref() else { return None };
    let (_, id) = m.iter().find(|(k, _)| matches!(k, Packed::Str(s) if s == "operationId"))?;
    let Packed::Tagged(_, ids) = id else { return None };
    let Packed::Array(a) = ids.as_ref() else { return None };
    match a.first()? {
        Packed::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// Every string in a packed query.
/// A packed face query's own edges (its TOPOLOGY's entities, not the curves those are told apart by in turn): their
/// sketch entity ids.
fn packed_face_edges(p: &noshape_model::custom::host::packed::Packed) -> Vec<String> {
    use noshape_model::custom::host::packed::Packed;
    fn field<'a>(p: &'a Packed, key: &str) -> Option<&'a Packed> {
        let Packed::Map(kv) = (match p {
            Packed::Tagged(_, inner) => inner.as_ref(),
            other => other,
        }) else {
            return None;
        };
        kv.iter().find(|(k, _)| matches!(k, Packed::Str(s) if s == key)).map(|(_, v)| v)
    }
    fn entity_id(q: &Packed) -> Option<String> {
        match field(q, "sketchEntityId") {
            Some(Packed::Str(s)) => Some(s.clone()),
            _ => field(q, "derivedFrom").and_then(entity_id),
        }
    }
    let mut out = Vec::new();
    let Some(Packed::Array(data)) = field(p, "disambiguationData") else { return out };
    for d in data {
        if !matches!(field(d, "disambiguationType"), Some(Packed::Str(t)) if t == "TOPOLOGY") {
            continue;
        }
        let Some(Packed::Array(groups)) = field(d, "entities") else { continue };
        for g in groups {
            let items: Vec<&Packed> = match g {
                Packed::Array(a) => a.iter().collect(),
                other => vec![other],
            };
            out.extend(items.into_iter().filter_map(entity_id));
        }
    }
    out
}

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
        // A conic: the rational quadratic Bézier of its start, control and end, the control's weight rho / (1 - rho).
        "BTCurveGeometryConic" => {
            let p: Vec<f64> = g["points"].as_array()?.iter().filter_map(Value::as_f64).collect();
            let rho = g["rho"].as_f64()?;
            if p.len() != 6 || !(0.0..1.0).contains(&rho) {
                return None;
            }
            let w = rho / (1.0 - rho);
            Some(
                (0..=16)
                    .map(|i| {
                        let t = i as f64 / 16.0;
                        let (a, b, c) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t) * w, t * t);
                        let d = a + b + c;
                        [(a * p[0] + b * p[2] + c * p[4]) / d, (a * p[1] + b * p[3] + c * p[5]) / d]
                    })
                    .collect(),
            )
        }
        // Splines through their fit or control points: enough to tell which
        // side of them a region lies.
        "BTCurveGeometryInterpolatedSpline" | "BTCurveGeometryControlPointSpline" | "BTCurveGeometrySpline" => {
            let key = if g["interpolationPoints"].is_array() { "interpolationPoints" } else { "controlPoints" };
            let flat: Vec<f64> = g[key].as_array()?.iter().filter_map(Value::as_f64).collect();
            let mut poly: Vec<[f64; 2]> = flat.chunks(2).map(|c| [c[0], c[1]]).collect();
            // A periodic one closes.
            if g["isPeriodic"].as_bool() == Some(true)
                && let Some(&first) = poly.first()
            {
                poly.push(first);
            }
            (flat.len() >= 4).then_some(poly)
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
    // bodydetails' `{x, y, z}`, or `[x, y, z]` (live's compact fetch).
    let xyz = |v: &Value| Some([v["x"].as_f64()?, v["y"].as_f64()?, v["z"].as_f64()?]);
    let v3 = |v: &Value| xyz(v).or_else(|| f3(v));
    // A type as `{ "type": … }` or the string alone.
    let kind = |v: &Value| v["type"].as_str().or(v.as_str()).map(str::to_string);
    let mut out = HashMap::new();
    let body_ids = |l: &Value| -> Vec<String> { l["bodies"].as_array().into_iter().flatten().filter_map(|b| b["id"].as_str().map(str::to_string)).collect() };
    let held: HashMap<u64, Vec<String>> = lines.iter().filter_map(|l| Some((l["index"].as_u64()?, body_ids(l)))).collect();
    for line in lines {
        let (Some(k), Some(bodies)) = (line["index"].as_u64(), line["bodies"].as_array()) else { continue };
        let at = k as usize + 1;
        let before = k.checked_sub(1).and_then(|j| held.get(&j));
        for b in bodies {
            let part_id = b["id"].as_str().filter(|_| b["type"] != "SKETCH").map(str::to_string);
            // A sketch's line: the bodies it made are its own (not the parts
            // before it), their faces its regions.
            let sketch_body = line["type"] == "newSketch" && b["type"] != "SOLID" && b["id"].as_str().is_none_or(|id| before.is_none_or(|ids| !ids.iter().any(|x| x == id)));
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for f in b["faces"].as_array().into_iter().flatten() {
                let (Some(mn), Some(mx)) = (v3(&f["box"]["minCorner"]).or_else(|| v3(&f["box"]["min"])), v3(&f["box"]["maxCorner"]).or_else(|| v3(&f["box"]["max"]))) else { continue };
                for i in 0..3 {
                    lo[i] = lo[i].min(mn[i]);
                    hi[i] = hi[i].max(mx[i]);
                }
                let c = [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0];
                let s = &f["surface"];
                let surface = kind(s);
                let kind = if sketch_body { "SKETCH_REGION" } else { "FACE" };
                let mut r = LiveRef { kind: kind.into(), point: Some(c), measure: f["area"].as_f64(), geometry: our_type(surface.as_deref()), bbox: Some([mn, mx]), body: part_id.clone().filter(|_| !sketch_body), ..Default::default() };
                // The compact fetch: origin and the outward normal beside the type.
                if surface.as_deref() == Some("PLANE")
                    && s.is_string()
                    && let (Some(o), Some(n)) = (v3(&f["origin"]), v3(&f["normal"]))
                {
                    let d: f64 = (0..3).map(|i| (c[i] - o[i]) * n[i]).sum();
                    r.point = Some([c[0] - d * n[0], c[1] - d * n[1], c[2] - d * n[2]]);
                    r.normal = Some(n);
                } else if surface.as_deref() == Some("PLANE")
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
                if let (Some(id), Some(p)) = (e["id"].as_str(), v3(&e["geometry"]["midPoint"]).or_else(|| v3(&e["midPoint"]))) {
                    let length = e["geometry"]["length"].as_f64().or(e["length"].as_f64());
                    out.insert((at, id.to_string()), LiveRef { kind: "EDGE".into(), point: Some(p), measure: length, geometry: our_type(kind(&e["curve"]).as_deref()), body: part_id.clone().filter(|_| !sketch_body), ..Default::default() });
                }
            }
            for v in b["vertices"].as_array().into_iter().flatten() {
                if let (Some(id), Some(p)) = (v["id"].as_str(), v3(&v["point"])) {
                    out.insert((at, id.to_string()), LiveRef { kind: "VERTEX".into(), point: Some(p), body: part_id.clone().filter(|_| !sketch_body), ..Default::default() });
                }
            }
            // Its box from its faces', else (bodies listed by id alone) the
            // line's part of that id.
            let part = line["parts"].as_array().into_iter().flatten().find(|p| p["partId"] == b["id"]);
            let part_box = part.and_then(|p| f3(&p["bbox"]["min"]).zip(f3(&p["bbox"]["max"])));
            let (lo, hi) = if lo[0].is_finite() { (lo, hi) } else { part_box.unwrap_or((lo, hi)) };
            if let Some(id) = b["id"].as_str()
                && lo[0].is_finite()
            {
                // Its kind in query_parts' words (c2-helical-sweep's helix: a wire in the box of the part it winds round).
                let kind = match b["type"].as_str() {
                    Some("SOLID") => Some("Solid"),
                    Some("WIRE") => Some("Wire"),
                    Some("SHEET") => Some("Surface"),
                    _ => None,
                };
                out.insert((at, id.to_string()), LiveRef { kind: "BODY".into(), point: Some([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0]), bbox: Some([lo, hi]), geometry: kind.map(str::to_string), ..Default::default() });
            }
            // A sketch's wire body and regions (live's FeatureScript fetch).
            if b["type"] == "SKETCH" {
                for key in ["faces", "edges", "vertices"] {
                    for e in b[key].as_array().into_iter().flatten() {
                        if let Some(r) = e["id"].as_str().and_then(|id| out.get_mut(&(at, id.to_string()))) {
                            r.sketch = true;
                        }
                    }
                }
            }
        }
    }
    out
}

/// Live's sketch bodies at a studio's final state (`<element>.sketches.json`: `sketches`, each a sketch feature's
/// `featureId` and its SKETCH `bodies`, faces / edges / vertices as bodydetails') as references from each sketch's
/// feature on: its faces the sketch's regions.
pub fn refs_from_sketches(file: &Value, features: &[Value]) -> HashMap<(usize, String), LiveRef> {
    let mut out = HashMap::new();
    for s in file["sketches"].as_array().into_iter().flatten() {
        let Some(k) = features.iter().position(|f| f["featureId"] == s["featureId"]) else { continue };
        // (Its plane normals are evPlane's, the faces' own: as bodydetails' with `orientation`.)
        let mut bodies = s["bodies"].clone();
        for f in bodies.as_array_mut().into_iter().flatten().flat_map(|b| b["faces"].as_array_mut().into_iter().flatten()) {
            f["orientation"] = json!(true);
        }
        let line = json!({ "index": k, "type": "newSketch", "bodies": bodies });
        let of = s["featureId"].as_str().map(str::to_string);
        out.extend(refs_from_bodies(std::slice::from_ref(&line)).into_iter().map(|(key, mut r)| {
            r.sketch_file = true;
            r.sketch_of = of.clone();
            (key, r)
        }));
    }
    out
}

/// A box in a sketch plane: (min, max).
type Box2 = ([f64; 2], [f64; 2]);

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
            // (The legacy `style` goes either way: our dialog has only styleV2.)
            if let Some(v) = done("style", params)
                && !params.contains_key("styleV2")
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
        // Sweeps before Profile control: a flag (std sweep.fs derives it from profileControl today).
        "sweep" => {
            if let Some(v) = done("keepProfileOrientation", params) {
                params.entry("profileControl").or_insert(json!(if v == true { "KEEP_ORIENTATION" } else { "NONE" }));
            }
        }
        // Helices before the axis type: one helixType for both (std helix.fs getHelixType, inverted).
        "helix" => {
            if let Some(t) = done("helixType", params) {
                let (axis, path) = match t.as_str().unwrap_or("TURNS") {
                    "PITCH" => ("SURFACE", "PITCH"),
                    "HEIGHT_TURNS" => ("CIRCLE", "TURNS"),
                    "HEIGHT_PITCH" => ("CIRCLE", "PITCH"),
                    "TURNS_PITCH" => ("CIRCLE", "TURNS_PITCH"),
                    _ => ("SURFACE", "TURNS"),
                };
                params.entry("axisType").or_insert(json!(axis));
                params.entry("pathType").or_insert(json!(path));
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
    /// Live's exact volume (evVolume at HIGH accuracy) where massproperties' band misses it (live 2058: the drive
    /// train's 62- and 50-tooth gears).
    pub volume_high: Option<f64>,
    pub area_band: Option<[f64; 2]>,
    pub bbox: Option<[[f64; 3]; 2]>,
    /// Live's box is the web client's body bounds (its UI fetch), loose
    /// around fillets' surfaces: ours only has to lie inside it.
    pub bbox_outer: bool,
    pub centroid: Option<[f64; 3]>,
    /// Live's centroid band ([min, max] per axis), when given.
    pub centroid_band: Option<[[f64; 3]; 2]>,
    /// Faces and edges by geometry (plane, cylinder…; line, circle…; the
    /// rest "other"), when known (live: its bodydetails).
    pub face_types: Option<std::collections::BTreeMap<String, usize>>,
    pub edge_types: Option<std::collections::BTreeMap<String, usize>>,
    /// Ours: the area of the part's faces on other surfaces (blends' procedural surfaces, B-splines), m², whose area
    /// live measures up to 0.22 % low (docs/PARITY-SUITES.md, "Area of procedural blend faces").
    pub freeform_area: Option<f64>,
}

/// How low live measures the area of procedural (OTHER) blend faces: c2-extension-spring's Curvature rounds read
/// 0.8113 / 2.4341 mm² in live where the exact arc length of live's own section (its G11 poles) gives 0.8131 / 2.4393.
pub const LIVE_FREEFORM_AREA_SHORT: f64 = 0.0022;

/// A face's or edge's geometry as the parity suite counts it (Onshape's
/// type names or ours): plane, cylinder, cone, sphere, torus; line, circle,
/// ellipse; else other.
fn geometry_kind(t: &str) -> String {
    let t = t.to_lowercase();
    if ["plane", "cylinder", "cone", "sphere", "torus", "line", "circle", "ellipse"].contains(&t.as_str()) { t } else { "other".into() }
}

/// Whether two sketch curves (sketch_get entities) cross or touch, within
/// `tol`: lines, circles and arcs (an arc as its circle); others may.
/// A text region query's face, by its IMPRINT edge: the live sketch entity the edge is derived from, the vertex that
/// bounds its piece (the two live entities meeting there and its orientation), and the face's side of the edge.
struct EdgeSide {
    edge: String,
    vertex: Option<([String; 2], f64)>,
    side: f64,
}

fn edge_side_query(text: &str) -> Option<EdgeSide> {
    let edge = face_edge_entity(text)?;
    let side = text.rfind("]])]").and_then(|end| text[..end].rsplit_once(',')).and_then(|(_, n)| n.trim().parse::<f64>().ok())?;
    let entity_of = |name: &str| -> Option<String> {
        let def = text.split(&format!("var {name}=sketchEntityQuery(")).nth(1)?;
        def.split(')').next().and_then(|args| args.rsplit('"').nth(1)).map(str::to_string)
    };
    let after = &text[text.find("\"IMPRINT\",EntityType.EDGE,{")?..];
    let vertex = after.split_once("topologyDisambiguation([[").filter(|(before, _)| !before.contains("derivedFrom")).and_then(|(_, rest)| {
        let (name, rest) = rest.split_once(',')?;
        let flag: f64 = rest.split(']').next()?.trim().parse().ok()?;
        let def = text.split(&format!("var {}=makeQuery(", name.trim())).nth(1)?;
        let pair = def.split("\"INTERSECT\",EntityType.VERTEX,{\"derivedFrom\":[").nth(1)?.split(']').next()?;
        let (a, b) = pair.split_once(',')?;
        Some(([entity_of(a.trim())?, entity_of(b.trim())?], flag))
    });
    Some(EdgeSide { edge, vertex, side })
}

/// The live sketch entity an IMPRINT edge of a text query is derived from (`"derivedFrom":subQk`, its
/// `var subQk=sketchEntityQuery(…,"<id>")`, or `"derivedFrom":sketchEntityQuery(…,"<id>")`).
fn face_edge_entity(text: &str) -> Option<String> {
    let at = text.find("\"IMPRINT\",EntityType.EDGE")?;
    let from = &text[at..][text[at..].find("\"derivedFrom\":")? + 14..];
    let last_quoted = |q: &str| q.split(')').next().and_then(|args| args.rsplit('"').nth(1)).map(str::to_string);
    if let Some(rest) = from.strip_prefix("sketchEntityQuery(") {
        return last_quoted(rest);
    }
    let name: String = from.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
    let def = text.split(&format!("var {name}=sketchEntityQuery(")).nth(1)?;
    last_quoted(def)
}

fn curves_meet(a: &Value, b: &Value, tol: f64) -> bool {
    let p = |v: &Value| Some(glam::DVec2::new(v[0].as_f64()?, v[1].as_f64()?));
    let circle = |e: &Value| -> Option<(glam::DVec2, f64)> {
        let c = p(&e["center"])?;
        Some((c, e["radius"].as_f64().or_else(|| Some(p(&e["start"])?.distance(c)))?))
    };
    let segment = |e: &Value| Some((p(&e["start"])?, p(&e["end"])?));
    let to_segment = |q: glam::DVec2, (s, t): (glam::DVec2, glam::DVec2)| {
        let d = t - s;
        let u = ((q - s).dot(d) / d.length_squared().max(1e-300)).clamp(0.0, 1.0);
        q.distance(s + d * u)
    };
    match (a["type"].as_str(), b["type"].as_str()) {
        (Some("line"), Some("line")) => {
            let (Some(x), Some(y)) = (segment(a), segment(b)) else { return true };
            let cross = |o: glam::DVec2, p: glam::DVec2, q: glam::DVec2| (p - o).perp_dot(q - o);
            let proper = cross(x.0, x.1, y.0) * cross(x.0, x.1, y.1) < 0.0 && cross(y.0, y.1, x.0) * cross(y.0, y.1, x.1) < 0.0;
            proper || [to_segment(y.0, x), to_segment(y.1, x), to_segment(x.0, y), to_segment(x.1, y)].iter().any(|d| *d <= tol)
        }
        (Some("line"), Some("circle" | "arc")) | (Some("circle" | "arc"), Some("line")) => {
            let (l, c) = if a["type"] == "line" { (a, b) } else { (b, a) };
            let (Some(seg), Some((c, r))) = (segment(l), circle(c)) else { return true };
            to_segment(c, seg) <= r + tol && seg.0.distance(c).max(seg.1.distance(c)) >= r - tol
        }
        (Some("circle" | "arc"), Some("circle" | "arc")) => {
            let (Some((c1, r1)), Some((c2, r2))) = (circle(a), circle(b)) else { return true };
            let d = c1.distance(c2);
            d <= r1 + r2 + tol && d >= (r1 - r2).abs() - tol
        }
        _ => true,
    }
}

/// Live's parts of one live.jsonl line.
pub fn live_parts(line: &Value) -> Vec<PartState> {
    let band = |v: &Value| v.as_array().and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
    // Its bodydetails body (by part id): faces and edges by geometry. Not from the UI-method fetch (its lines carry
    // `settle_ms`): it calls a face PLANE when its display triangles all face one way (c2-sm-louver's bends, skasm
    // 11-08: cylinders R 1.628 in live's exact read): not known there, not compared.
    let ui_fetch = line.get("settle_ms").is_some();
    let types = |p: &Value, list: &str, field: &str| {
        if ui_fetch {
            return None;
        }
        let body = line["bodies"].as_array()?.iter().find(|b| b["id"] == p["partId"])?;
        let mut m = std::collections::BTreeMap::new();
        for x in body[list].as_array()? {
            // `{"type": "PLANE", …}`, or the type alone in later fetches
            // ("undefined" there: not known, not compared).
            let t = x[field]["type"].as_str().or(x[field].as_str()).filter(|t| *t != "undefined")?;
            *m.entry(geometry_kind(t)).or_default() += 1;
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
            volume_high: p["volume_high"].as_f64(),
            area_band: band(&p["area_band"]),
            bbox: f3(&p["bbox"]["min"]).zip(f3(&p["bbox"]["max"])).map(|(a, b)| [a, b]),
            // The UI fetch: its parts' centroids are massproperties' exact
            // [0, 0, 0] (no material), a sheet's none (its box the control
            // points' bounds: cf-curve-pattern-normal's B-spline surface).
            bbox_outer: line["looseBoxes"].as_bool() == Some(true) || f3(&p["centroid"]).is_none_or(|c| c == [0.0; 3]),
            // massproperties gives exactly [0, 0, 0] for a part without a
            // material (live README; every part of the UI fetch): not known.
            centroid: f3(&p["centroid"]).filter(|c| *c != [0.0; 3]),
            centroid_band: p["centroid_band"].as_array().and_then(|b| Some([f3(b.first()?)?, f3(b.get(1)?)?])),
            face_types: types(p, "faces", "surface"),
            edge_types: types(p, "edges", "curve"),
            freeform_area: None,
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
    // The area of each part's faces on other surfaces.
    let mut freeform: HashMap<String, f64> = HashMap::new();
    if let Ok(e) = t.call("query_entities", json!({ "kind": "faces", "limit": 100000 })) {
        let k = per_unit(&e["units"]);
        for x in e["faces"].as_array().into_iter().flatten() {
            if geometry_kind(x["surface"].as_str().unwrap_or("")) == "other" {
                *freeform.entry(x["part"].as_str().unwrap_or("").to_string()).or_default() += x["area"].as_f64().unwrap_or(0.0) * k * k;
            }
        }
    }
    Ok(parts_from_query(&r)
        .into_iter()
        .zip(r["parts"].as_array().into_iter().flatten())
        .map(|(mut s, p)| {
            s.face_types = p["id"].as_str().map(|id| faces.get(id).cloned().unwrap_or_default());
            s.edge_types = p["id"].as_str().map(|id| edges.get(id).cloned().unwrap_or_default());
            s.freeform_area = p["id"].as_str().map(|id| freeform.get(id).copied().unwrap_or(0.0));
            s
        })
        .collect())
}

/// The parts of a `query_parts` answer, SI (faces and edges by geometry not known).
pub fn parts_from_query(r: &Value) -> Vec<PartState> {
    let k = per_unit(&r["units"]);
    let at = |v: &Value| f3(v).map(|p| p.map(|c| c * k));
    r["parts"]
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
            volume_high: None,
            area_band: None,
            bbox: at(&p["bounding_box"]["min"]).zip(at(&p["bounding_box"]["max"])).map(|(a, b)| [a, b]),
            bbox_outer: false,
            centroid: at(&p["centroid"]),
            centroid_band: None,
            face_types: None,
            edge_types: None,
            freeform_area: None,
        })
        .collect()
}

/// Parts as live reports an import (the exchange suite's live-self.json / live-ours.json, docs/PARITY-SUITES.md
/// §3): `{"parts": [{"name", "bodyType", "faces", "edges", "vertices", "volume" mm³, "volume_band" [lo, hi] mm³,
/// "area" mm², "area_band"}]}`, SI. `mesh`: a mesh body's counts aren't compared (zero).
pub fn parts_from_live_import(v: &Value, mesh: bool) -> Vec<PartState> {
    let n = |x: &Value| if mesh { 0 } else { x.as_u64().unwrap_or(0) };
    let band = |x: &Value, k: f64| x.as_array().and_then(|a| Some([a.first()?.as_f64()? * k, a.get(1)?.as_f64()? * k]));
    v["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| PartState {
            name: p["name"].as_str().unwrap_or("").to_string(),
            body_type: if mesh { String::new() } else { p["bodyType"].as_str().unwrap_or("").to_lowercase() },
            faces: n(&p["faces"]),
            edges: n(&p["edges"]),
            vertices: n(&p["vertices"]),
            volume: p["volume"].as_f64().map(|x| x * 1e-9),
            area: p["area"].as_f64().map(|x| x * 1e-6),
            volume_band: band(&p["volume_band"], 1e-9),
            volume_high: None,
            area_band: band(&p["area_band"], 1e-6),
            bbox: None,
            bbox_outer: false,
            centroid: None,
            centroid_band: None,
            face_types: None,
            edge_types: None,
            freeform_area: None,
        })
        .collect()
}

/// How ours differs from live after a feature (empty: it matches). Parts
/// are paired by volume and box; counts exact, volume and area within live's
/// band or 1e-6 relative, centroids and boxes within 1e-6 m.
/// Live's import of its own export (`own`) where it loses detail against its model (`model`, its Parasolid import):
/// part for part (by volume, within 1 %: a STEP's approximated sweep) as many faces, edges and vertices or more, some
/// more (live's STEP of cf-hose-routing splits its tubes' ring edges: 12/16/8 for the model's 12/12/0). Then what ours
/// is held to: the model's counts, the import's volume and area. Not a target to split them as live does.
pub fn model_topology(own: &[PartState], model: &[PartState]) -> Option<Vec<PartState>> {
    if own.len() != model.len() || own.is_empty() {
        return None;
    }
    let by_volume = |v: &[PartState]| {
        let mut v: Vec<PartState> = v.to_vec();
        v.sort_by(|a, b| a.volume.unwrap_or(0.0).total_cmp(&b.volume.unwrap_or(0.0)));
        v
    };
    let (mut a, b) = (by_volume(own), by_volume(model));
    let close = |x: &PartState, y: &PartState| {
        let (u, v) = (x.volume.unwrap_or(0.0), y.volume.unwrap_or(0.0));
        (u - v).abs() <= 1e-2 * v.abs()
    };
    if !a.iter().zip(&b).all(|(o, m)| close(o, m) && o.faces >= m.faces && o.edges >= m.edges && o.vertices >= m.vertices) {
        return None;
    }
    if a.iter().zip(&b).all(|(o, m)| (o.faces, o.edges, o.vertices) == (m.faces, m.edges, m.vertices)) {
        return None;
    }
    for (o, m) in a.iter_mut().zip(&b) {
        (o.faces, o.edges, o.vertices) = (m.faces, m.edges, m.vertices);
        o.face_types = m.face_types.clone();
        o.edge_types = m.edge_types.clone();
    }
    Some(a)
}

pub fn differences(live: &[PartState], ours: &[PartState]) -> Vec<String> {
    let mut out = Vec::new();
    if live.len() != ours.len() {
        out.push(format!("{} parts, live {}", ours.len(), live.len()));
    }
    let mut free: Vec<&PartState> = ours.iter().collect();
    for l in live {
        // Ours for live's: the same volume (within 1e-6: a pattern's
        // instances tie), then the nearest box.
        let d = |p: &PartState| {
            let (a, b) = (p.volume.unwrap_or(0.0), l.volume.unwrap_or(0.0));
            let volume = (a - b).abs() / b.abs().max(1e-12);
            let apart = p.bbox.zip(l.bbox).map_or(0.0, |(x, y)| (0..2).flat_map(|c| (0..3).map(move |i| (x[c][i] - y[c][i]).powi(2))).sum::<f64>().sqrt());
            (if volume < 1e-6 { 0.0 } else { volume }, apart)
        };
        let Some(i) = (0..free.len()).min_by(|a, b| d(free[*a]).partial_cmp(&d(free[*b])).unwrap_or(std::cmp::Ordering::Equal)) else {
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
            && !l.volume_high.is_some_and(|h| within(o.volume, h, None))
        {
            out.push(format!("{who}: volume {:.6} mm³, live {:.6}", o.volume.unwrap_or(f64::NAN) * 1e9, v * 1e9));
        }
        // (Widened above by how low live measures our faces on other surfaces: PARITY-SUITES.md.)
        let short = o.freeform_area.unwrap_or(0.0) * LIVE_FREEFORM_AREA_SHORT;
        if let Some(a) = l.area
            && !within(o.area, a, Some(l.area_band.unwrap_or([a, a])).map(|[lo, hi]| [lo, hi + short]))
        {
            out.push(format!("{who}: area {:.6} mm², live {:.6}", o.area.unwrap_or(f64::NAN) * 1e6, a * 1e6));
        }
        // Where live's mass properties are approximate (it gives a volume band), its centroid is as uncertain: the
        // band's relative half-width times the part's extent each way (c2-rings-loft's ring: our area is the exact
        // 136.82166 mm² of its profile, live's volume 7e-5 short, its centroid 2 µm off; band ±0.16 %).
        let centroid_tol = |j: usize| {
            let rel = l.volume_band.zip(l.volume).map_or(0.0, |([lo, hi], v)| (hi - lo) / 2.0 / v.abs().max(1e-30));
            l.bbox.map_or(1e-6, |b| (rel * (b[1][j] - b[0][j])).max(1e-6))
        };
        if let (Some(a), Some(b)) = (o.centroid, l.centroid)
            && (0..3).any(|j| (a[j] - b[j]).abs() > centroid_tol(j) && !l.centroid_band.is_some_and(|[lo, hi]| a[j] >= lo[j] - 1e-9 && a[j] <= hi[j] + 1e-9))
        {
            out.push(format!("{who}: centroid {:?} mm, live {:?}", a.map(|c| c * 1e3), b.map(|c| c * 1e3)));
        }
        // A free-form part (B-spline faces, ours or live's): live's box is its poles' and display mesh's bounds
        // (cf-surface-pattern's Thicken 1: its surface's poles 3.3 mm past its edge, the offset side 0.013-0.018 mm
        // inside ours; volume and area within 0.02 %): ours inside it, within 2e-4 of its diagonal.
        let freeform = [&o.face_types, &l.face_types].iter().any(|t| t.as_ref().is_some_and(|m| m.get("other").is_some_and(|&n| n > 0)));
        let outer = l.bbox_outer || freeform;
        let off = |a: [[f64; 3]; 2], b: [[f64; 3]; 2], j: usize| if outer { (b[0][j] - a[0][j]).max(a[1][j] - b[1][j]) } else { (a[0][j] - b[0][j]).abs().max((a[1][j] - b[1][j]).abs()) };
        let slack = |b: [[f64; 3]; 2]| if freeform { (2e-4 * (0..3).map(|j| (b[1][j] - b[0][j]).powi(2)).sum::<f64>().sqrt()).max(1e-6) } else { 1e-6 };
        if let (Some(a), Some(b)) = (o.bbox, l.bbox)
            && (0..3).any(|j| off(a, b, j) > slack(b))
        {
            out.push(format!("{who}: box {:?} mm, live {:?}", a.map(|p| p.map(|c| c * 1e3)), b.map(|p| p.map(|c| c * 1e3))));
        }
    }
    // Ours that live doesn't have: which they are.
    for o in free {
        let name = if o.name.is_empty() { "a part" } else { &o.name };
        out.push(format!("not in live: \"{name}\" ({} body, {}/{}/{} faces/edges/vertices, volume {:.6} mm³)", o.body_type, o.faces, o.edges, o.vertices, o.volume.unwrap_or(f64::NAN) * 1e9));
    }
    out
}

/// One assembly rebuilt from live's files (docs/PARITY-SUITES.md §1,
/// assembly phase; layout of REQUESTS 1842): its Part Studios translated into
/// tabs, the instances inserted, fixed and placed where live has them, the
/// mates added between the same connectors; then each occurrence and mate
/// compared with live.
#[derive(Debug, Default)]
pub struct AssemblyResult {
    pub occurrences: usize,
    pub occurrences_matching: usize,
    pub mates: usize,
    pub mates_matching: usize,
    /// Differences, first one first.
    pub differences: Vec<String>,
    /// What wasn't translated, with the reason.
    pub untranslated: Vec<String>,
}

/// Onshape's mate types in assembly_mate's words.
fn mate_type(t: &str) -> Option<&'static str> {
    Some(match t {
        "FASTENED" => "fastened",
        "REVOLUTE" => "revolute",
        "SLIDER" => "slider",
        "CYLINDRICAL" => "cylindrical",
        "PIN_SLOT" => "pinslot",
        "PLANAR" => "planar",
        "BALL" => "ball",
        "PARALLEL" => "parallel",
        "TANGENT" => "tangent",
        "WIDTH" => "width",
        _ => return None,
    })
}

/// Onshape's EntityInferenceType in our words.
fn inference(t: &str) -> Option<&'static str> {
    Some(match t {
        "CENTROID" => "centroid",
        "CENTER" => "center",
        "MID_POINT" => "midpoint",
        "POINT" => "vertex",
        "TOP_AXIS_POINT" => "top_axis",
        "MID_AXIS_POINT" => "mid_axis",
        "BOTTOM_AXIS_POINT" => "bottom_axis",
        "PART_ORIGIN" => "origin",
        _ => return None,
    })
}

/// A row-major 4×4 transform (metres) as translation (m) and unit quaternion [x, y, z, w].
fn transform_parts(m: &[f64]) -> Option<([f64; 3], [f64; 4])> {
    if m.len() < 12 {
        return None;
    }
    let rot = glam::DMat3::from_cols(glam::DVec3::new(m[0], m[4], m[8]), glam::DVec3::new(m[1], m[5], m[9]), glam::DVec3::new(m[2], m[6], m[10]));
    let q = glam::DQuat::from_mat3(&rot);
    Some(([m[3], m[7], m[11]], [q.x, q.y, q.z, q.w]))
}

/// One assembly level of live's definition: the root ("") or a subassembly (its element id).
struct Level<'a> {
    element: String,
    instances: Vec<&'a Value>,
    /// Live's /features body of this level (its mates with their connectors).
    features: Vec<&'a Value>,
    /// The definition's features of this level (each mate's `featureData`).
    defs: Vec<&'a Value>,
    /// Our tab for it.
    tab: String,
}

/// A row-major 4×4 transform (metres) as a matrix.
fn mat(m: &[f64]) -> Option<glam::DMat4> {
    (m.len() >= 16).then(|| glam::DMat4::from_cols_array(&[m[0], m[4], m[8], m[12], m[1], m[5], m[9], m[13], m[2], m[6], m[10], m[14], m[3], m[7], m[11], m[15]]))
}

fn row_major(m: glam::DMat4) -> Vec<f64> {
    let c = m.to_cols_array();
    vec![c[0], c[4], c[8], c[12], c[1], c[5], c[9], c[13], c[2], c[6], c[10], c[14], c[3], c[7], c[11], c[15]]
}

fn values(v: &Value) -> Vec<&Value> {
    v.as_array().into_iter().flatten().collect()
}

fn ids(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect()
}

/// `asm`: live's assembly.json (the assembly definition with "features", and "subAssemblyFeatures": element id →
/// that subassembly's features body); `studios`: element id → (features.json of that Part Studio, its parts list).
/// An instance's configuration as Onshape encodes a standard-content-like one (base-64 of
/// "%40scp=…;Pitch=7.9248E-4+meter;…"): its variables (name, value, unit), in order.
fn instance_config_values(i: &Value) -> Vec<(String, f64, String)> {
    let Some(enc) = i["configuration"].as_str().filter(|c| *c != "default") else { return Vec::new() };
    const B: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in enc.chars().map(|c| match c { '-' => '+', '_' => '/', c => c }) {
        let Some(v) = B.find(c) else { break };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    let Ok(text) = String::from_utf8(bytes) else { return Vec::new() };
    let mut decoded = Vec::new();
    let b = text.as_bytes();
    let mut k = 0;
    while k < b.len() {
        if b[k] == b'%' && k + 2 < b.len() && let Ok(v) = u8::from_str_radix(&text[k + 1..k + 3], 16) {
            decoded.push(v);
            k += 3;
        } else {
            decoded.push(b[k]);
            k += 1;
        }
    }
    let text = String::from_utf8_lossy(&decoded).to_string();
    text.split(';')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            if k.starts_with('@') {
                return None;
            }
            let (num, unit) = v.split_once('+').unwrap_or((v, ""));
            Some((k.to_string(), num.parse().ok()?, unit.to_string()))
        })
        .collect()
}

/// The standard content item live names `name` ("Hex socket head cap screw M3x0.50 x 8", without " <n>"): the first
/// catalogue component, size and length whose part name it is (for an instance whose Part Studio live didn't fetch).
pub fn standard_spec_by_part_name(name: &str) -> Option<noshape_model::assembly::standard::StandardSpec> {
    use noshape_model::assembly::standard::{CATALOGUE, StandardSpec};
    for c in CATALOGUE.iter() {
        for size in c.size_names() {
            let Some(row) = c.size(size) else { continue };
            // The size's label up to its length ("… M3x0.50", "… {l} x M5x0.8") starts the name.
            let head = row.label.split(" x {l}").next().unwrap_or_default().split("{l}").next().unwrap_or_default();
            if !name.starts_with(head.split('{').next().unwrap_or_default()) {
                continue;
            }
            let mut spec = StandardSpec::new(c);
            spec.set_size(size);
            if spec.part_name() == name {
                return Some(spec);
            }
            let lengths = spec.size_row().map(|s| c.lengths(s)).unwrap_or_default();
            for l in lengths {
                spec.length = Some(l);
                if spec.part_name() == name {
                    return Some(spec);
                }
            }
        }
    }
    None
}

/// A part's volume band (m³, lo and hi) in an assembly corpus' parts list: `volume` [lo, hi] from `.mass.json`, else
/// `volume_band` of live's per-feature lines (where `volume` is the number).
fn volume_band(p: &Value) -> Option<(f64, f64)> {
    let b = p["volume"].as_array().filter(|v| v.len() == 2).or_else(|| p["volume_band"].as_array())?;
    Some((b.first()?.as_f64()?, b.get(1)?.as_f64()?))
}

/// An Onshape deterministic id as the number its base-64 characters write (ids made later are larger).
/// One region named by id alone, without live's geometry: the sketch's only region inside no other's hole (a plate
/// around its holes, a ring around its bore).
fn outer_region(g: &Value, ids: usize) -> Option<Vec<String>> {
    let regions: Vec<&Value> = g["regions"].as_array()?.iter().filter(|r| r["imprinted"] != true).collect();
    if ids != 1 || regions.len() < 2 {
        return None;
    }
    let list = |r: &Value, k: &str| -> Vec<String> { r[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
    let inner = |r: &Value| {
        let mine = list(r, "outer");
        regions.iter().any(|o| {
            let theirs = list(o, "outer");
            let holes: Vec<String> = list(o, "boundary").into_iter().filter(|c| !theirs.contains(c)).collect();
            o["id"] != r["id"] && !mine.is_empty() && mine.iter().all(|c| holes.contains(c))
        })
    };
    match regions.iter().filter(|r| !inner(r)).collect::<Vec<_>>().as_slice() {
        [only] => Some(vec![only["id"].as_str()?.to_string()]),
        _ => None,
    }
}

/// The creation counter in an Onshape deterministic id of the forms that count up as a Part Studio is built:
/// `J` + counter + type (JGC, JHD … J/D), then `R` + counter + `B` + type (RBBC, RjBD). Other forms (`K…`, an
/// entity's sub-ids) don't follow it: None.
fn onshape_counter(id: &str) -> Option<u32> {
    const B64: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let c: Vec<char> = id.chars().collect();
    let index = |ch: char| B64.find(ch).map(|i| i as u32);
    match c.as_slice() {
        ['J', n, _] => index(*n),
        ['R', n, 'B', _] => index(*n).map(|i| 64 + i),
        _ => None,
    }
}

fn onshape_id_value(id: &str) -> u128 {
    const B: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    id.chars().fold(0u128, |n, c| n.saturating_mul(64).saturating_add(B.find(c).unwrap_or(0) as u128))
}

pub fn rebuild_assembly(ctx: &mut Ctx, path: &Path, asm: &Value, studios: &[(String, Value, Value)]) -> AssemblyResult {
    // NOSHAPE_PARITY_TRACE: each step on stderr with the seconds since the start (to find a hang).
    let started = std::time::Instant::now();
    let tracing = std::env::var_os("NOSHAPE_PARITY_TRACE").is_some();
    let trace = |what: &str| {
        if tracing {
            eprintln!("[{:8.2}s] {what}", started.elapsed().as_secs_f64());
        }
    };
    let mut res = AssemblyResult::default();
    let call = |ctx: &mut Ctx, tool: &str, args: Value| -> Result<Value, String> {
        let op = registry::by_tool_name(tool).ok_or_else(|| format!("no tool {tool}"))?;
        registry::run(ctx, op, &args).map(|o| o.value).map_err(|e| e.to_json().to_string())
    };
    let _ = std::fs::remove_file(path);
    if let Err(e) = call(ctx, "document_create", json!({ "path": path.to_str().unwrap_or("") })) {
        res.untranslated.push(format!("document: {e}"));
        return res;
    }
    // Bare numbers are in the document's units: the first Part Studio's.
    let (length, angle) = studios.first().map(|(_, s, _)| doc_units(&s["units"])).unwrap_or_default();
    if length.is_some() || angle.is_some() {
        let mut a = json!({});
        if let Some(l) = length {
            a["length_unit"] = json!(l);
        }
        if let Some(g) = angle {
            a["angle_unit"] = json!(g);
        }
        if let Err(e) = call(ctx, "document_set_units", a) {
            res.untranslated.push(format!("units: {e}"));
        }
    }
    let root = &asm["rootAssembly"];
    // The levels: the root, then each subassembly once (by element id).
    let mut levels: Vec<Level> = vec![Level { element: String::new(), instances: values(&root["instances"]), features: values(&asm["features"]["features"]), defs: values(&root["features"]), tab: "Assembly 1".into() }];
    for sub in values(&asm["subAssemblies"]) {
        let Some(e) = sub["elementId"].as_str() else { continue };
        if levels.iter().any(|l| l.element == e) {
            continue;
        }
        let features = values(&asm["subAssemblyFeatures"][e]["features"]);
        levels.push(Level { element: e.to_string(), instances: values(&sub["instances"]), features, defs: values(&sub["features"]), tab: format!("Sub {}", &e[..e.len().min(6)]) });
    }
    // An instance's element (its Part Studio, or its subassembly) at any level.
    let mut element_of: HashMap<String, String> = HashMap::new();
    for l in &levels {
        for i in &l.instances {
            if let (Some(id), Some(e)) = (i["id"].as_str(), i["elementId"].as_str()) {
                element_of.insert(id.to_string(), e.to_string());
            }
        }
    }
    // Mate connectors (implicit ones live in their mate's `mateConnectors`): (level, feature id) → (occurrence path in
    // that level, inference, the entity query).
    type Key = (String, String);
    let mut connectors: HashMap<Key, (Vec<String>, String, Value)> = HashMap::new();
    let mut connector_origins: HashMap<Key, [f64; 3]> = HashMap::new();
    for l in &levels {
        for f in &l.features {
            let all = std::iter::once(*f).chain(f["mateConnectors"].as_array().into_iter().flatten());
            for mc in all.filter(|m| m["btType"].as_str().is_some_and(|t| t.starts_with("BTMMateConnector"))) {
                let origin = mc["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == "originQuery");
                let Some(q) = origin.and_then(|p| p["queries"].as_array()).and_then(|q| q.first()) else { continue };
                let query = json!({ "queryString": q["entityQuery"], "deterministicIds": q["deterministicIds"] });
                if let Some(id) = mc["featureId"].as_str() {
                    connectors.insert((l.element.clone(), id.to_string()), (ids(&q["path"]), q["inferenceType"].as_str().unwrap_or("").to_string(), query));
                }
            }
        }
        // Live's frame of each mate's connectors (part coordinates), in the mate's pick order: where a connector is
        // when its query isn't read.
        for m in &l.defs {
            let Some(f) = l.features.iter().find(|f| f["featureId"] == m["id"]) else { continue };
            let picks = f["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == "mateConnectorsQuery").and_then(|p| p["queries"].as_array());
            for (q, e) in picks.into_iter().flatten().zip(m["featureData"]["matedEntities"].as_array().into_iter().flatten()) {
                if let (Some(id), Some(o)) = (q["featureId"].as_str(), f3(&e["mateConnectorCS"]["origin"])) {
                    connector_origins.insert((l.element.clone(), id.to_string()), o);
                }
            }
        }
    }
    // Each Part Studio into its own tab; the connectors' entities resolved on the part at their path's end.
    let mut tabs: HashMap<String, String> = HashMap::new();
    let mut part_names: HashMap<(String, String), String> = HashMap::new();
    let mut entities: HashMap<Key, String> = HashMap::new();
    // A studio whose sketch another's Frame takes as its profile first.
    let sources: HashSet<String> = studios
        .iter()
        .flat_map(|(_, s, _)| s["features"].as_array().into_iter().flatten())
        .flat_map(|f| f["parameters"].as_array().into_iter().flatten())
        .filter(|p| p["parameterId"] == "profileSketch")
        .filter_map(|p| p["namespace"].as_str()?.split("::").find_map(|x| x.strip_prefix('e')).map(str::to_string))
        .collect();
    let mut ordered: Vec<&(String, Value, Value)> = studios.iter().collect();
    ordered.sort_by_key(|(e, _, _)| !sources.contains(e));
    let mut built: HashMap<String, (String, HashMap<String, String>)> = HashMap::new();
    let defaults: HashMap<String, HashMap<String, Value>> = studios
        .iter()
        .filter(|(e, _, _)| sources.contains(e))
        .map(|(e, s, _)| (e.clone(), s["configuration"]["configurationParameters"].as_array().into_iter().flatten().filter_map(|c| Some((c["parameterId"].as_str()?.to_string(), c["defaultValue"].clone()))).collect()))
        .collect();
    for (element, studio, parts) in ordered {
        let name = format!("Studio {}", &element[..element.len().min(6)]);
        // The studio's part ids in creation order (see Translator::part_order).
        let mut studio = studio.clone();
        // A studio fetched without its configuration whose instances carry variables (standard content's #Pitch…):
        // its configuration variables from the first instance's values (asm 11-07).
        let first_config = levels.iter().flat_map(|l| l.instances.iter()).filter(|i| i["elementId"] == element.as_str()).map(|i| instance_config_values(i)).find(|v| !v.is_empty());
        if studio["configuration"].is_null()
            && let Some(vars) = first_config
        {
            let params: Vec<Value> = vars
                .iter()
                .map(|(name, v, unit)| {
                    let (kind, units) = match unit.as_str() {
                        "meter" => ("LENGTH", "meter"),
                        "radian" => ("ANGLE", "radian"),
                        _ => ("REAL", ""),
                    };
                    json!({ "btType": "BTMConfigurationParameterQuantity-1826", "parameterId": name, "quantityType": kind, "rangeAndDefault": { "units": units, "defaultValue": v } })
                })
                .collect();
            studio["configuration"] = json!({ "configurationParameters": params });
        }
        if studio["partOrder"].is_null() {
            let mut order: Vec<String> = parts.as_array().into_iter().flatten().filter_map(|p| p["partId"].as_str().map(str::to_string)).collect();
            order.sort_by_key(|id| onshape_id_value(id));
            order.dedup();
            studio["partOrder"] = json!(order);
        }
        if studio["partVolumes"].is_null() {
            let bands: Vec<Value> = parts.as_array().into_iter().flatten().filter_map(|p| volume_band(p).map(|(lo, hi)| json!([lo, hi]))).collect();
            studio["partVolumes"] = json!(bands);
        }
        let studio = &studio;
        // Live's per-feature geometry (the studio's `live` lines, `<element>.live.jsonl`) for its id-only queries.
        let mut refs = studio["live"].as_array().map(|lines| refs_from_bodies(lines)).unwrap_or_default();
        // Its sketches' bodies at the final state (live 2267, `<element>.sketches.json`): their regions, edges and
        // points, under live's own lines.
        let features = studio["features"].as_array().map(Vec::as_slice).unwrap_or_default();
        for (key, r) in refs_from_sketches(&studio["sketchBodies"], features) {
            refs.entry(key).or_insert(r);
        }
        let mut t = match Translator::open_in(ctx, &name, studio, refs) {
            Ok(t) => t,
            Err(e) => {
                res.untranslated.push(format!("Part Studio {element}: {e}"));
                continue;
            }
        };
        t.other_studios = built.clone();
        t.other_defaults = defaults.clone();
        for k in 0..t.len() {
            trace(&format!("{name}: feature {k}"));
            let step = t.step(k);
            res.untranslated.extend(step.problems.iter().map(|p| format!("{name} › {} ({}): {p}", step.name, step.feature_type)));
            if step.status.as_deref().is_some_and(|s| s != "Ok") {
                res.differences.push(format!("{name} › {}: {} ({})", step.name, step.status.as_deref().unwrap_or(""), step.message.as_deref().unwrap_or("")));
            }
        }
        for (key, (path, inference, q)) in &connectors {
            if path.last().and_then(|i| element_of.get(i)) != Some(element) {
                continue;
            }
            trace(&format!("{name}: mate connector {}", key.1));
            let found = match t.resolve(q) {
                Ok(ids) if !ids.is_empty() => Ok(ids[0].clone()),
                Ok(_) => Err("its entity resolved to nothing".to_string()),
                Err(e) => Err(e),
            };
            let found = match (found, connector_origins.get(key)) {
                (Err(_), Some(o)) => t.connector_entity(inference, *o),
                (found, _) => found,
            };
            match found {
                Ok(e) => {
                    entities.insert(key.clone(), e);
                }
                Err(e) => res.untranslated.push(format!("mate connector {}: {e}", key.1)),
            }
        }
        // (An Import's Part Studio is a tab of its own.)
        if sources.contains(element)
            && let Some(tab) = &t.tab
        {
            built.insert(element.clone(), (tab.clone(), t.feature_ids.clone()));
        }
        tabs.insert(element.clone(), t.tab.clone().unwrap_or(name));
        // Our parts (name, volume m³): live's part names come from its Part Studio's properties, which translation
        // doesn't carry ("Housing" here is our "Part 1"); such a part is ours by its only part or its volume band.
        let solids: Vec<(String, f64, String)> = t
            .call("query_parts", json!({}))
            .map(|q| {
                let k = per_unit(&q["units"]);
                q["parts"].as_array().into_iter().flatten().filter(|p| p["kind"] == "Solid").filter_map(|p| Some((p["name"].as_str()?.to_string(), p["volume"].as_f64().unwrap_or(0.0) * k * k * k, p["id"].as_str()?.to_string()))).collect()
            })
            .unwrap_or_default();
        let ours_parts: Vec<(String, f64)> = solids.iter().map(|(n, v, _)| (n.clone(), *v)).collect();
        let our_ids: Vec<String> = solids.iter().map(|(_, _, id)| id.clone()).collect();
        // Parts of the same volume are taken one each (alike parts place alike), then shared. Those whose volume
        // fits one of ours first; the others then by the feature that made them (ids in creation order), not taking
        // a part another fits (asm-clamp-pattern's Pin, ours off in volume, took the Spacer's part by its creator).
        let fits_of = |p: &Value| -> Vec<String> {
            volume_band(p).map(|(lo, hi)| ours_parts.iter().filter(|(_, v)| *v >= lo * (1.0 - 1e-6) && *v <= hi * (1.0 + 1e-6)).map(|(n, _)| n.clone()).collect()).unwrap_or_default()
        };
        let listed: Vec<&Value> = parts.as_array().into_iter().flatten().filter(|p| p["partId"].is_string() && p["name"].is_string()).collect();
        let fitting: Vec<String> = listed.iter().flat_map(|p| fits_of(p)).collect();
        let mut taken: Vec<String> = Vec::new();
        // By the list's entries (a configured studio lists a part id once per configuration), a later one winning.
        let mut chosen: Vec<Option<String>> = vec![None; listed.len()];
        for pass in [true, false] {
            for (ix, p) in listed.iter().enumerate() {
                let (pid, pname) = (p["partId"].as_str().unwrap_or_default(), p["name"].as_str().unwrap_or_default());
                if chosen[ix].is_some() {
                    continue;
                }
                let fits = fits_of(p);
                // By its name unless its volume fits other parts of ours and not that one (default names shift when
                // ours has a part fewer: asm-welding-config's "Part 7" is our Part 6).
                let by_name = ours_parts.iter().any(|(n, _)| n == pname) && (fits.is_empty() || fits.iter().any(|n| n == pname));
                let ours = if by_name || ours_parts.is_empty() {
                    pname.to_string()
                } else if let [(only, _)] = ours_parts.as_slice() {
                    only.clone()
                } else if let Some(n) = fits.iter().find(|n| !taken.contains(n)).or(fits.first()) {
                    n.clone()
                } else if pass {
                    continue;
                } else {
                    let by_creator = t.part_by_creator(t.len(), pid, &our_ids).and_then(|id| solids.iter().find(|s| s.2 == id).map(|s| s.0.clone()));
                    match by_creator {
                        Some(n) if fitting.contains(&n) => ours_parts.iter().map(|(n, _)| n).find(|n| !taken.contains(n) && !fitting.contains(n)).cloned().unwrap_or(n),
                        Some(n) => n,
                        None => pname.to_string(),
                    }
                };
                taken.push(ours.clone());
                trace(&format!("{element}: live part {pid} \"{pname}\" -> ours {ours} (volume fits {fits:?})"));
                chosen[ix] = Some(ours);
            }
        }
        for (p, ours) in listed.iter().zip(chosen) {
            if let (Some(pid), Some(ours)) = (p["partId"].as_str(), ours) {
                part_names.insert((element.clone(), pid.to_string()), ours);
            }
        }
    }
    let occurrences: Vec<&Value> = values(&root["occurrences"]);
    let transform_of = |path: &[String]| occurrences.iter().find(|o| ids(&o["path"]) == path).and_then(|o| o["transform"].as_array()).map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>());
    trace("placing");
    let mut placer = match Translator::new(ctx, &json!({ "features": [] }), HashMap::new()) {
        Ok(t) => t,
        Err(e) => {
            res.untranslated.push(format!("placement: {e}"));
            return res;
        }
    };
    // Subassemblies before the levels that insert them: each an Assembly tab, its instances placed where live has
    // them inside it, its mates added.
    let mut ours: HashMap<Key, String> = HashMap::new();
    let mut made: HashSet<String> = HashSet::new();
    let mut order: Vec<usize> = Vec::new();
    while order.len() + 1 < levels.len() {
        // (A subassembly live didn't send waits for nothing: its instances are reported as not translated.)
        let sent = |e: &str| levels.iter().skip(1).any(|l| l.element == e);
        let next = (1..levels.len()).find(|k| !order.contains(k) && levels[*k].instances.iter().all(|i| i["type"] != "Assembly" || i["elementId"].as_str().is_some_and(|e| made.contains(e) || !sent(e))));
        let Some(k) = next else {
            res.untranslated.push("subassemblies: a cycle, or one not in subAssemblies".into());
            break;
        };
        made.insert(levels[k].element.clone());
        order.push(k);
    }
    order.push(0);
    let mut mate_names: Vec<(String, String)> = Vec::new();
    for &k in &order {
        let l = &levels[k];
        if k != 0
            && let Err(e) = placer.call("tab_create", json!({ "kind": "assembly", "name": l.tab }))
        {
            res.untranslated.push(format!("subassembly {}: {e}", l.element));
            continue;
        }
        // The instances, in live's order; ours named as inserted.
        for i in &l.instances {
            let (Some(id), Some(element)) = (i["id"].as_str(), i["elementId"].as_str()) else { continue };
            let source = if i["type"] == "Assembly" { levels.iter().find(|x| x.element == element).map(|x| x.tab.clone()) } else { tabs.get(element).cloned() };
            let Some(source) = source else {
                // Standard content live didn't fetch: ours from the catalogue by its part name.
                let part = i["name"].as_str().and_then(|n| n.rsplit_once(" <")).map(|(n, _)| n);
                if i["isStandardContent"] == true
                    && let Some(spec) = part.and_then(standard_spec_by_part_name)
                {
                    trace(&format!("insert standard {}", i["name"]));
                    let mut args = json!({ "tab": l.tab, "standard": spec.standard, "category": spec.category, "class": spec.class, "component": spec.component, "size": spec.size });
                    if let Some(len) = &spec.length {
                        args["length"] = json!(len);
                    }
                    match placer.call("assembly_insert_standard", args) {
                        Ok(r) => {
                            let inserted = r["inserted"][0].as_str().unwrap_or_default();
                            if let Some(n) = r["instances"].as_array().into_iter().flatten().find(|x| x["id"] == inserted).and_then(|x| x["name"].as_str()) {
                                ours.insert((l.element.clone(), id.to_string()), n.to_string());
                            }
                        }
                        Err(e) => res.untranslated.push(format!("instance {}: {e}", i["name"])),
                    }
                    continue;
                }
                res.untranslated.push(format!("instance {}: its {} {element} wasn't translated", i["name"], if i["type"] == "Assembly" { "subassembly" } else { "Part Studio" }));
                continue;
            };
            trace(&format!("insert {}", i["name"]));
            let mut args = json!({ "tab": l.tab, "source": source });
            // Its configuration variables (see the studio's synthesized configuration).
            let vars = instance_config_values(i);
            if i["type"] == "Part" && !vars.is_empty() {
                let unit = |u: &str| match u {
                    "meter" => " m",
                    "radian" => " rad",
                    _ => "",
                };
                args["configuration"] = Value::Object(vars.iter().map(|(n, v, u)| (n.clone(), json!(format!("{v}{}", unit(u))))).collect());
            }
            if i["type"] == "Part" {
                match i["partId"].as_str().and_then(|p| part_names.get(&(element.to_string(), p.to_string()))) {
                    Some(n) => args["part"] = json!(n),
                    None => {
                        res.untranslated.push(format!("instance {}: part {} not in its Part Studio's parts list", i["name"], i["partId"]));
                        continue;
                    }
                }
            }
            match placer.call("assembly_insert", args) {
                Ok(r) => {
                    if let Some(n) = r["name"].as_str() {
                        ours.insert((l.element.clone(), id.to_string()), n.to_string());
                    }
                }
                Err(e) => res.untranslated.push(format!("instance {}: {e}", i["name"])),
            }
        }
        // Fixed as live's, placed where live has them: at the root as its occurrences say; inside a subassembly
        // relative to the first occurrence of an instance of it.
        for i in &l.instances {
            let Some(id) = i["id"].as_str() else { continue };
            let Some(name) = ours.get(&(l.element.clone(), id.to_string())).cloned() else { continue };
            let found = occurrences.iter().map(|o| ids(&o["path"])).find(|p| p.last().map(String::as_str) == Some(id) && (k == 0) == (p.len() == 1) && (k == 0 || p.len() >= 2 && element_of.get(&p[p.len() - 2]) == Some(&l.element)));
            let Some(p) = found else { continue };
            let o = occurrences.iter().find(|o| ids(&o["path"]) == p).copied();
            let fixed = o.is_some_and(|o| o["fixed"] == true);
            if let Err(e) = placer.call("assembly_set_state", json!({ "tab": l.tab, "instances": [name], "fixed": fixed })) {
                res.untranslated.push(format!("{name}: fixed: {e}"));
            }
            let m = match (transform_of(&p).and_then(|m| mat(&m)), (p.len() > 1).then(|| transform_of(&p[..p.len() - 1]).and_then(|m| mat(&m)))) {
                (Some(m), None) => Some(m),
                (Some(m), Some(Some(parent))) => Some(parent.inverse() * m),
                _ => None,
            };
            if let Some(m) = m
                && let Err(e) = placer.place(&l.tab, &name, &row_major(m))
            {
                res.untranslated.push(format!("{name}: placed: {e}"));
            }
        }
        // An occurrence path of this level as our instance names ("Sub <1> > Part 1 <1>").
        let path_names = |path: &[String]| -> Option<String> {
            let mut level = l.element.clone();
            let mut names = Vec::new();
            for id in path {
                names.push(ours.get(&(level.clone(), id.clone()))?.clone());
                level = element_of.get(id)?.clone();
            }
            Some(names.join(" > "))
        };
        // The mates, between the same connectors.
        for f in l.features.iter().filter(|f| f["btType"].as_str().is_some_and(|t| t.starts_with("BTMMate-"))) {
            let name = f["name"].as_str().unwrap_or("").to_string();
            let par = |id: &str| f["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == id);
            let Some(ty) = par("mateType").and_then(|p| p["value"].as_str()).and_then(mate_type) else {
                res.untranslated.push(format!("{name}: mate type {:?} not mapped", par("mateType").map(|p| &p["value"])));
                continue;
            };
            let ends: Vec<Option<Value>> = par("mateConnectorsQuery")
                .and_then(|p| p["queries"].as_array())
                .into_iter()
                .flatten()
                .map(|q| {
                    let key = (l.element.clone(), q["featureId"].as_str()?.to_string());
                    let (path, inf, _) = connectors.get(&key)?;
                    let mut c = json!({ "instance": path_names(path)?, "entity": entities.get(&key)? });
                    if let Some(i) = inference(inf) {
                        c["inference"] = json!(i);
                    }
                    Some(c)
                })
                .collect();
            let [Some(a), Some(b)] = ends.as_slice() else {
                res.untranslated.push(format!("{name}: its connectors not translated"));
                continue;
            };
            let mut args = json!({ "tab": l.tab, "type": ty, "a": a, "b": b, "name": name });
            // As stored: the flip, the secondary axis's quarter turns, the offset; a parameter left out (live's UI
            // fetch) has its default, false and PLUS_X, not the dialog's choice.
            let flip = par("primaryAxisAlignment").and_then(|p| p["value"].as_bool()).unwrap_or(false);
            args["flip_primary"] = json!(flip);
            let turns = par("secondaryAxisAlignment").and_then(|p| p["value"].as_str()).map_or(Some(0), |v| ["PLUS_X", "PLUS_Y", "MINUS_X", "MINUS_Y"].iter().position(|x| *x == v));
            if let Some(turns) = turns {
                args["reorient_secondary"] = json!(turns);
            }
            if par("mateOffsetEnabled").is_some_and(|p| p["value"] == true) {
                let expr = |id: &str, unit: &str| par(id).map(|p| p["expression"].as_str().map(str::to_string).unwrap_or_else(|| format!("{} {unit}", p["value"].as_f64().unwrap_or(0.0))));
                let mut offset = json!({});
                for (axis, id) in [("x", "mateOffsetDistanceX"), ("y", "mateOffsetDistanceY"), ("z", "mateOffsetDistanceZ")] {
                    if let Some(e) = expr(id, "m") {
                        offset[axis] = json!(e);
                    }
                }
                if let Some(e) = expr("rotation", "rad") {
                    offset["rotation"] = json!(e);
                }
                args["offset"] = offset;
                if let Some(axis) = par("rotationType").and_then(|p| p["value"].as_str()).and_then(|v| v.strip_prefix("ABOUT_")) {
                    args["rotation_axis"] = json!(axis.to_lowercase());
                }
            }
            trace(&format!("mate {}", args["name"]));
            match placer.call("assembly_mate", args) {
                Ok(_) => {
                    if k == 0 {
                        mate_names.push((f["featureId"].as_str().unwrap_or("").to_string(), name));
                    }
                }
                Err(e) => res.untranslated.push(format!("{name}: {e}")),
            }
        }
    }
    let top: Vec<&Value> = occurrences.iter().copied().filter(|o| ids(&o["path"]).len() == 1).collect();
    let top_name = |o: &Value| ids(&o["path"]).first().and_then(|id| ours.get(&(String::new(), id.clone()))).cloned();
    // Adding a mate snaps its connectors into line as the dialog does; live's places are after any drag since: moved
    // there again, the mates solving. All held fixed while placed (a free one dragged pulls its mated neighbours and
    // stops short), compared there, then freed as live has them.
    let held: Vec<String> = top.iter().filter(|o| o["fixed"] != true).filter_map(|o| top_name(o)).collect();
    if !held.is_empty()
        && let Err(e) = placer.call("assembly_set_state", json!({ "tab": "Assembly 1", "instances": held, "fixed": true }))
    {
        res.differences.push(format!("holding the instances: {e}"));
    }
    for o in top.iter().filter(|o| o["fixed"] != true) {
        let Some(name) = top_name(o) else { continue };
        if let Some(m) = o["transform"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
            && let Err(e) = placer.place("Assembly 1", &name, &m)
        {
            res.differences.push(format!("{name}: not moved to live's place after the mates: {e}"));
        }
    }
    trace("comparing");
    // Compared with live: each occurrence's place (a top-level instance by its placement, a part inside a
    // subassembly by assembly_list's occurrences; a subassembly inside another is placed by its parts), each mate's
    // status.
    let ours_list = match placer.call("assembly_list", json!({ "tab": "Assembly 1" })) {
        Ok(l) => l,
        Err(e) => {
            res.differences.push(format!("assembly_list: {e}"));
            return res;
        }
    };
    let k = per_unit(&ours_list["units"]);
    let names_of = |path: &[String]| -> Option<String> {
        let mut level = String::new();
        let mut names = Vec::new();
        for id in path {
            names.push(ours.get(&(level.clone(), id.clone()))?.clone());
            level = element_of.get(id)?.clone();
        }
        Some(names.join(" > "))
    };
    for o in &occurrences {
        let p = ids(&o["path"]);
        if p.len() > 1 && levels.iter().any(|l| Some(&l.element) == element_of.get(p.last().unwrap_or(&String::new()))) {
            continue;
        }
        res.occurrences += 1;
        let Some(name) = names_of(&p) else {
            res.differences.push(format!("occurrence {}: not inserted", o["path"]));
            continue;
        };
        let found = if p.len() == 1 { ours_list["instances"].as_array().into_iter().flatten().find(|i| i["name"] == name.as_str()) } else { ours_list["occurrences"].as_array().into_iter().flatten().find(|i| i["path"] == name.as_str()) };
        let Some(i) = found else {
            res.differences.push(format!("{name}: not in ours"));
            continue;
        };
        let Some((t, q)) = o["transform"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()).and_then(|m| transform_parts(&m)) else { continue };
        let ot: Vec<f64> = (0..3).map(|j| i["translation"][j].as_f64().unwrap_or(f64::NAN) * k).collect();
        let oq: Vec<f64> = (0..4).map(|j| i["rotation"][j].as_f64().unwrap_or(f64::NAN)).collect();
        let dt = (0..3).map(|j| (ot[j] - t[j]).abs()).fold(0.0, f64::max);
        // q and −q are one rotation.
        let dq = (0..4).map(|j| (oq[j] - q[j]).abs()).fold(0.0, f64::max).min((0..4).map(|j| (oq[j] + q[j]).abs()).fold(0.0, f64::max));
        if dt <= 1e-6 && dq <= 1e-6 {
            res.occurrences_matching += 1;
        } else {
            res.differences.push(format!("{name}{}: at {:?} mm, rotation {oq:?}; live {:?} mm, {q:?} (off {:.6} mm / {:.2e})", if i["fixed"] == true { " (fixed)" } else { "" }, ot.iter().map(|c| c * 1e3).collect::<Vec<_>>(), t.map(|c| c * 1e3), dt * 1e3, dq));
        }
    }
    // Freed: live's places satisfy its mates, so ours shouldn't move them.
    let after = if held.is_empty() { Ok(ours_list.clone()) } else { placer.call("assembly_set_state", json!({ "tab": "Assembly 1", "instances": held, "fixed": false })).and_then(|_| placer.call("assembly_list", json!({ "tab": "Assembly 1" }))) };
    let Ok(after) = after else {
        res.differences.push("freeing the placed instances failed".into());
        return res;
    };
    let states = &asm["features"]["featureStates"];
    for (id, name) in &mate_names {
        res.mates += 1;
        let live_ok = states[id.as_str()]["featureStatus"].as_str().unwrap_or("OK") == "OK";
        let Some(m) = after["mates"].as_array().into_iter().flatten().find(|m| m["name"] == name.as_str()) else { continue };
        let ok = m["ok"] == true;
        if ok == live_ok {
            res.mates_matching += 1;
        } else {
            res.differences.push(format!("{name}: {}, live {}", if ok { "OK".to_string() } else { format!("error ({})", m["message"].as_str().unwrap_or("")) }, states[id.as_str()]["featureStatus"].as_str().unwrap_or("?")));
        }
    }
    let at = |l: &Value, name: &str| l["instances"].as_array().into_iter().flatten().find(|i| i["name"] == name).map(|i| (0..3).map(|j| i["translation"][j].as_f64().unwrap_or(f64::NAN) * k).collect::<Vec<_>>());
    for name in &held {
        if let (Some(a), Some(b)) = (at(&ours_list, name), at(&after, name)) {
            let moved = (0..3).map(|j| (a[j] - b[j]).abs()).fold(0.0, f64::max);
            if moved > 1e-6 {
                res.differences.push(format!("{name}: freed at live's place, our mates move it {:.6} mm", moved * 1e3));
            }
        }
    }
    res
}

/// A sketch pattern instance id the sketch doesn't list (`<seed>.<group>.<i>.<k>`, `.center` for its centre): the
/// id of instance i mod n of the pattern constraint whose instances share its seed (`sketch`: the sketch feature).
fn pattern_alias(sketch: &Value, id: &str) -> Option<String> {
    let (id, center) = match id.strip_suffix(".center") {
        Some(i) => (i, true),
        None => (id, false),
    };
    let parts: Vec<&str> = id.split('.').collect();
    let [base @ .., _, i, _] = parts.as_slice() else { return None };
    let (prefix, i) = (format!("{}.", base.join(".")), i.parse::<usize>().ok()?);
    for c in sketch["constraints"].as_array().into_iter().flatten() {
        // Its instances (the curves, not their centres) by index.
        let mut instances: Vec<(usize, String)> = c["parameters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some((p["parameterId"].as_str()?.strip_prefix("localInstance1,")?.parse().ok()?, p["value"].as_str()?.to_string())))
            .collect();
        if !instances.iter().any(|(_, v)| v.starts_with(&prefix)) {
            continue;
        }
        instances.sort();
        let (_, alias) = instances.get(i % instances.len())?;
        return Some(if center { format!("{alias}.center") } else { alias.clone() });
    }
    None
}

#[cfg(test)]
mod tests {

    /// A written query's `var subQk=…;` definitions put in where they're used (subQ10 not read as subQ1 + "0"), and
    /// a written list split at its own commas only.
    #[test]
    fn a_written_querys_vars_are_inlined_and_its_lists_split_at_their_own_commas() {
        let q = super::inline_query_vars("{var subQ1=a(\"x\",1);var subQ10=b(2,3);query=f(subQ10,subQ1);}").unwrap();
        assert_eq!(q, "f(b(2,3),a(\"x\",1))");
        let items = super::top_level_items("g(1,[2,3]),\"p,q\", h({\"k\":4}) ],rest").unwrap();
        assert_eq!(items, ["g(1,[2,3])", "\"p,q\"", "h({\"k\":4})"]);
    }

    /// A packed face query's own edges, not the curves its vertices meet (named one level down): live's query for one
    /// of c2-sm-exhaust-clamp's small disks, a circle touching a line and a ring, names all three, its own edges only
    /// the circle (split in two at the tangent points).
    #[test]
    fn a_packed_faces_own_edges_are_its_topology_not_its_neighbours() {
        let tree = noshape_model::custom::host::packed::unpack("&242$eJyNUUtTwjAQ/jPxCNOUPuBY2hQy2hY3EemJCU2EKM8+dPDXmxbGUbx42s3u99hvcjd20WOjynPiMmwjqSuxW+l1I2p92EeiFgFO7L8bfj4qNkQ8m2UP2SQ3rdrXutaqCuzADq3EYyskVanflYzLw85MXCYuoHNLHgtEvh/MQSSaEEPZ6Ko+lOerfAgk4DRLzeJwVGXnTOXYRlQGmGHc91DMnreT02JgEyqnllhaH7pU2ZGN0KlN1QlJxO4JD6dLknLKc6ZQ9abqYnM5gEpWoLzxYjpP4sWiLsBqMwMGGwbXNC441+qCBz4MYQQrKECCMmTSnIo0dNV+LQRgC9zQZx6aE+BkcQGHhbm3J/s+0rtjqfc1FOZCmnICjIQ8wv9weeWZ0p9P2zyLqs4FsHMFYc+gsB/18A9mO2Q+oskMjBHg0eVfwGnJNxEBDwG/tFUZ4I0I2E43NKkcFAch+b37AjUdsPk=").unwrap();
        let own = super::packed_face_edges(&tree);
        assert_eq!(own, vec!["Yu6FIVMFXXtc".to_string(); 2]);
        let mut all = Vec::new();
        super::packed_strings(&tree, &mut all);
        for neighbour in ["EuqcNC5engaa", "jTOeizUlYODs"] {
            assert!(all.iter().any(|s| s == neighbour), "{all:?}");
        }
    }

    /// stef-radijator's Fillet 1 names `TNHaprdwY81W.1.18.0` of an 18-instance circular pattern listing the seed
    /// `aszIiqlQyJeU` at 0 and `TNHaprdwY81W.0.1.0` … `.1.17.0`: the seed (live rounds the edge at its crossing).
    #[test]
    fn a_pattern_instance_past_the_last_is_the_seed() {
        let mut params = vec![serde_json::json!({ "parameterId": "localInstance1,0", "value": "aszIiqlQyJeU" }), serde_json::json!({ "parameterId": "localInstance0,0", "value": "aszIiqlQyJeU.center" })];
        for i in 1..18 {
            let g = if i < 10 { 0 } else { 1 };
            params.push(serde_json::json!({ "parameterId": format!("localInstance1,{i}"), "value": format!("TNHaprdwY81W.{g}.{i}.0") }));
        }
        let sketch = serde_json::json!({ "featureId": "FS", "constraints": [{ "constraintType": "CIRCULAR_PATTERN", "parameters": params }] });
        assert_eq!(super::pattern_alias(&sketch, "TNHaprdwY81W.1.18.0").as_deref(), Some("aszIiqlQyJeU"));
        assert_eq!(super::pattern_alias(&sketch, "TNHaprdwY81W.1.18.0.center").as_deref(), Some("aszIiqlQyJeU.center"));
        assert_eq!(super::pattern_alias(&sketch, "TNHaprdwY81W.1.20.0").as_deref(), Some("TNHaprdwY81W.0.2.0"));
        assert_eq!(super::pattern_alias(&sketch, "QFFmUTV2brKS"), None);
    }

    /// Two circles cross twice; the crossings come in order along the
    /// first (stef-radijator's SWEPT_EDGE ORDER 0 / 1 of a ring and a
    /// patterned hole on it).
    #[test]
    fn crossings_in_order_along_the_first_curve() {
        let circle = |c: [f64; 2], r: f64| (0..=64).map(|i| {
            let a = i as f64 / 64.0 * std::f64::consts::TAU;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        }).collect::<Vec<_>>();
        let ring = circle([0.0, 0.0], 0.0225);
        let hole = circle([0.0, -0.0225], 0.0025);
        let x = super::crossings(&ring, &hole);
        assert_eq!(x.len(), 2, "{x:?}");
        // Counter-clockwise from +X: the crossing at x < 0 comes first.
        assert!(x[0][0] < 0.0 && x[1][0] > 0.0 && x.iter().all(|p| (p[0].hypot(p[1]) - 0.0225).abs() < 1e-4), "{x:?}");
    }
}
