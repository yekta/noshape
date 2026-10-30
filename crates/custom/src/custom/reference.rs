//! Values of FeatureScript reference parameters (std tabReferences.fs:
//! `TableData`, `ImageData`, `JSONData`, `TextData`) built from the
//! referenced file tab. Live (dom/fs-refparams.txt): a CSV gives
//! `{ blobType : CSV , csvData : [ [ 1 , 2 , a ] , ... ] , dataId : ... ,
//! documentId : BuiltIn: 1 , elementId : ... , mediaType : text/csv ,
//! microversionId : ... , versionId : ... }`; an unset field is `{}`.

use crate::file_tab::FileTab;
use crate::params::ReferenceKind;
use noshape_featurescript::value::{Builtin, Map, Tag, Value, next_serial};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// An opaque document / element / version handle (prints "BuiltIn: n").
#[derive(Debug)]
struct Handle {
    serial: u64,
}

impl Builtin for Handle {
    fn type_name(&self) -> &'static str {
        "BuiltIn"
    }
    fn serial(&self) -> u64 {
        self.serial
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// File contents by `dataId` (skImage finds the picture of a BLOB_DATA).
static BLOBS: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<String, Arc<str>>>> = std::sync::LazyLock::new(Default::default);

/// The base64 contents of the file a `dataId` came from.
pub fn blob_of(data_id: &str) -> Option<Arc<str>> {
    BLOBS.lock().get(data_id).cloned()
}

fn handle() -> Value {
    Value::Builtin(Arc::new(Handle { serial: next_serial() }), Tag::NONE)
}

/// The value of a reference parameter pointing at `file` (`{}` when unset),
/// tagged with its std type (`TableData`, ...).
pub fn reference_value(kind: ReferenceKind, file: Option<&FileTab>) -> Value {
    let tag = super::host::std_tag(
        "tabReferences.fs",
        match kind {
            ReferenceKind::Csv => "TableData",
            ReferenceKind::Image => "ImageData",
            ReferenceKind::Json => "JSONData",
            ReferenceKind::Text => "TextData",
            ReferenceKind::PartStudio => "PartStudioData",
            ReferenceKind::VariableStudio => "VariableStudioData",
            ReferenceKind::CadImport => "CADImportData",
        },
    );
    untagged(kind, file).with_tag(tag)
}

fn untagged(kind: ReferenceKind, file: Option<&FileTab>) -> Value {
    let Some(f) = file else { return Value::map(Map::new()) };
    let bytes = f.bytes().unwrap_or_default();
    let mut m = Map::new();
    let mut put = |k: &str, v: Value| {
        m.insert(Value::str(k), v);
    };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    f.file_name.hash(&mut h);
    let data_id = format!("{:024x}", h.finish() as u128 * 0x9e37_79b9);
    BLOBS.lock().insert(data_id.clone(), f.data_base64.clone());
    put("dataId", Value::str(data_id));
    // Created in this order (live: documentId 1, versionId 2, elementId 3,
    // microversionId 4).
    for k in ["documentId", "versionId", "elementId", "microversionId"] {
        put(k, handle());
    }
    let text = || String::from_utf8_lossy(&bytes).to_string();
    match kind {
        ReferenceKind::Csv => {
            put("blobType", Value::str("CSV"));
            put("mediaType", Value::str("text/csv"));
            let rows = csv_rows(&text());
            // A single row is a single array (std tabReferences.fs).
            let data = match rows.len() {
                1 => rows.into_iter().next().unwrap_or_default(),
                _ => Value::array(rows),
            };
            put("csvData", data);
        }
        ReferenceKind::Image => {
            put("blobType", Value::str("IMAGE"));
            let (w, ht, media) = image_size(&bytes);
            put("mediaType", Value::str(media));
            if let (Some(w), Some(ht)) = (w, ht) {
                put("imageWidth", Value::num(w as f64));
                put("imageHeight", Value::num(ht as f64));
            }
        }
        ReferenceKind::Json => {
            put("blobType", Value::str("JSON"));
            put("mediaType", Value::str("application/json"));
            if let Ok(j) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                put("jsonData", json_value(&j));
            }
        }
        ReferenceKind::Text => {
            put("blobType", Value::str("TEXT"));
            put("mediaType", Value::str("text/plain"));
            put("textData", Value::str(text()));
        }
        ReferenceKind::PartStudio | ReferenceKind::VariableStudio | ReferenceKind::CadImport => return Value::map(Map::new()),
    }
    Value::map(m)
}

