//! Drawing editor: view context menu › Show/hide › Show/hide sketches.
//! Onshape's dialog lists every sketch of the source Part Studio
//! (multi-select); the checkmark shows the selected sketches in the view and
//! hides the unselected ones.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::FeatureId;
use noshape_model::drawing::{ViewId, views};

/// Show/hide sketches dialog state.
pub struct ViewSketchesDlg {
    pub view: ViewId,
    /// Checked sketches.
    pub picked: Vec<FeatureId>,
}

impl DrawingEditor {
    /// The sketches of a view's source Part Studio: (feature, name).
    pub fn view_sketch_list(&self, id: ViewId) -> Vec<(FeatureId, String)> {
        let sheet = self.current_sheet();
        let Some((key, _)) = views::view_source(sheet, id) else { return Vec::new() };
        let Some(src) = self.sources.get(&key) else { return Vec::new() };
        if src.assembly {
            return Vec::new();
        }
        src.context.sketches.iter().map(|s| (s.feature, s.name.clone())).collect()
    }

    pub fn open_view_sketches(&mut self, id: ViewId, cx: &mut Context<Self>) {
        let picked = self.current_sheet().view(id).map(|v| v.shown_sketches.clone()).unwrap_or_default();
        self.view_sketches = Some(ViewSketchesDlg { view: id, picked });
        cx.notify();
    }

    fn close_view_sketches(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.view_sketches.take() else { return };
        if accept {
            let picked = d.picked;
            self.edit_view("Show/hide sketches", d.view, cx, |v| v.shown_sketches = picked);
        }
        cx.notify();
    }

    pub fn render_view_sketches(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.view_sketches.as_ref()?;
        let pal = p();
        let list = self.view_sketch_list(d.view);
        let mut rows = v_flex().p(px(6.)).gap(px(2.));
        for (i, (f, name)) in list.into_iter().enumerate() {
            let on = d.picked.contains(&f);
            rows = rows.child(h_flex().h(px(24.)).child(Checkbox::new(("vs-sketch", i)).label(name).small().checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                if let Some(d) = &mut this.view_sketches {
                    d.picked.retain(|x| *x != f);
                    if *v {
                        d.picked.push(f);
                    }
                }
                cx.notify();
            }))));
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
                .when(!accept, |e| e.child(crate::drawing_chrome::cancel_x(cx)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.close_view_sketches(accept, cx)))
        };
        Some(
            v_flex()
                .id("dr-view-sketches")
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Show/hide sketches"))
                        .child(header_btn("vs-ok", true, cx))
                        .child(header_btn("vs-cancel", false, cx)),
                )
                .child(rows)
                .into_any_element(),
        )
    }
}
