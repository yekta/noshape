//! Assembly configurations (right strip › Configuration panel, live
//! assembly/190–200, dom/assembly-configuration-*.txt): the same inputs as
//! a Part Studio's (List, Checkbox, Configuration variable) and a table of
//! configured assembly features: instance suppression ("Part 1 <2>" /
//! Unsuppressed) and mate parameters ("Revolute 1" / Unsuppressed, Offset,
//! offset values, flip). The configuration chosen above the instance list
//! (or for a subassembly instance) is applied before solving.

use super::{Assembly, InstanceId, Mate, MateId};
use crate::configuration::{ConfigInput, ConfigInputKind, ConfigValues, Configuration, ListOption};
use crate::params::ParamValue;
use noshape_units::ExprValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The assembly feature a configured column belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigTarget {
    Instance(InstanceId),
    Mate(MateId),
}

/// Configurable parameters (column ids).
pub mod param {
    /// "Unsuppressed" checkbox (instances and mates): `Bool(true)` = unsuppressed.
    pub const UNSUPPRESSED: &str = "unsuppressed";
    /// Mate "Offset" checkbox.
    pub const OFFSET: &str = "offset";
    /// Mate offset values (expressions).
    pub const OFFSET_Z: &str = "offsetZ";
    pub const OFFSET_X: &str = "offsetX";
    pub const OFFSET_Y: &str = "offsetY";
    pub const ROTATION: &str = "rotation";
    /// Mate "Flip primary axis".
    pub const FLIP: &str = "flipPrimary";
}

/// Column header label of a configured parameter (live 196: "Unsuppressed",
/// "Offset").
pub fn param_label(p: &str) -> &'static str {
    match p {
        param::UNSUPPRESSED => "Unsuppressed",
        param::OFFSET => "Offset",
        param::OFFSET_Z => "Offset distance",
        param::OFFSET_X => "Offset X",
        param::OFFSET_Y => "Offset Y",
        param::ROTATION => "Rotation angle",
        param::FLIP => "Flip primary axis",
        _ => "",
    }
}

/// Whether a configured parameter's cells are checkboxes.
pub fn param_is_bool(p: &str) -> bool {
    matches!(p, param::UNSUPPRESSED | param::OFFSET | param::FLIP)
}

/// One configured parameter of an assembly feature: its value per list
/// option (or checkbox state "true" / "false").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsmConfiguredParam {
    pub input: String,
    pub target: ConfigTarget,
    pub param: String,
    pub values: BTreeMap<String, ParamValue>,
}

/// The assembly's configuration inputs and configured parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AsmConfiguration {
    pub inputs: Vec<ConfigInput>,
    #[serde(default)]
    pub configured: Vec<AsmConfiguredParam>,
}

impl AsmConfiguration {
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    /// The inputs as a Part Studio style configuration (defaults, describe).
    pub fn inputs_only(&self) -> Configuration {
        Configuration { inputs: self.inputs.clone(), configured: Vec::new() }
    }

    pub fn defaults(&self) -> ConfigValues {
        self.inputs_only().defaults()
    }

    pub fn value_of(&self, values: &ConfigValues, input: &str) -> Option<String> {
        self.inputs_only().value_of(values, input)
    }

    /// The row keys of an input: list option ids, or "true" / "false".
    pub fn row_keys(&self, input: &str) -> Vec<String> {
        match self.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
            Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
            Some(ConfigInputKind::Checkbox { .. }) => vec!["true".into(), "false".into()],
            _ => Vec::new(),
        }
    }

    /// Configure parameters list: (target, param) pairs, in column order.
    pub fn columns(&self, input: &str) -> Vec<(ConfigTarget, String)> {
        let mut out: Vec<(ConfigTarget, String)> = Vec::new();
        for c in self.configured.iter().filter(|c| c.input == input) {
            out.push((c.target, c.param.clone()));
        }
        out
    }
}

/// A new List input (live 191: "Configuration" with the one "Default" row).
pub fn new_list_input(id: String, n: usize) -> ConfigInput {
    ConfigInput {
        id,
        name: if n == 1 { "Configuration".into() } else { format!("List {n}") },
        kind: ConfigInputKind::List { options: vec![ListOption { id: "default".into(), name: "Default".into() }], default: "default".into() },
        visible_when: None,
    }
}

