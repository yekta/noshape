//! Document-level edits (tabs, units), shared by the workspace window and
//! the agent API. Each function is what one UI command does; the caller
//! commits it with the matching description.

use noshape_model::Document;
use noshape_model::document::{TabContent, TabId};

/// Source of a new Feature Studio (Onshape's template).
/// A new Feature Studio: the two template lines, then empty lines 3 and 4 with the
/// caret on 4 (live, qa5 FS1: four lines, not five).
pub const FS_TEMPLATE: &str = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n\n";

/// Tab kinds of the "+" menu: (kind id, base name).
pub const TAB_KINDS: [(&str, &str); 6] = [
    ("partstudio", "Part Studio"),
    ("assembly", "Assembly"),
    ("drawing", "Drawing"),
    ("featurestudio", "Feature Studio"),
    ("materiallibrary", "Material Library"),
    ("variablestudio", "Variable Studio"),
];

/// Base name and empty content of a new tab of `kind` (see [`TAB_KINDS`]).
/// Also takes the display names `document_info` prints ("Part Studio",
/// any case or spacing; qa5 API2).
pub fn new_tab_content(kind: &str) -> Option<(&'static str, TabContent)> {
    let kind: String = kind.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    Some(match kind.as_str() {
        "partstudio" => ("Part Studio", TabContent::PartStudio(Default::default())),
        "assembly" => ("Assembly", TabContent::Assembly(Box::default())),
        "drawing" => ("Drawing", TabContent::Drawing(Default::default())),
        "featurestudio" => ("Feature Studio", TabContent::FeatureStudio(noshape_model::custom::FeatureStudio::new(FS_TEMPLATE))),
        "materiallibrary" => ("Material Library", TabContent::MaterialLibrary(Default::default())),
        "variablestudio" => ("Variable Studio", TabContent::VariableStudio(Default::default())),
        _ => return None,
    })
}

/// "Insert tab : Part Studio".
pub fn insert_tab_description(base: &str) -> String {
    format!("Insert tab : {base}")
}

/// "+" › new tab: the next default name ("Part Studio 2"), placed after
/// `after` (the active tab) and in its folder.
pub fn insert_tab(d: &mut Document, base: &str, content: TabContent, after: Option<TabId>) -> TabId {
    // Only the first Variable Studio is inserted into all elements.
    let content = match content {
        TabContent::VariableStudio(vs) => TabContent::VariableStudio(noshape_model::variables::VariableStudio { insert_into_all: noshape_model::variables::new_studio(d).insert_into_all, ..vs }),
        other => other,
    };
    let name = d.next_tab_name(base);
    let id = d.add_tab(&name, content);
    place_after(d, id, after);
    id
}

/// Put tab `id` right after `after` in the tab order, in the same folder.
pub fn place_after(d: &mut Document, id: TabId, after: Option<TabId>) {
    let order = &mut d.settings.tab_order;
    let Some(at) = after.and_then(|a| order.iter().position(|t| *t == a)) else { return };
    order.retain(|t| *t != id);
    order.insert(at + 1, id);
    // …and in the same folder.
    let parent = after.and_then(|a| d.parent_of(a));
    d.set_parent(id, parent);
}

/// "Rename tab : <name>".
pub fn rename_tab_description(name: &str) -> String {
    format!("Rename tab : {name}")
}

/// Tab › Rename (a folder's name when `tab` is a folder).
pub fn rename_tab(d: &mut Document, tab: TabId, name: &str) {
    if let Some(t) = d.tab_mut(tab) {
        t.name = name.to_string();
    } else {
        d.rename_folder(tab, name);
    }
}

/// "Duplicate tab : <name>".
pub fn duplicate_tab_description(name: &str) -> String {
    format!("Duplicate tab : {name}")
}

/// Tab › Duplicate: "<name> Copy 1" (the first free number; live qa5
/// TABS2) right after the original.
pub fn duplicate_tab(d: &mut Document, tab: TabId) -> Option<TabId> {
    let t = d.tab(tab)?.clone();
    let name = (1..).map(|n| format!("{} Copy {n}", t.name)).find(|n| !d.tabs.iter().any(|t| t.name == *n)).expect("a free number");
    let id = d.add_tab(&name, t.content);
    place_after(d, id, Some(tab));
    Some(id)
}

/// "Delete tab : <name>" (not undoable, as in Onshape).
pub fn delete_tab_description(name: &str) -> String {
    format!("Delete tab : {name}")
}

/// "Change workspace units" (document settings only).
pub const UNITS_DESCRIPTION: &str = "Change workspace units";

/// Create Drawing › OK: the drawing tab is named `name` (the next "Drawing
/// N" when empty or taken) and lands right after its source tab (Onshape).
pub fn insert_drawing_tab(d: &mut Document, name: &str, drawing: noshape_model::drawing::Drawing, source: Option<TabId>) -> TabId {
    let name = if name.is_empty() || d.tabs.iter().any(|t| t.name == name) { d.next_tab_name("Drawing") } else { name.to_string() };
    let id = d.add_tab(&name, TabContent::Drawing(drawing));
    if let Some(src) = source
        && let Some(at) = d.settings.tab_order.iter().position(|t| *t == src)
    {
        d.settings.tab_order.retain(|t| *t != id);
        d.settings.tab_order.insert(at + 1, id);
    }
    id
}

#[cfg(test)]
mod fs_template_tests {
    #[test]
    fn new_feature_studio_has_four_lines() {
        // Live (qa5 FS1): lines 1-2 template, empty lines 3-4, caret on 4.
        let lines: Vec<&str> = super::FS_TEMPLATE.split('\n').collect();
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert!(lines[2].is_empty() && lines[3].is_empty());
    }

    /// qa5 API2: the display names document_info prints work as kinds.
    #[test]
    fn tab_kind_accepts_display_names() {
        for (id, name) in super::TAB_KINDS {
            assert_eq!(super::new_tab_content(id).map(|c| c.0), Some(name));
            assert_eq!(super::new_tab_content(name).map(|c| c.0), Some(name));
            assert_eq!(super::new_tab_content(&name.to_uppercase()).map(|c| c.0), Some(name));
        }
        assert!(super::new_tab_content("sketch").is_none());
    }
}
