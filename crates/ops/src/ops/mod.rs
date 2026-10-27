//! The operations, by group. Each module's `ops()` lists its operations.

mod analysis;
mod appearance;
mod assembly;
mod configuration;
mod document;
mod context;
mod drawing;
mod drawing_annotation;
mod drawing_tools;
mod drawing_more;
mod drawing_table;
mod drawing_view;
mod export;
mod feature;
mod featurestudio;
mod history;
mod import;
mod list;
mod meta;
mod parts;
mod query;
mod camera;
mod display;
mod render;
mod select;
mod release;
mod sheetmetal;
mod sketch;
mod sketch_edit;
mod ui;
mod variables;

use crate::doc::{self, OpenDoc};
use crate::error::OpError;
use crate::registry::{Args, Field, Op, opt};
use noshape_model::document::{TabContent, TabId};
use noshape_model::partstudio::PartStudio;
use serde_json::{Value, json};

pub fn all() -> Vec<Op> {
    let mut v = Vec::new();
    v.extend(meta::ops());
    v.extend(document::ops());
    v.extend(feature::ops());
    v.extend(sketch::ops());
    v.extend(sketch_edit::ops());
    v.extend(variables::ops());
    v.extend(configuration::ops());
    v.extend(assembly::ops());
    v.extend(context::ops());
    v.extend(drawing::ops());
    v.extend(drawing_tools::ops());
    v.extend(drawing_view::ops());
    v.extend(drawing_table::ops());
    v.extend(drawing_annotation::ops());
    v.extend(drawing_more::ops());
    v.extend(ui::ops());
    v.extend(parts::ops());
    v.extend(appearance::ops());
    v.extend(query::ops());
    v.extend(analysis::ops());
    v.extend(sheetmetal::ops());
    v.extend(release::ops());
    v.extend(render::ops());
    v.extend(camera::ops());
    v.extend(display::ops());
    v.extend(select::ops());
    v.extend(list::ops());
    v.extend(export::ops());
    v.extend(import::ops());
    v.extend(featurestudio::ops());
    v.extend(history::ops());
    v
}

// ------------------------------------------------------------ shared fields

pub(crate) fn doc_field() -> Field {
    opt("doc", json!({ "type": "string" }), "Path of the .noshape document. Default: the current document (the last one created or opened).")
}

pub(crate) fn tab_field() -> Field {
    opt("tab", json!({ "type": "string" }), "Part Studio tab, by name (\"Part Studio 1\") or id. Default: the tab the window shows if it is a Part Studio (tab_activate, an import), else the first Part Studio.")
}

pub(crate) fn s() -> Value {
    json!({ "type": "string" })
}

pub(crate) fn strings() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

// ------------------------------------------------------------ helpers

/// The document and Part Studio tab a call names.
pub(crate) fn studio_target<'a>(ctx: &'a mut crate::Ctx, a: &Args) -> Result<(&'a mut OpenDoc, TabId), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = od.tab_of_kind(a.opt_str("tab")?, "Part Studio", doc::is_part_studio)?;
    Ok((od, tab))
}

/// Edit a Part Studio as one microversion: `f` runs on a copy first, so a
/// failing edit changes nothing.
pub(crate) fn edit_studio<R>(od: &mut OpenDoc, tab: TabId, description: &str, f: impl FnOnce(&mut PartStudio) -> Result<R, OpError>) -> Result<R, OpError> {
    let mut copy = doc::studio(od.doc(), tab)?.clone();
    let r = f(&mut copy)?;
    od.session.edit_tab(tab, description, |c| {
        if let TabContent::PartStudio(ps) = c {
            *ps = copy;
        }
    })?;
    Ok(r)
}

/// Regenerate a Part Studio and report every feature's state, the parts
/// and the failures (what the feature list and parts list show).
pub(crate) fn regen_report(od: &mut OpenDoc, tab: TabId) -> Result<Value, OpError> {
    regen_report_with(od, tab, &[])
}

/// The whole feature list's states (feature_list).
pub(crate) fn regen_report_all(od: &mut OpenDoc, tab: TabId) -> Result<Value, OpError> {
    let all: Vec<noshape_model::reference::FeatureId> = doc::studio(od.doc(), tab)?.features.iter().map(|f| f.id).collect();
    let mut r = regen_report_with(od, tab, &all)?;
    if let Some(o) = r.as_object_mut() {
        o.remove("features_ok");
    }
    Ok(r)
}

/// [`regen_report`], listing the `keep` features whatever their state (the
/// feature a call made or edited).
pub(crate) fn regen_report_with(od: &mut OpenDoc, tab: TabId, keep: &[noshape_model::reference::FeatureId]) -> Result<Value, OpError> {
    let r = od.regenerate(tab, None)?;
    let ps = doc::studio(od.doc(), tab)?;
    let mut features = Vec::new();
    let mut ok = 0;
    let mut failed = Vec::new();
    let tab_name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    for f in &ps.features {
        let st = r.display.feature_state(f.id);
        let status = st.map(|s| format!("{:?}", s.status)).unwrap_or_else(|| "NotEvaluated".into());
        let mut v = json!({ "id": f.id.to_string(), "name": f.name, "type": f.type_id, "status": status });
        if let Some(s) = st {
            if let Some(m) = &s.message {
                v["message"] = json!(m);
            }
            if !s.faulty.is_empty() {
                v["faulty_params"] = json!(s.faulty);
            }
            if s.underdefined {
                v["underdefined"] = json!(true);
            }
            // A custom feature's FeatureScript notices (the notices panel's Monitor) say why.
            let fs = custom_output(od.doc(), &tab_name, f);
            if let Some(o) = &fs {
                v["notices"] = o["notices"].clone();
                v["output"] = o["output"].clone();
            }
            if matches!(s.status, noshape_model::display::FeatureStatus::Error) {
                let mut x = json!({ "feature": f.id.to_string(), "name": f.name, "message": s.message, "faulty_params": s.faulty });
                if let Some(o) = fs {
                    x["notices"] = o["notices"].clone();
                }
                failed.push(x);
            }
        }
        if f.suppressed {
            v["suppressed"] = json!(true);
        }
        // Only the features that need a look (the rest: `features_ok`;
        // feature_list lists all).
        if keep.contains(&f.id) || v["status"] != "Ok" || v.as_object().is_some_and(|o| o.len() > 4) {
            features.push(v);
        } else {
            ok += 1;
        }
    }
    let parts: Vec<Value> = r
        .display
        .bodies
        .iter()
        .map(|b| json!({ "id": crate::refs::body_id(&b.id), "name": b.name, "kind": format!("{:?}", b.kind), "hidden": b.hidden }))
        .collect();
    Ok(json!({ "features": features, "features_ok": ok, "parts": parts, "failed": failed, "rollback_index": ps.rollback_index() }))
}

