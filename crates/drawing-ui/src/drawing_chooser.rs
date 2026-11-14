//! Drawing editor: Insert view › "Select a part or assembly" (live
//! drawing/170–173, behaviours "Flat pattern view"): a 300 px flyout right
//! of the Insert view panel. Current document | Other documents; the
//! document name and its branch; Part Studios | Assemblies; "Search parts
//! or sketches"; the filter toggles Part Studios | Parts | Composite parts
//! | Sketches | Surfaces | Curves | Flat patterns (independent, Parts on;
//! live "qa5 pass 7" (4)); then ▸ studio rows that expand to pictures of
//! what the filters show ("Flat pattern of Part 1" above "Part 1"). The
//! pictures are rendered offscreen in the background (the
//! document-thumbnail look), cached per document and studio content hash.
//! Other documents: the library's documents, an opened one listed like
//! this document at one of its versions.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::document::{TabContent, TabId};
use noshape_model::drawing::views::{self, SourceKey};
use noshape_model::drawing::ViewOrientation;
use std::collections::HashMap;
use std::sync::Arc;

/// The filter toggles (tooltip, icon), live order.
const FILTERS: [(&str, &str); 7] = [
    ("Part Studios", "part-studio"),
    ("Parts", "part"),
    ("Composite parts", "composite-part"),
    ("Sketches", "sketch"),
    ("Surfaces", "surface-body"),
    ("Curves", "wire-body"),
    ("Flat patterns", "flat-pattern"),
];
const F_STUDIOS: u8 = 1;
const F_PARTS: u8 = 1 << 1;
const F_COMPOSITES: u8 = 1 << 2;
const F_SKETCHES: u8 = 1 << 3;
const F_SURFACES: u8 = 1 << 4;
const F_CURVES: u8 = 1 << 5;
const F_FLAT: u8 = 1 << 6;

/// What a picture shows: a part (or the whole studio) or a sketch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ThumbOf {
    Part(Option<BodyId>),
    Sketch(noshape_model::FeatureId),
    /// A sheet metal part flattened (qa5 DSM1: the flat plate, not the part).
    Flat(BodyId),
}

/// A studio's row in the tree: label, picture, part, flat pattern, sketch.
type ChildRow = (String, ThumbOf, Option<BodyId>, bool, Option<noshape_model::FeatureId>);

/// The chooser's state besides its tab (`InsertViewState::chooser`).
#[derive(Default)]
pub struct ChooserState {
    /// The filter toggles that are on (bit `i`: [`FILTERS`]`[i]`; None:
    /// Parts only).
    pub filter: Option<u8>,
    /// Expanded studios.
    pub open: Vec<TabId>,
    /// Opened from + › Create Drawing…: the only Part Studio with parts starts expanded (live doc-plus-create-drawing-ok).
    pub expand_single: bool,
    pub search: Option<Entity<InputState>>,
    /// Create version… left the keyboard on the drawing (the state reads the drawing, not the chooser's field, until
    /// the chooser is used again).
    pub keyboard_left: bool,
    /// Pictures per (document, studio, what), the hash they were made for,
    /// busy. Document 0 is this one, else [`OtherDoc::tag`].
    pub thumbs: HashMap<(u64, TabId, ThumbOf), Arc<RenderImage>>,
    pub made_for: HashMap<(u64, TabId), u64>,
    pub busy: bool,
    /// Other documents tab; its "Search or paste URL" box, category, the
    /// scanned documents, a search and the opened document.
    pub other: bool,
    pub url: Option<Entity<InputState>>,
    pub other_cat: Option<usize>,
    pub other_docs: Option<Vec<crate::library::DocEntry>>,
    /// Each scanned document's latest version ("V1", else "Start"), its
    /// row's second line.
    pub other_versions: HashMap<std::path::PathBuf, String>,
    pub other_query: Option<String>,
    pub other_doc: Option<OtherDoc>,
    /// The Cut list table's "Select a composite part" (live2 2278): composite parts only, a pick is the table's.
    pub composite: bool,
    /// Current document › Version graph: the list shown, and the version
    /// chosen (new views are pinned to it).
    pub graph: bool,
    pub version: Option<noshape_model::drawing::PinnedVersion>,
    /// The opened other document's Version graph (its versions listed).
    pub other_graph: bool,
    /// The opened other document as a model while its Create version
    /// dialog is open (the document is reread when a version is made).
    pub other_model: Option<(Entity<crate::doc_model::DocModel>, Subscription)>,
    /// The opened other document's "View released items": its released
    /// revisions listed instead of its tree.
    pub other_released: bool,
}

/// A released revision of a part or assembly in an opened other document
/// (View released items; help: "pick a particular revision; released items
/// show a solid triangle icon").
#[derive(Clone)]
pub struct Released {
    pub tab: TabId,
    pub part: Option<BodyId>,
    pub name: String,
    pub assembly: bool,
    pub revision: String,
    pub part_number: String,
    /// The version it was released at.
    pub microversion: noshape_store::MicroversionId,
}

/// A release log's released parts and assemblies, newest release first
/// (obsoleted revisions and items linked from other documents left out);
/// `microversion` resolves a package's version (a version id, or a
/// microversion for versions linked from elsewhere).
pub fn released_items(log: &noshape_model::release::ReleaseLog, microversion: &dyn Fn(&str) -> Option<noshape_store::MicroversionId>) -> Vec<Released> {
    use noshape_model::release::{ItemKind, PackageKind};
    let obsolete: Vec<(&str, &str)> = log.packages.iter().filter(|p| p.kind == PackageKind::Obsoletion).flat_map(|p| p.items.iter().map(|i| (i.part_number.as_str(), i.revision.as_str()))).collect();
    let mut out = Vec::new();
    for p in log.packages.iter().rev().filter(|p| p.kind == PackageKind::Release) {
        let Some(mv) = p.version.as_deref().and_then(microversion) else { continue };
        for i in &p.items {
            if !matches!(i.kind, ItemKind::Part | ItemKind::Assembly) || i.document.is_some() || i.reverted || obsolete.contains(&(i.part_number.as_str(), i.revision.as_str())) {
                continue;
            }
            out.push(Released { tab: i.tab, part: i.part.clone(), name: i.name.clone(), assembly: i.kind == ItemKind::Assembly, revision: i.revision.clone(), part_number: i.part_number.clone(), microversion: mv });
        }
    }
    out
}

/// Other documents categories (live, as the assembly Insert dialog).
const OTHER_SOURCES: [(&str, &str); 5] = [("Owned by me", "src-owned"), ("Recently opened", "src-recent"), ("Created by me", "src-created"), ("Shared with me", "src-shared"), ("Public", "src-public")];

/// A document opened in Other documents: its versions (named ones newest
/// first, then Start; only versions can be referenced), the one shown and
/// that version's Part Studios and assemblies.
#[derive(Clone)]
pub struct OtherDoc {
    pub path: std::path::PathBuf,
    pub name: String,
    pub versions: Vec<crate::derived::VersionChoice>,
    /// Microversions in its workspace since its last version.
    pub changes: usize,
    pub version: usize,
    pub tabs: Vec<(TabId, String, bool)>,
    pub released: Vec<Released>,
}

