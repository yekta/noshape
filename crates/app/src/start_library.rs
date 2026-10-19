//! Document library for the start window: finds `.noshape` files (recent
//! list, documents folder, watched folders), reads their metadata and
//! thumbnails through read-only store handles, and caches decoded images
//! keyed by file modification stamps. Also the file operations of the
//! start window (rename, duplicate, move; the Trash is start_trash).

use anyhow::{Context as _, bail};
use gpui_kit::RenderImage;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

pub const EXT: &str = "noshape";

#[derive(Clone)]
pub struct DocEntry {
    pub path: PathBuf,
    /// Display name (store metadata name, else the file stem).
    pub name: String,
    /// Last change (unix ms): the store's `modified`, else the file mtime.
    pub modified: i64,
    /// Author of the latest change ("Modified by").
    pub modified_by: String,
    /// Author of the first change ("Owned by").
    pub owner: String,
    /// Document description (the "Description" column).
    pub description: String,
    pub thumbnail: Option<Arc<RenderImage>>,
    /// Position in the recent list.
    pub recent_rank: Option<usize>,
    /// Could not be read as a noshape document.
    pub broken: bool,
    /// In the Trash (`path` is the trashed copy).
    pub trashed: bool,
}

impl DocEntry {
    pub fn folder(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new(""))
    }
}

#[derive(Clone)]
struct Cached {
    stamp: (Option<SystemTime>, Option<SystemTime>, u64),
    name: String,
    modified: i64,
    modified_by: String,
    owner: String,
    description: String,
    thumbnail: Option<Arc<RenderImage>>,
    broken: bool,
}

/// Decoded metadata cache, shared with the background scanner.
#[derive(Clone, Default)]
pub struct Library {
    cache: Arc<Mutex<HashMap<PathBuf, Cached>>>,
}

pub struct ScanResult {
    pub docs: Vec<DocEntry>,
    /// Folders under the scanned places (canonical; folder tiles).
    pub dirs: Vec<PathBuf>,
    /// Thumbnails replaced in the cache (to free from the window's atlas).
    pub dropped: Vec<Arc<RenderImage>>,
}

/// Where to look.
#[derive(Clone)]
pub struct ScanInput {
    pub recent: Vec<PathBuf>,
    pub folders: Vec<PathBuf>,
    /// Trashed documents (start_trash).
    pub trash: Vec<PathBuf>,
}

impl ScanInput {
    pub fn current() -> Self {
        let mut folders = vec![crate::files::documents_dir()];
        for f in crate::files::watched_folders() {
            if !folders.contains(&f) {
                folders.push(f);
            }
        }
        Self { recent: crate::files::recent(), folders, trash: crate::start_trash::Trash::load().documents() }
    }
}

fn wal_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push("-wal");
    PathBuf::from(s)
}

fn sidecars(p: &Path) -> [PathBuf; 2] {
    let mut shm = p.as_os_str().to_os_string();
    shm.push("-shm");
    [wal_path(p), PathBuf::from(shm)]
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn to_ms(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// `.noshape` files in `dir` and its subfolders (a few levels deep), and
/// those subfolders.
fn list_dir(dir: &Path, depth: usize, out: &mut Vec<PathBuf>, dirs: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
        if hidden {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            dirs.push(p.canonicalize().unwrap_or_else(|_| p.clone()));
            if depth > 0 {
                list_dir(&p, depth - 1, out, dirs);
            }
        } else if p.extension().is_some_and(|x| x == EXT) {
            out.push(p);
        }
    }
}

fn decode_thumbnail(png: &[u8]) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    // GPUI render images are BGRA. The white background becomes
    // transparent so thumbnails sit on hovered and selected rows like
    // Onshape's.
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[0] >= 250 && px[1] >= 250 && px[2] >= 250 {
            *px = [0; 4];
        }
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}

