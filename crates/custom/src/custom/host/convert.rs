//! Conversions between FeatureScript values (ValueWithUnits, Vector, Plane,
//! Line, CoordSystem, Transform, Box3d) and model geometry. Lengths are
//! meters and angles radians on both sides.

use super::std_tag;
use crate::display::PlaneFrame;
use glam::{DMat3, DMat4, DVec2, DVec3};
use noshape_featurescript::interp::{Interp, R};
use noshape_featurescript::value::{Map, Tag, Value};
use std::sync::Arc;

fn unit_spec(entries: &[(&str, f64)]) -> Value {
    let mut m = Map::new();
    for (k, v) in entries {
        m.insert(Value::str(*k), Value::num(*v));
    }
    Value::Map(Arc::new(m), std_tag("units.fs", "UnitSpec"))
}

fn with_units(x: f64, unit: &[(&str, f64)]) -> Value {
    let mut m = Map::new();
    m.insert(Value::str("unit"), unit_spec(unit));
    m.insert(Value::str("value"), Value::num(x));
    Value::Map(Arc::new(m), std_tag("units.fs", "ValueWithUnits"))
}

pub fn length_value(x: f64) -> Value {
    with_units(x, &[("meter", 1.0)])
}
pub fn area_value(x: f64) -> Value {
    with_units(x, &[("meter", 2.0)])
}
pub fn volume_value(x: f64) -> Value {
    with_units(x, &[("meter", 3.0)])
}
pub fn angle_value(x: f64) -> Value {
    with_units(x, &[("radian", 1.0)])
}
pub fn mass_value(x: f64) -> Value {
    with_units(x, &[("kilogram", 1.0)])
}

/// The unit map of a ValueWithUnits (`{"meter": 1}`), `None` for numbers.
fn units_of(v: &Value) -> Option<Vec<(Arc<str>, f64)>> {
    let u = v.get("unit").as_map()?;
    Some(u.iter().filter_map(|(k, x)| Some((k.as_str()?, x.as_number()?))).collect())
}

fn is_unit(v: &Value, name: &str, power: f64) -> bool {
    units_of(v).is_some_and(|u| u.len() == 1 && &*u[0].0 == name && u[0].1 == power)
}

/// Meters of a length (`ValueWithUnits` with meter units) or of a plain
/// number (already stripped of units).
pub fn length_of(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n, _) => Some(*n),
        Value::Map(..) if is_unit(v, "meter", 1.0) => v.get("value").as_number(),
        _ => None,
    }
}

/// Radians of an angle (or a plain number).
pub fn angle_of(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n, _) => Some(*n),
        Value::Map(..) if is_unit(v, "radian", 1.0) => v.get("value").as_number(),
        _ => None,
    }
}

/// The magnitude of any number or ValueWithUnits (SI).
pub fn magnitude_of(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n, _) => Some(*n),
        Value::Map(..) => v.get("value").as_number(),
        _ => None,
    }
}

/// A 3D point or direction: an array of 3 lengths or numbers.
pub fn vec3_of(v: &Value) -> Option<DVec3> {
    let a = v.as_array()?;
    if a.len() != 3 {
        return None;
    }
    Some(DVec3::new(magnitude_of(&a[0])?, magnitude_of(&a[1])?, magnitude_of(&a[2])?))
}

pub fn vec2_of(v: &Value) -> Option<DVec2> {
    let a = v.as_array()?;
    if a.len() != 2 {
        return None;
    }
    Some(DVec2::new(magnitude_of(&a[0])?, magnitude_of(&a[1])?))
}

fn vector_tag() -> Tag {
    std_tag("vector.fs", "Vector")
}

/// Kernel results carry -0.0 where Onshape's have 0 (it prints a face normal
/// as "(-1, 0, 0)"; `-0` only prints for a literal -0).
fn pos0(x: f64) -> f64 {
    x + 0.0
}

/// A Vector of lengths.
pub fn point_value(p: DVec3) -> Value {
    Value::Array(Arc::new(vec![length_value(pos0(p.x)), length_value(pos0(p.y)), length_value(pos0(p.z))]), vector_tag())
}

/// A unitless Vector.
pub fn dir_value(d: DVec3) -> Value {
    Value::Array(Arc::new(vec![Value::num(pos0(d.x)), Value::num(pos0(d.y)), Value::num(pos0(d.z))]), vector_tag())
}

pub fn point2_value(p: DVec2) -> Value {
    Value::Array(Arc::new(vec![length_value(p.x), length_value(p.y)]), vector_tag())
}

pub fn plane_value(f: &PlaneFrame) -> Value {
    let m = Value::map_from([("origin", point_value(f.origin)), ("normal", dir_value(f.normal)), ("x", dir_value(f.x))]);
    m.with_tag(std_tag("surfaceGeometry.fs", "Plane"))
}

pub fn plane_of(v: &Value) -> Option<PlaneFrame> {
    let origin = vec3_of(v.get("origin"))?;
    let normal = vec3_of(v.get("normal"))?.normalize_or_zero();
    let x = match v.get("x") {
        Value::Undefined => any_perpendicular(normal),
        x => vec3_of(x)?.normalize_or_zero(),
    };
    if normal == DVec3::ZERO || x == DVec3::ZERO {
        return None;
    }
    Some(PlaneFrame { origin, x, y: normal.cross(x).normalize_or_zero(), normal })
}

/// Onshape's default x direction for a plane with only a normal.
pub fn any_perpendicular(n: DVec3) -> DVec3 {
    let a = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    (a - n * a.dot(n)).normalize_or_zero()
}