impl OtherDoc {
    fn open(path: &std::path::Path) -> anyhow::Result<Self> {
        let store = noshape_store::Document::open_read_only(path)?;
        let graph = store.graph()?;
        let mut versions: Vec<crate::derived::VersionChoice> = graph.versions.iter().rev().map(|v| crate::derived::VersionChoice { label: v.name.clone(), microversion: v.microversion }).collect();
        if let Some(root) = graph.microversions.iter().find(|m| m.parents.is_empty()) {
            versions.push(crate::derived::VersionChoice { label: "Start".into(), microversion: root.id });
        }
        let marks: std::collections::HashSet<noshape_store::MicroversionId> = versions.iter().map(|v| v.microversion).collect();
        let parent: HashMap<noshape_store::MicroversionId, Option<noshape_store::MicroversionId>> = graph.microversions.iter().map(|m| (m.id, m.parents.first().copied())).collect();
        let mut at = Some(store.head(store.default_branch()?)?);
        let mut changes = 0;
        while let Some(mv) = at.filter(|mv| !marks.contains(mv)) {
            changes += 1;
            at = parent.get(&mv).copied().flatten();
        }
        let name = store.meta().map(|m| m.name).unwrap_or_else(|_| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
        let at = |s: &str| graph.versions.iter().find(|v| v.id.to_string() == s).map(|v| v.microversion).or_else(|| s.parse().ok());
        let released = store.get_meta(noshape_ops::release::META_KEY).ok().flatten().map(|b| released_items(&noshape_model::release::ReleaseLog::from_json(&b), &at)).unwrap_or_default();
        let mut d = OtherDoc { path: path.to_path_buf(), name, versions, changes, version: 0, tabs: Vec::new(), released };
        d.load_tabs();
        Ok(d)
    }

    fn load_tabs(&mut self) {
        self.tabs = self
            .versions
            .get(self.version)
            .and_then(|v| crate::derived::external_snapshot(&self.path, v.microversion).ok())
            .map(|d| {
                d.ordered_tabs()
                    .into_iter()
                    .filter_map(|t| match &t.content {
                        TabContent::PartStudio(_) => Some((t.id, t.name.clone(), false)),
                        TabContent::Assembly(_) => Some((t.id, t.name.clone(), true)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    /// The pictures' document tag: this file at the shown version.
    pub fn tag(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.path.hash(&mut h);
        self.versions.get(self.version).map(|v| v.microversion.to_string()).hash(&mut h);
        h.finish() | 1
    }

    /// Its reference key for tab `tab` at the shown version, and the label.
    pub fn key(&self, tab: TabId) -> Option<(String, String)> {
        let v = self.versions.get(self.version)?;
        Some((crate::derived::ExternalRef { path: self.path.clone(), microversion: v.microversion, tab: tab.to_string() }.key(), format!("{} {}", self.name, v.label)))
    }
}

impl DrawingEditor {
    /// Create the search field with the chooser (called from render).
    pub fn sync_chooser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (self.insert.chooser.is_some(), self.chooser.search.is_some()) {
            (true, false) => {
                let placeholder = if self.chooser.composite { "Search composite parts" } else { "Search parts or sketches" };
                self.chooser.search = Some(cx.new(|cx| InputState::new(window, cx).placeholder(placeholder)));
                self.chooser.url = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Search or paste URL")));
                // Live 2063: the search box takes the focus as the chooser opens.
                self.chooser.keyboard_left = false;
                if let Some(search) = self.chooser.search.clone() {
                    window.defer(cx, move |window, cx| search.update(cx, |s, cx| s.focus(window, cx)));
                }
            }
            (false, true) => {
                self.chooser.search = None;
                self.chooser.url = None;
            }
            _ => {}
        }
        if self.insert.chooser.is_some() {
            self.request_chooser_thumbs(cx);
        }
    }

    fn chooser_filter(&self) -> u8 {
        if self.chooser.composite {
            return F_COMPOSITES;
        }
        self.chooser.filter.unwrap_or(F_PARTS)
    }

    /// The Cut list table dialog's Insert: Select a composite part.
    pub fn open_composite_chooser(&mut self, cx: &mut Context<Self>) {
        if self.insert.chooser.is_some() && !self.chooser.composite {
            // (The Insert view's chooser and search go first.)
            self.insert.chooser = None;
            self.chooser.search = None;
        }
        self.chooser.composite = true;
        self.chooser.other = false;
        self.insert.chooser = Some(0);
        self.ensure_sources(false, cx);
        cx.notify();
    }

    pub fn close_composite_chooser(&mut self) {
        if std::mem::take(&mut self.chooser.composite) {
            self.insert.chooser = None;
        }
    }

    /// A composite part picked: the cut list table's, placed by the next click.
    fn pick_composite(&mut self, id: TabId, part: Option<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(part) = part else { return };
        let other = self.chooser.other_doc.clone().filter(|_| self.chooser.other);
        let version = other.as_ref().and_then(|d| d.key(id)).map(|k| k.0);
        let key = SourceKey { version: version.clone(), ..SourceKey::studio(id) };
        let label = self.sources.get(&key).and_then(|s| s.context.bodies.iter().find(|b| b.id == part).map(|b| b.name.clone())).unwrap_or_default();
        if let Some(d) = &mut self.table_dlg {
            d.cut_source = Some(noshape_model::drawing::CompositeSource { tab: id, version, part, label });
        }
        self.close_composite_chooser();
        self.status = Some("Click to place the table".into());
        window.focus(&self.focus, cx);
        self.ensure_sources(false, cx);
        self.update_preview(cx);
        cx.notify();
    }

    /// Render the pictures of studios that changed since the last time.
    fn request_chooser_thumbs(&mut self, cx: &mut Context<Self>) {
        if self.chooser.busy {
            return;
        }
        // This document, or the version of the other one opened.
        let other = self.chooser.other_doc.clone().filter(|_| self.chooser.other);
        let (tag, doc) = match &other {
            Some(d) => {
                let Some(key) = d.tabs.first().and_then(|t| d.key(t.0)) else { return };
                let Some(doc) = self.version_doc(&key.0, cx) else { return };
                (d.tag(), doc)
            }
            None => (0, self.doc.read(cx).session.doc.clone()),
        };
        let todo: Vec<(TabId, u64)> = doc
            .ordered_tabs()
            .into_iter()
            .filter(|t| matches!(t.content, TabContent::PartStudio(_)))
            .filter_map(|t| views::source_hash(&doc, &SourceKey::studio(t.id)).map(|h| (t.id, h)))
            .filter(|(t, h)| self.chooser.made_for.get(&(tag, *t)) != Some(h))
            .collect();
        if todo.is_empty() {
            return;
        }
        self.chooser.busy = true;
        let gpu = self.gpu.clone();
        cx.spawn(async move |this, cx| {
            let made = cx
                .background_spawn(async move {
                    let mut regen = noshape_model::regen::Regenerator::default();
                    let mut out: Vec<((TabId, ThumbOf), Arc<RenderImage>)> = Vec::new();
                    for (tab, _) in &todo {
                        let Some(TabContent::PartStudio(ps)) = doc.tab(*tab).map(|t| &t.content) else { continue };
                        let result = regen.regenerate(ps, &Default::default(), &doc.settings.units, &Default::default());
                        let display = &result.display;
                        let parts: Vec<Option<BodyId>> = std::iter::once(None).chain(display.bodies.iter().filter(|b| matches!(b.kind, noshape_model::display::BodyKind::Solid | noshape_model::display::BodyKind::Surface) && !b.hidden).map(|b| Some(b.id.clone()))).collect();
                        for part in parts {
                            let mut d = display.clone();
                            if let Some(p) = &part {
                                // An open composite has no mesh of its own: its members show it (Select a composite part).
                                let members: Vec<BodyId> = display.bodies.iter().find(|b| &b.id == p).and_then(|b| b.composite.as_ref()).map(|c| c.members.clone()).unwrap_or_default();
                                for b in &mut d.bodies {
                                    b.hidden = &b.id != p && !members.contains(&b.id);
                                }
                            }
                            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
                            let built = crate::scene_builder::build(&d, &opts, &mut crate::scene_builder::GpuCache::default(), gpu.uploader());
                            let Some((lo, hi)) = built.scene.bounds() else { continue };
                            let (w, h) = crate::gpu::THUMBNAIL_SIZE;
                            let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64);
                            let Ok(png) = gpu.render_thumbnail(Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style()).await else { continue };
                            if let Some(img) = decode(&png) {
                                out.push(((*tab, ThumbOf::Part(part)), img));
                            }
                        }
                        // Sheet metal parts flattened: the flat region made solid.
                        for b in display.bodies.iter().filter(|b| result.context.sheet_metal.contains_key(&b.id)) {
                            let Ok(fp) = noshape_model::sheetmetal::flat::flat_pattern_for(&result.context, &b.id) else { continue };
                            let Some(face) = &fp.shape else { continue };
                            let Ok(solid) = noshape_kernel::extrude(face, fp.frame.normal * -fp.thickness) else { continue };
                            let Some(mesh) = noshape_model::regen::tessellate_shape(&solid.shape, 1.0) else { continue };
                            let mut d = display.clone();
                            let mesh = Arc::new(mesh);
                            for x in &mut d.bodies {
                                x.hidden = x.id != b.id;
                                if x.id == b.id {
                                    x.mesh = mesh.clone();
                                }
                            }
                            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
                            let built = crate::scene_builder::build(&d, &opts, &mut crate::scene_builder::GpuCache::default(), gpu.uploader());
                            let Some((lo, hi)) = built.scene.bounds() else { continue };
                            let (w, h) = crate::gpu::THUMBNAIL_SIZE;
                            let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64);
                            let Ok(png) = gpu.render_thumbnail(Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style()).await else { continue };
                            if let Some(img) = decode(&png) {
                                out.push(((*tab, ThumbOf::Flat(b.id.clone())), img));
                            }
                        }
                        for sk in &display.sketches {
                            let mut d = display.clone();
                            for b in &mut d.bodies {
                                b.hidden = true;
                            }
                            d.sketches.retain(|x| x.feature == sk.feature);
                            if let Some(x) = d.sketches.first_mut() {
                                x.hidden = false;
                            }
                            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: true, show_origin: false, ..Default::default() };
                            let built = crate::scene_builder::build(&d, &opts, &mut crate::scene_builder::GpuCache::default(), gpu.uploader());
                            let Some((lo, hi)) = built.scene.bounds() else { continue };
                            let (w, h) = crate::gpu::THUMBNAIL_SIZE;
                            let camera = fit_camera_normal(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64, &sk.frame);
                            let Ok(png) = gpu.render_thumbnail(Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style()).await else { continue };
                            if let Some(img) = decode(&png) {
                                out.push(((*tab, ThumbOf::Sketch(sk.feature)), img));
                            }
                        }
                    }
                    (todo, out)
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                let (todo, out) = made;
                this.chooser.busy = false;
                for (tab, h) in todo {
                    this.chooser.thumbs.retain(|(d, t, _), _| (*d, *t) != (tag, tab));
                    this.chooser.made_for.insert((tag, tab), h);
                }
                this.chooser.thumbs.extend(out.into_iter().map(|((t, w), img)| ((tag, t, w), img)));
                cx.notify();
            });
        })
        .detach();
    }

    /// A source of this document (at the Version graph's version, if chosen).
    /// A row of the tree: of this document, or of the opened other one.
    fn pick_tree(&mut self, id: TabId, part: Option<BodyId>, cx: &mut Context<Self>) {
        match self.chooser.other_doc.clone().filter(|_| self.chooser.other) {
            Some(d) => {
                if let Some((key, label)) = d.key(id) {
                    self.choose_external(key, id, part, label, cx);
                }
            }
            None => self.pick_current(id, part, cx),
        }
    }

    /// Create version… in the opened other document (and its blue box
    /// link): the Create version dialog for that document.
    fn create_version_in_other(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.chooser.other_doc.as_ref().map(|d| d.path.clone()) else { return };
        let session = match noshape_ops::session::Session::open(&path) {
            Ok(s) => s,
            Err(e) => {
                self.status = Some(format!("Could not open the document: {e:#}").into());
                cx.notify();
                return;
            }
        };
        let model = cx.new(|_| crate::doc_model::DocModel::new(session));
        let sub = cx.subscribe(&model, move |this: &mut Self, _, ev: &crate::doc_model::DocEvent, cx| {
            if matches!(ev, crate::doc_model::DocEvent::Reloaded) {
                // The new version is listed (and shown) at once.
                if let Ok(d) = OtherDoc::open(&path) {
                    this.chooser.other_doc = Some(d);
                }
                this.chooser.other_model = None;
                this.ensure_sources(false, cx);
                cx.notify();
            }
        });
        self.chooser.other_model = Some((model.clone(), sub));
        crate::drawing_hooks::create_version(model, window, cx);
    }

    fn pick_current(&mut self, id: TabId, part: Option<BodyId>, cx: &mut Context<Self>) {
        let version = self.chooser.version.clone();
        self.choose_source(id, part, cx);
        if version.is_some() {
            self.insert.pinned = version;
            self.ensure_sources(false, cx);
            self.update_preview(cx);
        }
    }

    /// Scan the local document library (background) once per chooser.
    fn ensure_other_docs(&mut self, cx: &mut Context<Self>) {
        if self.chooser.other_docs.is_some() {
            return;
        }
        self.chooser.other_docs = Some(Vec::new());
        cx.spawn(async move |this, cx| {
            let (docs, versions) = cx
                .background_spawn(async {
                    let docs = crate::drawing_hooks::library_docs();
                    let versions: HashMap<std::path::PathBuf, String> = docs
                        .iter()
                        .filter(|d| !d.broken && !d.trashed)
                        .filter_map(|d| {
                            let graph = noshape_store::Document::open_read_only(&d.path).ok()?.graph().ok()?;
                            Some((d.path.clone(), graph.versions.last().map(|v| v.name.clone()).unwrap_or_else(|| "Start".into())))
                        })
                        .collect();
                    (docs, versions)
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.chooser.other_docs = Some(docs);
                this.chooser.other_versions = versions;
                cx.notify();
            });
        })
        .detach();
    }

    /// Open a document in Other documents (its latest version listed).
    fn open_other(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        self.chooser.other_doc = OtherDoc::open(path).ok();
        self.chooser.other_graph = false;
        self.ensure_sources(false, cx);
        cx.notify();
    }

    /// "Search or paste URL" › Enter / 🔍: a document path opens it; other
    /// text lists the documents whose name contains it.
    fn search_other(&mut self, cx: &mut Context<Self>) {
        let text = self.chooser.url.as_ref().map(|s| s.read(cx).value().trim().to_string()).unwrap_or_default();
        let path = std::path::PathBuf::from(text.trim_start_matches("file://"));
        if path.extension().is_some_and(|e| e == crate::library::EXT) && path.exists() {
            self.open_other(&path, cx);
        } else {
            self.ensure_other_docs(cx);
            self.chooser.other_query = (!text.is_empty()).then_some(text);
        }
        cx.notify();
    }

    /// The Other documents tab (live as the assembly Insert dialog's).
    /// An opened other document's header (live drawing/1346): 🏠 back to
    /// the document filters, ‹ back to the list it came from, its name and
    /// the version listed, the Create version… / Version graph buttons.
    fn render_other_header(&mut self, d: &OtherDoc, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let label = d.versions.get(d.version).map(|v| v.label.clone()).unwrap_or_default();
        let back_tip: &'static str = match (self.chooser.other_query.is_some(), self.chooser.other_cat) {
            (false, Some(c)) => OTHER_SOURCES.get(c).map(|s| s.0).unwrap_or("Back"),
            _ => "Search results",
        };
        let nav = |id: &'static str, icon: &'static str, tip: &'static str| div().id(id).size(px(22.)).flex().items_center().justify_center().rounded(px(2.)).cursor_pointer().hover(|d| d.bg(pal.row_hover)).child(crate::ui::glyph(icon, 14., pal.text)).tooltip(crate::ui::tooltip(tip));
        h_flex()
            .px(px(6.))
            .py(px(4.))
            .gap(px(4.))
            .child(nav("dr-ch-other-home", "home", "Go to document filters").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.chooser.other_doc = None;
                this.chooser.other_cat = None;
                this.chooser.other_query = None;
                this.chooser.other_graph = false;
                cx.notify();
            })))
            .child(nav("dr-ch-other-back", "ui-chevron-left", back_tip).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.chooser.other_doc = None;
                this.chooser.other_graph = false;
                cx.notify();
            })))
            .child(v_flex().flex_1().min_w_0().child(div().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(crate::ui::user_text(d.name.clone()))).child(h_flex().gap(px(4.)).text_size(px(13.)).text_color(gpui_kit::rgb(0x999999)).child(crate::assets::icon("branch", 12., cx)).child(label)))
            .child(chooser_square("dr-ch-other-version", "version", "Create version…", false, cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.create_version_in_other(window, cx))))
            .child(chooser_square("dr-ch-other-graph", "version-graph", "Version graph", self.chooser.other_graph, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.chooser.other_graph = !this.chooser.other_graph;
                cx.notify();
            })))
            .into_any_element()
    }

    /// The opened document's blue box: its changes since the last version
    /// and the link to version them (live `select-item-warning`).
    fn render_other_changes(&mut self, d: &OtherDoc, cx: &mut Context<Self>) -> Option<AnyElement> {
        (d.changes > 0).then(|| {
            let ink = gpui_kit::rgb(0x094174);
            v_flex()
                .mx(px(4.))
                .my(px(4.))
                .p(px(4.))
                .rounded(px(5.))
                .border_1()
                .border_color(gpui_kit::rgb(0xb2ddf6))
                .bg(gpui_kit::rgb(0xdef1fb))
                .text_size(px(14.))
                .text_color(ink)
                .child(format!("{} change{} since the last version.", d.changes, if d.changes == 1 { "" } else { "s" }))
                .child(div().id("dr-ch-other-mkversion").underline().cursor_pointer().child(format!("Create a version in {}.", d.name)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.create_version_in_other(window, cx))))
                .into_any_element()
        })
    }

    /// View released items: the opened document's released parts (or
    /// assemblies), newest first, each a solid triangle, its name and
    /// "Rev A · part number"; a click inserts it at the version it was
    /// released at.
    fn render_released(&mut self, d: &OtherDoc, assemblies: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let query = self.chooser.search.as_ref().map(|s| s.read(cx).value().trim().to_lowercase()).unwrap_or_default();
        let mut col = v_flex().id("dr-ch-released-list").flex_1().min_h_0().overflow_y_scroll();
        let items: Vec<&Released> = d.released.iter().filter(|r| r.assembly == assemblies && (query.is_empty() || r.name.to_lowercase().contains(&query) || r.part_number.to_lowercase().contains(&query))).collect();
        if items.is_empty() {
            col = col.child(div().p(px(10.)).text_size(px(13.)).text_color(pal.text_muted).child(if assemblies { "No released assemblies." } else { "No released parts." }));
        }
        for (i, r) in items.into_iter().enumerate() {
            let key = crate::derived::ExternalRef { path: d.path.clone(), microversion: r.microversion, tab: r.tab.to_string() }.key();
            let label = format!("{} Rev {}", d.name, r.revision);
            let (tab, part) = (r.tab, r.part.clone());
            let second = if r.part_number.is_empty() { format!("Rev {}", r.revision) } else { format!("Rev {} · {}", r.revision, r.part_number) };
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("dr-ch-rel-{i}")))
                    .min_h(px(38.))
                    .px(px(6.))
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.row_hover))
                    .child(div().text_size(px(10.)).text_color(pal.text).child("▲"))
                    .child(v_flex().min_w_0().child(div().truncate().text_size(px(13.33)).child(crate::ui::user_text(r.name.clone()))).child(div().text_size(px(12.)).text_color(gpui_kit::rgb(0x999999)).child(second)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_external(key.clone(), tab, part.clone(), label.clone(), cx))),
            );
        }
        col.into_any_element()
    }

    /// The opened document's Version graph: its versions (newest first,
    /// then Start); picking one lists that version.
    fn render_other_versions(&mut self, d: &OtherDoc, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut col = v_flex().id("dr-ch-other-versions").flex_1().min_h_0().overflow_y_scroll();
        for (i, v) in d.versions.iter().enumerate() {
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("dr-ch-ov-{i}")))
                    .h(px(28.))
                    .px(px(10.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.33))
                    .when(i == d.version, |d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                    .hover(|d| d.bg(pal.row_hover))
                    .child(crate::assets::icon("version", 14., cx))
                    .child(crate::ui::user_text(v.label.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(o) = this.chooser.other_doc.as_mut() {
                            o.version = i;
                            o.load_tabs();
                        }
                        this.chooser.other_graph = false;
                        this.ensure_sources(false, cx);
                        cx.notify();
                    })),
            );
        }
        col.into_any_element()
    }

    fn render_other_documents(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(0x1651b0);
        let row = |id: SharedString| h_flex().id(id).h(px(32.)).pl(px(16.)).pr(px(6.)).gap(px(6.)).cursor_pointer().text_size(px(14.)).hover(|d| d.bg(pal.row_hover));
        let mut col = v_flex().flex_1().min_h_0();
        let has_text = self.chooser.url.as_ref().is_some_and(|s| !s.read(cx).value().trim().is_empty());
        col = col.child(
            h_flex()
                .m(px(6.))
                .child(div().flex_1().children(self.chooser.url.as_ref().map(|s| Input::new(s).small())))
                .child(div().id("dr-ch-other-search").w(px(30.)).h(px(26.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("search", 18., if has_text { blue } else { gpui_kit::rgb(0xcccccc) })).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.search_other(cx)))),
        );
        let listing = self.chooser.other_query.clone().map(|q| (None, Some(q))).or_else(|| self.chooser.other_cat.map(|c| (Some(c), None)));
        let Some((cat, query)) = listing else {
            for (i, (label, icon)) in OTHER_SOURCES.iter().enumerate() {
                col = col.child(row(SharedString::from(format!("dr-ch-cat-{i}"))).child(crate::assets::icon(icon, 20., cx)).child(div().truncate().child(*label)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ensure_other_docs(cx);
                    this.chooser.other_cat = Some(i);
                    cx.notify();
                })));
            }
            return col.into_any_element();
        };
        col = col.child(
            h_flex()
                .h(px(32.))
                .px(px(6.))
                .gap(px(4.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .child(div().id("dr-ch-cat-back").cursor_pointer().child(crate::ui::glyph("ui-chevron-left", 14., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.chooser.other_cat = None;
                    this.chooser.other_query = None;
                    cx.notify();
                })))
                .child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).child(match (&query, cat) {
                    (Some(q), _) => format!("\u{201c}{q}\u{201d}"),
                    (None, Some(c)) => OTHER_SOURCES[c].0.to_string(),
                    _ => String::new(),
                })),
        );
        let me = self.doc.read(cx).session.path.clone();
        let mut docs: Vec<crate::library::DocEntry> = self.chooser.other_docs.clone().unwrap_or_default().into_iter().filter(|d| !d.trashed && !d.broken && d.path != me).collect();
        match (cat, query.as_ref().map(|q| q.to_lowercase())) {
            // Local documents are all the user's own: nothing is shared or public.
            (Some(3 | 4), _) => docs.clear(),
            (Some(1), _) => {
                docs.retain(|d| d.recent_rank.is_some());
                docs.sort_by_key(|d| d.recent_rank);
            }
            (_, Some(q)) => docs.retain(|d| d.name.to_lowercase().contains(&q)),
            _ => docs.sort_by_key(|d| std::cmp::Reverse(d.modified)),
        }
        let mut list = v_flex().id("dr-ch-other-list").flex_1().min_h_0().overflow_y_scroll();
        for (i, d) in docs.into_iter().enumerate() {
            let path = d.path.clone();
            // Live: the name, then its latest version (or Start).
            let version = self.chooser.other_versions.get(&d.path).cloned().unwrap_or_default();
            list = list.child(
                row(SharedString::from(format!("dr-ch-doc-{i}")))
                    .h(px(40.))
                    .child(crate::assets::icon("document", 18., cx))
                    .child(v_flex().min_w_0().child(div().truncate().child(crate::ui::user_text(d.name))).child(h_flex().gap(px(4.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x999999)).child(crate::assets::icon("version", 11., cx)).child(version)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_other(&path, cx))),
            );
        }
        col.child(list).into_any_element()
    }

    /// The Insert view panel before a source is chosen (live drawing/170):
    /// Insert, then the greyed fields.
    fn render_empty_insert_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        crate::ui_script::note_dialog(|| {
            // Every text shown, in reading order (README "### Drawings"; live 2063).
            let text = ["Insert view", "Insert", "View orientation", "Top", "Left", "Right", "Front", "Back", "Bottom", "Isometric", "View scale", "View simplification", "None", "Absolute", "Ratio to studio", "Ratio to part", "Auto", "Show sketch appearances"];
            serde_json::json!({ "kind": "drawing-panel", "title": "Insert view", "text": text })
        });
        // Live drawing/122: the fields greyed with grey ▼, View scale focused
        // (a 2 px blue underline, a dark ▼).
        let row = |label: &'static str, value: &'static str, focused: bool| {
            h_flex()
                .h(px(28.))
                .px(px(6.))
                .child(div().w(px(105.)).flex_none().text_size(px(12.)).child(label))
                .child(
                    h_flex()
                        .flex_1()
                        .h(px(22.))
                        .px(px(4.))
                        .map(|d| if focused { d.border_b_2().border_color(gpui_kit::rgb(0x1651b0)) } else { d.border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)) })
                        .text_size(px(12.))
                        .text_color(gpui_kit::rgb(0xaaaaaa))
                        .child(div().flex_1().child(value))
                        .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., gpui_kit::rgb_static(if focused { 0x555555 } else { 0xaaaaaa }))),
                )
        };
        v_flex()
            .id("dr-insert-empty")
            .w(px(247.))
            .flex_none()
            .bg(pal.dialog_bg)
            .shadow(crate::drawing_chrome::dialog_shadow())
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(28.))
                    .pl(px(6.))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Insert view"))
                    .child(div().id("die-close").w(px(28.)).h(px(28.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.insert.chooser = None;
                        this.insert.panel = false;
                        cx.notify();
                    }))),
            )
            .child(
                div().px(px(4.)).py(px(2.)).child(
                    h_flex()
                        .id("die-insert")
                        .h(px(26.))
                        .justify_center()
                        .gap(px(6.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xcccccc))
                        .bg(crate::theme::light().panel)
                        .text_size(px(13.33))
                        .cursor_pointer()
                        .map(|d| crate::ui_script::mark(d, || "button:Insert".into()))
                        .child(crate::assets::icon("insert", 16., cx))
                        .child("Insert")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_insert_chooser(cx))),
                ),
            )
            .child(row("View orientation", "Front", false))
            .child(row("View scale", "", true))
            .child(row("View simplification", "Auto", false))
            .child(h_flex().h(px(26.)).px(px(6.)).gap(px(6.)).text_size(px(12.)).child(div().size(px(12.)).border_1().border_color(gpui_kit::rgb(0xbbbbbb)).rounded(px(2.))).child("Show sketch appearances"))
            .into_any_element()
    }

    /// "Select a part or assembly".
    pub fn render_insert_chooser(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        // The tool's panel alone until [Insert] opens the chooser.
        if self.insert.chooser.is_none() && self.insert.panel && !matches!(self.tool, Tool::InsertView { .. }) {
            let panel = self.render_empty_insert_panel(cx);
            return Some(
                div()
                    .id("dr-insert-panel-wrap")
                    .absolute()
                    .top(px(DIALOG_TOP))
                    .left(px(self.dialog_left()))
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(panel)
                    .into_any_element(),
            );
        }
        let tab = self.insert.chooser?;
        let pal = p();
        let blue = gpui_kit::rgb(0x1651b0);
        let filter = self.chooser_filter();
        let query = self.chooser.search.as_ref().map(|s| s.read(cx).value().trim().to_lowercase()).unwrap_or_default();
        let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
        let doc = self.doc.read(cx);
        let doc_name = doc.name().to_string();
        // The tree lists this document, or the other document opened in
        // Other documents (at its version; no pictures).
        let other_doc = self.chooser.other_doc.clone().filter(|_| self.chooser.other);
        let tabs: Vec<(TabId, String, bool)> = match &other_doc {
            Some(d) => d.tabs.clone(),
            None => doc
                .session
                .doc
                .ordered_tabs()
                .into_iter()
                .filter_map(|t| match &t.content {
                    TabContent::PartStudio(_) => Some((t.id, t.name.clone(), false)),
                    TabContent::Assembly(_) => Some((t.id, t.name.clone(), true)),
                    _ => None,
                })
                .collect(),
        };
        let key_of = |id: TabId| match &other_doc {
            Some(d) => SourceKey { version: d.key(id).map(|k| k.0), ..SourceKey::studio(id) },
            None => SourceKey::studio(id),
        };
        if self.chooser.expand_single && other_doc.is_none() {
            let with_parts: Vec<TabId> = tabs.iter().filter(|t| !t.2).map(|t| t.0).filter(|id| self.sources.get(&key_of(*id)).is_some_and(|s| !s.context.bodies.is_empty())).collect();
            if let [only] = with_parts[..] {
                self.chooser.expand_single = false;
                if !self.chooser.open.contains(&only) {
                    self.chooser.open.push(only);
                }
            }
        }
        let tag = other_doc.as_ref().map_or(0, |d| d.tag());
        let pic_of = |id: TabId, what: ThumbOf| self.chooser.thumbs.get(&(tag, id, what)).cloned();
        // Live: blue underlined tabs, 12 px.
        let tab_btn = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .flex_1()
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .text_color(if on { blue } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).border_b_2().border_color(blue))
                .child(label)
        };
        let row_base = |id: SharedString, indent: f32| h_flex().id(id).min_h(px(22.)).pl(px(6. + indent)).pr(px(6.)).gap(px(6.)).cursor_pointer().hover(|d| d.bg(pal.row_hover)).text_size(px(13.33));
        let mut list = v_flex().id("dr-chooser-list").flex_1().min_h_0().overflow_y_scroll();
        // The rows as live's recorder reads the picker (a checkbox per row; `child` under a studio).
        let mut read_rows: Vec<serde_json::Value> = Vec::new();
        let row_read = |label: &str, child: bool| serde_json::json!({ "label": label, "kind": "boolean", "checked": false, "child": child });
        let mut any = false;
        let has_tabs = tabs.iter().any(|(_, _, asm)| *asm == (tab == 1));
        for (id, name, asm) in tabs {
            if asm != (tab == 1) {
                continue;
            }
            if asm {
                if !matches(&name) {
                    continue;
                }
                any = true;
                read_rows.push(row_read(&name, false));
                list = list.child(row_base(SharedString::from(format!("dr-ch-asm-{id}")), 20.).child(div().truncate().child(crate::ui::user_text(name))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_tree(id, None, cx))));
                continue;
            }
            // Part Studios on and nothing else: the studio rows themselves.
            if filter & !F_STUDIOS == 0 {
                if filter == 0 || !matches(&name) {
                    continue;
                }
                any = true;
                read_rows.push(row_read(&name, false));
                let pic = pic_of(id, ThumbOf::Part(None));
                list = list.child(row_base(SharedString::from(format!("dr-ch-ps-{id}")), 20.).min_h(px(38.)).child(thumb(pic)).child(div().truncate().child(crate::ui::user_text(name))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_tree(id, None, cx))));
                continue;
            }
            // The studio's rows the filters show, in body order: its flat
            // patterns above their parts, then its sketches (live
            // drawing/1394–1396).
            let mut children: Vec<ChildRow> = Vec::new();
            if filter & F_STUDIOS != 0 && !self.chooser.composite {
                children.push((name.clone(), ThumbOf::Part(None), None, false, None));
            }
            if let Some(src) = self.sources.get(&key_of(id)) {
                for b in src.context.bodies.iter() {
                    let listed = match (b.kind, &b.composite) {
                        (_, Some(_)) => filter & F_COMPOSITES != 0,
                        (noshape_model::display::BodyKind::Solid, None) => filter & F_PARTS != 0,
                        (noshape_model::display::BodyKind::Surface, None) => filter & F_SURFACES != 0,
                        (noshape_model::display::BodyKind::Wire, None) => filter & F_CURVES != 0,
                        _ => false,
                    };
                    if filter & F_FLAT != 0 && src.context.sheet_metal.contains_key(&b.id) {
                        children.push((format!("Flat pattern of {}", b.name), ThumbOf::Flat(b.id.clone()), Some(b.id.clone()), true, None));
                    }
                    if listed {
                        children.push((b.name.clone(), ThumbOf::Part(Some(b.id.clone())), Some(b.id.clone()), false, None));
                    }
                }
                if filter & F_SKETCHES != 0 {
                    children.extend(src.context.sketches.iter().map(|s| (s.name.clone(), ThumbOf::Sketch(s.feature), None, false, Some(s.feature))));
                }
            }
            let shown: Vec<_> = children.into_iter().filter(|c| matches(&c.0) || matches(&name)).collect();
            // Only studios with something to show are listed.
            if shown.is_empty() {
                continue;
            }
            any = true;
            read_rows.push(row_read(&name, false));
            let open = self.chooser.open.contains(&id) || !query.is_empty();
            let studio = name.clone();
            list = list.child(
                row_base(SharedString::from(format!("dr-ch-ps-{id}")), 0.)
                    .map(|d| crate::ui_script::mark(d, || format!("list_row:{studio}")))
                    .child(crate::assets::icon(if open { "chevron-down" } else { "chevron-right" }, 14., cx))
                    .child(div().truncate().child(crate::ui::user_text(name)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(i) = this.chooser.open.iter().position(|t| *t == id) {
                            this.chooser.open.remove(i);
                        } else {
                            this.chooser.open.push(id);
                        }
                        cx.notify();
                    })),
            );
            if !open {
                continue;
            }
            for (i, (label, what, part, flat, sketch)) in shown.into_iter().enumerate() {
                read_rows.push(row_read(&label, true));
                let pic = pic_of(id, what);
                let name = label.to_string();
                list = list.child(row_base(SharedString::from(format!("dr-ch-c-{id}-{i}")), 36.).min_h(px(38.)).map(|d| crate::ui_script::mark(d, || format!("list_row:{name}"))).child(thumb(pic)).child(div().truncate().child(label)).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if this.chooser.composite {
                        this.pick_composite(id, part.clone(), window, cx);
                        return;
                    }
                    this.pick_tree(id, part.clone(), cx);
                    if flat {
                        this.insert.flat = true;
                        this.set_insert_orientation(ViewOrientation::Top, cx);
                    }
                    if let Some(f) = sketch {
                        this.insert_sketch(Some(f), cx);
                    }
                })));
            }
        }
        // A filter or search with no match leaves the list empty.
        if !any && !has_tabs {
            list = list.child(div().p_2().text_size(px(12.)).text_color(pal.text_muted).child(if tab == 0 { "No Part Studios in this document." } else { "No assemblies in this document." }));
        }
        // Only the kinds the document has (live: Part Studios and Parts always).
        let mut present = F_STUDIOS | F_PARTS;
        for k in self.chooser_sources(cx) {
            let Some(src) = self.sources.get(&k) else { continue };
            for b in &src.context.bodies {
                present |= match (b.kind, &b.composite) {
                    (_, Some(_)) => F_COMPOSITES,
                    (noshape_model::display::BodyKind::Surface, None) => F_SURFACES,
                    (noshape_model::display::BodyKind::Wire, None) => F_CURVES,
                    _ => 0,
                };
            }
            if !src.context.sketches.is_empty() {
                present |= F_SKETCHES;
            }
            if !src.context.sheet_metal.is_empty() {
                present |= F_FLAT;
            }
        }
        // Live (drawing/1394, dom/d2-picker-current): 30 px `os-button os-clear`
        // toggles 35 px apart; on = `os-active` (#eaeaea pressed in under a
        // #b6b6b6 top edge, as the assembly list's ≡).
        let mut filters = h_flex().h(px(34.)).pl(px(7.)).pr(px(6.)).gap(px(5.));
        for (i, (tip, icon)) in FILTERS.iter().enumerate() {
            if present & (1 << i) == 0 {
                continue;
            }
            let on = filter & (1 << i) != 0;
            filters = filters.child(
                div()
                    .id(SharedString::from(format!("dr-ch-filter-{i}")))
                    .size(px(30.))
                    .flex_none()
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.))
                    .cursor_pointer()
                    .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                    .when(on, |d| d.bg(gpui_kit::rgb(0xeaeaea)).border_l_1().border_r_1().border_color(gpui_kit::rgb(0xe4e4e4)).child(div().absolute().top_0().left_0().right_0().h(px(1.)).bg(gpui_kit::rgb(0xb6b6b6))))
                    .child(crate::assets::icon(icon, 20., cx))
                    .tooltip(crate::ui::tooltip(*tip))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.chooser.filter = Some(this.chooser_filter() ^ (1 << i));
                        cx.notify();
                    })),
            );
        }
        let panel = v_flex()
            .id("dr-chooser")
            // The chooser in use again: the keyboard is its (Create version… sets it off after this).
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, _| this.chooser.keyboard_left = false))
            // Replace...'s chooser: Esc closes it, even from its search box (live2 drw-sheet-ref-replace).
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Escape, window, cx| {
                if this.chooser.composite {
                    this.close_composite_chooser();
                    window.focus(&this.focus, cx);
                    cx.stop_propagation();
                    cx.notify();
                } else if this.insert.replace.is_some() {
                    this.insert.chooser = None;
                    this.insert.replace = None;
                    this.insert.replace_part = None;
                    window.focus(&this.focus, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .w(px(300.))
            .max_h(px(582.))
            .flex_none()
            .bg(pal.dialog_bg)
            .shadow(crate::drawing_chrome::dialog_shadow())
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(28.))
                    .pl(px(4.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd3d7d9))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(if self.chooser.composite { "Select a composite part" } else { "Select a part or assembly" }))
                    .child(div().id("dr-chooser-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 12., pal.text_muted)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.chooser.composite = false;
                        this.insert.chooser = None;
                        cx.notify();
                    }))),
            )
            .child(
                h_flex()
                    .child(tab_btn("dr-ch-cur", "Current document", !self.chooser.other).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.chooser.other = false;
                        cx.notify();
                    })))
                    .child(tab_btn("dr-ch-other", "Other documents", self.chooser.other).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.chooser.other = true;
                        cx.notify();
                    }))),
            );
        // The Part Studios | Assemblies sub-tabs (both document tabs).
        let sub_tabs = h_flex()
            .border_b_1()
            .border_color(gpui_kit::rgb(0xeeeeee))
            .child(tab_btn("dr-ch-ps", "Part Studios", tab == 0).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.insert.chooser = Some(0);
                cx.notify();
            })))
            .child(tab_btn("dr-ch-asm", "Assemblies", tab == 1).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.insert.chooser = Some(1);
                cx.notify();
            })));
        // Live 170: "⤒ Import..." under the list (both document tabs).
        let import_row = |cx: &App| {
            h_flex()
                .id("dr-ch-import")
                .h(px(24.))
                .px(px(6.))
                .gap(px(6.))
                .border_t_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .cursor_pointer()
                .text_size(px(13.33))
                .hover(|d| d.bg(pal.row_hover))
                .child(crate::assets::icon("import", 14., cx))
                .child("Import...")
                .on_click(|_, window, cx| crate::drawing_hooks::import(window, cx))
        };
        let composite = self.chooser.composite;
        let panel = if self.chooser.other {
            match other_doc {
                // An opened document: its header, Part Studios | Assemblies,
                // search, filters, the changes box, then its tree.
                Some(d) => {
                    let panel = panel.child(self.render_other_header(&d, cx));
                    if self.chooser.other_graph {
                        panel.child(self.render_other_versions(&d, cx)).child(import_row(cx))
                    } else {
                        let changes = self.render_other_changes(&d, cx);
                        // View released items (when it has any): its
                        // released revisions instead of its tree.
                        let released = self.chooser.other_released && !d.released.is_empty();
                        let view_released = (!d.released.is_empty()).then(|| {
                            div().h(px(28.)).flex().items_center().px(px(7.)).child(crate::drawing_chrome::DrCheckbox::new("dr-ch-released").label("View released items").checked(released).on_click(cx.listener(|this, v: &bool, _, cx| {
                                this.chooser.other_released = *v;
                                cx.notify();
                            })))
                        });
                        let body = if released { self.render_released(&d, tab == 1, cx) } else { list.into_any_element() };
                        panel
                            .when(!composite, |p| p.child(sub_tabs))
                            .children(self.chooser.search.as_ref().map(|s| div().px(px(6.)).pt(px(6.)).child(Input::new(s).small())))
                            .children(view_released)
                            .when(tab == 0 && !released && !composite, |d| d.child(filters).child(div().h(px(1.)).bg(gpui_kit::rgb(0xeeeeee))))
                            .children(changes)
                            .child(body)
                            .when(!composite, |p| p.child(import_row(cx)))
                    }
                }
                None => {
                    let other = self.render_other_documents(cx);
                    panel.child(other).when(!composite, |p| p.child(import_row(cx)))
                }
            }
        } else {
            // Live 170: the document, its workspace or chosen version, and
            // the Create version… / Version graph buttons.
            let place = self.chooser.version.as_ref().map(|v| v.name.clone()).unwrap_or_else(|| "Main".into());
            let doc = self.doc.clone();
            let header = h_flex()
                .px(px(6.))
                .py(px(4.))
                .gap(px(4.))
                .child(v_flex().flex_1().min_w_0().child(div().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(crate::ui::user_text(doc_name))).child(h_flex().gap(px(4.)).text_size(px(13.)).text_color(gpui_kit::rgb(0x999999)).child(crate::assets::icon("branch", 12., cx)).child(place)))
                // The drawing holds the keyboard under the modal: Esc gives it back to the drawing, not the search box (live
                // ver-insert-create-version-esc).
                .child(chooser_square("dr-ch-version", "version", "Create version…", false, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.chooser.keyboard_left = true;
                    crate::drawing_hooks::create_version(doc.clone(), window, cx);
                })))
                .child(chooser_square("dr-ch-graph", "version-graph", "Version graph", self.chooser.graph, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.chooser.graph = !this.chooser.graph;
                    cx.notify();
                })));
            let panel = panel.child(header);
            if self.chooser.graph {
                // Version graph: the workspace, then the versions (newest first).
                let cur = self.chooser.version.clone();
                let mut rows: Vec<(Option<noshape_model::drawing::PinnedVersion>, String)> = vec![(None, "Main".into())];
                rows.extend(self.doc.read(cx).session.store.list_versions().unwrap_or_default().into_iter().rev().map(|v| (Some(noshape_model::drawing::PinnedVersion { locked: false, id: v.id.to_string(), name: v.name.clone() }), v.name)));
                let mut col = v_flex().id("dr-ch-versions").flex_1().min_h_0().overflow_y_scroll();
                for (i, (pin, label)) in rows.into_iter().enumerate() {
                    let on = pin == cur;
                    col = col.child(
                        h_flex()
                            .id(SharedString::from(format!("dr-ch-v-{i}")))
                            .h(px(28.))
                            .px(px(10.))
                            .gap(px(6.))
                            .cursor_pointer()
                            .text_size(px(13.33))
                            .when(on, |d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                            .hover(|d| d.bg(pal.row_hover))
                            .child(crate::assets::icon(if pin.is_none() { "branch" } else { "version" }, 14., cx))
                            .child(label)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.chooser.version = pin.clone();
                                this.chooser.graph = false;
                                cx.notify();
                            })),
                    );
                }
                panel.child(col)
            } else {
                // (Select a composite part: no kinds, filters or Import, live2 2278.)
                panel
                    .when(!composite, |p| p.child(sub_tabs))
                    .children(self.chooser.search.as_ref().map(|s| div().px(px(6.)).pt(px(6.)).child(Input::new(s).small())))
                    .when(tab == 0 && !composite, |d| d.child(filters).child(div().h(px(1.)).bg(gpui_kit::rgb(0xeeeeee))))
                    .child(list)
                    .when(!composite, |p| p.child(import_row(cx)))
            }
        };
        // Beside the Insert view panel (the empty one until a source is chosen).
        // (Select a composite part: beside the Cut list table panel.)
        let left = if matches!(self.tool, Tool::InsertView { .. }) || composite { None } else { Some(self.render_empty_insert_panel(cx)) };
        // Live's recorder since 2026-11-14 reads the open picker as a feature dialog (doc-plus-create-drawing-ok): its
        // tabs, the search box and the rows; `panel` keeps what the panel beside it read (older takes read that).
        let panel_read = crate::ui_script::open_dialog();
        let other = self.chooser.other;
        let placeholder = if composite { "Search composite parts" } else { "Search parts or sketches" };
        let search_text = self.chooser.search.as_ref().map(|s| s.read(cx).value().to_string()).unwrap_or_default();
        crate::ui_script::note_dialog(|| {
            let mut fields = vec![serde_json::json!({ "label": placeholder, "kind": "string", "text": search_text })];
            fields.extend(read_rows);
            serde_json::json!({
                "title": null,
                "kind": "feature",
                "tabs": [
                    { "label": "Current document", "selected": !other },
                    { "label": "Other documents", "selected": other },
                    { "label": "Part Studios", "selected": tab == 0 },
                    { "label": "Assemblies", "selected": tab == 1 },
                ],
                "fields": fields,
                "error": null,
                "ok": null,
                "panel": panel_read,
            })
        });
        let offset = match (&left, composite) {
            (Some(_), _) => 0.,
            (None, true) => 214.,
            (None, false) => 251.,
        };
        Some(
            h_flex()
                .id("dr-chooser-wrap")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left() + offset))
                .items_start()
                .gap(px(4.))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(left)
                .child(panel)
                .into_any_element(),
        )
    }
}

