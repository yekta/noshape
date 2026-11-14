//! The documents start window: shown when noshape starts without a file and
//! whenever the last document window closes. Lists recent documents and the
//! `.noshape` files of the documents folder and user-added folders, as a
//! thumbnail grid or a sortable list, with search, create, open and per
//! document commands (rename, duplicate, reveal, trash, forget).

use crate::actions::{CloseWindow, NewDocument, OpenDocument};
use crate::start_library::{self as lib, DocEntry, Library, ScanInput};
use crate::os_ui as os;
use gpui_kit::component::Disableable as _;
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

actions!(noshape_start, [SelectNext, SelectPrev, ToggleRow, SelectAllRows]);

const CONTEXT: &str = "StartWindow";

fn icon(data: &'static [u8]) -> Icon {
    Icon::empty().data(data)
}

pub fn init(cx: &mut App) {
    cx.bind_keys(key_bindings());
}

/// The documents page's keys: only its list's (live chrome.json: no Onshape shortcuts there; Shift+/, Alt+C,
/// Ctrl+Space, Alt+T, Ctrl+Z do nothing). Live2 11-13: ↑ / ↓ move the selection (the Details pane follows), Space
/// toggles the row, Ctrl+A selects every row; Enter does nothing (a double-click opens), Ctrl+F is the browser's find.
fn key_bindings() -> Vec<KeyBinding> {
    let ctx = Some("StartWindow && !Input");
    vec![
        KeyBinding::new("secondary-a", SelectAllRows, ctx),
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("up", SelectPrev, ctx),
        KeyBinding::new("space", ToggleRow, ctx),
    ]
}

// ----------------------------------------------------------------- window

/// The documents windows (live: any number of browser tabs on the documents page; a folder's "Open in new browser
/// tab" opens another), oldest first.
#[derive(Default)]
struct StartWindows(Vec<(AnyWindowHandle, WeakEntity<StartView>)>);
impl Global for StartWindows {}

/// The documents window in front: the active one, else the newest.
fn front(cx: &App) -> Option<(AnyWindowHandle, WeakEntity<StartView>)> {
    let all = &cx.try_global::<StartWindows>()?.0;
    let active = cx.active_window();
    all.iter().find(|(h, _)| Some(*h) == active).or(all.last()).cloned()
}

/// Show the start window, or bring the existing one to the front.
pub fn show(cx: &mut App) {
    if let Some((handle, view)) = front(cx)
        && handle.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        let _ = view.update(cx, |v, cx| v.rescan(cx));
        return;
    }
    open_new(None, cx);
}

