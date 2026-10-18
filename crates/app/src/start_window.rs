//! The documents start window: shown when noshape starts without a file and
//! whenever the last document window closes. Lists recent documents and the
//! `.noshape` files of the documents folder and user-added folders, as a
//! thumbnail grid or a sortable list, with search, create, open and per
//! document commands (rename, duplicate, reveal, trash, forget).

use crate::actions::{CloseWindow, NewDocument, OpenDocument};
use crate::start_library::{self as lib, DocEntry, Library, ScanInput};
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, IconName, Sizable as _, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

actions!(noshape_start, [OpenSelected, RenameSelected, TrashSelected, FocusSearch, SelectNext, SelectPrev]);

const CONTEXT: &str = "StartWindow";
const CARD_W: f32 = 224.0;
const THUMB_H: f32 = 168.0;

// Lucide-style stroke icons that are not in the kit's default icon set.
const ICON_CLOCK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>"#;
const ICON_LIBRARY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m16 6 4 14"/><path d="M12 6v14"/><path d="M8 8v12"/><path d="M4 4v16"/></svg>"#;
const ICON_GRID: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="7" height="7" x="3" y="3" rx="1"/><rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/></svg>"#;
const ICON_LIST: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h.01"/><path d="M3 18h.01"/><path d="M3 6h.01"/><path d="M8 12h13"/><path d="M8 18h13"/><path d="M8 6h13"/></svg>"#;
const ICON_FOLDER_PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 10v6"/><path d="M9 13h6"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;
const ICON_PENCIL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/></svg>"#;
const ICON_BOX: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/></svg>"#;
const ICON_X_CIRCLE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/></svg>"#;

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
    match gpui_kit::open_window(options, cx, |window, cx| {
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
}

/// View choices remembered across sessions.
#[derive(Clone, Serialize, Deserialize)]
struct Prefs {
    view: ViewMode,
    sort: SortKey,
    descending: bool,
    source: Option<Source>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { view: ViewMode::Grid, sort: SortKey::Modified, descending: true, source: None }
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
        let subs = vec![
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
            scan_task: None,
            _poll: poll,
            _subs: subs,
        };
        this.rescan(cx);
        this
    }

    fn save_prefs(&self) {
        let prefs = Prefs { view: self.view, sort: self.sort, descending: self.descending, source: Some(self.source.clone()) };
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
                t.folders = input.folders;
                if let Source::Folder(f) = &t.source
                    && !t.folders.contains(f)
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
        match source {
            Source::Recent => d.recent_rank.is_some(),
            Source::All => true,
            Source::Folder(f) => {
                let f = f.canonicalize().unwrap_or_else(|_| f.clone());
                d.path.starts_with(&f)
            }
        }
    }

    /// Documents shown: current source, filtered by the search, sorted.
    fn visible(&self) -> Vec<DocEntry> {
        let q = self.query.trim().to_lowercase();
        let mut v: Vec<DocEntry> = self
            .docs
            .iter()
            .filter(|d| self.in_source(d, &self.source))
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

    fn count(&self, source: &Source) -> usize {
        self.docs.iter().filter(|d| self.in_source(d, source)).count()
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

    fn notify(window: &mut Window, cx: &mut App, n: Notification) {
        window.push_notification(n, cx);
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        match crate::files::open_document_window(path.clone(), None, gpu, cx) {
            Ok(()) => self.force_rescan(cx),
            Err(e) => Self::notify(window, cx, Notification::error(format!("{e:#}")).title("Could not open document")),
        }
    }

    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = match &self.source {
            Source::Folder(f) => f.clone(),
            _ => crate::files::documents_dir(),
        };
        let default = lib::next_untitled(&dir);
        let hint = format!("Saved in {}", display_path(&dir));
        let view = cx.entity();
        prompt_name("New document", "Create", default, hint, window, cx, move |name, window, cx| {
            let clean = lib::sanitize_name(&name);
            if clean.is_empty() {
                Self::notify(window, cx, Notification::warning("Enter a name for the document"));
                return false;
            }
            let path = dir.join(format!("{clean}.{}", lib::EXT));
            if path.exists() {
                Self::notify(window, cx, Notification::warning(format!("\"{clean}\" already exists in this folder")));
                return false;
            }
            view.update(cx, |this, cx| {
                this.selected = Some(path.clone());
                this.open_path(path, window, cx);
            });
            true
        });
    }

    fn open_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        crate::files::request_open_path("Open noshape document", "noshape document", &["noshape"], window, cx, move |path, cx| {
            let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
            if let Err(e) = crate::files::open_document_window(path, None, gpu, cx) {
                log::error!("open failed: {e:#}");
            }
            view.update(cx, |this, cx| this.force_rescan(cx));
        });
    }

    fn add_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        crate::files::request_folder_path("Add a folder to noshape", window, cx, move |path, cx| {
            if !path.is_dir() {
                return;
            }
            let path = path.canonicalize().unwrap_or(path);
            let mut list = crate::files::watched_folders();
            if !list.contains(&path) && path != crate::files::documents_dir() {
                list.push(path.clone());
                crate::files::set_watched_folders(&list);
            }
            view.update(cx, |this, cx| {
                this.folders = ScanInput::current().folders;
                this.set_source(Source::Folder(path), cx);
                this.force_rescan(cx);
            });
        });
    }

    fn remove_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        let mut list = crate::files::watched_folders();
        list.retain(|f| *f != folder);
        crate::files::set_watched_folders(&list);
        if self.source == Source::Folder(folder) {
            self.source = Source::All;
            self.save_prefs();
        }
        self.folders = ScanInput::current().folders;
        self.force_rescan(cx);
    }

    fn rename(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if crate::files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, Notification::warning("Close the document's window before renaming it").title(d.name.clone()));
            return;
        }
        let view = cx.entity();
        let hint = format!("Renames {}", d.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
        prompt_name("Rename document", "Rename", d.name.clone(), hint, window, cx, move |name, window, cx| match lib::rename_document(&d.path, &name) {
            Ok(dest) => {
                crate::files::replace_recent(&d.path, &dest);
                view.update(cx, |this, cx| {
                    this.selected = Some(dest.canonicalize().unwrap_or(dest));
                    this.force_rescan(cx);
                });
                true
            }
            Err(e) => {
                Self::notify(window, cx, Notification::error(format!("{e:#}")).title("Rename failed"));
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
                Err(e) => Self::notify(window, cx, Notification::error(format!("{e:#}")).title("Duplicate failed")),
            });
        })
        .detach();
    }

    fn reveal(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = crate::files::reveal_in_file_manager(path) {
            Self::notify(window, cx, Notification::error(format!("{e:#}")).title("Could not open the file manager"));
        }
    }

    fn trash(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if crate::files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, Notification::warning("Close the document's window before moving it to the trash").title(d.name.clone()));
            return;
        }
        match lib::trash_document(&d.path) {
            Ok(()) => {
                crate::files::remove_recent(&d.path);
                if self.selected.as_ref() == Some(&d.path) {
                    self.selected = None;
                }
                Self::notify(window, cx, Notification::success(format!("Moved \"{}\" to the trash", d.name)));
                self.force_rescan(cx);
            }
            Err(e) => Self::notify(window, cx, Notification::error(format!("{e:#}")).title("Could not move to trash")),
        }
    }

    fn forget(&mut self, d: DocEntry, cx: &mut Context<Self>) {
        crate::files::remove_recent(&d.path);
        self.force_rescan(cx);
    }

    // ------------------------------------------------------------- render

    fn doc_menu(view: Entity<StartView>, d: DocEntry, menu: PopupMenu) -> PopupMenu {
        let item = |label: &'static str, ic: Icon, view: &Entity<StartView>, d: &DocEntry, f: fn(&mut StartView, DocEntry, &mut Window, &mut Context<StartView>)| {
            let (view, d) = (view.clone(), d.clone());
            PopupMenuItem::new(label).icon(ic).on_click(move |_, window, cx| {
                let d = d.clone();
                view.update(cx, |this, cx| f(this, d, window, cx));
            })
        };
        let mut menu = menu
            .item(item("Open", Icon::new(IconName::FolderOpen), &view, &d, |t, d, w, cx| t.open_path(d.path, w, cx)))
            .item(item("Rename…", icon(ICON_PENCIL), &view, &d, |t, d, w, cx| t.rename(d, w, cx)))
            .item(item("Duplicate", Icon::new(IconName::Copy), &view, &d, |t, d, w, cx| t.duplicate(d, w, cx)))
            .item(item("Show in file manager", Icon::new(IconName::ExternalLink), &view, &d, |t, d, w, cx| t.reveal(&d.path, w, cx)))
            .separator();
        if d.recent_rank.is_some() {
            menu = menu.item(item("Remove from recent", icon(ICON_X_CIRCLE), &view, &d, |t, d, _, cx| t.forget(d, cx)));
        }
        menu.item(item("Move to trash", Icon::new(IconName::Delete), &view, &d, |t, d, w, cx| t.trash(d, w, cx)))
    }

    fn select(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.selected = Some(path.to_path_buf());
        cx.notify();
    }

    fn on_doc_click(&mut self, d: &DocEntry, ev: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(d.path.clone());
        window.focus(&self.focus, cx);
        if ev.click_count() >= 2 {
            self.open_path(d.path.clone(), window, cx);
        }
        cx.notify();
    }

    fn render_title_bar(&self) -> AnyElement {
        let pal = p();
        TitleBar::new()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().size(px(14.)).rounded_sm().bg(pal.accent))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(pal.text).child("noshape"))
                    .child(div().w(px(1.)).h(px(16.)).bg(pal.border))
                    .child(div().text_sm().text_color(pal.text_muted).child("Documents")),
            )
            .into_any_element()
    }

    fn sidebar_item(&self, id: &str, ic: Icon, label: String, count: Option<usize>, source: Source, tooltip: Option<String>, cx: &mut Context<Self>) -> Stateful<Div> {
        let pal = p();
        let active = self.source == source;
        h_flex()
            .id(SharedString::from(format!("src-{id}")))
            .h(px(30.))
            .px_2()
            .gap_2()
            .rounded_md()
            .cursor_pointer()
            .text_sm()
            .when(active, |d| d.bg(pal.accent_soft).text_color(pal.text))
            .when(!active, |d| d.text_color(pal.text_muted).hover(|d| d.bg(pal.panel_hover).text_color(pal.text)))
            .child(ic.size(px(15.)).text_color(if active { pal.accent_hover } else { pal.text_faint }))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .when_some(count, |d, n| d.child(div().text_xs().text_color(if active { pal.text_muted } else { pal.text_faint }).child(n.to_string())))
            .when_some(tooltip, |d, t| d.tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(t.clone()).build(window, cx)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_source(source.clone(), cx)))
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let docs_dir = crate::files::documents_dir();
        let section = |label: &'static str| div().px_2().pt_4().pb_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_faint).child(label);
        let mut folders = v_flex().gap_0p5();
        for (i, f) in self.folders.clone().into_iter().enumerate() {
            let is_default = f == docs_dir;
            let label = if is_default { "noshape documents".to_string() } else { f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| f.display().to_string()) };
            let count = self.count(&Source::Folder(f.clone()));
            let view = cx.entity();
            let folder = f.clone();
            let ic = if is_default { Icon::new(IconName::HardDrive) } else { Icon::new(IconName::Folder) };
            folders = folders.child(
                self.sidebar_item(&format!("folder-{i}"), ic, label, Some(count), Source::Folder(f.clone()), Some(display_path(&f)), cx).context_menu(move |menu, _, _| {
                    let (v1, v2) = (view.clone(), view.clone());
                    let (f1, f2) = (folder.clone(), folder.clone());
                    let menu = menu.item(PopupMenuItem::new("Show in file manager").icon(IconName::ExternalLink).on_click(move |_, window, cx| v1.update(cx, |t, cx| t.reveal(&f1, window, cx))));
                    if is_default {
                        menu
                    } else {
                        menu.separator().item(PopupMenuItem::new("Remove from list").icon(icon(ICON_X_CIRCLE)).on_click(move |_, _, cx| v2.update(cx, |t, cx| t.remove_folder(f2.clone(), cx))))
                    }
                }),
            );
        }
        v_flex()
            .w(px(236.))
            .h_full()
            .flex_none()
            .bg(pal.panel)
            .border_r_1()
            .border_color(pal.border)
            .px_3()
            .py_3()
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        accent_button("new-doc", "New document", true)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.new_document(window, cx))),
                    )
                    .child(
                        Button::new("open-doc")
                            .outline()
                            .icon(IconName::FolderOpen)
                            .label("Open…")
                            .w_full()
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_dialog(window, cx))),
                    ),
            )
            .child(section("LIBRARY"))
            .child(
                v_flex()
                    .gap_0p5()
                    .child(self.sidebar_item("recent", icon(ICON_CLOCK), "Recent".into(), Some(self.count(&Source::Recent)), Source::Recent, None, cx))
                    .child(self.sidebar_item("all", icon(ICON_LIBRARY), "All documents".into(), Some(self.docs.len()), Source::All, None, cx)),
            )
            .child(
                h_flex().justify_between().child(section("FOLDERS")).child(
                    div().pt_3().child(
                        Button::new("add-folder")
                            .icon(icon(ICON_FOLDER_PLUS))
                            .ghost()
                            .xsmall()
                            .tooltip("Add a folder…")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.add_folder(window, cx))),
                    ),
                ),
            )
            .child(div().id("folders").flex_1().min_h_0().overflow_y_scrollbar().child(folders))
            .child(
                h_flex()
                    .pt_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(pal.border)
                    .text_xs()
                    .text_color(pal.text_faint)
                    .child(Icon::new(IconName::HardDrive).size(px(13.)))
                    .child("Stored on this computer"),
            )
            .into_any_element()
    }

    fn source_title(&self) -> String {
        match &self.source {
            Source::Recent => "Recent".into(),
            Source::All => "All documents".into(),
            Source::Folder(f) if *f == crate::files::documents_dir() => "noshape documents".into(),
            Source::Folder(f) => f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| f.display().to_string()),
        }
    }

    fn render_header(&mut self, shown: usize, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let sort_label = match self.sort {
            SortKey::Modified => "Last modified",
            SortKey::Name => "Name",
        };
        let view = cx.entity();
        let (sort, desc) = (self.sort, self.descending);
        let subtitle: Option<String> = match &self.source {
            Source::Folder(f) => Some(display_path(f)),
            Source::Recent => Some("Documents you opened recently".into()),
            Source::All => Some("Recent documents and every document in your folders".into()),
        };
        h_flex()
            .px_6()
            .pt_5()
            .pb_4()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(self.source_title()))
                            .child(div().px_1p5().rounded_md().bg(pal.panel_hover).text_xs().text_color(pal.text_muted).child(shown.to_string())),
                    )
                    .when_some(subtitle, |d, s| d.child(div().text_xs().text_color(pal.text_faint).truncate().child(s))),
            )
            .child(
                div().w(px(260.)).child(
                    Input::new(&self.search)
                        .small()
                        .prefix(Icon::new(IconName::Search).size(px(14.)).text_color(pal.text_faint))
                        .cleanable(true),
                ),
            )
            .child(
                Button::new("sort")
                    .ghost()
                    .small()
                    .icon(if self.descending { IconName::SortDescending } else { IconName::SortAscending })
                    .label(sort_label)
                    .dropdown_caret(true)
                    .dropdown_menu(move |menu, _, _| {
                        let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                        menu.label("Sort by")
                            .item(PopupMenuItem::new("Last modified").checked(sort == SortKey::Modified).on_click(move |_, _, cx| {
                                v1.update(cx, |t, cx| {
                                    if t.sort != SortKey::Modified {
                                        t.set_sort(SortKey::Modified, cx)
                                    }
                                })
                            }))
                            .item(PopupMenuItem::new("Name").checked(sort == SortKey::Name).on_click(move |_, _, cx| {
                                v2.update(cx, |t, cx| {
                                    if t.sort != SortKey::Name {
                                        t.set_sort(SortKey::Name, cx)
                                    }
                                })
                            }))
                            .separator()
                            .item(PopupMenuItem::new(if desc { "Reverse order (ascending)" } else { "Reverse order (descending)" }).on_click(move |_, _, cx| {
                                v3.update(cx, |t, cx| {
                                    t.descending = !t.descending;
                                    t.save_prefs();
                                    cx.notify();
                                })
                            }))
                    }),
            )
            .child(
                h_flex()
                    .p_0p5()
                    .gap_0p5()
                    .rounded_md()
                    .bg(pal.panel)
                    .border_1()
                    .border_color(pal.border)
                    .child(
                        seg_button("view-grid", icon(ICON_GRID), "Grid view", self.view == ViewMode::Grid)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_view(ViewMode::Grid, cx))),
                    )
                    .child(
                        seg_button("view-list", icon(ICON_LIST), "List view", self.view == ViewMode::List)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_view(ViewMode::List, cx))),
                    ),
            )
            .into_any_element()
    }

    fn thumbnail(&self, d: &DocEntry, w: f32, h: f32, big: bool) -> AnyElement {
        let pal = p();
        let base = div().w(px(w)).h(px(h)).flex_none().overflow_hidden().bg(gpui_kit::linear_gradient(
            180.,
            gpui_kit::linear_color_stop(rgb(0x2a2f3a), 0.),
            gpui_kit::linear_color_stop(rgb(0x181b22), 1.),
        ));
        match &d.thumbnail {
            Some(img) => base.child(gpui_kit::img(img.clone()).size_full()).into_any_element(),
            None => {
                let (ic, color) = if d.broken { (Icon::new(IconName::TriangleAlert), pal.warning) } else { (icon(ICON_BOX), pal.text_faint) };
                base.flex()
                    .items_center()
                    .justify_center()
                    .child(ic.size(px(if big { 40. } else { 16. })).text_color(with_alpha(color, 0.7)))
                    .into_any_element()
            }
        }
    }

    fn open_badge() -> AnyElement {
        let pal = p();
        h_flex()
            .gap_1()
            .px_1p5()
            .py_0p5()
            .rounded_md()
            .bg(with_alpha(pal.app_bg, 0.8))
            .text_xs()
            .text_color(pal.success)
            .child(div().size(px(6.)).rounded_full().bg(pal.success))
            .child("Open")
            .into_any_element()
    }

    fn render_card(&self, ix: usize, d: &DocEntry, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let selected = self.selected.as_ref() == Some(&d.path);
        let open = crate::files::is_document_open(&d.path, cx);
        let view = cx.entity();
        let (d1, d2, d3) = (d.clone(), d.clone(), d.clone());
                let p1 = d.path.clone();
        let v2 = view.clone();
        let subtitle = if d.broken { "Cannot read this file".to_string() } else { format!("Edited {}", lib::format_modified(d.modified)) };
        v_flex()
            .id(SharedString::from(format!("card-{ix}")))
            .w(px(CARD_W))
            .rounded_lg()
            .overflow_hidden()
            .border_1()
            .bg(pal.panel)
            .cursor_pointer()
            .when(selected, |c| c.border_color(pal.accent).shadow_md())
            .when(!selected, |c| c.border_color(pal.border).hover(|c| c.border_color(pal.border_strong).bg(pal.panel_raised)))
            .child(
                div()
                    .relative()
                    .child(self.thumbnail(d, CARD_W - 2.0, THUMB_H, true))
                    .when(open, |c| c.child(div().absolute().top_2().left_2().child(Self::open_badge()))),
            )
            .child(
                h_flex()
                    .pl_3()
                    .pr_1()
                    .py_2()
                    .gap_1()
                    .border_t_1()
                    .border_color(pal.border)
                    .when(selected, |c| c.bg(pal.accent_soft))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(pal.text).truncate().child(d.name.clone()))
                            .child(div().text_xs().text_color(if d.broken { pal.warning } else { pal.text_faint }).truncate().child(subtitle)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("more-{ix}")))
                            .icon(IconName::EllipsisVertical)
                            .ghost()
                            .xsmall()
                            .dropdown_menu(move |menu, _, _| Self::doc_menu(view.clone(), d3.clone(), menu)),
                    ),
            )
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| this.on_doc_click(&d1, ev, window, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.select(&p1, cx)))
            .context_menu(move |menu, _, _| Self::doc_menu(v2.clone(), d2.clone(), menu))
            .into_any_element()
    }

    fn render_grid(&self, docs: &[DocEntry], cx: &mut Context<Self>) -> AnyElement {
        let cards: Vec<AnyElement> = docs.iter().enumerate().map(|(i, d)| self.render_card(i, d, cx)).collect();
        div()
            .id("doc-grid")
            .size_full()
            .overflow_y_scrollbar()
            .child(div().px_6().pb_6().flex().flex_wrap().gap_4().children(cards))
            .into_any_element()
    }

    fn list_header_cell(&self, id: &'static str, label: &'static str, key: Option<SortKey>, cx: &mut Context<Self>) -> Stateful<Div> {
        let pal = p();
        let active = key == Some(self.sort);
        h_flex()
            .id(id)
            .gap_1()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if active { pal.text_muted } else { pal.text_faint })
            .child(label)
            .when(active, |d| d.child(Icon::new(if self.descending { IconName::ChevronDown } else { IconName::ChevronUp }).size(px(12.))))
            .when_some(key, |d, k| d.cursor_pointer().hover(|d| d.text_color(pal.text)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_sort(k, cx))))
    }

    fn render_list(&self, docs: &[DocEntry], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let header = h_flex()
            .h(px(30.))
            .mx_6()
            .px_3()
            .gap_3()
            .border_b_1()
            .border_color(pal.border)
            .child(div().w(px(52.)))
            .child(self.list_header_cell("h-name", "NAME", Some(SortKey::Name), cx).flex_1().min_w_0())
            .child(self.list_header_cell("h-folder", "FOLDER", None, cx).w(px(260.)))
            .child(self.list_header_cell("h-mod", "MODIFIED", Some(SortKey::Modified), cx).w(px(120.)))
            .child(self.list_header_cell("h-size", "SIZE", None, cx).w(px(70.)))
            .child(div().w(px(28.)));
        let rows: Vec<AnyElement> = docs
            .iter()
            .enumerate()
            .map(|(ix, d)| {
                let selected = self.selected.as_ref() == Some(&d.path);
                let open = crate::files::is_document_open(&d.path, cx);
                let view = cx.entity();
                let v2 = view.clone();
                let (d1, d2, d3) = (d.clone(), d.clone(), d.clone());
                let p1 = d.path.clone();
                h_flex()
                    .id(SharedString::from(format!("row-{ix}")))
                    .h(px(48.))
                    .px_3()
                    .gap_3()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .when(selected, |r| r.bg(pal.accent_soft))
                    .when(!selected, |r| r.hover(|r| r.bg(pal.panel_hover)))
                    .child(div().rounded_sm().overflow_hidden().border_1().border_color(pal.border).child(self.thumbnail(d, 50.0, 36.0, false)))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(if d.broken { pal.warning } else { pal.text }).child(d.name.clone()))
                            .when(open, |r| r.child(Self::open_badge())),
                    )
                    .child(div().w(px(260.)).truncate().text_xs().text_color(pal.text_faint).child(display_path(d.folder())))
                    .child(div().w(px(120.)).text_xs().text_color(pal.text_muted).child(lib::capitalize(&lib::format_modified(d.modified))))
                    .child(div().w(px(70.)).text_xs().text_color(pal.text_faint).child(lib::format_size(d.size)))
                    .child(
                        div().w(px(28.)).flex().justify_end().child(
                            Button::new(SharedString::from(format!("rmore-{ix}")))
                                .icon(IconName::EllipsisVertical)
                                .ghost()
                                .xsmall()
                                .dropdown_menu(move |menu, _, _| Self::doc_menu(view.clone(), d3.clone(), menu)),
                        ),
                    )
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| this.on_doc_click(&d1, ev, window, cx)))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.select(&p1, cx)))
                    .context_menu(move |menu, _, _| Self::doc_menu(v2.clone(), d2.clone(), menu))
                    .into_any_element()
            })
            .collect();
        v_flex()
            .size_full()
            .child(header)
            .child(div().id("doc-list").flex_1().min_h_0().overflow_y_scrollbar().child(v_flex().px_6().py_1().gap_0p5().children(rows)))
            .into_any_element()
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let searching = !self.query.trim().is_empty();
        let (title, body): (String, String) = if !self.loaded {
            ("Loading documents…".into(), String::new())
        } else if searching {
            (format!("No documents match \"{}\"", self.query.trim()), "Try a different name, or look in All documents.".into())
        } else {
            match &self.source {
                Source::Recent => ("No recent documents".into(), "Documents you open appear here.".into()),
                Source::Folder(_) => ("This folder has no noshape documents".into(), "Create a document here, or add another folder.".into()),
                Source::All => ("No documents yet".into(), "Create your first document to start modeling.".into()),
            }
        };
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .pb(px(80.))
            .child(div().size(px(64.)).rounded_full().bg(pal.panel_raised).flex().items_center().justify_center().child(icon(ICON_BOX).size(px(30.)).text_color(pal.text_faint)))
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(title))
            .when(!body.is_empty(), |d| d.child(div().text_sm().text_color(pal.text_muted).child(body)))
            .when(self.loaded && !searching, |d| {
                d.child(
                    accent_button("empty-new", "New document", false)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.new_document(window, cx))),
                )
            })
            .into_any_element()
    }
}

