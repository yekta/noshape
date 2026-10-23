//! Onshape's analysis tool dialogs (Analysis tools menu at the bottom
//! right; Shift+C Curve/surface analysis, Shift+D Dihedral analysis): a
//! dialog at the feature dialog position with a selection field, the
//! tool's options and a comb scale slider; the combs are drawn over the
//! model. Selections persist for the next time the tool opens; a
//! preselection replaces them (Onshape).

use super::PartStudioView;
use crate::theme::p;
use glam::DVec3;
use crate::quantity_field::{QuantityEvent, QuantityInput, QuantitySpec};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::analysis::{Comb, Continuity};
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment, PointData, PointSprite};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    CurveSurface,
    Deviation,
    Connection,
    Dihedral,
    Zebra,
    Reflection,
    Thickness,
    Flatten,
    Interference,
    Draft,
}

impl ToolKind {
    /// Tools that change how the parts are drawn: ✓ keeps them on, ✗ (or
    /// Esc) turns them off.
    pub fn is_view_mode(self) -> bool {
        matches!(self, ToolKind::Zebra | ToolKind::Reflection | ToolKind::Thickness | ToolKind::Draft)
    }
}

/// Zebra stripes settings (they stay on after the dialog is accepted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZebraSettings {
    pub stripes: u32,
    pub flip: bool,
    pub show_edges: bool,
}

impl Default for ZebraSettings {
    fn default() -> Self {
        Self { stripes: 35, flip: false, show_edges: true }
    }
}

impl ToolKind {
    pub fn title(self) -> &'static str {
        match self {
            ToolKind::CurveSurface => "Curve/surface analysis",
            ToolKind::Deviation => "Deviation analysis",
            ToolKind::Connection => "Connection analysis",
            ToolKind::Dihedral => "Dihedral analysis",
            ToolKind::Zebra => "Zebra stripes",
            ToolKind::Reflection => "Reflection analysis",
            ToolKind::Thickness => "Thickness analysis",
            ToolKind::Flatten => "Flatten surfaces",
            ToolKind::Interference => "Interference detection",
            ToolKind::Draft => "Draft analysis",
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
    /// Deviation: Surface tab (else Curve).
    pub surface: bool,
    /// Connection: which classes are shown (No connection, G0…G3).
    pub show_class: [bool; 5],
    pub tolerances: noshape_model::analysis::ContinuityTolerances,
    pub zebra: ZebraSettings,
    pub reflection: super::reflection::ReflectionSettings,
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self { combs: true, show_u: true, show_v: true, u_curves: 8, v_curves: 8, inflections: true, min_radius: false, extend: false, control_points: false, show_max: true, threshold: false, high_deg: 180.0, low_deg: 0.0, scale: 0.3, surface: false, show_class: [false, true, true, true, true], tolerances: Default::default(), zebra: Default::default(), reflection: Default::default() }
    }
}

/// Computed geometry of the open tool.
#[derive(Clone, Default)]
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
    /// Deviation minimum and maximum (distance, point on 1, point on 2).
    pub deviation: Option<(noshape_model::analysis::Extremum, noshape_model::analysis::Extremum)>,
    /// Interference detection: the overlaps (with the two parts' names) and their red meshes.
    pub interferences: Vec<(noshape_model::interference::Interference, String, String)>,
    pub interference_meshes: Vec<noshape_render::MeshHandle>,
    /// Connection: curve endpoints, face edge segments and the boundaries
    /// of faces that connect to nothing.
    pub joints: Vec<(DVec3, Continuity)>,
    pub faces: noshape_model::analysis::FaceConnections,
}

