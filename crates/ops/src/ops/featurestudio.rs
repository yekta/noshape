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
            verb: "std",
            summary: "The FeatureScript standard library Noshape ships (Help › FeatureScript documentation): its modules, or one module's source",
            description: "Without `module`: `version` and the `modules` by name (\"geometry.fs\", \"evaluate.fs\", …). With one: its `source`, as the read-only std document the app opens shows it.",
            fields: vec![opt("module", s(), "A module's file name (\"common.fs\").")],
            mutates: false,
            destructive: false,
            run: std_library,
        },
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
        Op {
            group: "featurestudio",
            verb: "monitor",
            summary: "The FeatureScript notices panel's Monitor for a Part Studio: notices and println output",
            description: "Regenerates the Part Studio and returns what its FeatureScript notices panel's Monitor shows: the compile warnings of the Feature Studios its custom features come from and each custom feature's runtime notices (message, severity, `line` / `column`, stack of `at` line:col and `in` module (function), ending with the Part Studio and feature), in Onshape's order, then the text the features printed (`println`).",
            fields: vec![super::doc_field(), opt("part_studio", s(), "Part Studio tab (name or id); default: the first.")],
            mutates: false,
            destructive: false,
            run: monitor,
        },
        Op {
            group: "featurestudio",
            verb: "format",
            summary: "Format Studio: re-indent a Feature Studio's source",
            description: "The ⋯ menu's Format Studio: every line re-indented by its brace depth (4 spaces), line ends trimmed; saved as a draft like typing (\"Edit Feature Studio\") when it changes. Returns the source and notices (`changed`: whether it did).",
            fields: vec![super::doc_field(), tab_field()],
            mutates: true,
            destructive: false,
            run: format,
        },
        Op {
            group: "featurestudio",
            verb: "import",
            summary: "Import: add an import(...) line for another tab (Part Studio, Feature Studio, Variable Studio, file) to a Feature Studio",
            description: "The Import panel: the `import(path : …, version : …);` line for a tab of this document (its workspace, or a `version` of it) or of another `document` (at a version: its latest when none is given), after the source's last import (else after the FeatureScript header); saved as a draft. Returns the `line`, the source and notices.",
            fields: vec![
                super::doc_field(),
                tab_field(),
                req("import", s(), "The tab to import, by name or id: a Part Studio, Feature Studio, Variable Studio or file (not the Feature Studio itself)."),
                opt("version", s(), "A version by name: of this document (default: its workspace), or of `document` (default: its latest)."),
                opt("document", s(), "Other documents: another document's path."),
            ],
            mutates: true,
            destructive: false,
            run: import,
        },
        Op {
            group: "featurestudio",
            verb: "profile",
            summary: "Monitor ▾ › Profile: time a Part Studio's full regeneration per Feature Studio line",
            description: "Regenerates the Part Studio from scratch with FeatureScript line timing on, as Profile does: `total_ms` (the editor's badge, \"288ms to fully regenerate …\") and for each Feature Studio the lines that ran with their time (`label`: the bar's \"<1ms\" / \"15ms\").",
            fields: vec![super::doc_field(), opt("part_studio", s(), "Part Studio tab (name or id); default: the first.")],
            mutates: false,
            destructive: false,
            run: profile,
        },
    ]
}

fn monitor(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("part_studio")?, "Part Studio", |c| matches!(c, TabContent::PartStudio(_))).map_err(|e| e.with_param("part_studio"))?;
    // The Monitor shows the last run: regenerate so this session has one.
    od.regenerate(tab, None)?;
    let (entries, output) = noshape_model::custom::run::monitor(od.doc(), tab);
    let name = od.doc().tab(tab).map(|t| t.name.clone());
    Ok(json!({ "part_studio": name, "notices": entries.iter().map(super::monitor_entry_json).collect::<Vec<_>>(), "output": output }).into())
}

fn format(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let src = studio(od, tab).text().to_string();
    let formatted = noshape_model::custom::source::format_source(&src);
    let changed = formatted != src;
    if changed {
        save(od, tab, formatted)?;
    }
    let mut v = describe(od, tab);
    v["changed"] = json!(changed);
    Ok(v.into())
}

fn save(od: &mut OpenDoc, tab: TabId, src: String) -> Result<(), OpError> {
    od.session.edit_tab(tab, "Edit Feature Studio", |c| {
        if let TabContent::FeatureStudio(f) = c {
            f.save(src);
        }
    })?;
    Ok(())
}

