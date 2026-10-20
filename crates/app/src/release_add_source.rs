//! Add Items to Release › items from a version: the "Version graph" of the
//! current document and the "Other documents" tab (Onshape's select-item
//! widget, bundle template: the document title row with "Create version…"
//! and "Version graph"; Other documents' filters, "Search or paste URL" and
//! document list, as in the Assembly Insert dialog). Items added from a
//! version carry a [`LinkedDocument`] (Help
//! Release__selecting_parts_for_a_release_candidate); one already released
//! at that version is listed for reference with its revision (Help
//! Release__typical_release_workflow › Releases and linked documents).

use super::{AddItemsDialog, Entry, META_KEY, release_log};
use crate::derived::{ExternalRef, VersionChoice, external_context, external_doc_info, external_snapshot};
use crate::doc_model::DocModel;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::Document;
use noshape_model::document::{TabContent, TabId};
use noshape_model::reference::BodyId;
use noshape_model::release::{ItemKind, LinkedDocument, PackageKind, ReleaseItem, ReleaseLog};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Other documents' filters (live assembly/71, same widget).
const FILTERS: [(&str, &str); 5] = [("Owned by me", "src-owned"), ("Recently opened", "src-recent"), ("Created by me", "src-created"), ("Shared with me", "src-shared"), ("Public", "src-public")];

/// A document whose versions items are added from.
pub(super) struct Source {
    pub path: PathBuf,
    pub name: String,
    /// The releasing document itself (its versions).
    pub this_doc: bool,
    /// Named versions, newest first, then "Start" (the document's first
    /// microversion; Onshape documents always have it).
    pub versions: Vec<VersionChoice>,
    /// Workspace changes since the last version (live assembly/612:
    /// "N changes since the last version.").
    pub changes: usize,
    /// The chosen version; `None`: the workspace (this document only).
    pub version: Option<usize>,
    /// The document at the chosen version.
    pub doc: Option<Arc<Document>>,
    /// The document's release log (its releases mark versions).
    pub log: ReleaseLog,
    /// Tab part numbers of that document (store meta).
    pub tab_numbers: HashMap<TabId, String>,
    /// Regenerated parts of its Part Studios at the version.
    pub parts: HashMap<TabId, Vec<(BodyId, String)>>,
    /// The Version graph replaces the list.
    pub graph: bool,
}

/// A linked row's Edit properties / Export.
#[derive(Clone, Copy)]
pub(super) enum LinkedAction {
    Properties,
    Export,
}

/// Edit properties / Export of an item from a version (another document's,
/// or the releasing document's own): the document opens at that version in a
/// window of its own, read-only (Onshape opens a version in a new tab), and
/// the action runs there. `this_doc` is the releasing document's file.
pub(super) fn run_linked(item: &ReleaseItem, this_doc: &std::path::Path, action: LinkedAction, cx: &mut App) -> bool {
    let Some(l) = item.document.as_ref() else { return false };
    let Ok(mv) = l.version.parse::<noshape_store::MicroversionId>() else { return false };
    let path = if l.path.is_empty() { this_doc.to_path_buf() } else { PathBuf::from(&l.path) };
    let Some(gpu) = cx.try_global::<crate::gpu::GpuGlobal>().map(|g| g.0.clone()) else { return false };
    let Ok((handle, ws)) = crate::files::open_version_window(path, mv, gpu, cx) else { return false };
    let mut local = item.clone();
    local.document = None;
    // Once the new window has built its tab views and regenerated.
    cx.spawn(async move |cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(800)).await;
        let _ = handle.update(cx, move |_, window, cx| {
            let d = ws.read(cx).doc.clone();
            match action {
                LinkedAction::Export => ws.update(cx, |w, cx| w.export_item(local.tab, local.part.iter().cloned().collect(), window, cx)),
                LinkedAction::Properties => super::open_item_properties(d, local, |_, _, _| {}, window, cx),
            }
        });
    })
    .detach();
    true
}

/// Other documents' browsing state.
pub(super) struct Browse {
    pub filter: Option<usize>,
    pub query: Option<String>,
    pub docs: Option<Vec<crate::start_library::DocEntry>>,
    pub url: Entity<InputState>,
}

