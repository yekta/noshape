//! The document window: what a user does with the window that isn't an
//! edit, so an agent can do it and read it back. The tab shown, the Tab
//! manager, the instance and in-context navigation, viewing a version,
//! compare and repair, the drawing's sheets. Each one changes the
//! document's [`crate::window::WindowState`] (read with `ui_state`) and,
//! with the app open on the document (live mode), the window does the same.

use super::{doc_field, s, show, tab_json};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::window::{CompareRequest, CompareSide, TabSort, UiRequest, tab_compare, tab_kind, tab_manager_rows};
use crate::{Ctx, OpenDoc, doc};
use noshape_model::assembly::InstanceSource;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_store::MicroversionId;
use serde_json::{Value, json};

fn strings() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "ui",
            verb: "state",
            summary: "The document window's state: the tab shown, panels, the version viewed, compare and repair",
            description: "What the window shows besides the model: the tab shown (`active_tab`), the version viewed (`viewing`, null: the workspace), the Tab manager (open, search), the drawings whose Sheets panel is open, compare mode (base and target) and the Repair panel. Set by the window tools (tab_activate, tab_manager, history_view, history_compare, history_repair_view, drawing_sheets_panel…); with the app open (live mode) they change its window too.",
            fields: vec![doc_field()],
            mutates: false,
            destructive: false,
            run: state,
        },
        Op {
            group: "ui",
            verb: "preferences",
            summary: "Graphics preferences (the camera menu's Graphics preferences…)",
            description: "The user's Graphics preferences: `graphics` without fields replies with them (ambient_occlusion 0–1, fov_deg 20–120, line widths in px 0.1–3, shininess 0.01–1, colors \"#rrggbb\", construction [dash, gap, dash, gap] px); with fields, those change and are saved (per user, for every document), as the dialog's ✓ does. render_image draws with them; with the app open (live mode) every window applies them at once.",
            fields: vec![opt("graphics", json!({ "type": "object" }), "Preferences to change, by name (see the reply).")],
            mutates: false,
            destructive: false,
            run: preferences,
        },
        Op {
            group: "tab",
            verb: "activate",
            summary: "Show a tab (click it in the tab bar)",
            description: "The tab the window shows (`ui_state` › active_tab); with the app open (live mode) its window switches to it.",
            fields: vec![doc_field(), req("tab", s(), "Tab name or id.")],
            mutates: false,
            destructive: false,
            run: activate,
        },
        Op {
            group: "tab",
            verb: "manager",
            summary: "The Tab manager: the tabs matching a search and type filters, sorted",
            description: "The Tab manager (Alt+T) as it lists the tabs: `search` (in the names, case-insensitive), `kinds` (type filters: partstudio, assembly, drawing, variables, featurestudio, materiallibrary, file), `sort` (tab_order with folders, name_asc, name_desc, type). `open` (default true) opens it in the window (live mode) with that search; false closes it. `activate` shows one of the rows (Enter on it).",
            fields: vec![
                doc_field(),
                opt("search", s(), "Search text."),
                opt("kinds", strings(), "Type filters."),
                opt("sort", json!({ "type": "string", "enum": ["tab_order", "name_asc", "name_desc", "type"] }), "Sort order (default tab_order)."),
                opt("open", json!({ "type": "boolean", "default": true }), "Open (true) or close the panel."),
                opt("activate", s(), "Show this tab (name or id)."),
            ],
            mutates: false,
            destructive: false,
            run: manager,
        },
        Op {
            group: "assembly",
            verb: "switch_to_tab",
            summary: "The Part Studio or Assembly an instance comes from (instance menu › Switch to …)",
            description: "Instance (or BOM row) menu › Switch to <Part Studio> / <Assembly>: the source tab (id, name, kind), the part and the configuration the instance shows; the window shows the tab (live mode). Instances from another document: assembly_open_linked_document.",
            fields: vec![doc_field(), super::assembly::asm_field(), req("instance", s(), "Instance id or name.")],
            mutates: false,
            destructive: false,
            run: switch_to_tab,
        },
        Op {
            group: "assembly",
            verb: "open_linked_document",
            summary: "The document version an instance comes from (instance menu › Open linked document)",
            description: "For an instance inserted from another document: its path, the version and the tab; with the app open (live mode) that version opens in a window on the tab, as the menu item does.",
            fields: vec![doc_field(), super::assembly::asm_field(), req("instance", s(), "Instance id or name.")],
            mutates: false,
            destructive: false,
            run: open_linked_document,
        },
        Op {
            group: "context",
            verb: "go_to_assembly",
            summary: "The assembly of the Assembly context a Part Studio shows (in-context banner › Go to assembly)",
            description: "Modeling in context: the shown context's assembly tab; the window switches to it (live mode).",
            fields: vec![doc_field(), opt("tab", s(), "Part Studio tab (default: the first).")],
            mutates: false,
            destructive: false,
            run: go_to_assembly,
        },
        Op {
            group: "history",
            verb: "view",
            summary: "View the document at a version or history entry (Versions and history › Open), or return to the workspace",
            description: "Open a version or change read-only, as the history panel's Open does: every later call reads the document as it was then, edits are refused (\"This is a read-only view of history…\") until `to` \"workspace\". Replies with what the version holds (tabs; Part Studios' features and parts). With the app open (live mode) its window shows it.",
            fields: vec![doc_field(), req("to", s(), "A version name or id, a microversion id (history_list), or \"workspace\".")],
            mutates: true,
            destructive: false,
            run: view,
        },
        Op {
            group: "history",
            verb: "compare",
            summary: "Compare two history points (Versions and history › Compare)",
            description: "Compare mode between `base` (default: what the window shows, the workspace or the version viewed) and `target` (a version, a change or \"workspace\"): each tab's state (differs, same, not_supported for drawings, only_in_base) and, per Part Studio that differs, its features' changes (added, removed, modified, moved; parameters changed). `stop` leaves compare mode. With the app open (live mode) the window enters compare mode.",
            fields: vec![doc_field(), opt("target", s(), "Version name or id, microversion, or \"workspace\"."), opt("base", s(), "The other side (default: what the window shows)."), opt("stop", json!({ "type": "boolean" }), "Leave compare mode.")],
            mutates: false,
            destructive: false,
            run: compare,
        },
        Op {
            group: "history",
            verb: "repair_view",
            summary: "View a history entry in the Repair panel (Versions and history › View in repair)",
            description: "The Part Studios as they were at `at` (a version or a change), to repair references broken since: their features (with status) and parts. With the app open (live mode) the Repair panel opens on it. `close` closes it.",
            fields: vec![doc_field(), opt("at", s(), "Version name or id, or microversion."), opt("close", json!({ "type": "boolean" }), "Close the Repair panel.")],
            mutates: false,
            destructive: false,
            run: repair_view,
        },
        Op {
            group: "drawing",
            verb: "switch_sheet",
            summary: "Show another sheet (sheet tabs; First / Last / Next / Previous sheet)",
            description: "The sheet the drawing shows: `sheet` (number or name) or `step` first | last | next | previous (Home / End / Page down / Page up). As in Onshape a sheet switch is an undoable change (\"Switch sheet\"); the window follows it (live mode).",
            fields: vec![doc_field(), super::drawing::drawing_field(), super::drawing::sheet_field(), opt("step", json!({ "type": "string", "enum": ["first", "last", "next", "previous"] }), "Instead of `sheet`.")],
            mutates: true,
            destructive: false,
            run: switch_sheet,
        },
        Op {
            group: "drawing",
            verb: "sheets_panel",
            summary: "The Sheets panel: every sheet with its views and tables (Ctrl+S, Display sheet menu)",
            description: "What the Sheets panel lists: each sheet (number, name, the one shown) with its views (id, kind, label) and tables (id, kind). `open` (default true) opens it in the window (live mode); false closes it.",
            fields: vec![doc_field(), super::drawing::drawing_field(), opt("open", json!({ "type": "boolean", "default": true }), "Open (true) or close the panel.")],
            mutates: false,
            destructive: false,
            run: sheets_panel,
        },
    ]
}

