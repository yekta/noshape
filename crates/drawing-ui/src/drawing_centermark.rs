//! Drawing editor: Onshape's Centermark dialog (while the Centermark tool
//! is active): Single (click a circle, done), Circular (pick the circles of
//! a bolt pattern; Reference center, Bolt circle, Radial lines, Pattern
//! centermark) and Linear (circles in a row; Connecting lines), confirmed
//! with the checkmark. Also "Align centermark" (context menu).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
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
    /// Single: the centermark each pick placed at once (live2 2442), by pick.
    pub placed: Vec<u64>,
}

impl Default for CentermarkDlg {
    fn default() -> Self {
        Self { mode: 0, reference_center: true, bolt_circle: true, radial_lines: false, pattern: true, connecting: true, picks: Vec::new(), placed: Vec::new() }
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
        if self.centermark_dlg.is_none() {
            return false;
        }
        let Some(Pick::Circle { view, center, radius, r }) = self.circle_pick(self.cursor) else { return true };
        // A circle with a centermark from before: nothing more, no undo step (live2 tables_live2_1013).
        if self.pk().has_centermark(view, center) && self.centermark_dlg.as_ref().is_none_or(|d| !d.picks.iter().any(|p| p.view == view && p.center == center)) {
            return true;
        }
        // The circles listed under Entities to include; a Single centermark is drawn at once and stays (live2 2442,
        // the click's centermark in the drawing at once), the others on ✓ or Esc.
        let pos = self.cursor;
        let Some(d) = &mut self.centermark_dlg else { return true };
        let single = d.mode == 0;
        // Click again to drop a circle (and a Single's mark); all circles come from one view.
        if let Some(i) = d.picks.iter().position(|p| p.view == view && (p.center[0] - center[0]).abs() < 1e-9 && (p.center[1] - center[1]).abs() < 1e-9) {
            d.picks.remove(i);
            let gone = if single && i < d.placed.len() { Some(d.placed.remove(i)) } else { None };
            if let Some(id) = gone {
                self.edit_sheet("Centermark", cx, |s| s.annotations.retain(|a| a.id != id));
            }
        } else if d.picks.first().is_none_or(|p| p.view == view) {
            let mark = CmCircle { view, center, radius, r };
            d.picks.push(mark.clone());
            if single {
                let mut placed = None;
                self.edit_sheet("Centermark", cx, |s| {
                    let id = s.next_annotation_id();
                    s.annotations.push(noshape_scene::drawing_tools::centermark(id, &mark, pos));
                    placed = Some(id);
                });
                if let Some(d) = &mut self.centermark_dlg {
                    d.placed.extend(placed);
                }
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
    /// Esc with circles listed: they are marked as on ✓ (live 2146), and the tool ends.
    pub fn close_centermark_on_escape(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.centermark_dlg.as_ref().is_some_and(|d| !d.picks.is_empty()) {
            return false;
        }
        self.accept_centermark(cx);
        self.tool = Tool::Select;
        self.centermark_dlg = None;
        cx.notify();
        true
    }

    fn accept_centermark(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.centermark_dlg.clone() else { return };
        if d.picks.is_empty() {
            return;
        }
        let pos = self.cursor;
        if d.mode == 0 {
            // (Each already drawn on its click.)
            if let Some(d) = &mut self.centermark_dlg {
                d.picks.clear();
                d.placed.clear();
            }
            cx.notify();
            return;
        }
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
                .map(|d| crate::ui_script::mark(d, || format!("tab:{label}")))
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
            div().h(px(26.)).px(px(4.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                if let Some(d) = &mut this.centermark_dlg {
                    f(d, *v);
                }
                cx.notify();
            })))
        };
        let can_accept = !d.picks.is_empty();
        // (The panel's texts for the ui-parity runner, as live's recorder reads them: live 2146.)
        let picked = d.picks.len();
        crate::ui_script::note_dialog(|| {
            let mut text: Vec<String> = ["Centermark", "Single", "Circular", "Linear", "Entities to include"].map(String::from).to_vec();
            for i in 0..picked {
                text.extend([format!("Circle/ellipse/arc {}", i + 1), "×".to_string()]);
            }
            serde_json::json!({ "kind": "drawing-panel", "title": "Centermark", "text": text })
        });
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
        // Entities to include: a chip per circle (live 2146).
        let mut field = v_flex().mx(px(4.)).my(px(3.)).p(px(3.)).border_1().border_color(pal.accent).bg(gpui_kit::rgb(0xe8f1fb)).text_size(px(12.)).child("Entities to include");
        for i in 0..d.picks.len() {
            field = field.child(h_flex().h(px(22.)).gap(px(6.)).child(format!("Circle/ellipse/arc {}", i + 1)).child(div().id(("cm-x", i)).cursor_pointer().child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(d) = &mut this.centermark_dlg
                    && i < d.picks.len()
                {
                    d.picks.remove(i);
                }
                this.update_preview(cx);
                cx.notify();
            }))));
        }
        body = body.child(field);
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
                                .child(crate::drawing_chrome::ok_check(18.))
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
        // An automatic centermark: its turn stored on the view.
        if let Some((view, key)) = self.align_auto_centermark.take() {
            self.update_hover(cx);
            match self.line_pick() {
                Some(Pick::Line { a, b }) if a.view == view => {
                    let turn = (b.p[1] - a.p[1]).atan2(b.p[0] - a.p[0]).rem_euclid(std::f64::consts::FRAC_PI_2);
                    self.edit_view("Align centermark", view, cx, |v| {
                        v.centermark_angles.retain(|(k, _)| *k != key);
                        if turn.abs() > 1e-9 {
                            v.centermark_angles.push((key, turn));
                        }
                    });
                    self.status = None;
                }
                _ => self.status = Some("Align centermark cancelled: select a straight edge".into()),
            }
            cx.notify();
            return true;
        }
        let Some(id) = self.align_centermark.take() else { return false };
        self.update_hover(cx);
        match self.line_pick() {
            Some(Pick::Line { a, b }) => {
                self.edit_sheet("Align centermark", cx, |s| noshape_scene::drawing_tools::annotation_edit::align_centermark(s, id, a.p, b.p));
                self.status = None;
            }
            _ => self.status = Some("Align centermark cancelled: select a straight edge".into()),
        }
        cx.notify();
        true
    }
}
