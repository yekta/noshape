//! noshape's Preferences (docs/CRATES.md): the Preferences window and its pages (general, language and theming,
//! environment profiles, units and decimal format, mouse controls, drawings, export rules, material libraries,
//! Part Studio and Assembly settings, shortcut toolbars, keyboard shortcuts), on `noshape-app-core`. The Assembly's
//! and the drawing's toolbar settings come from `noshape-assembly-ui` and `noshape-drawing-ui`. What it needs of the
//! app above it (the Part Studio's tools, the documents page, the app theme) it reaches through
//! [`preferences_hooks`], which the `noshape` binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;
// The Assembly's and the drawing's toolbar settings (Shortcut toolbars).
use noshape_assembly_ui::assembly_view;
use noshape_drawing_ui::drawing_keys;

mod drawing_prefs;
pub mod material_libraries;
mod material_prefs;
mod mouse_prefs;
pub mod preferences_hooks;
pub mod preferences_window;
mod prefs_export_rules;
pub mod prefs_general;
pub mod units_prefs;
