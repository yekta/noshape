//! Document window: title bar, active tab content and the bottom tab bar.

use crate::actions::*;
use crate::assets::icon_path;
use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::part_studio::PartStudioView;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Disableable as _, Icon, IconName, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::assembly::Assembly;
use noshape_model::document::{TabContent, TabId};
use noshape_model::drawing::Drawing;
use noshape_model::partstudio::PartStudio;
use noshape_model::variables::VariableStudio;
use std::collections::HashMap;

enum TabView {
    PartStudio(Entity<PartStudioView>),
    Placeholder(&'static str),
}

pub struct Workspace {
    pub doc: Entity<DocModel>,
    gpu: Gpu,
    active: Option<TabId>,
    views: HashMap<TabId, TabView>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Workspace {
    pub fn new(doc: Entity<DocModel>, gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| match ev {
            DocEvent::TabsChanged | DocEvent::Reloaded => {
                this.sync_tabs(window, cx);
                cx.notify();
            }
            _ => cx.notify(),
        })];
        let active = doc.read(cx).session.doc.ordered_tabs().first().map(|t| t.id);
        let mut this = Self { doc, gpu, active, views: HashMap::new(), focus: cx.focus_handle(), _subs: subs };
        this.sync_tabs(window, cx);
        this
    }

    fn sync_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tabs: Vec<(TabId, &'static str)> = self.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| (t.id, kind_of(&t.content))).collect();
        self.views.retain(|id, _| tabs.iter().any(|(t, _)| t == id));
        if self.active.is_none_or(|a| !tabs.iter().any(|(t, _)| *t == a)) {
            self.active = tabs.first().map(|t| t.0);
        }
        // Views are created lazily for the active tab.
        if let Some(active) = self.active {
            self.ensure_view(active, window, cx);
        }
    }

