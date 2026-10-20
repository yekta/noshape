//! Analysis tools › Curve/surface analysis… in an assembly (help
//! View__curve_surface_analysis: "Available in: Sketch, Part Studio,
//! Assembly"): the Part Studio's dialog (Entities to analyze, Curvature
//! combs, Show U / V curves with their counts, Sketch inflection points,
//! Minimum radius, Extend across face, Control point grids) for edges and
//! faces of the instances, each evaluated in its Part Studio and drawn where
//! the instance sits.

use super::*;
use noshape_model::analysis::Comb;
use noshape_model::assembly::eval::InstancePath;
use noshape_render::{LineData, LineSegment};

/// Comb colors (as the Part Studio's): edge combs green, U curves red, V
/// curves blue, the envelope dark.
const COMB: [u8; 4] = [40, 170, 60, 255];
const U_CURVES: [u8; 4] = [220, 50, 40, 255];
const V_CURVES: [u8; 4] = [40, 90, 220, 255];
const ENVELOPE: [u8; 4] = [60, 60, 60, 255];

/// Dialog options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CurveOptions {
    pub combs: bool,
    pub show_u: bool,
    pub u_curves: usize,
    pub show_v: bool,
    pub v_curves: usize,
    pub inflections: bool,
    pub min_radius: bool,
    pub extend: bool,
    pub control_points: bool,
}

impl Default for CurveOptions {
    fn default() -> Self {
        Self { combs: true, show_u: true, u_curves: 8, show_v: true, v_curves: 8, inflections: true, min_radius: false, extend: false, control_points: false }
    }
}

/// The open Curve/surface analysis dialog: its picks and the evaluated
/// combs (world coordinates).
#[derive(Default)]
pub(crate) struct CurveDialog {
    pub picks: Vec<(InstancePath, Reference)>,
    pub options: CurveOptions,
    combs: Vec<(Comb, [u8; 4])>,
    polylines: Vec<Vec<DVec3>>,
    inflection_points: Vec<DVec3>,
    min_radius: Option<(f64, DVec3)>,
}

