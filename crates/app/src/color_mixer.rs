//! The colour pickers' "Mixer" tab, the same in the Appearance editor and
//! the drawing colour dialogs (live behaviours.md "Colour Mixer",
//! cursors/710–713, dom/mixer-*): a 152 px saturation / value square
//! (white → hue across, → black down; `crosshair`; a 5 px round marker,
//! white with a 2 px `#00008b` ring at 50 %) and, 6 px to its right, a
//! 50 x 152 hue strip (red at the top; `ns-resize`; a 50 x 10 white bar
//! with a 4 px `#add8e6` border at 70 %, shown once used). A press jumps
//! there, a drag follows.

use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;

/// The mixer's colour (hue 0–360, saturation and value 0–1) and drag.
#[derive(Clone, Copy, Default)]
pub(crate) struct MixerState {
    pub h: f32,
    pub s: f32,
    pub v: f32,
    /// The hue bar shows once the strip was used (live).
    pub hue_used: bool,
    drag: Option<Part>,
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Square,
    Strip,
}

impl MixerState {
    pub fn from_rgb(c: [u8; 3]) -> Self {
        let (h, s, v) = rgb_to_hsv(c);
        Self { h, s, v, hue_used: false, drag: None }
    }

    pub fn rgb(&self) -> [u8; 3] {
        hsv_to_rgb(self.h, self.s, self.v)
    }
}

pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = (h.rem_euclid(360.)) / 60.;
    let c = v * s;
    let x = c * (1. - (h % 2. - 1.).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = v - c;
    [((r + m) * 255.).round() as u8, ((g + m) * 255.).round() as u8, ((b + m) * 255.).round() as u8]
}

pub(crate) fn rgb_to_hsv(c: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = c.map(|x| x as f32 / 255.);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0. {
        0.
    } else if max == r {
        60. * ((g - b) / d).rem_euclid(6.)
    } else if max == g {
        60. * ((b - r) / d + 2.)
    } else {
        60. * ((r - g) / d + 4.)
    };
    (h, if max == 0. { 0. } else { d / max }, max)
}

/// How a view reaches its mixer.
pub(crate) type Field<V> = fn(&mut V) -> Option<&mut MixerState>;
/// The colour picked (on every press and drag step).
pub(crate) type OnColor<V> = fn(&mut V, [u8; 3], &mut Window, &mut Context<V>);

fn rgba(c: [u8; 3]) -> Rgba {
    gpui_kit::rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32)
}

/// The square and the strip.
pub(crate) fn mixer<V: 'static>(st: &MixerState, field: Field<V>, on_color: OnColor<V>, cx: &mut Context<V>) -> AnyElement {
    let hue = hsv_to_rgb(st.h, 1., 1.);
    let sq: Rc<Cell<Bounds<Pixels>>> = Default::default();
    let strip: Rc<Cell<Bounds<Pixels>>> = Default::default();
    // Where a pointer position puts the colour.
    let apply = move |v: &mut V, part: Part, at: Point<Pixels>, b: Bounds<Pixels>, window: &mut Window, cx: &mut Context<V>| {
        let fx = (f32::from(at.x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
        let fy = (f32::from(at.y - b.top()) / f32::from(b.size.height).max(1.)).clamp(0., 1.);
        let Some(m) = field(v) else { return };
        match part {
            Part::Square => {
                m.s = fx;
                m.v = 1. - fy;
            }
            Part::Strip => {
                m.h = fy * 360.;
                m.hue_used = true;
            }
        }
        let c = m.rgb();
        on_color(v, c, window, cx);
        cx.notify();
    };
    let area = move |part: Part, cell: Rc<Cell<Bounds<Pixels>>>, cx: &mut Context<V>| {
        let (c1, c2, c3) = (cell.clone(), cell.clone(), cell);
        div()
            .id(match part {
                Part::Square => "mixer-square",
                Part::Strip => "mixer-strip",
            })
            .relative()
            .flex_none()
            .h(px(152.))
            .cursor(if part == Part::Square { CursorStyle::Crosshair } else { CursorStyle::ResizeUpDown })
            .child(canvas(move |b, _, _| c1.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_mouse_down(MouseButton::Left, cx.listener(move |v, e: &MouseDownEvent, window, cx| {
                if let Some(m) = field(v) {
                    m.drag = Some(part);
                }
                apply(v, part, e.position, c2.get(), window, cx);
            }))
            // Moves come wherever the pointer is: only a press that started
            // here drags (the press is remembered in the view's state).
            .on_mouse_move(cx.listener(move |v, e: &MouseMoveEvent, window, cx| {
                let Some(m) = field(v) else { return };
                if e.pressed_button != Some(MouseButton::Left) {
                    m.drag = None;
                } else if m.drag == Some(part) {
                    apply(v, part, e.position, c3.get(), window, cx);
                }
            }))
    };
    // The square: white → hue across, transparent → black down.
    let square = area(Part::Square, sq, cx)
        .w(px(152.))
        .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(90., linear_color_stop(rgba([255, 255, 255]), 0.), linear_color_stop(rgba(hue), 1.))))
        .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(180., linear_color_stop(hsla(0., 0., 0., 0.), 0.), linear_color_stop(hsla(0., 0., 0., 1.), 1.))))
        .child(
            div()
                .absolute()
                .left(px(st.s * 152. - 2.5))
                .top(px((1. - st.v) * 152. - 2.5))
                .size(px(5.))
                .rounded(px(4.))
                .bg(gpui_kit::rgb(0xffffff))
                .border_2()
                .border_color(gpui_kit::rgb(0x00008b))
                .opacity(0.5),
        );
    // The strip: red, yellow, green, cyan, blue, magenta, red from the top.
    let stops = [0., 60., 120., 180., 240., 300., 360.];
    let mut bands = div().absolute().top_0().left_0().size_full().flex().flex_col();
    for w in stops.windows(2) {
        bands = bands.child(div().flex_1().bg(linear_gradient(180., linear_color_stop(rgba(hsv_to_rgb(w[0], 1., 1.)), 0.), linear_color_stop(rgba(hsv_to_rgb(w[1], 1., 1.)), 1.))));
    }
    let strip = area(Part::Strip, strip, cx).w(px(50.)).child(bands).when(st.hue_used, |d| {
        d.child(div().absolute().left_0().top(px(st.h / 360. * 152. - 5.)).w(px(50.)).h(px(10.)).rounded(px(4.)).bg(gpui_kit::rgb(0xffffff)).border_4().border_color(gpui_kit::rgb(0xadd8e6)).opacity(0.7))
    });
    h_flex().gap(px(6.)).child(square).child(strip).into_any_element()
}
