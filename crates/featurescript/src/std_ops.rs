//! std's ValueWithUnits and Vector operators (units.fs, vector.fs) run here: the values their FeatureScript bodies
//! give, without a FeatureScript call per operation (a Displacement map's 262,144 points made millions, ~5 µs each).
//! `None` wherever a body could answer otherwise (units that differ: its precondition's error; a NaN; a tagged
//! number; a std before 2506, whose texts differ): the body runs as written.

use crate::ir::FnDecl;
use crate::value::{Map, Tag, UNIT_KEY, VALUE_KEY, Value};
use std::sync::{Arc, OnceLock};

/// The first shipped std whose operators read as these (units.fs from 1024, vector.fs from 2506).
pub const SINCE: u32 = 2506;

#[cfg(test)]
thread_local! {
    /// Tests: std's bodies run instead (this thread).
    pub static OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The value std's operator `decl` answers for `args`, when it is one of these.
pub fn native(decl: &FnDecl, args: &[Value]) -> Option<Value> {
    #[cfg(test)]
    if OFF.with(|o| o.get()) {
        return None;
    }
    let version = decl.compiled.get()?.version?;
    if version < SINCE {
        return None;
    }
    match (&*decl.module, &*decl.name, args) {
        ("onshape/std/units.fs", "isLength", [v]) => Some(Value::bool(is_length(v))),
        ("onshape/std/vector.fs", "isLengthVector", [v]) => Some(Value::bool(is_length_vector(v))),
        ("onshape/std/vector.fs", "is3dLengthVector", [v]) => Some(Value::bool(is_length_vector(v) && v.as_array().is_some_and(|a| a.len() == 3))),
        ("onshape/std/surfaceGeometry.fs", "canBeControlPointMatrix", [v]) => Some(Value::bool(control_point_matrix(v))),
        ("onshape/std/units.fs", ..) => units(&decl.name, args),
        ("onshape/std/vector.fs", ..) => vectors(decl, args),
        _ => None,
    }
}

fn tag(key: &'static str, cell: &'static OnceLock<Tag>) -> Option<Tag> {
    if let Some(t) = cell.get() {
        return Some(*t);
    }
    let t = crate::value::tag_of_key(key)?;
    Some(*cell.get_or_init(|| t))
}

/// `isLength`: a ValueWithUnits whose unit is LENGTH_UNITS (`{ "meter" : 1 } as UnitSpec`).
fn is_length(v: &Value) -> bool {
    static SPEC: OnceLock<Tag> = OnceLock::new();
    static METER: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| Value::str("meter"));
    let Value::Map(m, t) = v else { return false };
    if Some(*t) != vwu_tag() {
        return false;
    }
    let Some(Value::Map(unit, t)) = m.get(&UNIT_KEY) else { return false };
    Some(*t) == tag("onshape/std/units.fs#UnitSpec", &SPEC) && unit.len() == 1 && unit.get(&METER) == Some(&Value::num(1.0))
}

/// `isLengthVector`: a Vector of lengths.
fn is_length_vector(v: &Value) -> bool {
    static VECTOR: OnceLock<Tag> = OnceLock::new();
    let Value::Array(a, t) = v else { return false };
    Some(*t) == tag("onshape/std/vector.fs#Vector", &VECTOR) && a.iter().all(is_length)
}

/// `canBeControlPointMatrix`: rows (arrays) as long as the first, of 3D length vectors.
fn control_point_matrix(v: &Value) -> bool {
    let Value::Array(rows, _) = v else { return false };
    let width = rows.first().map(|r| r.as_array().map(|a| a.len()));
    for row in rows.iter() {
        let Some(r) = row.as_array() else { return false };
        if Some(Some(r.len())) != width || !r.iter().all(|x| is_length_vector(x) && x.as_array().is_some_and(|a| a.len() == 3)) {
            return false;
        }
    }
    true
}

fn vwu_tag() -> Option<Tag> {
    static TAG: OnceLock<Tag> = OnceLock::new();
    tag("onshape/std/units.fs#ValueWithUnits", &TAG)
}

/// A ValueWithUnits: its map, tag, value and unit.
struct Vwu<'a> {
    map: &'a Arc<Map>,
    tag: Tag,
    value: f64,
    unit: &'a Value,
}

fn vwu(v: &Value) -> Option<Vwu<'_>> {
    let Value::Map(map, tag) = v else { return None };
    if Some(*tag) != vwu_tag() {
        return None;
    }
    let Some(Value::Number(value, Tag::NONE)) = map.get(&VALUE_KEY) else { return None };
    let unit = map.get(&UNIT_KEY)?;
    Some(Vwu { map, tag: *tag, value: *value, unit })
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(x, Tag::NONE) => Some(*x),
        _ => None,
    }
}

fn finite(x: f64) -> Option<f64> {
    (!x.is_nan()).then_some(x)
}

/// `v` with `value` set (`lhs.value *= rhs; return lhs;`).
fn with_value(v: &Vwu, x: f64) -> Option<Value> {
    let mut m = (**v.map).clone();
    m.insert(VALUE_KEY.clone(), Value::num(finite(x)?));
    Some(Value::Map(Arc::new(m), v.tag))
}

/// A unit map's exponents, each a plain number.
fn exponents(u: &Value) -> Option<(&Arc<Map>, Tag)> {
    let Value::Map(m, t) = u else { return None };
    m.iter().all(|(k, e)| matches!(k, Value::Str(..)) && number(e).is_some()).then_some((m, *t))
}

