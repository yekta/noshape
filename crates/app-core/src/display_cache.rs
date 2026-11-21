//! The display cache (lead 11-06: "how Onshape opens instantly"): a tab's last displayed model kept with the
//! document, so opening it shows that model at once while the real regeneration runs (view-only until that lands).
//! Keyed by the whole document's content, what the tab adds (its id, configuration, …) and the build, so an edit or
//! another build never shows a stale model. Kept in the .noshape itself (meta rows `display:<tag>`, one per tab and
//! level, replaced on each save: lead 11-21, a copied document or a new release opens at once too) and in the user
//! cache directory (`paths::cache_dir()/displays`, at most [`CAP_BYTES`], the least recently used evicted).
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
/// The cache directory's size bound.
const CAP_BYTES: u64 = 1 << 30;
/// A document's meta key prefix of its cached displays.
const META: &str = "display:";

/// The message while an action waits for the regeneration.
pub const WAITING: &str = "Regenerating the model: this opens as soon as it is ready.";

static ENABLED: AtomicBool = AtomicBool::new(false);
static COMMIT: OnceLock<&'static str> = OnceLock::new();

/// The app keeps and uses the display cache (called once at startup with its build's commit, if it has one; tests
/// and the parity driver don't).
pub fn enable(commit: Option<&'static str>) {
    if let Some(c) = commit {
        let _ = COMMIT.set(c);
    }
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Where the app keeps the cache outside documents.
pub fn default_dir() -> PathBuf {
    crate::paths::cache_dir().join("displays")
}

/// Where a tab's displays are kept: the cache directory and its document's file.
#[derive(Clone, Debug)]
pub struct Shelf {
    pub dir: PathBuf,
    pub doc: Option<PathBuf>,
}

impl Shelf {
    /// The app's: the user cache directory and the document at `doc`.
    pub fn app(doc: &Path) -> Self {
        Self { dir: default_dir(), doc: Some(doc.to_path_buf()) }
    }

    /// A directory only (tests).
    pub fn dir(dir: &Path) -> Self {
        Self { dir: dir.to_path_buf(), doc: None }
    }
}

/// A build's id from its parts: a release's stamped version names its code; a development build (one version for
/// all) adds its commit, or without one the executable's size and time.
pub(crate) fn build_id_of(version: &str, release: bool, commit: Option<&str>, exe_stamp: &str, occt_patches: &str) -> String {
    match (release, commit) {
        (true, _) => format!("{version}|{occt_patches}"),
        (false, Some(c)) => format!("{version}|{c}|{occt_patches}"),
        (false, None) => format!("{version}|{exe_stamp}|{occt_patches}"),
    }
}

/// This build's id.
pub(crate) fn build_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let exe = std::env::current_exe().ok().and_then(|p| std::fs::metadata(p).ok());
        let stamp = exe.map(|m| format!("{}:{:?}", m.len(), m.modified().ok())).unwrap_or_default();
        build_id_of(noshape_ops::VERSION, option_env!("NOSHAPE_VERSION").is_some(), COMMIT.get().copied(), &stamp, noshape_ops::OCCT_PATCHES)
    })
}

/// The cache key of a tab's display, "<tag>.<hash>": the tag names its slot (`slot`: the studio and tab, one row each
/// in the document), the hash its content (the whole document, as other tabs feed it: Part Studios, Feature Studios,
/// derived sources; the slot; what else the display depends on, `extra`: configuration, level; the build).
pub fn key(doc: &noshape_model::Document, slot: &impl Serialize, extra: &impl Serialize) -> Option<String> {
    key_for(doc, slot, extra, build_id())
}

pub(crate) fn key_for(doc: &noshape_model::Document, slot: &impl Serialize, extra: &impl Serialize, build: &str) -> Option<String> {
    let mut t = blake3::Hasher::new();
    serde_json::to_writer(&mut t, slot).ok()?;
    let mut h = blake3::Hasher::new();
    serde_json::to_writer(&mut h, doc).ok()?;
    serde_json::to_writer(&mut h, slot).ok()?;
    serde_json::to_writer(&mut h, extra).ok()?;
    h.update(build.as_bytes());
    Some(format!("{}.{}", &t.finalize().to_hex()[..16], h.finalize().to_hex()))
}

/// Another level of `key`'s display (the coarse preview), in its own slot.
pub fn variant(key: &str, level: &str) -> String {
    match key.split_once('.') {
        Some((tag, hash)) => format!("{tag}-{level}.{hash}"),
        None => format!("{key}-{level}"),
    }
}

fn tag(key: &str) -> &str {
    key.split_once('.').map_or(key, |(t, _)| t)
}

fn path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.display"))
}

