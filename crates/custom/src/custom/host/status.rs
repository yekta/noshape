//! Feature lifecycle and status, variables, attributes, debug output and
//! the other context natives that do no modeling.

use super::{FsContext, context_arg, convert::*, query};
use crate::reference::Reference;
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Value};
use noshape_units::{Dim, Quantity};

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("getTolerantParameterIds", get_tolerant_parameter_ids),
    ("getParameterToleranceInfo", get_parameter_tolerance_info),
    ("validateToleranceSchema", |_, _| Ok(Value::array(Vec::new()))),
    ("newContext", new_context),
    ("isContext", is_context),
    ("convert", convert),
    ("clampContextVersion", noop),
    ("startFeature", start_feature),
    ("endFeature", end_feature),
    ("abortFeature", abort_feature),
    ("functionReportFeatureStatus", report_status),
    ("functionGetFeatureStatus", get_status),
    ("clearFeatureStatus", clear_status),
    ("setFeatureComputedParameter", set_feature_computed_parameter),
    ("addManipulators", add_manipulators),
    ("setFeatureHiddenParameters", noop),
    ("recordQuery", noop),
    ("setExternalDisambiguation", noop),
    ("skipOrderDisambiguation", noop),
    ("setErrorEntities", noop),
    ("setHighlightedEntities", noop),
    ("transferSubfeatureErrorDisplay", noop),
    ("setDimensionedEntities", noop),
    ("addAuxiliaryEntities", noop),
    ("addReferenceCSysFrame", noop),
    ("setPatternData", noop),
    ("setFeaturePatternInstanceData", set_feature_pattern_instance_data),
    ("unsetFeaturePatternInstanceData", unset_feature_pattern_instance_data),
    ("addDebugEntities", add_debug_entities),
    ("isAtVersionOrLater", is_at_version_or_later),
    ("getCurrentVersion", get_current_version),
    ("isInSheetMetalFeature", false_native),
    ("isInFeaturePattern", false_native),
    ("getFeatureName", get_feature_name),
    ("getLastActiveId", get_last_active_id),
    ("lastOperationId", get_last_active_id),
    ("setVariable", set_variable),
    ("getVariable", get_variable),
    ("getAllVariables", get_all_variables),
    ("getAllVariablesAndDescriptions", get_all_variables_and_descriptions),
    ("setAttribute", set_attribute),
    ("getAttributes", get_attributes),
    ("getAttribute", get_attribute),
    ("getAllAttributes", get_all_attributes),
    ("removeAttributes", remove_attributes),
    ("valuesSortedById", values_sorted_by_id),
    ("containsSketch", contains_sketch),
    ("featureListFunction", feature_list_function),
    ("setProperty", set_property),
    ("getProperty", get_property),
    ("lastModifyingOperationId", last_modifying_operation_id),
];

/// Values of an id -> value map in the order their features started (std): picked Part Studio features (a
/// FeatureList's keys) in the feature list's order, then subfeatures in the order `startFeature` saw them; other keys
/// after, as the map has them.
fn values_sorted_by_id(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let m = noshape_featurescript::natives::map_arg(i, &a, 1)?;
    let st = cx.state.lock();
    let rank = |k: &Value| -> usize {
        let id: Option<super::FsId> = k.as_array().and_then(|x| x.iter().map(|s| s.as_str()).collect());
        let Some(id) = id else { return usize::MAX };
        let feature = match id.as_slice() {
            [f] => f.strip_prefix('F').and_then(|h| u64::from_str_radix(h, 16).ok()).and_then(|n| st.feature_defs.iter().position(|d| d.id.0 == n)),
            _ => None,
        };
        feature.or_else(|| st.start_order.iter().position(|s| *s == id).map(|p| st.feature_defs.len() + p)).unwrap_or(usize::MAX)
    };
    let mut entries: Vec<(usize, Value)> = m.iter().map(|(k, v)| (rank(k), v.clone())).collect();
    drop(st);
    entries.sort_by_key(|e| e.0);
    Ok(Value::array(entries.into_iter().map(|e| e.1).collect()))
}

