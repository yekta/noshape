//! Operations implemented by a built-in feature's evaluator: the
//! FeatureScript definition map is translated to that feature's parameters
//! (Onshape's ops take the same fields as their features).

use super::convert::*;
use super::query::evaluate_refs;
use super::{FsContext, context_arg, run_op_with};
use crate::params::{NumKind, ParamKind, ParamSpec, ParamValue, ParamValues, fill_defaults};
use crate::regen::eval::{EvalCtx, FResult};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("opModifyFillet", |i, a| builtin_op(i, a, "modifyFillet", "DIRECT_EDIT_MODIFY_FILLET_FAILED", &[("reFillet", ParamValue::Bool(false))], crate::regen::features::detail_a::modify_fillet)),
    ("opEnclose", |i, a| builtin_op(i, a, "enclose", "ENCLOSE_CANNOT_CREATE_SOLID", &[], crate::regen::features::solid_a::enclose)),
    ("opCreateCompositePart", |i, a| builtin_op(i, a, "compositePart", "CREATE_COMPOSITE_PART_FAILED", &[], crate::regen::features::refgeom::composite_part)),
    ("opNameEntity", |i, a| builtin_op(i, a, "nameEntity", "REGEN_ERROR", &[], crate::regen::features::refgeom::name_entity)),
    ("opFaceBlend", |i, a| builtin_op_renamed(i, a, "faceBlend", "FACE_BLEND_FAILED", FACE_BLEND_FIELDS, crate::regen::features::detail_a::face_blend)),
    ("opCreateIsocline", |i, a| {
        let op = BuiltinOp { defaults: &[("split", ParamValue::Bool(false))], vectors: &[("direction", "direction")], ..BuiltinOp::new("isocline", "ISOCLINE_NO_RESULT") };
        op.run(i, a, crate::regen::features::surfacing_c::isocline)
    }),
    ("opSplitByIsocline", |i, a| {
        let op = BuiltinOp { defaults: &[("split", ParamValue::Bool(true))], vectors: &[("direction", "direction")], ..BuiltinOp::new("isocline", "ISOCLINE_NO_RESULT") };
        op.run(i, a, crate::regen::features::surfacing_c::isocline)
    }),
    ("opBodyDraft", |i, a| BuiltinOp { vectors: &[("pullDirection", "pullDirection")], ..BuiltinOp::new("bodyDraft", "BODY_DRAFT_FAILED") }.run(i, a, crate::regen::features::detail_a::body_draft)),
    ("opSplineThroughEdges", |i, a| BuiltinOp { enums: &[("fitType", "EDGES")], ..BuiltinOp::new("fitSpline", "REGEN_ERROR") }.run(i, a, crate::regen::features::curves::fit_spline)),
    ("opRuledSurface", op_ruled_surface),
    ("opCreateCurvesOnFace", op_create_curves_on_face),
    ("opFullRoundFillet", |i, a| BuiltinOp { enums: &[("filletType", "FULL_ROUND")], ..BuiltinOp::new("fillet", "FILLET_FAILED") }.run(i, a, crate::regen::features::detail::fillet)),
    // Every opFillet option (cross sections, width, asymmetric, partial,
    // variable) as the Fillet feature builds it.
    ("opFillet", |i, a| BuiltinOp { enums: &[("filletType", "EDGE")], defaults: &[("tangentPropagation", ParamValue::Bool(false))], ..BuiltinOp::new("fillet", "FILLET_FAILED") }.run(i, a, crate::regen::features::detail::fillet)),
    ("evRuledSurfaceBases", ev_ruled_surface_bases),
    ("opDropCurve", |i, a| {
        let op = BuiltinOp { renames: &[("dropTools", "tools")], vectors: &[("directionQuery", "direction")], enums: &[("curveProjectionType", "CURVE_TO_FACE")], ..BuiltinOp::new("projectCurves", "REGEN_ERROR") };
        op.run(i, a, crate::regen::features::curves::project_curves)
    }),
];

