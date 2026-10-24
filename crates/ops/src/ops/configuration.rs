//! Part Studio and Assembly configurations (the Configurations panel).

use super::{doc_field, edit_studio, feature_arg, regen_report, s, studio_target};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, config, doc, values};
use noshape_model::configuration::{ConfigInputKind, SUPPRESSED_PARAM};
use noshape_model::document::TabId;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use serde_json::{Value, json};

pub(crate) mod asm;

/// The `tab` of the config ops: a Part Studio or an Assembly.
fn cfg_tab_field() -> crate::registry::Field {
    opt("tab", json!({ "type": "string" }), "Part Studio or Assembly tab, by name or id. Default: the first Part Studio.")
}

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "config",
            verb: "list",
            summary: "The Part Studio's configuration inputs, configurations and configured parameters",
            description: "Inputs (List with its configurations, Checkbox, Variable with default and range) with ids, and every configured parameter with its value per configuration.",
            fields: vec![doc_field(), cfg_tab_field()],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "config",
            verb: "add_input",
            summary: "Add a configuration input (List, Checkbox or configuration Variable)",
            description: "\"Configure Part Studio\" › List / Checkbox / Variable. A List starts with one configuration \"Default\" (add more with config_add_option). A Variable input is a #variable whose value each configuration can set.",
            fields: vec![
                doc_field(),
                cfg_tab_field(),
                req("kind", json!({ "type": "string", "enum": ["list", "checkbox", "variable"] }), "Input kind."),
                opt("name", s(), "Name (default \"Configuration\", \"List 2\", \"Checkbox 1\"...; required for a variable: its #name)."),
                opt("variable_kind", json!({ "type": "string", "enum": ["LENGTH", "ANGLE", "INTEGER", "REAL", "TEXT"] }), "Variable: kind (default LENGTH)."),
                opt("value", json!({ "type": ["string", "number"] }), "Variable: default value expression."),
                opt("range", json!({ "type": "array", "items": { "type": "string" }, "minItems": 2, "maxItems": 2 }), "Variable: [min, max] expressions."),
            ],
            mutates: true,
            destructive: false,
            run: add_input,
        },
        Op {
            group: "config",
            verb: "edit_input",
            summary: "Rename a configuration input or change a configuration variable's default and range",
            description: "",
            fields: vec![
                doc_field(),
                cfg_tab_field(),
                req("input", s(), "Input id or name."),
                opt("name", s(), "New name."),
                opt("value", json!({ "type": ["string", "number"] }), "Variable input: new default."),
                opt("range", json!({ "type": "array", "items": { "type": "string" }, "minItems": 2, "maxItems": 2 }), "Variable input: new [min, max] expressions (with `value`)."),
            ],
            mutates: true,
            destructive: false,
            run: edit_input,
        },
        Op {
            group: "config",
            verb: "delete_input",
            summary: "Delete a configuration input and what it configures",
            description: "",
            fields: vec![doc_field(), cfg_tab_field(), req("input", s(), "Input id or name.")],
            mutates: true,
            destructive: true,
            run: delete_input,
        },
        Op {
            group: "config",
            verb: "add_option",
            summary: "Add a configuration to a List input",
            description: "Returns its id as `option`. Configured parameters take the default configuration's value in it until set with config_set_value.",
            fields: vec![doc_field(), cfg_tab_field(), req("input", s(), "List input id or name."), req("name", s(), "Configuration name (e.g. \"Large\").")],
            mutates: true,
            destructive: false,
            run: add_option,
        },
        Op {
            group: "config",
            verb: "edit_option",
            summary: "Rename or delete a configuration of a List input",
            description: "A List keeps at least one configuration.",
            fields: vec![doc_field(), cfg_tab_field(), req("input", s(), "List input id or name."), req("option", s(), "Configuration id or name."), opt("name", s(), "New name."), opt("delete", json!({ "type": "boolean" }), "Delete it.")],
            mutates: true,
            destructive: false,
            run: edit_option,
        },
        Op {
            group: "config",
            verb: "configure",
            summary: "Make a parameter (or suppression) of a feature, assembly instance or mate configured by a List or Checkbox input",
            description: "Every configuration starts with the current value; then set values with config_set_value. Part Studio: `feature` and `param`, a parameter id of the feature (see feature_schema) or \"suppressed\" (Configure suppression). Assembly: `instance` with `param` \"suppressed\", or `mate` with \"suppressed\", \"offset\", \"offset_x\", \"offset_y\", \"offset_z\", \"rotation\" or \"flip_primary\".",
            fields: vec![doc_field(), cfg_tab_field(), opt("feature", s(), "Feature id or name (Part Studio)."), opt("instance", s(), "Instance id or name (Assembly)."), opt("mate", s(), "Mate id or name (Assembly)."), req("param", s(), "Parameter id, or \"suppressed\"."), req("input", s(), "List or Checkbox input id or name.")],
            mutates: true,
            destructive: false,
            run: configure,
        },
        Op {
            group: "config",
            verb: "set_value",
            summary: "Set a configured parameter's value in one configuration",
            description: "`configuration` is the List configuration's id or name, or true/false for a Checkbox. `value` uses the feature parameter's JSON form (\"40 mm\", an enum id, true/false...).",
            fields: vec![
                doc_field(),
                cfg_tab_field(),
                opt("feature", s(), "Feature id or name (Part Studio)."),
                opt("instance", s(), "Instance id or name (Assembly)."),
                opt("mate", s(), "Mate id or name (Assembly)."),
                req("param", s(), "Parameter id, or \"suppressed\" (see config_configure)."),
                req("configuration", json!({ "type": ["string", "boolean"] }), "Configuration id/name, or true/false."),
                req("value", json!({}), "The value."),
            ],
            mutates: true,
            destructive: false,
            run: set_value,
        },
        Op {
            group: "config",
            verb: "activate",
            summary: "Choose the configuration the Part Studio shows (the Configurations box)",
            description: "Sets the active value of configuration inputs; later regenerations, queries, renders, exports and feature edits use it (feature_edit then edits that configuration's configured values). `values` maps each input (id or name) to a List configuration (id or name), true/false for a Checkbox, or an expression for a configuration Variable (\"40 mm\"). Inputs not given keep their current value; `reset: true` returns every input to its default first. View state: no history entry, as in the app.",
            fields: vec![
                doc_field(),
                cfg_tab_field(),
                opt("values", json!({ "type": "object", "additionalProperties": { "type": ["string", "boolean", "number"] } }), "Input -> value."),
                opt("reset", json!({ "type": "boolean" }), "Start from the default configuration."),
            ],
            mutates: false,
            destructive: false,
            run: activate,
        },
        Op {
            group: "config",
            verb: "reorder_inputs",
            summary: "Move a configuration input onto another's place",
            description: "",
            fields: vec![doc_field(), cfg_tab_field(), req("input", s(), "Input to move."), req("to", s(), "Input whose place it takes.")],
            mutates: true,
            destructive: false,
            run: reorder,
        },
    ]
}

