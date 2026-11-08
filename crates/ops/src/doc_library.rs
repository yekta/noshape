//! The documents page's files (the start window's Documents, op group `documents`): naming, moving, renaming, copying
//! and the details of `.noshape` files, and the user's document labels. The app's page and the agent API share them.

use anyhow::{Context as _, bail};
use std::path::{Path, PathBuf};

/// A document file's extension.
pub const EXT: &str = "noshape";

/// The SQLite write-ahead log beside a document.
pub fn wal_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push("-wal");
    PathBuf::from(s)
}

/// The files SQLite keeps beside a document.
pub fn sidecars(p: &Path) -> [PathBuf; 2] {
    let mut shm = p.as_os_str().to_os_string();
    shm.push("-shm");
    [wal_path(p), PathBuf::from(shm)]
}

/// File-name-safe version of a document name.
pub fn sanitize_name(name: &str) -> String {
    let s: String = name.trim().chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '-' } else { c }).collect();
    s.trim_matches('.').trim().to_string()
}

/// A path in `dir` for `name` that does not exist yet ("name", "name 2", ...).
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(format!("{name}.{EXT}"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{name} {n}.{EXT}"))).find(|p| !p.exists()).expect("free name")
}

/// Suggested name for a new document in `dir`.
pub fn next_untitled(dir: &Path) -> String {
    // Onshape: "Untitled document", then "Untitled document (1)", ...
    std::iter::once("Untitled document".to_string())
        .chain((1..).map(|n| format!("Untitled document ({n})")))
        .find(|n| !dir.join(format!("{n}.{EXT}")).exists())
        .expect("free name")
}

/// Close our write handle cleanly so SQLite folds the WAL back into the file.
fn settle(path: &Path) -> anyhow::Result<()> {
    let store = noshape_store::Document::open(path)?;
    let _ = store.checkpoint();
    drop(store);
    Ok(())
}

/// Rename a (closed) document: file name and stored document name.
/// Move a (closed) document and its sidecar files into `dir`.
pub fn move_document(path: &Path, dir: &Path) -> anyhow::Result<PathBuf> {
    let name = path.file_name().context("document has no file name")?;
    let dest = dir.join(name);
    if dest == path {
        return Ok(dest);
    }
    if dest.exists() {
        bail!("A document named \"{}\" already exists in this folder", name.to_string_lossy());
    }
    let _ = settle(path);
    if std::fs::rename(path, &dest).is_err() {
        // Across file systems: copy, then remove the original.
        std::fs::copy(path, &dest)?;
        std::fs::remove_file(path)?;
    }
    for (old, new) in sidecars(path).into_iter().zip(sidecars(&dest)) {
        if old.exists() {
            let _ = std::fs::rename(old, new);
        }
    }
    Ok(dest)
}

/// What the documents page's Details and Versions panels show about a
/// document (read from its store).
#[derive(Clone, Debug, Default)]
pub struct DocDetails {
    pub name: String,
    pub description: String,
    pub created_by: String,
    pub created: i64,
    pub modified_by: String,
    pub modified: i64,
    /// Workspaces and versions, newest first.
    pub history: Vec<HistoryRow>,
    /// The tabs of the default workspace: id, name, Search history ▾ kind.
    pub tabs: Vec<(noshape_model::document::TabId, String, &'static str)>,
    /// Document notes (markdown, the rail's Notes panel) and the Properties
    /// panel's Library label.
    pub notes: String,
    pub library_label: String,
}

/// A row of the documents page's Versions and history panel (live
/// documents-versions-history-panel: graph, name, Show changes…, author,
/// time; the tooltip is its description).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryRow {
    pub name: String,
    pub workspace: bool,
    pub author: String,
    pub time: i64,
    pub description: String,
    /// The graph's lane: the workspace it was made in (Main is 0).
    pub lane: usize,
    /// Its changes since the version before it, newest first (Show
    /// changes…).
    pub changes: Vec<HistoryChange>,
    /// A workspace made from a version: that version's row (the graph
    /// draws the branch from it).
    pub from: Option<usize>,
    /// The state it shows (None for Start's row before sorting).
    pub microversion: Option<noshape_store::MicroversionId>,
}

/// One change under a row's "Show changes…".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryChange {
    pub description: String,
    pub author: String,
    pub time: i64,
    pub merge: bool,
    /// The tabs it changed (Search history ▾'s Tab and Tab types).
    pub tabs: Vec<noshape_model::document::TabId>,
}

