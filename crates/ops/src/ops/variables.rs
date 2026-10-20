//! Part Studio variables (the Variable table panel; Variable features).

use super::{doc_field, edit_studio, regen_report, s, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, doc};
use noshape_model::document::TabId;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::FeatureId;
use noshape_model::variable_table::{self as vt, NameError};
use serde_json::{Value, json};

const KINDS: [&str; 4] = ["LENGTH", "ANGLE", "NUMBER", "ANY"];

pub fn ops() -> Vec<Op> {
    let kind = json!({ "type": "string", "enum": KINDS });
    vec![
        Op {
            group: "variable",
            verb: "list",
            summary: "The Part Studio's variables with their values",
            description: "Variable features (#name) in feature order with kind, expression, evaluated value and description, plus the values of every variable visible at the end of the feature list (including Variable Studio and configuration variables).",
            fields: vec![doc_field(), tab_field()],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "variable",
            verb: "create",
            summary: "Create a Part Studio variable (#name)",
            description: "The Variable table's add row: a Variable feature at the rollback bar. Use it in any expression as #name.",
            fields: vec![
                doc_field(),
                tab_field(),
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
            description: "Renaming updates every expression that uses the variable, as the panel does.",
            fields: vec![
                doc_field(),
                tab_field(),
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
            fields: vec![doc_field(), tab_field(), req("variable", s(), "Variable name or feature id.")],
            mutates: true,
            destructive: true,
            run: delete,
        },
        Op {
            group: "variable",
            verb: "move",
            summary: "Move a variable up or down in the Variable table",
            description: "",
            fields: vec![doc_field(), tab_field(), req("variable", s(), "Variable name or feature id."), req("by", json!({ "type": "integer" }), "-1: up, 1: down.")],
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
    let (od, tab) = studio_target(ctx, a)?;
    Ok(list_value(od, tab)?.into())
}

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
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
    let (od, tab) = studio_target(ctx, a)?;
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
    let (od, tab) = studio_target(ctx, a)?;
    let id = find(doc::studio(od.doc(), tab)?, a.str("variable")?)?;
    let (_, repaired) = edit_table(od, tab, "Delete variable", |ps| {
        ps.remove(id);
        Ok(())
    })?;
    reply(od, tab, repaired)
}

fn move_row(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let id = find(doc::studio(od.doc(), tab)?, a.str("variable")?)?;
    let by = a.get("by").and_then(Value::as_i64).ok_or_else(|| OpError::param("by", "-1 or 1"))?.clamp(-1, 1) as i32;
    let (_, repaired) = edit_table(od, tab, "Move variable", |ps| {
        vt::move_row(ps, id, by);
        Ok(())
    })?;
    reply(od, tab, repaired)
}
