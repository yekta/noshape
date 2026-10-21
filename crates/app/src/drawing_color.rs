//! Drawing editor: Onshape's color dialog, opened from the color swatches
//! of Drawing properties. "Palette": pick a swatch, or enter a hex code or
//! RGB values; "Mixer": the saturation / value square and the hue strip (`color_mixer`); "+"
//! under Custom colors saves the current color (kept with the drawing).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::DrawingProps;

/// What the dialog sets.
pub type ColorSet = std::rc::Rc<dyn Fn(&mut DrawingProps, Option<[u8; 3]>)>;

/// Color dialog state.
pub struct ColorDlg {
    pub color: [u8; 3],
    pub mixer: bool,
    /// The Mixer tab (`color_mixer`).
    pub mix: crate::color_mixer::MixerState,
    hex: Entity<InputState>,
    rgb: [Entity<InputState>; 3],
    set: ColorSet,
    _subs: Vec<Subscription>,
}

/// The palette: 8 hues x 6 shades, plus grays.
fn palette() -> Vec<[u8; 3]> {
    let mut out = Vec::new();
    for v in [0.35, 0.55, 0.75, 0.9, 1.0] {
        for h in [0.0, 30.0, 55.0, 120.0, 180.0, 210.0, 270.0, 320.0] {
            out.push(hsv(h, if v > 0.95 { 0.45 } else { 0.85 }, v));
        }
    }
    for g in [0u8, 64, 128, 170, 212, 255, 96, 32] {
        out.push([g, g, g]);
    }
    out
}

/// HSV (h in degrees, s / v 0–1) to RGB.
pub fn hsv(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [((r + m) * 255.0).round() as u8, ((g + m) * 255.0).round() as u8, ((b + m) * 255.0).round() as u8]
}

/// "#RRGGBB" / "RRGGBB" to RGB.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn rgba(c: [u8; 3]) -> Hsla {
    gpui_kit::rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32).into()
}

