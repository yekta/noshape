//! The config ops on an Assembly tab (the assembly's Configuration panel):
//! inputs and configurations as in a Part Studio; configured columns are an
//! instance's suppression or a mate's parameters.

use super::text;
use crate::error::OpError;
use crate::registry::Args;
use crate::{Ctx, OpenDoc, asm_config, config, doc};
use noshape_model::assembly::Assembly;
use noshape_model::assembly::config::{ConfigTarget, param};
use noshape_model::configuration::{ConfigInputKind, ConfigValues, VariableKind};
use noshape_model::document::{TabContent, TabId};
use noshape_model::params::ParamValue;
use noshape_units::ExprValue;
use serde_json::{Value, json};

/// Whether `tab` names an Assembly.
pub(super) fn is_assembly(ctx: &mut Ctx, a: &Args) -> Result<bool, OpError> {
    let Some(t) = a.opt_str("tab")? else { return Ok(false) };
    let od = ctx.doc(a.opt_str("doc")?)?;
    Ok(doc::find_tab(od.doc(), t).ok().and_then(|id| od.doc().tab(id)).is_some_and(|x| matches!(x.content, TabContent::Assembly(_))))
}

fn target<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<(&'a mut OpenDoc, TabId), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("tab")?, "Assembly", |c| matches!(c, TabContent::Assembly(_)))?;
    Ok((od, tab))
}

fn asm(od: &OpenDoc, tab: TabId) -> &Assembly {
    match od.doc().tab(tab).map(|t| &t.content) {
        Some(TabContent::Assembly(a)) => a,
        _ => unreachable!("checked by target"),
    }
}

/// The values the assembly is solved in: defaults, then the active choice.
pub(crate) fn values(od: &OpenDoc, tab: TabId, a: &Assembly) -> ConfigValues {
    let mut v = a.configuration.defaults();
    if let Some(chosen) = od.active_config.get(&tab) {
        v.0.extend(chosen.0.clone());
    }
    v
}

fn find_input(a: &Assembly, key: &str, field: &str) -> Result<String, OpError> {
    let inputs = &a.configuration.inputs;
    inputs
        .iter()
        .find(|i| i.id == key)
        .or_else(|| inputs.iter().find(|i| i.name == key || i.name.eq_ignore_ascii_case(key)))
        .map(|i| i.id.clone())
        .ok_or_else(|| OpError::not_found(format!("no configuration input `{key}`; inputs: {}", inputs.iter().map(|i| format!("{} ({})", i.name, i.id)).collect::<Vec<_>>().join(", "))).with_param(field))
}

fn find_option(a: &Assembly, input: &str, key: &str, field: &str) -> Result<String, OpError> {
    match a.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
        Some(ConfigInputKind::List { options, .. }) => options
            .iter()
            .find(|o| o.id == key)
            .or_else(|| options.iter().find(|o| o.name == key || o.name.eq_ignore_ascii_case(key)))
            .map(|o| o.id.clone())
            .ok_or_else(|| OpError::not_found(format!("no configuration `{key}`; configurations: {}", options.iter().map(|o| format!("{} ({})", o.name, o.id)).collect::<Vec<_>>().join(", "))).with_param(field)),
        _ => Err(OpError::param("input", "not a List input")),
    }
}

fn target_name(a: &Assembly, t: ConfigTarget) -> String {
    match t {
        ConfigTarget::Instance(id) => a.instance(id).map(|i| i.name.clone()),
        ConfigTarget::Mate(id) => a.mate(id).map(|m| m.name.clone()),
    }
    .unwrap_or_default()
}

/// API names of the configurable parameters.
const PARAMS: [(&str, &str); 7] = [
    ("suppressed", param::UNSUPPRESSED),
    ("offset", param::OFFSET),
    ("offset_x", param::OFFSET_X),
    ("offset_y", param::OFFSET_Y),
    ("offset_z", param::OFFSET_Z),
    ("rotation", param::ROTATION),
    ("flip_primary", param::FLIP),
];

fn param_name(p: &str) -> String {
    PARAMS.iter().find(|x| x.1 == p).map(|x| x.0.to_string()).unwrap_or_else(|| p.to_string())
}

fn describe(a: &Assembly) -> Value {
    let mut v = super::describe_inputs(&a.configuration.inputs);
    v["configured"] = json!(a
        .configuration
        .configured
        .iter()
        .map(|c| {
            let vals: serde_json::Map<String, Value> = c
                .values
                .iter()
                .map(|(k, v)| {
                    let j = match v {
                        // Unsuppressed is stored; the API says suppressed.
                        ParamValue::Bool(b) if c.param == param::UNSUPPRESSED => json!(!b),
                        ParamValue::Bool(b) => json!(b),
                        ParamValue::Expr(e) => json!(e.text),
                        ParamValue::Text(t) => json!(t),
                        _ => Value::Null,
                    };
                    (k.clone(), j)
                })
                .collect();
            let mut j = json!({ "input": c.input, "param": param_name(&c.param), "values": vals });
            match c.target {
                ConfigTarget::Instance(id) => j["instance"] = json!(format!("inst:{}", id.0)),
                ConfigTarget::Mate(id) => j["mate"] = json!(format!("mate:{}", id.0)),
            }
            j["name"] = json!(target_name(a, c.target));
            j
        })
        .collect::<Vec<_>>());
    v
}