/// Another documents window, at `source` (else where a new one starts).
fn open_new(source: Option<Source>, cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1200.), px(780.)), cx))),
        window_min_size: Some(size(px(760.), px(480.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..TitleBar::window_options()
    };
    match crate::os_menu::open_window(options, cx, |window, cx| {
        window.set_window_title("Documents — Noshape");
        let view = cx.new(|cx| StartView::new(window, cx));
        // The ui-parity driver's documents page (tests/ui-parity/README.md "Documents page and Preferences").
        let weak = view.downgrade();
        crate::ui_script::register_page(window.window_handle().window_id(), move |window, cx| {
            weak.upgrade().map(|v| {
                v.update(cx, |v, cx| v.ui_redraw(cx));
                v.read(cx).ui_state(window, cx)
            }).unwrap_or(serde_json::Value::Null)
        });
        if let Some(source) = source {
            view.update(cx, |v, cx| v.set_source(source, cx));
        }
        let fh = view.read(cx).focus.clone();
        window.defer(cx, move |window, cx| fh.focus(window, cx));
        view
    }) {
        Ok((handle, view)) => cx.default_global::<StartWindows>().0.push((handle, view.downgrade())),
        Err(e) => log::error!("could not open the start window: {e:#}"),
    }
}

/// Show the start window with an error message bubble (a document named on
/// the command line could not be opened).
pub fn show_with_error(message: impl Into<String>, cx: &mut App) {
    show(cx);
    let message = message.into();
    if let Some((handle, _)) = front(cx) {
        // A document that couldn't be opened (qa OPEN-ERROR-SILENT): the
        // bubble stays until its × closes it; an 8 s one could run out before
        // a slow start showed the window.
        let _ = handle.update(cx, |_, window, cx| {
            let id = cx.default_global::<StartToast>().1 + 1;
            *cx.default_global::<StartToast>() = StartToast(Some((window.window_handle().window_id(), Toast { kind: AlertKind::Error.into(), bold: None, text: message, restore: Vec::new(), brief: false })), id);
            window.refresh();
        });
    }
}

/// Show the start window and ask for a new document's name (Ctrl+N from
/// any window).
pub fn show_and_create(cx: &mut App) {
    show(cx);
    let Some((handle, view)) = front(cx) else { return };
    let _ = handle.update(cx, |_, window, cx| {
        let _ = view.update(cx, |v, cx| v.new_document(window, cx));
    });
}

/// Re-read the library and redraw every documents window (e.g. after a document window closed).
pub fn refresh(cx: &mut App) {
    let views: Vec<_> = cx.try_global::<StartWindows>().map(|g| g.0.iter().map(|(_, v)| v.clone()).collect()).unwrap_or_default();
    for view in views {
        let _ = view.update(cx, |v, cx| {
            v.force_rescan(cx);
            cx.notify();
        });
    }
}

/// Called when a window closed: forgets it if it was a documents window.
pub fn forget_if(id: WindowId, cx: &mut App) -> bool {
    let g = cx.default_global::<StartWindows>();
    let before = g.0.len();
    g.0.retain(|(h, _)| h.window_id() != id);
    g.0.len() != before
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

/// A documents list row's height.
const DOC_ROW_H: f32 = 42.;

/// The first row's top and the visible part of the window around the list (window y, px), at the last paint.
#[derive(Clone, Copy)]
struct RowsClip {
    top: f32,
    clip: (f32, f32),
}

/// The rows (by their place in the list) to build: those on screen and two wheel steps' margin (the probe builds the
/// rest as they come into sight); before the list's place is known, a window's height of them.
fn rows_on_screen(clip: Option<RowsClip>) -> (usize, usize) {
    rows_within(clip, 240.)
}

/// The rows (by index) within `margin` px of the visible part of the list.
fn rows_within(clip: Option<RowsClip>, margin: f32) -> (usize, usize) {
    rows_of(clip, margin, DOC_ROW_H)
}

/// [`rows_within`] for rows `row_h` px high.
fn rows_of(clip: Option<RowsClip>, margin: f32, row_h: f32) -> (usize, usize) {
    let Some(c) = clip else { return (0, 40) };
    let row = |y: f32| ((y - c.top) / row_h).max(0.);
    (row(c.clip.0 - margin).floor() as usize, row(c.clip.1 + margin).ceil() as usize)
}

fn prefs_path() -> PathBuf {
    crate::paths::config_dir().join("start.json")
}

fn load_prefs() -> Prefs {
    std::fs::read(prefs_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// An import's choices and where its result goes (see `StartView::import_cad`).
struct ImportJob {
    choice: noshape_ops::import::ImportChoice,
    how: crate::start_hooks::PageImport,
    place: PathBuf,
    gpu: crate::gpu::Gpu,
    handle: AnyWindowHandle,
    view: Entity<StartView>,
}

/// How long Import files… waits for the file system to give a file before it says so (a hung network share).
const IMPORT_READ_TIMEOUT: Duration = Duration::from_secs(30);

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
    /// The folders set as the user's libraries (library key → folder): their icon is the library's, their menu
    /// ‹Type› library ▸ (live 2497, 2508).
    libraries: std::collections::BTreeMap<String, PathBuf>,
    docs: Vec<DocEntry>,
    loaded: bool,
    folders: Vec<PathBuf>,
    /// Folders added from outside the documents folder, and every folder's last change (unix ms): read with each scan,
    /// off the UI thread.
    outside: Vec<PathBuf>,
    dir_modified: std::collections::HashMap<PathBuf, i64>,
    /// A frame's drag of the selection (`drag_items`): made once, cleared as the next frame starts.
    drag_share: RefCell<Option<DragItems>>,
    /// The details panel, drawn as its own cached view.
    panel_slice: Option<Entity<crate::cached_slice::Slice<StartView>>>,
    /// Where the "N selections" list was last painted (its rows built around it).
    sel_clip: Rc<std::cell::Cell<Option<RowsClip>>>,
    source: Source,
    view: ViewMode,
    sort: SortKey,
    descending: bool,
    /// The sort arrow shows once a header was clicked this session (live
    /// documents/20, 80: the default order is unmarked).
    sort_shown: bool,
    search: Entity<InputState>,
    query: String,
    /// The selected row (the last clicked); with Ctrl / Shift + click, `also` holds the others, valid while `selected`
    /// is still `also_of` (anything else that selects a row drops them).
    selected: Option<PathBuf>,
    also: Vec<PathBuf>,
    also_of: Option<PathBuf>,
    /// Shift + click's range start.
    anchor: Option<PathBuf>,
    /// Ctrl+A selected every row (live: the Details panel shows no selection then).
    select_all: bool,
    /// A row took this click (the list's own click clears the selection otherwise).
    row_hit: bool,
    /// The rows being dragged (drawn faded).
    dragging: Vec<PathBuf>,
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
    history_dates: [Entity<crate::date_time_field::DateTimeField>; 2],
    /// The "Folders" section of "Owned by me" is expanded.
    /// A document's name being renamed inline in its row (row menu › Rename; live: no modal).
    renaming: Option<(PathBuf, Entity<InputState>, Subscription)>,
    /// Where the documents list's rows started and what of it was visible, at the last paint (`rows_on_screen`).
    rows_clip: Rc<std::cell::Cell<Option<RowsClip>>>,
    /// The rows the list built last (by index).
    rows_built: std::cell::Cell<(usize, usize)>,
    folders_open: bool,
    /// "Last opened by me" is expanded.
    recent_open: bool,
    /// The Notifications bell's panel (live: the documents page's bell, where an import's steps show).
    notifications_open: bool,
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
        // Live: native date-time inputs (live2 2405 (c)).
        let history_dates = [cx.new(crate::date_time_field::DateTimeField::new), cx.new(crate::date_time_field::DateTimeField::new)];
        for d in &history_dates {
            cx.observe(d, |_, _, cx| cx.notify()).detach();
        }
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
                    // The page takes the keyboard back (live docs-adv-name: focus none).
                    window.focus(&this.focus, cx);
                }
                crate::start_search::SearchEvent::Reset => this.adv_query = None,
            }
            cx.notify();
        });
        let mut subs = subs;
        subs.push(adv_sub);
        // Live docs-adv-name: the search box shows the query while its criteria are typed or picked.
        subs.push(cx.observe_in(&adv, window, |this: &mut Self, adv, window, cx| {
            let text = adv.read(cx).query(cx).to_text();
            if this.adv_open && this.search.read(cx).value() != text.as_str() {
                this.search.update(cx, |s, cx| s.set_value(text, window, cx));
            }
        }));
        let create_focus = cx.focus_handle().tab_stop(true);
        subs.push(cx.on_focus(&create_focus, window, |_, _, cx| cx.notify()));
        subs.push(cx.on_blur(&create_focus, window, |_, _, cx| cx.notify()));
        let prefs = load_prefs();
        let source = prefs.source.clone().unwrap_or(if crate::doc_files::recent().is_empty() { Source::All } else { Source::Recent });
        let mut this = Self {
            focus: cx.focus_handle(),
            shortcut_sheet: None,
            libraries: noshape_ops::prefs::LibraryFolders::load().0,
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
            also: Vec::new(),
            also_of: None,
            anchor: None,
            select_all: false,
            row_hit: false,
            dragging: Vec::new(),
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
            rows_clip: Default::default(),
            rows_built: Default::default(),
            recent_open: true,
            notifications_open: false,
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
            outside: Vec::new(),
            dir_modified: Default::default(),
            drag_share: Default::default(),
            panel_slice: None,
            sel_clip: Default::default(),
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
            let (input, result, outside, dir_modified) = cx
                .background_spawn(async move {
                    let t = std::time::Instant::now();
                    let input = ScanInput::current();
                    let result = library.scan(&input);
                    let outside = outside_folders();
                    let modified = |f: &Path| std::fs::metadata(f).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64);
                    let dir_modified: std::collections::HashMap<PathBuf, i64> = result.dirs.iter().chain(&outside).filter_map(|f| Some((f.clone(), modified(f)?))).collect();
                    log::info!(target: "perf", "documents page: {} documents read in {:.0} ms", result.docs.len(), t.elapsed().as_secs_f32() * 1e3);
                    (input, result, outside, dir_modified)
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
                t.outside = outside;
                t.dir_modified = dir_modified;
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
        self.visible_refs().into_iter().cloned().collect()
    }

    fn visible_refs(&self) -> Vec<&DocEntry> {
        let q = self.query.trim().to_lowercase();
        let mut v: Vec<&DocEntry> = self
            .docs
            .iter()
            .filter(|d| self.in_source(d, &self.source))
            .filter(|d| self.adv_matches(d))
            .filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q) || d.path.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&q)))
            .collect();
        match self.sort {
            SortKey::Name => v.sort_by_cached_key(|d| d.name.to_lowercase()),
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

    /// The rows the page lists, in order: folders, then documents.
    fn page_order(&self) -> Vec<PathBuf> {
        self.folder_tiles().into_iter().map(|(f, _)| f).chain(self.visible_refs().into_iter().map(|d| d.path.clone())).collect()
    }

    fn is_selected(&self, p: &Path) -> bool {
        self.selected.as_deref() == Some(p) || (self.also_of.is_some() && self.also_of == self.selected && self.also.iter().any(|q| q == p))
    }

    /// Every selected row, to test many rows against (a frame lists 2,000 rows with all of them selected). By the
    /// paths' text: a `Path` hashes component by component, several times slower (the page's paths are canonical).
    fn selection_set(&self) -> std::collections::HashSet<&std::ffi::OsStr> {
        let also = (self.also_of.is_some() && self.also_of == self.selected).then_some(&self.also);
        self.selected.iter().chain(also.into_iter().flatten()).map(|p| p.as_os_str()).collect()
    }

    /// How many rows are selected (without listing them).
    fn selection_len(&self) -> usize {
        let also = if self.also_of.is_some() && self.also_of == self.selected { self.also.len() } else { 0 };
        usize::from(self.selected.is_some()) + also
    }

    fn many_selected(&self) -> bool {
        self.selection_len() > 1
    }

    /// Every selected row, the last clicked first.
    fn selection(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.selected.iter().cloned().collect();
        if self.also_of.is_some() && self.also_of == self.selected {
            v.extend(self.also.iter().cloned());
        }
        v
    }

    /// A folder or document row clicked (`click_rows`).
    fn click_row(&mut self, row: &Path, m: &Modifiers) {
        self.row_hit = true;
        self.select_all = false;
        let (sel, anchor) = click_rows(&self.selection(), self.anchor.as_deref(), &self.page_order(), row, m.shift, m.secondary());
        self.selected = sel.first().cloned();
        self.also = sel.into_iter().skip(1).collect();
        self.also_of = self.selected.clone();
        self.anchor = anchor;
    }

    /// A right-click on `row`: it alone selected unless it is part of a multiple selection, which the menu acts on.
    fn context_select(&mut self, row: &Path) -> bool {
        if self.many_selected() && self.is_selected(row) {
            return true;
        }
        self.select_all = false;
        self.selected = Some(row.to_path_buf());
        self.anchor = Some(row.to_path_buf());
        false
    }

    /// The selected rows as the commands take them: (path, name, folder), the page's order.
    fn selection_items(&self) -> Vec<(PathBuf, String, bool)> {
        let sel = self.selection_set();
        let names: std::collections::HashMap<&std::ffi::OsStr, &str> = self.docs.iter().map(|d| (d.path.as_os_str(), d.name.as_str())).collect();
        self.page_order()
            .into_iter()
            .filter(|p| sel.contains(p.as_os_str()))
            .map(|p| match names.get(p.as_os_str()) {
                Some(name) => (p, name.to_string(), false),
                None => {
                    let name = folder_label(&p);
                    (p, name, true)
                }
            })
            .collect()
    }

    /// The menu of a multiple selection: what applies to every row in it.
    /// Live (feel/documents.json `contextMenuForN`): Details, Labels…, Move selected (N) to…, Send to trash.
    fn selection_menu(view: Entity<StartView>, menu: OsMenu, cx: &App) -> OsMenu {
        let n = view.read(cx).selection_len();
        let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view);
        menu.list_no_icons()
            .item(OsMenuItem::new("Details").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.show_selection_panel(false, cx))))
            .item(OsMenuItem::new("Labels…").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.show_selection_panel(true, cx))))
            .item(OsMenuItem::new(format!("Move selected ({n}) to…")).on_click(move |_, window, cx| v3.update(cx, |t, cx| t.move_selected(window, cx))))
            .item(OsMenuItem::new("Send to trash").on_click(move |_, window, cx| v4.update(cx, |t, cx| t.trash_selected(window, cx))))
    }

    /// The Details panel on the selection (`labels`: its Labels section open).
    fn show_selection_panel(&mut self, labels: bool, cx: &mut Context<Self>) {
        self.panel = Some(DocPanel::Details);
        if labels {
            self.details_labels_open = true;
        }
        self.save_prefs();
        cx.notify();
    }

    /// Ctrl+A: every row of the list.
    fn select_all_rows(&mut self, cx: &mut Context<Self>) {
        let order = self.page_order();
        self.selected = order.first().cloned();
        self.also = order.into_iter().skip(1).collect();
        self.also_of = self.selected.clone();
        self.select_all = true;
        cx.notify();
    }

    /// Clears the selection (a click on the list's empty space, the panel's Deselect all).
    fn deselect_all(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.also.clear();
        self.also_of = None;
        self.select_all = false;
        cx.notify();
    }

    /// One row out of a multiple selection (the panel's ×).
    fn deselect(&mut self, row: &Path, cx: &mut Context<Self>) {
        let rest: Vec<PathBuf> = self.selection().into_iter().filter(|p| p != row).collect();
        self.selected = rest.first().cloned();
        self.also = rest.into_iter().skip(1).collect();
        self.also_of = self.selected.clone();
        cx.notify();
    }

    /// Live (feel/documents.json `rightPanelForN`, docs-f1-shift-range): "⌄ N selections", a bordered list of the
    /// rows (each with its ✕), Trash and Deselect all, then "⌄ Labels" with the label checklist for all of them.
    fn render_selection_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let items = self.frame_selection_items();
        let view = cx.entity();
        let heading = |label: String| h_flex().h(px(32.)).gap(px(8.)).text_size(px(17.33)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(os::glyph(os::GLYPH_CHEVRON_DOWN, 12., pal.text)).child(label);
        // Only the rows in sight are built (2,000 selected took ~15 ms of layout a frame); the others are one spacer.
        const SEL_ROW_H: f32 = 36.;
        let built = rows_of(self.sel_clip.get(), 100., SEL_ROW_H);
        let probe = {
            let clip = self.sel_clip.clone();
            let panel = self.panel_slice.as_ref().map(|p| p.entity_id());
            canvas(
                move |b, window, cx| {
                    let mask = window.content_mask().bounds;
                    let now = RowsClip { top: f32::from(b.top()), clip: (f32::from(mask.top()), f32::from(mask.bottom())) };
                    clip.set(Some(now));
                    let seen = rows_of(Some(now), 0., SEL_ROW_H);
                    if seen.0 < built.0 || seen.1 > built.1 {
                        if let Some(panel) = panel {
                            cx.notify(panel);
                        }
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .w_full()
            .h(px(0.))
        };
        let spacer = |n: usize| div().h(px(SEL_ROW_H * n as f32)).flex_none().into_any_element();
        let mut list: Vec<AnyElement> = vec![probe.into_any_element()];
        let mut skipped = 0;
        for (i, item) in items.iter().enumerate() {
            if !(built.0..built.1).contains(&i) {
                skipped += 1;
                continue;
            }
            if skipped > 0 {
                list.push(spacer(std::mem::take(&mut skipped)));
            }
            list.push(Self::selection_row(&view, i, item));
        }
        if skipped > 0 {
            list.push(spacer(skipped));
        }
        let rows = v_flex().mt(px(6.)).border_1().border_color(rgb(0xcccccc)).children(list);
        let button = |id: &'static str| h_flex().id(id).h(px(32.)).px(px(10.)).gap(px(6.)).rounded(px(3.)).border_1().border_color(rgb(0xaaaaaa)).bg(rgb(0xffffff)).cursor_pointer().text_size(px(14.)).text_color(pal.text).map(move |d| crate::ui_script::mark(d, move || format!("button:{}", if id == "sel-trash" { "Trash" } else { "Deselect all" })));
        let (v1, v2) = (view.clone(), view.clone());
        let docs: Vec<PathBuf> = items.iter().filter(|i| !i.2).map(|i| i.0.clone()).collect();
        v_flex()
            .px(px(14.))
            .pt(px(10.))
            .child(heading(format!("{} selections", items.len())))
            .child(rows)
            .child(
                h_flex()
                    .mt(px(14.))
                    .justify_between()
                    .child(button("sel-trash").child(os::glyph(os::GLYPH_TRASH, 16., pal.text)).child("Trash").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.trash_selected(window, cx))))
                    .child(button("sel-deselect").child("Deselect all").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.deselect_all(cx)))),
            )
            .child(div().mt(px(20.)).child(heading("Labels".into())))
            .child(div().mt(px(8.)).rounded(px(2.)).border_1().border_color(rgb(0xcccccc)).child(Self::label_checklist(&view, &docs, &self.label_search, &self.labels, cx)))
            .into_any_element()
    }

    /// [`Self::selection_items`], made once a frame (each visible selected row's drag and the "N selections" list).
    fn frame_selection_items(&self) -> DragItems {
        self.drag_share.borrow_mut().get_or_insert_with(|| Rc::new(self.selection_items())).clone()
    }

    /// A row of the "N selections" list: icon, name, ✕.
    fn selection_row(view: &Entity<StartView>, i: usize, (path, name, folder): &(PathBuf, String, bool)) -> AnyElement {
        let pal = p();
        let (v, p1) = (view.clone(), path.clone());
        h_flex()
            .h(px(36.))
            .px(px(10.))
            .gap(px(8.))
            .when(i > 0, |d| d.border_t_1().border_color(rgb(0xcccccc)))
            .text_size(px(14.))
            .text_color(pal.text)
            .child(os::glyph(if *folder { os::GLYPH_FOLDER } else { os::GLYPH_DOCUMENT }, 18., pal.text))
            .child(div().flex_1().min_w_0().truncate().child(crate::ui::user_text(name.clone())))
            .child(div().id(("sel-remove", i)).cursor_pointer().child(os::glyph(os::GLYPH_CLOSE, 14., rgb(0x666666))).on_click(move |_, _, cx| v.update(cx, |t, cx| t.deselect(&p1, cx))))
            .into_any_element()
    }

    /// What a drag from `row` carries: the selection if the row is in a multiple one, else the row.
    /// `selected`: the row is. (A frame builds each visible row's drag at once: the selection's items are made once a
    /// frame and shared.)
    fn drag_items(&self, selected: bool, row: &Path, name: &str, folder: bool) -> DocsDrag {
        if selected && self.many_selected() {
            return DocsDrag::new(self.frame_selection_items());
        }
        DocsDrag::new(Rc::new(vec![(row.to_path_buf(), name.to_string(), folder)]))
    }

    /// Dragged rows dropped on folder `dir`: moved into it (those already there, and a folder onto itself, stay).
    fn drop_into_folder(&mut self, drag: &DocsDrag, dir: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let dir = canon(dir);
        let items = rows_to_move(&drag.items, &dir);
        if items.is_empty() {
            return;
        }
        if let Some((_, name, _)) = items.iter().find(|(p, _, folder)| !folder && crate::doc_files::is_document_open(p, cx)) {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the document's window before moving it"));
            return;
        }
        self.move_items(&items, &dir, window, cx);
        Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: None, text: "Selected items moved.".into(), restore: Vec::new(), brief: true });
    }

    /// Dragged rows dropped on the left panel's Trash: Send to trash for them (live: its dialog).
    fn drop_into_trash(&mut self, drag: &DocsDrag, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, name, _)) = drag.items.iter().find(|(p, _, folder)| !folder && crate::doc_files::is_document_open(p, cx)) {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the document's window before moving it to the trash"));
            return;
        }
        self.confirm_trash(drag.items.to_vec(), window, cx);
    }

    /// Send to trash on the selected rows (none of them open).
    fn trash_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.selection_items();
        if let Some((_, name, _)) = items.iter().find(|(p, _, folder)| !folder && crate::doc_files::is_document_open(p, cx)) {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the document's window before moving it to the trash"));
            return;
        }
        self.confirm_trash(items, window, cx);
    }

    /// Move to… on the selected rows: one folder picker, each moved into the folder chosen, the moved rows selected.
    fn move_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.selection_items();
        let Some((first, _, _)) = items.first().cloned() else { return };
        if let Some((_, name, _)) = items.iter().find(|(p, _, folder)| !folder && crate::doc_files::is_document_open(p, cx)) {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the document's window before moving it"));
            return;
        }
        let mut location = Location::new(first.parent().map(Path::to_path_buf).unwrap_or_else(crate::file_dialogs::documents_dir)).with_new_folder();
        location.exclude = items.iter().find(|i| i.2).map(|i| i.0.clone());
        let location = Rc::new(location);
        let view = cx.entity();
        let shown = Rc::new(std::cell::Cell::new(false));
        window.open_dialog(cx, move |dialog, _, cx| {
            let (loc, view, items) = (location.clone(), view.clone(), items.clone());
            let pal = p();
            let s2 = shown.clone();
            // Live (docs-move-dialog): "N items will be moved." and Show details under the folders.
            let count = h_flex()
                .mt(px(10.))
                .px(px(10.))
                .py(px(10.))
                .gap(px(14.))
                .bg(rgb(0xf0f0f0))
                .text_size(px(14.))
                .child(div().font_weight(FontWeight::BOLD).text_color(pal.text).child(format!("{} items will be moved.", items.len())))
                .child(div().id("move-details").cursor_pointer().text_color(pal.info_text).hover(|d| d.underline()).child(if shown.get() { "Hide details" } else { "Show details" }).on_click(move |_, window, _| {
                    s2.set(!s2.get());
                    window.refresh();
                }));
            os::modal_doc_page(dialog, "Move documents and folders to", 460.)
                // Live (docs-move-dialog): the folder browser (Home, Back, the place, New folder, Sort) under the title.
                .child(os::modal_body().child(location.render(cx)).child(count).when(shown.get(), |d| d.child(Self::selection_box(&items))))
                .footer(move_footer(&loc, items.iter().all(|i| already_in(&i.0, &loc.chosen.borrow())), cx))
                .on_ok(move |_, window, cx| {
                    if loc.open_picked(cx) {
                        window.refresh();
                        return false;
                    }
                    let dir = loc.chosen.borrow().clone();
                    view.update(cx, |this, cx| this.move_items(&items, &dir, window, cx));
                    true
                })
        });
    }

    /// Moves `items` into `dir` (documents and folders; a folder can't go into itself), the moved rows selected after.
    fn move_items(&mut self, items: &[(PathBuf, String, bool)], dir: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let (done, refused) = move_rows(items, dir);
        let mut moved = Vec::new();
        for (from, dest, folder) in done {
            if folder {
                self.folder_moved(&from, &dest);
            }
            moved.push(dest);
        }
        for (name, e) in refused {
            Self::notify(window, cx, AlertKind::Warning, format!("{name}: {e}"));
        }
        self.labels = lib::Labels::load();
        self.selected = moved.first().cloned();
        self.also = moved.into_iter().skip(1).collect();
        self.also_of = self.selected.clone();
        self.force_rescan(cx);
    }

    fn selected_entry(&self) -> Option<DocEntry> {
        let sel = self.selected.as_ref()?;
        self.docs.iter().find(|d| &d.path == sel).cloned()
    }

    /// ↑ / ↓: the row above / below the current one (the anchor, else the selected) alone, as a plain click selects it.
    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let order = self.page_order();
        if order.is_empty() {
            return;
        }
        let current = self.anchor.clone().or_else(|| self.selected.clone());
        let next = match current.and_then(|c| order.iter().position(|p| *p == c)) {
            None => 0,
            Some(i) => (i as isize + delta).clamp(0, order.len() as isize - 1) as usize,
        };
        self.click_row(&order[next].clone(), &Modifiers::default());
        cx.notify();
    }

    /// Space: the current row (the anchor) in or out of the selection, as a Ctrl+click.
    fn toggle_current_row(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.anchor.clone().or_else(|| self.selected.clone()) else { return };
        self.click_row(&row, &Modifiers { control: true, platform: cfg!(target_os = "macos"), ..Default::default() });
        cx.notify();
    }

    // ------------------------------------------------------------- commands

    /// Onshape's message bubble at the top center of the page (replaces the
    /// previous one; closes with its × or after 8 s).
    fn notify(window: &mut Window, cx: &mut App, kind: AlertKind, message: impl Into<String>) {
        Self::toast(window, cx, Toast { kind: kind.into(), bold: None, text: message.into(), restore: Vec::new(), brief: false });
    }

    /// Show a message bubble at the top of the page for 8 s.
    fn toast(window: &mut Window, cx: &mut App, toast: Toast) {
        let id = cx.default_global::<StartToast>().1 + 1;
        // Live (feel documents page): a toast with Restore stays ~5 s (4.97), a brief one 3, others 8.
        let secs = if !toast.restore.is_empty() { 5 } else if toast.brief { 3 } else { 8 };
        *cx.default_global::<StartToast>() = StartToast(Some((window.window_handle().window_id(), toast)), id);
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

    fn open_history(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        match crate::start_hooks::open_document_history(path, gpu, cx) {
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
                        let on_import = move |choice: noshape_ops::import::ImportChoice, how: crate::start_hooks::PageImport, _: &mut Window, cx: &mut App| {
                            // Split: each file its own document, a STEP assembly with subassemblies a folder of
                            // linked documents (noshape_ops::import_documents).
                            let split = how != crate::start_hooks::PageImport::Single;
                            let groups: Vec<Vec<PathBuf>> = if split { files.iter().map(|p| vec![p.clone()]).collect() } else { vec![files.clone()] };
                            for g in groups {
                                Self::import_cad(g, ImportJob { choice, how, place: place.clone(), gpu: gpu.clone(), handle, view: view.clone() }, cx);
                            }
                        };
                        crate::start_hooks::import_dialog(cad, Box::new(on_import), window, cx);
                    });
                });
            }
            view.update(cx, |this, cx| this.force_rescan(cx));
        });
    }

    /// The modal's documents for `paths` (noshape_ops::import_documents: one document, or a split file's folder of
    /// linked documents), made in the background while the bell's entry walks live's steps ("Importing <file>…":
    /// Uploading… → Uploaded, Translating… → Translated, Post-processing done, "Created link to <document>", which
    /// opens it; behaviours "STEP import flow", "dx speed"). The page stays as it is (live). A file the file system
    /// doesn't give within [`IMPORT_READ_TIMEOUT`] fails the import, saying so.
    fn import_cad(paths: Vec<PathBuf>, job_of: ImportJob, cx: &mut App) {
        use crate::notifications::{self as bell, Step};
        let ImportJob { choice, how, place, gpu, handle, view } = job_of;
        use noshape_ops::import_documents as imp;
        let names = paths.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect::<Vec<_>>().join(", ");
        let job = bell::start_job(cx, format!("Importing {names}…"), vec![Step::running("Uploading…")]);
        let template = crate::user_units::new_document();
        let ex = cx.background_executor().clone();
        cx.spawn(async move |cx| {
            let fail = |why: String, read: bool, cx: &mut App| {
                let mut steps = if read { vec![Step::done("Uploaded")] } else { Vec::new() };
                steps.extend([Step::failed(why.clone()), Step::failed("No post-processing")]);
                bell::update_job(cx, job, format!("Import {names} failed"), steps, true, None);
                let msg = format!("Import failed: {why}");
                let _ = handle.update(cx, |_, window, cx| Self::notify(window, cx, AlertKind::Error, msg));
            };
            // Uploading: the files read through once (a hung network share stops here, not in the translation).
            let files = paths.clone();
            let read = ex
                .spawn(async move { files.iter().try_for_each(|p| std::fs::read(p).map(drop).map_err(|e| format!("{}: {e}", p.display()))) })
                .with_timeout(IMPORT_READ_TIMEOUT, &ex)
                .await;
            let unread = match read {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(e),
                Err(_) => Some(format!("{names} could not be read: no answer from the file system in {} s", IMPORT_READ_TIMEOUT.as_secs())),
            };
            if let Some(why) = unread {
                cx.update(|cx| fail(why, false, cx));
                return;
            }
            cx.update(|cx| bell::update_job(cx, job, format!("Importing {names}…"), vec![Step::done("Uploaded"), Step::running("Translating…")], false, None));
            let made = ex
                .spawn(async move {
                    let new_doc = move || template.clone();
                    if how != crate::start_hooks::PageImport::Single
                        && let Some(m) = imp::import_split(&paths[0], &choice, how == crate::start_hooks::PageImport::PerPart, &place, &new_doc)?
                    {
                        let folder = m.folder.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        return anyhow::Ok((m.open, folder, Vec::new()));
                    }
                    let (target, errors) = imp::import_single(&paths, &choice, &place, new_doc())?;
                    let name = target.file_stem().map(|n| format!("Created link to {}", n.to_string_lossy())).unwrap_or_default();
                    Ok((Some(target), name, errors))
                })
                .await;
            cx.update(|cx| {
                match made {
                    Ok((open, link_text, errors)) => {
                        let mut steps = vec![Step::done("Uploaded"), Step::done("Translated"), Step::done("Post-processing done")];
                        // A file that didn't translate next to ones that did: the reason.
                        if !errors.is_empty() {
                            steps.push(Step::failed(errors.join("\n")));
                            let msg = format!("Import failed: {}", errors.join("\n"));
                            let _ = handle.update(cx, |_, window, cx| Self::notify(window, cx, AlertKind::Error, msg));
                        }
                        let link = open.map(|path| {
                            let action: bell::LinkAction = Rc::new(move |_: &mut Window, cx: &mut App| {
                                if let Err(e) = crate::start_hooks::open_document_window(path.clone(), None, gpu.clone(), cx) {
                                    log::warn!("open {}: {e:#}", path.display());
                                }
                            });
                            (link_text, action)
                        });
                        bell::update_job(cx, job, format!("Import {names}"), steps, false, link);
                    }
                    Err(e) => fail(format!("{e:#}"), true, cx),
                }
                view.update(cx, |this, cx| this.force_rescan(cx));
            });
        })
        .detach();
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
        crate::start_hooks::open_document_window(noshape_ops::import_documents::free_document_path(place, &name), Some(doc), gpu, cx)
    }

    /// "<name>.noshape" in `place`, else "<name> (2).noshape", ….
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
            // No bubble (live docs-create-folder: message none after Create).
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

    /// A row's Copy workspace…: live's "Copy the active workspace to a new document" (its name "<name> - Copy",
    /// "Copy document to"), the copy opened (as the document's ☰ › Copy workspace…; live docs-doc-menu-copy-workspace).
    fn copy_workspace(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        let start = d.path.parent().map(Path::to_path_buf).unwrap_or_else(crate::file_dialogs::documents_dir);
        let location = Location::new(start).with_new_folder();
        let chosen = location.chosen.clone();
        let view = cx.entity();
        let source = d.path.clone();
        let prompt = NamePrompt { title: "Copy the active workspace to a new document", ok_label: "Create document", label: "Document name", default: format!("{} - Copy", d.name), hint: None, location: Some(("Copy document to", location)), labels: None };
        prompt_name(prompt, window, cx, move |name, window, cx| {
            let clean = lib::sanitize_name(&name);
            if clean.is_empty() {
                Self::notify(window, cx, AlertKind::Warning, "Enter a name for the document");
                return false;
            }
            let dir = chosen.borrow().clone();
            match lib::copy_document_named(&source, &dir, &clean) {
                Ok(dest) => {
                    view.update(cx, |this, cx| {
                        this.selected = Some(dest.clone());
                        this.force_rescan(cx);
                        this.open_path(dest, window, cx);
                    });
                    true
                }
                Err(e) => {
                    Self::notify(window, cx, AlertKind::Error, format!("{e:#}"));
                    false
                }
            }
        });
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
        self.confirm_trash(vec![(d.path, d.name, false)], window, cx);
    }

    /// The bordered list of the items a trash dialog acts on (live
    /// documents/70): 1 px `#ccc`, one row per item with its icon.
    fn selection_box(items: &[(PathBuf, String, bool)]) -> Div {
        let pal = p();
        div().mt(px(14.)).ml(px(10.)).py(px(6.)).px(px(22.)).border_1().border_color(rgb(0xcccccc)).children(items.iter().map(|(_, name, folder)| {
            h_flex()
                .gap(px(8.))
                .text_size(px(14.))
                .text_color(pal.text)
                .child(os::glyph(if *folder { os::GLYPH_FOLDER } else { os::GLYPH_DOCUMENT }, 18., pal.text))
                .child(crate::ui::user_text(name.to_string()))
        }))
    }

    /// Onshape's "Send to trash" confirmation (live documents/47): "1
    /// selection will be sent to the trash." (bold) + "Show details" (the
    /// item list), **Send to trash** / Cancel.
    fn confirm_trash(&mut self, items: Vec<(PathBuf, String, bool)>, window: &mut Window, cx: &mut Context<Self>) {
        for (path, name, folder) in &items {
            if *folder && self.docs.iter().any(|d| d.path.starts_with(path) && crate::doc_files::is_document_open(&d.path, cx)) {
                Self::notify(window, cx, AlertKind::Warning, format!("{name}: Close the documents in this folder before moving it to the trash"));
                return;
            }
        }
        let view = cx.entity();
        let shown = Rc::new(std::cell::Cell::new(false));
        let count = match items.len() {
            1 => "1 selection will be sent to the trash.".to_string(),
            n => format!("{n} selections will be sent to the trash."),
        };
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            let (view, items, count) = (view.clone(), items.clone(), count.clone());
            let s2 = shown.clone();
            os::modal_doc_page(dialog, "Send to trash", 598.)
                .child(
                    os::modal_body()
                        .child(
                            h_flex()
                                .gap(px(10.))
                                .text_size(px(14.))
                                .child(div().font_weight(FontWeight::BOLD).text_color(pal.text).child(count))
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
                        .when(shown.get(), |d| d.child(Self::selection_box(&items))),
                )
                .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Send to trash"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let items = items.clone();
                    view.update(cx, |this, cx| this.send_to_trash(items, window, cx));
                    true
                })
        });
    }

    fn send_to_trash(&mut self, items: Vec<(PathBuf, String, bool)>, window: &mut Window, cx: &mut Context<Self>) {
        let mut sent = Vec::new();
        for (path, name, folder) in &items {
            match lib::send_to_trash(path, name, *folder) {
                Ok(item) => {
                    if *folder && matches!(&self.source, Source::Folder(f) if canon(f).starts_with(&item.original)) {
                        self.source = Source::All;
                        self.save_prefs();
                    }
                    sent.push((name.clone(), item.id));
                }
                Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not move to trash: {e:#}")),
            }
        }
        self.trash = crate::start_trash::Trash::load();
        self.selected = None;
        match sent.as_slice() {
            [] => {}
            [(name, id)] => Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(name.clone()), text: " has been sent to the trash and will be automatically deleted after 30 days.".into(), restore: vec![id.clone()], brief: true }),
            many => Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: None, text: format!("{} items have been sent to the trash and will be automatically deleted after 30 days.", many.len()), restore: many.iter().map(|(_, id)| id.clone()).collect(), brief: true }),
        }
        self.force_rescan(cx);
    }

    /// Trash row menu › Restore and the toast's Restore link.
    fn restore_trashed(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.restore_all(&[id], window, cx);
    }

    /// Trash items back where they were (the toast's Restore: all it sent), the selection cleared; live: "<name> has
    /// been restored." (documents/86), "N items have been restored." (feel documents page).
    fn restore_all(&mut self, ids: &[String], window: &mut Window, cx: &mut Context<Self>) {
        let mut names = Vec::new();
        for id in ids {
            match lib::restore(id, &crate::file_dialogs::documents_dir()) {
                Ok((_, item)) => names.push(item.name),
                Err(e) => Self::notify(window, cx, AlertKind::Error, format!("Could not restore: {e:#}")),
            }
        }
        self.trash = crate::start_trash::Trash::load();
        match names.as_slice() {
            [] => return,
            [name] => Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(name.clone()), text: " has been restored.".into(), restore: Vec::new(), brief: true }),
            many => Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: None, text: format!("{} items have been restored.", many.len()), restore: Vec::new(), brief: true }),
        }
        self.folders = ScanInput::current().folders;
        self.deselect_all(cx);
        self.force_rescan(cx);
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
                        .child(Self::selection_box(&[(PathBuf::new(), item.name.clone(), item.folder)])),
                )
                .footer(os::modal_footer().child(os::modal_ok(os::danger_button("ok", "Delete forever", cx))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let (id, name) = (id.clone(), name.clone());
                    view.update(cx, |this, cx| match this.trash.delete(&id) {
                        Ok(()) => {
                            this.selected = None;
                            Self::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: Some(name), text: " has been permanently deleted.".into(), restore: Vec::new(), brief: false });
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
        let library = view.read(cx).library_of(&f);
        let v3 = view.clone();
        // Live's "Open in new browser tab": another documents window, at the folder.
        let menu = menu
            .item(item("Open", &f, |t, f, _, cx| t.set_source(Source::Folder(f), cx)))
            .item(item("Open in new browser tab", &f, |_, f, _, cx| open_new(Some(Source::Folder(f)), cx)))
            .item(item("Rename", &f, |t, f, w, cx| t.rename_folder(f, w, cx)))
            .item(item("Details", &f, |t, f, _, cx| t.show_folder_details(f, cx)));
        // A folder set as a library (live 2497, 2508): no Move to… or Send to trash, its library's submenu.
        if let Some(key) = library {
            let (v1, v2) = (view.clone(), view);
            let (_, label, icon) = library_texts(key);
            return menu.text_inset(31.).submenu_with_icon(Some(icon), label, window, cx, move |m, _, _| {
                m.item(OsMenuItem::new("Update my library").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.set_library(key, None, window, cx))))
                    .item(OsMenuItem::new("Unset my library").on_click(move |_, window, cx| v2.update(cx, |t, cx| t.unset_library(key, window, cx))))
            });
        }
        menu.item(item("Move to…", &f, |t, f, w, cx| t.move_folder(f, w, cx)))
            .item(OsMenuItem::new("Send to trash").on_click(move |_, window, cx| {
                let (f, name) = (f2.clone(), name.clone());
                v.update(cx, |t, cx| t.confirm_trash(vec![(f, name, true)], window, cx));
            }))
            .submenu("Set library", window, cx, move |m, _, _| Self::library_menu(v3, f, m))
    }

    /// Set library ▸: the folder becomes one of the user's libraries (op `documents_set_library`), ✓ the ones it is.
    fn library_menu(view: Entity<StartView>, folder: PathBuf, menu: OsMenu) -> OsMenu {
        let set = noshape_ops::prefs::LibraryFolders::load();
        noshape_ops::prefs::LIBRARIES.iter().fold(menu, |m, &(key, label)| {
            let (folder, view) = (folder.clone(), view.clone());
            m.item(OsMenuItem::new(label).icon_name(library_texts(key).2).checked(set.0.get(key) == Some(&folder)).on_click(move |_, window, cx| {
                view.update(cx, |t, cx| t.set_library(key, Some(folder.clone()), window, cx));
            }))
        })
    }

    /// Set library ▸ (`folder`) or ‹Type› library ▸ Update my library (`None`): checked on a background thread (a
    /// forms library's every Part Studio), "Updating your … library..." until then; a failure is live's "Failed to
    /// update the library" with a paragraph per problem (live 2497, 2508).
    fn set_library(&mut self, key: &'static str, folder: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let (kind, _, _) = library_texts(key);
        Self::notify(window, cx, AlertKind::Info, format!("Updating your{kind} library..."));
        let args = match &folder {
            Some(f) => serde_json::json!({ "library": key, "folder": f }),
            None => serde_json::json!({ "library": key, "update": true }),
        };
        let run = cx.background_spawn(async move {
            let op = noshape_ops::registry::by_tool_name("documents_set_library").expect("documents_set_library");
            noshape_ops::registry::run(&mut noshape_ops::Ctx::new(), op, &args).map(|o| o.value["up_to_date"] == true).map_err(|e| e.message)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = run.await;
            let _ = this.update_in(cx, |t, window, cx| {
                match result {
                    Ok(same) => {
                        t.libraries = noshape_ops::prefs::LibraryFolders::load().0;
                        let text = if same { format!("Your{kind} library is already up to date.") } else { format!("Successfully updated your{kind} library.") };
                        Self::toast(window, cx, Toast { kind: os::BubbleKind::Success, bold: None, text, restore: Vec::new(), brief: false });
                    }
                    Err(message) => {
                        cx.default_global::<StartToast>().0 = None;
                        Self::library_failed(message, window, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// "Failed to update the library" (live 2497 15): one paragraph per problem, Close.
    fn library_failed(message: String, window: &mut Window, cx: &mut App) {
        let mut parts = message.split("\n\n").map(str::to_string);
        let title = parts.next().unwrap_or_default();
        let problems: Vec<String> = parts.collect();
        window.open_dialog(cx, move |dialog, _, cx| {
            let pal = p();
            os::modal_doc_page(dialog, title.clone(), 598.)
                .child(os::modal_body().gap(px(12.)).children(problems.iter().map(|t| div().text_size(px(13.)).text_color(pal.text).child(t.clone()))))
                .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("close", "Close", cx))))
        });
    }

    /// ‹Type› library ▸ Unset my library.
    fn unset_library(&mut self, key: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let op = noshape_ops::registry::by_tool_name("documents_set_library").expect("documents_set_library");
        let unset = noshape_ops::registry::run(&mut noshape_ops::Ctx::new(), op, &serde_json::json!({ "library": key, "clear": true }));
        if let Err(e) = unset {
            Self::notify(window, cx, AlertKind::Error, e.message);
            return;
        }
        self.libraries.remove(key);
        let (kind, _, _) = library_texts(key);
        Self::toast(window, cx, Toast { kind: os::BubbleKind::Success, bold: None, text: format!("Successfully unset your{kind} library."), restore: Vec::new(), brief: false });
        cx.notify();
    }

    /// The library `f` is set as, if any.
    fn library_of(&self, f: &Path) -> Option<&'static str> {
        noshape_ops::prefs::LIBRARIES.iter().map(|(k, _)| *k).find(|k| self.libraries.get(*k).is_some_and(|p| p == f))
    }

    /// A folder's icon: the library's for one set as a library (live 2497 21, 2508).
    fn folder_glyph(&self, f: &Path) -> &'static [u8] {
        if self.library_of(f).is_some() { os::GLYPH_FOLDER_LIBRARY } else { os::GLYPH_FOLDER }
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
        let mut location = Location::new(f.parent().map(Path::to_path_buf).unwrap_or_else(crate::file_dialogs::documents_dir)).with_new_folder();
        location.exclude = Some(f.clone());
        let location = Rc::new(location);
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (loc, view, f) = (location.clone(), view.clone(), f.clone());
            let here = already_in(&f, &loc.chosen.borrow());
            os::modal_doc_page(dialog, "Move documents and folders to", 460.)
                .child(os::modal_body().child(location.render(cx)))
                .footer(move_footer(&loc, here, cx))
                .on_ok(move |_, window, cx| {
                    if loc.open_picked(cx) {
                        window.refresh();
                        return false;
                    }
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

    /// Onshape's labels box (live documents-details-panel): the 30 px
    /// "Search labels" bar, the labels as checkbox rows for `path` (filtered
    /// by the search), and "Create new label".
    /// `labels` is passed in: the Details panel draws this while the view is
    /// being rendered, so it can't read the view.
    fn label_checklist(view: &Entity<StartView>, paths: &[PathBuf], search: &Entity<InputState>, labels: &lib::Labels, cx: &App) -> AnyElement {
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
            // Several documents: ticked when all have it; a click gives it to all, or takes it from all.
            let on = !paths.is_empty() && paths.iter().all(|p| labels.has(p, l));
            let (v, ps, l2) = (view.clone(), paths.to_vec(), l.clone());
            col = col.child(os::user_checkbox(("label-check", k), l.clone(), on).on_click(move |_, window, cx| {
                v.update(cx, |t, cx| {
                    let flip: Vec<PathBuf> = ps.iter().filter(|p| t.labels.has(p, &l2) == on).cloned().collect();
                    for p in &flip {
                        t.toggle_label(p, &l2, cx);
                    }
                });
                window.refresh();
            }));
        }
        let (v, p2) = (view.clone(), paths.first().cloned().unwrap_or_default());
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
        let date = |f: &Entity<crate::date_time_field::DateTimeField>, tip: &'static str| div().id(tip).w(px(148.)).h(px(34.)).pl(px(4.)).border_b_1().border_color(rgb(0xeaeaea)).flex().items_center().tooltip(crate::ui::tooltip(tip)).child(f.clone());
        // Live: Search is off while a date is partly typed.
        let dates_ok = self.history_dates.iter().all(|d| !d.read(cx).is_partial());
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
            .child(h_flex().mt(px(14.)).items_start().child(label("Date modified")).child(v_flex().gap(px(14.)).child(date(&self.history_dates[0], "Start date")).child(date(&self.history_dates[1], "End date"))))
            .child(
                h_flex()
                    .justify_end()
                    .gap(px(4.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .pr(px(5.))
                    .child(os::secondary_button("vh-f-reset", "Reset", cx).on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                        t.history_filter_edit = HistoryFilter::default();
                        t.history_filter = HistoryFilter::default();
                        for d in t.history_dates.clone() {
                            d.update(cx, |d, cx| d.clear(cx));
                        }
                        cx.notify();
                    })))
                    .child(os::primary_button("vh-f-search", "Search").disabled(!dates_ok).on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                        if !dates_ok {
                            return;
                        }
                        t.history_filter_edit.from = t.history_dates[0].read(cx).millis();
                        t.history_filter_edit.to = t.history_dates[1].read(cx).millis();
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
        let many = self.many_selected();
        if many && !self.select_all && panel == DocPanel::Details {
            let body = self.render_selection_panel(cx);
            return Some(self.doc_panel_frame("Details", body, cx).into_any_element());
        }
        let entry = if self.select_all { None } else { self.selected_entry() };
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
                        .when(self.details_labels_open, |d| d.child(div().mt(px(14.)).rounded(px(2.)).border_1().border_color(rgb(0xcccccc)).child(Self::label_checklist(&cx.entity(), std::slice::from_ref(&e.path), &self.label_search, &self.labels, cx)))),
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
                            .child(os::glyph(self.selected.as_deref().map_or(os::GLYPH_FOLDER, |f| self.folder_glyph(f)), 20., pal.text))
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
        let footer = if panel == DocPanel::Properties && entry.is_some() { self.render_props_footer(cx) } else { None };
        Some(self.doc_panel_frame(title, body, cx).children(footer).into_any_element())
    }

    /// The right panel's frame: its title bar with ✕, the scrolling body.
    fn doc_panel_frame(&self, title: &'static str, body: AnyElement, cx: &mut Context<Self>) -> Div {
        let pal = p();
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
            redo: None,
        });
    }

    /// The Properties panel's ↶ (back to the saved values) and ↷ (the edits again) (live docs-props-edit-undo).
    fn props_undo(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pe) = self.props.as_mut() else { return };
        let now = (pe.name.read(cx).value().to_string(), pe.description.read(cx).value().to_string(), pe.label.read(cx).value().to_string());
        let to = if redo {
            let Some(r) = pe.redo.take() else { return };
            r
        } else {
            if now == pe.saved {
                return;
            }
            pe.redo = Some(now);
            pe.saved.clone()
        };
        for (st, v) in [(&pe.name, to.0), (&pe.description, to.1), (&pe.label, to.2)] {
            st.update(cx, |s, cx| s.set_value(v, window, cx));
        }
        cx.notify();
    }

    /// Live documents/113: ↶ ↷ (pale) and Apply, disabled until a field changes.
    fn render_props_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pe = self.props.as_ref()?;
        let now = (pe.name.read(cx).value().to_string(), pe.description.read(cx).value().to_string(), pe.label.read(cx).value().to_string());
        let changed = now != pe.saved && !now.0.trim().is_empty();
        let (changed_any, can_redo) = (now != pe.saved, pe.redo.is_some());
        Some(
            h_flex()
                .h(px(64.))
                .flex_none()
                .px(px(20.))
                .gap(px(3.))
                .justify_end()
                .child(div().id("props-undo-w").when(!changed_any, |d| d.opacity(0.25)).map(|d| crate::ui_script::mark(d, || "button:Properties › Undo".into())).child(crate::ui::icon_button("props-undo", "undo", 30., false, cx)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.props_undo(false, window, cx))))
                .child(div().id("props-redo-w").when(!can_redo, |d| d.opacity(0.25)).map(|d| crate::ui_script::mark(d, || "button:Properties › Redo".into())).child(crate::ui::icon_button("props-redo", "redo", 30., false, cx)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.props_undo(true, window, cx))))
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
        let location = Rc::new(Location::new(d.folder().to_path_buf()).with_new_folder());
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (loc, view, d) = (location.clone(), view.clone(), d.clone());
            let here = already_in(&d.path, &loc.chosen.borrow());
            os::modal_doc_page(dialog, "Move documents and folders to", 460.)
                .child(os::modal_body().child(location.render(cx)))
                .footer(move_footer(&loc, here, cx))
                .on_ok(move |_, window, cx| {
                    if loc.open_picked(cx) {
                        window.refresh();
                        return false;
                    }
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
            .item(item("Copy workspace…", &view, &d, |t, d, w, cx| t.copy_workspace(d, w, cx)))
            .separator()
            .item(item("Details", &view, &d, |t, d, _, cx| t.show_panel(d, DocPanel::Details, cx)))
            // The document itself, on its Versions and history panel (live: /documents/<id>/versionsAndHistory); the
            // strip's icon is the page's own panel.
            .item(item("Versions and history", &view, &d, |t, d, w, cx| t.open_history(d.path, w, cx)))
            // The Details panel with its Document labels open, no dialog (live2 2351 (4)).
            .item(item("Labels…", &view, &d, |t, d, _, cx| {
                t.details_labels_open = true;
                t.show_panel(d, DocPanel::Details, cx);
            }))
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
        let m = ev.modifiers();
        self.click_row(&d.path, &m);
        window.focus(&self.focus, cx);
        if ev.click_count() >= 2 && !d.trashed && !m.shift && !m.secondary() {
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
                            .child(div().text_size(px(21.)).font_weight(FontWeight::LIGHT).text_color(pal.text_muted).child("Noshape")),
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
                    .child(div().w(px(388.)).flex_none().flex().justify_end().items_center().gap(px(8.)).pr(px(10.)).child(self.render_bell(cx)).child(help_menu(cx)).child(user_menu(cx))),
            )
            .into_any_element()
    }

    fn render_bell(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        crate::notifications::bell_button(
            self.notifications_open,
            move |_, cx| {
                view.update(cx, |this, cx| {
                    this.notifications_open = !this.notifications_open;
                    cx.notify();
                })
            },
            cx,
        )
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
                    // Rows dragged onto it go to the trash (live: a light-blue fill with a blue bottom border).
                    .child(self.nav_item("nav-trash", os::GLYPH_TRASH, "Trash", Source::Trash, cx).drag_over::<DocsDrag>(|st, d, _, _| {
                        d.over.set(true);
                        st.bg(rgb(0xdef1fb)).border_b_2().border_color(rgb(0x1651b0))
                    }).on_drop(cx.listener(|this, drag: &DocsDrag, window, cx| this.drop_into_trash(drag, window, cx)))),
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
            Source::Folder(f) if canon(f) == docs_root().1 => "Owned by me".into(),
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
                let docs = &docs_root().1;
                let top = self.outside.iter().find(|o| f.starts_with(o));
                let mut v: Vec<PathBuf> = f.ancestors().take_while(|a| *a != docs.as_path() && top.is_none_or(|t| a.starts_with(t))).map(Path::to_path_buf).collect();
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
                // The top level's title is another element than the crumb a click turns into it (live: its tooltip shows).
                div()
                    .id(if in_folder { "crumb-root" } else { "place-title" })
                    .map(|d| {
                        let r = root.to_string();
                        crate::ui_script::mark(d, move || format!("crumb:{r}"))
                    })
                    // A crumb's name is its title (live docs-folder-crumb: the tooltip under the pointer).
                    .tooltip(crate::ui::user_tooltip(root.clone()))
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
                }).child(folder_label(&c)).tooltip(crate::ui::user_tooltip(folder_label(&c))).when(i < last, |d| {
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
                let root = &docs_root().1;
                let mut v: Vec<(PathBuf, Option<String>)> = self.dirs.iter().filter(|d| d.parent() == Some(root.as_path())).map(|d| (d.clone(), None)).collect();
                let extra: Vec<PathBuf> = self.outside.iter().filter(|f| !v.iter().any(|(d, _)| d == *f)).cloned().collect();
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
            FolderSort::Modified => tiles.sort_by_key(|(d, _)| std::cmp::Reverse(self.dir_modified.get(d).copied())),
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
        let sel = self.selection_set();
        for (i, (f, trash_id)) in tiles_list.into_iter().enumerate() {
            let view = cx.entity();
            let selected = sel.contains(f.as_os_str());
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
                    .child(os::glyph(self.folder_glyph(&f), 20., pal.text))
                    .child(self.inline_rename_field(&f, cx).unwrap_or_else(|| div().flex_1().min_w_0().truncate().child(crate::ui::user_text(name.clone())).into_any_element()))
                    .when(trash_id.is_none(), |t| {
                        let target = f.clone();
                        t.drag_threshold(DOCS_DRAG_PX).on_drag(self.drag_items(selected, &f, &name, true), start_docs_drag(cx.entity().downgrade()))
                            .drag_over::<DocsDrag>(|st, d, _, _| folder_drop_style(st, d))
                            .on_drop(cx.listener(move |this, drag: &DocsDrag, window, cx| this.drop_into_folder(drag, &target, window, cx)))
                    })
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        let m = ev.modifiers();
                        this.click_row(&f1, &m);
                        if ev.click_count() >= 2 && !this.trash.items.iter().any(|t| t.stored() == f1) && !m.shift && !m.secondary() {
                            this.set_source(Source::Folder(f1.clone()), cx);
                        }
                        cx.notify();
                    }))
                    // Right-click selects too (the menu's handler stops propagation).
                    .os_context_menu(move |menu, window, cx| {
                        if view.update(cx, |this, cx| {
                            let many = this.context_select(&f2);
                            cx.notify();
                            many
                        }) {
                            return Self::selection_menu(view.clone(), menu, cx);
                        }
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
        let sel = self.selection_set();
        for (i, (f, trash_id)) in tiles.iter().enumerate() {
            let selected = sel.contains(f.as_os_str());
            let name = trash_id.as_ref().and_then(|id| self.trash.get(id)).map(|t| t.name.clone()).unwrap_or_else(|| folder_label(f));
            let modified = self.dir_modified.get(f).copied().unwrap_or(0);
            let (f1, f2, f3, v) = (f.clone(), f.clone(), f.clone(), cx.entity());
            let tid = trash_id.clone();
            let menu_name = name.clone();
            let name_for_drag = name.clone();
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
                    .child(h_flex().w(px(350.)).flex_none().pl(px(16.)).gap(px(10.)).child(os::glyph(self.folder_glyph(f), 20., pal.text)).child(div().min_w_0().truncate().child(crate::ui::user_text(name))))
                    .child(div().w(px(200.)).flex_none().pl(px(16.)).child(lib::format_onshape_time(modified)))
                    .child(div().w(px(200.)).flex_none().pl(px(16.)).child("me"))
                    .when(trash_id.is_none(), |r| {
                        let target = f.clone();
                        r.drag_threshold(DOCS_DRAG_PX).on_drag(self.drag_items(selected, f, &name_for_drag, true), start_docs_drag(cx.entity().downgrade()))
                            .drag_over::<DocsDrag>(|st, d, _, _| folder_drop_style(st, d))
                            .on_drop(cx.listener(move |this, drag: &DocsDrag, window, cx| this.drop_into_folder(drag, &target, window, cx)))
                    })
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        let m = ev.modifiers();
                        this.click_row(&f1, &m);
                        if ev.click_count() >= 2 && this.trash.item_of(&f1).is_none() && !m.shift && !m.secondary() {
                            this.set_source(Source::Folder(f1.clone()), cx);
                        }
                        cx.notify();
                    }))
                    // Right-click selects too (the menu's handler stops propagation).
                    .os_context_menu(move |menu, window, cx| {
                        if v.update(cx, |this, cx| {
                            let many = this.context_select(&f2);
                            cx.notify();
                            many
                        }) {
                            return Self::selection_menu(v.clone(), menu, cx);
                        }
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
                    // A card opens its document on one click (live docs-recent-card-open).
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.selected = Some(d1.path.clone());
                        this.open_path(d1.path.clone(), window, cx);
                        cx.notify();
                    }))
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
                            m.active_checked().item(item("Name: A → Z", FolderSort::NameAsc)).item(item("Name: Z → A", FolderSort::NameDesc)).item(item("Last modified", FolderSort::Modified))
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
                        // A press on + Add leaves it to the button's toggle (live docs-columns-panel: it closes).
                        .on_mouse_down_out(cx.listener(|this, e: &MouseDownEvent, _, cx| {
                            if !this.cols_anchor.get().contains(&e.position) {
                                this.cols_open = false;
                                cx.notify();
                            }
                        }))
                        // Esc closes it and the page takes the keyboard back (live docs-columns-search: focus none).
                        .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                            if e.keystroke.key == "escape" {
                                this.cols_open = false;
                                window.focus(&this.focus, cx);
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
        // Only the rows on screen (and a margin) are built: 1000 documents built every row each frame (~260 ms).
        let on_screen = rows_on_screen(self.rows_clip.get());
        let mut skipped = 0usize;
        let rows: Vec<AnyElement> = if let Some(hits) = &hits {
            self.hit_rows(hits, &cols, cx)
        } else {
            let mut rows = Vec::new();
            let sel = self.selection_set();
            let alone = sel.len() == 1;
            for (k, d) in docs.iter().enumerate() {
                // (Off screen only the row clicked last is built, not every selected one.)
                let keep = (on_screen.0..on_screen.1).contains(&k) || self.selected.as_ref() == Some(&d.path) || self.renaming.as_ref().is_some_and(|r| r.0 == d.path);
                if !keep {
                    skipped += 1;
                    continue;
                }
                if skipped > 0 {
                    rows.push(div().h(px(DOC_ROW_H * skipped as f32)).flex_none().into_any_element());
                    skipped = 0;
                }
                rows.push({
                let selected = sel.contains(d.path.as_os_str());
                // The quick actions show on the one selected row only.
                let alone = selected && alone;
                let open = crate::doc_files::is_document_open(&d.path, cx);
                let v2 = cx.entity();
                let (d1, d2) = (d.clone(), d.clone());
                let p1 = d.path.clone();
                let cell = |w: f32| div().w(px(w)).flex_none().pl(px(16.)).truncate();
                h_flex()
                    // A row is its document (a row that slides up under the pointer is another one: its tooltip shows).
                    .id(SharedString::from(format!("row-{}", d.path.display())))
                    .map(|r| crate::ui_script::mark(r, || format!("doc_row:{}", d.name)))
                    .h(px(DOC_ROW_H))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .border_b_1()
                    .border_color(rgb(0xececec))
                    .when(selected, |r| r.bg(pal.accent_soft))
                    .when(!selected, |r| r.hover(|r| r.bg(pal.row_hover)))
                    // Dragged: faded in place (live docs-f2-onto-document-row).
                    .when(self.dragging.contains(&d.path), |r| r.opacity(0.55))
                    .when(d.trashed, |r| r.tooltip(crate::ui::tooltip("This document is in the trash and cannot be opened.")))
                    .child(
                        h_flex()
                            .w(px(350.))
                            .flex_none()
                            .pl(px(16.))
                            .pr(px(15.))
                            .gap(px(14.))
                            .child(self.thumbnail(d, 64.0, 36.0, false))
                            .child(self.inline_rename_field(&d.path, cx).unwrap_or_else(|| {
                                // A plain click on the name opens the document (live2 11-13); Ctrl / Shift select as on the row.
                                let d5 = d.clone();
                                div()
                                    .id("doc-name")
                                    .min_w_0()
                                    .truncate()
                                    .text_color(if d.broken { pal.warning } else { pal.text })
                                    .child(crate::ui::user_text(d.name.clone()))
                                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                                        let m = ev.modifiers();
                                        if d5.trashed || m.shift || m.secondary() {
                                            return;
                                        }
                                        cx.stop_propagation();
                                        this.click_row(&d5.path, &m);
                                        this.open_path(d5.path.clone(), window, cx);
                                        cx.notify();
                                    }))
                                    .into_any_element()
                            }))
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
                    .when(alone && !d.trashed, |r| {
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
                                // The Details panel with its Document labels open (live docs-row-icon-labels), as the row menu's Labels….
                                .child(icon("row-labels", os::GLYPH_LABEL, "Labels").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.details_labels_open = true;
                                    this.show_panel(d4.clone(), DocPanel::Details, cx);
                                }))),
                        )
                    })
                    .when(!d.trashed, |r| r.drag_threshold(DOCS_DRAG_PX).on_drag(self.drag_items(selected, &d.path, &d.name, false), start_docs_drag(cx.entity().downgrade())))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| this.on_doc_click(&d1, ev, window, cx)))
                    // Right-click selects the row too (live documents/25); the
                    // menu's own handler stops propagation, so select here.
                    .os_context_menu(move |menu, _, cx| {
                        if v2.update(cx, |this, cx| {
                            let many = this.context_select(&p1);
                            cx.notify();
                            many
                        }) {
                            return Self::selection_menu(v2.clone(), menu, cx);
                        }
                        let in_recent = v2.read(cx).source == Source::Recent;
                        Self::doc_menu(v2.clone(), d2.clone(), in_recent, menu)
                    })
                    .into_any_element()
                });
            }
            if skipped > 0 {
                rows.push(div().h(px(DOC_ROW_H * skipped as f32)).flex_none().into_any_element());
            }
            rows
        };
        let empty = rows.is_empty();
        // The rows' place and the visible list around them, read as they're painted: a scroll or a resize that shows
        // rows not built builds them (the list is in the cached body: it is told so).
        self.rows_built.set(on_screen);
        let probe = {
            let clip = self.rows_clip.clone();
            let body = self.body.as_ref().map(|b| b.entity_id());
            canvas(
                move |b, window, cx| {
                    let mask = window.content_mask().bounds;
                    let now = RowsClip { top: f32::from(b.top()), clip: (f32::from(mask.top()), f32::from(mask.bottom())) };
                    clip.set(Some(now));
                    let seen = rows_within(Some(now), 0.);
                    if seen.0 < on_screen.0 || seen.1 > on_screen.1 {
                        if let Some(body) = body {
                            cx.notify(body);
                        }
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .w_full()
            .h(px(0.))
        };
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
                            // A click on the list's empty space clears the selection (live feel/documents.json).
                            div()
                                .id("doc-list")
                                .flex_1()
                                .min_h_0()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if !std::mem::take(&mut this.row_hit) {
                                        this.deselect_all(cx);
                                    }
                                }))
                                .overflow_y_scrollbar()
                                .child(v_flex().child(probe).children(rows))
                                .into_any_element()
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
        self.drag_share.get_mut().take();
        let docs = self.visible();
        let header = self.render_header(cx);
        // Searching hides the folder area, except an advanced search for folders.
        let folders = if self.query.trim().is_empty() && self.adv_query.as_ref().is_none_or(|q| q.folders_only()) { self.render_folders(cx) } else { None };
        let strip = self.render_strip(cx);
        // The details panel is a cached view of its own: a scroll of the list leaves it (2,000 selected rows) as it was.
        let panel = self.panel.is_some().then(|| {
            let slice = self.panel_slice.get_or_insert_with(|| crate::cached_slice::Slice::new(&cx.entity(), |this, _, cx| this.render_doc_panel(cx).unwrap_or_else(|| div().into_any_element()), cx)).clone();
            slice.cached(StyleRefinement::default().w(px(340.)).h_full().flex_none())
        });
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
        if !cx.has_active_drag() && !self.dragging.is_empty() {
            self.dragging.clear();
            self.refresh_body(cx);
        }
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
            // Dragged rows: off any target until one under the pointer says so as it is drawn (the ghost's icon).
            .on_drag_move::<DocsDrag>(|e, _, cx| e.drag(cx).over.set(false))
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
            .on_action(cx.listener(|this, _: &SelectAllRows, _, cx| this.select_all_rows(cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_selection(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_selection(-1, cx)))
            .on_action(cx.listener(|this, _: &ToggleRow, _, cx| this.toggle_current_row(cx)))
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
            // The bell's panel under the top bar's right end (as in a document's window).
            .children(self.notifications_open.then(|| {
                let view = cx.entity();
                div().absolute().top(px(crate::ui::NAVBAR_H)).right(px(40.)).child(crate::notifications::panel(
                    move |_, cx| {
                        view.update(cx, |this, cx| {
                            this.notifications_open = false;
                            cx.notify();
                        })
                    },
                    cx,
                ))
            }))
            // Advanced search: under the search box, left-aligned with it (live documents/110).
            .when(self.adv_open, |d| {
                let b = self.adv_anchor.get();
                d.child(
                    deferred(anchored().position(point(b.left(), b.bottom() + px(1.))).child(div().on_mouse_down_out(cx.listener(|this, e: &MouseDownEvent, _, cx| {
                        // A press on the ▾ leaves it to the caret's toggle (live docs-adv-open: it closes).
                        let caret = this.adv_anchor.get();
                        let on_caret = caret.contains(&e.position) && e.position.x >= caret.right() - px(20.);
                        if !crate::os_menu::is_open(cx) && !on_caret {
                            this.adv_open = false;
                            cx.notify();
                        }
                    })).child(self.adv.clone())))
                    .with_priority(1),
                )
            })
            // Onshape right-click menus (`os_context_menu`).
            .children(cx.try_global::<StartToast>().and_then(|t| t.0.clone()).filter(|(w, _)| *w == window.window_handle().window_id()).map(|(_, t)| {
                // Onshape: top center, 8 px from the top of the page (over the top bar).
                let text = match &t.bold {
                    Some(b) => StyledText::new(format!("{b}{}", t.text)).with_highlights([(0..b.len(), HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })]).into_any_element(),
                    None => t.text.clone().into_any_element(),
                };
                let close = |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    cx.default_global::<StartToast>().0 = None;
                    window.refresh();
                };
                let said = format!("{}{}{}", t.bold.clone().unwrap_or_default(), t.text, if t.restore.is_empty() { "" } else { " Restore" });
                let bubble = if t.restore.is_empty() {
                    os::message_bubble("start-toast", t.kind, text, close, cx)
                } else {
                    let (view, ids) = (cx.entity(), t.restore.clone());
                    os::message_bubble_with_link("start-toast", t.kind, text, "Restore", move |_, window, cx| view.update(cx, |this, cx| this.restore_all(&ids, window, cx)), close, cx)
                };
                crate::ui_script::mark_message(div().absolute().top(px(8.)).left_0().right_0().flex().justify_center().child(bubble), || said)
            }))
    }
}

