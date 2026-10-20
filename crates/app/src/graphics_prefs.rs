//! Onshape's Graphics preferences (Camera and render options > Graphics
//! preferences…): viewport, shaded body, phantom edge, mesh, sketch line
//! and construction line settings. Kept per user (a JSON file in the config
//! directory) and applied live to every graphics area.
//!
//! Defaults and ranges are Onshape's (help: View > Graphics preferences;
//! dialog in /root/onshape-research/ui-ref/dialogs/concepts__graphics-prefs-*).

use gpui_kit::{App, Global};

pub use noshape_scene::graphics_prefs::{GraphicsPrefs, lin3};

/// The preferences as a gpui global (observe it to apply changes live).
#[derive(Clone, Copy, Debug, Default)]
pub struct GraphicsGlobal(pub GraphicsPrefs);

impl Global for GraphicsGlobal {}

fn path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("noshape").join("graphics.json"))
}

/// Load the saved preferences into the app (call once at startup).
pub fn init(cx: &mut App) {
    let prefs = path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<GraphicsPrefs>(&b).ok()).unwrap_or_default();
    cx.set_global(GraphicsGlobal(prefs));
}

/// The current preferences.
pub fn get(cx: &App) -> GraphicsPrefs {
    cx.try_global::<GraphicsGlobal>().map(|g| g.0).unwrap_or_default()
}

/// Apply preferences everywhere (observers of the global refresh); `save`
/// also writes them to disk.
pub fn set(prefs: GraphicsPrefs, save: bool, cx: &mut App) {
    cx.set_global(GraphicsGlobal(prefs));
    if save
        && let Some(p) = path()
    {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(&prefs) {
            let _ = std::fs::write(p, json);
        }
    }
}
