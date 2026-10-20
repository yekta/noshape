//! User toolbar preferences (Onshape "Customize toolbar…" and the S
//! shortcut toolbar contents), stored as JSON in the config directory.

use serde::{Deserialize, Serialize};

/// Default S shortcut toolbar (Part Studio, live: two rows of five).
pub const DEFAULT_SHORTCUT_TOOLS: &[&str] = &["sketch", "extrude", "revolve", "sweep", "loft", "fillet", "chamfer", "hole", "booleanBodies", "transform"];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolbarPrefs {
    /// Tool ids removed from the Part Studio toolbar.
    #[serde(default)]
    pub removed: Vec<String>,
    /// Tool set order: indices into the default toolbar groups, ⊞ last
    /// (Customize toolbar…; `toolbar_customize::full_order`).
    #[serde(default)]
    pub group_order: Vec<usize>,
    /// Right-click the toolbar › Hide "Search tools".
    #[serde(default)]
    pub hide_search: bool,
    /// S shortcut toolbar tools (None = defaults).
    #[serde(default)]
    pub shortcut_tools: Option<Vec<String>>,
    /// S shortcut toolbar tools in a sketch, by tool name (None = defaults;
    /// Preferences › Shortcut toolbars › Sketch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch_shortcut_tools: Option<Vec<String>>,
}

impl ToolbarPrefs {
    fn path() -> std::path::PathBuf {
        crate::files::config_dir().join("part_studio_toolbar.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, t);
        }
    }

    pub fn shortcut_tools(&self) -> Vec<String> {
        self.shortcut_tools.clone().unwrap_or_else(|| DEFAULT_SHORTCUT_TOOLS.iter().map(|s| s.to_string()).collect())
    }

    /// Sketch S toolbar tool names (`defaults`: Onshape's ten).
    pub fn sketch_shortcut_tools(&self, defaults: &[&str]) -> Vec<String> {
        self.sketch_shortcut_tools.clone().unwrap_or_else(|| defaults.iter().map(|s| s.to_string()).collect())
    }

    pub fn toggle_sketch_shortcut(&mut self, name: &str, defaults: &[&str]) {
        let mut tools = self.sketch_shortcut_tools(defaults);
        match tools.iter().position(|t| t == name) {
            Some(i) => {
                tools.remove(i);
            }
            None => tools.push(name.to_string()),
        }
        self.sketch_shortcut_tools = Some(tools);
    }

    pub fn toggle_shortcut(&mut self, id: &str) {
        let mut tools = self.shortcut_tools();
        match tools.iter().position(|t| t == id) {
            Some(i) => {
                tools.remove(i);
            }
            None => tools.push(id.to_string()),
        }
        self.shortcut_tools = Some(tools);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_order_and_shortcuts() {
        let mut p = ToolbarPrefs::default();
        assert_eq!(crate::toolbar_customize::full_order(&p.group_order, 3), vec![0, 1, 2]);
        p.group_order = vec![2, 0];
        assert_eq!(crate::toolbar_customize::full_order(&p.group_order, 3), vec![2, 0, 1]);
        p.group_order = vec![1, 1, 7];
        assert_eq!(crate::toolbar_customize::full_order(&p.group_order, 3), vec![1, 0, 2]);
        assert!(p.shortcut_tools().contains(&"extrude".to_string()));
        p.toggle_shortcut("extrude");
        assert!(!p.shortcut_tools().contains(&"extrude".to_string()));
        p.toggle_shortcut("helix");
        assert_eq!(p.shortcut_tools().last().map(|s| s.as_str()), Some("helix"));
        let defaults = ["Line", "Trim"];
        assert_eq!(p.sketch_shortcut_tools(&defaults), ["Line", "Trim"]);
        p.toggle_sketch_shortcut("Line", &defaults);
        p.toggle_sketch_shortcut("Slot", &defaults);
        assert_eq!(p.sketch_shortcut_tools(&defaults), ["Trim", "Slot"]);
    }
}
