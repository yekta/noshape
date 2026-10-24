//! Derived-part provider: regenerates other Part Studios of the document on
//! demand (cached per tab) so Derived features can import their parts.
//!
//! Sources in other documents are pinned to a microversion (a named version
//! or the state when inserted, like Onshape's version-pinned links) and keyed
//! `ext:<microversion>|<tab>|<path>`. A pinned snapshot never changes, so its
//! regenerated context is cached for the whole session.

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

/// Regeneration options for an assembly evaluator's Part Studios: derived
/// parts read the document's other Part Studios, custom features its Feature
/// Studios (qa5 DER8: without them a Part Studio of derived parts is empty).
pub fn studio_options(cache: StudioCache) -> noshape_model::assembly::eval::StudioOptions {
    Arc::new(move |d: &Document| noshape_model::regen::RegenOptions {
        derived: Some(DocStudios::new(d.clone(), cache.clone())),
        custom_features: Some(noshape_model::custom::DocumentScripts::from_document(d)),
        ..Default::default()
    })
}

/// An assembly evaluator whose Part Studios find their derived and custom
/// feature sources.
pub fn assembly_evaluator(cache: StudioCache) -> noshape_model::assembly::eval::AssemblyEvaluator {
    let mut ev = noshape_model::assembly::eval::AssemblyEvaluator::new();
    ev.set_studio_options(studio_options(cache));
    ev
}

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

/// A Part Studio in another document, pinned to a microversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalRef {
    pub path: std::path::PathBuf,
    pub microversion: noshape_store::MicroversionId,
    pub tab: String,
}

impl ExternalRef {
    pub fn key(&self) -> String {
        format!("ext:{}|{}|{}", self.microversion, self.tab, self.path.display())
    }

    pub fn parse(key: &str) -> Option<Self> {
        let rest = key.strip_prefix("ext:")?;
        let mut it = rest.splitn(3, '|');
        let microversion = it.next()?.parse().ok()?;
        let tab = it.next()?.to_string();
        let path = std::path::PathBuf::from(it.next()?);
        Some(Self { path, microversion, tab })
    }
}

/// One version a Derived feature can pin: a named version or the current state.
#[derive(Clone, Debug)]
pub struct VersionChoice {
    pub label: String,
    pub microversion: noshape_store::MicroversionId,
}

/// What another document offers as Derived sources.
#[derive(Clone, Debug)]
pub struct ExternalDocInfo {
    pub name: String,
    /// Named versions, newest first, then "Current state".
    pub versions: Vec<VersionChoice>,
}

type SnapshotKey = (std::path::PathBuf, noshape_store::MicroversionId);
static SNAPSHOTS: std::sync::LazyLock<Mutex<HashMap<SnapshotKey, Arc<Document>>>> = std::sync::LazyLock::new(Default::default);
static EXT_CONTEXTS: std::sync::LazyLock<Mutex<HashMap<String, Option<Arc<Context>>>>> = std::sync::LazyLock::new(Default::default);

