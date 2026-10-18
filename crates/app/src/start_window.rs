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
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
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
const CARD_W: f32 = 224.0;
const THUMB_H: f32 = 168.0;

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
    /// Documents with this label (Onshape's Labels).
    Label(String),
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
        Self { view: ViewMode::List, sort: SortKey::Modified, descending: true, source: None }
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
    /// Right panel (Details / Versions and history) and the details read for it.
    panel: Option<DocPanel>,
    details: Option<(PathBuf, i64, lib::DocDetails)>,
    /// The "Folders" section of "Owned by me" is expanded.
    folders_open: bool,
    /// The search box placeholder shown ("Search in <place>").
    placeholder: String,
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
            labels: lib::Labels::load(),
            labels_open: true,
            panel: None,
            details: None,
            folders_open: true,
            placeholder: String::new(),
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
            Source::Label(l) => self.labels.has(&d.path, l),
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
        let id = cx.default_global::<StartToast>().1 + 1;
        *cx.default_global::<StartToast>() = StartToast(Some((kind, message.into())), id);
        window.refresh();
        let handle = window.window_handle();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(8)).await;
            let _ = cx.update(|cx| {
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
            Err(e) => Self::notify(window, cx, AlertKind::Error, format!("{}: {}", "Could not open document", format!("{e:#}"))),
        }
    }

    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let root = crate::files::documents_dir();
        let start = match &self.source {
            Source::Folder(f) => f.clone(),
            _ => root.clone(),
        };
        let default = lib::next_untitled(&start);
        let location = Location { others: self.folders.iter().filter(|f| **f != root).cloned().collect(), root, chosen: Rc::new(RefCell::new(start)) };
        let chosen = location.chosen.clone();
        let view = cx.entity();
        let prompt = NamePrompt { title: "New document", ok_label: "Create document", label: "Document name", default, hint: None, location: Some(location) };
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
        const EXTS: &[&str] = &["noshape", "step", "stp", "iges", "igs", "brep", "stl", "obj"];
        crate::files::request_open_path("Import files", "noshape documents, CAD and mesh files", EXTS, window, cx, move |path, cx| {
            let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let result = if ext == lib::EXT {
                crate::files::open_document_window(path, None, gpu, cx)
            } else {
                (|| {
                    let mut doc = noshape_model::Document::new();
                    let units = doc.settings.units;
                    let brep = crate::workspace::read_foreign(&path, &units)?;
                    let (name, file) = crate::workspace::import_names(&path);
                    let mut ps = noshape_model::PartStudio::new();
                    crate::workspace::add_import_feature(&mut ps, &file, brep, &units);
                    doc.tabs.clear();
                    doc.settings.tab_order.clear();
                    doc.add_tab(&name, noshape_model::TabContent::PartStudio(ps));
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
                let _ = handle.update(cx, |_, window, cx| Self::notify(window, cx, AlertKind::Error, format!("Import failed: {e:#}")));
            }
            view.update(cx, |this, cx| this.force_rescan(cx));
        });
    }

    /// Create ▾ › Folder…: a new folder in the current place.
    fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let place = self.current_place();
        let view = cx.entity();
        let prompt = NamePrompt { title: "New folder", ok_label: "Create", label: "Folder name", default: "New folder".into(), hint: None, location: None };
        prompt_name(prompt, window, cx, move |name, window, cx| {
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
            let path = path.canonicalize().unwrap_or(path);
            let mut list = crate::files::watched_folders();
            list.push(path);
            crate::files::set_watched_folders(&list);
            view.update(cx, |this, cx| {
                this.folders = ScanInput::current().folders;
                this.folders_open = true;
                this.force_rescan(cx);
            });
            true
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
            Self::notify(window, cx, AlertKind::Warning, format!("{}: {}", d.name.clone(), "Close the document's window before renaming it"));
            return;
        }
        let view = cx.entity();
        let hint = format!("Renames {}", d.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
        let prompt = NamePrompt { title: "Rename document", ok_label: "Rename", label: "Document name", default: d.name.clone(), hint: Some(hint), location: None };
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
                Self::notify(window, cx, AlertKind::Error, format!("{}: {}", "Rename failed", format!("{e:#}")));
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
                Err(e) => Self::notify(window, cx, AlertKind::Error, format!("{}: {}", "Duplicate failed", format!("{e:#}"))),
            });
        })
        .detach();
    }

    fn reveal(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = crate::files::reveal_in_file_manager(path) {
            Self::notify(window, cx, AlertKind::Error, format!("{}: {}", "Could not open the file manager", format!("{e:#}")));
        }
    }

    fn trash(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if crate::files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{}: {}", d.name.clone(), "Close the document's window before moving it to the trash"));
            return;
        }
        match lib::trash_document(&d.path) {
            Ok(()) => {
                crate::files::remove_recent(&d.path);
                if self.selected.as_ref() == Some(&d.path) {
                    self.selected = None;
                }
                Self::notify(window, cx, AlertKind::Info, format!("Moved \"{}\" to the trash", d.name));
                self.force_rescan(cx);
            }
            Err(e) => Self::notify(window, cx, AlertKind::Error, format!("{}: {}", "Could not move to trash", format!("{e:#}"))),
        }
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
        let prompt = NamePrompt { title: "Create label", ok_label: "Create", label: "Label name", default: String::new(), hint: None, location: None };
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
        window.open_dialog(cx, move |dialog, _, cx| {
            let body = Self::label_checklist(&view, &d.path, cx);
            os::modal(dialog, "Labels", 400.)
                .child(os::modal_body().child(body))
                .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("close", "Close", cx))))
        });
    }

    /// The labels as checkbox rows for `path`, and "Create new label".
    fn label_checklist(view: &Entity<StartView>, path: &Path, cx: &App) -> AnyElement {
        let pal = p();
        let labels = view.read(cx).labels.clone();
        let mut col = v_flex().gap(px(2.));
        if labels.labels.is_empty() {
            col = col.child(div().py(px(10.)).text_center().text_size(px(14.)).text_color(pal.text).child("You have no labels yet"));
        }
        for (k, l) in labels.labels.iter().enumerate() {
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
                .h(px(30.))
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
                .text_size(px(14.))
                .text_color(pal.text)
                .hover(|d| d.bg(rgb(0xf5f5f5)))
                .child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 10., pal.text))
                .child(os::glyph(os::GLYPH_LABEL, 18., pal.text))
                .child(div().flex_1().child("Labels"))
                .child(
                    div()
                        .id("nav-label-add")
                        .child(os::glyph(os::GLYPH_LABEL_ADD, 20., pal.accent))
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
            crate::ui::rail_button(id, icon, on, true, cx).tooltip(crate::ui::tooltip(tip)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_panel(panel, cx)))
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
            .child(btn("doc-strip-versions", "history", "Versions and history", DocPanel::Versions, cx))
            .into_any_element()
    }

    /// The open right panel: Details (name, thumbnail, Owner, Document
    /// description, Created by, Last modified by) or Versions and history.
    fn render_doc_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.panel?;
        let pal = p();
        let entry = self.selected_entry();
        let details = entry.as_ref().and_then(|e| self.details_of(&e.path));
        let title = match panel {
            DocPanel::Details => "Details",
            DocPanel::Versions => "Versions and history",
        };
        let section = |label: &'static str, lines: Vec<String>| {
            v_flex()
                .py(px(6.))
                .border_t_1()
                .border_color(rgb(0xcccccc))
                .child(div().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(label))
                .children(lines.into_iter().map(|l| div().pt(px(4.)).text_size(px(13.33)).font_weight(FontWeight::LIGHT).text_color(pal.text).child(l)))
        };
        let body: AnyElement = match (&entry, &details) {
            (Some(e), Some(d)) if panel == DocPanel::Details => v_flex()
                .px(px(10.))
                .child(
                    h_flex()
                        .h(px(32.))
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
                        .child(div().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Document labels"))
                        .child(div().mt(px(6.)).p(px(6.)).border_1().border_color(rgb(0xcccccc)).child(Self::label_checklist(&cx.entity(), &e.path, cx))),
                )
                .child(section("Created by", vec![d.created_by.clone(), lib::format_onshape_time(d.created)]))
                .child(section("Last modified by", vec![d.modified_by.clone(), lib::format_onshape_time(d.modified)]))
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
                        .child(v_flex().child(author.clone()).child(lib::format_onshape_time(*t)))
                }))
                .into_any_element(),
            _ => v_flex()
                .items_center()
                .pt(px(120.))
                .gap(px(12.))
                .child(os::glyph(os::GLYPH_EMPTY, 90., rgb(0xcccccc)))
                .child(div().max_w(px(240.)).text_center().text_size(px(14.)).text_color(pal.text_muted).child("Select a document or folder to view its details"))
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
                        .h(px(30.))
                        .px(px(10.))
                        .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(title))
                        .child(
                            div()
                                .id("doc-panel-close")
                                .cursor_pointer()
                                .child(os::glyph(os::GLYPH_CLOSE, 16., pal.text_faint))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.panel = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(div().id("doc-panel-body").flex_1().min_h_0().overflow_y_scroll().child(body))
                .into_any_element(),
        )
    }

    /// "Move to…": Onshape's folder picker; moves the (closed) document.
    fn move_to(&mut self, d: DocEntry, window: &mut Window, cx: &mut Context<Self>) {
        if crate::files::is_document_open(&d.path, cx) {
            Self::notify(window, cx, AlertKind::Warning, format!("{}: {}", d.name.clone(), "Close the document's window before moving it"));
            return;
        }
        let root = crate::files::documents_dir();
        let location = Rc::new(Location { others: self.folders.iter().filter(|f| **f != root).cloned().collect(), root, chosen: Rc::new(RefCell::new(d.folder().to_path_buf())) });
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
                            Self::notify(window, cx, AlertKind::Error, format!("{}: {}", "Move failed", format!("{e:#}")));
                            false
                        }
                    }
                })
        });
    }


    /// Onshape's document row menu (live, documents/25: a Bootstrap
    /// `list-no-icons` dropdown): Open, Rename, Copy workspace… | Details,
    /// Versions and history | Move to… | Remove from recently opened, Send
    /// to trash. Cloud items (Share, Labels, Transfer ownership, new browser
    /// tab) are not part of noshape.
    fn doc_menu(view: Entity<StartView>, d: DocEntry, menu: OsMenu) -> OsMenu {
        let item = |label: &'static str, view: &Entity<StartView>, d: &DocEntry, f: fn(&mut StartView, DocEntry, &mut Window, &mut Context<StartView>)| {
            let (view, d) = (view.clone(), d.clone());
            OsMenuItem::new(label).on_click(move |_, window, cx| {
                let d = d.clone();
                view.update(cx, |this, cx| f(this, d, window, cx));
            })
        };
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
        if ev.click_count() >= 2 {
            self.open_path(d.path.clone(), window, cx);
        }
        cx.notify();
    }

    /// Onshape documents page top bar: 40 px `#eaeaea`, logo at the left,
    /// the search box (390 px, with its square search button) centered.
    fn render_title_bar(&self, cx: &App) -> AnyElement {
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
                                .child(
                                    div()
                                        .flex_1()
                                        .h_full()
                                        .bg(pal.panel)
                                        .border_1()
                                        .border_color(pal.border_strong)
                                        .rounded_l(px(2.))
                                        .child(Input::new(&self.search).appearance(false).small().h(px(26.)).cleanable(true)),
                                )
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
                    .child(div().w(px(220.)).flex_none().flex().justify_end().pr(px(10.)).child(user_menu(cx))),
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
            .pl(px(20.))
            .gap(px(7.))
            .cursor_pointer()
            .text_size(px(14.))
            .text_color(color)
            .when(active, |d| d.font_weight(FontWeight::SEMIBOLD).child(div().absolute().left_0().top_0().bottom_0().w(px(4.)).bg(pal.accent)))
            .when(!active, |d| d.hover(|d| d.bg(rgb(0xf5f5f5))))
            .child(os::glyph(glyph, 18., color))
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
            .dropdown_menu(move |menu, _, _| {
                // Onshape: Document…, Folder… | Import files…, Import from ▸
                // (cloud drives, not offered locally), Label….
                let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
                menu.min_w(px(150.))
                    .item(os::menu_item(os::GLYPH_DOCUMENT, "Document…").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.new_document(window, cx))))
                    .item(os::menu_item(os::GLYPH_FOLDER, "Folder…").on_click(move |_, window, cx| v2.update(cx, |t, cx| t.new_folder(window, cx))))
                    .separator()
                    .item(os::menu_item(os::GLYPH_OPEN, "Import files…").on_click(move |_, window, cx| v3.update(cx, |t, cx| t.import_files(window, cx))))
                    .item(os::menu_item(os::GLYPH_LABEL, "Label…").on_click(move |_, window, cx| v4.update(cx, |t, cx| t.create_label(None, window, cx))))
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
                    .child(labels_nav),
            )
            .into_any_element()
    }

    fn source_title(&self) -> String {
        match &self.source {
            Source::Recent => "Recently opened".into(),
            Source::All => "Owned by me".into(),
            Source::Folder(f) => folder_label(f),
            Source::Label(l) => l.clone(),
        }
    }

    /// "Owned by me" / "Owned by me › folder" breadcrumb (16 px / 500).
    fn render_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let glyph = if self.source == Source::Recent { os::GLYPH_RECENT } else { os::GLYPH_OWNED };
        let root = if self.source == Source::Recent { "Recently opened" } else { "Owned by me" };
        let in_folder = matches!(self.source, Source::Folder(_));
        h_flex()
            .h(px(52.))
            .flex_none()
            .pl(px(20.))
            .gap(px(6.))
            .text_size(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(pal.text)
            .child(os::glyph(glyph, 18., pal.text))
            .child(
                div()
                    .id("crumb-root")
                    .child(root)
                    .when(in_folder, |d| {
                        d.cursor_pointer().hover(|d| d.text_color(pal.accent)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_source(Source::All, cx)))
                    }),
            )
            .when(in_folder, |d| d.child(os::glyph(os::GLYPH_CHEVRON_RIGHT, 14., pal.text_muted)).child(os::glyph(os::GLYPH_FOLDER, 18., pal.text)).child(self.source_title()))
            .into_any_element()
    }

    /// The "Folders" section of "Owned by me": a collapsible caption and
    /// gray folder tiles, then the divider with its collapse handle.
    fn render_folders(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let docs_dir = crate::files::documents_dir();
        let open = self.folders_open;
        let has_tiles = self.folders.iter().any(|f| *f != docs_dir);
        let mut tiles = div().flex().flex_wrap().gap(px(10.)).pl(px(14.)).pr(px(10.));
        // The documents folder is "Owned by me" itself; the tiles are the
        // other folders added to the start window.
        for (i, f) in self.folders.clone().into_iter().filter(|f| *f != docs_dir).enumerate() {
            let view = cx.entity();
            let folder = f.clone();
            let source = Source::Folder(f.clone());
            tiles = tiles.child(
                h_flex()
                    .id(SharedString::from(format!("folder-{i}")))
                    .w(px(213.))
                    .h(px(38.))
                    .px(px(10.))
                    .gap(px(7.))
                    .rounded(px(3.))
                    .bg(rgb(0xf5f5f5))
                    .border_1()
                    .border_color(rgb(0xf1f1f1))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(os::glyph(os::GLYPH_FOLDER, 18., pal.text))
                    .child(div().flex_1().min_w_0().truncate().child(folder_label(&f)))
                    .tooltip(crate::ui::tooltip(display_path(&f)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_source(source.clone(), cx)))
                    .os_context_menu(move |menu, _, _| {
                        let (v1, v2) = (view.clone(), view.clone());
                        let (f1, f2) = (folder.clone(), folder.clone());
                        menu.list_no_icons()
                            .item(OsMenuItem::new("Show in file manager").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.reveal(&f1, window, cx))))
                            .separator()
                            .item(OsMenuItem::new("Remove from list").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.remove_folder(f2.clone(), cx))))
                    }),
            );
        }
        v_flex()
            .flex_none()
            .child(
                h_flex()
                    .id("folders-caption")
                    .h(px(30.))
                    .pl(px(12.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(pal.text)
                    .child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 14., pal.text))
                    .child(os::glyph(os::GLYPH_FOLDER, 18., pal.text))
                    .child("Folders")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.folders_open = !this.folders_open;
                        cx.notify();
                    })),
            )
            .when(open && has_tiles, |d| d.child(div().pt(px(6.)).pb(px(14.)).child(tiles)))
            .child(
                // The splitter between folders and documents, with its
                // round collapse handle in the middle.
                div()
                    .relative()
                    .h(px(20.))
                    .mx(px(10.))
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
                                .child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 12., pal.text))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.folders_open = !this.folders_open;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// View toggles above the table (Onshape's list / grid buttons).
    fn render_list_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(px(40.))
            .flex_none()
            .pl(px(14.))
            .gap(px(4.))
            .child(
                view_toggle("view-list", os::GLYPH_LIST_VIEW, "List", self.view == ViewMode::List)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_view(ViewMode::List, cx))),
            )
            .child(
                view_toggle("view-grid", os::GLYPH_GRID_VIEW, "Grid", self.view == ViewMode::Grid)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_view(ViewMode::Grid, cx))),
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

    fn render_card(&self, ix: usize, d: &DocEntry, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let selected = self.selected.as_ref() == Some(&d.path);
        let open = crate::files::is_document_open(&d.path, cx);
        let (d1, d2) = (d.clone(), d.clone());
        let p1 = d.path.clone();
        let v2 = cx.entity();
        let subtitle = if d.broken { "Cannot read this file".to_string() } else { lib::format_onshape_time(d.modified) };
        v_flex()
            .id(SharedString::from(format!("card-{ix}")))
            .w(px(CARD_W))
            .rounded(px(3.))
            .overflow_hidden()
            .border_1()
            .bg(pal.panel)
            .cursor_pointer()
            .when(selected, |c| c.border_color(pal.accent_soft).bg(pal.accent_soft))
            .when(!selected, |c| c.border_color(rgb(0xececec)).hover(|c| c.bg(pal.row_hover).border_color(pal.row_hover)))
            .child(
                div()
                    .relative()
                    .bg(pal.panel)
                    .child(self.thumbnail(d, CARD_W - 2.0, THUMB_H, true))
                    .when(open, |c| c.child(div().absolute().top_2().left_2().child(Self::open_badge()))),
            )
            .child(
                h_flex()
                    .pl(px(10.))
                    .pr_1()
                    .py(px(6.))
                    .gap_1()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(14.)).text_color(pal.text).truncate().child(d.name.clone()))
                            .child(div().text_size(px(12.)).text_color(if d.broken { pal.warning } else { pal.text_muted }).truncate().child(subtitle)),
                    )
            )
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| this.on_doc_click(&d1, ev, window, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| this.select(&p1, cx)))
            .os_context_menu(move |menu, _, _| Self::doc_menu(v2.clone(), d2.clone(), menu))
            .into_any_element()
    }

    fn render_grid(&self, docs: &[DocEntry], cx: &mut Context<Self>) -> AnyElement {
        let cards: Vec<AnyElement> = docs.iter().enumerate().map(|(i, d)| self.render_card(i, d, cx)).collect();
        div()
            .id("doc-grid")
            .size_full()
            .overflow_y_scrollbar()
            .child(div().pl(px(14.)).pr(px(10.)).pt(px(4.)).pb_6().flex().flex_wrap().gap(px(12.)).children(cards))
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

    fn render_list(&self, docs: &[DocEntry], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let header = h_flex()
            .h(px(32.))
            .flex_none()
            .border_b_1()
            .border_color(pal.border_strong)
            .child(self.list_header_cell("h-name", "Name", Some(SortKey::Name), cx).w(px(350.)).flex_none())
            .child(self.list_header_cell("h-mod", "Modified", Some(SortKey::Modified), cx).w(px(200.)).flex_none())
            .child(self.list_header_cell("h-modby", "Modified by", None, cx).w(px(200.)).flex_none())
            .child(self.list_header_cell("h-owner", "Owned by", None, cx).w(px(200.)).flex_none());
        // Onshape writes the signed-in user as "me".
        let me = noshape_store::local_user_name();
        let who = move |name: &str| if name.is_empty() || name == me { "me".to_string() } else { name.to_string() };
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
                            .when(open, |r| r.child(Self::open_badge())),
                    )
                    .child(cell(200.).child(lib::format_onshape_time(d.modified)))
                    .child(cell(200.).child(who(&d.modified_by)))
                    .child(cell(200.).child(who(&d.owner)))
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
            .child(header)
            .child(div().id("doc-list").flex_1().min_h_0().overflow_y_scrollbar().child(v_flex().children(rows)))
            .into_any_element()
    }

    /// Empty list: the large gray noshape mark and one line of `#666` text,
    /// as Onshape's "No documents owned by you."
    fn render_empty(&self) -> AnyElement {
        let pal = p();
        let searching = !self.query.trim().is_empty();
        let text: String = if !self.loaded {
            "Loading documents…".into()
        } else if searching {
            format!("No documents match \"{}\".", self.query.trim())
        } else {
            match &self.source {
                Source::Recent => "No recently opened documents.".into(),
                Source::Folder(_) => "No documents in this folder.".into(),
                Source::Label(_) => "No documents with this label.".into(),
                Source::All => "No documents owned by you.".into(),
            }
        };
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(20.))
            .child(os::glyph(os::GLYPH_EMPTY, 150., rgb(0xd7d7d7)))
            .child(div().text_size(px(14.)).text_color(pal.text_muted).child(text))
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
        let folders = (self.source == Source::All && self.query.trim().is_empty()).then(|| self.render_folders(cx));
        let toolbar = self.render_list_toolbar(cx);
        let strip = self.render_strip(cx);
        let panel = self.render_doc_panel(cx);
        let content = if docs.is_empty() {
            self.render_empty()
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
                }
            }))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_selection(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_selection(-1, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| this.search.update(cx, |s, cx| s.focus(window, cx))))
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
                            .children(folders)
                            .child(toolbar)
                            .child(div().flex_1().min_h_0().child(content)),
                    )
                    .children(panel)
                    .child(strip),
            )
            .child(self.render_footer())
            // Onshape right-click menus (`os_context_menu`).
            .children(crate::os_menu::render_host(cx))
            .children(cx.try_global::<StartToast>().and_then(|t| t.0.clone()).map(|(kind, msg)| {
                div().absolute().top(px(crate::ui::NAVBAR_H + 9.)).left_0().right_0().flex().justify_center().child(os::message_bubble(
                    "start-toast",
                    kind,
                    msg,
                    |_, window, cx| {
                        cx.default_global::<StartToast>().0 = None;
                        window.refresh();
                    },
                    cx,
                ))
            }))
    }
}

