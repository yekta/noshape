//! What the Drawing studio asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): the workspace's Import, opening another document's window, the document's file tabs, the release
//! log and the drawing watermark, the documents library, Create version, the export rules link and a preference.
//! The `noshape` binary registers them once at startup (main.rs), before the first window opens.

use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::library::DocEntry;
use gpui_kit::*;
use noshape_model::document::TabId;
use noshape_model::file_tab::{FileKind, FileTab};
use noshape_model::release::ReleaseLog;
use std::path::PathBuf;
use std::sync::OnceLock;

/// What a file pick (Insert image, Insert DXF or DWG, Select a DWT) does with the file.
pub type OnPick = Box<dyn Fn(FileTab, &mut Window, &mut App)>;

/// [`Hooks::pick_file`]: the document, the dialog's title, the file kinds listed, the extensions Import accepts.
pub type PickFile = fn(Entity<DocModel>, &'static str, &'static [FileKind], &'static [&'static str], &mut Window, &mut App, OnPick);

/// The app's side of the Drawing studio.
pub struct Hooks {
    /// The workspace's + › Import… (the Insert view picker's "Import..." row).
    pub import: fn(&mut Window, &mut App),
    /// Open (or bring to the front) the document at a path in its own window.
    pub open_document_window: fn(PathBuf, Gpu, &mut App) -> anyhow::Result<()>,
    /// Onshape's insert-from-file dialog over the document's file tabs of some kinds (title, kinds, extensions).
    pub pick_file: PickFile,
    /// The document's release log (revision tables).
    pub release_log: fn(&DocModel) -> ReleaseLog,
    /// The watermark text of a drawing tab, if any (Company settings › Release management).
    pub watermark_text: fn(&DocModel, TabId) -> Option<String>,
    /// The local documents library (the Insert view picker's Other documents); runs on a background thread.
    pub library_docs: fn() -> Vec<DocEntry>,
    /// The Create version dialog of a document.
    pub create_version: fn(Entity<DocModel>, &mut Window, &mut App),
    /// The export dialogs' "View export rules" link.
    pub export_rules_link: fn(&'static str) -> Stateful<Div>,
    /// Preferences › Drawings › Background color of model space: Light.
    pub light_model_space: fn() -> bool,
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the drawing hooks at startup (noshape_drawing_ui::drawing_hooks::register)")
}

pub fn import(window: &mut Window, cx: &mut App) {
    (hooks().import)(window, cx)
}

pub fn open_document_window(path: PathBuf, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    (hooks().open_document_window)(path, gpu, cx)
}

pub fn pick_file(doc: Entity<DocModel>, title: &'static str, kinds: &'static [FileKind], exts: &'static [&'static str], window: &mut Window, cx: &mut App, on_pick: impl Fn(FileTab, &mut Window, &mut App) + 'static) {
    (hooks().pick_file)(doc, title, kinds, exts, window, cx, Box::new(on_pick))
}

pub fn release_log(doc: &DocModel) -> ReleaseLog {
    (hooks().release_log)(doc)
}

pub fn watermark_text(doc: &DocModel, tab: TabId) -> Option<String> {
    (hooks().watermark_text)(doc, tab)
}

pub fn library_docs() -> Vec<DocEntry> {
    (hooks().library_docs)()
}

pub fn create_version(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    (hooks().create_version)(doc, window, cx)
}

pub fn export_rules_link(id: &'static str) -> Stateful<Div> {
    (hooks().export_rules_link)(id)
}

pub fn light_model_space() -> bool {
    (hooks().light_model_space)()
}
