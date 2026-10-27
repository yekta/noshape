//! noshape's documents page (docs/CRATES.md): the start window with the documents list, folders, labels, search,
//! the Trash, Import and Create, on `noshape-app-core` (the documents library and the Trash are app-core's
//! `start_library` and `start_trash`). Its user menu opens Preferences (`noshape-preferences-ui`) and Company settings
//! (`noshape-release-ui`). What it needs of the app above it (a document's window, the build label) it reaches
//! through [`start_hooks`], which the `noshape` binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;
// The user menu's Preferences… and Company settings.
use noshape_preferences_ui::preferences_window;
use noshape_release_ui::company_settings;

mod start_search;
pub mod start_hooks;
pub mod start_window;
