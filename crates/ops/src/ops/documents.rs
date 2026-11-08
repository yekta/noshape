//! The documents page (the start window's Documents): its places, folders, labels, the Trash and the file actions of
//! a document's or folder's menu, as `noshape_ops::doc_library` / `doc_trash` do them for the page.

use super::s;
use crate::doc_library as lib;
use crate::doc_trash::{Trash, TrashItem};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::Ctx;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn ops() -> Vec<Op> {
    let path = || req("path", s(), "The document's (or folder's) path; a bare name is in the documents folder.");
    vec![
        Op {
            group: "documents",
            verb: "list",
            summary: "The documents page's list: a folder's documents and folders, Recently opened, a label's, or the Trash",
            description: "Without arguments the documents folder (Owned by me): its folders and documents (name, path, modified, labels). `folder` lists another folder; `place` \"recent\" the Recently opened list, \"trash\" the Trash (its items' `id`s for documents_restore / documents_delete); `label` the documents with that label. `labels` lists every label.",
            fields: vec![
                opt("folder", s(), "A folder to list (a path; a bare name is in the documents folder)."),
                opt("place", json!({ "type": "string", "enum": ["owned", "recent", "trash"] }), "Owned by me (default), Recently opened or the Trash."),
                opt("label", s(), "Only the documents with this label."),
                opt("sort", json!({ "type": "string", "enum": ["name", "name_desc", "modified"] }), "The folders' order, as the Folders row's sort menu: Name: A → Z (default), Name: Z → A, Last modified (newest first)."),
            ],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "documents",
            verb: "create_folder",
            summary: "Create a folder (Create ▾ › Folder…)",
            description: "A folder named `name` (default \"Untitled folder\", then \"Untitled folder (1)\"…) in `folder` (default the documents folder). Refused when one of that name is there.",
            fields: vec![opt("name", s(), "Folder name."), opt("folder", s(), "Where (default the documents folder).")],
            mutates: true,
            destructive: false,
            run: create_folder,
        },
        Op {
            group: "documents",
            verb: "rename",
            summary: "Rename a document or folder (its menu's Rename)",
            description: "A document: its file and its name (the recent list and its labels follow); a folder: in place. The document must not be open.",
            fields: vec![path(), req("name", s(), "The new name.")],
            mutates: true,
            destructive: false,
            run: rename,
        },
        Op {
            group: "documents",
            verb: "move",
            summary: "Move a document or folder into another folder (its menu's Move to…)",
            description: "Into `to` (a folder; a folder can't go into itself). The document must not be open; its recent entry and labels follow.",
            fields: vec![path(), req("to", s(), "The folder to move it into.")],
            mutates: true,
            destructive: false,
            run: move_to,
        },
        Op {
            group: "documents",
            verb: "copy",
            summary: "Copy a document's workspace to a new document (the row menu's Copy workspace…)",
            description: "A new document named `name` (default \"<name> - Copy\") in `to` (default the same folder) with the workspace's state.",
            fields: vec![path(), opt("name", s(), "The copy's name."), opt("to", s(), "The folder for the copy.")],
            mutates: true,
            destructive: false,
            run: copy,
        },
        Op {
            group: "documents",
            verb: "details",
            summary: "A document's details (the Details panel), and its description",
            description: "Name, description, owner, created and modified (with by whom), its workspaces and versions, its tabs. `description` sets it.",
            fields: vec![path(), opt("description", s(), "A new description.")],
            mutates: true,
            destructive: false,
            run: details,
        },
        Op {
            group: "documents",
            verb: "trash",
            summary: "Send a document or folder to the trash (Send to trash)",
            description: "It goes to the Trash (deleted after 30 days, Onshape's message in `message`) and leaves Recently opened. Restore it with documents_restore. The document must not be open.",
            fields: vec![path()],
            mutates: true,
            destructive: true,
            run: trash,
        },
        Op {
            group: "documents",
            verb: "restore",
            summary: "Restore an item of the trash (Trash › Restore)",
            description: "Back where it was, by its trash `id` or name (documents_list place \"trash\").",
            fields: vec![req("item", s(), "The trash item's id or name.")],
            mutates: true,
            destructive: false,
            run: restore,
        },
        Op {
            group: "documents",
            verb: "delete",
            summary: "Delete an item of the trash forever (Trash › Delete)",
            description: "By its trash `id` or name; it can't be undone.",
            fields: vec![req("item", s(), "The trash item's id or name.")],
            mutates: true,
            destructive: true,
            run: delete,
        },
        Op {
            group: "documents",
            verb: "empty_trash",
            summary: "Empty the trash (Trash › Empty trash…)",
            description: "Every item of the trash is deleted forever.",
            fields: vec![],
            mutates: true,
            destructive: true,
            run: empty_trash,
        },
        Op {
            group: "documents",
            verb: "label",
            summary: "Create a label, or put a label on a document or take it off (Labels…, Create ▾ › Label…)",
            description: "`create` makes a label (names are unique, case aside). With `path` the label goes on that document (`on` false takes it off).",
            fields: vec![
                req("label", s(), "The label's name."),
                opt("create", json!({ "type": "boolean" }), "Create the label first."),
                opt("path", s(), "A document to label."),
                opt("on", json!({ "type": "boolean", "default": true }), "On (default) or off the document."),
            ],
            mutates: true,
            destructive: false,
            run: label,
        },
        Op {
            group: "documents",
            verb: "forget_recent",
            summary: "Remove a document from Recently opened (its menu there)",
            description: "Only the list entry goes; the document stays.",
            fields: vec![path()],
            mutates: true,
            destructive: false,
            run: forget_recent,
        },
        Op {
            group: "documents",
            verb: "set_library",
            summary: "Set a folder as one of your libraries (Documents › a folder's Set library ▸)",
            description: "The folder becomes the user's library of that kind: My frame profiles library, My sheet metal forms, My default tolerances, My images or My standard content (one folder each; setting another replaces it). `clear` unsets the kind. The reply lists every library set.",
            fields: vec![
                req("library", json!({ "type": "string", "enum": crate::prefs::LIBRARIES.iter().map(|(k, _)| *k).collect::<Vec<_>>() }), "Which library (its key; the menu's row also works)."),
                opt("folder", s(), "The folder's path (required unless `clear`)."),
                opt("clear", json!({ "type": "boolean" }), "Unset that library."),
            ],
            mutates: true,
            destructive: false,
            run: set_library,
        },
    ]
}

