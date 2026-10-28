//! The instance and mate menus: subassemblies (Move to new / Create empty /
//! Dissolve), the instance list's drag and drop, folders, Copy and paste, Change configuration, Lock / follow
//! position, Instance properties, tessellation and Dynamic suppression.

use super::{assembly, asm_field, commit, describe, find_instance, find_mate, solve, target};
use crate::error::OpError;
use crate::ops::{config_arg, doc_field, s, strings};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, assembly as sa};
use noshape_model::assembly::eval::{AssemblyResult, SolveOptions};
use noshape_model::assembly::ops::InstanceClipboard;
use noshape_model::assembly::{Assembly, InstanceId, InstanceSource, MateId, SubassemblyLock};
use noshape_model::document::{TabContent, TabId};
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    let instances = || req("instances", strings(), "Instance ids or names.");
    let folder = || req("folder", s(), "Folder id or name (assembly_list `folders`).");
    vec![
        Op {
            group: "assembly",
            verb: "move_to_subassembly",
            summary: "Move instances to a new subassembly",
            description: "Move to new subassembly: a new Assembly tab holds the instances and the mates among them, and one instance of it takes their place. Patterns whose seeds and references all move go along with their copies.",
            fields: vec![doc_field(), asm_field(), instances()],
            mutates: true,
            destructive: false,
            run: move_to_subassembly,
        },
        Op {
            group: "assembly",
            verb: "create_subassembly",
            summary: "Create an empty subassembly",
            description: "Create empty subassembly: a new, empty Assembly tab and an instance of it here.",
            fields: vec![doc_field(), asm_field()],
            mutates: true,
            destructive: false,
            run: create_subassembly,
        },
        Op {
            group: "assembly",
            verb: "dissolve_subassembly",
            summary: "Dissolve subassemblies into this assembly",
            description: "Dissolve subassembly: the subassembly's instances and mates move up into this assembly; its tab is emptied when nothing else uses it.",
            fields: vec![doc_field(), asm_field(), instances()],
            mutates: true,
            destructive: false,
            run: dissolve_subassembly,
        },
        Op {
            group: "assembly",
            verb: "move_instance",
            summary: "Drag instances in the instance list: reorder, into a folder or a subassembly",
            description: "The instance list's drag and drop, one of: `before` (dropped on an instance row: placed above it, joining its folder; \"end\": the end of the list) → \"Reorder instances\"; `folder` (dropped on a folder row) → \"Add to folder\"; `subassembly` (dropped on a subassembly instance: they move into that subassembly, keeping their places, with the mates among them) → \"Move to subassembly\". Pattern and mirror copies don't move into a subassembly.",
            fields: vec![
                doc_field(),
                asm_field(),
                instances(),
                opt("before", s(), "Instance id or name to drop above, or \"end\"."),
                opt("folder", s(), "Folder id or name (assembly_list `folders`)."),
                opt("subassembly", s(), "Subassembly instance id or name."),
            ],
            mutates: true,
            destructive: false,
            run: move_instance,
        },
        Op {
            group: "assembly",
            verb: "add_folder",
            summary: "Add instances or mate features to a new folder",
            description: "Add selection to folder… in the instance list or the Mate features list. Rename it with assembly_rename (`feature`: the folder), unpack it with assembly_unpack_folder.",
            fields: vec![doc_field(), asm_field(), opt("instances", strings(), "Instance ids or names."), opt("features", strings(), "Mate, relation, group, pattern or connector ids or names."), opt("name", json!({ "type": "string", "default": "New folder" }), "Folder name.")],
            mutates: true,
            destructive: false,
            run: add_folder,
        },
        Op {
            group: "assembly",
            verb: "unpack_folder",
            summary: "Unpack an assembly folder",
            description: "The folder goes; its instances and features stay.",
            fields: vec![doc_field(), asm_field(), folder()],
            mutates: true,
            destructive: false,
            run: unpack_folder,
        },
        Op {
            group: "assembly",
            verb: "copy",
            summary: "Copy and paste instances",
            description: "Copy then Paste (Ctrl+C, Ctrl+V): new instances named \"<name> <n>\", with the mates and mate connectors among the copied instances; never fixed. `offset` moves the copies (the app places them at the cursor).",
            fields: vec![
                doc_field(),
                asm_field(),
                instances(),
                opt("to", s(), "Assembly tab to paste into (default: the same one)."),
                opt("offset", json!({ "type": "array", "items": { "type": ["number", "string"] }, "minItems": 3, "maxItems": 3 }), "[x, y, z] lengths (default [0, 0, 0])."),
            ],
            mutates: true,
            destructive: false,
            run: copy,
        },
        Op {
            group: "assembly",
            verb: "set_configuration",
            summary: "Change an instance's configuration",
            description: "Change configuration… of an instance of a configured Part Studio or assembly.",
            fields: vec![doc_field(), asm_field(), req("instance", s(), "Instance id or name."), req("configuration", json!({ "type": "object" }), "{input id: value} (list inputs: the configuration id; checkboxes: true / false).")],
            mutates: true,
            destructive: false,
            run: set_configuration,
        },
        Op {
            group: "assembly",
            verb: "lock",
            summary: "Lock a subassembly at a position or let it follow",
            description: "A subassembly instance's Lock / follow position menu: `flexible` (Unlock), `current` (lock at the current position) or a named position of the subassembly.",
            fields: vec![doc_field(), asm_field(), req("instance", s(), "Subassembly instance id or name."), req("lock", s(), "\"flexible\", \"current\" or a named position name.")],
            mutates: true,
            destructive: false,
            run: lock,
        },
        Op {
            group: "assembly",
            verb: "instance_properties",
            summary: "Set instance properties: reference designator, tessellation",
            description: "Instance properties… (Reference designator) and Use best available tessellation / Use automatic tessellation setting.",
            fields: vec![doc_field(), asm_field(), instances(), opt("reference_designator", s(), "Reference designator (one instance; empty clears)."), opt("best_tessellation", json!({ "type": "boolean" }), "Use best available tessellation (false: automatic).")],
            mutates: true,
            destructive: false,
            run: instance_properties,
        },
        Op {
            group: "assembly",
            verb: "dynamic_suppression",
            summary: "Set or remove an instance, mate or folder's dynamic suppression",
            description: "Dynamic suppression ▸ Add expression…: suppressed unless `unsuppressed_if` is true. Empty or omitted removes it.",
            fields: vec![doc_field(), asm_field(), opt("instance", s(), "Instance id or name."), opt("mate", s(), "Mate id or name."), opt("folder", s(), "Instance folder id or name."), opt("unsuppressed_if", s(), "Unsuppressed if: expression.")],
            mutates: true,
            destructive: false,
            run: dynamic_suppression,
        },
    ]
}

