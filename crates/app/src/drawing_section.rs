//! Drawing editor: Onshape's Section view dialog (while the Section view
//! tool is active): Vertical / Horizontal / Angular, the section label,
//! the cutting-line pick button and Flip, "Show cut geometry only" and
//! "Section depth". Vertical and horizontal cuts take one click; angular
//! cuts two.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Section view dialog state.
pub struct SectionDlg {
    /// 0 Vertical, 1 Horizontal, 2 Angular.
    pub mode: u8,
    pub label: Entity<InputState>,
}

impl DrawingEditor {
    /// Create / drop the dialog with the Section view tool (called from render).
    pub fn sync_section_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Section { partial: false, .. });
        match (&self.section_dlg, active) {
            (None, true) => {
                let next = self.current_sheet().next_label();
                let label = cx.new(|cx| InputState::new(window, cx).placeholder("Section id").default_value(next));
                self.section_dlg = Some(SectionDlg { mode: 0, label });
            }
            (Some(_), false) => self.section_dlg = None,
            _ => {}
        }
    }

    /// The cutting line direction (sheet) of the dialog's mode, None for angular.
    pub fn section_axis(&self) -> Option<[f64; 2]> {
        match self.section_dlg.as_ref().map(|d| d.mode) {
            Some(0) => Some([0.0, 1.0]),
            Some(1) => Some([1.0, 0.0]),
            _ => None,
        }
    }

    /// The label typed in the dialog (else the sheet's next free letter).
    pub fn section_label(&self, cx: &App) -> Option<String> {
        let t = self.section_dlg.as_ref()?.label.read(cx).value().trim().to_string();
        (!t.is_empty()).then_some(t)
    }

    fn set_section_mode(&mut self, mode: u8, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.section_dlg {
            d.mode = mode;
        }
        // Start the cut again in the new mode (the view stays chosen).
        if let Tool::Section { parent, partial, .. } = self.tool.clone() {
            self.tool = Tool::Section { parent, p0: None, line: None, partial };
        }
        self.update_preview(cx);
        cx.notify();
    }

    pub fn render_section_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.section_dlg.as_ref()?;
        let pal = p();
        let mode = d.mode;
        let line_done = matches!(self.tool, Tool::Section { line: Some(_), .. });
        let tab = |id: &'static str, label: &'static str, m: u8, cx: &mut Context<Self>| {
            let on = mode == m;
            div()
                .id(id)
                .h(px(26.))
                .px(px(7.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .border_b_1()
                .when(on, |d| d.border_b_2().border_color(pal.accent).text_color(pal.accent).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.border_color(gpui_kit::rgb(0xdddddd)).text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_section_mode(m, cx)))
        };
        Some(
            v_flex()
                .id("dr-section-dlg")
                .absolute()
                .top(px(4.))
                .left(px(4.))
                .w(px(200.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Section view"))
                        // ✓ is enabled once the section view can be placed.
                        .child(div().w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009500)).when(!line_done, |d| d.opacity(0.2)).child(crate::ui::glyph("check", 18., gpui_kit::white())))
                        .child(
                            div()
                                .id("ds-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::ui::glyph("close", 14., pal.cancel))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
                        ),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(h_flex().child(tab("ds-vert", "Vertical", 0, cx)).child(tab("ds-horiz", "Horizontal", 1, cx)).child(tab("ds-ang", "Angular", 2, cx)))
                        .child(div().mt(px(1.)).border_b_1().border_color(pal.accent).child(Input::new(&d.label).appearance(false).small()))
                        .child(
                            h_flex()
                                .mt(px(3.))
                                .h(px(30.))
                                .child(
                                    // The cutting line pick (active until the line is placed; click to pick again).
                                    div()
                                        .id("ds-line")
                                        .size(px(30.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(2.))
                                        .border_1()
                                        .border_color(gpui_kit::rgb(0x9aa5af))
                                        .when(!line_done, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip("Cutting line"))
                                        .child(crate::assets::icon(&crate::assets::icon_path("section-view"), 20., cx))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            let m = this.section_dlg.as_ref().map(|d| d.mode).unwrap_or(0);
                                            this.set_section_mode(m, cx);
                                        })),
                                )
                                .child(div().flex_1())
                                .child(
                                    div()
                                        .id("ds-flip")
                                        .size(px(26.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(gpui_kit::rgb(0xd3d7d9))
                                        .opacity(0.5)
                                        .tooltip(crate::ui::tooltip("Flip direction (drag the view to the other side of the cut)"))
                                        .child(crate::assets::icon(&crate::assets::icon_path("flip"), 18., cx)),
                                ),
                        )
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("ds-cut-only").label("Show cut geometry only").small().disabled(true)))
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("ds-depth").label("Section depth").small().disabled(true))),
                )
                .into_any_element(),
        )
    }
}
