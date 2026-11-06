//! std's ValueWithUnits and Vector operators (units.fs, vector.fs) run here: the values their FeatureScript bodies
//! give, without a FeatureScript call per operation (a Displacement map's 262,144 points made millions, ~5 µs each).
//! `None` wherever a body could answer otherwise (units that differ: its precondition's error; a NaN; a tagged
//! number; a std before 2506, whose texts differ): the body runs as written. With `compact`, the answers are
//! compact values (`Value::Quantity`, `Value::Vector`: no map, no array) where they can be.

use crate::ir::FnDecl;
use crate::value::{Compact, Map, Tag, UNIT_KEY, UnitId, VALUE_KEY, Value, VecLen, unit_id, unit_value, vector_tag, vwu_tag};
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
    /// `size(container is array)`, `size(container is map)`: `@size(container)`.
    Size,
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
        ("onshape/std/containers.fs", "size") if decl.arity() == 1 => Op::Size,
        ("onshape/std/vector.fs", "vector") if matches!(decl.arity(), 2 | 3) && decl.param_types.iter().all(Option::is_none) => Op::NewVector,
        ("onshape/std/units.fs", _) => Op::Units,
        ("onshape/std/vector.fs", _) => Op::Vectors,
        _ => return None,
    })
}

/// The value std's operator `decl` (classified `op`) answers for `args`, when it is one of these; compact values
/// where they can be with `compact`. `args` are taken only when answered.
pub fn apply(op: Op, decl: &FnDecl, args: &mut [Value], compact: bool) -> Option<Value> {
    #[cfg(test)]
    if OFF.with(|o| o.get()) {
        return None;
    }
    match (op, &*args) {
        (Op::IsLength, [v]) => Some(Value::bool(is_length(v))),
        (Op::IsLengthVector, [v]) => Some(Value::bool(is_length_vector(v))),
        (Op::Is3dLengthVector, [v]) => Some(Value::bool(is_length_vector(v) && array_len(v) == Some(3))),
        (Op::ControlPointMatrix, [v]) => Some(Value::bool(control_point_matrix(v))),
        (Op::Size, [v]) => array_len(v).or_else(|| map_len(v)).map(|n| Value::num(n as f64)),
        (Op::NewVector, _) => {
            let tag = vector_tag()?;
            if compact && let Some(c) = compact_of(args) {
                return Some(Value::Vector(c));
            }
            Some(Value::Array(Arc::new(args.iter_mut().map(std::mem::take).collect()), tag))
        }
        (Op::Units, _) => units(&decl.name, args, compact),
        (Op::Vectors, _) => vectors(decl, args, compact),
        _ => None,
    }
}

fn array_len(v: &Value) -> Option<usize> {
    match v {
        Value::Array(a, _) => Some(a.len()),
        Value::Vector(c) => Some(c.len()),
        _ => None,
    }
}

fn map_len(v: &Value) -> Option<usize> {
    match v {
        Value::Map(m, _) => Some(m.len()),
        Value::Quantity(..) => Some(2),
        _ => None,
    }
}

/// Two or three plain numbers, or quantities of one unit, as a compact Vector's elements.
fn compact_of(items: &[Value]) -> Option<Compact> {
    let len = match items.len() {
        2 => VecLen::Two,
        3 => VecLen::Three,
        _ => return None,
    };
    let mut v = [0.0; 3];
    let mut unit = None;
    for (i, x) in items.iter().enumerate() {
        let (n, u) = match x {
            Value::Number(n, Tag::NONE) => (*n, UnitId(0)),
            Value::Quantity(n, u) => (*n, *u),
            _ => return None,
        };
        if *unit.get_or_insert(u) != u {
            return None;
        }
        v[i] = n;
    }
    Some(Compact { v, unit: unit?, len })
}

fn tag(key: &'static str, cell: &'static OnceLock<Tag>) -> Option<Tag> {
    if let Some(t) = cell.get() {
        return Some(*t);
    }
    let t = crate::value::tag_of_key(key)?;
    Some(*cell.get_or_init(|| t))
}

/// LENGTH_UNITS: `{ "meter" : 1 } as UnitSpec`.
fn is_length_unit(unit: &Value) -> bool {
    static SPEC: OnceLock<Tag> = OnceLock::new();
    static METER: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| Value::str("meter"));
    let Value::Map(unit, t) = unit else { return false };
    Some(*t) == tag("onshape/std/units.fs#UnitSpec", &SPEC) && unit.len() == 1 && unit.get(&METER) == Some(&Value::num(1.0))
}