/// `unitProduct` and operator*(ValueWithUnits, ValueWithUnits): a number when the units cancel.
fn product(tag: Tag, u1: &Value, u2: &Value, x: f64) -> Option<Value> {
    let ((m1, t1), (m2, _)) = (exponents(u1)?, exponents(u2)?);
    let mut unit = (**m1).clone();
    for (k, e) in m2.iter() {
        match m1.get(k) {
            None => {
                unit.insert(k.clone(), e.clone());
            }
            Some(a) => {
                let sum = number(a)? + number(e)?;
                if sum == 0.0 {
                    unit.remove(k);
                } else {
                    unit.insert(k.clone(), Value::num(sum));
                }
            }
        }
    }
    let x = finite(x)?;
    if unit.is_empty() {
        return Some(Value::num(x));
    }
    let mut m = Map::new();
    m.insert(UNIT_KEY.clone(), Value::Map(Arc::new(unit), t1));
    m.insert(VALUE_KEY.clone(), Value::num(x));
    Some(Value::Map(Arc::new(m), tag))
}

/// `reciprocal`: every exponent negated, the value inverted.
fn reciprocal(v: &Vwu) -> Option<(Value, f64)> {
    let (m, t) = exponents(v.unit)?;
    let unit: Map = m.iter().map(|(k, e)| Some((k.clone(), Value::num(finite(-number(e)?)?)))).collect::<Option<_>>()?;
    Some((Value::Map(Arc::new(unit), t), 1.0 / v.value))
}

fn units(name: &str, args: &[Value]) -> Option<Value> {
    match (name, args) {
        ("operator+" | "operator-", [a, b]) => {
            let (a, b) = (vwu(a)?, vwu(b)?);
            if a.unit != b.unit {
                return None;
            }
            with_value(&a, if name == "operator+" { a.value + b.value } else { a.value - b.value })
        }
        ("operator-", [a]) => {
            let a = vwu(a)?;
            with_value(&a, -a.value)
        }
        ("operator*", [a, b]) => match (vwu(a), vwu(b)) {
            (Some(a), Some(b)) => product(a.tag, a.unit, b.unit, a.value * b.value),
            (Some(a), None) => with_value(&a, a.value * number(b)?),
            (None, Some(b)) => with_value(&b, b.value * number(a)?),
            (None, None) => None,
        },
        ("operator/", [a, b]) => match (vwu(a), vwu(b)) {
            (Some(a), Some(b)) => {
                let (unit, r) = reciprocal(&b)?;
                product(a.tag, a.unit, &unit, a.value * r)
            }
            (Some(a), None) => with_value(&a, a.value / number(b)?),
            (None, Some(b)) => {
                let (unit, r) = reciprocal(&b)?;
                let mut m = (**b.map).clone();
                m.insert(UNIT_KEY.clone(), unit);
                m.insert(VALUE_KEY.clone(), Value::num(finite(r * number(a)?)?));
                Some(Value::Map(Arc::new(m), b.tag))
            }
            (None, None) => None,
        },
        ("operator<", [a, b]) => match (vwu(a), vwu(b)) {
            (Some(a), Some(b)) => (a.unit == b.unit).then(|| Value::bool(a.value < b.value)),
            (Some(a), None) => (number(b)? == 0.0).then(|| Value::bool(a.value < 0.0)),
            (None, Some(b)) => (number(a)? == 0.0).then(|| Value::bool(0.0 < b.value)),
            (None, None) => None,
        },
        _ => None,
    }
}

/// An element's operation inside a vector operator: plain numbers, or what units.fs answers.
fn element(name: &str, a: &Value, b: &Value) -> Option<Value> {
    if let (Some(x), Some(y)) = (number(a), number(b)) {
        return Some(Value::num(finite(match name {
            "operator+" => x + y,
            "operator-" => x - y,
            "operator*" => x * y,
            "operator/" => x / y,
            _ => return None,
        })?));
    }
    if vwu(a).is_none() && vwu(b).is_none() {
        return None;
    }
    units(name, &[a.clone(), b.clone()])
}

fn vectors(decl: &FnDecl, args: &[Value]) -> Option<Value> {
    let array = |v: &Value| match v {
        Value::Array(a, t) => Some((a.clone(), *t)),
        _ => None,
    };
    let collect = |items: Vec<Option<Value>>, tag: Tag| Some(Value::Array(Arc::new(items.into_iter().collect::<Option<Vec<_>>>()?), tag));
    let name = &*decl.name;
    match (name, args) {
        ("operator+" | "operator-", [a, b]) => {
            let ((a, ta), (b, _)) = (array(a)?, array(b)?);
            if a.len() != b.len() {
                return None;
            }
            collect(a.iter().zip(b.iter()).map(|(x, y)| element(name, x, y)).collect(), ta)
        }
        ("operator-", [a]) => {
            let (a, ta) = array(a)?;
            collect(a.iter().map(|x| number(x).map(|n| Value::num(-n)).or_else(|| units("operator-", std::slice::from_ref(x)))).collect(), ta)
        }
        ("operator*" | "operator/", [a, b]) => {
            // (vector, scalar) or (scalar, vector); not two vectors, nor a matrix.
            match (decl.param_types[0].is_some(), decl.param_types[1].is_some()) {
                (true, false) => {
                    let (v, t) = array(a)?;
                    collect(v.iter().map(|x| element(name, x, b)).collect(), t)
                }
                (false, true) if name == "operator*" => {
                    let (v, t) = array(b)?;
                    collect(v.iter().map(|x| element(name, a, x)).collect(), t)
                }
                _ => None,
            }
        }
        _ => None,
    }
}
