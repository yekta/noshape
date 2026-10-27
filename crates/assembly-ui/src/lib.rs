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
