//! Access to a document's compiled Feature Studios during regeneration.

use super::studio::{CompiledStudio, compile_with, module_path};
pub use crate::regen::custom_features::CustomFeatureProvider;
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
        if tab.starts_with("ext:") {
            return external_studio(tab);
        }
        LATEST.read().get(tab).cloned()
    }
    fn file(&self, tab: &str) -> Option<crate::file_tab::FileTab> {
        LATEST_FILES.read().get(tab).cloned()
    }
    fn version(&self, tab: &str) -> u64 {
        self.studio(tab).map(|s| s.hash).unwrap_or(0) ^ LATEST_FILES_HASH.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn files_version(&self) -> u64 {
        LATEST_FILES_HASH.load(std::sync::atomic::Ordering::Relaxed)
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
    crate::register();
    let _ = EXTERNAL_TABS.set(f);
}

/// A tab of another document or version, by its `ext:` key.
pub fn external_tab(key: &str) -> Option<crate::document::Tab> {
    EXTERNAL_TABS.get().and_then(|f| f(key))
}

static EXTERNAL_STUDIOS: LazyLock<RwLock<HashMap<String, Option<Arc<CompiledStudio>>>>> = LazyLock::new(Default::default);

/// The Feature Studio of another document or version by its `ext:` key, compiled once (a version never changes),
/// its features (`custom:<key>:<name>`) known to `features::get`. None: not a Feature Studio, or unreadable.
pub fn external_studio(key: &str) -> Option<Arc<CompiledStudio>> {
    if let Some(c) = EXTERNAL_STUDIOS.read().get(key) {
        return c.clone();
    }
    let compiled = external_tab(key).and_then(|tab| {
        let TabContent::FeatureStudio(fs) = &tab.content else { return None };
        // Its own imports of std and of versions resolve; one of a tab of its document by tab id does not.
        let resolver = DocResolver { studios: &[], compiled: &HashMap::new(), others: &[] };
        Some(Arc::new(super::studio::compile_external(tab.id, key, &fs.source, &resolver)))
    });
    if let Some(c) = compiled.as_ref().filter(|c| !c.has_errors()) {
        let types: Vec<&'static FeatureType> = c.features.iter().map(|f| f.feature_type).collect();
        super::studio::activate(key, &types);
    }
    EXTERNAL_STUDIOS.write().insert(key.to_string(), compiled.clone());
    compiled
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
/// line 6, which is what Onshape prints for it ("<function 6>"). Version 1:
/// an imported element is no newer than any Feature Studio importing it
/// (Onshape's public Spur Gear, at 1576, imports its icon).
fn imported_source(tab: impl std::fmt::Display, what: Importable) -> String {
    match what {
        Importable::File => format!("FeatureScript 1;\nexport const BLOB_DATA = @blobData(\"{tab}\");\nexport const processedForeingDataInfo = @processedForeignDataInfo(\"{tab}\");\n"),
        Importable::PartStudio => format!("FeatureScript 1;\n\n\n\n\nexport const build = function(configuration)\n{{\n    return @buildPartStudio(\"{tab}\", configuration);\n}};\n"),
        Importable::VariableStudio => "FeatureScript 1;\n".to_string(),
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

/// A document's Feature Studios (tab, name, committed source), the other
/// tabs a studio can import, and the hash imports make part of the cache key.
type Importables = (Vec<(TabId, String, String)>, Vec<(TabId, Importable)>, u64);

fn importables(doc: &Document) -> Importables {
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
            (t.id, &f.file_name, f.size, f.uploaded, file_content_hash(f)).hash(&mut h);
        }
    }
    let deps = if sources.len() > 1 || !others.is_empty() { h.finish() } else { 0 };
    (sources, others, deps)
}

/// Studio `tab` with `source` (the editor's text) compiled as the editor
/// does: its imports of the document's other tabs resolved.
pub fn compile_in_document(doc: &Document, tab: TabId, source: &str) -> Arc<CompiledStudio> {
    compile_with_imports(&doc_imports(doc), tab, source)
}

/// What a studio of a document can import: its Feature Studios' sources and the other importable tabs (cheap to take
/// to a background compile, unlike the document).
#[derive(Clone, Debug)]
pub struct DocImports(Importables);

pub fn doc_imports(doc: &Document) -> DocImports {
    DocImports(importables(doc))
}

/// [`compile_in_document`] from a [`DocImports`] snapshot.
pub fn compile_with_imports(imports: &DocImports, tab: TabId, source: &str) -> Arc<CompiledStudio> {
    let (mut sources, others, deps) = imports.0.clone();
    if let Some(s) = sources.iter_mut().find(|s| s.0 == tab) {
        s.2 = source.to_string();
    }
    let compiled = HashMap::new();
    let resolver = DocResolver { studios: &sources, compiled: &compiled, others: &others };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (deps, source).hash(&mut h);
    compile_with(tab, source, &resolver, if deps == 0 { 0 } else { h.finish() })
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

/// What [`DocumentScripts::from_document`] reads of a document: its Feature Studios' sources, the tabs they can
/// import and its file tabs. Cheap to take (no compile), so the UI thread takes it and a background task compiles it
/// ([`DocumentScripts::from_sources`], [`LazyScripts`]).
#[derive(Clone, Debug)]
pub struct ScriptSources {
    importables: Importables,
    files: Vec<(TabId, crate::file_tab::FileTab)>,
}

impl ScriptSources {
    /// Every studio compiled already (the compile cache has each): compiling them costs no compile.
    pub fn compiled(&self) -> bool {
        let (sources, _, deps) = &self.importables;
        sources.iter().all(|(tab, _, src)| super::studio::is_compiled(*tab, src, *deps))
    }
}

impl DocumentScripts {
    pub fn from_document(doc: &Document) -> Arc<Self> {
        Self::from_sources(&Self::sources(doc))
    }

    /// The document's sources, uncompiled.
    pub fn sources(doc: &Document) -> ScriptSources {
        let files = doc
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::File(f) => Some((t.id, f.clone())),
                _ => None,
            })
            .collect();
        ScriptSources { importables: importables(doc), files }
    }

    /// The scripts of `doc` when its studios are compiled already, else None (compiling them is a background
    /// task's: the first compile of a document's studios takes ~50 ms).
    pub fn if_compiled(doc: &Document) -> Option<Arc<Self>> {
        let src = Self::sources(doc);
        src.compiled().then(|| Self::from_sources(&src))
    }

    /// `sources` compiled (each studio cached by its source), their feature types registered.
    pub fn from_sources(src: &ScriptSources) -> Arc<Self> {
        // Regenerations with these scripts evaluate custom features through the host.
        crate::register();
        let (sources, others, deps) = (&src.importables.0, &src.importables.1, src.importables.2);
        let mut compiled: HashMap<TabId, Arc<CompiledStudio>> = HashMap::new();
        let mut studios = Vec::new();
        for (tab, name, source) in sources {
            let c = {
                let resolver = DocResolver { studios: sources, compiled: &compiled, others };
                compile_with(*tab, source, &resolver, deps)
            };
            // A studio with errors keeps its last good schemas active, so
            // features using it stay editable while the source is fixed.
            if !c.has_errors() {
                let types: Vec<&'static FeatureType> = c.features.iter().map(|f| f.feature_type).collect();
                super::studio::activate(&tab.to_string(), &types);
            }
            LATEST.write().insert(tab.to_string(), c.clone());
            compiled.insert(*tab, c.clone());
            studios.push((*tab, name.clone(), c));
        }
        let files = src.files.clone();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut latest = LATEST_FILES.write();
        for (t, f) in &files {
            (t, &f.file_name, f.size, f.uploaded, file_content_hash(f)).hash(&mut h);
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

/// A document's scripts compiled on first use: a regeneration's provider, so its background task compiles, not the
/// UI thread that starts it.
#[derive(Debug)]
pub struct LazyScripts {
    sources: ScriptSources,
    compiled: std::sync::OnceLock<Arc<DocumentScripts>>,
}

impl LazyScripts {
    pub fn new(sources: ScriptSources) -> Arc<Self> {
        Arc::new(Self { sources, compiled: Default::default() })
    }

    pub fn get(&self) -> &Arc<DocumentScripts> {
        self.compiled.get_or_init(|| DocumentScripts::from_sources(&self.sources))
    }
}

impl CustomFeatureProvider for LazyScripts {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>> {
        self.get().studio(tab)
    }
    fn file(&self, tab: &str) -> Option<crate::file_tab::FileTab> {
        self.get().file(tab)
    }
    fn version(&self, tab: &str) -> u64 {
        self.get().version(tab)
    }
    fn files_version(&self) -> u64 {
        self.get().files_version()
    }
}

impl CustomFeatureProvider for DocumentScripts {
    fn studio(&self, tab: &str) -> Option<Arc<CompiledStudio>> {
        if tab.starts_with("ext:") {
            return external_studio(tab);
        }
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
    fn files_version(&self) -> u64 {
        self.files_hash
    }
}

/// A File tab's bytes as a hash (a replacement with the same name, size and upload second is another file), worked
/// out once per buffer.
pub fn file_content_hash(f: &crate::file_tab::FileTab) -> u64 {
    static MEMO: std::sync::LazyLock<parking_lot::Mutex<HashMap<(usize, usize), u64>>> = std::sync::LazyLock::new(Default::default);
    let at = (std::sync::Arc::as_ptr(&f.data_base64) as *const u8 as usize, f.data_base64.len());
    if let Some(&h) = MEMO.lock().get(&at) {
        return h;
    }
    let mut out = [0u8; 8];
    out.copy_from_slice(&blake3::hash(f.data_base64.as_bytes()).as_bytes()[..8]);
    let h = u64::from_le_bytes(out);
    let mut memo = MEMO.lock();
    if memo.len() > 4096 {
        memo.clear();
    }
    memo.insert(at, h);
    h
}
