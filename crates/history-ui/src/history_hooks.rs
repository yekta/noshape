//! What Versions and history asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): the Part Studio's feature icons (the compare's feature lists) and the Workspace protection dialog.
//! The `noshape` binary registers them once at startup (main.rs), before the first window opens.

use crate::doc_model::DocModel;
use gpui_kit::*;
use noshape_store::BranchId;
use std::sync::OnceLock;

/// The app's side of Versions and history.
pub struct Hooks {
    /// A feature type's icon path, as the Part Studio's feature list shows it.
    pub feature_icon: fn(&str) -> SharedString,
    /// The Workspace protection dialog of a branch.
    pub workspace_protection: fn(Entity<DocModel>, BranchId, &mut Window, &mut App),
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the history hooks at startup (noshape_history_ui::history_hooks::register)")
}

pub fn feature_icon(type_id: &str) -> SharedString {
    (hooks().feature_icon)(type_id)
}

pub fn workspace_protection(doc: Entity<DocModel>, branch: BranchId, window: &mut Window, cx: &mut App) {
    (hooks().workspace_protection)(doc, branch, window, cx)
}
