//! Part Studio configurations: inputs (list, checkbox, configuration
//! variable, text) and the table of configured feature parameters.

use crate::params::{ParamValue, ParamValues};
use crate::partstudio::Feature;
use crate::reference::FeatureId;
use noshape_units::{ExprValue, Quantity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListOption {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableKind {
    Length,
    Angle,
    Integer,
    Real,
    Text,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConfigInputKind {
    List { options: Vec<ListOption>, default: String },
    Checkbox { default: bool },
    /// Exposed as `#name` in expressions.
    Variable { kind: VariableKind, default: ExprValue, min: Option<ExprValue>, max: Option<ExprValue> },
    Text { default: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigInput {
    pub id: String,
    pub name: String,
    pub kind: ConfigInputKind,
    /// Onshape's visibility conditions (Configuration visibility conditions
    /// dialog): when the input shows and, for lists, which options do.
    #[serde(default, deserialize_with = "de_visibility")]
    pub visible_when: Option<Visibility>,
    /// Input ⋯ › Exclude from properties (Help
    /// PartStudio__configured_properties): configured properties, the BOM
    /// and releases treat the input as its default.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub excluded: bool,
}

/// An input's visibility conditions. Only inputs above it (lists and
/// checkboxes) can control it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Visibility {
    /// "Show <input> When …" conditions, all of which must hold (AND);
    /// empty: the input is always shown.
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// List inputs: option rules. With none, every option shows; with some,
    /// an option shows only when a rule covering it applies ("Some options
    /// do not have an option rule. They will always be hidden.").
    #[serde(default)]
    pub option_rules: Vec<OptionRule>,
}

/// "When <input> is <values>" / "is between <a> and <b>".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub input: String,
    pub test: ConditionTest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConditionTest {
    /// One of these list options, or "true" / "false" for a checkbox.
    Is(Vec<String>),
    /// A list option between these two (inclusive, in list order).
    Between(String, String),
}

/// "Show <options | range> Always / When … And …".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptionRule {
    pub show: ShownOptions,
    pub always: bool,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShownOptions {
    /// A new rule before "options" or "range" is chosen.
    Unset,
    Options(Vec<String>),
    Range(String, String),
}

/// Documents saved before the full model stored `(input, allowed values)`.
fn de_visibility<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Visibility>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        New(Visibility),
        Legacy((String, Vec<String>)),
    }
    Ok(Option::<Any>::deserialize(d)?.map(|a| match a {
        Any::New(v) => v,
        Any::Legacy((input, values)) => Visibility { conditions: vec![Condition { input, test: ConditionTest::Is(values) }], option_rules: Vec::new() },
    }))
}

/// What the configuration shows for the chosen values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputVisibility {
    pub visible: bool,
    /// List inputs: the options that show (all of them without rules).
    pub options: Vec<String>,
}

/// FeatureScript's reserved words (Onshape's bundle: language keywords,
/// `const`/`var`, standard types, `true`/`false`), which ids can't be.
pub const FS_KEYWORDS: &[&str] = &[
    "annotation", "as", "assert", "break", "case", "catch", "const", "continue", "default", "do", "else", "enum", "export", "FeatureScript", "for", "function", "if", "import", "in", "inf", "is", "new", "operator", "precondition", "predicate",
    "return", "returns", "switch", "throw", "try", "type", "typecheck", "typeconvert", "var", "while", "array", "boolean", "box", "builtin", "map", "number", "string", "undefined", "true", "false",
];

/// A FeatureScript identifier for a new input or list row made from its
/// name ("List 2" → `List_2`), not a keyword and not in `taken`.
pub fn fs_identifier(name: &str, taken: &[String]) -> String {
    let mut id: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if id.is_empty() || id.starts_with(|c: char| c.is_ascii_digit()) {
        id.insert(0, '_');
    }
    if FS_KEYWORDS.contains(&id.as_str()) {
        id.push('_');
    }
    let base = id.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{base}_{n}");
        n += 1;
    }
    id
}

/// One configured feature parameter: its value for each list option or
/// checkbox state ("true"/"false").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfiguredParam {
    pub input: String,
    pub feature: FeatureId,
    /// Parameter id; `"__suppressed"` configures feature suppression.
    pub param: String,
    pub values: BTreeMap<String, ParamValue>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    pub inputs: Vec<ConfigInput>,
    pub configured: Vec<ConfiguredParam>,
}

