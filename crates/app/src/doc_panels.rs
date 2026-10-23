//! Left-rail document panels (live document/rail-panel-*.png, DOM
//! dom/rail-panel-*.txt): **Document notes** (Markdown notes stored with the
//! document) and **Performance** (tab regeneration, tab graphics, document
//! tabs, system). Both sit in the 380 px left split under a 37 px header.

use crate::doc_model::DocModel;
use crate::theme::p;
use gpui_kit::component::input::{TextareaState, Textarea};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::HashSet;

/// The store key of the document's notes (plain Markdown).
const NOTES_KEY: &str = "notes";

/// Asks the workspace to close the panel (its ✕).
pub struct ClosePanel;

/// Performance › the stopwatch: show the feature list's regeneration times.
pub struct ShowRegenTimes;

/// Onshape's document-panel header: 37 px, 1 px `#d7d7d7` lines above and
/// below, 5 px padding, the title 14 px bold.
fn panel_header(title: &'static str) -> Div {
    let pal = p();
    h_flex()
        .h(px(37.))
        .flex_none()
        .px(px(5.))
        .gap(px(10.))
        .border_t_1()
        .border_b_1()
        .border_color(gpui_kit::rgb(0xd7d7d7))
        .child(div().flex_1().text_size(px(14.)).line_height(px(21.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(title))
}

/// A 20 px header icon with a tooltip.
fn header_icon(id: &'static str, icon: &'static str, tip: &'static str) -> Stateful<Div> {
    div().id(id).size(px(20.)).flex_none().cursor_pointer().child(crate::ui::glyph(icon, 20., p().text)).tooltip(crate::ui::tooltip(tip))
}

/// The panels' background (live `#f5f5f5`).
fn panel_bg() -> Rgba {
    gpui_kit::rgb(0xf5f5f5)
}

/// The gray footer bar with the help link (31 px, `#eeeeee`).
fn panel_footer(help: &'static str, cx: &App) -> Div {
    h_flex()
        .h(px(31.))
        .flex_none()
        .px(px(5.))
        .justify_end()
        .bg(gpui_kit::rgb(0xeeeeee))
        .border_t_1()
        .border_b_1()
        .border_color(gpui_kit::rgb(0xcccccc))
        .child(
            div()
                .id("panel-help")
                .size(px(14.))
                .mr(px(3.))
                .opacity(0.8)
                .cursor_pointer()
                .child(crate::assets::icon("help-filled", 14., cx))
                .on_click(move |_, _, cx| cx.open_url(help)),
        )
}

pub struct NotesPanel {
    doc: Entity<DocModel>,
    notes: String,
    /// The editor while editing (Apply / Cancel).
    editor: Option<Entity<TextareaState>>,
    preview: bool,
}

impl EventEmitter<ClosePanel> for NotesPanel {}

impl NotesPanel {
    pub fn new(doc: Entity<DocModel>, cx: &mut Context<Self>) -> Self {
        let notes = read_notes(&doc, cx);
        Self { doc, notes, editor: None, preview: false }
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.notes.clone();
        let st = cx.new(|cx| TextareaState::new(window, cx).default_value(text));
        cx.observe(&st, |_, _, cx| cx.notify()).detach();
        window.focus(&st.focus_handle(cx), cx);
        self.editor = Some(st);
        self.preview = false;
        cx.notify();
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let Some(st) = self.editor.take() else { return };
        let text = st.read(cx).value().to_string();
        let r = self.doc.update(cx, |dm, _| dm.session.store.set_meta(NOTES_KEY, Some(text.as_bytes())));
        if let Err(e) = r {
            self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::Error(format!("Could not save the notes: {e}"))));
        }
        self.notes = read_notes(&self.doc, cx);
        cx.notify();
    }
}

