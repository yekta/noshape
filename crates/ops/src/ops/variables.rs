//! Variables: a Part Studio's (the Variable table panel; Variable
//! features) or a Variable Studio's table (`crate::variable_studio`, the
//! same edits as the app's table).

use super::{doc_field, edit_studio, regen_report, s};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::variable_studio as vs;
use crate::{Ctx, OpenDoc, doc};
use noshape_model::document::{TabContent, TabId};
use noshape_model::variables::{VarKind, VariableStudio};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::FeatureId;
use noshape_model::variable_table::{self as vt, NameError};
use serde_json::{Value, json};

const KINDS: [&str; 4] = ["LENGTH", "ANGLE", "NUMBER", "ANY"];

fn var_tab() -> crate::registry::Field {
    opt("tab", json!({ "type": "string" }), "Part Studio or Variable Studio tab, by name or id. Default: the first Part Studio.")
}

/// Where the variables are: a Part Studio's Variable features, or a
/// Variable Studio's table.
#[derive(Clone, Copy)]
enum Table {
    Studio(TabId),
    Vs(TabId),
}

fn table<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<(&'a mut OpenDoc, Table), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    if let Some(k) = a.opt_str("tab")? {
        let id = doc::find_tab(od.doc(), k).map_err(|e| e.with_param("tab"))?;
        if matches!(od.doc().tab(id).map(|t| &t.content), Some(TabContent::VariableStudio(_))) {
            return Ok((od, Table::Vs(id)));
        }
    }
    let tab = od.tab_of_kind(a.opt_str("tab")?, "Part Studio or Variable Studio", doc::is_part_studio)?;
    Ok((od, Table::Studio(tab)))
}

fn kind_of(k: &str) -> Result<VarKind, OpError> {
    match k {
        "LENGTH" => Ok(VarKind::Length),
        "ANGLE" => Ok(VarKind::Angle),
        "NUMBER" => Ok(VarKind::Number),
        "ANY" => Ok(VarKind::Any),
        _ => Err(OpError::param("kind", "LENGTH, ANGLE, NUMBER or ANY")),
    }
}

fn vs_studio(od: &OpenDoc, tab: TabId) -> Result<&VariableStudio, OpError> {
    match od.doc().tab(tab).map(|t| &t.content) {
        Some(TabContent::VariableStudio(v)) => Ok(v),
        _ => Err(OpError::param("tab", "not a Variable Studio")),
    }
}

/// One edit of the Variable Studio's table (the app's `VariableStudioView::edit`).
fn vs_edit(od: &mut OpenDoc, tab: TabId, desc: &str, f: impl FnOnce(&mut VariableStudio)) -> Result<(), OpError> {
    od.session.edit_tab(tab, desc, |c| {
        if let TabContent::VariableStudio(v) = c {
            f(v);
        }
    })?;
    Ok(())
}

fn vs_find(s: &VariableStudio, key: &str) -> Result<usize, OpError> {
    let k = key.trim().trim_start_matches('#');
    s.variables.iter().position(|v| v.name == k).ok_or_else(|| OpError::not_found(format!("no variable `{key}`; variables: {}", s.variables.iter().map(|v| format!("#{}", v.name)).collect::<Vec<_>>().join(", "))).with_param("variable"))
}

/// A new name for the table: an identifier no other row uses.
fn vs_name(s: &VariableStudio, name: &str, except: Option<usize>) -> Result<String, OpError> {
    let n = name.trim().trim_start_matches('#');
    if !vt::is_identifier(n) {
        return Err(name_error(NameError::Invalid, n));
    }
    if s.variables.iter().enumerate().any(|(i, v)| v.name == n && Some(i) != except) {
        return Err(name_error(NameError::Duplicate, n));
    }
    Ok(n.to_string())
}

fn vs_list(od: &OpenDoc, tab: TabId) -> Result<Value, OpError> {
    let units = od.doc().settings.units;
    let s = vs_studio(od, tab)?;
    let eval = noshape_model::variables::studio_evaluation(od.doc(), tab);
    let rows: Vec<Value> = s
        .variables
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let mut j = json!({ "name": format!("#{}", v.name), "index": i, "type": v.kind.map(vs::kind_label).unwrap_or("Any"), "expression": v.expr.text, "description": v.description });
            match eval.results.get(i) {
                Some(Ok(q)) => j["value"] = json!(noshape_units::format_quantity_exact(*q, &units)),
                Some(Err(e)) => j["error"] = json!(e.to_string()),
                None => {}
            }
            j
        })
        .collect();
    Ok(json!({ "variable_studio": od.doc().tab(tab).map(|t| t.name.clone()), "variables": rows }))
}