impl DrawingEditor {
    /// Open the color dialog at `cur` (None: black ink); OK calls `set`.
    pub fn open_color_dialog(&mut self, cur: Option<[u8; 3]>, set: ColorSet, window: &mut Window, cx: &mut Context<Self>) {
        let c = cur.unwrap_or([0, 0, 0]);
        let hex_in = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(hex(c)));
        let rgb_in = [0, 1, 2].map(|i| cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(c[i].to_string())));
        let mut subs = vec![cx.subscribe(&hex_in, |this: &mut Self, st, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur)
                && let Some(c) = parse_hex(&st.read(cx).value())
                && let Some(d) = &mut this.color_dlg
            {
                d.color = c;
                cx.notify();
            }
        })];
        for (i, st) in rgb_in.iter().enumerate() {
            subs.push(cx.subscribe(st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur)
                    && let Ok(v) = st.read(cx).value().trim().parse::<u8>()
                    && let Some(d) = &mut this.color_dlg
                {
                    d.color[i] = v;
                    cx.notify();
                }
            }));
        }
        self.color_dlg = Some(ColorDlg { color: c, mixer: false, mix: crate::color_mixer::MixerState::from_rgb(c), hex: hex_in, rgb: rgb_in, set, _subs: subs });
        cx.notify();
    }

    fn pick_color(&mut self, c: [u8; 3], window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.color_dlg {
            d.color = c;
            d.hex.update(cx, |s, cx| s.set_value(hex(c), window, cx));
            for (i, st) in d.rgb.iter().enumerate() {
                st.update(cx, |s, cx| s.set_value(c[i].to_string(), window, cx));
            }
        }
        cx.notify();
    }

    fn close_color_dialog(&mut self, ok: bool, cx: &mut Context<Self>) {
        let Some(d) = self.color_dlg.take() else { return };
        if ok {
            let (set, c) = (d.set, d.color);
            // Black is the ink (no color override).
            let v = if c == [0, 0, 0] { None } else { Some(c) };
            self.edit("Color", cx, |dr| set(&mut dr.props, v));
        }
        cx.notify();
    }

    pub fn render_color_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.color_dlg.as_ref()?;
        let pal = p();
        let (color, mixer, mix) = (d.color, d.mixer, d.mix);
        let swatch = |id: SharedString, c: [u8; 3], size: f32, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(size))
                .bg(rgba(c))
                .border_1()
                .border_color(if c == color { pal.accent } else { gpui_kit::rgb(0xcccccc) })
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.pick_color(c, w, cx)))
        };
        let tab = |id: &'static str, label: &'static str, on: bool, m: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .px(px(10.))
                .h(px(26.))
                .flex()
                .items_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.border_b_2().border_color(pal.accent).text_color(pal.accent))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.color_dlg {
                        d.mixer = m;
                        // The mixer opens on the current colour.
                        if m {
                            let used = d.mix.hue_used;
                            d.mix = crate::color_mixer::MixerState::from_rgb(d.color);
                            d.mix.hue_used = used;
                        }
                    }
                    cx.notify();
                }))
        };
        let mut body = v_flex().p(px(10.)).gap(px(8.));
        if mixer {
            // Live 712: the saturation / value square and the hue strip.
            body = body.child(crate::color_mixer::mixer(&mix, |v: &mut Self| v.color_dlg.as_mut().map(|d| &mut d.mix), |v: &mut Self, c, window, cx| v.pick_color(c, window, cx), cx));
        } else {
            let mut grid = v_flex().gap(px(2.));
            for (r, chunk) in palette().chunks(8).enumerate() {
                let mut row = h_flex().gap(px(2.));
                for (i, c) in chunk.iter().enumerate() {
                    row = row.child(swatch(SharedString::from(format!("cd-pal-{r}-{i}")), *c, 22., cx));
                }
                grid = grid.child(row);
            }
            body = body.child(grid);
        }
        let d = self.color_dlg.as_ref()?;
        body = body
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().size(px(28.)).bg(rgba(color)).border_1().border_color(gpui_kit::rgb(0x999999)))
                    .child(div().text_size(px(12.)).child("Hex"))
                    .child(div().w(px(80.)).child(Input::new(&d.hex).xsmall().text_size(px(13.)).text_color(gpui_kit::rgb(0x000000)).pl(px(2.))))
                    .child(div().text_size(px(12.)).child("R"))
                    .child(div().w(px(40.)).child(Input::new(&d.rgb[0]).xsmall()))
                    .child(div().text_size(px(12.)).child("G"))
                    .child(div().w(px(40.)).child(Input::new(&d.rgb[1]).xsmall()))
                    .child(div().text_size(px(12.)).child("B"))
                    .child(div().w(px(40.)).child(Input::new(&d.rgb[2]).xsmall())),
            );
        // Custom colors, "+" saves the current one.
        let customs = self.drawing.props.custom_colors.clone();
        let mut custom_row = h_flex().gap(px(2.)).child(div().text_size(px(12.)).mr(px(4.)).child("Custom colors"));
        for (i, c) in customs.into_iter().enumerate() {
            custom_row = custom_row.child(swatch(SharedString::from(format!("cd-custom-{i}")), c, 18., cx));
        }
        custom_row = custom_row.child(div().id("cd-add").size(px(18.)).flex().items_center().justify_center().border_1().border_color(gpui_kit::rgb(0xbbbbbb)).cursor_pointer().child("+").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.edit("Save custom color", cx, |dr| {
                if !dr.props.custom_colors.contains(&color) {
                    dr.props.custom_colors.push(color);
                }
            });
        })));
        body = body.child(custom_row);
        Some(
            div()
                .id("dr-color-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(pal.modal_backdrop)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .w(px(330.))
                        .bg(pal.modal_bg)
                        .rounded(px(4.))
                        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.25), offset: point(px(0.), px(6.)), blur_radius: px(18.), spread_radius: px(0.), inset: false }])
                        .text_color(pal.text)
                        .child(h_flex().h(px(36.)).px(px(10.)).border_b_1().border_color(pal.border).child(div().flex_1().text_size(px(15.)).child("Color")))
                        .child(h_flex().px(px(6.)).border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).child(tab("cd-tab-pal", "Palette", !mixer, false, cx)).child(tab("cd-tab-mix", "Mixer", mixer, true, cx)))
                        .child(body)
                        .child(
                            h_flex()
                                .gap(px(6.))
                                .p(px(10.))
                                .justify_end()
                                .child(Button::new("cd-cancel").label("Cancel").ghost().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_color_dialog(false, cx))))
                                .child(Button::new("cd-ok").label("OK").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_color_dialog(true, cx)))),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{hsv, parse_hex};

    #[test]
    fn hex_and_hsv() {
        assert_eq!(parse_hex("#FF8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("00ff00"), Some([0, 255, 0]));
        assert_eq!(parse_hex("zz"), None);
        assert_eq!(hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(hsv(120.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(hsv(240.0, 0.0, 0.5), [128, 128, 128]);
    }
}
