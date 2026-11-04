//! noshape's Part Studio UI (docs/CRATES.md): the Part Studio editor, its feature dialog, sketch mode, panels and
//! tools, on `noshape-app-core`. What it needs of the app above it (other documents' windows, the release dialogs,
//! the documents library, the Variable Studio, Compare and Repair, preferences and the keyboard shortcuts page) it
//! reaches through [`part_studio_hooks`], which the `noshape` binary registers at startup.
// `cargo test`/`clippy --all-targets` hit "recursion limit reached while expanding `#[test]`" at the default of 128.
#![recursion_limit = "256"]

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;
// The Drawing studio UI (Create drawing of a part or a sketch).
use noshape_drawing_ui::drawing_create;

pub mod compare_graphics;
pub mod feature_dialog;
pub mod fs_reference;
pub mod part_studio;
pub mod part_studio_hooks;
pub mod select_item;
pub mod sketch_fonts;
pub mod sketch_mode;
pub mod var_complete;

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