fn documents(ctx: &Ctx) -> PathBuf {
    ctx.documents.clone().unwrap_or_else(noshape_scene::documents_dir)
}

/// A path as given, a bare name in the documents folder ("Bracket" → Bracket.noshape when no folder has that name).
fn resolve(ctx: &Ctx, given: &str) -> PathBuf {
    let p = Path::new(given);
    if p.is_absolute() || p.exists() {
        return p.to_path_buf();
    }
    let in_docs = documents(ctx).join(p);
    if in_docs.exists() || in_docs.extension().is_some() {
        return in_docs;
    }
    in_docs.with_extension(lib::EXT)
}

fn existing(ctx: &Ctx, a: &Args, param: &'static str) -> Result<PathBuf, OpError> {
    let p = resolve(ctx, a.str(param)?);
    if !p.exists() {
        return Err(OpError::not_found(format!("{} doesn't exist", p.display())).with_param(param));
    }
    Ok(p)
}

fn folder_arg(ctx: &Ctx, a: &Args, param: &'static str) -> Result<PathBuf, OpError> {
    let Some(f) = a.opt_str(param)? else { return Ok(documents(ctx)) };
    let p = resolve(ctx, f);
    if !p.is_dir() {
        return Err(OpError::not_found(format!("{} isn't a folder", p.display())).with_param(param));
    }
    Ok(p)
}

/// A document this session or the app has open can't be renamed, moved or trashed (the page: "Close the document's
/// window before …").
fn closed(ctx: &Ctx, p: &Path, what: &str) -> Result<(), OpError> {
    let canon = |x: &Path| x.canonicalize().unwrap_or_else(|_| x.to_path_buf());
    let open = |d: &Path| canon(d) == canon(p) || (p.is_dir() && canon(d).starts_with(canon(p)));
    if ctx.open_paths().into_iter().any(open) || ctx.live_server(p).is_some() {
        return Err(OpError::invalid(format!("Close the document before {what} it (document_close)")).with_param("path"));
    }
    Ok(())
}

fn failed(e: anyhow::Error, param: &str) -> OpError {
    OpError::invalid(format!("{e:#}")).with_param(param)
}

