//! The documents start window: shown when noshape starts without a file and
//! whenever the last document window closes. Lists recent documents and the
//! `.noshape` files of the documents folder and user-added folders, as a
//! thumbnail grid or a sortable list, with search, create, open and per
//! document commands (rename, duplicate, reveal, trash, forget).

use crate::actions::{CloseWindow, NewDocument, OpenDocument};
use crate::start_library::{self as lib, DocEntry, Library, ScanInput};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use crate::os_menu::{OsContextMenuExt as _, OsMenu, OsMenuItem};
use crate::os_ui::OsDropdownExt as _;
use crate::ui::AlertKind;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, IconName, Sizable as _, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

actions!(noshape_start, [OpenSelected, RenameSelected, TrashSelected, FocusSearch, SelectNext, SelectPrev]);

const CONTEXT: &str = "StartWindow";

fn icon(data: &'static [u8]) -> Icon {
    Icon::empty().data(data)
}

pub fn init(cx: &mut App) {
    let ctx = Some("StartWindow && !Input");
    cx.bind_keys([
        KeyBinding::new("enter", OpenSelected, ctx),
        KeyBinding::new("f2", RenameSelected, ctx),
        KeyBinding::new("delete", TrashSelected, ctx),
        KeyBinding::new("right", SelectNext, ctx),
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("left", SelectPrev, ctx),
        KeyBinding::new("up", SelectPrev, ctx),
        KeyBinding::new("secondary-f", FocusSearch, Some(CONTEXT)),
        KeyBinding::new("/", FocusSearch, ctx),
    ]);
}

// ----------------------------------------------------------------- window

struct StartWindowGlobal {
    handle: AnyWindowHandle,
    view: WeakEntity<StartView>,
}
impl Global for StartWindowGlobal {}

/// Show the start window, or bring the existing one to the front.
pub fn show(cx: &mut App) {
    if let Some(g) = cx.try_global::<StartWindowGlobal>() {
        let (handle, view) = (g.handle, g.view.clone());
        if handle.update(cx, |_, window, _| window.activate_window()).is_ok() {
            let _ = view.update(cx, |v, cx| v.rescan(cx));
            return;
        }
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1200.), px(780.)), cx))),
        window_min_size: Some(size(px(760.), px(480.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..TitleBar::window_options()
    };
    match crate::os_menu::open_window(options, cx, |window, cx| {
        window.set_window_title("Documents — noshape");
        let view = cx.new(|cx| StartView::new(window, cx));
        let fh = view.read(cx).focus.clone();
        window.defer(cx, move |window, cx| fh.focus(window, cx));
        view
    }) {
        Ok((handle, view)) => cx.set_global(StartWindowGlobal { handle, view: view.downgrade() }),
        Err(e) => log::error!("could not open the start window: {e:#}"),
    }
}

/// Show the start window and ask for a new document's name (Ctrl+N from
/// any window).
pub fn show_and_create(cx: &mut App) {
    show(cx);
    let Some((handle, view)) = cx.try_global::<StartWindowGlobal>().map(|g| (g.handle, g.view.clone())) else { return };
    let _ = handle.update(cx, |_, window, cx| {
        let _ = view.update(cx, |v, cx| v.new_document(window, cx));
    });
}

/// Re-read the library and redraw (e.g. after a document window closed).
pub fn refresh(cx: &mut App) {
    if let Some(view) = cx.try_global::<StartWindowGlobal>().map(|g| g.view.clone()) {
        let _ = view.update(cx, |v, cx| {
            v.force_rescan(cx);
            cx.notify();
        });
    }
}

/// Called when a window closed: forgets the start window if it was that one.
pub fn forget_if(id: WindowId, cx: &mut App) -> bool {
    let is_start = cx.try_global::<StartWindowGlobal>().is_some_and(|g| g.handle.window_id() == id);
    if is_start {
        cx.remove_global::<StartWindowGlobal>();
    }
    is_start
}

// ----------------------------------------------------------------- state

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum ViewMode {
    Grid,
    List,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum SortKey {
    Modified,
    Name,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum Source {
    Recent,
    All,
    Folder(PathBuf),
    /// Documents with this label (Onshape's Labels).
    Label(String),
    /// Documents whose first author is the user (Onshape's "Created by me").
    Created,
    /// Trashed documents and folders (start_trash).
    Trash,
}

/// View choices remembered across sessions.
#[derive(Clone, Serialize, Deserialize)]
struct Prefs {
    view: ViewMode,
    sort: SortKey,
    descending: bool,
    source: Option<Source>,
    /// The documents table's optional columns, in order, and whether shown
    /// (Onshape's "+ Add" column panel; global).
    #[serde(default = "default_columns")]
    columns: Vec<(Col, bool)>,
    /// The folder tiles' order (the Folders row's sort menu).
    #[serde(default)]
    folder_sort: FolderSort,
    /// The right panel shown (Onshape keeps Details open by default).
    #[serde(default = "default_panel")]
    panel: Option<DocPanel>,
}

/// The documents table's optional columns (live documents/81).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum Col {
    Modified,
    ModifiedBy,
    OwnedBy,
    Type,
    Description,
}

impl Col {
    fn label(self) -> &'static str {
        match self {
            Col::Modified => "Modified",
            Col::ModifiedBy => "Modified by",
            Col::OwnedBy => "Owned by",
            Col::Type => "Type",
            Col::Description => "Description",
        }
    }
}

fn default_columns() -> Vec<(Col, bool)> {
    vec![(Col::Modified, true), (Col::ModifiedBy, true), (Col::OwnedBy, true), (Col::Type, false), (Col::Description, false)]
}

/// The Folders row's sort menu: "Name: A → Z", "Name: Z → A", "Last modified".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
enum FolderSort {
    #[default]
    NameAsc,
    NameDesc,
    Modified,
}

/// A column being dragged in the "+ Add" panel (its index).
#[derive(Clone)]
struct ColDrag(usize, SharedString);

impl Render for ColDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        div().px(px(8.)).py(px(4.)).bg(pal.panel).border_1().border_color(rgb(0xeeeeee)).shadow_sm().text_size(px(14.)).text_color(pal.text).child(self.1.clone())
    }
}

fn default_panel() -> Option<DocPanel> {
    Some(DocPanel::Details)
}

impl Default for Prefs {
    fn default() -> Self {
        Self { view: ViewMode::Grid, sort: SortKey::Modified, descending: true, source: None, columns: default_columns(), folder_sort: FolderSort::NameAsc, panel: default_panel() }
    }
}

fn prefs_path() -> PathBuf {
    crate::files::config_dir().join("start.json")
}

