//! Local document files: recent list, new/open helpers and window opening.

use crate::gpu::Gpu;
use crate::{doc_model, session, workspace};
use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::path::{Path, PathBuf};

pub(crate) fn config_dir() -> PathBuf {
    noshape_scene::config_dir()
}

pub fn documents_dir() -> PathBuf {
    let base = dirs::document_dir().or_else(|| dirs::home_dir().map(|h| h.join("Documents"))).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("noshape");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn new_document_path() -> PathBuf {
    let dir = documents_dir();
    (1..).map(|n| dir.join(format!("Untitled {n}.noshape"))).find(|p| !p.exists()).expect("free name")
}

pub fn recent() -> Vec<PathBuf> {
    let f = config_dir().join("recent.json");
    std::fs::read(&f)
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.exists())
        .collect()
}

pub fn add_recent(p: &Path) {
    let mut list = recent();
    let p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    list.retain(|x| *x != p);
    list.insert(0, p);
    list.truncate(15);
    write_recent(&list);
}

fn write_recent(list: &[PathBuf]) {
    let _ = std::fs::write(config_dir().join("recent.json"), serde_json::to_vec_pretty(list).unwrap_or_default());
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Remove a document from the recent list (start window "Remove from recent").
pub fn remove_recent(p: &Path) {
    let p = canonical(p);
    let mut list = recent();
    list.retain(|x| *x != p);
    write_recent(&list);
}

/// Point a recent entry at a document's new location (after a rename).
pub fn replace_recent(old: &Path, new: &Path) {
    let new = canonical(new);
    let list: Vec<PathBuf> = recent_raw().into_iter().map(|x| if x == old { new.clone() } else { x }).collect();
    write_recent(&list);
}

fn recent_raw() -> Vec<PathBuf> {
    std::fs::read(config_dir().join("recent.json")).ok().and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok()).unwrap_or_default()
}

/// Extra folders the start window lists `.noshape` files from (besides
/// [`documents_dir`]). Persisted in the config dir.
pub fn watched_folders() -> Vec<PathBuf> {
    std::fs::read(config_dir().join("folders.json")).ok().and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok()).unwrap_or_default()
}

pub fn set_watched_folders(list: &[PathBuf]) {
    let _ = std::fs::write(config_dir().join("folders.json"), serde_json::to_vec_pretty(list).unwrap_or_default());
}

/// The user's name for version history (Preferences); None = the OS name.
pub fn user_name_pref() -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(config_dir().join("user.json")).ok()?).ok()?;
    v.get("name")?.as_str().map(str::to_string).filter(|n| !n.trim().is_empty())
}

/// Save the name (empty = use the OS name) and use it for new history.
pub fn set_user_name_pref(name: &str) {
    let name = name.trim();
    let _ = std::fs::write(config_dir().join("user.json"), serde_json::to_vec_pretty(&serde_json::json!({ "name": name })).unwrap_or_default());
    noshape_store::set_default_author(Some(name.to_string()).filter(|n| !n.is_empty()));
}

/// Open document windows by canonical path (so opening twice focuses).
#[derive(Default)]
struct OpenDocs(std::collections::HashMap<PathBuf, AnyWindowHandle>);
impl Global for OpenDocs {}

/// Whether the document at `path` is open in a window.
pub fn is_document_open(path: &Path, cx: &App) -> bool {
    cx.try_global::<OpenDocs>().is_some_and(|o| o.0.contains_key(&canonical(path)))
}

/// App-level window lifecycle: keeps the open-document registry current and
/// shows the start window when the last document window closes. Quits when
/// the start window is the last window closed (macOS keeps running and
/// reopens the start window from the dock).
pub fn init(cx: &mut App) {
    cx.set_global(OpenDocs::default());
    // Author of new history entries: the Preferences name, else the OS name.
    noshape_store::set_default_author(user_name_pref());
    // Variable Studios inserted from other documents read those documents'
    // versions (cached snapshots).
    noshape_model::variables::set_external_loader(|path, mv| mv.parse().ok().and_then(|mv| crate::derived::external_snapshot(path, mv).ok()));
    // Feature Studio imports find other documents by id in the document library.
    noshape_ops::derived::set_document_scanner(|| crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs.into_iter().map(|d| d.path).collect());
    crate::start_window::init(cx);
    cx.on_window_closed(|cx, id| {
        cx.global_mut::<OpenDocs>().0.retain(|_, h| h.window_id() != id);
        // The Preferences page closes like the start window (last window: quit).
        let was_start = crate::start_window::forget_if(id, cx) | crate::preferences_window::forget_if(id, cx) | crate::company_settings::forget_if(id, cx);
        if !cx.windows().is_empty() {
            crate::start_window::refresh(cx);
            return;
        }
        if cfg!(target_os = "macos") {
            return;
        }
        if was_start {
            cx.quit();
        } else {
            show_start_window(cx);
        }
    })
    .detach();
}

/// Show (or focus) the documents start window.
pub fn show_start_window(cx: &mut App) {
    crate::start_window::show(cx);
}


