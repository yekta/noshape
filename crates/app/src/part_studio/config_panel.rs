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
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, VariableKind};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanel {
    Appearance,
    Variables,
    Configurations,
    CustomTables,
    HoleTable,
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
            let focused = st.read(cx).focus_handle(cx).is_focused(window);
            if focused && last != value && st.read(cx).value() == last.as_str() {
                // Just committed and rewritten (live: "25" + Enter shows
                // "25 mm" at once, still selected).
                self.panel_focused = Some(key.clone());
                st.update(cx, |s, cx| {
                    s.set_value(value.clone(), window, cx);
                    s.select_all(window, cx);
                });
                self.panel_inputs.insert(key.clone(), (st.clone(), value));
            } else if focused {
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
                // Configured properties: Enter moves to the cell below.
                if matches!(ev, InputEvent::PressEnter { .. })
                    && let Some(next) = this.cp_neighbour(&k, 1, 0)
                {
                    next.update(cx, |s, cx| s.focus(window, cx));
                    return;
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
            RightPanel::HoleTable => self.render_hole_table(cx),
            RightPanel::Inspection => self.render_inspection(cx),
            RightPanel::SheetMetal => self.render_sheet_metal_panel(cx),
            RightPanel::CutList => self.render_cut_list_panel(cx),
        };
        let title = match panel {
            RightPanel::Appearance => "Appearances",
            RightPanel::Variables => "Variable table",
            RightPanel::Configurations => "Configurations",
            RightPanel::CustomTables => "Custom tables",
            RightPanel::HoleTable => "Hole table",
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
            crate::ui::right_resizable(v_flex().relative(), 6.)
                .w(px(crate::ui::right_panel_w() + 6.))
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
            let range = v.range.clone();
            self.edit_config(noshape_ops::config::ADD_VARIABLE_INPUT, cx, |ps| {
                noshape_ops::config::add_variable_input(ps, name, kind, value, range);
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
        // Live psb/t-807: the ⌄ at x 6 in the table, the title at x 21.
        let header = h_flex()
            .h(px(35.))
            .pl(px(5.))
            .pr(px(6.))
            .gap(px(4.))
            .bg(gpui_kit::rgb(0xeaeaea))
            .border_1()
            .border_color(grid)
            // ⌄ folds the table; ⋯ = Copy table only (live part-studio/562).
            .child(
                div()
                    .id("pvar-fold")
                    .cursor_pointer()
                    // The same fold glyph as the inserted tables (live psb/t-807).
                    .child(crate::os_ui::glyph(if self.ui.var_folded { crate::os_ui::GLYPH_COLLAPSED } else { crate::os_ui::GLYPH_EXPANDED }, 10., pal.text))
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
        let input_cell_sized = move |st: &Entity<InputState>, size: f32| gpui_kit::base::input::input_frame(div().w(px(140.)).h(px(34.)).flex_none().flex().items_center().px(px(2.)).border_r_1().border_b_1().border_color(grid).child(Input::new(st).appearance(false).text_size(px(size)).pl(px(4.))));
        let input_cell = move |st: &Entity<InputState>| input_cell_sized(st, 13.33);
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
            // Drawn as an inserted Variable Studio's (live psb/t-807, qa5 VS3).
            let kind_select = crate::variable_studio::type_cell(SharedString::from(format!("pvar-kind-{fid}")), KINDS.iter().map(|(_, n)| n.to_string()).collect(), current, move |i, _, cx| {
                let kind = KINDS[i].0;
                v_kind.update(cx, |this, cx| {
                    let units = this.doc.read(cx).session.doc.settings.units;
                    this.edit_config("Edit variable", cx, |ps| noshape_model::variable_table::set_kind(ps, fid, kind, &units));
                    this.config_dirty(cx);
                })
            });
            let (n1, name, n2) = (r.name.clone(), r.name.clone(), r.name.clone());
            let view = cx.entity();
            // Measured / From import: the evaluated value, or "Not set", and
            // a ✎ that opens the feature (live 559).
            let value_cell = if matches!(r.kind.as_str(), "MEASURED" | "TABLE") {
                let units = self.doc.read(cx).session.doc.settings.units;
                let shown = self.display.variables.get(&r.name).map(|q| noshape_units::format_quantity(*q, &units)).unwrap_or_else(|| "Not set".into());
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
                // The value at 14 px, as a Variable Studio's value cell (qa5 VS3).
                input_cell_sized(&value_st, 14.).into_any_element()
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
                            .child(kind_select)
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
        // Variable Studio tables inserted here: above the Part Studio's own
        // table, in their stored order (the lowest wins a name clash); same-
        // document ones are editable. Their headers drag to reorder (live
        // psb 10-20 t): a 1 px blue line with a circle shows the drop place,
        // and the Part Studio's own table can't be passed (red line).
        let refs = noshape_model::variables::element_studios(&self.doc.read(cx).session.doc, self.tab);
        let studios: Vec<noshape_model::document::TabId> = refs.iter().map(|r| r.studio).collect();
        self.ui.var_tables.retain(|k, _| studios.contains(k));
        if !cx.has_active_drag() {
            self.ui.var_drop = None;
        }
        let drop_at = self.ui.var_drop;
        // `top`: the line's offset from the table's top; None: at its bottom.
        let line = |color: Rgba, top: Option<f32>| {
            div()
                .absolute()
                .left_0()
                .right_0()
                .h(px(1.))
                .bg(color)
                .map(|d| match top {
                    Some(t) => d.top(px(t)),
                    None => d.bottom(px(4.)),
                })
                .child(div().absolute().left(px(1.)).top(px(-3.)).size(px(7.)).rounded_full().border_1().border_color(color).bg(gpui_kit::rgb(0xffffff)))
        };
        let blue = gpui_kit::rgb(0x1651b0);
        let mut tables: Vec<AnyElement> = Vec::new();
        for (k, r) in refs.iter().enumerate() {
            // Another document's studio: read-only (Onshape).
            let body: AnyElement = if r.external.is_some() {
                let target = crate::variable_insert::InsertTarget::Element(self.tab);
                match crate::variable_insert::readonly_table(&self.doc, target, r, k, refs.len(), cx) {
                    Some(t) => t.into_any_element(),
                    None => continue,
                }
            } else {
                let (doc, host, s) = (self.doc.clone(), self.tab, r.studio);
                self.ui.var_tables.entry(s).or_insert_with(|| cx.new(|cx| crate::variable_studio::VariableStudioView::new_in_panel(doc, s, host, window, cx))).clone().into_any_element()
            };
            // The header's title area is the drag handle (between the fold
            // chevron and ⋯, which stay clickable).
            let name: SharedString = self.doc.read(cx).session.doc.tabs.iter().find(|t| t.id == r.studio).map(|t| t.name.clone()).unwrap_or_default().into();
            let handle = var_table_drag_source(div().id(SharedString::from(format!("var-table-drag-{k}"))).absolute().top_0().left(px(24.)).right(px(48.)).h(px(35.)), r.studio, name);
            tables.push(
                div()
                    .id(SharedString::from(format!("var-table-{k}")))
                    .relative()
                    .pb(px(10.))
                    .child(body)
                    .child(handle)
                    // Between tables (the first table's at its top edge, which
                    // the panel would clip above).
                    .when(drop_at == Some(k), |d| d.child(line(blue, Some(if k == 0 { 0. } else { -6. }))))
                    .when(drop_at == Some(k + 1) && k + 1 == refs.len(), |d| d.child(line(blue, None)))
                    // Over the upper half: before this table; the lower: after.
                    .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<VarTableDrag>, _, cx| {
                        if !e.bounds.contains(&e.event.position) {
                            return;
                        }
                        let after = e.event.position.y > e.bounds.center().y;
                        let at = Some(if after { k + 1 } else { k });
                        if this.ui.var_drop != at {
                            this.ui.var_drop = at;
                            cx.notify();
                        }
                    }))
                    .on_drop(cx.listener(move |this, d: &VarTableDrag, _, cx| {
                        let to = this.ui.var_drop.take().unwrap_or(k);
                        this.move_var_table(d.studio, to, cx);
                    }))
                    .into_any_element(),
            );
        }
        // Over the Part Studio's own table: refused, a red line.
        let refused = drop_at == Some(usize::MAX);
        let empty = self.display.variables.is_empty() && studios.is_empty();
        // Live (164): the tables run flush with the pane (the body has 5 px
        // padding), the first header 2 px under the title's underline, and
        // every table ends with the italic "Name" row.
        let own = v_flex()
            .id("var-own-table")
            .relative()
            .child(header)
            .when(!self.ui.var_folded, |d| d.child(columns).children(rows).child(new_row))
            .when(refused, |d| d.child(line(gpui_kit::rgb(0xd52b1e), None)))
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<VarTableDrag>, _, cx| {
                if e.bounds.contains(&e.event.position) && this.ui.var_drop != Some(usize::MAX) {
                    this.ui.var_drop = Some(usize::MAX);
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, _: &VarTableDrag, _, cx| {
                this.ui.var_drop = None;
                cx.notify();
            }));
        v_flex()
            .mx(px(-5.))
            .mt(px(-3.))
            .children(tables)
            .child(own)
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

    /// Drop a dragged Variable Studio table at position `to` among the
    /// inserted tables; the whole order is stored with this Part Studio.
    fn move_var_table(&mut self, studio: noshape_model::document::TabId, to: usize, cx: &mut Context<Self>) {
        let list = noshape_model::variables::element_studios(&self.doc.read(cx).session.doc, self.tab);
        let Some(from) = list.iter().position(|r| r.studio == studio) else { return };
        let mut order = list.clone();
        let r = order.remove(from);
        let to = if to > from { to - 1 } else { to }.min(order.len());
        order.insert(to, r);
        if order == list {
            return;
        }
        let tab = self.tab;
        self.doc.update(cx, |d, cx| {
            d.edit_tab(tab, "Move table", cx, |c| {
                if let noshape_model::document::TabContent::PartStudio(ps) = c {
                    ps.variable_studios.removed.retain(|t| !order.iter().any(|r| r.studio == *t));
                    ps.variable_studios.inserted = order.clone();
                }
            });
        });
        self.ui.var_drop = None;
        self.config_dirty(cx);
    }

    /// The Variable table footer (live, part-studio/77): a split button
    /// "Create Variable Studio | ▴" whose menu also has Insert Variable Studio.
    fn variable_panel_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        crate::variable_insert::panel_footer(self.doc.clone(), self.tab, cx)
    }

    /// Add a configuration input of one of Onshape's kinds (List, Checkbox, Variable).
    fn add_config_input(&mut self, label: &'static str, cx: &mut Context<Self>) {
        self.edit_config(noshape_ops::config::ADD_INPUT, cx, |ps| {
            noshape_ops::config::add_input(ps, label);
        })
    }

    /// Onshape's Configurations panel before any input exists (live,
    /// part-studio/77): a "Configure Part Studio ▾" split button, a large faint
    /// illustration and the help text.
    fn render_configurations_empty(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let border = gpui_kit::rgb(0xaaaaaa);
        let view = cx.entity();
        // The split button's bounds: its menu drops from its bottom-left.
        let button: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let probe = button.clone();
        // Live (part-studio/181): the split button is 34 px tall with its
        // border, its top 2 px under the panel's tab strip.
        let main = div()
            .id("cfg-configure")
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::assets::icon("cfg-filter-list", 20., cx))
            .child(div().text_size(px(14.)).text_color(pal.text).child("Configure Part Studio"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_config_input("List", cx)));
        let caret = div()
            .id("cfg-configure-more")
            .w(px(24.))
            .h(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .border_l_1()
            .border_color(border)
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::ui::glyph("drawing-tool-caret", 12., pal.text))
            // Live (part-studio/181): List, Checkbox, Configuration variable, in
            // a dropdown under the split button, left-aligned and as wide as it,
            // 4 px below; 28.5 px rows, 20 px icons, text 48 px in.
            .on_click(move |_: &ClickEvent, window, cx| {
                let v = view.clone();
                let b = button.get();
                let width = f32::from(b.size.width);
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                    let mut m = m.dropdown().min_width(width).row_height(28.5).text_inset(48.);
                    for (label, icon, kind) in [("List", "cfg-filter-list", "List"), ("Checkbox", "cfg-filter-checkbox", "Checkbox"), ("Configuration variable", "cfg-filter-variable", "Variable")] {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, w, cx| v.update(cx, |this, cx| if kind == "Variable" { this.open_variable_input(None, w, cx) } else { this.add_config_input(kind, cx) })));
                    }
                    // Help config-paste-inputs: Paste inputs once something was copied.
                    if !crate::config_visibility::copied_inputs().is_empty() {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new("Paste inputs").icon_name("paste").on_click(move |_, _, cx| v.update(cx, |this, cx| this.paste_config_inputs(cx))));
                    }
                    m
                });
                crate::os_menu::open_anchored(point(b.left(), b.bottom() + px(4.)), Anchor::TopLeft, menu, window, cx);
            });
        v_flex()
            .items_center()
            .child(
                h_flex()
                    .relative()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(border)
                    .bg(pal.panel)
                    .child(main)
                    .child(caret)
                    .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()),
            )
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
            let section = self.render_config_input(&studio, input, grid, window, cx);
            col = col.child(self.cfg_drop_target(&input.id, section, cx));
        }
        if !cx.has_active_drag() {
            self.ui.cfg_drop = None;
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
            .id(SharedString::from(format!("cfg-head-{iid}")))
            // Live (cursors/670): drag the header to reorder the inputs.
            .on_drag(CfgInputDrag(iid.clone()), {
                let name = SharedString::from(input.name.clone());
                move |_, offset, _, cx| {
                    let name = name.clone();
                    cx.new(|_| CfgDragPill { name, offset })
                }
            })
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
                this.edit_config(noshape_ops::config::RENAME_INPUT, cx, |ps| noshape_ops::config::rename_input(ps, &id, v));
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
                        .bg(pal.panel)
                        // Live (psb 10-21 g): the button greys out (0.4) in the mode.
                        .when(picking, |d| d.opacity(0.4))
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
                let is_variable = matches!(input.kind, ConfigInputKind::Variable { .. });
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
                            // A variable input: Edit definition… after Rename (live 710).
                            if is_variable {
                                let i11 = id.clone();
                                m = m.item(item("Edit definition…", Box::new(move |this, window, cx| this.open_variable_input(Some(i11.clone()), window, cx))));
                            }
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
                                this.edit_config(noshape_ops::config::DELETE_INPUT, cx, |ps| noshape_ops::config::delete_input(ps, &id));
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
        // Column widths: live's grips (`ui::col_grip`), per input table (live:
        // widening Size's Name left Shape's alone).
        let table = format!("config-{iid}");
        let name_w = crate::ui::col_w(&table, "name", NAME_W);
        let col_key = |c: &ConfiguredParam| format!("{:?}-{}", c.feature, c.param);
        let col_ws: Vec<f32> = columns.iter().map(|(_, c)| crate::ui::col_w(&table, &col_key(c), COL_W)).collect();
        match &input.kind {
            ConfigInputKind::Variable { default, .. } => {
                let a = iid.clone();
                let value = self.config_name_cell(format!("var-def-{iid}"), default.text.clone(), (px(13.33), FontWeight::NORMAL), window, cx, move |this, v, cx| {
                    let a = a.clone();
                    this.edit_config(noshape_ops::config::EDIT_VARIABLE_INPUT, cx, |ps| noshape_ops::config::set_variable_default(ps, &a, v));
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
                    section = section.child(h_flex().border_l_1().border_color(grid).child(head(name_w).relative().child(first).child(crate::ui::col_grip(&table, "name", NAME_W))));
                } else {
                    let mut groups = h_flex();
                    let mut params = h_flex();
                    let mut k = 0;
                    while k < columns.len() {
                        let f = columns[k].1.feature;
                        let n = columns[k..].iter().take_while(|(_, c)| c.feature == f).count();
                        let fname = studio.feature(f).map(|f| f.name.clone()).unwrap_or_else(|| "Deleted feature".into());
                        groups = groups.child(head(col_ws[k..k + n].iter().sum()).child(div().truncate().child(fname)));
                        for (q, (_, c)) in columns[k..k + n].iter().enumerate() {
                            params = params.child(head(col_ws[k + q]).relative().child(div().truncate().child(label_of(c))).child(crate::ui::col_grip(&table, &col_key(c), COL_W)));
                        }
                        k += n;
                    }
                    section = section.child(
                        h_flex()
                            .border_l_1()
                            .border_color(grid)
                            .child(div().w(px(name_w)).h(px(ROW_H * 2.)).relative().flex_none().flex().items_center().justify_center().border_r_1().border_b_1().border_color(grid).bg(gpui_kit::rgb(0xeaeaea)).font_weight(FontWeight::BOLD).text_size(px(14.)).child(first).child(crate::ui::col_grip(&table, "name", NAME_W)))
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
                            this.edit_config(noshape_ops::config::RENAME_OPTION, cx, |ps| noshape_ops::config::rename_option(ps, &a, &b, v));
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
                            cell(name_w)
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
                                        this.edit_config(noshape_ops::config::DELETE_OPTION, cx, |ps| noshape_ops::config::delete_option(ps, &a, &b));
                                    })
                                }))
                            })
                        });
                    for (q, (ci, c)) in columns.iter().enumerate() {
                        row = row.child(cell(col_ws[q]).child(self.configured_value_cell(*ci, c, rk, window, cx)));
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
                        this.edit_config(noshape_ops::config::ADD_OPTION, cx, |ps| {
                            noshape_ops::config::add_option(ps, &a, name);
                        });
                    });
                    section = section.child(
                        h_flex().border_l_1().border_color(grid).child(
                            cell(name_w)
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
                .child(Input::new(&st).appearance(false).text_size(size).font_weight(weight).pl(px(3.)))
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
                                    this.edit_config(noshape_ops::config::EDIT_CONFIGURED_VALUE, cx, |ps| noshape_ops::config::set_configured_value(ps, ci, &kk, ParamValue::Text(o)));
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
                    this.edit_config(noshape_ops::config::EDIT_CONFIGURED_VALUE, cx, |ps| noshape_ops::config::set_configured_value(ps, ci, &kk, ParamValue::Bool(!b)));
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
            this.edit_config(noshape_ops::config::EDIT_CONFIGURED_VALUE, cx, |ps| noshape_ops::config::set_configured_text(ps, ci, &kk, v));
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
        // Add property ▾ drops below the button, right-aligned (live 534).
        let add_menu = {
            let (view, part, name) = (view.clone(), part.clone(), part_name.clone());
            let bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
            let probe = bounds.clone();
            h_flex()
                .id("cp-add-property")
                .relative()
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
                .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                .on_click(move |_: &ClickEvent, window, cx| {
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
                    crate::os_menu::open_anchored(bounds.get().bottom_right(), Anchor::TopRight, menu, window, cx);
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
        self.ui.cp_grid = table
            .rows
            .iter()
            .enumerate()
            .map(|(ri, _)| {
                let ins = inputs.iter().map(|i| matches!(i.kind, ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. }).then(|| format!("cp-in-{ri}-{}", i.id)));
                let ps = props.iter().map(|p| matches!(cp::cell_kind(p), cp::CellKind::Text | cp::CellKind::Mass).then(|| format!("cp-{ri}-{p}")));
                ins.chain(ps).collect()
            })
            .collect();
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
                    // Appearance / Material: "Default" / "- None -" until set;
                    // a click opens the Appearance or Material popup for the
                    // cell (live 634).
                    cp::CellKind::Appearance => {
                        let swatch = cp::parse_color(&value).map(|c| {
                            let s = |x: f32| if x <= 0.003_130_8 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
                            div().size(px(14.)).flex_none().border_1().border_color(grid).bg(gpui_kit::Rgba { r: s(c[0]), g: s(c[1]), b: s(c[2]), a: c[3] })
                        });
                        let part2 = part.clone();
                        cell()
                            .id(SharedString::from(format!("cp-cell-{ri}-{prop}")))
                            .gap(px(6.))
                            .cursor_pointer()
                            .children(swatch)
                            .child(div().min_w_0().truncate().child(if value.is_empty() { "Default".to_string() } else { value.clone() }))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.edit_appearance(super::appearance::EditTarget::ConfiguredCell(part2.clone(), ri), window, cx)))
                    }
                    cp::CellKind::Material => {
                        let part2 = part.clone();
                        cell()
                            .id(SharedString::from(format!("cp-cell-{ri}-{prop}")))
                            .cursor_pointer()
                            .child(div().min_w_0().truncate().child(if value.is_empty() { "- None -".to_string() } else { value.clone() }))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.open_material_panel(vec![part2.clone()], window, cx);
                                if let Some(m) = &mut this.ui.material_panel {
                                    m.cp_cell = Some((part2.clone(), ri));
                                }
                            }))
                    }
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
                    // Tab moves right; a multi-line paste on a property cell
                    // fills the column downward, existing rows only (live
                    // 10-20 q). Both arrive as the cell input's actions.
                    .capture_action(cx.listener(|this, _: &gpui_kit::component::input::IndentInline, window, cx| {
                        let Some(key) = this.cp_focused_cell(window, cx) else { return };
                        if let Some(next) = this.cp_neighbour(&key, 0, 1) {
                            cx.stop_propagation();
                            // The cell commits on its Blur.
                            next.update(cx, |s, cx| s.focus(window, cx));
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Paste, window, cx| {
                        let Some(key) = this.cp_focused_cell(window, cx).filter(|k| !k.starts_with("cp-in-")) else { return };
                        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
                        let lines: Vec<String> = text.lines().map(|l| l.split('\t').next().unwrap_or("").to_string()).collect();
                        if lines.len() < 2 {
                            return;
                        }
                        cx.stop_propagation();
                        this.cp_paste_column(&key, &lines, cx);
                        // The focused cell shows its line at once.
                        if let Some((st, last)) = this.panel_inputs.get_mut(&key) {
                            last.clone_from(&lines[0]);
                            let (st, v) = (st.clone(), lines[0].clone());
                            st.update(cx, |s, cx| s.set_value(v, window, cx));
                        }
                    }))
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

    /// The Configured properties cell `dr` rows down and `dc` columns right
    /// of cell `key`, if it is an input cell.
    fn cp_neighbour(&self, key: &str, dr: usize, dc: usize) -> Option<Entity<InputState>> {
        let (r, c) = self.ui.cp_grid.iter().enumerate().find_map(|(r, row)| row.iter().position(|k| k.as_deref() == Some(key)).map(|c| (r, c)))?;
        let next = if dc > 0 { self.ui.cp_grid.get(r)?.iter().skip(c + dc).flatten().next()? } else { self.ui.cp_grid.get(r + dr)?.get(c)?.as_ref()? };
        self.panel_inputs.get(next).map(|(st, _)| st.clone())
    }

    /// The Configured properties input cell that has the keyboard.
    fn cp_focused_cell(&self, window: &Window, cx: &App) -> Option<String> {
        self.ui.cp_grid.iter().flatten().flatten().find(|k| self.panel_inputs.get(*k).is_some_and(|(st, _)| st.read(cx).focus_handle(cx).is_focused(window))).cloned()
    }

    /// Paste `lines` down a property column from cell `key` (`cp-<row>-<prop>`).
    fn cp_paste_column(&mut self, key: &str, lines: &[String], cx: &mut Context<Self>) {
        use noshape_model::configured_properties as cp;
        let Some(rest) = key.strip_prefix("cp-") else { return };
        let Some((row, prop)) = rest.split_once('-') else { return };
        let Ok(row) = row.parse::<usize>() else { return };
        let Some(part) = self.ui.cp_part.clone().or_else(|| self.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| b.id.clone())) else { return };
        let (prop, lines) = (prop.to_string(), lines.to_vec());
        self.edit_config("Paste configured properties", cx, |ps| {
            let n = cp::table(ps, &part).map(|t| t.rows.len()).unwrap_or(0);
            for (i, v) in lines.iter().enumerate().take(n.saturating_sub(row)) {
                cp::set_cell(ps, &part, row + i, &prop, v);
            }
        });
        self.config_dirty(cx);
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
                    .child(crate::ui::glyph("ui-dropdown-arrow", 10., gpui_kit::rgb(0x666666)))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let v = view.clone();
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                            for (label, icon, kind) in [("List", "cfg-filter-list", "List"), ("Checkbox", "cfg-filter-checkbox", "Checkbox"), ("Configuration variable", "cfg-filter-variable", "Variable")] {
                                let v = v.clone();
                                m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, w, cx| v.update(cx, |this, cx| if kind == "Variable" { this.open_variable_input(None, w, cx) } else { this.add_config_input(kind, cx) })));
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
                // Variables and texts are typed in the box; a variable outside
                // its range turns pink with the reason (live psb/s-716–717).
                ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => match self.panel_inputs.get(&format!("cfgbox-val-{}", input.id)) {
                    Some((st, _)) => {
                        let err = config_value_error(input, &st.read(cx).value(), &self.doc.read(cx).session.doc.settings.units);
                        div()
                            .id(SharedString::from(format!("cfgbox-val-{}", input.id)))
                            .child(gpui_kit::base::input::input_frame(
                                div()
                                    .border_b_1()
                                    .border_color(if err.is_some() { gpui_kit::rgb(0xbd3039) } else { gpui_kit::rgb(0xd7d7d7) })
                                    .when(err.is_some(), |d| d.bg(gpui_kit::rgb(0xf9ebec)))
                                    .child(Input::new(st).small().appearance(false).text_size(px(14.))),
                            ))
                            .when_some(err, |d, e| d.tooltip(crate::ui::tooltip(e)))
                            .into_any_element()
                    }
                    None => div().text_size(px(14.)).text_color(pal.text).child(current).into_any_element(),
                },
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
        // Live (dom/cursor-rows.json): the line under the block is a
        // `row-resize` splitter (≈8 px). Its least is the bare title row; it
        // can grow past the inputs (blank space). Double-click does nothing.
        let height = self.ui.cfg_box_h;
        Some(
            div()
                .id("cfg-box")
                .relative()
                .flex_none()
                .child(col.when_some(height, |c, h| c.h(px(h)).overflow_hidden()))
                .child(
                    div()
                        .id("cfg-box-splitter")
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(-4.))
                        .h(px(8.))
                        .cursor(CursorStyle::ResizeRow)
                        .on_drag(CfgBoxDrag, |_, _, _, cx| cx.new(|_| CfgBoxDrag)),
                )
                .on_drag_move(cx.listener(|this, e: &DragMoveEvent<CfgBoxDrag>, window, cx| {
                    let max = (f32::from(window.viewport_size().height - e.bounds.top()) - 100.).max(37.);
                    let h = f32::from(e.event.position.y - e.bounds.top()).round().clamp(37., max);
                    if this.ui.cfg_box_h != Some(h) {
                        this.ui.cfg_box_h = Some(h);
                        cx.notify();
                    }
                }))
                .into_any_element(),
        )
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

    /// Hole table (live L59, part-studio/2181): read-only, one section per
    /// part ("▾ Part 1 ⋯"), Tag | Size | Drill angle | Qty, a row per
    /// distinct hole (tags A, B, …), the callout on several lines.
    fn render_hole_table(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let units = self.doc.read(cx).session.doc.settings.units;
        let tables = self.context.as_ref().map(|c| noshape_model::interaction::hole_table(c, &units)).unwrap_or_default();
        v_flex().children(tables.iter().enumerate().map(|(i, t)| self.table_section(1000 + i, t, cx)).collect::<Vec<_>>()).into_any_element()
    }

    /// Inspection table (live part-studio/77): "Model  No models.", the
    /// inspection tool row and the empty-state help.
    fn render_inspection(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let letters = self.studio(cx).map(|s| noshape_model::annotations::datum_letters(&s.annotations)).unwrap_or_default();
        // Toleranced parameters, the open dialog's included (live 596, 598:
        // the panel fills while the feature dialog is still open).
        // The studio as shown: the draft in place, later features rolled back
        // (live 10-21 b: their Nominal reads 0.000 on pink).
        let chars = self
            .effective_studio(cx)
            .map(|(s, upto)| {
                let upto = upto.unwrap_or_else(|| s.rollback_index());
                noshape_model::annotations::characteristics(&s, None, upto)
            })
            .unwrap_or_default();
        let standard = self.studio(cx).is_some_and(|s| s.default_tolerances.is_some());
        if !letters.is_empty() || !chars.is_empty() || standard {
            return self.render_inspection_filled(letters, chars, cx);
        }
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
                // Live part-studio/77: 172 px wide.
                172.,
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

    /// Model definitions with annotations (live psb/r-912–913): "Model
    /// Part 1 ▾", the default tolerances field and ⋯; the tools; "⌄ Datums
    /// (n/n)" with one cell per letter; "⌄ Characteristics (0/0)" with its
    /// table header.
    fn render_inspection_filled(&mut self, letters: Vec<String>, chars: Vec<noshape_model::annotations::Characteristic>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let grid = gpui_kit::rgb(0xaaaaaa);
        let solid = self.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid);
        let part = solid.map(|b| b.name.clone()).unwrap_or_default();
        let part_id = solid.map(|b| b.id.clone());
        let section = |title: &'static str, shown: usize, count: usize| {
            h_flex()
                .h(px(34.))
                .px(px(6.))
                .gap(px(4.))
                .bg(gpui_kit::rgb(0xeaeaea))
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .child(crate::ui::glyph("ui-chevron-down", 10., pal.text))
                .child(div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title))
                .child(div().text_size(px(14.)).text_color(gpui_kit::rgb(0x999999)).child(format!("({shown}/{count})")))
        };
        let section_plain = |title: &'static str| {
            h_flex()
                .h(px(34.))
                .px(px(6.))
                .gap(px(4.))
                .bg(gpui_kit::rgb(0xeaeaea))
                .border_1()
                .border_color(gpui_kit::rgb(0xcccccc))
                .child(crate::ui::glyph("ui-chevron-down", 10., pal.text))
                .child(div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title))
        };
        let cell = |t: String, bold: bool| div().w(px(140.)).h(px(34.)).flex_none().flex().items_center().justify_center().border_r_1().border_b_1().border_color(grid).text_size(px(14.)).text_color(pal.text).when(bold, |d| d.font_weight(FontWeight::BOLD).bg(gpui_kit::rgb(0xeaeaea))).child(t);
        let n = letters.len();
        let units = self.doc.read(cx).session.doc.settings.units;
        // Types ▾ (live 599-click-types): nothing ticked shows every row;
        // Hide default tolerances hides the type-Default rows (10-21 c).
        let shown: Vec<&noshape_model::annotations::Characteristic> = chars.iter().filter(|c| self.ui.insp.shows(c)).collect();
        let standard = self.studio(cx).and_then(|s| s.default_tolerances.clone());
        let defaults = self.render_default_tolerances_section(cx);
        v_flex()
            .mx(px(-5.))
            .child(
                h_flex()
                    .h(px(36.))
                    .px(px(12.))
                    .gap(px(12.))
                    .text_size(px(13.33))
                    .child(div().text_color(pal.text).child("Model"))
                    .child(h_flex().w(px(170.)).h(px(26.)).px(px(4.)).justify_between().border_b_1().border_color(grid).child(div().text_color(pal.text).child(part)).child(crate::ui::glyph("ui-caret-down", 8., gpui_kit::rgb(0x666666))))
                    .child(self.render_insp_model_tools(part_id, cx))
                    .child(div().flex_1())
                    .child(div().id("insp-more").child(crate::assets::icon("more", 20., cx))),
            )
            .child(self.render_inspection_tools(cx))
            .child(section("Datums", n, n))
            .child(h_flex().border_l_1().border_color(grid).children(letters.into_iter().map(|l| cell(l, true))))
            .child(div().h(px(8.)))
            .child(section("Characteristics", shown.len(), chars.len()).child(self.render_characteristics_tools(cx)))
            // Hovering a cell darkens its row's Type cell and its column's
            // header (live psb/tl-99a).
            .child(h_flex().border_l_1().border_color(grid).children(["Type", "Nominal value", "Tolerance", "Upper limit", "Lower limit"].into_iter().enumerate().map(|(col, h)| {
                let hot = self.ui.insp.hover_cell.is_some_and(|(_, c)| c == col);
                cell(h.to_string(), true).when(hot, |d| d.bg(gpui_kit::rgb(0xd5d5d5)))
            })))
            // Live 598 / 10-21 b: "Distance | 20.000 | ±0.000 | 20.000 |
            // 20.000", the type bold, numbers without their unit.
            .children(shown.iter().enumerate().map(|(row, c)| {
                let factor = units.si_factor(c.dim);
                let decimals = usize::from(if c.dim == noshape_units::Dim::ANGLE { units.angle_precision } else { units.length_precision });
                let [nominal, tol, upper, lower] = c.cells(factor, decimals, standard.as_deref());
                let feature = c.feature;
                let (hover, focus) = (self.ui.insp.hover_cell, self.ui.insp.focus_cell);
                let rolled_back = c.rolled_back;
                h_flex().border_l_1().border_color(grid).children([c.kind.to_string(), nominal, tol, upper, lower].into_iter().enumerate().map(|(col, text)| {
                    cell(text, false)
                        .id(SharedString::from(format!("insp-cell-{row}-{col}")))
                        .when(col == 0, |d| d.font_weight(FontWeight::BOLD).when(hover.is_some_and(|(r, _)| r == row), |d| d.bg(gpui_kit::rgb(0xd5d5d5))))
                        .when(col == 1 && rolled_back, |d| d.bg(gpui_kit::rgb(0xf9ebec)))
                        // Click: a 2 px blue focus border, nothing selected.
                        .when(focus == Some((row, col)), |d| d.border_2().border_color(pal.accent))
                        .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                            let cellid = (*h).then_some((row, col));
                            if *h || this.ui.insp.hover_cell == Some((row, col)) {
                                this.ui.insp.hover_cell = cellid;
                                // The feature's geometry pre-highlights and its
                                // list row lights up.
                                this.ui.insp.hover_feature = h.then_some(feature);
                                this.refresh_scene(false, cx);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ui.insp.focus_cell = Some((row, col));
                            cx.notify();
                        }))
                }))
            }))
            .when_some(defaults, |d, body| d.child(div().h(px(8.))).child(section_plain("Default tolerances")).child(body))
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
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    // Datum opens its dialog (live psb/r-910).
                    if id == "insp-datum" {
                        if this.ui.datum_edit.is_some() { this.close_datum_tool(cx) } else { this.open_datum_tool(window, cx) }
                        return;
                    }
                    if this.ui.datum_edit.is_some() {
                        this.close_datum_tool(cx);
                    }
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
            .child(crate::ui::glyph("ui-chevron-down", 10., pal.text))
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
            .child(tool("insp-gtol", "insp-gtol", "Geometric tolerance", cx))
            .child(tool("insp-weld", "insp-weld", "Weld symbol", cx))
            .child(div().w(px(1.)).h(px(20.)).mx(px(4.)).bg(pal.border))
            .child(tool("insp-coincident", "coincident", "Set coincident", cx))
            .child(tool("insp-parallel", "parallel", "Set parallel", cx))
            .child(tool("insp-align", "insp-text-align", "Set text alignment", cx))
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
            .child(crate::assets::icon("cfg-filter-list", 20., cx))
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
                    for (label, icon, kind) in [("List", "cfg-filter-list", "List"), ("Checkbox", "cfg-filter-checkbox", "Checkbox"), ("Configuration variable", "cfg-filter-variable", "Variable")] {
                        let v = v.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, w, cx| v.update(cx, |this, cx| if kind == "Variable" { this.open_variable_input(None, w, cx) } else { this.add_config_input(kind, cx) })));
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

    /// The parameters of `feature` that are configured, and whether the
    /// studio has a list or checkbox input (parameters can be configured).
    pub(super) fn configured_of(&self, feature: noshape_model::FeatureId, cx: &App) -> (std::collections::HashSet<String>, bool) {
        let Some(ps) = self.studio(cx) else { return Default::default() };
        let cfg = &ps.configuration;
        let set = cfg.configured.iter().filter(|c| c.feature == feature).map(|c| c.param.clone()).collect();
        (set, cfg.inputs.iter().any(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. })))
    }

    /// Configure / unconfigure a parameter of the open dialog (configure
    /// mode click, right-click Configure / Unconfigure; live psb 10-21 g):
    /// the dialog stays open, the parameter gets its "Configured" outline
    /// and Onshape's toast "Extrude 1 Depth added to Size" shows ~6.7 s.
    pub(super) fn configure_dialog_param(&mut self, param: &str, on: bool, cx: &mut Context<Self>) {
        let Some(dialog) = self.edit.as_ref().map(|e| e.dialog.clone()) else { return };
        let feature = dialog.read(cx).feature.clone();
        let Some(ps) = self.studio(cx) else { return };
        let inputs: Vec<(String, String)> = ps.configuration.inputs.iter().filter(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. })).map(|i| (i.id.clone(), i.name.clone())).collect();
        // The input being configured (Configure features), else the first.
        let Some((input, input_name)) = self.ui.configure_pick.as_ref().and_then(|id| inputs.iter().find(|(i, _)| i == id).cloned()).or_else(|| inputs.first().cloned()) else { return };
        let label = if param == noshape_model::configuration::SUPPRESSED_PARAM {
            "Unsuppressed".to_string()
        } else {
            feature.feature_type().and_then(|t| t.params.iter().find(|s| s.id == param).map(|s| s.label.to_string())).unwrap_or_else(|| param.to_string())
        };
        if on {
            self.add_configured(feature.id, param, &input, cx);
            self.flash_for(noshape_model::display::FeatureStatus::Info, format!("{} {label} added to {input_name}", feature.name), std::time::Duration::from_millis(6700), cx);
        } else {
            let (fid, p) = (feature.id, param.to_string());
            self.edit_config("Unconfigure parameter", cx, |ps| ps.configuration.configured.retain(|c| !(c.feature == fid && c.param == p)));
        }
        let (configured, _) = self.configured_of(feature.id, cx);
        dialog.update(cx, |d, cx| {
            d.configured = configured;
            cx.notify();
        });
        self.right_panel = Some(RightPanel::Configurations);
        cx.notify();
    }

    pub(super) fn add_configured(&mut self, feature: noshape_model::FeatureId, param: &str, input: &str, cx: &mut Context<Self>) {
        let param = param.to_string();
        let input = input.to_string();
        self.edit_config(noshape_ops::config::CONFIGURE_PARAMETER, cx, |ps| {
            noshape_ops::config::configure_parameter(ps, feature, &param, &input);
        });
    }

    fn set_config_value(&mut self, input: &str, value: String, window: &mut Window, cx: &mut Context<Self>) {
        self.config_values.0.insert(input.to_string(), value);
        crate::release_dialog::note_viewed_config(self.tab, &self.config_values);
        self.schedule_regen(window, cx);
        cx.notify();
    }

    /// The Configurations box's variable / text value fields. Cell inputs
    /// need the window and the box renders without it, so `render` makes
    /// them here.
    pub(super) fn ensure_config_box_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cfg) = self.studio(cx).map(|s| s.configuration.clone()) else { return };
        if cfg.inputs.is_empty() {
            return;
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let (_, effective) = cfg.visibility(&self.config_values);
        for input in cfg.inputs.iter().filter(|i| matches!(i.kind, ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. })) {
            let key = format!("cfgbox-val-{}", input.id);
            // An out-of-range value stays in the field (live 716).
            let typed = self.panel_inputs.get(&key).map(|(st, _)| st.read(cx).value().to_string()).filter(|t| config_value_error(input, t, &units).is_some());
            let current = typed.unwrap_or_else(|| effective.0.get(&input.id).cloned().unwrap_or_default());
            let i = input.clone();
            self.cell_input(key, current, window, cx, move |this, v, cx| {
                // Live (qa5 C8): a bare number takes the workspace unit, "25" →
                // "25 mm" (lengths) / "25 deg" (angles).
                let v = match &i.kind {
                    ConfigInputKind::Variable { kind: VariableKind::Length, .. } => noshape_model::variable_table::with_workspace_unit("LENGTH", &v, &units),
                    ConfigInputKind::Variable { kind: VariableKind::Angle, .. } => noshape_model::variable_table::with_workspace_unit("ANGLE", &v, &units),
                    _ => v,
                };
                // An invalid value stays in the field and isn't applied.
                if config_value_error(&i, &v, &units).is_some() {
                    cx.notify();
                    return;
                }
                this.config_values.0.insert(i.id.clone(), v);
                crate::release_dialog::note_viewed_config(this.tab, &this.config_values);
                this.config_dirty(cx);
            });
        }
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
            // Live L59 (part-studio/2181): the Hole table panel follows.
            .child(btn("panel-holes", "hole-table", "Hole table", RightPanel::HoleTable, cx))
            .child(btn("panel-inspection", "mbd-status", "Inspection table", RightPanel::Inspection, cx))
            .child(btn("panel-vars", "variable-table", "Variable table", RightPanel::Variables, cx))
            .when(self.has_sheet_metal(), |d| d.child(btn("panel-sm", "sheet-metal-table", "Sheet metal: flat pattern and bend table", RightPanel::SheetMetal, cx)))
            .when(self.has_frames(), |d| d.child(btn("panel-cut", "cut-list", "Cut list", RightPanel::CutList, cx)))
            // Live rp-906/914: with the Repair panel open, its wrench "Repair
            // panel" (active, joined to the panel) closes it.
            .when(crate::repair_mode::is_open(&self.doc, cx), |d| {
                let doc = self.doc.clone();
                d.child(
                    div()
                        .id("panel-repair")
                        .w(px(33.))
                        .h(px(34.))
                        .mt(px(-1.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(pal.panel)
                        .hover(|d| d.bg(pal.panel_hover))
                        .cursor_pointer()
                        .border_1()
                        .border_r_0()
                        .border_color(pal.border_strong)
                        .child(crate::assets::icon("repair", 20., cx))
                        .tip("Repair panel", crate::ui::TipSide::Left)
                        .on_click(move |_, _, cx| crate::repair_mode::close(&doc, cx)),
                )
            })
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

/// A configuration variable value outside its range (live psb/s-717:
/// "Enter a value between 0 and 20.0. Current value is 50.", numbers in the
/// range's unit without it).
fn config_value_error(input: &ConfigInput, text: &str, units: &noshape_units::UnitPrefs) -> Option<String> {
    noshape_ops::config::value_error(input, text, units)
}

/// Dragging the splitter under the feature list's Configurations block.
#[derive(Clone)]
pub(super) struct CfgBoxDrag;

impl Render for CfgBoxDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// An input of the Configurations panel dragged by its header.
#[derive(Clone)]
pub(super) struct CfgInputDrag(String);

/// The drag image (live cursors/670): a `#1651b0` pill with the input's
/// name in white bold; the pointer shows `grabbing` everywhere meanwhile.
struct CfgDragPill {
    name: SharedString,
    /// Where the header was grabbed: the pill sits at the pointer instead.
    offset: Point<Pixels>,
}

impl Render for CfgDragPill {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::ui::name_pill(self.name.clone(), self.offset)
    }
}

impl PartStudioView {
    /// A Configurations panel section as a place to drop a dragged input:
    /// below the inputs after the dragged one, above those before it.
    fn cfg_drop_target(&mut self, id: &str, section: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let line = self.ui.cfg_drop.as_ref().filter(|(t, _)| t == id).map(|(_, after)| *after);
        let (over, dropped) = (id.to_string(), id.to_string());
        div()
            .id(SharedString::from(format!("cfg-sec-{id}")))
            .relative()
            .child(section)
            // Sections are 10 px apart: the line is centred in the gap.
            .when_some(line, |d, after| d.child(if after { crate::ui::drop_line().bottom(px(2.)) } else { crate::ui::drop_line().top(px(-8.)) }))
            .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<CfgInputDrag>, _, cx| {
                if !e.bounds.contains(&e.event.position) {
                    return;
                }
                let dragged = e.drag(cx).0.clone();
                let Some(inputs) = this.studio(cx).map(|s| s.configuration.inputs.iter().map(|i| i.id.clone()).collect::<Vec<_>>()) else { return };
                let (Some(from), Some(to)) = (inputs.iter().position(|i| *i == dragged), inputs.iter().position(|i| *i == over)) else { return };
                let drop = (from != to).then(|| (over.clone(), to > from));
                if this.ui.cfg_drop != drop {
                    this.ui.cfg_drop = drop;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(move |this, d: &CfgInputDrag, _, cx| {
                this.ui.cfg_drop = None;
                let (from_id, to_id) = (d.0.clone(), dropped.clone());
                this.edit_config(noshape_ops::config::REORDER_INPUTS, cx, |ps| noshape_ops::config::reorder_inputs(ps, &from_id, &to_id));
                cx.notify();
            }))
            .into_any_element()
    }
}

/// A Variable Studio table being dragged by its header (live psb 10-20 t):
/// the ghost is a dark-blue pill with the table name, and the pointer shows
/// `grabbing` meanwhile.
#[derive(Clone)]
struct VarTableDrag {
    studio: noshape_model::document::TabId,
    name: SharedString,
}

impl Render for VarTableDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(3.))
            .rounded_full()
            .bg(gpui_kit::rgb(0x0b3d78))
            .text_size(px(13.))
            .text_color(gpui_kit::rgb(0xffffff))
            .child(self.name.clone())
            .child(canvas(|_, _, _| {}, |_, _, window, _| window.set_window_cursor_style(CursorStyle::ClosedHand)).absolute().size_0())
    }
}

/// Make `el` (over an inserted table's header) the drag handle for
/// reordering.
fn var_table_drag_source<E: StatefulInteractiveElement>(el: E, studio: noshape_model::document::TabId, name: SharedString) -> E {
    el.on_drag(VarTableDrag { studio, name }, |d, _, _, cx| cx.new(|_| d.clone()))
}