impl Render for StartView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let docs = self.visible();
        let header = self.render_header(docs.len(), cx);
        let content = if docs.is_empty() {
            self.render_empty(cx)
        } else if self.view == ViewMode::Grid {
            self.render_grid(&docs, cx)
        } else {
            self.render_list(&docs, cx)
        };
        v_flex()
            .size_full()
            .bg(pal.app_bg)
            .text_color(pal.text)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &NewDocument, window, cx| this.new_document(window, cx)))
            .on_action(cx.listener(|this, _: &OpenDocument, window, cx| this.open_dialog(window, cx)))
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
                }
            }))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_selection(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_selection(-1, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| this.search.update(cx, |s, cx| s.focus(window, cx))))
            .child(self.render_title_bar())
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.render_sidebar(cx))
                    .child(v_flex().flex_1().min_w_0().h_full().child(header).child(div().flex_1().min_h_0().child(content))),
            )
    }
}

/// One segment of the grid/list toggle.
fn seg_button(id: &'static str, ic: Icon, tip: &'static str, selected: bool) -> Stateful<Div> {
    let pal = p();
    div()
        .id(id)
        .w(px(28.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .cursor_pointer()
        .when(selected, |d| d.bg(pal.panel_active))
        .when(!selected, |d| d.hover(|d| d.bg(pal.panel_hover)))
        .child(ic.size(px(15.)).text_color(if selected { pal.accent_hover } else { pal.text_faint }))
        .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx))
}