/// Store meta key of the Properties panel's Library label.
pub const LIBRARY_LABEL_KEY: &str = "library_label";

/// Save the documents page Properties panel's description and library label.
pub fn set_doc_properties(path: &Path, description: &str, library_label: &str) -> anyhow::Result<()> {
    let mut store = noshape_store::Document::open(path)?;
    store.set_description(description)?;
    store.set_meta(LIBRARY_LABEL_KEY, Some(library_label.as_bytes()).filter(|b| !b.is_empty()))?;
    Ok(())
}

pub fn doc_details(path: &Path) -> anyhow::Result<DocDetails> {
    let store = noshape_store::Document::open_read_only(path)?;
    let meta = store.meta()?;
    let graph = store.graph()?;
    let text = |k: &str| store.get_meta(k).ok().flatten().map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let (notes, library_label) = (text("notes"), text(LIBRARY_LABEL_KEY));
    let first = graph.microversions.iter().min_by_key(|m| m.seq);
    let last = graph.microversions.iter().max_by_key(|m| m.seq);
    let lanes: Vec<noshape_store::BranchId> = graph.branches.iter().filter(|b| !b.deleted).map(|b| b.id).collect();
    let lane_of = |b: noshape_store::BranchId| lanes.iter().position(|l| *l == b).unwrap_or(0);
    let by_id: std::collections::HashMap<_, _> = graph.microversions.iter().map(|m| (m.id, m)).collect();
    let marks: std::collections::HashSet<_> = graph.versions.iter().map(|v| v.microversion).collect();
    // The changes from `mv` back along first parents to the version before
    // it (a workspace still at a version has none).
    let changes = |mv, version: bool| {
        let mut out = Vec::new();
        let mut cur = Some(mv);
        while let Some(m) = cur.and_then(|c| by_id.get(&c)) {
            if ((m.id != mv || !version) && marks.contains(&m.id)) || m.parents.is_empty() || out.len() >= 200 {
                break;
            }
            out.push(HistoryChange { description: m.description.clone(), author: m.author.clone(), time: m.created, merge: m.is_merge(), tabs: crate::history_filter::changed_tabs(&store, m) });
            cur = m.parents.first().copied();
        }
        out
    };
    let mut history: Vec<HistoryRow> = Vec::new();
    for b in graph.branches.iter().filter(|b| !b.deleted) {
        let head = graph.microversions.iter().find(|m| m.id == b.head);
        let description = text(&format!("{}{}", crate::versions::WORKSPACE_DESCRIPTION_KEY, b.id));
        let time = head.map(|m| m.created).unwrap_or(b.created).max(b.created);
        history.push(HistoryRow { name: b.name.clone(), workspace: true, author: head.map(|m| m.author.clone()).unwrap_or_default(), time, description, lane: lane_of(b.id), changes: changes(b.head, false), from: None, microversion: Some(b.head) });
    }
    for v in &graph.versions {
        let lane = by_id.get(&v.microversion).map(|m| lane_of(m.branch)).unwrap_or(0);
        history.push(HistoryRow { name: v.name.clone(), workspace: false, author: v.author.clone(), time: v.created, description: v.description.clone(), lane, changes: changes(v.microversion, true), from: None, microversion: Some(v.microversion) });
    }
    if let Some(f) = first {
        history.push(HistoryRow { name: "Start".into(), author: f.author.clone(), time: f.created, microversion: Some(f.id), ..Default::default() });
    }
    history.sort_by(|a, b| b.time.cmp(&a.time).then(b.workspace.cmp(&a.workspace)));
    let tabs = store
        .default_branch()
        .and_then(|b| store.head(b))
        .and_then(|h| store.read_all(h))
        .ok()
        .and_then(|blobs| noshape_model::document::Document::from_blobs(&blobs).ok())
        .map(|d| d.ordered_tabs().into_iter().map(|t| (t.id, t.name.clone(), crate::history_filter::filter_kind(&t.content))).collect())
        .unwrap_or_default();
    // Where each other workspace branched off (a version's or Start's row).
    for b in graph.branches.iter().filter(|b| !b.deleted && !b.is_default) {
        let Some(from) = b.created_from else { continue };
        let at = history.iter().position(|r| !r.workspace && r.microversion == Some(from));
        if let Some(r) = history.iter_mut().find(|r| r.workspace && r.name == b.name) {
            r.from = at;
        }
    }
    Ok(DocDetails {
        name: if meta.name.is_empty() { path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() } else { meta.name },
        description: meta.description,
        created_by: first.map(|m| m.author.clone()).unwrap_or_default(),
        created: first.map(|m| m.created).unwrap_or(meta.created),
        modified_by: last.map(|m| m.author.clone()).unwrap_or_default(),
        modified: last.map(|m| m.created).unwrap_or(meta.modified),
        history,
        tabs,
        notes,
        library_label,
    })
}