thread_local! {
    /// Nesting depth of external regenerations (guards against pathological chains).
    static EXT_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Read another document's state at a microversion (cached; snapshots are immutable).
pub fn external_snapshot(path: &std::path::Path, mv: noshape_store::MicroversionId) -> anyhow::Result<Arc<Document>> {
    let key = (path.to_path_buf(), mv);
    if let Some(d) = SNAPSHOTS.lock().get(&key) {
        return Ok(d.clone());
    }
    let store = noshape_store::Document::open_read_only(path)?;
    let doc = Arc::new(Document::from_blobs(&store.read_all(mv)?)?);
    SNAPSHOTS.lock().insert(key, doc.clone());
    Ok(doc)
}

/// Document files by document id ([`document_id`]), found by scanning the
/// local documents once per unknown id.
static DOCUMENT_IDS: std::sync::LazyLock<Mutex<HashMap<String, std::path::PathBuf>>> = std::sync::LazyLock::new(Default::default);

/// Lists the local document files (the app registers its document library
/// scan; headless callers may register their own or open documents by path).
static DOCUMENT_SCANNER: std::sync::OnceLock<fn() -> Vec<std::path::PathBuf>> = std::sync::OnceLock::new();

/// Register how to list local document files (once, at startup).
pub fn set_document_scanner(f: fn() -> Vec<std::path::PathBuf>) {
    let _ = DOCUMENT_SCANNER.set(f);
}

/// A document's id: its first microversion (stable across copies of the file).
pub fn document_id(path: &std::path::Path) -> Option<noshape_store::MicroversionId> {
    let store = noshape_store::Document::open_read_only(path).ok()?;
    store.graph().ok()?.microversions.iter().find(|m| m.parents.is_empty()).map(|m| m.id)
}

/// Remember where a document with a known id lives.
pub fn note_document(path: &std::path::Path) {
    if let Some(id) = document_id(path) {
        DOCUMENT_IDS.lock().insert(id.as_uuid().simple().to_string(), path.to_path_buf());
    }
}

/// Another document's (or a version's) tab from a reference or import path:
/// an `ext:` key, or a Feature Studio import path
/// `<document id>/<version microversion>/<tab id>` (fs_import.rs).
pub fn import_ref(path: &str) -> Option<ExternalRef> {
    if path.starts_with("ext:") {
        return ExternalRef::parse(path);
    }
    let mut it = path.splitn(3, '/');
    let (doc, mv, tab) = (it.next()?, it.next()?, it.next()?);
    let microversion = mv.parse().ok()?;
    let known = DOCUMENT_IDS.lock().get(doc).cloned();
    let file = match known {
        Some(f) => f,
        None => {
            let files = DOCUMENT_SCANNER.get().map(|f| f()).unwrap_or_default();
            let mut ids = DOCUMENT_IDS.lock();
            for path in files {
                if let Some(id) = document_id(&path) {
                    ids.insert(id.as_uuid().simple().to_string(), path);
                }
            }
            ids.get(doc).cloned()?
        }
    };
    Some(ExternalRef { path: file, microversion, tab: tab.to_string() })
}

/// A tab of another document or version (Feature Studio imports, references).
pub fn external_tab(key: &str) -> Option<noshape_model::document::Tab> {
    let r = import_ref(key)?;
    let doc = external_snapshot(&r.path, r.microversion).ok()?;
    doc.tabs.iter().find(|t| t.id.to_string() == r.tab).cloned()
}

/// Versions of another document available for pinning.
pub fn external_doc_info(path: &std::path::Path) -> anyhow::Result<ExternalDocInfo> {
    let store = noshape_store::Document::open_read_only(path)?;
    let graph = store.graph()?;
    let mut versions: Vec<VersionChoice> = graph.versions.iter().rev().map(|v| VersionChoice { label: v.name.clone(), microversion: v.microversion }).collect();
    let head = store.head(store.default_branch()?)?;
    versions.push(VersionChoice { label: "Current state".into(), microversion: head });
    let name = store.meta().map(|m| m.name).unwrap_or_else(|_| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    Ok(ExternalDocInfo { name, versions })
}

/// The newest named version of a document, if it differs from `pinned`
/// (Onshape's "update available").
pub fn newer_version(r: &ExternalRef) -> Option<VersionChoice> {
    let info = external_doc_info(&r.path).ok()?;
    let newest = info.versions.first().filter(|v| v.label != "Current state")?;
    (newest.microversion != r.microversion).then(|| newest.clone())
}

/// Human-readable description of an external source: "Doc · V2 · Part Studio 1".
pub fn describe_external(r: &ExternalRef) -> String {
    let info = external_doc_info(&r.path).ok();
    let doc = info.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| r.path.display().to_string());
    let version = info
        .as_ref()
        .and_then(|i| i.versions.iter().find(|v| v.microversion == r.microversion && v.label != "Current state"))
        .map(|v| v.label.clone())
        .unwrap_or_else(|| format!("state {}", r.microversion.short()));
    let tab = external_snapshot(&r.path, r.microversion).ok().and_then(|d| d.tabs.iter().find(|t| t.id.to_string() == r.tab).map(|t| t.name.clone())).unwrap_or_else(|| "missing Part Studio".into());
    format!("{doc} · {version} · {tab}")
}

/// Regenerated context of a pinned external Part Studio (cached).
pub fn external_context(r: &ExternalRef) -> Option<Arc<Context>> {
    let key = r.key();
    if let Some(c) = EXT_CONTEXTS.lock().get(&key) {
        return c.clone();
    }
    if EXT_DEPTH.with(|d| d.get()) >= 8 {
        return None;
    }
    EXT_DEPTH.with(|d| d.set(d.get() + 1));
    let ctx = external_snapshot(&r.path, r.microversion).ok().and_then(|doc| {
        let provider = DocStudios { doc, cache: Default::default(), active: Default::default() };
        provider.context(&r.tab)
    });
    EXT_DEPTH.with(|d| d.set(d.get() - 1));
    EXT_CONTEXTS.lock().insert(key, ctx.clone());
    ctx
}

impl DocStudios {
    /// A Part Studio regenerated in a configuration (not cached).
    pub fn context_in(&self, tab: &str, config: &noshape_model::configuration::ConfigValues) -> Option<Arc<Context>> {
        let studio = self.studio(tab)?.clone();
        let opts = RegenOptions {
            document_variables: self.doc.tabs.iter().find(|t| t.id.to_string() == tab).map(|t| noshape_model::variables::element_variables(&self.doc, t.id)).unwrap_or_default(),
            derived: Some(Arc::new(DocStudios { doc: self.doc.clone(), cache: self.cache.clone(), active: self.active.clone() })),
            custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&self.doc)),
            ..Default::default()
        };
        let mut values = studio.configuration.defaults();
        for (k, v) in &config.0 {
            values.0.insert(k.clone(), v.clone());
        }
        let mut reg = noshape_model::regen::Regenerator::new();
        Some(reg.regenerate(&studio, &values, &self.doc.settings.units, &opts).context)
    }
}