/// Find a folder by id ("folder:12" or 12) or name.
pub(super) fn find_folder(a: &Assembly, key: &str) -> Option<MateId> {
    let k = key.trim();
    let by_id = k.split_once(':').map_or(k, |(_, n)| n).parse::<u64>().ok().map(MateId);
    a.folders.iter().find(|f| Some(f.id) == by_id || f.name == k).map(|f| f.id)
}

pub(super) fn folders_json(a: &Assembly) -> Vec<Value> {
    a.folders
        .iter()
        .map(|f| {
            let mut v = json!({
                "id": f.id.0,
                "name": f.name,
                "instances": f.instances.iter().filter_map(|i| a.instance(*i)).map(|i| i.name.clone()).collect::<Vec<_>>(),
                "features": f.features.iter().map(|m| m.0).collect::<Vec<_>>(),
            });
            if let Some(e) = &f.unsuppressed_if {
                v["unsuppressed_if"] = json!(e);
            }
            v
        })
        .collect()
}

fn instance_list(asm: &Assembly, a: &Args, field: &str) -> Result<Vec<InstanceId>, OpError> {
    a.array(field)?.iter().enumerate().map(|(i, v)| find_instance(asm, v.as_str().unwrap_or_default(), &format!("{field}[{i}]"))).collect()
}

