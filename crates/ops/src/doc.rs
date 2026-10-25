//! The headless workspace: open documents (sessions bound to `.noshape`
//! files), their regeneration caches, and lookups of tabs and features by
//! id or name.

use crate::error::OpError;
use crate::session::Session;
use noshape_model::configuration::ConfigValues;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::FeatureId;
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One open document.
pub struct OpenDoc {
    pub session: Session,
    /// Regeneration caches of every Part Studio (shared with Derived sources).
    pub studio_cache: crate::derived::StudioCache,
    regens: HashMap<TabId, Regenerator>,
    assemblies: HashMap<TabId, noshape_model::assembly::eval::AssemblyEvaluator>,
    /// Each Part Studio's chosen configuration (the Configurations box's
    /// values; view state, not stored in the document).
    pub active_config: HashMap<TabId, ConfigValues>,
    /// The sketch fillet radius last used in each Part Studio: live's
    /// default for the next fillet (5 mm on a fresh page, then sticky per
    /// tab until the page reloads; here, while the document is open).
    pub sketch_fillet_radius: HashMap<TabId, f64>,
    /// The document window's state (tab shown, panels, compare…; see
    /// [`crate::window`]).
    pub window: crate::window::WindowState,
}

impl OpenDoc {
    pub fn new(session: Session) -> Self {
        OpenDoc { session, studio_cache: Default::default(), regens: HashMap::new(), assemblies: HashMap::new(), active_config: HashMap::new(), sketch_fillet_radius: HashMap::new(), window: Default::default() }
    }

    pub fn doc(&self) -> &Document {
        &self.session.doc
    }

    /// The regeneration options the UI uses for a Part Studio tab: the
    /// document's Variable Studio variables, other Part Studios for Derived
    /// features and the compiled Feature Studios.
    pub fn regen_options(&self, tab: TabId) -> RegenOptions {
        let doc = &self.session.doc;
        RegenOptions {
            document_variables: noshape_model::variables::element_variables(doc, tab),
            derived: Some(crate::derived::DocStudios::new(doc.clone(), self.studio_cache.clone())),
            custom_features: Some(noshape_model::custom::DocumentScripts::from_document(doc)),
            preview_meshes: false,
            ..Default::default()
        }
    }

    /// The configuration a Part Studio shows: its defaults overridden by
    /// the chosen values ([`OpenDoc::active_config`]).
    pub fn current_config(&self, tab: TabId) -> Result<ConfigValues, OpError> {
        let studio = studio(&self.session.doc, tab)?;
        Ok(crate::config::current(studio, self.active_config.get(&tab).unwrap_or(&ConfigValues::default())))
    }

    /// Regenerate a Part Studio in a configuration (defaults overridden by
    /// the given values; None: the active configuration), incrementally from
    /// the cached results.
    pub fn regenerate(&mut self, tab: TabId, config: Option<&ConfigValues>) -> Result<RegenResult, OpError> {
        self.regenerate_with(tab, config, |_| ())
    }

    /// [`OpenDoc::regenerate`] with adjusted options (`upto` and
    /// `editing_sketch` while a sketch is edited, as the sketch mode does).
    pub fn regenerate_with(&mut self, tab: TabId, config: Option<&ConfigValues>, adjust: impl FnOnce(&mut RegenOptions)) -> Result<RegenResult, OpError> {
        let mut opts = self.regen_options(tab);
        adjust(&mut opts);
        let units = self.session.doc.settings.units;
        let values = match config {
            Some(c) => crate::config::current(studio(&self.session.doc, tab)?, c),
            None => self.current_config(tab)?,
        };
        let studio = studio(&self.session.doc, tab)?;
        let reg = self.regens.entry(tab).or_default();
        Ok(reg.regenerate(studio, &values, &units, &opts))
    }
}

impl OpenDoc {
    /// Regenerate an edited copy of a Part Studio tab without committing it
    /// (a dialog's preview), in the active configuration.
    pub fn preview(&mut self, tab: TabId, edited: &noshape_model::partstudio::PartStudio) -> Result<RegenResult, OpError> {
        let opts = self.regen_options(tab);
        let units = self.session.doc.settings.units;
        let values = crate::config::current(edited, self.active_config.get(&tab).unwrap_or(&ConfigValues::default()));
        let reg = self.regens.entry(tab).or_default();
        Ok(reg.regenerate(edited, &values, &units, &opts))
    }

