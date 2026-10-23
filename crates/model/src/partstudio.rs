//! Part Studio: an ordered feature list producing parts, surfaces, curves,
//! sketches, planes and mate connectors.

use crate::configuration::Configuration;
use crate::features::{self, FeatureType};
use crate::params::{ParamValue, ParamValues, fill_defaults};
use crate::reference::{BodyId, FeatureId, Reference};
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
    /// Hidden in the graphics area (sketches, planes, mate connectors;
    /// Onshape's Hide/Show on a feature). Display only: not part of the
    /// regeneration key.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// Dynamic suppression: the feature is suppressed while this boolean
    /// expression (e.g. `#depth < 0.5 in`) is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression_expr: Option<String>,
}

impl Feature {
    pub fn feature_type(&self) -> Option<&'static FeatureType> {
        features::get(&self.type_id)
    }
}

/// Which rules a studio was saved under: 1, an empty merge scope is an error
/// unless "Merge with all" is on; 2, a Rib with no Parts is an error (live
/// L22); 3, a Mate connector needs an owner (live L37); 4, a Hole needs a
/// Merge scope (live L56); 0, older studios.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeRule(pub u8);

impl Default for ScopeRule {
    fn default() -> Self {
        ScopeRule(4)
    }
}

impl ScopeRule {
    fn legacy() -> Self {
        ScopeRule(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
    /// Enclosing folder (folders nest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    /// An empty folder's place: listed after this feature (None: first).
    /// Folders with features are placed by them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<FeatureId>,
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
    /// Revision (parts list properties).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Mass override, kg (Mass properties "Override").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_override: Option<f64>,
    /// Tessellation quality (Appearance dialog).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tessellation: Option<TessQuality>,
    /// The other Properties fields (State, Vendor, Title 1…).
    #[serde(default, skip_serializing_if = "ItemMeta::is_empty")]
    pub meta: ItemMeta,
}

/// Onshape's release states (Properties › State; "In progress" by default).
pub const RELEASE_STATES: [&str; 6] = ["In progress", "Pending", "Released", "Obsolete", "Rejected", "Discarded"];

/// Metadata fields of Onshape's Properties dialog that parts and
/// assemblies share (besides name, description, part number, revision).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemMeta {
    /// Release state, `None` = "In progress".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title3: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub not_revision_managed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclude_from_bom: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_of_measure: Option<String>,
}

impl ItemMeta {
    pub fn is_empty(&self) -> bool {
        *self == ItemMeta::default()
    }

    /// The release state shown in the instance list ("In progress").
    pub fn state(&self) -> &str {
        self.state.as_deref().unwrap_or(RELEASE_STATES[0])
    }
}

/// Onshape's tessellation quality setting of a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TessQuality {
    Auto,
    Coarse,
    Medium,
    Fine,
    VeryFine,
}

impl TessQuality {
    pub const ALL: [TessQuality; 5] = [TessQuality::Auto, TessQuality::Coarse, TessQuality::Medium, TessQuality::Fine, TessQuality::VeryFine];
    /// Segments Onshape draws a full circle with (live 459: the browser's
    /// mesh is angle based, the same at Ø20, Ø200 and Ø2000; Auto = Fine).
    pub fn segments_per_circle(self) -> u32 {
        match self {
            TessQuality::Auto | TessQuality::Fine => 144,
            TessQuality::Coarse => 24,
            TessQuality::Medium => 48,
            TessQuality::VeryFine => 600,
        }
    }
    /// Multiplier of Auto's angular deflection (smaller = finer).
    pub fn factor(self) -> f64 {
        f64::from(TessQuality::Auto.segments_per_circle()) / f64::from(self.segments_per_circle())
    }
    pub fn label(self) -> &'static str {
        match self {
            TessQuality::Auto => "Auto",
            TessQuality::Coarse => "Coarse",
            TessQuality::Medium => "Medium",
            TessQuality::Fine => "Fine",
            TessQuality::VeryFine => "Very fine",
        }
    }
}

/// What a face appearance colors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AppearanceTarget {
    /// Selected faces.
    Faces(Vec<Reference>),
    /// The faces a feature created (or a sketch's curves).
    Feature(FeatureId),
}