/// Dragged rows: (path, name, folder).
type DragItems = Rc<Vec<(PathBuf, String, bool)>>;

/// A documents-page message: kind, an optional bold lead (a document
/// name), the text, and the trash item its Restore link brings back.
/// Documents and folders dragged on the documents page: the selection when the grabbed row is part of it, else that
/// row; dropped on a folder (row, tile or breadcrumb) they move into it.
#[derive(Clone)]
struct DocsDrag {
    items: DragItems,
    /// Where the row was grabbed (the ghost sits 10 px left of and 8 px above the pointer).
    offset: Point<Pixels>,
    /// Over a target this frame (the ghost's move icon, else ⊘).
    over: Rc<std::cell::Cell<bool>>,
}

impl DocsDrag {
    fn new(items: DragItems) -> Self {
        Self { items, offset: Point::default(), over: Rc::default() }
    }
}

/// How far a row moves before it drags (live: 4 px or less is a click, 5 a drag).
const DOCS_DRAG_PX: f32 = 4.;

/// The drag ghost's icons (ag-grid's move arrows and not-allowed).
const GLYPH_DRAG_MOVE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M10 2.5v15M2.5 10h15M7.8 4.7 10 2.5l2.2 2.2M7.8 15.3l2.2 2.2 2.2-2.2M4.7 7.8 2.5 10l2.2 2.2M15.3 7.8l2.2 2.2-2.2 2.2"/></svg>"#;
const GLYPH_DRAG_NO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="10" cy="10" r="7"/><path d="m5.1 5.1 9.8 9.8"/></svg>"#;