impl Source {
    pub fn load(path: PathBuf, this_doc: bool, version: Option<usize>) -> anyhow::Result<Self> {
        let info = external_doc_info(&path)?;
        let mut versions: Vec<VersionChoice> = info.versions.into_iter().filter(|v| v.label != "Current state").collect();
        let store = noshape_store::Document::open_read_only(&path)?;
        // As the Insert dialog (asb's OtherDoc): the root is "Start", and the
        // changes are the steps from the head back to a version.
        let graph = store.graph()?;
        if let Some(root) = graph.microversions.iter().find(|m| m.parents.is_empty())
            && !versions.iter().any(|v| v.microversion == root.id)
        {
            versions.push(VersionChoice { label: "Start".into(), microversion: root.id });
        }
        let marks: std::collections::HashSet<noshape_store::MicroversionId> = versions.iter().map(|v| v.microversion).collect();
        let parent: HashMap<noshape_store::MicroversionId, Option<noshape_store::MicroversionId>> = graph.microversions.iter().map(|m| (m.id, m.parents.first().copied())).collect();
        let mut at = Some(store.head(store.default_branch()?)?);
        let mut changes = 0;
        while let Some(mv) = at.filter(|mv| !marks.contains(mv)) {
            changes += 1;
            at = parent.get(&mv).copied().flatten();
        }
        let log = store.get_meta(META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
        let mut s = Self { path, name: info.name, this_doc, versions, changes, version: None, doc: None, log, tab_numbers: HashMap::new(), parts: HashMap::new(), graph: false };
        s.choose(version);
        let ids: Vec<TabId> = s.doc.iter().flat_map(|d| d.tabs.iter().map(|t| t.id)).collect();
        for id in ids {
            if let Some(pn) = store.get_meta(&format!("tab_part_number:{}", id.0)).ok().flatten() {
                s.tab_numbers.insert(id, String::from_utf8_lossy(&pn).to_string());
            }
        }
        Ok(s)
    }

    /// Switch to a version (`None`: the workspace).
    pub fn choose(&mut self, version: Option<usize>) {
        self.version = version.filter(|i| *i < self.versions.len());
        self.doc = self.version.and_then(|i| external_snapshot(&self.path, self.versions[i].microversion).ok());
        self.parts.clear();
        self.graph = false;
    }

    /// The linked-document reference of the chosen version.
    pub fn linked(&self) -> Option<LinkedDocument> {
        self.chosen().map(|v| LinkedDocument { path: if self.this_doc { String::new() } else { self.path.display().to_string() }, name: self.name.clone(), version: v.microversion.to_string(), version_name: v.label.clone() })
    }

    fn chosen(&self) -> Option<&VersionChoice> {
        self.version.and_then(|i| self.versions.get(i))
    }

    /// The external key of a Part Studio at the chosen version.
    fn studio_key(&self, tab: TabId) -> Option<ExternalRef> {
        self.chosen().map(|v| ExternalRef { path: self.path.clone(), microversion: v.microversion, tab: tab.to_string() })
    }

    /// The release item of an object at the chosen version.
    pub fn item(&self, releasing: &ReleaseLog, tab: TabId, part: Option<BodyId>, name: String, kind: ItemKind) -> Option<ReleaseItem> {
        let doc = self.doc.as_ref()?;
        let v = self.chosen()?;
        let part_number = match (doc.tab(tab).map(|t| &t.content), &part) {
            (Some(TabContent::PartStudio(ps)), Some(b)) => ps.part_properties.get(b).and_then(|p| p.part_number.clone()).unwrap_or_default(),
            (Some(TabContent::Assembly(a)), _) => a.properties.part_number.clone().unwrap_or_default(),
            _ => self.tab_numbers.get(&tab).cloned().unwrap_or_default(),
        };
        let version = v.microversion.to_string();
        // Released at that version: listed for reference ("Including a
        // Released object in a new Release does not change the object's
        // State or Revision").
        let released = self.log.packages.iter().filter(|p| p.kind == PackageKind::Release && p.version.as_deref() == Some(version.as_str())).flat_map(|p| p.items.iter()).find(|i| i.tab == tab && i.part == part && i.kind == kind && i.document.is_none() && !i.reverted);
        let (revision, reverted) = match released {
            Some(r) => (r.revision.clone(), true),
            None => (releasing.next_revision(&part_number), false),
        };
        Some(ReleaseItem {
            tab,
            part,
            name,
            kind,
            part_number,
            revision,
            not_revision_managed: false,
            reverted,
            config: Default::default(),
            document: self.linked(),
        })
    }
}

/// The document a linked item's picture is rendered from.
pub(super) fn linked_snapshot(doc: &DocModel, item: &ReleaseItem) -> Option<Arc<Document>> {
    let l = item.document.as_ref()?;
    let path = if l.path.is_empty() { doc.session.path.clone() } else { PathBuf::from(&l.path) };
    external_snapshot(&path, l.version.parse().ok()?).ok()
}

/// Record a release in the linked documents' logs: "the version in the
/// linked document will also be marked as Released and the Release
/// attached to the version will be the one from the current document".
pub(super) fn mark_linked_versions(pkg: &noshape_model::release::Package) -> anyhow::Result<()> {
    let mut groups: Vec<(String, String)> = pkg.items.iter().filter(|i| !i.reverted).filter_map(|i| i.document.as_ref().filter(|l| !l.path.is_empty()).map(|l| (l.path.clone(), l.version.clone()))).collect();
    groups.sort();
    groups.dedup();
    let mut paths: Vec<String> = groups.iter().map(|(p, _)| p.clone()).collect();
    paths.dedup();
    for path in paths {
        let mut store = noshape_store::Document::open(&path)?;
        let mut log = store.get_meta(META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
        for (_, v) in groups.iter().filter(|(p, _)| *p == path) {
            let items: Vec<ReleaseItem> = pkg.items.iter().filter(|i| !i.reverted && i.document.as_ref().is_some_and(|l| l.path == path && l.version == *v)).map(|i| ReleaseItem { document: None, ..i.clone() }).collect();
            log.record(noshape_model::release::Package { items, version: Some(v.clone()), ..pkg.clone() });
        }
        store.set_meta(META_KEY, Some(&log.to_json()))?;
    }
    Ok(())
}

impl AddItemsDialog {
    /// The rows of the chosen type at the source's version.
    pub(super) fn source_entries(&self, kind: usize, matches: &dyn Fn(&str) -> bool, cx: &App) -> Vec<Entry> {
        let Some(src) = &self.source else { return Vec::new() };
        let Some(doc) = src.doc.clone() else { return Vec::new() };
        let releasing = release_log(self.doc.read(cx));
        let mut out = Vec::new();
        for t in doc.ordered_tabs() {
            let tab = t.id;
            match (&t.content, kind) {
                (TabContent::PartStudio(_), 0) => {
                    let parts: Vec<ReleaseItem> = src.parts.get(&tab).into_iter().flatten().filter(|(_, n)| matches(n) || matches(&t.name)).filter_map(|(b, n)| src.item(&releasing, tab, Some(b.clone()), n.clone(), ItemKind::Part)).collect();
                    if matches(&t.name) || !parts.is_empty() {
                        out.push(Entry::Studio(tab, t.name.clone(), parts, None));
                    }
                }
                (TabContent::Assembly(_), 1) | (TabContent::File(_), 2) | (TabContent::Drawing(_), 3) | (TabContent::VariableStudio(_), 4) if matches(&t.name) => {
                    let (kind, icon) = match &t.content {
                        TabContent::Assembly(_) => (ItemKind::Assembly, "assembly"),
                        TabContent::Drawing(_) => (ItemKind::Drawing, "drawing"),
                        TabContent::VariableStudio(_) => (ItemKind::VariableStudio, "variable-studio"),
                        _ => (ItemKind::File, "file"),
                    };
                    if let Some(it) = src.item(&releasing, tab, None, t.name.clone(), kind) {
                        out.push(Entry::Tab(it, icon));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Expanding a Part Studio of a version regenerates it (background).
    pub(super) fn load_source_parts(&mut self, tab: TabId, cx: &mut Context<Self>) {
        let Some(key) = self.source.as_ref().filter(|s| !s.parts.contains_key(&tab)).and_then(|s| s.studio_key(tab)) else { return };
        if let Some(s) = &mut self.source {
            s.parts.insert(tab, Vec::new());
        }
        cx.spawn(async move |this, cx| {
            let k2 = key.clone();
            let parts: Vec<(BodyId, String)> = cx
                .background_spawn(async move { external_context(&k2).map(|c| c.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default() })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                if let Some(s) = &mut this.source
                    && s.path == key.path
                    && s.chosen().is_some_and(|v| v.microversion == key.microversion)
                {
                    s.parts.insert(tab, parts);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Open a document of the Other documents list (its newest version).
    fn pick_document(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match Source::load(path.clone(), false, Some(0)) {
            Ok(s) => {
                self.source = Some(s);
                self.expanded.clear();
            }
            Err(e) => self.error = Some(format!("Could not open {}: {e:#}", path.display())),
        }
        cx.notify();
    }

    /// Current document › Version graph: this document's versions.
    pub(super) fn toggle_this_graph(&mut self, cx: &mut Context<Self>) {
        match &mut self.source {
            Some(s) if s.this_doc => s.graph = !s.graph,
            _ => {
                let path = self.doc.read(cx).session.path.clone();
                if let Ok(mut s) = Source::load(path, true, None) {
                    s.graph = true;
                    self.source = Some(s);
                }
            }
        }
        cx.notify();
    }

    fn ensure_docs(&mut self, cx: &mut Context<Self>) {
        if self.browse.docs.is_some() {
            return;
        }
        self.browse.docs = Some(Vec::new());
        cx.spawn(async move |this, cx| {
            let docs = cx.background_spawn(async { crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.browse.docs = Some(docs);
                cx.notify();
            });
        })
        .detach();
    }

    /// The title row of the source document: name and ⑂ workspace or
    /// version, "Create version…" and "Version graph" (bundle template).
    pub(super) fn render_title_row(&mut self, this_name: String, branch: String, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (name, sub, graph) = match &self.source {
            Some(s) => (s.name.clone(), s.chosen().map(|v| v.label.clone()).unwrap_or(branch), s.graph),
            None => (this_name, branch, false),
        };
        let square = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &App| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(gpui_kit::rgb(0xaaaaaa))
                .cursor_pointer()
                .when(on, |d| d.bg(gpui_kit::rgb(0xeaeaea)))
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::assets::icon(icon, 20., cx))
                .tooltip(crate::ui::tooltip(tip))
        };
        let other = self.other;
        h_flex()
            .px(px(6.))
            .py(px(4.))
            .gap(px(4.))
            .border_b_1()
            .border_color(gpui_kit::rgb(0xdddddd))
            .when(other, |d| {
                d.child(
                    div()
                        .id("add-other-back")
                        .size(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .tooltip(crate::ui::tooltip("Go to document filters"))
                        .child(crate::ui::glyph("chevron-left", 16., pal.text))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.source = None;
                            this.expanded.clear();
                            cx.notify();
                        })),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(name))
                    .child(h_flex().gap(px(3.)).text_size(px(13.)).text_color(pal.text_muted).child(crate::assets::icon("version-graph", 12., cx)).child(sub)),
            )
            .child(square("add-create-version", "version", "Create version…", false, cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.create_version(window, cx))))
            .child(square("add-version-graph", "version-graph", "Version graph", graph, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if this.other {
                    if let Some(s) = &mut this.source {
                        s.graph = !s.graph;
                    }
                    cx.notify();
                } else {
                    this.toggle_this_graph(cx);
                }
            })))
            .into_any_element()
    }

    /// Another document's unversioned work (live assembly/612): a light-blue
    /// box "N changes since the last version." with the underlined link
    /// "Create a version in <document>.".
    pub(super) fn render_changes(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.source.as_ref().filter(|s| !s.this_doc && s.changes > 0)?;
        let (n, name, path) = (s.changes, s.name.clone(), s.path.clone());
        Some(
            v_flex()
                .mx(px(4.))
                .mb(px(4.))
                .p(px(4.))
                .rounded(px(5.25))
                .border_1()
                .border_color(gpui_kit::rgb(0xb2ddf6))
                .bg(gpui_kit::rgb(0xdef1fb))
                .text_size(px(14.))
                .text_color(gpui_kit::rgb(0x094174))
                .child(div().px(px(1.)).child(format!("{n} change{} since the last version.", if n == 1 { "" } else { "s" })))
                .child(
                    div()
                        .id("add-other-create-link")
                        .pt(px(4.))
                        .pb(px(2.))
                        .flex()
                        .justify_center()
                        .font_weight(FontWeight::SEMIBOLD)
                        .underline()
                        .cursor_pointer()
                        .child(format!("Create a version in {name}."))
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            if let Some(gpu) = cx.try_global::<crate::gpu::GpuGlobal>().map(|g| g.0.clone()) {
                                let _ = crate::files::open_document_window(path.clone(), None, gpu, cx);
                            }
                        })),
                )
                .into_any_element(),
        )
    }

    /// "Create version…": the releasing document's Create version dialog;
    /// another document opens in its window.
    fn create_version(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.source.as_ref().filter(|s| !s.this_doc).map(|s| s.path.clone()) {
            Some(path) => {
                if let Some(gpu) = cx.try_global::<crate::gpu::GpuGlobal>().map(|g| g.0.clone()) {
                    let _ = crate::files::open_document_window(path, None, gpu, cx);
                }
            }
            // Called directly: an action dispatched from the dialog layer
            // doesn't reach the Workspace.
            None => crate::history_panel::open_create_version(self.doc.clone(), window, cx),
        }
    }

    /// The Version graph: ○ Workspace ◆ Version, "Main" (the workspace; only
    /// this document's can be picked) and the versions, newest first.
    pub(super) fn render_graph(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.source.as_ref().filter(|s| s.graph)?;
        let pal = p();
        let blue = gpui_kit::rgb(0x1651b0);
        let node = |hollow: bool, last: bool| {
            div()
                .w(px(70.))
                .h_full()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .child(div().absolute().left(px(34.5)).w(px(1.5)).top(px(if hollow { 18. } else { 0. })).bottom(px(if last { 18. } else { 0. })).bg(blue))
                .child(div().size(px(if hollow { 13. } else { 9. })).rounded_full().border_1().border_color(blue).bg(if hollow { gpui_kit::rgb(0xffffff) } else { blue }))
        };
        let mut col = v_flex().child(
            h_flex()
                .h(px(36.))
                .pl(px(6.))
                .gap(px(4.))
                .text_size(px(11.))
                .text_color(pal.text)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .child(div().size(px(9.)).rounded_full().border_1().border_color(pal.text))
                .child("Workspace")
                .child(div().ml(px(4.)).size(px(7.)).rounded_full().bg(pal.text))
                .child("Version"),
        );
        let this_doc = s.this_doc;
        let on_main = s.version.is_none();
        col = col.child(
            h_flex()
                .id("add-graph-main")
                .h(px(36.))
                .when(this_doc, |d| d.cursor_pointer().hover(|d| d.bg(pal.panel_hover)))
                .when(this_doc && on_main, |d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                .child(node(true, s.versions.is_empty()))
                .child(div().flex_1().text_size(px(13.33)).italic().text_color(gpui_kit::rgb(0x999999)).child("Main"))
                .when(this_doc, |d| {
                    d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        // The workspace is the plain Current document list.
                        this.source = None;
                        this.expanded.clear();
                        cx.notify();
                    }))
                }),
        );
        let n = s.versions.len();
        for (i, v) in s.versions.iter().enumerate() {
            let on = s.version == Some(i);
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("add-graph-v-{i}")))
                    .h(px(36.))
                    .cursor_pointer()
                    .when(on, |r| r.bg(gpui_kit::rgb(0xb2ddf6)))
                    .when(!on, |r| r.hover(|r| r.bg(pal.panel_hover)))
                    .child(node(false, i + 1 == n))
                    .child(div().flex_1().text_size(px(13.33)).text_color(pal.text).child(v.label.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(s) = &mut this.source {
                            s.choose(Some(i));
                        }
                        this.expanded.clear();
                        cx.notify();
                    })),
            );
        }
        Some(col.into_any_element())
    }

    /// Other documents before a document is opened: the search row, the
    /// filters, a filter's (or the search's) documents.
    pub(super) fn render_browser(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(0x1651b0);
        let searching = self.browse.query.is_some();
        let has_text = !self.browse.url.read(cx).value().trim().is_empty();
        let mut col = v_flex().child(
            h_flex()
                .h(px(39.))
                .pt(px(6.))
                .px(px(6.))
                .child(div().w(px(32.)).h(px(30.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph("filter", 20., if searching { blue } else { gpui_kit::rgb(0x666666) })))
                .child(gpui_kit::base::input::input_frame(div().ml(px(6.)).flex_1().h(px(30.)).rounded_l(px(3.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&self.browse.url).appearance(false).cleanable(true).small())))
                .child(
                    div()
                        .id("add-other-search")
                        .w(px(34.))
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_r(px(3.))
                        .border_1()
                        .border_l_0()
                        .border_color(if has_text { gpui_kit::rgb(0x90cef1) } else { gpui_kit::rgb(0xeaeaea) })
                        .cursor_pointer()
                        .child(crate::ui::glyph("search", 20., if has_text { blue } else { gpui_kit::rgb(0xcccccc) }))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.search_documents(cx))),
                ),
        );
        if let Some(e) = &self.error {
            col = col.child(div().px(px(10.)).py(px(4.)).text_size(px(12.)).text_color(gpui_kit::rgb(0xbd3039)).child(e.clone()));
        }
        let Some((filter, query)) = self.browse.query.clone().map(|q| (None, Some(q))).or_else(|| self.browse.filter.map(|c| (Some(c), None))) else {
            for (i, (label, icon)) in FILTERS.iter().enumerate() {
                col = col.child(
                    h_flex()
                        .id(SharedString::from(format!("add-other-cat-{i}")))
                        .h(px(32.))
                        .pl(px(16.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_size(px(14.67))
                        .text_color(pal.text)
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon(icon, 20., cx))
                        .child(label.to_string())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ensure_docs(cx);
                            this.browse.filter = Some(i);
                            cx.notify();
                        })),
                );
            }
            return col.into_any_element();
        };
        col = col.child(
            h_flex()
                .id("add-other-filter-back")
                .h(px(36.))
                .pl(px(6.))
                .gap(px(4.))
                .cursor_pointer()
                .border_b_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .child(crate::ui::glyph("chevron-left", 14., pal.text))
                .child(div().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(match (filter, &query) {
                    (Some(c), _) => FILTERS[c].0.to_string(),
                    (_, Some(q)) => format!("Results for \"{q}\""),
                    _ => String::new(),
                }))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.browse.filter = None;
                    this.browse.query = None;
                    cx.notify();
                })),
        );
        let me = self.doc.read(cx).session.path.clone();
        let mut docs: Vec<crate::start_library::DocEntry> = self.browse.docs.clone().unwrap_or_default().into_iter().filter(|d| !d.trashed && !d.broken && d.path != me).collect();
        let q = query.as_ref().map(|q| q.to_lowercase());
        match (filter, &q) {
            // Local documents are all the user's own; nothing is shared or public.
            (Some(3 | 4), _) => docs.clear(),
            (Some(1), _) => {
                docs.retain(|d| d.recent_rank.is_some());
                docs.sort_by_key(|d| d.recent_rank);
            }
            (_, Some(q)) => docs.retain(|d| d.name.to_lowercase().contains(q.as_str())),
            _ => docs.sort_by_key(|d| std::cmp::Reverse(d.modified)),
        }
        if docs.is_empty() && self.browse.docs.as_ref().is_some_and(|d| !d.is_empty()) {
            col = col.child(div().pl(px(10.)).h(px(28.)).flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("No documents found"));
        }
        for (i, d) in docs.into_iter().enumerate() {
            let path = d.path.clone();
            let thumb = match d.thumbnail.clone() {
                Some(im) => gpui_kit::img(im).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                None => crate::assets::icon("document", 32., cx).into_any_element(),
            };
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("add-other-doc-{i}")))
                    .h(px(51.))
                    .pl(px(2.))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .tooltip(crate::ui::tooltip(d.name.clone()))
                    .child(div().w(px(74.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(thumb))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(d.name.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_document(path.clone(), cx))),
            );
        }
        col.into_any_element()
    }

    /// "Search or paste URL": a pasted document path opens it, other text
    /// lists the documents whose name contains it.
    fn search_documents(&mut self, cx: &mut Context<Self>) {
        let text = self.browse.url.read(cx).value().trim().to_string();
        let path = PathBuf::from(text.trim_start_matches("file://"));
        if path.extension().is_some_and(|e| e == crate::start_library::EXT) && path.exists() {
            self.pick_document(path, cx);
            return;
        }
        self.ensure_docs(cx);
        self.browse.query = (!text.is_empty()).then_some(text);
        cx.notify();
    }
}
