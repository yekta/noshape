//! noshape-assembly: assembly evaluation, the mate solver, analysis, BOM, standard content, native assembly import and
//! in-context snapshots. The assembly data types are in `noshape_model_data::assembly` (re-exported by [`assembly`]).
//!
//! The model core's modules are imported at the crate root, so `crate::document`, `crate::regen`, … resolve here as
//! they did inside noshape-model.

#[allow(unused_imports)]
use noshape_model_core::*;

pub mod assembly;
pub mod demo;
pub mod in_context;
pub mod native_assembly;
