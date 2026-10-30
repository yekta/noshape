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
//! * Part Studios evaluate custom features (regeneration calls the host
//!   [`register`] installs) through a
//!   [`CustomFeatureProvider`] passed in `RegenOptions::custom_features`;
//!   [`run`] calls the feature with a `Context` over the regeneration
//!   context, and [`host`] implements the modeling natives.

pub mod host;
mod provider;
pub(crate) mod reference;
pub mod run;
mod sm_cut;
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
    register();
    std::sync::Arc::new(provider::Latest)
}

/// Install the host's hooks in the layers below (idempotent, cheap): custom feature schemas for `features::get`
/// (noshape-model-data) and the [`crate::regen::custom_features::CustomHost`] regeneration evaluates custom features
/// and the features std runs through. Every compile, [`DocumentScripts::from_document`], std's prewarm and switches,
/// the natives' registration and [`fallback_provider`] call it; the app and the agent API (`Ctx::new`,
/// `Session::open`) call it at startup, so a regeneration that comes first (a std-run Flex PCB feature) finds the
/// host. Without it such a feature fails with `regen::custom_features::NO_HOST`.
pub fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        crate::features::set_custom_lookup(lookup);
        crate::regen::custom_features::set_host(crate::regen::custom_features::CustomHost {
            fallback_provider: || std::sync::Arc::new(provider::Latest),
            evaluate_custom: run::evaluate,
            evaluate_std: std_feature::evaluate,
            sheet_metal_cut: sm_cut::cut,
        });
    });
}

pub use noshape_model_data::custom::*;
