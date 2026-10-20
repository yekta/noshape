//! FeatureScript reference parameters (`TableData`, `ImageData`,
//! `JSONData`, `TextData`, `PartStudioData`…) in the feature dialog: the
//! field (label above a 214×27 box with the type icon and "Select CSV
//! file…" or the chosen name) and the 300×559 "Select …" panel that opens
//! beside the dialog (live dom/fs-refparams*.txt, part-studio/481–486).

use crate::feature_dialog::{DialogEvent, FeatureDialog};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::{Document, TabContent};
use noshape_model::file_tab::FileKind;
use noshape_model::params::{ParamValue, ReferenceKind};
use std::collections::HashSet;

const BLUE: u32 = 0x1651b0;

/// A tab a reference parameter can point at.
#[derive(Clone, Debug)]
pub struct RefSource {
    pub tab: String,
    pub name: String,
    pub what: RefSourceKind,
}

#[derive(Clone, Debug)]
pub enum RefSourceKind {
    File(FileKind, String),
    PartStudio(Vec<(noshape_model::BodyId, String)>),
    VariableStudio,
}

/// A document whose tabs a reference can point at, at one of its versions.
struct VersionedDoc {
    path: std::path::PathBuf,
    name: String,
    /// Named versions, newest first, then "Start" (the first state).
    versions: Vec<crate::derived::VersionChoice>,
    /// The chosen version (index into `versions`); `None` is Main, the
    /// workspace (only the current document offers it).
    version: Option<usize>,
    /// Changes on Main since the chosen version.
    changes: usize,
    /// The matching tabs at the chosen version (`ext:` keys), once loaded.
    sources: Option<Vec<RefSource>>,
}

/// The open "Select …" panel.
pub struct RefPicker {
    pub path: String,
    pub kind: ReferenceKind,
    other_documents: bool,
    search: Entity<InputState>,
    _search_sub: Subscription,
    /// Part Studios whose parts are listed.
    expanded: HashSet<String>,
    /// The version graph replaces the list.
    graph: bool,
    /// The current document's versions (`version: None` = Main).
    current: Option<VersionedDoc>,
    /// Other documents: "Search or paste URL", the chosen filter or search,
    /// the documents found, and the document opened.
    url: Entity<InputState>,
    _url_sub: Subscription,
    category: Option<usize>,
    doc_query: Option<String>,
    docs: Option<Vec<crate::start_library::DocEntry>>,
    /// Latest version label and whether it has matching tabs, per document.
    doc_info: std::collections::HashMap<std::path::PathBuf, (String, bool)>,
    other: Option<VersionedDoc>,
}

/// Document filters of the Other documents tab (live 602).
const FILTERS: [(&str, &str); 6] = [("Recent", "recent"), ("Owned by me", "src-owned"), ("Recently opened", "src-recent"), ("Created by me", "src-created"), ("Shared with me", "src-shared"), ("Public", "src-public")];

