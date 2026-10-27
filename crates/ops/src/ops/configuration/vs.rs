//! The config ops on a Variable Studio tab (the studio's Configuration
//! panel, `crate::vs_config`): inputs and rows as in a Part Studio;
//! configured columns are a variable's Value or Description cell, and the
//! configuration the studio shows is part of the document.

use super::text;
use crate::error::OpError;
use crate::registry::Args;
use crate::vs_config as vsc;
use crate::{Ctx, OpenDoc, doc};
use noshape_model::configuration::{ConfigInputKind, VariableKind};
use noshape_model::document::{TabContent, TabId};
use noshape_model::variables::{VariableStudio, VsConfiguration, VsField};
use noshape_units::ExprValue;
use serde_json::{Value, json};

/// Whether `tab` names a Variable Studio.
pub(crate) fn is_vs(ctx: &mut Ctx, a: &Args) -> Result<bool, OpError> {
    let Some(t) = a.opt_str("tab")? else { return Ok(false) };
    let od = ctx.doc(a.opt_str("doc")?)?;
    Ok(doc::find_tab(od.doc(), t).ok().and_then(|id| od.doc().tab(id)).is_some_and(|x| matches!(x.content, TabContent::VariableStudio(_))))
}

fn target<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<(&'a mut OpenDoc, TabId), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = od.tab_of_kind(a.opt_str("tab")?, "Variable Studio", |c| matches!(c, TabContent::VariableStudio(_)))?;
    Ok((od, tab))
}

fn studio(od: &OpenDoc, tab: TabId) -> &VariableStudio {
    match od.doc().tab(tab).map(|t| &t.content) {
        Some(TabContent::VariableStudio(v)) => v,
        _ => unreachable!("checked by target"),
    }
}

fn cfg(od: &OpenDoc, tab: TabId) -> &VsConfiguration {
    &studio(od, tab).configuration
}

fn find_input(c: &VsConfiguration, key: &str, field: &str) -> Result<String, OpError> {
    let inputs = &c.inputs;
    inputs
        .iter()
        .find(|i| i.id == key)
        .or_else(|| inputs.iter().find(|i| i.name == key || i.name.eq_ignore_ascii_case(key)))
        .map(|i| i.id.clone())
        .ok_or_else(|| OpError::not_found(if inputs.is_empty() { "the Variable Studio has no configuration inputs (config_add_input)".into() } else { format!("no configuration input `{key}`; inputs: {}", inputs.iter().map(|i| format!("{} ({})", i.name, i.id)).collect::<Vec<_>>().join(", ")) }).with_param(field))
}

fn find_option(c: &VsConfiguration, input: &str, key: &str, field: &str) -> Result<String, OpError> {
    match c.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
        Some(ConfigInputKind::List { options, .. }) => options
            .iter()
            .find(|o| o.id == key)
            .or_else(|| options.iter().find(|o| o.name == key || o.name.eq_ignore_ascii_case(key)))
            .map(|o| o.id.clone())
            .ok_or_else(|| OpError::not_found(format!("no configuration `{key}`; configurations: {}", options.iter().map(|o| format!("{} ({})", o.name, o.id)).collect::<Vec<_>>().join(", "))).with_param(field)),
        _ => Err(OpError::param("input", "not a List input")),
    }
}

/// A row key of `input`: a List's configuration (id or name) or a
/// Checkbox's true / false.
fn row_key(c: &VsConfiguration, input: &str, v: &Value, field: &str) -> Result<String, OpError> {
    match (c.inputs.iter().find(|i| i.id == input).map(|i| &i.kind), v) {
        (Some(ConfigInputKind::Checkbox { .. }), Value::Bool(b)) => Ok(b.to_string()),
        (Some(ConfigInputKind::Checkbox { .. }), Value::String(s)) if s == "true" || s == "false" => Ok(s.clone()),
        (Some(ConfigInputKind::List { .. }), Value::String(s)) => find_option(c, input, s, field),
        (Some(ConfigInputKind::Checkbox { .. }), _) => Err(OpError::param(field, "true or false")),
        _ => Err(OpError::param(field, "a configuration id or name")),
    }
}