/// Set the given fields of row `i`, one table edit each, in the order the
/// cells are typed: type, value, description.
fn vs_set_fields(od: &mut OpenDoc, tab: TabId, i: usize, a: &Args) -> Result<(), OpError> {
    let units = od.doc().settings.units;
    if let Some(k) = a.opt_str("kind")? {
        let k = kind_of(k)?;
        vs_edit(od, tab, "Change variable type", |s| vs::set_kind(s, i, k))?;
    }
    if let Some(v) = a.get("value") {
        let v = value_text(v)?;
        vs_edit(od, tab, "Edit variable", |s| vs::set_value(s, i, &v, &units))?;
    }
    if let Some(d) = a.opt_str("description")? {
        let d = d.to_string();
        vs_edit(od, tab, "Edit variable description", |s| vs::set_description(s, i, &d))?;
    }
    Ok(())
}

pub fn ops() -> Vec<Op> {
    let kind = json!({ "type": "string", "enum": KINDS });
    vec![
        Op {
            group: "variable",
            verb: "list",
            summary: "The variables of a Part Studio or a Variable Studio, with their values",
            description: "Part Studio: Variable features (#name) in feature order with kind, expression, evaluated value and description, plus the values of every variable visible at the end of the feature list (including Variable Studio and configuration variables) and the inserted Variable Studio tables (`studio_tables`). Variable Studio (`tab` names it): its table rows in order (name, type, expression, value, description).",
            fields: vec![doc_field(), var_tab(), opt("copy", json!({ "type": "boolean" }), "The table's Copy table (Part Studio: the Variables panel's; Variable Studio: its ⋯ menu's), `copied` being the text it puts on the clipboard (a header row, then name, type, value, description per variable, tab-separated). A row's Copy copies its description (`description` here).")],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "variable",
            verb: "create",
            summary: "Create a variable (#name) in a Part Studio or a Variable Studio",
            description: "Part Studio: the Variable table's add row, a Variable feature at the rollback bar. Variable Studio: the table's last row (\"Add variable\", a Length variable), then its type, value and description cells, one edit each, as typed in the table. Use it in any expression as #name.",
            fields: vec![
                doc_field(),
                var_tab(),
                req("name", s(), "Identifier: a letter or underscore, then letters, digits or underscores."),
                req("value", json!({ "type": ["string", "number"] }), "Expression (\"20 mm\", \"#a * 2\"); a bare number takes the workspace unit."),
                opt("kind", kind.clone(), "LENGTH (default), ANGLE, NUMBER or ANY."),
                opt("description", s(), "Description."),
            ],
            mutates: true,
            destructive: false,
            run: create,
        },
        Op {
            group: "variable",
            verb: "set",
            summary: "Change a variable's value, kind, name or description",
            description: "Part Studio: renaming updates every expression that uses the variable, as the panel does. Variable Studio: each given field is one edit of its table cell (\"Change variable type\", \"Edit variable\", \"Edit variable description\", \"Rename variable\").",
            fields: vec![
                doc_field(),
                var_tab(),
                opt("variable", s(), "The variable to change: its name (with or without #) or feature id. When omitted, `name` names it."),
                opt("value", json!({ "type": ["string", "number"] }), "New expression."),
                opt("kind", kind, "New kind."),
                opt("new_name", s(), "New name (renames it)."),
                opt("name", s(), "With `variable`: the new name (as `new_name`). Without it: the variable to change, as variable_create names it."),
                opt("description", s(), "New description."),
            ],
            mutates: true,
            destructive: false,
            run: set,
        },
        Op {
            group: "variable",
            verb: "delete",
            summary: "Delete a variable",
            description: "Expressions that use it fail to regenerate, as in Onshape.",
            fields: vec![doc_field(), var_tab(), req("variable", s(), "Variable name or feature id.")],
            mutates: true,
            destructive: true,
            run: delete,
        },
        Op {
            group: "variable",
            verb: "move",
            summary: "Move a variable up or down in the Variable table or a Variable Studio",
            description: "",
            fields: vec![doc_field(), var_tab(), req("variable", s(), "Variable name or feature id."), req("by", json!({ "type": "integer" }), "-1: up, 1: down.")],
            mutates: true,
            destructive: false,
            run: move_row,
        },
        Op {
            group: "variable",
            verb: "insert_studio",
            summary: "Insert a Variable Studio (or some of its variables) into a Part Studio, Assembly or Variable Studio",
            description: "The Variable table's Insert Variable Studio dialog: the studio's variables become #names in `tab`'s expressions, shown as a read-only table under its own variables. Inserting a studio again replaces its reference. `variables` inserts only those (the dialog's variable rows); a configured studio is inserted in one configuration (`configuration`: its inputs by name or id, e.g. {\"Size\": \"Large\"}; default: its defaults); `document` + `version` insert another document's studio at a named version. One edit (\"Insert Variable Studio\"). The document's first Variable Studio is inserted into every Part Studio and Assembly already (\"Insert into all Part Studios and Assemblies\", variable_studio_settings). Returns `tab`'s inserted tables.",
            fields: vec![
                doc_field(),
                target_tab(),
                req("studio", s(), "The Variable Studio tab, by name or id (in `document` when given)."),
                opt("variables", json!({ "type": "array", "items": { "type": "string" }, "minItems": 1 }), "Only these variables (names, with or without #). Default: the whole table."),
                opt("configuration", json!({ "type": ["object", "string"] }), "A configured studio's configuration: input name or id -> option name or id (List), true/false (Checkbox), an expression (Variable); or \"default\" (Reset to default) or \"last_viewed\" (Set to last viewed: the configuration the studio shows)."),
                opt("document", s(), "Another .noshape document's studio (with `version`)."),
                opt("version", s(), "With `document`: the name of one of its versions (the dialog lists named versions only)."),
            ],
            mutates: true,
            destructive: false,
            run: insert_studio,
        },
        Op {
            group: "variable",
            verb: "remove_studio",
            summary: "Remove an inserted Variable Studio from a Variable table (Remove reference)",
            description: "The inserted table's menu › Remove reference: its variables stop being available in `tab` (expressions using them then fail). Also removes an automatically inserted studio from this one tab. One edit (\"Remove reference\").",
            fields: vec![doc_field(), target_tab(), req("studio", s(), "The inserted Variable Studio, by tab name or id.")],
            mutates: true,
            destructive: true,
            run: remove_studio,
        },
        Op {
            group: "variable",
            verb: "move_studio",
            summary: "Move an inserted Variable Studio's table up or down (its menu's Move up / Move down)",
            description: "The order decides which table's variable wins when two define the same name (the later one). One edit (\"Move table up\" / \"Move table down\").",
            fields: vec![doc_field(), target_tab(), req("studio", s(), "The inserted Variable Studio, by tab name or id."), req("by", json!({ "type": "integer", "enum": [-1, 1] }), "-1: up, 1: down.")],
            mutates: true,
            destructive: false,
            run: move_studio,
        },
        Op {
            group: "variable",
            verb: "create_studio",
            summary: "Create a Variable Studio from a Part Studio's or Assembly's Variable table",
            description: "The Variable table's Create Variable Studio: a new Variable Studio tab right after `tab` (\"Variable Studio N\"), shown in `tab`'s Variable table at once (the document's first studio is inserted into all Part Studios and Assemblies; a later one only into `tab`). One edit (\"Insert tab : Variable Studio\"). Then add its variables with variable_create {\"tab\": <its name>, ...}.",
            fields: vec![doc_field(), opt("tab", s(), "Part Studio or Assembly, by name or id. Default: the first Part Studio.")],
            mutates: true,
            destructive: false,
            run: create_studio,
        },
        Op {
            group: "variable",
            verb: "studio_settings",
            summary: "A Variable Studio's \"Insert into all Part Studios and Assemblies\" checkbox",
            description: "On: the studio's variables are available in every Part Studio and Assembly of the document (except where its reference was removed). A configured studio can't be inserted into all (the checkbox is greyed). One edit (\"Insert into all Part Studios and Assemblies\" / \"Do not insert into all Part Studios and Assemblies\").",
            fields: vec![doc_field(), req("tab", s(), "The Variable Studio, by name or id."), req("insert_into_all", json!({ "type": "boolean" }), "The checkbox.")],
            mutates: true,
            destructive: false,
            run: studio_settings,
        },
    ]
}