// ------------------------------------------------------------ state, tabs

fn state(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let w = od.window.clone();
    let active = w.active_tab.filter(|t| od.doc().tab(*t).is_some()).or_else(|| od.doc().ordered_tabs().first().map(|t| t.id));
    Ok(json!({
        "active_tab": active.map(|t| tab_json(od, t)),
        "viewing": od.session.viewing_version(),
        "tab_manager": { "open": w.tab_manager.is_some(), "search": w.tab_manager },
        "sheets_panel_open": w.sheets_panel.iter().map(|t| tab_json(od, *t)).collect::<Vec<_>>(),
        "compare": w.compare.as_ref().map(|c| json!({ "base": c.base_label, "target": c.target_label })),
        "repair": w.repair.as_ref().map(|r| json!({ "at": r.0.to_string(), "label": r.1 })),
    })
    .into())
}

fn preferences(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let mut prefs = crate::prefs::load_graphics();
    if let Some(set) = a.object("graphics")?.filter(|m| !m.is_empty()) {
        prefs = crate::prefs::graphics_with(&prefs, set).map_err(|(field, why)| OpError::param(&format!("graphics.{field}"), &why))?;
        crate::prefs::save_graphics(&prefs);
        ctx.reload_prefs = true;
    }
    Ok(json!({ "graphics": crate::prefs::graphics_json(&prefs), "shown_in_app": ctx.serving }).into())
}

