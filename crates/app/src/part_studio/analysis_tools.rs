//! Onshape's analysis tool dialogs (Analysis tools menu at the bottom
//! right; Shift+C Curve/surface analysis, Shift+D Dihedral analysis): a
//! dialog at the feature dialog position with a selection field, the
//! tool's options and a comb scale slider; the combs are drawn over the
//! model. Selections persist for the next time the tool opens; a
//! preselection replaces them (Onshape).

use super::PartStudioView;
use crate::theme::p;
use glam::DVec3;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::analysis::Comb;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment, PointData, PointSprite};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    CurveSurface,
    Dihedral,
}

impl ToolKind {
    pub fn title(self) -> &'static str {
        match self {
            ToolKind::CurveSurface => "Curve/surface analysis",
            ToolKind::Dihedral => "Dihedral analysis",
        }
    }
}

/// Options of the open tool (Onshape's checkboxes and fields).
#[derive(Clone, Debug, PartialEq)]
pub struct ToolOptions {
    pub combs: bool,
    pub show_u: bool,
    pub show_v: bool,
    pub u_curves: usize,
    pub v_curves: usize,
    pub inflections: bool,
    pub min_radius: bool,
    pub extend: bool,
    pub control_points: bool,
    /// Dihedral: show the maximum (else minimum) angle.
    pub show_max: bool,
    pub threshold: bool,
    pub high_deg: f64,
    pub low_deg: f64,
    /// Comb length factor (scale slider).
    pub scale: f32,
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self { combs: true, show_u: true, show_v: true, u_curves: 8, v_curves: 8, inflections: true, min_radius: false, extend: false, control_points: false, show_max: true, threshold: false, high_deg: 180.0, low_deg: 0.0, scale: 0.3 }
    }
}

/// Computed geometry of the open tool.
#[derive(Clone, Debug, Default)]
pub struct ToolResult {
    /// Combs with their tooth color (sRGB).
    pub combs: Vec<(Comb, [u8; 4])>,
    /// Polylines (control point grids), magenta.
    pub polylines: Vec<Vec<DVec3>>,
    /// Inflection points of sketch curves.
    pub inflections: Vec<DVec3>,
    /// Minimum radius and where.
    pub min_radius: Option<(f64, DVec3)>,
    /// Dihedral min / max angle (degrees).
    pub dihedral: Option<(f64, f64)>,
}

pub struct AnalysisTool {
    pub kind: ToolKind,
    pub selection: Vec<Reference>,
    pub options: ToolOptions,
    pub result: Option<ToolResult>,
    slider: Entity<SliderState>,
    _slider_sub: Subscription,
}

/// Selections of each tool, kept for the next time it opens.
#[derive(Default)]
pub struct ToolMemory {
    pub selection: std::collections::HashMap<&'static str, Vec<Reference>>,
    pub options: std::collections::HashMap<&'static str, ToolOptions>,
}

const COMB: [u8; 4] = [40, 170, 60, 255];
const ENVELOPE: [u8; 4] = [210, 40, 200, 255];
const U_CURVES: [u8; 4] = [220, 50, 40, 255];
const V_CURVES: [u8; 4] = [40, 90, 220, 255];

impl PartStudioView {
    /// Open a tool (or close it when it is the open one, like Shift+C/D).
    pub(super) fn toggle_analysis_tool(&mut self, kind: ToolKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.analysis_tool.as_ref().is_some_and(|t| t.kind == kind) {
            self.close_analysis_tool(cx);
            return;
        }
        self.close_analysis_tool(cx);
        let key = kind.title();
        // A preselection replaces the remembered selection.
        let pre: Vec<Reference> = self.selection.iter().filter(|r| self.analysis_accepts(kind, r)).cloned().collect();
        let mut selection = if pre.is_empty() { self.ui.tool_memory.selection.get(key).cloned().unwrap_or_default() } else { pre };
        // In a sketch, Shift+C takes every curve of the sketch.
        if kind == ToolKind::CurveSurface
            && let Some(sk) = self.sketch.as_ref().map(|s| s.feature)
            && let Some(sd) = self.display.sketches.iter().find(|s| s.feature == sk)
        {
            selection = sd.curves.iter().map(|c| Reference::SketchEntity { sketch: sk, entity: c.entity }).collect();
        }
        let options = self.ui.tool_memory.options.get(key).cloned().unwrap_or_default();
        let slider = cx.new(|_| SliderState::new().min(0.05).max(1.0).step(0.01).default_value(options.scale));
        let sub = cx.subscribe_in(&slider, window, |this: &mut Self, _, ev: &SliderEvent, _, cx| {
            let (SliderEvent::Change(v) | SliderEvent::Release(v)) = ev;
            if let Some(t) = &mut this.ui.analysis_tool {
                t.options.scale = v.start();
            }
            this.refresh_scene(false, cx);
            cx.notify();
        });
        self.ui.analysis_tool = Some(AnalysisTool { kind, selection, options, result: None, slider, _slider_sub: sub });
        self.recompute_analysis_tool(cx);
    }

