//! Document library for the start window: finds `.noshape` files (recent
//! list, documents folder, watched folders), reads their metadata and
//! thumbnails through read-only store handles, and caches decoded images
//! keyed by file modification stamps. The file operations and labels are
//! `noshape_ops::doc_library`, the Trash `noshape_ops::doc_trash`.

use anyhow::bail;
use gpui_kit::RenderImage;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

pub use crate::library::{DocEntry, EXT, decode_thumbnail};
pub use noshape_ops::doc_library::*;

#[derive(Clone)]
struct Cached {
    stamp: (Option<SystemTime>, Option<SystemTime>, u64),
    name: String,
    modified: i64,
    modified_by: String,
    owner: String,
    description: String,
    workspace: String,
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
        let mut folders = vec![crate::file_dialogs::documents_dir()];
        for f in crate::doc_files::watched_folders() {
            if !folders.contains(&f) {
                folders.push(f);
            }
        }
        Self { recent: crate::doc_files::recent(), folders, trash: crate::start_trash::Trash::load().documents() }
    }
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
                workspace: cached.workspace.clone(),
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
    workspace: String,
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
        let workspace = doc.list_branches()?.into_iter().next().map(|b| b.name).unwrap_or_default();
        return Ok(MetaRead { name: meta.name, description: meta.description, workspace, modified: meta.modified, owner, modified_by, thumbnail: meta.thumbnail });
    }
    use rusqlite::{Connection, OpenFlags, OptionalExtension as _};
    let mut uri = url_escape(&path.to_string_lossy());
    uri.insert_str(0, "file:");
    uri.push_str("?immutable=1");
    let conn = Connection::open_with_flags(&uri, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    let app_id: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    if app_id != i64::from(noshape_store::APPLICATION_ID) {
        bail!("not a Noshape document");
    }
    let get = |key: &str| -> anyhow::Result<Option<Vec<u8>>> { Ok(conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?) };
    let text = |v: Option<Vec<u8>>| v.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let (owner, modified_by) = noshape_store::owner_and_last_author(&conn)?;
    let workspace: String = conn.query_row("SELECT name FROM branches WHERE is_default = 1 LIMIT 1", [], |r| r.get(0)).optional()?.unwrap_or_default();
    Ok(MetaRead { name: text(get("name")?), description: text(get("description")?), workspace, modified: text(get("modified")?).parse().unwrap_or(0), owner, modified_by, thumbnail: get("thumbnail")? })
}

/// A document file's thumbnail (Reference manager rows).
pub fn file_thumbnail(path: &Path) -> Option<Arc<RenderImage>> {
    read_meta(path).ok()?.thumbnail.as_deref().and_then(decode_thumbnail)
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
            workspace: meta.workspace,
            thumbnail: meta.thumbnail.as_deref().and_then(decode_thumbnail),
            broken: false,
        },
        Err(e) => {
            log::warn!("cannot read {}: {e}", path.display());
            Cached { stamp, name: stem, modified: fallback_ms, modified_by: String::new(), owner: String::new(), description: String::new(), workspace: String::new(), thumbnail: None, broken: true }
        }
    }
}

/// Onshape's time stamps (live documents list and versions panel): "1:15 PM
/// Today", "9:02 AM Yesterday", "Oct 12", "Oct 12, 2024", in local time.
pub fn format_onshape_time(ms: i64) -> String {
    use chrono::{Datelike as _, Local, TimeZone as _};
    let Some(t) = Local.timestamp_millis_opt(ms).single() else { return "—".into() };
    let today = Local::now().date_naive();
    // Preferences › Units › Time format.
    let clock = if crate::user_units::time_24h() { t.format("%H:%M") } else { t.format("%-I:%M %p") }.to_string();
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