/// A FeatureList parameter value: picked feature Id -> feature function (run in `cx`, when given).
pub fn feature_list_value(refs: &[Reference], cx: Option<&std::sync::Arc<super::FsContext>>) -> Value {
    let mut m = Map::new();
    for f in refs.iter().filter_map(Reference::depends_on) {
        let id = id_value(&[std::sync::Arc::from(f.to_string())]);
        let bound = match cx {
            Some(cx) => vec![cx.value(), id.clone()],
            None => vec![Value::Undefined, id.clone()],
        };
        let func = noshape_featurescript::interp::native_function("featureListFunction", bound).unwrap_or_default();
        m.insert(id, func);
    }
    Value::map(m).with_tag(super::std_tag("featureList.fs", "FeatureList"))
}

/// The picked features of a FeatureList value (editing logic results).
pub fn feature_list_refs(v: &Value) -> Vec<Reference> {
    let Value::Map(m, _) = v else { return Vec::new() };
    m.keys()
        .filter_map(|k| {
            let s = k.as_array()?.first()?.as_str()?;
            u64::from_str_radix(s.strip_prefix('F')?, 16).ok().map(|n| Reference::Feature(crate::reference::FeatureId(n)))
        })
        .collect()
}

/// A picked feature's function (bound to its id), called with an instance id: the feature run again under that id,
/// with the variables as they are now and the instance's transform (std applyPattern's Reapply).
fn feature_list_function(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let Ok(cx) = context_arg(i, &a) else { return Err(i.error("A picked feature cannot be re-run")) };
    let picked = super::convert::id_arg(i, &a, 1)?;
    let instance = super::convert::id_arg(i, &a, 2)?;
    let feature = picked.first().and_then(|s| u64::from_str_radix(s.strip_prefix('F')?, 16).ok()).map(crate::reference::FeatureId);
    let (def, m) = {
        let st = cx.state.lock();
        let def = feature.and_then(|f| st.feature_defs.iter().find(|g| g.id == f).cloned());
        (def, st.instance_transforms.get(&instance).copied())
    };
    let Some(def) = def else { return Err(i.error("A picked feature cannot be re-run")) };
    let params = crate::regen::eval::expand_query_variables(&def.params, &cx.state.lock().ctx).unwrap_or(def.params.clone());
    // Each picked feature under the instance's id (std calls them all with it), its own id appended.
    let mut id = instance;
    id.extend(picked);
    let original = super::run_op_with(i, &cx, &id, "PATTERN_FEATURE_FAILED", &params, |ev| crate::regen::features::pattern::rerun_instance(ev, def.id, &def.type_id, m))?;
    if let Some(s) = original {
        cx.state.lock().replaced_sketches.entry(def.id).or_insert(s);
    }
    Ok(Value::Undefined)
}

/// `@setFeaturePatternInstanceData(context, id, { transform })`: the instance's transform, for the features re-run
/// under `id`.
fn set_feature_pattern_instance_data(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = super::convert::id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    if let Some(m) = super::convert::transform_of(def.get("transform")) {
        cx.state.lock().instance_transforms.insert(id, m);
    }
    Ok(Value::Undefined)
}

/// `@unsetFeaturePatternInstanceData(context, id)`: the instance done; sketches its features solved again give way to
/// the originals.
fn unset_feature_pattern_instance_data(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = super::convert::id_arg(i, &a, 1)?;
    let mut st = cx.state.lock();
    st.instance_transforms.remove(&id);
    for (f, s) in std::mem::take(&mut st.replaced_sketches) {
        if let Some(slot) = st.ctx.sketches.iter_mut().find(|x| x.feature == f) {
            *slot = s;
        }
    }
    Ok(Value::Undefined)
}