fn target_tab() -> crate::registry::Field {
    opt("tab", s(), "The Part Studio, Assembly or Variable Studio whose Variable table gets it, by name or id. Default: the first Part Studio.")
}

/// The tab whose Variable table an insert_studio-like call edits.
fn target<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<(&'a mut OpenDoc, TabId), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = match a.opt_str("tab")? {
        Some(k) => {
            let id = doc::find_tab(od.doc(), k).map_err(|e| e.with_param("tab"))?;
            match od.doc().tab(id).map(|t| &t.content) {
                Some(TabContent::PartStudio(_) | TabContent::Assembly(_) | TabContent::VariableStudio(_)) => id,
                _ => return Err(OpError::param("tab", "not a Part Studio, Assembly or Variable Studio")),
            }
        }
        None => od.tab_of_kind(None, "Part Studio", doc::is_part_studio)?,
    };
    Ok((od, tab))
}

/// A Variable Studio tab of `d` by name or id.
fn studio_tab(d: &noshape_model::Document, key: &str) -> Result<TabId, OpError> {
    let id = doc::find_tab(d, key).map_err(|e| e.with_param("studio"))?;
    match d.tab(id).map(|t| &t.content) {
        Some(TabContent::VariableStudio(_)) => Ok(id),
        _ => Err(OpError::param("studio", "not a Variable Studio")),
    }
}

