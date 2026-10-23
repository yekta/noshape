//! Drawing editor: Onshape's Centermark dialog (while the Centermark tool
//! is active): Single (click a circle, done), Circular (pick the circles of
//! a bolt pattern; Reference center, Bolt circle, Radial lines, Pattern
//! centermark) and Linear (circles in a row; Connecting lines), confirmed
//! with the checkmark. Also "Align centermark" (context menu).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::*;

/// A circle picked for a circular / linear centermark.
pub use noshape_scene::drawing_tools::CmCircle;

/// Centermark dialog state.
#[derive(Clone, Debug)]
pub struct CentermarkDlg {
    /// 0 Single, 1 Circular, 2 Linear.
    pub mode: u8,
    pub reference_center: bool,
    pub bolt_circle: bool,
    pub radial_lines: bool,
    pub pattern: bool,
    pub connecting: bool,
    pub picks: Vec<CmCircle>,
}

impl Default for CentermarkDlg {
    fn default() -> Self {
        Self { mode: 0, reference_center: true, bolt_circle: true, radial_lines: false, pattern: true, connecting: true, picks: Vec::new() }
    }
}

impl DrawingEditor {
    /// Keep the dialog in step with the Centermark tool (called from render).
    pub fn sync_centermark_dlg(&mut self) {
        match (self.centermark_dlg.is_some(), matches!(self.tool, Tool::Centermark)) {
            (false, true) => self.centermark_dlg = Some(CentermarkDlg::default()),
            (true, false) => self.centermark_dlg = None,
            _ => {}
        }
    }

    /// Click with the Centermark tool. True when the click was used.
    pub fn centermark_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(mode) = self.centermark_dlg.as_ref().map(|d| d.mode) else { return false };
        let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(self.cursor) else { return true };
        if mode == 0 {
            let id = self.current_sheet().next_annotation_id();
            let (c, mark) = (self.cursor, CmCircle { view, center, radius, r });
            self.edit_sheet("Centermark", cx, |s| s.annotations.push(noshape_scene::drawing_tools::centermark(id, &mark, c)));
            return true;
        }
        if let Some(d) = &mut self.centermark_dlg {
            // Click again to drop a circle; all circles come from one view.
            if let Some(i) = d.picks.iter().position(|p| p.view == view && (p.center[0] - center[0]).abs() < 1e-9 && (p.center[1] - center[1]).abs() < 1e-9) {
                d.picks.remove(i);
            } else if d.picks.first().is_none_or(|p| p.view == view) {
                d.picks.push(CmCircle { view, center, radius, r });
            }
        }
        self.update_preview(cx);
        cx.notify();
        true
    }

    /// The picked circles of a circular / linear centermark (overlay).
    pub fn centermark_overlay(&self) -> Vec<([f64; 2], f64)> {
        let Some(d) = &self.centermark_dlg else { return vec![] };
        d.picks.iter().filter_map(|c| self.placement(c.view).map(|pl| (pl.to_sheet(c.center), pl.len_to_sheet(c.radius)))).collect()
    }

    /// ✓: create the circular / linear centermark.
    fn accept_centermark(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.centermark_dlg.clone() else { return };
        if d.picks.is_empty() {
            return;
        }
        let pos = self.cursor;
        let opts = noshape_scene::drawing_tools::CentermarkOpts { mode: d.mode, reference_center: d.reference_center, bolt_circle: d.bolt_circle, radial_lines: d.radial_lines, pattern: d.pattern, connecting: d.connecting };
        let (new, name) = noshape_scene::drawing_tools::centermark_set(&d.picks, &opts);
        self.edit_sheet(name, cx, |s| {
            for (kind, refs) in new {
                let id = s.next_annotation_id();
                s.annotations.push(Annotation { id, kind, position: pos, refs, ..Default::default() });
            }
        });
        if let Some(d) = &mut self.centermark_dlg {
            d.picks.clear();
        }
        cx.notify();
    }

    pub fn render_centermark_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.centermark_dlg.clone()?;
        let pal = p();
        let tab = |id: &'static str, label: &'static str, m: u8, cx: &mut Context<Self>| {
            let on = d.mode == m;
            div()
                .id(id)
                .h(px(26.))
                .px(px(9.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .border_b_1()
                .when(on, |e| e.border_b_2().border_color(pal.accent).text_color(pal.emphasis).font_weight(FontWeight::MEDIUM))
                .when(!on, |e| e.border_color(gpui_kit::rgb(0xdddddd)).text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.centermark_dlg {
                        d.mode = m;
                        d.picks.clear();
                    }
                    cx.notify();
                }))
        };
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut CentermarkDlg, bool), cx: &mut Context<Self>| {
            div().h(px(26.)).px(px(4.)).flex().items_center().child(Checkbox::new(id).label(label).small().checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                if let Some(d) = &mut this.centermark_dlg {
                    f(d, *v);
                }
                cx.notify();
            })))
        };
        let can_accept = d.mode != 0 && !d.picks.is_empty();
        let mut body = v_flex().p(px(3.)).child(h_flex().child(tab("cm-single", "Single", 0, cx)).child(tab("cm-circ", "Circular", 1, cx)).child(tab("cm-lin", "Linear", 2, cx)));
        match d.mode {
            1 => {
                body = body
                    .child(check("cm-ref", "Reference center", d.reference_center, |d, v| d.reference_center = v, cx))
                    .child(check("cm-bolt", "Bolt circle", d.bolt_circle, |d, v| d.bolt_circle = v, cx))
                    .child(check("cm-radial", "Radial lines", d.radial_lines, |d, v| d.radial_lines = v, cx))
                    .child(check("cm-pattern", "Pattern centermark", d.pattern, |d, v| d.pattern = v, cx));
            }
            2 => body = body.child(check("cm-connect", "Connecting lines", d.connecting, |d, v| d.connecting = v, cx)),
            _ => {}
        }
        if d.mode != 0 {
            body = body.child(div().px(px(4.)).py(px(3.)).text_size(px(12.)).text_color(pal.text_muted).child(format!("{} circle(s) selected", d.picks.len())));
        }
        Some(
            v_flex()
                .id("dr-centermark-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(220.))
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Centermark"))
                        .child(
                            div()
                                .id("cm-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(0x009500))
                                .when(!can_accept, |e| e.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED)))
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_centermark(cx))),
                        )
                        .child(
                            div()
                                .id("cm-cancel")
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
                .child(body)
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ Align centermark

    pub fn start_align_centermark(&mut self, id: u64, cx: &mut Context<Self>) {
        self.align_centermark = Some(id);
        self.status = Some("Align centermark: select a line or edge".into());
        cx.notify();
    }

    /// Click while aligning a centermark: it turns to the clicked straight edge.
    pub fn align_centermark_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.align_centermark.take() else { return false };
        self.update_hover(cx);
        match self.line_pick() {
            Some(Pick::Line { a, b }) => {
                let angle = (b.p[1] - a.p[1]).atan2(b.p[0] - a.p[0]);
                self.edit_sheet("Align centermark", cx, |s| {
                    if let Some(AnnotationKind::Centermark { angle: x, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                        *x = angle;
                    }
                });
                self.status = None;
            }
            _ => self.status = Some("Align centermark cancelled: select a straight edge".into()),
        }
        cx.notify();
        true
    }
}