/// CSV rows; numeric cells become numbers (live: `[ 1 , 2 , a ]`).
fn csv_rows(text: &str) -> Vec<Value> {
    let mut rows = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut cells = Vec::new();
        let mut cur = String::new();
        let mut quoted = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' if quoted && chars.peek() == Some(&'"') => {
                    cur.push('"');
                    chars.next();
                }
                '"' => quoted = !quoted,
                ',' if !quoted => cells.push(std::mem::take(&mut cur)),
                c => cur.push(c),
            }
        }
        cells.push(cur);
        rows.push(Value::array(
            cells
                .into_iter()
                .map(|c| match c.trim().parse::<f64>() {
                    Ok(x) if x.is_finite() => Value::num(x),
                    _ => Value::str(c),
                })
                .collect(),
        ));
    }
    rows
}

pub(crate) use crate::regen::custom_features::image_size;

/// A JSON value as FeatureScript (null is undefined).
fn json_value(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Undefined,
        serde_json::Value::Bool(b) => Value::bool(*b),
        serde_json::Value::Number(n) => Value::num(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::str(s.as_str()),
        serde_json::Value::Array(a) => Value::array(a.iter().map(json_value).collect()),
        serde_json::Value::Object(o) => {
            let mut m = Map::new();
            for (k, v) in o {
                let v = json_value(v);
                if !v.is_undefined() {
                    m.insert(Value::str(k.as_str()), v);
                }
            }
            Value::map(m)
        }
    }
}

/// Natives of reference parameters.
pub(super) const NATIVES: &[(&str, noshape_featurescript::interp::NativeFn)] = &[
    ("buildPartStudio", build_part_studio),
    ("blobData", blob_data),
    ("processedForeignDataInfo", |_, _| Ok(handle())),
];

/// `BLOB_DATA` of an imported file tab: the reference parameter map of its
/// kind (CSV, image, JSON, text).
fn blob_data(i: &mut noshape_featurescript::interp::Interp, a: Vec<Value>) -> noshape_featurescript::interp::R<Value> {
    let tab = a.first().and_then(|v| v.as_str()).unwrap_or_default();
    let external = || match super::provider::external_tab(&tab)?.content {
        crate::document::TabContent::File(f) => Some(f),
        _ => None,
    };
    let Some(file) = super::fallback_provider().file(&tab).or_else(external) else {
        return Err(i.error("The imported file no longer exists"));
    };
    let kind = match file.ext().as_str() {
        "csv" => ReferenceKind::Csv,
        "json" => ReferenceKind::Json,
        _ if file.kind() == crate::file_tab::FileKind::Image => ReferenceKind::Image,
        _ => ReferenceKind::Text,
    };
    Ok(untagged(kind, Some(&file)))
}

/// A stored Part Studio value split into the studio key and the picked
/// part: `tab#part`, or `ext:<microversion>|<tab>#<part>|<path>` for another
/// document or a version.
pub fn split_part(value: &str) -> (String, Option<String>) {
    if let Some(rest) = value.strip_prefix("ext:") {
        let mut it = rest.splitn(3, '|');
        if let (Some(mv), Some(tab), Some(path)) = (it.next(), it.next(), it.next()) {
            let (tab, part) = match tab.split_once('#') {
                Some((t, p)) => (t, Some(p.to_string())),
                None => (tab, None),
            };
            return (format!("ext:{mv}|{tab}|{path}"), part);
        }
    }
    match value.split_once('#') {
        Some((t, p)) => (t.to_string(), Some(p.to_string())),
        None => (value.to_string(), None),
    }
}

