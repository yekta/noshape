//! Documents and tabs.

use super::{doc_field, s};
use crate::docedit;
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, doc};
use noshape_model::Document;
use noshape_units::{AngleUnit, LengthUnit};
use serde_json::{Value, json};
use std::path::Path;

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "document",
            verb: "create",
            summary: "Create a new document file",
            description: "Creates a .noshape document (one SQLite file holding every tab and the whole edit history). Like a new Onshape document it has \"Part Studio 1\" and \"Assembly 1\". Becomes the current document. Every later edit is saved immediately as a microversion (there is no Save).",
            fields: vec![
                req("path", s(), "Where to create it, e.g. \"/tmp/bracket.noshape\". Must not exist."),
                opt("length_unit", json!({ "type": "string", "enum": ["mm", "cm", "m", "in", "ft", "yd", "um"] }), "Workspace length unit (default mm): bare numbers in length fields use it."),
                opt("angle_unit", json!({ "type": "string", "enum": ["deg", "rad"] }), "Workspace angle unit (default deg)."),
            ],
            mutates: true,
            destructive: false,
            run: create,
        },
        Op {
            group: "document",
            verb: "open",
            summary: "Open a document file and make it current",
            description: "Opens an existing .noshape file. Later calls without `doc` use it.",
            fields: vec![req("path", s(), "Path of the .noshape file.")],
            mutates: false,
            destructive: false,
            run: open,
        },
        Op {
            group: "document",
            verb: "info",
            summary: "Describe a document: tabs, units, history state",
            description: "Lists the tabs in tab-bar order (id, name, kind), the workspace units, the branch, whether it is protected (workspace protection) or read-only, and whether undo/redo are available.",
            fields: vec![doc_field()],
            mutates: false,
            destructive: false,
            run: info,
        },
        Op {
            group: "document",
            verb: "close",
            summary: "Close an open document",
            description: "Forgets the document in this session (its file stays). Edits are already saved.",
            fields: vec![doc_field()],
            mutates: false,
            destructive: false,
            run: close,
        },
        Op {
            group: "document",
            verb: "protect",
            summary: "Turn on workspace protection for a branch",
            description: "Versions and history › workspace row › Workspace protection… › Enable workspace protection. A protected workspace takes no direct model edits (geometry, drawings, tabs, settings, undo/redo, restore): those fail with error `protected`. Make changes on another branch and merge them in; versions, branches and merges stay allowed.",
            fields: vec![doc_field(), opt("branch", s(), "Branch name or id (default: the current one).")],
            mutates: true,
            destructive: false,
            run: protect,
        },
        Op {
            group: "document",
            verb: "unprotect",
            summary: "Turn off workspace protection for a branch",
            description: "Clears Enable workspace protection: the workspace takes edits again.",
            fields: vec![doc_field(), opt("branch", s(), "Branch name or id (default: the current one).")],
            mutates: true,
            destructive: false,
            run: unprotect,
        },
        Op {
            group: "document",
            verb: "set_units",
            summary: "Change the workspace units",
            description: "Document › Workspace units: the units bare numbers take and values are shown in.",
            fields: vec![
                doc_field(),
                opt("length_unit", json!({ "type": "string", "enum": ["mm", "cm", "m", "in", "ft", "yd", "um"] }), "Length unit."),
                opt("angle_unit", json!({ "type": "string", "enum": ["deg", "rad"] }), "Angle unit."),
                opt("length_precision", json!({ "type": "integer", "minimum": 0, "maximum": 8 }), "Decimals shown for lengths."),
                opt("angle_precision", json!({ "type": "integer", "minimum": 0, "maximum": 8 }), "Decimals shown for angles."),
            ],
            mutates: true,
            destructive: false,
            run: set_units,
        },
        Op {
            group: "tab",
            verb: "create",
            summary: "Insert a new tab (Part Studio, Assembly, Drawing, Variable Studio, Feature Studio, Material Library)",
            description: "The tab bar's \"+\" menu. Gets the next default name (\"Part Studio 2\") unless `name` is given, and goes after `after` (default: the end).",
            fields: vec![
                doc_field(),
                req("kind", json!({ "type": "string", "enum": docedit::TAB_KINDS.iter().map(|k| k.0).collect::<Vec<_>>() }), "Tab kind."),
                opt("name", s(), "Name (default: next default name)."),
                opt("after", s(), "Tab (name or id) to place the new tab after."),
            ],
            mutates: true,
            destructive: false,
            run: tab_create,
        },
        Op {
            group: "tab",
            verb: "rename",
            summary: "Rename a tab",
            description: "",
            fields: vec![doc_field(), req("tab", s(), "Tab name or id."), req("name", s(), "New name.")],
            mutates: true,
            destructive: false,
            run: tab_rename,
        },
        Op {
            group: "tab",
            verb: "duplicate",
            summary: "Duplicate a tab",
            description: "Tab › Duplicate: \"<name> (copy)\" right after it.",
            fields: vec![doc_field(), req("tab", s(), "Tab name or id.")],
            mutates: true,
            destructive: false,
            run: tab_duplicate,
        },
        Op {
            group: "tab",
            verb: "delete",
            summary: "Delete a tab",
            description: "As in Onshape, undo does not bring it back (history restore does). Fails if it is the last tab. Other tabs that use it (assembly instances, drawings, derived parts) are listed in the reply; they will show errors.",
            fields: vec![doc_field(), req("tab", s(), "Tab name or id.")],
            mutates: true,
            destructive: true,
            run: tab_delete,
        },
    ]
}

