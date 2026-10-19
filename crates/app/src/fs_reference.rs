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

/// The open "Select …" panel.
pub struct RefPicker {
    pub path: String,
    pub kind: ReferenceKind,
    other_documents: bool,
    search: Entity<InputState>,
    _search_sub: Subscription,
    /// Part Studios whose parts are listed.
    expanded: HashSet<String>,
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

impl FeatureDialog {
    /// The parameter row: label, then the bordered field (blue while its
    /// panel is open).
    pub(crate) fn render_reference_field(&mut self, label: &'static str, path: &str, kind: ReferenceKind, value: &str, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (icon, placeholder, _, _) = texts(kind);
        let open = self.ref_picker.as_ref().is_some_and(|r| r.path == path);
        let chosen = self.ref_sources.iter().find(|s| s.tab == source_tab(value)).map(|s| s.name.clone());
        let empty = chosen.is_none();
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
        let search = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe(&search, |_: &mut Self, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        self.ref_picker = Some(RefPicker { path, kind, other_documents: false, search, _search_sub: sub, expanded: HashSet::new() });
    }

    fn pick_reference(&mut self, value: String, cx: &mut Context<Self>) {
        let Some(path) = self.ref_picker.as_ref().map(|r| r.path.clone()) else { return };
        self.set_value(&path, ParamValue::Text(value), cx);
    }

    /// The 300×559 "Select …" panel right of the dialog, or nothing.
    pub(crate) fn render_reference_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let picker = self.ref_picker.as_ref()?;
        let (path, kind, other) = (picker.path.clone(), picker.kind, picker.other_documents);
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
                    if let Some(r) = &mut this.ref_picker {
                        r.other_documents = id == "ref-other";
                    }
                    cx.notify();
                }))
        };
        let square = |id: &'static str, icon: &'static str, tip: &'static str, cx: &App| {
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
                .hover(|d| d.bg(p().panel_hover))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(icon, 20., cx))
        };
        let mut list = v_flex().id("ref-list").flex_1().min_h_0().pt(px(1.)).overflow_y_scroll();
        if !other {
            let row = |id: SharedString, indent: f32, expander: Option<AnyElement>, icon: &'static str, name: String, on: bool, cx: &App| {
                h_flex()
                    .id(id)
                    .h(px(40.))
                    .flex_none()
                    .pl(px(indent))
                    .when(on, |d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                    .when(!on, |d| d.hover(|d| d.bg(p().panel_hover)))
                    .cursor_pointer()
                    .children(expander)
                    .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(crate::assets::icon(icon, 30., cx)))
                    .child(div().flex_1().min_w_0().pl(px(4.)).truncate().text_size(px(13.33)).font_weight(FontWeight::LIGHT).text_color(p().text).child(name.clone()))
                    .tooltip(crate::ui::tooltip(name))
            };
            let sources: Vec<RefSource> = self.ref_sources.iter().filter(|s| accepts(kind, s)).cloned().collect();
            for s in sources {
                let parts = match &s.what {
                    RefSourceKind::PartStudio(parts) => Some(parts.clone()),
                    _ => None,
                };
                let name_hit = query.is_empty() || s.name.to_lowercase().contains(&query);
                let part_hits: Vec<(noshape_model::BodyId, String)> = parts.iter().flatten().filter(|(_, n)| name_hit || n.to_lowercase().contains(&query)).cloned().collect();
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
                let tab_id = s.tab.clone();
                let open = parts.is_some() && (expanded.contains(&s.tab) || !query.is_empty());
                let picked = current == s.tab;
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
                let r = row(SharedString::from(format!("ref-src-{}", s.tab)), 0., expander, icon, s.name.clone(), picked, cx);
                list = list.child(r.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_reference(tab_id.clone(), cx))));
                if open {
                    for (body, name) in part_hits {
                        let value = format!("{}#{body}", s.tab);
                        let on = current == value;
                        list = list.child(row(SharedString::from(format!("ref-part-{value}")), 36., None, "part", name, on, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_reference(value.clone(), cx))));
                    }
                }
            }
        }
        let doc_name = self.ref_document_name.clone();
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
            .when(!other, |d| {
                d.child(
                    h_flex()
                        .h(px(40.))
                        .flex_none()
                        .p(px(2.))
                        .pl(px(6.))
                        .gap(px(6.))
                        .border_t_1()
                        .border_color(line)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).child(doc_name))
                                .child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).line_height(px(15.6)).child(crate::ui::glyph("branch", 10., pal.text)).child("Main")),
                        )
                        .child(square("ref-create-version", "version", "Create version…", cx))
                        .child(square("ref-version-graph", "version-graph", "Version graph", cx)),
                )
                // The placeholder is italic grey (live 482), drawn over the field.
                .child(
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
                )
            })
            .child(list)
            .when(import && !other, |d| {
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
