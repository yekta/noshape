//! Variable Studio tab: a document-level table of variables usable as
//! `#name` in every Part Studio of the document.

use crate::doc_model::{DocEvent, DocModel};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
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
}

impl Focusable for VariableStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl VariableStudioView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe_in(&doc, window, |_: &mut Self, _, _: &DocEvent, _, cx| cx.notify())];
        Self { tab, doc, inputs: HashMap::new(), subs, focus: cx.focus_handle(), collapsed: false }
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
        let eval = studio.evaluate(&units);
        let view = cx.entity();
        let n = studio.variables.len();
        let plain = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(gpui_kit::transparent_white()).active(gpui_kit::transparent_white()).shadow(false);
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, v) in studio.variables.iter().enumerate() {
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
            // Onshape shows the typed value; an expression using other
            // variables shows its result until the cell is edited.
            let result = match eval.results.get(i) {
                Some(Ok(q)) => Some(noshape_units::format_quantity(*q, &units)),
                _ => None,
            };
            let failed = matches!(eval.results.get(i), Some(Err(_)));
            let editing = expr.read(cx).focus_handle(cx).is_focused(window);
            let show_result = !editing && !failed && v.expr.text.contains('#');
            let value_cell = if show_result {
                let st = expr.clone();
                cell_div()
                    .id(("vs-val", i))
                    .px(px(7.))
                    .text_size(px(14.))
                    .cursor_text()
                    .child(result.unwrap_or_default())
                    .on_click(move |_, window, cx| st.read(cx).focus_handle(cx).focus(window, cx))
                    .into_any_element()
            } else {
                cell_div().when(failed, |d| d.text_color(pal.error)).child(cell_input(&expr)).into_any_element()
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
                                .custom(plain.clone())
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
                    .context_menu(move |m, _, _| {
                        let (v1, v2, v3) = (menu_view.clone(), menu_view.clone(), menu_view.clone());
                        let m = m.item(PopupMenuItem::new("Delete").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.delete_variable(i, cx))));
                        let m = if i > 0 { m.item(PopupMenuItem::new("Move up").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.move_variable(i, false, cx)))) } else { m };
                        if i + 1 < n { m.item(PopupMenuItem::new("Move down").on_click(move |_, _, cx| v3.update(cx, |t, cx| t.move_variable(i, true, cx)))) } else { m }
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
        let table = v_flex()
            .w(px(COL_W * 4. + 1.))
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
                            .child(div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.collapsed = !this.collapsed;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("vs-more")
                            .custom(plain.clone())
                            .w(px(25.))
                            .h(px(22.))
                            .p_0()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(pal.info_text).child("•••"))
                            .dropdown_menu_with_anchor(Anchor::TopRight, move |m, _, _| {
                                let v = more_view.clone();
                                m.item(PopupMenuItem::new("Copy table").on_click(move |_, _, cx| v.update(cx, |t, cx| t.copy_table(cx))))
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
                .child(h_flex().child(cell_div().border_l_1().italic().child(cell_input(&new_name))))
            });
        // Onshape: the undo / redo toolbar, then the table 42 px lower
        // (under the room of the Insert Variable Studio button).
        v_flex()
            .size_full()
            .track_focus(&self.focus)
            .bg(rgb(0xffffff))
            .child(crate::ui::toolbar().child(crate::ui::undo_redo(&self.doc, cx)))
            .child(v_flex().id("vars").flex_1().min_h_0().pt(px(42.)).items_center().overflow_y_scroll().child(table))
    }
}