/// A built-in feature's evaluator run as an operation: op fields named
/// like the feature's parameters, except `renames` (parameter, field);
/// `vectors` are Vector fields where the feature takes a direction query
/// (passed as a temporary plane with that normal); `defaults` preset
/// parameters the op doesn't have.
struct BuiltinOp {
    type_id: &'static str,
    failure: &'static str,
    defaults: &'static [(&'static str, ParamValue)],
    renames: &'static [(&'static str, &'static str)],
    vectors: &'static [(&'static str, &'static str)],
    /// Parameters set by `adjust` rather than read from the op.
    skip: &'static [&'static str],
    /// Conversions the generic mapping can't express (definition → parameters).
    adjust: Option<fn(&Value, &mut ParamValues)>,
    /// Enum parameters preset (the op's kind, e.g. a full round fillet).
    enums: &'static [(&'static str, &'static str)],
}

impl BuiltinOp {
    const fn new(type_id: &'static str, failure: &'static str) -> Self {
        BuiltinOp { type_id, failure, defaults: &[], renames: &[], vectors: &[], skip: &[], adjust: None, enums: &[] }
    }

    fn run(&self, i: &mut Interp, a: Vec<Value>, f: fn(&mut EvalCtx) -> FResult<()>) -> R<Value> {
        let cx = context_arg(i, &a)?;
        let id = id_arg(i, &a, 1)?;
        let def = def_arg(i, &a, 2)?;
        let Some(ty) = crate::features::get(self.type_id) else {
            return Err(i.error(format!("{} is not supported", self.type_id)));
        };
        let mut p = ParamValues::default();
        let units = cx.state.lock().units;
        fill_defaults(&ty.params, &mut p, &units);
        for (k, v) in self.defaults {
            p.set(k, v.clone());
        }
        for (k, v) in self.enums {
            p.set(k, ParamValue::Enum(v.to_string()));
        }
        // Vector directions: temporary planes, removed afterwards.
        let mut temp = Vec::new();
        for (k, (param, field)) in self.vectors.iter().enumerate() {
            let Some(d) = vec3_of(def.get(field)).filter(|d| d.length() > 0.) else { continue };
            let n = d.normalize();
            let x = any_perpendicular(n);
            let tid = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                (&id, k, "direction").hash(&mut h);
                crate::reference::FeatureId(h.finish() | (1 << 63))
            };
            let r = crate::reference::Reference::FeaturePlane(tid);
            cx.state.lock().ctx.planes.push(crate::regen::context::PlaneEntity { reference: r.clone(), name: String::new(), frame: crate::display::PlaneFrame { origin: glam::DVec3::ZERO, x, y: n.cross(x), normal: n }, half_width: 0., half_height: 0. });
            p.set(param, ParamValue::Query(vec![r.clone()]));
            temp.push(r);
        }
        let skip: Vec<&str> = self.vectors.iter().map(|(p, _)| *p).chain(self.skip.iter().copied()).collect();
        if let Some(f) = self.adjust {
            f(&def, &mut p);
        }
        let res = params_renamed_skipping(i, &cx, &ty.params, &def, self.renames, &skip, &mut p).and_then(|()| run_op_with(i, &cx, &id, self.failure, &p, f));
        cx.state.lock().ctx.planes.retain(|pl| !temp.contains(&pl.reference));
        res.map(|()| Value::Undefined)
    }
}

/// opFaceBlend's fields that the Face blend feature names differently
/// (feature parameter, op field).
const FACE_BLEND_FIELDS: &[(&str, &str)] = &[("flipSide1", "flipSide1Normal"), ("flipSide2", "flipSide2Normal"), ("edgeLimits", "edgeLimit")];

fn expr(s: String) -> ParamValue {
    ParamValue::Expr(noshape_units::ExprValue::new(s))
}

/// Parameter values of `specs` from a definition map (missing fields keep
/// the feature's defaults).
fn params_of(i: &mut Interp, cx: &Arc<FsContext>, specs: &[ParamSpec], def: &Value, out: &mut ParamValues) -> R<()> {
    params_renamed(i, cx, specs, def, &[], out)
}

/// [`params_of`] where some parameters come from differently named op
/// fields (`(parameter, field)`).
fn params_renamed(i: &mut Interp, cx: &Arc<FsContext>, specs: &[ParamSpec], def: &Value, renames: &[(&str, &str)], out: &mut ParamValues) -> R<()> {
    params_renamed_skipping(i, cx, specs, def, renames, &[], out)
}

