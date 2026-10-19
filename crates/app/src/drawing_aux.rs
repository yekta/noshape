//! Drawing editor: Onshape's Auxiliary view dialog (while the tool is
//! active): the view label, Normal (perpendicular to a linear edge), In line
//! (about a sheet sketch line or a centerline), Axis (down a hole's axis),
//! the reverse button and "Show view reference indicator". The view then
//! follows the cursor along the fold direction; a click places it.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::AnnotationKind;

/// Auxiliary view dialog state.
pub struct AuxDlg {
    /// 0 Normal, 1 In line, 2 Axis.
    pub mode: u8,
    pub label: Entity<InputState>,
    pub reverse: bool,
    pub show_ref: bool,
    /// Where the picked edge / line is (parent view coordinates).
    pub anchor: Option<[f64; 2]>,
}

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn unit(a: [f64; 2]) -> [f64; 2] {
    let l = (a[0] * a[0] + a[1] * a[1]).sqrt().max(1e-12);
    [a[0] / l, a[1] / l]
}

impl DrawingEditor {
    /// Create / drop the dialog with the Auxiliary view tool (called from render).
    pub fn sync_aux_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Auxiliary { .. });
        match (self.aux_dlg.is_some(), active) {
            (false, true) => {
                let next = self.current_sheet().next_label();
                let label = cx.new(|cx| InputState::new(window, cx).default_value(next));
                self.aux_dlg = Some(AuxDlg { mode: 0, label, reverse: false, show_ref: true, anchor: None });
            }
            (true, false) => self.aux_dlg = None,
            _ => {}
        }
    }

    /// The fold edge for the dialog's mode under the cursor: (view, edge
    /// direction on the sheet, anchor in view coordinates).
    pub fn aux_pick(&mut self) -> Option<(noshape_model::drawing::ViewId, [f64; 2], [f64; 2])> {
        let mode = self.aux_dlg.as_ref().map(|d| d.mode).unwrap_or(0);
        let c = self.cursor;
        match mode {
            // In line: a sheet sketch line or a centerline annotation.
            1 => {
                let seg: Option<([f64; 2], [f64; 2])> = self
                    .sketch_line_at(c)
                    .or_else(|| self.current_sheet().annotations.iter().find_map(|a| {
                        let AnnotationKind::Centerline { view, a: pa, b: pb } = &a.kind else { return None };
                        let pl = self.placement(*view)?;
                        let (sa, sb) = (pl.to_sheet(*pa), pl.to_sheet(*pb));
                        (noshape_model::drawing::views::dist_to_segment(c, sa, sb) < self.tol_mm(6.0)).then_some((sa, sb))
                    }));
                let (sa, sb) = seg?;
                let mid = [(sa[0] + sb[0]) / 2.0, (sa[1] + sb[1]) / 2.0];
                let v = self.view_at(mid)?;
                let pl = self.placement(v)?;
                Some((v, unit(sub2(sb, sa)), pl.to_view(mid)))
            }
            // Axis: a hole's axis seen side-on (its centerline or a cylinder
            // silhouette); the view looks down it.
            2 => {
                let v = self.view_at(c)?;
                let pl = self.placement(v)?;
                let g = self.geoms.get(&v)?;
                let vp = pl.to_view(c);
                let tol = self.tol_mm(8.0) / (pl.scale * 1000.0);
                let axis = g
                    .axes
                    .iter()
                    .copied()
                    .filter(|a| noshape_model::drawing::views::dist_to_segment(vp, a[0], a[1]) < tol)
                    .min_by(|a, b| noshape_model::drawing::views::dist_to_segment(vp, a[0], a[1]).total_cmp(&noshape_model::drawing::views::dist_to_segment(vp, b[0], b[1])))
                    .or_else(|| match self.line_pick() {
                        Some(Pick::Line { a, b }) => Some([a.p, b.p]),
                        _ => None,
                    })?;
                let (sa, sb) = (pl.to_sheet(axis[0]), pl.to_sheet(axis[1]));
                let d = unit(sub2(sb, sa));
                Some((v, [-d[1], d[0]], [(axis[0][0] + axis[1][0]) / 2.0, (axis[0][1] + axis[1][1]) / 2.0]))
            }
            // Normal: a straight model edge.
            _ => {
                let Some(Pick::Line { a, b }) = self.line_pick() else { return None };
                let pl = self.placement(a.view)?;
                Some((a.view, unit(sub2(pl.to_sheet(b.p), pl.to_sheet(a.p))), [(a.p[0] + b.p[0]) / 2.0, (a.p[1] + b.p[1]) / 2.0]))
            }
        }
    }

    /// A straight sheet sketch line near a sheet point.
    fn sketch_line_at(&self, c: [f64; 2]) -> Option<([f64; 2], [f64; 2])> {
        let sk = &self.current_sheet().sketch.sketch;
        sk.entities().find_map(|e| {
            let noshape_sketch::Geometry::Line { start, end } = e.geometry else { return None };
            let (a, b) = (sk.point(start)?, sk.point(end)?);
            let (a, b) = ([a.x, a.y], [b.x, b.y]);
            (noshape_model::drawing::views::dist_to_segment(c, a, b) < self.tol_mm(6.0)).then_some((a, b))
        })
    }

    /// Reference indicator › Edit auxiliary view: the dialog for that view
    /// (label, Reverse, Show view reference indicator; the checkmark applies).
    pub fn edit_aux_view(&mut self, id: noshape_model::drawing::ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = self.current_sheet().view(id).cloned() else { return };
        let r = v.aux_ref.clone();
        let label = cx.new(|cx| InputState::new(window, cx).default_value(r.as_ref().map(|r| r.letter.clone()).unwrap_or_default()));
        self.aux_edit = Some((id, AuxDlg { mode: 0, label, reverse: v.flip, show_ref: r.as_ref().is_none_or(|r| r.show), anchor: r.map(|r| r.anchor) }));
        cx.notify();
    }

    fn apply_aux_edit(&mut self, cx: &mut Context<Self>) {
        let Some((id, d)) = self.aux_edit.take() else { return };
        let letter = d.label.read(cx).value().trim().to_string();
        self.edit_view("Edit auxiliary view", id, cx, |v| {
            v.flip = d.reverse;
            if let Some(r) = &mut v.aux_ref {
                r.letter = letter;
                r.show = d.show_ref;
            }
        });
    }

    /// The editing dialog (an existing auxiliary view).
    pub fn render_aux_edit(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, d) = self.aux_edit.as_ref()?;
        let pal = p();
        let (reverse, show) = (d.reverse, d.show_ref);
        Some(
            v_flex()
                .id("dr-aux-edit")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(240.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Auxiliary view"))
                        .child(div().id("auxe-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009500)).cursor_pointer().child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_aux_edit(cx))))
                        .child(div().id("auxe-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., pal.cancel)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.aux_edit = None;
                            cx.notify();
                        }))),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(div().mt(px(1.)).border_b_1().border_color(pal.accent).child(Input::new(&d.label).appearance(false).small()))
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("auxe-rev").label("Reverse").small().checked(reverse).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some((_, d)) = &mut this.aux_edit {
                                d.reverse = *v;
                            }
                            cx.notify();
                        }))))
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("auxe-show").label("Show view reference indicator").small().checked(show).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some((_, d)) = &mut this.aux_edit {
                                d.show_ref = *v;
                            }
                            cx.notify();
                        })))),
                )
                .into_any_element(),
        )
    }

    pub fn render_aux_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.aux_dlg.as_ref()?;
        let pal = p();
        let (mode, reverse, show) = (d.mode, d.reverse, d.show_ref);
        let tab = |id: &'static str, label: &'static str, m: u8, cx: &mut Context<Self>| {
            let on = mode == m;
            div()
                .id(id)
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .border_b_1()
                .when(on, |d| d.border_b_2().border_color(pal.accent).text_color(pal.accent).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.border_color(gpui_kit::rgb(0xdddddd)).text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.aux_dlg {
                        d.mode = m;
                        d.anchor = None;
                    }
                    this.tool = Tool::Auxiliary { parent: None };
                    this.update_preview(cx);
                    cx.notify();
                }))
        };
        let picked = matches!(self.tool, Tool::Auxiliary { parent: Some(_) });
        let field_text = match (picked, mode) {
            (true, _) => "1 entity",
            (false, 1) => "Select a sketch line or centerline",
            (false, 2) => "Select a hole axis",
            _ => "Select a linear edge",
        };
        Some(
            v_flex()
                .id("dr-aux-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(240.))
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Auxiliary view"))
                        .child(
                            div()
                                .id("aux-cancel")
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
                        .child(div().mt(px(1.)).border_b_1().border_color(pal.accent).child(Input::new(&d.label).appearance(false).small()))
                        .child(h_flex().mt(px(3.)).child(tab("aux-normal", "Normal", 0, cx)).child(tab("aux-inline", "In line", 1, cx)).child(tab("aux-axis", "Axis", 2, cx)))
                        .child(
                            h_flex()
                                .mt(px(3.))
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex_1()
                                        .h(px(26.))
                                        .px(px(6.))
                                        .flex()
                                        .items_center()
                                        .text_size(px(12.))
                                        .border_1()
                                        .border_color(if picked { gpui_kit::rgb(0xd3d7d9) } else { pal.accent })
                                        .when(!picked, |d| d.bg(gpui_kit::rgb(0xe8f1fb)))
                                        .child(field_text),
                                )
                                .child(
                                    div()
                                        .id("aux-reverse")
                                        .size(px(26.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(gpui_kit::rgb(0xd3d7d9))
                                        .when(reverse, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip("Reverse"))
                                        .child(crate::assets::icon(&crate::assets::icon_path("flip"), 18., cx))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            if let Some(d) = &mut this.aux_dlg {
                                                d.reverse = !d.reverse;
                                            }
                                            this.update_preview(cx);
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("aux-show-ref").label("Show view reference indicator").small().checked(show).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some(d) = &mut this.aux_dlg {
                                d.show_ref = *v;
                            }
                            this.update_preview(cx);
                            cx.notify();
                        })))),
                )
                .into_any_element(),
        )
    }
}