/// The documents page's message bubble (kind, text) and its generation.
#[derive(Default)]
struct StartToast(Option<(AlertKind, String)>, u64);

impl Global for StartToast {}

/// The documents page's right panels.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DocPanel {
    Details,
    Versions,
}

/// Onshape's account button at the top-right (avatar, name, ▾). Its menu
/// has the local items only: Preferences… (the name used as the author in
/// Versions and history).
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
        .dropdown_menu_with_anchor(Anchor::TopRight, |m, _, _| m.item(PopupMenuItem::new("Preferences…").on_click(|_, window, cx| open_preferences(window, cx))))
        .into_any_element()
}

/// Preferences: the author name for version history (empty = the OS name).
fn open_preferences(window: &mut Window, cx: &mut App) {
    let current = crate::files::user_name_pref().unwrap_or_default();
    let state = cx.new(|cx| InputState::new(window, cx).placeholder(noshape_store::os_login()).default_value(current));
    let os_name = noshape_store::os_user_name();
    window.open_dialog(cx, move |dialog, _, cx| {
        let st = state.clone();
        os::modal(dialog, "Preferences", 460.)
            .child(
                os::modal_body()
                    .child(os::field_label("Name"))
                    .child(Input::new(&state).h(px(32.)).text_size(px(14.)))
                    .child(div().text_size(px(12.)).text_color(p().text_muted).child(format!("Shown as the author in Versions and history. Leave empty to use your account name ({os_name})."))),
            )
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Save"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                crate::files::set_user_name_pref(&st.read(cx).value());
                window.refresh();
                true
            })
    });
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
    pub(crate) others: Vec<PathBuf>,
    pub(crate) chosen: Rc<RefCell<PathBuf>>,
}

