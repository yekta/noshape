//! FeatureScript: the language of Onshape's custom features.
//!
//! * [`lexer`], [`parser`], [`ast`]: the language syntax.
//! * [`compile`]: modules, imports and name resolution into [`ir`].
//! * [`interp`]: the evaluator; [`value`]: values, equality and ordering;
//!   [`natives`]: runtime builtins that need no modeling kernel (the host
//!   registers the modeling ones with [`interp::register_natives`]).
//! * The Onshape FeatureScript Standard Library sources (MIT License, see
//!   `std/LICENSE`) are embedded as [`STD_SOURCES`] and run unmodified; older
//!   releases that Feature Studios pin are shipped packed against it
//!   (`std-versions/<version>.zst`, [`std_files`]).

pub mod annot;
pub mod ast;
pub mod compile;
pub mod interp;
pub mod ir;
pub mod lexer;
pub mod natives;
pub mod parser;
mod spline;
mod std_ops;
pub mod value;

pub use compile::{CompileError, CompiledModule, Level, compile_module, std_module};
pub use interp::{Exception, Interp, Notice, NoticeKind};
pub use value::Value;

/// The embedded standard library version (`FeatureScript 3095;`).
pub const STD_VERSION: u32 = 3095;
/// Value of `@getLanguageVersion()`.
pub const LANGUAGE_VERSION: u32 = 3095;

include!(concat!(env!("OUT_DIR"), "/std_sources.rs"));

/// Source of a standard library module (`"geometry.fs"`).
pub fn std_source(name: &str) -> Option<&'static str> {
    STD_SOURCES.binary_search_by(|(n, _)| (*n).cmp(name)).ok().map(|i| STD_SOURCES[i].1)
}

/// The standard library releases shipped (the embedded one last), oldest first.
pub fn std_versions() -> Vec<u32> {
    STD_PACKS.iter().map(|(v, _)| *v).chain([STD_VERSION]).collect()
}

/// The standard library release served for an import pinned to `version`: the newest shipped at or below it, as
/// Onshape serves exactly the pin (a module compiles against the std of its own version); a pin older than every
/// release shipped gets the oldest.
pub fn std_version_for(version: u32) -> u32 {
    let shipped = std_versions();
    shipped.iter().copied().filter(|v| *v <= version).max().or_else(|| shipped.first().copied()).unwrap_or(STD_VERSION)
}

/// Every module (file name, source) of a shipped release, sorted by name; an older release is unpacked the first
/// time it's asked for (only studios that pin one pay for it).
pub fn std_files(version: u32) -> Option<&'static [(&'static str, &'static str)]> {
    type Unpacked = std::collections::HashMap<u32, &'static [(&'static str, &'static str)]>;
    static UNPACKED: std::sync::LazyLock<parking_lot::Mutex<Unpacked>> = std::sync::LazyLock::new(Default::default);
    if version == STD_VERSION {
        return Some(STD_SOURCES);
    }
    let (_, pack) = STD_PACKS.iter().find(|(v, _)| *v == version)?;
    let mut unpacked = UNPACKED.lock();
    if let Some(files) = unpacked.get(&version) {
        return Some(files);
    }
    let files: &'static [(&'static str, &'static str)] = Vec::leak(unpack(pack).unwrap_or_else(|e| panic!("std {version}: {e}")));
    unpacked.insert(version, files);
    Some(files)
}

/// The modules of a packed release: `pack` decompressed with the embedded release's blob as its zstd prefix, then
/// split into (name, source) (tools/std-pack.py writes them).
fn unpack(pack: &[u8]) -> Result<Vec<(&'static str, &'static str)>, String> {
    use zstd::zstd_safe::{DCtx, DParameter};
    let prefix = blob(STD_SOURCES);
    let size = zstd::zstd_safe::get_frame_content_size(pack).map_err(|e| format!("{e:?}"))?.ok_or("no content size")? as usize;
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let mut d = DCtx::create();
    d.set_parameter(DParameter::WindowLogMax(27)).map_err(zstd::zstd_safe::get_error_name)?;
    d.ref_prefix(&prefix).map_err(zstd::zstd_safe::get_error_name)?;
    d.decompress(&mut out, pack).map_err(zstd::zstd_safe::get_error_name)?;
    let text: &'static str = String::leak(String::from_utf8(out).map_err(|e| e.to_string())?);
    let mut files = Vec::new();
    let mut rest = text;
    while let Some(r) = rest.strip_prefix('\0') {
        let (name, r) = r.split_once('\0').ok_or("no name")?;
        let (len, r) = r.split_once('\0').ok_or("no length")?;
        let len: usize = len.parse().map_err(|_| "bad length")?;
        let (src, r) = (r.get(..len).ok_or("short source")?, &r[len..]);
        files.push((name, src));
        rest = r;
    }
    if !rest.is_empty() {
        return Err("trailing bytes".into());
    }
    Ok(files)
}

/// The modules as one blob: each `\0<name>\0<byte length>\0<source>`, in order.
fn blob(files: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, src) in files {
        out.extend_from_slice(format!("\0{name}\0{}\0", src.len()).as_bytes());
        out.extend_from_slice(src.as_bytes());
    }
    out
}

/// Source of a standard library module at a shipped release.
pub fn std_source_at(version: u32, name: &str) -> Option<&'static str> {
    let files = std_files(version)?;
    files.binary_search_by(|(n, _)| (*n).cmp(name)).ok().map(|i| files[i].1)
}

#[cfg(test)]
mod tests;
