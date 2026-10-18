//! Thickness analysis (Analysis tools menu): parts colored by wall
//! thickness. The dialog picks the parts; ✓ keeps the analysis on, and the
//! color bar at the top of the graphics area sets the palette, the method,
//! the bounds (click to type, ↻ to reset, drag the end ticks) and "Dim
//! color scale". Results are computed in the background and refine from the
//! ray method to the rolling ball method, with Onshape's progress messages;
//! they update after every edit.

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::reference::BodyId;
use noshape_model::thickness::{SampleMesh, ThicknessMethod};
use std::collections::HashMap;
use std::sync::Arc;

/// Color palettes of the Colors menu: sRGB stops from low to high.
pub const PALETTES: [(&str, &[u32]); 8] = [
    ("Viridis", &[0x440154, 0x482878, 0x3e4989, 0x31688e, 0x26828e, 0x1f9e89, 0x35b779, 0x6ece58, 0xb5de2b, 0xfde725]),
    ("Plasma", &[0x0d0887, 0x46039f, 0x7201a8, 0x9c179e, 0xbd3786, 0xd8576b, 0xed7953, 0xfb9f3a, 0xfdca26, 0xf0f921]),
    ("Inferno", &[0x000004, 0x1b0c41, 0x4a0c6b, 0x781c6d, 0xa52c60, 0xcf4446, 0xed6925, 0xfb9b06, 0xf7d13d, 0xfcffa4]),
    ("Magma", &[0x000004, 0x180f3d, 0x440f76, 0x721f81, 0x9e2f7f, 0xcd4071, 0xf1605d, 0xfd9668, 0xfeca8d, 0xfcfdbf]),
    ("Cividis", &[0x00224e, 0x123570, 0x3b496c, 0x575d6d, 0x707173, 0x8a8779, 0xa69d75, 0xc4b56c, 0xe4cf5b, 0xfee838]),
    ("Turbo", &[0x30123b, 0x4662d7, 0x36aaf9, 0x1ae4b6, 0x72fe5e, 0xc8ef34, 0xfaba39, 0xf66b19, 0xca2a04, 0x7a0403]),
    ("Rainbow", &[0x0000ff, 0x00ffff, 0x00ff00, 0xffff00, 0xff0000]),
    ("Grayscale", &[0x000000, 0xffffff]),
];

/// Values below / above user bounds, and in-range values with "Dim color
/// scale" (Onshape's legend).
const BELOW: u32 = 0xcc5fc0;
const ABOVE: u32 = 0x6b3a14;
const DIMMED: u32 = 0xe3e3e3;

