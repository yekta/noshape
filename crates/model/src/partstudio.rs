//! Part Studio: an ordered feature list producing parts, surfaces, curves,
//! sketches, planes and mate connectors.

use crate::configuration::Configuration;
use crate::features::{self, FeatureType};
use crate::params::{ParamValues, fill_defaults};
use crate::reference::{BodyId, FeatureId};
use noshape_units::UnitPrefs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeatureId,
    /// Registry id, e.g. "extrude".
    pub type_id: String,
    pub name: String,
    #[serde(default)]
    pub suppressed: bool,
    pub params: ParamValues,
    #[serde(default)]
    pub folder: Option<u32>,
    /// User has edited the name (otherwise it follows the default naming).
    #[serde(default)]
    pub custom_name: bool,
}

impl Feature {
    pub fn feature_type(&self) -> Option<&'static FeatureType> {
        features::get(&self.type_id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
}

/// Appearance of a part (linear RGBA).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PartProperties {
    pub name: Option<String>,
    pub appearance: Option<Appearance>,
    pub material: Option<String>,
    pub part_number: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    /// Density override, kg/m^3.
    pub density: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PartStudio {
    pub features: Vec<Feature>,
    /// Rollback bar position: number of features evaluated (None = all).
    #[serde(default)]
    pub rollback: Option<usize>,
    #[serde(default)]
    pub folders: Vec<Folder>,
    #[serde(default)]
    pub configuration: Configuration,
    #[serde(default)]
    pub part_properties: BTreeMap<BodyId, PartProperties>,
    #[serde(default)]
    pub next_folder_id: u32,
}

impl PartStudio {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn index_of(&self, id: FeatureId) -> Option<usize> {
        self.features.iter().position(|f| f.id == id)
    }

    pub fn feature(&self, id: FeatureId) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn feature_mut(&mut self, id: FeatureId) -> Option<&mut Feature> {
        self.features.iter_mut().find(|f| f.id == id)
    }

    /// Effective number of evaluated features (rollback bar position).
    pub fn rollback_index(&self) -> usize {
        self.rollback.unwrap_or(self.features.len()).min(self.features.len())
    }

    /// Default name for a new feature of `ty`: "Extrude 1", "Extrude 2"...
    pub fn next_name(&self, ty: &FeatureType) -> String {
        let mut n = 1;
        loop {
            let name = format!("{} {n}", ty.name);
            if !self.features.iter().any(|f| f.name == name) {
                return name;
            }
            n += 1;
        }
    }

    /// Create a feature with default parameters (not inserted).
    pub fn new_feature(&self, type_id: &str, units: &UnitPrefs) -> Option<Feature> {
        let ty = features::get(type_id)?;
        let mut params = ParamValues::default();
        fill_defaults(&ty.params, &mut params, units);
        Some(Feature {
            id: FeatureId::new_random(),
            type_id: type_id.to_string(),
            name: self.next_name(ty),
            suppressed: false,
            params,
            folder: None,
            custom_name: false,
        })
    }

    /// Insert at the rollback bar (Onshape inserts new features there) and
    /// advance the bar past it.
    pub fn insert_at_rollback(&mut self, f: Feature) -> usize {
        let at = self.rollback_index();
        self.features.insert(at, f);
        if self.rollback.is_some() {
            self.rollback = Some(at + 1);
        }
        if self.rollback == Some(self.features.len()) {
            self.rollback = None;
        }
        at
    }

    pub fn remove(&mut self, id: FeatureId) -> Option<Feature> {
        let i = self.index_of(id)?;
        if let Some(r) = self.rollback
            && i < r
        {
            self.rollback = Some(r - 1);
        }
        Some(self.features.remove(i))
    }

    /// Move a feature to a new index; returns false if invalid.
    pub fn reorder(&mut self, id: FeatureId, to: usize) -> bool {
        let Some(from) = self.index_of(id) else { return false };
        let to = to.min(self.features.len() - 1);
        let f = self.features.remove(from);
        self.features.insert(to, f);
        true
    }
}
