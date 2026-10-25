//! noshape-scene: GUI-free viewport logic shared by the app and the agent
//! API: the scene builder ([`builder`]), the sketch editor and its tools
//! ([`sketch_mode`]), the viewport [`palette`], the [`graphics_prefs`] data
//! and offscreen PNG rendering ([`png`]).

pub mod asm_scene;
pub mod builder;
pub mod drawing_tools;
pub mod environment_maps;
pub mod graphics_prefs;
pub mod palette;
pub mod png;
pub mod sketch_mode;

pub use graphics_prefs::GraphicsPrefs;

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
