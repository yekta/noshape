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
            description: "With `tab`, undoes the last edit of that tab (Onshape's undo is per tab); otherwise the last edit of the document.",
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
            description: "Microversions with their description (\"Part Studio 1 :: Insert feature : Extrude 1\"), author and time, plus versions and branches.",
            fields: vec![doc_field(), opt("limit", json!({ "type": "integer", "minimum": 1 }), "At most this many entries (default 50).")],
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
    Ok(json!({ "undone": done, "can_undo": od.session.can_undo(), "can_redo": od.session.can_redo() }).into())
}

fn redo(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    set_scope(od, a)?;
    let done = od.session.redo()?;
    Ok(json!({ "redone": done, "can_undo": od.session.can_undo(), "can_redo": od.session.can_redo() }).into())
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let limit = a.opt_u64("limit")?.unwrap_or(50) as usize;
    let store = &od.session.store;
    let hist = store.history(od.session.branch).map_err(|e| OpError::new("io", e.to_string()))?;
    let entries: Vec<_> = hist.iter().take(limit).map(|m| json!({ "id": m.id.to_string(), "description": m.description, "author": m.author, "time": format!("{:?}", m.created) })).collect();
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
