//! Access to a document's compiled Feature Studios during regeneration.

use super::studio::{CompiledStudio, compile_with, module_path};
use crate::document::{Document, TabContent, TabId};
use crate::features::FeatureType;
use noshape_featurescript::compile::{CompiledModule, Resolver};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

/// Most recently compiled studio per tab id string: the fallback for
/// regenerations that get no provider (assembly and drawing evaluation).
static LATEST: LazyLock<RwLock<HashMap<String, Arc<CompiledStudio>>>> = LazyLock::new(Default::default);
/// File tabs seen by the latest [`DocumentScripts::from_document`] calls,
/// by tab id string (reference parameters in those regenerations).
static LATEST_FILES: LazyLock<RwLock<HashMap<String, crate::file_tab::FileTab>>> = LazyLock::new(Default::default);
static LATEST_FILES_HASH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Fallback provider backed by the most recently compiled studios.
#[derive(Debug)]
pub(super) struct Latest;

impl CustomFeatureProvider for Latest {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>> {
        LATEST.read().get(tab).cloned()
    }
    fn file(&self, tab: &str) -> Option<crate::file_tab::FileTab> {
        LATEST_FILES.read().get(tab).cloned()
    }
    fn version(&self, tab: &str) -> u64 {
        self.studio(tab).map(|s| s.hash).unwrap_or(0) ^ LATEST_FILES_HASH.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Supplies compiled Feature Studios by tab id string (the `<tab>` part of
/// `custom:<tab>:<name>`). Passed to regeneration in
/// `RegenOptions::custom_features`, like the Derived provider.
pub trait CustomFeatureProvider: Send + Sync + std::fmt::Debug {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>>;
    /// A file tab of the document, by tab id string (reference parameters).
    fn file(&self, _tab: &str) -> Option<crate::file_tab::FileTab> {
        None
    }
    /// Changes whenever the studio's compiled result may change.
    fn version(&self, tab: &str) -> u64 {
        self.studio(tab).map(|s| s.hash).unwrap_or(0)
    }
}

/// Imports between the Feature Studios of one document: an import path is
/// a tab id (`import(path : "<tab id>", version : "...")`).
struct DocResolver<'a> {
    studios: &'a [(TabId, String, String)],
    compiled: &'a HashMap<TabId, Arc<CompiledStudio>>,
    /// Other tabs a Feature Studio can import (Feature Studio › Import).
    others: &'a [(TabId, Importable)],
}

/// Loads a tab of another document or version (`ext:` import paths); set
/// by the app, which reads document snapshots.
static EXTERNAL_TABS: std::sync::OnceLock<fn(&str) -> Option<crate::document::Tab>> = std::sync::OnceLock::new();

pub fn set_external_tab_loader(f: fn(&str) -> Option<crate::document::Tab>) {
    let _ = EXTERNAL_TABS.set(f);
}

/// A tab of another document or version, by its `ext:` key.
pub fn external_tab(key: &str) -> Option<crate::document::Tab> {
    EXTERNAL_TABS.get().and_then(|f| f(key))
}

/// A non-FeatureScript tab that can be imported.
#[derive(Clone, Copy, Debug)]
enum Importable {
    /// A file tab: exports `BLOB_DATA` (the same map as a reference
    /// parameter's) and `processedForeingDataInfo` (live dom/fs-import).
    File,
    /// A Part Studio: exports `build(configuration)` returning its Context.
    PartStudio,
    /// A Variable Studio (its exports are not captured yet: none).
    VariableStudio,
}

/// The module an imported tab stands for. Part Studios' `build` sits on
/// line 6, which is what Onshape prints for it ("<function 6>").
fn imported_source(tab: impl std::fmt::Display, what: Importable) -> String {
    match what {
        Importable::File => format!("FeatureScript 3095;\nexport const BLOB_DATA = @blobData(\"{tab}\");\nexport const processedForeingDataInfo = @processedForeignDataInfo(\"{tab}\");\n"),
        Importable::PartStudio => format!("FeatureScript 3095;\n\n\n\n\nexport const build = function(configuration)\n{{\n    return @buildPartStudio(\"{tab}\", configuration);\n}};\n"),
        Importable::VariableStudio => "FeatureScript 3095;\n".to_string(),
    }
}

impl Resolver for DocResolver<'_> {
    fn resolve(&self, path: &str) -> Option<(Arc<str>, Arc<str>)> {
        if let Some((tab, _, src)) = self.studios.iter().find(|(t, _, _)| t.to_string() == path || module_path(*t) == path) {
            return Some((module_path(*tab).into(), src.as_str().into()));
        }
        // Another document or version (Import › Other documents, or a version).
        if path.starts_with("ext:") || path.contains('/') {
            let tab = external_tab(path)?;
            let src = match &tab.content {
                TabContent::FeatureStudio(fs) => fs.source.clone(),
                TabContent::File(_) => imported_source(path, Importable::File),
                TabContent::PartStudio(_) => imported_source(path, Importable::PartStudio),
                TabContent::VariableStudio(_) => imported_source(path, Importable::VariableStudio),
                _ => return None,
            };
            return Some((path.into(), src.into()));
        }
        let (tab, what) = self.others.iter().find(|(t, _)| t.to_string() == path || module_path(*t) == path)?;
        Some((module_path(*tab).into(), imported_source(*tab, *what).into()))
    }
    fn cached(&self, path: &str) -> Option<Arc<CompiledModule>> {
        let (tab, _, _) = self.studios.iter().find(|(t, _, _)| module_path(*t) == path)?;
        self.compiled.get(tab).and_then(|c| c.module.clone())
    }
}

/// Every Feature Studio of a document, compiled (cached by source hash).
/// Creating it also registers the declared feature schemas, so
/// `features::get("custom:...")` and new-feature creation know them.
#[derive(Debug, Default)]
pub struct DocumentScripts {
    studios: Vec<(TabId, String, Arc<CompiledStudio>)>,
    /// File tabs (reference parameters: CSV, image, JSON, ...).
    files: Vec<(TabId, crate::file_tab::FileTab)>,
    /// Changes when a file tab is added, removed or updated.
    files_hash: u64,
}

impl DocumentScripts {
    pub fn from_document(doc: &Document) -> Arc<Self> {
        let sources: Vec<(TabId, String, String)> = doc
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::FeatureStudio(fs) => Some((t.id, t.name.clone(), fs.source.clone())),
                _ => None,
            })
            .collect();
        let others: Vec<(TabId, Importable)> = doc
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::File(_) => Some((t.id, Importable::File)),
                TabContent::PartStudio(_) => Some((t.id, Importable::PartStudio)),
                TabContent::VariableStudio(_) => Some((t.id, Importable::VariableStudio)),
                _ => None,
            })
            .collect();
        // Imports may refer to any studio or importable tab: they take part in the cache key.
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (t, _, s) in &sources {
            t.hash(&mut h);
            s.hash(&mut h);
        }
        for (t, _) in &others {
            t.hash(&mut h);
        }
        // Imported files' BLOB_DATA is a constant: a changed file recompiles.
        for t in doc.tabs.iter() {
            if let TabContent::File(f) = &t.content {
                (t.id, &f.file_name, f.size, f.uploaded).hash(&mut h);
            }
        }
        let deps = if sources.len() > 1 || !others.is_empty() { h.finish() } else { 0 };
        let mut compiled: HashMap<TabId, Arc<CompiledStudio>> = HashMap::new();
        let mut studios = Vec::new();
        for (tab, name, src) in &sources {
            let c = {
                let resolver = DocResolver { studios: &sources, compiled: &compiled, others: &others };
                compile_with(*tab, src, &resolver, deps)
            };
            // A studio with errors keeps its last good schemas active, so
            // features using it stay editable while the source is fixed.
            if !c.has_errors() {
                let types: Vec<&'static FeatureType> = c.features.iter().map(|f| f.feature_type).collect();
                super::studio::activate(*tab, &types);
            }
            LATEST.write().insert(tab.to_string(), c.clone());
            compiled.insert(*tab, c.clone());
            studios.push((*tab, name.clone(), c));
        }
        let files = doc
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::File(f) => Some((t.id, f.clone())),
                _ => None,
            })
            .collect::<Vec<(TabId, crate::file_tab::FileTab)>>();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut latest = LATEST_FILES.write();
        for (t, f) in &files {
            (t, &f.file_name, f.size, f.uploaded).hash(&mut h);
            latest.insert(t.to_string(), f.clone());
        }
        drop(latest);
        let files_hash = h.finish();
        LATEST_FILES_HASH.store(files_hash, std::sync::atomic::Ordering::Relaxed);
        Arc::new(Self { studios, files, files_hash })
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
    fn file(&self, tab: &str) -> Option<crate::file_tab::FileTab> {
        self.files.iter().find(|(t, _)| t.to_string() == tab).map(|(_, f)| f.clone())
    }
    /// Reference parameters read file tabs: a changed file re-runs the
    /// custom features too.
    fn version(&self, tab: &str) -> u64 {
        self.studio(tab).map(|s| s.hash).unwrap_or(0) ^ self.files_hash
    }
}
