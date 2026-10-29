//! Assembly features besides mates and patterns: groups, mate relations,
//! mate connectors, replace instance, rename, suppress / hide.

use super::{assembly, asm_field, commit, describe, expr, find_instance, find_mate, parse_connector, resolve_entity, solve, target};
use crate::error::OpError;
use crate::ops::{doc_field, s, strings};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, assembly as sa, doc, refs};
use noshape_model::assembly::eval::SolveOptions;
use noshape_model::assembly::{AsmMateConnector, InstanceSource, MateId, RelationType};
use noshape_model::document::TabContent;
use noshape_model::reference::Reference;
use noshape_units::ExprValue;
use serde_json::{Value, json};

const RELATIONS: [(&str, RelationType); 4] = [("gear", RelationType::Gear), ("rack_and_pinion", RelationType::RackAndPinion), ("screw", RelationType::Screw), ("linear", RelationType::Linear)];

pub fn ops() -> Vec<Op> {
    let connector = json!({ "type": "object" });
    vec![
        Op {
            group: "assembly",
            verb: "group",
            summary: "Group instances so they move as one rigid body",
            description: "The Group dialog (at least two instances). Give `edit` to change an existing group's instances. One edit (\"Insert : Group\" / \"Edit : Group\").",
            fields: vec![doc_field(), asm_field(), req("instances", strings(), "Instance ids or names (two or more)."), opt("edit", s(), "An existing group to edit (id or name).")],
            mutates: true,
            destructive: false,
            run: group,
        },
        Op {
            group: "assembly",
            verb: "relation",
            summary: "Couple two mates: gear, rack and pinion, screw or linear relation",
            description: "The mate relation dialogs. Gear couples two revolute (or cylindrical) mates by `ratio`; rack and pinion a revolute and a slider by the distance per revolution; screw one cylindrical mate's rotation and translation (one mate); linear two sliders. The coupling is measured from the mates' current positions. One edit (\"Insert : Gear relation\").",
            fields: vec![
                doc_field(),
                asm_field(),
                req("type", json!({ "type": "string", "enum": RELATIONS.iter().map(|r| r.0).collect::<Vec<_>>() }), "Relation type."),
                req("mates", strings(), "The two mates (ids or names); screw: one cylindrical mate."),
                opt("ratio", json!({ "type": ["string", "number"] }), "Gear / linear: the ratio (default 1); rack and pinion / screw: the distance per revolution (default 10 mm / 1 mm)."),
                opt("reverse", json!({ "type": "boolean" }), "Reverse direction."),
                opt("edit", s(), "An existing relation to edit (id or name)."),
            ],
            mutates: true,
            destructive: false,
            run: relation,
        },
        Op {
            group: "assembly",
            verb: "mate_connector",
            summary: "Create or edit an assembly mate connector",
            description: "The Mate connector dialog: a connector on an instance's geometry (or the assembly origin), moved with its instance; mates use it as {\"connector\": \"mc:<id>\" or its name}. One edit (\"Insert : Mate connector 1\").",
            fields: vec![
                doc_field(),
                asm_field(),
                opt("on", connector.clone(), "Origin entity (required for a new connector): a mate connector argument as in assembly_mate ({\"instance\", \"entity\", \"inference\"} or {\"origin\": true})."),
                opt("between", connector.clone(), "Origin type Between entities: the origin is halfway between `on` and this entity (same instance)."),
                opt("primary_axis", connector.clone(), "Realign: the Z axis along this entity (face normal, edge direction)."),
                opt("secondary_axis", connector.clone(), "Realign: the X axis toward this entity's direction."),
                opt("flip_primary", json!({ "type": "boolean" }), "Flip primary axis."),
                opt("reorient", json!({ "type": "integer", "minimum": 0, "maximum": 3 }), "Reorient secondary axis: quarter turns about Z."),
                opt("offset", json!({ "type": "object" }), "Move: {\"x\", \"y\", \"z\": distances, \"rotation\": angle, \"rotation_axis\": \"x\" | \"y\" | \"z\" (default z)}, in the connector's frame."),
                opt("hidden", json!({ "type": "boolean" }), "Hide the connector."),
                opt("name", s(), "Name (default \"Mate connector 1\"...)."),
                opt("edit", s(), "An existing assembly mate connector to edit (id or name)."),
            ],
            mutates: true,
            destructive: false,
            run: mate_connector,
        },
        Op {
            group: "assembly",
            verb: "replace",
            summary: "Replace an instance with another part, Part Studio or assembly",
            description: "Replace instance: the instance (with `all`, every instance of the same source) gets the new source and keeps its position; mates whose faces / edges don't exist on the replacement are re-pointed to the nearest geometry of the same type. One edit (\"Replace instance : Part 2\").",
            fields: vec![
                doc_field(),
                asm_field(),
                req("instance", s(), "Instance id or name."),
                req("source", s(), "Part Studio or Assembly tab (name or id)."),
                opt("part", s(), "A part of that Part Studio (`body:…` id or part name); default: the whole Part Studio."),
                opt("configuration", json!({ "type": "object" }), "Configuration of the source: {input id: value}."),
                opt("all", json!({ "type": "boolean" }), "Replace all instances of the same source."),
            ],
            mutates: true,
            destructive: false,
            run: replace,
        },
        Op {
            group: "assembly",
            verb: "rename",
            summary: "Rename an instance, mate, relation, group, pattern or mate connector",
            description: "Rename in the instance list. One edit (\"Rename : <new name>\").",
            fields: vec![doc_field(), asm_field(), opt("instance", s(), "Instance id or name."), opt("feature", s(), "Mate, relation, group, pattern or connector id or name."), req("name", s(), "The new name.")],
            mutates: true,
            destructive: false,
            run: rename,
        },
        Op {
            group: "assembly",
            verb: "set_feature_state",
            summary: "Suppress or unsuppress mates, relations, groups, patterns and mate connectors; show or hide mate connectors",
            description: "The list's Suppress / Unsuppress (\"Suppress mate\", \"Suppress pattern\", \"Suppress mate connector\"; a configured mate changes in the default configuration) and the mate connectors' eye (\"Hide mate connector\"). Features already in the asked state are left alone.",
            fields: vec![doc_field(), asm_field(), req("features", strings(), "Feature ids or names."), opt("suppressed", json!({ "type": "boolean" }), "Suppress or unsuppress."), opt("hidden", json!({ "type": "boolean" }), "Mate connectors: hide or show.")],
            mutates: true,
            destructive: false,
            run: set_feature_state,
        },
    ]
}