    pub(super) fn close_analysis_tool(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(t) = self.ui.analysis_tool.take() else { return false };
        let key = t.kind.title();
        self.ui.tool_memory.selection.insert(key, t.selection);
        self.ui.tool_memory.options.insert(key, t.options);
        self.refresh_scene(false, cx);
        cx.notify();
        true
    }

    fn analysis_accepts(&self, kind: ToolKind, r: &Reference) -> bool {
        match kind {
            ToolKind::CurveSurface => matches!(r, Reference::Edge(_) | Reference::Face(_) | Reference::SketchEntity { .. }),
            ToolKind::Dihedral => matches!(r, Reference::Edge(_)),
        }
    }

    /// A viewport pick while a tool is open toggles the entity in its field.
    pub(super) fn analysis_pick(&mut self, r: &Reference, cx: &mut Context<Self>) -> bool {
        let Some(kind) = self.ui.analysis_tool.as_ref().map(|t| t.kind) else { return false };
        eprintln!("PSBDBG analysis pick {r:?}");
        if !self.analysis_accepts(kind, r) {
            self.flash_info(if kind == ToolKind::Dihedral { "Select edges".into() } else { "Select edges, faces or sketch curves".into() }, cx);
            return true;
        }
        if let Some(t) = &mut self.ui.analysis_tool {
            match t.selection.iter().position(|x| x == r) {
                Some(i) => {
                    t.selection.remove(i);
                }
                None => t.selection.push(r.clone()),
            }
        }
        self.recompute_analysis_tool(cx);
        true
    }

