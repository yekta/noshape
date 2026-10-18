//! Sketch editing: drawing tools with inference snapping and automatic
//! constraints, live drag solving, smart dimensions and constraint tools.
//!
//! The editor operates on the [`SketchData`] of the sketch feature being
//! edited (owned by the feature dialog's draft). Every edit re-solves the
//! sketch immediately; the Part Studio view then regenerates the display.
//!
//! Besides editing, the editor computes the sketch-mode graphics that are not
//! part of the regenerated display: tool previews, inference guides,
//! dimension graphics (extension lines, dimension lines, arrowheads, radial
//! leaders and angle arcs, all in sketch-plane coordinates) and constraint
//! glyph anchors.

use glam::{DVec2, DVec3};
use noshape_model::display::PlaneFrame;
use noshape_model::reference::FeatureId;
use noshape_model::sketch_feature::SketchData;
use noshape_render::{LineData, LineSegment, PointData, PointSprite};
use noshape_sketch::{AutoConstraint, ConstraintId, ConstraintKind, Curve2, DistanceMode, DragHandle, EntityId, EntityKind, Inference, InferenceContext, InferenceKind, Sketch, SketchTool, Transform};
use noshape_units::{Dim, ExprValue, Quantity, UnitPrefs};
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintTool {
    Coincident,
    Concentric,
    Parallel,
    Tangent,
    Horizontal,
    Vertical,
    Midpoint,
    Perpendicular,
    Equal,
    Symmetric,
    Fix,
    Normal,
    Pierce,
    Curvature,
}

impl ConstraintTool {
    pub const ALL: [ConstraintTool; 14] = [
        ConstraintTool::Coincident,
        ConstraintTool::Concentric,
        ConstraintTool::Parallel,
        ConstraintTool::Tangent,
        ConstraintTool::Horizontal,
        ConstraintTool::Vertical,
        ConstraintTool::Midpoint,
        ConstraintTool::Normal,
        ConstraintTool::Pierce,
        ConstraintTool::Perpendicular,
        ConstraintTool::Equal,
        ConstraintTool::Symmetric,
        ConstraintTool::Fix,
        ConstraintTool::Curvature,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ConstraintTool::Coincident => "Coincident (I)",
            ConstraintTool::Concentric => "Concentric (Shift+O)",
            ConstraintTool::Parallel => "Parallel (B)",
            ConstraintTool::Tangent => "Tangent (T)",
            ConstraintTool::Horizontal => "Horizontal (H)",
            ConstraintTool::Vertical => "Vertical (V)",
            ConstraintTool::Midpoint => "Midpoint (Shift+M)",
            ConstraintTool::Perpendicular => "Perpendicular (Shift+L)",
            ConstraintTool::Equal => "Equal (E)",
            ConstraintTool::Symmetric => "Symmetric (Shift+Q)",
            ConstraintTool::Fix => "Fix (Shift+J)",
            ConstraintTool::Normal => "Normal (Shift+K)",
            ConstraintTool::Pierce => "Pierce (Shift+G)",
            ConstraintTool::Curvature => "Curvature (Shift+U)",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            ConstraintTool::Coincident => "coincident",
            ConstraintTool::Concentric => "concentric",
            ConstraintTool::Parallel => "parallel",
            ConstraintTool::Tangent => "tangent",
            ConstraintTool::Horizontal => "horizontal",
            ConstraintTool::Vertical => "vertical",
            ConstraintTool::Midpoint => "midpoint",
            ConstraintTool::Perpendicular => "perpendicular",
            ConstraintTool::Equal => "equal",
            ConstraintTool::Symmetric => "symmetric",
            ConstraintTool::Fix => "fix",
            ConstraintTool::Normal => "normal",
            ConstraintTool::Pierce => "pierce",
            ConstraintTool::Curvature => "curvature",
        }
    }
    /// Entities needed before the constraint can be created.
    fn arity(self) -> usize {
        match self {
            ConstraintTool::Horizontal | ConstraintTool::Vertical | ConstraintTool::Fix => 1,
            ConstraintTool::Symmetric => 3,
            _ => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Line,
    CornerRect,
    CenterRect,
    AlignedRect,
    Circle,
    ThreePointCircle,
    Arc3,
    TangentArc,
    CenterArc,
    Point,
    Spline,
    Bezier,
    Conic,
    Slot,
    Polygon,
    CircumPolygon,
    Ellipse,
    Text,
    Fillet,
    Trim,
    Extend,
    Split,
    Offset,
    Mirror,
    LinearPattern,
    CircularPattern,
    Transform,
    Use,
    Intersection,
    Dimension,
    Constraint(ConstraintTool),
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Line => "Line (L)",
            Tool::CornerRect => "Corner rectangle (G)",
            Tool::CenterRect => "Center rectangle (R)",
            Tool::AlignedRect => "Aligned rectangle",
            Tool::Circle => "Center point circle (C)",
            Tool::ThreePointCircle => "3 point circle",
            Tool::Arc3 => "3 point arc (A)",
            Tool::TangentArc => "Tangent arc",
            Tool::CenterArc => "Center point arc",
            Tool::Point => "Point (Shift+S)",
            Tool::Spline => "Spline",
            Tool::Bezier => "Bezier",
            Tool::Conic => "Conic",
            Tool::Slot => "Slot",
            Tool::Polygon => "Inscribed polygon",
            Tool::CircumPolygon => "Circumscribed polygon",
            Tool::Ellipse => "Ellipse",
            Tool::Text => "Text",
            Tool::Fillet => "Sketch fillet (Shift+F)",
            Tool::Trim => "Trim (M)",
            Tool::Extend => "Extend (X)",
            Tool::Split => "Split",
            Tool::Offset => "Offset (O)",
            Tool::Mirror => "Mirror",
            Tool::LinearPattern => "Linear pattern",
            Tool::CircularPattern => "Circular pattern",
            Tool::Transform => "Transform",
            Tool::Use => "Use / project edges (U)",
            Tool::Intersection => "Intersection",
            Tool::Dimension => "Dimension (D)",
            Tool::Constraint(c) => c.label(),
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => "select-box",
            Tool::Line => "line",
            Tool::CornerRect => "corner-rectangle",
            Tool::CenterRect => "center-rectangle",
            Tool::AlignedRect => "aligned-rectangle",
            Tool::Circle => "circle",
            Tool::ThreePointCircle => "three-point-circle",
            Tool::Arc3 => "three-point-arc",
            Tool::TangentArc => "tangent-arc",
            Tool::CenterArc => "center-point-arc",
            Tool::Point => "point-sketch",
            Tool::Spline => "spline",
            Tool::Bezier => "bezier",
            Tool::Conic => "conic",
            Tool::Slot => "slot",
            Tool::Polygon => "polygon-inscribed",
            Tool::CircumPolygon => "polygon-circumscribed",
            Tool::Ellipse => "ellipse",
            Tool::Text => "text",
            Tool::Fillet => "sketch-fillet",
            Tool::Trim => "trim",
            Tool::Extend => "extend",
            Tool::Split => "split-sketch",
            Tool::Offset => "offset",
            Tool::Mirror => "sketch-mirror",
            Tool::LinearPattern => "sketch-linear-pattern",
            Tool::CircularPattern => "sketch-circular-pattern",
            Tool::Transform => "transform-sketch",
            Tool::Use => "use",
            Tool::Intersection => "intersection-curve",
            Tool::Dimension => "dimension",
            Tool::Constraint(c) => c.icon(),
        }
    }
    fn inference_tool(self) -> SketchTool {
        match self {
            Tool::Select => SketchTool::Select,
            Tool::Point | Tool::Text => SketchTool::Point,
            Tool::Line => SketchTool::Line,
            Tool::CornerRect | Tool::CenterRect | Tool::AlignedRect => SketchTool::Rectangle,
            Tool::Circle | Tool::ThreePointCircle | Tool::Polygon | Tool::CircumPolygon => SketchTool::Circle,
            Tool::Arc3 | Tool::CenterArc => SketchTool::Arc,
            Tool::TangentArc => SketchTool::TangentArc,
            Tool::Spline | Tool::Bezier | Tool::Conic => SketchTool::Spline,
            _ => SketchTool::Other,
        }
    }
    /// Tools that place points by clicking in empty space.
    pub fn draws(self) -> bool {
        matches!(
            self,
            Tool::Line
                | Tool::CornerRect
                | Tool::CenterRect
                | Tool::AlignedRect
                | Tool::Circle
                | Tool::ThreePointCircle
                | Tool::Arc3
                | Tool::TangentArc
                | Tool::CenterArc
                | Tool::Point
                | Tool::Spline
                | Tool::Bezier
                | Tool::Conic
                | Tool::Slot
                | Tool::Polygon
                | Tool::CircumPolygon
                | Tool::Ellipse
                | Tool::Text
        )
    }
    /// Tools working on a selection with parameters and an explicit Apply.
    pub fn is_pattern(self) -> bool {
        matches!(self, Tool::LinearPattern | Tool::CircularPattern | Tool::Transform)
    }
}

#[derive(Clone, Debug)]
struct Click {
    pos: DVec2,
    inference: Option<Inference>,
}

#[derive(Clone, Debug)]
enum DragState {
    /// Pressed on an entity; becomes a geometry drag when moved.
    Pending { entity: EntityId, start: DVec2, additive: bool },
    Geometry { handle: DragHandle },
    /// Pressed on a dimension label: moves it when dragged, selects it on release.
    Label { constraint: ConstraintId, start: DVec2, moved: bool, additive: bool },
    /// Pressed on a constraint glyph: moves it when dragged, selects it on release.
    Glyph { constraint: ConstraintId, entity: EntityId, start: DVec2, orig: DVec2, moved: bool, additive: bool },
    /// Dragging an on-canvas pattern / transform manipulator.
    Handle(PatternHandle),
    /// Pressed on a text (glyph or box line): moves the whole box when dragged.
    Text { entity: EntityId, start: DVec2, points: Vec<(EntityId, DVec2)>, moved: bool },
    Box { start: DVec2 },
}

/// On-canvas manipulators of the pattern and transform tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternHandle {
    /// Linear pattern: first instance (direction and spacing).
    LinearStep,
    /// Linear pattern: last instance (instance count).
    LinearCount,
    /// Circular pattern / transform: center (pivot).
    Center,
    /// Circular pattern: end of the total angle.
    CircularAngle,
    /// Circular pattern: first instance (instance count).
    CircularCount,
    /// Transform: translation.
    Move,
    /// Transform: rotation.
    Rotate,
    /// Transform: scale.
    Scale,
}

/// Pending inline dimension edit.
#[derive(Clone, Debug)]
pub struct DimEdit {
    pub constraint: ConstraintId,
    pub text: String,
}

/// Dimension graphics in sketch-plane coordinates.
#[derive(Clone, Debug)]
pub struct DimGraphic {
    pub constraint: ConstraintId,
    pub label: DVec2,
    pub text: String,
    pub driving: bool,
    pub segments: Vec<[DVec2; 2]>,
}

/// A constraint glyph anchored next to one of the constrained entities.
#[derive(Clone, Debug)]
pub struct GlyphInfo {
    pub constraint: ConstraintId,
    pub icon: &'static str,
    pub entity: EntityId,
    /// Anchor in sketch-plane coordinates (entity midpoint / point).
    pub anchor: DVec2,
    /// The anchored entity is a point (glyph goes above-right) or a curve.
    pub on_point: bool,
    /// User placement offset (sketch-plane units) when the glyph was dragged.
    pub offset: Option<DVec2>,
}

/// Key of a moved constraint glyph in [`SketchData::glyph_offsets`].
pub fn glyph_key(c: ConstraintId, e: EntityId) -> String {
    format!("{}:{}", c.0, e.0)
}

/// Snap to model geometry (from the 3D hover) supplied by the view.
#[derive(Clone, Debug, PartialEq)]
pub enum ExternalSnap {
    /// A model vertex projected onto the sketch plane.
    Vertex { position: DVec2, reference: noshape_model::reference::Reference },
    /// A point on a model edge projected onto the sketch plane.
    Edge { position: DVec2 },
}

pub struct SketchEditor {
    pub feature: FeatureId,
    pub frame: PlaneFrame,
    pub tool: Tool,
    clicks: Vec<Click>,
    /// End point of the previous line (chaining).
    chain_end: Option<EntityId>,
    pub cursor: Option<DVec2>,
    pub inference: Option<Inference>,
    pub external_snap: Option<ExternalSnap>,
    pub selection: Vec<EntityId>,
    pub selected_constraints: Vec<ConstraintId>,
    /// Hovered sketch entity (from the viewport pick).
    pub hover: Option<EntityId>,
    /// Hovered dimension label or constraint glyph.
    pub hover_constraint: Option<ConstraintId>,
    /// Entity whose glyphs stay visible briefly after hover ends.
    pub glyph_entity: Option<(EntityId, std::time::Instant)>,
    drag: Option<DragState>,
    box_end: Option<DVec2>,
    /// Point snapped to while dragging a point (coincident on release).
    drag_snap: Option<(EntityId, DVec2, bool)>,
    /// Entities picked so far for a constraint/dimension tool.
    picks: Vec<(EntityId, DVec2)>,
    pub dim_edit: Option<DimEdit>,
    pub construction: bool,
    /// Show every constraint glyph (toolbar toggle).
    pub show_constraints: bool,
    /// Shift held while moving: show all glyphs temporarily (Onshape).
    pub shift_held: bool,
    pub polygon_sides: usize,
    pub fillet_radius: f64,
    pub offset_distance: f64,
    pub offset_flip: bool,
    pub conic_rho: f64,
    pub text_string: String,
    pub text_height: f64,
    pub text_font: String,
    pub text_bold: bool,
    pub text_italic: bool,
    pub pattern_count: usize,
    pub pattern_distance: f64,
    pub pattern_angle: f64,
    pub circular_count: usize,
    pub circular_angle: f64,
    pub pattern_center: DVec2,
    pub xf_offset: DVec2,
    pub xf_angle: f64,
    pub xf_scale: f64,
    /// First fillet arc of the current fillet session (later fillets are equal to it).
    fillet_session: Option<(EntityId, f64)>,
    /// Mirror line chosen in tool-first mode.
    mirror_axis: Option<EntityId>,
    /// Inline value entry while drawing (typed values, SI).
    pub entry_active: bool,
    pub entry_values: Vec<Option<f64>>,
    /// Entities created by the last tool action.
    last_created: Vec<EntityId>,
    /// Previous line of the current line chain (typed angles are relative to it)
    /// and the unit vector from the chain end back along it.
    last_line: Option<(EntityId, DVec2)>,
    /// Label/glyph pressed in the GPUI overlay (consumed by the next tool_down).
    pub pressed_label: Option<(ConstraintId, bool, bool)>,
    pub pressed_glyph: Option<(ConstraintId, EntityId, bool)>,
    /// Constraints the solver reported as conflicting (set by the view).
    pub conflicting: Vec<ConstraintId>,
    /// World units per pixel at the sketch (for snap radii).
    pub world_per_px: f64,
    pub message: Option<String>,
    /// View-side UI state (input entities), owned by the Part Studio view.
    pub ui: Option<Box<dyn std::any::Any>>,
}

pub struct EditOutcome {
    /// Sketch data changed (commit draft + regenerate).
    pub changed: bool,
    /// Only the overlay changed (redraw).
    pub redraw: bool,
}

impl EditOutcome {
    fn none() -> Self {
        Self { changed: false, redraw: false }
    }
    fn redraw() -> Self {
        Self { changed: false, redraw: true }
    }
    fn changed() -> Self {
        Self { changed: true, redraw: true }
    }
}

fn is_point(sk: &Sketch, id: EntityId) -> bool {
    sk.kind(id) == Some(EntityKind::Point)
}

fn is_angle_kind(k: &ConstraintKind) -> bool {
    matches!(k, ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. })
}