fn find_input(ps: &PartStudio, key: &str, field: &str) -> Result<String, OpError> {
    let inputs = &ps.configuration.inputs;
    inputs
        .iter()
        .find(|i| i.id == key)
        .or_else(|| inputs.iter().find(|i| i.name == key || i.name.eq_ignore_ascii_case(key)))
        .map(|i| i.id.clone())
        .ok_or_else(|| OpError::not_found(format!("no configuration input `{key}`; inputs: {}", inputs.iter().map(|i| format!("{} ({})", i.name, i.id)).collect::<Vec<_>>().join(", "))).with_param(field))
}

fn find_option(ps: &PartStudio, input: &str, key: &str) -> Result<String, OpError> {
    match ps.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
        Some(ConfigInputKind::List { options, .. }) => options
            .iter()
            .find(|o| o.id == key)
            .or_else(|| options.iter().find(|o| o.name == key || o.name.eq_ignore_ascii_case(key)))
            .map(|o| o.id.clone())
            .ok_or_else(|| OpError::not_found(format!("no configuration `{key}`; configurations: {}", options.iter().map(|o| format!("{} ({})", o.name, o.id)).collect::<Vec<_>>().join(", "))).with_param("option")),
        _ => Err(OpError::param("input", "not a List input")),
    }
}

