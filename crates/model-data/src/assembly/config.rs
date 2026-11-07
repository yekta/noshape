//! Assembly configurations (right strip › Configuration panel, live
//! assembly/190–200, dom/assembly-configuration-*.txt): the same inputs as
//! a Part Studio's (List, Checkbox, Configuration variable) and a table of
//! configured assembly features: instance suppression ("Part 1 <2>" /
//! Unsuppressed) and mate parameters ("Revolute 1" / Unsuppressed, Offset,
//! offset values, flip). The configuration chosen above the instance list
//! (or for a subassembly instance) is applied before solving.

use super::{AsmMateConnector, Assembly, InstanceId, InstancePattern, Mate, MateId};
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
    /// A linear or circular pattern (live 2209: Configure suppression, Distance, Instance count, Angle).
    Pattern(MateId),
    /// An assembly mate connector (live 2209: Realign, Move, its translations and rotation, the flips).
    Connector(MateId),
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
    /// Pattern parameters: a linear pattern's Distance / Instance count (and its second direction's), a circular
    /// pattern's Angle / Instance count.
    pub const DISTANCE: &str = "distance";
    pub const COUNT: &str = "instanceCount";
    pub const DISTANCE_2: &str = "distance2";
    pub const COUNT_2: &str = "instanceCount2";
    pub const ANGLE: &str = "angle";
    /// Mate connector parameters (ROTATION and FLIP as a mate's).
    pub const REALIGN: &str = "realign";
    pub const MOVE: &str = "move";
    pub const TRANSLATION: [&str; 3] = ["translationX", "translationY", "translationZ"];
    pub const ROTATION_AXIS: &str = "rotationAxis";
    pub const REORIENT: &str = "reorientSecondary";
    /// An instance's own configuration input (`input:<id>`), set from the
    /// assembly's input (Input propagation; values `Text(option id)` /
    /// `Text("true" | "false")`).
    pub const INPUT_PREFIX: &str = "input:";
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
        // Live 2209's column titles.
        param::DISTANCE | param::DISTANCE_2 => "Distance",
        param::COUNT | param::COUNT_2 => "Instance count",
        param::ANGLE => "Angle",
        param::REALIGN => "Realign",
        param::MOVE => "Move",
        "translationX" => "X translation",
        "translationY" => "Y translation",
        "translationZ" => "Z translation",
        param::ROTATION_AXIS => "Rotation axis",
        param::REORIENT => "Reorient secondary axis",
        _ => "",
    }
}

/// Whether a configured parameter's cells are checkboxes.
pub fn param_is_bool(p: &str) -> bool {
    matches!(p, param::UNSUPPRESSED | param::OFFSET | param::FLIP | param::REALIGN | param::MOVE)
}

/// A select cell's options (live 2209: Rotation axis and Reorient secondary axis read "+X").
pub fn param_options(p: &str) -> Option<&'static [&'static str]> {
    match p {
        param::ROTATION_AXIS => Some(&ROTATION_AXES),
        param::REORIENT => Some(&REORIENTS),
        _ => None,
    }
}

const ROTATION_AXES: [&str; 3] = ["+X", "+Y", "+Z"];
const REORIENTS: [&str; 4] = ["+X", "+Y", "-X", "-Y"];

fn pattern_param(p: &InstancePattern, k: &str) -> Option<ParamValue> {
    use super::PatternKind;
    let count = |n: u32| ParamValue::Expr(ExprValue::new(n.to_string()));
    Some(match (&p.kind, k) {
        (_, param::UNSUPPRESSED) => ParamValue::Bool(!p.suppressed),
        (PatternKind::Linear { distance, .. }, param::DISTANCE) => ParamValue::Expr(distance.clone()),
        (PatternKind::Linear { count: n, .. } | PatternKind::Circular { count: n, .. }, param::COUNT) => count(*n),
        (PatternKind::Linear { second: Some(s), .. }, param::DISTANCE_2) => ParamValue::Expr(s.distance.clone()),
        (PatternKind::Linear { second: Some(s), .. }, param::COUNT_2) => count(s.count),
        (PatternKind::Circular { angle, .. }, param::ANGLE) => ParamValue::Expr(angle.clone()),
        _ => return None,
    })
}