/// Chosen configuration values, keyed by input id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConfigValues(pub BTreeMap<String, String>);

impl ConfigValues {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub const SUPPRESSED_PARAM: &str = "__suppressed";

impl Configuration {
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    /// Short description of the values that differ from the defaults, in
    /// input order (list option names, checked checkboxes by name,
    /// `name = value` otherwise), e.g. "Large, Checkbox 2"; empty for the
    /// default configuration. Used to tell configured instances apart (BOM).
    pub fn describe(&self, values: &ConfigValues) -> String {
        let defaults = self.defaults();
        let mut parts = Vec::new();
        for i in &self.inputs {
            let Some(v) = values.0.get(&i.id).filter(|v| defaults.0.get(&i.id) != Some(*v)) else { continue };
            parts.push(match &i.kind {
                ConfigInputKind::List { options, .. } => options.iter().find(|o| &o.id == v).map_or_else(|| v.clone(), |o| o.name.clone()),
                ConfigInputKind::Checkbox { .. } if v == "true" => i.name.clone(),
                ConfigInputKind::Checkbox { .. } => format!("no {}", i.name),
                _ => format!("{} = {v}", i.name),
            });
        }
        parts.join(", ")
    }

    /// Values with every input at its default.
    pub fn defaults(&self) -> ConfigValues {
        let mut v = ConfigValues::default();
        for i in &self.inputs {
            let s = match &i.kind {
                ConfigInputKind::List { default, .. } => default.clone(),
                ConfigInputKind::Checkbox { default } => default.to_string(),
                ConfigInputKind::Variable { default, .. } => default.text.clone(),
                ConfigInputKind::Text { default } => default.clone(),
            };
            v.0.insert(i.id.clone(), s);
        }
        v
    }

    /// Effective value string of an input: its chosen value or default,
    /// after the visibility conditions ([`Configuration::effective`]).
    pub fn value_of<'a>(&'a self, values: &'a ConfigValues, input: &str) -> Option<String> {
        if self.inputs.iter().all(|i| i.visible_when.is_none()) {
            if let Some(v) = values.0.get(input) {
                return Some(v.clone());
            }
            return self.defaults().0.get(input).cloned();
        }
        self.effective(values).0.get(input).cloned()
    }

