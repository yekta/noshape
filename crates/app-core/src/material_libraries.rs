//! The material libraries added to the account (Onshape keeps them in account preferences; here a preferences
//! file), as the Part Studio's Material panel and Preferences › Material libraries list them.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AddedLibrary {
    pub path: PathBuf,
    pub tab: u64,
    pub name: String,
    pub document: String,
    pub owner: String,
}

fn prefs_path() -> PathBuf {
    crate::paths::config_dir().join("material_libraries.json")
}

/// The libraries added to the account, in the order they were added.
pub fn added() -> Vec<AddedLibrary> {
    std::fs::read(prefs_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn set_added(list: &[AddedLibrary]) {
    let _ = std::fs::write(prefs_path(), serde_json::to_vec_pretty(list).unwrap_or_default());
}

fn onshape_removed_path() -> PathBuf {
    crate::paths::config_dir().join("material_library_onshape_removed")
}

/// Whether the Onshape Material Library is listed (Preferences › Material
/// libraries can remove it too, live prefs/05; Add material library… →
/// Onshape brings it back).
pub fn onshape_listed() -> bool {
    !onshape_removed_path().exists()
}

pub fn set_onshape_listed(on: bool) {
    let p = onshape_removed_path();
    if on {
        let _ = std::fs::remove_file(p);
    } else {
        let _ = std::fs::write(p, b"");
    }
}
