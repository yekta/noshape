//! noshape's release management UI (docs/CRATES.md): the Release candidate, review and obsoletion dialogs, the
//! release workflow diagram, release thumbnails, revision history and the revision table template, and Company
//! settings (Numbering schemes, Release management, Revision tools), on `noshape-app-core` and
//! `noshape-preferences-ui` (the settings pages' layout). What it needs of the app above it (other documents' and
//! versions' windows, Create version, the workspace's Export) it reaches through [`release_hooks`], which the
//! `noshape` binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;
// The Preferences pages' layout and toasts (the Company settings pages are laid out like them).
use noshape_preferences_ui::{preferences_window, prefs_general};

pub mod company_settings;
mod numbering_prefs;
mod release_conditions;
pub mod release_dialog;
pub mod release_hooks;
mod release_prefs;
pub mod release_settings;
mod release_thumbs;
mod revision_template;
mod revision_tools;
