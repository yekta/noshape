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
