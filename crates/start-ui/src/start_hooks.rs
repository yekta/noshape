//! What the documents page asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): opening a document in its window, and the build it shows. The `noshape` binary registers them once
//! at startup (main.rs), before the first window opens.

use crate::gpu::Gpu;
use gpui_kit::*;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The app's side of the documents page.
pub struct Hooks {
    /// Open (or create, with that document) the document at a path in its own window, or bring it to the front.
    pub open_document_window: fn(PathBuf, Option<noshape_model::Document>, Gpu, &mut App) -> anyhow::Result<()>,
    /// The build as the documents page shows it ("0.4.4.c8d74e607a47").
    pub build_label: fn() -> String,
}

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

pub fn build_label() -> String {
    (hooks().build_label)()
}
