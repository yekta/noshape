//! Feature Studios: document-scoped custom features written in
//! FeatureScript, Onshape's feature language (`noshape-featurescript`).
//!
//! * A Feature Studio tab ([`FeatureStudio`]) holds FeatureScript source.
//!   Compiling it ([`compile`], [`studio`]) loads the module (and the
//!   standard library it imports) and finds its features: exported
//!   constants made with `defineFeature`, annotated with a
//!   "Feature Type Name". Their dialogs come from the preconditions.
//! * Every feature becomes a feature type `custom:<tab id>:<constant>`
//!   (see [`type_id`]) whose schema is registered so `features::get` and the
//!   generic feature dialog know it.
//! * Part Studios evaluate custom features through a
//!   [`CustomFeatureProvider`] passed in `RegenOptions::custom_features`;
//!   [`run`] calls the feature with a `Context` over the regeneration
//!   context, and [`host`] implements the modeling natives.

pub mod host;
mod provider;
pub(crate) mod reference;
pub mod run;
pub mod std_feature;
pub mod studio;
pub mod table;

pub use provider::{CustomFeatureProvider, DocumentScripts, external_tab, set_external_tab_loader};
pub use run::evaluate;
pub use noshape_featurescript::lexer::symbol_occurrences;
pub use studio::{CompiledStudio, CustomFeatureDef, Diagnostic, ParamDecl, Severity, compile, lookup};

/// Provider for regenerations without an explicit one: the most recently
/// compiled Feature Studios (registered by [`DocumentScripts::from_document`]).
pub fn fallback_provider() -> std::sync::Arc<dyn CustomFeatureProvider> {
    std::sync::Arc::new(provider::Latest)
}

pub use noshape_model_data::custom::*;
