//! Variable Studio tab: a document-level table of variables usable as
//! `#name` in every Part Studio of the document.

use crate::doc_model::{DocEvent, DocModel};
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
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
}

impl Focusable for VariableStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl VariableStudioView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe_in(&doc, window, |_: &mut Self, _, _: &DocEvent, _, cx| cx.notify())];
        Self { tab, doc, inputs: HashMap::new(), subs, focus: cx.focus_handle() }
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
        let st = cx.new(|cx| InputState::new(window, cx).default_value(value.clone()));
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

impl Render for VariableStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let studio = self.studio(cx).cloned().unwrap_or_default();
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let eval = studio.evaluate(&units);
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
            let result = match eval.results.get(i) {
                Some(Ok(q)) => (noshape_units::format_quantity(*q, &units), pal.text),
                Some(Err(e)) => (e.to_string(), pal.error),
                None => (String::new(), pal.text_faint),
            };
            let kind = v.kind.unwrap_or(VarKind::Any);
            let view = cx.entity();
            rows.push(
                h_flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().w(px(24.)).text_xs().text_color(pal.text_faint).child(format!("{}", i + 1)))
                    .child(div().w(px(180.)).child(Input::new(&name).small().prefix(div().text_color(pal.accent_hover).child("#"))))
                    .child(
                        Button::new(SharedString::from(format!("k{i}")))
                            .label(format!("{kind:?}"))
                            .outline()
                            .small()
                            .w(px(96.))
                            .dropdown_caret(true)
                            .dropdown_menu(move |mut m, _, _| {
                                for k in [VarKind::Length, VarKind::Angle, VarKind::Number, VarKind::Any] {
                                    let v = view.clone();
                                    m = m.item(PopupMenuItem::new(format!("{k:?}")).on_click(move |_, _, cx| {
                                        v.update(cx, |this, cx| this.edit("Change variable type", cx, |s| {
                                            if let Some(var) = s.variables.get_mut(i) {
                                                var.kind = Some(k);
                                            }
                                        }))
                                    }));
                                }
                                m
                            }),
                    )
                    .child(div().flex_1().child(Input::new(&expr).small()))
                    .child(div().w(px(200.)).text_sm().text_color(result.1).truncate().child(result.0))
                    .child(Button::new(SharedString::from(format!("d{i}"))).icon(IconName::Delete).ghost().small().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.inputs.clear();
                        this.edit("Delete variable", cx, |s| {
                            if i < s.variables.len() {
                                s.variables.remove(i);
                            }
                        })
                    })))
                    .into_any_element(),
            );
        }
        v_flex()
            .size_full()
            .track_focus(&self.focus)
            .bg(pal.app_bg)
            .child(
                h_flex()
                    .h(px(40.))
                    .px_3()
                    .gap_2()
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Variable Studio"))
                    .child(div().text_xs().text_color(pal.text_faint).child("Variables here can be used as #name in every Part Studio of this document."))
                    .child(div().flex_1())
                    .child(Button::new("add-var").label("Add variable").icon(IconName::Plus).primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.edit("Add variable", cx, |s| {
                            let n = s.variables.len() + 1;
                            s.variables.push(VariableDef { name: format!("var{n}"), expr: ExprValue::new("10 mm"), kind: Some(VarKind::Length), description: String::new() });
                        })
                    }))),
            )
            .child(
                h_flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(pal.text_muted)
                    .child(div().w(px(24.)))
                    .child(div().w(px(180.)).child("Name"))
                    .child(div().w(px(96.)).child("Type"))
                    .child(div().flex_1().child("Expression"))
                    .child(div().w(px(200.)).child("Value"))
                    .child(div().w(px(28.))),
            )
            .child(v_flex().id("vars").flex_1().overflow_y_scroll().children(rows))
    }
}