fn doc_json(p: &Path, labels: &lib::Labels) -> Value {
    let meta = noshape_store::Document::open_read_only(p).ok().and_then(|d| d.meta().ok());
    let name = meta.as_ref().map(|m| m.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let mine: Vec<&String> = labels.labels.iter().filter(|l| labels.has(p, l)).collect();
    json!({ "name": name, "path": p.display().to_string(), "modified": meta.as_ref().map(|m| m.modified), "description": meta.map(|m| m.description).unwrap_or_default(), "labels": mine })
}

fn trash_json(t: &TrashItem) -> Value {
    json!({ "id": t.id, "name": t.name, "folder": t.folder, "original": t.original.display().to_string(), "trashed": t.trashed })
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let labels = lib::Labels::load();
    let is_doc = |p: &Path| p.extension().is_some_and(|e| e == lib::EXT);
    if let Some(l) = a.opt_str("label")? {
        if !labels.labels.iter().any(|x| x == l) {
            return Err(OpError::not_found(format!("no label \"{l}\"; labels: {}", labels.labels.join(", "))).with_param("label"));
        }
        let docs: Vec<Value> = labels.assign.iter().filter(|(_, ls)| ls.iter().any(|x| x == l)).map(|(p, _)| PathBuf::from(p)).filter(|p| p.exists()).map(|p| doc_json(&p, &labels)).collect();
        return Ok(json!({ "label": l, "documents": docs, "labels": labels.labels }).into());
    }
    match a.opt_str("place")?.unwrap_or("owned") {
        "recent" => Ok(json!({ "place": "Recently opened", "documents": lib::recent().iter().map(|p| doc_json(p, &labels)).collect::<Vec<_>>(), "labels": labels.labels }).into()),
        "trash" => Ok(json!({ "place": "Trash", "items": Trash::load().items.iter().map(trash_json).collect::<Vec<_>>() }).into()),
        "owned" => {
            let dir = folder_arg(ctx, a, "folder")?;
            let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(|e| OpError::invalid(format!("{}: {e}", dir.display())).with_param("folder"))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
            let lower = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            let mtime = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
            match a.opt_str("sort")?.unwrap_or("name") {
                "name" => entries.sort_by_key(lower),
                "name_desc" => entries.sort_by_key(|p| std::cmp::Reverse(lower(p))),
                "modified" => entries.sort_by_key(|p| std::cmp::Reverse(mtime(p))),
                other => return Err(OpError::param("sort", &format!("\"{other}\": name, name_desc or modified"))),
            }
            let folders: Vec<Value> = entries.iter().filter(|p| p.is_dir()).map(|p| json!({ "name": p.file_name().map(|n| n.to_string_lossy().to_string()), "path": p.display().to_string() })).collect();
            let docs: Vec<Value> = entries.iter().filter(|p| p.is_file() && is_doc(p)).map(|p| doc_json(p, &labels)).collect();
            Ok(json!({ "place": "Owned by me", "folder": dir.display().to_string(), "folders": folders, "documents": docs, "labels": labels.labels }).into())
        }
        other => Err(OpError::param("place", &format!("\"{other}\" isn't a place: owned, recent or trash"))),
    }
}

fn create_folder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let place = folder_arg(ctx, a, "folder")?;
    let name = a.opt_str("name")?.map(str::to_string).unwrap_or_else(|| lib::next_untitled_folder(&place));
    let p = lib::create_folder(&place, &name).map_err(|e| failed(e, "name"))?;
    Ok(json!({ "created": p.display().to_string(), "message": "Folder created." }).into())
}

fn rename(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let p = existing(ctx, a, "path")?;
    closed(ctx, &p, "renaming")?;
    let name = a.str("name")?;
    let dest = if p.is_dir() { lib::rename_folder(&p, name) } else { lib::rename_doc(&p, name) }.map_err(|e| failed(e, "name"))?;
    Ok(json!({ "path": dest.display().to_string() }).into())
}

fn move_to(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let p = existing(ctx, a, "path")?;
    let to = folder_arg(ctx, a, "to")?;
    if a.opt_str("to")?.is_none() {
        return Err(OpError::param("to", "the folder to move it into is required"));
    }
    closed(ctx, &p, "moving")?;
    let dest = if p.is_dir() { lib::move_folder(&p, &to) } else { lib::move_doc(&p, &to) }.map_err(|e| failed(e, "to"))?;
    Ok(json!({ "path": dest.display().to_string() }).into())
}

fn copy(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let p = existing(ctx, a, "path")?;
    let to = match a.opt_str("to")? {
        Some(_) => folder_arg(ctx, a, "to")?,
        None => p.parent().map(Path::to_path_buf).unwrap_or_else(|| documents(ctx)),
    };
    let name = match a.opt_str("name")? {
        Some(n) => n.to_string(),
        None => format!("{} - Copy", doc_json(&p, &lib::Labels::default())["name"].as_str().unwrap_or_default()),
    };
    if let Some(od) = ctx.get_mut(&p) {
        let _ = od.session.store.checkpoint();
    }
    let dest = lib::copy_document_named(&p, &to, &name).map_err(|e| failed(e, "path"))?;
    Ok(json!({ "copy": dest.display().to_string(), "name": name }).into())
}