impl Render for DocsDrag {
    /// Live (feel/documents.json `ghost`, docs-f2-over-crop): a light-blue pill rgb(178,221,246), 48 px high, its
    /// top-left 10 px left of and 8 px above the pointer: the move icon over a target (⊘ elsewhere), then the name in
    /// bold, or "N items".
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let label = match self.items.as_slice() {
            [(_, name, _)] => name.clone(),
            many => format!("{} items", many.len()),
        };
        div().pl(self.offset.x - px(10.)).pt(self.offset.y - px(8.)).child(
            h_flex()
                .h(px(48.))
                .px(px(18.))
                .gap(px(10.))
                .rounded_full()
                .bg(rgb(0xb2ddf6))
                .shadow_md()
                .text_size(px(14.))
                .font_weight(FontWeight::BOLD)
                .text_color(pal.text)
                .child(os::glyph(if self.over.get() { GLYPH_DRAG_MOVE } else { GLYPH_DRAG_NO }, 18., pal.text))
                .child(crate::ui::user_text(label)),
        )
    }
}

/// The drag start of documents-page rows: the ghost, and the rows drawn faded while they're dragged.
fn start_docs_drag(view: WeakEntity<StartView>) -> impl Fn(&DocsDrag, Point<Pixels>, &mut Window, &mut App) -> Entity<DocsDrag> {
    move |d, offset, _, cx| {
        let rows: Vec<PathBuf> = d.items.iter().map(|i| i.0.clone()).collect();
        let _ = view.update(cx, |v, cx| {
            v.dragging = rows;
            cx.notify();
        });
        let mut ghost = d.clone();
        ghost.offset = offset;
        cx.new(|_| ghost)
    }
}

