//! Derived parts: access to other Part Studios of the same document.

use super::context::Context;
use std::sync::Arc;

/// Supplies regenerated contexts of other Part Studios, identified by the
/// tab id string stored in a Derived feature's `partStudio` parameter.
pub trait DerivedProvider: Send + Sync + std::fmt::Debug {
    /// Regenerated final context of the source Part Studio.
    fn context(&self, tab: &str) -> Option<Arc<Context>>;
    /// A hash that changes whenever the source's result may change.
    fn version(&self, tab: &str) -> u64;
}