/// The document header's 30 px buttons (Create version…, Version graph).
fn chooser_square(id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &App) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(30.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .border_color(gpui_kit::rgb(0xdddddd))
        .cursor_pointer()
        .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
        .child(crate::assets::icon(icon, 20., cx))
        .tooltip(crate::ui::tooltip(tip))
        .map(|d| crate::ui_script::mark(d, || format!("button:{tip}")))
}

/// A row picture (Onshape: about 56 × 32 px), blank until rendered.
fn thumb(img: Option<Arc<RenderImage>>) -> AnyElement {
    let base = div().w(px(56.)).h(px(32.)).flex_none();
    match img {
        Some(i) => base.child(gpui_kit::img(i).size_full()).into_any_element(),
        None => base.into_any_element(),
    }
}

/// A camera looking at the box normal to a sketch plane (its X right), as
/// the assembly Insert dialog's sketch pictures.
fn fit_camera_normal(lo: glam::DVec3, hi: glam::DVec3, aspect: f64, frame: &noshape_model::display::PlaneFrame) -> noshape_render::Camera {
    let mut camera = noshape_render::Camera { rotation: noshape_render::Camera::look_rotation(-frame.normal, frame.y), ..Default::default() };
    let (right, up) = (camera.right(), camera.up());
    let center = (lo + hi) * 0.5;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for i in 0..8 {
        let c = glam::DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }) - center;
        let (x, y) = (c.dot(right), c.dot(up));
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let target = center + right * ((x0 + x1) * 0.5) + up * ((y0 + y1) * 0.5);
    let half_h = ((y1 - y0) * 0.5).max((x1 - x0) * 0.5 / aspect).max(1e-5);
    camera.fit_sphere(target, half_h * 1.1 / 1.15, aspect.max(1.0));
    camera
}