fn instances(asm: &noshape_model::assembly::Assembly, a: &Args, key: &str) -> Result<Vec<noshape_model::assembly::InstanceId>, OpError> {
    a.array(key)?.iter().enumerate().map(|(i, v)| find_instance(asm, v.as_str().unwrap_or_default(), &format!("{key}[{i}]"))).collect()
}

fn group(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    // Pattern copies can't be grouped (the dialog drops them).
    let ids: Vec<_> = instances(&asm, a, "instances")?.into_iter().filter(|id| asm.generated_by(*id).is_none()).collect();
    if ids.len() < 2 {
        return Err(OpError::param("instances", sa::GROUP_TOO_FEW));
    }
    let editing = match a.opt_str("edit")? {
        Some(k) => Some(asm.groups.iter().find(|g| g.name == k || format!("group:{}", g.id.0) == k || g.id.0.to_string() == k).map(|g| g.id).ok_or_else(|| OpError::not_found(format!("no group `{k}`")).with_param("edit"))?),
        None => None,
    };
    let result = solve(od, tab, &SolveOptions::default())?;
    let id = commit(od, tab, sa::group_description(editing.is_some()), &result, |a| sa::commit_group(a, editing, ids))?;
    let name = assembly(od, tab)?.groups.iter().find(|g| g.id == id).map(|g| g.name.clone()).unwrap_or_default();
    let mut v = describe(od, tab)?;
    v["group"] = json!(format!("group:{}", id.0));
    v["name"] = json!(name);
    Ok(v.into())
}