/// [`params_renamed`] leaving the parameters in `skip` alone.
fn params_renamed_skipping(i: &mut Interp, cx: &Arc<FsContext>, specs: &[ParamSpec], def: &Value, renames: &[(&str, &str)], skip: &[&str], out: &mut ParamValues) -> R<()> {
    for s in specs {
        if skip.contains(&s.id) {
            continue;
        }
        let field = renames.iter().find(|(p, _)| *p == s.id).map_or(s.id, |(_, f)| *f);
        let v = def.get(field);
        if v.is_undefined() {
            continue;
        }
        let pv = match &s.kind {
            ParamKind::Bool { .. } => v.as_bool().map(ParamValue::Bool),
            ParamKind::Enum { .. } => v.as_str().map(|e| ParamValue::Enum(e.to_string())),
            ParamKind::Text { .. } => v.as_str().map(|t| ParamValue::Text(t.to_string())),
            ParamKind::Num(n) => match n.kind {
                NumKind::Length => length_of(v).map(|x| expr(format!("{x} m"))),
                NumKind::Angle => angle_of(v).map(|x| expr(format!("{x} rad"))),
                NumKind::Integer | NumKind::Real => v.as_number().map(|x| expr(format!("{x}"))),
            },
            ParamKind::Query { .. } => Some(ParamValue::Query(evaluate_refs(i, cx, v)?)),
            ParamKind::Array { item, .. } => {
                let mut items = Vec::new();
                for it in v.as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default() {
                    let mut p = ParamValues::default();
                    params_of(i, cx, item, &it, &mut p)?;
                    items.push(p);
                }
                Some(ParamValue::Array(items))
            }
            ParamKind::Lookup { .. } | ParamKind::Sketch | ParamKind::Reference { .. } => None,
        };
        match pv {
            Some(pv) => out.set(s.id, pv),
            None => return Err(i.error(format!("{} has an invalid value", s.id))),
        }
    }
    Ok(())
}

/// Run built-in feature `type_id`'s evaluator `f` as an operation.
fn builtin_op(i: &mut Interp, a: Vec<Value>, type_id: &str, failure: &str, op_defaults: &[(&str, ParamValue)], f: fn(&mut EvalCtx) -> FResult<()>) -> R<Value> {
    builtin_op_with(i, a, type_id, failure, op_defaults, &[], f)
}

/// [`builtin_op`] for ops whose fields differ from the feature's parameter ids.
fn builtin_op_renamed(i: &mut Interp, a: Vec<Value>, type_id: &str, failure: &str, renames: &[(&str, &str)], f: fn(&mut EvalCtx) -> FResult<()>) -> R<Value> {
    builtin_op_with(i, a, type_id, failure, &[], renames, f)
}

fn builtin_op_with(i: &mut Interp, a: Vec<Value>, type_id: &str, failure: &str, op_defaults: &[(&str, ParamValue)], renames: &[(&str, &str)], f: fn(&mut EvalCtx) -> FResult<()>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let Some(ty) = crate::features::get(type_id) else {
        return Err(i.error(format!("{type_id} is not supported")));
    };
    let mut p = ParamValues::default();
    let units = cx.state.lock().units;
    fill_defaults(&ty.params, &mut p, &units);
    for (k, v) in op_defaults {
        p.set(k, v.clone());
    }
    params_renamed(i, &cx, &ty.params, &def, renames, &mut p)?;
    run_op_with(i, &cx, &id, failure, &p, f)?;
    Ok(Value::Undefined)
}

/// `opRuledSurface` (std ruledSurface.fs builds it from the feature):
/// `path`, `width` (negative = the other side), `ruledSurfaceType`
/// ANGLE_FROM_FACE (std passes the feature's Normal as Tangent − 90°),
/// ALIGNED_WITH_VECTOR / ANGLE_FROM_VECTOR with `ruledDirection`, `angle`.
/// Returns a coordinate system at each `manipulatorPositions` entry.
fn op_ruled_surface(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    fn adjust(def: &Value, p: &mut ParamValues) {
        let kind = def.get("ruledSurfaceType").as_str().map(|s| s.to_string()).unwrap_or_else(|| "ANGLE_FROM_FACE".into());
        let angle = angle_of(def.get("angle")).unwrap_or(0.0);
        match kind.as_str() {
            "ALIGNED_WITH_VECTOR" | "ANGLE_FROM_VECTOR" => {
                p.set("ruledType", ParamValue::Enum(kind.clone()));
                p.set("angleFromVector", expr(format!("{angle} rad")));
            }
            _ => {
                p.set("ruledType", ParamValue::Enum("TANGENT".into()));
                p.set("angle", expr(format!("{angle} rad")));
            }
        }
        let width = length_of(def.get("width")).unwrap_or(0.0);
        p.set("distance", expr(format!("{} m", width.abs())));
        p.set("oppositeDirection", ParamValue::Bool(width < 0.0));
    }
    let op = BuiltinOp {
        renames: &[("edges", "path")],
        vectors: &[("axis", "ruledDirection")],
        skip: &["angle", "angleFromVector", "distance", "oppositeDirection", "ruledType", "vertexOverrides"],
        adjust: Some(adjust),
        ..BuiltinOp::new("ruledSurface", "RULED_SURFACE_ZERO_LENGTH")
    };
    let positions = def_arg(i, &a, 2)?.get("manipulatorPositions").clone();
    let cx = context_arg(i, &a)?;
    op.run(i, a, crate::regen::features::surfaces::ruled_surface)?;
    ruled_bases(i, &cx, &positions)
}

