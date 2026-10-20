//! Part Studio configuration edits (the Configurations panel), shared by the
//! panel and the agent API. The caller commits each as one microversion with
//! the description given here, then repairs visibility conditions
//! ([`commit_repair`]).

use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, ListOption, SUPPRESSED_PARAM, VariableKind, fs_identifier};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::FeatureId;
use noshape_units::ExprValue;

/// After every configuration edit (Help "Repairing visibility conditions"):
/// conditions naming deleted inputs or options are removed. Returns the
/// inputs whose conditions changed (the panel shows "Visibility conditions
/// for … were updated").
pub fn commit_repair(ps: &mut PartStudio) -> Vec<String> {
    ps.configuration.repair_visibility()
}

pub const ADD_INPUT: &str = "Add configuration input";

/// "Configure Part Studio" › List / Checkbox (a Variable input goes
/// through [`add_variable_input`]). `label` is "List" or "Checkbox".
pub fn add_input(ps: &mut PartStudio, label: &str) -> String {
    let n = ps.configuration.inputs.len() + 1;
    let taken: Vec<String> = ps.configuration.inputs.iter().map(|i| i.id.clone()).collect();
    let kind = match label {
        "Checkbox" => ConfigInputKind::Checkbox { default: false },
        "Variable" => ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("10 mm"), min: None, max: None },
        _ => ConfigInputKind::List {
            // Live (part-studio/182): a new List has the one "Default" row.
            options: vec![ListOption { id: "Default".into(), name: "Default".into() }],
            default: "Default".into(),
        },
    };
    let name = if label == "List" && n == 1 { "Configuration".to_string() } else { format!("{label} {n}") };
    // FeatureScript ids from the names (Edit FeatureScript IDs… shows them).
    let id = fs_identifier(&name, &taken);
    ps.configuration.inputs.push(ConfigInput { id: id.clone(), name, kind, visible_when: None, excluded: false });
    id
}

pub const ADD_VARIABLE_INPUT: &str = "Add configuration variable";

/// The Create variable dialog with "Configuration variable" checked.
/// `kind`: LENGTH, ANGLE, INTEGER, REAL or TEXT.
pub fn add_variable_input(ps: &mut PartStudio, name: &str, kind: &str, value: &str, range: Option<(String, String)>) -> String {
    let vkind = match kind {
        "ANGLE" => VariableKind::Angle,
        "INTEGER" => VariableKind::Integer,
        "REAL" => VariableKind::Real,
        "TEXT" => VariableKind::Text,
        _ => VariableKind::Length,
    };
    let (min, max) = match range {
        Some((lo, hi)) => (Some(ExprValue::new(lo)), Some(ExprValue::new(hi))),
        None => (None, None),
    };
    let taken: Vec<String> = ps.configuration.inputs.iter().map(|i| i.id.clone()).collect();
    let id = fs_identifier(name, &taken);
    ps.configuration.inputs.push(ConfigInput { id: id.clone(), name: name.to_string(), kind: ConfigInputKind::Variable { kind: vkind, default: ExprValue::new(value), min, max }, visible_when: None, excluded: false });
    id
}

pub const RENAME_INPUT: &str = "Rename configuration input";

pub fn rename_input(ps: &mut PartStudio, input: &str, name: String) {
    if let Some(i) = ps.configuration.inputs.iter_mut().find(|i| i.id == input) {
        i.name = name;
    }
}

pub const DELETE_INPUT: &str = "Delete configuration input";

/// An input and every parameter it configures.
pub fn delete_input(ps: &mut PartStudio, input: &str) {
    ps.configuration.inputs.retain(|i| i.id != input);
    ps.configuration.configured.retain(|c| c.input != input);
}

pub const EDIT_VARIABLE_INPUT: &str = "Edit configuration variable";

/// A configuration variable's default value.
pub fn set_variable_default(ps: &mut PartStudio, input: &str, value: String) {
    if let Some(ConfigInput { kind: ConfigInputKind::Variable { default, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == input) {
        *default = ExprValue::new(value);
    }
}

pub const ADD_OPTION: &str = "Add configuration";

/// A List input's "Name" row: a new configuration. Returns its id.
pub fn add_option(ps: &mut PartStudio, input: &str, name: String) -> Option<String> {
    if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == input) {
        let taken: Vec<String> = options.iter().map(|o| o.id.clone()).collect();
        let id = fs_identifier(&name, &taken);
        options.push(ListOption { id: id.clone(), name });
        return Some(id);
    }
    None
}

pub const RENAME_OPTION: &str = "Rename configuration";

pub fn rename_option(ps: &mut PartStudio, input: &str, option: &str, name: String) {
    if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == input)
        && let Some(o) = options.iter_mut().find(|o| o.id == option)
    {
        o.name = name;
    }
}

pub const DELETE_OPTION: &str = "Delete configuration";

