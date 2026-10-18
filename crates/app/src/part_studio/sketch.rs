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
    /// Text box the "tedit.text" field currently edits.
    text_edit_for: Option<u32>,
    text_edit_sub: Option<Subscription>,
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
    /// Decoded sketch images by (image id, data size).
    images: HashMap<(u32, usize), Arc<RenderImage>>,
    /// Keystroke interceptor applying the user's sketch shortcuts.
    shortcut_interceptor: Option<Subscription>,
    /// Dimension whose tolerance panel is open.
    tol_open: Option<noshape_sketch::ConstraintId>,
    /// Hover reference the Use preview was computed for.
    use_preview_for: Option<Reference>,
    /// Open dimension context menu: constraint and viewport position.
    dim_menu: Option<(noshape_sketch::ConstraintId, f32, f32)>,
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

/// User sketch shortcuts: `{"<tool name>": "<keystroke>"}` in
/// `<config>/noshape/sketch-shortcuts.json`, tool names as in the toolbar
/// tooltips without the shortcut ("Midpoint line": "alt-l").
fn custom_shortcuts() -> &'static HashMap<String, Tool> {
    static MAP: std::sync::OnceLock<HashMap<String, Tool>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let path = crate::files::config_dir().join("sketch-shortcuts.json");
        let Ok(text) = std::fs::read_to_string(path) else { return HashMap::new() };
        parse_custom_shortcuts(&text)
    })
}

/// Parses a sketch shortcut file into keystroke -> tool.
fn parse_custom_shortcuts(text: &str) -> HashMap<String, Tool> {
    let Ok(map) = serde_json::from_str::<HashMap<String, String>>(text) else { return HashMap::new() };
    let tools: Vec<Tool> = all_sketch_tools().chain([Tool::Select]).collect();
    map.into_iter()
        .filter_map(|(name, key)| {
            let tool = tools.iter().copied().find(|t| t.label().split(" (").next().is_some_and(|n| n.eq_ignore_ascii_case(name.trim())))?;
            Some((key.trim().to_lowercase(), tool))
        })
        .collect()
}

/// The tool bound to a keystroke by the user, if any.
fn custom_shortcut(k: &Keystroke) -> Option<Tool> {
    let map = custom_shortcuts();
    if map.is_empty() {
        return None;
    }
    let m = &k.modifiers;
    let mut s = String::new();
    for (on, name) in [(m.control, "ctrl-"), (m.alt, "alt-"), (m.shift, "shift-"), (m.platform, "cmd-")] {
        if on {
            s.push_str(name);
        }
    }
    s.push_str(&k.key.to_lowercase());
    map.get(&s).copied()
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
    Angle,
    Count,
    Number,
    Text,
}

