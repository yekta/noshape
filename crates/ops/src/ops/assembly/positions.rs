//! Named positions and exploded views.

use super::{assembly, asm_field, commit, find_instance, find_mate, parse_connector, resolve_entity, solve, target};
use crate::error::OpError;
use crate::ops::query::len_scale;
use crate::ops::{doc_field, s, strings};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, assembly as sa};
use glam::DVec3;
use noshape_model::assembly::eval::{AssemblyResult, SolveOptions};
use noshape_model::assembly::{Assembly, ExplodeStep, ExplodedView, MateId};
use noshape_model::document::TabId;
use noshape_units::{Dim, EmptyScope, UnitPrefs};
use serde_json::{Value, json};

const POSITION_ACTIONS: [&str; 9] = ["add", "update", "apply", "duplicate", "delete", "rename", "move_up", "move_down", "set_value"];
const DOFS: [&str; 4] = ["rz", "tx", "ty", "tz"];

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "assembly",
            verb: "named_position",
            summary: "Named positions: add, update, apply, duplicate, rename, reorder, delete, set a driving value",
            description: "The Named positions panel. `add` stores the current state (\"Named position 1\"...); `update` stores the current state in an existing position; `apply` moves the assembly to a position (its driving mates' values are imposed, the other mates stay); `set_value` sets a driving mate's value in a position (see assembly_named_position_drivers). Each action is one edit with the panel's text (\"Insert : Named position 1\", \"Apply named position : Open\"...).",
            fields: vec![
                doc_field(),
                asm_field(),
                req("action", json!({ "type": "string", "enum": POSITION_ACTIONS }), "What to do."),
                opt("position", s(), "The position: its name or 1-based row (all actions but add)."),
                opt("name", s(), "add: the new position's name; rename: the new name."),
                opt("mate", s(), "set_value: the driving mate (id or name)."),
                opt("dof", json!({ "type": "string", "enum": DOFS }), "set_value: the mate's value: rz = rotation about Z, tx / ty / tz = translation."),
                opt("value", json!({ "type": ["string", "number"] }), "set_value: the value (angle or length expression)."),
            ],
            mutates: true,
            destructive: false,
            run: named_position,
        },
        Op {
            group: "assembly",
            verb: "named_position_drivers",
            summary: "Choose the mates (and their degrees of freedom) that drive named positions",
            description: "The Driving mate selector: each driver is a mate and one of its free values (revolute: rz; slider: tz; cylindrical: tz, rz; pin slot: tx, rz; planar: tx, ty, rz; parallel: tx, ty, tz, rz). Replaces the list. One edit (\"Driving mate selector\").",
            fields: vec![doc_field(), asm_field(), req("drivers", json!({ "type": "array", "items": { "type": "object" } }), "[{\"mate\": id or name, \"dof\": \"rz\" | \"tx\" | \"ty\" | \"tz\"}]")],
            mutates: true,
            destructive: false,
            run: drivers,
        },
        Op {
            group: "assembly",
            verb: "exploded_view",
            summary: "Exploded views: add, rename or delete a view or a step",
            description: "The Exploded views panel: `add` creates \"Explode N\" (renamed to `name` if given); `rename` renames a view (or its `step`); `delete` deletes a view (or its `step`). Add steps with assembly_explode_step.",
            fields: vec![
                doc_field(),
                asm_field(),
                req("action", json!({ "type": "string", "enum": ["add", "rename", "delete"] }), "What to do."),
                opt("view", s(), "The view: its name or 1-based row."),
                opt("step", json!({ "type": "integer", "minimum": 1 }), "A step of the view (1-based)."),
                opt("name", s(), "add / rename: the name."),
            ],
            mutates: true,
            destructive: true,
            run: exploded_view,
        },
        Op {
            group: "assembly",
            verb: "explode_step",
            summary: "Add or edit an explode step: move or rotate instances",
            description: "An explode step of a view: translate the instances `distance` along `direction`, or rotate them by `angle` about `direction` (through `pivot`, the picked connector's origin, or the instances' center). `direction` is a world vector [x, y, z] or a mate connector (its Z axis). One edit (\"Edit : Explode step\").",
            fields: vec![
                doc_field(),
                asm_field(),
                req("view", s(), "The view: its name or 1-based row."),
                req("instances", strings(), "Instance ids or names."),
                req("direction", json!({ "type": ["array", "object"] }), "Direction / rotation axis: [x, y, z] or a mate connector (see assembly_mate)."),
                opt("distance", json!({ "type": ["string", "number"] }), "Translate by this distance."),
                opt("angle", json!({ "type": ["string", "number"] }), "Rotate by this angle instead."),
                opt("pivot", json!({ "type": "array", "items": { "type": "number" } }), "Rotation pivot [x, y, z] in document units."),
                opt("lines", json!({ "type": "boolean", "default": true }), "Show explode lines."),
                opt("step", json!({ "type": "integer", "minimum": 1 }), "Edit this step (1-based) instead of adding one."),
            ],
            mutates: true,
            destructive: false,
            run: explode_step,
        },
        Op {
            group: "assembly",
            verb: "display_state",
            summary: "Display states: add, apply, update, rename, remove",
            description: "The Display states panel: `add` saves which instances and mates are hidden now as a new state `name` (\"Add display state : Front\"); `apply` shows and hides them as the state says (instances added since keep theirs); `update` saves the current visibility into the state; `rename` to `name`; `remove`. One edit each with the panel's text.",
            fields: vec![
                doc_field(),
                asm_field(),
                req("action", json!({ "type": "string", "enum": ["add", "apply", "update", "rename", "remove"] }), "What to do."),
                opt("state", s(), "The display state: its name or 1-based row (all actions but add)."),
                opt("name", s(), "add: the new state's name; rename: the new name."),
            ],
            mutates: true,
            destructive: false,
            run: display_state,
        },
    ]
}

