//! Onshape's Graphics preferences dialog (Camera and render options >
//! Graphics preferences…; help images ui-ref/dialogs/concepts__graphics-
//! prefs-02-04.png and -sketch-01-01.png): the feature-dialog frame, tabs
//! General | Sketch, collapsible groups of value rows (label, right-aligned
//! value on an underline, ⓘ, slider under it) and color rows (swatch), and
//! "Reset to default" with `?` at the bottom. Changes preview live; ✓ keeps
//! (and saves) them, ✕ restores the values the dialog opened with.

use crate::graphics_prefs::{self, GraphicsPrefs};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub enum GraphicsPrefsEvent {
    Close,
}

/// A numeric preference: getter/setter, range, unit, scale for display
/// (percent = 100), slider or not, and its hint.
#[derive(Clone, Copy)]
struct Num {
    key: &'static str,
    label: &'static str,
    get: fn(&GraphicsPrefs) -> f32,
    set: fn(&mut GraphicsPrefs, f32),
    min: f32,
    max: f32,
    unit: &'static str,
    scale: f32,
    slider: bool,
    hint: &'static str,
}

#[derive(Clone, Copy)]
struct Color {
    key: &'static str,
    label: &'static str,
    get: fn(&GraphicsPrefs) -> u32,
    set: fn(&mut GraphicsPrefs, u32),
    hint: &'static str,
}

#[derive(Clone, Copy)]
enum Row {
    Num(Num),
    Color(Color),
}

const GENERAL: &[(&str, &[Row])] = &[
    (
        "Viewport",
        &[
            Row::Num(Num { key: "ao", label: "Ambient occlusion", get: |g| g.ambient_occlusion, set: |g, v| g.ambient_occlusion = v, min: 0.0, max: 1.0, unit: "%", scale: 100.0, slider: true, hint: "Soft shading where faces meet: corners, crevices, contacts" }),
            Row::Num(Num { key: "fov", label: "Field of view", get: |g| g.fov_deg, set: |g, v| g.fov_deg = v, min: 20.0, max: 120.0, unit: "deg", scale: 1.0, slider: true, hint: "Perspective camera field of view (Perspective view)" }),
        ],
    ),
    (
        "Shaded bodies",
        &[
            Row::Num(Num { key: "lw", label: "Line width", get: |g| g.line_width, set: |g, v| g.line_width = v, min: 0.1, max: 3.0, unit: "px", scale: 1.0, slider: true, hint: "Width of part and surface edges" }),
            Row::Num(Num { key: "shine", label: "Shininess", get: |g| g.shininess, set: |g, v| g.shininess = v, min: 0.01, max: 1.0, unit: "%", scale: 100.0, slider: true, hint: "Default shininess of shaded surfaces" }),
            Row::Color(Color { key: "spec", label: "Specular color", get: |g| g.specular, set: |g, v| g.specular = v, hint: "Default highlight color of shaded surfaces" }),
            Row::Color(Color { key: "interf", label: "Section interference color", get: |g| g.section_interference, set: |g, v| g.section_interference = v, hint: "Where cut parts or surfaces overlap in a section view" }),
        ],
    ),
    (
        "Phantom edges",
        &[
            Row::Num(Num { key: "pw", label: "Phantom tangent edge width", get: |g| g.phantom_width, set: |g, v| g.phantom_width = v, min: 0.1, max: 3.0, unit: "px", scale: 1.0, slider: true, hint: "Width of tangent edges when they are phantom" }),
            Row::Color(Color { key: "pc", label: "Phantom tangent edge color", get: |g| g.phantom_color, set: |g, v| g.phantom_color = v, hint: "Color of tangent edges when they are phantom" }),
        ],
    ),
    ("Mesh bodies", &[Row::Num(Num { key: "mw", label: "Mesh line width", get: |g| g.mesh_line_width, set: |g, v| g.mesh_line_width = v, min: 0.1, max: 3.0, unit: "px", scale: 1.0, slider: true, hint: "Width of mesh surface and edge lines" })]),
];