    /// Option ids of a list input, in order.
    fn option_ids(&self, input: &str) -> Vec<String> {
        match self.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
            Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
            _ => Vec::new(),
        }
    }

    fn condition_holds(&self, c: &Condition, values: &ConfigValues) -> bool {
        let Some(v) = values.0.get(&c.input) else { return false };
        match &c.test {
            ConditionTest::Is(allowed) => allowed.contains(v),
            ConditionTest::Between(a, b) => {
                let ids = self.option_ids(&c.input);
                let at = |x: &str| ids.iter().position(|o| o == x);
                match (at(a), at(b), at(v)) {
                    (Some(a), Some(b), Some(v)) => a.min(b) <= v && v <= a.max(b),
                    _ => false,
                }
            }
        }
    }

    /// Visibility of every input (in order) for the chosen values, with the
    /// effective values: a hidden input takes its default; a list whose
    /// chosen option doesn't show takes its default, or the first option
    /// that shows; a list with no options showing is hidden (Help:
    /// PartStudio__visibility_conditions).
    pub fn visibility(&self, values: &ConfigValues) -> (Vec<InputVisibility>, ConfigValues) {
        let defaults = self.defaults();
        let mut eff = ConfigValues::default();
        let mut out = Vec::with_capacity(self.inputs.len());
        for i in &self.inputs {
            let chosen = values.0.get(&i.id).or_else(|| defaults.0.get(&i.id)).cloned().unwrap_or_default();
            let default = defaults.0.get(&i.id).cloned().unwrap_or_default();
            let vis = i.visible_when.as_ref();
            let mut visible = vis.is_none_or(|v| v.conditions.iter().all(|c| self.condition_holds(c, &eff)));
            let mut options = self.option_ids(&i.id);
            if let Some(v) = vis.filter(|v| !v.option_rules.is_empty()) {
                let ids = options.clone();
                options.retain(|o| {
                    v.option_rules.iter().any(|r| {
                        let covers = match &r.show {
                            ShownOptions::Unset => false,
                            ShownOptions::Options(list) => list.contains(o),
                            ShownOptions::Range(a, b) => {
                                let at = |x: &str| ids.iter().position(|y| y == x);
                                match (at(a), at(b), at(o)) {
                                    (Some(a), Some(b), Some(k)) => a.min(b) <= k && k <= a.max(b),
                                    _ => false,
                                }
                            }
                        };
                        covers && (r.always || r.conditions.iter().all(|c| self.condition_holds(c, &eff)))
                    })
                });
            }
            if matches!(i.kind, ConfigInputKind::List { .. }) && options.is_empty() {
                visible = false;
            }
            let value = if !visible {
                default
            } else if matches!(i.kind, ConfigInputKind::List { .. }) && !options.contains(&chosen) {
                if options.contains(&default) { default } else { options.first().cloned().unwrap_or(default) }
            } else {
                chosen
            };
            eff.0.insert(i.id.clone(), value);
            out.push(InputVisibility { visible, options });
        }
        (out, eff)
    }

    /// Values after the visibility conditions (see [`Configuration::visibility`]).
    pub fn effective(&self, values: &ConfigValues) -> ConfigValues {
        self.visibility(values).1
    }

    /// Edit FeatureScript IDs…: rename input `input` to `new_id` and, for a
    /// list, its options (`(old, new)` pairs), here and in everything of
    /// this configuration that refers to them (configured columns and their
    /// rows, visibility conditions, option rules, the default). References
    /// from other tabs break, as in Onshape.
    pub fn change_ids(&mut self, input: &str, new_id: &str, options: &[(String, String)]) {
        let opt = |o: &String| options.iter().find(|(a, _)| a == o).map(|(_, b)| b.clone()).unwrap_or_else(|| o.clone());
        let renames_options = |name: &str| name == input;
        for i in &mut self.inputs {
            if i.id == input {
                if let ConfigInputKind::List { options: opts, default } = &mut i.kind {
                    for o in opts.iter_mut() {
                        o.id = opt(&o.id);
                    }
                    *default = opt(default);
                    if let Some(v) = &mut i.visible_when {
                        for r in &mut v.option_rules {
                            r.show = match &r.show {
                                ShownOptions::Options(xs) => ShownOptions::Options(xs.iter().map(opt).collect()),
                                ShownOptions::Range(a, b) => ShownOptions::Range(opt(a), opt(b)),
                                ShownOptions::Unset => ShownOptions::Unset,
                            };
                        }
                    }
                }
                i.id = new_id.to_string();
            }
            if let Some(v) = &mut i.visible_when {
                let conds = v.conditions.iter_mut().chain(v.option_rules.iter_mut().flat_map(|r| r.conditions.iter_mut()));
                for c in conds.filter(|c| renames_options(&c.input)) {
                    c.input = new_id.to_string();
                    c.test = match &c.test {
                        ConditionTest::Is(xs) => ConditionTest::Is(xs.iter().map(opt).collect()),
                        ConditionTest::Between(a, b) => ConditionTest::Between(opt(a), opt(b)),
                    };
                }
            }
        }
        for c in self.configured.iter_mut().filter(|c| c.input == input) {
            c.input = new_id.to_string();
            c.values = std::mem::take(&mut c.values).into_iter().map(|(k, v)| (opt(&k), v)).collect();
        }
    }

    /// The values configured properties, the BOM and releases use: inputs
    /// excluded from properties take their defaults (or, when visibility
    /// conditions hide the default, the first shown value; Help
    /// PartStudio__configured_properties).
    pub fn for_properties(&self, values: &ConfigValues) -> ConfigValues {
        let mut v = self.effective(values);
        let defaults = self.defaults();
        for i in self.inputs.iter().filter(|i| i.excluded) {
            match defaults.0.get(&i.id) {
                Some(d) => v.0.insert(i.id.clone(), d.clone()),
                None => v.0.remove(&i.id),
            };
        }
        self.effective(&v)
    }

    /// Visibility conditions are repaired (removed) when the inputs or
    /// options they name are gone (Help: "Repairing visibility conditions").
    /// Returns the names of the inputs whose conditions changed.
    pub fn repair_visibility(&mut self) -> Vec<String> {
        let ids: Vec<(String, Vec<String>, bool)> = self.inputs.iter().map(|i| (i.id.clone(), self.option_ids(&i.id), matches!(i.kind, ConfigInputKind::Checkbox { .. }))).collect();
        let mut changed = Vec::new();
        for (k, input) in self.inputs.iter_mut().enumerate() {
            let Some(v) = &mut input.visible_when else { continue };
            let before = v.clone();
            // Controllers must exist above this input; tests keep known values.
            let above = &ids[..k];
            let fix = |c: &mut Condition| -> bool {
                let Some((_, opts, checkbox)) = above.iter().find(|(id, ..)| *id == c.input) else { return false };
                let known = |x: &String| if *checkbox { x == "true" || x == "false" } else { opts.contains(x) };
                match &mut c.test {
                    ConditionTest::Is(list) => {
                        list.retain(known);
                        !list.is_empty()
                    }
                    ConditionTest::Between(a, b) => known(a) && known(b),
                }
            };
            v.conditions.retain_mut(|c| fix(c));
            let own = &ids[k].1;
            v.option_rules.retain_mut(|r| {
                r.conditions.retain_mut(|c| fix(c));
                let shows = match &mut r.show {
                    ShownOptions::Unset => false,
                    ShownOptions::Options(list) => {
                        list.retain(|o| own.contains(o));
                        !list.is_empty()
                    }
                    ShownOptions::Range(a, b) => own.contains(a) && own.contains(b),
                };
                shows && (r.always || !r.conditions.is_empty())
            });
            if *v != before {
                changed.push(input.name.clone());
            }
            if v.conditions.is_empty() && v.option_rules.is_empty() {
                input.visible_when = None;
            }
        }
        changed
    }

    /// Apply configured parameters to a feature, returning effective params
    /// and suppression.
    pub fn apply(&self, feature: &Feature, values: &ConfigValues) -> (ParamValues, bool) {
        let mut params = feature.params.clone();
        let mut suppressed = feature.suppressed;
        for c in self.configured.iter().filter(|c| c.feature == feature.id) {
            let Some(key) = self.value_of(values, &c.input) else { continue };
            let Some(v) = c.values.get(&key) else { continue };
            if c.param == SUPPRESSED_PARAM {
                suppressed = matches!(v, ParamValue::Bool(true));
            } else {
                params.set(&c.param, v.clone());
            }
        }
        (params, suppressed)
    }

    /// A feature as its dialog shows it: configured parameters take the
    /// active configuration's values (Onshape edits those in the dialog).
    pub fn for_editing(&self, feature: &Feature, values: &ConfigValues) -> Feature {
        let mut f = feature.clone();
        f.params = self.apply(feature, values).0;
        f
    }

    /// Accepting a dialog: values of configured parameters go to the active
    /// configuration's row; the feature keeps its own (unconfigured) value.
    pub fn store_edit(&mut self, base: &Feature, edited: &mut Feature, values: &ConfigValues) {
        let keys: Vec<(usize, String)> = self
            .configured
            .iter()
            .enumerate()
            .filter(|(_, c)| c.feature == edited.id && c.param != SUPPRESSED_PARAM)
            .filter_map(|(i, c)| self.value_of(values, &c.input).map(|k| (i, k)))
            .collect();
        for (i, key) in keys {
            let c = &mut self.configured[i];
            if let Some(v) = edited.params.get(&c.param).cloned() {
                c.values.insert(key, v);
            }
            match base.params.get(&c.param) {
                Some(b) => edited.params.set(&c.param, b.clone()),
                None => {
                    edited.params.0.shift_remove(&c.param);
                }
            }
        }
    }

    /// Configuration variables as expression variables (name -> text).
    pub fn variable_exprs(&self, values: &ConfigValues) -> Vec<(String, VariableKind, String)> {
        self.inputs
            .iter()
            .filter_map(|i| match &i.kind {
                ConfigInputKind::Variable { kind, .. } => {
                    Some((i.name.clone(), *kind, self.value_of(values, &i.id).unwrap_or_default()))
                }
                _ => None,
            })
            .collect()
    }
}