/// assembly_list's `relations` (qa API-ASM-RELATIONS): each relation's
/// name, type, mates, ratio, reverse and suppressed, with the solve's state
/// (ok, inactive, message: "One or more mates in this relation are invalid.").
pub(super) fn relations_json(a: &noshape_model::assembly::Assembly, r: &noshape_model::assembly::eval::AssemblyResult) -> Vec<Value> {
    a.relations
        .iter()
        .map(|x| {
            let st = r.relations.iter().find(|s| s.id == x.id);
            json!({
                "id": format!("mate:{}", x.id.0),
                "name": x.name,
                "type": RELATIONS.iter().find(|k| k.1 == x.relation_type).map(|k| k.0),
                "mates": x.mates.iter().map(|m| format!("mate:{}", m.0)).collect::<Vec<_>>(),
                "ratio": x.ratio.text,
                "reverse": x.reverse,
                "suppressed": x.suppressed,
                "ok": st.map(|s| s.ok),
                "inactive": st.map(|s| s.inactive),
                "message": st.and_then(|s| s.message.clone()),
            })
        })
        .collect()
}

fn relation(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let ty = a.str("type")?.to_lowercase().replace([' ', '-'], "_");
    let kind = RELATIONS.iter().find(|r| r.0 == ty).map(|r| r.1).ok_or_else(|| OpError::param("type", "gear, rack_and_pinion, screw or linear"))?;
    let mates: Vec<MateId> = a.array("mates")?.iter().map(|v| find_mate(&asm, v.as_str().unwrap_or_default())).collect::<Result<_, _>>()?;
    let mates = sa::check_relation(&asm, kind, mates.first().copied(), mates.get(1).copied()).map_err(|(_, e)| OpError::param("mates", &e))?;
    let editing = match a.opt_str("edit")? {
        Some(k) => Some(asm.relations.iter().find(|r| r.name == k || format!("mate:{}", r.id.0) == k || r.id.0.to_string() == k).map(|r| r.id).ok_or_else(|| OpError::not_found(format!("no relation `{k}`")).with_param("edit"))?),
        None => None,
    };
    let ratio = match a.get("ratio") {
        Some(Value::String(t)) => t.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => editing.and_then(|id| asm.relations.iter().find(|r| r.id == id)).map(|r| r.ratio.text.clone()).unwrap_or_else(|| sa::relation_default_ratio(kind).into()),
    };
    let reverse = a.bool_or("reverse", editing.and_then(|id| asm.relations.iter().find(|r| r.id == id)).is_some_and(|r| r.reverse))?;
    let result = solve(od, tab, &SolveOptions::default())?;
    let base = result.relation_base(&asm, kind, mates);
    let id = commit(od, tab, &sa::relation_description(kind), &result, |a| sa::commit_relation(a, editing, kind, mates, ExprValue::new(ratio), reverse, base))?;
    let name = assembly(od, tab)?.relations.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_default();
    let mut v = describe(od, tab)?;
    v["relation"] = json!(format!("mate:{}", id.0));
    v["name"] = json!(name);
    Ok(v.into())
}