impl Location {
    /// Onshape's "Document location" browser: a header with home, back and
    /// the current place, above the list of folders.
    pub(crate) fn render(&self) -> AnyElement {
        let pal = p();
        let chosen = self.chosen.borrow().clone();
        let at_root = chosen == self.root;
        let mut list = v_flex().p(px(6.)).gap(px(2.));
        if self.others.is_empty() {
            list = list.items_center().pt(px(15.)).child(os::info_box("No additional folders"));
        }
        for (i, f) in self.others.iter().enumerate() {
            let (c, f2) = (self.chosen.clone(), f.clone());
            list = list.child(
                os::row_states(
                    h_flex().id(SharedString::from(format!("loc-{i}"))).h(px(30.)).px(px(8.)).gap(px(7.)).rounded(px(3.)).cursor_pointer().text_size(px(14.)),
                    chosen == *f,
                )
                .child(os::glyph(os::GLYPH_FOLDER, 18., pal.text))
                .child(div().flex_1().min_w_0().truncate().child(folder_label(f)))
                .child(div().text_size(px(12.)).text_color(pal.text_faint).child(display_path(f)))
                .on_click(move |_, window, _| {
                    *c.borrow_mut() = f2.clone();
                    window.refresh();
                }),
            );
        }
        let home = {
            let (c, root) = (self.chosen.clone(), self.root.clone());
            move |_: &ClickEvent, window: &mut Window, _: &mut App| {
                *c.borrow_mut() = root.clone();
                window.refresh();
            }
        };
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
                    .child(div().id("loc-home").cursor_pointer().child(os::glyph(os::GLYPH_HOME, 20., pal.text_muted)).on_click(home.clone()))
                    .child(
                        div()
                            .id("loc-back")
                            .when(!at_root, |d| d.cursor_pointer().on_click(home))
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
    location: Option<Location>,
}

/// Modal name prompt in Onshape's modal style. `then` returns whether to
/// close the dialog.
fn prompt_name(prompt: NamePrompt, window: &mut Window, cx: &mut App, then: impl Fn(String, &mut Window, &mut App) -> bool + 'static) {
    let NamePrompt { title, ok_label, label, default, hint, location } = prompt;
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
    window.open_dialog(cx, move |dialog, _, cx| {
        let st = state.clone();
        let then = then.clone();
        os::modal(dialog, title, if location.is_some() { 598. } else { 460. })
            .child(
                os::modal_body()
                    .child(os::field_label(label))
                    .child(Input::new(&state).h(px(32.)).text_size(px(14.)))
                    .when_some(hint.clone(), |d, h| d.child(div().text_size(px(12.)).text_color(p().text_muted).child(h)))
                    .when_some(location.clone(), |d, loc| d.child(div().pt(px(10.)).child(os::field_label("Document location"))).child(loc.render())),
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
