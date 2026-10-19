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
    /// Optional visibility condition: only shown when another (earlier)
    /// input has one of these values.
    #[serde(default)]
    pub visible_when: Option<(String, Vec<String>)>,
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

    /// Effective value string of an input (falls back to its default).
    pub fn value_of<'a>(&'a self, values: &'a ConfigValues, input: &str) -> Option<String> {
        if let Some(v) = values.0.get(input) {
            return Some(v.clone());
        }
        self.defaults().0.get(input).cloned()
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
                ConfigInput { id: "s".into(), name: "Size".into(), kind: ConfigInputKind::List { options: vec![ListOption { id: "a".into(), name: "Small".into() }, ListOption { id: "b".into(), name: "Large".into() }], default: "a".into() }, visible_when: None },
                ConfigInput { id: "c".into(), name: "Fillet".into(), kind: ConfigInputKind::Checkbox { default: false }, visible_when: None },
                ConfigInput { id: "t".into(), name: "Label".into(), kind: ConfigInputKind::Text { default: "x".into() }, visible_when: None },
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
            inputs: vec![ConfigInput { id: "s".into(), name: "Size".into(), kind: ConfigInputKind::List { options: vec![ListOption { id: "a".into(), name: "Small".into() }, ListOption { id: "b".into(), name: "Large".into() }], default: "a".into() }, visible_when: None }],
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
