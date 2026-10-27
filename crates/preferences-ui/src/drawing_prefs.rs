//! Preferences › Drawings (live drawing/482, dom/preferences-drawings.txt;
//! the client's `osx-preference-drawings`): "Background color of model space
//! (imported DWG and DXF files)" with the select [Dark view (default), Light
//! view] and "Save drawing settings", faded until the choice changes. Stored
//! in `preferences.json` with the other page settings (`ListPrefs`).

use crate::preferences_window::ListPrefs;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;

/// The select (live 482: 250x34 at the section's left edge).
const SELECT_W: f32 = 250.;
/// The two options, in the live order.
const OPTIONS: [(&str, bool); 2] = [("Dark view (default)", false), ("Light view", true)];

pub struct DrawingPrefsView {
    saved: bool,
    edited: bool,
    /// The select's list (an `OsMenu` native select) is open, and the
    /// field's window bounds to open it under.
    open: Option<Subscription>,
    field_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// When the list last closed: a click on the field that closed it must
    /// not reopen it.
    closed_at: Option<std::time::Instant>,
}

impl DrawingPrefsView {
    pub(crate) fn new() -> Self {
        let saved = ListPrefs::load().drawing_light_model_space;
        Self { saved, edited: saved, open: None, field_bounds: Rc::default(), closed_at: None }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let mut prefs = ListPrefs::load();
        prefs.drawing_light_model_space = self.edited;
        prefs.save();
        self.saved = self.edited;
        cx.notify();
    }

    /// The browser `<select>`: closed, the value and a ▼; open, the native
    /// list right under it, drawn by `os_menu` in this window.
    fn select(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let current = self.edited;
        let bounds = self.field_bounds.clone();
        let label = OPTIONS.iter().find(|o| o.1 == current).map_or("", |o| o.0);
        div()
            .id("drawing-background-selector")
            .relative()
            .w(px(SELECT_W))
            .h(px(34.))
            .pl(px(10.))
            .pr(px(10.))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(3.))
            .border_1()
            .border_color(if self.open.is_some() { rgb(0x1651b0) } else { rgb(0xaaaaaa) })
            .bg(rgb(0xffffff))
            .cursor_pointer()
            .text_size(px(14.))
            .text_color(pal.text)
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., gpui_kit::rgb_static(0x777777)))
            .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if this.open.take().is_some() || this.closed_at.is_some_and(|t| t.elapsed().as_millis() < 300) {
                    cx.notify();
                    return;
                }
                let view = cx.entity();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    m = m.native_select(SELECT_W);
                    for (label, light) in OPTIONS {
                        let v = view.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(label).checked(light == current).on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.edited = light;
                                cx.notify();
                            })
                        }));
                    }
                    m
                });
                this.open = Some(cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
                    this.open = None;
                    this.closed_at = Some(std::time::Instant::now());
                    cx.notify();
                }));
                let b = this.field_bounds.get();
                crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height + px(1.)), Anchor::TopLeft, menu, window, cx);
                cx.notify();
            }))
            .into_any_element()
    }
}

impl Render for DrawingPrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.edited != self.saved;
        let select = self.select(cx);
        v_flex()
            .child(
                h_flex()
                    .h(px(32.))
                    .pl(px(15.))
                    .border_b_1()
                    .border_color(pal.text)
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Drawings")),
            )
            .child(
                v_flex()
                    .pl(px(15.))
                    .pt(px(16.))
                    .child(div().h(px(21.)).text_size(px(14.)).line_height(px(21.)).text_color(pal.text).child("Background color of model space (imported DWG and DXF files)"))
                    .child(div().mt(px(9.)).child(select))
                    .child(
                        h_flex().mt(px(24.)).child(
                            div()
                                .id("drawing-prefs-save")
                                .h(px(33.))
                                .px(px(11.))
                                .flex()
                                .items_center()
                                .rounded(px(3.))
                                .bg(gpui_kit::rgb_static(0x1651b0))
                                .text_size(px(14.))
                                .text_color(gpui_kit::rgb_static(0xffffff))
                                .when(!changed, |d| d.opacity(0.4))
                                .when(changed, |d| d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))))
                                .child("Save drawing settings"),
                        ),
                    ),
            )
    }
}