fn activate(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    let od = ctx.doc(doc_arg)?;
    let tab = doc::find_tab(od.doc(), a.str("tab")?).map_err(|e| e.with_param("tab"))?;
    let mut out = json!({ "tab": tab_json(od, tab) });
    out["shown_in_app"] = json!(show(ctx, doc_arg, UiRequest::ActivateTab(tab))?);
    Ok(out.into())
}

fn manager(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    let search = a.opt_str("search")?.unwrap_or("").to_string();
    let kinds: Vec<String> = match a.opt_array("kinds")? {
        None => Vec::new(),
        Some(l) => l
            .iter()
            .enumerate()
            .map(|(i, v)| v.as_str().filter(|k| crate::window::TAB_KINDS.contains(k)).map(str::to_string).ok_or_else(|| OpError::param(&format!("kinds[{i}]"), &format!("one of {}", crate::window::TAB_KINDS.join(", ")))))
            .collect::<Result<_, _>>()?,
    };
    let sort = match a.opt_str("sort")?.unwrap_or("tab_order") {
        "tab_order" => TabSort::TabOrder,
        "name_asc" => TabSort::NameAsc,
        "name_desc" => TabSort::NameDesc,
        "type" => TabSort::Type,
        _ => return Err(OpError::param("sort", "tab_order, name_asc, name_desc or type")),
    };
    let od = ctx.doc(doc_arg)?;
    let kind_refs: Vec<&str> = kinds.iter().map(String::as_str).collect();
    let rows: Vec<Value> = tab_manager_rows(od.doc(), &search, &kind_refs, sort, &tab_kind)
        .into_iter()
        .map(|(id, name, kind)| json!({ "id": id.to_string(), "name": name, "kind": kind, "folder": od.doc().is_folder(id) }))
        .collect();
    let activate = match a.opt_str("activate")? {
        Some(t) => Some(doc::find_tab(od.doc(), t).map_err(|e| e.with_param("activate"))?),
        None => None,
    };
    let open = a.bool_or("open", true)?;
    let mut shown = show(ctx, doc_arg, UiRequest::TabManager(open.then(|| search.clone())))?;
    if let Some(t) = activate {
        shown = show(ctx, doc_arg, UiRequest::ActivateTab(t))?;
    }
    Ok(json!({ "rows": rows, "open": open, "shown_in_app": shown }).into())
}

