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
];

fn expr(s: String) -> ParamValue {
    ParamValue::Expr(noshape_units::ExprValue::new(s))
}

/// Parameter values of `specs` from a definition map (missing fields keep
/// the feature's defaults).
fn params_of(i: &mut Interp, cx: &Arc<FsContext>, specs: &[ParamSpec], def: &Value, out: &mut ParamValues) -> R<()> {
    for s in specs {
        let v = def.get(s.id);
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
    params_of(i, &cx, &ty.params, &def, &mut p)?;
    run_op_with(i, &cx, &id, failure, &p, f)?;
    Ok(Value::Undefined)
}
