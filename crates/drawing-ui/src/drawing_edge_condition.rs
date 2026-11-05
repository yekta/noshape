//! Drawing editor: Onshape's Edge condition symbol panel (ISO 13715; live behaviours "dx (2026-11-04 00:19, P2)
//! drawing Edge condition symbol", shots dx/edge-condition-1955): Direction ▾, the main value box left of two rows
//! (a sign ▾ None / + / − / ± and its value, disabled while None), the glyph's preview, All around and ( ). Click an
//! edge, then where the symbol goes; the panel stays open for the next one with the same values until ✓ (enabled
//! once one is placed). Edit… / double-click reopens it on a placed symbol.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::edge_condition::{EdgeConditionSpec, EdgeDirection, EdgeSign};
use noshape_model::drawing::{AnnotationKind, ViewId};

/// Panel width (px, live: 40–280).
const WIDTH: f32 = 240.;

pub struct EdgeConditionDlg {
    pub spec: EdgeConditionSpec,
    /// The symbol being edited (Edit… / double-click).
    pub edit: Option<u64>,
    /// Symbols placed since the tool opened (✓ enabled).
    pub placed: usize,
    /// The edge point the leader goes to (view, point; sheet when no view).
    pub target: Option<(Option<ViewId>, [f64; 2])>,
    main: Entity<InputState>,
    values: [Entity<InputState>; 2],
}

