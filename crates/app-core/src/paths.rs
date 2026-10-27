//! Where the app keeps its files.

use std::path::PathBuf;

/// The app's configuration directory (preferences, panel layouts, export rules).
pub fn config_dir() -> PathBuf {
    noshape_scene::config_dir()
}
