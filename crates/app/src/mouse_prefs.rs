//! Preferences › Mouse controls (live DOM `dom/preferences-material-libraries.txt`
//! lines 185–247; the client's `osx-preference-mouse-controls`): the "View
//! settings" preset dropdown, the "3D Part & Assembly" / "2D Drawing" table
//! of what each drag does, the three checkboxes and "Save mouse controls".
//! The settings drive `navigation.rs`.

use crate::navigation::{MouseControls, Preset};
use crate::theme::p;
use gpui_kit::component::{Icon, IconName, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;

/// The table: 800 px, three 267 px columns, 28 px header, 80 px rows (the
/// three-line Pan row 86 px).
const TABLE_W: f32 = 800.;
const COL_W: f32 = 267.;
const HEADER_H: f32 = 28.;
const ROW_H: f32 = 80.;
const PAN_ROW_H: f32 = 86.;
/// Header underline `rgb(154, 165, 175)`.
const HEADER_RULE: u32 = 0x9aa5af;
/// Browser select (`form-control`): 170x34, 1 px `#aaa`, radius 5.25.
const SELECT_W: f32 = 170.;

pub struct MousePrefsView {
    saved: MouseControls,
    edited: MouseControls,
    /// The select's list (an `OsMenu` native select) is open, and the
    /// field's window bounds to open it under.
    open: Option<Subscription>,
    field_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// When the list last closed: a click on the field that closed it
    /// (mouse down outside the list) must not reopen it.
    closed_at: Option<std::time::Instant>,
}

impl MousePrefsView {
    pub fn new() -> Self {
        let saved = MouseControls::current();
        Self { saved, edited: saved, open: None, field_bounds: Rc::default(), closed_at: None }
    }

    /// Picking a preset (`onSelectedChoiceChange` + `setDefaultMouseScrollDirection`):
    /// SOLIDWORKS ticks "Reverse scroll wheel zoom direction" unless it is the
    /// saved preset (then the saved tick comes back).
    fn pick(&mut self, preset: Preset, cx: &mut Context<Self>) {
        self.edited.preset = preset;
        if preset == Preset::Solidworks {
            self.edited.reverse_scroll = if self.saved.preset != Preset::Solidworks { true } else { self.saved.reverse_scroll };
        }
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.edited.save();
        self.saved = self.edited;
        cx.notify();
    }

    fn checkbox(id: &'static str, label: &'static str, on: bool) -> Stateful<Div> {
        let pal = p();
        h_flex()
            .id(id)
            .h(px(26.))
            .gap(px(8.))
            .cursor_pointer()
            .text_size(px(14.))
            .line_height(px(21.))
            .text_color(pal.text)
            .child(
                div()
                    .size(px(13.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.))
                    .border_1()
                    .border_color(if on { rgb(0x0075ff) } else { rgb(0x767676) })
                    .bg(if on { rgb(0x0075ff) } else { rgb(0xffffff) })
                    .when(on, |d| d.child(Icon::new(IconName::Check).size(px(11.)).text_color(rgb(0xffffff)))),
            )
            .child(label)
    }

    /// The browser `<select>` (live 62): closed, the value and a ▼; open, the
    /// native list right under it (170 px, 27 px rows, the highlighted row
    /// blue with white text), drawn by `os_menu` in this window.
    fn select(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let current = self.edited.preset;
        let bounds = self.field_bounds.clone();
        div()
            .id("mouse-view-settings")
            .relative()
            .w(px(SELECT_W))
            .h(px(34.))
            .pl(px(10.))
            .pr(px(15.))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(5.25))
            .border_1()
            .border_color(if self.open.is_some() { rgb(0x1651b0) } else { rgb(0xaaaaaa) })
            .bg(rgb(0xffffff))
            .cursor_pointer()
            .text_size(px(14.))
            .text_color(pal.text)
            .child(div().truncate().child(current.label()))
            .child(div().text_size(px(8.)).text_color(rgb(0x555555)).child("▼"))
            .child(canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if this.open.take().is_some() || this.closed_at.is_some_and(|t| t.elapsed().as_millis() < 300) {
                    cx.notify();
                    return;
                }
                let view = cx.entity();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    m = m.native_select(SELECT_W);
                    for preset in Preset::ALL {
                        let v = view.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(preset.label()).checked(preset == current).on_click(move |_, _, cx| v.update(cx, |this, cx| this.pick(preset, cx))));
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

    fn table(&self) -> AnyElement {
        let pal = p();
        let alt = "Alt + ";
        // Constrained rotate carries "Alt + " unless it is the default (then Rotate does).
        let (rotate_prefix, constrained_prefix) = if self.edited.constrained_rotate { (alt, "") } else { ("", alt) };
        let header = |text: &'static str| {
            div()
                .w(px(COL_W))
                .h(px(HEADER_H))
                .flex()
                .justify_center()
                .border_b_1()
                .border_color(rgb(HEADER_RULE))
                .text_size(px(16.))
                .font_weight(FontWeight::BOLD)
                .line_height(px(16.))
                .child(text)
        };
        let cell = |prefix: &'static str, text: &'static str| {
            let lines: Vec<String> = text.split('\n').map(|l| l.trim_end().to_string()).collect();
            let n = lines.len();
            v_flex().w(px(COL_W)).h_full().pl(px(10.)).items_center().justify_center().text_size(px(14.)).line_height(px(21.)).children(lines.into_iter().enumerate().map(move |(i, l)| {
                div().child(if i == 0 && n > 0 { format!("{prefix}{l}") } else { l })
            }))
        };
        let rows = self.edited.preset.table().into_iter().enumerate().map(|(i, (name, d3, d2))| {
            let prefix = match i {
                0 => rotate_prefix,
                1 => constrained_prefix,
                _ => "",
            };
            h_flex()
                .h(px(if name == "Pan" { PAN_ROW_H } else { ROW_H }))
                .child(div().w(px(COL_W)).h_full().pl(px(20.)).pt(px(23.)).text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).line_height(px(24.)).child(name))
                .child(cell(prefix, d3))
                // The 2D column never shows the Alt prefix ("Not applicable").
                .child(cell("", d2))
        });
        v_flex()
            .w(px(TABLE_W))
            .text_color(pal.text)
            .child(h_flex().child(div().w(px(COL_W))).child(header("3D Part & Assembly")).child(header("2D Drawing")))
            .children(rows)
            .into_any_element()
    }
}

