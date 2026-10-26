//! Modeling in context, both sides: the assembly's Edit in context, Create
//! Part Studio in context, Update context and Set as primary instance, and
//! the Part Studio's Assembly contexts (show / None, Update, Rename,
//! Delete, Exit context, Insert and go to assembly). Shared code in
//! `crate::in_context`; context geometry is listed by query_entities with
//! `context: true` and referenced like any face, edge or vertex.

use super::assembly::{asm_target, find_instance, solve_assembly};
use super::{doc_field, s, strings, studio_target};
use crate::error::OpError;
use crate::in_context as ic;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, doc};
use noshape_model::assembly::eval::SolveOptions;
use noshape_model::assembly::{Assembly, InstanceId, InstanceSource};
use noshape_model::document::{TabContent, TabId};
use noshape_model::in_context::AssemblyContext;
use noshape_model::partstudio::PartStudio;
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    let asm_tab = || opt("tab", s(), "Assembly tab, by name or id. Default: the first Assembly.");
    let ps_tab = || opt("tab", s(), "Part Studio tab, by name or id. Default: the first Part Studio.");
    let context = || req("context", s(), "Context name (\"Context 1\") or id (context_list).");
    vec![
        Op {
            group: "assembly",
            verb: "edit_in_context",
            summary: "Edit in context: open an instance's Part Studio with the assembly around it as context geometry",
            description: "Instance › Edit in context (help: Modeling in context): the instance's Part Studio shows the rest of the assembly, placed around the part, as its Assembly context. Without `context`, a new snapshot \"Context N\" (Edit in context, or Create new context once the Part Studio has contexts); with `context`, that existing context of this assembly (Edit Context N). List its geometry with query_entities `context: true` and reference it in features; a context no feature references goes away on context_exit. Not undoable, like the app. Replies with the Part Studio and the context (context_list's fields).",
            fields: vec![doc_field(), asm_tab(), req("instance", s(), "A part or Part Studio instance (id or name)."), opt("context", s(), "An existing context of this assembly to edit in (default: a new one).")],
            mutates: true,
            destructive: false,
            run: edit_in_context,
        },
        Op {
            group: "assembly",
            verb: "create_part_studio_in_context",
            summary: "Create Part Studio in context: a new Part Studio with the assembly as context, its origin at a connector",
            description: "Toolbar › Create Part Studio in context and its \"Origin of new Part Studio\" dialog: a new Part Studio (\"Part Studio N\", after the assembly) whose origin sits at `origin` (an assembly mate connector, id or name; default the assembly origin), showing the whole assembly as its Assembly context. Model there, then context_insert_in_assembly places its parts at that origin. One edit (\"Create Part Studio in context\").",
            fields: vec![doc_field(), asm_tab(), opt("origin", s(), "An assembly mate connector (assembly_mate_connector) for the new Part Studio's origin.")],
            mutates: true,
            destructive: false,
            run: create_in_context,
        },
        Op {
            group: "assembly",
            verb: "update_context",
            summary: "Update context: re-snapshot a Part Studio's Assembly context from the assembly as it is now",
            description: "Instance › Update context ▸ <context>: a new snapshot around the context's primary instance (or this instance when that one is gone); features referencing it regenerate. `out_of_date` in context_list says when an update is available. One edit (\"Update context : Context 1\").",
            fields: vec![doc_field(), asm_tab(), req("instance", s(), "An instance of the context's Part Studio."), context()],
            mutates: true,
            destructive: false,
            run: asm_update_context,
        },
        Op {
            group: "assembly",
            verb: "set_primary_instance",
            summary: "Set as primary instance: re-anchor a context on another instance of the same part",
            description: "Instance › Set as primary instance ▸ <context> (other instances of the context's part): the context is re-anchored on this instance and re-snapshotted around it (\"Update context : Context 1\").",
            fields: vec![doc_field(), asm_tab(), req("instance", s(), "Another instance of the context's part."), context()],
            mutates: true,
            destructive: false,
            run: set_primary,
        },
        Op {
            group: "context",
            verb: "list",
            summary: "A Part Studio's Assembly contexts",
            description: "The Assembly contexts section above the Feature list: each context's `id`, `name`, `assembly`, whether it is `shown` and `referenced` (only referenced contexts persist), `out_of_date` (an update is available), its `follow_position` (Follow position ▸, whose only choice is \"Current assembly position\"), its `primary` instance, whether it has an `origin` (created in context: context_insert_in_assembly works) and its `instances` (instance and part names).",
            fields: vec![doc_field(), ps_tab()],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "context",
            verb: "show",
            summary: "Show an Assembly context, or None",
            description: "The Assembly contexts dropdown: show `context`, or None without it (the context stays). Not undoable (\"Assembly context\").",
            fields: vec![doc_field(), ps_tab(), opt("context", json!({ "type": ["string", "null"] }), "Context name or id; omitted or null: None.")],
            mutates: true,
            destructive: false,
            run: show,
        },
        Op {
            group: "context",
            verb: "update",
            summary: "Update context: re-snapshot the assembly around the context's primary instance",
            description: "Context ⋯ › Update context (\"Update context : Context 1\"); features referencing it regenerate.",
            fields: vec![doc_field(), ps_tab(), context()],
            mutates: true,
            destructive: false,
            run: update,
        },
        Op {
            group: "context",
            verb: "rename",
            summary: "Rename an Assembly context",
            description: "Context ⋯ › Rename… (\"Rename context\").",
            fields: vec![doc_field(), ps_tab(), context(), req("name", s(), "The new name.")],
            mutates: true,
            destructive: false,
            run: rename,
        },
        Op {
            group: "context",
            verb: "delete",
            summary: "Delete an Assembly context",
            description: "Context ⋯ › Delete (\"Delete context\"); features referencing its geometry lose those references.",
            fields: vec![doc_field(), ps_tab(), context()],
            mutates: true,
            destructive: true,
            run: delete,
        },
        Op {
            group: "context",
            verb: "exit",
            summary: "Exit context: stop showing the assembly",
            description: "The in-context banner's ✕ (or going back to the assembly): no context shown; a context no feature references goes away. Not undoable (\"Exit context\").",
            fields: vec![doc_field(), ps_tab()],
            mutates: true,
            destructive: false,
            run: exit,
        },
        Op {
            group: "context",
            verb: "insert_in_assembly",
            summary: "Insert and go to assembly: insert a Part Studio created in context at its origin",
            description: "The in-context banner's Insert and go to assembly and its \"Select items to insert\" dialog, in a Part Studio created in context (assembly_create_part_studio_in_context): `parts` and `sketches` (names) become instances of the context's assembly at the Part Studio's origin. One edit on the assembly (\"Insert and go to assembly\"); replies with the new instance ids.",
            fields: vec![doc_field(), ps_tab(), opt("parts", strings(), "Part names (query_parts)."), opt("sketches", strings(), "Sketch names.")],
            mutates: true,
            destructive: false,
            run: insert_in_assembly,
        },
    ]
}