impl DrawingEditor {
    fn new_edge_condition_dlg(&mut self, spec: EdgeConditionSpec, edit: Option<u64>, window: &mut Window, cx: &mut Context<Self>) -> EdgeConditionDlg {
        let input = |v: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(v));
        // A row's value in the drawing's format (live echoes ".300" / "0,3"); none while its sign is None.
        let shown = |i: usize| if spec.rows[i].sign == EdgeSign::None { String::new() } else { noshape_model::drawing::format::format_len(&self.drawing, &Default::default(), spec.rows[i].value) };
        let (v0, v1) = (shown(0), shown(1));
        let main = input(spec.main.clone(), window, cx);
        let values = [input(v0, window, cx), input(v1, window, cx)];
        EdgeConditionDlg { spec, edit, placed: 0, target: None, main, values }
    }

    /// Create / drop the panel with the tool (called from render).
    pub fn sync_edge_condition_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::EdgeCondition);
        let placing = self.edge_condition_dlg.as_ref().is_some_and(|d| d.edit.is_none());
        if active && self.edge_condition_dlg.is_none() {
            self.edge_condition_dlg = Some(self.new_edge_condition_dlg(EdgeConditionSpec::default(), None, window, cx));
        } else if !active && placing {
            self.edge_condition_dlg = None;
        }
    }

    /// Edit… / double-click on an edge condition symbol.
    pub fn open_edge_condition_edit(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spec) = self.current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| match &a.kind {
            AnnotationKind::EdgeCondition { spec, .. } => Some((**spec).clone()),
            _ => None,
        }) else {
            return;
        };
        self.tool = Tool::Select;
        self.edge_condition_dlg = Some(self.new_edge_condition_dlg(spec, Some(id), window, cx));
        cx.notify();
    }

    /// The spec with the typed values (numbers in the drawing's units, or lengths; a decimal comma reads as a point).
    fn edge_condition_spec(&self, cx: &App) -> Option<EdgeConditionSpec> {
        let d = self.edge_condition_dlg.as_ref()?;
        let mut s = d.spec.clone();
        s.main = d.main.read(cx).value().trim().to_string();
        let unit = noshape_model::drawing::format::length_units(&self.drawing, &Default::default()).meters();
        for (row, input) in s.rows.iter_mut().zip(&d.values) {
            let text = input.read(cx).value().trim().replace(',', ".");
            if text.is_empty() {
                continue;
            }
            row.value = match text.parse::<f64>() {
                Ok(v) => v * unit,
                Err(_) => noshape_units::evaluate_as(&text, noshape_units::Dim::LENGTH, &Default::default(), &noshape_units::EmptyScope).map(|q| q.value).unwrap_or(row.value),
            };
        }
        Some(s)
    }

    /// Tool clicks: the edge, then the symbol's place. True when used.
    pub fn edge_condition_click(&mut self, cx: &mut Context<Self>) -> bool {
        if !matches!(self.tool, Tool::EdgeCondition) {
            return false;
        }
        let c = self.cursor;
        match self.edge_condition_dlg.as_ref().and_then(|d| d.target) {
            None => {
                let t = self.pk().weld_target(c);
                if let Some(d) = &mut self.edge_condition_dlg {
                    d.target = Some(t);
                }
            }
            Some((view, target)) => {
                let Some(spec) = self.edge_condition_spec(cx) else { return true };
                let id = self.current_sheet().next_annotation_id();
                self.edit_sheet("Edge condition symbol", cx, |s| s.annotations.push(noshape_scene::drawing_tools::edge_condition(id, view, target, spec, c)));
                if let Some(d) = &mut self.edge_condition_dlg {
                    d.placed += 1;
                    d.target = None;
                }
            }
        }
        cx.notify();
        true
    }

    /// The leader's preview while placing.
    pub fn edge_condition_overlay(&self, overlay: &mut crate::drawing_scene::Overlay) {
        let Some((view, t)) = self.edge_condition_dlg.as_ref().and_then(|d| d.target) else { return };
        let t = match view.and_then(|v| self.placement(v)) {
            Some(pl) => pl.to_sheet(t),
            None => t,
        };
        overlay.lines.push((vec![t, self.cursor], true));
    }

    /// ✓ ends the tool (an edited symbol takes the panel's values); ✗ leaves an edited symbol as it was.
    pub fn close_edge_condition_dlg(&mut self, ok: bool, cx: &mut Context<Self>) {
        let spec = self.edge_condition_spec(cx);
        let Some(d) = self.edge_condition_dlg.take() else { return };
        if let (true, Some(id), Some(spec)) = (ok, d.edit, spec) {
            self.edit_sheet("Edit edge condition symbol", cx, |s| {
                if let Some(AnnotationKind::EdgeCondition { spec: sp, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                    **sp = spec;
                }
            });
        }
        self.tool = Tool::Select;
        cx.notify();
    }

    pub fn render_edge_condition_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.edge_condition_dlg.as_ref()?;
        let pal = p();
        let s = d.spec.clone();
        let (main, values) = (d.main.clone(), d.values.clone());
        let can_accept = d.edit.is_some() || d.placed > 0;
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        // A dropdown of `items` (label, on, change).
        type Set = std::rc::Rc<dyn Fn(&mut EdgeConditionSpec)>;
        let dropdown = |id: &'static str, label: &str, w: f32, items: Vec<(&'static str, bool, Set)>| {
            let ed = ed.clone();
            Button::new(id).ghost().xsmall().w(px(w)).h(px(26.)).dropdown_caret(true).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(3.)).label(label.to_string()).os_dropdown_menu(SharedString::from(format!("{id}-dd")), false, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(*l).checked(*on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.edge_condition_dlg {
                                f(&mut d.spec);
                            }
                            cx.notify();
                        })
                    }));
                }
                m
            })
        };
        let directions: Vec<(&'static str, bool, Set)> = EdgeDirection::ALL.iter().map(|&x| (x.label(), x == s.direction, std::rc::Rc::new(move |s: &mut EdgeConditionSpec| s.direction = x) as Set)).collect();
        let row = |i: usize| {
            let signs: Vec<(&'static str, bool, Set)> = EdgeSign::ALL.iter().map(|&x| (x.label(), x == s.rows[i].sign, std::rc::Rc::new(move |s: &mut EdgeConditionSpec| s.rows[i].sign = x) as Set)).collect();
            let off = s.rows[i].sign == EdgeSign::None;
            h_flex()
                .gap(px(4.))
                .child(dropdown(if i == 0 { "ec-sign-0" } else { "ec-sign-1" }, s.rows[i].sign.label(), 60., signs))
                .child(div().w(px(74.)).h(px(28.)).when(off, |d| d.opacity(0.45).bg(gpui_kit::rgb(0xeeeeee))).child(Input::new(&values[i]).small().text_size(px(13.33)).disabled(off)))
        };
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut EdgeConditionSpec), cx: &mut Context<Self>| {
            crate::drawing_chrome::DrCheckbox::new(id).label(label).checked(on).on_click(cx.listener(move |this: &mut DrawingEditor, _: &bool, _, cx| {
                if let Some(d) = &mut this.edge_condition_dlg {
                    f(&mut d.spec);
                }
                cx.notify();
            }))
        };
        Some(
            v_flex()
                .id("dr-edge-condition-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(WIDTH))
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
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Edge condition"))
                        .child(div().id("ec-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().bg(gpui_kit::rgb(0x009500)).when(!can_accept, |d| d.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED))).child(crate::drawing_chrome::ok_check(18.)).when(can_accept, |d| d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_edge_condition_dlg(true, cx)))))
                        .child(div().id("ec-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_edge_condition_dlg(false, cx)))),
                )
                .child(
                    v_flex()
                        .p(px(4.))
                        .gap(px(6.))
                        .child(h_flex().gap(px(4.)).child("Direction").child(dropdown("ec-direction", s.direction.label(), 103., directions)))
                        .child(h_flex().gap(px(4.)).child(div().w(px(90.)).h(px(28.)).child(Input::new(&main).small().text_size(px(13.33)))).child(v_flex().gap(px(6.)).child(row(0)).child(row(1))))
                        .child(
                            h_flex()
                                .gap(px(30.))
                                .child(div().pl(px(14.)).child(crate::ui::glyph("drawing-edge-condition-preview", 56., ink)))
                                .child(v_flex().gap(px(12.)).child(check("ec-all-around", "All around", s.all_around, |s| s.all_around = !s.all_around, cx)).child(check("ec-parens", "( )", s.parens, |s| s.parens = !s.parens, cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}
