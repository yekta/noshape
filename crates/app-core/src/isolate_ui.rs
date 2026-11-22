//! Onshape's Isolate / Make transparent dialog, the same in the Part Studio
//! and the assembly (live assembly/61, cursors/641-_Isolate,
//! 641-_Make_transparent, dom/dialog-assembly-isolate.txt): 220 px, the
//! title (it moves the dialog) and ✕; "Transparency controls" (Isolate's ☐ Show
//! internal geometry, ☐ Select transparent geometry, the transparency slider);
//! "Component controls" (the Expand select, the expand slider, ?).

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::rc::Rc;

/// Make transparent's slider range: the opacity of the parts left out.
pub const ALPHA_MIN: f32 = 0.12;
pub const ALPHA_MAX: f32 = 0.9;

/// Isolate's "Adjust opacity" knob in a new account (live asm/isolate-1122: 30 % with the box ticked or not).
pub const KNOB_START: f32 = 0.3;

/// The left-out parts' opacity at Isolate's knob `t` (live asm/isolate-1122): none at the far left (not drawn), up to
/// [`ALPHA_MAX`] at the right; with "Show internal geometry" ticked they're lighter (on-30 / on-100 against off-30 /
/// off-100: 0.55 of the cover over white).
pub fn isolate_alpha(t: f32, internal: bool) -> f32 {
    ALPHA_MAX * t.clamp(0.0, 1.0) * if internal { 0.55 } else { 1.0 }
}

/// Isolate's knob, one position per "Show internal geometry" state, the same in both studios (live: a drag in the
/// assembly shows next time in the Part Studio) and kept across sessions.
#[derive(Default)]
struct KnobMemory([Option<f32>; 2]);
impl Global for KnobMemory {}

fn knob_key(internal: bool) -> &'static str {
    if internal { "view:isolateOpacityInternal" } else { "view:isolateOpacity" }
}

/// Where Isolate's knob sits for the box `internal`.
pub fn remembered_knob(internal: bool, cx: &App) -> f32 {
    if let Some(t) = cx.try_global::<KnobMemory>().and_then(|m| m.0[internal as usize]) {
        return t;
    }
    match crate::feature_settings::get(knob_key(internal)).get("").and_then(|g| g.get("knob")) {
        Some(noshape_model::params::ParamValue::Text(t)) => t.parse::<f32>().ok().filter(|f| (0.0..=1.0).contains(f)).unwrap_or(KNOB_START),
        _ => KNOB_START,
    }
}

/// Keeps Isolate's knob for the box `internal`; `save` also writes it to disk (on a toggle or a close, not per drag
/// step).
pub fn remember_knob(internal: bool, t: f32, save: bool, cx: &mut App) {
    cx.default_global::<KnobMemory>().0[internal as usize] = Some(t);
    if save {
        let mut groups = crate::feature_settings::Groups::new();
        groups.entry(String::new()).or_default().insert("knob".to_string(), noshape_model::params::ParamValue::Text(t.to_string()));
        crate::feature_settings::put(knob_key(internal), &groups);
    }
}

/// The slider position of opacity `alpha`.
pub fn alpha_t(alpha: f32) -> f32 {
    ((alpha - ALPHA_MIN) / (ALPHA_MAX - ALPHA_MIN)).clamp(0.0, 1.0)
}

/// The opacity at slider position `t`.
pub fn t_alpha(t: f32) -> f32 {
    ALPHA_MIN + t * (ALPHA_MAX - ALPHA_MIN)
}