/// The configured cell a call names: `variable` (with or without #) and
/// `field` ("value", the default, or "description").
fn cell(s: &VariableStudio, a: &Args) -> Result<(String, VsField), OpError> {
    let v = a.opt_str("variable")?.ok_or_else(|| OpError::param("variable", "required on a Variable Studio: the variable whose cell is configured"))?.trim().trim_start_matches('#').to_string();
    if !s.variables.iter().any(|x| x.name == v) {
        return Err(OpError::not_found(format!("no variable `{v}`; variables: {}", s.variables.iter().map(|x| format!("#{}", x.name)).collect::<Vec<_>>().join(", "))).with_param("variable"));
    }
    let field = match a.opt_str("field")?.unwrap_or("value") {
        "value" => VsField::Value,
        "description" => VsField::Description,
        _ => return Err(OpError::param("field", "value or description")),
    };
    Ok((v, field))
}

fn field_name(f: VsField) -> &'static str {
    if f == VsField::Value { "value" } else { "description" }
}

fn describe(od: &OpenDoc, tab: TabId) -> Value {
    let c = cfg(od, tab);
    let mut v = super::describe_inputs(&c.inputs);
    v["configured"] = json!(c.configured.iter().map(|x| json!({ "input": x.input, "variable": format!("#{}", x.variable), "field": field_name(x.field), "values": x.values })).collect::<Vec<_>>());
    v["active"] = json!(c.active.0);
    v
}

/// One edit of the studio's configuration, on a copy first so a failing
/// edit changes nothing.
fn edit(od: &mut OpenDoc, tab: TabId, desc: &str, f: impl FnOnce(&mut VsConfiguration) -> Result<Value, OpError>) -> Result<crate::registry::OpOutput, OpError> {
    let mut copy = cfg(od, tab).clone();
    let mut v = f(&mut copy)?;
    od.session.edit_tab(tab, desc, |c| {
        if let TabContent::VariableStudio(s) = c {
            s.configuration = copy;
        }
    })?;
    if !v.is_object() {
        v = json!({});
    }
    v["configuration"] = describe(od, tab);
    Ok(v.into())
}

pub(super) fn list(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let mut v = describe(od, tab);
    // Each input's ⋯ › Copy table.
    if a.bool_or("copy", false)? {
        let c = cfg(od, tab);
        v["copied"] = json!(c.inputs.iter().map(|i| (i.id.clone(), json!(vsc::table_text(c, &i.id)))).collect::<serde_json::Map<_, _>>());
    }
    Ok(v.into())
}

pub(super) fn add_input(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let kind = a.str("kind")?;
    let name = a.opt_str("name")?.map(|n| n.trim().trim_start_matches('#').to_string()).filter(|n| !n.is_empty());
    let input_kind = match kind {
        "list" => None,
        // The Checkbox dialog: its name, default on (live vs-911).
        "checkbox" => Some(ConfigInputKind::Checkbox { default: true }),
        "variable" => {
            let vkind = match a.opt_str("variable_kind")?.unwrap_or("LENGTH") {
                "ANGLE" => VariableKind::Angle,
                "INTEGER" => VariableKind::Integer,
                "REAL" => VariableKind::Real,
                "TEXT" => VariableKind::Text,
                _ => VariableKind::Length,
            };
            if name.as_deref().is_some_and(|n| !noshape_model::variable_table::is_identifier(n)) {
                return Err(OpError::param("name", "Variable name must be a letter followed by a string of letters or numbers."));
            }
            // The dialog's prefilled Default / Min / Max: 1, 0, 100000 in the
            // workspace's unit (none for Text).
            let unit = match vkind {
                VariableKind::Length => Some(units.length.symbol()),
                VariableKind::Angle => Some(units.angle.symbol()),
                _ => None,
            };
            let with_unit = |n: &str| unit.map(|u| format!("{n} {u}")).unwrap_or_else(|| n.to_string());
            let default = match a.get("value") {
                Some(v) => text(v, "value")?,
                None if vkind == VariableKind::Text => String::new(),
                None => with_unit("1"),
            };
            let range = match a.opt_array("range")? {
                Some(r) => Some((text(&r[0], "range[0]")?, text(&r[1], "range[1]")?)),
                None if vkind == VariableKind::Text => None,
                None => Some((with_unit("0"), with_unit("100000"))),
            };
            if vkind == VariableKind::Text && range.is_some() {
                return Err(OpError::param("range", "a Text variable has no range"));
            }
            Some(ConfigInputKind::Variable { kind: vkind, default: ExprValue::new(default), min: range.as_ref().map(|r| ExprValue::new(r.0.clone())), max: range.map(|r| ExprValue::new(r.1)) })
        }
        _ => return Err(OpError::param("kind", "list, checkbox or variable")),
    };
    if input_kind.is_some() && name.is_none() {
        return Err(OpError::param("name", "required for a Checkbox or a configuration Variable (its dialog asks for it)"));
    }
    edit(od, tab, vsc::ADD_INPUT, |c| {
        let id = match input_kind {
            Some(k) => vsc::add_named_input(c, name.as_deref().unwrap_or_default(), k),
            None => {
                let id = vsc::add_list_input(c);
                if let Some(n) = &name {
                    vsc::rename_input(c, &id, n);
                }
                id
            }
        };
        Ok(json!({ "input": id }))
    })
}

