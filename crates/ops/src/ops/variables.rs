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
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("tab")?, "Part Studio or Variable Studio", doc::is_part_studio)?;
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
            description: "Part Studio: Variable features (#name) in feature order with kind, expression, evaluated value and description, plus the values of every variable visible at the end of the feature list (including Variable Studio and configuration variables). Variable Studio (`tab` names it): its table rows in order (name, type, expression, value, description).",
            fields: vec![doc_field(), var_tab(), opt("copy", json!({ "type": "boolean" }), "Part Studio: the Variables panel's Copy table, `copied` being the text it puts on the clipboard (a header row, then name, type, value, description per variable, tab-separated). A row's Copy copies its description (`description` here).")],
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
                req("variable", s(), "Variable name (with or without #) or its feature id."),
                opt("value", json!({ "type": ["string", "number"] }), "New expression."),
                opt("kind", kind, "New kind."),
                opt("name", s(), "New name."),
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
    ]
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
    Ok(json!({ "variables": rows, "visible_at_end": all }))
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
        (od, Table::Vs(tab)) => Ok(vs_list(od, tab)?.into()),
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

fn set(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = match table(ctx, a)? {
        (od, Table::Vs(tab)) => {
            let i = vs_find(vs_studio(od, tab)?, a.str("variable")?)?;
            let new_name = match a.opt_str("name")? {
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
    let id = find(doc::studio(od.doc(), tab)?, a.str("variable")?)?;
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
    if let Some(n) = a.opt_str("name")? {
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
