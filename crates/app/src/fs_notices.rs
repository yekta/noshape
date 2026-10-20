//! Onshape's "FeatureScript notices" panel (live document/73–75,
//! dom/featurescript-notices-panel.txt): opened from the navbar `{ }` icon
//! (red `{!}` while a Feature Studio has errors), a bottom panel under the
//! tab bar: title 18 px / 500, filter button + "Filter notices", ×; then a
//! section per Feature Studio with notices (✕ message / "line:column  Feature
//! Studio 1"), and one per Part Studio and Variable Studio with Monitor ▾
//! (the monitored one shows its "Result: …" console line).

use crate::doc_model::{DocEvent, DocModel};
use crate::feature_studio::{Notice, studio_notices};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::custom::Severity;
use noshape_model::document::TabId;
use std::collections::{HashMap, HashSet};

/// Icon of a notice: red ☒ error, ⚠ warning, ⓘ info.
pub(crate) fn notice_icon(error: bool, info: bool) -> &'static str {
    if error {
        "fs-error"
    } else if info {
        "fs-info"
    } else {
        "msg-warning"
    }
}

pub enum FsNoticesEvent {
    Goto(TabId, usize, usize),
    Close,
}

/// The studio each document monitors (shared by the notices panel and the
/// Feature Studio toolbar's Monitor ▾).
#[derive(Default)]
struct Monitored(HashMap<EntityId, TabId>);

impl Global for Monitored {}

pub fn monitored(doc: &Entity<DocModel>, cx: &App) -> Option<TabId> {
    let id = doc.entity_id();
    let pick = cx.try_global::<Monitored>().and_then(|m| m.0.get(&id).copied());
    let d = &doc.read(cx).session.doc;
    pick.filter(|t| d.tab(*t).is_some()).or_else(|| d.ordered_tabs().iter().find(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| t.id))
}

pub fn set_monitored(doc: &Entity<DocModel>, tab: TabId, cx: &mut App) {
    let id = doc.entity_id();
    cx.default_global::<Monitored>().0.insert(id, tab);
    cx.refresh_windows();
}

/// Notices of every Feature Studio's committed source (the compile hook).
pub fn document_notices(doc: &Entity<DocModel>, cx: &App) -> Vec<(TabId, String, Vec<Notice>)> {
    doc.read(cx)
        .session
        .doc
        .ordered_tabs()
        .iter()
        .filter_map(|t| match &t.content {
            TabContent::FeatureStudio(f) => Some((t.id, t.name.clone(), studio_notices(t.id, &f.source))),
            _ => None,
        })
        .filter(|(_, _, n)| !n.is_empty())
        .collect()
}

pub struct FsNoticesPanel {
    doc: Entity<DocModel>,
    filter: Entity<InputState>,
    notices: Vec<(TabId, String, Vec<Notice>)>,
    collapsed: HashSet<TabId>,
    /// "Result: …" lines of the Part Studios (from the workspace's views).
    results: HashMap<TabId, String>,
    /// `custom::run::output_generation` when the Monitor was last refreshed.
    output_generation: u64,
    _subs: Vec<Subscription>,
}

impl EventEmitter<FsNoticesEvent> for FsNoticesPanel {}

