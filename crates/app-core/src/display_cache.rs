//! The display cache (lead 11-06: "how Onshape opens instantly"): a tab's last displayed model kept on disk, so
//! reopening the document shows it at once while the real regeneration runs (view-only until that lands). Keyed by
//! the whole document's content, what the tab adds (its id, configuration, …) and the build (version, the executable
//! itself, the OCCT patch set), so an edit or a new build never shows a stale model. Kept in the user cache directory
//! (`paths::cache_dir()/displays`), outside the document; at most [`CAP_BYTES`], the least recently used evicted.
//! The studios keep their own snapshot types (Part Studio display, assembly occurrences, drawing sheets).

use gpui_kit::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// File header: the format of what follows (zstd-compressed bincode).
const MAGIC: &[u8; 8] = b"NSDISP02";
/// The cache's size bound.
const CAP_BYTES: u64 = 1 << 30;

/// The message while an action waits for the regeneration.
pub const WAITING: &str = "Regenerating the model: this opens as soon as it is ready.";

static ENABLED: AtomicBool = AtomicBool::new(false);

/// The app keeps and uses the display cache (called once at startup; tests and the parity driver don't).
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Where the app keeps it.
pub fn default_dir() -> PathBuf {
    crate::paths::cache_dir().join("displays")
}

/// This build: its version, the executable's size and time (a rebuild of the same version), the OCCT patch set.
pub(crate) fn build_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let exe = std::env::current_exe().ok().and_then(|p| std::fs::metadata(p).ok());
        let stamp = exe.map(|m| format!("{}:{:?}", m.len(), m.modified().ok())).unwrap_or_default();
        format!("{}|{stamp}|{}", env!("CARGO_PKG_VERSION"), noshape_ops::OCCT_PATCHES)
    })
}

/// The cache key of a tab's display: the whole document (other tabs feed it: Part Studios, Feature Studios, derived
/// sources), what the tab adds (`extra`: its id, configuration, the display's level) and the build.
pub fn key(doc: &noshape_model::Document, extra: &impl Serialize) -> Option<String> {
    let mut h = blake3::Hasher::new();
    serde_json::to_writer(&mut h, doc).ok()?;
    serde_json::to_writer(&mut h, extra).ok()?;
    h.update(build_id().as_bytes());
    Some(h.finalize().to_hex().to_string())
}

fn path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.display"))
}

/// What is cached under `key`, if anything (its use time refreshed for the eviction).
pub fn load<T: DeserializeOwned>(dir: &Path, key: &str) -> Option<T> {
    let p = path(dir, key);
    let bytes = std::fs::read(&p).ok()?;
    let body = bytes.strip_prefix(MAGIC.as_slice())?;
    let raw = zstd::decode_all(body).ok()?;
    let value = bincode::deserialize(&raw).ok()?;
    if let Ok(f) = std::fs::File::options().write(true).open(&p) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
    Some(value)
}

/// Keep `value` under `key` (written beside and renamed in, so a reader never sees half a file), then evict.
pub fn store<T: Serialize>(dir: &Path, key: &str, value: &T) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let raw = bincode::serialize(value).map_err(std::io::Error::other)?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend(zstd::encode_all(raw.as_slice(), 1)?);
    let tmp = dir.join(format!("{key}.tmp{}", std::process::id()));
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path(dir, key))?;
    evict(dir, CAP_BYTES);
    Ok(())
}

/// Whether `key` is cached.
pub fn contains(dir: &Path, key: &str) -> bool {
    path(dir, key).exists()
}

/// The spinner in a `size` px box (the overlay's 41 px; a list row's smaller).
pub fn spinner(size: f32) -> impl IntoElement {
    let scale = size / 41.;
    div().size(px(size)).flex_none().with_animation(SharedString::from(format!("pending-spinner-{size}")), Animation::new(Duration::from_millis(900)).repeat(), move |d, t| {
        d.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let c = bounds.center();
                    let r = 13.5_f32 * scale;
                    let ring = |from: f32, to: f32, color: Rgba, window: &mut Window| {
                        let steps = 48;
                        let mut pb = PathBuilder::stroke(px((3. * scale).max(1.5)));
                        for i in 0..=steps {
                            let a = from + (to - from) * i as f32 / steps as f32;
                            let p = point(c.x + px(r * a.cos()), c.y + px(r * a.sin()));
                            if i == 0 { pb.move_to(p) } else { pb.line_to(p) }
                        }
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, color);
                        }
                    };
                    let tau = std::f32::consts::TAU;
                    ring(0., tau, rgb(0xdddddd), window);
                    let start = t * tau - tau / 4.;
                    ring(start, start + tau / 4., rgb(0x1651b0), window);
                },
            )
            .size_full(),
        )
    })
}

/// Onshape's document-load state over a studio's graphics (live behaviours "speed: psui open a document",
/// hose-run2-spinner-stages.png; live2 feedback.json progress.documentLoad): the large spinner (a ~100 px ring) in the
/// middle over a translucent white, one message line under it ("Loading studio data…", "Loading graphics…").
pub fn loading_overlay(message: &'static str) -> AnyElement {
    let pal = crate::theme::p();
    div()
        .id("loading-overlay")
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(gpui_kit::rgba(0xffffffb3))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(spinner(136.))
        .child(div().mt(px(-6.)).text_size(px(13.)).text_color(pal.text_muted).child(message))
        .into_any_element()
}

/// How long a studio opens before its loading state shows (a quick open shows none).
pub const LOADING_DELAY: Duration = Duration::from_millis(500);

/// The list's top row while a cached display stands in (the feature list, the instance list): Onshape's spinner and
/// "Regenerating…".
pub fn regenerating_row() -> AnyElement {
    let pal = crate::theme::p();
    gpui_kit::component::h_flex()
        .id("regenerating-row")
        .h(px(crate::ui::ROW_H))
        .flex_none()
        .gap(px(6.))
        .pl(px(9.))
        .child(spinner(18.))
        .child(div().text_size(px(crate::ui::ROW_TEXT)).text_color(pal.text_muted).child("Regenerating…"))
        .into_any_element()
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
    use super::{evict, load, path, store};

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

    /// What is stored reads back; another key or a damaged file reads as nothing.
    #[test]
    fn a_stored_value_reads_back_under_its_key_only() {
        let d = std::env::temp_dir().join(format!("noshape-display-cache-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        store(&d, "k1", &(vec![1u32, 2, 3], "x".to_string())).unwrap();
        assert_eq!(load::<(Vec<u32>, String)>(&d, "k1"), Some((vec![1, 2, 3], "x".to_string())));
        assert_eq!(load::<(Vec<u32>, String)>(&d, "k2"), None);
        std::fs::write(path(&d, "k1"), b"NSDISP02garbage").unwrap();
        assert_eq!(load::<(Vec<u32>, String)>(&d, "k1"), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