fn read_notes(doc: &Entity<DocModel>, cx: &App) -> String {
    doc.read(cx).session.store.get_meta(NOTES_KEY).ok().flatten().map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

impl Render for NotesPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let link = |id: &'static str, label: &'static str| {
            div().id(id).text_size(px(14.)).line_height(px(21.)).text_color(gpui_kit::rgb(0x094174)).cursor_pointer().hover(|d| d.underline()).child(label)
        };
        let close = header_icon("notes-close", "close", "Close").on_click(cx.listener(|_, _, _, cx| cx.emit(ClosePanel)));
        let header = match &self.editor {
            None => panel_header("Document notes")
                .child(header_icon("notes-refresh", "refresh", "Refresh notes").on_click(cx.listener(|this, _, _, cx| {
                    this.notes = read_notes(&this.doc, cx);
                    cx.notify();
                })))
                .child(header_icon("notes-edit", "edit", "Edit notes").on_click(cx.listener(|this, _, window, cx| this.edit(window, cx))))
                .child(close),
            Some(_) => panel_header("Document notes")
                .child(link("notes-apply", "Apply").on_click(cx.listener(|this, _, _, cx| this.apply(cx))))
                .child(link("notes-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                    this.editor = None;
                    cx.notify();
                })))
                .child(header_icon("notes-preview", "show-preview", "Show preview").when(self.preview, |d| d.bg(pal.panel_active).rounded(px(3.))).on_click(cx.listener(|this, _, _, cx| {
                    this.preview = !this.preview;
                    cx.notify();
                })))
                .child(close),
        };
        let body: AnyElement = match &self.editor {
            Some(st) if self.preview => markdown(&st.read(cx).value()).into_any_element(),
            // The placeholder wraps (live), so it's drawn over the empty editor.
            Some(st) => div()
                .relative()
                .flex_1()
                .pl(px(5.))
                .pr(px(5.))
                .pt(px(9.))
                .child(Textarea::new(st).appearance(false).size_full().text_size(px(13.33)))
                .when(st.read(cx).value().is_empty(), |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(22.))
                            .left(px(15.))
                            .right(px(15.))
                            .text_size(px(13.33))
                            .line_height(px(21.))
                            .text_color(gpui_kit::rgb(0x666666))
                            .child("Add a description for your document, instructions, or notes for your collaborators here. This area supports Markdown."),
                    )
                })
                .into_any_element(),
            None if self.notes.trim().is_empty() => v_flex()
                .pt(px(30.))
                .items_center()
                .gap(px(20.))
                .child(svg().path(crate::assets::icon_path("notes-empty")).size(px(140.)).text_color(if gpui_kit::hex_theme_active() { crate::theme::p().illustration } else { gpui_kit::rgb(0xebebeb) }))
                .child(div().text_size(px(14.67)).line_height(px(22.)).text_color(pal.text_muted).child("Instructions, description, or notes for this document"))
                .into_any_element(),
            None => markdown(&self.notes).into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(panel_bg())
            .child(header)
            .child(v_flex().id("notes-body").flex_1().min_h_0().overflow_y_scroll().child(body))
            .child(panel_footer("https://cad.onshape.com/help/Content/document_notes.htm", cx))
    }
}

/// The Markdown subset the notes show: headings, bullet and numbered
/// lists, paragraphs, **bold** and *italic*.
fn markdown(src: &str) -> Div {
    let pal = p();
    let mut out = v_flex().px(px(15.)).py(px(15.)).gap(px(8.)).text_color(pal.text);
    let mut para: Vec<String> = Vec::new();
    let flush = |out: Div, para: &mut Vec<String>| {
        if para.is_empty() {
            return out;
        }
        let text = para.join(" ");
        para.clear();
        out.child(div().text_size(px(14.)).line_height(px(21.)).child(inline(&text)))
    };
    for line in src.lines() {
        let t = line.trim_end();
        let heading = [("### ", 15.), ("## ", 17.), ("# ", 20.)].into_iter().find(|(m, _)| t.starts_with(m));
        let bullet = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "));
        let numbered = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if t.trim().is_empty() {
            out = flush(out, &mut para);
        } else if let Some((m, size)) = heading {
            out = flush(out, &mut para);
            out = out.child(div().text_size(px(size)).font_weight(FontWeight::BOLD).child(inline(&t[m.len()..])));
        } else if let Some(rest) = bullet {
            out = flush(out, &mut para);
            out = out.child(h_flex().items_start().gap(px(8.)).pl(px(10.)).text_size(px(14.)).line_height(px(21.)).child("•").child(div().flex_1().child(inline(rest))));
        } else if let Some((n, rest)) = numbered {
            out = flush(out, &mut para);
            out = out.child(h_flex().items_start().gap(px(8.)).pl(px(10.)).text_size(px(14.)).line_height(px(21.)).child(format!("{n}.")).child(div().flex_1().child(inline(rest))));
        } else {
            para.push(t.trim().to_string());
        }
    }
    flush(out, &mut para)
}