pub(super) fn edit_input(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_input(cfg(od, tab), a.str("input")?, "input")?;
    if a.get("value").is_some() || a.get("range").is_some() {
        return Err(OpError::invalid("a Variable Studio's configuration variable keeps the default and range of its dialog; choose the value it shows with config_activate").with_param(if a.get("value").is_some() { "value" } else { "range" }));
    }
    let name = a.opt_str("name")?.ok_or_else(|| OpError::invalid("nothing to change: give `name`"))?.to_string();
    edit(od, tab, vsc::EDIT, |c| {
        vsc::rename_input(c, &id, &name);
        Ok(json!({ "input": id }))
    })
}

pub(super) fn delete_input(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_input(cfg(od, tab), a.str("input")?, "input")?;
    edit(od, tab, vsc::DELETE_INPUT, |c| {
        vsc::delete_input(c, &id);
        Ok(json!({ "deleted": id }))
    })
}

pub(super) fn add_option(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_input(cfg(od, tab), a.str("input")?, "input")?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "must not be empty"));
    }
    edit(od, tab, vsc::EDIT, |c| vsc::add_row(c, &id, &name).map(|o| json!({ "option": o })).ok_or_else(|| OpError::param("input", "not a List input")))
}

/// A row's name cell and its menu: Set as default, Duplicate row, Move
/// up / down, Delete row.
pub(super) fn edit_option(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let c = cfg(od, tab);
    let input = find_input(c, a.str("input")?, "input")?;
    let option = find_option(c, &input, a.str("option")?, "option")?;
    if a.bool_or("delete", false)? {
        if matches!(c.inputs.iter().find(|i| i.id == input).map(|i| &i.kind), Some(ConfigInputKind::List { options, .. }) if options.len() < 2) {
            return Err(OpError::param("option", "a List keeps at least one configuration"));
        }
        return edit(od, tab, vsc::DELETE_ROW, |c| {
            vsc::delete_row(c, &input, &option);
            Ok(json!({ "deleted": option }))
        });
    }
    let mut out = None;
    if let Some(n) = a.opt_str("name")? {
        let n = n.to_string();
        out = Some(edit(od, tab, vsc::EDIT, |c| {
            vsc::rename_row(c, &input, &option, &n);
            Ok(json!({ "option": option }))
        })?);
    }
    if a.bool_or("set_default", false)? {
        out = Some(edit(od, tab, vsc::SET_DEFAULT, |c| {
            vsc::set_default_row(c, &input, &option);
            Ok(json!({ "option": option }))
        })?);
    }
    if let Some(by) = a.get("move").and_then(Value::as_i64) {
        out = Some(edit(od, tab, vsc::move_description(by < 0), |c| {
            vsc::move_row(c, &input, &option, by < 0);
            Ok(json!({ "option": option }))
        })?);
    }
    if a.bool_or("duplicate", false)? {
        out = Some(edit(od, tab, vsc::DUPLICATE_ROW, |c| Ok(json!({ "option": vsc::duplicate_row(c, &input, &option) })))?);
    }
    out.ok_or_else(|| OpError::invalid("give `name`, `delete`, `set_default`, `move` or `duplicate`"))
}

