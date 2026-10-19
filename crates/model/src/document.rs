//! Document structure: tabs (Part Studios, Assemblies, Drawings, Variable
//! Studios, Feature Studios, Material Libraries, files) and document-level settings. A document serializes to a set of
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
    Assembly(Box<Assembly>),
    Drawing(Drawing),
    VariableStudio(VariableStudio),
    /// Script source defining custom features (see [`crate::custom`]).
    FeatureStudio(crate::custom::FeatureStudio),
    /// Materials for Part Studios' Material dialog (CSV-backed).
    MaterialLibrary(crate::material_library::MaterialLibrary),
    /// An imported or stored file (see [`crate::file_tab`]).
    File(crate::file_tab::FileTab),
}

impl TabContent {
    pub fn kind_name(&self) -> &'static str {
        match self {
            TabContent::PartStudio(_) => "Part Studio",
            TabContent::Assembly(_) => "Assembly",
            TabContent::Drawing(_) => "Drawing",
            TabContent::VariableStudio(_) => "Variable Studio",
            TabContent::FeatureStudio(_) => "Feature Studio",
            TabContent::MaterialLibrary(_) => "Material Library",
            TabContent::File(_) => "File",
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
    /// Tab bar folders (Onshape "+" › Create folder). Folder ids share the
    /// tab id space and sit in `tab_order` like tabs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<TabFolder>,
    /// (tab or folder, its folder); absent: the root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<(TabId, TabId)>,
}

/// A folder of tabs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabFolder {
    pub id: TabId,
    pub name: String,
}