/// `isLength`: a ValueWithUnits whose unit is LENGTH_UNITS.
fn is_length(v: &Value) -> bool {
    match v {
        Value::Quantity(_, u) => is_length_unit(unit_value(*u)),
        Value::Map(m, t) if Some(*t) == vwu_tag() => m.get(&UNIT_KEY).is_some_and(is_length_unit),
        _ => false,
    }
}

/// `isLengthVector`: a Vector of lengths.
fn is_length_vector(v: &Value) -> bool {
    match v {
        Value::Vector(c) => c.unit.0 != 0 && is_length_unit(unit_value(c.unit)),
        Value::Array(a, t) => Some(*t) == vector_tag() && a.iter().all(is_length),
        _ => false,
    }
}

/// `canBeControlPointMatrix`: rows (arrays) as long as the first, of 3D length vectors.
fn control_point_matrix(v: &Value) -> bool {
    let Value::Array(rows, _) = v else { return false };
    let width = rows.first().map(array_len);
    for row in rows.iter() {
        let Value::Array(r, _) = row else { return false };
        if Some(Some(r.len())) != width || !r.iter().all(|x| is_length_vector(x) && array_len(x) == Some(3)) {
            return false;
        }
    }
    true
}

/// A ValueWithUnits: its map (none: compact), tag, value and unit (a compact one's by id, its map fetched only when
/// read).
struct Vwu<'a> {
    map: Option<&'a Arc<Map>>,
    tag: Tag,
    value: f64,
    unit: Unit<'a>,
}

#[derive(Clone, Copy)]
enum Unit<'a> {
    Id(UnitId),
    Map(&'a Value),
}

/// The unit map of `u`.
fn unit_of<'a>(u: &Unit<'a>) -> &'a Value {
    match u {
        Unit::Id(id) => unit_value(*id),
        Unit::Map(v) => v,
    }
}

/// Whether two units are the same map (two ids: the same id; the table holds each map once).
fn same_unit(a: &Unit, b: &Unit) -> bool {
    match (a, b) {
        (Unit::Id(x), Unit::Id(y)) => x == y,
        _ => unit_of(a) == unit_of(b),
    }
}

fn vwu(v: &Value) -> Option<Vwu<'_>> {
    if let Value::Quantity(x, u) = v {
        return Some(Vwu { map: None, tag: vwu_tag()?, value: *x, unit: Unit::Id(*u) });
    }
    let Value::Map(map, tag) = v else { return None };
    if Some(*tag) != vwu_tag() {
        return None;
    }
    let (unit, value) = match map.unit_value() {
        Some(uv) => uv,
        None => (map.get(&UNIT_KEY)?, map.get(&VALUE_KEY)?),
    };
    let Value::Number(value, Tag::NONE) = value else { return None };
    Some(Vwu { map: Some(map), tag: *tag, value: *value, unit: Unit::Map(unit) })
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

/// Only these two entries (or compact): another map's value, with its own unit, is the same answer.
fn plain(v: &Vwu) -> bool {
    v.map.is_none_or(|m| m.len() == 2)
}

fn unique(v: &Vwu) -> bool {
    v.map.is_none_or(|m| Arc::strong_count(m) == 1 && Arc::weak_count(m) == 0)
}