pub fn line_value(origin: DVec3, dir: DVec3) -> Value {
    let m = Value::map_from([("origin", point_value(origin)), ("direction", dir_value(dir))]);
    m.with_tag(std_tag("curveGeometry.fs", "Line"))
}

pub fn line_of(v: &Value) -> Option<(DVec3, DVec3)> {
    let o = vec3_of(v.get("origin"))?;
    let d = vec3_of(v.get("direction"))?.normalize_or_zero();
    (d != DVec3::ZERO).then_some((o, d))
}

pub fn coord_system_value(origin: DVec3, x: DVec3, z: DVec3) -> Value {
    let m = Value::map_from([("origin", point_value(origin)), ("xAxis", dir_value(x)), ("zAxis", dir_value(z))]);
    m.with_tag(std_tag("coordSystem.fs", "CoordSystem"))
}

/// (origin, x, z) of a CoordSystem.
pub fn coord_system_of(v: &Value) -> Option<(DVec3, DVec3, DVec3)> {
    let o = vec3_of(v.get("origin"))?;
    let x = vec3_of(v.get("xAxis"))?.normalize_or_zero();
    let z = vec3_of(v.get("zAxis"))?.normalize_or_zero();
    (x != DVec3::ZERO && z != DVec3::ZERO).then_some((o, x, z))
}

/// A Transform (`{ linear : Matrix, translation : Vector }`) as a matrix.
pub fn transform_of(v: &Value) -> Option<DMat4> {
    let rows = v.get("linear").as_array()?;
    if rows.len() != 3 {
        return None;
    }
    let mut m = [[0.0f64; 3]; 3];
    for (r, row) in rows.iter().enumerate() {
        let row = row.as_array()?;
        if row.len() != 3 {
            return None;
        }
        for c in 0..3 {
            m[r][c] = row[c].as_number()?;
        }
    }
    let t = vec3_of(v.get("translation"))?;
    // Columns of the linear map.
    let lin = DMat3::from_cols(DVec3::new(m[0][0], m[1][0], m[2][0]), DVec3::new(m[0][1], m[1][1], m[2][1]), DVec3::new(m[0][2], m[1][2], m[2][2]));
    Some(DMat4::from_cols(lin.x_axis.extend(0.0), lin.y_axis.extend(0.0), lin.z_axis.extend(0.0), t.extend(1.0)))
}

pub fn transform_value(m: &DMat4) -> Value {
    let row = |r: usize| Value::array((0..3).map(|c| Value::num(m.col(c)[r])).collect());
    let linear = Value::Array(Arc::new(vec![row(0), row(1), row(2)]), std_tag("matrix.fs", "Matrix"));
    let t = m.col(3).truncate();
    Value::map_from([("linear", linear), ("translation", point_value(t))]).with_tag(std_tag("transform.fs", "Transform"))
}

pub fn box_value(min: DVec3, max: DVec3) -> Value {
    Value::map_from([("minCorner", point_value(min)), ("maxCorner", point_value(max))]).with_tag(std_tag("box.fs", "Box3d"))
}

// ---------------------------------------------------------------- definition fields

/// `definition[key]` as meters; error naming the field otherwise.
pub fn def_length(i: &mut Interp, def: &Value, key: &str) -> R<f64> {
    match length_of(def.get(key)) {
        Some(x) => Ok(x),
        None => Err(i.error(format!("{key} should be a length, was {}", def.get(key).type_description()))),
    }
}

pub fn def_length_opt(def: &Value, key: &str) -> Option<f64> {
    length_of(def.get(key))
}

pub fn def_angle(i: &mut Interp, def: &Value, key: &str) -> R<f64> {
    match angle_of(def.get(key)) {
        Some(x) => Ok(x),
        None => Err(i.error(format!("{key} should be an angle, was {}", def.get(key).type_description()))),
    }
}

pub fn def_vec3(i: &mut Interp, def: &Value, key: &str) -> R<DVec3> {
    match vec3_of(def.get(key)) {
        Some(x) => Ok(x),
        None => Err(i.error(format!("{key} should be a 3D vector, was {}", def.get(key).type_description()))),
    }
}

pub fn def_bool(def: &Value, key: &str, default: bool) -> bool {
    def.get(key).as_bool().unwrap_or(default)
}

pub fn def_number(def: &Value, key: &str) -> Option<f64> {
    def.get(key).as_number()
}

/// An enum field's member name (`BoundingType.BLIND` -> "BLIND").
pub fn def_enum(def: &Value, key: &str) -> Option<Arc<str>> {
    def.get(key).as_str()
}

/// The definition map argument of a native (`@op...(context, id, definition)`).
pub fn def_arg(i: &mut Interp, a: &[Value], n: usize) -> R<Value> {
    match a.get(n) {
        Some(v @ Value::Map(..)) => Ok(v.clone()),
        Some(other) => Err(i.error(format!("Argument {} should be a map, was {}", n + 1, other.type_description()))),
        None => Ok(Value::map(Map::new())),
    }
}

/// An Id argument (array of strings).
pub fn id_arg(i: &mut Interp, a: &[Value], n: usize) -> R<super::FsId> {
    let Some(arr) = a.get(n).and_then(|v| v.as_array()) else {
        return Err(i.error(format!("Argument {} should be an Id", n + 1)));
    };
    let mut out = Vec::with_capacity(arr.len());
    for c in arr.iter() {
        match c.as_str() {
            Some(s) => out.push(s),
            None => return Err(i.error("Id components must be strings")),
        }
    }
    Ok(out)
}

pub fn id_value(id: &[Arc<str>]) -> Value {
    Value::Array(Arc::new(id.iter().map(|s| Value::str(s.clone())).collect()), std_tag("context.fs", "Id"))
}
