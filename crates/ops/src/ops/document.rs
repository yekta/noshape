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
            description: "Tab › Duplicate: \"<name> Copy 1\" right after it.",
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
        Op {
            group: "tab",
            verb: "folder_create",
            summary: "Create a tab folder",
            description: "The tab bar's \"+\" › Create folder: \"Folder 1\" (next free number) unless `name` is given, inside `folder` (default: the root). Folders nest up to Onshape's depth limit.",
            fields: vec![doc_field(), opt("name", s(), "Folder name (default: the next \"Folder N\")."), opt("folder", s(), "Folder (name or id) to create it in (default: the root).")],
            mutates: true,
            destructive: false,
            run: tab_folder_create,
        },
        Op {
            group: "tab",
            verb: "move",
            summary: "Move a tab or tab folder into a folder, out to its parent, or before another tab",
            description: "Dragging a tab onto a folder tab (or onto 🏠: the root), Tab › Move to parent folder (`to_parent`), or dragging it along the bar (`before`: it goes in front of that tab, in that tab's folder). Like Onshape, the tab order isn't undoable; moving between folders is. Refused for a folder into itself or past the depth limit.",
            fields: vec![
                doc_field(),
                req("tab", s(), "Tab or folder (name or id) to move."),
                opt("folder", s(), "Target folder (name or id); omit (without `to_parent` / `before`) for the root."),
                opt("to_parent", json!({ "type": "boolean" }), "Move to parent folder: out of its folder into the one around it."),
                opt("before", s(), "Tab or folder (name or id) to put it in front of."),
            ],
            mutates: true,
            destructive: false,
            run: tab_move,
        },
        Op {
            group: "tab",
            verb: "folder_unpack",
            summary: "Unpack a tab folder",
            description: "Folder › Unpack: the folder goes; its tabs and folders move to its parent, at the end of the tab list (as Onshape).",
            fields: vec![doc_field(), req("folder", s(), "Folder name or id.")],
            mutates: true,
            destructive: false,
            run: tab_folder_unpack,
        },
    ]
}

/// A tab or tab folder by name or id.
fn find_tab_or_folder(d: &Document, key: &str, param: &'static str) -> Result<noshape_model::document::TabId, OpError> {
    let k = key.trim();
    if let Some(f) = d.settings.folders.iter().find(|f| f.id.to_string() == k || f.name == k) {
        return Ok(f.id);
    }
    doc::find_tab(d, k).map_err(|e| e.with_param(param))
}

fn find_folder(d: &Document, key: &str, param: &'static str) -> Result<noshape_model::document::TabId, OpError> {
    let k = key.trim();
    d.settings.folders.iter().find(|f| f.id.to_string() == k || f.name == k).map(|f| f.id).ok_or_else(|| {
        let names: Vec<&str> = d.settings.folders.iter().map(|f| f.name.as_str()).collect();
        OpError::not_found(format!("no tab folder `{key}`; folders: {}", names.join(", "))).with_param(param)
    })
}

fn tab_folder_create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let parent = a.opt_str("folder")?.map(|f| find_folder(od.doc(), f, "folder")).transpose()?;
    let name = match a.opt_str("name")? {
        Some(n) => n.to_string(),
        None => od.doc().next_folder_name(),
    };
    if parent.is_some_and(|p| od.doc().ancestors(p).len() + 2 > noshape_model::document::MAX_FOLDER_DEPTH) {
        return Err(OpError::invalid("folders don't nest that deep").with_param("folder"));
    }
    let id = od.session.edit_document("Create folder", |d| d.add_folder(&name, parent))?;
    Ok(json!({ "folder": { "id": id.to_string(), "name": name } }).into())
}

fn tab_move(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let id = find_tab_or_folder(od.doc(), a.str("tab")?, "tab")?;
    let before = a.opt_str("before")?.map(|b| find_tab_or_folder(od.doc(), b, "before")).transpose()?;
    let to_parent = a.bool_or("to_parent", false)?;
    let target = if let Some(b) = before {
        od.doc().parent_of(b)
    } else if to_parent {
        let Some(p) = od.doc().parent_of(id) else { return Err(OpError::invalid("it isn't in a folder").with_param("to_parent")) };
        od.doc().parent_of(p)
    } else {
        a.opt_str("folder")?.map(|f| find_folder(od.doc(), f, "folder")).transpose()?
    };
    if od.doc().parent_of(id) != target {
        let moved = od.session.edit_document("Move to folder", |d| d.set_parent(id, target))?;
        if !moved {
            return Err(OpError::invalid("a folder can't go into itself or nest deeper than Onshape allows").with_param("folder"));
        }
    }
    if let Some(b) = before {
        // Live: a reorder isn't undoable.
        od.session.edit_document_not_undoable("Reorder tabs", |d| d.move_in_order(id, Some(b)))?;
    }
    let folder = target.and_then(|t| od.doc().folder(t)).map(|f| f.name.clone());
    Ok(json!({ "moved": id.to_string(), "folder": folder }).into())
}

fn tab_folder_unpack(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let id = find_folder(od.doc(), a.str("folder")?, "folder")?;
    let name = od.doc().folder(id).map(|f| f.name.clone()).unwrap_or_default();
    od.session.edit_document("Unpack folder", |d| d.unpack_folder(id))?;
    Ok(json!({ "unpacked": name }).into())
}

fn length_unit(s: &str) -> Result<LengthUnit, OpError> {
    s.parse().map_err(|_| OpError::param("length_unit", "unknown length unit (mm, cm, m, in, ft, yd, um)"))
}

fn angle_unit(s: &str) -> Result<AngleUnit, OpError> {
    s.parse().map_err(|_| OpError::param("angle_unit", "unknown angle unit (deg, rad)"))
}

pub(crate) fn describe(od: &OpenDoc) -> Value {
    let d = od.doc();
    let folder_of = |id| d.parent_of(id).and_then(|p| d.folder(p)).map(|f| f.name.clone());
    let tabs: Vec<Value> = d.ordered_tabs().iter().map(|t| json!({ "id": t.id.to_string(), "name": t.name, "kind": t.content.kind_name(), "folder": folder_of(t.id) })).collect();
    let folders: Vec<Value> = d.settings.folders.iter().map(|f| json!({ "id": f.id.to_string(), "name": f.name, "folder": folder_of(f.id) })).collect();
    let u = &d.settings.units;
    json!({
        "path": od.session.path.display().to_string(),
        "name": od.session.name(),
        "tabs": tabs,
        "folders": folders,
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
    // The kind as document_info prints it ("Part Studio") works too.
    let kind: String = a.str("kind")?.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    let (base, content) = docedit::new_tab_content(&kind).ok_or_else(|| OpError::param("kind", &format!("unknown tab kind; one of {}", docedit::TAB_KINDS.iter().map(|k| k.0).collect::<Vec<_>>().join(", "))))?;
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
    // Onshape: a tab rename isn't undoable (qa5 TABS5).
    od.session.edit_document_not_undoable(&docedit::rename_tab_description(&name), |d| docedit::rename_tab(d, tab, &name))?;
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
