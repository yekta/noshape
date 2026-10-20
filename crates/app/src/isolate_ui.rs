//! Onshape's Isolate / Make transparent dialog, the same in the Part Studio
//! and the assembly (live assembly/61, cursors/641-_Isolate,
//! 641-_Make_transparent, dom/dialog-assembly-isolate.txt): 220 px, the
//! title (it moves the dialog) and ✕; "Transparency controls" (☐ Select
//! transparent geometry, the transparency slider); "Component controls"
//! (the Expand select, the expand slider, ?).

use gpui_kit::component::{Icon, IconName, h_flex, v_flex};
use gpui_kit::*;
use std::rc::Rc;

/// The transparency slider's range: the opacity of the parts left out.
pub(crate) const ALPHA_MIN: f32 = 0.12;
pub(crate) const ALPHA_MAX: f32 = 0.9;

/// The slider position of opacity `alpha`.
pub(crate) fn alpha_t(alpha: f32) -> f32 {
    ((alpha - ALPHA_MIN) / (ALPHA_MAX - ALPHA_MIN)).clamp(0.0, 1.0)
}

/// The opacity at slider position `t`.
pub(crate) fn t_alpha(t: f32) -> f32 {
    ALPHA_MIN + t * (ALPHA_MAX - ALPHA_MIN)
}

/// Where a view keeps the slider being dragged.
pub(crate) type SliderDrag<V> = fn(&mut V) -> &mut Option<&'static str>;
/// A view action.
pub(crate) type Act<V> = fn(&mut V, &mut Window, &mut Context<V>);
/// A slider's new position, 0 to 1.
pub(crate) type SetT<V> = Rc<dyn Fn(&mut V, f32, &mut Window, &mut Context<V>)>;
/// A pick in the Expand select, by index.
pub(crate) type PickIx = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// What the dialog shows and does.
pub(crate) struct IsoDialog<V> {
    /// `PanelOffsets` key of the dialog (shared by both studios).
    pub key: &'static str,
    pub title: &'static str,
    pub select_transparent: bool,
    /// Slider positions, 0 to 1.
    pub transparency: f32,
    pub expand: f32,
    pub expand_options: Vec<String>,
    pub expand_ix: usize,
    pub expand_tip: &'static str,
    pub drag: SliderDrag<V>,
    pub close: Act<V>,
    pub toggle_select: Act<V>,
    pub set_transparency: SetT<V>,
    pub set_expand: SetT<V>,
    pub pick_expand: PickIx,
}

/// The dialog (placed by the caller).
pub(crate) fn dialog<V: 'static>(d: IsoDialog<V>, cx: &mut Context<V>) -> AnyElement {
    let pal = crate::theme::p();
    let section = |t: &'static str| div().pl(px(3.)).h(px(15.)).mt(px(10.)).mb(px(10.)).text_size(px(14.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x666666)).child(t);
    let (close, toggle) = (d.close, d.toggle_select);
    let pick = d.pick_expand.clone();
    v_flex()
        .id(SharedString::from(format!("{}-dialog", d.key)))
        .occlude()
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
                        .opacity(0.5)
                        .child(Icon::new(IconName::Close).size(px(14.)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| close(this, window, cx))),
                ),
        )
        .child(section("Transparency controls"))
        .child(
            crate::os_ui::checkbox(SharedString::from(format!("{}-select-transparent", d.key)), "Select transparent geometry", d.select_transparent)
                .tooltip(crate::ui::tooltip("Enable selection of parts, faces, and edges from transparent bodies"))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| toggle(this, window, cx))),
        )
        .child(div().mt(px(6.)).child(slider(d.key, "transparency", d.transparency, "Show/hide internal geometry", d.drag, d.set_transparency, cx)))
        .child(section("Component controls"))
        .child(
            crate::os_ui::select_field(SharedString::from(format!("{}-type", d.key)), d.expand_options, d.expand_ix, move |i, window, cx| pick(i, window, cx))
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
            h_flex()
                .mt(px(5.))
                .mb(px(5.))
                .child(div().flex_1().child(slider(d.key, "expand", d.expand, d.expand_tip, d.drag, d.set_expand, cx)))
                .child(div().mr(px(4.)).size(px(14.)).rounded_full().bg(rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(rgb(0xffffff)).child("?")),
        )
        .into_any_element()
}

/// One slider: a 7 px rounded `#777` track and an 11 px white knob. Only a
/// press that started on it drags it (a box select or a dialog drag passing
/// over must not), so the view keeps the dragged slider (`drag`).
fn slider<V: 'static>(key: &'static str, name: &'static str, t: f32, tip: &'static str, drag: SliderDrag<V>, set: SetT<V>, cx: &mut Context<V>) -> AnyElement {
    let track: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
    let (t1, t2, t3) = (track.clone(), track.clone(), track);
    let s2 = set.clone();
    let frac = |x: Pixels, b: Bounds<Pixels>| (f32::from(x - b.left()) / f32::from(b.size.width).max(1.0)).clamp(0.0, 1.0);
    let id = name;
    div()
        .id(SharedString::from(format!("{key}-{name}")))
        .relative()
        .h(px(15.))
        .mx(px(11.))
        .cursor_pointer()
        .tooltip(crate::ui::tooltip(tip))
        .child(div().absolute().left_0().right_0().top(px(4.)).h(px(7.)).rounded(px(6.)).border_2().border_color(rgb(0x777777)))
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
