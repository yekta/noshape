//! What the Feature Studio asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): Create version, and a Feature Studio of a document version in a window of its own (std's source).
//! The `noshape` binary registers them once at startup (main.rs), before the first window opens.

use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use gpui_kit::*;
use std::path::PathBuf;
use std::sync::OnceLock;

/// [`Hooks::open_studio_in_version`]: the document's path, the version, the Feature Studio's name, the caret's line
/// and column, the GPU.
pub type OpenStudioInVersion = fn(PathBuf, noshape_store::MicroversionId, String, usize, usize, Gpu, &mut App) -> anyhow::Result<()>;

/// The app's side of the Feature Studio.
pub struct Hooks {
    /// The Create version dialog of a document.
    pub create_version: fn(Entity<DocModel>, &mut Window, &mut App),
    /// Open a version of the document at a path in a window of its own, read-only, on a Feature Studio with the
    /// caret at a line and column.
    pub open_studio_in_version: OpenStudioInVersion,
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the feature studio hooks at startup (noshape_feature_studio_ui::feature_studio_hooks::register)")
}

pub fn create_version(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    (hooks().create_version)(doc, window, cx)
}

pub fn open_studio_in_version(path: PathBuf, version: noshape_store::MicroversionId, module: String, line: usize, column: usize, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    (hooks().open_studio_in_version)(path, version, module, line, column, gpu, cx)
}
