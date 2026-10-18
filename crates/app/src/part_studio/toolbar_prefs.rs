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
    /// Group order: indices into the default toolbar groups.
    #[serde(default)]
    pub group_order: Vec<usize>,
    /// S shortcut toolbar tools (None = defaults).
    #[serde(default)]
    pub shortcut_tools: Option<Vec<String>>,
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

    /// Group indices in display order (missing groups appended).
    pub fn ordered_groups(&self, n: usize) -> Vec<usize> {
        let mut out: Vec<usize> = self.group_order.iter().copied().filter(|g| *g < n).collect();
        out.dedup();
        for g in 0..n {
            if !out.contains(&g) {
                out.push(g);
            }
        }
        out
    }

    /// Move a group one step left (-1) or right (+1).
    pub fn move_group(&mut self, n: usize, group: usize, delta: isize) {
        let mut order = self.ordered_groups(n);
        if let Some(i) = order.iter().position(|g| *g == group) {
            let j = (i as isize + delta).clamp(0, n as isize - 1) as usize;
            order.swap(i, j);
        }
        self.group_order = order;
    }

    pub fn shortcut_tools(&self) -> Vec<String> {
        self.shortcut_tools.clone().unwrap_or_else(|| DEFAULT_SHORTCUT_TOOLS.iter().map(|s| s.to_string()).collect())
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
        assert_eq!(p.ordered_groups(3), vec![0, 1, 2]);
        p.move_group(3, 2, -1);
        assert_eq!(p.ordered_groups(3), vec![0, 2, 1]);
        p.move_group(3, 0, -1);
        assert_eq!(p.ordered_groups(3), vec![0, 2, 1]);
        assert!(p.shortcut_tools().contains(&"extrude".to_string()));
        p.toggle_shortcut("extrude");
        assert!(!p.shortcut_tools().contains(&"extrude".to_string()));
        p.toggle_shortcut("helix");
        assert_eq!(p.shortcut_tools().last().map(|s| s.as_str()), Some("helix"));
    }
}
