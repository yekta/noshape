//! Stable string ids for [`Reference`]s, so agents can select geometry by id.
//!
//! Simple references read naturally (`plane:top`, `origin`, `sketch:F1a2b`,
//! `entity:F1a2b:7`, `region:F1a2b:0,15`, `body:F1a2b.0`, `feature:F1a2b`,
//! `mc:F1a2b.0`, `fplane:F1a2b`, `qvar:name`). Faces, edges and vertices are
//! named by their persistent topological name (see `noshape_model::reference`),
//! which is a tree; their ids are `face:`, `edge:` or `vertex:` followed by the
//! name's JSON, deflated and base64url-encoded. Ids are self-contained: they
//! stay valid across processes and regenerations exactly as long as the UI's
//! own selections do. A reference may also be given as its JSON form.

use crate::error::OpError;
use noshape_model::reference::{BodyId, DatumPlane, FeatureId, Inference, Reference, TopoName};
use serde_json::Value;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for ch in s.bytes() {
        let v = B64.iter().position(|b| *b == ch)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xFF) as u8);
        }
    }
    Some(out)
}

fn topo_id(prefix: &str, n: &TopoName) -> String {
    let json = serde_json::to_vec(n).expect("serialize name");
    format!("{prefix}:{}", b64_encode(&miniz_oxide::deflate::compress_to_vec(&json, 9)))
}

fn topo_parse(s: &str) -> Option<TopoName> {
    let raw = miniz_oxide::inflate::decompress_to_vec(&b64_decode(s)?).ok()?;
    serde_json::from_slice(&raw).ok()
}

pub fn feature_id(f: FeatureId) -> String {
    f.to_string()
}

pub fn parse_feature_id(s: &str) -> Option<FeatureId> {
    let h = s.strip_prefix('F').or_else(|| s.strip_prefix("feature:F"))?;
    u64::from_str_radix(h, 16).ok().map(FeatureId)
}

pub fn body_id(b: &BodyId) -> String {
    format!("body:{}.{}", b.feature, b.index)
}

fn parse_dotted(s: &str) -> Option<(FeatureId, u32)> {
    let (f, i) = s.split_once('.')?;
    Some((parse_feature_id(f)?, i.parse().ok()?))
}

fn inference_name(i: Inference) -> &'static str {
    match i {
        Inference::Centroid => "centroid",
        Inference::Center => "center",
        Inference::Midpoint => "midpoint",
        Inference::Vertex => "vertex",
        Inference::TopAxis => "top_axis",
        Inference::MidAxis => "mid_axis",
        Inference::BottomAxis => "bottom_axis",
        Inference::Origin => "origin",
    }
}

pub const INFERENCES: [Inference; 8] = [Inference::Centroid, Inference::Center, Inference::Midpoint, Inference::Vertex, Inference::TopAxis, Inference::MidAxis, Inference::BottomAxis, Inference::Origin];

/// The id of a reference.
pub fn to_id(r: &Reference) -> String {
    match r {
        Reference::Origin => "origin".into(),
        Reference::Plane(DatumPlane::Top) => "plane:top".into(),
        Reference::Plane(DatumPlane::Front) => "plane:front".into(),
        Reference::Plane(DatumPlane::Right) => "plane:right".into(),
        Reference::Face(n) => topo_id("face", n),
        Reference::Edge(n) => topo_id("edge", n),
        Reference::Vertex(n) => topo_id("vertex", n),
        Reference::Body(b) => body_id(b),
        Reference::SketchEntity { sketch, entity } => format!("entity:{sketch}:{entity}"),
        Reference::SketchRegion { sketch, key } => format!("region:{sketch}:{}", key.iter().map(u32::to_string).collect::<Vec<_>>().join(",")),
        Reference::Sketch(f) => format!("sketch:{f}"),
        Reference::FeaturePlane(f) => format!("fplane:{f}"),
        Reference::MateConnector { feature, index } => format!("mc:{feature}.{index}"),
        Reference::Feature(f) => format!("feature:{f}"),
        Reference::Implicit { on, inference } => format!("implicit:{}:{}", inference_name(*inference), to_id(on)),
        Reference::QueryVariable(n) => format!("qvar:{n}"),
    }
}