/// Where a view keeps the slider being dragged.
pub type SliderDrag<V> = fn(&mut V) -> &mut Option<&'static str>;
/// A view action.
pub type Act<V> = fn(&mut V, &mut Window, &mut Context<V>);
/// A slider's new position, 0 to 1.
pub type SetT<V> = Rc<dyn Fn(&mut V, f32, &mut Window, &mut Context<V>)>;
/// A pick in the Expand select, by index.
pub type PickIx = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// What the dialog shows and does.
pub struct IsoDialog<V> {
    /// `PanelOffsets` key of the dialog (shared by both studios).
    pub key: &'static str,
    pub title: &'static str,
    /// Isolate's ☐ Show internal geometry (live asm/isolate-1122); `None` in Make transparent, whose slider keeps
    /// the legend fill and the old tooltip.
    pub show_internal: Option<bool>,
    pub select_transparent: bool,
    /// Slider positions, 0 to 1.
    pub transparency: f32,
    pub expand: f32,
    pub expand_options: Vec<String>,
    /// Options shown greyed (live Part Studio: connectivity, no mates).
    pub expand_disabled: Vec<usize>,
    pub expand_ix: usize,
    pub expand_tip: &'static str,
    pub drag: SliderDrag<V>,
    pub close: Act<V>,
    pub toggle_internal: Act<V>,
    pub toggle_select: Act<V>,
    pub set_transparency: SetT<V>,
    pub set_expand: SetT<V>,
    pub pick_expand: PickIx,
}

/// The dialog (placed by the caller).
pub fn dialog<V: 'static>(d: IsoDialog<V>, cx: &mut Context<V>) -> AnyElement {
    let pal = crate::theme::p();
    let section = |t: &'static str| div().pl(px(3.)).h(px(15.)).mt(px(10.)).mb(px(10.)).text_size(px(14.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x666666)).child(t);
    let (close, toggle, toggle_internal) = (d.close, d.toggle_select, d.toggle_internal);
    // Isolate (live asm/isolate-1122 ps-panel): 28 px checkbox rows, the knob on a plain line 7 px under them, "Adjust
    // opacity"; Make transparent: the legend track (live ui/iso-907, a `#777` stub then blue to white).
    let isolate = d.show_internal.is_some();
    let (tip, track, slider_mt) = if isolate { ("Adjust opacity", Track::Line, 7.) } else { ("Show/hide internal geometry", Track::Legend, 6.) };
    let row_h = if isolate { 28. } else { 27. };
    let pick = d.pick_expand.clone();
    v_flex()
        .id(SharedString::from(format!("{}-dialog", d.key)))
        .debug_selector(move || format!("{}-dialog", d.key))
        .occlude()
        .child(crate::ui::panel_probe(d.key))
        .w(px(220.))
        .bg(crate::theme::with_alpha(rgb(0xffffff), 0.95))
        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
        .text_color(pal.text)
        .child(
            h_flex()
                .h(px(29.))
                .border_b_1()
                .border_color(rgb(0xd7d7d7))
                .child(crate::ui::panel_title(d.key, div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(d.title)))
                .child(
                    div()
                        .id(SharedString::from(format!("{}-close", d.key)))
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        // Live (ns-dialog close, as dom dialog-animate): an 11 px × at .5.
                        .opacity(0.86)
                        .child(crate::ui::glyph("close", 21., gpui_kit::rgb(0x333333)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| close(this, window, cx))),
                ),
        )
        .child(section("Transparency controls"))
        .when_some(d.show_internal, |el, on| {
            el.child(
                crate::os_ui::checkbox(SharedString::from(format!("{}-show-internal", d.key)), "Show internal geometry", on)
                    .h(px(row_h))
                    .tooltip(crate::ui::tooltip("Show/hide internal geometry"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| toggle_internal(this, window, cx))),
            )
        })
        .child(
            crate::os_ui::checkbox(SharedString::from(format!("{}-select-transparent", d.key)), "Select transparent geometry", d.select_transparent)
                .h(px(row_h))
                .tooltip(crate::ui::tooltip("Enable selection of parts, faces, and edges from transparent bodies"))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| toggle(this, window, cx))),
        )
        .child(div().mt(px(slider_mt)).child(slider(Slider { key: d.key, name: "transparency", t: d.transparency, tip, track }, d.drag, d.set_transparency, cx)))
        .child(section("Component controls"))
        .child(
            crate::os_ui::select_field_with(SharedString::from(format!("{}-type", d.key)), d.expand_options, d.expand_disabled, d.expand_ix, move |i, window, cx| pick(i, window, cx))
                .h(px(26.))
                .w(px(194.))
                .pl(px(6.))
                .rounded(px(0.))
                .border_0()
                .border_b_1()
                .border_color(rgb(0xd7d7d7))
                .text_size(px(13.33)),
        )
        .child(
            // Live (dom dialog-assembly-isolate): an 18 px row 5 px under the select, the knob 5 px into it; Make
            // transparent's bottom right under it (220 × 196 in all), Isolate's 5 px below (220 × 231, ps-panel).
            h_flex()
                .mt(px(5.))
                .when(isolate, |r| r.mb(px(5.)))
                .h(px(18.))
                .items_start()
                .child(div().flex_1().pt(px(3.)).child(slider(Slider { key: d.key, name: "expand", t: d.expand, tip: d.expand_tip, track: Track::Line }, d.drag, d.set_expand, cx)))
                .child(div().mt(px(5.)).mr(px(4.)).size(px(14.)).rounded_full().flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0xaaaaaa)))),
        )
        .into_any_element()
}

