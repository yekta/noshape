//! Access to a document's compiled Feature Studios during regeneration.

use super::compile::{CompiledStudio, compile};
use crate::document::{Document, TabContent, TabId};
use crate::features::FeatureType;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

/// Most recently compiled studio per tab id string: the fallback for
/// regenerations that get no provider (assembly and drawing evaluation).
static LATEST: LazyLock<RwLock<HashMap<String, Arc<CompiledStudio>>>> = LazyLock::new(Default::default);

/// Fallback provider backed by the most recently compiled studios.
#[derive(Debug)]
pub(super) struct Latest;

impl CustomFeatureProvider for Latest {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>> {
        LATEST.read().get(tab).cloned()
    }
}

/// Supplies compiled Feature Studios by tab id string (the `<tab>` part of
/// `custom:<tab>:<name>`). Passed to regeneration in
/// `RegenOptions::custom_features`, like the Derived provider.
pub trait CustomFeatureProvider: Send + Sync + std::fmt::Debug {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>>;
    /// Changes whenever the studio's compiled result may change.
    fn version(&self, tab: &str) -> u64 {
        self.studio(tab).map(|s| s.hash).unwrap_or(0)
    }
}

/// Every Feature Studio of a document, compiled (cached by source hash).
/// Creating it also registers the declared feature schemas, so
/// `features::get("custom:...")` and new-feature creation know them.
#[derive(Debug, Default)]
pub struct DocumentScripts {
    studios: Vec<(TabId, String, Arc<CompiledStudio>)>,
}

impl DocumentScripts {
    pub fn from_document(doc: &Document) -> Arc<Self> {
        let mut studios = Vec::new();
        for t in doc.ordered_tabs() {
            if let TabContent::FeatureStudio(fs) = &t.content {
                let c = compile(t.id, &fs.source);
                // A studio with errors keeps its last good schemas active, so
                // features using it stay editable while the source is fixed.
                if !c.has_errors() {
                    let types: Vec<&'static FeatureType> = c.features.iter().map(|f| f.feature_type).collect();
                    super::schema::activate(t.id, &types);
                }
                LATEST.write().insert(t.id.to_string(), c.clone());
                studios.push((t.id, t.name.clone(), c));
            }
        }
        Arc::new(Self { studios })
    }

    /// (tab, tab name, compiled studio) in tab order.
    pub fn studios(&self) -> impl Iterator<Item = (TabId, &str, &Arc<CompiledStudio>)> {
        self.studios.iter().map(|(t, n, c)| (*t, n.as_str(), c))
    }

    /// Feature types declared by studios that compile without errors, for
    /// toolbars and tool search.
    pub fn feature_types(&self) -> Vec<&'static FeatureType> {
        self.studios.iter().filter(|(_, _, c)| !c.has_errors()).flat_map(|(_, _, c)| c.features.iter().map(|f| f.feature_type)).collect()
    }
}

impl CustomFeatureProvider for DocumentScripts {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>> {
        self.studios.iter().find(|(t, _, _)| t.to_string() == tab).map(|(_, _, c)| c.clone())
    }
}
