//! Drawing editor: Insert view › "Select a part or assembly" (live
//! drawing/170–173, behaviours "Flat pattern view"): a 300 px flyout right
//! of the Insert view panel. Current document | Other documents; the
//! document name and its branch; Part Studios | Assemblies; "Search parts
//! or sketches"; the filters Part Studios | Parts | Sketches | Flat patterns
//! (one at a time, Parts first); then ▸ studio rows that expand to pictures
//! of their parts ("Flat pattern of Part 1" above "Part 1"), sketches or
//! flat patterns. The pictures are rendered offscreen in the background
//! (the document-thumbnail look), cached per studio content hash.

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

/// The filters (tooltip, icon), live order.
const FILTERS: [(&str, &str); 4] = [("Part Studios", "part-studio"), ("Parts", "part"), ("Sketches", "sketch"), ("Flat patterns", "flat-pattern")];

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
    /// The filter (index into [`FILTERS`]; default Parts).
    pub filter: Option<usize>,
    /// Expanded studios.
    pub open: Vec<TabId>,
    pub search: Option<Entity<InputState>>,
    /// Pictures per (studio, what), the hash they were made for, busy.
    pub thumbs: HashMap<(TabId, ThumbOf), Arc<RenderImage>>,
    pub made_for: HashMap<TabId, u64>,
    pub busy: bool,
    /// Other documents tab; its "Search or paste URL" box, category, the
    /// scanned documents, a search and the opened document.
    pub other: bool,
    pub url: Option<Entity<InputState>>,
    pub other_cat: Option<usize>,
    pub other_docs: Option<Vec<crate::start_library::DocEntry>>,
    pub other_query: Option<String>,
    pub other_doc: Option<OtherDoc>,
    /// Current document › Version graph: the list shown, and the version
    /// chosen (new views are pinned to it).
    pub graph: bool,
    pub version: Option<noshape_model::drawing::PinnedVersion>,
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
        let mut d = OtherDoc { path: path.to_path_buf(), name, versions, changes, version: 0, tabs: Vec::new() };
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

    /// Its reference key for tab `tab` at the shown version, and the label.
    fn key(&self, tab: TabId) -> Option<(String, String)> {
        let v = self.versions.get(self.version)?;
        Some((crate::derived::ExternalRef { path: self.path.clone(), microversion: v.microversion, tab: tab.to_string() }.key(), format!("{} {}", self.name, v.label)))
    }
}

