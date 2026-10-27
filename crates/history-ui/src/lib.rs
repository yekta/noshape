//! noshape's Versions and history UI (docs/CRATES.md): the history panel (versions, branches, changes, releases),
//! Create version, version properties, compare (the graphics and Feature Studio code) and merge, on
//! `noshape-app-core`, with `noshape-release-ui` (the release rows and dialogs) and `noshape-feature-studio-ui` (the
//! code compare's highlighting). What it needs of the app above it (the Part Studio's feature icons, the Workspace
//! protection dialog) it reaches through [`history_hooks`], which the `noshape` binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;
// The FeatureScript highlighting of the code compare.
use noshape_feature_studio_ui::feature_studio;
// Release rows, the Release candidate and review dialogs.
use noshape_release_ui::release_dialog;

pub mod compare_mode;
pub mod history_hooks;
pub mod history_panel;
mod merge_dialog;
mod version_properties;
