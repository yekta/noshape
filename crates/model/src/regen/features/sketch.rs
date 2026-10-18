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
    // Re-project used ("Use") 3D edges so they follow upstream changes.
    let mut missing = 0;
    for (eid, r) in &data.external_refs {
        let edges = ev.resolve_edges(std::slice::from_ref(r));
        let Some(&(bi, ei)) = edges.first() else {
            missing += 1;
            continue;
        };
        let Ok(edge) = ev.edge_shape(bi, ei) else { continue };
        if let Some(c) = project_edge(&edge, &frame) {
            update_external(&mut sk, noshape_sketch::EntityId(*eid), &c);
        }
    }
    if missing > 0 {
        ev.warnings.push(format!("{missing} projected edge(s) no longer exist"));
    }
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

/// Project a 3D edge onto a sketch plane (orthogonal projection).
pub fn project_edge(edge: &noshape_kernel::Shape, frame: &crate::display::PlaneFrame) -> Option<noshape_sketch::Curve2> {
    use noshape_kernel as k;
    use noshape_sketch::Curve2;
    let info = edge.edge_info().ok()?;
    match info.curve {
        k::Curve::Line { .. } => Some(Curve2::Line { a: frame.to_local(info.start), b: frame.to_local(info.end) }),
        k::Curve::Circle { center, axis, radius, .. } if axis.normalize().dot(frame.normal).abs() > 1.0 - 1e-9 => {
            let c = frame.to_local(center);
            if info.closed {
                return Some(Curve2::Circle { center: c, radius });
            }
            let ang = |p: glam::DVec3| {
                let d = frame.to_local(p) - c;
                d.y.atan2(d.x)
            };
            // The edge runs CCW around its axis; seen from the sketch normal it is
            // CCW when the axis agrees with the normal.
            let (mut a0, mut a1) = (ang(info.start), ang(info.end));
            if axis.dot(frame.normal) < 0.0 {
                std::mem::swap(&mut a0, &mut a1);
            }
            let mut e = a1;
            while e <= a0 {
                e += std::f64::consts::TAU;
            }
            Some(Curve2::Arc { center: c, radius, start_angle: a0, end_angle: e })
        }
        _ => None,
    }
}

/// Update an external entity's geometry in place from a projected curve.
pub fn update_external(sk: &mut noshape_sketch::Sketch, id: noshape_sketch::EntityId, c: &noshape_sketch::Curve2) {
    use noshape_sketch::{Curve2, Geometry};
    let Some(e) = sk.entity(id).cloned() else { return };
    match (&e.geometry, c) {
        (Geometry::Line { start, end }, Curve2::Line { a, b }) => {
            let _ = sk.set_point(*start, *a);
            let _ = sk.set_point(*end, *b);
        }
        (Geometry::Circle { center, .. }, Curve2::Circle { center: c2, radius }) => {
            let _ = sk.set_point(*center, *c2);
            if let Some(ent) = sk.entity_mut(id)
                && let Geometry::Circle { radius: r, .. } = &mut ent.geometry
            {
                *r = *radius;
            }
        }
        (Geometry::Arc { center, start, end }, Curve2::Arc { center: c2, radius, start_angle, end_angle }) => {
            let _ = sk.set_point(*center, *c2);
            let _ = sk.set_point(*start, *c2 + glam::DVec2::from_angle(*start_angle) * *radius);
            let _ = sk.set_point(*end, *c2 + glam::DVec2::from_angle(*end_angle) * *radius);
        }
        _ => {}
    }
}
