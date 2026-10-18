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
    /// Stored as a list of pairs: JSON map keys must be strings.
    #[serde(default, with = "body_map")]
    pub part_properties: BTreeMap<BodyId, PartProperties>,
    #[serde(default)]
    pub next_folder_id: u32,
    /// Saved camera views (Shift+V).
    #[serde(default)]
    pub named_views: Vec<NamedView>,
}

/// A saved camera (renderer-independent).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedView {
    pub name: String,
    pub target: [f64; 3],
    /// Camera rotation quaternion (x, y, z, w).
    pub rotation: [f64; 4],
    pub distance: f64,
    pub perspective: bool,
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
        let mut f = self.features.remove(from);
        // Joining the folder of the feature it is dropped onto keeps folders contiguous.
        f.folder = self.features.get(to).and_then(|n| n.folder);
        self.features.insert(to, f);
        true
    }

    /// Paste copies of `features` at `at`. Copies get new ids and names;
    /// references between copied features are remapped to the copies, while
    /// references to other geometry stay (Onshape copy/paste behavior).
    /// Returns the new ids.
    pub fn paste_features(&mut self, features: &[Feature], at: usize) -> Vec<FeatureId> {
        let map: std::collections::HashMap<u64, u64> = features.iter().map(|f| (f.id.0, FeatureId::new_random().0)).collect();
        let at = at.min(self.features.len());
        let mut out = Vec::new();
        for f in features {
            let mut v = match serde_json::to_value(f) {
                Ok(v) => v,
                Err(_) => continue,
            };
            remap_ids(&mut v, &map);
            let Ok(mut copy) = serde_json::from_value::<Feature>(v) else { continue };
            copy.folder = None;
            if let Some(ty) = copy.feature_type()
                && !copy.custom_name
            {
                copy.name = self.next_name(ty);
            } else {
                copy.name = format!("{} (copy)", f.name);
            }
            out.push(copy.id);
            self.features.insert(at + out.len() - 1, copy);
        }
        if let Some(r) = self.rollback
            && at <= r
        {
            self.rollback = Some(r + out.len());
        }
        if self.rollback == Some(self.features.len()) {
            self.rollback = None;
        }
        out
    }

    /// Put features into a new folder; returns its id.
    pub fn new_folder(&mut self, ids: &[FeatureId], name: Option<String>) -> u32 {
        self.next_folder_id = self.next_folder_id.max(self.folders.iter().map(|f| f.id + 1).max().unwrap_or(0));
        let id = self.next_folder_id;
        self.next_folder_id += 1;
        let name = name.unwrap_or_else(|| {
            let mut n = 1;
            while self.folders.iter().any(|f| f.name == format!("Folder {n}")) {
                n += 1;
            }
            format!("Folder {n}")
        });
        self.folders.push(Folder { id, name, collapsed: false });
        // Folders hold a contiguous run: gather members after the first one.
        let mut idx: Vec<usize> = ids.iter().filter_map(|i| self.index_of(*i)).collect();
        idx.sort_unstable();
        if let Some(&first) = idx.first() {
            let moved: Vec<Feature> = idx.iter().rev().map(|&i| self.features.remove(i)).collect::<Vec<_>>().into_iter().rev().collect();
            for (at, mut f) in (first..).zip(moved) {
                f.folder = Some(id);
                self.features.insert(at, f);
            }
        }
        id
    }

    /// Remove a folder (its features stay in place).
    pub fn dissolve_folder(&mut self, folder: u32) {
        self.folders.retain(|f| f.id != folder);
        for f in &mut self.features {
            if f.folder == Some(folder) {
                f.folder = None;
            }
        }
    }

    pub fn folder(&self, id: u32) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == id)
    }

    pub fn folder_mut(&mut self, id: u32) -> Option<&mut Folder> {
        self.folders.iter_mut().find(|f| f.id == id)
    }
}

/// Replace feature ids (serialized as integers) found anywhere in `v`.
fn remap_ids(v: &mut serde_json::Value, map: &std::collections::HashMap<u64, u64>) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(new) = n.as_u64().and_then(|u| map.get(&u)) {
                *n = (*new).into();
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| remap_ids(x, map)),
        serde_json::Value::Object(o) => o.values_mut().for_each(|x| remap_ids(x, map)),
        _ => {}
    }
}

/// Serde adapter for maps keyed by [`BodyId`] (serialized as `[[id, value], ...]`).
mod body_map {
    use super::{BTreeMap, BodyId};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<V: Serialize, S: Serializer>(m: &BTreeMap<BodyId, V>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(m.iter())
    }

    pub fn deserialize<'de, V: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<BodyId, V>, D::Error> {
        Ok(Vec::<(BodyId, V)>::deserialize(d)?.into_iter().collect())
    }
}