/// What a slider shows.
struct Slider {
    key: &'static str,
    name: &'static str,
    /// Knob position, 0 to 1.
    t: f32,
    tip: &'static str,
    track: Track,
}

/// A slider's track (live noUiSlider, 183 px, 11 px in).
enum Track {
    /// 7 px, radius 6, a 2 px inset `#777` ring; with the legend fill.
    Legend,
    /// A 2 px `#777` line.
    Line,
}

/// One slider: its track and an 11 px white knob (1 px `#888`, a 3 px dark
/// glow). Only a press that started on it drags it (a box select or a
/// dialog drag passing over must not), so the view keeps the dragged slider
/// (`drag`).
fn slider<V: 'static>(spec: Slider, drag: SliderDrag<V>, set: SetT<V>, cx: &mut Context<V>) -> AnyElement {
    let Slider { key, name, t, tip, track: track_kind } = spec;
    let track: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let (t1, t2, t3) = (track.clone(), track.clone(), track);
    let s2 = set.clone();
    let frac = |x: Pixels, b: Bounds<Pixels>| (f32::from(x - b.left()) / f32::from(b.size.width).max(1.0)).clamp(0.0, 1.0);
    let id = name;
    div()
        .id(SharedString::from(format!("{key}-{name}")))
        .relative()
        .h(px(15.))
        .ml(px(11.))
        .w(px(183.))
        .cursor_pointer()
        .tooltip(crate::ui::tooltip(tip))
        .child(match track_kind {
            Track::Legend => div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(4.))
                .h(px(7.))
                .rounded(px(6.))
                .overflow_hidden()
                .child(div().absolute().top_0().bottom_0().left_0().w(relative(0.1)).bg(rgb(0x777777)))
                .child(div().absolute().top_0().bottom_0().left(relative(0.1)).right_0().bg(linear_gradient(90., linear_color_stop(rgb(0x3e94ff), 0.), linear_color_stop(rgb(0xffffff), 1.))))
                .child(div().absolute().top_0().left_0().size_full().rounded(px(6.)).border_2().border_color(rgb(0x777777))),
            Track::Line => div().absolute().left_0().right_0().top(px(6.)).h(px(2.)).bg(rgb(0x777777)),
        })
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(relative(t))
                .ml(px(-6.))
                .size(px(11.))
                .rounded_full()
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0x888888))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.6), offset: point(px(0.), px(0.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }]),
        )
        .child(canvas(move |b, _, _| t1.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            *drag(this) = Some(id);
            set(this, frac(e.position.x, t2.get()), window, cx)
        }))
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
            if e.pressed_button != Some(MouseButton::Left) {
                *drag(this) = None;
            } else if *drag(this) == Some(id) {
                s2(this, frac(e.position.x, t3.get()), window, cx);
            }
        }))
        .into_any_element()
}