fn load_prefs() -> Prefs {
    std::fs::read(prefs_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub struct StartView {
    focus: FocusHandle,
    library: Library,
    docs: Vec<DocEntry>,
    loaded: bool,
    folders: Vec<PathBuf>,
    source: Source,
    view: ViewMode,
    sort: SortKey,
    descending: bool,
    search: Entity<InputState>,
    query: String,
    selected: Option<PathBuf>,
    /// Onshape's document labels and whether the nav section is expanded.
    labels: lib::Labels,
    labels_open: bool,
    /// The Details panel's "Search labels" field.
    label_search: Entity<InputState>,
    /// Right panel (Details / Versions and history) and the details read for it.
    panel: Option<DocPanel>,
    details: Option<(PathBuf, i64, lib::DocDetails)>,
    /// The "Folders" section of "Owned by me" is expanded.
    folders_open: bool,
    /// "Last opened by me" is expanded.
    recent_open: bool,
    /// A folder's own subfolder section is expanded (top-level "Folders" is
    /// `folders_open`, collapsed by default).
    sub_open: bool,
    /// Documents table columns, the "+ Add" panel (open, search field, the
    /// button's bounds) and the folder tiles' order.
    columns: Vec<(Col, bool)>,
    cols_open: bool,
    col_search: Entity<InputState>,
    cols_anchor: Rc<std::cell::Cell<Bounds<Pixels>>>,
    folder_sort: FolderSort,
    /// Folders pane height set by dragging its divider, and the drag
    /// (mouse y, height at the start).
    split_h: Option<f32>,
    split_drag: Option<(f32, f32)>,
    /// Height of the area under the page title (folders + list), measured.
    list_h: Rc<std::cell::Cell<f32>>,
    /// The names the user writes history under (Preferences name, OS full
    /// name, login): their documents show "me" and are "Created by me".
    me: Vec<String>,
    /// Trashed documents and folders.
    trash: crate::start_trash::Trash,
    /// Every folder under the scanned places (folder tiles).
    dirs: Vec<PathBuf>,
    /// The search box placeholder shown ("Search in <place>").
    placeholder: String,
    /// Advanced search (the ▾ in the search box): the panel, open or not,
    /// the applied query, the search box bounds (the panel hangs under
    /// it) and the documents' contents read for it (by path, with the
    /// modified time they were read at).
    adv: Entity<crate::start_search::AdvancedSearch>,
    adv_open: bool,
    adv_query: Option<crate::start_search::Query>,
    adv_anchor: Rc<std::cell::Cell<Bounds<Pixels>>>,
    /// The Properties panel's fields (for the selected document).
    props: Option<PropsEdit>,
    /// The Details panel's "Document labels" section is expanded.
    details_labels_open: bool,
    facts: std::collections::HashMap<PathBuf, (i64, crate::start_search::DocFacts)>,
    facts_task: Option<Task<()>>,
    scan_task: Option<Task<()>>,
    _poll: Task<()>,
    _subs: Vec<Subscription>,
}

impl Focusable for StartView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StartView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search documents"));
        let label_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search labels"));
        let col_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));
        let subs = vec![
            cx.subscribe(&col_search, |_, _, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    cx.notify();
                }
            }),
            cx.subscribe(&label_search, |_, _, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    cx.notify();
                }
            }),
            cx.subscribe_in(&search, window, |this: &mut Self, state, ev: &InputEvent, _, cx| {
                if let InputEvent::Change = ev {
                    this.query = state.read(cx).value().to_string();
                    cx.notify();
                }
            }),
            cx.observe_window_activation(window, |this: &mut Self, window, cx| {
                if window.is_window_active() {
                    this.rescan(cx);
                }
            }),
        ];
        // Pick up thumbnails and files changed by document windows or other apps.
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(4)).await;
                if this.update(cx, |t, cx| t.rescan(cx)).is_err() {
                    break;
                }
            }
        });
        let adv = cx.new(|cx| crate::start_search::AdvancedSearch::new(window, cx));
        let adv_sub = cx.subscribe(&adv, |this: &mut Self, _, ev: &crate::start_search::SearchEvent, cx| {
            match ev {
                crate::start_search::SearchEvent::Search(q) => {
                    this.adv_query = Some(q.clone()).filter(|q| *q != crate::start_search::Query::default());
                    this.adv_open = false;
                    this.load_facts(cx);
                }
                crate::start_search::SearchEvent::Reset => this.adv_query = None,
            }
            cx.notify();
        });
        let mut subs = subs;
        subs.push(adv_sub);
        let prefs = load_prefs();
        let source = prefs.source.clone().unwrap_or(if crate::files::recent().is_empty() { Source::All } else { Source::Recent });
        let mut this = Self {
            focus: cx.focus_handle(),
            library: Library::default(),
            docs: Vec::new(),
            loaded: false,
            folders: ScanInput::current().folders,
            source,
            view: prefs.view,
            sort: prefs.sort,
            descending: prefs.descending,
            search,
            query: String::new(),
            selected: None,
            labels: lib::Labels::load(),
            labels_open: false,
            label_search,
            panel: prefs.panel,
            details: None,
            folders_open: false,
            recent_open: true,
            sub_open: true,
            columns: prefs.columns.clone(),
            cols_open: false,
            col_search,
            cols_anchor: Rc::default(),
            folder_sort: prefs.folder_sort,
            split_h: None,
            split_drag: None,
            list_h: Rc::new(std::cell::Cell::new(600.)),
            me: my_names(),
            trash: crate::start_trash::Trash::load(),
            dirs: Vec::new(),
            placeholder: String::new(),
            adv,
            adv_open: false,
            adv_query: None,
            adv_anchor: Rc::default(),
            props: None,
            details_labels_open: true,
            facts: Default::default(),
            facts_task: None,
            scan_task: None,
            _poll: poll,
            _subs: subs,
        };
        this.rescan(cx);
        this
    }

    fn save_prefs(&self) {
        let prefs = Prefs { view: self.view, sort: self.sort, descending: self.descending, source: Some(self.source.clone()), columns: self.columns.clone(), folder_sort: self.folder_sort, panel: self.panel };
        let _ = std::fs::write(prefs_path(), serde_json::to_vec_pretty(&prefs).unwrap_or_default());
    }

    /// Re-read the file system in the background.
    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        if self.scan_task.is_some() {
            return;
        }
        let library = self.library.clone();
        self.scan_task = Some(cx.spawn(async move |this, cx| {
            let (input, result) = cx
                .background_spawn(async move {
                    let input = ScanInput::current();
                    let result = library.scan(&input);
                    (input, result)
                })
                .await;
            let _ = this.update(cx, |t, cx| {
                t.scan_task = None;
                t.loaded = true;
                for img in result.dropped {
                    cx.drop_image(img, None);
                }
                t.docs = result.docs;
                t.dirs = result.dirs;
                t.trash = crate::start_trash::Trash::load();
                t.folders = input.folders;
                if let Source::Folder(f) = &t.source
                    && !f.is_dir()
                {
                    t.source = Source::All;
                }
                cx.notify();
            });
        }));
    }

    fn force_rescan(&mut self, cx: &mut Context<Self>) {
        self.scan_task = None;
        self.rescan(cx);
    }

    fn in_source(&self, d: &DocEntry, source: &Source) -> bool {
        if d.trashed != (*source == Source::Trash) {
            return false;
        }
        match source {
            Source::Recent => d.recent_rank.is_some(),
            Source::All | Source::Trash => true,
            // A folder lists its own documents (subfolders are tiles).
            Source::Folder(f) => d.folder() == canon(f),
            Source::Label(l) => self.labels.has(&d.path, l),
            Source::Created => d.owner.is_empty() || self.me.contains(&d.owner),
        }
    }

    /// Advanced search: read what the documents hold (tabs, part
    /// properties) off the UI thread, for the documents not read yet.
    fn load_facts(&mut self, cx: &mut Context<Self>) {
        if !self.adv_query.as_ref().is_some_and(|q| q.needs_facts()) {
            return;
        }
        let todo: Vec<(PathBuf, i64)> = self.docs.iter().filter(|d| !d.trashed && self.facts.get(&d.path).is_none_or(|f| f.0 != d.modified)).map(|d| (d.path.clone(), d.modified)).collect();
        if todo.is_empty() {
            return;
        }
        self.facts_task = Some(cx.spawn(async move |this, cx| {
            let read = cx.background_spawn(async move { todo.into_iter().filter_map(|(p, m)| crate::start_search::facts(&p).map(|f| (p, (m, f)))).collect::<Vec<_>>() }).await;
            let _ = this.update(cx, |t, cx| {
                t.facts.extend(read);
                t.facts_task = None;
                cx.notify();
            });
        }));
    }

    /// Does a document pass the applied advanced search?
    fn adv_matches(&self, d: &DocEntry) -> bool {
        let Some(q) = &self.adv_query else { return true };
        let labels: Vec<String> = self.labels.labels.iter().filter(|l| self.labels.has(&d.path, l)).cloned().collect();
        crate::start_search::matches(q, &d.name, &d.description, &d.owner, &labels, self.facts.get(&d.path).map(|f| &f.1))
    }

    /// Documents shown: current source, filtered by the search, sorted.
    fn visible(&self) -> Vec<DocEntry> {
        let q = self.query.trim().to_lowercase();
        let mut v: Vec<DocEntry> = self
            .docs
            .iter()
            .filter(|d| self.in_source(d, &self.source))
            .filter(|d| self.adv_matches(d))
            .filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q) || d.path.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&q)))
            .cloned()
            .collect();
        match self.sort {
            SortKey::Name => v.sort_by_key(|d| d.name.to_lowercase()),
            SortKey::Modified => v.sort_by_key(|d| d.modified),
        }
        if self.descending {
            v.reverse();
        }
        v
    }

    fn set_source(&mut self, s: Source, cx: &mut Context<Self>) {
        self.source = s;
        self.selected = None;
        self.save_prefs();
        cx.notify();
    }

    fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        if self.sort == key {
            self.descending = !self.descending;
        } else {
            self.sort = key;
            self.descending = key == SortKey::Modified;
        }
        self.save_prefs();
        cx.notify();
    }

    fn set_view(&mut self, v: ViewMode, cx: &mut Context<Self>) {
        self.view = v;
        self.save_prefs();
        cx.notify();
    }

    fn selected_entry(&self) -> Option<DocEntry> {
        let sel = self.selected.as_ref()?;
        self.docs.iter().find(|d| &d.path == sel).cloned()
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let vis = self.visible();
        if vis.is_empty() {
            return;
        }
        let cur = self.selected.as_ref().and_then(|s| vis.iter().position(|d| &d.path == s));
        let next = match cur {
            None => 0,
            Some(i) => (i as isize + delta).clamp(0, vis.len() as isize - 1) as usize,
        };
        self.selected = Some(vis[next].path.clone());
        cx.notify();
    }

    // ------------------------------------------------------------- commands

    /// Onshape's message bubble at the top center of the page (replaces the
    /// previous one; closes with its × or after 8 s).
    fn notify(window: &mut Window, cx: &mut App, kind: AlertKind, message: impl Into<String>) {
        Self::toast(window, cx, Toast { kind: kind.into(), bold: None, text: message.into(), restore: None, brief: false });
    }

    /// Show a message bubble at the top of the page for 8 s.
    fn toast(window: &mut Window, cx: &mut App, toast: Toast) {
        let id = cx.default_global::<StartToast>().1 + 1;
        let secs = if toast.brief { 3 } else { 8 };
        *cx.default_global::<StartToast>() = StartToast(Some(toast), id);
        window.refresh();
        let handle = window.window_handle();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(secs)).await;
            cx.update(|cx| {
                if cx.default_global::<StartToast>().1 == id {
                    cx.default_global::<StartToast>().0 = None;
                    let _ = handle.update(cx, |_, window, _| window.refresh());
                }
            });
        })
        .detach();
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        match crate::files::open_document_window(path.clone(), None, gpu, cx) {
            Ok(()) => self.force_rescan(cx),
            Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not open document: {e:#}")),
        }
    }

    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = match &self.source {
            Source::Folder(f) => f.clone(),
            _ => crate::files::documents_dir(),
        };
        let default = lib::next_untitled(&start);
        let location = Location::new(start);
        let chosen = location.chosen.clone();
        let view = cx.entity();
        let labels = LabelPick { all: self.labels.labels.clone(), chosen: Rc::default() };
        let picked = labels.chosen.clone();
        let prompt = NamePrompt { title: "New document", ok_label: "Create document", label: "Document name", default, hint: None, location: Some(("Document location", location)), labels: Some(labels) };
        prompt_name(prompt, window, cx, move |name, window, cx| {
            let clean = lib::sanitize_name(&name);
            if clean.is_empty() {
                Self::notify(window, cx, AlertKind::Warning, "Enter a name for the document");
                return false;
            }
            let dir = chosen.borrow().clone();
            let path = dir.join(format!("{clean}.{}", lib::EXT));
            if path.exists() {
                Self::notify(window, cx, AlertKind::Warning, format!("\"{clean}\" already exists in this folder"));
                return false;
            }
            view.update(cx, |this, cx| {
                for l in picked.borrow().iter() {
                    this.labels.toggle(&path, l);
                }
                this.labels.save();
                this.selected = Some(path.clone());
                this.open_path(path, window, cx);
            });
            true
        });
    }

    /// The folder new documents, folders and imports go to: the folder being
    /// shown, else the documents folder.
    fn current_place(&self) -> PathBuf {
        match &self.source {
            Source::Folder(f) => f.clone(),
            _ => crate::files::documents_dir(),
        }
    }

    /// Create ▾ › Import files…: a noshape document opens; a CAD or mesh file
    /// becomes a new document named after it, with the file translated into a
    /// Part Studio (Onshape's import from the documents page).
    fn import_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let handle = window.window_handle();
        let place = self.current_place();
        // Onshape's picker takes any file: CAD files become a Part Studio (and
        // the file in "CAD Imports"), DXF / DWG a drawing plus the file, others
        // a file tab.
        crate::files::request_open_path("Import files", "All files", &["*"], window, cx, move |path, cx| {
            let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let result = if ext == lib::EXT {
                crate::files::open_document_window(path, None, gpu, cx)
            } else {
                (|| {
                    let mut doc = noshape_model::Document::new();
                    let units = doc.settings.units;
                    let (name, file) = crate::workspace::import_names(&path);
                    let bytes = std::fs::read(&path)?;
                    let stored = noshape_model::file_tab::FileTab::new(&file, &bytes, noshape_model::file_tab::now());
                    doc.tabs.clear();
                    doc.settings.tab_order.clear();
                    match stored.kind() {
                        noshape_model::file_tab::FileKind::Cad => {
                            let foreign = crate::workspace::read_foreign(&path, &units)?;
                            let mut ps = noshape_model::PartStudio::new();
                            crate::workspace::add_import_feature(&mut ps, &file, foreign, &units);
                            doc.add_tab(&name, noshape_model::TabContent::PartStudio(ps));
                            let folder = crate::file_tab_view::cad_imports_folder(&mut doc);
                            crate::file_tab_view::add_file_tab(&mut doc, stored, Some(folder), None);
                        }
                        noshape_model::file_tab::FileKind::Drawing2d => {
                            let f = crate::file_tab_view::add_file_tab(&mut doc, stored, None, None);
                            let dr = crate::file_tab_view::drawing_from_cad(&bytes).map_err(|e| anyhow::anyhow!(e))?;
                            let id = doc.add_tab(&name, noshape_model::TabContent::Drawing(dr));
                            crate::workspace::place_after(&mut doc, id, Some(f));
                        }
                        _ => {
                            crate::file_tab_view::add_file_tab(&mut doc, stored, None, None);
                        }
                    }
                    let clean = lib::sanitize_name(&name);
                    let mut target = place.join(format!("{clean}.{}", lib::EXT));
                    let mut n = 2;
                    while target.exists() {
                        target = place.join(format!("{clean} ({n}).{}", lib::EXT));
                        n += 1;
                    }
                    crate::files::open_document_window(target, Some(doc), gpu, cx)
                })()
            };
            if let Err(e) = result {
                let msg = format!("Import failed: {e:#}");
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, cx| Self::notify(window, cx, AlertKind::Error, msg));
                });
            }
            view.update(cx, |this, cx| this.force_rescan(cx));
        });
    }

    /// Create ▾ › Folder…: a new folder in the current place.
    fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.current_place();
        let default = (0..)
            .map(|i| if i == 0 { "Untitled folder".to_string() } else { format!("Untitled folder ({i})") })
            .find(|n| !start.join(n).exists())
            .unwrap_or_default();
        let location = Location::new(start);
        let chosen = location.chosen.clone();
        let view = cx.entity();
        let prompt = NamePrompt { title: "New folder", ok_label: "Create", label: "Folder name", default, hint: None, location: Some(("Folder location", location)), labels: None };
        prompt_name(prompt, window, cx, move |name, window, cx| {
            let place = chosen.borrow().clone();
            let clean = lib::sanitize_name(&name);
            if clean.is_empty() {
                Self::notify(window, cx, AlertKind::Warning, "Enter a name for the folder");
                return false;
            }
            let path = place.join(&clean);
            if path.exists() {
                Self::notify(window, cx, AlertKind::Warning, format!("\"{clean}\" already exists in this folder"));
                return false;
            }
            if let Err(e) = std::fs::create_dir_all(&path) {
                Self::notify(window, cx, AlertKind::Error, format!("Could not create the folder: {e}"));
                return false;
            }
            view.update(cx, |this, cx| {
                this.folders_open = true;
                this.force_rescan(cx);
            });
            Self::toast(window, cx, Toast { kind: os::BubbleKind::Success, bold: None, text: "Folder created.".into(), restore: None, brief: false });
            true
        });
    }

    fn rename(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if crate::files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{}: Close the document's window before renaming it", d.name));
            return;
        }
        let view = cx.entity();
        let hint = format!("Renames {}", d.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
        let prompt = NamePrompt { title: "Rename document", ok_label: "Rename", label: "Document name", default: d.name.clone(), hint: Some(hint), location: None, labels: None };
        prompt_name(prompt, window, cx, move |name, window, cx| match lib::rename_document(&d.path, &name) {
            Ok(dest) => {
                crate::files::replace_recent(&d.path, &dest);
                view.update(cx, |this, cx| {
                    this.labels.moved(&d.path, &dest);
                    this.labels.save();
                    this.selected = Some(dest.canonicalize().unwrap_or(dest));
                    this.force_rescan(cx);
                });
                true
            }
            Err(e) => {
                Self::notify(window, cx, AlertKind::Error, format!("Rename failed: {e:#}"));
                false
            }
        });
    }

    fn duplicate(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        let path = d.path.clone();
        let name = d.name.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { lib::duplicate_document(&path, &name) }).await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(dest) => {
                    this.selected = Some(dest.canonicalize().unwrap_or(dest));
                    this.force_rescan(cx);
                }
                Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Duplicate failed: {e:#}")),
            });
        })
        .detach();
    }

    fn trash(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if d.trashed {
            if let Some(id) = self.trash.item_of(&d.path).map(|i| i.id.clone()) {
                self.confirm_delete_forever(id, window, cx);
            }
            return;
        }
        if crate::files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{}: Close the document's window before moving it to the trash", d.name));
            return;
        }
        self.confirm_trash(d.path, d.name, false, window, cx);
    }

    /// The bordered list of the items a trash dialog acts on (live
    /// documents/70): 1 px `#ccc`, one row per item with its icon.
    fn selection_box(name: &str, folder: bool) -> Div {
        let pal = p();
        div().mt(px(14.)).ml(px(10.)).py(px(6.)).px(px(22.)).border_1().border_color(rgb(0xcccccc)).child(
            h_flex()
                .gap(px(8.))
                .text_size(px(14.))
                .text_color(pal.text)
                .child(os::glyph(if folder { os::GLYPH_FOLDER } else { os::GLYPH_DOCUMENT }, 18., pal.text))
                .child(name.to_string()),
        )
    }

    /// Onshape's "Send to trash" confirmation (live documents/47): "1
    /// selection will be sent to the trash." (bold) + "Show details" (the
    /// item list), **Send to trash** / Cancel.
    fn confirm_trash(&mut self, path: PathBuf, name: String, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        if folder && self.docs.iter().any(|d| d.path.starts_with(&path) && crate::files::is_document_open(&d.path, cx)) {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the documents in this folder before moving it to the trash"));
            return;
        }
        let view = cx.entity();
        let shown = Rc::new(std::cell::Cell::new(false));
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            let (view, path, name) = (view.clone(), path.clone(), name.clone());
            let s2 = shown.clone();
            os::modal(dialog, "Send to trash", 598.)
                .child(
                    os::modal_body()
                        .child(
                            h_flex()
                                .gap(px(10.))
                                .text_size(px(14.))
                                .child(div().font_weight(FontWeight::BOLD).text_color(pal.text).child("1 selection will be sent to the trash."))
                                .child(
                                    div()
                                        .id("trash-details")
                                        .cursor_pointer()
                                        .text_color(pal.info_text)
                                        .hover(|d| d.underline())
                                        .child(if shown.get() { "Hide details" } else { "Show details" })
                                        .on_click(move |_, window, _| {
                                            s2.set(!s2.get());
                                            window.refresh();
                                        }),
                                ),
                        )
                        .when(shown.get(), |d| d.child(Self::selection_box(&name, folder))),
                )
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Send to trash"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let (path, name) = (path.clone(), name.clone());
                    view.update(cx, |this, cx| this.send_to_trash(path, name, folder, window, cx));
                    true
                })
        });
    }

    fn send_to_trash(&mut self, path: PathBuf, name: String, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        match self.trash.send(&path, &name, folder) {
            Ok(item) => {
                for d in self.docs.iter().filter(|d| d.path == item.original || (folder && d.path.starts_with(&item.original))) {
                    crate::files::remove_recent(&d.path);
                }
                if folder {
                    let mut list = crate::files::watched_folders();
                    list.retain(|f| canon(f) != item.original);
                    crate::files::set_watched_folders(&list);
                    if matches!(&self.source, Source::Folder(f) if canon(f).starts_with(&item.original)) {
                        self.source = Source::All;
                        self.save_prefs();
                    }
                }
                self.selected = None;
                Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(name), text: " has been sent to the trash and will be automatically deleted after 30 days.".into(), restore: Some(item.id), brief: false });
                self.force_rescan(cx);
            }
            Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not move to trash: {e:#}")),
        }
    }

    /// Trash row menu › Restore and the toast's Restore link.
    fn restore_trashed(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let item = self.trash.get(&id).cloned();
        match self.trash.restore(&id) {
            Ok(dest) => {
                // Live documents/86: "<name> has been restored." (brief); a
                // restored document's Modified time becomes the restore time.
                if let Some(item) = &item {
                    if !item.folder
                        && let Ok(mut store) = noshape_store::Document::open(&dest)
                    {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
                        let _ = store.set_meta("modified", Some(now.to_string().as_bytes()));
                        let _ = store.checkpoint();
                    }
                    Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(item.name.clone()), text: " has been restored.".into(), restore: None, brief: true });
                }
                // A folder from outside the documents folder is listed again.
                if let Some(item) = item.filter(|i| i.folder && !i.original.starts_with(canon(&crate::files::documents_dir())))
                    && item.original.parent().is_some_and(|p| !self.dirs.iter().any(|d| d == p))
                {
                    let mut list = crate::files::watched_folders();
                    list.push(dest.clone());
                    crate::files::set_watched_folders(&list);
                    self.folders = ScanInput::current().folders;
                }
                self.selected = None;
                self.force_rescan(cx);
            }
            Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not restore: {e:#}")),
        }
    }

    /// Trash row menu › Delete: Onshape's "Delete forever?" (live
    /// documents/70): bold warning, the item list, red **Delete forever**.
    fn confirm_delete_forever(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.trash.get(&id).cloned() else { return };
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            let (view, id, name) = (view.clone(), id.clone(), item.name.clone());
            os::modal(dialog, "Delete forever?", 598.)
                .child(
                    os::modal_body()
                        .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("1 selection will be permanently deleted from your trash; you can't undo this action."))
                        .child(Self::selection_box(&item.name, item.folder)),
                )
                .footer(os::modal_footer().child(os::modal_ok(os::danger_button("ok", "Delete forever", cx))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let (id, name) = (id.clone(), name.clone());
                    view.update(cx, |this, cx| match this.trash.delete(&id) {
                        Ok(()) => {
                            this.selected = None;
                            Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(name), text: " has been permanently deleted.".into(), restore: None, brief: false });
                            this.force_rescan(cx);
                        }
                        Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not delete: {e:#}")),
                    });
                    true
                })
        });
    }

    /// Trash › "Empty trash…" (live documents/88): "Empty trash?", the bold
    /// warning, red **Empty trash** / Cancel; no toast afterwards.
    fn confirm_empty_trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.trash.items.is_empty() {
            return;
        }
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            let view = view.clone();
            os::modal(dialog, "Empty trash?", 598.)
                .child(os::modal_body().child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("This will permanently delete all items from your trash; you can't undo this action.")))
                .footer(os::modal_footer().child(os::modal_ok(os::danger_button("ok", "Empty trash", cx))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        if let Err(e) = this.trash.empty() {
                            Self::notify(window, cx, AlertKind::Error, format!("Could not empty the trash: {e:#}"));
                        }
                        this.selected = None;
                        this.force_rescan(cx);
                    });
                    true
                })
        });
    }

    // ------------------------------------------------------------- folders

    /// Onshape's folder menu (live documents/46, minus cloud items): Open,
    /// Rename, Details, Move to…, Send to trash; in the Trash: Details,
    /// Restore, Delete.
    fn folder_menu(view: Entity<StartView>, f: PathBuf, name: String, trash_id: Option<String>, menu: OsMenu) -> OsMenu {
        let item = |label: &'static str, f: &PathBuf, act: fn(&mut StartView, PathBuf, &mut Window, &mut Context<StartView>)| {
            let (view, f) = (view.clone(), f.clone());
            OsMenuItem::new(label).on_click(move |_, window, cx| {
                let f = f.clone();
                view.update(cx, |this, cx| act(this, f, window, cx));
            })
        };
        let menu = menu.list_no_icons();
        if let Some(id) = trash_id {
            let (v1, v2, i1, i2) = (view.clone(), view.clone(), id.clone(), id);
            return menu
                .item(item("Details", &f, |t, f, _, cx| t.show_folder_details(f, cx)))
                .item(OsMenuItem::new("Restore").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.restore_trashed(i1.clone(), window, cx))))
                .item(OsMenuItem::new("Delete").on_click(move |_, window, cx| v2.update(cx, |t, cx| t.confirm_delete_forever(i2.clone(), window, cx))));
        }
        let v = view.clone();
        let f2 = f.clone();
        menu.item(item("Open", &f, |t, f, _, cx| t.set_source(Source::Folder(f), cx)))
            .item(item("Rename", &f, |t, f, w, cx| t.rename_folder(f, w, cx)))
            .item(item("Details", &f, |t, f, _, cx| t.show_folder_details(f, cx)))
            .item(item("Move to…", &f, |t, f, w, cx| t.move_folder(f, w, cx)))
            .item(OsMenuItem::new("Send to trash").on_click(move |_, window, cx| {
                let (f, name) = (f2.clone(), name.clone());
                v.update(cx, |t, cx| t.confirm_trash(f, name, true, window, cx));
            }))
    }

    /// The selected folder tile: (name, created, modified) in unix ms.
    fn selected_folder(&self) -> Option<(String, i64, i64)> {
        let f = self.selected.as_ref()?;
        let name = match self.trash.item_of(f) {
            Some(t) if t.folder => t.name.clone(),
            Some(_) => return None,
            None if f.is_dir() => folder_label(f),
            None => return None,
        };
        let md = std::fs::metadata(f).ok();
        let ms = |t: Option<std::time::SystemTime>| t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
        let modified = ms(md.as_ref().and_then(|m| m.modified().ok()));
        let created = ms(md.as_ref().and_then(|m| m.created().ok())).max(0);
        Some((name, if created > 0 { created } else { modified }, modified))
    }

    fn show_folder_details(&mut self, f: PathBuf, cx: &mut Context<Self>) {
        self.selected = Some(f);
        self.panel = Some(DocPanel::Details);
        cx.notify();
    }

    /// Paths under `old` now live under `new` (recent list, labels, the
    /// place shown).
    fn folder_moved(&mut self, old: &Path, new: &Path) {
        for d in self.docs.iter().filter(|d| d.path.starts_with(old)) {
            if let Ok(rest) = d.path.strip_prefix(old) {
                let dest = new.join(rest);
                crate::files::replace_recent(&d.path, &dest);
                self.labels.moved(&d.path, &dest);
            }
        }
        self.labels.save();
        let mut list = crate::files::watched_folders();
        for w in list.iter_mut() {
            if canon(w) == old {
                *w = new.to_path_buf();
            }
        }
        crate::files::set_watched_folders(&list);
        if let Source::Folder(f) = &self.source
            && let Ok(rest) = canon(f).strip_prefix(old)
        {
            self.source = Source::Folder(new.join(rest));
            self.save_prefs();
        }
        self.folders = ScanInput::current().folders;
    }

    /// Folder menu › Rename: Onshape's name prompt.
    fn rename_folder(&mut self, f: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let prompt = NamePrompt { title: "Rename folder", ok_label: "Rename", label: "Folder name", default: folder_label(&f), hint: None, location: None, labels: None };
        prompt_name(prompt, window, cx, move |name, window, cx| {
            let clean = lib::sanitize_name(&name);
            let Some(parent) = f.parent() else { return false };
            if clean.is_empty() {
                Self::notify(window, cx, AlertKind::Warning, "Enter a name for the folder");
                return false;
            }
            let dest = parent.join(&clean);
            if dest == f {
                return true;
            }
            if dest.exists() {
                Self::notify(window, cx, AlertKind::Warning, format!("\"{clean}\" already exists in this folder"));
                return false;
            }
            if let Err(e) = std::fs::rename(&f, &dest) {
                Self::notify(window, cx, AlertKind::Error, format!("{}: {e}", "Rename failed"));
                return false;
            }
            view.update(cx, |this, cx| {
                this.folder_moved(&f, &dest);
                this.selected = Some(canon(&dest));
                this.force_rescan(cx);
            });
            true
        });
    }

    /// Folder menu › Move to…: the folder browser (not into itself).
    fn move_folder(&mut self, f: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let mut location = Location::new(f.parent().map(Path::to_path_buf).unwrap_or_else(crate::files::documents_dir));
        location.exclude = Some(f.clone());
        let location = Rc::new(location);
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (loc, view, f) = (location.clone(), view.clone(), f.clone());
            os::modal(dialog, "Move to", 460.)
                .child(os::modal_body().child(os::field_label("Location")).child(location.render()))
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Move"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let dir = loc.chosen.borrow().clone();
                    let Some(file_name) = f.file_name() else { return false };
                    let dest = dir.join(file_name);
                    if dest == f {
                        return true;
                    }
                    if dir.starts_with(&f) || dest.exists() {
                        Self::notify(window, cx, AlertKind::Warning, format!("\"{}\" can't be moved there", folder_label(&f)));
                        return false;
                    }
                    if let Err(e) = std::fs::rename(&f, &dest) {
                        Self::notify(window, cx, AlertKind::Error, format!("{}: {e}", "Move failed"));
                        return false;
                    }
                    view.update(cx, |this, cx| {
                        this.folder_moved(&f, &dest);
                        this.selected = Some(canon(&dest));
                        this.force_rescan(cx);
                    });
                    true
                })
        });
    }

    fn forget(&mut self, d: DocEntry, cx: &mut Context<Self>) {
        crate::files::remove_recent(&d.path);
        self.force_rescan(cx);
    }

    // ------------------------------------------------------------- render

    // ------------------------------------------------------------- labels

    /// Create ▾ › Label… / "Create new label": Onshape's name prompt; the new
    /// label is applied to `apply_to` when given.
    fn create_label(&mut self, apply_to: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let prompt = NamePrompt { title: "Create label", ok_label: "Create", label: "Label name", default: String::new(), hint: None, location: None, labels: None };
        prompt_name(prompt, window, cx, move |name, window, cx| {
            let name = name.trim().to_string();
            view.update(cx, |this, cx| {
                if !this.labels.create(&name) {
                    Self::notify(window, cx, AlertKind::Warning, format!("A label named \"{name}\" already exists"));
                    return false;
                }
                if let Some(p) = &apply_to {
                    this.labels.toggle(p, &name);
                }
                this.labels.save();
                cx.notify();
                true
            })
        });
    }

    fn toggle_label(&mut self, path: &Path, label: &str, cx: &mut Context<Self>) {
        self.labels.toggle(path, label);
        self.labels.save();
        cx.notify();
    }

    /// Row menu › Labels…: check the document's labels.
    fn labels_dialog(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search labels"));
        window.subscribe(&search, cx, |_, ev: &InputEvent, window, _| {
            if let InputEvent::Change = ev {
                window.refresh();
            }
        })
        .detach();
        window.open_dialog(cx, move |dialog, _, cx| {
            let labels = view.read(cx).labels.clone();
            let body = Self::label_checklist(&view, &d.path, &search, &labels, cx);
            os::modal(dialog, "Labels", 400.)
                .child(os::modal_body().child(body))
                .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("close", "Close", cx))))
        });
    }

    /// Onshape's labels box (live documents-details-panel): the 30 px
    /// "Search labels" bar, the labels as checkbox rows for `path` (filtered
    /// by the search), and "Create new label".
    /// `labels` is passed in: the Details panel draws this while the view is
    /// being rendered, so it can't read the view.
    fn label_checklist(view: &Entity<StartView>, path: &Path, search: &Entity<InputState>, labels: &lib::Labels, cx: &App) -> AnyElement {
        let pal = p();
        let labels = labels.clone();
        let query = search.read(cx).value().trim().to_lowercase();
        let mut col = v_flex().gap(px(2.)).child(
            // Live documents/24: the field 20 px into the box, italic placeholder.
            div()
                .mx(px(15.))
                .mt(px(20.))
                .mb(px(5.))
                .h(px(30.))
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(rgb(0xffffff))
                .child(Input::new(search).appearance(false).h(px(28.)).text_size(px(14.)).italic()),
        );
        if labels.labels.is_empty() {
            col = col.child(div().pt(px(6.)).pb(px(34.)).text_center().text_size(px(14.)).text_color(pal.text).child("You have no labels yet"));
        }
        for (k, l) in labels.labels.iter().enumerate().filter(|(_, l)| query.is_empty() || l.to_lowercase().contains(&query)) {
            let on = labels.has(path, l);
            let (v, p2, l2) = (view.clone(), path.to_path_buf(), l.clone());
            col = col.child(os::checkbox(("label-check", k), l.clone(), on).on_click(move |_, window, cx| {
                v.update(cx, |t, cx| t.toggle_label(&p2, &l2, cx));
                window.refresh();
            }));
        }
        let (v, p2) = (view.clone(), path.to_path_buf());
        col.child(
            h_flex()
                .id("label-create")
                .h(px(31.))
                .gap(px(6.))
                .pl(px(20.))
                .border_t_1()
                .border_color(rgb(0xcccccc))
                .cursor_pointer()
                .child(os::glyph(os::GLYPH_LABEL_ADD, 20., pal.text))
                .child(div().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Create new label"))
                .on_click(move |_, window, cx| v.update(cx, |t, cx| t.create_label(Some(p2.clone()), window, cx))),
        )
        .into_any_element()
    }

    /// The Labels section of the left nav: "Labels" with its caret and the
    /// create-label button, then one row per label when expanded.
    fn render_labels_nav(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = self.labels_open;
        let mut col = v_flex().child(
            h_flex()
                .id("nav-labels")
                .relative()
                .h(px(32.))
                .pl(px(6.))
                .pr(px(8.))
                .gap(px(4.))
                .cursor_pointer()
                .text_size(px(14.67))
                .text_color(pal.text)
                .hover(|d| d.bg(rgb(0xf5f5f5)))
                .child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 10., pal.text))
                .child(os::glyph(os::GLYPH_LABEL, 20., pal.text))
                .child(div().flex_1().child("Labels"))
                .child(
                    div()
                        .id("nav-label-add")
                        .child(os::glyph(os::GLYPH_LABEL_ADD, 20., pal.text))
                        .tooltip(crate::ui::tooltip("Create label"))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.create_label(None, window, cx);
                        })),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.labels_open = !this.labels_open;
                    cx.notify();
                })),
        );
        if open {
            for (k, l) in self.labels.labels.clone().into_iter().enumerate() {
                let source = Source::Label(l.clone());
                let active = self.source == source;
                let color = if active { pal.accent } else { pal.text };
                col = col.child(
                    h_flex()
                        .id(("nav-label", k))
                        .relative()
                        .h(px(30.))
                        .pl(px(40.))
                        .gap(px(7.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(color)
                        .when(active, |d| d.font_weight(FontWeight::SEMIBOLD).child(div().absolute().left_0().top_0().bottom_0().w(px(4.)).bg(pal.accent)))
                        .when(!active, |d| d.hover(|d| d.bg(rgb(0xf5f5f5))))
                        .child(os::glyph(os::GLYPH_LABEL, 16., color))
                        .child(div().truncate().child(l))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_source(source.clone(), cx))),
                );
            }
        }
        col.into_any_element()
    }


    // ------------------------------------------------------------- right panels

    /// Details / Versions and history of `path` (read once per change).
    fn details_of(&mut self, path: &Path) -> Option<lib::DocDetails> {
        let modified = self.docs.iter().find(|d| d.path == path).map(|d| d.modified).unwrap_or(0);
        if let Some((p, m, d)) = &self.details
            && p == path
            && *m == modified
        {
            return Some(d.clone());
        }
        let d = lib::doc_details(path).ok()?;
        self.details = Some((path.to_path_buf(), modified, d.clone()));
        Some(d)
    }

    fn toggle_panel(&mut self, panel: DocPanel, cx: &mut Context<Self>) {
        self.panel = if self.panel == Some(panel) { None } else { Some(panel) };
        self.save_prefs();
        cx.notify();
    }

    fn show_panel(&mut self, d: DocEntry, panel: DocPanel, cx: &mut Context<Self>) {
        self.selected = Some(d.path);
        self.panel = Some(panel);
        cx.notify();
    }

    /// Onshape's right strip (live, documents/24): a 40 px column of 20 px
    /// icons; the open panel's icon has a 4 px blue bar.
    fn render_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let btn = |id: &'static str, icon: &'static str, tip: &'static str, panel: DocPanel, cx: &mut Context<Self>| {
            let on = self.panel == Some(panel);
            crate::ui::rail_button(id, icon, on, false, cx).tooltip(crate::ui::tooltip(tip)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_panel(panel, cx)))
        };
        v_flex()
            .w(px(40.))
            .h_full()
            .flex_none()
            .pt(px(36.))
            .gap(px(9.))
            .border_l_1()
            .border_color(pal.border)
            .child(btn("doc-strip-details", "info", "Details", DocPanel::Details, cx))
            // Live documents/24 vs 26: the document panels show only while a document is
            // selected (113: details, (share), properties, versions, notes).
            .when(self.selected_entry().is_some(), |s| {
                s.child(btn("doc-strip-properties", "properties", "Properties", DocPanel::Properties, cx))
                    .child(btn("doc-strip-versions", "branch", "Versions and history", DocPanel::Versions, cx))
                    .child(btn("doc-strip-notes", "doc-notes", "Document notes", DocPanel::Notes, cx))
            })
            .into_any_element()
    }

    /// The open right panel: Details (name, thumbnail, Owner, Document
    /// description, Created by, Last modified by) or Versions and history.
    fn render_doc_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.panel?;
        let pal = p();
        let entry = self.selected_entry();
        let details = entry.as_ref().and_then(|e| self.details_of(&e.path));
        let folder = if entry.is_none() { self.selected_folder() } else { None };
        // Onshape writes the user as "me".
        let me = self.me.clone();
        let who = move |name: &str| if name.is_empty() || me.iter().any(|m| m == name) { "me".to_string() } else { name.to_string() };
        let title = match panel {
            DocPanel::Details => "Details",
            DocPanel::Versions => "Versions and history",
            DocPanel::Properties => "Properties",
            DocPanel::Notes => "Document notes",
        };
        let name_row = |name: String| {
            h_flex()
                .h(px(32.))
                .px(px(15.))
                .mt(px(11.))
                .gap(px(4.))
                .child(os::glyph(os::GLYPH_DOCUMENT, 20., pal.text))
                .child(div().truncate().text_size(px(17.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(name))
        };
        // Live documents/24: a section is 53 px with one value (label and value
        // 20 px apart, values 25 px apart), `#ccc` rule above.
        let section = |label: &'static str, lines: Vec<String>| {
            v_flex()
                .pt(px(4.))
                .pb(px(7.))
                .border_t_1()
                .border_color(rgb(0xcccccc))
                .child(div().line_height(px(21.)).text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(label))
                .children(lines.into_iter().enumerate().map(|(i, l)| div().when(i > 0, |d| d.pt(px(4.))).line_height(px(20.)).text_size(px(13.33)).font_weight(FontWeight::LIGHT).text_color(pal.text).child(l)))
        };
        let body: AnyElement = match (&entry, &details) {
            (Some(_), Some(d)) if panel == DocPanel::Notes => {
                // Live documents/114: a 140 px grey notepad and "No notes available".
                if d.notes.trim().is_empty() {
                    v_flex()
                        .child(name_row(d.name.clone()))
                        .child(v_flex().items_center().pt(px(43.)).gap(px(20.)).child(svg().path(crate::assets::icon_path("notes-empty")).size(px(140.)).text_color(rgb(0xebebeb))).child(div().text_size(px(14.67)).text_color(rgb(0x666666)).child("No notes available")))
                        .into_any_element()
                } else {
                    v_flex()
                        .child(name_row(d.name.clone()))
                        .child(v_flex().px(px(15.)).pt(px(15.)).gap(px(6.)).text_size(px(14.)).text_color(pal.text).children(d.notes.lines().map(|l| div().child(l.to_string()))))
                        .into_any_element()
                }
            }
            (Some(_), Some(d)) if panel == DocPanel::Properties => match &self.props {
                Some(pe) => {
                    let label = |t: &'static str, required: bool| h_flex().h(px(26.)).items_end().pb(px(4.)).text_size(px(14.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(t).when(required, |d| d.child(div().ml(px(4.)).text_color(rgb(0xaa252d)).child("*")));
                    let boxed = |el: AnyElement, h: f32| div().h(px(h)).border_1().border_color(rgb(0xaaaaaa)).rounded(px(3.)).px(px(10.)).flex().items_center().child(el);
                    // Live documents/113: 32 px fields, 21 px between a field and the next label.
                    let gap = || div().h(px(9.));
                    v_flex()
                        .child(name_row(d.name.clone()))
                        .child(
                            v_flex()
                                .px(px(15.))
                                .pt(px(14.))
                                .w(px(325.))
                                .child(label("Name", true))
                                .child(boxed(Input::new(&pe.name).appearance(false).text_size(px(14.)).into_any_element(), 32.))
                                .child(gap())
                                .child(label("Description", false))
                                .child(boxed(Input::new(&pe.description).appearance(false).text_size(px(14.)).into_any_element(), 52.))
                                .child(gap())
                                .child(label("Category", false))
                                // The category picker reads as disabled (one local category).
                                .child(boxed(h_flex().w_full().text_size(px(14.)).child(div().flex_1().child("Onshape Document")).child(os::glyph(os::GLYPH_CARET, 8., rgb(0x666666))).into_any_element(), 32.).bg(rgb(0xeeeeee)))
                                .child(gap())
                                .child(label("Library label", false))
                                .child(boxed(Input::new(&pe.label).appearance(false).text_size(px(14.)).into_any_element(), 32.)),
                        )
                        .into_any_element()
                }
                None => div().into_any_element(),
            },
            (Some(e), Some(d)) if panel == DocPanel::Details => v_flex()
                .px(px(14.))
                .child(
                    h_flex()
                        .h(px(32.))
                        .mt(px(13.))
                        .gap(px(4.))
                        .child(os::glyph(os::GLYPH_DOCUMENT, 20., pal.text))
                        .child(div().truncate().text_size(px(17.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(d.name.clone())),
                )
                .child(
                    div()
                        .h(px(170.))
                        .my(px(5.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .children(e.thumbnail.clone().map(|t| gpui_kit::img(t).w(px(285.)).h(px(170.)).object_fit(ObjectFit::Contain))),
                )
                .child(section("Owner", vec!["me".into()]))
                .child(section("Document description", if d.description.is_empty() { vec![] } else { vec![d.description.clone()] }))
                .child(
                    v_flex()
                        .py(px(6.))
                        .border_t_1()
                        .border_color(rgb(0xcccccc))
                        // Live documents/24: the ⌄ / › collapses the section.
                        .child(
                            h_flex()
                                .id("details-labels-toggle")
                                .gap(px(6.))
                                .cursor_pointer()
                                .line_height(px(21.))
                                .text_size(px(14.67))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(pal.text)
                                .child(os::glyph(if self.details_labels_open { os::GLYPH_EXPANDED } else { os::GLYPH_COLLAPSED }, 10., pal.text))
                                .child("Document labels")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.details_labels_open = !this.details_labels_open;
                                    cx.notify();
                                })),
                        )
                        .when(self.details_labels_open, |d| d.child(div().mt(px(14.)).rounded(px(2.)).border_1().border_color(rgb(0xcccccc)).child(Self::label_checklist(&cx.entity(), &e.path, &self.label_search, &self.labels, cx)))),
                )
                .child(section("Created by", vec![who(&d.created_by), lib::format_onshape_time(d.created)]))
                .child(section("Last modified by", vec![who(&d.modified_by), lib::format_onshape_time(d.modified)]))
                .into_any_element(),
            (Some(_), Some(d)) => v_flex()
                .child(
                    h_flex()
                        .h(px(38.))
                        .px(px(10.))
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .border_b_1()
                        .border_color(rgb(0xd0dbe5))
                        .child(div().w(px(150.)).child("Name"))
                        .child(div().child("Modified")),
                )
                .children(d.history.iter().map(|(name, ws, author, t)| {
                    let name_el = if *ws {
                        div().px(px(3.)).rounded(px(3.)).border_1().border_color(pal.accent).bg(rgb(0xd0dbe5)).font_weight(FontWeight::MEDIUM).child(name.clone())
                    } else {
                        div().font_weight(FontWeight::MEDIUM).child(name.clone())
                    };
                    h_flex()
                        .h(px(50.))
                        .px(px(10.))
                        .items_start()
                        .pt(px(5.))
                        .text_size(px(14.))
                        .text_color(pal.text_muted)
                        .child(div().w(px(150.)).text_color(pal.text).child(name_el))
                        .child(v_flex().child(who(author)).child(lib::format_onshape_time(*t)))
                }))
                .into_any_element(),
            // A folder (live documents/47): name, Owner, Created by, Last modified by.
            _ if panel == DocPanel::Details && folder.is_some() => {
                let (name, created, modified) = folder.clone().unwrap_or_default();
                v_flex()
                    .px(px(10.))
                    .child(
                        h_flex()
                            .h(px(32.))
                            .gap(px(4.))
                            .child(os::glyph(os::GLYPH_FOLDER, 20., pal.text))
                            .child(div().truncate().text_size(px(17.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(name)),
                    )
                    .child(div().h(px(8.)))
                    .child(section("Owner", vec!["me".into()]))
                    .child(section("Created by", vec!["me".into(), lib::format_onshape_time(created)]))
                    .child(section("Last modified by", vec!["me".into(), lib::format_onshape_time(modified)]))
                    .into_any_element()
            }
            // Live documents/26: a 200x95 `#eee` illustration 35 px under the header, the
            // text one line (14.67 px `#666`) 24 px below it.
            _ => v_flex()
                .items_center()
                .pt(px(35.))
                .gap(px(24.))
                .child(svg().path(crate::assets::icon_path("documents-empty")).w(px(200.)).h(px(95.)).text_color(rgb(0xeeeeee)))
                .child(div().text_center().text_size(px(14.67)).text_color(rgb(0x666666)).child("Select a document or folder to view its details"))
                .into_any_element(),
        };
        Some(
            v_flex()
                .w(px(340.))
                .h_full()
                .flex_none()
                .border_l_1()
                .border_color(pal.border)
                .child(
                    h_flex()
                        .h(px(32.))
                        .flex_none()
                        .px(px(10.))
                        .border_b_1()
                        .border_color(rgb(0xd0dbe5))
                        .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(title))
                        .child(
                            div()
                                .id("doc-panel-close")
                                .cursor_pointer()
                                .child(os::glyph(os::GLYPH_CLOSE, 16., pal.text_faint))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.panel = None;
                                    this.save_prefs();
                                    cx.notify();
                                })),
                        ),
                )
                .child(div().id("doc-panel-body").flex_1().min_h_0().overflow_y_scroll().child(body))
                .when(panel == DocPanel::Properties && entry.is_some(), |d| d.children(self.render_props_footer(cx)))
                .into_any_element(),
        )
    }

    /// The Properties panel's fields follow the selected document.
    fn ensure_props(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel != Some(DocPanel::Properties) {
            return;
        }
        let Some(e) = self.selected_entry() else {
            self.props = None;
            return;
        };
        if self.props.as_ref().is_some_and(|p| p.path == e.path) {
            return;
        }
        let Some(d) = self.details_of(&e.path) else { return };
        let field = |v: &str, window: &mut Window, cx: &mut Context<Self>| {
            let v = v.to_string();
            let st = cx.new(|cx| InputState::new(window, cx).default_value(v));
            cx.observe(&st, |_, _, cx| cx.notify()).detach();
            st
        };
        self.props = Some(PropsEdit {
            path: e.path.clone(),
            name: field(&d.name, window, cx),
            description: field(&d.description, window, cx),
            label: field(&d.library_label, window, cx),
            saved: (d.name.clone(), d.description.clone(), d.library_label.clone()),
        });
    }

    /// Live documents/113: ↶ ↷ (pale) and Apply, disabled until a field changes.
    fn render_props_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pe = self.props.as_ref()?;
        let now = (pe.name.read(cx).value().to_string(), pe.description.read(cx).value().to_string(), pe.label.read(cx).value().to_string());
        let changed = now != pe.saved && !now.0.trim().is_empty();
        Some(
            h_flex()
                .h(px(64.))
                .flex_none()
                .px(px(20.))
                .gap(px(3.))
                .justify_end()
                .child(div().opacity(0.25).child(crate::ui::icon_button("props-undo", "undo", 30., false, cx)))
                .child(div().opacity(0.25).child(crate::ui::icon_button("props-redo", "redo", 30., false, cx)))
                .child(os::primary_button("props-apply", "Apply").when(!changed, |b| b.opacity(0.5)).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if changed {
                        this.apply_props(window, cx);
                    }
                })))
                .into_any_element(),
        )
    }

    fn apply_props(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pe) = &self.props else { return };
        let path = pe.path.clone();
        let (name, desc, label) = (pe.name.read(cx).value().trim().to_string(), pe.description.read(cx).value().to_string(), pe.label.read(cx).value().trim().to_string());
        if crate::files::is_document_open(&path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the document's window before changing its properties"));
            return;
        }
        if let Err(e) = lib::set_doc_properties(&path, &desc, &label) {
            Self::notify(window, cx, AlertKind::Error, format!("Could not save the properties: {e:#}"));
            return;
        }
        let mut dest = path.clone();
        if name != pe.saved.0 {
            match lib::rename_document(&path, &name) {
                Ok(d) => {
                    crate::files::replace_recent(&path, &d);
                    self.labels.moved(&path, &d);
                    self.labels.save();
                    dest = d.canonicalize().unwrap_or(d);
                }
                Err(e) => {
                    Self::notify(window, cx, AlertKind::Error, format!("Rename failed: {e:#}"));
                    return;
                }
            }
        }
        self.props = None;
        self.details = None;
        self.selected = Some(dest);
        self.force_rescan(cx);
    }

    /// "Move to…": Onshape's folder picker; moves the (closed) document.
    fn move_to(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if crate::files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{}: Close the document's window before moving it", d.name));
            return;
        }
        let location = Rc::new(Location::new(d.folder().to_path_buf()));
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (loc, view, d) = (location.clone(), view.clone(), d.clone());
            os::modal(dialog, "Move to", 460.)
                .child(os::modal_body().child(os::field_label("Location")).child(location.render()))
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Move"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let dir = loc.chosen.borrow().clone();
                    match lib::move_document(&d.path, &dir) {
                        Ok(dest) => {
                            crate::files::replace_recent(&d.path, &dest);
                            view.update(cx, |this, cx| {
                                this.labels.moved(&d.path, &dest);
                                this.labels.save();
                                this.selected = Some(dest.canonicalize().unwrap_or(dest));
                                this.force_rescan(cx);
                            });
                            true
                        }
                        Err(e) => {
                            Self::notify(window, cx, AlertKind::Error, format!("Move failed: {e:#}"));
                            false
                        }
                    }
                })
        });
    }


    /// Onshape's document row menu (live, documents/25: a Bootstrap
    /// `list-no-icons` dropdown): Open, Rename, Copy workspace… | Details,
    /// Versions and history | Move to… | Remove from recently opened, Send
    /// to trash. Cloud items (Share, Transfer ownership, new browser tab)
    /// are not part of noshape. Trashed documents: Details, Restore, Delete.
    fn doc_menu(view: Entity<StartView>, d: DocEntry, menu: OsMenu) -> OsMenu {
        let item = |label: &'static str, view: &Entity<StartView>, d: &DocEntry, f: fn(&mut StartView, DocEntry, &mut Window, &mut Context<StartView>)| {
            let (view, d) = (view.clone(), d.clone());
            OsMenuItem::new(label).on_click(move |_, window, cx| {
                let d = d.clone();
                view.update(cx, |this, cx| f(this, d, window, cx));
            })
        };
        if d.trashed {
            // Trash (live documents/69): Details, Restore, Delete.
            let (v1, v2, p1, p2) = (view.clone(), view.clone(), d.path.clone(), d.path.clone());
            let id_of = |v: &StartView, p: &Path| v.trash.item_of(p).map(|i| i.id.clone());
            return menu
                .list_no_icons()
                .item(item("Details", &view, &d, |t, d, _, cx| t.show_panel(d, DocPanel::Details, cx)))
                .item(OsMenuItem::new("Restore").on_click(move |_, window, cx| {
                    v1.update(cx, |t, cx| {
                        if let Some(id) = id_of(t, &p1) {
                            t.restore_trashed(id, window, cx);
                        }
                    })
                }))
                .item(OsMenuItem::new("Delete").on_click(move |_, window, cx| {
                    v2.update(cx, |t, cx| {
                        if let Some(id) = id_of(t, &p2) {
                            t.confirm_delete_forever(id, window, cx);
                        }
                    })
                }));
        }
        let menu = menu
            .list_no_icons()
            .item(item("Open", &view, &d, |t, d, w, cx| t.open_path(d.path, w, cx)))
            .item(item("Rename", &view, &d, |t, d, w, cx| t.rename(d, w, cx)))
            .item(item("Copy workspace…", &view, &d, |t, d, w, cx| t.duplicate(d, w, cx)))
            .separator()
            .item(item("Details", &view, &d, |t, d, _, cx| t.show_panel(d, DocPanel::Details, cx)))
            .item(item("Versions and history", &view, &d, |t, d, _, cx| t.show_panel(d, DocPanel::Versions, cx)))
            .item(item("Labels…", &view, &d, |t, d, w, cx| t.labels_dialog(d, w, cx)))
            .separator()
            .item(item("Move to…", &view, &d, |t, d, w, cx| t.move_to(d, w, cx)))
            .separator();
        let menu = if d.recent_rank.is_some() { menu.item(item("Remove from recently opened", &view, &d, |t, d, _, cx| t.forget(d, cx))) } else { menu };
        menu.item(item("Send to trash", &view, &d, |t, d, w, cx| t.trash(d, w, cx)))
    }

    fn select(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.selected = Some(path.to_path_buf());
        cx.notify();
    }

    fn on_doc_click(&mut self, d: &DocEntry, ev: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(d.path.clone());
        window.focus(&self.focus, cx);
        if ev.click_count() >= 2 && !d.trashed {
            self.open_path(d.path.clone(), window, cx);
        }
        cx.notify();
    }

    /// Onshape documents page top bar: 40 px `#eaeaea`, logo at the left,
    /// the search box (390 px, with its square search button) centered.
    fn render_title_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        TitleBar::new()
            .h(px(crate::ui::NAVBAR_H))
            .bg(pal.panel_inactive)
            .border_b_1()
            .border_color(pal.border)
            .child(
                h_flex()
                    .w_full()
                    .h_full()
                    .child(
                        h_flex()
                            .w(px(220.))
                            .flex_none()
                            .gap(px(5.))
                            .pl(px(8.))
                            .child(crate::assets::icon("noshape-logo", 22., cx))
                            .child(div().text_size(px(21.)).font_weight(FontWeight::LIGHT).text_color(pal.text_muted).child("noshape")),
                    )
                    .child(
                        div().flex_1().flex().justify_center().child(
                            h_flex()
                                .w(px(390.))
                                .h(px(28.))
                                .child({
                                    let probe = self.adv_anchor.clone();
                                    // Live documents/23: italic placeholder, the advanced search ▾ at the right.
                                    h_flex()
                                        .relative()
                                        .flex_1()
                                        .h_full()
                                        .bg(pal.panel)
                                        .border_1()
                                        .border_color(pal.border_strong)
                                        .rounded_l(px(2.))
                                        // The whole box left of the ▾ takes clicks (input_frame).
                                        .child(gpui_kit::base::input::input_frame(div().flex_1().h_full().flex().items_center().child(Input::new(&self.search).appearance(false).small().h(px(26.)).italic().cleanable(true))))
                                        .child(
                                            div()
                                                .id("adv-caret")
                                                .w(px(20.))
                                                .h_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .child(os::glyph(os::GLYPH_CARET, 9., pal.text))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                    this.adv_open = !this.adv_open;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                                })
                                .child(
                                    div()
                                        .id("search-go")
                                        .w(px(30.))
                                        .h_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_l_0()
                                        .border_color(pal.border_strong)
                                        .rounded_r(px(2.))
                                        .cursor_pointer()
                                        .hover(|d| d.bg(pal.border))
                                        .child(os::glyph(os::GLYPH_SEARCH, 17., pal.text))
                                        .on_click({
                                            let search = self.search.clone();
                                            move |_, window, cx| search.update(cx, |s, cx| s.focus(window, cx))
                                        }),
                                ),
                        ),
                    )
                    // Live documents/23: the search box is centred between the 220 px left
                    // column and a 388 px right block (x 681–1071 at 1919 px).
                    .child(div().w(px(388.)).flex_none().flex().justify_end().pr(px(10.)).child(user_menu(cx))),
            )
            .into_any_element()
    }

    /// A left-nav row: 32 px, 18 px glyph, 14 px label; the current one is
    /// blue and semibold with a 4 px blue bar at the left edge.
    fn nav_item(&self, id: &'static str, glyph: &'static [u8], label: &'static str, source: Source, cx: &mut Context<Self>) -> Stateful<Div> {
        let pal = p();
        let active = match (&self.source, &source) {
            (Source::Folder(_), Source::All) => true,
            (a, b) => a == b,
        };
        let color = if active { pal.accent } else { pal.text };
        h_flex()
            .id(id)
            .relative()
            .h(px(32.))
            .pl(px(19.))
            .gap(px(6.))
            .cursor_pointer()
            .text_size(px(14.67))
            .text_color(color)
            .when(active, |d| d.font_weight(FontWeight::SEMIBOLD).child(div().absolute().left_0().top_0().bottom_0().w(px(4.)).bg(pal.accent)))
            .when(!active, |d| d.hover(|d| d.bg(rgb(0xf5f5f5))))
            .child(os::glyph(glyph, 20., color))
            .child(label)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_source(source.clone(), cx)))
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let view = cx.entity();
        let labels_nav = self.render_labels_nav(cx);
        let create = Button::new("create")
            .primary()
            .w(px(150.))
            .h(px(34.))
            .rounded(px(3.))
            .child(h_flex().gap(px(6.)).text_size(px(14.)).child("Create").child(os::glyph(os::GLYPH_CARET, 8., gpui_kit::rgb(0xffffff))))
            .os_dropdown_menu("create-dd", false, move |menu, _, _| {
                // Onshape (live documents/41): Document…, Folder… | Import
                // files…, Import from ▸ (cloud drives, not offered locally),
                // Label…, with 16 px icons.
                let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
                let pal = p();
                let item = |g: &'static [u8], label: &'static str| OsMenuItem::new(label).icon(os::glyph(g, 16., pal.text));
                menu.item(item(os::GLYPH_DOCUMENT, "Document…").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.new_document(window, cx))))
                    .item(item(os::GLYPH_FOLDER, "Folder…").on_click(move |_, window, cx| v2.update(cx, |t, cx| t.new_folder(window, cx))))
                    .separator()
                    .item(item(os::GLYPH_OPEN, "Import files…").on_click(move |_, window, cx| v3.update(cx, |t, cx| t.import_files(window, cx))))
                    .item(item(os::GLYPH_LABEL, "Label…").on_click(move |_, window, cx| v4.update(cx, |t, cx| t.create_label(None, window, cx))))
            });
        v_flex()
            .w(px(220.))
            .h_full()
            .flex_none()
            .bg(pal.panel)
            .border_r_1()
            .border_color(pal.border)
            .child(div().pt(px(8.)).flex().justify_center().child(create))
            .child(
                v_flex()
                    .pt(px(15.))
                    .child(self.nav_item("nav-owned", os::GLYPH_OWNED, "Owned by me", Source::All, cx))
                    .child(self.nav_item("nav-recent", os::GLYPH_RECENT, "Recently opened", Source::Recent, cx))
                    .child(self.nav_item("nav-created", os::GLYPH_CREATED, "Created by me", Source::Created, cx))
                    .child(labels_nav)
                    .child(self.nav_item("nav-trash", os::GLYPH_TRASH, "Trash", Source::Trash, cx)),
            )
            .into_any_element()
    }

    fn source_title(&self) -> String {
        match &self.source {
            Source::Recent => "Recently opened".into(),
            Source::All => "Owned by me".into(),
            Source::Folder(f) => folder_label(f),
            Source::Label(l) => l.clone(),
            Source::Created => "Created by me".into(),
            Source::Trash => "Trash".into(),
        }
    }

    /// The place's title: "Owned by me › folder › subfolder" breadcrumb (16
    /// px / 500), "Recently opened", "Trash"…; the Trash adds Onshape's
    /// notice and "Empty trash…" (live documents/69).
    fn render_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (glyph, root) = match &self.source {
            Source::Recent => (os::GLYPH_RECENT, "Recently opened".to_string()),
            Source::Created => (os::GLYPH_CREATED, "Created by me".to_string()),
            Source::Trash => (os::GLYPH_TRASH, "Trash".to_string()),
            Source::Label(l) => (os::GLYPH_LABEL, l.clone()),
            Source::All | Source::Folder(_) => (os::GLYPH_OWNED, "Owned by me".to_string()),
        };
        // Folders from the documents folder (or an added folder) down.
        let crumbs: Vec<PathBuf> = match &self.source {
            Source::Folder(f) => {
                let f = canon(f);
                let docs = canon(&crate::files::documents_dir());
                let top = outside_folders().into_iter().find(|o| f.starts_with(o));
                let mut v: Vec<PathBuf> = f.ancestors().take_while(|a| *a != docs && top.as_deref().is_none_or(|t| a.starts_with(t))).map(Path::to_path_buf).collect();
                v.reverse();
                v
            }
            _ => Vec::new(),
        };
        let in_folder = !crumbs.is_empty();
        let mut row = h_flex()
            .h(px(52.))
            .flex_none()
            .pl(px(20.))
            .gap(px(6.))
            .text_size(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(pal.text)
            .child(os::glyph(glyph, 18., pal.text))
            .child(
                // Live dom/documents-folder-row: parent crumbs 16 px / 400
                // `#666`, the leaf 16 px / 500 `#333`, blue › between them.
                div()
                    .id("crumb-root")
                    .child(root)
                    .when(in_folder, |d| {
                        d.font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0x666666))
                            .cursor_pointer()
                            .hover(|d| d.text_color(pal.accent))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_source(Source::All, cx)))
                    }),
            );
        let last = crumbs.len().saturating_sub(1);
        for (i, c) in crumbs.into_iter().enumerate() {
            let target = c.clone();
            row = row.child(os::glyph(os::GLYPH_CHEVRON_RIGHT, 14., pal.accent)).child(os::glyph(os::GLYPH_FOLDER, 18., pal.text)).child(
                div().id(("crumb", i)).child(folder_label(&c)).when(i < last, |d| {
                    d.font_weight(FontWeight::NORMAL)
                        .text_color(rgb(0x666666))
                        .cursor_pointer()
                        .hover(|d| d.text_color(pal.accent))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_source(Source::Folder(target.clone()), cx)))
                }),
            );
        }
        v_flex()
            .flex_none()
            .child(row)
            .when(self.source == Source::Trash && !self.trash.items.is_empty(), |d| {
                d.child(
                    h_flex()
                        .h(px(48.))
                        .justify_center()
                        .gap(px(8.))
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child("Documents in the trash will be automatically deleted after 30 days.")
                        .child(os::secondary_button("empty-trash", "Empty trash…", cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.confirm_empty_trash(window, cx)))),
                )
            })
            .into_any_element()
    }

    /// The folders shown as tiles in the current place: the top-level
    /// folders in "Owned by me", a folder's subfolders, trashed folders.
    fn folder_tiles(&self) -> Vec<(PathBuf, Option<String>)> {
        let mut tiles: Vec<(PathBuf, Option<String>)> = match &self.source {
            Source::All => {
                let root = canon(&crate::files::documents_dir());
                let mut v: Vec<(PathBuf, Option<String>)> = self.dirs.iter().filter(|d| d.parent() == Some(root.as_path())).map(|d| (d.clone(), None)).collect();
                let extra: Vec<PathBuf> = outside_folders().into_iter().filter(|f| !v.iter().any(|(d, _)| d == f)).collect();
                v.extend(extra.into_iter().map(|f| (f, None)));
                v.sort_by_key(|(d, _)| folder_label(d).to_lowercase());
                v
            }
            Source::Folder(f) => {
                let f = canon(f);
                let mut v: Vec<(PathBuf, Option<String>)> = self.dirs.iter().filter(|d| d.parent() == Some(f.as_path())).map(|d| (d.clone(), None)).collect();
                v.sort_by_key(|(d, _)| folder_label(d).to_lowercase());
                v
            }
            Source::Trash => self.trash.items.iter().filter(|i| i.folder).map(|i| (i.stored(), Some(i.id.clone()))).collect(),
            _ => Vec::new(),
        };
        let name = |(d, id): &(PathBuf, Option<String>)| id.as_ref().and_then(|id| self.trash.get(id)).map(|t| t.name.clone()).unwrap_or_else(|| folder_label(d)).to_lowercase();
        match self.folder_sort {
            FolderSort::NameAsc => tiles.sort_by_key(name),
            FolderSort::NameDesc => {
                tiles.sort_by_key(name);
                tiles.reverse();
            }
            FolderSort::Modified => tiles.sort_by_key(|(d, _)| std::cmp::Reverse(std::fs::metadata(d).and_then(|m| m.modified()).ok())),
        }
        tiles
    }

    /// The "Folders" section: a collapsible caption and gray folder tiles
    /// (live documents/45: 267x48, `#f5f5f5`, 1 px `#eee`, radius 4; click
    /// selects, double-click opens), then the divider with its collapse
    /// handle.
    fn render_folders(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        if !matches!(self.source, Source::All | Source::Folder(_) | Source::Trash) {
            return None;
        }
        let tiles_list = self.folder_tiles();
        let in_folder = matches!(self.source, Source::Folder(_));
        let open = if in_folder { self.sub_open } else { self.folders_open };
        let list_view = self.view == ViewMode::List;
        let folder_rows = if list_view && !tiles_list.is_empty() { Some(self.render_folder_rows(&tiles_list, cx)) } else { None };
        let has_tiles = !tiles_list.is_empty();
        let mut tiles = div().flex().flex_wrap().gap(px(12.)).pl(px(18.)).pr(px(10.));
        for (i, (f, trash_id)) in tiles_list.into_iter().enumerate() {
            let view = cx.entity();
            let selected = self.selected.as_ref() == Some(&f);
            let name = trash_id.as_ref().and_then(|id| self.trash.get(id)).map(|t| t.name.clone()).unwrap_or_else(|| folder_label(&f));
            let (f1, f2) = (f.clone(), f.clone());
            tiles = tiles.child(
                h_flex()
                    .id(SharedString::from(format!("folder-{i}")))
                    .w(px(267.))
                    .h(px(48.))
                    .px(px(8.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .border_1()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(pal.accent_soft).border_color(pal.accent_soft))
                    .when(!selected, |d| d.bg(rgb(0xf5f5f5)).border_color(rgb(0xeeeeee)).hover(|d| d.bg(pal.row_hover)))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(os::glyph(os::GLYPH_FOLDER, 20., pal.text))
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.selected = Some(f1.clone());
                        if ev.click_count() >= 2 && !this.trash.items.iter().any(|t| t.stored() == f1) {
                            this.set_source(Source::Folder(f1.clone()), cx);
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.selected = Some(f2.clone());
                        cx.notify();
                    }))
                    .os_context_menu(move |menu, _, _| Self::folder_menu(view.clone(), f.clone(), name.clone(), trash_id.clone(), menu)),
            );
        }
        // Inside a folder the caption is the folder's name (live dom/documents-folder-row).
        let caption = match &self.source {
            Source::Folder(f) => folder_label(f),
            _ => "Folders".to_string(),
        };
        let expanded = open;
        // Live documents/89–92: "No folders owned by you.", "No folders in <folder>.", "No folders in Trash."
        let empty_text = match &self.source {
            Source::Folder(f) => format!("No folders in {}.", folder_label(f)),
            Source::Trash => "No folders in Trash.".to_string(),
            _ => "No folders owned by you.".to_string(),
        };
        // Onshape's split pane: expanded, the folders take ~39 % of the list
        // area (357 of 908 px live) or the height the divider was dragged to.
        let list_h = self.list_h.get();
        let pane_h = expanded.then(|| self.split_h.unwrap_or((list_h * 0.393).max(150.)).min((list_h - 120.).max(150.)));
        let recent = self.render_last_opened(cx);
        let toggle = cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_folders(cx));
        Some(
            v_flex()
                .flex_none()
                .when_some(pane_h, |d, h| d.h(px(h)))
                .child(
                    v_flex()
                        .id("folders-pane")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(recent)
                        .child(
                            // Caption, and the view buttons at the right (live documents/69).
                            h_flex()
                                .h(px(36.))
                                .flex_none()
                                .pr(px(20.))
                                .child(
                                    h_flex()
                                        .id("folders-caption")
                                        .h_full()
                                        .pl(px(23.))
                                        .gap(px(7.))
                                        .cursor_pointer()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(pal.text)
                                        .child(os::glyph(if expanded { os::GLYPH_EXPANDED } else { os::GLYPH_COLLAPSED }, 10., pal.text))
                                        .child(os::glyph(os::GLYPH_FOLDER, 20., pal.text))
                                        .child(div().ml(px(1.)).child(caption))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_folders(cx))),
                                )
                                .child(div().flex_1())
                                .when(expanded, |d| d.child(self.render_view_toggles(cx))),
                        )
                        .when(expanded && !has_tiles, |d| d.child(div().pl(px(16.)).pt(px(4.)).text_size(px(14.)).text_color(pal.text).child(empty_text)))
                        .when(expanded && has_tiles, |d| match folder_rows {
                            Some(rows) => d.child(rows),
                            None => d.child(div().pb(px(10.)).child(tiles)),
                        }),
                )
                .child(
                    // The splitter between folders and documents (drag it; live
                    // documents/45, 69), with its round handle: ^ collapses an
                    // open pane, the down chevron opens it.
                    div()
                        .id("folders-splitter")
                        .relative()
                        .flex_none()
                        .h(px(20.))
                        .mx(px(10.))
                        .when(expanded, |d| {
                            d.cursor(CursorStyle::ResizeUpDown).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                                    this.split_drag = Some((f32::from(e.position.y), pane_h.unwrap_or(0.)));
                                    cx.notify();
                                }),
                            )
                        })
                        .child(div().absolute().left_0().right_0().top(px(4.)).h(px(1.)).bg(pal.tooltip))
                        .child(
                            div().absolute().left_0().right_0().top_0().flex().justify_center().child(
                                div()
                                    .id("folders-toggle")
                                    .w(px(26.))
                                    .h(px(16.))
                                    .rounded(px(8.))
                                    .bg(pal.panel)
                                    .shadow_sm()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 12., pal.text).when(expanded, |i| i.rotate(gpui_kit::percentage(0.5))))
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .on_click(toggle),
                            ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn toggle_folders(&mut self, cx: &mut Context<Self>) {
        if matches!(self.source, Source::Folder(_)) {
            self.sub_open = !self.sub_open;
        } else {
            self.folders_open = !self.folders_open;
        }
        cx.notify();
    }

    /// The folders as a small table (the Folders row's list button; live
    /// documents/97): Name | Modified | Modified by, a folder icon per row;
    /// the headers sort.
    fn render_folder_rows(&self, tiles: &[(PathBuf, Option<String>)], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let sort = self.folder_sort;
        let head = |id: &'static str, label: &'static str, w: f32, active: bool, desc: bool, next: FolderSort, cx: &mut Context<Self>| {
            h_flex()
                .id(id)
                .w(px(w))
                .h_full()
                .flex_none()
                .pl(px(16.))
                .gap(px(6.))
                .cursor_pointer()
                .hover(|d| d.bg(rgb(0xf5f5f5)))
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.text)
                .child(label)
                .when(active, |d| d.child(os::glyph(os::GLYPH_CARET, 9., pal.text_muted).when(!desc, |i| i.rotate(gpui_kit::percentage(0.5)))))
                .child(div().flex_1())
                .child(div().w(px(2.)).h(px(22.)).flex_none().bg(pal.border))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.folder_sort = next;
                    this.save_prefs();
                    cx.notify();
                }))
        };
        let name_next = if sort == FolderSort::NameAsc { FolderSort::NameDesc } else { FolderSort::NameAsc };
        let header = h_flex()
            .h(px(32.))
            .border_b_1()
            .border_color(pal.border_strong)
            .child(head("fh-name", "Name", 350., matches!(sort, FolderSort::NameAsc | FolderSort::NameDesc), sort == FolderSort::NameDesc, name_next, cx))
            .child(head("fh-mod", "Modified", 200., sort == FolderSort::Modified, true, FolderSort::Modified, cx))
            .child(head("fh-modby", "Modified by", 200., false, false, sort, cx));
        let mut rows = v_flex().child(header);
        for (i, (f, trash_id)) in tiles.iter().enumerate() {
            let selected = self.selected.as_ref() == Some(f);
            let name = trash_id.as_ref().and_then(|id| self.trash.get(id)).map(|t| t.name.clone()).unwrap_or_else(|| folder_label(f));
            let modified = std::fs::metadata(f).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
            let (f1, f2, f3, v) = (f.clone(), f.clone(), f.clone(), cx.entity());
            let tid = trash_id.clone();
            let menu_name = name.clone();
            rows = rows.child(
                h_flex()
                    .id(("folder-row", i))
                    .h(px(42.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .border_b_1()
                    .border_color(rgb(0xececec))
                    .when(selected, |r| r.bg(pal.accent_soft))
                    .when(!selected, |r| r.hover(|r| r.bg(pal.row_hover)))
                    .child(h_flex().w(px(350.)).flex_none().pl(px(16.)).gap(px(10.)).child(os::glyph(os::GLYPH_FOLDER, 20., pal.text)).child(div().min_w_0().truncate().child(name)))
                    .child(div().w(px(200.)).flex_none().pl(px(16.)).child(lib::format_onshape_time(modified)))
                    .child(div().w(px(200.)).flex_none().pl(px(16.)).child("me"))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.selected = Some(f1.clone());
                        if ev.click_count() >= 2 && this.trash.item_of(&f1).is_none() {
                            this.set_source(Source::Folder(f1.clone()), cx);
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.selected = Some(f2.clone());
                        cx.notify();
                    }))
                    .os_context_menu(move |menu, _, _| Self::folder_menu(v.clone(), f3.clone(), menu_name.clone(), tid.clone(), menu)),
            );
        }
        div().px(px(10.)).pb(px(10.)).child(rows).into_any_element()
    }

    /// Owned by me's "Last opened by me" row (live documents/45): the recently
    /// opened documents as 267x48 tiles with their thumbnails.
    fn render_last_opened(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.source != Source::All {
            return None;
        }
        let pal = p();
        let mut recent: Vec<DocEntry> = self.docs.iter().filter(|d| d.recent_rank.is_some() && !d.trashed).cloned().collect();
        if recent.is_empty() {
            return None;
        }
        recent.sort_by_key(|d| d.recent_rank);
        let open = self.recent_open;
        let mut tiles = div().h(px(80.)).overflow_hidden().flex().flex_wrap().gap(px(12.)).pl(px(18.)).pr(px(10.));
        for (i, d) in recent.into_iter().take(8).enumerate() {
            let selected = self.selected.as_ref() == Some(&d.path);
            let (d1, d2, v) = (d.clone(), d.clone(), cx.entity());
            tiles = tiles.child(
                h_flex()
                    .id(("recent-tile", i))
                    .w(px(267.))
                    .h(px(68.))
                    .mt(px(6.))
                    .px(px(6.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .border_1()
                    .cursor_pointer()
                    .when(selected, |t| t.bg(pal.accent_soft).border_color(pal.accent_soft))
                    .when(!selected, |t| t.bg(rgb(0xffffff)).border_color(rgb(0xeeeeee)).hover(|t| t.bg(pal.row_hover)))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(div().w(px(82.)).h(px(54.)).flex_none().rounded(px(4.)).bg(rgb(0xf5f5f5)).flex().items_center().justify_center().child(self.thumbnail(&d, 76., 48., false)))
                    .child(div().flex_1().min_w_0().truncate().child(d.name.clone()))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| this.on_doc_click(&d1, ev, window, cx)))
                    .os_context_menu(move |menu, _, _| Self::doc_menu(v.clone(), d2.clone(), menu)),
            );
        }
        Some(
            v_flex()
                .flex_none()
                .child(
                    h_flex()
                        .id("recent-caption")
                        .h(px(36.))
                        .pl(px(23.))
                        .gap(px(7.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(pal.text)
                        .child(os::glyph(if open { os::GLYPH_EXPANDED } else { os::GLYPH_COLLAPSED }, 10., pal.text))
                        .child(os::glyph(os::GLYPH_RECENT, 20., pal.text))
                        .child(div().ml(px(1.)).child("Last opened by me"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.recent_open = !this.recent_open;
                            cx.notify();
                        })),
                )
                .when(open, |d| d.child(tiles))
                .into_any_element(),
        )
    }

    /// The buttons at the right of the Folders row (live documents/94–98):
    /// sort ▾ (Name: A → Z, Name: Z → A, Last modified; grid view only),
    /// grid (tiles, default) and list. They act on the folders only.
    fn render_view_toggles(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let sort = self.folder_sort;
        h_flex()
            .flex_none()
            .gap(px(4.))
            .when(self.view == ViewMode::Grid, |d| {
                d.child(
                    div()
                        .id("folder-sort")
                        .w(px(28.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0xeaeaea)))
                        .child(os::glyph(os::GLYPH_SORT, 20., p().text))
                        .os_dropdown_menu("folder-sort-dd", false, move |m, _, _| {
                            let item = |label: &'static str, k: FolderSort| {
                                let v = view.clone();
                                OsMenuItem::new(label).checked(sort == k).on_click(move |_, _, cx| {
                                    v.update(cx, |this, cx| {
                                        this.folder_sort = k;
                                        this.save_prefs();
                                        cx.notify();
                                    })
                                })
                            };
                            m.item(item("Name: A → Z", FolderSort::NameAsc)).item(item("Name: Z → A", FolderSort::NameDesc)).item(item("Last modified", FolderSort::Modified))
                        }),
                )
                .child(div().w(px(1.)).h(px(22.)).mx(px(4.)).bg(rgb(0xd7d7d7)))
            })
            .child(
                view_toggle("view-grid", os::GLYPH_GRID_VIEW, "Grid", self.view == ViewMode::Grid)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_view(ViewMode::Grid, cx))),
            )
            .child(
                view_toggle("view-list", os::GLYPH_LIST_VIEW, "List", self.view == ViewMode::List)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_view(ViewMode::List, cx))),
            )
            .into_any_element()
    }

    fn thumbnail(&self, d: &DocEntry, w: f32, h: f32, big: bool) -> AnyElement {
        let pal = p();
        // Onshape document thumbnails: the model on white; documents
        // without one show the gray document glyph.
        let base = div().w(px(w)).h(px(h)).flex_none().overflow_hidden();
        match &d.thumbnail {
            Some(img) => base.child(gpui_kit::img(img.clone()).size_full()).into_any_element(),
            None => {
                let (ic, color) = if d.broken { (Icon::new(IconName::TriangleAlert), pal.warning) } else { (icon(os::GLYPH_DOCUMENT), pal.text_faint) };
                base.flex()
                    .items_center()
                    .justify_center()
                    .child(ic.size(px(if big { 40. } else { 22. })).text_color(color))
                    .into_any_element()
            }
        }
    }

    fn open_badge() -> AnyElement {
        let pal = p();
        h_flex()
            .gap_1()
            .px(px(5.))
            .rounded(px(3.))
            .bg(pal.info_fill)
            .border_1()
            .border_color(pal.info_outline)
            .text_size(px(11.))
            .text_color(pal.info_text)
            .child("Open")
            .into_any_element()
    }



    /// A table header cell: 14 px semibold, with the 2 px column divider at
    /// its right edge; sortable columns show ▲/▼.
    fn list_header_cell(&self, id: &'static str, label: &'static str, key: Option<SortKey>, cx: &mut Context<Self>) -> Stateful<Div> {
        let pal = p();
        let active = key == Some(self.sort);
        h_flex()
            .id(id)
            .h_full()
            .pl(px(16.))
            .gap(px(6.))
            .text_size(px(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(pal.text)
            .child(label)
            .when(active, |d| d.child(os::glyph(os::GLYPH_CARET, 9., pal.text_muted).when(!self.descending, |i| i.rotate(gpui_kit::percentage(0.5)))))
            .child(div().flex_1())
            .child(div().w(px(2.)).h(px(22.)).flex_none().bg(pal.border))
            .when_some(key, |d, k| d.cursor_pointer().hover(|d| d.bg(rgb(0xf5f5f5))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_sort(k, cx))))
    }

    /// "+ Add" (live documents/81): opens the column panel under the button's
    /// right edge: a tri-state "all" box + "Search...", then one row per
    /// optional column (checkbox, grip to drag the order, name). Ticking
    /// applies at once; Esc / a click outside closes it.
    fn render_add_columns(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let probe = self.cols_anchor.clone();
        let button = h_flex()
            .id("add-columns")
            .relative()
            .h(px(28.))
            .px(px(7.))
            .gap(px(4.))
            .rounded(px(3.5))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(0xeaeaea)))
            .text_size(px(14.))
            .text_color(pal.text)
            .child(os::glyph(os::GLYPH_PLUS, 14., pal.text))
            .child("Add")
            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.cols_open = !this.cols_open;
                cx.notify();
            }));
        if !self.cols_open {
            return button.into_any_element();
        }
        let q = self.col_search.read(cx).value().trim().to_lowercase();
        let on = self.columns.iter().filter(|c| c.1).count();
        let all = on == self.columns.len();
        let box_ = |checked: bool, partial: bool| {
            div()
                .size(px(16.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border_1()
                .border_color(if checked || partial { pal.accent } else { rgb(0xadadad) })
                .bg(if checked || partial { pal.accent } else { rgb(0xffffff) })
                .when(checked, |d| d.child(crate::ui::glyph("check", 11., rgb(0xffffff))))
                .when(partial && !checked, |d| d.child(div().w(px(8.)).h(px(2.)).bg(rgb(0xffffff))))
        };
        let mut panel = v_flex().w(px(194.)).py(px(4.)).bg(pal.panel).border_1().border_color(rgb(0xeeeeee)).shadow_md().child(
            h_flex()
                .h(px(40.))
                .px(px(8.))
                .gap(px(8.))
                .child(div().id("cols-all").cursor_pointer().child(box_(all, on > 0)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    for c in this.columns.iter_mut() {
                        c.1 = !all;
                    }
                    this.save_prefs();
                    cx.notify();
                })))
                .child(div().w(px(142.)).child(Input::new(&self.col_search).prefix(os::glyph(os::GLYPH_SEARCH, 14., pal.text_muted)).h(px(32.)).text_size(px(14.)))),
        );
        for (i, (c, shown)) in self.columns.clone().into_iter().enumerate() {
            if !q.is_empty() && !c.label().to_lowercase().contains(&q) {
                continue;
            }
            panel = panel.child(
                h_flex()
                    .id(("col-row", i))
                    .h(px(32.))
                    .px(px(8.))
                    .gap(px(8.))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(div().id(("col-check", i)).cursor_pointer().child(box_(shown, false)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.columns[i].1 = !this.columns[i].1;
                        this.save_prefs();
                        cx.notify();
                    })))
                    .child(
                        div()
                            .id(("col-grip", i))
                            .opacity(0.7)
                            .cursor_grab()
                            .child(os::glyph(os::GLYPH_GRIP, 16., pal.text))
                            .on_drag(ColDrag(i, c.label().into()), |d, _, _, cx| cx.new(|_| d.clone())),
                    )
                    .child(c.label())
                    .drag_over::<ColDrag>(|d, _, _, _| d.bg(rgb(0xdef1fb)))
                    .on_drop(cx.listener(move |this, d: &ColDrag, _, cx| {
                        let from = d.0;
                        if from != i && from < this.columns.len() {
                            let c = this.columns.remove(from);
                            this.columns.insert(i, c);
                            this.save_prefs();
                            cx.notify();
                        }
                    })),
            );
        }
        let b = self.cols_anchor.get();
        let panel = deferred(
            anchored()
                .anchor(Anchor::TopRight)
                .position(point(b.right(), b.bottom()))
                .snap_to_window_with_margin(px(4.))
                .child(
                    div()
                        .occlude()
                        .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            this.cols_open = false;
                            cx.notify();
                        }))
                        .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                            if e.keystroke.key == "escape" {
                                this.cols_open = false;
                                cx.notify();
                            }
                        }))
                        .child(panel),
                ),
        )
        .with_priority(100);
        div().child(button).child(panel).into_any_element()
    }

    fn render_list(&self, docs: &[DocEntry], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let cols: Vec<Col> = self.columns.iter().filter(|c| c.1).map(|c| c.0).collect();
        let mut header = h_flex()
            .h(px(32.))
            .flex_none()
            .border_b_1()
            .border_color(pal.border_strong)
            .child(self.list_header_cell("h-name", "Name", Some(SortKey::Name), cx).w(px(350.)).flex_none());
        for c in &cols {
            let key = (*c == Col::Modified).then_some(SortKey::Modified);
            header = header.child(self.list_header_cell(c.label(), c.label(), key, cx).w(px(200.)).flex_none());
        }
        // Onshape writes the signed-in user as "me".
        let me = self.me.clone();
        let who = move |name: &str| if name.is_empty() || me.iter().any(|m| m == name) { "me".to_string() } else { name.to_string() };
        let rows: Vec<AnyElement> = docs
            .iter()
            .enumerate()
            .map(|(ix, d)| {
                let selected = self.selected.as_ref() == Some(&d.path);
                let open = crate::files::is_document_open(&d.path, cx);
                let v2 = cx.entity();
                let (d1, d2) = (d.clone(), d.clone());
                let p1 = d.path.clone();
                let cell = |w: f32| div().w(px(w)).flex_none().pl(px(16.)).truncate();
                h_flex()
                    .id(SharedString::from(format!("row-{ix}")))
                    .h(px(42.))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .border_b_1()
                    .border_color(rgb(0xececec))
                    .when(selected, |r| r.bg(pal.accent_soft))
                    .when(!selected, |r| r.hover(|r| r.bg(pal.row_hover)))
                    .child(
                        h_flex()
                            .w(px(350.))
                            .flex_none()
                            .pl(px(16.))
                            .pr(px(15.))
                            .gap(px(14.))
                            .child(self.thumbnail(d, 64.0, 36.0, false))
                            .child(div().min_w_0().truncate().text_color(if d.broken { pal.warning } else { pal.text }).child(d.name.clone()))
                            // The workspace chip (live documents-list: 14 px workspace icon, 11 px name).
                            .when(!d.workspace.is_empty(), |r| {
                                r.child(h_flex().flex_none().ml(px(-7.)).gap(px(1.)).text_size(px(11.)).text_color(pal.text).child(os::glyph(os::GLYPH_WORKSPACE, 14., pal.text)).child(d.workspace.clone()))
                            })
                            .when(open, |r| r.child(Self::open_badge())),
                    )
                    .children(cols.iter().map(|c| {
                        cell(200.)
                            .child(match c {
                                Col::Modified => lib::format_onshape_time(d.modified),
                                Col::ModifiedBy => who(&d.modified_by),
                                Col::OwnedBy => who(&d.owner),
                                Col::Type => "Document".to_string(),
                                Col::Description => d.description.clone(),
                            })
                            .into_any_element()
                    }))
                    // Live documents/112: the selected row shows its quick actions at the
                    // right end (share is cloud-only): Send to trash, Labels.
                    .when(selected && !d.trashed, |r| {
                        let (d3, d4) = (d.clone(), d.clone());
                        let icon = |id: &'static str, glyph: &'static [u8], tip: &'static str| div().id(id).size(px(20.)).flex_none().cursor_pointer().tooltip(crate::ui::tooltip(tip)).child(os::glyph(glyph, 20., pal.text));
                        r.child(div().flex_1()).child(
                            h_flex()
                                .flex_none()
                                .gap(px(8.))
                                .pr(px(13.))
                                .child(icon("row-trash", os::GLYPH_TRASH, "Send to trash").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.trash(d3.clone(), window, cx);
                                })))
                                .child(icon("row-labels", os::GLYPH_LABEL, "Labels").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.labels_dialog(d4.clone(), window, cx);
                                }))),
                        )
                    })
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| this.on_doc_click(&d1, ev, window, cx)))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.select(&p1, cx)))
                    .os_context_menu(move |menu, _, _| Self::doc_menu(v2.clone(), d2.clone(), menu))
                    .into_any_element()
            })
            .collect();
        v_flex()
            .size_full()
            .pl(px(10.))
            .pr(px(12.))
            // The row above the table with Onshape's "+ Add" column button.
            // (live documents-list DOM: "+ Add" 57 px and the header 92 px below the page top)
            .child(h_flex().h(px(42.)).flex_none().items_start().pt(px(4.)).pr(px(14.)).child(div().flex_1()).child(self.render_add_columns(cx)))
            // Wider than the pane (more columns): the table scrolls sideways.
            .child(
                div().id("doc-table").flex_1().min_h_0().w_full().overflow_x_scroll().child(
                    v_flex()
                        .h_full()
                        .min_w_full()
                        .w(px(350. + 200. * cols.len() as f32))
                        .child(header)
                        .child(if docs.is_empty() {
                            self.render_empty(cx)
                        } else {
                            div().id("doc-list").flex_1().min_h_0().overflow_y_scrollbar().child(v_flex().children(rows)).into_any_element()
                        }),
                ),
            )
            .into_any_element()
    }

    /// Empty list: the large gray noshape mark and one line of `#666` text,
    /// as Onshape's "No documents owned by you."
    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let searching = !self.query.trim().is_empty();
        let text: AnyElement = if !self.loaded {
            "Loading documents…".into_any_element()
        } else if searching {
            format!("No documents match \"{}\".", self.query.trim()).into_any_element()
        } else if self.adv_query.is_some() {
            // An advanced search with no results (exact Onshape wording requested).
            "No documents match your search.".into_any_element()
        } else {
            match &self.source {
                // Live documents/91: the Create / Learning Center line.
                Source::Created => {
                    let lead = "Get started by clicking the ";
                    let mid = "Create";
                    let tail = " button, or follow the self-paced courses in the ";
                    let text = format!("{lead}{mid}{tail}");
                    h_flex()
                        .flex_wrap()
                        .justify_center()
                        .child(StyledText::new(text).with_highlights([(lead.len()..lead.len() + mid.len(), HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })]))
                        .child(
                            div()
                                .id("learning-center")
                                .cursor_pointer()
                                .font_weight(FontWeight::BOLD)
                                .underline()
                                .child("Learning Center.")
                                .on_click(|_, _, cx| cx.open_url("https://github.com/yekta/noshape#readme")),
                        )
                        .into_any_element()
                }
                Source::Recent => "No recently opened documents.".into_any_element(),
                Source::Folder(f) => format!("No documents in {}.", folder_label(f)).into_any_element(),
                Source::Label(_) => "No documents with this label.".into_any_element(),
                Source::All => "No documents owned by you.".into_any_element(),
                Source::Trash => "Nothing in trash.".into_any_element(),
            }
        };
        let _ = cx;
        v_flex()
            .flex_1()
            .w_full()
            .items_center()
            .pt(px(60.))
            .gap(px(20.))
            .child(os::glyph(os::GLYPH_EMPTY, 150., rgb(0xd7d7d7)))
            .child(div().max_w(px(520.)).text_center().text_size(px(14.)).text_color(pal.text).child(text))
            .into_any_element()
    }

    /// Bottom strip: 30 px, white, small `#666` text (Onshape's footer).
    fn render_footer(&self) -> AnyElement {
        let pal = p();
        h_flex()
            .h(px(28.))
            .flex_none()
            .px(px(20.))
            .border_t_1()
            .border_color(pal.border)
            .text_size(px(11.))
            .text_color(pal.text_muted)
            .child(div().flex_1().child(format!("Documents are stored on this computer in {}", display_path(&crate::files::documents_dir()))))
            .child(format!("noshape {}", env!("CARGO_PKG_VERSION")))
            .into_any_element()
    }
}

