//! The display cache (lead 11-06: "how Onshape opens instantly"): a tab's last displayed model kept with the
//! document, so opening it shows that model at once while the real regeneration runs (view-only until that lands).
//! Keyed by the whole document's content, what the tab adds (its id, configuration, …) and the build, so an edit never
//! shows a stale model; another build's display of the same content stands in too (a new release opens at once), as
//! every cached display does only until the regeneration replaces it. Kept in the .noshape itself (meta rows `display:<tag>`, one per tab and
//! level, replaced on each save: lead 11-21, a copied document or a new release opens at once too) and in the user
//! cache directory (`paths::cache_dir()/displays`, at most [`CAP_BYTES`], the least recently used evicted).
//! The studios keep their own snapshot types (Part Studio display, assembly occurrences, drawing sheets).

use gpui_kit::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
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

/// The cache key of a tab's display, "<tag>.<content>.<build>": the tag names its slot (`slot`: the studio and tab, one
/// row each in the document), the content hash what it shows (the whole document, as other tabs feed it: Part Studios,
/// Feature Studios, derived sources; the slot; what else the display depends on, `extra`: configuration, level), the
/// build hash the build that made it.
pub fn key(doc: &noshape_model::Document, slot: &impl Serialize, extra: &impl Serialize) -> Option<String> {
    key_for(doc, slot, extra, build_id())
}

/// The key of a Part Studio tab's display at configuration `config` (`fine`: meshed for View in high quality); the
/// Part Studio's own rows, which an assembly inserting it reads too.
pub fn studio_key(doc: &noshape_model::Document, tab: noshape_model::document::TabId, config: &impl Serialize, fine: bool) -> Option<String> {
    key(doc, &("part studio", tab), &(config, fine))
}

pub(crate) fn key_for(doc: &noshape_model::Document, slot: &impl Serialize, extra: &impl Serialize, build: &str) -> Option<String> {
    let tag = slot_tag(slot)?;
    // Binary: a large document's numbers hashed as their bytes, not printed as text (asm 11-22: hashing a 400 MB
    // document's JSON took 1.7-2.0 s of its 2.4-2.7 s cached first frame). JSON if bincode can't write a value.
    let mut h = content_hash(|w| bincode::serialize_into(&mut *w, doc).ok()).or_else(|| content_hash(|w| serde_json::to_writer(&mut *w, doc).ok()))?;
    serde_json::to_writer(&mut h, slot).ok()?;
    serde_json::to_writer(&mut h, extra).ok()?;
    let b = blake3::hash(build.as_bytes()).to_hex();
    Some(format!("{tag}.{}.{}", h.finalize().to_hex(), &b[..16]))
}

/// The hasher after `write` wrote the document into it (buffered: serializers write in small pieces).
fn content_hash(write: impl FnOnce(&mut std::io::BufWriter<&mut blake3::Hasher>) -> Option<()>) -> Option<blake3::Hasher> {
    let mut h = blake3::Hasher::new();
    {
        let mut w = std::io::BufWriter::with_capacity(1 << 16, &mut h);
        write(&mut w)?;
        std::io::Write::flush(&mut w).ok()?;
    }
    Some(h)
}

/// A key without its build ("<tag>.<content>"; an older key whole).
fn content(key: &str) -> &str {
    match key.rsplit_once('.') {
        Some((head, _)) if head.contains('.') => head,
        _ => key,
    }
}

/// Whether a stored key shows what `key` asks for: the same, or the same content made by another build.
fn shows(stored: &str, key: &str) -> bool {
    stored == key || content(stored) == content(key)
}

/// The tag of `slot`'s keys (their row in the document).
fn slot_tag(slot: &impl Serialize) -> Option<String> {
    let mut t = blake3::Hasher::new();
    serde_json::to_writer(&mut t, slot).ok()?;
    Some(t.finalize().to_hex()[..16].to_string())
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

/// A document's display rows by meta key, read when it was opened.
type Rows = std::collections::HashMap<String, Vec<u8>>;
/// A row's snapshot decoded to its studio's type.
type Decoded = Box<dyn std::any::Any + Send>;
type DecodeFn = fn(&[u8]) -> Option<Decoded>;
/// A row's key and its snapshot decoded as the document opened (taken by the first lookup).
type EarlyCell = OnceLock<parking_lot::Mutex<Option<(String, Decoded)>>>;

/// A document's early read: its rows, and the row of the tab it opens on decoded (`decode_early`), by tag.
struct Prefetch {
    rows: OnceLock<Rows>,
    early: Vec<(String, EarlyCell)>,
}

fn prefetched() -> &'static parking_lot::Mutex<std::collections::HashMap<PathBuf, Arc<Prefetch>>> {
    static P: OnceLock<parking_lot::Mutex<std::collections::HashMap<PathBuf, Arc<Prefetch>>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

/// The studios' early decoders: a tab's slot tag, and its row's snapshot decoded.
type Early = (Box<dyn Fn(noshape_model::document::TabId) -> Option<String> + Send + Sync>, DecodeFn);

fn early_decoders() -> &'static parking_lot::Mutex<Vec<Early>> {
    static E: OnceLock<parking_lot::Mutex<Vec<Early>>> = OnceLock::new();
    E.get_or_init(Default::default)
}