/// Transient id under which a part of a referenced Part Studio is known in
/// the context its `buildFunction` returns.
fn part_transient(tab: &str, body: &crate::reference::BodyId) -> Arc<str> {
    format!("{tab}:{body}").into()
}

/// The operation a feature type's body comes from (`extrude` → `opExtrude`).
fn op_name(type_id: &str) -> String {
    let mut c = type_id.chars();
    match c.next() {
        Some(f) => format!("op{}{}", f.to_ascii_uppercase(), c.as_str()),
        None => "op".into(),
    }
}

/// Onshape's query of a part made by a sweep-like feature from sketch
/// edges (live dom/fs-partquery): SWEPT_BODY by `<feature>.opExtrude`,
/// disambiguated by the sketch entities it came from (`<sketch>.wireOp`).
/// Fillets and booleans keep it. `None` when the faces don't tell.
fn swept_body_query(body: &crate::regen::context::Body, op: &str) -> Option<Value> {
    use crate::reference::{Role, Source, TopoName};
    let creator = body.id.feature;
    let mut originals: Vec<(crate::reference::FeatureId, u32)> = Vec::new();
    for f in &body.faces {
        if let TopoName::Face { feature, role: Role::Side(Source::Sketch { sketch, entity, .. }) } = f.root()
            && *feature == creator
            && !originals.contains(&(*sketch, *entity))
        {
            originals.push((*sketch, *entity));
        }
    }
    if originals.is_empty() {
        return None;
    }
    originals.sort();
    let e = |m: &str, n: &str, v: &str| super::host::std_enum(m, n, v).unwrap_or_else(|| Value::str(v));
    let tag = super::host::std_tag("query.fs", "Query");
    let originals: Vec<Value> = originals
        .iter()
        .map(|(sketch, entity)| {
            Value::map_from([
                ("entityType", e("query.fs", "EntityType", "EDGE")),
                ("historyType", Value::str("CREATION")),
                ("operationId", super::host::convert::id_value(&[format!("{sketch}.wireOp").into()])),
                ("queryType", e("query.fs", "QueryType", "SKETCH_ENTITY")),
                ("sketchEntityId", Value::str(entity.to_string())),
            ])
            .with_tag(tag)
        })
        .collect();
    let disambiguation = Value::map_from([("disambiguationType", Value::str("ORIGINAL_DEPENDENCY")), ("originals", Value::array(originals))]);
    Some(
        Value::map_from([
            ("disambiguationData", Value::array(vec![disambiguation])),
            ("entityType", e("query.fs", "EntityType", "BODY")),
            ("historyType", Value::str("CREATION")),
            ("operationId", super::host::convert::id_value(&[format!("{creator}.{op}").into()])),
            ("queryType", e("query.fs", "QueryType", "SWEPT_BODY")),
        ])
        .with_tag(tag),
    )
}

/// The picked part of a `PartStudioData` value.
/// (A sketch is listed like a part, as `reference::sketch_body_id`.)
fn picked_parts(ctx: &crate::regen::context::Context, part: Option<&str>) -> Vec<crate::reference::BodyId> {
    let sketches = ctx.sketches.iter().map(|s| crate::reference::sketch_body_id(s.feature));
    ctx.bodies.iter().map(|b| b.id.clone()).chain(sketches).filter(|b| part.is_some_and(|p| b.to_string() == p)).collect()
}

/// A stored Part Studio value: `<key>[#part]`, optionally followed by a tab
/// and the chosen configuration (`input id=value` lines joined by `;`).
pub fn split_configuration(value: &str) -> (&str, crate::configuration::ConfigValues) {
    let Some((v, cfg)) = value.split_once('\t') else { return (value, Default::default()) };
    let mut out = crate::configuration::ConfigValues::default();
    for kv in cfg.split(';').filter(|s| !s.is_empty()) {
        if let Some((k, x)) = kv.split_once('=') {
            out.0.insert(k.to_string(), x.to_string());
        }
    }
    (v, out)
}

/// The FeatureScript id of a configuration input (Onshape's default: the
/// input's name, live dom/fs-refcfg: "Size", "Hole", "Len", "Txt").
pub fn config_fs_id(input: &crate::configuration::ConfigInput) -> String {
    let mut id: String = input.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if id.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        id.insert(0, '_');
    }
    id
}

