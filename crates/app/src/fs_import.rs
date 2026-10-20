//! The Feature Studio toolbar's Import panel (live document/81–91,
//! dom/fs-import-panel.txt, behaviours.md "Import button"): a 300×530 panel
//! under the button with Current document | Other documents, the document
//! header (name, workspace or version, Create version…, Version graph), the
//! sub-tabs Part Studios | Files | Feature Studios | Variable Studios, a
//! search box and the list. Each pick appends one `import(...)` line after
//! the studio's last import; the panel stays open.
//!
//! Import paths: a tab of this document is its tab id with the workspace's
//! microversion as the version; another document's is
//! `<document id>/<version microversion>/<tab id>` (Onshape's
//! documentId/versionId/elementId), where the document id is its first
//! microversion ([`document_id`]).

use crate::doc_model::DocModel;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_store::MicroversionId;
use std::path::{Path, PathBuf};

const BLUE: u32 = 0x1651b0;
const LINE: u32 = 0xeeeeee;

/// The sub-tabs, in Onshape's order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    PartStudios,
    Files,
    FeatureStudios,
    VariableStudios,
}

impl Kind {
    const ALL: [Kind; 4] = [Kind::PartStudios, Kind::Files, Kind::FeatureStudios, Kind::VariableStudios];
    fn label(self) -> &'static str {
        match self {
            Kind::PartStudios => "Part Studios",
            Kind::Files => "Files",
            Kind::FeatureStudios => "Feature Studios",
            Kind::VariableStudios => "Variable Studios",
        }
    }
    fn placeholder(self) -> &'static str {
        match self {
            Kind::PartStudios => "Search Part Studios",
            Kind::Files => "Search files",
            Kind::FeatureStudios => "Search Feature Studios",
            Kind::VariableStudios => "Search Variable Studios",
        }
    }
    fn accepts(self, c: &TabContent) -> bool {
        matches!(
            (self, c),
            (Kind::PartStudios, TabContent::PartStudio(_)) | (Kind::Files, TabContent::File(_)) | (Kind::FeatureStudios, TabContent::FeatureStudio(_)) | (Kind::VariableStudios, TabContent::VariableStudio(_))
        )
    }
}

/// A document's id in import paths: its first microversion.
pub fn document_id(path: &Path) -> Option<MicroversionId> {
    let store = noshape_store::Document::open_read_only(path).ok()?;
    store.graph().ok()?.microversions.iter().find(|m| m.parents.is_empty()).map(|m| m.id)
}

/// A microversion as an import version string (32 hex digits).
fn version_text(mv: MicroversionId) -> String {
    mv.as_uuid().simple().to_string()
}

/// Another document opened in the panel, at one of its versions.
struct OtherDoc {
    path: PathBuf,
    name: String,
    id: Option<MicroversionId>,
    versions: Vec<crate::derived::VersionChoice>,
    version: usize,
    tabs: Option<std::sync::Arc<Document>>,
}

pub enum ImportEvent {
    /// One `import(...)` line to append after the last import.
    Insert(String),
    Close,
}