fn length_unit(s: &str) -> Result<LengthUnit, OpError> {
    s.parse().map_err(|_| OpError::param("length_unit", "unknown length unit (mm, cm, m, in, ft, yd, um)"))
}

fn angle_unit(s: &str) -> Result<AngleUnit, OpError> {
    s.parse().map_err(|_| OpError::param("angle_unit", "unknown angle unit (deg, rad)"))
}

pub(crate) fn describe(od: &OpenDoc) -> Value {
    let d = od.doc();
    let tabs: Vec<Value> = d.ordered_tabs().iter().map(|t| json!({ "id": t.id.to_string(), "name": t.name, "kind": t.content.kind_name() })).collect();
    let u = &d.settings.units;
    json!({
        "path": od.session.path.display().to_string(),
        "name": od.session.name(),
        "tabs": tabs,
        "units": { "length": u.length.to_string(), "angle": u.angle.to_string(), "length_precision": u.length_precision, "angle_precision": u.angle_precision },
        "can_undo": od.session.can_undo(),
        "can_redo": od.session.can_redo(),
        "read_only": od.session.read_only(),
        "branch": od.session.store.list_branches().ok().and_then(|l| l.into_iter().find(|b| b.id == od.session.branch)).map(|b| b.name),
        "protected": od.session.is_protected(),
    })
}

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let mut d = Document::new();
    if let Some(u) = a.opt_str("length_unit")? {
        d.settings.units.length = length_unit(u)?;
    }
    if let Some(u) = a.opt_str("angle_unit")? {
        d.settings.units.angle = angle_unit(u)?;
    }
    let od = ctx.create(Path::new(a.str("path")?), d)?;
    Ok(describe(od).into())
}

fn open(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.open(Path::new(a.str("path")?)).map_err(|e| e.with_param("path"))?;
    Ok(describe(od).into())
}

fn info(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    Ok(describe(ctx.doc(a.opt_str("doc")?)?).into())
}

fn close(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let p = ctx.close(a.opt_str("doc")?)?;
    Ok(json!({ "closed": p.display().to_string() }).into())
}

fn set_protected(ctx: &mut Ctx, a: &Args, on: bool) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let b = match a.opt_str("branch")? {
        Some(k) => super::history::branch_id(od, k)?,
        None => od.session.branch,
    };
    od.session.set_branch_protected(b, on)?;
    let mut v = describe(od);
    v["changed_branch"] = json!(b.to_string());
    Ok(v.into())
}

fn protect(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    set_protected(ctx, a, true)
}

fn unprotect(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    set_protected(ctx, a, false)
}

fn set_units(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut u = od.doc().settings.units;
    if let Some(x) = a.opt_str("length_unit")? {
        u.length = length_unit(x)?;
    }
    if let Some(x) = a.opt_str("angle_unit")? {
        u.angle = angle_unit(x)?;
    }
    if let Some(p) = a.opt_u64("length_precision")? {
        u.length_precision = p.min(8) as u8;
    }
    if let Some(p) = a.opt_u64("angle_precision")? {
        u.angle_precision = p.min(8) as u8;
    }
    od.session.set_units(u, docedit::UNITS_DESCRIPTION)?;
    Ok(describe(od).into())
}

fn tab_create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let kind = a.str("kind")?;
    let (base, content) = docedit::new_tab_content(kind).ok_or_else(|| OpError::param("kind", "unknown tab kind"))?;
    let after = match a.opt_str("after")? {
        Some(t) => Some(doc::find_tab(od.doc(), t).map_err(|e| e.with_param("after"))?),
        None => None,
    };
    let name = a.opt_str("name")?.map(str::to_string);
    let id = od.session.edit_document(&docedit::insert_tab_description(base), |d| {
        let id = docedit::insert_tab(d, base, content, after);
        if let Some(n) = &name {
            docedit::rename_tab(d, id, n);
        }
        id
    })?;
    let t = od.doc().tab(id).expect("created");
    Ok(json!({ "tab": { "id": id.to_string(), "name": t.name, "kind": t.content.kind_name() } }).into())
}

fn tab_rename(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::find_tab(od.doc(), a.str("tab")?)?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "must not be empty"));
    }
    od.session.edit_document(&docedit::rename_tab_description(&name), |d| docedit::rename_tab(d, tab, &name))?;
    Ok(json!({ "tab": tab.to_string(), "name": name }).into())
}

fn tab_duplicate(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::find_tab(od.doc(), a.str("tab")?)?;
    let name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    let id = od.session.edit_document(&docedit::duplicate_tab_description(&name), |d| docedit::duplicate_tab(d, tab))?.ok_or_else(|| OpError::not_found("tab vanished"))?;
    let t = od.doc().tab(id).expect("created");
    Ok(json!({ "tab": { "id": id.to_string(), "name": t.name, "kind": t.content.kind_name() } }).into())
}

fn tab_delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::find_tab(od.doc(), a.str("tab")?)?;
    if od.doc().tabs.len() <= 1 {
        return Err(OpError::invalid("a document keeps at least one tab").with_param("tab"));
    }
    let name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    let users: Vec<String> = od.doc().tab_users(tab).into_iter().filter_map(|u| od.doc().tab(u).map(|t| t.name.clone())).collect();
    od.session.edit_document_not_undoable(&docedit::delete_tab_description(&name), |d| {
        d.remove_tab(tab);
    })?;
    Ok(json!({ "deleted": name, "users_now_broken": users }).into())
}