// ------------------------------------------------------------ assembly / context

fn instance_source(ctx: &mut Ctx, a: &Args) -> Result<InstanceSource, OpError> {
    let (od, tab) = super::assembly::target(ctx, a)?;
    let asm = super::assembly::assembly(od, tab)?;
    let id = super::assembly::find_instance(asm, a.str("instance")?, "instance")?;
    Ok(asm.instance(id).expect("found").source.clone())
}

fn switch_to_tab(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    let source = instance_source(ctx, a)?;
    if source.external_key().is_some() {
        return Err(OpError::param("instance", "it comes from another document: assembly_open_linked_document"));
    }
    let tab = match &source {
        InstanceSource::Assembly { assembly, .. } => Some(*assembly),
        other => other.studio(),
    }
    .ok_or_else(|| OpError::param("instance", "it comes from no tab of this document (standard content)"))?;
    let od = ctx.doc(doc_arg)?;
    let mut out = json!({ "target": tab_json(od, tab) });
    if let InstanceSource::Part { body, config, .. } = &source {
        let name = od.regenerate(tab, Some(config)).ok().and_then(|r| r.display.bodies.iter().find(|b| b.id == *body).map(|b| b.name.clone()));
        out["part"] = json!({ "id": crate::refs::body_id(body), "name": name });
        out["configuration"] = json!(config.0);
    }
    out["shown_in_app"] = json!(show(ctx, doc_arg, UiRequest::ActivateTab(tab))?);
    Ok(out.into())
}

fn open_linked_document(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    let source = instance_source(ctx, a)?;
    let key = source.external_key().ok_or_else(|| OpError::param("instance", "it comes from this document: assembly_switch_to_tab"))?.to_string();
    let r = crate::derived::ExternalRef::parse(&key).ok_or_else(|| OpError::param("instance", "its reference can't be read"))?;
    let mut out = json!({ "document": r.path.display().to_string(), "version": r.microversion.to_string(), "tab": r.tab });
    out["shown_in_app"] = json!(show(ctx, doc_arg, UiRequest::OpenDocument { path: r.path, version: Some(r.microversion), external_key: Some(key) })?);
    Ok(out.into())
}

fn go_to_assembly(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    let od = ctx.doc(doc_arg)?;
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("tab")?, "Part Studio", |c| matches!(c, TabContent::PartStudio(_)))?;
    let ps = doc::studio(od.doc(), tab)?;
    let c = ps.active_context.and_then(|id| ps.contexts.iter().find(|c| c.id == id)).ok_or_else(|| OpError::param("tab", "no Assembly context is shown (assembly_edit_in_context, context_show)"))?;
    let asm = c.assembly;
    let mut out = json!({ "target": tab_json(od, asm), "context": c.name });
    out["shown_in_app"] = json!(show(ctx, doc_arg, UiRequest::ActivateTab(asm))?);
    Ok(out.into())
}

// ------------------------------------------------------------ history

/// A history point by version name / id, microversion id, or "workspace"
/// (None).
fn history_point(od: &OpenDoc, key: &str, param: &str) -> Result<Option<MicroversionId>, OpError> {
    if key == "workspace" {
        return Ok(None);
    }
    let versions = od.session.store.list_versions().map_err(|e| OpError::new("io", e.to_string()))?;
    match versions.iter().find(|v| v.name == key || v.id.to_string() == key) {
        Some(v) => Ok(Some(v.microversion)),
        None => key.parse().map(Some).map_err(|_| OpError::param(param, "a version name or id, a microversion id (history_list), or \"workspace\"")),
    }
}