/// `@evRuledSurfaceBases`: the frames `opRuledSurface` would return, without
/// making the surface.
pub fn ev_ruled_surface_bases(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let positions = a.get(1).map(|d| d.get("manipulatorPositions").clone()).unwrap_or_default();
    ruled_bases(i, &cx, &positions)
}

/// A frame at each requested edge position: origin on the edge, z along it.
fn ruled_bases(i: &mut Interp, cx: &Arc<FsContext>, positions: &Value) -> R<Value> {
    let mut bases = Vec::new();
    for pos in positions.as_array().iter().flat_map(|v| v.iter()) {
        let t = pos.get("parameter").as_number().unwrap_or(0.0);
        let frame = super::query::with_ents(i, cx, pos.get("edge"), |_, q, ents| {
            Ok(ents.first().and_then(|e| {
                let s = q.shape(e)?;
                let info = s.edge_info().ok()?;
                let u = info.first_param + (info.last_param - info.first_param) * t;
                Some((s.point_at(u).ok()?, s.tangent_at(u).ok()?.normalize_or_zero()))
            }))
        })?;
        let (o, z) = frame.unwrap_or((glam::DVec3::ZERO, glam::DVec3::Z));
        bases.push(coord_system_value(o, any_perpendicular(z), z));
    }
    Ok(Value::array(bases))
}

/// `opCreateCurvesOnFace`: isoparametric curves (the Isoparametric curve
/// feature). `DIR1_ISO` / `DIR2_ISO` make one curve per name at
/// `parameters[i]`, under `id + name`; the auto-spaced types make `nCurves`
/// equally spaced curves under `id`.
fn op_create_curves_on_face(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let f = crate::regen::features::curves::isoparametric_curve;
    for item in def.get("curveDefinition").as_array().map(|v| v.iter().cloned().collect::<Vec<_>>()).unwrap_or_default() {
        let face = evaluate_refs(i, &cx, item.get("face"))?;
        let kind = item.get("creationType").as_str().map(|s| s.to_string()).unwrap_or_default();
        let dir = if kind.starts_with("DIR2") { "V" } else { "U" };
        let base = |p: &mut ParamValues| {
            p.set("face", ParamValue::Query(face.clone()));
            p.set("directionType", ParamValue::Enum(dir.into()));
        };
        if kind.ends_with("AUTO_SPACED_ISO") {
            let mut p = ParamValues::default();
            base(&mut p);
            p.set("equalSpacing", ParamValue::Bool(true));
            p.set("nCurves", expr(format!("{}", item.get("nCurves").as_number().unwrap_or(1.0))));
            run_op_with(i, &cx, &id, "REGEN_ERROR", &p, f)?;
            continue;
        }
        let names: Vec<String> = item.get("names").as_array().map(|v| v.iter().filter_map(|n| n.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
        let params: Vec<f64> = item.get("parameters").as_array().map(|v| v.iter().filter_map(|n| n.as_number()).collect()).unwrap_or_default();
        for (name, t) in names.iter().zip(params) {
            let mut p = ParamValues::default();
            base(&mut p);
            p.set("equalSpacing", ParamValue::Bool(false));
            let mut row = ParamValues::default();
            row.set("selectPoint", ParamValue::Bool(false));
            row.set("uvValue", expr(format!("{t}")));
            p.set("uvParamList", ParamValue::Array(vec![row]));
            let mut sub = id.clone();
            sub.push(name.as_str().into());
            run_op_with(i, &cx, &sub, "REGEN_ERROR", &p, f)?;
        }
    }
    Ok(Value::Undefined)
}
