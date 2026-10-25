//! FeatureScript: the language of Onshape's custom features.
//!
//! * [`lexer`], [`parser`], [`ast`]: the language syntax.
//! * [`compile`]: modules, imports and name resolution into [`ir`].
//! * [`interp`]: the evaluator; [`value`]: values, equality and ordering;
//!   [`natives`]: runtime builtins that need no modeling kernel (the host
//!   registers the modeling ones with [`interp::register_natives`]).
//! * The Onshape FeatureScript Standard Library sources (MIT License, see
//!   `std/LICENSE`) are embedded as [`STD_SOURCES`] and run unmodified; older
//!   releases that Feature Studios pin (`std-versions/<version>/`) as
//!   [`STD_VERSIONS`].

pub mod annot;
pub mod ast;
pub mod compile;
pub mod interp;
pub mod ir;
pub mod lexer;
pub mod natives;
pub mod parser;
mod spline;
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

/// The standard library version served for an import pinned to `version`.
/// Onshape loads exactly the pinned library (std 1963 still exports
/// HelixType, private in 3095); we serve the nearest embedded one (the newer
/// on a tie).
pub fn std_version_for(version: u32) -> u32 {
    STD_VERSIONS.iter().map(|(v, _)| *v).chain([STD_VERSION]).min_by_key(|v| (v.abs_diff(version), std::cmp::Reverse(*v))).unwrap_or(STD_VERSION)
}

/// Source of a standard library module at an embedded version.
pub fn std_source_at(version: u32, name: &str) -> Option<&'static str> {
    if version == STD_VERSION {
        return std_source(name);
    }
    let (_, files) = STD_VERSIONS.iter().find(|(v, _)| *v == version)?;
    files.binary_search_by(|(n, _)| (*n).cmp(name)).ok().map(|i| files[i].1)
}

#[cfg(test)]
mod tests;
