//! What the Assembly studio asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not
//! dependencies): other documents' windows, the documents library and its folders, the release dialogs, Create
//! version, the tab's Properties, a preference, the Part Studio's Export and Input propagation dialogs and the
//! Variable Studio's tables. The `noshape` binary registers them once at startup (main.rs), before the first window
//! opens.

use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::library::DocEntry;
use gpui_kit::*;
use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabId;
use noshape_model::reference::{BodyId, FeatureId};
use noshape_model::release::ReleaseItem;
use noshape_model::variables::StudioRef;
use noshape_ops::export::ExportSettings;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

/// What Export does with the chosen settings and file name (the Part Studio's `export::RunExport`).
pub type RunExport = Rc<dyn Fn(ExportSettings, String, &mut Window, &mut App)>;

/// [`Hooks::store_export`]'s writer: writes the export to a path, returns the file written.
pub type StoreWrite<'a> = Box<dyn FnOnce(&Path) -> anyhow::Result<PathBuf> + 'a>;

/// [`Hooks::store_export`]: the document, the tab the file tab goes after, the file name, the writer.
pub type StoreExport = fn(&Entity<DocModel>, TabId, &str, StoreWrite<'_>, &mut App) -> anyhow::Result<String>;

/// Rows of the Input propagation dialog: (row key, reference name, per input: None = can't propagate, Some(already
/// propagated)).
pub type PropagationRows = Vec<(FeatureId, String, Vec<Option<bool>>)>;

/// What ✓ in the Input propagation dialog does with the newly ticked (input index, row key).
pub type OnConfirm = Rc<dyn Fn(Vec<(usize, FeatureId)>, &mut Window, &mut App)>;

/// [`Hooks::open_propagation_dialog`]: owner name, inputs (id, name), rows, checked, on ✓.
pub type OpenPropagation = fn(String, Vec<(String, String)>, PropagationRows, Vec<Vec<bool>>, OnConfirm, &mut Window, &mut App);

/// [`Hooks::release_part_items`]: the document, the Part Studio, its parts and their names, the configuration.
pub type PartItems = fn(&DocModel, TabId, &[BodyId], &[String], &ConfigValues) -> Vec<ReleaseItem>;

/// [`Hooks::readonly_variable_table`]: the document, the element, the studio, its index and the count.
pub type ReadonlyVariableTable = fn(&Entity<DocModel>, TabId, &StudioRef, usize, usize, &App) -> Option<AnyElement>;

/// The app's side of the Assembly studio.
pub struct Hooks {
    /// Open (or bring to the front) the document at a path in its own window.
    pub open_document_window: fn(PathBuf, Gpu, &mut App) -> anyhow::Result<()>,
    /// Open a version of the document at a path in its own window.
    pub open_version_window: fn(PathBuf, noshape_store::MicroversionId, Gpu, &mut App) -> anyhow::Result<AnyWindowHandle>,
    /// The local documents library (the Insert panel's Other documents); runs on a background thread.
    pub library_docs: fn() -> Vec<DocEntry>,
    /// The extra folders the documents page lists (besides the documents folder).
    pub watched_folders: fn() -> Vec<PathBuf>,
    /// A folder's subfolders, as the documents page lists them.
    pub subfolders: fn(&Path) -> Vec<PathBuf>,
    /// A document file's thumbnail (Reference manager rows).
    pub file_thumbnail: fn(&Path) -> Option<Arc<RenderImage>>,
    /// The release items of a Part Studio's parts.
    pub release_part_items: PartItems,
    /// The release items of a tab.
    pub release_tab_items: fn(&DocModel, TabId) -> Vec<ReleaseItem>,
    /// The Release candidate dialog for items.
    pub open_release_candidate: fn(Entity<DocModel>, Vec<ReleaseItem>, &mut Window, &mut App),
    /// The configuration last viewed in a Part Studio (Insert › Last viewed configuration).
    pub last_viewed_config: fn(TabId) -> Option<ConfigValues>,
    /// The Create version dialog of a document.
    pub create_version: fn(Entity<DocModel>, &mut Window, &mut App),
    /// A tab's Properties dialog.
    pub tab_properties: fn(Entity<DocModel>, TabId, &mut Window, &mut App),
    /// Preferences: the instance list shows properties.
    pub instance_list_properties: fn() -> bool,
    pub set_instance_list_properties: fn(bool),
    /// The Export modal (file name, `assembly`: the Assembly's options, `parts_only`: without the assembly's checkboxes).
    pub open_export_modal: fn(String, bool, bool, RunExport, &mut Window, &mut App),
    /// Options › Store file in a new tab: write the export to a temporary file and keep it as a file tab after a
    /// tab. Returns the stored file's name.
    pub store_export: StoreExport,
    /// Keep an exported file (already written) as a file tab after a tab.
    pub store_written: fn(&Entity<DocModel>, TabId, &Path, &mut App) -> String,
    /// The Input propagation dialog.
    pub open_propagation_dialog: OpenPropagation,
    /// A Variable Studio's table shown in an element's Variable table panel (document, studio, host element).
    pub variable_table: fn(Entity<DocModel>, TabId, TabId, &mut Window, &mut App) -> AnyView,
    /// Another document's Variable Studio, read-only, in an element's Variable table (element, studio, index, count).
    pub readonly_variable_table: ReadonlyVariableTable,
    /// The Variable table panel's footer (Create Variable Studio | ▴) of an element.
    pub variable_panel_footer: fn(Entity<DocModel>, TabId, &App) -> AnyElement,
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the assembly hooks at startup (noshape_assembly_ui::assembly_hooks::register)")
}

pub fn open_document_window(path: PathBuf, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    (hooks().open_document_window)(path, gpu, cx)
}

pub fn open_version_window(path: PathBuf, version: noshape_store::MicroversionId, gpu: Gpu, cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    (hooks().open_version_window)(path, version, gpu, cx)
}

pub fn library_docs() -> Vec<DocEntry> {
    (hooks().library_docs)()
}

pub fn watched_folders() -> Vec<PathBuf> {
    (hooks().watched_folders)()
}

pub fn subfolders(dir: &Path) -> Vec<PathBuf> {
    (hooks().subfolders)(dir)
}

pub fn file_thumbnail(path: &Path) -> Option<Arc<RenderImage>> {
    (hooks().file_thumbnail)(path)
}

pub fn release_part_items(doc: &DocModel, tab: TabId, parts: &[BodyId], names: &[String], config: &ConfigValues) -> Vec<ReleaseItem> {
    (hooks().release_part_items)(doc, tab, parts, names, config)
}

pub fn release_tab_items(doc: &DocModel, tab: TabId) -> Vec<ReleaseItem> {
    (hooks().release_tab_items)(doc, tab)
}

pub fn open_release_candidate(doc: Entity<DocModel>, items: Vec<ReleaseItem>, window: &mut Window, cx: &mut App) {
    (hooks().open_release_candidate)(doc, items, window, cx)
}

pub fn last_viewed_config(tab: TabId) -> Option<ConfigValues> {
    (hooks().last_viewed_config)(tab)
}

pub fn create_version(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    (hooks().create_version)(doc, window, cx)
}

pub fn tab_properties(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut App) {
    (hooks().tab_properties)(doc, tab, window, cx)
}

pub fn instance_list_properties() -> bool {
    (hooks().instance_list_properties)()
}

pub fn set_instance_list_properties(on: bool) {
    (hooks().set_instance_list_properties)(on)
}

pub fn open_export_modal(base: String, assembly: bool, parts_only: bool, run: RunExport, window: &mut Window, cx: &mut App) {
    (hooks().open_export_modal)(base, assembly, parts_only, run, window, cx)
}

pub fn store_export(doc: &Entity<DocModel>, tab: TabId, file: &str, write: impl FnOnce(&Path) -> anyhow::Result<PathBuf>, cx: &mut App) -> anyhow::Result<String> {
    (hooks().store_export)(doc, tab, file, Box::new(write), cx)
}

pub fn store_written(doc: &Entity<DocModel>, tab: TabId, path: &Path, cx: &mut App) -> String {
    (hooks().store_written)(doc, tab, path, cx)
}

pub fn open_propagation_dialog(owner_name: String, inputs: Vec<(String, String)>, rows: PropagationRows, checked: Vec<Vec<bool>>, on_confirm: OnConfirm, window: &mut Window, cx: &mut App) {
    (hooks().open_propagation_dialog)(owner_name, inputs, rows, checked, on_confirm, window, cx)
}

pub fn variable_table(doc: Entity<DocModel>, studio: TabId, host: TabId, window: &mut Window, cx: &mut App) -> AnyView {
    (hooks().variable_table)(doc, studio, host, window, cx)
}

pub fn readonly_variable_table(doc: &Entity<DocModel>, element: TabId, r: &StudioRef, k: usize, n: usize, cx: &App) -> Option<AnyElement> {
    (hooks().readonly_variable_table)(doc, element, r, k, n, cx)
}

pub fn variable_panel_footer(doc: Entity<DocModel>, element: TabId, cx: &App) -> AnyElement {
    (hooks().variable_panel_footer)(doc, element, cx)
}

/// Inert hooks for UI tests on GPUI's test platform (docs/TESTS.md "UI keystroke tests"): nothing above the
/// Assembly studio exists there.
#[cfg(test)]
pub fn register_for_tests() {
    register(Hooks {
        open_document_window: |_, _, _| Ok(()),
        open_version_window: |_, _, _, _| Err(anyhow::anyhow!("no version windows in UI tests")),
        library_docs: Vec::new,
        watched_folders: Vec::new,
        subfolders: |_| Vec::new(),
        file_thumbnail: |_| None,
        release_part_items: |_, _, _, _, _| Vec::new(),
        release_tab_items: |_, _| Vec::new(),
        open_release_candidate: |_, _, _, _| {},
        last_viewed_config: |_| None,
        create_version: |_, _, _| {},
        tab_properties: |_, _, _, _| {},
        instance_list_properties: || false,
        set_instance_list_properties: |_| {},
        open_export_modal: |_, _, _, _, _, _| {},
        store_export: |_, _, _, _, _| Err(anyhow::anyhow!("no stored exports in UI tests")),
        store_written: |_, _, _, _| String::new(),
        open_propagation_dialog: |_, _, _, _, _, _, _| {},
        variable_table: |_, _, _, _, cx| cx.new(|_| gpui_kit::Empty).into(),
        readonly_variable_table: |_, _, _, _, _, _| None,
        variable_panel_footer: |_, _, _| div().into_any_element(),
    });
}
