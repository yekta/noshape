//! Revolve (solid), full or partial, one or two directions, symmetric.

use crate::display::BodyKind;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, sweep_names};

pub fn eval(ev: &mut EvalCtx) -> FResult<()> {
    if !ev.is("bodyType", "SOLID") {
        return err("Only solid revolve is supported so far");
    }
    let refs = ev.query("entities");
    if refs.is_empty() {
        return err("Select sketch regions or planar faces to revolve");
    }
    let Some(axis_ref) = ev.query("axis").first() else { return err("Select a revolve axis") };
    let Some((origin, axis)) = ev.resolve_axis(axis_ref) else { return err("The revolve axis could not be resolved") };
    let axis = if ev.bool("oppositeDirection") { -axis } else { axis };
    let profiles = ev.profiles(refs)?;
    let full = ev.bool("fullRevolve");
    let mut tools = Vec::new();
    for p in &profiles {
        let res = if full {
            noshape_kernel::revolve(&p.shape, origin, axis, std::f64::consts::TAU)?
        } else {
            if !ev.is("endBound", "BLIND") {
                return err("Only blind revolve end types are supported so far");
            }
            let a = ev.angle("angle")?;
            if ev.bool("symmetric") {
                noshape_kernel::revolve_between(&p.shape, origin, axis, -a * 0.5, a * 0.5)?
            } else if ev.bool("hasStartBound") && ev.is("startBound", "BLIND") {
                let back = ev.angle("angleBack")?;
                let back = if ev.bool("startOppositeDirection") { -back } else { back };
                noshape_kernel::revolve_between(&p.shape, origin, axis, -back, a)?
            } else {
                noshape_kernel::revolve(&p.shape, origin, axis, a)?
            }
        };
        let names = sweep_names(ev, p, &res, !full, !full);
        tools.push(NamedShape::new(res.shape, names));
    }
    let op = ev.enum_("operationType").to_string();
    let scope = super::extrude::scope(ev);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}