/// Whether a feature list contains a sketch feature.
fn contains_sketch(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let m = noshape_featurescript::natives::map_arg(i, &a, 1)?;
    let st = cx.state.lock();
    let any = m.keys().any(|k| {
        let id: Vec<std::sync::Arc<str>> = k.as_array().map(|a| a.iter().filter_map(|c| c.as_str()).collect()).unwrap_or_default();
        let marker = super::marker_of(&id);
        st.ctx.sketches.iter().any(|s| s.feature == marker || id.first().is_some_and(|f| s.feature.to_string() == **f))
    });
    Ok(Value::bool(any))
}

/// Part properties: the name renames bodies; others are kept per entity.
fn set_property(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let refs = query::evaluate_refs(i, &cx, def.get("entities"))?;
    let kind = def.get("propertyType").as_str().unwrap_or_default();
    let value = def.get("value").clone();
    let mut st = cx.state.lock();
    for r in refs {
        if &*kind == "NAME"
            && let Reference::Body(id) = &r
            && let Some(name) = value.as_str()
            && let Some(b) = st.ctx.bodies.iter_mut().find(|b| &b.id == id)
        {
            *b = std::sync::Arc::new(b.renamed(name.to_string()));
            continue;
        }
        let key = Value::map_from([("property", Value::str(kind.clone())), ("custom", def.get("customPropertyId").clone())]);
        st.attributes.retain(|(e, v)| !(*e == r && *v.get("name") == key));
        st.push_attribute(r, Value::map_from([("name", key), ("attribute", value.clone())]));
    }
    Ok(Value::Undefined)
}

fn get_property(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let refs = query::evaluate_refs(i, &cx, def.get("entity"))?;
    let kind = def.get("propertyType").as_str().unwrap_or_default();
    let st = cx.state.lock();
    let Some(r) = refs.first() else { return Ok(Value::Undefined) };
    if &*kind == "NAME"
        && let Reference::Body(id) = r
        && let Some(b) = st.ctx.bodies.iter().find(|b| &b.id == id)
    {
        return Ok(Value::str(b.name.clone()));
    }
    let key = Value::map_from([("property", Value::str(kind)), ("custom", def.get("customPropertyId").clone())]);
    Ok(st.attributes.iter().find(|(e, v)| e == r && *v.get("name") == key).map(|(_, v)| v.get("attribute").clone()).unwrap_or_default())
}

fn last_modifying_operation_id(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let q = a.get(1).map(|d| d.get("entity").clone()).unwrap_or_default();
    let id = query::with_ents(i, &cx, &q, |_, qc, ents| Ok(ents.first().and_then(|e| qc.created_by(e))))?;
    match id {
        Some(id) => Ok(id_value(&id)),
        None => Err(super::raise_error(i, "CANNOT_RESOLVE_ENTITIES", None)),
    }
}

fn noop(_: &mut Interp, _: Vec<Value>) -> R<Value> {
    Ok(Value::Undefined)
}

fn false_native(_: &mut Interp, _: Vec<Value>) -> R<Value> {
    Ok(Value::bool(false))
}

fn new_context(_: &mut Interp, _: Vec<Value>) -> R<Value> {
    let c = FsContext::new(Default::default(), crate::reference::FeatureId(0), String::new(), Default::default(), Default::default(), None);
    Ok(Value::Builtin(c, noshape_featurescript::value::Tag::NONE))
}

fn is_context(_: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(Value::bool(a.first().and_then(|v| v.builtin::<FsContext>()).is_some()))
}

fn convert(_: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(a.into_iter().next().unwrap_or_default())
}

/// A status map: `{ statusType : "ERROR", statusEnum : "EXTRUDE_FAILED", statusMsg : ... }`.
pub fn status_map(kind: &str, enum_name: &str, msg: Option<&str>) -> Value {
    let mut entries = vec![("statusType", Value::str(kind)), ("statusEnum", Value::str(enum_name))];
    if let Some(m) = msg {
        entries.push(("statusMsg", Value::str(m)));
    }
    Value::map_from(entries)
}

