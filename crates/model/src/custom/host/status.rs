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
    ("setFeatureComputedParameter", noop),
    ("setFeatureHiddenParameters", noop),
    ("recordQuery", noop),
    ("setExternalDisambiguation", noop),
    ("skipOrderDisambiguation", noop),
    ("setErrorEntities", noop),
    ("setHighlightedEntities", noop),
    ("transferSubfeatureErrorDisplay", noop),
    ("setDimensionedEntities", noop),
    ("addManipulators", noop),
    ("addAuxiliaryEntities", noop),
    ("addReferenceCSysFrame", noop),
    ("setPatternData", noop),
    ("setFeaturePatternInstanceData", noop),
    ("unsetFeaturePatternInstanceData", noop),
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

/// Values of an id -> value map in feature order (ids sort like features).
fn values_sorted_by_id(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let m = noshape_featurescript::natives::map_arg(i, &a, 1)?;
    Ok(Value::array(m.values().cloned().collect()))
}

/// A FeatureList parameter value: picked feature Id -> feature function.
pub fn feature_list_value(refs: &[Reference]) -> Value {
    let mut m = Map::new();
    for f in refs.iter().filter_map(Reference::depends_on) {
        let id = id_value(&[std::sync::Arc::from(f.to_string())]);
        let func = noshape_featurescript::interp::native_function("featureListFunction", vec![id.clone()]).unwrap_or_default();
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

/// A picked feature's function: features are not re-run from a FeatureList here.
fn feature_list_function(i: &mut Interp, _a: Vec<Value>) -> R<Value> {
    Err(i.error("A picked feature cannot be re-run"))
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
    let status = a.get(2).cloned().unwrap_or_default();
    let mut st = cx.state.lock();
    let ok = status.get("statusType").as_str().is_some_and(|s| &*s == "OK");
    if ok {
        st.status.remove(&id);
    } else {
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
    let mut out: Vec<Value> = Vec::new();
    for (e, v) in &st.attributes {
        if !refs.contains(e) || (!name.is_undefined() && *v.get("name") != name) {
            continue;
        }
        let attr = v.get("attribute");
        if matches_pattern(attr, &pattern) && !out.contains(attr) {
            out.push(attr.clone());
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