fn details(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let p = existing(ctx, a, "path")?;
    if let Some(d) = a.opt_str("description")? {
        closed(ctx, &p, "editing")?;
        let label = lib::doc_details(&p).map(|d| d.library_label).unwrap_or_default();
        lib::set_doc_properties(&p, d, &label).map_err(|e| failed(e, "description"))?;
    }
    let d = lib::doc_details(&p).map_err(|e| failed(e, "path"))?;
    let history: Vec<Value> = d.history.iter().map(|h| json!({ "name": h.name, "description": h.description })).collect();
    let tabs: Vec<&String> = d.tabs.iter().map(|t| &t.1).collect();
    Ok(json!({ "name": d.name, "description": d.description, "created_by": d.created_by, "created": d.created, "modified_by": d.modified_by, "modified": d.modified, "workspaces_and_versions": history, "tabs": tabs }).into())
}

fn trash(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let p = existing(ctx, a, "path")?;
    closed(ctx, &p, "sending to the trash")?;
    let folder = p.is_dir();
    let name = if folder { p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() } else { doc_json(&p, &lib::Labels::default())["name"].as_str().unwrap_or_default().to_string() };
    let item = lib::send_to_trash(&p, &name, folder).map_err(|e| failed(e, "path"))?;
    Ok(json!({ "trashed": trash_json(&item), "message": format!("{name} has been sent to the trash and will be automatically deleted after 30 days.") }).into())
}

fn trash_item(a: &Args) -> Result<TrashItem, OpError> {
    let key = a.str("item")?;
    let t = Trash::load();
    let found: Vec<&TrashItem> = t.items.iter().filter(|i| i.id == key || i.name == key).collect();
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(OpError::not_found(format!("nothing in the trash is \"{key}\"; items: {}", t.items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(", "))).with_param("item")),
        _ => Err(OpError::param("item", &format!("{} items are named \"{key}\": give its id", found.len()))),
    }
}

fn restore(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let item = trash_item(a)?;
    let (dest, item) = lib::restore(&item.id, &documents(ctx)).map_err(|e| failed(e, "item"))?;
    Ok(json!({ "restored": dest.display().to_string(), "message": format!("{} has been restored.", item.name) }).into())
}

fn delete(_ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let item = trash_item(a)?;
    Trash::load().delete(&item.id).map_err(|e| failed(e, "item"))?;
    Ok(json!({ "deleted": item.name, "message": format!("{} has been permanently deleted.", item.name) }).into())
}

fn empty_trash(_ctx: &mut Ctx, _a: &Args) -> Result<OpOutput, OpError> {
    let mut t = Trash::load();
    let n = t.items.len();
    t.empty().map_err(|e| OpError::invalid(format!("Could not empty the trash: {e:#}")))?;
    Ok(json!({ "deleted": n }).into())
}

fn label(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let name = a.str("label")?.trim().to_string();
    let mut labels = lib::Labels::load();
    if a.bool_or("create", false)? && !labels.create(&name) {
        return Err(OpError::invalid(format!("A label named \"{name}\" already exists")).with_param("label"));
    }
    if !labels.labels.contains(&name) {
        return Err(OpError::not_found(format!("no label \"{name}\" (create it with `create`); labels: {}", labels.labels.join(", "))).with_param("label"));
    }
    if a.opt_str("path")?.is_some() {
        let p = existing(ctx, a, "path")?;
        if labels.has(&p, &name) != a.bool_or("on", true)? {
            labels.toggle(&p, &name);
        }
    }
    labels.save();
    Ok(json!({ "labels": labels.labels }).into())
}

fn forget_recent(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let p = resolve(ctx, a.str("path")?);
    lib::remove_recent(&p);
    Ok(json!({ "recent": lib::recent().iter().map(|p| p.display().to_string()).collect::<Vec<_>>() }).into())
}

fn set_library(_ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let Some(key) = crate::prefs::LibraryFolders::key(a.str("library")?) else {
        return Err(OpError::param("library", "not a library: frame_profiles, sheet_metal_forms, tolerances, images or standard_content"));
    };
    let mut libs = crate::prefs::LibraryFolders::load();
    if a.bool_or("clear", false)? {
        libs.0.remove(key);
    } else {
        let Some(folder) = a.opt_str("folder")? else { return Err(OpError::param("folder", "the folder to use is required")) };
        let folder = PathBuf::from(folder);
        if !folder.is_dir() {
            return Err(OpError::invalid(format!("{} isn't a folder", folder.display())).with_param("folder"));
        }
        libs.0.insert(key.to_string(), folder);
    }
    libs.save().map_err(|e| OpError::new("io", format!("saving the libraries: {e}")))?;
    Ok(json!({ "libraries": libs.0 }).into())
}