impl FsNoticesPanel {
    pub fn new(doc: Entity<DocModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter notices"));
        let subs = vec![
            cx.subscribe_in(&filter, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe_in(&doc, window, |this: &mut Self, _, _: &DocEvent, _, cx| this.refresh(cx)),
        ];
        let notices = document_notices(&doc, cx);
        Self { doc, filter, notices, collapsed: HashSet::new(), results: HashMap::new(), output_generation: 0, _subs: subs }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.notices = document_notices(&self.doc, cx);
        cx.notify();
    }

    pub fn set_results(&mut self, results: HashMap<TabId, String>, cx: &mut Context<Self>) {
        let generation = noshape_model::custom::run::output_generation();
        if results != self.results || generation != self.output_generation {
            self.results = results;
            self.output_generation = generation;
            cx.notify();
        }
    }

    /// A Monitor entry: the notice, then its stack rows (`line:col`, place).
    fn monitor_item(&self, id: String, e: noshape_model::custom::run::MonitorEntry, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut item = v_flex()
            .id(SharedString::from(id))
            .cursor_pointer()
            .hover(|d| d.bg(rgb(0xf5f5f5)))
            .child(
                h_flex()
                    .h(px(22.))
                    .child(div().w(px(38.)).flex_none().pl(px(23.)).child(os::glyph(os::GLYPH_CHEVRON_DOWN, 10., pal.text)))
                    .child(div().w(px(28.)).flex_none().pl(px(5.)).child(crate::assets::icon(notice_icon(e.severity == Severity::Error, e.severity == Severity::Info), 16., cx)))
                    .child(div().flex_1().min_w_0().truncate().px(px(5.)).text_size(px(14.)).text_color(pal.text).child(e.message.clone())),
            );
        for (pos, place) in e.rows {
            item = item.child(
                h_flex()
                    .h(px(22.))
                    .pl(px(66.))
                    .child(div().min_w(px(45.)).flex_none().px(px(5.)).text_size(px(14.)).text_color(rgb(0x888888)).child(pos))
                    .child(div().px(px(5.)).text_size(px(14.67)).font_weight(FontWeight::LIGHT).text_color(pal.text).child(place)),
            );
        }
        if let Some((t, line, col)) = e.at {
            item = item.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(FsNoticesEvent::Goto(t, line, col))));
        }
        item.into_any_element()
    }

    fn header(&self, id: TabId, icon: &'static str, name: String, gray: bool, monitor: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = !self.collapsed.contains(&id);
        let text = rgb(if gray { 0xaaaaaa } else { 0x666666 });
        let doc = self.doc.clone();
        h_flex()
            .id(SharedString::from(format!("fsn-h-{id}")))
            .h(px(41.))
            .px(px(4.))
            .gap(px(6.))
            .cursor_pointer()
            .child(div().opacity(if gray { 0.5 } else { 1. }).child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 12., text)))
            .child(div().opacity(if gray { 0.5 } else { 1. }).child(crate::assets::icon(icon, 18., cx)))
            .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::MEDIUM).text_color(text).child(name))
            .when(monitor, |d| {
                d.child(
                    h_flex()
                        .id(SharedString::from(format!("fsn-mon-{id}")))
                        .h(px(30.))
                        .px(px(5.))
                        .gap(px(5.))
                        .rounded(px(3.))
                        .hover(|d| d.bg(pal.border))
                        .child(os::glyph(os::GLYPH_PULSE, 20., rgb(0x666666)))
                        .child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x666666)).child("Monitor"))
                        .child(os::glyph(os::GLYPH_CARET, 8., rgb(0x1651b0)))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            set_monitored(&doc, id, cx)
                        }),
                )
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.collapsed.remove(&id) {
                    this.collapsed.insert(id);
                }
                cx.notify();
            }))
            .into_any_element()
    }
}

