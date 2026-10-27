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
use crate::quantity_field::{CommitVia, QuantityEvent, QuantityInput, QuantitySpec};
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

/// The curvature color map's Colors menu (Onshape help).
pub const CURVATURE_PALETTES: [(&str, &[u32]); 4] = [
    ("Viridis", &[0x440154, 0x482878, 0x3e4989, 0x31688e, 0x26828e, 0x1f9e89, 0x35b779, 0x6ece58, 0xb5de2b, 0xfde725]),
    ("Blue - > Red", &[0x0000ff, 0x8000ff, 0xff00ff, 0xff0080, 0xff0000]),
    ("Plasma", &[0x0d0887, 0x46039f, 0x7201a8, 0x9c179e, 0xbd3786, 0xd8576b, 0xed7953, 0xfb9f3a, 0xfdca26, 0xf0f921]),
    ("Rainbow", &[0x0000ff, 0x00ffff, 0x00ff00, 0xffff00, 0xff0000]),
];

/// The palettes of a bar.
pub fn palettes(bar: Bar) -> &'static [(&'static str, &'static [u32])] {
    match bar {
        Bar::Thickness => &PALETTES,
        Bar::Curvature => &CURVATURE_PALETTES,
    }
}

/// Palette color at `t` in 0..=1.
pub fn palette_color(palette: usize, t: f64) -> u32 {
    palette_color_in(&PALETTES, palette, t)
}

/// Color at `t` in 0..=1 of palette `palette` of `table`.
pub fn palette_color_in(table: &[(&str, &[u32])], palette: usize, t: f64) -> u32 {
    let stops = table[palette.min(table.len() - 1)].1;
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

/// Which color bar: Thickness analysis or the curvature color map (the
/// same Onshape legend: Colors, a second menu, bounds you can type or drag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Thickness,
    Curvature,
}

/// A color bar's palette, user bounds and on-screen state.
#[derive(Default)]
pub struct BarUi {
    pub palette: usize,
    /// User bounds (display units); `None` follows the data.
    pub bounds: [Option<f64>; 2],
    /// Offset from its default spot, and an active drag.
    offset: Point<Pixels>,
    drag: Option<LegendDrag>,
    /// The pointer is over the color map (Onshape: the sketch crosshair).
    pub map_hover: bool,
    /// The bound being typed (0 / 1) and its field.
    editing: Option<(usize, Entity<QuantityInput>, Subscription)>,
}

pub struct ThicknessState {
    pub parts: Vec<BodyId>,
    pub method: ThicknessMethod,
    pub dim: bool,
    pub bar: BarUi,
    pub results: HashMap<BodyId, PartThickness>,
    /// Progress message while computing.
    pub progress: Option<&'static str>,
    generation: u64,
    task: Option<Task<()>>,
}