pub struct ImportPanel {
    doc: Entity<DocModel>,
    /// The Feature Studio importing (not listed among Feature Studios).
    studio: TabId,
    other: bool,
    kind: Kind,
    search: Entity<InputState>,
    /// The version list replaces the tab list.
    graph: bool,
    /// This document at a version (`None` = the workspace).
    current_version: Option<crate::derived::VersionChoice>,
    current_snapshot: Option<std::sync::Arc<Document>>,
    docs: Option<Vec<crate::start_library::DocEntry>>,
    other_doc: Option<OtherDoc>,
    /// Part Studio pictures (live: 70×40 thumbnails), per listed document
    /// state (`thumbs_for`).
    thumbs: std::collections::HashMap<TabId, std::sync::Arc<RenderImage>>,
    thumbs_for: Option<String>,
    evaluator: std::sync::Arc<parking_lot::Mutex<noshape_model::assembly::eval::AssemblyEvaluator>>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<ImportEvent> for ImportPanel {}

impl ImportPanel {
    pub fn new(doc: Entity<DocModel>, studio: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(Kind::PartStudios.placeholder()));
        let subs = vec![cx.subscribe(&search, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })];
        Self { doc, studio, other: false, kind: Kind::PartStudios, search, graph: false, current_version: None, current_snapshot: None, docs: None, other_doc: None, thumbs: Default::default(), thumbs_for: None, evaluator: Default::default(), _subs: subs }
    }

    fn set_kind(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        self.kind = kind;
        self.search.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.set_placeholder(kind.placeholder(), window, cx);
        });
        cx.notify();
    }

    fn ensure_documents(&mut self, cx: &mut Context<Self>) {
        if self.docs.is_some() {
            return;
        }
        self.docs = Some(Vec::new());
        let own = self.doc.read(cx).session.path.clone();
        cx.spawn(async move |this, cx| {
            let docs = cx.background_spawn(async move { crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs }).await;
            let _ = this.update(cx, |this, cx| {
                this.docs = Some(docs.into_iter().filter(|d| !d.trashed && d.path != own).collect());
                cx.notify();
            });
        })
        .detach();
    }

    /// Another document opens at its latest version (live: never its workspace).
    fn open_other(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some((name, versions)) = crate::fs_reference::document_versions(&path) else { return };
        let id = document_id(&path);
        let tabs = versions.first().and_then(|v| crate::derived::external_snapshot(&path, v.microversion).ok());
        self.other_doc = Some(OtherDoc { path, name, id, versions, version: 0, tabs });
        self.graph = false;
        cx.notify();
    }

    /// Render the Part Studio pictures of the listed document state once.
    fn request_thumbs(&mut self, cx: &mut Context<Self>) {
        let (key, doc): (String, std::sync::Arc<Document>) = if self.other {
            let Some(o) = self.other_doc.as_ref() else { return };
            let Some(d) = o.tabs.clone() else { return };
            (format!("{}@{}", o.path.display(), o.version), d)
        } else if let Some(d) = self.current_snapshot.clone() {
            (format!("current@{:?}", self.current_version.as_ref().map(|v| v.microversion)), d)
        } else {
            let s = &self.doc.read(cx).session;
            (format!("current@{:?}", s.head().ok()), std::sync::Arc::new(s.doc.clone()))
        };
        if self.thumbs_for.as_deref() == Some(key.as_str()) {
            return;
        }
        self.thumbs_for = Some(key);
        self.thumbs.clear();
        let tabs: Vec<TabId> = doc.ordered_tabs().iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| t.id).collect();
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        let evaluator = self.evaluator.clone();
        cx.spawn(async move |this, cx| {
            let made = cx
                .background_spawn(async move {
                    let mut out = Vec::new();
                    for tab in tabs {
                        if let Some(img) = crate::tab_manager::tab_thumbnail(&doc, tab, false, &evaluator, &gpu).await {
                            out.push((tab, img));
                        }
                    }
                    out
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.thumbs.extend(made);
                cx.notify();
            });
        })
        .detach();
    }

    /// The tabs listed (kind, search) as (tab, name, icon).
    fn rows(&self, cx: &App) -> Vec<(TabId, String, &'static str)> {
        let q = self.search.read(cx).value().trim().to_lowercase();
        let here = self.doc.read(cx).session.doc.clone();
        let doc: &Document = if self.other {
            match self.other_doc.as_ref().and_then(|o| o.tabs.as_deref()) {
                Some(d) => d,
                None => return Vec::new(),
            }
        } else {
            self.current_snapshot.as_deref().unwrap_or(&here)
        };
        let mut rows: Vec<(TabId, String, &'static str)> = doc
            .ordered_tabs()
            .iter()
            .filter(|t| self.kind.accepts(&t.content) && (self.other || t.id != self.studio))
            .filter(|t| q.is_empty() || t.name.to_lowercase().contains(&q))
            .map(|t| {
                let icon = match &t.content {
                    TabContent::PartStudio(_) => "part-studio",
                    TabContent::FeatureStudio(_) => "feature-studio",
                    TabContent::VariableStudio(_) => "variable-studio",
                    TabContent::File(f) => crate::file_tab_view::file_icon(f),
                    _ => "file",
                };
                (t.id, t.name.clone(), icon)
            })
            .collect();
        rows.sort_by_key(|r| r.1.to_lowercase());
        rows
    }

    /// The `import(...)` line for a tab (live document/83–89).
    fn import_line(&self, tab: TabId, cx: &App) -> Option<String> {
        let session = &self.doc.read(cx).session;
        if self.other {
            let o = self.other_doc.as_ref()?;
            let v = o.versions.get(o.version)?.microversion;
            let id = o.id?;
            return Some(format!("import(path : \"{}/{}/{tab}\", version : \"{}\");", version_text(id), version_text(v), version_text(v)));
        }
        match &self.current_version {
            Some(v) => {
                let id = document_id(&session.path)?;
                Some(format!("import(path : \"{}/{}/{tab}\", version : \"{}\");", version_text(id), version_text(v.microversion), version_text(v.microversion)))
            }
            None => Some(format!("import(path : \"{tab}\", version : \"{}\");", version_text(session.head().ok()?))),
        }
    }
}

