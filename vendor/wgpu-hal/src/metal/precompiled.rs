//! NOSHAPE PATCH (agent psui): Metal libraries compiled ahead of time.
//!
//! Compiling the MSL naga writes takes the Metal compiler ~0.5 s for the
//! viewport's first pipelines on a first launch (macOS caches it after).
//! The release job writes each MSL source (`NOSHAPE_MSL_DUMP=<dir>`: one
//! `<key>.metal` per source), compiles them with `xcrun metal` and ships
//! `<key>.metallib` in the app's `Contents/Resources/metallib`
//! (`NOSHAPE_METALLIB_DIR` elsewhere); [`library`] loads the one for a source
//! instead of compiling it. The key is a hash of the source without naga's
//! `// language:` line (the same MSL on macOS 14 and 15), so a library is
//! only ever used for the very source it was compiled from; any other
//! source compiles as before.

use alloc::{format, string::String};
use std::{
    eprintln,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLDevice, MTLLibrary};

/// Libraries loaded and sources compiled, for logs and tests.
pub static HITS: AtomicUsize = AtomicUsize::new(0);
pub static MISSES: AtomicUsize = AtomicUsize::new(0);

/// FNV-1a over the source after its `// language:` line.
pub fn key(source: &str) -> String {
    let body = match source.strip_prefix("// language:") {
        Some(rest) => rest.split_once('\n').map_or("", |(_, b)| b),
        None => source,
    };
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in body.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        if let Some(d) = std::env::var_os("NOSHAPE_METALLIB_DIR") {
            return Some(PathBuf::from(d));
        }
        // noshape.app/Contents/MacOS/noshape -> noshape.app/Contents/Resources/metallib
        let exe = std::env::current_exe().ok()?;
        let d = exe.parent()?.parent()?.join("Resources").join("metallib");
        d.is_dir().then_some(d)
    })
    .as_ref()
}

/// Writes `source` to `NOSHAPE_MSL_DUMP`'s directory, if set.
pub fn dump(source: &str) {
    if let Some(d) = std::env::var_os("NOSHAPE_MSL_DUMP") {
        let d = PathBuf::from(d);
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join(format!("{}.metal", key(source))), source);
    }
}

/// The library compiled ahead of time from `source`, if one ships.
pub fn library(
    device: &ProtocolObject<dyn MTLDevice>,
    source: &str,
) -> Option<Retained<ProtocolObject<dyn MTLLibrary>>> {
    let found = dir()
        .and_then(|d| std::fs::read(d.join(format!("{}.metallib", key(source)))).ok())
        .and_then(|bytes| {
            super::library_from_metallib::new_library_from_metallib_bytes(device, &bytes).ok()
        });
    if found.is_some() {
        HITS.fetch_add(1, Ordering::Relaxed);
    } else {
        MISSES.fetch_add(1, Ordering::Relaxed);
    }
    if std::env::var_os("NOSHAPE_METALLIB_LOG").is_some() {
        eprintln!("metallib {}: {}", key(source), if found.is_some() { "precompiled" } else { "compiled now" });
    }
    found
}
