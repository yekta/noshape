//! noshape-scene: GUI-free viewport logic shared by the app and the agent
//! API: the scene builder ([`builder`]), the sketch editor and its tools
//! ([`sketch_mode`]), the viewport [`palette`], the [`graphics_prefs`] data
//! and offscreen PNG rendering ([`png`]).

pub mod asm_scene;
pub mod builder;
pub mod drawing_tools;
pub mod graphics_prefs;
pub mod palette;
pub mod png;
pub mod sketch_mode;

pub use graphics_prefs::GraphicsPrefs;

/// noshape's per-user configuration directory (created on first use).
pub fn config_dir() -> std::path::PathBuf {
    let d = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("noshape");
    let _ = std::fs::create_dir_all(&d);
    d
}
