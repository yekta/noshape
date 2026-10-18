//! Feature Studios: document-scoped custom features written in a sandboxed
//! script language ([Rhai](https://rhai.rs)), noshape's FeatureScript
//! equivalent.
//!
//! * A Feature Studio tab ([`FeatureStudio`]) holds script source. Compiling
//!   it ([`compile`]) runs its top level once, which declares features with
//!   `feature(name, #{ params: [...], evaluate: ... })`.
//! * Every declared feature becomes a feature type with id
//!   `custom:<tab id>:<feature name>` (see [`type_id`]). Its parameter
//!   schema is built at run time and registered so `features::get` (and
//!   therefore the generic feature dialog) knows it.
//! * Part Studios evaluate custom features through a
//!   [`CustomFeatureProvider`] passed in `RegenOptions::custom_features`.
//!   The script's `evaluate(ctx, params)` function calls kernel operations
//!   through the `ctx` object; results are named with the regular persistent
//!   naming machinery, keyed by an operation counter of the script run.
//!
//! The script API is documented in `docs/custom-features.md`.

mod api;
mod compile;
mod eval;
mod provider;
mod schema;

pub use compile::{CompiledStudio, CustomFeatureDef, DeclKind, Diagnostic, ParamDecl, Severity, compile};
pub use eval::evaluate;
pub use provider::{CustomFeatureProvider, DocumentScripts};
pub use schema::lookup;

/// Provider for regenerations without an explicit one: the most recently
/// compiled Feature Studios (registered by [`DocumentScripts::from_document`]).
pub fn fallback_provider() -> std::sync::Arc<dyn CustomFeatureProvider> {
    std::sync::Arc::new(provider::Latest)
}

use crate::document::TabId;
use serde::{Deserialize, Serialize};

/// Content of a Feature Studio tab.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FeatureStudio {
    pub source: String,
}

impl FeatureStudio {
    /// Source of a new Feature Studio: a small working example.
    pub fn template() -> Self {
        Self { source: TEMPLATE.to_string() }
    }
}

/// Prefix of custom feature type ids.
pub const TYPE_PREFIX: &str = "custom:";

/// Feature type id of a custom feature: `custom:<tab id>:<feature name>`.
pub fn type_id(tab: TabId, name: &str) -> String {
    format!("{TYPE_PREFIX}{tab}:{name}")
}

/// Whether a feature type id names a custom feature.
pub fn is_custom(type_id: &str) -> bool {
    type_id.starts_with(TYPE_PREFIX)
}

/// Split a custom type id into (tab id string, feature name).
pub fn parse_type_id(type_id: &str) -> Option<(&str, &str)> {
    type_id.strip_prefix(TYPE_PREFIX)?.split_once(':')
}

const TEMPLATE: &str = r#"// Feature Studio: custom features for this document's Part Studios.
// See docs/custom-features.md for the script API. Lengths are in meters
// (use mm(), inch() ...) and angles in radians (use deg()).

feature("Rounded block", #{
    icon: "fillet",
    description: "A block with rounded vertical edges",
    params: [
        length("width", "Width", mm(40)),
        length("depth", "Depth", mm(25)),
        length("height", "Height", mm(10)),
        length("radius", "Corner radius", mm(4)),
    ],
    evaluate: "rounded_block",
});

fn rounded_block(ctx, p) {
    let body = ctx.box([0, 0, 0], [p.width, p.depth, p.height]);
    // Vertical edges are the lines parallel to Z.
    let vertical = [];
    for e in ctx.filter(ctx.edges(body), "LINE") {
        let a = ctx.axis(e);
        if abs(a.direction[2]) > 0.99 { vertical.push(e); }
    }
    if p.radius > 0.0 { ctx.fillet(vertical, p.radius); }
}
"#;
