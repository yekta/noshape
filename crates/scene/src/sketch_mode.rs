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
    MidpointLine,
    CornerRect,
    CenterRect,
    AlignedRect,
    Circle,
    ThreePointCircle,
    Arc3,
    TangentArc,
    CenterArc,
    EllipticalArc,
    Point,
    Spline,
    Bezier,
    SplinePoint,
    Conic,
    Slot,
    Polygon,
    CircumPolygon,
    Ellipse,
    Text,
    Fillet,
    Chamfer,
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
    InsertDxf,
    InsertImage,
    Dimension,
    Constraint(ConstraintTool),
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Line => "Line (L)",
            Tool::MidpointLine => "Midpoint line",
            Tool::CornerRect => "Corner rectangle (G)",
            Tool::CenterRect => "Center rectangle (R)",
            Tool::AlignedRect => "Aligned rectangle",
            Tool::Circle => "Center point circle (C)",
            Tool::ThreePointCircle => "3 point circle",
            Tool::Arc3 => "3 point arc (A)",
            Tool::TangentArc => "Tangent arc",
            Tool::CenterArc => "Center point arc",
            Tool::EllipticalArc => "Elliptical arc",
            Tool::Point => "Point (Shift+S)",
            Tool::Spline => "Spline",
            Tool::Bezier => "Bezier",
            Tool::SplinePoint => "Spline control point",
            Tool::Conic => "Conic",
            Tool::Slot => "Slot",
            Tool::Polygon => "Inscribed polygon",
            Tool::CircumPolygon => "Circumscribed polygon",
            Tool::Ellipse => "Ellipse",
            Tool::Text => "Text",
            Tool::Fillet => "Sketch fillet (Shift+F)",
            Tool::Chamfer => "Sketch chamfer",
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
            Tool::InsertDxf => "Insert DXF or DWG",
            Tool::InsertImage => "Insert image",
            Tool::Dimension => "Dimension (D)",
            Tool::Constraint(c) => c.label(),
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => "select-box",
            Tool::Line => "line",
            Tool::MidpointLine => "midpoint-line",
            Tool::CornerRect => "corner-rectangle",
            Tool::CenterRect => "center-rectangle",
            Tool::AlignedRect => "aligned-rectangle",
            Tool::Circle => "circle",
            Tool::ThreePointCircle => "three-point-circle",
            Tool::Arc3 => "three-point-arc",
            Tool::TangentArc => "tangent-arc",
            Tool::CenterArc => "center-point-arc",
            Tool::EllipticalArc => "elliptical-arc",
            Tool::Point => "point-sketch",
            Tool::Spline => "spline",
            Tool::Bezier => "bezier",
            Tool::SplinePoint => "fit-spline",
            Tool::Conic => "conic",
            Tool::Slot => "slot",
            Tool::Polygon => "polygon-inscribed",
            Tool::CircumPolygon => "polygon-circumscribed",
            Tool::Ellipse => "ellipse",
            Tool::Text => "text",
            Tool::Fillet => "sketch-fillet",
            Tool::Chamfer => "sketch-chamfer",
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
            Tool::InsertDxf => "import",
            Tool::InsertImage => "image",
            Tool::Dimension => "dimension",
            Tool::Constraint(c) => c.icon(),
        }
    }
    fn inference_tool(self) -> SketchTool {
        match self {
            Tool::Select => SketchTool::Select,
            Tool::Point | Tool::Text => SketchTool::Point,
            Tool::Line | Tool::MidpointLine => SketchTool::Line,
            Tool::CornerRect | Tool::CenterRect | Tool::AlignedRect => SketchTool::Rectangle,
            Tool::Circle | Tool::ThreePointCircle | Tool::Polygon | Tool::CircumPolygon => SketchTool::Circle,
            Tool::Arc3 | Tool::CenterArc | Tool::EllipticalArc => SketchTool::Arc,
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
                | Tool::MidpointLine
                | Tool::CornerRect
                | Tool::CenterRect
                | Tool::AlignedRect
                | Tool::Circle
                | Tool::ThreePointCircle
                | Tool::Arc3
                | Tool::TangentArc
                | Tool::CenterArc
                | Tool::EllipticalArc
                | Tool::Point
                | Tool::Spline
                | Tool::Bezier
                | Tool::Conic
                | Tool::Polygon
                | Tool::CircumPolygon
                | Tool::Ellipse
                | Tool::Text
                | Tool::InsertImage
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
    Pending { entity: EntityId, start: DVec2 },
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
    /// Dragging (or clicking) the offset direction arrow.
    OffsetArrow { start: DVec2, moved: bool },
    /// Dragging the fillet radius manipulator (the session's first fillet).
    FilletRadius { arc: EntityId },
    /// Dragging the pending chamfer's distance arrow.
    ChamferDistance,
    /// Trim tool drag: every curve the path crosses is trimmed.
    TrimPath { points: Vec<DVec2> },
}

/// On-canvas manipulators of the pattern and transform tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternHandle {
    /// Linear pattern: first instance (direction and spacing).
    LinearStep,
    /// Linear pattern: last instance (instance count).
    LinearCount,
    LinearSecondStep,
    LinearSecondCount,
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
    /// The box of a dimension the Dimension tool just created (applying it
    /// clears the selection, Onshape `cleanUpAfterCommit`), not of an
    /// existing one being edited.
    pub created: bool,
}

/// One value box of an on-the-fly dimension (shown right after an entity
/// is created; typing a value and pressing Enter adds a driving dimension).
#[derive(Clone, Debug)]
pub struct OtfField {
    /// Dimension to add (its value is replaced by the typed one).
    pub kind: ConstraintKind,
    /// The box shows `value * scale` (ellipse axes are entered as diameters).
    pub scale: f64,
    /// Label position of the resulting dimension (sketch coordinates).
    pub label: DVec2,
}

/// On-the-fly dimension boxes of the last created geometry (Onshape:
/// "the suggested dimension value appears in a box"; Alt+arrows switch).
#[derive(Clone, Debug)]
pub struct OnTheFly {
    pub fields: Vec<OtfField>,
    pub active: usize,
}

/// Dimension graphics in sketch-plane coordinates.
#[derive(Clone, Debug)]
pub struct DimGraphic {
    pub constraint: ConstraintId,
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
    /// The constraint refers to external (used / projected) geometry.
    pub external: bool,
}

/// Glyph ids of "Use" links (Onshape's use glyph on projected geometry):
/// not solver constraints but the entity's link in `SketchData::external_refs`.
const USE_GLYPH_BASE: u32 = 0xC000_0000;

/// The glyph id of an entity's Use link.
pub fn use_glyph_id(e: EntityId) -> ConstraintId {
    ConstraintId(USE_GLYPH_BASE | e.0)
}

/// The used entity of a Use-link glyph id.
pub fn use_glyph_entity(c: ConstraintId) -> Option<EntityId> {
    (c.0 & USE_GLYPH_BASE == USE_GLYPH_BASE).then_some(EntityId(c.0 & !USE_GLYPH_BASE))
}

/// Deleting a Use glyph unlinks the geometry: it stays as regular, free
/// sketch geometry no longer following the model (Onshape).
fn unlink_used(data: &mut SketchData, e: EntityId) -> bool {
    if data.external_refs.remove(&e.0).is_none() {
        return false;
    }
    let mut ids = vec![e];
    if let Some((a, b)) = data.sketch.endpoints(e) {
        ids.extend([a, b]);
    }
    if let Some(c) = data.sketch.entity(e).and_then(|x| x.center()) {
        ids.push(c);
    }
    for id in ids {
        let _ = data.sketch.set_external(id, false);
    }
    true
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
    Edge { position: DVec2, reference: noshape_model::reference::Reference },
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
    /// Use tool: projection preview of the hovered model edges.
    pub use_preview: Vec<Vec<DVec2>>,
    /// Hovered sketch region (key) when no entity is under the cursor.
    pub hover_region: Option<Vec<u32>>,
    /// Selected sketch regions (clicking inside a closed profile).
    pub selected_regions: Vec<Vec<u32>>,
    /// Hovered dimension label or constraint glyph.
    pub hover_constraint: Option<ConstraintId>,
    /// Entity whose glyphs stay visible briefly after hover ends.
    pub glyph_entity: Option<(EntityId, std::time::Instant)>,
    drag: Option<DragState>,
    box_end: Option<DVec2>,
    /// Ctrl held during the box selection: it removes from the selection.
    pub box_remove: bool,
    /// Point snapped to while dragging a point (coincident on release).
    drag_snap: Option<(EntityId, DVec2, bool)>,
    /// Entities picked so far for a constraint/dimension tool.
    picks: Vec<(EntityId, DVec2)>,
    /// Picks of the dimension being placed (click positions decide min/max).
    picks_placed: Vec<(EntityId, DVec2)>,
    pub dim_edit: Option<DimEdit>,
    /// The value box to open after `dim_edit` is accepted (a chamfer's
    /// second distance; live: Enter goes from distance 1 to distance 2).
    value_box_next: Option<ConstraintId>,
    pub construction: bool,
    /// Show every constraint glyph (toolbar toggle).
    pub show_constraints: bool,
    /// Show dimension expressions.
    pub show_expressions: bool,
    /// Show errors (over-defined constraints in red; on by default).
    pub show_errors: bool,
    /// The saved options this sketch opened with (see [`SketchOptions`]).
    opened_options: SketchOptions,
    /// Shift held while moving: show all glyphs temporarily (Onshape).
    pub shift_held: bool,
    pub polygon_sides: usize,
    pub fillet_radius: f64,
    pub offset_distance: f64,
    pub offset_flip: bool,
    /// Curves of the offset being set up (preview, arrow and distance box).
    pub offset_pending: Option<Vec<EntityId>>,
    /// An offset was made: later curve clicks repeat it with the same distance.
    offset_repeat: bool,
    pub conic_rho: f64,
    pub text_string: String,
    pub text_height: f64,
    pub text_font: String,
    pub text_bold: bool,
    pub text_italic: bool,
    pub text_flip_horizontal: bool,
    pub text_flip_vertical: bool,
    /// The text rectangle is provisional until the Text dialog is confirmed.
    pub pending_text: Option<(DVec2, f64)>,
    /// Existing text reopened in the Text dialog (double-click, live Onshape).
    pub editing_text: Option<u32>,
    /// The Text dialog's "Convert to expression" state.
    pub text_expression: bool,
    pub pattern_count: usize,
    pub pattern_distance: f64,
    pub pattern_angle: f64,
    pub pattern_second_count: usize,
    /// A count arrow drag: the count and the arrow's position along its direction at the grab.
    pattern_grab: Option<(usize, f64)>,
    pub pattern_second_distance: f64,
    pub pattern_second_angle: f64,
    pub circular_count: usize,
    pub circular_angle: f64,
    pub pattern_center: DVec2,
    /// Transform: the triad was dragged away from the selection's center.
    xf_center_pinned: bool,
    pub xf_offset: DVec2,
    pub xf_angle: f64,
    pub xf_scale: f64,
    /// First fillet arc of the current fillet session (later fillets are equal to it).
    fillet_session: Option<EntityId>,
    /// Fillet/chamfer preview is isolated from the document until Enter.
    pending_corner: Option<(SketchData, noshape_sketch::EditResult)>,
    /// Slot preview (live Onshape: light blue until the next click, Enter or
    /// leaving the tool commits it): the previewed data and the new curves.
    pending_slot: Option<(SketchData, Vec<EntityId>)>,
    pub chamfer_distance: f64,
    pub slot_width: f64,
    /// End cap of the first slot of the session (later slots are equal to it).
    slot_session: Option<EntityId>,
    /// Pattern being edited (Edit pattern): replaced on commit.
    pub editing_pattern: Option<u32>,
    /// Insert image: the chosen file (name, bytes, width / height) waiting for its rectangle.
    pub pending_image: Option<(String, Vec<u8>, f64)>,
    /// Insert DXF: meters per drawing unit (`None`: the file's units, else millimeters).
    pub dxf_unit_scale: Option<f64>,
    /// Insert DXF: keep the file origin at the sketch origin (else center the drawing).
    pub dxf_use_origin: bool,
    /// Mirror line chosen in tool-first mode.
    mirror_axis: Option<EntityId>,
    /// On-the-fly dimension boxes of the geometry created last.
    pub otf: Option<OnTheFly>,
    /// The user started typing into the active on-the-fly box.
    pub otf_typing: bool,
    /// Curves the cursor passed over while drawing (wake-up inferences).
    pub woken: Vec<EntityId>,
    /// Press position of a drawing tool (press-drag-release placement).
    press: Option<DVec2>,
    /// Line tool temporarily drawing a tangent arc (Shift+A or hovering
    /// back over the chain end); reverts to lines after one arc.
    pub tangent_mode: bool,
    /// The cursor left the chain end since the last segment (re-entering
    /// it toggles the tangent arc mode).
    left_chain_end: bool,
    /// Alt held while drawing a rectangle: square.
    pub alt_held: bool,
    /// Polygon side count typed while placing the circumference.
    pub polygon_typed: String,
    /// Side count when the polygon's circumference was locked (drag adjusts from it).
    polygon_base_sides: usize,
    /// Signed angle the cursor swept around a center point arc's center
    /// (the arc follows the cursor's direction).
    center_arc_sweep: f64,
    /// Label/glyph pressed in the GPUI overlay (consumed by the next tool_down).
    pub pressed_label: Option<(ConstraintId, bool, bool)>,
    pub pressed_glyph: Option<(ConstraintId, EntityId, bool)>,
    /// Constraints the solver reported as conflicting (set by the view).
    pub conflicting: Vec<ConstraintId>,
    /// World units per pixel at the sketch (for snap radii).
    pub world_per_px: f64,
    /// Screen to sketch map set by the app on every view change: the sketch
    /// point under the screen origin and the sketch steps of one pixel right
    /// and down. Manipulators sit on pixel centres with it (Onshape's crisp
    /// 2D overlays, live sketch/303).
    pub screen_map: Option<(DVec2, DVec2, DVec2)>,
    /// The sketch frame (lower-left, upper-right in sketch coordinates),
    /// fixed in the plane once first drawn (see [`Self::place_frame`]).
    pub frame_bounds: Option<(DVec2, DVec2)>,
    pub message: Option<String>,
    /// Profile inspector: the focused issue (position and curves), drawn red.
    pub issue_highlight: Option<(DVec2, Vec<EntityId>)>,
    /// Profile inspector: positions of all open issues.
    pub issue_markers: Vec<DVec2>,
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

/// The Sketch dialog's options, kept as user settings (Onshape
/// `UserSettingsService`: `sketchShowConstraints || false`,
/// `sketchShowExpressions || false`, `sketchShowErrors !== false`; saved when
/// a sketch is accepted with options that differ from those it opened with).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SketchOptions {
    pub show_constraints: bool,
    pub show_expressions: bool,
    pub show_errors: bool,
}

impl Default for SketchOptions {
    fn default() -> Self {
        // Live (sketch/311, a fresh profile: qa4 F1): Show constraints and
        // Show errors ticked.
        Self { show_constraints: true, show_expressions: false, show_errors: true }
    }
}

static SKETCH_OPTIONS: std::sync::Mutex<Option<SketchOptions>> = std::sync::Mutex::new(None);

fn sketch_options_path() -> std::path::PathBuf {
    crate::config_dir().join("sketch-options.json")
}

/// The saved sketch options (read once).
pub fn sketch_options() -> SketchOptions {
    let mut saved = SKETCH_OPTIONS.lock().unwrap_or_else(|e| e.into_inner());
    *saved.get_or_insert_with(|| if cfg!(test) { SketchOptions::default() } else { std::fs::read(sketch_options_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default() })
}

fn save_sketch_options(o: SketchOptions) {
    *SKETCH_OPTIONS.lock().unwrap_or_else(|e| e.into_inner()) = Some(o);
    if !cfg!(test)
        && let Ok(b) = serde_json::to_vec_pretty(&o)
    {
        let _ = std::fs::write(sketch_options_path(), b);
    }
}

/// Inference guide lines and snap markers (`INFERENCE_SNAP_LINE_COLOR`).
const INFERENCE_COLOR: [u8; 4] = [0xff, 0xc2, 0x0e, 255];
/// Geometry being drawn (`SKETCH_PREVIEW_COLOR`).
const PREVIEW_COLOR: [u8; 4] = [0x44, 0x9c, 0xcd, 255];
/// Onshape's hollow arrow manipulator in a sketch (live sketch/303, offset):
/// 11 px past the curve's copy, an 18 px stem, a 31 x 23 px head; outline
/// #989fa3 (3 px) around a white core.
const OFFSET_ARROW_GAP_PX: f64 = 11.0;
const OFFSET_ARROW_STEM_PX: f64 = 18.0;
const OFFSET_ARROW_HEAD_PX: f64 = 31.0;
const OFFSET_ARROW_HEAD_W_PX: f64 = 23.0;
const MANIPULATOR_OUTLINE: [u8; 4] = [0x98, 0x9f, 0xa3, 255];
/// The fillet radius manipulator's stem (live sketch/1934: the arrow starts
/// at the preview arc's midpoint and points at its centre).
const FILLET_ARROW_STEM_PX: f64 = 28.0;
/// Linear pattern manipulators (live sketch/136): a 16 px hollow square at
/// the first instance, the count arrow's 30 px stem from its edge; labels
/// ~95 px off diagonally; the spacing dimension 101 px to the side.
const PATTERN_SQUARE_PX: f64 = 16.0;
const PATTERN_ARROW_STEM_PX: f64 = 30.0;
const PATTERN_ARROW_HANDLE_PX: f64 = PATTERN_SQUARE_PX * 0.5 + PATTERN_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX * 0.5;
const PATTERN_LABEL_PX: f64 = 95.0;
const PATTERN_DIM_PX: f64 = 101.0;
/// Polygon side spinner (live sketch/91): 24 px out from the circumference
/// point, its ◀ / ▶ 15 px either side of the count along the ray.
const POLYGON_SPINNER_PX: f64 = 24.0;
pub const POLYGON_SPINNER_ARROW_PX: f64 = 15.0;

/// Onshape's hollow drag arrow (offset, fillet and chamfer manipulators):
/// `start`, an `stem_px` stem, then the 31 x 23 px head, in `dir`.
fn push_hollow_arrow(wide: &mut Vec<(Vec<DVec2>, [u8; 4], f32)>, start: DVec2, dir: DVec2, px: f64, stem_px: f64) {
    let base = start + dir * px * stem_px;
    let apex = base + dir * px * OFFSET_ARROW_HEAD_PX;
    let half = dir.perp() * px * OFFSET_ARROW_HEAD_W_PX * 0.5;
    // A #989fa3 3 px outline under a white core (as the Part Studio manipulators).
    for (color, width) in [(MANIPULATOR_OUTLINE, 3.0), ([0xff, 0xff, 0xff, 0xff], 1.2)] {
        wide.push((vec![start, base], color, width));
        wide.push((vec![base + half, apex, base - half, base + half], color, width));
    }
}

/// A hollow square manipulator (live sketch/136), `side` px, centred on `q`.
fn push_hollow_square(wide: &mut Vec<(Vec<DVec2>, [u8; 4], f32)>, q: DVec2, px: f64, side: f64) {
    let h = px * side * 0.5;
    let pts = vec![q + DVec2::new(-h, -h), q + DVec2::new(h, -h), q + DVec2::new(h, h), q + DVec2::new(-h, h), q + DVec2::new(-h, -h)];
    for (color, width) in [(MANIPULATOR_OUTLINE, 3.0), ([0xff, 0xff, 0xff, 0xff], 1.2)] {
        wide.push((pts.clone(), color, width));
    }
}

/// Text drawn as filled glyphs in the sketch overlay, so it can turn with
/// a manipulator (gpui text can't rotate): Noto Sans outlines `em_px` high,
/// centred on `at` with the baseline along `dir`, filled with scanlines.
fn hatched_text(text: &str, at: DVec2, dir: DVec2, px: f64, em_px: f64) -> Vec<[DVec2; 2]> {
    let layout = noshape_sketch::text::layout_text(text, "Noto Sans", false, false);
    // Box units: ascender = 1 (Noto's ascender is 1.069 em).
    let unit = px * em_px * 1.069;
    let polys = layout.polylines(8);
    let (Some(y0), Some(y1)) = (polys.iter().flatten().map(|p| p.y).min_by(f64::total_cmp), polys.iter().flatten().map(|p| p.y).max_by(f64::total_cmp)) else { return Vec::new() };
    let centre = DVec2::new(layout.width * 0.5, (y0 + y1) * 0.5);
    let (ux, uy) = (dir, dir.perp());
    let map = |q: DVec2| at + ux * (q.x - centre.x) * unit + uy * (q.y - centre.y) * unit;
    let mut out = Vec::new();
    // Even-odd scanlines every 0.45 px (drawn 1 px wide: solid).
    let step = 0.45 * px / unit;
    let mut y = y0 + step * 0.5;
    while y < y1 {
        let mut xs: Vec<f64> = Vec::new();
        for poly in &polys {
            for w in poly.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for [x0, x1] in xs.as_chunks::<2>().0 {
            out.push([map(DVec2::new(*x0, y)), map(DVec2::new(*x1, y))]);
        }
        y += step;
    }
    out
}

/// Entities a hovered glyph shows in the secondary yellow (live
/// behaviours "sketch constraint-glyph hover colours", sketch/2203–2217;
/// everything else it constrains turns pre-highlight orange, the symmetry or
/// mirror line included): a point-pair symmetric's owning lines (the Mirror
/// tool's pairs) and a slot offset's centre line and its end (sketch/1956).
fn constraint_masters(kind: &ConstraintKind, sk: &Sketch) -> Vec<EntityId> {
    match kind {
        ConstraintKind::Offset { master_ref, master, .. } => vec![*master_ref, *master],
        ConstraintKind::Symmetric { a, b, .. } if is_point(sk, *a) && is_point(sk, *b) => sk
            .entities()
            .filter(|e| e.kind() == EntityKind::Line && e.endpoints().is_some_and(|(s, en)| [s, en].iter().any(|p| p == a || p == b)))
            .map(|e| e.id)
            .collect(),
        _ => Vec::new(),
    }
}

/// Distance from `p` to the segment `a`–`b`.
fn dist_to_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-30)).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// Unit kind of a dimension's value.
pub fn dim_unit(k: &ConstraintKind) -> Dim {
    match k {
        ConstraintKind::Rho { .. } => Dim::UNITLESS,
        _ if is_angle_kind(k) => Dim::ANGLE,
        _ => Dim::LENGTH,
    }
}

/// Displayed text of a dimension value (Ø for diameters, R for radii).
pub fn dim_value_text(k: &ConstraintKind, v: f64, units: &UnitPrefs) -> String {
    dim_value_text_tol(k, v, units, None)
}

/// A number in display units with a fixed number of decimals (no unit).
fn fixed_number(v: f64, dim: Dim, units: &UnitPrefs, decimals: u8) -> String {
    // Value in display units: format with many decimals and parse the number back.
    let exact = noshape_units::format_quantity_with(Quantity::new(v, dim), units, 12);
    let num: String = exact.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    let x: f64 = num.parse().unwrap_or(0.0);
    format!("{x:.prec$}", prec = decimals as usize)
}

/// Unit suffix of a dimension kind as formatted (e.g. " mm", "°").
fn unit_suffix_of(dim: Dim, units: &UnitPrefs) -> String {
    let t = noshape_units::format_quantity_with(Quantity::new(0.0, dim), units, 0);
    t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-').to_string()
}

/// Displayed text of a dimension with its tolerance and precision.
pub fn dim_value_text_tol(k: &ConstraintKind, v: f64, units: &UnitPrefs, tol: Option<&noshape_model::sketch_feature::DimTolerance>) -> String {
    use noshape_model::sketch_feature::ToleranceKind as T;
    let dim = dim_unit(k);
    let scale = dim_display_scale(k);
    let prefix = match k {
        ConstraintKind::Diameter { .. } | ConstraintKind::EllipseRadius { .. } | ConstraintKind::Distance { mode: DistanceMode::Centerline, .. } => "Ø",
        ConstraintKind::Radius { .. } => "R",
        ConstraintKind::Rho { .. } => "Rho ",
        _ => "",
    };
    // Onshape shows lengths without the document unit ("4", "Ø1", "R2.5");
    // angles keep their degree sign.
    let suffix = unit_suffix_of(dim, units);
    let shown_suffix = if dim == Dim::LENGTH { String::new() } else { suffix.clone() };
    let prec = tol.and_then(|t| t.precision);
    let fmt = |x: f64| match prec {
        Some(n) => format!("{}{shown_suffix}", fixed_number(x, dim, units, n)),
        None => {
            let t = noshape_units::format_quantity(Quantity::new(x, dim), units);
            if dim == Dim::LENGTH { t.strip_suffix(suffix.as_str()).map(str::to_string).unwrap_or(t) } else { t }
        }
    };
    let dev = |x: f64| fixed_number(x, dim, units, prec.unwrap_or(if dim == Dim::ANGLE { units.angle_precision } else { units.length_precision }));
    let base = format!("{prefix}{}", fmt(v * scale));
    let Some(t) = tol else { return base };
    match t.kind {
        T::None => base,
        T::Symmetrical => format!("{base} ±{}", dev(t.upper)),
        T::Deviation => format!("{base} +{}/-{}", dev(t.upper), dev(t.lower)),
        T::Limits => format!("{prefix}{} / {}", fmt(v * scale + t.upper), fmt(v * scale - t.lower)),
        T::Min => format!("{base} MIN"),
        T::Max => format!("{base} MAX"),
        T::Basic => format!("[{base}]"),
    }
}

/// Tolerances are not allowed on single line lengths and arc lengths (Onshape).
pub fn tolerance_allowed(k: &ConstraintKind) -> bool {
    !matches!(k, ConstraintKind::Length { .. })
}

/// A dimension text that is more than a plain number with an optional unit.
/// [`SketchEditor::apply_dimension`]'s error for a value the sketch can't
/// be solved with (Onshape's warning toast text).
pub const SKETCH_NOT_SOLVED: &str = "Sketch could not be solved.";

/// What Enter does in sketch mode ([`SketchEditor::enter_target`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterTarget {
    /// The text tool's dialog.
    Text,
    Slot,
    /// A sketch fillet / chamfer preview (then its value box opens).
    Corner,
    PolygonSides,
    /// A pattern / transform being set up.
    Pattern,
    Offset,
    /// The on-the-fly value box becomes a driving dimension; the tool goes on.
    OnTheFly,
    /// Nothing: the sketch stays open.
    Nothing,
}

/// Label prefixes of a dimension driven by a variable (`#var`: Onshape's
/// grey "ƒx") or by another expression ("ƒ"); live sketch/992.
pub const VARIABLE_MARK: &str = "ƒx ";
pub const EXPRESSION_MARK: &str = "ƒ ";

/// A dimension label from its formatted value and the text typed for it:
/// a variable's or expression's marker, or the expression itself with Show
/// expressions; numbers with a unit ("1 mm", "1 in") show just the value.
pub fn dimension_label_text(expr: Option<&str>, value: String, show_expressions: bool) -> String {
    match expr {
        Some(e) if is_expression(e) && show_expressions => format!("{} = {value}", truncate_chars(e, 70)),
        Some(e) if is_variable(e) => format!("{VARIABLE_MARK}{value}"),
        Some(e) if is_expression(e) => format!("{EXPRESSION_MARK}{value}"),
        _ => value,
    }
}

