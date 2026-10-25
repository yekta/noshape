//! User toolbar preferences (Onshape "Customize toolbar…" and the S
//! shortcut toolbar contents), stored as JSON in the config directory
//! (shared with the agent API: `noshape_ops::prefs`).

pub use noshape_ops::prefs::ToolbarPrefs;

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
