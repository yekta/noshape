//! Drawing editor: Sheets panel › reference row › Change configuration...
//! (live behaviours S6): a panel docked right of the Sheets panel with the
//! document, its workspace, the Part Studio and one row per configuration
//! input; ✓ shows the chosen configuration in every view of the reference,
//! on every sheet (one edit).

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::configuration::{ConfigInputKind, ConfigValues};
use noshape_model::document::{TabContent, TabId};

/// The Change configuration panel's state.
pub struct ChangeConfigDlg {
    pub tab: TabId,
    pub part: Option<noshape_model::BodyId>,
    pub values: ConfigValues,
    /// Cut list table › Change configuration...: the table configured (only
    /// it changes; live request 1638 (b)); None: the reference's views.
    pub table: Option<u64>,
    /// Variable / text inputs: (input id, field).
    fields: Vec<(String, Entity<InputState>)>,
}

impl DrawingEditor {
    /// The reference's configuration: its first base view's.
    fn reference_config(&self, tab: TabId, part: &Option<noshape_model::BodyId>) -> ConfigValues {
        self.drawing
            .sheets
            .iter()
            .flat_map(|s| s.views.iter())
            .find_map(|v| match &v.kind {
                noshape_model::drawing::ViewKind::Base { source, part: p, config, .. } if *source == tab && p == part => Some(config.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// A configuration as properties panels list it ("Length: Long"; live
    /// request 1638 (b)); empty when the Part Studio has no inputs.
    pub fn config_summary(&self, tab: TabId, values: &ConfigValues, cx: &App) -> String {
        let Some(TabContent::PartStudio(ps)) = self.doc.read(cx).session.doc.tab(tab).map(|t| &t.content) else { return String::new() };
        ps.configuration
            .inputs
            .iter()
            .map(|input| {
                let v = values.0.get(&input.id);
                let shown = match &input.kind {
                    ConfigInputKind::List { options, default } => {
                        let cur = v.unwrap_or(default);
                        options.iter().find(|o| &o.id == cur || &o.name == cur).map(|o| o.name.clone()).unwrap_or_else(|| cur.clone())
                    }
                    ConfigInputKind::Checkbox { default } => v.map(|s| s == "true").unwrap_or(*default).to_string(),
                    ConfigInputKind::Variable { default, .. } => v.cloned().unwrap_or_else(|| default.text.clone()),
                    ConfigInputKind::Text { default } => v.cloned().unwrap_or_else(|| default.clone()),
                };
                format!("{}: {shown}", input.name)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Whether the reference's Part Studio has configuration inputs.
    pub fn reference_configurable(&self, tab: TabId, cx: &App) -> bool {
        matches!(self.doc.read(cx).session.doc.tab(tab).map(|t| &t.content), Some(TabContent::PartStudio(ps)) if !ps.configuration.inputs.is_empty())
    }

    /// Cut list table › Change configuration...: the panel for the table's
    /// Part Studio, showing the table's configuration (else its view's).
    pub fn open_table_change_config(&mut self, table: u64, window: &mut Window, cx: &mut Context<Self>) {
        let sheet = self.current_sheet();
        let Some(t) = sheet.table(table) else { return };
        let Some(key) = noshape_model::drawing::views::table_source(sheet, t) else { return };
        let part = t.view().and_then(|v| noshape_model::drawing::views::view_source(sheet, v)).and_then(|(_, p)| p);
        self.open_change_config(key.tab, part, window, cx);
        if let Some(d) = &mut self.change_config {
            d.values = key.config;
            d.table = Some(table);
        }
        // The variable / text fields show the table's values.
        if let Some(d) = &self.change_config {
            let fields: Vec<(String, Entity<InputState>)> = d.fields.clone();
            let values = d.values.clone();
            for (id, st) in fields {
                if let Some(v) = values.0.get(&id).cloned() {
                    st.update(cx, |i, cx| i.set_value(v, window, cx));
                }
            }
        }
    }

    pub fn open_change_config(&mut self, tab: TabId, part: Option<noshape_model::BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        let values = self.reference_config(tab, &part);
        let mut fields = Vec::new();
        if let Some(TabContent::PartStudio(ps)) = self.doc.read(cx).session.doc.tab(tab).map(|t| t.content.clone()) {
            for input in &ps.configuration.inputs {
                let shown = match &input.kind {
                    ConfigInputKind::Variable { default, .. } => values.0.get(&input.id).cloned().unwrap_or_else(|| default.text.clone()),
                    ConfigInputKind::Text { default } => values.0.get(&input.id).cloned().unwrap_or_else(|| default.clone()),
                    _ => continue,
                };
                let st = cx.new(|cx| InputState::new(window, cx).default_value(shown));
                fields.push((input.id.clone(), st));
            }
        }
        self.change_config = Some(ChangeConfigDlg { tab, part, values, table: None, fields });
        cx.notify();
    }

    fn close_change_config(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.change_config.take() else { return };
        if accept {
            let mut values = d.values;
            for (id, st) in &d.fields {
                let t = st.read(cx).value().trim().to_string();
                if !t.is_empty() {
                    values.0.insert(id.clone(), t);
                }
            }
            let (tab, part) = (d.tab, d.part);
            match d.table {
                // Only the table changes; the views keep theirs (live 1638 (b)).
                Some(id) => self.edit_table(id, "Change configuration", cx, move |t| {
                    if let noshape_model::drawing::TableKind::CutList { config, .. } = &mut t.kind {
                        *config = Some(values);
                    }
                }),
                None => self.edit("Change configuration", cx, |dr| {
                    noshape_scene::drawing_tools::view_edit::change_configuration(dr, tab, &part, &values);
                }),
            }
            self.ensure_sources(false, cx);
        }
        cx.notify();
    }

    pub fn render_change_config(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.change_config.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let grey = gpui_kit::rgb(0x8a8a8a);
        let ed = cx.entity();
        let session = &self.doc.read(cx).session;
        let document = session.store.meta().map(|m| m.name).unwrap_or_else(|_| session.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
        let branch = session.store.branch(session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into());
        let Some(TabContent::PartStudio(ps)) = session.doc.tab(d.tab).map(|t| &t.content) else { return None };
        let studio = session.doc.tab(d.tab).map(|t| t.name.clone()).unwrap_or_default();
        let part = self.sources.iter().find(|(k, _)| k.tab == d.tab).and_then(|(_, m)| d.part.as_ref().and_then(|_| m.part(d.part.as_ref())).map(|p| p.name.clone())).unwrap_or_else(|| studio.clone());
        let row = |label: String, body: AnyElement| h_flex().h(px(30.)).px(px(10.)).gap(px(8.)).child(div().w(px(80.)).flex_none().text_size(px(12.)).child(label)).child(body).into_any_element();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, input) in ps.configuration.inputs.iter().enumerate() {
            let iid = input.id.clone();
            match &input.kind {
                ConfigInputKind::List { options, default } => {
                    let cur = d.values.0.get(&iid).cloned().unwrap_or(default.clone());
                    let label = options.iter().find(|o| o.id == cur).map(|o| o.name.clone()).unwrap_or_default();
                    let opts: Vec<(String, String)> = options.iter().map(|o| (o.id.clone(), o.name.clone())).collect();
                    let ed = ed.clone();
                    let select = Button::new(("change-config-list", i))
                        .label(label)
                        .ghost()
                        .xsmall()
                        .w(px(120.))
                        .dropdown_caret(true)
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xdddddd))
                        .rounded(px(0.))
                        .os_dropdown_menu(SharedString::from(format!("change-config-dd-{i}")), true, move |mut m, _, _| {
                            for (oid, name) in &opts {
                                let (e, iid, oid) = (ed.clone(), iid.clone(), oid.clone());
                                m = m.item(crate::os_menu::OsMenuItem::new(name.clone()).checked(oid == cur).on_click(move |_, _, cx| {
                                    e.update(cx, |t, cx| {
                                        if let Some(d) = &mut t.change_config {
                                            d.values.0.insert(iid.clone(), oid.clone());
                                        }
                                        cx.notify();
                                    })
                                }));
                            }
                            m
                        });
                    rows.push(row(input.name.clone(), select.into_any_element()));
                }
                ConfigInputKind::Checkbox { default } => {
                    let on = d.values.0.get(&iid).map(|v| v == "true").unwrap_or(*default);
                    rows.push(
                        h_flex()
                            .h(px(26.))
                            .px(px(10.))
                            .child(crate::drawing_chrome::DrCheckbox::new(("change-config-check", i)).label(input.name.clone()).user().checked(on).on_click(cx.listener(move |this, on: &bool, _, cx| {
                                if let Some(d) = &mut this.change_config {
                                    d.values.0.insert(iid.clone(), on.to_string());
                                }
                                cx.notify();
                            })))
                            .into_any_element(),
                    );
                }
                ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => {
                    if let Some((_, st)) = d.fields.iter().find(|(id, _)| *id == iid) {
                        rows.push(row(input.name.clone(), Input::new(st).xsmall().w(px(120.)).into_any_element()));
                    }
                }
            }
        }
        Some(
            v_flex()
                .id("dr-change-config")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(260.))
                .pb(px(8.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Change configuration"))
                        .child(
                            div()
                                .id("change-config-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .bg(pal.accept)
                                .child(crate::ui::glyph("check", 16., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_change_config(true, cx))),
                        )
                        .child(div().id("change-config-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_change_config(false, cx)))),
                )
                .child(div().px(px(10.)).pt(px(6.)).text_size(px(13.)).child(document))
                .child(h_flex().px(px(10.)).gap(px(4.)).text_size(px(11.)).text_color(grey).child(crate::ui::glyph("branch", 12., grey)).child(branch))
                .child(h_flex().h(px(30.)).mt(px(6.)).px(px(10.)).border_t_1().border_color(gpui_kit::rgb(0xe5e5e5)).child(div().flex_1().text_color(grey).child(studio)).child(crate::ui::glyph("part-studio", 16., grey)))
                .children(rows)
                .child(
                    v_flex()
                        .mx(px(10.))
                        .mt(px(8.))
                        .w(px(96.))
                        .items_center()
                        .child(div().size(px(80.)).border_1().border_color(gpui_kit::rgb(0xdddddd)).bg(pal.panel).child(crate::ui::glyph("part", 40., grey)).flex().items_center().justify_center())
                        .child(div().pt(px(4.)).text_size(px(11.)).child(part)),
                )
                .into_any_element(),
        )
    }
}