fn import(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let key = a.str("import")?;
    let version = a.opt_str("version")?;
    let mv = |m: noshape_store::MicroversionId| m.as_uuid().simple().to_string();
    let importable = |c: &TabContent| matches!(c, TabContent::PartStudio(_) | TabContent::FeatureStudio(_) | TabContent::VariableStudio(_) | TabContent::File(_));
    let kinds = "a Part Studio, Feature Studio, Variable Studio or file";
    let line = match a.opt_str("document")? {
        Some(document) => {
            // Another document opens at a version (its latest when none is named), never its workspace.
            let version = match version {
                Some(v) => v.to_string(),
                None => {
                    let path = std::path::Path::new(document);
                    let info = crate::derived::external_doc_info(path).map_err(|e| OpError::param("document", &format!("can't read it: {e}")))?;
                    info.versions.iter().find(|v| v.label != "Current state").map(|v| v.label.clone()).ok_or_else(|| OpError::not_found("the document has no versions: create one in it first (history_create_version)").with_param("version"))?
                }
            };
            let (path, v, snapshot) = crate::derived::named_version(document, &version)?;
            let t = doc::find_tab(&snapshot, key).map_err(|e| e.with_param("import"))?;
            let t = snapshot.tab(t).filter(|t| importable(&t.content)).ok_or_else(|| OpError::param("import", kinds))?;
            let id = crate::derived::document_id(&path).ok_or_else(|| OpError::param("document", "not a Noshape document"))?;
            format!("import(path : \"{}/{}/{}\", version : \"{}\");", mv(id), mv(v.microversion), t.id, mv(v.microversion))
        }
        None => {
            let t = doc::find_tab(od.doc(), key).map_err(|e| e.with_param("import"))?;
            if t == tab {
                return Err(OpError::param("import", "a Feature Studio doesn't import itself"));
            }
            if !od.doc().tab(t).is_some_and(|t| importable(&t.content)) {
                return Err(OpError::param("import", kinds));
            }
            match version {
                Some(v) => {
                    let path = od.session.path.clone();
                    let (_, v, snapshot) = crate::derived::named_version(&path.to_string_lossy(), v)?;
                    if snapshot.tab(t).is_none() {
                        return Err(OpError::param("import", &format!("`{key}` isn't in version {}", v.label)));
                    }
                    let id = crate::derived::document_id(&path).ok_or_else(|| OpError::param("version", "this document has no versions"))?;
                    format!("import(path : \"{}/{}/{t}\", version : \"{}\");", mv(id), mv(v.microversion), mv(v.microversion))
                }
                None => format!("import(path : \"{t}\", version : \"{}\");", mv(od.session.head().map_err(|e| OpError::new("store", e.to_string()))?)),
            }
        }
    };
    let src = noshape_model::custom::source::append_import(studio(od, tab).text(), &line);
    save(od, tab, src)?;
    let mut v = describe(od, tab);
    v["line"] = json!(line);
    Ok(v.into())
}

fn profile(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::custom::run;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("part_studio")?, "Part Studio", |c| matches!(c, TabContent::PartStudio(_))).map_err(|e| e.with_param("part_studio"))?;
    let opts = od.regen_options(tab);
    let values = od.current_config(tab)?;
    let units = od.doc().settings.units;
    let ps = doc::studio(od.doc(), tab)?;
    // From scratch, as Profile: a fresh regenerator.
    run::set_profiling(true);
    let start = std::time::Instant::now();
    let _ = noshape_model::regen::Regenerator::new().regenerate(ps, &values, &units, &opts);
    let total = start.elapsed();
    run::set_profiling(false);
    let ms = |t: std::time::Duration| t.as_secs_f64() * 1000.0;
    let label = |t: std::time::Duration| if ms(t) < 1.0 { "<1ms".to_string() } else { format!("{}ms", ms(t).round() as u64) };
    let d = od.doc();
    let mut studios: Vec<(TabId, Value)> = run::profile(d, tab)
        .into_iter()
        .map(|(fs, lines)| {
            let name = d.tab(fs).map(|t| t.name.clone()).unwrap_or_default();
            (fs, json!({ "tab": name, "lines": lines.iter().map(|(l, t)| json!({ "line": l, "ms": ms(*t), "label": label(*t) })).collect::<Vec<_>>() }))
        })
        .collect();
    let order: Vec<TabId> = d.ordered_tabs().iter().map(|t| t.id).collect();
    studios.sort_by_key(|(t, _)| order.iter().position(|o| o == t));
    let name = d.tab(tab).map(|t| t.name.clone());
    Ok(json!({ "part_studio": name, "total_ms": ms(total), "label": format!("{} to fully regenerate {}", label(total), name.clone().unwrap_or_default()), "feature_studios": studios.into_iter().map(|s| s.1).collect::<Vec<_>>() }).into())
}

fn target<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<(&'a mut OpenDoc, TabId), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = od.tab_of_kind(a.opt_str("tab")?, "Feature Studio", is_studio)?;
    Ok((od, tab))
}

fn studio(od: &OpenDoc, tab: TabId) -> &noshape_model::custom::FeatureStudio {
    match od.doc().tab(tab).map(|t| &t.content) {
        Some(TabContent::FeatureStudio(f)) => f,
        _ => unreachable!("checked by target"),
    }
}

/// The editor's notices and the features a source defines (as the editor
/// compiles it: imports of the document's other tabs resolved).
fn compiled(doc: &noshape_model::document::Document, tab: TabId, source: &str) -> (Vec<Value>, Vec<Value>) {
    let c = noshape_model::custom::compile_in_document(doc, tab, source);
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
    let (notices, features) = compiled(od.doc(), tab, f.text());
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
        save(od, tab, src)?;
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

fn std_library(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::custom::{STD_SOURCES, STD_VERSION};
    let Some(name) = a.opt_str("module")? else {
        return Ok(json!({ "version": format!("{STD_VERSION}.0"), "modules": STD_SOURCES.iter().map(|(m, _)| *m).collect::<Vec<_>>() }).into());
    };
    let (module, source) = STD_SOURCES.iter().find(|(m, _)| *m == name || m.trim_end_matches(".fs") == name).ok_or_else(|| OpError::param("module", &format!("no module `{name}` in the standard library (featurestudio_std lists them)")))?;
    Ok(json!({ "version": format!("{STD_VERSION}.0"), "module": module, "source": source }).into())
}