/// A face, feature or sketch appearance (Appearance panel).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceAppearance {
    pub target: AppearanceTarget,
    /// Linear RGBA.
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PartStudio {
    pub features: Vec<Feature>,
    /// Saved after empty merge scopes became an error, as in Onshape (see
    /// [`ScopeRule`]); older studios are upgraded on load.
    #[serde(default = "ScopeRule::legacy")]
    pub scope_rule: ScopeRule,
    /// Rollback bar position: number of features evaluated (None = all).
    #[serde(default)]
    pub rollback: Option<usize>,
    #[serde(default)]
    pub folders: Vec<Folder>,
    #[serde(default)]
    pub configuration: Configuration,
    /// Variable Studio tables in this Part Studio's Variable table.
    #[serde(default)]
    pub variable_studios: crate::variables::VariableStudioRefs,
    /// Stored as a list of pairs: JSON map keys must be strings.
    #[serde(default, with = "body_map")]
    pub part_properties: BTreeMap<BodyId, PartProperties>,
    /// Configured properties tables, one per part (Configurations panel ›
    /// Configured properties).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub property_tables: Vec<crate::configured_properties::PropertyTable>,
    /// Inspection panel annotations (datums…); not features.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<crate::annotations::Annotation>,
    /// Inspection panel: the applied default tolerances ("ISO 2768-c").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_tolerances: Option<String>,
    #[serde(default)]
    pub next_folder_id: u32,
    /// Saved camera views (Shift+V).
    #[serde(default)]
    pub named_views: Vec<NamedView>,
    /// Face, feature and sketch appearances.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub appearances: Vec<FaceAppearance>,
    /// Hidden default geometry (Origin, Top, Front, Right planes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_defaults: Vec<Reference>,
    /// Renamed default geometry (Onshape: Rename on Origin / Top / Front / Right).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_names: Vec<(Reference, String)>,
    /// Plane display sizes from "Size to Part Studio": (plane, half width, half height).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plane_sizes: Vec<(Reference, f64, f64)>,
    /// Assembly contexts (Edit in context snapshots, see
    /// [`crate::in_context`]) and the one shown (`None`: no context).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contexts: Vec<crate::in_context::AssemblyContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_context: Option<u32>,
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
    /// Section view planes (normal xyz, offset along it); empty = none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub section: Vec<[f64; 4]>,
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

    /// Upgrade feature parameters saved by older versions (run on load).
    pub fn upgrade_saved_params(&mut self) {
        for f in self.features.iter_mut().filter(|f| f.type_id == "cPlane") {
            crate::features::core::upgrade_plane_params(&mut f.params);
        }
        // Before Onshape's "No merge scope selected." rule an empty merge
        // scope merged with the parts the tool touched; "Merge with all"
        // gives those studios the same result.
        if self.scope_rule.0 < 1 {
            for f in &mut self.features {
                let adds = matches!(f.params.get("operationType"), Some(ParamValue::Enum(o)) if o == "ADD" || o == "REMOVE" || o == "INTERSECT");
                if adds && f.params.get("defaultScope").is_some() && !f.params.bool("defaultScope") && f.params.query("booleanScope").is_empty() {
                    f.params.set("defaultScope", ParamValue::Bool(true));
                }
            }
        }
        // Ribs without Parts used every part; they keep doing so.
        if self.scope_rule.0 < 2 {
            for f in self.features.iter_mut().filter(|f| f.type_id == "rib" && f.params.query("parts").is_empty()) {
                f.params.set("legacyAllParts", ParamValue::Bool(true));
            }
        }
        // Mate connectors without an owner worked before.
        if self.scope_rule.0 < 3 {
            for f in self.features.iter_mut().filter(|f| f.type_id == "mateConnector" && f.params.query("ownerPart").is_empty()) {
                f.params.set("legacyNoOwner", ParamValue::Bool(true));
            }
        }
        // Holes without a Merge scope cut every part they reached.
        if self.scope_rule.0 < 4 {
            for f in self.features.iter_mut().filter(|f| f.type_id == "hole" && f.params.query("scope").is_empty()) {
                f.params.set("legacyAllParts", ParamValue::Bool(true));
            }
        }
        self.scope_rule = ScopeRule::default();
    }

    /// Effective number of evaluated features (rollback bar position).
    pub fn rollback_index(&self) -> usize {
        self.rollback.unwrap_or(self.features.len()).min(self.features.len())
    }

    /// Default name for a new feature of `ty`: "Extrude 1", "Extrude 2"...
    /// (in the UI language, [`crate::default_names`]).
    pub fn next_name(&self, ty: &FeatureType) -> String {
        let base = crate::default_names::base(ty.name);
        let mut n = 1;
        loop {
            let name = format!("{base} {n}");
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
        if type_id == "hole" {
            features::hole_tables::init_new_hole(&mut params);
        }
        Some(Feature {
            id: FeatureId::new_random(),
            type_id: type_id.to_string(),
            name: self.next_name(ty),
            suppressed: false,
            params,
            folder: None,
            custom_name: false,
            hidden: false,
            suppression_expr: None,
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
            if copy.type_id == "cPlane" {
                crate::features::core::upgrade_plane_params(&mut copy.params);
            }
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
        // A folder made from features of one folder nests inside it.
        let first_folder = ids.first().and_then(|i| self.feature(*i)).and_then(|f| f.folder);
        let parent = first_folder.filter(|p| ids.iter().all(|i| self.feature(*i).is_some_and(|f| f.folder == Some(*p))));
        // Live (part-studio/587–590): a new folder is collapsed; an empty one
        // sits at the rollback bar.
        let after = if ids.is_empty() { self.rollback_index().checked_sub(1).and_then(|i| self.features.get(i)).map(|f| f.id) } else { None };
        self.folders.push(Folder { id, name, collapsed: !ids.is_empty(), parent, after });
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

    /// Features directly in a folder (the "(n)" after its name in the list).
    pub fn folder_count(&self, folder: u32) -> usize {
        self.features.iter().filter(|f| f.folder == Some(folder)).count()
    }

    /// Empty folders listed after feature `after` (None: before the first).
    pub fn empty_folders_after(&self, after: Option<FeatureId>) -> Vec<&Folder> {
        self.folders.iter().filter(|fd| fd.after == after && self.folder_count(fd.id) == 0 && !self.folders.iter().any(|c| c.parent == Some(fd.id))).collect()
    }

    /// Remove a folder (its features stay in place).
    pub fn dissolve_folder(&mut self, folder: u32) {
        // Contents (features and sub-folders) move up to the parent folder.
        let parent = self.folder(folder).and_then(|f| f.parent);
        self.folders.retain(|f| f.id != folder);
        for fd in &mut self.folders {
            if fd.parent == Some(folder) {
                fd.parent = parent;
            }
        }
        for f in &mut self.features {
            if f.folder == Some(folder) {
                f.folder = parent;
            }
        }
    }

    /// Folder chain of a folder id, outermost first.
    pub fn folder_chain(&self, id: Option<u32>) -> Vec<u32> {
        let mut out = Vec::new();
        let mut cur = id;
        while let Some(c) = cur {
            if out.contains(&c) {
                break;
            }
            out.push(c);
            cur = self.folder(c).and_then(|f| f.parent);
        }
        out.reverse();
        out
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