/// The Variable Studio tables `tab`'s Variable table shows, in order.
fn shown_refs(d: &noshape_model::Document, tab: TabId) -> Vec<noshape_model::variables::StudioRef> {
    match d.tab(tab).map(|t| &t.content) {
        Some(TabContent::VariableStudio(v)) => v.inserted.clone(),
        _ => noshape_model::variables::element_studios(d, tab),
    }
}

/// An inserted table's studio name (in its own document).
fn studio_name(d: &noshape_model::Document, r: &noshape_model::variables::StudioRef) -> String {
    let src = r.external.as_ref().and_then(noshape_model::variables::external_document);
    src.as_deref().unwrap_or(d).tab(r.studio).map(|t| t.name.clone()).unwrap_or_else(|| "missing Variable Studio".into())
}

/// [`shown_refs`] as JSON, with the tab's name.
fn tables_json(d: &noshape_model::Document, tab: TabId) -> Value {
    use noshape_model::variables as vars;
    let list: Vec<Value> = shown_refs(d, tab)
        .iter()
        .map(|r| {
            let mut j = json!({ "studio": studio_name(d, r), "id": r.studio.to_string() });
            if r.external.is_none() && vars::is_auto_referenced(d, tab, r.studio) {
                j["automatic"] = json!(true);
            }
            if let Some(v) = &r.variables {
                j["variables"] = json!(v.iter().map(|n| format!("#{n}")).collect::<Vec<_>>());
            }
            if !r.config.is_empty() {
                j["configuration"] = json!(r.config.0);
            }
            if let Some(e) = &r.external {
                j["document"] = json!(e.path);
                j["version"] = json!(e.version);
            }
            j
        })
        .collect();
    json!({ "tab": d.tab(tab).map(|t| t.name.clone()), "tables": list })
}

/// Configuration values for a studio's inputs, by input / option name or id.
fn studio_config(inputs: &[noshape_model::configuration::ConfigInput], values: &serde_json::Map<String, Value>) -> Result<noshape_model::configuration::ConfigValues, OpError> {
    use noshape_model::configuration::ConfigInputKind;
    let mut out = noshape_model::configuration::ConfigValues::default();
    let names = || inputs.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join(", ");
    for (key, v) in values {
        let field = format!("configuration.{key}");
        let input = inputs
            .iter()
            .find(|i| i.id == *key)
            .or_else(|| inputs.iter().find(|i| i.name.eq_ignore_ascii_case(key)))
            .ok_or_else(|| OpError::not_found(if inputs.is_empty() { "the studio has no configuration".into() } else { format!("no configuration input `{key}`; inputs: {}", names()) }).with_param(&field))?;
        let value = match (&input.kind, v) {
            (ConfigInputKind::List { options, .. }, Value::String(o)) => options
                .iter()
                .find(|x| x.id == *o)
                .or_else(|| options.iter().find(|x| x.name.eq_ignore_ascii_case(o)))
                .map(|x| x.id.clone())
                .ok_or_else(|| OpError::not_found(format!("no configuration `{o}`; configurations: {}", options.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(", "))).with_param(&field))?,
            (ConfigInputKind::Checkbox { .. }, Value::Bool(b)) => b.to_string(),
            (ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. }, Value::String(t)) => t.clone(),
            (ConfigInputKind::Variable { .. }, Value::Number(n)) => n.to_string(),
            (ConfigInputKind::List { .. }, _) => return Err(OpError::param(&field, "a configuration name or id")),
            (ConfigInputKind::Checkbox { .. }, _) => return Err(OpError::param(&field, "true or false")),
            _ => return Err(OpError::param(&field, "a string")),
        };
        out.0.insert(input.id.clone(), value);
    }
    Ok(out)
}

