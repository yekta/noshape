//! The model parity suite's translator (docs/PARITY-SUITES.md §1): an
//! Onshape Part Studio's feature list (`GET …/features` JSON) rebuilt in a
//! noshape document through the ops registry, the CLI's path, one feature at
//! a time. A test tool, not a user feature. What it can't translate it
//! reports per feature with the reason.
//!
//! Queries name Onshape's own entities (`deterministicIds`), which mean
//! nothing here: each is matched by geometry from `refs.jsonl` (live's
//! evaluation of every referenced id before the feature), else by the few
//! forms readable from the query itself (default planes, a sketch's
//! regions).

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
    /// Our regeneration status of it ("Ok", "Error", …).
    pub status: Option<String>,
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
    /// What isn't translated of the document itself (configuration inputs…).
    pub notes: Vec<String>,
}

fn f3(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

fn mm(p: [f64; 3]) -> Value {
    json!([p[0] * 1000.0, p[1] * 1000.0, p[2] * 1000.0])
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
            let (Some(id), Some(m)) = (s["featureId"].as_str(), s["sketchMatrix"].as_array()) else { continue };
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
        let mut t = Self { ctx, features, refs, feature_ids: HashMap::new(), sketch_entities: HashMap::new(), sketch_points: Vec::new(), frames, curves: HashMap::new(), config_defaults, notes: Vec::new() };
        t.call("document_create", json!({ "path": path.to_str().ok_or("path")? }))?;
        for c in doc["configuration"]["configurationParameters"].as_array().into_iter().flatten() {
            t.config_input(c);
        }
        Ok(t)
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
        let made = if f["btType"].as_str().is_some_and(|t| t.starts_with("BTMSketch")) { self.sketch(k, &f, &mut step.problems) } else { self.feature(k, &f, &mut step.problems) };
        match made {
            Ok((id, status)) => {
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
            }
            Err(e) => step.problems.insert(0, format!("not built: {e}")),
        }
        step
    }

    /// A non-sketch feature through feature_add, its parameters by name.
    fn feature(&mut self, k: usize, f: &Value, problems: &mut Vec<String>) -> Result<(String, String), String> {
        let ty = f["featureType"].as_str().ok_or("no featureType")?;
        let schema = self.call("feature_schema", json!({ "type": ty, "all": true })).map_err(|e| format!("feature type `{ty}` is not in noshape ({e})"))?;
        let known: Vec<String> = schema["params"]["properties"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
        let mut params = Map::new();
        let mut unknown = Vec::new();
        for p in f["parameters"].as_array().into_iter().flatten() {
            let Some(id) = p["parameterId"].as_str() else { continue };
            if INTERNAL.contains(&id) {
                continue;
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
        upgrade(ty, &mut params, &mut unknown);
        if !unknown.is_empty() {
            problems.push(format!("parameters not in noshape's {ty}: {}", unknown.join(", ")));
        }
        let mut args = json!({ "type": ty, "params": params });
        if let Some(n) = f["name"].as_str() {
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
        Ok((r["feature"].as_str().unwrap_or("").to_string(), r["feature_status"]["status"].as_str().unwrap_or("").to_string()))
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
            return ids.iter().map(|id| self.by_geometry(k, id)).collect();
        }
        if let Some(p) = default_plane(text) {
            return Ok(vec![p.to_string()]);
        }
        // A Plane feature's plane (`dummyQuery(id+"F….planeOp", …)`).
        if let Some(fid) = text.split("id+\"").nth(1).and_then(|r| r.split(".planeOp\"").next()).filter(|f| !f.contains('"'))
            && let Some(ours) = self.feature_ids.get(fid)
        {
            return Ok(vec![format!("fplane:{ours}")]);
        }
        if let Some(r) = self.region_from_query(text)? {
            return Ok(vec![r]);
        }
        if let Some(e) = self.sketch_entity_from_query(text) {
            return Ok(vec![e]);
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
        // qSketchRegion(id + "F…", true): every region of that sketch.
        if let Some(sk) = text.split("qSketchRegion(id + \"").nth(1).and_then(|r| r.split('"').next()) {
            let ours = self.feature_ids.get(sk).cloned().ok_or_else(|| format!("sketch {sk} was not built"))?;
            let g = self.call("sketch_get", json!({ "sketch": ours }))?;
            return Ok(g["regions"].as_array().into_iter().flatten().filter_map(|r| r["id"].as_str().map(str::to_string)).collect());
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
        let segments: Vec<[f64; 2]> = strings.iter().filter_map(|e| self.curves.get(&(sk.clone(), e.clone()))).flat_map(|poly| poly.windows(2).flat_map(|w| [w[0], w[1]])).collect();
        if segments.is_empty() {
            return Err(format!("region of sketch {sk}: its curves were not translated"));
        }
        let ours = self.feature_ids.get(&sk).cloned().ok_or_else(|| format!("sketch {sk} was not built"))?;
        let g = self.call("sketch_get", json!({ "sketch": ours }))?;
        let (o, x, y) = (f3(&g["frame"]["origin"]), f3(&g["frame"]["x"]), f3(&g["frame"]["y"]));
        let (Some(o), Some(x), Some(y)) = (o, x, y) else { return Err("sketch frame".into()) };
        let frame = self.frames.get(&sk).copied();
        let mut best: Option<(f64, String)> = None;
        for r in g["regions"].as_array().into_iter().flatten() {
            let (u, v) = (r["inside"][0].as_f64().unwrap_or(0.0), r["inside"][1].as_f64().unwrap_or(0.0));
            // Ours (mm, our frame) → Onshape's sketch plane (m).
            let w: Vec<f64> = (0..3).map(|i| (o[i] + u * x[i] + v * y[i]) / 1000.0).collect();
            let p = match frame {
                Some([fo, fx, fy]) => [(0..3).map(|i| (w[i] - fo[i]) * fx[i]).sum::<f64>(), (0..3).map(|i| (w[i] - fo[i]) * fy[i]).sum::<f64>()],
                None => [u / 1000.0, v / 1000.0],
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
        best.map(|b| Some(b.1)).ok_or_else(|| format!("no region of ours inside sketch {sk}'s curves {:?}", strings.iter().filter(|e| self.curves.contains_key(&(sk.clone(), (*e).clone()))).collect::<Vec<_>>()))
    }

    /// A sketch curve or point named by its Onshape id (packed
    /// `SKETCH_ENTITY` query of a sketch's own wire, not of a feature made
    /// from it).
    fn sketch_entity_from_query(&self, text: &str) -> Option<String> {
        let packed = text.split("qCompressed(1.0,\"").nth(1)?.split('"').next()?;
        let tree = noshape_model::custom::host::packed::unpack(packed).ok()?;
        let mut strings = Vec::new();
        packed_strings(&tree, &mut strings);
        if !strings.iter().any(|s| s == "SKETCH_ENTITY") || strings.iter().any(|s| s == "IMPRINT") {
            return None;
        }
        let mut ops = strings.iter().filter(|s| s.contains(".op") || s.ends_with("Op"));
        let sk = ops.next()?.strip_suffix(".wireOp")?.to_string();
        if ops.next().is_some() {
            return None;
        }
        strings.iter().find_map(|e| self.sketch_entities.get(&(sk.clone(), e.clone())).cloned())
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
                let q = mm(p);
                let at = |v: &Value, i: usize| v.as_array().and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(f64::NAN);
                parts["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|b| (0..3).all(|i| at(&b["bounding_box"]["min"], i) - 1e-3 <= at(&q, i) && at(&q, i) <= at(&b["bounding_box"]["max"], i) + 1e-3))
                    .and_then(|b| b["id"].as_str().map(str::to_string))
                    .ok_or_else(|| format!("{id}: no part of ours around {q}"))
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
        let mut found = self.call("query_entities", a.clone()).ok()?[kind].as_array().cloned().unwrap_or_default();
        if found.is_empty() && r.normal.is_some() {
            // The normal's sense isn't always certain.
            a.as_object_mut().map(|m| m.remove("normal"));
            found = self.call("query_entities", a).ok()?[kind].as_array().cloned().unwrap_or_default();
        }
        let (key, scale) = if kind == "faces" { ("area", 1e6) } else { ("length", 1e3) };
        let same = |e: &&Value| r.measure.zip(e[key].as_f64()).is_some_and(|(m, ours)| (ours / scale - m).abs() <= 1e-5 * m.abs() + 1e-12);
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
                let w: Vec<f64> = (0..3).map(|i| (o[i] + u * x[i] + v * y[i]) / 1000.0).collect();
                let d = (0..3).map(|i| (w[i] - p[i]).powi(2)).sum::<f64>().sqrt();
                if let Some(a) = area
                    && r["area"].as_f64().is_none_or(|ours| (ours * 1e-6 - a).abs() > 1e-6 * a.max(1e-12))
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
    fn sketch(&mut self, k: usize, f: &Value, problems: &mut Vec<String>) -> Result<(String, String), String> {
        let oid = f["featureId"].as_str().unwrap_or("").to_string();
        let plane_q = f["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == "sketchPlane").ok_or("no sketchPlane")?;
        let plane = self.param(k, plane_q)?.and_then(|v| v.as_array().and_then(|a| a.first()).cloned()).ok_or("no sketch plane")?;
        let mut args = json!({ "plane": plane });
        if let Some(n) = f["name"].as_str() {
            args["name"] = json!(n);
        }
        let made = self.call("sketch_create", args)?;
        let id = made["sketch"].as_str().ok_or("sketch_create gave no id")?.to_string();
        let frame = self.frames.get(&oid).copied();
        // Sketch (x, y) in metres → a sketch_draw point: model mm through
        // Onshape's frame when known, else our frame's mm.
        let at = |u: f64, v: f64| match frame {
            Some([o, x, y]) => mm([o[0] + u * x[0] + v * y[0], o[1] + u * x[1] + v * y[1], o[2] + u * x[2] + v * y[2]]),
            None => json!([u * 1000.0, v * 1000.0]),
        };
        let mut shapes = Vec::new();
        let mut local = Vec::new();
        for e in f["entities"].as_array().into_iter().flatten() {
            let eid = e["entityId"].as_str().unwrap_or("").to_string();
            let g = &e["geometry"];
            let construction = e["isConstruction"].as_bool() == Some(true);
            let gt = g["btType"].as_str().unwrap_or("");
            let num = |k: &str| g[k].as_f64().unwrap_or(0.0);
            let shape = match gt.split('-').next().unwrap_or("") {
                "BTCurveGeometryLine" => {
                    let (t0, t1) = (e["startParam"].as_f64().unwrap_or(0.0), e["endParam"].as_f64().unwrap_or(0.0));
                    let (px, py, dx, dy) = (num("pntX"), num("pntY"), num("dirX"), num("dirY"));
                    Some(json!({ "tool": "line", "points": [at(px + t0 * dx, py + t0 * dy), at(px + t1 * dx, py + t1 * dy)], "construction": construction }))
                }
                "BTCurveGeometryCircle" => {
                    let (cx, cy, r, xd, yd) = (num("xCenter"), num("yCenter"), num("radius"), num("xDir"), num("yDir"));
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
                        let (t0, t1) = (e["startParam"].as_f64().unwrap_or(0.0), e["endParam"].as_f64().unwrap_or(0.0));
                        Some(json!({ "tool": "three_point_arc", "points": [pt(t0), pt(t1), pt((t0 + t1) / 2.0)], "construction": construction }))
                    }
                }
                "BTCurveGeometryEllipse" => {
                    let (cx, cy, a, b, xd, yd) = (num("xCenter"), num("yCenter"), num("majorRadius"), num("minorRadius"), num("xDir"), num("yDir"));
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
            if let Some(poly) = sampled(e) {
                self.curves.insert((oid.clone(), eid.clone()), poly);
            }
            match shape {
                Some(s) => {
                    shapes.push(s);
                    local.push(eid);
                }
                None => problems.push(format!("sketch entity {eid}: {} not translated", if gt.is_empty() { e["btType"].as_str().unwrap_or("?") } else { gt })),
            }
        }
        // Curves first, then the free points, so each reply lists ours in
        // Onshape's order.
        for points in [false, true] {
            let (batch, ids): (Vec<Value>, Vec<String>) = shapes.iter().zip(&local).filter(|(s, _)| (s["tool"] == "point") == points).map(|(s, e)| (s.clone(), e.clone())).unzip();
            if batch.is_empty() {
                continue;
            }
            let r = self.call("sketch_draw", json!({ "sketch": id, "shapes": batch }))?;
            let made: Vec<&Value> = r["created"].as_array().into_iter().flatten().filter(|c| (c["type"] == "point") == points).collect();
            for (eid, c) in ids.iter().zip(made) {
                if let Some(cid) = c["id"].as_str() {
                    self.sketch_entities.insert((oid.clone(), eid.clone()), cid.to_string());
                    if points {
                        self.sketch_points.push((oid.clone(), cid.to_string()));
                    }
                }
                for (end, key) in [("start", "start_point"), ("end", "end_point"), ("center", "center_point")] {
                    if let Some(pid) = c[key].as_str() {
                        self.sketch_entities.insert((oid.clone(), format!("{eid}.{end}")), pid.to_string());
                    }
                }
            }
        }
        self.dimensions(&oid, &id, f, problems);
        let st = self.call("feature_list", json!({}))?;
        let status = st["features"].as_array().into_iter().flatten().find(|x| x["id"] == id).and_then(|x| x["status"].as_str()).unwrap_or("Ok").to_string();
        Ok((id, status))
    }

    /// The sketch's dimensions with Onshape's expressions (so variables and
    /// configurations drive them); other constraints are left to our
    /// inference at the solved positions and counted.
    fn dimensions(&mut self, oid: &str, id: &str, f: &Value, problems: &mut Vec<String>) {
        let mut others = 0;
        for c in f["constraints"].as_array().into_iter().flatten() {
            let ty = c["constraintType"].as_str().unwrap_or("");
            let kind = match ty {
                "DISTANCE" => "distance",
                "LENGTH" => "length",
                "DIAMETER" => "diameter",
                "RADIUS" => "radius",
                "ANGLE" => "angle",
                _ => {
                    others += 1;
                    continue;
                }
            };
            let par = |name: &str| c["parameters"].as_array().into_iter().flatten().find(|p| p["parameterId"] == name).cloned();
            let value = ["length", "angle"].iter().find_map(|n| par(n)).and_then(|p| p["expression"].as_str().map(str::to_string));
            let mut entities = Vec::new();
            for n in ["localFirst", "externalFirst", "localSecond", "externalSecond", "localEntity1", "localEntity2"] {
                if n.starts_with("external") {
                    let Some(ext) = par(n) else { continue };
                    let text: String = ext["queries"].as_array().into_iter().flatten().filter_map(|q| q["queryString"].as_str()).collect();
                    if text.contains("Origin") || text.contains("originOp") {
                        entities.push("origin".to_string());
                    } else {
                        problems.push(format!("{ty} dimension: external reference not translated ({})", text.chars().take(60).collect::<String>()));
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
            if entities.is_empty() {
                continue;
            }
            let mut args = json!({ "sketch": id, "entities": entities, "kind": kind });
            if let Some(v) = value {
                args["value"] = json!(v);
            }
            if let Err(e) = self.call("sketch_dimension", args) {
                problems.push(format!("{ty} dimension: {e}"));
            }
        }
        if others > 0 {
            problems.push(format!("{others} geometric constraints left to inference (geometry at Onshape's solved positions)"));
        }
    }
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
            let (t0, t1) = match (e["startParam"].as_f64(), e["endParam"].as_f64()) {
                (Some(a), Some(b)) => (a, b),
                _ => (0.0, std::f64::consts::TAU),
            };
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
            let (cx, cy, a, b, xd, yd) = (num("xCenter"), num("yCenter"), num("majorRadius"), num("minorRadius"), num("xDir"), num("yDir"));
            let sense = if g["clockwise"].as_bool() == Some(true) { -1.0 } else { 1.0 };
            let (t0, t1) = match (e["startParam"].as_f64(), e["endParam"].as_f64()) {
                (Some(x), Some(y)) => (x, y),
                _ => (0.0, std::f64::consts::TAU),
            };
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
/// Onshape upgrades them: Revolve's `revolveType`, a Variable's `value`.
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
        "assignVariable" => {
            if let Some(v) = done("value", params) {
                let key = match params.get("variableType").and_then(Value::as_str) {
                    Some("ANGLE") => "angleValue",
                    Some("NUMBER") => "numberValue",
                    Some("ANY") => "anyValue",
                    _ => "lengthValue",
                };
                params.insert(key.into(), v);
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
#[derive(Clone, Debug)]
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
}

/// Live's parts of one live.jsonl line.
pub fn live_parts(line: &Value) -> Vec<PartState> {
    let band = |v: &Value| v.as_array().and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]));
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
        })
        .collect()
}

/// Our parts now, SI.
pub fn our_parts(t: &mut Translator) -> Result<Vec<PartState>, String> {
    let r = t.call("query_parts", json!({}))?;
    let at = |v: &Value| f3(v).map(|p| p.map(|c| c / 1000.0));
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
            volume: p["volume"].as_f64().map(|v| v * 1e-9),
            area: p["surface_area"].as_f64().map(|a| a * 1e-6),
            volume_band: None,
            area_band: None,
            bbox: at(&p["bounding_box"]["min"]).zip(at(&p["bounding_box"]["max"])).map(|(a, b)| [a, b]),
            centroid: at(&p["centroid"]),
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
        if (o.faces, o.edges, o.vertices) != (l.faces, l.edges, l.vertices) {
            out.push(format!("{who}: {}/{}/{} faces/edges/vertices, live {}/{}/{}", o.faces, o.edges, o.vertices, l.faces, l.edges, l.vertices));
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
            && (0..3).any(|j| (a[j] - b[j]).abs() > 1e-6)
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
