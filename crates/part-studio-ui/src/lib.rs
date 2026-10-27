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
