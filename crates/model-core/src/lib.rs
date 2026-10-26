//! noshape-model-core: regeneration and the rest of the model logic (drawings are noshape-drawing; the
//! noshape-model facade puts everything under one path).
//!
//! * [`document`]: documents and tabs.
//! * [`partstudio`]: feature lists; [`features`]: feature schemas;
//!   [`params`]: generic parameter values; [`reference`]: persistent naming.
//! * [`assembly`], [`drawing`], [`variables`], [`configuration`].

pub use noshape_model_data::file_tab;
pub use noshape_model_data::quantity_text;
pub mod analysis;
pub mod thickness;
pub mod flatten;
pub mod interference;
pub use noshape_model_data::fits;
pub mod release;
pub mod assembly;
pub mod compare;
pub use noshape_model_data::configuration;
pub mod custom;
pub mod demo;
pub use noshape_model_data::default_names;
pub mod demo_fab;
pub mod display;
pub use noshape_model_data::document;
/// Drawing data (the views, sheet building and export are in noshape-drawing).
pub use noshape_model_data::drawing;
pub mod export_rules;
pub use noshape_model_data::features;
pub mod in_context;
pub mod interaction;
pub use noshape_model_data::material_library;
pub mod merge;
pub mod native_assembly;
pub use noshape_model_data::naming;
pub use noshape_model_data::params;
pub use noshape_model_data::partstudio;
pub use noshape_model_data::reference;
pub mod regen;
pub mod select;
pub mod sheetmetal;
pub use noshape_model_data::sketch_feature;
pub use noshape_model_data::variables;
pub use noshape_model_data::annotations;
pub use noshape_model_data::configured_properties;
pub use noshape_model_data::variable_table;

pub use document::{Document, Tab, TabContent, TabId};
pub use params::{ParamSpec, ParamValue, ParamValues};
pub use partstudio::{Feature, PartStudio};
pub use reference::{BodyId, FeatureId, Reference, TopoName};

#[cfg(test)]
mod configuration_tests;

/// The extension traits that give data types (`noshape_model_data`) their logic methods: `use noshape_model::prelude::*;`.
pub mod prelude {
    pub use crate::in_context::ContextBodies;
}