fn mate_param(m: &Mate, p: &str) -> Option<ParamValue> {
    let e = |v: &Option<ExprValue>| ParamValue::Expr(v.clone().unwrap_or_else(|| ExprValue::new("0")));
    Some(match p {
        param::UNSUPPRESSED => ParamValue::Bool(!m.suppressed),
        param::OFFSET => ParamValue::Bool(m.offset_z.is_some() || m.offset_x.is_some() || m.offset_y.is_some() || m.rotation.is_some()),
        param::OFFSET_Z => e(&m.offset_z),
        param::OFFSET_X => e(&m.offset_x),
        param::OFFSET_Y => e(&m.offset_y),
        param::ROTATION => e(&m.rotation),
        param::FLIP => ParamValue::Bool(m.flip_primary),
        _ => return None,
    })
}

fn set_mate_param(m: &mut Mate, p: &str, v: &ParamValue) {
    let expr = |v: &ParamValue| match v {
        ParamValue::Expr(e) => Some(e.clone()),
        _ => None,
    };
    match (p, v) {
        (param::UNSUPPRESSED, ParamValue::Bool(b)) => m.suppressed = !b,
        (param::OFFSET, ParamValue::Bool(false)) => {
            m.offset_z = None;
            m.offset_x = None;
            m.offset_y = None;
            m.rotation = None;
        }
        (param::OFFSET, ParamValue::Bool(true)) => {
            if m.offset_z.is_none() && m.offset_x.is_none() && m.offset_y.is_none() && m.rotation.is_none() {
                m.offset_z = Some(ExprValue::new("0 mm"));
            }
        }
        (param::OFFSET_Z, v) => m.offset_z = expr(v),
        (param::OFFSET_X, v) => m.offset_x = expr(v),
        (param::OFFSET_Y, v) => m.offset_y = expr(v),
        (param::ROTATION, v) => m.rotation = expr(v),
        (param::FLIP, ParamValue::Bool(b)) => m.flip_primary = *b,
        _ => {}
    }
}

/// Put the mate's own value of a configured parameter back.
fn restore_mate_param(m: &mut Mate, base: &Mate, p: &str) {
    match p {
        param::UNSUPPRESSED => m.suppressed = base.suppressed,
        param::OFFSET => {
            m.offset_z = base.offset_z.clone();
            m.offset_x = base.offset_x.clone();
            m.offset_y = base.offset_y.clone();
            m.rotation = base.rotation.clone();
        }
        param::OFFSET_Z => m.offset_z = base.offset_z.clone(),
        param::OFFSET_X => m.offset_x = base.offset_x.clone(),
        param::OFFSET_Y => m.offset_y = base.offset_y.clone(),
        param::ROTATION => m.rotation = base.rotation.clone(),
        param::FLIP => m.flip_primary = base.flip_primary,
        _ => {}
    }
}

impl Assembly {
    /// The current (unconfigured) value of a configurable parameter.
    pub fn config_param_value(&self, target: ConfigTarget, p: &str) -> Option<ParamValue> {
        match target {
            ConfigTarget::Instance(id) => match p {
                param::UNSUPPRESSED => self.instance(id).map(|i| ParamValue::Bool(!i.suppressed)),
                _ => None,
            },
            ConfigTarget::Mate(id) => self.mate(id).and_then(|m| mate_param(m, p)),
        }
    }

    /// Add a configured column for `target`'s `p` to `input`, every row
    /// starting at the current value (live 195–196, 199). False when it is
    /// already configured or not configurable.
    pub fn configure_param(&mut self, input: &str, target: ConfigTarget, p: &str) -> bool {
        if self.configuration.configured.iter().any(|c| c.target == target && c.param == p) {
            return false;
        }
        let Some(v) = self.config_param_value(target, p) else { return false };
        let values = self.configuration.row_keys(input).into_iter().map(|k| (k, v.clone())).collect();
        self.configuration.configured.push(AsmConfiguredParam { input: input.to_string(), target, param: p.to_string(), values });
        true
    }

    /// The assembly as configuration `values` makes it: configured
    /// suppression and mate parameters applied.
    pub fn configured(&self, values: &ConfigValues) -> Assembly {
        let mut out = self.clone();
        out.apply_configuration(values);
        out
    }