/// `v` (a ValueWithUnits) with value `x`: compact when it is or (with `compact`) when its map is only its unit and
/// value; else its map changed in place when nothing else holds it, else a copy.
fn put_value(v: Value, x: f64, compact: bool) -> Value {
    if let Value::Quantity(_, u) = v {
        return Value::Quantity(x, u);
    }
    let Value::Map(mut m, t) = v else { return v };
    if compact
        && let Some((unit, Value::Number(_, Tag::NONE))) = m.unit_value()
        && let Some(id) = unit_id(unit)
    {
        return Value::Quantity(x, id);
    }
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

/// A ValueWithUnits of `unit` and `x`: compact with `compact`.
fn quantity(tag: Tag, unit: Value, x: f64, compact: bool) -> Value {
    if compact
        && Some(tag) == vwu_tag()
        && let Some(id) = unit_id(&unit)
    {
        return Value::Quantity(x, id);
    }
    Value::Map(Arc::new(Map::with_unit_value(unit, Value::num(x))), tag)
}

/// The value of `out`, taking operand `i` from `take`.
fn finish(out: Out, take: impl FnOnce(usize) -> Value, compact: bool) -> Value {
    match out {
        Out::Num(x) => Value::num(x),
        Out::Set(i, x) => put_value(take(i), x, compact),
        Out::New(v) => v,
    }
}

/// A unit map's exponents, each a plain number.
fn exponents(u: &Value) -> Option<(&Arc<Map>, Tag)> {
    let Value::Map(m, t) = u else { return None };
    m.iter().all(|(k, e)| matches!(k, Value::Str(..)) && number(e).is_some()).then_some((m, *t))
}

/// `unitProduct` and operator*(ValueWithUnits, ValueWithUnits): a number when the units cancel.
fn product(tag: Tag, u1: &Value, u2: &Value, x: f64, compact: bool) -> Option<Value> {
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
    Some(quantity(tag, Value::Map(Arc::new(unit), t1), x, compact))
}

/// `reciprocal`: every exponent negated, the value inverted.
fn reciprocal(v: &Vwu) -> Option<(Value, f64)> {
    let (m, t) = exponents(unit_of(&v.unit))?;
    let unit: Map = m.iter().map(|(k, e)| Some((k.clone(), Value::num(finite(-number(e)?)?)))).collect::<Option<_>>()?;
    Some((Value::Map(Arc::new(unit), t), 1.0 / v.value))
}

fn units(name: &str, args: &mut [Value], compact: bool) -> Option<Value> {
    let out = match args {
        [a] => plan_units(name, a, None, compact)?,
        [a, b] => plan_units(name, a, Some(b), compact)?,
        _ => return None,
    };
    Some(finish(out, |i| std::mem::take(&mut args[i]), compact))
}

/// A units.fs operator on `a` and `b` (none: unary), as an [`Out`].
fn plan_units(name: &str, a: &Value, b: Option<&Value>, compact: bool) -> Option<Out> {
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
            if !same_unit(&a.unit, &b.unit) {
                return None;
            }
            let x = if name == "operator+" { a.value + b.value } else { a.value - b.value };
            // (The right operand's map when only it is free: the same answer.)
            set(if !unique(&a) && unique(&b) && plain(&a) && plain(&b) { 1 } else { 0 }, x)
        }
        "operator*" => match (vwu(a), vwu(b)) {
            (Some(a), Some(b)) => product(a.tag, unit_of(&a.unit), unit_of(&b.unit), a.value * b.value, compact).map(Out::New),
            (Some(a), None) => set(0, a.value * number(b)?),
            (None, Some(b)) => set(1, b.value * number(a)?),
            (None, None) => None,
        },
        "operator/" => match (vwu(a), vwu(b)) {
            // (The units cancel: `lhs * reciprocal(rhs)`'s number, without their maps.)
            (Some(a), Some(b)) if same_unit(&a.unit, &b.unit) && exponents(unit_of(&a.unit)).is_some() => finite(a.value * (1.0 / b.value)).map(Out::Num),
            (Some(a), Some(b)) => {
                let (unit, r) = reciprocal(&b)?;
                product(a.tag, unit_of(&a.unit), &unit, a.value * r, compact).map(Out::New)
            }
            (Some(a), None) => set(0, a.value / number(b)?),
            (None, Some(b)) => {
                let (unit, r) = reciprocal(&b)?;
                let x = finite(r * number(a)?)?;
                Some(Out::New(match b.map {
                    // (Its own other entries kept, as std's `rhs.unit = …; rhs.value = …`.)
                    Some(m) if m.len() != 2 => {
                        let mut m = (**m).clone();
                        m.insert(UNIT_KEY.clone(), unit);
                        m.insert(VALUE_KEY.clone(), Value::num(x));
                        Value::Map(Arc::new(m), b.tag)
                    }
                    _ => quantity(b.tag, unit, x, compact),
                }))
            }
            (None, None) => None,
        },
        "operator<" => match (vwu(a), vwu(b)) {
            (Some(a), Some(b)) => same_unit(&a.unit, &b.unit).then(|| Out::New(Value::bool(a.value < b.value))),
            (Some(a), None) => (number(b)? == 0.0).then(|| Out::New(Value::bool(a.value < 0.0))),
            (None, Some(b)) => (number(a)? == 0.0).then(|| Out::New(Value::bool(0.0 < b.value))),
            (None, None) => None,
        },
        _ => None,
    }
}