/// The named positions and exploded views, for assembly_list.
pub(super) fn json(a: &Assembly) -> (Value, Value) {
    let positions: Vec<Value> = a.named_positions.iter().map(|p| json!(p.name)).collect();
    let views: Vec<Value> = a.exploded_views.iter().map(|v| json!({ "name": v.name, "steps": v.steps.len() })).collect();
    (json!(positions), json!(views))
}

fn row(names: Vec<&str>, key: &str, field: &str) -> Result<usize, OpError> {
    let k = key.trim();
    names
        .iter()
        .position(|n| *n == k)
        .or_else(|| names.iter().position(|n| n.eq_ignore_ascii_case(k)))
        .or_else(|| k.parse::<usize>().ok().filter(|i| (1..=names.len()).contains(i)).map(|i| i - 1))
        .ok_or_else(|| OpError::not_found(format!("no `{key}`; have: {}", names.join(", "))).with_param(field))
}

fn quantity(v: &Value, dim: Dim, units: &UnitPrefs, field: &str) -> Result<f64, OpError> {
    let text = match v {
        Value::String(t) => t.clone(),
        Value::Number(n) => n.to_string(),
        _ => return Err(OpError::param(field, "an expression or a number")),
    };
    noshape_units::evaluate_as(&text, dim, units, &EmptyScope).map(|q| q.value).map_err(|e| OpError::param(field, &e.to_string()))
}

/// Commit with `solved` in place of the current solution (the app's
/// apply_solved followed by an edit).
fn commit_solved(od: &mut OpenDoc, tab: TabId, desc: &str, mut result: AssemblyResult, solved: noshape_model::assembly::eval::Solved, f: impl FnOnce(&mut Assembly)) -> Result<(), OpError> {
    result.solved = solved;
    commit(od, tab, desc, &result, f)
}

