//! Undo/redo, versions, branches (Versions and history panel).

use super::{doc_field, s};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, doc};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "history",
            verb: "undo",
            summary: "Undo the last edit",
            description: "With `tab`, undoes the last edit of that tab (Onshape's undo is per tab); otherwise the last edit of the document. `description` names the edit undone.",
            fields: vec![doc_field(), opt("tab", s(), "Tab whose edit to undo.")],
            mutates: true,
            destructive: false,
            run: undo,
        },
        Op {
            group: "history",
            verb: "redo",
            summary: "Redo the last undone edit",
            description: "",
            fields: vec![doc_field(), opt("tab", s(), "Tab whose edit to redo.")],
            mutates: true,
            destructive: false,
            run: redo,
        },
        Op {
            group: "history",
            verb: "list",
            summary: "The edit history of the current branch, newest first",
            description: "Microversions with their description (\"Part Studio 1 :: Insert feature : Extrude 1\"), author and time (ms since 1970), plus versions and branches. The Versions and history panel's search ▾ filters them: `tab` (changes to that tab), `kinds` (changes to tabs of these kinds), `modified_by`, a `from` / `to` time range, and `search` (text in the description); `limit` counts the entries that pass.",
            fields: vec![
                doc_field(),
                opt("limit", json!({ "type": "integer", "minimum": 1 }), "At most this many entries (default 50)."),
                opt("search", s(), "Only entries whose description contains this (any case)."),
                opt("tab", s(), "Only changes to this tab (name or id)."),
                opt("kinds", json!({ "type": "array", "items": { "type": "string", "enum": crate::history_filter::KINDS } }), "Only changes to tabs of these kinds."),
                opt("modified_by", json!({ "type": "string", "enum": ["any", "only_me", "not_me"] }), "Modified by (default any)."),
                opt("from", json!({ "type": "integer" }), "Only changes at or after this time (ms since 1970)."),
                opt("to", json!({ "type": "integer" }), "Only changes at or before this time (ms since 1970)."),
            ],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "history",
            verb: "create_version",
            summary: "Create a named version of the current state",
            description: "",
            fields: vec![doc_field(), req("name", s(), "Version name (\"V1\")."), opt("description", s(), "Description.")],
            mutates: true,
            destructive: false,
            run: create_version,
        },
        Op {
            group: "history",
            verb: "version_properties",
            summary: "A version's or workspace's Properties… (name and description)",
            description: "The Versions and history panel's row menu › Properties…: renames `version` or `branch` (a workspace) and sets its description. Not an undoable edit (as in the app).",
            fields: vec![doc_field(), opt("version", s(), "Version name or id."), opt("branch", s(), "Branch (workspace) name or id."), opt("name", s(), "New name."), opt("description", s(), "New description (empty clears it).")],
            mutates: true,
            destructive: false,
            run: version_properties,
        },
        Op {
            group: "history",
            verb: "publish",
            summary: "A version's Publish FeatureScript…: publish its custom features and tables",
            description: "The version row menu › Publish FeatureScript…: allowed once every requirement is met (at least one custom feature or table, a PDF in the document, no FeatureScript errors or warnings, every custom feature described and used in a Part Studio, every Feature Studio committed); the error lists the unmet ones. Published versions are found by the documents page's advanced search (Published FeatureScript).",
            fields: vec![doc_field(), req("version", s(), "Version name or id.")],
            mutates: true,
            destructive: false,
            run: publish,
        },
        Op {
            group: "history",
            verb: "create_branch",
            summary: "Create a branch (workspace) from the current state and switch to it",
            description: "",
            fields: vec![doc_field(), req("name", s(), "Branch name.")],
            mutates: true,
            destructive: false,
            run: create_branch,
        },
        Op {
            group: "history",
            verb: "switch_branch",
            summary: "Switch to another branch",
            description: "",
            fields: vec![doc_field(), req("branch", s(), "Branch name or id.")],
            mutates: true,
            destructive: false,
            run: switch_branch,
        },
        Op {
            group: "history",
            verb: "merge",
            summary: "Merge another branch into the current one",
            description: "Part Studios merge feature by feature.",
            fields: vec![doc_field(), req("branch", s(), "Branch to merge from (name or id).")],
            mutates: true,
            destructive: false,
            run: merge,
        },
        Op {
            group: "history",
            verb: "restore",
            summary: "Restore the document to an earlier microversion or version",
            description: "Adds a new microversion with that state (history is kept).",
            fields: vec![doc_field(), req("to", s(), "Microversion id or version name/id (from history_list).")],
            mutates: true,
            destructive: false,
            run: restore,
        },
    ]
}

