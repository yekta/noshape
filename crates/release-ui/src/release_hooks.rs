//! What release management asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): other documents' and versions' windows, Create version and the workspace's Export of an item. The
//! `noshape` binary registers them once at startup (main.rs), before the first window opens.

use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::document::TabId;
use std::path::PathBuf;
use std::sync::OnceLock;

/// [`Hooks::open_version_window`]: the document's path, the version, the GPU; returns the window and its document.
pub type OpenVersionWindow = fn(PathBuf, noshape_store::MicroversionId, Gpu, &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<DocModel>)>;

/// The app's side of release management.
pub struct Hooks {
    /// Open (or bring to the front) the document at a path in its own window.
    pub open_document_window: fn(PathBuf, Gpu, &mut App) -> anyhow::Result<()>,
    /// Open a version of the document at a path in a window of its own, read-only.
    pub open_version_window: OpenVersionWindow,
    /// The Create version dialog of a document.
    pub create_version: fn(Entity<DocModel>, &mut Window, &mut App),
    /// The window's workspace exports a tab (or some of its parts).
    pub request_export: fn(TabId, Vec<BodyId>, &mut Window, &mut App),
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the release hooks at startup (noshape_release_ui::release_hooks::register)")
}

pub fn open_document_window(path: PathBuf, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    (hooks().open_document_window)(path, gpu, cx)
}

pub fn open_version_window(path: PathBuf, version: noshape_store::MicroversionId, gpu: Gpu, cx: &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<DocModel>)> {
    (hooks().open_version_window)(path, version, gpu, cx)
}

pub fn create_version(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    (hooks().create_version)(doc, window, cx)
}

pub fn request_export(tab: TabId, parts: Vec<BodyId>, window: &mut Window, cx: &mut App) {
    (hooks().request_export)(tab, parts, window, cx)
}