fn named_position(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let asm = assembly(od, tab)?.clone();
    let action = a.str("action")?;
    let result = solve(od, tab, &SolveOptions::default())?;
    let pos = || -> Result<usize, OpError> { row(asm.named_positions.iter().map(|p| p.name.as_str()).collect(), a.opt_str("position")?.ok_or_else(|| OpError::param("position", "required"))?, "position") };
    let name_of = |i: usize| asm.named_positions[i].name.clone();
    match action {
        "add" => {
            let name = a.opt_str("name")?.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| sa::next_named_position_name(&asm));
            let np = sa::capture_named_position(&asm, &result, name.clone());
            commit(od, tab, &format!("Insert : {name}"), &result, |a| a.named_positions.push(np))?;
        }
        "update" => {
            let i = pos()?;
            let np = sa::capture_named_position(&asm, &result, name_of(i));
            commit(od, tab, &format!("Update named position : {}", name_of(i)), &result, |a| {
                if let Some(p) = a.named_positions.get_mut(i) {
                    p.placements = np.placements;
                    p.mate_values = np.mate_values;
                }
            })?;
        }
        "apply" => {
            let i = pos()?;
            let solved = sa::apply_named_position(&asm, &result, &asm.named_positions[i]);
            commit_solved(od, tab, &format!("Apply named position : {}", name_of(i)), result, solved, |_| {})?;
        }
        "duplicate" => {
            let i = pos()?;
            let mut np = asm.named_positions[i].clone();
            np.name = sa::duplicate_position_name(&asm, &np.name);
            commit(od, tab, &format!("Duplicate named position : {}", np.name), &result, |a| a.named_positions.insert(i + 1, np))?;
        }
        "delete" => {
            let i = pos()?;
            commit(od, tab, &format!("Delete named position : {}", name_of(i)), &result, |a| {
                if i < a.named_positions.len() {
                    a.named_positions.remove(i);
                }
            })?;
        }
        "rename" => {
            let i = pos()?;
            let name = a.opt_str("name")?.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).ok_or_else(|| OpError::param("name", "the new name"))?;
            commit(od, tab, &format!("Rename named position : {name}"), &result, |a| {
                if let Some(p) = a.named_positions.get_mut(i) {
                    p.name = name.clone();
                }
            })?;
        }
        "move_up" | "move_down" => {
            let i = pos()?;
            let down = action == "move_down";
            let j = if down { i + 1 } else { i.wrapping_sub(1) };
            if j >= asm.named_positions.len() {
                return Err(OpError::param("position", if down { "already the last" } else { "already the first" }));
            }
            commit(od, tab, if down { "Move down" } else { "Move up" }, &result, |a| a.named_positions.swap(i, j))?;
        }
        "set_value" => {
            let i = pos()?;
            let mate = find_mate(&asm, a.opt_str("mate")?.ok_or_else(|| OpError::param("mate", "required"))?).map_err(|e| e.with_param("mate"))?;
            let ix = DOFS.iter().position(|d| Some(*d) == a.opt_str("dof").ok().flatten()).ok_or_else(|| OpError::param("dof", "rz, tx, ty or tz"))?;
            let q = quantity(a.get("value").ok_or_else(|| OpError::param("value", "required"))?, if ix == 0 { Dim::ANGLE } else { Dim::LENGTH }, &units, "value")?;
            let current = result.solved.mate_values.get(&mate).copied().unwrap_or_default();
            commit(od, tab, "Named position value", &result, |a| sa::set_position_value(a, i, mate, ix, q, current))?;
        }
        _ => return Err(OpError::param("action", &POSITION_ACTIONS.join(", "))),
    }
    let now = assembly(od, tab)?;
    Ok(json!({ "named_positions": now.named_positions.iter().map(|p| json!({ "name": p.name, "mate_values": p.mate_values.iter().map(|(m, v)| json!({ "mate": format!("mate:{}", m.0), "rz_deg": v[0].to_degrees(), "t": [v[1] * len_scale(&units), v[2] * len_scale(&units), v[3] * len_scale(&units)] })).collect::<Vec<_>>() })).collect::<Vec<_>>() }).into())
}

