//! The persistent regeneration cache's store (lead 11-11, docs/handoff/core.md "Design 11-11"): a Part Studio's whole
//! regeneration kept on disk under its key, so reopening the document shows its parts live without regenerating.
//! Kept in the user cache directory (`paths::cache_dir()/results`), outside the document, per build, at most
//! [`CAP_BYTES`] ([`noshape_model::regen::persist::DirStore`]).

use noshape_model::regen::ResultStore;
use noshape_model::regen::persist::DirStore;
use std::sync::Arc;

/// The store's size bound.
const CAP_BYTES: u64 = 2 << 30;

/// The app's store (`None` in tests and the parity driver, as the display cache).
pub fn store() -> Option<Arc<dyn ResultStore>> {
    static STORE: std::sync::OnceLock<Arc<DirStore>> = std::sync::OnceLock::new();
    if !super::display_cache::enabled() {
        return None;
    }
    let s = STORE.get_or_init(|| {
        let build = blake3::hash(super::display_cache::build_id().as_bytes()).to_hex()[..16].to_string();
        Arc::new(DirStore { dir: crate::paths::cache_dir().join("results"), build, cap: CAP_BYTES })
    });
    Some(s.clone())
}

/// Keeps a final regeneration's result in the store (off this thread).
pub fn keep(store: Arc<dyn ResultStore>, result: &noshape_model::regen::RegenResult) {
    noshape_model::regen::persist::keep(store, result);
}
