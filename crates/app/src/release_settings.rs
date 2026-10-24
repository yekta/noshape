//! Company settings › Release management, as saved (Help
//! enterprise_settings_release_management): revisions and part numbers,
//! workflows, access, actions, watermarks, release conditions, the Create
//! Release candidate dialog's requirements and obsoletion.

use noshape_model::release::RevisionScheme;
use serde::{Deserialize, Serialize};

/// A custom revision scheme ("New custom revision scheme…": a name and a
/// text file with one revision per line, at least 50).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomScheme {
    pub name: String,
    pub labels: Vec<String>,
}

/// Help: "The text file must list one revision per line and contain a
/// minimum of 50 lines."
pub const CUSTOM_SCHEME_MIN: usize = 50;

/// Watermarks › Edit watermarks: the text of each release state (at most
/// 64 characters, no Unicode; help image watermarks-dialog).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Watermarks {
    pub in_progress: String,
    pub pending: String,
    pub released: String,
    pub obsolete: String,
    pub rejected: String,
    pub discarded: String,
}

impl Default for Watermarks {
    fn default() -> Self {
        Self { in_progress: "In progress".into(), pending: "Pending".into(), released: String::new(), obsolete: "Obsolete".into(), rejected: "Rejected".into(), discarded: "Discarded".into() }
    }
}