/// Parse a reference id (or its JSON form).
pub fn parse(v: &Value) -> Result<Reference, OpError> {
    match v {
        Value::String(s) => parse_str(s).or_else(|| serde_json::from_value(v.clone()).ok()).ok_or_else(|| OpError::invalid(format!("not a valid reference id: `{s}` (ids come from the query tools, e.g. `plane:top`, `face:…`, `region:…`)"))),
        Value::Object(_) => serde_json::from_value(v.clone()).map_err(|e| OpError::invalid(format!("not a valid reference object: {e}"))),
        _ => Err(OpError::invalid("a reference is an id string")),
    }
}

pub fn parse_str(s: &str) -> Option<Reference> {
    let s = s.trim();
    let (kind, rest) = s.split_once(':').unwrap_or((s, ""));
    Some(match kind {
        "origin" => Reference::Origin,
        "plane" => Reference::Plane(match rest.to_ascii_lowercase().as_str() {
            "top" => DatumPlane::Top,
            "front" => DatumPlane::Front,
            "right" => DatumPlane::Right,
            _ => return None,
        }),
        "face" => Reference::Face(topo_parse(rest)?),
        "edge" => Reference::Edge(topo_parse(rest)?),
        "vertex" => Reference::Vertex(topo_parse(rest)?),
        "body" => {
            let (feature, index) = parse_dotted(rest)?;
            Reference::Body(BodyId { feature, index })
        }
        "entity" => {
            let (f, e) = rest.split_once(':')?;
            Reference::SketchEntity { sketch: parse_feature_id(f)?, entity: e.parse().ok()? }
        }
        "region" => {
            let (f, k) = rest.split_once(':')?;
            let key = k.split(',').map(|x| x.trim().parse().ok()).collect::<Option<Vec<u32>>>()?;
            Reference::SketchRegion { sketch: parse_feature_id(f)?, key }
        }
        "sketch" => Reference::Sketch(parse_feature_id(rest)?),
        "fplane" => Reference::FeaturePlane(parse_feature_id(rest)?),
        "mc" => {
            let (feature, index) = parse_dotted(rest)?;
            Reference::MateConnector { feature, index }
        }
        "feature" => Reference::Feature(parse_feature_id(rest)?),
        "implicit" => {
            let (inf, on) = rest.split_once(':')?;
            let inference = *INFERENCES.iter().find(|i| inference_name(**i) == inf)?;
            Reference::Implicit { on: Box::new(parse_str(on)?), inference }
        }
        "qvar" => Reference::QueryVariable(rest.to_string()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_model::reference::{Role, Source};
    use std::sync::Arc;

    #[test]
    fn ids_round_trip() {
        let f = FeatureId(0x1a2b);
        let face = Arc::new(TopoName::face(f, Role::Side(Source::Sketch { sketch: FeatureId(3), entity: 4, segment: 0 })));
        let cap = Arc::new(TopoName::face(f, Role::CapEnd(None)));
        let refs = [
            Reference::Origin,
            Reference::Plane(DatumPlane::Front),
            Reference::Face((*face).clone()),
            Reference::Edge(TopoName::edge(face.clone(), cap.clone(), 1)),
            Reference::Vertex(TopoName::vertex(vec![face, cap], 0)),
            Reference::Body(BodyId { feature: f, index: 2 }),
            Reference::SketchEntity { sketch: f, entity: 9 },
            Reference::SketchRegion { sketch: f, key: vec![0, 15, 22] },
            Reference::Sketch(f),
            Reference::FeaturePlane(f),
            Reference::MateConnector { feature: f, index: 1 },
            Reference::Feature(f),
            Reference::Implicit { on: Box::new(Reference::SketchEntity { sketch: f, entity: 3 }), inference: Inference::Center },
            Reference::QueryVariable("holes".into()),
        ];
        for r in refs {
            let id = to_id(&r);
            assert_eq!(parse_str(&id).as_ref(), Some(&r), "{id}");
            // JSON form is accepted too.
            assert_eq!(parse(&serde_json::to_value(&r).unwrap()).unwrap(), r);
        }
        assert_eq!(to_id(&Reference::Sketch(f)), "sketch:F1a2b");
        assert!(parse_str("face:!!").is_none());
    }
}