fn param_id(p: &str) -> &str {
    if p == "suppressed" { SUPPRESSED_PARAM } else { p }
}

/// One panel edit: commit, repair visibility conditions, regenerate.
fn edit(od: &mut OpenDoc, tab: TabId, desc: &str, f: impl FnOnce(&mut PartStudio) -> Result<Value, OpError>) -> Result<OpOutput, OpError> {
    let (mut v, repaired) = edit_studio(od, tab, desc, |ps| {
        let v = f(ps)?;
        Ok((v, config::commit_repair(ps)))
    })?;
    if !v.is_object() {
        v = json!({});
    }
    if !repaired.is_empty() {
        v["info"] = json!(format!("Visibility conditions for {} were updated", repaired.join(", ")));
    }
    v["configuration"] = describe(doc::studio(od.doc(), tab)?);
    v["regeneration"] = regen_report(od, tab)?;
    Ok(v.into())
}

fn describe(ps: &PartStudio) -> Value {
    let mut v = describe_inputs(&ps.configuration.inputs);
    v["configured"] = describe_configured(ps);
    v
}

/// The inputs as config_list shows them.
fn describe_inputs(inputs: &[noshape_model::configuration::ConfigInput]) -> Value {
    let inputs: Vec<Value> = inputs
        .iter()
        .map(|i| {
            let mut v = json!({ "id": i.id, "name": i.name });
            match &i.kind {
                ConfigInputKind::List { options, default } => {
                    v["kind"] = json!("list");
                    v["configurations"] = json!(options.iter().map(|o| json!({ "id": o.id, "name": o.name })).collect::<Vec<_>>());
                    v["default"] = json!(default);
                }
                ConfigInputKind::Checkbox { default } => {
                    v["kind"] = json!("checkbox");
                    v["default"] = json!(default);
                }
                ConfigInputKind::Variable { kind, default, min, max } => {
                    v["kind"] = json!("variable");
                    v["variable_kind"] = json!(format!("{kind:?}").to_uppercase());
                    v["default"] = json!(default.text);
                    if let (Some(a), Some(b)) = (min, max) {
                        v["range"] = json!([a.text, b.text]);
                    }
                }
                ConfigInputKind::Text { default } => {
                    v["kind"] = json!("text");
                    v["default"] = json!(default);
                }
            }
            v
        })
        .collect();
    json!({ "inputs": inputs })
}

