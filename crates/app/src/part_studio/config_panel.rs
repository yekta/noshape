//! Right-hand Part Studio panel: variable table and configurations
//! (inputs, current configuration, configured feature parameters).

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, ListOption, VariableKind};
use noshape_model::params::{ParamKind, ParamValue};
use noshape_model::partstudio::PartStudio;
use noshape_units::{ExprValue, Quantity};

/// A feature with its configurable parameters: (id, name, [(param id, label)]).
type ConfigurableFeature = (noshape_model::FeatureId, String, Vec<(String, String)>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanel {
    Appearance,
    Variables,
    Configurations,
    SheetMetal,
    CutList,
}

/// Commit action of a panel text cell (gets the cell's text).
pub(super) type CellCommit = std::rc::Rc<dyn Fn(&mut PartStudioView, String, &mut Context<PartStudioView>)>;

impl PartStudioView {
    /// Commit the cell that had the focus at the last render if the focus has
    /// left it (clicking another cell or elsewhere), like Onshape's commit on
    /// blur. Called at the start of every panel render.
    fn commit_unfocused_cell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.panel_focused.clone() else { return };
        let Some((st, last)) = self.panel_inputs.get(&key).cloned() else {
            self.panel_focused = None;
            return;
        };
        if st.read(cx).focus_handle(cx).is_focused(window) {
            return;
        }
        self.panel_focused = None;
        let text = st.read(cx).value().to_string();
        if text != last
            && let Some(commit) = self.panel_commits.get(&key).cloned()
        {
            commit(self, text, cx);
        }
    }

    pub(super) fn toggle_panel(&mut self, panel: RightPanel, window: &mut Window, cx: &mut Context<Self>) {
        self.right_panel = if self.right_panel == Some(panel) { None } else { Some(panel) };
        let _ = window;
        cx.notify();
    }

    fn edit_config(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut PartStudio)) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, desc, cx, f);
        });
    }

    /// A text cell bound to a stable key; `commit` runs on Enter/blur.
    fn cell_input(&mut self, key: String, value: String, window: &mut Window, cx: &mut Context<Self>, commit: impl Fn(&mut Self, String, &mut Context<Self>) + 'static) -> Entity<InputState> {
        let commit: CellCommit = std::rc::Rc::new(commit);
        // The newest closure (it captures the current ids).
        self.panel_commits.insert(key.clone(), commit.clone());
        if let Some((st, last)) = self.panel_inputs.get(&key).cloned() {
            if st.read(cx).focus_handle(cx).is_focused(window) {
                self.panel_focused = Some(key.clone());
            } else if last != value {
                // The model value changed underneath (undo, other edits).
                st.update(cx, |s, cx| s.set_value(value.clone(), window, cx));
                self.panel_inputs.insert(key.clone(), (st.clone(), value));
            }
            return st;
        }
        let st = cx.new(|cx| InputState::new(window, cx).default_value(value.clone()));
        let k = key.clone();
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
                // Committed: losing the focus later must not commit it again.
                if let Some(e) = this.panel_inputs.get_mut(&k) {
                    e.1 = v.clone();
                }
                commit(this, v, cx);
            }
        });
        self.panel_subs.push(sub);
        self.panel_inputs.insert(key, (st.clone(), value));
        st
    }

    pub(super) fn render_right_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.right_panel?;
        self.commit_unfocused_cell(window, cx);
        let pal = p();
        let body = match panel {
            RightPanel::Appearance => self.render_appearance_panel(cx),
            RightPanel::Variables => self.render_variables(window, cx),
            RightPanel::Configurations => self.render_configurations(window, cx),
            RightPanel::SheetMetal => self.render_sheet_metal_panel(cx),
            RightPanel::CutList => self.render_cut_list_panel(cx),
        };
        let title = match panel {
            RightPanel::Appearance => "Appearances",
            RightPanel::Variables => "Variable table",
            RightPanel::Configurations => "Configurations",
            RightPanel::SheetMetal => "Sheet metal",
            RightPanel::CutList => "Cut list",
        };
        // Onshape (live, dom/panel-appearance.txt): 712 px, white; a 30 px
        // header whose title is one full-width active tab (13.33 px / 500
        // blue, 2 px blue underline); a 40 px `#eee` footer with `?`. The
        // panel closes from its strip button (no ×).
        Some(
            v_flex()
                .w(px(712.))
                .max_w(relative(0.5))
                .h_full()
                .flex_none()
                .bg(pal.panel)
                .border_l_1()
                .border_color(pal.border)
                .child(
                    div().h(px(30.)).flex_none().pt(px(4.)).relative().child(
                        div()
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_b_2()
                            .border_color(pal.accent)
                            .text_size(px(13.33))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(pal.accent)
                            .child(title),
                    )
                    // Appearances: "Add face appearance" at the title's right (live).
                    .when(panel == RightPanel::Appearance, |d| d.child(div().absolute().right(px(10.)).top(px(2.)).child(self.render_add_face_appearance(cx)))),
                )
                .child(v_flex().id("right-panel-body").flex_1().p(px(5.)).gap_2().overflow_y_scroll().child(body))
                .child(
                    h_flex()
                        .h(px(40.))
                        .flex_none()
                        .px(px(10.))
                        .justify_end()
                        .bg(gpui_kit::rgb(0xeeeeee))
                        .border_t_1()
                        .border_color(gpui_kit::rgb(0xcccccc))
                        .child(div().flex_1().flex().justify_center().children((panel == RightPanel::Variables).then(|| self.variable_panel_footer(cx))))
                        .child(div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?")),
                )
                .into_any_element(),
        )
    }

    /// Onshape's Variable table (live, part-studio/77): a gray "⌄ Part
    /// Studio 1 ⋯" section header, then a table Name | Variable type | Value
    /// | Description (98 px columns, bold headers, `#aaa` grid); with no
    /// variables, a large faint illustration and a hint.
    fn render_variables(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let units = self.doc.read(cx).session.doc.settings.units;
        let vars: Vec<(String, Quantity)> = self.display.variables.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let tab_name = self.doc.read(cx).session.doc.tabs.iter().find(|t| t.id == self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let grid = gpui_kit::rgb(0xaaaaaa);
        // Live (part-studio/77): 140 px columns, 34 px rows, like a Variable
        // Studio table.
        let cell = |text: SharedString, bold: bool| {
            div()
                .w(px(140.))
                .h(px(34.))
                .flex_none()
                .flex()
                .items_center()
                .px(px(5.))
                .border_r_1()
                .border_b_1()
                .border_color(grid)
                .text_size(px(13.33))
                .text_color(pal.text)
                .when(bold, |d| d.justify_center().font_weight(FontWeight::BOLD))
                .truncate()
                .child(text)
        };
        let header = h_flex()
            .h(px(35.))
            .px(px(6.))
            .gap(px(3.))
            .bg(gpui_kit::rgb(0xeaeaea))
            .border_1()
            .border_color(grid)
            .child(crate::ui::glyph("ui-chevron-down", 11., pal.text))
            .child(div().flex_1().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(tab_name))
            .child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(pal.info_text).child("•••"));
        let columns = h_flex().border_l_1().border_color(grid).bg(gpui_kit::rgb(0xeaeaea)).children(["Name", "Variable type", "Value", "Description"].map(|h| cell(h.into(), true).text_size(px(14.))));
        let rows = vars.into_iter().map(|(name, q)| {
            let (n1, n2) = (name.clone(), name.clone());
            let view = cx.entity();
            let kind = if q.dim == noshape_units::Dim::LENGTH {
                "Length"
            } else if q.dim == noshape_units::Dim::ANGLE {
                "Angle"
            } else if q.is_unitless() {
                "Number"
            } else {
                "Any"
            };
            h_flex()
                .id(SharedString::from(format!("var-row-{name}")))
                .border_l_1()
                .border_color(grid)
                // Onshape: hovering a variable highlights the features that use it.
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    let new = h.then(|| n1.clone());
                    if this.ui.var_hover != new {
                        this.ui.var_hover = new;
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }
                }))
                .hover(|d| d.bg(pal.row_hover))
                .child(cell(name.clone().into(), false))
                .child(cell(kind.into(), false))
                .child(cell(noshape_units::format_quantity(q, &units).into(), false))
                .child(cell(SharedString::default(), false))
                .os_context_menu(move |menu, _, _| {
                    let v = view.clone();
                    let n = n2.clone();
                    menu.item(crate::os_menu::OsMenuItem::new("Filter feature list").on_click(move |_, w, cx| v.update(cx, |this, cx| this.set_filter(&format!(":variable {n}"), w, cx))))
                })
        });
        let rows: Vec<_> = rows.collect();
        // Variable Studio tables inserted here (Onshape: below the Part
        // Studio's own table; same-document ones are editable).
        let refs = noshape_model::variables::element_studios(&self.doc.read(cx).session.doc, self.tab);
        let studios: Vec<noshape_model::document::TabId> = refs.iter().map(|r| r.studio).collect();
        self.ui.var_tables.retain(|k, _| studios.contains(k));
        let mut tables: Vec<AnyElement> = Vec::new();
        for (k, r) in refs.iter().enumerate() {
            // Another document's studio: read-only (Onshape).
            if r.external.is_some() {
                let target = crate::variable_insert::InsertTarget::Element(self.tab);
                tables.extend(crate::variable_insert::readonly_table(&self.doc, target, r, k, refs.len(), cx).map(|t| div().pt(px(10.)).child(t).into_any_element()));
                continue;
            }
            let (doc, host, s) = (self.doc.clone(), self.tab, r.studio);
            if !self.ui.var_tables.contains_key(&s) {
                let view = cx.new(|cx| crate::variable_studio::VariableStudioView::new_in_panel(doc, s, host, window, cx));
                self.ui.var_tables.insert(s, view);
            }
            if let Some(view) = self.ui.var_tables.get(&s) {
                tables.push(div().pt(px(10.)).child(view.clone()).into_any_element());
            }
        }
        let empty = self.display.variables.is_empty() && studios.is_empty();
        v_flex()
            .child(header)
            .child(columns)
            .children(rows)
            .children(tables)
            .when(empty, |d| {
                d.child(h_flex().border_l_1().border_color(grid).child(cell(SharedString::default(), false).child(div().text_color(pal.text_faint).italic().child("Name"))))
                    .child(
                        v_flex()
                            .items_center()
                            .pt(px(60.))
                            .gap(px(10.))
                            .child(div().opacity(0.15).child(crate::assets::icon("variable-table", 180., cx)))
                            .child(div().max_w(px(400.)).text_size(px(13.33)).text_color(pal.text_muted).text_center().child("Add variables to drive sketch and feature parameters in the Part Studio.")),
                    )
            })
            .into_any_element()
    }

    /// The Variable table footer (live, part-studio/77): a split button
    /// "Create Variable Studio | ▴" whose menu also has Insert Variable Studio.
    fn variable_panel_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        crate::variable_insert::panel_footer(self.doc.clone(), self.tab, cx)
    }

    /// Add a configuration input of one of Onshape's kinds (List, Checkbox, Variable).
    fn add_config_input(&mut self, label: &'static str, cx: &mut Context<Self>) {
        self.edit_config("Add configuration input", cx, |ps| {
            let n = ps.configuration.inputs.len() + 1;
            let id = format!("in{}", noshape_rand());
            let kind = match label {
                "Checkbox" => ConfigInputKind::Checkbox { default: false },
                "Variable" => ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("10 mm"), min: None, max: None },
                _ => ConfigInputKind::List {
                    options: vec![ListOption { id: "default".into(), name: "Default".into() }, ListOption { id: format!("opt{}", noshape_rand()), name: "Configuration 2".into() }],
                    default: "default".into(),
                },
            };
            let name = if label == "List" && n == 1 { "Configuration".to_string() } else { format!("{label} {n}") };
            ps.configuration.inputs.push(ConfigInput { id, name, kind, visible_when: None });
        })
    }

    /// Onshape's Configurations panel before any input exists (live,
    /// part-studio/77): a "Configure Part Studio ▾" split button, a large faint
    /// illustration and the help text.
    fn render_configurations_empty(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let border = gpui_kit::rgb(0xaaaaaa);
        let view = cx.entity();
        let main = div()
            .id("cfg-configure")
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::assets::icon("table", 16., cx))
            .child(div().text_size(px(14.)).text_color(pal.text).child("Configure Part Studio"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_config_input("List", cx)));
        let caret = Button::new("cfg-configure-more")
            .custom(gpui_kit::component::button::ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false))
            .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
            .w(px(22.))
            .h(px(28.))
            .p_0()
            .rounded(px(0.))
            .border_l_1()
            .border_color(border)
            .dropdown_menu(move |mut m, _, _| {
                for label in ["List", "Checkbox", "Variable"] {
                    let v = view.clone();
                    m = m.item(PopupMenuItem::new(label).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_config_input(label, cx))));
                }
                m
            });
        v_flex()
            .items_center()
            .pt(px(4.))
            .child(h_flex().rounded(px(3.)).border_1().border_color(border).bg(pal.panel).child(main).child(caret))
            .child(div().mt(px(105.)).child(crate::assets::icon("config-illustration", 160., cx)))
            .child({
                // Onshape's text in its own four lines (the panel is as wide as
                // live), the help link continuing the third line (bold,
                // underlined, `#094174`).
                let link = |id: &'static str, t: &'static str| {
                    div()
                        .id(id)
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui_kit::rgb(0x094174))
                        .underline()
                        .cursor_pointer()
                        .child(t)
                        .on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/configurations.htm"))
                };
                v_flex()
                    .mt(px(18.))
                    .items_center()
                    .text_size(px(13.33))
                    .line_height(px(20.))
                    .text_color(pal.text_muted)
                    .child("Add configurations to drive different design variants of a Part")
                    .child("Studio using sketch dimensions of parts and surfaces, feature")
                    .child(h_flex().child("parameters, or suppression states.\u{a0}").child(link("cfg-help-1", "Learn more about Part")))
                    .child(link("cfg-help-2", "Studio configurations."))
            })
            .into_any_element()
    }

    fn render_configurations(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let cfg = studio.configuration.clone();
        if cfg.inputs.is_empty() {
            return self.render_configurations_empty(cx);
        }
        let mut col = v_flex().gap_3();

        // Current configuration selector.
        if !cfg.inputs.is_empty() {
            let mut sel = v_flex().gap_1().p_2().rounded_md().bg(pal.app_bg).child(div().text_xs().text_color(pal.text_muted).child("Current configuration"));
            for input in &cfg.inputs {
                let current = cfg.value_of(&self.config_values, &input.id).unwrap_or_default();
                let id = input.id.clone();
                let control: AnyElement = match &input.kind {
                    ConfigInputKind::List { options, .. } => {
                        let label = options.iter().find(|o| o.id == current).map(|o| o.name.clone()).unwrap_or(current.clone());
                        let view = cx.entity();
                        let opts = options.clone();
                        Button::new(SharedString::from(format!("cur-{id}")))
                            .label(label)
                            .outline()
                            .xsmall()
                            .dropdown_caret(true)
                            .dropdown_menu(move |mut m, _, _| {
                                for o in &opts {
                                    let v = view.clone();
                                    let (iid, oid) = (id.clone(), o.id.clone());
                                    m = m.item(PopupMenuItem::new(o.name.clone()).on_click(move |_, w, cx| v.update(cx, |t, cx| t.set_config_value(&iid, oid.clone(), w, cx))));
                                }
                                m
                            })
                            .into_any_element()
                    }
                    ConfigInputKind::Checkbox { .. } => {
                        let on = current == "true";
                        gpui_kit::component::checkbox::Checkbox::new(SharedString::from(format!("cur-{id}")))
                            .checked(on)
                            .small()
                            .on_click(cx.listener(move |this, v: &bool, w, cx| this.set_config_value(&id, v.to_string(), w, cx)))
                            .into_any_element()
                    }
                    ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => {
                        let key = format!("curval-{id}");
                        let iid = id.clone();
                        let st = self.cell_input(key, current.clone(), window, cx, move |this, v, cx| {
                            this.config_values.0.insert(iid.clone(), v);
                            this.config_dirty(cx);
                        });
                        Input::new(&st).xsmall().into_any_element()
                    }
                };
                sel = sel.child(h_flex().gap_2().justify_between().child(div().text_xs().text_color(pal.text).child(input.name.clone())).child(div().w(px(150.)).child(control)));
            }
            col = col.child(sel);
        }

        // Inputs.
        for (ix, input) in cfg.inputs.iter().enumerate() {
            let iid = input.id.clone();
            let name_key = format!("in-name-{iid}");
            let iid2 = iid.clone();
            let name_input = self.cell_input(name_key, input.name.clone(), window, cx, move |this, v, cx| {
                let id = iid2.clone();
                this.edit_config("Rename configuration input", cx, |ps| {
                    if let Some(i) = ps.configuration.inputs.iter_mut().find(|i| i.id == id) {
                        i.name = v;
                    }
                });
            });
            let kind_label = match &input.kind {
                ConfigInputKind::List { .. } => "List",
                ConfigInputKind::Checkbox { .. } => "Checkbox",
                ConfigInputKind::Variable { .. } => "Variable",
                ConfigInputKind::Text { .. } => "Text",
            };
            let iid3 = iid.clone();
            let mut card = v_flex()
                .gap_1p5()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(pal.border)
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().text_xs().px_1p5().rounded_sm().bg(pal.accent_soft).text_color(pal.accent_hover).child(kind_label))
                        .child(div().flex_1().child(Input::new(&name_input).xsmall()))
                        .child(Button::new(SharedString::from(format!("in-del-{ix}"))).icon(IconName::Delete).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let id = iid3.clone();
                            this.edit_config("Delete configuration input", cx, |ps| {
                                ps.configuration.inputs.retain(|i| i.id != id);
                                ps.configuration.configured.retain(|c| c.input != id);
                            });
                        }))),
                );
            match &input.kind {
                ConfigInputKind::List { options, .. } => {
                    for (oi, o) in options.iter().enumerate() {
                        let key = format!("opt-{iid}-{}", o.id);
                        let (a, b) = (iid.clone(), o.id.clone());
                        let st = self.cell_input(key, o.name.clone(), window, cx, move |this, v, cx| {
                            let (a, b) = (a.clone(), b.clone());
                            this.edit_config("Rename configuration", cx, |ps| {
                                if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a)
                                    && let Some(o) = options.iter_mut().find(|o| o.id == b)
                                {
                                    o.name = v;
                                }
                            });
                        });
                        let (a2, b2) = (iid.clone(), o.id.clone());
                        card = card.child(
                            h_flex()
                                .gap_1()
                                .child(div().w(px(16.)).text_xs().text_color(pal.text_faint).child(format!("{}", oi + 1)))
                                .child(div().flex_1().child(Input::new(&st).xsmall()))
                                .child(Button::new(SharedString::from(format!("opt-del-{iid}-{oi}"))).icon(IconName::Minus).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let (a, b) = (a2.clone(), b2.clone());
                                    this.edit_config("Delete configuration", cx, |ps| {
                                        if let Some(ConfigInput { kind: ConfigInputKind::List { options, default }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a)
                                            && options.len() > 1
                                        {
                                            options.retain(|o| o.id != b);
                                            if *default == b {
                                                *default = options[0].id.clone();
                                            }
                                        }
                                    });
                                }))),
                        );
                    }
                    let a = iid.clone();
                    card = card.child(Button::new(SharedString::from(format!("opt-add-{iid}"))).label("Add configuration").icon(IconName::Plus).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let a = a.clone();
                        this.edit_config("Add configuration", cx, |ps| {
                            if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a) {
                                let n = options.len() + 1;
                                options.push(ListOption { id: format!("opt{}", noshape_rand()), name: format!("Configuration {n}") });
                            }
                        });
                    })));
                }
                ConfigInputKind::Variable { default, kind, .. } => {
                    let key = format!("var-def-{iid}");
                    let a = iid.clone();
                    let st = self.cell_input(key, default.text.clone(), window, cx, move |this, v, cx| {
                        let a = a.clone();
                        this.edit_config("Edit configuration variable", cx, |ps| {
                            if let Some(ConfigInput { kind: ConfigInputKind::Variable { default, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a) {
                                *default = ExprValue::new(v);
                            }
                        });
                    });
                    card = card.child(
                        h_flex()
                            .gap_2()
                            .child(div().text_xs().text_color(pal.text_muted).child(format!("{kind:?} default")))
                            .child(div().flex_1().child(Input::new(&st).xsmall())),
                    );
                    card = card.child(div().text_xs().text_color(pal.text_faint).child(format!("Use as #{} in any field", input.name)));
                }
                _ => {}
            }
            col = col.child(card);
        }

        // Add input menu (Onshape's input kinds).
        let view = cx.entity();
        col = col.child(
            Button::new("add-input").label("Add input").icon(IconName::Plus).outline().xsmall().dropdown_menu(move |mut m, _, _| {
                for label in ["List", "Checkbox", "Variable"] {
                    let v = view.clone();
                    m = m.item(PopupMenuItem::new(label).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_config_input(label, cx))));
                }
                m
            }),
        );

        // Configured parameters table.
        let list_inputs: Vec<ConfigInput> = cfg.inputs.iter().filter(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. })).cloned().collect();
        if !list_inputs.is_empty() {
            col = col.child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).mt_2().child("CONFIGURED PARAMETERS"));
            for (ci, c) in cfg.configured.iter().enumerate() {
                let Some(input) = cfg.inputs.iter().find(|i| i.id == c.input) else { continue };
                let fname = studio.feature(c.feature).map(|f| f.name.clone()).unwrap_or_else(|| "deleted feature".into());
                let plabel = studio
                    .feature(c.feature)
                    .and_then(|f| f.feature_type())
                    .and_then(|t| t.param(&c.param))
                    .map(|p| p.label)
                    .unwrap_or(if c.param == noshape_model::configuration::SUPPRESSED_PARAM { "Suppressed" } else { "?" });
                let mut card = v_flex().gap_1().p_2().rounded_md().border_1().border_color(pal.border).child(
                    h_flex()
                        .justify_between()
                        // Which input drives it, when there are several.
                        .child(div().text_xs().text_color(pal.text).child(if list_inputs.len() > 1 { format!("{fname} › {plabel} ({})", input.name) } else { format!("{fname} › {plabel}") }))
                        .child(Button::new(SharedString::from(format!("cp-del-{ci}"))).icon(IconName::Delete).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.edit_config("Remove configured parameter", cx, |ps| {
                                if ci < ps.configuration.configured.len() {
                                    ps.configuration.configured.remove(ci);
                                }
                            });
                        }))),
                );
                let keys: Vec<(String, String)> = match &input.kind {
                    ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
                    _ => vec![("true".into(), "Checked".into()), ("false".into(), "Unchecked".into())],
                };
                for (k, label) in keys {
                    let val = c.values.get(&k).cloned();
                    let text = match &val {
                        Some(ParamValue::Expr(e)) => e.text.clone(),
                        Some(ParamValue::Bool(b)) => b.to_string(),
                        Some(ParamValue::Enum(s)) => s.clone(),
                        Some(ParamValue::Text(s)) => s.clone(),
                        _ => String::new(),
                    };
                    let key = format!("cp-{ci}-{k}");
                    let kk = k.clone();
                    let st = self.cell_input(key, text, window, cx, move |this, v, cx| {
                        let kk = kk.clone();
                        this.edit_config("Edit configured value", cx, |ps| {
                            if let Some(cp) = ps.configuration.configured.get_mut(ci) {
                                let new = match cp.values.get(&kk) {
                                    Some(ParamValue::Bool(_)) => ParamValue::Bool(v.trim() == "true" || v.trim() == "1" || v.trim().eq_ignore_ascii_case("yes")),
                                    Some(ParamValue::Enum(_)) => ParamValue::Enum(v.trim().to_string()),
                                    Some(ParamValue::Text(_)) => ParamValue::Text(v),
                                    _ => ParamValue::Expr(ExprValue::new(v)),
                                };
                                cp.values.insert(kk.clone(), new);
                            }
                        });
                    });
                    card = card.child(h_flex().gap_2().child(div().w(px(110.)).text_xs().text_color(pal.text_muted).truncate().child(label)).child(div().flex_1().child(Input::new(&st).xsmall())));
                }
                col = col.child(card);
            }
            // Add a configured parameter: pick feature then parameter.
            let view = cx.entity();
            let feats: Vec<ConfigurableFeature> = studio
                .features
                .iter()
                .filter_map(|f| {
                    let t = f.feature_type()?;
                    let params: Vec<(String, String)> = std::iter::once((noshape_model::configuration::SUPPRESSED_PARAM.to_string(), "Suppressed".to_string()))
                        .chain(t.params.iter().filter(|p| matches!(p.kind, ParamKind::Num(_) | ParamKind::Bool { .. } | ParamKind::Enum { .. })).filter(|p| p.is_visible(&f.params) && !p.hints.has(noshape_model::params::Hints::UNCONFIGURABLE)).map(|p| (p.id.to_string(), p.label.to_string())))
                        .collect();
                    Some((f.id, f.name.clone(), params))
                })
                .collect();
            // With several inputs, first choose the one that drives the
            // parameter (Onshape configures a parameter per input).
            let inputs: Vec<(String, String)> = list_inputs.iter().map(|i| (i.id.clone(), i.name.clone())).collect();
            // Onshape: "+ Configure features", then click a feature in the
            // list and the parameter in its dialog.
            let picking = self.ui.configure_pick.is_some();
            let first_input = inputs.first().map(|(id, _)| id.clone());
            col = col.child(
                Button::new("configure-features")
                    .label(if picking { "Click a feature, then a parameter (Esc ends)" } else { "Configure features" })
                    .icon(IconName::Plus)
                    .outline()
                    .xsmall()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ui.configure_pick = if this.ui.configure_pick.is_some() { None } else { first_input.clone() };
                        cx.notify();
                    })),
            );
            col = col.child(
                Button::new("add-configured").label("Configure a parameter").icon(IconName::Plus).outline().xsmall().dropdown_menu(move |mut m, window, cx| {
                    let (feats, view) = (feats.clone(), view.clone());
                    let feature_items = move |mut m: gpui_kit::component::menu::PopupMenu, input: String, window: &mut Window, cx: &mut Context<gpui_kit::component::menu::PopupMenu>| {
                        for (fid, fname, params) in &feats {
                            let v = view.clone();
                            let params = params.clone();
                            let fid = *fid;
                            let input = input.clone();
                            m = m.submenu(fname.clone(), window, cx, move |mut sm, _, _| {
                                for (pid, plabel) in &params {
                                    let v = v.clone();
                                    let pid = pid.clone();
                                    let input = input.clone();
                                    sm = sm.item(PopupMenuItem::new(plabel.clone()).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_configured(fid, &pid, &input, cx))));
                                }
                                sm
                            });
                        }
                        m
                    };
                    if let [(id, _)] = inputs.as_slice() {
                        return feature_items(m, id.clone(), window, cx);
                    }
                    for (id, name) in &inputs {
                        let id = id.clone();
                        let feature_items = feature_items.clone();
                        m = m.submenu(name.clone(), window, cx, move |sm, window, cx| feature_items(sm, id.clone(), window, cx));
                    }
                    m
                }),
            );
        }
        col.into_any_element()
    }

    pub(super) fn add_configured(&mut self, feature: noshape_model::FeatureId, param: &str, input: &str, cx: &mut Context<Self>) {
        let param = param.to_string();
        let input = input.to_string();
        self.edit_config("Configure parameter", cx, |ps| {
            let Some(f) = ps.feature(feature) else { return };
            let current = if param == noshape_model::configuration::SUPPRESSED_PARAM { Some(ParamValue::Bool(f.suppressed)) } else { f.params.get(&param).cloned() };
            let Some(current) = current else { return };
            let keys: Vec<String> = match ps.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
                Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
                Some(ConfigInputKind::Checkbox { .. }) => vec!["true".into(), "false".into()],
                _ => return,
            };
            let values = keys.into_iter().map(|k| (k, current.clone())).collect();
            ps.configuration.configured.push(ConfiguredParam { input, feature, param, values });
        });
    }

    fn set_config_value(&mut self, input: &str, value: String, window: &mut Window, cx: &mut Context<Self>) {
        self.config_values.0.insert(input.to_string(), value);
        self.schedule_regen(window, cx);
        cx.notify();
    }

    fn config_dirty(&mut self, cx: &mut Context<Self>) {
        self.needs_regen = true;
        cx.notify();
    }

    /// Small toolbar of panel toggles shown above the view controls.
    pub(super) fn render_panel_toggles(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Onshape's right panel rail (§3.6): a strip of 32 px white buttons
        // flush with the right edge, vertically centered on the viewport.
        let pal = p();
        let active = self.right_panel;
        let btn = |id: &'static str, icon: &'static str, tip: &'static str, panel: RightPanel, cx: &mut Context<Self>| {
            let on = active == Some(panel);
            div()
                .id(id)
                .relative()
                .w(px(33.))
                .h(px(34.))
                .mt(px(-1.))
                .flex()
                .items_center()
                .justify_center()
                .bg(if on { pal.row_hover } else { pal.panel })
                .hover(|d| d.bg(pal.panel_hover))
                .cursor_pointer()
                .border_1()
                .border_color(pal.border_strong)
                .child(crate::assets::icon(icon, 20., cx))
                .when(on, |d| d.child(div().absolute().right_0().top_0().bottom_0().w(px(3.)).bg(pal.accent)))
                .tooltip(crate::ui::tooltip(tip))
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.toggle_panel(panel, w, cx)))
        };
        v_flex()
            .pt(px(1.))
            .mr(px(-1.))
            // Onshape order: Appearance, Configurations, Variable table.
            .child(btn("panel-appearance", "appearance", "Appearance panel", RightPanel::Appearance, cx))
            .child(btn("panel-config", "configuration", "Configurations", RightPanel::Configurations, cx))
            .child(btn("panel-vars", "variable-table", "Variable table", RightPanel::Variables, cx))
            .when(self.has_sheet_metal(), |d| d.child(btn("panel-sm", "sheet-metal-table", "Sheet metal: flat pattern and bend table", RightPanel::SheetMetal, cx)))
            .when(self.has_frames(), |d| d.child(btn("panel-cut", "cut-list", "Cut list", RightPanel::CutList, cx)))
            .into_any_element()
    }
}

fn noshape_rand() -> String {
    let u = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}", (u as u64) ^ 0x9E37_79B9_7F4A_7C15)
}