/// A folder under dragged rows (live: a 2 px blue rgb(22,81,176) border on a light rgb(232,238,247) fill).
fn folder_drop_style(st: StyleRefinement, d: &DocsDrag) -> StyleRefinement {
    d.over.set(true);
    st.border_2().border_color(rgb(0x1651b0)).bg(rgb(0xe8eef7))
}

#[derive(Clone)]
struct Toast {
    kind: os::BubbleKind,
    bold: Option<String>,
    text: String,
    restore: Vec<String>,
    /// Fades after ~3 s (Onshape's restore toast) instead of 8 s.
    brief: bool,
}

/// The documents page's message bubble and its generation.
#[derive(Default)]
/// The message bubble and the documents window it shows in.
struct StartToast(Option<(WindowId, Toast)>, u64);

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
    /// The edits an Undo took back (Redo brings them back).
    redo: Option<(String, String, String)>,
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
                // Shown, not saved; turning dark says so (live docs-account-dark-mode).
                .item_keep_open(OsMenuItem::new("View in dark mode beta").on_click(|_, window, cx| {
                    let dark = !gpui_kit::hex_theme_active();
                    noshape_preferences_ui::prefs_general::show_theme(dark, cx);
                    if dark {
                        StartView::toast(window, cx, Toast { kind: os::BubbleKind::Info, bold: None, text: "You are now viewing in dark mode. You may persist this setting. Go to user preferences.".into(), restore: Vec::new(), brief: false });
                    }
                }))
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