    /// Whether `values` change anything (configured columns exist).
    pub fn has_configured(&self) -> bool {
        !self.configuration.configured.is_empty()
    }

    pub fn apply_configuration(&mut self, values: &ConfigValues) {
        let cfg = self.configuration.clone();
        for c in &cfg.configured {
            let Some(key) = cfg.value_of(values, &c.input) else { continue };
            let Some(v) = c.values.get(&key) else { continue };
            match c.target {
                ConfigTarget::Instance(id) => {
                    if let (param::UNSUPPRESSED, ParamValue::Bool(b)) = (c.param.as_str(), v)
                        && let Some(i) = self.instance_mut(id)
                    {
                        i.suppressed = !b;
                    }
                }
                ConfigTarget::Mate(id) => {
                    if let Some(m) = self.mates.iter_mut().find(|m| m.id == id) {
                        set_mate_param(m, &c.param, v);
                    }
                }
            }
        }
    }

    /// Accepting a mate dialog in a configured assembly: configured values
    /// go to the active configuration's row; the mate keeps its own.
    pub fn store_mate_edit(&mut self, edited: &mut Mate, values: &ConfigValues) {
        let Some(base) = self.mate(edited.id).cloned() else { return };
        let cfg = self.configuration.clone();
        let mine: Vec<(usize, &AsmConfiguredParam)> = cfg.configured.iter().enumerate().filter(|(_, c)| c.target == ConfigTarget::Mate(edited.id)).collect();
        // Read every configured value first (restoring one, e.g. Offset,
        // changes others).
        for (i, c) in &mine {
            if let (Some(key), Some(v)) = (cfg.value_of(values, &c.input), mate_param(edited, &c.param)) {
                self.configuration.configured[*i].values.insert(key, v);
            }
        }
        for (_, c) in &mine {
            restore_mate_param(edited, &base, &c.param);
        }
    }

    /// A change to a configured parameter (e.g. Suppress on a configured
    /// instance) goes to the active configuration's row. False when the
    /// parameter is not configured (the caller changes the feature itself).
    pub fn set_configured_value(&mut self, target: ConfigTarget, p: &str, values: &ConfigValues, v: ParamValue) -> bool {
        let cfg = self.configuration.clone();
        let Some(i) = cfg.configured.iter().position(|c| c.target == target && c.param == p) else { return false };
        let Some(key) = cfg.value_of(values, &cfg.configured[i].input) else { return false };
        self.configuration.configured[i].values.insert(key, v);
        true
    }

    /// Set one configured cell.
    pub fn set_config_cell(&mut self, column: usize, key: &str, v: ParamValue) {
        if let Some(c) = self.configuration.configured.get_mut(column) {
            c.values.insert(key.to_string(), v);
        }
    }

    /// A new configuration row (list option) copies the default row's
    /// values in every configured column.
    pub fn add_config_row(&mut self, input: &str, name: &str, id: String) {
        let Some(i) = self.configuration.inputs.iter_mut().find(|i| i.id == input) else { return };
        let ConfigInputKind::List { options, default } = &mut i.kind else { return };
        let default = default.clone();
        options.push(ListOption { id: id.clone(), name: name.to_string() });
        for c in self.configuration.configured.iter_mut().filter(|c| c.input == input) {
            if let Some(v) = c.values.get(&default).cloned() {
                c.values.insert(id.clone(), v);
            }
        }
    }

    /// Input ⋯ › Delete: the input and its configured columns go at once
    /// (no confirmation, live 200).
    pub fn delete_config_input(&mut self, input: &str) {
        self.configuration.inputs.retain(|i| i.id != input);
        self.configuration.configured.retain(|c| c.input != input);
    }

    /// Instances / mates deleted: their configured columns go too.
    pub fn prune_configured(&mut self) {
        let instances: Vec<InstanceId> = self.instances.iter().map(|i| i.id).collect();
        let mates: Vec<MateId> = self.mates.iter().map(|m| m.id).collect();
        self.configuration.configured.retain(|c| match c.target {
            ConfigTarget::Instance(id) => instances.contains(&id),
            ConfigTarget::Mate(id) => mates.contains(&id),
        });
    }
}
