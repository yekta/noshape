//! What the Preferences pages ask of the app above them (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): the Part Studio's tools for the Shortcut toolbars page, refreshing the documents page after the
//! user name changes, and re-theming the app after Theming changes. The `noshape` binary registers them once at
//! startup (main.rs), before the first window opens.

use gpui_kit::*;
use std::sync::OnceLock;

/// A tool of a Shortcut toolbars list: (stored id, name, icon path).
pub type ToolRow = (String, String, SharedString);

/// The app's side of the Preferences pages.
pub struct Hooks {
    /// The Part Studio toolbar's features the S toolbar can show (Shortcut toolbars › Part Studio).
    pub part_studio_toolbar_tools: fn() -> Vec<ToolRow>,
    /// Shortcut toolbars › Sketch: Onshape's list of rows (stored id, label, icon path).
    pub sketch_prefs_rows: fn() -> Vec<(String, &'static str, SharedString)>,
    /// Onshape's default sketch S toolbar, by name.
    pub default_sketch_shortcut_names: fn() -> Vec<&'static str>,
    /// Redraw the documents page (the user name it shows changed).
    pub refresh_start_window: fn(&mut App),
    /// Apply the palette in effect to GPUI Kit's theme (after Theming changes).
    pub apply_kit_theme: fn(&mut App),
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the preferences hooks at startup (noshape_preferences_ui::preferences_hooks::register)")
}

pub fn part_studio_toolbar_tools() -> Vec<ToolRow> {
    (hooks().part_studio_toolbar_tools)()
}

pub fn sketch_prefs_rows() -> Vec<(String, &'static str, SharedString)> {
    (hooks().sketch_prefs_rows)()
}

pub fn default_sketch_shortcut_names() -> Vec<&'static str> {
    (hooks().default_sketch_shortcut_names)()
}

pub fn refresh_start_window(cx: &mut App) {
    (hooks().refresh_start_window)(cx)
}

pub fn apply_kit_theme(cx: &mut App) {
    (hooks().apply_kit_theme)(cx)
}