/// A structural edit over several tabs, with the solved placements folded in.
fn commit_doc<R>(od: &mut OpenDoc, tab: TabId, desc: &str, result: &AssemblyResult, f: impl FnOnce(&mut noshape_model::Document) -> R) -> Result<R, OpError> {
    Ok(od.session.edit_document(desc, |d| {
        if let Some(TabContent::Assembly(a)) = d.tab_mut(tab).map(|t| &mut t.content) {
            sa::fold_solved(a, result);
        }
        f(d)
    })?)
}

fn with_instance(od: &mut OpenDoc, tab: TabId, id: Option<InstanceId>) -> Result<Value, OpError> {
    let mut v = describe(od, tab)?;
    if let Some(id) = id {
        v["new_instance"] = json!(assembly(od, tab)?.instance(id).map(|i| json!({ "id": format!("inst:{}", id.0), "name": i.name })));
    }
    Ok(v)
}

fn move_to_subassembly(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let ids = sa::movable_to_subassembly(asm, &instance_list(asm, a, "instances")?);
    if ids.is_empty() {
        return Err(OpError::param("instances", "pattern and mirror copies can't move"));
    }
    let r = solve(od, tab, &SolveOptions::default())?;
    let id = commit_doc(od, tab, sa::MOVE_TO_NEW_SUBASSEMBLY, &r, |d| sa::move_to_new_subassembly(d, tab, &ids))?;
    Ok(with_instance(od, tab, id)?.into())
}

fn create_subassembly(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let r = solve(od, tab, &SolveOptions::default())?;
    let id = commit_doc(od, tab, sa::CREATE_EMPTY_SUBASSEMBLY, &r, |d| sa::create_empty_subassembly(d, tab))?;
    Ok(with_instance(od, tab, id)?.into())
}

fn dissolve_subassembly(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let ids = instance_list(asm, a, "instances")?;
    if let Some((i, _)) = ids.iter().enumerate().find(|(_, id)| !matches!(asm.instance(**id).map(|x| &x.source), Some(InstanceSource::Assembly { .. }))) {
        return Err(OpError::param(&format!("instances[{i}]"), "not a subassembly instance"));
    }
    let r = solve(od, tab, &SolveOptions::default())?;
    commit_doc(od, tab, sa::DISSOLVE_SUBASSEMBLY, &r, |d| sa::dissolve_subassemblies(d, tab, &ids))?;
    Ok(describe(od, tab)?.into())
}

fn move_instance(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = &assembly(od, tab)?.clone();
    let ids = instance_list(asm, a, "instances")?;
    let given: Vec<&str> = ["before", "folder", "subassembly"].into_iter().filter(|k| a.get(k).is_some()).collect();
    if given.len() != 1 {
        return Err(OpError::invalid("give one of `before`, `folder` or `subassembly`"));
    }
    if let Some(k) = a.opt_str("subassembly")? {
        let sub = find_instance(asm, k, "subassembly")?;
        if !matches!(asm.instance(sub).map(|i| &i.source), Some(InstanceSource::Assembly { .. })) {
            return Err(OpError::param("subassembly", "not a subassembly instance"));
        }
        let r = solve(od, tab, &SolveOptions::default())?;
        if sa::would_cycle(od.doc(), asm, &ids, sub) {
            return Err(OpError::invalid(sa::CYCLIC_SUBASSEMBLY));
        }
        let moved = commit_doc(od, tab, sa::MOVE_TO_SUBASSEMBLY, &r, |d| sa::move_into_subassembly(d, tab, &ids, sub))?.map_err(OpError::invalid)?;
        let mut v = describe(od, tab)?;
        v["moved"] = json!(moved.iter().map(|i| i.0).collect::<Vec<_>>());
        return Ok(v.into());
    }
    let r = solve(od, tab, &SolveOptions::default())?;
    if let Some(k) = a.opt_str("folder")? {
        let f = find_folder(asm, k).ok_or_else(|| OpError::param("folder", "no such folder (see assembly_list)"))?;
        commit(od, tab, sa::ADD_TO_FOLDER, &r, |x| {
            for id in &ids {
                x.move_into_folder(*id, f);
            }
        })?;
    } else {
        let before = match a.str("before")? {
            "end" => None,
            k => Some(find_instance(asm, k, "before")?),
        };
        if before.is_some_and(|b| ids.contains(&b)) {
            return Err(OpError::param("before", "one of the moved instances"));
        }
        commit(od, tab, sa::REORDER_INSTANCES, &r, |x| {
            for id in &ids {
                x.move_instance(*id, before);
            }
        })?;
    }
    Ok(describe(od, tab)?.into())
}

