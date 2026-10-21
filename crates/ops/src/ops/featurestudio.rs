//! Feature Studios: read and edit the FeatureScript source, commit it so
//! Part Studios use its custom features (`feature_types` / `feature_add`).

use super::s;
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, doc};
use noshape_model::custom::Severity;
use noshape_model::document::{TabContent, TabId};
use serde_json::{Value, json};

fn is_studio(c: &TabContent) -> bool {
    matches!(c, TabContent::FeatureStudio(_))
}

fn tab_field() -> crate::registry::Field {
    opt("tab", s(), "Feature Studio tab (name or id); default: the first.")
}

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "featurestudio",
            verb: "get",
            summary: "A Feature Studio's FeatureScript source, its compile notices and the features it defines",
            description: "The text the editor shows (the saved draft, else the committed source), whether it has uncommitted changes, the notices (errors, warnings, info with line and column) and the custom features it defines. Part Studios use the committed source.",
            fields: vec![super::doc_field(), tab_field()],
            mutates: false,
            destructive: false,
            run: get,
        },
        Op {
            group: "featurestudio",
            verb: "edit",
            summary: "Replace a Feature Studio's source (saved, not yet committed)",
            description: "Typing in the Feature Studio: the text is saved as a draft (\"Edit Feature Studio\"); Part Studios keep the committed source until featurestudio_commit. Returns the compile notices.",
            fields: vec![super::doc_field(), tab_field(), req("source", s(), "The whole FeatureScript source.")],
            mutates: true,
            destructive: true,
            run: edit,
        },
        Op {
            group: "featurestudio",
            verb: "commit",
            summary: "Commit a Feature Studio (or all) so Part Studios use its features",
            description: "Commit (Ctrl+S) / Commit all (`all`): the draft becomes the source Part Studios use; one edit per Feature Studio (\"Commit Feature Studio\"). Returns the custom feature types now available to feature_add.",
            fields: vec![super::doc_field(), tab_field(), opt("all", json!({ "type": "boolean" }), "Commit every Feature Studio with uncommitted changes.")],
            mutates: true,
            destructive: false,
            run: commit,
        },
    ]
}

fn target<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<(&'a mut OpenDoc, TabId), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("tab")?, "Feature Studio", is_studio)?;
    Ok((od, tab))
}

fn studio(od: &OpenDoc, tab: TabId) -> &noshape_model::custom::FeatureStudio {
    match od.doc().tab(tab).map(|t| &t.content) {
        Some(TabContent::FeatureStudio(f)) => f,
        _ => unreachable!("checked by target"),
    }
}

/// The editor's notices and the features a source defines (as the editor
/// compiles it).
fn compiled(tab: TabId, source: &str) -> (Vec<Value>, Vec<Value>) {
    let c = noshape_model::custom::compile(tab, source);
    let notices = c
        .diagnostics
        .iter()
        .map(|d| {
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            };
            json!({ "severity": sev, "line": d.line, "column": d.column, "message": d.message })
        })
        .collect();
    let features = c.features.iter().map(|f| json!({ "name": f.name, "display_name": f.display_name, "type": f.type_id, "description": f.description, "params": f.params.len() })).collect();
    (notices, features)
}

fn describe(od: &OpenDoc, tab: TabId) -> Value {
    let f = studio(od, tab);
    let (notices, features) = compiled(tab, f.text());
    let name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    json!({ "tab": name, "source": f.text(), "uncommitted": f.has_uncommitted(), "notices": notices, "features": features })
}

fn get(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    Ok(describe(od, tab).into())
}

fn edit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let src = a.str("source")?.to_string();
    if studio(od, tab).text() != src {
        od.session.edit_tab(tab, "Edit Feature Studio", |c| {
            if let TabContent::FeatureStudio(f) = c {
                f.save(src);
            }
        })?;
    }
    Ok(describe(od, tab).into())
}

fn commit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let all = a.bool_or("all", false)?;
    let (od, tab) = target(ctx, a)?;
    let tabs: Vec<TabId> = if all { od.doc().tabs.iter().filter(|t| matches!(&t.content, TabContent::FeatureStudio(f) if f.has_uncommitted())).map(|t| t.id).collect() } else { std::iter::once(tab).filter(|t| studio(od, *t).has_uncommitted()).collect() };
    for t in &tabs {
        od.session.edit_tab(*t, "Commit Feature Studio", |c| {
            if let TabContent::FeatureStudio(f) = c {
                f.commit();
            }
        })?;
    }
    let types: Vec<Value> = noshape_model::custom::DocumentScripts::from_document(od.doc()).feature_types().iter().map(|t| json!({ "type": t.id, "name": t.name })).collect();
    Ok(json!({ "committed": tabs.len(), "feature_types": types }).into())
}
