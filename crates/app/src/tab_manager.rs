//! Onshape's Tab manager (Alt+T): a 380 px panel docked left of the
//! document with a tab search, type filters, one 54 px row per tab (icon,
//! name, italic type), a large preview of the selected tab and the
//! keyboard hint at the bottom. ↑/↓ (or Tab) move the selection, Enter
//! opens it, Esc closes the panel.

use crate::doc_model::{DocEvent, DocModel};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use std::collections::HashSet;

pub enum TabManagerEvent {
    Open(TabId),
    Close,
}

impl EventEmitter<TabManagerEvent> for TabManager {}

/// Tab kinds in Onshape's filter order: (key, icon, type label).
const KINDS: [(&str, &str, &str); 5] = [
    ("partstudio", "part-studio", "Part Studio"),
    ("assembly", "assembly", "Assembly"),
    ("drawing", "drawing", "Drawing"),
    ("variables", "variable-studio", "Variable Studio"),
    ("featurestudio", "feature-studio", "Feature Studio"),
];

fn kind_of(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "partstudio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::VariableStudio(_) => "variables",
        TabContent::FeatureStudio(_) => "featurestudio",
    }
}

fn kind_info(kind: &str) -> (&'static str, &'static str) {
    KINDS.iter().find(|k| k.0 == kind).map(|k| (k.1, k.2)).unwrap_or(("part-studio", "Part Studio"))
}

pub struct TabManager {
    doc: Entity<DocModel>,
    search: Entity<InputState>,
    query: String,
    /// Type filters (empty = all).
    kinds: HashSet<&'static str>,
    selected: Option<TabId>,
    _subs: Vec<Subscription>,
}