/// Delete a configuration of a List input (never its last one); the
/// default moves to the first configuration when it was the deleted one.
pub fn delete_option(ps: &mut PartStudio, input: &str, option: &str) {
    if let Some(ConfigInput { kind: ConfigInputKind::List { options, default }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == input)
        && options.len() > 1
    {
        options.retain(|o| o.id != option);
        if *default == option {
            *default = options[0].id.clone();
        }
    }
}

pub const CONFIGURE_PARAMETER: &str = "Configure parameter";

/// Configure a feature parameter (or its suppression, [`SUPPRESSED_PARAM`])
/// by a List or Checkbox input: every configuration starts with the current
/// value. Returns false when the parameter or input doesn't allow it.
pub fn configure_parameter(ps: &mut PartStudio, feature: FeatureId, param: &str, input: &str) -> bool {
    let Some(f) = ps.feature(feature) else { return false };
    let current = if param == SUPPRESSED_PARAM { Some(ParamValue::Bool(f.suppressed)) } else { f.params.get(param).cloned() };
    let Some(current) = current else { return false };
    let keys: Vec<String> = match ps.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
        Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
        Some(ConfigInputKind::Checkbox { .. }) => vec!["true".into(), "false".into()],
        _ => return false,
    };
    let values = keys.into_iter().map(|k| (k, current.clone())).collect();
    ps.configuration.configured.push(ConfiguredParam { input: input.to_string(), feature, param: param.to_string(), values });
    true
}

pub const EDIT_CONFIGURED_VALUE: &str = "Edit configured value";

/// A configured parameter's value in one configuration (`key`: the option
/// id, or "true"/"false"), typed as text: enums and texts keep their kind,
/// anything else is an expression.
pub fn set_configured_text(ps: &mut PartStudio, index: usize, key: &str, text: String) {
    if let Some(cp) = ps.configuration.configured.get_mut(index) {
        let new = match cp.values.get(key) {
            Some(ParamValue::Enum(_)) => ParamValue::Enum(text.trim().to_string()),
            Some(ParamValue::Text(_)) => ParamValue::Text(text),
            _ => ParamValue::Expr(ExprValue::new(text)),
        };
        cp.values.insert(key.to_string(), new);
    }
}

/// A configured parameter's value in one configuration, as a value.
pub fn set_configured_value(ps: &mut PartStudio, index: usize, key: &str, value: ParamValue) {
    if let Some(cp) = ps.configuration.configured.get_mut(index) {
        cp.values.insert(key.to_string(), value);
    }
}

pub const REORDER_INPUTS: &str = "Reorder configuration inputs";

/// Drag input `from` onto input `to`'s place.
pub fn reorder_inputs(ps: &mut PartStudio, from: &str, to: &str) {
    let inputs = &mut ps.configuration.inputs;
    if let (Some(from), Some(to)) = (inputs.iter().position(|i| i.id == from), inputs.iter().position(|i| i.id == to))
        && from != to
    {
        let input = inputs.remove(from);
        inputs.insert(to, input);
    }
}

/// The unit dimension of a configuration variable kind (`None`: text).
pub fn kind_dim(kind: VariableKind) -> Option<noshape_units::Dim> {
    use noshape_units::Dim;
    match kind {
        VariableKind::Length => Some(Dim::LENGTH),
        VariableKind::Angle => Some(Dim::ANGLE),
        VariableKind::Integer | VariableKind::Real => Some(Dim::UNITLESS),
        VariableKind::Text => None,
    }
}

/// A configuration variable value outside its range: the message the
/// Configurations box shows under the field (the value isn't applied).
pub fn value_error(input: &ConfigInput, text: &str, units: &noshape_units::UnitPrefs) -> Option<String> {
    let ConfigInputKind::Variable { kind, min, max, .. } = &input.kind else { return None };
    let dim = kind_dim(*kind)?;
    let eval = |t: &str| noshape_units::evaluate_as(t, dim, units, &noshape_units::EmptyScope).ok().map(|q| q.value / units.si_factor(dim));
    let v = eval(text)?;
    let lo = min.as_ref().and_then(|m| eval(&m.text));
    let hi = max.as_ref().and_then(|m| eval(&m.text));
    let fmt = |x: f64| if x.fract() == 0.0 { format!("{x:.0}") } else { format!("{x}") };
    let out = lo.is_some_and(|l| v < l) || hi.is_some_and(|h| v > h);
    out.then(|| format!("Enter a value between {} and {}. Current value is {}.", lo.map(fmt).unwrap_or_default(), hi.map(fmt).unwrap_or_default(), fmt(v)))
}

/// The values a Part Studio evaluates: defaults overridden by the chosen
/// ones (inputs that no longer exist are dropped), as the Part Studio view
/// does.
pub fn current(ps: &PartStudio, chosen: &noshape_model::configuration::ConfigValues) -> noshape_model::configuration::ConfigValues {
    let mut v = ps.configuration.defaults();
    for (k, val) in &chosen.0 {
        if v.0.contains_key(k) {
            v.0.insert(k.clone(), val.clone());
        }
    }
    v
}
