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
    put("dataId", Value::str(format!("{:024x}", h.finish() as u128 * 0x9e37_79b9)));
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

/// (width, height, media type) from PNG / GIF / JPEG headers.
fn image_size(b: &[u8]) -> (Option<u32>, Option<u32>, &'static str) {
    let be32 = |i: usize| b.get(i..i + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let be16 = |i: usize| b.get(i..i + 2).map(|s| u16::from_be_bytes([s[0], s[1]]) as u32);
    let le16 = |i: usize| b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as u32);
    if b.starts_with(b"\x89PNG") {
        return (be32(16), be32(20), "image/png");
    }
    if b.starts_with(b"GIF8") {
        return (le16(6), le16(8), "image/gif");
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        // Walk the segments to a start-of-frame marker.
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = b[i + 1];
            let len = be16(i + 2).unwrap_or(0) as usize;
            if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
                return (be16(i + 7), be16(i + 5), "image/jpeg");
            }
            i += 2 + len.max(1);
        }
        return (None, None, "image/jpeg");
    }
    (None, None, "application/octet-stream")
}

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
pub(super) const NATIVES: &[(&str, noshape_featurescript::interp::NativeFn)] = &[("buildPartStudio", build_part_studio)];

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

/// The parts a `PartStudioData` value stands for: the picked part, or every
/// solid of the Part Studio.
fn picked_parts(ctx: &crate::regen::context::Context, part: Option<&str>) -> Vec<crate::reference::BodyId> {
    ctx.bodies
        .iter()
        .filter(|b| match part {
            Some(p) => b.id.to_string() == p,
            None => matches!(b.kind, crate::display::BodyKind::Solid),
        })
        .map(|b| b.id.clone())
        .collect()
}

/// `PartStudioData` for `value` (`tab` or `tab#part`): `buildFunction`
/// (rebuilds the Part Studio as a new Context), `configuration`,
/// `configurationData` and `partQuery`. `{}` when unset.
pub fn part_studio_value(value: &str, derived: Option<&dyn crate::regen::derived::DerivedProvider>) -> Value {
    let tag = super::host::std_tag("tabReferences.fs", "PartStudioData");
    let (key, part) = split_part(value);
    let tab = key.as_str();
    let Some(ctx) = (!tab.is_empty()).then(|| derived.and_then(|d| d.context(tab))).flatten() else {
        return Value::map(Map::new()).with_tag(tag);
    };
    let ids: Vec<Value> = picked_parts(&ctx, part.as_deref()).iter().map(|b| super::host::query::transient_query(part_transient(tab, b))).collect();
    let union = super::host::std_enum("query.fs", "QueryType", "UNION").unwrap_or_else(|| Value::str("UNION"));
    let part_query = Value::map_from([("queryType", union), ("subqueries", Value::array(ids))]).with_tag(super::host::std_tag("query.fs", "Query"));
    let mut m = Map::new();
    if let Some(f) = noshape_featurescript::interp::native_function("buildPartStudio", vec![Value::str(tab)]) {
        m.insert(Value::str("buildFunction"), f);
    }
    m.insert(Value::str("configuration"), Value::map(Map::new()));
    m.insert(Value::str("configurationData"), Value::map(Map::new()));
    m.insert(Value::str("partQuery"), part_query);
    Value::map(m).with_tag(tag)
}

/// `buildFunction(configuration)`: a new Context holding the referenced
/// Part Studio's final state.
fn build_part_studio(i: &mut noshape_featurescript::interp::Interp, a: Vec<Value>) -> noshape_featurescript::interp::R<Value> {
    let tab = a.first().and_then(|v| v.as_str()).unwrap_or_default();
    let Some(ctx) = super::host::derived().and_then(|d| d.context(&tab)) else {
        return Err(i.error("The referenced Part Studio is not available"));
    };
    let c = super::host::FsContext::new((*ctx).clone(), crate::reference::FeatureId(0), String::new(), Default::default(), Default::default(), None);
    {
        let mut st = c.state.lock();
        for b in &ctx.bodies {
            st.name_transient(part_transient(&tab, &b.id), crate::reference::Reference::Body(b.id.clone()));
        }
    }
    Ok(c.value())
}