fn find_context<'a>(ps: &'a PartStudio, key: &str) -> Result<&'a AssemblyContext, OpError> {
    let k = key.trim();
    ps.contexts
        .iter()
        .find(|c| c.name == k || c.id.to_string() == k)
        .ok_or_else(|| OpError::not_found(format!("no context `{key}`; contexts: {}", ps.contexts.iter().map(|c| format!("{} ({})", c.name, c.id)).collect::<Vec<_>>().join(", "))).with_param("context"))
}

fn assembly_of(doc: &noshape_model::Document, tab: TabId) -> Option<&Assembly> {
    match doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::Assembly(a)) => Some(a),
        _ => None,
    }
}

/// A context as context_list shows it.
fn context_json(od: &mut OpenDoc, ps: &PartStudio, c: &AssemblyContext) -> Value {
    let referenced = noshape_model::in_context::referenced_contexts(&ps.features).contains(&c.id);
    let asm = assembly_of(od.doc(), c.assembly).cloned();
    let out_of_date = asm.as_ref().is_some_and(|a| ic::out_of_date(c, &solve_assembly(od, c.assembly, a, &SolveOptions::default())));
    let primary = c.primary.first().and_then(|p| asm.as_ref()?.instance(InstanceId(*p))).map(|i| i.name.clone());
    json!({
        "id": c.id,
        "name": c.name,
        "assembly": od.doc().tab(c.assembly).map(|t| t.name.clone()),
        "shown": ps.active_context == Some(c.id),
        "referenced": referenced,
        "out_of_date": out_of_date,
        // The context menu's Follow position ▸: its only choice, checked.
        "follow_position": "Current assembly position",
        "primary": primary,
        "origin": c.origin.is_some(),
        "instances": c.bodies.iter().map(|b| json!({ "instance": b.instance, "part": b.part, "hidden": c.hidden.contains(&b.path) })).collect::<Vec<_>>(),
    })
}