/// Onshape nests folders at most 10 levels below the root.
pub const MAX_FOLDER_DEPTH: usize = 10;

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
        d.add_tab("Assembly 1", TabContent::Assembly(Box::default()));
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
            // Files hold no references (and are large).
            .filter(|t| !matches!(t.content, TabContent::File(_)))
            .filter(|t| serde_json::to_string(&t.content).is_ok_and(|j| j.contains(&dec) || j.contains(&hex)))
            .map(|t| t.id)
            .collect()
    }

    pub fn remove_tab(&mut self, id: TabId) -> Option<Tab> {
        let i = self.tabs.iter().position(|t| t.id == id)?;
        self.settings.tab_order.retain(|t| *t != id);
        self.settings.parents.retain(|(t, _)| *t != id);
        Some(self.tabs.remove(i))
    }

    // ------------------------------------------------------------ tab folders

    pub fn folder(&self, id: TabId) -> Option<&TabFolder> {
        self.settings.folders.iter().find(|f| f.id == id)
    }

    pub fn is_folder(&self, id: TabId) -> bool {
        self.folder(id).is_some()
    }

    /// Next default folder name ("Folder 1").
    pub fn next_folder_name(&self) -> String {
        (1..).map(|n| format!("Folder {n}")).find(|n| !self.settings.folders.iter().any(|f| f.name == *n)).unwrap_or_default()
    }

    /// A new folder inside `parent` (None: the root), placed after `after`.
    pub fn add_folder(&mut self, name: &str, parent: Option<TabId>) -> TabId {
        let id = TabId::new_random();
        self.settings.folders.push(TabFolder { id, name: name.to_string() });
        self.settings.tab_order.push(id);
        self.set_parent(id, parent);
        id
    }

    pub fn rename_folder(&mut self, id: TabId, name: &str) {
        if let Some(f) = self.settings.folders.iter_mut().find(|f| f.id == id) {
            f.name = name.to_string();
        }
    }

    /// The folder a tab or folder is in (None: the root).
    pub fn parent_of(&self, id: TabId) -> Option<TabId> {
        self.settings.parents.iter().find(|(t, _)| *t == id).map(|(_, p)| *p).filter(|p| self.is_folder(*p))
    }

    /// The folders above `id`, innermost first.
    pub fn ancestors(&self, id: TabId) -> Vec<TabId> {
        let mut out = Vec::new();
        let mut at = self.parent_of(id);
        while let Some(p) = at {
            if out.contains(&p) {
                break;
            }
            out.push(p);
            at = self.parent_of(p);
        }
        out
    }

    /// Levels of folders below `id` (0: no subfolders).
    fn subfolder_depth(&self, id: TabId) -> usize {
        self.children(Some(id)).into_iter().filter(|c| self.is_folder(*c)).map(|c| 1 + self.subfolder_depth(c)).max().unwrap_or(0)
    }

    /// Move a tab or folder into `parent` (None: the root). Refused for a
    /// folder into itself or below itself, or deeper than
    /// [`MAX_FOLDER_DEPTH`]. Returns whether it moved.
    pub fn set_parent(&mut self, id: TabId, parent: Option<TabId>) -> bool {
        if let Some(p) = parent {
            if p == id || !self.is_folder(p) || self.ancestors(p).contains(&id) {
                return false;
            }
            let depth = self.ancestors(p).len() + 1 + if self.is_folder(id) { 1 + self.subfolder_depth(id) } else { 0 };
            if self.is_folder(id) && depth > MAX_FOLDER_DEPTH {
                return false;
            }
        }
        self.settings.parents.retain(|(t, _)| *t != id);
        if let Some(p) = parent {
            self.settings.parents.push((id, p));
        }
        true
    }

    /// Tabs and folders directly inside `folder` (None: the root), in order.
    pub fn children(&self, folder: Option<TabId>) -> Vec<TabId> {
        let mut ids: Vec<TabId> = self.settings.tab_order.iter().copied().filter(|id| self.tab(*id).is_some() || self.is_folder(*id)).collect();
        for t in &self.tabs {
            if !ids.contains(&t.id) {
                ids.push(t.id);
            }
        }
        ids.into_iter().filter(|id| self.parent_of(*id) == folder).collect()
    }

    /// Every tab inside `folder`, at any depth.
    pub fn tabs_in(&self, folder: TabId) -> Vec<TabId> {
        self.children(Some(folder)).into_iter().flat_map(|c| if self.is_folder(c) { self.tabs_in(c) } else { vec![c] }).collect()
    }

    /// Unpack folder: its contents move to its parent; the folder goes.
    pub fn unpack_folder(&mut self, id: TabId) {
        let parent = self.parent_of(id);
        for c in self.children(Some(id)) {
            self.settings.parents.retain(|(t, _)| *t != c);
            if let Some(p) = parent {
                self.settings.parents.push((c, p));
            }
        }
        self.settings.folders.retain(|f| f.id != id);
        self.settings.parents.retain(|(t, _)| *t != id);
        self.settings.tab_order.retain(|t| *t != id);
    }

    /// Remove a folder with every tab and folder inside it (Move to document).
    pub fn delete_folder_tree(&mut self, id: TabId) {
        for c in self.children(Some(id)) {
            if self.is_folder(c) {
                self.delete_folder_tree(c);
            } else {
                self.remove_tab(c);
            }
        }
        self.settings.folders.retain(|f| f.id != id);
        self.settings.parents.retain(|(t, _)| *t != id);
        self.settings.tab_order.retain(|t| *t != id);
    }

    /// Put `id` just before `before` in the tab order (tab bar dragging).
    pub fn move_in_order(&mut self, id: TabId, before: Option<TabId>) {
        let order = &mut self.settings.tab_order;
        order.retain(|t| *t != id);
        let at = before.and_then(|b| order.iter().position(|t| *t == b)).unwrap_or(order.len());
        order.insert(at, id);
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
                let mut tab = serde_json::from_slice::<Tab>(data)?;
                if let TabContent::PartStudio(ps) = &mut tab.content {
                    ps.upgrade_saved_params();
                }
                tabs.push(tab);
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

#[cfg(test)]
mod folder_tests {
    use super::*;

    #[test]
    fn folders_nest_unpack_and_keep_old_documents_loading() {
        let mut d = Document::new();
        let (ps, asm) = (d.tabs[0].id, d.tabs[1].id);
        let a = d.add_folder("A", None);
        assert!(d.set_parent(ps, Some(a)));
        assert_eq!(d.children(Some(a)), vec![ps]);
        assert_eq!(d.children(None), vec![asm, a]);
        let b = d.add_folder("B", Some(a));
        assert_eq!(d.ancestors(b), vec![a]);
        // No cycles.
        assert!(!d.set_parent(a, Some(b)));
        assert!(d.set_parent(asm, Some(b)));
        assert_eq!(d.tabs_in(a), vec![ps, asm]);
        d.unpack_folder(a);
        assert_eq!(d.parent_of(b), None);
        assert_eq!(d.parent_of(ps), None);
        assert!(d.folder(a).is_none());
        // Depth limit: 10 levels below the root.
        let mut at = None;
        for i in 0..MAX_FOLDER_DEPTH {
            at = Some(d.add_folder(&format!("F{i}"), at));
        }
        assert!(!d.set_parent(b, at));
        // Settings without folders (old documents) omit both fields and load.
        let json = serde_json::to_string(&DocumentSettings::default()).unwrap();
        assert!(!json.contains("folders") && !json.contains("parents"));
        let s: DocumentSettings = serde_json::from_str(&json).unwrap();
        assert!(s.folders.is_empty() && s.parents.is_empty());
    }
}
