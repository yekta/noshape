//! Onshape's remembered feature settings (live 1987): a feature dialog's parameters marked REMEMBER_PREVIOUS_VALUE
//! are saved on ✓ (never ✕), per user and per feature type, and a new feature of that type opens with them in any
//! document. Values without units carry everywhere; lengths and angles are kept per the document's length / angle
//! unit. Kept in `feature_settings.json` in the config directory, read once and written off the UI thread; only the
//! app keeps them across sessions ([`enable`]), so tests stay independent.

use noshape_model::params::ParamValue;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// A feature type's remembered values: group ("" unitless, "length:<unit>", "angle:<unit>") → parameter → value.
pub type Groups = BTreeMap<String, BTreeMap<String, ParamValue>>;

static PERSIST: AtomicBool = AtomicBool::new(false);
static STORE: Mutex<Option<BTreeMap<String, Groups>>> = Mutex::new(None);

/// The app keeps them across sessions (called once at startup).
pub fn enable() {
    PERSIST.store(true, Ordering::Relaxed);
}

fn path() -> std::path::PathBuf {
    crate::paths::config_dir().join("feature_settings.json")
}

fn with_store<R>(f: impl FnOnce(&mut BTreeMap<String, Groups>) -> R) -> R {
    let mut g = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = g.get_or_insert_with(|| std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
    f(store)
}

/// What the user last committed in feature type `key` (empty in tests or before any commit).
pub fn get(key: &str) -> Groups {
    if !PERSIST.load(Ordering::Relaxed) {
        return Groups::new();
    }
    with_store(|s| s.get(key).cloned().unwrap_or_default())
}

/// A commit's groups replace the same groups of `key` (other units' lists stay) and are written to disk.
pub fn put(key: &str, groups: &Groups) {
    if !PERSIST.load(Ordering::Relaxed) {
        return;
    }
    let bytes = with_store(|s| {
        let entry = s.entry(key.to_string()).or_default();
        for (g, values) in groups {
            entry.insert(g.clone(), values.clone());
        }
        serde_json::to_vec_pretty(s).unwrap_or_default()
    });
    let p = path();
    std::thread::spawn(move || {
        let _ = std::fs::write(p, bytes);
    });
}
