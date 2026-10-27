//! What the Part Studio asks of the app above it (docs/CRATES.md rule 3: upward calls are hooks, not dependencies):
//! other documents' windows, the documents library, the release dialogs, Create version, preferences and the
//! keyboard shortcuts page, the Variable Studio's tables, Compare and Repair, the added material libraries. The
//! `noshape` binary registers them once at startup (main.rs), before the first window opens.

use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::library::DocEntry;
use crate::material_libraries::AddedLibrary;
use crate::shortcuts::KeyStyle;
use gpui_kit::*;
use noshape_model::configuration::ConfigValues;
use noshape_model::document::{Document, TabId};
use noshape_model::material_library::MaterialLibrary;
use noshape_model::reference::BodyId;
use noshape_model::release::{ReleaseItem, ReleaseLog};
use noshape_model::variables::StudioRef;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// [`Hooks::release_part_items`]: the document, the Part Studio, its parts and their names, the configuration.
pub type PartItems = fn(&DocModel, TabId, &[BodyId], &[String], &ConfigValues) -> Vec<ReleaseItem>;

/// [`Hooks::readonly_variable_table`]: the document, the element, the studio, its index and the count.
pub type ReadonlyVariableTable = fn(&Entity<DocModel>, TabId, &StudioRef, usize, usize, &App) -> Option<AnyElement>;

/// [`Hooks::variable_type_cell`]: the select's id, its options, the current one, on select.
pub type TypeCell = fn(ElementId, Vec<String>, usize, Box<dyn Fn(usize, &mut Window, &mut App)>) -> Div;

/// [`Hooks::compare_documents`]: the compared documents and their configurations (base, then target).
pub type CompareDocuments = fn(&Entity<DocModel>, &App) -> Option<(Arc<Document>, ConfigValues, Arc<Document>, ConfigValues)>;

/// What the Add material library modal does with the chosen library (None: closed without one).
pub type OnAddLibrary = Box<dyn Fn(Option<AddedLibrary>, &mut Window, &mut App)>;

