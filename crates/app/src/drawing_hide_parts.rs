//! Drawing editor: view context menu › Show/hide › Show/hide parts. Onshape's
//! dialog has a "Hidden parts" field: click parts in the view to add (or
//! remove) them, then the checkmark leaves them out of the view.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::drawing::{ViewId, views};

/// Show/hide parts dialog state.
pub struct HidePartsDlg {
    pub view: ViewId,
    /// The Hidden parts field: (body, name).
    pub parts: Vec<(BodyId, String)>,
}

impl DrawingEditor {
    pub fn open_hide_parts(&mut self, id: ViewId, cx: &mut Context<Self>) {
        let hidden = self.current_sheet().view(id).map(|v| v.hidden_parts.clone()).unwrap_or_default();
        let names = self.view_part_names(id);
        let parts = hidden.into_iter().map(|b| {
            let n = names.iter().find(|(x, _)| *x == b).map(|(_, n)| n.clone()).unwrap_or_else(|| "Part".into());
            (b, n)
        });
        self.hide_parts = Some(HidePartsDlg { view: id, parts: parts.collect() });
        cx.notify();
    }

    /// The parts of a view's source: (body, name).
    fn view_part_names(&self, id: ViewId) -> Vec<(BodyId, String)> {
        let Some((key, _)) = views::view_source(self.current_sheet(), id) else { return Vec::new() };
        self.sources.get(&key).map(|s| s.context.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default()
    }

    /// A click while the dialog is open: the part whose edge is nearest the
    /// cursor (in the dialog's view) toggles in the Hidden parts field.
    pub fn hide_parts_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(view) = self.hide_parts.as_ref().map(|d| d.view) else { return false };
        let (Some(pl), Some(g)) = (self.placement(view), self.geoms.get(&view).cloned()) else { return true };
        let p = pl.to_view(self.cursor);
        let tol = self.tol_mm(4.0) / pl.len_to_sheet(1.0).max(1e-12);
        let dist = |a: [f64; 2], b: [f64; 2]| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-18)).clamp(0.0, 1.0);
            ((a[0] + dx * t - p[0]).powi(2) + (a[1] + dy * t - p[1]).powi(2)).sqrt()
        };
        let best = g
            .edges
            .iter()
            .filter(|e| e.visible)
            .filter_map(|e| Some((e.points.windows(2).map(|w| dist(w[0], w[1])).fold(f64::MAX, f64::min), e.model?)))
            .filter(|(d, _)| *d <= tol)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .and_then(|(_, m)| g.model_edges.get(m as usize).map(|me| me.body.clone()));
        let Some(body) = best else { return true };
        let name = self.view_part_names(view).into_iter().find(|(b, _)| *b == body).map(|(_, n)| n).unwrap_or_else(|| "Part".into());
        if let Some(d) = &mut self.hide_parts {
            if d.parts.iter().any(|(b, _)| *b == body) {
                d.parts.retain(|(b, _)| *b != body);
            } else {
                d.parts.push((body, name));
            }
        }
        cx.notify();
        true
    }

    fn close_hide_parts(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.hide_parts.take() else { return };
        if accept {
            let parts: Vec<BodyId> = d.parts.into_iter().map(|(b, _)| b).collect();
            self.edit_view("Show/hide parts", d.view, cx, |v| v.hidden_parts = parts);
        }
        cx.notify();
    }

    pub fn render_hide_parts(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.hide_parts.as_ref()?;
        let pal = p();
        let mut field = v_flex().mx(px(8.)).my(px(6.)).min_h(px(28.)).p(px(4.)).gap(px(2.)).border_1().border_color(pal.accent).bg(gpui_kit::rgb(0xe8f1fb));
        if d.parts.is_empty() {
            field = field.child(div().text_size(px(12.)).text_color(pal.text_muted).child("Hidden parts"));
        }
        for (i, (_, name)) in d.parts.iter().enumerate() {
            field = field.child(
                h_flex()
                    .h(px(20.))
                    .text_size(px(12.))
                    .child(div().flex_1().child(name.clone()))
                    .child(div().id(("hp-remove", i)).cursor_pointer().child(crate::ui::glyph("close", 10., pal.text_muted)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.hide_parts
                            && i < d.parts.len()
                        {
                            d.parts.remove(i);
                        }
                        cx.notify();
                    }))),
            );
        }
        let header_btn = |id: &'static str, accept: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(28.))
                .h(px(27.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .when(accept, |e| e.bg(gpui_kit::rgb(0x009500)).child(crate::ui::glyph("check", 18., gpui_kit::white())))
                .when(!accept, |e| e.child(crate::ui::glyph("close", 14., pal.cancel)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.close_hide_parts(accept, cx)))
        };
        Some(
            v_flex()
                .id("dr-hide-parts")
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Show/hide parts"))
                        .child(header_btn("hp-ok", true, cx))
                        .child(header_btn("hp-cancel", false, cx)),
                )
                .child(field)
                .into_any_element(),
        )
    }
}