/// Set a pattern parameter; true when its copies change (the count or spacing).
fn set_pattern_param(p: &mut InstancePattern, k: &str, v: &ParamValue) -> bool {
    use super::PatternKind;
    let count = |v: &ParamValue| match v {
        ParamValue::Expr(e) => e.text.trim().parse::<f64>().ok().filter(|n| *n >= 1.0).map(|n| n.round() as u32),
        _ => None,
    };
    match (&mut p.kind, k, v) {
        (_, param::UNSUPPRESSED, ParamValue::Bool(b)) => {
            p.suppressed = !b;
            false
        }
        (PatternKind::Linear { distance, .. }, param::DISTANCE, ParamValue::Expr(e)) | (PatternKind::Circular { angle: distance, .. }, param::ANGLE, ParamValue::Expr(e)) => {
            *distance = e.clone();
            false
        }
        (PatternKind::Linear { second: Some(s), .. }, param::DISTANCE_2, ParamValue::Expr(e)) => {
            s.distance = e.clone();
            false
        }
        (PatternKind::Linear { count: n, .. } | PatternKind::Circular { count: n, .. }, param::COUNT, v) => count(v).is_some_and(|c| std::mem::replace(n, c) != c),
        (PatternKind::Linear { second: Some(s), .. }, param::COUNT_2, v) => count(v).is_some_and(|c| std::mem::replace(&mut s.count, c) != c),
        _ => false,
    }
}

fn connector_param(c: &AsmMateConnector, k: &str) -> Option<ParamValue> {
    let e = |v: &Option<ExprValue>| ParamValue::Expr(v.clone().unwrap_or_else(|| ExprValue::new("0")));
    Some(match k {
        param::UNSUPPRESSED => ParamValue::Bool(!c.suppressed),
        param::REALIGN => ParamValue::Bool(c.primary_axis.is_some()),
        param::MOVE => ParamValue::Bool(!c.move_off),
        param::ROTATION => e(&c.rotation),
        param::FLIP => ParamValue::Bool(c.flip_primary),
        param::ROTATION_AXIS => ParamValue::Text(ROTATION_AXES.get(c.rotation_axis as usize).copied().unwrap_or("+Z").to_string()),
        param::REORIENT => ParamValue::Text(REORIENTS.get(c.reorient as usize).copied().unwrap_or("+X").to_string()),
        k => e(&c.translation[param::TRANSLATION.iter().position(|t| *t == k)?]),
    })
}