pub fn rename_document(path: &Path, new_name: &str) -> anyhow::Result<PathBuf> {
    let clean = sanitize_name(new_name);
    if clean.is_empty() {
        bail!("Enter a name");
    }
    let dir = path.parent().context("document has no folder")?;
    let dest = dir.join(format!("{clean}.{EXT}"));
    {
        let mut store = noshape_store::Document::open(path)?;
        store.set_name(new_name.trim())?;
        let _ = store.checkpoint();
    }
    if dest != path {
        if dest.exists() {
            bail!("A document named \"{clean}\" already exists in this folder");
        }
        std::fs::rename(path, &dest)?;
        for (old, new) in sidecars(path).into_iter().zip(sidecars(&dest)) {
            if old.exists() {
                let _ = std::fs::rename(old, new);
            }
        }
    }
    Ok(dest)
}

/// Copy a document next to the original as "<name> copy".
pub fn duplicate_document(path: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let dir = path.parent().context("document has no folder")?;
    let copy_name = format!("{name} copy");
    let dest = unique_path(dir, &sanitize_name(&copy_name));
    // A closed document is opened writable so closing it leaves no side files.
    let src = if wal_path(path).exists() { noshape_store::Document::open_read_only(path)? } else { noshape_store::Document::open(path)? };
    src.backup_to(&dest)?;
    drop(src);
    let display = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(copy_name);
    let mut copy = noshape_store::Document::open(&dest)?;
    copy.set_name(&display)?;
    Ok(dest)
}

/// Copy a document under `name` into `dir` (☰ › Copy workspace…).
pub fn copy_document_named(path: &Path, dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let clean = sanitize_name(name);
    if clean.is_empty() {
        bail!("Enter a name");
    }
    let dest = unique_path(dir, &clean);
    let src = noshape_store::Document::open_read_only(path)?;
    src.backup_to(&dest)?;
    drop(src);
    let mut copy = noshape_store::Document::open(&dest)?;
    copy.set_name(name.trim())?;
    Ok(dest)
}

/// A new document under `name` in `dir` holding the document as it was at
/// `mv` (☰ › Copy version… on a version: the copy starts its own history at
/// that state).
pub fn copy_version_named(path: &Path, mv: noshape_store::MicroversionId, dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let clean = sanitize_name(name);
    if clean.is_empty() {
        bail!("Enter a name");
    }
    let dest = unique_path(dir, &clean);
    let blobs: Vec<(String, Vec<u8>)> = noshape_store::Document::open_read_only(path)?.read_all(mv)?.into_iter().collect();
    let mut copy = noshape_store::Document::create(&dest, &blobs)?;
    copy.set_name(name.trim())?;
    Ok(dest)
}

/// The Recently opened list (recent.json in the config dir), newest first, existing files only.
pub fn recent() -> Vec<PathBuf> {
    let f = noshape_scene::config_dir().join("recent.json");
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
    let _ = std::fs::write(noshape_scene::config_dir().join("recent.json"), serde_json::to_vec_pretty(list).unwrap_or_default());
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

/// The recent list as stored (moved or deleted files too).
pub fn recent_raw() -> Vec<PathBuf> {
    std::fs::read(noshape_scene::config_dir().join("recent.json")).ok().and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok()).unwrap_or_default()
}

/// Extra folders the start window lists `.noshape` files from (besides
/// the documents folder). Persisted in the config dir.
pub fn watched_folders() -> Vec<PathBuf> {
    std::fs::read(noshape_scene::config_dir().join("folders.json")).ok().and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok()).unwrap_or_default()
}

