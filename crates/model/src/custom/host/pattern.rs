//! Pattern transform natives (`@computeLinearPatternTransforms`, ...): the
//! instances of std's linear and circular patterns, as computed by the std's
//! pre-builtin FeatureScript (kept in linearPattern.fs / circularPattern.fs
//! for old features), plus skipped instances.

use super::convert::*;
use glam::{DMat4, DVec3};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Value};

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("computeLinearPatternTransforms", compute_linear),
    ("computeCircularPatternTransforms", compute_circular),
    ("computeCurvePatternTransforms", compute_curve),
];

fn int_field(def: &Value, key: &str, default: i64) -> i64 {
    def_number(def, key).map(|x| x.round() as i64).unwrap_or(default)
}

/// `{ transforms, instanceNames, manipulatorPoints }`.
fn result(transforms: Vec<Value>, names: Vec<Value>) -> Value {
    let mut m = Map::new();
    m.insert(Value::str("transforms"), Value::array(transforms));
    m.insert(Value::str("instanceNames"), Value::array(names));
    m.insert(Value::str("manipulatorPoints"), Value::array(Vec::new()));
    Value::map(m)
}

/// Skipped (index1, index2) pairs when `skipInstances` is on.
fn skipped(def: &Value) -> Vec<(i64, i64)> {
    if !def_bool(def, "skipInstances", false) {
        return Vec::new();
    }
    def.get("skippedInstances")
        .as_array()
        .map(|a| a.iter().map(|s| (int_field(s, "index1", int_field(s, "index", 0)), int_field(s, "index2", 0))).collect())
        .unwrap_or_default()
}

fn compute_linear(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let def = def_arg(i, &a, 1)?;
    let o1 = vec3_of(def.get("offset1")).unwrap_or(DVec3::ZERO);
    let o2 = vec3_of(def.get("offset2")).unwrap_or(DVec3::ZERO);
    let count1 = int_field(&def, "instanceCount", 1);
    let count2 = int_field(&def, "instanceCountTwo", 1);
    let start1 = if def_bool(&def, "isCentered", false) { 1 - count1 } else { 0 };
    let start2 = if def_bool(&def, "isCenteredTwo", false) { 1 - count2 } else { 0 };
    let skip = skipped(&def);
    let (mut transforms, mut names) = (Vec::new(), Vec::new());
    for j in start2..count2 {
        for k in start1..count1 {
            if (j == 0 && k == 0) || skip.contains(&(k, j)) {
                continue;
            }
            transforms.push(transform_value(&DMat4::from_translation(o1 * k as f64 + o2 * j as f64)));
            names.push(Value::str(if j == 0 { k.to_string() } else { format!("{k}_{j}") }));
        }
    }
    Ok(result(transforms, names))
}

fn compute_circular(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let def = def_arg(i, &a, 1)?;
    let Some((origin, dir)) = line_of(def.get("axis")) else {
        return Err(i.error("axis should be a Line"));
    };
    let angle = angle_of(def.get("angle")).unwrap_or(0.0);
    let count = int_field(&def, "instanceCount", 1);
    let start = if def_bool(&def, "isCentered", false) { 1 - count } else { 0 };
    let skip = skipped(&def);
    let dir = dir.normalize_or_zero();
    let (mut transforms, mut names) = (Vec::new(), Vec::new());
    for k in start..count {
        if k == 0 || skip.contains(&(k, 0)) {
            continue;
        }
        let m = DMat4::from_translation(origin) * DMat4::from_axis_angle(dir, angle * k as f64) * DMat4::from_translation(-origin);
        transforms.push(transform_value(&m));
        names.push(Value::str(k.to_string()));
    }
    Ok(result(transforms, names))
}

/// std vector.fs perpendicularVector.
fn perpendicular(v: DVec3) -> DVec3 {
    if v.length_squared() < 1e-16 {
        return DVec3::X;
    }
    let different = if v.x.abs() > 1.036_663_652_861_932_7 * v.y.abs() {
        if v.x.abs() > 0.951_702_989_392_233_5 * v.z.abs() { DVec3::Z } else { DVec3::Y }
    } else if v.y.abs() > 0.920_419_947_455_385_9 * v.z.abs() {
        DVec3::X
    } else {
        DVec3::Y
    };
    different.cross(v).normalize()
}

/// std rotationMatrix3d(from, to): the minimal rotation.
fn rotation_between(from: DVec3, to: DVec3) -> DMat4 {
    let axis = from.cross(to);
    if axis.length_squared() < 1e-16 {
        return if from.dot(to) > 0.0 { DMat4::IDENTITY } else { DMat4::from_axis_angle(perpendicular(from), std::f64::consts::PI) };
    }
    DMat4::from_axis_angle(axis.normalize(), axis.length().atan2(from.dot(to)))
}

/// std transform(from, to) for two Lines or two Planes.
fn frame_transform(from: &Value, to: &Value) -> Option<DMat4> {
    if !from.get("normal").is_undefined() {
        let (a, b) = (plane_of(from)?, plane_of(to)?);
        let frame = |p: &crate::display::PlaneFrame| DMat4::from_cols(p.x.extend(0.0), p.y.extend(0.0), p.normal.extend(0.0), p.origin.extend(1.0));
        return Some(frame(&b) * frame(&a).inverse());
    }
    let ((o1, d1), (o2, d2)) = (line_of(from)?, line_of(to)?);
    let r = rotation_between(d1, d2);
    Some(DMat4::from_translation(o2 - r.transform_point3(o1)) * r)
}

fn compute_curve(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let def = def_arg(i, &a, 1)?;
    let tangents: Vec<Value> = def.get("tangents").as_array().map(|t| t.iter().cloned().collect()).unwrap_or_default();
    let count = int_field(&def, "instanceCount", 1);
    let skip = skipped(&def);
    let (mut transforms, mut names) = (Vec::new(), Vec::new());
    let mut acc = DMat4::IDENTITY;
    for k in 1..count {
        let (Some(from), Some(to)) = (tangents.get(k as usize - 1), tangents.get(k as usize)) else { break };
        let Some(step) = frame_transform(from, to) else {
            return Err(i.error("tangents should be Lines or Planes"));
        };
        acc = step * acc;
        if !skip.contains(&(k, 0)) {
            transforms.push(transform_value(&acc));
            names.push(Value::str(k.to_string()));
        }
    }
    Ok(result(transforms, names))
}
