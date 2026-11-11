//! Regeneration results as bytes (the persistent regeneration cache's entries): a header, the value as JSON (zstd),
//! its shapes as OCCT's binary B-rep, each once in a table the JSON numbers into, and a checksum. A context holding
//! FeatureScript attributes or variables isn't stored (they can hold functions): its studio regenerates as before.

use super::context::Context;
use crate::display::FeatureState;
use noshape_kernel::with_shape_blobs;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// What starts every entry: a different start (another format) is never read.
const MAGIC: &[u8; 8] = b"NSCTX002";

/// A context as bytes, or `None` where it can't be stored.
pub fn encode(ctx: &Context) -> Option<Vec<u8>> {
    storable(ctx).then(|| frame(ctx)).flatten()
}

/// What [`encode`] wrote, or `None` for anything else (another format, cut short, changed).
pub fn decode(bytes: &[u8]) -> Option<Context> {
    unframe(bytes)
}

/// A whole regeneration's result: its context and its features' states.
pub fn encode_result(ctx: &Context, states: &[FeatureState]) -> Option<Vec<u8>> {
    storable(ctx).then(|| frame(&(states, ctx))).flatten()
}

/// What [`encode_result`] wrote.
pub fn decode_result(bytes: &[u8]) -> Option<(Context, Vec<FeatureState>)> {
    let (states, ctx): (Vec<FeatureState>, Context) = unframe(bytes)?;
    Some((ctx, states))
}

fn storable(ctx: &Context) -> bool {
    ctx.fs_attributes.is_empty() && ctx.fs_variables.is_empty()
}

fn frame<T: Serialize>(value: &T) -> Option<Vec<u8>> {
    let (json, blobs) = with_shape_blobs(Vec::new(), || serde_json::to_vec(value));
    let json = zstd::encode_all(&json.ok()?[..], 3).ok()?;
    let mut out = Vec::with_capacity(json.len() + blobs.iter().map(|b| b.len() + 8).sum::<usize>() + 48);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(json.len() as u64).to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(blobs.len() as u64).to_le_bytes());
    for b in &blobs {
        out.extend_from_slice(&(b.len() as u64).to_le_bytes());
        out.extend_from_slice(b);
    }
    // (A checksum of all of it last: a file cut short or changed reads as nothing.)
    out.extend_from_slice(&blake3::hash(&out).as_bytes()[..16]);
    Some(out)
}

fn unframe<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    let (body, sum) = bytes.split_at_checked(bytes.len().checked_sub(16)?)?;
    if blake3::hash(body).as_bytes()[..16] != *sum {
        return None;
    }
    let mut r = body.strip_prefix(MAGIC.as_slice())?;
    let mut take = |n: usize| -> Option<&[u8]> {
        let (a, b) = r.split_at_checked(n)?;
        r = b;
        Some(a)
    };
    let n = u64::from_le_bytes(take(8)?.try_into().ok()?) as usize;
    let json = zstd::decode_all(take(n)?).ok()?;
    let count = u64::from_le_bytes(take(8)?.try_into().ok()?) as usize;
    let mut blobs = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        let n = u64::from_le_bytes(take(8)?.try_into().ok()?) as usize;
        blobs.push(take(n)?.to_vec());
    }
    let (value, _) = with_shape_blobs(blobs, || serde_json::from_slice::<T>(&json));
    value.ok()
}

/// A [`super::ResultStore`] in a directory: one file per key and build (`<key>-<build>.result`, a new build never
/// reads an old one's), written beside and renamed in; past `cap` bytes the least recently used go.
#[derive(Debug)]
pub struct DirStore {
    pub dir: std::path::PathBuf,
    /// What tells this build's results from another's (its version and executable).
    pub build: String,
    pub cap: u64,
}

impl DirStore {
    fn path(&self, key: super::RegenKey) -> std::path::PathBuf {
        self.dir.join(format!("{key:032x}-{}.result", self.build))
    }

    /// Removes files until the directory holds at most `cap` bytes, least recently used first.
    fn evict(&self) {
        let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = std::fs::read_dir(&self.dir)
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
            if total <= self.cap {
                break;
            }
            if std::fs::remove_file(&p).is_ok() {
                total -= len;
            }
        }
    }
}

impl DirStore {
    fn blob_path(&self, hash: &[u8; 16]) -> std::path::PathBuf {
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(format!("{hex}-{}.brep", self.build))
    }
}

impl super::ResultStore for DirStore {
    fn get(&self, key: super::RegenKey) -> Option<Vec<u8>> {
        let p = self.path(key);
        let bytes = std::fs::read(&p).ok()?;
        if let Ok(f) = std::fs::File::options().write(true).open(&p) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        Some(bytes)
    }