/// `**bold**` and `*italic*` runs of one line.
fn inline(src: &str) -> StyledText {
    let mut text = String::new();
    let mut runs: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
    let mut rest = src;
    while !rest.is_empty() {
        let (marker, style) = if rest.starts_with("**") {
            ("**", HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })
        } else if rest.starts_with('*') {
            ("*", HighlightStyle { font_style: Some(FontStyle::Italic), ..Default::default() })
        } else {
            let next = rest.find('*').unwrap_or(rest.len());
            text.push_str(&rest[..next]);
            rest = &rest[next..];
            continue;
        };
        match rest[marker.len()..].find(marker) {
            Some(end) if end > 0 => {
                let start = text.len();
                text.push_str(&rest[marker.len()..marker.len() + end]);
                runs.push((start..text.len(), style));
                rest = &rest[marker.len() * 2 + end..];
            }
            _ => {
                text.push_str(marker);
                rest = &rest[marker.len()..];
            }
        }
    }
    StyledText::new(text).with_highlights(runs)
}

/// The active tab's numbers for the Performance panel (Part Studios only).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabPerf {
    pub regen_ms: f32,
    /// Feature type name and count.
    pub features: Vec<(String, usize)>,
    /// Kind of drawn primitive and count.
    pub primitives: Vec<(String, usize)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerfSnapshot {
    pub tab: Option<TabPerf>,
    /// Tab kind and count (Part Studios, Other tabs, Assemblies, Drawings).
    pub tabs: Vec<(String, usize)>,
    /// The GPU adapter ("name (Backend)"), once the renderer is up.
    pub adapter: Option<String>,
}

impl PerfSnapshot {
    /// Onshape flags software GL renderers (SwiftShader) as a performance
    /// error; ours are llvmpipe, lavapipe, WARP and the like.
    pub fn software_renderer(&self) -> bool {
        let a = self.adapter.as_deref().unwrap_or_default().to_lowercase();
        ["llvmpipe", "lavapipe", "softpipe", "swiftshader", "software", "basic render"].iter().any(|k| a.contains(k))
    }
}

pub struct PerformancePanel {
    snap: PerfSnapshot,
    /// Folded sections and opened sub-sections, by key.
    folded: HashSet<&'static str>,
    opened: HashSet<&'static str>,
    /// Per table: sort by the name column (else the count), descending.
    sort: std::collections::HashMap<&'static str, (bool, bool)>,
}

impl EventEmitter<ClosePanel> for PerformancePanel {}
impl EventEmitter<ShowRegenTimes> for PerformancePanel {}

impl PerformancePanel {
    pub fn new(snap: PerfSnapshot) -> Self {
        Self { snap, folded: HashSet::new(), opened: HashSet::new(), sort: Default::default() }
    }

    pub fn set_snapshot(&mut self, snap: PerfSnapshot, cx: &mut Context<Self>) {
        if snap != self.snap {
            self.snap = snap;
            cx.notify();
        }
    }

