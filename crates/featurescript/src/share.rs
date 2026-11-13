//! NOSHAPE_FSSHARE: process-wide time counters (regeneration, FeatureScript features, their natives by kind).
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NOSHAPE_FSSHARE").is_some())
}

// regen, fs, op, query, ev, attr, other (nanoseconds)
static C: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];
pub const REGEN: usize = 0;
pub const FS: usize = 1;

pub fn add(k: usize, d: std::time::Duration) {
    C[k].fetch_add(d.as_nanos() as u64, Relaxed);
}

static OTHER: std::sync::Mutex<Option<std::collections::HashMap<String, f64>>> = std::sync::Mutex::new(None);
pub fn add_other(name: &str, d: std::time::Duration) {
    *OTHER.lock().unwrap().get_or_insert_with(Default::default).entry(name.to_string()).or_default() += d.as_secs_f64() * 1e3;
}
pub fn take_other() -> String {
    let m = OTHER.lock().unwrap().take().unwrap_or_default();
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v.iter().take(8).map(|(k, t)| format!("{k} {t:.0}")).collect::<Vec<_>>().join(", ")
}

pub fn native_kind(name: &str) -> usize {
    if name.starts_with("op") {
        2
    } else if name.contains("uery") || name.starts_with('q') {
        3
    } else if name.starts_with("ev") {
        4
    } else if name.contains("ttribute") {
        5
    } else {
        6
    }
}

thread_local! { pub static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }

/// "regen fs op query ev attr other" in ms, and the counters reset.
pub fn take() -> String {
    let v: Vec<f64> = C.iter().map(|c| c.swap(0, Relaxed) as f64 / 1e6).collect();
    format!("regen {:.1} fs {:.1} op {:.1} query {:.1} ev {:.1} attr {:.1} other {:.1}", v[0], v[1], v[2], v[3], v[4], v[5], v[6])
}