const SKETCH: &[(&str, &[Row])] = &[
    (
        "Active sketches",
        &[
            Row::Num(Num { key: "slw", label: "Line width", get: |g| g.sketch_line_width, set: |g, v| g.sketch_line_width = v, min: 0.1, max: 3.0, unit: "px", scale: 1.0, slider: true, hint: "Width of the lines of the sketch being edited" }),
            Row::Color(Color { key: "sc", label: "Constrained color", get: |g| g.sketch_constrained, set: |g, v| g.sketch_constrained = v, hint: "Color of fully constrained lines and points" }),
            Row::Color(Color { key: "su", label: "Underconstrained color", get: |g| g.sketch_under, set: |g, v| g.sketch_under = v, hint: "Color of underconstrained lines and points" }),
            Row::Color(Color { key: "suo", label: "Underconstrained occluded color", get: |g| g.sketch_under_occluded, set: |g, v| g.sketch_under_occluded = v, hint: "Color of underconstrained lines and points hidden behind parts" }),
        ],
    ),
    (
        "Inactive sketches",
        &[
            Row::Num(Num { key: "ilw", label: "Line width", get: |g| g.inactive_line_width, set: |g, v| g.inactive_line_width = v, min: 0.1, max: 3.0, unit: "px", scale: 1.0, slider: true, hint: "Width of the lines of other sketches" }),
            Row::Color(Color { key: "ic", label: "Color", get: |g| g.inactive_color, set: |g, v| g.inactive_color = v, hint: "Color of the lines and points of other sketches" }),
        ],
    ),
    (
        "Construction lines",
        &[
            Row::Num(Num { key: "d1", label: "First dash", get: |g| g.construction[0], set: |g, v| g.construction[0] = v, min: 1.0, max: 50.0, unit: "px", scale: 1.0, slider: false, hint: "Length of the first dash of the construction line pattern" }),
            Row::Num(Num { key: "g1", label: "First gap", get: |g| g.construction[1], set: |g, v| g.construction[1] = v, min: 0.0, max: 50.0, unit: "px", scale: 1.0, slider: false, hint: "Length of the first gap" }),
            Row::Num(Num { key: "d2", label: "Second dash", get: |g| g.construction[2], set: |g, v| g.construction[2] = v, min: 1.0, max: 50.0, unit: "px", scale: 1.0, slider: false, hint: "Length of the second dash" }),
            Row::Num(Num { key: "g2", label: "Second gap", get: |g| g.construction[3], set: |g, v| g.construction[3] = v, min: 0.0, max: 50.0, unit: "px", scale: 1.0, slider: false, hint: "Length of the second gap" }),
        ],
    ),
];

/// Swatches of the color picker (Onshape's basic colors).
const PICKER: [u32; 24] = [
    0x000000, 0x232323, 0x404040, 0x666666, 0x999999, 0xcccccc, 0xeaeaea, 0xffffff, //
    0xdc0000, 0xff6a00, 0xffd100, 0x7ac143, 0x009400, 0x00a5b5, 0x0000d0, 0x4040ff, //
    0x1651b0, 0x769fcc, 0x9dcfed, 0x672f8f, 0xc04080, 0xa0522d, 0xf5c97a, 0xfab601,
];

pub struct GraphicsPrefsDialog {
    /// Values when the dialog opened (✕ restores them).
    opened: GraphicsPrefs,
    sketch_tab: bool,
    collapsed: HashSet<&'static str>,
    inputs: HashMap<&'static str, (Entity<InputState>, Subscription)>,
    /// Open color picker: which preference.
    picker: Option<&'static str>,
    hex: Option<(Entity<InputState>, Subscription)>,
    focus: FocusHandle,
}

impl EventEmitter<GraphicsPrefsEvent> for GraphicsPrefsDialog {}

impl Focusable for GraphicsPrefsDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn format_value(n: &Num, v: f32) -> String {
    let d = v * n.scale;
    let s = if (d - d.round()).abs() < 1e-4 { format!("{}", d.round() as i64) } else { format!("{}", (d * 10.0).round() / 10.0) };
    format!("{s} {}", n.unit)
}