impl Default for MousePrefsView {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for MousePrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let changed = self.edited != self.saved;
        let e = self.edited;
        let select = self.select(cx);
        let toggle = |f: fn(&mut MouseControls)| {
            cx.listener(move |this: &mut Self, _: &ClickEvent, _, cx| {
                f(&mut this.edited);
                cx.notify();
            })
        };
        v_flex()
            .mb(px(38.))
            .child(
                h_flex()
                    .h(px(32.))
                    .pl(px(15.))
                    .border_b_1()
                    .border_color(pal.text)
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Mouse controls")),
            )
            .child(
                v_flex()
                    .pl(px(15.))
                    .pt(px(15.))
                    .child(div().h(px(21.)).text_size(px(14.)).line_height(px(21.)).text_color(pal.text).child("View settings"))
                    .child(div().mt(px(10.)).child(select))
                    .child(div().mt(px(25.)).child(self.table()))
                    .child(
                        v_flex()
                            .mt(px(20.))
                            .child(Self::checkbox("mouse-reverse-scroll", "Reverse scroll wheel zoom direction", e.reverse_scroll).on_click(toggle(|c| c.reverse_scroll = !c.reverse_scroll)))
                            .child(Self::checkbox("mouse-constrained", "Set default rotation behavior to constrained rotate", e.constrained_rotate).on_click(toggle(|c| c.constrained_rotate = !c.constrained_rotate)))
                            .child(Self::checkbox("mouse-pen", "Use pen input as mouse", e.pen_as_mouse).on_click(toggle(|c| c.pen_as_mouse = !c.pen_as_mouse))),
                    )
                    .child(
                        h_flex().mt(px(24.)).child(
                            div()
                                .id("mouse-save")
                                .h(px(34.))
                                .px(px(10.5))
                                .flex()
                                .items_center()
                                .rounded(px(5.25))
                                .bg(rgb(0x1651b0))
                                .text_size(px(14.))
                                .text_color(rgb(0xffffff))
                                .when(!changed, |d| d.opacity(0.4))
                                .when(changed, |d| d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))))
                                .child("Save mouse controls"),
                        ),
                    ),
            )
    }
}