    fn put(&self, key: super::RegenKey, bytes: Vec<u8>) {
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let tmp = self.dir.join(format!("{key:032x}.tmp{}", std::process::id()));
        if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, self.path(key)).is_ok() {
            self.evict();
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    fn get_blob(&self, hash: &[u8; 16]) -> Option<Vec<u8>> {
        let p = self.blob_path(hash);
        let bytes = std::fs::read(&p).ok()?;
        if let Ok(f) = std::fs::File::options().write(true).open(&p) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        Some(bytes)
    }

    fn put_blob(&self, hash: &[u8; 16], bytes: Vec<u8>) {
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        let tmp = self.dir.join(format!("{hex}.tmp{}", std::process::id()));
        if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, self.blob_path(hash)).is_ok() {
            self.evict();
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    fn has_blob(&self, hash: &[u8; 16]) -> bool {
        self.blob_path(hash).exists()
    }
}


/// The key a feature's entry is kept under: its chain key, apart from whole results' keys.
pub fn feature_key(key: super::RegenKey) -> super::RegenKey {
    super::hash_bytes(key, b"feature entry")
}

fn content_hash(bytes: &[u8]) -> [u8; 16] {
    let mut h = [0u8; 16];
    h.copy_from_slice(&blake3::hash(bytes).as_bytes()[..16]);
    h
}

/// Shapes written once across a writer's entries: by identity (OCCT's IsSame hash, then IsEqual), their content hash.
#[derive(Default)]
pub struct ShapeMemo(std::collections::HashMap<u64, Vec<(noshape_kernel::Shape, [u8; 16])>>);

/// Writing: each shape a number into the entry's list of content hashes, its bytes put in the store once.
struct BlobWriter {
    store: std::sync::Arc<dyn super::ResultStore>,
    memo: ShapeMemo,
    hashes: Vec<[u8; 16]>,
}

impl noshape_kernel::ShapeCodec for BlobWriter {
    fn put(&mut self, shape: &noshape_kernel::Shape) -> Result<u64, String> {
        let id = shape.hash_code();
        let known = self.memo.0.get(&id).and_then(|v| v.iter().find(|(s, _)| s.is_equal(shape)).map(|(_, h)| *h));
        let hash = match known {
            Some(h) => h,
            None => {
                let bytes = if shape.is_null() { Vec::new() } else { shape.to_bin_bytes(false).map_err(|e| e.to_string())? };
                let h = content_hash(&bytes);
                if !self.store.has_blob(&h) {
                    self.store.put_blob(&h, bytes);
                }
                self.memo.0.entry(id).or_default().push((shape.clone(), h));
                h
            }
        };
        let at = match self.hashes.iter().position(|x| *x == hash) {
            Some(i) => i,
            None => {
                self.hashes.push(hash);
                self.hashes.len() - 1
            }
        };
        Ok(at as u64)
    }
    fn get(&mut self, _at: u64) -> Result<noshape_kernel::Shape, String> {
        Err("a writer reads nothing".into())
    }
}

/// Shapes read back, by content hash (shared bodies decode once a session).
fn read_cache() -> &'static parking_lot::Mutex<std::collections::HashMap<[u8; 16], noshape_kernel::Shape>> {
    static C: std::sync::OnceLock<parking_lot::Mutex<std::collections::HashMap<[u8; 16], noshape_kernel::Shape>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

/// Reading: each number an entry's content hash, its shape from the cache or the store (the bytes checked against it).
struct BlobReader {
    store: std::sync::Arc<dyn super::ResultStore>,
    hashes: Vec<[u8; 16]>,
}

impl noshape_kernel::ShapeCodec for BlobReader {
    fn put(&mut self, _shape: &noshape_kernel::Shape) -> Result<u64, String> {
        Err("a reader writes nothing".into())
    }
    fn get(&mut self, at: u64) -> Result<noshape_kernel::Shape, String> {
        let hash = *self.hashes.get(at as usize).ok_or("no such shape")?;
        if let Some(s) = read_cache().lock().get(&hash) {
            return Ok(s.clone());
        }
        let bytes = self.store.get_blob(&hash).ok_or("shape not kept")?;
        if content_hash(&bytes) != hash {
            return Err("shape changed".into());
        }
        let shape = if bytes.is_empty() { noshape_kernel::Shape::null() } else { noshape_kernel::Shape::from_bin_bytes(&bytes).map_err(|e| e.to_string())? };
        let mut c = read_cache().lock();
        if c.len() > 8192 {
            c.clear();
        }
        c.insert(hash, shape.clone());
        Ok(shape)
    }
}

/// The key a feature's context is kept under (apart from its state's: a reopened studio reads the states, and only
/// the context a later feature runs on).
pub fn context_key(key: super::RegenKey) -> super::RegenKey {
    super::hash_bytes(key, b"feature context")
}

/// What starts a feature's context entry.
const CONTEXT_MAGIC: &[u8; 8] = b"NSCTF001";

/// A stored feature's state, without its context: what stepping past it needs.
pub struct FeatureLight {
    pub state: FeatureState,
    /// The variables after it (later features' expressions read them).
    pub variables: std::collections::BTreeMap<String, noshape_units::Quantity>,
    /// Whether sheet metal is active after it (a later feature's route).
    pub sheet_metal: bool,
}

/// A feature's state, its variables and whether sheet metal is active after it.
pub fn encode_feature_state(state: &FeatureState, ctx: &Context) -> Option<Vec<u8>> {
    let sheet_metal = ctx.sheet_metal.values().any(|d| d.active);
    frame(&(state, &ctx.variables, sheet_metal))
}

/// What [`encode_feature_state`] wrote.
pub fn decode_feature_state(bytes: &[u8]) -> Option<FeatureLight> {
    let (state, variables, sheet_metal) = unframe(bytes)?;
    Some(FeatureLight { state, variables, sheet_metal })
}

/// A feature's context as its entry, its shapes put in `store` (once, `memo` across a writer's entries).
pub fn encode_context(store: std::sync::Arc<dyn super::ResultStore>, memo: &mut ShapeMemo, ctx: &Context) -> Option<Vec<u8>> {
    if !storable(ctx) {
        return None;
    }
    let writer = BlobWriter { store, memo: std::mem::take(memo), hashes: Vec::new() };
    let (json, codec) = noshape_kernel::with_shape_codec(Box::new(writer), || serde_json::to_vec(ctx));
    let any: Box<dyn std::any::Any> = codec;
    let writer = any.downcast::<BlobWriter>().ok()?;
    *memo = writer.memo;
    let json = zstd::encode_all(&json.ok()?[..], 3).ok()?;
    let mut out = Vec::with_capacity(json.len() + 16 * writer.hashes.len() + 48);
    out.extend_from_slice(CONTEXT_MAGIC);
    out.extend_from_slice(&(json.len() as u64).to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(writer.hashes.len() as u64).to_le_bytes());
    for h in &writer.hashes {
        out.extend_from_slice(h);
    }
    out.extend_from_slice(&blake3::hash(&out).as_bytes()[..16]);
    Some(out)
}

/// What [`encode_context`] wrote, its shapes from `store`; `None` for anything missing, cut short or changed.
pub fn decode_context(store: std::sync::Arc<dyn super::ResultStore>, bytes: &[u8]) -> Option<Context> {
    let (body, sum) = bytes.split_at_checked(bytes.len().checked_sub(16)?)?;
    if blake3::hash(body).as_bytes()[..16] != *sum {
        return None;
    }
    let mut r = body.strip_prefix(CONTEXT_MAGIC.as_slice())?;
    let mut take = |n: usize| -> Option<&[u8]> {
        let (a, b) = r.split_at_checked(n)?;
        r = b;
        Some(a)
    };
    let n = u64::from_le_bytes(take(8)?.try_into().ok()?) as usize;
    let json = zstd::decode_all(take(n)?).ok()?;
    let count = u64::from_le_bytes(take(8)?.try_into().ok()?) as usize;
    let mut hashes = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        hashes.push(take(16)?.try_into().ok()?);
    }
    let reader = BlobReader { store, hashes };
    let (value, _) = noshape_kernel::with_shape_codec(Box::new(reader), || serde_json::from_slice::<Context>(&json));
    value.ok()
}

/// Keeps a full regeneration's result, and each feature it evaluated, in `store` on a background thread (encoding
/// big parts takes a while).
pub fn keep(store: std::sync::Arc<dyn super::ResultStore>, result: &super::RegenResult) {
    if result.cancelled || (result.fresh.is_empty() && (result.result_key.is_none() || result.from_store)) {
        return;
    }
    let result = result.clone();
    std::thread::Builder::new().name("noshape-keep-result".into()).spawn(move || keep_now(store, &result)).ok();
}

/// [`keep`] on this thread.
pub fn keep_now(store: std::sync::Arc<dyn super::ResultStore>, result: &super::RegenResult) {
    if result.cancelled {
        return;
    }
    // (The whole result first: a reopen needs only it.)
    if let Some(key) = result.result_key.filter(|_| !result.from_store)
        && let Some(bytes) = encode_result(&result.context, &result.display.features)
    {
        store.put(key, bytes);
    }
    let mut memo = ShapeMemo::default();
    for (key, ctx, state) in &result.fresh {
        // (The context first: a state is only read with its context there.)
        if let Some(bytes) = encode_context(store.clone(), &mut memo, ctx)
            && let Some(light) = encode_feature_state(state, ctx)
        {
            store.put(context_key(*key), bytes);
            store.put(feature_key(*key), light);
        }
    }
}
