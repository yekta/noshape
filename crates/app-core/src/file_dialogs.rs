//! The file dialogs: a save location or files to open, through the native dialog when the platform has one (an XDG
//! desktop portal on Linux), else an in-app path prompt. Shared by every studio (moved from the app's `files`).

use gpui_kit::*;
use std::path::{Path, PathBuf};

pub fn documents_dir() -> PathBuf {
    let base = dirs::document_dir().or_else(|| dirs::home_dir().map(|h| h.join("Documents"))).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("noshape");
    let _ = std::fs::create_dir_all(&dir);
    dir
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
