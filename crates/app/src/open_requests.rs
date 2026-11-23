//! Documents the OS asks the running app to open: macOS's open events (Finder, `open`, the dock), and a second start
//! with a document (Windows and Linux file opens, `noshape <file>`) handed over through the app's own server
//! (`noshape_ops::live::serve_app`). An open document's window comes forward; it isn't opened twice.

use crate::gpu::Gpu;
use gpui_kit::*;
use std::path::{Path, PathBuf};

/// The document a start was given (the first argument that isn't an option), unless the start does something else
/// with it (an update's reopen, a demo, a UI script).
pub fn document_arg(args: &[String]) -> Option<PathBuf> {
    if args.iter().any(|a| a == "--reopen" || a == "--update-failed" || a.starts_with("--demo")) {
        return None;
    }
    args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from)
}

/// A second start with a document: handed to the app already running. True when it took it (this start ends).
pub fn hand_over(args: &[String]) -> bool {
    if crate::ui_script::enabled() || crate::soak::cycles().is_some() {
        return false;
    }
    let Some(doc) = document_arg(args) else { return false };
    // Windows lets the running app bring its window forward (this start had the user's click).
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn AllowSetForegroundWindow(pid: u32) -> i32;
        }
        const ASFW_ANY: u32 = u32::MAX;
        unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    }
    noshape_ops::live::open_in_app(&noshape_ops::live::app_info_path(), &doc)
}

/// A `file://` URL as its path (macOS's open events); other URLs aren't documents.
pub fn url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    // "file://localhost/…" names the same file as "file:///…".
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push(h * 16 + l);
                i += 3;
            }
            (b, ..) => {
                out.push(b);
                i += 1;
            }
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

/// Serve the app for second starts: each path they hand over goes to `opens`.
pub fn serve(opens: async_channel::Sender<PathBuf>) -> Option<noshape_ops::live::Server> {
    let served = noshape_ops::live::serve_app(noshape_ops::live::app_info_path(), move |r| {
        let path = (r.tool == noshape_ops::live::APP_OPEN).then(|| r.args.get("path").and_then(|p| p.as_str()).map(PathBuf::from)).flatten();
        let answer = match path {
            Some(p) => opens.send_blocking(p).map_err(|_| noshape_ops::OpError::new("live", "the app is quitting")),
            None => Err(noshape_ops::OpError::invalid(format!("unknown call `{}`", r.tool))),
        };
        r.answer(answer.map(|_| noshape_ops::registry::OpOutput { value: serde_json::json!({}), images: Vec::new() }));
    });
    served.inspect_err(|e| log::warn!("can't take documents from other starts: {e}")).ok()
}

/// Open `path` as asked (its window comes forward when it is open already) and bring the app to the front; a file
/// that can't be opened lands on the Documents page with the error.
pub fn open(path: &Path, gpu: Gpu, cx: &mut App) {
    cx.activate(true);
    if let Err(e) = crate::files::open_document_window(path.to_path_buf(), None, gpu, cx) {
        log::error!("could not open {}: {e:#}", path.display());
        crate::start_window::show_with_error(format!("Could not open document: {e:#}"), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{document_arg, url_path};
    use std::path::PathBuf;

    #[test]
    fn finder_urls_are_paths() {
        assert_eq!(url_path("file:///Users/anna/Desktop/My%20part%20%C3%A7.noshape"), Some(PathBuf::from("/Users/anna/Desktop/My part ç.noshape")));
        assert_eq!(url_path("file://localhost/tmp/a.noshape"), Some(PathBuf::from("/tmp/a.noshape")));
        assert_eq!(url_path("https://cad.onshape.com/documents/1"), None);
    }

    #[test]
    fn only_a_plain_document_start_is_handed_over() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(document_arg(&args(&["/tmp/a.noshape"])), Some(PathBuf::from("/tmp/a.noshape")));
        assert_eq!(document_arg(&args(&[])), None);
        assert_eq!(document_arg(&args(&["--reopen", "/tmp/list"])), None);
        assert_eq!(document_arg(&args(&["--demo", "/tmp/a.noshape"])), None);
    }
}