/// A folder's display name ("Noshape documents" for the default folder).
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

/// A library type's words (live 2508, the web client's libraryName): its part of the toasts ("Updating your images
/// library..."), its submenu once set and its icon.
fn library_texts(key: &str) -> (&'static str, &'static str, &'static str) {
    match key {
        "frame_profiles" => (" frame profiles", "Frame profile library", "frame"),
        "sheet_metal_forms" => (" sheet metal forms", "Sheet metal form library", "form"),
        "tolerances" => (" default tolerances", "Default tolerance library", "tolerance"),
        "images" => (" images", "Image library", "image"),
        _ => (" standard content", "Standard content library", "standard-content"),
    }
}

/// Whether `path` is in folder `dir` already (Move here has nothing to do there).
fn already_in(path: &Path, dir: &Path) -> bool {
    path.parent().is_some_and(|p| canon(p) == canon(dir))
}

/// Move to…'s footer: Move here (Open while a folder row is picked or a new one named), off while the folder shown
/// is where the rows already are (live docs-move-dialog), and Cancel.
fn move_footer(loc: &Location, here: bool, cx: &App) -> Div {
    let label = loc.ok_label("Move here");
    os::modal_footer().child(os::modal_ok(os::primary_button("ok", label).disabled(here && label == "Move here"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx)))
}

