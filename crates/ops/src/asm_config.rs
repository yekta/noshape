//! Assembly configuration edits (the assembly's Configuration panel),
//! shared by the panel and the agent API. Descriptions are the Part Studio
//! panel's ([`crate::config`]) where the edit is the same.

use noshape_model::assembly::Assembly;
use noshape_model::assembly::config::{self as acfg, ConfigTarget};
use noshape_model::configuration::{ConfigInput, ConfigInputKind, VariableKind};
use noshape_model::params::ParamValue;
use noshape_units::ExprValue;

/// A fresh input / option id suffix (the panel's ids are not names).
pub fn rand_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    format!("{:x}", (n ^ SEQ.fetch_add(1, Ordering::Relaxed)) & 0xffff_ffff_ffff)
}

/// Configure Assembly ▾ / Add configuration input ▾: "List", "Checkbox"
/// or "Variable" (live 191). Returns the input id.
pub fn add_input(a: &mut Assembly, label: &str) -> String {
    let n = a.configuration.inputs.len() + 1;
    let id = format!("in{}", rand_id());
    let input = match label {
        "Checkbox" => ConfigInput { id: id.clone(), name: format!("Checkbox {n}"), kind: ConfigInputKind::Checkbox { default: false }, visible_when: None, excluded: false },
        "Variable" => ConfigInput { id: id.clone(), name: format!("Variable {n}"), kind: ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("10 mm"), min: None, max: None }, visible_when: None, excluded: false },
        _ => acfg::new_list_input(id.clone(), n),
    };
    a.configuration.inputs.push(input);
    id
}

pub fn rename_input(a: &mut Assembly, input: &str, name: &str) {
    if let Some(i) = a.configuration.inputs.iter_mut().find(|i| i.id == input) {
        i.name = name.trim().to_string();
    }
}

/// A configuration variable's default value.
pub fn set_variable_default(a: &mut Assembly, input: &str, text: &str) {
    if let Some(ConfigInput { kind: ConfigInputKind::Variable { default, .. }, .. }) = a.configuration.inputs.iter_mut().find(|i| i.id == input) {
        *default = ExprValue::new(text);
    }
}

/// The italic "Name" row: a new configuration (copies the default row's
/// configured values). Returns its id.
pub fn add_option(a: &mut Assembly, input: &str, name: &str) -> Option<String> {
    if !matches!(a.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind), Some(ConfigInputKind::List { .. })) {
        return None;
    }
    let id = format!("opt{}", rand_id());
    a.add_config_row(input, name.trim(), id.clone());
    Some(id)
}

pub fn rename_option(a: &mut Assembly, input: &str, option: &str, name: &str) {
    if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = a.configuration.inputs.iter_mut().find(|i| i.id == input)
        && let Some(o) = options.iter_mut().find(|o| o.id == option)
    {
        o.name = name.to_string();
    }
}

/// Row › Delete (never the last configuration); its configured values go.
pub fn delete_option(a: &mut Assembly, input: &str, option: &str) {
    if let Some(ConfigInput { kind: ConfigInputKind::List { options, default }, .. }) = a.configuration.inputs.iter_mut().find(|i| i.id == input)
        && options.len() > 1
    {
        options.retain(|o| o.id != option);
        if *default == option {
            *default = options[0].id.clone();
        }
    }
    for c in a.configuration.configured.iter_mut() {
        c.values.remove(option);
    }
}

pub const COPY_INPUT: &str = "Copy configuration input";

/// Input ⋯ › Copy: "<name> copy" after the others.
pub fn duplicate_input(a: &mut Assembly, input: &str) {
    if let Some(mut i) = a.configuration.inputs.iter().find(|i| i.id == input).cloned() {
        i.id = format!("in{}", rand_id());
        i.name = format!("{} copy", i.name);
        a.configuration.inputs.push(i);
    }
}

pub fn configure_mate_description(mate: &str) -> String {
    format!("Configure {mate}")
}

pub fn configure_suppression_description(instance: &str) -> String {
    format!("Configure suppression : {instance}")
}

/// Configure an instance's suppression or a mate parameter in `input`.
/// False when already configured or not configurable.
pub fn configure(a: &mut Assembly, input: &str, target: ConfigTarget, param: &str) -> bool {
    a.configure_param(input, target, param)
}

/// A configured column's value for one row (list option id, or "true" /
/// "false" for a checkbox input).
pub fn set_cell(a: &mut Assembly, column: usize, key: &str, v: ParamValue) {
    a.set_config_cell(column, key, v);
}

/// Input propagation… for one instance: None when its source has no input
/// with `own`'s id and kind (List / Checkbox), else whether it follows the
/// assembly's input already.
pub fn propagation_state(a: &Assembly, own: &ConfigInput, theirs: &[ConfigInput], instance: noshape_model::assembly::InstanceId) -> Option<bool> {
    if !matches!(own.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }) {
        return None;
    }
    let t = theirs.iter().find(|t| t.id == own.id)?;
    (std::mem::discriminant(&t.kind) == std::mem::discriminant(&own.kind)).then(|| a.configuration.configured.iter().any(|c| c.target == ConfigTarget::Instance(instance) && c.param == format!("{}{}", acfg::param::INPUT_PREFIX, own.id)))
}

/// Input propagation… candidates of `input`: the instances whose source
/// (`source_inputs`) has an input with the same id and kind, and whether
/// each follows the assembly's already.
pub fn propagation_candidates(a: &Assembly, input: &str, source_inputs: &dyn Fn(&noshape_model::assembly::Instance) -> Vec<ConfigInput>) -> Vec<(noshape_model::assembly::InstanceId, String, bool)> {
    let Some(own) = a.configuration.inputs.iter().find(|i| i.id == input) else { return Vec::new() };
    a.instances.iter().filter_map(|i| propagation_state(a, own, &source_inputs(i), i.id).map(|on| (i.id, i.name.clone(), on))).collect()
}

/// Input propagation ✓: each picked instance's input follows the
/// assembly's (each option to the option with the same id; a checkbox to
/// its state).
pub fn propagate_inputs(a: &mut Assembly, picks: &[(String, noshape_model::assembly::InstanceId)]) {
    for (input, iid) in picks {
        let Some(i) = a.configuration.inputs.iter().find(|i| &i.id == input) else { continue };
        let keys: Vec<String> = match &i.kind {
            ConfigInputKind::List { options, .. } => options.iter().map(|o| o.id.clone()).collect(),
            ConfigInputKind::Checkbox { .. } => vec!["true".into(), "false".into()],
            _ => continue,
        };
        let param = format!("{}{input}", acfg::param::INPUT_PREFIX);
        if a.configuration.configured.iter().any(|c| c.target == ConfigTarget::Instance(*iid) && c.param == param) {
            continue;
        }
        let values = keys.iter().map(|k| (k.clone(), ParamValue::Text(k.clone()))).collect();
        a.configuration.configured.push(acfg::AsmConfiguredParam { input: input.clone(), target: ConfigTarget::Instance(*iid), param, values });
    }
}

/// Visibility conditions… of one input (None: always shown).
pub fn set_visibility(a: &mut Assembly, input: &str, v: Option<noshape_model::configuration::Visibility>) {
    if let Some(i) = a.configuration.inputs.iter_mut().find(|i| i.id == input) {
        i.visible_when = v;
    }
}
