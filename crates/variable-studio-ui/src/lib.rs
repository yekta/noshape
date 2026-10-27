//! noshape's Variable Studio UI (docs/CRATES.md): the Variable Studio table and its configurations, CSV import and
//! inserting Variable Studios into elements (the Variable table panel), on `noshape-app-core`. What it needs of the
//! app above it (other documents' windows, Create version) it reaches through [`variable_hooks`], which the
//! `noshape` binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;

pub mod variable_hooks;
pub mod variable_import;
pub mod variable_insert;
pub mod variable_studio;