    /// Evaluate (solve) an assembly tab as given (already configured, with
    /// any previewed edits), as the Assembly view does.
    pub fn evaluate_assembly(&mut self, tab: TabId, asm: &noshape_model::assembly::Assembly, opts: &noshape_model::assembly::eval::SolveOptions) -> noshape_model::assembly::eval::AssemblyResult {
        let doc = &self.session.doc;
        let ev = self.assemblies.entry(tab).or_insert_with(|| {
            let mut ev = noshape_model::assembly::eval::AssemblyEvaluator::default();
            // Instances showing a version of this or another document.
            ev.set_external(std::sync::Arc::new(crate::derived::ExternalDocs));
            ev
        });
        ev.retain_tabs(doc);
        ev.evaluate(doc, asm, opts, None)
    }
}

/// The agent API's state: the documents it has open. CLI calls open one
/// document per process; the MCP server keeps them open between calls.
#[derive(Default)]
pub struct Ctx {
    docs: Vec<(PathBuf, OpenDoc)>,
    /// The document used when a call names none (the last one opened).
    current: Option<PathBuf>,
    /// Headless renderer, created on first use.
    offscreen: Option<noshape_scene::png::Offscreen>,
    /// Documents the app has open (live mode): their port and token.
    live: Vec<(PathBuf, (u16, String))>,
    /// The app's own context: calls run here, never forwarded.
    pub serving: bool,
    /// What drawing_copy copied (the editor's clipboard text), for
    /// drawing_paste in this session.
    pub drawing_clipboard: Option<String>,
    /// What the window should do after a call (live mode: the app acts on
    /// them, as the menu item would); taken by the app after each call.
    pub ui_requests: Vec<UiRequest>,
}

pub use crate::window::UiRequest;

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl Ctx {
    pub fn new() -> Self {
        init_once();
        Ctx::default()
    }

    /// Create a new document file (Onshape's new document: Part Studio 1 and Assembly 1).
    pub fn create(&mut self, path: &Path, doc: Document) -> Result<&mut OpenDoc, OpError> {
        if path.exists() {
            return Err(OpError::invalid(format!("{} already exists", path.display())).with_param("path"));
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| OpError::new("io", format!("creating {}: {e}", dir.display())))?;
        }
        let session = Session::create(path, doc)?;
        let key = canonical(path);
        self.docs.retain(|(p, _)| *p != key);
        self.docs.push((key.clone(), OpenDoc::new(session)));
        self.current = Some(key);
        Ok(&mut self.docs.last_mut().expect("just pushed").1)
    }

    /// Open (or return the already open) document at `path` and make it current.
    pub fn open(&mut self, path: &Path) -> Result<&mut OpenDoc, OpError> {
        let key = canonical(path);
        if !self.docs.iter().any(|(p, _)| *p == key) {
            if !path.exists() {
                return Err(OpError::not_found(format!("no document at {}", path.display())).with_param("doc"));
            }
            let session = Session::open(path)?;
            crate::derived::note_document(path);
            self.docs.push((key.clone(), OpenDoc::new(session)));
        }
        self.current = Some(key.clone());
        Ok(self.docs.iter_mut().find(|(p, _)| *p == key).map(|(_, d)| d).expect("opened"))
    }

    /// The document a call names (`doc` path), else the current one.
    pub fn doc(&mut self, path: Option<&str>) -> Result<&mut OpenDoc, OpError> {
        match path {
            Some(p) => self.open(Path::new(p)),
            None => {
                let cur = self.current.clone().ok_or_else(|| OpError::invalid("no document is open: pass `doc` (a .noshape path) or call document_open / document_create first").with_param("doc"))?;
                Ok(self.docs.iter_mut().find(|(p, _)| *p == cur).map(|(_, d)| d).expect("current is open"))
            }
        }
    }

    pub fn close(&mut self, path: Option<&str>) -> Result<PathBuf, OpError> {
        let key = match path {
            Some(p) => canonical(Path::new(p)),
            None => self.current.clone().ok_or_else(|| OpError::invalid("no document is open"))?,
        };
        let before = self.docs.len();
        self.docs.retain(|(p, _)| *p != key);
        if self.docs.len() == before {
            return Err(OpError::not_found(format!("{} is not open", key.display())));
        }
        if self.current.as_ref() == Some(&key) {
            self.current = self.docs.last().map(|(p, _)| p.clone());
        }
        Ok(key)
    }

    /// The headless renderer (created on the first render).
    pub fn offscreen(&mut self) -> Result<&mut noshape_scene::png::Offscreen, OpError> {
        if self.offscreen.is_none() {
            self.offscreen = Some(noshape_scene::png::Offscreen::new().map_err(|e| OpError::new("render_failed", e))?);
        }
        Ok(self.offscreen.as_mut().expect("created"))
    }

    pub fn open_paths(&self) -> Vec<&Path> {
        self.docs.iter().map(|(p, _)| p.as_path()).collect()
    }

    pub fn current_path(&self) -> Option<&Path> {
        self.current.as_deref()
    }

    /// The app serving document `key` (canonical path), if this context
    /// sends its calls there.
    pub fn live_server(&self, key: &Path) -> Option<(u16, String)> {
        self.live.iter().find(|(p, _)| p == key).map(|(_, s)| s.clone())
    }

    /// Send calls for `key` to the app (live mode); it becomes current.
    pub fn set_live(&mut self, key: &Path, server: (u16, String)) {
        self.live.retain(|(p, _)| p != key);
        self.live.push((key.to_path_buf(), server));
        self.current = Some(key.to_path_buf());
    }

    /// Stop sending calls for `key` to the app.
    pub fn unset_live(&mut self, key: &Path) {
        self.live.retain(|(p, _)| p != key);
        if self.current.as_deref() == Some(key) {
            self.current = self.docs.last().map(|(p, _)| p.clone()).or_else(|| self.live.last().map(|(p, _)| p.clone()));
        }
    }

    /// The app's context: a document it has open (live mode), current.
    pub fn adopt(&mut self, path: &Path, doc: OpenDoc) {
        let key = canonical(path);
        self.docs.retain(|(p, _)| *p != key);
        self.docs.push((key.clone(), doc));
        self.current = Some(key);
    }

    /// A document this context has open.
    pub fn get_mut(&mut self, path: &Path) -> Option<&mut OpenDoc> {
        let key = canonical(path);
        self.docs.iter_mut().find(|(p, _)| *p == key).map(|(_, d)| d)
    }
}

