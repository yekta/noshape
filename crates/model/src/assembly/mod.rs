//! Assembly logic; the data types are in `noshape_model_data::assembly` (re-exported here).

pub use noshape_model_data::assembly::*;
pub use noshape_model_data::assembly::{config, ops};

pub mod analysis;
pub mod bom;
pub mod eval;
pub mod solver;
pub mod standard;
#[cfg(test)]
mod tests;
