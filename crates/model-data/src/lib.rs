//! noshape-model-data: the document data model, below all evaluation logic.
//!
//! * [`document`]: documents and tabs; [`partstudio`]: feature lists; [`features`]: feature schemas;
//!   [`params`]: generic parameter values; [`reference`]: persistent naming.
//! * [`assembly`], [`drawing`], [`custom`]: the data of Assembly, Drawing and Feature Studio tabs.
//! * [`variables`], [`configuration`], [`configured_properties`], [`variable_table`].
//!
//! Regeneration, assembly solving, drawing views and FeatureScript evaluation live in noshape-model, which
//! re-exports every module here under the same path (`noshape_model::document`, …).

pub mod annotations;
pub mod assembly;
pub mod configuration;
pub mod configured_properties;
pub mod custom;
pub mod default_names;
pub mod document;
pub mod drawing;
pub mod features;
pub mod file_tab;
pub mod fits;
pub mod in_context;
pub mod material_library;
pub mod naming;
pub mod params;
pub mod partstudio;
pub mod quantity_text;
pub mod reference;
pub mod sketch_feature;
pub mod variable_table;
pub mod variables;

pub use document::{Document, Tab, TabContent, TabId};
pub use params::{ParamSpec, ParamValue, ParamValues};
pub use partstudio::{Feature, PartStudio};
pub use reference::{BodyId, FeatureId, Reference, TopoName};