/// PNG -> GPUI image (BGRA); the white background becomes transparent so
/// the pictures sit on hovered rows.
fn decode(png: &[u8]) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[0] >= 250 && px[1] >= 250 && px[2] >= 250 {
            *px = [0; 4];
        }
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}

#[cfg(test)]
mod released_tests {
    use noshape_model::release::{ItemKind, Package, PackageKind, ReleaseItem, ReleaseLog};

    fn released_item(name: &str, kind: ItemKind, pn: &str, rev: &str) -> ReleaseItem {
        ReleaseItem { tab: noshape_model::document::TabId(7), part: None, name: name.into(), kind, part_number: pn.into(), revision: rev.into(), not_revision_managed: false, reverted: false, config: Default::default(), document: None }
    }

    fn package(id: u64, kind: PackageKind, version: Option<&str>, items: Vec<ReleaseItem>) -> Package {
        Package { id, kind, name: format!("R{id}"), notes: String::new(), created: 0, version: version.map(String::from), items, re_releasable: false, approvers: Vec::new(), observers: Vec::new(), state: None, decided_by: None, comments: Vec::new() }
    }

    /// View released items: parts and assemblies, newest release first;
    /// drawings, obsoleted revisions and releases without a version left out.
    #[test]
    fn released_items_newest_first_without_obsolete() {
        let mut log = ReleaseLog::default();
        log.packages.push(package(1, PackageKind::Release, Some("00000000-0000-0000-0000-000000000001"), vec![released_item("Bracket", ItemKind::Part, "PN-1", "A"), released_item("Drawing", ItemKind::Drawing, "PN-9", "A")]));
        log.packages.push(package(2, PackageKind::Release, Some("00000000-0000-0000-0000-000000000002"), vec![released_item("Bracket", ItemKind::Part, "PN-1", "B"), released_item("Gearbox", ItemKind::Assembly, "PN-2", "A")]));
        log.packages.push(package(3, PackageKind::Obsoletion, None, vec![released_item("Bracket", ItemKind::Part, "PN-1", "A")]));
        log.packages.push(package(4, PackageKind::Release, None, vec![released_item("Lost", ItemKind::Part, "PN-3", "A")]));
        let r: Vec<(String, String, bool)> = super::released_items(&log, &|s| s.parse().ok()).into_iter().map(|r| (r.name, r.revision, r.assembly)).collect();
        assert_eq!(r, vec![("Bracket".into(), "B".into(), false), ("Gearbox".into(), "A".into(), true)]);
    }
}