pub struct AnalysisTool {
    pub kind: ToolKind,
    pub selection: Vec<Reference>,
    /// Deviation: the second chain / surface, and whether its field is active.
    pub selection2: Vec<Reference>,
    pub second_active: bool,
    pub options: ToolOptions,
    pub result: Option<ToolResult>,
    slider: Entity<SliderState>,
    _slider_sub: Subscription,
    /// Connection: the G1, G2 and G3 tolerance fields; Zebra: stripe count.
    tolerance_inputs: Vec<Entity<QuantityInput>>,
    _input_subs: Vec<Subscription>,
    /// Interference detection: its top in the viewport, fixed when it opens
    /// (live: a resize keeps the top and the right offset).
    pub anchor_top: Option<f32>,
    /// Interference detection: the model changed while it was open; the
    /// Refresh row shows and the old result stays until it is clicked.
    pub stale: bool,
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

/// Connection analysis colors (Onshape): (main, light) per class, from
/// No connection to G3.
const CONTINUITY: [([u8; 4], [u8; 4]); 5] = [
    ([0x93, 0x22, 0x29, 255], [0x93, 0x22, 0x29, 255]),
    ([0x98, 0x71, 0x07, 255], [0xfc, 0xda, 0x7f, 255]),
    ([0x02, 0x85, 0x02, 255], [0xa2, 0xdb, 0x7d, 255]),
    ([0x44, 0x01, 0x54, 255], [0xb6, 0x89, 0xbe, 255]),
    ([0x46, 0x7b, 0xb6, 255], [0xd0, 0xdb, 0xe5, 255]),
];

/// Edge color of a class: G0 edges are drawn in the light yellow, the
/// others in their main color (Onshape).
fn continuity_edge_color(c: Continuity) -> [u8; 4] {
    match c {
        Continuity::G0 => CONTINUITY[1].1,
        c => CONTINUITY[c as usize].0,
    }
}

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
        let pre: Vec<Reference> = self.selection.iter().filter(|r| self.analysis_accepts(kind, r)).filter_map(|r| self.tool_entity(kind, r)).collect();
        // The tool takes the preselection (it doesn't stay selected).
        let had_pre = !pre.is_empty();
        let mut selection = if pre.is_empty() { self.ui.tool_memory.selection.get(key).cloned().unwrap_or_default() } else { pre };
        if kind == ToolKind::Thickness {
            // Edit thickness analysis: the analyzed parts.
            let t = self.ui.thickness.get_or_insert_with(Default::default);
            if !t.parts.is_empty() {
                selection = t.parts.iter().map(|b| Reference::Body(b.clone())).collect();
            }
        }
        // In a sketch, Shift+C takes every curve of the sketch.
        if kind == ToolKind::CurveSurface
            && let Some(sk) = self.sketch.as_ref().map(|s| s.feature)
            && let Some(sd) = self.display.sketches.iter().find(|s| s.feature == sk)
        {
            selection = sd.curves.iter().map(|c| Reference::SketchEntity { sketch: sk, entity: c.entity }).collect();
        }
        let mut options = self.ui.tool_memory.options.get(key).cloned().unwrap_or_default();
        if kind == ToolKind::Zebra {
            options.zebra = self.ui.zebra.unwrap_or(options.zebra);
            // Zebra stripes and the curvature color map replace each other.
            if matches!(self.ui.analysis, Some(super::analysis::Analysis::Curvature)) {
                self.ui.analysis = None;
            }
            self.ui.reflection = None;
        }
        if kind == ToolKind::Reflection {
            options.reflection = self.ui.reflection.clone().unwrap_or(options.reflection);
            self.ui.zebra = None;
        }
        let slider = cx.new(|_| SliderState::new().min(0.05).max(1.0).step(0.01).default_value(options.scale));
        let sub = cx.subscribe_in(&slider, window, |this: &mut Self, _, ev: &SliderEvent, _, cx| {
            let (SliderEvent::Change(v) | SliderEvent::Release(v)) = ev;
            if let Some(t) = &mut this.ui.analysis_tool {
                t.options.scale = v.start();
            }
            this.refresh_scene(false, cx);
            cx.notify();
        });
        // Deviation with two curves preselected: one per chain (Onshape).
        let mut selection2 = Vec::new();
        if kind == ToolKind::Deviation && selection.len() == 2 {
            selection2 = vec![selection.remove(1)];
        }
        let second_active = kind == ToolKind::Deviation && !selection.is_empty() && selection2.is_empty();
        let (mut tolerance_inputs, mut input_subs) = (Vec::new(), Vec::new());
        // Onshape's number fields (quantity_field): units appended, invalid
        // values marked in the field, ↑ / ↓ steps (live).
        let units = self.doc.read(cx).session.doc.settings.units;
        let vars = self.display.variables.clone();
        let qty = |text: String, spec: QuantitySpec, window: &mut Window, cx: &mut Context<Self>| {
            let vars = vars.clone();
            cx.new(|cx| {
                let mut q = QuantityInput::new(text, spec, units, window, cx);
                q.set_variables(vars, cx);
                q
            })
        };
        if kind == ToolKind::Zebra {
            // Live: a count of 1–100, no expressions ("1 in", "1+1" invalid,
            // hover "Enter an integer.").
            let st = qty(options.zebra.stripes.to_string(), QuantitySpec::count().no_expressions().range(Some(1.), Some(100.)).label("Stripe count"), window, cx);
            input_subs.push(cx.subscribe(&st, |this: &mut Self, st, ev: &QuantityEvent, cx| {
                if !matches!(ev, QuantityEvent::Committed(_)) {
                    return;
                }
                if let Some(n) = st.read(cx).value() {
                    if let Some(t) = &mut this.ui.analysis_tool {
                        t.options.zebra.stripes = n.round() as u32;
                    }
                    this.rebuild_scene(cx);
                }
            }));
            tolerance_inputs.push(st);
        }
        if kind == ToolKind::Reflection {
            let st = qty(format!("{} deg", options.reflection.rotation_deg), QuantitySpec::quantity(noshape_units::Dim::ANGLE).label("Rotation angle"), window, cx);
            input_subs.push(cx.subscribe(&st, |this: &mut Self, st, ev: &QuantityEvent, cx| {
                if !matches!(ev, QuantityEvent::Committed(_)) {
                    return;
                }
                if let Some(v) = st.read(cx).value() {
                    if let Some(t) = &mut this.ui.analysis_tool {
                        t.options.reflection.rotation_deg = v.to_degrees();
                    }
                    this.rebuild_scene(cx);
                }
            }));
            tolerance_inputs.push(st);
        }
        if kind == ToolKind::Connection {
            let t = options.tolerances;
            let g1 = QuantitySpec::quantity(noshape_units::Dim::ANGLE).range(Some(1e-9), None).text_px(12.);
            let g = QuantitySpec::quantity(noshape_units::Dim::UNITLESS).range(Some(1e-12), None).text_px(12.);
            for (i, (v, spec)) in [(format!("{} deg", t.g1_deg), g1), (t.g2.to_string(), g.clone()), (t.g3.to_string(), g)].into_iter().enumerate() {
                let st = qty(v, spec, window, cx);
                input_subs.push(cx.subscribe(&st, move |this: &mut Self, st, ev: &QuantityEvent, cx| {
                    if !matches!(ev, QuantityEvent::Committed(_)) {
                        return;
                    }
                    let Some(v) = st.read(cx).value() else { return };
                    let Some(t) = &mut this.ui.analysis_tool else { return };
                    let tol = &mut t.options.tolerances;
                    *[&mut tol.g1_deg, &mut tol.g2, &mut tol.g3][i] = if i == 0 { v.to_degrees() } else { v };
                    this.recompute_analysis_tool(cx);
                }));
                tolerance_inputs.push(st);
            }
        }
        if kind == ToolKind::Flatten {
            self.open_flatten(window, cx);
        }
        if kind == ToolKind::Draft {
            self.open_draft(window, cx);
        }
        if had_pre || matches!(kind, ToolKind::Draft | ToolKind::Flatten) {
            self.selection.clear();
        }
        // Interference detection keeps the top it opened at (live psb 10-19 i).
        let anchor_top = (kind == ToolKind::Interference).then(|| ((self.viewport.read(cx).size().y as f32) - 401.).max(0.));
        self.ui.analysis_tool = Some(AnalysisTool { kind, selection, selection2, second_active, options, result: None, slider, _slider_sub: sub, tolerance_inputs, _input_subs: input_subs, anchor_top, stale: false });
        self.recompute_analysis_tool(cx);
        if kind.is_view_mode() {
            self.rebuild_scene(cx);
        }
    }

    /// Close the open tool (✗, Esc, another tool); closing Zebra stripes
    /// this way turns the stripes off.
    pub(super) fn close_analysis_tool(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(t) = self.ui.analysis_tool.take() else { return false };
        self.ui.env_picker = None;
        self.ui.flatten = None;
        match t.kind {
            ToolKind::Zebra => self.ui.zebra = None,
            ToolKind::Reflection => self.ui.reflection = None,
            ToolKind::Thickness => self.ui.thickness = None,
            ToolKind::Draft => {
                self.ui.draft = None;
                self.ui.analysis = None;
            }
            _ => {}
        }
        if t.kind.is_view_mode() || t.kind == ToolKind::Interference {
            self.rebuild_scene(cx);
        }
        let key = t.kind.title();
        self.ui.tool_memory.selection.insert(key, t.selection);
        self.ui.tool_memory.options.insert(key, t.options);
        self.refresh_scene(false, cx);
        cx.notify();
        true
    }

    /// ✓ / Enter in the Zebra stripes or Reflection analysis dialog: the
    /// view stays that way.
    pub(super) fn accept_view_tool(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.ui.analysis_tool.as_ref().is_some_and(|t| t.kind.is_view_mode()) {
            return false;
        }
        let Some(t) = self.ui.analysis_tool.take() else { return false };
        match t.kind {
            ToolKind::Zebra => self.ui.zebra = Some(t.options.zebra),
            ToolKind::Reflection => self.ui.reflection = Some(t.options.reflection.clone()),
            _ => {}
        }
        self.ui.env_picker = None;
        self.ui.draft = None;
        self.ui.tool_memory.options.insert(t.kind.title(), t.options);
        self.rebuild_scene(cx);
        true
    }

    /// The zebra stripes on display: the open dialog's, else accepted ones.
    pub(super) fn active_zebra(&self) -> Option<ZebraSettings> {
        match &self.ui.analysis_tool {
            Some(t) if t.kind == ToolKind::Zebra => Some(t.options.zebra),
            _ => self.ui.zebra,
        }
    }

    fn analysis_accepts(&self, kind: ToolKind, r: &Reference) -> bool {
        let surface = self.ui.analysis_tool.as_ref().is_some_and(|t| t.options.surface);
        match kind {
            ToolKind::CurveSurface => matches!(r, Reference::Edge(_) | Reference::Face(_) | Reference::SketchEntity { .. }),
            ToolKind::Deviation if surface => matches!(r, Reference::Face(_)),
            ToolKind::Deviation => matches!(r, Reference::Edge(_) | Reference::SketchEntity { .. }),
            ToolKind::Connection => matches!(r, Reference::Edge(_) | Reference::Face(_) | Reference::SketchEntity { .. }),
            ToolKind::Dihedral => matches!(r, Reference::Edge(_)),
            ToolKind::Zebra | ToolKind::Reflection => false,
            ToolKind::Thickness | ToolKind::Interference => matches!(r, Reference::Body(_) | Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_)),
            ToolKind::Flatten | ToolKind::Draft => true,
        }
    }

    /// What a pick puts in the tool's field: Thickness analysis takes the
    /// whole part.
    fn tool_entity(&self, kind: ToolKind, r: &Reference) -> Option<Reference> {
        if !matches!(kind, ToolKind::Thickness | ToolKind::Interference) || matches!(r, Reference::Body(_)) {
            return Some(r.clone());
        }
        self.display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r)).map(|b| Reference::Body(b.id.clone()))
    }

    /// A viewport pick while a tool is open toggles the entity in its field.
    pub(super) fn analysis_pick(&mut self, r: &Reference, cx: &mut Context<Self>) -> bool {
        let Some(kind) = self.ui.analysis_tool.as_ref().map(|t| t.kind) else { return false };
        if kind == ToolKind::Flatten {
            self.flatten_pick(r, cx);
            return true;
        }
        if kind == ToolKind::Draft {
            self.draft_pick(r, cx);
            return true;
        }
        if !self.analysis_accepts(kind, r) {
            let msg = match kind {
                ToolKind::Dihedral => "Select edges",
                ToolKind::Deviation if self.ui.analysis_tool.as_ref().is_some_and(|t| t.options.surface) => "Select faces",
                ToolKind::Deviation => "Select curves",
                ToolKind::CurveSurface => "Select edges, faces or sketch curves",
                ToolKind::Connection => "Select curves or faces",
                ToolKind::Thickness | ToolKind::Interference => "Select parts",
                ToolKind::Zebra | ToolKind::Reflection | ToolKind::Flatten | ToolKind::Draft => return false,
            };
            self.flash_info(msg.into(), cx);
            return true;
        }
        let Some(r) = &self.tool_entity(kind, r) else { return true };
        if let Some(t) = &mut self.ui.analysis_tool {
            let list = if t.second_active { &mut t.selection2 } else { &mut t.selection };
            match list.iter().position(|x| x == r) {
                Some(i) => {
                    list.remove(i);
                }
                None => list.push(r.clone()),
            }
        }
        self.recompute_analysis_tool(cx);
        true
    }

    /// Evaluate the combs for the selection and options (kernel calls happen
    /// here only, not per frame).
    pub(super) fn recompute_analysis_tool(&mut self, cx: &mut Context<Self>) {
        let Some(t) = &self.ui.analysis_tool else { return };
        let (kind, sel, sel2, o) = (t.kind, t.selection.clone(), t.selection2.clone(), t.options.clone());
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
                ToolKind::Deviation if o.surface => {
                    let pts: Vec<DVec3> = sel.iter().flat_map(|r| self.face_triangles(r)).flatten().collect();
                    let tris: Vec<[DVec3; 3]> = sel2.iter().flat_map(|r| self.face_triangles(r)).collect();
                    res.deviation = noshape_model::analysis::surface_deviation(&pts, &tris);
                }
                ToolKind::Deviation => {
                    let a = densify(&sel.iter().flat_map(|r| self.curve_points(r)).collect::<Vec<_>>(), 400);
                    let b: Vec<DVec3> = sel2.iter().flat_map(|r| self.curve_points(r)).collect();
                    res.deviation = noshape_model::analysis::curve_deviation(&a, &b);
                }
                ToolKind::Connection => {
                    // Curves meet at their endpoints; faces along shared edges.
                    let curves: Vec<Vec<DVec3>> = sel
                        .iter()
                        .filter_map(|r| match r {
                            Reference::Edge(_) => noshape_model::analysis::edge_polyline(&ctx, r, 64),
                            Reference::SketchEntity { .. } => Some(self.curve_points(r)),
                            _ => None,
                        })
                        .collect();
                    res.joints = noshape_model::analysis::curve_connections(&curves, &o.tolerances);
                    let faces: Vec<Reference> = sel.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect();
                    res.faces = noshape_model::analysis::face_connections(&ctx, &faces, samples, &o.tolerances);
                }
                ToolKind::Zebra | ToolKind::Reflection | ToolKind::Flatten | ToolKind::Draft => {}
                ToolKind::Interference => {
                    let ids: Vec<noshape_model::reference::BodyId> = sel.iter().filter_map(|r| if let Reference::Body(b) = r { Some(b.clone()) } else { None }).collect();
                    let bodies: Vec<&noshape_model::regen::context::Body> = ids.iter().filter_map(|id| ctx.bodies.iter().find(|b| b.id == *id).map(|b| &**b)).collect();
                    let shapes: Vec<noshape_kernel::Shape> = bodies.iter().map(|b| b.shape.clone()).collect();
                    let up = self.gpu.uploader();
                    for i in noshape_model::interference::interferences(&shapes) {
                        let pick = vec![0u32; i.positions.len()];
                        res.interference_meshes.push(up.upload_mesh_parts(&i.positions, &i.normals, &pick, &i.indices));
                        let (a, b) = (bodies[i.a].name.clone(), bodies[i.b].name.clone());
                        res.interferences.push((i, a, b));
                    }
                }
                ToolKind::Thickness => {
                    let parts: Vec<noshape_model::reference::BodyId> = sel.iter().filter_map(|r| if let Reference::Body(b) = r { Some(b.clone()) } else { None }).collect();
                    let t = self.ui.thickness.get_or_insert_with(Default::default);
                    if t.parts != parts {
                        t.parts = parts;
                        self.recompute_thickness(cx);
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
        if kind == ToolKind::Interference {
            self.ui.interference_hover = None;
            self.ui.interference_picked = None;
            self.rebuild_scene(cx);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Combs, envelopes, control polygons and markers over the model.
    pub(super) fn analysis_tool_overlay(&self) -> Vec<Instance> {
        if let Some(s) = &self.ui.flatten {
            return s.drawn.clone();
        }
        if self.ui.draft.is_some() {
            return self.draft_arrow();
        }
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
        // Deviation: minimum (blue) and maximum (red) between the two.
        let mut dev_pts = Vec::new();
        if let Some((min, max)) = res.deviation {
            for ((_, a, b), c) in [(min, V_CURVES), (max, U_CURVES)] {
                lines.push_polyline(&[f(a), f(b)], c, 0, 2.0, LineSegment::ON_TOP);
                dev_pts.push((a, c));
                dev_pts.push((b, c));
            }
        }
        // Connection: edges colored by continuity, unconnected faces outlined.
        let shown = |c: Continuity| t.options.show_class[c as usize];
        for ([a, b], c) in res.faces.segments.iter().filter(|s| shown(s.1)) {
            lines.push_polyline(&[f(*a), f(*b)], continuity_edge_color(*c), 0, 3.0, LineSegment::ON_TOP);
        }
        if shown(Continuity::None) {
            for pl in &res.faces.unconnected {
                let pts: Vec<[f32; 3]> = pl.iter().map(|p| f(*p)).collect();
                lines.push_polyline(&pts, CONTINUITY[0].0, 0, 3.0, LineSegment::ON_TOP);
            }
        }
        // Interference detection: the hovered overlap's bounding box.
        if let Some((i, _, _)) = self.ui.interference_hover.and_then(|k| res.interferences.get(k)) {
            let (lo, hi) = i.bounds;
            let c = |k: usize| DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z });
            for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                // Live analysis/83: the query-hover yellow #f2ff80, ~6 px.
                lines.push_polyline(&[f(c(a)), f(c(b))], [242, 255, 128, 255], 0, 6.0, LineSegment::ON_TOP);
            }
        }
        let up = self.gpu.uploader();
        let mut out = vec![Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(35)];
        // The interfering material, red (seen through the translucent parts).
        // Live analysis/82–83 shows it vivid: drawn after the see-through
        // parts (its faces are coplanar with theirs).
        for m in &res.interference_meshes {
            out.push(Instance::new(Drawable::Mesh(m.clone())).with_color(crate::theme::lin(0xc71010)).over_transparent().unpickable().layer(34));
        }
        // Curve joints: a dark ring around a light center; free ends a small red dot.
        let (mut rings, mut centers) = (PointData::default(), PointData::default());
        for (p, c) in res.joints.iter().filter(|j| shown(j.1)) {
            let (main, light) = CONTINUITY[*c as usize];
            let ring = if *c == Continuity::None { 7.0 } else { 11.0 };
            rings.points.push(PointSprite { position: f(*p), color: main, pick_id: 0, size: ring, style: LineSegment::ON_TOP, _pad: 0 });
            if *c != Continuity::None {
                centers.points.push(PointSprite { position: f(*p), color: if *c == Continuity::G1 { main } else { light }, pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
            }
        }
        for (data, layer) in [(rings, 37), (centers, 38)] {
            if !data.points.is_empty() {
                out.push(Instance::new(Drawable::Points(up.upload_points(&data))).per_primitive_color().unpickable().ignore_clip().layer(layer));
            }
        }
        let mut pts = PointData::default();
        for (p, c) in dev_pts {
            pts.points.push(PointSprite { position: f(p), color: c, pick_id: 0, size: 8.0, style: LineSegment::ON_TOP, _pad: 0 });
        }
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
    /// The tool's readouts at model points ("Min: …", "Max: …", "R …"), drawn
    /// in the 3D frame: the value boxes `render_analysis_tool` draws with
    /// NOSHAPE_GPUI_LABELS.
    pub(super) fn analysis_labels(&self, window: &Window, cx: &App) -> Vec<noshape_render::overlay::Label> {
        let Some(t) = &self.ui.analysis_tool else { return Vec::new() };
        let pal = p();
        let units = self.doc.read(cx).session.doc.settings.units;
        let fmt_len = |v: f64| noshape_units::format_quantity(noshape_units::Quantity::new(v, noshape_units::Dim::LENGTH), &units);
        let value = |text: &str, border: u32, offset: [f32; 2], at: DVec3| {
            let b = crate::overlay_labels::ValueBox { text, text_size: 12., text_color: pal.text.into(), background: gpui_kit::rgb(0xffffff).into(), border: Some(gpui_kit::rgb(border).into()), pad_x: 3., offset };
            crate::overlay_labels::value_box(window, cx, &b, at)
        };
        let mut out = Vec::new();
        if let Some((min, max)) = t.result.as_ref().and_then(|r| r.deviation) {
            for ((d, a, b), label, color) in [(min, "Min", 0x2858dcu32), (max, "Max", 0xdc3228)] {
                let off = 10.0 + 120.0 * t.options.scale;
                out.extend(value(&format!("{label}: {}", fmt_len(d)), color, [off, -10.], (a + b) * 0.5));
            }
        }
        if t.options.min_radius
            && t.kind == ToolKind::CurveSurface
            && let Some((r, at)) = t.result.as_ref().and_then(|r| r.min_radius)
        {
            out.extend(value(&format!("R {}", fmt_len(r)), 0xd228c8, [8., -8.], at));
        }
        out
    }

    pub(super) fn render_analysis_tool(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(t) = &self.ui.analysis_tool else { return Vec::new() };
        let pal = p();
        let units = self.doc.read(cx).session.doc.settings.units;
        let labeler = self.labeler(cx);
        let (kind, o, stale) = (t.kind, t.options.clone(), t.stale);
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
        // A selection field: rows with × on hover; clicking it makes it the
        // field picks go to (Deviation has two).
        let field = |id: &'static str, caption: &'static str, list: &[Reference], second: bool, active: bool, cx: &mut Context<Self>| {
            let rows: Vec<AnyElement> = list
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let r2 = r.clone();
                    // Live (dom/dialog-interference-results.txt): 24 px rows,
                    // 13.33 px #555, 6 px in, "×" 16.67 px #666 always shown.
                    h_flex()
                        .id(SharedString::from(format!("{id}-{i}")))
                        .h(px(24.))
                        .pl(px(6.))
                        .text_size(px(13.33))
                        .text_color(gpui_kit::rgb(0x555555))
                        .child(div().flex_1().truncate().child(labeler(r)))
                        .child(
                            div()
                                .id(SharedString::from(format!("{id}-x-{i}")))
                                .w(px(20.))
                                .flex()
                                .justify_center()
                                .cursor_pointer()
                                .text_size(px(16.67))
                                .text_color(gpui_kit::rgb(0x666666))
                                .child("×")
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(t) = &mut this.ui.analysis_tool {
                                        t.selection.retain(|x| *x != r2);
                                        t.selection2.retain(|x| *x != r2);
                                    }
                                    this.recompute_analysis_tool(cx);
                                })),
                        )
                        .into_any_element()
                })
                .collect();
            // An Onshape query list: 1 px #1651b0 on #def1fb while active
            // (#d7d7d7 otherwise), radius 2; the label 12 px, 6/6/2 px padding.
            let empty = rows.is_empty();
            v_flex()
                .id(id)
                .min_h(px(26.))
                .rounded(px(2.))
                .border_1()
                .border_color(if active { pal.accent } else { gpui_kit::rgb(0xd7d7d7) })
                .when(active, |d| d.bg(crate::theme::p().query_active))
                .cursor_pointer()
                .child(div().pt(px(6.)).px(px(6.)).pb(px(if empty { 5. } else { 2. })).text_size(px(12.)).line_height(px(13.33)).text_color(gpui_kit::rgb(if active { 0x555555 } else { 0x666666 })).child(caption))
                .children(rows)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(t) = &mut this.ui.analysis_tool {
                        t.second_active = second;
                    }
                    cx.notify();
                }))
        };
        let mut body = v_flex().p(px(4.)).gap(px(6.));
        if kind == ToolKind::Deviation {
            // Curve | Surface tabs.
            let tab = |label: &'static str, surface: bool, on: bool, cx: &mut Context<Self>| {
                div()
                    .id(label)
                    .flex_1()
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_1().border_color(pal.accent))
                    .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(t) = &mut this.ui.analysis_tool
                            && t.options.surface != surface
                        {
                            t.options.surface = surface;
                            t.selection.clear();
                            t.selection2.clear();
                            t.second_active = false;
                        }
                        this.recompute_analysis_tool(cx);
                    }))
            };
            let (c1, c2) = if o.surface { ("Surface 1", "Surface 2") } else { ("Chain 1", "Chain 2") };
            body = body
                .child(h_flex().child(tab("Curve", false, !o.surface, cx)).child(tab("Surface", true, o.surface, cx)))
                .child(field("at-sel", c1, &t.selection, false, !t.second_active, cx))
                .child(field("at-sel2", c2, &t.selection2, true, t.second_active, cx));
            if let Some((min, max)) = t.result.as_ref().and_then(|r| r.deviation) {
                body = body
                    .child(div().text_size(px(12.)).text_color(pal.text).child(format!("Min: {}", fmt_len(min.0))))
                    .child(div().text_size(px(12.)).text_color(pal.text).child(format!("Max: {}", fmt_len(max.0))));
            }
        } else if kind == ToolKind::Flatten {
            if let Some(b) = self.render_flatten_body(cx) {
                body = body.child(b);
            }
        } else if kind == ToolKind::Draft {
            if let Some(b) = self.render_draft_body(cx) {
                body = body.child(b);
            }
        } else if !matches!(kind, ToolKind::Zebra | ToolKind::Reflection) {
            let caption = match kind {
                ToolKind::Dihedral => "Edges to analyze",
                ToolKind::Connection => "Entities",
                ToolKind::Thickness => "Parts to analyze",
                ToolKind::Interference => "Parts to analyze",
                _ => "Entities to analyze",
            };
            body = body.child(field("at-sel", caption, &t.selection, false, true, cx));
        }
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
            ToolKind::Deviation | ToolKind::Thickness | ToolKind::Flatten | ToolKind::Draft => {}
            ToolKind::Interference => {
                // Interferences: one row per overlapping pair; hovering a row
                // shows its bounding box and size.
                // Live (analysis/81–84, dom/dialog-interference-results.txt): a
                // read-only query list "Interferences" (#d7d7d7, #666), one
                // 24 px row "Part 1 and Part 2" per pair with the overlap's
                // "L x W x H" as tooltip; hover outlines it, a click makes the
                // row bold and zooms to it; "No interferences" otherwise.
                let rows = t.result.as_ref().map(|r| r.interferences.clone()).unwrap_or_default();
                let picked = self.ui.interference_picked;
                let row = |id: SharedString, text: String| h_flex().id(id).h(px(24.)).pl(px(6.)).pr(px(20.)).text_size(px(13.33)).text_color(gpui_kit::rgb(0x666666)).child(div().flex_1().truncate().child(text));
                let mut list = v_flex()
                    .rounded(px(2.))
                    .border_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .child(div().pt(px(6.)).px(px(6.)).pb(px(2.)).text_size(px(12.)).line_height(px(13.33)).text_color(gpui_kit::rgb(0x666666)).child("Interferences"));
                if rows.is_empty() && !t.selection.is_empty() && t.result.is_some() {
                    list = list.child(row("at-intf-none".into(), "No interferences".into()));
                }
                for (k, (i, a, b)) in rows.iter().enumerate() {
                    let size = i.bounds.1 - i.bounds.0;
                    let dims = format!("{} x {} x {}", fmt_len(size.x), fmt_len(size.y), fmt_len(size.z));
                    let bounds = i.bounds;
                    list = list.child(
                        row(SharedString::from(format!("at-intf-{k}")), format!("{a} and {b}"))
                            .cursor_pointer()
                            .when(picked == Some(k), |d| d.font_weight(FontWeight::BOLD))
                            .tooltip(crate::ui::tooltip(dims))
                            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                                let new = if *h { Some(k) } else { None };
                                if this.ui.interference_hover != new {
                                    this.ui.interference_hover = new;
                                    this.refresh_scene(false, cx);
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.ui.interference_picked = Some(k);
                                this.push_camera_history(cx);
                                let inset = this.fit_inset_left();
                                this.viewport.update(cx, |vp, cx| {
                                    let mut to = vp.camera;
                                    crate::viewport::fit_camera_to_box(&mut to, bounds.0, bounds.1, vp.size(), inset);
                                    vp.animate_to(to, cx);
                                });
                                cx.notify();
                            })),
                    );
                }
                body = body.child(list);
            }
            ToolKind::Connection => {
                // A row per class: checkbox, swatch, name and (G1–G3) tolerance.
                let names = ["No connection", "G0", "G1", "G2", "G3"];
                let icons = ["connectivity-analysis-no-connection", "connectivity-analysis-g0", "connectivity-analysis-g1", "connectivity-analysis-g2", "connectivity-analysis-g3"];
                for i in 0..5 {
                    let on = o.show_class[i];
                    let mut row = h_flex()
                        .h(px(26.))
                        .gap(px(6.))
                        .text_size(px(12.))
                        .child(gpui_kit::component::checkbox::Checkbox::new(SharedString::from(format!("at-class-{i}"))).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                            if let Some(t) = &mut this.ui.analysis_tool {
                                t.options.show_class[i] = *v;
                            }
                            this.refresh_scene(false, cx);
                            cx.notify();
                        })))
                        .child(div().w(px(32.)).h(px(24.)).flex().items_center().justify_center().child(crate::assets::icon(icons[i], 24., cx)))
                        .child(div().w(px(if i == 0 { 100. } else { 22. })).text_color(if i == 0 { pal.text } else { pal.text_muted }).child(names[i]));
                    if let Some(st) = i.checked_sub(2).and_then(|k| t.tolerance_inputs.get(k)) {
                        row = row.child(div().flex_1().child(st.clone()));
                    }
                    body = body.child(row);
                }
            }
            ToolKind::Zebra => {
                let z = o.zebra;
                let set = |f: fn(&mut ZebraSettings, bool)| {
                    move |this: &mut Self, v: &bool, _: &mut Window, cx: &mut Context<Self>| {
                        if let Some(t) = &mut this.ui.analysis_tool {
                            f(&mut t.options.zebra, *v);
                        }
                        this.rebuild_scene(cx);
                    }
                };
                if let Some(st) = t.tolerance_inputs.first() {
                    body = body.child(
                        h_flex()
                            .h(px(26.))
                            .gap_2()
                            .text_size(px(12.))
                            .child(div().text_color(pal.text_muted).child("Stripe count"))
                            .child(div().flex_1().child(st.clone())),
                    );
                }
                body = body
                    .child(gpui_kit::component::checkbox::Checkbox::new("at-zflip").label("Flip stripes").checked(z.flip).on_click(cx.listener(set(|z, v| z.flip = v))))
                    .child(gpui_kit::component::checkbox::Checkbox::new("at-zedges").label("Show edges").checked(z.show_edges).on_click(cx.listener(set(|z, v| z.show_edges = v))));
            }
            ToolKind::Reflection => {
                let r = o.reflection.clone();
                let set = |f: fn(&mut super::reflection::ReflectionSettings, bool)| {
                    move |this: &mut Self, v: &bool, _: &mut Window, cx: &mut Context<Self>| {
                        if let Some(t) = &mut this.ui.analysis_tool {
                            f(&mut t.options.reflection, *v);
                        }
                        this.rebuild_scene(cx);
                    }
                };
                let picking = self.ui.env_picker.is_some();
                body = body.child(div().text_size(px(12.)).text_color(pal.text_muted).child("Environment")).child(
                    h_flex()
                        .id("at-env")
                        .h(px(28.))
                        .px(px(6.))
                        .gap(px(6.))
                        .border_1()
                        .rounded(px(2.))
                        .border_color(if picking { pal.accent } else { gpui_kit::rgb(0xbdbdbd) })
                        .cursor_pointer()
                        .child(crate::assets::icon("image", 16., cx))
                        .child(div().flex_1().truncate().text_size(px(13.)).text_color(if r.image.is_some() { pal.text } else { pal.text_muted }).child(r.image.as_ref().map_or("Select image…".to_string(), |i| i.name().to_string())))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_env_picker(window, cx))),
                );
                if let Some(st) = t.tolerance_inputs.first() {
                    body = body.child(
                        h_flex()
                            .h(px(26.))
                            .gap_2()
                            .text_size(px(12.))
                            .child(div().text_color(pal.text_muted).child("Rotation angle"))
                            .child(div().flex_1().child(st.clone()))
                            .child(
                                div()
                                    .id("at-rot-opp")
                                    .cursor_pointer()
                                    .rounded(px(2.))
                                    .when(r.opposite, |d| d.bg(gpui_kit::rgb(0xdef1fb)))
                                    .child(crate::assets::icon("flip-circular-opposite", 18., cx))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        if let Some(t) = &mut this.ui.analysis_tool {
                                            t.options.reflection.opposite = !t.options.reflection.opposite;
                                        }
                                        this.rebuild_scene(cx);
                                    })),
                            ),
                    );
                }
                body = body
                    .child(gpui_kit::component::checkbox::Checkbox::new("at-redges").label("Show edges").checked(r.show_edges).on_click(cx.listener(set(|r, v| r.show_edges = v))))
                    .child(gpui_kit::component::checkbox::Checkbox::new("at-rblur").label("Blur environment").checked(r.blur).on_click(cx.listener(set(|r, v| r.blur = v))))
                    .child(gpui_kit::component::checkbox::Checkbox::new("at-rlock").label("Lock environment").checked(r.lock).on_click(cx.listener(set(|r, v| r.lock = v))));
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
                    .child(div().text_size(px(12.)).text_color(pal.text).child(if t.selection.is_empty() { String::new() } else if o.show_max { format!("Max: {} deg", noshape_units::to_fixed(hi, 2)) } else { format!("Min: {} deg", noshape_units::to_fixed(lo, 2)) }))
                    .child(check("at-thr", "Enable threshold display", o.threshold, |o, v| o.threshold = v, cx));
                if o.threshold {
                    body = body
                        .child(stepper("at-hi", "High threshold", format!("{} deg", noshape_units::to_fixed(o.high_deg, 0)), |o, d| o.high_deg = (o.high_deg + d as f64).clamp(0.0, 180.0), cx))
                        .child(stepper("at-lo", "Low threshold", format!("{} deg", noshape_units::to_fixed(o.low_deg, 0)), |o, d| o.low_deg = (o.low_deg + d as f64).clamp(0.0, 180.0), cx));
                }
            }
        }
        // Live: Interference detection has no Scale slider (dom/dialog-interference-*.txt).
        if !matches!(kind, ToolKind::Connection | ToolKind::Flatten | ToolKind::Interference) && !kind.is_view_mode() {
            body = body.child(h_flex().gap_2().text_size(px(12.)).text_color(pal.text_muted).child("Scale").child(div().flex_1().child(Slider::new(&t.slider))));
        }
        // Live (psb 10-19 i): Interference detection's panel is a 600 × 80
        // rect snapped to the viewport's right edge, left = W − 600, top =
        // H − 401, the 220 px dialog at its top-left; a later resize keeps the
        // right offset and the top. The other analysis dialogs open at the top
        // left like feature dialogs.
        let (left, top) = if kind == ToolKind::Interference {
            let size = self.viewport.read(cx).size();
            let top = t.anchor_top.unwrap_or(((size.y as f32) - 401.).max(0.));
            (((size.x as f32) - 600.).max(0.), top)
        } else {
            (0., 0.)
        };
        let dialog = v_flex()
            .id("analysis-tool")
            .occlude()
            .absolute()
            .top(px(top))
            .left(px(left))
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
                    // Zebra stripes has ✓ (keep them on) and ✗ (turn them off).
                    .when(kind.is_view_mode(), |d| {
                        d.child(
                            div()
                                .id("at-accept")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .bg(gpui_kit::rgb(0x2b9d3a))
                                .child(crate::assets::icon("dialog-ok", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.accept_view_tool(cx);
                                })),
                        )
                    })
                    .child(
                        div()
                            .id("at-close")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child(crate::assets::icon("dialog-cancel", 24., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.close_analysis_tool(cx);
                            })),
                    ),
            )
            .child(body)
            // The help row every analysis dialog ends with: 22 px, a 14 px
            // `?` 4 px in from the right (dialog-help-container).
            // Interference detection, model changed while open (live psb 10-19 i):
            // a 26 px row with a 26 px ↻ button at the left ("Interference
            // detection not accurate, instances have moved."); the old result
            // stays until it is clicked.
            .child(
                h_flex()
                    .h(px(if stale { 26. } else { 22. }))
                    .pl(px(3.))
                    .pr(px(4.))
                    .when(stale, |d| {
                        d.child(
                            div()
                                .id("at-refresh")
                                .size(px(26.))
                                .p(px(3.))
                                .rounded(px(3.))
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .child(crate::assets::icon("refresh", 20., cx))
                                .tooltip(crate::ui::tooltip("Interference detection not accurate, instances have moved."))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let Some(t) = &mut this.ui.analysis_tool {
                                        t.stale = false;
                                    }
                                    this.recompute_analysis_tool(cx);
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xa0a0a0)).flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("?")),
            )
            .into_any_element();
        let mut out = vec![dialog];
        // "Min: …" / "Max: …" beside the deviation lines; the scale slider
        // moves them away from the curves (Onshape). Drawn in the 3D frame
        // (`analysis_labels`) unless NOSHAPE_GPUI_LABELS.
        let gpui_labels = crate::overlay_labels::gpui_labels();
        if gpui_labels && let Some((min, max)) = t.result.as_ref().and_then(|r| r.deviation) {
            let vp = self.viewport.read(cx);
            for ((d, a, b), label, color) in [(min, "Min", 0x2858dcu32), (max, "Max", 0xdc3228)] {
                let mid = (a + b) * 0.5;
                if let Some(sp) = vp.project(mid) {
                    let off = 10.0 + 120.0 * o.scale;
                    out.push(div().absolute().left(px(sp.x as f32 + off)).top(px(sp.y as f32 - 10.)).px(px(3.)).bg(gpui_kit::rgb(0xffffff)).border_1().border_color(gpui_kit::rgb(color)).text_size(px(12.)).text_color(pal.text).child(format!("{label}: {}", fmt_len(d))).into_any_element());
                }
            }
        }
        // "R 12.5 mm" beside the minimum-radius point.
        if gpui_labels
            && o.min_radius
            && kind == ToolKind::CurveSurface
            && let Some((r, at)) = t.result.as_ref().and_then(|r| r.min_radius)
            && let Some(s) = self.viewport.read(cx).project(at)
        {
            out.push(div().absolute().left(px(s.x as f32 + 8.)).top(px(s.y as f32 - 8.)).px(px(3.)).bg(gpui_kit::rgb(0xffffff)).border_1().border_color(gpui_kit::rgb(0xd228c8)).text_size(px(12.)).text_color(pal.text).child(format!("R {}", fmt_len(r))).into_any_element());
        }
        out.extend(self.render_env_picker(cx));
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