fn add_folder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let insts = if a.get("instances").is_some() { instance_list(asm, a, "instances")? } else { Vec::new() };
    let feats: Vec<MateId> = match a.opt_array("features")? {
        Some(fs) => fs.iter().map(|v| find_mate(asm, v.as_str().unwrap_or_default()).map_err(|e| e.with_param("features"))).collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    if insts.is_empty() && feats.is_empty() {
        return Err(OpError::invalid("give `instances` or `features`"));
    }
    let name = a.opt_str("name")?.unwrap_or_default().to_string();
    let r = solve(od, tab, &SolveOptions::default())?;
    let id = commit(od, tab, sa::ADD_FOLDER_DESCRIPTION, &r, |x| sa::add_folder(x, &insts, &feats, &name))?;
    let mut v = describe(od, tab)?;
    v["folder"] = json!(id.0);
    Ok(v.into())
}

fn unpack_folder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_folder(assembly(od, tab)?, a.str("folder")?).ok_or_else(|| OpError::param("folder", "no such folder (see assembly_list)"))?;
    let r = solve(od, tab, &SolveOptions::default())?;
    commit(od, tab, sa::UNPACK_FOLDER_DESCRIPTION, &r, |x| x.unpack_folder(id))?;
    Ok(describe(od, tab)?.into())
}

fn copy(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let asm = assembly(od, tab)?;
    let ids = instance_list(asm, a, "instances")?;
    let clip = InstanceClipboard::copy(asm, &ids);
    let to = match a.opt_str("to")? {
        Some(t) => crate::doc::tab_of_kind(od.doc(), Some(t), "Assembly", |c| matches!(c, TabContent::Assembly(_))).map_err(|e| e.with_param("to"))?,
        None => tab,
    };
    // A subassembly can't be pasted into itself or anything inside it.
    for i in &clip.instances {
        if let InstanceSource::Assembly { assembly: sub, .. } = i.source
            && sa::contains_assembly(od.doc(), sub, to, 0)
        {
            return Err(OpError::param("to", "the copied subassembly contains that assembly"));
        }
    }
    let mut offset = [0.0; 3];
    if let Some(o) = a.opt_array("offset")? {
        for (i, v) in o.iter().take(3).enumerate() {
            offset[i] = crate::ops::sketch::length_value(v, &units).map_err(|e| e.with_param(format!("offset[{i}]")))?;
        }
    }
    let r = solve(od, to, &SolveOptions::default())?;
    let new = commit(od, to, &sa::paste_description(&clip), &r, |x| clip.paste(x, offset))?;
    let names: Vec<Value> = new.iter().filter_map(|id| assembly(od, to).ok()?.instance(*id).map(|i| json!({ "id": format!("inst:{}", id.0), "name": i.name }))).collect();
    let mut v = describe(od, to)?;
    v["pasted"] = json!(names);
    Ok(v.into())
}