impl TabManager {
    pub fn new(doc: Entity<DocModel>, active: Option<TabId>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tabs"));
        let subs = vec![
            cx.subscribe_in(&search, window, |this: &mut Self, st, ev: &InputEvent, _, cx| match ev {
                InputEvent::Change => {
                    this.query = st.read(cx).value().to_string();
                    let m = this.matches(cx);
                    if this.selected.is_none_or(|s| !m.iter().any(|t| t.0 == s)) {
                        this.selected = m.first().map(|t| t.0);
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(t) = this.selected {
                        cx.emit(TabManagerEvent::Open(t));
                    }
                }
                _ => {}
            }),
            cx.subscribe_in(&doc, window, |_: &mut Self, _, _: &DocEvent, _, cx| cx.notify()),
        ];
        search.update(cx, |s, cx| s.focus(window, cx));
        Self { doc, search, query: String::new(), kinds: HashSet::new(), selected: active, _subs: subs }
    }

    /// The workspace's active tab changed (keep it selected, like Onshape).
    pub fn set_active(&mut self, tab: Option<TabId>, cx: &mut Context<Self>) {
        self.selected = tab;
        cx.notify();
    }

    fn matches(&self, cx: &App) -> Vec<(TabId, String, &'static str)> {
        let q = self.query.trim().to_lowercase();
        self.doc
            .read(cx)
            .session
            .doc
            .ordered_tabs()
            .iter()
            .map(|t| (t.id, t.name.clone(), kind_of(&t.content)))
            .filter(|(_, n, k)| (q.is_empty() || n.to_lowercase().contains(&q)) && (self.kinds.is_empty() || self.kinds.contains(k)))
            .collect()
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let m = self.matches(cx);
        if m.is_empty() {
            return;
        }
        let i = self.selected.and_then(|s| m.iter().position(|t| t.0 == s)).map(|i| i as isize + delta).unwrap_or(0);
        self.selected = Some(m[i.rem_euclid(m.len() as isize) as usize].0);
        cx.notify();
    }
}

impl Render for TabManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let matches = self.matches(cx);
        let present: Vec<&'static str> = {
            let tabs = self.doc.read(cx).session.doc.ordered_tabs();
            KINDS.iter().map(|k| k.0).filter(|k| tabs.iter().any(|t| kind_of(&t.content) == *k)).collect()
        };
        let rows: Vec<AnyElement> = matches
            .iter()
            .map(|(id, name, kind)| {
                let (icon, label) = kind_info(kind);
                let sel = self.selected == Some(*id);
                let id = *id;
                h_flex()
                    .id(SharedString::from(format!("tm-{id}")))
                    .relative()
                    .h(px(54.))
                    .flex_none()
                    .pl(px(22.))
                    .cursor_pointer()
                    .when(sel, |d| d.bg(pal.accent_soft).border_1().border_color(pal.accent).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().top_0().bottom_0().w(px(6.)).bg(pal.accent)))
                    .when(!sel, |d| d.hover(|d| d.bg(pal.panel_hover)))
                    .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(crate::assets::icon(icon, 30., cx)))
                    .child(
                        v_flex()
                            .pl(px(7.))
                            .flex_1()
                            .min_w_0()
                            .child(h_flex().gap(px(4.)).text_size(px(14.)).text_color(pal.text).child(crate::assets::icon(icon, 20., cx)).child(div().truncate().child(name.clone())))
                            .child(div().text_size(px(12.6)).italic().text_color(pal.text_muted).child(label)),
                    )
                    .tooltip(crate::ui::tooltip(format!("Click to navigate to {name}")))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected = Some(id);
                        cx.emit(TabManagerEvent::Open(id));
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        let filters = present.iter().map(|k| {
            let (icon, label) = kind_info(k);
            let k: &'static str = k;
            let on = self.kinds.contains(k);
            crate::ui::icon_button(SharedString::from(format!("tm-kind-{k}")), icon, 30., on, cx).tooltip(label).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.kinds.remove(k) {
                    this.kinds.insert(k);
                }
                cx.notify();
            }))
        });
        let preview = self.selected.and_then(|s| matches.iter().find(|t| t.0 == s)).map(|(_, name, kind)| {
            let (icon, _) = kind_info(kind);
            v_flex()
                .h(px(214.))
                .flex_none()
                .bg(rgb(0xf5f5f5))
                .px(px(9.))
                .pt(px(12.))
                .child(h_flex().gap(px(4.)).text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(crate::assets::icon(icon, 20., cx)).child(div().truncate().child(name.clone())))
                .child(div().flex_1().flex().items_center().justify_center().child(crate::assets::icon(icon, 130., cx)))
        });
        let key = |t: &'static str| div().px(px(3.)).border_1().border_color(rgb(0xaaaaaa)).rounded(px(2.)).bg(rgb(0xffffff)).child(t);
        let any_filter = !self.kinds.is_empty();
        v_flex()
            .id("tab-manager")
            .size_full()
            .bg(pal.panel)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| match ev.keystroke.key.as_str() {
                "escape" => cx.emit(TabManagerEvent::Close),
                "down" => this.step(1, cx),
                "up" => this.step(-1, cx),
                "tab" => this.step(if ev.keystroke.modifiers.shift { -1 } else { 1 }, cx),
                _ => {}
            }))
            .child(
                h_flex()
                    .h(px(37.))
                    .flex_none()
                    .pl(px(5.))
                    .pr(px(6.))
                    .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Tabs"))
                    .child(crate::ui::icon_button("tm-search", "search", 30., true, cx).tooltip("Search tabs"))
                    .child(crate::ui::icon_button("tm-close", "close", 30., false, cx).tooltip("Close").on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(TabManagerEvent::Close)))),
            )
            .child(div().px(px(6.)).pt(px(2.)).flex_none().child(div().h(px(30.)).border_1().border_color(rgb(0xaaaaaa)).rounded(px(4.)).child(Input::new(&self.search).appearance(false).text_size(px(14.)).italic())))
            .child(
                h_flex()
                    .px(px(6.))
                    .py(px(4.))
                    .gap(px(4.))
                    .flex_none()
                    .border_b_1()
                    .border_color(pal.border)
                    .children(filters)
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("tm-clear")
                            .px(px(7.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(pal.border)
                            .text_size(px(12.))
                            .text_color(if any_filter { pal.text } else { pal.text_faint })
                            .when(any_filter, |d| d.cursor_pointer().hover(|d| d.bg(pal.panel_hover)))
                            .child("Clear")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.kinds.clear();
                                cx.notify();
                            })),
                    ),
            )
            .child(v_flex().id("tm-rows").flex_1().min_h_0().overflow_y_scroll().children(rows))
            .children(preview)
            .child(
                h_flex()
                    .h(px(30.))
                    .flex_none()
                    .px(px(6.))
                    .gap(px(3.))
                    .border_t_1()
                    .border_color(pal.border)
                    .text_size(px(11.))
                    .text_color(pal.text_muted)
                    .child(key("tab"))
                    .child("or")
                    .child(key("↑"))
                    .child(key("↓"))
                    .child("to navigate")
                    .child(div().w(px(10.)))
                    .child(key("↵"))
                    .child("to open"),
            )
    }
}