/// Evaluated configuration variable.
#[derive(Clone, Debug)]
pub struct ConfigVariable {
    pub name: String,
    pub value: Quantity,
}

#[cfg(test)]
mod describe_tests {
    use super::*;

    #[test]
    fn describes_non_default_values() {
        let cfg = Configuration {
            inputs: vec![
                ConfigInput { id: "s".into(), name: "Size".into(), kind: ConfigInputKind::List { options: vec![ListOption { id: "a".into(), name: "Small".into() }, ListOption { id: "b".into(), name: "Large".into() }], default: "a".into() }, visible_when: None, excluded: false },
                ConfigInput { id: "c".into(), name: "Fillet".into(), kind: ConfigInputKind::Checkbox { default: false }, visible_when: None, excluded: false },
                ConfigInput { id: "t".into(), name: "Label".into(), kind: ConfigInputKind::Text { default: "x".into() }, visible_when: None, excluded: false },
            ],
            configured: vec![],
        };
        assert_eq!(cfg.describe(&cfg.defaults()), "");
        let mut v = cfg.defaults();
        v.0.insert("s".into(), "b".into());
        v.0.insert("c".into(), "true".into());
        v.0.insert("t".into(), "y".into());
        assert_eq!(cfg.describe(&v), "Large, Fillet, Label = y");
    }