impl DerivedProvider for DocStudios {
    fn file(&self, key: &str) -> Option<noshape_model::file_tab::FileTab> {
        let r = import_ref(key)?;
        let doc = external_snapshot(&r.path, r.microversion).ok()?;
        match &doc.tabs.iter().find(|t| t.id.to_string() == r.tab)?.content {
            noshape_model::document::TabContent::File(f) => Some(f.clone()),
            _ => None,
        }
    }

    fn feature_type(&self, tab: &str, feature: noshape_model::reference::FeatureId) -> Option<String> {
        let find = |ps: &noshape_model::PartStudio| ps.features.iter().find(|f| f.id == feature).map(|f| f.type_id.clone());
        if let Some(r) = import_ref(tab) {
            let doc = external_snapshot(&r.path, r.microversion).ok()?;
            return match &doc.tabs.iter().find(|t| t.id.to_string() == r.tab)?.content {
                TabContent::PartStudio(ps) => find(ps),
                _ => None,
            };
        }
        self.studio(tab).and_then(find)
    }

    fn part_name(&self, tab: &str, part: &noshape_model::reference::BodyId) -> Option<String> {
        self.studio(tab)?.part_properties.get(part)?.name.clone()
    }

    fn configuration(&self, tab: &str) -> Option<noshape_model::configuration::Configuration> {
        if let Some(r) = import_ref(tab) {
            let doc = external_snapshot(&r.path, r.microversion).ok()?;
            return match &doc.tabs.iter().find(|t| t.id.to_string() == r.tab)?.content {
                TabContent::PartStudio(ps) => Some(ps.configuration.clone()),
                _ => None,
            };
        }
        self.studio(tab).map(|ps| ps.configuration.clone())
    }

    fn context(&self, tab: &str) -> Option<Arc<Context>> {
        if let Some(r) = import_ref(tab) {
            return external_context(&r);
        }
        let studio = self.studio(tab)?.clone();
        if !self.active.lock().insert(tab.to_string()) {
            return None; // circular reference
        }
        let opts = RegenOptions {
            document_variables: self.doc.tabs.iter().find(|t| t.id.to_string() == tab).map(|t| noshape_model::variables::element_variables(&self.doc, t.id)).unwrap_or_default(),
            derived: Some(Arc::new(DocStudios { doc: self.doc.clone(), cache: self.cache.clone(), active: self.active.clone() })),
            custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&self.doc)),
            ..Default::default()
        };
        let mut reg = self.cache.lock().remove(tab).unwrap_or_default();
        let r = reg.regenerate(&studio, &studio.configuration.defaults(), &self.doc.settings.units, &opts);
        self.cache.lock().insert(tab.to_string(), reg);
        self.active.lock().remove(tab);
        Some(r.context)
    }

    fn context_configured(&self, tab: &str, values: &noshape_model::configuration::ConfigValues) -> Option<Arc<Context>> {
        if values.is_empty() || tab.starts_with("ext:") || tab.contains('/') {
            return self.context(tab);
        }
        // Cached per configuration like the default one.
        let key = format!("{tab}#{}", serde_json::to_string(&values.0).unwrap_or_default());
        let studio = self.studio(tab)?.clone();
        if !self.active.lock().insert(tab.to_string()) {
            return None; // circular reference
        }
        let opts = RegenOptions {
            document_variables: self.doc.tabs.iter().find(|t| t.id.to_string() == tab).map(|t| noshape_model::variables::element_variables(&self.doc, t.id)).unwrap_or_default(),
            derived: Some(Arc::new(DocStudios { doc: self.doc.clone(), cache: self.cache.clone(), active: self.active.clone() })),
            custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&self.doc)),
            ..Default::default()
        };
        let mut config = studio.configuration.defaults();
        for (k, v) in &values.0 {
            config.0.insert(k.clone(), v.clone());
        }
        let mut reg = self.cache.lock().remove(&key).unwrap_or_default();
        let r = reg.regenerate(&studio, &config, &self.doc.settings.units, &opts);
        self.cache.lock().insert(key, reg);
        self.active.lock().remove(tab);
        Some(r.context)
    }

    fn version(&self, tab: &str) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if tab.starts_with("ext:") || tab.contains('/') {
            // Pinned: the key fully identifies the content.
            tab.hash(&mut h);
            return h.finish();
        }
        if let Some(ps) = self.studio(tab) {
            serde_json::to_string(ps).unwrap_or_default().hash(&mut h);
            // Nested derived sources and variable studios also matter.
            for t in &self.doc.tabs {
                if let TabContent::VariableStudio(v) = &t.content {
                    serde_json::to_string(v).unwrap_or_default().hash(&mut h);
                }
                // Custom features of the source depend on Feature Studios.
                if let TabContent::FeatureStudio(f) = &t.content {
                    f.source.hash(&mut h);
                }
            }
            for f in ps.features.iter().filter(|f| noshape_model::features::importing::is_derived(&f.type_id)) {
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