impl Render for ImportPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let blue = rgb(BLUE);
        let tab = |id: &'static str, label: &'static str, on: bool, w: f32, h: f32| {
            div()
                .id(id)
                .w(px(w))
                .h(px(h))
                .flex_none()
                .p(px(5.))
                .flex()
                .items_start()
                .justify_center()
                .text_center()
                .text_size(px(12.))
                .line_height(px(18.))
                .cursor_pointer()
                .when(on, |d| d.text_color(blue).font_weight(FontWeight::MEDIUM).border_b_2().border_color(blue))
                .when(!on, |d| d.text_color(pal.text))
                .child(div().w_full().text_center().child(label))
        };
        let square = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &App| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .cursor_pointer()
                .when(on, |d| d.bg(rgb(0xeaeaea)))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(icon, 20., cx))
        };
        // Header: "Import" ✕.
        let header = h_flex()
            .h(px(29.))
            .flex_none()
            .pl(px(5.))
            .pr(px(6.))
            .border_b_1()
            .border_color(rgb(LINE))
            .child(div().flex_1().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Import"))
            .child(div().id("fsimp-close").cursor_pointer().child(os::glyph(os::GLYPH_CLOSE, 14., pal.text_faint)).on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(ImportEvent::Close))));
        let sources = h_flex()
            .h(px(28.))
            .flex_none()
            .border_b_1()
            .border_color(rgb(LINE))
            .child(tab("fsimp-current", "Current document", !self.other, 150., 28.).on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                t.other = false;
                t.graph = false;
                cx.notify();
            })))
            .child(tab("fsimp-other", "Other documents", self.other, 150., 28.).on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                t.other = true;
                t.graph = false;
                t.ensure_documents(cx);
                cx.notify();
            })));
        let mut body = v_flex().w(px(300.)).h(px(530.)).bg(pal.panel).border_1().border_color(rgb(0xd7d7d7)).shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(6.)), blur_radius: px(12.), spread_radius: px(0.), inset: false }]).child(header).child(sources);

        // Other documents without a document chosen: the document list.
        if self.other && self.other_doc.is_none() {
            let q = self.search.read(cx).value().trim().to_lowercase();
            let docs = self.docs.clone().unwrap_or_default();
            let mut list = v_flex().id("fsimp-docs").flex_1().min_h_0().overflow_y_scroll();
            for (i, d) in docs.iter().filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q)).enumerate() {
                let path = d.path.clone();
                list = list.child(
                    h_flex()
                        .id(("fsimp-doc", i))
                        .h(px(40.))
                        .flex_none()
                        .gap(px(4.))
                        .cursor_pointer()
                        .hover(|r| r.bg(pal.row_hover))
                        .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(match d.thumbnail.clone() {
                            Some(img) => gpui_kit::img(img).w(px(64.)).h(px(36.)).object_fit(ObjectFit::Contain).into_any_element(),
                            None => crate::assets::icon("document", 24., cx),
                        }))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(13.33)).font_weight(FontWeight::LIGHT).text_color(pal.text).child(d.name.clone()))
                        .child(div().pr(px(8.)).child(os::glyph(os::GLYPH_CHEVRON_RIGHT, 10., pal.text)))
                        .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| t.open_other(path.clone(), cx))),
                );
            }
            return body.child(div().px(px(6.)).pt(px(6.)).pb(px(6.)).flex_none().child(div().h(px(30.)).border_1().border_color(rgb(0xaaaaaa)).rounded(px(3.)).child(Input::new(&self.search).appearance(false).text_size(px(14.)).italic()))).child(list).into_any_element();
        }

        // The document header: name, workspace / version, buttons.
        let (name, place) = if self.other {
            let o = self.other_doc.as_ref();
            (o.map(|o| o.name.clone()).unwrap_or_default(), o.and_then(|o| o.versions.get(o.version)).map(|v| v.label.clone()).unwrap_or_default())
        } else {
            let d = self.doc.read(cx);
            (d.name(), self.current_version.as_ref().map(|v| v.label.clone()).unwrap_or_else(|| "Main".into()))
        };
        let doc = self.doc.clone();
        let title = h_flex()
            .h(px(40.))
            .flex_none()
            .px(px(6.))
            .gap(px(6.))
            .border_b_1()
            .border_color(rgb(LINE))
            .when(self.other, |d| {
                d.child(div().id("fsimp-back").cursor_pointer().child(os::glyph(os::GLYPH_BACK, 16., pal.text)).on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                    t.other_doc = None;
                    cx.notify();
                })))
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(18.)).text_color(pal.text).child(name))
                    .child(h_flex().gap(px(3.)).opacity(0.5).text_size(px(14.)).line_height(px(18.)).child(crate::ui::glyph("branch", 10., pal.text)).child(place)),
            )
            .when(!self.other, |d| d.child(square("fsimp-version", "version", "Create version…", false, cx).on_click(move |_, window, cx| crate::history_panel::open_create_version_in(doc.clone(), window, cx))))
            .child(square("fsimp-graph", "version-graph", "Version graph", self.graph, cx).on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                t.graph = !t.graph;
                cx.notify();
            })));
        body = body.child(title);

        if self.graph {
            // Version graph: the workspace (this document only), versions newest first.
            let choices: Vec<(Option<usize>, String)> = if self.other {
                self.other_doc.as_ref().map(|o| o.versions.iter().enumerate().map(|(i, v)| (Some(i), v.label.clone())).collect()).unwrap_or_default()
            } else {
                let path = self.doc.read(cx).session.path.clone();
                let versions = crate::fs_reference::document_versions(&path).map(|v| v.1).unwrap_or_default();
                std::iter::once((None, "Main".to_string())).chain(versions.iter().enumerate().map(|(i, v)| (Some(i), v.label.clone()))).collect()
            };
            let mut list = v_flex().id("fsimp-graph-list").flex_1().min_h_0().overflow_y_scroll().pt(px(4.));
            for (n, (ix, label)) in choices.into_iter().enumerate() {
                let on = if self.other { self.other_doc.as_ref().is_some_and(|o| Some(o.version) == ix) } else { ix.is_none() == self.current_version.is_none() && (ix.is_none() || self.current_version.as_ref().map(|v| v.label.as_str()) == Some(label.as_str())) };
                list = list.child(
                    h_flex()
                        .id(("fsimp-ver", n))
                        .h(px(30.))
                        .flex_none()
                        .px(px(10.))
                        .gap(px(8.))
                        .cursor_pointer()
                        .text_size(px(13.33))
                        .text_color(pal.text)
                        .when(on, |r| r.bg(pal.accent_soft))
                        .when(!on, |r| r.hover(|r| r.bg(pal.row_hover)))
                        .child(div().size(px(9.)).rounded_full().when(ix.is_none(), |d| d.border_2().border_color(blue)).when(ix.is_some(), |d| d.bg(blue)))
                        .child(div().when(ix.is_none(), |d| d.italic().text_color(rgb(0x888888))).child(label))
                        .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                            if t.other {
                                if let (Some(i), Some(o)) = (ix, t.other_doc.as_mut()) {
                                    o.version = i;
                                    o.tabs = o.versions.get(i).and_then(|v| crate::derived::external_snapshot(&o.path, v.microversion).ok());
                                }
                            } else {
                                let path = t.doc.read(cx).session.path.clone();
                                t.current_version = ix.and_then(|i| crate::fs_reference::document_versions(&path).and_then(|v| v.1.get(i).cloned()));
                                t.current_snapshot = t.current_version.as_ref().and_then(|v| crate::derived::external_snapshot(&path, v.microversion).ok());
                            }
                            t.graph = false;
                            cx.notify();
                        })),
                );
            }
            return body.child(list).into_any_element();
        }

        let kinds = Kind::ALL.iter().enumerate().fold(h_flex().h(px(46.)).flex_none().border_b_1().border_color(rgb(LINE)), |row, (i, k)| {
            let k = *k;
            row.child(tab(["fsimp-k0", "fsimp-k1", "fsimp-k2", "fsimp-k3"][i], k.label(), self.kind == k, 75., 46.).on_click(cx.listener(move |t, _: &ClickEvent, window, cx| t.set_kind(k, window, cx))))
        });
        if self.kind == Kind::PartStudios {
            self.request_thumbs(cx);
        }
        let rows = self.rows(cx);
        let mut list = v_flex().id("fsimp-rows").flex_1().min_h_0().overflow_y_scroll();
        if rows.is_empty() {
            list = list.child(div().px(px(10.)).py(px(8.)).text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(format!("No {} available.", self.kind.label().to_lowercase())));
        }
        for (i, (tab_id, name, icon)) in rows.into_iter().enumerate() {
            list = list.child(
                h_flex()
                    .id(("fsimp-row", i))
                    .h(px(40.))
                    .flex_none()
                    .mb(px(1.))
                    .cursor_pointer()
                    .hover(|r| r.bg(pal.row_hover))
                    .tooltip(crate::ui::tooltip(name.clone()))
                    .child(div().w(px(70.)).h(px(40.)).mx(px(2.)).flex_none().flex().items_center().justify_center().child(match self.thumbs.get(&tab_id) {
                        Some(img) => gpui_kit::img(img.clone()).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                        None => crate::assets::icon(icon, 30., cx),
                    }))
                    .child(div().pl(px(4.)).flex_1().min_w_0().truncate().text_size(px(13.33)).font_weight(FontWeight::LIGHT).text_color(pal.text).child(name))
                    .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                        // The panel stays open for more picks (live).
                        if let Some(line) = t.import_line(tab_id, cx) {
                            cx.emit(ImportEvent::Insert(line));
                        }
                    })),
            );
        }
        body.child(kinds)
            .child(div().px(px(6.)).pt(px(6.)).flex_none().child(div().h(px(30.)).border_1().border_color(rgb(0xaaaaaa)).rounded(px(3.)).child(Input::new(&self.search).appearance(false).text_size(px(14.)).italic())))
            .child(div().h(px(6.)).flex_none())
            .child(list)
            .into_any_element()
    }
}