    /// A section: chevron, 20 px icon, 14.67 px title, an optional state
    /// icon at the right, its body, a 1 px `#aaaaaa` line under it.
    fn section(&self, key: &'static str, icon: &'static str, title: &'static str, state: Option<&'static str>, body: Vec<AnyElement>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = !self.folded.contains(key);
        v_flex()
            .border_b_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .child(
                h_flex()
                    .id(key)
                    .h(px(36.))
                    .pt(px(7.))
                    .items_start()
                    .cursor_pointer()
                    .child(div().size(px(20.)).p(px(4.)).child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 12., pal.text)))
                    .child(crate::ui::glyph(icon, 20., pal.text))
                    .child(div().flex_1().pl(px(4.)).text_size(px(14.67)).line_height(px(22.)).text_color(pal.text).child(title))
                    .children(state.map(|s| div().w(px(60.)).flex().justify_center().child(crate::assets::icon(s, 20., cx))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.folded.remove(key) {
                            this.folded.insert(key);
                        }
                        cx.notify();
                    })),
            )
            .when(open, |d| d.child(v_flex().pt(px(4.)).gap(px(4.)).children(body)))
            .into_any_element()
    }

    /// A sub-section heading: 13.33 px medium, with a chevron when it
    /// unfolds (`key`) and an optional state icon at the right.
    fn sub_heading(&self, key: Option<&'static str>, label: String, state: Option<&'static str>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = key.is_some_and(|k| self.opened.contains(k));
        h_flex()
            .id(SharedString::from(format!("perf-sub-{label}")))
            .when(key.is_none(), |d| d.h(px(32.)).items_center())
            .min_h(px(26.))
            .pl(px(if key.is_some() { 10. } else { 15. }))
            .items_start()
            .when_some(key, |d, k| {
                d.cursor_pointer()
                    .child(div().size(px(20.)).p(px(4.)).child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 12., pal.text)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.opened.remove(k) {
                            this.opened.insert(k);
                        }
                        cx.notify();
                    }))
            })
            .child(div().flex_1().pt(px(4.)).text_size(px(13.33)).line_height(px(15.)).text_color(pal.text).child(label))
            .children(state.map(|s| div().w(px(60.)).flex().justify_center().child(crate::assets::icon(s, 20., cx))))
            .into_any_element()
    }

    /// Onshape's sortable two-column table: 250 px, `#aaaaaa` lines,
    /// a 35 px `#f5f5f5` bold header, 26 px rows striped at 2% black.
    fn table(&self, key: &'static str, head: &'static str, rows: &[(String, usize)], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (by_name, desc) = self.sort.get(key).copied().unwrap_or((false, true));
        let mut rows = rows.to_vec();
        rows.sort_by(|a, b| {
            let o = if by_name { a.0.cmp(&b.0) } else { a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)) };
            if desc { o.reverse() } else { o }
        });
        let line = gpui_kit::rgb(0xaaaaaa);
        let head_cell = |id: &'static str, label: &'static str, w: f32, name_col: bool, cx: &mut Context<Self>| {
            let icon = (by_name == name_col).then_some(if desc { "sort-desc" } else { "sort-asc" });
            h_flex()
                .id(id)
                .w(px(w))
                .h(px(34.))
                .border_r_1()
                .border_color(line)
                .cursor_pointer()
                .child(div().flex_1().px(px(6.)).text_size(px(12.)).font_weight(FontWeight::BOLD).child(label))
                .child(div().size(px(30.)).flex().items_center().justify_center().children(icon.map(|i| crate::ui::glyph(i, 20., pal.text))))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let (n, d) = this.sort.get(key).copied().unwrap_or((false, true));
                    this.sort.insert(key, if n == name_col { (n, !d) } else { (name_col, true) });
                    cx.notify();
                }))
        };
        let name_id: &'static str = if key == "features" { "perf-th-features-name" } else { "perf-th-tabs-name" };
        let count_id: &'static str = if key == "features" { "perf-th-features-count" } else { "perf-th-tabs-count" };
        div()
            .pl(px(29.))
            .pt(px(2.))
            .pb(px(6.))
            .child(
                v_flex()
                    .w(px(250.))
                    .border_1()
                    .border_r_0()
                    .border_color(line)
                    .text_color(pal.text)
                    .child(h_flex().h(px(35.)).bg(gpui_kit::rgb(0xf5f5f5)).border_b_1().border_color(line).child(head_cell(name_id, head, 149., true, cx)).child(head_cell(count_id, "Count", 100., false, cx)))
                    .children(rows.into_iter().enumerate().map(|(i, (name, n))| {
                        let cell = |w: f32, text: String| div().w(px(w)).h(px(26.)).px(px(6.)).py(px(2.)).border_r_1().border_color(line).text_size(px(12.)).line_height(px(21.6)).child(text);
                        h_flex().when(i % 2 == 0, |d| d.bg(gpui_kit::rgba(0x00000005))).child(cell(149., name)).child(cell(100., n.to_string()))
                    })),
            )
            .into_any_element()
    }
}

