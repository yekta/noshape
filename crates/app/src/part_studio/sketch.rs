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

/// Onshape's sketch toolbar groups (the button shows the group's last used tool).
const DRAW_TOOLS: &[&[Tool]] = &[
    &[Tool::Line, Tool::MidpointLine],
    &[Tool::CornerRect, Tool::CenterRect, Tool::AlignedRect],
    &[Tool::Circle, Tool::ThreePointCircle, Tool::Ellipse],
    &[Tool::Arc3, Tool::TangentArc, Tool::CenterArc, Tool::EllipticalArc, Tool::Conic],
    &[Tool::Polygon, Tool::CircumPolygon],
    &[Tool::Spline, Tool::Bezier, Tool::SplinePoint],
    &[Tool::Point],
    &[Tool::Text],
];

const USE_TOOLS: &[&[Tool]] = &[&[Tool::Use, Tool::Intersection]];

/// Live Onshape's default two-row S toolbar, in reading order.
pub(super) const SKETCH_SHORTCUT_TOOLS: [Tool; 10] = [Tool::Line, Tool::CornerRect, Tool::CenterRect, Tool::Circle, Tool::Arc3, Tool::Spline, Tool::Point, Tool::Use, Tool::Trim, Tool::Dimension];

const EDIT_TOOLS: &[&[Tool]] = &[
    &[Tool::Fillet, Tool::Chamfer],
    &[Tool::Trim, Tool::Extend, Tool::Split],
    &[Tool::Offset, Tool::Slot],
    &[Tool::Mirror],
    &[Tool::LinearPattern, Tool::CircularPattern, Tool::Transform],
];

/// Every sketch toolbar tool, in toolbar order (tool search).
pub(super) fn all_sketch_tools() -> impl Iterator<Item = Tool> {
    let groups = DRAW_TOOLS.iter().chain(USE_TOOLS).chain(EDIT_TOOLS).chain(INSERT_TOOLS);
    groups.flat_map(|g| g.iter().copied()).chain(std::iter::once(Tool::Dimension)).chain(CONSTRAINT_TOOLS.iter().map(|c| Tool::Constraint(*c)))
}

const INSERT_TOOLS: &[&[Tool]] = &[&[Tool::InsertDxf, Tool::InsertImage]];

const CONSTRAINT_TOOLS: &[ConstraintTool] = &[
    ConstraintTool::Coincident,
    ConstraintTool::Concentric,
    ConstraintTool::Parallel,
    ConstraintTool::Tangent,
    ConstraintTool::Horizontal,
    ConstraintTool::Vertical,
    ConstraintTool::Perpendicular,
    ConstraintTool::Equal,
    ConstraintTool::Midpoint,
    ConstraintTool::Normal,
    ConstraintTool::Pierce,
    ConstraintTool::Symmetric,
    ConstraintTool::Fix,
    ConstraintTool::Curvature,
];

thread_local! {
    /// Last tool used from each toolbar group (keyed by the group's first tool).
    static LAST_IN_GROUP: std::cell::RefCell<HashMap<String, Tool>> = std::cell::RefCell::new(HashMap::new());
}

fn remember_group_tool(tool: Tool) {
    for g in DRAW_TOOLS.iter().chain(USE_TOOLS).chain(EDIT_TOOLS).chain(INSERT_TOOLS) {
        if g.len() > 1 && g.contains(&tool) {
            LAST_IN_GROUP.with(|m| m.borrow_mut().insert(format!("{:?}", g[0]), tool));
        }
    }
}

fn group_face(group: &[Tool]) -> Tool {
    LAST_IN_GROUP.with(|m| m.borrow().get(&format!("{:?}", group[0])).copied()).unwrap_or(group[0])
}

/// View-side state of the sketch editor (GPUI input entities), stored in
/// [`SketchEditor::ui`] so it lives exactly as long as the sketch session.
#[derive(Default)]
pub(super) struct SketchUi {
    /// Input of the active on-the-fly dimension box (while typing).
    otf_input: Option<(Entity<InputState>, Subscription)>,
    opts: HashMap<&'static str, Entity<InputState>>,
    opt_subs: Vec<Subscription>,
    text_dialog_focused: bool,
    /// Constraint manager panel state (`None`: closed).
    cm: Option<ConstraintManager>,
    /// Profile inspector open, and the focused issue.
    pi: Option<usize>,
    /// Bezier whose degree is being edited.
    degree_edit: Option<(noshape_sketch::EntityId, Entity<InputState>, Subscription)>,
    /// Distance box of the pending offset.
    offset_input: Option<(Entity<InputState>, Subscription)>,
    /// Open pattern glyph menu: pattern id and viewport position.
    pattern_menu: Option<(u32, f32, f32)>,
    /// Dimension whose tolerance panel is open.
    tol_open: Option<noshape_sketch::ConstraintId>,
    /// Hover reference the Use preview was computed for.
    use_preview_for: Option<Reference>,
    /// Open dimension context menu: constraint and viewport position.
    dim_menu: Option<(noshape_sketch::ConstraintId, f32, f32)>,
    /// Pattern instance-count label being typed into (`3x`).
    count_edit: Option<(crate::sketch_mode::PatternHandle, Entity<InputState>, Subscription)>,
}

/// Constraint manager filters and list state.
#[derive(Default)]
pub(super) struct ConstraintManager {
    auto_select: bool,
    types: std::collections::BTreeSet<&'static str>,
    /// "Internal" / "External".
    modes: std::collections::BTreeSet<&'static str>,
    /// "Driven" / "Solved" / "Errors".
    status: std::collections::BTreeSet<&'static str>,
    by_entity: bool,
    removed: std::collections::HashSet<u32>,
    collapsed: bool,
}

/// Display name of a sketch entity in the constraint manager ("Line 4").
fn entity_name(sk: &noshape_sketch::Sketch, e: noshape_sketch::EntityId) -> String {
    use noshape_sketch::EntityKind as K;
    let k = match sk.kind(e) {
        Some(K::Point) => "Point",
        Some(K::Line) => "Line",
        Some(K::Circle) => "Circle",
        Some(K::Arc) => "Arc",
        Some(K::Ellipse) => "Ellipse",
        Some(K::EllipticalArc) => "Elliptical arc",
        Some(K::BSpline) => "Bezier",
        Some(K::FitSpline) => "Spline",
        Some(K::Conic) => "Conic",
        None => "Entity",
    };
    format!("{k} {}", e.0)
}

/// Configuration parameter id of a sketch dimension.
fn configured_dim_key(cid: noshape_sketch::ConstraintId) -> String {
    format!("{}{}", noshape_model::regen::features::sketch::CONFIGURED_DIMENSION_PREFIX, cid.0)
}

/// A sketch context menu entry's action on the view.
type SketchMenuAction = Box<dyn Fn(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;

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
    Text,
}

/// Unit of an on-canvas pattern / transform value (`None`: an instance count).
fn pattern_value_dim(h: crate::sketch_mode::PatternHandle) -> Option<Dim> {
    use crate::sketch_mode::PatternHandle as H;
    match h {
        H::LinearStep | H::LinearSecondStep => Some(Dim::LENGTH),
        H::CircularAngle | H::Rotate => Some(Dim::ANGLE),
        H::Scale => Some(Dim::UNITLESS),
        _ => None,
    }
}

