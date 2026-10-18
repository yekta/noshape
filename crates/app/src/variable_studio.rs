//! Variable Studio tab: a document-level table of variables usable as
//! `#name` in every Part Studio of the document.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use noshape_model::variables::{VarKind, VariableDef, VariableStudio};
use noshape_units::ExprValue;
use std::collections::HashMap;

pub struct VariableStudioView {
    tab: TabId,
    doc: Entity<DocModel>,
    inputs: HashMap<String, (Entity<InputState>, String)>,
    subs: Vec<Subscription>,
    focus: FocusHandle,
    /// Table collapsed with its title caret.
    collapsed: bool,
    /// Title being renamed (more menu, Rename).
    renaming: Option<Entity<InputState>>,
    /// Shown as a table in this Part Studio's / Assembly's Variable table
    /// panel (no toolbar, button or footer; reference menu).
    host: Option<TabId>,
}

impl Focusable for VariableStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl VariableStudioView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe_in(&doc, window, |_: &mut Self, _, _: &DocEvent, _, cx| cx.notify())];
        Self { tab, doc, inputs: HashMap::new(), subs, focus: cx.focus_handle(), collapsed: false, renaming: None, host: None }
    }

    /// The studio's table inside `host`'s Variable table panel.
    pub fn new_in_panel(doc: Entity<DocModel>, tab: TabId, host: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut v = Self::new(doc, tab, window, cx);
        v.host = Some(host);
        v
    }

    fn studio<'a>(&self, cx: &'a App) -> Option<&'a VariableStudio> {
        match &self.doc.read(cx).session.doc.tab(self.tab)?.content {
            TabContent::VariableStudio(v) => Some(v),
            _ => None,
        }
    }

    fn edit(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut VariableStudio)) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(self.tab, desc, cx, |c| {
                if let TabContent::VariableStudio(v) = c {
                    f(v)
                }
            });
        });
    }

    fn cell(&mut self, key: String, value: String, window: &mut Window, cx: &mut Context<Self>, commit: impl Fn(&mut Self, String, &mut Context<Self>) + 'static) -> Entity<InputState> {
        if let Some((st, last)) = self.inputs.get(&key).cloned() {
            if last != value {
                st.update(cx, |s, cx| s.set_value(value.clone(), window, cx));
                self.inputs.insert(key, (st.clone(), value));
            }
            return st;
        }
        // The add row's cell shows Onshape's "Name" placeholder.
        let placeholder = if key == "new" { "Name" } else { "" };
        let st = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value.clone()));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
                commit(this, v, cx);
            }
        });
        self.subs.push(sub);
        self.inputs.insert(key, (st.clone(), value));
        st
    }
}

/// Onshape's table metrics (live DOM): 140 px columns, 34 px rows, `#aaa`
/// rules, `#eaeaea` title and header rows.
const COL_W: f32 = 140.;
const ROW_H: f32 = 34.;
const RULE: u32 = 0xaaaaaa;
const HEAD_BG: u32 = 0xeaeaea;

/// Whether a value is an expression (references a variable or computes),
/// rather than a plain number with an optional unit ("4 in").
fn is_expression(text: &str) -> bool {
    let t = text.trim();
    t.contains('#') || t.trim_start_matches(['-', '+']).contains(['+', '-', '*', '/', '(', '^'])
}

/// The length unit an expression is written in: the first unit symbol in
/// it, else the unit of the first variable it references (followed a few
/// levels deep).
fn expression_unit(studio: &VariableStudio, text: &str, depth: usize) -> Option<noshape_units::LengthUnit> {
    let words: Vec<&str> = text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '#')).filter(|w| !w.is_empty()).collect();
    for w in &words {
        if let Some(u) = noshape_units::LengthUnit::ALL.iter().find(|u| u.symbol() == *w) {
            return Some(*u);
        }
    }
    if depth > 4 {
        return None;
    }
    words.iter().filter_map(|w| w.strip_prefix('#')).find_map(|name| studio.variables.iter().find(|v| v.name == name).and_then(|v| expression_unit(studio, &v.expr.text, depth + 1)))
}