fn mate_connector(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let asm = assembly(od, tab)?.clone();
    let before = solve(od, tab, &SolveOptions::default())?;
    let existing: Option<AsmMateConnector> = match a.opt_str("edit")? {
        Some(k) => Some(asm.connectors.iter().find(|c| c.name == k || format!("mc:{}", c.id.0) == k || c.id.0.to_string() == k).cloned().ok_or_else(|| OpError::not_found(format!("no assembly mate connector `{k}`")).with_param("edit"))?),
        None => None,
    };
    let pick = |od: &crate::OpenDoc, key: &str| connector_arg(&asm, od, tab, &before, &units, a, key);
    let on = match pick(od, "on")? {
        Some(c) => c,
        None => existing.as_ref().map(|c| c.on.clone()).ok_or_else(|| OpError::param("on", "Pick an origin entity (geometry, an inference point or the assembly origin)."))?,
    };
    let (id, name) = match &existing {
        Some(c) => (c.id, a.opt_str("name")?.map(|n| n.trim().to_string()).unwrap_or_else(|| c.name.clone())),
        None => (MateId(asm.next_id()), a.opt_str("name")?.map(|n| n.trim().to_string()).unwrap_or_else(|| asm.next_feature_name("Mate connector"))),
    };
    let mut c = existing.clone().unwrap_or_else(|| AsmMateConnector::new(id, name.clone(), on.clone()));
    c.name = name.clone();
    c.on = on;
    if let Some(f) = a.get("flip_primary").and_then(Value::as_bool) {
        c.flip_primary = f;
    }
    if let Some(r) = a.get("reorient").and_then(Value::as_u64) {
        c.reorient = (r % 4) as u8;
    }
    if let Some(h) = a.get("hidden").and_then(Value::as_bool) {
        c.hidden = h;
    }
    if let Some(b) = pick(od, "between")? {
        c.between = Some(b);
    }
    let (p, q) = (pick(od, "primary_axis")?, pick(od, "secondary_axis")?);
    if p.is_some() || q.is_some() {
        c.primary_axis = p.or(c.primary_axis);
        c.secondary_axis = q.or(c.secondary_axis);
    }
    if let Some(off) = a.object("offset")? {
        c.translation = [expr(off, "x"), expr(off, "y"), expr(off, "z")];
        c.rotation = expr(off, "rotation");
        if let Some(ax) = off.get("rotation_axis").and_then(Value::as_str) {
            c.rotation_axis = ["x", "y", "z"].iter().position(|x| x.eq_ignore_ascii_case(ax)).ok_or_else(|| OpError::param("offset.rotation_axis", "x, y or z"))? as u8;
        }
        // The dialog's Move box: on when there is something to move by.
        c.move_off = c.translation.iter().all(Option::is_none) && c.rotation.is_none();
    }
    let desc = if existing.is_some() { format!("Edit : {name}") } else { format!("Insert : {name}") };
    commit(od, tab, &desc, &before, |a| sa::commit_connector(a, c))?;
    let mut v = describe(od, tab)?;
    v["connector"] = json!(format!("mc:{}", id.0));
    v["name"] = json!(name);
    Ok(v.into())
}

/// A connector argument for the Mate connector dialog: geometry or the
/// origin, not another connector.
fn connector_arg(asm: &noshape_model::assembly::Assembly, od: &crate::OpenDoc, tab: noshape_model::document::TabId, before: &noshape_model::assembly::eval::AssemblyResult, units: &noshape_units::UnitPrefs, a: &Args, key: &str) -> Result<Option<noshape_model::assembly::MateConnectorRef>, OpError> {
    match a.get(key) {
        Some(v) if !v.is_null() => {
            let c = resolve_entity(before, parse_connector(asm, od, tab, v, key)?, units);
            if c.connector.is_some() {
                return Err(OpError::param(key, "Pick geometry or the origin for a mate connector."));
            }
            Ok(Some(c))
        }
        _ => Ok(None),
    }
}

fn replace(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let id = find_instance(&asm, a.str("instance")?, "instance")?;
    let src = doc::find_tab(od.doc(), a.str("source")?).map_err(|e| e.with_param("source"))?;
    let config = crate::ops::source_config_arg(od, src, a)?;
    let src_name = od.doc().tab(src).map(|t| t.name.clone()).unwrap_or_default();
    let before = solve(od, tab, &SolveOptions::default())?;
    let (source, base, studio) = match od.doc().tab(src).map(|t| &t.content) {
        Some(TabContent::PartStudio(_)) => {
            let r = od.regenerate(src, Some(&config))?;
            match a.opt_str("part")? {
                Some(p) => {
                    let b = r.display.bodies.iter().find(|b| matches!(refs::parse_str(p), Some(Reference::Body(id)) if id == b.id) || b.name == p).ok_or_else(|| OpError::param("part", "no such part in the source Part Studio (see query_parts)"))?;
                    (InstanceSource::Part { studio: src, body: b.id.clone(), config }, b.name.clone(), Some((r.context.clone(), Some(b.id.clone()))))
                }
                None => (InstanceSource::PartStudio { studio: src, config }, src_name, Some((r.context.clone(), None))),
            }
        }
        Some(TabContent::Assembly(_)) if src != tab => (InstanceSource::Assembly { assembly: src, config }, src_name, None),
        _ => return Err(OpError::param("source", "a Part Studio or another Assembly")),
    };
    let all = a.bool_or("all", false)?;
    let (count, remapped) = commit(od, tab, &sa::replace_description(&base), &before, |asm| sa::replace_instance(asm, id, source, &base, all, &before, studio.as_ref().map(|(c, b)| (&**c, b.as_ref()))))?;
    let mut v = describe(od, tab)?;
    v["message"] = json!(sa::replace_message(count, remapped));
    Ok(v.into())
}