/// A document's versions (named ones newest first, then "Start") and name.
pub(crate) fn document_versions(path: &std::path::Path) -> Option<(String, Vec<crate::derived::VersionChoice>)> {
    let store = noshape_store::Document::open_read_only(path).ok()?;
    let graph = store.graph().ok()?;
    let mut versions: Vec<crate::derived::VersionChoice> = graph.versions.iter().rev().map(|v| crate::derived::VersionChoice { label: v.name.clone(), microversion: v.microversion }).collect();
    if let Some(root) = graph.microversions.iter().find(|m| m.parents.is_empty()) {
        versions.push(crate::derived::VersionChoice { label: "Start".into(), microversion: root.id });
    }
    let name = store.meta().map(|m| m.name).unwrap_or_else(|_| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    Some((name, versions))
}

/// Changes on Main since microversion `mv`.
pub(crate) fn changes_since(path: &std::path::Path, mv: noshape_store::MicroversionId) -> usize {
    let Ok(store) = noshape_store::Document::open_read_only(path) else { return 0 };
    let Ok(history) = store.default_branch().and_then(|b| store.history(b)) else { return 0 };
    history.iter().take_while(|m| m.id != mv).count()
}

/// The tabs of a document at a version, keyed by `ext:` references.
fn version_sources(path: &std::path::Path, mv: noshape_store::MicroversionId, except: &str) -> Vec<RefSource> {
    let Ok(doc) = crate::derived::external_snapshot(path, mv) else { return Vec::new() };
    let key = |tab: &str| crate::derived::ExternalRef { path: path.to_path_buf(), microversion: mv, tab: tab.to_string() };
    sources(&doc, except, |tab| crate::derived::external_context(&key(tab)).map(|c| c.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default())
        .into_iter()
        .map(|s| RefSource { tab: key(&s.tab).key(), ..s })
        .collect()
}

/// "No CSV files available." (live 604) for each kind.
fn none_text(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::Csv => "No CSV files available.",
        ReferenceKind::Image => "No image files available.",
        ReferenceKind::Json => "No JSON files available.",
        ReferenceKind::Text => "No text files available.",
        ReferenceKind::PartStudio => "No Part Studios available.",
        ReferenceKind::VariableStudio => "No Variable Studios available.",
        ReferenceKind::CadImport => "No CAD files available.",
    }
}

/// A stored `ext:` value's part-less key (`ext:mv|tab#part|path` → `ext:mv|tab|path`).
fn ext_key(value: &str) -> Option<crate::derived::ExternalRef> {
    let rest = value.strip_prefix("ext:")?;
    let mut it = rest.splitn(3, '|');
    let (mv, tab, path) = (it.next()?, it.next()?, it.next()?);
    crate::derived::ExternalRef::parse(&format!("ext:{mv}|{}|{path}", source_tab(tab)))
}

/// The name a stored value shows: a tab of this document, or of another
/// document or version (the field shows only the file name, live 610, 628).
fn value_name(sources: &[RefSource], value: &str) -> Option<String> {
    if let Some(r) = ext_key(value) {
        let doc = crate::derived::external_snapshot(&r.path, r.microversion).ok()?;
        return doc.tabs.iter().find(|t| t.id.to_string() == r.tab).map(|t| t.name.clone());
    }
    sources.iter().find(|s| s.tab == source_tab(value)).map(|s| s.name.clone())
}

/// The file tabs and studios of `doc` (Part Studios without `except`,
/// the one being edited), sorted by name like Onshape's lists.
pub fn sources(doc: &Document, except: &str, parts: impl Fn(&str) -> Vec<(noshape_model::BodyId, String)>) -> Vec<RefSource> {
    let mut out: Vec<RefSource> = doc
        .tabs
        .iter()
        .filter(|t| t.id.to_string() != except)
        .filter_map(|t| {
            let what = match &t.content {
                TabContent::File(f) => RefSourceKind::File(f.kind(), f.ext()),
                TabContent::PartStudio(_) => RefSourceKind::PartStudio(parts(&t.id.to_string())),
                TabContent::VariableStudio(_) => RefSourceKind::VariableStudio,
                _ => return None,
            };
            Some(RefSource { tab: t.id.to_string(), name: t.name.clone(), what })
        })
        .collect();
    out.sort_by_key(|s| s.name.to_lowercase());
    out
}

fn accepts(kind: ReferenceKind, s: &RefSource) -> bool {
    match (&s.what, kind) {
        (RefSourceKind::File(_, ext), ReferenceKind::Csv) => ext == "csv",
        (RefSourceKind::File(k, _), ReferenceKind::Image) => *k == FileKind::Image,
        (RefSourceKind::File(_, ext), ReferenceKind::Json) => ext == "json",
        (RefSourceKind::File(_, ext), ReferenceKind::Text) => ext == "txt",
        (RefSourceKind::File(k, _), ReferenceKind::CadImport) => *k == FileKind::Cad,
        (RefSourceKind::PartStudio(_), ReferenceKind::PartStudio) => true,
        (RefSourceKind::VariableStudio, ReferenceKind::VariableStudio) => true,
        _ => false,
    }
}

/// Field icon, placeholder, panel title and search placeholder (live).
fn texts(kind: ReferenceKind) -> (&'static str, &'static str, &'static str, &'static str) {
    match kind {
        ReferenceKind::Csv => ("file", "Select CSV file…", "Select CSV file", "Search CSV files"),
        ReferenceKind::Image => ("image", "Select image…", "Select image", "Search image files"),
        ReferenceKind::Json => ("file", "Select JSON file…", "Select JSON file", "Search JSON files"),
        ReferenceKind::Text => ("file", "Select text file…", "Select text file", "Search text files"),
        ReferenceKind::PartStudio => ("part-studio", "Select Part Studio…", "Select Part Studio", "Search entities or features"),
        ReferenceKind::VariableStudio => ("variable-studio", "Select Variable Studio…", "Select Variable Studio", "Search Variable Studios"),
        ReferenceKind::CadImport => ("file-cad", "Select CAD file…", "Select CAD file", "Search CAD files"),
    }
}

/// The stored value: the tab, or `tab#part` for one part of a Part Studio.
fn source_tab(value: &str) -> &str {
    value.split_once('#').map_or(value, |(t, _)| t)
}

/// The value of one part of Part Studio `key`: `tab#part`, or for another
/// document / version `ext:mv|tab#part|path`.
fn part_value(key: &str, body: &noshape_model::BodyId) -> String {
    match crate::derived::ExternalRef::parse(key) {
        Some(r) => format!("ext:{}|{}#{body}|{}", r.microversion, r.tab, r.path.display()),
        None => format!("{key}#{body}"),
    }
}

impl FeatureDialog {
    /// The parameter row: label, then the bordered field (blue while its
    /// panel is open).
    pub(crate) fn render_reference_field(&mut self, label: &'static str, path: &str, kind: ReferenceKind, value: &str, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (icon, placeholder, _, _) = texts(kind);
        let open = self.ref_picker.as_ref().is_some_and(|r| r.path == path);
        let chosen = value_name(&self.ref_sources, value);
        let empty = chosen.is_none();
        // Live 611: "Table: fsrefs.csv".
        let tip = chosen.as_ref().map(|c| format!("{label}: {c}"));
        let path_s = path.to_string();
        v_flex()
            .mx(px(1.))
            .mt(px(5.))
            .child(div().h(px(22.)).px(px(7.)).text_size(px(12.)).line_height(px(21.6)).text_color(gpui_kit::rgb(0x666666)).child(label))
            .child(
                h_flex()
                    .id(SharedString::from(format!("ref-{path}")))
                    .h(px(27.))
                    .p(px(2.))
                    .gap(px(4.))
                    .rounded(px(2.))
                    .border_1()
                    .border_color(gpui_kit::rgb(if open { BLUE } else { 0xd7d7d7 }))
                    .cursor_pointer()
                    .when_some(tip, |d, t| d.tooltip(crate::ui::tooltip(t)))
                    .child(div().when(empty, |d| d.opacity(0.6)).child(crate::assets::icon(icon, 20., cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(14.))
                            .line_height(px(21.))
                            .text_color(pal.text)
                            .when(empty, |d| d.opacity(0.6))
                            .when(open, |d| d.font_weight(FontWeight::MEDIUM))
                            .child(chosen.unwrap_or_else(|| placeholder.to_string())),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if this.ref_picker.as_ref().is_some_and(|r| r.path == path_s) {
                            this.ref_picker = None;
                        } else {
                            this.open_reference_picker(path_s.clone(), kind, window, cx);
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn open_reference_picker(&mut self, path: String, kind: ReferenceKind, window: &mut Window, cx: &mut Context<Self>) {
        let notify = |_: &mut Self, _, ev: &InputEvent, cx: &mut Context<Self>| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        };
        let search = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe(&search, notify);
        let url = cx.new(|cx| InputState::new(window, cx));
        let url_sub = cx.subscribe(&url, |this: &mut Self, st, ev: &InputEvent, cx| match ev {
            InputEvent::PressEnter { .. } => {
                let text = st.read(cx).value().trim().to_string();
                this.search_documents(text, cx);
            }
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        let current = self.ref_document_path.as_deref().and_then(document_versions).map(|(name, versions)| VersionedDoc {
            path: self.ref_document_path.clone().unwrap_or_default(),
            name,
            versions,
            version: None,
            changes: 0,
            sources: None,
        });
        self.ref_picker = Some(RefPicker {
            path,
            kind,
            other_documents: false,
            search,
            _search_sub: sub,
            expanded: HashSet::new(),
            graph: false,
            current,
            url,
            _url_sub: url_sub,
            category: None,
            doc_query: None,
            docs: None,
            doc_info: Default::default(),
            other: None,
        });
    }

    /// Load the matching tabs of the shown document at its chosen version.
    fn load_version_sources(&mut self, other: bool, cx: &mut Context<Self>) {
        let me = self.ref_document_path.clone();
        let except_tab = self.ref_except.clone();
        let Some(p) = self.ref_picker.as_mut() else { return };
        let doc = if other { p.other.as_mut() } else { p.current.as_mut() };
        let Some(d) = doc else { return };
        let Some(v) = d.version.and_then(|i| d.versions.get(i)).cloned() else { return };
        d.sources = None;
        d.changes = 0;
        let path = d.path.clone();
        let path2 = path.clone();
        let except = if Some(&path) == me.as_ref() { except_tab } else { String::new() };
        cx.spawn(async move |this, cx| {
            let (list, changes) = cx.background_spawn(async move { (version_sources(&path, v.microversion, &except), changes_since(&path, v.microversion)) }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = this.ref_picker.as_mut()
                    && let Some(d) = if other { p.other.as_mut() } else { p.current.as_mut() }
                    && d.version.and_then(|i| d.versions.get(i)).is_some_and(|x| x.microversion == v.microversion)
                {
                    d.sources = Some(list);
                    d.changes = changes;
                }
                this.render_version_pictures(path2, v.microversion, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Pictures of the Part Studios of a document at a version (keyed by
    /// their `ext:` values), for Part Studio references.
    fn render_version_pictures(&mut self, path: std::path::PathBuf, mv: noshape_store::MicroversionId, cx: &mut Context<Self>) {
        let wants = self.ref_picker.as_ref().is_some_and(|p| p.kind == ReferenceKind::PartStudio);
        let Some(gpu) = self.ref_gpu.clone().filter(|_| wants) else { return };
        let Ok(doc) = crate::derived::external_snapshot(&path, mv) else { return };
        let studios: Vec<noshape_model::document::TabId> = doc.tabs.iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| t.id).collect();
        let doc = (*doc).clone();
        cx.spawn(async move |this, cx| {
            let key = move |t: noshape_model::document::TabId| crate::derived::ExternalRef { path: path.clone(), microversion: mv, tab: t.to_string() }.key();
            let pictures = cx.background_spawn(render_thumbnails(gpu, doc, studios, key)).await;
            let _ = this.update(cx, |this, cx| {
                this.ref_thumbs.extend(pictures);
                cx.notify();
            });
        })
        .detach();
    }

    /// The documents of this computer (Other documents), and per document
    /// its latest version and whether it has matching tabs (others grey).
    fn ensure_documents(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.ref_picker.as_mut() else { return };
        if p.docs.is_some() {
            return;
        }
        p.docs = Some(Vec::new());
        let kind = p.kind;
        cx.spawn(async move |this, cx| {
            let (docs, info) = cx
                .background_spawn(async move {
                    let docs = crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs;
                    let info: std::collections::HashMap<std::path::PathBuf, (String, bool)> = docs
                        .iter()
                        .filter_map(|d| {
                            let (_, versions) = document_versions(&d.path)?;
                            let latest = versions.first()?;
                            let any = version_sources(&d.path, latest.microversion, "").iter().any(|s| accepts(kind, s));
                            Some((d.path.clone(), (latest.label.clone(), any)))
                        })
                        .collect();
                    (docs, info)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = this.ref_picker.as_mut() {
                    p.docs = Some(docs);
                    p.doc_info = info;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// "Search or paste URL": a document path opens it, other text lists
    /// the documents whose name contains it.
    fn search_documents(&mut self, text: String, cx: &mut Context<Self>) {
        let path = std::path::PathBuf::from(text.trim_start_matches("file://"));
        if path.extension().is_some_and(|e| e == crate::start_library::EXT) && path.exists() {
            self.open_other_document(path, cx);
            return;
        }
        self.ensure_documents(cx);
        if let Some(p) = self.ref_picker.as_mut() {
            p.other = None;
            p.doc_query = (!text.is_empty()).then_some(text);
        }
        cx.notify();
    }

    /// Open another document at its latest version (live 604: never its workspace).
    fn open_other_document(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let Some((name, versions)) = document_versions(&path) else { return };
        if let Some(p) = self.ref_picker.as_mut() {
            p.other = Some(VersionedDoc { path, name, version: (!versions.is_empty()).then_some(0), versions, changes: 0, sources: None });
            p.graph = false;
        }
        self.load_version_sources(true, cx);
        cx.notify();
    }

    /// Picking a file closes the panel (live 483, 610); a Part Studio's
    /// panel stays open to pick its parts (486).
    fn pick_reference(&mut self, value: String, cx: &mut Context<Self>) {
        let Some((path, kind)) = self.ref_picker.as_ref().map(|r| (r.path.clone(), r.kind)) else { return };
        if !matches!(kind, ReferenceKind::PartStudio | ReferenceKind::VariableStudio) {
            self.ref_picker = None;
        }
        self.set_value(&path, ParamValue::Text(value), cx);
    }

    /// The rows of `list` matching the search (Part Studios expand to
    /// their parts; the picked one light blue).
    fn source_rows(&self, list: &[RefSource], kind: ReferenceKind, query: &str, expanded: &HashSet<String>, current: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let thumbs = self.ref_thumbs.clone();
        let row = |id: SharedString, indent: f32, expander: Option<AnyElement>, key: &str, icon: &'static str, name: String, on: bool, cx: &App| {
            // A picture of the Part Studio or part when rendered, else its icon.
            let picture = match thumbs.get(key) {
                Some(img) => gpui_kit::img(img.clone()).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                None => crate::assets::icon(icon, 30., cx),
            };
            h_flex()
                .id(id)
                .h(px(40.))
                .flex_none()
                .pl(px(indent))
                .when(on, |d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                .when(!on, |d| d.hover(|d| d.bg(p().panel_hover)))
                .cursor_pointer()
                .children(expander)
                .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(picture))
                .child(div().flex_1().min_w_0().pl(px(4.)).truncate().text_size(px(13.33)).font_weight(FontWeight::LIGHT).text_color(p().text).child(name.clone()))
                .tooltip(crate::ui::tooltip(name))
        };
        let mut out = Vec::new();
        for s in list.iter().filter(|s| accepts(kind, s)) {
            let parts = match &s.what {
                RefSourceKind::PartStudio(parts) => Some(parts.clone()),
                _ => None,
            };
            let name_hit = query.is_empty() || s.name.to_lowercase().contains(query);
            let part_hits: Vec<(noshape_model::BodyId, String)> = parts.iter().flatten().filter(|(_, n)| name_hit || n.to_lowercase().contains(query)).cloned().collect();
            if !name_hit && part_hits.is_empty() {
                continue;
            }
            let icon = match &s.what {
                RefSourceKind::File(FileKind::Image, _) => "image",
                RefSourceKind::File(FileKind::Cad, _) => "file-cad",
                RefSourceKind::File(..) => "file",
                RefSourceKind::PartStudio(_) => "part-studio",
                RefSourceKind::VariableStudio => "variable-studio",
            };
            let key = s.tab.clone();
            let open = parts.is_some() && (expanded.contains(&s.tab) || !query.is_empty());
            // The expander toggles the parts; the rest of the row picks.
            let expander = parts.as_ref().map(|_| {
                let t = s.tab.clone();
                div()
                    .id(SharedString::from(format!("ref-exp-{}", s.tab)))
                    .w(px(18.))
                    .h_full()
                    .flex()
                    .flex_none()
                    .items_center()
                    .pl(px(4.))
                    .child(crate::ui::glyph(if open { "expander-down" } else { "expander-right" }, 10., pal.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        if let Some(p) = &mut this.ref_picker
                            && !p.expanded.remove(&t)
                        {
                            p.expanded.insert(t.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            });
            let r = row(SharedString::from(format!("ref-src-{}", s.tab)), 0., expander, &s.tab, icon, s.name.clone(), current == s.tab, cx);
            out.push(r.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_reference(key.clone(), cx))).into_any_element());
            if open {
                for (body, name) in part_hits {
                    let value = part_value(&s.tab, &body);
                    let on = current == value;
                    out.push(row(SharedString::from(format!("ref-part-{value}")), 36., None, &value, "part", name, on, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_reference(value.clone(), cx))).into_any_element());
                }
            }
        }
        out
    }

    /// The 300×559 "Select …" panel right of the dialog, or nothing.
    pub(crate) fn render_reference_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let picker = self.ref_picker.as_ref()?;
        let (path, kind, other, graph) = (picker.path.clone(), picker.kind, picker.other_documents, picker.graph);
        let search = picker.search.clone();
        let expanded = picker.expanded.clone();
        let typed = !search.read(cx).value().is_empty();
        let query = search.read(cx).value().trim().to_lowercase();
        let current = self.values_text(&path);
        let (_, _, title, search_placeholder) = texts(kind);
        let import = matches!(kind, ReferenceKind::Csv | ReferenceKind::Image);
        let blue = gpui_kit::rgb(BLUE);
        let line = gpui_kit::rgb(0xeeeeee);
        let tab = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(150.))
                .h_full()
                .p(px(5.))
                .flex()
                .justify_center()
                .text_size(px(12.))
                .line_height(px(21.6))
                .cursor_pointer()
                .when(on, |d| d.text_color(blue).font_weight(FontWeight::MEDIUM).border_b_2().border_color(blue))
                .when(!on, |d| d.text_color(p().text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let to_other = id == "ref-other";
                    let Some(r) = &mut this.ref_picker else { return };
                    if r.other_documents == to_other {
                        return;
                    }
                    r.other_documents = to_other;
                    r.graph = false;
                    // Live: switching to Other documents clears the field
                    // until something is picked (✕ on the dialog restores it).
                    if to_other {
                        let path = r.path.clone();
                        this.set_value(&path, ParamValue::Text(String::new()), cx);
                    }
                    cx.notify();
                }))
        };
        let mut body = v_flex().flex_1().min_h_0();
        let shown = if other { picker.other.as_ref() } else { picker.current.as_ref() };
        if other && shown.is_none() {
            body = body.child(self.render_document_browser(cx));
        } else {
            let (doc_name, doc_path, version_label) = match shown {
                Some(d) => (d.name.clone(), Some(d.path.clone()), d.version.and_then(|i| d.versions.get(i)).map_or_else(|| "Main".to_string(), |v| v.label.clone())),
                None => (self.ref_document_name.clone(), None, "Main".to_string()),
            };
            body = body.child(self.document_header(doc_name.clone(), version_label, other, graph, doc_path.clone(), cx));
            if graph {
                body = body.child(self.render_version_graph(other, cx));
            } else {
                // The placeholder is italic grey (live 482), drawn over the field.
                body = body.child(
                    h_flex().h(px(42.)).flex_none().pt(px(6.)).px(px(6.)).child(
                        div()
                            .flex_1()
                            .relative()
                            .h(px(30.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xcccccc))
                            .child(Input::new(&search).appearance(false).cleanable(true).text_size(px(12.)).h(px(28.)))
                            .when(!typed, |d| d.child(div().absolute().left(px(6.)).top_0().bottom_0().flex().items_center().text_size(px(12.)).italic().text_color(pal.text_faint).child(search_placeholder))),
                    ),
                );
                // At a version with later changes: the blue box (live 604, 627b).
                let (list, changes, loading) = match shown {
                    Some(d) if d.version.is_some() => (d.sources.clone().unwrap_or_default(), d.changes, d.sources.is_none()),
                    _ => (self.ref_sources.clone(), 0, false),
                };
                let mut rows = v_flex().id("ref-list").flex_1().min_h_0().pt(px(1.)).overflow_y_scroll();
                if changes > 0 {
                    let link = format!("Create a version in {doc_name}.");
                    let target = if other { doc_path.clone() } else { None };
                    rows = rows.child(
                        v_flex()
                            .m(px(4.))
                            .p(px(4.))
                            .pl(px(5.))
                            .rounded(px(5.25))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xb2ddf6))
                            .bg(gpui_kit::rgb(0xdef1fb))
                            .text_size(px(14.))
                            .line_height(px(21.6))
                            .text_color(gpui_kit::rgb(0x094174))
                            .child(format!("{changes} change{} since the last version.", if changes == 1 { "" } else { "s" }))
                            .child(
                                div()
                                    .id("ref-create-version-link")
                                    .pt(px(4.))
                                    .pb(px(4.))
                                    .flex()
                                    .justify_center()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .underline()
                                    .cursor_pointer()
                                    .child(link)
                                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(DialogEvent::CreateVersion(target.clone())))),
                            ),
                    );
                }
                let items = self.source_rows(&list, kind, &query, &expanded, &current, cx);
                if items.is_empty() && !loading {
                    rows = rows.child(div().p(px(10.)).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).line_height(px(28.)).text_color(pal.text).child(none_text(kind)));
                }
                body = body.child(rows.children(items));
            }
        }
        let panel = v_flex()
            .id("ref-panel")
            .occlude()
            .absolute()
            .top_0()
            .left_full()
            .ml(px(1.))
            .w(px(300.))
            .h(px(559.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(29.))
                    .flex_none()
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .child(div().flex_1().pl(px(6.)).text_size(px(13.)).line_height(px(28.)).font_weight(FontWeight::MEDIUM).child(title))
                    .child(
                        div()
                            .id("ref-close")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.5)
                            .cursor_pointer()
                            .child(Icon::new(IconName::Close).with_size(px(14.)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ref_picker = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(h_flex().h(px(32.)).flex_none().border_t_1().border_color(line).child(tab("ref-current", "Current document", !other, cx)).child(tab("ref-other", "Other documents", other, cx)))
            .child(body)
            .when(import, |d| {
                d.child(
                    h_flex()
                        .id("ref-import")
                        .h(px(23.))
                        .flex_none()
                        .p(px(3.))
                        .gap(px(2.))
                        .text_size(px(14.))
                        .text_color(gpui_kit::rgb(0x666666))
                        .cursor_pointer()
                        .child(crate::ui::glyph("import-file", 16., gpui_kit::rgb(0x666666)))
                        .child("Import…")
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.import_reference(kind, window, cx))),
                )
            });
        Some(panel.into_any_element())
    }

    /// The document row: (home ‹ for another document) name and "⑂ Main" /
    /// "⑂ V2", then Create version… and Version graph (live 482, 604).
    fn document_header(&self, name: String, version: String, other: bool, graph: bool, path: Option<std::path::PathBuf>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(BLUE);
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
                .hover(|d| d.bg(p().panel_hover))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(icon, 20., cx))
        };
        let nav = |id: &'static str, icon: &'static str, tip: String| {
            div().id(id).size(px(30.)).flex().flex_none().items_center().justify_center().rounded(px(3.)).cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xeaeaea))).tooltip(crate::ui::tooltip(tip)).child(crate::ui::glyph(icon, if icon == "ui-chevron-left" { 16. } else { 20. }, gpui_kit::rgb(0x333333)))
        };
        let back_tip = self.ref_picker.as_ref().and_then(|p| p.doc_query.clone().map(|q| format!("Results for \"{q}\"")).or_else(|| p.category.map(|c| FILTERS[c].0.to_string()))).unwrap_or_else(|| "Go to document filters".into());
        h_flex()
            .h(px(40.))
            .flex_none()
            .p(px(2.))
            .pl(px(if other { 2. } else { 6. }))
            .gap(px(6.))
            .border_t_1()
            .border_color(gpui_kit::rgb(0xeeeeee))
            .when(other, |d| {
                d.child(nav("ref-home", "home", "Go to document filters".into()).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(p) = this.ref_picker.as_mut() {
                        p.other = None;
                        p.category = None;
                        p.doc_query = None;
                        p.graph = false;
                    }
                    cx.notify();
                })))
                .child(nav("ref-back", "ui-chevron-left", back_tip).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(p) = this.ref_picker.as_mut() {
                        p.other = None;
                        p.graph = false;
                    }
                    cx.notify();
                })))
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).child(name))
                    .child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).line_height(px(15.6)).child(crate::ui::glyph("branch", 10., pal.text)).child(version)),
            )
            .child(square("ref-create-version", "version", "Create version…", false, cx).on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(DialogEvent::CreateVersion(if other { path.clone() } else { None })))))
            .child(square("ref-version-graph", "version-graph", "Version graph", graph, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                // Reopening the graph shows versions created meanwhile.
                if let Some(p) = this.ref_picker.as_mut() {
                    p.graph = !p.graph;
                    let d = if other { p.other.as_mut() } else { p.current.as_mut() };
                    if p.graph
                        && let Some(d) = d
                        && let Some((_, versions)) = document_versions(&d.path)
                    {
                        let chosen = d.version.and_then(|i| d.versions.get(i)).map(|v| v.microversion);
                        d.version = chosen.and_then(|mv| versions.iter().position(|v| v.microversion == mv));
                        d.versions = versions;
                    }
                }
                cx.notify();
            })))
            .into_any_element()
    }

    /// Legend "○ Workspace ● Version", then Main and the versions on a blue
    /// line, Start last with an end bar; the chosen one light blue. Main can
    /// be chosen only in the current document (live 608, 626).
    fn render_version_graph(&self, other: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(BLUE);
        let Some(d) = self.ref_picker.as_ref().and_then(|p| if other { p.other.as_ref() } else { p.current.as_ref() }) else { return div().into_any_element() };
        let n = d.versions.len();
        let node = |hollow: bool, first: bool, last: bool| {
            div()
                .w(px(140.))
                .h_full()
                .flex_none()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                // The line; the last (Start) ends in a short bar below its dot (live 608).
                .child(div().absolute().left(px(69.)).w(px(2.)).top(px(if first { 18. } else { 0. })).bottom(px(if last { 9. } else { 0. })).bg(blue))
                .when(last, |d| d.child(div().absolute().left(px(64.)).w(px(12.)).h(px(2.)).bottom(px(8.)).bg(blue)))
                .child(div().size(px(if hollow { 13. } else { 9. })).rounded_full().when(hollow, |d| d.border_2()).border_color(blue).bg(if hollow { gpui_kit::rgb(0xffffff) } else { blue }))
        };
        let mut col = v_flex()
            .id("ref-graph")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                h_flex()
                    .h(px(40.))
                    .flex_none()
                    .pl(px(6.))
                    .gap(px(4.))
                    .text_size(px(12.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xeeeeee))
                    // Legend icons (live 626): a ring on a stem, a dot on a line.
                    .child(v_flex().items_center().child(div().size(px(9.)).rounded_full().border_2().border_color(pal.text)).child(div().w(px(2.)).h(px(5.)).bg(pal.text)))
                    .child("Workspace")
                    .child(div().ml(px(6.)).w(px(8.)).h(px(14.)).relative().flex().items_center().justify_center().child(div().absolute().left(px(3.)).top_0().bottom_0().w(px(2.)).bg(pal.text)).child(div().size(px(7.)).rounded_full().bg(pal.text)))
                    .child("Version"),
            );
        let main_on = d.version.is_none();
        col = col.child(
            h_flex()
                .id("ref-v-main")
                .h(px(36.))
                .flex_none()
                .when(main_on && !other, |r| r.bg(gpui_kit::rgb(0xb2ddf6)))
                .when(!other, |r| r.cursor_pointer())
                .child(node(true, true, n == 0))
                .child(div().flex_1().text_size(px(14.)).when(other, |d| d.italic().text_color(gpui_kit::rgb(0x999999))).child("Main"))
                .tooltip(crate::ui::tooltip("Main"))
                .when(!other, |r| {
                    r.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(p) = this.ref_picker.as_mut()
                            && let Some(d) = p.current.as_mut()
                        {
                            d.version = None;
                            d.sources = None;
                            p.graph = false;
                        }
                        cx.notify();
                    }))
                }),
        );
        for (i, v) in d.versions.iter().enumerate() {
            let on = d.version == Some(i);
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("ref-v-{i}")))
                    .h(px(36.))
                    .flex_none()
                    .cursor_pointer()
                    .when(on, |r| r.bg(gpui_kit::rgb(0xb2ddf6)))
                    .when(!on, |r| r.hover(|r| r.bg(pal.panel_hover)))
                    .child(node(false, false, i + 1 == n))
                    .child(div().flex_1().text_size(px(14.)).child(v.label.clone()))
                    .tooltip(crate::ui::tooltip(v.label.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(p) = this.ref_picker.as_mut()
                            && let Some(d) = if other { p.other.as_mut() } else { p.current.as_mut() }
                        {
                            d.version = Some(i);
                            p.graph = false;
                        }
                        this.load_version_sources(other, cx);
                        cx.notify();
                    })),
            );
        }
        col.into_any_element()
    }

    /// Other documents before a document is opened: the filter and
    /// "Search or paste URL" row, then the filters (live 602) or a filter's /
    /// search's documents (603): folders first, documents with their
    /// picture, name and latest version; those without matching tabs grey.
    fn render_document_browser(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(BLUE);
        let Some(p) = self.ref_picker.as_ref() else { return div().into_any_element() };
        let (category, query, url) = (p.category, p.doc_query.clone(), p.url.clone());
        let url_empty = url.read(cx).value().is_empty();
        let placeholder = match (category, &query) {
            (Some(c), None) => format!("Search in \"{}\"", FILTERS[c].0),
            _ => "Search or paste URL".to_string(),
        };
        let mut col = v_flex().flex_1().min_h_0().child(
            h_flex()
                .h(px(39.))
                .flex_none()
                .pt(px(6.))
                .px(px(6.))
                .child(div().id("ref-doc-filters").w(px(32.)).h(px(30.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph("filter", 20., blue)).tooltip(crate::ui::tooltip("Show advanced search options")))
                .child(
                    div()
                        .ml(px(6.))
                        .flex_1()
                        .relative()
                        .h(px(30.))
                        .rounded_l(px(3.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xcccccc))
                        .child(Input::new(&url).appearance(false).cleanable(true).text_size(px(12.)).h(px(28.)))
                        .when(url_empty, |d| d.child(div().absolute().left(px(6.)).top_0().bottom_0().flex().items_center().text_size(px(12.)).italic().text_color(pal.text_faint).child(placeholder))),
                )
                .child(
                    div()
                        .id("ref-doc-search")
                        .w(px(34.))
                        .h(px(30.))
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .border_1()
                        .border_color(gpui_kit::rgb(0xaaaaaa))
                        .rounded_r(px(3.))
                        .when(url_empty, |d| d.opacity(0.25))
                        .cursor_pointer()
                        .child(crate::ui::glyph("search", 20., pal.text))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let text = this.ref_picker.as_ref().map(|p| p.url.read(cx).value().trim().to_string()).unwrap_or_default();
                            this.search_documents(text, cx);
                        })),
                ),
        );
        let Some(_) = category.or(query.as_ref().map(|_| 0)) else {
            // The filters: 32 px rows, 20 px icons.
            let mut list = v_flex().pt(px(10.)).pl(px(10.));
            for (i, (label, icon)) in FILTERS.iter().enumerate() {
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("ref-filter-{i}")))
                        .h(px(32.))
                        .pl(px(6.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_size(px(14.67))
                        .font_weight(FontWeight::LIGHT)
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon(icon, 20., cx))
                        .child(*label)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ensure_documents(cx);
                            if let Some(p) = this.ref_picker.as_mut() {
                                p.category = Some(i);
                            }
                            cx.notify();
                        })),
                );
            }
            return col.child(list).into_any_element();
        };
        let title = query.clone().map(|q| format!("Results for \"{q}\"")).or_else(|| category.map(|c| FILTERS[c].0.to_string())).unwrap_or_default();
        let nav = |id: &'static str, icon: &'static str, tip: &'static str| {
            div().id(id).size(px(30.)).flex().flex_none().items_center().justify_center().rounded(px(3.)).cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xeaeaea))).tooltip(crate::ui::tooltip(tip)).child(crate::ui::glyph(icon, if icon == "ui-chevron-left" { 16. } else { 20. }, gpui_kit::rgb(0x333333)))
        };
        let home = |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
            if let Some(p) = this.ref_picker.as_mut() {
                p.category = None;
                p.doc_query = None;
            }
            cx.notify();
        };
        col = col.child(
            h_flex()
                .h(px(40.))
                .flex_none()
                .p(px(2.))
                .gap(px(3.))
                .border_t_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .child(nav("ref-docs-home", "home", "Go to document filters").on_click(cx.listener(home)))
                .child(nav("ref-docs-back", "ui-chevron-left", "Filters").on_click(cx.listener(home)))
                .child(div().pl(px(3.)).text_size(px(14.67)).font_weight(FontWeight::MEDIUM).truncate().child(title)),
        );
        let me = self.ref_document_path.clone();
        let Some(p) = self.ref_picker.as_ref() else { return col.into_any_element() };
        let mut docs: Vec<crate::start_library::DocEntry> = p.docs.clone().unwrap_or_default().into_iter().filter(|d| !d.trashed && !d.broken && Some(&d.path) != me.as_ref()).collect();
        let q = query.as_ref().map(|q| q.to_lowercase());
        match (category, &q) {
            (_, Some(q)) => docs.retain(|d| d.name.to_lowercase().contains(q.as_str())),
            // Local documents are all the user's own; none are shared or public.
            (Some(4 | 5), _) => docs.clear(),
            (Some(0 | 2), _) => {
                docs.retain(|d| d.recent_rank.is_some());
                docs.sort_by_key(|d| d.recent_rank);
            }
            _ => docs.sort_by_key(|d| std::cmp::Reverse(d.modified)),
        }
        let mut list = v_flex().id("ref-docs").flex_1().min_h_0().overflow_y_scroll();
        // Owned by me: the folders first (folder outline, bold name).
        if category == Some(1) && q.is_none() {
            let mut dirs: Vec<std::path::PathBuf> = docs.iter().map(|d| d.folder().to_path_buf()).collect();
            dirs.sort();
            dirs.dedup();
            if dirs.len() > 1 {
                for (i, dir) in dirs.iter().enumerate() {
                    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    list = list.child(
                        h_flex()
                            .id(SharedString::from(format!("ref-dir-{i}")))
                            .h(px(50.))
                            .flex_none()
                            .px(px(2.))
                            .hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)))
                            .tooltip(crate::ui::tooltip(name.clone()))
                            .child(div().w(px(74.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph("folder", 28., gpui_kit::rgb(0x666666))))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).child(name)),
                    );
                }
            }
        }
        if docs.is_empty() && p.docs.as_ref().is_some_and(|d| !d.is_empty()) {
            list = list.child(div().pl(px(10.)).h(px(28.)).flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("No documents found"));
        }
        for (i, d) in docs.into_iter().enumerate() {
            let (version, has) = p.doc_info.get(&d.path).cloned().unwrap_or_default();
            let thumb = match d.thumbnail.clone() {
                Some(img) => gpui_kit::img(img).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                None => crate::assets::icon("document", 32., cx),
            };
            let path = d.path.clone();
            let tip = if version.is_empty() { d.name.clone() } else { format!("{} -- {version}", d.name) };
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("ref-doc-{i}")))
                    .h(px(50.))
                    .flex_none()
                    .px(px(2.))
                    .cursor_pointer()
                    .when(!has, |r| r.opacity(0.35))
                    .hover(|r| r.bg(gpui_kit::rgb(0xdef1fb)))
                    .tooltip(crate::ui::tooltip(tip))
                    .child(div().w(px(74.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(thumb))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).child(d.name.clone()))
                            .when(!version.is_empty(), |c| c.child(div().opacity(0.5).text_size(px(12.)).line_height(px(15.6)).child(version))),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_other_document(path.clone(), cx))),
            );
        }
        col.child(list).into_any_element()
    }

    /// "Import…": choose a file; the Part Studio adds it as a tab and picks it.
    fn import_reference(&mut self, kind: ReferenceKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.ref_picker.as_ref().map(|r| r.path.clone()) else { return };
        let exts: &'static [&'static str] = match kind {
            ReferenceKind::Image => &["png", "jpg", "jpeg", "gif", "PNG", "JPG", "JPEG", "GIF"],
            _ => &["csv", "CSV"],
        };
        let view = cx.entity();
        crate::files::request_open_path("Import", "Files", exts, window, cx, move |file, cx| {
            view.update(cx, |_, cx| cx.emit(DialogEvent::ImportReference { param: path, file }));
        });
    }

    fn values_text(&self, path: &str) -> String {
        match self.feature.params.get(path) {
            Some(ParamValue::Text(t)) => t.clone(),
            _ => String::new(),
        }
    }
}