fn set_scope(od: &mut crate::OpenDoc, a: &Args) -> Result<(), OpError> {
    od.session.active_tab = match a.opt_str("tab")? {
        Some(t) => Some(doc::find_tab(od.doc(), t)?),
        None => None,
    };
    Ok(())
}

fn undo(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    set_scope(od, a)?;
    let done = od.session.undo()?;
    let what = if done { od.session.last_undone() } else { None };
    Ok(json!({ "undone": done, "description": what, "can_undo": od.session.can_undo(), "can_redo": od.session.can_redo() }).into())
}

fn redo(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    set_scope(od, a)?;
    let done = od.session.redo()?;
    let what = if done { od.session.last_redone() } else { None };
    Ok(json!({ "redone": done, "description": what, "can_undo": od.session.can_undo(), "can_redo": od.session.can_redo() }).into())
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    use crate::history_filter::{HistoryFilter, KINDS, ModifiedBy, changed_tabs};
    let limit = a.opt_u64("limit")?.unwrap_or(50) as usize;
    let mut f = HistoryFilter::default();
    if let Some(t) = a.opt_str("tab")? {
        f.tab = Some(crate::doc::tab_of_kind(od.doc(), Some(t), "tab", |_| true).map_err(|e| e.with_param("tab"))?);
    }
    for (i, k) in a.opt_array("kinds")?.into_iter().flatten().enumerate() {
        f.kinds.push(KINDS.into_iter().find(|x| Some(*x) == k.as_str()).ok_or_else(|| OpError::param(&format!("kinds[{i}]"), &KINDS.join(", ")))?);
    }
    f.by = match a.opt_str("modified_by")? {
        None | Some("any") => ModifiedBy::Any,
        Some("only_me") => ModifiedBy::OnlyMe,
        Some("not_me") => ModifiedBy::NotMe,
        Some(_) => return Err(OpError::param("modified_by", "any, only_me or not_me")),
    };
    let time = |k: &str| a.get(k).map(|v| v.as_i64().ok_or_else(|| OpError::param(k, "ms since 1970 (an entry's `time`)"))).transpose();
    (f.from, f.to) = (time("from")?, time("to")?);
    let q = a.opt_str("search")?.unwrap_or("").trim().to_lowercase();
    let store = &od.session.store;
    let hist = store.history(od.session.branch).map_err(|e| OpError::new("io", e.to_string()))?;
    let doc = &od.session.doc;
    let entries: Vec<_> = hist.iter().filter(|m| (q.is_empty() || m.description.to_lowercase().contains(&q)) && f.matches(m, doc, || changed_tabs(store, m))).take(limit).map(|m| json!({ "id": m.id.to_string(), "description": m.description, "author": m.author, "time": format!("{:?}", m.created) })).collect();
    let versions: Vec<_> = store.list_versions().map_err(|e| OpError::new("io", e.to_string()))?.into_iter().map(|v| json!({ "id": v.id.to_string(), "name": v.name, "microversion": v.microversion.to_string() })).collect();
    let branches: Vec<_> = store.list_branches().map_err(|e| OpError::new("io", e.to_string()))?.into_iter().map(|b| json!({ "id": b.id.to_string(), "name": b.name, "current": b.id == od.session.branch })).collect();
    Ok(json!({ "history": entries, "versions": versions, "branches": branches }).into())
}

fn create_version(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let v = od.session.create_version(a.str("name")?, a.opt_str("description")?.unwrap_or(""))?;
    Ok(json!({ "version": v.to_string(), "name": a.str("name")? }).into())
}

pub(super) fn branch_id(od: &crate::OpenDoc, key: &str) -> Result<noshape_store::BranchId, OpError> {
    let list = od.session.store.list_branches().map_err(|e| OpError::new("io", e.to_string()))?;
    list.iter().find(|b| b.id.to_string() == key || b.name == key).map(|b| b.id).ok_or_else(|| OpError::not_found(format!("no branch `{key}`; branches: {}", list.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", "))).with_param("branch"))
}