fn init_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Variable Studios inserted from other documents read those
        // documents' versions (the app sets the same loader).
        noshape_model::variables::set_external_loader(|path, mv| mv.parse().ok().and_then(|mv| crate::derived::external_snapshot(path, mv).ok()));
    });
}

/// A tab by id (hex, as listed) or by name (case-insensitive).
pub fn find_tab(doc: &Document, key: &str) -> Result<TabId, OpError> {
    let k = key.trim();
    if let Some(t) = doc.tabs.iter().find(|t| t.id.to_string() == k) {
        return Ok(t.id);
    }
    if let Some(t) = doc.tabs.iter().find(|t| t.name == k).or_else(|| doc.tabs.iter().find(|t| t.name.eq_ignore_ascii_case(k))) {
        return Ok(t.id);
    }
    let names: Vec<String> = doc.ordered_tabs().iter().map(|t| format!("{} ({})", t.name, t.content.kind_name())).collect();
    Err(OpError::not_found(format!("no tab `{key}`; tabs: {}", names.join(", "))).with_param("tab"))
}

/// The tab a call names, or the first tab of the wanted kind.
pub fn tab_of_kind(doc: &Document, key: Option<&str>, kind: &str, is: fn(&TabContent) -> bool) -> Result<TabId, OpError> {
    let id = match key {
        Some(k) => find_tab(doc, k)?,
        None => doc.ordered_tabs().into_iter().find(|t| is(&t.content)).map(|t| t.id).ok_or_else(|| OpError::not_found(format!("the document has no {kind}")).with_param("tab"))?,
    };
    let t = doc.tab(id).expect("found");
    if !is(&t.content) {
        return Err(OpError::invalid(format!("tab `{}` is a {}, not a {kind}", t.name, t.content.kind_name())).with_param("tab"));
    }
    Ok(id)
}

pub fn is_part_studio(c: &TabContent) -> bool {
    matches!(c, TabContent::PartStudio(_))
}

pub fn studio(doc: &Document, tab: TabId) -> Result<&PartStudio, OpError> {
    match doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => Ok(ps),
        _ => Err(OpError::invalid("not a Part Studio").with_param("tab")),
    }
}

/// A feature by id (`F1a2b`) or name ("Extrude 1").
pub fn find_feature(ps: &PartStudio, key: &str) -> Result<FeatureId, OpError> {
    let k = key.trim();
    if let Some(id) = crate::refs::parse_feature_id(k)
        && ps.feature(id).is_some()
    {
        return Ok(id);
    }
    if let Some(f) = ps.features.iter().find(|f| f.name == k).or_else(|| ps.features.iter().find(|f| f.name.eq_ignore_ascii_case(k))) {
        return Ok(f.id);
    }
    Err(OpError::not_found(format!("no feature `{key}`; features: {}", ps.features.iter().map(|f| format!("{} ({})", f.name, f.id)).collect::<Vec<_>>().join(", "))).with_param("feature"))
}
