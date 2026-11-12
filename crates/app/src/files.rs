//! Local document files: recent list, new/open helpers and window opening.

use crate::gpu::Gpu;
use crate::{doc_model, session, workspace};
use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::path::PathBuf;

pub use noshape_app_core::file_dialogs::{DialogPurpose, documents_dir, native_dialogs_available, remember_choice, request_open_path, request_open_path_for, request_open_paths, request_save_path, start_dir};
pub use noshape_app_core::doc_files::{OpenDocs, add_recent, canonical, is_document_open, new_document_path, replace_recent, user_name_pref, watched_folders};

/// App-level window lifecycle: keeps the open-document registry current and
/// shows the start window when the last document window closes (every OS).
/// Quits when the start window is the last window closed (macOS keeps running
/// and reopens the start window from the dock).
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
        // A document's last window shows the start window on every OS; closing that one quits, except that a
        // macOS app keeps running without windows (the OS convention; the dock reopens the start window).
        if !was_start {
            show_start_window(cx);
        } else if !cfg!(target_os = "macos") {
            cx.quit();
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
/// The document named on the command line, opened on a thread from process start so its store's reading overlaps the
/// app's and the window's setup (28–115 ms of a macOS launch); [`open_document_window`] takes it.
type Prefetched = (PathBuf, std::thread::JoinHandle<anyhow::Result<session::Session>>);
static PREFETCHED: std::sync::Mutex<Option<Prefetched>> = std::sync::Mutex::new(None);

pub fn prefetch_session(path: &std::path::Path) {
    if !path.exists() {
        return;
    }
    let p = path.to_path_buf();
    if let Ok(h) = std::thread::Builder::new().name("open-document".into()).spawn(move || session::Session::open(&p))
        && let Ok(mut slot) = PREFETCHED.lock()
    {
        *slot = Some((path.to_path_buf(), h));
    }
}

/// The prefetched session of `path`, waiting for it if it isn't open yet.
fn take_prefetched(path: &std::path::Path) -> Option<anyhow::Result<session::Session>> {
    let (_, h) = PREFETCHED.lock().ok()?.take_if(|(p, _)| p == path)?;
    h.join().ok()
}

pub fn open_document_window(path: PathBuf, create_with: Option<noshape_model::Document>, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    if let Some(h) = cx.try_global::<OpenDocs>().and_then(|o| o.0.get(&canonical(&path)).copied())
        && h.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return Ok(());
        }
    noshape_render::perf::milestone("opening document");
    let session = if let Some(s) = take_prefetched(&path) {
        s?
    } else if path.exists() {
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

/// Open a document (or bring its window to the front) with `tab` active
/// (the documents page's search results: a part or a tab).
pub fn open_document_at(path: PathBuf, tab: noshape_model::document::TabId, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    open_document_window(path.clone(), None, gpu, cx)?;
    if let Some(h) = cx.try_global::<OpenDocs>().and_then(|o| o.0.get(&canonical(&path)).copied()) {
        let _ = h.update(cx, |_, window, cx| {
            if let Some(ws) = crate::os_menu::content_view(window, cx).and_then(|v| v.downcast::<workspace::Workspace>().ok()) {
                ws.update(cx, |w, cx| w.activate(tab, window, cx));
            }
        });
    }
    Ok(())
}

/// Edit in context on another document's part (live 2464): `path` opened on Main (or its window brought to the
/// front) on Part Studio `studio`, the assembly's context snapshot stored there and shown around the part.
pub fn open_in_context(path: PathBuf, studio: noshape_model::document::TabId, ctx: noshape_app_core::assembly_context::AssemblyContext, install: (u32, Option<noshape_model::in_context::AssemblyContext>), gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    open_document_window(path.clone(), None, gpu, cx)?;
    let Some(h) = cx.try_global::<OpenDocs>().and_then(|o| o.0.get(&canonical(&path)).copied()) else { return Ok(()) };
    let _ = h.update(cx, |_, window, cx| {
        let Some(ws) = crate::os_menu::content_view(window, cx).and_then(|v| v.downcast::<workspace::Workspace>().ok()) else { return };
        ws.update(cx, |w, cx| w.enter_context_from_other(studio, ctx, install, window, cx));
    });
    Ok(())
}

/// Create version and update in assembly (live 2464): the assembly of `other` opened (or brought to the front) on
/// tab `assembly`, its instance of the document at `path` moved to microversion `mv` (version `name`).
pub fn update_linked_instance(other: noshape_app_core::assembly_context::OtherAssembly, assembly: noshape_model::document::TabId, path: PathBuf, mv: noshape_store::MicroversionId, name: &str, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    open_document_at(other.path.clone(), assembly, gpu, cx)?;
    let Some(h) = cx.try_global::<OpenDocs>().and_then(|o| o.0.get(&canonical(&other.path)).copied()) else { return Ok(()) };
    let name = name.to_string();
    let _ = h.update(cx, |_, window, cx| {
        let Some(ws) = crate::os_menu::content_view(window, cx).and_then(|v| v.downcast::<workspace::Workspace>().ok()) else { return };
        let Some(v) = ws.read(cx).assembly_view(assembly) else { return };
        v.update(cx, |v, cx| v.move_instance_to_version(other.instance, &path, mv, &name, cx));
    });
    Ok(())
}

/// Open a document (or bring its window to the front) with its Versions and history panel open (the documents
/// page's row › Versions and history: live lands on the document's /versionsAndHistory).
pub fn open_document_history(path: PathBuf, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    open_document_window(path.clone(), None, gpu, cx)?;
    if let Some(h) = cx.try_global::<OpenDocs>().and_then(|o| o.0.get(&canonical(&path)).copied()) {
        let _ = h.update(cx, |_, window, cx| {
            if let Some(ws) = crate::os_menu::content_view(window, cx).and_then(|v| v.downcast::<workspace::Workspace>().ok()) {
                ws.update(cx, |w, cx| w.show_history(window, cx));
            }
        });
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

/// Another workspace of a document in a window of its own (Onshape's new
/// browser tab on that workspace): another window on the same file, editing
/// that workspace; the document stays registered to its first window.
pub fn open_workspace_window(path: PathBuf, branch: noshape_store::BranchId, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    open_branch_window(path, branch, gpu, cx).map(|_| ())
}

/// [`open_workspace_window`], returning the window and its workspace (the merge dialog's Compare with base).
pub fn open_branch_window(path: PathBuf, branch: noshape_store::BranchId, gpu: Gpu, cx: &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<workspace::Workspace>)> {
    let mut session = session::Session::open(&path)?;
    session.switch_branch(branch)?;
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
    let title = format!("{} — Noshape", doc.read(cx).session.name());
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
    noshape_render::perf::milestone("window: opening");
    let (handle, ws) = crate::os_menu::open_window(options, cx, move |window, cx| {
        noshape_render::perf::milestone("window: created");
        window.set_window_title(&format!("{} — Noshape", session.name()));
        let doc = cx.new(|cx| {
            let mut d = doc_model::DocModel::new(session);
            d.watch_external(cx);
            d
        });
        noshape_render::perf::milestone("window: document model");
        // The CLI and MCP server edit this document here while it is open.
        crate::live_server::serve(&doc, cx);
        noshape_render::perf::milestone("window: live server");
        let ws = cx.new(|cx| workspace::Workspace::new(doc, gpu, window, cx));
        noshape_render::perf::milestone("window: workspace built");
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
            crate::start_window::show_with_error(format!("Could not open document: {e:#}"), cx);
        }
    };
    if !native_dialogs_available()
        && let Some(handle) = cx.active_window().or_else(|| cx.windows().into_iter().next())
    {
        // Deferred: the action is dispatched while that window is being
        // updated, so it can't be borrowed right now.
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| request_open_path_for(DialogPurpose::Document, "Open Noshape document", "Noshape document", &["noshape"], window, cx, open));
        });
        return;
    }
    // The folder a document was last opened from (user 11-12).
    let dir = start_dir(DialogPurpose::Document);
    cx.spawn(async move |cx| {
        let file = rfd::AsyncFileDialog::new().set_title("Open Noshape document").set_directory(&dir).add_filter("Noshape document", &["noshape"]).pick_file().await;
        if let Some(f) = file {
            let path = f.path().to_path_buf();
            remember_choice(DialogPurpose::Document, &path);
            cx.update(|cx| open(path, cx));
        }
    })
    .detach();
}
