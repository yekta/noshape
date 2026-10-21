//! Drawing editor: the Note panel's "Note color" panel (live drawing/1137,
//! behaviours.md "Note panel popups and values" (4)): 220 px under the
//! colour button, ✓ (enabled once the colour changed) ✕; tabs Palette |
//! Mixer; a row of 11 standard colours (3 + 8) over a 3 + 8 by 7 grid of
//! greys and tints / shades; the colour bar; # hex, R, G, B; Custom colors
//! with ⊞ (kept with the drawing); help.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::note::NoteStyle;

/// The standard row (live 1137, left to right).
const STANDARD: [u32; 11] = [0x000000, 0x0000ff, 0xff0000, 0x9dcfed, 0xa5a5a5, 0x3b61b4, 0xeaeaea, 0xc4e2f3, 0xf88701, 0x7f7f7f, 0xfab601];

/// The grid (live 1137): 7 rows of 3 greys and 8 hues, light to dark.
const GRID: [[u32; 11]; 7] = [
    [0xe6e6e6, 0xe6e0cf, 0xcfdbe5, 0xe3b2a3, 0xebcca7, 0xf0dfad, 0xcbe7be, 0xbcefea, 0xc2d6ec, 0xccbfe9, 0xd9c1dd],
    [0xcccccc, 0xccc6b6, 0xb6c2cc, 0xe1927a, 0xeab97d, 0xf5d578, 0xa7d293, 0x89dad3, 0x8aaed6, 0xa795d2, 0xc38bcd],
    [0xb3b3b3, 0xb3ae9f, 0x9faab3, 0xdf7251, 0xe9a554, 0xfacb43, 0x83bc68, 0x57c6bd, 0x5387c0, 0x836abb, 0xae55bc],
    [0x999999, 0x999487, 0x879199, 0xdd5228, 0xe8922a, 0xffc10e, 0x5fa73d, 0x24b1a6, 0x1651b0, 0x5e40a4, 0x981fac],
    [0x808080, 0x807c70, 0x707980, 0xba401b, 0xc1761c, 0xd29f09, 0x488c29, 0x18948b, 0x165294, 0x4b2e8f, 0x7d168e],
    [0x666666, 0x666359, 0x596066, 0x982d0d, 0x995a0e, 0xa67c05, 0x327014, 0x0c786f, 0x10447d, 0x371b7a, 0x610c6f],
    [0x4d4d4d, 0x4d4a43, 0x43484d, 0x751b00, 0x723e00, 0x795a00, 0x1b5500, 0x005b54, 0x0b3767, 0x240965, 0x460351],
];

fn split(c: u32) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

fn join(c: [u8; 3]) -> u32 {
    ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32
}

/// The open Note color panel.
pub struct NoteColorDlg {
    pub color: [u8; 3],
    orig: [u8; 3],
    pub mixer: bool,
    pub mix: crate::color_mixer::MixerState,
    hex: Entity<InputState>,
    rgb: [Entity<InputState>; 3],
    _subs: Vec<Subscription>,
}

impl DrawingEditor {
    /// The note (or note-like cell) style the panel edits.
    fn note_color_now(&self) -> Option<[u8; 3]> {
        let st: &NoteStyle = match &self.panel {
            Some(Panel::Note { style, .. }) => style,
            Some(Panel::Cell { note: Some(n), .. }) => &n.style,
            _ => return None,
        };
        Some(st.color.unwrap_or([0, 0, 0]))
    }