/// A studio's row of the tab a document opens on (`slot(tab)`, as its `load_for`'s) is decoded as soon as it is read,
/// alongside the session and window (infento: its 0.15 s decode off the open's path). `T` is what `load_for` asks.
pub fn decode_early<T: DeserializeOwned + Send + 'static, S: Serialize + 'static>(slot: fn(noshape_model::document::TabId) -> S) {
    early_decoders().lock().push((Box::new(move |tab| slot_tag(&slot(tab))), decode_boxed::<T>));
}

fn decode_boxed<T: DeserializeOwned + Send + 'static>(bytes: &[u8]) -> Option<Decoded> {
    decode::<T>(bytes).map(|v| Box::new(v) as Decoded)
}

/// Reads `doc`'s display rows on a thread of their own as the document opens, alongside its session and window (the
/// open's first lookup then waits on this read instead of starting its own), and decodes the row of `tab`, the tab it
/// opens on, for the studio that keeps it.
pub fn prefetch(doc: &Path, tab: Option<noshape_model::document::TabId>) {
    if !enabled() {
        return;
    }
    let early = tab.map(|t| early_decoders().lock().iter().filter_map(|(tag, decode)| Some((tag(t)?, *decode))).collect()).unwrap_or_default();
    prefetch_rows(doc, early);
}

fn prefetch_rows(doc: &Path, early: Vec<(String, DecodeFn)>) {
    if !doc.exists() {
        return;
    }
    let p = Arc::new(Prefetch { rows: OnceLock::new(), early: early.iter().map(|(tag, _)| (tag.clone(), OnceLock::new())).collect() });
    prefetched().lock().insert(doc.to_path_buf(), p.clone());
    let path = doc.to_path_buf();
    let read = move || {
        let rows = noshape_store::Document::open_read_only(&path).and_then(|d| d.meta_with_prefix(META)).unwrap_or_default();
        let rows = p.rows.get_or_init(|| rows.into_iter().collect());
        noshape_render::perf::milestone("open: display rows read");
        for ((tag, decode), (_, cell)) in early.iter().zip(&p.early) {
            let decoded = rows.get(&format!("{META}{tag}")).and_then(|row| split_row(row)).and_then(|(key, body)| Some((key, decode(body)?)));
            let _ = cell.set(parking_lot::Mutex::new(decoded));
        }
    };
    if std::thread::Builder::new().name("display-rows".into()).spawn(read).is_err() {
        prefetched().lock().remove(doc);
    }
}

/// A row's key and snapshot (split at the first newline).
fn split_row(row: &[u8]) -> Option<(String, &[u8])> {
    let at = row.iter().position(|b| *b == b'\n')?;
    Some((String::from_utf8(row[..at].to_vec()).ok()?, &row[at + 1..]))
}

/// A document's row of `tag`: its key, a newline, then the snapshot.
fn tag_row(doc: &Path, tag: &str) -> Option<(String, Vec<u8>)> {
    let meta = format!("{META}{tag}");
    let early = prefetched().lock().get(doc).cloned();
    let row = match early {
        Some(p) => p.rows.wait().get(&meta).cloned()?,
        None => noshape_store::Document::open_read_only(doc).ok()?.get_meta(&meta).ok()??,
    };
    split_row(&row).map(|(key, body)| (key, body.to_vec()))
}

/// The row of `tag` and its key as decoded when the document opened, if it was (taken: the first lookup's).
fn take_early<T: 'static>(doc: &Path, tag: &str) -> Option<(String, T)> {
    let p = prefetched().lock().get(doc).cloned()?;
    let (_, cell) = p.early.iter().find(|(t, _)| t == tag)?;
    let (key, v) = cell.wait().lock().take()?;
    Some((key, *v.downcast::<T>().ok()?))
}

/// A document's row of `key`, if its row holds that key.
fn doc_row(doc: &Path, key: &str) -> Option<Vec<u8>> {
    tag_row(doc, tag(key)).filter(|(k, _)| k == key).map(|(_, body)| body)
}

