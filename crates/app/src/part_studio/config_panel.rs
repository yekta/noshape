//! Right-hand Part Studio panel: variable table and configurations
//! (inputs, current configuration, configured feature parameters).

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, ListOption, VariableKind};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_units::{ExprValue, Quantity};


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanel {
    Appearance,
    Variables,
    Configurations,
    CustomTables,
    Inspection,
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
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
                // Committed: losing the focus later must not commit it again.
                if let Some(e) = this.panel_inputs.get_mut(&k) {
                    e.1 = v.clone();
                }
                commit(this, v, cx);
                // Enter hands the keyboard back to the Part Studio (its
                // shortcuts work again), like Onshape.
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    let fh = this.focus_handle(cx);
                    window.focus(&fh, cx);
                }
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
        let configured = panel == RightPanel::Configurations && self.studio(cx).is_some_and(|s| !s.configuration.inputs.is_empty());
        let body = match panel {
            RightPanel::Appearance => self.render_appearance_panel(cx),
            RightPanel::Variables => self.render_variables(window, cx),
            RightPanel::Configurations => self.render_configurations(window, cx),
            RightPanel::CustomTables => self.render_custom_tables(cx),
            RightPanel::Inspection => self.render_inspection(cx),
            RightPanel::SheetMetal => self.render_sheet_metal_panel(cx),
            RightPanel::CutList => self.render_cut_list_panel(cx),
        };
        let title = match panel {
            RightPanel::Appearance => "Appearances",
            RightPanel::Variables => "Variable table",
            RightPanel::Configurations => "Configurations",
            RightPanel::CustomTables => "Custom tables",
            RightPanel::Inspection => "Model definitions",
            RightPanel::SheetMetal => "Sheet metal",
            RightPanel::CutList => "Cut list",
        };
        // Onshape (live, dom/panel-appearance.txt): 712 px, white; a 30 px
        // header whose title is one full-width active tab (13.33 px / 500
        // blue, 2 px blue underline); a 40 px `#eee` footer with `?`. The
        // panel closes from its strip button (no ×).
        // Live (183): a 1 px `#d7d7d7` divider and a 5 px white resize
        // gutter left of the 712 px pane; `#ccc` line over gutter and pane.
        Some(
            v_flex()
                .w(px(718.))
                .max_w(relative(0.5))
                .h_full()
                .flex_none()
                .bg(pal.panel)
                .border_l_1()
                .border_color(pal.border)
                .pl(px(5.))
                .child(div().h(px(1.)).ml(px(-5.)).flex_none().bg(gpui_kit::rgb(0xcccccc)))
                .child(
                    div().h(px(30.)).flex_none().pt(px(4.)).relative().child(if configured {
                        // Configured Part Studio (live 182): two tabs.
                        let on = self.ui.cfg_properties_tab;
                        let tab = |id: &'static str, label: &'static str, active: bool, props: bool| {
                            // Live: the tabs share the width by their labels
                            // (Configurations 336 px, Configured properties 376).
                            div()
                                .id(id)
                                .flex_grow(1.)
                                .h(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_size(px(13.33))
                                .when(active, |d| d.border_b_2().border_color(pal.accent).font_weight(FontWeight::MEDIUM).text_color(pal.accent))
                                .when(!active, |d| d.border_b_1().border_color(pal.border).text_color(pal.text))
                                .child(label)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.ui.cfg_properties_tab = props;
                                    cx.notify();
                                }))
                        };
                        h_flex().child(tab("cfg-tab-configs", "Configurations", !on, false)).child(tab("cfg-tab-props", "Configured properties", on, true)).into_any_element()
                    } else {
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
                            .child(title)
                            .into_any_element()
                    })
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
                        .child(div().flex_1().flex().justify_center().children((panel == RightPanel::Variables).then(|| self.variable_panel_footer(cx))).children(configured.then(|| self.render_add_config_input(cx))))
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
            .pl(px(4.))
            .pr(px(6.))
            .gap(px(3.))
            .bg(gpui_kit::rgb(0xeaeaea))
            .border_1()
            .border_color(grid)
            .child(crate::ui::glyph("ui-chevron-down", 14., pal.text))
            .child(div().flex_1().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(tab_name))
            .child(crate::assets::icon("more", 20., cx));
        // Live (164): only the header cells are gray, not the rest of the row.
        let columns = h_flex().border_l_1().border_color(grid).children(["Name", "Variable type", "Value", "Description"].map(|h| cell(h.into(), true).text_size(px(14.)).bg(gpui_kit::rgb(0xeaeaea))));
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
            let view = self.ui.var_tables.entry(s).or_insert_with(|| cx.new(|cx| crate::variable_studio::VariableStudioView::new_in_panel(doc, s, host, window, cx)));
            tables.push(div().pt(px(10.)).child(view.clone()).into_any_element());
        }
        let empty = self.display.variables.is_empty() && studios.is_empty();
        // Live (164): the tables run flush with the pane (the body has 5 px
        // padding), the first header 2 px under the title's underline, and
        // every table ends with the italic "Name" row.
        v_flex()
            .mx(px(-5.))
            .mt(px(-3.))
            .child(header)
            .child(columns)
            .children(rows)
            .child(h_flex().border_l_1().border_color(grid).child(cell(SharedString::default(), false).child(div().text_color(pal.text_faint).italic().child("Name"))))
            .children(tables)
            .when(empty, |d| {
                d.child(
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
                    // Live (part-studio/182): a new List has the one "Default" row.
                    options: vec![ListOption { id: "default".into(), name: "Default".into() }],
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
        let caret = div()
            .id("cfg-configure-more")
            .w(px(22.))
            .h(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .border_l_1()
            .border_color(border)
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
            // Live (part-studio/181): List, Checkbox, Configuration variable.
            .on_click(move |e: &ClickEvent, window, cx| {
                let v = view.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    for (label, icon, kind) in [("List", "table", "List"), ("Checkbox", "checkbox-input", "Checkbox"), ("Configuration variable", "variable", "Variable")] {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_config_input(kind, cx))));
                    }
                    m
                });
                crate::os_menu::open(e.position(), menu, window, cx);
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

    /// Onshape's configured Configurations panel (live part-studio/182–185,
    /// dom/panel-configuration-*.txt): a filter row, then one section per
    /// input: a gray `#eaeaea` header bar ("⌄ Configuration  Default:
    /// Default  [+ Configure features] ⋯") over a table whose frozen Name
    /// column lists the configurations (the current one blue with a blue
    /// left bar, an italic "Name" row to add one) and one column per
    /// configured parameter; a blue info box while nothing is configured.
    fn render_configurations(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let cfg = studio.configuration.clone();
        if cfg.inputs.is_empty() {
            return self.render_configurations_empty(cx);
        }
        if self.ui.cfg_properties_tab {
            return self.render_configured_properties();
        }
        let grid = gpui_kit::rgb(0xaaaaaa);
        let filter = match &self.ui.cfg_filter {
            Some(f) => f.clone(),
            None => {
                let f = cx.new(|cx| InputState::new(window, cx));
                let sub = cx.subscribe(&f, |_, _, ev: &InputEvent, cx| {
                    if matches!(ev, InputEvent::Change) {
                        cx.notify();
                    }
                });
                self.panel_subs.push(sub);
                self.ui.cfg_filter = Some(f.clone());
                f
            }
        };
        let query = filter.read(cx).value().trim().to_lowercase();
        // Filter row: funnel, "Filter inputs", kind toggles, ⋯.
        // Live (183): window-style icons (dark frame with a header band).
        let kinds = [("cfg-filter-list", "List"), ("cfg-filter-checkbox", "Checkbox"), ("cfg-filter-variable", "Configuration variable")];
        let mut toggles = h_flex().ml(px(9.)).gap(px(10.));
        // Live: plain icons; clicking one shows only inputs of that kind.
        for (k, (icon, tip)) in kinds.iter().enumerate() {
            let only = self.ui.cfg_only_kinds[k];
            toggles = toggles.child(
                div()
                    .id(SharedString::from(format!("cfg-kind-{k}")))
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .cursor_pointer()
                    .when(only, |d| d.bg(pal.panel_active))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon(icon, 20., cx))
                    .tooltip(crate::ui::tooltip(format!("Show only {} inputs", tip.to_lowercase())))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ui.cfg_only_kinds[k] = !this.ui.cfg_only_kinds[k];
                        cx.notify();
                    })),
            );
        }
        // Live: the tables run flush with the panel's left edge (the panel
        // body has 5 px padding).
        // Live (183): a 44 px row; 15 px funnel 10 px in, a 30 px box from
        // x 39, the three toggles 36 px apart, ⋯ 3 px from the edge.
        let mut col = v_flex().mx(px(-5.)).mt(px(-5.)).child(
            h_flex()
                .h(px(44.))
                .pl(px(8.))
                .pr(px(3.))
                .child(crate::ui::glyph("filter", 19., pal.text))
                .child(
                    div()
                        .flex_1()
                        .relative()
                        .ml(px(11.))
                        .h(px(30.))
                        .border_1()
                        .border_color(pal.border_strong)
                        .rounded(px(2.))
                        .flex()
                        .items_center()
                        .child(Input::new(&filter).appearance(false).small().text_size(px(14.)))
                        .when(query.is_empty(), |d| d.child(div().absolute().left(px(6.)).top_0().bottom_0().flex().items_center().text_size(px(14.)).italic().text_color(pal.text_faint).child("Filter inputs"))),
                )
                .child(toggles)
                .child(div().id("cfg-panel-more").ml(px(6.)).size(px(26.)).flex().items_center().justify_center().cursor_pointer().child(crate::assets::icon("more", 20., cx)).os_context_menu(|m, _, _| m.item(crate::os_menu::OsMenuItem::new("Copy all inputs").disabled(true)))),
        );
        for input in &cfg.inputs {
            let kind_ix = match input.kind {
                ConfigInputKind::List { .. } => 0,
                ConfigInputKind::Checkbox { .. } => 1,
                _ => 2,
            };
            let only = self.ui.cfg_only_kinds;
            if (only.iter().any(|o| *o) && !only[kind_ix]) || (!query.is_empty() && !input.name.to_lowercase().contains(&query)) {
                continue;
            }
            col = col.child(self.render_config_input(&studio, input, grid, window, cx));
        }
        col.into_any_element()
    }

    fn render_config_input(&mut self, studio: &PartStudio, input: &ConfigInput, grid: Rgba, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let cfg = &studio.configuration;
        let iid = input.id.clone();
        let folded = self.ui.cfg_folded.contains(&iid);
        let current = cfg.value_of(&self.config_values, &iid).unwrap_or_default();
        let default_text = match &input.kind {
            ConfigInputKind::List { options, default } => options.iter().find(|o| &o.id == default).map(|o| o.name.clone()).unwrap_or_default(),
            ConfigInputKind::Checkbox { default } => if *default { "Checked".into() } else { "Unchecked".into() },
            ConfigInputKind::Variable { default, .. } => default.text.clone(),
            ConfigInputKind::Text { default } => default.clone(),
        };
        // Header bar.
        let (i1, i2, i3) = (iid.clone(), iid.clone(), iid.clone());
        let picking = self.ui.configure_pick.as_deref() == Some(iid.as_str());
        let configurable = matches!(input.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. });
        let view = cx.entity();
        let header = h_flex()
            .h(px(35.))
            .flex_none()
            .px(px(6.))
            .gap(px(4.))
            .bg(gpui_kit::rgb(0xeaeaea))
            .border_t_1()
            .border_b_1()
            .border_l_1()
            .border_color(grid)
            .child(
                div()
                    .id(SharedString::from(format!("cfg-fold-{iid}")))
                    .w(px(12.))
                    .cursor_pointer()
                    .child(crate::ui::glyph(if folded { "ui-chevron-right" } else { "ui-chevron-down" }, 10., pal.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.ui.cfg_folded.remove(&i1) {
                            this.ui.cfg_folded.insert(i1.clone());
                        }
                        cx.notify();
                    })),
            )
            .child(self.config_name_cell(format!("in-name-{iid}"), input.name.clone(), (px(16.), FontWeight::MEDIUM), window, cx, move |this, v, cx| {
                let id = i2.clone();
                this.edit_config("Rename configuration input", cx, |ps| {
                    if let Some(i) = ps.configuration.inputs.iter_mut().find(|i| i.id == id) {
                        i.name = v;
                    }
                });
            }))
            .child(div().flex_1().min_w_0().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x999999)).child(format!("Default: {default_text}")))
            .when(configurable, |d| {
                d.child(
                    h_flex()
                        .id(SharedString::from(format!("cfg-configure-{iid}")))
                        .h(px(23.))
                        .px(px(4.))
                        .gap(px(2.))
                        .bg(if picking { pal.panel_active } else { pal.panel })
                        .border_1()
                        .border_color(grid)
                        .rounded(px(3.5))
                        .cursor_pointer()
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::ui::glyph("plus", 14., pal.text))
                        .child(div().pr(px(4.)).text_size(px(13.)).text_color(pal.text).child("Configure features"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ui.configure_pick = if this.ui.configure_pick.as_deref() == Some(i3.as_str()) { None } else { Some(i3.clone()) };
                            cx.notify();
                        })),
                )
            })
            .child({
                let (v1, v2, rid, did) = (view.clone(), view.clone(), iid.clone(), iid.clone());
                div()
                    .id(SharedString::from(format!("cfg-more-{iid}")))
                    .size(px(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(crate::assets::icon("more", 20., cx))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let (v1, v2, rid, did) = (v1.clone(), v2.clone(), rid.clone(), did.clone());
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                            let (v1, v2, rid, did) = (v1.clone(), v2.clone(), rid.clone(), did.clone());
                            m.item(crate::os_menu::OsMenuItem::new("Rename").on_click(move |_, _, cx| {
                                v1.update(cx, |this, cx| {
                                    this.ui.cfg_editing = Some(format!("in-name-{rid}"));
                                    cx.notify();
                                })
                            }))
                            .separator()
                            .item(crate::os_menu::OsMenuItem::new("Delete").icon_name("menu-delete").on_click(move |_, _, cx| {
                                v2.update(cx, |this, cx| {
                                    let id = did.clone();
                                    this.edit_config("Delete configuration input", cx, |ps| {
                                        ps.configuration.inputs.retain(|i| i.id != id);
                                        ps.configuration.configured.retain(|c| c.input != id);
                                    });
                                })
                            }))
                        });
                        crate::os_menu::open(e.position(), menu, window, cx);
                    })
            });
        let mut section = v_flex().mb(px(10.)).child(header);
        if folded {
            return section.into_any_element();
        }
        // Rows: the configurations (List), the two states (Checkbox) or the
        // variable's default (Configuration variable).
        let rows: Vec<(String, String)> = match &input.kind {
            ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
            ConfigInputKind::Checkbox { .. } => vec![("true".into(), "Checked".into()), ("false".into(), "Unchecked".into())],
            _ => Vec::new(),
        };
        let columns: Vec<(usize, &ConfiguredParam)> = cfg.configured.iter().enumerate().filter(|(_, c)| c.input == iid).collect();
        const NAME_W: f32 = 140.;
        const COL_W: f32 = 140.;
        const ROW_H: f32 = 34.;
        let cell = |w: f32| div().w(px(w)).h(px(ROW_H)).flex_none().px(px(3.)).flex().items_center().border_r_1().border_b_1().border_color(grid).text_size(px(14.));
        let head = |w: f32| cell(w).bg(gpui_kit::rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD);
        match &input.kind {
            ConfigInputKind::Variable { default, .. } => {
                let a = iid.clone();
                let value = self.config_name_cell(format!("var-def-{iid}"), default.text.clone(), (px(14.), FontWeight::NORMAL), window, cx, move |this, v, cx| {
                    let a = a.clone();
                    this.edit_config("Edit configuration variable", cx, |ps| {
                        if let Some(ConfigInput { kind: ConfigInputKind::Variable { default, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a) {
                            *default = ExprValue::new(v);
                        }
                    });
                });
                section = section.child(h_flex().border_l_1().border_color(grid).child(head(NAME_W).child("Name")).child(head(COL_W).child("Default value")));
                section = section.child(h_flex().border_l_1().border_color(grid).child(cell(NAME_W).child(format!("#{}", input.name))).child(cell(COL_W).child(value)));
            }
            ConfigInputKind::Text { .. } => {}
            _ => {
                // Header row: Name + one column per configured parameter.
                let mut hrow = h_flex().border_l_1().border_color(grid).child(head(NAME_W).child("Name"));
                for (_, c) in &columns {
                    let fname = studio.feature(c.feature).map(|f| f.name.clone()).unwrap_or_else(|| "Deleted feature".into());
                    let plabel = studio.feature(c.feature).and_then(|f| f.feature_type()).and_then(|t| t.param(&c.param)).map(|p| p.label).unwrap_or(if c.param == noshape_model::configuration::SUPPRESSED_PARAM { "Suppressed" } else { "?" });
                    hrow = hrow.child(head(COL_W).flex_col().line_height(px(15.)).child(div().text_size(px(12.)).font_weight(FontWeight::NORMAL).text_color(pal.text_muted).truncate().child(fname)).child(div().truncate().child(plabel)));
                }
                section = section.child(hrow);
                for (rk, rname) in &rows {
                    let active = *rk == current;
                    let (ia, rka) = (iid.clone(), rk.clone());
                    let name_el: AnyElement = if matches!(input.kind, ConfigInputKind::List { .. }) {
                        let (a, b) = (iid.clone(), rk.clone());
                        self.config_name_cell(format!("opt-{iid}-{rk}"), rname.clone(), (px(14.), if active { FontWeight::MEDIUM } else { FontWeight::NORMAL }), window, cx, move |this, v, cx| {
                            let (a, b) = (a.clone(), b.clone());
                            this.edit_config("Rename configuration", cx, |ps| {
                                if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a)
                                    && let Some(o) = options.iter_mut().find(|o| o.id == b)
                                {
                                    o.name = v;
                                }
                            });
                        })
                    } else {
                        rname.clone().into_any_element()
                    };
                    let (da, db, vr) = (iid.clone(), rk.clone(), view.clone());
                    let mut row = h_flex()
                        .id(SharedString::from(format!("cfg-row-{iid}-{rk}")))
                        .border_l_1()
                        .border_color(grid)
                        .child(
                            cell(NAME_W)
                                .relative()
                                .cursor_pointer()
                                .pl(px(6.))
                                .when(active, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(pal.accent)))
                                .child(name_el),
                        )
                        // Clicking a row makes it the current configuration.
                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                            if e.click_count() == 1 {
                                this.set_config_value(&ia, rka.clone(), window, cx);
                            }
                        }))
                        .when(matches!(input.kind, ConfigInputKind::List { .. }), |d| {
                            d.os_context_menu(move |m, _, _| {
                                let (v, a, b) = (vr.clone(), da.clone(), db.clone());
                                m.item(crate::os_menu::OsMenuItem::new("Delete").icon_name("menu-delete").on_click(move |_, _, cx| {
                                    let (a, b) = (a.clone(), b.clone());
                                    v.update(cx, |this, cx| {
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
                                    })
                                }))
                            })
                        });
                    for (ci, c) in &columns {
                        row = row.child(cell(COL_W).child(self.configured_value_cell(*ci, c, rk, window, cx)));
                    }
                    section = section.child(row);
                }
                // The italic "Name" row adds a configuration (List).
                if matches!(input.kind, ConfigInputKind::List { .. }) {
                    let a = iid.clone();
                    // Keyed by the row count, so the row is empty again after adding.
                    let key = format!("opt-new-{iid}-{}", rows.len());
                    let st = self.cell_input(key, String::new(), window, cx, move |this, v, cx| {
                        let name = v.trim().to_string();
                        if name.is_empty() {
                            return;
                        }
                        let a = a.clone();
                        this.edit_config("Add configuration", cx, |ps| {
                            if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a) {
                                options.push(ListOption { id: format!("opt{}", noshape_rand()), name });
                            }
                        });
                    });
                    let empty = st.read(cx).value().is_empty();
                    section = section.child(
                        h_flex().border_l_1().border_color(grid).child(
                            cell(NAME_W)
                                .relative()
                                .child(Input::new(&st).appearance(false).text_size(px(14.)))
                                .when(empty, |d| d.child(div().absolute().left(px(6.)).top_0().bottom_0().flex().items_center().italic().text_size(px(14.)).text_color(pal.text_faint).child("Name"))),
                        ),
                    );
                }
                if columns.is_empty() {
                    section = section.child(
                        div().pt(px(5.)).pr(px(5.)).child(
                            v_flex()
                                .p(px(14.))
                                .rounded(px(5.25))
                                .bg(pal.info_fill)
                                .border_1()
                                .border_color(pal.info_outline)
                                .text_size(px(14.))
                                .line_height(px(21.))
                                .text_color(pal.info_text)
                                .child(div().font_weight(FontWeight::BOLD).child("No features are configured"))
                                .child("Right click sketch dimensions, features, or feature parameters to configure them."),
                        ),
                    );
                }
            }
        }
        section.into_any_element()
    }

    /// A table/title text that turns into an input on double-click (or from
    /// "Rename"); Enter or a click elsewhere commits.
    fn config_name_cell(&mut self, key: String, text: String, (size, weight): (Pixels, FontWeight), window: &mut Window, cx: &mut Context<Self>, commit: impl Fn(&mut Self, String, &mut Context<Self>) + 'static) -> AnyElement {
        if self.ui.cfg_editing.as_deref() == Some(key.as_str()) {
            let k = key.clone();
            let st = self.cell_input(key.clone(), text, window, cx, move |this, v, cx| {
                if this.ui.cfg_editing.as_deref() == Some(k.as_str()) {
                    this.ui.cfg_editing = None;
                }
                commit(this, v, cx);
            });
            if !st.read(cx).focus_handle(cx).is_focused(window) && self.panel_focused.as_deref() != Some(key.as_str()) {
                st.update(cx, |s, cx| s.focus(window, cx));
            }
            return div().flex_1().min_w_0().child(Input::new(&st).appearance(false).text_size(size).font_weight(weight)).into_any_element();
        }
        div()
            .id(SharedString::from(format!("cfgtxt-{key}")))
            .min_w_0()
            .truncate()
            .text_size(size)
            .font_weight(weight)
            .child(text)
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                if e.click_count() >= 2 {
                    this.ui.cfg_editing = Some(key.clone());
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    /// A configured parameter's value for one configuration: a checkbox for
    /// booleans (Suppressed), editable text otherwise.
    fn configured_value_cell(&mut self, ci: usize, c: &ConfiguredParam, key: &str, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let val = c.values.get(key).cloned();
        let kk = key.to_string();
        if let Some(ParamValue::Bool(b)) = val {
            return div()
                .flex_1()
                .flex()
                .justify_center()
                .child(crate::ui::os_checkbox(SharedString::from(format!("cpv-{ci}-{key}")), "", b).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let kk = kk.clone();
                    this.edit_config("Edit configured value", cx, |ps| {
                        if let Some(cp) = ps.configuration.configured.get_mut(ci) {
                            cp.values.insert(kk, ParamValue::Bool(!b));
                        }
                    });
                })))
                .into_any_element();
        }
        let text = match &val {
            Some(ParamValue::Expr(e)) => e.text.clone(),
            Some(ParamValue::Enum(s)) | Some(ParamValue::Text(s)) => s.clone(),
            _ => String::new(),
        };
        self.config_name_cell(format!("cp-{ci}-{key}"), text, (px(14.), FontWeight::NORMAL), window, cx, move |this, v, cx| {
            let kk = kk.clone();
            this.edit_config("Edit configured value", cx, |ps| {
                if let Some(cp) = ps.configuration.configured.get_mut(ci) {
                    let new = match cp.values.get(&kk) {
                        Some(ParamValue::Enum(_)) => ParamValue::Enum(v.trim().to_string()),
                        Some(ParamValue::Text(_)) => ParamValue::Text(v),
                        _ => ParamValue::Expr(ExprValue::new(v)),
                    };
                    cp.values.insert(kk.clone(), new);
                }
            });
        })
    }

    /// "Configured properties" tab: per configuration, the parts and their
    /// names in that configuration.
    fn render_configured_properties(&self) -> AnyElement {
        let pal = p();
        let grid = gpui_kit::rgb(0xaaaaaa);
        let parts: Vec<String> = self.display.bodies.iter().filter(|b| b.kind != noshape_model::display::BodyKind::Wire).map(|b| b.name.clone()).collect();
        let cell = |w: f32| div().w(px(w)).h(px(34.)).flex_none().px(px(3.)).flex().items_center().border_r_1().border_b_1().border_color(grid).text_size(px(14.));
        let mut col = v_flex().pt(px(6.)).child(h_flex().border_l_1().border_t_1().border_color(grid).child(cell(180.).bg(gpui_kit::rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD).child("Part")).child(cell(180.).bg(gpui_kit::rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD).child("Name")));
        for name in parts {
            col = col.child(h_flex().border_l_1().border_color(grid).child(cell(180.).child(name.clone())).child(cell(180.).text_color(pal.text_muted).child(name)));
        }
        col.into_any_element()
    }

    /// The "Configurations" box at the top of the feature list of a
    /// configured Part Studio (live part-studio/182, 183): a bold title, a
    /// [▦][▾] split button (show the Configurations panel | add an input),
    /// then one row per input: its name and the current value.
    pub(super) fn render_config_box(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let cfg = self.studio(cx)?.configuration.clone();
        if cfg.inputs.is_empty() {
            return None;
        }
        let border = gpui_kit::rgb(0xaaaaaa);
        let view = cx.entity();
        let split = h_flex()
            .rounded(px(3.))
            .border_1()
            .border_color(border)
            .child(
                // Live (dom/panel-configuration-left-box): 35 x 28 and 20 x 28
                // halves in a 1 px `#aaa` box, white even while the panel is open.
                div()
                    .id("cfgbox-panel")
                    .w(px(35.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("config-show", 20., cx))
                    .tooltip(crate::ui::tooltip("Configurations"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_panel(RightPanel::Configurations, window, cx))),
            )
            .child(
                div()
                    .id("cfgbox-more")
                    .w(px(20.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_l_1()
                    .border_color(border)
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let v = view.clone();
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                            for (label, icon, kind) in [("List", "table", "List"), ("Checkbox", "checkbox-input", "Checkbox"), ("Configuration variable", "variable", "Variable")] {
                                let v = v.clone();
                                m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_config_input(kind, cx))));
                            }
                            m
                        });
                        crate::os_menu::open(e.position(), menu, window, cx);
                    }),
            );
        let mut col = v_flex()
            .flex_none()
            .px(px(5.))
            .pt(px(6.))
            // Live (183): 19 px under the last input to the line.
            .pb(px(18.))
            .border_b_1()
            .border_color(pal.border)
            .child(h_flex().h(px(30.)).justify_between().child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Configurations")).child(split));
        for input in &cfg.inputs {
            let current = cfg.value_of(&self.config_values, &input.id).unwrap_or_default();
            let control: AnyElement = match &input.kind {
                ConfigInputKind::List { options, .. } => {
                    let items = options.iter().map(|o| (SharedString::from(o.id.clone()), SharedString::from(o.name.clone()))).collect();
                    let sel = match self.ui.cfg_selects.get(&input.id) {
                        Some((s, _)) => s.clone(),
                        None => {
                            let s = cx.new(|_| crate::os_select::OsSelect::new(SharedString::from(format!("cfgsel-{}", input.id))));
                            let iid = input.id.clone();
                            let sub = cx.subscribe(&s, move |this, _, ev: &crate::os_select::OsSelectEvent, cx| {
                                let crate::os_select::OsSelectEvent::Picked(id) = ev;
                                this.config_values.0.insert(iid.clone(), id.to_string());
                                crate::release_dialog::note_viewed_config(this.tab, &this.config_values);
                                this.config_dirty(cx);
                            });
                            self.ui.cfg_selects.insert(input.id.clone(), (s.clone(), sub));
                            s
                        }
                    };
                    sel.update(cx, |s, _| s.set_items(items, current.clone()));
                    sel.into_any_element()
                }
                ConfigInputKind::Checkbox { .. } => {
                    let on = current == "true";
                    let iid = input.id.clone();
                    crate::ui::os_checkbox(SharedString::from(format!("cfgbox-cb-{}", input.id)), "", on)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.set_config_value(&iid, (!on).to_string(), window, cx)))
                        .into_any_element()
                }
                ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => div().text_size(px(14.)).text_color(pal.text).child(current).into_any_element(),
            };
            col = col.child(
                h_flex()
                    .min_h(px(26.))
                    .gap(px(4.))
                    .child(div().w(px(80.)).flex_none().pl(px(4.)).text_size(px(12.)).text_color(pal.text_muted).truncate().child(input.name.clone()))
                    .child(div().flex_1().min_w_0().child(control)),
            );
        }
        Some(col.into_any_element())
    }

    /// An empty panel's illustration and help text with its "Learn more"
    /// link (live part-studio/77).
    fn panel_empty_state(&self, illustration: &'static str, size: f32, lines: &[&'static str], link: (&'static str, &'static str), url: &'static str, cx: &mut Context<Self>) -> Div {
        let pal = p();
        let mut text = v_flex().mt(px(16.)).items_center().text_size(px(13.33)).line_height(px(20.)).text_color(pal.text_muted);
        for (i, l) in lines.iter().enumerate() {
            if i + 1 == lines.len() {
                text = text.child(
                    h_flex().child(format!("{l}\u{a0}")).child(div().id("panel-help-1").font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0x094174)).underline().cursor_pointer().child(link.0).on_click(move |_, _, cx| cx.open_url(url))),
                );
            } else {
                text = text.child(*l);
            }
        }
        text = text.child(div().id("panel-help-2").font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0x094174)).underline().cursor_pointer().child(link.1).on_click(move |_, _, cx| cx.open_url(url)));
        v_flex().items_center().child(crate::assets::icon(illustration, size, cx)).child(text)
    }

    /// Custom tables (live part-studio/77): "Add custom tables" and the
    /// empty-state help.
    fn render_custom_tables(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        v_flex()
            .items_center()
            .pt(px(4.))
            .child(
                h_flex()
                    .id("add-custom-tables")
                    .h(px(28.))
                    .px(px(8.))
                    .gap(px(4.))
                    .rounded(px(3.))
                    .border_1()
                    .border_color(gpui_kit::rgb(0xaaaaaa))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("featurescript-table", 18., cx))
                    .child(div().text_size(px(14.)).text_color(pal.text).child("Add custom tables"))
                    .on_click(|e: &ClickEvent, window, cx| {
                        let menu = crate::os_menu::OsMenu::build(window, cx, |m, _, _| m.item(crate::os_menu::OsMenuItem::new("No custom tables in this document").disabled(true)));
                        crate::os_menu::open(e.position(), menu, window, cx);
                    }),
            )
            .child(div().mt(px(95.)).child(self.panel_empty_state(
                "custom-tables-illustration",
                160.,
                &[
                    "Custom tables allow you to view information from the current",
                    "Part Studio. They are defined using FeatureScript. Sample or",
                    "community-created tables can be added. When a table is added",
                    "to this panel, it will be available in all documents that you have",
                    "access to. Learn more about custom tables.",
                ],
                ("Learn more about", "custom tables."),
                "https://cad.onshape.com/help/Content/customtables.htm",
                cx,
            )))
            .into_any_element()
    }

    /// Inspection table (live part-studio/77): "Model  No models.", the
    /// inspection tool row and the empty-state help.
    fn render_inspection(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        v_flex()
            .child(
                h_flex()
                    .h(px(30.))
                    .px(px(6.))
                    .gap(px(8.))
                    .text_size(px(13.33))
                    .child(div().text_color(pal.text).child("Model"))
                    .child(div().flex_1().text_color(pal.text).child("No models."))
                    .child(crate::assets::icon("more", 20., cx)),
            )
            .child(h_flex().h(px(36.)).px(px(6.)).gap(px(6.)).bg(gpui_kit::rgb(0xf5f5f5)).child(crate::assets::icon("inspection-item", 20., cx)))
            .child(div().mt(px(60.)).child(self.panel_empty_state(
                "inspection-illustration",
                130.,
                &[
                    "Add tolerances to sketch dimensions and feature parameters, or",
                    "add annotations and dimensions with the Inspection panel tools",
                    "to see data on any part in the Part Studio.",
                ],
                ("Learn more about", "Inspection data."),
                "https://cad.onshape.com/help/Content/inspection.htm",
                cx,
            )))
            .into_any_element()
    }

    /// Bottom bar of the configured panel: "[▦ Add configuration input][▲]".
    fn render_add_config_input(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let border = gpui_kit::rgb(0xaaaaaa);
        let view = cx.entity();
        let main = div()
            .id("cfg-add-input")
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::assets::icon("table", 16., cx))
            .child(div().text_size(px(14.)).text_color(pal.text).child("Add configuration input"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_config_input("List", cx)));
        let caret = div()
            .id("cfg-add-input-more")
            .w(px(22.))
            .h(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .border_l_1()
            .border_color(border)
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::ui::glyph("ui-caret-up", 10., pal.text))
            .on_click(move |e: &ClickEvent, window, cx| {
                let v = view.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    for (label, icon, kind) in [("List", "table", "List"), ("Checkbox", "checkbox-input", "Checkbox"), ("Configuration variable", "variable", "Variable")] {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_config_input(kind, cx))));
                    }
                    m
                });
                crate::os_menu::open_anchored(e.position(), Anchor::BottomLeft, menu, window, cx);
            });
        h_flex().rounded(px(3.)).border_1().border_color(border).bg(pal.panel).child(main).child(caret).into_any_element()
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
        crate::release_dialog::note_viewed_config(self.tab, &self.config_values);
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
                // Live (00, 183): white while no panel is open; with one
                // open the others turn `#eaeaea` and the open one stays white
                // and joins the panel (no right border).
                .bg(if on || active.is_none() { pal.panel } else { gpui_kit::rgb(0xeaeaea) })
                .hover(|d| d.bg(pal.panel_hover))
                .cursor_pointer()
                .border_1()
                .border_color(pal.border_strong)
                .when(on, |d| d.border_r_0())
                .child(crate::assets::icon(icon, 20., cx))
                .tooltip(crate::ui::tooltip(tip))
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.toggle_panel(panel, w, cx)))
        };
        v_flex()
            .pt(px(1.))
            .mr(px(-1.))
            // Live order (part-studio/77): Appearance, Configuration, Custom
            // tables, Inspection table, Variable table. An empty Part Studio
            // has nothing to color: no Appearance button (live 00).
            .when(!self.display.bodies.is_empty() || !self.display.sketches.is_empty(), |d| d.child(btn("panel-appearance", "appearance", "Appearance panel", RightPanel::Appearance, cx)))
            .child(btn("panel-config", "configuration", "Configuration panel", RightPanel::Configurations, cx))
            .child(btn("panel-custom", "featurescript-table", "Custom tables", RightPanel::CustomTables, cx))
            .child(btn("panel-inspection", "inspection-table", "Inspection table", RightPanel::Inspection, cx))
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