/// 70×40 pictures of Part Studios and their parts for the panel, keyed like
/// the stored values (`tab`, `tab#part`): each studio regenerated and
/// rendered with the document-thumbnail look (Onshape's select-item rows).
pub async fn render_thumbnails(gpu: crate::gpu::Gpu, doc: Document, tabs: Vec<noshape_model::document::TabId>, key: impl Fn(noshape_model::document::TabId) -> String) -> Vec<(String, std::sync::Arc<RenderImage>)> {
    use noshape_model::display::BodyKind;
    use noshape_model::regen::{RegenOptions, Regenerator};
    let custom = noshape_model::custom::DocumentScripts::from_document(&doc);
    let mut out = Vec::new();
    for tab in tabs {
        let Some(TabContent::PartStudio(ps)) = doc.tab(tab).map(|t| &t.content) else { continue };
        let opts = RegenOptions { custom_features: Some(custom.clone()), ..Default::default() };
        let r = Regenerator::new().regenerate(ps, &ps.configuration.defaults(), &doc.settings.units, &opts);
        let display = &r.display;
        let parts = std::iter::once(None).chain(display.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface) && !b.hidden).map(|b| Some(b.id.clone())));
        for part in parts {
            let mut d = display.clone();
            if let Some(p) = &part {
                for b in &mut d.bodies {
                    b.hidden = &b.id != p;
                }
            }
            let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
            let mut cache = crate::scene_builder::GpuCache::default();
            let built = crate::scene_builder::build(&d, &opts, &mut cache, gpu.uploader());
            let Some((lo, hi)) = built.scene.bounds() else { continue };
            let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), 140. / 80.);
            let Ok(png) = gpu.render_png(std::sync::Arc::new(built.scene), camera, crate::start_thumbnail::thumbnail_style(), 140, 80).await else { continue };
            if let Some(img) = decode(&png) {
                let studio = key(tab);
                out.push((part.map_or_else(|| studio.clone(), |p| part_value(&studio, &p)), img));
            }
        }
    }
    out
}

/// PNG -> GPUI image (BGRA), white made transparent so it sits on the
/// selected (blue) row.
fn decode(png: &[u8]) -> Option<std::sync::Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[0] >= 250 && px[1] >= 250 && px[2] >= 250 {
            *px = [0; 4];
        }
    }
    Some(std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
