//! The file dialogs: a save location or files to open, through the native dialog when the platform has one (an XDG
//! desktop portal on Linux), else an in-app path prompt. Shared by every studio (moved from the app's `files`).

use gpui_kit::*;
use std::path::{Path, PathBuf};

/// Where new documents go ("Owned by me"): see `noshape_scene::documents_dir`.
pub fn documents_dir() -> PathBuf {
    noshape_scene::documents_dir()
}

/// The path conventions a folder is shown in to its user.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathStyle {
    /// `C:\Users\ann\Documents\noshape`: the full path, backslashes, no `~`.
    Windows,
    /// `~/Documents/noshape` under the home folder (the shell's and file managers' way), else the full path.
    Unix,
}

impl PathStyle {
    pub fn current() -> Self {
        if cfg!(windows) { Self::Windows } else { Self::Unix }
    }
}

/// `path` as this OS shows it to its user (the documents page's footer, a document's details).
pub fn display_path(path: &Path) -> String {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().to_string());
    display_path_in(&path.to_string_lossy(), home.as_deref(), PathStyle::current())
}

/// `path` as the OS's own programs take it: on Windows without the verbatim prefix canonicalising adds (`\\?\C:\…`,
/// `\\?\UNC\server\…`), which Explorer refuses ("The parameter is incorrect.").
pub fn os_path(path: &Path) -> std::path::PathBuf {
    if !cfg!(windows) {
        return path.to_path_buf();
    }
    os_path_in(&path.to_string_lossy(), PathStyle::Windows).into()
}

/// [`os_path`] for a path written as `style`'s OS does.
pub fn os_path_in(path: &str, style: PathStyle) -> String {
    if style == PathStyle::Unix {
        return path.to_string();
    }
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

/// [`display_path`] for a path and home folder written as `style`'s OS does.
pub fn display_path_in(path: &str, home: Option<&str>, style: PathStyle) -> String {
    match style {
        PathStyle::Windows => os_path_in(path, style).replace('/', "\\"),
        PathStyle::Unix => match home.map(|h| h.trim_end_matches('/')).filter(|h| !h.is_empty()) {
            Some(h) if path == h => "~".to_string(),
            Some(h) if path.starts_with(&format!("{h}/")) => format!("~{}", &path[h.len()..]),
            _ => path.to_string(),
        },
    }
}

/// The row menu's item that shows a file in its folder, in the OS's file manager's name.
pub fn show_in_folder_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "Show in Explorer"
    } else if cfg!(target_os = "macos") {
        "Show in Finder"
    } else {
        "Show in folder"
    }
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
    if crate::ui_script::active() {
        crate::ui_script::await_pick(Box::new(move |paths, cx| {
            if let Some(p) = paths.into_iter().next() {
                then(p, cx)
            }
        }));
        return;
    }
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
    if crate::ui_script::active() {
        crate::ui_script::await_pick(Box::new(then));
        return;
    }
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
        let modal = crate::os_ui::modal(dialog, title, 560.);
        crate::ui_script::note_native_standin();
        modal
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

#[cfg(test)]
mod tests {
    use super::{PathStyle, display_path_in, os_path_in};

    /// Windows shows the documents folder as Explorer does: the full path, backslashes, no "~" (user 11-07: "~/Documents\noshape").
    #[test]
    fn windows_paths_are_full_and_backslashed() {
        let home = Some(r"C:\Users\ann");
        assert_eq!(display_path_in(r"C:\Users\ann\Documents\noshape", home, PathStyle::Windows), r"C:\Users\ann\Documents\noshape");
        assert_eq!(display_path_in(r"C:\Users\ann\OneDrive\Documents/noshape", home, PathStyle::Windows), r"C:\Users\ann\OneDrive\Documents\noshape");
        assert_eq!(display_path_in(r"\\?\C:\Users\ann\Documents\noshape", home, PathStyle::Windows), r"C:\Users\ann\Documents\noshape");
    }

    /// Explorer is handed the path without canonicalising's verbatim prefix (qa WIN-SHOW-IN-EXPLORER: "The parameter is
    /// incorrect."); other paths stay as they are.
    #[test]
    fn os_paths_drop_the_verbatim_prefix() {
        assert_eq!(os_path_in(r"\\?\C:\Users\ann\Documents\noshape\uip Bracket.noshape", PathStyle::Windows), r"C:\Users\ann\Documents\noshape\uip Bracket.noshape");
        assert_eq!(os_path_in(r"\\?\UNC\nas\cad\a.noshape", PathStyle::Windows), r"\\nas\cad\a.noshape");
        assert_eq!(os_path_in(r"C:\Users\ann\a.noshape", PathStyle::Windows), r"C:\Users\ann\a.noshape");
        assert_eq!(os_path_in(r"/home/ann/\\?\a", PathStyle::Unix), r"/home/ann/\\?\a");
    }

    /// macOS and Linux write the home folder as "~", and only a whole folder name of it.
    #[test]
    fn unix_paths_start_with_the_home_tilde() {
        assert_eq!(display_path_in("/Users/ann/Documents/noshape", Some("/Users/ann"), PathStyle::Unix), "~/Documents/noshape");
        assert_eq!(display_path_in("/home/ann/Documents/noshape", Some("/home/ann/"), PathStyle::Unix), "~/Documents/noshape");
        assert_eq!(display_path_in("/home/anna/Documents", Some("/home/ann"), PathStyle::Unix), "/home/anna/Documents");
        assert_eq!(display_path_in("/srv/cad/noshape", Some("/home/ann"), PathStyle::Unix), "/srv/cad/noshape");
        assert_eq!(display_path_in("/srv/cad/noshape", None, PathStyle::Unix), "/srv/cad/noshape");
    }
}
