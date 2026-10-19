//! FeatureScript: the language of Onshape's custom features.
//!
//! * [`lexer`], [`parser`], [`ast`]: the language syntax.
//! * [`compile`]: modules, imports and name resolution into [`ir`].
//! * [`interp`]: the evaluator; [`value`]: values, equality and ordering;
//!   [`natives`]: runtime builtins that need no modeling kernel (the host
//!   registers the modeling ones with [`interp::register_natives`]).
//! * The Onshape FeatureScript Standard Library sources (MIT License, see
//!   `std/LICENSE`) are embedded as [`STD_SOURCES`] and run unmodified.

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

#[cfg(test)]
mod tests;