/// A borderless input whose text starts 7 px into its cell (Onshape).
fn cell_input(st: &Entity<InputState>) -> Div {
    div().flex_1().ml(px(-4.)).child(Input::new(st).appearance(false).text_size(px(14.)))
}

fn cell_div() -> Div {
    div().w(px(COL_W)).h(px(ROW_H)).flex_none().flex().items_center().border_r_1().border_b_1().border_color(rgb(RULE))
}

impl VariableStudioView {
    fn move_variable(&mut self, i: usize, down: bool, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.edit(if down { "Move variable down" } else { "Move variable up" }, cx, |s| {
            let j = if down { i + 1 } else { i.wrapping_sub(1) };
            if i < s.variables.len() && j < s.variables.len() {
                s.variables.swap(i, j);
            }
        })
    }

    fn delete_variable(&mut self, i: usize, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.edit("Delete variable", cx, |s| {
            if i < s.variables.len() {
                s.variables.remove(i);
            }
        })
    }

    /// The more menu's Rename: edit the studio's name in its title row.
    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) && this.renaming.is_some() {
                let name = st.read(cx).value().trim().to_string();
                let tab = this.tab;
                this.renaming = None;
                if !name.is_empty() {
                    this.doc.update(cx, |d, cx| {
                        d.edit_document(&format!("Rename tab : {name}"), cx, |doc| {
                            if let Some(t) = doc.tab_mut(tab) {
                                t.name = name.clone();
                            }
                        })
                    });
                }
                cx.notify();
            }
        });
        self.subs.push(sub);
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.renaming = Some(st);
        cx.notify();
    }

    /// Tables of the Variable Studios inserted into this one: read-only,
    /// titled with the studio's name; more menu: Switch to <studio>, Move up,
    /// Move down, Remove reference.
    fn render_inserted_tables(&self, studio: &VariableStudio, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let n = studio.inserted.len();
        let target = crate::variable_insert::InsertTarget::Studio(self.tab);
        studio
            .inserted
            .iter()
            .enumerate()
            .filter_map(|(k, r)| crate::variable_insert::readonly_table(&self.doc, target, r, k, n, cx))
            .map(|t| div().w(px(COL_W * 4. + 1.)).mb(px(12.)).child(t).into_any_element())
            .collect()
    }

    /// Onshape's footer (live DOM): a 40 px `#eee` bar, 1 px `#ccc` top rule,
    /// centered "Insert into all Part Studios and Assemblies" with its
    /// checkbox (307x35, `#aaa` outline, left corners 5.25) and a `?` menu
    /// button (49x35).
    fn render_footer(&self, on: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let variant = ButtonCustomVariant::new(cx).color(rgb(0xffffff).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        let check = if on {
            div().size(px(14.)).flex().items_center().justify_center().rounded(px(2.)).border_1().border_color(rgb(0x2b8a3e)).child(crate::ui::glyph("check", 12., rgb(0x2b8a3e)))
        } else {
            div().size(px(14.)).rounded(px(2.)).border_1().border_color(rgb(0x888888))
        };
        h_flex()
            .h(px(40.))
            .flex_none()
            .justify_center()
            .bg(rgb(0xeeeeee))
            .border_t_1()
            .border_color(rgb(0xcccccc))
            .child(
                Button::new("vs-insert-all")
                    .custom(variant)
                    .h(px(35.))
                    .px(px(10.5))
                    .rounded_l(px(5.25))
                    .rounded_r(px(0.))
                    .border_1()
                    .border_color(rgb(RULE))
                    .child(h_flex().gap(px(9.)).child(div().w(px(20.)).flex().justify_center().child(check)).child(div().text_size(px(14.)).child("Insert into all Part Studios and Assemblies")))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.edit(if on { "Do not insert into all Part Studios and Assemblies" } else { "Insert into all Part Studios and Assemblies" }, cx, |s| s.insert_into_all = !on)
                    })),
            )
            .child(
                Button::new("vs-insert-all-help")
                    .custom(variant)
                    .h(px(35.))
                    .w(px(49.))
                    .rounded_r(px(5.25))
                    .rounded_l(px(0.))
                    .border_1()
                    .border_l_0()
                    .border_color(rgb(RULE))
                    .child(h_flex().gap(px(4.)).child(crate::assets::icon("help", 20., cx)).child(os::glyph(os::GLYPH_CARET, 7., pal.text)))
                    .dropdown_menu_with_anchor(Anchor::BottomRight, |m, _, _| {
                        m.item(PopupMenuItem::new("Checked: the variables can be used in every Part Studio and Assembly of the workspace.").disabled(true))
                            .item(PopupMenuItem::new("Unchecked: only where the studio is inserted (keep it unchecked while the variables are a draft).").disabled(true))
                    }),
            )
            .into_any_element()
    }

    /// Tab-separated copy of the table (Onshape's "Copy table").
    fn copy_table(&self, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let mut out = String::from("Name\tVariable type\tValue\tDescription\n");
        for v in &studio.variables {
            out.push_str(&format!("{}\t{:?}\t{}\t{}\n", v.name, v.kind.unwrap_or(VarKind::Any), v.expr.text, v.description));
        }
        cx.write_to_clipboard(ClipboardItem::new_string(out));
    }
}