    fn ensure_view(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        if self.views.contains_key(&tab) {
            return;
        }
        let kind = self.doc.read(cx).session.doc.tab(tab).map(|t| kind_of(&t.content));
        let view = match kind {
            Some("partstudio") => {
                let doc = self.doc.clone();
                let gpu = self.gpu.clone();
                TabView::PartStudio(cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx)))
            }
            Some("assembly") => TabView::Placeholder("Assembly"),
            Some("drawing") => TabView::Placeholder("Drawing"),
            Some("variables") => TabView::Placeholder("Variable Studio"),
            _ => return,
        };
        self.views.insert(tab, view);
    }

    fn activate(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.active = Some(tab);
        self.ensure_view(tab, window, cx);
        if let Some(TabView::PartStudio(v)) = self.views.get(&tab) {
            let fh = v.read(cx).focus_handle(cx);
            window.focus(&fh, cx);
        }
        cx.notify();
    }

    fn add_tab(&mut self, kind: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let mut new_id = None;
        self.doc.update(cx, |doc, cx| {
            let (base, content) = match kind {
                "partstudio" => ("Part Studio", TabContent::PartStudio(PartStudio::new())),
                "assembly" => ("Assembly", TabContent::Assembly(Assembly::default())),
                "drawing" => ("Drawing", TabContent::Drawing(Drawing::default())),
                _ => ("Variable Studio", TabContent::VariableStudio(VariableStudio::default())),
            };
            doc.edit_document(&format!("Insert tab : {base}"), cx, |d| {
                let name = d.next_tab_name(base);
                new_id = Some(d.add_tab(&name, content));
            });
        });
        if let Some(id) = new_id {
            self.activate(id, window, cx);
        }
    }

    fn delete_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            let name = doc.session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
            if doc.session.doc.tabs.len() > 1 {
                doc.edit_document(&format!("Delete tab : {name}"), cx, |d| {
                    d.remove_tab(tab);
                });
            }
        });
    }

    fn duplicate_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            let Some(t) = doc.session.doc.tab(tab).cloned() else { return };
            doc.edit_document(&format!("Duplicate tab : {}", t.name), cx, |d| {
                let name = format!("{} (copy)", t.name);
                d.add_tab(&name, t.content);
            });
        });
    }

    fn render_title_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let doc = self.doc.read(cx);
        let name = doc.name();
        let can_undo = doc.session.can_undo();
        let can_redo = doc.session.can_redo();
        let undo_tip: SharedString = doc.session.undo_description().map(|d| format!("Undo {d}")).unwrap_or_else(|| "Undo".into()).into();
        let redo_tip: SharedString = doc.session.redo_description().map(|d| format!("Redo {d}")).unwrap_or_else(|| "Redo".into()).into();
        let read_only = doc.session.read_only();
        TitleBar::new().child(
            h_flex()
                .w_full()
                .pr_2()
                .gap_2()
                .child(
                    h_flex()
                        .gap_1p5()
                        .child(div().size(px(14.)).rounded_sm().bg(pal.accent))
                        .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(pal.text).child("noshape")),
                )
                .child(div().w(px(1.)).h(px(16.)).bg(pal.border))
                .child(div().text_sm().text_color(pal.text_muted).child(name))
                .when(read_only, |d| d.child(div().px_2().rounded_sm().bg(pal.warning).text_xs().text_color(rgb(0x000000)).child("VIEWING HISTORY")))
                .child(div().flex_1())
                .child(
                    Button::new("undo")
                        .icon(IconName::Undo)
                        .ghost()
                        .small()
                        .disabled(!can_undo)
                        .tooltip(undo_tip)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.doc.update(cx, |d, cx| d.undo(cx)))),
                )
                .child(
                    Button::new("redo")
                        .icon(IconName::Redo)
                        .ghost()
                        .small()
                        .disabled(!can_redo)
                        .tooltip(redo_tip)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.doc.update(cx, |d, cx| d.redo(cx)))),
                )
                .child(Button::new("history").icon(Icon::empty().path(icon_path("version-graph"))).ghost().small().tooltip("Versions and history")),
        )
        .into_any_element()
    }

    fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let tabs: Vec<(TabId, String, &'static str)> = self.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| (t.id, t.name.clone(), kind_of(&t.content))).collect();
        let view = cx.entity();
        let add = Button::new("add-tab").icon(IconName::Plus).ghost().small().tooltip("Insert new tab").dropdown_menu(move |menu, _, _| {
            let v1 = view.clone();
            let v2 = view.clone();
            let v3 = view.clone();
            let v4 = view.clone();
            menu.item(PopupMenuItem::new("Part Studio").icon(Icon::empty().path(icon_path("part-studio"))).on_click(move |_, w, cx| v1.update(cx, |t, cx| t.add_tab("partstudio", w, cx))))
                .item(PopupMenuItem::new("Assembly").icon(Icon::empty().path(icon_path("assembly"))).on_click(move |_, w, cx| v2.update(cx, |t, cx| t.add_tab("assembly", w, cx))))
                .item(PopupMenuItem::new("Drawing").icon(Icon::empty().path(icon_path("drawing"))).on_click(move |_, w, cx| v3.update(cx, |t, cx| t.add_tab("drawing", w, cx))))
                .item(PopupMenuItem::new("Variable Studio").icon(Icon::empty().path(icon_path("variable-studio"))).on_click(move |_, w, cx| v4.update(cx, |t, cx| t.add_tab("variables", w, cx))))
        });
        let mut bar = h_flex().h(px(32.)).px_1().gap_0p5().bg(pal.panel).border_t_1().border_color(pal.border).child(add);
        for (id, name, kind) in tabs {
            let active = self.active == Some(id);
            let icon = match kind {
                "partstudio" => "part-studio",
                "assembly" => "assembly",
                "drawing" => "drawing",
                _ => "variable-studio",
            };
            let view = cx.entity();
            bar = bar.child(
                h_flex()
                    .id(SharedString::from(format!("tab-{id}")))
                    .h(px(26.))
                    .px_3()
                    .gap_1p5()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .when(active, |d| d.bg(pal.panel_active).text_color(pal.text))
                    .when(!active, |d| d.text_color(pal.text_muted).hover(|d| d.bg(pal.panel_hover)))
                    .child(svg().path(icon_path(icon)).size_3p5().text_color(if active { pal.accent } else { pal.text_faint }))
                    .child(name)
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate(id, window, cx)))
                    .context_menu(move |menu, _, _| {
                        let v1 = view.clone();
                        let v2 = view.clone();
                        menu.item(PopupMenuItem::new("Duplicate").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.duplicate_tab(id, cx))))
                            .separator()
                            .item(PopupMenuItem::new("Delete").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.delete_tab(id, cx))))
                    }),
            );
        }
        bar
        .into_any_element()
    }
}

fn kind_of(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "partstudio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::VariableStudio(_) => "variables",
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let title = self.render_title_bar(cx);
        let tabs = self.render_tab_bar(cx);
        let content: AnyElement = match self.active.and_then(|a| self.views.get(&a)) {
            Some(TabView::PartStudio(v)) => v.clone().into_any_element(),
            Some(TabView::Placeholder(kind)) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(pal.text_faint)
                .child(format!("{kind} tab"))
                .into_any_element(),
            None => div().into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(pal.app_bg)
            .text_color(pal.text)
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.doc.update(cx, |d, cx| d.undo(cx))))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.doc.update(cx, |d, cx| d.redo(cx))))
            .child(title)
            .child(div().flex_1().min_h_0().child(content))
            .child(tabs)
    }
}
