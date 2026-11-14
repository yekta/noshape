//! A part's Where used… (Parts list menu; live2 11-14, feel/whereused.json, feel/whereused/*.png): a 380 px left panel
//! "Where used" ×: the part and a version selector, then "Results:" with Refresh and the version filter, one row per
//! assembly, drawing or Derived feature using it (another document's prefixed ↳), its version and state. Live lists
//! versions only and only for company documents; ours searches this document's workspace ("Main") too and the
//! documents on this computer (`noshape_ops::where_used`).

use super::PartStudioView;
use crate::os_menu::OsContextMenuExt as _;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::reference::BodyId;
use noshape_ops::where_used::Use;

/// Live's filter of the results.
pub const FILTERS: [&str; 4] = ["Latest revision", "All revisions", "Latest versions", "All versions"];

pub struct WhereUsedPanel {
    pub body: BodyId,
    pub part: String,
    /// The search's result (None while it runs).
    pub uses: Option<Vec<Use>>,
    /// The version shown ("Main" or a version's name) and the filter (an index of [`FILTERS`]).
    pub version: String,
    pub filter: usize,
    pub selected: Option<usize>,
    version_open: bool,
    filter_open: bool,
    _task: Option<Task<()>>,
}

impl WhereUsedPanel {
    /// The versions offered: Main, then each version a use pins.
    pub fn versions(&self) -> Vec<String> {
        let mut v = vec!["Main".to_string()];
        for u in self.uses.iter().flatten() {
            if !v.contains(&u.version) {
                v.push(u.version.clone());
            }
        }
        v
    }

    /// The uses of the version shown.
    pub fn shown(&self) -> Vec<&Use> {
        self.uses.iter().flatten().filter(|u| u.version == self.version).collect()
    }
}

impl PartStudioView {
    /// Parts list › Where used…: the panel, and the search run off the UI thread.
    pub(super) fn open_where_used(&mut self, body: BodyId, cx: &mut Context<Self>) {
        let part = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_default();
        self.ui.where_used = Some(WhereUsedPanel { body: body.clone(), part, uses: None, version: "Main".into(), filter: 3, selected: None, version_open: false, filter_open: false, _task: None });
        self.refresh_where_used(cx);
    }

