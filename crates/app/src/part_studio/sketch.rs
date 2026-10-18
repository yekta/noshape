//! Sketch mode integration for the Part Studio view: routes viewport input
//! to the [`SketchEditor`], renders the sketch toolbar, dimension labels,
//! constraint glyphs, inference icons, inline value entry and tool option
//! boxes, and regenerates synchronously so dragging and drawing feel
//! immediate.

use super::PartStudioView;
use crate::actions::*;
use crate::assets::icon_path;
use crate::sketch_mode::{ConstraintTool, EditOutcome, ExternalSnap, SketchEditor, Tool};
use crate::theme::{p, with_alpha};
use crate::viewport::{Viewport, ViewportEvent};
use glam::{DVec2, DVec3};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Disableable as _, Icon, WindowExt as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::display::{FeatureStatus, PlaneFrame};
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::RegenOptions;
use noshape_model::sketch_feature::SketchData;
use noshape_render::{Drawable, Instance};
use noshape_units::{Dim, Quantity};
use std::collections::HashMap;
use std::sync::Arc;

actions!(sketch_entry, [SketchEntryNext, SketchEntryPrev, SketchEntryCancel]);

static BIND_ENTRY_KEYS: std::sync::Once = std::sync::Once::new();

const DRAW_TOOLS: &[&[Tool]] = &[
    &[Tool::Line],
    &[Tool::CornerRect, Tool::CenterRect, Tool::AlignedRect],
    &[Tool::Circle, Tool::ThreePointCircle],
    &[Tool::Arc3, Tool::TangentArc, Tool::CenterArc],
    &[Tool::Conic],
    &[Tool::Spline, Tool::Bezier],
    &[Tool::Point],
    &[Tool::Slot],
    &[Tool::Polygon, Tool::CircumPolygon],
    &[Tool::Ellipse],
    &[Tool::Text],
];

const EDIT_TOOLS: &[&[Tool]] = &[
    &[Tool::Fillet],
    &[Tool::Trim],
    &[Tool::Extend],
    &[Tool::Split],
    &[Tool::Offset],
    &[Tool::Mirror],
    &[Tool::LinearPattern, Tool::CircularPattern],
    &[Tool::Transform],
    &[Tool::Use, Tool::Intersection],
];

const CONSTRAINT_TOOLS: &[&[Tool]] = &[
    &[Tool::Constraint(ConstraintTool::Coincident)],
    &[Tool::Constraint(ConstraintTool::Concentric)],
    &[Tool::Constraint(ConstraintTool::Parallel)],
    &[Tool::Constraint(ConstraintTool::Tangent)],
    &[Tool::Constraint(ConstraintTool::Horizontal)],
    &[Tool::Constraint(ConstraintTool::Vertical)],
    &[Tool::Constraint(ConstraintTool::Midpoint)],
    &[Tool::Constraint(ConstraintTool::Perpendicular)],
    &[Tool::Constraint(ConstraintTool::Equal)],
    &[Tool::Constraint(ConstraintTool::Symmetric)],
    &[Tool::Constraint(ConstraintTool::Fix)],
    &[Tool::Constraint(ConstraintTool::Normal), Tool::Constraint(ConstraintTool::Pierce), Tool::Constraint(ConstraintTool::Curvature)],
];

/// View-side state of the sketch editor (GPUI input entities), stored in
/// [`SketchEditor::ui`] so it lives exactly as long as the sketch session.
#[derive(Default)]
pub(super) struct SketchUi {
    entry: Vec<Entity<InputState>>,
    entry_field: usize,
    entry_subs: Vec<Subscription>,
    opts: HashMap<&'static str, Entity<InputState>>,
    opt_subs: Vec<Subscription>,
    /// Text box the "tedit.text" field currently edits.
    text_edit_for: Option<u32>,
    text_edit_sub: Option<Subscription>,
    /// Open dimension context menu: constraint and viewport position.
    dim_menu: Option<(noshape_sketch::ConstraintId, f32, f32)>,
}

/// Style changes of sketch text from the options box.
#[derive(Clone, Copy)]
enum TextStyle {
    Font(&'static str),
    Bold,
    Italic,
    FlipHorizontal,
    FlipVertical,
}

/// Kind of value an option field holds.
#[derive(Clone, Copy, PartialEq)]
enum OptKind {
    Length,
    Angle,
    Count,
    Number,
    Text,
}

/// Option fields shown in the floating options box for a tool.
fn tool_options(tool: Tool) -> &'static [(&'static str, &'static str, OptKind)] {
    match tool {
        Tool::Fillet => &[("fillet.radius", "Radius", OptKind::Length)],
        Tool::Offset => &[("offset.distance", "Distance", OptKind::Length)],
        Tool::Polygon | Tool::CircumPolygon => &[("polygon.sides", "Sides", OptKind::Count)],
        Tool::Conic => &[("conic.rho", "Rho", OptKind::Number)],
        Tool::Text => &[("text.text", "Text", OptKind::Text), ("text.height", "Height (click)", OptKind::Length)],
        Tool::LinearPattern => &[("lp.count", "Instances", OptKind::Count), ("lp.distance", "Distance", OptKind::Length), ("lp.angle", "Direction", OptKind::Angle)],
        Tool::CircularPattern => &[("cp.count", "Instances", OptKind::Count), ("cp.angle", "Total angle", OptKind::Angle)],
        Tool::Transform => &[("xf.dx", "Move X", OptKind::Length), ("xf.dy", "Move Y", OptKind::Length), ("xf.angle", "Rotate", OptKind::Angle), ("xf.scale", "Scale", OptKind::Number)],
        _ => &[],
    }
}

fn icon_px(name: &str, size: f32, color: Rgba) -> Icon {
    Icon::empty().path(icon_path(name)).text_color(color).with_size(px(size))
}

impl PartStudioView {
    /// Id of the sketch feature being edited (if the dialog is a sketch).
    pub(super) fn editing_sketch_id(&self, cx: &App) -> Option<FeatureId> {
        let e = self.edit.as_ref()?;
        let d = e.dialog.read(cx);
        (d.feature.type_id == "sketch").then_some(d.feature.id)
    }

