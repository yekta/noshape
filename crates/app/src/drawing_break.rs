//! Drawing editor: Onshape's Break view dialog (while the Break view tool
//! is active): Horizontal or Vertical, the break line type (Zig zag, Small
//! zig zag, Curve, Straight) and the gap distance; then two clicks in the
//! view place the break lines.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::BreakStyle;

const STYLES: [(&str, BreakStyle); 4] = [("Zig zag", BreakStyle::ZigZag), ("Small zig zag", BreakStyle::SmallZigZag), ("Curve", BreakStyle::Curve), ("Straight", BreakStyle::Straight)];

/// Break view dialog state.
pub struct BreakDlg {
    pub style: BreakStyle,
    /// Gap distance (mm on the sheet).
    pub gap: f64,
    pub gap_input: Entity<InputState>,
    _sub: Subscription,
}

impl DrawingEditor {
    /// Create / drop the dialog with the Break view tool (called from render).
    pub fn sync_break_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Break { .. });
        match (self.break_dlg.is_some(), active) {
            (false, true) => {
                let mm = self.drawing.units_mm;
                let text = if mm { "6 mm".to_string() } else { "0.25 in".to_string() };
                let gap = crate::drawing_section::parse_depth(&text, mm).unwrap_or(0.006) * 1000.0;
                let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text));
                let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                    if matches!(ev, InputEvent::Change)
                        && let Some(v) = crate::drawing_section::parse_depth(&st.read(cx).value(), mm)
                        && let Some(d) = &mut this.break_dlg
                    {
                        d.gap = v * 1000.0;
                    }
                });
                self.break_dlg = Some(BreakDlg { style: BreakStyle::ZigZag, gap, gap_input: st, _sub: sub });
            }
            (true, false) => self.break_dlg = None,
            _ => {}
        }
    }

    fn set_break_vertical(&mut self, vertical: bool, cx: &mut Context<Self>) {
        if let Tool::Break { parent, .. } = self.tool.clone() {
            self.tool = Tool::Break { parent, first: None, vertical };
        }
        self.update_preview(cx);
        cx.notify();
    }

    pub fn render_break_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.break_dlg.as_ref()?;
        let Tool::Break { vertical, .. } = self.tool else { return None };
        let pal = p();
        let tab = |id: &'static str, label: &'static str, on: bool, v: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(9.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .border_b_1()
                .when(on, |d| d.border_b_2().border_color(pal.accent).text_color(pal.emphasis).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.border_color(gpui_kit::rgb(0xdddddd)).text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_break_vertical(v, cx)))
        };
        let style = d.style;
        let ed = cx.entity();
        let styles = Button::new("brk-style").label(STYLES.iter().find(|s| s.1 == style).map(|s| s.0).unwrap_or("Zig zag")).ghost().small().w(px(130.)).dropdown_caret(true).os_dropdown_menu("brk-style-dd", true, move |mut m, _, _| {
            for (n, st) in STYLES {
                let e = ed.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(n).checked(st == style).on_click(move |_, _, cx| {
                    e.update(cx, |t, cx| {
                        if let Some(d) = &mut t.break_dlg {
                            d.style = st;
                        }
                        cx.notify();
                    })
                }));
            }
            m
        });
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(30.)).px(px(6.)).child(div().w(px(90.)).text_size(px(12.)).child(label)).child(body).into_any_element();
        Some(
            v_flex()
                .id("dr-break-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(240.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Break view"))
                        .child(
                            div()
                                .id("brk-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::drawing_chrome::cancel_x(cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
                        ),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(h_flex().child(tab("brk-h", "Horizontal", !vertical, false, cx)).child(tab("brk-v", "Vertical", vertical, true, cx)))
                        .child(row("Break line", styles.into_any_element()))
                        .child(row("Gap distance", div().w(px(130.)).child(Input::new(&d.gap_input).small().text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).pl(px(2.))).into_any_element()))
                        .child(div().px(px(6.)).py(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child("Click in the view to place the two break lines")),
                )
                .into_any_element(),
        )
    }
}