impl GraphicsPrefsDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self { opened: graphics_prefs::get(cx), sketch_tab: false, collapsed: HashSet::new(), inputs: HashMap::new(), picker: None, hex: None, focus: cx.focus_handle() }
    }

    fn update(&mut self, f: impl FnOnce(&mut GraphicsPrefs), cx: &mut Context<Self>) {
        let mut g = graphics_prefs::get(cx);
        f(&mut g);
        graphics_prefs::set(g, false, cx);
        cx.notify();
    }

    fn accept(&mut self, cx: &mut Context<Self>) {
        graphics_prefs::set(graphics_prefs::get(cx), true, cx);
        cx.emit(GraphicsPrefsEvent::Close);
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        graphics_prefs::set(self.opened, false, cx);
        cx.emit(GraphicsPrefsEvent::Close);
    }

    /// The value field of a numeric row (typed values apply on Enter / blur,
    /// clamped to the range; the unit may be left out).
    fn input(&mut self, n: Num, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        let current = format_value(&n, (n.get)(&graphics_prefs::get(cx)));
        if let Some((st, _)) = self.inputs.get(n.key) {
            let st = st.clone();
            if !st.read(cx).focus_handle(cx).is_focused(window) && st.read(cx).value() != current.as_str() {
                st.update(cx, |s, cx| s.set_value(current, window, cx));
            }
            return st;
        }
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let text = st.read(cx).value().to_string();
                let num: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
                if let Ok(v) = num.parse::<f32>() {
                    this.update(|g| (n.set)(g, (v / n.scale).clamp(n.min, n.max)), cx);
                }
                let shown = format_value(&n, (n.get)(&graphics_prefs::get(cx)));
                st.update(cx, |s, cx| s.set_value(shown, window, cx));
            }
        });
        self.inputs.insert(n.key, (st.clone(), sub));
        st
    }

    fn slider(&self, n: Num, cx: &mut Context<Self>) -> AnyElement {
        let v = (n.get)(&graphics_prefs::get(cx));
        let t = ((v - n.min) / (n.max - n.min)).clamp(0.0, 1.0);
        let track: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let (t1, t2, t3) = (track.clone(), track.clone(), track.clone());
        let set = move |this: &mut Self, x: Pixels, b: Bounds<Pixels>, cx: &mut Context<Self>| {
            let f = (f32::from(x - b.left()) / f32::from(b.size.width).max(1.0)).clamp(0.0, 1.0);
            this.update(|g| (n.set)(g, n.min + f * (n.max - n.min)), cx);
        };
        div()
            .id(SharedString::from(format!("gp-slider-{}", n.key)))
            .relative()
            .h(px(14.))
            .mx(px(8.))
            .mt(px(2.))
            .mb(px(6.))
            .cursor_pointer()
            .child(div().absolute().left_0().right_0().top(px(6.)).h(px(2.)).bg(rgb(0x777777)))
            .child(
                div()
                    .absolute()
                    .top(px(1.))
                    .left(relative(t))
                    .ml(px(-6.))
                    .size(px(12.))
                    .rounded_full()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0x888888))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.6), offset: point(px(0.), px(0.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }]),
            )
            .child(canvas(move |b, _, _| t1.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| set(this, e.position.x, t2.get(), cx)))
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) && !cx.has_active_drag() {
                    set(this, e.position.x, t3.get(), cx);
                }
            }))
            .into_any_element()
    }

    fn info(hint: &'static str) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("gp-info-{hint}")))
            .size(px(14.))
            .flex_none()
            .rounded_full()
            .border_1()
            .border_color(rgb(0x666666))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0x666666))
            .child("i")
            .tooltip(crate::ui::tooltip(hint))
    }

    fn render_row(&mut self, row: Row, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        match row {
            Row::Num(n) => {
                let st = self.input(n, window, cx);
                v_flex()
                    .child(
                        h_flex()
                            .min_h(px(30.))
                            .pl(px(5.))
                            .pr(px(4.))
                            .gap(px(6.))
                            .child(div().flex_1().text_size(px(13.33)).text_color(pal.text_muted).child(n.label))
                            .child(gpui_kit::base::input::input_frame(div().w(px(95.)).border_b_1().border_color(pal.border).hover(|d| d.border_color(pal.accent)).child(Input::new(&st).small().appearance(false).text_size(px(14.)).text_align(TextAlign::Right))))
                            .child(Self::info(n.hint)),
                    )
                    .when(n.slider, |d| d.child(self.slider(n, cx)))
                    .into_any_element()
            }
            Row::Color(c) => {
                let hex = (c.get)(&graphics_prefs::get(cx));
                let key = c.key;
                h_flex()
                    .min_h(px(38.))
                    .pl(px(5.))
                    .pr(px(4.))
                    .gap(px(6.))
                    // Long labels wrap (live: "Section interference / color").
                    .child(div().flex_1().max_w(px(150.)).text_size(px(13.33)).line_height(px(22.)).text_color(pal.text_muted).child(c.label))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id(SharedString::from(format!("gp-swatch-{key}")))
                            .w(px(100.))
                            .h(px(28.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(0xaaaaaa))
                            .bg(rgb(hex))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.picker = if this.picker == Some(key) { None } else { Some(key) };
                                this.hex = None;
                                if this.picker.is_some() {
                                    let st = cx.new(|cx| InputState::new(window, cx).default_value(format!("#{hex:06x}")));
                                    let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
                                        if matches!(ev, InputEvent::PressEnter { .. }) {
                                            let t = st.read(cx).value().trim().trim_start_matches('#').to_string();
                                            if let Ok(v) = u32::from_str_radix(&t, 16) {
                                                this.update(|g| (c.set)(g, v & 0xffffff), cx);
                                            }
                                        }
                                    });
                                    this.hex = Some((st, sub));
                                }
                                cx.notify();
                            })),
                    )
                    .child(Self::info(c.hint))
                    .into_any_element()
            }
        }
    }

    /// The color picker under the swatch being edited: basic colors and a hex field.
    fn render_picker(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let key = self.picker?;
        let rows = GENERAL.iter().chain(SKETCH.iter()).flat_map(|(_, r)| r.iter());
        let Some(Row::Color(c)) = rows.copied().find(|r| matches!(r, Row::Color(c) if c.key == key)) else { return None };
        let pal = p();
        let current = (c.get)(&graphics_prefs::get(cx));
        let swatches = PICKER.iter().map(|&hex| {
            div()
                .id(SharedString::from(format!("gp-pick-{hex:06x}")))
                .size(px(20.))
                .rounded(px(3.))
                .border_1()
                .border_color(if hex == current { pal.accent } else { rgb(0xaaaaaa) })
                .bg(rgb(hex))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.update(|g| (c.set)(g, hex), cx)))
        });
        Some(
            v_flex()
                .mx(px(8.))
                .mb(px(6.))
                .p(px(6.))
                .gap(px(6.))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(pal.border)
                .rounded(px(4.))
                .child(div().text_size(px(12.)).text_color(pal.text_muted).child(c.label))
                .child(div().flex().flex_wrap().gap(px(4.)).children(swatches))
                .children(self.hex.as_ref().map(|(st, _)| h_flex().gap(px(6.)).child(div().text_size(px(12.)).text_color(pal.text_muted).child("Hex")).child(gpui_kit::base::input::input_frame(div().flex_1().border_b_1().border_color(pal.border).child(Input::new(st).small().appearance(false))))))
                .into_any_element(),
        )
    }
}

