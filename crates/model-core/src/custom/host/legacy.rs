//! Natives as older standard libraries call them. Up to some release
//! between 1963 and 3095, evaluation builtins returned raw maps (numbers in
//! SI units, `curveType` / `surfaceType` strings, transient ids) and std
//! converted them (`planeFromBuiltin`, `qTransient`, `* meter`); now they
//! return std's types. A module of an older library (`FeatureScript 1963;`)
//! gets these wrappers, which turn the current result back into the raw form.

use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Tag, Value, enum_name, tag_name};
use std::sync::Arc;

/// The library version whose evaluation builtins return std's types.
const TYPED_SINCE: u32 = noshape_featurescript::STD_VERSION;

/// The current native `name`, its result in the raw form.
fn raw_call(name: &str, i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let Some(f) = noshape_featurescript::interp::native(name) else { return Err(i.error(format!("Builtin {name} not found"))) };
    let raw = to_builtin(&f(i, a)?);
    Ok(if name == "evDistance" { with_mesh_flags(raw) } else { raw })
}

/// evDistance's raw sides said whether they were mesh faces (std 1963 reads
/// `isMesh` before converting the parameter); ours never are.
fn with_mesh_flags(v: Value) -> Value {
    let Value::Map(m, tag) = v else { return v };
    let mut m = Arc::unwrap_or_clone(m);
    if let Some(Value::Array(sides, t)) = m.get(&Value::str("sides")).cloned() {
        let sides = sides
            .iter()
            .map(|side| match side {
                Value::Map(s, st) => {
                    let mut s = (**s).clone();
                    s.entry(Value::str("isMesh")).or_insert(Value::bool(false));
                    Value::Map(Arc::new(s), *st)
                }
                other => other.clone(),
            })
            .collect();
        m.insert(Value::str("sides"), Value::Array(Arc::new(sides), t));
    }
    Value::Map(Arc::new(m), tag)
}

/// A std value in the form builtins returned it: units dropped (SI
/// numbers), transient queries as their ids, enums as strings, std's types
/// as plain maps (curves with their `curveType`).
pub fn to_builtin(v: &Value) -> Value {
    match v {
        Value::Map(m, tag) => {
            let field = |k: &str| m.get(&Value::str(k));
            if let (Some(value), Some(Value::Map(..))) = (field("value"), field("unit")) {
                return to_builtin(value);
            }
            if let (Some(q), Some(id)) = (field("queryType"), field("transientId"))
                && q.as_str().as_deref() == Some("TRANSIENT")
            {
                return id.clone();
            }
            let mut out: Map = m.iter().map(|(k, x)| (k.clone(), to_builtin(x))).collect();
            let curve = match &*tag_name(*tag) {
                "Line" => Some("LINE"),
                "Circle" => Some("CIRCLE"),
                "Ellipse" => Some("ELLIPSE"),
                "BSplineCurve" => Some("SPLINE"),
                _ => None,
            };
            if let Some(c) = curve {
                out.entry(Value::str("curveType")).or_insert_with(|| Value::str(c));
            }
            Value::Map(Arc::new(out), Tag::NONE)
        }
        Value::Array(a, _) => Value::array(a.iter().map(to_builtin).collect()),
        Value::Enum(t, o) => Value::str(&*enum_name(*t, *o)),
        other => other.clone(),
    }
}

macro_rules! legacy {
    ($($name:literal),* $(,)?) => {
        &[$(($name, {
            fn raw(i: &mut Interp, a: Vec<Value>) -> R<Value> {
                raw_call($name, i, a)
            }
            raw as NativeFn
        })),*]
    };
}

/// The builtins std 1963 converts (its `@name` calls followed by a
/// `…FromBuiltin`, `* meter` or `qTransient`).
const RAW_RESULTS: &[(&str, NativeFn)] = legacy![
    "alignCanonically",
    "constructPaths",
    "evApproximateBSplineCurve",
    "evApproximateCentroid",
    "evArea",
    "evAxis",
    "evBox",
    "evCollisionDetection",
    "evCurveDefinition",
    "evDistance",
    "evEdgeCurvatures",
    "evEdgeTangentLines",
    "evFaceCurvatures",
    "evFaceTangentPlanes",
    "evFaceTangentPlanesAtEdge",
    "evFilletRadius",
    "evLength",
    "evLine",
    "evMateConnector",
    "evMateConnectorCoordSystem",
    "evOwnerSketchPlane",
    "evPlanarEdge",
    "evPlane",
    "evRaycast",
    "evSurfaceDefinition",
    "evVertexPoint",
    "evVolume",
    "evaluateQuery",
    "getFullPatternTransform",
    "getRemainderPatternTransform",
];

pub fn register() {
    noshape_featurescript::interp::register_legacy_natives(TYPED_SINCE, RAW_RESULTS);
}