fn start_feature(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let mut st = cx.state.lock();
    let snapshot = st.ctx.clone();
    if !st.start_order.contains(&id) {
        st.start_order.push(id.clone());
    }
    st.started.push(super::Started { id, snapshot });
    Ok(Value::map(Map::new()))
}

fn end_feature(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let mut st = cx.state.lock();
    if let Some(k) = st.started.iter().rposition(|s| s.id == id) {
        st.started.truncate(k);
    }
    Ok(Value::Undefined)
}

fn abort_feature(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let mut st = cx.state.lock();
    if let Some(k) = st.started.iter().rposition(|s| s.id == id) {
        let s = st.started.remove(k);
        st.started.truncate(k);
        st.ctx = s.snapshot;
    }
    Ok(Value::Undefined)
}

fn report_status(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let mut status = a.get(2).cloned().unwrap_or_default();
    let mut st = cx.state.lock();
    let ok = status.get("statusType").as_str().is_some_and(|s| &*s == "OK");
    if ok {
        st.status.remove(&id);
    } else {
        // Live "fs (10-25 e)": a report replaces the type and message but
        // faulty parameters accumulate, newest first (a bare HOLE_NO_HITS
        // thrown after opHole's mapped status keeps Merge scope red).
        let old: Vec<Value> = st.status.get(&id).and_then(|o| o.get("faultyParameters").as_array().map(|a| a.to_vec())).unwrap_or_default();
        if !old.is_empty()
            && let Value::Map(m, t) = &status
        {
            let mut faulty: Vec<Value> = status.get("faultyParameters").as_array().map(|a| a.to_vec()).unwrap_or_default();
            for f in old {
                if !faulty.contains(&f) {
                    faulty.push(f);
                }
            }
            let mut m = (**m).clone();
            m.insert(Value::str("faultyParameters"), Value::array(faulty));
            status = Value::Map(std::sync::Arc::new(m), *t);
        }
        st.status.insert(id, status);
    }
    Ok(Value::Undefined)
}

fn get_status(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let st = cx.state.lock();
    Ok(st.status.get(&id).cloned().unwrap_or_else(|| Value::map_from([("statusType", Value::str("OK"))])))
}

fn clear_status(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    cx.state.lock().status.remove(&id);
    Ok(Value::Undefined)
}

fn add_debug_entities(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let color = debug_color(def.get("color").as_str().as_deref().unwrap_or("RED"));
    let shapes = query::with_ents(i, &cx, def.get("entities"), |_, q, ents| Ok(ents.iter().filter_map(|e| q.shape(e)).collect::<Vec<_>>()))?;
    cx.state.lock().debug_entities.extend(shapes.into_iter().map(|s| (s, color)));
    Ok(Value::Undefined)
}

/// The client's colour of a `DebugColor` (live "fs (10-25 c)": ORANGE is a
/// pale yellow-green; anything else is red).
fn debug_color(name: &str) -> [u8; 3] {
    match name {
        "GREEN" => [0, 255, 0],
        "BLUE" => [0, 0, 255],
        "CYAN" => [0, 255, 255],
        "MAGENTA" => [255, 0, 255],
        "YELLOW" => [255, 255, 0],
        "BLACK" => [0, 0, 0],
        "ORANGE" => [242, 255, 128],
        _ => [255, 0, 0],
    }
}

fn version_ordinal(v: &Value) -> Option<(noshape_featurescript::value::Tag, u32)> {
    match v {
        Value::Enum(t, o) => Some((*t, *o)),
        _ => None,
    }
}

/// `as_version >= introduced`: by declaration order in one library, by the
/// number in the member's name (`V858_SM_FLAT_BUG_FIXES`) across library
/// versions (a Feature Studio pinned to std 1963 against the current one).
fn version_at_least(as_version: (noshape_featurescript::value::Tag, u32), introduced: (noshape_featurescript::value::Tag, u32)) -> bool {
    if as_version.0 == introduced.0 {
        return as_version.1 >= introduced.1;
    }
    let number = |(t, o): (noshape_featurescript::value::Tag, u32)| noshape_featurescript::value::enum_name(t, o).trim_start_matches('V').split('_').next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
    number(as_version) >= number(introduced)
}