impl Render for GraphicsPrefsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let current = graphics_prefs::get(cx);
        let changed = current != self.opened;
        let groups: &[(&str, &[Row])] = if self.sketch_tab { SKETCH } else { GENERAL };
        let mut body = v_flex().pb(px(4.));
        for (name, rows) in groups {
            let open = !self.collapsed.contains(name);
            let name: &'static str = name;
            body = body.child(
                h_flex()
                    .id(SharedString::from(format!("gp-group-{name}")))
                    .h(px(32.))
                    .gap(px(4.))
                    .cursor_pointer()
                    .child(div().w(px(20.)).flex().justify_center().child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 14., rgb(0x333333))))
                    .child(div().text_size(px(14.)).text_color(pal.text).child(name))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.collapsed.remove(name) {
                            this.collapsed.insert(name);
                        }
                        cx.notify();
                    })),
            );
            if open {
                let mut group = v_flex().relative().pl(px(24.)).child(div().absolute().left(px(10.)).top_0().bottom_0().w(px(1.)).bg(pal.border));
                for row in rows.iter() {
                    let key = match row {
                        Row::Num(n) => n.key,
                        Row::Color(c) => c.key,
                    };
                    group = group.child(self.render_row(*row, window, cx));
                    if self.picker == Some(key) {
                        group = group.children(self.render_picker(cx));
                    }
                }
                body = body.child(group);
            }
        }
        let tab = |id: &'static str, label: &'static str, sketch: bool, cx: &mut Context<Self>| {
            let on = self.sketch_tab == sketch;
            div()
                .id(id)
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(pal.border))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.sketch_tab = sketch;
                    this.picker = None;
                    cx.notify();
                }))
        };
        let defaults = current == GraphicsPrefs::default();
        v_flex()
            .id("graphics-prefs")
            .occlude()
            .track_focus(&self.focus)
            .w(px(330.))
            .max_h(px(720.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "escape" => this.cancel(cx),
                "enter" => this.accept(cx),
                _ => {}
            }))
            .child(
                h_flex()
                    .h(px(29.))
                    .flex_none()
                    .pl(px(6.))
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Graphics preferences"))
                    .child(
                        div()
                            .id("gp-ok")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(pal.accept).when(!changed, |d| d.opacity(0.2))
                            .cursor_pointer()
                            .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-ok", 24., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept(cx))),
                    )
                    .child(
                        div()
                            .id("gp-cancel")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
                    ),
            )
            .child(h_flex().w_full().mb(px(4.)).child(tab("gp-tab-general", "General", false, cx)).child(tab("gp-tab-sketch", "Sketch", true, cx)))
            .child(div().id("gp-body").flex_1().overflow_y_scroll().child(body))
            .child(
                h_flex()
                    .h(px(34.))
                    .flex_none()
                    .px(px(6.))
                    .gap(px(8.))
                    .justify_center()
                    .child(
                        div()
                            .id("gp-reset")
                            .h(px(24.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(if defaults { rgb(0xdddddd) } else { rgb(0xaaaaaa) })
                            .text_size(px(12.))
                            .text_color(if defaults { rgb(0xcccccc) } else { pal.text })
                            .when(!defaults, |d| d.cursor_pointer().hover(|d| d.bg(pal.panel_hover)))
                            .child("Reset to default")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.update(|g| *g = GraphicsPrefs::default(), cx);
                            })),
                    )
                    .child(div().size(px(14.)).rounded_full().bg(crate::theme::help_dot(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(crate::theme::p().dialog_bg).child("?")),
            )
    }
}
