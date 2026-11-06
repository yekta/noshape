//! Natives as older standard libraries call them. Up to some release
//! between 1963 and 3095, evaluation builtins returned raw maps (numbers in
//! SI units, `curveType` / `surfaceType` strings, transient ids) and std
//! converted them (`planeFromBuiltin`, `qTransient`, `* meter`); now they
//! return std's types. A module of an older library (`FeatureScript 1963;`)
//! gets these wrappers, which turn the current result back into the raw form.

use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Tag, Value, enum_name, tag_name};
use std::sync::Arc;

/// The first library release whose evaluation builtins return std's types: 2455's evaluate.fs still converts
/// (`lineFromBuiltin(@evAxis(…))`), 2473's doesn't (the test `releases_up_to_typed_since_convert_raw_results` checks
/// every shipped release against this).
pub const TYPED_SINCE: u32 = 2473;

/// The first release whose `evaluateQuery` takes `@evaluateQuery`'s queries as they are: up to 2559 its query.fs wraps
/// each transient id itself (`{ "queryType" : TRANSIENT, "transientId" : id }`), so it needs the ids (a std 2506 studio
/// got queries of queries, which evaluate to nothing: cf-fix-pcb's "No components selected"). The test
/// `releases_up_to_queries_since_wrap_transient_ids` checks every shipped release against this.
pub const QUERIES_SINCE: u32 = 2581;

/// The current native `name`, its result in the raw form.
fn raw_call(name: &str, i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let Some(f) = noshape_featurescript::interp::native(name) else { return Err(i.error(format!("Builtin {name} not found"))) };
    // (Its result in maps and arrays first: a plane's compact origin is units dropped like a map's.)
    let raw = to_builtin(&f(i, a)?.materialized());
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
                    if !s.contains_key(&Value::str("isMesh")) {
                        s.insert(Value::str("isMesh"), Value::bool(false));
                    }
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
            if let Some(c) = curve
                && !out.contains_key(&Value::str("curveType"))
            {
                out.insert(Value::str("curveType"), Value::str(c));
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
    "getFullPatternTransform",
    "getRemainderPatternTransform",
];

/// The query builtins older releases wrap themselves (see [`QUERIES_SINCE`]).
const RAW_QUERY_RESULTS: &[(&str, NativeFn)] = legacy!["evaluateQuery"];

pub fn register() {
    noshape_featurescript::interp::register_legacy_natives(TYPED_SINCE, RAW_RESULTS);
    noshape_featurescript::interp::register_legacy_natives(QUERIES_SINCE, RAW_QUERY_RESULTS);
}

#[cfg(test)]
mod tests {
    /// A shipped release converts raw evaluation results itself (its evaluate.fs wraps `@evAxis` in
    /// `lineFromBuiltin`) exactly when it is older than [`super::TYPED_SINCE`].
    #[test]
    fn releases_up_to_typed_since_convert_raw_results() {
        let wrong: Vec<u32> = noshape_featurescript::std_versions()
            .into_iter()
            .filter(|&v| {
                let converts = noshape_featurescript::std_source_at(v, "evaluate.fs").is_some_and(|s| s.contains("lineFromBuiltin(@evAxis"));
                converts != (v < super::TYPED_SINCE)
            })
            .collect();
        assert!(wrong.is_empty(), "releases on the wrong side of TYPED_SINCE ({}): {wrong:?}", super::TYPED_SINCE);
    }

    /// A shipped release (from 1431; older ones word it otherwise) wraps `@evaluateQuery`'s transient ids in queries itself
    /// exactly when it is older than [`super::QUERIES_SINCE`].
    #[test]
    fn releases_up_to_queries_since_wrap_transient_ids() {
        let wrong: Vec<u32> = noshape_featurescript::std_versions()
            .into_iter()
            .filter(|&v| v >= 1431)
            .filter(|&v| {
                let wraps = noshape_featurescript::std_source_at(v, "query.fs").is_some_and(|s| s.contains("for (var transientId in result)"));
                wraps != (v < super::QUERIES_SINCE)
            })
            .collect();
        assert!(wrong.is_empty(), "releases on the wrong side of QUERIES_SINCE ({}): {wrong:?}", super::QUERIES_SINCE);
    }
}
