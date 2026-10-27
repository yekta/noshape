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
    // Live (qa5 configurations (1)): the first List is "Configuration", the
    // next ones "List input 1", "List input 2"…
    let names: Vec<&str> = ps.configuration.inputs.iter().map(|i| i.name.as_str()).collect();
    let name = if label != "List" {
        format!("{label} {n}")
    } else if !names.contains(&"Configuration") {
        "Configuration".to_string()
    } else {
        (1..).map(|k| format!("List input {k}")).find(|c| !names.contains(&c.as_str())).unwrap_or_default()
    };
    // Live (CONFIG-INPUT-ID): a List gets a random internal id ("List_8iFesnytxapQpg", as the Variable Studio's);
    // the others FeatureScript ids from their names (Edit FeatureScript IDs… shows them).
    let id = if label == "List" || !matches!(label, "Checkbox" | "Variable") {
        std::iter::repeat_with(crate::vs_config::list_id).find(|i| !taken.contains(i)).unwrap_or_default()
    } else {
        fs_identifier(&name, &taken)
    };
    ps.configuration.inputs.push(ConfigInput { id: id.clone(), name, kind, visible_when: None, excluded: false });
    id
}

/// Live's Checkbox create dialog (CONFIG-INPUT-ID, live 10-27): a required
/// Name, its FeatureScript id made from it ("My flag" → `My_flag`), and
/// Default (checked when the dialog opens). Returns its id.
pub fn add_checkbox_input(ps: &mut PartStudio, name: &str, default: bool) -> String {
    let taken: Vec<String> = ps.configuration.inputs.iter().map(|i| i.id.clone()).collect();
    let id = fs_identifier(name, &taken);
    ps.configuration.inputs.push(ConfigInput { id: id.clone(), name: name.to_string(), kind: ConfigInputKind::Checkbox { default }, visible_when: None, excluded: false });
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
    push_variable_input(ps, name.to_string(), vkind, ExprValue::new(value), range.map(|(lo, hi)| (ExprValue::new(lo), ExprValue::new(hi))))
}

