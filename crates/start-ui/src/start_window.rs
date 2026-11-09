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
        // The ui-parity driver's documents page (tests/ui-parity/README.md "Documents page and Preferences").
        let weak = view.downgrade();
        crate::ui_script::register_page(window.window_handle().window_id(), move |window, cx| {
            weak.upgrade().map(|v| {
                v.update(cx, |v, cx| v.ui_redraw(cx));
                v.read(cx).ui_state(window, cx)
            }).unwrap_or(serde_json::Value::Null)
        });
        let fh = view.read(cx).focus.clone();
        window.defer(cx, move |window, cx| fh.focus(window, cx));
        view
    }) {
        Ok((handle, view)) => cx.set_global(StartWindowGlobal { handle, view: view.downgrade() }),
        Err(e) => log::error!("could not open the start window: {e:#}"),
    }
}

/// Show the start window with an error message bubble (a document named on
/// the command line could not be opened).
pub fn show_with_error(message: impl Into<String>, cx: &mut App) {
    show(cx);
    let message = message.into();
    if let Some(handle) = cx.try_global::<StartWindowGlobal>().map(|g| g.handle) {
        // A document that couldn't be opened (qa OPEN-ERROR-SILENT): the
        // bubble stays until its × closes it; an 8 s one could run out before
        // a slow start showed the window.
        let _ = handle.update(cx, |_, window, cx| {
            let id = cx.default_global::<StartToast>().1 + 1;
            *cx.default_global::<StartToast>() = StartToast(Some(Toast { kind: AlertKind::Error.into(), bold: None, text: message, restore: None, brief: false }), id);
            window.refresh();
        });
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
    /// Owned by me's Folders section is open (collapsed by default; live remembers it).
    #[serde(default)]
    folders_open: bool,
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
        Self { view: ViewMode::Grid, sort: SortKey::Modified, descending: true, source: None, columns: default_columns(), folder_sort: FolderSort::NameAsc, panel: default_panel(), folders_open: false }
    }
}

fn prefs_path() -> PathBuf {
    crate::paths::config_dir().join("start.json")
}