fn create_branch(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let head = od.session.head()?;
    let b = od.session.branch_from(head, a.str("name")?)?;
    Ok(json!({ "branch": b.to_string(), "name": a.str("name")? }).into())
}

fn switch_branch(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let b = branch_id(od, a.str("branch")?)?;
    od.session.switch_branch(b)?;
    Ok(json!({ "branch": b.to_string() }).into())
}

fn merge(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let b = branch_id(od, a.str("branch")?)?;
    let out = od.session.merge_from(b)?;
    Ok(json!({ "microversion": out.microversion.to_string(), "conflicts": out.conflicts, "up_to_date": out.up_to_date }).into())
}

fn restore(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let key = a.str("to")?;
    let versions = od.session.store.list_versions().map_err(|e| OpError::new("io", e.to_string()))?;
    let mv = match versions.iter().find(|v| v.name == key || v.id.to_string() == key) {
        Some(v) => v.microversion,
        None => key.parse().map_err(|_| OpError::param("to", "not a microversion or version"))?,
    };
    od.session.restore(mv)?;
    Ok(json!({ "restored_to": mv.to_string() }).into())
}

fn io(e: impl std::fmt::Display) -> OpError {
    OpError::new("io", e.to_string())
}

fn version_info(od: &crate::OpenDoc, key: &str) -> Result<noshape_store::VersionInfo, OpError> {
    let list = od.session.store.list_versions().map_err(|e| OpError::new("io", e.to_string()))?;
    list.iter().find(|v| v.id.to_string() == key || v.name == key).cloned().ok_or_else(|| OpError::not_found(if list.is_empty() { "the document has no versions (history_create_version)".into() } else { format!("no version `{key}`; versions: {}", list.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(", ")) }).with_param("version"))
}

fn version_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let (name, desc) = (a.opt_str("name")?.map(str::trim), a.opt_str("description")?);
    if name.is_some_and(str::is_empty) {
        return Err(OpError::param("name", "must not be empty"));
    }
    match (a.opt_str("version")?, a.opt_str("branch")?) {
        (Some(v), None) => {
            let info = version_info(od, v)?;
            let (n, d) = (name.unwrap_or(&info.name).to_string(), desc.unwrap_or(&info.description).to_string());
            od.session.store.update_version(info.id, &n, &d).map_err(io)?;
            Ok(json!({ "version": info.id.to_string(), "name": n, "description": d }).into())
        }
        (None, Some(b)) => {
            let id = branch_id(od, b)?;
            let key = format!("{}{id}", crate::versions::WORKSPACE_DESCRIPTION_KEY);
            if let Some(n) = name {
                od.session.store.rename_branch(id, n).map_err(io)?;
            }
            if let Some(d) = desc {
                od.session.store.set_meta(&key, (!d.is_empty()).then_some(d.as_bytes())).map_err(io)?;
            }
            let d = od.session.store.get_meta(&key).ok().flatten().map(|d| String::from_utf8_lossy(&d).into_owned()).unwrap_or_default();
            let n = od.session.store.list_branches().map_err(io)?.into_iter().find(|x| x.id == id).map(|x| x.name).unwrap_or_default();
            Ok(json!({ "branch": id.to_string(), "name": n, "description": d }).into())
        }
        _ => Err(OpError::invalid("give `version` or `branch`").with_param("version")),
    }
}

fn publish(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let info = version_info(od, a.str("version")?)?;
    let at = noshape_model::document::Document::from_blobs(&od.session.store.read_all(info.microversion).map_err(io)?).map_err(io)?;
    let checks = crate::versions::publish_checklist(&at);
    let unmet: Vec<&str> = checks.iter().filter(|c| !c.1).map(|c| c.0).collect();
    if !unmet.is_empty() {
        return Err(OpError::invalid(format!("Requirements to publish not met: {}", unmet.join("; "))).with_param("version"));
    }
    od.session.store.set_meta(&format!("{}{}", crate::versions::PUBLISHED_FS_KEY, info.id), Some(b"1")).map_err(io)?;
    Ok(json!({ "version": info.id.to_string(), "name": info.name, "published": true }).into())
}
