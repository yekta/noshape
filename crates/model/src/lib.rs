//! noshape-model: the headless CAD document model.
//!
//! * [`document`]: documents and tabs.
//! * [`partstudio`]: feature lists; [`features`]: feature schemas;
//!   [`params`]: generic parameter values; [`reference`]: persistent naming.
//! * [`assembly`], [`drawing`], [`variables`], [`configuration`].

pub mod analysis;
pub mod assembly;
pub mod compare;
pub mod configuration;
pub mod custom;
pub mod demo;
pub mod demo_fab;
pub mod display;
pub mod document;
pub mod drawing;
pub mod features;
pub mod interaction;
pub mod merge;
pub mod naming;
pub mod params;
pub mod partstudio;
pub mod reference;
pub mod regen;
pub mod select;
pub mod sheetmetal;
pub mod sketch_feature;
pub mod variables;

pub use document::{Document, Tab, TabContent, TabId};
pub use params::{ParamSpec, ParamValue, ParamValues};
pub use partstudio::{Feature, PartStudio};
pub use reference::{BodyId, FeatureId, Reference, TopoName};
