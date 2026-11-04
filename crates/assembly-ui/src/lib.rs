//! noshape's Assembly studio UI (docs/CRATES.md): the assembly editor, its panels, dialogs and tools, on
//! `noshape-app-core`. What it needs of the app above it (the workspace, other documents' windows, the release
//! dialogs, the documents library, the Variable Studio, preferences, the Part Studio's export and input propagation
//! dialogs) it reaches through [`assembly_hooks`], which the `noshape` binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;
// The Drawing studio UI (Create drawing from an assembly or a part).
use noshape_drawing_ui::drawing_create;

pub mod assembly_hooks;
pub mod assembly_mate_dialog;
pub mod assembly_scene;
pub mod assembly_view;
pub mod measure_panel;

// UI tests get their own settings directory, not the user's ~/.config/noshape (noshape-ui-test-support).
#[cfg(test)]
use noshape_ui_test_support as _;

/// The settings the UI tests read are their own: another run's or an app's panel widths, columns and preferences on
/// the same machine made simulated clicks miss.
#[cfg(test)]
#[test]
fn ui_tests_have_their_own_settings_directory() {
    let dir = noshape_app_core::paths::config_dir();
    assert!(noshape_ui_test_support::is_not_the_users(&dir), "{dir:?} is the user's");
}