/// The dragged rows a drop on folder `dir` moves: not the folder itself, nor the rows already in it.
fn rows_to_move(items: &[(PathBuf, String, bool)], dir: &Path) -> Vec<(PathBuf, String, bool)> {
    let dir = canon(dir);
    items.iter().filter(|(p, _, _)| canon(p) != dir && p.parent().is_none_or(|f| canon(f) != dir)).cloned().collect()
}

/// A row moved: its old path, its new one, whether it is a folder.
type Moved = (PathBuf, PathBuf, bool);

/// Rows moved into `dir` (Move to… and drops): (old, new, folder) of those moved, (name, why) of those refused (a
/// folder into itself, a name taken there).
fn move_rows(items: &[(PathBuf, String, bool)], dir: &Path) -> (Vec<Moved>, Vec<(String, String)>) {
    let (mut done, mut refused) = (Vec::new(), Vec::new());
    for (path, name, folder) in items {
        let from = canon(path);
        match if *folder { lib::move_folder(path, dir) } else { lib::move_doc(path, dir) } {
            Ok(dest) => done.push((from, canon(&dest), *folder)),
            Err(e) => refused.push((name.clone(), format!("{e:#}"))),
        }
    }
    (done, refused)
}

/// A row clicked on the documents page (folders and documents, `order` as on the page), `sel` selected before (the
/// last clicked first): the selection after it, the clicked row first, and Shift's anchor. Alone it selects the row;
/// Ctrl (Cmd on a Mac) toggles it; Shift adds the rows from the anchor (the last row clicked without Shift) to it
/// (live feel/documents.json `multiSelect`).
fn click_rows(sel: &[PathBuf], anchor: Option<&Path>, order: &[PathBuf], row: &Path, shift: bool, toggle: bool) -> (Vec<PathBuf>, Option<PathBuf>) {
    let at = |p: &Path| order.iter().position(|q| q == p);
    if shift && let Some(a) = anchor && let (Some(i), Some(j)) = (at(a), at(row)) {
        let range = &order[i.min(j)..=i.max(j)];
        let mut out = vec![row.to_path_buf()];
        out.extend(range.iter().chain(sel.iter()).filter(|q| q.as_path() != row).cloned());
        out.dedup_by(|a, b| a == b);
        let mut seen = std::collections::HashSet::new();
        out.retain(|q| seen.insert(q.clone()));
        return (out, Some(a.to_path_buf()));
    }
    if toggle {
        if sel.iter().any(|q| q == row) {
            return (sel.iter().filter(|q| q.as_path() != row).cloned().collect(), Some(row.to_path_buf()));
        }
        let mut out = vec![row.to_path_buf()];
        out.extend(sel.iter().cloned());
        return (out, Some(row.to_path_buf()));
    }
    (vec![row.to_path_buf()], Some(row.to_path_buf()))
}

/// The documents folder as given and canonical, read once (the page asks for it many times a frame; each read makes
/// sure the folder exists).
fn docs_root() -> &'static (PathBuf, PathBuf) {
    static ROOT: std::sync::OnceLock<(PathBuf, PathBuf)> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let d = crate::file_dialogs::documents_dir();
        let c = canon(&d);
        (d, c)
    })
}