/// A document's row showing `key`'s content, made by this build or another (then only if it reads as this build's
/// type, [`same_layout`]), decoded.
fn doc_row_any_build<T: DeserializeOwned + Serialize>(doc: &Path, key: &str) -> Option<T> {
    let (stored, body) = tag_row(doc, tag(key)).filter(|(k, _)| shows(k, key))?;
    let v = decode(&body)?;
    (stored == key || same_layout(&v, &body)).then_some(v)
}

/// Whether `v`, decoded from another build's `body`, encodes back to the same bytes: its type's layout unchanged
/// (bincode isn't self-describing: a field added or retyped since reads as other values, not an error).
fn same_layout<T: Serialize>(v: &T, body: &[u8]) -> bool {
    let Some(raw) = body.strip_prefix(MAGIC.as_slice()).and_then(|b| zstd::decode_all(b).ok()) else { return false };
    bincode::serialize(v).is_ok_and(|e| e == raw)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    let body = bytes.strip_prefix(MAGIC.as_slice())?;
    bincode::deserialize(&zstd::decode_all(body).ok()?).ok()
}

/// What is cached under `key`, if anything: the document's own row first (this build's or another's of the same
/// content), then the directory's (its use time refreshed for the eviction). A row of an older content is never used.
pub fn load<T: DeserializeOwned + Serialize>(shelf: &Shelf, key: &str) -> Option<T> {
    if let Some(v) = shelf.doc.as_deref().and_then(|d| doc_row_any_build(d, key)) {
        return Some(v);
    }
    dir_load(shelf, key)
}

/// The directory's file of `key` (its use time refreshed for the eviction).
fn dir_load<T: DeserializeOwned>(shelf: &Shelf, key: &str) -> Option<T> {
    let p = path(&shelf.dir, key);
    let value = decode(&std::fs::read(&p).ok()?)?;
    if let Ok(f) = std::fs::File::options().write(true).open(&p) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
    Some(value)
}

/// `load` of `key(doc, slot, extra)`, the document's row of the slot decoded as it opened (`decode_early`) or meanwhile
/// on a thread of its own, used only if it shows that key's content. Returns the key too.
pub fn load_for<T: DeserializeOwned + Serialize + Send + 'static>(shelf: &Shelf, doc: &noshape_model::Document, slot: &impl Serialize, extra: &impl Serialize) -> Option<(String, T)> {
    load_keyed(shelf, slot, || key(doc, slot, extra))
}

fn load_keyed<T: DeserializeOwned + Serialize + Send + 'static>(shelf: &Shelf, slot: &impl Serialize, key: impl FnOnce() -> Option<String>) -> Option<(String, T)> {
    let early = shelf.doc.as_deref().zip(slot_tag(slot)).and_then(|(d, tag)| match take_early::<T>(d, &tag) {
        Some((stored, v)) => Some((stored, Decoding::Done(v))),
        None => {
            let (stored, body) = tag_row(d, &tag)?;
            let decoding = std::thread::Builder::new().name("display-decode".into()).spawn(move || decode::<T>(&body)).ok()?;
            Some((stored, Decoding::Running(decoding)))
        }
    });
    let key = key()?;
    noshape_render::perf::milestone("open: display key");
    if let Some((stored, decoding)) = early
        && shows(&stored, &key)
        && let Some(v) = decoding.value()
        && (stored == key || shelf.doc.as_deref().and_then(|d| tag_row(d, tag(&key))).is_some_and(|(k, body)| k == stored && same_layout(&v, &body)))
    {
        return Some((key, v));
    }
    let value = dir_load(shelf, &key)?;
    Some((key, value))
}

/// A row's snapshot: decoded, or being decoded.
enum Decoding<T> {
    Done(T),
    Running(std::thread::JoinHandle<Option<T>>),
}

impl<T> Decoding<T> {
    fn value(self) -> Option<T> {
        match self {
            Self::Done(v) => Some(v),
            Self::Running(h) => h.join().ok().flatten(),
        }
    }
}