impl Library {
    /// Scan the file system (blocking; run on a background thread).
    pub fn scan(&self, input: &ScanInput) -> ScanResult {
        let (mut paths, mut dirs): (Vec<PathBuf>, Vec<PathBuf>) = (Vec::new(), Vec::new());
        for f in &input.folders {
            list_dir(f, 3, &mut paths, &mut dirs);
        }
        dirs.sort();
        dirs.dedup();
        let mut seen = std::collections::HashSet::new();
        let mut unique = Vec::new();
        let trash: Vec<PathBuf> = input.trash.iter().map(|p| p.canonicalize().unwrap_or_else(|_| p.clone())).collect();
        for p in input.recent.iter().cloned().chain(paths).chain(trash.iter().cloned()) {
            let c = p.canonicalize().unwrap_or(p);
            if seen.insert(c.clone()) {
                unique.push(c);
            }
        }
        let recent: Vec<PathBuf> = input.recent.iter().map(|p| p.canonicalize().unwrap_or_else(|_| p.clone())).collect();
        let mut dropped = Vec::new();
        let mut docs = Vec::with_capacity(unique.len());
        let mut fresh: HashMap<PathBuf, Cached> = HashMap::new();
        let old = self.cache.lock().clone();
        for path in unique {
            let Ok(md) = std::fs::metadata(&path) else { continue };
            if !md.is_file() {
                continue;
            }
            let stamp = (md.modified().ok(), mtime(&wal_path(&path)), md.len());
            let cached = match old.get(&path) {
                Some(c) if c.stamp == stamp => c.clone(),
                prev => {
                    let c = read_entry(&path, stamp, md.modified().ok());
                    if let Some(img) = prev.and_then(|p| p.thumbnail.clone()) {
                        dropped.push(img);
                    }
                    c
                }
            };
            docs.push(DocEntry {
                name: cached.name.clone(),
                modified: cached.modified,
                modified_by: cached.modified_by.clone(),
                owner: cached.owner.clone(),
                description: cached.description.clone(),
                thumbnail: cached.thumbnail.clone(),
                recent_rank: recent.iter().position(|r| *r == path),
                broken: cached.broken,
                trashed: trash.contains(&path),
                path: path.clone(),
            });
            fresh.insert(path, cached);
        }
        // Files that disappeared.
        for (p, c) in &old {
            if !fresh.contains_key(p)
                && let Some(img) = &c.thumbnail {
                    dropped.push(img.clone());
                }
        }
        *self.cache.lock() = fresh;
        ScanResult { docs, dirs, dropped }
    }
}

struct MetaRead {
    name: String,
    description: String,
    modified: i64,
    owner: String,
    modified_by: String,
    thumbnail: Option<Vec<u8>>,
}

/// Read name, modified time and thumbnail. A document that is closed (no
/// WAL file) is read with SQLite's `immutable` flag, which creates no
/// `-wal`/`-shm` side files (a plain read-only handle would leave them
/// behind); an open document goes through a regular read-only store handle.
fn read_meta(path: &Path) -> anyhow::Result<MetaRead> {
    if wal_path(path).exists() {
        let doc = noshape_store::Document::open_read_only(path)?;
        let (meta, (owner, modified_by)) = (doc.meta()?, doc.owner_and_last_author()?);
        return Ok(MetaRead { name: meta.name, description: meta.description, modified: meta.modified, owner, modified_by, thumbnail: meta.thumbnail });
    }
    use rusqlite::{Connection, OpenFlags, OptionalExtension as _};
    let mut uri = url_escape(&path.to_string_lossy());
    uri.insert_str(0, "file:");
    uri.push_str("?immutable=1");
    let conn = Connection::open_with_flags(&uri, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    let app_id: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    if app_id != i64::from(noshape_store::APPLICATION_ID) {
        bail!("not a noshape document");
    }
    let get = |key: &str| -> anyhow::Result<Option<Vec<u8>>> { Ok(conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?) };
    let text = |v: Option<Vec<u8>>| v.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let (owner, modified_by) = noshape_store::owner_and_last_author(&conn)?;
    Ok(MetaRead { name: text(get("name")?), description: text(get("description")?), modified: text(get("modified")?).parse().unwrap_or(0), owner, modified_by, thumbnail: get("thumbnail")? })
}

/// Percent-encode the characters that are special in SQLite URIs.
fn url_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' | '?' | '#' => out.push_str(&format!("%{:02X}", c as u32)),
            _ => out.push(c),
        }
    }
    out
}