/// Points evenly spread along a polyline (at least `n` of them).
fn densify(pl: &[DVec3], n: usize) -> Vec<DVec3> {
    let total: f64 = pl.windows(2).map(|w| w[0].distance(w[1])).sum();
    if pl.len() < 2 || total <= 0.0 {
        return pl.to_vec();
    }
    let step = total / n.max(1) as f64;
    let mut out = vec![pl[0]];
    for w in pl.windows(2) {
        let l = w[0].distance(w[1]);
        let k = (l / step).ceil().max(1.0) as usize;
        for i in 1..=k {
            out.push(w[0].lerp(w[1], i as f64 / k as f64));
        }
    }
    out
}

impl PartStudioView {
    /// Polyline of a part edge or sketch curve (display tessellation).
    fn curve_points(&self, r: &Reference) -> Vec<DVec3> {
        let p3 = |p: &[f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        match r {
            Reference::SketchEntity { sketch, entity } => self
                .display
                .sketches
                .iter()
                .find(|s| s.feature == *sketch)
                .and_then(|s| s.curves.iter().find(|c| c.entity == *entity))
                .map(|c| c.points.iter().map(p3).collect())
                .unwrap_or_default(),
            _ => self.display.bodies.iter().find_map(|b| b.edge_refs.iter().position(|x| x == r).and_then(|i| b.mesh.edges.get(i)).map(|pl| pl.iter().map(p3).collect())).unwrap_or_default(),
        }
    }

    /// Triangles of a part face (display tessellation).
    fn face_triangles(&self, r: &Reference) -> Vec<[DVec3; 3]> {
        let p3 = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        self.display
            .bodies
            .iter()
            .find_map(|b| {
                let fi = b.face_refs.iter().position(|x| x == r)?;
                let &(first, count) = b.mesh.face_ranges.get(fi)?;
                let idx = b.mesh.indices.get(first as usize..(first + count) as usize)?;
                Some(idx.as_chunks::<3>().0.iter().filter_map(|t| Some([p3(*b.mesh.positions.get(t[0] as usize)?), p3(*b.mesh.positions.get(t[1] as usize)?), p3(*b.mesh.positions.get(t[2] as usize)?)])).collect())
            })
            .unwrap_or_default()
    }
}