/// Keep `value` under `key`: in the document (its slot's row replaced) and in the directory (written beside and
/// renamed in, so a reader never sees half a file; then evicted to size).
pub fn store<T: Serialize>(shelf: &Shelf, key: &str, value: &T) -> std::io::Result<()> {
    let raw = bincode::serialize(value).map_err(std::io::Error::other)?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend(zstd::encode_all(raw.as_slice(), 1)?);
    if let Some(doc) = &shelf.doc {
        prefetched().lock().remove(doc);
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

    /// asm 11-22: the key hashes the document's binary form, not its JSON (a 400 MB document's JSON took 1.7-2.0 s):
    /// a document with a sketch, a feature and a configuration writes as bincode (no JSON fallback), the same twice.
    #[test]
    fn a_documents_key_hashes_its_binary_form() {
        let d = std::env::temp_dir().join(format!("noshape-display-cache-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap();
        call(&mut ctx, "document_create", serde_json::json!({ "path": d.join("doc.noshape").to_str().unwrap() }));
        call(&mut ctx, "sketch_create", serde_json::json!({ "plane": "plane:top" }));
        call(&mut ctx, "sketch_draw", serde_json::json!({ "sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [20, 10]] }));
        call(&mut ctx, "config_add_input", serde_json::json!({ "kind": "list", "name": "Size" }));
        let doc = ctx.doc(None).unwrap().doc().clone();
        assert!(bincode::serialize(&doc).is_ok(), "bincode writes the document");
        let tab = doc.tabs[0].id;
        assert_eq!(key_for(&doc, &("part studio", tab), &"c", "b"), key_for(&doc.clone(), &("part studio", tab), &"c", "b"));
    }

    /// Lead 11-21 (fast open): the display kept in the .noshape opens on another machine (an empty cache directory) and
    /// with another release (until the regeneration replaces it); after an edit its key differs and the row isn't
    /// used; a save replaces its tab's row.
    #[test]
    fn a_document_keeps_its_display_for_its_content() {
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
        let other = key_for(&doc, &("part studio", tab), &"config", &build_id_of("0.5.1", true, Some("c8d74e607a47"), "1:2", "p1")).unwrap();
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
        // Read as the document opens: the same answers; a save drops the early read (its rows changed).
        super::prefetch_rows(&file, Vec::new());
        assert_eq!(load::<Vec<u32>>(&elsewhere, &k), Some(vec![7; 3]), "from the early read");
        assert_eq!(load::<Vec<u32>>(&elsewhere, &config2), None);
        store(&here, &config2, &vec![9u32]).unwrap();
        assert!(!super::prefetched().lock().contains_key(&file), "a save drops the early read");
        assert_eq!(load::<Vec<u32>>(&elsewhere, &config2), Some(vec![9]));
        store(&here, &k, &vec![7u32; 3]).unwrap();
        // A miss after an edit.
        doc.tabs[0].name.push_str(" (renamed)");
        let edited = key_for(&doc, &("part studio", tab), &"config", &build).unwrap();
        assert_ne!(edited, k);
        assert_eq!(load::<Vec<u32>>(&elsewhere, &edited), None);
        // Decoded while the key is worked out: used only when the row holds that key, else the directory's file.
        let slot = ("part studio", tab);
        assert_eq!(super::load_keyed::<Vec<u32>>(&elsewhere, &slot, || Some(k.clone())), Some((k.clone(), vec![7; 3])));
        assert_eq!(super::load_keyed::<Vec<u32>>(&elsewhere, &slot, || Some(edited.clone())), None, "the row holds the unedited key");
        assert_eq!(super::load_keyed::<Vec<u32>>(&here, &slot, || Some(config2.clone())), Some((config2.clone(), vec![9])), "the directory's file");
        // Decoded as the document opens (the tab it opens on): taken by the first lookup, the next decodes its own.
        let tag = super::slot_tag(&slot).unwrap();
        super::prefetch_rows(&file, vec![(tag.clone(), super::decode_boxed::<Vec<u32>>)]);
        let decoded = |p: &super::Prefetch| p.early[0].1.wait().lock().as_ref().map(|(key, _)| key.clone());
        let p = super::prefetched().lock()[&file].clone();
        assert_eq!(decoded(&p), Some(k.clone()), "decoded as the rows are read");
        assert_eq!(super::load_keyed::<Vec<u32>>(&elsewhere, &slot, || Some(k.clone())), Some((k.clone(), vec![7; 3])));
        assert_eq!(decoded(&p), None, "taken");
        assert_eq!(super::load_keyed::<Vec<u32>>(&elsewhere, &slot, || Some(k.clone())), Some((k.clone(), vec![7; 3])));
        assert_eq!(super::take_early::<Vec<u32>>(&file, "another tab"), None, "only the tab it opens on");
        super::prefetch_rows(&file, vec![(tag, super::decode_boxed::<Vec<u32>>)]);
        assert_eq!(super::load_keyed::<Vec<u32>>(&elsewhere, &slot, || Some(edited.clone())), None, "decoded early, another key");
        // Another release's key: the row of the same content stands in (the next save adds this build's); a release's
        // commit or executable doesn't matter, a development build's does.
        assert_ne!(other, edited);
        assert_ne!(other, k);
        assert_eq!(load::<Vec<u32>>(&elsewhere, &other), Some(vec![7; 3]));
        assert_eq!(super::load_keyed::<Vec<u32>>(&elsewhere, &slot, || Some(other.clone())), Some((other.clone(), vec![7; 3])));
        assert!(!super::contains(&elsewhere, &other), "kept again for this build");
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
