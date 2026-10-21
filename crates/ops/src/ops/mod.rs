//! The operations, by group. Each module's `ops()` lists its operations.

mod analysis;
mod appearance;
mod assembly;
mod configuration;
mod document;
mod drawing;
mod drawing_tools;
mod export;
mod feature;
mod featurestudio;
mod history;
mod import;
mod meta;
mod parts;
mod query;
mod render;
mod sketch;
mod sketch_edit;
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
    v.extend(drawing::ops());
    v.extend(drawing_tools::ops());
    v.extend(parts::ops());
    v.extend(appearance::ops());
    v.extend(query::ops());
    v.extend(analysis::ops());
    v.extend(render::ops());
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
    opt("tab", json!({ "type": "string" }), "Part Studio tab, by name (\"Part Studio 1\") or id. Default: the first Part Studio.")
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
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("tab")?, "Part Studio", doc::is_part_studio)?;
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
    let r = od.regenerate(tab, None)?;
    let ps = doc::studio(od.doc(), tab)?;
    let mut features = Vec::new();
    let mut failed = Vec::new();
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
            if matches!(s.status, noshape_model::display::FeatureStatus::Error) {
                failed.push(json!({ "feature": f.id.to_string(), "name": f.name, "message": s.message, "faulty_params": s.faulty }));
            }
        }
        if f.suppressed {
            v["suppressed"] = json!(true);
        }
        features.push(v);
    }
    let parts: Vec<Value> = r
        .display
        .bodies
        .iter()
        .map(|b| json!({ "id": crate::refs::body_id(&b.id), "name": b.name, "kind": format!("{:?}", b.kind), "hidden": b.hidden }))
        .collect();
    Ok(json!({ "features": features, "parts": parts, "failed": failed, "rollback_index": ps.rollback_index() }))
}

/// A feature id or name argument resolved in a studio.
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