/// The accent call-to-action button (the kit's primary style is neutral here).
fn accent_button(id: &'static str, label: &'static str, full_width: bool) -> Stateful<Div> {
    let pal = p();
    h_flex()
        .id(id)
        .h(px(32.))
        .px_4()
        .gap_2()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .bg(pal.accent)
        .text_color(rgb(0x0f0d1f))
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .shadow_sm()
        .hover(|d| d.bg(pal.accent_hover))
        .active(|d| d.opacity(0.85))
        .when(full_width, |d| d.w_full())
        .child(Icon::new(IconName::Plus).size(px(15.)))
        .child(label)
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

/// Modal name prompt. `then` returns whether to close the dialog.
fn prompt_name(
    title: &'static str,
    ok_label: &'static str,
    default: String,
    hint: String,
    window: &mut Window,
    cx: &mut App,
    then: impl Fn(String, &mut Window, &mut App) -> bool + 'static,
) {
    use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
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
    window.open_dialog(cx, move |dialog, _, _| {
        let st = state.clone();
        let then = then.clone();
        dialog
            .title(title)
            .w(px(440.))
            .child(
                v_flex()
                    .gap_2()
                    .child(Input::new(&state))
                    .child(div().text_xs().text_color(p().text_faint).child(hint.clone())),
            )
            .footer(
                DialogFooter::new()
                    .gap_2()
                    .child(DialogClose::new().child(Button::new("cancel").label("Cancel").outline()))
                    .child(DialogAction::new().child(Button::new("ok").label(ok_label).primary())),
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