pub fn set_watched_folders(list: &[PathBuf]) {
    let _ = std::fs::write(noshape_scene::config_dir().join("folders.json"), serde_json::to_vec_pretty(list).unwrap_or_default());
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// A document renamed (Rename): its file and stored name; the recent list and its labels follow.
pub fn rename_doc(path: &Path, name: &str) -> anyhow::Result<PathBuf> {
    // (Canonical before it moves: the old path can't be resolved after.)
    let from = canon(path);
    let dest = rename_document(path, name)?;
    followed(&from, &dest);
    Ok(dest)
}

/// A document moved into `dir` (Move to…); the recent list and its labels follow.
pub fn move_doc(path: &Path, dir: &Path) -> anyhow::Result<PathBuf> {
    let from = canon(path);
    let dest = move_document(path, dir)?;
    followed(&from, &dest);
    Ok(dest)
}

fn followed(from: &Path, to: &Path) {
    replace_recent(from, to);
    let mut labels = Labels::load();
    labels.moved(from, to);
    labels.save();
}

/// Create ▾ › Folder…: `name` in `place`, Onshape's messages when it can't.
pub fn create_folder(place: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let clean = sanitize_name(name);
    if clean.is_empty() {
        bail!("Enter a name for the folder");
    }
    let path = place.join(&clean);
    if path.exists() {
        bail!("\"{clean}\" already exists in this folder");
    }
    std::fs::create_dir_all(&path).with_context(|| format!("Could not create the folder {}", path.display()))?;
    Ok(path)
}

/// The free "Untitled folder" name in `place` (the New folder prompt's).
pub fn next_untitled_folder(place: &Path) -> String {
    (0..).map(|i| if i == 0 { "Untitled folder".to_string() } else { format!("Untitled folder ({i})") }).find(|n| !place.join(n).exists()).unwrap_or_default()
}

/// A folder renamed in place (folder menu › Rename).
pub fn rename_folder(folder: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let clean = sanitize_name(name);
    if clean.is_empty() {
        bail!("Enter a name for the folder");
    }
    let parent = folder.parent().context("the folder has no parent")?;
    let dest = parent.join(&clean);
    if dest == folder {
        return Ok(dest);
    }
    if dest.exists() {
        bail!("\"{clean}\" already exists in this folder");
    }
    std::fs::rename(folder, &dest).context("Rename failed")?;
    folder_moved(&canon(folder), &canon(&dest));
    Ok(dest)
}

/// A folder moved into `dir` (folder menu › Move to…; not into itself).
pub fn move_folder(folder: &Path, dir: &Path) -> anyhow::Result<PathBuf> {
    let file_name = folder.file_name().context("the folder has no name")?;
    let dest = dir.join(file_name);
    if dest == folder {
        return Ok(dest);
    }
    if canon(dir).starts_with(canon(folder)) || dest.exists() {
        bail!("\"{}\" can't be moved there", file_name.to_string_lossy());
    }
    let old = canon(folder);
    std::fs::rename(folder, &dest).context("Move failed")?;
    folder_moved(&old, &canon(&dest));
    Ok(dest)
}

/// What follows a folder's move: the recent entries and labels of the documents in it, a watched folder.
pub fn folder_moved(old: &Path, new: &Path) {
    let moved = |p: &Path| p.strip_prefix(old).ok().map(|rest| new.join(rest));
    for r in recent_raw() {
        if let Some(dest) = moved(&r) {
            replace_recent(&r, &dest);
        }
    }
    let mut labels = Labels::load();
    let keys: Vec<String> = labels.assign.keys().cloned().collect();
    for k in keys {
        if let Some(dest) = moved(Path::new(&k))
            && let Some(l) = labels.assign.remove(&k)
        {
            labels.assign.insert(dest.to_string_lossy().to_string(), l);
        }
    }
    labels.save();
    let list: Vec<PathBuf> = watched_folders().into_iter().map(|w| if canon(&w) == old { new.to_path_buf() } else { w }).collect();
    set_watched_folders(&list);
}

/// Send to trash: the document or folder goes to the Trash; it leaves the recent list (a folder: its documents) and
/// the watched folders.
pub fn send_to_trash(path: &Path, name: &str, folder: bool) -> anyhow::Result<crate::doc_trash::TrashItem> {
    let mut trash = crate::doc_trash::Trash::load();
    let item = trash.send(path, name, folder)?;
    for r in recent_raw() {
        if canon(&r) == item.original || (folder && canon(&r).starts_with(&item.original)) {
            remove_recent(&r);
        }
    }
    if folder {
        let mut list = watched_folders();
        list.retain(|f| canon(f) != item.original);
        set_watched_folders(&list);
    }
    Ok(item)
}

/// Trash › Restore: back where it was; a restored document's Modified time is the restore time (live
/// documents/86), a folder from outside the documents folder is listed again.
pub fn restore(id: &str, documents: &Path) -> anyhow::Result<(PathBuf, crate::doc_trash::TrashItem)> {
    let mut trash = crate::doc_trash::Trash::load();
    let item = trash.get(id).cloned().context("No such item in the trash")?;
    let dest = trash.restore(id)?;
    if !item.folder
        && let Ok(mut store) = noshape_store::Document::open(&dest)
    {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let _ = store.set_meta("modified", Some(now.to_string().as_bytes()));
        let _ = store.checkpoint();
    }
    if item.folder && !item.original.starts_with(canon(documents)) {
        let mut list = watched_folders();
        if !list.iter().any(|w| item.original.starts_with(canon(w))) {
            list.push(dest.clone());
            set_watched_folders(&list);
        }
    }
    Ok((dest, item))
}

/// Onshape's document labels (per user, kept in the config dir): the label
/// names and the labels of each document (by canonical path).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Labels {
    pub labels: Vec<String>,
    pub assign: std::collections::BTreeMap<String, Vec<String>>,
}