impl Render for StartView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        // Onshape: "Search in Owned by me" (the current place).
        let placeholder = format!("Search in {}", self.source_title());
        if self.placeholder != placeholder {
            self.placeholder = placeholder.clone();
            self.search.update(cx, |s, cx| s.set_placeholder(placeholder, window, cx));
        }
        let docs = self.visible();
        let header = self.render_header(cx);
        // Searching hides the folder area, except an advanced search for folders.
        let folders = if self.query.trim().is_empty() && self.adv_query.as_ref().is_none_or(|q| q.folders_only()) { self.render_folders(cx) } else { None };
        let strip = self.render_strip(cx);
        self.ensure_props(window, cx);
        let panel = self.render_doc_panel(cx);
        let content = self.render_list(&docs, cx);
        v_flex()
            .size_full()
            .bg(pal.app_bg)
            .text_color(pal.text)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NewDocument, window, cx| this.new_document(window, cx)))
            .on_action(cx.listener(|this, _: &OpenDocument, window, cx| this.import_files(window, cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|this, _: &OpenSelected, window, cx| {
                if let Some(d) = this.selected_entry() {
                    this.open_path(d.path, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RenameSelected, window, cx| {
                if let Some(d) = this.selected_entry() {
                    this.rename(d, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &TrashSelected, window, cx| {
                if let Some(d) = this.selected_entry() {
                    this.trash(d, window, cx);
                } else if let Some(f) = this.selected.clone().filter(|f| this.folder_tiles().iter().any(|(t, _)| t == f)) {
                    match this.trash.item_of(&f).map(|i| i.id.clone()) {
                        Some(id) => this.confirm_delete_forever(id, window, cx),
                        None => this.confirm_trash(f.clone(), folder_label(&f), true, window, cx),
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_selection(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_selection(-1, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| this.search.update(cx, |s, cx| s.focus(window, cx))))
            // Dragging the folders divider.
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if let Some((y0, h0)) = this.split_drag {
                    let max = (this.list_h.get() - 120.).max(150.);
                    this.split_h = Some((h0 + f32::from(e.position.y) - y0).clamp(80., max));
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.split_drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.render_sidebar(cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(header)
                            .child({
                                let probe = self.list_h.clone();
                                v_flex()
                                    .relative()
                                    .flex_1()
                                    .min_h_0()
                                    .child(canvas(move |b, _, _| probe.set(f32::from(b.size.height)), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                                    .children(folders)
                                    .child(div().flex_1().min_h_0().child(content))
                            }),
                    )
                    // Live documents/69: the icon strip, then the open panel.
                    .child(strip)
                    .children(panel),
            )
            .child(self.render_footer())
            // Advanced search: under the search box, left-aligned with it (live documents/110).
            .when(self.adv_open, |d| {
                let b = self.adv_anchor.get();
                d.child(
                    deferred(anchored().position(point(b.left(), b.bottom() + px(1.))).child(div().on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        if !crate::os_menu::is_open(cx) {
                            this.adv_open = false;
                            cx.notify();
                        }
                    })).child(self.adv.clone())))
                    .with_priority(1),
                )
            })
            // Onshape right-click menus (`os_context_menu`).
            .children(cx.try_global::<StartToast>().and_then(|t| t.0.clone()).map(|t| {
                // Onshape: top center, 8 px from the top of the page (over the top bar).
                let text = match &t.bold {
                    Some(b) => StyledText::new(format!("{b}{}", t.text)).with_highlights([(0..b.len(), HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })]).into_any_element(),
                    None => t.text.clone().into_any_element(),
                };
                let close = |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    cx.default_global::<StartToast>().0 = None;
                    window.refresh();
                };
                let bubble = match t.restore.clone() {
                    Some(id) => {
                        let view = cx.entity();
                        os::message_bubble_with_link("start-toast", t.kind, text, "Restore", move |_, window, cx| view.update(cx, |this, cx| this.restore_trashed(id.clone(), window, cx)), close, cx)
                    }
                    None => os::message_bubble("start-toast", t.kind, text, close, cx),
                };
                div().absolute().top(px(8.)).left_0().right_0().flex().justify_center().child(bubble)
            }))
    }
}

/// A documents-page message: kind, an optional bold lead (a document
/// name), the text, and the trash item its Restore link brings back.
#[derive(Clone)]
struct Toast {
    kind: os::BubbleKind,
    bold: Option<String>,
    text: String,
    restore: Option<String>,
    /// Fades after ~3 s (Onshape's restore toast) instead of 8 s.
    brief: bool,
}

/// The documents page's message bubble and its generation.
#[derive(Default)]
struct StartToast(Option<Toast>, u64);

impl Global for StartToast {}

/// The documents page's right panels.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum DocPanel {
    Details,
    Versions,
    /// Live documents/113: Name *, Description, Category, Library label; Apply.
    Properties,
    /// Live documents/114: the document notes, read-only.
    Notes,
}

/// The Properties panel's fields for one document, and their saved values.
struct PropsEdit {
    path: PathBuf,
    name: Entity<InputState>,
    description: Entity<InputState>,
    label: Entity<InputState>,
    saved: (String, String, String),
}

/// Onshape's account button at the top-right (avatar, name, ▾). Its menu
/// has the local items only: Preferences… (the account page: the name used
/// as the author in Versions and history, keyboard shortcuts, …).
fn user_menu(cx: &App) -> AnyElement {
    let pal = p();
    let name = noshape_store::local_user_name();
    let variant = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.border.into()).active(pal.border.into()).shadow(false);
    Button::new("user-menu")
        .custom(variant)
        .h(px(30.))
        .px(px(4.))
        .rounded(px(3.))
        .child(
            h_flex()
                .gap(px(5.))
                .child(crate::assets::icon("user-avatar", 22., cx))
                .child(div().text_size(px(14.)).text_color(pal.text).max_w(px(150.)).truncate().child(name))
                .child(os::glyph(os::GLYPH_CARET, 8., pal.text)),
        )
        .os_dropdown_menu("user-menu-dd", false, |m, _, _| m.item(OsMenuItem::new("Preferences…").on_click(|_, _, cx| crate::preferences_window::show(crate::preferences_window::Page::Preferences, cx))))
        .into_any_element()
}

/// A 28x26 view toggle (list / grid): `#eaeaea` when on.
fn view_toggle(id: &'static str, glyph: &'static [u8], tip: &'static str, selected: bool) -> Stateful<Div> {
    let pal = p();
    div()
        .id(id)
        .w(px(28.))
        .h(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .cursor_pointer()
        .when(selected, |d| d.bg(pal.panel_active))
        .when(!selected, |d| d.hover(|d| d.bg(pal.panel_hover)))
        .child(os::glyph(glyph, 18., pal.text))
        .tooltip(crate::ui::tooltip(tip))
}

/// A folder's display name ("noshape documents" for the default folder).
fn folder_label(f: &Path) -> String {
    if f == crate::files::documents_dir() {
        "noshape documents".to_string()
    } else {
        f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| f.display().to_string())
    }
}

/// Path for display, with the home directory shortened to `~`.
fn display_path(p: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = p.strip_prefix(&home)
    {
        return format!("~/{}", rest.display());
    }
    p.display().to_string()
}

/// Where a new document goes: the documents folder ("Owned by me") or one
/// of the other folders shown in the start window.
pub(crate) struct Location {
    pub(crate) root: PathBuf,
    /// Folders added from outside the documents folder (top level).
    pub(crate) others: Vec<PathBuf>,
    /// The place shown; the destination.
    pub(crate) chosen: Rc<RefCell<PathBuf>>,
    /// A folder that can't be a destination (the folder being moved).
    pub(crate) exclude: Option<PathBuf>,
}

/// The names the user's history is written under (see `StartView::me`).
fn my_names() -> Vec<String> {
    let mut v = vec![noshape_store::local_user_name(), noshape_store::os_user_name(), noshape_store::os_login()];
    v.dedup();
    v
}

/// Subfolders of `dir` (not hidden), by name.
pub(crate) fn subfolders(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !e.file_name().to_string_lossy().starts_with('.')).map(|e| canon(&e.path())).collect())
        .unwrap_or_default();
    out.sort_by_key(|p| folder_label(p).to_lowercase());
    out
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Folders added from outside the documents folder (shown at the top level).
fn outside_folders() -> Vec<PathBuf> {
    let root = canon(&crate::files::documents_dir());
    crate::files::watched_folders().into_iter().map(|f| canon(&f)).filter(|f| !f.starts_with(&root) && f.is_dir()).collect()
}

impl Location {
    /// A browser starting at `chosen`.
    pub(crate) fn new(chosen: PathBuf) -> Self {
        Self { root: canon(&crate::files::documents_dir()), others: outside_folders(), chosen: Rc::new(RefCell::new(canon(&chosen))), exclude: None }
    }

    /// The folders listed in `dir`.
    fn children(&self, dir: &Path) -> Vec<PathBuf> {
        let mut v = subfolders(dir);
        if dir == self.root {
            let extra: Vec<PathBuf> = self.others.iter().filter(|o| !v.contains(o)).cloned().collect();
            v.extend(extra);
        }
        v.retain(|f| self.exclude.as_ref() != Some(f));
        v
    }

    /// Onshape's "Document location" browser: a header with home, back and
    /// the current place, above the list of its folders (click a folder to
    /// go into it; the place shown is the destination).
    pub(crate) fn render(&self) -> AnyElement {
        let pal = p();
        let chosen = self.chosen.borrow().clone();
        let at_root = chosen == self.root;
        let children = self.children(&chosen);
        let mut list = v_flex().p(px(6.)).gap(px(2.));
        if children.is_empty() {
            list = list.items_center().pt(px(15.)).child(os::info_box("No additional folders"));
        }
        for (i, f) in children.iter().enumerate() {
            let (c, f2) = (self.chosen.clone(), f.clone());
            list = list.child(
                os::row_states(h_flex().id(SharedString::from(format!("loc-{i}"))).h(px(30.)).px(px(8.)).gap(px(7.)).rounded(px(3.)).cursor_pointer().text_size(px(14.)), false)
                    .child(os::glyph(os::GLYPH_FOLDER, 18., pal.text))
                    .child(div().flex_1().min_w_0().truncate().child(folder_label(f)))
                    .on_click(move |_, window, _| {
                        *c.borrow_mut() = f2.clone();
                        window.refresh();
                    }),
            );
        }
        let go = |to: PathBuf| {
            let c = self.chosen.clone();
            move |_: &ClickEvent, window: &mut Window, _: &mut App| {
                *c.borrow_mut() = to.clone();
                window.refresh();
            }
        };
        let parent = if self.others.contains(&chosen) { self.root.clone() } else { chosen.parent().map(Path::to_path_buf).filter(|p| p.starts_with(&self.root) || self.others.iter().any(|o| p.starts_with(o))).unwrap_or_else(|| self.root.clone()) };
        v_flex()
            .h(px(242.))
            .border_1()
            .border_color(pal.border_strong)
            .rounded(px(4.))
            .child(
                h_flex()
                    .h(px(40.))
                    .flex_none()
                    .px(px(8.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(rgb(os::MODAL_DIVIDER))
                    .child(div().id("loc-home").cursor_pointer().child(os::glyph(os::GLYPH_HOME, 20., pal.text_muted)).on_click(go(self.root.clone())))
                    .child(
                        div()
                            .id("loc-back")
                            .when(!at_root, |d| d.cursor_pointer().on_click(go(parent)))
                            .child(os::glyph(os::GLYPH_BACK, 18., if at_root { pal.text_faint } else { pal.text_muted })),
                    )
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(os::glyph(if at_root { os::GLYPH_OWNED } else { os::GLYPH_FOLDER }, 18., pal.text))
                            .child(if at_root { "Owned by me".to_string() } else { folder_label(&chosen) }),
                    ),
            )
            .child(div().id("loc-list").flex_1().min_h_0().overflow_y_scrollbar().child(list))
            .into_any_element()
    }
}

/// What a name prompt shows.
struct NamePrompt {
    title: &'static str,
    ok_label: &'static str,
    label: &'static str,
    default: String,
    hint: Option<String>,
    /// The location browser and its label ("Document location").
    location: Option<(&'static str, Location)>,
    /// New document's "Document labels" field.
    labels: Option<LabelPick>,
}

/// The labels a new document gets (New document › Document labels).
struct LabelPick {
    all: Vec<String>,
    chosen: Rc<RefCell<Vec<String>>>,
}

impl LabelPick {
    /// Onshape's "Document labels" field: a 34 px box with the chosen labels
    /// as chips and the "Search labels" input (gray and disabled while there
    /// are no labels); while it has focus, the matching labels list under it.
    fn render(&self, search: &Entity<InputState>, window: &Window, cx: &App) -> AnyElement {
        let pal = p();
        let none = self.all.is_empty();
        let chosen = self.chosen.borrow().clone();
        let query = search.read(cx).value().trim().to_lowercase();
        let focused = search.read(cx).focus_handle(cx).is_focused(window);
        let mut field = h_flex()
            .min_h(px(34.))
            .flex_wrap()
            .gap(px(4.))
            .px(px(4.))
            .rounded(px(3.))
            .border_1()
            .border_color(if focused { rgb(0x3e94ff) } else { rgb(0xcccccc) })
            .bg(if none { rgb(0xeeeeee) } else { rgb(0xffffff) });
        for (k, l) in chosen.iter().enumerate() {
            let (c, l2) = (self.chosen.clone(), l.clone());
            field = field.child(
                h_flex()
                    .h(px(24.))
                    .px(px(6.))
                    .gap(px(4.))
                    .rounded(px(3.))
                    .bg(rgb(0xe9eef2))
                    .text_size(px(13.33))
                    .text_color(pal.text)
                    .child(os::glyph(os::GLYPH_LABEL, 14., pal.text))
                    .child(l.clone())
                    .child(div().id(("new-doc-label-x", k)).cursor_pointer().child(os::glyph(os::GLYPH_CLOSE, 10., pal.text_muted)).on_click(move |_, window, _| {
                        c.borrow_mut().retain(|x| *x != l2);
                        window.refresh();
                    })),
            );
        }
        field = field.child(div().flex_1().min_w(px(120.)).child(Input::new(search).appearance(false).disabled(none).h(px(30.)).text_size(px(14.))));
        let matches: Vec<String> = self.all.iter().filter(|l| !chosen.contains(l) && (query.is_empty() || l.to_lowercase().contains(&query))).cloned().collect();
        let mut col = v_flex().relative().child(field);
        if focused && !none {
            let mut list = v_flex().absolute().top(px(36.)).left_0().right_0().py(px(4.)).rounded(px(3.)).border_1().border_color(rgb(0xcccccc)).bg(rgb(0xffffff)).shadow_md();
            if matches.is_empty() {
                list = list.child(div().h(px(27.)).px(px(10.)).flex().items_center().text_size(px(14.)).text_color(pal.text_muted).child("No matching labels"));
            }
            for (k, l) in matches.into_iter().enumerate() {
                let (c, st) = (self.chosen.clone(), search.clone());
                list = list.child(
                    h_flex()
                        .id(("new-doc-label", k))
                        .h(px(27.))
                        .px(px(10.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .hover(|d| d.bg(rgb(0xeaeaea)))
                        .child(os::glyph(os::GLYPH_LABEL, 16., pal.text))
                        .child(l.clone())
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            c.borrow_mut().push(l.clone());
                            st.update(cx, |s, cx| s.set_value("", window, cx));
                            window.prevent_default();
                            window.refresh();
                        }),
                );
            }
            col = col.child(list);
        }
        col.into_any_element()
    }
}

/// Modal name prompt in Onshape's modal style. `then` returns whether to
/// close the dialog.
fn prompt_name(prompt: NamePrompt, window: &mut Window, cx: &mut App, then: impl Fn(String, &mut Window, &mut App) -> bool + 'static) {
    let NamePrompt { title, ok_label, label, default, hint, location, labels } = prompt;
    let label_search = labels.as_ref().map(|_| cx.new(|cx| InputState::new(window, cx).placeholder("Search labels")));
    if let Some(st) = &label_search {
        window
            .subscribe(st, cx, |_, ev: &InputEvent, window, _| {
                if matches!(ev, InputEvent::Change | InputEvent::Focus | InputEvent::Blur) {
                    window.refresh();
                }
            })
            .detach();
    }
    let state = cx.new(|cx| InputState::new(window, cx).default_value(default));
    // Enter reaches both the field subscription and the dialog's OK handler;
    // run the action once.
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let then = {
        let done = done.clone();
        Arc::new(move |name: String, window: &mut Window, cx: &mut App| {
            if done.get() {
                return true;
            }
            let ok = then(name, window, cx);
            done.set(ok);
            ok
        })
    };
    {
        let then = then.clone();
        let st = state.clone();
        window
            .subscribe(&state, cx, move |_, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    let name = st.read(cx).value().to_string();
                    if then(name, window, cx) {
                        window.close_dialog(cx);
                    }
                }
            })
            .detach();
    }
    let focus_state = state.clone();
    let location = location.map(Rc::new);
    let labels = labels.map(Rc::new);
    window.open_dialog(cx, move |dialog, window, cx| {
        let st = state.clone();
        let then = then.clone();
        os::modal(dialog, title, if location.is_some() { 598. } else { 460. })
            .child(
                os::modal_body()
                    .child(os::field_label(label))
                    .child(Input::new(&state).h(px(32.)).text_size(px(14.)))
                    .when_some(hint.clone(), |d, h| d.child(div().text_size(px(12.)).text_color(p().text_muted).child(h)))
                    .when_some(labels.clone().zip(label_search.clone()), |d, (l, st)| d.child(div().pt(px(10.)).child(os::field_label("Document labels"))).child(l.render(&st, window, cx)))
                    .when_some(location.clone(), |d, loc| d.child(div().pt(px(10.)).child(os::field_label(loc.0))).child(loc.1.render())),
            )
            .footer(
                os::modal_footer()
                    .child(os::modal_ok(os::primary_button("ok", ok_label)))
                    .child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))),
            )
            .on_ok(move |_, window, cx| {
                let name = st.read(cx).value().to_string();
                then(name, window, cx)
            })
    });
    window.defer(cx, move |window, cx| {
        focus_state.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        })
    });
}
