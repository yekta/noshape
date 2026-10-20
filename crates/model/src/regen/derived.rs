//! Derived parts: access to other Part Studios of the same document.

use super::context::Context;
use std::sync::Arc;

/// Supplies regenerated contexts of other Part Studios, identified by the
/// tab id string stored in a Derived feature's `partStudio` parameter.
pub trait DerivedProvider: Send + Sync + std::fmt::Debug {
    /// Regenerated final context of the source Part Studio.
    fn context(&self, tab: &str) -> Option<Arc<Context>>;
    /// The source regenerated in a configuration (a configured Part
    /// Studio's Derived feature); unset inputs take their defaults.
    fn context_configured(&self, tab: &str, values: &crate::configuration::ConfigValues) -> Option<Arc<Context>> {
        let _ = values;
        self.context(tab)
    }
    /// A hash that changes whenever the source's result may change.
    fn version(&self, tab: &str) -> u64;
    /// A file tab of another document or version (`ext:` keys), for
    /// FeatureScript reference parameters.
    fn file(&self, _key: &str) -> Option<crate::file_tab::FileTab> {
        None
    }
    /// The source Part Studio's configuration inputs (PartStudioData).
    fn configuration(&self, _tab: &str) -> Option<crate::configuration::Configuration> {
        None
    }
    /// The type of a feature of the source Part Studio (`extrude`, …), for
    /// FeatureScript part queries (`<feature>.opExtrude`).
    fn feature_type(&self, _tab: &str, _feature: crate::reference::FeatureId) -> Option<String> {
        None
    }
}