fn load_prefs() -> Prefs {
    std::fs::read(prefs_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// The documents page opens on Owned by me with its Folders open next time (the ui-parity driver's page starts: live's
/// account).
pub fn open_on_owned_by_me() {
    let prefs = Prefs { source: Some(Source::All), folders_open: true, ..load_prefs() };
    let _ = std::fs::write(prefs_path(), serde_json::to_vec_pretty(&prefs).unwrap_or_default());
}

pub struct StartView {
    focus: FocusHandle,
    /// Help ▸ Keyboard shortcuts: the studios' sheet, over the page (live psui/docs-help-3-shortcuts).
    shortcut_sheet: Option<(Entity<crate::shortcut_sheet::ShortcutSheet>, Subscription)>,
    /// The Create button's keyboard focus: pressed, and back after its menu
    /// closes with Esc, it keeps live's focus look (dk-915).
    create_focus: FocusHandle,
    /// Set by a press on Create, so the window's press handler (after it)
    /// leaves the focus there.
    create_press: bool,
    /// The window's body (sidebar, documents, details) as a cached view, so a
    /// keystroke or caret blink in the search box re-renders only the title
    /// bar (see `crate::cached_slice`); the search notifies it.
    body: Option<Entity<crate::cached_slice::Slice<StartView>>>,
    library: Library,
    docs: Vec<DocEntry>,
    loaded: bool,
    folders: Vec<PathBuf>,
    source: Source,
    view: ViewMode,
    sort: SortKey,
    descending: bool,
    /// The sort arrow shows once a header was clicked this session (live
    /// documents/20, 80: the default order is unmarked).
    sort_shown: bool,
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
    /// Versions and history: the selected row and the rows whose "Show
    /// changes…" is open, of the document shown.
    history_sel: Option<(PathBuf, usize)>,
    history_open: std::collections::HashSet<(PathBuf, usize)>,
    /// Its "Search history" box and the Hide merges / Hide automatic
    /// versions toggles (on by default, live documents/118).
    history_search: Entity<InputState>,
    history_hide_merges: bool,
    history_hide_auto: bool,
    /// Its Search history ▾ panel: open, the filter applied and the one
    /// being edited, the two date fields.
    history_filter_open: bool,
    history_filter: noshape_ops::history_filter::HistoryFilter,
    history_filter_edit: noshape_ops::history_filter::HistoryFilter,
    history_dates: [Entity<InputState>; 2],
    /// The "Folders" section of "Owned by me" is expanded.
    /// A document's name being renamed inline in its row (row menu › Rename; live: no modal).
    renaming: Option<(PathBuf, Entity<InputState>, Subscription)>,
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
    /// Help ▸ Keyboard shortcuts opens (or closes) the sheet, on its General tab.
    fn toggle_shortcut_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.shortcut_sheet.take().is_some() {
            window.focus(&self.focus, cx);
        } else {
            let sheet = crate::shortcut_sheet::ShortcutSheet::open(crate::shortcuts::Tab::General, window, cx);
            let sub = cx.subscribe_in(&sheet, window, |this: &mut Self, _, _: &DismissEvent, window, cx| this.toggle_shortcut_sheet(window, cx));
            self.shortcut_sheet = Some((sheet, sub));
        }
        cx.notify();
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search documents"));
        let label_search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search labels"));
        let history_search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search history"));
        let history_dates = [cx.new(|cx| InputState::new(window, cx).placeholder("mm/dd/yyyy, --:--")), cx.new(|cx| InputState::new(window, cx).placeholder("mm/dd/yyyy, --:--"))];
        let col_search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search..."));
        let subs = vec![
            cx.subscribe(&col_search, |_, _, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    cx.notify();
                }
            }),
            cx.subscribe(&history_search, |_, _, ev: &InputEvent, cx| {
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
                let text = state.read(cx).value().to_string();
                match ev {
                    // The search runs on Enter, plain or written (`type:part name:pvz`, live documents/130-135;
                    // docs-search: the breadcrumb stays while typing); clearing the box ends it.
                    InputEvent::Change => {
                        if text.trim().is_empty() {
                            this.query = String::new();
                            this.adv_query = None;
                            this.refresh_body(cx);
                        }
                    }
                    InputEvent::PressEnter { .. } => {
                        match crate::start_search::Query::parse_text(&text, this.adv_query.as_ref().unwrap_or(&Default::default())) {
                            Some(q) => {
                                this.query = String::new();
                                this.adv_query = Some(q).filter(|q| *q != crate::start_search::Query::default());
                                this.load_facts(cx);
                            }
                            None => this.query = text.clone(),
                        }
                        this.refresh_body(cx);
                    }
                    _ => {}
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
        let adv_sub = cx.subscribe_in(&adv, window, |this: &mut Self, _, ev: &crate::start_search::SearchEvent, window, cx| {
            match ev {
                crate::start_search::SearchEvent::Search(q) => {
                    this.adv_query = Some(q.clone()).filter(|q| *q != crate::start_search::Query::default());
                    this.adv_open = false;
                    this.load_facts(cx);
                    // Live: the query is written into the search box.
                    // (Criteria the text can't hold — added criteria,
                    // Results from — leave the box as it is.)
                    let text = this.adv_query.as_ref().map(|q| q.to_text()).unwrap_or_default();
                    if !text.is_empty() {
                        this.search.update(cx, |s, cx| s.set_value(text, window, cx));
                        this.query = String::new();
                    }
                }
                crate::start_search::SearchEvent::Reset => this.adv_query = None,
            }
            cx.notify();
        });
        let mut subs = subs;
        subs.push(adv_sub);
        let create_focus = cx.focus_handle().tab_stop(true);
        subs.push(cx.on_focus(&create_focus, window, |_, _, cx| cx.notify()));
        subs.push(cx.on_blur(&create_focus, window, |_, _, cx| cx.notify()));
        let prefs = load_prefs();
        let source = prefs.source.clone().unwrap_or(if crate::doc_files::recent().is_empty() { Source::All } else { Source::Recent });
        let mut this = Self {
            focus: cx.focus_handle(),
            shortcut_sheet: None,
            create_focus,
            create_press: false,
            body: None,
            library: Library::default(),
            docs: Vec::new(),
            loaded: false,
            folders: ScanInput::current().folders,
            source,
            view: prefs.view,
            sort: prefs.sort,
            descending: prefs.descending,
            sort_shown: false,
            search,
            query: String::new(),
            selected: None,
            labels: lib::Labels::load(),
            labels_open: false,
            label_search,
            panel: prefs.panel,
            details: None,
            history_sel: None,
            history_open: Default::default(),
            history_search: history_search.clone(),
            history_hide_merges: true,
            history_hide_auto: true,
            history_filter_open: false,
            history_filter: Default::default(),
            history_filter_edit: Default::default(),
            history_dates,
            folders_open: prefs.folders_open,
            renaming: None,
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
        let prefs = Prefs { view: self.view, sort: self.sort, descending: self.descending, source: Some(self.source.clone()), columns: self.columns.clone(), folder_sort: self.folder_sort, panel: self.panel, folders_open: self.folders_open };
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
        self.sort_shown = true;
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
        match crate::start_hooks::open_document_window(path.clone(), None, gpu, cx) {
            Ok(()) => self.force_rescan(cx),
            Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not open document: {e:#}")),
        }
    }

    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = match &self.source {
            Source::Folder(f) => f.clone(),
            _ => crate::file_dialogs::documents_dir(),
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
            _ => crate::file_dialogs::documents_dir(),
        }
    }

    /// Create ▾ › Import files…: noshape documents open; CAD files go
    /// through live's "Import to Onshape" modal (the app's import dialog:
    /// one document, a document each, or one combined Part Studio, named
    /// after the first file, the files in "CAD Imports"); a DXF / DWG becomes
    /// a document with its drawing and the file, other files a document with
    /// the file tab (Onshape's import from the documents page).
    fn import_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let handle = window.window_handle();
        let place = self.current_place();
        crate::file_dialogs::request_open_paths("Import files", "All files", &["*"], window, cx, move |paths, cx| {
            let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
            let ext = |p: &PathBuf| p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let (cad, rest): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|p| noshape_ops::import::IMPORT_EXTS.contains(&ext(p).as_str()));
            for path in rest {
                let result = if ext(&path) == lib::EXT { crate::start_hooks::open_document_window(path.clone(), None, gpu.clone(), cx) } else { Self::import_file(&path, &place, gpu.clone(), cx) };
                Self::report_import(&[path], result, handle, cx);
            }
            if !cad.is_empty() {
                let view = view.clone();
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, cx| {
                        let files = cad.clone();
                        let on_import = move |choice: noshape_ops::import::ImportChoice, split: bool, _: &mut Window, cx: &mut App| {
                            // Split: a document each (a native assembly zip keeps its
                            // subassemblies and parts together).
                            let groups: Vec<Vec<PathBuf>> = if split { files.iter().map(|p| vec![p.clone()]).collect() } else { vec![files.clone()] };
                            for g in groups {
                                let result = Self::import_cad(&g, &choice, &place, gpu.clone(), cx);
                                Self::report_import(&g, result, handle, cx);
                            }
                            view.update(cx, |this, cx| this.force_rescan(cx));
                        };
                        crate::start_hooks::import_dialog(cad, Box::new(on_import), window, cx);
                    });
                });
            }
            view.update(cx, |this, cx| this.force_rescan(cx));
        });
    }

    /// A new document for CAD files with the modal's options, named after the
    /// first file (with its extension, as live): its Part Studios / Assemblies
    /// first, then "CAD Imports"; V1 made at once.
    fn import_cad(paths: &[PathBuf], choice: &noshape_ops::import::ImportChoice, place: &Path, gpu: crate::gpu::Gpu, cx: &mut App) -> anyhow::Result<()> {
        use noshape_ops::import as imp;
        let mut doc = crate::user_units::new_document();
        let units = doc.settings.units;
        doc.tabs.clear();
        doc.settings.tab_order.clear();
        let (inputs, mut errors) = imp::read_inputs(paths, choice, true, &units);
        let (_, failed) = imp::apply(&mut doc, inputs, choice, None, &units);
        errors.extend(failed);
        if !doc.tabs.iter().any(|t| !matches!(t.content, noshape_model::TabContent::File(_))) {
            anyhow::bail!("{}", errors.join("\n"));
        }
        let models: Vec<_> = doc.ordered_tabs().iter().filter(|t| !matches!(t.content, noshape_model::TabContent::File(_))).map(|t| t.id).collect();
        let rest: Vec<_> = doc.settings.tab_order.iter().copied().filter(|id| !models.contains(id)).collect();
        doc.settings.tab_order = models.into_iter().chain(rest).collect();
        // Live request 1742: the document is named after the whole file
        // name ("nested-asm-A.step") and has V1 at once (Start → V1 → Main).
        let (_, name) = imp::import_names(&paths[0]);
        let target = Self::free_target(place, &name);
        let mut session = noshape_ops::session::Session::create(&target, doc)?;
        session.create_version("V1", "")?;
        drop(session);
        crate::start_hooks::open_document_window(target, None, gpu, cx)?;
        // A file that didn't translate next to ones that did: the reason.
        if !errors.is_empty() {
            anyhow::bail!("{}", errors.join("\n"));
        }
        Ok(())
    }

    /// A new document for a file that isn't CAD: DXF / DWG the file and a
    /// drawing of it, any other file its file tab.
    fn import_file(path: &Path, place: &Path, gpu: crate::gpu::Gpu, cx: &mut App) -> anyhow::Result<()> {
        let mut doc = crate::user_units::new_document();
        let (name, file) = noshape_ops::import::import_names(path);
        let bytes = std::fs::read(path)?;
        let stored = noshape_model::file_tab::FileTab::new(&file, &bytes, noshape_model::file_tab::now());
        doc.tabs.clear();
        doc.settings.tab_order.clear();
        if stored.kind() == noshape_model::file_tab::FileKind::Drawing2d {
            let f = crate::file_tab_view::add_file_tab(&mut doc, stored, None, None);
            let dr = crate::file_tab_view::drawing_from_cad(&bytes).map_err(|e| anyhow::anyhow!(e))?;
            let id = doc.add_tab(&name, noshape_model::TabContent::Drawing(dr));
            noshape_ops::docedit::place_after(&mut doc, id, Some(f));
        } else {
            crate::file_tab_view::add_file_tab(&mut doc, stored, None, None);
        }
        crate::start_hooks::open_document_window(Self::free_target(place, &name), Some(doc), gpu, cx)
    }

    /// "<name>.noshape" in `place`, else "<name> (2).noshape", ….
    fn free_target(place: &Path, name: &str) -> PathBuf {
        let clean = lib::sanitize_name(name);
        let mut target = place.join(format!("{clean}.{}", lib::EXT));
        let mut n = 2;
        while target.exists() {
            target = place.join(format!("{clean} ({n}).{}", lib::EXT));
            n += 1;
        }
        target
    }

    /// An import's result in the Notifications bell (live EX11), and a failure's alert.
    fn report_import(paths: &[PathBuf], result: anyhow::Result<()>, handle: AnyWindowHandle, cx: &mut App) {
        let names: Vec<String> = paths.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
        if paths.iter().any(|p| p.extension().is_none_or(|e| e.to_string_lossy().to_lowercase() != lib::EXT)) {
            match &result {
                Ok(_) => {
                    for file in &names {
                        crate::notifications::push(cx, format!("Import {file}"), format!("Uploaded · Translated · Post-processing done · Created {file}"), false);
                    }
                }
                Err(e) => crate::notifications::push(cx, format!("Import {} failed", names.join(", ")), format!("{e:#}"), true),
            }
        }
        if let Err(e) = result {
            let msg = format!("Import failed: {e:#}");
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| Self::notify(window, cx, AlertKind::Error, msg));
            });
        }
    }

    /// Create ▾ › Folder…: a new folder in the current place.
    fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.current_place();
        let default = lib::next_untitled_folder(&start);
        let location = Location::new(start);
        let chosen = location.chosen.clone();
        let view = cx.entity();
        let prompt = NamePrompt { title: "New folder", ok_label: "Create", label: "Folder name", default, hint: None, location: Some(("Folder location", location)), labels: None };
        prompt_name(prompt, window, cx, move |name, window, cx| {
            let place = chosen.borrow().clone();
            let made = match lib::create_folder(&place, &name) {
                Ok(made) => made,
                Err(e) => {
                    Self::notify(window, cx, AlertKind::Warning, format!("{e:#}"));
                    return false;
                }
            };
            // Live docs-create-folder: the page goes into the new folder.
            view.update(cx, |this, cx| {
                this.folders_open = true;
                this.set_source(Source::Folder(made), cx);
                this.force_rescan(cx);
            });
            Self::toast(window, cx, Toast { kind: os::BubbleKind::Success, bold: None, text: "Folder created.".into(), restore: None, brief: false });
            true
        });
    }

    fn rename(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if crate::doc_files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{}: Close the document's window before renaming it", d.name));
            return;
        }
        let old = d.name.clone();
        self.start_inline_rename(d.path.clone(), d.name.clone(), move |this, name, window, cx| {
            if name == old {
                return;
            }
            match lib::rename_doc(&d.path, &name) {
                Ok(dest) => {
                    this.labels = lib::Labels::load();
                    this.selected = Some(dest.canonicalize().unwrap_or(dest));
                    this.force_rescan(cx);
                }
                Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Rename failed: {e:#}")),
            }
        }, window, cx);
    }

    /// A row's or folder tile's inline rename (live docs-row-menu-rename, docs-folder-menu-rename: no modal): the
    /// name becomes a field, all selected; Enter or leaving it runs `commit` with the new name, Esc keeps it.
    fn start_inline_rename(&mut self, path: PathBuf, name: String, commit: impl Fn(&mut Self, String, &mut Window, &mut Context<Self>) + 'static, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) || this.renaming.is_none() {
                return;
            }
            this.renaming = None;
            cx.notify();
            let name = st.read(cx).value().trim().to_string();
            if !name.is_empty() {
                commit(this, name, window, cx);
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.selected = None;
        self.renaming = Some((path, st, sub));
        cx.notify();
    }

    /// The inline rename's field when `path` is being renamed.
    fn inline_rename_field(&self, path: &Path, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, st, _) = self.renaming.as_ref().filter(|r| r.0 == path)?;
        Some(
            div()
                .id("inline-rename")
                .flex_1()
                .min_w_0()
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    // Esc keeps the name: the field goes before its blur would rename.
                    if ev.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.renaming = None;
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }
                }))
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(os::input_h(Input::new(st).text_size(px(14.)), 28.))
                .into_any_element(),
        )
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
        if crate::doc_files::is_document_open(&d.path, cx) {
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
                .child(crate::ui::user_text(name.to_string())),
        )
    }

    /// Onshape's "Send to trash" confirmation (live documents/47): "1
    /// selection will be sent to the trash." (bold) + "Show details" (the
    /// item list), **Send to trash** / Cancel.
    fn confirm_trash(&mut self, path: PathBuf, name: String, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        if folder && self.docs.iter().any(|d| d.path.starts_with(&path) && crate::doc_files::is_document_open(&d.path, cx)) {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the documents in this folder before moving it to the trash"));
            return;
        }
        let view = cx.entity();
        let shown = Rc::new(std::cell::Cell::new(false));
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            let (view, path, name) = (view.clone(), path.clone(), name.clone());
            let s2 = shown.clone();
            os::modal_doc_page(dialog, "Send to trash", 598.)
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
        match lib::send_to_trash(&path, &name, folder) {
            Ok(item) => {
                self.trash = crate::start_trash::Trash::load();
                if folder && matches!(&self.source, Source::Folder(f) if canon(f).starts_with(&item.original)) {
                    self.source = Source::All;
                    self.save_prefs();
                }
                self.selected = None;
                Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(name), text: " has been sent to the trash and will be automatically deleted after 30 days.".into(), restore: Some(item.id), brief: true });
                self.force_rescan(cx);
            }
            Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not move to trash: {e:#}")),
        }
    }

    /// Trash row menu › Restore and the toast's Restore link.
    fn restore_trashed(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        match lib::restore(&id, &crate::file_dialogs::documents_dir()) {
            Ok((_, item)) => {
                self.trash = crate::start_trash::Trash::load();
                // Live documents/86: "<name> has been restored." (brief).
                Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(item.name.clone()), text: " has been restored.".into(), restore: None, brief: true });
                self.folders = ScanInput::current().folders;
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
            os::modal_doc_page(dialog, "Delete forever?", 598.)
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
            os::modal_doc_page(dialog, "Empty trash?", 598.)
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
    fn folder_menu(view: Entity<StartView>, f: PathBuf, name: String, trash_id: Option<String>, menu: OsMenu, window: &mut Window, cx: &mut Context<OsMenu>) -> OsMenu {
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
        // Live's "Open in new browser tab": the documents page is one window here, so it opens the folder in it.
        menu.item(item("Open", &f, |t, f, _, cx| t.set_source(Source::Folder(f), cx)))
            .item(item("Open in new browser tab", &f, |t, f, _, cx| t.set_source(Source::Folder(f), cx)))
            .item(item("Rename", &f, |t, f, w, cx| t.rename_folder(f, w, cx)))
            .item(item("Details", &f, |t, f, _, cx| t.show_folder_details(f, cx)))
            .item(item("Move to…", &f, |t, f, w, cx| t.move_folder(f, w, cx)))
            .item(OsMenuItem::new("Send to trash").on_click(move |_, window, cx| {
                let (f, name) = (f2.clone(), name.clone());
                v.update(cx, |t, cx| t.confirm_trash(f, name, true, window, cx));
            }))
            .submenu("Set library", window, cx, |m, _, _| Self::library_menu(f, m))
    }

    /// Set library ▸: the folder becomes one of the user's libraries (op `documents_set_library`), ✓ the ones it is.
    fn library_menu(folder: PathBuf, menu: OsMenu) -> OsMenu {
        let set = noshape_ops::prefs::LibraryFolders::load();
        noshape_ops::prefs::LIBRARIES.iter().fold(menu, |m, (key, label)| {
            let folder = folder.clone();
            m.item(OsMenuItem::new(*label).checked(set.0.get(*key) == Some(&folder)).on_click(move |_, _, _| {
                let mut libs = noshape_ops::prefs::LibraryFolders::load();
                libs.0.insert(key.to_string(), folder.clone());
                let _ = libs.save();
            }))
        })
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
    /// After a folder's rename or move (`lib::rename_folder` / `lib::move_folder` moved its recent entries, labels
    /// and watched folder): the page's place follows.
    fn folder_moved(&mut self, old: &Path, new: &Path) {
        self.labels = lib::Labels::load();
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
        let name = folder_label(&f);
        self.start_inline_rename(f.clone(), name, move |this, name, window, cx| {
            let old = canon(&f);
            let dest = match lib::rename_folder(&f, &name) {
                Ok(dest) if dest == f => return,
                Ok(dest) => dest,
                Err(e) => {
                    Self::notify(window, cx, AlertKind::Warning, format!("{e:#}"));
                    return;
                }
            };
            this.folder_moved(&old, &canon(&dest));
            this.selected = Some(canon(&dest));
            this.force_rescan(cx);
        }, window, cx);
    }

    /// Folder menu › Move to…: the folder browser (not into itself).
    fn move_folder(&mut self, f: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let mut location = Location::new(f.parent().map(Path::to_path_buf).unwrap_or_else(crate::file_dialogs::documents_dir));
        location.exclude = Some(f.clone());
        let location = Rc::new(location);
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (loc, view, f) = (location.clone(), view.clone(), f.clone());
            os::modal_doc_page(dialog, "Move documents and folders to", 460.)
                .child(os::modal_body().child(os::field_label("Location")).child(location.render()))
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Move here"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let dir = loc.chosen.borrow().clone();
                    let old = canon(&f);
                    let dest = match lib::move_folder(&f, &dir) {
                        Ok(dest) if dest == f => return true,
                        Ok(dest) => dest,
                        Err(e) => {
                            Self::notify(window, cx, AlertKind::Warning, format!("{e:#}"));
                            return false;
                        }
                    };
                    view.update(cx, |this, cx| {
                        this.folder_moved(&old, &canon(&dest));
                        this.selected = Some(canon(&dest));
                        this.force_rescan(cx);
                    });
                    true
                })
        });
    }

    /// The page and its cached body drawn again (a search started or ended).
    fn refresh_body(&self, cx: &mut Context<Self>) {
        if let Some(b) = &self.body {
            b.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    fn forget(&mut self, d: DocEntry, cx: &mut Context<Self>) {
        crate::doc_files::remove_recent(&d.path);
        if self.selected.as_ref() == Some(&d.path) {
            self.selected = None;
        }
        self.force_rescan(cx);
    }

    // ------------------------------------------------------------- render

    // ------------------------------------------------------------- labels

    /// Create ▾ › Label… / "Create new label": Onshape's name prompt; the new
    /// label is applied to `apply_to` when given.
    fn create_label(&mut self, apply_to: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let prompt = NamePrompt { title: "Create new label", ok_label: "Create", label: "Label name", default: "Untitled label".into(), hint: None, location: None, labels: None };
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
        let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search labels"));
        window.subscribe(&search, cx, |_, ev: &InputEvent, window, _| {
            if let InputEvent::Change = ev {
                window.refresh();
            }
        })
        .detach();
        window.open_dialog(cx, move |dialog, _, cx| {
            let labels = view.read(cx).labels.clone();
            let body = Self::label_checklist(&view, &d.path, &search, &labels, cx);
            os::modal_doc_page(dialog, "Labels", 400.)
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
                .child(Input::new(search).appearance(false).h(px(28.)).text_size(px(14.)).pl(px(9.))),
        );
        if labels.labels.is_empty() {
            col = col.child(div().pt(px(6.)).pb(px(34.)).text_center().text_size(px(14.)).text_color(pal.text).child("You have no labels yet"));
        }
        for (k, l) in labels.labels.iter().enumerate().filter(|(_, l)| query.is_empty() || l.to_lowercase().contains(&query)) {
            let on = labels.has(path, l);
            let (v, p2, l2) = (view.clone(), path.to_path_buf(), l.clone());
            col = col.child(os::user_checkbox(("label-check", k), l.clone(), on).on_click(move |_, window, cx| {
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
                .child(os::label_add_glyph(20., pal.text))
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
                        .child(os::label_add_glyph(20., pal.text))
                        .tooltip(crate::ui::tooltip("Create label"))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.create_label(None, window, cx);
                        })),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.labels_open = !this.labels_open;
                    cx.notify();
                }))
                .map(|d| crate::ui_script::mark(d, || "nav:Labels".into())),
        );
        if open {
            for (k, l) in self.labels.labels.clone().into_iter().enumerate() {
                let source = Source::Label(l.clone());
                let mark_name = format!("nav:{l}");
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
                        .child(div().truncate().child(crate::ui::user_text(l)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_source(source.clone(), cx)))
                        .map(|d| crate::ui_script::mark(d, || mark_name)),
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
            crate::ui::rail_button(id, icon, on, false, cx).map(|d| crate::ui_script::mark(d, move || format!("button:{tip}"))).tooltip(crate::ui::tooltip(tip)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_panel(panel, cx)))
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

    /// Search history ▾ (live documents/117, dom/documents-history-search-filter):
    /// 260 px, Tab (Any or a tab), Tab types (Part Studio, Assembly,
    /// Drawing, Feature Studio, File) with Clear, Modified by (Any / Only me /
    /// Not me), Date modified from / to; Reset | Search. It filters the
    /// changes listed (and the rows that have one).
    fn render_history_filter(&self, tabs: &[(noshape_model::document::TabId, String, &'static str)], cx: &mut Context<Self>) -> AnyElement {
        use noshape_ops::history_filter::{HistoryFilter, ModifiedBy};
        const KINDS: [(&str, &str, &str); 5] = [("partstudio", "part-studio", "Part Studio"), ("assembly", "assembly", "Assembly"), ("drawing", "drawing", "Drawing"), ("featurestudio", "feature-studio", "Feature Studio"), ("file", "file", "File")];
        let pal = p();
        let edit = self.history_filter_edit.clone();
        let label = |t: &'static str| div().w(px(100.)).flex_none().pt(px(12.)).pl(px(4.)).text_size(px(11.)).text_color(rgb(0x666666)).child(t);
        let field = |id: &'static str, text: String| {
            h_flex()
                .id(id)
                .w(px(148.))
                .h(px(34.))
                .pl(px(8.))
                .pr(px(6.))
                .border_b_1()
                .border_color(rgb(0xeaeaea))
                .cursor_pointer()
                .text_size(px(14.))
                .child(div().flex_1().min_w_0().truncate().child(text))
                .child(os::glyph(os::GLYPH_CARET, 10., rgb(0x666666)))
        };
        let view = cx.entity();
        let tab_name = edit.tab.and_then(|t| tabs.iter().find(|x| x.0 == t)).map(|x| x.1.clone()).unwrap_or_else(|| "Any".into());
        let tab_list: Vec<(noshape_model::document::TabId, String)> = tabs.iter().map(|(t, n, _)| (*t, n.clone())).collect();
        let tab_select = field("vh-f-tab", tab_name).on_click(move |e: &ClickEvent, window, cx| {
            let (view, tabs) = (view.clone(), tab_list.clone());
            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                m = m.select_list(148.);
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new("Any").on_click(move |_, _, cx| v.update(cx, |t, cx| {
                    t.history_filter_edit.tab = None;
                    cx.notify();
                })));
                for (id, name) in &tabs {
                    let (v, id) = (view.clone(), *id);
                    m = m.item(crate::os_menu::OsMenuItem::new(name.clone()).on_click(move |_, _, cx| v.update(cx, |t, cx| {
                        t.history_filter_edit.tab = Some(id);
                        cx.notify();
                    })));
                }
                m
            });
            crate::os_menu::open(e.position(), menu, window, cx);
        });
        let view = cx.entity();
        let by_select = field("vh-f-by", edit.by.label().into()).on_click(move |e: &ClickEvent, window, cx| {
            let view = view.clone();
            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                m = m.select_list(148.);
                for by in ModifiedBy::ALL {
                    let v = view.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(by.label()).on_click(move |_, _, cx| v.update(cx, |t, cx| {
                        t.history_filter_edit.by = by;
                        cx.notify();
                    })));
                }
                m
            });
            crate::os_menu::open(e.position(), menu, window, cx);
        });
        let mut types = h_flex().w(px(148.)).flex_wrap().gap(px(6.)).pl(px(3.)).pt(px(4.));
        for (kind, icon, tip) in KINDS {
            let on = edit.kinds.contains(&kind);
            types = types.child(crate::ui::icon_button(SharedString::from(format!("vh-f-kind-{kind}")), icon, 30., on, cx).tooltip(tip).on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                let kinds = &mut t.history_filter_edit.kinds;
                if let Some(i) = kinds.iter().position(|k| *k == kind) {
                    kinds.remove(i);
                } else {
                    kinds.push(kind);
                }
                cx.notify();
            })));
        }
        let any_kind = !edit.kinds.is_empty();
        let date = |st: &Entity<InputState>, tip: &'static str, cx: &App| div().id(tip).w(px(148.)).h(px(34.)).pl(px(4.)).border_b_1().border_color(rgb(0xeaeaea)).flex().items_center().tooltip(crate::ui::tooltip(tip)).child(div().flex_1().child(Input::new(st).appearance(false).text_size(px(14.)))).child(div().pr(px(6.)).child(crate::assets::icon("calendar", 16., cx)));
        v_flex()
            .id("vh-filter-panel")
            .occlude()
            .w(px(260.))
            .bg(pal.panel)
            .border_1()
            .border_color(rgb(0xd7d7d7))
            .rounded(px(4.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(6.)), blur_radius: px(12.), spread_radius: px(0.), inset: false }])
            .px(px(5.))
            .text_color(pal.text)
            .on_mouse_down_out(cx.listener(|t, _: &MouseDownEvent, _, cx| {
                if !crate::os_menu::is_open(cx) {
                    t.history_filter_open = false;
                    cx.notify();
                }
            }))
            .child(h_flex().h(px(36.)).items_start().child(label("Tab")).child(tab_select))
            .child(
                h_flex().mt(px(14.)).items_start().child(label("Tab types")).child(
                    v_flex().w(px(148.)).child(types).child(
                        h_flex().justify_end().pt(px(3.)).child(
                            div()
                                .id("vh-f-clear")
                                .h(px(27.))
                                .px(px(7.))
                                .flex()
                                .items_center()
                                .rounded(px(3.5))
                                .border_1()
                                .border_color(rgb(0xaaaaaa))
                                .text_size(px(12.25))
                                .text_color(rgb(0x999999))
                                .when(!any_kind, |d| d.opacity(0.4))
                                .when(any_kind, |d| d.cursor_pointer().text_color(pal.text))
                                .child("Clear")
                                .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                                    t.history_filter_edit.kinds.clear();
                                    cx.notify();
                                })),
                        ),
                    ),
                ),
            )
            .child(h_flex().mt(px(14.)).h(px(36.)).items_start().child(label("Modified by")).child(by_select))
            .child(h_flex().mt(px(14.)).items_start().child(label("Date modified")).child(v_flex().gap(px(14.)).child(date(&self.history_dates[0], "Start date", cx)).child(date(&self.history_dates[1], "End date", cx))))
            .child(
                h_flex()
                    .justify_end()
                    .gap(px(4.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .pr(px(5.))
                    .child(os::secondary_button("vh-f-reset", "Reset", cx).on_click(cx.listener(|t, _: &ClickEvent, window, cx| {
                        t.history_filter_edit = HistoryFilter::default();
                        t.history_filter = HistoryFilter::default();
                        for st in t.history_dates.clone() {
                            st.update(cx, |s, cx| s.set_value("", window, cx));
                        }
                        cx.notify();
                    })))
                    .child(os::primary_button("vh-f-search", "Search").on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                        t.history_filter_edit.from = lib::parse_filter_date(&t.history_dates[0].read(cx).value(), false);
                        t.history_filter_edit.to = lib::parse_filter_date(&t.history_dates[1].read(cx).value(), true);
                        t.history_filter = t.history_filter_edit.clone();
                        t.history_filter_open = false;
                        cx.notify();
                    }))),
            )
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
                .child(div().truncate().text_size(px(17.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(crate::ui::user_text(name)))
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
                        .child(v_flex().items_center().pt(px(43.)).gap(px(20.)).child(svg().path(crate::assets::icon_path("notes-empty")).size(px(140.)).text_color(if gpui_kit::hex_theme_active() { crate::theme::p().illustration } else { gpui_kit::rgb(0xebebeb) })).child(div().text_size(px(14.67)).text_color(rgb(0x666666)).child("No notes available")))
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
                                .child(boxed(Input::new(&pe.name).appearance(false).text_size(px(14.)).into_any_element(), 32.).map(|d| crate::ui_script::mark(d, || "field:Properties › Name".into())))
                                .child(gap())
                                .child(label("Description", false))
                                .child(boxed(Input::new(&pe.description).appearance(false).text_size(px(14.)).into_any_element(), 52.).map(|d| crate::ui_script::mark(d, || "field:Properties › Description".into())))
                                .child(gap())
                                .child(label("Category", false))
                                // The category picker reads as disabled (one local category).
                                .child(boxed(h_flex().w_full().text_size(px(14.)).child(div().flex_1().child("Onshape Document")).child(os::glyph(os::GLYPH_CARET, 8., rgb(0x666666))).into_any_element(), 32.).bg(rgb(0xeeeeee)))
                                .child(gap())
                                .child(label("Library label", false))
                                .child(boxed(Input::new(&pe.label).appearance(false).text_size(px(14.)).into_any_element(), 32.).map(|d| crate::ui_script::mark(d, || "field:Properties › Library label".into()))),
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
                        .child(div().truncate().text_size(px(17.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(crate::ui::user_text(d.name.clone()))),
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
                // Where the file is on this computer: a link that shows it in its folder.
                .child(section("Location", vec![]).child(div().text_size(px(13.33)).line_height(px(20.)).child(folder_link("details-location", e.path.clone(), true))))
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
            (Some(_), Some(d)) => {
                let d_tabs = d.tabs.clone();
                v_flex()
                .size_full()
                // Live documents/116: the name, "Search history" with its 🔍,
                // the Hide merges / Hide automatic versions toggles.
                .child(name_row(d.name.clone()))
                .child(
                    h_flex()
                        .mx(px(12.))
                        .mt(px(8.))
                        .h(px(30.))
                        .relative()
                        .child(
                            h_flex()
                                .flex_1()
                                .h_full()
                                .border_1()
                                .border_color(rgb(0xaaaaaa))
                                .rounded_l(px(4.))
                                .child(div().flex_1().child(Input::new(&self.history_search).appearance(false).cleanable(true).text_size(px(14.)).h(px(28.)).px(px(6.))))
                                // ▾: the filter panel (live documents/117).
                                .child(
                                    div()
                                        .id("vh-filter-caret")
                                        .w(px(24.))
                                        .h_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .child(os::glyph(os::GLYPH_CARET, 10., if self.history_filter.is_active() { pal.accent } else { pal.text }))
                                        .on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                                            t.history_filter_open = !t.history_filter_open;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(div().size(px(30.)).flex().items_center().justify_center().border_1().border_l_0().border_color(rgb(0xaaaaaa)).rounded_r(px(4.)).child(crate::assets::icon("search", 20., cx)))
                        .when(self.history_filter_open, |d| d.child(deferred(div().absolute().top(px(31.)).right(px(0.)).child(self.render_history_filter(&d_tabs, cx))).with_priority(2))),
                )
                .child({
                    let toggle = |id: &'static str, icon: &'static str, on: bool, tip: &'static str, cx: &mut Context<Self>| {
                        div()
                            .id(id)
                            .size(px(30.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .cursor_pointer()
                            .when(on, |d| d.bg(rgb(0xeaeaea)))
                            .when(!on, |d| d.hover(|d| d.bg(rgb(0xeaeaea))))
                            .tooltip(crate::ui::tooltip(tip))
                            .child(crate::assets::icon(icon, 20., cx))
                    };
                    h_flex()
                        .px(px(12.))
                        .pt(px(6.))
                        .gap(px(6.))
                        .child(toggle("vh-merges", "hide-merges", self.history_hide_merges, if self.history_hide_merges { "Hide merges" } else { "Show merges" }, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.history_hide_merges = !this.history_hide_merges;
                            cx.notify();
                        })))
                        .child(toggle("vh-auto", "hide-auto-versions", self.history_hide_auto, if self.history_hide_auto { "Hide automatic versions" } else { "Show automatic versions" }, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.history_hide_auto = !this.history_hide_auto;
                            cx.notify();
                        })))
                })
                .child(
                    h_flex()
                        .h(px(38.))
                        .px(px(10.))
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .border_b_1()
                        .border_color(rgb(0xd0dbe5))
                        .child(div().w(px(100.)).flex_none())
                        .child(div().w(px(120.)).flex_none().child("Name"))
                        .child(div().child("Modified")),
                )
                .child(div().id("vh-rows").flex_1().min_h_0().overflow_y_scroll().child(v_flex().children({
                    // Live documents-versions-history-panel: a 140 px graph
                    // column (the workspace a white ring, versions blue
                    // dots, 16 px down, a line through each workspace's
                    // rows), then Name and Modified, 50 px rows.
                    let path = entry.as_ref().map(|e| e.path.clone()).unwrap_or_default();
                    // Search history: rows whose name or description holds the text.
                    let needle = self.history_search.read(cx).value().trim().to_lowercase();
                    let shown: Vec<lib::HistoryRow> = d
                        .history
                        .iter()
                        .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(&needle) || r.description.to_lowercase().contains(&needle))
                        // A filtered history shows the rows with a change that passes.
                        .filter(|r| !self.history_filter.is_active() || r.changes.iter().any(|c| history_change_matches(&self.history_filter, c, &d.tabs)))
                        .map(|r| {
                            let mut r = r.clone();
                            if self.history_hide_merges {
                                r.changes.retain(|c| !c.merge);
                            }
                            if self.history_filter.is_active() {
                                r.changes.retain(|c| history_change_matches(&self.history_filter, c, &d.tabs));
                            }
                            // A filtered list has other row numbers.
                            if !needle.is_empty() || self.history_filter.is_active() {
                                r.from = None;
                            }
                            r
                        })
                        .collect();
                    let rows = &shown;
                    let lanes = rows.iter().map(|r| r.lane).max().unwrap_or(0) + 1;
                    // A lane runs from its first row to its last, or on to
                    // the row it branched from.
                    let span = |lane: usize| {
                        let first = rows.iter().position(|r| r.lane == lane || (lane == 0 && r.name == "Start"));
                        let last = rows.iter().rposition(|r| r.lane == lane || (lane == 0 && r.name == "Start"));
                        let from = rows.iter().filter(|r| r.lane == lane).filter_map(|r| r.from).max();
                        first.zip(last.max(from))
                    };
                    let spans: Vec<Option<(usize, usize)>> = (0..lanes).map(span).collect();
                    let blue = rgb(0x1651b0);
                    let mut out = Vec::new();
                    for (i, r) in rows.iter().enumerate() {
                        let x = |lane: usize| 40. + 16. * lane as f32;
                        let mut graph = div().relative().w(px(100.)).flex_none();
                        // A branch's foot: from the version's dot across to its lane.
                        for (lane, sp) in spans.iter().enumerate() {
                            if lane > 0 && sp.is_some_and(|(_, b)| b == i) && rows.iter().any(|r| r.lane == lane && r.from == Some(i)) {
                                graph = graph.child(div().absolute().left(px(x(0))).top(px(15.5)).h(px(1.)).w(px(x(lane) - x(0))).bg(blue));
                            }
                        }
                        for (lane, sp) in spans.iter().enumerate() {
                            let Some((a, b)) = *sp else { continue };
                            if i > a && i <= b {
                                graph = graph.child(div().absolute().left(px(x(lane) - 0.5)).top_0().h(px(16.)).w(px(1.)).bg(blue));
                            }
                            if i >= a && i < b {
                                graph = graph.child(div().absolute().left(px(x(lane) - 0.5)).top(px(16.)).bottom_0().w(px(1.)).bg(blue));
                            }
                        }
                        let dot = if r.workspace {
                            div().absolute().left(px(x(r.lane) - 6.)).top(px(10.)).size(px(12.)).rounded_full().border_1().border_color(blue).bg(rgb(0xffffff))
                        } else {
                            div().absolute().left(px(x(r.lane) - 4.)).top(px(12.)).size(px(8.)).rounded_full().bg(blue)
                        };
                        graph = graph.child(dot);
                        let name_el = if r.workspace {
                            div().px(px(3.)).rounded(px(3.)).border_1().border_color(pal.accent).bg(rgb(0xd0dbe5)).font_weight(FontWeight::MEDIUM).child(crate::ui::user_text(r.name.clone()))
                        } else {
                            div().font_weight(FontWeight::MEDIUM).text_color(pal.text).child(crate::ui::user_text(r.name.clone()))
                        };
                        let open = self.history_open.contains(&(path.clone(), i));
                        let selected = self.history_sel.as_ref() == Some(&(path.clone(), i));
                        let tip = if r.description.trim().is_empty() { "No description".to_string() } else { format!("Description: {}", r.description) };
                        let (p1, p2) = (path.clone(), path.clone());
                        let show = (!r.changes.is_empty()).then(|| {
                            h_flex()
                                .id(SharedString::from(format!("vh-changes-{i}")))
                                .gap(px(3.))
                                .cursor_pointer()
                                .child(os::glyph(if open { os::GLYPH_EXPANDED } else { os::GLYPH_COLLAPSED }, 10., pal.text_muted))
                                .child(if open { "Hide changes…" } else { "Show changes…" })
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    let key = (p1.clone(), i);
                                    if !this.history_open.remove(&key) {
                                        this.history_open.insert(key);
                                    }
                                    cx.notify();
                                }))
                        });
                        let changes = open.then(|| {
                            v_flex().pt(px(4.)).gap(px(2.)).text_size(px(12.)).children(r.changes.iter().map(|lib::HistoryChange { description: desc, author, time: t, .. }| {
                                h_flex().gap(px(6.)).child(div().w(px(150.)).flex_none().truncate().text_color(pal.text).child(desc.clone())).child(div().truncate().child(format!("{} · {}", who(author), lib::format_onshape_time(*t))))
                            }))
                        });
                        out.push(h_flex()
                            .id(SharedString::from(format!("vh-row-{i}")))
                            .min_h(px(50.))
                            .items_stretch()
                            .text_size(px(14.))
                            .text_color(pal.text_muted)
                            .when(selected, |d| d.bg(rgb(0xb2ddf6)))
                            .tooltip(crate::ui::tooltip(tip))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.history_sel = Some((p2.clone(), i));
                                cx.notify();
                            }))
                            .child(graph)
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .pt(px(5.))
                                    .pb(px(4.))
                                    .child(
                                        h_flex()
                                            .items_start()
                                            .child(v_flex().w(px(120.)).flex_none().child(h_flex().child(name_el)).children(show))
                                            .child(v_flex().min_w_0().child(div().truncate().child(who(&r.author))).child(div().truncate().child(lib::format_onshape_time(r.time)))),
                                    )
                                    .children(changes),
                            ));
                    }
                    out
                })))
                // The legend (live documents/116, as the document's panel).
                .child({
                    let legend = |icon: &'static str, label: &'static str, faint: bool, cx: &mut Context<Self>| h_flex().gap(px(4.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(if faint { pal.text_faint } else { pal.text }).child(crate::assets::icon(icon, 20., cx)).child(label);
                    v_flex()
                        .flex_none()
                        .items_center()
                        .gap(px(10.))
                        .py(px(14.))
                        .child(h_flex().gap(px(30.)).child(legend("legend-workspace", "Workspace", false, cx)).child(legend("legend-version", "Version", false, cx)).child(legend("legend-change", "Change", true, cx)))
                        .child(h_flex().gap(px(30.)).child(legend("legend-release-candidate", "Release candidate", false, cx)).child(legend("legend-release", "Release", false, cx)))
                        .child(h_flex().gap(px(30.)).child(legend("legend-obsoletion", "Contains obsoletion", false, cx)))
                })
                .into_any_element()
            }
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
                            .child(div().truncate().text_size(px(17.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(crate::ui::user_text(name))),
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
                .child(svg().path(crate::assets::icon_path("documents-empty")).w(px(200.)).h(px(95.)).text_color(crate::theme::p().illustration))
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
                .child(div().opacity(0.25).map(|d| crate::ui_script::mark(d, || "button:Properties › Undo".into())).child(crate::ui::icon_button("props-undo", "undo", 30., false, cx)))
                .child(div().opacity(0.25).map(|d| crate::ui_script::mark(d, || "button:Properties › Redo".into())).child(crate::ui::icon_button("props-redo", "redo", 30., false, cx)))
                .child(os::primary_button("props-apply", "Apply").map(|d| crate::ui_script::mark(d, || "button:Properties › Apply".into())).when(!changed, |b| b.opacity(0.5)).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
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
        if crate::doc_files::is_document_open(&path, cx) {
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
                    crate::doc_files::replace_recent(&path, &d);
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
        if crate::doc_files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{}: Close the document's window before moving it", d.name));
            return;
        }
        let location = Rc::new(Location::new(d.folder().to_path_buf()));
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (loc, view, d) = (location.clone(), view.clone(), d.clone());
            os::modal_doc_page(dialog, "Move documents and folders to", 460.)
                .child(os::modal_body().child(os::field_label("Location")).child(location.render()))
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Move here"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let dir = loc.chosen.borrow().clone();
                    match lib::move_doc(&d.path, &dir) {
                        Ok(dest) => {
                            view.update(cx, |this, cx| {
                                this.labels = lib::Labels::load();
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
    fn doc_menu(view: Entity<StartView>, d: DocEntry, in_recent: bool, menu: OsMenu) -> OsMenu {
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
            // Live's "Open in new browser tab": a document always opens in a window of its own here.
            .item(item("Open in new browser tab", &view, &d, |t, d, w, cx| t.open_path(d.path, w, cx)))
            .item(item("Rename", &view, &d, |t, d, w, cx| t.rename(d, w, cx)))
            .item(item("Copy workspace…", &view, &d, |t, d, w, cx| t.duplicate(d, w, cx)))
            .separator()
            .item(item("Details", &view, &d, |t, d, _, cx| t.show_panel(d, DocPanel::Details, cx)))
            .item(item("Versions and history", &view, &d, |t, d, _, cx| t.show_panel(d, DocPanel::Versions, cx)))
            .item(item("Labels…", &view, &d, |t, d, w, cx| t.labels_dialog(d, w, cx)))
            .separator()
            .item(item("Move to…", &view, &d, |t, d, w, cx| t.move_to(d, w, cx)))
            .item(item(crate::file_dialogs::show_in_folder_label(), &view, &d, |_, d, _, cx| open_in_file_manager(&d.path, true, cx)))
            .separator();
        // Only in Recently opened (live docs-row-menu: Owned by me's rows have none).
        let menu = if in_recent && d.recent_rank.is_some() { menu.item(item("Remove from recently opened", &view, &d, |t, d, _, cx| t.forget(d, cx))) } else { menu };
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
                                        .child(gpui_kit::base::input::input_frame(div().flex_1().h_full().flex().items_center().map(|d| crate::ui_script::mark(d, || "field:Search".into())).child(Input::new(&self.search).text_size(px(14.)).appearance(false).small().h(px(26.)).cleanable(true).pl(px(12.)))))
                                        .child(
                                            div()
                                                .id("adv-caret")
                                                .map(|d| crate::ui_script::mark(d, || "caret:Search".into()))
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
                    // The update banner (the app's, start_hooks) beside the account menu, inside the block's fixed width.
                    .child(div().w(px(388.)).flex_none().flex().justify_end().items_center().gap(px(8.)).pr(px(10.)).child(help_menu(cx)).child(user_menu(cx))),
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
            .map(|d| crate::ui_script::mark(d, || format!("nav:{label}")))
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

    fn render_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let view = cx.entity();
        let labels_nav = self.render_labels_nav(cx);
        // Live (dk-913b, request 1669): Create's :focus look, #769fcc with a
        // 2 px #3e94ff ring, while its menu is open and after Esc closes it
        // (the focus comes back) until Tab or a press elsewhere moves the
        // focus; hover #769fcc too. Not themed.
        let lit = crate::os_ui::dropdown_open("create-dd", cx) || self.create_focus.is_focused(window);
        let create = div()
            .id("create")
            .map(|d| crate::ui_script::mark(d, || "button:Create".into()))
            .track_focus(&self.create_focus)
            .w(px(150.))
            .h(px(34.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .cursor_pointer()
            .text_color(gpui_kit::rgb_static(0xffffff))
            .bg(gpui_kit::rgb_static(if lit { 0x769fcc } else { 0x1651b0 }))
            .hover(|d| d.bg(gpui_kit::rgb_static(0x769fcc)))
            .when(lit, |d| d.shadow(vec![BoxShadow { color: gpui_kit::rgb_static(0x3e94ff).into(), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false }]))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, window, cx| {
                this.create_press = true;
                window.focus(&this.create_focus, cx);
            }))
            .child(h_flex().gap(px(6.)).text_size(px(14.)).child("Create").child(os::glyph(os::GLYPH_CARET, 8., gpui_kit::rgb_static(0xffffff))))
            .os_dropdown_menu("create-dd", false, move |menu, _, _| {
                // Onshape (live documents/41): Document…, Folder… | Import
                // files…, Import from ▸ (cloud drives, not offered locally),
                // Label…. Live dom documents-create-menu: a Bootstrap
                // dropdown, 20 px `#555` icons 20 px in, 14 px `#555` text
                // 48 px in; 150 px wide with 29.5 px rows.
                let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
                let item = |g: &'static [u8], label: &'static str| OsMenuItem::new(label).icon(os::glyph(g, 20., rgb(0x555555)));
                menu.dropdown().min_width(150.).row_height(29.5).text_inset(48.).item(item(os::GLYPH_DOCUMENT, "Document…").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.new_document(window, cx))))
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

    /// Every control repainted (the body is a cached slice): the ui-parity driver's marks.
    fn ui_redraw(&mut self, cx: &mut Context<Self>) {
        if let Some(b) = &self.body {
            b.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    /// The ui-parity state of the documents page (tests/ui-parity/README.md "Documents page and Preferences").
    fn ui_state(&self, window: &Window, cx: &App) -> serde_json::Value {
        let renaming = self.renaming.as_ref().map(|r| r.1.read(cx)).filter(|st| st.focus_handle(cx).is_focused(window));
        let focus = if let Some(st) = renaming {
            let mut f = crate::ui_script::field_focus("", st);
            f["kind"] = "other".into();
            f["label"] = serde_json::Value::Null;
            f
        } else if self.search.read(cx).focus_handle(cx).is_focused(window) {
            let mut f = crate::ui_script::field_focus("", self.search.read(cx));
            f["kind"] = "other".into();
            f["label"] = serde_json::Value::Null;
            f
        } else if window.focused(cx).is_some() && !self.focus.is_focused(window) {
            serde_json::json!({ "kind": "other" })
        } else {
            // The page itself holding the focus is live's body: none.
            serde_json::json!({ "kind": "none" })
        };
        let selected = self.selected.as_ref().map(|p| self.docs.iter().find(|d| d.path == *p).map(|d| d.name.clone()).unwrap_or_else(|| folder_label(p)));
        let panel = self.panel.map(|p| match p {
            DocPanel::Details => "Details",
            DocPanel::Versions => "Versions and history",
            DocPanel::Properties => "Properties",
            DocPanel::Notes => "Document notes",
        });
        serde_json::json!({
            "focus": focus,
            "dialog": crate::ui_script::open_dialog(),
            "list": crate::os_menu::ui_state(cx).or_else(crate::ui_script::open_list),
            "selection": null,
            "features": null,
            "parts": [],
            "message": crate::ui_script::window_message(window.window_handle().window_id()),
            "tooltip": null,
            "undo": null,
            "sketch": null,
            "drawing": null,
            "document": null,
            "documents": {
                "place": self.ui_place(),
                "selected": selected,
                "search": self.search.read(cx).value().to_string(),
                "panel": panel,
                "advanced": self.adv_open,
                "columns": self.cols_open,
            },
        })
    }

    /// The ui-parity state's `place`: live's breadcrumb leaf (a folder's name inside it, none over search results).
    fn ui_place(&self) -> Option<String> {
        if !self.query.trim().is_empty() || self.adv_query.is_some() {
            return None;
        }
        Some(match &self.source {
            Source::Folder(f) if canon(f) == canon(&crate::file_dialogs::documents_dir()) => "Owned by me".into(),
            _ => self.source_title(),
        })
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
                let docs = canon(&crate::file_dialogs::documents_dir());
                let top = outside_folders().into_iter().find(|o| f.starts_with(o));
                let mut v: Vec<PathBuf> = f.ancestors().take_while(|a| *a != docs && top.as_deref().is_none_or(|t| a.starts_with(t))).map(Path::to_path_buf).collect();
                v.reverse();
                v
            }
            _ => Vec::new(),
        };
        // A search (typed or advanced; live documents/130-135): "Search
        // results in" and the place in blue / 600 replace the header.
        if !self.query.trim().is_empty() || self.adv_query.is_some() {
            let place = match &self.source {
                Source::Folder(f) => folder_label(f),
                _ => root,
            };
            return h_flex()
                .h(px(52.))
                .flex_none()
                .pl(px(20.))
                .gap(px(5.))
                .text_size(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(pal.text)
                .child("Search results in")
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(pal.accent).child(place))
                .into_any_element();
        }
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
                    .map(|d| {
                        let r = root.to_string();
                        crate::ui_script::mark(d, move || format!("crumb:{r}"))
                    })
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
                div().id(("crumb", i)).map(|d| {
                    let name = folder_label(&c);
                    crate::ui_script::mark(d, move || format!("crumb:{name}"))
                }).child(folder_label(&c)).when(i < last, |d| {
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
                        .child(os::page_secondary_button("empty-trash", "Empty trash…", cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.confirm_empty_trash(window, cx)))),
                )
            })
            .into_any_element()
    }

    /// The folders shown as tiles in the current place: the top-level
    /// folders in "Owned by me", a folder's subfolders, trashed folders.
    fn folder_tiles(&self) -> Vec<(PathBuf, Option<String>)> {
        let mut tiles: Vec<(PathBuf, Option<String>)> = match &self.source {
            Source::All => {
                let root = canon(&crate::file_dialogs::documents_dir());
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
                    .map(|d| crate::ui_script::mark(d, || format!("doc_row:{name}")))
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
                    .child(self.inline_rename_field(&f, cx).unwrap_or_else(|| div().flex_1().min_w_0().truncate().child(crate::ui::user_text(name.clone())).into_any_element()))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.selected = Some(f1.clone());
                        if ev.click_count() >= 2 && !this.trash.items.iter().any(|t| t.stored() == f1) {
                            this.set_source(Source::Folder(f1.clone()), cx);
                        }
                        cx.notify();
                    }))
                    // Right-click selects too (the menu's handler stops propagation).
                    .os_context_menu(move |menu, window, cx| {
                        view.update(cx, |this, cx| {
                            this.selected = Some(f2.clone());
                            cx.notify();
                        });
                        Self::folder_menu(view.clone(), f.clone(), name.clone(), trash_id.clone(), menu, window, cx)
                    }),
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
                                        .map(|d| crate::ui_script::mark(d, || "button:Folders".into()))
                                        .h_full()
                                        .pl(px(23.))
                                        .gap(px(7.))
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(pal.text)
                                        // Only the chevron folds it (live: a click on the word does nothing).
                                        .child(div().id("folders-chevron").cursor_pointer().child(os::glyph(if expanded { os::GLYPH_EXPANDED } else { os::GLYPH_COLLAPSED }, 10., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_folders(cx))))
                                        .child(os::glyph(os::GLYPH_FOLDER, 20., pal.text))
                                        .child(div().ml(px(1.)).child(caption)),
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
                        // Live 90: a 2 px `#d0dbe5` rule.
                        .child(div().absolute().left_0().right_0().top(px(4.)).h(px(2.)).bg(pal.tooltip))
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
            self.save_prefs();
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
                    .child(h_flex().w(px(350.)).flex_none().pl(px(16.)).gap(px(10.)).child(os::glyph(os::GLYPH_FOLDER, 20., pal.text)).child(div().min_w_0().truncate().child(crate::ui::user_text(name))))
                    .child(div().w(px(200.)).flex_none().pl(px(16.)).child(lib::format_onshape_time(modified)))
                    .child(div().w(px(200.)).flex_none().pl(px(16.)).child("me"))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.selected = Some(f1.clone());
                        if ev.click_count() >= 2 && this.trash.item_of(&f1).is_none() {
                            this.set_source(Source::Folder(f1.clone()), cx);
                        }
                        cx.notify();
                    }))
                    // Right-click selects too (the menu's handler stops propagation).
                    .os_context_menu(move |menu, window, cx| {
                        v.update(cx, |this, cx| {
                            this.selected = Some(f2.clone());
                            cx.notify();
                        });
                        Self::folder_menu(v.clone(), f3.clone(), menu_name.clone(), tid.clone(), menu, window, cx)
                    }),
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
                    .map(|el| {
                        let name = d.name.clone();
                        crate::ui_script::mark(el, move || format!("recent_card:{name}"))
                    })
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
                    .child(div().flex_1().min_w_0().truncate().child(crate::ui::user_text(d.name.clone())))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| this.on_doc_click(&d1, ev, window, cx)))
                    .os_context_menu(move |menu, _, cx| {
                        let path = d2.path.clone();
                        v.update(cx, |this, cx| this.select(&path, cx));
                        let in_recent = v.read(cx).source == Source::Recent;
                        Self::doc_menu(v.clone(), d2.clone(), in_recent, menu)
                    }),
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
                        .map(|d| crate::ui_script::mark(d, || "button:Folders sort".into()))
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
        let active = self.sort_shown && key == Some(self.sort);
        h_flex()
            .id(id)
            .map(|d| crate::ui_script::mark(d, move || format!("col_header:{label}")))
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
            .map(|d| crate::ui_script::mark(d, || "button:Add".into()))
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
                .when(checked, |d| d.child(crate::ui::glyph("check", 11., gpui_kit::rgb_static(0xffffff))))
                .when(partial && !checked, |d| d.child(div().w(px(8.)).h(px(2.)).bg(rgb(0xffffff))))
        };
        let mut panel = v_flex().w(px(194.)).py(px(4.)).bg(pal.panel).border_1().border_color(rgb(0xeeeeee)).shadow_md().child(
            h_flex()
                .h(px(40.))
                .px(px(8.))
                .gap(px(8.))
                .child(div().id("cols-all").map(|d| crate::ui_script::mark(d, || "checkbox:All columns".into())).cursor_pointer().child(box_(all, on > 0)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    for c in this.columns.iter_mut() {
                        c.1 = !all;
                    }
                    this.save_prefs();
                    cx.notify();
                })))
                .child(div().w(px(142.)).map(|d| crate::ui_script::mark(d, || "field:Search…".into())).child(os::input_h(Input::new(&self.col_search).prefix(os::glyph(os::GLYPH_SEARCH, 14., pal.text_muted)), 32.).text_size(px(14.)).pl(px(7.)))),
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
                    .child(div().id(("col-check", i)).map(|d| crate::ui_script::mark(d, || format!("checkbox:{}", c.label()))).cursor_pointer().child(box_(shown, false)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.columns[i].1 = !this.columns[i].1;
                        this.save_prefs();
                        cx.notify();
                    })))
                    .child(
                        div()
                            .id(("col-grip", i))
                            .map(|d| {
                                let name = c.label();
                                crate::ui_script::mark(d, move || format!("col_grip:{name}"))
                            })
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

    /// A search for parts or tabs (live documents/135): a row for each item
    /// of the documents in the current place; None for a search of documents.
    fn item_hits(&self) -> Option<Vec<(DocEntry, crate::start_search::Hit)>> {
        let q = self.adv_query.as_ref().filter(|q| !q.item_types().is_empty())?;
        let typed = self.query.trim();
        let mut out = Vec::new();
        for d in self.docs.iter().filter(|d| !d.trashed && self.in_source(d, &self.source)) {
            let labels: Vec<String> = self.labels.labels.iter().filter(|l| self.labels.has(&d.path, l)).cloned().collect();
            if !crate::start_search::matches_document(q, &d.description, &d.owner, &labels) {
                continue;
            }
            let Some((_, facts)) = self.facts.get(&d.path) else { continue };
            out.extend(crate::start_search::hits(q, typed, facts).into_iter().map(|h| (d.clone(), h)));
        }
        match self.sort {
            SortKey::Name => out.sort_by_key(|(_, h)| h.name.to_lowercase()),
            SortKey::Modified => out.sort_by_key(|(d, _)| d.modified),
        }
        if self.descending {
            out.reverse();
        }
        Some(out)
    }

    /// The rows of a search for parts or tabs (live documents/135): the
    /// document's thumbnail, the item's icon, its name with the searched
    /// text in bold, ⚲ the workspace, a part's state badge; the document's
    /// Modified / Modified by / Owned by, a part's Part number and Revision.
    /// A double click opens the document at the item's tab.
    fn hit_rows(&self, hits: &[(DocEntry, crate::start_search::Hit)], cols: &[Col], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let me = self.me.clone();
        let who = move |name: &str| if name.is_empty() || me.iter().any(|m| m == name) { "me".to_string() } else { name.to_string() };
        let needle = self.adv_query.as_ref().map(|q| q.name.trim().to_lowercase()).filter(|n| !n.is_empty()).unwrap_or_else(|| self.query.trim().to_lowercase());
        hits.iter()
            .enumerate()
            .map(|(ix, (d, h))| {
                let cell = |w: f32| div().w(px(w)).flex_none().pl(px(16.)).truncate();
                let icon = match h.kind {
                    "Part" => "part",
                    "Part Studio" => "part-studio",
                    "Assembly" => "assembly",
                    "Drawing" => "drawing",
                    "Feature Studio" => "feature-studio",
                    "Variable Studio" => "variable-studio",
                    _ => "material-library",
                };
                // The searched text in bold ("**Part** 1").
                let name = match h.name.to_lowercase().find(&needle).filter(|_| !needle.is_empty()) {
                    Some(at) if h.name.is_char_boundary(at) && h.name.is_char_boundary(at + needle.len()) => h_flex()
                        .min_w_0()
                        .child(div().flex_none().child(h.name[..at].to_string()))
                        .child(div().flex_none().font_weight(FontWeight::BOLD).child(h.name[at..at + needle.len()].to_string()))
                        .child(div().min_w_0().truncate().child(h.name[at + needle.len()..].to_string())),
                    _ => h_flex().min_w_0().child(div().min_w_0().truncate().child(h.name.clone())),
                };
                let props = h.props.clone().unwrap_or_default();
                let state = (h.kind == "Part").then(|| props.meta.state().to_string());
                let (path, tab) = (d.path.clone(), h.tab);
                h_flex()
                    .id(SharedString::from(format!("hit-{ix}")))
                    .h(px(42.))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .border_b_1()
                    .border_color(rgb(0xececec))
                    .hover(|r| r.bg(pal.row_hover))
                    .child(
                        h_flex()
                            .w(px(350.))
                            .flex_none()
                            .pl(px(16.))
                            .pr(px(15.))
                            .gap(px(8.))
                            .child(self.thumbnail(d, 64.0, 36.0, false))
                            .child(div().flex_none().child(crate::assets::icon(icon, 16., cx)))
                            .child(name)
                            .child(h_flex().flex_none().gap(px(1.)).text_size(px(11.)).text_color(pal.text).child(os::glyph(os::GLYPH_WORKSPACE, 14., pal.text)).child(crate::ui::user_text(h.at.clone())))
                            .children(state.map(|s| div().flex_none().px(px(4.)).rounded(px(3.)).bg(rgb(0xe2e2e2)).text_size(px(11.)).text_color(rgb(0x666666)).child(s))),
                    )
                    .children(cols.iter().map(|c| {
                        cell(200.)
                            .child(match c {
                                Col::Modified => lib::format_onshape_time(d.modified),
                                Col::ModifiedBy => who(&d.modified_by),
                                Col::OwnedBy => who(&d.owner),
                                Col::Type => h.kind.to_string(),
                                Col::Description => props.description.clone().unwrap_or_default(),
                            })
                            .into_any_element()
                    }))
                    .child(cell(200.).child(props.part_number.clone().unwrap_or_default()))
                    .child(cell(200.).child(props.revision.clone().unwrap_or_default()))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                        if ev.click_count() >= 2 {
                            let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
                            match crate::start_hooks::open_document_at(path.clone(), tab, gpu, cx) {
                                Ok(()) => this.force_rescan(cx),
                                Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not open document: {e:#}")),
                            }
                        }
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn render_list(&self, docs: &[DocEntry], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let cols: Vec<Col> = self.columns.iter().filter(|c| c.1).map(|c| c.0).collect();
        // Live documents/131-135: a search adds Part number and Revision.
        let searching = !self.query.trim().is_empty() || self.adv_query.is_some();
        let search_cols: &[&'static str] = if searching { &["Part number", "Revision"] } else { &[] };
        let hits = self.item_hits();
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
        for c in search_cols {
            header = header.child(self.list_header_cell(c, c, None, cx).w(px(200.)).flex_none());
        }
        // Onshape writes the signed-in user as "me".
        let me = self.me.clone();
        let who = move |name: &str| if name.is_empty() || me.iter().any(|m| m == name) { "me".to_string() } else { name.to_string() };
        let rows: Vec<AnyElement> = if let Some(hits) = &hits {
            self.hit_rows(hits, &cols, cx)
        } else {
            docs
            .iter()
            .enumerate()
            .map(|(ix, d)| {
                let selected = self.selected.as_ref() == Some(&d.path);
                let open = crate::doc_files::is_document_open(&d.path, cx);
                let v2 = cx.entity();
                let (d1, d2) = (d.clone(), d.clone());
                let p1 = d.path.clone();
                let cell = |w: f32| div().w(px(w)).flex_none().pl(px(16.)).truncate();
                h_flex()
                    .id(SharedString::from(format!("row-{ix}")))
                    .map(|r| crate::ui_script::mark(r, || format!("doc_row:{}", d.name)))
                    .h(px(42.))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .border_b_1()
                    .border_color(rgb(0xececec))
                    .when(selected, |r| r.bg(pal.accent_soft))
                    .when(!selected, |r| r.hover(|r| r.bg(pal.row_hover)))
                    .when(d.trashed, |r| r.tooltip(crate::ui::tooltip("This document is in the trash and cannot be opened.")))
                    .child(
                        h_flex()
                            .w(px(350.))
                            .flex_none()
                            .pl(px(16.))
                            .pr(px(15.))
                            .gap(px(14.))
                            .child(self.thumbnail(d, 64.0, 36.0, false))
                            .child(self.inline_rename_field(&d.path, cx).unwrap_or_else(|| div().min_w_0().truncate().text_color(if d.broken { pal.warning } else { pal.text }).child(crate::ui::user_text(d.name.clone())).into_any_element()))
                            // The workspace chip (live documents-list: 14 px workspace icon, 11 px name).
                            .when(!d.workspace.is_empty(), |r| {
                                r.child(h_flex().flex_none().ml(px(-7.)).gap(px(1.)).text_size(px(11.)).text_color(pal.text).child(os::glyph(os::GLYPH_WORKSPACE, 14., pal.text)).child(crate::ui::user_text(d.workspace.clone())))
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
                    .children(search_cols.iter().map(|_| cell(200.).into_any_element()))
                    // Live documents/112: the selected row shows its quick actions at the
                    // right end (share is cloud-only): Send to trash, Labels.
                    .when(selected && !d.trashed, |r| {
                        let (d3, d4) = (d.clone(), d.clone());
                        let icon = |id: &'static str, glyph: &'static [u8], tip: &'static str| div().id(id).map(|d| crate::ui_script::mark(d, move || format!("row_icon:{tip}"))).size(px(20.)).flex_none().cursor_pointer().tooltip(crate::ui::tooltip(tip)).child(os::glyph(glyph, 20., pal.text));
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
                    // Right-click selects the row too (live documents/25); the
                    // menu's own handler stops propagation, so select here.
                    .os_context_menu(move |menu, _, cx| {
                        v2.update(cx, |this, cx| this.select(&p1, cx));
                        let in_recent = v2.read(cx).source == Source::Recent;
                        Self::doc_menu(v2.clone(), d2.clone(), in_recent, menu)
                    })
                    .into_any_element()
            })
            .collect()
        };
        let empty = rows.is_empty();
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
                        .w(px(350. + 200. * (cols.len() + search_cols.len()) as f32))
                        .child(header)
                        .child(if empty {
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
        } else if searching || self.adv_query.is_some() {
            // Live documents/131: two centred lines.
            div().max_w(px(560.)).text_center().child("No results found that match this search. Try other search terms or refine the search by clicking the arrow in the search box.").into_any_element()
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
                                .on_click(|_, _, cx| noshape_app_core::help::open("noshape help", cx)),
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
        // Live documents/90–92: centred in the list area, both ways.
        v_flex()
            .flex_1()
            .w_full()
            .items_center()
            .justify_center()
            // The art 166 x 150 in `#eeeeee`, 20 px over the text (live 90: the
            // glyph's drawing fills ~95 % x 87 % of its box).
            .gap(px(12.))
            .child(os::glyph(os::GLYPH_EMPTY, 167., rgb(0xeeeeee)))
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
            // The folder in the OS's own form, a link that opens it in Explorer / Finder / the file manager.
            .child(h_flex().flex_1().min_w_0().child("Documents are stored on this computer in\u{a0}").child(folder_link("documents-folder", crate::file_dialogs::documents_dir(), false)))
            // Live documents/90: the build, in parentheses, at the right end.
            .child(format!("({})", crate::start_hooks::build_label()))
            .into_any_element()
    }
}

impl StartView {
    /// Sidebar, documents and the details panel (see `body`).
    fn render_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let docs = self.visible();
        let header = self.render_header(cx);
        // Searching hides the folder area, except an advanced search for folders.
        let folders = if self.query.trim().is_empty() && self.adv_query.as_ref().is_none_or(|q| q.folders_only()) { self.render_folders(cx) } else { None };
        let strip = self.render_strip(cx);
        let panel = self.render_doc_panel(cx);
        let content = self.render_list(&docs, cx);
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .child(self.render_sidebar(window, cx))
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
            .children(panel)
            .size_full()
            .into_any_element()
    }
}

impl Render for StartView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The window has drawn: the viewport's shaders compile now.
        noshape_render::worker::release_prewarm();
        let pal = p();
        // Onshape: "Search in Owned by me" (the current place).
        let placeholder = format!("Search in {}", self.source_title());
        if self.placeholder != placeholder {
            self.placeholder = placeholder.clone();
            self.search.update(cx, |s, cx| s.set_placeholder(placeholder, window, cx));
        }
        self.ensure_props(window, cx);
        let body = match &self.body {
            Some(b) => b.clone(),
            None => {
                let b = crate::cached_slice::Slice::new(&cx.entity(), |this, window, cx| this.render_body(window, cx), cx);
                self.body = Some(b.clone());
                b
            }
        };
        v_flex()
            .size_full()
            .bg(pal.app_bg)
            .text_color(pal.text)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            // A press anywhere but on Create takes the focus off it, as a
            // click on the page blurs a focused button (live request 1669).
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, window, cx| {
                if !std::mem::take(&mut this.create_press) && this.create_focus.is_focused(window) {
                    window.focus(&this.focus, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NewDocument, window, cx| this.new_document(window, cx)))
            .on_action(cx.listener(|this, _: &crate::actions::ToggleShortcutHelp, window, cx| this.toggle_shortcut_sheet(window, cx)))
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
            .child(body.cached(StyleRefinement::default().w_full().flex_1().min_h_0()))
            .child(self.render_footer())
            .children(self.shortcut_sheet.as_ref().map(|(s, _)| s.clone().into_any_element()))
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
                let said = format!("{}{}{}", t.bold.clone().unwrap_or_default(), t.text, if t.restore.is_some() { " Restore" } else { "" });
                let bubble = match t.restore.clone() {
                    Some(id) => {
                        let view = cx.entity();
                        os::message_bubble_with_link("start-toast", t.kind, text, "Restore", move |_, window, cx| view.update(cx, |this, cx| this.restore_trashed(id.clone(), window, cx)), close, cx)
                    }
                    None => os::message_bubble("start-toast", t.kind, text, close, cx),
                };
                crate::ui_script::mark_message(div().absolute().top(px(8.)).left_0().right_0().flex().justify_center().child(bubble), || said)
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
/// Onshape's ? Help menu, in the documents page's navbar beside the account menu (live psui/docs-help-2-open: Help,
/// Explore Onshape, FeatureScript documentation, Keyboard shortcuts | What's new, Forums, Report a bug | System check,
/// About): its rows that work offline, Keyboard shortcuts and About, around the app's (Install command line tool…,
/// Connect to coding agents…), as a document's Help menu has them.
pub fn help_menu(cx: &App) -> AnyElement {
    let pal = p();
    let variant = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.border.into()).active(pal.border.into()).shadow(false);
    Button::new("start-help-menu")
        .custom(variant)
        .h(px(30.))
        .px(px(4.))
        .rounded(px(3.))
        .child(h_flex().gap(px(6.)).child(crate::assets::icon("help", 20., cx)).child(os::glyph(os::GLYPH_CARET, 8., pal.text)))
        .os_dropdown_menu(HELP_MENU, false, |m, _, _| crate::start_hooks::help_items(m))
        .debug_selector(|| HELP_MENU.into())
        .into_any_element()
}

/// The Help dropdown's id.
pub const HELP_MENU: &str = "start-help-menu-dd";

fn user_menu(cx: &App) -> AnyElement {
    let pal = p();
    let name = noshape_store::local_user_name();
    let variant = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.border.into()).active(pal.border.into()).shadow(false);
    Button::new("user-menu")
        .map(|d| crate::ui_script::mark(d, || "button:Account menu".into()))
        .custom(variant)
        .h(px(30.))
        .px(px(4.))
        .rounded(px(3.))
        .child(
            h_flex()
                .gap(px(5.))
                .child(crate::assets::icon("user-avatar", 22., cx))
                .child(div().text_size(px(14.)).text_color(pal.text).max_w(px(150.)).truncate().child(crate::ui::user_text(name)))
                .child(os::glyph(os::GLYPH_CARET, 8., pal.text)),
        )
        .os_dropdown_menu("user-menu-dd", false, |m, _, _| {
            // Live (cp-1719 c1-account-menu): My account · View support tickets · App Store dev portal · View in dark
            // mode beta · Sign out; the cloud ones left out, Company settings kept (release management, local).
            m.item(OsMenuItem::new("My account").on_click(|_, _, cx| crate::preferences_window::show(crate::preferences_window::Page::Profile, cx)))
                .item(OsMenuItem::new("View in dark mode beta").on_click(|_, _, cx| noshape_preferences_ui::prefs_general::apply_theme(!gpui_kit::hex_theme_active(), cx)))
                .item(OsMenuItem::new("Company settings").on_click(|_, _, cx| crate::company_settings::show(crate::company_settings::Section::ReleaseManagement, cx)))
        })
        .into_any_element()
}

/// A 28x26 view toggle (list / grid): `#eaeaea` when on.
fn view_toggle(id: &'static str, glyph: &'static [u8], tip: &'static str, selected: bool) -> Stateful<Div> {
    let pal = p();
    div()
        .id(id)
        .map(|d| crate::ui_script::mark(d, move || format!("button:{tip}")))
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
/// Does a change pass the Search history ▾ filter (author, time, tab,
/// tab kinds; `tabs`: the document's tabs and their kinds)?
fn history_change_matches(f: &noshape_ops::history_filter::HistoryFilter, c: &lib::HistoryChange, tabs: &[(noshape_model::document::TabId, String, &'static str)]) -> bool {
    use noshape_ops::history_filter::{is_me, ModifiedBy};
    let mine = is_me(&c.author);
    if (f.by == ModifiedBy::OnlyMe && !mine) || (f.by == ModifiedBy::NotMe && mine) {
        return false;
    }
    if f.from.is_some_and(|t| c.time < t) || f.to.is_some_and(|t| c.time > t) {
        return false;
    }
    if f.tab.is_none() && f.kinds.is_empty() {
        return true;
    }
    c.tabs.iter().any(|t| f.tab.is_none_or(|ft| ft == *t) && (f.kinds.is_empty() || tabs.iter().any(|(id, _, k)| id == t && f.kinds.contains(k))))
}

fn folder_label(f: &Path) -> String {
    if f == crate::file_dialogs::documents_dir() {
        "noshape documents".to_string()
    } else {
        f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| f.display().to_string())
    }
}

/// Path for display, with the home directory shortened to `~`.
/// `path` as the OS shows it, as a link: a folder opens in the OS's file manager; a document (`reveal`) shows in its
/// folder, selected.
pub fn folder_link(id: &'static str, path: PathBuf, reveal: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .map(|d| crate::ui_script::mark(d, || format!("button:{id}")))
        .min_w_0()
        .truncate()
        .cursor_pointer()
        .text_color(p().link)
        .hover(|d| d.underline())
        .child(crate::file_dialogs::display_path(&path))
        .on_click(move |_, _, cx| open_in_file_manager(&path, reveal, cx))
}

/// Opens `path` in the OS's file manager: a folder itself, a file (`reveal`) shown in its folder.
pub fn open_in_file_manager(path: &Path, reveal: bool, cx: &mut App) {
    if reveal {
        cx.reveal_path(path);
    } else {
        cx.open_with_system(path);
    }
}

/// Where a new document goes: the documents folder ("Owned by me") or one
/// of the other folders shown in the start window.
pub struct Location {
    pub root: PathBuf,
    /// Folders added from outside the documents folder (top level).
    pub others: Vec<PathBuf>,
    /// The place shown; the destination.
    pub chosen: Rc<RefCell<PathBuf>>,
    /// A folder that can't be a destination (the folder being moved).
    pub exclude: Option<PathBuf>,
}

/// The names the user's history is written under (see `StartView::me`).
fn my_names() -> Vec<String> {
    let mut v = vec![noshape_store::local_user_name(), noshape_store::os_user_name(), noshape_store::os_login()];
    v.dedup();
    v
}

/// Subfolders of `dir` (not hidden), by name.
pub fn subfolders(dir: &Path) -> Vec<PathBuf> {
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
    let root = canon(&crate::file_dialogs::documents_dir());
    crate::doc_files::watched_folders().into_iter().map(|f| canon(&f)).filter(|f| !f.starts_with(&root) && f.is_dir()).collect()
}

impl Location {
    /// A browser starting at `chosen`.
    pub fn new(chosen: PathBuf) -> Self {
        Self { root: canon(&crate::file_dialogs::documents_dir()), others: outside_folders(), chosen: Rc::new(RefCell::new(canon(&chosen))), exclude: None }
    }

    /// The place shown in the header: "Owned by me" or the folder's name.
    fn shown(&self) -> String {
        let chosen = self.chosen.borrow();
        if *chosen == self.root { "Owned by me".to_string() } else { folder_label(&chosen) }
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
    pub fn render(&self) -> AnyElement {
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
                os::row_states(h_flex().id(SharedString::from(format!("loc-{i}"))).map(|d| crate::ui_script::mark(d, || format!("list_row:{}", folder_label(f)))).h(px(30.)).px(px(8.)).gap(px(7.)).rounded(px(3.)).cursor_pointer().text_size(px(14.)), false)
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
                    .child(div().id("loc-home").map(|d| crate::ui_script::mark(d, || "button:Home".into())).cursor_pointer().child(os::glyph(os::GLYPH_HOME, 20., pal.text)).on_click(go(self.root.clone())))
                    .child(
                        div()
                            .id("loc-back")
                            .map(|d| crate::ui_script::mark(d, || "button:Back".into()))
                            .when(!at_root, |d| d.cursor_pointer().on_click(go(parent)))
                            .child(os::glyph(os::GLYPH_BACK, 18., pal.text)),
                    )
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(os::glyph(if at_root { os::GLYPH_OWNED } else { os::GLYPH_FOLDER }, 18., pal.text))
                            .child(self.shown()),
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
        field = field.child(div().flex_1().min_w(px(120.)).map(|d| crate::ui_script::mark(d, || "field:Search labels".into())).child(os::input_h(Input::new(search).appearance(false).disabled(none), 30.).text_size(px(14.)).pl(px(8.))));
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
    let label_search = labels.as_ref().map(|_| cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search labels").placeholder_style(gpui_kit::base::input::PlaceholderStyle { italic: true, weight: Some(gpui_kit::FontWeight::LIGHT), color: Some(gpui_kit::rgb(0x333333).into()) })));
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
        crate::ui_script::note_dialog(|| {
            let mut fields = vec![serde_json::json!({ "label": label, "kind": "input", "value": state.read(cx).value().to_string() })];
            if let Some((l, st)) = labels.as_ref().zip(label_search.as_ref()) {
                fields.push(serde_json::json!({ "label": "Document labels", "kind": "input", "value": l.chosen.borrow().join(", "), "search": st.read(cx).value().to_string() }));
            }
            // (Live's recorder reads no field for the location browser.)
            serde_json::json!({ "kind": "modal", "title": title, "fields": fields, "buttons": [ok_label, "Cancel"] })
        });
        os::modal_doc_page(dialog, title, if location.is_some() { 598. } else { 460. })
            .child(
                // Live documents/10: the first label right under the header,
                // 6 px between a field and the next label.
                os::modal_body()
                    .pt(px(0.))
                    .child(os::field_label(label))
                    // Live (dom/documents, inputs/5_New_document_modal_documentName):
                    // Bootstrap `form-control`, 34 px, radius 3, 1 px `#aaa`; focused
                    // a 2 px `#3e94ff` border (no glow) with the text kept 11 px in.
                    .child({
                        let focused = state.focus_handle(cx).is_focused(window);
                        gpui_kit::base::input::input_frame(
                            div()
                                .h(px(34.))
                                .flex()
                                .items_center()
                                .rounded(px(3.))
                                .bg(rgb(0xffffff))
                                .map(|d| if focused { d.border_2().border_color(rgb(0x3e94ff)).pl(px(9.)).pr(px(14.)) } else { d.border_1().border_color(rgb(0xaaaaaa)).pl(px(10.)).pr(px(15.)) })
                                .child(Input::new(&state).appearance(false).text_size(px(14.)).px(px(0.))),
                        )
                    })
                    .when_some(hint.clone(), |d, h| d.child(div().text_size(px(12.)).text_color(p().text_muted).child(h)))
                    .when_some(labels.clone().zip(label_search.clone()), |d, (l, st)| d.child(div().pt(px(6.)).child(os::field_label("Document labels"))).child(l.render(&st, window, cx)))
                    .when_some(location.clone(), |d, loc| d.child(div().pt(px(6.)).child(os::field_label(loc.0))).child(loc.1.render())),
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

#[cfg(test)]
mod tests {
    use super::{history_change_matches, lib};
    use noshape_model::document::TabId;
    use noshape_ops::history_filter::HistoryFilter;
    use std::prelude::v1::test;

    /// Search history ▾ on the documents page: a change passes by the tab
    /// or kind it changed and its time.
    #[test]
    fn history_filter_picks_changes() {
        let tabs = vec![(TabId(1), "Part Studio 1".to_string(), "partstudio"), (TabId(2), "Drawing 1".to_string(), "drawing")];
        let change = |tab: u64, time: i64| lib::HistoryChange { description: String::new(), author: "someone".into(), time, merge: false, tabs: vec![TabId(tab)] };
        let f = HistoryFilter { kinds: vec!["drawing"], ..Default::default() };
        assert!(history_change_matches(&f, &change(2, 10), &tabs));
        assert!(!history_change_matches(&f, &change(1, 10), &tabs));
        let f = HistoryFilter { tab: Some(TabId(1)), from: Some(5), to: Some(20), ..Default::default() };
        assert!(history_change_matches(&f, &change(1, 10), &tabs));
        assert!(!history_change_matches(&f, &change(1, 30), &tabs));
        assert!(!history_change_matches(&f, &change(2, 10), &tabs));
    }
}
