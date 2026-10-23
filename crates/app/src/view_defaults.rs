//! Onshape's camera-menu defaults (A-partstudio-ui 6.4 "Set as default"):
//! Perspective view and Highlight boundary edges are off in a new graphics
//! area unless the user right-clicked the item › Set as default. Kept per
//! user (a JSON file in the config directory), apart from the Graphics
//! preferences (whose Reset doesn't touch them).

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewDefaults {
    /// New graphics areas start in perspective.
    pub perspective: bool,
    /// New Part Studios highlight boundary edges.
    pub highlight_boundary: bool,
}

impl ViewDefaults {
    /// A new graphics area's projection: orthographic unless Perspective
    /// view was set as default.
    pub fn projection(&self) -> noshape_render::Projection {
        if self.perspective { noshape_render::Projection::Perspective } else { noshape_render::Projection::Orthographic }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ViewDefaultsGlobal(ViewDefaults);

impl Global for ViewDefaultsGlobal {}

fn path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("noshape").join("view_defaults.json"))
}

/// Load the saved defaults into the app (call once at startup).
pub fn init(cx: &mut App) {
    let d = path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<ViewDefaults>(&b).ok()).unwrap_or_default();
    cx.set_global(ViewDefaultsGlobal(d));
}

/// The current defaults (all off before [`init`]).
pub fn get(cx: &App) -> ViewDefaults {
    cx.try_global::<ViewDefaultsGlobal>().map(|g| g.0).unwrap_or_default()
}

/// Make `d` the defaults and save them.
pub fn set(d: ViewDefaults, cx: &mut App) {
    cx.set_global(ViewDefaultsGlobal(d));
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(&d) {
            let _ = std::fs::write(p, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_graphics_area_is_orthographic() {
        // Onshape: Perspective view is off unless set as default.
        assert_eq!(ViewDefaults::default().projection(), noshape_render::Projection::Orthographic);
        assert_eq!(noshape_render::Camera::default().projection, noshape_render::Projection::Orthographic);
        assert_eq!(ViewDefaults { perspective: true, ..Default::default() }.projection(), noshape_render::Projection::Perspective);
    }

    #[test]
    fn defaults_are_off_and_old_files_load() {
        assert_eq!(ViewDefaults::default(), ViewDefaults { perspective: false, highlight_boundary: false });
        let d: ViewDefaults = serde_json::from_str(r#"{"perspective":true}"#).unwrap();
        assert!(d.perspective && !d.highlight_boundary);
    }
}