fn is_at_version_or_later(_: &mut Interp, a: Vec<Value>) -> R<Value> {
    // The context is always at the current version; `asVersion` (a feature
    // saved with an older library) compares against that version instead.
    let Some(introduced) = a.get(1).and_then(version_ordinal) else { return Ok(Value::bool(true)) };
    match a.get(2).and_then(version_ordinal) {
        Some(as_version) => Ok(Value::bool(version_at_least(as_version, introduced))),
        None => Ok(Value::bool(true)),
    }
}

fn get_current_version(_: &mut Interp, _: Vec<Value>) -> R<Value> {
    let tag = super::std_tag("featurescriptversionnumber.gen.fs", "FeatureScriptVersionNumber");
    if tag.is_none() {
        return Ok(Value::Undefined);
    }
    let n = noshape_featurescript::value::type_info(tag).members.len();
    Ok(Value::Enum(tag, n.saturating_sub(1) as u32))
}

fn get_feature_name(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    Ok(Value::str(cx.state.lock().feature_name.clone()))
}

fn get_last_active_id(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let st = cx.state.lock();
    let last = st.ops.values().max_by_key(|o| o.seq).map(|o| o.id.clone()).unwrap_or_default();
    Ok(id_value(&last))
}

// ---------------------------------------------------------------- variables

/// A FeatureScript value as a variable quantity (lengths, angles, numbers).
pub(crate) fn quantity_of(v: &Value) -> Option<Quantity> {
    match v {
        Value::Number(n, _) => Some(Quantity::new(*n, Dim::UNITLESS)),
        Value::Map(..) => {
            if let Some(x) = length_of(v) {
                Some(Quantity::new(x, Dim::LENGTH))
            } else {
                angle_of(v).map(|x| Quantity::new(x, Dim::ANGLE))
            }
        }
        _ => None,
    }
}

fn quantity_value(q: &Quantity) -> Value {
    if q.dim == Dim::LENGTH {
        length_value(q.value)
    } else if q.dim == Dim::ANGLE {
        angle_value(q.value)
    } else {
        Value::num(q.value)
    }
}

/// `@addManipulators(context, id, { key: Manipulator })` (std manipulator.fs):
/// the feature's manipulators, kept by key (a later call with a key replaces
/// it). Transform's and Routing curve's triads stay native; std's triadManipulator (Edit curve) is a Triad.
fn add_manipulators(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    use crate::display::FeatureManipulator as M;
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    let Some(map) = def.as_map() else { return Ok(Value::Undefined) };
    let member = |v: &Value| match v {
        Value::Enum(t, o) => noshape_featurescript::value::type_info(*t).members.get(*o as usize).map(|m| m.to_string()),
        _ => None,
    };
    let mut out = Vec::new();
    for (k, m) in map.iter() {
        let (Some(key), Some(kind)) = (k.as_str(), member(m.get("manipulatorType"))) else { continue };
        let secondary = member(m.get("style")).as_deref() == Some("SECONDARY");
        let primary = m.get("primaryParameterId").as_str().map(|s| s.to_string());
        let manip = match kind.as_str() {
            "LINEAR_1D" => (|| {
                Some(M::Linear {
                    base: vec3_of(m.get("base"))?,
                    direction: vec3_of(m.get("direction"))?.normalize_or_zero(),
                    offset: length_of(m.get("offset"))?,
                    min: length_of(m.get("minValue")).unwrap_or(-500.0),
                    max: length_of(m.get("maxValue")).unwrap_or(500.0),
                    secondary,
                    primary: primary.clone(),
                })
            })(),
            "ANGULAR" => (|| {
                Some(M::Angular {
                    axis_origin: vec3_of(m.get("axisOrigin"))?,
                    axis_direction: vec3_of(m.get("axisDirection"))?.normalize_or_zero(),
                    rotation_origin: vec3_of(m.get("rotationOrigin"))?,
                    angle: angle_of(m.get("angle"))?,
                    min: angle_of(m.get("minValue")).unwrap_or(-std::f64::consts::PI),
                    max: angle_of(m.get("maxValue")).unwrap_or(std::f64::consts::PI),
                    secondary,
                    primary: primary.clone(),
                })
            })(),
            "FLIP" => (|| Some(M::Flip { base: vec3_of(m.get("base"))?, direction: vec3_of(m.get("direction"))?.normalize_or_zero(), flipped: m.get("flipped").as_bool()?, other_direction: vec3_of(m.get("otherDirection")).map(|d| d.normalize_or_zero()), secondary }))(),
            "LINEAR_3D" => (|| Some(M::Triad { base: vec3_of(m.get("base"))?, offset: vec3_of(m.get("offset"))? }))(),
            "POINTS" => (|| Some(M::Points { points: m.get("points").as_array()?.iter().filter_map(vec3_of).collect(), index: m.get("index").as_number()? as usize }))(),
            _ => None,
        };
        if let Some(manip) = manip {
            out.push((key.to_string(), manip));
        }
    }
    let mut st = cx.state.lock();
    for (k, v) in out {
        st.manipulators.retain(|(x, _)| *x != k);
        st.manipulators.push((k, v));
    }
    Ok(Value::Undefined)
}

