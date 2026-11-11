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
}

/// Keeps a full regeneration's result in `store` on a background thread (encoding a big part takes a while).
pub fn keep(store: std::sync::Arc<dyn super::ResultStore>, result: &super::RegenResult) {
    let (Some(key), false, false) = (result.result_key, result.from_store, result.cancelled) else { return };
    let (ctx, states) = (result.context.clone(), result.display.features.clone());
    std::thread::Builder::new()
        .name("noshape-keep-result".into())
        .spawn(move || {
            if let Some(bytes) = encode_result(&ctx, &states) {
                store.put(key, bytes);
            }
        })
        .ok();
}