/// A Value / Description cell's Configure (with <input>) / Set to
/// #<configuration variable>, or `unconfigure`.
pub(super) fn configure(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let s = studio(od, tab);
    let (variable, field) = cell(s, a)?;
    if a.bool_or("unconfigure", false)? {
        if s.configuration.configured_cell(&variable, field).is_none() {
            return Err(OpError::param("variable", "this cell isn't configured"));
        }
        return edit(od, tab, vsc::UNCONFIGURE, |c| {
            c.unconfigure(&variable, field);
            Ok(json!({ "unconfigured": format!("#{variable}") }))
        });
    }
    let input = find_input(&s.configuration, a.opt_str("input")?.ok_or_else(|| OpError::param("input", "required: the input that configures the cell"))?, "input")?;
    let def = s.variables.iter().find(|x| x.name == variable).expect("checked");
    match s.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
        Some(ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }) => {}
        // Set to #<variable>: a Value of its type.
        Some(ConfigInputKind::Variable { kind, .. }) => {
            use noshape_model::variables::VarKind;
            let fits = match def.kind {
                Some(VarKind::Length) => *kind == VariableKind::Length,
                Some(VarKind::Angle) => *kind == VariableKind::Angle,
                Some(VarKind::Number) => matches!(kind, VariableKind::Integer | VariableKind::Real),
                _ => true,
            };
            if field != VsField::Value || !fits {
                return Err(OpError::param("input", "a configuration variable sets a Value of its own type"));
            }
        }
        _ => return Err(OpError::param("input", "a List, Checkbox or configuration Variable input")),
    }
    if s.configuration.configured_cell(&variable, field).is_some() {
        return Err(OpError::param("variable", "already configured (unconfigure it first)"));
    }
    let current = if field == VsField::Value { def.expr.text.clone() } else { def.description.clone() };
    edit(od, tab, vsc::CONFIGURE, |c| {
        c.configure(&input, &variable, field, &current);
        Ok(json!({ "configured": format!("#{variable}"), "field": field_name(field) }))
    })
}

pub(super) fn set_value(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let s = studio(od, tab);
    let (variable, field) = cell(s, a)?;
    let input = s.configuration.configured_cell(&variable, field).map(|x| x.input.clone()).ok_or_else(|| OpError::param("variable", "not configured; use config_configure first"))?;
    let key = row_key(&s.configuration, &input, a.get("configuration").expect("required"), "configuration")?;
    let value = text(a.get("value").ok_or_else(|| OpError::param("value", "required"))?, "value")?;
    edit(od, tab, vsc::EDIT, |c| {
        vsc::set_cell(c, &variable, field, &key, &value);
        Ok(json!({ "set": { "variable": format!("#{variable}"), "configuration": key } }))
    })
}

/// The configuration the studio shows (its rows' blue bar, the
/// Configuration box): saved with the studio, one edit per input.
pub(super) fn activate(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let c = cfg(od, tab).clone();
    let mut picks: Vec<(String, String)> = Vec::new();
    if a.bool_or("reset", false)? {
        for i in &c.inputs {
            let d = match &i.kind {
                ConfigInputKind::List { default, .. } => default.clone(),
                ConfigInputKind::Checkbox { default } => default.to_string(),
                ConfigInputKind::Variable { default, .. } => default.text.clone(),
                ConfigInputKind::Text { default } => default.clone(),
            };
            picks.push((i.id.clone(), d));
        }
    }
    for (key, v) in a.object("values")?.into_iter().flatten() {
        let field = format!("values.{key}");
        let input = find_input(&c, key, &field)?;
        let value = match c.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
            Some(ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }) => row_key(&c, &input, v, &field)?,
            _ => text(v, &field)?,
        };
        picks.retain(|p| p.0 != input);
        picks.push((input, value));
    }
    for (input, value) in picks.into_iter().filter(|(i, v)| c.active.0.get(i) != Some(v)) {
        edit(od, tab, vsc::SWITCH, |c| {
            vsc::switch(c, &input, &value);
            Ok(json!({}))
        })?;
    }
    let mut v = describe(od, tab);
    v["active"] = json!(cfg(od, tab).active.0);
    Ok(v.into())
}

