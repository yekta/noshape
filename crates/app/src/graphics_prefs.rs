//! Onshape's Graphics preferences (Camera and render options > Graphics
//! preferences…): viewport, shaded body, phantom edge, mesh, sketch line
//! and construction line settings. Kept per user (a JSON file in the config
//! directory) and applied live to every graphics area.
//!
//! Defaults and ranges are Onshape's (help: View > Graphics preferences;
//! dialog in /root/onshape-research/ui-ref/dialogs/concepts__graphics-prefs-*).

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphicsPrefs {
    /// Viewport: ambient occlusion intensity, 0..1.
    pub ambient_occlusion: f32,
    /// Viewport: perspective field of view, degrees (20..120).
    pub fov_deg: f32,
    /// Shaded bodies: edge line width, px (0.1..3).
    pub line_width: f32,
    /// Shaded bodies: shininess, 0.01..1.
    pub shininess: f32,
    /// Shaded bodies: specular color (sRGB hex).
    pub specular: u32,
    /// Shaded bodies: section interference color.
    pub section_interference: u32,
    /// Phantom tangent edges: width px (0.1..3) and color.
    pub phantom_width: f32,
    pub phantom_color: u32,
    /// Mesh bodies: line width px (0.1..3).
    pub mesh_line_width: f32,
    /// Active sketches: line width px and colors.
    pub sketch_line_width: f32,
    pub sketch_constrained: u32,
    pub sketch_under: u32,
    pub sketch_under_occluded: u32,
    /// Inactive sketches: line width px and color.
    pub inactive_line_width: f32,
    pub inactive_color: u32,
    /// Construction lines: first dash, first gap, second dash, second gap (px).
    pub construction: [f32; 4],
}

impl Default for GraphicsPrefs {
    fn default() -> Self {
        Self {
            ambient_occlusion: 0.375,
            fov_deg: 45.0,
            line_width: 1.0,
            shininess: 0.03,
            specular: 0x999999,
            section_interference: 0xdc0000,
            phantom_width: 0.5,
            phantom_color: 0x232323,
            mesh_line_width: 0.3,
            sketch_line_width: 2.0,
            sketch_constrained: 0x000000,
            sketch_under: 0x0000d0,
            sketch_under_occluded: 0x4040ff,
            inactive_line_width: 1.0,
            inactive_color: 0x999999,
            construction: [4.0, 6.0, 30.0, 6.0],
        }
    }
}

impl Global for GraphicsPrefs {}

fn path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("noshape").join("graphics.json"))
}

/// Load the saved preferences into the app (call once at startup).
pub fn init(cx: &mut App) {
    let prefs = path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<GraphicsPrefs>(&b).ok()).unwrap_or_default();
    cx.set_global(prefs);
}

/// The current preferences.
pub fn get(cx: &App) -> GraphicsPrefs {
    cx.try_global::<GraphicsPrefs>().copied().unwrap_or_default()
}

/// Apply preferences everywhere (observers of the global refresh); `save`
/// also writes them to disk.
pub fn set(prefs: GraphicsPrefs, save: bool, cx: &mut App) {
    cx.set_global(prefs);
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

/// Linear RGB of an sRGB hex color.
pub fn lin3(hex: u32) -> [f32; 3] {
    let c = crate::theme::lin(hex);
    [c[0], c[1], c[2]]
}
