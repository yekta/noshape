//! Evaluating a custom feature inside a Part Studio regeneration.

use super::api::{ScriptCtx, Session, entities};
use super::compile::{DeclKind, ENGINE, error_message, with_deadline};
use crate::params::{NumKind, ParamValues, fill_defaults};
use crate::regen::eval::{EvalCtx, FResult, err};
use parking_lot::Mutex;
use rhai::{Dynamic, Map};
use std::sync::Arc;

/// Evaluator for every `custom:<tab>:<name>` feature type.
pub fn evaluate(ev: &mut EvalCtx) -> FResult<()> {
    let Some(type_id) = ev.custom_type.clone().or_else(|| ev.features.iter().find(|f| f.id == ev.feature).map(|f| f.type_id.clone())) else {
        return err("Custom feature type is unknown");
    };
    let Some((tab, name)) = super::parse_type_id(&type_id) else { return err("Invalid custom feature type") };
    let provider = ev.custom.clone().unwrap_or_else(super::fallback_provider);
    let Some(studio) = provider.studio(tab) else {
        return err(format!("The Feature Studio defining \"{name}\" no longer exists"));
    };
    if let Some(d) = studio.first_error() {
        return err(format!("Feature Studio has errors: {d}"));
    }
    let (Some(def), Some(ast)) = (studio.feature(name), studio.ast.as_ref()) else {
        return err(format!("The Feature Studio no longer defines \"{name}\""));
    };

    // Parameters: stored values, defaults for parameters added since.
    let mut params: ParamValues = ev.params.clone();
    fill_defaults(&def.feature_type.params, &mut params, ev.units);
    let mut p = Map::new();
    for d in &def.params {
        let id = d.id.as_str();
        let range = |v: f64, min: f64, max: f64, fmt: &dyn Fn(f64) -> String| -> FResult<f64> {
            if v < min - 1e-12 || v > max + 1e-12 {
                return err(format!("{} must be between {} and {}", d.label, fmt(min), fmt(max)));
            }
            Ok(v)
        };
        let units = ev.units;
        let len = |x: f64| noshape_units::format_quantity(noshape_units::Quantity::new(x, noshape_units::Dim::LENGTH), units);
        let ang = |x: f64| noshape_units::format_quantity(noshape_units::Quantity::new(x, noshape_units::Dim::ANGLE), units);
        let plain = |x: f64| format!("{x}");
        let value: Dynamic = match &d.kind {
            DeclKind::Length { min, max, .. } => Dynamic::from_float(range(ev.eval_num_in(&params, id, NumKind::Length)?, *min, *max, &len)?),
            DeclKind::Angle { min, max, .. } => Dynamic::from_float(range(ev.eval_num_in(&params, id, NumKind::Angle)?, *min, *max, &ang)?),
            DeclKind::Real { min, max, .. } => Dynamic::from_float(range(ev.eval_num_in(&params, id, NumKind::Real)?, *min, *max, &plain)?),
            DeclKind::Int { min, max, .. } => {
                let v = ev.eval_num_in(&params, id, NumKind::Integer)?.round();
                Dynamic::from_int(range(v, *min as f64, *max as f64, &plain)? as i64)
            }
            DeclKind::Bool { .. } => Dynamic::from_bool(params.bool(id)),
            DeclKind::Enum { options, default } => {
                let v = params.enum_(id);
                Dynamic::from(if options.iter().any(|o| o.0 == v) { v.to_string() } else { default.clone() })
            }
            DeclKind::Text { .. } => Dynamic::from(params.text(id).to_string()),
            DeclKind::Query { .. } => entities(params.query(id)),
        };
        p.insert(id.into(), value);
    }

    let session = Arc::new(Mutex::new(Session {
        feature: ev.feature,
        name: ev.name.to_string(),
        params,
        units: *ev.units,
        outer_vars: ev.outer_vars.clone(),
        ctx: std::mem::take(&mut ev.ctx),
        xform: ev.xform,
        op: 0,
        warnings: Vec::new(),
    }));
    let ctx = Dynamic::from(ScriptCtx(session.clone()));
    let result = with_deadline(|| def.evaluate.call::<Dynamic>(&ENGINE, ast, (ctx, Dynamic::from_map(p))));
    let mut s = session.lock();
    ev.ctx = std::mem::take(&mut s.ctx);
    for w in s.warnings.drain(..) {
        ev.warnings.push(w);
    }
    match result {
        Ok(_) => Ok(()),
        Err(e) => err(error_message(&e)),
    }
}