/// Glyph icon for a geometric constraint (None for dimensions).
pub fn constraint_icon(k: &ConstraintKind) -> Option<&'static str> {
    use ConstraintKind::*;
    Some(match k {
        Coincident { .. } => "coincident",
        Concentric { .. } => "concentric",
        Parallel { .. } => "parallel",
        Perpendicular { .. } => "perpendicular",
        Horizontal { .. } => "horizontal",
        Vertical { .. } => "vertical",
        Tangent { .. } => "tangent",
        Equal { .. } => "equal",
        Midpoint { .. } => "midpoint",
        Symmetric { .. } => "symmetric",
        Fix { .. } => "fix",
        Normal { .. } => "normal",
        Collinear { .. } => "collinear",
        Curvature { .. } => "curvature",
        Pierce { .. } => "pierce",
        _ => return None,
    })
}

/// Icon shown next to the cursor for an inference.
pub fn inference_icon(k: InferenceKind) -> &'static str {
    match k {
        InferenceKind::Endpoint | InferenceKind::Quadrant | InferenceKind::OnCurve | InferenceKind::AlignBoth => "coincident",
        InferenceKind::Midpoint => "midpoint",
        InferenceKind::Center => "concentric",
        InferenceKind::AlignHorizontal | InferenceKind::Horizontal => "horizontal",
        InferenceKind::AlignVertical | InferenceKind::Vertical => "vertical",
        InferenceKind::Perpendicular => "perpendicular",
        InferenceKind::Parallel => "parallel",
        InferenceKind::Tangent => "tangent",
    }
}

impl SketchEditor {
    pub fn new(feature: FeatureId, frame: PlaneFrame) -> Self {
        Self {
            feature,
            frame,
            tool: Tool::Select,
            clicks: Vec::new(),
            chain_end: None,
            cursor: None,
            inference: None,
            external_snap: None,
            selection: Vec::new(),
            selected_constraints: Vec::new(),
            hover: None,
            hover_constraint: None,
            glyph_entity: None,
            drag: None,
            box_end: None,
            drag_snap: None,
            picks: Vec::new(),
            dim_edit: None,
            construction: false,
            show_constraints: false,
            shift_held: false,
            polygon_sides: 6,
            fillet_radius: 0.005,
            offset_distance: 0.005,
            offset_flip: false,
            conic_rho: 0.5,
            text_string: "Text".into(),
            text_height: 0.01,
            text_font: noshape_sketch::text::DEFAULT_FONT.into(),
            text_bold: false,
            text_italic: false,
            pattern_count: 3,
            pattern_distance: 0.02,
            pattern_angle: 0.0,
            circular_count: 4,
            circular_angle: TAU,
            pattern_center: DVec2::ZERO,
            xf_offset: DVec2::ZERO,
            xf_angle: 0.0,
            xf_scale: 1.0,
            fillet_session: None,
            mirror_axis: None,
            entry_active: false,
            entry_values: Vec::new(),
            last_created: Vec::new(),
            last_line: None,
            pressed_label: None,
            pressed_glyph: None,
            conflicting: Vec::new(),
            world_per_px: 1e-4,
            message: None,
            ui: None,
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        // Pressing the active tool's key again toggles back to select.
        self.tool = if self.tool == tool && tool != Tool::Select { Tool::Select } else { tool };
        self.clicks.clear();
        self.chain_end = None;
        self.last_line = None;
        self.picks.clear();
        self.inference = None;
        self.external_snap = None;
        self.fillet_session = None;
        self.mirror_axis = None;
        self.entry_active = false;
        self.entry_values.clear();
        self.message = match self.tool {
            Tool::Fillet => Some("Click corners to fillet; each fillet is equal to the first".into()),
            Tool::Trim => Some("Click the part of a curve to remove".into()),
            Tool::Extend => Some("Click near the end of a curve to extend it".into()),
            Tool::Split => Some("Click a curve where it should be split".into()),
            Tool::Offset => Some("Click curves to offset them (selected curves are offset first)".into()),
            Tool::Mirror => Some(if self.selection.is_empty() { "Click a mirror line, then click entities to mirror" } else { "Click a mirror line" }.into()),
            Tool::Dimension => Some("Select one or two entities, then click to place the dimension".into()),
            Tool::Use => Some("Click model edges or faces to project them into the sketch".into()),
            Tool::Intersection => Some("Click a model face crossing the sketch plane".into()),
            Tool::Text => Some("Drag (or click two corners) to place the text box; its height sets the text size".into()),
            Tool::Conic => Some("Click start, end, then the control point".into()),
            Tool::Bezier => Some("Click control points; double-click to finish".into()),
            Tool::LinearPattern => Some("Select entities, set the pattern, click empty space to aim, then Apply".into()),
            Tool::CircularPattern => Some("Select entities, click empty space to set the center, then Apply".into()),
            Tool::Transform => Some("Select entities, set the transform (click empty space for the center), then Apply".into()),
            Tool::Constraint(c) => Some(format!("Select {} entit{} for {}", c.arity(), if c.arity() == 1 { "y" } else { "ies" }, c.label())),
            _ => None,
        };
    }

    /// Cancel the in-progress operation (Esc). Returns true if something was cancelled.
    pub fn cancel(&mut self) -> bool {
        if self.dim_edit.take().is_some() {
            return true;
        }
        if self.entry_active {
            self.entry_active = false;
            self.entry_values.clear();
            return true;
        }
        if !self.clicks.is_empty() || !self.picks.is_empty() || self.chain_end.is_some() || self.mirror_axis.is_some() {
            self.clicks.clear();
            self.picks.clear();
            self.chain_end = None;
            self.last_line = None;
            self.mirror_axis = None;
            return true;
        }
        if self.tool != Tool::Select {
            self.set_tool(Tool::Select);
            return true;
        }
        if !self.selection.is_empty() || !self.selected_constraints.is_empty() {
            self.selection.clear();
            self.selected_constraints.clear();
            return true;
        }
        false
    }

    /// Cursor position on the sketch plane from a world ray.
    pub fn plane_point(&self, origin: DVec3, dir: DVec3) -> Option<DVec2> {
        self.frame.intersect_ray(origin, dir).map(|p| self.frame.to_local(p))
    }

    fn infer(&self, sk: &Sketch, p: DVec2, suppress: bool) -> Option<Inference> {
        if suppress {
            return None;
        }
        let ctx = InferenceContext {
            tool: self.tool.inference_tool(),
            anchor: self.clicks.last().map(|c| c.pos),
            anchor_point: self.chain_end,
            snap_radius: self.world_per_px * 9.0,
            angle_tolerance: 3f64.to_radians(),
            exclude: Vec::new(),
        };
        noshape_sketch::infer(sk, p, &ctx).into_iter().next()
    }

    pub fn pointer_move(&mut self, data: &mut SketchData, p: DVec2, shift: bool) -> EditOutcome {
        self.cursor = Some(p);
        self.shift_held = shift;
        self.external_snap = None;
        if self.tool.draws() {
            self.inference = self.infer(&data.sketch, p, shift);
            return EditOutcome::redraw();
        }
        self.inference = None;
        EditOutcome::redraw()
    }

    /// Apply a snap to model geometry when it beats the sketch inference.
    pub fn apply_external_snap(&mut self, snap: ExternalSnap) {
        if !self.tool.draws() || self.entry_active {
            return;
        }
        let strong = self.inference.as_ref().is_some_and(|i| matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Midpoint | InferenceKind::Center));
        if strong {
            return;
        }
        let (position, kind) = match &snap {
            ExternalSnap::Vertex { position, .. } => (*position, InferenceKind::Endpoint),
            ExternalSnap::Edge { position } => (*position, InferenceKind::OnCurve),
        };
        if self.inference.as_ref().is_some_and(|i| kind == InferenceKind::OnCurve && i.kind != InferenceKind::OnCurve) {
            return;
        }
        let distance = self.cursor.map(|c| (c - position).length()).unwrap_or(0.0);
        self.inference = Some(Inference { kind, position, entity: None, reference: None, constraints: Vec::new(), distance, priority: 0 });
        self.external_snap = Some(snap);
    }

    fn snapped(&self) -> Option<(DVec2, Option<Inference>)> {
        if let Some(p) = self.entry_cursor() {
            return Some((p, None));
        }
        let c = self.cursor?;
        Some(match &self.inference {
            Some(i) => (i.position, Some(i.clone())),
            None => (c, None),
        })
    }

    /// Create (or reuse) a point entity for a placed click and return it plus
    /// constraints to apply once the new curve exists.
    fn point_for(sk: &mut Sketch, click: &Click) -> (EntityId, Vec<AutoConstraint>) {
        if let Some(inf) = &click.inference
            && inf.kind == InferenceKind::Endpoint
            && let Some(AutoConstraint::CoincidentPoint(p)) = inf.constraints.first()
        {
            let rest = inf.constraints.iter().skip(1).copied().collect();
            return (*p, rest);
        }
        let id = sk.add_point(click.pos);
        (id, click.inference.as_ref().map(|i| i.constraints.clone()).unwrap_or_default())
    }

    fn apply_auto(sk: &mut Sketch, point: EntityId, curve: Option<EntityId>, autos: &[AutoConstraint]) {
        for a in autos {
            if let Some(kind) = a.to_constraint(point, curve) {
                let _ = sk.add_constraint(kind);
            }
        }
    }

    fn finish_create(&mut self, data: &mut SketchData, created: &[EntityId]) {
        if self.construction {
            for id in created {
                let _ = data.sketch.set_construction(*id, true);
            }
        }
        self.last_created = created.to_vec();
        data.sketch.solve();
    }

    /// Link an external snap (model vertex) to a fixed sketch point before a
    /// click is placed, so the new geometry becomes coincident with it.
    pub fn take_vertex_snap(&mut self, data: &mut SketchData) {
        let Some(ExternalSnap::Vertex { position, reference }) = self.external_snap.clone() else { return };
        let id = data.sketch.use_external_point(position);
        data.external_refs.insert(id.0, reference);
        if let Some(inf) = &mut self.inference {
            inf.kind = InferenceKind::Endpoint;
            inf.constraints = vec![AutoConstraint::CoincidentPoint(id)];
        }
        self.external_snap = None;
    }

