//! Drawing editor: Onshape's Format painter dialog. Choose Paste style,
//! Paste tolerance and precision and / or Paste content, click the source
//! dimension or annotation, then every destination; the checkmark closes.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_model::drawing::painter::{PaintOptions, paint};

/// Format painter state: options and the source annotation.
#[derive(Clone, Copy, Debug, Default)]
pub struct PainterState {
    pub options: PaintOptions,
    pub source: Option<u64>,
}

impl DrawingEditor {
    pub fn toggle_format_painter(&mut self, cx: &mut Context<Self>) {
        self.painter = if self.painter.is_some() { None } else { Some(PainterState::default()) };
        self.tool = Tool::Select;
        self.status = self.painter.map(|_| "Format painter: select the dimension or annotation to copy from".into());
        cx.notify();
    }

    /// Click while the Format painter is open. True when used.
    pub fn painter_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(st) = self.painter else { return false };
        self.update_hover(cx);
        let Some(Sel::Annotation(id)) = self.sel_at(self.hover, self.cursor) else { return true };
        match st.source {
            None => {
                self.painter = Some(PainterState { source: Some(id), ..st });
                self.selection = vec![Sel::Annotation(id)];
                self.status = Some("Format painter: select the destinations, then the checkmark".into());
            }
            Some(src) if src != id => {
                let Some(source) = self.current_sheet().annotations.iter().find(|a| a.id == src).cloned() else { return true };
                self.edit_sheet("Format painter", cx, |s| {
                    if let Some(dst) = s.annotations.iter_mut().find(|a| a.id == id) {
                        paint(&source, dst, st.options);
                    }
                });
            }
            Some(_) => {}
        }
        self.redraw(cx);
        true
    }

    pub fn render_painter(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let st = self.painter?;
        let pal = p();
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut PaintOptions, bool), cx: &mut Context<Self>| {
            div().h(px(26.)).px(px(6.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                if let Some(p) = &mut this.painter {
                    f(&mut p.options, *v);
                }
                cx.notify();
            })))
        };
        let o = st.options;
        Some(
            v_flex()
                .id("dr-painter")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(250.))
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Format painter"))
                        .child(
                            div()
                                .id("fp-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(0x009500))
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.painter = None;
                                    this.status = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("fp-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::drawing_chrome::cancel_x(cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.painter = None;
                                    this.status = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(check("fp-style", "Paste style", o.style, |o, v| o.style = v, cx))
                        .child(check("fp-tol", "Paste tolerance and precision", o.tolerance_precision, |o, v| o.tolerance_precision = v, cx))
                        .child(check("fp-content", "Paste content", o.content, |o, v| o.content = v, cx))
                        .child(div().px(px(6.)).py(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child(if st.source.is_some() { "Select the destinations" } else { "Select the source dimension or annotation" })),
                )
                .into_any_element(),
        )
    }
}