impl Watermarks {
    /// The dialog's rows, in order.
    pub const LABELS: [&'static str; 6] = ["In progress", "Pending", "Released", "Obsolete", "Rejected", "Discarded"];
    pub const MAX_LEN: usize = 64;

    pub fn get(&self, i: usize) -> &str {
        [&self.in_progress, &self.pending, &self.released, &self.obsolete, &self.rejected, &self.discarded][i.min(5)]
    }

    pub fn set(&mut self, i: usize, text: String) {
        *[&mut self.in_progress, &mut self.pending, &mut self.released, &mut self.obsolete, &mut self.rejected, &mut self.discarded][i.min(5)] = text;
    }
}

/// Release conditions: "Selected conditions will prevent a Release from
/// being created" (help image release-conditions-01, in order).
pub const CONDITIONS: [&str; 8] = [
    "Prevent if a Part Studio feature list contains errors",
    "Prevent if a Part Studio feature list is not rolled to the end",
    "Prevent if an Assembly instance list contains errors",
    "Prevent if an Assembly has pending reference updates",
    "Prevent if a Drawing has a pending update",
    "Prevent if a part number is included in another pending Release candidate",
    "Prevent if an item has a pending task",
    "Prevent if an item references obsolete revisions",
];

/// Access (help image relmgmt-workflows-15, in order).
pub const ACCESS: [&str; 3] = [
    "Only administrators can edit the properties of released objects",
    "Only administrators can mark objects as not revision-managed",
    "Only administrators can delete documents containing released objects",
];

/// "Enter display names for Release & Obsoletion management properties":
/// the fields it renames, in order.
pub const DISPLAY_NAME_FIELDS: [&str; 4] = ["Release name", "Release notes", "Obsoletion names", "Obsoletion notes"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReleaseSettings {
    #[serde(default)]
    pub scheme: RevisionScheme,
    /// Saved custom revision schemes, selectable by name.
    #[serde(default)]
    pub custom_schemes: Vec<CustomScheme>,
    #[serde(default = "default_suffix")]
    pub unreleased_suffix: String,
    /// Workflows › Enable managed workflows.
    #[serde(default = "yes")]
    pub managed_workflows: bool,
    /// Access, as [`ACCESS`].
    #[serde(default)]
    pub access: [bool; 3],
    /// Actions › Require a new revision when an item's properties have changed.
    #[serde(default)]
    pub require_new_revision: bool,
    /// Watermarks › Show watermark for drawings.
    #[serde(default)]
    pub show_watermark: bool,
    /// Watermarks › Hide watermark for drawings that are not revision managed.
    #[serde(default)]
    pub hide_watermark_unmanaged: bool,
    #[serde(default)]
    pub watermarks: Watermarks,
    /// Release conditions, as [`CONDITIONS`].
    #[serde(default)]
    pub conditions: [bool; 8],
    /// Create Release candidate dialog.
    #[serde(default)]
    pub require_approver: bool,
    #[serde(default)]
    pub no_creator_approver: bool,
    #[serde(default)]
    pub require_all_approvers: bool,
    #[serde(default)]
    pub require_note: bool,
    #[serde(default = "yes")]
    pub allow_other_documents: bool,
    /// "Enter display names for Release & Obsoletion management properties"
    /// and the names, as [`DISPLAY_NAME_FIELDS`].
    #[serde(default)]
    pub custom_display_names: bool,
    #[serde(default = "default_display_names")]
    pub display_names: [String; 4],
    /// Obsoletion › Automatically obsolete previous released revisions.
    #[serde(default)]
    pub auto_obsolete: bool,
    /// Revisions and part numbers › Revision table template.
    #[serde(default)]
    pub revision_table: noshape_model::drawing::tables::RevisionTemplate,
}

fn yes() -> bool {
    true
}

fn default_suffix() -> String {
    "*".into()
}

fn default_display_names() -> [String; 4] {
    DISPLAY_NAME_FIELDS.map(String::from)
}

impl Default for ReleaseSettings {
    fn default() -> Self {
        Self {
            scheme: Default::default(),
            custom_schemes: Vec::new(),
            unreleased_suffix: default_suffix(),
            managed_workflows: true,
            access: [false; 3],
            require_new_revision: false,
            show_watermark: false,
            hide_watermark_unmanaged: false,
            watermarks: Watermarks::default(),
            conditions: [false; 8],
            require_approver: false,
            no_creator_approver: false,
            require_all_approvers: false,
            require_note: false,
            allow_other_documents: true,
            custom_display_names: false,
            display_names: default_display_names(),
            auto_obsolete: false,
            revision_table: Default::default(),
        }
    }
}

impl ReleaseSettings {
    /// A release-dialog field's label ([`DISPLAY_NAME_FIELDS`]): its display
    /// name when they're on.
    pub fn display_name(&self, field: usize) -> String {
        match self.display_names.get(field).filter(|n| self.custom_display_names && !n.trim().is_empty()) {
            Some(n) => n.clone(),
            None => DISPLAY_NAME_FIELDS[field.min(3)].to_string(),
        }
    }
}

/// The revision scheme's id ("Revision scheme id", used by custom release
/// workflows): 24 hex digits, stable for a scheme.
pub fn scheme_id(scheme: &RevisionScheme) -> String {
    let key = match scheme {
        RevisionScheme::Custom(labels) => format!("custom:{}", labels.join("\n")),
        s => s.label().to_string(),
    };
    // Two FNV-1a passes with different offsets: 96 bits.
    let fnv = |seed: u64| key.bytes().fold(seed, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let (a, b) = (fnv(0xcbf2_9ce4_8422_2325), fnv(0x84222325_cbf29ce4));
    format!("{:016x}{:08x}", a, b as u32)
}

fn settings_path() -> std::path::PathBuf {
    crate::files::config_dir().join("release_settings.json")
}

pub fn release_settings() -> ReleaseSettings {
    let s: ReleaseSettings = std::fs::read(settings_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    // Drawings' revision tables follow the company template.
    noshape_model::drawing::tables::set_revision_template(Some(s.revision_table.clone()));
    s
}

pub fn set_release_settings(s: &ReleaseSettings) {
    let _ = std::fs::write(settings_path(), serde_json::to_vec_pretty(s).unwrap_or_default());
    noshape_model::drawing::tables::set_revision_template(Some(s.revision_table.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_files_keep_their_values_and_get_the_defaults() {
        let s: ReleaseSettings = serde_json::from_str(r#"{"scheme":"Alphabetical","unreleased_suffix":"-"}"#).unwrap();
        assert_eq!(s.unreleased_suffix, "-");
        assert!(s.managed_workflows && s.allow_other_documents && !s.require_note);
        assert_eq!(s.watermarks.get(0), "In progress");
        assert_eq!(s.display_name(1), "Release notes");
        let named = ReleaseSettings { custom_display_names: true, display_names: ["ECO".into(), String::new(), String::new(), String::new()], ..s };
        assert_eq!((named.display_name(0), named.display_name(1)), ("ECO".to_string(), "Release notes".to_string()));
    }

    #[test]
    fn scheme_ids_are_stable_and_distinct() {
        let a = scheme_id(&RevisionScheme::Alphabetical);
        assert_eq!(a.len(), 24);
        assert_eq!(a, scheme_id(&RevisionScheme::Alphabetical));
        assert_ne!(a, scheme_id(&RevisionScheme::Numeric { width: 1 }));
    }
}
