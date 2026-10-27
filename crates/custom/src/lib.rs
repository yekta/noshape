//! noshape-custom: the FeatureScript host. Compiling Feature Studios, custom features and custom tables, the
//! modeling natives std calls, and the built-in features std runs. The Feature Studio data types are in
//! `noshape_model_data::custom` (re-exported by [`custom`]); what regeneration needs of the host (the compiled studio
//! types, `CustomFeatureProvider`, the `CustomHost` hook) is in the core's `regen::custom_features`.
//!
//! The model core's modules are imported at the crate root, so `crate::document`, `crate::regen`, … resolve here as
//! they did inside noshape-model.

#[allow(unused_imports)]
use noshape_model_core::*;

pub mod custom;
pub mod demo;

pub use custom::register;
