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

use crate::document::TabId;
use serde::{Deserialize, Serialize};

/// The embedded standard library (Ctrl+click on a std symbol opens its source).
pub use noshape_featurescript::{STD_SOURCES, STD_VERSION};

/// Content of a Feature Studio tab.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FeatureStudio {
    /// Committed source: what Part Studios use.
    pub source: String,
    /// Onshape saves edits as you type but applies them only on Commit: the
    /// saved, not yet committed text (None: same as `source`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<String>,
}

impl FeatureStudio {
    /// Source of a new Feature Studio (Onshape's template).
    pub fn template() -> Self {
        Self::new(TEMPLATE)
    }

    pub fn new(source: &str) -> Self {
        Self { source: source.to_string(), draft: None }
    }

    /// The text the editor shows: the draft, else the committed source.
    pub fn text(&self) -> &str {
        self.draft.as_deref().unwrap_or(&self.source)
    }

    /// Save `text` as typed (a draft unless it equals the committed source).
    pub fn save(&mut self, text: String) {
        self.draft = (text != self.source).then_some(text);
    }

    /// Commit the draft. Returns whether anything changed.
    pub fn commit(&mut self) -> bool {
        match self.draft.take() {
            Some(d) => {
                self.source = d;
                true
            }
            None => false,
        }
    }

    pub fn has_uncommitted(&self) -> bool {
        self.draft.is_some()
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

/// Onshape's new Feature Studio: the version header and the common.fs
/// import, caret on line 4.
const TEMPLATE: &str = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n\n";

#[cfg(test)]
mod studio_commit_tests {
    use super::FeatureStudio;

    #[test]
    fn edits_are_saved_but_used_only_after_commit() {
        let mut f = FeatureStudio::new("a");
        f.save("b".into());
        assert_eq!((f.source.as_str(), f.text(), f.has_uncommitted()), ("a", "b", true));
        // Typing back to the committed text leaves nothing to commit.
        f.save("a".into());
        assert!(!f.has_uncommitted());
        f.save("c".into());
        assert!(f.commit());
        assert_eq!((f.source.as_str(), f.has_uncommitted()), ("c", false));
        assert!(!f.commit());
        // Old documents (no draft field) still load.
        let old: FeatureStudio = serde_json::from_str(r#"{"source":"x"}"#).unwrap();
        assert_eq!(old, FeatureStudio::new("x"));
    }
}
