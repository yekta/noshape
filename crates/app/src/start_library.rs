//! Document library for the start window: finds `.noshape` files (recent
//! list, documents folder, watched folders), reads their metadata and
//! thumbnails through read-only store handles, and caches decoded images
//! keyed by file modification stamps. Also the file operations of the
//! start window (rename, duplicate, trash).

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
    pub size: u64,
    pub thumbnail: Option<Arc<RenderImage>>,
    /// Position in the recent list.
    pub recent_rank: Option<usize>,
    /// Could not be read as a noshape document.
    pub broken: bool,
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
    /// Thumbnails replaced in the cache (to free from the window's atlas).
    pub dropped: Vec<Arc<RenderImage>>,
}

/// Where to look.
#[derive(Clone)]
pub struct ScanInput {
    pub recent: Vec<PathBuf>,
    pub folders: Vec<PathBuf>,
}

impl ScanInput {
    pub fn current() -> Self {
        let mut folders = vec![crate::files::documents_dir()];
        for f in crate::files::watched_folders() {
            if !folders.contains(&f) {
                folders.push(f);
            }
        }
        Self { recent: crate::files::recent(), folders }
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

/// `.noshape` files in `dir` and its subfolders (a few levels deep).
fn list_dir(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
        if hidden {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            if depth > 0 {
                list_dir(&p, depth - 1, out);
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
        let mut paths: Vec<PathBuf> = Vec::new();
        for f in &input.folders {
            list_dir(f, 3, &mut paths);
        }
        let mut seen = std::collections::HashSet::new();
        let mut unique = Vec::new();
        for p in input.recent.iter().cloned().chain(paths) {
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
                size: md.len() + std::fs::metadata(wal_path(&path)).map(|m| m.len()).unwrap_or(0),
                thumbnail: cached.thumbnail.clone(),
                recent_rank: recent.iter().position(|r| *r == path),
                broken: cached.broken,
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
        ScanResult { docs, dropped }
    }
}

struct MetaRead {
    name: String,
    modified: i64,
    thumbnail: Option<Vec<u8>>,
}

/// Read name, modified time and thumbnail. A document that is closed (no
/// WAL file) is read with SQLite's `immutable` flag, which creates no
/// `-wal`/`-shm` side files (a plain read-only handle would leave them
/// behind); an open document goes through a regular read-only store handle.
fn read_meta(path: &Path) -> anyhow::Result<MetaRead> {
    if wal_path(path).exists() {
        let meta = noshape_store::Document::open_read_only(path)?.meta()?;
        return Ok(MetaRead { name: meta.name, modified: meta.modified, thumbnail: meta.thumbnail });
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
    Ok(MetaRead { name: text(get("name")?), modified: text(get("modified")?).parse().unwrap_or(0), thumbnail: get("thumbnail")? })
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
            thumbnail: meta.thumbnail.as_deref().and_then(decode_thumbnail),
            broken: false,
        },
        Err(e) => {
            log::warn!("cannot read {}: {e}", path.display());
            Cached { stamp, name: stem, modified: fallback_ms, thumbnail: None, broken: true }
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

/// Move a (closed) document to the system trash.
pub fn trash_document(path: &Path) -> anyhow::Result<()> {
    let _ = settle(path);
    trash::delete(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    for s in sidecars(path) {
        if s.exists() {
            let _ = std::fs::remove_file(s);
        }
    }
    Ok(())
}

/// "just now", "5 min ago", "yesterday", "Mar 4", "Mar 4, 2024".
pub fn format_modified(ms: i64) -> String {
    if ms <= 0 {
        return "—".into();
    }
    let now = to_ms(SystemTime::now());
    let s = ((now - ms) / 1000).max(0);
    match s {
        0..=59 => return "just now".into(),
        60..=3599 => return format!("{} min ago", s / 60),
        3600..=86399 => return format!("{} h ago", s / 3600),
        86400..=172799 => return "yesterday".into(),
        172800..=604799 => return format!("{} days ago", s / 86400),
        _ => {}
    }
    let (y, m, d) = civil_from_days(ms.div_euclid(86_400_000));
    let (ny, _, _) = civil_from_days(now.div_euclid(86_400_000));
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mon = MONTHS[(m as usize).saturating_sub(1).min(11)];
    if y == ny { format!("{mon} {d}") } else { format!("{mon} {d}, {y}") }
}

/// Days since 1970-01-01 to (year, month, day) (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Upper-case the first letter (for column cells).
pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

pub fn format_size(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{:.0} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
        assert_eq!(sanitize_name(" a/b:c "), "a-b-c");
    }
}
