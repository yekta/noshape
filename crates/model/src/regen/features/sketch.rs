//! Sketch feature.

use crate::params::ParamValue;
use crate::regen::context::SketchResult;
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_sketch::{ConstraintId, ConstraintKind};
use noshape_units::Dim;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    let Some(plane_ref) = ev.query("sketchPlane").first() else {
        return err("Select a sketch plane");
    };
    let Some(frame) = ev.resolve_plane(plane_ref) else {
        return err("The sketch plane could not be found");
    };
    let data = match ev.params.get("sketch") {
        Some(ParamValue::Sketch(d)) => d.clone(),
        _ => Box::default(),
    };
    let mut sk = data.sketch.clone();
    // Expression-driven dimensions.
    let mut problems = Vec::new();
    for (cid, expr) in &data.dimension_exprs {
        let id = ConstraintId(*cid);
        let Some(c) = sk.constraint(id) else { continue };
        if !c.driving {
            continue;
        }
        let dim = match c.kind {
            ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. } => Dim::ANGLE,
            _ => Dim::LENGTH,
        };
        match ev.eval_text(&expr.text, dim) {
            Ok(q) => {
                let _ = sk.set_dimension_value(id, q.value);
            }
            Err(e) => problems.push(e.message),
        }
    }
    let report = sk.solve();
    let regions = sk.regions();
    let wires = sk.wires();
    let mut dimension_text = BTreeMap::new();
    for c in sk.constraints() {
        if let Some(v) = c.kind.value() {
            let q = match c.kind {
                ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. } => noshape_units::Quantity::new(v, Dim::ANGLE),
                _ => noshape_units::Quantity::new(v, Dim::LENGTH),
            };
            let mut text = noshape_units::format_quantity(q, ev.units);
            if matches!(c.kind, ConstraintKind::Diameter { .. }) {
                text = format!("Ø{text}");
            } else if matches!(c.kind, ConstraintKind::Radius { .. }) {
                text = format!("R{text}");
            }
            if !c.driving {
                text = format!("({text})");
            }
            dimension_text.insert(c.id.0, text);
        }
    }
    if let noshape_sketch::SolveStatus::Overconstrained { .. } = report.status {
        ev.warnings.push("Sketch has conflicting constraints".into());
    }
    if !problems.is_empty() {
        ev.warnings.push(problems.join("; "));
    }
    let name = ev.name.to_string();
    ev.ctx.sketches.retain(|s| s.feature != ev.feature);
    ev.ctx.sketches.push(Arc::new(SketchResult { feature: ev.feature, name, frame, sketch: sk, report, regions, wires, dimension_text }));
    Ok(())
}