/// A manipulator as FeatureScript's `Manipulator` (what `addManipulators`
/// took, with its current offset / angle / flip / index): the
/// `newManipulators` entry a manipulator change function reads.
pub fn manipulator_value(m: &crate::display::FeatureManipulator) -> Option<Value> {
    use crate::display::FeatureManipulator as M;
    let ty = |member: &str| super::std_enum("manipulatortype.gen.fs", "ManipulatorType", member);
    let style = |secondary: bool| super::std_enum("manipulatorstyleenum.gen.fs", "ManipulatorStyleEnum", if secondary { "SECONDARY" } else { "DEFAULT" });
    let mut out = Map::new();
    let mut put = |k: &str, v: Value| {
        out.insert(Value::str(k), v);
    };
    match m {
        M::Linear { base, direction, offset, min, max, secondary, primary } => {
            put("manipulatorType", ty("LINEAR_1D")?);
            put("base", point_value(*base));
            put("direction", dir_value(*direction));
            put("offset", length_value(*offset));
            put("minValue", length_value(*min));
            put("maxValue", length_value(*max));
            put("style", style(*secondary)?);
            if let Some(p) = primary {
                put("primaryParameterId", Value::str(p.as_str()));
            }
        }
        M::Angular { axis_origin, axis_direction, rotation_origin, angle, min, max, secondary, primary } => {
            put("manipulatorType", ty("ANGULAR")?);
            put("axisOrigin", point_value(*axis_origin));
            put("axisDirection", dir_value(*axis_direction));
            put("rotationOrigin", point_value(*rotation_origin));
            put("angle", angle_value(*angle));
            put("minValue", angle_value(*min));
            put("maxValue", angle_value(*max));
            put("style", style(*secondary)?);
            if let Some(p) = primary {
                put("primaryParameterId", Value::str(p.as_str()));
            }
        }
        M::Flip { base, direction, flipped, other_direction, secondary } => {
            put("manipulatorType", ty("FLIP")?);
            put("base", point_value(*base));
            put("direction", dir_value(*direction));
            put("flipped", Value::bool(*flipped));
            if let Some(d) = other_direction {
                put("otherDirection", dir_value(*d));
            }
            put("style", style(*secondary)?);
        }
        M::Triad { base, offset } => {
            put("manipulatorType", ty("LINEAR_3D")?);
            put("base", point_value(*base));
            put("offset", point_value(*offset));
        }
        M::Points { points, index } => {
            put("manipulatorType", ty("POINTS")?);
            put("points", Value::array(points.iter().map(|p| point_value(*p)).collect()));
            put("index", Value::num(*index as f64));
        }
    }
    Some(Value::Map(std::sync::Arc::new(out), super::std_tag("manipulator.fs", "Manipulator")))
}