/// A configuration value as FeatureScript sees it: a list option's name,
/// a boolean, a quantity (`0.01 meter`) or a string.
fn config_value(input: &crate::configuration::ConfigInput, text: &str) -> Value {
    use crate::configuration::{ConfigInputKind, VariableKind};
    match &input.kind {
        ConfigInputKind::List { options, .. } => Value::str(options.iter().find(|o| o.id == text).map_or(text, |o| o.name.as_str())),
        ConfigInputKind::Checkbox { .. } => Value::bool(text == "true"),
        ConfigInputKind::Text { .. } => Value::str(text),
        ConfigInputKind::Variable { kind, .. } => {
            let q = noshape_units::parse(text).ok().and_then(|e| e.eval(&noshape_units::EmptyScope).ok());
            match (kind, q) {
                (VariableKind::Text, _) => Value::str(text),
                (VariableKind::Length, Some(q)) => super::host::convert::length_value(q.value),
                (VariableKind::Angle, Some(q)) => super::host::convert::angle_value(q.value),
                (_, Some(q)) => Value::num(q.value),
                (_, None) => Value::str(text),
            }
        }
    }
}

/// A FeatureScript configuration map back as stored values (missing inputs
/// keep their defaults; list values are option names or ids).
fn config_from_fs(cfg: &crate::configuration::Configuration, v: &Value) -> crate::configuration::ConfigValues {
    use crate::configuration::{ConfigInputKind, VariableKind};
    let mut out = cfg.defaults();
    for input in &cfg.inputs {
        let x = v.get(&config_fs_id(input));
        if x.is_undefined() {
            continue;
        }
        let text = match &input.kind {
            ConfigInputKind::List { options, .. } => {
                let s = x.as_str().map(|s| s.to_string()).unwrap_or_default();
                options.iter().find(|o| o.name == s || o.id == s).map_or(s, |o| o.id.clone())
            }
            ConfigInputKind::Checkbox { .. } => x.as_bool().unwrap_or(false).to_string(),
            ConfigInputKind::Text { .. } => x.as_str().map(|s| s.to_string()).unwrap_or_default(),
            ConfigInputKind::Variable { kind, .. } => match kind {
                VariableKind::Text => x.as_str().map(|s| s.to_string()).unwrap_or_default(),
                VariableKind::Length => super::host::convert::length_of(x).map(|m| format!("{m} m")).unwrap_or_default(),
                VariableKind::Angle => super::host::convert::angle_of(x).map(|r| format!("{r} rad")).unwrap_or_default(),
                _ => x.as_number().map(|n| n.to_string()).unwrap_or_default(),
            },
        };
        out.0.insert(input.id.clone(), text);
    }
    out
}