fn edit_studio_not_undoable(od: &mut OpenDoc, studio: TabId, description: &str, f: impl FnOnce(&mut PartStudio)) -> Result<(), OpError> {
    od.session.edit_document_not_undoable(description, |d| {
        if let Some(TabContent::PartStudio(ps)) = d.tab_mut(studio).map(|t| &mut t.content) {
            f(ps);
        }
    })?;
    Ok(())
}

fn studio_reply(od: &mut OpenDoc, studio: TabId, id: u32) -> Result<OpOutput, OpError> {
    let ps = doc::studio(od.doc(), studio)?.clone();
    let c = ps.contexts.iter().find(|c| c.id == id).cloned().ok_or_else(|| OpError::invalid("the context is gone"))?;
    let v = context_json(od, &ps, &c);
    Ok(json!({ "part_studio": od.doc().tab(studio).map(|t| t.name.clone()), "context": v }).into())
}

fn edit_in_context(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = asm_target(ctx, a)?;
    let asm = assembly_of(od.doc(), tab).cloned().expect("assembly");
    let top = find_instance(&asm, a.str("instance")?, "instance")?;
    let studio = match &asm.instance(top).expect("found").source {
        InstanceSource::Part { studio, .. } | InstanceSource::PartStudio { studio, .. } => *studio,
        _ => return Err(OpError::param("instance", "Edit in context is for part and Part Studio instances")),
    };
    let ps = doc::studio(od.doc(), studio)?.clone();
    let existing = match a.opt_str("context")? {
        Some(k) => {
            let known = ic::contexts_of(&ps, tab);
            let c = known.iter().find(|c| c.1 == k || c.0.to_string() == k).ok_or_else(|| OpError::param("context", &format!("this assembly's contexts of the Part Studio: {}", if known.is_empty() { "none".into() } else { known.iter().map(|c| c.1.clone()).collect::<Vec<_>>().join(", ") })))?;
            Some(c.0)
        }
        None => None,
    };
    let result = solve_assembly(od, tab, &asm, &SolveOptions::default());
    let name_of = |p: &noshape_model::assembly::eval::InstancePath| asm.instance(p[0]).map(|i| i.name.clone()).unwrap_or_default();
    let (id, new) = ic::context_for(&ps, &result, tab, Some(&vec![top]), existing, glam::DMat4::IDENTITY, &name_of).ok_or_else(|| OpError::param("instance", "the instance has no geometry to edit in context"))?;
    edit_studio_not_undoable(od, studio, "Edit in context", |ps| ic::install(ps, id, new))?;
    studio_reply(od, studio, id)
}

fn create_in_context(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = asm_target(ctx, a)?;
    let asm = assembly_of(od.doc(), tab).cloned().expect("assembly");
    let result = solve_assembly(od, tab, &asm, &SolveOptions::default());
    let frame = match a.opt_str("origin")? {
        Some(k) => {
            let mc = asm.connectors.iter().find(|c| c.name == k || format!("mc:{}", c.id.0) == k || c.id.0.to_string() == k).ok_or_else(|| OpError::not_found(format!("no assembly mate connector `{k}`")).with_param("origin"))?;
            result.connector_frame(mc.id).ok_or_else(|| OpError::param("origin", "the connector doesn't resolve"))?
        }
        None => Default::default(),
    };
    let to_studio = noshape_model::assembly::eval::frame_matrix(&frame).inverse();
    let mut studio = None;
    od.session.edit_document("Create Part Studio in context", |d| {
        let name = d.next_tab_name("Part Studio");
        let id = d.add_tab(&name, TabContent::PartStudio(PartStudio::new()));
        crate::docedit::place_after(d, id, Some(tab));
        studio = Some(id);
    })?;
    let studio = studio.expect("added");
    let ps = doc::studio(od.doc(), studio)?.clone();
    let name_of = |p: &noshape_model::assembly::eval::InstancePath| asm.instance(p[0]).map(|i| i.name.clone()).unwrap_or_default();
    let (id, mut new) = ic::context_for(&ps, &result, tab, None, None, to_studio, &name_of).expect("a new context");
    if let Some(c) = &mut new {
        c.origin = Some(noshape_model::assembly::eval::frame_to_placement(&frame));
    }
    edit_studio_not_undoable(od, studio, "Edit in context", |ps| ic::install(ps, id, new))?;
    studio_reply(od, studio, id)
}