/// Option fields shown in the floating options box for a tool.
fn tool_options(tool: Tool) -> &'static [(&'static str, &'static str, OptKind)] {
    match tool {
        Tool::Fillet => &[("fillet.radius", "Radius", OptKind::Length)],
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
        let frame = self.display.sketches.iter().find(|s| s.feature == id).map(|s| s.frame).or(match plane {
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
        // Every sketch edit regenerates through here: record it for Ctrl+Z.
        self.draft_checkpoint(cx);
        let Some((studio, upto)) = self.effective_studio(cx) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
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
                    self.sketch_data(cx)?.sketch.pick(p, r)
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
            for er in self.hovered_edge_refs() {
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
        if let Some(ed) = &mut self.sketch {
            ed.set_tool(tool);
        }
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
        let units = self.doc.read(cx).session.doc.settings.units;
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
        let len = |v: f64| noshape_units::format_quantity(Quantity::new(v, Dim::LENGTH), &units);
        let ang = |v: f64| noshape_units::format_quantity(Quantity::new(v, Dim::ANGLE), &units);
        match key {
            "fillet.radius" => len(ed.fillet_radius),
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
        let units = self.doc.read(cx).session.doc.settings.units;
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

    /// The Sketch dialog's display options and diagnostics tools (Onshape
    /// shows them in the dialog under the plane field), drawn under it.
    fn render_sketch_options(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ed = self.sketch.as_ref()?;
        let pal = p();
        let (sc, se, serr) = (ed.show_constraints, ed.show_expressions, ed.show_errors);
        let (cm_open, pi_open) = self.ui_ref().map(|u| (u.cm.is_some(), u.pi.is_some())).unwrap_or_default();
        let view = cx.entity();
        let check = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>, f: fn(&mut SketchEditor)| {
            Button::new(id).label(label).icon(icon_px(if on { "check" } else { "minus" }, 12.0, if on { pal.text } else { pal.text_faint })).ghost().xsmall().w_full().justify_start().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(ed) = &mut this.sketch {
                    f(ed);
                }
                this.refresh_scene(false, cx);
                cx.notify();
            }))
        };
        Some(
            v_flex()
                .absolute()
                .left(px(12.))
                .top(px(146.))
                .occlude()
                .w(px(218.))
                .p_1()
                .gap_0p5()
                .rounded_md()
                .bg(with_alpha(pal.panel_raised, 0.96))
                .border_1()
                .border_color(pal.border)
                .child(check("sk-opt-constraints", "Show constraints", sc, cx, |ed| ed.set_show_constraints(!ed.show_constraints)))
                .child(check("sk-opt-expressions", "Show expressions", se, cx, |ed| ed.show_expressions = !ed.show_expressions))
                .child(check("sk-opt-errors", "Show errors", serr, cx, |ed| ed.show_errors = !ed.show_errors))
                .child(
                    Button::new("sk-diagnostics")
                        .label("Sketch diagnostics tools")
                        .icon(icon_px("under-defined", 12.0, pal.text))
                        .ghost()
                        .xsmall()
                        .w_full()
                        .justify_start()
                        .selected(cm_open || pi_open)
                        .tooltip("Show sketch diagnostics tools")
                        .dropdown_menu(move |m, _, _| {
                            let (v1, v2) = (view.clone(), view.clone());
                            m.item(PopupMenuItem::new(if pi_open { "Turn profile inspector off" } else { "Profile inspector…" }).on_click(move |_, _, cx| v1.update(cx, |this, cx| this.toggle_profile_inspector(cx))))
                                .item(PopupMenuItem::new(if cm_open { "Turn constraint manager off" } else { "Constraint manager…" }).on_click(move |_, _, cx| v2.update(cx, |this, cx| this.toggle_constraint_manager(cx))))
                        }),
                )
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ sketch diagnostics

    fn toggle_constraint_manager(&mut self, cx: &mut Context<Self>) {
        if let Some(ui) = self.ui_mut() {
            ui.cm = if ui.cm.is_some() { None } else { Some(ConstraintManager { auto_select: true, ..Default::default() }) };
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn toggle_profile_inspector(&mut self, cx: &mut Context<Self>) {
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
    fn install_shortcut_interceptor(&mut self, cx: &mut Context<Self>) {
        if custom_shortcuts().is_empty() || self.ui_ref().is_some_and(|u| u.shortcut_interceptor.is_some()) {
            return;
        }
        let view = cx.entity().downgrade();
        let sub = cx.intercept_keystrokes(move |ev, window, cx| {
            let Some(view) = view.upgrade() else { return };
            if window.has_focused_input(cx) || !view.read(cx).focus.contains_focused(window, cx) || view.read(cx).sketch.is_none() {
                return;
            }
            if let Some(tool) = custom_shortcut(&ev.keystroke) {
                view.update(cx, |this, cx| this.activate_tool(tool, window, cx));
                cx.stop_propagation();
            }
        });
        if let Some(ui) = self.ui_mut() {
            ui.shortcut_interceptor = Some(sub);
        }
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
            Button::new("sk-dxf-file").label("Choose a DXF file…").icon(icon_px("import", 13.0, pal.text)).primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.choose_dxf(window, cx))).into_any_element(),
            div().text_xs().text_color(pal.text_faint).child("DWG files: save them as DXF first.").into_any_element(),
        ]
    }

    fn choose_dxf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let win = window.window_handle();
        crate::files::request_open_path("Insert DXF or DWG", "DXF drawing", &["dxf", "DXF"], window, cx, move |path, cx| {
            let _ = win.update(cx, |_, window, cx| view.update(cx, |this, cx| this.insert_dxf_file(&path, window, cx)));
        });
    }

    pub(super) fn insert_dxf_file(&mut self, path: &std::path::Path, window: &mut Window, cx: &mut Context<Self>) {
        let parsed = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| noshape_sketch::dxf::parse_dxf(&String::from_utf8_lossy(&b)).map_err(|e| e.to_string()));
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
        let win = window.window_handle();
        crate::files::request_open_path("Insert image", "Image", &["png", "jpg", "jpeg", "gif", "bmp", "PNG", "JPG", "JPEG"], window, cx, move |path, cx| {
            let _ = win.update(cx, |_, _, cx| view.update(cx, |this, cx| this.load_image_file(&path, cx)));
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

    /// Decoded image of a sketch image (cached).
    fn sketch_image(&mut self, img: &noshape_model::sketch_feature::SketchImage) -> Option<Arc<RenderImage>> {
        let key = (img.id, img.data_base64.len());
        if let Some(r) = self.ui_ref().and_then(|u| u.images.get(&key).cloned()) {
            return Some(r);
        }
        let bytes = noshape_model::sketch_feature::base64_decode(&img.data_base64)?;
        let mut rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
        // GPUI render images are BGRA.
        for px in rgba.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let r = Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]));
        self.ui_mut()?.images.insert(key, r.clone());
        Some(r)
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
        self.install_shortcut_interceptor(cx);

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
            // Convert to expression / Convert to text (Onshape: text field context menu).
            let (tid, is_expr) = (t.id, t.expression);
            rows.push(
                Button::new("sk-text-expr")
                    .label(if is_expr { "Convert to text" } else { "Convert to expression" })
                    .ghost()
                    .xsmall()
                    .tooltip("Expressions: \"text\" ~ #variable ~ roundToPrecision(#len / in, 3)")
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if this.with_sketch(cx, |ed, data| ed.edit_text(data, tid, |t| t.expression = !t.expression)) == Some(true) {
                            this.regen_sync(window, cx);
                        }
                    }))
                    .into_any_element(),
            );
        } else if tool == Tool::Text {
            let (f, b, i) = self.sketch.as_ref().map(|e| (e.text_font.clone(), e.text_bold, e.text_italic)).unwrap_or_default();
            rows.push(self.text_style_row(&f, b, i, None, cx));
        } else if tool == Tool::InsertDxf {
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

        // Inserted images, drawn in their (projected) boxes behind the labels.
        let image_boxes: Vec<(noshape_model::sketch_feature::SketchImage, Vec<DVec2>)> = data
            .images
            .iter()
            .map(|i| (i.clone(), i.box_points.iter().filter_map(|p| data.sketch.point(noshape_sketch::EntityId(*p))).collect::<Vec<DVec2>>()))
            .filter(|(_, pts)| pts.len() == 4)
            .collect();
        for (img, pts) in image_boxes {
            let Some(render) = self.sketch_image(&img) else { continue };
            let Some(ed) = self.sketch.as_ref() else { break };
            let frame = ed.frame;
            let vp = self.viewport.read(cx);
            let scr: Vec<(f32, f32)> = pts.iter().filter_map(|q| vp.project(frame.to_world(*q)).map(|s| (s.x as f32, s.y as f32))).collect();
            if scr.len() != 4 {
                continue;
            }
            // GPUI draws axis-aligned images: show it when the box is upright on
            // screen (sketch seen normal, box not rotated), else only its outline.
            let upright = (scr[0].1 - scr[1].1).abs() < 2.0 && (scr[0].0 - scr[3].0).abs() < 2.0 && scr[1].0 > scr[0].0 && scr[3].1 < scr[0].1;
            if !upright {
                continue;
            }
            let (x0, y0, x1, y1) = (scr[0].0, scr[3].1, scr[1].0, scr[0].1);
            out.push(div().absolute().left(px(x0)).top(px(y0)).w(px(x1 - x0)).h(px(y1 - y0)).opacity(0.85).child(gpui_kit::img(render).size_full()).into_any_element());
        }

        // Tolerance / precision panel of the dimension being edited.
        let tol_panel = self.dimension_tolerance_panel(&data, window, cx);
        let mut tol_panel = tol_panel;

        let ed = self.sketch.as_ref().expect("sketch");
        let frame = ed.frame;
        let vp = self.viewport.read(cx);
        let project = |q: DVec2| vp.project(frame.to_world(q)).map(|s| (s.x as f32, s.y as f32));
        let editing = self.dim_input.as_ref().map(|(c, st, _)| (*c, st.clone()));

        // ---- dimension labels
        let configured: std::collections::HashSet<u32> = data.sketch.constraints().filter(|c| c.is_dimension() && self.dimension_configuration(c.id, cx).is_some()).map(|c| c.id.0).collect();
        let regenerated = self.context.as_ref().and_then(|c| c.sketches.iter().find(|s| s.feature == ed.feature)).map(|s| s.sketch.clone());
        for (cid, pos, mut text, driving) in ed.dimension_labels(&data, &units) {
            let Some((x, y)) = project(pos) else { continue };
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
                    // The 110 x 20 px field is centered on the label position.
                    h_flex()
                        .absolute()
                        .left(px(x - 55.0))
                        .top(px(y - 10.0))
                        .gap_0p5()
                        .occlude()
                        // The value is centered on the dimension, like its label.
                        .child(div().w(px(110.)).child(Input::new(st).xsmall().text_center()))
                        .when(allowed, |d| {
                            d.child(
                                Button::new("sk-dim-tol")
                                    .label("±")
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Tolerance and precision")
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Some(ui) = this.ui_mut() {
                                            ui.tol_open = if ui.tol_open == Some(cid) { None } else { Some(cid) };
                                            ui.opts.remove("tol.upper");
                                            ui.opts.remove("tol.lower");
                                        }
                                        cx.notify();
                                    })),
                            )
                        })
                        .into_any_element(),
                );
                if let Some(panel) = tol_panel.take() {
                    out.push(div().absolute().left(px(x - 55.0)).top(px(y + 14.0)).child(panel).into_any_element());
                }
                continue;
            }
            let selected = ed.selected_constraints.contains(&cid);
            let hovered = ed.hover_constraint == Some(cid);
            let conflict = ed.show_errors && ed.conflicting.contains(&cid);
            let est_w = text.chars().count() as f32 * 6.6 + 10.0;
            // Home > Colors: dimension hover #d9bfa0, selected #d9a566.
            let (bg, fg, border) = if selected {
                (rgb(0xd9a566), pal.text, rgb(0xbf6900))
            } else if hovered {
                (rgb(0xd9bfa0), pal.text, rgb(0xbf9460))
            } else if conflict {
                (with_alpha(rgb(0xaa252d), 0.85), pal.text, rgb(0xff6b6b))
            } else {
                (with_alpha(pal.panel_raised, 0.75), if driving { pal.text } else { pal.text_faint }, with_alpha(pal.border, 0.6))
            };
            // Configured dimensions have a gold border (Onshape).
            let border = if is_configured { rgb(0xffc20e) } else { border };
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
                    .child(format!("Degree {deg}"))
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
            // Onshape constraint tokens: black on light gray, white on red for
            // problems, blue background for external (used) references.
            let (bg, fg) = match (conflict, g.external, selected, hovered) {
                (true, _, true, _) => (rgb(0xbd3039), rgb(0xffffff)),
                (true, _, false, true) => (rgb(0xaa252d), rgb(0xffffff)),
                (true, _, false, false) => (rgb(0xd66e74), rgb(0xffffff)),
                (false, true, true, _) => (rgb(0x90cef1), rgb(0x000000)),
                (false, true, false, true) => (rgb(0xb2ddf6), rgb(0x000000)),
                (false, true, false, false) => (rgb(0xdef1fb), rgb(0x000000)),
                (false, false, true, _) => (rgb(0xfab601), rgb(0x000000)),
                (false, false, false, true) => (rgb(0xfcda7f), rgb(0x000000)),
                _ => (rgb(0xf2f2f2), rgb(0x000000)),
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
                    .border_1()
                    .border_color(rgb(0x9aa5af))
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
        for (pos, value, active) in ed.otf_boxes(&data, &units) {
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
            let est_w = value.chars().count() as f32 * 6.6 + 14.0;
            out.push(
                div()
                    .absolute()
                    .left(px(x - est_w * 0.5))
                    .top(px(y - 10.0))
                    .px_1p5()
                    .py_0p5()
                    .rounded(px(3.))
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(if active { pal.accent } else { pal.border_strong })
                    .text_xs()
                    .text_color(if active { pal.text } else { pal.text_muted })
                    .whitespace_nowrap()
                    .child(value)
                    .into_any_element(),
            );
        }

        // ---- value of the dimension being placed (follows the cursor)
        if let Some((kind, pos, v)) = ed.dimension_preview(&data)
            && let Some((x, y)) = project(pos)
        {
            let text = crate::sketch_mode::dim_value_text(&kind, v / crate::sketch_mode::dim_display_scale(&kind), &units);
            out.push(div().absolute().left(px(x + 8.0)).top(px(y - 22.0)).px_1().rounded(px(3.)).bg(with_alpha(pal.panel_raised, 0.85)).text_xs().text_color(pal.text).whitespace_nowrap().child(text).into_any_element());
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
        if !rows.is_empty() {
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
            if tool.is_pattern() {
                let n = ed.selection.len();
                let hint = if n == 0 { "Select the entities".to_string() } else { format!("{n} entit{} · click in empty space or press Enter to accept, Esc to cancel", if n == 1 { "y" } else { "ies" }) };
                panel = panel.child(div().text_xs().text_color(pal.text_faint).child(hint));
            }
            out.push(panel.into_any_element());
        }

        if let Some(panel) = self.render_sketch_options(cx) {
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
                    .child(Button::new("sk-pattern-edit").label("Edit pattern").ghost().xsmall().w_full().justify_start().on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if let Some(ui) = this.ui_mut() {
                            ui.pattern_menu = None;
                        }
                        let data = this.sketch_data(cx);
                        if let (Some(ed), Some(data)) = (&mut this.sketch, data) {
                            ed.edit_pattern(&data, pid);
                        }
                        this.sync_pattern_inputs(window, cx);
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

    pub(super) fn render_sketch_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(ed) = &self.sketch else { return div().into_any_element() };
        let active = ed.tool;
        let construction = ed.construction;
        // Constraint buttons not applicable to the selection are disabled.
        let selection = ed.selection.clone();
        let data = self.sketch_data(cx);
        // Onshape's sketch toolbar has the feature toolbar's metrics (§3.10).
        let tool_btn = |tool: Tool, cx: &mut Context<Self>| {
            crate::ui::with_tooltip(
                SharedString::from(format!("skt-{tool:?}")),
                tool.label(),
                crate::ui::tool_button(SharedString::from(format!("sk-{tool:?}")), tool.icon(), active == tool, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.activate_tool(tool, window, cx))),
            )
        };
        let sep = || crate::ui::tool_separator().into_any_element();
        let group = |bar: gpui_kit::Div, group: &'static [Tool], cx: &mut Context<Self>| -> gpui_kit::Div {
            // The active member, else the last used one, is the group's face.
            let face = group.iter().copied().find(|t| *t == active).unwrap_or_else(|| group_face(group));
            let mut bar = bar.child(tool_btn(face, cx));
            if group.len() > 1 {
                let view = cx.entity();
                let members: Vec<Tool> = group.to_vec();
                bar = bar.child(
                    crate::ui::tool_caret(SharedString::from(format!("skm-{:?}", group[0])), cx)
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
        let action_btn = |id: &'static str, icon: &'static str, tip: &'static str, action: Box<dyn Action>, cx: &mut Context<Self>| {
            crate::ui::with_tooltip(SharedString::from(format!("{id}-tt")), tip, crate::ui::tool_button(id, icon, false, cx).on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx)))
        };
        let mut bar = h_flex().flex_1().min_w_0().overflow_hidden().gap(px(2.));
        bar = bar
            .child(action_btn("sk-undo", "undo", "Undo (Ctrl+Z)", Box::new(Undo), cx))
            .child(action_btn("sk-redo", "redo", "Redo (Ctrl+Y)", Box::new(Redo), cx))
            .child(sep())
            // Extrude / Revolve accept the sketch and pre-select all its regions.
            .child(action_btn("sk-extrude", "extrude", "Extrude (Shift+E)", Box::new(Extrude), cx))
            .child(action_btn("sk-revolve", "revolve", "Revolve (Shift+W)", Box::new(Revolve), cx))
            .child(sep());
        for g in DRAW_TOOLS {
            bar = group(bar, g, cx);
        }
        bar = bar.child(sep());
        for g in USE_TOOLS {
            bar = group(bar, g, cx);
        }
        bar = bar.child(crate::ui::with_tooltip(
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
            bar = bar.child(crate::ui::with_tooltip(
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
            .child(crate::ui::search_tools_box("sk-search").ml(px(10.)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_palette(window, cx))))
            .into_any_element()
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
        let menu = gpui_kit::component::menu::PopupMenu::build(window, cx, move |mut m, _window, _cx| {
            for (e, name) in &names {
                let (v, e) = (view.clone(), *e);
                m = m.item(PopupMenuItem::new(name.clone()).on_click(move |_, _, cx| {
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
        let has_clip = cx.read_from_clipboard().and_then(|c| c.text()).is_some_and(|t| t.contains("noshape_sketch_entities"));
        let view = cx.entity();
        let menu = gpui_kit::component::menu::PopupMenu::build(window, cx, move |mut m, _window, _cx| {
            let v = view.clone();
            let item = |label: &str, f: SketchMenuAction| {
                let v = v.clone();
                PopupMenuItem::new(label.to_string()).on_click(move |_, window, cx| v.update(cx, |this, cx| {
                    this.close_context_menu(cx);
                    f(this, window, cx)
                }))
            };
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
            if model_edge && tool != Tool::Use {
                m = m.item(item("Use", Box::new(|this, w, cx| this.use_hovered(w, cx))));
            }
            if n > 0 {
                let label = if n == 1 { "Delete sketch entity".to_string() } else { format!("Delete {n} sketch entities") };
                m = m.item(item(&label, Box::new(|this, w, cx| this.sketch_delete(w, cx))));
                m = m.item(item("Construction", Box::new(|this, w, cx| this.toggle_construction(w, cx))));
                m = m.item(item("Transform sketch entities", Box::new(|this, _, cx| this.set_tool(Tool::Transform, cx))));
                m = m.item(item("Copy sketch entities", Box::new(|this, _, cx| {
                    let text = this.sketch_data(cx).and_then(|d| this.sketch.as_ref()?.copy_selection(&d));
                    if let Some(text) = text {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                })));
            }
            if has_clip {
                m = m.item(item("Paste sketch entities", Box::new(|this, w, cx| this.paste_sketch_entities(w, cx))));
            }
            m = m.separator();
            m = m.item(item("View normal to sketch plane", Box::new(|this, _, cx| {
                let prev = std::mem::take(&mut this.selection);
                this.view_normal(cx);
                this.selection = prev;
            })));
            if tool != Tool::Select {
                m = m.item(item(&format!("Exit {}", tool.label().split(" (").next().unwrap_or("tool").to_lowercase()), Box::new(|this, _, cx| this.set_tool(Tool::Select, cx))));
            }
            if n > 0 {
                m = m.item(item("Clear selection", Box::new(|this, _, cx| {
                    if let Some(ed) = &mut this.sketch {
                        ed.selection.clear();
                        ed.selected_constraints.clear();
                    }
                    this.refresh_scene(false, cx);
                    cx.notify();
                })));
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
                if ed.polygon_sides_pending() {
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
                }
            }))
            // Esc in a sketch cancels the tool / chain / selection, never the
            // sketch itself, even when the focus is in the Sketch dialog.
            .capture_action(cx.listener(|this, _: &Cancel, window, cx| {
                let transient = this.show_shortcuts || this.ui.mini_toolbar.is_some() || this.ui.select_other.is_some() || this.ui.dependencies.is_some() || this.viewport.read(cx).zoom_window;
                if !transient && this.sketch.is_some() {
                    if let Some(ui) = this.ui_mut() {
                        ui.degree_edit = None;
                        ui.tol_open = None;
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
    fn custom_sketch_shortcuts_parse_tool_names() {
        let m = parse_custom_shortcuts(r#"{"Midpoint line": "alt-l", "slot": "Shift-T", "nonsense": "x", "Coincident": "ctrl-i"}"#);
        assert_eq!(m.get("alt-l"), Some(&Tool::MidpointLine));
        assert_eq!(m.get("shift-t"), Some(&Tool::Slot));
        assert_eq!(m.get("ctrl-i"), Some(&Tool::Constraint(ConstraintTool::Coincident)));
        assert_eq!(m.len(), 3);
        assert!(parse_custom_shortcuts("not json").is_empty());
    }

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