    /// (Re)run the search (the panel's Refresh).
    fn refresh_where_used(&mut self, cx: &mut Context<Self>) {
        let Some(body) = self.ui.where_used.as_ref().map(|w| w.body.clone()) else { return };
        let (doc, name, path) = {
            let d = self.doc.read(cx);
            (d.session.doc.clone(), d.session.name(), d.session.path.clone())
        };
        let others = crate::part_studio_hooks::other_documents(&path);
        let tab = self.tab;
        let task = cx.spawn(async move |this, cx| {
            let uses = cx.background_spawn(async move { noshape_ops::where_used::where_used(&doc, &name, &path, tab, &body, &others) }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(w) = &mut this.ui.where_used {
                    w.uses = Some(uses);
                    w.selected = None;
                }
                cx.notify();
            });
        });
        if let Some(w) = &mut self.ui.where_used {
            w.uses = None;
            w._task = Some(task);
        }
        cx.notify();
    }

    pub(super) fn render_where_used(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let w = self.ui.where_used.as_ref()?;
        let pal = p();
        let (part, version, filter, selected) = (w.part.clone(), w.version.clone(), w.filter, w.selected);
        let (versions, version_open, filter_open) = (w.versions(), w.version_open, w.filter_open);
        let loading = w.uses.is_none();
        let rows: Vec<Use> = w.shown().into_iter().cloned().collect();
        let used_anywhere = w.uses.as_ref().is_some_and(|u| !u.is_empty());
        let me = self.doc.read(cx).session.path.clone();
        let menu_list = |id: &'static str, items: Vec<String>, current: usize, pick: fn(&mut PartStudioView, usize), cx: &mut Context<Self>| {
            v_flex().id(id).absolute().top(px(26.)).left_0().min_w(px(160.)).py(px(4.)).bg(gpui_kit::rgb(0xffffff)).border_1().border_color(pal.border).shadow_md().children(items.into_iter().enumerate().map(|(i, t)| {
                div()
                    .id(SharedString::from(format!("{id}-{i}")))
                    .px(px(10.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .when(i == current, |d| d.font_weight(FontWeight::MEDIUM))
                    .hover(|d| d.bg(pal.panel_hover))
                    .cursor_pointer()
                    .child(t)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        pick(this, i);
                        cx.notify();
                    }))
            }))
        };
        let mut body = v_flex().flex_1().min_h_0().px(px(10.)).pt(px(8.)).gap(px(6.));
        // The part and the version selector.
        let current = versions.iter().position(|v| *v == version).unwrap_or(0);
        body = body.child(
            h_flex()
                .gap(px(8.))
                .child(crate::assets::icon("part", 16., cx))
                .child(div().flex_1().truncate().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(part.clone()))
                .child(
                    div()
                        .id("wu-version")
                        .relative()
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .border_1()
                        .border_color(pal.border)
                        .text_size(px(12.))
                        .cursor_pointer()
                        .child(version.clone())
                        .child(crate::ui::glyph("ui-select-caret", 8., gpui_kit::rgb(0x777777)))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(w) = &mut this.ui.where_used {
                                w.version_open = !w.version_open;
                            }
                            cx.notify();
                        }))
                        .when(version_open, |d| {
                            d.child(menu_list("wu-versions", versions.clone(), current, |this, i| {
                                if let Some(w) = &mut this.ui.where_used {
                                    w.version = w.versions().get(i).cloned().unwrap_or_else(|| "Main".into());
                                    w.version_open = false;
                                    w.selected = None;
                                }
                            }, cx))
                        }),
                ),
        );
        if loading {
            body = body.child(div().text_size(px(12.)).text_color(pal.text_muted).child("Searching…"));
        } else if !used_anywhere {
            // Live: the part used nowhere.
            body = body.child(div().text_size(px(12.)).text_color(pal.text_muted).child(format!("{part} is not used in any versions.")));
        } else {
            body = body.child(
                h_flex()
                    .gap(px(6.))
                    .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child("Results:"))
                    .child(div().id("wu-refresh").cursor_pointer().child(crate::ui::glyph("refresh", 14., pal.text_muted)).tooltip(crate::ui::tooltip("Refresh")).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh_where_used(cx))))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("wu-filter")
                            .relative()
                            .h(px(24.))
                            .px(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(12.))
                            .cursor_pointer()
                            .child(FILTERS[filter])
                            .child(crate::ui::glyph("ui-select-caret", 8., gpui_kit::rgb(0x777777)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(w) = &mut this.ui.where_used {
                                    w.filter_open = !w.filter_open;
                                }
                                cx.notify();
                            }))
                            .when(filter_open, |d| {
                                d.child(menu_list("wu-filters", FILTERS.map(String::from).to_vec(), filter, |this, i| {
                                    if let Some(w) = &mut this.ui.where_used {
                                        w.filter = i;
                                        w.filter_open = false;
                                    }
                                }, cx))
                            }),
                    ),
            );
            for (i, u) in rows.iter().enumerate() {
                let other = noshape_ops::doc_library::canonical(&u.path) != noshape_ops::doc_library::canonical(&me);
                let icon = match u.kind {
                    "assembly" => "tab-assembly",
                    "drawing" => "tab-drawing",
                    _ => "tab-part-studio",
                };
                let tip = format!("{} -> {}", u.document, u.element);
                let open = u.path.clone();
                body = body.child(
                    h_flex()
                        .id(SharedString::from(format!("wu-row-{i}")))
                        .h(px(28.))
                        .px(px(4.))
                        .gap(px(6.))
                        .text_size(px(12.))
                        .when(selected == Some(i), |d| d.bg(pal.accent_soft))
                        .hover(|d| d.bg(pal.panel_hover))
                        .cursor_pointer()
                        .when(other, |d| d.child(div().text_color(pal.text_muted).child("↳")))
                        .child(crate::assets::icon(icon, 16., cx))
                        .child(div().flex_1().truncate().child(u.element.clone()))
                        .child(div().text_color(pal.text_muted).child(u.version.clone()))
                        .child(div().px(px(6.)).rounded(px(8.)).bg(gpui_kit::rgb(0xe8e8e8)).text_size(px(11.)).child("In progress"))
                        .tooltip(crate::ui::tooltip(tip))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(w) = &mut this.ui.where_used {
                                w.selected = Some(i);
                            }
                            cx.notify();
                        }))
                        .os_context_menu({
                            let view = cx.entity();
                            move |menu, _, _| {
                                let (v, path) = (view.clone(), open.clone());
                                menu.item(crate::os_menu::OsMenuItem::new("Open in new browser tab").on_click(move |_, _, cx| {
                                    let gpu = v.read(cx).gpu.clone();
                                    let _ = crate::part_studio_hooks::open_document_window(path.clone(), gpu, cx);
                                }))
                            }
                        }),
                );
            }
        }
        Some(
            v_flex()
                .id("where-used")
                .debug_selector(|| "where-used".into())
                .occlude()
                .absolute()
                .left(px(40.))
                .top_0()
                .bottom_0()
                .w(px(380.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(pal.panel)
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().flex_1().pl(px(8.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Where used"))
                        .child(
                            div()
                                .id("wu-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::ui::glyph("close", 12., pal.text_muted))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.where_used = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(body)
                .into_any_element(),
        )
    }
}