/// `src` with `line` after its last `import(...)` line (live: not at the
/// cursor), or after the `FeatureScript` header when it has none.
pub fn append_import(src: &str, line: &str) -> String {
    let lines: Vec<&str> = src.split('\n').collect();
    let is_import = |l: &str| {
        let t = l.trim_start();
        t.starts_with("import(") || t.starts_with("export import(") || (t.contains("::import(") && !t.starts_with("//"))
    };
    let at = lines.iter().rposition(|l| is_import(l)).or_else(|| lines.iter().position(|l| l.trim_start().starts_with("FeatureScript"))).map(|i| i + 1).unwrap_or(0);
    let mut out: Vec<&str> = lines[..at].to_vec();
    out.push(line);
    out.extend_from_slice(&lines[at..]);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::append_import;

    #[test]
    fn imports_go_after_the_last_import() {
        let src = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n\nexport const x = 1;";
        let out = append_import(src, "import(path : \"a\", version : \"b\");");
        assert_eq!(out, "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\nimport(path : \"a\", version : \"b\");\n\nexport const x = 1;");
        let out = append_import("FeatureScript 3095;\n", "import(path : \"a\", version : \"b\");");
        assert_eq!(out, "FeatureScript 3095;\nimport(path : \"a\", version : \"b\");\n");
        let out = append_import("FeatureScript 3095;\nCsv::import(path : \"c\", version : \"d\");\nx", "import(path : \"a\", version : \"b\");");
        assert_eq!(out, "FeatureScript 3095;\nCsv::import(path : \"c\", version : \"d\");\nimport(path : \"a\", version : \"b\");\nx");
    }
}