    pub fn tool_down(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>, shift: bool, double_click: bool) -> EditOutcome {
        self.cursor = Some(p);
        // Overlay labels and glyphs take the press first, whatever the tool.
        if let Some((cid, additive, select)) = self.pressed_label.take() {
            // `moved: true` up front suppresses the selection toggle (double-click edit).
            self.drag = Some(DragState::Label { constraint: cid, start: p, moved: !select, additive });
            return EditOutcome::none();
        }
        if let Some((cid, entity, additive)) = self.pressed_glyph.take() {
            let orig = data.glyph_offsets.get(&glyph_key(cid, entity)).map(|o| DVec2::new(o[0], o[1])).unwrap_or(DVec2::ZERO);
            self.drag = Some(DragState::Glyph { constraint: cid, entity, start: p, orig, moved: false, additive });
            return EditOutcome::none();
        }
        if self.tool.is_pattern()
            && let Some(h) = self.handle_at(&data.sketch, p)
        {
            self.drag = Some(DragState::Handle(h));
            return EditOutcome::redraw();
        }
        if self.entry_active && self.tool.draws() {
            // Clicking while typing a value commits the typed value.
            return self.commit_entry_values(data, &[]);
        }
        match self.tool {
            Tool::Select => {
                match hovered {
                    // Text: glyphs and box lines move the whole box (corners resize it).
                    Some(e) if !is_point(&data.sketch, e) && data.text_of_entity(e).is_some() => {
                        let tid = data.text_of_entity(e).unwrap_or_default();
                        let points: Vec<(EntityId, DVec2)> = data
                            .texts
                            .iter()
                            .find(|t| t.id == tid)
                            .map(|t| t.box_points.iter().filter_map(|q| data.sketch.point(EntityId(*q)).map(|pos| (EntityId(*q), pos))).collect())
                            .unwrap_or_default();
                        self.drag = Some(DragState::Text { entity: e, start: p, points, moved: false });
                    }
                    Some(e) => {
                        self.drag = Some(DragState::Pending { entity: e, start: p, additive: shift });
                    }
                    None => {
                        self.drag = Some(DragState::Box { start: p });
                        self.box_end = Some(p);
                    }
                }
                EditOutcome::none()
            }
            t if t.draws() => {
                if self.inference.is_none() {
                    self.inference = self.infer(&data.sketch, p, shift);
                }
                self.take_vertex_snap(data);
                let (pos, inference) = self.snapped().unwrap_or((p, None));
                let click = Click { pos, inference };
                self.place(data, click, double_click)
            }
            Tool::Fillet => {
                let Some(pid) = hovered.filter(|e| is_point(&data.sketch, *e)).or_else(|| nearest_point(&data.sketch, p, self.world_per_px * 10.0)) else {
                    self.message = Some("Click a corner point".into());
                    return EditOutcome::none();
                };
                match data.sketch.fillet_corner(pid, self.fillet_radius) {
                    Ok(r) => {
                        let arc = r.created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc));
                        // The fillet comes with its own radius dimension.
                        let rad_dim = r.constraints_added.iter().copied().find(|c| data.sketch.constraint(*c).is_some_and(|c| matches!(c.kind, ConstraintKind::Radius { .. })));
                        self.finish_create(data, &r.created);
                        if let Some(arc) = arc {
                            match self.fillet_session {
                                // Later fillets of the session are equal to the first one,
                                // so editing the first dimension changes them all (Onshape).
                                Some((first, rad)) if (rad - self.fillet_radius).abs() < 1e-12 && data.sketch.entity(first).is_some() => {
                                    if let Some(d) = rad_dim {
                                        let _ = data.sketch.remove_constraint(d);
                                    }
                                    let _ = data.sketch.add_constraint(ConstraintKind::Equal { a: first, b: arc });
                                }
                                _ => {
                                    if let (Some(d), Some(m)) = (rad_dim, curve_mid(&data.sketch, arc)) {
                                        let c = match data.sketch.curve(arc) {
                                            Some(Curve2::Arc { center, .. }) => center,
                                            _ => m,
                                        };
                                        let l = m + (m - c).try_normalize().unwrap_or(DVec2::X) * self.world_per_px * 30.0;
                                        data.dimension_labels.insert(d.0, [l.x, l.y]);
                                    }
                                    self.fillet_session = Some((arc, self.fillet_radius));
                                }
                            }
                            data.sketch.solve();
                        }
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("Fillet failed: {e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Trim | Tool::Extend | Tool::Split => {
                let Some(id) = hovered.filter(|e| !is_point(&data.sketch, *e)).or_else(|| data.sketch.pick(p, self.world_per_px * 8.0).filter(|e| !is_point(&data.sketch, *e))) else {
                    return EditOutcome::none();
                };
                let r = match self.tool {
                    Tool::Trim => data.sketch.trim(id, p),
                    Tool::Extend => data.sketch.extend(id, p),
                    _ => {
                        let at = data.sketch.curve(id).map(|c| c.closest(p).1).unwrap_or(p);
                        data.sketch.split(id, at)
                    }
                };
                match r {
                    Ok(_) => {
                        data.sketch.solve();
                        prune_exprs(data);
                        self.selection.retain(|e| data.sketch.entity(*e).is_some());
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Offset => {
                let ids: Vec<EntityId> = if self.selection.is_empty() { hovered.into_iter().filter(|e| !is_point(&data.sketch, *e)).collect() } else { self.selection.clone() };
                if ids.is_empty() {
                    return EditOutcome::none();
                }
                let d = if self.offset_flip { -self.offset_distance } else { self.offset_distance };
                match data.sketch.offset(&ids, d) {
                    Ok(r) => {
                        // Repeat mode: later clicks offset single curves by the same distance.
                        self.selection.clear();
                        self.finish_create(data, &r.created);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("Offset failed: {e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Mirror => {
                let is_line = |e: &EntityId| data.sketch.kind(*e) == Some(EntityKind::Line);
                match self.mirror_axis.filter(|a| data.sketch.entity(*a).is_some()) {
                    None => {
                        let Some(axis) = hovered.filter(is_line) else {
                            self.message = Some("Click a line to mirror about".into());
                            return EditOutcome::none();
                        };
                        self.mirror_axis = Some(axis);
                        let ids: Vec<EntityId> = self.selection.iter().copied().filter(|e| *e != axis).collect();
                        if ids.is_empty() {
                            self.message = Some("Click entities to mirror (Esc to finish)".into());
                            return EditOutcome::redraw();
                        }
                        self.selection.clear();
                        self.mirror_entities(data, &ids)
                    }
                    Some(axis) => {
                        let Some(e) = hovered.filter(|e| *e != axis) else { return EditOutcome::none() };
                        self.mirror_entities(data, &[e])
                    }
                }
            }
            Tool::LinearPattern | Tool::CircularPattern | Tool::Transform => {
                match hovered {
                    Some(e) if data.text_of_entity(e).is_some() && !is_point(&data.sketch, e) => self.toggle_text_selection(data, e),
                    Some(e) => {
                        if let Some(pos) = self.selection.iter().position(|x| *x == e) {
                            self.selection.remove(pos);
                        } else {
                            self.selection.push(e);
                        }
                    }
                    None => match self.tool {
                        Tool::LinearPattern => {
                            if let Some(c) = self.selection_center(&data.sketch) {
                                let d = p - c;
                                if d.length() > 1e-9 {
                                    self.pattern_angle = d.to_angle();
                                    self.pattern_distance = d.length();
                                }
                            }
                        }
                        _ => self.pattern_center = self.inference_free_snap(&data.sketch, p),
                    },
                }
                EditOutcome::redraw()
            }
            Tool::Dimension => self.dimension_click(data, p, hovered),
            Tool::Constraint(c) => {
                let Some(e) = hovered else { return EditOutcome::none() };
                if !self.picks.iter().any(|(x, _)| *x == e) {
                    self.picks.push((e, p));
                }
                if self.picks.len() >= c.arity() {
                    let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
                    self.picks.clear();
                    match make_constraint(&data.sketch, c, &ids) {
                        Some(kind) => match data.sketch.add_constraint(kind) {
                            Ok(_) => {
                                data.sketch.solve();
                                return EditOutcome::changed();
                            }
                            Err(err) => self.message = Some(format!("Can't add constraint: {err}")),
                        },
                        None => self.message = Some("Those entities can't take this constraint".into()),
                    }
                }
                EditOutcome::redraw()
            }
            _ => EditOutcome::none(),
        }
    }

    fn mirror_entities(&mut self, data: &mut SketchData, ids: &[EntityId]) -> EditOutcome {
        let Some(axis) = self.mirror_axis else { return EditOutcome::none() };
        match data.sketch.mirror(ids, axis) {
            Ok(r) => {
                self.finish_create(data, &r.created);
                self.message = Some("Click more entities to mirror (Esc to finish)".into());
                EditOutcome::changed()
            }
            Err(e) => {
                self.message = Some(format!("Mirror failed: {e}"));
                EditOutcome::redraw()
            }
        }
    }

    /// Snap a free click to an existing sketch point (pattern centers).
    fn inference_free_snap(&self, sk: &Sketch, p: DVec2) -> DVec2 {
        if p.length() < self.world_per_px * 9.0 {
            return DVec2::ZERO;
        }
        nearest_point(sk, p, self.world_per_px * 9.0).and_then(|id| sk.point(id)).unwrap_or(p)
    }

    fn selection_center(&self, sk: &Sketch) -> Option<DVec2> {
        if self.selection.is_empty() {
            return None;
        }
        let bb = sk.entities_bbox(&self.selection);
        (!bb.is_empty()).then(|| bb.center())
    }

    /// Place a click for the active drawing tool.
    fn place(&mut self, data: &mut SketchData, click: Click, double_click: bool) -> EditOutcome {
        let sk = &mut data.sketch;
        match self.tool {
            Tool::Point => {
                let (pid, autos) = Self::point_for(sk, &click);
                Self::apply_auto(sk, pid, None, &autos);
                self.finish_create(data, &[pid]);
                EditOutcome::changed()
            }
            Tool::Line => {
                if double_click {
                    self.clicks.clear();
                    self.chain_end = None;
                    self.last_line = None;
                    return EditOutcome::redraw();
                }
                let Some(prev) = self.clicks.last().cloned() else {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                };
                if (prev.pos - click.pos).length() < 1e-9 {
                    return EditOutcome::none();
                }
                let (a, autos_a) = match self.chain_end {
                    Some(e) => (e, Vec::new()),
                    None => Self::point_for(sk, &prev),
                };
                let (b, autos_b) = Self::point_for(sk, &click);
                let Ok(line) = sk.add_line_between(a, b) else { return EditOutcome::none() };
                Self::apply_auto(sk, a, Some(line), &autos_a);
                Self::apply_auto(sk, b, Some(line), &autos_b);
                // Closing onto an existing point ends the chain (Onshape).
                let closed = click.inference.as_ref().is_some_and(|i| i.kind == InferenceKind::Endpoint);
                self.finish_create(data, &[line]);
                if closed {
                    self.clicks.clear();
                    self.chain_end = None;
                    self.last_line = None;
                } else {
                    let back = match (data.sketch.point(a), data.sketch.point(b)) {
                        (Some(pa), Some(pb)) => (pa - pb).try_normalize().unwrap_or(-DVec2::X),
                        _ => -DVec2::X,
                    };
                    self.clicks = vec![click];
                    self.chain_end = Some(b);
                    self.last_line = Some((line, back));
                }
                EditOutcome::changed()
            }
            Tool::CornerRect | Tool::CenterRect => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let first = self.clicks.remove(0);
                if (first.pos - click.pos).abs().min_element() < 1e-9 {
                    return EditOutcome::none();
                }
                let ids = if self.tool == Tool::CornerRect { sk.corner_rectangle(first.pos, click.pos) } else { sk.center_rectangle(first.pos, click.pos) };
                if self.tool == Tool::CornerRect {
                    if let (Some(&p0), Some(inf)) = (ids.points.first(), &first.inference) {
                        Self::apply_auto(sk, p0, None, &inf.constraints);
                    }
                    if let (Some(&p2), Some(inf)) = (ids.points.get(2), &click.inference) {
                        Self::apply_auto(sk, p2, None, &inf.constraints);
                    }
                }
                self.finish_create(data, &ids.curves);
                EditOutcome::changed()
            }
            Tool::AlignedRect => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                match sk.three_point_rectangle(c[0].pos, c[1].pos, c[2].pos) {
                    Ok(ids) => {
                        self.finish_create(data, &ids.curves);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Circle => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let c = self.clicks.remove(0);
                let r = (click.pos - c.pos).length();
                if r < 1e-9 {
                    return EditOutcome::none();
                }
                let (center, autos) = Self::point_for(sk, &c);
                let Ok(circle) = sk.add_circle_at(center, r) else { return EditOutcome::none() };
                Self::apply_auto(sk, center, None, &autos);
                // A point snapped on the rim makes the circle pass through it.
                if let Some(inf) = &click.inference
                    && let Some(AutoConstraint::CoincidentPoint(pt)) = inf.constraints.first()
                {
                    let _ = sk.add_constraint(ConstraintKind::Coincident { a: *pt, b: circle });
                }
                self.finish_create(data, &[circle]);
                EditOutcome::changed()
            }
            Tool::ThreePointCircle => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let pts: Vec<DVec2> = self.clicks.drain(..).map(|c| c.pos).collect();
                match sk.three_point_circle(pts[0], pts[1], pts[2]) {
                    Ok(id) => {
                        self.finish_create(data, &[id]);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::Arc3 => {
                // Click start, end, then a point on the arc.
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                match sk.three_point_arc(c[0].pos, c[2].pos, c[1].pos) {
                    Ok(arc) => {
                        if let Some((s, e)) = sk.endpoints(arc) {
                            for pt in [s, e] {
                                let p = sk.point(pt).unwrap_or_default();
                                let near = |q: DVec2| (q - p).length();
                                let cl = if near(c[0].pos) < near(c[1].pos) { &c[0] } else { &c[1] };
                                if let Some(inf) = &cl.inference {
                                    Self::apply_auto(sk, pt, Some(arc), &inf.constraints);
                                }
                            }
                        }
                        self.finish_create(data, &[arc]);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::CenterArc => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                let id = sk.center_point_arc(c[0].pos, c[1].pos, c[2].pos, true);
                self.finish_create(data, &[id]);
                EditOutcome::changed()
            }
            Tool::TangentArc => {
                let from = self.chain_end.or_else(|| {
                    click.inference.as_ref().and_then(|i| match i.constraints.first() {
                        Some(AutoConstraint::CoincidentPoint(p)) => Some(*p),
                        _ => None,
                    })
                });
                let Some(from) = from else {
                    if let Some(p) = nearest_point(sk, click.pos, self.world_per_px * 10.0) {
                        self.chain_end = Some(p);
                    } else {
                        self.message = Some("Start a tangent arc at the end of a curve".into());
                    }
                    return EditOutcome::redraw();
                };
                if self.chain_end.is_none() {
                    self.chain_end = Some(from);
                    return EditOutcome::redraw();
                }
                match sk.tangent_arc(from, click.pos) {
                    Ok(ids) => {
                        self.chain_end = ids.points.last().copied();
                        self.finish_create(data, &ids.curves);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Spline | Tool::Bezier => {
                if double_click && self.clicks.len() >= 2 {
                    let pts: Vec<DVec2> = self.clicks.drain(..).map(|c| c.pos).collect();
                    let r = if self.tool == Tool::Spline {
                        sk.add_fit_spline(&pts)
                    } else if pts.len() == 2 {
                        Ok(sk.add_line(pts[0], pts[1]))
                    } else {
                        let deg = (pts.len() - 1).min(15);
                        sk.add_bspline(&pts, deg)
                    };
                    if let Ok(id) = r {
                        self.finish_create(data, &[id]);
                        return EditOutcome::changed();
                    }
                    return EditOutcome::redraw();
                }
                if self.clicks.last().is_some_and(|c| (c.pos - click.pos).length() < 1e-9) {
                    return EditOutcome::none();
                }
                self.clicks.push(click);
                EditOutcome::redraw()
            }
            Tool::Conic => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                match sk.add_conic(c[0].pos, c[2].pos, c[1].pos, self.conic_rho.clamp(0.01, 0.99)) {
                    Ok(id) => {
                        self.finish_create(data, &[id]);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Slot => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                let width = 2.0 * dist_point_line(c[2].pos, c[0].pos, c[1].pos);
                match sk.straight_slot(c[0].pos, c[1].pos, width.max(1e-6)) {
                    Ok(ids) => {
                        let mut all = ids.curves.clone();
                        all.extend(ids.construction.iter().copied());
                        self.finish_create(data, &ids.curves);
                        self.last_created = all;
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::Polygon | Tool::CircumPolygon => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let c = self.clicks.remove(0);
                match sk.polygon(c.pos, click.pos, self.polygon_sides.clamp(3, 50), self.tool == Tool::Polygon) {
                    Ok(ids) => {
                        let mut all = ids.curves.clone();
                        all.extend(ids.construction.iter().copied());
                        self.finish_create(data, &ids.curves);
                        self.last_created = all;
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::Ellipse => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                match sk.ellipse_3pt(c[0].pos, c[1].pos, c[2].pos) {
                    Ok(id) => {
                        self.finish_create(data, &[id]);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::Text => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let first = self.clicks.remove(0);
                let (origin, height) = self.text_box_from(first.pos, click.pos);
                let text = if self.text_string.trim().is_empty() { "Text".to_string() } else { self.text_string.clone() };
                match data.add_text(&text, &self.text_font, self.text_bold, self.text_italic, origin, height) {
                    Ok(id) => {
                        data.sketch.solve();
                        // Select the new text so its content and style can be edited.
                        self.set_tool(Tool::Select);
                        self.selection = data.texts.iter().find(|t| t.id == id).map(|t| t.box_lines.iter().map(|l| EntityId(*l)).collect()).unwrap_or_default();
                        self.message = Some("Edit the text in the options box; drag the text to move it, a corner to resize".into());
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(e);
                        EditOutcome::redraw()
                    }
                }
            }
            _ => EditOutcome::none(),
        }
    }

    pub fn tool_drag(&mut self, data: &mut SketchData, p: DVec2) -> EditOutcome {
        self.cursor = Some(p);
        match self.drag.clone() {
            Some(DragState::Pending { entity, start, .. }) => {
                if (p - start).length() < self.world_per_px * 3.0 {
                    return EditOutcome::none();
                }
                let handle = if is_point(&data.sketch, entity) { DragHandle::Point(entity) } else { DragHandle::Entity { id: entity, grab: start } };
                self.drag = Some(DragState::Geometry { handle });
                self.tool_drag(data, p)
            }
            Some(DragState::Geometry { handle }) => {
                let _report = data.sketch.drag(&[(handle, p)]);
                match handle {
                    DragHandle::Entity { id, .. } => self.drag = Some(DragState::Geometry { handle: DragHandle::Entity { id, grab: p } }),
                    DragHandle::Point(id) => self.drag_snap = self.point_drop_target(&data.sketch, id),
                    #[allow(unreachable_patterns)]
                    _ => {}
                }
                EditOutcome::changed()
            }
            Some(DragState::Label { constraint, start, additive, .. }) => {
                if (p - start).length() < self.world_per_px * 3.0 {
                    return EditOutcome::none();
                }
                self.drag = Some(DragState::Label { constraint, start, moved: true, additive });
                data.dimension_labels.insert(constraint.0, [p.x, p.y]);
                EditOutcome::changed()
            }
            Some(DragState::Box { .. }) => {
                self.box_end = Some(p);
                EditOutcome::redraw()
            }
            Some(DragState::Glyph { constraint, entity, start, orig, moved, additive }) => {
                if !moved && (p - start).length() < self.world_per_px * 3.0 {
                    return EditOutcome::none();
                }
                self.drag = Some(DragState::Glyph { constraint, entity, start, orig, moved: true, additive });
                let o = orig + (p - start);
                data.glyph_offsets.insert(glyph_key(constraint, entity), [o.x, o.y]);
                EditOutcome::redraw()
            }
            Some(DragState::Handle(h)) => {
                self.drag_handle(&data.sketch, h, p);
                EditOutcome::redraw()
            }
            Some(DragState::Text { entity, start, points, moved }) => {
                if !moved && (p - start).length() < self.world_per_px * 3.0 {
                    return EditOutcome::none();
                }
                let d = p - start;
                let handles: Vec<(DragHandle, DVec2)> = points.iter().map(|(id, q)| (DragHandle::Point(*id), *q + d)).collect();
                let _ = data.sketch.drag(&handles);
                self.drag = Some(DragState::Text { entity, start, points, moved: true });
                EditOutcome::changed()
            }
            // Drawing tools: keep the preview following the pointer while a
            // button is held (text box / click-drag placement).
            None if self.tool.draws() => {
                self.inference = self.infer(&data.sketch, p, false);
                EditOutcome::redraw()
            }
            None => EditOutcome::none(),
        }
    }

    /// Point or curve under a dragged point (coincident on release).
    fn point_drop_target(&self, sk: &Sketch, id: EntityId) -> Option<(EntityId, DVec2, bool)> {
        let pos = sk.point(id)?;
        let r = self.world_per_px * 9.0;
        let users: Vec<EntityId> = sk.users_of_point(id);
        // Points of curves using the dragged point would collapse those curves.
        let siblings: Vec<EntityId> = users.iter().filter_map(|u| sk.entity(*u)).flat_map(|e| e.point_refs()).collect();
        let best_pt = sk
            .entities()
            .filter(|e| e.kind() == EntityKind::Point && e.id != id && !siblings.contains(&e.id))
            .filter_map(|e| sk.point(e.id).map(|q| (e.id, q, (q - pos).length())))
            .filter(|x| x.2 <= r)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((e, q, _)) = best_pt {
            return Some((e, q, true));
        }
        sk.entities()
            .filter(|e| e.kind() != EntityKind::Point && !users.contains(&e.id) && !e.point_refs().contains(&id))
            .filter_map(|e| {
                let c = sk.curve(e.id)?;
                let (_, q, d) = c.closest(pos);
                (d <= r).then_some((e.id, q, d))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(e, q, _)| (e, q, false))
    }

    pub fn tool_up(&mut self, data: &mut SketchData, p: DVec2) -> EditOutcome {
        // Text box by click-drag: the release is the second corner.
        if self.tool == Tool::Text && self.drag.is_none() && self.clicks.len() == 1 && (p - self.clicks[0].pos).length() > self.world_per_px * 6.0 {
            return self.place(data, Click { pos: p, inference: None }, false);
        }
        let out = match self.drag.take() {
            Some(DragState::Pending { entity, .. }) => {
                if let Some(pos) = self.selection.iter().position(|e| *e == entity) {
                    self.selection.remove(pos);
                } else {
                    self.selection.push(entity);
                }
                EditOutcome::redraw()
            }
            Some(DragState::Geometry { handle }) => {
                if let (DragHandle::Point(id), Some((target, _, is_pt))) = (handle, self.drag_snap.take()) {
                    let ok = if is_pt {
                        // Keep the fixed/external point when merging.
                        let ext = data.sketch.entity(target).is_some_and(|e| e.external);
                        if ext || !data.sketch.entity(id).is_some_and(|e| e.external) { data.sketch.merge_points(id, target).is_ok() } else { data.sketch.merge_points(target, id).is_ok() }
                    } else {
                        data.sketch.add_constraint(ConstraintKind::Coincident { a: id, b: target }).is_ok()
                    };
                    if ok {
                        self.selection.retain(|e| data.sketch.entity(*e).is_some());
                        prune_exprs(data);
                    }
                }
                self.drag_snap = None;
                data.sketch.solve();
                EditOutcome::changed()
            }
            Some(DragState::Box { start }) => {
                let (lo, hi) = (start.min(p), start.max(p));
                if (hi - lo).length() > self.world_per_px * 3.0 {
                    let crossing = p.x < start.x;
                    for e in data.sketch.entities() {
                        let Some(bb) = data.sketch.entity_bbox(e.id) else { continue };
                        let inside = bb.min.x >= lo.x && bb.min.y >= lo.y && bb.max.x <= hi.x && bb.max.y <= hi.y;
                        let touches = bb.max.x >= lo.x && bb.min.x <= hi.x && bb.max.y >= lo.y && bb.min.y <= hi.y;
                        if (inside || (crossing && touches)) && !self.selection.contains(&e.id) {
                            self.selection.push(e.id);
                        }
                    }
                } else {
                    self.selection.clear();
                    self.selected_constraints.clear();
                }
                self.box_end = None;
                EditOutcome::redraw()
            }
            Some(DragState::Label { constraint, moved, additive, .. }) => {
                if moved {
                    EditOutcome::changed()
                } else {
                    self.toggle_constraint_selection(constraint, additive);
                    EditOutcome::redraw()
                }
            }
            Some(DragState::Glyph { constraint, moved, additive, .. }) => {
                if moved {
                    EditOutcome::changed()
                } else {
                    self.toggle_constraint_selection(constraint, additive);
                    EditOutcome::redraw()
                }
            }
            Some(DragState::Handle(_)) => EditOutcome::redraw(),
            Some(DragState::Text { entity, moved, .. }) => {
                if moved {
                    data.sketch.solve();
                    EditOutcome::changed()
                } else {
                    self.toggle_text_selection(data, entity);
                    EditOutcome::redraw()
                }
            }
            None => EditOutcome::none(),
        };
        self.box_end = None;
        out
    }

    fn toggle_constraint_selection(&mut self, cid: ConstraintId, _additive: bool) {
        // Onshape sketch selection is additive; clicking a selected item deselects it.
        if let Some(pos) = self.selected_constraints.iter().position(|c| *c == cid) {
            self.selected_constraints.remove(pos);
        } else {
            self.selected_constraints.push(cid);
        }
    }

    // ------------------------------------------------------------ inline value entry

    /// Value fields offered while drawing with the current tool state.
    pub fn entry_fields(&self) -> &'static [&'static str] {
        match (self.tool, self.clicks.len()) {
            (Tool::Line, n) if n >= 1 => &["Length", "Angle"],
            (Tool::Circle, 1) => &["Diameter"],
            (Tool::CornerRect | Tool::CenterRect, 1) => &["Width", "Height"],
            (Tool::Polygon | Tool::CircumPolygon, 1) => &["Diameter"],
            (Tool::Slot, 2) => &["Width"],
            _ => &[],
        }
    }

    pub fn entry_dims(&self) -> Vec<Dim> {
        self.entry_fields().iter().map(|f| if *f == "Angle" { Dim::ANGLE } else { Dim::LENGTH }).collect()
    }

    /// Cursor implied by the typed values (preview and commit position).
    fn entry_cursor(&self) -> Option<DVec2> {
        if !self.entry_active {
            return None;
        }
        let c = self.cursor?;
        let v = |i: usize| self.entry_values.get(i).copied().flatten();
        let last = self.clicks.last()?.pos;
        let first = self.clicks.first()?.pos;
        let dir_of = |a: DVec2| (c - a).try_normalize().unwrap_or(DVec2::X);
        Some(match self.tool {
            Tool::Line => {
                let len = v(0).unwrap_or((c - last).length());
                let dir = match (v(1), self.chained_back()) {
                    (None, _) => dir_of(last),
                    // Chained: the typed angle is measured from the previous
                    // line at the joint, on the cursor's side.
                    (Some(ang), Some(u)) => {
                        let (d1, d2) = (DVec2::from_angle(ang).rotate(u), DVec2::from_angle(-ang).rotate(u));
                        if d1.dot(dir_of(last)) >= d2.dot(dir_of(last)) { d1 } else { d2 }
                    }
                    (Some(ang), None) => DVec2::from_angle(ang),
                };
                last + dir * len
            }
            Tool::Circle | Tool::Polygon | Tool::CircumPolygon => {
                let r = v(0).map(|d| d * 0.5).unwrap_or((c - first).length());
                first + dir_of(first) * r
            }
            Tool::CornerRect | Tool::CenterRect => {
                let d = c - first;
                let sx = if d.x < 0.0 { -1.0 } else { 1.0 };
                let sy = if d.y < 0.0 { -1.0 } else { 1.0 };
                let k = if self.tool == Tool::CenterRect { 0.5 } else { 1.0 };
                let w = v(0).map(|w| w * k).unwrap_or(d.x.abs());
                let h = v(1).map(|h| h * k).unwrap_or(d.y.abs());
                first + DVec2::new(sx * w, sy * h)
            }
            Tool::Slot => {
                let (a, b) = (self.clicks[0].pos, self.clicks[1].pos);
                let n = (b - a).try_normalize().unwrap_or(DVec2::X).perp();
                let side = if (c - a).dot(n) < 0.0 { -1.0 } else { 1.0 };
                let w = v(0).unwrap_or(2.0 * (c - a).dot(n).abs());
                (a + b) * 0.5 + n * side * w * 0.5
            }
            _ => return None,
        })
    }

    /// Unit vector from the chain end back along the previous line, while chaining.
    fn chained_back(&self) -> Option<DVec2> {
        self.chain_end.and(self.last_line).map(|(_, u)| u)
    }

    /// Start typing values (first key press while drawing).
    pub fn begin_entry(&mut self) -> bool {
        if self.entry_fields().is_empty() {
            return false;
        }
        self.entry_active = true;
        self.entry_values = vec![None; self.entry_fields().len()];
        true
    }

    /// Commit the typed values: place the click and add driving dimensions.
    /// `texts` are the raw field texts (kept as expressions when they use variables).
    pub fn commit_entry_values(&mut self, data: &mut SketchData, texts: &[String]) -> EditOutcome {
        let Some(pos) = self.entry_cursor() else {
            self.entry_active = false;
            return EditOutcome::redraw();
        };
        let tool = self.tool;
        let values = std::mem::take(&mut self.entry_values);
        let anchor = self.clicks.first().map(|c| c.pos);
        let prev_line = self.chain_end.and(self.last_line).map(|(l, _)| l);
        self.entry_active = false;
        self.last_created.clear();
        let out = self.place(data, Click { pos, inference: None }, false);
        if self.last_created.is_empty() {
            return out;
        }
        let expr = |i: usize| texts.get(i).filter(|t| t.contains('#')).map(|t| t.trim().trim_start_matches('=').to_string());
        let v = |i: usize| values.get(i).copied().flatten();
        let off = self.world_per_px * 28.0;
        let created = self.last_created.clone();
        let sk = &data.sketch;
        let lines: Vec<(EntityId, DVec2, DVec2)> = created
            .iter()
            .filter(|e| sk.kind(**e) == Some(EntityKind::Line))
            .filter_map(|e| {
                let (a, b) = sk.endpoints(*e)?;
                Some((*e, sk.point(a)?, sk.point(b)?))
            })
            .collect();
        let mut dims: Vec<(ConstraintKind, Option<DVec2>, Option<String>)> = Vec::new();
        match tool {
            Tool::Line => {
                if let Some(&(l, a, b)) = lines.first() {
                    if let Some(len) = v(0) {
                        let n = (b - a).try_normalize().unwrap_or(DVec2::X).perp();
                        dims.push((ConstraintKind::Length { curve: l, value: len }, Some((a + b) * 0.5 + n * off), expr(0)));
                    }
                    if let Some(ang) = v(1) {
                        dims.extend(line_angle_dimension(data, l, prev_line, ang, off, expr(1)));
                    }
                }
            }
            Tool::Circle | Tool::Polygon | Tool::CircumPolygon => {
                if let Some(d) = v(0)
                    && let Some(c) = created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Circle))
                {
                    let label = anchor.map(|a| a + DVec2::new(1.0, 1.0).normalize() * (d * 0.5 + off));
                    dims.push((ConstraintKind::Diameter { curve: c, value: d }, label, expr(0)));
                }
            }
            Tool::CornerRect | Tool::CenterRect => {
                let horiz = lines.iter().filter(|(_, a, b)| (b.y - a.y).abs() < 1e-9).max_by(|x, y| x.1.y.min(x.2.y).total_cmp(&y.1.y.min(y.2.y)));
                let vert = lines.iter().filter(|(_, a, b)| (b.x - a.x).abs() < 1e-9).min_by(|x, y| x.1.x.total_cmp(&y.1.x));
                if let (Some(w), Some(&(l, a, b))) = (v(0), horiz) {
                    dims.push((ConstraintKind::Length { curve: l, value: w }, Some((a + b) * 0.5 + DVec2::Y * off), expr(0)));
                }
                if let (Some(h), Some(&(l, a, b))) = (v(1), vert) {
                    dims.push((ConstraintKind::Length { curve: l, value: h }, Some((a + b) * 0.5 - DVec2::X * off), expr(1)));
                }
            }
            Tool::Slot => {
                if let Some(w) = v(0)
                    && let Some(arc) = created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc))
                {
                    let label = curve_mid(&data.sketch, arc).map(|m| m + DVec2::new(1.0, 1.0).normalize() * off);
                    dims.push((ConstraintKind::Radius { curve: arc, value: w * 0.5 }, label, None));
                }
            }
            _ => {}
        }
        for (kind, label, ex) in dims {
            add_dimension(data, kind, label, ex.as_deref());
        }
        data.sketch.solve();
        EditOutcome::changed()
    }

    // ------------------------------------------------------------ patterns / transform

    fn pattern_transforms(&self) -> Vec<Box<dyn Fn(DVec2) -> DVec2>> {
        let mut out: Vec<Box<dyn Fn(DVec2) -> DVec2>> = Vec::new();
        match self.tool {
            Tool::LinearPattern => {
                let step = DVec2::from_angle(self.pattern_angle) * self.pattern_distance;
                for k in 1..self.pattern_count.clamp(1, 10) {
                    let off = step * k as f64;
                    out.push(Box::new(move |p| p + off));
                }
            }
            Tool::CircularPattern => {
                let n = self.circular_count.clamp(1, 10);
                if n >= 2 {
                    let full = (self.circular_angle.abs() - TAU).abs() < 1e-9;
                    let step = if full { self.circular_angle / n as f64 } else { self.circular_angle / (n - 1) as f64 };
                    for k in 1..n {
                        let t = Transform::Rotate { center: self.pattern_center, angle: step * k as f64 };
                        out.push(Box::new(move |p| t.apply(p)));
                    }
                }
            }
            Tool::Transform => {
                let (c, a, s, d) = (self.pattern_center, self.xf_angle, self.xf_scale, self.xf_offset);
                out.push(Box::new(move |p| c + DVec2::from_angle(a).rotate(p - c) * s + d));
            }
            _ => {}
        }
        out
    }

    /// Apply the pattern/transform tool to the selection.
    pub fn apply_pattern(&mut self, data: &mut SketchData) -> EditOutcome {
        let mut ids: Vec<EntityId> = self.selection.iter().copied().filter(|e| data.sketch.entity(*e).is_some()).collect();
        // Text can be transformed but not patterned inside a sketch (Onshape).
        if self.tool != Tool::Transform {
            let n = ids.len();
            ids.retain(|e| data.text_of_entity(*e).is_none());
            if ids.len() != n {
                self.message = Some("Text can't be patterned in a sketch; use a feature pattern".into());
                if ids.is_empty() {
                    return EditOutcome::redraw();
                }
            }
        }
        if ids.is_empty() {
            self.message = Some("Select entities first".into());
            return EditOutcome::redraw();
        }
        let sk = &mut data.sketch;
        let r = match self.tool {
            Tool::LinearPattern => sk.linear_pattern(&ids, DVec2::from_angle(self.pattern_angle) * self.pattern_distance, self.pattern_count.max(1)),
            Tool::CircularPattern => sk.circular_pattern(&ids, self.pattern_center, self.circular_count.max(1), self.circular_angle),
            Tool::Transform => {
                let c = self.pattern_center;
                let mut r = Ok(Default::default());
                if self.xf_angle.abs() > 1e-12 {
                    r = sk.transform(&ids, Transform::Rotate { center: c, angle: self.xf_angle });
                }
                if r.is_ok() && (self.xf_scale - 1.0).abs() > 1e-12 && self.xf_scale > 0.0 {
                    r = sk.transform(&ids, Transform::Scale { center: c, factor: self.xf_scale });
                }
                if r.is_ok() && self.xf_offset.length() > 1e-12 {
                    r = sk.transform(&ids, Transform::Translate(self.xf_offset));
                }
                r
            }
            _ => return EditOutcome::none(),
        };
        match r {
            Ok(res) => {
                let created = res.created.clone();
                self.finish_create(data, &created);
                if self.tool == Tool::Transform {
                    self.xf_offset = DVec2::ZERO;
                    self.xf_angle = 0.0;
                    self.xf_scale = 1.0;
                }
                self.selection.clear();
                EditOutcome::changed()
            }
            Err(e) => {
                self.message = Some(format!("{e}"));
                EditOutcome::redraw()
            }
        }
    }

    /// On-canvas manipulators of the active pattern / transform tool.
    pub fn pattern_handles(&self, sk: &Sketch) -> Vec<(PatternHandle, DVec2)> {
        let mut out = Vec::new();
        let px = self.world_per_px;
        match self.tool {
            Tool::LinearPattern => {
                let Some(c) = self.selection_center(sk) else { return out };
                let dir = DVec2::from_angle(self.pattern_angle);
                let step = dir * self.pattern_distance;
                out.push((PatternHandle::LinearStep, c + step));
                let n = self.pattern_count.max(2);
                out.push((PatternHandle::LinearCount, c + step * (n - 1) as f64 + dir * px * 22.0));
            }
            Tool::CircularPattern => {
                out.push((PatternHandle::Center, self.pattern_center));
                if let Some((r, a0)) = self.circular_arm(sk) {
                    let (step, _) = self.circular_step();
                    out.push((PatternHandle::CircularAngle, self.pattern_center + DVec2::from_angle(a0 + self.circular_angle) * (r + px * 18.0)));
                    out.push((PatternHandle::CircularCount, self.pattern_center + DVec2::from_angle(a0 + step) * (r - px * 18.0).max(r * 0.5)));
                }
            }
            Tool::Transform => {
                let c = self.pattern_center;
                let m = c + self.xf_offset;
                out.push((PatternHandle::Center, c));
                out.push((PatternHandle::Move, m + DVec2::new(px * 14.0, px * 14.0)));
                out.push((PatternHandle::Rotate, m + DVec2::from_angle(self.xf_angle) * px * 70.0));
                out.push((PatternHandle::Scale, m + DVec2::from_angle(self.xf_angle + PI * 0.25) * px * 45.0 * self.xf_scale));
            }
            _ => {}
        }
        out
    }

    /// Radius and start angle of the circular pattern arm (selection center around the pattern center).
    fn circular_arm(&self, sk: &Sketch) -> Option<(f64, f64)> {
        let c = self.selection_center(sk)?;
        let d = c - self.pattern_center;
        let r = d.length();
        (r > self.world_per_px * 4.0).then(|| (r, d.to_angle()))
    }

    /// Angle between circular instances and whether the pattern is a full circle.
    fn circular_step(&self) -> (f64, bool) {
        let n = self.circular_count.max(2);
        let full = (self.circular_angle.abs() - TAU).abs() < 1e-9;
        (if full { self.circular_angle / n as f64 } else { self.circular_angle / (n - 1) as f64 }, full)
    }

    fn handle_at(&self, sk: &Sketch, p: DVec2) -> Option<PatternHandle> {
        let r = self.world_per_px * 9.0;
        self.pattern_handles(sk).into_iter().map(|(h, q)| (h, (q - p).length())).filter(|x| x.1 <= r).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0)
    }

    /// Updates the pattern / transform parameters from a dragged manipulator.
    fn drag_handle(&mut self, sk: &Sketch, h: PatternHandle, p: DVec2) {
        let px = self.world_per_px;
        match h {
            PatternHandle::LinearStep => {
                if let Some(c) = self.selection_center(sk) {
                    let d = p - c;
                    if d.length() > px * 3.0 {
                        self.pattern_angle = d.to_angle();
                        self.pattern_distance = d.length();
                    }
                }
            }
            PatternHandle::LinearCount => {
                if let Some(c) = self.selection_center(sk) {
                    let along = (p - c).dot(DVec2::from_angle(self.pattern_angle)) - px * 22.0;
                    self.pattern_count = ((along / self.pattern_distance.max(1e-12)).round().max(1.0) as usize + 1).clamp(2, 100);
                }
            }
            PatternHandle::Center => self.pattern_center = self.inference_free_snap(sk, p),
            PatternHandle::CircularAngle => {
                if let Some((_, a0)) = self.circular_arm(sk) {
                    let mut a = noshape_sketch::geom::ccw_sweep(a0, (p - self.pattern_center).to_angle());
                    if a < 3f64.to_radians() || a > TAU - 3f64.to_radians() {
                        a = TAU;
                    }
                    self.circular_angle = a;
                }
            }
            PatternHandle::CircularCount => {
                if let Some((_, a0)) = self.circular_arm(sk) {
                    let a = noshape_sketch::geom::ccw_sweep(a0, (p - self.pattern_center).to_angle()).max(1f64.to_radians());
                    let (_, full) = self.circular_step();
                    let n = (self.circular_angle / a).round().max(1.0) as usize;
                    self.circular_count = if full { n } else { n + 1 }.clamp(2, 100);
                }
            }
            PatternHandle::Move => self.xf_offset = p - DVec2::new(px * 14.0, px * 14.0) - self.pattern_center,
            PatternHandle::Rotate => {
                let d = p - (self.pattern_center + self.xf_offset);
                if d.length() > px * 3.0 {
                    let mut a = d.to_angle();
                    // Snap to whole 15 degree steps near them.
                    let step = 15f64.to_radians();
                    if (a / step - (a / step).round()).abs() * step < 2f64.to_radians() {
                        a = (a / step).round() * step;
                    }
                    self.xf_angle = a;
                }
            }
            PatternHandle::Scale => {
                let d = (p - (self.pattern_center + self.xf_offset)).length();
                self.xf_scale = (d / (px * 45.0)).clamp(0.01, 100.0);
            }
        }
    }

    // ------------------------------------------------------------ text

    /// Lower-left corner and height of a text box dragged between two corners.
    fn text_box_from(&self, a: DVec2, b: DVec2) -> (DVec2, f64) {
        let lo = a.min(b);
        let h = (b.y - a.y).abs();
        if h < self.world_per_px * 4.0 { (a, self.text_height) } else { (lo, h) }
    }

    /// The text whose box is (partly) selected.
    pub fn selected_text(&self, data: &SketchData) -> Option<u32> {
        data.texts.iter().find(|t| t.box_lines.iter().any(|l| self.selection.contains(&EntityId(*l)))).map(|t| t.id)
    }

    fn toggle_text_selection(&mut self, data: &SketchData, e: EntityId) {
        let Some(t) = data.text_of_entity(e).and_then(|id| data.texts.iter().find(|t| t.id == id)) else { return };
        let lines: Vec<EntityId> = t.box_lines.iter().map(|l| EntityId(*l)).collect();
        if lines.iter().any(|l| self.selection.contains(l)) {
            self.selection.retain(|x| !lines.contains(x));
        } else {
            self.selection.extend(lines);
        }
    }

    // ------------------------------------------------------------ dimensions

    fn dimension_click(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>) -> EditOutcome {
        if let Some(e) = hovered
            && !self.picks.iter().any(|(x, _)| *x == e)
            && self.picks.len() < 2
        {
            self.picks.push((e, p));
            return EditOutcome::redraw();
        }
        if self.picks.is_empty() {
            return EditOutcome::none();
        }
        // Click in empty space places the dimension.
        let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
        self.picks.clear();
        let Some(kind) = smart_dimension(&data.sketch, &ids, p) else {
            self.message = Some("A dimension cannot be created between these items".into());
            return EditOutcome::redraw();
        };
        let Some(cid) = add_dimension(data, kind, Some(p), None) else {
            self.message = Some("Could not create the dimension".into());
            return EditOutcome::redraw();
        };
        let c = data.sketch.constraint(cid);
        let value = c.and_then(|c| c.kind.value()).unwrap_or(0.0);
        let is_angle = c.is_some_and(|c| is_angle_kind(&c.kind));
        let driving = c.is_some_and(|c| c.driving);
        if driving {
            let q = Quantity::new(value, if is_angle { Dim::ANGLE } else { Dim::LENGTH });
            self.dim_edit = Some(DimEdit { constraint: cid, text: noshape_units::format_quantity(q, &UnitPrefs::default()) });
        } else {
            self.message = Some("Dimension would over-constrain the sketch; added as driven".into());
        }
        EditOutcome::changed()
    }

    /// Apply a typed dimension value (expression). Returns an error message.
    pub fn apply_dimension(&mut self, data: &mut SketchData, cid: ConstraintId, text: &str, units: &UnitPrefs, vars: &dyn noshape_units::VariableScope) -> Result<(), String> {
        let Some(c) = data.sketch.constraint(cid) else { return Err("Dimension no longer exists".into()) };
        let dim = if is_angle_kind(&c.kind) { Dim::ANGLE } else { Dim::LENGTH };
        let q = noshape_units::evaluate_as(text, dim, units, vars).map_err(|e| e.to_string())?;
        if dim == Dim::LENGTH && q.value <= 0.0 && matches!(c.kind, ConstraintKind::Radius { .. } | ConstraintKind::Diameter { .. } | ConstraintKind::Length { .. }) {
            return Err("Value must be positive".into());
        }
        data.sketch.set_dimension_value(cid, q.value).map_err(|e| e.to_string())?;
        // Plain numbers are stored as literals; expressions keep their text.
        data.dimension_exprs.insert(cid.0, ExprValue::new(text.trim()));
        let report = data.sketch.solve();
        if !report.is_solved() {
            return Err("The sketch could not be solved with that value".into());
        }
        Ok(())
    }

    fn label_pos(&self, data: &SketchData, cid: ConstraintId, k: &ConstraintKind) -> Option<DVec2> {
        data.dimension_labels.get(&cid.0).map(|l| DVec2::new(l[0], l[1])).or_else(|| default_label_pos(&data.sketch, k, self.world_per_px))
    }

    /// Dimension label positions and texts for display (local coords).
    pub fn dimension_labels(&self, data: &SketchData, units: &UnitPrefs) -> Vec<(ConstraintId, DVec2, String, bool)> {
        let mut out = Vec::new();
        for c in data.sketch.constraints() {
            let Some(v) = c.kind.value() else { continue };
            let Some(pos) = self.label_pos(data, c.id, &c.kind) else { continue };
            let q = Quantity::new(v, if is_angle_kind(&c.kind) { Dim::ANGLE } else { Dim::LENGTH });
            let mut text = match data.dimension_exprs.get(&c.id.0) {
                Some(e) if e.text.contains('#') => format!("{} = {}", e.text, noshape_units::format_quantity(q, units)),
                _ => noshape_units::format_quantity(q, units),
            };
            match c.kind {
                ConstraintKind::Diameter { .. } => text = format!("Ø{text}"),
                ConstraintKind::Radius { .. } => text = format!("R{text}"),
                _ => {}
            }
            if !c.driving {
                text = format!("({text})");
            }
            out.push((c.id, pos, text, c.driving));
        }
        out
    }

    /// Dimension graphics (extension/dimension lines, arrows, arcs) in local coords.
    pub fn dimension_graphics(&self, data: &SketchData) -> Vec<DimGraphic> {
        let sk = &data.sketch;
        let s = self.world_per_px;
        let mut out = Vec::new();
        for c in sk.constraints() {
            if c.kind.value().is_none() {
                continue;
            }
            let Some(pos) = self.label_pos(data, c.id, &c.kind) else { continue };
            let mut segs = Vec::new();
            dim_segments(sk, &c.kind, pos, s, &mut segs);
            out.push(DimGraphic { constraint: c.id, label: pos, text: String::new(), driving: c.driving, segments: segs });
        }
        out
    }

    /// Constraint glyph anchors for all geometric constraints.
    pub fn constraint_glyphs(&self, data: &SketchData) -> Vec<GlyphInfo> {
        let sk = &data.sketch;
        let mut out = Vec::new();
        // Internal shape constraints of text boxes (perpendicular / parallel
        // edges) are not shown; the horizontal one is (delete it to rotate).
        let text_lines: std::collections::HashSet<u32> = data.texts.iter().flat_map(|t| t.box_lines.iter().copied()).collect();
        for c in sk.constraints() {
            let Some(icon) = constraint_icon(&c.kind) else { continue };
            if matches!(c.kind, ConstraintKind::Parallel { .. } | ConstraintKind::Perpendicular { .. }) && c.kind.entities().iter().all(|e| text_lines.contains(&e.0)) {
                continue;
            }
            for e in glyph_entities(sk, &c.kind) {
                let Some(anchor) = glyph_anchor(sk, e) else { continue };
                let offset = data.glyph_offsets.get(&glyph_key(c.id, e)).map(|o| DVec2::new(o[0], o[1]));
                out.push(GlyphInfo { constraint: c.id, icon, entity: e, anchor, on_point: is_point(sk, e), offset });
            }
        }
        out
    }

    // ------------------------------------------------------------ editing commands

    pub fn delete_selection(&mut self, data: &mut SketchData) -> bool {
        let mut changed = false;
        for c in self.selected_constraints.drain(..) {
            if data.sketch.remove_constraint(c).is_ok() {
                data.dimension_exprs.remove(&c.0);
                data.dimension_labels.remove(&c.0);
                changed = true;
            }
        }
        self.hover_constraint = None;
        if !self.selection.is_empty() {
            let ids = std::mem::take(&mut self.selection);
            data.sketch.remove_entities(&ids);
            changed = true;
        }
        if changed {
            data.external_refs.retain(|k, _| data.sketch.entity(EntityId(*k)).is_some());
            prune_exprs(data);
            data.sketch.solve();
        }
        changed
    }

    /// Deletes one constraint or dimension (context menu).
    pub fn delete_constraint(&mut self, data: &mut SketchData, cid: ConstraintId) -> bool {
        self.selected_constraints.retain(|c| *c != cid);
        if self.hover_constraint == Some(cid) {
            self.hover_constraint = None;
        }
        if data.sketch.remove_constraint(cid).is_err() {
            return false;
        }
        prune_exprs(data);
        data.sketch.solve();
        true
    }

    /// Toggles a dimension between driving and driven. A dimension that
    /// would over-constrain the sketch stays driven.
    pub fn toggle_driving(&mut self, data: &mut SketchData, cid: ConstraintId) -> Result<(), String> {
        let Some(c) = data.sketch.constraint(cid) else { return Err("Dimension no longer exists".into()) };
        if c.driving {
            data.sketch.set_driving(cid, false).map_err(|e| e.to_string())?;
            data.dimension_exprs.remove(&cid.0);
            data.sketch.solve();
            return Ok(());
        }
        data.sketch.set_driving(cid, true).map_err(|e| e.to_string())?;
        let report = data.sketch.solve();
        if report.conflicting().contains(&cid) || report.redundant.contains(&cid) || !report.is_solved() {
            let _ = data.sketch.set_driving(cid, false);
            data.sketch.solve();
            return Err("Making this dimension driving would over-constrain the sketch".into());
        }
        Ok(())
    }

    pub fn toggle_construction(&mut self, data: &mut SketchData) -> bool {
        if self.selection.is_empty() {
            self.construction = !self.construction;
            return false;
        }
        for id in &self.selection {
            if is_point(&data.sketch, *id) {
                continue;
            }
            let cur = data.sketch.entity(*id).is_some_and(|e| e.construction);
            let _ = data.sketch.set_construction(*id, !cur);
        }
        true
    }

    /// Apply a constraint tool to the current selection if it fits; else
    /// switch to the constraint tool.
    pub fn constraint_command(&mut self, data: &mut SketchData, c: ConstraintTool) -> bool {
        if self.selection.len() >= c.arity()
            && let Some(kind) = make_constraint(&data.sketch, c, &self.selection)
            && data.sketch.add_constraint(kind).is_ok()
        {
            data.sketch.solve();
            self.selection.clear();
            return true;
        }
        self.set_tool(Tool::Constraint(c));
        false
    }

    // ------------------------------------------------------------ overlay

    /// Preview geometry, inference guides, picks, box selection and dimension
    /// graphics (world space).
    pub fn overlay(&self, data: &SketchData) -> (LineData, PointData) {
        let mut lines = LineData::default();
        let mut points = PointData::default();
        let frame = self.frame;
        let w = |p: DVec2| -> [f32; 3] {
            let v = frame.to_world(p) + frame.normal * 1e-6;
            [v.x as f32, v.y as f32, v.z as f32]
        };
        let preview_color = [0x44, 0x9c, 0xcd, 255];
        let guide_color = [0xff, 0xc2, 0x0e, 255];
        let on_top = LineSegment::ON_TOP;
        let mut poly = |pts: &[DVec2], color: [u8; 4], dashed: bool| {
            let pts3: Vec<[f32; 3]> = pts.iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, color, 0, 1.5, on_top | if dashed { LineSegment::DASHED } else { 0 });
        };
        let circle_pts = |c: DVec2, r: f64, n: usize| -> Vec<DVec2> { (0..=n).map(|i| c + DVec2::from_angle(i as f64 / n as f64 * TAU) * r).collect() };
        let cursor = self.snapped().map(|s| s.0);
        if let Some(c) = cursor {
            let anchors: Vec<DVec2> = self.clicks.iter().map(|k| k.pos).collect();
            match self.tool {
                Tool::Line => {
                    if let Some(a) = anchors.last() {
                        poly(&[*a, c], preview_color, false);
                    }
                }
                Tool::CornerRect => {
                    if let Some(a) = anchors.first() {
                        poly(&[*a, DVec2::new(c.x, a.y), c, DVec2::new(a.x, c.y), *a], preview_color, false);
                    }
                }
                Tool::CenterRect => {
                    if let Some(a) = anchors.first() {
                        let d = c - *a;
                        let q = [*a + DVec2::new(-d.x, -d.y), *a + DVec2::new(d.x, -d.y), *a + d, *a + DVec2::new(-d.x, d.y)];
                        poly(&[q[0], q[1], q[2], q[3], q[0]], preview_color, false);
                        poly(&[q[0], q[2]], preview_color, true);
                        poly(&[q[1], q[3]], preview_color, true);
                    }
                }
                Tool::AlignedRect => match anchors.len() {
                    1 => poly(&[anchors[0], c], preview_color, false),
                    2 => {
                        let (a, b) = (anchors[0], anchors[1]);
                        let n = (b - a).try_normalize().unwrap_or(DVec2::X).perp();
                        let h = (c - a).dot(n);
                        poly(&[a, b, b + n * h, a + n * h, a], preview_color, false);
                    }
                    _ => {}
                },
                Tool::Circle => {
                    if let Some(a) = anchors.first() {
                        poly(&circle_pts(*a, (c - *a).length(), 72), preview_color, false);
                        poly(&[*a, c], preview_color, true);
                    }
                }
                Tool::Polygon | Tool::CircumPolygon => {
                    if let Some(a) = anchors.first() {
                        let r = (c - *a).length();
                        let n = self.polygon_sides.clamp(3, 50);
                        let start = (c - *a).to_angle();
                        // Inscribed: vertices on the circle; circumscribed: edge midpoints on it.
                        let (rv, off) = if self.tool == Tool::Polygon { (r, 0.0) } else { (r / (PI / n as f64).cos(), PI / n as f64) };
                        let pts: Vec<DVec2> = (0..=n).map(|i| *a + DVec2::from_angle(start + off + i as f64 / n as f64 * TAU) * rv).collect();
                        poly(&pts, preview_color, false);
                        poly(&circle_pts(*a, r, 72), preview_color, true);
                    }
                }
                Tool::ThreePointCircle => match anchors.len() {
                    1 => poly(&[anchors[0], c], preview_color, true),
                    2 => {
                        if let Some((ctr, r)) = noshape_sketch::shapes::circle_through(anchors[0], anchors[1], c) {
                            poly(&circle_pts(ctr, r, 72), preview_color, false);
                        }
                    }
                    _ => {}
                },
                Tool::Arc3 => match anchors.len() {
                    1 => poly(&[anchors[0], c], preview_color, true),
                    2 => {
                        if let Some((ctr, r)) = noshape_sketch::shapes::circle_through(anchors[0], c, anchors[1]) {
                            let a0 = (anchors[0] - ctr).to_angle();
                            let a1 = (anchors[1] - ctr).to_angle();
                            let am = (c - ctr).to_angle();
                            let (s, e) = arc_span(a0, a1, am);
                            let pts: Vec<DVec2> = (0..=48).map(|i| ctr + DVec2::from_angle(s + (e - s) * i as f64 / 48.0) * r).collect();
                            poly(&pts, preview_color, false);
                        }
                    }
                    _ => {}
                },
                Tool::CenterArc => match anchors.len() {
                    1 => {
                        poly(&[anchors[0], c], preview_color, true);
                        poly(&circle_pts(anchors[0], (c - anchors[0]).length(), 72), preview_color, true);
                    }
                    2 => {
                        let ctr = anchors[0];
                        let r = (anchors[1] - ctr).length();
                        let a0 = (anchors[1] - ctr).to_angle();
                        let sweep = noshape_sketch::geom::ccw_sweep(a0, (c - ctr).to_angle());
                        let pts: Vec<DVec2> = (0..=48).map(|i| ctr + DVec2::from_angle(a0 + sweep * i as f64 / 48.0) * r).collect();
                        poly(&pts, preview_color, false);
                        poly(&[ctr, anchors[1]], preview_color, true);
                        poly(&[ctr, c], preview_color, true);
                    }
                    _ => {}
                },
                Tool::TangentArc => {
                    if let Some(from) = self.chain_end
                        && let Some(a) = data.sketch.point(from)
                    {
                        let mut tmp = data.sketch.clone();
                        if let Ok(ids) = tmp.tangent_arc(from, c)
                            && let Some(&arc) = ids.curves.first()
                        {
                            poly(&tmp.tessellate(arc, self.world_per_px), preview_color, false);
                        } else {
                            poly(&[a, c], preview_color, true);
                        }
                    }
                }
                Tool::Slot => match anchors.len() {
                    1 => poly(&[anchors[0], c], preview_color, true),
                    2 => {
                        let (a, b) = (anchors[0], anchors[1]);
                        let d = (b - a).try_normalize().unwrap_or(DVec2::X);
                        let n = d.perp();
                        let r = dist_point_line(c, a, b).max(1e-9);
                        let ang = d.to_angle();
                        let mut pts = vec![a + n * r, b + n * r];
                        pts.extend((1..24).map(|i| b + DVec2::from_angle(ang + PI * 0.5 - PI * i as f64 / 24.0) * r));
                        pts.extend([b - n * r, a - n * r]);
                        pts.extend((1..=24).map(|i| a + DVec2::from_angle(ang - PI * 0.5 - PI * i as f64 / 24.0) * r));
                        poly(&pts, preview_color, false);
                        poly(&[a, b], preview_color, true);
                    }
                    _ => {}
                },
                Tool::Ellipse => match anchors.len() {
                    1 => {
                        poly(&[anchors[0], c], preview_color, true);
                        poly(&circle_pts(anchors[0], (c - anchors[0]).length(), 72), preview_color, true);
                    }
                    2 => {
                        let ctr = anchors[0];
                        let major = anchors[1] - ctr;
                        let u = major.try_normalize().unwrap_or(DVec2::X);
                        let minor = (c - ctr).dot(u.perp()).abs().max(1e-9);
                        let pts: Vec<DVec2> = (0..=72).map(|i| {
                            let t = i as f64 / 72.0 * TAU;
                            ctr + major * t.cos() + u.perp() * minor * t.sin()
                        }).collect();
                        poly(&pts, preview_color, false);
                        poly(&[ctr - major, anchors[1]], preview_color, true);
                    }
                    _ => {}
                },
                Tool::Spline => {
                    let mut pts = anchors.clone();
                    pts.push(c);
                    if pts.len() >= 2 {
                        let mut tmp = Sketch::new();
                        if let Ok(id) = tmp.add_fit_spline(&pts) {
                            poly(&tmp.tessellate(id, self.world_per_px), preview_color, false);
                        }
                    }
                }
                Tool::Bezier => {
                    let mut pts = anchors.clone();
                    pts.push(c);
                    if pts.len() >= 2 {
                        poly(&pts, [0x80, 0x80, 0x80, 255], true);
                        let curve: Vec<DVec2> = (0..=64).map(|i| de_casteljau(&pts, i as f64 / 64.0)).collect();
                        poly(&curve, preview_color, false);
                    }
                }
                Tool::Conic => match anchors.len() {
                    1 => poly(&[anchors[0], c], preview_color, true),
                    2 => {
                        let (p0, p2) = (anchors[0], anchors[1]);
                        poly(&[p0, c, p2], [0x80, 0x80, 0x80, 255], true);
                        let rho = self.conic_rho.clamp(0.01, 0.99);
                        let wgt = rho / (1.0 - rho);
                        let pts: Vec<DVec2> = (0..=64)
                            .map(|i| {
                                let t = i as f64 / 64.0;
                                let (b0, b1, b2) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t) * wgt, t * t);
                                (p0 * b0 + c * b1 + p2 * b2) / (b0 + b1 + b2)
                            })
                            .collect();
                        poly(&pts, preview_color, false);
                    }
                    _ => {}
                },
                Tool::Text => {
                    let (origin, h) = match anchors.first() {
                        Some(a) => self.text_box_from(*a, c),
                        None => (c, self.text_height),
                    };
                    let text = if self.text_string.trim().is_empty() { "Text" } else { self.text_string.as_str() };
                    let layout = noshape_sketch::text::layout_text(text, &self.text_font, self.text_bold, self.text_italic);
                    let wdt = layout.width.max(0.3) * h;
                    poly(&[origin, origin + DVec2::X * wdt, origin + DVec2::new(wdt, h), origin + DVec2::Y * h, origin], preview_color, true);
                    for pl in layout.polylines(6) {
                        let pts: Vec<DVec2> = pl.iter().map(|q| origin + *q * h).collect();
                        poly(&pts, preview_color, false);
                    }
                }
                _ => {}
            }
            if self.tool.draws() {
                points.points.push(PointSprite { position: w(c), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
            }
        }
        // Split marker on the hovered curve.
        if self.tool == Tool::Split
            && let (Some(h), Some(c)) = (self.hover, self.cursor)
            && let Some(curve) = data.sketch.curve(h)
        {
            let q = curve.closest(c).1;
            points.points.push(PointSprite { position: w(q), color: guide_color, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
        }
        // Pattern / transform previews.
        if self.tool.is_pattern() && !self.selection.is_empty() {
            let tol = self.world_per_px;
            let shapes: Vec<Vec<DVec2>> = self.selection.iter().filter(|e| !is_point(&data.sketch, **e)).map(|e| data.sketch.tessellate(*e, tol * 0.25)).collect();
            for t in self.pattern_transforms() {
                for s in &shapes {
                    let pts: Vec<DVec2> = s.iter().map(|p| t(*p)).collect();
                    poly(&pts, preview_color, false);
                }
            }
        }
        // Pattern / transform manipulators.
        if self.tool.is_pattern() {
            let px = self.world_per_px;
            let handle_color = [0x4c, 0xc9, 0xf0, 255];
            let active = match &self.drag {
                Some(DragState::Handle(h)) => Some(*h),
                _ => None,
            };
            let sel_c = self.selection_center(&data.sketch);
            for (h, q) in self.pattern_handles(&data.sketch) {
                match h {
                    PatternHandle::LinearStep => {
                        if let Some(c) = sel_c {
                            let dir = DVec2::from_angle(self.pattern_angle);
                            let end = c + dir * self.pattern_distance * (self.pattern_count.max(2) - 1) as f64;
                            poly(&[c, end], guide_color, true);
                            let n = dir.perp();
                            poly(&[q - dir * px * 9.0 + n * px * 5.0, q, q - dir * px * 9.0 - n * px * 5.0], guide_color, false);
                        }
                    }
                    PatternHandle::Center => {
                        let r = px * 7.0;
                        poly(&[q - DVec2::X * r, q + DVec2::X * r], guide_color, false);
                        poly(&[q - DVec2::Y * r, q + DVec2::Y * r], guide_color, false);
                    }
                    PatternHandle::CircularAngle => {
                        if let Some((r, a0)) = self.circular_arm(&data.sketch) {
                            let rr = r + px * 18.0;
                            let n = ((self.circular_angle.abs() / TAU * 96.0).ceil() as usize).clamp(4, 96);
                            let pts: Vec<DVec2> = (0..=n).map(|i| self.pattern_center + DVec2::from_angle(a0 + self.circular_angle * i as f64 / n as f64) * rr).collect();
                            poly(&pts, guide_color, true);
                            poly(&[self.pattern_center, self.pattern_center + DVec2::from_angle(a0) * rr], guide_color, true);
                        }
                    }
                    PatternHandle::Rotate => {
                        let m = self.pattern_center + self.xf_offset;
                        poly(&[m, q], guide_color, true);
                        poly(&circle_pts(m, px * 70.0, 96), [0xff, 0xc2, 0x0e, 110], true);
                    }
                    PatternHandle::Scale => {
                        let m = self.pattern_center + self.xf_offset;
                        poly(&[m, q], guide_color, true);
                        let d = (q - m).try_normalize().unwrap_or(DVec2::X);
                        let n = d.perp();
                        let s = px * 6.0;
                        poly(&[q + d * s, q - d * s * 0.6 + n * s, q - d * s * 0.6 - n * s, q + d * s], handle_color, false);
                    }
                    PatternHandle::Move => {
                        let m = self.pattern_center + self.xf_offset;
                        let s = px * 5.0;
                        poly(&[m, q], guide_color, true);
                        poly(&[q + DVec2::new(-s, -s), q + DVec2::new(s, -s), q + DVec2::new(s, s), q + DVec2::new(-s, s), q + DVec2::new(-s, -s)], handle_color, false);
                    }
                    _ => {}
                }
                let size = if active == Some(h) { 13.0 } else { 10.0 };
                points.points.push(PointSprite { position: w(q), color: if active == Some(h) { guide_color } else { handle_color }, pick_id: 0, size, style: on_top, _pad: 0 });
            }
        }
        // Related entities (constraint/dimension tool picks, mirror axis, entities
        // of the selected/hovered constraint): Onshape's yellow highlight.
        let related = self.picked(Some(data));
        for e in &related {
            if is_point(&data.sketch, *e) {
                if let Some(p) = data.sketch.point(*e) {
                    points.points.push(PointSprite { position: w(p), color: [0xf2, 0xff, 0x80, 200], pick_id: 0, size: 11.0, style: on_top, _pad: 0 });
                }
            } else {
                let pts3: Vec<[f32; 3]> = data.sketch.tessellate(*e, self.world_per_px * 0.5).iter().map(|p| w(*p)).collect();
                lines.push_polyline(&pts3, [0xf2, 0xff, 0x80, 150], 0, 5.0, on_top);
            }
        }
        // Control polygons of selected/hovered B-splines (Bezier editing).
        for e in self.selection.iter().chain(self.hover.iter()) {
            if let Some(noshape_sketch::Entity { geometry: noshape_sketch::Geometry::BSpline { control, .. }, .. }) = data.sketch.entity(*e) {
                let pts3: Vec<[f32; 3]> = control.iter().filter_map(|c| data.sketch.point(*c)).map(w).collect();
                lines.push_polyline(&pts3, [0x90, 0x90, 0x90, 255], 0, 1.0, on_top | LineSegment::DASHED);
            }
        }
        if let Some(inf) = &self.inference
            && !self.entry_active
            && let Some(r) = inf.reference
        {
            let pts3 = [w(r), w(inf.position)];
            lines.push_polyline(&pts3, guide_color, 0, 1.0, on_top | LineSegment::DASHED);
        }
        if let Some(inf) = &self.inference
            && !self.entry_active
        {
            points.points.push(PointSprite { position: w(inf.position), color: guide_color, pick_id: 0, size: 8.0, style: on_top, _pad: 0 });
        }
        if let Some((_, q, _)) = self.drag_snap {
            points.points.push(PointSprite { position: w(q), color: guide_color, pick_id: 0, size: 10.0, style: on_top, _pad: 0 });
        }
        // Box selection rectangle.
        if let (Some(DragState::Box { start }), Some(end)) = (&self.drag, self.box_end) {
            let color = if end.x < start.x { [0x4a, 0xde, 0x80, 255] } else { [0x4c, 0xc9, 0xf0, 255] };
            let a = *start;
            let b = end;
            let pts3: Vec<[f32; 3]> = [a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y), a].iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, color, 0, 1.0, on_top | if end.x < start.x { LineSegment::DASHED } else { 0 });
        }
        // Dimension graphics.
        for g in self.dimension_graphics(data) {
            let selected = self.selected_constraints.contains(&g.constraint);
            let hovered = self.hover_constraint == Some(g.constraint);
            let color = if selected {
                [0xff, 0xb0, 0x20, 255]
            } else if hovered {
                [0xfc, 0xda, 0x7f, 255]
            } else if self.conflicting.contains(&g.constraint) {
                [0xff, 0x6b, 0x6b, 255]
            } else if g.driving {
                [0xd6, 0xda, 0xe2, 255]
            } else {
                [0x80, 0x88, 0x96, 255]
            };
            let width = if selected { 1.8 } else { 1.0 };
            for s in &g.segments {
                lines.segments.push(LineSegment { a: w(s[0]), b: w(s[1]), color, pick_id: 0, width, style: on_top });
            }
        }
        (lines, points)
    }

    /// Entities highlighted as related (picks, mirror axis, entities of the
    /// selected / hovered constraints).
    pub fn picked(&self, data: Option<&SketchData>) -> Vec<EntityId> {
        let mut v: Vec<EntityId> = self.picks.iter().map(|p| p.0).collect();
        v.extend(self.mirror_axis);
        if let Some(data) = data {
            for cid in self.selected_constraints.iter().chain(self.hover_constraint.iter()) {
                if let Some(c) = data.sketch.constraint(*cid) {
                    v.extend(c.kind.entities());
                }
            }
        }
        v
    }
}

/// Constraint or angle dimension for a typed line angle. The first line of a
/// chain is measured from horizontal (a construction reference line equal to
/// the line is added at its start); later lines are measured from the
/// previous line at the joint. Right and straight angles become
/// horizontal/vertical or perpendicular/parallel constraints instead.
fn line_angle_dimension(data: &mut SketchData, line: EntityId, prev: Option<EntityId>, ang: f64, off: f64, expr: Option<String>) -> Option<(ConstraintKind, Option<DVec2>, Option<String>)> {
    let (s, e) = data.sketch.endpoints(line)?;
    let (ps, pe) = (data.sketch.point(s)?, data.sketch.point(e)?);
    let dir = (pe - ps).try_normalize()?;
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    if let Some(prev) = prev.filter(|p| data.sketch.entity(*p).is_some()) {
        let deg = ang.to_degrees().rem_euclid(360.0);
        let deg = if deg > 180.0 { 360.0 - deg } else { deg };
        if near(deg, 90.0) {
            let _ = data.sketch.add_constraint(ConstraintKind::Perpendicular { a: prev, b: line });
            return None;
        }
        if near(deg, 0.0) || near(deg, 180.0) {
            let _ = data.sketch.add_constraint(ConstraintKind::Parallel { a: prev, b: line });
            return None;
        }
        let (a0, a1) = data.sketch.endpoints(prev)?;
        let far = if a1 == s { a0 } else { a1 };
        let back = (data.sketch.point(far)? - ps).try_normalize()?;
        let bis = (back + dir).try_normalize().unwrap_or(dir.perp());
        return Some((ConstraintKind::Angle { a: prev, b: line, value: deg.to_radians() }, Some(ps + bis * off * 1.6), expr));
    }
    let deg = ang.to_degrees().rem_euclid(180.0);
    if near(deg, 0.0) || near(deg, 180.0) {
        let _ = data.sketch.add_constraint(ConstraintKind::Horizontal { a: line, b: None });
        return None;
    }
    if near(deg, 90.0) {
        let _ = data.sketch.add_constraint(ConstraintKind::Vertical { a: line, b: None });
        return None;
    }
    // Horizontal construction reference on the side the line leans to.
    let ref_dir = if dir.x >= 0.0 { DVec2::X } else { -DVec2::X };
    let len = (pe - ps).length();
    let q = data.sketch.add_point(ps + ref_dir * len);
    let r = data.sketch.add_line_between(s, q).ok()?;
    let _ = data.sketch.set_construction(r, true);
    let _ = data.sketch.add_constraint(ConstraintKind::Horizontal { a: r, b: None });
    let _ = data.sketch.add_constraint(ConstraintKind::Equal { a: r, b: line });
    let theta = ref_dir.dot(dir).clamp(-1.0, 1.0).acos();
    let bis = (ref_dir + dir).try_normalize().unwrap_or(dir.perp());
    // The typed text only matches the dimension when it is the acute angle itself.
    let expr = expr.filter(|_| (theta - ang).abs() < 1e-9);
    Some((ConstraintKind::Angle { a: r, b: line, value: theta }, Some(ps + bis * off * 1.6), expr))
}

/// Add a dimension: driving if possible, driven when it would over-constrain.
pub fn add_dimension(data: &mut SketchData, kind: ConstraintKind, label: Option<DVec2>, expr: Option<&str>) -> Option<ConstraintId> {
    // Added as a reference first so it takes the measured value, then made
    // driving unless that over-constrains the sketch (Onshape behavior).
    let cid = data.sketch.add_reference(kind).ok()?;
    let _ = data.sketch.set_driving(cid, true);
    let report = data.sketch.solve();
    if report.conflicting().contains(&cid) || report.redundant.contains(&cid) {
        let _ = data.sketch.set_driving(cid, false);
        data.sketch.solve();
    } else if let Some(e) = expr {
        data.dimension_exprs.insert(cid.0, ExprValue::new(e));
    }
    if let Some(p) = label {
        data.dimension_labels.insert(cid.0, [p.x, p.y]);
    }
    Some(cid)
}

fn de_casteljau(pts: &[DVec2], t: f64) -> DVec2 {
    let mut v = pts.to_vec();
    while v.len() > 1 {
        for i in 0..v.len() - 1 {
            v[i] = v[i].lerp(v[i + 1], t);
        }
        v.pop();
    }
    v[0]
}

fn arc_span(a0: f64, a1: f64, am: f64) -> (f64, f64) {
    let norm = |x: f64| x.rem_euclid(TAU);
    let s = a0;
    // Choose the direction that passes through am.
    let ccw_sweep = norm(a1 - s);
    let mid_off = norm(am - s);
    let e = if mid_off <= ccw_sweep { s + ccw_sweep } else { s - (TAU - ccw_sweep) };
    (s, e)
}

fn nearest_point(sk: &Sketch, p: DVec2, radius: f64) -> Option<EntityId> {
    sk.entities()
        .filter(|e| e.kind() == EntityKind::Point)
        .filter_map(|e| sk.point(e.id).map(|q| (e.id, (q - p).length())))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
}

fn dist_point_line(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let l = d.length();
    if l < 1e-15 {
        return (p - a).length();
    }
    (d.perp_dot(p - a) / l).abs()
}

/// Remove texts whose box was deleted, and dimension expressions, labels
/// and glyph offsets whose constraints are gone.
fn prune_exprs(data: &mut SketchData) {
    data.prune_texts();
    let live: std::collections::HashSet<u32> = data.sketch.constraints().map(|c| c.id.0).collect();
    data.dimension_exprs.retain(|k, _| live.contains(k));
    data.dimension_labels.retain(|k, _| live.contains(k));
    data.glyph_offsets.retain(|k, _| k.split(':').next().and_then(|c| c.parse::<u32>().ok()).is_some_and(|c| live.contains(&c)));
    data.intersections.retain_mut(|l| {
        l.entities.retain(|e| data.sketch.entity(EntityId(*e)).is_some());
        !l.entities.is_empty()
    });
}

fn curve_mid(sk: &Sketch, id: EntityId) -> Option<DVec2> {
    match sk.curve(id)? {
        Curve2::Line { a, b } => Some((a + b) * 0.5),
        Curve2::Circle { center, radius } => Some(center + DVec2::new(radius, 0.0)),
        Curve2::Arc { center, radius, start_angle, end_angle } => Some(center + DVec2::from_angle((start_angle + end_angle) * 0.5) * radius),
        c => {
            let (t0, t1) = c.domain();
            Some(c.point((t0 + t1) * 0.5))
        }
    }
}

/// Representative point of an entity for distances (point, center, midpoint).
fn rep_point(sk: &Sketch, id: EntityId) -> Option<DVec2> {
    if let Some(p) = sk.point(id) {
        return Some(p);
    }
    match sk.curve(id)? {
        Curve2::Line { a, b } => Some((a + b) * 0.5),
        Curve2::Circle { center, .. } | Curve2::Arc { center, .. } | Curve2::Ellipse { center, .. } | Curve2::EllipticalArc { center, .. } => Some(center),
        c => Some(c.bbox().center()),
    }
}

fn line_of(sk: &Sketch, id: EntityId) -> Option<(DVec2, DVec2)> {
    match sk.curve(id)? {
        Curve2::Line { a, b } => Some((a, b)),
        _ => None,
    }
}

fn foot(p: DVec2, a: DVec2, b: DVec2) -> DVec2 {
    let d = b - a;
    let l2 = d.length_squared();
    if l2 < 1e-30 {
        return a;
    }
    a + d * ((p - a).dot(d) / l2)
}

/// The two measured points of a linear distance between entities.
fn linear_pair(sk: &Sketch, a: EntityId, b: EntityId) -> Option<(DVec2, DVec2)> {
    match (line_of(sk, a), line_of(sk, b)) {
        (Some(la), Some(lb)) => {
            let m = (la.0 + la.1) * 0.5;
            Some((m, foot(m, lb.0, lb.1)))
        }
        (Some(la), None) => {
            let q = rep_point(sk, b)?;
            Some((foot(q, la.0, la.1), q))
        }
        (None, Some(lb)) => {
            let q = rep_point(sk, a)?;
            Some((q, foot(q, lb.0, lb.1)))
        }
        _ => Some((rep_point(sk, a)?, rep_point(sk, b)?)),
    }
}

fn dimension_anchor(sk: &Sketch, k: &ConstraintKind) -> Option<DVec2> {
    match k {
        ConstraintKind::Distance { a, b, .. } | ConstraintKind::HorizontalDistance { a, b, .. } | ConstraintKind::VerticalDistance { a, b, .. } => {
            let (p, q) = linear_pair(sk, *a, *b)?;
            Some((p + q) * 0.5)
        }
        ConstraintKind::Angle { a, b, .. } => Some((rep_point(sk, *a)? + rep_point(sk, *b)?) * 0.5),
        ConstraintKind::Length { curve, .. } | ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => curve_mid(sk, *curve),
        ConstraintKind::ArcAngle { arc, .. } => curve_mid(sk, *arc),
        ConstraintKind::EllipseRadius { ellipse, .. } => curve_mid(sk, *ellipse),
        _ => None,
    }
}

fn default_label_pos(sk: &Sketch, k: &ConstraintKind, wpp: f64) -> Option<DVec2> {
    let off = wpp * 30.0;
    match k {
        ConstraintKind::Length { curve, .. } => {
            if let Some((a, b)) = line_of(sk, *curve) {
                let n = (b - a).try_normalize().unwrap_or(DVec2::X).perp();
                return Some((a + b) * 0.5 + n * off);
            }
        }
        ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => {
            if let Some(Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. }) = sk.curve(*curve) {
                let dir = match sk.curve(*curve) {
                    Some(Curve2::Arc { start_angle, end_angle, .. }) => DVec2::from_angle((start_angle + end_angle) * 0.5),
                    _ => DVec2::new(1.0, 1.0).normalize(),
                };
                return Some(center + dir * (radius + off));
            }
        }
        ConstraintKind::Distance { a, b, .. } => {
            let (p, q) = linear_pair(sk, *a, *b)?;
            let n = (q - p).try_normalize().unwrap_or(DVec2::X).perp();
            return Some((p + q) * 0.5 + n * off);
        }
        ConstraintKind::HorizontalDistance { a, b, .. } => {
            let (p, q) = linear_pair(sk, *a, *b)?;
            return Some(DVec2::new((p.x + q.x) * 0.5, p.y.max(q.y) + off));
        }
        ConstraintKind::VerticalDistance { a, b, .. } => {
            let (p, q) = linear_pair(sk, *a, *b)?;
            return Some(DVec2::new(p.x.max(q.x) + off, (p.y + q.y) * 0.5));
        }
        _ => {}
    }
    let a = dimension_anchor(sk, k)?;
    Some(a + DVec2::new(0.0, off))
}

// ---------------------------------------------------------------- dimension graphics

fn arrow(out: &mut Vec<[DVec2; 2]>, tip: DVec2, dir: DVec2, s: f64) {
    // `dir` is the direction the arrow travels (pointing at `tip`).
    let d = dir.try_normalize().unwrap_or(DVec2::X);
    let n = d.perp();
    let (len, half) = (s * 9.0, s * 3.2);
    let base = tip - d * len;
    out.push([tip, base + n * half]);
    out.push([tip, base - n * half]);
    out.push([base + n * half, base - n * half]);
    out.push([tip, base]);
    out.push([tip - d * len * 0.15 + n * half * 0.5, base + n * half * 0.85]);
    out.push([tip - d * len * 0.15 - n * half * 0.5, base - n * half * 0.85]);
}

fn polyline(out: &mut Vec<[DVec2; 2]>, pts: &[DVec2]) {
    for w in pts.windows(2) {
        out.push([w[0], w[1]]);
    }
}

fn arc_points(c: DVec2, r: f64, a0: f64, sweep: f64) -> Vec<DVec2> {
    let n = ((sweep.abs() / TAU * 96.0).ceil() as usize).clamp(4, 96);
    (0..=n).map(|i| c + DVec2::from_angle(a0 + sweep * i as f64 / n as f64) * r).collect()
}

/// Linear dimension between `p1` and `p2` measured along `u` (None = aligned).
fn linear_dim(out: &mut Vec<[DVec2; 2]>, p1: DVec2, p2: DVec2, u: Option<DVec2>, pos: DVec2, s: f64) {
    let u = u.unwrap_or_else(|| (p2 - p1).try_normalize().unwrap_or(DVec2::X));
    let n = u.perp();
    let q1 = p1 + n * (pos - p1).dot(n);
    let q2 = p2 + n * (pos - p2).dot(n);
    let (gap, ext) = (s * 5.0, s * 5.0);
    for (p, q) in [(p1, q1), (p2, q2)] {
        let d = q - p;
        let l = d.length();
        if l > gap {
            let dn = d / l;
            out.push([p + dn * gap, q + dn * ext]);
        }
    }
    let span = (q2 - q1).dot(u);
    let (dir, len) = if span.abs() < 1e-15 { (u, 0.0) } else { (u * span.signum(), span.abs()) };
    let t = (pos - q1).dot(dir);
    let alen = s * 9.0;
    if len > 2.6 * alen {
        out.push([q1 + dir * t.min(0.0), q1 + dir * t.max(len)]);
        arrow(out, q1, -dir, s);
        arrow(out, q2, dir, s);
    } else {
        // Short dimension: arrows outside pointing in.
        out.push([q1 + dir * t.min(-2.0 * alen), q1 + dir * t.max(len + 2.0 * alen)]);
        arrow(out, q1, dir, s);
        arrow(out, q2, -dir, s);
    }
}

fn radial_dim(out: &mut Vec<[DVec2; 2]>, c: DVec2, r: f64, pos: DVec2, diameter: bool, span: Option<(f64, f64)>, s: f64) {
    let v = (pos - c).try_normalize().unwrap_or(DVec2::X);
    let dl = (pos - c).length();
    let tip = c + v * r;
    let far = if dl > r { pos } else { tip };
    if diameter {
        let t2 = c - v * r;
        out.push([t2, far]);
        arrow(out, tip, v, s);
        arrow(out, t2, -v, s);
    } else {
        out.push([c, far]);
        arrow(out, tip, v, s);
        // Extension arc when the leader lies outside the arc's span.
        if let Some((a0, a1)) = span {
            let a = v.to_angle();
            let sweep = a1 - a0;
            if noshape_sketch::geom::angle_in_range(a, a0, sweep, 1e-9).is_none() {
                let to_start = noshape_sketch::geom::ccw_sweep(a, a0);
                let from_end = noshape_sketch::geom::ccw_sweep(a1, a);
                let pts = if to_start < from_end { arc_points(c, r, a, to_start) } else { arc_points(c, r, a1, from_end) };
                polyline(out, &pts);
            }
        }
    }
}

/// Angular dimension arc at radius `r` around `c` from ray `s1` ccw to `s2`.
fn angle_arc(out: &mut Vec<[DVec2; 2]>, c: DVec2, s1: DVec2, s2: DVec2, r: f64, s: f64) {
    let a0 = s1.to_angle();
    let sweep = noshape_sketch::geom::ccw_sweep(a0, s2.to_angle());
    polyline(out, &arc_points(c, r, a0, sweep));
    let a1 = a0 + sweep;
    let t0 = DVec2::from_angle(a0).perp();
    let t1 = DVec2::from_angle(a1).perp();
    if r * sweep > s * 24.0 {
        arrow(out, c + DVec2::from_angle(a0) * r, -t0, s);
        arrow(out, c + DVec2::from_angle(a1) * r, t1, s);
    } else {
        arrow(out, c + DVec2::from_angle(a0) * r, t0, s);
        arrow(out, c + DVec2::from_angle(a1) * r, -t1, s);
    }
}

fn dim_segments(sk: &Sketch, k: &ConstraintKind, pos: DVec2, s: f64, out: &mut Vec<[DVec2; 2]>) {
    match k {
        ConstraintKind::Distance { a, b, .. } => {
            if let Some((p, q)) = linear_pair(sk, *a, *b) {
                // Parallel lines: measure perpendicular to them.
                let u = match (line_of(sk, *a), line_of(sk, *b)) {
                    (Some(l), _) | (_, Some(l)) => Some((l.1 - l.0).try_normalize().unwrap_or(DVec2::X).perp()),
                    _ => None,
                };
                let u = u.filter(|_| (q - p).length() > 1e-12).map(|u| if u.dot(q - p) < 0.0 { -u } else { u });
                linear_dim(out, p, q, u, pos, s);
            }
        }
        ConstraintKind::HorizontalDistance { a, b, .. } => {
            if let Some((p, q)) = linear_pair(sk, *a, *b) {
                linear_dim(out, p, q, Some(DVec2::X), pos, s);
            }
        }
        ConstraintKind::VerticalDistance { a, b, .. } => {
            if let Some((p, q)) = linear_pair(sk, *a, *b) {
                linear_dim(out, p, q, Some(DVec2::Y), pos, s);
            }
        }
        ConstraintKind::Length { curve, .. } => match sk.curve(*curve) {
            Some(Curve2::Line { a, b }) => linear_dim(out, a, b, None, pos, s),
            Some(Curve2::Arc { center, radius, start_angle, end_angle }) => arc_angle_dim(out, center, radius, start_angle, end_angle, pos, s),
            Some(c) => {
                let (t0, t1) = c.domain();
                out.push([c.point((t0 + t1) * 0.5), pos]);
            }
            None => {}
        },
        ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => {
            let dia = matches!(k, ConstraintKind::Diameter { .. });
            match sk.curve(*curve) {
                Some(Curve2::Circle { center, radius }) => radial_dim(out, center, radius, pos, dia, None, s),
                Some(Curve2::Arc { center, radius, start_angle, end_angle }) => radial_dim(out, center, radius, pos, dia, Some((start_angle, end_angle)), s),
                _ => {}
            }
        }
        ConstraintKind::ArcAngle { arc, .. } => {
            if let Some(Curve2::Arc { center, radius, start_angle, end_angle }) = sk.curve(*arc) {
                arc_angle_dim(out, center, radius, start_angle, end_angle, pos, s);
            }
        }
        ConstraintKind::Angle { a, b, .. } => {
            let (Some(la), Some(lb)) = (line_of(sk, *a), line_of(sk, *b)) else { return };
            let da = (la.1 - la.0).try_normalize().unwrap_or(DVec2::X);
            let db = (lb.1 - lb.0).try_normalize().unwrap_or(DVec2::Y);
            let den = da.perp_dot(db);
            if den.abs() < 1e-9 {
                out.push([(la.0 + la.1) * 0.5, pos]);
                return;
            }
            let t = (lb.0 - la.0).perp_dot(db) / den;
            let c = la.0 + da * t;
            let r = (pos - c).length().max(s * 10.0);
            let th = (pos - c).to_angle();
            // Rays sorted ccw; the sector containing the label is drawn.
            let mut rays = [da, db, -da, -db];
            rays.sort_by(|x, y| x.to_angle().rem_euclid(TAU).total_cmp(&y.to_angle().rem_euclid(TAU)));
            let mut chosen = (rays[0], rays[1]);
            for i in 0..4 {
                let (s1, s2) = (rays[i], rays[(i + 1) % 4]);
                let a0 = s1.to_angle();
                if noshape_sketch::geom::angle_in_range(th, a0, noshape_sketch::geom::ccw_sweep(a0, s2.to_angle()), 1e-12).is_some() {
                    chosen = (s1, s2);
                    break;
                }
            }
            angle_arc(out, c, chosen.0, chosen.1, r, s);
            // Extension lines where the lines don't reach the arc.
            for (ray, l) in [(chosen.0, if chosen.0.dot(da).abs() > 0.5 { la } else { lb }), (chosen.1, if chosen.1.dot(da).abs() > 0.5 { la } else { lb })] {
                let tmax = ((l.0 - c).dot(ray)).max((l.1 - c).dot(ray));
                if tmax < r - s * 2.0 {
                    out.push([c + ray * (tmax.max(0.0) + s * 4.0), c + ray * (r + s * 5.0)]);
                }
            }
        }
        ConstraintKind::EllipseRadius { ellipse, major, .. } => {
            let (center, major_v, minor) = match sk.curve(*ellipse) {
                Some(Curve2::Ellipse { center, major, minor }) | Some(Curve2::EllipticalArc { center, major, minor, .. }) => (center, major, minor),
                _ => return,
            };
            let axis = if *major { major_v } else { major_v.try_normalize().unwrap_or(DVec2::X).perp() * minor };
            let v = if (pos - center).dot(axis) < 0.0 { -axis } else { axis };
            let tip = center + v;
            out.push([center, tip]);
            arrow(out, tip, v, s);
            out.push([tip, pos]);
        }
        _ => {}
    }
}

fn arc_angle_dim(out: &mut Vec<[DVec2; 2]>, c: DVec2, r0: f64, a0: f64, a1: f64, pos: DVec2, s: f64) {
    let r = (pos - c).length().max(s * 10.0);
    angle_arc(out, c, DVec2::from_angle(a0), DVec2::from_angle(a1), r, s);
    if (r - r0).abs() > s * 4.0 {
        let sign = if r > r0 { 1.0 } else { -1.0 };
        for a in [a0, a1] {
            let d = DVec2::from_angle(a);
            out.push([c + d * (r0 + sign * s * 4.0), c + d * (r + sign * s * 5.0)]);
        }
    }
}

// ---------------------------------------------------------------- glyphs

fn glyph_entities(sk: &Sketch, k: &ConstraintKind) -> Vec<EntityId> {
    use ConstraintKind::*;
    match *k {
        Coincident { a, b } => {
            if is_point(sk, a) {
                vec![a]
            } else if is_point(sk, b) {
                vec![b]
            } else {
                vec![a, b]
            }
        }
        Horizontal { a, b } | Vertical { a, b } => match b {
            Some(b) => vec![a, b],
            None => vec![a],
        },
        Midpoint { point, .. } => vec![point],
        Pierce { point, .. } => vec![point],
        Symmetric { a, b, .. } => vec![a, b],
        Fix { entity } => vec![entity],
        Normal { line, curve } => vec![line, curve],
        Concentric { a, b } | Parallel { a, b } | Perpendicular { a, b } | Tangent { a, b } | Equal { a, b } | Collinear { a, b } | Curvature { a, b } => vec![a, b],
        _ => Vec::new(),
    }
}

fn glyph_anchor(sk: &Sketch, id: EntityId) -> Option<DVec2> {
    if let Some(p) = sk.point(id) {
        return Some(p);
    }
    match sk.curve(id)? {
        Curve2::Circle { center, radius } => Some(center + DVec2::from_angle(PI * 0.25) * radius),
        _ => curve_mid(sk, id),
    }
}

/// Onshape-style smart dimension from picked entities and cursor position.
fn smart_dimension(sk: &Sketch, ids: &[EntityId], cursor: DVec2) -> Option<ConstraintKind> {
    let kind = |id: EntityId| sk.kind(id);
    match ids {
        [a] => match kind(*a)? {
            EntityKind::Line => {
                let (pa, pb) = sk.endpoints(*a)?;
                let (qa, qb) = (sk.point(pa)?, sk.point(pb)?);
                Some(directed(pa, pb, qa, qb, cursor).unwrap_or(ConstraintKind::Length { curve: *a, value: (qb - qa).length() }))
            }
            EntityKind::Circle => Some(ConstraintKind::Diameter { curve: *a, value: 0.0 }),
            EntityKind::Arc => Some(ConstraintKind::Radius { curve: *a, value: 0.0 }),
            EntityKind::Ellipse | EntityKind::EllipticalArc => {
                // Major or minor radius depending on which axis the cursor is nearer.
                let major = match sk.curve(*a) {
                    Some(Curve2::Ellipse { center, major, .. }) | Some(Curve2::EllipticalArc { center, major, .. }) => {
                        let d = cursor - center;
                        d.dot(major.normalize()).abs() >= d.dot(major.normalize().perp()).abs()
                    }
                    _ => true,
                };
                Some(ConstraintKind::EllipseRadius { ellipse: *a, major, value: 0.0 })
            }
            _ => None,
        },
        [a, b] => {
            let (ka, kb) = (kind(*a)?, kind(*b)?);
            match (ka, kb) {
                (EntityKind::Point, EntityKind::Point) => {
                    let (qa, qb) = (sk.point(*a)?, sk.point(*b)?);
                    Some(directed(*a, *b, qa, qb, cursor).unwrap_or(ConstraintKind::Distance { a: *a, b: *b, value: 0.0, mode: DistanceMode::Center }))
                }
                (EntityKind::Line, EntityKind::Line) => {
                    let dir = |l: EntityId| -> Option<DVec2> {
                        let (pa, pb) = sk.endpoints(l)?;
                        Some((sk.point(pb)? - sk.point(pa)?).normalize())
                    };
                    let (da, db) = (dir(*a)?, dir(*b)?);
                    if da.perp_dot(db).abs() < 1e-6 {
                        Some(ConstraintKind::Distance { a: *a, b: *b, value: 0.0, mode: DistanceMode::Center })
                    } else {
                        Some(ConstraintKind::Angle { a: *a, b: *b, value: 0.0 })
                    }
                }
                _ => Some(ConstraintKind::Distance { a: *a, b: *b, value: 0.0, mode: DistanceMode::Center }),
            }
        }
        _ => None,
    }
}

/// Horizontal/vertical distance when the cursor is outside the band between two points.
fn directed(a: EntityId, b: EntityId, qa: DVec2, qb: DVec2, cursor: DVec2) -> Option<ConstraintKind> {
    let (lo, hi) = (qa.min(qb), qa.max(qb));
    let dx = (qb.x - qa.x).abs();
    let dy = (qb.y - qa.y).abs();
    if (cursor.y > hi.y || cursor.y < lo.y) && cursor.x >= lo.x && cursor.x <= hi.x && dx > 1e-9 {
        return Some(ConstraintKind::HorizontalDistance { a, b, value: dx });
    }
    if (cursor.x > hi.x || cursor.x < lo.x) && cursor.y >= lo.y && cursor.y <= hi.y && dy > 1e-9 {
        return Some(ConstraintKind::VerticalDistance { a, b, value: dy });
    }
    None
}

fn make_constraint(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> Option<ConstraintKind> {
    let pt = |i: usize| ids.get(i).copied();
    let a = pt(0)?;
    let b = pt(1);
    Some(match c {
        ConstraintTool::Horizontal => ConstraintKind::Horizontal { a, b },
        ConstraintTool::Vertical => ConstraintKind::Vertical { a, b },
        ConstraintTool::Fix => ConstraintKind::Fix { entity: a },
        ConstraintTool::Coincident => ConstraintKind::Coincident { a, b: b? },
        ConstraintTool::Concentric => ConstraintKind::Concentric { a, b: b? },
        ConstraintTool::Parallel => ConstraintKind::Parallel { a, b: b? },
        ConstraintTool::Perpendicular => ConstraintKind::Perpendicular { a, b: b? },
        ConstraintTool::Tangent => ConstraintKind::Tangent { a, b: b? },
        ConstraintTool::Equal => ConstraintKind::Equal { a, b: b? },
        ConstraintTool::Curvature => ConstraintKind::Curvature { a, b: b? },
        ConstraintTool::Midpoint => {
            let b = b?;
            if is_point(sk, a) { ConstraintKind::Midpoint { point: a, curve: b } } else { ConstraintKind::Midpoint { point: b, curve: a } }
        }
        ConstraintTool::Normal => ConstraintKind::Normal { line: a, curve: b? },
        ConstraintTool::Pierce => ConstraintKind::Pierce { point: a, target: b? },
        // Onshape order: the axis is picked first.
        ConstraintTool::Symmetric => {
            let (x, y, z) = (a, b?, pt(2)?);
            if sk.kind(x) == Some(EntityKind::Line) && sk.kind(z) != Some(EntityKind::Line) {
                ConstraintKind::Symmetric { a: y, b: z, axis: x }
            } else {
                ConstraintKind::Symmetric { a: x, b: y, axis: z }
            }
        }
    })
}

impl SketchEditor {
    /// Add intersection curves of a model face with the sketch plane; they
    /// are re-computed on every regeneration.
    pub fn add_intersection(&mut self, data: &mut SketchData, face: noshape_model::reference::Reference, curves: &[noshape_model::regen::features::sketch::SectionCurve]) -> bool {
        let ids = noshape_model::regen::features::sketch::add_section_curves(&mut data.sketch, curves);
        if ids.is_empty() {
            self.message = Some("That face does not cross the sketch plane".into());
            return false;
        }
        data.intersections.push(noshape_model::sketch_feature::IntersectionLink { face, entities: ids.iter().map(|e| e.0).collect() });
        data.sketch.solve();
        true
    }

    /// Edits the selected text box (content / font / style); the glyphs are
    /// regenerated from it.
    pub fn edit_text(&mut self, data: &mut SketchData, id: u32, f: impl FnOnce(&mut noshape_model::sketch_feature::SketchText)) -> bool {
        match data.update_text(id, f) {
            Ok(()) => {
                data.sketch.solve();
                true
            }
            Err(e) => {
                self.message = Some(e);
                false
            }
        }
    }

    /// Add a projected (used) curve linked to a 3D reference.
    pub fn add_used(&mut self, data: &mut SketchData, curve: &Curve2, reference: Option<noshape_model::reference::Reference>) -> bool {
        match data.sketch.use_external(curve) {
            Ok(r) => {
                if let (Some(&id), Some(reference)) = (r.created.last(), reference) {
                    data.external_refs.insert(id.0, reference);
                }
                data.sketch.solve();
                true
            }
            Err(e) => {
                self.message = Some(format!("{e}"));
                false
            }
        }
    }
}