/// Option fields shown in the floating options box for a tool. Live
/// Onshape has none for fillet (arrow manipulator + radius box), polygon
/// (on-canvas side spinner), conic (rho box), patterns (on-canvas `Nx`
/// counts, spacing / angle values, arrows) or transform (triad with
/// rotation / scale values), so those have none.
fn tool_options(tool: Tool) -> &'static [(&'static str, &'static str, OptKind)] {
    match tool {
        Tool::Text => &[("text.text", "Text", OptKind::Text), ("text.height", "Height (click)", OptKind::Length)],
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
        let frame = self.display.sketches.iter().find(|s| s.feature == id).map(|s| s.frame).or(match plane {
            Some(Reference::Plane(DatumPlane::Top)) => Some(PlaneFrame::TOP),
            Some(Reference::Plane(DatumPlane::Front)) => Some(PlaneFrame::FRONT),
            Some(Reference::Plane(DatumPlane::Right)) => Some(PlaneFrame::RIGHT),
            _ => None,
        });
        let Some(frame) = frame else { return };
        match &mut self.sketch {
            Some(ed) if ed.feature == id => {
                // A new sketch plane places the frame again (Onshape
                // `onPlaneChange`).
                if ed.frame != frame {
                    ed.frame = frame;
                    ed.frame_bounds = None;
                }
            }
            _ => {
                self.sketch = Some(SketchEditor::new(id, frame));
                let length = self.doc.read(cx).session.doc.settings.units.length;
                if let Some(ed) = &mut self.sketch {
                    ed.apply_unit_defaults(length);
                }
                // A fresh sketch starts with no tool active (live
                // sketch/00, 01, 44: no toolbar button is down after the
                // plane is picked).
            }
        }
        self.place_sketch_frame(plane.as_ref(), cx);
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
        // Every sketch edit regenerates through here: record it for Ctrl+Z.
        self.draft_checkpoint(cx);
        let Some((studio, upto)) = self.effective_studio(cx) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
        let config = self.current_config(&studio);
        let editing_sketch = self.sketch.as_ref().map(|s| s.feature);
        self.regen_seq += 1;
        let result = {
            let mut regen = self.regen.lock();
            let document_variables = noshape_model::variables::element_variables(&self.doc.read(cx).session.doc, self.tab);
            let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
            regen.regenerate(&studio, &config, &units, &RegenOptions { upto, editing_sketch, document_variables, derived: Some(derived), ..Default::default() })
        };
        self.regen_task = None;
        self.context = Some(result.context.clone());
        self.update_conflicts();
        self.apply_display(Arc::new(result.display), window, cx);
        // The profile inspector follows every edit (resolved issues disappear).
        self.update_profile_issues(cx);
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
        // Points that are not fully constrained win over overlapping fully
        // constrained ones (Onshape selection priority).
        if let Some(res) = self.context.as_ref().and_then(|c| c.sketches.iter().find(|s| s.feature == ed.feature))
            && res.report.state(e) == Some(noshape_sketch::ConstraintState::FullyConstrained)
            && let Some(pos) = res.sketch.point(e)
        {
            let r = ed.world_per_px * 6.0;
            let better = res
                .sketch
                .entities()
                .filter(|x| x.id != e && x.kind() == noshape_sketch::EntityKind::Point)
                .filter(|x| res.report.state(x.id).is_some_and(|st| st != noshape_sketch::ConstraintState::FullyConstrained))
                .filter_map(|x| res.sketch.point(x.id).map(|q| (x.id, (q - pos).length())))
                .filter(|(_, d)| *d <= r)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((id, _)) = better {
                return Some(id);
            }
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
                ((s - cursor_s).length() <= radius_px).then_some(ExternalSnap::Edge { position: q, reference: r.clone() })
            }
            _ => None,
        }
    }

    /// Fixes the sketch frame like Onshape's sketch plane display
    /// (`updateBounds`): centred on the bounds centre of the sketch plane's
    /// entity (a plane, face or mate connector), or, when that is off screen,
    /// where the ray through the graphics area's centre meets the plane.
    fn place_sketch_frame(&mut self, plane: Option<&Reference>, cx: &App) {
        let Some(ed) = self.sketch.as_ref().filter(|ed| ed.frame_bounds.is_none()) else { return };
        let vp = self.viewport.read(cx);
        let size = vp.size();
        if size.y < 1.0 {
            return;
        }
        let frame = ed.frame;
        let center = plane.and_then(|r| {
            if let Some(pl) = self.display.planes.iter().find(|p| &p.reference == r) {
                return Some(pl.frame.origin);
            }
            if let Some(c) = self.display.connectors.iter().find(|c| &c.reference == r) {
                return Some(c.origin);
            }
            let body = self.display.bodies.iter().chain(&self.display.context_bodies).find(|b| b.face_refs.contains(r))?;
            let face = body.face_refs.iter().position(|f| f == r)?;
            let &(first, count) = body.mesh.face_ranges.get(face)?;
            let idx = body.mesh.indices.get(first as usize..(first + count) as usize)?;
            let mut lo = DVec3::splat(f64::INFINITY);
            let mut hi = DVec3::splat(f64::NEG_INFINITY);
            for &i in idx {
                let q = DVec3::from(body.mesh.positions[i as usize].map(f64::from));
                lo = lo.min(q);
                hi = hi.max(q);
            }
            (lo.x <= hi.x).then(|| (lo + hi) * 0.5)
        });
        let mut center = center.map_or(DVec2::ZERO, |c| frame.to_local(c));
        let on_screen = vp.project(frame.to_world(center)).is_some_and(|q| q.x >= 0.0 && q.x < size.x && q.y >= 0.0 && q.y < size.y);
        if !on_screen {
            // Onshape intersects only rays at least 15 degrees off the plane
            // (sin 15 = 0.258819).
            let (o, d) = vp.ray(size * 0.5);
            if d.normalize_or_zero().dot(frame.normal).abs() >= 0.258819
                && let Some(q) = frame.intersect_ray(o, d)
            {
                center = frame.to_local(q);
            }
        }
        let wpp = vp.camera.world_per_pixel(size.y);
        if let Some(ed) = &mut self.sketch {
            ed.world_per_px = wpp;
            ed.place_frame(center, size.y);
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
                // Live Onshape: the sketch's own plane (or face) is never
                // pre-highlighted while sketching on it.
                if r.as_ref().is_some_and(|r| self.is_own_sketch_plane(r, cx)) {
                    if self.hover.take().is_some() {
                        self.refresh_scene(false, cx);
                    }
                    if let Some(ed) = &mut self.sketch {
                        ed.hover_region = None;
                        ed.hover = None;
                    }
                    return true;
                }
                if let Some(ed) = &mut self.sketch {
                    ed.hover_region = match &r {
                        Some(Reference::SketchRegion { sketch, key }) if *sketch == ed.feature => Some(key.clone()),
                        _ => None,
                    };
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
                // The GPU hover result arrives after PointerMove. Refresh Use
                // here as well, so stopping on an edge still previews it.
                if self.sketch.as_ref().is_some_and(|s| s.tool == Tool::Use) {
                    self.hover = r;
                    self.update_use_preview();
                    self.refresh_scene(false, cx);
                    cx.notify();
                    return true;
                }
                false
            }
            ViewportEvent::PointerMove { position, modifiers } => {
                let Some(p) = to_plane(self, *position, cx) else { return false };
                let shift = modifiers.shift;
                let was_shift = self.sketch.as_ref().is_some_and(|s| s.shift_held);
                let alt = modifiers.alt;
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.pointer_move(data, p, shift, alt)) {
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
                    self.update_use_preview();
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
                // The GPU hover pick may lag behind a quick click: fall back to
                // a geometric pick for tools that act on existing entities.
                let near = |this: &Self, e: noshape_sketch::EntityId, cx: &App| -> bool {
                    let Some(ed) = this.sketch.as_ref() else { return false };
                    let Some(d) = this.sketch_data(cx) else { return false };
                    let r = ed.world_per_px * 10.0;
                    match d.sketch.point(e) {
                        Some(q) => (q - p).length() <= r,
                        // Text box lines stand for their glyphs; keep them.
                        None => d.text_of_entity(e).is_some() || d.sketch.curve(e).is_some_and(|c| c.closest(p).2 <= r),
                    }
                };
                // A stale hover (the pick lags behind fast moves) doesn't count.
                let hovered = self.hovered_sketch_entity(cx).filter(|e| near(self, *e, cx)).or_else(|| {
                    let ed = self.sketch.as_ref()?;
                    if ed.tool.draws() {
                        return None;
                    }
                    let r = ed.world_per_px * 5.0;
                    let data = self.sketch_data(cx)?;
                    match self.context.as_ref().and_then(|c| c.sketches.iter().find(|s| s.feature == ed.feature)) {
                        Some(s) => data.sketch.pick_with_report(p, r, &s.report),
                        None => data.sketch.pick(p, r),
                    }
                });
                // Dimensions and constraints to planes, model edges / vertices and
                // other sketches: a linked external reference entity stands in.
                let tool_now = self.sketch.as_ref().map(|s| s.tool);
                let hovered = match (hovered, self.hover.clone(), tool_now) {
                    (None, Some(r), Some(t @ (Tool::Dimension | Tool::Constraint(_) | Tool::Mirror)))
                        if matches!(r, Reference::Edge(_) | Reference::Vertex(_) | Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::Face(_))
                            || matches!(&r, Reference::SketchEntity { sketch, .. } if Some(*sketch) != self.sketch.as_ref().map(|s| s.feature)) =>
                    {
                        let pierce = t == Tool::Constraint(ConstraintTool::Pierce) && matches!(r, Reference::Edge(_));
                        // Mirror: only the mirror line may be model geometry.
                        if t == Tool::Mirror && self.sketch.as_ref().is_some_and(|s| s.has_mirror_axis()) { None } else { self.external_entity(&r, pierce, true, cx) }
                    }
                    (h, _, _) => h,
                };
                // A snap onto a model edge links the new point to it.
                if let Some(ExternalSnap::Edge { reference, .. }) = self.sketch.as_ref().filter(|s| s.tool.draws()).and_then(|s| s.external_snap.clone())
                    && let Some(id) = self.external_entity(&reference, false, true, cx)
                    && let Some(ed) = &mut self.sketch
                {
                    ed.link_edge_snap(id);
                }
                let (shift, dbl) = (modifiers.shift || modifiers.control, *click_count >= 2);
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_down(data, p, hovered, shift, dbl)) {
                    self.apply_outcome(out, window, cx);
                }
                self.sync_otf_input();
                self.sync_offset_input(window, cx);
                // A conic's rho box opens for typing right away (Onshape).
                if self.sketch.as_ref().and_then(|s| s.otf.as_ref()).is_some_and(|o| matches!(o.fields.first().map(|f| &f.kind), Some(noshape_sketch::ConstraintKind::Rho { .. })))
                    && self.ui_ref().is_none_or(|u| u.otf_input.is_none())
                {
                    self.open_otf_input(None, window, cx);
                }
                self.sync_dim_edit(window, cx);
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(p) = to_plane(self, *position, cx) else { return true };
                // Ctrl during a box selection removes from the selection.
                if let Some(ed) = &mut self.sketch {
                    ed.box_remove = window.modifiers().control;
                }
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_drag(data, p)) {
                    self.apply_outcome(out, window, cx);
                }
                true
            }
            ViewportEvent::ToolUp { position, .. } => {
                let Some(p) = to_plane(self, *position, cx) else { return true };
                if let Some(ed) = &mut self.sketch {
                    ed.box_remove = window.modifiers().control;
                }
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.tool_up(data, p)) {
                    self.apply_outcome(out, window, cx);
                }
                true
            }
            ViewportEvent::Click { .. } => true,
            ViewportEvent::ContextMenu { position, .. } => {
                self.open_sketch_context_menu(*position, window, cx);
                true
            }
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
        selection.extend(ed.selected_regions.iter().map(|k| Reference::SketchRegion { sketch: f, key: k.clone() }));
        accent.clear();
        // Live (sketch/12): what the Dimension tool measures shows selected
        // (amber) while the dimension is placed.
        let tool_picks = if ed.tool == Tool::Dimension { ed.tool_picks() } else { Vec::new() };
        selection.extend(tool_picks.iter().map(|e| Reference::SketchEntity { sketch: f, entity: e.0 }));
        accent.extend(ed.picked(data.as_ref()).iter().filter(|e| !tool_picks.contains(e)).map(|e| Reference::SketchEntity { sketch: f, entity: e.0 }));
        let Some(data) = data else { return Vec::new() };
        let units = self.doc.read(cx).session.doc.settings.units;
        let (lines, points) = ed.overlay(&data, &units);
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

    /// Use tool: preview the projection of the hovered edge / face edges.
    fn update_use_preview(&mut self) {
        let Some(ed) = &self.sketch else { return };
        if ed.tool != Tool::Use {
            if !ed.use_preview.is_empty()
                && let Some(ed) = &mut self.sketch
            {
                ed.use_preview.clear();
            }
            return;
        }
        let key = self.hover.clone();
        if self.ui_ref().is_some_and(|u| u.use_preview_for == key) {
            return;
        }
        let (frame, tol) = (ed.frame, ed.world_per_px);
        let mut out = Vec::new();
        // A curved face previews its silhouettes.
        if let (Some(r @ Reference::Face(_)), Some(ctx)) = (self.hover.clone(), self.context.clone())
            && self.curved_face(&r)
        {
            if let Some(face) = noshape_model::regen::measure::resolve_shapes(&ctx, &r).into_iter().next() {
                out.extend(noshape_model::regen::features::sketch::face_silhouettes(&face, &frame).iter().map(|c| c.tessellate(tol)));
            }
        } else if let Some(ctx) = self.context.clone() {
            let refs = if matches!(self.hover, Some(Reference::SketchEntity { .. })) {
                self.hover.clone().into_iter().collect()
            } else { self.hovered_edge_refs() };
            for er in refs {
                let shapes = noshape_model::regen::measure::resolve_shapes(&ctx, &er);
                match shapes.first().and_then(|e| noshape_model::regen::features::sketch::project_edge_any(e, &frame)) {
                    Some(noshape_model::regen::features::sketch::SectionCurve::Exact(c)) => out.push(c.tessellate(tol)),
                    Some(noshape_model::regen::features::sketch::SectionCurve::Sampled(p)) => out.push(p),
                    None => {}
                }
            }
        }
        if let Some(ed) = &mut self.sketch {
            ed.use_preview = out;
        }
        if let Some(ui) = self.ui_mut() {
            ui.use_preview_for = key;
        }
    }

    /// Project the hovered model edge (or all edges of a hovered face, or a vertex).
    fn use_hovered(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.hover.clone() else { return };
        let mut changed = false;
        match &r {
            Reference::Vertex(_) | Reference::SketchEntity { .. } | Reference::Edge(_) => {
                changed = self.external_entity(&r, false, false, cx).is_some();
            }
            Reference::Face(_) => {
                // A curved face gives its silhouette edges, a planar face its edges.
                let curved = self.curved_face(&r);
                if curved {
                    changed = self.external_entity(&r, false, false, cx).is_some();
                } else {
                    for er in self.hovered_edge_refs() {
                        changed |= self.external_entity(&er, false, false, cx).is_some();
                    }
                }
            }
            _ => {}
        }
        if changed {
            self.regen_sync(window, cx);
        } else if let Some(ed) = &mut self.sketch {
            ed.message = Some("That geometry can't be used in this sketch".into());
        }
        cx.notify();
    }

    /// Is the referenced face curved (not planar)?
    fn curved_face(&self, r: &Reference) -> bool {
        let Some(ctx) = self.context.as_ref() else { return false };
        noshape_model::regen::measure::resolve_shapes(ctx, r).first().is_some_and(|f| !f.is_planar())
    }

    /// The plane or face the edited sketch lies on (or a plane coincident with it).
    fn is_own_sketch_plane(&self, r: &Reference, cx: &App) -> bool {
        let Some(ed) = self.sketch.as_ref() else { return false };
        let own = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.query("sketchPlane").first().cloned());
        if own.as_ref() == Some(r) {
            return true;
        }
        if !matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_)) {
            return false;
        }
        self.reference_plane(r).is_some_and(|f| {
            f.normal.cross(ed.frame.normal).length() < 1e-9 && (f.origin - ed.frame.origin).dot(ed.frame.normal).abs() < 1e-9
        })
    }

    /// Plane frame of a plane-like reference (datum / feature plane, planar face).
    fn reference_plane(&self, r: &Reference) -> Option<PlaneFrame> {
        match r {
            Reference::Plane(_) | Reference::FeaturePlane(_) => self.display.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
            Reference::Face(_) => {
                let ctx = self.context.as_ref()?;
                let f = noshape_model::regen::measure::resolve_shapes(ctx, r).into_iter().next()?;
                let pl = f.face_plane().ok()??;
                let (n, x) = (pl.unit_normal(), pl.unit_x());
                Some(PlaneFrame { origin: pl.origin, x, y: n.cross(x), normal: n })
            }
            _ => None,
        }
    }

    /// The external sketch entity standing for a model reference, created
    /// on first use and linked so it follows the model: projected vertices,
    /// edges and other sketches' entities, plane traces, silhouettes of
    /// curved faces and (with `pierce`) the point where an edge crosses the
    /// sketch plane. `construction` marks reference-only geometry (picked
    /// by the dimension / constraint tools rather than used).
    pub(super) fn external_entity(&mut self, r: &Reference, pierce: bool, construction: bool, cx: &mut Context<Self>) -> Option<noshape_sketch::EntityId> {
        use noshape_model::regen::features::sketch as sk_feat;
        let ed = self.sketch.as_ref()?;
        let (frame, wpp, cursor) = (ed.frame, ed.world_per_px, ed.cursor.unwrap_or_default());
        let data = self.sketch_data(cx)?;
        let want_point = pierce || matches!(r, Reference::Vertex(_));
        if let Some((id, _)) = data.external_refs.iter().find(|(id, v)| *v == r && data.sketch.entity(noshape_sketch::EntityId(**id)).is_some_and(|e| (e.kind() == noshape_sketch::EntityKind::Point) == want_point)) {
            return Some(noshape_sketch::EntityId(*id));
        }
        let ctx = self.context.clone()?;
        let shape = noshape_model::regen::measure::resolve_shapes(&ctx, r).into_iter().next();
        enum Made {
            Point(DVec2),
            Curves(Vec<sk_feat::SectionCurve>),
        }
        let made = if let Some(pl) = self.reference_plane(r).filter(|_| !matches!(r, Reference::Face(_)) || !self.curved_face(r)) {
            let (p, d) = sk_feat::plane_trace(&pl, &frame)?;
            let half = (data.sketch.bbox().size().length() * 0.6).max(wpp * 150.0);
            let c = p + d * (cursor - p).dot(d);
            Made::Curves(vec![sk_feat::SectionCurve::Exact(noshape_sketch::Curve2::Line { a: c - d * half, b: c + d * half })])
        } else if matches!(r, Reference::Face(_)) {
            let face = shape?;
            Made::Curves(sk_feat::face_silhouettes(&face, &frame).into_iter().map(sk_feat::SectionCurve::Exact).collect())
        } else {
            let sh = shape?;
            if let Ok(p) = sh.vertex_point() {
                Made::Point(frame.to_local(p))
            } else if pierce {
                Made::Point(sk_feat::pierce_point(&sh, &frame)?)
            } else {
                Made::Curves(vec![sk_feat::project_edge_any(&sh, &frame)?])
            }
        };
        let r = r.clone();
        self.with_sketch(cx, move |_, data| {
            let mut first = None;
            match made {
                Made::Point(p) => {
                    let id = data.sketch.use_external_point(p);
                    data.external_refs.insert(id.0, r.clone());
                    first = Some(id);
                }
                Made::Curves(curves) => {
                    for id in sk_feat::add_section_curves(&mut data.sketch, &curves) {
                        if construction {
                            let _ = data.sketch.set_construction(id, true);
                        }
                        data.external_refs.insert(id.0, r.clone());
                        first.get_or_insert(id);
                    }
                }
            }
            data.sketch.solve();
            first
        })?
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
        if self.ui_ref().is_some_and(|u| u.dim_menu.is_some() || u.pattern_menu.is_some()) {
            self.close_dim_menu(cx);
            return;
        }
        let cancelled = self.sketch.as_mut().map(|s| s.cancel()).unwrap_or(false);
        if self.sketch.as_ref().is_some_and(|s| s.offset_pending.is_none())
            && let Some(ui) = self.ui_mut()
        {
            ui.offset_input = None;
        }
        if cancelled {
            self.dim_input = None;
            self.close_otf_input();
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
    pub(super) fn activate_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        match tool {
            Tool::Constraint(c) => self.constraint(c, window, cx),
            t => {
                self.set_tool(t, cx);
                if let Some(out) = self.with_sketch(cx, |ed, data| ed.apply_preselection(data)) {
                    self.apply_outcome(out, window, cx);
                    self.sync_dim_edit(window, cx);
                    self.sync_offset_input(window, cx);
                }
            }
        }
    }

    fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        remember_group_tool(tool);
        // Leaving the Slot tool commits its preview (live Onshape).
        if self.with_sketch(cx, |ed, data| ed.commit_slot(data)).is_some_and(|o| o.changed) {
            self.draft_checkpoint(cx);
            self.needs_regen = true;
        }
        if let Some(ed) = &mut self.sketch {
            ed.set_tool(tool);
        }
        if let Some(ui) = self.ui_mut() {
            ui.use_preview_for = None;
        }
        self.update_use_preview();
        self.close_otf_input();
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
                let units = self.doc.read(cx).session.doc.settings.units;
                let text = self.sketch_data(cx).and_then(|data| crate::sketch_mode::dimension_edit_text(&data, edit.constraint, &units)).unwrap_or(edit.text);
                let state = cx.new(|cx| InputState::new(window, cx).default_value(text));
                let cid = edit.constraint;
                // (Clicking the canvas closes the box; the tolerance panel next
                // to it may take the focus without closing it.)
                let sub = cx.subscribe_in(&state, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        let text = st.read(cx).value().to_string();
                        this.commit_dimension(cid, &text, window, cx);
                    }
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
        let units = self.doc.read(cx).session.doc.settings.units;
        // Enter on the untouched box keeps the exact value (the box shows it
        // rounded to 6 / 7 decimals).
        if self.sketch_data(cx).and_then(|data| crate::sketch_mode::dimension_edit_text(&data, cid, &units)).is_some_and(|shown| shown == text.trim()) {
            if let Some(ed) = &mut self.sketch {
                // A new dimension leaves nothing selected (see below).
                if ed.dim_edit.take().is_some_and(|d| d.created) {
                    ed.selection.clear();
                    ed.selected_constraints.clear();
                }
            }
            self.dim_input = None;
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        let vars = self.display.variables.clone();
        // A configured dimension edits the value of the active configuration.
        if let Some((input, option)) = self.dimension_configuration(cid, cx) {
            let (Some(feature), key, tab, text) = (self.sketch.as_ref().map(|s| s.feature), configured_dim_key(cid), self.tab, text.trim().to_string()) else { return };
            self.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, "Edit configured dimension", cx, |ps| {
                    if let Some(cp) = ps.configuration.configured.iter_mut().find(|c| c.feature == feature && c.param == key && c.input == input) {
                        cp.values.insert(option, noshape_model::params::ParamValue::Expr(noshape_units::ExprValue::new(text)));
                    }
                });
            });
            if let Some(ed) = &mut self.sketch {
                ed.dim_edit = None;
            }
            self.dim_input = None;
            self.regen_sync(window, cx);
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        let res = self.with_sketch(cx, |ed, data| {
            let created = ed.dim_edit.as_ref().is_some_and(|d| d.constraint == cid && d.created);
            let r = ed.apply_dimension(data, cid, text, &units, &vars);
            if r.is_ok() {
                ed.dim_edit = None;
                // Onshape (bundle: the dimension tool's commit ends in
                // `cleanUpAfterCommit`, which resets the selection list; the
                // entity highlights go with the edit box; live sketch/15): a
                // new dimension leaves nothing selected. Editing an existing
                // one keeps the selection as it was.
                if created {
                    ed.selection.clear();
                    ed.selected_constraints.clear();
                }
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
        let units = self.doc.read(cx).session.doc.settings.units;
        let text = self
            .edit
            .as_ref()
            .and_then(|e| {
                e.dialog.read(cx).sketch_data().and_then(|d| {
                    d.dimension_exprs.get(&cid.0).map(|x| x.text.clone()).or_else(|| {
                        let c = d.sketch.constraint(cid)?;
                        crate::sketch_mode::dimension_edit_text(d, c.id, &units)
                    })
                })
            })
            .unwrap_or_default();
        if let Some(ed) = &mut self.sketch {
            ed.dim_edit = Some(crate::sketch_mode::DimEdit { constraint: cid, text, created: false });
            // The label is replaced by the input, so it never sees the hover end.
            ed.hover_constraint = None;
        }
        self.sync_dim_edit(window, cx);
        cx.notify();
    }

    // ------------------------------------------------------------ on-the-fly dimension boxes

    /// Key press on the view while sketching: digits start typing into the
    /// on-the-fly dimension box (or set a polygon's side count).
    fn sketch_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = &ev.keystroke.modifiers;
        if m.control || m.platform || !self.focus.contains_focused(window, cx) || window.has_focused_input(cx) {
            return;
        }
        let Some(ed) = &mut self.sketch else { return };
        // Alt+arrows switch between the boxes of a rectangle / ellipse.
        if m.alt {
            if ed.otf.is_some() && matches!(ev.keystroke.key.as_str(), "left" | "right") {
                ed.otf_step(if ev.keystroke.key == "left" { -1 } else { 1 });
                self.refresh_scene(false, cx);
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }
        let ch = ev.keystroke.key_char.clone().unwrap_or_else(|| ev.keystroke.key.clone());
        if ch.chars().count() != 1 {
            return;
        }
        let Some(c) = ch.chars().next() else { return };
        // Polygon: the number of sides can be typed before the circumference click.
        if matches!(ed.tool, Tool::Polygon | Tool::CircumPolygon) && ed.otf.is_none() && c.is_ascii_digit() {
            if ed.polygon_typed.len() >= 2 {
                ed.polygon_typed.clear();
            }
            ed.polygon_typed.push(c);
            if let Ok(n) = ed.polygon_typed.parse::<usize>() {
                ed.polygon_sides = n.clamp(3, 50);
                ed.message = Some(format!("Sides: {}", ed.polygon_sides));
            }
            self.refresh_scene(false, cx);
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if ed.otf.is_none() || !(c.is_ascii_digit() || matches!(c, '.' | '-' | '=' | '(')) {
            return;
        }
        // "=#name" enters a variable ("#" alone is the view shortcut).
        let first = if c == '=' { String::new() } else { c.to_string() };
        self.open_otf_input(Some(first), window, cx);
        cx.stop_propagation();
    }

    /// Show an input for the active on-the-fly box: `first` replaces the
    /// suggested value (typing), `None` keeps it selected.
    fn open_otf_input(&mut self, first: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(data) = self.sketch_data(cx) else { return };
        let Some(ed) = &mut self.sketch else { return };
        let Some((_, suggested, _)) = ed.otf_boxes(&data, &units).into_iter().find(|b| b.2) else { return };
        ed.otf_typing = true;
        let init = first.clone().unwrap_or(suggested);
        let state = cx.new(|cx| InputState::new(window, cx).default_value(init.clone()));
        let sub = cx.subscribe_in(&state, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = st.read(cx).value().to_string();
                this.commit_otf_input(&text, window, cx);
            }
        });
        state.update(cx, |s, cx| {
            s.focus(window, cx);
            match &first {
                Some(f) => s.set_cursor_position(gpui_kit::component::input::Position { line: 0, character: f.chars().count() as u32 }, window, cx),
                None => s.select_all(window, cx),
            }
        });
        if let Some(ui) = self.ui_mut() {
            ui.otf_input = Some((state, sub));
        }
        cx.notify();
    }

    /// Click on a pattern's `Nx` label or an on-canvas value: type a new one.
    fn open_count_edit(&mut self, h: crate::sketch_mode::PatternHandle, init: String, window: &mut Window, cx: &mut Context<Self>) {
        let state = cx.new(|cx| InputState::new(window, cx).default_value(init));
        let sub = cx.subscribe_in(&state, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            match ev {
                InputEvent::PressEnter { .. } => {
                    let text = st.read(cx).value().to_string();
                    if let Some(dim) = pattern_value_dim(h) {
                        let units = this.doc.read(cx).session.doc.settings.units;
                        let vars = this.display.variables.clone();
                        match noshape_units::evaluate_as(text.trim().trim_start_matches('='), dim, &units, &vars) {
                            Ok(q) => {
                                if let Some(ed) = &mut this.sketch {
                                    ed.set_pattern_value(h, q.value);
                                }
                            }
                            Err(e) => {
                                this.message = Some((FeatureStatus::Error, e.to_string()));
                                cx.notify();
                                return;
                            }
                        }
                    } else if let Ok(n) = text.trim().parse::<usize>()
                        && let Some(ed) = &mut this.sketch
                    {
                        ed.set_pattern_count(h, n);
                    }
                    if let Some(ui) = this.ui_mut() {
                        ui.count_edit = None;
                    }
                    window.focus(&this.focus, cx);
                    this.refresh_scene(false, cx);
                    cx.notify();
                }
                InputEvent::Blur => {
                    if let Some(ui) = this.ui_mut() {
                        ui.count_edit = None;
                    }
                    cx.notify();
                }
                _ => {}
            }
        });
        state.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        if let Some(ui) = self.ui_mut() {
            ui.count_edit = Some((h, state, sub));
        }
        cx.notify();
    }

    fn close_otf_input(&mut self) {
        if let Some(ui) = self.ui_mut() {
            ui.otf_input = None;
        }
        if let Some(ed) = &mut self.sketch {
            ed.otf_typing = false;
        }
    }

    fn commit_otf_input(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let vars = self.display.variables.clone();
        let res = self.with_sketch(cx, |ed, data| ed.commit_otf(data, text, &units, &vars));
        match res {
            Some(Err(e)) => {
                self.message = Some((FeatureStatus::Error, e));
                cx.notify();
                return;
            }
            Some(Ok(())) => self.regen_sync(window, cx),
            None => {}
        }
        self.close_otf_input();
        // The next box (rectangle width, ellipse minor axis) is ready for typing.
        if self.sketch.as_ref().is_some_and(|s| s.otf.is_some()) {
            self.open_otf_input(None, window, cx);
        } else {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// Tab / Alt+arrows in the box: switch to the other box.
    fn otf_switch(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ed) = &mut self.sketch {
            ed.otf_step(delta);
        }
        self.close_otf_input();
        self.open_otf_input(None, window, cx);
        self.refresh_scene(false, cx);
    }

    /// Esc in the box: drop the boxes (the geometry stays undimensioned).
    fn cancel_otf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_otf_input();
        if let Some(ed) = &mut self.sketch {
            ed.otf = None;
        }
        window.focus(&self.focus, cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Drop the box input when the editor dropped its boxes (new click, tool change).
    fn sync_otf_input(&mut self) {
        let gone = self.sketch.as_ref().is_none_or(|s| s.otf.is_none());
        if gone && self.ui_ref().is_some_and(|u| u.otf_input.is_some()) {
            self.close_otf_input();
        }
    }

    // ------------------------------------------------------------ tool option fields

    fn option_text(&self, key: &str, cx: &App) -> String {
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(ed) = &self.sketch else { return String::new() };
        match key {
            "text.text" => ed.text_string.clone(),
            "text.height" => noshape_units::format_quantity(Quantity::new(ed.text_height, Dim::LENGTH), &units),
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
                if key == "text.text" && this.sketch.as_ref().is_some_and(|e| e.text_dialog_open()) {
                    this.accept_sketch_text(window, cx);
                    return;
                }
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
        let units = self.doc.read(cx).session.doc.settings.units;
        let vars = self.display.variables.clone();
        let t = text.trim();
        let val = match kind {
            OptKind::Length => noshape_units::evaluate_as(t, Dim::LENGTH, &units, &vars).ok().map(|q| q.value),
            OptKind::Text => Some(0.0),
        }
        .filter(|v| v.is_finite());
        let Some(ed) = &mut self.sketch else { return };
        let Some(v) = val else {
            ed.message = Some(format!("Invalid value \"{t}\""));
            cx.notify();
            return;
        };
        match key {
            "text.text" => ed.text_string = text.to_string(),
            "text.height" if v > 0.0 => ed.text_height = v,
            _ => {
                ed.message = Some("Value out of range".into());
                cx.notify();
                return;
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    // ------------------------------------------------------------ sketch text

    /// Font / bold / italic / flip in the Text dialog.
    fn set_text_style(&mut self, style: TextStyle, cx: &mut Context<Self>) {
        // The Text dialog edits its own copy; ✓ applies it (live Onshape).
        if let Some(ed) = &mut self.sketch {
            match style {
                TextStyle::Font(f) => ed.text_font = f.to_string(),
                TextStyle::Bold => ed.text_bold = !ed.text_bold,
                TextStyle::Italic => ed.text_italic = !ed.text_italic,
                TextStyle::FlipHorizontal => ed.text_flip_horizontal = !ed.text_flip_horizontal,
                TextStyle::FlipVertical => ed.text_flip_vertical = !ed.text_flip_vertical,
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
                        menu = menu.item(PopupMenuItem::new(f).checked(f == cur).on_click(move |_, _, cx| v.update(cx, |this, cx| this.set_text_style(TextStyle::Font(f), cx))));
                    }
                    menu
                }),
        );
        row = row
            .child(Button::new("sk-text-bold").label("B").ghost().xsmall().selected(bold).tooltip("Bold").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_text_style(TextStyle::Bold, cx))))
            .child(Button::new("sk-text-italic").label("I").ghost().xsmall().selected(italic).tooltip("Italic").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_text_style(TextStyle::Italic, cx))));
        if let Some((fh, fv)) = flips {
            row = row
                .child(Button::new("sk-text-fliph").label("Flip H").ghost().xsmall().selected(fh).tooltip("Flip about the horizontal center").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_text_style(TextStyle::FlipHorizontal, cx))))
                .child(Button::new("sk-text-flipv").label("Flip V").ghost().xsmall().selected(fv).tooltip("Flip about the vertical center").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_text_style(TextStyle::FlipVertical, cx))));
        }
        let _ = pal;
        row.into_any_element()
    }

    /// The Sketch dialog's display options (rows under "Disable imprinting")
    /// and its diagnostics button (footer, left of `?`), drawn inside the
    /// dialog as in Onshape. Set as the dialog's [`crate::feature_dialog::FeatureDialog::extra`].
    pub(super) fn sketch_dialog_extra(view: WeakEntity<Self>) -> crate::feature_dialog::DialogExtra {
        Arc::new(move |_, cx| {
            let Some(view) = view.upgrade() else { return (Vec::new(), None) };
            let this = view.read(cx);
            let Some(ed) = this.sketch.as_ref() else { return (Vec::new(), None) };
            let pal = p();
            let (sc, se, serr) = (ed.show_constraints, ed.show_expressions, ed.show_errors);
            let (cm_open, pi_open) = this.ui_ref().map(|u| (u.cm.is_some(), u.pi.is_some())).unwrap_or_default();
            let dialog = cx.entity().downgrade();
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut SketchEditor)| {
                let (view, dialog) = (view.clone(), dialog.clone());
                crate::ui::os_checkbox(id, label, on)
                    .on_click(move |_, _, cx| {
                        view.update(cx, |this, cx| {
                            if let Some(ed) = &mut this.sketch {
                                f(ed);
                            }
                            this.refresh_scene(false, cx);
                            cx.notify();
                        });
                        let _ = dialog.update(cx, |_, cx| cx.notify());
                    })
                    .into_any_element()
            };
            let rows = vec![
                check("sk-opt-constraints", "Show constraints", sc, |ed| ed.set_show_constraints(!ed.show_constraints)),
                check("sk-opt-expressions", "Show expressions", se, |ed| ed.show_expressions = !ed.show_expressions),
                check("sk-opt-errors", "Show errors", serr, |ed| ed.show_errors = !ed.show_errors),
            ];
            // 22 px outline button, 1 px `#aaa`, radius 3.5 (live).
            let diagnostics = Button::new("sk-diagnostics")
                .child(crate::assets::icon("sketch-diagnostics", 18., cx))
                .outline()
                .w(px(22.))
                .h(px(22.))
                .p_0()
                .rounded(px(3.5))
                .border_color(pal.border_strong)
                .selected(cm_open || pi_open)
                .tooltip("Sketch diagnostics tools")
                .dropdown_menu(move |m, _, _| {
                    let (v1, v2) = (view.clone(), view.clone());
                    m.item(PopupMenuItem::new(if pi_open { "Turn profile inspector off" } else { "Profile inspector…" }).on_click(move |_, _, cx| v1.update(cx, |this, cx| this.toggle_profile_inspector(cx))))
                        .item(PopupMenuItem::new(if cm_open { "Turn constraint manager off" } else { "Constraint manager…" }).on_click(move |_, _, cx| v2.update(cx, |this, cx| this.toggle_constraint_manager(cx))))
                })
                .into_any_element();
            (rows, Some(diagnostics))
        })
    }

    // ------------------------------------------------------------ sketch diagnostics

    pub(super) fn toggle_constraint_manager(&mut self, cx: &mut Context<Self>) {
        if let Some(ui) = self.ui_mut() {
            ui.cm = if ui.cm.is_some() { None } else { Some(ConstraintManager { auto_select: true, ..Default::default() }) };
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn toggle_profile_inspector(&mut self, cx: &mut Context<Self>) {
        let open = self.ui_ref().is_some_and(|u| u.pi.is_some());
        if let Some(ui) = self.ui_mut() {
            ui.pi = if open { None } else { Some(0) };
        }
        if open && let Some(ed) = &mut self.sketch {
            ed.issue_highlight = None;
            ed.issue_markers.clear();
        }
        self.update_profile_issues(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Recompute the profile inspector's issues (live: resolved issues disappear).
    pub(super) fn update_profile_issues(&mut self, cx: &mut Context<Self>) {
        let Some(focus) = self.ui_ref().and_then(|u| u.pi) else { return };
        let issues = self.sketch_data(cx).map(|d| d.sketch.profile_issues()).unwrap_or_default();
        let focus = focus.min(issues.len().saturating_sub(1));
        if let Some(ui) = self.ui_mut() {
            ui.pi = Some(focus);
        }
        if let Some(ed) = &mut self.sketch {
            ed.issue_markers = issues.iter().map(|i| i.position).collect();
            ed.issue_highlight = issues.get(focus).map(|i| (i.position, i.entities.clone()));
        }
        self.refresh_scene(false, cx);
    }

    /// Constraints listed by the constraint manager with the current filters.
    fn cm_rows(&self, data: &SketchData) -> Vec<(noshape_sketch::ConstraintId, &'static str, Vec<noshape_sketch::EntityId>)> {
        let (Some(ed), Some(cm)) = (self.sketch.as_ref(), self.ui_ref().and_then(|u| u.cm.as_ref())) else { return Vec::new() };
        let sk = &data.sketch;
        let mut rows: Vec<(noshape_sketch::ConstraintId, &'static str, Vec<noshape_sketch::EntityId>)> = sk
            .constraints()
            .filter(|c| !matches!(c.kind, noshape_sketch::ConstraintKind::LengthRatio { .. }) && !cm.removed.contains(&c.id.0))
            .filter(|c| cm.types.is_empty() || cm.types.contains(c.kind.name()))
            .filter(|c| {
                let external = c.kind.entities().iter().any(|e| sk.entity(*e).is_some_and(|x| x.external));
                cm.modes.is_empty() || cm.modes.contains(if external { "External" } else { "Internal" })
            })
            .filter(|c| {
                let st = if ed.conflicting.contains(&c.id) {
                    "Errors"
                } else if c.is_dimension() && !c.driving {
                    "Driven"
                } else {
                    "Solved"
                };
                cm.status.is_empty() || cm.status.contains(st)
            })
            // Without automatic selection the list follows the entity selection.
            .filter(|c| cm.auto_select || ed.selection.is_empty() || c.kind.entities().iter().any(|e| ed.selection.contains(e)))
            .map(|c| (c.id, c.kind.name(), c.kind.entities().to_vec()))
            .collect();
        if cm.by_entity {
            rows.sort_by_key(|r| (r.2.first().map(|e| e.0).unwrap_or(0), r.1));
        } else {
            rows.sort_by_key(|r| (r.1, r.0.0));
        }
        rows
    }

    fn cm_update(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut ConstraintManager)) {
        if let Some(cm) = self.ui_mut().and_then(|u| u.cm.as_mut()) {
            f(cm);
        }
        // Automatically select constraints: the listed constraints are selected.
        let auto = self.ui_ref().and_then(|u| u.cm.as_ref()).is_some_and(|c| c.auto_select);
        if auto && let Some(data) = self.sketch_data(cx) {
            let ids: Vec<noshape_sketch::ConstraintId> = self.cm_rows(&data).iter().map(|r| r.0).collect();
            if let Some(ed) = &mut self.sketch {
                ed.selected_constraints = ids;
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn render_constraint_manager(&self, data: &SketchData, cx: &mut Context<Self>) -> Option<AnyElement> {
        let cm = self.ui_ref()?.cm.as_ref()?;
        let pal = p();
        let chip = |id: String, label: &'static str, on: bool, cx: &mut Context<Self>, f: Box<dyn Fn(&mut ConstraintManager)>| {
            Button::new(SharedString::from(id)).label(label).xsmall().selected(on).when(!on, |b| b.ghost()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.cm_update(cx, |c| f(c))))
        };
        let types_present: std::collections::BTreeSet<&'static str> = data.sketch.constraints().filter(|c| !matches!(c.kind, noshape_sketch::ConstraintKind::LengthRatio { .. })).map(|c| c.kind.name()).collect();
        let mut col = v_flex().absolute().top(px(230.)).right(px(12.)).w(px(300.)).occlude().p_2().gap_1p5().rounded_md().bg(with_alpha(pal.panel_raised, 0.98)).border_1().border_color(pal.border_strong).shadow_lg();
        col = col.child(
            h_flex()
                .justify_between()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Constraint manager"))
                .child(Button::new("cm-close").icon(icon_px("close", 13.0, pal.text)).ghost().xsmall().tooltip("Turn constraint manager off").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_constraint_manager(cx)))),
        );
        let collapsed = cm.collapsed;
        col = col.child(
            h_flex()
                .gap_1()
                .child(Button::new("cm-filters").label(if collapsed { "▸ Filters" } else { "▾ Filters" }).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cm_update(cx, |c| c.collapsed = !c.collapsed))))
                .child(Button::new("cm-auto").label("Automatically select constraints").xsmall().selected(cm.auto_select).when(!cm.auto_select, |b| b.ghost()).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cm_update(cx, |c| c.auto_select = !c.auto_select)))),
        );
        if !collapsed {
            let mut types = h_flex().flex_wrap().gap_0p5();
            for t in types_present {
                let on = cm.types.contains(t);
                types = types.child(chip(format!("cm-t-{t}"), t, on, cx, Box::new(move |c| {
                    if !c.types.remove(t) {
                        c.types.insert(t);
                    }
                })));
            }
            col = col.child(div().text_xs().text_color(pal.text_muted).child("Type")).child(types);
            let mut modes = h_flex().gap_0p5();
            for m in ["Internal", "External"] {
                let on = cm.modes.contains(m);
                modes = modes.child(chip(format!("cm-m-{m}"), m, on, cx, Box::new(move |c| {
                    if !c.modes.remove(m) {
                        c.modes.insert(m);
                    }
                })));
            }
            col = col.child(div().text_xs().text_color(pal.text_muted).child("Mode")).child(modes);
            let mut status = h_flex().gap_0p5();
            for st in ["Driven", "Solved", "Errors"] {
                let on = cm.status.contains(st);
                status = status.child(chip(format!("cm-s-{st}"), st, on, cx, Box::new(move |c| {
                    if !c.status.remove(st) {
                        c.status.insert(st);
                    }
                })));
            }
            col = col.child(div().text_xs().text_color(pal.text_muted).child("Status")).child(status);
        }
        let by_entity = cm.by_entity;
        col = col.child(
            h_flex()
                .gap_0p5()
                .child(chip("cm-sort-c".into(), "Sort by constraint", !by_entity, cx, Box::new(|c| c.by_entity = false)))
                .child(chip("cm-sort-e".into(), "Sort by entity", by_entity, cx, Box::new(|c| c.by_entity = true))),
        );
        let rows = self.cm_rows(data);
        let ed = self.sketch.as_ref()?;
        let mut list = v_flex().id("cm-list").gap_0p5().max_h(px(260.)).overflow_y_scroll();
        for (cid, name, ents) in &rows {
            let cid = *cid;
            let conflict = ed.conflicting.contains(&cid);
            let selected = ed.selected_constraints.contains(&cid);
            let label = format!("{name}  ·  {}", ents.iter().map(|e| entity_name(&data.sketch, *e)).collect::<Vec<_>>().join(", "));
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("cm-row-{}", cid.0)))
                    .justify_between()
                    .px_1()
                    .rounded(px(3.))
                    .when(selected, |d| d.bg(rgb(0xf2ff80)))
                    .hover(|d| d.bg(pal.row_hover))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if let Some(ed) = &mut this.sketch {
                            let new = if *h { Some(cid) } else if ed.hover_constraint == Some(cid) { None } else { ed.hover_constraint };
                            if new != ed.hover_constraint {
                                ed.hover_constraint = new;
                                this.refresh_scene(false, cx);
                                cx.notify();
                            }
                        }
                    }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(ed) = &mut this.sketch {
                            if let Some(i) = ed.selected_constraints.iter().position(|c| *c == cid) {
                                ed.selected_constraints.remove(i);
                            } else {
                                ed.selected_constraints.push(cid);
                            }
                        }
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }))
                    .child(div().text_xs().text_color(if conflict { rgb(0xbd3039) } else { pal.text }).truncate().child(label))
                    .child(
                        h_flex()
                            .child(Button::new(SharedString::from(format!("cm-del-{}", cid.0))).icon(icon_px("delete", 12.0, pal.text_muted)).ghost().xsmall().tooltip("Delete constraint").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                if this.with_sketch(cx, |ed, data| ed.delete_constraint(data, cid)) == Some(true) {
                                    this.regen_sync(window, cx);
                                }
                            })))
                            .child(Button::new(SharedString::from(format!("cm-rm-{}", cid.0))).icon(icon_px("close", 11.0, pal.text_muted)).ghost().xsmall().tooltip("Remove selection").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.cm_update(cx, |c| {
                                    c.removed.insert(cid.0);
                                })
                            }))),
                    ),
            );
        }
        let ids: Vec<noshape_sketch::ConstraintId> = rows.iter().map(|r| r.0).collect();
        col = col
            .child(div().text_xs().text_color(pal.text_muted).child(format!("Entities and constraints ({})", rows.len())))
            .child(list)
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("cm-clear").label("Clear all").ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let all: Vec<u32> = this.sketch_data(cx).map(|d| d.sketch.constraints().map(|c| c.id.0).collect()).unwrap_or_default();
                        this.cm_update(cx, |c| c.removed.extend(all));
                    })))
                    .child(Button::new("cm-delete-all").label("Delete all").ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let ids = ids.clone();
                        let changed = this.with_sketch(cx, |ed, data| ids.iter().map(|c| ed.delete_constraint(data, *c)).filter(|d| *d).count() > 0);
                        if changed == Some(true) {
                            this.regen_sync(window, cx);
                        }
                    }))),
            );
        Some(col.into_any_element())
    }

    fn render_profile_inspector(&self, data: &SketchData, cx: &mut Context<Self>) -> Option<AnyElement> {
        let focus = self.ui_ref()?.pi?;
        let pal = p();
        let issues = data.sketch.profile_issues();
        let mut col = v_flex().absolute().top(px(230.)).right(px(12.)).w(px(260.)).occlude().p_2().gap_1().rounded_md().bg(with_alpha(pal.panel_raised, 0.98)).border_1().border_color(pal.border_strong).shadow_lg();
        col = col.child(
            h_flex()
                .justify_between()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Profile inspector"))
                .child(Button::new("pi-close").icon(icon_px("close", 13.0, pal.text)).ghost().xsmall().tooltip("Turn profile inspector off").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_profile_inspector(cx)))),
        );
        if issues.is_empty() {
            col = col.child(div().text_xs().text_color(pal.text_muted).child("No issues: every profile is closed."));
        }
        for (i, issue) in issues.iter().enumerate() {
            let names = issue.entities.iter().map(|e| entity_name(&data.sketch, *e)).collect::<Vec<_>>().join(", ");
            col = col.child(
                div()
                    .id(SharedString::from(format!("pi-{i}")))
                    .px_1()
                    .rounded(px(3.))
                    .cursor_pointer()
                    .when(i == focus, |d| d.bg(rgb(0xf2ff80)))
                    .hover(|d| d.bg(pal.row_hover))
                    .text_xs()
                    .text_color(pal.text)
                    .child(format!("{}: {names}", issue.message))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(ui) = this.ui_mut() {
                            ui.pi = Some(i);
                        }
                        this.update_profile_issues(cx);
                        cx.notify();
                    })),
            );
        }
        let n = issues.len();
        let step = move |d: isize| {
            move |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                if let Some(ui) = this.ui_mut()
                    && let Some(f) = ui.pi
                    && n > 0
                {
                    ui.pi = Some((f as isize + d).rem_euclid(n as isize) as usize);
                }
                this.update_profile_issues(cx);
                cx.notify();
            }
        };
        col = col.child(
            h_flex()
                .gap_1()
                .child(Button::new("pi-prev").label("Previous").ghost().xsmall().disabled(n == 0).on_click(cx.listener(step(-1))))
                .child(Button::new("pi-next").label("Next").ghost().xsmall().disabled(n == 0).on_click(cx.listener(step(1)))),
        );
        Some(col.into_any_element())
    }

    // ------------------------------------------------------------ offset distance box

    /// Open / close the offset distance box with the editor's pending offset.
    pub(super) fn sync_offset_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self.sketch.as_ref().is_some_and(|s| s.offset_pending.is_some());
        let open = self.ui_ref().is_some_and(|u| u.offset_input.is_some());
        if !pending {
            if open && let Some(ui) = self.ui_mut() {
                ui.offset_input = None;
            }
            return;
        }
        if open {
            return;
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let d = self.sketch.as_ref().map(|s| s.offset_distance).unwrap_or(0.0);
        let st = cx.new(|cx| InputState::new(window, cx).default_value(noshape_units::format_quantity(Quantity::new(d, Dim::LENGTH), &units)));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            let units = this.doc.read(cx).session.doc.settings.units;
            let vars = this.display.variables.clone();
            let parsed = noshape_units::evaluate_as(st.read(cx).value().trim(), Dim::LENGTH, &units, &vars).ok().map(|q| q.value).filter(|v| v.is_finite() && *v != 0.0);
            match ev {
                InputEvent::Change => {
                    // A negative value flips the direction.
                    if let (Some(v), Some(ed)) = (parsed, &mut this.sketch) {
                        ed.offset_distance = v.abs();
                        ed.offset_flip = v < 0.0;
                        this.refresh_scene(false, cx);
                    }
                }
                InputEvent::PressEnter { .. } => {
                    if let (Some(v), Some(ed)) = (parsed, &mut this.sketch) {
                        ed.offset_distance = v.abs();
                        ed.offset_flip = v < 0.0;
                    }
                    if let Some(out) = this.with_sketch(cx, |ed, data| ed.commit_offset(data)) {
                        this.apply_outcome(out, window, cx);
                    }
                    this.sync_offset_input(window, cx);
                    window.focus(&this.focus, cx);
                }
                _ => {}
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        if let Some(ui) = self.ui_mut() {
            ui.offset_input = Some((st, sub));
        }
        cx.notify();
    }

    /// User sketch shortcuts run before the default key bindings: a
    /// keystroke interceptor for the sketch session (only when the user
    /// defined some).
    fn accept_sketch_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(out) = self.with_sketch(cx, |ed, data| ed.accept_text(data)) {
            self.apply_outcome(out, window, cx);
        }
        if self.sketch.as_ref().is_none_or(|e| !e.text_dialog_open()) {
            window.focus(&self.focus, cx);
        }
    }

    fn render_text_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.sketch.as_ref()?.text_dialog_open() {
            if let Some(ui) = self.ui_mut() { ui.text_dialog_focused = false; }
            return None;
        }
        let input = self.option_input("text.text", OptKind::Text, window, cx)?;
        if self.ui_ref().is_some_and(|u| !u.text_dialog_focused) {
            let text = self.sketch.as_ref().map(|e| e.text_string.clone()).unwrap_or_default();
            input.update(cx, |s, cx| {
                s.set_value(text, window, cx);
                s.focus(window, cx);
                s.select_all(window, cx);
            });
            if let Some(ui) = self.ui_mut() { ui.text_dialog_focused = true; }
        }
        let ed = self.sketch.as_ref()?;
        let count = ed.text_string.chars().count();
        let style = self.text_style_row(&ed.text_font, ed.text_bold, ed.text_italic, Some((ed.text_flip_horizontal, ed.text_flip_vertical)), cx);
        let preview = ed.text_string.clone();
        let is_expr = ed.text_expression;
        let error = ed.message.clone();
        let pal = p();
        Some(v_flex().id("sk-text-dialog").absolute().left(px(342.)).top(px(12.)).w(px(404.)).occlude().bg(pal.panel).border_1().border_color(pal.border).shadow_lg()
            .child(h_flex().justify_between().p_2().border_b_1().border_color(pal.border)
                .child(div().text_sm().child("Text"))
                .child(h_flex()
                    .child(Button::new("sk-text-confirm").label("✓").primary().small().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_sketch_text(window, cx))))
                    .child(Button::new("sk-text-cancel").label("×").ghost().small().on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.sketch_cancel(cx);
                        window.focus(&this.focus, cx);
                    })))))
            .child(v_flex().p_3().gap_2().child(style)
                .child(div().p_2().min_h(px(50.)).bg(pal.panel_hover).text_sm().child(preview))
                .child(Input::new(&input))
                .child(h_flex().justify_between()
                    .child(div().text_xs().text_color(if count > 250 { rgb(0xaa252d) } else { pal.text_muted }).child(format!("{count}/250 characters")))
                    // Convert to expression / Convert to text (Onshape: text field context menu).
                    .child(Button::new("sk-text-expr").label(if is_expr { "Convert to text" } else { "Convert to expression" }).ghost().xsmall()
                        .tooltip("Expressions: \"text\" ~ #variable ~ roundToPrecision(#len / in, 3)")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(ed) = &mut this.sketch { ed.text_expression = !ed.text_expression; }
                            cx.notify();
                        }))))
                .when_some(error, |this, error| this.child(div().text_xs().text_color(rgb(0xaa252d)).child(error))))
            .into_any_element())
    }

    // ------------------------------------------------------------ insert DXF / image

    /// Insert DXF or DWG options: units, file origin, file.
    fn insert_dxf_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let Some(ed) = self.sketch.as_ref() else { return Vec::new() };
        let (scale, use_origin) = (ed.dxf_unit_scale, ed.dxf_use_origin);
        const UNITS: &[(&str, Option<f64>)] = &[("From file", None), ("Millimeter", Some(0.001)), ("Centimeter", Some(0.01)), ("Meter", Some(1.0)), ("Inch", Some(0.0254)), ("Foot", Some(0.3048)), ("Yard", Some(0.9144))];
        let label = UNITS.iter().find(|u| u.1 == scale).map(|u| u.0).unwrap_or("From file");
        let view = cx.entity();
        let units = Button::new("sk-dxf-units").label(label).ghost().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
            for (name, sc) in UNITS {
                let (v, sc) = (view.clone(), *sc);
                m = m.item(PopupMenuItem::new(*name).checked(sc == scale).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        if let Some(ed) = &mut this.sketch {
                            ed.dxf_unit_scale = sc;
                        }
                        cx.notify();
                    })
                }));
            }
            m
        });
        vec![
            h_flex().gap_2().justify_between().child(div().text_xs().text_color(pal.text_muted).child("Units")).child(units).into_any_element(),
            Button::new("sk-dxf-origin")
                .label("Use file origin position")
                .ghost()
                .xsmall()
                .selected(use_origin)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(ed) = &mut this.sketch {
                        ed.dxf_use_origin = !ed.dxf_use_origin;
                    }
                    cx.notify();
                }))
                .into_any_element(),
            Button::new("sk-dxf-file").label("Choose a DXF or DWG file…").icon(icon_px("import", 13.0, pal.text)).primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.choose_dxf(window, cx))).into_any_element(),
        ]
    }

    fn choose_dxf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let win = window.window_handle();
        crate::files::request_open_path("Insert DXF or DWG", "DXF or DWG drawing", &["dxf", "DXF", "dwg", "DWG"], window, cx, move |path, cx| {
            // The fallback chooser confirms inside a window update. Defer so
            // the same window is available when regeneration needs it.
            cx.defer(move |cx| {
                let _ = win.update(cx, |_, window, cx| view.update(cx, |this, cx| this.insert_dxf_file(&path, window, cx)));
            });
        });
    }

    pub(super) fn insert_dxf_file(&mut self, path: &std::path::Path, window: &mut Window, cx: &mut Context<Self>) {
        let parsed = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| noshape_sketch::dwg::parse_cad(&b).map_err(|e| e.to_string()));
        let drawing = match parsed {
            Ok(d) => d,
            Err(e) => {
                self.message = Some((FeatureStatus::Error, format!("Could not insert the drawing: {e}")));
                cx.notify();
                return;
            }
        };
        let Some(ed) = self.sketch.as_ref() else { return };
        let scale = ed.dxf_unit_scale.or(drawing.unit_scale).unwrap_or(0.001);
        let use_origin = ed.dxf_use_origin;
        let n = self.with_sketch(cx, |ed, data| {
            let ids = crate::sketch_mode::insert_dxf(data, &drawing, scale, use_origin);
            ed.selection = ids.clone();
            ids.len()
        });
        if n.unwrap_or(0) > 0 {
            if let Some(ed) = &mut self.sketch {
                ed.set_tool(Tool::Select);
                ed.message = Some(format!("Inserted {} entities from {}", n.unwrap_or(0), path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
            }
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    fn insert_image_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let pending = self.sketch.as_ref().and_then(|e| e.pending_image.as_ref().map(|p| p.0.clone()));
        let mut rows = vec![Button::new("sk-image-file").label("Choose an image…").icon(icon_px("image", 13.0, pal.text)).primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.choose_image(window, cx))).into_any_element()];
        rows.push(div().text_xs().text_color(pal.text_faint).child(match pending {
            Some(n) => format!("{n}: draw the image rectangle (the aspect ratio is locked)"),
            None => "PNG, JPEG, GIF or BMP".to_string(),
        }).into_any_element());
        rows
    }

    fn choose_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        crate::files::request_open_path("Insert image", "Image", &["png", "jpg", "jpeg", "gif", "bmp", "PNG", "JPG", "JPEG"], window, cx, move |path, cx| {
            view.update(cx, |this, cx| this.load_image_file(&path, cx));
        });
    }

    pub(super) fn load_image_file(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let loaded = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| image::load_from_memory(&b).map(|img| (b, img.width(), img.height())).map_err(|e| e.to_string()));
        match loaded {
            Ok((bytes, w, h)) if w > 0 && h > 0 => {
                if let Some(ed) = &mut self.sketch {
                    let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "image".into());
                    ed.pending_image = Some((name, bytes, w as f64 / h as f64));
                    ed.message = Some("Draw image rectangle.".into());
                }
            }
            Ok(_) => self.message = Some((FeatureStatus::Error, "The image is empty".into())),
            Err(e) => self.message = Some((FeatureStatus::Error, format!("Could not read the image: {e}"))),
        }
        cx.notify();
    }

    // ------------------------------------------------------------ bezier degree

    fn open_degree_edit(&mut self, id: noshape_sketch::EntityId, deg: usize, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).default_value(deg.to_string()));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = st.read(cx).value().trim().to_string();
                let res = match text.parse::<usize>() {
                    Ok(n) => this.with_sketch(cx, |_, data| {
                        let r = data.sketch.set_bezier_degree(id, n).map_err(|e| match e {
                            noshape_sketch::SketchError::InvalidGeometry(m) => m,
                            e => e.to_string(),
                        });
                        data.sketch.solve();
                        r
                    }),
                    Err(_) => Some(Err("Enter a whole number".into())),
                };
                match res {
                    Some(Ok(())) => {
                        if let Some(ui) = this.ui_mut() {
                            ui.degree_edit = None;
                        }
                        if let Some(ed) = &mut this.sketch {
                            ed.message = None;
                        }
                        this.regen_sync(window, cx);
                        window.focus(&this.focus, cx);
                    }
                    Some(Err(m)) => {
                        if let Some(ed) = &mut this.sketch {
                            ed.message = Some(m);
                        }
                    }
                    None => {}
                }
                cx.notify();
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        if let Some(ui) = self.ui_mut() {
            ui.degree_edit = Some((id, st, sub));
        }
        cx.notify();
    }

    // ------------------------------------------------------------ dimension tolerances

    fn set_tolerance(&mut self, cid: noshape_sketch::ConstraintId, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::sketch_feature::DimTolerance)) {
        let changed = self.with_sketch(cx, |_, data| {
            let t = data.dimension_tolerances.entry(cid.0).or_default();
            f(t);
            if t.kind == noshape_model::sketch_feature::ToleranceKind::None && t.precision.is_none() {
                data.dimension_tolerances.remove(&cid.0);
            }
            true
        });
        if changed == Some(true) {
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    /// The tolerance panel (type, deviations, precision) of the dimension
    /// whose value box is open (Onshape dimension context dialog).
    fn dimension_tolerance_panel(&mut self, data: &SketchData, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        use noshape_model::sketch_feature::ToleranceKind as T;
        let cid = self.dim_input.as_ref().map(|(c, _, _)| *c)?;
        if self.ui_ref()?.tol_open != Some(cid) {
            return None;
        }
        let c = data.sketch.constraint(cid)?;
        let dim = crate::sketch_mode::dim_unit(&c.kind);
        let tol = data.dimension_tolerances.get(&cid.0).copied().unwrap_or_default();
        let units = self.doc.read(cx).session.doc.settings.units;
        let pal = p();
        let field = |this: &mut Self, value: f64, upper: bool, window: &mut Window, cx: &mut Context<Self>| -> Option<Entity<InputState>> {
            let k = if upper { "tol.upper" } else { "tol.lower" };
            if let Some(st) = this.ui_ref().and_then(|u| u.opts.get(k).cloned()) {
                return Some(st);
            }
            let text = noshape_units::format_quantity(Quantity::new(value, dim), &units);
            let st = cx.new(|cx| InputState::new(window, cx).default_value(text));
            let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
                if let InputEvent::Change = ev {
                    let units = this.doc.read(cx).session.doc.settings.units;
                    let vars = this.display.variables.clone();
                    if let Ok(q) = noshape_units::evaluate_as(st.read(cx).value().trim(), dim, &units, &vars) {
                        this.set_tolerance(cid, window, cx, |t| if upper { t.upper = q.value.abs() } else { t.lower = q.value.abs() });
                    }
                }
            });
            let ui = this.ui_mut()?;
            ui.opts.insert(k, st.clone());
            ui.opt_subs.push(sub);
            Some(st)
        };
        let up = field(self, tol.upper, true, window, cx)?;
        let lo = field(self, tol.lower, false, window, cx)?;
        let view = cx.entity();
        let kind_label = |k: T| match k {
            T::None => "No tolerance",
            T::Symmetrical => "Symmetrical",
            T::Deviation => "Deviation",
            T::Limits => "Limits",
            T::Min => "Min",
            T::Max => "Max",
            T::Basic => "Basic",
        };
        let cur = tol.kind;
        let v1 = view.clone();
        let kinds = Button::new("sk-tol-kind").label(kind_label(cur)).ghost().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
            for k in [T::None, T::Symmetrical, T::Deviation, T::Limits, T::Min, T::Max, T::Basic] {
                let v = v1.clone();
                m = m.item(PopupMenuItem::new(kind_label(k)).checked(k == cur).on_click(move |_, window, cx| v.update(cx, |this, cx| this.set_tolerance(cid, window, cx, |t| t.kind = k))));
            }
            m
        });
        let prec = tol.precision;
        let v2 = view.clone();
        let precision = Button::new("sk-tol-prec")
            .label(match prec {
                None => "Workspace precision".to_string(),
                Some(n) => format!("{n} decimals"),
            })
            .ghost()
            .xsmall()
            .dropdown_caret(true)
            .dropdown_menu(move |mut m, _, _| {
                let v = v2.clone();
                m = m.item(PopupMenuItem::new("Workspace precision").checked(prec.is_none()).on_click(move |_, window, cx| v.update(cx, |this, cx| this.set_tolerance(cid, window, cx, |t| t.precision = None))));
                for n in 0..=6u8 {
                    let v = v2.clone();
                    let label = format!("{:.prec$}", 0.0, prec = n as usize);
                    m = m.item(PopupMenuItem::new(label).checked(prec == Some(n)).on_click(move |_, window, cx| v.update(cx, |this, cx| this.set_tolerance(cid, window, cx, |t| t.precision = Some(n)))));
                }
                m
            });
        let row = |label: &'static str, st: &Entity<InputState>| h_flex().gap_1().child(div().w(px(46.)).text_xs().text_color(pal.text_muted).child(label)).child(div().w(px(90.)).child(Input::new(st).xsmall()));
        let mut col = v_flex().occlude().p_1p5().gap_1().w(px(170.)).rounded_md().bg(pal.panel_raised).border_1().border_color(pal.border_strong).shadow_md();
        col = col.child(h_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Tolerance")).child(kinds));
        match cur {
            T::Symmetrical => col = col.child(row("±", &up)),
            T::Deviation | T::Limits => col = col.child(row("Upper", &up)).child(row("Lower", &lo)),
            _ => {}
        }
        col = col.child(h_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Precision")).child(precision));
        Some(col.into_any_element())
    }

    // ------------------------------------------------------------ dimension context menu

    fn close_dim_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(ui) = self.ui_mut()
            && (ui.dim_menu.take().is_some() | ui.pattern_menu.take().is_some())
        {
            cx.notify();
        }
    }

    fn dim_menu_action(&mut self, cid: noshape_sketch::ConstraintId, action: &str, window: &mut Window, cx: &mut Context<Self>) {
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
            "delete"
                if self.with_sketch(cx, |ed, data| ed.delete_constraint(data, cid)) == Some(true) => {
                    self.regen_sync(window, cx);
                }
            "tolerance" => {
                self.open_dim_edit(cid, window, cx);
                if let Some(ui) = self.ui_mut() {
                    ui.tol_open = Some(cid);
                    ui.opts.remove("tol.upper");
                    ui.opts.remove("tol.lower");
                }
            }
            "unconfigure" => {
                let (feature, key) = (self.sketch.as_ref().map(|s| s.feature), configured_dim_key(cid));
                if let Some(feature) = feature {
                    let tab = self.tab;
                    self.doc.update(cx, |doc, cx| {
                        doc.edit_studio(tab, "Unconfigure dimension", cx, |ps| ps.configuration.configured.retain(|c| !(c.feature == feature && c.param == key)));
                    });
                }
            }
            a if a.starts_with("configure:") => {
                let input = a.trim_start_matches("configure:").to_string();
                self.configure_dimension(cid, &input, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Configure dimension with a configuration input: each option starts
    /// with the dimension's current value (edit them in the configuration
    /// table, or edit the dimension with that configuration active).
    fn configure_dimension(&mut self, cid: noshape_sketch::ConstraintId, input: &str, cx: &mut Context<Self>) {
        use noshape_model::configuration::{ConfigInputKind, ConfiguredParam};
        use noshape_model::params::ParamValue;
        let Some(feature) = self.sketch.as_ref().map(|s| s.feature) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(text) = self.sketch_data(cx).and_then(|d| {
            d.dimension_exprs.get(&cid.0).map(|e| e.text.clone()).or_else(|| {
                let c = d.sketch.constraint(cid)?;
                Some(noshape_units::format_quantity_exact(Quantity::new(c.kind.value()?, crate::sketch_mode::dim_unit(&c.kind)), &units))
            })
        }) else {
            return;
        };
        let (input, key, tab) = (input.to_string(), configured_dim_key(cid), self.tab);
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(tab, "Configure dimension", cx, |ps| {
                let keys: Vec<String> = match ps.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
                    Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
                    Some(ConfigInputKind::Checkbox { .. }) => vec!["true".into(), "false".into()],
                    _ => return,
                };
                ps.configuration.configured.retain(|c| !(c.feature == feature && c.param == key));
                let values = keys.into_iter().map(|k| (k, ParamValue::Expr(noshape_units::ExprValue::new(text.clone())))).collect();
                ps.configuration.configured.push(ConfiguredParam { input, feature, param: key, values });
            });
        });
    }

    /// The configuration input and option a configured dimension currently
    /// takes its value from.
    fn dimension_configuration(&self, cid: noshape_sketch::ConstraintId, cx: &App) -> Option<(String, String)> {
        let feature = self.sketch.as_ref()?.feature;
        let studio = self.studio(cx)?;
        let key = configured_dim_key(cid);
        let cp = studio.configuration.configured.iter().find(|c| c.feature == feature && c.param == key)?;
        let values = self.current_config(studio);
        let option = studio.configuration.value_of(&values, &cp.input)?;
        Some((cp.input.clone(), option))
    }

    fn render_dim_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (cid, x, y) = self.ui_ref()?.dim_menu?;
        let (driving, is_dim) = self.sketch_data(cx).and_then(|d| d.sketch.constraint(cid).map(|c| (c.driving, c.is_dimension())))?;
        let item = |id: &str, label: &str, icon: &'static str, action: &str, cx: &mut Context<Self>| {
            let action = action.to_string();
            Button::new(SharedString::from(id.to_string()))
                .label(SharedString::from(label.to_string()))
                .icon(icon_px(icon, 13.0, pal.text))
                .ghost()
                .xsmall()
                .w_full()
                .justify_start()
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.dim_menu_action(cid, &action, window, cx)))
        };
        let mut menu = v_flex().absolute().left(px(x)).top(px(y)).occlude().w(px(170.)).p_1().gap_0p5().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.border_strong).shadow_lg();
        // Onshape: dimensions toggle driving/driven; constraints only delete.
        if is_dim {
            menu = menu.child(item("sk-dim-driving", if driving { "Change to driven dimension" } else { "Change to driving dimension" }, "driven-dimension", "driving", cx));
        }
        if is_dim {
            let allowed = self.sketch_data(cx).and_then(|d| d.sketch.constraint(cid).map(|c| crate::sketch_mode::tolerance_allowed(&c.kind))).unwrap_or(false);
            if allowed {
                menu = menu.child(item("sk-dim-tol", "Tolerance and precision…", "dimension", "tolerance", cx));
            }
            // Configure dimension with a list / checkbox configuration input.
            if self.dimension_configuration(cid, cx).is_some() {
                menu = menu.child(item("sk-dim-unconf", "Unconfigure dimension", "configuration", "unconfigure", cx));
            } else {
                use noshape_model::configuration::ConfigInputKind;
                let inputs: Vec<(String, String)> = self
                    .studio(cx)
                    .map(|s| s.configuration.inputs.iter().filter(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. })).map(|i| (i.id.clone(), i.name.clone())).collect())
                    .unwrap_or_default();
                for (id, name) in inputs {
                    menu = menu.child(item(&format!("sk-dim-conf-{id}"), &format!("Configure dimension with {name}"), "configuration", &format!("configure:{id}"), cx));
                }
            }
        }
        menu = menu.child(item("sk-dim-delete", if is_dim { "Delete dimension" } else { "Delete" }, "delete", "delete", cx));
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
        let units = self.doc.read(cx).session.doc.settings.units;
        let mut out = Vec::new();

        // Tool options box (creates inputs lazily, needs &mut self).
        let tool = self.sketch.as_ref().map(|s| s.tool).unwrap_or(Tool::Select);
        let fields = tool_options(tool);
        let mut rows: Vec<AnyElement> = Vec::new();
        // Text is edited in the Text dialog (double-click), as in Onshape.
        if tool == Tool::InsertDxf {
            rows.extend(self.insert_dxf_rows(cx));
        } else if tool == Tool::InsertImage {
            rows.extend(self.insert_image_rows(cx));
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

        // Tolerance / precision panel of the dimension being edited.
        let tol_panel = self.dimension_tolerance_panel(&data, window, cx);
        let mut tol_panel = tol_panel;

        let ed = self.sketch.as_ref().expect("sketch");
        let frame = ed.frame;
        let vp = self.viewport.read(cx);
        let project = |q: DVec2| vp.project(frame.to_world(q)).map(|s| (s.x as f32, s.y as f32));
        let editing = self.dim_input.as_ref().map(|(c, st, _)| (*c, st.clone()));

        if let Some((at, sides)) = ed.polygon_side_control()
            && let Some((x, y)) = project(at)
        {
            out.push(h_flex().absolute().left(px(x - 30.)).top(px(y - 10.)).occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(Button::new("sk-polygon-less").label("◀").ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(ed) = &mut this.sketch { ed.adjust_polygon_sides(-1); }
                    this.refresh_scene(false, cx); cx.notify();
                })))
                .child(div().text_sm().child(sides.to_string()))
                .child(Button::new("sk-polygon-more").label("▶").ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(ed) = &mut this.sketch { ed.adjust_polygon_sides(1); }
                    this.refresh_scene(false, cx); cx.notify();
                })))
                .into_any_element());
        }

        // ---- pattern instance counts (`3x`, click to type; live sketch/136–138)
        let count_edit = self.ui_ref().and_then(|u| u.count_edit.as_ref().map(|(h, st, _)| (*h, st.clone())));
        for (h, at, n) in ed.pattern_count_labels(&data.sketch) {
            let Some((x, y)) = project(at) else { continue };
            if let Some((eh, st)) = &count_edit
                && *eh == h
            {
                out.push(div().absolute().left(px(x - 4.)).top(px(y - 12.)).w(px(56.)).occlude().child(Input::new(st).xsmall()).into_any_element());
                continue;
            }
            out.push(
                div()
                    .id(SharedString::from(format!("sk-count-{h:?}")))
                    .absolute()
                    .left(px(x))
                    .top(px(y - 10.))
                    .text_size(px(16.))
                    .text_color(rgb(0x333333))
                    .cursor_pointer()
                    .whitespace_nowrap()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_count_edit(h, n.to_string(), window, cx)))
                    .child(format!("{n}x"))
                    .into_any_element(),
            );
        }
        for (h, at, d) in ed.pattern_value_labels(&data.sketch) {
            let Some((x, y)) = project(at) else { continue };
            if let Some((eh, st)) = &count_edit
                && *eh == h
            {
                out.push(div().absolute().left(px(x - 40.)).top(px(y - 12.)).w(px(80.)).occlude().child(Input::new(st).xsmall()).into_any_element());
                continue;
            }
            let dim = pattern_value_dim(h).unwrap_or(Dim::UNITLESS);
            let (text, init) = if dim == Dim::UNITLESS {
                (format!("{}", (d * 1000.0).round() / 1000.0), format!("{d}"))
            } else {
                // Labels read like dimensions (no unit); the field shows the unit.
                let full = noshape_units::format_quantity(Quantity::new(d, dim), &units);
                let bare = if dim == Dim::ANGLE { full.replace(" deg", "°") } else { full.split(' ').next().unwrap_or_default().to_string() };
                (bare, full)
            };
            out.push(
                div()
                    .id(SharedString::from(format!("sk-spacing-{h:?}")))
                    .absolute()
                    .left(px(x - 20.))
                    .top(px(y - 10.))
                    .text_size(px(16.))
                    .text_color(rgb(0x333333))
                    .cursor_pointer()
                    .whitespace_nowrap()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_count_edit(h, init.clone(), window, cx)))
                    .child(text)
                    .into_any_element(),
            );
        }

        // ---- one-line measurement of the selection, left of the evaluation
        // tools at the bottom right (live: `Length: 1.003 in`, monospace value)
        if let Some((label, value)) = ed.selection_readout(&data, &units) {
            out.push(
                h_flex()
                    .absolute()
                    .bottom_0()
                    .right(px(120.))
                    .h(px(30.))
                    .gap_1()
                    .items_center()
                    .text_size(px(13.))
                    .text_color(rgb(0x333333))
                    .whitespace_nowrap()
                    .child(format!("{label}:"))
                    .child(div().font_family("monospace").child(value))
                    .into_any_element(),
            );
        }

        // ---- the sketch frame's name ("Sketch 1", light blue, top-left inside)
        {
            let (lo, hi) = ed.sketch_box();
            let name = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.name.clone()).unwrap_or_default();
            // Onshape `updateText`: drawn in the sketch plane, a 20 px label
            // (`PLANE_TEXT_HEIGHT_PX`, 2 px padding) 5 px inside the top-left
            // corner (`PLANE_TEXT_MARGIN`), shrunk to fit a small frame.
            let mut polys = Vec::new();
            let wpp = ed.world_per_px;
            let width = super::plane_text(&name).width;
            let (text_w, text_h) = ((width * 16.5 + 4.0) * wpp, 24.0 * wpp);
            let (avail_w, avail_h) = (hi.x - lo.x - 10.0 * wpp, hi.y - lo.y - 10.0 * wpp);
            let scale = if text_w > avail_w || text_h > avail_h { (avail_w / text_w).min(avail_h / text_h) } else { 1.0 };
            if scale > 0.0 {
                // `in_plane_text` puts the ascender 0.25 asc right of and
                // 0.3 asc below the point it is given.
                let (asc, inset) = (16.5 * wpp * scale, (5.0 + 2.0 * scale) * wpp);
                let top_left = DVec2::new(lo.x + inset - 0.25 * asc, hi.y - inset + 0.3 * asc);
                super::in_plane_text(vp, &name, frame.to_world(top_left), frame.x, frame.y, asc, &mut polys);
            }
            if !polys.is_empty() {
                out.push(super::paint_paths(polys, rgb(crate::sketch_mode::SKETCH_BOX_COLOR).into()));
            }
        }

        // ---- dimension labels
        let configured: std::collections::HashSet<u32> = data.sketch.constraints().filter(|c| c.is_dimension() && self.dimension_configuration(c.id, cx).is_some()).map(|c| c.id.0).collect();
        let regenerated = self.context.as_ref().and_then(|c| c.sketches.iter().find(|s| s.feature == ed.feature)).map(|s| s.sketch.clone());
        let beside = ed.labels_beside_line(&data);
        for (cid, pos, mut text, driving) in ed.dimension_labels(&data, &units) {
            let Some((x, y)) = project(pos) else { continue };
            // Steep dimension lines: the text sits right of the line (live sketch/17).
            let x = if beside.contains(&cid) { x + crate::sketch_mode::label_width_px(&text) as f32 * 0.5 + 8.0 } else { x };
            let is_configured = configured.contains(&cid.0);
            // Configured dimensions show the active configuration's value.
            if is_configured
                && let Some(c) = regenerated.as_ref().and_then(|s| s.constraint(cid))
                && let Some(v) = c.kind.value()
            {
                text = crate::sketch_mode::dim_value_text_tol(&c.kind, v, &units, data.dimension_tolerances.get(&cid.0));
            }
            if let Some((ec, st)) = &editing
                && *ec == cid
            {
                let allowed = data.sketch.constraint(cid).is_some_and(|c| crate::sketch_mode::tolerance_allowed(&c.kind));
                out.push(
                    // Onshape's dimension box (live DOM): a 13 px drag strip
                    // with "◻◻◻" over a 209 x 34 white panel (1 px `#ccc`,
                    // shadow) holding the disabled ">" expander, the 100 px
                    // underlined value field, the ± toggle and 22 px ✓ / ✗.
                    // The box is centered on the label position (live sketch/12-13).
                    v_flex()
                        .absolute()
                        .left(px(x - 104.0))
                        .top(px(y - 30.0))
                        .w(px(209.))
                        .occlude()
                        .child(
                            div()
                                .h(px(13.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb(0xffffff))
                                .border_1()
                                .border_color(rgb(0xcccccc))
                                .text_size(px(9.33))
                                .text_color(pal.text)
                                .child("◻◻◻"),
                        )
                        .child(
                            h_flex()
                                .h(px(34.))
                                .pb(px(5.))
                                .bg(rgb(0xffffff))
                                .border_1()
                                .border_t_0()
                                .border_color(rgb(0xcccccc))
                                .shadow_md()
                                .child(div().w(px(20.)).flex().justify_center().child(crate::ui::glyph("ui-chevron-right", 12., rgb(0xd7d7d7))))
                                .child(gpui_kit::base::input::input_frame(
                                    div()
                                        .w(px(100.))
                                        .h(px(24.))
                                        .border_b_1()
                                        .border_color(pal.accent)
                                        // Live sketch/141: right-aligned, ~13.33 px, ending 7 px from the field's edge.
                                        .child(Input::new(st).xsmall().appearance(false).text_size(px(13.33)).text_align(TextAlign::Right).h(px(24.))),
                                ))
                                .child(
                                    div()
                                        .id("sk-dim-tol")
                                        .ml(px(5.))
                                        .size(px(20.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(3.))
                                        // Live dom/dimension-input-box.txt: `parameter-state-toggle`,
                                        // the tolerance-precision icon (our redrawn `tolerance`) at opacity 0.5.
                                        .opacity(0.5)
                                        .when(allowed, |d| {
                                            d.cursor_pointer().hover(|d| d.bg(pal.panel_hover)).tooltip(crate::ui::tooltip("Tolerance and precision")).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                if let Some(ui) = this.ui_mut() {
                                                    ui.tol_open = if ui.tol_open == Some(cid) { None } else { Some(cid) };
                                                    ui.opts.remove("tol.upper");
                                                    ui.opts.remove("tol.lower");
                                                }
                                                cx.notify();
                                            }))
                                        })
                                        .child(crate::assets::icon("tolerance", 20., cx)),
                                )
                                .child(div().flex_1())
                                .child(
                                    div()
                                        .id("sk-dim-ok")
                                        .size(px(22.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(4.))
                                        .bg(pal.accept)
                                        .cursor_pointer()
                                        .hover(|d| d.bg(rgb(0x74ce3a)))
                                        .child(Icon::new(gpui_kit::component::IconName::Check).with_size(px(16.)).text_color(rgb(0xffffff)))
                                        .on_click(cx.listener({
                                            let st = st.clone();
                                            move |this, _: &ClickEvent, window, cx| {
                                                let text = st.read(cx).value().to_string();
                                                this.commit_dimension(cid, &text, window, cx);
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .id("sk-dim-cancel")
                                        .ml(px(4.))
                                        .mr(px(6.))
                                        .size(px(22.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(4.))
                                        .bg(rgb(0xffffff))
                                        .border_1()
                                        .border_color(rgb(0xcccccc))
                                        .cursor_pointer()
                                        .hover(|d| d.bg(pal.panel_hover))
                                        .child(crate::assets::icon("dialog-cancel", 22., cx))
                                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                            this.sketch_cancel(cx);
                                            window.focus(&this.focus, cx);
                                        })),
                                ),
                        )
                        .into_any_element(),
                );
                if let Some(panel) = tol_panel.take() {
                    out.push(div().absolute().left(px(x - 70.0)).top(px(y + 22.0)).child(panel).into_any_element());
                }
                continue;
            }
            let selected = ed.selected_constraints.contains(&cid);
            let hovered = ed.hover_constraint == Some(cid);
            let conflict = ed.show_errors && ed.conflicting.contains(&cid);
            // Onshape: 16 px black value with no box at rest (the dimension
            // line is broken around it); hover / selected get a rounded
            // `#d9bfa0` / `#d9a566` box; driven values are `#666`.
            let est_w = crate::sketch_mode::label_width_px(&text) as f32 + 8.0;
            let (bg, fg) = if selected {
                (Some(rgb(0xd9a566)), rgb(0x000000))
            } else if hovered {
                (Some(rgb(0xd9bfa0)), rgb(0x000000))
            } else if conflict {
                (None, rgb(crate::theme::viewport::SKETCH_OVER))
            } else {
                (None, if driving { rgb(0x000000) } else { rgb(crate::theme::viewport::DIM_DRIVEN) })
            };
            out.push(
                div()
                    .id(SharedString::from(format!("dim-{}", cid.0)))
                    .absolute()
                    .left(px(x - est_w * 0.5))
                    .top(px(y - 11.0))
                    .h(px(22.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .when_some(bg, |d, bg| d.bg(bg))
                    // Configured dimensions have a gold outline (Onshape).
                    .when(is_configured, |d| d.border_1().border_color(rgb(0xffc20e)))
                    .text_size(px(16.))
                    .text_color(fg)
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

        // ---- bezier degree labels (double-click to change the degree)
        let degree_edit = self.ui_ref().and_then(|u| u.degree_edit.as_ref().map(|(e, st, _)| (*e, st.clone())));
        for e in data.sketch.entities() {
            let Some(deg) = data.sketch.bezier_degree(e.id) else { continue };
            let noshape_sketch::Geometry::BSpline { control, .. } = &e.geometry else { continue };
            let pts: Vec<DVec2> = control.iter().filter_map(|c| data.sketch.point(*c)).collect();
            let Some(mid) = pts.get(pts.len() / 2).copied() else { continue };
            let Some((x, y)) = project(mid) else { continue };
            let id = e.id;
            if let Some((ee, st)) = &degree_edit
                && *ee == id
            {
                out.push(div().absolute().left(px(x + 10.0)).top(px(y - 26.0)).w(px(60.)).occlude().child(Input::new(st).xsmall()).into_any_element());
                continue;
            }
            out.push(
                div()
                    .id(SharedString::from(format!("bez-deg-{}", id.0)))
                    .absolute()
                    .left(px(x + 10.0))
                    .top(px(y - 24.0))
                    .px_1()
                    .rounded(px(3.))
                    .bg(with_alpha(pal.panel_raised, 0.85))
                    .border_1()
                    .border_color(pal.border)
                    .text_xs()
                    .text_color(pal.text)
                    .cursor_pointer()
                    .child(format!("Deg {deg}"))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if ev.click_count >= 2 {
                            this.open_degree_edit(id, deg, window, cx);
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
            let conflict = ed.show_errors && ed.conflicting.contains(&cid);
            // Onshape constraint boxes (§1.9): white, hover `#fcda7f`, selected
            // `#fab601`; blue tints for external references; red boxes with
            // white glyphs for conflicts. No outline.
            let (bg, fg) = match (conflict, g.external, selected, hovered) {
                (true, _, true, _) => (rgb(0xbd3039), rgb(0xffffff)),
                (true, _, false, true) => (rgb(0xaa252d), rgb(0xffffff)),
                (true, _, false, false) => (rgb(0xd66e74), rgb(0xffffff)),
                (false, true, true, _) => (rgb(0x90cef1), rgb(0x000000)),
                (false, true, false, true) => (rgb(0xb2ddf6), rgb(0x000000)),
                (false, true, false, false) => (rgb(0xdef1fb), rgb(0x000000)),
                (false, false, true, _) => (rgb(0xfab601), rgb(0x000000)),
                (false, false, false, true) => (rgb(0xfcda7f), rgb(0x000000)),
                _ => (rgb(0xffffff), rgb(0x000000)),
            };
            out.push(
                div()
                    .id(SharedString::from(format!("glyph-{}-{}", cid.0, g.entity.0)))
                    .absolute()
                    .left(px(x + dx))
                    .top(px(y + dy))
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.))
                    .bg(bg)
                    .shadow_xs()
                    .cursor_pointer()
                    .child(icon_px(g.icon, 14.0, fg))
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
                        cx.stop_propagation();
                        if let Some(ui) = this.ui_mut() {
                            ui.dim_menu = Some((cid, x + dx + 6.0, y + dy + 18.0));
                        }
                        cx.notify();
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

        // ---- pattern glyphs (right-click: Edit pattern)
        for pat in &data.patterns {
            let ents: Vec<noshape_sketch::EntityId> = pat.instances.iter().flatten().map(|e| noshape_sketch::EntityId(*e)).filter(|e| data.sketch.kind(*e).is_some_and(|k| k != noshape_sketch::EntityKind::Point)).collect();
            let hovered = ed.hover.is_some_and(|h| ents.contains(&h) || pat.seeds.contains(&h.0));
            if !(show_all || hovered) {
                continue;
            }
            let Some(first) = ents.first() else { continue };
            let Some(anchor) = data.sketch.curve(*first).map(|c| {
                let (a, b) = c.domain();
                c.point((a + b) * 0.5)
            }) else {
                continue;
            };
            let Some((x, y)) = project(anchor) else { continue };
            let pid = pat.id;
            let icon = if pat.circular { "sketch-circular-pattern" } else { "sketch-linear-pattern" };
            out.push(
                div()
                    .id(SharedString::from(format!("pattern-glyph-{pid}")))
                    .absolute()
                    .left(px(x + 6.0))
                    .top(px(y + 6.0))
                    .size(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .bg(rgb(0xf2f2f2))
                    .border_1()
                    .border_color(rgb(0x9aa5af))
                    .cursor_pointer()
                    .child(icon_px(icon, 13.0, rgb(0x000000)))
                    .on_mouse_down(MouseButton::Right, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if let Some(ui) = this.ui_mut() {
                            ui.pattern_menu = Some((pid, x + 12.0, y + 26.0));
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        // ---- inference icon next to the cursor
        if ed.tool.draws()
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

        // ---- on-the-fly dimension boxes of the geometry just created
        let otf_input = self.ui_ref().and_then(|u| u.otf_input.as_ref().map(|(st, _)| st.clone()));
        let otf_labels = ed.otf_labels(&data, &units);
        for (i, (pos, value, active)) in ed.otf_boxes(&data, &units).into_iter().enumerate() {
            let value = otf_labels.get(i).cloned().unwrap_or(value);
            let Some((x, y)) = project(pos) else { continue };
            if active && let Some(st) = &otf_input {
                out.push(
                    div()
                        .absolute()
                        .left(px(x - 50.0))
                        .top(px(y - 12.0))
                        .w(px(100.))
                        .occlude()
                        .key_context("SketchEntry")
                        .on_action(cx.listener(|this, _: &SketchEntryNext, window, cx| this.otf_switch(1, window, cx)))
                        .on_action(cx.listener(|this, _: &SketchEntryPrev, window, cx| this.otf_switch(-1, window, cx)))
                        .on_action(cx.listener(|this, _: &SketchEntryCancel, window, cx| this.cancel_otf(window, cx)))
                        .child(Input::new(st).xsmall())
                        .into_any_element(),
                );
                continue;
            }
            // Live (sketch/06): the other values stay plain like the rubber
            // band's (13 px `#449ccd`); the active one is a 24 px box, white
            // 90%, 1 px `#788188` border and gray 14 px text.
            if !active {
                let w = crate::sketch_mode::label_width_px(&value) as f32 * 13.0 / 14.0;
                out.push(div().absolute().left(px(x - w * 0.5)).top(px(y - 9.0)).text_size(px(13.)).text_color(rgb(0x449ccd)).whitespace_nowrap().child(value).into_any_element());
                continue;
            }
            let est_w = crate::sketch_mode::label_width_px(&value) as f32 * 14.0 / 16.0 + 12.0;
            out.push(
                div()
                    .absolute()
                    .left(px(x - est_w * 0.5))
                    .top(px(y - 12.0))
                    .h(px(24.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .rounded(px(1.))
                    .bg(with_alpha(rgb(0xffffff), 0.9))
                    .border_1()
                    .border_color(rgb(0x788188))
                    .text_size(px(14.))
                    .text_color(rgb(0x788188))
                    .whitespace_nowrap()
                    .child(value)
                    .into_any_element(),
            );
        }

        // ---- live values of the shape being drawn (live: `#449ccd`, 13 px, no box)
        for (pos, text) in ed.preview_values(&data, &units) {
            let Some((x, y)) = project(pos) else { continue };
            let w = crate::sketch_mode::label_width_px(&text) as f32 * 13.0 / 14.0;
            out.push(div().absolute().left(px(x - w * 0.5)).top(px(y - 9.0)).text_size(px(13.)).text_color(rgb(0x449ccd)).whitespace_nowrap().child(text).into_any_element());
        }

        // ---- value of the dimension being placed (follows the cursor)
        if let Some((kind, pos, v)) = ed.dimension_preview(&data)
            && let Some((x, y)) = project(pos)
        {
            // Live (sketch/12): like a placed value, 16 px black in the
            // dimension line's gap at the cursor, no box.
            let text = crate::sketch_mode::dim_value_text(&kind, v / crate::sketch_mode::dim_display_scale(&kind), &units);
            let est_w = crate::sketch_mode::label_width_px(&text) as f32 + 8.0;
            out.push(div().absolute().left(px(x - est_w * 0.5)).top(px(y - 11.0)).h(px(22.)).px(px(4.)).flex().items_center().text_size(px(16.)).text_color(rgb(0x000000)).whitespace_nowrap().child(text).into_any_element());
        }

        // ---- offset distance box at the direction arrow
        if let Some(st) = self.ui_ref().and_then(|u| u.offset_input.as_ref().map(|(st, _)| st.clone()))
            && let Some((_, tip)) = ed.offset_arrow(&data.sketch)
            && let Some((x, y)) = project(tip)
        {
            out.push(
                div()
                    .absolute()
                    .left(px(x + 10.0))
                    .top(px(y - 12.0))
                    .w(px(100.))
                    .occlude()
                    .key_context("SketchEntry")
                    .on_action(cx.listener(|this, _: &SketchEntryCancel, window, cx| {
                        if let Some(ed) = &mut this.sketch {
                            ed.offset_pending = None;
                        }
                        this.sync_offset_input(window, cx);
                        window.focus(&this.focus, cx);
                        this.refresh_scene(false, cx);
                    }))
                    .child(Input::new(&st).xsmall())
                    .into_any_element(),
            );
        }

        // ---- tool options box: a small floating panel at the top of the
        // viewport, right of the feature dialog (clear of the view toggles).
        if !rows.is_empty() && !ed.text_dialog_open() {
            let (icon, title) = (tool.icon(), tool.label());
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
            if tool.is_pattern() {
                let n = ed.selection.len();
                let hint = if n == 0 { "Select the entities".to_string() } else { format!("{n} entit{} · click in empty space or press Enter to accept, Esc to cancel", if n == 1 { "y" } else { "ies" }) };
                panel = panel.child(div().text_xs().text_color(pal.text_faint).child(hint));
            }
            out.push(panel.into_any_element());
        }

        if let Some(panel) = self.render_text_dialog(window, cx) {
            out.push(panel);
        }
        if let Some(panel) = self.render_constraint_manager(&data, cx) {
            out.push(panel);
        }
        if let Some(panel) = self.render_profile_inspector(&data, cx) {
            out.push(panel);
        }
        if let Some((pid, x, y)) = self.ui_ref().and_then(|u| u.pattern_menu) {
            out.push(
                v_flex()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .occlude()
                    .w(px(150.))
                    .p_1()
                    .rounded_md()
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(pal.border_strong)
                    .shadow_lg()
                    .child(Button::new("sk-pattern-edit").label("Edit pattern").ghost().xsmall().w_full().justify_start().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(ui) = this.ui_mut() {
                            ui.pattern_menu = None;
                        }
                        let data = this.sketch_data(cx);
                        if let (Some(ed), Some(data)) = (&mut this.sketch, data) {
                            ed.edit_pattern(&data, pid);
                        }
                        this.refresh_scene(false, cx);
                        cx.notify();
                    })))
                    .into_any_element(),
            );
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

    pub(super) fn render_sketch_toolbar(&mut self, viewport_w: f32, cx: &mut Context<Self>) -> AnyElement {
        let Some(ed) = &self.sketch else { return div().into_any_element() };
        let active = ed.tool;
        let construction = ed.construction;
        // Constraint buttons not applicable to the selection are disabled.
        let selection = ed.selection.clone();
        let data = self.sketch_data(cx);
        // Onshape's sketch toolbar has the feature toolbar's metrics (§3.10).
        let tool_btn = |tool: Tool, cx: &mut Context<Self>| {
            crate::ui::with_tool_tooltip(
                SharedString::from(format!("skt-{tool:?}")),
                tool.label(),
                crate::ui::tool_button(SharedString::from(format!("sk-{tool:?}")), tool.icon(), active == tool, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate_tool(tool, window, cx))),
            )
        };
        let sep = || crate::ui::tool_separator().into_any_element();
        let group = |bar: gpui_kit::Div, group: &'static [Tool], cx: &mut Context<Self>| -> gpui_kit::Div {
            // The active member, else the last used one, is the group's face.
            let face = group.iter().copied().find(|t| *t == active).unwrap_or_else(|| group_face(group));
            if group.len() == 1 {
                return bar.child(tool_btn(face, cx));
            }
            // Live: a split tool (icon + 18 px caret, no gap, group hover)
            // whose dropdown lists the group with shortcut chips.
            let main = crate::ui::with_tool_tooltip(
                SharedString::from(format!("skt-{face:?}")),
                face.label(),
                crate::ui::split_tool_button(SharedString::from(format!("sk-{face:?}")), face.icon(), active == face, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate_tool(face, window, cx))),
            );
            let view = cx.entity();
            let members: Vec<Tool> = group.to_vec();
            let caret = crate::ui::tool_caret(SharedString::from(format!("skm-{:?}", group[0])), cx).dropdown_menu(move |mut menu, window, _| {
                menu = crate::ui::fit_tool_menu(menu.min_w(px(180.)), window);
                for t in &members {
                    let (v, t) = (view.clone(), *t);
                    let label: &'static str = t.label();
                    let shortcut = label.rsplit_once(" (").map(|(_, k)| k.trim_end_matches(')')).filter(|k| k.len() <= 7);
                    let name = label.split(" (").next().unwrap_or_default();
                    menu = menu.item(crate::ui::menu_tool_item(name, t.icon(), shortcut).on_click(move |_, window, cx| v.update(cx, |this, cx| this.activate_tool(t, window, cx))));
                }
                menu
            });
            bar.child(crate::ui::tool_group(SharedString::from(format!("skg-{:?}", group[0])), main, caret))
        };
        let action_btn = |id: &'static str, icon: &'static str, tip: &'static str, action: Box<dyn Action>, cx: &mut Context<Self>| {
            crate::ui::with_tool_tooltip(SharedString::from(format!("{id}-tt")), tip, crate::ui::tool_button(id, icon, false, cx).on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx)))
        };
        let mut bar = h_flex().flex_initial().min_w_0().overflow_hidden().gap(px(2.));
        bar = bar
            .child(action_btn("sk-undo", "undo", "Undo (cmd+z/ctrl+z)", Box::new(Undo), cx))
            .child(action_btn("sk-redo", "redo", "Redo (ctrl+y/shift+cmd+z)", Box::new(Redo), cx))
            .child(sep())
            // Extrude / Revolve accept the sketch and pre-select all its regions.
            .child(action_btn("sk-extrude", "extrude", "Extrude (Shift+E)", Box::new(Extrude), cx))
            .child(action_btn("sk-revolve", "revolve", "Revolve (Shift+W)", Box::new(Revolve), cx))
            .child(sep());
        // Live (sketch/420-421): when the tools don't fit, the drawing tools
        // (with Use, Intersection, Construction), the edit tools and the
        // constraints collapse into "Line ▾", "Sketch fillet ▾" and
        // "Coincident ▾"; Dimension stays.
        let (cell, caret) = (crate::ui::TOOL_CELL + 2., crate::ui::TOOL_CARET_W);
        let groups_w = |gs: &[&[Tool]]| gs.iter().map(|g| cell + if g.len() > 1 { caret } else { 0. }).sum::<f32>();
        let full = 4. * cell + 5. * 4. + groups_w(DRAW_TOOLS) + groups_w(USE_TOOLS) + cell + groups_w(EDIT_TOOLS) + groups_w(INSERT_TOOLS) + cell + CONSTRAINT_TOOLS.len() as f32 * cell;
        if full > viewport_w - crate::ui::RAIL_W - 4. - 172. {
            let draw: Vec<Tool> = DRAW_TOOLS.iter().chain(USE_TOOLS).flat_map(|g| g.iter().copied()).collect();
            let edit: Vec<Tool> = EDIT_TOOLS.iter().chain(INSERT_TOOLS).flat_map(|g| g.iter().copied()).collect();
            let cons: Vec<Tool> = CONSTRAINT_TOOLS.iter().map(|c| Tool::Constraint(*c)).collect();
            for (members, construction_item) in [(draw, true), (edit, false)] {
                bar = self.collapsed_sketch_group(bar, members, construction_item, active, cx).child(sep());
            }
            bar = bar.child(tool_btn(Tool::Dimension, cx)).child(sep());
            bar = self.collapsed_sketch_group(bar, cons, false, active, cx);
            return crate::ui::toolbar()
                .child(bar)
                .child(crate::ui::search_tools_toggle("sk-search", self.palette.is_some()).ml(px(10.)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_palette(window, cx))))
                .into_any_element();
        }
        for g in DRAW_TOOLS {
            bar = group(bar, g, cx);
        }
        for g in USE_TOOLS {
            bar = group(bar, g, cx);
        }
        bar = bar.child(crate::ui::with_tool_tooltip(
            "sk-construction-tt",
            "Construction (Q)",
            crate::ui::tool_button("sk-construction", "construction", construction, cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_construction(window, cx))),
        ));
        bar = bar.child(sep());
        for g in EDIT_TOOLS {
            bar = group(bar, g, cx);
        }
        for g in INSERT_TOOLS {
            bar = group(bar, g, cx);
        }
        bar = bar.child(sep()).child(tool_btn(Tool::Dimension, cx)).child(sep());
        for c in CONSTRAINT_TOOLS {
            let tool = Tool::Constraint(*c);
            let enabled = selection.is_empty() || data.as_ref().is_some_and(|d| crate::sketch_mode::constraint_applicable(&d.sketch, *c, &selection));
            // Not applicable to the selection: Onshape dims the tool to 40%.
            bar = bar.child(crate::ui::with_tool_tooltip(
                SharedString::from(format!("skt-{tool:?}")),
                tool.label(),
                crate::ui::tool_button(SharedString::from(format!("sk-{tool:?}")), tool.icon(), active == tool, cx)
                    .opacity(if enabled { 1.0 } else { 0.4 })
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if enabled {
                            this.activate_tool(tool, window, cx);
                        }
                    })),
            ));
        }
        // Onshape has no "Finish" button in the toolbar: the sketch dialog's
        // green check (or Enter) closes the sketch.
        crate::ui::toolbar()
            .child(bar)
            .child(crate::ui::search_tools_toggle("sk-search", self.palette.is_some()).ml(px(10.)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_palette(window, cx))))
            .into_any_element()
    }

    /// A collapsed sketch toolbar group (narrow window): the active member,
    /// else the first, with every member in its dropdown (and Construction
    /// at the end of the drawing tools).
    fn collapsed_sketch_group(&mut self, bar: gpui_kit::Div, members: Vec<Tool>, construction_item: bool, active: Tool, cx: &mut Context<Self>) -> gpui_kit::Div {
        let face = members.iter().copied().find(|t| *t == active).unwrap_or(members[0]);
        let key = format!("{:?}", members[0]);
        let main = crate::ui::with_tool_tooltip(
            SharedString::from(format!("skt-c-{key}")),
            face.label(),
            crate::ui::split_tool_button(SharedString::from(format!("sk-c-{face:?}")), face.icon(), active == face, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate_tool(face, window, cx))),
        );
        let view = cx.entity();
        let caret = crate::ui::tool_caret(SharedString::from(format!("skm-c-{key}")), cx).dropdown_menu(move |mut menu, window, _| {
            menu = crate::ui::fit_tool_menu(menu.min_w(px(180.)), window);
            for t in &members {
                let (v, t) = (view.clone(), *t);
                // "Line (L)": the name, and the shortcut as chips.
                let label: &'static str = t.label();
                let shortcut = label.rsplit_once(" (").map(|(_, k)| k.trim_end_matches(')')).filter(|k| k.len() <= 7);
                let name = label.split(" (").next().unwrap_or_default();
                menu = menu.item(crate::ui::menu_tool_item(name, t.icon(), shortcut).on_click(move |_, window, cx| v.update(cx, |this, cx| this.activate_tool(t, window, cx))));
            }
            if construction_item {
                let v = view.clone();
                menu = menu.item(crate::ui::menu_tool_item("Construction", "construction", Some("q")).on_click(move |_, window, cx| v.update(cx, |this, cx| this.toggle_construction(window, cx))));
            }
            menu
        });
        bar.child(crate::ui::tool_group(SharedString::from(format!("skg-c-{key}")), main, caret))
    }

    fn paste_sketch_entities(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        if self.with_sketch(cx, |ed, data| ed.paste_clipboard(data, &text)) == Some(true) {
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    /// Select other: a list of the sketch entities under the cursor.
    fn open_select_other(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ed) = self.sketch.as_ref() else { return };
        let (Some(c), wpp, frame) = (ed.cursor, ed.world_per_px, ed.frame) else { return };
        let Some(data) = self.sketch_data(cx) else { return };
        let r = wpp * 8.0;
        let mut cands: Vec<(f64, noshape_sketch::EntityId)> = data
            .sketch
            .entities()
            .filter_map(|e| {
                let d = match data.sketch.point(e.id) {
                    Some(q) => (q - c).length(),
                    None => data.sketch.curve(e.id)?.closest(c).2,
                };
                (d <= r).then_some((d, e.id))
            })
            .collect();
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        if cands.is_empty() {
            return;
        }
        let Some(pos) = self.viewport.read(cx).project(frame.to_world(c)) else { return };
        let names: Vec<(noshape_sketch::EntityId, String)> = cands.iter().map(|(_, e)| (*e, entity_name(&data.sketch, *e))).collect();
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _window, _cx| {
            for (e, name) in &names {
                let (v, e) = (view.clone(), *e);
                m = m.item(crate::os_menu::OsMenuItem::new(name.clone()).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.close_context_menu(cx);
                        if let Some(ed) = &mut this.sketch {
                            if let Some(i) = ed.selection.iter().position(|x| *x == e) {
                                ed.selection.remove(i);
                            } else {
                                ed.selection.push(e);
                            }
                        }
                        this.refresh_scene(false, cx);
                        cx.notify();
                    })
                }));
            }
            m
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((point(px(pos.x as f32), px(pos.y as f32)), menu, sub));
        cx.notify();
    }

    /// Right-click in an active sketch: the sketch context menu (Onshape).
    fn open_sketch_context_menu(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let hovered = self.hovered_sketch_entity(cx);
        let model_edge = matches!(self.hover, Some(Reference::Edge(_) | Reference::Face(_)));
        // Model geometry under the cursor: its part (Hide / Isolate / Make
        // transparent) and, for a straight edge, Align view (live sketch/204).
        let model_ref = self.hover.clone().filter(|_| model_edge);
        let model_body = model_ref.as_ref().and_then(|r| self.display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r)).map(|b| (b.id.clone(), b.name.clone())));
        let linear_model_edge = model_ref.as_ref().filter(|r| self.linear_edge_direction(r).is_some()).cloned();
        let sketch_name = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.name.clone()).unwrap_or_else(|| "sketch".into());
        let own_plane = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.query("sketchPlane").first().cloned());
        let Some(ed) = &mut self.sketch else { return };
        // The entity under the cursor joins the selection the menu acts on.
        if let Some(h) = hovered
            && !ed.selection.contains(&h)
        {
            ed.selection.push(h);
        }
        let n = ed.selection.len();
        let tool = ed.tool;
        let drawing_spline = matches!(tool, Tool::Spline | Tool::Bezier) && !ed.clicks_empty();
        let tool_active = tool != Tool::Select && !drawing_spline;
        let tool_name = tool.label().split(" (").next().unwrap_or("tool").to_lowercase();
        // A single selected line: Align view with vertical axis (tool menu).
        let (sel0, frame) = ((n == 1).then(|| ed.selection[0]), ed.frame);
        let single_line = sel0.zip(self.sketch_data(cx)).and_then(|(e, d)| match d.sketch.curve(e)? {
            noshape_sketch::Curve2::Line { a, b } => Some(frame.to_world(b) - frame.to_world(a)),
            _ => None,
        });
        let has_clip = cx.read_from_clipboard().and_then(|c| c.text()).is_some_and(|t| t.contains("noshape_sketch_entities"));
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, window, cx| {
            let v = view.clone();
            let item = |label: &str, f: SketchMenuAction| {
                let v = v.clone();
                crate::os_menu::OsMenuItem::new(label.to_string()).on_click(move |_, window, cx| v.update(cx, |this, cx| {
                    this.close_context_menu(cx);
                    f(this, window, cx)
                }))
            };
            // A tool is active: "Escape <tool>" first, and the first Esc
            // exits the tool with the menu left open (live sketch/400–406).
            if tool_active {
                m = m.item(item(&format!("Escape {tool_name}"), Box::new(|this, _, cx| this.set_tool(Tool::Select, cx))));
                let v = v.clone();
                m = m.on_first_escape(move |_, cx| v.update(cx, |this, cx| this.set_tool(Tool::Select, cx)));
            }
            m = m.item(item(&format!("Confirm {sketch_name}"), Box::new(|this, window, cx| this.accept_edit(false, window, cx))));
            m = m.separator();
            if drawing_spline {
                if tool == Tool::Spline {
                    m = m.item(item("Confirm spline", Box::new(|this, w, cx| {
                        if let Some(out) = this.with_sketch(cx, |ed, data| ed.confirm_spline(data)) {
                            this.apply_outcome(out, w, cx);
                        }
                    })));
                } else {
                    m = m.item(item("Escape bezier", Box::new(|this, _, cx| this.sketch_cancel(cx))));
                }
                m = m.separator();
            }
            // Live Onshape order (sketch/31, 37, 204): Confirm | Use | copy &
            // transform | analysis | selection | zoom & view | delete.
            if model_edge && tool != Tool::Use {
                m = m.item(item("Use", Box::new(|this, w, cx| this.use_hovered(w, cx)))).separator();
            }
            if model_edge && n == 0 {
                if let Some((b, name)) = model_body.clone() {
                    let (b1, b2, b3) = (b.clone(), b.clone(), b);
                    m = m.item(item(&format!("Hide {name}"), Box::new(move |this, _, cx| {
                        this.set_part_hidden(&b1, true, cx);
                        this.hover = None;
                    })));
                    m = m.item(item("Curve/surface analysis…", Box::new(|this, w, cx| this.toggle_analysis(super::analysis::Analysis::Curvature, w, cx))));
                    m = m.item(item("Isolate…", Box::new(move |this, w, cx| {
                        let previous = std::mem::replace(&mut this.selection, vec![Reference::Body(b2.clone())]);
                        this.hover = None;
                        this.isolate(super::behaviors::IsolateMode::Isolate, w, cx);
                        this.selection = previous;
                    })));
                    m = m.item(item("Make transparent…", Box::new(move |this, w, cx| {
                        let previous = std::mem::replace(&mut this.selection, vec![Reference::Body(b3.clone())]);
                        this.hover = None;
                        this.isolate(super::behaviors::IsolateMode::Transparent, w, cx);
                        this.selection = previous;
                    })));
                } else {
                    m = m.item(item("Curve/surface analysis…", Box::new(|this, w, cx| this.toggle_analysis(super::analysis::Analysis::Curvature, w, cx))));
                }
                m = m.separator();
                m = m.item(item("Clear selection", Box::new(|this, _, cx| {
                    this.selection.clear();
                    if let Some(ed) = &mut this.sketch {
                        ed.selection.clear();
                    }
                    this.refresh_scene(false, cx);
                    cx.notify();
                })));
                if let Some(r) = model_ref.clone() {
                    let criteria: &[noshape_model::select::Criterion] = if matches!(r, Reference::Face(_)) { &noshape_model::select::Criterion::FACE } else { &noshape_model::select::Criterion::SKETCH_EDGE };
                    let vs = v.clone();
                    m = m.submenu("Select", window, cx, move |mut sm, _, _| {
                        for c in criteria {
                            let (v, r, c) = (vs.clone(), r.clone(), *c);
                            sm = sm.item(crate::os_menu::OsMenuItem::new(c.label()).on_click(move |_, _, cx| v.update(cx, |this, cx| {
                                this.close_context_menu(cx);
                                this.select_related(&r, c, cx);
                            })));
                        }
                        let v = vs.clone();
                        sm.item(crate::os_menu::OsMenuItem::new("Create selection…").on_click(move |_, _, cx| v.update(cx, |this, cx| {
                            this.close_context_menu(cx);
                            this.open_create_selection_at(this.ui.menu_pos, cx);
                        })))
                    });
                }
                m = m.item(item("Select other…", Box::new(|this, window, cx| this.open_select_other(window, cx))));
                m = m.separator();
                m = m.item(item("Zoom to fit", Box::new(|_, window, cx| window.dispatch_action(Box::new(ZoomToFit), cx))));
                if let Some(r) = model_ref.clone() {
                    m = m.item(item("Zoom to selection", Box::new(move |this, _, cx| {
                        let previous = std::mem::replace(&mut this.selection, vec![r.clone()]);
                        this.zoom_to_selection(cx);
                        this.selection = previous;
                    })));
                }
                m = m.item(item("View normal to sketch plane", Box::new(|this, _, cx| {
                    let prev = std::mem::take(&mut this.selection);
                    this.view_normal(cx);
                    this.selection = prev;
                })));
                if let Some(r) = linear_model_edge.clone() {
                    m = m.item(item("Align view with vertical axis", Box::new(move |this, _, cx| this.align_view_vertical(&r, cx))));
                }
                return m;
            }
            if n > 0 {
                m = m.item(item("Copy sketch entities", Box::new(|this, _, cx| {
                    let text = this.sketch_data(cx).and_then(|d| this.sketch.as_ref()?.copy_selection(&d));
                    if let Some(text) = text {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                })));
                if !tool_active {
                    m = m.item(item("Transform sketch entities", Box::new(|this, _, cx| this.set_tool(Tool::Transform, cx))));
                }
            } else {
                m = m.item(item("Copy sketch", Box::new(|this, _, cx| {
                    let text = this.sketch_data(cx).and_then(|d| {
                        let ed = this.sketch.as_mut()?;
                        let previous = std::mem::replace(&mut ed.selection, d.sketch.entities().map(|e| e.id).collect());
                        let text = ed.copy_selection(&d);
                        ed.selection = previous;
                        text
                    });
                    if let Some(text) = text {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                })));
            }
            if has_clip {
                m = m.item(item("Paste sketch entities", Box::new(|this, w, cx| this.paste_sketch_entities(w, cx))));
            }
            m = m.separator();
            if n == 0 {
                m = m.item(item("Show all", Box::new(|this, _, cx| this.show_everything(cx))));
            }
            if !tool_active {
                m = m.item(item("Curve/surface analysis…", Box::new(|this, w, cx| this.toggle_analysis(super::analysis::Analysis::Curvature, w, cx))));
            }
            if n > 0 {
                if let Some(plane) = own_plane.clone().filter(|_| !tool_active) {
                    m = m.item(item("Section view…", Box::new(move |this, w, cx| this.section_through(plane.clone(), w, cx))));
                }
                m = m.separator();
                m = m.item(item("Clear selection", Box::new(|this, _, cx| {
                    if let Some(ed) = &mut this.sketch {
                        ed.selection.clear();
                        ed.selected_constraints.clear();
                    }
                    this.refresh_scene(false, cx);
                    cx.notify();
                })));
                let vs = v.clone();
                m = m.submenu("Select", window, cx, move |mut sm, _, _| {
                    for how in noshape_sketch::RelatedSelection::ALL {
                        let v = vs.clone();
                        sm = sm.item(crate::os_menu::OsMenuItem::new(how.label()).on_click(move |_, _, cx| v.update(cx, |this, cx| {
                            this.close_context_menu(cx);
                            if let Some(d) = this.sketch_data(cx)
                                && let Some(ed) = &mut this.sketch
                            {
                                ed.selection = d.sketch.select_related(&ed.selection, how);
                            }
                            this.refresh_scene(false, cx);
                            cx.notify();
                        })));
                    }
                    let v = vs.clone();
                    sm.item(crate::os_menu::OsMenuItem::new("Create selection…").on_click(move |_, _, cx| v.update(cx, |this, cx| {
                        this.close_context_menu(cx);
                        this.open_create_selection_at(this.ui.menu_pos, cx);
                    })))
                });
            } else {
                // Nothing selected: only "Create selection…" (live
                // menus/rmb-sketch-empty-select-submenu.png), after a
                // separator (live rmb-sketch-linetool-empty).
                m = m.separator();
                let vs = v.clone();
                m = m.submenu("Select", window, cx, move |sm, _, _| {
                    let v = vs.clone();
                    sm.item(crate::os_menu::OsMenuItem::new("Create selection…").on_click(move |_, _, cx| v.update(cx, |this, cx| {
                        this.close_context_menu(cx);
                        this.open_create_selection_at(this.ui.menu_pos, cx);
                    })))
                });
            }
            m = m.item(item("Select other…", Box::new(|this, window, cx| this.open_select_other(window, cx))));
            m = m.separator();
            m = m.item(item("Zoom to fit", Box::new(|_, window, cx| window.dispatch_action(Box::new(ZoomToFit), cx))));
            if n > 0 {
                m = m.item(item("Zoom to selection", Box::new(|this, _, cx| {
                    let Some(ed) = &this.sketch else { return };
                    let refs = ed.selection.iter().map(|e| Reference::SketchEntity { sketch: ed.feature, entity: e.0 }).collect();
                    let previous = std::mem::replace(&mut this.selection, refs);
                    this.zoom_to_selection(cx);
                    this.selection = previous;
                })));
            }
            m = m.item(item("View normal to sketch plane", Box::new(|this, _, cx| {
                let prev = std::mem::take(&mut this.selection);
                this.view_normal(cx);
                this.selection = prev;
            })));
            if tool_active
                && let Some(d) = single_line
            {
                m = m.item(item("Align view with vertical axis", Box::new(move |this, _, cx| {
                    let cam = this.viewport.read(cx).camera;
                    let fwd = cam.forward();
                    let Some(mut up) = (d - fwd * d.dot(fwd)).try_normalize() else { return };
                    if up.dot(cam.up()) < 0.0 {
                        up = -up;
                    }
                    this.viewport.update(cx, |vp, cx| vp.animate_to_direction(fwd, up, cx));
                })));
            }
            if n > 0 && !tool_active {
                let label = if n == 1 { "Delete sketch entity".to_string() } else { format!("Delete {n} sketch entities") };
                m = m.separator().item(item(&label, Box::new(|this, w, cx| this.sketch_delete(w, cx))));
            }
            m
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((position, menu, sub));
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Sketch keyboard actions on the view root.
    pub(super) fn with_sketch_actions(&mut self, root: Div, cx: &mut Context<Self>) -> AnyElement {
        if self.sketch.is_none() {
            return root.into_any_element();
        }
        root.on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| this.sketch_key_down(ev, window, cx)))
            .on_modifiers_changed(cx.listener(|this, ev: &ModifiersChangedEvent, _, cx| {
                let shift = ev.modifiers.shift;
                let alt = ev.modifiers.alt;
                let cursor = this.sketch.as_ref().and_then(|s| s.cursor);
                let _ = this.with_sketch(cx, |ed, data| ed.modifiers_changed(data, shift, alt));
                if !shift && let Some(p) = cursor
                    && let Some(snap) = this.model_snap(p, this.viewport.read(cx))
                    && let Some(ed) = &mut this.sketch
                {
                    ed.apply_external_snap(snap);
                }
                this.refresh_scene(false, cx);
                cx.notify();
            }))

            // Sketch versions of the document-level commands (capture phase,
            // before the Part Studio handles them).
            .capture_action(cx.listener(|this, _: &ClearSelection, _, cx| {
                if let Some(ed) = &mut this.sketch {
                    ed.selection.clear();
                    ed.selected_constraints.clear();
                    ed.selected_regions.clear();
                }
                this.selection.clear();
                this.refresh_scene(false, cx);
                cx.notify();
                cx.stop_propagation();
            }))
            // Enter accepts a pattern / transform / offset being set up before
            // it would accept the whole sketch.
            .capture_action(cx.listener(|this, _: &Accept, window, cx| {
                let Some(ed) = this.sketch.as_ref() else { return };
                if ed.text_dialog_open() {
                    this.accept_sketch_text(window, cx);
                    cx.stop_propagation();
                } else if ed.slot_pending() {
                    if let Some(out) = this.with_sketch(cx, |ed, data| ed.commit_slot(data)) {
                        this.apply_outcome(out, window, cx);
                    }
                    cx.stop_propagation();
                } else if ed.corner_pending() {
                    if let Some(out) = this.with_sketch(cx, |ed, data| ed.commit_corner(data)) {
                        this.apply_outcome(out, window, cx);
                    }
                    // Enter opens the radius value box (live sketch/115).
                    this.sync_dim_edit(window, cx);
                    cx.stop_propagation();
                } else if ed.polygon_sides_pending() {
                    if let Some(out) = this.with_sketch(cx, |ed, data| ed.finish_polygon(data)) {
                        this.apply_outcome(out, window, cx);
                    }
                    cx.stop_propagation();
                } else if ed.tool.is_pattern() && !ed.selection.is_empty() {
                    this.apply_pattern(window, cx);
                    cx.stop_propagation();
                } else if ed.offset_pending.is_some() {
                    if let Some(out) = this.with_sketch(cx, |ed, data| ed.commit_offset(data)) {
                        this.apply_outcome(out, window, cx);
                    }
                    this.sync_offset_input(window, cx);
                    cx.stop_propagation();
                } else if ed.otf.is_some() {
                    // Live Onshape: Enter while drawing places the boxed
                    // value as a driving dimension; the tool keeps going.
                    match this.with_sketch(cx, |ed, data| ed.accept_otf(data)) {
                        Some(Err(e)) => {
                            if let Some(ed) = &mut this.sketch { ed.message = Some(e); }
                        }
                        Some(Ok(true)) => this.regen_sync(window, cx),
                        _ => {}
                    }
                    this.close_otf_input();
                    this.refresh_scene(false, cx);
                    cx.notify();
                    cx.stop_propagation();
                } else {
                    // Live Onshape: Enter with nothing pending does nothing,
                    // with or without a tool (the sketch stays open, sketch/311).
                    cx.stop_propagation();
                }
            }))
            // Esc in a sketch cancels the tool / chain / selection, never the
            // sketch itself, even when the focus is in the Sketch dialog.
            .capture_action(cx.listener(|this, _: &Cancel, window, cx| {
                let transient = this.show_shortcuts || this.context_menu.is_some() || this.ui.mini_toolbar.is_some() || this.ui.select_other.is_some() || this.ui.dependencies.is_some() || this.viewport.read(cx).zoom_window;
                if !transient && this.sketch.is_some() {
                    if let Some(ui) = this.ui_mut() {
                        ui.degree_edit = None;
                        ui.tol_open = None;
                    }
                    if let Some(Some(out)) = this.with_sketch(cx, |ed, data| ed.escape_commit(data)) {
                        this.close_otf_input();
                        this.apply_outcome(out, window, cx);
                        window.focus(&this.focus, cx);
                        cx.stop_propagation();
                        return;
                    }
                    this.sketch_cancel(cx);
                    this.sync_offset_input(window, cx);
                    window.focus(&this.focus, cx);
                    cx.stop_propagation();
                }
            }))
            // Select other (`): pick among the sketch entities under the cursor.
            .capture_action(cx.listener(|this, _: &SelectOther, window, cx| {
                this.open_select_other(window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &Copy, _, cx| {
                let text = this.sketch_data(cx).and_then(|d| this.sketch.as_ref()?.copy_selection(&d));
                if let Some(text) = text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &Paste, window, cx| {
                this.paste_sketch_entities(window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &SketchLine, _, cx| this.set_tool(Tool::Line, cx)))
            .on_action(cx.listener(|this, _: &SketchArc3, _, cx| this.set_tool(Tool::Arc3, cx)))
            .on_action(cx.listener(|this, _: &SketchCircle, _, cx| this.set_tool(Tool::Circle, cx)))
            .on_action(cx.listener(|this, _: &SketchCenterRectangle, _, cx| this.set_tool(Tool::CenterRect, cx)))
            .on_action(cx.listener(|this, _: &SketchCornerRectangle, _, cx| this.set_tool(Tool::CornerRect, cx)))
            .on_action(cx.listener(|this, _: &SketchPoint, _, cx| this.set_tool(Tool::Point, cx)))
            .on_action(cx.listener(|this, _: &SketchFillet, _, cx| this.set_tool(Tool::Fillet, cx)))
            .on_action(cx.listener(|this, _: &SketchOffset, window, cx| this.activate_tool(Tool::Offset, window, cx)))
            .on_action(cx.listener(|this, _: &SketchTrim, _, cx| this.set_tool(Tool::Trim, cx)))
            .on_action(cx.listener(|this, _: &SketchExtend, _, cx| this.set_tool(Tool::Extend, cx)))
            .on_action(cx.listener(|this, _: &SketchDimension, _, cx| this.set_tool(Tool::Dimension, cx)))
            .on_action(cx.listener(|this, _: &SketchUse, _, cx| this.set_tool(Tool::Use, cx)))
            .on_action(cx.listener(|this, _: &SketchConstruction, window, cx| this.toggle_construction(window, cx)))
            .on_action(cx.listener(|this, _: &SketchTangentArcToggle, _, cx| {
                if let Some(ed) = &mut this.sketch {
                    ed.toggle_tangent_arc();
                }
                this.refresh_scene(false, cx);
                cx.notify();
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

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn toolbar_groups_cover_every_tool_once() {
        let tools: Vec<Tool> = all_sketch_tools().collect();
        let mut seen = std::collections::HashSet::new();
        for t in &tools {
            assert!(seen.insert(format!("{t:?}")), "{t:?} twice");
        }
        assert!(tools.contains(&Tool::InsertDxf) && tools.contains(&Tool::Chamfer) && tools.contains(&Tool::SplinePoint));
    }
}
