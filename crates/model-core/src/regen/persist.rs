//! A regeneration context as bytes (the persistent regeneration cache's entries): a header, the context as JSON
//! (zstd), then its shapes as OCCT's binary B-rep, each once in a table the JSON numbers into, and a checksum. A
//! context holding FeatureScript attributes or variables isn't stored (they can hold functions): its feature
//! regenerates as before.

use super::context::Context;
use noshape_kernel::with_shape_blobs;

/// What starts every entry: a different start (another format) is never read.
const MAGIC: &[u8; 8] = b"NSCTX001";

/// `ctx` as bytes, or `None` where it can't be stored.
pub fn encode(ctx: &Context) -> Option<Vec<u8>> {
    if !ctx.fs_attributes.is_empty() || !ctx.fs_variables.is_empty() {
        return None;
    }
    let (json, blobs) = with_shape_blobs(Vec::new(), || serde_json::to_vec(ctx));
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

/// What [`encode`] wrote, or `None` for anything else (another format, cut short, changed).
pub fn decode(bytes: &[u8]) -> Option<Context> {
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
    let (ctx, _) = with_shape_blobs(blobs, || serde_json::from_slice::<Context>(&json));
    ctx.ok()
}
