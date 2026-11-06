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

/// Which of these `decl` is (found once per resolved call: the interpreter keeps it with the resolution).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    IsLength,
    IsLengthVector,
    Is3dLengthVector,
    ControlPointMatrix,
    /// `vector(x, y)`, `vector(x, y, z)`: `[x, y] as Vector`.
    NewVector,
    Units,
    Vectors,
}

/// The operator of std's `decl` these answer, if any.
pub fn classify(decl: &FnDecl) -> Option<Op> {
    let version = decl.compiled.get()?.version?;
    if version < SINCE {
        return None;
    }
    Some(match (&*decl.module, &*decl.name) {
        ("onshape/std/units.fs", "isLength") => Op::IsLength,
        ("onshape/std/vector.fs", "isLengthVector") => Op::IsLengthVector,
        ("onshape/std/vector.fs", "is3dLengthVector") => Op::Is3dLengthVector,
        ("onshape/std/surfaceGeometry.fs", "canBeControlPointMatrix") => Op::ControlPointMatrix,
        ("onshape/std/vector.fs", "vector") if matches!(decl.arity(), 2 | 3) && decl.param_types.iter().all(Option::is_none) => Op::NewVector,
        ("onshape/std/units.fs", _) => Op::Units,
        ("onshape/std/vector.fs", _) => Op::Vectors,
        _ => return None,
    })
}