impl Labels {
    fn file() -> PathBuf {
        noshape_scene::config_dir().join("labels.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::write(Self::file(), serde_json::to_vec_pretty(self).unwrap_or_default());
    }

    fn key(path: &Path) -> String {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().to_string()
    }

    pub fn has(&self, path: &Path, label: &str) -> bool {
        self.assign.get(&Self::key(path)).is_some_and(|l| l.iter().any(|x| x == label))
    }

    /// Create a label (names are unique, case-insensitively).
    pub fn create(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.labels.iter().any(|l| l.eq_ignore_ascii_case(name)) {
            return false;
        }
        self.labels.push(name.to_string());
        self.labels.sort_by_key(|l| l.to_lowercase());
        true
    }

    pub fn toggle(&mut self, path: &Path, label: &str) {
        let list = self.assign.entry(Self::key(path)).or_default();
        match list.iter().position(|l| l == label) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(label.to_string()),
        }
    }

    /// Keep labels when a document moves or is renamed.
    pub fn moved(&mut self, from: &Path, to: &Path) {
        if let Some(l) = self.assign.remove(&Self::key(from)) {
            self.assign.insert(Self::key(to), l);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Documents › Versions and history: each row's changes since the
    /// version before it, the description, and where a workspace branched.
    #[test]
    fn versions_panel_rows() {
        let dir = std::env::temp_dir().join(format!("noshape-history-rows-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.noshape");
        let mut store = noshape_store::Document::create(&path, &[("a", b"0")]).unwrap();
        let main = store.default_branch().unwrap();
        store.commit(main, &[("a", Some(b"1"))], "one").unwrap();
        let v1 = store.commit(main, &[("a", Some(b"2"))], "two").unwrap();
        store.create_version(v1, "V1", "First cut").unwrap();
        store.create_branch(v1, "experiment").unwrap();
        store.commit(main, &[("a", Some(b"3"))], "three").unwrap();
        drop(store);
        let h = doc_details(&path).unwrap().history;
        let row = |n: &str| h.iter().find(|r| r.name == n).unwrap().clone();
        let descs = |r: HistoryRow| r.changes.into_iter().map(|c| c.description).collect::<Vec<_>>();
        assert_eq!(descs(row("Main")), ["three"]);
        assert_eq!(descs(row("V1")), ["two", "one"]);
        assert!(row("experiment").changes.is_empty(), "still at V1");
        assert_eq!(row("V1").description, "First cut");
        let at = h.iter().position(|r| r.name == "V1");
        assert_eq!(row("experiment").from, at);
        assert_eq!((row("Main").lane, row("experiment").lane), (0, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ☰ › Copy version…: the copy holds the version's state, not the workspace's.
    #[test]
    fn copy_version_holds_the_versions_state() {
        let dir = std::env::temp_dir().join(format!("noshape-copy-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.noshape");
        let mut store = noshape_store::Document::create(&path, &[("a", b"one")]).unwrap();
        let branch = store.default_branch().unwrap();
        let v1 = store.head(branch).unwrap();
        store.commit(branch, &[("a", Some(b"two"))], "edit").unwrap();
        drop(store);
        let dest = copy_version_named(&path, v1, &dir, "At V1").unwrap();
        let copy = noshape_store::Document::open_read_only(&dest).unwrap();
        let head = copy.head(copy.default_branch().unwrap()).unwrap();
        assert_eq!(copy.read(head, "a").unwrap().as_deref(), Some(&b"one"[..]));
        assert_eq!(dest.file_stem().unwrap(), "At V1");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