/// The configuration Variable dialog's new input. Returns its id.
pub fn push_variable_input(ps: &mut PartStudio, name: String, kind: VariableKind, default: ExprValue, range: Option<(ExprValue, ExprValue)>) -> String {
    let (min, max) = range.map(|(a, b)| (Some(a), Some(b))).unwrap_or((None, None));
    let taken: Vec<String> = ps.configuration.inputs.iter().map(|i| i.id.clone()).collect();
    let id = fs_identifier(&name, &taken);
    ps.configuration.inputs.push(ConfigInput { id: id.clone(), name, kind: ConfigInputKind::Variable { kind, default, min, max }, visible_when: None, excluded: false });
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

/// The configuration Variable dialog's edit: default and (for a kind
/// with units) range.
pub fn edit_variable_input(ps: &mut PartStudio, input: &str, value: ExprValue, range: Option<(ExprValue, ExprValue)>) {
    if let Some(ConfigInput { kind: ConfigInputKind::Variable { default, min, max, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == input) {
        *default = value;
        if let Some((lo, hi)) = range {
            *min = Some(lo);
            *max = Some(hi);
        }
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

pub const UNCONFIGURE_PARAMETER: &str = "Unconfigure parameter";

/// A configured parameter's Unconfigure (the feature dialog's configure
/// mode, clicking a configured field again): its column goes; the feature
/// keeps its own value. False when it wasn't configured.
pub fn unconfigure_parameter(ps: &mut PartStudio, feature: FeatureId, param: &str) -> bool {
    let n = ps.configuration.configured.len();
    ps.configuration.configured.retain(|c| !(c.feature == feature && c.param == param));
    ps.configuration.configured.len() < n
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

// ------------------------------------------------------------ inputs between studios

pub const PASTE_INPUTS: &str = "Paste configuration inputs";

/// Paste inputs: the copied inputs whose ids this studio doesn't have yet;
/// visibility conditions needing missing inputs are repaired. Returns the
/// pasted inputs' names and whether conditions were repaired.
pub fn paste_inputs(ps: &mut PartStudio, copied: Vec<ConfigInput>) -> (Vec<String>, bool) {
    let mut pasted = Vec::new();
    for i in copied {
        if !ps.configuration.inputs.iter().any(|x| x.id == i.id) {
            pasted.push(i.name.clone());
            ps.configuration.inputs.push(i);
        }
    }
    let repaired = !ps.configuration.repair_visibility().is_empty();
    (pasted, repaired)
}

pub const INPUT_PROPAGATION: &str = "Input propagation";

/// Input propagation: the Derived features of `ps` whose source Part Studio
/// (in `doc`) has a List / Checkbox input with `input`'s id and kind, with
/// whether each is propagated already.
pub fn propagation_candidates(doc: &noshape_model::Document, ps: &PartStudio, input: &str) -> Vec<(FeatureId, String, bool)> {
    use noshape_model::regen::features::importing::DERIVED_CONFIG_PREFIX;
    let Some(own) = ps.configuration.inputs.iter().find(|i| i.id == input) else { return Vec::new() };
    if !matches!(own.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }) {
        return Vec::new();
    }
    let source_input = |tab: &str| -> Option<ConfigInput> {
        doc.tabs.iter().find(|t| t.id.to_string() == tab).and_then(|t| match &t.content {
            noshape_model::document::TabContent::PartStudio(s) => s.configuration.inputs.iter().find(|i| i.id == input).cloned(),
            _ => None,
        })
    };
    let param = format!("{DERIVED_CONFIG_PREFIX}{input}");
    ps.features
        .iter()
        .filter(|f| f.type_id == "importDerived")
        .filter(|f| source_input(f.params.text("partStudio")).is_some_and(|s| std::mem::discriminant(&s.kind) == std::mem::discriminant(&own.kind)))
        .map(|f| (f.id, f.name.clone(), ps.configuration.configured.iter().any(|c| c.feature == f.id && c.param == param)))
        .collect()
}

/// Propagate inputs to Derived features: each (input, feature) gets a
/// configured column mapping every option to the same option of the source.
pub fn propagate_inputs(ps: &mut PartStudio, picks: &[(String, FeatureId)]) {
    use noshape_model::regen::features::importing::DERIVED_CONFIG_PREFIX;
    for (input, f) in picks {
        let Some(i) = ps.configuration.inputs.iter().find(|i| &i.id == input) else { continue };
        let keys: Vec<String> = match &i.kind {
            ConfigInputKind::List { options, .. } => options.iter().map(|o| o.id.clone()).collect(),
            ConfigInputKind::Checkbox { .. } => vec!["true".into(), "false".into()],
            _ => continue,
        };
        let param = format!("{DERIVED_CONFIG_PREFIX}{input}");
        if ps.configuration.configured.iter().any(|c| c.feature == *f && c.param == param) {
            continue;
        }
        let values = keys.iter().map(|k| (k.clone(), ParamValue::Text(k.clone()))).collect();
        ps.configuration.configured.push(ConfiguredParam { input: input.clone(), feature: *f, param, values });
    }
}

pub const EDIT_IDS: &str = "Edit FeatureScript IDs";
pub const EDIT_VISIBILITY: &str = "Edit visibility conditions";

/// Visibility conditions… of one input (None: always shown).
pub fn set_visibility(ps: &mut PartStudio, input: &str, v: Option<noshape_model::configuration::Visibility>) {
    if let Some(i) = ps.configuration.inputs.iter_mut().find(|i| i.id == input) {
        i.visible_when = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CONFIG-INPUT-ID: a new List's internal id is live's random "List_" + 14 letters and digits (the Variable
    /// Studio's form), unique, not its name; a Checkbox keeps a FeatureScript id from its name.
    #[test]
    fn a_new_list_gets_a_random_internal_id() {
        let mut ps = PartStudio::new();
        let (a, b) = (add_input(&mut ps, "List"), add_input(&mut ps, "List"));
        for id in [&a, &b] {
            assert!(id.starts_with("List_") && id.len() == 19 && id[5..].chars().all(|c| c.is_ascii_alphanumeric()), "{id}");
        }
        assert_ne!(a, b);
        let c = add_input(&mut ps, "Checkbox");
        assert!(!c.starts_with("List_"), "{c}");
    }

    /// qa5 C1: the first List is "Configuration" (also after a variable
    /// input), then "List input 1", "List input 2".
    #[test]
    fn a_new_list_is_named_configuration() {
        let mut ps = PartStudio::new();
        add_variable_input(&mut ps, "d", "LENGTH", "10 mm", None);
        let id = add_input(&mut ps, "List");
        assert_eq!(ps.configuration.inputs.iter().find(|i| i.id == id).unwrap().name, "Configuration");
        let id2 = add_input(&mut ps, "List");
        assert_eq!(ps.configuration.inputs.iter().find(|i| i.id == id2).unwrap().name, "List input 1");
        let id3 = add_input(&mut ps, "List");
        assert_eq!(ps.configuration.inputs.iter().find(|i| i.id == id3).unwrap().name, "List input 2");
    }
}