/// How the window names a history point: "V2", or "<branch> :: <change>".
fn point_label(od: &OpenDoc, mv: Option<MicroversionId>) -> String {
    let branch = od.session.store.list_branches().ok().and_then(|bs| bs.into_iter().find(|b| b.id == od.session.branch)).map(|b| b.name).unwrap_or_else(|| "Main".into());
    let Some(mv) = mv else { return branch };
    if let Some(v) = od.session.store.list_versions().ok().and_then(|vs| vs.into_iter().find(|v| v.microversion == mv)) {
        return v.name;
    }
    let desc = od.session.store.history(od.session.branch).ok().and_then(|h| h.into_iter().find(|m| m.id == mv).map(|m| m.description));
    match desc {
        Some(d) => format!("{branch} :: {d}"),
        None => mv.to_string(),
    }
}

fn read_at(od: &OpenDoc, mv: Option<MicroversionId>) -> Result<Document, OpError> {
    let store = &od.session.store;
    let mv = match mv {
        Some(m) => m,
        None => store.head(od.session.branch).map_err(|e| OpError::new("io", e.to_string()))?,
    };
    Document::from_blobs(&store.read_all(mv).map_err(|e| OpError::new("io", e.to_string()))?).map_err(|e| OpError::new("io", e.to_string()))
}

/// A document state's tabs, and its Part Studios' features (with their
/// status) and parts.
fn summary(d: &Document) -> Value {
    let mut regen = noshape_model::regen::Regenerator::new();
    let tabs: Vec<Value> = d
        .ordered_tabs()
        .into_iter()
        .map(|t| {
            let mut v = json!({ "id": t.id.to_string(), "name": t.name, "kind": tab_kind(&t.content) });
            if let TabContent::PartStudio(ps) = &t.content {
                let r = regen.regenerate(ps, &Default::default(), &d.settings.units, &Default::default());
                v["features"] = json!(ps.features.iter().map(|f| json!({ "id": f.id.to_string(), "name": f.name, "type": f.type_id, "status": r.display.feature_state(f.id).map(|s| format!("{:?}", s.status)) })).collect::<Vec<_>>());
                v["parts"] = json!(r.display.bodies.iter().map(|b| json!({ "id": crate::refs::body_id(&b.id), "name": b.name })).collect::<Vec<_>>());
            }
            v
        })
        .collect();
    json!(tabs)
}

fn view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mv = history_point(od, a.str("to")?, "to")?;
    od.session.view_microversion(mv).map_err(|e| OpError::new("history", format!("{e:#}")))?;
    let label = point_label(od, mv);
    Ok(json!({ "viewing": od.session.viewing_version(), "label": label, "tabs": summary(od.doc()) }).into())
}

fn compare(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    if a.bool_or("stop", false)? {
        let shown = show(ctx, doc_arg, UiRequest::Compare(None))?;
        return Ok(json!({ "comparing": false, "shown_in_app": shown }).into());
    }
    let od = ctx.doc(doc_arg)?;
    let target = history_point(od, a.opt_str("target")?.ok_or_else(|| OpError::param("target", "required (or `stop`)"))?, "target")?;
    // The base: given, else what the window shows.
    let base = match a.opt_str("base")? {
        Some(b) => history_point(od, b, "base")?,
        None => od.session.viewing,
    };
    let side = |mv: Option<MicroversionId>| match mv {
        Some(m) => CompareSide::Entry(m),
        None => CompareSide::Workspace(od.session.branch),
    };
    let (bd, td) = (read_at(od, base)?, read_at(od, target)?);
    let req = CompareRequest { base: side(base), base_label: point_label(od, base), target: side(target), target_label: point_label(od, target) };
    let tabs: Vec<Value> = bd
        .ordered_tabs()
        .into_iter()
        .filter_map(|t| {
            let st = tab_compare(&bd, &td, t.id)?;
            let mut v = json!({ "id": t.id.to_string(), "name": t.name, "status": st.name() });
            if st == crate::window::TabCompare::Differs
                && let (TabContent::PartStudio(b), Some(TabContent::PartStudio(tt))) = (&t.content, td.tab(t.id).map(|x| &x.content))
            {
                let feats: Vec<Value> = noshape_model::compare::compare_features(b, tt)
                    .into_iter()
                    .filter(|f| f.status.is_some() || f.moved.is_some())
                    .map(|f| {
                        let status = match (&f.status, f.moved) {
                            (_, Some(_)) => "moved",
                            (Some(noshape_model::compare::ChangeKind::Added), _) => "added",
                            (Some(noshape_model::compare::ChangeKind::Removed), _) => "removed",
                            _ => "modified",
                        };
                        json!({ "id": f.id.to_string(), "name": f.name, "type": f.type_id, "status": status })
                    })
                    .collect();
                v["features"] = json!(feats);
            }
            Some(v)
        })
        .collect();
    let (bl, tl) = (req.base_label.clone(), req.target_label.clone());
    let shown = show(ctx, doc_arg, UiRequest::Compare(Some(req)))?;
    Ok(json!({ "comparing": true, "base": bl, "target": tl, "tabs": tabs, "shown_in_app": shown }).into())
}