/// Open (or create) a document in a new window. If it is already open, its
/// window is brought to the front instead.
pub fn open_document_window(path: PathBuf, create_with: Option<noshape_model::Document>, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    if let Some(h) = cx.try_global::<OpenDocs>().and_then(|o| o.0.get(&canonical(&path)).copied())
        && h.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return Ok(());
        }
    noshape_render::perf::milestone("opening document");
    let session = if path.exists() {
        session::Session::open(&path)?
    } else {
        session::Session::create(&path, create_with.unwrap_or_else(crate::units_prefs::new_document))?
    };
    noshape_render::perf::milestone("document session opened");
    add_recent(&path);
    let (handle, _) = open_session_window(session, gpu, cx)?;
    noshape_render::perf::milestone("document window opened");
    if cx.has_global::<OpenDocs>() {
        cx.global_mut::<OpenDocs>().0.insert(canonical(&path), handle);
    }
    Ok(())
}

/// A document at a version in a window of its own, read-only (Onshape opens
/// a version in a new browser tab): another window on the same file, not the
/// document's registered one. Returns the window and its workspace.
pub fn open_version_window(path: PathBuf, version: noshape_store::MicroversionId, gpu: Gpu, cx: &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<workspace::Workspace>)> {
    let mut session = session::Session::open(&path)?;
    session.view_microversion(Some(version))?;
    open_session_window(session, gpu, cx)
}

/// Tab › Open in new browser tab: another window on the same open document
/// (Onshape's second browser tab on the workspace; both show every edit),
/// with `tab` active. The document stays registered to its first window.
pub fn open_tab_window(doc: Entity<doc_model::DocModel>, tab: noshape_model::document::TabId, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
        window_min_size: Some(size(px(900.), px(560.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..TitleBar::window_options()
    };
    let title = format!("{} — noshape", doc.read(cx).session.name());
    crate::os_menu::open_window(options, cx, move |window, cx| {
        window.set_window_title(&title);
        let ws = cx.new(|cx| workspace::Workspace::new(doc, gpu, window, cx));
        ws.update(cx, |w, cx| w.activate(tab, window, cx));
        ws
    })?;
    Ok(())
}

fn open_session_window(session: session::Session, gpu: Gpu, cx: &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<workspace::Workspace>)> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
        window_min_size: Some(size(px(900.), px(560.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..TitleBar::window_options()
    };
    let (handle, ws) = crate::os_menu::open_window(options, cx, move |window, cx| {
        window.set_window_title(&format!("{} — noshape", session.name()));
        let doc = cx.new(|_| doc_model::DocModel::new(session));
        // The CLI and MCP server edit this document here while it is open.
        crate::live_server::serve(&doc, cx);
        let ws = cx.new(|cx| workspace::Workspace::new(doc, gpu, window, cx));
        let fh = ws.read(cx).focus_handle(cx);
        // The workspace focuses its active tab view (so Part Studio shortcuts
        // work right away); only fall back to the workspace itself.
        window.defer(cx, move |window, cx| {
            if window.focused(cx).is_none() {
                fh.focus(window, cx);
            }
        });
        ws
    })?;
    Ok((handle, ws))
}

/// Ask for a document to open and open it: the native dialog when available,
/// otherwise the in-app path prompt in the active window (Linux without an
/// XDG desktop portal), like [`request_open_path`].
pub fn prompt_open(gpu: Gpu, cx: &mut App) {
    let open = move |path: PathBuf, cx: &mut App| {
        if let Err(e) = open_document_window(path, None, gpu, cx) {
            log::error!("open failed: {e:#}");
        }
    };
    if !native_dialogs_available()
        && let Some(handle) = cx.active_window().or_else(|| cx.windows().into_iter().next())
    {
        // Deferred: the action is dispatched while that window is being
        // updated, so it can't be borrowed right now.
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| request_open_path("Open noshape document", "noshape document", &["noshape"], window, cx, open));
        });
        return;
    }
    let dir = documents_dir();
    cx.spawn(async move |cx| {
        let file = rfd::AsyncFileDialog::new().set_title("Open noshape document").set_directory(&dir).add_filter("noshape document", &["noshape"]).pick_file().await;
        if let Some(f) = file {
            let path = f.path().to_path_buf();
            cx.update(|cx| open(path, cx));
        }
    })
    .detach();
}

/// Whether a native file dialog can be shown. macOS and Windows always can;
/// Linux needs an XDG desktop portal (present on regular desktops).
pub fn native_dialogs_available() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    if std::env::var_os("NOSHAPE_NO_NATIVE_DIALOGS").is_some() {
        return false;
    }
    ["/usr/libexec/xdg-desktop-portal", "/usr/lib/xdg-desktop-portal", "/usr/lib/xdg-desktop-portal/xdg-desktop-portal", "/usr/bin/xdg-desktop-portal"]
        .iter()
        .any(|p| Path::new(p).exists())
}