impl ThicknessState {
    pub fn new() -> Self {
        Self { parts: Vec::new(), method: ThicknessMethod::RollingBall, dim: false, bar: BarUi::default(), results: HashMap::new(), progress: None, generation: 0, task: None }
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
        Some((t.bar.bounds[0].unwrap_or(lo), t.bar.bounds[1].unwrap_or(hi)))
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
                    if let Some(t) = this.ui.thickness.as_mut().filter(|t| t.generation == generation && i == 0) {
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
                                let (samples, values) = noshape_model::thickness::analyze(mesh, m, 25_000);
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
        let user = [t.bar.bounds[0].is_some(), t.bar.bounds[1].is_some()];
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
                        palette_color(t.bar.palette, if hi > lo { (v - lo) / (hi - lo) } else { 0.5 })
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

    fn bar_ui(&self, bar: Bar) -> Option<&BarUi> {
        match bar {
            Bar::Thickness => self.ui.thickness.as_ref().map(|t| &t.bar),
            Bar::Curvature => self.ui.curvature_map.as_ref().map(|c| &c.bar),
        }
    }

    fn bar_ui_mut(&mut self, bar: Bar) -> Option<&mut BarUi> {
        match bar {
            Bar::Thickness => self.ui.thickness.as_mut().map(|t| &mut t.bar),
            Bar::Curvature => self.ui.curvature_map.as_mut().map(|c| &mut c.bar),
        }
    }

    /// Data range shown by a bar.
    fn bar_range(&self, bar: Bar, cx: &App) -> Option<(f64, f64)> {
        match bar {
            Bar::Thickness => self.thickness_range(cx),
            Bar::Curvature => self.ui.curvature_map.as_ref()?.range,
        }
    }

    /// Bounds in use: the user's, else the data's.
    fn bar_bounds(&self, bar: Bar, cx: &App) -> Option<(f64, f64)> {
        let (lo, hi) = self.bar_range(bar, cx)?;
        let b = self.bar_ui(bar)?;
        Some((b.bounds[0].unwrap_or(lo), b.bounds[1].unwrap_or(hi)))
    }

    /// Change a bar's settings and recolor.
    fn set_bar(&mut self, bar: Bar, cx: &mut Context<Self>, f: impl FnOnce(&mut BarUi)) {
        if let Some(b) = self.bar_ui_mut(bar) {
            f(b);
        }
        self.rebuild_scene(cx);
    }

    fn edit_bound(&mut self, bar: Bar, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.bar_bounds(bar, cx) else { return };
        // Live ("Curvature color map › upper bound"): the label turns into a
        // number field with the caret at the end; Enter applies a valid value
        // and closes it, an invalid one ("1 in", "abc") stays pink and open;
        // Tab applies it and the field stays; Esc closes it unchanged.
        let units = self.doc.read(cx).session.doc.settings.units;
        let spec = QuantitySpec::quantity(noshape_units::Dim::UNITLESS).caret_on_focus().text_px(12.);
        let st = cx.new(|cx| QuantityInput::new(fmt_value([lo, hi][i]), spec, units, window, cx));
        st.update(cx, |q, cx| q.state().update(cx, |s, cx| s.focus(window, cx)));
        let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &QuantityEvent, cx| match ev {
            QuantityEvent::Committed(via) => {
                let Some(v) = st.read(cx).value() else { return };
                let close = *via == CommitVia::Enter;
                this.set_bar(bar, cx, |b| {
                    b.bounds[i] = Some(v);
                    if close {
                        b.editing = None;
                    }
                });
            }
            QuantityEvent::Escape => this.set_bar(bar, cx, |b| b.editing = None),
            _ => {}
        });
        if let Some(b) = self.bar_ui_mut(bar) {
            b.editing = Some((i, st, sub));
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

    /// The color bar at the top of the graphics area (Thickness analysis,
    /// Curvature color map).
    pub(super) fn render_color_bar(&mut self, bar: Bar, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (lo, hi) = self.bar_bounds(bar, cx)?;
        let (data_lo, data_hi) = self.bar_range(bar, cx)?;
        let unit = self.length_factor(cx).1;
        let b = self.bar_ui(bar)?;
        let pal = p();
        let (palette, user, offset) = (b.palette, [b.bounds[0].is_some(), b.bounds[1].is_some()], b.offset);
        let moving = matches!(b.drag, Some(LegendDrag::Move { .. }));
        let editing = b.editing.as_ref().map(|(i, st, _)| (*i, st.clone()));
        let dim = bar == Bar::Thickness && self.ui.thickness.as_ref().is_some_and(|t| t.dim);
        let table = palettes(bar);
        const BAR: f32 = 600.;
        let view = cx.entity();
        type Items = Vec<(String, bool, Box<dyn Fn(&mut PartStudioView, &mut Context<PartStudioView>)>)>;
        let menu = |id: &'static str, label: String, items: Items| {
            let view = view.clone();
            let items: std::rc::Rc<Items> = std::rc::Rc::new(items);
            Button::new(id).ghost().xsmall().child(h_flex().gap_2().child(label).child(Icon::new(IconName::ChevronDown).with_size(px(10.)))).dropdown_menu(move |mut m, _, _| {
                for k in 0..items.len() {
                    let (v, items2) = (view.clone(), items.clone());
                    m = m.item(PopupMenuItem::new(items[k].0.clone()).checked(items[k].1).on_click(move |_, _, cx| v.update(cx, |this, cx| (items2[k].2)(this, cx))));
                }
                m
            })
        };
        let colors_menu = menu(
            "cb-colors",
            table[palette.min(table.len() - 1)].0.to_string(),
            table.iter().enumerate().map(|(i, (name, _))| (name.to_string(), i == palette, Box::new(move |this: &mut Self, cx: &mut Context<Self>| this.set_bar(bar, cx, |b| b.palette = i)) as Box<dyn Fn(&mut Self, &mut Context<Self>)>)).collect(),
        );
        let mut controls = h_flex().gap(px(14.)).text_size(px(12.)).child(h_flex().gap_1().child(div().text_color(pal.text_muted).child("Colors")).child(colors_menu));
        match bar {
            Bar::Thickness => {
                let method = self.ui.thickness.as_ref().map_or(ThicknessMethod::RollingBall, |t| t.method);
                let method_menu = menu(
                    "cb-method",
                    method.label(unit),
                    ThicknessMethod::ALL
                        .into_iter()
                        .map(|mm| {
                            (mm.label(unit), mm == method, Box::new(move |this: &mut Self, cx: &mut Context<Self>| {
                                if let Some(t) = &mut this.ui.thickness {
                                    t.method = mm;
                                    t.bar.bounds = [None; 2];
                                }
                                this.recompute_thickness(cx);
                            }) as Box<dyn Fn(&mut Self, &mut Context<Self>)>)
                        })
                        .collect(),
                );
                controls = controls
                    .child(Checkbox::new("cb-dim").label("Dim color scale").checked(dim).on_click(cx.listener(|this, v: &bool, _, cx| {
                        let v = *v;
                        this.set_thickness(cx, |t| t.dim = v);
                    })))
                    .child(h_flex().gap_1().child(div().text_color(pal.text_muted).child("Method")).child(method_menu));
            }
            Bar::Curvature => {
                let kind = self.ui.curvature_map.as_ref().map_or(CurvatureKind::Gaussian, |c| c.kind);
                let kind_menu = menu(
                    "cb-kind",
                    kind.label(unit),
                    CurvatureKind::ALL
                        .into_iter()
                        .map(|k| {
                            (k.label(unit), k == kind, Box::new(move |this: &mut Self, cx: &mut Context<Self>| {
                                if let Some(c) = &mut this.ui.curvature_map {
                                    c.kind = k;
                                    c.bar.bounds = [None; 2];
                                }
                                this.rebuild_scene(cx);
                            }) as Box<dyn Fn(&mut Self, &mut Context<Self>)>)
                        })
                        .collect(),
                );
                controls = controls.child(h_flex().gap_1().child(div().text_color(pal.text_muted).child("Curvature")).child(kind_menu));
            }
        }
        let rgb_of = |c: u32| gpui_kit::rgb(c);
        // The gradient (or the dimmed bar) between the bounds.
        let segments: Vec<AnyElement> = (0..120).map(|i| div().flex_1().h_full().bg(rgb_of(if dim { DIMMED } else { palette_color_in(table, palette, (i as f64 + 0.5) / 120.0) })).into_any_element()).collect();
        let per_px = (hi - lo) / BAR as f64;
        let tick = |i: usize, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("cb-tick-{i}")))
                .w(px(3.))
                .h(px(30.))
                .bg(gpui_kit::rgb(0x333333))
                .cursor(CursorStyle::ResizeLeftRight)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if let Some(b) = this.bar_ui_mut(bar) {
                        b.drag = Some(LegendDrag::Bound { i, from: ev.position.x, start: [lo, hi][i], per_px });
                    }
                }))
        };
        // Swatches for values beyond user bounds, labeled with the data extremes.
        let outside = |show: bool, color: u32, value: f64| {
            div().when(show, |d| d.child(h_flex().gap_1().child(div().text_size(px(11.)).font_family("monospace").child(fmt_value(value))).child(div().w(px(8.)).h(px(22.)).bg(rgb_of(color)))))
        };
        let strip = h_flex()
            .items_end()
            .child(outside(user[0], BELOW, data_lo))
            .child(tick(0, cx))
            .child(
                h_flex()
                    .id(SharedString::from(format!("cb-map-{bar:?}")))
                    .w(px(BAR))
                    .h(px(22.))
                    .children(segments)
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if let Some(b) = this.bar_ui_mut(bar) {
                            b.map_hover = *h;
                        }
                        this.push_tool_cursor(cx);
                    })),
            )
            .child(tick(1, cx))
            .child(outside(user[1], ABOVE, data_hi).flex().flex_row_reverse());
        // Tick labels: the bounds (blue, editable) and four between.
        let label = |i: usize, value: f64, cx: &mut Context<Self>| -> AnyElement {
            if let Some((_, st)) = editing.as_ref().filter(|(e, _)| *e == i) {
                return div().w(px(70.)).child(st.clone()).into_any_element();
            }
            h_flex()
                .gap_1()
                .child(
                    div()
                        .id(SharedString::from(format!("cb-bound-{i}")))
                        .text_color(gpui_kit::rgb(0x2b6cd4))
                        .underline()
                        .cursor_pointer()
                        .child(format!("{}{}", fmt_value(value), if user[i] { "*" } else { "" }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.edit_bound(bar, i, window, cx))),
                )
                .when(user[i], |d| {
                    d.child(
                        div()
                            .id(SharedString::from(format!("cb-reset-{i}")))
                            .cursor_pointer()
                            .text_color(pal.text_muted)
                            .child("↻")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_bar(bar, cx, |b| b.bounds[i] = None))),
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
                        .id(SharedString::from(format!("color-bar-{bar:?}")))
                        .occlude()
                        .ml(offset.x * 2.)
                        .items_center()
                        .gap(px(4.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(2.))
                        .hover(|d| d.bg(hsla(0., 0., 1., 0.85)))
                        // Onshape's legend: grab, grabbing while it is moved.
                        .cursor(if moving { CursorStyle::ClosedHand } else { CursorStyle::OpenHand })
                        .child(controls)
                        .child(strip)
                        .child(labels)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, _| {
                            if let Some(b) = this.bar_ui_mut(bar)
                                && b.drag.is_none()
                            {
                                b.drag = Some(LegendDrag::Move { from: ev.position, start: b.offset });
                            }
                        }))
                        .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                            let Some(b) = this.bar_ui_mut(bar) else { return };
                            let mut moved_bound = false;
                            match b.drag {
                                Some(LegendDrag::Move { from, start }) if ev.dragging() => {
                                    b.offset = point(start.x + (ev.position.x - from.x) * 0.5, start.y + ev.position.y - from.y);
                                    cx.notify();
                                }
                                Some(LegendDrag::Bound { i, from, start, per_px }) if ev.dragging() => {
                                    // The bound follows the tick along the bar.
                                    let v = start + f32::from(ev.position.x - from) as f64 * per_px;
                                    b.bounds[i] = Some(if i == 0 { v.min(range.1) } else { v.max(range.0) });
                                    moved_bound = true;
                                }
                                Some(_) => b.drag = None,
                                None => {}
                            }
                            if moved_bound {
                                this.rebuild_scene(cx);
                            }
                        }))
                        .on_mouse_up(MouseButton::Left, cx.listener(move |this, _: &MouseUpEvent, _, _| {
                            if let Some(b) = this.bar_ui_mut(bar) {
                                b.drag = None;
                            }
                        })),
                )
                .into_any_element(),
        )
    }
}

pub use noshape_model::analysis::CurvatureKind;

/// The curvature color map's settings and its data range.
#[derive(Default)]
pub struct CurvatureMap {
    pub kind: CurvatureKind,
    pub bar: BarUi,
    pub range: Option<(f64, f64)>,
}

/// A legend number: three significant decimals at most.
fn fmt_value(v: f64) -> String {
    let s = noshape_units::to_fixed(v, 3);
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
        assert_eq!(fmt_value(std::f64::consts::FRAC_1_SQRT_2), "0.707");
        assert_eq!(fmt_value(3.0), "3");
    }
}