impl Render for FsNoticesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let q = self.filter.read(cx).value().trim().to_lowercase();
        let watched = monitored(&self.doc, cx);
        let mut sections: Vec<AnyElement> = Vec::new();
        for (tab, name, notices) in self.notices.clone() {
            let shown: Vec<Notice> = notices.into_iter().filter(|n| q.is_empty() || n.message.to_lowercase().contains(&q)).collect();
            if shown.is_empty() {
                continue;
            }
            let mut body = v_flex();
            if !self.collapsed.contains(&tab) {
                for (i, n) in shown.into_iter().enumerate() {
                    let (line, col) = (n.line, n.column);
                    body = body.child(
                        v_flex()
                            .id(SharedString::from(format!("fsn-{tab}-{i}")))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(0xf5f5f5)))
                            .child(
                                h_flex()
                                    .h(px(22.))
                                    .child(div().w(px(38.)).flex_none().pl(px(23.)).child(os::glyph(os::GLYPH_CHEVRON_DOWN, 10., pal.text)))
                                    .child(div().w(px(28.)).flex_none().pl(px(5.)).child(crate::assets::icon(notice_icon(n.error, n.info), 16., cx)))
                                    .child(div().flex_1().min_w_0().truncate().px(px(5.)).text_size(px(14.)).text_color(pal.text).child(n.message.clone())),
                            )
                            .child(
                                h_flex()
                                    .h(px(22.))
                                    .pl(px(66.))
                                    .child(div().w(px(45.)).flex_none().px(px(5.)).text_size(px(14.)).text_color(rgb(0x888888)).child(format!("{line}:{col}")))
                                    .child(div().px(px(5.)).text_size(px(14.67)).font_weight(FontWeight::LIGHT).text_color(pal.text).child(name.clone())),
                            )
                            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(FsNoticesEvent::Goto(tab, line, col)))),
                    );
                }
            }
            let head = self.header(tab, "feature-studio", name.clone(), false, false, cx);
            sections.push(v_flex().border_b_1().border_color(rgb(0xeaeaea)).child(head).child(body).into_any_element());
        }
        // Custom tables that threw (live 652): "<table> (<Part Studio>)"
        // with the error, its stack and the table's println output.
        for (studio, g) in crate::custom_tables::notice_groups(self.doc.entity_id(), cx) {
            let key = TabId(studio.0 | 1 << 62);
            let head = self.header(key, "featurescript-table", g.title.clone(), false, false, cx);
            let mut body = v_flex();
            if !self.collapsed.contains(&key) {
                for (i, e) in g.entries.into_iter().filter(|e| q.is_empty() || e.message.to_lowercase().contains(&q)).enumerate() {
                    body = body.child(self.monitor_item(format!("fst-{studio}-{i}"), e, cx));
                }
                if !g.output.is_empty() {
                    body = body.child(v_flex().border_t_1().border_color(rgb(0xeaeaea)).pl(px(24.)).pr(px(8.)).py(px(8.)).font_family("monospace").text_size(px(13.33)).children(g.output.lines().map(|l| div().whitespace_nowrap().text_color(pal.text).child(l.to_string())).collect::<Vec<_>>()));
                }
            }
            sections.push(v_flex().border_b_1().border_color(rgb(0xeaeaea)).child(head).child(body).into_any_element());
        }
        let studios: Vec<(TabId, String, bool)> = self.doc.read(cx).session.doc.ordered_tabs().iter().filter_map(|t| match t.content {
            TabContent::PartStudio(_) => Some((t.id, t.name.clone(), true)),
            TabContent::VariableStudio(_) => Some((t.id, t.name.clone(), false)),
            _ => None,
        }).collect();
        for (tab, name, part_studio) in studios {
            let is_watched = watched == Some(tab);
            let head = self.header(tab, if part_studio { "part-studio" } else { "variable-studio" }, name, true, true, cx);
            let open = is_watched && !self.collapsed.contains(&tab);
            // Monitor (live dom/fs-probe-output.txt): the custom features'
            // notices with their stacks, their printed output, then the
            // "Result:" line.
            let (entries, printed) = if open && part_studio { noshape_model::custom::run::monitor(&self.doc.read(cx).session.doc, tab) } else { Default::default() };
            let entries: Vec<_> = entries.into_iter().filter(|e| q.is_empty() || e.message.to_lowercase().contains(&q)).collect();
            let mut body = v_flex();
            for (i, e) in entries.into_iter().enumerate() {
                body = body.child(self.monitor_item(format!("fsm-{tab}-{i}"), e, cx));
            }
            let result = open.then(|| {
                let text = self.results.get(&tab).cloned().unwrap_or_else(|| "Result: Regeneration complete".into());
                v_flex()
                    .border_t_1()
                    .border_color(rgb(0xeaeaea))
                    .pl(px(24.))
                    .pr(px(8.))
                    .py(px(8.))
                    .font_family("monospace")
                    .text_size(px(13.33))
                    .children(printed.lines().map(|l| div().whitespace_nowrap().text_color(pal.text).child(l.to_string())).collect::<Vec<_>>())
                    .child(div().text_color(rgb(0xaaaaaa)).child(text))
            });
            sections.push(v_flex().border_b_1().border_color(rgb(0xeaeaea)).child(head).child(body).children(result).into_any_element());
        }
        v_flex()
            .h(px(524.))
            .flex_none()
            .bg(rgb(0xffffff))
            .border_t_1()
            .border_color(rgb(0xeaeaea))
            .child(
                h_flex()
                    .h(px(41.))
                    .flex_none()
                    .pl(px(4.))
                    .pr(px(8.))
                    .gap(px(6.))
                    .border_b_1()
                    .border_color(rgb(0xeaeaea))
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("FeatureScript notices"))
                    .child(div().id("fsn-filter").size(px(30.)).flex().items_center().justify_center().rounded(px(3.)).tooltip(crate::ui::tooltip("Filter FeatureScript notices")).child(crate::ui::glyph("filter", 20., pal.text)))
                    .child(div().w(px(902.)).child(Input::new(&self.filter).h(px(30.)).text_size(px(14.)).pl(px(5.))))
                    .child(div().flex_1())
                    .child(div().id("fsn-close").cursor_pointer().child(os::glyph(os::GLYPH_CLOSE, 16., pal.text_muted)).on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(FsNoticesEvent::Close)))),
            )
            .child(v_flex().id("fsn-list").flex_1().min_h_0().overflow_y_scroll().children(sections))
    }
}