/// `setFeatureComputedParameter(context, id, { name, value })`: a value the
/// dialog shows (lengths as lengths, other quantities in the units, text).
fn set_feature_computed_parameter(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    let Some(name) = def.get("name").as_str() else { return Ok(Value::Undefined) };
    let value = def.get("value");
    let mut st = cx.state.lock();
    let shown = match quantity_of(value) {
        Some(q) if q.dim == noshape_units::Dim::LENGTH => crate::display::ComputedValue::Length(q.value),
        Some(q) => crate::display::ComputedValue::Text(noshape_units::format_quantity(q, &st.units)),
        None => match value.as_str() {
            Some(s) => crate::display::ComputedValue::Text(s.to_string()),
            None if value.as_bool().is_some() => crate::display::ComputedValue::Text(value.as_bool().unwrap_or_default().to_string()),
            None => return Ok(Value::Undefined),
        },
    };
    st.computed.retain(|(k, _)| **k != *name);
    st.computed.push((name.to_string(), shown));
    Ok(Value::Undefined)
}

fn set_variable(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let Some(name) = def.get("name").as_str() else { return Err(i.error("setVariable: name should be a string")) };
    let value = def.get("value").clone();
    let mut st = cx.state.lock();
    match quantity_of(&value) {
        Some(q) => {
            st.ctx.variables.insert(name.to_string(), q);
            st.variables.remove(&*name);
        }
        None => {
            st.ctx.variables.remove(&*name);
            st.variables.insert(name.to_string(), value);
        }
    }
    Ok(Value::Undefined)
}

fn get_variable(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let name = def.get("name").as_str().unwrap_or_default();
    let st = cx.state.lock();
    if let Some(v) = st.variables.get(&*name) {
        return Ok(v.clone());
    }
    if let Some(q) = st.ctx.variables.get(&*name).or_else(|| st.outer_vars.get(&*name)) {
        return Ok(quantity_value(q));
    }
    match def.get("defaultValue") {
        Value::Undefined => {
            drop(st);
            Err(super::raise_error(i, "VARIABLE_CANNOT_RESOLVE", None))
        }
        d => Ok(d.clone()),
    }
}

fn all_variables(cx: &FsContext, include_config: bool, descriptions: bool) -> Value {
    let st = cx.state.lock();
    let mut m = Map::new();
    let wrap = |v: Value| if descriptions { Value::map_from([("value", v), ("description", Value::str(""))]) } else { v };
    if include_config {
        for (k, q) in &st.outer_vars {
            m.insert(Value::str(k.as_str()), wrap(quantity_value(q)));
        }
    }
    for (k, q) in &st.ctx.variables {
        m.insert(Value::str(k.as_str()), wrap(quantity_value(q)));
    }
    for (k, v) in &st.variables {
        m.insert(Value::str(k.as_str()), wrap(v.clone()));
    }
    Value::map(m)
}

fn get_all_variables(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let include = a.get(1).map(|d| d.get("includeConfiguration").as_bool().unwrap_or(false)).unwrap_or(false);
    Ok(all_variables(&cx, include, false))
}

fn get_all_variables_and_descriptions(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let include = a.get(1).map(|d| d.get("includeConfiguration").as_bool().unwrap_or(false)).unwrap_or(false);
    Ok(all_variables(&cx, include, true))
}

// ---------------------------------------------------------------- attributes

/// Attribute entries are (entity, `{ name?, attribute }`).
fn set_attribute(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let refs = query::evaluate_refs(i, &cx, def.get("entities"))?;
    let name = def.get("name").clone();
    let attribute = def.get("attribute").clone();
    let mut st = cx.state.lock();
    for r in refs {
        if !name.is_undefined() {
            st.attributes.retain(|(e, v)| !(*e == r && *v.get("name") == name));
        }
        st.push_attribute(r, Value::map_from([("name", name.clone()), ("attribute", attribute.clone())]));
    }
    Ok(Value::Undefined)
}