fn folder_label(f: &Path) -> String {
    if f == docs_root().0 {
        "Noshape documents".to_string()
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
    let path = crate::file_dialogs::os_path(path);
    if reveal {
        cx.reveal_path(&path);
    } else {
        cx.open_with_system(&path);
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
    /// The header's Sort (↓≡): its rows by name either way or newest first (live documents/900).
    sort: Rc<std::cell::Cell<FolderSort>>,
    /// A folder row clicked once: selected, the dialog's button reads "Open" (live docs-new-doc-location).
    picked: Rc<RefCell<Option<PathBuf>>>,
    /// Copy workspace's New folder (live2 2403; New document's box has none): the name of the folder being made,
    /// while its inline row is open.
    new_folder: Option<Rc<RefCell<Option<Entity<InputState>>>>>,
}

/// The inline New folder row's name made into a folder in `chosen`, which then shows it (live2 2403: made at once,
/// kept if the dialog is cancelled).
fn make_inline_folder(chosen: &Rc<RefCell<PathBuf>>, editing: &Rc<RefCell<Option<Entity<InputState>>>>, picked: &Rc<RefCell<Option<PathBuf>>>, cx: &App) {
    let Some(st) = editing.borrow_mut().take() else { return };
    let typed = st.read(cx).value().trim().to_string();
    let name = lib::sanitize_name(if typed.is_empty() { "Untitled folder" } else { &typed });
    let dir = chosen.borrow().join(name);
    if std::fs::create_dir_all(&dir).is_ok() {
        *chosen.borrow_mut() = canon(&dir);
        *picked.borrow_mut() = None;
    }
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
    let root = &docs_root().1;
    crate::doc_files::watched_folders().into_iter().map(|f| canon(&f)).filter(|f| !f.starts_with(root) && f.is_dir()).collect()
}

impl Location {
    /// A browser starting at `chosen`.
    pub fn new(chosen: PathBuf) -> Self {
        Self { root: canon(&crate::file_dialogs::documents_dir()), others: outside_folders(), chosen: Rc::new(RefCell::new(canon(&chosen))), exclude: None, sort: Default::default(), picked: Default::default(), new_folder: None }
    }

    /// With the header's New folder button (Copy workspace…).
    pub fn with_new_folder(mut self) -> Self {
        self.new_folder = Some(Default::default());
        self
    }

    fn naming_folder(&self) -> bool {
        self.new_folder.as_ref().is_some_and(|n| n.borrow().is_some())
    }

    /// The dialog's primary button: "Open" while a folder row is selected, else `label`.
    pub fn ok_label(&self, label: &'static str) -> &'static str {
        if self.picked.borrow().is_some() || self.naming_folder() { "Open" } else { label }
    }

    /// The primary button while a folder row is selected (or a new one named): into that folder (true; the dialog
    /// stays).
    pub fn open_picked(&self, cx: &App) -> bool {
        if let Some(editing) = self.new_folder.as_ref().filter(|_| self.naming_folder()) {
            make_inline_folder(&self.chosen, editing, &self.picked, cx);
            return true;
        }
        let Some(f) = self.picked.borrow_mut().take() else { return false };
        *self.chosen.borrow_mut() = f;
        true
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
        match self.sort.get() {
            FolderSort::NameAsc => v.sort_by_cached_key(|f| folder_label(f).to_lowercase()),
            FolderSort::NameDesc => v.sort_by_cached_key(|f| std::cmp::Reverse(folder_label(f).to_lowercase())),
            FolderSort::Modified => v.sort_by_cached_key(|f| std::cmp::Reverse(std::fs::metadata(f).and_then(|m| m.modified()).ok())),
        }
        v
    }

    /// Onshape's "Document location" browser: a header with home, back and
    /// the current place, above the list of its folders (click a folder to
    /// go into it; the place shown is the destination).
    pub fn render(&self, cx: &App) -> AnyElement {
        let pal = p();
        let chosen = self.chosen.borrow().clone();
        let at_root = chosen == self.root;
        let children = self.children(&chosen);
        let mut list = v_flex().p(px(6.)).gap(px(2.));
        let naming = self.new_folder.as_ref().and_then(|n| n.borrow().clone());
        if children.is_empty() && naming.is_none() {
            list = list.items_center().pt(px(15.)).child(os::info_box("No additional folders"));
        }
        let picked = self.picked.borrow().clone();
        for (i, f) in children.iter().enumerate() {
            let (c, p, f2) = (self.chosen.clone(), self.picked.clone(), f.clone());
            // A click selects the folder (it takes the keyboard), a double-click goes into it (live docs-new-doc-location).
            list = list.child(
                os::row_states(h_flex().id(SharedString::from(format!("loc-{i}"))).map(|d| crate::ui_script::mark(d, || format!("list_row:{}", folder_label(f)))).focusable().h(px(30.)).px(px(8.)).gap(px(7.)).rounded(px(3.)).cursor_pointer().text_size(px(14.)), picked.as_ref() == Some(f))
                    .child(os::glyph(os::GLYPH_FOLDER, 18., pal.text))
                    .child(div().flex_1().min_w_0().truncate().child(folder_label(f)))
                    .on_click(move |e, window, _| {
                        if e.click_count() >= 2 {
                            *c.borrow_mut() = f2.clone();
                            *p.borrow_mut() = None;
                        } else {
                            *p.borrow_mut() = Some(f2.clone());
                        }
                        window.refresh();
                    }),
            );
        }
        // New folder's inline row: its name selected and focused; Enter makes it.
        if let (Some(st), Some(editing)) = (naming, self.new_folder.clone()) {
            let (c, p) = (self.chosen.clone(), self.picked.clone());
            list = list.child(
                h_flex()
                    .id("loc-new-folder")
                    .h(px(30.))
                    .px(px(8.))
                    .gap(px(7.))
                    .rounded(px(3.))
                    .bg(rgb(0xdeecf9))
                    .text_size(px(14.))
                    .capture_key_down(move |e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key == "enter" {
                            cx.stop_propagation();
                            make_inline_folder(&c, &editing, &p, cx);
                            window.refresh();
                        }
                    })
                    .child(os::glyph(os::GLYPH_FOLDER, 18., pal.text))
                    .child(div().flex_1().min_w_0().map(|d| crate::ui_script::mark(d, || "field:New folder name".into())).child(Input::new(&st).appearance(false).text_size(px(14.)))),
            );
        }
        // Home and Back clear a selected row (live: at the top, Back only does that).
        let go = |to: PathBuf| {
            let (c, p) = (self.chosen.clone(), self.picked.clone());
            move |_: &ClickEvent, window: &mut Window, _: &mut App| {
                *p.borrow_mut() = None;
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
                            .when(!at_root || picked.is_some(), |d| d.cursor_pointer().on_click(go(parent)))
                            .child(os::glyph(os::GLYPH_BACK, 18., pal.text)),
                    )
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(os::glyph(if at_root { os::GLYPH_OWNED } else { os::GLYPH_FOLDER }, 18., pal.text))
                            .child(self.shown()),
                    )
                    .child(div().flex_1())
                    .when_some(self.new_folder.clone(), |d, editing| {
                        d.child(
                            div()
                                .id("loc-new-folder-button")
                                .map(|d| crate::ui_script::mark(d, || "button:New folder".into()))
                                .w(px(28.))
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.))
                                .cursor_pointer()
                                .hover(|d| d.bg(rgb(0xeaeaea)))
                                .tooltip(crate::ui::tooltip("New folder"))
                                .child(crate::assets::icon("new-folder", 20., cx))
                                .on_click(move |_, window, cx| {
                                    let st = cx.new(|cx| InputState::new(window, cx).default_value("Untitled folder"));
                                    st.update(cx, |s, cx| {
                                        s.focus(window, cx);
                                        s.select_all(window, cx);
                                    });
                                    *editing.borrow_mut() = Some(st);
                                    window.refresh();
                                }),
                        )
                    })
                    .child({
                        let sort = self.sort.clone();
                        div()
                            .id("loc-sort")
                            .map(|d| crate::ui_script::mark(d, || "button:Sort".into()))
                            .w(px(28.))
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(0xeaeaea)))
                            .tooltip(crate::ui::tooltip("Sort"))
                            .child(os::glyph(os::GLYPH_SORT, 20., pal.text))
                            .os_dropdown_menu("loc-sort-dd", false, move |m, _, _| {
                                let item = |label: &'static str, k: FolderSort| {
                                    let sort = sort.clone();
                                    OsMenuItem::new(label).checked(sort.get() == k).on_click(move |_, window, _| {
                                        sort.set(k);
                                        window.refresh();
                                    })
                                };
                                m.active_checked().item(item("Name: A → Z", FolderSort::NameAsc)).item(item("Name: Z → A", FolderSort::NameDesc)).item(item("Last modified", FolderSort::Modified))
                            })
                    }),
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
            let ok = location.as_ref().map_or(ok_label, |l| l.1.ok_label(ok_label));
            serde_json::json!({ "kind": "modal", "title": title, "fields": fields, "buttons": [ok, "Cancel"] })
        });
        let ok = location.as_ref().map_or(ok_label, |l| l.1.ok_label(ok_label));
        let loc_ok = location.clone();
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
                    .when_some(location.clone(), |d, loc| d.child(div().pt(px(6.)).child(os::field_label(loc.0))).child(loc.1.render(cx))),
            )
            .footer(
                os::modal_footer()
                    .child(os::modal_ok(os::primary_button("ok", ok)))
                    .child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))),
            )
            .on_ok(move |_, window, cx| {
                // "Open": into the selected folder, the dialog stays.
                if loc_ok.as_ref().is_some_and(|l| l.1.open_picked(cx)) {
                    window.refresh();
                    return false;
                }
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
#[path = "start_bench.rs"]
mod bench;

#[cfg(test)]
mod tests {
    use super::{SelectAllRows, SelectNext, SelectPrev, ToggleRow, click_rows, history_change_matches, key_bindings, lib, move_rows, rows_to_move};
    use std::path::PathBuf;

    /// Live chrome.json (07:33): no Onshape shortcut works on the documents page, only its list's keys.
    #[test]
    fn the_documents_page_has_no_onshape_shortcuts() {
        use gpui_kit::{Action, KeyContext, Keymap, Keystroke};
        let mut all = key_bindings();
        all.extend(noshape_app_core::actions::default_key_bindings());
        let keymap = Keymap::new(all);
        let page = [KeyContext::parse("StartWindow").unwrap()];
        let action = |key: &str| keymap.bindings_for_input(&[Keystroke::parse(key).unwrap()], &page).0.first().map(|b| b.action().name());
        for key in ["shift-?", "alt-c", "ctrl-space", "alt-t", "secondary-z", "secondary-y", "delete", "f2", "/", "s", "f", "enter", "secondary-f", "left", "right"] {
            assert_eq!(action(key), None, "{key}");
        }
        // Live2 11-13: ↑ / ↓ move the selection, Space toggles the row.
        assert_eq!(action("down"), Some(SelectNext.name()));
        assert_eq!(action("up"), Some(SelectPrev.name()));
        assert_eq!(action("space"), Some(ToggleRow.name()));
        // The list's own keys (live2 feel/documents.json: Ctrl+A selects every row).
        assert_eq!(action("secondary-a"), Some(SelectAllRows.name()));
    }

    /// Lead 11-13 (interaction feel): rows dragged onto a folder move into it, documents and folders with what's in
    /// them; a drop on the folder itself or where they already are moves nothing; a folder can't go into itself.
    #[test]
    fn rows_dropped_on_a_folder_move_into_it() {
        let root = std::env::temp_dir().join(format!("noshape-docs-drop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (f, g) = (root.join("F"), root.join("G"));
        std::fs::create_dir_all(&f).unwrap();
        std::fs::create_dir_all(&g).unwrap();
        for p in [root.join("a.noshape"), root.join("b.noshape"), g.join("c.noshape")] {
            std::fs::write(&p, b"doc").unwrap();
        }
        let row = |p: PathBuf, folder: bool| {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            (p, name, folder)
        };
        let rows = vec![row(root.join("a.noshape"), false), row(root.join("b.noshape"), false), row(g.clone(), true), row(f.clone(), true)];
        // F itself drops out.
        let take = rows_to_move(&rows, &f);
        assert_eq!(take.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), ["a", "b", "G"]);
        let (done, refused) = move_rows(&take, &f);
        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(done.len(), 3);
        assert!(f.join("a.noshape").is_file() && f.join("b.noshape").is_file() && f.join("G").join("c.noshape").is_file(), "moved, G with its document");
        assert!(!root.join("a.noshape").exists() && !g.exists());
        // Dropped again where they are: nothing to move.
        let again = vec![row(f.join("a.noshape"), false), row(f.join("G"), true)];
        assert!(rows_to_move(&again, &f).is_empty());
        // F into its own G: refused, F stays.
        let (done, refused) = move_rows(&[row(f.clone(), true)], &f.join("G"));
        assert!(done.is_empty() && refused.len() == 1 && f.is_dir(), "{refused:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Lead 11-13 (interaction feel; live2 feel/documents.json): a click selects one row, Ctrl toggles one, Shift adds
    /// the rows from the anchor (the last click without Shift) to the row, folders and documents alike in page order.
    #[test]
    fn rows_select_alone_with_ctrl_toggles_and_with_shift_ranges_from_the_anchor() {
        let order: Vec<PathBuf> = ["f1", "f2", "a", "b", "c", "d", "e"].iter().map(PathBuf::from).collect();
        let p = |s: &str| PathBuf::from(s);
        let names = |v: &[PathBuf]| v.iter().map(|q| q.display().to_string()).collect::<Vec<_>>().join(" ");
        let (sel, anchor) = click_rows(&[], None, &order, &p("b"), false, false);
        assert_eq!((names(&sel), anchor.clone()), ("b".into(), Some(p("b"))));
        // Shift: b..e, e first (the row clicked last), the anchor kept.
        let (sel, anchor) = click_rows(&sel, anchor.as_deref(), &order, &p("e"), true, false);
        assert_eq!((names(&sel), anchor.clone()), ("e b c d".into(), Some(p("b"))));
        // Ctrl adds a folder, then takes c out; the anchor follows Ctrl's row.
        let (sel, anchor) = click_rows(&sel, anchor.as_deref(), &order, &p("f1"), false, true);
        assert_eq!(names(&sel), "f1 e b c d");
        let (sel, anchor) = click_rows(&sel, anchor.as_deref(), &order, &p("c"), false, true);
        assert_eq!((names(&sel), anchor.clone()), ("f1 e b d".into(), Some(p("c"))));
        // Shift from the new anchor upward adds the range to the selection.
        let (sel, anchor) = click_rows(&sel, anchor.as_deref(), &order, &p("f2"), true, false);
        assert_eq!((names(&sel), anchor.clone()), ("f2 a b c f1 e d".into(), Some(p("c"))));
        // A plain click: that row alone.
        let (sel, _) = click_rows(&sel, anchor.as_deref(), &order, &p("d"), false, false);
        assert_eq!(names(&sel), "d");
        // Shift with no anchor (nothing clicked yet): the row alone.
        assert_eq!(names(&click_rows(&[], None, &order, &p("c"), true, false).0), "c");
        // live2's three sequences (rows 3, 6, 5, 4, 2 in that order).
        let order: Vec<PathBuf> = ["3", "6", "5", "4", "2"].iter().map(PathBuf::from).collect();
        let run = |clicks: &[(&str, bool, bool)]| {
            let (mut sel, mut anchor) = (Vec::new(), None);
            for (r, shift, ctrl) in clicks {
                (sel, anchor) = click_rows(&sel, anchor.as_deref(), &order, &p(r), *shift, *ctrl);
            }
            let mut v: Vec<String> = sel.iter().map(|q| q.display().to_string()).collect();
            v.sort();
            v.join(" ")
        };
        assert_eq!(run(&[("6", false, false), ("4", false, true), ("3", true, false)]), "3 4 5 6");
        assert_eq!(run(&[("3", false, false), ("5", false, true), ("6", true, false)]), "3 5 6");
        assert_eq!(run(&[("6", false, false), ("4", true, false), ("2", true, false)]), "2 4 5 6");
    }
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