    #[test]
    fn dialogs_edit_the_active_configuration() {
        let mut cfg = Configuration {
            inputs: vec![ConfigInput { id: "s".into(), name: "Size".into(), kind: ConfigInputKind::List { options: vec![ListOption { id: "a".into(), name: "Small".into() }, ListOption { id: "b".into(), name: "Large".into() }], default: "a".into() }, visible_when: None, excluded: false }],
            configured: vec![],
        };
        let e = |t: &str| ParamValue::Expr(ExprValue::new(t));
        let mut base = Feature { id: FeatureId(7), type_id: "extrude".into(), name: "Extrude 1".into(), suppressed: false, params: ParamValues::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None };
        base.params.set("depth", e("12 mm"));
        cfg.configured.push(ConfiguredParam { input: "s".into(), feature: FeatureId(7), param: "depth".into(), values: [("a".to_string(), e("12 mm")), ("b".to_string(), e("30 mm"))].into_iter().collect() });
        let mut large = cfg.defaults();
        large.0.insert("s".into(), "b".into());
        let mut shown = cfg.for_editing(&base, &large);
        assert_eq!(shown.params.get("depth"), Some(&e("30 mm")));
        shown.params.set("depth", e("35 mm"));
        cfg.store_edit(&base, &mut shown, &large);
        assert_eq!(cfg.configured[0].values.get("b"), Some(&e("35 mm")));
        assert_eq!(cfg.configured[0].values.get("a"), Some(&e("12 mm")));
        assert_eq!(shown.params.get("depth"), Some(&e("12 mm")));
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;

    fn list(id: &str, opts: &[&str], default: &str) -> ConfigInput {
        ConfigInput { id: id.into(), name: id.into(), kind: ConfigInputKind::List { options: opts.iter().map(|o| ListOption { id: (*o).into(), name: (*o).into() }).collect(), default: default.into() }, visible_when: None, excluded: false }
    }

    fn values(pairs: &[(&str, &str)]) -> ConfigValues {
        ConfigValues(pairs.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect())
    }

    /// Help example: Width only shows for the Tall height; hidden, it takes
    /// its default.
    #[test]
    fn input_condition_hides_and_defaults() {
        let mut width = list("Width", &["Default", "Wide"], "Default");
        width.visible_when = Some(Visibility { conditions: vec![Condition { input: "Height".into(), test: ConditionTest::Is(vec!["Tall".into()]) }], option_rules: Vec::new() });
        let cfg = Configuration { inputs: vec![list("Height", &["Short", "Default", "Tall"], "Default"), width], configured: Vec::new() };
        let (vis, eff) = cfg.visibility(&values(&[("Height", "Short"), ("Width", "Wide")]));
        assert!(!vis[1].visible);
        assert_eq!(eff.0["Width"], "Default");
        let (vis, eff) = cfg.visibility(&values(&[("Height", "Tall"), ("Width", "Wide")]));
        assert!(vis[1].visible);
        assert_eq!(eff.0["Width"], "Wide");
    }

    /// Edit FeatureScript IDs…: the input and option ids change everywhere
    /// in the configuration.
    #[test]
    fn change_ids_rewrites_references() {
        let mut width = list("Width", &["Default", "Wide"], "Default");
        width.visible_when = Some(Visibility { conditions: vec![Condition { input: "Height".into(), test: ConditionTest::Is(vec!["Tall".into()]) }], option_rules: Vec::new() });
        let mut cfg = Configuration { inputs: vec![list("Height", &["Short", "Tall"], "Short"), width], configured: Vec::new() };
        cfg.configured.push(ConfiguredParam { input: "Height".into(), feature: crate::FeatureId(1), param: "depth".into(), values: [("Short".to_string(), crate::params::ParamValue::Bool(true)), ("Tall".to_string(), crate::params::ParamValue::Bool(false))].into_iter().collect() });
        cfg.change_ids("Height", "height_id", &[("Tall".into(), "tall_id".into())]);
        assert_eq!(cfg.inputs[0].id, "height_id");
        assert!(matches!(&cfg.inputs[0].kind, ConfigInputKind::List { options, .. } if options[1].id == "tall_id"));
        let c = &cfg.inputs[1].visible_when.as_ref().unwrap().conditions[0];
        assert_eq!(c.input, "height_id");
        assert_eq!(c.test, ConditionTest::Is(vec!["tall_id".into()]));
        assert_eq!(cfg.configured[0].input, "height_id");
        assert!(cfg.configured[0].values.contains_key("tall_id"));
    }

    /// Assemblies evaluate their own configuration through the same rules.
    #[test]
    fn assembly_configuration_respects_visibility() {
        let mut width = list("Width", &["Default", "Wide"], "Default");
        width.visible_when = Some(Visibility { conditions: vec![Condition { input: "Height".into(), test: ConditionTest::Is(vec!["Tall".into()]) }], option_rules: Vec::new() });
        let asm = crate::assembly::config::AsmConfiguration { inputs: vec![list("Height", &["Short", "Default", "Tall"], "Default"), width], configured: Vec::new() };
        assert_eq!(asm.value_of(&values(&[("Height", "Short"), ("Width", "Wide")]), "Width").as_deref(), Some("Default"));
        assert_eq!(asm.value_of(&values(&[("Height", "Tall"), ("Width", "Wide")]), "Width").as_deref(), Some("Wide"));
    }

    /// Option rules: options without a rule are hidden; a range rule with a
    /// condition; a chosen option that doesn't show falls back.
    #[test]
    fn option_rules_filter_options() {
        let mut size = list("Size", &["A", "B", "C", "D"], "A");
        size.visible_when = Some(Visibility {
            conditions: Vec::new(),
            option_rules: vec![
                OptionRule { show: ShownOptions::Options(vec!["A".into()]), always: true, conditions: Vec::new() },
                OptionRule { show: ShownOptions::Range("B".into(), "C".into()), always: false, conditions: vec![Condition { input: "Big".into(), test: ConditionTest::Is(vec!["true".into()]) }] },
            ],
        });
        let big = ConfigInput { id: "Big".into(), name: "Big".into(), kind: ConfigInputKind::Checkbox { default: false }, visible_when: None, excluded: false };
        let cfg = Configuration { inputs: vec![big, size], configured: Vec::new() };
        let (vis, eff) = cfg.visibility(&values(&[("Big", "false"), ("Size", "C")]));
        assert_eq!(vis[1].options, vec!["A".to_string()]);
        assert_eq!(eff.0["Size"], "A");
        let (vis, eff) = cfg.visibility(&values(&[("Big", "true"), ("Size", "C")]));
        assert_eq!(vis[1].options, vec!["A".to_string(), "B".into(), "C".into()]);
        assert_eq!(eff.0["Size"], "C");
        assert_eq!(cfg.value_of(&values(&[("Big", "true"), ("Size", "D")]), "Size").as_deref(), Some("A"));
    }

    /// Deleting the controlling input removes the conditions; the legacy
    /// `(input, values)` form still loads.
    #[test]
    fn repair_and_legacy() {
        let mut width = list("Width", &["Default", "Wide"], "Default");
        width.visible_when = Some(Visibility { conditions: vec![Condition { input: "Height".into(), test: ConditionTest::Is(vec!["Tall".into()]) }], option_rules: Vec::new() });
        let mut cfg = Configuration { inputs: vec![list("Height", &["Tall"], "Tall"), width], configured: Vec::new() };
        cfg.inputs.remove(0);
        assert_eq!(cfg.repair_visibility(), vec!["Width".to_string()]);
        assert!(cfg.inputs[0].visible_when.is_none());
        let legacy = r#"{"id":"w","name":"W","kind":{"Checkbox":{"default":false}},"visible_when":["h",["Tall"]]}"#;
        let i: ConfigInput = serde_json::from_str(legacy).expect("legacy");
        assert_eq!(i.visible_when.map(|v| v.conditions.len()), Some(1));
    }
}