/// The app's side of the Part Studio.
pub struct Hooks {
    /// Open (or bring to the front) the document at a path in its own window.
    pub open_document_window: fn(PathBuf, Gpu, &mut App) -> anyhow::Result<()>,
    /// The local documents library (Select item's Other documents); runs on a background thread.
    pub library_docs: fn() -> Vec<DocEntry>,
    /// The other documents a Variable Studio can be referenced from.
    pub other_documents: fn(&Path) -> Vec<PathBuf>,
    /// The document's release log.
    pub release_log: fn(&DocModel) -> ReleaseLog,
    /// The release items of a Part Studio's parts.
    pub release_part_items: PartItems,
    /// The Release candidate dialog for items.
    pub open_release_candidate: fn(Entity<DocModel>, Vec<ReleaseItem>, &mut Window, &mut App),
    /// A part number's Revision history dialog.
    pub open_revision_history: fn(Entity<DocModel>, String, &mut Window, &mut App),
    /// Remember the configuration a Part Studio shows (Insert › Last viewed configuration).
    pub note_viewed_config: fn(TabId, &ConfigValues),
    /// The configuration last viewed in a Part Studio.
    pub last_viewed_config: fn(TabId) -> Option<ConfigValues>,
    /// The Create version dialog of a document.
    pub create_version: fn(Entity<DocModel>, &mut Window, &mut App),
    /// Preferences › Keyboard shortcuts.
    pub show_keyboard_shortcuts: fn(&mut App),
    /// The Shift+/ keyboard shortcuts sheet's window (beside the opener's size).
    pub show_shortcuts_page: fn(Option<Size<Pixels>>, &mut App),
    /// A key chip as Preferences › Keyboard shortcuts draws it.
    pub key_chip: fn(SharedString, KeyStyle) -> Div,
    /// The Export dialogs' "View export rules" link.
    pub export_rules_link: fn(&'static str) -> Stateful<Div>,
    /// Preferences: the parts list shows properties.
    pub parts_list_properties: fn() -> bool,
    pub set_parts_list_properties: fn(bool),
    /// The width a feature type's dialog was last dragged to.
    pub feature_dialog_width: fn(&str) -> Option<u32>,
    pub set_feature_dialog_width: fn(&str, u32),
    /// A Variable Studio's table shown in an element's Variable table panel (document, studio, host element).
    pub variable_table: fn(Entity<DocModel>, TabId, TabId, &mut Window, &mut App) -> AnyView,
    /// Another document's Variable Studio, read-only, in an element's Variable table (element, studio, index, count).
    pub readonly_variable_table: ReadonlyVariableTable,
    /// The Variable table panel's footer (Create Variable Studio | ▴) of an element.
    pub variable_panel_footer: fn(Entity<DocModel>, TabId, &App) -> AnyElement,
    /// The Variable Studio's Type select.
    pub variable_type_cell: TypeCell,
    /// Versions and history › Compare: the blend (None: not comparing).
    pub compare_blend: fn(&Entity<DocModel>, &App) -> Option<f32>,
    pub compare_documents: CompareDocuments,
    /// Compare's panel beside the feature list, and its slider over the graphics.
    pub compare_panel: fn(&Entity<DocModel>, TabId, &App) -> Option<AnyElement>,
    pub compare_slider: fn(&Entity<DocModel>, &App) -> Option<AnyElement>,
    /// Repair mode: open, close, its panel (over the graphics).
    pub repair_open: fn(&Entity<DocModel>, &App) -> bool,
    pub repair_close: fn(&Entity<DocModel>, &mut App),
    pub repair_panel: fn(&Entity<DocModel>, Option<AnyElement>, &App) -> Option<AnyElement>,
    /// The materials of an added material library.
    pub materials_of: fn(&AddedLibrary) -> MaterialLibrary,
    /// The Add material library modal.
    pub open_add_material_library: fn(OnAddLibrary, &mut Window, &mut App),
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Install the app's side (once; later calls are ignored).
pub fn register(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

fn hooks() -> &'static Hooks {
    HOOKS.get().expect("the app registers the Part Studio hooks at startup (noshape_part_studio_ui::part_studio_hooks::register)")
}

pub fn open_document_window(path: PathBuf, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    (hooks().open_document_window)(path, gpu, cx)
}

pub fn library_docs() -> Vec<DocEntry> {
    (hooks().library_docs)()
}

pub fn other_documents(current: &Path) -> Vec<PathBuf> {
    (hooks().other_documents)(current)
}

pub fn release_log(doc: &DocModel) -> ReleaseLog {
    (hooks().release_log)(doc)
}

pub fn release_part_items(doc: &DocModel, tab: TabId, parts: &[BodyId], names: &[String], config: &ConfigValues) -> Vec<ReleaseItem> {
    (hooks().release_part_items)(doc, tab, parts, names, config)
}

pub fn open_release_candidate(doc: Entity<DocModel>, items: Vec<ReleaseItem>, window: &mut Window, cx: &mut App) {
    (hooks().open_release_candidate)(doc, items, window, cx)
}

pub fn open_revision_history(doc: Entity<DocModel>, part_number: String, window: &mut Window, cx: &mut App) {
    (hooks().open_revision_history)(doc, part_number, window, cx)
}

pub fn note_viewed_config(tab: TabId, values: &ConfigValues) {
    (hooks().note_viewed_config)(tab, values)
}

pub fn last_viewed_config(tab: TabId) -> Option<ConfigValues> {
    (hooks().last_viewed_config)(tab)
}

pub fn create_version(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    (hooks().create_version)(doc, window, cx)
}

pub fn show_keyboard_shortcuts(cx: &mut App) {
    (hooks().show_keyboard_shortcuts)(cx)
}

pub fn show_shortcuts_page(opener: Option<Size<Pixels>>, cx: &mut App) {
    (hooks().show_shortcuts_page)(opener, cx)
}

pub fn key_chip(text: impl Into<SharedString>, style: KeyStyle) -> Div {
    (hooks().key_chip)(text.into(), style)
}

pub fn export_rules_link(id: &'static str) -> Stateful<Div> {
    (hooks().export_rules_link)(id)
}

pub fn parts_list_properties() -> bool {
    (hooks().parts_list_properties)()
}

pub fn set_parts_list_properties(on: bool) {
    (hooks().set_parts_list_properties)(on)
}

pub fn feature_dialog_width(type_id: &str) -> Option<u32> {
    (hooks().feature_dialog_width)(type_id)
}

pub fn set_feature_dialog_width(type_id: &str, width: u32) {
    (hooks().set_feature_dialog_width)(type_id, width)
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

pub fn variable_type_cell(id: impl Into<ElementId>, options: Vec<String>, current: usize, on_select: impl Fn(usize, &mut Window, &mut App) + 'static) -> Div {
    (hooks().variable_type_cell)(id.into(), options, current, Box::new(on_select))
}

pub fn compare_blend(doc: &Entity<DocModel>, cx: &App) -> Option<f32> {
    (hooks().compare_blend)(doc, cx)
}

pub fn compare_documents(doc: &Entity<DocModel>, cx: &App) -> Option<(Arc<Document>, ConfigValues, Arc<Document>, ConfigValues)> {
    (hooks().compare_documents)(doc, cx)
}

pub fn compare_panel(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<AnyElement> {
    (hooks().compare_panel)(doc, tab, cx)
}

pub fn compare_slider(doc: &Entity<DocModel>, cx: &App) -> Option<AnyElement> {
    (hooks().compare_slider)(doc, cx)
}

pub fn repair_open(doc: &Entity<DocModel>, cx: &App) -> bool {
    (hooks().repair_open)(doc, cx)
}

pub fn repair_close(doc: &Entity<DocModel>, cx: &mut App) {
    (hooks().repair_close)(doc, cx)
}

pub fn repair_panel(doc: &Entity<DocModel>, graphics: Option<AnyElement>, cx: &App) -> Option<AnyElement> {
    (hooks().repair_panel)(doc, graphics, cx)
}

pub fn materials_of(lib: &AddedLibrary) -> MaterialLibrary {
    (hooks().materials_of)(lib)
}

pub fn open_add_material_library(on_add: impl Fn(Option<AddedLibrary>, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    (hooks().open_add_material_library)(Box::new(on_add), window, cx)
}