fn read_entry(path: &Path, stamp: (Option<SystemTime>, Option<SystemTime>, u64), file_mtime: Option<SystemTime>) -> Cached {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let fallback_ms = file_mtime.map(to_ms).unwrap_or(0);
    match read_meta(path) {
        Ok(meta) => Cached {
            stamp,
            name: if meta.name.trim().is_empty() { stem } else { meta.name },
            modified: if meta.modified > 0 { meta.modified } else { fallback_ms },
            modified_by: meta.modified_by,
            owner: meta.owner,
            description: meta.description,
            thumbnail: meta.thumbnail.as_deref().and_then(decode_thumbnail),
            broken: false,
        },
        Err(e) => {
            log::warn!("cannot read {}: {e}", path.display());
            Cached { stamp, name: stem, modified: fallback_ms, modified_by: String::new(), owner: String::new(), description: String::new(), thumbnail: None, broken: true }
        }
    }
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
    /// Workspaces and versions, newest first: (name, is_workspace, author, time).
    pub history: Vec<(String, bool, String, i64)>,
}

pub fn doc_details(path: &Path) -> anyhow::Result<DocDetails> {
    let store = noshape_store::Document::open_read_only(path)?;
    let meta = store.meta()?;
    let graph = store.graph()?;
    let first = graph.microversions.iter().min_by_key(|m| m.seq);
    let last = graph.microversions.iter().max_by_key(|m| m.seq);
    let mut history: Vec<(String, bool, String, i64)> = Vec::new();
    for b in graph.branches.iter().filter(|b| !b.deleted) {
        let head = graph.microversions.iter().find(|m| m.id == b.head);
        history.push((b.name.clone(), true, head.map(|m| m.author.clone()).unwrap_or_default(), head.map(|m| m.created).unwrap_or(b.created).max(b.created)));
    }
    for v in &graph.versions {
        history.push((v.name.clone(), false, v.author.clone(), v.created));
    }
    if let Some(f) = first {
        history.push(("Start".into(), false, f.author.clone(), f.created));
    }
    history.sort_by(|a, b| b.3.cmp(&a.3).then(b.1.cmp(&a.1)));
    Ok(DocDetails {
        name: if meta.name.is_empty() { path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() } else { meta.name },
        description: meta.description,
        created_by: first.map(|m| m.author.clone()).unwrap_or_default(),
        created: first.map(|m| m.created).unwrap_or(meta.created),
        modified_by: last.map(|m| m.author.clone()).unwrap_or_default(),
        modified: last.map(|m| m.created).unwrap_or(meta.modified),
        history,
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

/// Copy a document under `name` next to it (☰ › Copy workspace…).
pub fn copy_document_named(path: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let dir = path.parent().context("document has no folder")?;
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

/// Onshape's time stamps (live documents list and versions panel): "1:15 PM
/// Today", "9:02 AM Yesterday", "Oct 12", "Oct 12, 2024", in local time.
pub fn format_onshape_time(ms: i64) -> String {
    use chrono::{Datelike as _, Local, TimeZone as _};
    let Some(t) = Local.timestamp_millis_opt(ms).single() else { return "—".into() };
    let today = Local::now().date_naive();
    let clock = t.format("%-I:%M %p").to_string();
    match (today - t.date_naive()).num_days() {
        0 => format!("{clock} Today"),
        1 => format!("{clock} Yesterday"),
        _ if t.year() == today.year() => t.format("%b %-d").to_string(),
        _ => t.format("%b %-d, %Y").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_times() {
        assert_eq!(sanitize_name(" a/b:c "), "a-b-c");
        let now = to_ms(SystemTime::now());
        assert!(format_onshape_time(now).ends_with(" Today"));
        assert!(format_onshape_time(now - 400 * 86_400_000).contains(", "));
    }
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
        crate::files::config_dir().join("labels.json")
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