fn repair_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    if a.bool_or("close", false)? {
        let shown = show(ctx, doc_arg, UiRequest::Repair(None))?;
        return Ok(json!({ "open": false, "shown_in_app": shown }).into());
    }
    let od = ctx.doc(doc_arg)?;
    let mv = history_point(od, a.opt_str("at")?.ok_or_else(|| OpError::param("at", "required (or `close`)"))?, "at")?.ok_or_else(|| OpError::param("at", "a version or change, not the workspace"))?;
    let label = point_label(od, Some(mv));
    let tabs = summary(&read_at(od, Some(mv))?);
    let shown = show(ctx, doc_arg, UiRequest::Repair(Some((mv, label.clone()))))?;
    Ok(json!({ "open": true, "label": label, "tabs": tabs, "shown_in_app": shown }).into())
}

// ------------------------------------------------------------ drawings

fn drawing_tab(ctx: &mut Ctx, a: &Args) -> Result<TabId, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    super::drawing::drawing_tab(od, a)
}

fn switch_sheet(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = super::drawing::drawing_tab(od, a)?;
    let mut d = crate::drawing::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let n = d.sheets.len();
    let cur = d.active_sheet.min(n.saturating_sub(1));
    let i = match (a.opt_str("step")?, a.get("sheet")) {
        (Some(_), Some(_)) => return Err(OpError::param("step", "give `sheet` or `step`")),
        (Some("first"), None) => 0,
        (Some("last"), None) => n - 1,
        (Some("next"), None) => (cur + 1).min(n - 1),
        (Some("previous"), None) => cur.saturating_sub(1),
        (Some(_), None) => return Err(OpError::param("step", "first, last, next or previous")),
        (None, Some(_)) => super::drawing::sheet_index(&d, a)?,
        (None, None) => return Err(OpError::param("sheet", "give `sheet` or `step`")),
    };
    if i != cur {
        d.active_sheet = i;
        super::drawing::commit(od, tab, i, d, "Switch sheet")?;
    }
    let mut out = super::drawing::reply(od, tab, i, false)?;
    out["shown_sheet"] = json!(i + 1);
    Ok(out.into())
}

fn sheets_panel(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc_arg = a.opt_str("doc")?;
    let tab = drawing_tab(ctx, a)?;
    let od = ctx.doc(doc_arg)?;
    let d = crate::drawing::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let cur = d.active_sheet.min(d.sheets.len().saturating_sub(1));
    let sheets: Vec<Value> = d
        .sheets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            json!({
                "sheet": i + 1,
                "name": s.name,
                "shown": i == cur,
                "views": s.views.iter().map(|v| json!({ "id": v.id.0, "kind": super::drawing::kind_name(&v.kind), "label": v.label, "name": v.name })).collect::<Vec<_>>(),
                "tables": s.tables.iter().map(|t| json!({ "id": t.id, "kind": super::drawing::variant_name(&t.kind) })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let open = a.bool_or("open", true)?;
    let shown = show(ctx, doc_arg, UiRequest::SheetsPanel { tab, open })?;
    Ok(json!({ "sheets": sheets, "open": open, "shown_in_app": shown }).into())
}