fn set_connector_param(c: &mut AsmMateConnector, k: &str, v: &ParamValue) {
    let expr = |v: &ParamValue| match v {
        ParamValue::Expr(e) => Some(e.clone()),
        _ => None,
    };
    let pick = |list: &[&str], v: &ParamValue| match v {
        ParamValue::Text(t) => list.iter().position(|x| x == t).map(|i| i as u8),
        _ => None,
    };
    match (k, v) {
        (param::UNSUPPRESSED, ParamValue::Bool(b)) => c.suppressed = !b,
        // Realign off drops the alignment entities; on keeps them (there is nothing to realign to otherwise).
        (param::REALIGN, ParamValue::Bool(false)) => {
            c.primary_axis = None;
            c.secondary_axis = None;
        }
        (param::MOVE, ParamValue::Bool(b)) => c.move_off = !b,
        (param::ROTATION, v) => c.rotation = expr(v),
        (param::FLIP, ParamValue::Bool(b)) => c.flip_primary = *b,
        (param::ROTATION_AXIS, v) => c.rotation_axis = pick(&ROTATION_AXES, v).unwrap_or(c.rotation_axis),
        (param::REORIENT, v) => c.reorient = pick(&REORIENTS, v).unwrap_or(c.reorient),
        (k, v) => {
            if let Some(i) = param::TRANSLATION.iter().position(|t| *t == k) {
                c.translation[i] = expr(v);
            }
        }
    }
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
        excluded: false,
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
            ConfigTarget::Pattern(id) => self.pattern(id).and_then(|x| pattern_param(x, p)),
            ConfigTarget::Connector(id) => self.connectors.iter().find(|c| c.id == id).and_then(|c| connector_param(c, p)),
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

    /// Whether `values` change anything (configured columns or configuration variables exist).
    pub fn has_configured(&self) -> bool {
        !self.configuration.configured.is_empty() || self.configuration.inputs.iter().any(|i| matches!(i.kind, ConfigInputKind::Variable { .. }))
    }

    /// The configuration variables' values (`#name` → quantity) in this copy's configuration, else the defaults.
    pub fn configuration_variables(&self, units: &noshape_units::UnitPrefs) -> std::collections::BTreeMap<String, noshape_units::Quantity> {
        let cfg = self.configuration.inputs_only();
        let values = self.active_config.clone().unwrap_or_else(|| cfg.defaults());
        cfg.variable_values(&values, units)
    }

    pub fn apply_configuration(&mut self, values: &ConfigValues) {
        self.active_config = Some(values.clone());
        let cfg = self.configuration.clone();
        let mut resync: Vec<MateId> = Vec::new();
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
                    // A propagated input: the instance's configuration follows.
                    if let (Some(input), ParamValue::Text(t)) = (c.param.strip_prefix(param::INPUT_PREFIX), v)
                        && let Some(cfg) = self.instance_mut(id).and_then(|i| i.source.config_mut())
                    {
                        cfg.0.insert(input.to_string(), t.clone());
                    }
                }
                ConfigTarget::Mate(id) => {
                    if let Some(m) = self.mates.iter_mut().find(|m| m.id == id) {
                        set_mate_param(m, &c.param, v);
                    }
                }
                ConfigTarget::Pattern(id) => {
                    if let Some(p) = self.patterns.iter_mut().find(|p| p.id == id)
                        && set_pattern_param(p, &c.param, v)
                        && !resync.contains(&id)
                    {
                        resync.push(id);
                    }
                }
                ConfigTarget::Connector(id) => {
                    if let Some(x) = self.connectors.iter_mut().find(|x| x.id == id) {
                        set_connector_param(x, &c.param, v);
                    }
                }
            }
        }
        // A configured count: this configuration's copies (live 2209: Long's 5 copies; no mates for the extra ones).
        for id in resync {
            self.sync_pattern(id);
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

    /// Accepting a pattern dialog in a configured assembly: configured values go to the active configuration's row;
    /// the pattern keeps its own (as [`Self::store_mate_edit`]).
    pub fn store_pattern_edit(&mut self, edited: &mut InstancePattern, values: &ConfigValues) {
        let Some(base) = self.pattern(edited.id).cloned() else { return };
        let target = ConfigTarget::Pattern(edited.id);
        let cfg = self.configuration.clone();
        for (i, c) in cfg.configured.iter().enumerate().filter(|(_, c)| c.target == target) {
            if let (Some(key), Some(v)) = (cfg.value_of(values, &c.input), pattern_param(edited, &c.param)) {
                self.configuration.configured[i].values.insert(key, v);
            }
            if let Some(own) = pattern_param(&base, &c.param) {
                set_pattern_param(edited, &c.param, &own);
            }
        }
    }

    /// The same for a mate connector's dialog.
    pub fn store_connector_edit(&mut self, edited: &mut AsmMateConnector, values: &ConfigValues) {
        let Some(base) = self.connectors.iter().find(|c| c.id == edited.id).cloned() else { return };
        let target = ConfigTarget::Connector(edited.id);
        let cfg = self.configuration.clone();
        for (i, c) in cfg.configured.iter().enumerate().filter(|(_, c)| c.target == target) {
            if let (Some(key), Some(v)) = (cfg.value_of(values, &c.input), connector_param(edited, &c.param)) {
                self.configuration.configured[i].values.insert(key, v);
            }
            match c.param.as_str() {
                // Realign's entities come back with the connector's own.
                param::REALIGN => {
                    edited.primary_axis = base.primary_axis.clone();
                    edited.secondary_axis = base.secondary_axis.clone();
                }
                p => {
                    if let Some(own) = connector_param(&base, p) {
                        set_connector_param(edited, p, &own);
                    }
                }
            }
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
        let patterns: Vec<MateId> = self.patterns.iter().map(|p| p.id).collect();
        let connectors: Vec<MateId> = self.connectors.iter().map(|c| c.id).collect();
        self.configuration.configured.retain(|c| match c.target {
            ConfigTarget::Instance(id) => instances.contains(&id),
            ConfigTarget::Mate(id) => mates.contains(&id),
            ConfigTarget::Pattern(id) => patterns.contains(&id),
            ConfigTarget::Connector(id) => connectors.contains(&id),
        });
    }
}