    /// Evaluate the combs for the selection and options (kernel calls happen
    /// here only, not per frame).
    pub(super) fn recompute_analysis_tool(&mut self, cx: &mut Context<Self>) {
        let Some(t) = &self.ui.analysis_tool else { return };
        let (kind, sel, o) = (t.kind, t.selection.clone(), t.options.clone());
        let mut res = ToolResult::default();
        if let Some(ctx) = self.context.clone() {
            let samples = 32;
            match kind {
                ToolKind::CurveSurface => {
                    let mut faces: Vec<Reference> = sel.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect();
                    if o.extend {
                        // Extend across face: the tangent-connected faces too.
                        for f in faces.clone() {
                            for g in noshape_model::select::select_related(&ctx, &f, noshape_model::select::Criterion::TangentFaces) {
                                if !faces.contains(&g) {
                                    faces.push(g);
                                }
                            }
                        }
                    }
                    for r in &sel {
                        match r {
                            Reference::Edge(_) => {
                                if let Some((comb, min)) = noshape_model::analysis::edge_curvature(&ctx, r, samples) {
                                    if o.combs {
                                        res.combs.push((comb, COMB));
                                    }
                                    keep_min(&mut res.min_radius, min);
                                }
                                if o.control_points
                                    && let Some(poles) = edge_poles(&ctx, r)
                                {
                                    res.polylines.push(poles);
                                }
                            }
                            Reference::SketchEntity { sketch, entity } => {
                                let Some(c) = self.display.sketches.iter().find(|s| s.feature == *sketch).and_then(|s| s.curves.iter().find(|c| c.entity == *entity)) else { continue };
                                let pts: Vec<DVec3> = c.points.iter().map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).collect();
                                let (comb, min) = noshape_model::analysis::polyline_comb(&pts);
                                if o.inflections {
                                    res.inflections.extend(inflections(&comb));
                                }
                                if o.combs {
                                    res.combs.push((comb, COMB));
                                }
                                keep_min(&mut res.min_radius, min);
                            }
                            _ => {}
                        }
                    }
                    for f in &faces {
                        if let Some((along_u, along_v)) = noshape_model::analysis::face_curvature(&ctx, f, o.u_curves, o.v_curves, samples) {
                            if o.show_u {
                                res.combs.extend(along_u.into_iter().map(|c| (c, U_CURVES)));
                            }
                            if o.show_v {
                                res.combs.extend(along_v.into_iter().map(|c| (c, V_CURVES)));
                            }
                        }
                    }
                }
                ToolKind::Dihedral => {
                    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
                    for r in &sel {
                        if let Some((comb, a, b)) = noshape_model::analysis::edge_dihedral(&ctx, r, samples) {
                            lo = lo.min(a);
                            hi = hi.max(b);
                            res.combs.push((comb, COMB));
                        }
                    }
                    res.dihedral = (lo <= hi).then_some((lo, hi));
                }
            }
        }
        if let Some(t) = &mut self.ui.analysis_tool {
            t.result = Some(res);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Combs, envelopes, control polygons and markers over the model.
    pub(super) fn analysis_tool_overlay(&self) -> Vec<Instance> {
        let Some(t) = &self.ui.analysis_tool else { return Vec::new() };
        let Some(res) = &t.result else { return Vec::new() };
        let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
        let size = self.fit_bounds().map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.1);
        let max = res.combs.iter().map(|(c, _)| c.max_len()).fold(0.0, f64::max);
        let k = if max > 0.0 { size * 0.25 * t.options.scale as f64 / max } else { 0.0 };
        let mut lines = LineData::default();
        for (comb, color) in &res.combs {
            let mut tips = Vec::with_capacity(comb.base.len());
            for (b, tooth) in comb.base.iter().zip(&comb.teeth) {
                // Dihedral thresholds: teeth outside the limits turn red / blue.
                let mut c = *color;
                if t.kind == ToolKind::Dihedral && t.options.threshold {
                    let deg = tooth.length().to_degrees();
                    if deg > t.options.high_deg {
                        c = U_CURVES;
                    } else if deg < t.options.low_deg {
                        c = V_CURVES;
                    }
                }
                let tip = *b + *tooth * k;
                lines.push_polyline(&[f(*b), f(tip)], c, 0, 1.0, LineSegment::ON_TOP);
                tips.push(f(tip));
            }
            lines.push_polyline(&tips, ENVELOPE, 0, 1.25, LineSegment::ON_TOP);
        }
        for pl in &res.polylines {
            let pts: Vec<[f32; 3]> = pl.iter().map(|p| f(*p)).collect();
            lines.push_polyline(&pts, ENVELOPE, 0, 1.0, LineSegment::ON_TOP | LineSegment::DASHED);
        }
        let up = self.gpu.uploader();
        let mut out = vec![Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(35)];
        let mut pts = PointData::default();
        for p in &res.inflections {
            pts.points.push(PointSprite { position: f(*p), color: [0, 0, 0, 255], pick_id: 0, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
        }
        if let Some((_, p)) = res.min_radius.filter(|_| t.options.min_radius && t.kind == ToolKind::CurveSurface) {
            pts.points.push(PointSprite { position: f(p), color: ENVELOPE, pick_id: 0, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
        }
        for p in res.polylines.iter().flatten() {
            pts.points.push(PointSprite { position: f(*p), color: ENVELOPE, pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
        }
        if !pts.points.is_empty() {
            out.push(Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(36));
        }
        out
    }

    /// The tool's dialog (left, like feature dialogs) and the minimum
    /// radius label in the viewport.
    pub(super) fn render_analysis_tool(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(t) = &self.ui.analysis_tool else { return Vec::new() };
        let pal = p();
        let units = self.doc.read(cx).session.doc.settings.units;
        let labeler = self.labeler(cx);
        let (kind, o) = (t.kind, t.options.clone());
        let fmt_len = |v: f64| noshape_units::format_quantity(noshape_units::Quantity::new(v, noshape_units::Dim::LENGTH), &units);
        let check = |id: &'static str, label: &'static str, on: bool, set: fn(&mut ToolOptions, bool), cx: &mut Context<Self>| {
            gpui_kit::component::checkbox::Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                if let Some(t) = &mut this.ui.analysis_tool {
                    set(&mut t.options, *v);
                }
                this.recompute_analysis_tool(cx);
            }))
        };
        let stepper = |id: &'static str, label: &'static str, value: String, step: fn(&mut ToolOptions, i32), cx: &mut Context<Self>| {
            let btn = |sid: String, glyph: &'static str, d: i32, cx: &mut Context<Self>| {
                div().id(SharedString::from(sid)).px_1().cursor_pointer().text_color(pal.text_muted).child(glyph).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(t) = &mut this.ui.analysis_tool {
                        step(&mut t.options, d);
                    }
                    this.recompute_analysis_tool(cx);
                }))
            };
            h_flex()
                .gap_1()
                .text_size(px(12.))
                .child(div().flex_1().text_color(pal.text_muted).child(label))
                .child(btn(format!("{id}-m"), "−", -1, cx))
                .child(div().min_w(px(40.)).flex().justify_center().text_color(pal.text).child(value))
                .child(btn(format!("{id}-p"), "+", 1, cx))
        };
        let rows: Vec<AnyElement> = t
            .selection
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let r2 = r.clone();
                h_flex()
                    .id(SharedString::from(format!("at-sel-{i}")))
                    .group("atrow")
                    .h(px(23.))
                    .px(px(6.))
                    .text_size(px(12.))
                    .text_color(pal.text)
                    .child(div().flex_1().truncate().child(labeler(r)))
                    .child(
                        div()
                            .id(SharedString::from(format!("at-x-{i}")))
                            .invisible()
                            .group_hover("atrow", |d| d.visible())
                            .cursor_pointer()
                            .child(Icon::new(IconName::Close).with_size(px(11.)).text_color(pal.text_muted))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(t) = &mut this.ui.analysis_tool {
                                    t.selection.retain(|x| *x != r2);
                                }
                                this.recompute_analysis_tool(cx);
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        let caption = if kind == ToolKind::Dihedral { "Edges to analyze" } else { "Entities to analyze" };
        let field = v_flex()
            .min_h(px(26.))
            .rounded(px(2.))
            .border_1()
            .border_color(pal.accent)
            .bg(gpui_kit::rgb(0xdef1fb))
            .child(div().px(px(6.)).when(empty, |d| d.py(px(5.)).text_size(px(12.))).when(!empty, |d| d.pt(px(2.)).text_size(px(10.))).text_color(pal.text_muted).child(caption))
            .children(rows);
        let mut body = v_flex().p(px(4.)).gap(px(6.)).child(field);
        match kind {
            ToolKind::CurveSurface => {
                body = body
                    .child(check("at-combs", "Curvature combs", o.combs, |o, v| o.combs = v, cx))
                    .child(check("at-u", "Show U curves", o.show_u, |o, v| o.show_u = v, cx))
                    .child(stepper("at-uc", "U curves", o.v_curves.to_string(), |o, d| o.v_curves = (o.v_curves as i32 + d).clamp(2, 64) as usize, cx))
                    .child(check("at-v", "Show V curves", o.show_v, |o, v| o.show_v = v, cx))
                    .child(stepper("at-vc", "V curves", o.u_curves.to_string(), |o, d| o.u_curves = (o.u_curves as i32 + d).clamp(2, 64) as usize, cx))
                    .child(check("at-infl", "Sketch inflection points", o.inflections, |o, v| o.inflections = v, cx))
                    .child(check("at-minr", "Minimum radius", o.min_radius, |o, v| o.min_radius = v, cx))
                    .child(check("at-ext", "Extend across face", o.extend, |o, v| o.extend = v, cx))
                    .child(check("at-cp", "Control point grids", o.control_points, |o, v| o.control_points = v, cx));
                if o.min_radius
                    && let Some((r, _)) = t.result.as_ref().and_then(|r| r.min_radius)
                {
                    body = body.child(div().text_size(px(12.)).text_color(pal.text).child(format!("Minimum radius: {}", fmt_len(r))));
                }
            }
            ToolKind::Dihedral => {
                let (lo, hi) = t.result.as_ref().and_then(|r| r.dihedral).unwrap_or((0.0, 0.0));
                body = body
                    .child(
                        h_flex()
                            .gap_2()
                            .text_size(px(12.))
                            .child(check("at-max", "Max", o.show_max, |o, v| o.show_max = v, cx))
                            .child(check("at-min", "Min", !o.show_max, |o, v| o.show_max = !v, cx)),
                    )
                    .child(div().text_size(px(12.)).text_color(pal.text).child(if t.selection.is_empty() { String::new() } else if o.show_max { format!("Max: {hi:.2} deg") } else { format!("Min: {lo:.2} deg") }))
                    .child(check("at-thr", "Enable threshold display", o.threshold, |o, v| o.threshold = v, cx));
                if o.threshold {
                    body = body
                        .child(stepper("at-hi", "High threshold", format!("{:.0} deg", o.high_deg), |o, d| o.high_deg = (o.high_deg + d as f64).clamp(0.0, 180.0), cx))
                        .child(stepper("at-lo", "Low threshold", format!("{:.0} deg", o.low_deg), |o, d| o.low_deg = (o.low_deg + d as f64).clamp(0.0, 180.0), cx));
                }
            }
        }
        body = body.child(h_flex().gap_2().text_size(px(12.)).text_color(pal.text_muted).child("Scale").child(div().flex_1().child(Slider::new(&t.slider))));
        let dialog = v_flex()
            .id("analysis-tool")
            .occlude()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .w(px(220.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .child(
                h_flex()
                    .h(px(29.))
                    .pl(px(6.))
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(kind.title()))
                    .child(
                        div()
                            .id("at-close")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child(Icon::new(IconName::Close).with_size(px(16.)).text_color(pal.cancel))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.close_analysis_tool(cx);
                            })),
                    ),
            )
            .child(body)
            .into_any_element();
        let mut out = vec![dialog];
        // "R 12.5 mm" beside the minimum-radius point.
        if o.min_radius
            && kind == ToolKind::CurveSurface
            && let Some((r, at)) = t.result.as_ref().and_then(|r| r.min_radius)
            && let Some(s) = self.viewport.read(cx).project(at)
        {
            out.push(div().absolute().left(px(s.x as f32 + 8.)).top(px(s.y as f32 - 8.)).px(px(3.)).bg(gpui_kit::rgb(0xffffff)).border_1().border_color(gpui_kit::rgb(0xd228c8)).text_size(px(12.)).text_color(pal.text).child(format!("R {}", fmt_len(r))).into_any_element());
        }
        out
    }
}

fn keep_min(cur: &mut Option<(f64, DVec3)>, new: Option<(f64, DVec3)>) {
    if let Some(n) = new
        && cur.is_none_or(|c| n.0 < c.0)
    {
        *cur = Some(n);
    }
}

/// Inflection points of a comb: where the tooth flips to the other side.
fn inflections(c: &Comb) -> Vec<DVec3> {
    let mut out = Vec::new();
    for i in 1..c.teeth.len().saturating_sub(1) {
        let (a, b) = (c.teeth[i - 1], c.teeth[i + 1]);
        if a.length() > 0.0 && b.length() > 0.0 && a.dot(b) < 0.0 && c.teeth[i].length() < a.length().max(b.length()) {
            out.push(c.base[i]);
        }
    }
    out
}

/// Control polygon of a B-spline part edge.
fn edge_poles(ctx: &noshape_model::regen::context::Context, r: &Reference) -> Option<Vec<DVec3>> {
    let Reference::Edge(n) = r else { return None };
    ctx.bodies.iter().find_map(|b| {
        let e = *b.find_edges(n).first()?;
        let s = b.shape.sub_shape(noshape_kernel::ShapeType::Edge, e).ok()?;
        s.edge_bspline().ok().map(|d| d.poles)
    })
}