/// The Part Studio an instance comes from, and context `key` of it.
fn instance_context(od: &OpenDoc, asm: &Assembly, a: &Args) -> Result<(InstanceId, TabId, AssemblyContext), OpError> {
    let top = find_instance(asm, a.str("instance")?, "instance")?;
    let studio = asm.instance(top).and_then(|i| i.source.studio()).ok_or_else(|| OpError::param("instance", "not a part or Part Studio instance"))?;
    let ps = doc::studio(od.doc(), studio)?;
    let c = find_context(ps, a.str("context")?)?.clone();
    Ok((top, studio, c))
}

fn commit_update(od: &mut OpenDoc, studio: TabId, fresh: AssemblyContext, old_name: &str) -> Result<(), OpError> {
    let id = fresh.id;
    // A document edit, as in the app (no tab prefix in the history).
    od.session.edit_document(&ic::update_description(old_name), |d| {
        if let Some(TabContent::PartStudio(ps)) = d.tab_mut(studio).map(|t| &mut t.content)
            && let Some(x) = ps.contexts.iter_mut().find(|c| c.id == id)
        {
            *x = fresh;
        }
    })?;
    Ok(())
}

fn asm_update_context(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = asm_target(ctx, a)?;
    let asm = assembly_of(od.doc(), tab).cloned().expect("assembly");
    let (top, studio, old) = instance_context(od, &asm, a)?;
    if old.assembly != tab {
        return Err(OpError::param("context", "a context of another assembly"));
    }
    let result = solve_assembly(od, tab, &asm, &SolveOptions::default());
    let fresh = ic::refreshed(&old, &result, &asm, Some(top)).ok_or_else(|| OpError::param("instance", "the instance has no geometry"))?;
    commit_update(od, studio, fresh, &old.name)?;
    studio_reply(od, studio, old.id)
}

fn set_primary(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = asm_target(ctx, a)?;
    let asm = assembly_of(od.doc(), tab).cloned().expect("assembly");
    let (top, studio, old) = instance_context(od, &asm, a)?;
    if old.primary == vec![top.0] {
        return Err(OpError::param("instance", "already the context's primary instance"));
    }
    let body = match &asm.instance(top).expect("found").source {
        InstanceSource::Part { body, .. } => Some(body.clone()),
        _ => None,
    };
    if old.assembly != tab || body.is_none() || old.primary_body != body {
        return Err(OpError::param("instance", "another instance of the context's part (Set as primary instance)"));
    }
    let mut moved = old.clone();
    moved.primary = vec![top.0];
    edit_studio_not_undoable(od, studio, "Set as primary instance", |ps| {
        if let Some(c) = ps.contexts.iter_mut().find(|c| c.id == old.id) {
            *c = moved.clone();
        }
    })?;
    let result = solve_assembly(od, tab, &asm, &SolveOptions::default());
    let fresh = ic::refreshed(&moved, &result, &asm, Some(top)).ok_or_else(|| OpError::param("instance", "the instance has no geometry"))?;
    commit_update(od, studio, fresh, &old.name)?;
    studio_reply(od, studio, old.id)
}

// ------------------------------------------------------------ Part Studio side

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?.clone();
    let list: Vec<Value> = ps.contexts.iter().map(|c| context_json(od, &ps, c)).collect();
    Ok(json!({ "contexts": list }).into())
}

fn show(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?.clone();
    let id = match a.opt_str("context")? {
        Some(k) => Some(find_context(&ps, k)?.id),
        None => None,
    };
    edit_studio_not_undoable(od, tab, "Assembly context", |ps| ps.active_context = id)?;
    list(ctx, a)
}

