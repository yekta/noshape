//! The last drawing menu items: table properties (BOM, cut list, revision,
//! custom, general), Copy / Paste of views and annotations, Paste as table,
//! Replace (a view's reference, a BOM table's source), Replicate
//! annotations, and the navigation items Switch to reference / Open Feature
//! Studio in linked document (the target in the reply; with the app open,
//! the window goes there as the menu item does). Shared code in
//! `noshape_scene::drawing_tools` (table_edit, view_edit, replicate) and the
//! model's clipboard.

use super::drawing::{commit, drawing_field, part_arg, point, point_value, reply, sheet_field, source_tab};
use super::drawing_tools::{CORNERS, Target, done, enum_arg, pt_arg, target, view_arg};
use super::{doc_field, s};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, UiRequest};
use noshape_model::drawing::clipboard::ViewClipboard;
use noshape_model::drawing::{TableKind, ViewId};
use noshape_scene::drawing_tools::{self as dt, Tool, replicate, table_edit as te, view_edit as ve};
use serde_json::{Value, json};

fn ids() -> Value {
    json!({ "type": "array", "items": { "type": "integer" } })
}

fn common() -> Vec<crate::registry::Field> {
    vec![doc_field(), drawing_field(), sheet_field()]
}

fn with(mut a: Vec<crate::registry::Field>, b: Vec<crate::registry::Field>) -> Vec<crate::registry::Field> {
    a.extend(b);
    a
}

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "drawing",
            verb: "table_properties",
            summary: "A placed table's properties (BOM, cut list, revision, custom or general table)",
            description: "The table's right-click › … table properties dialog, one edit with its name. BOM: `structured`, `bottom_to_top`, `corner` (\"BOM table properties\"); cut list: `bottom_to_top`, `corner` (\"Cut list table properties\"); revision: `see_sheet1`, `visible_rows` (1-100), `newest_top`, `hide_callouts`, `corner` (\"Revision table properties\"); custom: `bottom_to_top`, `corner` (\"Custom table properties\"; its table and Part Studio: drawing_custom_table with `id`); general and hole tables: Table properties… `corner`, the table staying where it is (\"Table fixed corner\"). Fields not given keep their values; a field the table's dialog doesn't have is refused.",
            fields: with(
                common(),
                vec![
                    req("table", json!({ "type": "integer" }), "Table id (drawing_get)."),
                    opt("corner", json!({ "type": "string", "enum": CORNERS.iter().map(|c| c.0).collect::<Vec<_>>() }), "Fixed corner."),
                    opt("structured", json!({ "type": "boolean" }), "BOM: Structured (else Flattened)."),
                    opt("bottom_to_top", json!({ "type": "boolean" }), "BOM, cut list, custom: rows bottom to top."),
                    opt("see_sheet1", json!({ "type": "boolean" }), "Revision: \"See sheet 1\" on the other sheets (else Linked)."),
                    opt("visible_rows", json!({ "type": "integer", "minimum": 1, "maximum": 100 }), "Revision: visible rows."),
                    opt("newest_top", json!({ "type": "boolean" }), "Revision: order Bottom to top (newest at the top)."),
                    opt("hide_callouts", json!({ "type": "boolean" }), "Revision: hide callouts no longer in the table."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: table_properties,
        },
        Op {
            group: "drawing",
            verb: "copy",
            summary: "Copy views (with their derived views, annotations and tables), or annotations and tables",
            description: "Copy (Ctrl+C, the view and annotation menus): with `views`, those views with the views derived from them and their annotations; else the `annotations` and `tables`. Kept for drawing_paste in this session, and returned as `clipboard` (the editor's clipboard text, to paste into another session or drawing).",
            fields: with(common(), vec![opt("views", ids(), "View ids."), opt("annotations", ids(), "Annotation ids (without `views`)."), opt("tables", ids(), "Table ids (without `views`).")]),
            mutates: false,
            destructive: false,
            run: copy,
        },
        Op {
            group: "drawing",
            verb: "paste",
            summary: "Paste copied views, annotations and tables centered at a point",
            description: "Paste (Ctrl+V, the drawing menus): what drawing_copy copied (or `clipboard` text), centered on `position` (sheet mm), one edit \"Paste views\". Returns the new view ids and annotation ids.",
            fields: with(common(), vec![req("position", point(), "Where the copy's center goes (sheet mm)."), opt("clipboard", s(), "Clipboard text from drawing_copy (default: this session's copy).")]),
            mutates: true,
            destructive: false,
            run: paste,
        },
        Op {
            group: "drawing",
            verb: "paste_table",
            summary: "Paste spreadsheet cells as a new table",
            description: "Paste as table: spreadsheet text (rows of tab-separated cells, as a spreadsheet copies them) becomes a general table with its top-left corner at `position` (sheet mm); one edit \"Paste as table\". Returns the table id.",
            fields: with(common(), vec![req("text", s(), "Rows separated by newlines, cells by tabs."), req("position", point(), "Top-left corner (sheet mm).")]),
            mutates: true,
            destructive: false,
            run: paste_table,
        },
        Op {
            group: "drawing",
            verb: "replace",
            summary: "Replace a view's model reference, or a BOM table's assembly",
            description: "With `view`: the reference's Replace… (sheets panel reference row, view row): on every sheet, the base views showing that view's source (and part) show `source` (and `part`) instead. With `table` (a BOM table): its Replace, every base view on the sheet showing the table's assembly shows `source`. One edit \"Replace\".",
            fields: with(
                common(),
                vec![
                    opt("view", json!({ "type": "integer" }), "A view whose reference to replace."),
                    opt("table", json!({ "type": "integer" }), "A BOM table whose source to replace (instead of `view`)."),
                    req("source", s(), "The new Part Studio or Assembly (tab name or id)."),
                    opt("part", s(), "With `view`: the new part (id or name) of a Part Studio; default the whole Part Studio."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: replace,
        },
        Op {
            group: "drawing",
            verb: "replicate",
            summary: "Replicate annotations from one view onto another (beta)",
            description: "Select for replication ▸ View (or Region… with `region`), then Replicate annotations on view `to`: a copy of each annotation where `to` shows the same geometry (each reference the same model point, or a vertex / circle center at the mapped spot); the others are skipped. With a region, `at` is where its box's center goes on `to` (sheet mm; snapped to the nearest matching vertex or center within 3 mm). One edit \"Replicate annotations\"; the reply's message is the editor's (\"Successfully replicated 7/7 annotations\").",
            fields: with(
                common(),
                vec![
                    req("from", json!({ "type": "integer" }), "The view whose annotations to replicate."),
                    opt("region", json!({ "type": "array", "items": point(), "minItems": 2, "maxItems": 2 }), "Region…: two opposite corners (sheet mm); only the annotations drawn inside."),
                    req("to", json!({ "type": "integer" }), "The view to copy them onto."),
                    opt("at", point(), "With `region`: the box's center on `to` (sheet mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: replicate_op,
        },
        Op {
            group: "drawing",
            verb: "switch_to_reference",
            summary: "The Part Studio, Assembly or Feature Studio a view or table refers to (Switch to …)",
            description: "Switch to reference (view reference menu), Switch to <assembly> (BOM table), Switch to Custom tables : <Part Studio> / Switch to <Feature Studio> (custom table): the target tab (id, name, kind), the part a view shows, a pinned version. With the app open on the document (live mode) the window switches to that tab, as the menu item does.",
            fields: with(common(), vec![opt("view", json!({ "type": "integer" }), "A view."), opt("table", json!({ "type": "integer" }), "A BOM or custom table (instead of `view`)."), opt("feature_studio", json!({ "type": "boolean" }), "Custom table: its Feature Studio instead of its Part Studio.")]),
            mutates: false,
            destructive: false,
            run: switch_to_reference,
        },
        Op {
            group: "drawing",
            verb: "open_feature_studio",
            summary: "The Feature Studio a custom table comes from (Open Feature Studio in linked document)",
            description: "A custom table's Feature Studio: in a linked document (a table defined in another document's version) the document's path, the version and the Feature Studio tab, and with the app open (live mode) the document opens in a window, as \"Open Feature Studio in linked document\" does; in this document the tab, switched to (\"Switch to <Feature Studio>\").",
            fields: with(common(), vec![req("table", json!({ "type": "integer" }), "A custom table.")]),
            mutates: false,
            destructive: false,
            run: open_feature_studio,
        },
    ]
}

// ------------------------------------------------------------ tables

fn table_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let id = a.opt_u64("table")?.ok_or_else(|| OpError::param("table", "a table id"))?;
    let sheet = t.sheet;
    let Some(old) = t.d.sheets[sheet].table(id).cloned() else { return Err(OpError::param("table", "no table with that id on the sheet (drawing_get)")) };
    let allowed: &[&str] = match old.kind {
        TableKind::Bom { .. } => &["corner", "structured", "bottom_to_top"],
        TableKind::CutList { .. } | TableKind::Custom { .. } => &["corner", "bottom_to_top"],
        TableKind::Revision { .. } => &["corner", "see_sheet1", "visible_rows", "newest_top", "hide_callouts"],
        _ => &["corner"],
    };
    for k in ["corner", "structured", "bottom_to_top", "see_sheet1", "visible_rows", "newest_top", "hide_callouts"] {
        if a.get(k).is_some() && !allowed.contains(&k) {
            return Err(OpError::param(k, &format!("this table's properties have no `{k}` (they have: {})", allowed.join(", "))));
        }
    }
    let corner = enum_arg(a, "corner", &CORNERS, old.corner)?;
    let layout = t.m.built.as_ref().and_then(|b| b.table_layout(id)).cloned();
    let table = t.d.sheets[sheet].tables.iter_mut().find(|x| x.id == id).expect("found");
    let description = match &mut table.kind {
        TableKind::Bom { structured, bottom_to_top, .. } => {
            *structured = a.bool_or("structured", *structured)?;
            *bottom_to_top = a.bool_or("bottom_to_top", *bottom_to_top)?;
            table.corner = corner;
            "BOM table properties"
        }
        TableKind::CutList { bottom_to_top, .. } => {
            *bottom_to_top = a.bool_or("bottom_to_top", *bottom_to_top)?;
            table.corner = corner;
            "Cut list table properties"
        }
        TableKind::Custom { bottom_to_top, .. } => {
            *bottom_to_top = a.bool_or("bottom_to_top", *bottom_to_top)?;
            table.corner = corner;
            "Custom table properties"
        }
        TableKind::Revision { see_sheet1, visible_rows, newest_top, hide_callouts, .. } => {
            let rows = match a.opt_u64("visible_rows")? {
                Some(n) if (1..=100).contains(&n) => n as u32,
                Some(_) => return Err(OpError::param("visible_rows", "1 to 100")),
                None => *visible_rows,
            };
            let o = dt::RevisionTableOpts { see_sheet1: a.bool_or("see_sheet1", *see_sheet1)?, visible_rows: rows, newest_top: a.bool_or("newest_top", *newest_top)?, hide_callouts: a.bool_or("hide_callouts", *hide_callouts)?, corner };
            table.kind = dt::revision_table_kind(&o, Some(&old.kind));
            table.corner = corner;
            "Revision table properties"
        }
        _ => {
            te::set_fixed_corner(table, layout.as_ref(), corner);
            "Table fixed corner"
        }
    };
    done(od, t, description, "table", id)
}

fn paste_table(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let cells = te::parse_cells(a.str("text")?).ok_or_else(|| OpError::param("text", "not a table: rows of tab-separated cells (a tab, or several lines)"))?;
    let at = pt_arg(a, "position")?;
    let sheet = t.sheet;
    let id = t.d.sheets[sheet].next_table_id();
    t.d.sheets[sheet].tables.push(te::pasted_table(id, cells, at));
    done(od, t, "Paste as table", "table", id)
}

// ------------------------------------------------------------ copy / paste

fn id_list(a: &Args, name: &str) -> Result<Vec<u64>, OpError> {
    match a.opt_array(name)? {
        None => Ok(Vec::new()),
        Some(l) => l.iter().enumerate().map(|(i, v)| v.as_u64().ok_or_else(|| OpError::param(&format!("{name}[{i}]"), "an id"))).collect(),
    }
}

fn copy(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (views, anns, tables) = (id_list(a, "views")?, id_list(a, "annotations")?, id_list(a, "tables")?);
    if views.is_empty() && anns.is_empty() && tables.is_empty() {
        return Err(OpError::invalid("nothing to copy: give `views`, or `annotations` / `tables`"));
    }
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let sheet = &t.d.sheets[t.sheet];
    for (i, v) in views.iter().enumerate() {
        if sheet.view(ViewId(*v)).is_none() {
            return Err(OpError::param(&format!("views[{i}]"), "no such view on the sheet"));
        }
    }
    let clip = if views.is_empty() {
        t.d.copy_entities(t.sheet, &anns, &tables)
    } else {
        let ids: Vec<ViewId> = views.iter().map(|v| ViewId(*v)).collect();
        t.d.copy_views(t.sheet, &ids, Some(&t.m.sources))
    }
    .map_err(OpError::invalid)?;
    let c = &clip.sheet;
    let (nv, na, nt) = (c.views.len(), c.annotations.len(), c.tables.len());
    let text = clip.to_json();
    ctx.drawing_clipboard = Some(text.clone());
    let message = match nv {
        0 => None,
        1 => Some("Copied 1 view".to_string()),
        n => Some(format!("Copied {n} views")),
    };
    Ok(json!({ "copied": { "views": nv, "annotations": na, "tables": nt }, "message": message, "clipboard": text }).into())
}

fn paste(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let text = match a.opt_str("clipboard")? {
        Some(t) => t.to_string(),
        None => ctx.drawing_clipboard.clone().ok_or_else(|| OpError::invalid("nothing copied in this session: call drawing_copy first, or pass `clipboard`"))?,
    };
    let clip = ViewClipboard::from_json(&text).ok_or_else(|| OpError::param("clipboard", "not drawing content copied with drawing_copy"))?;
    let at = point_value(a.get("position").ok_or_else(|| OpError::param("position", "required"))?, "position")?;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let sheet = t.sheet;
    let first = t.d.sheets[sheet].next_annotation_id();
    let tables_before: Vec<u64> = t.d.sheets[sheet].tables.iter().map(|x| x.id).collect();
    let views = t.d.sheets[sheet].paste_views(&clip, Some(at));
    let anns: Vec<u64> = t.d.sheets[sheet].annotations.iter().map(|x| x.id).filter(|id| *id >= first).collect();
    let tables: Vec<u64> = t.d.sheets[sheet].tables.iter().map(|x| x.id).filter(|id| !tables_before.contains(id)).collect();
    let tab = t.tab;
    commit(od, tab, sheet, t.d, "Paste views")?;
    let mut out = reply(od, tab, sheet, false)?;
    out["pasted"] = json!({ "views": views.iter().map(|v| v.0).collect::<Vec<_>>(), "annotations": anns, "tables": tables });
    Ok(out.into())
}

// ------------------------------------------------------------ replace / replicate

fn replace(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let new = source_tab(od, a, "source")?;
    let sheet = t.sheet;
    match (a.opt_u64("view")?, a.opt_u64("table")?) {
        (Some(_), Some(_)) => return Err(OpError::param("table", "give `view` or `table`, not both")),
        (Some(_), None) => {
            let v = view_arg(&t, a, "view")?;
            let (key, old_part) = noshape_model::drawing::views::view_source(&t.d.sheets[sheet], v).ok_or_else(|| OpError::param("view", "the view shows no model"))?;
            let new_part = part_arg(od, new, a)?;
            ve::replace_reference(&mut t.d, key.tab, &old_part, new, new_part);
        }
        (None, Some(id)) => {
            if a.get("part").is_some() {
                return Err(OpError::param("part", "a BOM table's Replace takes an assembly"));
            }
            let view = match t.d.sheets[sheet].table(id).map(|x| &x.kind) {
                Some(TableKind::Bom { view, .. }) => *view,
                Some(_) => return Err(OpError::param("table", "Replace is on BOM tables")),
                None => return Err(OpError::param("table", "no table with that id on the sheet")),
            };
            let (key, _) = noshape_model::drawing::views::view_source(&t.d.sheets[sheet], view).ok_or_else(|| OpError::param("table", "the table's view shows no model"))?;
            ve::replace_sheet_source(&mut t.d.sheets[sheet], key.tab, new);
        }
        (None, None) => return Err(OpError::param("view", "give `view` (its reference) or `table` (a BOM table)")),
    }
    let tab = t.tab;
    commit(od, tab, sheet, t.d, "Replace")?;
    Ok(reply(od, tab, sheet, false)?.into())
}

fn replicate_op(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let (from, to) = (view_arg(&t, a, "from")?, view_arg(&t, a, "to")?);
    let region = match a.opt_array("region")? {
        Some(r) => {
            let p = |i: usize| point_value(&r[i], &format!("region[{i}]"));
            let (p0, p1) = (p(0)?, p(1)?);
            Some([[p0[0].min(p1[0]), p0[1].min(p1[1])], [p0[0].max(p1[0]), p0[1].max(p1[1])]])
        }
        None => None,
    };
    let delta = match (region, a.get("at")) {
        (Some(r), Some(_)) => Some(replicate::region_delta(r, pt_arg(a, "at")?)),
        (Some(_), None) => return Err(OpError::param("at", "required with `region`: the box's center on `to`")),
        (None, Some(_)) => return Err(OpError::param("at", "only with `region`")),
        (None, None) => None,
    };
    let (out, tried) = {
        let tool = Tool::Select;
        let pk = t.m.pick(&tool, &[], [0.0; 2]);
        let r = pk.select_for_replication(from, region).ok_or_else(|| OpError::param("from", "the view shows no model geometry"))?;
        pk.replicate_into(&r, to, delta).ok_or_else(|| OpError::param("to", "the view shows no model geometry"))?
    };
    let n = out.len();
    let ids: Vec<u64> = out.iter().map(|x| x.id).collect();
    let sheet = t.sheet;
    let tab = t.tab;
    let mut reply_v = if n > 0 {
        t.d.sheets[sheet].annotations.extend(out);
        commit(od, tab, sheet, t.d, "Replicate annotations")?;
        reply(od, tab, sheet, false)?
    } else {
        json!({})
    };
    reply_v["replicated"] = json!(ids);
    reply_v["tried"] = json!(tried);
    reply_v["message"] = json!(if n > 0 { format!("Successfully replicated {n}/{tried} annotations") } else { "No annotations replicated".to_string() });
    Ok(reply_v.into())
}

// ------------------------------------------------------------ navigation

fn switch_to_reference(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t: Target = target(od, a)?;
    let sheet = &t.d.sheets[t.sheet];
    let want_fs = a.bool_or("feature_studio", false)?;
    let (tab, mut out) = match (a.opt_u64("view")?, a.opt_u64("table")?) {
        (Some(_), Some(_)) => return Err(OpError::param("table", "give `view` or `table`, not both")),
        (Some(_), None) => {
            if want_fs {
                return Err(OpError::param("feature_studio", "custom tables only"));
            }
            let v = view_arg(&t, a, "view")?;
            let (key, part) = noshape_model::drawing::views::view_source(sheet, v).ok_or_else(|| OpError::param("view", "the view shows no model"))?;
            let mut o = json!({ "target": super::tab_json(od, key.tab) });
            if let Some(p) = part {
                let name = od.regenerate(key.tab, None).ok().and_then(|r| r.display.bodies.iter().find(|b| b.id == p).map(|b| b.name.clone()));
                o["part"] = json!({ "id": crate::refs::body_id(&p), "name": name });
            }
            if let Some(v) = &key.version {
                o["version"] = json!(v);
            }
            (key.tab, o)
        }
        (None, Some(id)) => match sheet.table(id).map(|x| x.kind.clone()) {
            Some(TableKind::Bom { view, .. }) => {
                if want_fs {
                    return Err(OpError::param("feature_studio", "custom tables only"));
                }
                let (key, _) = noshape_model::drawing::views::view_source(sheet, view).ok_or_else(|| OpError::param("table", "the table's view shows no model"))?;
                (key.tab, json!({ "target": super::tab_json(od, key.tab) }))
            }
            Some(TableKind::Custom { studio, fs, fs_version, .. }) => {
                if want_fs {
                    if fs_version.as_deref().and_then(crate::derived::ExternalRef::parse).is_some() {
                        return Err(OpError::param("feature_studio", "the table's Feature Studio is in a linked document: drawing_open_feature_studio"));
                    }
                    (fs, json!({ "target": super::tab_json(od, fs) }))
                } else {
                    (studio, json!({ "target": super::tab_json(od, studio) }))
                }
            }
            Some(_) => return Err(OpError::param("table", "a BOM or custom table (other tables refer to no tab)")),
            None => return Err(OpError::param("table", "no table with that id on the sheet")),
        },
        (None, None) => return Err(OpError::param("view", "give `view` or `table`")),
    };
    out["shown_in_app"] = json!(super::show(ctx, a.opt_str("doc")?, UiRequest::ActivateTab(tab))?);
    Ok(out.into())
}

fn open_feature_studio(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let id = a.opt_u64("table")?.ok_or_else(|| OpError::param("table", "a table id"))?;
    let Some(TableKind::Custom { fs, name, fs_version, .. }) = t.d.sheets[t.sheet].table(id).map(|x| x.kind.clone()) else {
        return Err(OpError::param("table", "not a custom table on the sheet"));
    };
    match fs_version.as_deref().and_then(crate::derived::ExternalRef::parse) {
        Some(e) => {
            let mut out = json!({ "linked": true, "document": e.path.display().to_string(), "version": e.microversion.to_string(), "feature_studio": e.tab, "table": name });
            out["shown_in_app"] = json!(super::show(ctx, a.opt_str("doc")?, UiRequest::OpenDocument { path: e.path, version: None, external_key: None })?);
            Ok(out.into())
        }
        None => {
            let mut out = json!({ "linked": false, "target": super::tab_json(od, fs), "table": name });
            out["shown_in_app"] = json!(super::show(ctx, a.opt_str("doc")?, UiRequest::ActivateTab(fs))?);
            Ok(out.into())
        }
    }
}