/// `PartStudioData` for a stored value: `buildFunction` (rebuilds the Part
/// Studio as a new Context), `configuration` and `configurationData` (live
/// dom/fs-refcfg), `partQuery` (the picked part, or everything) and
/// `referenceType`. `{}` when unset.
pub fn part_studio_value(value: &str, derived: Option<&dyn crate::regen::derived::DerivedProvider>) -> Value {
    let tag = super::host::std_tag("tabReferences.fs", "PartStudioData");
    let (value, chosen) = split_configuration(value);
    let (key, part) = split_part(value);
    let tab = key.as_str();
    let cfg = derived.and_then(|d| d.configuration(tab)).unwrap_or_default();
    let mut values = cfg.defaults();
    values.0.extend(chosen.0);
    let Some(ctx) = (!tab.is_empty()).then(|| derived.and_then(|d| d.context_configured(tab, &values))).flatten() else {
        return Value::map(Map::new()).with_tag(tag);
    };
    let query = |m: Vec<(&str, Value)>| Value::map_from(m).with_tag(super::host::std_tag("query.fs", "Query"));
    let qtype = |t: &str| super::host::std_enum("query.fs", "QueryType", t).unwrap_or_else(|| Value::str(t));
    // Live: the whole studio is UNION › UNION › everything (bodies).
    let inner = match part.as_deref() {
        // Onshape's persistent query of the part when we can say how it was made.
        Some(_) => Value::array(
            picked_parts(&ctx, part.as_deref())
                .iter()
                .map(|b| {
                    let body = ctx.bodies.iter().find(|x| &x.id == b);
                    let op = derived.and_then(|d| d.feature_type(tab, b.feature)).map(|t| op_name(&t));
                    body.zip(op).and_then(|(body, op)| swept_body_query(body, &op)).unwrap_or_else(|| super::host::query::transient_query(part_transient(tab, b)))
                })
                .collect(),
        ),
        None => {
            let body = super::host::std_enum("query.fs", "EntityType", "BODY").unwrap_or_else(|| Value::str("BODY"));
            Value::array(vec![query(vec![("queryType", qtype("UNION")), ("subqueries", Value::array(vec![query(vec![("entityType", body), ("queryType", qtype("EVERYTHING"))])]))])])
        }
    };
    let part_query = query(vec![("queryType", qtype("UNION")), ("subqueries", inner)]);
    let mut configuration = Map::new();
    let mut data = Map::new();
    for input in &cfg.inputs {
        let id = config_fs_id(input);
        let current = values.0.get(&input.id).cloned().unwrap_or_default();
        configuration.insert(Value::str(id.as_str()), config_value(input, &current));
        let default = cfg.defaults().0.get(&input.id).cloned().unwrap_or_default();
        let mut d = Map::new();
        d.insert(Value::str("defaultValue"), config_value(input, &default));
        if let crate::configuration::ConfigInputKind::List { options, .. } = &input.kind {
            let mut o = Map::new();
            for opt in options {
                o.insert(Value::str(opt.name.as_str()), Value::str(opt.name.as_str()));
            }
            d.insert(Value::str("options"), Value::map(o));
        }
        data.insert(Value::str(id.as_str()), Value::map(d));
    }
    let mut m = Map::new();
    if let Some(f) = noshape_featurescript::interp::native_function("buildPartStudio", vec![Value::str(tab)]) {
        m.insert(Value::str("buildFunction"), f);
    }
    m.insert(Value::str("configuration"), Value::map(configuration));
    m.insert(Value::str("configurationData"), Value::map(data));
    m.insert(Value::str("partQuery"), part_query);
    // The workspace of this document, or a version (std importDerived.fs ReferenceType).
    let reference_type = if tab.starts_with("ext:") || tab.contains('/') { "VERSION" } else { "WORKSPACE" };
    m.insert(Value::str("referenceType"), Value::str(reference_type));
    Value::map(m).with_tag(tag)
}

/// `buildFunction(configuration)`: a new Context holding the referenced
/// Part Studio's final state in that configuration (missing inputs at
/// their defaults; live: `{ "Len" : 30 mm }` builds a 30 mm box).
fn build_part_studio(i: &mut noshape_featurescript::interp::Interp, a: Vec<Value>) -> noshape_featurescript::interp::R<Value> {
    let tab = a.first().and_then(|v| v.as_str()).unwrap_or_default();
    let derived = super::host::derived();
    let cfg = derived.as_ref().and_then(|d| d.configuration(&tab)).unwrap_or_default();
    let values = config_from_fs(&cfg, a.get(1).unwrap_or(&Value::Undefined));
    let Some(ctx) = derived.and_then(|d| d.context_configured(&tab, &values)) else {
        return Err(i.error("The referenced Part Studio is not available"));
    };
    let c = super::host::FsContext::new((*ctx).clone(), crate::reference::FeatureId(0), String::new(), Default::default(), Default::default(), None);
    {
        let mut st = c.state.lock();
        st.loaded = true;
        for b in &ctx.bodies {
            st.name_transient(part_transient(&tab, &b.id), crate::reference::Reference::Body(b.id.clone()));
        }
        for sk in &ctx.sketches {
            st.name_transient(part_transient(&tab, &crate::reference::sketch_body_id(sk.feature)), crate::reference::Reference::Sketch(sk.feature));
        }
    }
    Ok(c.value())
}