fn edit(od: &mut OpenDoc, tab: TabId, desc: &str, f: impl FnOnce(&mut Assembly) -> Result<Value, OpError>) -> Result<crate::registry::OpOutput, OpError> {
    // On a copy first, so a failing edit changes nothing.
    let mut copy = asm(od, tab).clone();
    let mut v = f(&mut copy)?;
    od.session.edit_tab(tab, desc, |c| *c = TabContent::Assembly(Box::new(copy)))?;
    if !v.is_object() {
        v = json!({});
    }
    v["configuration"] = describe(asm(od, tab));
    Ok(v.into())
}

pub(super) fn list(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let x = asm(od, tab);
    let mut v = describe(x);
    v["active"] = json!(values(od, tab, x).0);
    Ok(v.into())
}

pub(super) fn activate(ctx: &mut Ctx, args: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, args)?;
    let units = od.doc().settings.units;
    let a = asm(od, tab);
    let mut chosen = if args.bool_or("reset", false)? { Default::default() } else { od.active_config.get(&tab).cloned().unwrap_or_default() };
    for (key, v) in args.object("values")?.into_iter().flatten() {
        let field = format!("values.{key}");
        let id = find_input(a, key, &field)?;
        let input = a.configuration.inputs.iter().find(|i| i.id == id).expect("found");
        let value = match (&input.kind, v) {
            (ConfigInputKind::List { .. }, Value::String(o)) => find_option(a, &id, o, &field)?,
            (ConfigInputKind::Checkbox { .. }, Value::Bool(b)) => b.to_string(),
            (ConfigInputKind::Checkbox { .. }, Value::String(b)) if b == "true" || b == "false" => b.clone(),
            (ConfigInputKind::Variable { .. }, v) => {
                let t = text(v, &field)?;
                if let Some(msg) = config::value_error(input, &t, &units) {
                    return Err(OpError::param(&field, &msg));
                }
                t
            }
            (ConfigInputKind::Text { .. }, v) => text(v, &field)?,
            (ConfigInputKind::List { .. }, _) => return Err(OpError::param(&field, "a configuration id or name")),
            (ConfigInputKind::Checkbox { .. }, _) => return Err(OpError::param(&field, "true or false")),
        };
        chosen.0.insert(id, value);
    }
    od.active_config.insert(tab, chosen);
    let x = asm(od, tab);
    Ok(json!({ "active": values(od, tab, x).0 }).into())
}

pub(super) fn add_input(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let kind = a.str("kind")?;
    let label = match kind {
        "list" => "List",
        "checkbox" => "Checkbox",
        "variable" => "Variable",
        _ => return Err(OpError::param("kind", "list, checkbox or variable")),
    };
    let name = a.opt_str("name")?.map(|n| n.trim_start_matches('#').to_string());
    if kind == "variable" && name.as_deref().is_some_and(|n| !noshape_model::variable_table::is_identifier(n)) {
        return Err(OpError::param("name", "Variable name must be a letter followed by a string of letters or numbers."));
    }
    let vkind = match a.opt_str("variable_kind")?.unwrap_or("LENGTH") {
        "ANGLE" => VariableKind::Angle,
        "INTEGER" => VariableKind::Integer,
        "REAL" => VariableKind::Real,
        "TEXT" => VariableKind::Text,
        _ => VariableKind::Length,
    };
    let value = a.get("value").map(|v| text(v, "value")).transpose()?;
    if a.get("range").is_some() {
        return Err(OpError::param("range", "assembly configuration variables have no range"));
    }
    edit(od, tab, config::ADD_INPUT, |x| {
        let id = asm_config::add_input(x, label);
        if let Some(n) = &name {
            asm_config::rename_input(x, &id, n);
        }
        if kind == "variable"
            && let Some(ConfigInputKind::Variable { kind: k, default, .. }) = x.configuration.inputs.iter_mut().find(|i| i.id == id).map(|i| &mut i.kind)
        {
            *k = vkind;
            if let Some(v) = &value {
                *default = ExprValue::new(v.clone());
            }
        }
        Ok(json!({ "input": id }))
    })
}

pub(super) fn edit_input(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_input(asm(od, tab), a.str("input")?, "input")?;
    if a.get("range").is_some() {
        return Err(OpError::param("range", "assembly configuration variables have no range"));
    }
    let mut out = None;
    if let Some(n) = a.opt_str("name")? {
        let n = n.to_string();
        out = Some(edit(od, tab, config::RENAME_INPUT, |x| {
            asm_config::rename_input(x, &id, &n);
            Ok(json!({ "input": id }))
        })?);
    }
    if let Some(v) = a.get("value") {
        let v = text(v, "value")?;
        out = Some(edit(od, tab, config::EDIT_VARIABLE_INPUT, |x| {
            asm_config::set_variable_default(x, &id, &v);
            Ok(json!({ "input": id }))
        })?);
    }
    out.ok_or_else(|| OpError::invalid("nothing to change: give `name` or `value`"))
}