/// A document's row of `key`: the key, a newline, then the snapshot.
fn doc_row(doc: &Path, key: &str) -> Option<Vec<u8>> {
    let d = noshape_store::Document::open_read_only(doc).ok()?;
    let row = d.get_meta(&format!("{META}{}", tag(key))).ok()??;
    let body = row.strip_prefix(key.as_bytes())?.strip_prefix(b"\n")?;
    Some(body.to_vec())
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    let body = bytes.strip_prefix(MAGIC.as_slice())?;
    bincode::deserialize(&zstd::decode_all(body).ok()?).ok()
}

/// What is cached under `key`, if anything: the document's own row first, then the directory's (its use time
/// refreshed for the eviction). A row of another key (an older content or build) is never used.
pub fn load<T: DeserializeOwned>(shelf: &Shelf, key: &str) -> Option<T> {
    if let Some(v) = shelf.doc.as_deref().and_then(|d| doc_row(d, key)).and_then(|b| decode(&b)) {
        return Some(v);
    }
    let p = path(&shelf.dir, key);
    let value = decode(&std::fs::read(&p).ok()?)?;
    if let Ok(f) = std::fs::File::options().write(true).open(&p) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
    Some(value)
}

/// Keep `value` under `key`: in the document (its slot's row replaced) and in the directory (written beside and
/// renamed in, so a reader never sees half a file; then evicted to size).
pub fn store<T: Serialize>(shelf: &Shelf, key: &str, value: &T) -> std::io::Result<()> {
    let raw = bincode::serialize(value).map_err(std::io::Error::other)?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend(zstd::encode_all(raw.as_slice(), 1)?);
    if let Some(doc) = &shelf.doc {
        let mut row = Vec::with_capacity(key.len() + 1 + bytes.len());
        row.extend_from_slice(key.as_bytes());
        row.push(b'\n');
        row.extend_from_slice(&bytes);
        let written = noshape_store::Document::open(doc).and_then(|mut d| d.set_meta(&format!("{META}{}", tag(key)), Some(&row)));
        if let Err(e) = written {
            log::warn!("could not keep the display in {}: {e}", doc.display());
        }
    }
    std::fs::create_dir_all(&shelf.dir)?;
    let tmp = shelf.dir.join(format!("{key}.tmp{}", std::process::id()));
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path(&shelf.dir, key))?;
    evict(&shelf.dir, CAP_BYTES);
    Ok(())
}

/// Whether `key` is cached in both places (else a store fills the missing one).
pub fn contains(shelf: &Shelf, key: &str) -> bool {
    path(&shelf.dir, key).exists() && shelf.doc.as_deref().is_none_or(|d| doc_row(d, key).is_some())
}

/// The spinner in a `size` px box (the overlay's 41 px; a list row's smaller).
pub fn spinner(size: f32) -> impl IntoElement {
    spinner_ring(size, 13.5 * size / 41.)
}

/// One turn of the spinner.
pub const SPINNER_TURN: Duration = Duration::from_millis(1100);

/// The spinner's arcs at `t` (0 to 1) into a turn, drawn in order: (from, to radians clockwise from 3 o'clock, colour):
/// the whole grey ring, then the blue quarter turning with it.
pub fn spinner_arcs(t: f32) -> [(f32, f32, Rgba); 2] {
    let tau = std::f32::consts::TAU;
    let start = t * tau - tau / 4.;
    [(0., tau, rgb(0xd7d7d7)), (start, start + tau / 4., rgb(0x1651b0))]
}