    /// Open the Note color panel on the note's colour.
    pub fn open_note_color(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.note_color_now() else { return };
        let hex = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(format!("{:06X}", join(c))));
        let rgb = [0, 1, 2].map(|i| cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(c[i].to_string())));
        let mut subs = vec![cx.subscribe_in(&hex, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur)
                && let Some(c) = crate::drawing_color::parse_hex(&st.read(cx).value())
            {
                this.pick_note_color(c, window, cx);
            }
        })];
        for (i, st) in rgb.iter().enumerate() {
            subs.push(cx.subscribe_in(st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur)
                    && let Ok(v) = st.read(cx).value().trim().parse::<u8>()
                    && let Some(mut c) = this.note_color.as_ref().map(|d| d.color)
                {
                    c[i] = v;
                    this.pick_note_color(c, window, cx);
                }
            }));
        }
        self.note_color = Some(NoteColorDlg { color: c, orig: c, mixer: false, mix: crate::color_mixer::MixerState::from_rgb(c), hex, rgb, _subs: subs });
        cx.notify();
    }

    fn pick_note_color(&mut self, c: [u8; 3], window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.note_color {
            d.color = c;
            d.hex.update(cx, |s, cx| s.set_value(format!("{:06X}", join(c)), window, cx));
            for (i, st) in d.rgb.iter().enumerate() {
                st.update(cx, |s, cx| s.set_value(c[i].to_string(), window, cx));
            }
        }
        cx.notify();
    }

    fn close_note_color(&mut self, ok: bool, cx: &mut Context<Self>) {
        let Some(d) = self.note_color.take() else { return };
        if ok {
            // Black is the ink (no colour override).
            let v = (d.color != [0, 0, 0]).then_some(d.color);
            match &mut self.panel {
                Some(Panel::Note { style, .. }) => style.color = v,
                Some(Panel::Cell { note: Some(n), .. }) => n.style.color = v,
                _ => {}
            }
        }
        cx.notify();
    }

    /// The panel, positioned under the Note panel's colour button.
    pub fn render_note_color(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.note_color_now().is_none() {
            self.note_color = None;
            return None;
        }
        let d = self.note_color.as_ref()?;
        let pal = p();
        let (color, mixer, mix, changed) = (d.color, d.mixer, d.mix, d.color != d.orig);
        let swatch = |id: SharedString, c: [u8; 3], cx: &mut Context<Self>| {
            let dark = (c[0] as u32 + c[1] as u32 + c[2] as u32) < 384;
            div()
                .id(id)
                .size(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui_kit::rgb(join(c)))
                .cursor_pointer()
                .when(c == color, |d| d.child(crate::ui::glyph("check", 12., if dark { gpui_kit::white() } else { Hsla::from(gpui_kit::rgb(0x000000)) })))
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.pick_note_color(c, w, cx)))
        };
        let group = |cells: &[u32], key: &str, cx: &mut Context<Self>| {
            let mut a = h_flex().gap(px(2.));
            let mut b = h_flex().gap(px(2.));
            for (i, c) in cells.iter().enumerate() {
                let s = swatch(SharedString::from(format!("nc-{key}-{i}")), split(*c), cx);
                if i < 3 {
                    a = a.child(s);
                } else {
                    b = b.child(s);
                }
            }
            h_flex().gap(px(13.)).child(a).child(b)
        };
        let tab = |id: &'static str, label: &'static str, on: bool, m: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .when(on, |d| d.border_b_2().border_color(gpui_kit::rgb(0x1651b0)).text_color(gpui_kit::rgb(0x1651b0)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.note_color {
                        d.mixer = m;
                        if m {
                            let used = d.mix.hue_used;
                            d.mix = crate::color_mixer::MixerState::from_rgb(d.color);
                            d.mix.hue_used = used;
                        }
                    }
                    cx.notify();
                }))
        };
        let mut body = v_flex().px(px(6.)).pt(px(8.)).gap(px(2.));
        if mixer {
            body = body.child(crate::color_mixer::mixer(&mix, |v: &mut Self| v.note_color.as_mut().map(|d| &mut d.mix), |v: &mut Self, c, window, cx| v.pick_note_color(c, window, cx), cx));
        } else {
            body = body.child(div().mb(px(8.)).child(group(&STANDARD, "std", cx)));
            for (r, row) in GRID.iter().enumerate() {
                body = body.child(group(row, &format!("g{r}"), cx));
            }
        }
        let d = self.note_color.as_ref()?;
        let field = |label: &'static str, input: &Entity<InputState>, w: f32| {
            h_flex()
                .gap(px(2.))
                .child(div().text_size(px(12.)).text_color(gpui_kit::rgb(0x777777)).child(label))
                .child(div().w(px(w)).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).child(Input::new(input).appearance(false).xsmall().text_size(px(13.33)).text_color(gpui_kit::rgb(0x000000))))
        };
        body = body
            .child(div().mt(px(6.)).h(px(20.)).w_full().bg(gpui_kit::rgb(join(color))).border_1().border_color(gpui_kit::rgb(0xcccccc)))
            .child(h_flex().mt(px(4.)).gap(px(8.)).child(field("#", &d.hex, 56.)).child(field("R", &d.rgb[0], 28.)).child(field("G", &d.rgb[1], 28.)).child(field("B", &d.rgb[2], 28.)));
        // Custom colors: ⊞ saves the current colour with the drawing.
        let customs = self.drawing.props.custom_colors.clone();
        let mut custom_row = h_flex().gap(px(2.)).mt(px(4.));
        for (i, c) in customs.into_iter().enumerate() {
            custom_row = custom_row.child(swatch(SharedString::from(format!("nc-custom-{i}")), c, cx));
        }
        custom_row = custom_row.child(div().id("nc-add").size(px(16.)).flex().items_center().justify_center().border_1().border_color(gpui_kit::rgb(0x999999)).text_size(px(13.)).text_color(gpui_kit::rgb(0x777777)).cursor_pointer().child("+").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.edit("Save custom color", cx, |dr| {
                if !dr.props.custom_colors.contains(&color) {
                    dr.props.custom_colors.push(color);
                }
            });
        })));
        body = body.child(div().mt(px(8.)).text_size(px(13.)).text_color(gpui_kit::rgb(0x777777)).child("Custom colors")).child(custom_row);
        body = body.child(h_flex().justify_end().pb(px(4.)).child(div().size(px(16.)).rounded_full().bg(gpui_kit::rgb(0x999999)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::white()).child("?")));
        Some(
            v_flex()
                .id("dr-note-color")
                .absolute()
                // Under the colour button (B I U S: 4 × 32 px in).
                .left(px(132.))
                .top(px(97.))
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(gpui_kit::rgb(0x333333))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(7.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Note color"))
                        .child(
                            div()
                                .id("nc-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(if changed { pal.accept } else { gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED) })
                                .when(changed, |e| e.cursor_pointer())
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if changed {
                                        this.close_note_color(true, cx)
                                    }
                                })),
                        )
                        .child(div().id("nc-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_note_color(false, cx)))),
                )
                .child(h_flex().border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).child(tab("nc-tab-pal", "Palette", !mixer, false, cx)).child(tab("nc-tab-mix", "Mixer", mixer, true, cx)))
                .child(body)
                .into_any_element(),
        )
    }
}