pub(super) fn delete_input(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_input(asm(od, tab), a.str("input")?, "input")?;
    edit(od, tab, config::DELETE_INPUT, |x| {
        x.delete_config_input(&id);
        Ok(json!({ "deleted": id }))
    })
}

pub(super) fn add_option(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_input(asm(od, tab), a.str("input")?, "input")?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "must not be empty"));
    }
    edit(od, tab, config::ADD_OPTION, |x| asm_config::add_option(x, &id, &name).map(|o| json!({ "option": o })).ok_or_else(|| OpError::param("input", "not a List input")))
}

pub(super) fn edit_option(ctx: &mut Ctx, a: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let x = asm(od, tab);
    let input = find_input(x, a.str("input")?, "input")?;
    let option = find_option(x, &input, a.str("option")?, "option")?;
    if a.bool_or("delete", false)? {
        return edit(od, tab, config::DELETE_OPTION, |x| {
            asm_config::delete_option(x, &input, &option);
            Ok(json!({ "deleted": option }))
        });
    }
    let name = a.opt_str("name")?.ok_or_else(|| OpError::invalid("give `name` or `delete`"))?.trim().to_string();
    edit(od, tab, config::RENAME_OPTION, |x| {
        asm_config::rename_option(x, &input, &option, &name);
        Ok(json!({ "option": option }))
    })
}

/// The configured target and parameter of config_configure / set_value.
fn column(a: &Assembly, args: &Args) -> Result<(ConfigTarget, &'static str), OpError> {
    let p = args.str("param")?;
    let pid = PARAMS.iter().find(|x| x.0 == p).map(|x| x.1).ok_or_else(|| OpError::param("param", "suppressed, offset, offset_x, offset_y, offset_z, rotation or flip_primary"))?;
    let t = match (args.opt_str("instance")?, args.opt_str("mate")?) {
        (Some(i), None) => {
            if pid != param::UNSUPPRESSED {
                return Err(OpError::param("param", "an instance configures only `suppressed`"));
            }
            ConfigTarget::Instance(crate::ops::assembly::find_instance(a, i, "instance")?)
        }
        (None, Some(m)) => ConfigTarget::Mate(a.mates.iter().find(|x| x.name == m || format!("mate:{}", x.id.0) == m || x.id.0.to_string() == m).map(|x| x.id).ok_or_else(|| OpError::param("mate", "no such mate"))?),
        _ => return Err(OpError::invalid("give `instance` or `mate` (assemblies configure instances and mates)")),
    };
    Ok((t, pid))
}

pub(super) fn configure(ctx: &mut Ctx, args: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, args)?;
    let a = asm(od, tab);
    let input = find_input(a, args.str("input")?, "input")?;
    if !matches!(a.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind), Some(ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. })) {
        return Err(OpError::param("input", "a List or Checkbox input"));
    }
    let (t, p) = column(a, args)?;
    if a.configuration.configured.iter().any(|c| c.target == t && c.param == p) {
        return Err(OpError::param("param", "already configured"));
    }
    let name = target_name(a, t);
    let desc = match t {
        ConfigTarget::Instance(_) => asm_config::configure_suppression_description(&name),
        ConfigTarget::Mate(_) => asm_config::configure_mate_description(&name),
    };
    edit(od, tab, &desc, |x| {
        if asm_config::configure(x, &input, t, p) { Ok(json!({ "configured": param_name(p) })) } else { Err(OpError::param("param", "the mate has no such parameter")) }
    })
}

pub(super) fn set_value(ctx: &mut Ctx, args: &Args) -> Result<crate::registry::OpOutput, OpError> {
    let (od, tab) = target(ctx, args)?;
    let a = asm(od, tab);
    let (t, p) = column(a, args)?;
    let index = a.configuration.configured.iter().position(|c| c.target == t && c.param == p).ok_or_else(|| OpError::param("param", "not configured; use config_configure first"))?;
    let input = a.configuration.configured[index].input.clone();
    let key = match args.get("configuration").expect("required") {
        Value::Bool(b) => b.to_string(),
        Value::String(s) if s == "true" || s == "false" => s.clone(),
        Value::String(s) => find_option(a, &input, s, "configuration")?,
        _ => return Err(OpError::param("configuration", "configuration id/name or true/false")),
    };
    let raw = args.get("value").ok_or_else(|| OpError::param("value", "required"))?;
    let value = if noshape_model::assembly::config::param_is_bool(p) {
        let b = raw.as_bool().ok_or_else(|| OpError::param("value", "true or false"))?;
        // `suppressed` in the API; the column stores Unsuppressed.
        ParamValue::Bool(if p == param::UNSUPPRESSED { !b } else { b })
    } else {
        ParamValue::Expr(ExprValue::new(text(raw, "value")?))
    };
    edit(od, tab, config::EDIT_CONFIGURED_VALUE, |x| {
        asm_config::set_cell(x, index, &key, value);
        Ok(json!({ "set": { "param": param_name(p), "configuration": key } }))
    })
}