fn insert_studio(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::variables as vars;
    let (od, tab) = target(ctx, a)?;
    // The studio: this document's, or another document's at a version.
    let (src, external): (std::sync::Arc<noshape_model::Document>, Option<vars::ExternalStudio>) = match (a.opt_str("document")?, a.opt_str("version")?) {
        (Some(path), Some(version)) => {
            let path = std::path::Path::new(path);
            let path = path.canonicalize().map_err(|e| OpError::param("document", &format!("{}: {e}", path.display())))?;
            let info = crate::derived::external_doc_info(&path).map_err(|e| OpError::param("document", &format!("can't read it: {e}")))?;
            let named: Vec<&crate::derived::VersionChoice> = info.versions.iter().filter(|v| v.label != "Current state").collect();
            let v = named.iter().find(|v| v.label == version).ok_or_else(|| OpError::not_found(if named.is_empty() { "the document has no versions: create one in it first (history_create_version)".into() } else { format!("no version `{version}`; versions: {}", named.iter().map(|v| v.label.as_str()).collect::<Vec<_>>().join(", ")) }).with_param("version"))?;
            let snapshot = crate::derived::external_snapshot(&path, v.microversion).map_err(|e| OpError::param("version", &format!("can't read it: {e}")))?;
            (snapshot, Some(vars::ExternalStudio { path, microversion: v.microversion.to_string(), version: v.label.clone() }))
        }
        (Some(_), None) => return Err(OpError::param("version", "required with `document`: one of its named versions")),
        (None, Some(_)) => return Err(OpError::param("document", "required with `version`")),
        (None, None) => (std::sync::Arc::new(od.doc().clone()), None),
    };
    let studio = studio_tab(&src, a.str("studio")?)?;
    if external.is_none() && studio == tab {
        return Err(OpError::param("studio", "a Variable Studio can't insert itself"));
    }
    let Some(TabContent::VariableStudio(def)) = src.tab(studio).map(|t| &t.content) else { unreachable!("checked") };
    let variables = match a.opt_array("variables")? {
        Some(list) => Some(
            list.iter()
                .enumerate()
                .map(|(i, v)| {
                    let n = v.as_str().ok_or_else(|| OpError::param(&format!("variables[{i}]"), "a variable name"))?.trim().trim_start_matches('#');
                    def.variables.iter().any(|x| x.name == n).then(|| n.to_string()).ok_or_else(|| OpError::not_found(format!("no variable `{n}` in the studio; variables: {}", def.variables.iter().map(|x| format!("#{}", x.name)).collect::<Vec<_>>().join(", "))).with_param(&format!("variables[{i}]")))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        None => None,
    };
    let config = match a.get("configuration") {
        None | Some(Value::Null) => Default::default(),
        Some(Value::String(k)) if k == "default" => Default::default(),
        Some(Value::String(k)) if k == "last_viewed" => def.configuration.active.clone(),
        Some(Value::Object(values)) => studio_config(&def.configuration.inputs, values)?,
        Some(_) => return Err(OpError::param("configuration", "an object of input values, \"default\" or \"last_viewed\"")),
    };
    let r = vars::StudioRef { studio, variables, external, config };
    od.session.edit_tab(tab, vs::reference_description(true), |c| vs::set_reference(c, studio, Some(r)))?;
    Ok(tables_json(od.doc(), tab).into())
}

fn remove_studio(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let studio = inserted(od.doc(), tab, a.str("studio")?)?;
    od.session.edit_tab(tab, vs::reference_description(false), |c| vs::set_reference(c, studio, None))?;
    Ok(tables_json(od.doc(), tab).into())
}

/// A table `tab`'s Variable table shows, by its studio's name or id.
fn inserted(d: &noshape_model::Document, tab: TabId, key: &str) -> Result<TabId, OpError> {
    let shown = shown_refs(d, tab);
    let k = key.trim();
    let name = |r: &noshape_model::variables::StudioRef| studio_name(d, r);
    shown
        .iter()
        .find(|r| r.studio.to_string() == k)
        .or_else(|| shown.iter().find(|r| name(r).eq_ignore_ascii_case(k)))
        .map(|r| r.studio)
        .ok_or_else(|| OpError::not_found(format!("`{key}` is not inserted here; tables: {}", shown.iter().map(name).collect::<Vec<_>>().join(", "))).with_param("studio"))
}

fn move_studio(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let studio = inserted(od.doc(), tab, a.str("studio")?)?;
    let down = a.get("by").and_then(Value::as_i64).ok_or_else(|| OpError::param("by", "-1 or 1"))? > 0;
    let edit = vs::move_reference(od.doc(), tab, studio, down).ok_or_else(|| OpError::invalid(format!("the table is already the {}", if down { "last" } else { "first" })).with_param("by"))?;
    od.session.edit_tab(tab, vs::move_table_description(down), edit)?;
    Ok(tables_json(od.doc(), tab).into())
}

fn create_studio(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let element = match a.opt_str("tab")? {
        Some(k) => {
            let id = doc::find_tab(od.doc(), k).map_err(|e| e.with_param("tab"))?;
            match od.doc().tab(id).map(|t| &t.content) {
                Some(TabContent::PartStudio(_) | TabContent::Assembly(_)) => id,
                _ => return Err(OpError::param("tab", "not a Part Studio or Assembly")),
            }
        }
        None => od.tab_of_kind(None, "Part Studio", doc::is_part_studio)?,
    };
    let id = od.session.edit_document(vs::CREATE_STUDIO_DESCRIPTION, |d| vs::create_studio(d, element))?;
    let mut v = tables_json(od.doc(), element);
    v["created"] = json!({ "id": id.to_string(), "name": od.doc().tab(id).map(|t| t.name.clone()) });
    Ok(v.into())
}

fn studio_settings(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = studio_tab(od.doc(), a.str("tab")?).map_err(|e| e.with_param("tab"))?;
    let on = a.get("insert_into_all").and_then(Value::as_bool).ok_or_else(|| OpError::param("insert_into_all", "true or false"))?;
    let s = vs_studio(od, tab)?;
    if on && !s.configuration.is_empty() {
        return Err(OpError::invalid("a configured Variable Studio can't be inserted into all Part Studios and Assemblies (insert it with variable_insert_studio in one configuration)").with_param("insert_into_all"));
    }
    if s.insert_into_all != on {
        vs_edit(od, tab, vs::insert_into_all_description(on), |s| s.insert_into_all = on)?;
    }
    Ok(json!({ "studio": od.doc().tab(tab).map(|t| t.name.clone()), "insert_into_all": on }).into())
}

fn name_error(e: NameError, name: &str) -> OpError {
    let why = match e {
        NameError::Invalid => "Variable name must be a letter followed by a string of letters or numbers.".to_string(),
        NameError::Duplicate => format!("another variable is already named `{name}`"),
    };
    OpError::param("name", &why)
}

fn find(ps: &PartStudio, key: &str) -> Result<FeatureId, OpError> {
    let k = key.trim().trim_start_matches('#');
    if let Some(r) = vt::rows(ps).into_iter().find(|r| r.name == k) {
        return Ok(r.feature);
    }
    if let Some(id) = crate::refs::parse_feature_id(k)
        && vt::rows(ps).iter().any(|r| r.feature == id)
    {
        return Ok(id);
    }
    Err(OpError::not_found(format!("no variable `{key}`; variables: {}", vt::rows(ps).iter().map(|r| format!("#{}", r.name)).collect::<Vec<_>>().join(", "))).with_param("variable"))
}

fn value_text(v: &Value) -> Result<String, OpError> {
    match v {
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(s.trim().to_string()),
        _ => Err(OpError::param("value", "an expression")),
    }
}

/// A Variable table edit: the panel's `edit_config` (one microversion,
/// then visibility conditions that name deleted inputs are repaired).
fn edit_table<R>(od: &mut OpenDoc, tab: TabId, desc: &str, f: impl FnOnce(&mut PartStudio) -> Result<R, OpError>) -> Result<(R, Vec<String>), OpError> {
    edit_studio(od, tab, desc, |ps| {
        let r = f(ps)?;
        let repaired = ps.configuration.repair_visibility();
        Ok((r, repaired))
    })
}

fn reply(od: &mut OpenDoc, tab: TabId, repaired: Vec<String>) -> Result<OpOutput, OpError> {
    let mut v = list_value(od, tab)?;
    v["regeneration"] = regen_report(od, tab)?;
    if !repaired.is_empty() {
        v["info"] = json!(format!("Visibility conditions for {} were updated", repaired.join(", ")));
    }
    Ok(v.into())
}

fn list_value(od: &mut OpenDoc, tab: TabId) -> Result<Value, OpError> {
    let units = od.doc().settings.units;
    let r = od.regenerate(tab, None)?;
    let ps = doc::studio(od.doc(), tab)?;
    let shown = |q: &noshape_units::Quantity| noshape_units::format_quantity_exact(*q, &units);
    let rows: Vec<Value> = vt::rows(ps)
        .into_iter()
        .map(|row| {
            let mut v = json!({ "name": format!("#{}", row.name), "feature": row.feature.to_string(), "kind": row.kind, "expression": row.value, "description": row.description });
            if let Some(q) = r.context.variables.get(&row.name) {
                v["value"] = json!(shown(q));
            }
            v
        })
        .collect();
    let all: serde_json::Map<String, Value> = r.context.variables.iter().map(|(k, q)| (format!("#{k}"), json!(shown(q)))).collect();
    let mut v = json!({ "variables": rows, "visible_at_end": all });
    // The Variable Studio tables shown under them (variable_insert_studio).
    let tables = tables_json(od.doc(), tab)["tables"].clone();
    if tables.as_array().is_some_and(|t| !t.is_empty()) {
        v["studio_tables"] = tables;
    }
    Ok(v)
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    match table(ctx, a)? {
        (od, Table::Studio(tab)) => {
            let mut v = list_value(od, tab)?;
            if a.bool_or("copy", false)? {
                v["copied"] = json!(crate::variable_studio::part_studio_table_text(crate::doc::studio(od.doc(), tab)?));
            }
            Ok(v.into())
        }
        (od, Table::Vs(tab)) => {
            let mut v = vs_list(od, tab)?;
            if a.bool_or("copy", false)? {
                v["copied"] = json!(vs::studio_table_text(vs_studio(od, tab)?));
            }
            Ok(v.into())
        }
    }
}

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = match table(ctx, a)? {
        (od, Table::Vs(tab)) => {
            let name = vs_name(vs_studio(od, tab)?, a.str("name")?, None)?;
            if let Some(k) = a.opt_str("kind")? {
                kind_of(k)?;
            }
            value_text(a.get("value").expect("required"))?;
            let units = od.doc().settings.units;
            vs_edit(od, tab, "Add variable", |s| vs::add_variable(s, &name, &units))?;
            let i = vs_studio(od, tab)?.variables.len() - 1;
            vs_set_fields(od, tab, i, a)?;
            return Ok(vs_list(od, tab)?.into());
        }
        (od, Table::Studio(tab)) => (od, tab),
    };
    let units = od.doc().settings.units;
    let name = a.str("name")?.to_string();
    let kind = a.opt_str("kind")?.unwrap_or("LENGTH").to_string();
    if !KINDS.contains(&kind.as_str()) {
        return Err(OpError::param("kind", "LENGTH, ANGLE, NUMBER or ANY"));
    }
    let value = value_text(a.get("value").expect("required"))?;
    let description = a.opt_str("description")?.unwrap_or("").to_string();
    let (_, repaired) = edit_table(od, tab, "Create variable", |ps| vt::create(ps, &name, &kind, &value, &description, None, &units).map_err(|e| name_error(e, &name)))?;
    reply(od, tab, repaired)
}

/// variable_set's variable and new name: `variable` + `new_name` (or
/// `name`), or `name` alone as the variable (what agents try first).
fn set_target<'a>(a: &'a Args) -> Result<(&'a str, Option<&'a str>), OpError> {
    match (a.opt_str("variable")?, a.opt_str("name")?, a.opt_str("new_name")?) {
        (Some(_), Some(_), Some(_)) => Err(OpError::param("new_name", "give the new name once: `new_name` or `name`, not both")),
        (Some(v), n, nn) => Ok((v, nn.or(n))),
        (None, Some(n), nn) => Ok((n, nn)),
        (None, None, _) => Err(OpError::param("variable", "required: the variable to change (its name or feature id)")),
    }
}

fn set(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (var, rename) = set_target(a)?;
    let (od, tab) = match table(ctx, a)? {
        (od, Table::Vs(tab)) => {
            let i = vs_find(vs_studio(od, tab)?, var)?;
            let new_name = match rename {
                Some(n) => Some(vs_name(vs_studio(od, tab)?, n, Some(i))?),
                None => None,
            };
            if let Some(k) = a.opt_str("kind")? {
                kind_of(k)?;
            }
            vs_set_fields(od, tab, i, a)?;
            if let Some(n) = new_name {
                vs_edit(od, tab, "Rename variable", |s| vs::rename_variable(s, i, &n))?;
            }
            return Ok(vs_list(od, tab)?.into());
        }
        (od, Table::Studio(tab)) => (od, tab),
    };
    let units = od.doc().settings.units;
    let id = find(doc::studio(od.doc(), tab)?, var)?;
    let mut all = Vec::new();
    if let Some(k) = a.opt_str("kind")? {
        if !KINDS.contains(&k) {
            return Err(OpError::param("kind", "LENGTH, ANGLE, NUMBER or ANY"));
        }
        let k = k.to_string();
        all.extend(edit_table(od, tab, "Edit variable", |ps| {
            vt::set_kind(ps, id, &k, &units);
            Ok(())
        })?.1);
    }
    if let Some(v) = a.get("value") {
        let v = value_text(v)?;
        all.extend(edit_table(od, tab, "Edit variable", |ps| {
            vt::set_value(ps, id, &v, &units);
            Ok(())
        })?.1);
    }
    if let Some(d) = a.opt_str("description")? {
        let d = d.to_string();
        all.extend(edit_table(od, tab, "Edit variable", |ps| {
            vt::set_description(ps, id, &d);
            Ok(())
        })?.1);
    }
    if let Some(n) = rename {
        let n = n.to_string();
        all.extend(edit_table(od, tab, "Rename variable", |ps| vt::rename(ps, id, &n).map_err(|e| name_error(e, &n)))?.1);
    }
    reply(od, tab, all)
}

fn delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = match table(ctx, a)? {
        (od, Table::Vs(tab)) => {
            let i = vs_find(vs_studio(od, tab)?, a.str("variable")?)?;
            vs_edit(od, tab, "Delete variable", |s| vs::delete_variable(s, i))?;
            return Ok(vs_list(od, tab)?.into());
        }
        (od, Table::Studio(tab)) => (od, tab),
    };
    let id = find(doc::studio(od.doc(), tab)?, a.str("variable")?)?;
    let (_, repaired) = edit_table(od, tab, "Delete variable", |ps| {
        ps.remove(id);
        Ok(())
    })?;
    reply(od, tab, repaired)
}

fn move_row(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = match table(ctx, a)? {
        (od, Table::Vs(tab)) => {
            let i = vs_find(vs_studio(od, tab)?, a.str("variable")?)?;
            let down = a.get("by").and_then(Value::as_i64).ok_or_else(|| OpError::param("by", "-1 or 1"))? > 0;
            vs_edit(od, tab, vs::move_description(down), |s| vs::move_variable(s, i, down))?;
            return Ok(vs_list(od, tab)?.into());
        }
        (od, Table::Studio(tab)) => (od, tab),
    };
    let id = find(doc::studio(od.doc(), tab)?, a.str("variable")?)?;
    let by = a.get("by").and_then(Value::as_i64).ok_or_else(|| OpError::param("by", "-1 or 1"))?.clamp(-1, 1) as i32;
    let (_, repaired) = edit_table(od, tab, "Move variable", |ps| {
        vt::move_row(ps, id, by);
        Ok(())
    })?;
    reply(od, tab, repaired)
}
