//! Boolean, Variable and Delete part (Plane: see `cplane`).

use crate::regen::eval::{EvalCtx, FResult, NamedShape, err};
use noshape_kernel as k;
use noshape_units::Dim;

pub fn boolean(ev: &mut EvalCtx) -> FResult<()> {
    let tools_idx = ev.resolve_bodies(ev.query("tools"));
    if tools_idx.is_empty() {
        return crate::regen::eval::err_at(&["tools"], "Cannot resolve entities.");
    }
    let op = ev.enum_("operationType").to_string();
    match op.as_str() {
        "UNION" => {
            if tools_idx.len() < 2 {
                return err("Select at least two parts to union");
            }
            // One union of all the parts: those that touch become one part
            // (the first one's), parts apart keep theirs (qa5 BOOL1: the old
            // tool-by-tool removal by index deleted an unpicked part).
            // The first-picked part keeps its name, id and colour (live NAME2:
            // Pin, then Part 1 → "Pin").
            let refs = ev.query("tools").to_vec();
            let mut picked = tools_idx.clone();
            picked.sort_by_key(|&i| refs.iter().position(|r| ev.resolve_bodies(std::slice::from_ref(r)).contains(&i)).unwrap_or(usize::MAX));
            let before = ev.ctx.bodies.len();
            ev.fuse_targets(&picked, Vec::new(), &k::BooleanOptions::default())?;
            // Live (qa5 BOOL1): parts apart stay as they were, with a blue
            // note while editing.
            if ev.ctx.bodies.len() == before {
                ev.infos.push("Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.".into());
            }
            Ok(())
        }
        "SUBTRACTION" | "INTERSECTION" => {
            let targets = ev.resolve_bodies(ev.query("targets"));
            if targets.is_empty() && op == "SUBTRACTION" {
                return err("Select target parts");
            }
            let tool_ids: Vec<_> = tools_idx.iter().map(|&i| ev.ctx.bodies[i].id.clone()).collect();
            let tools: Vec<NamedShape> = tools_idx.iter().map(|&i| {
                let b = &ev.ctx.bodies[i];
                NamedShape { shape: b.shape.clone(), names: b.shape_names() }
            }).collect();
            if op == "INTERSECTION" && targets.is_empty() {
                // Intersect all tools with each other: keep the common volume in the first.
                // Live ("qa5 booleans"): the common volume is a new part (new
                // id and colour); the tools go.
                let first = tools_idx[0];
                let others = tools[1..].to_vec();
                // ☑ Keep tools keeps every tool (live BOOL2).
                let kept: Vec<(usize, std::sync::Arc<crate::regen::context::Body>)> = if ev.bool("keepTools") { tools_idx.iter().map(|&i| (i, ev.ctx.bodies[i].clone())).collect() } else { Vec::new() };
                ev.boolean_into(&[first], others, k::BooleanOp::Common)?;
                let first_id = tool_ids[0].clone();
                let made: Vec<NamedShape> = ev.ctx.bodies.iter().filter(|b| b.id == first_id || (b.id.feature == ev.feature && !tool_ids.contains(&b.id))).map(|b| NamedShape { shape: b.shape.clone(), names: b.shape_names() }).collect();
                let kind = ev.ctx.bodies.iter().find(|b| b.id == first_id).map(|b| b.kind).unwrap_or(crate::display::BodyKind::Solid);
                let feature = ev.feature;
                ev.ctx.bodies.retain(|b| !tool_ids.contains(&b.id) && b.id.feature != feature);
                for (i, b) in kept {
                    let at = i.min(ev.ctx.bodies.len());
                    ev.ctx.bodies.insert(at, b);
                }
                for (i, m) in made.into_iter().enumerate() {
                    ev.add_new_bodies(m, kind, i as u32)?;
                }
                return Ok(());
            }
            let bop = if op == "SUBTRACTION" { k::BooleanOp::Cut } else { k::BooleanOp::Common };
            ev.boolean_into(&targets, tools, bop)?;
            if !ev.bool("keepTools") {
                ev.ctx.bodies.retain(|b| !tool_ids.contains(&b.id));
            }
            Ok(())
        }
        other => err(format!("Unknown boolean operation {other}")),
    }
}

pub fn variable(ev: &mut EvalCtx) -> FResult<()> {
    let name = ev.params.text("name").trim().trim_start_matches('#').to_string();
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return err("Variable name must be a letter followed by a string of letters or numbers.");
    }
    // Measured (variable.fs): a distance (minimum or maximum, or one of its
    // components), the total length of edges, or a diameter / radius.
    if ev.is("mode", "MEASURED") {
        use crate::regen::measure;
        let value = match ev.enum_("measurementMode") {
            "LENGTH" => {
                let refs = ev.query("lengthEntities").to_vec();
                if refs.is_empty() {
                    return err("Select edges to measure");
                }
                measure::total_length(&ev.ctx, &refs)
            }
            "DIAMETER" => {
                let Some(r) = ev.query("diameterEntity").first().cloned() else { return err("Select a circular edge, cylinder or sphere") };
                let d = measure::entity_diameter(&ev.ctx, &r).ok_or_else(|| crate::regen::eval::FeatureError::new("Select a circular edge, cylinder or sphere".to_string()))?;
                if ev.bool("radius") { d / 2.0 } else { d }
            }
            _ => {
                let refs = ev.query("entityCouple").to_vec();
                let [a, b] = refs.as_slice() else { return err("Select two entities to measure between") };
                let d = measure::entity_distance(&ev.ctx, a, b, ev.is("minmax", "MAXIMUM")).ok_or_else(|| crate::regen::eval::FeatureError::new("Could not measure between the selections".to_string()))?;
                match ev.enum_("componentSelector") {
                    "X" if ev.bool("axisMeasurements") => d.delta.x.abs(),
                    "Y" if ev.bool("axisMeasurements") => d.delta.y.abs(),
                    "Z" if ev.bool("axisMeasurements") => d.delta.z.abs(),
                    _ => d.distance,
                }
            }
        };
        ev.ctx.variables.insert(name, noshape_units::Quantity::new(value, Dim::LENGTH));
        // The list name shows the measured value ("#d = 95.635 mm").
        ev.computed.push(("measuredValue".into(), crate::display::ComputedValue::Length(value)));
        return Ok(());
    }
    if ev.is("mode", "TABLE") {
        return err("Importing variables from a table is not available yet");
    }
    let q = match ev.enum_("variableType") {
        "LENGTH" => noshape_units::Quantity::new(ev.length("lengthValue")?, Dim::LENGTH),
        "ANGLE" => noshape_units::Quantity::new(ev.angle("angleValue")?, Dim::ANGLE),
        "NUMBER" => noshape_units::Quantity::new(ev.real("numberValue")?, Dim::UNITLESS),
        _ => {
            let text = ev.params.text("anyValue").to_string();
            let scope_vars = ev.ctx.variables.clone();
            let scope = noshape_units::LayeredScope { inner: &scope_vars, outer: ev.outer_vars };
            noshape_units::evaluate(&text, &scope).map_err(|e| crate::regen::eval::FeatureError::new(format!("{text}: {e}")))?
        }
    };
    ev.ctx.variables.insert(name, q);
    Ok(())
}

pub fn delete_bodies(ev: &mut EvalCtx) -> FResult<()> {
    let ids: Vec<_> = ev.resolve_bodies(ev.query("entities")).into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect();
    if ids.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select entities to delete.");
    }
    ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
    Ok(())
}
