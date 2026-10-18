//! Document structure: tabs (Part Studios, Assemblies, Drawings, Variable
//! Studios, Feature Studios) and document-level settings. A document serializes to a set of
//! named blobs (one per tab plus `document/meta`) that `noshape-store`
//! versions; see [`Document::to_blobs`].

use crate::assembly::Assembly;
use crate::drawing::Drawing;
use crate::partstudio::PartStudio;
use crate::variables::VariableStudio;
use noshape_units::UnitPrefs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TabId(pub u64);

impl TabId {
    pub fn new_random() -> Self {
        TabId(crate::reference::rand_u64())
    }
}

impl std::fmt::Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TabContent {
    PartStudio(PartStudio),
    Assembly(Assembly),
    Drawing(Drawing),
    VariableStudio(VariableStudio),
    /// Script source defining custom features (see [`crate::custom`]).
    FeatureStudio(crate::custom::FeatureStudio),
}

impl TabContent {
    pub fn kind_name(&self) -> &'static str {
        match self {
            TabContent::PartStudio(_) => "Part Studio",
            TabContent::Assembly(_) => "Assembly",
            TabContent::Drawing(_) => "Drawing",
            TabContent::VariableStudio(_) => "Variable Studio",
            TabContent::FeatureStudio(_) => "Feature Studio",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub name: String,
    pub content: TabContent,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DocumentSettings {
    pub units: UnitPrefs,
    /// Tab order (ids); tabs not listed are appended.
    pub tab_order: Vec<TabId>,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub settings: DocumentSettings,
    pub tabs: Vec<Tab>,
}

pub const META_BLOB: &str = "document/meta";

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// A new document with one empty Part Studio and one Assembly, like Onshape.
    pub fn new() -> Self {
        let mut d = Document { settings: DocumentSettings::default(), tabs: Vec::new() };
        d.add_tab("Part Studio 1", TabContent::PartStudio(PartStudio::new()));
        d.add_tab("Assembly 1", TabContent::Assembly(Assembly::default()));
        d
    }

    pub fn add_tab(&mut self, name: &str, content: TabContent) -> TabId {
        let id = TabId::new_random();
        self.tabs.push(Tab { id, name: name.to_string(), content });
        self.settings.tab_order.push(id);
        id
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Other tabs that use `tab` (assembly instances, drawings, derived
    /// parts): deleting it would break them, so Onshape asks first.
    /// Tab ids are random 64-bit numbers, so finding one in a tab's data
    /// (decimal, or the hex form derived features store) is reliable.
    pub fn tab_users(&self, tab: TabId) -> Vec<TabId> {
        let (dec, hex) = (tab.0.to_string(), tab.to_string());
        self.tabs
            .iter()
            .filter(|t| t.id != tab)
            .filter(|t| serde_json::to_string(&t.content).is_ok_and(|j| j.contains(&dec) || j.contains(&hex)))
            .map(|t| t.id)
            .collect()
    }

    pub fn remove_tab(&mut self, id: TabId) -> Option<Tab> {
        let i = self.tabs.iter().position(|t| t.id == id)?;
        self.settings.tab_order.retain(|t| *t != id);
        Some(self.tabs.remove(i))
    }

    /// Tabs in display order.
    pub fn ordered_tabs(&self) -> Vec<&Tab> {
        let mut out: Vec<&Tab> = self.settings.tab_order.iter().filter_map(|id| self.tab(*id)).collect();
        for t in &self.tabs {
            if !self.settings.tab_order.contains(&t.id) {
                out.push(t);
            }
        }
        out
    }

    /// Next default name for a new tab of a kind ("Part Studio 2").
    pub fn next_tab_name(&self, base: &str) -> String {
        let mut n = 1;
        loop {
            let name = format!("{base} {n}");
            if !self.tabs.iter().any(|t| t.name == name) {
                return name;
            }
            n += 1;
        }
    }

    pub fn blob_name(tab: TabId) -> String {
        format!("tab/{tab}")
    }

    /// Serialize into named blobs for the store.
    pub fn to_blobs(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        out.insert(META_BLOB.to_string(), serde_json::to_vec(&self.settings).expect("serialize settings"));
        for t in &self.tabs {
            out.insert(Self::blob_name(t.id), serde_json::to_vec(t).expect("serialize tab"));
        }
        out
    }

    /// Serialize one tab (for incremental commits).
    pub fn tab_blob(&self, id: TabId) -> Option<Vec<u8>> {
        self.tab(id).map(|t| serde_json::to_vec(t).expect("serialize tab"))
    }

    pub fn settings_blob(&self) -> Vec<u8> {
        serde_json::to_vec(&self.settings).expect("serialize settings")
    }

    pub fn from_blobs(blobs: &BTreeMap<String, Vec<u8>>) -> Result<Self, serde_json::Error> {
        let settings: DocumentSettings = match blobs.get(META_BLOB) {
            Some(b) => serde_json::from_slice(b)?,
            None => DocumentSettings::default(),
        };
        let mut tabs = Vec::new();
        for (name, data) in blobs {
            if name.starts_with("tab/") {
                tabs.push(serde_json::from_slice::<Tab>(data)?);
            }
        }
        Ok(Document { settings, tabs })
    }
}

#[cfg(test)]
mod tab_user_tests {
    use super::*;

    #[test]
    fn tabs_using_a_part_studio() {
        let mut doc = Document::default();
        let src = doc.add_tab("Source", TabContent::PartStudio(PartStudio::new()));
        let mut user = PartStudio::new();
        let mut params = crate::params::ParamValues::default();
        params.set("partStudio", crate::params::ParamValue::Text(src.to_string()));
        user.features.push(crate::partstudio::Feature { id: crate::reference::FeatureId(1), type_id: "importDerived".into(), name: "Derived 1".into(), suppressed: false, params, folder: None, custom_name: false, hidden: false, suppression_expr: None });
        let user = doc.add_tab("User", TabContent::PartStudio(user));
        let other = doc.add_tab("Other", TabContent::PartStudio(PartStudio::new()));
        assert_eq!(doc.tab_users(src), vec![user]);
        assert!(doc.tab_users(other).is_empty());
    }
}