/// Paste inputs copied from a Part Studio, an Assembly or another studio.
pub(super) fn copy_inputs(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let from = doc::find_tab(od.doc(), a.str("from")?).map_err(|e| e.with_param("from"))?;
    if from == tab {
        return Err(OpError::param("from", "another Part Studio, Assembly or Variable Studio"));
    }
    let src: Vec<noshape_model::configuration::ConfigInput> = match od.doc().tab(from).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => ps.configuration.inputs.clone(),
        Some(TabContent::Assembly(x)) => x.configuration.inputs.clone(),
        Some(TabContent::VariableStudio(v)) => v.configuration.inputs.clone(),
        _ => return Err(OpError::param("from", "a Part Studio, Assembly or Variable Studio")),
    };
    let copied: Vec<noshape_model::configuration::ConfigInput> = match a.opt_array("inputs")? {
        Some(list) => list
            .iter()
            .enumerate()
            .map(|(k, v)| {
                let key = v.as_str().unwrap_or_default();
                src.iter().find(|i| i.id == key).or_else(|| src.iter().find(|i| i.name.eq_ignore_ascii_case(key))).cloned().ok_or_else(|| OpError::not_found(format!("no configuration input `{key}` in `from`")).with_param(format!("inputs[{k}]")))
            })
            .collect::<Result<_, _>>()?,
        None => src,
    };
    if copied.is_empty() {
        return Err(OpError::param("from", "it has no configuration inputs"));
    }
    edit(od, tab, vsc::PASTE_INPUTS, |c| {
        let before = c.inputs.len();
        vsc::paste_inputs(c, copied);
        Ok(json!({ "pasted": c.inputs[before..].iter().map(|i| i.id.clone()).collect::<Vec<_>>() }))
    })
}

pub(super) fn edit_ids(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let c = cfg(od, tab);
    let input = find_input(c, a.str("input")?, "input")?;
    let new_id = a.opt_str("id")?.map(str::to_string).unwrap_or_else(|| input.clone());
    if let Some(e) = vsc::id_error(&new_id) {
        return Err(OpError::param("id", e));
    }
    if new_id != input && c.inputs.iter().any(|i| i.id == new_id) {
        return Err(OpError::param("id", "another input has this ID"));
    }
    let mut rows: Vec<(String, String)> = Vec::new();
    for (k, v) in a.object("options")?.into_iter().flatten() {
        let field = format!("options.{k}");
        let old = find_option(c, &input, k, &field)?;
        let new = v.as_str().ok_or_else(|| OpError::param(&field, "the new ID"))?.to_string();
        if let Some(e) = vsc::id_error(&new) {
            return Err(OpError::param(&field, e));
        }
        rows.push((old, new));
    }
    edit(od, tab, vsc::EDIT_IDS, |c| {
        c.change_ids(&input, &new_id, &rows);
        let mut seen = std::collections::HashSet::new();
        if let Some(ConfigInputKind::List { options, .. }) = c.inputs.iter().find(|i| i.id == new_id).map(|i| &i.kind)
            && let Some(d) = options.iter().find(|o| !seen.insert(o.id.clone()))
        {
            return Err(OpError::param("options", &format!("two rows would have the ID `{}`", d.id)));
        }
        Ok(json!({ "input": new_id }))
    })
}

pub(super) fn visibility(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let c = cfg(od, tab);
    let input = find_input(c, a.str("input")?, "input")?;
    let v = super::visibility_arg(a)?;
    super::check_visibility(&c.inputs, &input, &v)?;
    edit(od, tab, vsc::VISIBILITY, |c| {
        if let Some(i) = c.inputs.iter_mut().find(|i| i.id == input) {
            i.visible_when = v;
        }
        Ok(json!({ "input": input }))
    })
}

/// The input's ⋯ › Move up / Move down until it takes `to`'s place.
pub(super) fn reorder(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let c = cfg(od, tab);
    let from = find_input(c, a.str("input")?, "input")?;
    let to = find_input(c, a.str("to")?, "to")?;
    let pos = |id: &str| c.inputs.iter().position(|i| i.id == id).expect("found");
    let (i, j) = (pos(&from), pos(&to));
    let up = j < i;
    edit(od, tab, vsc::move_description(up), |c| {
        for _ in 0..i.abs_diff(j) {
            vsc::move_input(c, &from, up);
        }
        Ok(json!({}))
    })
}
