//! noshape's Feature Studio UI (docs/CRATES.md): the FeatureScript editor (highlighting, outline, completion,
//! squiggles and gutter notices), the notices panel, search, the Import panel, the profiler and std's source
//! document, on `noshape-app-core` (and `noshape-preferences-ui` for the Labs switches). What it needs of the app
//! above it (Create version, a version's window) it reaches through [`feature_studio_hooks`], which the `noshape`
//! binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;
// Preferences › Onshape Labs (FeatureScript Co-complete).
use noshape_preferences_ui::preferences_window;

pub mod feature_studio;
pub mod feature_studio_hooks;
mod fs_complete;
mod fs_import;
pub mod fs_notices;
mod fs_profile;
pub mod fs_search;
pub mod std_source;