impl Render for VariableStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let studio = self.studio(cx).cloned().unwrap_or_default();
        let (units, title) = {
            let d = &self.doc.read(cx).session.doc;
            (d.settings.units, d.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default())
        };
        // Results with the tables inserted into this studio in scope.
        let eval = noshape_model::variables::studio_evaluation(&self.doc.read(cx).session.doc, self.tab);
        let view = cx.entity();
        let n = studio.variables.len();
        let plain = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(gpui_kit::transparent_white()).active(gpui_kit::transparent_white()).shadow(false);
        let mut rows: Vec<AnyElement> = Vec::new();
        // In a panel, a reference to single variables shows only those.
        let only: Option<Vec<String>> = self.host.and_then(|host| {
            noshape_model::variables::element_studios(&self.doc.read(cx).session.doc, host).into_iter().find(|r| r.studio == self.tab).and_then(|r| r.variables)
        });
        for (i, v) in studio.variables.iter().enumerate() {
            if only.as_ref().is_some_and(|o| !o.contains(&v.name)) {
                continue;
            }
            let name = self.cell(format!("n{i}"), v.name.clone(), window, cx, move |this, val, cx| {
                this.edit("Rename variable", cx, |s| {
                    if let Some(v) = s.variables.get_mut(i) {
                        v.name = val.trim().trim_start_matches('#').to_string();
                    }
                })
            });
            let expr = self.cell(format!("e{i}"), v.expr.text.clone(), window, cx, move |this, val, cx| {
                this.edit("Edit variable", cx, |s| {
                    if let Some(v) = s.variables.get_mut(i) {
                        v.expr = ExprValue::new(val);
                    }
                })
            });
            let desc = self.cell(format!("d{i}"), v.description.clone(), window, cx, move |this, val, cx| {
                this.edit("Edit variable description", cx, |s| {
                    if let Some(v) = s.variables.get_mut(i) {
                        v.description = val;
                    }
                })
            });
            // Onshape (behaviours.md, Variable Studio): a plain value shows as
            // typed; an expression (`#width * 2`) shows its result ("4 in")
            // until the cell is clicked; a failing one is red with the error
            // as its tooltip.
            let (error, result) = match eval.results.get(i) {
                Some(Err(e)) => (Some(e.to_string()), None),
                Some(Ok(q)) => (None, Some(*q)),
                None => (None, None),
            };
            let editing = expr.read(cx).focus_handle(cx).is_focused(window);
            let shown = result.filter(|_| !editing && is_expression(&v.expr.text)).map(|q| {
                let unit = expression_unit(&studio, &v.expr.text, 0).unwrap_or(units.length);
                noshape_units::format_quantity(q, &noshape_units::UnitPrefs { length: unit, ..units })
            });
            let value_cell = match shown {
                Some(text) => {
                    let st = expr.clone();
                    cell_div()
                        .id(("vs-val", i))
                        .px(px(7.))
                        .cursor_text()
                        .child(text)
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            st.read(cx).focus_handle(cx).focus(window, cx);
                            cx.notify();
                        }))
                        .into_any_element()
                }
                None => cell_div()
                    .id(("vs-val", i))
                    .when_some(error, |d, e| d.text_color(pal.error).tooltip(crate::ui::tooltip(e)))
                    .child(cell_input(&expr))
                    .into_any_element(),
            };
            let kind = v.kind.unwrap_or(VarKind::Any);
            let kinds = view.clone();
            let menu_view = view.clone();
            rows.push(
                h_flex()
                    .id(("vs-row", i))
                    .border_l_1()
                    .border_color(rgb(RULE))
                    .bg(rgb(0xffffff))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(cell_div().child(cell_input(&name)))
                    .child(
                        cell_div().relative().child(div().absolute().right(px(6.)).top(px(11.)).child(crate::ui::glyph("ui-caret-down", 12., pal.text))).child(
                            Button::new(("vs-kind", i))
                                .custom(plain)
                                .size_full()
                                .rounded_none()
                                .justify_start()
                                .pl(px(7.))
                                .pt(px(2.))
                                .child(div().text_size(px(14.)).child(format!("{kind:?}")))
                                .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut m, _, _| {
                                    for k in [VarKind::Length, VarKind::Angle, VarKind::Number, VarKind::Any] {
                                        let v = kinds.clone();
                                        m = m.item(PopupMenuItem::new(format!("{k:?}")).checked(k == kind).on_click(move |_, _, cx| {
                                            v.update(cx, |this, cx| {
                                                this.edit("Change variable type", cx, |s| {
                                                    if let Some(var) = s.variables.get_mut(i) {
                                                        var.kind = Some(k);
                                                    }
                                                })
                                            })
                                        }));
                                    }
                                    m
                                }),
                        ),
                    )
                    .child(value_cell)
                    .child(cell_div().child(cell_input(&desc)))
                    .os_context_menu(move |m, _, _| {
                        let (v1, v2, v3) = (menu_view.clone(), menu_view.clone(), menu_view.clone());
                        let m = m.item(crate::os_menu::OsMenuItem::new("Delete").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.delete_variable(i, cx))));
                        let m = if i > 0 { m.item(crate::os_menu::OsMenuItem::new("Move up").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.move_variable(i, false, cx)))) } else { m };
                        if i + 1 < n { m.item(crate::os_menu::OsMenuItem::new("Move down").on_click(move |_, _, cx| v3.update(cx, |t, cx| t.move_variable(i, true, cx)))) } else { m }
                    })
                    .into_any_element(),
            );
        }
        // The last row: an italic "Name" cell; typing a name adds a Length
        // variable set to 0 in the document's length unit.
        let unit = units.length.symbol();
        let new_name = self.cell("new".into(), String::new(), window, cx, move |this, val, cx| {
            let name = val.trim().trim_start_matches('#').to_string();
            if name.is_empty() {
                return;
            }
            this.inputs.remove("new");
            this.edit("Add variable", cx, |s| s.variables.push(VariableDef { name, expr: ExprValue::new(format!("0 {unit}")), kind: Some(VarKind::Length), description: String::new() }));
        });
        let collapsed = self.collapsed;
        let more_view = view.clone();
        let host_menu = self.host.map(|host| {
            let d = &self.doc.read(cx).session.doc;
            let list = noshape_model::variables::element_studios(d, host);
            let k = list.iter().position(|r| r.studio == self.tab).unwrap_or(0);
            (self.doc.clone(), host, self.tab, title.clone(), k, list.len())
        });
        // In a panel the title row spans the panel (live, part-studio/77).
        let in_panel = self.host.is_some();
        let table = v_flex()
            .when(!in_panel, |d| d.w(px(COL_W * 4. + 1.)))
            .when(in_panel, |d| d.w_full())
            .child(
                h_flex()
                    .h(px(35.))
                    .px(px(6.))
                    .bg(rgb(HEAD_BG))
                    .border_1()
                    .border_color(rgb(RULE))
                    .child(
                        h_flex()
                            .id("vs-collapse")
                            .gap(px(3.))
                            .cursor_pointer()
                            .child(crate::ui::glyph(if collapsed { "ui-chevron-right" } else { "ui-chevron-down" }, 11., pal.text))
                            .child(match &self.renaming {
                                Some(st) => div().w(px(260.)).text_size(px(16.)).child(Input::new(st).h(px(26.)).text_size(px(16.))).into_any_element(),
                                None => div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title).into_any_element(),
                            })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.collapsed = !this.collapsed;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("vs-more")
                            .custom(plain)
                            .w(px(25.))
                            .h(px(22.))
                            .p_0()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(pal.info_text).child("•••"))
                            .dropdown_menu_with_anchor(Anchor::TopRight, move |m, _, _| {
                                let (v, v2) = (more_view.clone(), more_view.clone());
                                let m = m.item(PopupMenuItem::new("Copy table").on_click(move |_, _, cx| v.update(cx, |t, cx| t.copy_table(cx))));
                                match host_menu.clone() {
                                    // In a Part Studio / Assembly panel: the reference's menu.
                                    Some((doc, host, me, name, k, n)) => {
                                        let (d1, d2, d3) = (doc.clone(), doc.clone(), doc.clone());
                                        let m = m.item(PopupMenuItem::new(format!("Switch to {name}")).on_click(move |_, window, cx| crate::workspace::request_activate_tab(me, window, cx)));
                                        let m = if k > 0 { m.item(PopupMenuItem::new("Move up").on_click(move |_, _, cx| crate::variable_insert::move_table(&d1, host, me, false, cx))) } else { m };
                                        let m = if k + 1 < n { m.item(PopupMenuItem::new("Move down").on_click(move |_, _, cx| crate::variable_insert::move_table(&d2, host, me, true, cx))) } else { m };
                                        m.item(PopupMenuItem::new("Remove reference").on_click(move |_, _, cx| crate::variable_insert::set_ref(&d3, crate::variable_insert::InsertTarget::Element(host), me, None, cx)))
                                    }
                                    None => m.item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| v2.update(cx, |t, cx| t.start_rename(window, cx)))),
                                }
                            }),
                    ),
            )
            .when(!collapsed, |t| {
                t.child(
                    h_flex()
                        .border_l_1()
                        .border_color(rgb(RULE))
                        .bg(rgb(HEAD_BG))
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(pal.text)
                        .children(["Name", "Variable type", "Value", "Description"].map(|h| cell_div().justify_center().child(h))),
                )
                .children(rows)
                .when(only.is_none(), |t| t.child(h_flex().child(cell_div().border_l_1().italic().child(cell_input(&new_name)))))
            });
        if self.host.is_some() {
            return v_flex().track_focus(&self.focus).child(table).into_any_element();
        }
        // Onshape: "Insert Variable Studio" (178x35, `#aaa` outline, radius
        // 5) centered above the tables; it opens the Insert Variable Studio
        // dialog for this studio.
        let doc = self.doc.clone();
        let me = self.tab;
        let insert = Button::new("vs-insert")
            .custom(ButtonCustomVariant::new(cx).color(rgb(0xffffff).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false))
            .h(px(35.))
            .px(px(10.5))
            .rounded(px(5.25))
            .border_1()
            .border_color(rgb(RULE))
            .child(h_flex().gap(px(3.)).child(crate::assets::icon("variable-studio", 20., cx)).child(div().text_size(px(14.)).child("Insert Variable Studio")))
            .tooltip("Insert Variable Studio")
            .on_click(move |_, window, cx| crate::variable_insert::open(doc.clone(), crate::variable_insert::InsertTarget::Studio(me), window, cx));
        let inserted = self.render_inserted_tables(&studio, cx);
        let footer = self.render_footer(studio.insert_into_all, cx);
        // Onshape: the undo / redo toolbar, then the button and the table.
        v_flex()
            .size_full()
            .track_focus(&self.focus)
            .bg(rgb(0xffffff))
            .child(crate::ui::toolbar().child(crate::ui::undo_redo(&self.doc, cx)))
            .child(v_flex().id("vars").flex_1().min_h_0().pt(px(5.)).items_center().overflow_y_scroll().child(insert).child(div().h(px(2.))).children(inserted).child(table))
            .child(footer)
            .into_any_element()
    }
}
