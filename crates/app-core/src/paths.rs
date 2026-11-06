//! Where the app keeps its files.

use std::path::PathBuf;

/// The app's configuration directory (preferences, panel layouts, export rules).
pub fn config_dir() -> PathBuf {
    noshape_scene::config_dir()
}

/// The app's cache directory (things it can rebuild: the display cache): `NOSHAPE_CACHE_DIR`, else the user's cache
/// directory's `noshape` (Linux ~/.cache/noshape, macOS ~/Library/Caches/noshape, Windows %LOCALAPPDATA%\noshape).
pub fn cache_dir() -> PathBuf {
    match std::env::var_os("NOSHAPE_CACHE_DIR") {
        Some(d) => PathBuf::from(d),
        None => dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("noshape"),
    }
}