fn drivers(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let mut picked: Vec<(MateId, u8)> = Vec::new();
    for (i, d) in a.array("drivers")?.iter().enumerate() {
        let field = format!("drivers[{i}]");
        let m = find_mate(&asm, d.get("mate").and_then(Value::as_str).ok_or_else(|| OpError::param(&field, "{\"mate\", \"dof\"}"))?).map_err(|e| e.with_param(field.clone()))?;
        let ix = DOFS.iter().position(|x| Some(*x) == d.get("dof").and_then(Value::as_str)).ok_or_else(|| OpError::param(&format!("{field}.dof"), "rz, tx, ty or tz"))? as u8;
        let t = asm.mate(m).map(|x| x.mate_type).ok_or_else(|| OpError::param(&field, "not a mate"))?;
        if !sa::driven_dofs(t).contains(&ix) {
            return Err(OpError::param(&format!("{field}.dof"), &format!("this mate's driving values: {}", sa::driven_dofs(t).iter().map(|i| DOFS[*i as usize]).collect::<Vec<_>>().join(", "))));
        }
        picked.push((m, ix));
    }
    // The mates' order, each mate's values in the selector's order.
    let mut mates: Vec<MateId> = Vec::new();
    for (m, _) in &picked {
        if !mates.contains(m) {
            mates.push(*m);
        }
    }
    let mut list = Vec::new();
    for m in mates {
        let t = asm.mate(m).map(|x| x.mate_type).expect("checked");
        for ix in sa::driven_dofs(t) {
            if picked.contains(&(m, *ix)) {
                list.push((m, *ix));
            }
        }
    }
    let result = solve(od, tab, &SolveOptions::default())?;
    let out: Vec<Value> = list.iter().map(|(m, ix)| json!({ "mate": format!("mate:{}", m.0), "dof": DOFS[*ix as usize] })).collect();
    commit(od, tab, "Driving mate selector", &result, |a| a.position_drivers = list)?;
    Ok(json!({ "drivers": out }).into())
}

fn view_row(asm: &Assembly, key: &str) -> Result<usize, OpError> {
    row(asm.exploded_views.iter().map(|v| v.name.as_str()).collect(), key, "view")
}

fn step_row(asm: &Assembly, v: usize, a: &Args) -> Result<Option<usize>, OpError> {
    match a.opt_u64("step")? {
        Some(k) if k >= 1 && (k as usize) <= asm.exploded_views[v].steps.len() => Ok(Some(k as usize - 1)),
        Some(_) => Err(OpError::param("step", &format!("1..{}", asm.exploded_views[v].steps.len()))),
        None => Ok(None),
    }
}

fn exploded_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let result = solve(od, tab, &SolveOptions::default())?;
    let name = a.opt_str("name")?.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    match a.str("action")? {
        "add" => {
            let n = sa::next_exploded_view_name(&asm);
            let view = ExplodedView { name: n.clone(), steps: vec![] };
            commit(od, tab, &format!("Insert : {n}"), &result, |a| a.exploded_views.push(view))?;
            if let Some(name) = name {
                let i = asm.exploded_views.len();
                commit(od, tab, &format!("Rename : {name}"), &result, |a| a.exploded_views[i].name = name.clone())?;
            }
        }
        "rename" => {
            let v = view_row(&asm, a.opt_str("view")?.ok_or_else(|| OpError::param("view", "required"))?)?;
            let name = name.ok_or_else(|| OpError::param("name", "the new name"))?;
            let step = step_row(&asm, v, a)?;
            commit(od, tab, &format!("Rename : {name}"), &result, |a| match step {
                Some(k) => a.exploded_views[v].steps[k].name = Some(name.clone()),
                None => a.exploded_views[v].name = name.clone(),
            })?;
        }
        "delete" => {
            let v = view_row(&asm, a.opt_str("view")?.ok_or_else(|| OpError::param("view", "required"))?)?;
            match step_row(&asm, v, a)? {
                Some(k) => commit(od, tab, "Delete : Explode step", &result, |a| {
                    a.exploded_views[v].steps.remove(k);
                })?,
                None => commit(od, tab, &format!("Delete : {}", asm.exploded_views[v].name), &result, |a| {
                    a.exploded_views.remove(v);
                })?,
            }
        }
        _ => return Err(OpError::param("action", "add, rename or delete")),
    }
    let now = assembly(od, tab)?;
    Ok(json!({ "exploded_views": json(now).1 }).into())
}