/// The text is a single variable reference (`#var`).
fn is_variable(t: &str) -> bool {
    t.trim().strip_prefix('#').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

fn is_expression(t: &str) -> bool {
    let t = t.trim();
    let num_end = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(t.len());
    let rest = t[num_end..].trim();
    num_end == 0 || !(rest.is_empty() || rest.chars().all(|c| c.is_ascii_alphabetic()))
}

fn truncate_chars(t: &str, n: usize) -> String {
    if t.chars().count() <= n { t.to_string() } else { format!("{}…", t.chars().take(n).collect::<String>()) }
}

/// Factor between a dimension's stored value and the value shown / typed:
/// ellipse axes are shown as diameters (Onshape's major / minor diameter).
pub fn dim_display_scale(k: &ConstraintKind) -> f64 {
    k.display_scale()
}

/// Value used by in-place editing, in the same units and scale as its label.
pub fn dimension_edit_text(data: &SketchData, cid: ConstraintId, units: &UnitPrefs) -> Option<String> {
    // A typed plain number reopens with the document unit ("80" → "80 mm",
    // QA3; live sketch/13: the box shows the value with its unit).
    if let Some(expr) = data.dimension_exprs.get(&cid.0).filter(|e| e.text.trim().parse::<f64>().is_err()) {
        return Some(expr.text.clone());
    }
    let kind = &data.sketch.constraint(cid)?.kind;
    let value = kind.value()? * dim_display_scale(kind);
    let dim = dim_unit(kind);
    // Live sketch/13, 141, 143: lengths with 6 decimals ("3.51041 in",
    // "1.725582 in"), angles with 7 ("35.5376823 deg"), zeros trimmed.
    let precision = if dim == Dim::ANGLE { units.angle_precision.max(7) } else { units.length_precision.max(6) };
    Some(noshape_units::format_quantity_with(Quantity::new(value, dim), units, precision))
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
        Quadrant { .. } => "coincident",
        // Live (qa5 sketch tools): the slot's three offsets, an arch glyph.
        Offset { .. } => "offset",
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
        let opened_options = sketch_options();
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
            use_preview: Vec::new(),
            hover_region: None,
            selected_regions: Vec::new(),
            hover_constraint: None,
            glyph_entity: None,
            drag: None,
            box_end: None,
            box_remove: false,
            drag_snap: None,
            picks: Vec::new(),
            picks_placed: Vec::new(),
            dim_edit: None,
            value_box_next: None,
            construction: false,
            show_constraints: opened_options.show_constraints,
            show_expressions: opened_options.show_expressions,
            show_errors: opened_options.show_errors,
            opened_options,
            shift_held: false,
            polygon_sides: 6,
            fillet_radius: 0.005,
            offset_distance: 0.005,
            offset_flip: false,
            offset_pending: None,
            offset_repeat: false,
            conic_rho: 0.5,
            text_string: "Default text".into(),
            text_height: 0.01,
            text_font: noshape_sketch::text::SKETCH_DEFAULT_FONT.into(),
            text_bold: false,
            text_italic: false,
            text_flip_horizontal: false,
            text_flip_vertical: false,
            pending_text: None,
            editing_text: None,
            text_expression: false,
            pattern_count: 3,
            pattern_distance: 0.02,
            pattern_angle: 0.0,
            pattern_second_count: 1,
            pattern_grab: None,
            pattern_second_distance: 0.02,
            pattern_second_angle: PI * 0.5,
            circular_count: 3,
            circular_angle: TAU,
            pattern_center: DVec2::ZERO,
            xf_center_pinned: false,
            xf_offset: DVec2::ZERO,
            xf_angle: 0.0,
            xf_scale: 1.0,
            fillet_session: None,
            pending_corner: None,
            pending_slot: None,
            chamfer_distance: 0.005,
            slot_width: 0.005,
            slot_session: None,
            mirror_axis: None,
            pending_image: None,
            dxf_unit_scale: None,
            dxf_use_origin: false,
            editing_pattern: None,
            otf: None,
            otf_typing: false,
            woken: Vec::new(),
            press: None,
            tangent_mode: false,
            left_chain_end: false,
            alt_held: false,
            polygon_typed: String::new(),
            polygon_base_sides: 6,
            center_arc_sweep: 0.0,
            pressed_label: None,
            pressed_glyph: None,
            conflicting: Vec::new(),
            world_per_px: 1e-4,
            screen_map: None,
            frame_bounds: None,
            message: None,
            issue_highlight: None,
            issue_markers: Vec::new(),
            ui: None,
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        // Pressing the active tool's key again toggles back to select.
        self.tool = if self.tool == tool && tool != Tool::Select { Tool::Select } else { tool };
        // Choosing a drawing tool clears the selection (live: L with a line
        // selected deselects it).
        if self.tool.draws() {
            self.selection.clear();
            self.selected_constraints.clear();
        }
        self.clicks.clear();
        self.chain_end = None;
        self.picks.clear();
        self.inference = None;
        self.external_snap = None;
        self.fillet_session = None;
        self.pending_corner = None;
        self.pending_slot = None;
        self.editing_text = None;
        self.text_expression = false;
        self.xf_center_pinned = false;
        self.pending_text = None;
        self.slot_session = None;
        self.offset_pending = None;
        self.offset_repeat = false;
        self.mirror_axis = None;
        self.otf = None;
        self.otf_typing = false;
        self.woken.clear();
        self.press = None;
        self.tangent_mode = false;
        self.message = match self.tool {
            Tool::Fillet => Some("Select a point or two sketch curves to fillet".into()),
            Tool::Chamfer => Some("Select a vertex or two edges to chamfer".into()),
            Tool::Trim => Some("Click entities to trim away, or drag across them".into()),
            Tool::Extend => Some("Click the line or arc to extend".into()),
            Tool::Split => Some("Click the sketch curve to split".into()),
            Tool::SplinePoint => Some("Click a spline or bezier control polygon to add a control point".into()),
            Tool::Offset => Some("Select one or more curves to offset".into()),
            Tool::Mirror => Some("Select a mirror line.".into()),
            Tool::Dimension => Some("Select the entities to dimension, then click to place the dimension".into()),
            Tool::Use => Some("Click edges, faces or sketch entities to use (project) them".into()),
            Tool::Intersection => Some("Select a surface or face crossing the sketch plane".into()),
            Tool::InsertDxf => Some("Choose the units, then a DXF file".into()),
            Tool::InsertImage => Some("Choose an image file, then draw the image rectangle".into()),
            Tool::Text => Some("Drag a box: its lower-left corner and height set the first line of text".into()),
            Tool::Conic => Some("Click the start point, the end point, then the control point".into()),
            Tool::Spline => Some("Click to add points; double-click to end".into()),
            Tool::Bezier => Some("Click to add control points; double-click to end".into()),
            Tool::MidpointLine => Some("Click the midpoint, then an end point".into()),
            Tool::EllipticalArc => Some("Click the center, the primary axis, the start point, then the end point".into()),
            Tool::Polygon | Tool::CircumPolygon => Some("Click the center, then the circumference (type a number first to set the sides)".into()),
            Tool::LinearPattern => Some("Select entities and set the pattern; click in empty space or press Enter to accept".into()),
            Tool::CircularPattern => Some("Select entities and set the center; click in empty space or press Enter to accept".into()),
            Tool::Transform => Some("Select entities and set the transform; click in empty space or press Enter to accept".into()),
            Tool::Constraint(c) => Some(format!("Select {} entit{} for {}", c.arity(), if c.arity() == 1 { "y" } else { "ies" }, c.label())),
            _ => None,
        };
    }

    /// Tools that act on a preselection as soon as they are chosen (Slot,
    /// Offset; Onshape "select curves, then click the tool").
    pub fn apply_preselection(&mut self, data: &mut SketchData) -> EditOutcome {
        self.follow_transform_selection(&data.sketch);
        if self.selection.is_empty() {
            return EditOutcome::none();
        }
        match self.tool {
            Tool::Slot => {
                let ids = std::mem::take(&mut self.selection);
                self.make_slots(data, &ids)
            }
            Tool::Offset => {
                let ids: Vec<EntityId> = std::mem::take(&mut self.selection).into_iter().filter(|e| !is_point(&data.sketch, *e)).collect();
                if !ids.is_empty() {
                    self.offset_pending = Some(ids);
                    self.message = Some("Set the offset distance (drag or click the arrow), then press Enter".into());
                }
                EditOutcome::redraw()
            }
            _ => EditOutcome::none(),
        }
    }

    /// Cancel the in-progress operation (Esc). Returns true if something was cancelled.
    pub fn cancel(&mut self) -> bool {
        self.value_box_next = None;
        if let Some(d) = self.dim_edit.take() {
            // Onshape resets the selection when the Dimension tool commits a
            // new dimension, so dismissing its box leaves nothing selected.
            if d.created {
                self.selection.clear();
                self.selected_constraints.clear();
            }
            return true;
        }
        if self.editing_text.take().is_some() {
            return true;
        }
        // A box being typed into closes first; an idle box goes away with
        // the chain / tool.
        if self.otf_typing && self.otf.take().is_some() {
            self.otf_typing = false;
            return true;
        }
        // Live Onshape: one Escape ends the Line chain and exits Line.
        // A double-click ends only the chain and leaves the tool active.
        if self.tool == Tool::Line {
            self.set_tool(Tool::Select);
            return true;
        }
        let had_box = self.otf.take().is_some();
        if self.offset_pending.take().is_some() {
            self.set_tool(Tool::Select);
            return true;
        }
        // Esc cancels a pattern / transform being set up.
        if self.tool.is_pattern() && (!self.selection.is_empty() || self.editing_pattern.is_some()) {
            self.selection.clear();
            self.editing_pattern = None;
            self.xf_offset = DVec2::ZERO;
            self.xf_angle = 0.0;
            self.xf_scale = 1.0;
            self.set_tool(Tool::Select);
            return true;
        }
        if !self.clicks.is_empty() || !self.picks.is_empty() || self.chain_end.is_some() || self.mirror_axis.is_some() || self.tangent_mode {
            self.set_tool(Tool::Select);
            return true;
        }
        if self.tool != Tool::Select {
            self.set_tool(Tool::Select);
            return true;
        }
        // Onshape keeps a sketch selection on Escape; Space or an empty
        // canvas click clears it.
        had_box
    }

    /// Cursor position on the sketch plane from a world ray.
    pub fn plane_point(&self, origin: DVec3, dir: DVec3) -> Option<DVec2> {
        self.frame.intersect_ray(origin, dir).map(|p| self.frame.to_local(p))
    }

    fn infer(&self, sk: &Sketch, p: DVec2, suppress: bool) -> Option<Inference> {
        if suppress {
            return None;
        }
        // While a line chain is drawing a tangent arc there is no anchor
        // direction to infer from (the arc is tangent by construction).
        let anchor = if self.tangent_mode { None } else { self.anchor_pos(sk) };
        let ctx = InferenceContext {
            tool: if self.tangent_mode { SketchTool::TangentArc } else { self.tool.inference_tool() },
            anchor,
            anchor_point: self.chain_end,
            snap_radius: self.world_per_px * 9.0,
            angle_tolerance: 3f64.to_radians(),
            exclude: Vec::new(),
            origin: true,
            woken: self.woken.clone(),
        };
        let mut best = noshape_sketch::infer(sk, p, &ctx).into_iter().next();
        // Elliptical arc: the end snaps to the construction ellipse's axis points.
        if self.tool == Tool::EllipticalArc
            && self.clicks.len() == 3
            && best.as_ref().is_none_or(|i| i.priority > 1)
            && let Some((c, major, minor)) = construction_ellipse(self.clicks[0].pos, self.clicks[1].pos, self.clicks[2].pos)
        {
            let n = major.normalize().perp() * minor;
            let r = self.world_per_px * 9.0;
            if let Some(q) = [c + major, c - major, c + n, c - n].into_iter().filter(|q| (*q - p).length() <= r).min_by(|a, b| (*a - p).length().total_cmp(&(*b - p).length())) {
                best = Some(Inference { kind: InferenceKind::Quadrant, position: q, entity: None, reference: Some(c), constraints: Vec::new(), distance: (q - p).length(), priority: 1 });
            }
        }
        // 3 point arc: snap to an exact semicircle when the center reaches the chord.
        if self.tool == Tool::Arc3
            && self.clicks.len() == 2
            && best.as_ref().is_none_or(|i| i.priority > 1)
            && let Some(s) = semicircle_snap(self.clicks[0].pos, self.clicks[1].pos, p, self.world_per_px * 9.0)
        {
            best = Some(Inference { kind: InferenceKind::Midpoint, position: s, entity: None, reference: Some((self.clicks[0].pos + self.clicks[1].pos) * 0.5), constraints: Vec::new(), distance: (s - p).length(), priority: 1 });
        }
        best
    }

    /// Current anchor of the tool (last click; a chained line starts at the
    /// solved position of the chain end, which a typed dimension may move).
    fn anchor_pos(&self, sk: &Sketch) -> Option<DVec2> {
        if let Some(e) = self.chain_end
            && let Some(p) = sk.point(e)
        {
            return Some(p);
        }
        self.clicks.last().map(|c| c.pos)
    }

    pub fn pointer_move(&mut self, data: &mut SketchData, p: DVec2, shift: bool, alt: bool) -> EditOutcome {
        // Polygon side count: distance from the locked circumference, one side per 15 px.
        if matches!(self.tool, Tool::Polygon | Tool::CircumPolygon) && self.clicks.len() == 2 && self.polygon_typed.is_empty() {
            let (ctr, rim) = (self.clicks[0].pos, self.clicks[1].pos);
            let delta = ((p - ctr).length() - (rim - ctr).length()) / (self.world_per_px * 15.0);
            let n = (self.polygon_base_sides as f64 + delta.round()).clamp(3.0, 50.0) as usize;
            if n != self.polygon_sides {
                self.polygon_sides = n;
                self.message = Some(format!("Sides: {n}"));
            }
        }
        if self.tool == Tool::CenterArc && self.clicks.len() == 2 {
            let c = self.clicks[0].pos;
            let prev = self.cursor.unwrap_or(self.clicks[1].pos);
            let delta = ((p - c).to_angle() - (prev - c).to_angle() + PI).rem_euclid(TAU) - PI;
            self.center_arc_sweep += delta;
        } else {
            self.center_arc_sweep = 0.0;
        }
        self.cursor = Some(p);
        self.shift_held = shift;
        self.alt_held = alt;
        self.external_snap = None;
        if self.tool.draws() {
            // Curves the cursor passes over wake up their inferences.
            if let Some(h) = self.hover
                && !is_point(&data.sketch, h)
            {
                self.woken.retain(|e| *e != h);
                self.woken.insert(0, h);
                self.woken.truncate(10);
            }
            // Line chain: moving back onto the chain end switches to a tangent arc.
            if self.tool == Tool::Line
                && let Some(end) = self.chain_end.and_then(|e| data.sketch.point(e))
            {
                let d = (p - end).length();
                let r = self.world_per_px * 9.0;
                if d > r * 2.5 {
                    self.left_chain_end = true;
                } else if d < r && self.left_chain_end {
                    self.left_chain_end = false;
                    self.tangent_mode = !self.tangent_mode;
                }
            }
            self.inference = self.infer(&data.sketch, p, shift);
            return EditOutcome::redraw();
        }
        self.inference = None;
        EditOutcome::redraw()
    }

    /// Modifier changes also affect a stationary cursor: holding Shift must
    /// immediately suppress both drawing inferences and a pending drop snap.
    pub fn modifiers_changed(&mut self, data: &SketchData, shift: bool, alt: bool) {
        self.shift_held = shift;
        self.alt_held = alt;
        if shift {
            self.external_snap = None;
            self.drag_snap = None;
        } else if let Some(DragState::Geometry { handle: DragHandle::Point(id) }) = self.drag {
            self.drag_snap = self.point_drop_target(&data.sketch, id);
        }
        if let Some(p) = self.cursor && self.tool.draws() {
            self.inference = self.infer(&data.sketch, p, shift);
        }
    }

    /// Show constraints toggle.
    pub fn set_show_constraints(&mut self, on: bool) {
        self.show_constraints = on;
    }

    /// Accepting the sketch saves its dialog options when they changed
    /// (Onshape `commitFeature`: `sketchOptionsWereChanged` → `saveSketchOptions`).
    pub fn save_options_if_changed(&self) {
        let now = SketchOptions { show_constraints: self.show_constraints, show_expressions: self.show_expressions, show_errors: self.show_errors };
        if now != self.opened_options {
            save_sketch_options(now);
        }
    }

    /// Polygon waiting for its side count (circumference locked).
    pub fn polygon_sides_pending(&self) -> bool {
        matches!(self.tool, Tool::Polygon | Tool::CircumPolygon) && self.clicks.len() == 2
    }

    /// The side-count spinner "◀ 6 ▶" (live sketch/91): 24 px outside the
    /// middle of the edge after the clicked vertex, laid along that edge's
    /// outward normal (◀ inward, ▶ outward; measured: the ◀ / ▶ axis is
    /// perpendicular to the edge); (centre, outward direction, sides).
    pub fn polygon_side_control(&self) -> Option<(DVec2, DVec2, usize)> {
        if !self.polygon_sides_pending() {
            return None;
        }
        let (c, rim) = (self.clicks[0].pos, self.clicks[1].pos);
        let n = self.polygon_sides.clamp(3, 50) as f64;
        let r = (rim - c).length();
        let start = (rim - c).to_angle();
        // Inscribed: the vertex is on the click; circumscribed: the edge midpoint.
        let (rv, off) = if self.tool == Tool::Polygon { (r, 0.0) } else { (r / (PI / n).cos(), PI / n) };
        let v0 = c + DVec2::from_angle(start + off) * rv;
        let v1 = c + DVec2::from_angle(start + off + TAU / n) * rv;
        let mid = (v0 + v1) * 0.5;
        let out = (mid - c).try_normalize().unwrap_or(DVec2::Y);
        Some((mid + out * self.world_per_px * POLYGON_SPINNER_PX, out, self.polygon_sides))
    }

    /// The on-canvas polygon spinner locks an explicitly chosen side count.
    pub fn adjust_polygon_sides(&mut self, delta: isize) {
        self.polygon_sides = (self.polygon_sides as isize + delta).clamp(3, 50) as usize;
        self.polygon_typed = self.polygon_sides.to_string();
    }

    /// Set the pending polygon's side count (Enter / click).
    pub fn finish_polygon(&mut self, data: &mut SketchData) -> EditOutcome {
        if !self.polygon_sides_pending() {
            return EditOutcome::none();
        }
        let pos = self.clicks[1].pos;
        self.place(data, Click { pos, inference: None }, false)
    }

    /// Mirror tool: the mirror line is chosen.
    pub fn has_mirror_axis(&self) -> bool {
        self.mirror_axis.is_some()
    }

    /// No click of the current tool placed yet.
    pub fn clicks_empty(&self) -> bool {
        self.clicks.is_empty()
    }

    /// Default tool sizes follow the document's length unit. Live Onshape,
    /// inch workspace: fillet R0.5, chamfer 0.25, offset 0.25, slot Ø0.5,
    /// linear pattern spacing 1 in (sketch/107–137); millimetre workspace:
    /// R5 (behaviours.md 568: on every fresh load, whatever the size or
    /// zoom; then the last radius used in the tab), 5, 5, Ø20, 25 mm
    /// (sketch/300–312).
    pub fn apply_unit_defaults(&mut self, length: noshape_units::LengthUnit) {
        use noshape_units::LengthUnit as L;
        const IN: f64 = 0.0254;
        let imperial = matches!(length, L::Inch | L::Foot | L::Yard);
        let (fillet, quarter, slot, spacing) = if imperial { (0.5 * IN, 0.25 * IN, 0.5 * IN, IN) } else { (0.005, 0.005, 0.02, 0.025) };
        self.fillet_radius = fillet;
        self.chamfer_distance = quarter;
        self.offset_distance = quarter;
        self.slot_width = slot;
        self.pattern_distance = spacing;
        self.pattern_second_distance = spacing;
    }

    /// Toggle between Line and Tangent arc while chaining (Shift+A).
    pub fn toggle_tangent_arc(&mut self) {
        match self.tool {
            Tool::Line if self.chain_end.is_some() => {
                self.tangent_mode = !self.tangent_mode;
                self.left_chain_end = false;
            }
            Tool::TangentArc => self.set_tool(Tool::Line),
            _ => self.set_tool(Tool::TangentArc),
        }
    }

    /// Apply a snap to model geometry when it beats the sketch inference.
    pub fn apply_external_snap(&mut self, snap: ExternalSnap) {
        if !self.tool.draws() {
            return;
        }
        let strong = self.inference.as_ref().is_some_and(|i| matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Midpoint | InferenceKind::Center));
        if strong {
            return;
        }
        let (position, kind) = match &snap {
            ExternalSnap::Vertex { position, .. } => (*position, InferenceKind::Endpoint),
            ExternalSnap::Edge { position, .. } => (*position, InferenceKind::OnCurve),
        };
        if self.inference.as_ref().is_some_and(|i| kind == InferenceKind::OnCurve && i.kind != InferenceKind::OnCurve) {
            return;
        }
        let distance = self.cursor.map(|c| (c - position).length()).unwrap_or(0.0);
        self.inference = Some(Inference { kind, position, entity: None, reference: None, constraints: Vec::new(), distance, priority: 0 });
        self.external_snap = Some(snap);
    }

    fn snapped(&self) -> Option<(DVec2, Option<Inference>)> {
        let c = self.cursor?;
        let (mut pos, inf) = match &self.inference {
            Some(i) => (i.position, Some(i.clone())),
            None => (c, None),
        };
        // Alt while drawing a rectangle: square.
        if self.alt_held
            && matches!(self.tool, Tool::CornerRect | Tool::CenterRect)
            && let Some(a) = self.clicks.first()
        {
            pos = square_corner(a.pos, pos);
        }
        Some((pos, inf))
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
        data.sketch.solve();
    }

    /// Constrain the next click onto an external entity (a snapped model edge).
    pub fn link_edge_snap(&mut self, id: EntityId) {
        if let Some(inf) = &mut self.inference {
            inf.kind = InferenceKind::OnCurve;
            inf.constraints = vec![AutoConstraint::OnCurve(id)];
        }
        self.external_snap = None;
    }

    /// Link an external snap (model vertex) to a fixed sketch point before a
    /// click is placed, so the new geometry becomes coincident with it.
    pub fn take_vertex_snap(&mut self, data: &mut SketchData) {
        // The sketch origin becomes a fixed point the new geometry is constrained to.
        if let Some(inf) = &mut self.inference
            && inf.constraints.iter().any(|c| auto_target(c) == Some(noshape_sketch::ORIGIN))
        {
            let o = origin_point(&mut data.sketch);
            for c in &mut inf.constraints {
                *c = retarget_auto(*c, noshape_sketch::ORIGIN, o);
            }
            if inf.entity == Some(noshape_sketch::ORIGIN) {
                inf.entity = Some(o);
            }
        }
        let Some(ExternalSnap::Vertex { position, reference }) = self.external_snap.clone() else { return };
        // (Edge snaps are linked by the view, which can project the edge.)
        let id = data.sketch.use_external_point(position);
        data.external_refs.insert(id.0, reference);
        if let Some(inf) = &mut self.inference {
            inf.kind = InferenceKind::Endpoint;
            inf.constraints = vec![AutoConstraint::CoincidentPoint(id)];
        }
        self.external_snap = None;
    }

    pub fn tool_down(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>, shift: bool, double_click: bool) -> EditOutcome {
        if self.text_dialog_open() {
            return EditOutcome::none();
        }
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
            self.pattern_grab = self.selection_center(&data.sketch).and_then(|c| match h {
                PatternHandle::LinearCount => Some((self.pattern_count, (p - c).dot(DVec2::from_angle(self.pattern_angle)))),
                PatternHandle::LinearSecondCount => Some((self.pattern_second_count, (p - c).dot(DVec2::from_angle(self.pattern_second_angle)))),
                _ => None,
            });
            self.drag = Some(DragState::Handle(h));
            return EditOutcome::redraw();
        }
        match self.tool {
            Tool::Select => {
                // Double-clicking a text reopens it in the Text dialog.
                if double_click
                    && let Some(id) = hovered.and_then(|e| data.text_of_entity(e))
                {
                    self.drag = None;
                    return if self.begin_text_edit(data, id) { EditOutcome::redraw() } else { EditOutcome::none() };
                }
                match hovered {
                    // Text: glyphs and box lines move the whole box (corners resize it).
                    Some(e) if !is_point(&data.sketch, e) && (data.text_of_entity(e).is_some() || data.image_of_entity(e).is_some()) => {
                        let box_points: Vec<u32> = match data.text_of_entity(e) {
                            Some(tid) => data.texts.iter().find(|t| t.id == tid).map(|t| t.box_points.clone()).unwrap_or_default(),
                            None => data.image_of_entity(e).and_then(|i| data.images.iter().find(|x| x.id == i)).map(|x| x.box_points.clone()).unwrap_or_default(),
                        };
                        let points: Vec<(EntityId, DVec2)> = box_points.iter().filter_map(|q| data.sketch.point(EntityId(*q)).map(|pos| (EntityId(*q), pos))).collect();
                        self.drag = Some(DragState::Text { entity: e, start: p, points, moved: false });
                    }
                    Some(e) => {
                        self.drag = Some(DragState::Pending { entity: e, start: p });
                    }
                    None => {
                        self.drag = Some(DragState::Box { start: p });
                        self.box_end = Some(p);
                        self.box_remove = false;
                    }
                }
                EditOutcome::none()
            }
            t if t.draws() => {
                if self.inference.is_none() {
                    self.inference = self.infer(&data.sketch, p, self.shift_held || shift);
                }
                self.take_vertex_snap(data);
                let (pos, inference) = self.snapped().unwrap_or((p, None));
                // Live Onshape: pressing on a Line chain's end and dragging
                // neither makes an arc nor places a point on release; the
                // rubber band just keeps following the cursor.
                let on_chain_end = self.tool == Tool::Line
                    && self.chain_end.and_then(|e| data.sketch.point(e)).is_some_and(|e| (e - pos).length() < 1e-9);
                if on_chain_end && !double_click {
                    self.press = None;
                    return EditOutcome::redraw();
                }
                // Continuing to sketch leaves the last geometry undimensioned.
                self.otf = None;
                self.otf_typing = false;
                let out = self.place(data, Click { pos, inference }, double_click);
                // Press-drag-release: the release places the next click.
                let pending = !self.clicks.is_empty() || (self.tool == Tool::Line && self.chain_end.is_some());
                self.press = (pending && !on_chain_end && !matches!(self.tool, Tool::Spline | Tool::Bezier | Tool::Point)).then_some(p);
                out
            }
            Tool::Fillet if self.fillet_arrow(&data.sketch).is_some_and(|(a, b)| dist_to_segment(p, a, b) <= self.world_per_px * 10.0) => {
                if let Some((arc, _)) = self.fillet_handle(&data.sketch) {
                    self.drag = Some(DragState::FilletRadius { arc });
                }
                EditOutcome::redraw()
            }
            Tool::Chamfer if self.chamfer_arrow().is_some_and(|(a, b)| dist_to_segment(p, a, b) <= self.world_per_px * 10.0) => {
                self.drag = Some(DragState::ChamferDistance);
                EditOutcome::redraw()
            }
            Tool::Fillet | Tool::Chamfer => self.corner_click(data, p, hovered),
            Tool::Trim => {
                self.drag = Some(DragState::TrimPath { points: vec![p] });
                self.trim_at(data, p, hovered)
            }
            Tool::Extend | Tool::Split => {
                let Some(id) = hovered.filter(|e| !is_point(&data.sketch, *e)).or_else(|| data.sketch.pick(p, self.world_per_px * 8.0).filter(|e| !is_point(&data.sketch, *e))) else {
                    return EditOutcome::none();
                };
                let r = match self.tool {
                    Tool::Extend if double_click => data.sketch.extend(id, p),
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
                    Err(_) if self.tool == Tool::Extend => {
                        self.message = Some("Unable to extend the selected geometry.".into());
                        EditOutcome::redraw()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::SplinePoint => {
                let Some(id) = hovered.filter(|e| matches!(data.sketch.kind(*e), Some(EntityKind::FitSpline | EntityKind::BSpline))) else {
                    return EditOutcome::none();
                };
                match data.sketch.insert_spline_point(id, p) {
                    Ok(_) => {
                        data.sketch.solve();
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Slot => {
                let ids: Vec<EntityId> = if self.selection.is_empty() { hovered.into_iter().collect() } else { std::mem::take(&mut self.selection) };
                self.make_slots(data, &ids)
            }
            Tool::Offset => {
                // The direction arrow: drag to set the distance, click to flip.
                if let Some((base, tip)) = self.offset_arrow(&data.sketch)
                    && (p - tip).length().min((p - (base + tip) * 0.5).length()) <= self.world_per_px * 9.0
                {
                    self.drag = Some(DragState::OffsetArrow { start: p, moved: false });
                    return EditOutcome::redraw();
                }
                let ids: Vec<EntityId> = if self.selection.is_empty() { hovered.into_iter().filter(|e| !is_point(&data.sketch, *e)).collect() } else { std::mem::take(&mut self.selection) };
                if ids.is_empty() {
                    // Clicking empty space accepts a pending offset.
                    return if self.offset_pending.is_some() { self.commit_offset(data) } else { EditOutcome::none() };
                }
                if self.offset_repeat && self.offset_pending.is_none() {
                    // Later clicks offset by the same distance right away.
                    self.offset_pending = Some(ids);
                    return self.commit_offset(data);
                }
                match &mut self.offset_pending {
                    // Add curves to the pending offset (chain selection).
                    Some(p) => {
                        for e in ids {
                            if !p.contains(&e) {
                                p.push(e);
                            }
                        }
                    }
                    None => self.offset_pending = Some(ids),
                }
                self.message = Some("Set the offset distance (drag or click the arrow), then press Enter".into());
                EditOutcome::redraw()
            }
            Tool::Mirror => {
                let is_line = |e: &EntityId| data.sketch.kind(*e) == Some(EntityKind::Line);
                match self.mirror_axis.filter(|a| data.sketch.entity(*a).is_some()) {
                    None => {
                        let Some(axis) = hovered.filter(is_line) else {
                            self.message = Some("Select a mirror line.".into());
                            return EditOutcome::none();
                        };
                        self.mirror_axis = Some(axis);
                        let ids: Vec<EntityId> = self.selection.iter().copied().filter(|e| *e != axis).collect();
                        if ids.is_empty() {
                            self.message = Some("Select entities to be mirrored.".into());
                            return EditOutcome::redraw();
                        }
                        self.selection.clear();
                        let out = self.mirror_entities(data, &ids);
                        // Pre-selection: the mirror resolves at once and the tool ends.
                        self.set_tool(Tool::Select);
                        out
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
                    // Clicking white space accepts the pattern / transform (Onshape).
                    None if !self.selection.is_empty() => return self.apply_pattern(data),
                    None => {}
                }
                EditOutcome::redraw()
            }
            Tool::Dimension => self.dimension_click(data, p, hovered),
            Tool::Constraint(c) => {
                let Some(e) = hovered else { return EditOutcome::none() };
                if !self.picks.iter().any(|(x, _)| *x == e) {
                    self.picks.push((e, p));
                }
                // One-entity constraints (horizontal, vertical, fix) apply to each
                // pick; two points make a horizontal/vertical pair.
                let need = if matches!(c, ConstraintTool::Horizontal | ConstraintTool::Vertical) && self.picks.len() == 1 && is_point(&data.sketch, e) { 2 } else { c.arity() };
                if self.picks.len() >= need {
                    let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
                    self.picks.clear();
                    if already_coincident(&data.sketch, c, &ids) {
                        return EditOutcome::redraw();
                    }
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

    /// Fillet / chamfer tool click: a corner point, or the first / second curve.
    fn corner_click(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>) -> EditOutcome {
        if self.pending_corner.is_some() {
            return self.commit_corner(data);
        }
        let chamfer = self.tool == Tool::Chamfer;
        let corner = hovered.filter(|e| is_point(&data.sketch, *e)).or_else(|| if hovered.is_none() { nearest_point(&data.sketch, p, self.world_per_px * 10.0) } else { None });
        let mut preview = data.clone();
        let sk = &mut preview.sketch;
        let r = if let Some(pid) = corner {
            self.picks.clear();
            if chamfer { sk.chamfer_corner(pid, self.chamfer_distance, self.chamfer_distance) } else { sk.fillet_corner(pid, self.fillet_radius) }
        } else if let Some(c) = hovered {
            if let Some(&(first, _)) = self.picks.first().filter(|(f, _)| *f != c) {
                self.picks.clear();
                if chamfer { sk.chamfer(first, c, self.chamfer_distance, self.chamfer_distance) } else { sk.fillet(first, c, self.fillet_radius) }
            } else {
                self.picks = vec![(c, p)];
                return EditOutcome::redraw();
            }
        } else {
            return EditOutcome::none();
        };
        match r {
            Ok(r) => {
                self.pending_corner = Some((preview, r));
                self.message = Some("Adjust the preview, then press Enter to confirm".into());
                EditOutcome::redraw()
            }
            Err(e) => {
                self.message = Some(if chamfer { format!("Cannot chamfer selected point. {e}") } else { format!("Cannot fillet selected point. {e}") });
                EditOutcome::redraw()
            }
        }
    }

    pub fn corner_pending(&self) -> bool {
        self.pending_corner.is_some()
    }

    pub fn commit_corner(&mut self, data: &mut SketchData) -> EditOutcome {
        let Some((preview, result)) = self.pending_corner.take() else { return EditOutcome::none() };
        *data = preview;
        self.finish_create(data, &result.created);
        if self.tool == Tool::Chamfer {
            self.after_chamfer(data, &result);
        } else {
            self.after_fillet(data, &result);
        }
        self.message = None;
        data.sketch.solve();
        EditOutcome::changed()
    }

    /// First fillet of a session gets a radius value box; later ones are equal
    /// to it, so editing the first dimension changes them all (Onshape).
    fn after_fillet(&mut self, data: &mut SketchData, r: &noshape_sketch::EditResult) {
        let Some(arc) = r.created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc)) else { return };
        let rad_dim = r.constraints_added.iter().copied().find(|c| data.sketch.constraint(*c).is_some_and(|c| matches!(c.kind, ConstraintKind::Radius { .. })));
        match self.fillet_session {
            Some(first) if data.sketch.entity(first).is_some() => {
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
                    self.open_value_box(data, d);
                }
                self.fillet_session = Some(arc);
            }
        }
    }

    fn after_chamfer(&mut self, data: &mut SketchData, r: &noshape_sketch::EditResult) {
        let dims = chamfer_dims(&data.sketch, r);
        for (cid, at) in chamfer_dim_labels(&data.sketch, &dims, self.world_per_px) {
            data.dimension_labels.insert(cid.0, [at.x, at.y]);
        }
        if let Some(d) = dims.first() {
            self.open_value_box(data, *d);
        }
        self.value_box_next = dims.get(1).copied();
    }

    /// After a value box was accepted: opens the queued one (a chamfer's
    /// second distance). Returns whether a box opened.
    pub fn open_next_value_box(&mut self, data: &SketchData) -> bool {
        let Some(cid) = self.value_box_next.take() else { return false };
        if data.sketch.constraint(cid).is_none() {
            return false;
        }
        self.open_value_box(data, cid);
        self.dim_edit.is_some()
    }

    /// The pending chamfer: its virtual sharp, the unit direction from it
    /// along the first distance's edge, that distance's chamfer end and the
    /// two distance dimensions.
    fn chamfer_frame(&self) -> Option<(DVec2, DVec2, DVec2, Vec<ConstraintId>)> {
        if self.tool != Tool::Chamfer {
            return None;
        }
        let (data, result) = self.pending_corner.as_ref()?;
        let dims = chamfer_dims(&data.sketch, result);
        let Some(ConstraintKind::Distance { a, b, .. }) = data.sketch.constraint(*dims.first()?).map(|c| c.kind.clone()) else { return None };
        let (sharp, end) = (data.sketch.point(a)?, data.sketch.point(b)?);
        Some((sharp, (end - sharp).try_normalize()?, end, dims))
    }

    /// The pending chamfer's distance manipulator (live sketch/1960b): the
    /// hollow drag arrow from the chamfer's end on the first edge, pointing
    /// along it away from the corner; (start, apex of the head).
    pub fn chamfer_arrow(&self) -> Option<(DVec2, DVec2)> {
        let (_, u, end, _) = self.chamfer_frame()?;
        Some((end, end + u * self.world_per_px * (FILLET_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX)))
    }

    /// The pending chamfer's preview distance labels (live sketch/1960:
    /// "5", no unit), with their positions.
    pub fn chamfer_value_labels(&self) -> Vec<(DVec2, f64)> {
        let Some((data, result)) = self.pending_corner.as_ref().filter(|_| self.tool == Tool::Chamfer) else { return Vec::new() };
        let dims = chamfer_dims(&data.sketch, result);
        chamfer_dim_labels(&data.sketch, &dims, self.world_per_px)
            .into_iter()
            .filter_map(|(cid, at)| Some((at, data.sketch.constraint(cid)?.kind.value()?)))
            .collect()
    }

    /// Offset curves of the pending offset with the current distance (preview / commit).
    fn offset_result(&self, sk: &Sketch) -> Option<(Sketch, Vec<EntityId>)> {
        let ids = self.offset_pending.as_ref()?;
        let d = if self.offset_flip { -self.offset_distance } else { self.offset_distance };
        let mut tmp = sk.clone();
        let r = tmp.offset(ids, d).ok()?;
        let created = r.created.into_iter().filter(|e| tmp.kind(*e).is_some_and(|k| k != EntityKind::Point)).collect();
        Some((tmp, created))
    }

    /// Midpoint of the first pending curve and its left normal.
    fn offset_frame(&self, sk: &Sketch) -> Option<(DVec2, DVec2)> {
        let first = *self.offset_pending.as_ref()?.first()?;
        let c = sk.curve(first)?;
        let (a, b) = c.domain();
        let t = (a + b) * 0.5;
        Some((c.point(t), c.tangent(t).try_normalize()?.perp()))
    }

    /// Whether a positive offset of the first pending curve goes against its
    /// left normal (closed curves offset inward).
    fn offset_flip_base(&self, sk: &Sketch) -> bool {
        let Some(first) = self.offset_pending.as_ref().and_then(|p| p.first().copied()) else { return false };
        let Some((mid, n)) = self.offset_frame(sk) else { return false };
        let mut tmp = sk.clone();
        let Ok(r) = tmp.offset(&[first], self.world_per_px.max(1e-9)) else { return false };
        let probe = r.created.iter().filter_map(|e| tmp.curve(*e)).map(|c| c.closest(mid).1).min_by(|x, y| (*x - mid).length().total_cmp(&(*y - mid).length()));
        probe.is_some_and(|q| (q - mid).dot(n) < 0.0)
    }

    /// `p` moved onto the nearest pixel centre (unchanged without a screen map).
    pub fn snap_to_pixel(&self, p: DVec2) -> DVec2 {
        let Some((o, ex, ey)) = self.screen_map else { return p };
        let m = glam::DMat2::from_cols(ex, ey);
        if m.determinant().abs() < 1e-30 {
            return p;
        }
        let s = m.inverse() * (p - o);
        o + m * DVec2::new(s.x.floor() + 0.5, s.y.floor() + 0.5)
    }

    /// The offset manipulator (live sketch/303): Onshape's hollow arrow,
    /// starting 11 px past the offset copy and pointing away from the curve;
    /// (start of the 18 px stem, apex of the 31 px head).
    pub fn offset_arrow(&self, sk: &Sketch) -> Option<(DVec2, DVec2)> {
        let (mid, n) = self.offset_frame(sk)?;
        let sign = if self.offset_flip != self.offset_flip_base(sk) { -1.0 } else { 1.0 };
        let (dir, px) = (n * sign, self.world_per_px);
        let start = mid + dir * (self.offset_distance + px * OFFSET_ARROW_GAP_PX);
        Some((start, start + dir * px * (OFFSET_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX)))
    }

    /// The pending offset's distance label, between the curve and its copy;
    /// clicking it opens the distance box (Onshape's offset tool opens its
    /// in-place edit view from the preview dimension; the manipulator has
    /// none). Shown once it fits: live sketch/123 shows "0.25" at a ~40 px
    /// gap, sketch/303 (5 mm, ~19 px) none.
    pub fn offset_value_label(&self, sk: &Sketch) -> Option<(DVec2, f64)> {
        if self.tool != Tool::Offset || self.offset_distance < self.world_per_px * 30.0 {
            return None;
        }
        let (mid, n) = self.offset_frame(sk)?;
        let sign = if self.offset_flip != self.offset_flip_base(sk) { -1.0 } else { 1.0 };
        Some((mid + n * sign * self.offset_distance * 0.5, self.offset_distance))
    }

    /// Create the pending offset (Enter / click in space); later clicks repeat it.
    pub fn commit_offset(&mut self, data: &mut SketchData) -> EditOutcome {
        let Some(ids) = self.offset_pending.take() else { return EditOutcome::none() };
        let d = if self.offset_flip { -self.offset_distance } else { self.offset_distance };
        match data.sketch.offset(&ids, d) {
            Ok(r) => {
                self.offset_repeat = true;
                self.finish_create(data, &r.created);
                self.message = Some("Click more curves to offset them by the same distance".into());
                EditOutcome::changed()
            }
            Err(e) => {
                self.message = Some(format!("Offset failed: {e}"));
                EditOutcome::redraw()
            }
        }
    }

    /// The fillet radius manipulator: the session's first fillet arc and its midpoint.
    fn fillet_handle(&self, sk: &Sketch) -> Option<(EntityId, DVec2)> {
        if let Some((data, result)) = &self.pending_corner {
            let arc = result.created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc))?;
            return Some((arc, curve_mid(&data.sketch, arc)?));
        }
        let arc = self.fillet_session.filter(|a| sk.entity(*a).is_some())?;
        Some((arc, curve_mid(sk, arc)?))
    }

    /// The radius manipulator (live sketch/1934): from the fillet arc's
    /// midpoint toward its centre; (start, apex of the head).
    pub fn fillet_arrow(&self, sk: &Sketch) -> Option<(DVec2, DVec2)> {
        let (arc, m) = self.fillet_handle(sk)?;
        let sk = self.pending_corner.as_ref().map(|(d, _)| &d.sketch).unwrap_or(sk);
        let Some(Curve2::Arc { center, .. }) = sk.curve(arc) else { return None };
        let d = (center - m).try_normalize()?;
        Some((m, m + d * self.world_per_px * (FILLET_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX)))
    }

    /// Where the pending fillet's "R" leader meets the arc: its upper end
    /// (live sketch/1934: the tangent point on the top edge).
    fn fillet_label_foot(&self, sk: &Sketch) -> Option<DVec2> {
        let (arc, _) = self.fillet_handle(sk)?;
        let sk = self.pending_corner.as_ref().map(|(d, _)| &d.sketch).unwrap_or(sk);
        let Some(Curve2::Arc { center, radius, start_angle, end_angle }) = sk.curve(arc) else { return None };
        let (a, b) = (center + DVec2::from_angle(start_angle) * radius, center + DVec2::from_angle(end_angle) * radius);
        Some(if (a.y - b.y).abs() > 1e-12 { if a.y > b.y { a } else { b } } else if a.x < b.x { a } else { b })
    }

    /// The pending fillet's radius label ("R5", live sketch/1934): its
    /// right-centre anchor, up-left of the leader, and the radius.
    pub fn fillet_value_label(&self, sk: &Sketch) -> Option<(DVec2, f64)> {
        if self.tool != Tool::Fillet || self.pending_corner.is_none() {
            return None;
        }
        let pa = self.fillet_label_foot(sk)?;
        let px = self.world_per_px;
        Some((pa + DVec2::new(-7.5 * px, 13.0 * px), self.fillet_radius))
    }

    /// Preview of a fillet at the hovered corner: the arc, and whether it fits.
    fn fillet_preview(&self, sk: &Sketch) -> Option<(Vec<DVec2>, bool)> {
        if let Some((data, result)) = &self.pending_corner {
            let arc = result.created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc))?;
            return Some((data.sketch.tessellate(arc, self.world_per_px), true));
        }
        let pid = self.hover.filter(|e| is_point(sk, *e))?;
        let users: Vec<EntityId> = sk.entities().filter(|e| e.endpoints().is_some_and(|(s, en)| s == pid || en == pid)).map(|e| e.id).collect();
        if users.len() != 2 {
            return None;
        }
        let v = sk.point(pid)?;
        let far = |l: EntityId| -> Option<DVec2> {
            let (s, e) = sk.endpoints(l)?;
            sk.point(if s == pid { e } else { s })
        };
        let (Some(a), Some(b)) = (far(users[0]), far(users[1])) else { return None };
        let (Some(_), Some(_)) = (line_of(sk, users[0]), line_of(sk, users[1])) else { return None };
        let (ua, ub) = ((a - v).try_normalize()?, (b - v).try_normalize()?);
        let theta = ua.angle_to(ub).abs();
        if theta < 1e-6 || (PI - theta).abs() < 1e-6 {
            return None;
        }
        let r = self.fillet_radius;
        let t = r / (theta * 0.5).tan();
        let c = v + (ua + ub).normalize() * (r / (theta * 0.5).sin());
        let (pa, pb) = (v + ua * t, v + ub * t);
        let fits = t <= (a - v).length() && t <= (b - v).length();
        let a0 = (pa - c).to_angle();
        let sweep = noshape_sketch::geom::ccw_sweep(a0, (pb - c).to_angle());
        let sweep = if sweep > PI { sweep - TAU } else { sweep };
        Some((arc_points(c, r, a0, sweep), fits))
    }

    /// Open the value box of a dimension (the Onshape dimension context dialog).
    fn open_value_box(&mut self, data: &SketchData, cid: ConstraintId) {
        let Some(text) = dimension_edit_text(data, cid, &UnitPrefs::default()) else { return };
        self.dim_edit = Some(DimEdit { constraint: cid, text, created: true });
    }

    /// Trim tool: trim the curve under `p` (click, or each curve a drag crosses).
    fn trim_at(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>) -> EditOutcome {
        let r = self.world_per_px * 6.0;
        let id = hovered.or_else(|| data.sketch.pick(p, r));
        let Some(id) = id else { return EditOutcome::none() };
        let res = if is_point(&data.sketch, id) {
            // Standalone points are deleted.
            if data.sketch.users_of_point(id).is_empty() {
                data.sketch.remove_entities(&[id]);
                Ok(())
            } else {
                return EditOutcome::none();
            }
        } else {
            data.sketch.trim(id, p).map(|_| ())
        };
        match res {
            Ok(()) => {
                data.sketch.solve();
                prune_exprs(data);
                self.selection.retain(|e| data.sketch.entity(*e).is_some());
                if self.hover.is_some_and(|h| data.sketch.entity(h).is_none()) {
                    self.hover = None;
                }
                EditOutcome::changed()
            }
            Err(e) => {
                self.message = Some(format!("{e}"));
                EditOutcome::redraw()
            }
        }
    }

    /// Slot tool: slots of one shared width around the given centerlines
    /// (connected curves give one slot following the chain).
    fn make_slots(&mut self, data: &mut SketchData, ids: &[EntityId]) -> EditOutcome {
        // A previous preview commits first (live: the next click commits it).
        let committed = self.commit_slot(data).changed;
        let curves: Vec<EntityId> = ids.iter().copied().filter(|e| data.sketch.kind(*e).is_some_and(|k| k != EntityKind::Point)).collect();
        if curves.is_empty() {
            return if committed { EditOutcome::changed() } else { EditOutcome::redraw() };
        }
        let mut preview = data.clone();
        let out = self.make_slots_in(&mut preview, &curves);
        if let Some(created) = out {
            self.pending_slot = Some((preview, created));
        }
        if committed { EditOutcome::changed() } else { EditOutcome::redraw() }
    }

    /// The pending slot's width dimension: its constraint, label position
    /// and value (live sketch/304 shows "Ø20" before the slot commits).
    pub fn slot_dimension(&self) -> Option<(ConstraintId, DVec2, f64)> {
        let (pd, created) = self.pending_slot.as_ref()?;
        let c = pd.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::Diameter { curve, .. } if created.contains(&curve)))?;
        let at = pd.dimension_labels.get(&c.id.0).map(|l| DVec2::new(l[0], l[1]))?;
        Some((c.id, at, c.kind.value()?))
    }

    /// Is a slot preview waiting to be committed?
    pub fn slot_pending(&self) -> bool {
        self.pending_slot.is_some()
    }

    /// Commit the slot preview into the sketch.
    pub fn commit_slot(&mut self, data: &mut SketchData) -> EditOutcome {
        let Some((preview, created)) = self.pending_slot.take() else { return EditOutcome::none() };
        *data = preview;
        self.finish_create(data, &created);
        EditOutcome::changed()
    }

    fn make_slots_in(&mut self, data: &mut SketchData, curves: &[EntityId]) -> Option<Vec<EntityId>> {
        let (created, caps) = match data.sketch.slot_chains(curves, self.slot_width) {
            Ok(r) => r,
            Err(e) => {
                self.message = Some(format!("{e}"));
                return None;
            }
        };
        if created.is_empty() {
            return None;
        }
        // One width for every slot of the session: the end cap (the one centred
        // on the first curve's end, live REST "DIAMETER endCap") carries the
        // diameter dimension, the others are equal to it.
        let first = self.slot_session.filter(|f| data.sketch.entity(*f).is_some());
        let end_point = data.sketch.endpoints(curves[0]).map(|(_, e)| e);
        let lead = caps.iter().position(|c| end_point.is_some() && data.sketch.entity(*c).and_then(|e| e.center()) == end_point).unwrap_or(0);
        for (k, cap) in caps.iter().enumerate() {
            match first {
                Some(f) => {
                    let _ = data.sketch.add_constraint(ConstraintKind::Equal { a: f, b: *cap });
                }
                None if k == lead => {
                    // Live (sketch/1952): "Ø20" straight above the cap's centre at twice
                    // the width, its leader radial (vertical). No value box opens
                    // for a slot (sketch/307–311).
                    let label = data.sketch.entity(*cap).and_then(|e| e.center()).and_then(|c| data.sketch.point(c)).map(|c| c + DVec2::Y * self.slot_width * 2.0);
                    let _ = add_dimension(data, ConstraintKind::Diameter { curve: *cap, value: self.slot_width }, label, None);
                    self.slot_session = Some(*cap);
                }
                None => {
                    let _ = data.sketch.add_constraint(ConstraintKind::Equal { a: caps[lead], b: *cap });
                }
            }
        }
        data.sketch.solve();
        Some(created)
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
            Tool::Line if self.tangent_mode => {
                let Some(from) = self.chain_end else {
                    self.tangent_mode = false;
                    return EditOutcome::redraw();
                };
                match sk.tangent_arc(from, click.pos) {
                    Ok(ids) => {
                        let end = ids.points.last().copied();
                        let arc = ids.curves[0];
                        let end = end.map(|e| attach(sk, e, Some(arc), &click)).unwrap_or(from);
                        // Back to lines, continuing from the arc end (Onshape).
                        self.tangent_mode = false;
                        self.left_chain_end = false;
                        self.chain_end = Some(end);
                        self.clicks = vec![Click { pos: sk.point(end).unwrap_or(click.pos), inference: None }];
                        self.finish_create(data, &ids.curves);
                        self.set_otf(data, vec![radius_field(&data.sketch, arc, self.world_per_px)]);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Line => {
                if double_click {
                    self.clicks.clear();
                    self.chain_end = None;
                    return EditOutcome::redraw();
                }
                let Some(prev) = self.clicks.last().cloned() else {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                };
                let start_pos = self.chain_end.and_then(|e| sk.point(e)).unwrap_or(prev.pos);
                if (start_pos - click.pos).length() < 1e-9 {
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
                } else {
                    self.clicks = vec![click];
                    self.chain_end = Some(b);
                    self.left_chain_end = false;
                }
                self.set_otf(data, vec![length_field(&data.sketch, line, self.world_per_px)]);
                EditOutcome::changed()
            }
            Tool::MidpointLine => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let m = self.clicks.remove(0);
                if (m.pos - click.pos).length() < 1e-9 {
                    return EditOutcome::none();
                }
                let pa = sk.add_point(m.pos * 2.0 - click.pos);
                let pb = sk.add_point(click.pos);
                let Ok(line) = sk.add_line_between(pa, pb) else { return EditOutcome::none() };
                let mid = sk.add_point(m.pos);
                let _ = sk.add_constraint(ConstraintKind::Midpoint { point: mid, curve: line });
                attach(sk, mid, None, &m);
                attach(sk, pb, Some(line), &click);
                self.finish_create(data, &[line]);
                self.set_otf(data, vec![length_field(&data.sketch, line, self.world_per_px)]);
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
                let corners = &ids.points[..4];
                let nearest = |q: DVec2, sk: &Sketch| corners.iter().copied().min_by(|x, y| (sk.point(*x).unwrap_or_default() - q).length().total_cmp(&(sk.point(*y).unwrap_or_default() - q).length()));
                let first_pt = if self.tool == Tool::CornerRect { nearest(first.pos, sk) } else { ids.points.last().copied() };
                let second_pt = nearest(click.pos, sk);
                if let Some(p0) = first_pt {
                    attach(sk, p0, None, &first);
                }
                if let Some(p2) = second_pt {
                    attach(sk, p2, None, &click);
                }
                // Alt: square.
                if self.alt_held {
                    let _ = sk.add_constraint(ConstraintKind::Equal { a: ids.curves[0], b: ids.curves[1] });
                }
                self.finish_create(data, &ids.curves);
                let w = self.world_per_px;
                // Live (sketch/05-06): the width and height boxes sit on the
                // two edges through the first corner (edges run bottom,
                // right, top, left).
                let (horizontal, vertical) = if self.tool == Tool::CornerRect {
                    let (lo, hi) = (first.pos.min(click.pos), first.pos.max(click.pos));
                    let top = (first.pos.y - hi.y).abs() < (first.pos.y - lo.y).abs();
                    let left = (first.pos.x - lo.x).abs() < (first.pos.x - hi.x).abs();
                    (ids.curves[if top { 2 } else { 0 }], ids.curves[if left { 3 } else { 1 }])
                } else {
                    (ids.curves[0], ids.curves[1])
                };
                let mut fields = vec![length_field(&data.sketch, horizontal, w)];
                if !self.alt_held {
                    fields.push(length_field(&data.sketch, vertical, w));
                }
                self.set_otf(data, fields);
                EditOutcome::changed()
            }
            Tool::AlignedRect => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                let c2 = if self.alt_held { aligned_square(c[0].pos, c[1].pos, c[2].pos) } else { c[2].pos };
                match sk.three_point_rectangle(c[0].pos, c[1].pos, c2) {
                    Ok(ids) => {
                        for (k, cl) in c.iter().take(2).enumerate() {
                            let near = ids.points.iter().copied().min_by(|x, y| (sk.point(*x).unwrap_or_default() - cl.pos).length().total_cmp(&(sk.point(*y).unwrap_or_default() - cl.pos).length()));
                            if let Some(pt) = near {
                                attach(sk, pt, None, cl);
                            }
                            let _ = k;
                        }
                        if self.alt_held {
                            let _ = sk.add_constraint(ConstraintKind::Equal { a: ids.curves[0], b: ids.curves[1] });
                        }
                        self.finish_create(data, &ids.curves);
                        let w = self.world_per_px;
                        let mut fields = vec![length_field(&data.sketch, ids.curves[0], w)];
                        if !self.alt_held {
                            fields.push(length_field(&data.sketch, ids.curves[1], w));
                        }
                        self.set_otf(data, fields);
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
                rim_autos(sk, circle, &click);
                self.finish_create(data, &[circle]);
                self.set_otf(data, vec![diameter_field(&data.sketch, circle, self.world_per_px)]);
                EditOutcome::changed()
            }
            Tool::ThreePointCircle => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                match sk.three_point_circle(c[0].pos, c[1].pos, c[2].pos) {
                    Ok(id) => {
                        for cl in &c {
                            rim_autos(sk, id, cl);
                        }
                        self.finish_create(data, &[id]);
                        self.set_otf(data, vec![diameter_field(&data.sketch, id, self.world_per_px)]);
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
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                match sk.three_point_arc(c[0].pos, c[2].pos, c[1].pos) {
                    Ok(arc) => {
                        if let Some((s, e)) = sk.endpoints(arc) {
                            let near = |q: DVec2, p: DVec2| (q - p).length();
                            let ps = sk.point(s).unwrap_or_default();
                            let (cs, ce) = if near(c[0].pos, ps) < near(c[1].pos, ps) { (&c[0], &c[1]) } else { (&c[1], &c[0]) };
                            attach(sk, s, Some(arc), cs);
                            attach(sk, e, Some(arc), ce);
                        }
                        rim_autos(sk, arc, &c[2]);
                        self.finish_create(data, &[arc]);
                        self.set_otf(data, vec![radius_field(&data.sketch, arc, self.world_per_px)]);
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
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                // The sweep follows the cursor: counter-clockwise unless the cursor went the other way.
                let ccw = self.center_arc_sweep >= 0.0;
                let id = sk.center_point_arc(c[0].pos, c[1].pos, c[2].pos, ccw);
                if let Some(noshape_sketch::Geometry::Arc { center, start, end }) = sk.entity(id).map(|e| e.geometry.clone()) {
                    attach(sk, center, None, &c[0]);
                    let (s_click, e_click) = if ccw { (&c[1], &c[2]) } else { (&c[2], &c[1]) };
                    attach(sk, start, Some(id), s_click);
                    attach(sk, end, Some(id), e_click);
                }
                self.finish_create(data, &[id]);
                self.set_otf(data, vec![radius_field(&data.sketch, id, self.world_per_px)]);
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
                    self.clicks = vec![click];
                    return EditOutcome::redraw();
                }
                match sk.tangent_arc(from, click.pos) {
                    Ok(ids) => {
                        let arc = ids.curves[0];
                        let end = ids.points.last().map(|e| attach(sk, *e, Some(arc), &click));
                        self.chain_end = end;
                        self.clicks = vec![click];
                        self.finish_create(data, &ids.curves);
                        self.set_otf(data, vec![radius_field(&data.sketch, arc, self.world_per_px)]);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Spline | Tool::Bezier => {
                if self.tool == Tool::Bezier && self.clicks.len() >= 16 && !double_click {
                    self.message = Some("The maximum bezier degree (15) has been reached.".into());
                    return EditOutcome::redraw();
                }
                if double_click && self.clicks.len() >= 2 {
                    return self.finish_spline(data, false);
                }
                if self.clicks.last().is_some_and(|c| (c.pos - click.pos).length() < 1e-9) {
                    return EditOutcome::none();
                }
                // A spline closes when its last point lands on its first.
                let closes = self.tool == Tool::Spline && self.clicks.len() >= 3 && self.clicks.first().is_some_and(|f| (f.pos - click.pos).length() <= self.world_per_px * 9.0);
                self.clicks.push(click);
                if closes {
                    return self.finish_spline(data, true);
                }
                EditOutcome::redraw()
            }
            Tool::Conic => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                match sk.add_conic(c[0].pos, c[2].pos, c[1].pos, self.conic_rho.clamp(0.01, 0.99)) {
                    Ok(id) => {
                        if let Some((s, e)) = sk.endpoints(id) {
                            attach(sk, s, Some(id), &c[0]);
                            attach(sk, e, Some(id), &c[1]);
                        }
                        self.finish_create(data, &[id]);
                        // The rho box opens right away (Onshape).
                        let label = curve_mid(&data.sketch, id).unwrap_or_default() + DVec2::new(0.0, self.world_per_px * 26.0);
                        // Rho is an intrinsic dimension: accepting or dismissing
                        // the initial box keeps the default driving value.
                        let rho = self.conic_rho.clamp(0.01, 0.99);
                        add_dimension(data, ConstraintKind::Rho { conic: id, value: rho }, Some(label), None);
                        self.set_otf(data, vec![OtfField { kind: ConstraintKind::Rho { conic: id, value: 0.0 }, scale: 1.0, label }]);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Polygon | Tool::CircumPolygon => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                // The second click locks the circumference; then moving toward /
                // away from the polygon sets the side count; click (or Enter) sets it.
                if self.clicks.len() == 1 {
                    if (click.pos - self.clicks[0].pos).length() < 1e-9 {
                        return EditOutcome::none();
                    }
                    self.polygon_base_sides = self.polygon_sides.clamp(3, 50);
                    self.clicks.push(click);
                    self.message = Some(format!("Sides: {} (drag toward / away from the polygon, type a number, click or press Enter to set)", self.polygon_sides));
                    return EditOutcome::redraw();
                }
                let c = self.clicks.remove(0);
                let rim = self.clicks.remove(0);
                let next_center = ((click.pos - rim.pos).length() > self.world_per_px * 9.0).then_some(click);
                let sides = self.polygon_sides.clamp(3, 50);
                self.polygon_typed.clear();
                match sk.polygon(c.pos, rim.pos, sides, self.tool == Tool::Polygon) {
                    Ok(ids) => {
                        if let Some(&cp) = ids.points.last() {
                            attach(sk, cp, None, &c);
                        }
                        let drawn = (rim.pos - c.pos).length();
                        let circle = ids.construction.iter().copied().min_by(|x, y| {
                            let r = |e: EntityId| match sk.curve(e) {
                                Some(Curve2::Circle { radius, .. }) => (radius - drawn).abs(),
                                _ => f64::INFINITY,
                            };
                            r(*x).total_cmp(&r(*y))
                        });
                        self.finish_create(data, &ids.curves);
                        if let Some(circle) = circle {
                            self.set_otf(data, vec![diameter_field(&data.sketch, circle, self.world_per_px)]);
                        }
                        if let Some(center) = next_center {
                            self.clicks.push(center);
                        }
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
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                match sk.ellipse_3pt(c[0].pos, c[1].pos, c[2].pos) {
                    Ok(id) => {
                        if let Some(ctr) = sk.entity(id).and_then(|e| e.center()) {
                            attach(sk, ctr, None, &c[0]);
                        }
                        self.finish_create(data, &[id]);
                        let w = self.world_per_px;
                        self.set_otf(data, ellipse_fields(&data.sketch, id, w));
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::EllipticalArc => {
                self.clicks.push(click);
                if self.clicks.len() < 4 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                let Some((center, major, minor)) = construction_ellipse(c[0].pos, c[1].pos, c[2].pos) else { return EditOutcome::redraw() };
                let t0 = noshape_sketch::sketch::ellipse_param(center, major, minor, c[2].pos);
                let mut t1 = noshape_sketch::sketch::ellipse_param(center, major, minor, c[3].pos);
                while t1 <= t0 {
                    t1 += TAU;
                }
                let id = sk.add_elliptical_arc(center, major, minor, t0, t1);
                if let Some(e) = sk.entity(id).cloned() {
                    if let Some(ctr) = e.center() {
                        attach(sk, ctr, None, &c[0]);
                    }
                    if let Some((s, en)) = e.endpoints() {
                        attach(sk, s, Some(id), &c[2]);
                        attach(sk, en, Some(id), &c[3]);
                    }
                }
                self.finish_create(data, &[id]);
                let w = self.world_per_px;
                self.set_otf(data, ellipse_fields(&data.sketch, id, w));
                EditOutcome::changed()
            }
            Tool::InsertImage => {
                let Some((name, bytes, aspect)) = self.pending_image.clone() else {
                    self.message = Some("Choose an image file first".into());
                    return EditOutcome::redraw();
                };
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let first = self.clicks.remove(0);
                let (origin, height) = image_box(first.pos, click.pos, aspect);
                if height < self.world_per_px * 2.0 {
                    return EditOutcome::redraw();
                }
                match data.add_image(&name, &bytes, origin, height, aspect) {
                    Ok(_) => {
                        data.sketch.solve();
                        self.pending_image = None;
                        self.set_tool(Tool::Select);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(e);
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Text => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let first = self.clicks.remove(0);
                let (origin, height) = self.text_box_from(first.pos, click.pos);
                self.pending_text = Some((origin, height));
                self.message = None;
                EditOutcome::redraw()
            }
            _ => EditOutcome::none(),
        }
    }

    /// One-line measurement of the sketch selection for the status area at
    /// the bottom right (live Onshape: `Length: 1.003 in`, `Diameter: 0.401
    /// in`, `Angle: 35.538 deg`, `Parallel dist: …`, `Point: X … Y … Z …`).
    /// Returns (label, value); `None` for an empty or mixed selection.
    pub fn selection_readout(&self, data: &SketchData, units: &UnitPrefs) -> Option<(String, String)> {
        let sk = &data.sketch;
        let len = |v: f64| noshape_units::format_quantity(Quantity::new(v, Dim::LENGTH), units);
        let sel: Vec<EntityId> = self.selection.iter().copied().filter(|e| sk.entity(*e).is_some()).collect();
        match sel.as_slice() {
            [e] => {
                if let Some(p) = sk.point(*e) {
                    let w = self.frame.to_world(p);
                    return Some(("Point".into(), format!("X {}  Y {}  Z {}", len(w.x), len(w.y), len(w.z))));
                }
                match sk.curve(*e)? {
                    Curve2::Line { a, b } => Some(("Length".into(), len((b - a).length()))),
                    Curve2::Circle { radius, .. } => Some(("Diameter".into(), len(radius * 2.0))),
                    Curve2::Arc { radius, .. } => Some(("Radius".into(), len(radius))),
                    c => Some(("Length".into(), len(c.length()))),
                }
            }
            [a, b] => {
                if let (Some(p), Some(q)) = (sk.point(*a), sk.point(*b)) {
                    return Some(("Distance".into(), len((q - p).length())));
                }
                let (Some(Curve2::Line { a: a0, b: a1 }), Some(Curve2::Line { a: b0, b: b1 })) = (sk.curve(*a), sk.curve(*b)) else { return None };
                let (u, v) = ((a1 - a0).try_normalize()?, (b1 - b0).try_normalize()?);
                if u.perp_dot(v).abs() < 1e-9 {
                    Some(("Parallel dist".into(), len((b0 - a0).perp_dot(u).abs())))
                } else {
                    let ang = u.dot(v).clamp(-1.0, 1.0).acos();
                    let ang = ang.min(PI - ang);
                    let text = noshape_units::format_quantity(Quantity::new(ang, Dim::ANGLE), units).replace('°', " deg");
                    Some(("Angle".into(), text))
                }
            }
            _ => None,
        }
    }

    /// Open an existing text in the Text dialog with its content and style.
    pub fn begin_text_edit(&mut self, data: &SketchData, id: u32) -> bool {
        let Some(t) = data.texts.iter().find(|t| t.id == id) else { return false };
        self.text_string = t.text.clone();
        self.text_font = t.font.clone();
        self.text_bold = t.bold;
        self.text_italic = t.italic;
        self.text_flip_horizontal = t.mirror_horizontal;
        self.text_flip_vertical = t.mirror_vertical;
        self.text_expression = t.expression;
        self.editing_text = Some(id);
        self.message = None;
        true
    }

    /// The Text dialog is open (creating a new text or editing one).
    /// What Enter commits in sketch mode (live "Sketch dimension display +
    /// Enter/Esc"): the step being set up, if any. Enter never accepts the
    /// sketch (only ✓ does), with or without a tool, a selection or the
    /// focus on the Sketch dialog. (An open dimension box takes its own
    /// Enter: it commits the value and the Dimension tool stays.)
    pub fn enter_target(&self) -> EnterTarget {
        if self.text_dialog_open() {
            EnterTarget::Text
        } else if self.slot_pending() {
            EnterTarget::Slot
        } else if self.corner_pending() {
            EnterTarget::Corner
        } else if self.polygon_sides_pending() {
            EnterTarget::PolygonSides
        } else if self.tool.is_pattern() && !self.selection.is_empty() {
            EnterTarget::Pattern
        } else if self.offset_pending.is_some() {
            EnterTarget::Offset
        } else if self.otf.is_some() {
            EnterTarget::OnTheFly
        } else {
            EnterTarget::Nothing
        }
    }

    pub fn text_dialog_open(&self) -> bool {
        self.pending_text.is_some() || self.editing_text.is_some()
    }

    pub fn accept_text(&mut self, data: &mut SketchData) -> EditOutcome {
        if self.text_string.chars().count() > 250 {
            self.message = Some("Text is limited to 250 characters".into());
            return EditOutcome::redraw();
        }
        if self.text_string.trim().is_empty() {
            self.message = Some("Enter text".into());
            return EditOutcome::redraw();
        }
        if let Some(id) = self.editing_text {
            let (text, font, bold, italic, fh, fv, expr) = (self.text_string.clone(), self.text_font.clone(), self.text_bold, self.text_italic, self.text_flip_horizontal, self.text_flip_vertical, self.text_expression);
            let ok = self.edit_text(data, id, |t| {
                t.text = text;
                t.font = font;
                t.bold = bold;
                t.italic = italic;
                t.mirror_horizontal = fh;
                t.mirror_vertical = fv;
                t.expression = expr;
            });
            if !ok {
                return EditOutcome::redraw();
            }
            self.editing_text = None;
            self.message = None;
            return EditOutcome::changed();
        }
        let Some((origin, height)) = self.pending_text else { return EditOutcome::none() };
        match data.add_text(&self.text_string, &self.text_font, self.text_bold, self.text_italic, origin, height) {
            Ok(id) => {
                if let Some(t) = data.texts.iter_mut().find(|t| t.id == id) {
                    t.mirror_horizontal = self.text_flip_horizontal;
                    t.mirror_vertical = self.text_flip_vertical;
                    t.expression = self.text_expression;
                }
                self.pending_text = None;
                self.set_tool(Tool::Select);
                self.selection = data.texts.iter().find(|t| t.id == id).map(|t| t.box_lines.iter().map(|l| EntityId(*l)).collect()).unwrap_or_default();
                data.sketch.solve();
                EditOutcome::changed()
            }
            Err(e) => {
                self.message = Some(e);
                EditOutcome::redraw()
            }
        }
    }

    /// Create the spline / bezier from the clicked points.
    fn finish_spline(&mut self, data: &mut SketchData, closed: bool) -> EditOutcome {
        let clicks: Vec<Click> = std::mem::take(&mut self.clicks);
        let mut pts: Vec<DVec2> = clicks.iter().map(|c| c.pos).collect();
        if closed
            && let Some(f) = pts.first().copied()
            && let Some(l) = pts.last_mut()
        {
            *l = f;
        }
        if pts.len() < 2 {
            return EditOutcome::redraw();
        }
        let sk = &mut data.sketch;
        let r = if self.tool == Tool::Spline {
            // Open splines get tangent handles at their ends (Onshape).
            if closed { sk.add_fit_spline(&pts) } else { sk.add_fit_spline_with_handles(&pts) }
        } else if pts.len() == 2 {
            Ok(sk.add_line(pts[0], pts[1]))
        } else {
            let deg = (pts.len() - 1).min(15);
            sk.add_bspline(&pts, deg)
        };
        let Ok(id) = r else { return EditOutcome::redraw() };
        // Snapped first / last points carry their inferred constraints.
        if let Some((first, last)) = sk.entity(id).and_then(|e| e.endpoints()) {
            if closed && first != last {
                let _ = sk.merge_points(last, first);
            } else {
                attach(sk, last, Some(id), clicks.last().expect("clicks"));
            }
            attach(sk, first, Some(id), clicks.first().expect("clicks"));
        }
        self.finish_create(data, &[id]);
        EditOutcome::changed()
    }

    /// Live Onshape: the first Esc in a Spline / Bezier with at least two
    /// placed points commits the curve through them (the rubber-band point
    /// is dropped) and keeps the tool; the next Esc exits it.
    pub fn escape_commit(&mut self, data: &mut SketchData) -> Option<EditOutcome> {
        // A slot preview commits when leaving the tool (live).
        if self.pending_slot.is_some() {
            let out = self.commit_slot(data);
            self.set_tool(Tool::Select);
            return Some(out);
        }
        if !matches!(self.tool, Tool::Spline | Tool::Bezier) || self.clicks.len() < 2 {
            return None;
        }
        self.otf = None;
        self.otf_typing = false;
        Some(self.finish_spline(data, false))
    }

    /// Confirm spline (context menu): finish at the point before the cursor.
    pub fn confirm_spline(&mut self, data: &mut SketchData) -> EditOutcome {
        if self.tool == Tool::Spline && self.clicks.len() >= 2 {
            return self.finish_spline(data, false);
        }
        EditOutcome::none()
    }

    // ------------------------------------------------------------ on-the-fly dimensions

    fn set_otf(&mut self, data: &SketchData, fields: Vec<OtfField>) {
        let fields: Vec<OtfField> = fields.into_iter().filter(|f| data.sketch.measure_new(&f.kind).is_some()).collect();
        self.otf = (!fields.is_empty()).then_some(OnTheFly { fields, active: 0 });
        self.otf_typing = false;
    }

    /// Boxes to show: label position, suggested value text and whether the box is active.
    pub fn otf_boxes(&self, data: &SketchData, units: &UnitPrefs) -> Vec<(DVec2, String, bool)> {
        let Some(o) = &self.otf else { return Vec::new() };
        o.fields
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                let v = data.sketch.measure_new(&f.kind)? * f.scale;
                let dim = dim_unit(&f.kind);
                Some((f.label, noshape_units::format_quantity(Quantity::new(v, dim), units), i == o.active))
            })
            .collect()
    }

    /// Text of the idle boxes, like a dimension label (live Onshape shows a
    /// bare `1.269`, without units, until you type into the box).
    pub fn otf_labels(&self, data: &SketchData, units: &UnitPrefs) -> Vec<String> {
        let Some(o) = &self.otf else { return Vec::new() };
        o.fields.iter().filter_map(|f| Some(dim_value_text(&f.kind, data.sketch.measure_new(&f.kind)?, units))).collect()
    }

    /// Switch the active on-the-fly box (Alt+arrows / Tab).
    pub fn otf_step(&mut self, delta: isize) {
        if let Some(o) = &mut self.otf {
            let n = o.fields.len() as isize;
            o.active = (o.active as isize + delta).rem_euclid(n.max(1)) as usize;
        }
    }

    /// Commit the active on-the-fly box: add a driving dimension with the
    /// typed value. Returns an error message for invalid input.
    pub fn commit_otf(&mut self, data: &mut SketchData, text: &str, units: &UnitPrefs, vars: &dyn noshape_units::VariableScope) -> Result<(), String> {
        let Some(o) = self.otf.clone() else { return Ok(()) };
        let Some(f) = o.fields.get(o.active).cloned() else { return Ok(()) };
        let dim = dim_unit(&f.kind);
        // Stored with the workspace unit (live: "10+5" → "(10+5) mm"), so a
        // later change of the workspace units keeps the geometry.
        let text = noshape_model::quantity_text::commit_text(text.trim().trim_start_matches('=').trim(), dim, units, vars);
        let q = noshape_units::evaluate_as(&text, dim, units, vars).map_err(|e| e.to_string())?;
        let expr = is_expression(&text).then_some(text.as_str());
        self.commit_otf_value(data, o, f, q.value, expr)
    }

    /// Enter while a tool shows an on-the-fly box (live Onshape: Enter in a
    /// Line chain turns the previous segment's boxed length into a placed
    /// driving dimension): dimension the active box at its current value.
    /// Returns false when no box is showing.
    pub fn accept_otf(&mut self, data: &mut SketchData) -> Result<bool, String> {
        let Some(o) = self.otf.clone() else { return Ok(false) };
        let Some(f) = o.fields.get(o.active).cloned() else { return Ok(false) };
        let Some(v) = data.sketch.measure_new(&f.kind) else { return Ok(false) };
        let value = v * f.scale;
        self.commit_otf_value(data, o, f, value, None).map(|_| true)
    }

    fn commit_otf_value(&mut self, data: &mut SketchData, o: OnTheFly, f: OtfField, value: f64, expr: Option<&str>) -> Result<(), String> {
        let q = Quantity::new(value, dim_unit(&f.kind));
        if !(q.value.is_finite() && q.value > 0.0) {
            return Err("Value must be positive".into());
        }
        let mut kind = f.kind.clone();
        kind.set_value(q.value / f.scale);
        if matches!(kind, ConstraintKind::Rho { .. }) && q.value >= 1.0 {
            return Err("Rho must be between 0 and 1".into());
        }
        let existing = match kind {
            ConstraintKind::Rho { conic, .. } => data.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::Rho { conic: id, .. } if id == conic)).map(|c| c.id),
            _ => None,
        };
        let cid = if let Some(cid) = existing {
            if let Some(text) = expr { data.dimension_exprs.insert(cid.0, ExprValue::new(text)); }
            else { data.dimension_exprs.remove(&cid.0); }
            cid
        } else {
            add_dimension(data, kind, Some(f.label), expr).ok_or("Could not create the dimension.")?
        };
        if data.sketch.constraint(cid).is_some_and(|c| c.driving) {
            data.sketch.set_dimension_value(cid, q.value / f.scale).map_err(|e| e.to_string())?;
            data.sketch.solve();
        } else {
            self.message = Some("The dimension over-defines the sketch; it was added as a driven dimension".into());
        }
        // Next box (rectangle width after length, ellipse minor after major).
        let mut o = o;
        o.fields.remove(o.active);
        o.active = o.active.min(o.fields.len().saturating_sub(1));
        self.otf = (!o.fields.is_empty()).then_some(o);
        self.otf_typing = false;
        // The geometry moved: refresh the snap of the tool in progress.
        if let Some(c) = self.cursor
            && self.tool.draws()
        {
            self.inference = self.infer(&data.sketch, c, self.shift_held);
        }
        Ok(())
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
                    // Shift suppresses snapping onto other geometry.
                    DragHandle::Point(id) => self.drag_snap = if self.shift_held { None } else { self.point_drop_target(&data.sketch, id) },
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
            Some(DragState::OffsetArrow { start, moved }) => {
                if !moved && (p - start).length() < self.world_per_px * 3.0 {
                    return EditOutcome::none();
                }
                self.drag = Some(DragState::OffsetArrow { start, moved: true });
                // Signed distance of the cursor from the curve along the arrow's normal.
                if let Some((mid, n)) = self.offset_frame(&data.sketch) {
                    let d = (p - mid).dot(n);
                    if d.abs() > self.world_per_px {
                        self.offset_distance = d.abs();
                        self.offset_flip = (d < 0.0) != self.offset_flip_base(&data.sketch);
                    }
                }
                EditOutcome::redraw()
            }
            Some(DragState::ChamferDistance) => {
                // Both distances follow the cursor along the arrow's edge.
                let Some((sharp, u, _, dims)) = self.chamfer_frame() else { return EditOutcome::none() };
                let d = (p - sharp).dot(u).max(self.world_per_px);
                if let Some((preview, _)) = &mut self.pending_corner {
                    for cid in dims {
                        let _ = preview.sketch.set_dimension_value(cid, d);
                    }
                    preview.sketch.solve();
                    self.chamfer_distance = d;
                }
                EditOutcome::redraw()
            }
            Some(DragState::FilletRadius { arc }) => {
                let pending = self.pending_corner.is_some();
                let data = if let Some((preview, _)) = &mut self.pending_corner { preview } else { data };
                // The arc midpoint follows the cursor along the corner bisector.
                let Some(Curve2::Arc { center, radius, start_angle, end_angle }) = data.sketch.curve(arc) else { return EditOutcome::none() };
                let half = (end_angle - start_angle) * 0.5;
                let u = DVec2::from_angle(start_angle + half);
                let k = 1.0 / half.cos().max(1e-6) - 1.0;
                let sharp = center + u * radius / half.cos().max(1e-6);
                let d = (sharp - p).dot(u);
                let r = (d / k.max(1e-9)).max(self.world_per_px);
                let dim = data.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::Radius { curve, .. } if curve == arc)).map(|c| c.id);
                if let Some(dim) = dim {
                    let _ = data.sketch.set_dimension_value(dim, r);
                    data.dimension_exprs.remove(&dim.0);
                    data.sketch.solve();
                    self.fillet_radius = r;
                }
                if pending { EditOutcome::redraw() } else { EditOutcome::changed() }
            }
            Some(DragState::TrimPath { mut points }) => {
                let last = points.last().copied().unwrap_or(p);
                let mut out = EditOutcome::redraw();
                // Intersect the swept segment rather than sampling it: a fast
                // pointer event can cross the whole viewport and must still
                // trim every crossed curve, including very short ones.
                let path = Curve2::Line { a: last, b: p };
                let radius = self.world_per_px * 6.0;
                let bounds = path.bbox().expanded(radius);
                let mut hits = Vec::new();
                for entity in data.sketch.entities() {
                    if !data.sketch.entity_bbox(entity.id).is_some_and(|b| b.intersects(&bounds)) { continue; }
                    if let Some(q) = data.sketch.point(entity.id) {
                        if data.sketch.users_of_point(entity.id).is_empty() {
                            let (t, _, d) = path.closest(q);
                            if d <= radius { hits.push((t, q, entity.id)); }
                        }
                    } else if let Some(curve) = data.sketch.curve(entity.id) {
                        for hit in noshape_sketch::intersect::intersect(&path, &curve, self.world_per_px * 0.01) {
                            hits.push((hit.t1, hit.point, entity.id));
                        }
                    }
                }
                hits.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (_, q, entity) in hits {
                    // Trimming an earlier hit may have replaced this curve.
                    let hovered = data.sketch.entity(entity).map(|_| entity);
                    let o = self.trim_at(data, q, hovered);
                    if o.changed {
                        out = o;
                    }
                }
                points.push(p);
                self.drag = Some(DragState::TrimPath { points });
                out
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
            // button is held (click-drag placement).
            None if self.tool.draws() => self.pointer_move(data, p, self.shift_held, self.alt_held),
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
        // The sketch origin (a fixed point is created on drop).
        if pos.length() <= r && !sk.entity(id).is_some_and(|e| e.external) {
            return Some((noshape_sketch::ORIGIN, DVec2::ZERO, true));
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
        let out = self.tool_up_inner(data, p);
        self.follow_transform_selection(&data.sketch);
        out
    }

    /// Live Onshape: the Transform triad sits on the picked entities
    /// (sketch/139) until its center is dragged or a transform is applied.
    fn follow_transform_selection(&mut self, sk: &Sketch) {
        if self.tool != Tool::Transform || self.xf_center_pinned || self.xf_offset != DVec2::ZERO || self.xf_angle != 0.0 || self.xf_scale != 1.0 {
            return;
        }
        if let Some(c) = self.selection_center(sk) {
            self.pattern_center = c;
        }
    }

    fn tool_up_inner(&mut self, data: &mut SketchData, p: DVec2) -> EditOutcome {
        // Press-drag-release with a drawing tool: the release is the next click.
        if self.tool.draws()
            && self.drag.is_none()
            && let Some(start) = self.press.take()
            && (p - start).length() > self.world_per_px * 4.0
        {
            let tool = self.tool;
            let tangent = self.tangent_mode;
            self.cursor = Some(p);
            self.inference = self.infer(&data.sketch, p, self.shift_held);
            self.take_vertex_snap(data);
            let (pos, inference) = self.snapped().unwrap_or((p, None));
            let out = self.place(data, Click { pos, inference }, false);
            // A dragged line is a single segment (no chaining).
            if tool == Tool::Line && !tangent {
                self.clicks.clear();
                self.chain_end = None;
            }
            return out;
        }
        self.press = None;
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
                    let target = if target == noshape_sketch::ORIGIN { origin_point(&mut data.sketch) } else { target };
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
                    // Left to right: window (entirely inside); right to left: crossing.
                    let crossing = p.x < start.x;
                    let tol = self.world_per_px;
                    for e in data.sketch.entities() {
                        let Some(bb) = data.sketch.entity_bbox(e.id) else { continue };
                        let inside = bb.min.x >= lo.x && bb.min.y >= lo.y && bb.max.x <= hi.x && bb.max.y <= hi.y;
                        let hit = inside || (crossing && crosses_box(&data.sketch, e.id, lo, hi, tol));
                        if hit && self.box_remove {
                            self.selection.retain(|x| *x != e.id);
                        } else if hit && !self.selection.contains(&e.id) {
                            self.selection.push(e.id);
                        }
                    }
                } else if let Some(r) = self.hover_region.clone() {
                    // A click inside a closed profile selects the region.
                    if let Some(i) = self.selected_regions.iter().position(|x| *x == r) {
                        self.selected_regions.remove(i);
                    } else {
                        self.selected_regions.push(r);
                    }
                } else {
                    self.selection.clear();
                    self.selected_constraints.clear();
                    self.selected_regions.clear();
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
            Some(DragState::Handle(_)) | Some(DragState::TrimPath { .. }) => EditOutcome::redraw(),
            Some(DragState::FilletRadius { .. }) => if self.pending_corner.is_some() { EditOutcome::redraw() } else { EditOutcome::changed() },
            Some(DragState::ChamferDistance) => EditOutcome::redraw(),
            Some(DragState::OffsetArrow { moved, .. }) => {
                if !moved {
                    self.offset_flip = !self.offset_flip;
                }
                EditOutcome::redraw()
            }
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

    // ------------------------------------------------------------ patterns / transform

    fn pattern_transforms(&self) -> Vec<Box<dyn Fn(DVec2) -> DVec2>> {
        let mut out: Vec<Box<dyn Fn(DVec2) -> DVec2>> = Vec::new();
        match self.tool {
            Tool::LinearPattern => {
                let step = DVec2::from_angle(self.pattern_angle) * self.pattern_distance;
                let second = DVec2::from_angle(self.pattern_second_angle) * self.pattern_second_distance;
                for j in 0..self.pattern_second_count.clamp(1, 10) {
                    for k in 0..self.pattern_count.clamp(1, 10) {
                        if j == 0 && k == 0 { continue; }
                        let off = step * k as f64 + second * j as f64;
                        out.push(Box::new(move |p| p + off));
                    }
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
        // Editing a pattern: its old instances are replaced.
        if let Some(pid) = self.editing_pattern.take()
            && let Some(i) = data.patterns.iter().position(|p| p.id == pid)
        {
            let old = data.patterns.remove(i);
            let gone: Vec<EntityId> = old.instances.iter().flatten().map(|e| EntityId(*e)).filter(|e| data.sketch.entity(*e).is_some()).collect();
            data.sketch.remove_entities(&gone);
        }
        let sk = &mut data.sketch;
        let groups = match self.tool {
            Tool::LinearPattern => Some(linear_grid_pattern(data, &ids,
                (DVec2::from_angle(self.pattern_angle) * self.pattern_distance, self.pattern_count.max(1)),
                (DVec2::from_angle(self.pattern_second_angle) * self.pattern_second_distance, self.pattern_second_count.max(1)), self.world_per_px)),
            Tool::CircularPattern => Some(sk.circular_pattern_linked_groups(&ids, self.pattern_center, self.circular_count.max(1), self.circular_angle)),
            _ => None,
        };
        if let Some(groups) = groups {
            return match groups {
                Ok(groups) => {
                    let created: Vec<EntityId> = groups.iter().flatten().copied().collect();
                    let id = data.patterns.iter().map(|p| p.id + 1).max().unwrap_or(1);
                    let circular = self.tool == Tool::CircularPattern;
                    data.patterns.push(noshape_model::sketch_feature::SketchPattern {
                        id,
                        circular,
                        seeds: ids.iter().map(|e| e.0).collect(),
                        instances: groups.iter().map(|g| g.iter().map(|e| e.0).collect()).collect(),
                        count: if circular { self.circular_count } else { self.pattern_count },
                        distance: self.pattern_distance,
                        angle: if circular { self.circular_angle } else { self.pattern_angle },
                        center: [self.pattern_center.x, self.pattern_center.y],
                        second_count: self.pattern_second_count,
                        second_distance: self.pattern_second_distance,
                        second_angle: self.pattern_second_angle,
                    });
                    self.finish_create(data, &created);
                    self.selection.clear();
                    EditOutcome::changed()
                }
                Err(e) => {
                    self.message = Some(format!("{e}"));
                    EditOutcome::redraw()
                }
            };
        }
        let sk = &mut data.sketch;
        let r = match self.tool {
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

    /// Edit pattern: the pattern tool with the pattern's seeds and values.
    pub fn edit_pattern(&mut self, data: &SketchData, id: u32) -> bool {
        let Some(p) = data.patterns.iter().find(|p| p.id == id).cloned() else { return false };
        let seeds: Vec<EntityId> = p.seeds.iter().map(|e| EntityId(*e)).filter(|e| data.sketch.entity(*e).is_some()).collect();
        if seeds.is_empty() {
            self.message = Some("The pattern's seed entities no longer exist".into());
            return false;
        }
        let tool = if p.circular { Tool::CircularPattern } else { Tool::LinearPattern };
        if self.tool != tool {
            self.set_tool(tool);
        }
        self.selection = seeds;
        self.editing_pattern = Some(id);
        if p.circular {
            self.circular_count = p.count;
            self.circular_angle = p.angle;
            self.pattern_center = data.sketch.constraints().find_map(|c| match c.kind {
                ConstraintKind::CircularPatternLink { inst, center, .. } if p.instances.iter().flatten().any(|e| *e == inst.0) => data.sketch.point(center),
                _ => None,
            }).unwrap_or(DVec2::new(p.center[0], p.center[1]));
        } else {
            self.pattern_count = p.count;
            self.pattern_distance = p.distance;
            self.pattern_angle = p.angle;
            self.pattern_second_count = p.second_count.max(1);
            self.pattern_second_distance = if p.second_distance > 0.0 { p.second_distance } else { p.distance };
            self.pattern_second_angle = if p.second_distance > 0.0 { p.second_angle } else { p.angle + PI * 0.5 };
        }
        self.message = Some("Adjust the pattern, then click in empty space or press Enter".into());
        true
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
                // Live (sketch/136): a square at the first instance (spacing and
                // direction), then the count arrow right after it (handle = its head).
                out.push((PatternHandle::LinearStep, c + step));
                out.push((PatternHandle::LinearCount, c + step + dir * px * PATTERN_ARROW_HANDLE_PX));
                let second_dir = DVec2::from_angle(self.pattern_second_angle);
                let second = second_dir * self.pattern_second_distance;
                out.push((PatternHandle::LinearSecondStep, c + second));
                out.push((PatternHandle::LinearSecondCount, c + second + second_dir * px * PATTERN_ARROW_HANDLE_PX));
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

    /// One linear pattern direction (live sketch/136): its unit direction,
    /// step, count and the diagonal its label and spacing dimension go to
    /// (first direction down-right of +X, second up-left of +Y, turned with
    /// the direction).
    fn linear_direction(&self, second: bool) -> (DVec2, DVec2, usize, DVec2) {
        let (angle, distance, count) = if second {
            (self.pattern_second_angle, self.pattern_second_distance, self.pattern_second_count)
        } else {
            (self.pattern_angle, self.pattern_distance, self.pattern_count)
        };
        let dir = DVec2::from_angle(angle);
        let diag = dir.rotate(if second { DVec2::new(1.0, 1.0) } else { DVec2::new(1.0, -1.0) });
        (dir, dir * distance, count, diag)
    }

    /// The spacing dimension of a linear pattern direction: its two feet
    /// (seed and first instance, moved to the label's side).
    fn linear_spacing_feet(&self, c: DVec2, second: bool) -> (DVec2, DVec2) {
        let (dir, step, _, diag) = self.linear_direction(second);
        let n = dir.perp();
        let side = if diag.dot(n) >= 0.0 { n } else { -n } * self.world_per_px * PATTERN_DIM_PX;
        (c + side, c + step + side)
    }

    /// On-canvas instance-count labels of the active pattern (live Onshape:
    /// `3x` / `1x` next to each direction's count manipulator, click to
    /// type a count; sketch/136–138).
    pub fn pattern_count_labels(&self, sk: &Sketch) -> Vec<(PatternHandle, DVec2, usize)> {
        let px = self.world_per_px;
        self.pattern_handles(sk)
            .into_iter()
            .filter_map(|(h, at)| {
                let n = match h {
                    PatternHandle::LinearCount | PatternHandle::LinearSecondCount => {
                        // Centred ~95 px diagonally off the square (leader drawn in the overlay).
                        let second = h == PatternHandle::LinearSecondCount;
                        let (dir, _, n, diag) = self.linear_direction(second);
                        let q = at - dir * px * PATTERN_ARROW_HANDLE_PX;
                        return Some((h, q + diag * px * PATTERN_LABEL_PX, n));
                    }
                    PatternHandle::CircularCount => self.circular_count,
                    _ => return None,
                };
                Some((h, at + DVec2::new(px * 26.0, px * 22.0), n))
            })
            .collect()
    }

    /// On-canvas values of the active pattern / transform, each typed in by
    /// clicking it (Onshape has no options panel for these tools):
    /// linear spacing (a dimension between the seed and the first instance,
    /// live sketch/137; the second direction once it has 2+ instances), the
    /// angle of an open circular pattern, and the rotation / scale of a
    /// transform once they are changed (H:sketch_circular_pattern,
    /// H:sketch_transform). Values are SI (meters, radians, ratio).
    pub fn pattern_value_labels(&self, sk: &Sketch) -> Vec<(PatternHandle, DVec2, f64)> {
        let px = self.world_per_px;
        let mut out = Vec::new();
        match self.tool {
            Tool::LinearPattern => {
                let Some(c) = self.selection_center(sk) else { return out };
                for (h, distance, count) in [
                    (PatternHandle::LinearStep, self.pattern_distance, self.pattern_count),
                    (PatternHandle::LinearSecondStep, self.pattern_second_distance, self.pattern_second_count),
                ] {
                    if h == PatternHandle::LinearSecondStep && count < 2 {
                        continue;
                    }
                    // On the spacing dimension's line (live sketch/136).
                    let (f1, f2) = self.linear_spacing_feet(c, h == PatternHandle::LinearSecondStep);
                    out.push((h, (f1 + f2) * 0.5, distance));
                }
            }
            Tool::CircularPattern => {
                let (_, full) = self.circular_step();
                if !full
                    && let Some((r, a0)) = self.circular_arm(sk)
                {
                    let mid = DVec2::from_angle(a0 + self.circular_angle * 0.5);
                    out.push((PatternHandle::CircularAngle, self.pattern_center + mid * (r + px * 34.0), self.circular_angle));
                }
            }
            Tool::Transform if !self.selection.is_empty() => {
                let m = self.pattern_center + self.xf_offset;
                if self.xf_angle.abs() > 1e-12 {
                    out.push((PatternHandle::Rotate, m + DVec2::from_angle(self.xf_angle) * px * 92.0, self.xf_angle));
                }
                if (self.xf_scale - 1.0).abs() > 1e-12 {
                    out.push((PatternHandle::Scale, m + DVec2::from_angle(self.xf_angle + PI * 0.25) * px * (45.0 * self.xf_scale + 22.0), self.xf_scale));
                }
            }
            _ => {}
        }
        out
    }

    /// Set a value typed into an on-canvas pattern / transform label.
    pub fn set_pattern_value(&mut self, h: PatternHandle, v: f64) {
        if !v.is_finite() {
            return;
        }
        match h {
            PatternHandle::LinearStep if v > 0.0 => self.pattern_distance = v,
            PatternHandle::LinearSecondStep if v > 0.0 => self.pattern_second_distance = v,
            PatternHandle::CircularAngle if v.abs() > 1e-9 => self.circular_angle = v.clamp(-TAU, TAU),
            PatternHandle::Rotate => self.xf_angle = v,
            PatternHandle::Scale if v > 0.0 => self.xf_scale = v,
            _ => {}
        }
    }

    /// Set a pattern direction's instance count (typed into its `Nx` label).
    pub fn set_pattern_count(&mut self, h: PatternHandle, n: usize) {
        match h {
            PatternHandle::LinearCount => self.pattern_count = n.clamp(1, 100),
            PatternHandle::LinearSecondCount => self.pattern_second_count = n.clamp(1, 100),
            PatternHandle::CircularCount => self.circular_count = n.clamp(2, 100),
            _ => {}
        }
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
                // Each spacing dragged adds (or removes) an instance.
                if let (Some(c), Some((n0, a0))) = (self.selection_center(sk), self.pattern_grab) {
                    let along = (p - c).dot(DVec2::from_angle(self.pattern_angle));
                    let k = ((along - a0) / self.pattern_distance.max(1e-12)).round() as i64;
                    self.pattern_count = (n0 as i64 + k).clamp(2, 100) as usize;
                }
            }
            PatternHandle::LinearSecondStep => {
                if let Some(c) = self.selection_center(sk) {
                    let d = p - c;
                    if d.length() > px * 3.0 {
                        self.pattern_second_angle = d.to_angle();
                        self.pattern_second_distance = d.length();
                    }
                }
            }
            PatternHandle::LinearSecondCount => {
                if let (Some(c), Some((n0, a0))) = (self.selection_center(sk), self.pattern_grab) {
                    let along = (p - c).dot(DVec2::from_angle(self.pattern_second_angle));
                    let k = ((along - a0) / self.pattern_second_distance.max(1e-12)).round() as i64;
                    self.pattern_second_count = (n0 as i64 + k).clamp(1, 100) as usize;
                }
            }
            PatternHandle::Center => {
                self.pattern_center = self.inference_free_snap(sk, p);
                self.xf_center_pinned = true;
            }
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

    fn toggle_text_selection(&mut self, data: &SketchData, e: EntityId) {
        let box_lines = match data.text_of_entity(e).and_then(|id| data.texts.iter().find(|t| t.id == id)) {
            Some(t) => t.box_lines.clone(),
            None => match data.image_of_entity(e).and_then(|id| data.images.iter().find(|i| i.id == id)) {
                Some(i) => i.box_lines.clone(),
                None => return,
            },
        };
        let lines: Vec<EntityId> = box_lines.iter().map(|l| EntityId(*l)).collect();
        if lines.iter().any(|l| self.selection.contains(l)) {
            self.selection.retain(|x| !lines.contains(x));
        } else {
            self.selection.extend(lines);
        }
    }

    // ------------------------------------------------------------ dimensions

    fn dimension_click(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>) -> EditOutcome {
        // The sketch origin can be dimensioned to (a fixed point is created).
        let hovered = hovered.or_else(|| (p.length() <= self.world_per_px * 9.0).then(|| origin_point(&mut data.sketch)));
        if let Some(e) = hovered
            && !self.picks.iter().any(|(x, _)| *x == e)
        {
            let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
            // Arc length: both end points, then the arc.
            let arc_length = ids.len() == 2 && data.sketch.endpoints(e).is_some_and(|(s, en)| data.sketch.kind(e) == Some(EntityKind::Arc) && ids.contains(&s) && ids.contains(&en));
            if self.picks.len() < 2 || arc_length {
                self.picks.push((e, p));
                return EditOutcome::redraw();
            }
        }
        if self.picks.is_empty() {
            return EditOutcome::none();
        }
        // This click places the dimension and opens its value box.
        let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
        self.picks_placed = std::mem::take(&mut self.picks);
        let picks = std::mem::take(&mut self.picks_placed);
        let Some(kind) = smart_dimension(&data.sketch, &ids, p).map(|k| circle_mode(&data.sketch, k, &picks, self.world_per_px)) else {
            self.message = Some("A dimension cannot be created between these items.".into());
            return EditOutcome::redraw();
        };
        let Some(cid) = add_dimension(data, kind, Some(p), None) else {
            self.message = Some("Could not create the dimension.".into());
            return EditOutcome::redraw();
        };
        if data.sketch.constraint(cid).is_some_and(|c| c.driving) {
            self.open_value_box(data, cid);
        } else {
            self.message = Some("The dimension over-defines the sketch; it was added as a driven dimension".into());
        }
        EditOutcome::changed()
    }

    /// Live preview of the dimension being placed: kind, label position and
    /// displayed value.
    pub fn dimension_preview(&self, data: &SketchData) -> Option<(ConstraintKind, DVec2, f64)> {
        if self.tool != Tool::Dimension || self.picks.is_empty() {
            return None;
        }
        let c = self.cursor?;
        let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
        let kind = circle_mode(&data.sketch, smart_dimension(&data.sketch, &ids, c)?, &self.picks, self.world_per_px);
        let v = data.sketch.measure_new(&kind)? * dim_display_scale(&kind);
        Some((kind, c, v))
    }

    /// Apply a typed dimension value (expression). Returns an error message.
    /// Negative values flip the geometry where Onshape allows it (distances,
    /// angles, line lengths); radii, diameters and arc lengths must be positive.
    pub fn apply_dimension(&mut self, data: &mut SketchData, cid: ConstraintId, text: &str, units: &UnitPrefs, vars: &dyn noshape_units::VariableScope) -> Result<(), String> {
        let Some(c) = data.sketch.constraint(cid).cloned() else { return Err("Dimension no longer exists".into()) };
        // The shape to keep when the new value can't be solved.
        let solved = data.sketch.clone();
        let dim = dim_unit(&c.kind);
        // Stored with the workspace unit (live: "1" reopens as "1 mm", "2*5"
        // as "(2*5) mm"; "1 in" and "#var" stay), so a later change of the
        // workspace units keeps the geometry.
        let text = noshape_model::quantity_text::commit_text(text.trim().trim_start_matches('=').trim(), dim, units, vars);
        let text = text.as_str();
        let q = noshape_units::evaluate_as(text, dim, units, vars).map_err(|e| e.to_string())?;
        let value = q.value / dim_display_scale(&c.kind);
        if !value.is_finite() {
            return Err("Invalid value".into());
        }
        let positive_only = match &c.kind {
            ConstraintKind::Radius { .. } | ConstraintKind::Diameter { .. } | ConstraintKind::EllipseRadius { .. } | ConstraintKind::ArcAngle { .. } | ConstraintKind::Rho { .. } => true,
            ConstraintKind::Length { curve, .. } => data.sketch.kind(*curve) != Some(EntityKind::Line),
            _ => false,
        };
        if value == 0.0 && dim != Dim::ANGLE && !matches!(c.kind, ConstraintKind::Distance { .. } | ConstraintKind::HorizontalDistance { .. } | ConstraintKind::VerticalDistance { .. }) {
            return Err("Value must be positive".into());
        }
        if value < 0.0 {
            if positive_only {
                return Err("This dimension can't be negative".into());
            }
            match &c.kind {
                // A negative line length flips the line about its start point.
                ConstraintKind::Length { curve, .. } => {
                    if let Some((s, e)) = data.sketch.endpoints(*curve)
                        && let (Some(ps), Some(pe)) = (data.sketch.point(s), data.sketch.point(e))
                    {
                        let _ = data.sketch.set_point(e, ps * 2.0 - pe);
                    }
                }
                // Distances and angles: solve to the mirrored side, then keep it.
                _ => {
                    data.sketch.set_dimension_value(cid, value).map_err(|e| e.to_string())?;
                    data.sketch.solve();
                    let _ = data.sketch.recompute_hints(cid);
                }
            }
            data.sketch.set_dimension_value(cid, value.abs()).map_err(|e| e.to_string())?;
            let _ = data.sketch.recompute_hints(cid);
            data.dimension_exprs.remove(&cid.0);
        } else {
            data.sketch.set_dimension_value(cid, value).map_err(|e| e.to_string())?;
            // The text in the shown scale (regeneration divides by
            // `display_scale`), with its unit.
            data.dimension_exprs.insert(cid.0, ExprValue::new(text));
        }
        let opts = data.sketch.dimension_edit_options(cid);
        let report = data.sketch.solve_with(&opts);
        // The next fillet defaults to the radius just typed (live, 568).
        if let Some(ConstraintKind::Radius { curve, value }) = data.sketch.constraint(cid).map(|c| c.kind.clone())
            && self.fillet_session == Some(curve)
            && report.is_solved()
        {
            self.fillet_radius = value;
        }
        if !report.is_solved() {
            // Live (qa5 dim conflict): the value is taken (the label reads it,
            // the sketch turns red) but the last solved shape stays.
            let mut kept = solved;
            let _ = kept.set_dimension_value(cid, data.sketch.constraint(cid).and_then(|c| c.kind.value()).unwrap_or(value.abs()));
            data.sketch = kept;
            return Err(SKETCH_NOT_SOLVED.into());
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
            if let ConstraintKind::Rho { conic, .. } = c.kind
                && self.otf.as_ref().is_some_and(|o| o.fields.iter().any(|f| matches!(f.kind, ConstraintKind::Rho { conic: id, .. } if id == conic)))
            { continue; }
            let Some(pos) = self.label_pos(data, c.id, &c.kind) else { continue };
            let value = dim_value_text_tol(&c.kind, v, units, data.dimension_tolerances.get(&c.id.0));
            let mut text = dimension_label_text(data.dimension_exprs.get(&c.id.0).map(|e| e.text.as_str()), value, self.show_expressions);
            if !c.driving {
                text = format!("({text})");
            }
            out.push((c.id, pos, text, c.driving));
        }
        out
    }

    /// Linear dimensions whose dimension line runs steeper than 60°: live
    /// Onshape writes their (horizontal) text beside the line, which stays
    /// unbroken (sketch/17 "2.5"); flatter ones break the line around it.
    pub fn labels_beside_line(&self, data: &SketchData) -> std::collections::HashSet<ConstraintId> {
        let labels: std::collections::HashMap<ConstraintId, DVec2> = data.sketch.constraints().filter_map(|c| Some((c.id, self.label_pos(data, c.id, &c.kind)?))).collect();
        let mut out = std::collections::HashSet::new();
        for g in self.dimension_graphics(data) {
            let Some(c) = data.sketch.constraint(g.constraint) else { continue };
            if !matches!(c.kind, ConstraintKind::Distance { .. } | ConstraintKind::HorizontalDistance { .. } | ConstraintKind::VerticalDistance { .. } | ConstraintKind::Length { .. }) {
                continue;
            }
            let Some(at) = labels.get(&g.constraint) else { continue };
            // The dimension line: the segment passing closest to the label.
            let line = g.segments.iter().filter(|sg| (sg[1] - sg[0]).length() > self.world_per_px * 2.0).min_by(|a, b| {
                let d = |sg: &[DVec2; 2]| Curve2::Line { a: sg[0], b: sg[1] }.distance(*at);
                d(a).total_cmp(&d(b))
            });
            if let Some(sg) = line {
                let d = sg[1] - sg[0];
                if d.y.abs() > d.x.abs() * 3f64.sqrt() {
                    out.insert(g.constraint);
                }
            }
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
            out.push(DimGraphic { constraint: c.id, driving: c.driving, segments: segs });
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
                let external = c.kind.entities().iter().any(|x| sk.entity(*x).is_some_and(|x| x.external));
                out.push(GlyphInfo { constraint: c.id, icon, entity: e, anchor, on_point: is_point(sk, e), offset, external });
            }
        }
        // Use links: the use glyph on the projected entity and at its ends
        // (live sketch/202).
        for k in data.external_refs.keys() {
            let e = EntityId(*k);
            if sk.entity(e).is_none_or(|x| x.reference_only) || noshape_sketch::text::glyph_text_id(e).is_some() {
                continue;
            }
            let cid = use_glyph_id(e);
            let mut at: Vec<EntityId> = vec![e];
            if let Some((a, b)) = sk.endpoints(e).filter(|(a, b)| a != b) {
                at.extend([a, b]);
            }
            for g in at {
                let Some(anchor) = glyph_anchor(sk, g) else { continue };
                let offset = data.glyph_offsets.get(&glyph_key(cid, g)).map(|o| DVec2::new(o[0], o[1]));
                out.push(GlyphInfo { constraint: cid, icon: "use", entity: g, anchor, on_point: is_point(sk, g), offset, external: true });
            }
        }
        out
    }

    // ------------------------------------------------------------ editing commands

    /// Copy sketch entities: the selection (with internal constraints) as
    /// clipboard JSON.
    pub fn copy_selection(&self, data: &SketchData) -> Option<String> {
        let ids: Vec<EntityId> = self.selection.iter().copied().filter(|e| data.text_of_entity(*e).is_none() && noshape_sketch::text::glyph_text_id(*e).is_none()).collect();
        if ids.is_empty() {
            return None;
        }
        let clip = data.sketch.extract(&ids);
        serde_json::to_string(&serde_json::json!({ "noshape_sketch_entities": clip })).ok()
    }

    /// Paste sketch entities from clipboard JSON; the pasted entities are
    /// selected so they can be dragged or transformed into place.
    pub fn paste_clipboard(&mut self, data: &mut SketchData, text: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return false };
        let Some(clip) = v.get("noshape_sketch_entities").and_then(|c| serde_json::from_value::<Sketch>(c.clone()).ok()) else { return false };
        self.paste_entities(data, &clip, DVec2::ZERO)
    }

    /// Paste sketch entities (copied with [`Sketch::extract`]) moved by
    /// `offset`; the pasted entities become the selection.
    pub fn paste_entities(&mut self, data: &mut SketchData, clip: &Sketch, offset: DVec2) -> bool {
        let created = data.sketch.paste(clip, offset);
        if created.is_empty() {
            return false;
        }
        data.sketch.solve();
        self.selection = created;
        self.message = Some("Pasted; drag or transform the selection into place".into());
        true
    }

    pub fn delete_selection(&mut self, data: &mut SketchData) -> bool {
        let mut changed = false;
        for c in self.selected_constraints.drain(..) {
            if let Some(e) = use_glyph_entity(c) {
                changed |= unlink_used(data, e);
                continue;
            }
            if data.sketch.remove_constraint(c).is_ok() {
                data.dimension_exprs.remove(&c.0);
                data.dimension_labels.remove(&c.0);
                changed = true;
            }
        }
        self.hover_constraint = None;
        if !self.selection.is_empty() {
            let mut ids = std::mem::take(&mut self.selection);
            // Spline tangent handles can't be deleted (Onshape).
            let handles: Vec<EntityId> = data.sketch.entities().filter_map(|e| e.spline_handles()).flat_map(|(a, b)| [a, b]).collect();
            let n = ids.len();
            ids.retain(|e| !handles.contains(e));
            if ids.len() != n {
                self.message = Some("Spline handles cannot be deleted".into());
            }
            if !ids.is_empty() {
                data.sketch.remove_entities(&ids);
                changed = true;
            }
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
        if let Some(e) = use_glyph_entity(cid) {
            if !unlink_used(data, e) {
                return false;
            }
            data.sketch.solve();
            return true;
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
        // Select-first: the constraint applies to the whole selection at once.
        if already_coincident(&data.sketch, c, &self.selection) {
            return false;
        }
        if !self.selection.is_empty() && constraint_applicable(&data.sketch, c, &self.selection) {
            match apply_constraint(&mut data.sketch, c, &self.selection) {
                Ok(n) if n > 0 => {
                    data.sketch.solve();
                    self.selection.clear();
                    return true;
                }
                Ok(_) => {}
                Err(e) => {
                    self.message = Some(format!("Can't add constraint: {e}"));
                    return false;
                }
            }
        }
        // Tool-first: each pair of entities picked is constrained.
        self.set_tool(Tool::Constraint(c));
        false
    }

    // ------------------------------------------------------------ overlay

    /// Preview geometry, inference guides, picks, box selection and dimension
    /// Live values next to the rubber-band preview while drawing (live
    /// Onshape: line length, rectangle width and height, circle `Ø`, arc `R`,
    /// 13 px light blue, no box). Positions are sketch coordinates.
    pub fn preview_values(&self, data: &SketchData, units: &UnitPrefs) -> Vec<(DVec2, String)> {
        let Some(c) = self.snapped().map(|s| s.0) else { return Vec::new() };
        let anchors: Vec<DVec2> = self.clicks.iter().map(|k| k.pos).collect();
        let px = self.world_per_px;
        let e0 = EntityId(0);
        let len = |v: f64| dim_value_text(&ConstraintKind::Length { curve: e0, value: v }, v, units);
        let dia = |v: f64| dim_value_text(&ConstraintKind::Diameter { curve: e0, value: v }, v, units);
        let rad = |v: f64| dim_value_text(&ConstraintKind::Radius { curve: e0, value: v }, v, units);
        // A label beside a segment's midpoint (on the side away from `away`).
        let beside = |a: DVec2, b: DVec2, away: Option<DVec2>| -> DVec2 {
            let m = (a + b) * 0.5;
            let mut n = (b - a).try_normalize().unwrap_or(DVec2::X).perp();
            if let Some(o) = away
                && n.dot(m - o) < 0.0
            {
                n = -n;
            }
            m + n * px * 16.0
        };
        let valid = |v: f64| v > px * 0.5;
        let mut out = Vec::new();
        match self.tool {
            Tool::Line | Tool::TangentArc if self.tangent_mode || self.tool == Tool::TangentArc => {
                if let Some(from) = self.chain_end {
                    let mut tmp = data.sketch.clone();
                    if let Ok(ids) = tmp.tangent_arc(from, c)
                        && let Some(Curve2::Arc { center, radius, .. }) = ids.curves.first().and_then(|e| tmp.curve(*e))
                    {
                        out.push((center + (c - center).try_normalize().unwrap_or(DVec2::X) * radius * 0.5, rad(radius)));
                    }
                }
            }
            Tool::Line => {
                if let Some(a) = self.anchor_pos(&data.sketch)
                    && valid((c - a).length())
                {
                    out.push((beside(a, c, None), len((c - a).length())));
                }
            }
            Tool::MidpointLine => {
                if let Some(m) = anchors.first()
                    && valid((c - *m).length())
                {
                    out.push((beside(*m * 2.0 - c, c, None), len((c - *m).length() * 2.0)));
                }
            }
            Tool::CornerRect | Tool::CenterRect => {
                if let Some(a) = anchors.first() {
                    let (lo, hi) = if self.tool == Tool::CornerRect { (a.min(c), a.max(c)) } else { let d = (c - *a).abs(); (*a - d, *a + d) };
                    let mid = (lo + hi) * 0.5;
                    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
                    // Corner rectangle (live sketch/05): the values sit on the
                    // two edges through the first corner.
                    let (ey, ex) = if self.tool == Tool::CornerRect { (a.y, a.x) } else { (hi.y, hi.x) };
                    if valid(w) {
                        out.push((beside(DVec2::new(lo.x, ey), DVec2::new(hi.x, ey), Some(mid)), len(w)));
                    }
                    if valid(h) {
                        out.push((beside(DVec2::new(ex, lo.y), DVec2::new(ex, hi.y), Some(mid)), len(h)));
                    }
                }
            }
            Tool::AlignedRect => match anchors.len() {
                1 if valid((c - anchors[0]).length()) => out.push((beside(anchors[0], c, None), len((c - anchors[0]).length()))),
                2 => {
                    let (a, b) = (anchors[0], anchors[1]);
                    let n = (b - a).try_normalize().unwrap_or(DVec2::X).perp();
                    let h = (c - a).dot(n);
                    if valid(h.abs()) {
                        out.push((beside(b, b + n * h, Some((a + b) * 0.5 + n * h * 0.5)), len(h.abs())));
                    }
                }
                _ => {}
            },
            Tool::Circle | Tool::Polygon | Tool::CircumPolygon => {
                if let Some(a) = anchors.first() {
                    let r = (anchors.get(1).copied().unwrap_or(c) - *a).length();
                    if valid(r) {
                        // Outside the circle, opposite the cursor (live sketch/09).
                        let label_w = label_width_px(&dia(r * 2.0)) * 0.5 + 6.0;
                        out.push((*a - (c - *a).normalize_or_zero() * (r + px * label_w), dia(r * 2.0)));
                    }
                }
            }
            Tool::ThreePointCircle => match anchors.len() {
                1 if valid((c - anchors[0]).length()) => out.push((beside(anchors[0], c, None), dia((c - anchors[0]).length()))),
                2 => {
                    if let Some((ctr, r)) = noshape_sketch::shapes::circle_through(anchors[0], anchors[1], c) {
                        out.push((ctr + (c - ctr).normalize_or_zero() * r + (c - ctr).normalize_or_zero() * px * 18.0, dia(r * 2.0)));
                    }
                }
                _ => {}
            },
            Tool::Arc3 => {
                if anchors.len() == 2
                    && let Some((ctr, r)) = noshape_sketch::shapes::circle_through(anchors[0], c, anchors[1])
                {
                    out.push((c + (c - ctr).normalize_or_zero() * px * 18.0, rad(r)));
                }
            }
            Tool::CenterArc | Tool::EllipticalArc | Tool::Ellipse if anchors.len() == 1 => {
                let r = (c - anchors[0]).length();
                if valid(r) {
                    let text = if self.tool == Tool::CenterArc { rad(r) } else { dia(r * 2.0) };
                    out.push((beside(anchors[0], c, None), text));
                }
            }
            Tool::CenterArc if anchors.len() == 2 => {
                let r = (anchors[1] - anchors[0]).length();
                out.push((anchors[0] + (c - anchors[0]).normalize_or_zero() * (r + px * 18.0), rad(r)));
            }
            Tool::Ellipse | Tool::EllipticalArc if anchors.len() == 2 => {
                let major = anchors[1] - anchors[0];
                let u = major.try_normalize().unwrap_or(DVec2::X);
                let minor = (c - anchors[0]).dot(u.perp()).abs();
                if valid(minor) {
                    let end = anchors[0] + u.perp() * (c - anchors[0]).dot(u.perp());
                    out.push((beside(anchors[0], end, None), dia(minor * 2.0)));
                }
            }
            _ => {}
        }
        out
    }

    /// graphics (world space).
    pub fn overlay(&self, data: &SketchData, units: &UnitPrefs) -> (LineData, PointData) {
        let mut lines = LineData::default();
        let mut points = PointData::default();
        let frame = self.frame;
        let w = |p: DVec2| -> [f32; 3] {
            let v = frame.to_world(p) + frame.normal * 1e-6;
            [v.x as f32, v.y as f32, v.z as f32]
        };
        let preview_color = PREVIEW_COLOR;
        let guide_color = INFERENCE_COLOR;
        let on_top = LineSegment::ON_TOP;
        // Manipulators drawn wider than `poly`'s 2 px, added after it (in order).
        let mut wide: Vec<(Vec<DVec2>, [u8; 4], f32)> = Vec::new();
        // Construction previews, dash-dot (live sketch/91: the polygon's circle).
        let mut dash_dot: Vec<Vec<DVec2>> = Vec::new();
        let mut poly = |pts: &[DVec2], color: [u8; 4], dashed: bool| {
            let pts3: Vec<[f32; 3]> = pts.iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, color, 0, 2.0, on_top | if dashed { LineSegment::DASHED } else { 0 });
        };
        let circle_pts = |c: DVec2, r: f64, n: usize| -> Vec<DVec2> { (0..=n).map(|i| c + DVec2::from_angle(i as f64 / n as f64 * TAU) * r).collect() };
        let cursor = self.snapped().map(|s| s.0);
        if let Some(c) = cursor {
            let anchors: Vec<DVec2> = self.clicks.iter().map(|k| k.pos).collect();
            match self.tool {
                Tool::Line if self.tangent_mode => {
                    if let Some(from) = self.chain_end
                        && let Some(a) = data.sketch.point(from)
                    {
                        let mut tmp = data.sketch.clone();
                        match tmp.tangent_arc(from, c) {
                            Ok(ids) => poly(&tmp.tessellate(ids.curves[0], self.world_per_px), preview_color, false),
                            Err(_) => poly(&[a, c], preview_color, true),
                        }
                    }
                }
                Tool::Line => {
                    if let Some(a) = self.anchor_pos(&data.sketch) {
                        // A line inferred parallel to another is drawn dotted.
                        let parallel = self.inference.as_ref().is_some_and(|i| i.kind == InferenceKind::Parallel);
                        poly(&[a, c], preview_color, parallel);
                    }
                }
                Tool::MidpointLine => {
                    if let Some(m) = anchors.first() {
                        poly(&[*m * 2.0 - c, c], preview_color, false);
                        points.points.push(PointSprite { position: w(*m), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                    }
                }
                Tool::EllipticalArc => match anchors.len() {
                    1 => {
                        poly(&[anchors[0], c], preview_color, true);
                        poly(&circle_pts(anchors[0], (c - anchors[0]).length(), 72), preview_color, true);
                    }
                    2 | 3 => {
                        let minor_pt = if anchors.len() == 2 { c } else { anchors[2] };
                        if let Some((ctr, major, minor)) = construction_ellipse(anchors[0], anchors[1], minor_pt) {
                            let u = major.normalize();
                            let at = |t: f64| ctr + major * t.cos() + u.perp() * minor * t.sin();
                            let pts: Vec<DVec2> = (0..=72).map(|i| at(i as f64 / 72.0 * TAU)).collect();
                            poly(&pts, preview_color, true);
                            if anchors.len() == 3 {
                                let t0 = noshape_sketch::sketch::ellipse_param(ctr, major, minor, anchors[2]);
                                let mut t1 = noshape_sketch::sketch::ellipse_param(ctr, major, minor, c);
                                while t1 <= t0 {
                                    t1 += TAU;
                                }
                                let arc: Vec<DVec2> = (0..=64).map(|i| at(t0 + (t1 - t0) * i as f64 / 64.0)).collect();
                                poly(&arc, preview_color, false);
                            }
                        }
                    }
                    _ => {}
                },
                Tool::CornerRect => {
                    if let Some(a) = anchors.first() {
                        poly(&[*a, DVec2::new(c.x, a.y), c, DVec2::new(a.x, c.y), *a], preview_color, false);
                        // Live (sketch/05): every corner shows its point.
                        for q in [*a, DVec2::new(c.x, a.y), DVec2::new(a.x, c.y)] {
                            points.points.push(PointSprite { position: w(q), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                        }
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
                    // Live (sketch/09): the circle and its center point; no
                    // radius line and no point at the cursor.
                    if let Some(a) = anchors.first() {
                        poly(&circle_pts(*a, (c - *a).length(), 72), preview_color, false);
                        points.points.push(PointSprite { position: w(*a), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                    }
                }
                Tool::Polygon | Tool::CircumPolygon => {
                    if let Some(a) = anchors.first() {
                        // After the circumference is locked the cursor sets the side count.
                        let c = anchors.get(1).copied().unwrap_or(c);
                        let r = (c - *a).length();
                        let n = self.polygon_sides.clamp(3, 50);
                        let start = (c - *a).to_angle();
                        // Inscribed: vertices on the circle; circumscribed: edge midpoints on it.
                        let (rv, off) = if self.tool == Tool::Polygon { (r, 0.0) } else { (r / (PI / n as f64).cos(), PI / n as f64) };
                        let pts: Vec<DVec2> = (0..=n).map(|i| *a + DVec2::from_angle(start + off + i as f64 / n as f64 * TAU) * rv).collect();
                        poly(&pts, preview_color, false);
                        // Live (sketch/91): the construction circle is dash-dot.
                        dash_dot.push(circle_pts(*a, r, 72));
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
                        // The sweep follows the direction the cursor moved around the center.
                        let ccw = noshape_sketch::geom::ccw_sweep(a0, (c - ctr).to_angle());
                        let sweep = if self.center_arc_sweep >= 0.0 { ccw } else { ccw - TAU };
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
                Tool::InsertImage => {
                    if let (Some(a), Some((_, _, aspect))) = (anchors.first(), &self.pending_image) {
                        let (o, h) = image_box(*a, c, *aspect);
                        let wdt = h * aspect;
                        poly(&[o, o + DVec2::X * wdt, o + DVec2::new(wdt, h), o + DVec2::Y * h, o], preview_color, true);
                    }
                }
                Tool::Text => {
                    let (origin, h) = self.pending_text.unwrap_or_else(|| match anchors.first() {
                        Some(a) => self.text_box_from(*a, c),
                        None => (c, self.text_height),
                    });
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
            if self.tool.draws() && !(self.tool == Tool::Circle && !self.clicks.is_empty()) {
                points.points.push(PointSprite { position: w(c), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
            }
        }
        // Use: preview of the projection of the hovered edges.
        if self.tool == Tool::Use {
            for pl in &self.use_preview {
                poly(pl, preview_color, false);
            }
        }
        // Offset: preview of the pending offset and its direction arrow.
        if self.tool == Tool::Offset {
            if let Some((tmp, created)) = self.offset_result(&data.sketch) {
                for e in created {
                    poly(&tmp.tessellate(e, self.world_per_px), preview_color, false);
                }
            }
            if let Some((start, apex)) = self.offset_arrow(&data.sketch) {
                // Onshape's hollow arrow (as the Part Studio manipulators): a
                // light grey 3 px outline under a 1.5 px white line.
                let d = (apex - start).try_normalize().unwrap_or(DVec2::X);
                push_hollow_arrow(&mut wide, self.snap_to_pixel(start), d, self.world_per_px, OFFSET_ARROW_STEM_PX);
            }
        }
        // Fillet: preview at the hovered corner (red when it doesn't fit) and the
        // radius manipulator of the session's first fillet.
        if self.tool == Tool::Fillet {
            if let Some((arc, ok)) = self.fillet_preview(&data.sketch) {
                poly(&arc, if ok { preview_color } else { crate::palette::rgb8(crate::palette::pick(crate::palette::viewport::SKETCH_OVER, crate::palette::viewport_dark::SKETCH_OVER)) }, false);
            }
            // Live (sketch/1934): the radius manipulator is the hollow drag
            // arrow from the arc's midpoint toward its centre; the "R" label
            // has a short leader down to the arc's upper tangent point.
            if let Some((start, apex)) = self.fillet_arrow(&data.sketch) {
                let d = (apex - start).try_normalize().unwrap_or(DVec2::X);
                push_hollow_arrow(&mut wide, self.snap_to_pixel(start), d, self.world_per_px, FILLET_ARROW_STEM_PX);
            }
            if let Some(pa) = self.fillet_label_foot(&data.sketch) {
                let px = self.world_per_px;
                wide.push((vec![pa + DVec2::new(-4.0 * px, 6.0 * px), pa + DVec2::new(-4.0 * px, 0.0), pa], [0x33, 0x33, 0x33, 255], 1.5));
            }
        }
        // Polygon side spinner (live sketch/91): filled black ◀ / ▶ and the
        // count laid along the ray (kept readable: never upside down).
        if let Some((at, u, sides)) = self.polygon_side_control() {
            let reading = if u.x < 0.0 { -u } else { u };
            for sg in hatched_text(&sides.to_string(), at, reading, self.world_per_px, 16.0) {
                wide.push((vec![sg[0], sg[1]], [0, 0, 0, 255], 1.0));
            }
            let px = self.world_per_px;
            for k in [-1.0, 1.0] {
                let tip = at + u * px * (POLYGON_SPINNER_ARROW_PX + 5.0) * k;
                let base = tip - u * px * 10.0 * k;
                for i in 0..=10 {
                    let t = i as f64 / 10.0 * 2.0 - 1.0;
                    wide.push((vec![tip, base + u.perp() * px * 5.0 * t], [0x33, 0x33, 0x33, 255], 1.0));
                }
            }
        }
        let mut dim_preview: Vec<[DVec2; 2]> = Vec::new();
        // Chamfer: the pending chamfer line in light blue, its two distance
        // dimensions and the distance arrow (live sketch/1959–1960).
        if self.tool == Tool::Chamfer
            && let Some((pd, result)) = &self.pending_corner
        {
            for id in &result.created {
                if pd.sketch.kind(*id) == Some(EntityKind::Line) && !pd.sketch.entity(*id).is_some_and(|e| e.construction) {
                    poly(&pd.sketch.tessellate(*id, self.world_per_px), preview_color, false);
                }
            }
            let dims = chamfer_dims(&pd.sketch, result);
            for (cid, at) in chamfer_dim_labels(&pd.sketch, &dims, self.world_per_px) {
                let Some(c) = pd.sketch.constraint(cid) else { continue };
                let mut segs = Vec::new();
                dim_segments(&pd.sketch, &c.kind, at, self.world_per_px, &mut segs);
                let text = dim_value_text(&c.kind, c.kind.value().unwrap_or(0.0) / dim_display_scale(&c.kind), units);
                let half = DVec2::new(label_width_px(&text) * 0.5 + 3.0, 10.0) * self.world_per_px;
                dim_preview.extend(segs.iter().flat_map(|sg| clip_outside_box(*sg, at, half)));
            }
            if let Some((start, apex)) = self.chamfer_arrow() {
                let d = (apex - start).try_normalize().unwrap_or(DVec2::X);
                push_hollow_arrow(&mut wide, start, d, self.world_per_px, FILLET_ARROW_STEM_PX);
            }
        }
        // Slot preview (light blue until committed), with its join points
        // and the width dimension (live sketch/304: "Ø20" with a leader).
        if let Some((pd, created)) = &self.pending_slot {
            for id in created {
                if pd.sketch.curve(*id).is_some() {
                    poly(&pd.sketch.tessellate(*id, self.world_per_px * 0.2), preview_color, false);
                } else if let Some(q) = pd.sketch.point(*id) {
                    points.points.push(PointSprite { position: w(q), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                }
                // The cap / side joins (live sketch/1952: four light-blue dots).
                if pd.sketch.kind(*id) == Some(EntityKind::Line)
                    && let Some((a, b)) = pd.sketch.endpoints(*id)
                {
                    for q in [a, b].into_iter().filter_map(|e| pd.sketch.point(e)) {
                        points.points.push(PointSprite { position: w(q), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                    }
                }
            }
            if let Some((cid, at, _)) = self.slot_dimension()
                && let Some(c) = pd.sketch.constraint(cid)
            {
                let mut segs = Vec::new();
                dim_segments(&pd.sketch, &c.kind, at, self.world_per_px, &mut segs);
                let text = dim_value_text(&c.kind, c.kind.value().unwrap_or(0.0) / dim_display_scale(&c.kind), units);
                let half = DVec2::new(label_width_px(&text) * 0.5 + 3.0, 10.0) * self.world_per_px;
                dim_preview.extend(segs.iter().flat_map(|sg| clip_outside_box(*sg, at, half)));
            }
        }
        // Trim drag path.
        if let Some(DragState::TrimPath { points: path }) = &self.drag
            && path.len() > 1
        {
            poly(path, crate::palette::rgb8(0x555555), false);
        }
        // Dimension being placed: drawn like a placed one (live sketch/12:
        // black, the line broken around the 16 px value at the cursor).
        if let Some((kind, pos, v)) = self.dimension_preview(data) {
            let mut segs = Vec::new();
            dim_segments(&data.sketch, &kind, pos, self.world_per_px, &mut segs);
            let text = dim_value_text(&kind, v / dim_display_scale(&kind), units);
            let half = DVec2::new(label_width_px(&text) * 0.5 + 3.0, 10.0) * self.world_per_px;
            dim_preview.extend(segs.iter().flat_map(|sg| clip_outside_box(*sg, pos, half)));
        }
        // Trim: the hovered curve's piece between its intersections, thick
        // light orange with its end dots (live, qa5 sketch tools).
        if self.tool == Tool::Trim
            && self.drag.is_none()
            && let (Some(h), Some(c)) = (self.hover, self.cursor)
            && let Some(curve) = data.sketch.curve(h)
            && let Some((t0, t1)) = data.sketch.trim_span(h, c)
        {
            let n = 64;
            let (a, b) = curve.domain();
            let period = b - a;
            let at = |t: f64| curve.point(if curve.is_periodic() && t > b { t - period } else { t });
            let pts: Vec<DVec2> = (0..=n).map(|i| at(t0 + (t1 - t0) * i as f64 / n as f64)).collect();
            let color = crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT);
            wide.push((pts, color, 4.0));
            for q in [at(t0), at(t1)] {
                points.points.push(PointSprite { position: w(q), color, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
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
            // Onshape SPLINE_HANDLE_COLOR.
            let handle_color = [0x80, 0x80, 0x80, 255];
            let active = match &self.drag {
                Some(DragState::Handle(h)) => Some(*h),
                _ => None,
            };
            let sel_c = self.selection_center(&data.sketch);
            for (h, q) in self.pattern_handles(&data.sketch) {
                match h {
                    PatternHandle::LinearStep | PatternHandle::LinearSecondStep => {
                        // Live (sketch/136): hollow squares at the seed and the first
                        // instance, a light-blue dash-dot guide between them, the
                        // count arrow after the square, the "3x" label's leader and
                        // the spacing dimension with extension lines.
                        if let Some(c) = sel_c {
                            let second = h == PatternHandle::LinearSecondStep;
                            let (dir, _, count, diag) = self.linear_direction(second);
                            if !second {
                                push_hollow_square(&mut wide, self.snap_to_pixel(c), px, PATTERN_SQUARE_PX);
                            }
                            let qs = self.snap_to_pixel(q);
                            push_hollow_square(&mut wide, qs, px, PATTERN_SQUARE_PX);
                            push_hollow_arrow(&mut wide, qs + dir * px * PATTERN_SQUARE_PX * 0.5, dir, px, PATTERN_ARROW_STEM_PX);
                            let ink = [0x33, 0x33, 0x33, 255];
                            let mut segs = Vec::new();
                            // The leader runs into the square, its filled ~8 x 5 px arrowhead
                            // tip just off the centre (live sketch/136), 1.5 px.
                            let along = diag.normalize_or_zero();
                            let tip = q + along * px * 3.0;
                            let lead_end = q + diag * px * (PATTERN_LABEL_PX - 13.0);
                            wide.push((vec![lead_end, tip], ink, 1.5));
                            for i in 0..=16 {
                                let t = i as f64 / 16.0 * 2.0 - 1.0;
                                wide.push((vec![tip, tip + along * px * 8.0 + along.perp() * px * 2.5 * t], ink, 1.0));
                            }
                            if count >= 2 {
                                wide.push((vec![c, q], preview_color, -1.0));
                                let (f1, f2) = self.linear_spacing_feet(c, second);
                                segs.push([c, f1]);
                                segs.push([q, f2]);
                                segs.push([f1, f2]);
                                let d = (f2 - f1).try_normalize().unwrap_or(dir);
                                arrow(&mut segs, f1, -d, px);
                                arrow(&mut segs, f2, d, px);
                            }
                            for sg in segs {
                                wide.push((vec![sg[0], sg[1]], ink, 1.0));
                            }
                        }
                    }
                    PatternHandle::LinearCount | PatternHandle::LinearSecondCount => {}
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
                if matches!(h, PatternHandle::LinearStep | PatternHandle::LinearSecondStep | PatternHandle::LinearCount | PatternHandle::LinearSecondCount) {
                    continue;
                }
                let size = if active == Some(h) { 13.0 } else { 10.0 };
                points.points.push(PointSprite { position: w(q), color: if active == Some(h) { guide_color } else { handle_color }, pick_id: 0, size, style: on_top, _pad: 0 });
            }
        }
        // Related entities (constraint/dimension tool picks, mirror axis, entities
        // of the selected/hovered constraint): Onshape's yellow highlight.
        // (The Dimension tool's picks show selected instead, live sketch/12.)
        let mut related = self.picked(Some(data));
        if self.tool == Tool::Dimension {
            let picks = self.tool_picks();
            related.retain(|e| !picks.contains(e));
        }
        // A hovered glyph (live sketch/27, 1956, 2203–2217): its constrained
        // entities turn pre-highlight orange; `constraint_masters` secondary yellow.
        let hover_roles: Vec<(EntityId, bool)> = self
            .hover_constraint
            .filter(|c| !self.selected_constraints.contains(c) && use_glyph_entity(*c).is_none())
            .and_then(|c| data.sketch.constraint(c))
            .map(|c| {
                let masters = constraint_masters(&c.kind, &data.sketch);
                let mut roles: Vec<(EntityId, bool)> = c.kind.entities().into_iter().filter(|e| !masters.contains(e)).map(|e| (e, false)).collect();
                roles.extend(masters.into_iter().map(|e| (e, true)));
                roles
            })
            .unwrap_or_default();
        if !hover_roles.is_empty() {
            let selected_entities: Vec<EntityId> = self.selected_constraints.iter().filter_map(|c| data.sketch.constraint(*c)).flat_map(|c| c.kind.entities()).collect();
            let picks = self.tool_picks();
            related.retain(|e| !hover_roles.iter().any(|(h, _)| h == e) || picks.contains(e) || selected_entities.contains(e) || self.mirror_axis == Some(*e));
            for (e, master) in &hover_roles {
                let color = if *master { [0xf2, 0xff, 0x80, 255] } else { crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT) };
                if is_point(&data.sketch, *e) {
                    if let Some(p) = data.sketch.point(*e) {
                        points.points.push(PointSprite { position: w(p), color, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
                    }
                } else {
                    let pts3: Vec<[f32; 3]> = data.sketch.tessellate(*e, self.world_per_px * 0.5).iter().map(|p| w(*p)).collect();
                    lines.push_polyline(&pts3, color, 0, 3.0, on_top);
                }
            }
        }
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
        // Spline tangent handles: white points joined to the spline ends.
        for e in data.sketch.entities() {
            let Some((h0, h1)) = e.spline_handles() else { continue };
            let Some((s, en)) = e.endpoints() else { continue };
            for (end, h) in [(s, h0), (en, h1)] {
                if let (Some(a), Some(b)) = (data.sketch.point(end), data.sketch.point(h)) {
                    lines.push_polyline(&[w(a), w(b)], [0x80, 0x80, 0x80, 255], 0, 1.0, on_top);
                    points.points.push(PointSprite { position: w(b), color: [0x60, 0x60, 0x60, 255], pick_id: 0, size: 10.0, style: on_top, _pad: 0 });
                    points.points.push(PointSprite { position: w(b), color: [0xff, 0xff, 0xff, 255], pick_id: 0, size: 7.0, style: on_top, _pad: 0 });
                }
            }
        }
        // Control polygons of selected/hovered B-splines (Bezier editing).
        for e in self.selection.iter().chain(self.hover.iter()) {
            if let Some(noshape_sketch::Entity { geometry: noshape_sketch::Geometry::BSpline { control, .. }, .. }) = data.sketch.entity(*e) {
                let pts3: Vec<[f32; 3]> = control.iter().filter_map(|c| data.sketch.point(*c)).map(w).collect();
                lines.push_polyline(&pts3, [0x90, 0x90, 0x90, 255], 0, 1.0, on_top | LineSegment::DASHED);
            }
        }
        // The entity a parallel / perpendicular / tangent inference relates to
        // is highlighted orange (Onshape).
        if let Some(inf) = &self.inference
            && matches!(inf.kind, InferenceKind::Parallel | InferenceKind::Perpendicular | InferenceKind::Tangent)
            && let Some(e) = inf.entity.filter(|e| data.sketch.entity(*e).is_some())
        {
            let pts3: Vec<[f32; 3]> = data.sketch.tessellate(e, self.world_per_px * 0.5).iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT), 0, 4.0, on_top);
        }
        if let Some(inf) = &self.inference
            && let Some(r) = inf.reference
        {
            let pts3 = [w(r), w(inf.position)];
            lines.push_polyline(&pts3, guide_color, 0, 1.0, on_top | LineSegment::DASHED);
        }
        if let Some(inf) = &self.inference {
            points.points.push(PointSprite { position: w(inf.position), color: guide_color, pick_id: 0, size: 8.0, style: on_top, _pad: 0 });
        }
        if let Some((_, q, _)) = self.drag_snap {
            points.points.push(PointSprite { position: w(q), color: guide_color, pick_id: 0, size: 10.0, style: on_top, _pad: 0 });
        }
        // Profile inspector issues: red markers, the focused one larger with its curves.
        let issue_red = [0xbd, 0x30, 0x39, 255];
        for q in &self.issue_markers {
            points.points.push(PointSprite { position: w(*q), color: issue_red, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
        }
        if let Some((q, ents)) = &self.issue_highlight {
            for e in ents {
                let pts3: Vec<[f32; 3]> = data.sketch.tessellate(*e, self.world_per_px * 0.5).iter().map(|p| w(*p)).collect();
                lines.push_polyline(&pts3, [0xf2, 0xff, 0x80, 200], 0, 5.0, on_top);
            }
            points.points.push(PointSprite { position: w(*q), color: [0xf2, 0xe0, 0x30, 255], pick_id: 0, size: 14.0, style: on_top, _pad: 0 });
        }
        // Box selection rectangle.
        if let (Some(DragState::Box { start }), Some(end)) = (&self.drag, self.box_end) {
            // Onshape box select: crossing #ffdc74 (right to left), window #789ad1.
            let color = if end.x < start.x { [0xff, 0xdc, 0x74, 255] } else { [0x78, 0x9a, 0xd1, 255] };
            let a = *start;
            let b = end;
            let pts3: Vec<[f32; 3]> = [a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y), a].iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, color, 0, 1.0, on_top | if end.x < start.x { LineSegment::DASHED } else { 0 });
        }
        // The sketch frame.
        {
            let (lo, hi) = self.sketch_box();
            let pts3: Vec<[f32; 3]> = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y), lo].iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, crate::palette::rgb8(SKETCH_BOX_COLOR), 0, 1.0, 0);
        }
        for sg in &dim_preview {
            lines.segments.push(LineSegment { a: w(sg[0]), b: w(sg[1]), color: crate::palette::rgb8(crate::palette::pick(crate::palette::viewport::SKETCH_FULL, crate::palette::viewport_dark::SKETCH_FULL)), pick_id: 0, width: 1.0, style: on_top });
        }
        // Dimension graphics. The dimension line is broken around its value
        // (Onshape draws no box behind the text).
        let gaps: std::collections::HashMap<ConstraintId, (DVec2, DVec2)> = self
            .dimension_labels(data, units)
            .into_iter()
            .map(|(cid, pos, text, _)| (cid, (pos, DVec2::new(label_width_px(&text) * 0.5 + 3.0, 10.0) * self.world_per_px)))
            .collect();
        let beside = self.labels_beside_line(data);
        for mut g in self.dimension_graphics(data) {
            if let Some((c, half)) = gaps.get(&g.constraint).filter(|_| !beside.contains(&g.constraint)) {
                g.segments = g.segments.iter().flat_map(|sg| clip_outside_box(*sg, *c, *half)).collect();
            }
            let selected = self.selected_constraints.contains(&g.constraint);
            let hovered = self.hover_constraint == Some(g.constraint);
            // Onshape dimension colors: black driving, gray driven, gold hover/selection.
            use crate::palette::{rgb8, viewport as vc};
            let color = if selected {
                rgb8(0x6a4f06)
            } else if hovered {
                rgb8(0x987107)
            } else if self.conflicting.contains(&g.constraint) {
                rgb8(vc::SKETCH_OVER)
            } else if g.driving {
                rgb8(vc::SKETCH_FULL)
            } else {
                rgb8(vc::DIM_DRIVEN)
            };
            let width = if selected { 2.0 } else { 1.0 };
            for s in &g.segments {
                lines.segments.push(LineSegment { a: w(s[0]), b: w(s[1]), color, pick_id: 0, width, style: on_top });
            }
        }
        for (pts, color, width) in wide {
            let pts3: Vec<[f32; 3]> = pts.iter().map(|p| w(*p)).collect();
            // A negative width marks a 2 px dash-dot guide.
            let (width, style) = if width < 0.0 { (2.0, on_top | LineSegment::DASH_DOT) } else { (width, on_top) };
            lines.push_polyline(&pts3, color, 0, width, style);
        }
        for pts in dash_dot {
            let pts3: Vec<[f32; 3]> = pts.iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, preview_color, 0, 2.0, on_top | LineSegment::DASH_DOT);
        }
        (lines, points)
    }

    /// Entities highlighted as related (picks, mirror axis, entities of the
    /// selected / hovered constraints).
    /// Entities the active tool has picked so far (e.g. the Dimension
    /// tool's first entity).
    pub fn tool_picks(&self) -> Vec<EntityId> {
        self.picks.iter().map(|p| p.0).collect()
    }

    pub fn picked(&self, data: Option<&SketchData>) -> Vec<EntityId> {
        let mut v: Vec<EntityId> = self.picks.iter().map(|p| p.0).collect();
        v.extend(self.mirror_axis);
        if let Some(data) = data {
            for cid in self.selected_constraints.iter().chain(self.hover_constraint.iter()) {
                if let Some(e) = use_glyph_entity(*cid) {
                    v.push(e);
                } else if let Some(c) = data.sketch.constraint(*cid) {
                    v.extend(c.kind.entities());
                }
            }
        }
        v
    }
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

/// Apply a click's inferred constraints to a new point `pt` of `curve`.
/// A snapped existing point is merged with it. Returns the surviving point.
fn attach(sk: &mut Sketch, pt: EntityId, curve: Option<EntityId>, click: &Click) -> EntityId {
    let Some(inf) = &click.inference else { return pt };
    let mut out = pt;
    for a in &inf.constraints {
        match *a {
            AutoConstraint::CoincidentPoint(q) if q != pt => {
                // Don't collapse the new curve onto itself.
                let own = curve.and_then(|c| sk.entity(c)).is_some_and(|e| e.point_refs().contains(&q));
                if own {
                    continue;
                }
                if sk.merge_points(pt, q).is_ok() {
                    out = q;
                }
            }
            AutoConstraint::CoincidentPoint(_) => {}
            other => {
                if let Some(kind) = other.to_constraint(out, curve) {
                    let _ = sk.add_constraint(kind);
                }
            }
        }
    }
    out
}

/// A click on a circle's rim that snapped to a point or curve: coincident / tangent.
fn rim_autos(sk: &mut Sketch, circle: EntityId, click: &Click) {
    let Some(inf) = &click.inference else { return };
    match inf.constraints.first() {
        Some(AutoConstraint::CoincidentPoint(pt)) => {
            let _ = sk.add_constraint(ConstraintKind::Coincident { a: *pt, b: circle });
        }
        Some(AutoConstraint::MidpointOf(c)) => {
            // Rim through a line's midpoint: a point at the midpoint on the circle.
            if let Some(m) = curve_mid(sk, *c) {
                let q = sk.add_point(m);
                let _ = sk.add_constraint(ConstraintKind::Midpoint { point: q, curve: *c });
                let _ = sk.add_constraint(ConstraintKind::Coincident { a: q, b: circle });
            }
        }
        _ => {}
    }
}

fn length_field(sk: &Sketch, line: EntityId, wpp: f64) -> OtfField {
    let label = match sk.curve(line) {
        // Closed shapes run counter-clockwise: the label goes outside (right of the edge).
        Some(Curve2::Line { a, b }) => (a + b) * 0.5 - (b - a).try_normalize().unwrap_or(DVec2::X).perp() * wpp * 26.0,
        _ => DVec2::ZERO,
    };
    OtfField { kind: ConstraintKind::Length { curve: line, value: 0.0 }, scale: 1.0, label }
}

fn radius_field(sk: &Sketch, arc: EntityId, wpp: f64) -> OtfField {
    let label = default_label_pos(sk, &ConstraintKind::Radius { curve: arc, value: 0.0 }, wpp).unwrap_or_default();
    OtfField { kind: ConstraintKind::Radius { curve: arc, value: 0.0 }, scale: 1.0, label }
}

fn diameter_field(sk: &Sketch, circle: EntityId, wpp: f64) -> OtfField {
    let label = default_label_pos(sk, &ConstraintKind::Diameter { curve: circle, value: 0.0 }, wpp).unwrap_or_default();
    OtfField { kind: ConstraintKind::Diameter { curve: circle, value: 0.0 }, scale: 1.0, label }
}

/// Primary then secondary axis diameter boxes of an ellipse / elliptical arc.
fn ellipse_fields(sk: &Sketch, id: EntityId, wpp: f64) -> Vec<OtfField> {
    let Some(Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. }) = sk.curve(id) else { return Vec::new() };
    let u = major.try_normalize().unwrap_or(DVec2::X);
    let off = wpp * 26.0;
    vec![
        OtfField { kind: ConstraintKind::EllipseRadius { ellipse: id, major: true, value: 0.0 }, scale: 2.0, label: center + major + u * off },
        OtfField { kind: ConstraintKind::EllipseRadius { ellipse: id, major: false, value: 0.0 }, scale: 2.0, label: center + u.perp() * (minor + off) },
    ]
}

/// Construction ellipse of the elliptical arc tool: center, primary axis end
/// and a point fixing the secondary radius.
fn construction_ellipse(c: DVec2, major_end: DVec2, minor_pt: DVec2) -> Option<(DVec2, DVec2, f64)> {
    let major = major_end - c;
    let u = major.try_normalize()?;
    let minor = u.perp().dot(minor_pt - c).abs();
    // A point on the ellipse through `minor_pt` would need both axes; the
    // click's distance across the axis sets the secondary radius.
    (minor > 1e-12).then_some((c, major, minor))
}

/// Third point of an aligned square on the cursor's side of the first edge.
fn aligned_square(a: DVec2, b: DVec2, c: DVec2) -> DVec2 {
    let d = b - a;
    let n = d.perp();
    let side = if (c - a).dot(n) < 0.0 { -1.0 } else { 1.0 };
    b + n * side
}

/// Does entity `id` cross (or lie in) the box `lo`-`hi`?
fn crosses_box(sk: &Sketch, id: EntityId, lo: DVec2, hi: DVec2, tol: f64) -> bool {
    let inside = |p: DVec2| p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y;
    if let Some(p) = sk.point(id) {
        return inside(p);
    }
    let pts = sk.tessellate(id, tol);
    let corners = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
    let seg_x = |p: DVec2, q: DVec2, a: DVec2, b: DVec2| {
        let (d1, d2) = (q - p, b - a);
        let den = d1.perp_dot(d2);
        if den.abs() < 1e-300 {
            return false;
        }
        let t = (a - p).perp_dot(d2) / den;
        let u = (a - p).perp_dot(d1) / den;
        (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
    };
    pts.iter().any(|p| inside(*p)) || pts.windows(2).any(|w| (0..4).any(|k| seg_x(w[0], w[1], corners[k], corners[(k + 1) % 4])))
}

/// The fixed point on the sketch origin (created on first use).
pub fn origin_point(sk: &mut Sketch) -> EntityId {
    let existing = sk.entities().find(|e| e.external && matches!(e.geometry, noshape_sketch::Geometry::Point { pos } if pos.length() < 1e-12)).map(|e| e.id);
    existing.unwrap_or_else(|| sk.use_external_point(DVec2::ZERO))
}

/// Entity an automatic constraint refers to.
fn auto_target(c: &AutoConstraint) -> Option<EntityId> {
    use AutoConstraint::*;
    match *c {
        CoincidentPoint(e) | OnCurve(e) | MidpointOf(e) | HorizontalWith(e) | VerticalWith(e) | PerpendicularTo(e) | ParallelTo(e) | TangentTo(e) | QuadrantOf(e, _) => Some(e),
        HorizontalLine | VerticalLine => None,
    }
}

fn retarget_auto(c: AutoConstraint, from: EntityId, to: EntityId) -> AutoConstraint {
    use AutoConstraint::*;
    let m = |e: EntityId| if e == from { to } else { e };
    match c {
        CoincidentPoint(e) => CoincidentPoint(m(e)),
        OnCurve(e) => OnCurve(m(e)),
        MidpointOf(e) => MidpointOf(m(e)),
        HorizontalWith(e) => HorizontalWith(m(e)),
        VerticalWith(e) => VerticalWith(m(e)),
        PerpendicularTo(e) => PerpendicularTo(m(e)),
        ParallelTo(e) => ParallelTo(m(e)),
        TangentTo(e) => TangentTo(m(e)),
        QuadrantOf(e, major) => QuadrantOf(m(e), major),
        other => other,
    }
}

/// Constrain a linear pattern's instances to their seeds (Onshape's linear
/// pattern constraint): every instance point = seed point + k * step, the
/// step's length is a spacing dimension and an axis-aligned step keeps its
/// direction. Instance points are found by position (seed + k * step).
fn linear_grid_pattern(data: &mut SketchData, seeds: &[EntityId], x: (DVec2, usize), y: (DVec2, usize), wpp: f64) -> Result<Vec<Vec<EntityId>>, noshape_sketch::SketchError> {
    let mut groups = data.sketch.linear_pattern_groups(seeds, x.0, x.1)?;
    link_linear_pattern(data, seeds, &groups, x.0, wpp);
    if y.1 > 1 {
        let mut row = seeds.to_vec();
        row.extend(groups.iter().flatten().copied());
        let rows = data.sketch.linear_pattern_groups(&row, y.0, y.1)?;
        link_linear_pattern(data, &row, &rows, y.0, wpp);
        groups.extend(rows);
    }
    Ok(groups)
}

fn link_linear_pattern(data: &mut SketchData, seeds: &[EntityId], groups: &[Vec<EntityId>], step: DVec2, wpp: f64) {
    let sk = &mut data.sketch;
    let seed_points: Vec<EntityId> = seeds
        .iter()
        .flat_map(|e| if is_point(sk, *e) { vec![*e] } else { sk.entity(*e).map(|x| x.point_refs().to_vec()).unwrap_or_default() })
        .fold(Vec::new(), |mut acc, p| {
            if !acc.contains(&p) && sk.entity(p).is_some_and(|x| !x.external) {
                acc.push(p);
            }
            acc
        });
    let tol = step.length().max(1e-9) * 1e-7;
    let find = |sk: &Sketch, g: &[EntityId], at: DVec2| g.iter().copied().find(|e| is_point(sk, *e) && sk.point(*e).is_some_and(|q| (q - at).length() <= tol));
    let Some(&s0) = seed_points.first() else { return };
    let Some(p0) = sk.point(s0) else { return };
    let Some(first0) = groups.first().and_then(|g| find(sk, g, p0 + step)) else { return };
    for (k, g) in groups.iter().enumerate() {
        let kk = (k + 1) as f64;
        for (j, seed) in seed_points.iter().enumerate() {
            if k == 0 && j == 0 {
                continue;
            }
            let Some(ps) = sk.point(*seed) else { continue };
            if let Some(inst) = find(sk, g, ps + step * kk) {
                let _ = sk.add_constraint(ConstraintKind::PatternLink { seed: *seed, inst, ref_seed: s0, ref_inst: first0, k: kk });
            }
        }
        // Circles keep the seed's radius.
        let circles: Vec<EntityId> = seeds.iter().copied().filter(|e| matches!(sk.kind(*e), Some(EntityKind::Circle))).collect();
        for seed in &circles {
            let Some(Curve2::Circle { center, .. }) = sk.curve(*seed) else { continue };
            let target = center + step * kk;
            let inst = g.iter().copied().find(|e| matches!(sk.curve(*e), Some(Curve2::Circle { center: c, .. }) if (c - target).length() <= tol));
            if let Some(inst) = inst {
                let _ = sk.add_constraint(ConstraintKind::Equal { a: *seed, b: inst });
            }
        }
    }
    // Spacing dimension and direction.
    if step.y.abs() < 1e-12 {
        let _ = sk.add_constraint(ConstraintKind::Horizontal { a: s0, b: Some(first0) });
    } else if step.x.abs() < 1e-12 {
        let _ = sk.add_constraint(ConstraintKind::Vertical { a: s0, b: Some(first0) });
    }
    let label = p0 + step * 0.5 + step.try_normalize().unwrap_or(DVec2::X).perp() * wpp * 24.0;
    add_dimension(data, ConstraintKind::Distance { a: s0, b: first0, value: step.length(), mode: DistanceMode::Center }, Some(label), None);
}

/// Lower-left corner and height of an image rectangle dragged from `a` to
/// `b` with its aspect ratio (width / height) locked.
fn image_box(a: DVec2, b: DVec2, aspect: f64) -> (DVec2, f64) {
    let d = b - a;
    let h = d.y.abs().max(d.x.abs() / aspect.max(1e-9));
    let w = h * aspect;
    let x = if d.x < 0.0 { a.x - w } else { a.x };
    let y = if d.y < 0.0 { a.y - h } else { a.y };
    (DVec2::new(x, y), h)
}

/// The Insert DXF or DWG panel's Units choices (meters per drawing unit;
/// `None`: the file's own units, else millimeters).
pub const DXF_UNITS: &[(&str, Option<f64>)] = &[("From file", None), ("Millimeter", Some(0.001)), ("Centimeter", Some(0.01)), ("Meter", Some(1.0)), ("Inch", Some(0.0254)), ("Foot", Some(0.3048)), ("Yard", Some(0.9144))];

/// Insert DXF or DWG from a file: parse it and add its geometry in the
/// chosen units (see [`DXF_UNITS`]). Errors carry the message the panel shows.
pub fn insert_dxf_file(data: &mut SketchData, path: &std::path::Path, unit_scale: Option<f64>, use_origin: bool) -> Result<Vec<EntityId>, String> {
    let drawing = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| noshape_sketch::dwg::parse_cad(&b).map_err(|e| e.to_string())).map_err(|e| format!("Could not insert the drawing: {e}"))?;
    let scale = unit_scale.or(drawing.unit_scale).unwrap_or(0.001);
    Ok(insert_dxf(data, &drawing, scale, use_origin))
}

/// Insert image: an image file's name, bytes and aspect ratio (width /
/// height), what the editor's `pending_image` holds. Errors carry the
/// message the panel shows.
pub fn load_image(path: &std::path::Path) -> Result<(String, Vec<u8>, f64), String> {
    let (bytes, w, h) = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| image::load_from_memory(&b).map(|img| (b, img.width(), img.height())).map_err(|e| e.to_string())).map_err(|e| format!("Could not read the image: {e}"))?;
    if w == 0 || h == 0 {
        return Err("The image is empty".into());
    }
    let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "image".into());
    Ok((name, bytes, w as f64 / h as f64))
}

/// Insert DXF: add a drawing's geometry (`scale` meters per drawing unit),
/// centered on the sketch origin unless `use_origin`.
pub fn insert_dxf(data: &mut SketchData, drawing: &noshape_sketch::dxf::DxfDrawing, scale: f64, use_origin: bool) -> Vec<EntityId> {
    let offset = if use_origin {
        DVec2::ZERO
    } else {
        let mut tmp = Sketch::new();
        tmp.insert_drawing(drawing, scale, DVec2::ZERO);
        let bb = tmp.bbox();
        if bb.is_empty() { DVec2::ZERO } else { -bb.center() }
    };
    let ids = data.sketch.insert_drawing(drawing, scale, offset);
    data.sketch.solve();
    ids
}

/// Corner of a square with the given first corner/center, on the cursor's side.
fn square_corner(a: DVec2, c: DVec2) -> DVec2 {
    let d = c - a;
    let s = d.x.abs().max(d.y.abs());
    a + DVec2::new(if d.x < 0.0 { -s } else { s }, if d.y < 0.0 { -s } else { s })
}

/// Third point of an exact semicircle over the chord `a`-`b` when the
/// cursor's 3-point circle has its center within `tol` of the chord midpoint.
fn semicircle_snap(a: DVec2, b: DVec2, p: DVec2, tol: f64) -> Option<DVec2> {
    let m = (a + b) * 0.5;
    let (ctr, _) = noshape_sketch::shapes::circle_through(a, p, b)?;
    if (ctr - m).length() > tol {
        return None;
    }
    let n = (b - a).perp().try_normalize()?;
    let side = if (p - m).dot(n) < 0.0 { -1.0 } else { 1.0 };
    Some(m + n * side * (b - a).length() * 0.5)
}

fn nearest_point(sk: &Sketch, p: DVec2, radius: f64) -> Option<EntityId> {
    sk.entities()
        .filter(|e| e.kind() == EntityKind::Point)
        .filter_map(|e| sk.point(e.id).map(|q| (e.id, (q - p).length())))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
}

/// Remove texts whose box was deleted, and dimension expressions, labels
/// and glyph offsets whose constraints are gone.
/// Drops reference-only model geometry (see `Entity::reference_only`) no
/// constraint or dimension uses any more, with its link.
pub fn prune_reference_only(data: &mut SketchData) {
    let sk = &data.sketch;
    let used: std::collections::HashSet<EntityId> = sk.constraints().flat_map(|c| c.kind.entities().to_vec()).collect();
    let mut drop: Vec<EntityId> = Vec::new();
    for e in sk.entities().filter(|e| e.reference_only) {
        // A curve stays while it or one of its points is used; its points go with it.
        if e.kind() == EntityKind::Point {
            if !used.contains(&e.id) && sk.users_of_point(e.id).is_empty() {
                drop.push(e.id);
            }
        } else if !used.contains(&e.id) && !e.point_refs().iter().any(|p| used.contains(p)) {
            drop.push(e.id);
        }
    }
    if drop.is_empty() {
        return;
    }
    data.sketch.remove_entities(&drop);
    data.external_refs.retain(|k, _| data.sketch.entity(EntityId(*k)).is_some());
}

fn prune_exprs(data: &mut SketchData) {
    prune_reference_only(data);
    data.prune_texts();
    data.prune_images();
    let live: std::collections::HashSet<u32> = data.sketch.constraints().map(|c| c.id.0).collect();
    data.dimension_exprs.retain(|k, _| live.contains(k));
    data.dimension_labels.retain(|k, _| live.contains(k));
    data.glyph_offsets.retain(|k, _| k.split(':').next().and_then(|c| c.parse::<u32>().ok()).is_some_and(|c| live.contains(&c)));
    data.intersections.retain_mut(|l| {
        l.entities.retain(|e| data.sketch.entity(EntityId(*e)).is_some());
        !l.entities.is_empty()
    });
    data.dimension_tolerances.retain(|k, _| live.contains(k));
    // A pattern lives while a seed does (deleted instances leave a gap).
    let sk = &data.sketch;
    data.patterns.retain(|p| p.seeds.iter().any(|e| sk.entity(EntityId(*e)).is_some()));
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
        ConstraintKind::Rho { conic, .. } => curve_mid(sk, *conic),
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
    // `dir` is the direction the arrow travels (pointing at `tip`). Onshape's
    // arrowheads are filled, narrow triangles (live sketch/12: 16 x 7 px):
    // hatched with 1 px lines from the tip to the base so they read as solid.
    let d = dir.try_normalize().unwrap_or(DVec2::X);
    let n = d.perp();
    let (len, half) = (s * 16.0, s * 3.5);
    let base = tip - d * len;
    let steps = 12;
    for i in 0..=steps {
        let t = i as f64 / steps as f64 * 2.0 - 1.0;
        out.push([tip, base + n * half * t]);
    }
}

/// Screen width of a dimension value at 14 px (Roboto, digits ~7.6 px).
pub fn label_width_px(text: &str) -> f64 {
    // 16 px text (Onshape's dimension values).
    text.chars().count() as f64 * 8.7 + 4.0
}

/// The parts of a segment outside an axis-aligned box (center, half size).
fn clip_outside_box(sg: [DVec2; 2], c: DVec2, half: DVec2) -> Vec<[DVec2; 2]> {
    let (a, b) = (sg[0], sg[1]);
    let d = b - a;
    // Liang-Barsky: the parameter interval of the segment inside the box.
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-d.x, a.x - (c.x - half.x)), (d.x, (c.x + half.x) - a.x), (-d.y, a.y - (c.y - half.y)), (d.y, (c.y + half.y) - a.y)] {
        if p.abs() < 1e-15 {
            if q < 0.0 {
                return vec![sg];
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    if t0 >= t1 {
        return vec![sg];
    }
    let mut out = Vec::new();
    if t0 > 1e-9 {
        out.push([a, a + d * t0]);
    }
    if t1 < 1.0 - 1e-9 {
        out.push([a + d * t1, b]);
    }
    out
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
    if diameter && span.is_some() && dl > r {
        // An arc's diameter (live sketch/1946, a slot's end cap): one leader
        // from the label to the near side, its arrow pointing in.
        out.push([tip, far]);
        arrow(out, tip, -v, s);
    } else if diameter {
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

/// A sketch chamfer's two distance dimensions (virtual sharp → chamfer end).
fn chamfer_dims(sk: &Sketch, r: &noshape_sketch::EditResult) -> Vec<ConstraintId> {
    r.constraints_added.iter().copied().filter(|c| sk.constraint(*c).is_some_and(|c| matches!(c.kind, ConstraintKind::Distance { .. }))).collect()
}

/// Where a chamfer's distance labels go (live sketch/1960–1961): 50 px
/// outside each edge, at the middle of the distance.
fn chamfer_dim_labels(sk: &Sketch, dims: &[ConstraintId], px: f64) -> Vec<(ConstraintId, DVec2)> {
    let ends: Vec<(ConstraintId, DVec2, DVec2)> = dims
        .iter()
        .filter_map(|cid| match sk.constraint(*cid)?.kind {
            ConstraintKind::Distance { a, b, .. } => Some((*cid, sk.point(a)?, sk.point(b)?)),
            _ => None,
        })
        .collect();
    ends.iter()
        .map(|&(cid, sharp, end)| {
            let other = ends.iter().find(|e| e.0 != cid).map_or(sharp, |e| e.2);
            let mut n = (end - sharp).try_normalize().unwrap_or(DVec2::X).perp();
            if n.dot(other - sharp) > 0.0 {
                n = -n;
            }
            (cid, (sharp + end) * 0.5 + n * px * 50.0)
        })
        .collect()
}

fn dim_segments(sk: &Sketch, k: &ConstraintKind, pos: DVec2, s: f64, out: &mut Vec<[DVec2; 2]>) {
    match k {
        // Centerline: from the entity to its mirror image across the axis.
        ConstraintKind::Distance { a, b, mode: DistanceMode::Centerline, .. } => {
            let (other, axis) = if sk.kind(*b) == Some(EntityKind::Line) && sk.entity(*b).is_some_and(|e| e.construction) { (*a, *b) } else { (*b, *a) };
            if let (Some(q), Some((l0, l1))) = (rep_point(sk, other), line_of(sk, axis)) {
                let m = noshape_sketch::reflect(q, l0, l1);
                linear_dim(out, q, m, None, pos, s);
            }
        }
        ConstraintKind::Distance { a, b, mode, .. } => {
            if let Some((mut p, mut q)) = linear_pair(sk, *a, *b) {
                // Minimum / maximum: measured to the circles' rims.
                if matches!(mode, DistanceMode::Minimum | DistanceMode::Maximum) {
                    let sign = if *mode == DistanceMode::Minimum { 1.0 } else { -1.0 };
                    let radius = |e: EntityId| match sk.curve(e) {
                        Some(Curve2::Circle { radius, .. } | Curve2::Arc { radius, .. }) => Some(radius),
                        _ => None,
                    };
                    let u = (q - p).try_normalize().unwrap_or(DVec2::X);
                    if let Some(r) = radius(*a) {
                        p += u * r * sign;
                    }
                    if let Some(r) = radius(*b) {
                        q -= u * r * sign;
                    }
                }
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
        ConstraintKind::Rho { conic, .. } => {
            if let Some(m) = curve_mid(sk, *conic) {
                out.push([m, pos]);
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
        Quadrant { point, .. } => vec![point],
        Symmetric { a, b, .. } => vec![a, b],
        Fix { entity } => vec![entity],
        // Live: the slot's three offset glyphs sit together at the end cap.
        Offset { reference, .. } => vec![reference],
        Normal { line, curve } => vec![line, curve],
        // Live draws a tangent's glyph once, on its first entity (the line of
        // a fillet, the cap of a slot; qa5 sketch tools).
        Tangent { a, .. } => vec![a],
        Concentric { a, b } | Parallel { a, b } | Perpendicular { a, b } | Equal { a, b } | Collinear { a, b } | Curvature { a, b } => vec![a, b],
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
pub fn smart_dimension(sk: &Sketch, ids: &[EntityId], cursor: DVec2) -> Option<ConstraintKind> {
    let kind = |id: EntityId| sk.kind(id);
    let dist = |a: EntityId, b: EntityId| ConstraintKind::Distance { a, b, value: 0.0, mode: DistanceMode::Center };
    match ids {
        [a] => match kind(*a)? {
            EntityKind::Line => {
                let (pa, pb) = sk.endpoints(*a)?;
                let (qa, qb) = (sk.point(pa)?, sk.point(pb)?);
                let len = ConstraintKind::Length { curve: *a, value: (qb - qa).length() };
                // Axis-aligned lines always get their length.
                let d = qb - qa;
                if d.x.abs() < 1e-9 * d.length().max(1e-300) || d.y.abs() < 1e-9 * d.length().max(1e-300) {
                    return Some(len);
                }
                Some(directed(pa, pb, qa, qb, cursor).unwrap_or(len))
            }
            EntityKind::Circle => Some(ConstraintKind::Diameter { curve: *a, value: 0.0 }),
            EntityKind::Arc => Some(ConstraintKind::Radius { curve: *a, value: 0.0 }),
            EntityKind::Conic => Some(ConstraintKind::Rho { conic: *a, value: 0.0 }),
            EntityKind::Ellipse | EntityKind::EllipticalArc => {
                // Major or minor diameter depending on which axis the cursor is nearer.
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
        [p1, p2, arc] if kind(*arc) == Some(EntityKind::Arc) && kind(*p1) == Some(EntityKind::Point) && kind(*p2) == Some(EntityKind::Point) => Some(ConstraintKind::Length { curve: *arc, value: 0.0 }),
        [a, b] => {
            let (ka, kb) = (kind(*a)?, kind(*b)?);
            match (ka, kb) {
                (EntityKind::Point, EntityKind::Point) => {
                    let (qa, qb) = (sk.point(*a)?, sk.point(*b)?);
                    Some(directed(*a, *b, qa, qb, cursor).unwrap_or(dist(*a, *b)))
                }
                (EntityKind::Line, EntityKind::Line) => {
                    let (la, lb) = (line_of(sk, *a)?, line_of(sk, *b)?);
                    let (da, db) = ((la.1 - la.0).normalize(), (lb.1 - lb.0).normalize());
                    if da.perp_dot(db).abs() < 1e-6 {
                        Some(centerline_or(sk, *a, *b, cursor).unwrap_or(dist(*a, *b)))
                    } else {
                        // The angle of the quadrant the cursor is in.
                        Some(ConstraintKind::Angle { a: *a, b: *b, value: sector_angle(la, lb, cursor) })
                    }
                }
                _ => Some(centerline_or(sk, *a, *b, cursor).unwrap_or(dist(*a, *b))),
            }
        }
        _ => None,
    }
}

/// Distances to circles / arcs picked on their rim measure to the nearest
/// (minimum) or farthest (maximum) point instead of the center.
fn circle_mode(sk: &Sketch, k: ConstraintKind, picks: &[(EntityId, DVec2)], wpp: f64) -> ConstraintKind {
    let ConstraintKind::Distance { a, b, value, mode: DistanceMode::Center } = k else { return k };
    let circ = |e: EntityId| sk.kind(e).is_some_and(|k| k.is_circular());
    let on_rim = |e: EntityId| {
        let q = picks.iter().find(|p| p.0 == e)?.1;
        let (ctr, r) = match sk.curve(e)? {
            Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. } => (center, radius),
            _ => return None,
        };
        ((q - ctr).length() - r).abs().le(&(wpp * 8.0)).then_some((q, ctr))
    };
    let (c, other) = if circ(b) { (b, a) } else if circ(a) { (a, b) } else { return k };
    let Some((q, ctr)) = on_rim(c) else { return k };
    let o = match sk.curve(other) {
        Some(Curve2::Line { a: l0, b: l1 }) => foot(ctr, l0, l1),
        _ => rep_point(sk, other).unwrap_or(ctr),
    };
    let mode = if (q - o).length() < (ctr - o).length() { DistanceMode::Minimum } else { DistanceMode::Maximum };
    ConstraintKind::Distance { a, b, value, mode }
}

/// Centerline dimension when one entity is a construction line and the
/// cursor is on the other side of it than the other entity (Onshape toggles
/// between distance and centerline dimensions as the cursor crosses).
fn centerline_or(sk: &Sketch, a: EntityId, b: EntityId, cursor: DVec2) -> Option<ConstraintKind> {
    let is_cl = |e: EntityId| sk.entity(e).is_some_and(|x| x.construction && x.kind() == EntityKind::Line);
    let (other, axis) = if is_cl(b) { (a, b) } else if is_cl(a) { (b, a) } else { return None };
    if is_cl(other) {
        return None;
    }
    let (l0, l1) = line_of(sk, axis)?;
    let q = rep_point(sk, other)?;
    let side = |p: DVec2| (l1 - l0).perp_dot(p - l0);
    (side(q) * side(cursor) < 0.0).then_some(ConstraintKind::Distance { a: other, b: axis, value: 0.0, mode: DistanceMode::Centerline })
}

/// Angle (0..180 degrees) of the sector between two lines that contains `cursor`.
fn sector_angle(la: (DVec2, DVec2), lb: (DVec2, DVec2), cursor: DVec2) -> f64 {
    let da = (la.1 - la.0).try_normalize().unwrap_or(DVec2::X);
    let db = (lb.1 - lb.0).try_normalize().unwrap_or(DVec2::Y);
    let den = da.perp_dot(db);
    if den.abs() < 1e-12 {
        return 0.0;
    }
    let t = (lb.0 - la.0).perp_dot(db) / den;
    let c = la.0 + da * t;
    let th = (cursor - c).to_angle();
    let mut rays = [da, db, -da, -db];
    rays.sort_by(|x, y| x.to_angle().rem_euclid(TAU).total_cmp(&y.to_angle().rem_euclid(TAU)));
    for i in 0..4 {
        let (s1, s2) = (rays[i], rays[(i + 1) % 4]);
        let a0 = s1.to_angle();
        let sweep = noshape_sketch::geom::ccw_sweep(a0, s2.to_angle());
        if noshape_sketch::geom::angle_in_range(th, a0, sweep, 1e-12).is_some() {
            return sweep;
        }
    }
    da.angle_to(db).abs()
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
        ConstraintTool::Pierce => {
            let b = b?;
            let ext = |e: EntityId| sk.entity(e).is_some_and(|x| x.external);
            if ext(a) && !ext(b) { ConstraintKind::Pierce { point: b, target: a } } else { ConstraintKind::Pierce { point: a, target: b } }
        }
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

/// Can constraint `c` be applied to the selected entities? (Onshape enables
/// only the applicable constraint buttons for a selection.)
pub fn constraint_applicable(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> bool {
    let kinds: Vec<EntityKind> = ids.iter().filter_map(|e| sk.kind(*e)).collect();
    if kinds.len() != ids.len() || kinds.is_empty() {
        return false;
    }
    let n = kinds.len();
    let all = |f: &dyn Fn(EntityKind) -> bool| kinds.iter().all(|k| f(*k));
    let count = |f: &dyn Fn(EntityKind) -> bool| kinds.iter().filter(|k| f(**k)).count();
    let point = |k: EntityKind| k == EntityKind::Point;
    let line = |k: EntityKind| k == EntityKind::Line;
    let round = |k: EntityKind| k.is_circular() || k.is_elliptical();
    let curve = |k: EntityKind| k != EntityKind::Point;
    match c {
        ConstraintTool::Coincident => n >= 2 && (count(&point) >= 1 && (count(&point) == n || count(&point) == 1 || n == 2) || all(&line) || all(&|k| k.is_circular())),
        ConstraintTool::Concentric => n >= 2 && all(&|k| round(k) || point(k)) && count(&round) >= 1,
        ConstraintTool::Parallel => n >= 2 && all(&line),
        ConstraintTool::Tangent => n >= 2 && all(&curve) && !all(&line),
        ConstraintTool::Horizontal | ConstraintTool::Vertical => all(&|k| line(k) || k == EntityKind::Ellipse) || (n >= 2 && all(&point)),
        ConstraintTool::Perpendicular => n == 2 && all(&line),
        ConstraintTool::Equal => n >= 2 && (all(&line) || all(&|k| k.is_circular())),
        ConstraintTool::Midpoint => (n == 2 && count(&point) == 1 && kinds.iter().any(|k| matches!(k, EntityKind::Line | EntityKind::Arc))) || (n == 3 && all(&point)),
        ConstraintTool::Normal => n == 2 && count(&line) >= 1 && all(&curve),
        ConstraintTool::Pierce => n == 2 && ids.iter().filter(|e| sk.entity(**e).is_some_and(|x| x.external)).count() == 1 && ids.iter().any(|e| is_point(sk, *e) && !sk.entity(*e).is_some_and(|x| x.external)),
        ConstraintTool::Symmetric => n == 3 && count(&line) >= 1,
        ConstraintTool::Fix => true,
        ConstraintTool::Curvature => n == 2 && all(&curve) && kinds.iter().any(|k| k.is_spline()),
    }
}

/// Points already coincident (one point, or joined by coincident
/// constraints): Onshape's Coincident adds nothing for them and shows nothing.
pub fn already_coincident(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> bool {
    c == ConstraintTool::Coincident && ids.len() >= 2 && ids.iter().all(|e| is_point(sk, *e)) && {
        let joint = sk.joint_points(ids[0]);
        ids[1..].iter().all(|e| joint.contains(e))
    }
}

/// Apply constraint `c` to a selection: n-ary constraints chain the first
/// entity with each other one, one-entity constraints apply to each entity.
/// Returns the number of constraints added.
pub fn apply_constraint(sk: &mut Sketch, c: ConstraintTool, ids: &[EntityId]) -> Result<usize, String> {
    let mut kinds: Vec<ConstraintKind> = Vec::new();
    let pts = ids.iter().all(|e| is_point(sk, *e));
    match c {
        ConstraintTool::Horizontal | ConstraintTool::Vertical if pts => {
            for e in &ids[1..] {
                kinds.extend(make_constraint(sk, c, &[ids[0], *e]));
            }
        }
        ConstraintTool::Horizontal | ConstraintTool::Vertical | ConstraintTool::Fix => {
            for e in ids {
                kinds.extend(make_constraint(sk, c, &[*e]));
            }
        }
        ConstraintTool::Symmetric | ConstraintTool::Perpendicular | ConstraintTool::Normal | ConstraintTool::Pierce | ConstraintTool::Curvature => {
            kinds.extend(make_constraint(sk, c, ids));
        }
        ConstraintTool::Midpoint if ids.len() == 3 => {
            // Third point midway between the first two: a construction line carries it.
            let l = sk.add_line_between(ids[0], ids[1]).map_err(|e| e.to_string())?;
            let _ = sk.set_construction(l, true);
            kinds.push(ConstraintKind::Midpoint { point: ids[2], curve: l });
        }
        ConstraintTool::Coincident if ids.iter().filter(|e| is_point(sk, **e)).count() == 1 && ids.len() > 2 => {
            // One point on several curves.
            let p = *ids.iter().find(|e| is_point(sk, **e)).expect("point");
            for e in ids.iter().filter(|e| **e != p) {
                kinds.push(ConstraintKind::Coincident { a: p, b: *e });
            }
        }
        _ => {
            for e in ids[1..].iter().filter(|e| !already_coincident(sk, c, &[ids[0], **e])) {
                kinds.extend(make_constraint(sk, c, &[ids[0], *e]));
            }
        }
    }
    let mut n = 0;
    let mut last_err = None;
    for k in kinds {
        match sk.add_constraint(k) {
            Ok(_) => n += 1,
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    match (n, last_err) {
        (0, Some(e)) => Err(e),
        (n, _) => Ok(n),
    }
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
}

/// Onshape's frame around the sketch being edited (`SKETCH_PLANE_LINE_COLOR`
/// = `--os-color-seaport-500`; the name uses the same colour).
pub const SKETCH_BOX_COLOR: u32 = 0x90cef1;
/// Onshape `VIEWPORT_HEIGHT_TO_PLANE_HEIGHT_RATIO`: the frame's height as a
/// fraction of the graphics area's height when it is first drawn.
const SKETCH_FRAME_VIEWPORT_RATIO: f64 = 0.7;
/// Onshape `PLANE_DEFAULT_WIDTH_HEIGHT_RATIO`.
const SKETCH_FRAME_ASPECT: f64 = 1.77777;

impl SketchEditor {
    /// Fixes the sketch frame in the plane the way Onshape's sketch plane
    /// display does on its first draw: centred on `center` (the sketch
    /// plane entity's bounds centre, in sketch coordinates), 0.7 of the
    /// graphics area's height tall at the current zoom, 16:9. It then stays
    /// put while zooming or drawing; a new sketch plane places it again.
    pub fn place_frame(&mut self, center: DVec2, viewport_height_px: f64) {
        if self.frame_bounds.is_some() {
            return;
        }
        let h = SKETCH_FRAME_VIEWPORT_RATIO * viewport_height_px * self.world_per_px;
        let half = DVec2::new(h * SKETCH_FRAME_ASPECT, h) * 0.5;
        self.frame_bounds = Some((center - half, center + half));
    }

    /// The light-blue frame Onshape draws around the sketch being edited
    /// (with the sketch's name at its top-left). Returns the lower-left and
    /// upper-right corners in sketch coordinates; before the view placed it,
    /// a frame of 0.7 x 1000 px at the current zoom around the origin.
    /// A box selection is being dragged.
    pub fn box_dragging(&self) -> bool {
        matches!(self.drag, Some(DragState::Box { .. }))
    }

    pub fn sketch_box(&self) -> (DVec2, DVec2) {
        self.frame_bounds.unwrap_or_else(|| {
            let h = SKETCH_FRAME_VIEWPORT_RATIO * 1000.0 * self.world_per_px;
            let half = DVec2::new(h * SKETCH_FRAME_ASPECT, h) * 0.5;
            (-half, half)
        })
    }
}

#[cfg(test)]
mod tests {
    //! Headless tests of the sketch editor's Onshape interaction rules:
    //! click sequences, drags, inference, on-the-fly dimensions, the
    //! dimension tool and constraint application.
    use super::*;

    const PX: f64 = 1e-4; // world units per pixel in these tests

    fn editor(tool: Tool) -> (SketchEditor, SketchData) {
        let mut ed = SketchEditor::new(FeatureId(1), PlaneFrame::TOP);
        ed.world_per_px = PX;
        ed.set_tool(tool);
        (ed, SketchData::default())
    }

    fn v(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    /// Move to `p` and click (press + release in place).
    fn click(ed: &mut SketchEditor, data: &mut SketchData, p: DVec2) {
        ed.pointer_move(data, p, false, false);
        let h = data.sketch.pick(p, PX * 4.0);
        ed.hover = h;
        ed.tool_down(data, p, h, false, false);
        ed.tool_up(data, p);
    }

    fn drag(ed: &mut SketchEditor, data: &mut SketchData, a: DVec2, b: DVec2) {
        ed.pointer_move(data, a, false, false);
        let h = data.sketch.pick(a, PX * 4.0);
        ed.hover = h;
        ed.tool_down(data, a, h, false, false);
        for k in 1..=4 {
            ed.tool_drag(data, a.lerp(b, k as f64 / 4.0));
        }
        ed.tool_up(data, b);
    }

    fn count(data: &SketchData, k: EntityKind) -> usize {
        data.sketch.entities().filter(|e| e.kind() == k).count()
    }

    /// Live "Sketch dimension display + Enter/Esc" (lead sketch dim): labels
    /// are the value in workspace units with no unit, 3 decimals with the
    /// zeros dropped, "°" for angles, "Ø" for diameters; a variable gets
    /// "ƒx", an expression "ƒ".
    #[test]
    fn dimension_labels_read_as_live() {
        let e = EntityId(1);
        let mm = UnitPrefs::default();
        let inch = UnitPrefs { length: noshape_units::LengthUnit::Inch, ..UnitPrefs::default() };
        let len = |v: f64| ConstraintKind::Length { curve: e, value: v };
        assert_eq!(dim_value_text(&len(0.001), 0.001, &mm), "1");
        assert_eq!(dim_value_text(&len(0.0254), 0.0254, &mm), "25.4");
        assert_eq!(dim_value_text(&len(0.0306071), 0.0306071, &mm), "30.607");
        assert_eq!(dim_value_text(&len(0.0254), 0.0254, &inch), "1");
        assert_eq!(dim_value_text(&len(0.254), 0.254, &inch), "10");
        let angle = ConstraintKind::Angle { a: e, b: EntityId(2), value: 30f64.to_radians() };
        assert_eq!(dim_value_text(&angle, 30f64.to_radians(), &mm), "30°");
        let dia = ConstraintKind::Diameter { curve: e, value: 0.01 };
        assert_eq!(dim_value_text(&dia, 0.01, &mm), "Ø10");
        let label = |expr: &str| dimension_label_text(Some(expr), "10".into(), false);
        assert_eq!(label("10 mm"), "10");
        assert_eq!(label("1 in"), "10");
        assert_eq!(label("#var"), "ƒx 10");
        assert_eq!(label("(2*5) mm"), "ƒ 10");
        assert_eq!(label("#var*2"), "ƒ 10");
        assert_eq!(dimension_label_text(None, "30.607".into(), false), "30.607");
    }

    /// Live (lead sketch dim): Enter never accepts the sketch: with a tool
    /// after a commit, with no tool, with a selection. Esc ends the tool and
    /// never clears the selection or leaves the sketch.
    #[test]
    fn enter_never_accepts_the_sketch_and_esc_keeps_the_selection() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.03, 0.01));
        assert_eq!(ed.enter_target(), EnterTarget::OnTheFly, "mid-chain Enter places the boxed length");
        assert!(ed.cancel());
        assert_eq!(ed.enter_target(), EnterTarget::Nothing, "no tool");
        ed.set_tool(Tool::Dimension);
        assert_eq!(ed.enter_target(), EnterTarget::Nothing, "Dimension tool active, nothing pending");
        assert!(ed.cancel(), "Esc ends the Dimension tool");
        assert_eq!(ed.tool, Tool::Select);
        click(&mut ed, &mut d, v(0.02, 0.01));
        assert!(!ed.selection.is_empty(), "the line is selected");
        assert_eq!(ed.enter_target(), EnterTarget::Nothing, "a selection");
        assert!(!ed.cancel(), "Esc with a selection does nothing");
        assert!(!ed.selection.is_empty(), "and keeps it");
    }

    /// Live (qa5 dim conflict, SK1): a value the sketch can't be solved with
    /// is taken (the dimension reads it) but the last solved shape stays,
    /// with Onshape's "Sketch could not be solved.".
    #[test]
    fn an_unsolvable_value_keeps_the_solved_shape() {
        let (mut ed, mut d) = editor(Tool::Select);
        let a = d.sketch.add_point(v(0.0, 0.0));
        let b = d.sketch.add_point(v(0.03, 0.0));
        let line = d.sketch.add_line_between(a, b).unwrap();
        d.sketch.add_constraint(ConstraintKind::Horizontal { a: line, b: None }).unwrap();
        d.sketch.add_constraint(ConstraintKind::HorizontalDistance { a, b, value: 0.03 }).unwrap();
        let len = d.sketch.add_reference(ConstraintKind::Length { curve: line, value: 0.03 }).unwrap();
        let _ = d.sketch.set_driving(len, true);
        let before = d.sketch.point(b).unwrap();
        let r = ed.apply_dimension(&mut d, len, "10", &UnitPrefs::default(), &noshape_units::EmptyScope);
        assert_eq!(r, Err(SKETCH_NOT_SOLVED.to_string()));
        assert!((d.sketch.point(b).unwrap() - before).length() < 1e-12, "the solved shape stays");
        assert!((d.sketch.constraint(len).unwrap().kind.value().unwrap() - 0.01).abs() < 1e-12, "the value is taken");
        assert_eq!(d.dimension_exprs[&len.0].text, "10 mm");
    }

    /// Live (lead sketch dim): Esc on the dimension box closes it and the
    /// Dimension tool stays; the next Esc ends the tool (qa4 F29: the app
    /// then gives the keys back to the sketch).
    #[test]
    fn esc_closes_the_dimension_box_then_the_tool() {
        let (mut ed, _) = editor(Tool::Dimension);
        ed.dim_edit = Some(DimEdit { constraint: ConstraintId(1), text: "abc".into(), created: false });
        assert!(ed.cancel());
        assert!(ed.dim_edit.is_none() && ed.tool == Tool::Dimension, "the box closes, the tool stays");
        assert!(ed.cancel());
        assert_eq!(ed.tool, Tool::Select);
    }

    #[test]
    fn sketch_options_default_as_live() {
        let o = SketchOptions::default();
        assert!(o.show_constraints && o.show_errors && !o.show_expressions);
    }

    #[test]
    fn line_clicks_chain_and_one_esc_exits_the_tool() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.01, 0.013));
        click(&mut ed, &mut d, v(0.03, 0.017));
        click(&mut ed, &mut d, v(0.035, 0.04));
        assert_eq!(count(&d, EntityKind::Line), 2);
        // the two segments share their joint
        let pts = count(&d, EntityKind::Point);
        assert_eq!(pts, 3);
        assert!(ed.cancel(), "one Esc ends the chain and exits Line");
        assert_eq!(ed.tool, Tool::Select);
        assert!(ed.clicks.is_empty() && ed.chain_end.is_none());
        assert_eq!(count(&d, EntityKind::Line), 2, "completed segments remain");
    }

    #[test]
    fn enter_mid_chain_places_the_boxed_length_as_a_driving_dimension() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.01, 0.013));
        click(&mut ed, &mut d, v(0.03, 0.017));
        assert!(ed.otf.is_some(), "the segment's length is boxed");
        let labels = ed.otf_labels(&d, &UnitPrefs::default());
        assert!(labels.len() == 1 && !labels[0].contains("mm") && labels[0].starts_with("20."), "bare number like a dimension label: {labels:?}");
        let before = d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::Length { .. })).count();
        assert!(ed.accept_otf(&mut d).unwrap());
        let dims: Vec<_> = d.sketch.constraints().filter(|c| c.driving && matches!(c.kind, ConstraintKind::Length { .. })).collect();
        assert_eq!(dims.len(), before + 1);
        let len = (v(0.03, 0.017) - v(0.01, 0.013)).length();
        assert!((dims[0].kind.value().unwrap() - len).abs() < 1e-9, "dimensioned at the drawn length");
        assert_eq!(ed.tool, Tool::Line);
        assert!(ed.chain_end.is_some(), "the chain keeps going");
        assert!(ed.otf.is_none());
        assert!(!ed.accept_otf(&mut d).unwrap(), "nothing left to accept");
    }

    #[test]
    fn press_drag_from_the_chain_end_places_nothing() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.03, 0.01));
        drag(&mut ed, &mut d, v(0.03, 0.01), v(0.04, 0.03));
        assert_eq!(count(&d, EntityKind::Line), 1, "release does not end a segment");
        assert_eq!(count(&d, EntityKind::Arc), 0, "and no arc is made");
        assert!(ed.chain_end.is_some() && ed.otf.is_some(), "chain and boxed length stay");
        click(&mut ed, &mut d, v(0.04, 0.03));
        assert_eq!(count(&d, EntityKind::Line), 2);
    }

    #[test]
    fn first_escape_commits_a_spline_and_the_second_exits() {
        for tool in [Tool::Spline, Tool::Bezier] {
            let (mut ed, mut d) = editor(tool);
            click(&mut ed, &mut d, v(0.01, 0.01));
            click(&mut ed, &mut d, v(0.02, 0.03));
            click(&mut ed, &mut d, v(0.04, 0.02));
            ed.pointer_move(&mut d, v(0.05, 0.05), false, false);
            assert!(ed.escape_commit(&mut d).is_some());
            assert_eq!(count(&d, EntityKind::FitSpline) + count(&d, EntityKind::BSpline), 1, "{tool:?} committed through the clicked points");
            assert_eq!(ed.tool, tool, "tool stays active");
            assert!(ed.escape_commit(&mut d).is_none());
            assert!(ed.cancel());
            assert_eq!(ed.tool, Tool::Select);
        }
        // One placed point: nothing to commit, Esc just exits.
        let (mut ed, mut d) = editor(Tool::Spline);
        click(&mut ed, &mut d, v(0.01, 0.01));
        assert!(ed.escape_commit(&mut d).is_none());
    }

    /// Live's slot glyphs (qa5 sketch tools): a coincident at each of the 6
    /// joints, 4 tangents (on the caps only), 3 offsets at the end cap.
    #[test]
    fn slot_glyphs_are_lives() {
        let (mut ed, mut d) = editor(Tool::Slot);
        d.sketch.add_line(v(0.01, 0.01), v(0.05, 0.01));
        click(&mut ed, &mut d, v(0.03, 0.01));
        assert!(ed.commit_slot(&mut d).changed);
        let glyphs = ed.constraint_glyphs(&d);
        let n = |icon: &str| glyphs.iter().filter(|g| g.icon == icon).count();
        assert_eq!((n("coincident"), n("tangent"), n("offset")), (6, 4, 3));
        assert!(glyphs.iter().filter(|g| g.icon == "tangent").all(|g| d.sketch.kind(g.entity) == Some(EntityKind::Arc)), "tangent glyphs on the caps");
    }

    #[test]
    fn slot_previews_until_the_next_click_and_keeps_its_centerline_solid() {
        let (mut ed, mut d) = editor(Tool::Slot);
        let line = d.sketch.add_line(v(0.01, 0.01), v(0.05, 0.01));
        click(&mut ed, &mut d, v(0.03, 0.01));
        assert!(ed.slot_pending(), "light-blue preview");
        let (_, at, w) = ed.slot_dimension().expect("the preview shows its Ø20 dimension");
        assert!((w - ed.slot_width).abs() < 1e-12 && at.x > 0.04, "{at} {w}");
        assert_eq!(count(&d, EntityKind::Arc), 0, "nothing committed yet");
        assert!(ed.dim_edit.is_none(), "no value box for a slot");
        click(&mut ed, &mut d, v(0.03, 0.04));
        assert!(!ed.slot_pending());
        assert_eq!(count(&d, EntityKind::Arc), 2, "the empty-space click committed it");
        assert!(!d.sketch.entity(line).unwrap().construction, "centerline stays solid");
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Diameter { .. })), "Ø width dimension");
        // Leaving the tool (Esc) commits a pending slot as well.
        let other = d.sketch.add_line(v(0.01, 0.06), v(0.05, 0.06));
        click(&mut ed, &mut d, v(0.03, 0.06));
        assert!(ed.slot_pending());
        assert!(ed.escape_commit(&mut d).is_some_and(|o| o.changed));
        assert_eq!(ed.tool, Tool::Select);
        assert_eq!(count(&d, EntityKind::Arc), 4);
        assert!(!d.sketch.entity(other).unwrap().construction);
    }

    #[test]
    fn pattern_count_labels_default_to_live_counts_and_accept_typing() {
        let (mut ed, mut d) = editor(Tool::LinearPattern);
        let line = d.sketch.add_line(v(0.01, 0.01), v(0.02, 0.01));
        ed.selection.push(line);
        let counts = |ed: &SketchEditor, d: &SketchData| ed.pattern_count_labels(&d.sketch).into_iter().map(|(h, _, n)| (h, n)).collect::<Vec<_>>();
        assert_eq!(counts(&ed, &d), vec![(PatternHandle::LinearCount, 3), (PatternHandle::LinearSecondCount, 1)], "3x along X, 1x along Y");
        assert_eq!(ed.pattern_value_labels(&d.sketch).len(), 1, "Y spacing hidden while Y is 1x");
        ed.set_pattern_count(PatternHandle::LinearSecondCount, 2);
        assert_eq!(ed.pattern_second_count, 2);
        assert_eq!(ed.pattern_value_labels(&d.sketch).len(), 2);
        ed.set_pattern_value(PatternHandle::LinearStep, 0.03);
        assert!((ed.pattern_distance - 0.03).abs() < 1e-12);
        ed.set_pattern_value(PatternHandle::LinearStep, -1.0);
        assert!((ed.pattern_distance - 0.03).abs() < 1e-12, "non-positive spacing ignored");
        ed.set_tool(Tool::CircularPattern);
        ed.selection.push(line);
        ed.pattern_center = v(0.0, 0.0);
        assert_eq!(counts(&ed, &d), vec![(PatternHandle::CircularCount, 3)], "live circular default is 3");
        ed.set_pattern_count(PatternHandle::CircularCount, 1);
        assert_eq!(ed.circular_count, 2, "at least two instances");
        assert!(ed.pattern_value_labels(&d.sketch).is_empty(), "a closed (360°) pattern shows no angle");
        ed.set_pattern_value(PatternHandle::CircularAngle, PI);
        let labels = ed.pattern_value_labels(&d.sketch);
        assert_eq!(labels.len(), 1);
        assert!(labels[0].0 == PatternHandle::CircularAngle && (labels[0].2 - PI).abs() < 1e-12, "open pattern angle is typed on canvas");
        ed.set_tool(Tool::Transform);
        ed.selection = vec![line];
        assert!(ed.pattern_value_labels(&d.sketch).is_empty(), "rotation / scale fields appear once used");
        ed.set_pattern_value(PatternHandle::Rotate, 0.5);
        ed.set_pattern_value(PatternHandle::Scale, 2.0);
        assert_eq!(ed.pattern_value_labels(&d.sketch).iter().map(|l| l.0).collect::<Vec<_>>(), vec![PatternHandle::Rotate, PatternHandle::Scale]);
        let _ = &mut d;
    }

    #[test]
    fn transform_triad_follows_the_selection_until_its_center_is_dragged() {
        let (mut ed, mut d) = editor(Tool::Select);
        let c = d.sketch.add_circle(v(0.03, 0.02), 0.004);
        ed.selection.push(c);
        ed.set_tool(Tool::Transform);
        ed.apply_preselection(&mut d);
        assert!((ed.pattern_center - v(0.03, 0.02)).length() < 1e-9, "triad on the preselected circle");
        ed.drag_handle(&d.sketch.clone(), PatternHandle::Center, v(0.0, 0.0));
        ed.follow_transform_selection(&d.sketch);
        assert!(ed.pattern_center.length() < 1e-6, "a dragged center stays put");
    }

    /// qa5 SK3: a dimension to a model edge adds no visible sketch
    /// geometry, and its reference goes with the dimension.
    #[test]
    fn model_edge_references_are_hidden_and_go_with_their_dimension() {
        let (mut ed, mut d) = editor(Tool::Select);
        let edge = d.sketch.add_line(v(-0.05, 0.0), v(0.05, 0.0));
        let _ = d.sketch.set_external(edge, true);
        let _ = d.sketch.set_construction(edge, true);
        let _ = d.sketch.set_reference_only(edge, true);
        d.external_refs.insert(edge.0, noshape_model::reference::Reference::Plane(noshape_model::reference::DatumPlane::Top));
        let c = d.sketch.add_circle(v(0.0, 0.02), 0.004);
        let center = d.sketch.entity(c).unwrap().point_refs()[0];
        let dim = d.sketch.add_constraint(ConstraintKind::Distance { a: center, b: edge, value: 0.02, mode: DistanceMode::Center }).unwrap();
        prune_reference_only(&mut d);
        assert!(d.sketch.entity(edge).is_some(), "kept while the dimension uses it");
        assert!(ed.constraint_glyphs(&d).iter().all(|g| g.icon != "use"), "no use glyph");
        assert!(ed.delete_constraint(&mut d, dim));
        assert!(d.sketch.entity(edge).is_none(), "removed with its dimension");
        assert!(d.external_refs.is_empty());
        assert!(d.sketch.entity(c).is_some());
    }

    #[test]
    fn selection_readout_matches_the_live_status_line() {
        let (mut ed, mut d) = editor(Tool::Select);
        let u = UnitPrefs::default();
        let l1 = d.sketch.add_line(v(0.0, 0.0), v(0.02, 0.0));
        let l2 = d.sketch.add_line(v(0.0, 0.01), v(0.03, 0.01));
        let l3 = d.sketch.add_line(v(0.0, 0.0), v(0.01, 0.01));
        let c = d.sketch.add_circle(v(0.05, 0.05), 0.005);
        let p = d.sketch.add_point(v(0.01, 0.02));
        let readout = |ed: &SketchEditor, sel: &[EntityId]| {
            let mut ed2 = SketchEditor::new(ed.feature, ed.frame);
            ed2.selection = sel.to_vec();
            ed2.selection_readout(&d, &u).map(|(l, v)| format!("{l}: {v}"))
        };
        assert_eq!(readout(&ed, &[l1]).as_deref(), Some("Length: 20 mm"));
        assert_eq!(readout(&ed, &[c]).as_deref(), Some("Diameter: 10 mm"));
        assert_eq!(readout(&ed, &[l1, l2]).as_deref(), Some("Parallel dist: 10 mm"));
        assert!(readout(&ed, &[l1, l3]).is_some_and(|t| t.starts_with("Angle: 45") && t.ends_with(" deg")));
        assert!(readout(&ed, &[p]).is_some_and(|t| t.starts_with("Point: X 10 mm")), "{:?}", readout(&ed, &[p]));
        assert_eq!(readout(&ed, &[l1, l2, c]), None, "mixed selection shows nothing");
        ed.selection.clear();
        assert_eq!(ed.selection_readout(&d, &u), None);
        let _ = &mut d;
    }

    #[test]
    fn used_geometry_shows_use_glyphs_and_deleting_one_unlinks_it() {
        let (mut ed, mut d) = editor(Tool::Select);
        let r = d.sketch.use_external(&noshape_sketch::Curve2::Line { a: v(0.0, 0.0), b: v(0.02, 0.0) }).unwrap();
        let line = *r.created.last().unwrap();
        d.external_refs.insert(line.0, noshape_model::reference::Reference::Plane(noshape_model::reference::DatumPlane::Top));
        let glyphs: Vec<GlyphInfo> = ed.constraint_glyphs(&d).into_iter().filter(|g| g.icon == "use").collect();
        assert_eq!(glyphs.len(), 3, "middle and both ends");
        assert!(glyphs.iter().all(|g| g.constraint == use_glyph_id(line)));
        assert!(d.sketch.entity(line).unwrap().external);
        ed.hover_constraint = Some(use_glyph_id(line));
        assert!(ed.picked(Some(&d)).contains(&line), "hovering highlights the used entity");
        ed.selected_constraints.push(use_glyph_id(line));
        assert!(ed.delete_selection(&mut d));
        assert!(d.external_refs.is_empty());
        assert!(!d.sketch.entity(line).unwrap().external, "unlinked: regular sketch geometry");
        assert!(ed.constraint_glyphs(&d).iter().all(|g| g.icon != "use"));
    }

    #[test]
    fn rubber_band_shows_live_values() {
        let (mut ed, mut d) = editor(Tool::CornerRect);
        let u = UnitPrefs::default();
        click(&mut ed, &mut d, v(0.0, 0.0));
        ed.pointer_move(&mut d, v(0.03, 0.02), true, false);
        let vals: Vec<String> = ed.preview_values(&d, &u).into_iter().map(|x| x.1).collect();
        assert_eq!(vals, vec!["30".to_string(), "20".to_string()], "width and height");
        let (mut ed, mut d) = editor(Tool::Circle);
        click(&mut ed, &mut d, v(0.0, 0.0));
        ed.pointer_move(&mut d, v(0.01, 0.0), true, false);
        let vals: Vec<String> = ed.preview_values(&d, &u).into_iter().map(|x| x.1).collect();
        assert_eq!(vals, vec!["Ø20".to_string()]);
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        ed.pointer_move(&mut d, v(0.0, 0.015), true, false);
        assert_eq!(ed.preview_values(&d, &u).into_iter().map(|x| x.1).collect::<Vec<_>>(), vec!["15".to_string()]);
    }

    #[test]
    fn escape_preserves_sketch_selection() {
        let (mut ed, mut data) = editor(Tool::Select);
        let line = data.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        ed.selection.push(line);
        assert!(!ed.cancel());
        assert_eq!(ed.selection, vec![line]);
    }

    #[test]
    fn polygon_spinner_and_next_center_commit_the_previous_polygon() {
        let (mut ed, mut data) = editor(Tool::Polygon);
        click(&mut ed, &mut data, v(0.02, 0.02));
        click(&mut ed, &mut data, v(0.03, 0.02));
        assert_eq!(count(&data, EntityKind::Line), 0, "circumference is still a preview");
        ed.adjust_polygon_sides(1);
        assert_eq!(ed.polygon_side_control().map(|c| c.2), Some(7));
        // Outside the middle of the edge after the clicked vertex (0.03, 0.02),
        // along its outward normal.
        let (at, out, _) = ed.polygon_side_control().unwrap();
        let edge_mid = (v(0.03, 0.02) + v(0.02, 0.02) + DVec2::from_angle(TAU / 7.0) * 0.01) * 0.5;
        assert!((at - v(0.02, 0.02)).length() > (edge_mid - v(0.02, 0.02)).length() && (at - edge_mid).length() < 0.003, "{at} {edge_mid}");
        assert!((out - (edge_mid - v(0.02, 0.02)).normalize()).length() < 1e-9, "{out}");
        ed.pointer_move(&mut data, v(0.08, 0.06), false, false);
        assert_eq!(ed.polygon_sides, 7, "spinner's explicit value stays locked");
        click(&mut ed, &mut data, v(0.08, 0.06));
        assert_eq!(count(&data, EntityKind::Line), 7);
        assert_eq!(ed.clicks.len(), 1, "the click also starts the next polygon");
        assert_eq!(ed.clicks[0].pos, v(0.08, 0.06));
        ed.cancel();
        assert_eq!(ed.tool, Tool::Select);
        assert_eq!(count(&data, EntityKind::Line), 7, "only the new preview is discarded");
    }

    #[test]
    fn line_drag_makes_one_segment() {
        let (mut ed, mut d) = editor(Tool::Line);
        drag(&mut ed, &mut d, v(0.01, 0.013), v(0.03, 0.027));
        assert_eq!(count(&d, EntityKind::Line), 1);
        // the next click starts a new line instead of chaining
        click(&mut ed, &mut d, v(0.05, 0.05));
        assert_eq!(count(&d, EntityKind::Line), 1);
    }

    #[test]
    fn bezier_creation_stops_at_degree_fifteen_and_can_still_finish() {
        let (mut ed, mut data) = editor(Tool::Bezier);
        for k in 0..16 {
            ed.place(&mut data, Click { pos: v(0.01 + k as f64 * 0.002, 0.02 + (k as f64).sin() * 0.004), inference: None }, false);
        }
        let end = ed.clicks.last().unwrap().clone();
        ed.place(&mut data, Click { pos: v(0.08, 0.03), inference: None }, false);
        assert_eq!(ed.clicks.len(), 16);
        assert_eq!(ed.message.as_deref(), Some("The maximum bezier degree (15) has been reached."));
        assert!(ed.place(&mut data, end, true).changed);
        let curve = data.sketch.entities().find(|e| e.kind() == EntityKind::BSpline).unwrap().id;
        assert_eq!(data.sketch.bezier_degree(curve), Some(15));
    }

    #[test]
    fn extend_without_a_boundary_reports_the_onshape_error() {
        let (mut ed, mut data) = editor(Tool::Extend);
        data.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        click(&mut ed, &mut data, v(0.025, 0.01));
        assert_eq!(ed.message.as_deref(), Some("Unable to extend the selected geometry."));
        assert_eq!(count(&data, EntityKind::Line), 1);
        assert_eq!(ed.tool, Tool::Extend);
    }

    #[test]
    fn fast_trim_drag_does_not_skip_curves_between_pointer_events() {
        let (mut ed, mut data) = editor(Tool::Trim);
        let (a, b) = (v(0.001, 0.02), v(0.13, 0.02));
        // All fall between samples of the former 64-sample path limit.
        let step = (b.x - a.x) / 64.0;
        for k in [0.5, 10.5, 30.5] {
            let x = a.x + k * step;
            data.sketch.add_line(v(x, 0.017), v(x, 0.023));
        }
        ed.tool_down(&mut data, a, None, false, false);
        assert!(ed.tool_drag(&mut data, b).changed);
        ed.tool_up(&mut data, b);
        assert_eq!(count(&data, EntityKind::Line), 0);
    }

    #[test]
    fn line_snaps_to_origin_and_infers_horizontal() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(PX * 2.0, -PX * 3.0));
        click(&mut ed, &mut d, v(0.03, PX * 2.0));
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).unwrap().id;
        let (s, _) = d.sketch.endpoints(line).unwrap();
        assert!(d.sketch.entity(s).unwrap().external, "start point is the fixed origin point");
        assert_eq!(d.sketch.point(s).unwrap(), DVec2::ZERO);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { a, b: None } if a == line)));
    }

    #[test]
    fn shift_suppresses_inference_and_drop_snap_without_pointer_movement() {
        let (mut ed, mut data) = editor(Tool::Line);
        ed.pointer_move(&mut data, v(PX, PX), false, false);
        assert!(ed.inference.is_some());
        ed.modifiers_changed(&data, true, false);
        assert!(ed.inference.is_none());
        ed.modifiers_changed(&data, false, false);
        assert!(ed.inference.is_some());
        let (mut ed, mut data) = editor(Tool::Select);
        let p = data.sketch.add_point(v(0.01, 0.01));
        ed.tool_down(&mut data, v(0.01, 0.01), Some(p), false, false);
        ed.tool_drag(&mut data, v(PX, PX));
        assert!(ed.drag_snap.is_some());
        ed.modifiers_changed(&data, true, false);
        ed.tool_up(&mut data, v(PX, PX));
        assert_eq!(data.sketch.entity_count(), 1);
        assert!(!data.sketch.entity(p).unwrap().external);
    }

    #[test]
    fn reopening_ellipse_and_rho_dimensions_preserves_the_displayed_value() {
        let (mut ed, mut data) = editor(Tool::Select);
        let ellipse = data.sketch.add_ellipse(v(0.02, 0.02), v(0.01, 0.0), 0.005);
        let cid = data.sketch.add_constraint(ConstraintKind::EllipseRadius { ellipse, major: true, value: 0.01 }).unwrap();
        let units = UnitPrefs::default();
        let text = dimension_edit_text(&data, cid, &units).unwrap();
        let q = noshape_units::evaluate_as(&text, Dim::LENGTH, &units, &noshape_units::EmptyScope).unwrap();
        assert!((q.value - 0.02).abs() < 1e-12, "ellipse input shows diameter: {text}");
        let inches = UnitPrefs { length: noshape_units::LengthUnit::Inch, ..units };
        let inch_text = dimension_edit_text(&data, cid, &inches).unwrap();
        assert!(inch_text.ends_with("in"), "entry uses document units: {inch_text}");
        let inch_value = noshape_units::evaluate_as(&inch_text, Dim::LENGTH, &inches, &noshape_units::EmptyScope).unwrap();
        assert_eq!(inch_text, "0.787402 in", "6 decimals like live sketch/141");
        assert!((inch_value.value - 0.02).abs() < 1e-7);
        ed.apply_dimension(&mut data, cid, &text, &units, &noshape_units::EmptyScope).unwrap();
        assert!((data.sketch.constraint(cid).unwrap().kind.value().unwrap() - 0.01).abs() < 1e-12);
        // An expression typed into an ellipse diameter is kept (shown scale).
        ed.apply_dimension(&mut data, cid, "2 * 15 mm", &units, &noshape_units::EmptyScope).unwrap();
        assert!((data.sketch.constraint(cid).unwrap().kind.value().unwrap() - 0.015).abs() < 1e-12, "radius = diameter / 2");
        assert_eq!(dimension_edit_text(&data, cid, &units).as_deref(), Some("2 * 15 mm"), "expression survives");
        // A plain number reopens with the document unit (QA3: "80" → "80 mm").
        ed.apply_dimension(&mut data, cid, "40", &units, &noshape_units::EmptyScope).unwrap();
        assert_eq!(dimension_edit_text(&data, cid, &units).as_deref(), Some("40 mm"));
        let conic = data.sketch.add_conic(v(0.05, 0.01), v(0.06, 0.02), v(0.07, 0.01), 0.7).unwrap();
        let rho = data.sketch.add_constraint(ConstraintKind::Rho { conic, value: 0.7 }).unwrap();
        let text = dimension_edit_text(&data, rho, &units).unwrap();
        assert_eq!(text, "0.7");
        ed.apply_dimension(&mut data, rho, &text, &units, &noshape_units::EmptyScope).unwrap();
        assert_eq!(data.sketch.constraint(rho).unwrap().kind.value(), Some(0.7));
    }

    #[test]
    fn conic_default_rho_survives_dismissal_and_typing_edits_one_dimension() {
        let draw = || {
            let (mut ed, mut data) = editor(Tool::Conic);
            for p in [v(0.01, 0.01), v(0.03, 0.01), v(0.02, 0.02)] { click(&mut ed, &mut data, p); }
            (ed, data)
        };
        let (mut ed, mut data) = draw();
        ed.cancel();
        assert!(data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Rho { value, .. } if value == 0.5) && c.driving));
        let (mut ed, mut data2) = draw();
        let units = UnitPrefs::default();
        assert!(ed.commit_otf(&mut data2, "1.5", &units, &noshape_units::EmptyScope).is_err());
        ed.commit_otf(&mut data2, "0.7", &units, &noshape_units::EmptyScope).unwrap();
        let dims: Vec<_> = data2.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::Rho { .. })).collect();
        assert_eq!(dims.len(), 1);
        assert_eq!(dims[0].kind.value(), Some(0.7));
        assert!(dims[0].driving);
        assert!(data.sketch.solve().is_solved());
    }

    #[test]
    fn on_the_fly_length_box() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.01, 0.013));
        click(&mut ed, &mut d, v(0.03, 0.021));
        let boxes = ed.otf_boxes(&d, &UnitPrefs::default());
        assert_eq!(boxes.len(), 1);
        ed.commit_otf(&mut d, "25", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        assert!(ed.otf.is_none());
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).unwrap().id;
        assert!((d.sketch.curve(line).unwrap().length() - 0.025).abs() < 1e-9);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Length { .. }) && c.driving));
    }

    #[test]
    fn rectangle_boxes_then_square_with_alt() {
        let (mut ed, mut d) = editor(Tool::CornerRect);
        click(&mut ed, &mut d, v(0.011, 0.013));
        click(&mut ed, &mut d, v(0.031, 0.027));
        assert_eq!(ed.otf.as_ref().map(|o| o.fields.len()), Some(2));
        let u = UnitPrefs::default();
        ed.commit_otf(&mut d, "30", &u, &noshape_units::EmptyScope).unwrap();
        assert_eq!(ed.otf.as_ref().map(|o| o.fields.len()), Some(1), "width box follows");
        ed.commit_otf(&mut d, "10", &u, &noshape_units::EmptyScope).unwrap();
        let bb = d.sketch.bbox();
        assert!((bb.size().x - 0.03).abs() < 1e-9 && (bb.size().y - 0.01).abs() < 1e-9, "{:?}", bb.size());
        // Alt: square
        let (mut ed, mut d) = editor(Tool::CornerRect);
        ed.pointer_move(&mut d, v(0.011, 0.013), false, true);
        click_alt(&mut ed, &mut d, v(0.011, 0.013));
        click_alt(&mut ed, &mut d, v(0.031, 0.020));
        d.sketch.solve();
        let bb = d.sketch.bbox();
        assert!((bb.size().x - bb.size().y).abs() < 1e-9);
    }

    fn click_alt(ed: &mut SketchEditor, data: &mut SketchData, p: DVec2) {
        ed.pointer_move(data, p, false, true);
        ed.tool_down(data, p, None, false, false);
        ed.tool_up(data, p);
    }

    #[test]
    fn shift_a_draws_one_tangent_arc_then_lines() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.011, 0.013));
        click(&mut ed, &mut d, v(0.031, 0.013));
        ed.toggle_tangent_arc();
        assert!(ed.tangent_mode);
        click(&mut ed, &mut d, v(0.041, 0.023));
        assert_eq!(count(&d, EntityKind::Arc), 1);
        assert!(!ed.tangent_mode, "back to lines after one arc");
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Tangent { .. })));
        click(&mut ed, &mut d, v(0.041, 0.043));
        assert_eq!(count(&d, EntityKind::Line), 2);
    }

    #[test]
    fn hovering_back_over_the_chain_end_switches_to_a_tangent_arc() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.011, 0.013));
        click(&mut ed, &mut d, v(0.031, 0.013));
        assert!(!ed.tangent_mode, "the cursor starts on the chain end");
        ed.pointer_move(&mut d, v(0.036, 0.02), false, false);
        ed.pointer_move(&mut d, v(0.031 + PX, 0.013), false, false);
        assert!(ed.tangent_mode, "moving back onto the end switches to the tangent arc");
        click(&mut ed, &mut d, v(0.041, 0.023));
        assert_eq!(count(&d, EntityKind::Arc), 1);
        assert!(!ed.tangent_mode);
    }

    #[test]
    fn spline_closes_on_its_first_point_and_confirm_spline() {
        let (mut ed, mut d) = editor(Tool::Spline);
        for p in [v(0.01, 0.01), v(0.03, 0.01), v(0.03, 0.03), v(0.01 + PX, 0.01)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!(count(&d, EntityKind::FitSpline), 1);
        let s = d.sketch.entities().find(|e| e.kind() == EntityKind::FitSpline).unwrap();
        let refs = s.point_refs();
        assert_eq!(refs.first(), refs.last(), "closed spline");
        for p in [v(0.05, 0.01), v(0.06, 0.02), v(0.07, 0.01)] {
            click(&mut ed, &mut d, p);
        }
        assert!(ed.confirm_spline(&mut d).changed);
        assert_eq!(count(&d, EntityKind::FitSpline), 2);
    }

    #[test]
    fn choosing_a_drawing_tool_clears_the_selection() {
        // Live sketch/401–402: L with a line selected deselects it.
        let (mut ed, mut d) = editor(Tool::Select);
        let line = d.sketch.add_line(v(0.0, 0.0), v(0.04, 0.0));
        ed.selection.push(line);
        ed.set_tool(Tool::Line);
        assert!(ed.selection.is_empty());
        assert_eq!(ed.tool, Tool::Line);
        // Non-drawing tools keep it (Transform acts on the selection).
        ed.set_tool(Tool::Select);
        ed.selection.push(line);
        ed.set_tool(Tool::Transform);
        assert_eq!(ed.selection, vec![line]);
    }

    #[test]
    fn mirror_tool_first_mirrors_each_click_until_esc() {
        let (mut ed, mut d) = editor(Tool::Mirror);
        d.sketch.add_line(v(0.0, 0.0), v(0.0, 0.04));
        d.sketch.add_circle(v(0.01, 0.01), 0.003);
        d.sketch.add_circle(v(0.01, 0.03), 0.003);
        click(&mut ed, &mut d, v(0.0, 0.02));
        assert_eq!(ed.message.as_deref(), Some("Select entities to be mirrored."));
        click(&mut ed, &mut d, v(0.013, 0.01));
        click(&mut ed, &mut d, v(0.013, 0.03));
        assert_eq!(count(&d, EntityKind::Circle), 4);
        assert_eq!(ed.tool, Tool::Mirror);
        ed.cancel();
        assert_eq!(ed.tool, Tool::Select, "Escape exits Mirror");
    }

    #[test]
    fn remaining_geometry_tools_click_sequences() {
        // aligned rectangle: two clicks for the side, one for the width
        let (mut ed, mut d) = editor(Tool::AlignedRect);
        for p in [v(0.01, 0.01), v(0.03, 0.02), v(0.025, 0.03)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!(count(&d, EntityKind::Line), 4);
        assert_eq!(ed.otf.as_ref().map(|o| o.fields.len()), Some(2));
        // 3 point circle: drag 1 -> 2, then click 3
        let (mut ed, mut d) = editor(Tool::ThreePointCircle);
        drag(&mut ed, &mut d, v(0.01, 0.01), v(0.03, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.025));
        assert_eq!(count(&d, EntityKind::Circle), 1);
        // tangent arc tool from a line end
        let (mut ed, mut d) = editor(Tool::TangentArc);
        d.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        click(&mut ed, &mut d, v(0.03, 0.01));
        click(&mut ed, &mut d, v(0.04, 0.02));
        assert_eq!(count(&d, EntityKind::Arc), 1);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Tangent { .. })));
        // center point arc: the sweep follows the cursor (clockwise here)
        let (mut ed, mut d) = editor(Tool::CenterArc);
        click(&mut ed, &mut d, v(0.02, 0.02));
        click(&mut ed, &mut d, v(0.03, 0.02));
        for a in [-0.3f64, -0.8, -1.2] {
            ed.pointer_move(&mut d, v(0.02, 0.02) + DVec2::from_angle(a) * 0.01, false, false);
        }
        click(&mut ed, &mut d, v(0.02, 0.02) + DVec2::from_angle(-1.2) * 0.01);
        let Some(Curve2::Arc { start_angle, end_angle, .. }) = d.sketch.entities().find(|e| e.kind() == EntityKind::Arc).and_then(|e| d.sketch.curve(e.id)) else { panic!() };
        assert!((end_angle - start_angle - 1.2).abs() < 1e-6, "short clockwise arc, {start_angle} {end_angle}");
        // ellipse
        let (mut ed, mut d) = editor(Tool::Ellipse);
        for p in [v(0.02, 0.02), v(0.04, 0.02), v(0.02, 0.03)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!(count(&d, EntityKind::Ellipse), 1);
        // bezier: Esc removes the curve in progress
        let (mut ed, mut d) = editor(Tool::Bezier);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.03));
        assert!(ed.cancel());
        assert_eq!(ed.tool, Tool::Select);
        ed.set_tool(Tool::Bezier);
        click(&mut ed, &mut d, v(0.03, 0.01));
        assert!(ed.clicks.len() == 1 && count(&d, EntityKind::BSpline) == 0);
    }

    #[test]
    fn more_dimension_kinds() {
        let mut sk = Sketch::new();
        let l1 = sk.add_line(v(0.0, 0.0), v(10.0, 0.0));
        let l2 = sk.add_line(v(0.0, 3.0), v(10.0, 3.0));
        assert!(matches!(smart_dimension(&sk, &[l1, l2], v(5.0, 1.0)), Some(ConstraintKind::Distance { mode: DistanceMode::Center, .. })));
        let arc = sk.add_arc(v(20.0, 0.0), v(25.0, 0.0), v(20.0, 5.0));
        let (s, e) = sk.endpoints(arc).unwrap();
        assert!(matches!(smart_dimension(&sk, &[s, e, arc], v(26.0, 6.0)), Some(ConstraintKind::Length { curve, .. }) if curve == arc));
        let el = sk.add_ellipse(v(40.0, 0.0), v(4.0, 0.0), 2.0);
        assert!(matches!(smart_dimension(&sk, &[el], v(46.0, 0.0)), Some(ConstraintKind::EllipseRadius { major: true, .. })));
        assert!(matches!(smart_dimension(&sk, &[el], v(40.0, 4.0)), Some(ConstraintKind::EllipseRadius { major: false, .. })));
        // dimension to the origin
        let (mut ed, mut d) = editor(Tool::Dimension);
        d.sketch.add_point(v(0.02, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01));
        click(&mut ed, &mut d, v(PX, PX));
        assert_eq!(ed.picks.len(), 2, "the origin is pickable");
        click(&mut ed, &mut d, v(0.01, 0.03));
        assert!(d.sketch.constraints().any(|c| c.kind.is_dimension()));
    }

    #[test]
    fn crossing_box_and_drag_to_origin() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.01, 0.01), v(0.05, 0.01));
        // right to left box that only touches the middle of the line
        drag(&mut ed, &mut d, v(0.035, 0.02), v(0.025, 0.0));
        assert!(ed.selection.contains(&l), "crossing selects what the box touches");
        ed.selection.clear();
        drag(&mut ed, &mut d, v(0.025, 0.0), v(0.035, 0.02));
        assert!(!ed.selection.contains(&l), "window needs the whole entity inside");
        // drag a free point onto the origin
        let (mut ed, mut d) = editor(Tool::Select);
        let p = d.sketch.add_point(v(0.01, 0.01));
        drag(&mut ed, &mut d, v(0.01, 0.01), v(PX, -PX));
        let survivor = if d.sketch.entity(p).is_some() { p } else { d.sketch.entities().next().unwrap().id };
        assert_eq!(d.sketch.point(survivor), Some(DVec2::ZERO));
        assert!(d.sketch.entity(survivor).unwrap().external);
        // Shift suppresses the snap
        let (mut ed, mut d) = editor(Tool::Select);
        d.sketch.add_point(v(0.01, 0.01));
        ed.shift_held = true;
        ed.tool_down(&mut d, v(0.01, 0.01), Some(EntityId(0)), false, false);
        for k in 1..=4 {
            ed.tool_drag(&mut d, v(0.01, 0.01).lerp(v(PX, -PX), k as f64 / 4.0));
        }
        ed.tool_up(&mut d, v(PX, -PX));
        assert_eq!(d.sketch.entity_count(), 1, "no merge with Shift held");
    }

    #[test]
    fn expression_detection_and_value_text() {
        for t in ["10*3", "#d", "#w / 2", "sin(30 deg) * 10", "(3)"] {
            assert!(is_expression(t), "{t}");
        }
        for t in ["25", "25 mm", "-3.5in", "0.5"] {
            assert!(!is_expression(t), "{t}");
        }
        let u = UnitPrefs::default();
        let k = ConstraintKind::EllipseRadius { ellipse: EntityId(0), major: true, value: 0.0 };
        assert!(dim_value_text(&k, 0.01, &u).starts_with('Ø'));
        assert!(dim_value_text(&k, 0.01, &u).contains("20"), "ellipse axes show diameters");
    }

    #[test]
    fn fillet_two_curves() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let a = d.sketch.add_point(v(0.0, 0.0));
        let b = d.sketch.add_point(v(0.04, 0.0));
        let c = d.sketch.add_point(v(0.04, 0.04));
        d.sketch.add_line_between(a, b).unwrap();
        d.sketch.add_line_between(b, c).unwrap();
        click(&mut ed, &mut d, v(0.02, 0.0));
        assert_eq!(count(&d, EntityKind::Arc), 0, "the first curve only picks");
        click(&mut ed, &mut d, v(0.04, 0.02));
        assert_eq!(count(&d, EntityKind::Arc), 0, "the second curve stages the preview");
        assert!(ed.corner_pending());
        assert!(ed.dim_edit.is_none(), "the radius box waits for Enter");
        assert!(ed.commit_corner(&mut d).changed);
        assert_eq!(count(&d, EntityKind::Arc), 1);
        assert!(ed.dim_edit.is_some(), "Enter opens the radius box");
    }

    /// SK5 in the editor (live, qa5 sketch tools): Corner rectangle
    /// 30.089 × 22.065, fillet the top-right corner (R5), type 3 / 8 in its
    /// box: the top keeps 25.089, the height stays, the right edge follows.
    #[test]
    fn fillet_radius_edit_in_the_editor_moves_like_onshape() {
        for (text, bottom, right) in [("3", 28.089, 19.065), ("8", 33.089, 14.065)] {
            let (mut ed, mut d) = editor(Tool::CornerRect);
            ed.apply_unit_defaults(noshape_units::LengthUnit::Millimeter);
            click(&mut ed, &mut d, v(0.0, 0.0));
            click(&mut ed, &mut d, v(0.030089, 0.022065));
            ed.set_tool(Tool::Fillet);
            click(&mut ed, &mut d, v(0.030089, 0.022065));
            assert!(ed.commit_corner(&mut d).changed);
            let cid = ed.dim_edit.as_ref().expect("radius box").constraint;
            ed.apply_dimension(&mut d, cid, text, &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
            let mut lens: Vec<(DVec2, f64)> = d
                .sketch
                .entities()
                .filter(|e| e.kind() == EntityKind::Line && !e.construction)
                .filter_map(|e| {
                    let c = d.sketch.curve(e.id)?;
                    Some(((c.start_point() + c.end_point()) * 0.5, (c.end_point() - c.start_point()).length() * 1000.0))
                })
                .collect();
            lens.sort_by(|a, b| a.0.y.total_cmp(&b.0.y).then(a.0.x.total_cmp(&b.0.x)));
            // bottom, right (shortened from the top), left, top by midpoint height.
            let got: Vec<f64> = lens.iter().map(|l| (l.1 * 1000.0).round() / 1000.0).collect();
            assert_eq!(got, vec![bottom, right, 22.065, 25.089], "R{text}: {lens:?}");
        }
    }

    /// Live (behaviours.md 568): the fillet default is R5 in a millimetre
    /// workspace, and a radius typed into the first fillet's box becomes the
    /// next default.
    #[test]
    fn fillet_default_r5_then_sticky() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        ed.apply_unit_defaults(noshape_units::LengthUnit::Millimeter);
        assert_eq!(ed.fillet_radius, 0.005);
        let a = d.sketch.add_point(v(0.0, 0.0));
        let b = d.sketch.add_point(v(0.04, 0.0));
        let c = d.sketch.add_point(v(0.0, 0.03));
        d.sketch.add_line_between(a, b).unwrap();
        d.sketch.add_line_between(a, c).unwrap();
        click(&mut ed, &mut d, v(0.0, 0.0));
        assert!(ed.commit_corner(&mut d).changed);
        let cid = ed.dim_edit.as_ref().expect("radius box").constraint;
        ed.apply_dimension(&mut d, cid, "3", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        assert!((ed.fillet_radius - 0.003).abs() < 1e-12);
    }

    /// CH1 (live sketch/1959–1962): a corner click previews the 5 mm
    /// chamfer with two distance labels and the drag arrow; Enter commits it
    /// and opens distance 1's box, accepting that opens distance 2's.
    #[test]
    fn chamfer_preview_then_two_value_boxes() {
        let (mut ed, mut d) = editor(Tool::Chamfer);
        let a = d.sketch.add_point(v(0.0, 0.0));
        let b = d.sketch.add_point(v(0.04, 0.0));
        let c = d.sketch.add_point(v(0.0, 0.03));
        d.sketch.add_line_between(a, b).unwrap();
        d.sketch.add_line_between(a, c).unwrap();
        click(&mut ed, &mut d, v(0.0, 0.0));
        assert!(ed.corner_pending() && ed.dim_edit.is_none(), "preview, no value box yet");
        let labels = ed.chamfer_value_labels();
        assert_eq!(labels.len(), 2);
        assert!(labels.iter().all(|(at, v)| (v - 0.005).abs() < 1e-12 && (at.x < 0.0 || at.y < 0.0)), "labels outside the corner: {labels:?}");
        let (start, apex) = ed.chamfer_arrow().expect("drag arrow");
        assert!((start.length() - 0.005).abs() < 1e-9 && (apex - start).length() > 0.0);
        // Dragging the arrow sets both distances.
        ed.drag = Some(DragState::ChamferDistance);
        let to = start + (apex - start).normalize() * 0.003;
        ed.tool_drag(&mut d, to);
        assert!(ed.chamfer_value_labels().iter().all(|(_, v)| (v - 0.008).abs() < 1e-9), "{:?}", ed.chamfer_value_labels());
        ed.drag = None;
        assert!(ed.commit_corner(&mut d).changed);
        let first = ed.dim_edit.as_ref().expect("distance 1 box").constraint;
        ed.dim_edit = None;
        assert!(ed.open_next_value_box(&d));
        assert_ne!(ed.dim_edit.as_ref().unwrap().constraint, first, "then distance 2");
        assert!(!ed.open_next_value_box(&d));
    }

    #[test]
    fn text_dialog_is_provisional_and_validates_character_limit() {
        let (mut ed, mut d) = editor(Tool::Text);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.03, 0.02));
        assert!(ed.pending_text.is_some());
        assert!(d.texts.is_empty());
        assert!(ed.cancel());
        assert!(ed.pending_text.is_none());
        assert!(d.texts.is_empty(), "cancel never adds text geometry");
        ed.set_tool(Tool::Text);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.03, 0.02));
        ed.text_string = "é".repeat(251);
        assert!(!ed.accept_text(&mut d).changed);
        assert!(ed.pending_text.is_some());
        assert!(d.texts.is_empty());
        ed.text_string = "é".repeat(250);
        ed.text_flip_horizontal = true;
        assert!(ed.accept_text(&mut d).changed);
        assert_eq!(d.texts.len(), 1);
        assert!(d.texts[0].mirror_horizontal);
        assert_eq!(ed.tool, Tool::Select);
        // Double-clicking the text reopens it in the dialog (live Onshape).
        let line = EntityId(d.texts[0].box_lines[0]);
        let at = d.sketch.curve(line).unwrap().point(0.5);
        ed.tool_down(&mut d, at, Some(line), false, true);
        assert_eq!(ed.editing_text, Some(d.texts[0].id));
        assert!(ed.text_dialog_open() && ed.text_flip_horizontal, "dialog shows the text's style");
        ed.text_string = "Changed".into();
        assert!(ed.cancel(), "Esc / × closes the dialog");
        assert!(d.texts[0].text.starts_with('é'), "cancel keeps the text");
        ed.tool_down(&mut d, at, Some(line), false, true);
        ed.text_string = "Changed".into();
        ed.text_expression = false;
        assert!(ed.accept_text(&mut d).changed);
        assert_eq!(d.texts.len(), 1, "edited in place");
        assert_eq!(d.texts[0].text, "Changed");
        assert!(ed.editing_text.is_none());
    }

    #[test]
    fn a_hovered_glyph_highlights_orange_and_a_mirrored_pairs_lines_yellow() {
        let (mut ed, mut d) = editor(Tool::Select);
        let a = d.sketch.add_point(v(-0.01, 0.0));
        let b = d.sketch.add_point(v(0.01, 0.0));
        let axis = d.sketch.add_line(v(0.0, -0.01), v(0.0, 0.01));
        let cid = d.sketch.add_constraint(ConstraintKind::Symmetric { a, b, axis }).unwrap();
        ed.hover_constraint = Some(cid);
        let (lines, points) = ed.overlay(&d, &noshape_units::UnitPrefs::default());
        let orange = crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT);
        // Live (sketch/2203–2217): two free points about a line: all orange, no yellow.
        let yellow = [0xf2, 0xff, 0x80, 255];
        assert_eq!(points.points.iter().filter(|p| p.color == orange).count(), 2, "both points orange");
        assert!(lines.segments.iter().any(|s| s.color == orange) && !lines.segments.iter().any(|s| s.color == yellow), "the symmetry line orange");
        assert!(!lines.segments.iter().any(|s| s.color == [0xf2, 0xff, 0x80, 150]), "no translucent halo for a hovered glyph");
        // A Mirror-made point pair: the lines owning the points turn yellow.
        let (p1, p2) = (d.sketch.add_point(v(-0.02, 0.01)), d.sketch.add_point(v(0.02, 0.01)));
        let l1 = d.sketch.add_line_between(a, p1).unwrap();
        let l2 = d.sketch.add_line_between(b, p2).unwrap();
        let (lines, _) = ed.overlay(&d, &noshape_units::UnitPrefs::default());
        let yellow_lines = lines.segments.iter().filter(|s| s.color == yellow).count();
        assert!(yellow_lines >= 2, "{l1:?} {l2:?} yellow");
    }

    #[test]
    fn hatched_text_is_a_filled_digit_along_its_direction() {
        let px = 1e-4;
        let bounds = |segs: &[[DVec2; 2]]| {
            let pts: Vec<DVec2> = segs.iter().flatten().copied().collect();
            let (mut lo, mut hi) = (pts[0], pts[0]);
            for p in &pts {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            (hi - lo) / px
        };
        let flat = hatched_text("6", DVec2::ZERO, DVec2::X, px, 16.0);
        assert!(flat.len() > 15, "scanlines fill the glyph");
        let size = bounds(&flat);
        // A 16 px digit: ~9 px wide, ~12 px tall (live sketch/91).
        assert!((7.0..12.0).contains(&size.x) && (10.0..14.0).contains(&size.y), "{size:?}");
        // Laid along a vertical ray it turns: tall becomes wide.
        let turned = bounds(&hatched_text("6", DVec2::ZERO, DVec2::Y, px, 16.0));
        assert!((size.x - turned.y).abs() < 1.0 && (size.y - turned.x).abs() < 1.0, "{turned:?}");
    }

    #[test]
    fn fillet_preview_cancel_preserves_original_geometry() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let rect = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.02));
        let original = d.sketch.clone();
        let p = d.sketch.point(rect.points[0]).unwrap();
        click(&mut ed, &mut d, p);
        assert!(ed.corner_pending());
        assert!(ed.cancel());
        assert!(!ed.corner_pending());
        assert_eq!(count(&d, EntityKind::Arc), 0);
        for e in original.entities() {
            assert_eq!(original.curve(e.id), d.sketch.curve(e.id));
            assert_eq!(original.point(e.id), d.sketch.point(e.id));
        }
    }

    #[test]
    fn linear_pattern_second_direction_links_grid_and_round_trips_edit() {
        let (mut ed, mut d) = editor(Tool::LinearPattern);
        let c = d.sketch.add_circle(v(0.01, 0.01), 0.002);
        ed.selection = vec![c];
        assert_eq!(ed.pattern_count, 3);
        assert_eq!(ed.pattern_second_count, 1);
        ed.pattern_distance = 0.01;
        ed.pattern_second_distance = 0.015;
        ed.pattern_second_count = 2;
        assert_eq!(ed.pattern_transforms().len(), 5);
        assert!(ed.apply_pattern(&mut d).changed);
        assert_eq!(count(&d, EntityKind::Circle), 6);
        assert!(d.sketch.solve().is_solved());
        let center = d.sketch.entity(c).unwrap().center().unwrap();
        d.sketch.set_point(center, v(0.012, 0.013)).unwrap();
        assert!(d.sketch.solve().is_solved());
        let actual: Vec<DVec2> = d.sketch.entities().filter_map(|e| match d.sketch.curve(e.id) { Some(Curve2::Circle { center, .. }) => Some(center), _ => None }).collect();
        // Translation is shared by all rows; relative grid offsets remain exact.
        let origin = d.sketch.point(center).unwrap();
        for j in 0..2 {
            for i in 0..3 {
                let target = origin + v(i as f64 * 0.01, j as f64 * 0.015);
                assert!(actual.iter().any(|p| (*p - target).length() < 1e-7), "missing {target:?} in {actual:?}");
            }
        }
        let id = d.patterns[0].id;
        assert!(ed.edit_pattern(&d, id));
        assert_eq!(ed.pattern_second_count, 2);
        assert!((ed.pattern_second_distance - 0.015).abs() < 1e-12);
        ed.pattern_second_count = 3;
        assert!(ed.apply_pattern(&mut d).changed);
        assert_eq!(count(&d, EntityKind::Circle), 9);
        assert!(d.sketch.solve().is_solved());
    }

    #[test]
    fn offset_preview_arrow_and_repeat() {
        let (mut ed, mut d) = editor(Tool::Offset);
        let l1 = d.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        d.sketch.add_line(v(0.01, 0.03), v(0.03, 0.03));
        click(&mut ed, &mut d, v(0.02, 0.01));
        assert!(ed.offset_pending.is_some(), "picking a curve previews the offset");
        assert_eq!(count(&d, EntityKind::Line), 2, "nothing is created yet");
        let (base, tip) = ed.offset_arrow(&d.sketch).unwrap();
        // drag the arrow to the other side: 4 mm below the line
        ed.pointer_move(&mut d, tip, false, false);
        ed.tool_down(&mut d, tip, None, false, false);
        for k in 1..=4 {
            ed.tool_drag(&mut d, tip.lerp(v(0.02, 0.006), k as f64 / 4.0));
        }
        ed.tool_up(&mut d, v(0.02, 0.006));
        assert!((ed.offset_distance - 0.004).abs() < 1e-9);
        let _ = base;
        assert!(ed.commit_offset(&mut d).changed);
        let below = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).filter_map(|e| d.sketch.curve(e.id)).any(|c| (c.start_point().y - 0.006).abs() < 1e-9);
        assert!(below, "offset toward the dragged side");
        // later clicks repeat the offset right away
        click(&mut ed, &mut d, v(0.02, 0.03));
        assert_eq!(count(&d, EntityKind::Line), 4);
        let _ = l1;
    }

    #[test]
    fn pattern_accepts_on_click_in_space_and_edit_pattern() {
        let (mut ed, mut d) = editor(Tool::Select);
        let c = d.sketch.add_circle(v(0.01, 0.01), 0.002);
        ed.selection = vec![c];
        ed.set_tool(Tool::LinearPattern);
        ed.pattern_count = 3;
        ed.pattern_distance = 0.01;
        ed.pattern_angle = 0.0;
        click(&mut ed, &mut d, v(0.04, 0.04));
        assert_eq!(count(&d, EntityKind::Circle), 3, "click in empty space accepts");
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::PatternLink { .. })), "instances are linked to the seed");
        assert!(d.sketch.solve().is_solved());
        assert_eq!(d.patterns.len(), 1);
        let pid = d.patterns[0].id;
        assert!(ed.edit_pattern(&d, pid));
        ed.pattern_count = 5;
        assert!(ed.apply_pattern(&mut d).changed);
        assert_eq!(count(&d, EntityKind::Circle), 5, "edit replaces the old instances");
        assert_eq!(d.patterns.len(), 1);
        // Esc cancels a pattern being set up
        ed.selection = vec![c];
        assert!(ed.cancel());
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn fillet_radius_manipulator_and_conic_rho_box() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.02));
        let p0 = d.sketch.point(r.points[0]).unwrap();
        click(&mut ed, &mut d, p0);
        let (arc, m) = ed.fillet_handle(&d.sketch).expect("manipulator");
        ed.dim_edit = None;
        ed.tool_down(&mut d, m, None, false, false);
        let target = m + (m - p0).normalize() * -0.0005;
        for k in 1..=4 {
            ed.tool_drag(&mut d, m.lerp(target, k as f64 / 4.0));
        }
        ed.tool_up(&mut d, target);
        assert_eq!(count(&d, EntityKind::Arc), 0, "dragging the preview leaves the document untouched");
        assert!(ed.commit_corner(&mut d).changed);
        let Some(Curve2::Arc { radius, .. }) = d.sketch.curve(arc) else { panic!() };
        assert!((radius - 0.005).abs() > 1e-6, "dragging changed the radius");
        let (mut ed, mut d) = editor(Tool::Conic);
        for p in [v(0.01, 0.01), v(0.03, 0.01), v(0.02, 0.02)] {
            click(&mut ed, &mut d, p);
        }
        assert!(matches!(ed.otf.as_ref().and_then(|o| o.fields.first()).map(|f| &f.kind), Some(ConstraintKind::Rho { .. })));
        ed.commit_otf(&mut d, "0.3", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        let conic = d.sketch.entities().find(|e| e.kind() == EntityKind::Conic).unwrap();
        assert!(matches!(conic.geometry, noshape_sketch::Geometry::Conic { rho, .. } if (rho - 0.3).abs() < 1e-12));
    }

    #[test]
    fn rim_picks_measure_min_and_max() {
        let mut sk = Sketch::new();
        let p = sk.add_point(v(0.0, 0.0));
        let c = sk.add_circle(v(10.0, 0.0), 2.0);
        let k = ConstraintKind::Distance { a: p, b: c, value: 0.0, mode: DistanceMode::Center };
        assert!(matches!(circle_mode(&sk, k.clone(), &[(c, v(8.0, 0.0))], 0.01), ConstraintKind::Distance { mode: DistanceMode::Minimum, .. }));
        assert!(matches!(circle_mode(&sk, k.clone(), &[(c, v(12.0, 0.0))], 0.01), ConstraintKind::Distance { mode: DistanceMode::Maximum, .. }));
        assert!(matches!(circle_mode(&sk, k, &[(c, v(10.0, 0.0))], 0.01), ConstraintKind::Distance { mode: DistanceMode::Center, .. }));
    }

    #[test]
    fn coincident_on_already_coincident_points_adds_nothing() {
        let mut sk = Sketch::new();
        let (a, b, c) = (sk.add_point(v(0.0, 0.0)), sk.add_point(v(1.0, 0.0)), sk.add_point(v(2.0, 0.0)));
        assert_eq!(apply_constraint(&mut sk, ConstraintTool::Coincident, &[a, b]), Ok(1));
        assert_eq!(apply_constraint(&mut sk, ConstraintTool::Coincident, &[b, c]), Ok(1));
        // a-c are joined through b: nothing to add (live: no glyph, no message).
        assert!(already_coincident(&sk, ConstraintTool::Coincident, &[a, c]));
        assert_eq!(apply_constraint(&mut sk, ConstraintTool::Coincident, &[a, c]), Ok(0));
        let mut data = SketchData { sketch: sk, ..Default::default() };
        let n = data.sketch.constraints().count();
        let mut ed = SketchEditor::new(FeatureId(1), PlaneFrame::TOP);
        ed.selection = vec![c, a];
        assert!(!ed.constraint_command(&mut data, ConstraintTool::Coincident));
        assert_eq!(data.sketch.constraints().count(), n);
        assert!(ed.message.is_none());
        assert_ne!(ed.tool, Tool::Constraint(ConstraintTool::Coincident));
    }

    #[test]
    fn select_first_constraints_apply_to_the_whole_selection() {
        let mut sk = Sketch::new();
        let l1 = sk.add_line(v(0.0, 0.0), v(1.0, 0.1));
        let l2 = sk.add_line(v(0.0, 1.0), v(1.0, 1.3));
        let l3 = sk.add_line(v(0.0, 2.0), v(1.0, 2.2));
        let c = sk.add_circle(v(5.0, 0.0), 1.0);
        assert!(constraint_applicable(&sk, ConstraintTool::Parallel, &[l1, l2, l3]));
        assert!(!constraint_applicable(&sk, ConstraintTool::Parallel, &[l1, c]));
        assert!(!constraint_applicable(&sk, ConstraintTool::Perpendicular, &[l1, l2, l3]));
        assert!(constraint_applicable(&sk, ConstraintTool::Tangent, &[l1, c]));
        assert!(constraint_applicable(&sk, ConstraintTool::Horizontal, &[l1, l2]));
        assert_eq!(apply_constraint(&mut sk, ConstraintTool::Parallel, &[l1, l2, l3]), Ok(2));
        assert_eq!(apply_constraint(&mut sk, ConstraintTool::Horizontal, &[l1]), Ok(1));
        assert!(sk.solve().is_solved());
        for l in [l1, l2, l3] {
            let (a, b) = line_of(&sk, l).unwrap();
            assert!((a.y - b.y).abs() < 1e-9);
        }
    }

    #[test]
    fn dimension_tool_infers_type_from_picks_and_cursor() {
        let mut sk = Sketch::new();
        let axis = sk.add_line(v(0.0, -1.0), v(0.0, 1.0));
        sk.set_construction(axis, true).unwrap();
        let p = sk.add_point(v(2.0, 0.5));
        assert!(matches!(smart_dimension(&sk, &[p, axis], v(3.0, 0.0)), Some(ConstraintKind::Distance { mode: DistanceMode::Center, .. })));
        assert!(matches!(smart_dimension(&sk, &[p, axis], v(-1.0, 0.0)), Some(ConstraintKind::Distance { mode: DistanceMode::Centerline, .. })));
        let a = sk.add_point(v(0.0, 0.0));
        let b = sk.add_point(v(4.0, 3.0));
        assert!(matches!(smart_dimension(&sk, &[a, b], v(2.0, 5.0)), Some(ConstraintKind::HorizontalDistance { .. })));
        assert!(matches!(smart_dimension(&sk, &[a, b], v(6.0, 1.0)), Some(ConstraintKind::VerticalDistance { .. })));
        assert!(matches!(smart_dimension(&sk, &[a, b], v(1.0, 2.5)), Some(ConstraintKind::Distance { .. })));
        let l1 = sk.add_line(v(0.0, 0.0), v(10.0, 0.0));
        let l2 = sk.add_line(v(0.0, 0.0), v(10.0, 10.0));
        let Some(ConstraintKind::Angle { value, .. }) = smart_dimension(&sk, &[l1, l2], v(5.0, 1.0)) else { panic!() };
        assert!((value - PI / 4.0).abs() < 1e-9);
        let Some(ConstraintKind::Angle { value, .. }) = smart_dimension(&sk, &[l1, l2], v(-5.0, 1.0)) else { panic!() };
        assert!((value - 3.0 * PI / 4.0).abs() < 1e-9, "obtuse quadrant");
    }

    #[test]
    fn dimension_click_sequence_opens_the_value_box() {
        let (mut ed, mut d) = editor(Tool::Dimension);
        let l = d.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01));
        assert!(ed.dimension_preview(&d).is_none() || ed.picked(None) == vec![l]);
        ed.pointer_move(&mut d, v(0.02, 0.015), false, false);
        let (k, _, val) = ed.dimension_preview(&d).expect("preview follows the cursor");
        assert!(matches!(k, ConstraintKind::Length { .. }));
        assert!((val - 0.02).abs() < 1e-12);
        click(&mut ed, &mut d, v(0.02, 0.015));
        assert!(ed.dim_edit.is_some(), "placement opens the value box");
        assert_eq!(ed.tool, Tool::Dimension, "tool stays active");
        let cid = ed.dim_edit.as_ref().unwrap().constraint;
        ed.apply_dimension(&mut d, cid, "-30", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        let c = d.sketch.curve(l).unwrap();
        assert!((c.length() - 0.03).abs() < 1e-9);
        assert!(c.end_point().x < c.start_point().x, "negative length flips the line");
    }

    #[test]
    fn a_new_dimension_leaves_nothing_selected() {
        // Onshape: the Dimension tool's commit resets the selection list
        // (bundle `cleanUpAfterCommit`; live sketch/15).
        let (mut ed, mut d) = editor(Tool::Dimension);
        let l = d.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01));
        ed.pointer_move(&mut d, v(0.02, 0.015), false, false);
        click(&mut ed, &mut d, v(0.02, 0.015));
        let cid = ed.dim_edit.as_ref().expect("value box").constraint;
        assert!(ed.dim_edit.as_ref().unwrap().created);
        ed.selection = vec![l];
        assert!(ed.cancel());
        assert!(ed.selection.is_empty() && ed.selected_constraints.is_empty());
        // Editing an existing dimension keeps the selection.
        ed.selected_constraints = vec![cid];
        ed.dim_edit = Some(DimEdit { constraint: cid, text: "20 mm".into(), created: false });
        assert!(ed.cancel());
        assert_eq!(ed.selected_constraints, vec![cid]);
    }

    #[test]
    fn trim_drag_and_point_delete() {
        let (mut ed, mut d) = editor(Tool::Trim);
        d.sketch.add_line(v(0.0, 0.01), v(0.04, 0.01));
        d.sketch.add_line(v(0.0, 0.02), v(0.04, 0.02));
        let pt = d.sketch.add_point(v(0.05, 0.05));
        drag(&mut ed, &mut d, v(0.02, 0.0), v(0.02, 0.03));
        assert_eq!(count(&d, EntityKind::Line), 0, "both lines crossed by the path are trimmed away");
        click(&mut ed, &mut d, v(0.05, 0.05));
        assert!(d.sketch.entity(pt).is_none());
    }

    #[test]
    fn trim_drag_across_a_rectangle_removes_only_crossed_sides() {
        let (mut ed, mut d) = editor(Tool::Trim);
        let r = d.sketch.corner_rectangle(v(0.01, 0.01), v(0.03, 0.025));
        drag(&mut ed, &mut d, v(0.015, 0.005), v(0.015, 0.03));
        let alive: Vec<bool> = r.curves.iter().map(|c| d.sketch.entity(*c).is_some()).collect();
        assert_eq!(alive, vec![false, true, false, true]);
    }

    #[test]
    fn polygon_sides_typed_and_midpoint_line() {
        let (mut ed, mut d) = editor(Tool::Polygon);
        click(&mut ed, &mut d, v(0.011, 0.013));
        ed.polygon_sides = 5;
        click(&mut ed, &mut d, v(0.031, 0.013));
        assert!(ed.polygon_sides_pending(), "the circumference is locked first");
        // moving away from the polygon adds sides, back restores them
        ed.pointer_move(&mut d, v(0.031 + PX * 31.0, 0.013), false, false);
        assert_eq!(ed.polygon_sides, 7);
        ed.pointer_move(&mut d, v(0.031, 0.013), false, false);
        assert_eq!(ed.polygon_sides, 5);
        click(&mut ed, &mut d, v(0.031, 0.013));
        assert_eq!(count(&d, EntityKind::Line), 5);
        assert!(ed.otf.is_some(), "diameter box");
        let (mut ed, mut d) = editor(Tool::MidpointLine);
        click(&mut ed, &mut d, v(0.021, 0.023));
        click(&mut ed, &mut d, v(0.031, 0.027));
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).unwrap().id;
        let c = d.sketch.curve(line).unwrap();
        assert!(((c.start_point() + c.end_point()) * 0.5 - v(0.021, 0.023)).length() < 1e-9);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Midpoint { .. })));
    }

    #[test]
    fn elliptical_arc_four_clicks() {
        let (mut ed, mut d) = editor(Tool::EllipticalArc);
        for p in [v(0.011, 0.013), v(0.031, 0.013), v(0.011, 0.023), v(0.031, 0.0131)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!(count(&d, EntityKind::EllipticalArc), 1);
        assert_eq!(ed.otf.as_ref().map(|o| o.fields.len()), Some(2));
    }

    #[test]
    fn fillet_vertex_then_equal_fillets() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.02));
        let (p0, p1) = (d.sketch.point(r.points[0]).unwrap(), d.sketch.point(r.points[1]).unwrap());
        click(&mut ed, &mut d, p0);
        assert!(ed.commit_corner(&mut d).changed);
        assert!(ed.dim_edit.is_some(), "radius box after the first fillet");
        click(&mut ed, &mut d, p1);
        assert!(ed.commit_corner(&mut d).changed);
        assert_eq!(count(&d, EntityKind::Arc), 2);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Equal { .. })));
    }

    #[test]
    fn mirror_with_preselection_resolves_and_ends() {
        let (mut ed, mut d) = editor(Tool::Select);
        let axis = d.sketch.add_line(v(0.0, 0.0), v(0.0, 0.04));
        let c = d.sketch.add_circle(v(0.01, 0.02), 0.003);
        ed.selection = vec![c];
        ed.set_tool(Tool::Mirror);
        click(&mut ed, &mut d, v(0.0, 0.03));
        let _ = axis;
        assert_eq!(count(&d, EntityKind::Circle), 2);
        assert_eq!(ed.tool, Tool::Select);
    }

    #[test]
    fn copy_paste_selection() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.0, 0.0), v(0.01, 0.0));
        ed.selection = vec![l];
        let clip = ed.copy_selection(&d).unwrap();
        assert!(ed.paste_clipboard(&mut d, &clip));
        assert_eq!(count(&d, EntityKind::Line), 2);
        assert_eq!(ed.selection.len(), 1);
        assert_ne!(ed.selection[0], l);
        assert!(!ed.paste_clipboard(&mut d, "hello"));
    }
}