fn update(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?.clone();
    let old = find_context(&ps, a.str("context")?)?.clone();
    let asm = assembly_of(od.doc(), old.assembly).cloned().ok_or_else(|| OpError::param("context", "its assembly is gone"))?;
    let result = solve_assembly(od, old.assembly, &asm, &SolveOptions::default());
    let fresh = ic::refreshed(&old, &result, &asm, None).ok_or_else(|| {
        let why = if old.primary.is_empty() { "it has no primary instance (a Part Studio created in context): update it from the assembly (assembly_update_context on an instance of this Part Studio)" } else { "its primary instance is gone; update it from the assembly (assembly_update_context)" };
        OpError::param("context", why)
    })?;
    commit_update(od, tab, fresh, &old.name)?;
    list(ctx, a)
}

fn rename(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?.clone();
    let id = find_context(&ps, a.str("context")?)?.id;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "a name"));
    }
    od.session.edit_document("Rename context", |d| {
        if let Some(TabContent::PartStudio(ps)) = d.tab_mut(tab).map(|t| &mut t.content)
            && let Some(c) = ps.contexts.iter_mut().find(|c| c.id == id)
        {
            c.name = name;
        }
    })?;
    list(ctx, a)
}

fn delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?.clone();
    let id = find_context(&ps, a.str("context")?)?.id;
    od.session.edit_document("Delete context", |d| {
        if let Some(TabContent::PartStudio(ps)) = d.tab_mut(tab).map(|t| &mut t.content) {
            ps.contexts.retain(|c| c.id != id);
            if ps.active_context == Some(id) {
                ps.active_context = None;
            }
        }
    })?;
    list(ctx, a)
}

fn exit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let id = doc::studio(od.doc(), tab)?.active_context.ok_or_else(|| OpError::invalid("no context is shown"))?;
    edit_studio_not_undoable(od, tab, "Exit context", |ps| ic::exit(ps, id))?;
    list(ctx, a)
}

fn insert_in_assembly(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?.clone();
    let c = ps.active_context.and_then(|id| ps.contexts.iter().find(|c| c.id == id)).ok_or_else(|| OpError::invalid("no context is shown (Insert and go to assembly is for a Part Studio created in context)"))?;
    let at = c.origin.ok_or_else(|| OpError::invalid("the Part Studio was not created in context (assembly_create_part_studio_in_context); insert it with assembly_insert"))?;
    let asm_tab = c.assembly;
    let r = od.regenerate(tab, None)?;
    let names = |k: &str| -> Result<Vec<String>, OpError> { a.opt_array(k)?.map(|l| l.iter().enumerate().map(|(i, v)| v.as_str().map(str::to_string).ok_or_else(|| OpError::param(&format!("{k}[{i}]"), "a name"))).collect()).unwrap_or(Ok(Vec::new())) };
    let mut parts = Vec::new();
    for (i, n) in names("parts")?.into_iter().enumerate() {
        let b = r.display.bodies.iter().find(|b| b.name == n).ok_or_else(|| OpError::param(&format!("parts[{i}]"), &format!("the Part Studio's parts: {}", r.display.bodies.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(", "))))?;
        parts.push((b.id.clone(), b.name.clone()));
    }
    let mut sketches = Vec::new();
    for (i, n) in names("sketches")?.into_iter().enumerate() {
        let f = ps.features.iter().find(|f| f.type_id == "sketch" && f.name == n).ok_or_else(|| OpError::param(&format!("sketches[{i}]"), "a sketch of the Part Studio"))?;
        sketches.push((f.id, f.name.clone()));
    }
    if parts.is_empty() && sketches.is_empty() {
        return Err(OpError::param("parts", "Select items to insert: give `parts` and / or `sketches`"));
    }
    let asm = assembly_of(od.doc(), asm_tab).cloned().ok_or_else(|| OpError::invalid("the context's assembly is gone"))?;
    let result = solve_assembly(od, asm_tab, &asm, &SolveOptions::default());
    let mut ids = Vec::new();
    od.session.edit_tab(asm_tab, "Insert and go to assembly", |content| {
        if let TabContent::Assembly(x) = content {
            crate::assembly::fold_solved(x, &result);
            ids = ic::insert_from_context(x, tab, &parts, &sketches, at);
        }
    })?;
    Ok(json!({ "assembly": od.doc().tab(asm_tab).map(|t| t.name.clone()), "instances": ids.iter().map(|i| format!("inst:{}", i.0)).collect::<Vec<_>>() }).into())
}
