//! What the documents page asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): opening a document in its window, the import modal, and the build it shows. The `noshape` binary
//! registers them once at startup (main.rs), before the first window opens.

use crate::gpu::Gpu;
use gpui_kit::*;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The app's side of the documents page.
pub struct Hooks {
    /// Open (or create, with that document) the document at a path in its own window, or bring it to the front.
    pub open_document_window: fn(PathBuf, Option<noshape_model::Document>, Gpu, &mut App) -> anyhow::Result<()>,
    /// Open the document (or bring its window to the front) at a tab: a search result row (a part or a tab).
    pub open_document_at: fn(PathBuf, noshape_model::document::TabId, Gpu, &mut App) -> anyhow::Result<()>,
    /// The build as the documents page shows it ("0.4.4.c8d74e607a47").
    pub build_label: fn() -> String,
    /// Import files…'s "Import to Onshape" modal for these CAD files; Import calls back with the options and whether to
    /// split them into a document each.
    pub import_dialog: fn(Vec<PathBuf>, ImportCallback, &mut Window, &mut App),
    /// The update banner (the app's updates.rs), when there is one.
    pub update_banner: fn(&App) -> Option<AnyElement>,
}

/// What Import does with the modal's options (and "Split into multiple documents").
pub type ImportCallback = Box<dyn Fn(noshape_ops::import::ImportChoice, bool, &mut Window, &mut App)>;

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the start window hooks at startup (noshape_start_ui::start_hooks::register)")
}

pub fn open_document_window(path: PathBuf, create_with: Option<noshape_model::Document>, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    (hooks().open_document_window)(path, create_with, gpu, cx)
}

pub fn open_document_at(path: PathBuf, tab: noshape_model::document::TabId, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    (hooks().open_document_at)(path, tab, gpu, cx)
}

pub fn import_dialog(paths: Vec<PathBuf>, on_import: ImportCallback, window: &mut Window, cx: &mut App) {
    (hooks().import_dialog)(paths, on_import, window, cx)
}

pub fn build_label() -> String {
    (hooks().build_label)()
}

pub fn update_banner(cx: &App) -> Option<AnyElement> {
    HOOKS.get().and_then(|h| (h.update_banner)(cx))
}