fn rename(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "a name"));
    }
    let (inst, feat) = match (a.opt_str("instance")?, a.opt_str("feature")?) {
        (Some(i), None) => (Some(find_instance(&asm, i, "instance")?), None),
        (None, Some(f)) => (None, Some(find_mate(&asm, f).map_err(|e| e.with_param("feature"))?)),
        _ => return Err(OpError::invalid("give `instance` or `feature`")),
    };
    // Renaming an instance to its own name is no edit.
    if !inst.and_then(|i| asm.instance(i)).is_some_and(|i| i.name == name) {
        let result = solve(od, tab, &SolveOptions::default())?;
        let n = name.clone();
        commit(od, tab, &format!("Rename : {name}"), &result, move |a| a.rename(inst, feat, &n))?;
    }
    Ok(describe(od, tab)?.into())
}

fn set_feature_state(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let ids: Vec<MateId> = a.array("features")?.iter().map(|v| find_mate(&asm, v.as_str().unwrap_or_default()).map_err(|e| e.with_param("features"))).collect::<Result<_, _>>()?;
    let (sup, hid) = (a.get("suppressed").and_then(Value::as_bool), a.get("hidden").and_then(Value::as_bool));
    if sup.is_none() && hid.is_none() {
        return Err(OpError::invalid("give `suppressed` or `hidden`"));
    }
    let values = asm.configuration.defaults();
    for id in ids {
        let asm = assembly(od, tab)?.clone();
        let conf = asm.configured(&values);
        if let Some(want) = sup {
            let now = conf.mate(id).map(|m| m.suppressed).or_else(|| asm.relations.iter().find(|r| r.id == id).map(|r| r.suppressed)).or_else(|| asm.groups.iter().find(|g| g.id == id).map(|g| g.suppressed));
            let pattern = asm.pattern(id).map(|p| p.suppressed);
            let connector = asm.connectors.iter().find(|c| c.id == id).map(|c| c.suppressed);
            let result = solve(od, tab, &SolveOptions::default())?;
            match (now, pattern, connector) {
                (Some(now), _, _) if now != want => commit(od, tab, "Suppress mate", &result, |a| sa::toggle_mate_suppressed(a, id, &values))?,
                (None, Some(now), _) if now != want => commit(od, tab, "Suppress pattern", &result, |a| {
                    if let Some(p) = a.patterns.iter_mut().find(|p| p.id == id) {
                        p.suppressed = !p.suppressed;
                    }
                })?,
                // A mate connector's menu › Suppress / Unsuppress.
                (None, None, Some(now)) if now != want => commit(od, tab, "Suppress mate connector", &result, |a| {
                    if let Some(c) = a.connectors.iter_mut().find(|c| c.id == id) {
                        c.suppressed = want;
                    }
                })?,
                (None, None, None) => return Err(OpError::param("features", "only mates, relations, groups, patterns and mate connectors can be suppressed")),
                _ => {}
            }
        }
        if let Some(want) = hid {
            let Some(now) = asm.connectors.iter().find(|c| c.id == id).map(|c| c.hidden) else {
                return Err(OpError::param("features", "only assembly mate connectors are hidden here (instances: assembly_set_state)"));
            };
            if now != want {
                let result = solve(od, tab, &SolveOptions::default())?;
                commit(od, tab, if now { "Show mate connector" } else { "Hide mate connector" }, &result, |a| {
                    if let Some(c) = a.connectors.iter_mut().find(|c| c.id == id) {
                        c.hidden = want;
                    }
                })?;
            }
        }
    }
    Ok(describe(od, tab)?.into())
}
