//! The export dialogs' default file names through the user's export rules
//! (Preferences › User export rules; `noshape_model::export_rules`). With no
//! rules saved every dialog keeps its own default name.

use noshape_model::export_rules::{ExportElement, ExportRules, NameValues};

fn rules_path() -> std::path::PathBuf {
    crate::files::config_dir().join("export_rules.json")
}

/// The saved rules.
pub fn rules() -> ExportRules {
    std::fs::read(rules_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Save the rules (Preferences › User export rules).
pub fn set_rules(r: &ExportRules) {
    let _ = std::fs::write(rules_path(), serde_json::to_vec_pretty(r).unwrap_or_default());
}

/// The values a rule's template can use for an export of `name` (a part,
/// tab or instance) in tab `tab` of document `document`, with the part's
/// properties when it is one.
pub fn values(document: &str, tab: &str, name: &str, props: Option<&noshape_model::partstudio::PartProperties>) -> NameValues {
    // Live's property names (dom/preferences-export-rules-variables.json).
    let mut v = NameValues::new();
    v.insert("document.name".into(), document.to_string());
    v.insert("partStudio.name".into(), tab.to_string());
    v.insert("name".into(), name.to_string());
    if let Some(p) = props {
        for (k, val) in [("partNumber", &p.part_number), ("revision", &p.revision), ("description", &p.description), ("material", &p.material)] {
            if let Some(val) = val {
                v.insert(k.into(), val.clone());
            }
        }
    }
    v
}

/// The default file name (without extension) for exporting `element` as
/// `format` (Onshape's format label): the matching rule's, else `base`.
pub fn default_name(element: ExportElement, format: &str, values: &NameValues, base: String) -> String {
    let mut values = values.clone();
    values.insert("export.format".into(), format.to_string());
    rules().file_name(element, format, &values, &base)
}