fn set_configuration(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let id = find_instance(asm, a.str("instance")?, "instance")?;
    let name = asm.instance(id).map(|i| i.name.clone()).unwrap_or_default();
    let src = match asm.instance(id).map(|i| &i.source) {
        Some(InstanceSource::Part { studio, .. } | InstanceSource::PartStudio { studio, .. }) => Some(*studio),
        Some(InstanceSource::Assembly { assembly, .. }) => Some(*assembly),
        _ => None,
    };
    let values = match src {
        Some(src) => crate::ops::source_config_arg(od, src, a)?,
        None => config_arg(a)?,
    };
    let r = solve(od, tab, &SolveOptions::default())?;
    let ok = commit(od, tab, &sa::change_configuration_description(&name), &r, |x| x.set_instance_config(id, values))?;
    if !ok {
        return Err(OpError::param("instance", "this instance has no configuration"));
    }
    Ok(describe(od, tab)?.into())
}

fn lock(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let id = find_instance(asm, a.str("instance")?, "instance")?;
    let inst = asm.instance(id).expect("found");
    let Some(InstanceSource::Assembly { assembly: sub_tab, .. }) = Some(&inst.source) else {
        return Err(OpError::param("instance", "not a subassembly instance"));
    };
    let lock = match a.str("lock")? {
        "flexible" => SubassemblyLock::Flexible,
        "current" => SubassemblyLock::Current,
        n => {
            let positions: Vec<String> = match od.doc().tab(*sub_tab).map(|t| &t.content) {
                Some(TabContent::Assembly(s)) => s.named_positions.iter().map(|p| p.name.clone()).collect(),
                _ => Vec::new(),
            };
            let p = positions.iter().find(|p| p.as_str() == n).ok_or_else(|| OpError::param("lock", &format!("\"flexible\", \"current\" or a named position of the subassembly ({})", positions.join(", "))))?;
            SubassemblyLock::Named(p.clone())
        }
    };
    let (desc, message) = sa::lock_description(&inst.name, &lock);
    let r = solve(od, tab, &SolveOptions::default())?;
    commit(od, tab, &desc, &r, |x| sa::set_lock(x, id, lock))?;
    let mut v = describe(od, tab)?;
    v["message"] = json!(message);
    Ok(v.into())
}

fn instance_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let ids = instance_list(assembly(od, tab)?, a, "instances")?;
    let rd = a.opt_str("reference_designator")?.map(str::to_string);
    let best = a.get("best_tessellation").and_then(Value::as_bool);
    if rd.is_none() && best.is_none() {
        return Err(OpError::invalid("give reference_designator or best_tessellation"));
    }
    if let Some(rd) = rd {
        let [id] = ids[..] else { return Err(OpError::param("instances", "one instance for reference_designator")) };
        let r = solve(od, tab, &SolveOptions::default())?;
        commit(od, tab, sa::INSTANCE_PROPERTIES_DESCRIPTION, &r, |x| sa::set_reference_designator(x, id, &rd))?;
    }
    if let Some(on) = best {
        let r = solve(od, tab, &SolveOptions::default())?;
        commit(od, tab, sa::best_tessellation_description(on), &r, |x| sa::set_best_tessellation(x, &ids, on))?;
    }
    Ok(describe(od, tab)?.into())
}

fn dynamic_suppression(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let t = match (a.opt_str("instance")?, a.opt_str("mate")?, a.opt_str("folder")?) {
        (Some(i), None, None) => sa::DynTarget::Instance(find_instance(asm, i, "instance")?),
        (None, Some(m), None) => sa::DynTarget::Mate(asm.mates.iter().find(|x| x.name == m || format!("mate:{}", x.id.0) == m || x.id.0.to_string() == m).map(|x| x.id).ok_or_else(|| OpError::param("mate", "no such mate"))?),
        (None, None, Some(f)) => sa::DynTarget::Folder(find_folder(asm, f).ok_or_else(|| OpError::param("folder", "no such folder"))?),
        _ => return Err(OpError::invalid("give one of `instance`, `mate` or `folder`")),
    };
    let text = a.opt_str("unsuppressed_if")?.map(str::trim).filter(|e| !e.is_empty()).map(str::to_string);
    let r = solve(od, tab, &SolveOptions::default())?;
    commit(od, tab, sa::DYNAMIC_SUPPRESSION_DESCRIPTION, &r, |x| sa::set_unsuppressed_if(x, t, text))?;
    Ok(describe(od, tab)?.into())
}