/// A tab as navigation replies give it: id, name and kind.
pub(crate) fn tab_json(od: &OpenDoc, tab: TabId) -> Value {
    let t = od.doc().tab(tab);
    let kind = t.map(|t| match &t.content {
        TabContent::PartStudio(_) => "Part Studio",
        TabContent::Assembly(_) => "Assembly",
        TabContent::FeatureStudio(_) => "Feature Studio",
        TabContent::Drawing(_) => "Drawing",
        TabContent::VariableStudio(_) => "Variable Studio",
        TabContent::MaterialLibrary(_) => "Material Library",
        TabContent::File(_) => "File",
    });
    json!({ "id": tab.to_string(), "name": t.map(|t| t.name.clone()), "kind": kind })
}

/// Do `req` in the document `doc`'s window: its [`crate::window::WindowState`]
/// changes, and with the app open (live mode, this context serving) the
/// window does it after the call. Returns whether the app's window did.
pub(crate) fn show(ctx: &mut crate::Ctx, doc: Option<&str>, req: crate::UiRequest) -> Result<bool, OpError> {
    ctx.doc(doc)?.window.apply(&req);
    ctx.ui_requests.push(req);
    Ok(ctx.serving)
}

/// One FeatureScript notice as the Monitor shows it: the message, its
/// severity and its stack (`at` "line:col", `in` "module (function)"),
/// innermost first, ending with the Part Studio and feature.
pub(crate) fn monitor_entry_json(e: &noshape_model::custom::run::MonitorEntry) -> Value {
    let mut v = json!({
        "message": e.message,
        "severity": format!("{:?}", e.severity).to_lowercase(),
        "stack": e.rows.iter().map(|(at, inside)| if at.is_empty() { json!({ "in": inside }) } else { json!({ "at": at, "in": inside }) }).collect::<Vec<_>>(),
    });
    if let Some((_, line, column)) = e.at {
        v["line"] = json!(line);
        v["column"] = json!(column);
    }
    v
}

/// The last run of a custom feature in this session: its FeatureScript
/// notices and `println` output (`None` for other features, or before it
/// ran). Read after regenerating.
pub(crate) fn custom_output(d: &noshape_model::document::Document, tab_name: &str, f: &noshape_model::Feature) -> Option<Value> {
    if !noshape_model::custom::is_custom(&f.type_id) {
        return None;
    }
    let out = noshape_model::custom::run::output_of(f.id)?;
    if out.notices.is_empty() && out.output.is_empty() {
        return None;
    }
    let last = format!("{tab_name} ({})", out.feature_name);
    let notices: Vec<Value> = noshape_model::custom::run::notice_entries(d, &out.notices, &last).iter().map(monitor_entry_json).collect();
    Some(json!({ "notices": notices, "output": out.output }))
}

/// A feature id or name argument resolved in a studio.
/// A `configuration` argument: {input id: value} (values as given; numbers as text).
pub(crate) fn config_arg(a: &Args) -> Result<noshape_model::configuration::ConfigValues, OpError> {
    Ok(noshape_model::configuration::ConfigValues(a.object("configuration")?.map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect()).unwrap_or_default()))
}

/// `configuration` by input and option ids or names ({"Size": "Large"}),
/// resolved against source tab `src`'s inputs as config_activate does (a
/// Part Studio's); other sources take it as given (qa API-ASM-CONFIG-NAMES:
/// names were stored as keys and the default configuration used).
pub(crate) fn source_config_arg(od: &OpenDoc, src: TabId, a: &Args) -> Result<noshape_model::configuration::ConfigValues, OpError> {
    let Some(map) = a.object("configuration")? else { return Ok(Default::default()) };
    match od.doc().tab(src).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => configuration::studio_values(ps, &od.doc().settings.units, map, "configuration"),
        _ => config_arg(a),
    }
}

pub(crate) fn feature_arg(ps: &PartStudio, a: &Args, name: &str) -> Result<noshape_model::reference::FeatureId, OpError> {
    doc::find_feature(ps, a.str(name)?).map_err(|e| e.with_param(name))
}

/// Several features (ids or names).
pub(crate) fn features_arg(ps: &PartStudio, a: &Args, name: &str) -> Result<Vec<noshape_model::reference::FeatureId>, OpError> {
    a.array(name)?
        .iter()
        .enumerate()
        .map(|(i, v)| v.as_str().ok_or_else(|| OpError::param(&format!("{name}[{i}]"), "feature id or name")).and_then(|s| doc::find_feature(ps, s).map_err(|e| e.with_param(format!("{name}[{i}]")))))
        .collect()
}