fn describe_configured(ps: &PartStudio) -> Value {
    let configured: Vec<Value> = ps
        .configuration
        .configured
        .iter()
        .map(|c| {
            let spec = ps.feature(c.feature).and_then(|f| f.feature_type()).and_then(|t| t.param(&c.param));
            let vals: serde_json::Map<String, Value> = c.values.iter().map(|(k, v)| (k.clone(), values::to_json(spec, v))).collect();
            json!({
                "input": c.input,
                "feature": c.feature.to_string(),
                "feature_name": ps.feature(c.feature).map(|f| f.name.clone()),
                "param": if c.param == SUPPRESSED_PARAM { "suppressed".to_string() } else { c.param.clone() },
                "values": vals,
            })
        })
        .collect();
    json!(configured)
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::list(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let mut v = describe(doc::studio(od.doc(), tab)?);
    v["active"] = active(od, tab)?;
    Ok(v.into())
}

/// The active configuration: each input's effective value.
fn active(od: &OpenDoc, tab: TabId) -> Result<Value, OpError> {
    let ps = doc::studio(od.doc(), tab)?;
    let (_, effective) = ps.configuration.visibility(&od.current_config(tab)?);
    Ok(json!(effective.0))
}

fn activate(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::activate(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let ps = doc::studio(od.doc(), tab)?;
    let mut chosen = if a.bool_or("reset", false)? { Default::default() } else { od.active_config.get(&tab).cloned().unwrap_or_default() };
    for (key, v) in a.object("values")?.into_iter().flatten() {
        let field = format!("values.{key}");
        let id = find_input(ps, key, &field)?;
        let input = ps.configuration.inputs.iter().find(|i| i.id == id).expect("found");
        let value = match (&input.kind, v) {
            (ConfigInputKind::List { .. }, Value::String(o)) => find_option(ps, &id, o).map_err(|e| e.with_param(&field))?,
            (ConfigInputKind::Checkbox { .. }, Value::Bool(b)) => b.to_string(),
            (ConfigInputKind::Checkbox { .. }, Value::String(b)) if b == "true" || b == "false" => b.clone(),
            (ConfigInputKind::Variable { kind, .. }, v) => {
                let t = text(v, &field)?;
                if let Some(dim) = config::kind_dim(*kind) {
                    noshape_units::evaluate_as(&t, dim, &units, &noshape_units::EmptyScope).map_err(|e| OpError::param(&field, &e.to_string()))?;
                }
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
    Ok(json!({ "active": active(od, tab)?, "regeneration": regen_report(od, tab)? }).into())
}

fn text(v: &Value, field: &str) -> Result<String, OpError> {
    match v {
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(s.trim().to_string()),
        _ => Err(OpError::param(field, "an expression")),
    }
}

fn add_input(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::add_input(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let kind = a.str("kind")?;
    let name = a.opt_str("name")?.map(str::to_string);
    match kind {
        "variable" => {
            let name = name.ok_or_else(|| OpError::param("name", "a configuration variable needs a name"))?;
            if !noshape_model::variable_table::is_identifier(name.trim_start_matches('#')) {
                return Err(OpError::param("name", "Variable name must be a letter followed by a string of letters or numbers."));
            }
            let vk = a.opt_str("variable_kind")?.unwrap_or("LENGTH").to_string();
            let value = match a.get("value") {
                Some(v) => text(v, "value")?,
                None => "10 mm".to_string(),
            };
            let range = match a.opt_array("range")? {
                Some(r) => Some((text(&r[0], "range")?, text(&r[1], "range")?)),
                None => None,
            };
            edit(od, tab, config::ADD_VARIABLE_INPUT, |ps| Ok(json!({ "input": config::add_variable_input(ps, name.trim_start_matches('#'), &vk, &value, range) })))
        }
        "list" | "checkbox" => {
            let label = if kind == "list" { "List" } else { "Checkbox" };
            edit(od, tab, config::ADD_INPUT, |ps| {
                let id = config::add_input(ps, label);
                if let Some(n) = name {
                    config::rename_input(ps, &id, n);
                }
                Ok(json!({ "input": id }))
            })
        }
        _ => Err(OpError::param("kind", "list, checkbox or variable")),
    }
}

fn edit_input(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::edit_input(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let id = find_input(doc::studio(od.doc(), tab)?, a.str("input")?, "input")?;
    let mut out = None;
    if let Some(n) = a.opt_str("name")? {
        let n = n.to_string();
        out = Some(edit(od, tab, config::RENAME_INPUT, |ps| {
            config::rename_input(ps, &id, n);
            Ok(json!({ "input": id }))
        })?);
    }
    if let Some(v) = a.get("value") {
        let v = text(v, "value")?;
        let range = match a.opt_array("range")? {
            Some(r) => Some((noshape_units::ExprValue::new(text(&r[0], "range")?), noshape_units::ExprValue::new(text(&r[1], "range")?))),
            None => None,
        };
        out = Some(edit(od, tab, config::EDIT_VARIABLE_INPUT, |ps| {
            // The panel's default cell, or the Variable dialog with a range.
            match range {
                Some(r) => config::edit_variable_input(ps, &id, noshape_units::ExprValue::new(v), Some(r)),
                None => config::set_variable_default(ps, &id, v),
            }
            Ok(json!({ "input": id }))
        })?);
    } else if a.get("range").is_some() {
        return Err(OpError::param("range", "give `value` with `range`"));
    }
    out.ok_or_else(|| OpError::invalid("nothing to change: give `name` or `value`"))
}

fn delete_input(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::delete_input(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let id = find_input(doc::studio(od.doc(), tab)?, a.str("input")?, "input")?;
    edit(od, tab, config::DELETE_INPUT, |ps| {
        config::delete_input(ps, &id);
        Ok(json!({ "deleted": id }))
    })
}

fn add_option(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::add_option(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let id = find_input(doc::studio(od.doc(), tab)?, a.str("input")?, "input")?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "must not be empty"));
    }
    edit(od, tab, config::ADD_OPTION, |ps| config::add_option(ps, &id, name).map(|o| json!({ "option": o })).ok_or_else(|| OpError::param("input", "not a List input")))
}

fn edit_option(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::edit_option(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let input = find_input(ps, a.str("input")?, "input")?;
    let option = find_option(ps, &input, a.str("option")?)?;
    if a.bool_or("delete", false)? {
        return edit(od, tab, config::DELETE_OPTION, |ps| {
            config::delete_option(ps, &input, &option);
            Ok(json!({ "deleted": option }))
        });
    }
    let name = a.opt_str("name")?.ok_or_else(|| OpError::invalid("give `name` or `delete`"))?.to_string();
    edit(od, tab, config::RENAME_OPTION, |ps| {
        config::rename_option(ps, &input, &option, name);
        Ok(json!({ "option": option }))
    })
}

fn configure(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::configure(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let feature = feature_arg(ps, a, "feature")?;
    let input = find_input(ps, a.str("input")?, "input")?;
    let param = param_id(a.str("param")?).to_string();
    if ps.configuration.configured.iter().any(|c| c.feature == feature && c.param == param) {
        return Err(OpError::param("param", "already configured"));
    }
    edit(od, tab, config::CONFIGURE_PARAMETER, |ps| {
        if config::configure_parameter(ps, feature, &param, &input) {
            Ok(json!({ "configured": param }))
        } else {
            Err(OpError::invalid("the feature has no such parameter, or the input is not a List or Checkbox").with_param("param"))
        }
    })
}

fn set_value(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return asm::set_value(ctx, a);
    }
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let ps = doc::studio(od.doc(), tab)?;
    let feature = feature_arg(ps, a, "feature")?;
    let param = param_id(a.str("param")?).to_string();
    let index = ps.configuration.configured.iter().position(|c| c.feature == feature && c.param == param).ok_or_else(|| OpError::param("param", "not configured; use config_configure first"))?;
    let input = ps.configuration.configured[index].input.clone();
    let key = match a.get("configuration").expect("required") {
        Value::Bool(b) => b.to_string(),
        Value::String(s) if s == "true" || s == "false" => s.clone(),
        Value::String(s) => find_option(ps, &input, s).map_err(|e| e.with_param("configuration"))?,
        _ => return Err(OpError::param("configuration", "configuration id/name or true/false")),
    };
    if !ps.configuration.configured[index].values.contains_key(&key) && !matches!(ps.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind), Some(ConfigInputKind::List { .. })) {
        return Err(OpError::param("configuration", "not a configuration of this input"));
    }
    let raw = a.get("value").ok_or_else(|| OpError::param("value", "required"))?;
    let value = if param == SUPPRESSED_PARAM {
        ParamValue::Bool(raw.as_bool().ok_or_else(|| OpError::param("value", "true or false"))?)
    } else if param.starts_with(noshape_model::regen::features::sketch::CONFIGURED_DIMENSION_PREFIX) {
        // A configured sketch dimension (sketch_dimension_options `configure`).
        ParamValue::Expr(noshape_units::ExprValue::new(text(raw, "value")?))
    } else {
        let spec = ps.feature(feature).and_then(|f| f.feature_type()).and_then(|t| t.param(&param)).ok_or_else(|| OpError::param("param", "unknown parameter"))?;
        values::from_json(spec, raw, &units, od.doc()).map_err(|e| e.with_param("value"))?
    };
    edit(od, tab, config::EDIT_CONFIGURED_VALUE, |ps| {
        config::set_configured_value(ps, index, &key, value);
        Ok(json!({ "set": { "param": a.str("param").unwrap_or_default(), "configuration": key } }))
    })
}

fn reorder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    if asm::is_assembly(ctx, a)? {
        return Err(OpError::param("tab", "assembly configuration inputs keep their order"));
    }
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let from = find_input(ps, a.str("input")?, "input")?;
    let to = find_input(ps, a.str("to")?, "to")?;
    edit(od, tab, config::REORDER_INPUTS, |ps| {
        config::reorder_inputs(ps, &from, &to);
        Ok(json!({}))
    })
}
