//! Local document files the app keeps track of (moved from the app's `files`): the recent list, the watched folders,
//! the user name for version history, the registry of documents open in windows, and the other documents a document
//! can reference. Opening windows stays in the app (`files`).

use crate::file_dialogs::documents_dir;
use crate::paths::config_dir;
use gpui_kit::*;
use std::path::{Path, PathBuf};

pub fn new_document_path() -> PathBuf {
    let dir = documents_dir();
    (1..).map(|n| dir.join(format!("Untitled {n}.noshape"))).find(|p| !p.exists()).expect("free name")
}

pub use noshape_ops::doc_library::{add_recent, canonical, recent, remove_recent, replace_recent};

pub use noshape_ops::doc_library::{set_watched_folders, watched_folders};

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

/// Open document windows by canonical path (so opening twice focuses); the app's `files` keeps it current.
#[derive(Default)]
pub struct OpenDocs(pub std::collections::HashMap<PathBuf, AnyWindowHandle>);
impl Global for OpenDocs {}

/// Whether the document at `path` is open in a window.
pub fn is_document_open(path: &Path, cx: &App) -> bool {
    cx.try_global::<OpenDocs>().is_some_and(|o| o.0.contains_key(&canonical(path)))
}

/// .noshape documents besides `current`: recently opened, the documents
/// folder and the watched folders.
pub fn other_documents(current: &std::path::Path) -> Vec<std::path::PathBuf> {
    let canon = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let me = canon(current);
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let mut push = |p: std::path::PathBuf| {
        let c = canon(&p);
        if c != me && c.extension().is_some_and(|e| e == "noshape") && c.exists() && !out.contains(&c) {
            out.push(c);
        }
    };
    for p in recent() {
        push(p);
    }
    for dir in std::iter::once(documents_dir()).chain(watched_folders()) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                push(e.path());
            }
        }
    }
    out
}