impl Render for PerformancePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let snap = self.snap.clone();
        let mut sections: Vec<AnyElement> = Vec::new();
        if let Some(tab) = &snap.tab {
            let total: usize = tab.features.iter().map(|f| f.1).sum();
            let mut body = vec![
                h_flex()
                    .h(px(32.))
                    .pl(px(15.))
                    .gap(px(5.))
                    .items_center()
                    .child(div().text_size(px(13.33)).child(format!("Total regeneration time: {}", crate::ui::regen_time_text(tab.regen_ms))))
                    .child(
                        div()
                            .id("perf-regen-times")
                            .px(px(5.))
                            .cursor_pointer()
                            .child(crate::ui::glyph("regen-timer", 20., pal.text))
                            .tooltip(crate::ui::tooltip("Show regeneration times"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(ShowRegenTimes))),
                    )
                    .into_any_element(),
                self.sub_heading(Some("features"), format!("Features ({total})"), None, cx),
            ];
            if self.opened.contains("features") {
                body.push(self.table("features", "Feature", &tab.features, cx));
            }
            sections.push(self.section("tab", "part-studio", "Tab performance", None, body, cx));
            let prims: usize = tab.primitives.iter().map(|f| f.1).sum();
            let mut body = vec![self.sub_heading(Some("primitives"), format!("Primitives ({prims})"), None, cx)];
            if self.opened.contains("primitives") {
                body.push(self.table("primitives", "Primitive", &tab.primitives, cx));
            }
            sections.push(self.section("graphics", "perf-graphics", "Tab graphics", None, body, cx));
        }
        let tabs: usize = snap.tabs.iter().map(|t| t.1).sum();
        let body = vec![self.sub_heading(None, format!("Tabs ({tabs})"), None, cx), self.table("tabs", "Tab", &snap.tabs, cx)];
        sections.push(self.section("document", "document", "Document performance", None, body, cx));
        let software = snap.software_renderer();
        let (renderer, vendor) = match snap.adapter.as_deref().and_then(|a| a.rsplit_once(" (")) {
            Some((name, backend)) => (name.to_string(), backend.trim_end_matches(')').to_string()),
            None => (snap.adapter.clone().unwrap_or_else(|| "Starting…".into()), String::new()),
        };
        let platform = match std::env::consts::OS {
            "linux" => "Linux",
            "macos" => "Mac",
            "windows" => "Windows",
            other => other,
        };
        let mut body = vec![self.sub_heading(Some("browser"), "Browser and platform".into(), software.then_some("error"), cx)];
        if self.opened.contains("browser") {
            let row = |k: &'static str, v: String, bad: bool, cx: &App| {
                h_flex()
                    .items_start()
                    .pl(px(29.))
                    .text_size(px(13.33))
                    .line_height(px(20.))
                    .child(div().w(px(118.)).flex_none().font_weight(FontWeight::SEMIBOLD).child(k))
                    .child(div().flex_1().child(v))
                    .child(div().w(px(60.)).flex_none().flex().justify_center().pt(px(3.)).when(bad, |d| d.child(crate::assets::icon("error", 20., cx))))
                    .into_any_element()
            };
            body.push(
                v_flex()
                    .pb(px(5.))
                    .child(row("Browser", "noshape".into(), false, cx))
                    .child(row("Version", crate::VERSION.into(), false, cx))
                    .child(row("Platform", platform.into(), false, cx))
                    .child(row("GL vendor", vendor, false, cx))
                    .child(row("GL renderer", renderer, software, cx))
                    .child(row("Local storage", "Present".into(), false, cx))
                    .into_any_element(),
            );
        }
        body.push(self.sub_heading(Some("webgl"), "WebGL and extensions".into(), Some("check"), cx));
        if self.opened.contains("webgl") {
            body.push(
                v_flex()
                    .pb(px(5.))
                    .children(
                        ["Float textures", "Float buffer blending", "Float framebuffer rendering", "Anisotropic texture filtering", "32-bit indices", "High precision fragment shaders"].map(|k| {
                            h_flex()
                                .h(px(22.))
                                .pl(px(29.))
                                .text_size(px(13.33))
                                .child(div().flex_1().child(k))
                                .child(div().w(px(60.)).flex().justify_center().child(crate::assets::icon("check", 20., cx)))
                        }),
                    )
                    .into_any_element(),
            );
        }
        sections.push(self.section("system", "perf-system", "System", None, body, cx));
        v_flex()
            .size_full()
            .bg(panel_bg())
            .text_color(pal.text)
            .child(panel_header("Performance").child(header_icon("perf-close", "close", "Close").on_click(cx.listener(|_, _, _, cx| cx.emit(ClosePanel)))))
            .child(v_flex().id("perf-body").flex_1().min_h_0().overflow_y_scroll().pr(px(15.)).children(sections))
            .child(panel_footer("https://cad.onshape.com/help/Content/performance_panel.htm", cx))
    }
}