impl AssemblyView {
    pub(super) fn open_curve_surface(&mut self, cx: &mut Context<Self>) {
        self.ui.curve_dialog = Some(CurveDialog::default());
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn close_curve_surface(&mut self, cx: &mut Context<Self>) {
        if self.ui.curve_dialog.take().is_some() {
            self.update_capture(cx);
            self.refresh_scene(false, cx);
            cx.notify();
        }
    }

    /// A viewport click while the dialog is open: toggles an edge or face.
    pub(super) fn curve_pick(&mut self, t: AsmPickTarget, cx: &mut Context<Self>) {
        let Some(d) = &mut self.ui.curve_dialog else { return };
        if !matches!(t.kind, PickKind::Face | PickKind::Edge) {
            return;
        }
        let key = (t.path.clone(), t.reference.clone());
        match d.picks.iter().position(|x| *x == key) {
            Some(i) => {
                d.picks.remove(i);
            }
            None => d.picks.push(key),
        }
        self.recompute_curve_surface(cx);
    }

    /// Evaluate the combs (kernel calls here only, not per frame).
    fn recompute_curve_surface(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.ui.curve_dialog else { return };
        let (picks, o) = (d.picks.clone(), d.options);
        let (mut combs, mut polylines, mut infl, mut min_radius) = (Vec::new(), Vec::new(), Vec::new(), None);
        let samples = 32;
        for (path, r) in &picks {
            let Some(occ) = self.result.occurrences.iter().find(|x| x.path == *path && (x.face_refs.contains(r) || x.edge_refs.contains(r))) else { continue };
            let ctx = &occ.context;
            let m = occ.transform;
            let world = |c: Comb| Comb { base: c.base.iter().map(|p| m.transform_point3(*p)).collect(), teeth: c.teeth.iter().map(|t| m.transform_vector3(*t)).collect() };
            match r {
                Reference::Edge(_) => {
                    if let Some((comb, min)) = noshape_model::analysis::edge_curvature(ctx, r, samples) {
                        if let Some((rad, p)) = min
                            && min_radius.is_none_or(|(cur, _)| rad < cur)
                        {
                            min_radius = Some((rad, m.transform_point3(p)));
                        }
                        let comb = world(comb);
                        if o.inflections && occ.kind == noshape_model::display::BodyKind::Wire {
                            infl.extend(inflection_points(&comb));
                        }
                        if o.combs {
                            combs.push((comb, COMB));
                        }
                    }
                    if o.control_points
                        && let Some(poles) = edge_poles(ctx, r)
                    {
                        polylines.push(poles.into_iter().map(|p| m.transform_point3(p)).collect());
                    }
                }
                Reference::Face(_) => {
                    let mut faces = vec![r.clone()];
                    if o.extend {
                        // Extend across face: the tangent-connected faces too.
                        for g in noshape_model::select::select_related(ctx, r, noshape_model::select::Criterion::TangentFaces) {
                            if !faces.contains(&g) {
                                faces.push(g);
                            }
                        }
                    }
                    for f in &faces {
                        if let Some((along_u, along_v)) = noshape_model::analysis::face_curvature(ctx, f, o.u_curves, o.v_curves, samples) {
                            if o.show_u {
                                combs.extend(along_u.into_iter().map(|c| (world(c), U_CURVES)));
                            }
                            if o.show_v {
                                combs.extend(along_v.into_iter().map(|c| (world(c), V_CURVES)));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(d) = &mut self.ui.curve_dialog {
            d.combs = combs;
            d.polylines = polylines;
            d.inflection_points = infl;
            d.min_radius = min_radius;
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The combs, control polygons and markers, sized to the assembly.
    pub(super) fn curve_surface_lines(&self) -> Option<LineData> {
        let d = self.ui.curve_dialog.as_ref()?;
        let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
        let size = self.result.bounds().map(|(lo, hi)| (hi - lo).length()).unwrap_or(0.1);
        let max = d.combs.iter().map(|(c, _)| c.max_len()).fold(0.0, f64::max);
        let k = if max > 0.0 { size * 0.25 * 0.3 / max } else { 0.0 };
        let mut lines = LineData::default();
        for (comb, color) in &d.combs {
            let mut tips = Vec::with_capacity(comb.base.len());
            for (b, tooth) in comb.base.iter().zip(&comb.teeth) {
                let tip = *b + *tooth * k;
                lines.push_polyline(&[f(*b), f(tip)], *color, 0, 1.0, LineSegment::ON_TOP);
                tips.push(f(tip));
            }
            lines.push_polyline(&tips, ENVELOPE, 0, 1.25, LineSegment::ON_TOP);
        }
        for pl in &d.polylines {
            let pts: Vec<[f32; 3]> = pl.iter().map(|p| f(*p)).collect();
            lines.push_polyline(&pts, ENVELOPE, 0, 1.0, LineSegment::ON_TOP | LineSegment::DASHED);
        }
        // Inflection points and the minimum radius: small crosses.
        let mark = size * 0.01;
        let marks = d.inflection_points.iter().map(|p| (*p, U_CURVES)).chain(d.min_radius.filter(|_| d.options.min_radius).map(|(_, p)| (p, V_CURVES)));
        for (p, c) in marks {
            for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
                lines.push_polyline(&[f(p - axis * mark), f(p + axis * mark)], c, 0, 2.0, LineSegment::ON_TOP);
            }
        }
        Some(lines)
    }

    /// The 220 px dialog (as the Part Studio's Curve/surface analysis).
    pub(super) fn render_curve_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.curve_dialog.as_ref()?;
        let pal = p();
        let o = d.options;
        let names: Vec<String> = {
            let label = self.labeler(cx);
            d.picks.iter().map(|(path, r)| label(&MateConnectorRef::on(path[0], path[1..].to_vec(), r.clone()))).collect()
        };
        let units = self.units(cx);
        let min_text = d.min_radius.filter(|_| o.min_radius).map(|(r, _)| format!("Minimum radius: {}", noshape_units::format_quantity(noshape_units::Quantity::meters(r), &units)));
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut CurveOptions), cx: &mut Context<Self>| {
            crate::os_ui::checkbox(id, label, on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(d) = &mut this.ui.curve_dialog {
                    f(&mut d.options);
                }
                this.recompute_curve_surface(cx);
            }))
        };
        let stepper = |id: &'static str, label: &'static str, value: usize, f: fn(&mut CurveOptions, i32), cx: &mut Context<Self>| {
            let btn = |sid: String, glyph: &'static str, delta: i32, cx: &mut Context<Self>| {
                div().id(SharedString::from(sid)).size(px(20.)).flex().items_center().justify_center().rounded(px(3.)).cursor_pointer().hover(|d| d.bg(pal.panel_hover)).text_size(px(14.)).child(glyph).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.ui.curve_dialog {
                        f(&mut d.options, delta);
                    }
                    this.recompute_curve_surface(cx);
                }))
            };
            h_flex()
                .h(px(28.))
                .pl(px(22.))
                .pr(px(5.))
                .gap(px(4.))
                .text_size(px(13.33))
                .text_color(pal.text_muted)
                .child(div().flex_1().child(label))
                .child(btn(format!("{id}-dec"), "−", -1, cx))
                .child(div().w(px(28.)).text_center().text_color(pal.text).child(value.to_string()))
                .child(btn(format!("{id}-inc"), "+", 1, cx))
        };
        let rows: Vec<AnyElement> = names
            .into_iter()
            .enumerate()
            .map(|(i, n)| {
                crate::os_ui::query_row(SharedString::from(format!("acs-pick-{i}")), n)
                    .child(div().id(SharedString::from(format!("acs-del-{i}"))).px(px(5.)).text_size(px(16.67)).text_color(rgb(0x666666)).cursor_pointer().child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.ui.curve_dialog
                            && i < d.picks.len()
                        {
                            d.picks.remove(i);
                        }
                        this.recompute_curve_surface(cx);
                    })))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .id("acs-dialog")
                .occlude()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .pl(px(6.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Curve/surface analysis"))
                        .child(
                            div()
                                .id("acs-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .tooltip(crate::ui::tooltip("Close (Esc)"))
                                .child(crate::assets::icon("dialog-cancel", 18., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_curve_surface(cx))),
                        ),
                )
                .child(div().px(px(3.)).pt(px(5.)).child(crate::os_ui::query_box("acs-picks", "Entities to analyze", true, rows)))
                .child(check("acs-combs", "Curvature combs", o.combs, |o| o.combs = !o.combs, cx))
                .child(check("acs-u", "Show U curves", o.show_u, |o| o.show_u = !o.show_u, cx))
                .child(stepper("acs-uc", "U curves", o.v_curves, |o, d| o.v_curves = (o.v_curves as i32 + d).clamp(2, 64) as usize, cx))
                .child(check("acs-v", "Show V curves", o.show_v, |o| o.show_v = !o.show_v, cx))
                .child(stepper("acs-vc", "V curves", o.u_curves, |o, d| o.u_curves = (o.u_curves as i32 + d).clamp(2, 64) as usize, cx))
                .child(check("acs-infl", "Sketch inflection points", o.inflections, |o| o.inflections = !o.inflections, cx))
                .child(check("acs-minr", "Minimum radius", o.min_radius, |o| o.min_radius = !o.min_radius, cx))
                .children(min_text.map(|t| div().pl(px(5.)).h(px(24.)).flex().items_center().text_size(px(12.)).text_color(pal.text).child(t)))
                .child(check("acs-ext", "Extend across face", o.extend, |o| o.extend = !o.extend, cx))
                .child(check("acs-cp", "Control point grids", o.control_points, |o| o.control_points = !o.control_points, cx))
                .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(super::props::help_dot()))
                .into_any_element(),
        )
    }
}

/// Inflection points of a comb: where the tooth flips to the other side.
fn inflection_points(c: &Comb) -> Vec<DVec3> {
    let mut out = Vec::new();
    for i in 1..c.teeth.len().saturating_sub(1) {
        let (a, b) = (c.teeth[i - 1], c.teeth[i + 1]);
        if a.length() > 0.0 && b.length() > 0.0 && a.dot(b) < 0.0 && c.teeth[i].length() < a.length().max(b.length()) {
            out.push(c.base[i]);
        }
    }
    out
}

/// Control polygon of a B-spline part edge (Part Studio coordinates).
fn edge_poles(ctx: &noshape_model::regen::context::Context, r: &Reference) -> Option<Vec<DVec3>> {
    let Reference::Edge(n) = r else { return None };
    ctx.bodies.iter().find_map(|b| {
        let e = *b.find_edges(n).first()?;
        let s = b.shape.sub_shape(noshape_kernel::ShapeType::Edge, e).ok()?;
        s.edge_bspline().ok().map(|d| d.poles)
    })
}
