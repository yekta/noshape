//! Derived-part provider: regenerates other Part Studios of the document on
//! demand (cached per tab) so Derived features can import their parts.

use noshape_model::Document;
use noshape_model::TabContent;
use noshape_model::regen::context::Context;
use noshape_model::regen::derived::DerivedProvider;
use noshape_model::regen::{RegenOptions, Regenerator};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Regenerators per source tab, shared by every view of a document.
pub type StudioCache = Arc<Mutex<HashMap<String, Regenerator>>>;

pub struct DocStudios {
    doc: Arc<Document>,
    cache: Arc<Mutex<HashMap<String, Regenerator>>>,
    /// Tabs currently being regenerated (cycle detection).
    active: Arc<Mutex<HashSet<String>>>,
}

impl std::fmt::Debug for DocStudios {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DocStudios")
    }
}

impl DocStudios {
    pub fn new(doc: Document, cache: StudioCache) -> Arc<Self> {
        Arc::new(Self { doc: Arc::new(doc), cache, active: Default::default() })
    }

    fn studio(&self, tab: &str) -> Option<&noshape_model::PartStudio> {
        self.doc.tabs.iter().find(|t| t.id.to_string() == tab).and_then(|t| match &t.content {
            TabContent::PartStudio(ps) => Some(ps),
            _ => None,
        })
    }

    /// Part Studios available as Derived sources: (tab id, name).
    pub fn sources(doc: &Document) -> Vec<(String, String)> {
        doc.ordered_tabs().into_iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| (t.id.to_string(), t.name.clone())).collect()
    }
}

impl DerivedProvider for DocStudios {
    fn context(&self, tab: &str) -> Option<Arc<Context>> {
        let studio = self.studio(tab)?.clone();
        if !self.active.lock().insert(tab.to_string()) {
            return None; // circular reference
        }
        let opts = RegenOptions {
            document_variables: noshape_model::variables::document_variables(&self.doc),
            derived: Some(Arc::new(DocStudios { doc: self.doc.clone(), cache: self.cache.clone(), active: self.active.clone() })),
            ..Default::default()
        };
        let mut reg = self.cache.lock().remove(tab).unwrap_or_default();
        let r = reg.regenerate(&studio, &studio.configuration.defaults(), &self.doc.settings.units, &opts);
        self.cache.lock().insert(tab.to_string(), reg);
        self.active.lock().remove(tab);
        Some(r.context)
    }

    fn version(&self, tab: &str) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(ps) = self.studio(tab) {
            serde_json::to_string(ps).unwrap_or_default().hash(&mut h);
            // Nested derived sources and variable studios also matter.
            for t in &self.doc.tabs {
                if let TabContent::VariableStudio(v) = &t.content {
                    serde_json::to_string(v).unwrap_or_default().hash(&mut h);
                }
            }
            for f in ps.features.iter().filter(|f| f.type_id == "importDerived") {
                let src = f.params.text("partStudio").to_string();
                if src != tab && self.active.lock().insert(src.clone()) {
                    self.version(&src).hash(&mut h);
                    self.active.lock().remove(&src);
                }
            }
        }
        h.finish()
    }
}
