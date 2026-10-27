//! Drawing editor: view menu › Show/hide › Show/hide edges… (help
//! H:view_context_menu, images show-hide-edges-03 / -05): a small dialog
//! "Show/hide edges" (✓ ✕, "Select edges in a view to show/hide"). While
//! it is open the view's hidden edges draw thick and grey and occluded
//! edges grey; clicking an edge marks it hidden (grey) or shown again
//! (black). ✓ applies, ✕ / Esc cancels.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_model::drawing::ViewId;
use noshape_model::reference::{BodyId, TopoName};

/// The open Show/hide edges dialog: the view and its edges marked hidden.
pub struct HideEdgesDlg {
    pub view: ViewId,
    pub edges: Vec<(BodyId, TopoName)>,
}

impl DrawingEditor {
    pub fn open_hide_edges(&mut self, view: ViewId, cx: &mut Context<Self>) {
        let edges = self.current_sheet().view(view).map(|v| v.hidden_edges.clone()).unwrap_or_default();
        // The view's selection highlight would cover the grey marks.
        self.selection.clear();
        self.hide_edges = Some(HideEdgesDlg { view, edges });
        self.preview_hide_edges(cx);
    }

    /// The drawing with the dialog's marks; `editing` shows them grey.
    fn edges_marked(&self, editing: bool) -> Option<noshape_model::drawing::Drawing> {
        let d = self.hide_edges.as_ref()?;
        let mut dr = self.drawing.clone();
        let v = dr.sheets.get_mut(self.sheet)?.view_mut(d.view)?;
        v.hidden_edges = d.edges.clone();
        v.editing_edges = editing;
        Some(dr)
    }

    fn preview_hide_edges(&mut self, cx: &mut Context<Self>) {
        let dr = self.edges_marked(true);
        self.set_preview(dr, cx);
    }

    /// A click with the dialog open: mark the hovered edge of its view
    /// hidden, or shown again. True when the dialog is open.
    pub fn hide_edges_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(view) = self.hide_edges.as_ref().map(|d| d.view) else { return false };
        if let Some((v, me)) = self.hovered_edge()
            && v == view
            && let Some(d) = &mut self.hide_edges
        {
            let key = (me.body.clone(), (*me.name).clone());
            match d.edges.iter().position(|e| *e == key) {
                Some(i) => {
                    d.edges.remove(i);
                }
                None => d.edges.push(key),
            }
            self.preview_hide_edges(cx);
        }
        true
    }

    pub fn close_hide_edges(&mut self, accept: bool, cx: &mut Context<Self>) {
        let dr = if accept { self.edges_marked(false) } else { None };
        self.hide_edges = None;
        self.set_preview(None, cx);
        if let Some(dr) = dr
            && dr != self.drawing
        {
            self.commit("Show/hide edges", dr, cx);
        }
        cx.notify();
    }

    pub fn render_hide_edges(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.hide_edges.as_ref()?;
        let pal = p();
        Some(
            v_flex()
                .id("dr-hide-edges")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(260.))
                .pb(px(8.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(gpui_kit::rgb(0x333333))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(10.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Show/hide edges"))
                        .child(div().id("he-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_hide_edges(true, cx))))
                        .child(div().id("he-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., pal.cancel)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_hide_edges(false, cx)))),
                )
                .child(div().px(px(10.)).pt(px(6.)).text_size(px(13.)).child("Select edges in a view to show/hide"))
                .into_any_element(),
        )
    }
}
