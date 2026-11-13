//! Boolean, Variable and Delete part (Plane: see `cplane`).

use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, err_at};
use noshape_kernel as k;
use noshape_units::Dim;

/// Onshape's note for a Boolean that changes nothing (live qa5 BOOL1).
use crate::regen::eval::NO_GEOMETRY_CHANGE;

/// The seconds OCCT's face offset of an offset Subtract's tool may take.
const OFFSET_DEADLINE_S: f64 = 2.0;

/// Live's info for an Intersect of parts that only touch.
const TOUCHING_INTERSECT: &str = "The parts either do not intersect or are totally contained.";

pub fn boolean(ev: &mut EvalCtx) -> FResult<()> {
    let tools_idx = ev.resolve_bodies(ev.query("tools"));
    if tools_idx.is_empty() {
        // As Onshape's opBoolean, its `tools` at fault (live ui-parity boolean-new-edit-reedit: with Keep tools
        // remembered on, the new dialog runs and Tools is red).
        return err_at(&["tools"], "Cannot resolve entities.");
    }
    let op = ev.enum_("operationType").to_string();
    match op.as_str() {
        "UNION" => {
            if tools_idx.len() < 2 {
                return err("Need at least two parts or surfaces for a Boolean operation.");
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
            let picked_shapes: Vec<k::Shape> = picked.iter().filter_map(|&i| ev.ctx.bodies.get(i)).map(|b| b.shape.clone()).collect();
            // ☑ Keep tools: the union is a new part and the tools stay as they were (live, asm-hand-drill-pinslot's
            // studio 9fbc05: three parts after it, the union 3605.4 mm³ and both gears unchanged; as Intersect's).
            let kept = ev.bool("keepTools").then(|| ev.ctx.bodies.clone());
            ev.fuse_targets(&picked, Vec::new(), &k::BooleanOptions::default())?;
            if let Some(saved) = kept
                && ev.ctx.bodies.len() < before
            {
                let made: Vec<NamedShape> = ev.ctx.bodies.iter().filter(|b| !saved.iter().any(|s| std::sync::Arc::ptr_eq(s, b))).map(|b| NamedShape { shape: b.shape.clone(), names: b.shape_names() }).collect();
                let kind = picked.first().and_then(|&i| saved.get(i)).map_or(crate::display::BodyKind::Solid, |b| b.kind);
                ev.ctx.bodies = saved;
                for (i, m) in made.into_iter().enumerate() {
                    ev.add_new_bodies(m, kind, i as u32)?;
                }
                return Ok(());
            }
            // Live (qa5 BOOL1): parts apart stay as they were, with a blue
            // note while editing; parts touching only along an edge or at a
            // point stay two parts too, with no note (live 1760).
            if ev.ctx.bodies.len() == before {
                let s = &picked_shapes;
                let touching = (0..s.len()).any(|i| (i + 1..s.len()).any(|j| k::touching(&s[i], &s[j], 1e-9).unwrap_or(false)));
                if !touching {
                    ev.infos.push(NO_GEOMETRY_CHANGE.into());
                }
            }
            Ok(())
        }
        "SUBTRACTION" | "INTERSECTION" => {
            let targets = ev.resolve_bodies(ev.query("targets"));
            if targets.is_empty() && op == "SUBTRACTION" {
                // Live (qa5 boolean, Subtract with no Targets).
                return err("Cannot resolve entities.");
            }
            let tool_ids: Vec<_> = tools_idx.iter().map(|&i| ev.ctx.bodies[i].id.clone()).collect();
            // Offset (boolean.fs): copies of the tools with their picked faces (every face, Offset all) offset by the
            // distance as Move face's Offset, subtracted in their place; the tools stay or go as without it. A zero
            // distance offsets nothing (V1700_BOOLEAN_ZERO_OFFSET).
            let offset = if op == "SUBTRACTION" && ev.bool("offset") { ev.length("offsetDistance")? * if ev.bool("oppositeDirection") { -1.0 } else { 1.0 } } else { 0.0 };
            let picked = (offset.abs() > 1e-12 && !ev.bool("offsetAll")).then(|| ev.resolve_faces(ev.query("entitiesToOffset")));
            let mut tools: Vec<NamedShape> = Vec::with_capacity(tools_idx.len());
            for &i in &tools_idx {
                let b = ev.ctx.bodies[i].clone();
                let names = b.shape_names();
                let faces: Vec<usize> = match &picked {
                    _ if offset.abs() <= 1e-12 => Vec::new(),
                    None => (0..b.faces.len()).collect(),
                    Some(p) => p.iter().filter(|f| f.0 == i).map(|f| f.1).collect(),
                };
                if faces.is_empty() {
                    tools.push(NamedShape { shape: b.shape.clone(), names });
                    continue;
                }
                let shapes: Vec<k::Shape> = faces.iter().filter_map(|&f| b.shape.sub_shape(k::ShapeType::Face, f).ok()).collect();
                // (OCCT's face offset of planes, cylinders, cones, spheres and tori, within a time limit; else the tool
                // as it is, as before Offset was taken here: a thread's free-form flanks, c2-fan-blade-sweep's Boolean 2,
                // failed after 32 s, and took 4 s with the limit.)
                let analytic = shapes.iter().all(|f| matches!(f.face_surface(), Ok(k::Surface::Plane(_) | k::Surface::Cylinder { .. } | k::Surface::Cone { .. } | k::Surface::Sphere { .. } | k::Surface::Torus { .. })));
                let res = analytic.then(|| k::with_deadline(OFFSET_DEADLINE_S, || k::offset_faces(&b.shape, &shapes, offset)).0);
                let Some(res) = res.and_then(|r| r.ok()).filter(|r| r.shape.is_valid() && r.shape.count(k::ShapeType::Solid) > 0) else {
                    log::debug!("boolean: the tools' faces not offset (OCCT's offset failed)");
                    tools.push(NamedShape { shape: b.shape.clone(), names });
                    continue;
                };
                let role = |n: &crate::naming::Name| crate::reference::Role::Offset(n.clone());
                let names = ev.name_result(&[&names], &res.history, &res.shape, &std::collections::BTreeMap::new(), Some(&role));
                tools.push(NamedShape::new(res.shape, names));
            }
            if op == "INTERSECTION" && targets.is_empty() && tools_idx.len() < 2 {
                // Live (ui-parity boolean-intersect): one part fails, as Union's (opBoolean's input check).
                return err("Need at least two parts or surfaces for a Boolean operation.");
            }
            if op == "INTERSECTION" && targets.is_empty() {
                // Intersect all tools with each other: keep the common volume in the first.
                // Live ("qa5 booleans"): the common volume is a new part (new
                // id and colour); the tools go.
                let first = tools_idx[0];
                let others = tools[1..].to_vec();
                // ☑ Keep tools keeps every tool (live BOOL2).
                let kept: Vec<(usize, std::sync::Arc<crate::regen::context::Body>)> = if ev.bool("keepTools") { tools_idx.iter().map(|&i| (i, ev.ctx.bodies[i].clone())).collect() } else { Vec::new() };
                let saved = ev.ctx.bodies.clone();
                ev.boolean_into(&[first], others, k::BooleanOp::Common)?;
                let first_id = tool_ids[0].clone();
                // Parts that don't meet: nothing is made and nothing changes
                // (qa5 INT2; the untouched first part isn't a result).
                let untouched = ev.ctx.bodies.iter().any(|b| std::sync::Arc::ptr_eq(b, &saved[first]));
                let made: Vec<NamedShape> = if untouched { Vec::new() } else { ev.ctx.bodies.iter().filter(|b| b.id == first_id || (b.id.feature == ev.feature && !tool_ids.contains(&b.id))).map(|b| NamedShape { shape: b.shape.clone(), names: b.shape_names() }).collect() };
                if made.is_empty() {
                    ev.ctx.bodies = saved;
                    // Parts that only touch (two blocks face to face, live
                    // BOOL-TOUCH-INFO): they go (unless kept), with live's
                    // shorter info; parts apart change nothing (INT2).
                    let first_shape = ev.ctx.bodies[first].shape.clone();
                    let touching = tools_idx[1..].iter().any(|&i| k::touching(&first_shape, &ev.ctx.bodies[i].shape, 1e-9).unwrap_or(false));
                    if touching {
                        if !ev.bool("keepTools") {
                            ev.ctx.bodies.retain(|b| !tool_ids.contains(&b.id));
                        }
                        ev.infos.push(TOUCHING_INTERSECT.into());
                        return Ok(());
                    }
                    ev.infos.push(NO_GEOMETRY_CHANGE.into());
                    return Ok(());
                }
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
            let before: Vec<(crate::reference::BodyId, k::Shape)> = targets.iter().map(|&i| (ev.ctx.bodies[i].id.clone(), ev.ctx.bodies[i].shape.clone())).collect();
            ev.boolean_into(&targets, tools, bop)?;
            // A subtraction that leaves every target as it was (live oracle_p1 B5).
            if op == "SUBTRACTION" && before.iter().all(|(id, s)| ev.ctx.bodies.position_id(id).is_some_and(|i| crate::regen::eval::unchanged(s, &ev.ctx.bodies[i].shape))) {
                ev.infos.push(crate::regen::eval::TOOLS_MISS_TARGETS.into());
            }
            if !ev.bool("keepTools") {
                ev.ctx.bodies.remove_ids(&tool_ids);
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
                    return crate::regen::eval::err_at(&["lengthEntities"], "Select entities to measure.");
                }
                measure::total_length(&ev.ctx, &refs)
            }
            "DIAMETER" => {
                let Some(r) = ev.query("diameterEntity").first().cloned() else { return crate::regen::eval::err_at(&["diameterEntity"], "Select entity to measure.") };
                let d = measure::entity_diameter(&ev.ctx, &r).ok_or_else(|| crate::regen::eval::FeatureError::new("Unable to find geometry with a diameter in query.".to_string()))?;
                if ev.bool("radius") { d / 2.0 } else { d }
            }
            _ => {
                let refs = ev.query("entityCouple").to_vec();
                let [a, b] = refs.as_slice() else {
                    return match refs.len() {
                        0 => crate::regen::eval::err_at(&["entityCouple"], "Select the first entity to measure between."),
                        1 => err("Select the second entity to measure between."),
                        _ => err("Only two entities can be selected for distance measurement."),
                    };
                };
                let d = measure::entity_distance(&ev.ctx, a, b, ev.is("minmax", "MAXIMUM")).ok_or_else(|| crate::regen::eval::FeatureError::new("Could not measure between the selections".to_string()))?;
                // std variable.fs selectDistanceComponent: the value along the
                // chosen axis, whether or not the Axis measurements group is
                // open (a group's fold is the dialog's only).
                match ev.enum_("componentSelector") {
                    "X" => d.delta.x.abs(),
                    "Y" => d.delta.y.abs(),
                    "Z" => d.delta.z.abs(),
                    "CUSTOM" => {
                        let Some(r) = ev.query("customDirection").first().cloned() else { return err(crate::regen::custom_features::error_text("VARIABLE_SELECT_CUSTOM_DIRECTION")) };
                        let dir = ev.resolve_direction(&r).ok_or_else(|| crate::regen::eval::FeatureError::at(&["customDirection"], "Select custom direction.".to_string()))?;
                        d.delta.dot(dir.normalize_or_zero()).abs()
                    }
                    _ => d.distance,
                }
            }
        };
        ev.ctx.variables.insert(name, noshape_units::Quantity::new(value, Dim::LENGTH));
        // The list name shows the measured value ("#d = 95.635 mm").
        ev.computed.push(("measuredValue".into(), crate::display::ComputedValue::Length(value)));
        return Ok(());
    }
    // From import: std's variable.fs reads the CSV tab (rows / columns by
    // index, range or label, the conversion factor).
    if ev.is("mode", "TABLE") {
        return crate::regen::custom_features::evaluate_std(ev, "variable.fs", "assignVariable");
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
