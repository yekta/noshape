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

pub fn canonical(p: &Path) -> PathBuf {
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
