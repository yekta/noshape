//! The display cache (lead 11-06: "how Onshape opens instantly"): a Part Studio's last regenerated display kept on
//! disk, so reopening the document shows its model at once while the real regeneration runs (view-only until that
//! lands, `cached_view`). Keyed by the document's content, the tab, its configuration and display fineness, and the
//! build (version, the executable itself, the OCCT patch set), so an edit or a new build never shows a stale model.
//! Kept in the user cache directory (`paths::cache_dir()/displays`), outside the document; at most [`CAP_BYTES`],
//! the least recently used evicted.

use noshape_model::display::StudioDisplay;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// File header: the format of what follows (zstd-compressed bincode of the display).
const MAGIC: &[u8; 8] = b"NSDISP01";
/// The cache's size bound.
const CAP_BYTES: u64 = 1 << 30;

/// Where the app keeps it.
pub(super) fn default_dir() -> PathBuf {
    crate::paths::cache_dir().join("displays")
}

/// This build: its version, the executable's size and time (a rebuild of the same version), the OCCT patch set.
fn build_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let exe = std::env::current_exe().ok().and_then(|p| std::fs::metadata(p).ok());
        let stamp = exe.map(|m| format!("{}:{:?}", m.len(), m.modified().ok())).unwrap_or_default();
        format!("{}|{stamp}|{}", env!("CARGO_PKG_VERSION"), noshape_kernel::OCCT_PATCHES)
    })
}

/// The cache key of tab `tab`'s display: the whole document (other tabs feed it: Feature Studios, derived
/// sources), its configuration values, whether it is meshed finer (View in high quality) and the build.
pub(super) fn key(doc: &noshape_model::Document, tab: impl std::fmt::Display, config: &impl serde::Serialize, fine: bool) -> Option<String> {
    let mut h = blake3::Hasher::new();
    serde_json::to_writer(&mut h, doc).ok()?;
    serde_json::to_writer(&mut h, config).ok()?;
    h.update(format!("|{tab}|{fine}|{}", build_id()).as_bytes());
    Some(h.finalize().to_hex().to_string())
}

fn path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.display"))
}

/// The display cached under `key`, if any (its use time refreshed for the eviction).
pub(super) fn load(dir: &Path, key: &str) -> Option<StudioDisplay> {
    let p = path(dir, key);
    let bytes = std::fs::read(&p).ok()?;
    let body = bytes.strip_prefix(MAGIC.as_slice())?;
    let raw = zstd::decode_all(body).ok()?;
    let mut d: StudioDisplay = bincode::deserialize(&raw).ok()?;
    // A pattern instance's shared mesh stands for its body's own (`SharedMesh::of` compares the two Arcs).
    let bodies: std::collections::HashMap<_, _> = d.bodies.iter().map(|b| (b.id.clone(), b.mesh.clone())).collect();
    d.shared_meshes.retain(|id, _| bodies.contains_key(id));
    for (id, s) in d.shared_meshes.iter_mut() {
        s.world = bodies[id].clone();
    }
    if let Ok(f) = std::fs::File::options().write(true).open(&p) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
    Some(d)
}

/// Keep `display` under `key` (written beside and renamed in, so a reader never sees half a file), then evict.
pub(super) fn store(dir: &Path, key: &str, display: &StudioDisplay) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    // Only the shared meshes that still stand for their body's.
    let mut d = display.clone();
    d.shared_meshes.retain(|id, s| d.bodies.iter().any(|b| &b.id == id && Arc::ptr_eq(&b.mesh, &s.world)));
    let raw = bincode::serialize(&d).map_err(std::io::Error::other)?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend(zstd::encode_all(raw.as_slice(), 1)?);
    let tmp = dir.join(format!("{key}.tmp{}", std::process::id()));
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path(dir, key))?;
    evict(dir, CAP_BYTES);
    Ok(())
}

/// Remove the least recently used entries until the cache fits `cap` bytes.
fn evict(dir: &Path, cap: u64) {
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, len, p) in files {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The least recently used go first, until the cache fits.
    #[test]
    fn eviction_keeps_the_newest_within_the_cap() {
        let d = std::env::temp_dir().join(format!("noshape-display-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for (i, name) in ["old", "mid", "new"].iter().enumerate() {
            let p = d.join(name);
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            let f = std::fs::File::options().write(true).open(&p).unwrap();
            f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000 + i as u64)).unwrap();
        }
        evict(&d, 250);
        let left: Vec<String> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(!left.contains(&"old".to_string()) && left.contains(&"mid".to_string()) && left.contains(&"new".to_string()), "{left:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