fn explode_step(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let asm = assembly(od, tab)?.clone();
    let v = view_row(&asm, a.str("view")?)?;
    let editing = step_row(&asm, v, a)?;
    let ids: Vec<_> = a.array("instances")?.iter().enumerate().map(|(i, x)| find_instance(&asm, x.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<_, _>>()?;
    if ids.is_empty() {
        return Err(OpError::param("instances", "at least one instance"));
    }
    let result = solve(od, tab, &SolveOptions::default())?;
    // Direction: a vector, or a connector's Z axis (its origin is the pivot).
    let (dir, origin) = match a.get("direction") {
        Some(Value::Array(c)) => (DVec3::new(c.first().and_then(Value::as_f64).unwrap_or(0.0), c.get(1).and_then(Value::as_f64).unwrap_or(0.0), c.get(2).and_then(Value::as_f64).unwrap_or(0.0)), None),
        Some(o @ Value::Object(_)) => {
            let c = resolve_entity(&result, parse_connector(&asm, od, tab, o, "direction")?, &units);
            let f = result.connector_world(&c, &units).ok_or_else(|| OpError::param("direction", "the connector doesn't resolve"))?;
            (f.z(), Some(f.origin))
        }
        _ => return Err(OpError::param("direction", "[x, y, z] or a mate connector")),
    };
    let dir = dir.normalize_or_zero();
    if dir == DVec3::ZERO {
        return Err(OpError::param("direction", "a non-zero vector"));
    }
    let mut step = match (a.get("distance"), a.get("angle")) {
        (Some(d), None) => ExplodeStep::translate(ids, (dir * quantity(d, Dim::LENGTH, &units, "distance")?).into()),
        (None, Some(t)) => {
            let pivot = match a.opt_array("pivot")? {
                Some(p) => Some([p.first().and_then(Value::as_f64).unwrap_or(0.0) / k, p.get(1).and_then(Value::as_f64).unwrap_or(0.0) / k, p.get(2).and_then(Value::as_f64).unwrap_or(0.0) / k]),
                None => origin.map(Into::into),
            };
            let mut s = ExplodeStep::rotate(ids, dir.into(), quantity(t, Dim::ANGLE, &units, "angle")?, pivot.unwrap_or_default());
            s.pivot = pivot;
            s
        }
        _ => return Err(OpError::invalid("give `distance` (translate) or `angle` (rotate)")),
    };
    step.lines = a.bool_or("lines", true)?;
    commit(od, tab, "Edit : Explode step", &result, |a| {
        let steps = &mut a.exploded_views[v].steps;
        match editing {
            Some(k) => {
                step.name = steps[k].name.clone();
                step.line_refs = std::mem::take(&mut steps[k].line_refs);
                steps[k] = step;
            }
            None => steps.push(step),
        }
    })?;
    let now = assembly(od, tab)?;
    Ok(json!({ "view": now.exploded_views[v].name, "steps": now.exploded_views[v].steps.len() }).into())
}

fn display_state(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let result = solve(od, tab, &SolveOptions::default())?;
    let state = || -> Result<usize, OpError> { row(asm.display_states.iter().map(|d| d.name.as_str()).collect(), a.opt_str("state")?.ok_or_else(|| OpError::param("state", "required"))?, "state") };
    let new_name = || -> Result<String, OpError> { a.opt_str("name")?.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).ok_or_else(|| OpError::param("name", "a name")) };
    match a.str("action")? {
        "add" => {
            let name = new_name()?;
            if asm.display_states.iter().any(|d| d.name == name) {
                return Err(OpError::param("name", "There is already a display state with this name."));
            }
            commit(od, tab, &format!("Add display state : {name}"), &result, |x| x.save_display_state(&name))?;
        }
        "apply" => {
            let i = state()?;
            commit(od, tab, &format!("Apply display state : {}", asm.display_states[i].name), &result, |x| x.apply_display_state(i))?;
        }
        "update" => {
            let name = asm.display_states[state()?].name.clone();
            commit(od, tab, &format!("Update display state : {name}"), &result, |x| x.save_display_state(&name))?;
        }
        "rename" => {
            let i = state()?;
            let name = new_name()?;
            commit(od, tab, &format!("Rename display state : {name}"), &result, |x| {
                if let Some(d) = x.display_states.get_mut(i) {
                    d.name = name.clone();
                }
            })?;
        }
        "remove" => {
            let i = state()?;
            commit(od, tab, "Remove display state", &result, |x| {
                if i < x.display_states.len() {
                    x.display_states.remove(i);
                }
            })?;
        }
        _ => return Err(OpError::param("action", "add, apply, update, rename or remove")),
    }
    let now = assembly(od, tab)?;
    Ok(json!({
        "display_states": now.display_states.iter().map(|d| json!({ "name": d.name, "hidden_instances": d.hidden_instances.iter().filter_map(|i| now.instance(*i)).map(|i| i.name.clone()).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "hidden_instances": now.instances.iter().filter(|i| i.hidden).map(|i| i.name.clone()).collect::<Vec<_>>(),
    })
    .into())
}