    /// Create/update/drop the sketch editor to match the edit session.
    pub(super) fn sync_sketch_editor(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.editing_sketch_id(cx) else {
            if self.sketch.take().is_some() {
                self.opts.editing_sketch = None;
                self.viewport.update(cx, |vp, _| vp.tool_captures_left = false);
            }
            return;
        };
        BIND_ENTRY_KEYS.call_once(|| {
            cx.bind_keys([
                KeyBinding::new("tab", SketchEntryNext, Some("SketchEntry")),
                KeyBinding::new("shift-tab", SketchEntryPrev, Some("SketchEntry")),
                KeyBinding::new("alt-right", SketchEntryNext, Some("SketchEntry")),
                KeyBinding::new("alt-left", SketchEntryPrev, Some("SketchEntry")),
                KeyBinding::new("escape", SketchEntryCancel, Some("SketchEntry")),
            ]);
        });
        let plane = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.query("sketchPlane").first().cloned());
        let frame = self.display.sketches.iter().find(|s| s.feature == id).map(|s| s.frame).or_else(|| match plane {
            Some(Reference::Plane(DatumPlane::Top)) => Some(PlaneFrame::TOP),
            Some(Reference::Plane(DatumPlane::Front)) => Some(PlaneFrame::FRONT),
            Some(Reference::Plane(DatumPlane::Right)) => Some(PlaneFrame::RIGHT),
            _ => None,
        });
        let Some(frame) = frame else { return };
        match &mut self.sketch {
            Some(ed) if ed.feature == id => ed.frame = frame,
            _ => {
                self.sketch = Some(SketchEditor::new(id, frame));
                // A fresh sketch starts with the line tool, like Onshape.
                let empty = self.edit.as_ref().and_then(|e| e.dialog.read(cx).sketch_data().map(|d| d.sketch.entity_count() == 0)).unwrap_or(false);
                if empty && let Some(ed) = &mut self.sketch {
                    ed.set_tool(Tool::Line);
                }
            }
        }
        self.update_conflicts();
        self.opts.editing_sketch = Some(id);
        self.viewport.update(cx, |vp, _| vp.tool_captures_left = true);
    }

    pub(super) fn leave_sketch(&mut self, cx: &mut Context<Self>) {
        if self.sketch.take().is_some() {
            self.dim_input = None;
            self.opts.editing_sketch = None;
            self.viewport.update(cx, |vp, _| vp.tool_captures_left = false);
        }
    }

    /// Copy the solver's conflicting constraints into the editor (red glyphs/dims).
    fn update_conflicts(&mut self) {
        let Some(ed) = &mut self.sketch else { return };
        let f = ed.feature;
        ed.conflicting = self.context.as_ref().and_then(|c| c.sketches.iter().find(|s| s.feature == f)).map(|s| s.report.conflicting().to_vec()).unwrap_or_default();
    }

    fn ui_mut(&mut self) -> Option<&mut SketchUi> {
        let ed = self.sketch.as_mut()?;
        if !ed.ui.as_ref().is_some_and(|u| u.is::<SketchUi>()) {
            ed.ui = Some(Box::new(SketchUi::default()));
        }
        ed.ui.as_mut()?.downcast_mut::<SketchUi>()
    }

    fn ui_ref(&self) -> Option<&SketchUi> {
        self.sketch.as_ref()?.ui.as_ref()?.downcast_ref::<SketchUi>()
    }

    /// Run `f` with the editor and the draft sketch data.
    fn with_sketch<R>(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut SketchEditor, &mut SketchData) -> R) -> Option<R> {
        let dialog = self.edit.as_ref()?.dialog.clone();
        let mut editor = self.sketch.take()?;
        let r = dialog.update(cx, |d, _| d.sketch_data_mut().map(|data| f(&mut editor, data)));
        self.sketch = Some(editor);
        r
    }

    fn sketch_data(&self, cx: &App) -> Option<SketchData> {
        self.edit.as_ref().and_then(|e| e.dialog.read(cx).sketch_data().cloned())
    }

    /// Regenerate immediately on the UI thread (sketch edits are cheap: only
    /// the sketch is evaluated, upstream results come from the cache).
    pub(super) fn regen_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((studio, upto)) = self.effective_studio(cx) else { return };
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let config = self.current_config(&studio);
        let editing_sketch = self.sketch.as_ref().map(|s| s.feature);
        self.regen_seq += 1;
        let result = {
            let mut regen = self.regen.lock();
            let document_variables = noshape_model::variables::document_variables(&self.doc.read(cx).session.doc);
            let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
            regen.regenerate(&studio, &config, &units, &RegenOptions { upto, editing_sketch, document_variables, derived: Some(derived), ..Default::default() })
        };
        self.regenerating = false;
        self.regen_task = None;
        self.context = Some(result.context.clone());
        self.update_conflicts();
        self.apply_display(Arc::new(result.display), window, cx);
    }

    fn apply_outcome(&mut self, out: EditOutcome, window: &mut Window, cx: &mut Context<Self>) {
        if out.changed {
            self.regen_sync(window, cx);
        } else if out.redraw {
            self.refresh_scene(false, cx);
        }
        cx.notify();
    }

    fn hovered_sketch_entity(&self, cx: &App) -> Option<noshape_sketch::EntityId> {
        let ed = self.sketch.as_ref()?;
        let e = match &self.hover {
            Some(Reference::SketchEntity { sketch, entity }) if *sketch == ed.feature => noshape_sketch::EntityId(*entity),
            _ => return None,
        };
        // Text glyph outlines are derived geometry: they stand for their text
        // box when selecting / transforming, and are ignored by other tools.
        if noshape_sketch::text::glyph_text_id(e).is_some() {
            if !(ed.tool == Tool::Select || ed.tool.is_pattern()) {
                return None;
            }
            let d = self.edit.as_ref()?.dialog.read(cx);
            let data = d.sketch_data()?;
            let t = data.text_of_entity(e)?;
            return data.texts.iter().find(|x| x.id == t).and_then(|x| x.box_lines.first()).map(|l| noshape_sketch::EntityId(*l));
        }
        Some(e)
    }

    /// Snap candidate from the hovered model vertex/edge (sketching on parts).
    fn model_snap(&self, cursor: DVec2, vp: &Viewport) -> Option<ExternalSnap> {
        let ed = self.sketch.as_ref()?;
        let frame = ed.frame;
        let r = self.hover.as_ref()?;
        let radius_px = 10.0;
        let to_screen = |w: DVec3| vp.project(w);
        let cursor_s = to_screen(frame.to_world(cursor))?;
        match r {
            Reference::Vertex(_) => {
                let b = self.display.bodies.iter().find(|b| b.vertex_refs.contains(r))?;
                let i = b.vertex_refs.iter().position(|x| x == r)?;
                let v = b.mesh.vertices.get(i)?;
                let w = DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
                let local = frame.to_local(w);
                let s = to_screen(frame.to_world(local))?;
                ((s - cursor_s).length() <= radius_px * 1.6).then(|| ExternalSnap::Vertex { position: local, reference: r.clone() })
            }
            Reference::Edge(_) => {
                let b = self.display.bodies.iter().find(|b| b.edge_refs.contains(r))?;
                let i = b.edge_refs.iter().position(|x| x == r)?;
                let pl = b.mesh.edges.get(i)?;
                let pts: Vec<DVec2> = pl.iter().map(|v| frame.to_local(DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64))).collect();
                let mut best: Option<(f64, DVec2)> = None;
                for w in pts.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let d = b - a;
                    let t = if d.length_squared() < 1e-30 { 0.0 } else { ((cursor - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) };
                    let q = a + d * t;
                    let dist = (q - cursor).length();
                    if best.is_none_or(|bb| dist < bb.0) {
                        best = Some((dist, q));
                    }
                }
                let (_, q) = best?;
                let s = to_screen(frame.to_world(q))?;
                ((s - cursor_s).length() <= radius_px).then_some(ExternalSnap::Edge { position: q })
            }
            _ => None,
        }
    }

    /// Handle viewport input in sketch mode. Returns true when consumed.
    pub(super) fn sketch_viewport_event(&mut self, vp: &Entity<Viewport>, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let wpp = {
            let v = vp.read(cx);
            v.camera.world_per_pixel(v.size().y)
        };
        let to_plane = |this: &Self, pos: DVec2, cx: &App| -> Option<DVec2> {
            let (o, d) = vp.read(cx).ray(pos);
            this.sketch.as_ref()?.plane_point(o, d)
        };
        let wpp_changed = self.sketch.as_ref().is_some_and(|ed| (ed.world_per_px - wpp).abs() > wpp * 1e-6);
        if let Some(ed) = &mut self.sketch {
            ed.world_per_px = wpp;
        }
        match ev {
            ViewportEvent::CameraChanged => {
                // Arrowheads and glyph offsets are sized in pixels.
                if wpp_changed {
                    self.refresh_scene(false, cx);
                }
                cx.notify();
                false
            }
            ViewportEvent::Hover(id) => {
                // While a glyph/label is hovered, the geometry under it isn't.
                if self.sketch.as_ref().is_some_and(|s| s.hover_constraint.is_some()) {
                    if self.hover.take().is_some() {
                        self.refresh_scene(false, cx);
                    }
                    return true;
                }
                let r = id.and_then(|id| self.built.as_ref()?.picks.get(id).map(|t| t.reference.clone()));
                if let Some(ed) = &mut self.sketch {
                    let ent = match &r {
                        Some(Reference::SketchEntity { sketch, entity }) if *sketch == ed.feature && noshape_sketch::text::glyph_text_id(noshape_sketch::EntityId(*entity)).is_none() => Some(noshape_sketch::EntityId(*entity)),
                        _ => None,
                    };
                    if ed.hover != ent {
                        if let Some(old) = ed.hover {
                            ed.glyph_entity = Some((old, std::time::Instant::now()));
                        }
                        ed.hover = ent;
                        cx.notify();
                    }
                }
                false
            }
            ViewportEvent::PointerMove { position, modifiers } => {
                let Some(p) = to_plane(self, *position, cx) else { return false };
                let shift = modifiers.shift;
                let was_shift = self.sketch.as_ref().is_some_and(|s| s.shift_held);
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.pointer_move(data, p, shift)) {
                    if !shift
                        && let Some(snap) = self.model_snap(p, vp.read(cx))
                        && let Some(ed) = &mut self.sketch
                    {
                        ed.apply_external_snap(snap);
                    }
                    let hov = self.hovered_sketch_entity(cx);
                    if let Some(ed) = &mut self.sketch {
                        ed.hover = hov;
                    }
                    self.apply_outcome(out, window, cx);
                }
                if was_shift != shift {
                    cx.notify();
                }
                // Let hover highlighting run too.
                false
            }
            ViewportEvent::ToolDown { position, modifiers, click_count } => {
                self.close_dim_menu(cx);
                let tool = self.sketch.as_ref().map(|s| s.tool);
                let overlay_pressed = self.sketch.as_ref().is_some_and(|s| s.pressed_label.is_some() || s.pressed_glyph.is_some());
                if !overlay_pressed && tool == Some(Tool::Use) {
                    self.use_hovered(window, cx);
                    return true;
                }
                if !overlay_pressed && tool == Some(Tool::Intersection) {
                    self.intersect_hovered(window, cx);
                    return true;
                }
                if !overlay_pressed && !self.focus.is_focused(window) {
                    // Clicking the canvas leaves option/value fields.
                    window.focus(&self.focus, cx);
                }
                if !overlay_pressed && self.dim_input.take().is_some() {
                    // ... and drops an open dimension value field.
                    if let Some(ed) = &mut self.sketch {
                        ed.dim_edit = None;
                    }
                }
                let Some(p) = to_plane(self, *position, cx) else { return true };
                let hovered = self.hovered_sketch_entity(cx);
                let (shift, dbl) = (modifiers.shift || modifiers.control, *click_count >= 2);
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_down(data, p, hovered, shift, dbl)) {
                    self.apply_outcome(out, window, cx);
                }
                self.sync_entry_after_place(window, cx);
                self.sync_pattern_inputs(window, cx);
                self.sync_dim_edit(window, cx);
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(p) = to_plane(self, *position, cx) else { return true };
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_drag(data, p)) {
                    self.apply_outcome(out, window, cx);
                }
                if self.sketch.as_ref().is_some_and(|s| s.tool.is_pattern()) {
                    self.sync_pattern_inputs(window, cx);
                }
                true
            }
            ViewportEvent::ToolUp { position, .. } => {
                let Some(p) = to_plane(self, *position, cx) else { return true };
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_up(data, p)) {
                    self.apply_outcome(out, window, cx);
                }
                self.sync_pattern_inputs(window, cx);
                true
            }
            ViewportEvent::Click { .. } => true,
            _ => false,
        }
    }

    /// Overlay instances and selection/accent refs for the sketch editor.
    pub(super) fn sketch_overlay(&self, selection: &mut Vec<Reference>, accent: &mut Vec<Reference>, cx: &App) -> Vec<Instance> {
        let Some(ed) = &self.sketch else { return Vec::new() };
        let f = ed.feature;
        let data = self.sketch_data(cx);
        selection.clear();
        selection.extend(ed.selection.iter().map(|e| Reference::SketchEntity { sketch: f, entity: e.0 }));
        accent.clear();
        accent.extend(ed.picked(data.as_ref()).iter().map(|e| Reference::SketchEntity { sketch: f, entity: e.0 }));
        let Some(data) = data else { return Vec::new() };
        let (lines, points) = ed.overlay(&data);
        let up = self.gpu.uploader();
        let mut out = Vec::new();
        if !lines.segments.is_empty() {
            out.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(30));
        }
        if !points.points.is_empty() {
            out.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().unpickable().ignore_clip().layer(31));
        }
        out
    }

    /// Edges of a hovered model face (or the hovered edge itself).
    fn hovered_edge_refs(&self) -> Vec<Reference> {
        let Some(r) = self.hover.clone() else { return Vec::new() };
        let Some(ctx) = self.context.clone() else { return Vec::new() };
        let mut refs: Vec<Reference> = Vec::new();
        match &r {
            Reference::Edge(_) => refs.push(r.clone()),
            Reference::Face(_) => {
                if let Some(b) = self.display.bodies.iter().find(|b| b.face_refs.contains(&r)) {
                    let fi = b.face_refs.iter().position(|x| *x == r).unwrap_or(0);
                    if let Some(body) = ctx.body(&b.id)
                        && let Ok(face) = body.shape.sub_shape(noshape_kernel::ShapeType::Face, fi)
                    {
                        for e in face.edges() {
                            if let Some(i) = body.shape.index_of(&e, noshape_kernel::ShapeType::Edge)
                                && let Some(n) = body.edges.get(i)
                            {
                                refs.push(Reference::Edge((**n).clone()));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        refs
    }

    /// Project the hovered model edge (or all edges of a hovered face, or a vertex).
    fn use_hovered(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.hover.clone() else { return };
        let Some(ctx) = self.context.clone() else { return };
        let Some(frame) = self.sketch.as_ref().map(|s| s.frame) else { return };
        let mut changed = false;
        if let Reference::Vertex(_) = &r {
            let cursor = self.sketch.as_ref().and_then(|s| s.cursor).unwrap_or_default();
            let snap = self.model_snap(cursor, self.viewport.read(cx));
            if let Some(ExternalSnap::Vertex { position, reference }) = snap {
                changed = self
                    .with_sketch(cx, |_, data| {
                        let id = data.sketch.use_external_point(position);
                        data.external_refs.insert(id.0, reference);
                        true
                    })
                    .unwrap_or(false);
            }
        }
        for er in self.hovered_edge_refs() {
            let shapes = noshape_model::regen::measure::resolve_shapes(&ctx, &er);
            let Some(edge) = shapes.first() else { continue };
            let Some(curve) = noshape_model::regen::features::sketch::project_edge(edge, &frame) else {
                if let Some(ed) = &mut self.sketch {
                    ed.message = Some("Only lines and circles parallel to the sketch can be projected so far".into());
                }
                continue;
            };
            if self.with_sketch(cx, |ed, data| ed.add_used(data, &curve, Some(er.clone()))) == Some(true) {
                changed = true;
            }
        }
        if changed {
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    /// Intersection tool: section the hovered face with the sketch plane and
    /// add the resulting curves as fixed external geometry.
    fn intersect_hovered(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r @ Reference::Face(_)) = self.hover.clone() else {
            if let Some(ed) = &mut self.sketch {
                ed.message = Some("Click a model face that crosses the sketch plane".into());
            }
            cx.notify();
            return;
        };
        let Some(ctx) = self.context.clone() else { return };
        let Some(frame) = self.sketch.as_ref().map(|s| s.frame) else { return };
        let shapes = noshape_model::regen::measure::resolve_shapes(&ctx, &r);
        let Some(face) = shapes.first() else { return };
        let curves = match noshape_model::regen::features::sketch::section_face(face, &frame) {
            Ok(c) => c,
            Err(e) => {
                if let Some(ed) = &mut self.sketch {
                    ed.message = Some(format!("Intersection failed: {e}"));
                }
                cx.notify();
                return;
            }
        };
        if curves.is_empty() {
            if let Some(ed) = &mut self.sketch {
                ed.message = Some("That face does not cross the sketch plane".into());
            }
            cx.notify();
            return;
        }
        // The curves stay linked to the face and follow model changes.
        if self.with_sketch(cx, |ed, data| ed.add_intersection(data, r.clone(), &curves)) == Some(true) {
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    pub(super) fn sketch_cancel(&mut self, cx: &mut Context<Self>) {
        if self.ui_ref().is_some_and(|u| u.dim_menu.is_some()) {
            self.close_dim_menu(cx);
            return;
        }
        let cancelled = self.sketch.as_mut().map(|s| s.cancel()).unwrap_or(false);
        if cancelled {
            self.dim_input = None;
            self.close_entry();
            self.refresh_scene(false, cx);
            cx.notify();
        }
    }

    pub(super) fn sketch_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.with_sketch(cx, |ed, data| ed.delete_selection(data)) == Some(true) {
            self.regen_sync(window, cx);
        }
    }

    /// Toolbar activation: constraint tools apply to the selection first.
    fn activate_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        match tool {
            Tool::Constraint(c) => self.constraint(c, window, cx),
            t => self.set_tool(t, cx),
        }
    }

    fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if let Some(ed) = &mut self.sketch {
            ed.set_tool(tool);
        }
        self.close_entry();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn constraint(&mut self, c: ConstraintTool, window: &mut Window, cx: &mut Context<Self>) {
        if self.with_sketch(cx, |ed, data| ed.constraint_command(data, c)) == Some(true) {
            self.regen_sync(window, cx);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn toggle_construction(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.with_sketch(cx, |ed, data| ed.toggle_construction(data)) == Some(true) {
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    fn apply_pattern(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(out) = self.with_sketch(cx, |ed, data| ed.apply_pattern(data)) {
            self.apply_outcome(out, window, cx);
        }
        self.sync_pattern_inputs(window, cx);
    }

    /// Accept the sketch and return references to all its regions.
    pub(super) fn finish_sketch(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<Reference> {
        let Some(id) = self.sketch.as_ref().map(|s| s.feature) else { return Vec::new() };
        let regions: Vec<Reference> = self
            .display
            .sketches
            .iter()
            .find(|s| s.feature == id)
            .map(|s| s.regions.iter().map(|r| Reference::SketchRegion { sketch: id, key: r.key.clone() }).collect())
            .unwrap_or_default();
        self.accept_edit(false, window, cx);
        regions
    }

    // ------------------------------------------------------------ dimension value editing

    /// Keep the inline dimension input in sync with the editor's pending edit.
    fn sync_dim_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self.sketch.as_ref().and_then(|s| s.dim_edit.clone());
        match pending {
            Some(edit) => {
                if self.dim_input.as_ref().is_some_and(|(c, _, _)| *c == edit.constraint) {
                    return;
                }
                let state = cx.new(|cx| InputState::new(window, cx).default_value(edit.text.clone()));
                let cid = edit.constraint;
                let sub = cx.subscribe_in(&state, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
                    InputEvent::PressEnter { .. } => {
                        let text = st.read(cx).value().to_string();
                        this.commit_dimension(cid, &text, window, cx);
                    }
                    InputEvent::Blur => {
                        if let Some(ed) = &mut this.sketch {
                            ed.dim_edit = None;
                        }
                        this.dim_input = None;
                        cx.notify();
                    }
                    _ => {}
                });
                state.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                });
                self.dim_input = Some((cid, state, sub));
            }
            None => self.dim_input = None,
        }
    }

    fn commit_dimension(&mut self, cid: noshape_sketch::ConstraintId, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let vars = self.display.variables.clone();
        let res = self.with_sketch(cx, |ed, data| {
            let r = ed.apply_dimension(data, cid, text, &units, &vars);
            if r.is_ok() {
                ed.dim_edit = None;
            }
            r
        });
        match res {
            Some(Ok(())) => {
                self.dim_input = None;
                self.regen_sync(window, cx);
                window.focus(&self.focus, cx);
            }
            Some(Err(e)) => {
                // Show the resulting (possibly conflicting) state, red where it fails.
                self.regen_sync(window, cx);
                self.message = Some((FeatureStatus::Error, e));
            }
            None => {}
        }
        cx.notify();
    }

    fn open_dim_edit(&mut self, cid: noshape_sketch::ConstraintId, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let text = self
            .edit
            .as_ref()
            .and_then(|e| {
                e.dialog.read(cx).sketch_data().and_then(|d| {
                    d.dimension_exprs.get(&cid.0).map(|x| x.text.clone()).or_else(|| {
                        let c = d.sketch.constraint(cid)?;
                        let v = c.kind.value()?;
                        let ang = matches!(c.kind, noshape_sketch::ConstraintKind::Angle { .. } | noshape_sketch::ConstraintKind::ArcAngle { .. });
                        Some(noshape_units::format_quantity(Quantity::new(v, if ang { Dim::ANGLE } else { Dim::LENGTH }), &units))
                    })
                })
            })
            .unwrap_or_default();
        if let Some(ed) = &mut self.sketch {
            ed.dim_edit = Some(crate::sketch_mode::DimEdit { constraint: cid, text });
            // The label is replaced by the input, so it never sees the hover end.
            ed.hover_constraint = None;
        }
        self.sync_dim_edit(window, cx);
        cx.notify();
    }

    // ------------------------------------------------------------ inline value entry while drawing

    /// Key press on the view while sketching: digits start value entry.
    fn sketch_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = &ev.keystroke.modifiers;
        if m.control || m.alt || m.platform || !self.focus.contains_focused(window, cx) || window.has_focused_input(cx) {
            return;
        }
        let Some(ed) = &self.sketch else { return };
        if ed.entry_active || ed.entry_fields().is_empty() {
            return;
        }
        let ch = ev.keystroke.key_char.clone().unwrap_or_else(|| ev.keystroke.key.clone());
        let Some(c) = ch.chars().next().filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '=' | '(')) else { return };
        if ch.chars().count() != 1 {
            return;
        }
        let first = if c == '=' { String::new() } else { c.to_string() };
        self.open_entry(first, window, cx);
        cx.stop_propagation();
    }

    fn open_entry(&mut self, first: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ed) = &mut self.sketch else { return };
        if !ed.begin_entry() {
            return;
        }
        let fields = ed.entry_fields();
        let mut inputs = Vec::new();
        let mut subs = Vec::new();
        for (i, name) in fields.iter().enumerate() {
            let init = if i == 0 { first.clone() } else { String::new() };
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(*name).default_value(init));
            subs.push(cx.subscribe_in(&state, window, move |this: &mut Self, _st, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => this.entry_changed(window, cx),
                InputEvent::PressEnter { .. } => this.commit_entry(window, cx),
                InputEvent::Focus => {
                    if let Some(ui) = this.ui_mut() {
                        ui.entry_field = i;
                    }
                    cx.notify();
                }
                _ => {}
            }));
            inputs.push(state);
        }
        if let Some(st) = inputs.first() {
            let n = first.chars().count() as u32;
            st.update(cx, |s, cx| s.set_cursor_position(gpui_kit::component::input::Position { line: 0, character: n }, window, cx));
        }
        if let Some(ui) = self.ui_mut() {
            ui.entry = inputs;
            ui.entry_field = 0;
            ui.entry_subs = subs;
        }
        self.entry_changed(window, cx);
    }

    fn close_entry(&mut self) {
        if let Some(ui) = self.ui_mut() {
            ui.entry.clear();
            ui.entry_subs.clear();
            ui.entry_field = 0;
        }
    }

    /// Typed values changed: re-parse and refresh the preview.
    fn entry_changed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let vars = self.display.variables.clone();
        let texts: Vec<String> = self.ui_ref().map(|u| u.entry.iter().map(|s| s.read(cx).value().to_string()).collect()).unwrap_or_default();
        let Some(ed) = &mut self.sketch else { return };
        let dims = ed.entry_dims();
        ed.entry_values = texts
            .iter()
            .zip(dims)
            .map(|(t, d)| {
                let t = t.trim().trim_start_matches('=');
                if t.is_empty() {
                    return None;
                }
                noshape_units::evaluate_as(t, d, &units, &vars).ok().map(|q| q.value).filter(|v| v.is_finite() && (d == Dim::ANGLE || *v > 0.0))
            })
            .collect();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn commit_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let texts: Vec<String> = self.ui_ref().map(|u| u.entry.iter().map(|s| s.read(cx).value().to_string()).collect()).unwrap_or_default();
        self.close_entry();
        if let Some(out) = self.with_sketch(cx, |ed, data| ed.commit_entry_values(data, &texts)) {
            self.apply_outcome(out, window, cx);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn entry_step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.ui_mut() else { return };
        let n = ui.entry.len() as isize;
        if n == 0 {
            return;
        }
        ui.entry_field = ((ui.entry_field as isize + delta).rem_euclid(n)) as usize;
        let st = ui.entry[ui.entry_field].clone();
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        cx.notify();
    }

    fn cancel_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ed) = &mut self.sketch {
            ed.entry_active = false;
            ed.entry_values.clear();
        }
        self.close_entry();
        window.focus(&self.focus, cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Drop the entry UI if the editor finished it (e.g. a click committed it).
    fn sync_entry_after_place(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if self.sketch.as_ref().is_some_and(|s| !s.entry_active) {
            self.close_entry();
        }
    }

    // ------------------------------------------------------------ tool option fields

    fn option_text(&self, key: &str, cx: &App) -> String {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let Some(ed) = &self.sketch else { return String::new() };
        let len = |v: f64| noshape_units::format_quantity(Quantity::new(v, Dim::LENGTH), &units);
        let ang = |v: f64| noshape_units::format_quantity(Quantity::new(v, Dim::ANGLE), &units);
        match key {
            "fillet.radius" => len(ed.fillet_radius),
            "offset.distance" => len(ed.offset_distance),
            "polygon.sides" => ed.polygon_sides.to_string(),
            "conic.rho" => format!("{}", ed.conic_rho),
            "text.text" => ed.text_string.clone(),
            "text.height" => len(ed.text_height),
            "lp.count" => ed.pattern_count.to_string(),
            "lp.distance" => len(ed.pattern_distance),
            "lp.angle" => ang(ed.pattern_angle),
            "cp.count" => ed.circular_count.to_string(),
            "cp.angle" => ang(ed.circular_angle),
            "xf.dx" => len(ed.xf_offset.x),
            "xf.dy" => len(ed.xf_offset.y),
            "xf.angle" => ang(ed.xf_angle),
            "xf.scale" => format!("{}", ed.xf_scale),
            _ => String::new(),
        }
    }

    fn option_input(&mut self, key: &'static str, kind: OptKind, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<InputState>> {
        if let Some(st) = self.ui_ref().and_then(|u| u.opts.get(key).cloned()) {
            return Some(st);
        }
        let text = self.option_text(key, cx);
        let state = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let sub = cx.subscribe_in(&state, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                let t = st.read(cx).value().to_string();
                this.set_option(key, kind, &t, cx);
            }
            InputEvent::PressEnter { .. } => {
                // Leave the field first so the post-apply refresh updates it.
                window.focus(&this.focus, cx);
                let tool = this.sketch.as_ref().map(|s| s.tool);
                if tool.is_some_and(|t| t.is_pattern()) {
                    this.apply_pattern(window, cx);
                }
            }
            _ => {}
        });
        let ui = self.ui_mut()?;
        ui.opts.insert(key, state.clone());
        ui.opt_subs.push(sub);
        Some(state)
    }

    fn set_option(&mut self, key: &str, kind: OptKind, text: &str, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let vars = self.display.variables.clone();
        let t = text.trim();
        let val = match kind {
            OptKind::Length => noshape_units::evaluate_as(t, Dim::LENGTH, &units, &vars).ok().map(|q| q.value),
            OptKind::Angle => noshape_units::evaluate_as(t, Dim::ANGLE, &units, &vars).ok().map(|q| q.value),
            OptKind::Count | OptKind::Number => noshape_units::evaluate_as(t, Dim::UNITLESS, &units, &vars).ok().map(|q| q.value),
            OptKind::Text => Some(0.0),
        }
        .filter(|v| v.is_finite());
        let Some(ed) = &mut self.sketch else { return };
        let Some(v) = val else {
            ed.message = Some(format!("Invalid value \"{t}\""));
            cx.notify();
            return;
        };
        let count = |v: f64, lo: usize, hi: usize| (v.round().max(0.0) as usize).clamp(lo, hi);
        match key {
            "fillet.radius" if v > 0.0 => ed.fillet_radius = v,
            "offset.distance" if v != 0.0 => {
                ed.offset_distance = v.abs();
                ed.offset_flip = v < 0.0;
            }
            "polygon.sides" => ed.polygon_sides = count(v, 3, 50),
            "conic.rho" if v > 0.0 && v < 1.0 => ed.conic_rho = v,
            "text.text" => ed.text_string = text.to_string(),
            "text.height" if v > 0.0 => ed.text_height = v,
            "lp.count" => ed.pattern_count = count(v, 1, 100),
            "lp.distance" if v > 0.0 => ed.pattern_distance = v,
            "lp.angle" => ed.pattern_angle = v,
            "cp.count" => ed.circular_count = count(v, 1, 100),
            "cp.angle" if v.abs() > 1e-9 => ed.circular_angle = v,
            "xf.dx" => ed.xf_offset.x = v,
            "xf.dy" => ed.xf_offset.y = v,
            "xf.angle" => ed.xf_angle = v,
            "xf.scale" if v > 0.0 => ed.xf_scale = v,
            _ => {
                ed.message = Some("Value out of range".into());
                cx.notify();
                return;
            }
        }
        if key == "fillet.radius" {
            // A new radius starts a new fillet session (new dimension).
            ed.message = None;
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Refresh pattern fields that a viewport click may have changed.
    fn sync_pattern_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keys = ["lp.count", "lp.distance", "lp.angle", "cp.count", "cp.angle", "xf.dx", "xf.dy", "xf.angle", "xf.scale"];
        for k in keys {
            let text = self.option_text(k, cx);
            if let Some(st) = self.ui_ref().and_then(|u| u.opts.get(k).cloned())
                && st.read(cx).value() != text.as_str()
                && !st.read(cx).focus_handle(cx).is_focused(window)
            {
                st.update(cx, |s, cx| s.set_value(text, window, cx));
            }
        }
    }

    // ------------------------------------------------------------ sketch text

    /// Text box being edited (selected in the Select tool).
    fn selected_text(&self, cx: &App) -> Option<noshape_model::sketch_feature::SketchText> {
        let ed = self.sketch.as_ref()?;
        if ed.tool != Tool::Select {
            return None;
        }
        let d = self.edit.as_ref()?.dialog.read(cx);
        let data = d.sketch_data()?;
        let id = ed.selected_text(data)?;
        data.texts.iter().find(|t| t.id == id).cloned()
    }

    /// Input bound to the selected text's content (recreated when the selection changes).
    fn text_edit_input(&mut self, t: &noshape_model::sketch_feature::SketchText, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<InputState>> {
        if let Some(ui) = self.ui_ref()
            && ui.text_edit_for == Some(t.id)
            && let Some(st) = ui.opts.get("tedit.text")
        {
            return Some(st.clone());
        }
        let id = t.id;
        let state = cx.new(|cx| InputState::new(window, cx).default_value(t.text.clone()));
        let sub = cx.subscribe_in(&state, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                let text = st.read(cx).value().to_string();
                if text.trim().is_empty() {
                    return;
                }
                if this.with_sketch(cx, |ed, data| ed.edit_text(data, id, |t| t.text = text)) == Some(true) {
                    this.regen_sync(window, cx);
                }
            }
            InputEvent::PressEnter { .. } => window.focus(&this.focus, cx),
            _ => {}
        });
        let ui = self.ui_mut()?;
        ui.opts.insert("tedit.text", state.clone());
        ui.text_edit_for = Some(id);
        ui.text_edit_sub = Some(sub);
        Some(state)
    }

    /// Font / bold / italic / flip for the selected text, or for new texts.
    fn set_text_style(&mut self, style: TextStyle, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.selected_text(cx).map(|t| t.id);
        if let Some(ed) = &mut self.sketch {
            match style {
                TextStyle::Font(f) => ed.text_font = f.to_string(),
                TextStyle::Bold => ed.text_bold = !ed.text_bold,
                TextStyle::Italic => ed.text_italic = !ed.text_italic,
                _ => {}
            }
        }
        if let Some(id) = selected {
            let ok = self.with_sketch(cx, |ed, data| {
                ed.edit_text(data, id, |t| match style {
                    TextStyle::Font(f) => t.font = f.to_string(),
                    TextStyle::Bold => t.bold = !t.bold,
                    TextStyle::Italic => t.italic = !t.italic,
                    TextStyle::FlipHorizontal => t.mirror_horizontal = !t.mirror_horizontal,
                    TextStyle::FlipVertical => t.mirror_vertical = !t.mirror_vertical,
                })
            });
            if ok == Some(true) {
                self.regen_sync(window, cx);
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Font, bold, italic (and flip for existing texts) controls.
    fn text_style_row(&self, font: &str, bold: bool, italic: bool, flips: Option<(bool, bool)>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let view = cx.entity();
        let cur = font.to_string();
        let mut row = h_flex().gap_1().flex_wrap().child(
            Button::new("sk-text-font")
                .label(SharedString::from(cur.clone()))
                .ghost()
                .xsmall()
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for f in noshape_sketch::text::FONTS {
                        let v = view.clone();
                        menu = menu.item(PopupMenuItem::new(f).checked(f == cur).on_click(move |_, window, cx| v.update(cx, |this, cx| this.set_text_style(TextStyle::Font(f), window, cx))));
                    }
                    menu
                }),
        );
        row = row
            .child(Button::new("sk-text-bold").label("B").ghost().xsmall().selected(bold).tooltip("Bold").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.set_text_style(TextStyle::Bold, window, cx))))
            .child(Button::new("sk-text-italic").label("I").ghost().xsmall().selected(italic).tooltip("Italic").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.set_text_style(TextStyle::Italic, window, cx))));
        if let Some((fh, fv)) = flips {
            row = row
                .child(Button::new("sk-text-fliph").label("Flip H").ghost().xsmall().selected(fh).tooltip("Flip about the horizontal center").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.set_text_style(TextStyle::FlipHorizontal, window, cx))))
                .child(Button::new("sk-text-flipv").label("Flip V").ghost().xsmall().selected(fv).tooltip("Flip about the vertical center").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.set_text_style(TextStyle::FlipVertical, window, cx))));
        }
        let _ = pal;
        row.into_any_element()
    }

    // ------------------------------------------------------------ dimension context menu

    fn close_dim_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(ui) = self.ui_mut()
            && ui.dim_menu.take().is_some()
        {
            cx.notify();
        }
    }

    fn dim_menu_action(&mut self, cid: noshape_sketch::ConstraintId, action: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.close_dim_menu(cx);
        match action {
            "edit" => self.open_dim_edit(cid, window, cx),
            "driving" => {
                let r = self.with_sketch(cx, |ed, data| ed.toggle_driving(data, cid));
                if let Some(Err(e)) = &r {
                    self.message = Some((FeatureStatus::Warning, e.clone()));
                }
                self.regen_sync(window, cx);
            }
            "delete" => {
                if self.with_sketch(cx, |ed, data| ed.delete_constraint(data, cid)) == Some(true) {
                    self.regen_sync(window, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn render_dim_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (cid, x, y) = self.ui_ref()?.dim_menu?;
        let driving = self.sketch_data(cx).and_then(|d| d.sketch.constraint(cid).map(|c| c.driving))?;
        let item = |id: &'static str, label: &'static str, icon: &'static str, action: &'static str, cx: &mut Context<Self>| {
            Button::new(id)
                .label(label)
                .icon(icon_px(icon, 13.0, pal.text))
                .ghost()
                .xsmall()
                .w_full()
                .justify_start()
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.dim_menu_action(cid, action, window, cx)))
        };
        let mut menu = v_flex().absolute().left(px(x)).top(px(y)).occlude().w(px(170.)).p_1().gap_0p5().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.border_strong).shadow_lg();
        if driving {
            menu = menu.child(item("sk-dim-edit", "Edit value", "dimension", "edit", cx));
        }
        menu = menu
            .child(item("sk-dim-driving", if driving { "Make driven (reference)" } else { "Make driving" }, "dimension", "driving", cx))
            .child(item("sk-dim-delete", "Delete", "delete", "delete", cx));
        Some(menu.into_any_element())
    }

    // ------------------------------------------------------------ overlay rendering

    /// Dimension labels, constraint glyphs, inference icons, value entry and
    /// tool options (viewport overlay elements).
    pub(super) fn render_sketch_labels(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        if self.sketch.is_none() {
            return Vec::new();
        }
        let Some(data) = self.sketch_data(cx) else { return Vec::new() };
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let mut out = Vec::new();

        // Tool options box (creates inputs lazily, needs &mut self).
        let tool = self.sketch.as_ref().map(|s| s.tool).unwrap_or(Tool::Select);
        let fields = tool_options(tool);
        let mut rows: Vec<AnyElement> = Vec::new();
        // Editing a selected text box: content, font and style.
        let editing_text = self.selected_text(cx);
        if let Some(t) = &editing_text
            && let Some(st) = self.text_edit_input(t, window, cx)
        {
            rows.push(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .child(div().text_xs().text_color(pal.text_muted).child("Text"))
                    .child(div().w(px(160.)).child(Input::new(&st).xsmall()))
                    .into_any_element(),
            );
            rows.push(self.text_style_row(&t.font, t.bold, t.italic, Some((t.mirror_horizontal, t.mirror_vertical)), cx));
        } else if tool == Tool::Text {
            let (f, b, i) = self.sketch.as_ref().map(|e| (e.text_font.clone(), e.text_bold, e.text_italic)).unwrap_or_default();
            rows.push(self.text_style_row(&f, b, i, None, cx));
        }
        for (key, label, kind) in fields {
            if let Some(st) = self.option_input(key, *kind, window, cx) {
                let w = if *kind == OptKind::Text { 150.0 } else { 90.0 };
                rows.push(
                    h_flex()
                        .gap_2()
                        .justify_between()
                        .child(div().text_xs().text_color(pal.text_muted).child(*label))
                        .child(div().w(px(w)).child(Input::new(&st).xsmall()))
                        .into_any_element(),
                );
            }
        }

        let ed = self.sketch.as_ref().expect("sketch");
        let frame = ed.frame;
        let vp = self.viewport.read(cx);
        let project = |q: DVec2| vp.project(frame.to_world(q)).map(|s| (s.x as f32, s.y as f32));
        let editing = self.dim_input.as_ref().map(|(c, st, _)| (*c, st.clone()));

        // ---- dimension labels
        for (cid, pos, text, driving) in ed.dimension_labels(&data, &units) {
            let Some((x, y)) = project(pos) else { continue };
            if let Some((ec, st)) = &editing
                && *ec == cid
            {
                out.push(div().absolute().left(px(x - 55.0)).top(px(y - 12.0)).w(px(110.)).child(Input::new(st).xsmall()).into_any_element());
                continue;
            }
            let selected = ed.selected_constraints.contains(&cid);
            let hovered = ed.hover_constraint == Some(cid);
            let conflict = ed.conflicting.contains(&cid);
            let est_w = text.chars().count() as f32 * 6.6 + 10.0;
            let (bg, fg, border) = if selected {
                (rgb(0xbf6900), pal.text, rgb(0xffb020))
            } else if hovered {
                (rgb(0x6b5313), pal.text, rgb(0xfcda7f))
            } else if conflict {
                (with_alpha(rgb(0xaa252d), 0.85), pal.text, rgb(0xff6b6b))
            } else {
                (with_alpha(pal.panel_raised, 0.75), if driving { pal.text } else { pal.text_faint }, with_alpha(pal.border, 0.6))
            };
            out.push(
                div()
                    .id(SharedString::from(format!("dim-{}", cid.0)))
                    .absolute()
                    .left(px(x - est_w * 0.5))
                    .top(px(y - 9.0))
                    .px_1()
                    .rounded(px(4.))
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(fg)
                    .cursor_pointer()
                    .whitespace_nowrap()
                    .child(text)
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if let Some(ed) = &mut this.sketch {
                            let new = if *h { Some(cid) } else if ed.hover_constraint == Some(cid) { None } else { ed.hover_constraint };
                            if new != ed.hover_constraint {
                                ed.hover_constraint = new;
                                if new.is_some() {
                                    this.hover = None;
                                }
                                this.refresh_scene(false, cx);
                                cx.notify();
                            }
                        }
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        // Context menu: edit / driving-driven / delete (the viewport must not rotate).
                        cx.stop_propagation();
                        if let Some(ui) = this.ui_mut() {
                            ui.dim_menu = Some((cid, x + 6.0, y + 10.0));
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                        // Not consumed: the viewport receives the press too and the
                        // editor turns it into a label drag / selection.
                        let additive = ev.modifiers.shift || ev.modifiers.control;
                        if ev.click_count >= 2 && driving {
                            if let Some(ed) = &mut this.sketch {
                                ed.pressed_label = Some((cid, additive, false));
                            }
                            this.open_dim_edit(cid, window, cx);
                        } else if let Some(ed) = &mut this.sketch {
                            ed.pressed_label = Some((cid, additive, true));
                        }
                    }))
                    .into_any_element(),
            );
        }

        // ---- constraint glyphs
        let show_all = ed.show_constraints || ed.shift_held;
        let recent = ed.glyph_entity.filter(|(_, t)| t.elapsed().as_millis() < 750).map(|(e, _)| e);
        let mut per_entity: HashMap<u32, usize> = HashMap::new();
        for g in ed.constraint_glyphs(&data) {
            let visible = show_all
                || ed.hover == Some(g.entity)
                || recent == Some(g.entity)
                || ed.selection.contains(&g.entity)
                || ed.selected_constraints.contains(&g.constraint)
                || ed.hover_constraint == Some(g.constraint);
            if !visible {
                continue;
            }
            // Dragged glyphs sit centered on their stored offset; others stack next to the entity.
            let (x, y, dx, dy) = match g.offset {
                Some(o) => {
                    let Some((x, y)) = project(g.anchor + o) else { continue };
                    (x, y, -9.0, -9.0)
                }
                None => {
                    let Some((x, y)) = project(g.anchor) else { continue };
                    let k = per_entity.entry(g.entity.0).or_default();
                    let d = if g.on_point { (7.0 + 19.0 * *k as f32, -25.0) } else { (6.0 + 19.0 * *k as f32, 6.0) };
                    *k += 1;
                    (x, y, d.0, d.1)
                }
            };
            let cid = g.constraint;
            let gent = g.entity;
            let selected = ed.selected_constraints.contains(&cid);
            let hovered = ed.hover_constraint == Some(cid);
            let conflict = ed.conflicting.contains(&cid);
            let (bg, fg) = match (conflict, selected, hovered) {
                (true, true, _) => (rgb(0xbd3039), rgb(0xffffff)),
                (true, false, true) => (rgb(0xaa252d), rgb(0xffffff)),
                (true, false, false) => (rgb(0xd66e74), rgb(0xffffff)),
                (false, true, _) => (rgb(0xfab601), rgb(0x1a1a1a)),
                (false, false, true) => (rgb(0xfcda7f), rgb(0x1a1a1a)),
                _ => (rgb(0x555555), rgb(0xf2f2f2)),
            };
            out.push(
                div()
                    .id(SharedString::from(format!("glyph-{}-{}", cid.0, g.entity.0)))
                    .absolute()
                    .left(px(x + dx))
                    .top(px(y + dy))
                    .size(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .bg(bg)
                    .when(hovered, |d| d.border_2().border_color(rgb(0xffa90a)))
                    .cursor_pointer()
                    .child(icon_px(g.icon, 13.0, fg))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if let Some(ed) = &mut this.sketch {
                            let new = if *h { Some(cid) } else if ed.hover_constraint == Some(cid) { None } else { ed.hover_constraint };
                            if new != ed.hover_constraint {
                                ed.hover_constraint = new;
                                if new.is_some() {
                                    this.hover = None;
                                }
                                this.refresh_scene(false, cx);
                                cx.notify();
                            }
                        }
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, _| {
                        // Drag to move the glyph, click to select its constraint.
                        if let Some(ed) = &mut this.sketch {
                            ed.pressed_glyph = Some((cid, gent, ev.modifiers.shift || ev.modifiers.control));
                        }
                    }))
                    .into_any_element(),
            );
        }

        // ---- inference icon next to the cursor
        if ed.tool.draws()
            && !ed.entry_active
            && let Some(inf) = &ed.inference
            && let Some((x, y)) = project(inf.position)
        {
            let icons: Vec<&'static str> = if inf.kind == noshape_sketch::InferenceKind::AlignBoth { vec!["horizontal", "vertical"] } else { vec![crate::sketch_mode::inference_icon(inf.kind)] };
            out.push(
                h_flex()
                    .absolute()
                    .left(px(x + 12.0))
                    .top(px(y + 10.0))
                    .gap_0p5()
                    .children(icons.into_iter().map(|i| div().size(px(18.)).flex().items_center().justify_center().rounded(px(3.)).bg(with_alpha(rgb(0x27aae1), 0.35)).border_1().border_color(rgb(0xffc20e)).child(icon_px(i, 13.0, rgb(0xffffff)))))
                    .into_any_element(),
            );
        }

        // ---- inline value entry next to the cursor
        if ed.entry_active
            && let Some(ui) = self.ui_ref()
            && !ui.entry.is_empty()
            && let Some((x, y)) = ed.cursor.and_then(|c| project(c))
        {
            let names = ed.entry_fields();
            let active = ui.entry_field;
            let mut col = v_flex().gap_1();
            for (i, st) in ui.entry.iter().enumerate() {
                col = col.child(
                    h_flex()
                        .gap_1()
                        .child(div().w(px(56.)).text_xs().text_color(if i == active { pal.accent } else { pal.text_muted }).child(names.get(i).copied().unwrap_or("")))
                        .child(div().w(px(96.)).child(Input::new(st).xsmall())),
                );
            }
            let size = vp.size();
            let bx = if x as f64 + 240.0 > size.x { x - 236.0 } else { x + 16.0 };
            let by = if y as f64 + 110.0 > size.y { y - 104.0 } else { y + 14.0 };
            out.push(
                div()
                    .absolute()
                    .left(px(bx))
                    .top(px(by))
                    .occlude()
                    .p_1()
                    .rounded_md()
                    .bg(with_alpha(pal.panel_raised, 0.95))
                    .border_1()
                    .border_color(pal.accent)
                    .key_context("SketchEntry")
                    .on_action(cx.listener(|this, _: &SketchEntryNext, window, cx| this.entry_step(1, window, cx)))
                    .on_action(cx.listener(|this, _: &SketchEntryPrev, window, cx| this.entry_step(-1, window, cx)))
                    .on_action(cx.listener(|this, _: &SketchEntryCancel, window, cx| this.cancel_entry(window, cx)))
                    .child(col)
                    .child(div().pt_0p5().text_size(px(10.)).text_color(pal.text_faint).child("Enter to apply · Tab next field · Esc cancel"))
                    .into_any_element(),
            );
        }

        // ---- tool options box: a small floating panel at the top of the
        // viewport, right of the feature dialog (clear of the view toggles).
        if !rows.is_empty() || tool == Tool::Offset {
            let (icon, title) = if editing_text.is_some() { ("text", "Edit text") } else { (tool.icon(), tool.label()) };
            let mut panel = v_flex()
                .absolute()
                .top(px(12.))
                .left(px(342.))
                .occlude()
                .w(px(260.))
                .p_2()
                .gap_1p5()
                .rounded_md()
                .bg(with_alpha(pal.panel_raised, 0.96))
                .border_1()
                .border_color(pal.border)
                .child(h_flex().gap_1p5().child(icon_px(icon, 14.0, pal.text)).child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(title)))
                .children(rows);
            if tool == Tool::Offset {
                let flip = ed.offset_flip;
                panel = panel.child(
                    Button::new("sk-offset-flip")
                        .label(if flip { "Direction: flipped" } else { "Direction: default" })
                        .icon(icon_px("flip", 13.0, pal.text))
                        .ghost()
                        .xsmall()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(ed) = &mut this.sketch {
                                ed.offset_flip = !ed.offset_flip;
                            }
                            cx.notify();
                        })),
                );
            }
            if tool.is_pattern() {
                let n = ed.selection.len();
                panel = panel.child(div().text_xs().text_color(pal.text_faint).child(format!("{n} entit{} selected", if n == 1 { "y" } else { "ies" }))).child(
                    h_flex().gap_1().child(
                        Button::new("sk-pattern-apply")
                            .label("Apply")
                            .icon(icon_px("check", 13.0, pal.text))
                            .primary()
                            .xsmall()
                            .disabled(n == 0)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.apply_pattern(window, cx))),
                    ),
                );
            }
            out.push(panel.into_any_element());
        }

        if let Some(menu) = self.render_dim_menu(cx) {
            out.push(menu);
        }
        let ed = self.sketch.as_ref().expect("sketch");
        if let Some(m) = ed.message.clone() {
            out.push(div().absolute().bottom_3().left(px(340.)).px_2().py_1().rounded_md().bg(with_alpha(pal.panel_raised, 0.9)).text_xs().text_color(pal.text_muted).child(m).into_any_element());
        }
        out
    }

    pub(super) fn render_sketch_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(ed) = &self.sketch else { return div().into_any_element() };
        let active = ed.tool;
        let construction = ed.construction;
        let show_constraints = ed.show_constraints;
        let tool_btn = |tool: Tool, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("sk-{tool:?}")))
                .icon(icon_px(tool.icon(), 16.0, pal.text))
                .ghost()
                .with_size(px(26.))
                .selected(active == tool)
                .tooltip(tool.label())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate_tool(tool, window, cx)))
                .into_any_element()
        };
        let sep = || div().w(px(1.)).h(px(22.)).mx_1().bg(pal.border).into_any_element();
        let group = |bar: gpui_kit::Div, group: &'static [Tool], cx: &mut Context<Self>| -> gpui_kit::Div {
            // Show the active member of a group as its face.
            let face = group.iter().copied().find(|t| *t == active).unwrap_or(group[0]);
            let mut bar = bar.child(tool_btn(face, cx));
            if group.len() > 1 {
                let view = cx.entity();
                let members: Vec<Tool> = group.to_vec();
                bar = bar.child(
                    Button::new(SharedString::from(format!("skm-{:?}", group[0])))
                        .icon(Icon::new(gpui_kit::component::IconName::ChevronDown).xsmall())
                        .ghost()
                        .xsmall()
                        .w(px(14.))
                        .dropdown_menu(move |mut menu, _, _| {
                            for t in &members {
                                let v = view.clone();
                                let t = *t;
                                menu = menu.item(PopupMenuItem::new(t.label()).icon(Icon::empty().path(icon_path(t.icon()))).on_click(move |_, window, cx| v.update(cx, |this, cx| this.activate_tool(t, window, cx))));
                            }
                            menu
                        }),
                );
            }
            bar
        };
        let mut bar = h_flex().h(px(40.)).px_1().gap(px(1.)).bg(pal.panel).border_b_1().border_color(pal.accent);
        bar = bar.child(tool_btn(Tool::Select, cx)).child(sep());
        for g in DRAW_TOOLS {
            bar = group(bar, g, cx);
        }
        bar = bar.child(
            Button::new("sk-construction")
                .icon(icon_px("construction", 16.0, pal.text))
                .ghost()
                .small()
                .selected(construction)
                .tooltip("Construction (Q)")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_construction(window, cx))),
        );
        bar = bar.child(sep());
        for g in EDIT_TOOLS {
            bar = group(bar, g, cx);
        }
        bar = bar.child(sep()).child(tool_btn(Tool::Dimension, cx)).child(sep());
        for g in CONSTRAINT_TOOLS {
            bar = group(bar, g, cx);
        }
        bar = bar.child(sep()).child(
            Button::new("sk-show-constraints")
                .icon(icon_px(if show_constraints { "eye" } else { "eye-off" }, 16.0, pal.text))
                .ghost()
                .small()
                .selected(show_constraints)
                .tooltip("Show constraints (hold Shift to peek)")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(ed) = &mut this.sketch {
                        ed.show_constraints = !ed.show_constraints;
                    }
                    cx.notify();
                })),
        );
        bar = bar.child(div().flex_1()).child(
            Button::new("sk-done")
                .label("Finish sketch")
                .icon(Icon::empty().path(icon_path("check")))
                .primary()
                .small()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_edit(false, window, cx))),
        );
        bar.into_any_element()
    }

    /// Sketch keyboard actions on the view root.
    pub(super) fn with_sketch_actions(&mut self, root: Div, cx: &mut Context<Self>) -> AnyElement {
        if self.sketch.is_none() {
            return root.into_any_element();
        }
        root.on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| this.sketch_key_down(ev, window, cx)))
            .on_action(cx.listener(|this, _: &SketchLine, _, cx| this.set_tool(Tool::Line, cx)))
            .on_action(cx.listener(|this, _: &SketchArc3, _, cx| this.set_tool(Tool::Arc3, cx)))
            .on_action(cx.listener(|this, _: &SketchCircle, _, cx| this.set_tool(Tool::Circle, cx)))
            .on_action(cx.listener(|this, _: &SketchCenterRectangle, _, cx| this.set_tool(Tool::CenterRect, cx)))
            .on_action(cx.listener(|this, _: &SketchCornerRectangle, _, cx| this.set_tool(Tool::CornerRect, cx)))
            .on_action(cx.listener(|this, _: &SketchPoint, _, cx| this.set_tool(Tool::Point, cx)))
            .on_action(cx.listener(|this, _: &SketchFillet, _, cx| this.set_tool(Tool::Fillet, cx)))
            .on_action(cx.listener(|this, _: &SketchOffset, _, cx| this.set_tool(Tool::Offset, cx)))
            .on_action(cx.listener(|this, _: &SketchTrim, _, cx| this.set_tool(Tool::Trim, cx)))
            .on_action(cx.listener(|this, _: &SketchExtend, _, cx| this.set_tool(Tool::Extend, cx)))
            .on_action(cx.listener(|this, _: &SketchDimension, _, cx| this.set_tool(Tool::Dimension, cx)))
            .on_action(cx.listener(|this, _: &SketchUse, _, cx| this.set_tool(Tool::Use, cx)))
            .on_action(cx.listener(|this, _: &SketchConstruction, window, cx| this.toggle_construction(window, cx)))
            .on_action(cx.listener(|this, _: &SketchTangentArcToggle, _, cx| {
                let next = if this.sketch.as_ref().is_some_and(|s| s.tool == Tool::TangentArc) { Tool::Line } else { Tool::TangentArc };
                this.set_tool(next, cx)
            }))
            .on_action(cx.listener(|this, _: &ConstraintCoincident, w, cx| this.constraint(ConstraintTool::Coincident, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintConcentric, w, cx| this.constraint(ConstraintTool::Concentric, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintCurvature, w, cx| this.constraint(ConstraintTool::Curvature, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintEqual, w, cx| this.constraint(ConstraintTool::Equal, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintFix, w, cx| this.constraint(ConstraintTool::Fix, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintHorizontal, w, cx| this.constraint(ConstraintTool::Horizontal, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintVertical, w, cx| this.constraint(ConstraintTool::Vertical, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintMidpoint, w, cx| this.constraint(ConstraintTool::Midpoint, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintNormal, w, cx| this.constraint(ConstraintTool::Normal, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintParallel, w, cx| this.constraint(ConstraintTool::Parallel, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintPerpendicular, w, cx| this.constraint(ConstraintTool::Perpendicular, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintPierce, w, cx| this.constraint(ConstraintTool::Pierce, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintSymmetric, w, cx| this.constraint(ConstraintTool::Symmetric, w, cx)))
            .on_action(cx.listener(|this, _: &ConstraintTangent, w, cx| this.constraint(ConstraintTool::Tangent, w, cx)))
            .into_any_element()
    }
}
