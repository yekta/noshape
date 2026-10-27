//! noshape-drawing: drawing views (HLR projection), sheet building, tables, inspection, PDF/DXF/DWG/DWT export.
//! The drawing data types are in `noshape_model_data::drawing` (re-exported by [`drawing`]).
//!
//! The model core's modules (and noshape-assembly's `assembly`, noshape-custom's `custom`) are imported at the crate root, so `crate::document`, `crate::regen`, … resolve here as
//! they did inside noshape-model.

#[allow(unused_imports)]
use noshape_model_core::*;
use noshape_assembly::assembly;
use noshape_custom::custom;

pub mod demo;
pub mod drawing;

/// The extension traits that give drawing data their logic methods.
pub mod prelude {
    pub use crate::drawing::{CadTables, DrawingClipboard, ModelAnnotations, SheetPaste, ViewMoves};
}
