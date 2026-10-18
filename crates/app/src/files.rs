//! Local document files: recent list, new/open helpers and window opening.

use crate::gpu::Gpu;
use crate::{doc_model, session, workspace};
use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::path::{Path, PathBuf};

fn config_dir() -> PathBuf {
    let d = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("noshape");
    let _ = std::fs::create_dir_all(&d);
    d
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
    let _ = std::fs::write(config_dir().join("recent.json"), serde_json::to_vec_pretty(&list).unwrap_or_default());
}

/// Open (or create) a document in a new window.
pub fn open_document_window(path: PathBuf, create_with: Option<noshape_model::Document>, gpu: Gpu, cx: &mut App) -> anyhow::Result<()> {
    let session = if path.exists() {
        session::Session::open(&path)?
    } else {
        session::Session::create(&path, create_with.unwrap_or_default())?
    };
    add_recent(&path);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
        window_min_size: Some(size(px(900.), px(560.))),
        #[cfg(target_os = "linux")]
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("dev.noshape.noshape".into()),
        ..TitleBar::window_options()
    };
    gpui_kit::open_window(options, cx, move |window, cx| {
        window.set_window_title(&format!("{} — noshape", session.name()));
        let doc = cx.new(|_| doc_model::DocModel::new(session));
        let ws = cx.new(|cx| workspace::Workspace::new(doc, gpu, window, cx));
        let fh = ws.read(cx).focus_handle(cx);
        window.defer(cx, move |window, cx| fh.focus(window, cx));
        ws
    })?;
    Ok(())
}

/// Ask for a document to open (native dialog) and open it.
pub fn prompt_open(gpu: Gpu, cx: &mut App) {
    let dir = documents_dir();
    cx.spawn(async move |cx| {
        let file = rfd::AsyncFileDialog::new().set_title("Open noshape document").set_directory(&dir).add_filter("noshape document", &["noshape"]).pick_file().await;
        if let Some(f) = file {
            let path = f.path().to_path_buf();
            let _ = cx.update(|cx| {
                if let Err(e) = open_document_window(path, None, gpu, cx) {
                    log::error!("open failed: {e:#}");
                }
            });
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
                let _ = cx.update(|cx| then(p, cx));
            }
        })
        .detach();
        return;
    }
    in_app_save_prompt(title, "Save", documents_dir().join(&default_name), window, cx, then);
}

/// Ask for a file to open, then call `then` with it.
pub fn request_open_path(title: &'static str, filter_name: &'static str, exts: &'static [&'static str], window: &mut Window, cx: &mut App, then: impl FnOnce(PathBuf, &mut App) + 'static) {
    if native_dialogs_available() {
        cx.spawn(async move |cx| {
            let picked = rfd::AsyncFileDialog::new().set_title(title).set_directory(documents_dir()).add_filter(filter_name, exts).pick_file().await.map(|f| f.path().to_path_buf());
            if let Some(p) = picked {
                let _ = cx.update(|cx| then(p, cx));
            }
        })
        .detach();
        return;
    }
    in_app_save_prompt(title, "Open", documents_dir().join(""), window, cx, then);
}

fn in_app_save_prompt(title: &'static str, ok_label: &'static str, default: PathBuf, window: &mut Window, cx: &mut App, then: impl FnOnce(PathBuf, &mut App) + 'static) {
    use gpui_kit::component::WindowExt as _;
    use gpui_kit::component::button::{Button, ButtonVariants as _};
    use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
    use gpui_kit::component::input::{Input, InputEvent, InputState};
    let state = cx.new(|cx| InputState::new(window, cx).default_value(default.to_string_lossy().to_string()));
    let then: std::rc::Rc<std::cell::RefCell<Option<Box<dyn FnOnce(PathBuf, &mut App)>>>> = std::rc::Rc::new(std::cell::RefCell::new(Some(Box::new(then))));
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
    window.open_dialog(cx, move |dialog, _, _| {
        let st = state.clone();
        let then = then.clone();
        dialog
            .title(title)
            .w(px(560.))
            .child(Input::new(&state))
            .footer(
                DialogFooter::new()
                    .gap_2()
                    .child(DialogClose::new().child(Button::new("cancel").label("Cancel").outline()))
                    .child(DialogAction::new().child(Button::new("ok").label(ok_label).primary())),
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

/// Async variant used where no window is at hand (native dialog only).
pub async fn prompt_save(title: &str, default_name: &str, ext: &str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .set_directory(documents_dir())
        .set_file_name(default_name)
        .add_filter(ext.to_uppercase(), &[ext])
        .save_file()
        .await
        .map(|f| f.path().to_path_buf())
}
