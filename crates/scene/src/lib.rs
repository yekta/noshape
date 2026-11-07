//! noshape-scene: GUI-free viewport logic shared by the app and the agent
//! API: the scene builder ([`builder`]), the sketch editor and its tools
//! ([`sketch_mode`]), the viewport [`palette`], the [`graphics_prefs`] data
//! and offscreen PNG rendering ([`png`]).

pub mod asm_scene;
pub mod builder;
pub mod camera_ops;
pub mod drawing_tools;
pub mod environment_maps;
pub mod graphics_prefs;
pub mod palette;
pub mod png;
pub mod sketch_mode;

pub use graphics_prefs::GraphicsPrefs;

/// Where documents go and what the documents page lists as "Owned by me" (created on first use): the user's
/// Documents folder's `noshape` (the OS's Known Folder, OneDrive-redirected on Windows when it is), or
/// `NOSHAPE_DOCUMENTS_DIR`. The app, the CLI and the MCP server all use this one.
pub fn documents_dir() -> std::path::PathBuf {
    let d = match std::env::var_os("NOSHAPE_DOCUMENTS_DIR") {
        Some(d) => std::path::PathBuf::from(d),
        None => dirs::document_dir().or_else(|| dirs::home_dir().map(|h| h.join("Documents"))).unwrap_or_else(|| std::path::PathBuf::from(".")).join("noshape"),
    };
    let _ = std::fs::create_dir_all(&d);
    d
}

/// noshape's per-user configuration directory (created on first use;
/// `NOSHAPE_CONFIG_DIR` overrides it, for tests).
pub fn config_dir() -> std::path::PathBuf {
    let d = match std::env::var_os("NOSHAPE_CONFIG_DIR") {
        Some(d) => std::path::PathBuf::from(d),
        None => dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("noshape"),
    };
    let _ = std::fs::create_dir_all(&d);
    d
}