/// An element's operation inside a vector operator (`b` none: unary minus): plain numbers, or what units.fs answers
/// (`Out::Set` naming `a` 0, `b` 1).
fn plan_element(name: &str, a: &Value, b: Option<&Value>, compact: bool) -> Option<Out> {
    let Some(b) = b else {
        return match number(a) {
            Some(n) => Some(Out::Num(-n)),
            None => plan_units(name, a, None, compact),
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
    plan_units(name, a, Some(b), compact)
}

/// A vector operand's elements: borrowed from its array, or made from a compact Vector.
enum Elems<'a> {
    Slice(&'a [Value]),
    Made([Value; 3], usize),
}

impl Elems<'_> {
    fn get(&self) -> &[Value] {
        match self {
            Elems::Slice(s) => s,
            Elems::Made(a, n) => &a[..*n],
        }
    }
}

fn elems(v: &Value) -> Option<Elems<'_>> {
    match v {
        Value::Array(a, _) => Some(Elems::Slice(a)),
        Value::Vector(c) => Some(Elems::Made([c.at(0), c.at(1), if c.len() == 3 { c.at(2) } else { Value::Undefined }], c.len())),
        _ => None,
    }
}

/// std's vector operators, element by element: every element planned and made from copies of the operands (nothing
/// of them changed: std's body runs on them when one has no answer here); the answer a compact Vector where it can
/// be, else an array with its vector's tag (std returns vector1, or the vector; both vectors are Vectors).
fn vectors(decl: &FnDecl, args: &mut [Value], compact: bool) -> Option<Value> {
    let name = &*decl.name;
    // The vector operand, the other (a vector, a scalar, none) and their elements' operation.
    enum Other<'a> {
        Vector(Elems<'a>),
        Scalar(&'a Value),
        None,
    }
    let (v, other) = match (name, args.len()) {
        ("operator+" | "operator-", 2) => (0, Other::Vector(elems(&args[1])?)),
        ("operator-", 1) => (0, Other::None),
        ("operator*" | "operator/", 2) => {
            // (vector, scalar) or (scalar, vector); not two vectors, nor a matrix.
            match (decl.param_types[0].is_some(), decl.param_types[1].is_some()) {
                (true, false) => (0, Other::Scalar(&args[1])),
                (false, true) if name == "operator*" => (1, Other::Scalar(&args[0])),
                _ => return None,
            }
        }
        _ => return None,
    };
    let a = elems(&args[v])?;
    let a = a.get();
    if let Other::Vector(b) = &other
        && b.get().len() != a.len()
    {
        return None;
    }
    let element = |i: usize| -> Option<Value> {
        let x = &a[i];
        match &other {
            Other::Vector(b) => {
                let y = &b.get()[i];
                let p = plan_element(name, x, Some(y), compact)?;
                Some(finish(p, |k| if k == 0 { x.clone() } else { y.clone() }, compact))
            }
            Other::Scalar(s) => {
                let p = if v == 0 { plan_element(name, x, Some(s), compact) } else { plan_element(name, s, Some(x), compact) }?;
                Some(finish(p, |k| if k == v { x.clone() } else { (*s).clone() }, compact))
            }
            Other::None => {
                let p = plan_element(name, x, None, compact)?;
                Some(finish(p, |_| x.clone(), compact))
            }
        }
    };
    let tag = args[v].tag();
    // (Two or three: in a buffer, a compact Vector without an array made.)
    if compact && (2..=3).contains(&a.len()) && Some(tag) == vector_tag() {
        let mut buf = [Value::Undefined, Value::Undefined, Value::Undefined];
        for (i, slot) in buf.iter_mut().enumerate().take(a.len()) {
            *slot = element(i)?;
        }
        if let Some(c) = compact_of(&buf[..a.len()]) {
            return Some(Value::Vector(c));
        }
        return Some(Value::Array(Arc::new(buf.into_iter().take(a.len()).collect()), tag));
    }
    let out = (0..a.len()).map(element).collect::<Option<Vec<_>>>()?;
    Some(Value::Array(Arc::new(out), tag))
}