impl DrawingEditor {
    /// Create the search field with the chooser (called from render).
    pub fn sync_chooser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (self.insert.chooser.is_some(), self.chooser.search.is_some()) {
            (true, false) => {
                self.chooser.search = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Search parts or sketches")));
                self.chooser.url = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Search or paste URL")));
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

    fn chooser_filter(&self) -> usize {
        self.chooser.filter.unwrap_or(1)
    }

    /// Render the pictures of studios that changed since the last time.
    fn request_chooser_thumbs(&mut self, cx: &mut Context<Self>) {
        if self.chooser.busy {
            return;
        }
        let doc = self.doc.read(cx).session.doc.clone();
        let todo: Vec<(TabId, u64)> = doc
            .ordered_tabs()
            .into_iter()
            .filter(|t| matches!(t.content, TabContent::PartStudio(_)))
            .filter_map(|t| views::source_hash(&doc, &SourceKey::studio(t.id)).map(|h| (t.id, h)))
            .filter(|(t, h)| self.chooser.made_for.get(t) != Some(h))
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
                                for b in &mut d.bodies {
                                    b.hidden = &b.id != p;
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
                    this.chooser.thumbs.retain(|(t, _), _| *t != tab);
                    this.chooser.made_for.insert(tab, h);
                }
                this.chooser.thumbs.extend(out);
                cx.notify();
            });
        })
        .detach();
    }

    /// A source of this document (at the Version graph's version, if chosen).
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
            let docs = cx.background_spawn(async { crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.chooser.other_docs = Some(docs);
                cx.notify();
            });
        })
        .detach();
    }

    /// "Search or paste URL" › Enter / 🔍: a document path opens it; other
    /// text lists the documents whose name contains it.
    fn search_other(&mut self, cx: &mut Context<Self>) {
        let text = self.chooser.url.as_ref().map(|s| s.read(cx).value().trim().to_string()).unwrap_or_default();
        let path = std::path::PathBuf::from(text.trim_start_matches("file://"));
        if path.extension().is_some_and(|e| e == crate::start_library::EXT) && path.exists() {
            self.chooser.other_doc = OtherDoc::open(&path).ok();
        } else {
            self.ensure_other_docs(cx);
            self.chooser.other_query = (!text.is_empty()).then_some(text);
        }
        cx.notify();
    }

    /// The Other documents tab (live as the assembly Insert dialog's).
    fn render_other_documents(&mut self, tab: u8, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(0x1651b0);
        let row = |id: SharedString| h_flex().id(id).h(px(32.)).pl(px(16.)).pr(px(6.)).gap(px(6.)).cursor_pointer().text_size(px(14.)).hover(|d| d.bg(pal.row_hover));
        let mut col = v_flex().flex_1().min_h_0();
        // An opened document: its name and version, the changes since the
        // last version, then its Part Studios | Assemblies.
        if let Some(d) = self.chooser.other_doc.clone() {
            let label = d.versions.get(d.version).map(|v| v.label.clone()).unwrap_or_default();
            col = col.child(
                h_flex()
                    .px(px(6.))
                    .py(px(4.))
                    .gap(px(6.))
                    .child(div().id("dr-ch-other-back").cursor_pointer().child(crate::ui::glyph("ui-chevron-left", 14., pal.text)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.chooser.other_doc = None;
                        cx.notify();
                    })))
                    .child(v_flex().flex_1().min_w_0().child(div().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(d.name.clone())).child(h_flex().gap(px(4.)).text_size(px(13.)).text_color(gpui_kit::rgb(0x999999)).child(crate::assets::icon("branch", 12., cx)).child(label))),
            );
            if d.changes > 0 {
                col = col.child(div().mx(px(4.)).mb(px(4.)).p(px(4.)).rounded(px(5.)).border_1().border_color(gpui_kit::rgb(0xb2ddf6)).bg(gpui_kit::rgb(0xdef1fb)).text_size(px(14.)).text_color(gpui_kit::rgb(0x094174)).child(format!("{} change{} since the last version.", d.changes, if d.changes == 1 { "" } else { "s" })));
            }
            for (i, (t, name, asm)) in d.tabs.iter().cloned().enumerate() {
                if asm != (tab == 1) {
                    continue;
                }
                let d2 = d.clone();
                col = col.child(row(SharedString::from(format!("dr-ch-ext-{i}"))).child(crate::assets::icon(if asm { "assembly" } else { "part-studio" }, 18., cx)).child(div().truncate().child(name)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some((key, label)) = d2.key(t) {
                        this.choose_external(key, t, label, cx);
                    }
                })));
            }
            return col.into_any_element();
        }
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
        let mut docs: Vec<crate::start_library::DocEntry> = self.chooser.other_docs.clone().unwrap_or_default().into_iter().filter(|d| !d.trashed && !d.broken && d.path != me).collect();
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
            list = list.child(row(SharedString::from(format!("dr-ch-doc-{i}"))).child(crate::assets::icon("document", 18., cx)).child(div().truncate().child(d.name)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.chooser.other_doc = OtherDoc::open(&path).ok();
                cx.notify();
            })));
        }
        col.child(list).into_any_element()
    }

    /// The Insert view panel before a source is chosen (live drawing/170):
    /// Insert, then the greyed fields.
    fn render_empty_insert_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let row = |label: &'static str, value: &'static str| {
            h_flex()
                .h(px(28.))
                .px(px(6.))
                .child(div().w(px(105.)).flex_none().text_size(px(12.)).child(label))
                .child(h_flex().flex_1().h(px(22.)).px(px(4.)).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).text_size(px(12.)).text_color(gpui_kit::rgb(0xaaaaaa)).child(div().flex_1().child(value)).child(crate::assets::icon("drawing-tool-caret", 12., cx)))
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
                        cx.notify();
                    }))),
            )
            .child(
                div().px(px(4.)).py(px(2.)).child(
                    h_flex().h(px(26.)).justify_center().gap(px(6.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).bg(gpui_kit::white()).text_size(px(13.33)).child(crate::assets::icon("insert", 16., cx)).child("Insert"),
                ),
            )
            .child(row("View orientation", "Front"))
            .child(row("View scale", ""))
            .child(row("View simplification", "Auto"))
            .child(h_flex().h(px(26.)).px(px(6.)).gap(px(6.)).text_size(px(12.)).child(div().size(px(12.)).border_1().border_color(gpui_kit::rgb(0xbbbbbb)).rounded(px(2.))).child("Show sketch appearances"))
            .into_any_element()
    }

    /// "Select a part or assembly".
    pub fn render_insert_chooser(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let tab = self.insert.chooser?;
        let pal = p();
        let blue = gpui_kit::rgb(0x1651b0);
        let filter = self.chooser_filter();
        let query = self.chooser.search.as_ref().map(|s| s.read(cx).value().trim().to_lowercase()).unwrap_or_default();
        let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
        let doc = self.doc.read(cx);
        let doc_name = doc.name().to_string();
        let tabs: Vec<(TabId, String, bool)> = doc
            .session
            .doc
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::PartStudio(_) => Some((t.id, t.name.clone(), false)),
                TabContent::Assembly(_) => Some((t.id, t.name.clone(), true)),
                _ => None,
            })
            .collect();
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
                list = list.child(row_base(SharedString::from(format!("dr-ch-asm-{id}")), 20.).child(div().truncate().child(name)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_current(id, None, cx))));
                continue;
            }
            // Part Studios filter: the studio rows themselves.
            if filter == 0 {
                if !matches(&name) {
                    continue;
                }
                any = true;
                let pic = self.chooser.thumbs.get(&(id, ThumbOf::Part(None))).cloned();
                list = list.child(row_base(SharedString::from(format!("dr-ch-ps-{id}")), 20.).min_h(px(38.)).child(thumb(pic)).child(div().truncate().child(name)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_current(id, None, cx))));
                continue;
            }
            // Children per filter: parts, sketches, or flat patterns (with
            // their parts).
            let mut children: Vec<ChildRow> = Vec::new();
            if let Some(src) = self.sources.get(&SourceKey::studio(id)) {
                match filter {
                    2 => children.extend(src.context.sketches.iter().map(|s| (s.name.clone(), ThumbOf::Sketch(s.feature), None, false, Some(s.feature)))),
                    _ => {
                        for b in views::view_bodies(&src.context, None) {
                            let sm = src.context.sheet_metal.contains_key(&b.id);
                            if filter == 3 && !sm {
                                continue;
                            }
                            if sm && filter == 3 {
                                children.push((format!("Flat pattern of {}", b.name), ThumbOf::Flat(b.id.clone()), Some(b.id.clone()), true, None));
                            }
                            children.push((b.name.clone(), ThumbOf::Part(Some(b.id.clone())), Some(b.id.clone()), false, None));
                        }
                    }
                }
            }
            let shown: Vec<_> = children.into_iter().filter(|c| matches(&c.0) || matches(&name)).collect();
            if shown.is_empty() && (filter == 3 || !query.is_empty()) {
                continue;
            }
            any = true;
            let open = self.chooser.open.contains(&id) || !query.is_empty();
            list = list.child(
                row_base(SharedString::from(format!("dr-ch-ps-{id}")), 0.)
                    .child(crate::assets::icon(if open { "chevron-down" } else { "chevron-right" }, 14., cx))
                    .child(div().truncate().child(name))
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
                let pic = self.chooser.thumbs.get(&(id, what)).cloned();
                list = list.child(row_base(SharedString::from(format!("dr-ch-c-{id}-{i}")), 36.).min_h(px(38.)).child(thumb(pic)).child(div().truncate().child(label)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.pick_current(id, part.clone(), cx);
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
        let mut filters = h_flex().h(px(34.)).px(px(6.)).gap(px(6.));
        for (i, (tip, icon)) in FILTERS.iter().enumerate() {
            let on = i == filter;
            filters = filters.child(
                div()
                    .id(SharedString::from(format!("dr-ch-filter-{i}")))
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                    .when(!on, |d| d.hover(|d| d.bg(gpui_kit::rgb(0xe6ebef))))
                    .child(crate::assets::icon(icon, 20., cx))
                    .tooltip(crate::ui::tooltip(*tip))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.chooser.filter = Some(i);
                        cx.notify();
                    })),
            );
        }
        let panel = v_flex()
            .id("dr-chooser")
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
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Select a part or assembly"))
                    .child(div().id("dr-chooser-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 12., pal.text_muted)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
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
        let panel = if self.chooser.other {
            let opened = self.chooser.other_doc.is_some();
            let other = self.render_other_documents(tab, cx);
            panel.when(opened, |d| d.child(sub_tabs)).child(other)
        } else {
            // Live 170: the document, its workspace or chosen version, and
            // the Create version… / Version graph buttons.
            let square = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &App| {
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
            };
            let place = self.chooser.version.as_ref().map(|v| v.name.clone()).unwrap_or_else(|| "Main".into());
            let doc = self.doc.clone();
            let header = h_flex()
                .px(px(6.))
                .py(px(4.))
                .gap(px(4.))
                .child(v_flex().flex_1().min_w_0().child(div().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(doc_name)).child(h_flex().gap(px(4.)).text_size(px(13.)).text_color(gpui_kit::rgb(0x999999)).child(crate::assets::icon("branch", 12., cx)).child(place)))
                .child(square("dr-ch-version", "version", "Create version…", false, cx).on_click(move |_, window, cx| crate::history_panel::open_create_version_in(doc.clone(), window, cx)))
                .child(square("dr-ch-graph", "version-graph", "Version graph", self.chooser.graph, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.chooser.graph = !this.chooser.graph;
                    cx.notify();
                })));
            let panel = panel.child(header);
            if self.chooser.graph {
                // Version graph: the workspace, then the versions (newest first).
                let cur = self.chooser.version.clone();
                let mut rows: Vec<(Option<noshape_model::drawing::PinnedVersion>, String)> = vec![(None, "Main".into())];
                rows.extend(self.doc.read(cx).session.store.list_versions().unwrap_or_default().into_iter().rev().map(|v| (Some(noshape_model::drawing::PinnedVersion { id: v.id.to_string(), name: v.name.clone() }), v.name)));
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
                panel
                    .child(sub_tabs)
                    .children(self.chooser.search.as_ref().map(|s| div().px(px(6.)).pt(px(6.)).child(Input::new(s).small())))
                    .when(tab == 0, |d| d.child(filters).child(div().h(px(1.)).bg(gpui_kit::rgb(0xeeeeee))))
                    .child(list)
                    // Live 170: "⤒ Import..." under the list.
                    .child(
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
                            .on_click(|_, window, cx| crate::workspace::request_import(window, cx)),
                    )
            }
        };
        // Beside the Insert view panel (the empty one until a source is chosen).
        let left = if matches!(self.tool, Tool::InsertView { .. }) { None } else { Some(self.render_empty_insert_panel(cx)) };
        Some(
            h_flex()
                .id("dr-chooser-wrap")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left() + if left.is_some() { 0. } else { 251. }))
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