/// Ask for a save location, then call `then` with the chosen path.
/// Uses the native dialog when available, otherwise an in-app prompt.
pub fn request_save_path(title: &'static str, default_name: String, ext: &'static str, window: &mut Window, cx: &mut App, then: impl FnOnce(PathBuf, &mut App) + 'static) {
    if native_dialogs_available() {
        let name = default_name.clone();
        cx.spawn(async move |cx| {
            let picked = rfd::AsyncFileDialog::new()
                .set_title(title)
                .set_directory(documents_dir())
                .set_file_name(&name)
                .add_filter(ext.to_uppercase(), &[ext])
                .save_file()
                .await
                .map(|f| f.path().to_path_buf());
            if let Some(p) = picked {
                cx.update(|cx| then(p, cx));
            }
        })
        .detach();
        return;
    }
    // Deferred: callers often ask from a dialog's OK handler, and closing that
    // dialog would otherwise close this prompt (it is a dialog too).
    window.defer(cx, move |window, cx| in_app_save_prompt(title, "Save", documents_dir().join(&default_name), window, cx, then));
}

/// Ask for a file to open, then call `then` with it.
pub fn request_open_path(title: &'static str, filter_name: &'static str, exts: &'static [&'static str], window: &mut Window, cx: &mut App, then: impl FnOnce(PathBuf, &mut App) + 'static) {
    if native_dialogs_available() {
        cx.spawn(async move |cx| {
            let picked = rfd::AsyncFileDialog::new().set_title(title).set_directory(documents_dir()).add_filter(filter_name, exts).pick_file().await.map(|f| f.path().to_path_buf());
            if let Some(p) = picked {
                cx.update(|cx| then(p, cx));
            }
        })
        .detach();
        return;
    }
    window.defer(cx, move |window, cx| in_app_save_prompt(title, "Open", documents_dir().join(""), window, cx, then));
}

/// Ask for one or more files (Onshape's import picker allows several), then
/// call `then` with them. The in-app prompt takes paths separated by `;`.
pub fn request_open_paths(title: &'static str, filter_name: &'static str, exts: &'static [&'static str], window: &mut Window, cx: &mut App, then: impl FnOnce(Vec<PathBuf>, &mut App) + 'static) {
    if native_dialogs_available() {
        cx.spawn(async move |cx| {
            let picked: Vec<PathBuf> = rfd::AsyncFileDialog::new().set_title(title).set_directory(documents_dir()).add_filter(filter_name, exts).pick_files().await.map(|v| v.iter().map(|f| f.path().to_path_buf()).collect()).unwrap_or_default();
            if !picked.is_empty() {
                cx.update(|cx| then(picked, cx));
            }
        })
        .detach();
        return;
    }
    window.defer(cx, move |window, cx| {
        in_app_save_prompt(title, "Open", documents_dir().join(""), window, cx, move |p, cx| {
            let paths: Vec<PathBuf> = p.to_string_lossy().split(';').map(|s| PathBuf::from(s.trim())).filter(|p| !p.as_os_str().is_empty()).collect();
            then(paths, cx)
        })
    });
}

/// The continuation of a path prompt, taken once when the user confirms.
type PendingPath = std::rc::Rc<std::cell::RefCell<Option<Box<dyn FnOnce(PathBuf, &mut App)>>>>;

fn in_app_save_prompt(title: &'static str, ok_label: &'static str, default: PathBuf, window: &mut Window, cx: &mut App, then: impl FnOnce(PathBuf, &mut App) + 'static) {
    use gpui_kit::component::WindowExt as _;
    use gpui_kit::component::input::{Input, InputEvent, InputState};
    let state = cx.new(|cx| InputState::new(window, cx).default_value(default.to_string_lossy().to_string()));
    let then: PendingPath = std::rc::Rc::new(std::cell::RefCell::new(Some(Box::new(then))));
    // Enter in the field confirms.
    {
        let then = then.clone();
        let st = state.clone();
        window
            .subscribe(&state, cx, move |_, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    let path = PathBuf::from(st.read(cx).value().to_string());
                    if let Some(f) = then.borrow_mut().take() {
                        f(path, cx);
                    }
                    window.close_dialog(cx);
                }
            })
            .detach();
    }
    let focus_state = state.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let st = state.clone();
        let then = then.clone();
        // Onshape modal: title bar, body, blue OK then gray Cancel.
        crate::os_ui::modal(dialog, title, 560.)
            .child(crate::os_ui::modal_body().child(Input::new(&state).h(px(32.)).text_size(px(14.))))
            .footer(
                crate::os_ui::modal_footer()
                    .child(crate::os_ui::modal_ok(crate::os_ui::primary_button("ok", ok_label)))
                    .child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("cancel", "Cancel", cx))),
            )
            .on_ok(move |_, _, cx| {
                let path = PathBuf::from(st.read(cx).value().to_string());
                if let Some(f) = then.borrow_mut().take() {
                    f(path, cx);
                }
                true
            })
    });
    // Focus the field once the dialog is mounted (opening it moves focus).
    window.defer(cx, move |window, cx| {
        focus_state.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        })
    });
}