/// Palette color at `t` in 0..=1.
pub fn palette_color(palette: usize, t: f64) -> u32 {
    let stops = PALETTES[palette.min(PALETTES.len() - 1)].1;
    let x = t.clamp(0.0, 1.0) * (stops.len() - 1) as f64;
    let i = (x.floor() as usize).min(stops.len() - 2);
    let f = x - i as f64;
    let ch = |c: u32, s: u32| ((c >> s) & 0xff) as f64;
    let mix = |s: u32| (ch(stops[i], s) * (1.0 - f) + ch(stops[i + 1], s) * f).round() as u32;
    (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

/// Result for one part: samples and values (meters, or percent).
#[derive(Clone)]
pub struct PartThickness {
    pub mesh: Arc<SampleMesh>,
    pub values: Arc<Vec<Option<f64>>>,
}

/// Dragging an end tick of the color bar (0 lower, 1 upper) or the bar itself.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LegendDrag {
    /// Bound `i`: its value and the cursor x when the drag began, value per pixel.
    Bound { i: usize, from: Pixels, start: f64, per_px: f64 },
    Move { from: Point<Pixels>, start: Point<Pixels> },
}

pub struct ThicknessState {
    pub parts: Vec<BodyId>,
    pub method: ThicknessMethod,
    pub palette: usize,
    pub dim: bool,
    /// User bounds (display units); `None` follows the data.
    pub bounds: [Option<f64>; 2],
    pub results: HashMap<BodyId, PartThickness>,
    /// Progress message while computing.
    pub progress: Option<&'static str>,
    generation: u64,
    task: Option<Task<()>>,
    /// Legend offset from its default spot, and an active drag.
    offset: Point<Pixels>,
    drag: Option<LegendDrag>,
    /// The bound being typed (0 / 1) and its field.
    editing: Option<(usize, Entity<InputState>, Subscription)>,
}

impl ThicknessState {
    pub fn new() -> Self {
        Self { parts: Vec::new(), method: ThicknessMethod::RollingBall, palette: 0, dim: false, bounds: [None; 2], results: HashMap::new(), progress: None, generation: 0, task: None, offset: Point::default(), drag: None, editing: None }
    }
}

impl Default for ThicknessState {
    fn default() -> Self {
        Self::new()
    }
}

impl PartStudioView {
    fn length_factor(&self, cx: &App) -> (f64, &'static str) {
        let u = self.doc.read(cx).session.doc.settings.units.length;
        (u.meters(), u.symbol())
    }

    /// Data range of the results (display units / percent).
    fn thickness_range(&self, cx: &App) -> Option<(f64, f64)> {
        let t = self.ui.thickness.as_ref()?;
        let k = if t.method.is_gradient() { 1.0 } else { 1.0 / self.length_factor(cx).0 };
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for r in t.results.values() {
            for v in r.values.iter().flatten() {
                lo = lo.min(v * k);
                hi = hi.max(v * k);
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    /// Bounds in use: the user's, else the data's.
    fn thickness_bounds(&self, cx: &App) -> Option<(f64, f64)> {
        let t = self.ui.thickness.as_ref()?;
        let (lo, hi) = self.thickness_range(cx)?;
        Some((t.bounds[0].unwrap_or(lo), t.bounds[1].unwrap_or(hi)))
    }

    /// Recompute after a change of parts, method or geometry.
    pub(super) fn recompute_thickness(&mut self, cx: &mut Context<Self>) {
        let Some(t) = &mut self.ui.thickness else { return };
        t.generation += 1;
        let generation = t.generation;
        let method = t.method;
        let meshes: Vec<(BodyId, Arc<noshape_model::display::MeshBuffers>)> = self.display.bodies.iter().filter(|b| t.parts.contains(&b.id) && !b.hidden).map(|b| (b.id.clone(), b.mesh.clone())).collect();
        if meshes.is_empty() {
            t.results.clear();
            t.progress = None;
            t.task = None;
            self.rebuild_scene(cx);
            return;
        }
        t.progress = Some("Preparing thickness analysis");
        let task = cx.spawn(async move |this, cx| {
            // The ray method first (quick); the rolling ball refines it.
            let phases: &[(ThicknessMethod, &'static str)] = match method {
                ThicknessMethod::RollingBall => &[(ThicknessMethod::Ray, "Refining thickness analysis"), (ThicknessMethod::RollingBall, "Postprocessing thickness analysis")],
                ThicknessMethod::RollingBallGradient => &[(ThicknessMethod::RayGradient, "Refining thickness analysis"), (ThicknessMethod::RollingBallGradient, "Postprocessing thickness analysis")],
                ThicknessMethod::Ray => &[(ThicknessMethod::Ray, "Postprocessing thickness analysis")],
                ThicknessMethod::RayGradient => &[(ThicknessMethod::RayGradient, "Postprocessing thickness analysis")],
            };
            for (i, &(m, next)) in phases.iter().enumerate() {
                let _ = this.update(cx, |this: &mut Self, cx| {
                    if let Some(t) = this.ui.thickness.as_mut().filter(|t| t.generation == generation) {
                        t.progress = Some("Calculating thickness");
                        cx.notify();
                    }
                });
                let input = meshes.clone();
                let results: Vec<(BodyId, PartThickness)> = cx
                    .background_spawn(async move {
                        input
                            .iter()
                            .map(|(id, mesh)| {
                                let (samples, values) = noshape_model::thickness::analyze(mesh, m, 40_000);
                                (id.clone(), PartThickness { mesh: Arc::new(samples), values: Arc::new(values) })
                            })
                            .collect()
                    })
                    .await;
                let last = i + 1 == phases.len();
                let ok = this.update(cx, |this: &mut Self, cx| {
                    let Some(t) = this.ui.thickness.as_mut().filter(|t| t.generation == generation) else { return false };
                    t.results = results.into_iter().collect();
                    t.progress = (!last).then_some(next);
                    this.rebuild_scene(cx);
                    true
                });
                if !matches!(ok, Ok(true)) {
                    return;
                }
            }
        });
        if let Some(t) = &mut self.ui.thickness {
            t.task = Some(task);
        }
        cx.notify();
    }

    /// Colored meshes replacing the analyzed parts.
    pub(super) fn thickness_meshes(&self, cx: &App) -> Option<Arc<HashMap<BodyId, noshape_render::MeshHandle>>> {
        let t = self.ui.thickness.as_ref()?;
        if t.results.is_empty() {
            return None;
        }
        let (lo, hi) = self.thickness_bounds(cx)?;
        let k = if t.method.is_gradient() { 1.0 } else { 1.0 / self.length_factor(cx).0 };
        let user = [t.bounds[0].is_some(), t.bounds[1].is_some()];
        let color = |v: Option<f64>| -> [u8; 4] {
            let c = match v {
                None => 0x9a9a9a,
                Some(v) => {
                    let v = v * k;
                    if user[0] && v < lo {
                        BELOW
                    } else if user[1] && v > hi {
                        ABOVE
                    } else if t.dim {
                        DIMMED
                    } else {
                        palette_color(t.palette, if hi > lo { (v - lo) / (hi - lo) } else { 0.5 })
                    }
                }
            };
            [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
        };
        let up = self.gpu.uploader();
        let f = |v: glam::DVec3| [v.x as f32, v.y as f32, v.z as f32];
        let map = t
            .results
            .iter()
            .map(|(id, r)| {
                let positions: Vec<[f32; 3]> = r.mesh.positions.iter().map(|p| f(*p)).collect();
                let normals: Vec<[f32; 3]> = r.mesh.normals.iter().map(|p| f(*p)).collect();
                let colors: Vec<[u8; 4]> = r.values.iter().map(|v| color(*v)).collect();
                (id.clone(), up.upload_mesh_colored(&positions, &normals, &r.mesh.faces, &colors, &r.mesh.indices))
            })
            .collect();
        Some(Arc::new(map))
    }

    /// Analysis tools menu: "Turn thickness analysis off".
    pub(super) fn thickness_off(&mut self, cx: &mut Context<Self>) {
        self.ui.thickness = None;
        self.rebuild_scene(cx);
    }

    fn set_thickness(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut ThicknessState)) {
        if let Some(t) = &mut self.ui.thickness {
            f(t);
        }
        self.rebuild_scene(cx);
    }

    fn edit_bound(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.thickness_bounds(cx) else { return };
        let st = cx.new(|cx| InputState::new(window, cx).default_value(fmt_value([lo, hi][i])));
        st.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let v = st.read(cx).value().trim().parse::<f64>().ok();
            if let Some(t) = &mut this.ui.thickness {
                t.editing = None;
                if let Some(v) = v {
                    t.bounds[i] = Some(v);
                }
            }
            this.rebuild_scene(cx);
        });
        if let Some(t) = &mut self.ui.thickness {
            t.editing = Some((i, st, sub));
        }
        cx.notify();
    }

    /// Progress message at the bottom left while computing.
    pub(super) fn render_thickness_progress(&self) -> Option<AnyElement> {
        let msg = self.ui.thickness.as_ref()?.progress?;
        let pal = p();
        Some(
            h_flex()
                .absolute()
                .left(px(8.))
                .bottom(px(8.))
                .gap_2()
                .px(px(8.))
                .py(px(4.))
                .bg(pal.dialog_bg)
                .rounded(px(2.))
                .text_size(px(12.))
                .text_color(pal.text)
                .child(Icon::new(IconName::Loader).with_size(px(14.)).text_color(pal.accent))
                .child(msg)
                .into_any_element(),
        )
    }

    /// The color bar at the top of the graphics area.
    pub(super) fn render_thickness_legend(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (lo, hi) = self.thickness_bounds(cx)?;
        let (data_lo, data_hi) = self.thickness_range(cx)?;
        let unit = self.length_factor(cx).1;
        let t = self.ui.thickness.as_ref()?;
        let pal = p();
        let (method, palette, dim, user, offset) = (t.method, t.palette, t.dim, [t.bounds[0].is_some(), t.bounds[1].is_some()], t.offset);
        let editing = t.editing.as_ref().map(|(i, st, _)| (*i, st.clone()));
        const BAR: f32 = 600.;
        let view = cx.entity();
        let colors_menu = {
            let view = view.clone();
            Button::new("th-colors").ghost().xsmall().child(h_flex().gap_2().child(PALETTES[palette].0).child(Icon::new(IconName::ChevronDown).with_size(px(10.)))).dropdown_menu(move |mut m, _, _| {
                for (i, (name, _)) in PALETTES.iter().enumerate() {
                    let v = view.clone();
                    m = m.item(PopupMenuItem::new(*name).checked(i == palette).on_click(move |_, _, cx| v.update(cx, |this, cx| this.set_thickness(cx, |t| t.palette = i))));
                }
                m
            })
        };
        let method_menu = {
            let view = view.clone();
            let label = method.label(unit);
            Button::new("th-method").ghost().xsmall().child(h_flex().gap_2().child(label).child(Icon::new(IconName::ChevronDown).with_size(px(10.)))).dropdown_menu(move |mut m, _, _| {
                for mm in ThicknessMethod::ALL {
                    let v = view.clone();
                    m = m.item(PopupMenuItem::new(mm.label(unit)).checked(mm == method).on_click(move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(t) = &mut this.ui.thickness {
                                t.method = mm;
                                t.bounds = [None; 2];
                            }
                            this.recompute_thickness(cx);
                        })
                    }));
                }
                m
            })
        };
        let controls = h_flex()
            .gap(px(14.))
            .text_size(px(12.))
            .child(h_flex().gap_1().child(div().text_color(pal.text_muted).child("Colors")).child(colors_menu))
            .child(Checkbox::new("th-dim").label("Dim color scale").checked(dim).on_click(cx.listener(|this, v: &bool, _, cx| {
                let v = *v;
                this.set_thickness(cx, |t| t.dim = v);
            })))
            .child(h_flex().gap_1().child(div().text_color(pal.text_muted).child("Method")).child(method_menu));
        let rgb_of = |c: u32| gpui_kit::rgb(c);
        // The gradient (or the dimmed bar) between the bounds.
        let segments: Vec<AnyElement> = (0..120).map(|i| div().flex_1().h_full().bg(rgb_of(if dim { DIMMED } else { palette_color(palette, (i as f64 + 0.5) / 120.0) })).into_any_element()).collect();
        let per_px = (hi - lo) / BAR as f64;
        let tick = |i: usize, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("th-tick-{i}")))
                .w(px(3.))
                .h(px(30.))
                .bg(gpui_kit::rgb(0x333333))
                .cursor(CursorStyle::ResizeLeftRight)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if let Some(t) = &mut this.ui.thickness {
                        t.drag = Some(LegendDrag::Bound { i, from: ev.position.x, start: [lo, hi][i], per_px });
                    }
                }))
        };
        // Swatches for values beyond user bounds, labeled with the data extremes.
        let outside = |show: bool, color: u32, value: f64| {
            div().when(show, |d| d.child(h_flex().gap_1().child(div().text_size(px(11.)).font_family("monospace").child(fmt_value(value))).child(div().w(px(8.)).h(px(22.)).bg(rgb_of(color)))))
        };
        let bar = h_flex()
            .items_end()
            .child(outside(user[0], BELOW, data_lo))
            .child(tick(0, cx))
            .child(h_flex().w(px(BAR)).h(px(22.)).children(segments))
            .child(tick(1, cx))
            .child(outside(user[1], ABOVE, data_hi).flex().flex_row_reverse());
        // Tick labels: the bounds (blue, editable) and four between.
        let label = |i: usize, value: f64, cx: &mut Context<Self>| -> AnyElement {
            if let Some((_, st)) = editing.as_ref().filter(|(e, _)| *e == i) {
                return div().w(px(70.)).child(Input::new(st).xsmall()).into_any_element();
            }
            h_flex()
                .gap_1()
                .child(
                    div()
                        .id(SharedString::from(format!("th-bound-{i}")))
                        .text_color(gpui_kit::rgb(0x2b6cd4))
                        .underline()
                        .cursor_pointer()
                        .child(format!("{}{}", fmt_value(value), if user[i] { "*" } else { "" }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.edit_bound(i, window, cx))),
                )
                .when(user[i], |d| {
                    d.child(
                        div()
                            .id(SharedString::from(format!("th-reset-{i}")))
                            .cursor_pointer()
                            .text_color(pal.text_muted)
                            .child("↻")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_thickness(cx, |t| t.bounds[i] = None))),
                    )
                })
                .into_any_element()
        };
        let mut labels = h_flex().w(px(BAR + 6.)).justify_between().text_size(px(12.)).font_family("monospace").text_color(pal.text).child(label(0, lo, cx));
        for k in 1..5 {
            labels = labels.child(div().child(fmt_value(lo + (hi - lo) * k as f64 / 5.0)));
        }
        labels = labels.child(label(1, hi, cx));
        let range = (lo, hi);
        Some(
            div()
                .absolute()
                .top(px(8.) + offset.y)
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    v_flex()
                        .id("thickness-legend")
                        .occlude()
                        .ml(offset.x * 2.)
                        .items_center()
                        .gap(px(4.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(2.))
                        .hover(|d| d.bg(hsla(0., 0., 1., 0.85)))
                        .child(controls)
                        .child(bar)
                        .child(labels)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, _| {
                            if let Some(t) = &mut this.ui.thickness
                                && t.drag.is_none()
                            {
                                t.drag = Some(LegendDrag::Move { from: ev.position, start: t.offset });
                            }
                        }))
                        .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                            let Some(t) = &mut this.ui.thickness else { return };
                            let mut moved_bound = false;
                            match t.drag {
                                Some(LegendDrag::Move { from, start }) if ev.dragging() => {
                                    t.offset = point(start.x + (ev.position.x - from.x) * 0.5, start.y + ev.position.y - from.y);
                                    cx.notify();
                                }
                                Some(LegendDrag::Bound { i, from, start, per_px }) if ev.dragging() => {
                                    // The bound follows the tick along the bar.
                                    let v = start + f32::from(ev.position.x - from) as f64 * per_px;
                                    t.bounds[i] = Some(if i == 0 { v.min(range.1) } else { v.max(range.0) });
                                    moved_bound = true;
                                }
                                Some(_) => t.drag = None,
                                None => {}
                            }
                            if moved_bound {
                                this.rebuild_scene(cx);
                            }
                        }))
                        .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| {
                            if let Some(t) = &mut this.ui.thickness {
                                t.drag = None;
                            }
                        })),
                )
                .into_any_element(),
        )
    }
}

/// A legend number: three significant decimals at most.
fn fmt_value(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn palettes_span_their_stops() {
        assert_eq!(palette_color(0, 0.0), 0x440154);
        assert_eq!(palette_color(0, 1.0), 0xfde725);
        assert_eq!(palette_color(7, 0.5), 0x808080);
        assert_eq!(fmt_value(0.70710678), "0.707");
        assert_eq!(fmt_value(3.0), "3");
    }
}