/// The value std's operator `decl` (classified `op`) answers for `args`, when it is one of these.
/// `args` are taken only when answered (an operand's map or array then reused when nothing else holds it).
pub fn apply(op: Op, decl: &FnDecl, args: &mut [Value]) -> Option<Value> {
    #[cfg(test)]
    if OFF.with(|o| o.get()) {
        return None;
    }
    match (op, &*args) {
        (Op::IsLength, [v]) => Some(Value::bool(is_length(v))),
        (Op::IsLengthVector, [v]) => Some(Value::bool(is_length_vector(v))),
        (Op::Is3dLengthVector, [v]) => Some(Value::bool(is_length_vector(v) && v.as_array().is_some_and(|a| a.len() == 3))),
        (Op::ControlPointMatrix, [v]) => Some(Value::bool(control_point_matrix(v))),
        (Op::NewVector, _) => {
            let tag = vector_tag()?;
            Some(Value::Array(Arc::new(args.iter_mut().map(std::mem::take).collect()), tag))
        }
        (Op::Units, _) => units(&decl.name, args),
        (Op::Vectors, _) => vectors(decl, args),
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

fn vector_tag() -> Option<Tag> {
    static VECTOR: OnceLock<Tag> = OnceLock::new();
    tag("onshape/std/vector.fs#Vector", &VECTOR)
}

/// `isLengthVector`: a Vector of lengths.
fn is_length_vector(v: &Value) -> bool {
    let Value::Array(a, t) = v else { return false };
    Some(*t) == vector_tag() && a.iter().all(is_length)
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
    let (unit, value) = match map.unit_value() {
        Some(uv) => uv,
        None => (map.get(&UNIT_KEY)?, map.get(&VALUE_KEY)?),
    };
    let Value::Number(value, Tag::NONE) = value else { return None };
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

/// What an operation answers: a number, operand `i` (a ValueWithUnits) with value `x` (std's `lhs.value *= rhs;
/// return lhs;`), or a new value. Planned without changing the operands: std's body runs on them when one of a
/// vector's elements has no answer here.
enum Out {
    Num(f64),
    Set(usize, f64),
    New(Value),
}

/// Only these two entries: another map's value, with its own unit, is the same answer.
fn plain(v: &Vwu) -> bool {
    v.map.len() == 2
}

fn unique(v: &Vwu) -> bool {
    Arc::strong_count(v.map) == 1 && Arc::weak_count(v.map) == 0
}

/// `v` (a ValueWithUnits) with value `x`: its map changed in place when nothing else holds it, else a copy.
fn put_value(v: Value, x: f64) -> Value {
    let Value::Map(mut m, t) = v else { return v };
    let num = Value::num(x);
    match Arc::get_mut(&mut m) {
        Some(mm) => {
            if let Some(slot) = mm.get_mut(&VALUE_KEY) {
                *slot = num;
            }
        }
        None => {
            let mut c = (*m).clone();
            c.insert(VALUE_KEY.clone(), num);
            m = Arc::new(c);
        }
    }
    Value::Map(m, t)
}

/// The value of `out`, taking operand `i` from `take` (owned: changed in place when nothing else holds it).
fn finish(out: Out, take: impl FnOnce(usize) -> Value) -> Value {
    match out {
        Out::Num(x) => Value::num(x),
        Out::Set(i, x) => put_value(take(i), x),
        Out::New(v) => v,
    }
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
    Some(Value::Map(Arc::new(Map::with_unit_value(Value::Map(Arc::new(unit), t1), Value::num(x))), tag))
}

/// `reciprocal`: every exponent negated, the value inverted.
fn reciprocal(v: &Vwu) -> Option<(Value, f64)> {
    let (m, t) = exponents(v.unit)?;
    let unit: Map = m.iter().map(|(k, e)| Some((k.clone(), Value::num(finite(-number(e)?)?)))).collect::<Option<_>>()?;
    Some((Value::Map(Arc::new(unit), t), 1.0 / v.value))
}

fn units(name: &str, args: &mut [Value]) -> Option<Value> {
    let out = match args {
        [a] => plan_units(name, a, None)?,
        [a, b] => plan_units(name, a, Some(b))?,
        _ => return None,
    };
    Some(finish(out, |i| std::mem::take(&mut args[i])))
}

/// A units.fs operator on `a` and `b` (none: unary), as an [`Out`].
fn plan_units(name: &str, a: &Value, b: Option<&Value>) -> Option<Out> {
    let set = |i: usize, x: f64| finite(x).map(|x| Out::Set(i, x));
    let Some(b) = b else {
        return match name {
            "operator-" => set(0, -vwu(a)?.value),
            _ => None,
        };
    };
    match name {
        "operator+" | "operator-" => {
            let (a, b) = (vwu(a)?, vwu(b)?);
            if a.unit != b.unit {
                return None;
            }
            let x = if name == "operator+" { a.value + b.value } else { a.value - b.value };
            // (The right operand's map when only it is free: the same answer.)
            set(if !unique(&a) && unique(&b) && plain(&a) && plain(&b) { 1 } else { 0 }, x)
        }
        "operator*" => match (vwu(a), vwu(b)) {
            (Some(a), Some(b)) => product(a.tag, a.unit, b.unit, a.value * b.value).map(Out::New),
            (Some(a), None) => set(0, a.value * number(b)?),
            (None, Some(b)) => set(1, b.value * number(a)?),
            (None, None) => None,
        },
        "operator/" => match (vwu(a), vwu(b)) {
            // (The units cancel: `lhs * reciprocal(rhs)`'s number, without their maps.)
            (Some(a), Some(b)) if a.unit == b.unit && exponents(a.unit).is_some() => finite(a.value * (1.0 / b.value)).map(Out::Num),
            (Some(a), Some(b)) => {
                let (unit, r) = reciprocal(&b)?;
                product(a.tag, a.unit, &unit, a.value * r).map(Out::New)
            }
            (Some(a), None) => set(0, a.value / number(b)?),
            (None, Some(b)) => {
                let (unit, r) = reciprocal(&b)?;
                let mut m = (**b.map).clone();
                m.insert(UNIT_KEY.clone(), unit);
                m.insert(VALUE_KEY.clone(), Value::num(finite(r * number(a)?)?));
                Some(Out::New(Value::Map(Arc::new(m), b.tag)))
            }
            (None, None) => None,
        },
        "operator<" => match (vwu(a), vwu(b)) {
            (Some(a), Some(b)) => (a.unit == b.unit).then(|| Out::New(Value::bool(a.value < b.value))),
            (Some(a), None) => (number(b)? == 0.0).then(|| Out::New(Value::bool(a.value < 0.0))),
            (None, Some(b)) => (number(a)? == 0.0).then(|| Out::New(Value::bool(0.0 < b.value))),
            (None, None) => None,
        },
        _ => None,
    }
}

/// An element's operation inside a vector operator (`b` none: unary minus): plain numbers, or what units.fs answers
/// (`Out::Set` naming `a` 0, `b` 1).
fn plan_element(name: &str, a: &Value, b: Option<&Value>) -> Option<Out> {
    let Some(b) = b else {
        return match number(a) {
            Some(n) => Some(Out::Num(-n)),
            None => plan_units(name, a, None),
        };
    };
    if let (Some(x), Some(y)) = (number(a), number(b)) {
        return finite(match name {
            "operator+" => x + y,
            "operator-" => x - y,
            "operator*" => x * y,
            "operator/" => x / y,
            _ => return None,
        })
        .map(Out::Num);
    }
    if vwu(a).is_none() && vwu(b).is_none() {
        return None;
    }
    plan_units(name, a, Some(b))
}

fn unique_array(v: &Value) -> bool {
    matches!(v, Value::Array(a, _) if Arc::strong_count(a) == 1 && Arc::weak_count(a) == 0)
}

/// Element `i` of an array operand: taken when nothing else holds the array (then changed in place when nothing
/// holds it either), else a copy.
fn take_at(a: &mut Arc<Vec<Value>>, i: usize) -> Value {
    match Arc::get_mut(a) {
        Some(v) => std::mem::take(&mut v[i]),
        None => a[i].clone(),
    }
}

/// std's vector operators, element by element: the answer in an operand's own array when nothing else holds it (every
/// element planned first, [`Out`]: std's body runs on untouched operands when one has no answer here), else in a new
/// one.
fn vectors(decl: &FnDecl, args: &mut [Value]) -> Option<Value> {
    let name = &*decl.name;
    // The vector whose array may become the answer, and the other operand: a vector, a scalar or none.
    #[derive(PartialEq)]
    enum Other {
        Vector,
        Scalar,
        None,
    }
    let (out_side, other) = match (name, args.len()) {
        ("operator+" | "operator-", 2) => {
            let (a, b) = (args[0].as_array()?, args[1].as_array()?);
            if a.len() != b.len() {
                return None;
            }
            (if unique_array(&args[0]) || !unique_array(&args[1]) { 0 } else { 1 }, Other::Vector)
        }
        ("operator-", 1) => {
            args[0].as_array()?;
            (0, Other::None)
        }
        ("operator*" | "operator/", 2) => {
            // (vector, scalar) or (scalar, vector); not two vectors, nor a matrix.
            let v = match (decl.param_types[0].is_some(), decl.param_types[1].is_some()) {
                (true, false) => 0,
                (false, true) if name == "operator*" => 1,
                _ => return None,
            };
            args[v].as_array()?;
            (v, Other::Scalar)
        }
        _ => return None,
    };
    let o = 1 - out_side;
    // Element i's operation, operands in std's order.
    let plan = |elements: &[Value], rest: &Value, i: usize| -> Option<Out> {
        let x = &elements[i];
        let y = match (&other, rest) {
            (Other::Vector, Value::Array(r, _)) => Some(&r[i]),
            (Other::Scalar, scalar) => Some(scalar),
            _ => None,
        };
        match y {
            None => plan_element(name, x, None),
            Some(y) if out_side == 0 => plan_element(name, x, Some(y)),
            Some(y) => plan_element(name, y, Some(x)),
        }
    };
    // The answer has its vector's tag (std returns vector1, or the vector; both vectors are Vectors).
    let tag = args[out_side].tag();
    let undefined = Value::Undefined;
    if !unique_array(&args[out_side]) {
        // Nothing of the operands changes: each element's answer made anew, none when one has no answer.
        let elements = args[out_side].as_array()?;
        let rest = if other == Other::None { &undefined } else { &args[o] };
        let mut out = Vec::with_capacity(elements.len());
        for i in 0..elements.len() {
            let p = plan(elements, rest, i)?;
            out.push(finish(p, |k| match (&other, rest) {
                (Other::None, _) => elements[i].clone(),
                _ if k == out_side => elements[i].clone(),
                (Other::Vector, Value::Array(r, _)) => r[i].clone(),
                (_, scalar) => scalar.clone(),
            }));
        }
        return Some(Value::Array(Arc::new(out), tag));
    }
    {
        let elements = args[out_side].as_array()?;
        let rest = if other == Other::None { &undefined } else { &args[o] };
        if !(0..elements.len()).all(|i| plan(elements, rest, i).is_some()) {
            return None;
        }
    }
    let Value::Array(mut out, _) = std::mem::take(&mut args[out_side]) else { return None };
    let mut rest = match other {
        Other::None => Value::Undefined,
        _ => std::mem::take(&mut args[o]),
    };
    let elements = Arc::make_mut(&mut out);
    for i in 0..elements.len() {
        let p = plan(elements, &rest, i)?;
        let v = finish(p, |k| match (&other, &mut rest) {
            (Other::None, _) => std::mem::take(&mut elements[i]),
            _ if k == out_side => std::mem::take(&mut elements[i]),
            (Other::Vector, Value::Array(r, _)) => take_at(r, i),
            (_, scalar) => scalar.clone(),
        });
        elements[i] = v;
    }
    Some(Value::Array(out, tag))
}
