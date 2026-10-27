//! Drawing logic; the data types are in `noshape_model_data::drawing` (re-exported here).

pub use noshape_model_data::drawing::*;
pub use noshape_model_data::drawing::{links, sheet, note, weld, tb_lang, sheet_sketch, imported, templates, font, shx, hatch_pat, format, cad_objects, cad_layers, sketch_dims, inspection};

pub mod build;
mod moves;
pub use moves::ViewMoves;
pub use tables::CadTables;
pub use clipboard::{DrawingClipboard, SheetPaste};
pub use mbd::ModelAnnotations;
pub mod clipboard;
pub mod create;
mod cad_dimstyles;
pub mod cad_export;
pub mod cad_fit;
pub mod dwt;
pub mod mbd;
pub mod export;
pub mod painter;
pub mod pdf_font;
pub mod showcase;
pub mod tables;
pub mod views;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod links_tests;
