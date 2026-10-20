//! Right-hand Part Studio panel: variable table and configurations
//! (inputs, current configuration, configured feature parameters).

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use crate::ui::TipExt as _;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, ListOption, VariableKind};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_units::ExprValue;


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
/// An input ⋯ menu entry's action.
type MenuAction = Box<dyn Fn(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;

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

    pub(super) fn edit_config(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut PartStudio)) {
        let mut repaired = Vec::new();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, desc, cx, |ps| {
                f(ps);
                // Help "Repairing visibility conditions": conditions naming
                // deleted inputs or options are removed, with a warning.
                repaired = ps.configuration.repair_visibility();
            });
        });
        if !repaired.is_empty() {
            self.flash_info(format!("Visibility conditions for {} were updated", repaired.join(", ")), cx);
        }
    }

    /// "Configuration visibility conditions" for every input (`input` None)
    /// or one; Save stores the inputs' conditions.
    pub(super) fn open_visibility_conditions(&mut self, input: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cfg) = self.studio(cx).map(|s| s.configuration.clone()) else { return };
        let view = cx.entity().downgrade();
        let values = self.config_values.clone();
        crate::config_visibility::open(
            cfg,
            values,
            input,
            move |edited, _, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.edit_config("Edit visibility conditions", cx, |ps| {
                        for i in &mut ps.configuration.inputs {
                            if let Some(e) = edited.inputs.iter().find(|e| e.id == i.id) {
                                i.visible_when = e.visible_when.clone();
                            }
                        }
                    });
                    this.config_dirty(cx);
                });
            },
            window,
            cx,
        );
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
                // A refused value isn't committed and keeps the focus (live
                // Variable table: Enter on an invalid name does nothing).
                if this.panel_checks.get(&k).is_some_and(|ok| !ok(&v)) {
                    return;
                }
                // Committed: losing the focus later must not commit it again
                // (qa4 Fixed 9: Enter then the Blur that follows added an
                // add-row variable twice). An unchanged value commits nothing.
                let last = this.panel_inputs.get(&k).map(|e| e.1.clone());
                if let Some(e) = this.panel_inputs.get_mut(&k) {
                    e.1 = v.clone();
                }
                if last.as_deref() != Some(v.as_str()) {
                    commit(this, v, cx);
                }
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
                        .child(div().flex_1().flex().justify_center().children((panel == RightPanel::Variables).then(|| self.variable_panel_footer(cx))).children(if panel == RightPanel::CustomTables { self.custom_tables_footer(cx) } else { None }).children(configured.then(|| self.render_add_config_input(cx))))
                        .child(div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?")),
                )
                // Add custom tables flyout over the panel's top right (live 639).
                .relative()
                .children(self.render_add_tables(cx))
                .into_any_element(),
        )
    }

    /// Onshape's Variable table (live, part-studio/77): a gray "⌄ Part
    /// Studio 1 ⋯" section header, then a table Name | Variable type | Value
    /// | Description (98 px columns, bold headers, `#aaa` grid); with no
    /// variables, a large faint illustration and a hint.
    /// The "Create variable" popup's ✓ (live part-studio/153): a Variable
    /// feature placed before the feature being edited (new or existing), or
    /// a configuration variable.
    pub(super) fn create_variable_from_field(&mut self, v: &crate::feature_dialog::NewVariable, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let (name, kind, value, description) = (v.name.as_str(), v.kind, v.value.as_str(), v.description.as_str());
        if v.configuration {
            let vkind = match kind {
                "ANGLE" => VariableKind::Angle,
                "INTEGER" => VariableKind::Integer,
                "REAL" => VariableKind::Real,
                "TEXT" => VariableKind::Text,
                _ => VariableKind::Length,
            };
            let (name, value) = (name.to_string(), value.to_string());
            let (min, max) = match v.range.clone() {
                Some((lo, hi)) => (Some(ExprValue::new(lo)), Some(ExprValue::new(hi))),
                None => (None, None),
            };
            self.edit_config("Add configuration variable", cx, |ps| {
                let taken: Vec<String> = ps.configuration.inputs.iter().map(|i| i.id.clone()).collect();
                let id = noshape_model::configuration::fs_identifier(&name, &taken);
                ps.configuration.inputs.push(ConfigInput { id, name, kind: ConfigInputKind::Variable { kind: vkind, default: ExprValue::new(value), min, max }, visible_when: None, excluded: false });
            });
        } else {
            // A new feature is drafted at `index` too, so the variable takes
            // that place and the draft moves after it.
            let at = self.edit.as_ref().map(|e| e.index);
            let mut res = Ok(noshape_model::FeatureId(0));
            self.edit_config("Create variable", cx, |ps| res = noshape_model::variable_table::create(ps, name, kind, value, description, at, &units));
            self.variable_name_error(res.err(), name, cx);
            // The edited feature moved down one place.
            if let (Some(_), Some(e)) = (at, self.edit.as_mut()) {
                e.index += 1;
            }
        }
        self.config_dirty(cx);
    }

    /// Variable table ⋯ › Copy table: the table as tab-separated text (Help:
    /// header rows included).
    fn copy_variable_table(&mut self, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let kind = |k: &str| match k {
            "ANGLE" => "Angle",
            "NUMBER" => "Number",
            "ANY" => "Any",
            "MEASURED" => "Measured",
            "TABLE" => "From import",
            _ => "Length",
        };
        let mut lines = vec!["Name\tVariable type\tValue\tDescription".to_string()];
        for r in noshape_model::variable_table::active_rows(studio) {
            lines.push(format!("{}\t{}\t{}\t{}", r.name, kind(&r.kind), r.value, r.description));
        }
        cx.write_to_clipboard(ClipboardItem::new_string(lines.join("\n")));
    }

    /// A refused variable name (Help: names must be FeatureScript
    /// identifiers; an existing name is not overwritten).
    fn variable_name_error(&mut self, err: Option<noshape_model::variable_table::NameError>, name: &str, cx: &mut Context<Self>) {
        use noshape_model::variable_table::NameError;
        let text = match err {
            None => return,
            Some(NameError::Invalid) => format!("\"{name}\" is not a valid variable name. Use a letter or underscore followed by letters, digits or underscores."),
            Some(NameError::Duplicate) => format!("A variable named \"{name}\" already exists. The variable was not overwritten."),
        };
        self.message = Some((noshape_model::display::FeatureStatus::Warning, text));
        cx.notify();
    }

    fn render_variables(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
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
            // ⌄ folds the table; ⋯ = Copy table only (live part-studio/562).
            .child(
                div()
                    .id("pvar-fold")
                    .cursor_pointer()
                    .child(crate::ui::glyph(if self.ui.var_folded { "ui-chevron-right" } else { "ui-chevron-down" }, 14., pal.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ui.var_folded = !this.ui.var_folded;
                        cx.notify();
                    })),
            )
            .child(div().flex_1().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(tab_name))
            .child({
                let v1 = cx.entity();
                div().id("pvar-more").cursor_pointer().child(crate::assets::icon("more", 20., cx)).on_click(move |e: &ClickEvent, window, cx| {
                    let v1 = v1.clone();
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                        let v1 = v1.clone();
                        m.item(crate::os_menu::OsMenuItem::new("Copy table").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.copy_variable_table(cx))))
                    });
                    crate::os_menu::open(e.position(), menu, window, cx);
                })
            });
        // Live (164): only the header cells are gray, not the rest of the row.
        let columns = h_flex().border_l_1().border_color(grid).children(["Name", "Variable type", "Value", "Description"].map(|h| cell(h.into(), true).text_size(px(14.)).bg(gpui_kit::rgb(0xeaeaea))));
        // The Part Studio's own variables are its Variable features (Help
        // PartStudio__variable_table); every cell edits them.
        // Only the variables before the rollback bar (live 10-20 o).
        let var_rows = self.studio(cx).map(noshape_model::variable_table::active_rows).unwrap_or_default();
        let count = var_rows.len();
        let input_cell = |st: &Entity<InputState>| gpui_kit::base::input::input_frame(div().w(px(140.)).h(px(34.)).flex_none().flex().items_center().px(px(2.)).border_r_1().border_b_1().border_color(grid).child(Input::new(st).appearance(false).text_size(px(13.33)).pl(px(4.))));
        // A Name cell holding an invalid name: pink with a red border and the
        // reason as tooltip (live part-studio/554–555).
        let name_cell = |st: &Entity<InputState>, cx: &App| {
            let v = st.read(cx).value().to_string();
            let bad = !v.trim().is_empty() && !noshape_model::variable_table::is_identifier(v.trim().trim_start_matches('#'));
            div()
                .id(SharedString::from(format!("pvar-cell-{}", st.entity_id())))
                .when(bad, |d| d.bg(gpui_kit::rgb(0xf9ebec)).border_1().border_color(gpui_kit::rgb(0xbd3039)).tooltip(crate::ui::tooltip("Variable name must be a letter followed by a string of letters or numbers.")))
                .child(input_cell(st))
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        for (k, r) in var_rows.into_iter().enumerate() {
            let fid = r.feature;
            // Renaming to a taken name silently reverts (live 556–557).
            let name_st = self.cell_input(format!("pvar-name-{fid}"), r.name.clone(), window, cx, move |this, v, cx| {
                if v.trim().is_empty() {
                    return;
                }
                this.edit_config("Rename variable", cx, |ps| {
                    let _ = noshape_model::variable_table::rename(ps, fid, &v);
                });
                this.config_dirty(cx);
                cx.notify();
            });
            self.panel_checks.insert(format!("pvar-name-{fid}"), valid_variable_name);
            let value_st = self.cell_input(format!("pvar-value-{fid}"), r.value.clone(), window, cx, move |this, v, cx| {
                let units = this.doc.read(cx).session.doc.settings.units;
                this.edit_config("Edit variable", cx, |ps| noshape_model::variable_table::set_value(ps, fid, &v, &units));
                this.config_dirty(cx);
            });
            // An expression Description (live 652–656) shows a string
            // literal's text, other expressions as typed, and nothing with
            // the tooltip "Error regenerating." when it doesn't evaluate.
            let desc_error = r.description_expr && noshape_model::variable_table::string_literal(&r.description).is_none() && !r.description.trim().is_empty() && noshape_units::evaluate(&r.description, &self.display.variables).is_err();
            let desc_shown = if desc_error { String::new() } else { noshape_model::variable_table::string_literal(&r.description).filter(|_| r.description_expr).map(str::to_string).unwrap_or_else(|| r.description.clone()) };
            let desc_st = self.cell_input(format!("pvar-desc-{fid}"), desc_shown, window, cx, move |this, v, cx| {
                this.edit_config("Edit variable", cx, |ps| noshape_model::variable_table::set_description(ps, fid, &v));
            });
            // Live part-studio/558.
            const KINDS: [(&str, &str); 6] = [("LENGTH", "Length"), ("ANGLE", "Angle"), ("NUMBER", "Number"), ("ANY", "Any"), ("MEASURED", "Measured"), ("TABLE", "From import")];
            let current = KINDS.iter().position(|(k, _)| *k == r.kind).unwrap_or(0);
            let v_kind = cx.entity();
            let kind_select = crate::os_ui::select_field(SharedString::from(format!("pvar-kind-{fid}")), KINDS.iter().map(|(_, n)| n.to_string()).collect(), current, move |i, _, cx| {
                let kind = KINDS[i].0;
                v_kind.update(cx, |this, cx| {
                    let units = this.doc.read(cx).session.doc.settings.units;
                    this.edit_config("Edit variable", cx, |ps| noshape_model::variable_table::set_kind(ps, fid, kind, &units));
                    this.config_dirty(cx);
                })
            })
            .h(px(32.))
            .border_0()
            .rounded(px(0.));
            let (n1, name, n2) = (r.name.clone(), r.name.clone(), r.name.clone());
            let view = cx.entity();
            // Measured / From import: the evaluated value, or "Not set", and
            // a ✎ that opens the feature (live 559).
            let value_cell = if matches!(r.kind.as_str(), "MEASURED" | "TABLE") {
                let shown = self.display.variables.get(&r.name).map(|q| q.to_string()).unwrap_or_else(|| "Not set".into());
                h_flex()
                    .w(px(140.))
                    .h(px(34.))
                    .flex_none()
                    .px(px(5.))
                    .border_r_1()
                    .border_b_1()
                    .border_color(grid)
                    .child(div().flex_1().min_w_0().truncate().text_size(px(13.33)).text_color(pal.text).child(shown))
                    .child(
                        div()
                            .id(SharedString::from(format!("pvar-pencil-{fid}")))
                            .cursor_pointer()
                            .child(crate::ui::glyph("edit", 14., pal.text_muted))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.edit_feature(fid, window, cx))),
                    )
                    .into_any_element()
            } else {
                input_cell(&value_st).into_any_element()
            };
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("var-row-{fid}")))
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
                    // Help: Filter feature list, Delete, Move up / down; the row
                    // menu also over the cells' inputs (qa4 Fixed 11).
                    .child(
                        h_flex()
                            .child(name_cell(&name_st, cx))
                            .child(div().w(px(140.)).h(px(34.)).flex_none().border_r_1().border_b_1().border_color(grid).child(kind_select))
                            .child(value_cell)
                            .os_context_menu_over_inputs(move |menu, _, _| var_row_menu(menu, view.clone(), fid, &n2, k, count)),
                    )
                    // Right-clicking the Description adds Copy | Convert to
                    // expression (or text) (live 563, 652–656).
                    .child({
                        let (view, text, expr) = (cx.entity(), r.description.clone(), r.description_expr);
                        let tip = if desc_error { Some("Error regenerating.".to_string()) } else { r.description_expr.then(|| r.description.clone()).filter(|d| !d.is_empty()) };
                        div()
                            .id(SharedString::from(format!("pvar-desc-cell-{fid}")))
                            .child(input_cell(&desc_st))
                            .when_some(tip, |d, t| d.tooltip(crate::ui::tooltip(t)))
                            .os_context_menu_over_inputs(move |menu, _, _| {
                                let (text, v) = (text.clone(), view.clone());
                                var_row_menu(menu, view.clone(), fid, &name, k, count)
                                    .item(crate::os_menu::OsMenuItem::new("Copy").on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))))
                                    .item(crate::os_menu::OsMenuItem::new(if expr { "Convert to text" } else { "Convert to expression" }).on_click(move |_, _, cx| {
                                        v.update(cx, |this, cx| {
                                            this.edit_config("Edit variable", cx, |ps| noshape_model::variable_table::toggle_description_expr(ps, fid));
                                            this.config_dirty(cx);
                                        })
                                    }))
                            })
                    })
                    .into_any_element(),
            );
        }
        // The italic "Name" row: typing a name and Enter (or Tab) creates a
        // Length variable, 0 in the document unit (Help steps 3–4).
        // A taken name is created as `name_0` with an info bubble; the focus
        // then moves to the next add row (live 550–554).
        let new_key = format!("pvar-new-{count}");
        let new_st = self.cell_input(new_key.clone(), String::new(), window, cx, move |this, v, cx| {
            let name = v.trim().to_string();
            if name.is_empty() {
                return;
            }
            let units = this.doc.read(cx).session.doc.settings.units;
            let mut renamed = false;
            this.edit_config("Add variable", cx, |ps| {
                let unique = noshape_model::variable_table::unique_name(ps, &name);
                renamed = unique != name.trim_start_matches('#');
                let _ = noshape_model::variable_table::create(ps, &unique, "LENGTH", &noshape_model::variable_table::default_value("LENGTH", &units), "", None, &units);
            });
            if renamed {
                this.message = Some((noshape_model::display::FeatureStatus::Info, "Renamed some new variables to avoid name conflicts.".into()));
            }
            this.ui.var_focus_new = true;
            this.config_dirty(cx);
        });
        self.panel_checks.insert(new_key, valid_variable_name);
        if std::mem::take(&mut self.ui.var_focus_new) {
            new_st.update(cx, |s, cx| s.focus(window, cx));
        }
        // The input draws the italic "Name" itself, so the caret starts on it.
        crate::ui::ensure_placeholder(&new_st, "Name", cx);
        let new_row = h_flex().border_l_1().border_color(grid).child(
            div()
                .relative()
                .child(name_cell(&new_st, cx))
                // Esc clears the add row (live 555).
                .on_key_down({
                    let st = new_st.clone();
                    move |e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key == "escape" {
                            st.update(cx, |s, cx| s.set_value("", window, cx));
                        }
                    }
                }),
        );
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
            .when(!self.ui.var_folded, |d| d.child(columns).children(rows).child(new_row))
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
            let taken: Vec<String> = ps.configuration.inputs.iter().map(|i| i.id.clone()).collect();
            let kind = match label {
                "Checkbox" => ConfigInputKind::Checkbox { default: false },
                "Variable" => ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("10 mm"), min: None, max: None },
                _ => ConfigInputKind::List {
                    // Live (part-studio/182): a new List has the one "Default" row.
                    options: vec![ListOption { id: "Default".into(), name: "Default".into() }],
                    default: "Default".into(),
                },
            };
            let name = if label == "List" && n == 1 { "Configuration".to_string() } else { format!("{label} {n}") };
            // FeatureScript ids from the names (Edit FeatureScript IDs… shows them).
            let id = noshape_model::configuration::fs_identifier(&name, &taken);
            ps.configuration.inputs.push(ConfigInput { id, name, kind, visible_when: None, excluded: false });
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
                    // Help config-paste-inputs: Paste inputs once something was copied.
                    if !crate::config_visibility::copied_inputs().is_empty() {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new("Paste inputs").icon_name("paste").on_click(move |_, _, cx| v.update(cx, |this, cx| this.paste_config_inputs(cx))));
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
            return self.render_configured_properties(window, cx);
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
        let studio_inputs: Vec<String> = self.studio(cx).map(|s| s.configuration.inputs.iter().map(|i| i.id.clone()).collect()).unwrap_or_default();
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
                        .child(crate::ui::input_with_placeholder(&filter, "Filter inputs", cx).appearance(false).small().text_size(px(13.33)).pl(px(6.))),
                )
                .child(toggles)
                .child({
                    // Help config-panel-context: Copy all inputs, Collapse /
                    // Expand all inputs.
                    let (v1, v2) = (cx.entity(), cx.entity());
                    let all_folded = studio_inputs.iter().all(|id| self.ui.cfg_folded.contains(id));
                    let ids = studio_inputs.clone();
                    div().id("cfg-panel-more").ml(px(6.)).size(px(26.)).flex().items_center().justify_center().cursor_pointer().child(crate::assets::icon("more", 20., cx)).on_click(move |e: &ClickEvent, window, cx| {
                        let (v1, v2, ids) = (v1.clone(), v2.clone(), ids.clone());
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                            let (v1, v2, ids) = (v1.clone(), v2.clone(), ids.clone());
                            // Live (434): Copy all inputs | Collapse / Expand all
                            // inputs (two or more inputs) | Visibility conditions… |
                            // Input propagation….
                            let (v3, v4) = (v1.clone(), v1.clone());
                            let several = ids.len() > 1;
                            let mut m = m.item(crate::os_menu::OsMenuItem::new("Copy all inputs").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.copy_config_inputs(None, cx))));
                            if several {
                                m = m.item(crate::os_menu::OsMenuItem::new(if all_folded { "Expand all inputs" } else { "Collapse all inputs" }).on_click(move |_, _, cx| {
                                    let ids = ids.clone();
                                    v2.update(cx, |this, cx| {
                                        if all_folded {
                                            this.ui.cfg_folded.clear();
                                        } else {
                                            this.ui.cfg_folded.extend(ids);
                                        }
                                        cx.notify();
                                    })
                                }));
                            }
                            m.item(crate::os_menu::OsMenuItem::new("Visibility conditions…").on_click(move |_, window, cx| v3.update(cx, |this, cx| this.open_visibility_conditions(None, window, cx))))
                                .item(crate::os_menu::OsMenuItem::new("Input propagation…").on_click(move |_, window, cx| v4.update(cx, |this, cx| this.open_input_propagation(None, window, cx))))
                        });
                        crate::os_menu::open(e.position(), menu, window, cx);
                    })
                }),
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

    /// A configuration variable's table (live 433): "Usages ①" over one row
    /// per use ("Extrude 1 Depth"), or the "No features are configured" note.
    fn render_usages(&self, section: Div, name: &str, grid: Rgba, cx: &mut Context<Self>) -> Div {
        let pal = p();
        let usages = self.variable_usages(name, cx);
        if usages.is_empty() {
            return section.child(
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
                        .child(div().font_weight(FontWeight::BOLD).child("No features are configured")),
                ),
            );
        }
        let cell = || div().w(px(140.)).h(px(34.)).flex_none().px(px(3.)).flex().items_center().border_r_1().border_b_1().border_color(grid).text_size(px(14.));
        let badge = div().size(px(16.)).rounded_full().bg(gpui_kit::rgb(0x999999)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child(usages.len().to_string());
        let mut out = section.child(h_flex().border_l_1().border_color(grid).child(cell().bg(gpui_kit::rgb(0xeaeaea)).justify_center().gap(px(4.)).font_weight(FontWeight::BOLD).child("Usages").child(badge)));
        for u in usages {
            out = out.child(h_flex().border_l_1().border_color(grid).child(cell().child(div().truncate().child(u))));
        }
        out
    }

    fn render_config_input(&mut self, studio: &PartStudio, input: &ConfigInput, grid: Rgba, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let cfg = &studio.configuration;
        let iid = input.id.clone();
        let folded = self.ui.cfg_folded.contains(&iid);
        let current = cfg.value_of(&self.config_values, &iid).unwrap_or_default();
        let default_text = match &input.kind {
            ConfigInputKind::List { options, default } => options.iter().find(|o| &o.id == default).map(|o| o.name.clone()).unwrap_or_default(),
            // Live 433: "Hole Default: false".
            ConfigInputKind::Checkbox { default } => default.to_string(),
            ConfigInputKind::Variable { default, .. } => default.text.clone(),
            ConfigInputKind::Text { default } => default.clone(),
        };
        // Live 433: variables read "Len (#Len)".
        let variable_ref = matches!(input.kind, ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. }).then(|| format!("(#{})", input.name));
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
            // An input excluded from properties shows
            // `configuration-exclude-properties` between the chevron and the
            // name (live part-studio/531).
            .when(input.excluded, |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!("cfg-excluded-{iid}")))
                        .size(px(25.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::assets::icon("configuration-exclude-properties", 20., cx))
                        .tooltip(crate::ui::tooltip("Excluded from affecting configured properties")),
                )
            })
            .child(self.config_name_cell(format!("in-name-{iid}"), input.name.clone(), (px(16.), FontWeight::MEDIUM), window, cx, move |this, v, cx| {
                let id = i2.clone();
                this.edit_config("Rename configuration input", cx, |ps| {
                    if let Some(i) = ps.configuration.inputs.iter_mut().find(|i| i.id == id) {
                        i.name = v;
                    }
                });
            }))
            .children(variable_ref.map(|r| div().flex_none().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(r)))
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
            // Help: an input with conditions shows the Visibility conditions
            // icon; clicking it opens the dialog for that input.
            .when(input.visible_when.is_some(), |d| {
                let vid = iid.clone();
                d.child(
                    div()
                        .id(SharedString::from(format!("cfg-vis-{iid}")))
                        .size(px(25.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(crate::assets::icon("configuration-visibiliity", 20., cx))
                        .tooltip(crate::ui::tooltip("Modify visibility conditions"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_visibility_conditions(Some(vid.clone()), window, cx))),
                )
            })
            .child({
                let (vm, mid) = (view.clone(), iid.clone());
                let index = studio.configuration.inputs.iter().position(|i| i.id == iid).unwrap_or(0);
                let count = studio.configuration.inputs.len();
                let excluded = input.excluded;
                div()
                    .id(SharedString::from(format!("cfg-more-{iid}")))
                    .size(px(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(crate::assets::icon("more", 20., cx))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let (v, id) = (vm.clone(), mid.clone());
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                            // Live (Help input-prop-contextmenu, config-copy-table):
                            // Copy table, Copy input, Move up / down, Rename,
                            // Visibility conditions…, Input propagation…, Delete.
                            let item = |label: &str, f: MenuAction| {
                                let v = v.clone();
                                crate::os_menu::OsMenuItem::new(label.to_string()).on_click(move |_, window, cx| v.update(cx, |this, cx| f(this, window, cx)))
                            };
                            let i1 = id.clone();
                            m = m.item(item("Copy table", Box::new(move |this, _, cx| this.copy_config_table(&i1, cx))));
                            let i2 = id.clone();
                            m = m.item(item("Copy input", Box::new(move |this, _, cx| this.copy_config_inputs(Some(&i2), cx))));
                            if index > 0 {
                                let i3 = id.clone();
                                m = m.item(item("Move up", Box::new(move |this, _, cx| this.move_config_input(&i3, -1, cx))));
                            }
                            if index + 1 < count {
                                let i4 = id.clone();
                                m = m.item(item("Move down", Box::new(move |this, _, cx| this.move_config_input(&i4, 1, cx))));
                            }
                            let i5 = id.clone();
                            m = m.item(item("Rename", Box::new(move |this, _, cx| {
                                this.ui.cfg_editing = Some(format!("in-name-{i5}"));
                                this.ui.cfg_editing_focus = true;
                                cx.notify();
                            })));
                            // Live (434): Edit FeatureScript IDs…, Exclude from /
                            // Include in properties (bundle `setIsCosmetic`).
                            let i9 = id.clone();
                            m = m.item(item("Edit FeatureScript IDs…", Box::new(move |this, window, cx| this.open_edit_fs_ids(&i9, window, cx))));
                            let i10 = id.clone();
                            m = m.item(item(if excluded { "Include in properties" } else { "Exclude from properties" }, Box::new(move |this, _, cx| {
                                let id = i10.clone();
                                this.edit_config(if excluded { "Include in properties" } else { "Exclude from properties" }, cx, |ps| {
                                    if let Some(i) = ps.configuration.inputs.iter_mut().find(|i| i.id == id) {
                                        i.excluded = !excluded;
                                    }
                                });
                            })));
                            let i6 = id.clone();
                            m = m.item(item("Visibility conditions…", Box::new(move |this, window, cx| this.open_visibility_conditions(Some(i6.clone()), window, cx))));
                            let i7 = id.clone();
                            m = m.item(item("Input propagation…", Box::new(move |this, window, cx| this.open_input_propagation(Some(&i7), window, cx))));
                            let i8 = id.clone();
                            m.item(item("Delete", Box::new(move |this, _, cx| {
                                let id = i8.clone();
                                this.edit_config("Delete configuration input", cx, |ps| {
                                    ps.configuration.inputs.retain(|i| i.id != id);
                                    ps.configuration.configured.retain(|c| c.input != id);
                                });
                            })))
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
            // Live 433: a Value column, ☐ (false) then ☑ (true).
            ConfigInputKind::Checkbox { .. } => vec![("false".into(), "false".into()), ("true".into(), "true".into())],
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
                let value = self.config_name_cell(format!("var-def-{iid}"), default.text.clone(), (px(13.33), FontWeight::NORMAL), window, cx, move |this, v, cx| {
                    let a = a.clone();
                    this.edit_config("Edit configuration variable", cx, |ps| {
                        if let Some(ConfigInput { kind: ConfigInputKind::Variable { default, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a) {
                            *default = ExprValue::new(v);
                        }
                    });
                });
                let _ = value;
                section = self.render_usages(section, &input.name, grid, cx);
            }
            ConfigInputKind::Text { .. } => {
                section = self.render_usages(section, &input.name, grid, cx);
            }
            _ => {
                // Header (live 444): "Name" (or "Value"), then per feature a
                // bold group cell over its parameters' bold cells.
                let first = if matches!(input.kind, ConfigInputKind::Checkbox { .. }) { "Value" } else { "Name" };
                let label_of = |c: &ConfiguredParam| -> String {
                    if let Some(id) = c.param.strip_prefix(noshape_model::regen::features::importing::DERIVED_CONFIG_PREFIX) {
                        // Input propagation: the source input's name ("Size").
                        return self.derived_input_name(c.feature, id, cx);
                    }
                    studio.feature(c.feature).and_then(|f| f.feature_type()).and_then(|t| t.param(&c.param)).map(|p| p.label.to_string()).unwrap_or_else(|| if c.param == noshape_model::configuration::SUPPRESSED_PARAM { "Suppressed".into() } else { "?".into() })
                };
                if columns.is_empty() {
                    section = section.child(h_flex().border_l_1().border_color(grid).child(head(NAME_W).child(first)));
                } else {
                    let mut groups = h_flex();
                    let mut params = h_flex();
                    let mut k = 0;
                    while k < columns.len() {
                        let f = columns[k].1.feature;
                        let n = columns[k..].iter().take_while(|(_, c)| c.feature == f).count();
                        let fname = studio.feature(f).map(|f| f.name.clone()).unwrap_or_else(|| "Deleted feature".into());
                        groups = groups.child(head(COL_W * n as f32).child(div().truncate().child(fname)));
                        for (_, c) in &columns[k..k + n] {
                            params = params.child(head(COL_W).child(div().truncate().child(label_of(c))));
                        }
                        k += n;
                    }
                    section = section.child(
                        h_flex()
                            .border_l_1()
                            .border_color(grid)
                            .child(div().w(px(NAME_W)).h(px(ROW_H * 2.)).flex_none().flex().items_center().justify_center().border_r_1().border_b_1().border_color(grid).bg(gpui_kit::rgb(0xeaeaea)).font_weight(FontWeight::BOLD).text_size(px(14.)).child(first))
                            .child(v_flex().child(groups).child(params)),
                    );
                }
                for (rk, rname) in &rows {
                    let active = *rk == current;
                    let (ia, rka) = (iid.clone(), rk.clone());
                    let name_el: AnyElement = if matches!(input.kind, ConfigInputKind::List { .. }) {
                        let (a, b) = (iid.clone(), rk.clone());
                        self.config_name_cell(format!("opt-{iid}-{rk}"), rname.clone(), (px(13.33), if active { FontWeight::MEDIUM } else { FontWeight::NORMAL }), window, cx, move |this, v, cx| {
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
                        // A checkbox's rows show its states as read-only boxes
                        // (live 433: #999 filled, a white check for true).
                        div()
                            .size(px(12.))
                            .ml(px(1.))
                            .rounded(px(3.))
                            .bg(gpui_kit::rgb(0x999999))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(rk == "true", |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).with_size(px(10.)).text_color(gpui_kit::rgb(0xffffff))))
                            .into_any_element()
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
                                .when(matches!(input.kind, ConfigInputKind::Checkbox { .. }), |d| d.bg(gpui_kit::rgb(0xf4f4f4)))
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
                                let taken: Vec<String> = options.iter().map(|o| o.id.clone()).collect();
                                options.push(ListOption { id: noshape_model::configuration::fs_identifier(&name, &taken), name });
                            }
                        });
                    });
                    section = section.child(
                        h_flex().border_l_1().border_color(grid).child(
                            cell(NAME_W)
                                .relative()
                                .child(crate::ui::input_with_placeholder(&st, "Name", cx).appearance(false).text_size(px(13.33)).pl(px(3.))),
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
            let st = self.cell_input(key.clone(), text.clone(), window, cx, move |this, v, cx| {
                if this.ui.cfg_editing.as_deref() == Some(k.as_str()) {
                    this.ui.cfg_editing = None;
                }
                commit(this, v, cx);
            });
            // Focus once when the rename opens (a dialog opened later keeps
            // the keyboard).
            if std::mem::take(&mut self.ui.cfg_editing_focus) {
                st.update(cx, |s, cx| s.focus(window, cx));
            }
            let (k2, st2, original) = (key.clone(), st.clone(), text.clone());
            return div()
                .flex_1()
                .min_w_0()
                // Escape cancels the rename.
                .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                    if e.keystroke.key == "escape" && this.ui.cfg_editing.as_deref() == Some(k2.as_str()) {
                        // Back to the old name, so the blur commits no change.
                        st2.update(cx, |s, cx| s.set_value(original.clone(), window, cx));
                        this.ui.cfg_editing = None;
                        let fh = this.focus_handle(cx);
                        window.focus(&fh, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(Input::new(&st).appearance(false).text_size(size).font_weight(weight))
                .into_any_element();
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
                    this.ui.cfg_editing_focus = true;
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
        // A propagated input: the Derived source's option, as a dropdown.
        if let Some(src_input) = c.param.strip_prefix(noshape_model::regen::features::importing::DERIVED_CONFIG_PREFIX) {
            let tab = self.studio(cx).and_then(|s| s.feature(c.feature)).map(|f| f.params.text("partStudio").to_string()).unwrap_or_default();
            let options: Vec<(String, String)> = match self.source_input(&tab, src_input, cx).map(|i| i.kind) {
                Some(ConfigInputKind::List { options, .. }) => options.into_iter().map(|o| (o.id, o.name)).collect(),
                Some(ConfigInputKind::Checkbox { .. }) => vec![("true".into(), "true".into()), ("false".into(), "false".into())],
                _ => Vec::new(),
            };
            let current = match &val {
                Some(ParamValue::Text(t)) | Some(ParamValue::Enum(t)) => t.clone(),
                _ => String::new(),
            };
            let label = options.iter().find(|(o, _)| *o == current).map(|(_, n)| n.clone()).unwrap_or(current);
            let view = cx.entity();
            let pal = p();
            return h_flex()
                .id(SharedString::from(format!("cpv-sel-{ci}-{key}")))
                .flex_1()
                .justify_between()
                .cursor_pointer()
                .child(div().truncate().child(label))
                .child(crate::ui::glyph("ui-caret-down", 9., pal.text))
                .on_click(move |e: &ClickEvent, window, cx| {
                    let (v, kk, options) = (view.clone(), kk.clone(), options.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        for (o, n) in &options {
                            let (v, kk, o) = (v.clone(), kk.clone(), o.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new(n.clone()).on_click(move |_, _, cx| {
                                let (kk, o) = (kk.clone(), o.clone());
                                v.update(cx, |this, cx| {
                                    this.edit_config("Edit configured value", cx, |ps| {
                                        if let Some(cp) = ps.configuration.configured.get_mut(ci) {
                                            cp.values.insert(kk, ParamValue::Text(o));
                                        }
                                    });
                                })
                            }));
                        }
                        m
                    });
                    crate::os_menu::open(e.position(), menu, window, cx);
                })
                .into_any_element();
        }
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
        self.config_name_cell(format!("cp-{ci}-{key}"), text, (px(13.33), FontWeight::NORMAL), window, cx, move |this, v, cx| {
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

    /// "Configured properties" tab (Help PartStudio__configured_properties;
    /// live psb 10-20 q, part-studio/600–656): "Properties for <part> ▾",
    /// Add property ▾ and ⋯ over the part's sparse table, or a blue info box
    /// while it has none; a blue line on top when inputs are excluded.
    fn render_configured_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use noshape_model::configured_properties as cp;
        let pal = p();
        let grid = gpui_kit::rgb(0xaaaaaa);
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let parts: Vec<(noshape_model::BodyId, String)> = self.display.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| (b.id.clone(), b.name.clone())).collect();
        let part = self.ui.cp_part.clone().filter(|p| parts.iter().any(|(id, _)| id == p)).or_else(|| parts.first().map(|(id, _)| id.clone()));
        let Some(part) = part else { return div().into_any_element() };
        let part_name = parts.iter().find(|(id, _)| *id == part).map(|(_, n)| n.clone()).unwrap_or_default();
        let table = cp::table(&studio, &part).cloned();
        let view = cx.entity();
        // "Properties for Part 1 ▾": the part list, ✓ before parts with a
        // table (live 618).
        let part_menu = {
            let (view, parts) = (view.clone(), parts.clone());
            let with_table: Vec<noshape_model::BodyId> = studio.property_tables.iter().map(|t| t.part.clone()).collect();
            div()
                .id("cp-part")
                .flex()
                .items_center()
                .gap(px(2.))
                .cursor_pointer()
                .text_color(gpui_kit::rgb(0x6b7b8c))
                .child(part_name.clone())
                .child(crate::ui::glyph("ui-caret-down", 8., gpui_kit::rgb(0x6b7b8c)))
                .on_click(move |e: &ClickEvent, window, cx| {
                    let (view, parts, with_table) = (view.clone(), parts.clone(), with_table.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        for (id, name) in &parts {
                            let (v, id2) = (view.clone(), id.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new(name.clone()).checked(with_table.contains(id)).on_click(move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.ui.cp_part = Some(id2.clone());
                                    this.ui.cp_selected = None;
                                    cx.notify();
                                })
                            }));
                        }
                        m
                    });
                    crate::os_menu::open(e.position(), menu, window, cx);
                })
        };
        let add_menu = {
            let (view, part, name) = (view.clone(), part.clone(), part_name.clone());
            h_flex()
                .id("cp-add-property")
                .h(px(24.))
                .px(px(10.))
                .gap(px(14.))
                .bg(gpui_kit::rgb(0xffffff))
                .border_1()
                .border_color(gpui_kit::rgb(0xbbbbbb))
                .rounded(px(3.))
                .cursor_pointer()
                .text_size(px(13.33))
                .text_color(pal.text)
                .child("Add property")
                .child(crate::ui::glyph("ui-caret-down", 8., pal.text))
                .on_click(move |e: &ClickEvent, window, cx| {
                    let (view, part, name) = (view.clone(), part.clone(), name.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        for prop in cp::PROPERTIES {
                            let (v, part, name) = (view.clone(), part.clone(), name.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new(prop).on_click(move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    let active = this.config_values.clone();
                                    this.edit_config("Add configured property", cx, |ps| cp::add_property(ps, &part, prop, &active, &name));
                                    this.config_dirty(cx);
                                })
                            }));
                        }
                        m
                    });
                    crate::os_menu::open(e.position(), menu, window, cx);
                })
        };
        // ⋯ (with a table): Copy table | Delete table for <part> (live 603).
        let more = table.is_some().then(|| {
            let (view, part, name) = (view.clone(), part.clone(), part_name.clone());
            div().id("cp-more").cursor_pointer().child(crate::assets::icon("more", 20., cx)).on_click(move |e: &ClickEvent, window, cx| {
                let (v1, v2, p1, p2, name) = (view.clone(), view.clone(), part.clone(), part.clone(), name.clone());
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                    let (v1, v2, p1, p2) = (v1.clone(), v2.clone(), p1.clone(), p2.clone());
                    m.item(crate::os_menu::OsMenuItem::new("Copy table").on_click(move |_, _, cx| {
                        let text = v1.read(cx).studio(cx).map(|ps| cp::copy_table(ps, &p1)).unwrap_or_default();
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }))
                    .item(crate::os_menu::OsMenuItem::new(format!("Delete table for {name}")).on_click(move |_, _, cx| {
                        v2.update(cx, |this, cx| {
                            this.edit_config("Delete configured properties table", cx, |ps| cp::delete_table(ps, &p2));
                            this.ui.cp_selected = None;
                            this.config_dirty(cx);
                        })
                    }))
                });
                crate::os_menu::open(e.position(), menu, window, cx);
            })
        });
        let header = h_flex()
            .h(px(34.))
            .pl(px(9.))
            .pr(px(4.))
            .gap(px(8.))
            .bg(gpui_kit::rgb(0xeaeaea))
            .border_1()
            .border_color(gpui_kit::rgb(0xcccccc))
            .child(h_flex().flex_1().min_w_0().gap(px(4.)).text_size(px(16.)).font_weight(FontWeight::MEDIUM).child(div().text_color(pal.text).child("Properties for")).child(part_menu))
            .child(add_menu)
            .children(more);
        // Live 535: excluded inputs are called out above the header.
        let excluded_note = studio.configuration.inputs.iter().any(|i| i.excluded).then(|| {
            h_flex()
                .mb(px(6.))
                .px(px(10.))
                .py(px(10.))
                .gap(px(6.))
                .rounded(px(3.))
                .bg(pal.info_fill)
                .border_1()
                .border_color(pal.info_outline)
                .text_size(px(14.))
                .text_color(pal.info_text)
                .child(crate::assets::icon("configuration-exclude-properties", 20., cx))
                .child(div().flex_1().child("Some configuration inputs are marked as not affecting configured properties. ").child(div().font_weight(FontWeight::BOLD).underline().child("Learn more")))
        });
        let Some(table) = table else {
            // Live 600: the empty state.
            return v_flex()
                .mx(px(-5.))
                .children(excluded_note)
                .child(header)
                .child(
                    v_flex()
                        .mt(px(6.))
                        .px(px(14.))
                        .py(px(12.))
                        .gap(px(4.))
                        .rounded(px(3.))
                        .bg(pal.info_fill)
                        .border_1()
                        .border_color(pal.info_outline)
                        .text_size(px(14.))
                        .text_color(pal.info_text)
                        .child(div().font_weight(FontWeight::BOLD).child(format!("{part_name} has no configured part properties.")))
                        .child("Properties which are different in different configurations should be set here. Properties shared by all configurations should be set by right-clicking the part and assigning properties, material, or appearance."),
                )
                .into_any_element();
        };
        // Columns: the non-excluded inputs, then the properties in their
        // fixed order; 140 px each, scrolling sideways (live 602, 637).
        let cfg = studio.configuration.clone();
        let inputs: Vec<ConfigInput> = cp::input_columns(&cfg).into_iter().cloned().collect();
        let props = table.columns();
        // The workspace mass unit (Workspace units › Mass).
        let mass_unit = match self.doc.read(cx).session.doc.settings.units.other_unit_name(2) {
            "Gram" => "g",
            "Pound" => "lb",
            "Ounce" => "oz",
            _ => "kg",
        };
        let cell = || div().w(px(140.)).h(px(34.)).flex_none().flex().items_center().px(px(5.)).border_r_1().border_b_1().border_color(grid).text_size(px(14.)).text_color(pal.text).overflow_hidden();
        let focus = self.ui.cp_focus.get_or_insert_with(|| cx.focus_handle()).clone();
        let space_part = part.clone();
        // A property cell or header: right-click › Delete column (live 605, 606).
        let delete_column = |el: Stateful<Div>, prop: &'static str| {
            let (view, part) = (view.clone(), part.clone());
            el.os_context_menu(move |m, _, _| {
                let (v, part) = (view.clone(), part.clone());
                m.item(crate::os_menu::OsMenuItem::new("Delete column").on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.edit_config("Delete configured property", cx, |ps| cp::delete_column(ps, &part, prop));
                        this.config_dirty(cx);
                    })
                }))
            })
        };
        let mut head = h_flex().border_l_1().border_color(grid);
        for i in &inputs {
            head = head.child(cell().justify_center().bg(gpui_kit::rgb(0xeaeaea)).font_weight(FontWeight::BOLD).child(div().truncate().child(i.name.clone())));
        }
        for prop in &props {
            let prop = *prop;
            let h = cell().id(SharedString::from(format!("cp-head-{prop}"))).justify_center().gap(px(3.)).bg(gpui_kit::rgb(0xeaeaea)).font_weight(FontWeight::BOLD).child(div().truncate().child(prop))
                // Live 637: "Mass ƒ", the value is otherwise computed.
                .when(prop == "Mass", |d| d.child(div().italic().font_weight(FontWeight::NORMAL).child("ƒ")));
            head = head.child(delete_column(h, prop));
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        for (ri, row) in table.rows.iter().enumerate() {
            let mut r = h_flex().border_l_1().border_color(grid);
            for i in &inputs {
                let value = cp::row_input_value(&cfg, row, &i.id);
                let iid = i.id.clone();
                let el: AnyElement = match &i.kind {
                    // List inputs are read-only, grey, the option name.
                    ConfigInputKind::List { options, .. } => cell().bg(gpui_kit::rgb(0xf5f5f5)).child(options.iter().find(|o| o.id == value).map(|o| o.name.clone()).unwrap_or(value)).into_any_element(),
                    ConfigInputKind::Checkbox { .. } => self.cp_check_cell((ri, &iid), value == "true", cx),
                    _ => {
                        let (part2, iid2) = (part.clone(), iid.clone());
                        let st = self.cell_input(format!("cp-in-{ri}-{iid}"), value, window, cx, move |this, v, cx| {
                            let (part2, iid2) = (part2.clone(), iid2.clone());
                            this.edit_config("Edit configured properties", cx, |ps| cp::set_input_cell(ps, &part2, ri, &iid2, &v));
                            this.config_dirty(cx);
                        });
                        cell().px(px(2.)).child(Input::new(&st).appearance(false).text_size(px(13.33))).into_any_element()
                    }
                };
                r = r.child(el);
            }
            for prop in &props {
                let prop = *prop;
                let value = row.values.get(prop).cloned().unwrap_or_default();
                let (part2, view2) = (part.clone(), view.clone());
                let set = move |v: String, cx: &mut App| {
                    let part2 = part2.clone();
                    view2.update(cx, |this, cx| {
                        this.edit_config("Edit configured properties", cx, |ps| cp::set_cell(ps, &part2, ri, prop, &v));
                        this.config_dirty(cx);
                    })
                };
                let el: Stateful<Div> = match cp::cell_kind(prop) {
                    cp::CellKind::Check => div().id(SharedString::from(format!("cp-cellwrap-{ri}-{prop}"))).child(self.cp_check_cell((ri, prop), value == "true", cx)),
                    cp::CellKind::Select(options) => {
                        // Unit of measure shows "Each", tessellation "Auto"
                        // until set (live 636).
                        let shown = if value.is_empty() { if prop == "Unit of measure" { "Each".to_string() } else { "Auto".to_string() } } else { value.clone() };
                        let current = options.iter().position(|o| *o == shown).unwrap_or(0);
                        let sel = crate::os_ui::select_field(SharedString::from(format!("cp-sel-{ri}-{prop}")), options.iter().map(|o| o.to_string()).collect(), current, move |k, _, cx| set(options[k].to_string(), cx)).h(px(32.)).border_0().rounded(px(0.));
                        cell().id(SharedString::from(format!("cp-cell-{ri}-{prop}"))).px_0().child(sel)
                    }
                    // Appearance / Material: the part's value as text ("Default",
                    // "- None -" when unset).
                    cp::CellKind::Appearance => cell().id(SharedString::from(format!("cp-cell-{ri}-{prop}"))).child(if value.is_empty() { "Default".to_string() } else { value.clone() }),
                    cp::CellKind::Material => cell().id(SharedString::from(format!("cp-cell-{ri}-{prop}"))).child(if value.is_empty() { "- None -".to_string() } else { value.clone() }),
                    kind => {
                        // Text cells; Mass reads a grey italic "Computed" while
                        // empty and a bare number takes the mass unit (live 637).
                        let mass = kind == cp::CellKind::Mass;
                        let part3 = part.clone();
                        let st = self.cell_input(format!("cp-{ri}-{prop}"), value.clone(), window, cx, move |this, v, cx| {
                            let v = if mass && v.trim().parse::<f64>().is_ok() { format!("{} {mass_unit}", v.trim()) } else { v };
                            let part3 = part3.clone();
                            this.edit_config("Edit configured properties", cx, |ps| cp::set_cell(ps, &part3, ri, prop, &v));
                            this.config_dirty(cx);
                        });
                        let empty = st.read(cx).value().is_empty();
                        cell()
                            .id(SharedString::from(format!("cp-cell-{ri}-{prop}")))
                            .relative()
                            .px(px(2.))
                            .child(Input::new(&st).appearance(false).text_size(px(13.33)))
                            .when(mass && empty, |d| d.child(div().absolute().left(px(7.)).top_0().bottom_0().flex().items_center().italic().text_color(pal.text_faint).child("Computed")))
                    }
                };
                r = r.child(delete_column(el, prop));
            }
            rows.push(r.into_any_element());
        }
        v_flex()
            .mx(px(-5.))
            .children(excluded_note)
            .child(header)
            .child(
                div()
                    .id("cp-table")
                    .track_focus(&focus)
                    // Space toggles the selected checkbox cell.
                    .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
                        let Some((row, key)) = this.ui.cp_selected.clone().filter(|_| e.keystroke.key == "space") else { return };
                        let Some(ps) = this.studio(cx) else { return };
                        let Some(r) = cp::table(ps, &space_part).and_then(|t| t.rows.get(row)) else { return };
                        let input = ps.configuration.inputs.iter().any(|i| i.id == key);
                        let on = if input { cp::row_input_value(&ps.configuration, r, &key) } else { r.values.get(&key).cloned().unwrap_or_default() } == "true";
                        let v = (!on).to_string();
                        let part = space_part.clone();
                        cx.stop_propagation();
                        this.edit_config("Edit configured properties", cx, |ps| if input { cp::set_input_cell(ps, &part, row, &key, &v) } else { cp::set_cell(ps, &part, row, &key, &v) });
                        this.config_dirty(cx);
                    }))
                    .overflow_x_scroll()
                    .child(v_flex().child(head).children(rows)),
            )
            .into_any_element()
    }

    /// A checkbox cell of the Configured properties table at (row, input id
    /// or property): a click selects it (blue outline) and Space toggles it
    /// (live 10-20 q; the table handles the key).
    fn cp_check_cell(&self, at: (usize, &str), on: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (row, key) = at;
        let is_sel = self.ui.cp_selected.as_ref().is_some_and(|(r, k)| *r == row && k == key);
        let Some(focus) = self.ui.cp_focus.clone() else { return div().into_any_element() };
        let key2 = key.to_string();
        div()
            .id(SharedString::from(format!("cp-check-{row}-{key}")))
            .w(px(140.))
            .h(px(34.))
            .flex_none()
            .flex()
            .items_center()
            .px(px(5.))
            .border_r_1()
            .border_b_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .when(is_sel, |d| d.border_2().border_color(pal.accent))
            .child(
                div()
                    .size(px(13.))
                    .rounded(px(2.))
                    .border_1()
                    .border_color(gpui_kit::rgb(0x888888))
                    .bg(gpui_kit::rgb(0xffffff))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(on, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).with_size(px(10.)).text_color(pal.text))),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.ui.cp_selected = Some((row, key2.clone()));
                window.focus(&focus, cx);
                cx.notify();
            }))
            .into_any_element()
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
                    // Live (dom/panel-configuration-left-box): "Modify
                    // visibility conditions", the dialog for every input.
                    .child(crate::assets::icon("configuration-visibiliity", 20., cx))
                    .tooltip(crate::ui::tooltip("Modify visibility conditions"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_visibility_conditions(None, window, cx))),
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
                            // Help config-paste-inputs: Paste inputs once something was copied.
                            if !crate::config_visibility::copied_inputs().is_empty() {
                                let v = v.clone();
                                m = m.item(crate::os_menu::OsMenuItem::new("Paste inputs").icon_name("paste").on_click(move |_, _, cx| v.update(cx, |this, cx| this.paste_config_inputs(cx))));
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
        // Visibility conditions: hidden inputs and options don't show.
        let (shown, effective) = cfg.visibility(&self.config_values);
        for (input, vis) in cfg.inputs.iter().zip(&shown) {
            if !vis.visible {
                continue;
            }
            let current = effective.0.get(&input.id).cloned().unwrap_or_default();
            let control: AnyElement = match &input.kind {
                ConfigInputKind::List { options, .. } => {
                    let items = options.iter().filter(|o| vis.options.contains(&o.id)).map(|o| (SharedString::from(o.id.clone()), SharedString::from(o.name.clone()))).collect();
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
                    .child(div().flex_1().min_w_0().child(control))
                    // Live part-studio/535: the excluded icon follows the value.
                    .when(input.excluded, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("cfgbox-excluded-{}", input.id)))
                                .flex_none()
                                .child(crate::assets::icon("configuration-exclude-properties", 20., cx))
                                .tooltip(crate::ui::tooltip("Excluded from affecting configured properties")),
                        )
                    }),
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
        // With tables: the selector, sections and grids (custom_tables.rs).
        if let Some(panel) = self.custom_tables_panel(cx) {
            return panel;
        }
        v_flex()
            .items_center()
            .pt(px(4.))
            .child(self.add_custom_tables_button(cx))
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
                    // Live (592): ⋯ opens an (empty) dropdown and looks pressed.
                    .child(
                        div()
                            .id("insp-more")
                            .relative()
                            .w(px(43.))
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(self.ui.inspection_more, |d| d.bg(gpui_kit::rgb(0xffffff)).border_1().border_color(pal.border_strong))
                            .child(crate::assets::icon("more", 20., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.inspection_more = !this.ui.inspection_more;
                                cx.notify();
                            }))
                            .when(self.ui.inspection_more, |d| {
                                d.child(
                                    deferred(anchored().child(
                                        div()
                                            .id("insp-more-menu")
                                            .occlude()
                                            .mt(px(30.))
                                            .w(px(160.))
                                            .h(px(8.))
                                            .bg(gpui_kit::rgb(0xffffff))
                                            .border_1()
                                            .border_color(pal.menu_outline)
                                            .shadow_md()
                                            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                                this.ui.inspection_more = false;
                                                cx.notify();
                                            })),
                                    ))
                                    .with_priority(3),
                                )
                            }),
                    ),
            )
            .child(self.render_inspection_tools(cx))
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

    /// The Inspection tools (live 591, 593): Datum, Dimension ▾ (Dimension,
    /// Fillet / Minimum / Maximum dimension), Geometric tolerance, Weld
    /// symbol | Set coincident, Set parallel, Set text alignment. A tool
    /// stays pressed (a pick mode) until clicked again or Esc.
    fn render_inspection_tools(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let pressed = self.ui.inspection_tool;
        let tool = |id: &'static str, icon: &'static str, tip: &'static str, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .cursor_pointer()
                .when(pressed == Some(id), |d| d.bg(pal.accent_soft))
                .hover(|d| d.bg(pal.row_hover))
                .child(crate::assets::icon(icon, 20., cx))
                .tooltip(crate::ui::tooltip(tip))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.inspection_tool = if this.ui.inspection_tool == Some(id) { None } else { Some(id) };
                    cx.notify();
                }))
        };
        let v = cx.entity();
        let dim_caret = div()
            .id("insp-dim-more")
            .w(px(14.))
            .h(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(crate::ui::glyph("ui-caret-down", 8., pal.text))
            .on_click(move |e: &ClickEvent, window, cx| {
                let v = v.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    for label in ["Dimension", "Fillet dimension", "Minimum dimension", "Maximum dimension"] {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name("dimension").on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.ui.inspection_tool = Some("insp-dimension");
                                cx.notify();
                            })
                        }));
                    }
                    m
                });
                crate::os_menu::open(e.position(), menu, window, cx);
            });
        h_flex()
            .h(px(36.))
            .px(px(6.))
            .gap(px(4.))
            .bg(gpui_kit::rgb(0xf5f5f5))
            // Esc ends the tool (the studio's Cancel action).
            .child(tool("insp-datum", "datum", "Datum", cx))
            .child(h_flex().child(tool("insp-dimension", "dimension", "Dimension", cx)).child(dim_caret))
            .child(tool("insp-gtol", "tolerance", "Geometric tolerance", cx))
            .child(tool("insp-weld", "weld-bead", "Weld symbol", cx))
            .child(div().w(px(1.)).h(px(20.)).mx(px(4.)).bg(pal.border))
            .child(tool("insp-coincident", "coincident", "Set coincident", cx))
            .child(tool("insp-parallel", "parallel", "Set parallel", cx))
            .child(tool("insp-align", "aligned-rectangle", "Set text alignment", cx))
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
                    // Help config-paste-inputs: Paste inputs once something was copied.
                    if !crate::config_visibility::copied_inputs().is_empty() {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new("Paste inputs").icon_name("paste").on_click(move |_, _, cx| v.update(cx, |this, cx| this.paste_config_inputs(cx))));
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

    pub(super) fn config_dirty(&mut self, cx: &mut Context<Self>) {
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
                .tip(tip, crate::ui::TipSide::Left)
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

/// The Variable table's name check (FeatureScript identifier; empty is
/// allowed, it commits nothing).
fn valid_variable_name(v: &str) -> bool {
    let v = v.trim();
    v.is_empty() || noshape_model::variable_table::is_identifier(v.trim_start_matches('#'))
}

/// A Variable table row's right-click menu (live part-studio/561): Filter
/// feature list | Delete | Move up (not the first row) | Move down (not the
/// last).
fn var_row_menu(menu: crate::os_menu::OsMenu, view: Entity<PartStudioView>, fid: noshape_model::FeatureId, name: &str, k: usize, count: usize) -> crate::os_menu::OsMenu {
    type RowEdit = fn(&mut noshape_model::partstudio::PartStudio, noshape_model::FeatureId);
    let edit = |label: &'static str, undo: &'static str, f: RowEdit| {
        let view = view.clone();
        crate::os_menu::OsMenuItem::new(label).on_click(move |_, _, cx| {
            view.update(cx, |this, cx| {
                this.edit_config(undo, cx, |ps| f(ps, fid));
                this.config_dirty(cx);
            })
        })
    };
    // Live 564: the filter matches the name exactly.
    let filter = format!(":variable \"{name}\"");
    let v = view.clone();
    let mut menu = menu.item(crate::os_menu::OsMenuItem::new("Filter feature list").on_click(move |_, w, cx| v.update(cx, |this, cx| this.set_filter(&filter, w, cx)))).item(edit("Delete", "Delete variable", |ps, fid| {
        ps.remove(fid);
    }));
    if k > 0 {
        menu = menu.item(edit("Move up", "Move variable", |ps, fid| noshape_model::variable_table::move_row(ps, fid, -1)));
    }
    if k + 1 < count {
        menu = menu.item(edit("Move down", "Move variable", |ps, fid| noshape_model::variable_table::move_row(ps, fid, 1)));
    }
    menu
}