/// Onshape's spinner (live2 11-18, computed CSS): a 30 px circle, its 3 px ring rgb(215,215,215) with a quarter in
/// rgb(22,81,176), turning once in 1.1 s; `r` the ring's middle radius in a `size` px box.
pub fn spinner_ring(size: f32, r: f32) -> impl IntoElement {
    let scale = r / 13.5;
    div().size(px(size)).flex_none().with_animation(SharedString::from(format!("pending-spinner-{size}")), Animation::new(SPINNER_TURN).repeat(), move |d, t| {
        d.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let c = bounds.center();
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
                    for (from, to, color) in spinner_arcs(t) {
                        ring(from, to, color, window);
                    }
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
    use super::{SPINNER_TURN, Shelf, build_id_of, evict, key_for, load, path, spinner_arcs, store, variant};

    /// Live2 spinner 11-18 (computed CSS): a grey rgb(215,215,215) ring under a blue rgb(22,81,176) quarter that turns
    /// once in 1.1 s, starting at 12 o'clock.
    #[test]
    fn the_spinner_is_a_grey_ring_with_a_blue_quarter_turning_in_1_1_s() {
        let tau = std::f32::consts::TAU;
        assert_eq!(SPINNER_TURN, std::time::Duration::from_millis(1100));
        let [ring, quarter] = spinner_arcs(0.);
        assert_eq!((ring.0, ring.1, ring.2), (0., tau, gpui_kit::rgb(0xd7d7d7)), "the whole ring, grey");
        assert_eq!(quarter.2, gpui_kit::rgb(0x1651b0), "the quarter, blue");
        assert!((quarter.0 + tau / 4.).abs() < 1e-6 && (quarter.1 - quarter.0 - tau / 4.).abs() < 1e-6, "a quarter from 12 o'clock: {quarter:?}");
        let [_, half] = spinner_arcs(0.5);
        assert!((half.0 - quarter.0 - tau / 2.).abs() < 1e-5, "half a turn on at t = 0.5: {half:?}");
    }

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
        let shelf = Shelf::dir(&d);
        store(&shelf, "k1", &(vec![1u32, 2, 3], "x".to_string())).unwrap();
        assert_eq!(load::<(Vec<u32>, String)>(&shelf, "k1"), Some((vec![1, 2, 3], "x".to_string())));
        assert_eq!(load::<(Vec<u32>, String)>(&shelf, "k2"), None);
        std::fs::write(path(&d, "k1"), b"NSDISP02garbage").unwrap();
        assert_eq!(load::<(Vec<u32>, String)>(&shelf, "k1"), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Lead 11-21 (fast open): the display kept in the .noshape opens on another machine (an empty cache directory);
    /// after an edit or with another build its key differs and the row isn't used; a save replaces its tab's row.
    #[test]
    fn a_document_keeps_its_display_for_its_content_and_build_only() {
        let d = std::env::temp_dir().join(format!("noshape-display-cache-doc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let file = d.join("doc.noshape");
        noshape_store::Document::create(&file, &[("document/meta", b"{}".as_slice())]).unwrap();
        let here = Shelf { dir: d.join("cache-here"), doc: Some(file.clone()) };
        let elsewhere = Shelf { dir: d.join("cache-elsewhere"), doc: Some(file.clone()) };
        let mut doc = noshape_model::Document::new();
        let tab = doc.tabs[0].id;
        let build = build_id_of("0.5.0", true, Some("c8d74e607a47"), "1:2", "p1");
        let k = key_for(&doc, &("part studio", tab), &"config", &build).unwrap();
        store(&here, &k, &vec![7u32; 3]).unwrap();
        // A hit from the file alone, its coarse level beside it.
        assert_eq!(load::<Vec<u32>>(&elsewhere, &k), Some(vec![7; 3]));
        store(&here, &variant(&k, "coarse"), &vec![1u32]).unwrap();
        assert_eq!(load::<Vec<u32>>(&elsewhere, &variant(&k, "coarse")), Some(vec![1]));
        assert_eq!(load::<Vec<u32>>(&elsewhere, &k), Some(vec![7; 3]), "the coarse level has its own row");
        // Another configuration: the tab's one row, another key.
        let config2 = key_for(&doc, &("part studio", tab), &"config 2", &build).unwrap();
        assert_eq!(config2.split_once('.').unwrap().0, k.split_once('.').unwrap().0, "one row per tab");
        assert_ne!(config2, k);
        assert_eq!(load::<Vec<u32>>(&elsewhere, &config2), None);
        // A miss after an edit.
        doc.tabs[0].name.push_str(" (renamed)");
        let edited = key_for(&doc, &("part studio", tab), &"config", &build).unwrap();
        assert_ne!(edited, k);
        assert_eq!(load::<Vec<u32>>(&elsewhere, &edited), None);
        // A miss with another release; a release's commit or executable doesn't matter, a development build's does.
        let other = key_for(&doc, &("part studio", tab), &"config", &build_id_of("0.5.1", true, Some("c8d74e607a47"), "1:2", "p1")).unwrap();
        assert_ne!(other, edited);
        assert_eq!(build_id_of("0.5.0", true, Some("aaa"), "3:4", "p1"), build);
        assert_ne!(build_id_of("0.0.0-dev", false, Some("aaa"), "1:2", "p1"), build_id_of("0.0.0-dev", false, Some("bbb"), "1:2", "p1"));
        assert_ne!(build_id_of("0.0.0-dev", false, None, "1:2", "p1"), build_id_of("0.0.0-dev", false, None, "1:3", "p1"));
        assert_ne!(build_id_of("0.5.0", true, None, "1:2", "p1"), build_id_of("0.5.0", true, None, "1:2", "p2"), "the OCCT patch set");
        // A save of the edited document replaces the tab's row: the old key no longer reads from the file.
        store(&here, &edited, &vec![8u32]).unwrap();
        assert_eq!(load::<Vec<u32>>(&elsewhere, &edited), Some(vec![8]));
        assert_eq!(load::<Vec<u32>>(&Shelf { dir: d.join("cache-third"), doc: Some(file.clone()) }, &k), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