/// Whether `attribute` matches `pattern`: same type tag, and for maps every
/// key of the pattern with an equal value.
fn matches_pattern(attribute: &Value, pattern: &Value) -> bool {
    if pattern.is_undefined() {
        return true;
    }
    if attribute.tag() != pattern.tag() || attribute.std_type() != pattern.std_type() {
        return false;
    }
    match (attribute, pattern) {
        (Value::Map(am, _), Value::Map(pm, _)) => pm.iter().all(|(k, v)| am.get(k) == Some(v)),
        _ => true,
    }
}

fn get_attributes(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let refs = query::evaluate_refs(i, &cx, def.get("entities"))?;
    let pattern = def.get("attributePattern").clone();
    let name = def.get("name").clone();
    let st = cx.state.lock();
    // In the entities' order (Onshape: a Flange's walls are numbered in the
    // order its edges were picked, which Butt joint - Direction 1 follows).
    let mut by_entity: std::collections::HashMap<&Reference, Vec<&Value>> = std::collections::HashMap::new();
    for (e, v) in &st.attributes {
        by_entity.entry(e).or_default().push(v);
    }
    let mut out: Vec<Value> = Vec::new();
    for e in &refs {
        for v in by_entity.get(e).into_iter().flatten() {
            if !name.is_undefined() && *v.get("name") != name {
                continue;
            }
            let attr = v.get("attribute");
            if matches_pattern(attr, &pattern) && !out.contains(attr) {
                out.push(attr.clone());
            }
        }
    }
    Ok(Value::array(out))
}

fn get_attribute(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let refs = query::evaluate_refs(i, &cx, def.get("entity"))?;
    let name = def.get("name").clone();
    let st = cx.state.lock();
    let Some(first) = refs.first() else { return Ok(Value::Undefined) };
    Ok(st.attributes.iter().find(|(e, v)| e == first && *v.get("name") == name).map(|(_, v)| v.get("attribute").clone()).unwrap_or_default())
}

fn get_all_attributes(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let refs = query::evaluate_refs(i, &cx, def.get("entity"))?;
    let st = cx.state.lock();
    let mut m = Map::new();
    if let Some(first) = refs.first() {
        for (e, v) in &st.attributes {
            if e == first
                && let Some(n) = v.get("name").as_str()
            {
                m.insert(Value::str(n), v.get("attribute").clone());
            }
        }
    }
    Ok(Value::map(m))
}

fn remove_attributes(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let def = def_arg(i, &a, 1)?;
    let refs: Option<Vec<Reference>> = match def.get("entities") {
        Value::Undefined => None,
        q => Some(query::evaluate_refs(i, &cx, q)?),
    };
    let pattern = def.get("attributePattern").clone();
    let name = def.get("name").clone();
    let mut st = cx.state.lock();
    st.attributes.retain(|(e, v)| {
        let hit = refs.as_ref().is_none_or(|r| r.contains(e)) && (name.is_undefined() || *v.get("name") == name) && matches_pattern(v.get("attribute"), &pattern);
        !hit
    });
    Ok(Value::Undefined)
}

/// Tolerant quantity parameters of the running feature with a tolerance set.
/// Feature parameter values carry no tolerances yet, so there are none.
fn get_tolerant_parameter_ids(_: &mut Interp, _: Vec<Value>) -> R<Value> {
    Ok(Value::map(Map::new()))
}

/// The tolerance of a parameter: `ToleranceType.NONE` (see above).
fn get_parameter_tolerance_info(_: &mut Interp, _: Vec<Value>) -> R<Value> {
    let none = super::std_enum("toleranceTypes.fs", "ToleranceType", "NONE").unwrap_or_else(|| Value::str("NONE"));
    Ok(Value::map_from([("toleranceType", none)]))
}
