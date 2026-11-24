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
    Flow,
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
            ConstraintTool::Flow => "Flow",
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
            ConstraintTool::Flow => "flow",
        }
    }
    /// Entities needed before the constraint can be created.
    pub fn arity(self) -> usize {
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

/// How near (screen px) a 3 point arc's third click takes the semicircle over its ends (live2 arcmid: 7.5 yes, 8 no).
const ARC3_SEMICIRCLE_PX: f64 = 7.75;

#[derive(Clone, Debug)]
enum DragState {
    /// Pressed on an entity; becomes a geometry drag when moved.
    Pending { entity: EntityId, start: DVec2 },
    /// `at`: the pointer the last step solved for (a repeated pointer
    /// position isn't solved again).
    Geometry { handle: DragHandle, at: Option<DVec2> },
    /// Pressed on a dimension label: moves it when dragged, selects it on release.
    /// (`dragging`: past the drag start, so the label follows the pointer wherever it goes after.)
    Label { constraint: ConstraintId, start: DVec2, moved: bool, additive: bool, dragging: bool },
    /// Pressed on a constraint glyph: moves it when dragged, selects it on release.
    Glyph { constraint: ConstraintId, entity: EntityId, start: DVec2, orig: DVec2, moved: bool, additive: bool },
    /// Dragging an on-canvas pattern / transform manipulator.
    Handle(PatternHandle),
    /// Pressed on a text (glyph or box line): moves the whole box when dragged.
    Text { entity: EntityId, start: DVec2, points: Vec<(EntityId, DVec2)>, moved: bool },
    Box { start: DVec2 },
    /// Dragging (or clicking) the offset direction arrow.
    /// `from`: the offset at the press, signed along the first curve's left normal.
    OffsetArrow { start: DVec2, moved: bool, from: f64 },
    /// Dragging the fillet radius manipulator (the session's first fillet).
    FilletRadius { arc: EntityId },
    /// The arrow of an invalid (red) fillet preview ([`SketchEditor::invalid_corner`]).
    FilletInvalid,
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
    /// Transform: translation in the plane (the square).
    Move,
    /// Transform: translation along the triad's X / Y arrow.
    MoveX,
    MoveY,
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
    /// Where live puts the value while it is an idle value (blue text),
    /// and while it is the active box (behaviours "skasm (2026-10-27) sketch
    /// value boxes right after drawing — rule capture"); `None`: `label`.
    pub place: Option<BoxPlace>,
    pub place_active: Option<BoxPlace>,
    /// The dimension's label off a line's middle at commit time (the line
    /// may have moved since, e.g. a rectangle's edge once its width is
    /// typed): the line, the unit direction and the screen distance.
    pub label_at: Option<(EntityId, DVec2, f64)>,
}

/// A value box's place relative to its geometry, in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxPlace {
    /// Beside the circle (or arc's circle) on side `u` (unit vector), the
    /// box's near edge 14 px off the rim.
    Rim(DVec2),
    /// Beside a line's midpoint on side `u`, the near edge `gap` px off the
    /// line (inside a shape when `u` points into it).
    Edge { line: EntityId, u: DVec2, gap: f64 },
    /// Centred on a point.
    At(DVec2),
}

/// On-the-fly dimension boxes of the last created geometry (Onshape:
/// "the suggested dimension value appears in a box"; Alt+arrows switch).
#[derive(Clone, Debug)]
pub struct OnTheFly {
    pub fields: Vec<OtfField>,
    pub active: usize,
    /// A box of these has been made a dimension: Enter on the next, untouched one does nothing (live 2025: an
    /// ellipse's minor typed and entered, then Enter on the major makes no dimension).
    pub committed: bool,
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

/// How far inference reaches on screen, in pixels: snapping to points and curves, a line turning horizontal or
/// vertical. Live draws its inferences 15 px wide (woolsthorpe INFERENCE_POINT_SIZE_PX / INFERENCE_LINE_WIDTH_PX) and
/// picks the pointer's pixel (INFERENCE_PICK_RADIUS_PX 0).
const INFERENCE_PX: f64 = 7.5;
/// A snap that has lit holds until the pointer is this far from it on screen (live 11-17 sketch-snap-1117: points let
/// go at 9–10 px straight out and 11–12 diagonally, curves, edges and the horizontal / vertical bands at 8–10).
const HOLD_PX: f64 = 9.5;
const HOLD_DIAGONAL_PX: f64 = 11.5;
/// A line's own horizontal / vertical from its start lets go at the low end of that (ui-parity sketch-vertical-key:
/// (0, 0) to (3, 30) on Top, coming in 8 moves through the band, ends 9 px off plumb and isn't vertical).
const BAND_HOLD_PX: f64 = 8.0;
/// How long the pointer rests on a model edge before it wakes (live sketch-gaps 1c: 60 ms did, 30 didn't).
const EDGE_WAKE_REST: std::time::Duration = std::time::Duration::from_millis(60);
/// Stands for the woken model edge in an inference's constraints until the click uses the edge
/// ([`SketchEditor::link_woken_edge`]).
pub const WOKEN_EDGE: EntityId = EntityId(u32::MAX - 1);
/// Stand for the woken model edge's ends in an inference's alignments until the click uses the edge.
const WOKEN_EDGE_ENDS: [EntityId; 2] = [EntityId(u32::MAX - 2), EntityId(u32::MAX - 3)];
/// Stands for the woken point outside the sketch (another sketch's point, a model vertex) in an inference's alignments
/// until the click uses it.
const WOKEN_POINT: EntityId = EntityId(u32::MAX - 4);
/// Stand for the midpoints of lines woken while drawing ([`SketchEditor::woken_mids`]) in an inference's alignments until
/// the click makes a point there.
const WOKEN_MIDS: [EntityId; 4] = [EntityId(u32::MAX - 5), EntityId(u32::MAX - 6), EntityId(u32::MAX - 7), EntityId(u32::MAX - 8)];
/// Stands for a model edge's midpoint a level crosses ([`SketchEditor::woken_edge_mid`]) in an inference's alignment
/// until the click makes a point there.
const WOKEN_EDGE_MID: EntityId = EntityId(u32::MAX - 9);
/// Trim: crossings this close to a chain's corner don't count (live sktools-1118 ~1 px, recheck b5b664aa 0.6 px).
const TRIM_CORNER_PX: f64 = 1.5;
/// How far the pointer moves from a press, on screen, before the press is a drag (live2 "feel (sketch)" (5): a drag
/// starts at about 5 px on an end point or a line, whatever the time; less is a click).
const DRAG_START_PX: f64 = 5.0;
/// The square round a snapped point: its 1 px sides 8 px from the point (live qsnap/za-p2p-d8: 17 px outside), #FFC92A.
const SNAP_SQUARE_HALF_PX: f64 = 8.0;
const SNAP_SQUARE_COLOR: [u8; 4] = [0xff, 0xc9, 0x2a, 0xff];
/// A manipulator's drag step (live getLinearSnapValue, as the feature arrows' `linear_step`): 10^(round(log10(100 px
/// in mm)) − 2) mm; 0.1 mm at live's sketch-tools-1118 scale (5.31 px/mm: 10.1 mm), 1 mm zoomed out.
fn drag_step(m_per_px: f64) -> f64 {
    let span = 100.0 * m_per_px / 1e-3;
    if !(span.is_finite() && span > 0.0) {
        return 1e-4;
    }
    10f64.powf(span.log10().round() - 2.0) * 1e-3
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

/// Glyph ids of points shared by curves' ends (a rectangle's corners, a line
/// chain's joints): ours shares the point where Onshape stores a Coincident,
/// so the glyph stands for the sharing.
const SHARED_GLYPH_BASE: u32 = 0xA000_0000;

/// The glyph id of a shared point's Coincident.
pub fn shared_glyph_id(p: EntityId) -> ConstraintId {
    ConstraintId(SHARED_GLYPH_BASE | p.0)
}

/// The shared point of a shared-point glyph id.
pub fn shared_glyph_point(c: ConstraintId) -> Option<EntityId> {
    (c.0 & 0xE000_0000 == SHARED_GLYPH_BASE).then_some(EntityId(c.0 & !SHARED_GLYPH_BASE))
}

/// Line and arc ends meeting on one point entity, by point (2 or more).
fn shared_ends(sk: &Sketch) -> Vec<(EntityId, Vec<EntityId>)> {
    let mut by: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
    for e in sk.entities() {
        if !matches!(e.kind(), EntityKind::Line | EntityKind::Arc) || e.reference_only || noshape_sketch::text::glyph_text_id(e.id).is_some() {
            continue;
        }
        if let Some((a, b)) = sk.endpoints(e.id) {
            by.entry(a).or_default().push(e.id);
            if b != a {
                by.entry(b).or_default().push(e.id);
            }
        }
    }
    by.into_iter().filter(|(p, c)| c.len() > 1 && p.0 < SHARED_GLYPH_BASE >> 2).collect()
}

/// Deleting a shared point's Coincident glyph splits the point: every curve
/// but the first gets its own (Onshape: the ends are no longer joined).
fn split_shared(data: &mut SketchData, p: EntityId) -> bool {
    let Some((_, curves)) = shared_ends(&data.sketch).into_iter().find(|(q, _)| *q == p) else { return false };
    let mut changed = false;
    for c in curves.into_iter().skip(1) {
        changed |= data.sketch.split_point(p, c).is_ok();
    }
    changed
}

/// A used line's or arc's ends before a Trim or Split (`used_ends_before`), for [`keep_used_ends`].
fn used_ends_before(data: &SketchData, id: EntityId) -> Option<[(EntityId, DVec2); 2]> {
    data.external_refs.get(&id.0)?;
    if !matches!(data.sketch.kind(id), Some(EntityKind::Line | EntityKind::Arc)) {
        return None;
    }
    let (s, e) = data.sketch.endpoints(id)?;
    Some([(s, data.sketch.point(s)?), (e, data.sketch.point(e)?)])
}

/// Live (skasm 11-19 split / trim of a used line): an end a Trim or Split cut off a used curve still follows the edge's
/// end. Split: the far piece's end (its USE_END). Trim: a point kept there (live's `.orphan`), on the trimmed curve so it
/// stays on the edge's line. An end another used curve shares follows that one already.
fn keep_used_ends(data: &mut SketchData, id: EntityId, before: Option<[(EntityId, DVec2); 2]>, trimmed: bool) {
    let Some(ends) = before else { return };
    let Some(r) = data.external_refs.get(&id.0).cloned() else { return };
    let Some((s, e)) = data.sketch.endpoints(id) else { return };
    for (i, (p, at)) in ends.into_iter().enumerate() {
        if p == s || p == e {
            continue;
        }
        let shared = data.sketch.entities().any(|x| x.id != id && x.external && data.external_refs.contains_key(&x.id.0) && x.point_refs().contains(&p));
        if shared {
            continue;
        }
        let kept = match data.sketch.entity(p) {
            Some(_) if !trimmed => p,
            _ => {
                let Ok(q) = data.sketch.add_entity(noshape_sketch::Geometry::Point { pos: at }, false, true) else { continue };
                let _ = data.sketch.add_constraint(ConstraintKind::Coincident { a: q, b: id });
                q
            }
        };
        let _ = data.sketch.set_external(kept, true);
        data.external_refs.insert(kept.0, r.clone());
        data.use_ends.insert(kept.0, i == 1);
    }
}

/// Deleting a Use glyph unlinks the geometry: it stays as regular, free
/// sketch geometry no longer following the model (Onshape).
fn unlink_used(data: &mut SketchData, e: EntityId) -> bool {
    if data.external_refs.remove(&e.0).is_none() {
        return false;
    }
    data.use_ends.remove(&e.0);
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

/// What a dragged point dropped on its snap target becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drop {
    /// Merged with a point (or coincident with the origin).
    Merge,
    /// Coincident with a curve.
    OnCurve,
    /// The line's midpoint.
    Midpoint,
    /// An arc's end level (true) or plumb with its other end (live2 sketch-drags b1x: HORIZONTAL on release).
    Level(bool),
}

/// A direction through the tool's anchor (horizontal, vertical, square to, parallel to, a tangent line to a curve): one
/// of live's infinite line zones, below the curves.
fn is_direction_line(i: &Inference) -> bool {
    matches!(i.kind, InferenceKind::Horizontal | InferenceKind::Vertical | InferenceKind::Perpendicular | InferenceKind::Parallel | InferenceKind::Tangent)
        && !i.constraints.iter().any(|c| matches!(c, AutoConstraint::OnCurve(_)))
}

/// A horizontal / vertical alignment with a woken point (or the origin): the line through that point, a zone a
/// direction or a curve can cross (live2 combo alignCurve, perpAlign).
fn is_alignment(i: &Inference) -> bool {
    matches!(i.kind, InferenceKind::AlignHorizontal | InferenceKind::AlignVertical) && i.constraints.len() == 1
}

/// A line zone: a direction through the anchor or an alignment through a woken point.
fn is_line_zone(i: &Inference) -> bool {
    is_direction_line(i) || is_alignment(i)
}

/// A line zone's line: a point on it and its direction.
fn zone_line(i: &Inference) -> Option<(DVec2, DVec2)> {
    let r = i.reference?;
    match i.kind {
        InferenceKind::AlignHorizontal => Some((r, DVec2::X)),
        InferenceKind::AlignVertical => Some((r, DVec2::Y)),
        _ => Some((r, (i.position - r).try_normalize()?)),
    }
}

/// Snap to model geometry (from the 3D hover) supplied by the view.
#[derive(Clone, Debug, PartialEq)]
pub enum ExternalSnap {
    /// A model vertex projected onto the sketch plane.
    Vertex { position: DVec2, reference: noshape_model::reference::Reference },
    /// The centre of a circular model edge in the sketch's plane (live: a CONCENTRIC point, offered lit or not).
    Centre { position: DVec2, reference: noshape_model::reference::Reference },
    /// A point on a model edge projected onto the sketch plane, and the edge's direction there; or (`midpoint`) a
    /// straight edge's midpoint.
    Edge { position: DVec2, tangent: DVec2, reference: noshape_model::reference::Reference, midpoint: bool },
}

pub struct SketchEditor {
    pub feature: FeatureId,
    pub frame: PlaneFrame,
    /// The handle arms the scene draws (from the shown display, set when one is applied); `None`: every arm the
    /// sketch has. The overlay draws the others, so an arm the display doesn't have yet still shows.
    pub scene_arms: Option<std::collections::HashSet<EntityId>>,
    pub tool: Tool,
    clicks: Vec<Click>,
    /// End point of the previous line (chaining).
    chain_end: Option<EntityId>,
    pub cursor: Option<DVec2>,
    /// A draw tool started while the pointer rests: nothing is drawn at it until it moves (live shortcuts-sk-1118).
    pointer_rests: bool,
    pub inference: Option<Inference>,
    /// The entity the last freshly chosen snap was on: kept while it is still under the pointer (live's getInference
    /// lastInferenceTarget; not cleared when nothing is under the pointer).
    snap_target: std::cell::Cell<Option<EntityId>>,
    pub external_snap: Option<ExternalSnap>,
    /// The centres of the model's circular edges in the sketch's plane (the view fills it once a sketch is open).
    pub model_centres: std::sync::OnceLock<Vec<(DVec2, noshape_model::reference::Reference)>>,
    /// The model's straight edges in the sketch's plane: each one's reference and ends (the view fills it once a
    /// sketch is open; a woken one gives a perpendicular and its ends' alignments).
    pub model_lines: std::sync::OnceLock<Vec<(noshape_model::reference::Reference, DVec2, DVec2)>>,
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
    /// Model edges / vertices selected in the sketch (live sketch.md 8.11):
    /// they join the selection for a constraint, as linked external entities.
    pub model_selection: Vec<noshape_model::reference::Reference>,
    /// A plane pressed with no tool (the view sets it): released without a drag it joins `model_selection`, as live
    /// (a click on a plane isn't on empty space, live2 Part 2).
    pub model_press: Option<noshape_model::reference::Reference>,
    /// Hovered dimension label or constraint glyph.
    pub hover_constraint: Option<ConstraintId>,
    /// Hovered sketch pattern glyph (pattern id).
    pub hover_pattern: Option<u32>,
    /// Entity whose glyphs stay visible briefly after hover ends.
    pub glyph_entity: Option<(EntityId, std::time::Instant)>,
    /// The hovered line and the pointer's spot on the plane where its hover started: with Show constraints off its
    /// glyph row sits there, not at the line's middle, and stays put while the pointer slides along the line and after
    /// the hover ends (live2 feel-1417).
    pub glyph_row_at: Option<(EntityId, DVec2)>,
    drag: Option<DragState>,
    /// The sketch as a geometry drag found it: each step solves from it.
    drag_origin: Option<Sketch>,
    /// Curves a dragged point lay on when its drag began: its drop takes nothing on them.
    drag_held: Vec<EntityId>,
    box_end: Option<DVec2>,
    /// Ctrl held during the box selection: it removes from the selection.
    pub box_remove: bool,
    /// Point snapped to while dragging a point (constrained on release).
    drag_snap: Option<(EntityId, DVec2, Drop)>,
    /// A dragged point's model edge under the pointer (the view finds it; no sketch target in reach): on release the
    /// view links the edge and the point lands on it ([`Self::drop_onto_external`]).
    pub drag_external: Option<ExternalSnap>,
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
    /// The last committed polygon's centre and 2nd click: live shows its Ø
    /// in a grey box (a readout, not a dimension) until the next polygon.
    polygon_readout: Option<(DVec2, DVec2)>,
    pub fillet_radius: f64,
    pub offset_distance: f64,
    pub offset_flip: bool,
    /// The pending offset's default side needs a negative distance (live
    /// qa4 OFF-SIDE: the first side is away from the region the first curve
    /// bounds, so a circle offsets outward).
    pub offset_out: bool,
    /// A warning toast for the host to show (taken once shown).
    pub toast: Option<String>,
    /// Why the last constraint command added nothing (live 2270's banner: it stays until the next message, × or a
    /// constraint added; taken by the host).
    pub refusal: Option<String>,
    /// Curves of the offset being set up (preview, arrow and distance box).
    pub offset_pending: Option<Vec<EntityId>>,
    /// The pointer is on the offset's distance label (set by the host): its arrowheads aren't drawn (live offset-look-1121).
    pub offset_label_hovered: bool,
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
    /// The two clicks that placed it: a corner on a snapped point gets that point's Coincident at ✓.
    text_clicks: Vec<Click>,
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
    /// A paste follows the pointer (live feel round 2 R6): where the pointer last moved the copy to; the next click
    /// places it and leaves the tool.
    pub paste_follow: Option<DVec2>,
    pub xf_offset: DVec2,
    pub xf_angle: f64,
    pub xf_scale: f64,
    /// Where the scale factor's box stays once its drag ends (live editing-gaps4: where the pointer left it).
    xf_scale_box: Option<DVec2>,
    /// The arrow last dragged and where its distance's box stays (live sk2-1121 transform: a box after the drag).
    xf_move_box: Option<(PatternHandle, DVec2)>,
    /// An arrow's or the square's press: where, and the move then (the drag adds the pointer's travel since: live
    /// sk2-1121 transform, a press on the Y shaft dragged 10 mm moved 9.9 mm).
    xf_grab: Option<(DVec2, DVec2)>,
    /// A rotate / scale / arrow drag just ended: the view opens its value box for typing (live: its text selected).
    pub xf_released: Option<PatternHandle>,
    /// First fillet arc of the current fillet session (later fillets are equal to it).
    fillet_session: Option<EntityId>,
    /// Fillet/chamfer preview is isolated from the document until Enter.
    pending_corner: Option<(SketchData, noshape_sketch::EditResult)>,
    /// The corner point of the pending fillet (clicked on its vertex).
    pending_corner_pid: Option<EntityId>,
    /// Where the press that made the pending fillet / chamfer was: dragged from there and released, it is made
    /// (live feel/sketchtools.json "sketchFillet": the radius box opens).
    corner_pressed: Option<DVec2>,
    /// A corner whose fillet at the current radius is invalid: previewed red
    /// (live), its arrow drags the radius to one that fits.
    invalid_corner: Option<EntityId>,
    /// Slot preview (live Onshape: light blue until the next click, Enter or
    /// leaving the tool commits it): the previewed data and the new curves.
    pending_slot: Option<(SketchData, Vec<EntityId>)>,
    pub chamfer_distance: f64,
    pub slot_width: f64,
    /// End cap of the first slot of the session (later slots are equal to it).
    slot_session: Option<EntityId>,
    /// Pattern being edited (Edit pattern): replaced on commit.
    pub editing_pattern: Option<u32>,
    /// World units per pixel when the pattern tool started: its labels' offsets (live sketch-gaps3).
    pattern_label_wpp: f64,
    /// Insert image: the chosen file (name, bytes, width / height) waiting for its rectangle.
    pub pending_image: Option<(String, Vec<u8>, f64)>,
    /// Insert DXF: meters per drawing unit (`None`: the file's units, else millimeters).
    pub dxf_unit_scale: Option<f64>,
    /// Insert DXF: keep the file origin at the sketch origin (else center the drawing).
    pub dxf_use_origin: bool,
    /// Mirror line chosen in tool-first mode.
    mirror_axis: Option<EntityId>,
    /// Extend: the curve picked first, waiting for its boundary.
    pub extend_from: Option<EntityId>,
    /// Extend on a line: the end picked (the nearer one), which follows the pointer (live 11-18 sketch-tools t4-ext).
    pub extend_end: Option<EntityId>,
    /// The picked end's preview has snapped onto a boundary since the pick: the picked line then shows selected (live
    /// mc-reach-1122 R6).
    extend_crossed: bool,
    /// The draft holds the regeneration's solve of the sketch (taken on opening, or an edit came first): live edits
    /// the geometry it shows, not the stored one an upstream change has since moved.
    pub draft_solved: bool,
    /// On-the-fly dimension boxes of the geometry created last.
    pub otf: Option<OnTheFly>,
    /// The user started typing into the active on-the-fly box.
    pub otf_typing: bool,
    /// Curves the cursor passed over while drawing, and points it rested on (wake-up inferences).
    pub woken: Vec<EntityId>,
    /// The next click doesn't snap onto existing points (the agent API's `apart`: the parity translator draws an end live
    /// keeps apart from a point it lies on, only coincident with a curve).
    pub no_point_snap: bool,
    /// Lines whose midpoint a snap landed on while drawing, the newest first ([`WOKEN_MIDS`]; live align-mid-1118: it
    /// gives alignment, the click making a point there, MIDPOINT on the line).
    pub woken_mids: Vec<EntityId>,
    /// The point snapped onto at the last move (at a tool's start, the one hovered): a point wakes when the pointer
    /// arrives on it, not while it stays (live ui-parity revolve-dialog-axis, sketch-mirror-undo-midway: the corner the
    /// pointer rests on when Line starts gives no alignment).
    snapped_point: Option<EntityId>,
    /// The straight model edge the pointer is on while drawing, and since when ([`EDGE_WAKE_REST`]); the edge snapped
    /// since the last move.
    edge_acquiring: Option<(noshape_model::reference::Reference, std::time::Instant)>,
    edge_seen: Option<noshape_model::reference::Reference>,
    /// The model edge woken last and its ends in the sketch (live sketch-gaps 1c: a line from a start then offers
    /// PERPENDICULAR to it; live2 wake-parallel: its ends give horizontal / vertical alignment, never a parallel).
    pub woken_edge: Option<(noshape_model::reference::Reference, DVec2, DVec2)>,
    /// The point outside the sketch woken last: snapped onto while drawing (an external vertex snap; live 11-17
    /// sketch-snap-1117, Sketch 2's point then offers its horizontal / vertical alignment, as a point of the sketch does).
    pub woken_point: Option<(noshape_model::reference::Reference, DVec2)>,
    /// A model edge's midpoint the pointer is on while a level crosses near it: the edge, the midpoint and (once the
    /// click used the edge) its curve in the sketch.
    woken_edge_mid: Option<(noshape_model::reference::Reference, DVec2, Option<EntityId>)>,
    /// Press position of a drawing tool (press-drag-release placement).
    press: Option<DVec2>,
    /// A press on the Line chain's last point: released there it ends the chain, dragged it keeps the rubber band.
    chain_end_press: Option<DVec2>,
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
    /// The agent API's headless editor: clicks snap to points only at the same place (0.1 `world_per_px`: c2-sm-louver's
    /// 0.9 µm line), and a line infers horizontal / vertical / parallel / perpendicular, a point another's x or y, only
    /// when it practically is (the screen's 3° and 9 px moved c2-hunting-knife's 2.2° blade edge and exercise-anthony's
    /// 0.03° hexagon).
    pub exact_inference: bool,
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

/// Two curves share an end point (one offset chain).
fn curves_touch(sk: &Sketch, a: EntityId, b: EntityId) -> bool {
    let (Some((a0, a1)), Some((b0, b1))) = (sk.endpoints(a), sk.endpoints(b)) else { return false };
    let at = |p: EntityId, q: EntityId| p == q || sk.point(p).zip(sk.point(q)).is_some_and(|(x, y)| (x - y).length() < 1e-9);
    [a0, a1].iter().any(|&p| at(p, b0) || at(p, b1))
}

/// Whether a positive offset of the chain `ids` goes, at its first curve,
/// into a region that curve bounds and not out of another: live's default
/// side is away from the region.
fn offset_goes_in(sk: &Sketch, ids: &[EntityId]) -> bool {
    let Some(&first) = ids.first() else { return false };
    let Some(c) = sk.curve(first) else { return false };
    let (t0, t1) = c.domain();
    let mid = c.point((t0 + t1) * 0.5);
    let mut tmp = sk.clone();
    let eps = c.bbox().size().max_element().max(1e-9) * 1e-3;
    let Ok(r) = tmp.offset(ids, eps) else { return false };
    let Some(q) = r.created.iter().filter_map(|e| tmp.curve(*e)).map(|c| c.closest(mid).1).min_by(|x, y| (*x - mid).length().total_cmp(&(*y - mid).length())) else { return false };
    let dir = q - mid;
    let regions: Vec<_> = sk.regions().into_iter().filter(|r| r.entities().contains(&first)).collect();
    // Bounding no region: to the right of the first curve's direction (live workflows-1122 22 s16: the line drawn
    // left to right, its copy below).
    if regions.is_empty() {
        let t = c.tangent((t0 + t1) * 0.5);
        return dir.dot(DVec2::new(t.y, -t.x)) < 0.0;
    }
    let inside = |p: DVec2| regions.iter().any(|r| r.contains(p));
    inside(mid + dir) && !inside(mid - dir)
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
        // Live (1940: today's dialog template has Show constraints unticked
        // for a user who never saved it; sketch/311's ☑ was an older
        // template): Show errors alone ticked.
        Self { show_constraints: false, show_expressions: false, show_errors: true }
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

/// Saves the Sketch dialog's options (per user, every sketch opens with them).
pub fn save_sketch_options(o: SketchOptions) {
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
/// The Use tool's hovered edges: the hover's light orange (live sketch-tools-1123 use-convert (254,199,136)).
const USE_HOVER_COLOR: [u8; 4] = [0xff, 0xc6, 0x85, 255];
/// The hovered edge itself through the Use tool's band (qa SKT-USE-HOVER, live sk23 use-0-hover-edge / -vertex: across the
/// edge 1 px (254,199,136), 2-3 px (213,182,138), 1 px (254,199,136)).
const USE_HOVER_EDGE_COLOR: [u8; 4] = [0xd5, 0xb6, 0x8a, 255];
/// The dash along a rubber band whose direction is inferred (live sketch/51).
const INFERRED_DASH_COLOR: [u8; 4] = [0xe2, 0xbc, 0x2c, 255];

/// The inference sets the direction of the line being drawn (not a point to
/// snap to): shown on the rubber band as its dash (live sketch/51–53).
fn is_direction(k: InferenceKind) -> bool {
    matches!(k, InferenceKind::Horizontal | InferenceKind::Vertical | InferenceKind::Perpendicular | InferenceKind::Parallel)
}
/// Onshape's hollow arrow manipulator in a sketch (live sketch/303, offset):
/// an 18 px stem, a 31 x 23 px head; outline #989fa3 (3 px) around a white core.
const OFFSET_ARROW_STEM_PX: f64 = 18.0;
/// The offset preview's dimension (live offset-look-1121): its label in the gap from this gap on (in at ~18 px, on
/// the picked curve at ~9 px), its two filled arrowheads ~5 px.
const OFFSET_LABEL_IN_GAP_PX: f64 = 13.5;
const OFFSET_DIM_ARROW_PX: f64 = 5.0;
const MANIPULATOR_HOVER: ([u8; 4], [u8; 4]) = ([0x46, 0x74, 0xaf, 255], [0x9b, 0xb2, 0xcc, 255]);
const OFFSET_ARROW_HEAD_PX: f64 = 31.0;
const OFFSET_ARROW_HEAD_W_PX: f64 = 23.0;
const MANIPULATOR_OUTLINE: [u8; 4] = [0x98, 0x9f, 0xa3, 255];
/// The fillet radius manipulator's stem (live sketch/1934: the arrow starts
/// at the preview arc's midpoint and points at its centre).
const FILLET_ARROW_STEM_PX: f64 = 28.0;
/// One previewed pattern instance: where a seed point goes.
type PointMap = Box<dyn Fn(DVec2) -> DVec2>;
/// Linear pattern manipulators (live sketch/136): a 16 px hollow square at
/// the first instance, the count arrow's 30 px stem from its edge. Live
/// sketch-gaps3: the "3x" label's centre (+97, +99.8) px from the 2nd instance
/// (along, across the direction) and the spacing dimension 99.8 px to the side,
/// both in px at the zoom the tool started at, then fixed in the model.
const PATTERN_SQUARE_PX: f64 = 16.0;
const PATTERN_ARROW_STEM_PX: f64 = 30.0;
const PATTERN_ARROW_HANDLE_PX: f64 = PATTERN_SQUARE_PX * 0.5 + PATTERN_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX * 0.5;
const PATTERN_LABEL_ALONG_PX: f64 = 97.0;
const PATTERN_DIM_PX: f64 = 99.8;
/// Half a pattern label's box (live sketch-gaps3: ~30 px wide): where the leader meets it.
const PATTERN_LABEL_HALF_PX: f64 = 15.0;
/// A circular pattern's "3x" (live sketch-gaps3): 0.4 × the seed's radius beyond the seed, in the model.
const CIRCULAR_LABEL_BEYOND: f64 = 0.4;
/// A circular pattern's handle arc (live sketch-gaps4): 1.2 × the seed's radius, from the seed to the last instance,
/// ~4 px light blue, its open arrow ~20 px on past the end along the arc; below 360° an angle dimension (black, filled
/// arrowheads) at 1.4 × the radius, its text ~0.64 of the way along (one take: 115° into 180°).
const CIRCULAR_HANDLE_AT: f64 = 1.2;
const CIRCULAR_HANDLE_COLOR: [u8; 4] = [0x86, 0xa5, 0xcc, 0xff];
const CIRCULAR_RAY_COLOR: [u8; 4] = [0x55, 0x83, 0xb8, 0xff];
const CIRCULAR_ARROW_STEM_PX: f64 = 20.0;
const CIRCULAR_ANGLE_TEXT_AT: f64 = 0.64;
/// The sketch 2D manipulator (Transform, a paste), px from its anchor along its X / Y (live editing-gaps3, the client's
/// triad: arrows' gap 8 + shaft 60 then the head; the square 20–45; the rotation ring 108 out on X; the scale head
/// from (60, 60) on the diagonal, 30 long). Handles are screen-sized; no shafts are drawn.
const XF_ARROW_BASE_PX: f64 = 68.0;
/// The arrows' shafts start 8 px from the anchor (live editing-gaps3) and take a click 6 px either side.
const XF_ARROW_GAP_PX: f64 = 8.0;
const XF_ARROW_SHAFT_REACH_PX: f64 = 6.0;
const XF_ARROW_HEAD_PX: f64 = 28.0;
const XF_ARROW_HALF_W_PX: f64 = 11.0;
const XF_SQUARE_PX: (f64, f64) = (20.0, 45.0);
const XF_RING_AT_PX: f64 = 108.0;
const XF_RING_R_PX: f64 = 9.0;
const XF_SCALE_BASE_PX: f64 = 84.85;
const XF_SCALE_HEAD_PX: f64 = 30.0;
const XF_SCALE_HALF_W_PX: f64 = 16.0;
/// The rotate / scale value boxes' centres (75 × 26 px; live editing-gaps4, sketch x right, y up): rotate during its
/// drag off the pointer, after it off the anchor; scale off the pointer, then left where the drag ended (or off the
/// anchor).
const XF_ROTATE_BOX_FROM_POINTER: (f64, f64) = (60.5, 16.0);
const XF_ROTATE_BOX_AFTER: (f64, f64) = (162.5, 87.0);
const XF_SCALE_BOX_FROM_POINTER: (f64, f64) = (45.5, -47.0);
/// An arrow's distance box from the pointer, during its drag and where the drag leaves it (live sk2-1121
/// transform-2-after-drag: up and right of the arrow's head).
const XF_MOVE_BOX_FROM_POINTER: (f64, f64) = (72.0, -48.0);
const XF_SCALE_BOX_AFTER: (f64, f64) = (138.5, 46.0);
/// The line shader's dash-dot period (`fs_line`, style 4) in pixels.
const DASH_DOT_PERIOD_PX: f64 = 16.0;

/// The dash and dot pieces of a dash-dot polyline with the given period,
/// measured along the whole polyline (the shader's proportions: dash
/// 0–0.55, dot 0.68–0.8 of each period).
pub fn dash_dot_pieces(pts: &[DVec2], period: f64) -> Vec<Vec<DVec2>> {
    const MARKS: [(f64, f64); 2] = [(0.0, 0.55), (0.68, 0.8)];
    let mut out: Vec<Vec<DVec2>> = Vec::new();
    if period <= 0.0 || pts.len() < 2 {
        return vec![pts.to_vec()];
    }
    let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    // Every mark interval along the curve, then the polyline clipped to it.
    let mut start = 0.0;
    while start < total {
        for (a, b) in MARKS {
            let (lo, hi) = (start + a * period, (start + b * period).min(total));
            if lo >= hi {
                continue;
            }
            let mut piece = Vec::new();
            let mut d = 0.0;
            for w in pts.windows(2) {
                let len = (w[1] - w[0]).length();
                let (s0, s1) = (d, d + len);
                if s1 > lo && s0 < hi && len > 0.0 {
                    let at = |x: f64| w[0] + (w[1] - w[0]) * ((x - s0) / len);
                    if piece.is_empty() {
                        piece.push(at(lo.max(s0)));
                    }
                    piece.push(at(hi.min(s1)));
                }
                d = s1;
            }
            if piece.len() >= 2 {
                out.push(piece);
            }
        }
        start += period;
    }
    out
}

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

/// A closed outline in the manipulators' grey double line (3 px outline under a white core).
fn push_hollow(wide: &mut Vec<(Vec<DVec2>, [u8; 4], f32)>, pts: Vec<DVec2>) {
    for (color, width) in [(MANIPULATOR_OUTLINE, 3.0), ([0xff, 0xff, 0xff, 0xff], 1.2)] {
        wide.push((pts.clone(), color, width));
    }
}

/// [`push_hollow_arrow`] in other colours (a circular pattern's handle, live sketch-gaps4: light blue; a hovered
/// offset arrow, live offset-look-1121: #4674af around #9bb2cc).
fn push_hollow_arrow_in(wide: &mut Vec<(Vec<DVec2>, [u8; 4], f32)>, start: DVec2, dir: DVec2, px: f64, stem_px: f64, outline: [u8; 4], core: [u8; 4]) {
    let base = start + dir * px * stem_px;
    let apex = base + dir * px * OFFSET_ARROW_HEAD_PX;
    let half = dir.perp() * px * OFFSET_ARROW_HEAD_W_PX * 0.5;
    for (color, width) in [(outline, 3.0), (core, 1.2)] {
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
        ConstraintKind::OffsetPair { master_source, master_copy, .. } => vec![*master_source, *master_copy],
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
    // (Live's label writes radians out in full: "1.2 radian", live 11-18 server-side number formats.)
    let label_suffix = if dim == Dim::ANGLE && suffix.trim() == "rad" { " radian".to_string() } else { suffix.clone() };
    let shown_suffix = if dim == Dim::LENGTH { String::new() } else { label_suffix.clone() };
    let prec = tol.and_then(|t| t.precision);
    let fmt = |x: f64| match prec {
        Some(n) => format!("{}{shown_suffix}", fixed_number(x, dim, units, n)),
        None => {
            let t = noshape_units::format_quantity(Quantity::new(x, dim), units);
            match t.strip_suffix(suffix.as_str()) {
                Some(n) => format!("{n}{shown_suffix}"),
                None => t,
            }
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

/// A tangent arc's preview, within this many pixels of the arc (live sketch-tools-1119: a smooth arc; at a pixel
/// it showed ~5 straight pieces).
const ARC_PREVIEW_TOL_PX: f64 = 0.05;
/// [`SketchEditor::apply_dimension`]'s refusals of a value the dimension
/// can't take (a negative radius, a zero length). Live shows no message: the
/// box turns pink and ✓ greys out.
pub const DIM_NOT_POSITIVE: &str = "Value must be positive";
pub const DIM_NOT_NEGATIVE: &str = "This dimension can't be negative";
/// Live's warning when the picked items can't be dimensioned.
pub const DIMENSION_FAILED: &str = "A dimension cannot be created between these items.";

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
    // Live sketch/13, 141, 143, ui-parity sketch-dimension-*: lengths to
    // 1e-8 m ("24.49437 mm", "1.725582 in"), angles with 7 decimals
    // ("35.5376823 deg"), zeros trimmed.
    let length_decimals = (8.0 + units.length.meters().log10() + 1e-9).floor().max(0.0) as u8;
    let precision = if dim == Dim::ANGLE { units.angle_precision.max(7) } else { units.length_precision.max(length_decimals) };
    Some(noshape_units::format_quantity_with(Quantity::new(value, dim), units, precision).replace('°', " deg"))
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
        Midpoint { .. } | MidpointOf { .. } => "midpoint",
        Symmetric { .. } => "symmetric",
        Fix { .. } => "fix",
        Normal { .. } => "normal",
        Collinear { .. } => "collinear",
        Curvature { .. } => "curvature",
        Flow { .. } => "flow",
        Pierce { .. } => "pierce",
        Quadrant { .. } => "coincident",
        // Live (qa5 sketch tools): the slot's three offsets, an arch glyph.
        Offset { .. } | OffsetPair { .. } => "offset",
        _ => return None,
    })
}

/// How near (screen px) a circle's radius point must come to a tangent radius to snap to it: live snapped 1.85 px off
/// (sketch-mirror-preselected-batch) and not 3 px off (the snaps matrix's tool/circle-radius/no-midpoint).
const CIRCLE_TANGENT_PX: f64 = 2.5;

/// The sketch Mirror's prompts (live's i18n), shown as Onshape's tool prompt at the top of the graphics area.
pub const MIRROR_LINE_PROMPT: &str = "Select a mirror line.";
pub const MIRROR_ENTITIES_PROMPT: &str = "Select entities to be mirrored.";

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

/// The glyph of a constraint an inference creates.
fn auto_icon(c: &AutoConstraint) -> &'static str {
    match c {
        AutoConstraint::CoincidentPoint(_) | AutoConstraint::OnCurve(_) | AutoConstraint::QuadrantOf(..) => "coincident",
        AutoConstraint::MidpointOf(_) => "midpoint",
        AutoConstraint::HorizontalWith(_) | AutoConstraint::HorizontalLine => "horizontal",
        AutoConstraint::VerticalWith(_) | AutoConstraint::VerticalLine => "vertical",
        AutoConstraint::PerpendicularTo(_) => "perpendicular",
        AutoConstraint::ParallelTo(_) => "parallel",
        AutoConstraint::TangentTo(_) => "tangent",
        AutoConstraint::ConcentricTo(_) => "concentric",
    }
}

/// Where the first of `n` snap glyphs (20 px boxes, 2 px apart) sits from the pointer, in px right and down: a single
/// one at (20, 20); a combined snap's row 5 px left and 5 px lower (live2 sketch-snaps.json "combined": pairs at
/// +15 / +25, a single glyph at +20 / +20).
pub fn glyph_offset(n: usize) -> (f32, f32) {
    if n > 1 { (15.0, 25.0) } else { (20.0, 20.0) }
}

/// The glyphs shown by the pointer for inference `inf`: one per constraint it creates, in its order (a combined snap
/// shows both: the direction's, then the curve's), and coincident for a model edge it ends on (linked at the click);
/// an inference creating nothing shows its kind's.
pub fn inference_icons(inf: &Inference, on_model_edge: bool) -> Vec<&'static str> {
    let mut icons: Vec<&'static str> = Vec::new();
    for i in inf.constraints.iter().map(auto_icon).chain(on_model_edge.then_some("coincident")) {
        if !icons.contains(&i) {
            icons.push(i);
        }
    }
    if icons.is_empty() {
        icons.push(inference_icon(inf.kind));
    }
    icons
}

impl SketchEditor {
    /// Whether a plane's face (not only its outline) can be picked: not by the Dimension tool once an ellipse is
    /// picked (live plane-behind-1121: Front's face then lights nowhere, the Right plane's outline still does).
    pub fn takes_plane_faces(&self, data: &SketchData) -> bool {
        self.tool != Tool::Dimension || !self.picks.iter().any(|(e, _)| matches!(data.sketch.kind(*e), Some(EntityKind::Ellipse | EntityKind::EllipticalArc)))
    }

    pub fn new(feature: FeatureId, frame: PlaneFrame) -> Self {
        let opened_options = sketch_options();
        Self {
            feature,
            frame,
            pointer_rests: false,
            scene_arms: None,
            tool: Tool::Select,
            clicks: Vec::new(),
            chain_end: None,
            cursor: None,
            inference: None,
            snap_target: std::cell::Cell::new(None),
            draft_solved: false,
            external_snap: None,
            model_centres: std::sync::OnceLock::new(),
            model_lines: std::sync::OnceLock::new(),
            selection: Vec::new(),
            selected_constraints: Vec::new(),
            hover: None,
            use_preview: Vec::new(),
            hover_region: None,
            selected_regions: Vec::new(),
            model_selection: Vec::new(),
            model_press: None,
            hover_constraint: None,
            hover_pattern: None,
            glyph_entity: None,
            glyph_row_at: None,
            drag: None,
            drag_origin: None,
            drag_held: Vec::new(),
            box_end: None,
            box_remove: false,
            drag_snap: None,
            drag_external: None,
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
            polygon_readout: None,
            fillet_radius: 0.005,
            offset_distance: 0.005,
            offset_flip: false,
            offset_out: false,
            toast: None,
            refusal: None,
            offset_pending: None,
            offset_label_hovered: false,
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
            text_clicks: Vec::new(),
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
            // Live runs clockwise (sketch-gaps5): counter-clockwise is positive here.
            circular_angle: -TAU,
            pattern_center: DVec2::ZERO,
            xf_center_pinned: false,
            paste_follow: None,
            xf_offset: DVec2::ZERO,
            xf_angle: 0.0,
            xf_scale: 1.0,
            xf_scale_box: None,
            xf_move_box: None,
            xf_grab: None,
            xf_released: None,
            fillet_session: None,
            pending_corner: None,
            pending_corner_pid: None,
            corner_pressed: None,
            invalid_corner: None,
            pending_slot: None,
            chamfer_distance: 0.005,
            slot_width: 0.005,
            slot_session: None,
            mirror_axis: None,
            extend_from: None,
            extend_crossed: false,
            extend_end: None,
            pending_image: None,
            dxf_unit_scale: None,
            dxf_use_origin: false,
            editing_pattern: None,
            pattern_label_wpp: 0.0,
            otf: None,
            otf_typing: false,
            woken: Vec::new(),
            woken_mids: Vec::new(),
            snapped_point: None,
            no_point_snap: false,
            edge_acquiring: None,
            edge_seen: None,
            woken_edge: None,
            woken_point: None,
            woken_edge_mid: None,
            press: None,
            chain_end_press: None,
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
            exact_inference: false,
            screen_map: None,
            frame_bounds: None,
            message: None,
            issue_highlight: None,
            issue_markers: Vec::new(),
            ui: None,
        }
    }

    /// The tool the toolbar shows active: Tangent arc while Shift+A (or a press at the chain's end) switches a Line
    /// chain to its tangent arc (live ui-parity sketch-line-tangent-arc-switch), else the tool.
    pub fn shown_tool(&self) -> Tool {
        if self.tangent_mode && self.tool == Tool::Line { Tool::TangentArc } else { self.tool }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        // Pressing the active tool's key again toggles back to select.
        let previous = self.tool;
        self.polygon_readout = None;
        self.tool = if self.tool == tool && tool != Tool::Select { Tool::Select } else { tool };
        // Live: a polygon tool session starts at 6 sides; within it the next
        // polygon keeps the last count.
        if self.tool != previous && matches!(self.tool, Tool::Polygon | Tool::CircumPolygon) {
            self.polygon_sides = 6;
        }
        if self.tool != previous {
            self.pattern_label_wpp = self.world_per_px;
            self.pointer_rests = self.tool.draws();
        }
        // Choosing a drawing tool or Dimension clears the selection (live: L with a line selected deselects it;
        // skasm/hm-sk-1122 c5a: D too, no dimension started).
        if self.tool.draws() || self.tool == Tool::Dimension {
            self.selection.clear();
            self.selected_constraints.clear();
        }
        self.clicks.clear();
        self.chain_end = None;
        self.picks.clear();
        self.inference = None;
        self.snap_target.set(None);
        self.external_snap = None;
        self.fillet_session = None;
        self.pending_corner = None;
        self.pending_corner_pid = None;
        self.invalid_corner = None;
        self.pending_slot = None;
        self.editing_text = None;
        self.text_expression = false;
        self.xf_center_pinned = false;
        self.paste_follow = None;
        self.pending_text = None;
        self.text_clicks.clear();
        self.slot_session = None;
        self.offset_pending = None;
        self.offset_label_hovered = false;
        self.offset_repeat = false;
        self.mirror_axis = None;
        // Extend's pending pick goes with the tool (live: Esc, selection 0).
        self.extend_end = None;
        if self.extend_from.take().is_some() {
            self.selection.clear();
        }
        self.otf = None;
        self.otf_typing = false;
        self.woken.clear();
        self.woken_mids.clear();
        self.snapped_point = self.hover;
        self.edge_acquiring = None;
        self.woken_edge = None;
        self.woken_point = None;
        self.woken_edge_mid = None;
        self.press = None;
        self.chain_end_press = None;
        self.tangent_mode = false;
        // Live shows no hint for a sketch tool (behaviours "qa (2026-10-28,
        // P3) sketch tool hints: none in live"; SKETCH-HINT-OVERLAP: ours drew
        // over the measure readout); Mirror's prompt is live's i18n text.
        self.message = match self.tool {
            Tool::Mirror => Some(MIRROR_LINE_PROMPT.into()),
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
                    self.start_offset(&data.sketch, ids);
                    self.selection = self.offset_pending.clone().unwrap_or_default();
                }
                EditOutcome::redraw()
            }
            // Live (sketch-mirror recording, sketch-mirror-preselected-batch): the one line in the preselection is the
            // mirror line, the rest is mirrored at once and the tool asks for more. Two lines or more: no mirror line,
            // the selection stays and the tool asks for one (live2 feel/mirror-preselected).
            Tool::Mirror if let [axis] = self.selection.iter().copied().filter(|e| data.sketch.kind(*e) == Some(EntityKind::Line)).collect::<Vec<_>>()[..] => {
                self.mirror_axis = Some(axis);
                self.message = Some(MIRROR_ENTITIES_PROMPT.into());
                let rest: Vec<EntityId> = std::mem::take(&mut self.selection).into_iter().filter(|e| *e != axis).collect();
                if rest.is_empty() {
                    // (A lone line: it stays selected while the tool asks, ui-parity sketch-mirror-tool.)
                    self.selection = vec![axis];
                    return EditOutcome::redraw();
                }
                self.mirror_entities(data, &rest)
            }
            _ => EditOutcome::none(),
        }
    }

    /// Cancel the in-progress operation (Esc). Returns true if something was cancelled.
    pub fn cancel(&mut self) -> bool {
        let tool = self.tool;
        let out = self.cancel_step();
        // Live 1853 addendum: Escape out of a tool ends construction mode.
        if tool != Tool::Select && self.tool == Tool::Select {
            self.construction = false;
        }
        out
    }

    fn cancel_step(&mut self) -> bool {
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
            self.selection.clear();
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
            self.xf_scale_box = None;
            self.xf_move_box = None;
            self.set_tool(Tool::Select);
            return true;
        }
        // Live (ui-parity sketch-chamfer-tool): the corner a fillet / chamfer previews doesn't stay selected.
        if matches!(self.tool, Tool::Fillet | Tool::Chamfer) && self.pending_corner.take().is_some() {
            self.selection.clear();
            self.set_tool(Tool::Select);
            return true;
        }
        if !self.clicks.is_empty() || !self.picks.is_empty() || self.chain_end.is_some() || self.mirror_axis.is_some() || self.tangent_mode {
            // Live 2270: a constraint tool's picks (Midpoint started on two points) go with it.
            if matches!(self.tool, Tool::Constraint(_)) {
                let picks: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
                self.selection.retain(|e| !picks.contains(e));
            }
            // Live (sketch-mirror-escape-after-line): the mirror line doesn't stay selected, so Mirror again asks for one.
            if let Some(axis) = self.mirror_axis.filter(|_| self.tool == Tool::Mirror) {
                self.selection.retain(|e| *e != axis);
            }
            self.set_tool(Tool::Select);
            return true;
        }
        // Live2 (feel/mirror-preselected): Escape out of Mirror still asking for its line clears the selection.
        if self.tool == Tool::Mirror {
            self.selection.clear();
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

    /// The existing point a Line's first click snapped to: the first segment
    /// infers tangent / perpendicular / parallel from the curves there (live,
    /// behaviours "qa (2026-10-28, P2) SKETCH-START-INFERENCE"; it had no
    /// anchor point until a segment existed).
    fn line_start_point(&self) -> Option<EntityId> {
        if self.tool != Tool::Line {
            return None;
        }
        let inf = self.clicks.first()?.inference.as_ref().filter(|i| i.kind == InferenceKind::Endpoint)?;
        match inf.constraints.first() {
            Some(AutoConstraint::CoincidentPoint(p)) => Some(*p),
            _ => None,
        }
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
            anchor_point: self.chain_end.or_else(|| self.line_start_point()),
            snap_radius: self.world_per_px * if self.exact_inference { 0.1 } else { INFERENCE_PX },
            angle_tolerance: if self.exact_inference { 1e-9 } else { 3f64.to_radians() },
            align_radius: self.exact_inference.then_some(self.world_per_px * 1e-3),
            exclude: Vec::new(),
            origin: true,
            woken: self.woken.clone(),
            woken_at: self.woken_edge.as_ref().map_or(Vec::new(), |(_, a, z)| vec![(WOKEN_EDGE_ENDS[0], *a), (WOKEN_EDGE_ENDS[1], *z)]).into_iter().chain(self.woken_point.as_ref().map(|(_, q)| (WOKEN_POINT, *q))).chain(self.woken_mid_points(sk)).collect(),
            screen: self.screen_metric(),
        };
        let mut all = noshape_sketch::infer(sk, p, &ctx);
        // A sketch point on the Origin is taken before the Origin (live sk2-1121 3pt-circle-on-points: the circle
        // ends on the line's start there).
        let on_origin = |i: &Inference| i.entity.is_some_and(|e| e == noshape_sketch::ORIGIN || is_origin(sk, e));
        all.sort_by(|x, y| x.priority.cmp(&y.priority).then(x.distance.total_cmp(&y.distance)).then(on_origin(x).cmp(&on_origin(y))));
        // (Live walk3-1121: a line chain, tangent arc or slot from a line's end on the Origin goes on from that end.)
        if all.first().is_some_and(on_origin)
            && let Some(k) = all.iter().position(|i| i.priority == all[0].priority && !on_origin(i) && matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Center) && i.position.distance(all[0].position) <= self.world_per_px * 1e-3)
        {
            let own = all.remove(k);
            all.insert(0, own);
        }
        // A centre point arc's end takes no point or curve, only alignments: it sets the sweep alone (live
        // cparc-end-1121: a point or a line's end, on its circle or off, gives VERTICAL to the centre, no COINCIDENT,
        // the radius kept).
        if self.tool == Tool::CenterArc && self.clicks.len() == 2 {
            all.retain(|i| !matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Midpoint | InferenceKind::Center | InferenceKind::Quadrant | InferenceKind::OnCurve));
        }
        if self.no_point_snap {
            all.retain(|i| i.kind != InferenceKind::Endpoint);
        }
        if let Some(i) = self.woken_edge_perpendicular(p, anchor) {
            all.push(i);
            all.sort_by(|x, y| x.priority.cmp(&y.priority).then(x.distance.total_cmp(&y.distance)));
        }
        // A circle's radius point takes coincident points only (live/feel/sketch-snaps.json "toolNotes": coincident
        // points and tangent points; no midpoints, no curves, no horizontal / vertical).
        if self.radius_point() {
            all.retain(|i| matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Center));
            all.extend(self.circle_tangents(sk, p));
            all.sort_by(|x, y| x.priority.cmp(&y.priority).then(x.distance.total_cmp(&y.distance)));
        }
        let mut best = all.first().cloned();
        // Two zones crossing near the pointer: the end where they cross, both constraints (live2 combo: a direction
        // or an alignment and a curve; a direction and an alignment; two directions through the anchor meet only
        // there, two alignments are AlignBoth, a direction and another line's extension give nothing); the crossing
        // nearest the pointer of all (live2 combo "three"). Without one, a curve beats a direction line (live/feel/
        // sketch-snaps.json "priority": curves over H / V, alignment, parallel, perpendicular and tangent lines).
        // (A tangent line and its own circle cross at the tangent point.)
        let point_like = |i: &Inference| matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Midpoint | InferenceKind::Center | InferenceKind::Quadrant);
        if best.as_ref().is_some_and(|b| !point_like(b)) {
            let zones: Vec<&Inference> = all.iter().filter(|i| is_line_zone(i)).collect();
            // (The crossings `infer` makes itself: H / V through the anchor on a curve or with an alignment, two
            // alignments.)
            let mut crossings: Vec<Inference> = all.iter().filter(|i| i.constraints.len() >= 2 && !point_like(i)).cloned().collect();
            for l in &zones {
                for on in all.iter().filter(|i| i.kind == InferenceKind::OnCurve && i.constraints.len() == 1 && i.entity.is_some_and(|e| Some(e) != l.entity || l.kind == InferenceKind::Tangent)) {
                    if let Some(c) = on.entity.and_then(|e| sk.curve(e))
                        && let Some(mut both) = self.direction_onto_curve(l, &c, on.position)
                    {
                        both.constraints.extend(on.constraints.iter().copied());
                        crossings.push(both);
                    }
                }
                if !is_direction_line(l) {
                    continue;
                }
                for m in zones.iter().filter(|m| is_alignment(m)) {
                    if let Some((q, u)) = zone_line(m)
                        && let Some(mut both) = self.direction_onto(l, q, u)
                    {
                        // (live2 perpAlign: the alignment's constraint first.)
                        both.constraints = m.constraints.iter().chain(&l.constraints).copied().collect();
                        crossings.push(both);
                    }
                }
            }
            if let Some(x) = crossings.into_iter().min_by(|a, b| a.distance.total_cmp(&b.distance)) {
                best = Some(x);
            } else if let Some(b) = &best
                && is_direction_line(b)
                && let Some(on) = all.iter().find(|i| i.kind == InferenceKind::OnCurve && i.entity.is_some_and(|e| Some(e) != b.entity || b.kind == InferenceKind::Tangent))
            {
                best = Some(on.clone());
            }
        }
        // Live's getInference: the target snapped to last stays while anything under the pointer is on it (WT@9146346
        // lastInferenceTarget), among snaps of its rank: a point or a line's end coming into reach still takes over from
        // the line (live 11-18 sticky-snap, real pointer).
        let held_target = self.snap_target.get().filter(|_| !self.exact_inference);
        if let Some(t) = held_target
            && let Some(b) = best.as_ref().filter(|b| !b.targets().any(|x| x == t))
            && let Some(on) = all.iter().find(|i| i.priority <= b.priority && i.targets().any(|x| x == t))
        {
            best = Some(on.clone());
        }
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
        // The snap lit last holds out to its let-go reach unless a stronger one lights.
        if !self.exact_inference
            && let Some(prev) = &self.inference
            && best.as_ref().is_none_or(|b| b.priority > prev.priority)
            && let Some(kept) = self.held_snap(sk, p, &ctx, prev)
        {
            best = Some(kept);
        }
        if let Some(b) = &best
            && held_target.is_none_or(|t| !b.targets().any(|x| x == t))
            && let Some(t) = b.targets().next()
        {
            self.snap_target.set(Some(t));
        }
        best
    }

    /// `prev` again at `p` if the pointer is still within its let-go reach ([`HOLD_PX`]): the same target, kind and
    /// constraints, found with the wider reach.
    fn held_snap(&self, sk: &Sketch, p: DVec2, ctx: &InferenceContext, prev: &Inference) -> Option<Inference> {
        let own_band = matches!(prev.constraints.as_slice(), [AutoConstraint::HorizontalLine | AutoConstraint::VerticalLine]);
        let wide = InferenceContext { snap_radius: self.world_per_px * if own_band { BAND_HOLD_PX } else { HOLD_PX }, ..ctx.clone() };
        let metric = self.screen_metric();
        let seen = |v: DVec2| metric.as_ref().map_or(v.length(), |m| m.len(v));
        // (A point's square: its larger screen coordinate.)
        let square = |v: DVec2| metric.as_ref().map_or(v.x.abs().max(v.y.abs()), |m| m.box_len(v));
        let reach = |v: DVec2| square(v) <= self.world_per_px * HOLD_PX && seen(v) <= self.world_per_px * HOLD_DIAGONAL_PX;
        noshape_sketch::infer(sk, p, &wide).into_iter().find(|i| {
            let point = matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Midpoint | InferenceKind::Center | InferenceKind::Quadrant);
            i.kind == prev.kind && i.entity == prev.entity && i.constraints == prev.constraints && (!point || reach(i.position - p))
        })
    }

    /// A circle's radius point tangent to a sketch line (its infinite line) or a circle or arc: the radius it would
    /// take, where the pointer is within [`CIRCLE_TANGENT_PX`] of it on screen (live sketch-mirror-preselected-batch:
    /// a circle at (20, 20) through (23, 20) tangent to the line from (5, 0) to (15, 10), 1.85 px off at 0.275 mm a
    /// px; the catalogue's "toolNotes": a radius point takes coincident points and tangent points).
    fn circle_tangents(&self, sk: &Sketch, p: DVec2) -> Vec<Inference> {
        let Some(c) = self.clicks.first().map(|k| k.pos) else { return Vec::new() };
        let Some(u) = (p - c).try_normalize() else { return Vec::new() };
        let seen = |v: DVec2| self.screen_metric().map_or(v.length(), |m| m.len(v));
        let mut out = Vec::new();
        for e in sk.entities().filter(|e| !e.external || e.kind() == EntityKind::Line) {
            let radii: Vec<f64> = match sk.curve(e.id) {
                Some(Curve2::Line { a, b }) => (b - a).try_normalize().map(|d| (c - a).perp_dot(d).abs()).into_iter().collect(),
                Some(Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. }) => {
                    let d = (center - c).length();
                    [(d - radius).abs(), d + radius].into_iter().filter(|_| d > 1e-12).collect()
                }
                _ => Vec::new(),
            };
            for t in radii.into_iter().filter(|t| *t > 1e-12) {
                let q = c + u * t;
                let off = seen(q - p);
                if off <= self.world_per_px * CIRCLE_TANGENT_PX {
                    out.push(Inference { kind: InferenceKind::Tangent, position: q, entity: Some(e.id), reference: Some(c), constraints: vec![AutoConstraint::TangentTo(e.id)], distance: off, priority: 1 });
                }
            }
        }
        out
    }

    /// The pointer places a circle's radius point (its centre clicked).
    fn radius_point(&self) -> bool {
        self.tool == Tool::Circle && self.clicks.len() == 1
    }

    /// A drawing tool has placed its first click (or chains on): live recolours nothing under the pointer then, the
    /// snap shows by its square and glyph.
    pub fn mid_draw(&self) -> bool {
        self.tool.draws() && (!self.clicks.is_empty() || self.chain_end.is_some())
    }

    /// The woken lines' midpoints, by their stand-in ids.
    fn woken_mid_points<'a>(&'a self, sk: &'a Sketch) -> impl Iterator<Item = (EntityId, DVec2)> + 'a {
        self.woken_mids.iter().zip(WOKEN_MIDS).filter_map(|(l, k)| match sk.curve(*l)? {
            Curve2::Line { a, b } => Some((k, (a + b) * 0.5)),
            _ => None,
        })
    }

    /// A click aligned with a woken midpoint: a point there, MIDPOINT on its line, the alignment to that point (live
    /// align-mid-1118: `<new line>.endSnap0`, MIDPOINT, HORIZONTAL from the new end to it; it follows the line).
    fn link_woken_mids(&mut self, sk: &mut Sketch, inference: Option<Inference>) -> Option<Inference> {
        let mut inf = inference?;
        let mut mids: Vec<(EntityId, DVec2)> = self.woken_mid_points(sk).collect();
        // (A model edge's midpoint: its used curve's, live lenbox-1122 E.)
        mids.extend(self.woken_edge_mid.as_ref().filter(|w| w.2.is_some()).map(|w| (WOKEN_EDGE_MID, w.1)));
        for (k, at) in mids {
            if !inf.constraints.iter().any(|c| matches!(c, AutoConstraint::HorizontalWith(p) | AutoConstraint::VerticalWith(p) if *p == k)) {
                continue;
            }
            let line = match k {
                WOKEN_EDGE_MID => self.woken_edge_mid.as_ref().and_then(|w| w.2),
                _ => WOKEN_MIDS.iter().position(|x| *x == k).and_then(|i| self.woken_mids.get(i).copied()),
            };
            let Some(line) = line else { continue };
            let q = sk.add_point(at);
            let _ = sk.add_constraint(ConstraintKind::Midpoint { point: q, curve: line });
            for c in &mut inf.constraints {
                *c = retarget_auto(*c, k, q);
            }
        }
        Some(inf)
    }

    /// `e` wakes its inferences (the most recent first, ten kept).
    fn wake(&mut self, e: EntityId) {
        self.woken.retain(|x| *x != e);
        self.woken.insert(0, e);
        self.woken.truncate(10);
    }

    /// A press's motion `d` (in the sketch) is still a click: less than [`DRAG_START_PX`] on screen.
    fn below_drag_start(&self, d: DVec2) -> bool {
        let seen = self.screen_metric().map_or(d.length(), |m| m.len(d));
        seen < self.world_per_px * DRAG_START_PX
    }

    /// The glyphs by the pointer for the snap now ([`inference_icons`]); for a dragged point, its drop's (live: the
    /// coincident glyph under the pointer during the drag, "feel (sketch)" (3)).
    pub fn snap_icons(&self) -> Vec<&'static str> {
        if self.dragged_point().is_some() {
            let drop = self.drag_snap.map(|(_, _, d)| d).or(self.drag_external.as_ref().map(|s| if matches!(s, ExternalSnap::Edge { midpoint: true, .. }) { Drop::Midpoint } else { Drop::OnCurve }));
            return match drop {
                Some(Drop::Merge | Drop::OnCurve) => vec!["coincident"],
                Some(Drop::Midpoint) => vec!["midpoint"],
                Some(Drop::Level(true)) => vec!["horizontal"],
                Some(Drop::Level(false)) => vec!["vertical"],
                None => Vec::new(),
            };
        }
        let Some(inf) = &self.inference else { return Vec::new() };
        let mut icons = inference_icons(inf, matches!(self.external_snap, Some(ExternalSnap::Edge { midpoint: false, .. })));
        // A model point snapped to that lies on the sketch's horizontal or vertical axis shows that alignment too
        // (live qa walk 1 w4-vertex: × and —; qa FEEL-LINE-SNAP-MODEL-LOOK). The click links the point; nothing more.
        if let Some(ExternalSnap::Vertex { position, .. } | ExternalSnap::Centre { position, .. } | ExternalSnap::Edge { position, midpoint: true, .. }) = &self.external_snap {
            for (on_axis, icon) in [(position.y.abs() < 1e-9, "horizontal"), (position.x.abs() < 1e-9, "vertical")] {
                if on_axis && !icons.contains(&icon) {
                    icons.push(icon);
                }
            }
        }
        icons
    }

    /// Geometry is being dragged (past the press): live hides the glyphs and the regions' fill until the release.
    pub fn geometry_dragging(&self) -> bool {
        matches!(self.drag, Some(DragState::Geometry { .. }))
    }

    /// The point being dragged, if a point drag is in progress.
    pub fn dragged_point(&self) -> Option<EntityId> {
        match self.drag {
            Some(DragState::Geometry { handle: DragHandle::Point(id), .. }) => Some(id),
            _ => None,
        }
    }

    /// A model edge `snap` for the dragged point, when no sketch target is in reach and the edge is within a line's
    /// reach on screen (live2 "feel (sketch)" (2): a dragged end lands on a line from 7 px); `None` clears it.
    pub fn set_drag_external(&mut self, snap: Option<ExternalSnap>) {
        let reach = |q: DVec2| self.cursor.is_some_and(|c| self.screen_metric().map_or((q - c).length(), |m| m.len(q - c)) <= self.world_per_px * INFERENCE_PX);
        self.drag_external = snap.filter(|s| self.drag_snap.is_none() && !self.shift_held && matches!(s, ExternalSnap::Edge { position, .. } if reach(*position)));
    }

    /// The release of a point dragged onto a model edge: `id` the edge's linked (used) curve; the point lands on it,
    /// coincident, or at its midpoint (live2: MIDPOINT on release).
    pub fn drop_onto_external(&mut self, id: EntityId) {
        if let Some(ExternalSnap::Edge { position, midpoint, .. }) = self.drag_external.take()
            && self.drag_snap.is_none()
        {
            self.drag_snap = Some((id, position, if midpoint { Drop::Midpoint } else { Drop::OnCurve }));
        }
    }

    /// The snap again once the GPU hover the last move asked for arrives (a model edge or vertex it lit snaps too).
    pub fn reinfer(&mut self, sk: &Sketch) {
        let Some(p) = self.cursor.filter(|_| self.tool.draws()) else { return };
        self.external_snap = None;
        self.inference = self.infer(sk, p, self.shift_held);
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
        if let Some(at) = self.paste_follow.filter(|_| self.tool == Tool::Transform) {
            return self.move_paste(data, p - at, p);
        }
        // Polygon side count from the pointer's distance to the centre (live,
        // behaviours "skasm (2026-10-27) sketch polygon side count"): one
        // side more per 10 px beyond the 2nd click, one less per 10 px in.
        if matches!(self.tool, Tool::Polygon | Tool::CircumPolygon) && self.clicks.len() == 2 && self.polygon_typed.is_empty() {
            let (ctr, rim) = (self.clicks[0].pos, self.clicks[1].pos);
            let delta = ((p - ctr).length() - (rim - ctr).length()) / (self.world_per_px * 10.0);
            let n = (self.polygon_base_sides as f64 + delta.floor()).clamp(3.0, 50.0) as usize;
            self.polygon_sides = n;
        }
        if self.tool == Tool::CenterArc && self.clicks.len() == 2 {
            let c = self.clicks[0].pos;
            let prev = self.cursor.unwrap_or(self.clicks[1].pos);
            let delta = ((p - c).to_angle() - (prev - c).to_angle() + PI).rem_euclid(TAU) - PI;
            self.center_arc_sweep += delta;
        } else {
            self.center_arc_sweep = 0.0;
        }
        self.pointer_rests &= self.cursor == Some(p);
        self.cursor = Some(p);
        if self.tool == Tool::Extend && self.extend_end.is_some() {
            self.extend_crossed |= self.extend_preview(&data.sketch, p).is_some_and(|(_, on)| on.is_some());
        }
        self.shift_held = shift;
        self.alt_held = alt;
        self.external_snap = None;
        if self.tool.draws() {
            // Curves the cursor passes over wake up their inferences (live "skb (10-25) sketch inference"): what lies
            // within 9 px of it (the GPU hover arrives a move late), else the hover. A point wakes when a snap lands
            // on it (below).
            let now = std::time::Instant::now();
            // A model edge wakes once rested on (live sketch-gaps 1c), the pointer on it until this move.
            let seen = self.edge_seen.take();
            match self.edge_acquiring.take() {
                Some((r, since)) if seen.as_ref() == Some(&r) => {
                    if now.duration_since(since) >= EDGE_WAKE_REST
                        && let Some(Some((a, z))) = self.model_lines.get().map(|l| l.iter().find(|l| l.0 == r).map(|l| (l.1, l.2)))
                    {
                        self.woken_edge = Some((r.clone(), a, z));
                    }
                    self.edge_acquiring = Some((r, since));
                }
                _ => {}
            }
            // (The hover only while it is still within reach: one left from an earlier place isn't rested on here,
            // sketch-trim-drag's ends aligned with the previous line's end clicked before the tool was restarted.)
            let reach = self.world_per_px * 9.0;
            let metric = self.screen_metric();
            let near = |e: EntityId| data.sketch.point(e).is_none_or(|q| metric.as_ref().map_or((q - p).length(), |m| m.len(q - p)) <= reach);
            if let Some(h) = data.sketch.pick_on_screen(p, reach, metric.as_ref()).or(self.hover.filter(|h| near(*h))).filter(|h| !is_point(&data.sketch, *h)) {
                self.wake(h);
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
            // Live (feel retaken 11-18): a point wakes for alignment as soon as a snap lands on it (its
            // wakeupInferenceSource, no rest time: a 10 ms rest or a slow pass wakes it) and stays woken for the line.
            let snapped = self.inference.as_ref().filter(|i| matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Center)).and_then(|i| i.entity).filter(|e| is_point(&data.sketch, *e));
            if let Some(e) = snapped.filter(|e| self.snapped_point != Some(*e)) {
                self.wake(e);
            }
            self.snapped_point = snapped;
            // A line's midpoint snapped onto wakes too (live align-mid-1118: MIDPOINT, then HORIZONTAL with it later).
            if let Some(l) = self.inference.as_ref().filter(|i| i.kind == InferenceKind::Midpoint).and_then(|i| i.entity).filter(|l| data.sketch.kind(*l) == Some(EntityKind::Line)) {
                self.woken_mids.retain(|x| *x != l);
                self.woken_mids.insert(0, l);
                self.woken_mids.truncate(WOKEN_MIDS.len());
            }
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
        } else if let Some(DragState::Geometry { handle: DragHandle::Point(id), .. }) = self.drag {
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
    /// Live (skasm 10-26): the Circumscribed polygon's second click is a
    /// vertex (the construction circle through the vertices); the Inscribed
    /// polygon's is an edge midpoint (the circle tangent to the edges).
    fn polygon_on_vertex(&self) -> bool {
        self.tool == Tool::CircumPolygon
    }

    pub fn polygon_sides_pending(&self) -> bool {
        matches!(self.tool, Tool::Polygon | Tool::CircumPolygon) && self.clicks.len() == 2
    }

    /// The side-count spinner "◀ 6 ▶": 24 px outside the middle of an edge,
    /// laid along that edge's outward normal (◀ inward, ▶ outward; measured:
    /// the ◀ / ▶ axis is perpendicular to the edge); (centre, outward
    /// direction, sides). Inscribed: the clicked edge (live sketch/91 and
    /// skasm 10-26); circumscribed: the edge after the clicked vertex.
    pub fn polygon_side_control(&self) -> Option<(DVec2, DVec2, usize)> {
        if !self.polygon_sides_pending() {
            return None;
        }
        let (c, rim) = (self.clicks[0].pos, self.clicks[1].pos);
        let n = self.polygon_sides.clamp(3, 50) as f64;
        let r = (rim - c).length();
        let start = (rim - c).to_angle();
        let mid = if self.polygon_on_vertex() { (c + DVec2::from_angle(start) * r + c + DVec2::from_angle(start + TAU / n) * r) * 0.5 } else { rim };
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

    /// The Mirror tool's line while it waits selected for the entities to mirror (drawn in its own look, not as a
    /// selection or hover).
    pub fn shown_mirror_line(&self) -> Option<EntityId> {
        self.mirror_axis.filter(|a| self.tool == Tool::Mirror && self.selection.contains(a))
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

    /// Apply a snap to model geometry when it beats the sketch inference. A point of the sketch (an end, a midpoint, a
    /// centre) beats it; a direction inference (horizontal, vertical, perpendicular) is kept where its line meets the
    /// edge near the pointer, both constraints given; otherwise the edge under the pointer wins (the user's P0 11-13:
    /// a perpendicular to the line's start segment placed the end short of the lit model edge; live2 "feel (sketch)":
    /// horizontal and on a line both, the end on the line).
    pub fn apply_external_snap(&mut self, snap: ExternalSnap) {
        if !self.tool.draws() || (self.radius_point() && matches!(snap, ExternalSnap::Edge { .. })) {
            return;
        }
        if let ExternalSnap::Edge { reference, .. } = &snap {
            self.edge_seen = Some(reference.clone());
            if self.edge_acquiring.as_ref().is_none_or(|(r, _)| r != reference) {
                self.edge_acquiring = Some((reference.clone(), std::time::Instant::now()));
            }
        }
        if let ExternalSnap::Vertex { reference, position } = &snap {
            self.woken_point = Some((reference.clone(), *position));
        }
        let strong = self.inference.as_ref().is_some_and(|i| matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Midpoint | InferenceKind::Center));
        if strong {
            return;
        }
        let distance = |q: DVec2| self.cursor.map(|c| (c - q).length()).unwrap_or(0.0);
        // A level near a model edge's midpoint: the end where it crosses the midpoint's vertical (or horizontal), aligned
        // with the midpoint, not on the edge (live lenbox-1122 E: HORIZONTAL + VERTICAL to the midpoint, no diamond).
        if let ExternalSnap::Edge { position: mid, midpoint: true, reference, .. } = &snap
            && let Some(combined) = self.level_onto_midpoint(*mid)
        {
            self.woken_edge_mid = Some((reference.clone(), *mid, None));
            self.inference = Some(combined);
            self.external_snap = None;
            return;
        }
        // (A parallel doesn't meet a model edge: live2 combo parEdge, Lb woken, the end only on the edge.) The crossing
        // beats the edge's midpoint too: on the edge then, not at its middle (live workflow 05 step 11: horizontal and
        // coincident over the left edge's midpoint, the segment exactly 20).
        if let ExternalSnap::Edge { position, tangent, midpoint, reference } = &snap
            && let Some(combined) = self.inference.as_ref().filter(|i| i.kind != InferenceKind::Parallel).and_then(|i| self.direction_onto(i, *position, *tangent))
        {
            let snap = if *midpoint { ExternalSnap::Edge { position: combined.position, tangent: *tangent, reference: reference.clone(), midpoint: false } } else { snap };
            self.inference = Some(combined);
            self.external_snap = Some(snap);
            return;
        }
        let (position, kind) = match &snap {
            ExternalSnap::Vertex { position, .. } => (*position, InferenceKind::Endpoint),
            ExternalSnap::Centre { position, .. } => (*position, InferenceKind::Center),
            ExternalSnap::Edge { position, midpoint: true, .. } => (*position, InferenceKind::Midpoint),
            ExternalSnap::Edge { position, .. } => (*position, InferenceKind::OnCurve),
        };
        self.inference = Some(Inference { kind, position, entity: None, reference: None, constraints: Vec::new(), distance: distance(position), priority: 0 });
        self.external_snap = Some(snap);
    }

    /// The level inference (a horizontal or vertical line from the anchor) carried to the vertical (horizontal) through
    /// `mid`, aligned with it, where that is within reach of the pointer.
    fn level_onto_midpoint(&self, mid: DVec2) -> Option<Inference> {
        let i = self.inference.as_ref().filter(|i| i.constraints.iter().any(|c| matches!(c, AutoConstraint::HorizontalLine | AutoConstraint::VerticalLine)))?;
        let (a, dir) = zone_line(i)?;
        let horizontal = dir.y.abs() < 1e-9;
        if !horizontal && dir.x.abs() >= 1e-9 {
            return None;
        }
        let x = if horizontal { DVec2::new(mid.x, a.y) } else { DVec2::new(a.x, mid.y) };
        let mut combined = self.crossing_in_reach(i, x)?;
        combined.constraints.push(if horizontal { AutoConstraint::VerticalWith(WOKEN_EDGE_MID) } else { AutoConstraint::HorizontalWith(WOKEN_EDGE_MID) });
        Some(combined)
    }

    /// A direction inference `i` (from the tool's anchor) carried onto the line through `on` along `tangent`: where
    /// they meet, if that is within reach of the pointer; its constraints kept (the on-curve one is added by whoever
    /// links the curve).
    fn direction_onto(&self, i: &Inference, on: DVec2, tangent: DVec2) -> Option<Inference> {
        let (a, dir) = zone_line(i)?;
        let den = dir.perp_dot(tangent);
        if den.abs() < 1e-9 {
            return None;
        }
        self.crossing_in_reach(i, a + dir * ((on - a).perp_dot(tangent) / den))
    }

    /// [`Self::direction_onto`] for a curve `c` (the pointer near `on` on it): where the direction's line crosses it
    /// exactly (a line, a circle or arc: the crossing nearest the pointer; a tangent line touches at its point),
    /// along the curve's tangent there for other curves.
    fn direction_onto_curve(&self, i: &Inference, c: &Curve2, on: DVec2) -> Option<Inference> {
        let (a, dir) = zone_line(i)?;
        match *c {
            Curve2::Line { a: la, b: lb } => self.direction_onto(i, la, lb - la),
            Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. } => {
                // |a + t dir - centre| = radius; a touching line's (near-zero) discriminant is its tangent point.
                let w = a - center;
                let (b, k) = (dir.dot(w), w.length_squared() - radius * radius);
                let disc = b * b - k;
                if disc < -(radius * radius) * 1e-3 {
                    return None;
                }
                let r = disc.max(0.0).sqrt();
                let p = self.cursor?;
                let x = [-b - r, -b + r].map(|t| a + dir * t).into_iter().min_by(|x, y| (*x - p).length().total_cmp(&(*y - p).length()))?;
                // A line touching the circle: its point, a 15 px square (the matrix's line-end/circle/tangent: 6 px
                // holds, at 10 only on the circle); the zones overlap along it.
                if disc <= (radius * radius) * 1e-3 {
                    return self.point_in_reach(i, x);
                }
                self.crossing_in_reach(i, x)
            }
            _ => self.direction_onto(i, on, c.tangent(c.closest(on).0)),
        }
    }

    /// `i` moved to the crossing `x`: the pointer is in both zones (each one's snap offered), so live takes the
    /// crossing (live2 combo: along a line or an edge it holds while the pointer stays in the direction's band, along
    /// the direction while it stays in the line's 15 px band: 9 px out along a square at 0.79 of the line's normal
    /// holds, 12 doesn't; a 15 px square round the crossing held too little). (Capped at three snap reaches away.)
    fn crossing_in_reach(&self, i: &Inference, x: DVec2) -> Option<Inference> {
        if !is_line_zone(i) {
            return None;
        }
        let c = self.cursor?;
        let seen = self.screen_metric().map_or((x - c).length(), |m| m.len(x - c));
        (seen <= self.world_per_px * INFERENCE_PX * 3.0).then(|| Inference { position: x, distance: seen, ..i.clone() })
    }

    /// `i` moved to a touching point `x` within a point's reach of the pointer (a 15 px square).
    fn point_in_reach(&self, i: &Inference, x: DVec2) -> Option<Inference> {
        if !is_line_zone(i) {
            return None;
        }
        let c = self.cursor?;
        let within = match self.screen_metric() {
            Some(m) => m.box_len(x - c) < self.world_per_px * INFERENCE_PX,
            None => (x - c).length() <= self.world_per_px * INFERENCE_PX * noshape_sketch::inference::POINT_REACH,
        };
        let seen = self.screen_metric().map_or((x - c).length(), |m| m.len(x - c));
        within.then(|| Inference { position: x, distance: seen, ..i.clone() })
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
            // On the Origin, another sketch's point or a model vertex: a point of its own, coincident (live; 11-18
            // use-glyph).
            && !sk.entity(*p).is_some_and(|e| e.reference_only)
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

    /// Constrain the next click to the centre of an external circle (a circular model edge used): CONCENTRIC.
    pub fn link_centre_snap(&mut self, id: EntityId) {
        if let Some(inf) = &mut self.inference {
            inf.kind = InferenceKind::Center;
            inf.constraints = vec![AutoConstraint::ConcentricTo(id)];
        }
        self.external_snap = None;
    }

    /// Constrain the next click onto an external entity (a snapped model edge), keeping a direction met on it; at a
    /// straight edge's midpoint, its MIDPOINT.
    pub fn link_edge_snap(&mut self, id: EntityId) {
        let midpoint = matches!(self.external_snap, Some(ExternalSnap::Edge { midpoint: true, .. }));
        if let Some(inf) = &mut self.inference {
            if midpoint {
                inf.kind = InferenceKind::Midpoint;
                inf.constraints = vec![AutoConstraint::MidpointOf(id)];
            } else {
                if inf.constraints.is_empty() {
                    inf.kind = InferenceKind::OnCurve;
                }
                inf.constraints.push(AutoConstraint::OnCurve(id));
            }
        }
        self.external_snap = None;
    }

    /// The woken model edge's perpendicular through the anchor (live sketch-gaps 1c): a line zone as wide as the
    /// horizontal's (7–9 px either side 60 mm out), not an angle; nothing for a level or plumb edge (the horizontal and
    /// vertical give it).
    fn woken_edge_perpendicular(&self, p: DVec2, anchor: Option<DVec2>) -> Option<Inference> {
        let (a, (_, s, e)) = (anchor?, self.woken_edge.as_ref()?);
        let dir = (*e - *s).try_normalize()?;
        if dir.x.abs() < 1e-9 || dir.y.abs() < 1e-9 || !matches!(self.tool, Tool::Line) {
            return None;
        }
        let n = dir.perp();
        let u = if (p - a).dot(n) < 0.0 { -n } else { n };
        let foot = a + u * (p - a).dot(u);
        let seen = self.screen_metric().map_or((p - foot).length(), |m| m.len(p - foot));
        (seen <= self.world_per_px * INFERENCE_PX && (foot - a).length() > self.world_per_px * INFERENCE_PX).then(|| Inference {
            kind: InferenceKind::Perpendicular,
            position: foot,
            entity: None,
            reference: Some(a),
            constraints: vec![AutoConstraint::PerpendicularTo(WOKEN_EDGE)],
            distance: seen,
            priority: 2,
        })
    }

    /// The click takes the woken model edge's perpendicular or an end's alignment: the edge is used in `sk` as `id`.
    pub fn link_woken_edge(&mut self, sk: &Sketch, id: EntityId) {
        if self.inference.as_ref().is_some_and(|i| i.constraints.iter().any(|c| auto_target(c) == Some(WOKEN_EDGE_MID)))
            && let Some(w) = &mut self.woken_edge_mid
        {
            w.2 = sk.curve(id).is_some().then_some(id);
            return;
        }
        if sk.point(id).is_some() {
            if let Some(inf) = &mut self.inference {
                for c in &mut inf.constraints {
                    *c = retarget_auto(*c, WOKEN_POINT, id);
                }
            }
            return;
        }
        let ends = self.woken_edge.as_ref().map(|w| [w.1, w.2]);
        let used = sk.endpoints(id).and_then(|(a, b)| Some((a, sk.point(a)?, b, sk.point(b)?)));
        let Some(inf) = &mut self.inference else { return };
        for c in &mut inf.constraints {
            *c = retarget_auto(*c, WOKEN_EDGE, id);
            for (k, end) in WOKEN_EDGE_ENDS.into_iter().zip(ends.into_iter().flatten()) {
                if let Some((a, pa, b, pb)) = used {
                    *c = retarget_auto(*c, k, if (pa - end).length() <= (pb - end).length() { a } else { b });
                }
            }
        }
    }

    /// The inference about to be clicked needs the woken model edge (or the woken point outside the sketch) used.
    pub fn needs_woken_edge(&self) -> Option<&noshape_model::reference::Reference> {
        let i = self.inference.as_ref()?;
        if i.constraints.iter().any(|c| auto_target(c) == Some(WOKEN_POINT)) {
            return self.woken_point.as_ref().map(|w| &w.0);
        }
        if i.constraints.iter().any(|c| auto_target(c) == Some(WOKEN_EDGE_MID)) {
            return self.woken_edge_mid.as_ref().map(|w| &w.0);
        }
        let woken = |e: EntityId| e == WOKEN_EDGE || WOKEN_EDGE_ENDS.contains(&e);
        i.constraints.iter().any(|c| auto_target(c).is_some_and(woken)).then(|| self.woken_edge.as_ref().map(|w| &w.0))?
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
        // (Edge snaps are linked by the view, which can project the edge.) Live 11-18 use-glyph: the snap makes only a
        // Coincident to the point, not a used (projected) point: no use glyph.
        let id = data.sketch.use_external_point(position);
        let _ = data.sketch.set_reference_only(id, true);
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
        self.pointer_rests = false;
        // A following paste is placed where it is, and the tool ends (live feel round 2 R6: tool off, nothing selected).
        if self.paste_follow.is_some() && self.tool == Tool::Transform {
            self.selection.clear();
            self.set_tool(Tool::Select);
            return EditOutcome::redraw();
        }
        // A dimension being placed takes the press (live: ui-parity
        // sketch-dimension-edit, its value over the line's Horizontal glyph),
        // and so does a drawing tool (live ui-parity revolve-dialog-axis: a
        // Line started over a rectangle's glyph draws).
        // Mirror picks its entities through them too (its selected line's glyphs show).
        if (self.tool == Tool::Dimension && !self.picks.is_empty()) || self.tool.draws() || (self.tool == Tool::Mirror && self.mirror_axis.is_some()) {
            self.pressed_label = None;
            self.pressed_glyph = None;
        }
        // Overlay labels and glyphs take the press first, whatever the tool.
        if let Some((cid, additive, select)) = self.pressed_label.take() {
            // `moved: true` up front suppresses the selection toggle (double-click edit).
            self.drag = Some(DragState::Label { constraint: cid, start: p, moved: !select, additive, dragging: false });
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
            self.xf_grab = matches!(h, PatternHandle::Move | PatternHandle::MoveX | PatternHandle::MoveY).then_some((p, self.xf_offset));
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
                    self.chain_end_press = Some(p);
                    return EditOutcome::redraw();
                }
                // Continuing to sketch leaves the last geometry undimensioned.
                self.otf = None;
                self.otf_typing = false;
                let inference = self.link_woken_mids(&mut data.sketch, inference);
                let out = self.place(data, Click { pos, inference }, double_click);
                // Press-drag-release: the release places the next click.
                let pending = !self.clicks.is_empty() || (self.tool == Tool::Line && self.chain_end.is_some());
                self.press = (pending && !on_chain_end && !matches!(self.tool, Tool::Spline | Tool::Bezier | Tool::Point)).then_some(p);
                out
            }
            Tool::Fillet if self.fillet_arrow(&data.sketch).is_some_and(|(a, b)| dist_to_segment(p, a, b) <= self.world_per_px * 10.0) => {
                if self.invalid_corner.is_some() {
                    self.drag = Some(DragState::FilletInvalid);
                } else if let Some((arc, _)) = self.fillet_handle() {
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
            // A line's picked end goes where the pointer sets it (`extend_target`).
            Tool::Extend if self.extend_end.is_some() => {
                let Some((q, on)) = self.extend_target(&data.sketch, p) else { return EditOutcome::none() };
                let end = self.extend_end.take().unwrap_or(EntityId(0));
                self.extend_from = None;
                self.selection.clear();
                if data.sketch.set_point(end, q).is_err() {
                    return EditOutcome::redraw();
                }
                if let Some(c) = on {
                    let _ = data.sketch.add_constraint(ConstraintKind::Coincident { a: end, b: c });
                }
                data.sketch.solve();
                prune_exprs(data);
                EditOutcome::changed()
            }
            Tool::Extend | Tool::Split => {
                // The curve under the click, even where an end point is nearer
                // (live: Extend 2 mm from a line's end takes the line).
                let r = self.world_per_px * 8.0;
                let nearest_curve = || {
                    let sk = &data.sketch;
                    sk.entities().filter(|e| e.kind() != EntityKind::Point).filter_map(|e| sk.distance_to(e.id, p).filter(|d| *d <= r).map(|d| (d, e.id))).min_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1)
                };
                let Some(id) = hovered.filter(|e| !is_point(&data.sketch, *e)).or_else(nearest_curve) else {
                    return EditOutcome::none();
                };
                let r = match self.tool {
                    // Live: the first pick is the curve to extend (it stays
                    // lit), the second the boundary.
                    Tool::Extend => match self.extend_from.take().filter(|c| *c != id && data.sketch.entity(*c).is_some()) {
                        // It is selected too (live: selection 1, its length in the status bar).
                        None => {
                            self.extend_from = Some(id);
                            self.selection = vec![id];
                            // A line: its nearer end is picked and follows the pointer (live 11-18 sketch-tools).
                            self.extend_end = nearer_line_end(&data.sketch, id, p);
                            self.extend_crossed = false;
                            return EditOutcome::redraw();
                        }
                        Some(curve) => {
                            self.selection.clear();
                            data.sketch.extend(curve, id)
                        }
                    },
                    _ => {
                        let ends = data.sketch.endpoints(id);
                        let (at, mid) = self.split_point(&data.sketch, id, p);
                        let used = used_ends_before(data, id);
                        let r = data.sketch.split(id, at);
                        if let Ok(res) = &r {
                            if mid
                                && let (Some(&point), Some((a, b))) = (res.created.first(), ends)
                            {
                                let _ = data.sketch.add_constraint(ConstraintKind::MidpointOf { point, a, b });
                            }
                            keep_used_ends(data, id, used, false);
                        }
                        r
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
                // A curve's end point stands for its curve (live walk3-1121 6b: a line's end on the Origin).
                let hovered = hovered.map(|h| match data.sketch.users_of_point(h).as_slice() {
                    [only] if is_point(&data.sketch, h) => *only,
                    _ => h,
                });
                let ids: Vec<EntityId> = if self.selection.is_empty() || self.slot_pending() { hovered.into_iter().collect() } else { std::mem::take(&mut self.selection) };
                self.make_slots(data, &ids)
            }
            Tool::Offset => {
                // The direction arrow: drag to set the distance, click to flip.
                if let Some((base, tip)) = self.offset_arrow(&data.sketch)
                    && (p - tip).length().min((p - (base + tip) * 0.5).length()) <= self.world_per_px * 9.0
                {
                    let sign = if (self.offset_flip != self.offset_out) != self.offset_flip_base(&data.sketch) { -1.0 } else { 1.0 };
                    self.drag = Some(DragState::OffsetArrow { start: p, moved: false, from: sign * self.offset_distance });
                    return EditOutcome::redraw();
                }
                // While previewing, the picked curves stay selected (live: orange) and a click picks under the pointer.
                let ids: Vec<EntityId> = if self.selection.is_empty() || self.offset_pending.is_some() { hovered.into_iter().filter(|e| !is_point(&data.sketch, *e)).collect() } else { std::mem::take(&mut self.selection) };
                if ids.is_empty() {
                    // Live (workflows-1122 22 s18, ui-parity sketch-offset-key): a click away from the curves places
                    // the offset as previewed, whatever the click's side and distance, and opens its distance box.
                    if self.offset_pending.is_none() {
                        return EditOutcome::none();
                    }
                    // (Placed by a click the tool ends: live's tool is none after it.)
                    let out = self.commit_offset(data);
                    if out.changed {
                        self.tool = Tool::Select;
                        self.offset_repeat = false;
                    }
                    return out;
                }
                if self.offset_repeat && self.offset_pending.is_none() {
                    // Later clicks offset by the same distance right away.
                    self.start_offset(&data.sketch, ids);
                    return self.commit_offset(data);
                }
                match &mut self.offset_pending {
                    // Add connected curves to the pending offset (chain
                    // selection). Live (qa4 OFF-TOAST): a curve off the chain
                    // is not added, only warned about.
                    Some(p) => {
                        // Live (ui-parity sketch-offset): a picked curve clicked again leaves the offset; with none
                        // left the preview goes and nothing is made.
                        if ids.iter().all(|e| p.contains(e)) {
                            p.retain(|e| !ids.contains(e));
                            if p.is_empty() {
                                self.offset_pending = None;
                                self.selection.clear();
                                return EditOutcome::redraw();
                            }
                        }
                        let Some(p) = self.offset_pending.as_mut() else { return EditOutcome::redraw() };
                        for e in ids {
                            if p.contains(&e) {
                                continue;
                            }
                            if !p.iter().any(|q| curves_touch(&data.sketch, *q, e)) {
                                self.toast = Some("Can only offset a single chain at once.".into());
                                return EditOutcome::redraw();
                            }
                            p.push(e);
                        }
                        // A longer chain may run the other way: the default
                        // side stays away from the region.
                        self.offset_out = offset_goes_in(&data.sketch, p);
                    }
                    None => self.start_offset(&data.sketch, ids),
                }
                self.selection = self.offset_pending.clone().unwrap_or_default();
                // Live shows no prompt for it.
                self.message = None;
                EditOutcome::redraw()
            }
            Tool::Mirror => {
                let is_line = |e: &EntityId| data.sketch.kind(*e) == Some(EntityKind::Line);
                match self.mirror_axis.filter(|a| data.sketch.entity(*a).is_some()) {
                    None => {
                        let Some(axis) = hovered.filter(is_line) else {
                            self.message = Some(MIRROR_LINE_PROMPT.into());
                            return EditOutcome::none();
                        };
                        self.mirror_axis = Some(axis);
                        let ids: Vec<EntityId> = self.selection.iter().copied().filter(|e| *e != axis).collect();
                        if ids.is_empty() {
                            // Live (ui-parity sketch-mirror-tool): the mirror line stays selected while it asks.
                            self.selection = vec![axis];
                            self.message = Some(MIRROR_ENTITIES_PROMPT.into());
                            return EditOutcome::redraw();
                        }
                        self.selection.clear();
                        // Pre-selection: mirrored at once; the tool stays on, asking for more (live
                        // sketch-mirror-preselected-batch).
                        self.mirror_entities(data, &ids)
                    }
                    Some(axis) => {
                        // A click on the mirror line itself deselects it; it stays the mirror line (live
                        // sketch-mirror-construction-line).
                        if hovered == Some(axis) {
                            self.selection.retain(|e| *e != axis);
                            return EditOutcome::redraw();
                        }
                        let Some(e) = hovered else { return EditOutcome::none() };
                        self.mirror_entities(data, &[e])
                    }
                }
            }
            Tool::LinearPattern | Tool::CircularPattern | Tool::Transform => {
                match hovered {
                    Some(e) if data.text_of_entity(e).is_some() && !is_point(&data.sketch, e) => self.toggle_text_selection(data, e),
                    // Transform keeps a picked entity clicked again (live uip sketch-entity-copy-paste: a click on the
                    // pasted line leaves it selected, the tool on).
                    Some(e) if self.tool == Tool::Transform && self.selection.contains(&e) => {}
                    Some(e) => {
                        if let Some(pos) = self.selection.iter().position(|x| *x == e) {
                            self.selection.remove(pos);
                        } else {
                            self.selection.push(e);
                        }
                    }
                    // Clicking white space accepts the pattern / transform (Onshape); a pattern's tool ends there
                    // (live sketch-linear-pattern-click-commit, sketch-circular-pattern-click-commit: tool none).
                    None if !self.selection.is_empty() => {
                        let out = self.apply_pattern(data);
                        if out.changed && self.tool != Tool::Transform {
                            self.set_tool(Tool::Select);
                            self.selection.clear();
                        }
                        return out;
                    }
                    None => {}
                }
                EditOutcome::redraw()
            }
            Tool::Dimension => self.dimension_click(data, p, hovered),
            Tool::Constraint(c) => {
                let Some(e) = hovered else { return EditOutcome::none() };
                // Live: a pick the constraint can't take isn't accepted (and
                // nothing is said).
                let picked: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
                if !picked.contains(&e) && !pick_fits(&data.sketch, c, &picked, e) {
                    return EditOutcome::none();
                }
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
                    // Picks that fit but can't take the constraint: nothing is added.
                    if let Some(kind) = make_constraint(&data.sketch, c, &ids)
                        && !data.sketch.constraints().any(|x| x.kind == kind)
                        && data.sketch.add_constraint(kind).is_ok()
                    {
                        // A selection the tool started with (Midpoint on two points) goes with its constraint.
                        self.selection.retain(|x| !ids.contains(x));
                        if !solve_after_constraint(&mut data.sketch, c, &ids) {
                            self.toast = Some(SKETCH_NOT_SOLVED.into());
                        }
                        return EditOutcome::changed();
                    }
                }
                EditOutcome::redraw()
            }
            _ => EditOutcome::none(),
        }
    }

    /// Fillet / chamfer tool click: a corner point, or the first / second curve.
    /// Extend's preview for the pointer at `p` (live mc-reach-1122 R6): to the pointer's foot, past a boundary too; onto
    /// the boundary (its curve given) only while the pointer is on it. The click still stops there ([`Self::extend_target`]).
    pub fn extend_preview(&self, sk: &Sketch, p: DVec2) -> Option<(DVec2, Option<EntityId>)> {
        let (q, on) = self.extend_target(sk, p)?;
        let Some(c) = on else { return Some((q, None)) };
        if sk.distance_to(c, p).is_some_and(|d| d <= self.world_per_px * INFERENCE_PX) {
            return Some((q, on));
        }
        let (line, end) = (self.extend_from?, self.extend_end?);
        let (s, e) = sk.endpoints(line)?;
        let a = sk.point(if e == end { s } else { e })?;
        let d = (sk.point(end)? - a).try_normalize()?;
        Some((a + d * (p - a).dot(d), None))
    }

    /// Extend (live 11-18 sketch-tools t4-ext, t7): where the picked line end goes for the pointer at `p`: along the
    /// line to the pointer's foot, back over the line too (shorter); past a crossing curve it stops on the first one
    /// (even clicked 42 px beyond it). The place and the curve it stops on.
    pub fn extend_target(&self, sk: &Sketch, p: DVec2) -> Option<(DVec2, Option<EntityId>)> {
        let (line, end) = (self.extend_from?, self.extend_end?);
        let (s, e) = sk.endpoints(line)?;
        let fixed = if e == end { s } else { e };
        let (a, b) = (sk.point(fixed)?, sk.point(end)?);
        let d = (b - a).try_normalize()?;
        let t = (p - a).dot(d).max(self.world_per_px);
        let q = a + d * t;
        if t <= (b - a).length() {
            return Some((q, None));
        }
        let path = Curve2::Line { a: b, b: q };
        let near_end = self.world_per_px * 0.5;
        let hit = sk
            .entities()
            .filter(|x| x.id != line && x.kind() != EntityKind::Point && x.handle_of.is_none())
            .filter_map(|x| sk.curve(x.id).map(|c| (x.id, c)))
            .flat_map(|(id, c)| noshape_sketch::intersect::intersect(&path, &c, self.world_per_px * 0.01).into_iter().map(move |h| (h.point, id)))
            .filter(|(at, _)| at.distance(b) > near_end)
            .min_by(|x, y| x.0.distance(b).total_cmp(&y.0.distance(b)));
        Some(match hit {
            Some((at, id)) => (at, Some(id)),
            None => (q, None),
        })
    }

    fn corner_click(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>) -> EditOutcome {
        if self.pending_corner.is_some() {
            return self.commit_corner(data);
        }
        // Another corner replaces a red preview.
        self.invalid_corner = None;
        let chamfer = self.tool == Tool::Chamfer;
        let corner = hovered.filter(|e| is_point(&data.sketch, *e)).or_else(|| if hovered.is_none() { nearest_point(&data.sketch, p, self.world_per_px * 10.0) } else { None });
        // The corner point, or the two curves.
        let target = if let Some(pid) = corner {
            self.picks.clear();
            (pid, None)
        } else if let Some(c) = hovered {
            if let Some(&(first, _)) = self.picks.first().filter(|(f, _)| *f != c) {
                self.picks.clear();
                (first, Some(c))
            } else {
                self.picks = vec![(c, p)];
                return EditOutcome::redraw();
            }
        } else {
            return EditOutcome::none();
        };
        let mut preview = data.clone();
        let sk = &mut preview.sketch;
        let (dist, r) = (self.chamfer_distance, self.fillet_radius);
        let made = match (target, chamfer) {
            ((pid, None), true) => sk.chamfer_corner(pid, dist, dist),
            ((pid, None), false) => sk.fillet_corner(pid, r),
            ((a, Some(b)), true) => sk.chamfer(a, b, dist, dist),
            ((a, Some(b)), false) => sk.fillet(a, b, r),
        };
        match made {
            Ok(res) => {
                self.pending_corner = Some((preview, res));
                self.corner_pressed = Some(p);
                self.pending_corner_pid = target.1.is_none().then_some(target.0);
                // Live (ui-parity sketch-fillet-corner): the clicked corner
                // stays selected until the fillet is confirmed.
                if target.1.is_none() {
                    self.selection = vec![target.0];
                }
                // Live shows no prompt for it (ui-parity sketch-fillet-corner).
                self.message = None;
                EditOutcome::redraw()
            }
            // Live (behaviours "qa (2026-10-28) SKETCH-FILLET-STUCK-RADIUS: two
            // corners in one fillet session"): a radius the corner can't take
            // previews red at that radius, with no value box; Enter does
            // nothing; its arrow drags it to one that fits.
            Err(_) if !chamfer && target.1.is_none() && corner_arc(&data.sketch, target.0, r).is_some() => {
                self.invalid_corner = Some(target.0);
                self.message = None;
                EditOutcome::redraw()
            }
            // Live's text alone (i18n; FILLET-POINT-MSG: the solver's reason
            // isn't shown).
            Err(_) => {
                self.message = Some(if chamfer { "Cannot chamfer selected point.".to_string() } else { "Cannot add sketch fillet.".to_string() });
                EditOutcome::redraw()
            }
        }
    }

    /// The fillet of the corner `pid` at `r` on a copy of `data`, if valid.
    fn try_fillet_corner(data: &SketchData, pid: EntityId, r: f64) -> Option<(SketchData, noshape_sketch::EditResult)> {
        let mut preview = data.clone();
        let res = preview.sketch.fillet_corner(pid, r).ok()?;
        Some((preview, res))
    }

    pub fn corner_pending(&self) -> bool {
        self.pending_corner.is_some() || self.invalid_corner.is_some()
    }

    pub fn commit_corner(&mut self, data: &mut SketchData) -> EditOutcome {
        // Enter on a red preview does nothing (live).
        let Some((preview, result)) = self.pending_corner.take() else { return EditOutcome::none() };
        if let Some(pid) = self.pending_corner_pid.take() {
            self.selection.retain(|e| *e != pid);
        }
        *data = preview;
        self.finish_create(data, &result.created);
        if self.tool == Tool::Chamfer {
            self.after_chamfer(data, &result);
        } else {
            // The committed radius is the tab's default from now on.
            if let Some(Curve2::Arc { radius, .. }) = result.created.iter().find_map(|e| data.sketch.curve(*e).filter(|c| matches!(c, Curve2::Arc { .. }))) {
                self.fillet_radius = radius;
            }
            self.after_fillet(data, &result);
        }
        self.message = None;
        data.sketch.solve();
        EditOutcome::changed()
    }

    /// Every fillet keeps its own radius dimension, its value box open
    /// (live, behaviours "qa (2026-10-28) SKETCH-FILLET-STUCK-RADIUS": no
    /// Equal between the fillets of a session).
    fn after_fillet(&mut self, data: &mut SketchData, r: &noshape_sketch::EditResult) {
        let Some(arc) = r.created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc)) else { return };
        let rad_dim = r.constraints_added.iter().copied().find(|c| data.sketch.constraint(*c).is_some_and(|c| matches!(c.kind, ConstraintKind::Radius { .. })));
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

    /// A new pending offset: the arrow flip is not remembered from the
    /// last one, and the default side goes away from the region the first
    /// curve bounds (live, qa4 OFF-SIDE).
    fn start_offset(&mut self, sk: &Sketch, ids: Vec<EntityId>) {
        self.offset_flip = false;
        self.offset_out = offset_goes_in(sk, &ids);
        self.offset_pending = Some(ids);
    }

    /// Offset tool on a model face (live: accepted, its outer loop offset as one chain): `ids` the loop's reference
    /// curves. With an offset already pending it isn't added (one chain at once).
    pub fn offset_loop(&mut self, sk: &Sketch, ids: Vec<EntityId>) -> EditOutcome {
        if ids.is_empty() || self.tool != Tool::Offset {
            return EditOutcome::none();
        }
        if self.offset_pending.is_some() {
            self.toast = Some("Can only offset a single chain at once.".into());
            return EditOutcome::redraw();
        }
        self.start_offset(sk, ids);
        self.offset_out_of_face(sk);
        self.selection = self.offset_pending.clone().unwrap_or_default();
        self.message = None;
        EditOutcome::redraw()
    }

    /// A face's pending offset goes out of the face by default (its loop's reference curves bound no region of the
    /// sketch): the side whose copies span more.
    pub fn offset_out_of_face(&mut self, sk: &Sketch) {
        let span = |ed: &Self| {
            ed.offset_result(sk).map_or(0.0, |(tmp, made)| made.iter().filter_map(|e| tmp.curve(*e)).fold(noshape_sketch::BBox::EMPTY, |b, c| b.union(&c.bbox())).size().length())
        };
        let here = span(self);
        self.offset_out = !self.offset_out;
        if span(self) <= here {
            self.offset_out = !self.offset_out;
        }
    }

    /// The pending offset's signed distance for `Sketch::offset`.
    fn offset_signed(&self) -> f64 {
        if self.offset_flip != self.offset_out { -self.offset_distance } else { self.offset_distance }
    }

    /// Offset curves of the pending offset with the current distance (preview / commit).
    fn offset_result(&self, sk: &Sketch) -> Option<(Sketch, Vec<EntityId>)> {
        let ids = self.offset_pending.as_ref()?;
        let d = self.offset_signed();
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

    /// Whether a positive offset of the pending chain goes against its first
    /// curve's left normal.
    fn offset_flip_base(&self, sk: &Sketch) -> bool {
        let Some(ids) = self.offset_pending.as_ref() else { return false };
        let Some((mid, n)) = self.offset_frame(sk) else { return false };
        let mut tmp = sk.clone();
        let Ok(r) = tmp.offset(ids, self.world_per_px.max(1e-9)) else { return false };
        let probe = r.created.iter().filter_map(|e| tmp.curve(*e)).map(|c| c.closest(mid).1).min_by(|x, y| (*x - mid).length().total_cmp(&(*y - mid).length()));
        probe.is_some_and(|q| (q - mid).dot(n) < 0.0)
    }

    /// How the view shows the plane, for pick and inference reach measured on screen as live's (none headless, or
    /// before the view has set its map).
    pub fn screen_metric(&self) -> Option<noshape_sketch::ScreenMetric> {
        if self.exact_inference {
            return None;
        }
        let (_, ex, ey) = self.screen_map?;
        let m = glam::DMat2::from_cols(ex, ey);
        (m.determinant().abs() > 1e-30).then(|| noshape_sketch::ScreenMetric(m.inverse() * self.world_per_px))
    }

    /// A sketch point in screen pixels (itself without a screen map).
    fn plane_to_screen(&self, p: DVec2) -> DVec2 {
        let Some((o, ex, ey)) = self.screen_map else { return p };
        let m = glam::DMat2::from_cols(ex, ey);
        if m.determinant().abs() < 1e-30 {
            return p;
        }
        m.inverse() * (p - o)
    }

    #[cfg(test)]
    fn screen_to_plane(&self, s: DVec2) -> DVec2 {
        let Some((o, ex, ey)) = self.screen_map else { return s };
        o + glam::DMat2::from_cols(ex, ey) * s
    }

    /// An entity's outline in screen pixels (a point: itself).
    fn screen_outline(&self, sk: &Sketch, id: EntityId) -> Option<Vec<DVec2>> {
        let pts = match sk.point(id) {
            Some(p) => vec![p],
            None => sk.tessellate(id, self.world_per_px),
        };
        let pts: Vec<DVec2> = if pts.is_empty() {
            let bb = sk.entity_bbox(id)?;
            vec![bb.min, DVec2::new(bb.max.x, bb.min.y), bb.max, DVec2::new(bb.min.x, bb.max.y)]
        } else {
            pts
        };
        Some(pts.into_iter().map(|p| self.plane_to_screen(p)).collect())
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

    /// The offset manipulator (live offset-look-1121): Onshape's hollow arrow, its tail on the offset copy, pointing
    /// away from the curve; (start of the 18 px stem, apex of the 31 px head).
    pub fn offset_arrow(&self, sk: &Sketch) -> Option<(DVec2, DVec2)> {
        let (mid, n) = self.offset_frame(sk)?;
        let sign = if (self.offset_flip != self.offset_out) != self.offset_flip_base(sk) { -1.0 } else { 1.0 };
        let (dir, px) = (n * sign, self.world_per_px);
        let start = mid + dir * self.offset_distance;
        Some((start, start + dir * px * (OFFSET_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX)))
    }

    /// The pending offset's distance label; clicking it opens the distance box (Onshape's offset tool opens its
    /// in-place edit view from the preview dimension; the manipulator has none). Live offset-look-1121: in the gap
    /// at ~18 px, on the picked curve at ~9 px and less (drawn at any gap).
    pub fn offset_value_label(&self, sk: &Sketch) -> Option<(DVec2, f64)> {
        let (mid, dir) = self.offset_dimension(sk)?;
        let into_gap = if self.offset_distance >= self.world_per_px * OFFSET_LABEL_IN_GAP_PX { 0.5 } else { 0.0 };
        Some((mid + dir * self.offset_distance * into_gap, self.offset_distance))
    }

    /// The pending offset's dimension: the first picked curve's middle and the unit direction to its copy.
    fn offset_dimension(&self, sk: &Sketch) -> Option<(DVec2, DVec2)> {
        if self.tool != Tool::Offset {
            return None;
        }
        let (mid, n) = self.offset_frame(sk)?;
        let sign = if (self.offset_flip != self.offset_out) != self.offset_flip_base(sk) { -1.0 } else { 1.0 };
        Some((mid, n * sign))
    }

    /// Create the pending offset (Enter / click in space); later clicks repeat it.
    pub fn commit_offset(&mut self, data: &mut SketchData) -> EditOutcome {
        let d = self.offset_signed();
        let Some(ids) = self.offset_pending.take() else { return EditOutcome::none() };
        self.selection.retain(|e| !ids.contains(e));
        match data.sketch.offset(&ids, d) {
            Ok(r) => {
                self.offset_repeat = true;
                self.finish_create(data, &r.created);
                // Live (qa4 OFF-BOX): Enter creates the offset and opens its
                // distance's value box, the value pre-selected.
                let dist = r.constraints_added.iter().copied().find(|c| matches!(data.sketch.constraint(*c).map(|c| &c.kind), Some(ConstraintKind::Distance { .. })));
                if let Some(cid) = dist {
                    self.open_value_box(data, cid);
                }
                EditOutcome::changed()
            }
            Err(e) => {
                self.message = Some(format!("Offset failed: {e}"));
                EditOutcome::redraw()
            }
        }
    }

    /// The fillet radius manipulator: the pending fillet's arc and its midpoint (live 1935: gone once Enter commits).
    fn fillet_handle(&self) -> Option<(EntityId, DVec2)> {
        let (data, result) = self.pending_corner.as_ref()?;
        let arc = result.created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc))?;
        Some((arc, curve_mid(&data.sketch, arc)?))
    }

    /// The radius manipulator (live sketch/1934): from the fillet arc's
    /// midpoint toward its centre; (start, apex of the head).
    pub fn fillet_arrow(&self, sk: &Sketch) -> Option<(DVec2, DVec2)> {
        if let Some(pid) = self.invalid_corner {
            let a = corner_arc(sk, pid, self.fillet_radius)?;
            let m = a.mid();
            let d = (a.center - m).try_normalize()?;
            return Some((m, m + d * self.world_per_px * (FILLET_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX)));
        }
        let (arc, m) = self.fillet_handle()?;
        let sk = self.pending_corner.as_ref().map(|(d, _)| &d.sketch).unwrap_or(sk);
        let Some(Curve2::Arc { center, .. }) = sk.curve(arc) else { return None };
        let d = (center - m).try_normalize()?;
        Some((m, m + d * self.world_per_px * (FILLET_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX)))
    }

    /// Where the pending fillet's "R" leader meets the arc: its upper end
    /// (live sketch/1934: the tangent point on the top edge).
    fn fillet_label_foot(&self, sk: &Sketch) -> Option<DVec2> {
        let (arc, _) = self.fillet_handle()?;
        let sk = self.pending_corner.as_ref().map(|(d, _)| &d.sketch).unwrap_or(sk);
        let Some(Curve2::Arc { center, radius, start_angle, end_angle }) = sk.curve(arc) else { return None };
        let (a, b) = (center + DVec2::from_angle(start_angle) * radius, center + DVec2::from_angle(end_angle) * radius);
        Some(if (a.y - b.y).abs() > 1e-12 { if a.y > b.y { a } else { b } } else if a.x < b.x { a } else { b })
    }

    /// The pending fillet's radius label ("R5", live sketch/1934): its
    /// right-centre anchor, up-left of the leader, and the radius.
    pub fn fillet_value_label(&self, sk: &Sketch) -> Option<(DVec2, f64)> {
        if self.tool != Tool::Fillet {
            return None;
        }
        let px = self.world_per_px;
        if let Some(pid) = self.invalid_corner {
            let a = corner_arc(sk, pid, self.fillet_radius)?;
            let (pa, pb) = (a.ends[0], a.ends[1]);
            let foot = if (pa.y - pb.y).abs() > 1e-12 { if pa.y > pb.y { pa } else { pb } } else if pa.x < pb.x { pa } else { pb };
            return Some((foot + DVec2::new(-7.5 * px, 13.0 * px), self.fillet_radius));
        }
        self.pending_corner.as_ref()?;
        let pa = self.fillet_label_foot(sk)?;
        // The preview's own radius (it follows the arrow's drag).
        let (arc, _) = self.fillet_handle()?;
        let r = match self.pending_corner.as_ref().and_then(|(d, _)| d.sketch.curve(arc)) {
            Some(Curve2::Arc { radius, .. }) => radius,
            _ => self.fillet_radius,
        };
        Some((pa + DVec2::new(-7.5 * px, 13.0 * px), r))
    }

    /// Preview of the clicked corner's fillet: the arc, and whether it fits (live sketch/110, 1933: a hovered corner
    /// only lights its point; the arc comes on the click).
    fn fillet_preview(&self, sk: &Sketch) -> Option<(Vec<DVec2>, bool)> {
        if let Some((data, result)) = &self.pending_corner {
            let arc = result.created.iter().copied().find(|e| data.sketch.kind(*e) == Some(EntityKind::Arc))?;
            return Some((data.sketch.tessellate(arc, self.world_per_px), true));
        }
        let a = corner_arc(sk, self.invalid_corner?, self.fillet_radius)?;
        Some((arc_points(a.center, self.fillet_radius, a.a0, a.sweep), false))
    }

    /// Open the value box of a dimension (the Onshape dimension context dialog).
    fn open_value_box(&mut self, data: &SketchData, cid: ConstraintId) {
        let Some(text) = dimension_edit_text(data, cid, &UnitPrefs::default()) else { return };
        self.dim_edit = Some(DimEdit { constraint: cid, text, created: true });
    }

    /// Trim tool: trim the curve under `p` (click, or each curve a drag crosses).
    fn trim_at(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>) -> EditOutcome {
        let r = self.world_per_px * 6.0;
        let id = hovered.or_else(|| data.sketch.pick_on_screen(p, r, self.screen_metric().as_ref()));
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
            let used = used_ends_before(data, id);
            let r = data.sketch.trim_near(id, p, self.world_per_px * TRIM_CORNER_PX).map(|_| ());
            if r.is_ok() && data.sketch.entity(id).is_some() {
                keep_used_ends(data, id, used, true);
            }
            r
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
            // Live (ui-parity sketch-slot-on-line): the picked curves stay
            // selected until the slot commits.
            self.selection = curves;
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
        self.selection.clear();
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
            Ok(_) => {
                // A copy is what its source is, construction mode or not (live sketch-mirror-construction-line).
                data.sketch.solve();
                // Live (ui-parity sketch-mirror-tool): nothing stays selected, the prompt stays.
                self.selection.clear();
                self.message = Some(MIRROR_ENTITIES_PROMPT.into());
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
                // Always a new point: on an existing point (the Origin too)
                // it is Coincident with it (live, behaviours "dx (2026-10-26,
                // for skasm) Point tool on the Origin").
                let pid = sk.add_point(click.pos);
                let autos = click.inference.as_ref().map(|i| i.constraints.clone()).unwrap_or_default();
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
                        self.set_otf(data, vec![radius_field(&data.sketch, arc, self.world_per_px, tangent_arc_box(&data.sketch, arc, self.world_per_px))]);
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
                // Closing onto an existing point ends the chain (Onshape); the Origin doesn't (live walk3-1121 (2)).
                let closed = click.inference.as_ref().is_some_and(|i| i.kind == InferenceKind::Endpoint && !i.entity.is_some_and(|e| e == noshape_sketch::ORIGIN || is_origin(sk, e)));
                self.finish_create(data, &[line]);
                if closed {
                    self.clicks.clear();
                    self.chain_end = None;
                } else {
                    self.clicks = vec![click];
                    self.chain_end = Some(b);
                    self.left_chain_end = false;
                }
                self.set_otf(data, vec![line_length_field(&data.sketch, line, self.world_per_px, true)]);
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
                // Live: the line runs from the clicked end to the mirrored one.
                let pa = sk.add_point(m.pos * 2.0 - click.pos);
                let pb = sk.add_point(click.pos);
                let Ok(line) = sk.add_line_between(pb, pa) else { return EditOutcome::none() };
                let mid = sk.add_point(m.pos);
                let _ = sk.add_constraint(ConstraintKind::Midpoint { point: mid, curve: line });
                attach(sk, mid, None, &m);
                attach(sk, pb, Some(line), &click);
                self.finish_create(data, &[line]);
                self.set_otf(data, vec![line_length_field(&data.sketch, line, self.world_per_px, false)]);
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
                // two edges through the first corner (the edges at the first
                // corner's y, the second's x, the second's y, the first's x).
                let mid = |id: EntityId| match data.sketch.curve(id) {
                    Some(Curve2::Line { a, b }) => (a + b) * 0.5,
                    _ => DVec2::ZERO,
                };
                let (horizontal, vertical) = if self.tool == Tool::CornerRect {
                    (ids.curves[0], ids.curves[3])
                } else {
                    // Live (behaviours "… center point rectangle and tangent
                    // arc", request 1503): the width on the bottom edge, the
                    // height on the vertical edge opposite the corner click.
                    let edge = |key: fn(DVec2) -> f64| ids.curves.iter().copied().take(4).min_by(|x, y| key(mid(*x)).total_cmp(&key(mid(*y))));
                    let bottom = edge(|m| m.y).unwrap_or(ids.curves[0]);
                    let opposite = if click.pos.x > first.pos.x { edge(|m| m.x) } else { edge(|m| -m.x) };
                    (bottom, opposite.unwrap_or(ids.curves[1]))
                };
                let mut fields = vec![length_field(&data.sketch, horizontal, w)];
                if !self.alt_held {
                    fields.push(length_field(&data.sketch, vertical, w));
                }
                // Live (behaviours "… value boxes … rule capture" (3)): the width
                // box outside the first click's horizontal edge, 14 px off; the
                // height as blue text outside the first click's vertical edge,
                // 16 px off; once it is the box, inside 14 px from the other
                // vertical edge.
                if self.tool == Tool::CenterRect {
                    // Live (request 1503), in every direction: the width box
                    // 14 px below the bottom edge; the height as blue text
                    // 16 px left of the left edge, then its box centred 40 px
                    // screen-left of its edge (outside the left edge, or
                    // inside the right one).
                    let left = ids.curves.iter().copied().take(4).min_by(|x, y| mid(*x).x.total_cmp(&mid(*y).x));
                    let width = BoxPlace::Edge { line: horizontal, u: DVec2::NEG_Y, gap: 14.0 };
                    fields[0].place = Some(width);
                    fields[0].place_active = Some(width);
                    if let Some(f) = fields.get_mut(1) {
                        f.place = left.map(|l| BoxPlace::Edge { line: l, u: DVec2::NEG_X, gap: 16.0 });
                        f.place_active = Some(BoxPlace::Edge { line: vertical, u: DVec2::NEG_X, gap: 14.0 });
                    }
                }
                if self.tool == Tool::CornerRect {
                    let centre = (first.pos + click.pos) * 0.5;
                    let out_h = DVec2::new(0.0, (mid(horizontal).y - centre.y).signum());
                    let out_v = DVec2::new((mid(vertical).x - centre.x).signum(), 0.0);
                    let other = ids.curves.iter().copied().find(|c| *c != vertical && *c != horizontal && (mid(*c).x - centre.x).abs() > 1e-12 && (mid(*c).y - centre.y).abs() < 1e-12);
                    let width = BoxPlace::Edge { line: horizontal, u: out_h, gap: 14.0 };
                    fields[0].place = Some(width);
                    fields[0].place_active = Some(width);
                    // Live (behaviours "skasm (2026-10-28) dimension labels vs
                    // glyph rows"): the width's dimension outside its edge,
                    // past the edge's glyph box (42 px) when that is on the same
                    // side (below), else 26 px; the height's on the other
                    // vertical edge, 41 px inside, where its box was.
                    let off = if out_h.y < 0.0 { 42.0 } else { 26.0 };
                    fields[0].label = mid(horizontal) + out_h * w * off;
                    fields[0].label_at = Some((horizontal, out_h, off));
                    if let Some(f) = fields.get_mut(1) {
                        f.place = Some(BoxPlace::Edge { line: vertical, u: out_v, gap: 16.0 });
                        f.place_active = other.map(|o| BoxPlace::Edge { line: o, u: out_v, gap: 14.0 });
                        if let Some(o) = other {
                            f.kind = ConstraintKind::Length { curve: o, value: 0.0 };
                            f.label = mid(o) + out_v * w * 41.0;
                            f.label_at = Some((o, out_v, 41.0));
                        }
                    }
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
                let away = (c.pos - click.pos).try_normalize();
                self.set_otf(data, vec![diameter_field(&data.sketch, circle, self.world_per_px, away)]);
                EditOutcome::changed()
            }
            Tool::ThreePointCircle => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = std::mem::take(&mut self.clicks);
                // Live: a construction point at each click, on the circle.
                match sk.three_point_circle_points(c[0].pos, c[1].pos, c[2].pos) {
                    Ok(ids) => {
                        let id = ids.curves[0];
                        for (q, cl) in ids.points.iter().zip(&c) {
                            attach(sk, *q, None, cl);
                        }
                        self.finish_create(data, &[id]);
                        self.set_otf(data, vec![diameter_field(&data.sketch, id, self.world_per_px, data.sketch.entity(id).and_then(|e| e.center()).and_then(|cp| data.sketch.point(cp)).and_then(|ctr| (ctr - c[0].pos).try_normalize()))]);
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
                let semicircle = self.arc3_semicircle(c[0].pos, c[1].pos, c[2].pos);
                match sk.three_point_arc(c[0].pos, semicircle.unwrap_or(c[2].pos), c[1].pos) {
                    Ok(arc) => {
                        if let Some((s, e)) = sk.endpoints(arc) {
                            let near = |q: DVec2, p: DVec2| (q - p).length();
                            let ps = sk.point(s).unwrap_or_default();
                            let (cs, ce) = if near(c[0].pos, ps) < near(c[1].pos, ps) { (&c[0], &c[1]) } else { (&c[1], &c[0]) };
                            attach(sk, s, Some(arc), cs);
                            attach(sk, e, Some(arc), ce);
                        }
                        // Live (live2 arcmid): snapped to the semicircle, the centre is held at the ends' midpoint.
                        if semicircle.is_some()
                            && let Some(noshape_sketch::Geometry::Arc { center, start, end }) = sk.entity(arc).map(|e| e.geometry.clone())
                        {
                            let _ = sk.add_constraint(ConstraintKind::MidpointOf { point: center, a: start, b: end });
                        }
                        rim_autos(sk, arc, &c[2]);
                        self.finish_create(data, &[arc]);
                        // Live: the box centred on the start-end chord.
                        let chord = data.sketch.endpoints(arc).and_then(|(a, b)| Some((data.sketch.point(a)? + data.sketch.point(b)?) * 0.5));
                        self.set_otf(data, vec![radius_field(&data.sketch, arc, self.world_per_px, chord.map(BoxPlace::At))]);
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
                // Live: the box opposite the start click, as a circle's.
                let away = (c[0].pos - c[1].pos).try_normalize().map(BoxPlace::Rim);
                self.set_otf(data, vec![radius_field(&data.sketch, id, self.world_per_px, away)]);
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
                        // The Tangent arc tool stays but starts nothing: the next arc needs a start click on an end (live
                        // sketch-tools-1119). A line chain's tangent arc (Shift+A) goes on from its end.
                        if self.tool == Tool::TangentArc {
                            self.chain_end = None;
                            self.clicks.clear();
                        } else {
                            self.chain_end = end;
                            self.clicks = vec![click];
                        }
                        self.finish_create(data, &ids.curves);
                        self.set_otf(data, vec![radius_field(&data.sketch, arc, self.world_per_px, tangent_arc_box(&data.sketch, arc, self.world_per_px))]);
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
                            let s = attach(sk, s, Some(id), &c[0]);
                            let e = attach(sk, e, Some(id), &c[1]);
                            // A level or plumb rubber band to the second end makes the ends horizontal / vertical (live
                            // sketch-tool-conic), not the conic.
                            for a in c[1].inference.iter().flat_map(|i| i.constraints.iter()) {
                                let kind = match a {
                                    AutoConstraint::HorizontalLine => ConstraintKind::Horizontal { a: s, b: Some(e) },
                                    AutoConstraint::VerticalLine => ConstraintKind::Vertical { a: s, b: Some(e) },
                                    _ => continue,
                                };
                                let _ = sk.add_constraint(kind);
                            }
                        }
                        self.finish_create(data, &[id]);
                        // The rho box opens right away (Onshape).
                        let label = curve_mid(&data.sketch, id).unwrap_or_default() + DVec2::new(0.0, self.world_per_px * 26.0);
                        // Rho is an intrinsic dimension: accepting or dismissing
                        // the initial box keeps the default driving value.
                        let rho = self.conic_rho.clamp(0.01, 0.99);
                        add_dimension(data, ConstraintKind::Rho { conic: id, value: rho }, Some(label), None);
                        self.set_otf(data, vec![OtfField { kind: ConstraintKind::Rho { conic: id, value: 0.0 }, scale: 1.0, label, place: None, place_active: None, label_at: None }]);
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
                    self.polygon_readout = None;
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
                    // Live shows no prompt for it (ui-parity sketch-polygon).
                    self.message = None;
                    return EditOutcome::redraw();
                }
                // Any click commits (live: it doesn't start the next polygon).
                let c = self.clicks.remove(0);
                let rim = self.clicks.remove(0);
                let sides = self.polygon_sides.clamp(3, 50);
                self.polygon_typed.clear();
                match sk.polygon(c.pos, rim.pos, sides, self.polygon_on_vertex()) {
                    Ok(ids) => {
                        if let Some(&cp) = ids.points.last() {
                            attach(sk, cp, None, &c);
                        }
                        // The click's inferences (live: Horizontal with the centre) go to its point.
                        attach(sk, ids.points[0], None, &rim);
                        // Live makes no dimension (its boxed Ø is a readout).
                        self.polygon_readout = Some((c.pos, rim.pos));
                        self.finish_create(data, &ids.curves);
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
                        // Live (behaviours "spec: skasm sketch tools"): a
                        // horizontal major-axis click makes the ellipse
                        // Horizontal (its major axis).
                        for a in c[1].inference.iter().flat_map(|i| &i.constraints) {
                            if matches!(a, AutoConstraint::HorizontalLine | AutoConstraint::VerticalLine)
                                && let Some(k) = a.to_constraint(id, Some(id))
                            {
                                let _ = sk.add_constraint(k);
                            }
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
                self.text_clicks = vec![first, click];
                self.message = None;
                EditOutcome::redraw()
            }
            _ => EditOutcome::none(),
        }
    }

    /// A 3 point arc's third point snapped onto the semicircle over its ends (live2 arcmid: within 7.5 px of it, inside
    /// or out, anywhere along it; the arc then gets a MIDPOINT: its centre at the ends' midpoint).
    fn arc3_semicircle(&self, a: DVec2, b: DVec2, p: DVec2) -> Option<DVec2> {
        let m = (a + b) * 0.5;
        let r = (b - a).length() * 0.5;
        let d = p - m;
        (r > 1e-12 && ((d.length() - r).abs() <= ARC3_SEMICIRCLE_PX * self.world_per_px)).then(|| m + d.try_normalize().unwrap_or(DVec2::Y) * r)
    }

    /// One-line measurement of the sketch selection for the status area at
    /// the bottom right (live Onshape: `Length: 1.003 in`, `Diameter: 0.401
    /// in`, `Angle: 35.538 deg`, `Parallel dist: …`, `Point: X … Y … Z …`).
    /// Returns (label, value); `None` for an empty or mixed selection.
    /// The icon live puts before a lone circle's or arc's readout (its tool's: psui/sketch-readouts).
    pub fn selection_readout_icon(&self, data: &SketchData) -> Option<&'static str> {
        let picks = self.readout_entities();
        let [e] = picks.as_slice() else { return None };
        match data.sketch.curve(*e)? {
            Curve2::Circle { .. } => Some("circle"),
            Curve2::Arc { .. } => Some("center-point-arc"),
            _ => None,
        }
    }

    /// What the readout measures: the Dimension tool's picks while it has any (live cov2-dim-orient: "Point" after the
    /// first, "Min dist" after the second, kept while the dimension is placed), else the selection.
    pub fn readout_entities(&self) -> Vec<EntityId> {
        if self.tool == Tool::Dimension && !self.picks.is_empty() { self.tool_picks() } else { self.selection.clone() }
    }

    pub fn selection_readout(&self, data: &SketchData, units: &UnitPrefs) -> Vec<(String, String)> {
        let sk = &data.sketch;
        // Live's readouts keep their precision ("30.000 mm", sketch-mirror-model-edge).
        let len = |v: f64| format!("{} {}", noshape_units::to_fixed(v / units.si_factor(Dim::LENGTH), units.length_precision), units.length.symbol());
        let one = |label: &str, text: String| vec![(label.to_string(), text)];
        let sel: Vec<EntityId> = self.readout_entities().into_iter().filter(|e| sk.entity(*e).is_some()).collect();
        match sel.as_slice() {
            [e] => {
                if let Some(p) = sk.point(*e) {
                    let w = self.frame.to_world(p);
                    return one("Point", format!("X {}  Y {}  Z {}", len(w.x), len(w.y), len(w.z)));
                }
                match sk.curve(*e) {
                    Some(Curve2::Line { a, b }) => one("Length", len((b - a).length())),
                    Some(Curve2::Circle { radius, .. }) => one("Diameter", len(radius * 2.0)),
                    Some(Curve2::Arc { radius, .. }) => one("Radius", len(radius)),
                    Some(c) => one("Length", len(c.length())),
                    None => Vec::new(),
                }
            }
            [a, b] => {
                // Live psui/sketch-readouts (11-17): the labels' order follows the kinds, never the pick order; arcs
                // count as their arc for Min and Max.
                let kind = |e: EntityId| match (sk.point(e), sk.curve(e)) {
                    (Some(p), _) => Some(Pick::Point(p)),
                    (_, Some(Curve2::Line { a, b })) => Some(Pick::Segment(a, b)),
                    (_, Some(Curve2::Circle { center, radius })) => Some(Pick::Round(Round { center, radius, start: 0.0, sweep: TAU })),
                    (_, Some(Curve2::Arc { center, radius, start_angle, end_angle })) => {
                        Some(Pick::Round(Round { center, radius, start: start_angle, sweep: noshape_sketch::geom::ccw_sweep(start_angle, end_angle) }))
                    }
                    _ => None,
                };
                let (Some(ka), Some(kb)) = (kind(*a), kind(*b)) else { return Vec::new() };
                let min_max_center = |(min, max): (f64, f64), center: f64| vec![("Min dist".into(), len(min)), ("Max dist".into(), len(max)), ("Center dist".into(), len(center))];
                match (ka, kb) {
                    (Pick::Point(p), Pick::Point(q)) => one("Min dist", len((q - p).length())),
                    (Pick::Point(p), Pick::Segment(l0, l1)) | (Pick::Segment(l0, l1), Pick::Point(p)) => one("Min dist", len(segment_distance(p, l0, l1))),
                    (Pick::Point(p), Pick::Round(r)) | (Pick::Round(r), Pick::Point(p)) => min_max_center(r.extremes_from(p), (p - r.center).length()),
                    (Pick::Segment(l0, l1), Pick::Round(r)) | (Pick::Round(r), Pick::Segment(l0, l1)) => {
                        let Some(d) = (l1 - l0).try_normalize() else { return Vec::new() };
                        min_max_center(r.extremes_from_segment(l0, l1), (r.center - l0).perp_dot(d).abs())
                    }
                    (Pick::Round(r), Pick::Round(s)) => {
                        let (min, max) = r.extremes_from_round(&s);
                        vec![("Center dist".into(), len((s.center - r.center).length())), ("Max dist".into(), len(max)), ("Min dist".into(), len(min))]
                    }
                    (Pick::Segment(a0, a1), Pick::Segment(b0, b1)) => {
                        let (Some(u), Some(v)) = ((a1 - a0).try_normalize(), (b1 - b0).try_normalize()) else { return Vec::new() };
                        if u.perp_dot(v).abs() < 1e-9 {
                            one("Parallel dist", len((b0 - a0).perp_dot(u).abs()))
                        } else {
                            let ang = u.dot(v).clamp(-1.0, 1.0).acos();
                            let ang = ang.min(PI - ang);
                            one("Angle", format!("{} deg", noshape_units::to_fixed(ang.to_degrees(), units.angle_precision)))
                        }
                    }
                }
            }
            _ => Vec::new(),
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
                // Live (sketch-text-font-typed-ok: COINCIDENT, HORIZONTAL): a corner placed on a point is Coincident with it.
                let corners: Vec<EntityId> = data.texts.iter().find(|t| t.id == id).map(|t| t.box_points.iter().map(|p| EntityId(*p)).collect()).unwrap_or_default();
                for c in std::mem::take(&mut self.text_clicks) {
                    let Some(pt) = corners.iter().copied().find(|p| data.sketch.point(*p).is_some_and(|q| q.distance(c.pos) < 1e-9)) else { continue };
                    for a in c.inference.iter().flat_map(|i| &i.constraints) {
                        if let AutoConstraint::CoincidentPoint(q) = *a
                            && q != pt
                        {
                            let _ = data.sketch.add_constraint(ConstraintKind::Coincident { a: pt, b: q });
                        }
                    }
                }
                self.set_tool(Tool::Select);
                // Live (sketch-text-font-typed-ok): nothing is selected after ✓.
                self.selection.clear();
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

    /// Confirm spline / Confirm bezier (context menu): the curve through the
    /// clicked points, then the tool turns off (live).
    pub fn confirm_spline(&mut self, data: &mut SketchData) -> EditOutcome {
        if !matches!(self.tool, Tool::Spline | Tool::Bezier) || self.clicks.len() < 2 {
            return EditOutcome::none();
        }
        self.otf = None;
        self.otf_typing = false;
        let out = self.finish_spline(data, false);
        self.set_tool(Tool::Select);
        out
    }

    // ------------------------------------------------------------ on-the-fly dimensions

    fn set_otf(&mut self, data: &SketchData, fields: Vec<OtfField>) {
        let fields: Vec<OtfField> = fields.into_iter().filter(|f| data.sketch.measure_new(&f.kind).is_some()).collect();
        self.otf = (!fields.is_empty()).then_some(OnTheFly { fields, active: 0, committed: false });
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
                Some((self.otf_label(data, f, units, i == o.active), noshape_units::format_quantity(Quantity::new(v, dim), units), i == o.active))
            })
            .collect()
    }

    /// The active box is a Line tool's length: it lies in the sketch plane, foreshortened with it in a 3D view (live
    /// workflow 05, qa SKETCH-LENGTH-LABEL-IN-PLANE; other tools' boxes not seen in a 3D view yet).
    pub fn otf_active_in_plane(&self, data: &SketchData) -> bool {
        let Some(f) = self.otf.as_ref().and_then(|o| o.fields.get(o.active)) else { return false };
        matches!(self.tool, Tool::Line | Tool::MidpointLine)
            && matches!(f.kind, ConstraintKind::Length { curve, .. } if matches!(data.sketch.curve(curve), Some(Curve2::Line { .. })))
    }

    /// Where a value sits (sketch coordinates of its centre): its label, or
    /// its [`BoxPlace`]. The app draws the active box 24 px high and its
    /// width from the text, an idle value as ~18 px high text.
    fn otf_label(&self, data: &SketchData, f: &OtfField, units: &UnitPrefs, active: bool) -> DVec2 {
        let Some(place) = (if active { f.place_active } else { f.place }) else { return f.label };
        let text = data.sketch.measure_new(&f.kind).map(|v| dim_value_text(&f.kind, v, units)).unwrap_or_default();
        let (hw, hh) = if active { ((label_width_px(&text) * 14.0 / 16.0 + 12.0) * 0.5, 12.0) } else { (label_width_px(&text) * 13.0 / 28.0, 9.0) };
        let extent = |u: DVec2| u.x.abs() * hw + u.y.abs() * hh;
        let wpp = self.world_per_px;
        let curve = match f.kind {
            ConstraintKind::Diameter { curve, .. } | ConstraintKind::Radius { curve, .. } | ConstraintKind::Length { curve, .. } => data.sketch.curve(curve),
            _ => None,
        };
        match place {
            BoxPlace::Rim(u) => match curve {
                Some(Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. }) => center + u * (radius + (14.0 + extent(u)) * wpp),
                _ => f.label,
            },
            BoxPlace::Edge { line, u, gap } => match data.sketch.curve(line) {
                Some(Curve2::Line { a, b }) => (a + b) * 0.5 + u * (gap + extent(u)) * wpp,
                _ => f.label,
            },
            BoxPlace::At(p) => p,
        }
    }

    /// Text of the idle boxes, like a dimension label (live Onshape shows a
    /// bare `1.269`, without units, until you type into the box).
    pub fn otf_labels(&self, data: &SketchData, units: &UnitPrefs) -> Vec<String> {
        let Some(o) = &self.otf else { return Vec::new() };
        o.fields.iter().filter_map(|f| Some(dim_value_text(&f.kind, data.sketch.measure_new(&f.kind)?, units))).collect()
    }

    /// Switch the active on-the-fly box (Alt+arrows / Tab).
    /// The unit kind of the active on-the-fly box's value.
    pub fn otf_active_dim(&self) -> Option<Dim> {
        let o = self.otf.as_ref()?;
        o.fields.get(o.active).map(|f| dim_unit(&f.kind))
    }

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
        // Kept as typed, a plain value too (live: the box's "20" is the dimension's "20 mm", as a dimension's own box).
        self.commit_otf_value(data, o, f, q.value, Some(text.as_str()))
    }

    /// Enter while a tool shows an on-the-fly box (live Onshape: Enter in a
    /// Line chain turns the previous segment's boxed length into a placed
    /// driving dimension): dimension the active box at its current value.
    /// Returns false when no box is showing.
    pub fn accept_otf(&mut self, data: &mut SketchData) -> Result<bool, String> {
        let Some(o) = self.otf.clone() else { return Ok(false) };
        if o.committed {
            return Ok(true);
        }
        let Some(f) = o.fields.get(o.active).cloned() else { return Ok(false) };
        let Some(v) = data.sketch.measure_new(&f.kind) else { return Ok(false) };
        let value = v * f.scale;
        self.commit_otf_value(data, o, f, value, None).map(|_| true)
    }

    fn commit_otf_value(&mut self, data: &mut SketchData, o: OnTheFly, f: OtfField, value: f64, expr: Option<&str>) -> Result<(), String> {
        let q = Quantity::new(value, dim_unit(&f.kind));
        if !(q.value.is_finite() && q.value > 0.0) {
            return Err(DIM_NOT_POSITIVE.into());
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
            // (A free line's typed length moves its start, never scales: live 2022.)
            match data.sketch.free_line_edit_options(cid) {
                Some(opts) => data.sketch.solve_with(&opts),
                None => data.sketch.solve(),
            };
        } else {
            self.message = Some("The dimension over-defines the sketch; it was added as a driven dimension".into());
        }
        // The label off its line as the line now is.
        if let Some((line, u, px)) = f.label_at
            && let Some(Curve2::Line { a, b }) = data.sketch.curve(line)
        {
            let l = (a + b) * 0.5 + u * px * self.world_per_px;
            data.dimension_labels.insert(cid.0, [l.x, l.y]);
        }
        // Next box (rectangle width after length, ellipse minor after major).
        let mut o = o;
        o.fields.remove(o.active);
        o.active = o.active.min(o.fields.len().saturating_sub(1));
        o.committed = true;
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
                if self.below_drag_start(p - start) {
                    return EditOutcome::none();
                }
                // (A spline's handle arm drags its handle: the tip follows, the spline's end stays.)
                let tip = data.sketch.entity(entity).filter(|e| data.sketch.is_spline_arm(e.id)).and_then(|e| e.endpoints()).map(|(_, tip)| tip);
                let handle = match tip {
                    Some(t) => DragHandle::Point(t),
                    None if is_point(&data.sketch, entity) => DragHandle::Point(entity),
                    None => DragHandle::Entity { id: entity, grab: start },
                };
                // The DOF analysis once, before the drag: dragging keeps it
                // current, so every step's regeneration reuses it instead of
                // redoing it (qa SKETCH-DRAG-BIG: 30–50 ms per step on 1000
                // entities).
                self.drag_origin = data.sketch.begin_drag(&[(handle, p)]).then(|| data.sketch.clone());
                let sk = &data.sketch;
                self.drag_held = match handle {
                    DragHandle::Point(id) => sk.point(id).map_or(Vec::new(), |q| sk.entities().filter(|e| e.id != id && sk.curve(e.id).is_some_and(|c| c.distance(q) < 1e-9)).map(|e| e.id).collect()),
                    DragHandle::Entity { .. } => Vec::new(),
                };
                // A drag aligns only with points it snaps onto itself.
                self.woken.clear();
                self.woken_mids.clear();
                self.drag = Some(DragState::Geometry { handle, at: None });
                self.tool_drag(data, p)
            }
            Some(DragState::Geometry { handle, at }) => {
                if at == Some(p) {
                    return EditOutcome::none();
                }
                // Live: a drag's result depends only on where it ends (drag
                // probes 12: four paths to one pixel, the same points), so
                // each step solves from the drag's start (a jump too far for
                // one solve goes there in steps, `Sketch::drag`); in a big
                // component from the last step (`Sketch::begin_drag`).
                if let Some(o) = &self.drag_origin {
                    data.sketch.restore_geometry(o);
                }
                data.sketch.drag(&[(handle, p)]);
                match handle {
                    // From the last step, a dragged curve moves on from there.
                    DragHandle::Entity { id, .. } if self.drag_origin.is_none() => self.drag = Some(DragState::Geometry { handle: DragHandle::Entity { id, grab: p }, at: Some(p) }),
                    DragHandle::Entity { .. } => self.drag = Some(DragState::Geometry { handle, at: Some(p) }),
                    DragHandle::Point(id) => {
                        self.drag = Some(DragState::Geometry { handle, at: Some(p) });
                        // Shift suppresses snapping onto other geometry.
                        self.drag_snap = if self.shift_held { None } else { self.point_drop_target(&data.sketch, id) };
                        // A point the drag snaps onto wakes: the drag then aligns with it (live feel retaken 11-18).
                        if let Some((t, _, Drop::Merge)) = self.drag_snap
                            && is_point(&data.sketch, t)
                        {
                            self.wake(t);
                        }
                    }
                    #[allow(unreachable_patterns)]
                    _ => {}
                }
                EditOutcome::changed()
            }
            Some(DragState::Label { constraint, start, additive, dragging, .. }) => {
                if !dragging && self.below_drag_start(p - start) {
                    return EditOutcome::none();
                }
                self.drag = Some(DragState::Label { constraint, start, moved: true, additive, dragging: true });
                data.dimension_labels.insert(constraint.0, [p.x, p.y]);
                EditOutcome::changed()
            }
            Some(DragState::Box { .. }) => {
                self.box_end = Some(p);
                EditOutcome::redraw()
            }
            Some(DragState::OffsetArrow { start, moved, from }) => {
                if !moved && self.below_drag_start(p - start) {
                    return EditOutcome::none();
                }
                self.drag = Some(DragState::OffsetArrow { start, moved: true, from });
                // The offset at the press moved by the drag along the normal, on the manipulators' steps (live 11-18
                // sketch-tools: 5 mm and a 27 px = 5.08 mm drag gave 10.1 mm).
                if let Some((_, n)) = self.offset_frame(&data.sketch) {
                    let step = drag_step(self.world_per_px);
                    let d = ((from + (p - start).dot(n)) / step).round() * step;
                    if d.abs() > self.world_per_px {
                        self.offset_distance = d.abs();
                        self.offset_flip = ((d < 0.0) != self.offset_flip_base(&data.sketch)) != self.offset_out;
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
                    // A radius the corner can't take keeps the last one that
                    // solved, so the preview stays and drags back; the radius
                    // only becomes the tab's default once committed
                    // (SKETCH-FILLET-STUCK-RADIUS: an over-drag made every
                    // later fillet "radius too large").
                    let before = data.sketch.clone();
                    // The legs the arc trims: a radius the corner can't take
                    // flips them (it still "solves"); each keeps 2 px, from
                    // which the fillet can shrink again.
                    let legs = fillet_legs(&data.sketch, arc);
                    let _ = data.sketch.set_dimension_value(dim, r);
                    data.dimension_exprs.remove(&dim.0);
                    let solved = data.sketch.solve().is_solved();
                    // A leg used up to its end is fine (live types a fillet
                    // of a whole leg smaller again).
                    let kept = legs.iter().all(|(e, d0)| matches!(data.sketch.curve(*e), Some(Curve2::Line { a, b }) if (b - a).dot(*d0) >= -1e-12 * d0.length_squared()));
                    if !solved || !kept {
                        data.sketch = before;
                        // Live: the preview turns red at that radius (only a
                        // corner clicked on its vertex; a committed fillet or
                        // two picked curves keep the last good one).
                        if let Some(pid) = self.pending_corner_pid.filter(|_| pending) {
                            self.pending_corner = None;
                            self.pending_corner_pid = None;
                            self.invalid_corner = Some(pid);
                            self.fillet_radius = r;
                            self.drag = Some(DragState::FilletInvalid);
                        }
                    } else if pending {
                        // The default is the last radius shown (live).
                        self.fillet_radius = r;
                    }
                }
                if pending { EditOutcome::redraw() } else { EditOutcome::changed() }
            }
            Some(DragState::FilletInvalid) => {
                let Some(pid) = self.invalid_corner else { return EditOutcome::none() };
                let Some(a) = corner_arc(&data.sketch, pid, self.fillet_radius) else { return EditOutcome::none() };
                // The arc's middle follows the pointer along the bisector:
                // at radius r it is r (1 / sin(θ/2) − 1) from the vertex.
                let k = 1.0 / (a.theta * 0.5).sin() - 1.0;
                let r = ((p - a.vertex).dot(a.bisector) / k.max(1e-9)).max(self.world_per_px);
                self.fillet_radius = r;
                if let Some(made) = Self::try_fillet_corner(data, pid, r)
                    && let Some(arc) = made.1.created.iter().copied().find(|e| made.0.sketch.kind(*e) == Some(EntityKind::Arc))
                {
                    self.pending_corner = Some(made);
                    self.pending_corner_pid = Some(pid);
                    self.invalid_corner = None;
                    self.drag = Some(DragState::FilletRadius { arc });
                }
                EditOutcome::redraw()
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
                if !moved && self.below_drag_start(p - start) {
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
                if !moved && self.below_drag_start(p - start) {
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
    fn point_drop_target(&self, sk: &Sketch, id: EntityId) -> Option<(EntityId, DVec2, Drop)> {
        let pos = sk.point(id)?;
        // Live (behaviours "sketch snapping and hover", 11-12): a dragged point lands on a line from 7 px, not 9; the
        // reach of a drawn line's end (points and midpoints as there).
        let r = self.world_per_px * INFERENCE_PX;
        let (r_point, r_mid) = (r * noshape_sketch::inference::POINT_REACH, r * noshape_sketch::inference::MIDPOINT_REACH);
        // (On screen, as live measures it: in an iso view a pixel across the plane's foreshortened axis is longer in it.)
        let metric = self.screen_metric();
        let seen = |v: DVec2| metric.as_ref().map_or(v.length(), |m| m.len(v));
        // A point (or a midpoint) is live's 15 px square on screen (live/feel/sketch-snaps.json), its distance the
        // larger screen coordinate; without a view, a circle.
        let in_point = |v: DVec2, headless: f64| match &metric {
            Some(m) => (m.box_len(v) < r).then(|| m.box_len(v)),
            None => (v.length() <= headless).then(|| v.length()),
        };
        let users: Vec<EntityId> = sk.users_of_point(id);
        // Points of curves using the dragged point would collapse those curves.
        let siblings: Vec<EntityId> = users.iter().filter_map(|u| sk.entity(*u)).flat_map(|e| e.point_refs()).collect();
        let best_pt = sk
            .entities()
            .filter(|e| e.kind() == EntityKind::Point && e.id != id && !siblings.contains(&e.id))
            .filter_map(|e| sk.point(e.id).and_then(|q| Some((e.id, q, in_point(q - pos, r_point)?))))
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((e, q, _)) = best_pt {
            return Some((e, q, Drop::Merge));
        }
        // The sketch origin (a fixed point is created on drop).
        if in_point(pos, r_point).is_some() && !sk.entity(id).is_some_and(|e| e.external) {
            return Some((noshape_sketch::ORIGIN, DVec2::ZERO, Drop::Merge));
        }
        // Nor a curve the point was on when the drag began (live sk2-1121 polygon-corner-drag: an inscribed polygon's
        // side midpoint takes no MIDPOINT on its side or COINCIDENT on its circle on release).
        let curves = || sk.entities().filter(|e| e.kind() != EntityKind::Point && !users.contains(&e.id) && !e.point_refs().contains(&id) && !self.drag_held.contains(&e.id));
        // Live (audit 1969): by a line's midpoint the drop takes it (MIDPOINT).
        let mid = curves()
            .filter_map(|e| {
                let Some(Curve2::Line { a, b }) = sk.curve(e.id) else { return None };
                let q = (a + b) * 0.5;
                Some((e.id, q, in_point(q - pos, r_mid)?))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((e, q, _)) = mid {
            return Some((e, q, Drop::Midpoint));
        }
        curves()
            .filter_map(|e| {
                let c = sk.curve(e.id)?;
                let q = match (&c, &metric) {
                    (Curve2::Line { a, b }, Some(m)) => m.closest_on_segment(*a, *b, pos),
                    _ => c.closest(pos).1,
                };
                let d = seen(q - pos);
                (d <= r).then_some((e.id, q, d))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(e, q, _)| (e, q, Drop::OnCurve))
            .or_else(|| {
                // An arc's end level or plumb with its other end, within a line's reach (live2 sketch-drags b1x); a
                // point the drag snapped onto earlier (woken) too, never the dragged line's own ends (live feel retaken
                // 11-18).
                let arc_end = sk.entities().find_map(|e| match e.geometry {
                    noshape_sketch::Geometry::Arc { start, end, .. } if start == id => Some(end),
                    noshape_sketch::Geometry::Arc { start, end, .. } if end == id => Some(start),
                    _ => None,
                });
                let woken = self.woken.iter().copied().filter(|w| *w != id && !siblings.contains(w) && sk.point(*w).is_some());
                arc_end
                    .into_iter()
                    .chain(woken)
                    .filter_map(|other| {
                        let q = sk.point(other)?;
                        let (dy, dx) = (seen(DVec2::new(0.0, pos.y - q.y)), seen(DVec2::new(pos.x - q.x, 0.0)));
                        [(dy, DVec2::new(pos.x, q.y), true), (dx, DVec2::new(q.x, pos.y), false)].into_iter().filter(|(d, ..)| *d <= r).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(d, at, h)| (d, other, at, h))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, other, at, h)| (other, at, Drop::Level(h)))
            })
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
        if let Some(c) = self.transform_anchor(sk) {
            self.pattern_center = c;
        }
    }

    /// Where the 2D manipulator sits on the selection (live editing-gaps3, paste and Transform alike): its lowest key
    /// point (curve ends, centres, points), the leftmost of equals; the bounding box's centre if it has none.
    pub fn transform_anchor(&self, sk: &Sketch) -> Option<DVec2> {
        let mut keys = Vec::new();
        for e in self.selection.iter().filter_map(|e| sk.entity(*e)) {
            match (e.endpoints(), e.center()) {
                (None, None) => keys.push(e.id),
                (ends, centre) => keys.extend(ends.into_iter().flat_map(|(a, b)| [a, b]).chain(centre)),
            }
        }
        let tol = 1e-9;
        keys.into_iter()
            .filter_map(|k| sk.point(k))
            .reduce(|a, b| if b.y < a.y - tol || (b.y < a.y + tol && b.x < a.x) { b } else { a })
            .or_else(|| self.selection_center(sk))
    }

    /// The 2D manipulator's axes (turned with its rotation).
    fn xf_axes(&self) -> (DVec2, DVec2) {
        let u = DVec2::from_angle(self.xf_angle);
        (u, u.perp())
    }

    fn tool_up_inner(&mut self, data: &mut SketchData, p: DVec2) -> EditOutcome {
        // A fillet / chamfer press dragged from its corner and released makes it (a click only previews).
        if let Some(start) = self.corner_pressed.take()
            && self.drag.is_none()
            && self.pending_corner.is_some()
            && (p - start).length() > self.world_per_px * 4.0
        {
            self.press = None;
            return self.commit_corner(data);
        }
        // Live (feel round 2 R1): a click on the chain's last point ends the chain, the tool stays.
        if let Some(start) = self.chain_end_press.take()
            && self.tool == Tool::Line
            && (p - start).length() <= self.world_per_px * 4.0
        {
            self.clicks.clear();
            self.chain_end = None;
            return EditOutcome::redraw();
        }
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
            let inference = self.link_woken_mids(&mut data.sketch, inference);
            let out = self.place(data, Click { pos, inference }, false);
            // A dragged line is a single segment (no chaining).
            if tool == Tool::Line && !tangent {
                self.clicks.clear();
                self.chain_end = None;
            }
            return out;
        }
        self.press = None;
        self.drag_origin = None;
        self.drag_held.clear();
        if let Some(DragState::Handle(h @ (PatternHandle::Rotate | PatternHandle::Scale | PatternHandle::MoveX | PatternHandle::MoveY))) = &self.drag {
            self.xf_released = Some(*h);
            if *h == PatternHandle::Scale {
                self.xf_scale_box = self.cursor.map(|c| c + DVec2::new(XF_SCALE_BOX_FROM_POINTER.0, XF_SCALE_BOX_FROM_POINTER.1) * self.world_per_px);
            }
            if matches!(h, PatternHandle::MoveX | PatternHandle::MoveY) {
                self.xf_move_box = self.cursor.map(|c| (*h, c + DVec2::new(XF_MOVE_BOX_FROM_POINTER.0, XF_MOVE_BOX_FROM_POINTER.1) * self.world_per_px));
            }
        }
        let out = match self.drag.take() {
            Some(DragState::Pending { entity, .. }) => {
                // A curve or point clicked drops a selected constraint glyph (live2 glyph-select (b)).
                self.selected_constraints.retain(|c| data.sketch.constraint(*c).is_some_and(|k| k.kind.is_dimension()));
                if let Some(pos) = self.selection.iter().position(|e| *e == entity) {
                    self.selection.remove(pos);
                } else {
                    self.selection.push(entity);
                }
                EditOutcome::redraw()
            }
            Some(DragState::Geometry { handle, .. }) => {
                if let (DragHandle::Point(id), Some((target, at, drop))) = (handle, self.drag_snap.take()) {
                    let target = if target == noshape_sketch::ORIGIN { origin_point(&mut data.sketch) } else { target };
                    // The hidden Origin stand-in is constrained to, not merged into.
                    let is_pt = drop == Drop::Merge && !data.sketch.entity(target).is_some_and(|e| e.reference_only);
                    // Already coincident with it (a point held on the origin,
                    // dragged and released where it stays): nothing to add
                    // (DUP-COINCIDENT: every release added another one).
                    let already = data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Coincident { a, b } if (a == id && b == target) || (a == target && b == id)));
                    let ok = if already {
                        false
                    } else if is_pt {
                        // Keep the fixed/external point when merging.
                        let ext = data.sketch.entity(target).is_some_and(|e| e.external);
                        if ext || !data.sketch.entity(id).is_some_and(|e| e.external) { data.sketch.merge_points(id, target).is_ok() } else { data.sketch.merge_points(target, id).is_ok() }
                    } else if let Drop::Level(h) = drop {
                        let _ = data.sketch.set_point(id, at);
                        let kind = if h { ConstraintKind::Horizontal { a: id, b: Some(target) } } else { ConstraintKind::Vertical { a: id, b: Some(target) } };
                        !data.sketch.constraints().any(|c| c.kind == kind) && data.sketch.add_constraint(kind).is_ok()
                    } else if drop == Drop::Midpoint {
                        // The point lands on the midpoint; the line stays.
                        let _ = data.sketch.set_point(id, at);
                        data.sketch.add_constraint(ConstraintKind::Midpoint { point: id, curve: target }).is_ok()
                    } else {
                        data.sketch.add_constraint(ConstraintKind::Coincident { a: id, b: target }).is_ok()
                    };
                    if ok {
                        self.selection.retain(|e| data.sketch.entity(*e).is_some());
                        prune_exprs(data);
                    }
                }
                self.drag_snap = None;
                self.drag_external = None;
                data.sketch.solve();
                EditOutcome::changed()
            }
            Some(DragState::Box { start }) => {
                // The box is the screen rectangle the pointer drew (live: in
                // an iso view it isn't a rectangle on the plane).
                let (a, b) = (self.plane_to_screen(start), self.plane_to_screen(p));
                let (lo, hi) = (a.min(b), a.max(b));
                let px = if self.screen_map.is_some() { 1.0 } else { self.world_per_px };
                if (hi - lo).length() > px * 3.0 {
                    // Left to right: window (entirely inside); right to left: crossing.
                    let crossing = b.x < a.x;
                    // A curve's own points (its ends, a circle's centre) come with it, not as picks of their own (live
                    // sketch-mirror-preselected-batch: a line and a circle boxed, 2 selected); a lone point is one.
                    let owned: std::collections::HashSet<EntityId> = data.sketch.entities().filter(|e| e.kind() != EntityKind::Point).flat_map(|e| e.point_refs()).collect();
                    for e in data.sketch.entities() {
                        if owned.contains(&e.id) {
                            continue;
                        }
                        let Some(pts) = self.screen_outline(&data.sketch, e.id) else { continue };
                        let inside = pts.iter().all(|q| q.x >= lo.x && q.y >= lo.y && q.x <= hi.x && q.y <= hi.y);
                        let hit = inside || (crossing && crosses_box(&pts, lo, hi));
                        if hit && self.box_remove {
                            self.selection.retain(|x| *x != e.id);
                        } else if hit && !self.selection.contains(&e.id) {
                            self.selection.push(e.id);
                        }
                    }
                } else if let Some(r) = self.model_press.take() {
                    if let Some(i) = self.model_selection.iter().position(|x| *x == r) {
                        self.model_selection.remove(i);
                    } else {
                        self.model_selection.push(r);
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
                self.model_press = None;
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
            // Live2 glyph-select: one glyph at a time, a click (Shift too) on another replaces it and drops the selected
            // curves and points; Shift+click on the selected one deselects it.
            Some(DragState::Glyph { moved: true, .. }) => EditOutcome::changed(),
            Some(DragState::Glyph { constraint, additive, .. }) => {
                if additive && self.selected_constraints.contains(&constraint) {
                    self.selected_constraints.retain(|c| *c != constraint);
                } else {
                    self.selected_constraints = vec![constraint];
                    self.selection.clear();
                    self.selected_regions.clear();
                }
                EditOutcome::redraw()
            }
            Some(DragState::Handle(_)) | Some(DragState::TrimPath { .. }) => EditOutcome::redraw(),
            Some(DragState::FilletRadius { arc }) => {
                if self.pending_corner.is_some() {
                    EditOutcome::redraw()
                } else {
                    // A committed fillet dragged: its radius is the default now.
                    if let Some(Curve2::Arc { radius, .. }) = data.sketch.curve(arc) {
                        self.fillet_radius = radius;
                    }
                    EditOutcome::changed()
                }
            }
            Some(DragState::ChamferDistance) => EditOutcome::redraw(),
            // Released on a radius the corner can't take: nothing is made
            // (live's toast); the red preview stays to drag back.
            Some(DragState::FilletInvalid) => {
                self.toast = Some("The current fillet radius produces invalid geometry.".into());
                EditOutcome::redraw()
            }
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

    /// The previewed instances (each with whether it is drawn dashed).
    /// Live: a linear pattern previews at most 7 per direction; with more,
    /// the original, 5 copies and a dashed 7th that says more follow.
    fn pattern_transforms(&self) -> Vec<(PointMap, bool)> {
        let mut out: Vec<(PointMap, bool)> = Vec::new();
        let shown = |n: usize| -> Vec<(usize, bool)> {
            if n <= 7 { (0..n.max(1)).map(|k| (k, false)).collect() } else { (0..7).map(|k| (k, k == 6)).collect() }
        };
        match self.tool {
            Tool::LinearPattern => {
                let step = DVec2::from_angle(self.pattern_angle) * self.pattern_distance;
                let second = DVec2::from_angle(self.pattern_second_angle) * self.pattern_second_distance;
                for (j, dj) in shown(self.pattern_second_count) {
                    for (k, dk) in shown(self.pattern_count) {
                        if j == 0 && k == 0 { continue; }
                        let off = step * k as f64 + second * j as f64;
                        out.push((Box::new(move |p| p + off), dj || dk));
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
                        out.push((Box::new(move |p| t.apply(p)), false));
                    }
                }
            }
            Tool::Transform => {
                let (c, a, s, d) = (self.pattern_center, self.xf_angle, self.xf_scale, self.xf_offset);
                out.push((Box::new(move |p| c + DVec2::from_angle(a).rotate(p - c) * s + d), false));
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
                    self.xf_scale_box = None;
                    self.xf_move_box = None;
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
        self.pattern_label_wpp = self.world_per_px;
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
        self.message = Some("Adjust the pattern, then click in empty space".into());
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
                    if let Some((base, t)) = self.circular_arrow(sk) {
                        out.push((PatternHandle::CircularAngle, base + t * px * (CIRCULAR_ARROW_STEM_PX + OFFSET_ARROW_HEAD_PX * 0.5)));
                    }
                    out.push((PatternHandle::CircularCount, self.pattern_center + DVec2::from_angle(a0 + step) * (r - px * 18.0).max(r * 0.5)));
                }
            }
            Tool::Transform => {
                let m = self.pattern_center + self.xf_offset;
                let (u, v) = self.xf_axes();
                let arrow = px * (XF_ARROW_BASE_PX + XF_ARROW_HEAD_PX * 0.5);
                let sq = px * (XF_SQUARE_PX.0 + XF_SQUARE_PX.1) * 0.5;
                out.push((PatternHandle::Center, m));
                out.push((PatternHandle::Move, m + (u + v) * sq));
                out.push((PatternHandle::MoveX, m + u * arrow));
                out.push((PatternHandle::MoveY, m + v * arrow));
                out.push((PatternHandle::Rotate, m + u * px * XF_RING_AT_PX));
                out.push((PatternHandle::Scale, m + (u + v).normalize() * px * (XF_SCALE_BASE_PX + XF_SCALE_HEAD_PX * 0.5) * self.xf_scale));
            }
            _ => {}
        }
        out
    }

    /// One linear pattern direction (live sketch/136): its unit direction,
    /// step, count and the diagonal its label and spacing dimension go to
    /// (first direction down-right of +X, second up-left of +Y, turned with
    /// the direction).
    /// World units per pixel for the pattern labels' offsets: the zoom when the tool started.
    fn label_wpp(&self) -> f64 {
        if self.pattern_label_wpp > 0.0 { self.pattern_label_wpp } else { self.world_per_px }
    }

    /// A linear pattern direction's "3x" label centre (live sketch-gaps3).
    fn linear_count_label(&self, second: bool, sk: &Sketch) -> Option<DVec2> {
        let c = self.selection_center(sk)?;
        let (dir, step, _, diag) = self.linear_direction(second);
        let across = if diag.dot(dir.perp()) >= 0.0 { dir.perp() } else { -dir.perp() };
        Some(c + step + (dir * PATTERN_LABEL_ALONG_PX + across * PATTERN_DIM_PX) * self.label_wpp())
    }

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
        let side = if diag.dot(n) >= 0.0 { n } else { -n } * self.label_wpp() * PATTERN_DIM_PX;
        (c + side, c + step + side)
    }

    /// On-canvas instance-count labels of the active pattern (live Onshape:
    /// `3x` / `1x` next to each direction's count manipulator, click to
    /// type a count; sketch/136–138).
    pub fn pattern_count_labels(&self, sk: &Sketch) -> Vec<(PatternHandle, DVec2, usize)> {
        self.pattern_handles(sk)
            .into_iter()
            .filter_map(|(h, _)| match h {
                // The leader to it is drawn in the overlay.
                PatternHandle::LinearCount | PatternHandle::LinearSecondCount => {
                    let second = h == PatternHandle::LinearSecondCount;
                    Some((h, self.linear_count_label(second, sk)?, self.linear_direction(second).2))
                }
                PatternHandle::CircularCount => {
                    let (r, a0) = self.circular_arm(sk)?;
                    Some((h, self.pattern_center + DVec2::from_angle(a0) * r * (1.0 + CIRCULAR_LABEL_BEYOND), self.circular_count))
                }
                _ => None,
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
                    let at = DVec2::from_angle(a0 + self.circular_angle * CIRCULAR_ANGLE_TEXT_AT);
                    out.push((PatternHandle::CircularAngle, self.pattern_center + at * r * (1.0 + CIRCULAR_LABEL_BEYOND), -self.circular_angle));
                }
            }
            Tool::Transform if !self.selection.is_empty() => {
                let m = self.pattern_center + self.xf_offset;
                // Live editing-gaps4 (one take each): the boxes' centres, during a drag off the pointer, after it off
                // the anchor (rotate) or where the drag left it (scale).
                let dragging = |h: PatternHandle| matches!(self.drag, Some(DragState::Handle(d)) if d == h);
                let off = |o: (f64, f64)| DVec2::new(o.0, o.1) * px;
                if self.xf_angle.abs() > 1e-12 {
                    let at = match self.cursor.filter(|_| dragging(PatternHandle::Rotate)) {
                        Some(c) => c + off(XF_ROTATE_BOX_FROM_POINTER),
                        None => m + off(XF_ROTATE_BOX_AFTER),
                    };
                    out.push((PatternHandle::Rotate, at, self.xf_angle));
                }
                if (self.xf_scale - 1.0).abs() > 1e-12 {
                    let at = match self.cursor.filter(|_| dragging(PatternHandle::Scale)) {
                        Some(c) => c + off(XF_SCALE_BOX_FROM_POINTER),
                        None => self.xf_scale_box.unwrap_or(m + off(XF_SCALE_BOX_AFTER)),
                    };
                    out.push((PatternHandle::Scale, at, self.xf_scale));
                }
                for h in [PatternHandle::MoveX, PatternHandle::MoveY] {
                    let at = match self.cursor.filter(|_| dragging(h)) {
                        Some(c) => Some(c + off(XF_MOVE_BOX_FROM_POINTER)),
                        None => self.xf_move_box.filter(|b| b.0 == h).map(|b| b.1),
                    };
                    if let Some(at) = at {
                        out.push((h, at, self.xf_move_value(h)));
                    }
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
            // Typed (and the agent API's) angles read clockwise, as live shows them ("180°").
            PatternHandle::CircularAngle if v.abs() > 1e-9 => self.circular_angle = -v.clamp(-TAU, TAU),
            PatternHandle::Rotate => self.xf_angle = v,
            PatternHandle::Scale if v > 0.0 => self.xf_scale = v,
            PatternHandle::MoveX | PatternHandle::MoveY => {
                let a = self.xf_move_axis(h);
                self.xf_offset += a * (v - self.xf_offset.dot(a));
            }
            _ => {}
        }
    }

    /// The Transform arrow's direction (X or Y of the manipulator).
    fn xf_move_axis(&self, h: PatternHandle) -> DVec2 {
        let (u, v) = self.xf_axes();
        if h == PatternHandle::MoveX { u } else { v }
    }

    /// Where a Split click at `p` cuts curve `id`, and whether that is a line's midpoint: by a line's middle the split
    /// takes it, MIDPOINT of the line's ends (live sk2-1121 split-at-crossing, geo-list3 / geo-list6).
    fn split_point(&self, sk: &Sketch, id: EntityId, p: DVec2) -> (DVec2, bool) {
        let reach = self.world_per_px * INFERENCE_PX;
        let near = |v: DVec2| self.screen_metric().map_or(v.length() <= reach * noshape_sketch::inference::MIDPOINT_REACH, |m| m.box_len(v) < reach);
        match sk.curve(id) {
            Some(Curve2::Line { a, b }) if near((a + b) * 0.5 - p) => ((a + b) * 0.5, true),
            Some(c) => (c.closest(p).1, false),
            None => (p, false),
        }
    }

    /// How far the selection has moved along a Transform arrow.
    pub fn xf_move_value(&self, h: PatternHandle) -> f64 {
        self.xf_offset.dot(self.xf_move_axis(h))
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

    /// The way a circular pattern runs: -1 clockwise (live's), 1 counter-clockwise.
    fn circular_way(&self) -> f64 {
        if self.circular_angle < 0.0 { -1.0 } else { 1.0 }
    }

    /// How far round from the seed (at angle `a0`) point `p` is, the pattern's way.
    fn sweep_from_seed(&self, a0: f64, p: DVec2) -> f64 {
        let at = (p - self.pattern_center).to_angle();
        if self.circular_way() < 0.0 { noshape_sketch::geom::ccw_sweep(at, a0) } else { noshape_sketch::geom::ccw_sweep(a0, at) }
    }

    /// A circular pattern's handle arc: its radius and its start and end angles (the seed's and the last instance's).
    fn circular_handle_arc(&self, sk: &Sketch) -> Option<(f64, f64, f64)> {
        let (r, a0) = self.circular_arm(sk)?;
        let (step, _) = self.circular_step();
        Some((r * CIRCULAR_HANDLE_AT, a0, a0 + step * (self.circular_count.max(2) - 1) as f64))
    }

    /// The handle arc's arrow: its base (the arc's end) and the way it points (on along the arc).
    fn circular_arrow(&self, sk: &Sketch) -> Option<(DVec2, DVec2)> {
        let (rr, a0, a1) = self.circular_handle_arc(sk)?;
        let base = self.pattern_center + DVec2::from_angle(a1) * rr;
        Some((base, DVec2::from_angle(a1).perp() * (a1 - a0).signum()))
    }

    /// Angle between circular instances and whether the pattern is a full circle.
    fn circular_step(&self) -> (f64, bool) {
        let n = self.circular_count.max(2);
        let full = (self.circular_angle.abs() - TAU).abs() < 1e-9;
        (if full { self.circular_angle / n as f64 } else { self.circular_angle / (n - 1) as f64 }, full)
    }

    fn handle_at(&self, sk: &Sketch, p: DVec2) -> Option<PatternHandle> {
        // The 2D manipulator's handles are bigger than the pattern handles' 9 px.
        let reach = |h: PatternHandle| match h {
            PatternHandle::MoveX | PatternHandle::MoveY | PatternHandle::Scale => 15.0,
            PatternHandle::Move => 13.0,
            PatternHandle::Rotate | PatternHandle::Center if self.tool == Tool::Transform => 11.0,
            _ => 9.0,
        };
        // An arrow is grabbed along its shaft too (live editing-gaps3: 8 px from the anchor, 60 px long; a click on it
        // leaves the selection as it is, sketch-entity-copy-paste).
        let m = self.pattern_center + self.xf_offset;
        let shaft = |q: DVec2| {
            let d = (q - m).try_normalize().unwrap_or(DVec2::X);
            let (a, len) = (m + d * self.world_per_px * XF_ARROW_GAP_PX, self.world_per_px * (XF_ARROW_BASE_PX - XF_ARROW_GAP_PX));
            let along = (p - a).dot(d).clamp(0.0, len);
            (p - (a + d * along)).length() / XF_ARROW_SHAFT_REACH_PX
        };
        self.pattern_handles(sk)
            .into_iter()
            .map(|(h, q)| match h {
                PatternHandle::MoveX | PatternHandle::MoveY => (h, ((q - p).length() / reach(h)).min(shaft(q))),
                _ => (h, (q - p).length() / reach(h)),
            })
            .filter(|x| x.1 <= self.world_per_px)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
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
                // The 2D manipulator's ring moves the triad only, the geometry stays (live editing-gaps3).
                let shift = if self.tool == Tool::Transform { self.xf_offset } else { DVec2::ZERO };
                self.pattern_center = self.inference_free_snap(sk, p) - shift;
                self.xf_center_pinned = true;
            }
            // Live (sketch-gaps5): the arrow sweeps the pattern's own way in 5° steps; back onto the seed it closes the
            // pattern (it can't turn it the other way).
            PatternHandle::CircularAngle => {
                if let Some((_, a0)) = self.circular_arm(sk) {
                    let step = 5f64.to_radians();
                    let a = (self.sweep_from_seed(a0, p) / step).round() * step;
                    let a = if a < step * 0.5 || a > TAU - step * 0.5 { TAU } else { a };
                    self.circular_angle = a * self.circular_way();
                }
            }
            PatternHandle::CircularCount => {
                if let Some((_, a0)) = self.circular_arm(sk) {
                    let a = self.sweep_from_seed(a0, p).max(1f64.to_radians());
                    let (_, full) = self.circular_step();
                    let n = (self.circular_angle.abs() / a).round().max(1.0) as usize;
                    self.circular_count = if full { n } else { n + 1 }.clamp(2, 100);
                }
            }
            PatternHandle::Move => {
                let (u, v) = self.xf_axes();
                self.xf_offset = match self.xf_grab {
                    Some((at, from)) => from + (p - at),
                    None => p - (u + v) * px * (XF_SQUARE_PX.0 + XF_SQUARE_PX.1) * 0.5 - self.pattern_center,
                };
            }
            PatternHandle::MoveX | PatternHandle::MoveY => {
                let (u, v) = self.xf_axes();
                let a = if h == PatternHandle::MoveX { u } else { v };
                let along = match self.xf_grab {
                    Some((at, from)) => (from + (p - at)).dot(a),
                    None => (p - a * px * (XF_ARROW_BASE_PX + XF_ARROW_HEAD_PX * 0.5) - self.pattern_center).dot(a),
                };
                self.xf_offset += a * (along - self.xf_offset.dot(a));
            }
            PatternHandle::Rotate => {
                let d = p - (self.pattern_center + self.xf_offset);
                if d.length() > px * 3.0 {
                    let mut a = d.to_angle();
                    // Snap to whole 15 degree steps near them; else whole degrees (live editing-gaps4: "20 deg").
                    let step = 15f64.to_radians();
                    if (a / step - (a / step).round()).abs() * step < 2f64.to_radians() {
                        a = (a / step).round() * step;
                    }
                    self.xf_angle = a.to_degrees().round().to_radians();
                }
            }
            PatternHandle::Scale => {
                let d = (p - (self.pattern_center + self.xf_offset)).length();
                self.xf_scale = (d / (px * (XF_SCALE_BASE_PX + XF_SCALE_HEAD_PX * 0.5))).clamp(1e-5, 1000.0);
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
            // Live (behaviours "qa (2026-10-28) sketch.md 7.22"): two conics /
            // Beziers / splines that don't touch can't be dimensioned; the
            // second pick is refused with the warning bubble, the picks clear
            // and the tool stays on.
            if let [first] = ids.as_slice()
                && (unmeasurable_curve_pair(&data.sketch, *first, e) || ellipse_and_plane(data, *first, e))
            {
                self.picks.clear();
                self.toast = Some(DIMENSION_FAILED.into());
                return EditOutcome::redraw();
            }
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
        // Live (behaviours "qa (2026-10-28) sketch.md 7.22", SKETCH-DIM-CURVE-PAIR):
        // the yellow warning bubble; the picks clear, the tool stays on.
        let Some(kind) = smart_dimension(&data.sketch, &ids, p).map(|k| circle_mode(&data.sketch, k, &picks, self.world_per_px)) else {
            self.toast = Some(DIMENSION_FAILED.into());
            return EditOutcome::redraw();
        };
        let Some(cid) = add_dimension(data, kind, Some(p), None) else {
            self.toast = Some(DIMENSION_FAILED.into());
            return EditOutcome::redraw();
        };
        // Live ("qa4 round 5 s5 driven dims" (1)): a dimension that would
        // over-define the sketch is added driven and still opens its value
        // box ("10 mm" selected); no toast, nothing red.
        self.open_value_box(data, cid);
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
        // Over a second curve that can't be dimensioned to the first, the
        // first one's own dimension (a conic's Rho) doesn't follow the pointer.
        if let ([first], Some(h)) = (ids.as_slice(), self.hover)
            && unmeasurable_curve_pair(&data.sketch, *first, h)
        {
            return None;
        }
        let kind = circle_mode(&data.sketch, smart_dimension(&data.sketch, &ids, c)?, &self.picks, self.world_per_px);
        let v = data.sketch.measure_new(&kind)? * dim_display_scale(&kind);
        Some((kind, c, v))
    }

    /// Apply a typed dimension value (expression). Returns an error message.
    /// Negative values flip the geometry where Onshape allows it (distances,
    /// angles, line lengths); radii, diameters and arc lengths must be positive.
    pub fn apply_dimension(&mut self, data: &mut SketchData, cid: ConstraintId, text: &str, units: &UnitPrefs, vars: &dyn noshape_units::VariableScope) -> Result<(), String> {
        let Some(c) = data.sketch.constraint(cid).cloned() else { return Err("Dimension no longer exists".into()) };
        // A new dimension the Dimension tool added driven (it over-defined the sketch) takes a typed value as a driving
        // one: the sketch then can't be solved and shows so (live errors-1118: a redundant 40 or a conflicting 50 on a
        // rectangle's opposite side, accepted, the sketch red).
        if !c.driving && self.dim_edit.as_ref().is_some_and(|d| d.constraint == cid && d.created) {
            data.sketch.set_driving(cid, true).map_err(|e| e.to_string())?;
        }
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
            return Err(DIM_NOT_POSITIVE.into());
        }
        if value < 0.0 {
            if positive_only {
                return Err(DIM_NOT_NEGATIVE.into());
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
        // Live 1785 / 2022: a sketch's first dimension scales it onto the new value; else a free line's start moves.
        let scaled = data.sketch.scale_for_edit(cid);
        let opts = (!scaled).then(|| data.sketch.free_line_edit_options(cid)).flatten().unwrap_or_else(|| data.sketch.dimension_edit_options(cid));
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
            // Live ("qa4 round 5 s5 driven dims"): a driven dimension is the
            // plain value in grey ("Ø10"), no parentheses.
            let text = dimension_label_text(data.dimension_exprs.get(&c.id.0).map(|e| e.text.as_str()), value, self.show_expressions);
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
            dim_segments(sk, &c.kind, Some(&c.hints), pos, s, &mut segs);
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
                let anchor = match c.kind {
                    ConstraintKind::Flow { a, b } if a == e => flow_anchor(sk, a, b),
                    _ => glyph_anchor(sk, e),
                };
                let Some(anchor) = anchor else { continue };
                let offset = data.glyph_offsets.get(&glyph_key(c.id, e)).map(|o| DVec2::new(o[0], o[1]));
                // The Origin's glyphs are the usual grey on white (live).
                let external = c.kind.entities().iter().any(|x| sk.entity(*x).is_some_and(|x| x.external) && !is_origin(sk, *x));
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
        // Shared ends: a Coincident glyph at each (live: one per corner of a
        // rectangle, up-left of the point).
        for (p, curves) in shared_ends(sk) {
            let Some(anchor) = glyph_anchor(sk, p) else { continue };
            let cid = shared_glyph_id(p);
            let offset = data.glyph_offsets.get(&glyph_key(cid, p)).map(|o| DVec2::new(o[0], o[1]));
            let external = curves.iter().all(|c| sk.entity(*c).is_some_and(|x| x.external));
            out.push(GlyphInfo { constraint: cid, icon: "coincident", entity: p, anchor, on_point: true, offset, external });
        }
        out
    }

    /// `glyphs` for Show constraints off: a curve in `focus` (hovered, lingering, selected) shows every constraint it
    /// takes part in, also those drawn once on the other curve (live sketch-tool walk 3: a rectangle's top shows ⊥ ⫽ —,
    /// its right edge its own; qa FEEL-HOVER-GLYPHS-SECOND-ENTITY). Such a glyph is added on the focused curve, in the
    /// constraint's place in the list.
    pub fn focus_glyphs(&self, data: &SketchData, glyphs: Vec<GlyphInfo>, focus: &[EntityId]) -> Vec<GlyphInfo> {
        let sk = &data.sketch;
        let mut out: Vec<GlyphInfo> = Vec::with_capacity(glyphs.len());
        for g in &glyphs {
            out.push(g.clone());
            if g.on_point || focus.contains(&g.entity) {
                continue;
            }
            let Some(c) = sk.constraint(g.constraint) else { continue };
            for f in c.kind.entities().into_iter().filter(|f| focus.contains(f) && !is_point(sk, *f)) {
                if glyphs.iter().chain(out.iter()).any(|h| h.constraint == g.constraint && h.entity == f) {
                    continue;
                }
                let Some(anchor) = glyph_anchor(sk, f) else { continue };
                out.push(GlyphInfo { entity: f, anchor, on_point: false, offset: None, ..g.clone() });
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
        // Live (behaviours "qa (2026-10-26, P2 sketch) Copy/paste inside a
        // sketch"): the copy lands centred on the pointer, selected, with
        // the Transform tool on it, and nothing is said.
        let b = clip.bbox();
        let offset = match self.cursor {
            Some(c) if !b.is_empty() => c - b.center(),
            _ => DVec2::ZERO,
        };
        if !self.paste_entities(data, &clip, offset) {
            return false;
        }
        let pasted = std::mem::take(&mut self.selection);
        if self.tool != Tool::Transform {
            self.set_tool(Tool::Transform);
        }
        self.selection = pasted;
        // The Transform triad on the copy (qa: it stayed on the original).
        self.follow_transform_selection(&data.sketch);
        self.paste_follow = self.cursor.filter(|_| !b.is_empty());
        self.message = None;
        true
    }

    /// Moves a following paste by `d`, the pointer now at `p`.
    fn move_paste(&mut self, data: &mut SketchData, d: DVec2, p: DVec2) -> EditOutcome {
        self.cursor = Some(p);
        if d.length() < 1e-12 {
            return EditOutcome::none();
        }
        let ids: Vec<EntityId> = self.selection.iter().copied().filter(|e| data.sketch.entity(*e).is_some()).collect();
        if ids.is_empty() || data.sketch.transform(&ids, Transform::Translate(d)).is_err() {
            self.paste_follow = None;
            return EditOutcome::none();
        }
        data.sketch.solve();
        self.paste_follow = Some(p);
        self.follow_transform_selection(&data.sketch);
        EditOutcome::changed()
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
        true
    }

    pub fn delete_selection(&mut self, data: &mut SketchData) -> bool {
        let mut changed = false;
        for c in self.selected_constraints.drain(..) {
            if let Some(e) = use_glyph_entity(c) {
                changed |= unlink_used(data, e);
                continue;
            }
            if let Some(p) = shared_glyph_point(c) {
                changed |= split_shared(data, p);
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
            data.use_ends.retain(|k, _| data.external_refs.contains_key(k));
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
        if let Some(p) = shared_glyph_point(cid) {
            if !split_shared(data, p) {
                return false;
            }
            data.sketch.solve();
            return true;
        }
        // A circular pattern goes whole (live: one constraint, its links ours).
        let group = data.sketch.constraint_group(cid);
        if group.iter().filter(|c| data.sketch.remove_constraint(**c).is_ok()).count() == 0 {
            return false;
        }
        self.selected_constraints.retain(|c| !group.contains(c));
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
        let mut now = None;
        for id in &self.selection {
            // (A spline's handle arm stays construction.)
            if is_point(&data.sketch, *id) || data.sketch.entity(*id).is_some_and(|e| e.handle_of.is_some()) {
                continue;
            }
            let cur = data.sketch.entity(*id).is_some_and(|e| e.construction);
            let _ = data.sketch.set_construction(*id, !cur);
            now = Some(now.unwrap_or(true) && !cur);
        }
        // Live 1853 addendum: the next curve drawn is construction after
        // `q` turned a selection into construction.
        if let Some(c) = now {
            self.construction = c;
        }
        true
    }

    /// The Construction button's state (live 1853 addendum): construction
    /// mode, or a selection of construction curves only.
    pub fn construction_shown(&self, sk: &Sketch) -> bool {
        // (A handle line selected shows it too: live's ui-parity sketch-spline-handle-arm.)
        // (A model edge's reference line isn't: live sketch-mirror-model-edge, the edge as the mirror line, Mirror shown.)
        // (A circle's, arc's or ellipse's centre counts as construction, picked by a tool too: live sketch-dim-circle-centres;
        // a line's end doesn't: sketch-dim-hv-two-points.)
        if self.construction {
            return true;
        }
        let centre = |id: EntityId| is_point(sk, id) && sk.entities().any(|e| e.center() == Some(id));
        let mut shown = self.selection.iter().copied().chain(self.picks.iter().map(|p| p.0)).filter(|id| centre(*id) || (!is_point(sk, *id) && sk.entity(*id).is_some_and(|e| !e.external))).peekable();
        shown.peek().is_some() && shown.all(|id| centre(id) || sk.entity(id).is_some_and(|e| e.construction))
    }

    /// Apply a constraint tool to the current selection if it fits; a selection that more picks could complete
    /// starts the tool with it; one the constraint can't take is refused with live's words (live 2270: the
    /// selection cleared, no tool, the message in [`Self::refusal`]); nothing selected starts the tool.
    pub fn constraint_command(&mut self, data: &mut SketchData, c: ConstraintTool) -> bool {
        self.refusal = None;
        // Select-first: the constraint applies to the whole selection at once.
        if already_coincident(&data.sketch, c, &self.selection) {
            return false;
        }
        if self.selection.is_empty() {
            // Tool-first: each pair of entities picked is constrained.
            self.set_tool(Tool::Constraint(c));
            return false;
        }
        if let Some(why) = constraint_refusal(&data.sketch, c, &self.selection) {
            self.refuse(why);
            return false;
        }
        if constraint_applicable(&data.sketch, c, &self.selection) {
            return match apply_constraint(&mut data.sketch, c, &self.selection) {
                Ok(n) if n > 0 => {
                    if !solve_after_constraint(&mut data.sketch, c, &self.selection) {
                        self.toast = Some(SKETCH_NOT_SOLVED.into());
                    }
                    self.selection.clear();
                    true
                }
                Ok(_) | Err(_) => {
                    self.refuse(ADD_CONSTRAINT_FAILED);
                    false
                }
            };
        }
        if completes_later(&data.sketch, c, &self.selection) {
            // Live: Midpoint on two points, Symmetric on two lines: the tool starts with them as its picks.
            let picks: Vec<(EntityId, DVec2)> = self.selection.iter().map(|e| (*e, DVec2::ZERO)).collect();
            if self.tool != Tool::Constraint(c) {
                self.set_tool(Tool::Constraint(c));
            }
            self.picks = picks;
            return false;
        }
        self.refuse(refusal_words(&data.sketch, c, &self.selection));
        false
    }

    fn refuse(&mut self, why: &str) {
        self.refusal = Some(why.into());
        self.selection.clear();
        self.selected_constraints.clear();
        if matches!(self.tool, Tool::Constraint(_)) {
            self.set_tool(Tool::Select);
        }
    }

    // ------------------------------------------------------------ overlay

    /// Preview geometry, inference guides, picks, box selection and dimension
    /// The committed polygon's grey-boxed Ø readout (live: after Enter, at
    /// the drawing readout's place, until the next polygon starts).
    pub fn polygon_readout(&self, units: &UnitPrefs) -> Option<(DVec2, String)> {
        let (a, tip) = self.polygon_readout?;
        let e0 = EntityId(0);
        let r = (tip - a).length();
        let text = dim_value_text(&ConstraintKind::Diameter { curve: e0, value: r * 2.0 }, r * 2.0, units);
        Some((circle_readout_at(a, tip, label_width_px(&text), self.world_per_px), text))
    }

    /// Live values next to the rubber-band preview while drawing (live
    /// Onshape: line length, rectangle width and height, circle `Ø`, arc `R`,
    /// 13 px light blue, no box). Positions are sketch coordinates.
    /// The direction (sketch plane) the line being drawn runs: its length value is written along it, lying in the plane
    /// (live hover-matrix-1121 sk/inference-horiz-to-point-at, inference-vertical-at). None for other tools' values.
    pub fn preview_value_direction(&self, data: &SketchData) -> Option<DVec2> {
        let c = self.snapped()?.0;
        let from = match self.tool {
            Tool::Line if !self.tangent_mode => self.anchor_pos(&data.sketch)?,
            Tool::MidpointLine => *self.clicks.first().map(|k| &k.pos)?,
            _ => return None,
        };
        (c - from).try_normalize()
    }

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
                    let d = anchors.get(1).copied().unwrap_or(c) - *a;
                    let r = d.length();
                    if valid(r) {
                        // Live (behaviours "skasm (2026-10-27) where the live
                        // Ø readout sits"): halfway to the cursor when the
                        // label fits there, otherwise outside: across the
                        // centre for a mostly horizontal cursor, else
                        // straight below (above) the shape.
                        let text = dia(r * 2.0);
                        out.push((circle_readout_at(*a, *a + d, label_width_px(&text), px), text));
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
    /// The Line tool's end lands on what its snap lights (a point, a line's midpoint or a point on a curve).
    fn end_on_lit_target(&self) -> bool {
        self.tool == Tool::Line
            && !self.tangent_mode
            && self.mid_draw()
            && self.inference.as_ref().is_some_and(|i| (i.entity.is_some() && !is_direction(i.kind) && i.kind != InferenceKind::Tangent) || i.constraints.iter().any(|c| matches!(c, AutoConstraint::OnCurve(_))))
    }

    /// The Line tool's end dot when it lands on the lit target: seen through the target's highlight, not over it (live
    /// hover-matrix-recheck-1123: the dot shows through the hovered point's disc, #C3B89C, and under the line's band).
    pub fn end_dot_under_target(&self, data: &SketchData) -> PointData {
        let mut points = PointData::default();
        let Some(c) = self.snapped().map(|s| s.0).filter(|_| !self.pointer_rests && self.end_on_lit_target() && self.anchor_pos(&data.sketch).is_some()) else { return points };
        let v = self.frame.to_world(c) + self.frame.normal * 1e-6;
        // A third of it through the lit disc or band, drawn over them (live: #C3B89C is #449CCD at 32 % over the hover's
        // #FFC686, in sRGB; drawn under them it was hidden).
        let [r, g, b, _] = PREVIEW_COLOR;
        points.points.push(PointSprite { position: [v.x as f32, v.y as f32, v.z as f32], color: [r, g, b, 82], pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
        points
    }

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
        // The rubber band's inferred-direction dash (drawn after `poly`'s lines).
        let mut inferred_dash: Option<(DVec2, DVec2)> = None;
        let mut poly = |pts: &[DVec2], color: [u8; 4], dashed: bool| {
            let pts3: Vec<[f32; 3]> = pts.iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, color, 0, 2.0, on_top | if dashed { LineSegment::DASHED } else { 0 });
        };
        let circle_pts = |c: DVec2, r: f64, n: usize| -> Vec<DVec2> { (0..=n).map(|i| c + DVec2::from_angle(i as f64 / n as f64 * TAU) * r).collect() };
        let cursor = self.snapped().map(|s| s.0).filter(|_| !self.pointer_rests);
        // The Text tool's box between its two corners: no text until ✓ (live sketch/132, 134).
        let mut text_box = match self.text_clicks.as_slice() {
            [a, b] if self.pending_text.is_some() => Some((a.pos, b.pos)),
            _ => None,
        };
        if let Some(c) = cursor {
            let anchors: Vec<DVec2> = self.clicks.iter().map(|k| k.pos).collect();
            match self.tool {
                Tool::Line if self.tangent_mode => {
                    if let Some(from) = self.chain_end
                        && let Some(a) = data.sketch.point(from)
                    {
                        let mut tmp = data.sketch.clone();
                        match tmp.tangent_arc(from, c) {
                            Ok(ids) => poly(&tmp.tessellate(ids.curves[0], self.world_per_px * ARC_PREVIEW_TOL_PX), preview_color, false),
                            Err(_) => poly(&[a, c], preview_color, true),
                        }
                    }
                }
                Tool::Line => {
                    if let Some(a) = self.anchor_pos(&data.sketch) {
                        // Live (sketch/51–54): a 2 px blue line with 6 px blue dots at
                        // both ends; when its direction is inferred (horizontal,
                        // vertical, perpendicular, parallel) a 1 px yellow dash runs
                        // along its middle.
                        poly(&[a, c], preview_color, false);
                        if self.inference.as_ref().is_some_and(|i| is_direction(i.kind)) {
                            inferred_dash = Some((a, c));
                        }
                        let ends: &[DVec2] = if self.end_on_lit_target() { &[a] } else { &[a, c] };
                        for q in ends {
                            points.points.push(PointSprite { position: w(*q), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                        }
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
                        // Circumscribed: vertices on the circle; inscribed: edge midpoints on it.
                        let (rv, off) = if self.polygon_on_vertex() { (r, 0.0) } else { (r / (PI / n as f64).cos(), PI / n as f64) };
                        let pts: Vec<DVec2> = (0..=n).map(|i| *a + DVec2::from_angle(start + off + i as f64 / n as f64 * TAU) * rv).collect();
                        poly(&pts, preview_color, false);
                        // Live (skasm 10-26): dots at the centre and every vertex.
                        for p in std::iter::once(a).chain(&pts[..n]) {
                            points.points.push(PointSprite { position: w(*p), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                        }
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
                        let c = self.arc3_semicircle(anchors[0], anchors[1], c).unwrap_or(c);
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
                            poly(&tmp.tessellate(arc, self.world_per_px * ARC_PREVIEW_TOL_PX), preview_color, false);
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
                    // The pointer still on the last click: the spline through the clicks (live walk3-5 c2still).
                    let mut pts = anchors.clone();
                    if pts.last().is_none_or(|l| l.distance(c) > self.world_per_px * 0.5) {
                        pts.push(c);
                    }
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
                Tool::Text if self.pending_text.is_none() => {
                    if let Some(a) = anchors.first() {
                        text_box = Some((*a, c));
                    }
                }
                _ => {}
            }
            // (Not on a point or curve the snap lands on before the first click: the lit point and the snap square, or
            // the lit curve, show alone, live sketch/54, qa walk 1 w4-vertex, w4-mid; qa FEEL-LINE-SNAP-MODEL-LOOK;
            // skasm/hm-sk-1122 c4a, c4d.)
            let on_point = self.clicks.is_empty() && self.inference.as_ref().is_some_and(|i| matches!(i.kind, InferenceKind::Endpoint | InferenceKind::Midpoint | InferenceKind::Center | InferenceKind::Quadrant | InferenceKind::OnCurve));
            if self.tool.draws() && !on_point && !self.end_on_lit_target() && !(self.tool == Tool::Circle && !self.clicks.is_empty()) {
                points.points.push(PointSprite { position: w(c), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
            }
        }
        if let Some((a, b)) = text_box {
            let corners = [a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)];
            dash_dot.push(corners.iter().chain(&corners[..1]).copied().collect());
            for p in corners {
                points.points.push(PointSprite { position: w(p), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
            }
        }
        // Use: the hovered edges (a face's whole loop) drawn as hovered, a light orange band ~5 px with the edge's
        // darker core along it, small light-blue dots on their ends (live sketch-tools-1123 use-convert: ends
        // (68,156,205); qa SKT-USE-HOVER: ours a light-blue line, then a solid 4 px band 2.3x live's orange).
        if self.tool == Tool::Use {
            for pl in &self.use_preview {
                wide.push((pl.clone(), USE_HOVER_COLOR, 5.0));
                wide.push((pl.clone(), USE_HOVER_EDGE_COLOR, 2.5));
                let (Some(&a), Some(&b)) = (pl.first(), pl.last()) else { continue };
                if a.distance(b) > self.world_per_px {
                    for q in [a, b] {
                        points.points.push(PointSprite { position: w(q), color: preview_color, pick_id: 0, size: 4.0, style: on_top, _pad: 0 });
                    }
                }
            }
        }
        // Offset: preview of the pending offset and its direction arrow.
        if self.tool == Tool::Offset {
            if let Some((tmp, created)) = self.offset_result(&data.sketch) {
                // With its end points (live sketch/124, 303).
                for e in created {
                    poly(&tmp.tessellate(e, self.world_per_px), preview_color, false);
                    for q in tmp.endpoints(e).into_iter().flat_map(|(s, t)| [s, t]).filter_map(|p| tmp.point(p)) {
                        points.points.push(PointSprite { position: w(q), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
                    }
                }
            }
            if let Some((start, apex)) = self.offset_arrow(&data.sketch) {
                // Onshape's hollow arrow (as the Part Studio manipulators): a light grey 3 px outline under a 1.5 px
                // white line; hovered blue (live offset-look-1121).
                let d = (apex - start).try_normalize().unwrap_or(DVec2::X);
                let hovered = self.cursor.is_some_and(|c| dist_to_segment(c, start, apex) <= self.world_per_px * 10.0);
                let (outline, core) = if hovered { MANIPULATOR_HOVER } else { (MANIPULATOR_OUTLINE, [0xff, 0xff, 0xff, 0xff]) };
                push_hollow_arrow_in(&mut wide, self.snap_to_pixel(start), d, self.world_per_px, OFFSET_ARROW_STEM_PX, outline, core);
            }
            // Its dimension: a black line across the gap, a small filled arrowhead on each curve, run on past the
            // picked curve to a label there.
            if let (Some((mid, dir)), Some((label, _))) = (self.offset_dimension(&data.sketch), self.offset_value_label(&data.sketch)) {
                let (px, far) = (self.world_per_px, mid + dir * self.offset_distance);
                let back = if label.distance(mid) < px { mid - dir * px * 8.0 } else { mid };
                wide.push((vec![back, far], [0, 0, 0, 255], 1.0));
                for (tip, d) in [(mid, -dir), (far, dir)].into_iter().filter(|_| !self.offset_label_hovered) {
                    let base = tip - d * px * OFFSET_DIM_ARROW_PX;
                    for i in 0..=6 {
                        let t = i as f64 / 6.0 * 2.0 - 1.0;
                        wide.push((vec![tip, base + d.perp() * px * 1.75 * t], [0, 0, 0, 255], 1.0));
                    }
                }
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
                dim_segments(&pd.sketch, &c.kind, Some(&c.hints), at, self.world_per_px, &mut segs);
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
                dim_segments(&pd.sketch, &c.kind, Some(&c.hints), at, self.world_per_px, &mut segs);
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
            dim_segments(&data.sketch, &kind, None, pos, self.world_per_px, &mut segs);
            let text = dim_value_text(&kind, v / dim_display_scale(&kind), units);
            let half = DVec2::new(label_width_px(&text) * 0.5 + 3.0, 10.0) * self.world_per_px;
            dim_preview.extend(segs.iter().flat_map(|sg| clip_outside_box(*sg, pos, half)));
        }
        // Trim: the hovered curve's piece between its intersections, lit with its end dots (live, qa5 sketch tools).
        if self.tool == Tool::Trim
            && self.drag.is_none()
            && let (Some(h), Some(c)) = (self.hover, self.cursor)
            && let Some(curve) = data.sketch.curve(h)
            && let Some((t0, t1)) = data.sketch.trim_span_near(h, c, self.world_per_px * TRIM_CORNER_PX)
        {
            let n = 64;
            let (a, b) = curve.domain();
            let period = b - a;
            let at = |t: f64| curve.point(if curve.is_periodic() && t > b { t - period } else { t });
            let pts: Vec<DVec2> = (0..=n).map(|i| at(t0 + (t1 - t0) * i as f64 / n as f64)).collect();
            // The piece the hover orange, 5 px, its end dots orange (live sketch-edit-feel-1122 S4: lit like any hovered
            // curve; live2 sktools-1118's rgb(214, 189, 149) was the curve showing through the band's middle).
            let color = crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT);
            wide.push((pts, color, 5.0));
            for q in [at(t0), at(t1)] {
                points.points.push(PointSprite { position: w(q), color, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
            }
        }
        // Mirror: its line yellow-green while it waits selected for the entities (live workflows-1122 22 s34:
        // (189, 199, 100), no hover band); unlit once something is mirrored (s35).
        if let Some(axis) = self.shown_mirror_line() {
            let color = crate::palette::rgb8(crate::palette::viewport::MIRROR_LINE);
            wide.push((data.sketch.tessellate(axis, self.world_per_px * 0.25), color, 2.0));
        }
        // Extend: the curve picked first, lit yellow-green (live 12e).
        if self.tool == Tool::Extend
            && let Some(id) = self.extend_from
            && data.sketch.entity(id).is_some()
        {
            // (Selected orange once the preview snapped onto a boundary: live mc-reach-1122 R6.)
            let color = crate::palette::rgb8(if self.extend_crossed { crate::palette::viewport::SELECTION } else { crate::palette::viewport::QUERY_HOVER });
            wide.push((data.sketch.tessellate(id, self.world_per_px * 0.25), color, 4.0));
        }
        // Extend (live 11-18 sketch-tools): a hovered line's nearer end marked; once picked, the light-blue extension
        // to where the end goes, the orange square where it stops on a crossing curve.
        if self.tool == Tool::Extend
            && self.extend_from.is_none()
            && let (Some(h), Some(c)) = (self.hover, self.cursor)
            && let Some(q) = nearer_line_end(&data.sketch, h, c).and_then(|e| data.sketch.point(e))
        {
            let color = crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT);
            points.points.push(PointSprite { position: w(q), color, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
        }
        if self.tool == Tool::Extend
            && let (Some(c), Some(end)) = (self.cursor, self.extend_end)
            && let (Some((q, on)), Some(b)) = (self.extend_preview(&data.sketch, c), data.sketch.point(end))
        {
            poly(&[b, q], preview_color, false);
            if on.is_some() {
                points.points.push(PointSprite { position: w(q), color: SNAP_SQUARE_COLOR, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
            }
        }
        // Split marker on the hovered curve.
        if self.tool == Tool::Split
            && let (Some(h), Some(c)) = (self.hover, self.cursor)
            && data.sketch.curve(h).is_some()
        {
            let q = self.split_point(&data.sketch, h, c).0;
            points.points.push(PointSprite { position: w(q), color: guide_color, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
        }
        // Pattern / transform previews.
        if self.tool.is_pattern() && !self.selection.is_empty() {
            let tol = self.world_per_px;
            let shapes: Vec<Vec<DVec2>> = self.selection.iter().filter(|e| !is_point(&data.sketch, **e)).map(|e| data.sketch.tessellate(*e, tol * 0.25)).collect();
            for (t, dashed) in self.pattern_transforms() {
                for s in &shapes {
                    let pts: Vec<DVec2> = s.iter().map(|p| t(*p)).collect();
                    poly(&pts, preview_color, dashed);
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
                            let (dir, _, count, _) = self.linear_direction(second);
                            if !second {
                                push_hollow_square(&mut wide, self.snap_to_pixel(c), px, PATTERN_SQUARE_PX);
                            }
                            let qs = self.snap_to_pixel(q);
                            push_hollow_square(&mut wide, qs, px, PATTERN_SQUARE_PX);
                            push_hollow_arrow(&mut wide, qs + dir * px * PATTERN_SQUARE_PX * 0.5, dir, px, PATTERN_ARROW_STEM_PX);
                            let ink = [0x33, 0x33, 0x33, 255];
                            let mut segs = Vec::new();
                            // Live sketch-gaps2 / 3: a 1 px leader from the instance's centre to the label's
                            // near side, no arrowhead.
                            if let Some(label) = self.linear_count_label(second, &data.sketch) {
                                wide.push((vec![q, label - dir * px * PATTERN_LABEL_HALF_PX], ink, 1.0));
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
                    PatternHandle::Center if self.tool == Tool::Transform => push_hollow(&mut wide, circle_pts(q, px * XF_RING_R_PX, 32)),
                    PatternHandle::Center => {
                        let r = px * 7.0;
                        poly(&[q - DVec2::X * r, q + DVec2::X * r], guide_color, false);
                        poly(&[q - DVec2::Y * r, q + DVec2::Y * r], guide_color, false);
                    }
                    // Live sketch-gaps4: the handle arc and its open arrow; below 360° the rays and the angle dimension.
                    PatternHandle::CircularAngle => {
                        let c = self.pattern_center;
                        if let (Some((rr, a0, a1)), Some((base, t))) = (self.circular_handle_arc(&data.sketch), self.circular_arrow(&data.sketch)) {
                            let arc = |r: f64, from: f64, to: f64| -> Vec<DVec2> {
                                let n = (((to - from).abs() / TAU * 96.0).ceil() as usize).clamp(4, 96);
                                (0..=n).map(|i| c + DVec2::from_angle(from + (to - from) * i as f64 / n as f64) * r).collect()
                            };
                            wide.push((arc(rr, a0, a1), CIRCULAR_HANDLE_COLOR, 4.0));
                            push_hollow_arrow_in(&mut wide, base, t, px, CIRCULAR_ARROW_STEM_PX, CIRCULAR_HANDLE_COLOR, [0xff, 0xff, 0xff, 0xff]);
                            if !self.circular_step().1 {
                                let ra = rr / CIRCULAR_HANDLE_AT * (1.0 + CIRCULAR_LABEL_BEYOND);
                                let end = a0 + self.circular_angle;
                                let ink = [0x00, 0x00, 0x00, 0xff];
                                let mut segs = Vec::new();
                                for a in [a0, end] {
                                    let u = DVec2::from_angle(a);
                                    wide.push((vec![c, c + u * rr], CIRCULAR_RAY_COLOR, 2.0));
                                    segs.push([c + u * rr, c + u * (ra + px * 6.0)]);
                                }
                                let pts = arc(ra, a0, end);
                                for w in pts.windows(2) {
                                    segs.push([w[0], w[1]]);
                                }
                                let way = (end - a0).signum();
                                arrow(&mut segs, c + DVec2::from_angle(a0) * ra, -DVec2::from_angle(a0).perp() * way, px);
                                arrow(&mut segs, c + DVec2::from_angle(end) * ra, DVec2::from_angle(end).perp() * way, px);
                                for sg in segs {
                                    wide.push((vec![sg[0], sg[1]], ink, 1.0));
                                }
                            }
                        }
                    }
                    // The 2D manipulator (live editing-gaps2 / 3): grey double outlines, no lines to the anchor.
                    PatternHandle::Rotate => {
                        let m = self.pattern_center + self.xf_offset;
                        push_hollow(&mut wide, circle_pts(q, px * XF_RING_R_PX, 32));
                        // Two short ticks of the turning circle above and below the ring.
                        let r = px * XF_RING_AT_PX;
                        let (a0, a1) = (((XF_RING_R_PX + 1.0) / XF_RING_AT_PX).asin(), (23.0 / XF_RING_AT_PX).asin());
                        for side in [1.0, -1.0] {
                            let arc: Vec<DVec2> = (0..=6).map(|i| m + DVec2::from_angle(self.xf_angle + side * (a0 + (a1 - a0) * i as f64 / 6.0)) * r).collect();
                            push_hollow(&mut wide, arc);
                        }
                    }
                    PatternHandle::Scale | PatternHandle::MoveX | PatternHandle::MoveY => {
                        let m = self.pattern_center + self.xf_offset;
                        let d = (q - m).try_normalize().unwrap_or(DVec2::X);
                        let (head, half) = if h == PatternHandle::Scale { (XF_SCALE_HEAD_PX, XF_SCALE_HALF_W_PX) } else { (XF_ARROW_HEAD_PX, XF_ARROW_HALF_W_PX) };
                        let (base, n) = (q - d * px * head * 0.5, d.perp() * px * half);
                        push_hollow(&mut wide, vec![base + n, q + d * px * head * 0.5, base - n, base + n]);
                    }
                    PatternHandle::Move => {
                        let m = self.pattern_center + self.xf_offset;
                        let (u, v) = self.xf_axes();
                        let (a, b) = (px * XF_SQUARE_PX.0, px * XF_SQUARE_PX.1);
                        push_hollow(&mut wide, vec![m + u * a + v * a, m + u * b + v * a, m + u * b + v * b, m + u * a + v * b, m + u * a + v * a]);
                    }
                    _ => {}
                }
                if matches!(h, PatternHandle::LinearStep | PatternHandle::LinearSecondStep | PatternHandle::LinearCount | PatternHandle::LinearSecondCount | PatternHandle::CircularAngle) || self.tool == Tool::Transform {
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
        // A selected dimension with the pointer on it too (live workflow 32 step 42, qa
        // FEEL-DIM-SELECTED-REF-LINE-YELLOW: its line (255,198,133), not the selection's yellow).
        let hover_roles: Vec<(EntityId, bool)> = self
            .hover_constraint
            .filter(|c| (!self.selected_constraints.contains(c) || data.sketch.constraint(*c).is_some_and(|k| k.kind.is_dimension())) && use_glyph_entity(*c).is_none())
            .and_then(|c| data.sketch.constraint(c))
            .map(|c| {
                let masters = constraint_masters(&c.kind, &data.sketch);
                let mut roles: Vec<(EntityId, bool)> = c.kind.entities().into_iter().filter(|e| !masters.contains(e)).map(|e| (e, false)).collect();
                roles.extend(masters.into_iter().map(|e| (e, true)));
                roles
            })
            .or_else(|| {
                // A shared point's glyph: the curves meeting there.
                let p = self.hover_constraint.filter(|c| !self.selected_constraints.contains(c)).and_then(shared_glyph_point)?;
                Some(shared_ends(&data.sketch).into_iter().filter(|(q, _)| *q == p).flat_map(|(_, c)| c).map(|e| (e, false)).collect())
            })
            .unwrap_or_default();
        if !hover_roles.is_empty() {
            let selected_entities: Vec<EntityId> = self.selected_constraints.iter().filter(|c| Some(**c) != self.hover_constraint).filter_map(|c| data.sketch.constraint(*c)).flat_map(|c| c.kind.entities()).collect();
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
        // A hovered pattern glyph (live sketch/2222): every instance's curves
        // secondary yellow, the seed's included; their centre points orange.
        if let Some(pat) = self.hover_pattern.and_then(|id| data.patterns.iter().find(|p| p.id == id)) {
            let orange = crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT);
            for e in pat.seeds.iter().chain(pat.instances.iter().flatten()).map(|e| EntityId(*e)) {
                let Some(ent) = data.sketch.entity(e) else { continue };
                let centre = match ent.geometry {
                    noshape_sketch::Geometry::Point { .. } => Some(e),
                    noshape_sketch::Geometry::Circle { center, .. } | noshape_sketch::Geometry::Arc { center, .. } | noshape_sketch::Geometry::Ellipse { center, .. } | noshape_sketch::Geometry::EllipticalArc { center, .. } => Some(center),
                    _ => None,
                };
                if let Some(p) = centre.and_then(|c| data.sketch.point(c)) {
                    points.points.push(PointSprite { position: w(p), color: orange, pick_id: 0, size: 9.0, style: on_top, _pad: 0 });
                }
                if !is_point(&data.sketch, e) {
                    let pts3: Vec<[f32; 3]> = data.sketch.tessellate(e, self.world_per_px * 0.5).iter().map(|p| w(*p)).collect();
                    lines.push_polyline(&pts3, [0xf2, 0xff, 0x80, 255], 0, 3.0, on_top);
                }
            }
        }
        // Spline tangent handles: white points at the tips (the arms joining them to the spline ends are the sketch's
        // handle-arm lines, drawn and picked with its curves).
        for e in data.sketch.entities() {
            let Some((h0, h1)) = e.spline_handles() else { continue };
            let Some((s, en)) = e.endpoints() else { continue };
            for (end, h) in [(s, h0), (en, h1)] {
                if let (Some(a), Some(b)) = (data.sketch.point(end), data.sketch.point(h)) {
                    // (An arm the scene doesn't draw, a spline from before arms or one the shown display doesn't have
                    // yet: its line drawn here.)
                    // (The tip itself is the sketch's point, drawn with its hover and selection states.)
                    let drawn = |arm: &EntityId| self.scene_arms.as_ref().is_none_or(|shown| shown.contains(arm));
                    if !data.sketch.handle_arms(e.id).iter().flatten().filter(|arm| drawn(arm)).any(|arm| data.sketch.endpoints(*arm).is_some_and(|(p, q)| (p, q) == (end, h))) {
                        lines.push_polyline(&[w(a), w(b)], crate::palette::rgb8(crate::palette::viewport::SKETCH_HANDLE_LINE), 0, 1.0, on_top);
                    }
                }
            }
        }
        // Control polygons of selected/hovered B-splines (a Bezier's is its handle lines, drawn as the sketch's).
        for e in self.selection.iter().chain(self.hover.iter()) {
            if data.sketch.handle_arms(*e).iter().any(Option::is_some) {
                continue;
            }
            // (A used B-spline has none: live 2461.)
            if let Some(noshape_sketch::Entity { geometry: noshape_sketch::Geometry::BSpline { control, .. }, external: false, .. }) = data.sketch.entity(*e) {
                let pts3: Vec<[f32; 3]> = control.iter().filter_map(|c| data.sketch.point(*c)).map(w).collect();
                lines.push_polyline(&pts3, [0x90, 0x90, 0x90, 255], 0, 1.0, on_top | LineSegment::DASHED);
            }
        }
        // The guide to each point an alignment refers to (live2 combo perpAlign: a direction and an alignment show
        // the rubber band's dash and the guide); otherwise the guide to the inference's reference (a direction
        // inference of the line being drawn shows as its dash instead).
        if let Some(inf) = &self.inference {
            let aligned: Vec<DVec2> = inf
                .constraints
                .iter()
                .filter_map(|c| match c {
                    AutoConstraint::HorizontalWith(p) | AutoConstraint::VerticalWith(p) if *p == noshape_sketch::ORIGIN => Some(DVec2::ZERO),
                    AutoConstraint::HorizontalWith(p) | AutoConstraint::VerticalWith(p) if *p == WOKEN_POINT => self.woken_point.as_ref().map(|w| w.1),
                    AutoConstraint::HorizontalWith(p) | AutoConstraint::VerticalWith(p) if *p == WOKEN_EDGE_MID => self.woken_edge_mid.as_ref().map(|w| w.1),
                    AutoConstraint::HorizontalWith(p) | AutoConstraint::VerticalWith(p) if WOKEN_MIDS.contains(p) => self.woken_mid_points(&data.sketch).find(|(k, _)| k == p).map(|(_, q)| q),
                    AutoConstraint::HorizontalWith(p) | AutoConstraint::VerticalWith(p) if WOKEN_EDGE_ENDS.contains(p) => {
                        self.woken_edge.as_ref().map(|w| if *p == WOKEN_EDGE_ENDS[0] { w.1 } else { w.2 })
                    }
                    AutoConstraint::HorizontalWith(p) | AutoConstraint::VerticalWith(p) => data.sketch.point(*p),
                    _ => None,
                })
                .collect();
            let guides = if aligned.is_empty() { inf.reference.filter(|_| !(self.tool == Tool::Line && is_direction(inf.kind))).into_iter().collect() } else { aligned };
            for r in guides {
                // Live (part-studio/1032): a 1 px yellow dash, 4 px on / 4 off, as on a rubber band.
                lines.push_polyline(&[w(r), w(inf.position)], INFERRED_DASH_COLOR, 0, 1.0, on_top | LineSegment::INFERRED);
            }
        }
        // A point inference marks its point; a direction inference of the
        // line being drawn shows on the line itself (its dash and end dot),
        // an alignment by its guide alone (live part-studio/1032).
        // Live (behaviours "sketch snapping and hover", 11-12): a point or midpoint snapped to gets a yellow-orange 1 px
        // square round it, 17 px across (the point keeps its colour); an end snapped onto a curve only its glyph. The
        // square lies in the sketch plane, along its axes: a rhombus in an iso view (live hover-matrix-1121
        // sk/inference-on-point-at, inference-mid-of-line-at).
        let mut snap_square = |q: DVec2| {
            let half = SNAP_SQUARE_HALF_PX * self.world_per_px;
            let pts3: Vec<[f32; 3]> = [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.), (-1., -1.)].iter().map(|(x, y)| w(q + DVec2::new(*x, *y) * half)).collect();
            lines.push_polyline(&pts3, SNAP_SQUARE_COLOR, 0, 1.0, on_top);
        };
        let point_like = |k: InferenceKind| matches!(k, InferenceKind::Endpoint | InferenceKind::Center | InferenceKind::Midpoint | InferenceKind::Quadrant);
        if let Some(inf) = self.inference.as_ref().filter(|i| point_like(i.kind)) {
            snap_square(inf.position);
        }
        if let Some((_, q, Drop::Merge | Drop::Midpoint)) = self.drag_snap {
            snap_square(q);
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
        // The sketch frame.
        {
            let (lo, hi) = self.sketch_box();
            let pts3: Vec<[f32; 3]> = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y), lo].iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, crate::palette::rgb8(SKETCH_BOX_COLOR), 0, 1.0, 0);
        }
        for sg in &dim_preview {
            lines.segments.push(LineSegment { a: w(sg[0]), b: w(sg[1]), color: crate::palette::rgb8(crate::palette::pick(crate::palette::viewport::SKETCH_FULL, crate::palette::viewport_dark::SKETCH_FULL)), pick_id: 0, width: 1.5, style: on_top });
        }
        // Dimension graphics. The dimension line is broken around its value
        // (Onshape draws no box behind the text).
        let gaps: std::collections::HashMap<ConstraintId, (DVec2, DVec2)> = self
            .dimension_labels(data, units)
            .into_iter()
            .map(|(cid, pos, text, _)| (cid, (pos, DVec2::new(label_width_px(&text) * 0.5 + 3.0, 10.0) * self.world_per_px)))
            .collect();
        // Live (behaviours "skasm (2026-10-28) rectangle glyphs and vertical
        // dimension text"): the text always sits centred on the dimension
        // line, steep ones too (a "2.5" beside its line had been dragged there).
        for mut g in self.dimension_graphics(data) {
            if let Some((c, half)) = gaps.get(&g.constraint) {
                g.segments = g.segments.iter().flat_map(|sg| clip_outside_box(*sg, *c, *half)).collect();
            }
            let selected = self.selected_constraints.contains(&g.constraint);
            let hovered = self.hover_constraint == Some(g.constraint);
            // Onshape dimension colors: black driving, gray driven, gold hover/selection;
            // dark (live dk-932, the dark --os-dimension-* tokens): white, #ccc, #fcda7f / #dba517.
            use crate::palette::{pick, rgb8, viewport as vc, viewport_dark as vd};
            let color = rgb8(if selected {
                pick(0x6a4f06, 0xdba517)
            } else if hovered && g.driving {
                pick(0x987107, 0xfcda7f)
            } else if hovered {
                pick(vc::DIM_DRIVEN, vd::DIM_DRIVEN)
            } else if self.conflicting.contains(&g.constraint) {
                pick(vc::SKETCH_OVER, vd::SKETCH_OVER)
            } else if g.driving {
                pick(vc::SKETCH_FULL, vd::SKETCH_FULL)
            } else {
                pick(vc::DIM_DRIVEN, vd::DIM_DRIVEN)
            });
            // Live (sketch/17, 24): ~1.1 px of ink; selected too (workflow 32 step 42: its leader's core (112,86,17), 1 px,
            // and small arrowheads). Our line AA puts about width − 0.4 px of ink down.
            let width = 1.5;
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
        if let Some((a, c)) = inferred_dash {
            lines.push_polyline(&[w(a), w(c)], INFERRED_DASH_COLOR, 0, 1.0, on_top | LineSegment::INFERRED);
        }
        // Dash-dot laid out along the whole curve here: the line shader's
        // pattern restarts on every segment, so a circle's short segments
        // (foreshortened ones especially) came out solid (qa5 POLY1).
        for pts in dash_dot {
            for piece in dash_dot_pieces(&pts, DASH_DOT_PERIOD_PX * self.world_per_px) {
                let pts3: Vec<[f32; 3]> = piece.iter().map(|p| w(*p)).collect();
                lines.push_polyline(&pts3, preview_color, 0, 2.0, on_top);
            }
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

    /// The curves the snap relates to, lit as hovered (the caller marks them): a parallel / perpendicular / tangent's
    /// reference and a curve the end lands on, both for a combined snap (live2 combo perpLine: the last segment and the
    /// line; live hover-matrix-1121 sk/inference-on-line-at: the hovered band, the blue line through it).
    pub fn inference_lit(&self, sk: &Sketch) -> Vec<EntityId> {
        let Some(inf) = &self.inference else { return Vec::new() };
        let reference = inf.entity.filter(|_| matches!(inf.kind, InferenceKind::Parallel | InferenceKind::Perpendicular | InferenceKind::Tangent));
        let related = inf.constraints.iter().filter_map(|c| match c {
            AutoConstraint::OnCurve(e) | AutoConstraint::PerpendicularTo(e) | AutoConstraint::ParallelTo(e) | AutoConstraint::TangentTo(e) => Some(*e),
            _ => None,
        });
        let mut lit: Vec<EntityId> = Vec::new();
        for e in reference.into_iter().chain(related) {
            if !lit.contains(&e) && sk.entity(e).is_some_and(|x| x.kind() != EntityKind::Point) {
                lit.push(e);
            }
        }
        lit
    }

    pub fn picked(&self, data: Option<&SketchData>) -> Vec<EntityId> {
        self.picks_with(data, &self.selected_constraints)
    }

    /// What lights as picked: the tool's picks, the hovered glyph's entities (a selected glyph's light only under the
    /// pointer) and those of a dimension whose box is open on it (live sketch-edit-feel-1122 S1, S3).
    pub fn lit_picks(&self, data: Option<&SketchData>) -> Vec<EntityId> {
        let editing: Vec<ConstraintId> = self.dim_edit.iter().filter(|d| !d.created).map(|d| d.constraint).collect();
        self.picks_with(data, &editing)
    }

    fn picks_with(&self, data: Option<&SketchData>, constraints: &[ConstraintId]) -> Vec<EntityId> {
        let mut v: Vec<EntityId> = self.picks.iter().map(|p| p.0).collect();
        if let Some(data) = data {
            for cid in constraints.iter().chain(self.hover_constraint.iter()) {
                if let Some(e) = use_glyph_entity(*cid) {
                    v.push(e);
                } else if let Some(p) = shared_glyph_point(*cid) {
                    v.extend(shared_ends(&data.sketch).into_iter().filter(|(q, _)| *q == p).flat_map(|(_, c)| c));
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
    // An angle keeps the sector it was placed in (live sketch-edit-feel-1122).
    if let Some(p) = label {
        data.sketch.orient_angle_sector(cid, p);
    }
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

/// A line's end nearer `p` (Extend's pick); none for other curves.
fn nearer_line_end(sk: &Sketch, id: EntityId, p: DVec2) -> Option<EntityId> {
    let Some(Curve2::Line { .. }) = sk.curve(id) else { return None };
    let (s, e) = sk.endpoints(id)?;
    Some(if sk.point(s)?.distance(p) <= sk.point(e)?.distance(p) { s } else { e })
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
                // Live (ORIGIN-COINCIDENT-GLYPH; REST: COINCIDENT start-to-Origin):
                // a point on the Origin gets a Coincident, with its glyph; so does one on a point outside the sketch
                // it doesn't use (live 11-18 use-glyph: another sketch's point, a model vertex).
                if is_origin(sk, q) || sk.entity(q).is_some_and(|e| e.reference_only) {
                    let _ = sk.add_constraint(ConstraintKind::Coincident { a: pt, b: q });
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
        Some(AutoConstraint::TangentTo(c)) => {
            let _ = sk.add_constraint(ConstraintKind::Tangent { a: circle, b: *c });
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

/// A lone line's length box and dimension (live, behaviours "skasm
/// (2026-10-28) dimension labels vs glyph rows"): off the line's middle above
/// a slanted or horizontal line, left of a vertical one. A line drawn from `a` to
/// `b` (`drawn`, the Line tool): left of its drawing direction, a downward one's
/// on its right (live lenbox-1122, behaviours "skasm length box side").
fn line_length_field(sk: &Sketch, line: EntityId, wpp: f64, drawn: bool) -> OtfField {
    let mut f = length_field(sk, line, wpp);
    if let Some(Curve2::Line { a, b }) = sk.curve(line) {
        let n = (b - a).try_normalize().unwrap_or(DVec2::X).perp();
        // Above a slanted or horizontal line, left of a vertical one (live,
        // SKETCH-VDIM-SIDE-ARROWS).
        let vertical = n.y.abs() <= 1e-9;
        let n = if drawn || !(n.y < -1e-9 || (vertical && n.x > 0.0)) { n } else { -n };
        // The 55 × 25 px box's near edge 13 px off the line (lenbox-1122: its centre 25 px off a horizontal line, 41
        // off a vertical one, 35 off a slanted one).
        let off = 13.0 + 27.5 * n.x.abs() + 12.5 * n.y.abs();
        f.label = (a + b) * 0.5 + n * wpp * off;
        f.label_at = Some((line, n, off));
    }
    f
}

fn length_field(sk: &Sketch, line: EntityId, wpp: f64) -> OtfField {
    let label = match sk.curve(line) {
        // Closed shapes run counter-clockwise: the label goes outside (right of the edge).
        Some(Curve2::Line { a, b }) => (a + b) * 0.5 - (b - a).try_normalize().unwrap_or(DVec2::X).perp() * wpp * 26.0,
        _ => DVec2::ZERO,
    };
    OtfField { kind: ConstraintKind::Length { curve: line, value: 0.0 }, scale: 1.0, label, place: None, place_active: None, label_at: None }
}

/// A tangent arc's radius box (live, behaviours "… center point rectangle and
/// tangent arc", request 1503): on the ray from the centre through the middle
/// of the sweep, 30 px inside the arc.
fn tangent_arc_box(sk: &Sketch, arc: EntityId, wpp: f64) -> Option<BoxPlace> {
    match sk.curve(arc)? {
        Curve2::Arc { center, radius, start_angle, end_angle } => Some(BoxPlace::At(center + DVec2::from_angle((start_angle + end_angle) * 0.5) * (radius - 30.0 * wpp).max(0.0))),
        _ => None,
    }
}

fn radius_field(sk: &Sketch, arc: EntityId, wpp: f64, place: Option<BoxPlace>) -> OtfField {
    let label = default_label_pos(sk, &ConstraintKind::Radius { curve: arc, value: 0.0 }, wpp).unwrap_or_default();
    OtfField { kind: ConstraintKind::Radius { curve: arc, value: 0.0 }, scale: 1.0, label, place, place_active: place, label_at: None }
}

fn diameter_field(sk: &Sketch, circle: EntityId, wpp: f64, away: Option<DVec2>) -> OtfField {
    let place = away.map(BoxPlace::Rim);
    let label = default_label_pos(sk, &ConstraintKind::Diameter { curve: circle, value: 0.0 }, wpp).unwrap_or_default();
    OtfField { kind: ConstraintKind::Diameter { curve: circle, value: 0.0 }, scale: 1.0, label, place, place_active: place, label_at: None }
}

/// Primary then secondary axis diameter boxes of an ellipse / elliptical arc.
fn ellipse_fields(sk: &Sketch, id: EntityId, wpp: f64) -> Vec<OtfField> {
    let Some(Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. }) = sk.curve(id) else { return Vec::new() };
    let u = major.try_normalize().unwrap_or(DVec2::X);
    let off = wpp * 26.0;
    vec![
        OtfField { kind: ConstraintKind::EllipseRadius { ellipse: id, major: true, value: 0.0 }, scale: 2.0, label: center + major + u * off, place: None, place_active: None, label_at: None },
        OtfField { kind: ConstraintKind::EllipseRadius { ellipse: id, major: false, value: 0.0 }, scale: 2.0, label: center + u.perp() * (minor + off), place: None, place_active: None, label_at: None },
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
/// An outline (points in order) touching the box `lo`..`hi`.
fn crosses_box(pts: &[DVec2], lo: DVec2, hi: DVec2) -> bool {
    let inside = |p: DVec2| p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y;
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
/// A fillet arc of radius `r` at the corner point `pid` of two lines (drawn
/// whether or not the legs are long enough).
struct CornerArc {
    vertex: DVec2,
    /// From the vertex toward the arc's centre.
    bisector: DVec2,
    /// The angle between the legs.
    theta: f64,
    center: DVec2,
    radius: f64,
    a0: f64,
    sweep: f64,
    /// The tangent points on the two legs.
    ends: [DVec2; 2],
}

impl CornerArc {
    fn mid(&self) -> DVec2 {
        self.center + DVec2::from_angle(self.a0 + self.sweep * 0.5) * self.radius
    }
}

fn corner_arc(sk: &Sketch, pid: EntityId, r: f64) -> Option<CornerArc> {
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
    let t = r / (theta * 0.5).tan();
    let bisector = (ua + ub).normalize();
    let c = v + bisector * (r / (theta * 0.5).sin());
    let (pa, pb) = (v + ua * t, v + ub * t);
    let a0 = (pa - c).to_angle();
    let sweep = noshape_sketch::geom::ccw_sweep(a0, (pb - c).to_angle());
    let sweep = if sweep > PI { sweep - TAU } else { sweep };
    Some(CornerArc { vertex: v, bisector, theta, center: c, radius: r, a0, sweep, ends: [pa, pb] })
}

/// The lines a fillet arc trims (at its ends) and their vectors.
fn fillet_legs(sk: &Sketch, arc: EntityId) -> Vec<(EntityId, DVec2)> {
    sk.endpoints(arc)
        .map(|(a, b)| [a, b])
        .into_iter()
        .flatten()
        .flat_map(|pt| sk.users_of_point(pt))
        .filter(|e| *e != arc)
        .filter_map(|e| match sk.curve(e) {
            Some(Curve2::Line { a, b }) => Some((e, b - a)),
            _ => None,
        })
        .collect()
}

/// The sketch's Origin point ([`origin_point`]).
/// The sketch's Origin point (external, reference only, at 0).
pub fn is_origin(sk: &Sketch, e: EntityId) -> bool {
    sk.entity(e).is_some_and(|e| e.external && e.reference_only && matches!(e.geometry, noshape_sketch::Geometry::Point { pos } if pos.length() < 1e-12))
}

pub fn origin_point(sk: &mut Sketch) -> EntityId {
    let existing = sk.entities().find(|e| e.external && matches!(e.geometry, noshape_sketch::Geometry::Point { pos } if pos.length() < 1e-12)).map(|e| e.id);
    existing.unwrap_or_else(|| {
        // Live (qa5 P1-4): constraints go to the external Origin; no sketch
        // point is made there (none drawn, picked or used as a hole location).
        let o = sk.use_external_point(DVec2::ZERO);
        let _ = sk.set_construction(o, true);
        let _ = sk.set_reference_only(o, true);
        o
    })
}

/// Entity an automatic constraint refers to.
fn auto_target(c: &AutoConstraint) -> Option<EntityId> {
    use AutoConstraint::*;
    match *c {
        CoincidentPoint(e) | OnCurve(e) | MidpointOf(e) | HorizontalWith(e) | VerticalWith(e) | PerpendicularTo(e) | ParallelTo(e) | TangentTo(e) | QuadrantOf(e, _) | ConcentricTo(e) => Some(e),
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
        ConcentricTo(e) => ConcentricTo(m(e)),
        other => other,
    }
}

/// Constrain a linear pattern's instances to their seeds (Onshape's linear
/// pattern constraint): every instance point = seed point + k * step, the
/// step's length is a spacing dimension and an axis-aligned step keeps its
/// direction. Instance points are found by position (seed + k * step).
fn linear_grid_pattern(data: &mut SketchData, seeds: &[EntityId], x: (DVec2, usize), y: (DVec2, usize), wpp: f64) -> Result<Vec<Vec<EntityId>>, noshape_sketch::SketchError> {
    // A direction line goes with the pattern's instances (an edit replaces it with them).
    let with_line = |data: &SketchData, groups: &mut [Vec<EntityId>], line: Option<EntityId>| {
        if let (Some(line), Some(first)) = (line, groups.first_mut()) {
            first.push(line);
            first.extend(data.sketch.endpoints(line).map(|(a, b)| [a, b]).into_iter().flatten());
        }
    };
    let mut groups = data.sketch.linear_pattern_groups(seeds, x.0, x.1)?;
    let line = link_linear_pattern(data, seeds, &groups, x.0, wpp);
    if y.1 > 1 {
        let mut row = seeds.to_vec();
        row.extend(groups.iter().flatten().copied());
        let mut rows = data.sketch.linear_pattern_groups(&row, y.0, y.1)?;
        let second = link_linear_pattern(data, &row, &rows, y.0, wpp);
        with_line(data, &mut rows, second);
        groups.extend(rows);
    }
    with_line(data, &mut groups, line);
    Ok(groups)
}

/// Points the links made so far relate (a link between two of one group would only repeat what the others imply:
/// a pattern closing on its own seed, the solver's dependent equations).
#[derive(Default)]
struct Related(Vec<(EntityId, EntityId)>);

impl Related {
    fn root(&self, mut p: EntityId) -> EntityId {
        while let Some((_, up)) = self.0.iter().find(|(x, _)| *x == p) {
            p = *up;
        }
        p
    }

    /// Whether `a` and `b` were apart (then related from now on).
    fn join(&mut self, a: EntityId, b: EntityId) -> bool {
        let (ra, rb) = (self.root(a), self.root(b));
        if ra == rb {
            return false;
        }
        self.0.push((rb, ra));
        true
    }
}

/// The points of `instance`'s entities paired with the seeds' (entities in the seeds' order, as Onshape lists a
/// pattern's instances): a point with its seed point, a curve's points with its seed's.
/// `map` the instance's transform: a curve's points are paired by where they land (an arc drawn the other way round
/// lists its ends the other way).
fn instance_points(sk: &Sketch, seeds: &[EntityId], instance: &[EntityId], map: &dyn Fn(DVec2) -> DVec2, tol: f64) -> Option<Vec<(EntityId, EntityId)>> {
    let mut out = Vec::new();
    for (s, i) in seeds.iter().zip(instance) {
        let (es, ei) = (sk.entity(*s)?, sk.entity(*i)?);
        if es.kind() != ei.kind() {
            return None;
        }
        let (ps, pi): (Vec<EntityId>, Vec<EntityId>) = if es.kind() == EntityKind::Point { (vec![*s], vec![*i]) } else { (es.point_refs().to_vec(), ei.point_refs().to_vec()) };
        if ps.len() != pi.len() {
            return None;
        }
        for p in ps {
            let to = map(sk.point(p)?);
            out.push((p, pi.iter().copied().find(|q| sk.point(*q).is_some_and(|x| (x - to).length() <= tol))?));
        }
    }
    Some(out)
}

/// Entities already in the sketch linked as a circular pattern about point `center`, as the tool links the copies
/// it makes (a translated sketch: Onshape stores the instances, ours makes them; linked, editing the pattern or a
/// dimension carries them along). `instances[0]`: the seeds; each instance's entities in the seeds' order; `full`:
/// spread over a turn. Nothing when they aren't rotated copies of the seeds. The pattern's id.
pub fn link_circular_pattern(data: &mut SketchData, instances: &[Vec<EntityId>], center: EntityId, full: bool) -> Option<u32> {
    let seeds = instances.first()?.clone();
    if instances.len() < 2 || instances.iter().any(|g| g.len() != seeds.len()) {
        return None;
    }
    let sk = &data.sketch;
    let c = sk.point(center)?;
    // The seeds' farthest point from the centre: each instance's angle from where its copy is (any of the first
    // seed's points: the arcs' ends may be listed either way).
    let far = seeds.iter().flat_map(|e| if sk.kind(*e) == Some(EntityKind::Point) { vec![*e] } else { sk.entity(*e).map(|x| x.point_refs().to_vec()).unwrap_or_default() }).filter_map(|p| Some((p, sk.point(p)?))).max_by(|x, y| (x.1 - c).length().total_cmp(&(y.1 - c).length()))?;
    let r = (far.1 - c).length();
    if r < 1e-12 {
        return None;
    }
    let tol = r * 1e-7;
    let mut related = Related::default();
    let mut links: Vec<(EntityId, EntityId, f64)> = Vec::new();
    let mut angles = Vec::new();
    for g in &instances[1..] {
        // Rotations taking the far point onto one of the instance's points; the one that maps every seed point.
        let candidates: Vec<f64> = g.iter().flat_map(|e| if sk.kind(*e) == Some(EntityKind::Point) { vec![*e] } else { sk.entity(*e).map(|x| x.point_refs().to_vec()).unwrap_or_default() }).filter_map(|q| sk.point(q)).filter(|q| ((*q - c).length() - r).abs() <= tol).map(|q| (far.1 - c).angle_to(q - c)).collect();
        // (Over a full turn, a whole step of it exactly: what the geometry measures is a hair off, and the sketch's
        // perpendiculars and the like hold the exact angle.)
        let n = instances.len() as f64;
        let exact = |a: f64| {
            let k = (a * n / TAU).round();
            if full && (a - k * TAU / n).abs() < 1e-6 { k * TAU / n } else { a }
        };
        let candidates: Vec<f64> = candidates.into_iter().map(exact).collect();
        let (angle, pairs) = candidates.into_iter().find_map(|angle| {
            let t = Transform::Rotate { center: c, angle };
            instance_points(sk, &seeds, g, &move |p| t.apply(p), tol).map(|p| (angle, p))
        })?;
        for (a, b) in pairs {
            if related.join(a, b) {
                links.push((a, b, angle));
            }
        }
        angles.push(angle);
    }
    let sk = &mut data.sketch;
    for (seed, inst, angle) in links {
        let _ = sk.add_constraint(ConstraintKind::CircularPatternLink { seed, inst, center, angle });
    }
    for (g, angle) in instances[1..].iter().zip(&angles) {
        for (seed, inst) in seeds.iter().zip(g) {
            if matches!(sk.kind(*seed), Some(EntityKind::Circle | EntityKind::Ellipse | EntityKind::EllipticalArc)) {
                let _ = sk.add_constraint(ConstraintKind::CircularPatternLink { seed: *seed, inst: *inst, center, angle: *angle });
            }
        }
    }
    let angle = if full { TAU } else { *angles.last()? };
    let ids = |g: &[EntityId]| g.iter().map(|e| e.0).collect::<Vec<u32>>();
    Some(record_pattern(data, noshape_model::sketch_feature::SketchPattern {
        id: 0,
        circular: true,
        seeds: ids(&seeds),
        instances: instances[1..].iter().map(|g| ids(g)).collect(),
        count: instances.len(),
        distance: 0.0,
        angle,
        center: [c.x, c.y],
        second_count: 1,
        second_distance: 0.0,
        second_angle: 0.0,
    }))
}

/// A linear pattern's step: its offset, and each seed point with its copy one step on.
type Step = (DVec2, Vec<(EntityId, EntityId)>);

/// Entities already in the sketch linked as a linear pattern (as [`link_circular_pattern`]): `grid[a][b]` the
/// instance b along the first direction and a along the second, `grid[0][0]` the seeds, each in the seeds' order.
pub fn link_linear_pattern_grid(data: &mut SketchData, grid: &[Vec<Vec<EntityId>>]) -> Option<u32> {
    let seeds = grid.first()?.first()?.clone();
    if grid.iter().flatten().any(|g| g.len() != seeds.len()) || grid.iter().any(|row| row.len() != grid[0].len()) || grid.len() * grid[0].len() < 2 {
        return None;
    }
    let sk = &data.sketch;
    let points = |g: &[EntityId]| g.iter().flat_map(|e| if sk.kind(*e) == Some(EntityKind::Point) { vec![*e] } else { sk.entity(*e).map(|x| x.point_refs().to_vec()).unwrap_or_default() }).collect::<Vec<_>>();
    let s0 = *points(&seeds).first()?;
    let p0 = sk.point(s0)?;
    // The step along each direction: the one taking the seeds onto the next instance (none: one instance that way).
    let step_to = |g: Option<&Vec<EntityId>>| -> Option<Option<Step>> {
        let Some(g) = g else { return Some(None) };
        points(g).into_iter().filter_map(|q| sk.point(q)).find_map(|q| {
            let d = q - p0;
            let tol = d.length().max(1e-9) * 1e-7;
            instance_points(sk, &seeds, g, &move |p| p + d, tol).map(|pairs| (d, pairs))
        }).map(Some)
    };
    let (first, second) = (step_to(grid[0].get(1))?, step_to(grid.get(1).and_then(|r| r.first()))?);
    let (d1, d2) = (first.as_ref().map_or(DVec2::ZERO, |x| x.0), second.as_ref().map_or(DVec2::ZERO, |x| x.0));
    let reference = |x: &Option<Step>| x.as_ref().and_then(|(_, pairs)| pairs.iter().find(|(a, _)| *a == s0).map(|p| p.1));
    let (r1, r2) = (reference(&first), reference(&second));
    let tol = d1.length().max(d2.length()).max(1e-9) * 1e-7;
    let mut related = Related::default();
    let mut links: Vec<ConstraintKind> = Vec::new();
    let mut groups: Vec<Vec<EntityId>> = Vec::new();
    // The reference copies relate to the seed point through their own step.
    for r in [r1, r2].into_iter().flatten() {
        related.join(s0, r);
    }
    for (a, row) in grid.iter().enumerate() {
        for (b, g) in row.iter().enumerate() {
            if a == 0 && b == 0 {
                continue;
            }
            // Along the first direction from the seeds; a later row from its first row's instance.
            let (base, k, r, d) = if a == 0 { (&seeds, b, r1?, d1) } else { (&grid[0][b], a, r2?, d2) };
            let off = d * k as f64;
            for (sp, ip) in instance_points(sk, base, g, &move |p| p + off, tol)? {
                if related.join(sp, ip) {
                    links.push(ConstraintKind::PatternLink { seed: sp, inst: ip, ref_seed: s0, ref_inst: r, k: k as f64 });
                }
            }
            for (seed, inst) in base.iter().zip(g) {
                if sk.kind(*seed) == Some(EntityKind::Circle) {
                    links.push(ConstraintKind::PatternLink { seed: *seed, inst: *inst, ref_seed: s0, ref_inst: r, k: k as f64 });
                }
            }
            groups.push(g.clone());
        }
    }
    for k in links {
        let _ = data.sketch.add_constraint(k);
    }
    Some(record_pattern(data, noshape_model::sketch_feature::SketchPattern {
        id: 0,
        circular: false,
        seeds: seeds.iter().map(|e| e.0).collect(),
        instances: groups.iter().map(|g| g.iter().map(|e| e.0).collect()).collect(),
        count: grid[0].len(),
        distance: d1.length(),
        angle: d1.y.atan2(d1.x),
        center: [p0.x, p0.y],
        second_count: grid.len(),
        second_distance: d2.length(),
        second_angle: d2.y.atan2(d2.x),
    }))
}

/// A pattern's record (Edit pattern from its glyph), as the tool keeps it, under a new id.
fn record_pattern(data: &mut SketchData, mut p: noshape_model::sketch_feature::SketchPattern) -> u32 {
    p.id = data.patterns.iter().map(|p| p.id + 1).max().unwrap_or(1);
    let id = p.id;
    data.patterns.push(p);
    id
}

fn link_linear_pattern(data: &mut SketchData, seeds: &[EntityId], groups: &[Vec<EntityId>], step: DVec2, wpp: f64) -> Option<EntityId> {
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
    let s0 = *seed_points.first()?;
    let p0 = sk.point(s0)?;
    let first0 = groups.first().and_then(|g| find(sk, g, p0 + step))?;
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
                let _ = sk.add_constraint(ConstraintKind::PatternLink { seed: *seed, inst, ref_seed: s0, ref_inst: first0, k: kk });
            }
        }
    }
    // Live's direction (a pattern's `direction1` / `direction2`): a construction line from the seed to its first
    // instance, its ends on them, horizontal / vertical along an axis, its length the spacing dimension.
    let line = sk.add_line(p0, p0 + step);
    let _ = sk.set_construction(line, true);
    let (a, b) = sk.endpoints(line)?;
    let _ = sk.add_constraint(ConstraintKind::Coincident { a, b: s0 });
    let _ = sk.add_constraint(ConstraintKind::Coincident { a: b, b: first0 });
    if step.y.abs() < 1e-12 {
        let _ = sk.add_constraint(ConstraintKind::Horizontal { a: line, b: None });
    } else if step.x.abs() < 1e-12 {
        let _ = sk.add_constraint(ConstraintKind::Vertical { a: line, b: None });
    }
    let label = p0 + step * 0.5 + step.try_normalize().unwrap_or(DVec2::X).perp() * wpp * 24.0;
    add_dimension(data, ConstraintKind::Length { curve: line, value: step.length() }, Some(label), None);
    Some(line)
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
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read the image: {e}"))?;
    let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "image".into());
    image_from_bytes(name, bytes)
}

/// An image's bytes as Insert image takes them: (name, bytes, width / height).
pub fn image_from_bytes(name: String, bytes: Vec<u8>) -> Result<(String, Vec<u8>, f64), String> {
    let (w, h) = image::load_from_memory(&bytes).map(|img| (img.width(), img.height())).map_err(|e| format!("Could not read the image: {e}"))?;
    if w == 0 || h == 0 {
        return Err("The image is empty".into());
    }
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
    data.use_ends.retain(|k, _| data.external_refs.contains_key(k));
}

/// Paste sketch onto a plane: the copy loses its ties to the Origin and is centred on it (live workflows-1122 29:
/// a 40 × 30 rectangle drawn from the origin lands at −20..20 × −15..15), keeping its own constraints.
pub fn centre_pasted_sketch(data: &mut SketchData) {
    let origin: Vec<EntityId> = data.sketch.entities().filter(|e| is_origin(&data.sketch, e.id)).map(|e| e.id).collect();
    // (A point curves share with the Origin stays theirs.)
    let (shared, lone): (Vec<EntityId>, Vec<EntityId>) = origin.into_iter().partition(|o| !data.sketch.users_of_point(*o).is_empty());
    for o in shared {
        let _ = data.sketch.set_external(o, false);
        let _ = data.sketch.set_reference_only(o, false);
    }
    data.sketch.remove_entities(&lone);
    let own: Vec<EntityId> = data.sketch.entities().filter(|e| !e.external).map(|e| e.id).collect();
    // Live (behaviours "skasm paste sketch corners"): the mean of its line end points lands on the origin (an L's corner
    // mean, not its box centre or centroid); with no lines, the box centre.
    let ends: Vec<DVec2> = own.iter().filter_map(|e| match data.sketch.curve(*e) {
        Some(Curve2::Line { a, b }) => Some([a, b]),
        _ => None,
    }).flatten().collect();
    let b = own.iter().filter_map(|e| data.sketch.entity_bbox(*e)).fold(noshape_sketch::BBox::EMPTY, |a, b| a.union(&b));
    let centre = if ends.is_empty() { (!b.is_empty()).then(|| b.center()) } else { Some(ends.iter().sum::<DVec2>() / ends.len() as f64) };
    if let Some(c) = centre.filter(|c| c.length() > 1e-12)
        && data.sketch.transform(&own, Transform::Translate(-c)).is_ok()
    {
        for l in data.dimension_labels.values_mut() {
            *l = [l[0] - c.x, l[1] - c.y];
        }
    }
    prune_exprs(data);
    data.sketch.solve();
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
            // Between two lines it sits on its dimension line at the first one's midpoint (live workflow 22 s35, an
            // Offset's distance; behaviours "skasm (11-21 20:55) sketch Offset preview").
            if line_of(sk, *a).is_some() && line_of(sk, *b).is_some() {
                return Some((p + q) * 0.5);
            }
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

/// Where a circle's / polygon's Ø readout sits (live, behaviours "skasm
/// (2026-10-27) where the live Ø readout sits"): halfway to the cursor
/// `tip` when the label (`w` px wide) fits there, otherwise outside: across
/// the centre for a mostly horizontal cursor, else straight below (above)
/// the shape.
fn circle_readout_at(a: DVec2, tip: DVec2, w: f64, px: f64) -> DVec2 {
    let d = tip - a;
    let r = d.length();
    if r >= px * (w + 40.0) {
        a + d * 0.5
    } else if d.x.abs() > d.y.abs() {
        a + DVec2::new(-d.x.signum() * (r + px * (w * 0.5 + 4.0)), 0.0)
    } else {
        a + DVec2::new(0.0, -d.y.signum() * (r + px * 22.0))
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
    // Live (behaviours "skasm (2026-10-28) rectangle glyphs and vertical
    // dimension text"): the arrows stay inside however short the dimension;
    // below ~50 px they overlap the text.
    out.push([q1 + dir * t.min(0.0), q1 + dir * t.max(len)]);
    arrow(out, q1, -dir, s);
    arrow(out, q2, dir, s);
}

/// A radius or diameter dimension as its text sits (live sketch-edit-feel-1122 S2): outside the curve a leader from
/// the text to it, the arrow pointing in; inside a circle the whole diameter through the text, an arrow at each end;
/// inside an arc the radius from the centre through the text, its arrow at the arc; off an arc's ends a short leader
/// to the nearer end.
fn radial_dim(out: &mut Vec<[DVec2; 2]>, c: DVec2, r: f64, pos: DVec2, diameter: bool, span: Option<(f64, f64)>, s: f64) {
    let v = (pos - c).try_normalize().unwrap_or(DVec2::X);
    let tip = c + v * r;
    if let Some((a0, a1)) = span
        && noshape_sketch::geom::angle_in_range(v.to_angle(), a0, a1 - a0, 1e-9).is_none()
    {
        let (e0, e1) = (c + DVec2::from_angle(a0) * r, c + DVec2::from_angle(a1) * r);
        let end = if e0.distance(pos) <= e1.distance(pos) { e0 } else { e1 };
        out.push([end, pos]);
        arrow(out, end, end - pos, s);
        return;
    }
    if (pos - c).length() > r {
        out.push([tip, pos]);
        arrow(out, tip, -v, s);
    } else if diameter {
        let t2 = c - v * r;
        out.push([t2, tip]);
        arrow(out, tip, v, s);
        arrow(out, t2, -v, s);
    } else {
        out.push([c, tip]);
        arrow(out, tip, v, s);
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

/// `hints`: the placed constraint's (an angle's sector); a dimension being placed has none and follows the pointer.
fn dim_segments(sk: &Sketch, k: &ConstraintKind, hints: Option<&noshape_sketch::Hints>, pos: DVec2, s: f64, out: &mut Vec<[DVec2; 2]>) {
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
            // A placed angle keeps its sector, its rays as the solve measures it (live sketch-edit-feel-1122); while
            // placing, the sector holding the label (rays sorted ccw).
            let placed = hints.filter(|h| h.sector).map(|h| {
                let u = if h.flip_a { -da } else { da };
                let v = if h.flip_b { -db } else { db };
                if h.reverse { (v, u) } else { (u, v) }
            });
            let chosen = placed.unwrap_or_else(|| {
                let mut rays = [da, db, -da, -db];
                rays.sort_by(|x, y| x.to_angle().rem_euclid(TAU).total_cmp(&y.to_angle().rem_euclid(TAU)));
                (0..4)
                    .map(|i| (rays[i], rays[(i + 1) % 4]))
                    .find(|(s1, s2)| noshape_sketch::geom::angle_in_range(th, s1.to_angle(), noshape_sketch::geom::ccw_sweep(s1.to_angle(), s2.to_angle()), 1e-12).is_some())
                    .unwrap_or((rays[0], rays[1]))
            });
            angle_arc(out, c, chosen.0, chosen.1, r, s);
            // The text outside its sector: the arc runs on past the nearer line to it.
            let (a0, sweep) = (chosen.0.to_angle(), noshape_sketch::geom::ccw_sweep(chosen.0.to_angle(), chosen.1.to_angle()));
            if noshape_sketch::geom::angle_in_range(th, a0, sweep, 1e-12).is_none() {
                let (past, before) = (noshape_sketch::geom::ccw_sweep(a0 + sweep, th), noshape_sketch::geom::ccw_sweep(th, a0));
                let pts = if past <= before { arc_points(c, r, a0 + sweep, past) } else { arc_points(c, r, th, before) };
                polyline(out, &pts);
            }
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
        Midpoint { point, .. } | MidpointOf { point, .. } => vec![point],
        Pierce { point, .. } => vec![point],
        Quadrant { point, .. } => vec![point],
        Symmetric { a, b, .. } => vec![a, b],
        Fix { entity } => vec![entity],
        // Live: the slot's three offset glyphs sit together at the end cap.
        Offset { reference, .. } => vec![reference],
        OffsetPair { copy, .. } => vec![copy],
        Normal { line, curve } => vec![line, curve],
        // Live draws a tangent's glyph once, on its first entity (the line of
        // a fillet, the cap of a slot; qa5 sketch tools).
        Tangent { a, .. } => vec![a],
        // Live (same behaviours): a Parallel or Perpendicular is drawn once, on
        // its first entity (a rectangle's on its bottom, left and top edges).
        Parallel { a, .. } | Perpendicular { a, .. } => vec![a],
        // Live 1716: one glyph, on the curve.
        Flow { a, .. } => vec![a],
        Concentric { a, b } | Equal { a, b } | Collinear { a, b } | Curvature { a, b } => vec![a, b],
        _ => Vec::new(),
    }
}

/// Live 1716: a Flow's glyph sits two-thirds along its curve from the joint
/// (the curve's end nearer the other entity).
fn flow_anchor(sk: &Sketch, curve: EntityId, other: EntityId) -> Option<DVec2> {
    let c = sk.curve(curve)?;
    let (t0, t1) = c.domain();
    let gap = |p: DVec2| sk.curve(other).map_or(f64::MAX, |o| o.distance(p));
    let t = if gap(c.point(t0)) <= gap(c.point(t1)) { t0 + (t1 - t0) * 2.0 / 3.0 } else { t0 + (t1 - t0) / 3.0 };
    Some(c.point(t))
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
    // A spline's end-tangent handle alone: its length (Onshape dimensions
    // the handle as a line from the spline's end; SKETCH-SPLINE-END-DERIVATIVES).
    if let [h] = ids
        && let Some(end) = sk.entities().find_map(|e| {
            let (h0, h1) = e.spline_handles()?;
            let (s0, s1) = e.endpoints()?;
            if *h == h0 { Some(s0) } else if *h == h1 { Some(s1) } else { None }
        })
    {
        return Some(dist(end, *h));
    }
    match ids {
        [a] => match kind(*a)? {
            // Live: one line is always its aligned length, wherever the label
            // goes; the horizontal / vertical forms come from its two points.
            EntityKind::Line => {
                let (pa, pb) = sk.endpoints(*a)?;
                Some(ConstraintKind::Length { curve: *a, value: (sk.point(pb)? - sk.point(pa)?).length() })
            }
            EntityKind::Circle => Some(ConstraintKind::Diameter { curve: *a, value: 0.0 }),
            EntityKind::Arc => Some(ConstraintKind::Radius { curve: *a, value: 0.0 }),
            EntityKind::Conic => Some(ConstraintKind::Rho { conic: *a, value: 0.0 }),
            EntityKind::Ellipse | EntityKind::EllipticalArc => {
                // The diameter whose dimension line runs along the label's side: off across the major axis (a
                // horizontal ellipse's label above) the major, off along it (at the side) the minor (live (6)
                // Dimension tool; sk2-1121 dim-ellipse-axes: picked at its minor end, a click at (5, 25) mm gives MAJOR).
                let major = match sk.curve(*a) {
                    Some(Curve2::Ellipse { center, major, .. }) | Some(Curve2::EllipticalArc { center, major, .. }) => {
                        let d = cursor - center;
                        d.dot(major.normalize().perp()).abs() >= d.dot(major.normalize()).abs()
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

/// Two conics, Beziers or splines that don't share an end point: Onshape
/// makes no dimension between them.
fn unmeasurable_curve_pair(sk: &Sketch, a: EntityId, b: EntityId) -> bool {
    let spline = |e: EntityId| sk.kind(e).is_some_and(|k| k.is_spline());
    if a == b || !spline(a) || !spline(b) {
        return false;
    }
    let ends = |e: EntityId| sk.endpoints(e).map(|(s, t)| vec![s, t]).unwrap_or_default();
    !ends(a).iter().any(|p| ends(b).contains(p))
}

/// An ellipse and a plane can't be dimensioned (live sk2-1121 dim-ellipse-axes: an ellipse, then the Right plane by its
/// outline: "A dimension cannot be created between these items.").
fn ellipse_and_plane(data: &SketchData, a: EntityId, b: EntityId) -> bool {
    use noshape_model::reference::Reference;
    let ellipse = |e: EntityId| matches!(data.sketch.kind(e), Some(EntityKind::Ellipse | EntityKind::EllipticalArc));
    let plane = |e: EntityId| matches!(data.external_refs.get(&e.0), Some(Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. }));
    (ellipse(a) && plane(b)) || (ellipse(b) && plane(a))
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
        // Live 1716: the spline / conic / Bezier first, whatever the pick order.
        ConstraintTool::Flow => {
            let b = b?;
            let curve = |e: EntityId| sk.kind(e).is_some_and(|k| k.is_spline());
            if !curve(a) && curve(b) { ConstraintKind::Flow { a: b, b: a } } else { ConstraintKind::Flow { a, b } }
        }
        ConstraintTool::Midpoint => {
            let b = b?;
            if is_point(sk, a) { ConstraintKind::Midpoint { point: a, curve: b } } else { ConstraintKind::Midpoint { point: b, curve: a } }
        }
        // Live: either pick order (the line is the one that is a line).
        ConstraintTool::Normal => {
            let b = b?;
            if sk.kind(a) != Some(EntityKind::Line) && sk.kind(b) == Some(EntityKind::Line) { ConstraintKind::Normal { line: b, curve: a } } else { ConstraintKind::Normal { line: a, curve: b } }
        }
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

/// Live's words when nothing can be added (std SKETCH_ADD_CONSTRAINT_FAILED).
pub const ADD_CONSTRAINT_FAILED: &str = "Could not add constraint.";

/// A selection a constraint refuses though its kinds would fit (live 2270: Coincident on a curve's own two points).
fn constraint_refusal(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> Option<&'static str> {
    let same_curve = |a: EntityId, b: EntityId| sk.entities().any(|e| e.endpoints().is_some_and(|(s, t)| (s, t) == (a, b) || (s, t) == (b, a)));
    match (c, ids) {
        (ConstraintTool::Coincident, [a, b]) if is_point(sk, *a) && is_point(sk, *b) && same_curve(*a, *b) => {
            Some("A coincident constraint cannot be added to two points from the same curve.")
        }
        _ => None,
    }
}

/// A selection more picks could turn into constraint `c` (live 2270: Midpoint on two points, Symmetric on two lines
/// start their tool with them).
fn completes_later(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> bool {
    let all_points = ids.iter().all(|e| is_point(sk, *e));
    let need = match c {
        ConstraintTool::Midpoint if all_points => 3,
        ConstraintTool::Horizontal | ConstraintTool::Vertical if all_points => 2,
        _ => c.arity(),
    };
    ids.len() < need && ids.iter().all(|e| kind_fits(sk, c, *e))
}

/// Live's words for a selection constraint `c` can't take (live 2270; std's errorstringenum).
fn refusal_words(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> &'static str {
    let lines = ids.iter().filter(|e| sk.kind(**e) == Some(EntityKind::Line)).count();
    let points = ids.iter().filter(|e| is_point(sk, **e)).count();
    match c {
        ConstraintTool::Coincident if points == 0 => "A coincident constraint cannot be applied to two curves of different types.",
        ConstraintTool::Concentric => "A concentric constraint can only be added to circles, arcs, ellipses and points.",
        ConstraintTool::Parallel => "A parallel constraint requires two lines.",
        ConstraintTool::Perpendicular if ids.len() == 2 && lines < 2 => "A perpendicular constraint requires two lines.",
        ConstraintTool::Tangent => "A tangent constraint requires two curves, one of which must not be a line.",
        ConstraintTool::Equal => "An equal constraint can only be added between two lines or two curves.",
        ConstraintTool::Normal => "A normal constraint requires a line and a curve.",
        ConstraintTool::Horizontal | ConstraintTool::Vertical => "The entities must be all points or all lines and ellipses.",
        ConstraintTool::Midpoint => "Midpoint constraint requires a point and a line or arc.",
        ConstraintTool::Curvature => "An equal curvature constraint requires two curves one of which must be a conic or spline.",
        // Live (skasm 10:12): a circle, a spline and a point picked at once.
        ConstraintTool::Pierce if ids.len() > 2 => "A pierce constraint requires a curve or point from the sketch and an external edge.",
        _ => ADD_CONSTRAINT_FAILED,
    }
}

/// Whether constraint `c` takes an entity of `e`'s kind among its picks.
fn kind_fits(sk: &Sketch, c: ConstraintTool, e: EntityId) -> bool {
    let Some(k) = sk.kind(e) else { return false };
    let point = k == EntityKind::Point;
    match c {
        ConstraintTool::Coincident | ConstraintTool::Fix | ConstraintTool::Symmetric => true,
        ConstraintTool::Concentric => point || k.is_circular() || k.is_elliptical(),
        ConstraintTool::Parallel | ConstraintTool::Perpendicular => k == EntityKind::Line,
        ConstraintTool::Tangent | ConstraintTool::Normal | ConstraintTool::Curvature | ConstraintTool::Flow => !point,
        ConstraintTool::Equal => k == EntityKind::Line || k.is_circular(),
        ConstraintTool::Midpoint => point || matches!(k, EntityKind::Line | EntityKind::Arc),
        // A point or a curve (live: c2-surf-workshop-pipe's Loft profiles pierce their ellipse onto two guides).
        ConstraintTool::Pierce => true,
        ConstraintTool::Horizontal | ConstraintTool::Vertical => point || k == EntityKind::Line || k == EntityKind::Ellipse,
    }
}

/// Can constraint `c` be applied to the selected entities at once?
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
        ConstraintTool::Coincident => n >= 2 && (count(&point) >= 1 && (count(&point) == n || count(&point) == 1 || n == 2) || all(&line) || all(&|k| k.is_circular()) || all(&|k| k.is_elliptical())),
        ConstraintTool::Concentric => n >= 2 && all(&|k| round(k) || point(k)) && count(&round) >= 1,
        ConstraintTool::Parallel => n >= 2 && all(&line),
        ConstraintTool::Tangent => n >= 2 && all(&curve) && !all(&line),
        ConstraintTool::Horizontal | ConstraintTool::Vertical => all(&|k| line(k) || k.is_elliptical()) || (n >= 2 && all(&point)),
        ConstraintTool::Perpendicular => n == 2 && all(&line),
        ConstraintTool::Equal => n >= 2 && (all(&line) || all(&|k| k.is_circular())),
        ConstraintTool::Midpoint => (n == 2 && count(&point) == 1 && kinds.iter().any(|k| matches!(k, EntityKind::Line | EntityKind::Arc))) || (n == 3 && all(&point)),
        ConstraintTool::Normal => n == 2 && count(&line) == 1 && all(&curve),
        ConstraintTool::Pierce => n == 2 && ids.iter().filter(|e| sk.entity(**e).is_some_and(|x| x.external)).count() == 1,
        ConstraintTool::Symmetric => n == 3 && count(&line) >= 1,
        ConstraintTool::Fix => true,
        // Live 1716: a line and an arc (no spline, conic or Bezier) take
        // nothing, silently.
        ConstraintTool::Curvature | ConstraintTool::Flow => n == 2 && all(&curve) && kinds.iter().any(|k| k.is_spline()),
    }
}

/// Solve after constraint `c` was added on `ids`. Live (qa4 round 5 v3 s6,
/// CON-TANGENT-APPLY): Tangent between a line and a circle moves the
/// circle and keeps its radius, so only the circle's centre takes the change.
/// Solve after constraint `c` was added to `ids`. Live (qa4 round 5 s6 (1)):
/// a constraint the sketch can't be solved with is kept, the shape stays as
/// it was and Onshape says "Sketch could not be solved."; returns false then.
pub fn solve_after_constraint(sk: &mut Sketch, c: ConstraintTool, ids: &[EntityId]) -> bool {
    let centers: Vec<EntityId> = if c == ConstraintTool::Tangent && ids.iter().any(|e| sk.kind(*e) == Some(EntityKind::Line)) {
        ids.iter().filter(|e| sk.kind(**e) == Some(EntityKind::Circle)).filter_map(|e| sk.entity(*e).and_then(|x| x.center())).collect()
    } else {
        Vec::new()
    };
    let before = sk.clone();
    // Live (live2 11-17 part 2b): the first-picked entity stays, the second-picked moves (placed as live places it).
    place_second_line(sk, c, ids);
    level_lines(sk, c, ids);
    carry_text_box(sk, c, ids);
    mirror_second_line(sk, c, ids);
    let light = if centers.is_empty() { what_moves(sk, c, ids) } else { centers };
    let report = sk.solve_with(&noshape_sketch::SolveOptions { light, ..Default::default() });
    if !report.is_solved() {
        *sk = before;
        return false;
    }
    true
}

/// Symmetric on two lines (a lines-only mirror): the second line moves onto the first's mirror image, keeping its
/// length and direction, its start at the foot of its perpendicular there (live2 mirror-sym-1118; live re-measured
/// 11-18: B (70, -5)-(85, 15) to (81.538, 2.692)-(67.671, 23.494) for A (30, -10)-(50, 20) about x = 60).
fn mirror_second_line(sk: &mut Sketch, c: ConstraintTool, ids: &[EntityId]) {
    if c != ConstraintTool::Symmetric {
        return;
    }
    let Some((a, b, axis)) = sk.constraints().filter(|k| k.hints.lines_only).find_map(|k| match k.kind {
        ConstraintKind::Symmetric { a, b, axis } if [a, b, axis].iter().all(|e| ids.contains(e)) => Some((a, b, axis)),
        _ => None,
    }) else {
        return;
    };
    let (Some(Curve2::Line { a: a0, b: a1 }), Some(Curve2::Line { a: b0, b: b1 }), Some(Curve2::Line { a: x0, b: x1 })) = (sk.curve(a), sk.curve(b), sk.curve(axis)) else { return };
    let Some(u) = (x1 - x0).try_normalize() else { return };
    let mirror = |p: DVec2| x0 + u * (2.0 * (p - x0).dot(u)) - (p - x0);
    let (m0, m1) = (mirror(a0), mirror(a1));
    let Some(dir) = (m1 - m0).try_normalize() else { return };
    let dir = if dir.dot(b1 - b0) >= 0.0 { dir } else { -dir };
    let on = m0 + dir * (b0 - m0).dot(dir);
    let Some((s, e)) = sk.endpoints(b) else { return };
    let _ = sk.set_point(s, on);
    let _ = sk.set_point(e, on + dir * (b1 - b0).length());
}

/// A text box's corner made coincident with what can't move (a model edge or vertex, a fixed point): the whole box is
/// carried there, its size and direction kept (live c2-surf-intro-mutualtrim's Title: the text's baseline start onto
/// edge JHV, the text translated 40.46 mm; the solve alone scaled the box, its height free).
fn carry_text_box(sk: &mut Sketch, c: ConstraintTool, ids: &[EntityId]) {
    let [a, b] = ids else { return };
    if c != ConstraintTool::Coincident {
        return;
    }
    let pinned = |e: EntityId| sk.entity(e).is_some_and(|x| x.external) || sk.constraints().any(|k| k.kind == ConstraintKind::Fix { entity: e });
    let (corner, target) = if is_point(sk, *a) && pinned(*b) { (*a, *b) } else if is_point(sk, *b) && pinned(*a) { (*b, *a) } else { return };
    if pinned(corner) {
        return;
    }
    // The box: the lines of its LengthRatio (bottom, left) and the lines sharing their corners (right, top).
    let sides: Vec<EntityId> = sk.constraints().filter_map(|k| match k.kind {
        ConstraintKind::LengthRatio { a, b, .. } => Some([a, b]),
        _ => None,
    }).find(|[x, y]| [x, y].iter().any(|l| sk.endpoints(**l).is_some_and(|(s, e)| s == corner || e == corner))).map(|s| s.to_vec()).unwrap_or_default();
    if sides.is_empty() {
        return;
    }
    let mut corners: Vec<EntityId> = sides.iter().filter_map(|l| sk.endpoints(*l)).flat_map(|(s, e)| [s, e]).collect();
    let more: Vec<EntityId> = sk.entities().filter(|e| e.kind() == EntityKind::Line).filter_map(|e| sk.endpoints(e.id)).filter(|(s, e)| corners.contains(s) || corners.contains(e)).flat_map(|(s, e)| [s, e]).collect();
    corners.extend(more);
    corners.sort();
    corners.dedup();
    if corners.len() != 4 {
        return;
    }
    let Some(at) = sk.point(corner) else { return };
    let to = match (sk.point(target), sk.curve(target)) {
        (Some(q), _) => q,
        // (On a line: its whole line, as the Coincident holds it.)
        (None, Some(Curve2::Line { a, b })) if (b - a).length_squared() > 0.0 => a + (b - a) * ((at - a).dot(b - a) / (b - a).length_squared()),
        (None, Some(curve)) => curve.closest(at).1,
        _ => return,
    };
    let shift = to - at;
    for p in corners {
        if let Some(q) = sk.point(p) {
            let _ = sk.set_point(p, q + shift);
        }
    }
}

/// What a constraint from two picks moves, the rest held where it can be (live2 11-17): the second pick; a point and a
/// circle, arc or ellipse made coincident, the point, or, when the point can't move (a model vertex, a fixed point),
/// the curve's centre, so it shifts the shortest way with its radii kept.
fn what_moves(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> Vec<EntityId> {
    let [first, second] = ids else { return Vec::new() };
    if matches!(c, ConstraintTool::Horizontal | ConstraintTool::Vertical) && ids.iter().all(|e| sk.kind(*e) == Some(EntityKind::Line)) {
        return ids.to_vec();
    }
    let pinned = |p: EntityId| sk.entity(p).is_some_and(|e| e.external) || sk.constraints().any(|k| k.kind == ConstraintKind::Fix { entity: p });
    if matches!(c, ConstraintTool::Coincident | ConstraintTool::Pierce) {
        let (point, curve) = if is_point(sk, *first) { (*first, *second) } else { (*second, *first) };
        if is_point(sk, point) && !is_point(sk, curve) && let Some(center) = sk.entity(curve).and_then(|e| e.center()) {
            return vec![if pinned(point) { center } else { point }];
        }
    }
    vec![*second]
}

/// Two lines constrained, the second free of everything else: it moves as live moves it before the solve (live2
/// 11-17 part 2b, L2 picked second): Parallel turns it about its midpoint, Perpendicular about where the two lines
/// cross, Coincident lays it on the first's line (turned about its midpoint, then across), lengths kept; Equal keeps
/// its start and direction.
fn place_second_line(sk: &mut Sketch, c: ConstraintTool, ids: &[EntityId]) {
    let [first, second] = ids else { return };
    let (Some(Curve2::Line { a: a1, b: b1 }), Some(Curve2::Line { a, b })) = (sk.curve(*first), sk.curve(*second)) else { return };
    let Some((s, e)) = sk.endpoints(*second) else { return };
    // Nothing else holds it: no other constraint on it or its ends, no other curve through its ends.
    let held = sk.constraints().any(|k| {
        let on = k.kind.entities();
        on.iter().any(|x| [*second, s, e].contains(x)) && !on.contains(first)
    }) || sk.entities().any(|x| x.id != *second && x.point_refs().iter().any(|p| *p == s || *p == e));
    if held {
        return;
    }
    let (d1, d) = ((b1 - a1).normalize_or_zero(), b - a);
    if d1 == DVec2::ZERO || d.length() < 1e-12 {
        return;
    }
    // The direction along `want` (either way) nearest the line's own.
    let toward = |want: DVec2| if want.dot(d) >= 0.0 { want } else { -want };
    let turned = |pivot: DVec2, dir: DVec2| {
        let (from, to) = (d.normalize(), dir);
        let rot = DVec2::new(from.dot(to), from.perp_dot(to));
        (pivot + (a - pivot).rotate(rot), pivot + (b - pivot).rotate(rot))
    };
    let mid = (a + b) * 0.5;
    let (na, nb) = match c {
        ConstraintTool::Parallel => turned(mid, toward(d1)),
        ConstraintTool::Perpendicular => {
            let dir = toward(d1.perp());
            let den = d1.perp_dot(d);
            let pivot = if den.abs() > 1e-12 { a1 + d1 * ((a - a1).perp_dot(d) / den) } else { mid };
            turned(pivot, dir)
        }
        ConstraintTool::Coincident => {
            let (ta, tb) = turned(mid, toward(d1));
            let across = d1.perp() * d1.perp().dot(a1 - mid);
            (ta + across, tb + across)
        }
        ConstraintTool::Equal => (a, a + d.normalize() * (b1 - a1).length()),
        _ => return,
    };
    let _ = sk.set_point(s, na);
    let _ = sk.set_point(e, nb);
}

/// Horizontal or Vertical on lines (live2 11-17 part 2b: one constraint each): each line free of anything else turns
/// level or plumb about its midpoint, its length kept.
fn level_lines(sk: &mut Sketch, c: ConstraintTool, ids: &[EntityId]) {
    let want = match c {
        ConstraintTool::Horizontal => DVec2::X,
        ConstraintTool::Vertical => DVec2::Y,
        _ => return,
    };
    for &l in ids {
        let (Some(Curve2::Line { a, b }), Some((s, e))) = (sk.curve(l), sk.endpoints(l)) else { continue };
        let own = |k: &ConstraintKind| matches!(k, ConstraintKind::Horizontal { .. } | ConstraintKind::Vertical { .. }) && k.entities().as_slice() == [l];
        let held = sk.constraints().any(|k| !own(&k.kind) && k.kind.entities().iter().any(|x| [l, s, e].contains(x)))
            || sk.entities().any(|x| x.id != l && x.point_refs().iter().any(|p| *p == s || *p == e));
        let d = b - a;
        if held || d.length() < 1e-12 {
            continue;
        }
        let half = if want.dot(d) >= 0.0 { want } else { -want } * d.length() * 0.5;
        let mid = (a + b) * 0.5;
        let _ = sk.set_point(s, mid - half);
        let _ = sk.set_point(e, mid + half);
    }
}

/// Whether constraint tool `c` takes `e` after `picks` (live: it doesn't
/// accept a pick it can't use): the full set must be applicable; earlier
/// picks only need a kind the constraint takes.
pub fn pick_fits(sk: &Sketch, c: ConstraintTool, picks: &[EntityId], e: EntityId) -> bool {
    let Some(k) = sk.kind(e) else { return false };
    let mut ids = picks.to_vec();
    ids.push(e);
    let point = k == EntityKind::Point;
    let complete = match c {
        // One line, or two points.
        ConstraintTool::Horizontal | ConstraintTool::Vertical => !(point && ids.len() == 1),
        _ => ids.len() >= c.arity(),
    };
    if complete {
        return constraint_applicable(sk, c, &ids);
    }
    kind_fits(sk, c, e)
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
/// Returns the number of constraints applied: added, or there already (not added again).
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
        ConstraintTool::Symmetric | ConstraintTool::Perpendicular | ConstraintTool::Normal | ConstraintTool::Pierce | ConstraintTool::Curvature | ConstraintTool::Flow => {
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
    // One the sketch already has isn't added again, but counts as applied (live, ui-parity sketch-horizontal-key: H on
    // a horizontal line adds nothing and clears the selection).
    for k in kinds {
        if sk.constraints().any(|c| c.kind == k) {
            n += 1;
            continue;
        }
        // Live (live2 mirror-sym-1118): Symmetric on two lines is one MIRROR of the lines, their ends free.
        let lines = matches!(k, ConstraintKind::Symmetric { a, b, .. } if [a, b].iter().all(|e| sk.kind(*e) == Some(EntityKind::Line)));
        match sk.add_constraint(k) {
            Ok(id) => {
                n += 1;
                if lines {
                    let _ = sk.mirror_lines_only(id);
                }
            }
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    match (n, last_err) {
        (0, Some(e)) => Err(e),
        (n, _) => Ok(n),
    }
}

/// A sketch selection as the readout measures it.
#[derive(Clone, Copy)]
enum Pick {
    Point(DVec2),
    Segment(DVec2, DVec2),
    Round(Round),
}

/// A circle, or an arc from `start` sweeping `sweep` counter-clockwise.
#[derive(Clone, Copy)]
struct Round {
    center: DVec2,
    radius: f64,
    start: f64,
    sweep: f64,
}

fn segment_distance(p: DVec2, l0: DVec2, l1: DVec2) -> f64 {
    let d = l1 - l0;
    let t = if d.length_squared() > 0.0 { ((p - l0).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (l0 + d * t - p).length()
}

fn min_max(ds: &[f64]) -> (f64, f64) {
    ds.iter().fold((f64::INFINITY, 0.0_f64), |(lo, hi), d| (lo.min(*d), hi.max(*d)))
}

impl Round {
    fn holds(&self, angle: f64) -> bool {
        self.sweep >= TAU || noshape_sketch::geom::angle_in_range(angle, self.start, self.sweep, 1e-12).is_some()
    }

    fn ends(&self) -> Vec<DVec2> {
        if self.sweep >= TAU {
            return Vec::new();
        }
        [self.start, self.start + self.sweep].iter().map(|a| self.center + DVec2::from_angle(*a) * self.radius).collect()
    }

    /// The nearest and farthest distances from `p` to the curve.
    fn extremes_from(&self, p: DVec2) -> (f64, f64) {
        let d = (p - self.center).length();
        if d < 1e-12 {
            return (self.radius, self.radius);
        }
        let toward = (p - self.center).to_angle();
        let mut ds: Vec<f64> = self.ends().iter().map(|e| (*e - p).length()).collect();
        if self.holds(toward) {
            ds.push((d - self.radius).abs());
        }
        if self.holds(toward + PI) {
            ds.push(d + self.radius);
        }
        min_max(&ds)
    }

    /// The nearest and farthest distances between the segment `l0`–`l1` and the curve.
    fn extremes_from_segment(&self, l0: DVec2, l1: DVec2) -> (f64, f64) {
        let (e0, e1) = (self.extremes_from(l0), self.extremes_from(l1));
        let mut ds = vec![e0.0, e1.0];
        ds.extend(self.ends().iter().map(|e| segment_distance(*e, l0, l1)));
        let line = l1 - l0;
        if let Some(u) = line.try_normalize() {
            let t = (self.center - l0).dot(u);
            let foot = l0 + u * t;
            let h = (foot - self.center).length();
            let normal = if h > 1e-12 { (foot - self.center) / h } else { u.perp() };
            if (0.0..=line.length()).contains(&t) && self.holds(normal.to_angle()) {
                ds.push((h - self.radius).abs());
            }
            // Where the segment crosses the curve.
            if h <= self.radius {
                let half = (self.radius * self.radius - h * h).sqrt();
                for s in [t - half, t + half] {
                    if (0.0..=line.length()).contains(&s) && self.holds((l0 + u * s - self.center).to_angle()) {
                        ds.push(0.0);
                    }
                }
            }
        }
        (min_max(&ds).0, e0.1.max(e1.1))
    }

    /// The nearest and farthest distances between two circles or arcs.
    fn extremes_from_round(&self, other: &Round) -> (f64, f64) {
        let mut ds = Vec::new();
        for (from, to) in [(self, other), (other, self)] {
            for e in from.ends() {
                let (a, b) = to.extremes_from(e);
                ds.extend([a, b]);
            }
        }
        let between = other.center - self.center;
        let cd = between.length();
        let (r1, r2) = (self.radius, other.radius);
        if cd > 1e-12 {
            // Inside both curves the extremes lie on the line through the centres.
            let u = between / cd;
            for s1 in [1.0, -1.0] {
                for s2 in [1.0, -1.0] {
                    if self.holds((u * s1).to_angle()) && other.holds((u * s2).to_angle()) {
                        ds.push((self.center + u * s1 * r1 - other.center - u * s2 * r2).length());
                    }
                }
            }
            // Where they cross.
            if cd <= r1 + r2 && cd >= (r1 - r2).abs() {
                let along = (cd * cd + r1 * r1 - r2 * r2) / (2.0 * cd);
                let off = (r1 * r1 - along * along).max(0.0).sqrt();
                for side in [1.0, -1.0] {
                    let x = self.center + u * along + u.perp() * off * side;
                    if self.holds((x - self.center).to_angle()) && other.holds((x - other.center).to_angle()) {
                        ds.push(0.0);
                    }
                }
            }
        } else {
            // Concentric: a shared direction gives the radii's difference, opposite ones their sum.
            if self.holds(other.start) || other.holds(self.start) {
                ds.push((r1 - r2).abs());
            }
            if self.holds(other.start + PI) || other.holds(self.start + PI) {
                ds.push(r1 + r2);
            }
        }
        min_max(&ds)
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
    /// The box selection being dragged: its press and the pointer, on the plane (the app draws it as the Part Studio's).
    pub fn box_ends(&self) -> Option<(DVec2, DVec2)> {
        match (&self.drag, self.box_end) {
            (Some(DragState::Box { start }), Some(end)) => Some((*start, end)),
            _ => None,
        }
    }

    /// A box selection is being dragged.
    pub fn box_dragging(&self) -> bool {
        matches!(self.drag, Some(DragState::Box { .. }))
    }

    /// Drop a box selection that never moved (a Ctrl+click opens the menu instead).
    pub fn cancel_box(&mut self) {
        if self.box_dragging() {
            self.drag = None;
            self.box_end = None;
            self.model_press = None;
        }
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
        // Even after a rest on its midpoint (live's 02:24-02:40 re-recordings; 1937 / 1954 saw nothing selected).
        click(&mut ed, &mut d, v(0.02, 0.01));
        assert!(!ed.selection.is_empty(), "the line is selected");
        assert_eq!(ed.enter_target(), EnterTarget::Nothing, "a selection");
        assert!(!ed.cancel(), "Esc with a selection does nothing");
        assert!(!ed.selection.is_empty(), "and keeps it");
    }

    /// Live (behaviours "skasm (2026-10-27) where the live Ø readout
    /// sits"): circles and both polygons put it halfway to the cursor when
    /// it fits, otherwise outside: across the centre for a horizontal
    /// cursor, straight below the shape for a diagonal one.
    #[test]
    fn circle_and_polygon_readout_sits_halfway_or_outside_as_live() {
        let u = UnitPrefs::default();
        for tool in [Tool::Circle, Tool::Polygon, Tool::CircumPolygon] {
            let (mut ed, mut d) = editor(tool);
            let o = v(0.05, 0.05);
            click(&mut ed, &mut d, o);
            let at = |ed: &mut SketchEditor, d: &mut SketchData, p: DVec2| {
                ed.pointer_move(d, p, false, false);
                (ed.preview_values(d, &u)[0].0 - o) / PX
            };
            let mid = at(&mut ed, &mut d, o + v(150.0 * PX, 0.0));
            assert!((mid - v(75.0, 0.0)).length() < 1e-6, "{tool:?} {mid}");
            let left = at(&mut ed, &mut d, o + v(40.0 * PX, 0.0));
            assert!(left.x < -44.0 && left.y.abs() < 1e-6, "{tool:?} {left}");
            let below = at(&mut ed, &mut d, o + v(-28.0 * PX, 28.0 * PX));
            assert!(below.x.abs() < 1e-6 && (below.y + 39.6 + 22.0).abs() < 0.5, "{tool:?} {below}");
        }
    }

    /// Live (behaviours "dx (2026-10-26, for skasm) Point tool on the
    /// Origin"): the Point tool clicked on the Origin makes a sketch point
    /// there, Coincident with the Origin (a Hole can then use it).
    #[test]
    fn point_tool_on_the_origin_makes_a_point_coincident_with_it() {
        let (mut ed, mut d) = editor(Tool::Point);
        click(&mut ed, &mut d, v(0.0, 0.0));
        let points: Vec<EntityId> = d.sketch.entities().filter(|e| e.kind() == EntityKind::Point && !e.external).map(|e| e.id).collect();
        assert_eq!(points.len(), 1, "one new sketch point");
        assert!(d.sketch.point(points[0]).unwrap().length() < 1e-12);
        let origin = d.sketch.entities().find(|e| e.external).map(|e| e.id).expect("the Origin stand-in");
        assert!(d.sketch.constraints().any(|c| c.kind == ConstraintKind::Coincident { a: points[0], b: origin }), "Coincident with the Origin");
    }

    /// Live (qa4 round 5 s6 (1)): V on a Horizontal line is added anyway,
    /// the line keeps its shape and Onshape says "Sketch could not be
    /// solved.".
    #[test]
    fn vertical_on_a_horizontal_line_does_not_solve() {
        let (mut ed, mut d) = editor(Tool::Select);
        let a = d.sketch.add_point(v(0.0, 0.0));
        let b = d.sketch.add_point(v(0.03, 0.0));
        let line = d.sketch.add_line_between(a, b).unwrap();
        d.sketch.add_constraint(ConstraintKind::Horizontal { a: line, b: None }).unwrap();
        ed.selection = vec![line];
        assert!(ed.constraint_command(&mut d, ConstraintTool::Vertical));
        assert_eq!(ed.toast.as_deref(), Some(SKETCH_NOT_SOLVED));
        assert!((d.sketch.point(a).unwrap() - v(0.0, 0.0)).length() < 1e-12 && (d.sketch.point(b).unwrap() - v(0.03, 0.0)).length() < 1e-12, "the line stays");
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Vertical { .. })), "the constraint is kept");
    }

    /// Live 1785: a corner rectangle's width typed in the editor (40 -> 50)
    /// scales the whole rectangle ×1.25 about its centre, the undimensioned
    /// height too.
    #[test]
    fn a_width_typed_in_the_editor_scales_the_rectangle() {
        let (mut ed, mut d) = editor(Tool::CornerRect);
        ed.apply_unit_defaults(noshape_units::LengthUnit::Millimeter);
        click(&mut ed, &mut d, v(0.010, 0.010));
        click(&mut ed, &mut d, v(0.050, 0.030));
        let bottom = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).find_map(|e| {
            let c = d.sketch.curve(e.id)?;
            ((c.start_point().y - 0.010).abs() < 1e-9 && (c.end_point().y - 0.010).abs() < 1e-9).then_some(e.id)
        }).expect("the bottom");
        let len = d.sketch.add_constraint(ConstraintKind::Length { curve: bottom, value: 0.04 }).unwrap();
        assert!(d.sketch.solve().is_solved());
        ed.apply_dimension(&mut d, len, "50", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
        for e in d.sketch.entities().filter(|e| e.kind() == EntityKind::Point && !e.external && !e.construction) {
            let p = d.sketch.point(e.id).unwrap();
            lo = lo.min(p);
            hi = hi.max(p);
        }
        assert!((lo - v(0.005, 0.0075)).length() < 1e-9 && (hi - v(0.055, 0.0325)).length() < 1e-9, "{lo:?} {hi:?}");
    }

    /// Live (psb 10-24 j (c), qa OFFSET-EDIT-SCALES-SKETCH): a free circle r5 offset 5 mm, its distance typed 3: no
    /// scaling (the offset's distance is no first dimension); the copy keeps r10, the free source moves to r7.
    #[test]
    fn an_offsets_new_distance_moves_the_source_and_scales_nothing() {
        let (mut ed, mut d) = editor(Tool::Select);
        let source = d.sketch.add_circle(v(0.0, 0.0), 0.005);
        let r = d.sketch.offset(&[source], -0.005).unwrap();
        assert!(d.sketch.solve().is_solved());
        let radius = |d: &SketchData, e: EntityId| match d.sketch.curve(e) {
            Some(Curve2::Circle { radius, .. }) => radius,
            c => panic!("{c:?}"),
        };
        let copy = r.created.iter().copied().find(|e| d.sketch.kind(*e) == Some(EntityKind::Circle)).expect("the copy");
        let dist = r.constraints_added.iter().copied().find(|c| matches!(d.sketch.constraint(*c).map(|c| &c.kind), Some(ConstraintKind::Distance { .. }))).expect("its distance");
        assert!((radius(&d, copy) - 0.010).abs() < 1e-9, "the copy r10 before: {}", radius(&d, copy));
        ed.apply_dimension(&mut d, dist, "3", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        assert_eq!(((radius(&d, source) * 1e6).round() / 1e3, (radius(&d, copy) * 1e6).round() / 1e3), (7.0, 10.0), "source, copy radius (mm)");
    }

    /// Live (qa sellat-1118): a horizontal line off the origin with only its length, 40 -> 50 typed in the editor,
    /// moves both ends equally (start -5, end +5 mm); the label stays where it was, on the unchanged midpoint.
    #[test]
    fn a_free_lines_new_length_moves_both_ends_equally() {
        let (mut ed, mut d) = editor(Tool::Select);
        let line = d.sketch.add_line(v(0.002, 0.003), v(0.042, 0.003));
        d.sketch.add_constraint(ConstraintKind::Horizontal { a: line, b: None }).unwrap();
        let len = d.sketch.add_constraint(ConstraintKind::Length { curve: line, value: 0.04 }).unwrap();
        assert!(d.sketch.solve().is_solved());
        d.dimension_labels.insert(len.0, [0.022, 0.008]);
        ed.apply_dimension(&mut d, len, "50", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        let c = d.sketch.curve(line).unwrap();
        assert!((c.start_point() - v(-0.003, 0.003)).length() < 1e-9 && (c.end_point() - v(0.047, 0.003)).length() < 1e-9, "{c:?}");
        assert_eq!(d.dimension_labels.get(&len.0), Some(&[0.022, 0.008]));
        ed.apply_dimension(&mut d, len, "40", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        let c = d.sketch.curve(line).unwrap();
        assert!((c.start_point() - v(0.002, 0.003)).length() < 1e-9 && (c.end_point() - v(0.042, 0.003)).length() < 1e-9, "back: {c:?}");
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
        assert!(!o.show_constraints && o.show_errors && !o.show_expressions);
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

    /// Live (feel round 2 R1, cov2-lastpoint): a second click on the chain's last point ends the chain, no
    /// zero-length line; the tool stays and the next click starts a new line.
    #[test]
    fn a_second_click_on_the_last_point_ends_the_chain() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.03, 0.0));
        click(&mut ed, &mut d, v(0.03, 0.0));
        assert!(ed.chain_end.is_none() && ed.clicks.is_empty() && ed.tool == Tool::Line);
        click(&mut ed, &mut d, v(0.05, 0.03));
        assert_eq!(count(&d, EntityKind::Line), 1, "the next click only starts a line");
        click(&mut ed, &mut d, v(0.06, 0.01));
        assert_eq!(count(&d, EntityKind::Line), 2);
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

    /// Live 2025 (b): an ellipse's boxes after its 3rd click, the major active; Enter dimensions the active box at
    /// its value and moves to the other; once a box is a dimension, Enter on the other, untouched one does nothing.
    #[test]
    fn ellipse_boxes_major_first_and_one_enter_after_a_typed_box() {
        let ellipse_dims = |d: &SketchData| d.sketch.constraints().filter_map(|c| match c.kind { ConstraintKind::EllipseRadius { major, .. } => Some(major), _ => None }).collect::<Vec<_>>();
        // b1: Tab to the minor, type 12, Enter, Enter.
        let (mut ed, mut d) = editor(Tool::Ellipse);
        for p in [v(0.0, 0.0), v(0.03, 0.0), v(0.0, 0.012)] {
            click(&mut ed, &mut d, p);
        }
        let o = ed.otf.as_ref().expect("the boxes");
        assert!(matches!(o.fields[o.active].kind, ConstraintKind::EllipseRadius { major: true, .. }), "the major box first");
        ed.otf_step(1);
        ed.commit_otf(&mut d, "12", &UnitPrefs::default(), &noshape_units::EmptyScope).unwrap();
        assert_eq!(ellipse_dims(&d), vec![false]);
        assert!(ed.accept_otf(&mut d).unwrap(), "Enter taken");
        assert_eq!(ellipse_dims(&d), vec![false], "no major dimension");
        // b2: Enter on the untouched major, then the minor is active.
        let (mut ed, mut d) = editor(Tool::Ellipse);
        for p in [v(0.0, 0.0), v(0.03, 0.0), v(0.0, 0.012)] {
            click(&mut ed, &mut d, p);
        }
        assert!(ed.accept_otf(&mut d).unwrap());
        assert_eq!(ellipse_dims(&d), vec![true]);
        let o = ed.otf.as_ref().expect("the minor box");
        assert!(matches!(o.fields[o.active].kind, ConstraintKind::EllipseRadius { major: false, .. }));
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
        assert_eq!(ed.selection, vec![line], "the picked line stays selected meanwhile (live sketch-slot-on-line)");
        click(&mut ed, &mut d, v(0.03, 0.04));
        assert!(!ed.slot_pending());
        assert!(ed.selection.is_empty(), "committing clears it");
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

    /// qa SKETCH-SLOT-ON-ARC: the Slot tool on a free 3-point arc wraps it in a slot of the tool's width (live behaviours 164); Esc
    /// commits it with the arc kept (ours drew "Ø0", no slot, and Esc left the arc's centre and a Ø0 dimension).
    #[test]
    fn a_slot_on_a_free_arc_wraps_it_and_esc_keeps_the_arc() {
        let (mut ed, mut d) = editor(Tool::Slot);
        let c = v(0.03, 0.03);
        let arc = d.sketch.add_arc(c, c + v(0.0227, 0.0), c + v(0.0, 0.0227));
        click(&mut ed, &mut d, c + DVec2::from_angle(std::f64::consts::FRAC_PI_4) * 0.0227);
        let (_, _, w) = ed.slot_dimension().expect("the preview shows its Ø dimension");
        assert!((w - ed.slot_width).abs() < 1e-12, "{w}");
        assert!(ed.escape_commit(&mut d).is_some_and(|o| o.changed));
        let r = |d: &SketchData, e: EntityId| d.sketch.curve(e).map(|k| (k.start_point() - c).length());
        assert!(r(&d, arc).is_some_and(|r| (r - 0.0227).abs() < 1e-9), "the arc kept: {:?}", r(&d, arc));
        assert_eq!(count(&d, EntityKind::Arc), 5, "the arc, two sides and two caps");
        let dia: Vec<f64> = d.sketch.constraints().filter_map(|k| match k.kind { ConstraintKind::Diameter { value, .. } => Some(value), _ => None }).collect();
        assert!(dia.len() == 1 && (dia[0] - ed.slot_width).abs() < 1e-12, "{dia:?}");
        // (The solid arc splits the slot in two: a quarter annulus and the two half-disc caps.)
        let area: f64 = d.sketch.regions().iter().map(|g| g.area()).sum();
        let h = ed.slot_width / 2.0;
        let expect = std::f64::consts::FRAC_PI_4 * ((0.0227 + h).powi(2) - (0.0227 - h).powi(2)) + std::f64::consts::PI * h * h;
        assert!((area - expect).abs() < 1e-8, "{area} vs {expect}");
    }

    /// Live (sketch-linear-pattern-click-commit, sketch-circular-pattern-click-commit): a click in white space accepts
    /// the pattern and ends the tool, nothing left selected.
    #[test]
    fn a_pattern_accepted_in_white_space_ends_the_tool() {
        for tool in [Tool::LinearPattern, Tool::CircularPattern] {
            let (mut ed, mut d) = editor(tool);
            let line = d.sketch.add_line(v(0.01, 0.01), v(0.02, 0.01));
            ed.selection.push(line);
            ed.pattern_center = v(0.0, 0.0);
            let before = d.sketch.entities().count();
            click(&mut ed, &mut d, v(0.2, 0.2));
            assert!(d.sketch.entities().count() > before, "{tool:?}: the pattern added its instances");
            assert_eq!(ed.tool, Tool::Select, "{tool:?}: the tool ended");
            assert!(ed.selection.is_empty(), "{tool:?}: nothing selected");
        }
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
    fn pattern_count_labels_sit_where_live_puts_them() {
        // Live sketch-gaps3: a linear "3x" (+97, +99.8) px from the 2nd instance at the zoom the tool started at, then
        // fixed in the model; a circular one 0.4 × the seed's radius beyond the seed.
        let (mut ed, mut d) = editor(Tool::Select);
        ed.world_per_px = 1.778e-4;
        let circle = d.sketch.add_circle(v(0.01, 0.05), 0.003);
        ed.set_tool(Tool::LinearPattern);
        ed.selection.push(circle);
        let label = |ed: &SketchEditor, d: &SketchData| ed.pattern_count_labels(&d.sketch)[0].1;
        let want = v(0.01 + ed.pattern_distance + 97.0 * 1.778e-4, 0.05 - 99.8 * 1.778e-4);
        assert!((label(&ed, &d) - want).length() < 1e-9, "{:?} vs {want:?}", label(&ed, &d));
        ed.world_per_px = 1.495e-4;
        assert!((label(&ed, &d) - want).length() < 1e-9, "a zoom after the start keeps it in the model");
        ed.set_tool(Tool::CircularPattern);
        ed.selection = vec![circle];
        ed.pattern_center = v(0.0, 0.0);
        assert!((label(&ed, &d) - v(0.014, 0.07)).length() < 1e-9, "{:?}", label(&ed, &d));
        // Clockwise, as live (sketch-gaps5): closed 3x, the arc's end on the first instance after the seed (240°
        // round); open 4x at 180°, instances 60° apart clockwise from the seed at 78.69°.
        let (r, a0) = (v(0.01, 0.05).length(), v(0.01, 0.05).to_angle());
        let end = |ed: &SketchEditor, d: &SketchData| ed.circular_handle_arc(&d.sketch).unwrap().2;
        assert!((end(&ed, &d) - (a0 - TAU * 2.0 / 3.0)).abs() < 1e-9, "{}", end(&ed, &d).to_degrees());
        // Below 360°: the angle on its dimension arc at 1.4 × the radius, ~0.64 of the way (live sketch-gaps4).
        assert!(ed.pattern_value_labels(&d.sketch).is_empty(), "no angle at 360°");
        ed.set_pattern_value(PatternHandle::CircularAngle, PI);
        ed.set_pattern_count(PatternHandle::CircularCount, 4);
        assert!((end(&ed, &d) - (a0 - PI)).abs() < 1e-9 && (ed.circular_step().0 + PI / 3.0).abs() < 1e-9, "60° steps clockwise");
        let (h, at, shown) = ed.pattern_value_labels(&d.sketch)[0];
        assert!(h == PatternHandle::CircularAngle && (shown - PI).abs() < 1e-12, "shown as 180°");
        assert!((at - DVec2::from_angle(a0 - PI * 0.64) * r * 1.4).length() < 1e-9, "{at:?}");
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

    /// Live editing-gaps3 (the client's triad): the 2D manipulator sits on the selection's lowest key point, the
    /// leftmost of equals; its handles are screen-sized; an arrow moves the selection along its axis only.
    #[test]
    fn transform_triad_sits_and_moves_as_live() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let anchor = |make: &dyn Fn(&mut SketchData) -> Vec<EntityId>| {
            let (mut ed, mut d) = editor(Tool::Select);
            ed.selection = make(&mut d);
            ed.set_tool(Tool::Transform);
            ed.apply_preselection(&mut d);
            ed.pattern_center / 1e-3
        };
        let near = |a: DVec2, b: DVec2| (a - b).length() < 1e-6;
        assert!(near(anchor(&|d| vec![d.sketch.add_line(mm(40., 80.), mm(20., 65.))]), v(20., 65.)), "a right-to-left line: its end");
        let tri = anchor(&|d| {
            let t0 = d.sketch.add_line(mm(60., 10.), mm(80., 10.));
            let t1 = d.sketch.add_line(mm(80., 10.), mm(70., 25.));
            let t2 = d.sketch.add_line(mm(70., 25.), mm(60., 10.));
            vec![t2, t0, t1]
        });
        assert!(near(tri, v(60., 10.)), "a triangle picked T2, T0, T1: {tri}");
        assert!(near(anchor(&|d| vec![d.sketch.add_circle(mm(-30., 40.), 5e-3)]), v(-30., 40.)), "a circle: its centre");
        // An arc 0..120° about (-30, 0) r 6: its start (-24, 0) ties the centre's Y; the centre is further left.
        let arc = anchor(&|d| vec![d.sketch.add_arc_angles(mm(-30., 0.), 6e-3, 0.0, TAU / 3.0)]);
        assert!(near(arc, v(-30., 0.)), "an arc: its centre: {arc}");

        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(mm(0., 0.), mm(10., 0.));
        ed.selection = vec![l];
        ed.set_tool(Tool::Transform);
        ed.apply_preselection(&mut d);
        let px = ed.world_per_px;
        let at = |ed: &SketchEditor, d: &SketchData, h: PatternHandle| ed.pattern_handles(&d.sketch).into_iter().find(|x| x.0 == h).unwrap().1 / px;
        assert!(near(at(&ed, &d, PatternHandle::MoveX), v(82., 0.)) && near(at(&ed, &d, PatternHandle::MoveY), v(0., 82.)), "the arrows' heads 68..96 px out");
        assert!(near(at(&ed, &d, PatternHandle::Move), v(32.5, 32.5)), "the square 20..45 px");
        assert!(near(at(&ed, &d, PatternHandle::Rotate), v(108., 0.)), "the rotation ring 108 px along X");
        assert!(near(at(&ed, &d, PatternHandle::Scale), v(1., 1.).normalize() * 99.85), "the scale head from (60, 60) px");
        assert_eq!(ed.handle_at(&d.sketch, v(90., 4.) * px), Some(PatternHandle::MoveX), "a press on the X head takes it");
        assert_eq!(ed.handle_at(&d.sketch, v(40., 3.) * px), Some(PatternHandle::MoveX), "and one on its shaft, over the line");
        assert_eq!(ed.handle_at(&d.sketch, v(3., 40.) * px), Some(PatternHandle::MoveY), "the Y shaft too");
        assert_eq!(ed.handle_at(&d.sketch, v(40., 9.) * px), None, "6 px either side of a shaft");
        ed.drag_handle(&d.sketch.clone(), PatternHandle::MoveX, v(82. + 50., 30.) * px);
        assert!(near(ed.xf_offset / px, v(50., 0.)), "the X arrow moves along X only: {}", ed.xf_offset / px);
        ed.drag_handle(&d.sketch.clone(), PatternHandle::Center, mm(5., 5.));
        assert!(near(ed.xf_offset / px, v(50., 0.)) && near((ed.pattern_center + ed.xf_offset) / 1e-3, v(5., 5.)), "the ring moves the triad only");
    }

    /// Live sk2-1121 transform: an arrow's drag shows the distance moved in a box up and right of the pointer, which
    /// stays where the drag left it and opens for typing; a typed distance sets the move along that arrow.
    #[test]
    fn transform_arrow_drag_leaves_its_distance_box() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.0, 0.0), v(0.01, 0.0));
        ed.selection = vec![l];
        ed.set_tool(Tool::Transform);
        ed.apply_preselection(&mut d);
        let px = ed.world_per_px;
        let head = ed.pattern_handles(&d.sketch).into_iter().find(|x| x.0 == PatternHandle::MoveY).unwrap().1;
        let to = head + v(0.0, 40.0) * px;
        ed.tool_down(&mut d, head, None, false, false);
        ed.tool_drag(&mut d, to);
        let label = |ed: &SketchEditor, d: &SketchData| ed.pattern_value_labels(&d.sketch).into_iter().find(|x| x.0 == PatternHandle::MoveY);
        let (_, at, shown) = label(&ed, &d).expect("the box during the drag");
        assert!((at - (to + v(72.0, -48.0) * px)).length() < 1e-9 && (shown / px - 40.0).abs() < 1e-6, "{at:?} {shown}");
        ed.tool_up(&mut d, to);
        assert_eq!(ed.xf_released, Some(PatternHandle::MoveY), "the view opens it for typing");
        let (_, kept, _) = label(&ed, &d).expect("the box after the drag");
        assert!((kept - at).length() < 1e-9, "where the drag left it");
        assert!(label(&ed, &d).is_some() && !ed.pattern_value_labels(&d.sketch).iter().any(|x| x.0 == PatternHandle::MoveX), "the X arrow's not shown");
        ed.set_pattern_value(PatternHandle::MoveY, 0.005);
        assert!((ed.xf_offset - v(0.0, 0.005)).length() < 1e-12, "typed: 5 mm along Y: {:?}", ed.xf_offset);
        // A press on the shaft, short of the head, dragged 40 px moves 40 px (sk2-1121 transform-drag-copy: 9.9 mm).
        let (mut ed, mut d) = editor(Tool::Select);
        ed.selection = vec![d.sketch.add_line(v(0.0, 0.0), v(0.01, 0.0))];
        ed.set_tool(Tool::Transform);
        ed.apply_preselection(&mut d);
        let shaft = v(-2.0, 40.0) * px;
        ed.tool_down(&mut d, shaft, None, false, false);
        ed.tool_drag(&mut d, shaft + v(1.0, 20.0) * px);
        ed.tool_drag(&mut d, shaft + v(1.0, 40.0) * px);
        ed.tool_up(&mut d, shaft + v(1.0, 40.0) * px);
        assert!((ed.xf_offset / px - v(0.0, 40.0)).length() < 1e-6, "the pointer's travel along Y: {:?}", ed.xf_offset / px);
    }

    /// live2's combined snaps (feel/combo.json, 0.2507 mm a pixel, mm here): a chain's next end where two zones cross
    /// near the pointer takes both constraints; of several crossings the nearest the pointer.
    #[test]
    fn crossing_zones_combine_as_live() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let px = 0.2507e-3;
        // seg1 (10, 5) -> (25, 15) unless given, chained on; `woken` points woken; the pointer at `at`: the snap's constraints and point.
        let probe_from = |seg1: (DVec2, DVec2), setup: &dyn Fn(&mut SketchData) -> Vec<EntityId>, at: DVec2| {
            let (mut ed, mut d) = editor(Tool::Line);
            ed.world_per_px = px;
            let woken = setup(&mut d);
            click(&mut ed, &mut d, seg1.0);
            click(&mut ed, &mut d, seg1.1);
            for w in woken {
                ed.wake(w);
            }
            ed.pointer_move(&mut d, at, false, false);
            let seg1 = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).map(|e| e.id).max().unwrap();
            let inf = ed.inference.clone().unwrap_or_else(|| panic!("no snap at {at:?}"));
            (inf, seg1, d)
        };
        let probe = |setup: &dyn Fn(&mut SketchData) -> Vec<EntityId>, at: DVec2| probe_from((mm(10.0, 5.0), mm(25.0, 15.0)), setup, at);
        // Square to seg1 from (25, 15), up and left.
        let up = v(-10.0, 15.0).normalize();
        let square_at = |y: f64| mm(25.0, 15.0) + up * ((y - 15.0) * 1e-3 / up.y);
        // perpAlign: the square and the horizontal through a woken (5, 24): both, the alignment's first. (2 px along it:
        // headless, a direction's reach is 3 deg.)
        let crossing = square_at(24.0);
        let (inf, seg1, d) = probe(&|d| { let l = d.sketch.add_line(mm(2.0, 24.0), mm(5.0, 24.0)); vec![d.sketch.endpoints(l).unwrap().1] }, crossing + v(2.0 * px, 0.0));
        let w = d.sketch.entities().filter(|e| e.kind() == EntityKind::Point).find(|e| d.sketch.point(e.id).is_some_and(|q| (q - mm(5.0, 24.0)).length() < 1e-9)).unwrap().id;
        assert_eq!(inf.constraints, vec![AutoConstraint::HorizontalWith(w), AutoConstraint::PerpendicularTo(seg1)], "{inf:?}");
        assert!((inf.position - crossing).length() < 1e-9);
        // alignCurve (seg1 (20, 8) -> (25, 12)): the vertical through a woken (31, 28) and a circle: on the circle
        // there, vertical with the point.
        let on_circle = mm(31.0, 20.0 - 12f64.sqrt());
        let (inf, _, d) = probe_from((mm(20.0, 8.0), mm(25.0, 12.0)), &|d| { d.sketch.add_circle(mm(33.0, 20.0), 4e-3); let l = d.sketch.add_line(mm(24.0, 28.0), mm(31.0, 28.0)); vec![d.sketch.endpoints(l).unwrap().1] }, on_circle + v(3.0 * px, 0.0));
        let circle = d.sketch.entities().find(|e| e.kind() == EntityKind::Circle).unwrap().id;
        assert!(inf.constraints.contains(&AutoConstraint::OnCurve(circle)) && inf.constraints.iter().any(|c| matches!(c, AutoConstraint::VerticalWith(_))), "{inf:?}");
        assert!((inf.position - on_circle).length() < 1e-9);
        // three: the square, a line along y = 30 and the vertical through a woken (15, 20): the crossing nearest the
        // pointer (on the line at the square's crossing; the square and the vertical's 1.3 px from it).
        let three = |d: &mut SketchData| { d.sketch.add_line(mm(0.0, 30.0), mm(40.0, 30.0)); let l = d.sketch.add_line(mm(12.0, 20.0), mm(15.0, 20.0)); vec![d.sketch.endpoints(l).unwrap().1] };
        // (seg1 where live's clicks put it: from (10, 5) to (25, 15) all three would meet at (15, 30).)
        let (s0, s1) = (mm(9.9703, 5.221), mm(24.7641, 15.2507));
        let up = (s1 - s0).perp().normalize();
        let (on_line, on_vertical) = (s1 + up * ((30e-3 - s1.y) / up.y), s1 + up * ((15e-3 - s1.x) / up.x));
        let (inf, seg1, _) = probe_from((s0, s1), &three, on_line);
        assert!(inf.constraints.contains(&AutoConstraint::PerpendicularTo(seg1)) && inf.constraints.iter().any(|c| matches!(c, AutoConstraint::OnCurve(_))), "{inf:?}");
        let (inf, seg1, _) = probe_from((s0, s1), &three, on_vertical);
        assert!(inf.constraints.contains(&AutoConstraint::PerpendicularTo(seg1)) && inf.constraints.iter().any(|c| matches!(c, AutoConstraint::VerticalWith(_))), "{inf:?}");
    }

    /// User P0 11-14: a combined snap shows a glyph for each constraint it creates, the direction's first; a model
    /// edge it ends on (linked at the click) adds coincident; a snap creating nothing shows its kind's.
    #[test]
    fn a_combined_snap_shows_each_glyph() {
        let (l, e) = (EntityId(3), EntityId(4));
        let inf = |kind, constraints: Vec<AutoConstraint>| Inference { kind, position: DVec2::ZERO, entity: None, reference: None, constraints, distance: 0.0, priority: 1 };
        let both = inf(InferenceKind::Perpendicular, vec![AutoConstraint::PerpendicularTo(l), AutoConstraint::OnCurve(e)]);
        assert_eq!(inference_icons(&both, false), vec!["perpendicular", "coincident"]);
        assert_eq!(inference_icons(&both, true), vec!["perpendicular", "coincident"], "one coincident");
        let square = inf(InferenceKind::Perpendicular, vec![AutoConstraint::PerpendicularTo(l)]);
        assert_eq!(inference_icons(&square, false), vec!["perpendicular"]);
        assert_eq!(inference_icons(&square, true), vec!["perpendicular", "coincident"], "onto a lit model edge");
        let aligned = inf(InferenceKind::AlignBoth, vec![AutoConstraint::HorizontalWith(l), AutoConstraint::VerticalWith(e)]);
        assert_eq!(inference_icons(&aligned, false), vec!["horizontal", "vertical"]);
        assert_eq!(inference_icons(&inf(InferenceKind::OnCurve, vec![]), true), vec!["coincident"], "a model edge alone");
        assert_eq!(inference_icons(&inf(InferenceKind::Midpoint, vec![]), false), vec!["midpoint"]);
        assert_eq!((glyph_offset(1), glyph_offset(2)), ((20.0, 20.0), (15.0, 25.0)), "live2's crops");
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
        let q = d.sketch.add_point(v(0.04, 0.06));
        let c2 = d.sketch.add_circle(v(0.08, 0.09), 0.01);
        let readout = |ed: &SketchEditor, sel: &[EntityId]| {
            let mut ed2 = SketchEditor::new(ed.feature, ed.frame);
            ed2.selection = sel.to_vec();
            let r = ed2.selection_readout(&d, &u);
            (!r.is_empty()).then(|| r.iter().map(|(l, v)| format!("{l}: {v}")).collect::<Vec<_>>().join("; "))
        };
        assert_eq!(readout(&ed, &[l1]).as_deref(), Some("Length: 20.000 mm"));
        assert_eq!(readout(&ed, &[c]).as_deref(), Some("Diameter: 10.000 mm"));
        assert_eq!(readout(&ed, &[l1, l2]).as_deref(), Some("Parallel dist: 10.000 mm"));
        assert!(readout(&ed, &[l1, l3]).is_some_and(|t| t.starts_with("Angle: 45") && t.ends_with(" deg")));
        assert!(readout(&ed, &[p]).is_some_and(|t| t.starts_with("Point: X 10.000 mm")), "{:?}", readout(&ed, &[p]));
        // A line and a circle (live sketch-mirror-preselected-batch): nearest, farthest, the centre to the line.
        assert_eq!(readout(&ed, &[l1, c]).as_deref(), Some("Min dist: 53.310 mm; Max dist: 75.711 mm; Center dist: 50.000 mm"));
        // Two points, a point and a line, two circles (live sketch-key-symmetric / -midpoint / -concentric-two-circles).
        assert_eq!(readout(&ed, &[p, q]).as_deref(), Some("Min dist: 50.000 mm"));
        assert_eq!(readout(&ed, &[p, l1]).as_deref(), Some("Min dist: 20.000 mm"));
        assert_eq!(readout(&ed, &[c, c2]).as_deref(), Some("Center dist: 50.000 mm; Max dist: 65.000 mm; Min dist: 35.000 mm"));
        assert_eq!(readout(&ed, &[l1, l2, c]), None, "mixed selection shows nothing");
        ed.selection.clear();
        assert!(ed.selection_readout(&d, &u).is_empty());
        let _ = &mut d;
    }

    #[test]
    fn pair_readouts_follow_lives_kinds_and_arcs() {
        // Live2 feel/sketch-readouts (11-17), mm: each pair either way round reads the same.
        let (ed, mut d) = editor(Tool::Select);
        let u = UnitPrefs::default();
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let pt1 = d.sketch.add_point(mm(-80.0, 40.0));
        let c1 = d.sketch.add_circle(mm(0.0, 0.0), 0.025);
        let c3 = d.sketch.add_circle(mm(0.0, 0.0), 0.006);
        let c4 = d.sketch.add_circle(mm(15.0, 0.0), 0.005);
        let a1 = d.sketch.add_arc_angles(mm(70.0, 0.0), 0.015, 0.0, PI);
        let a2 = d.sketch.add_arc_angles(mm(70.0, -50.0), 0.010, PI, TAU);
        let l1 = d.sketch.add_line(mm(-80.0, -40.0), mm(20.0, -40.0));
        let pt2 = d.sketch.add_point(mm(-30.0, -40.0));
        let readout = |sel: [EntityId; 2]| {
            let text = |sel: &[EntityId]| {
                let mut ed2 = SketchEditor::new(ed.feature, ed.frame);
                ed2.selection = sel.to_vec();
                ed2.selection_readout(&d, &u).iter().map(|(l, v)| format!("{l} {}", v.trim_end_matches(" mm"))).collect::<Vec<_>>().join(" | ")
            };
            let (ab, ba) = (text(&sel), text(&[sel[1], sel[0]]));
            assert_eq!(ab, ba, "the pick order changes nothing");
            ab
        };
        assert_eq!(readout([pt1, c1]), "Min dist 64.443 | Max dist 114.443 | Center dist 89.443");
        assert_eq!(readout([pt1, a1]), "Min dist 140.242 | Max dist 169.779 | Center dist 155.242");
        assert_eq!(readout([l1, a1]), "Min dist 53.151 | Max dist 170.242 | Center dist 40.000");
        assert_eq!(readout([c1, a1]), "Center dist 70.000 | Max dist 110.000 | Min dist 30.000");
        assert_eq!(readout([a1, a2]), "Center dist 50.000 | Max dist 75.000 | Min dist 50.249");
        assert_eq!(readout([c1, c3]), "Center dist 0.000 | Max dist 31.000 | Min dist 19.000");
        assert_eq!(readout([c1, c4]), "Center dist 15.000 | Max dist 45.000 | Min dist 5.000");
        assert_eq!(readout([pt2, l1]), "Min dist 0.000");
        assert_eq!(readout([pt1, pt2]), "Min dist 94.340");
        assert_eq!(readout([c1, l1]), "Min dist 15.000 | Max dist 114.443 | Center dist 40.000");
        let mut lone = SketchEditor::new(ed.feature, ed.frame);
        lone.selection = vec![a1];
        assert_eq!(lone.selection_readout(&d, &u), vec![("Radius".to_string(), "15.000 mm".to_string())]);
        assert_eq!(lone.selection_readout_icon(&d), Some("center-point-arc"));
        lone.selection = vec![c1];
        assert_eq!(lone.selection_readout_icon(&d), Some("circle"));
        lone.selection = vec![c1, a1];
        assert_eq!(lone.selection_readout_icon(&d), None, "pairs have no icon");
    }

    /// Live sk2-1121 dim-ellipse-axes: the Dimension tool on an ellipse, then a plane: refused with live's words, the
    /// picks dropped.
    #[test]
    fn an_ellipse_and_a_plane_cannot_be_dimensioned() {
        let (mut ed, mut d) = editor(Tool::Dimension);
        let ellipse = d.sketch.add_ellipse(v(0.0, 0.0), v(0.03, 0.0), 0.015);
        let r = d.sketch.use_external(&noshape_sketch::Curve2::Line { a: v(-0.05, 0.0), b: v(0.05, 0.0) }).unwrap();
        let plane = *r.created.last().unwrap();
        d.external_refs.insert(plane.0, noshape_model::reference::Reference::Plane(noshape_model::reference::DatumPlane::Front));
        for (p, e) in [(v(0.03, 0.0), ellipse), (v(0.04, 0.005), plane)] {
            ed.tool_down(&mut d, p, Some(e), false, false);
            ed.tool_up(&mut d, p);
        }
        assert_eq!((ed.toast.as_deref(), ed.picks.len()), (Some(DIMENSION_FAILED), 0));
    }

    /// Live sk2-1121 dim-circle-centres: a circle's centre picked by Dimension shows Construction active; a line's end
    /// (dim-hv-two-points) doesn't.
    /// Live cparc-end-1121: a centre point arc's end clicked on a line's end, on its circle or a few pixels off, makes no
    /// COINCIDENT (to the end or the line), only VERTICAL to the centre, and keeps the start click's radius.
    #[test]
    fn a_centre_point_arc_end_takes_no_point() {
        for off in [0.0, 3.0 * PX] {
            let (mut ed, mut d) = editor(Tool::CenterArc);
            let l = d.sketch.add_line(v(0.03, -0.02 - off), v(0.0, -0.02 - off));
            let end = d.sketch.endpoints(l).unwrap().1;
            click(&mut ed, &mut d, v(0.0, 0.0));
            click(&mut ed, &mut d, v(0.02, 0.0));
            for a in [-0.4f64, -0.9, -1.4] {
                ed.pointer_move(&mut d, DVec2::from_angle(a) * 0.02, false, false);
            }
            click(&mut ed, &mut d, v(0.0, -0.02 - off));
            let Some(Curve2::Arc { radius, .. }) = d.sketch.entities().find(|e| e.kind() == EntityKind::Arc).and_then(|e| d.sketch.curve(e.id)) else { panic!() };
            let on_line = d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Coincident { a, b } if [a, b].iter().any(|e| *e == end || *e == l)));
            let vertical = d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Vertical { b: Some(_), .. }));
            assert_eq!((on_line, vertical, (radius - 0.02).abs() < 1e-9), (false, true, true), "off {off}: radius {radius}");
        }
    }

    #[test]
    fn a_picked_circle_centre_shows_construction() {
        for centre in [true, false] {
            let (mut ed, mut d) = editor(Tool::Dimension);
            let circle = d.sketch.add_circle(v(0.03, 0.0), 0.004);
            let line = d.sketch.add_line(v(0.0, 0.02), v(0.03, 0.02));
            let (p, e) = match centre {
                true => (v(0.03, 0.0), d.sketch.entity(circle).unwrap().center().unwrap()),
                false => (v(0.0, 0.02), d.sketch.endpoints(line).unwrap().0),
            };
            ed.tool_down(&mut d, p, Some(e), false, false);
            ed.tool_up(&mut d, p);
            assert_eq!((ed.picks.len(), ed.construction_shown(&d.sketch)), (1, centre));
        }
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
        // A line's length lies in the plane along it (live sk/inference-vertical-at); a circle's Ø doesn't.
        assert_eq!(ed.preview_value_direction(&d), Some(DVec2::Y));
        let (mut ed, mut d) = editor(Tool::Circle);
        click(&mut ed, &mut d, v(0.0, 0.0));
        ed.pointer_move(&mut d, v(0.01, 0.0), true, false);
        assert_eq!(ed.preview_value_direction(&d), None);
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
        // Inscribed: outside the middle of the clicked edge (0.03, 0.02),
        // along its outward normal.
        let (at, out, _) = ed.polygon_side_control().unwrap();
        assert!(at.x > 0.03 && at.x < 0.033 && (at.y - 0.02).abs() < 1e-12, "{at}");
        assert!((out - DVec2::X).length() < 1e-9, "{out}");
        ed.pointer_move(&mut data, v(0.08, 0.06), false, false);
        assert_eq!(ed.polygon_sides, 7, "spinner's explicit value stays locked");
        click(&mut ed, &mut data, v(0.08, 0.06));
        assert_eq!(count(&data, EntityKind::Line), 7);
        // Live (behaviours "skasm (2026-10-27) sketch polygon side count" (d)):
        // the click commits and doesn't start the next polygon.
        assert!(ed.clicks.is_empty(), "no next polygon at the click");
        assert_eq!(ed.tool, Tool::Polygon);
        assert_eq!(ed.polygon_sides, 7, "the next polygon of the session keeps the count");
        ed.set_tool(Tool::Line);
        ed.set_tool(Tool::Polygon);
        assert_eq!(ed.polygon_sides, 6, "a new polygon tool session starts at 6 (live)");
    }

    /// Live (skasm 10-26, PS "skasm poly"): the Inscribed polygon's second
    /// click is the middle of an edge (the dash-dot circle touches the
    /// edges), the Circumscribed polygon's a vertex (the circle through the
    /// vertices); the Ø box is the circle's through the click.
    #[test]
    fn polygon_second_click_is_an_edge_middle_inscribed_and_a_vertex_circumscribed() {
        for (tool, on_vertex) in [(Tool::Polygon, false), (Tool::CircumPolygon, true)] {
            let (mut ed, mut data) = editor(tool);
            click(&mut ed, &mut data, v(0.02, 0.02));
            click(&mut ed, &mut data, v(0.03, 0.02));
            assert!(ed.finish_polygon(&mut data).changed);
            let rim = v(0.03, 0.02);
            let lines: Vec<_> = data.sketch.entities().filter(|e| e.kind() == EntityKind::Line && !e.construction).map(|e| data.sketch.curve(e.id).unwrap()).collect();
            assert_eq!(lines.len(), 6);
            let at_vertex = lines.iter().any(|c| (c.start_point() - rim).length() < 1e-9);
            let at_middle = lines.iter().any(|c| ((c.start_point() + c.end_point()) * 0.5 - rim).length() < 1e-9);
            assert_eq!((at_vertex, at_middle), (on_vertex, !on_vertex), "{tool:?}");
            assert!(ed.otf.is_none(), "live makes no Ø dimension");
            let (at, text) = ed.polygon_readout(&UnitPrefs::default()).expect("the boxed Ø readout");
            assert!((at - v(0.025, 0.02)).length() < 1e-9 && text.starts_with('Ø'), "{at} {text}");
            click(&mut ed, &mut data, v(0.07, 0.02));
            assert!(ed.polygon_readout(&UnitPrefs::default()).is_none(), "the next polygon clears it");
            assert!(data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::CircularPatternLink { .. })), "the edges are a circular pattern");
            let circle = data.sketch.entities().find(|e| e.kind() == EntityKind::Circle).map(|e| data.sketch.curve(e.id).unwrap());
            assert!(matches!(circle, Some(Curve2::Circle { radius, .. }) if (radius - 0.01).abs() < 1e-9), "{tool:?}: the circle goes through the click");
            // Its pattern is one constraint (live 2496): deleting it takes every link.
            let link = data.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::CircularPatternLink { .. })).map(|c| c.id).unwrap();
            assert!(ed.delete_constraint(&mut data, link));
            assert!(!data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::CircularPatternLink { .. })), "{tool:?}: the whole pattern goes");
        }
    }

    /// Live sk2-1121 polygon-corner-drag: the inscribed polygon's side midpoint dragged off and released keeps its one
    /// MIDPOINT (none added on its own side).
    #[test]
    fn a_dragged_polygon_side_midpoint_adds_no_second_midpoint() {
        let (mut ed, mut data) = editor(Tool::Polygon);
        click(&mut ed, &mut data, v(0.0, 0.0));
        click(&mut ed, &mut data, v(0.02, 0.0));
        assert!(ed.finish_polygon(&mut data).changed);
        let mids = |d: &SketchData| d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::Midpoint { .. })).count();
        let before = (mids(&data), data.sketch.constraints().count());
        ed.set_tool(Tool::Select);
        drag(&mut ed, &mut data, v(0.02, 0.0), v(0.025, 0.005));
        assert_eq!((mids(&data), data.sketch.constraints().count()), before, "no constraint added on release");
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
    fn an_angle_label_writes_radians_out_and_degrees_as_a_sign() {
        // Live 11-18 (server-side number formats, row B): "… radian" in a radian workspace, "…°" in degrees.
        let k = ConstraintKind::Angle { a: EntityId(0), b: EntityId(1), value: 1.2 };
        let rad = UnitPrefs { angle: noshape_units::AngleUnit::Radian, ..UnitPrefs::default() };
        let text = dim_value_text(&k, 1.2, &rad);
        assert!(text.ends_with(" radian") && text.starts_with("1.2"), "{text}");
        assert!(dim_value_text(&k, std::f64::consts::FRAC_PI_4, &UnitPrefs::default()).ends_with('°'));
    }

    #[test]
    fn the_offset_arrow_steps_as_the_feature_arrows() {
        // Live 11-18 sketch-tools at 5.31 px/mm: 0.1 mm (10.1 mm); the triad / feature rule zoomed out: 1 mm.
        let step = |mm_per_px: f64| drag_step(mm_per_px * 1e-3) * 1e3;
        assert!((step(1.0 / 5.31) - 0.1).abs() < 1e-12 && (step(1.0) - 1.0).abs() < 1e-12 && (step(0.0288) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn enter_leaves_a_spline_being_drawn_as_it_is() {
        // Live 11-18 sketch-tools: after 3 clicks Enter (twice) does nothing; the pending spline and the tool stay.
        let (mut ed, mut d) = editor(Tool::Spline);
        for p in [v(0.01, 0.01), v(0.02, 0.02), v(0.03, 0.01)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!((ed.enter_target(), ed.clicks.len(), ed.tool), (EnterTarget::Nothing, 3, Tool::Spline));
    }

    #[test]
    fn extend_picks_the_nearer_end_which_follows_the_pointer_and_stops_on_a_crossing_curve() {
        // Live 11-18 sketch-tools (t4-ext, t7), mc-reach-1122 R6: a click picks the line's nearer end (the line lit,
        // selected); the end follows the pointer along the line, and a click past a crossing curve stops it there.
        let (mut ed, mut data) = editor(Tool::Extend);
        let l = data.sketch.add_line(v(0.01, 0.06), v(0.03, 0.06));
        let x = data.sketch.add_line(v(0.05, 0.03), v(0.05, 0.09));
        click(&mut ed, &mut data, v(0.03 - 2.0 * PX, 0.06));
        assert_eq!((ed.extend_from, ed.selection.clone()), (Some(l), vec![l]));
        let end = data.sketch.endpoints(l).unwrap().1;
        assert_eq!(ed.extend_end, Some(end), "the nearer end");
        assert_eq!(ed.extend_target(&data.sketch, v(0.04, 0.065)), Some((v(0.04, 0.06), None)), "short of the crossing: at the pointer's foot");
        // Live mc-reach-1122 R6: the preview runs past the boundary to the pointer, onto it only while the pointer is on
        // it; passing over it turns the picked line selected.
        let (past, _) = ed.extend_preview(&data.sketch, v(0.05 + 42.0 * PX, 0.062)).unwrap();
        assert!((past - v(0.05 + 42.0 * PX, 0.06)).length() < 1e-9 && !ed.extend_crossed, "jumped past: to the pointer's foot, {past:?}");
        ed.pointer_move(&mut data, v(0.05 + 2.0 * PX, 0.0605), false, false);
        assert_eq!(ed.extend_preview(&data.sketch, v(0.05 + 2.0 * PX, 0.0605)).map(|(_, on)| on), Some(Some(x)), "on the boundary: snapped");
        assert!(ed.extend_crossed, "passed over the boundary");
        click(&mut ed, &mut data, v(0.05 + 42.0 * PX, 0.062));
        assert!((data.sketch.curve(l).unwrap().end_point() - v(0.05, 0.06)).length() < 1e-9, "stopped on the crossing line");
        assert!(data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Coincident { a, b } if (a, b) == (end, x))));
        assert_eq!((ed.extend_from, ed.extend_end, ed.message.clone(), ed.tool), (None, None, None, Tool::Extend));
        // With nothing in the way the end goes to the pointer, back over the line too: shorter.
        let l2 = data.sketch.add_line(v(0.01, 0.02), v(0.03, 0.02));
        click(&mut ed, &mut data, v(0.03 - 2.0 * PX, 0.02));
        click(&mut ed, &mut data, v(0.025, 0.018));
        assert!((data.sketch.curve(l2).unwrap().end_point() - v(0.025, 0.02)).length() < 1e-9, "shortened to the pointer");
        click(&mut ed, &mut data, v(0.025, 0.06));
        assert_eq!(ed.selection, vec![l]);
        assert!(ed.cancel());
        assert!(ed.selection.is_empty() && ed.extend_end.is_none(), "Esc drops the pending pick");
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

    /// qa5 P1-4 (live "qa5 (10-25 p1)" (1)): points placed in horizontal
    /// alignment with the Origin are Horizontal with it, and no sketch point
    /// appears at the origin (none shown, picked or used by a Hole).
    #[test]
    fn points_aligned_with_the_origin_add_no_origin_point() {
        let (mut ed, mut d) = editor(Tool::Point);
        click(&mut ed, &mut d, v(0.03, -PX * 3.0));
        click(&mut ed, &mut d, v(-0.04, PX * 3.0));
        let shown: Vec<EntityId> = d.sketch.entities().filter(|e| e.kind() == EntityKind::Point && !e.reference_only).map(|e| e.id).collect();
        assert_eq!(shown.len(), 2, "{:?}", d.sketch.entities().collect::<Vec<_>>());
        let hidden: Vec<&noshape_sketch::Entity> = d.sketch.entities().filter(|e| e.reference_only).collect();
        assert!(hidden.iter().all(|e| e.construction && e.external), "{hidden:?}");
        for p in shown {
            assert!(d.sketch.point(p).unwrap().y.abs() < 1e-12);
            assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { a, b: Some(o) } if a == p && d.sketch.entity(o).is_some_and(|e| e.reference_only))));
        }
    }

    /// Live measures inference and pick reach on screen (SKETCH-SCREEN-SPACE-REACH; ui-parity sketch-horizontal-key,
    /// sketch-vertical-key, sketch-offset): in a view that shows the plane's y four times shorter than its x, a line
    /// 1.5 mm (15 px of the plane's x) off level is 3.75 px off on screen, inside the 7.5 px reach, so Horizontal; one
    /// 1.5 mm off plumb is 15 px off, so no Vertical; a click 2.5 mm below a line (6.25 px) picks it.
    #[test]
    fn reach_is_measured_on_the_screen() {
        let foreshortened = |ed: &mut SketchEditor| ed.screen_map = Some((DVec2::ZERO, v(PX, 0.0), v(0.0, -4.0 * PX)));
        let (mut ed, mut d) = editor(Tool::Line);
        foreshortened(&mut ed);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.04, 0.0115));
        let level = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).unwrap().id;
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { a, b: None } if a == level)), "Horizontal: 3.75 px off on screen");
        let (mut ed, mut d) = editor(Tool::Line);
        foreshortened(&mut ed);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.0115, 0.04));
        assert!(!d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Vertical { .. })), "no Vertical: 15 px off on screen");
        // Without the view (headless): in the plane, 15 px off level is no Horizontal.
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.04, 0.0115));
        assert!(!d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { .. })));
        // A pick 2.5 mm below a line: 6.25 px on screen.
        let (mut ed, mut d) = editor(Tool::Select);
        let line = d.sketch.add_line(v(0.0, 0.02), v(0.03, 0.02));
        assert_eq!(d.sketch.pick_on_screen(v(0.015, 0.0175), PX * 9.0, ed.screen_metric().as_ref()), None, "in the plane: 25 px");
        foreshortened(&mut ed);
        assert_eq!(d.sketch.pick_on_screen(v(0.015, 0.0175), PX * 9.0, ed.screen_metric().as_ref()), Some(line), "on screen: 6.25 px");
    }

    /// The reach in live's own view of a Top sketch (behaviours 2155: x 3.0 / 0.87 px, y 1.7 / -1.5 px a mm on
    /// screen), as its recordings: (0, 0) to (30, 3) turns horizontal (5.7 px off level), (0, 0) to (3, 30) not
    /// vertical (7.9 px off plumb); a point at (10, 5) isn't put on the line along y = 0 (9.6 px off it).
    #[test]
    fn reach_matches_lives_recordings() {
        let lives = |ed: &mut SketchEditor| {
            let px_per_m = glam::DMat2::from_cols(v(3.0, 0.867), v(1.7, -1.5)) * 1e3;
            let per_px = px_per_m.inverse();
            ed.screen_map = Some((DVec2::ZERO, per_px.x_axis, per_px.y_axis));
        };
        let drawn = |to: DVec2| {
            let (mut ed, mut d) = editor(Tool::Line);
            lives(&mut ed);
            click(&mut ed, &mut d, v(0.0, 0.0));
            click(&mut ed, &mut d, to);
            d
        };
        let d = drawn(v(0.030, 0.003));
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { b: None, .. })), "Horizontal");
        let d = drawn(v(0.003, 0.030));
        assert!(!d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Vertical { .. })), "no Vertical");
        let (mut ed, mut d) = editor(Tool::Point);
        lives(&mut ed);
        let line = d.sketch.add_line(v(0.0, 0.0), v(0.030, 0.0));
        click(&mut ed, &mut d, v(0.010, 0.005));
        assert!(!d.sketch.constraints().any(|c| c.kind.entities().contains(&line)), "not on the line");
    }

    /// A constraint the sketch already has isn't added again, and counts as applied (live, ui-parity
    /// sketch-horizontal-key: H on a line drawn horizontal adds nothing and clears the selection).
    #[test]
    fn an_existing_constraint_isnt_added_again() {
        let mut sk = Sketch::new();
        let l = sk.add_line(v(0.0, 0.0), v(0.03, 0.0));
        assert_eq!(apply_constraint(&mut sk, ConstraintTool::Horizontal, &[l]), Ok(1));
        assert_eq!(apply_constraint(&mut sk, ConstraintTool::Horizontal, &[l]), Ok(1), "applied: it's there");
        assert_eq!(sk.constraints().count(), 1);
    }

    #[test]
    fn line_snaps_to_origin_and_infers_horizontal() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(PX * 2.0, -PX * 3.0));
        click(&mut ed, &mut d, v(0.03, PX * 2.0));
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).unwrap().id;
        let (s, _) = d.sketch.endpoints(line).unwrap();
        // Live: the start is coincident with the Origin (ORIGIN-COINCIDENT-GLYPH).
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Coincident { a, b } if a == s && is_origin(&d.sketch, b))), "start coincident with the Origin");
        assert_eq!(d.sketch.point(s).unwrap(), DVec2::ZERO);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { a, b: None } if a == line)));
    }

    /// Live (behaviours "qa4 round 5 s1 follow-up 2" (2)): a line chained to
    /// the origin from a point a hair (0.66°) off vertical gets only the
    /// Coincident there, no Vertical, and keeps its tilt.
    #[test]
    fn line_ending_on_a_point_off_vertical_adds_no_vertical() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.015, 0.005));
        click(&mut ed, &mut d, v(0.015, 0.0227));
        // Shift: no snap to x 0 for the third point.
        let c = v(PX * 2.6, 0.0227);
        ed.pointer_move(&mut d, c, true, false);
        ed.tool_down(&mut d, c, None, true, false);
        ed.tool_up(&mut d, c);
        click(&mut ed, &mut d, v(PX * 0.5, PX * 0.5));
        let last = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).last().unwrap().id;
        let (s, e) = d.sketch.endpoints(last).unwrap();
        assert_eq!(d.sketch.point(e).unwrap(), DVec2::ZERO, "ends on the origin");
        let kinds: Vec<ConstraintKind> = d.sketch.constraints().map(|c| c.kind.clone()).collect();
        assert!(!kinds.iter().any(|k| matches!(k, ConstraintKind::Vertical { a, .. } if *a == last)), "{kinds:?}");
        assert!(d.sketch.point(s).unwrap().x > PX, "keeps its tilt: {}", d.sketch.point(s).unwrap());
    }

    /// Live (behaviours "skb (10-25) sketch inference", 6.175 px/mm): Line
    /// strokes a few px off their targets. Midpoint start + Vertical; no
    /// alignment snap to a free point 1.9 px away (Horizontal only) unless
    /// the cursor passed over it first (then Horizontal + Vertical with it);
    /// an end near a circle's top gets Coincident + Tangent (no quadrant, no
    /// Horizontal); an end at a circle centre is Coincident with it.
    #[test]
    fn line_inference_matches_live_strokes() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let (mut ed, mut d) = editor(Tool::Select);
        ed.world_per_px = 1e-3 / 6.175;
        let h = d.sketch.add_line(mm(0.0, 0.0), mm(40.0, 0.0));
        let c = d.sketch.add_circle(mm(70.0, 20.0), 0.01);
        let pt = d.sketch.add_point(mm(20.0, 30.0));
        let line = |ed: &mut SketchEditor, d: &mut SketchData, a: DVec2, b: DVec2| {
            ed.set_tool(Tool::Line);
            for p in [a, b] {
                ed.pointer_move(d, p, false, false);
                let hv = d.sketch.pick(p, ed.world_per_px * 4.0);
                ed.hover = hv;
                ed.tool_down(d, p, hv, false, false);
                ed.tool_up(d, p);
            }
            ed.cancel();
            d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).last().unwrap().id
        };
        let kinds = |d: &SketchData, l: EntityId| -> Vec<String> {
            let (s, e) = d.sketch.endpoints(l).unwrap();
            let mut k: Vec<String> = d.sketch.constraints().filter(|c| c.kind.entities().iter().any(|x| [l, s, e].contains(x))).map(|c| format!("{:?}", c.kind).split([' ', '{']).next().unwrap().to_string()).collect();
            k.sort();
            k
        };
        let at = |d: &SketchData, l: EntityId| {
            let (s, e) = d.sketch.endpoints(l).unwrap();
            (d.sketch.point(s).unwrap() * 1e3, d.sketch.point(e).unwrap() * 1e3)
        };
        let l1 = line(&mut ed, &mut d, mm(20.3, 0.2), mm(20.3, 12.0));
        assert_eq!(kinds(&d, l1), ["Midpoint", "Vertical"], "{:?}", at(&d, l1));
        assert!((at(&d, l1).0 - DVec2::new(20.0, 0.0)).length() < 1e-6, "{:?}", at(&d, l1));
        let l2 = line(&mut ed, &mut d, mm(40.0, 45.0), mm(20.3, 45.3));
        assert_eq!(kinds(&d, l2), ["Horizontal"], "{:?}", at(&d, l2));
        assert!((at(&d, l2).1.x - 20.3).abs() < 0.05, "no snap to the point's x: {:?}", at(&d, l2));
        // Snapping onto the point first wakes it (live feel retaken 11-18: the snap wakes it, no rest): the end snaps
        // to its x and gets a Vertical with it.
        ed.set_tool(Tool::Line);
        for (p, hv) in [(mm(40.0, 60.0), None), (mm(20.0, 30.0), Some(pt)), (mm(20.3, 60.3), None)] {
            ed.hover = hv;
            ed.pointer_move(&mut d, p, false, false);
            if hv.is_none() {
                ed.tool_down(&mut d, p, None, false, false);
                ed.tool_up(&mut d, p);
            }
        }
        ed.cancel();
        let lw = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).last().unwrap().id;
        assert!((at(&d, lw).1.x - 20.0).abs() < 1e-6, "{:?}", at(&d, lw));
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Vertical { b: Some(b), .. } if b == pt) || matches!(c.kind, ConstraintKind::Vertical { a, .. } if a == pt)), "vertical with the point");
        let l3 = line(&mut ed, &mut d, mm(50.0, 30.3), mm(70.3, 30.2));
        assert_eq!(kinds(&d, l3), ["Coincident", "Tangent"], "{:?}", at(&d, l3));
        let l4 = line(&mut ed, &mut d, mm(85.0, 3.0), mm(70.3, 20.3));
        assert_eq!(kinds(&d, l4), ["Coincident"], "{:?}", at(&d, l4));
        let Some(Curve2::Circle { center, .. }) = d.sketch.curve(c) else { panic!() };
        assert!((at(&d, l4).1 - center * 1e3).length() < 1e-6, "on the centre: {:?} {center}", at(&d, l4));
        let _ = h;
    }

    #[test]
    fn a_point_dropped_by_a_lines_midpoint_takes_it() {
        // Live (audit 1969 (8)): dropping a point onto a line by its middle
        // gives MIDPOINT; elsewhere on the line, a coincident.
        let (mut ed, mut data) = editor(Tool::Select);
        let l = data.sketch.add_line(v(0.0, 0.0), v(0.04, 0.0));
        let p = data.sketch.add_point(v(0.01, 0.01));
        drag(&mut ed, &mut data, v(0.01, 0.01), v(0.02 + PX, PX));
        assert!(data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Midpoint { point, curve } if point == p && curve == l)));
        assert!((data.sketch.point(p).unwrap() - v(0.02, 0.0)).length() < 1e-9);
        let q = data.sketch.add_point(v(0.03, 0.01));
        drag(&mut ed, &mut data, v(0.03, 0.01), v(0.032, PX));
        assert!(data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Coincident { a, b } if a == q && b == l)));
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
    fn a_drag_event_at_the_same_pointer_position_is_not_solved_again() {
        // The viewport repeats pointer moves at one position (seen in the
        // app: 4 of 21 drag steps twice); each would cost a whole drag solve.
        let (mut ed, mut data) = editor(Tool::Select);
        let p = data.sketch.add_point(v(0.01, 0.01));
        ed.tool_down(&mut data, v(0.01, 0.01), Some(p), false, false);
        assert!(ed.tool_drag(&mut data, v(0.012, 0.011)).changed);
        assert!(!ed.tool_drag(&mut data, v(0.012, 0.011)).changed);
        assert!(ed.tool_drag(&mut data, v(0.013, 0.011)).changed);
        assert_eq!(data.sketch.point(p), Some(v(0.013, 0.011)));
    }

    #[test]
    fn a_drag_ends_where_its_pointer_ends_whatever_the_path() {
        // Live drag probes 12: a bolt circle's hole dragged to one pixel by
        // four different paths lands on the same points; the next hole
        // stays, the last one goes radially onto its circle.
        use std::f64::consts::{FRAC_PI_2, TAU};
        let bolt_circle = || {
            let (mut ed, mut data) = editor(Tool::Select);
            let sk = &mut data.sketch;
            let centre_of = |sk: &Sketch, c: EntityId| match sk.entity(c).unwrap().geometry {
                noshape_sketch::Geometry::Circle { center, .. } => center,
                _ => unreachable!(),
            };
            let big = sk.add_circle(v(0.0, 0.0), 0.02);
            let holes: Vec<EntityId> = (0..3).map(|i| sk.add_circle(DVec2::from_angle(FRAC_PI_2 + i as f64 * TAU / 3.0) * 0.014, 0.003)).collect();
            for h in &holes[1..] {
                sk.add_constraint(ConstraintKind::Equal { a: holes[0], b: *h }).unwrap();
            }
            sk.add_constraint(ConstraintKind::Radius { curve: big, value: 0.02 }).unwrap();
            sk.add_constraint(ConstraintKind::Radius { curve: holes[0], value: 0.003 }).unwrap();
            let c = centre_of(sk, big);
            let hc: Vec<EntityId> = holes.iter().map(|h| centre_of(sk, *h)).collect();
            for h in &hc {
                sk.add_constraint(ConstraintKind::Distance { a: c, b: *h, value: 0.014, mode: DistanceMode::Center }).unwrap();
            }
            assert!(sk.solve().is_solved());
            ed.tool_down(&mut data, v(0.0, 0.014), Some(hc[0]), false, false);
            (ed, data, [c, hc[0], hc[1], hc[2]])
        };
        let end = v(0.0030736, 0.0162029);
        let mut results = Vec::new();
        for path in [vec![v(0.001, 0.015), end], vec![v(0.004, 0.012), v(-0.002, 0.017), end], (1..=25).map(|k| v(0.0, 0.014) + (end - v(0.0, 0.014)) * (k as f64 / 25.0)).collect()] {
            let (mut ed, mut data, pts) = bolt_circle();
            for p in path {
                ed.tool_drag(&mut data, p);
            }
            results.push(pts.map(|q| data.sketch.point(q).unwrap()));
        }
        for r in &results[1..] {
            for (a, b) in r.iter().zip(&results[0]) {
                assert!((*a - *b).length() < 1e-9, "{results:?}");
            }
        }
        let live = [v(-2.9245, 3.5528), v(3.0736, 16.2029), v(-12.1244, -7.0), v(8.5381, -4.4852)];
        for (p, q) in results[0].iter().zip(&live) {
            assert!((*p * 1e3 - *q).length() < 1e-3, "ours {:?} live {live:?}", results[0]);
        }
    }

    /// Live ui-parity sketch-angular-dimension: a new angle's value box reads
    /// "31.686223 deg" (7 decimals, zeros trimmed, " deg"; its label "°").
    #[test]
    fn an_angle_value_box_reads_deg() {
        let (mut ed, mut data) = editor(Tool::Select);
        let a = data.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
        let b = data.sketch.add_line(v(0.0, 0.0), v(0.025, 0.015));
        let value = 31.686223f64.to_radians();
        let cid = data.sketch.add_constraint(ConstraintKind::Angle { a, b, value }).unwrap();
        let units = UnitPrefs::default();
        let text = dimension_edit_text(&data, cid, &units).unwrap();
        assert_eq!(text, "31.686223 deg");
        ed.apply_dimension(&mut data, cid, &text, &units, &noshape_units::EmptyScope).unwrap();
        assert!((data.sketch.constraint(cid).unwrap().kind.value().unwrap() - value).abs() < 1e-9);
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
        // A bare expression too (live, qa4 DIM-EXPR-UNIT: "2*5" → "(2*5) mm").
        ed.apply_dimension(&mut data, cid, "2*5", &units, &noshape_units::EmptyScope).unwrap();
        assert_eq!(dimension_edit_text(&data, cid, &units).as_deref(), Some("(2*5) mm"));
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

    /// Live lenbox-1122 (behaviours "skasm length box side"; workflow 05, qa SKETCH-LENGTH-LABEL-IN-PLANE): a drawn
    /// segment's box lies left of its drawing direction on the perpendicular through its middle, its centre (px)
    /// 27 above one drawn rightward, 25 below one drawn leftward, 41 right of one drawn downward, 35 down-right of one
    /// drawn down-left; left of one drawn upward (live2 11-17 value-box).
    #[test]
    fn a_drawn_lines_box_lies_left_of_its_direction() {
        let side = |from: DVec2, to: DVec2| {
            let (mut ed, mut d) = editor(Tool::Line);
            click(&mut ed, &mut d, from);
            click(&mut ed, &mut d, to);
            (ed.otf_boxes(&d, &UnitPrefs::default())[0].0 - (from + to) * 0.5) / PX
        };
        let near = |got: DVec2, want: DVec2, what: &str| assert!((got - want).length() < 2.5, "{what}: {got:?} px off the middle, live {want:?}");
        near(side(v(0.01, 0.012), v(0.03, 0.012)), v(0.0, 27.0), "rightward: above");
        near(side(v(0.03, 0.01), v(0.01, 0.01)), v(0.0, -25.0), "leftward: below");
        assert!(side(v(0.02, 0.01), v(0.02, 0.03)).x < -38.5, "upward: left");
        near(side(v(0.02, 0.03), v(0.02, 0.01)), v(41.0, 0.0), "downward: right");
        let down_left = side(v(0.03, 0.03), v(0.01, 0.02));
        near(down_left, DVec2::new(1.0, -2.0).normalize() * 35.0, "down-left: down-right, on the normal");
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
        assert_eq!(ed.tool, Tool::Select, "Confirm spline turns the tool off");
    }

    #[test]
    fn confirm_bezier_finishes_with_the_clicked_points() {
        let (mut ed, mut d) = editor(Tool::Bezier);
        for p in [v(0.01, 0.01), v(0.03, 0.03), v(0.05, 0.01)] {
            click(&mut ed, &mut d, p);
        }
        assert!(ed.confirm_spline(&mut d).changed);
        assert_eq!(count(&d, EntityKind::BSpline), 1);
        assert_eq!(ed.tool, Tool::Select);
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
        // Dimension too, starting no dimension (live skasm/hm-sk-1122 c5a).
        ed.set_tool(Tool::Select);
        ed.selection.push(line);
        ed.set_tool(Tool::Dimension);
        assert!(ed.selection.is_empty() && ed.picks.is_empty());
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
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { b: None, .. })), "live: a horizontal major axis is Horizontal");
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
        assert!(matches!(smart_dimension(&sk, &[el], v(46.0, 0.0)), Some(ConstraintKind::EllipseRadius { major: false, .. })));
        assert!(matches!(smart_dimension(&sk, &[el], v(40.0, 4.0)), Some(ConstraintKind::EllipseRadius { major: true, .. })));
        assert!(matches!(smart_dimension(&sk, &[el], v(41.0, 5.0)), Some(ConstraintKind::EllipseRadius { major: true, .. })), "above, a little off the centre");
        // dimension to the origin
        let (mut ed, mut d) = editor(Tool::Dimension);
        d.sketch.add_point(v(0.02, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01));
        click(&mut ed, &mut d, v(PX, PX));
        assert_eq!(ed.picks.len(), 2, "the origin is pickable");
        click(&mut ed, &mut d, v(0.01, 0.03));
        assert!(d.sketch.constraints().any(|c| c.kind.is_dimension()));
    }

    /// Live (ui-parity sketch-box-select-delete): the box is the screen
    /// rectangle; in an iso view a window over a line's plane rectangle
    /// can miss it on the screen.
    #[test]
    fn a_box_selects_on_the_screen() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.0, 0.02), v(0.02, 0.02));
        // A sheared view: one pixel right is (PX, 0.3 PX) on the plane, one down (0.5 PX, -PX).
        ed.screen_map = Some((DVec2::ZERO, v(PX, 0.3 * PX), v(0.5 * PX, -PX)));
        drag(&mut ed, &mut d, v(-0.005, 0.015), v(0.025, 0.025));
        assert!(!ed.selection.contains(&l), "the plane box holds the line, the screen box doesn't");
        let (a, b) = (ed.screen_to_plane(ed.plane_to_screen(v(0.0, 0.02)) - DVec2::splat(5.0)), ed.screen_to_plane(ed.plane_to_screen(v(0.02, 0.02)) + DVec2::splat(5.0)));
        let (lo, hi) = (ed.plane_to_screen(a).min(ed.plane_to_screen(b)), ed.plane_to_screen(a).max(ed.plane_to_screen(b)));
        let (from, to) = (ed.screen_to_plane(lo), ed.screen_to_plane(hi));
        drag(&mut ed, &mut d, from, to);
        assert!(ed.selection.contains(&l), "a screen window around it selects it");
    }

    /// Live (feel/sketch.json "dimension.pointToPointType": two points 50.1165 × 30.0148 mm apart, the pointer R px
    /// from their middle at each 15°, zoom A 0.2754 mm/px): above or below their box horizontal, beside it vertical,
    /// inside it or off a corner aligned.
    #[test]
    fn two_points_dimension_as_live_by_where_the_pointer_is() {
        let mut sk = Sketch::new();
        let (a, b) = (sk.add_point(v(0.0, 0.0)), sk.add_point(v(0.0501165, 0.0300148)));
        let mid = v(0.0501165, 0.0300148) * 0.5;
        let live: &[(u32, u32, char)] = &[(90, 0, 'A'), (90, 15, 'A'), (90, 30, 'A'), (90, 45, 'H'), (90, 60, 'H'), (90, 75, 'H'), (90, 90, 'H'), (90, 105, 'H'), (90, 120, 'H'), (90, 135, 'H'), (90, 150, 'A'), (90, 165, 'A'), (90, 180, 'A'), (90, 195, 'A'), (90, 210, 'A'), (90, 225, 'H'), (90, 240, 'H'), (90, 255, 'H'), (90, 270, 'H'), (90, 285, 'H'), (90, 300, 'H'), (90, 315, 'H'), (90, 330, 'A'), (90, 345, 'A'), (180, 0, 'V'), (180, 15, 'V'), (180, 30, 'A'), (180, 45, 'A'), (180, 60, 'H'), (180, 75, 'H'), (180, 90, 'H'), (180, 105, 'H'), (180, 120, 'H'), (180, 135, 'A'), (180, 150, 'A'), (180, 165, 'V'), (180, 180, 'V'), (180, 195, 'V'), (180, 210, 'A'), (180, 225, 'A'), (180, 240, 'H'), (180, 255, 'H'), (180, 270, 'H'), (180, 285, 'H'), (180, 300, 'H'), (180, 315, 'A'), (180, 330, 'A'), (180, 345, 'V')];
        let mut wrong = Vec::new();
        for &(r, deg, want) in live {
            let cursor = mid + DVec2::from_angle(f64::from(deg).to_radians()) * (f64::from(r) * 0.0002754);
            let got = match smart_dimension(&sk, &[a, b], cursor) {
                Some(ConstraintKind::HorizontalDistance { .. }) => 'H',
                Some(ConstraintKind::VerticalDistance { .. }) => 'V',
                _ => 'A',
            };
            if got != want {
                wrong.push((r, deg, got, want));
            }
        }
        assert!(wrong.is_empty(), "(R, deg, ours, live): {wrong:?}");
    }

    /// Live (live2 arcmid): a 3 point arc's third click within 7.5 px of the semicircle over its ends draws that
    /// semicircle, its centre held at the ends' midpoint (MIDPOINT); 10 px out the arc goes through the click.
    #[test]
    fn a_three_point_arc_snaps_to_the_semicircle() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let arc = |third: DVec2| {
            let (mut ed, mut d) = editor(Tool::Arc3);
            for p in [mm(0.0, 0.0), mm(20.0, 0.0), third] {
                click(&mut ed, &mut d, p);
            }
            let a = d.sketch.entities().find(|e| e.kind() == EntityKind::Arc).map(|e| e.id).expect("an arc");
            let Some(noshape_sketch::Geometry::Arc { center, .. }) = d.sketch.entity(a).map(|e| e.geometry.clone()) else { panic!("no arc") };
            let mid = d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::MidpointOf { point, .. } if point == center));
            (d.sketch.point(center).unwrap(), mid)
        };
        // PX is 0.1 mm: 5 px outside the r 10 semicircle, then 10 px.
        let (c, mid) = arc(mm(10.0, 10.5));
        assert!(mid && (c - mm(10.0, 0.0)).length() < 1e-9, "snapped: centre {c:?}, midpoint {mid}");
        let (c, mid) = arc(mm(10.0, 11.0));
        assert!(!mid && (c - mm(10.0, 0.0)).length() > 1e-6, "free: centre {c:?}, midpoint {mid}");
    }

    /// Live (feel/sketch.json "drag" A-first, mm): a free line's end follows the pointer, its body moves it whole; a
    /// horizontal line's end takes the start along (level kept, its x kept), its body moves it whole; a line from the origin
    /// doesn't move by its body, its free end follows.
    #[test]
    fn drags_move_lines_as_live() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let ends = |d: &SketchData, l: EntityId| {
            let (a, b) = d.sketch.endpoints(l).unwrap();
            (d.sketch.point(a).unwrap(), d.sketch.point(b).unwrap())
        };
        let close = |got: (DVec2, DVec2), want: (DVec2, DVec2)| (got.0 - want.0).length() < 1e-6 && (got.1 - want.1).length() < 1e-6;
        let mut wrong = Vec::new();
        // Free: the end, then the body.
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(mm(-79.8559, 20.2393), mm(-29.7394, 40.0656));
        drag(&mut ed, &mut d, mm(-29.7394, 40.0656), mm(-12.9422, 27.6742));
        if !close(ends(&d, l), (mm(-79.8559, 20.2393), mm(-12.9422, 27.6742))) {
            wrong.push(("free-end", ends(&d, l)));
        }
        let (s, e) = ends(&d, l);
        let delta = mm(-63.334, 7.8479) - mm(-79.8559, 20.2393);
        drag(&mut ed, &mut d, (s + e) * 0.5, (s + e) * 0.5 + delta);
        if !close(ends(&d, l), (s + delta, e + delta)) {
            wrong.push(("free-body", ends(&d, l)));
        }
        // Horizontal: the end up, then the body.
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(mm(-79.8559, 20.2393), mm(-29.7394, 20.2393));
        d.sketch.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
        drag(&mut ed, &mut d, mm(-29.7394, 20.2393), mm(-26.7104, 36.7613));
        if !close(ends(&d, l), (mm(-79.8559, 36.7613), mm(-26.7104, 36.7613))) {
            wrong.push(("horizontal-end-up", ends(&d, l)));
        }
        let (s, e) = ends(&d, l);
        let delta = mm(-68.8413, 50.5295) - mm(-79.8559, 36.7613);
        drag(&mut ed, &mut d, (s + e) * 0.5, (s + e) * 0.5 + delta);
        if !close(ends(&d, l), (s + delta, e + delta)) {
            wrong.push(("horizontal-body", ends(&d, l)));
        }
        // From the origin (drawn with the Line tool from it): its body doesn't move (live2's recheck, 3 takes: no swing),
        // its end follows.
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, mm(40.2033, 25.1959));
        ed.set_tool(Tool::Select);
        let l = d.sketch.entities().find(|x| x.kind() == EntityKind::Line && !x.reference_only).map(|x| x.id).unwrap();
        let before = ends(&d, l);
        drag(&mut ed, &mut d, (before.0 + before.1) * 0.5, (before.0 + before.1) * 0.5 + mm(10.0, -12.0));
        if !close(ends(&d, l), before) {
            wrong.push(("fromOrigin-body", ends(&d, l)));
        }
        drag(&mut ed, &mut d, mm(40.2033, 25.1959), mm(51.2179, 8.674));
        if !close(ends(&d, l), (v(0.0, 0.0), mm(51.2179, 8.674))) {
            wrong.push(("fromOrigin-end", ends(&d, l)));
        }
        assert!(wrong.is_empty(), "{wrong:?}");
    }

    /// Live2 11-17 part 2 (hover-p0, iso view): a corner rectangle (-30, -20)–(30, 20) mm. Its corner (30, -20) dragged
    /// to (39.19, -14.22): the opposite corner stays, the bottom and right lines follow. Its right line dragged right:
    /// it moves along its normal only, to x = 39.20, the top and bottom lines stretch.
    #[test]
    fn a_rectangles_corner_and_side_drag_as_lives() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let corners = |d: &SketchData| {
            let mut c: Vec<DVec2> = d.sketch.entities().filter(|e| e.kind() == EntityKind::Point).filter_map(|e| d.sketch.point(e.id)).filter(|p| p.length() > 1e-9).collect();
            c.dedup_by(|a, b| (*a - *b).length() < 1e-9);
            c
        };
        let near = |got: &[DVec2], want: &[DVec2]| got.len() == want.len() && want.iter().all(|w| got.iter().any(|g| (*g - *w).length() < 0.01e-3));
        for side in [false, true] {
            let (mut ed, mut d) = editor(Tool::CornerRect);
            click(&mut ed, &mut d, mm(-30.0, -20.0));
            click(&mut ed, &mut d, mm(30.0, 20.0));
            ed.set_tool(Tool::Select);
            let want = if side {
                drag(&mut ed, &mut d, mm(30.0, 0.0), mm(39.2, 0.0));
                [mm(-30.0, -20.0), mm(-30.0, 20.0), mm(39.2, -20.0), mm(39.2, 20.0)]
            } else {
                drag(&mut ed, &mut d, mm(30.0, -20.0), mm(39.19, -14.22));
                [mm(-30.0, -14.22), mm(-30.0, 20.0), mm(39.19, -14.22), mm(39.19, 20.0)]
            };
            let got = corners(&d);
            assert!(near(&got, &want), "side {side}: {:?}", got.iter().map(|p| *p * 1e3).collect::<Vec<_>>());
        }
        // Live psui fc-1120 (2), sketchdrag-b: the bottom line dimensioned 30 mm, the top-right corner dragged: the width
        // holds, the rectangle slides with the corner, the top follows it, the bottom keeps its height.
        let (mut ed, mut d) = editor(Tool::CornerRect);
        click(&mut ed, &mut d, mm(19.8, 10.48));
        click(&mut ed, &mut d, mm(49.8, 30.34));
        ed.set_tool(Tool::Select);
        let bottom = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).map(|e| e.id).find(|&l| matches!(d.sketch.curve(l), Some(Curve2::Line { a, b }) if (a.y - 10.48e-3).abs() < 1e-9 && (b.y - 10.48e-3).abs() < 1e-9)).expect("the bottom line");
        d.sketch.add_constraint(ConstraintKind::Length { curve: bottom, value: 30e-3 }).unwrap();
        drag(&mut ed, &mut d, mm(49.8, 30.34), mm(71.97, 46.71));
        let got = corners(&d);
        let want = [mm(41.97, 10.48), mm(71.97, 10.48), mm(41.97, 46.71), mm(71.97, 46.71)];
        assert!(near(&got, &want), "dimensioned: {:?}", got.iter().map(|p| *p * 1e3).collect::<Vec<_>>());
    }

    /// Live2 sketch-drags (11-17, drags.json; 40 px = 10.795 mm): a circle's rim resizes it and its centre moves it;
    /// an arc's end keeps its radius and other end until the chord passes the diameter (then a semicircle on it), its
    /// middle keeps the centre, its centre keeps the end; a shared end takes both lines; a LENGTH line's end
    /// translates it, with the other end fixed it swings; a fixed point stays; an ellipse's rim scales it about its
    /// centre. Within a pixel (0.27 mm) of live's.
    #[test]
    fn curves_drag_as_lives() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let d = 10.795;
        let near = |got: Option<DVec2>, want: (f64, f64)| got.is_some_and(|g| (g - mm(want.0, want.1)).length() < 0.27e-3);
        let run = |setup: &dyn Fn(&mut Sketch) -> EntityId, from: (f64, f64), by: (f64, f64)| {
            let (mut ed, mut data) = editor(Tool::Select);
            let e = setup(&mut data.sketch);
            drag(&mut ed, &mut data, mm(from.0, from.1), mm(from.0 + by.0, from.1 + by.1));
            (data.sketch, e)
        };
        // (a) The circle.
        let circle = |sk: &mut Sketch| sk.add_circle(mm(-150.0, 0.0), 20e-3);
        for (from, want_c, want_r) in [((-130.0, 0.0), (-150.0, 0.0), 30.795), ((-150.0, 0.0), (-139.054, 0.0), 20.0)] {
            let (sk, c) = run(&circle, from, (d, 0.0));
            let Some(Curve2::Circle { center, radius }) = sk.curve(c) else { panic!() };
            assert!(near(Some(center), want_c) && (radius * 1e3 - want_r).abs() < 0.27, "circle from {from:?}: {center:?} {radius}");
        }
        // (b) The arc: centre (-80, -15), r 25, from (-60, 0) to (-100, 0).
        let arc = |sk: &mut Sketch| sk.add_arc(mm(-80.0, -15.0), mm(-60.0, 0.0), mm(-100.0, 0.0));
        let cases = [
            ((-60.0, 0.0), (0.0, d), (-76.349, -8.102), 25.0, (-59.981, 10.795), (-100.0, 0.0)),
            ((-60.0, 0.0), (d, 0.0), (-74.593, 0.0), 25.407, (-49.186, 0.0), (-100.0, 0.0)),
            ((-60.0, 0.0), (d, -6.7469), (-74.593, -3.373), 25.630, (-49.186, -6.747), (-100.0, 0.0)),
            ((-80.0, 10.0), (0.0, d), (-80.0, -15.0), 35.795, (-51.364, 6.477), (-108.636, 6.477)),
            ((-80.0, -15.0), (d, 0.0), (-69.157, -14.843), 34.229, (-51.185, 14.288), (-100.0, 0.0)),
        ];
        for (from, by, c, r, s0, e0) in cases {
            let (sk, a) = run(&arc, from, by);
            let Some(noshape_sketch::Geometry::Arc { center, start, end }) = sk.entity(a).map(|e| e.geometry.clone()) else { panic!() };
            let radius = (sk.point(start).unwrap() - sk.point(center).unwrap()).length() * 1e3;
            assert!(near(sk.point(center), c) && (radius - r).abs() < 0.27 && near(sk.point(start), s0) && near(sk.point(end), e0), "arc from {from:?} by {by:?}: {:?} r {radius} {:?} {:?}", sk.point(center).map(|p| p * 1e3), sk.point(start).map(|p| p * 1e3), sk.point(end).map(|p| p * 1e3));
        }
        // (c) The shared end of two chained lines: both follow, their far ends kept.
        let chain = |sk: &mut Sketch| {
            let (a, o, c) = (sk.add_point(mm(-29.755, -51.816)), sk.add_point(mm(0.0, -59.912)), sk.add_point(mm(20.172, -39.941)));
            sk.add_line_between(a, o).unwrap();
            sk.add_line_between(o, c).unwrap();
            o
        };
        let (sk, o) = run(&chain, (0.0, -59.912), (0.0, -d));
        assert!(near(sk.point(o), (0.0, -70.707)) && sk.entities().filter(|e| e.kind() == EntityKind::Point).filter(|e| e.id != o).count() == 2, "shared end");
        // (d) A LENGTH 40 line: an end along or across translates it; with its start fixed it swings (along: stays).
        let sized = |fixed: bool| {
            move |sk: &mut Sketch| {
                let l = sk.add_line(mm(60.0, 0.0), mm(100.0, 0.0));
                sk.add_constraint(ConstraintKind::Length { curve: l, value: 40e-3 }).unwrap();
                if fixed {
                    let (s, _) = sk.endpoints(l).unwrap();
                    sk.add_constraint(ConstraintKind::Fix { entity: s }).unwrap();
                }
                l
            }
        };
        for (fixed, by, s0, e0) in [
            (false, (d, 0.0), (70.849, 0.0), (110.849, 0.0)),
            (false, (0.0, d), (60.054, 10.795), (100.054, 10.795)),
            (true, (0.0, d), (60.0, 0.0), (98.601, 10.488)),
            (true, (d, 0.0), (60.0, 0.0), (100.0, 0.0)),
        ] {
            let (sk, l) = run(&sized(fixed), (100.0, 0.0), by);
            let Some(Curve2::Line { a, b }) = sk.curve(l) else { panic!() };
            assert!(near(Some(a), s0) && near(Some(b), e0), "length line, fixed {fixed}, by {by:?}: {:?} {:?}", a * 1e3, b * 1e3);
        }
        // (e) A fixed point doesn't move.
        let fixed = |sk: &mut Sketch| {
            let p = sk.add_point(mm(150.0, 0.0));
            sk.add_constraint(ConstraintKind::Fix { entity: p }).unwrap();
            p
        };
        let (sk, p) = run(&fixed, (150.0, 0.0), (d, 0.0));
        assert!(near(sk.point(p), (150.0, 0.0)), "fixed point");
        // (f) The ellipse's rim on its major axis: both axes scale, the centre kept.
        let ellipse = |sk: &mut Sketch| sk.add_ellipse(mm(230.0, 0.0), mm(30.0, 0.0), 15e-3);
        let (sk, e) = run(&ellipse, (260.0, 0.0), (d, 0.0));
        let Some(noshape_sketch::Geometry::Ellipse { center, major, minor }) = sk.entity(e).map(|x| x.geometry.clone()) else { panic!() };
        assert!(near(sk.point(center), (230.0, 0.0)) && (major.length() * 1e3 - 40.525).abs() < 0.3 && (minor * 1e3 - 20.263).abs() < 0.15, "ellipse: {major:?} {minor}");
        // live2 run4 (8.41 mm a 40 px): the minor-axis rim follows the pointer's distance from the centre, the ratio
        // kept; a chain's L2 dragged by its middle keeps its direction and moves across itself through the pointer, its
        // free end straight across, the shared end along L1 (L1's far end and direction kept).
        let (sk, e) = run(&ellipse, (229.899, 15.033), (0.0, 8.41));
        let Some(noshape_sketch::Geometry::Ellipse { major, minor, .. }) = sk.entity(e).map(|x| x.geometry.clone()) else { panic!() };
        assert!((minor * 1e3 - 23.41).abs() < 0.1 && (major.length() * 1e3 - 46.82).abs() < 0.2, "ellipse minor rim: {major:?} {minor}");
        let chain2 = |sk: &mut Sketch| {
            let (a, o, c) = (sk.add_point(mm(-30.0, -50.0)), sk.add_point(mm(0.0, -60.0)), sk.add_point(mm(20.0, -40.0)));
            sk.add_line_between(a, o).unwrap();
            sk.add_line_between(o, c).unwrap()
        };
        let (sk, l2) = run(&chain2, (10.0, -50.0), (8.41, 0.0));
        let Some(Curve2::Line { a, b }) = sk.curve(l2) else { panic!() };
        let far = sk.entities().filter(|e| e.kind() == EntityKind::Point).filter_map(|e| sk.point(e.id)).any(|p| (p - mm(-30.0, -50.0)).length() < 1e-9);
        assert!(near(Some(a), (6.3075, -62.1025)) && near(Some(b), (24.205, -44.205)) && far, "chain L2 by its middle: {:?} {:?}", a * 1e3, b * 1e3);
        // An arc's end dragged level with its other end: the horizontal glyph by the pointer, HORIZONTAL on release
        // (live2 b1x); 25 px below, none (b1y).
        for (by, level) in [((d, 0.0), true), ((d, -6.7469), false)] {
            let (mut ed, mut data) = editor(Tool::Select);
            arc(&mut data.sketch);
            let (a, b) = (mm(-60.0, 0.0), mm(-60.0 + by.0, by.1));
            ed.pointer_move(&mut data, a, false, false);
            let h = data.sketch.pick(a, PX * 4.0);
            ed.hover = h;
            ed.tool_down(&mut data, a, h, false, false);
            for k in 1..=4 {
                ed.tool_drag(&mut data, a.lerp(b, k as f64 / 4.0));
            }
            assert_eq!(ed.snap_icons(), if level { vec!["horizontal"] } else { Vec::<&str>::new() }, "by {by:?}");
            ed.tool_up(&mut data, b);
            let horizontal = data.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { b: Some(_), .. }));
            assert_eq!(horizontal, level, "by {by:?}");
        }
    }

    /// Live (feel/sketch.json "boxA": A selected, then a right-to-left box over B): a box adds to the selection.
    #[test]
    fn a_box_adds_to_the_selection() {
        let (mut ed, mut d) = editor(Tool::Select);
        let a = d.sketch.add_line(v(0.0, 0.01), v(0.02, 0.01));
        let b = d.sketch.add_line(v(0.0, 0.03), v(0.02, 0.03));
        ed.selection = vec![a];
        drag(&mut ed, &mut d, v(0.015, 0.035), v(0.005, 0.025));
        assert!(ed.selection.contains(&a) && ed.selection.contains(&b), "{:?}", ed.selection);
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
        // It stays the user's point, Coincident with the (hidden) Origin.
        assert_eq!(d.sketch.point(p), Some(DVec2::ZERO));
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Coincident { a, b } if a == p && d.sketch.entity(b).is_some_and(|e| e.reference_only))));
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

    /// Live (behaviours "qa (2026-10-28) SKETCH-FILLET-STUCK-RADIUS: two
    /// corners in one fillet session"): every fillet keeps its own radius
    /// dimension (no Equal), its value box open.
    #[test]
    fn fillet_vertex_then_own_radius() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.02));
        let (p0, p1) = (d.sketch.point(r.points[0]).unwrap(), d.sketch.point(r.points[1]).unwrap());
        click(&mut ed, &mut d, p0);
        assert_eq!(ed.selection, vec![r.points[0]], "the corner stays selected until confirmed (live ui-parity sketch-fillet-corner)");
        assert!(ed.commit_corner(&mut d).changed);
        assert!(ed.selection.is_empty(), "confirming clears it");
        assert!(ed.dim_edit.is_some(), "radius box after the first fillet");
        ed.dim_edit = None;
        click(&mut ed, &mut d, p1);
        assert!(ed.commit_corner(&mut d).changed);
        assert!(ed.dim_edit.is_some(), "and after the second");
        assert_eq!(count(&d, EntityKind::Arc), 2);
        assert!(!d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Equal { .. })));
        assert_eq!(d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::Radius { .. })).count(), 2);
    }

    /// Live (sketch/110, 1933–1935; qa FEEL-SKFILLET-FLOW): a hovered corner shows no arc, the click previews it with
    /// its arrow, and Enter drops the arrow for the radius dimension's value box.
    #[test]
    fn fillet_arc_comes_on_the_click_and_its_arrow_goes_on_enter() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.02));
        ed.hover = Some(r.points[0]);
        assert!(ed.fillet_preview(&d.sketch).is_none() && ed.fillet_arrow(&d.sketch).is_none(), "hover: only the point");
        let corner = d.sketch.point(r.points[0]).unwrap();
        click(&mut ed, &mut d, corner);
        assert!(ed.fillet_preview(&d.sketch).is_some() && ed.fillet_arrow(&d.sketch).is_some(), "the click: arc and arrow");
        assert!(ed.commit_corner(&mut d).changed && ed.dim_edit.is_some());
        assert!(ed.fillet_arrow(&d.sketch).is_none() && ed.fillet_value_label(&d.sketch).is_none(), "Enter: the value box only");
    }

    /// The arc's middle at `r` for the corner `pid` (where the arrow drags it).
    fn fillet_mid(d: &SketchData, pid: EntityId, r: f64) -> DVec2 {
        corner_arc(&d.sketch, pid, r).unwrap().mid()
    }

    /// Live (same behaviours; SKETCH-FILLET-STUCK-RADIUS): a default too large
    /// for the corner previews red at that radius, with no value box, and
    /// Enter does nothing; dragging its arrow to a radius that fits makes the
    /// preview, which commits with its own radius and types smaller.
    #[test]
    fn a_default_radius_too_large_for_the_corner_previews_red() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let r = d.sketch.corner_rectangle(v(0.1, 0.0), v(0.115, 0.011));
        let pid = r.points[0];
        let corner = d.sketch.point(pid).unwrap();
        ed.fillet_radius = 0.12;
        click(&mut ed, &mut d, corner);
        assert!(ed.pending_corner.is_none() && ed.invalid_corner == Some(pid), "a red preview: {:?}", ed.message);
        assert!(ed.fillet_preview(&d.sketch).is_some_and(|(_, fits)| !fits), "drawn red");
        assert_eq!(ed.fillet_value_label(&d.sketch).map(|(_, r)| r), Some(0.12), "its label reads the default");
        assert!(!ed.commit_corner(&mut d).changed, "Enter does nothing");
        assert!(ed.dim_edit.is_none() && count(&d, EntityKind::Arc) == 0);
        // The arrow dragged in to R5: a valid preview.
        assert!(ed.fillet_arrow(&d.sketch).is_some());
        ed.drag = Some(DragState::FilletInvalid);
        let m5 = fillet_mid(&d, pid, 0.005);
        ed.tool_drag(&mut d, m5);
        assert!(ed.invalid_corner.is_none() && ed.pending_corner.is_some());
        assert!((ed.fillet_radius - 0.005).abs() < 1e-9, "the default follows the radius shown: {}", ed.fillet_radius);
        ed.tool_up(&mut d, m5);
        assert!(ed.commit_corner(&mut d).changed);
        assert!(ed.dim_edit.is_some(), "its value box");
        assert!(!d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Equal { .. })));
    }

    /// Live: an over-drag turns the preview red (the default follows the
    /// radius shown); released there, a toast and nothing made; dragged back,
    /// the preview is valid again.
    #[test]
    fn fillet_overdrag_turns_red_and_drags_back() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.023, 0.015));
        let pid = r.points[0];
        let corner = d.sketch.point(pid).unwrap();
        click(&mut ed, &mut d, corner);
        let arc = ed.pending_corner.as_ref().and_then(|(pv, res)| res.created.iter().copied().find(|e| pv.sketch.kind(*e) == Some(EntityKind::Arc))).expect("a preview");
        ed.drag = Some(DragState::FilletRadius { arc });
        let far = fillet_mid(&d, pid, 0.05);
        ed.tool_drag(&mut d, far);
        assert!(ed.pending_corner.is_none() && ed.invalid_corner == Some(pid), "red past the corner");
        assert!((ed.fillet_radius - 0.05).abs() < 1e-6, "{}", ed.fillet_radius);
        ed.tool_up(&mut d, far);
        assert_eq!(ed.toast.as_deref(), Some("The current fillet radius produces invalid geometry."));
        assert!(!ed.commit_corner(&mut d).changed && count(&d, EntityKind::Arc) == 0);
        ed.drag = Some(DragState::FilletInvalid);
        let m4 = fillet_mid(&d, pid, 0.004);
        ed.tool_drag(&mut d, m4);
        assert!(ed.pending_corner.is_some(), "valid again");
    }

    /// Live (c): a fillet using a whole leg types smaller; the leg grows back.
    #[test]
    fn a_fillet_using_a_whole_leg_types_smaller() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let c = d.sketch.add_point(v(0.0, 0.0));
        let a = d.sketch.add_point(v(0.02186, 0.0));
        let b = d.sketch.add_point(v(0.0, 0.03));
        d.sketch.add_line_between(c, a).unwrap();
        d.sketch.add_line_between(c, b).unwrap();
        ed.fillet_radius = 0.021859;
        click(&mut ed, &mut d, v(0.0, 0.0));
        assert!(ed.pending_corner.is_some(), "{:?}", ed.message);
        let (arc, _) = ed.fillet_handle().unwrap();
        assert!(ed.commit_corner(&mut d).changed);
        let dim = d.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::Radius { curve, .. } if curve == arc)).map(|c| c.id).expect("its radius");
        let r = ed.apply_dimension(&mut d, dim, "5", &UnitPrefs::default(), &std::collections::BTreeMap::<String, noshape_units::Quantity>::new());
        assert!(r.is_ok(), "{r:?}");
        assert!(matches!(d.sketch.curve(arc), Some(Curve2::Arc { radius, .. }) if (radius - 0.005).abs() < 1e-9), "{:?}", d.sketch.curve(arc));
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
        assert!(ed.selection.is_empty(), "nothing selected after ✓ (live sketch-text-font-typed-ok)");
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

    /// Live (behaviours "sketch snapping and hover", 11-12): a line's end snapped to a point or a midpoint shows a
    /// yellow-orange square round it; snapped onto a line, no square (its glyph only).
    #[test]
    fn a_snapped_point_or_midpoint_gets_the_square_and_a_curve_none() {
        let (mut ed, mut d) = editor(Tool::Line);
        d.sketch.add_line(v(0.0, 0.0), v(200.0 * PX, 0.0));
        let squares = |ed: &SketchEditor, d: &SketchData| ed.overlay(d, &noshape_units::UnitPrefs::default()).0.segments.iter().filter(|s| s.color == SNAP_SQUARE_COLOR).count();
        click(&mut ed, &mut d, v(60.0 * PX, 80.0 * PX));
        for (at, kind, square) in [(v(203.0 * PX, 4.0 * PX), InferenceKind::Endpoint, true), (v(103.0 * PX, 3.0 * PX), InferenceKind::Midpoint, true), (v(150.0 * PX, 3.0 * PX), InferenceKind::OnCurve, false)] {
            ed.pointer_move(&mut d, at, false, false);
            assert_eq!(ed.inference.as_ref().map(|i| i.kind), Some(kind), "at {at:?}");
            assert_eq!(squares(&ed, &d) == 4, square, "{kind:?}: {} square sides", squares(&ed, &d));
        }
    }

    /// Qa SKT-USE-HOVER (live sketch-tools-1123 use-convert, sk23 use-0-hover-edge): Use draws the hovered edge as a light
    /// orange band ~5 px with the edge's darker core along it and small light-blue dots on its ends, not a light-blue
    /// line; a closed loop has no end dots.
    #[test]
    fn use_draws_the_hovered_edge_as_hovered() {
        let (mut ed, d) = editor(Tool::Use);
        let units = noshape_units::UnitPrefs::default();
        ed.use_preview = vec![vec![v(0.0, 0.0), v(100.0 * PX, 0.0)]];
        let (lines, points) = ed.overlay(&d, &units);
        let band = |c: [u8; 4], w: f32| lines.segments.iter().position(|s| s.color == c && s.width == w);
        assert!(band(USE_HOVER_COLOR, 5.0).is_some() && !lines.segments.iter().any(|s| s.color == PREVIEW_COLOR), "the edge light orange, no light-blue line");
        assert!(band(USE_HOVER_EDGE_COLOR, 2.5) > band(USE_HOVER_COLOR, 5.0), "the edge's darker core over the band's middle");
        assert_eq!(points.points.iter().filter(|p| p.color == PREVIEW_COLOR).count(), 2, "its two end dots");
        ed.use_preview = vec![(0..=16).map(|k| DVec2::from_angle(f64::from(k) / 16.0 * std::f64::consts::TAU) * (50.0 * PX)).collect()];
        assert!(ed.overlay(&d, &units).1.points.is_empty(), "a circle: no end dots");
    }

    /// Live hover-matrix-recheck-1123: the Line tool's end dot, snapped onto a point, a midpoint or a curve, is drawn
    /// under the lit target (the dot through the point's disc, #C3B89C), not over it; elsewhere it stays on top.
    #[test]
    fn the_line_end_dot_goes_under_a_lit_target() {
        let (mut ed, mut d) = editor(Tool::Line);
        d.sketch.add_line(v(0.0, 0.0), v(200.0 * PX, 0.0));
        let units = noshape_units::UnitPrefs::default();
        click(&mut ed, &mut d, v(60.0 * PX, 80.0 * PX));
        for (at, under) in [(v(203.0 * PX, 4.0 * PX), true), (v(103.0 * PX, 3.0 * PX), true), (v(150.0 * PX, 3.0 * PX), true), (v(300.0 * PX, 200.0 * PX), false)] {
            ed.pointer_move(&mut d, at, false, false);
            // The rubber band's end: where the snap puts it.
            let end = ed.frame.to_world(ed.snapped().map_or(at, |s| s.0));
            let at_end = |p: &PointSprite| p.size == 6.0 && p.color[..3] == PREVIEW_COLOR[..3] && glam::DVec3::new(f64::from(p.position[0]), f64::from(p.position[1]), f64::from(p.position[2])).distance(end) < 1e-5;
            let (over, below) = (ed.overlay(&d, &units).1.points.iter().filter(|p| at_end(p)).count(), ed.end_dot_under_target(&d).points.iter().filter(|p| at_end(p)).count());
            // (In free space the Line's end dot and the cursor dot coincide.)
            assert!(if under { (over, below) == (0, 1) } else { over >= 1 && below == 0 }, "at {at:?} ({:?}): end dots over {over}, under {below}", ed.inference.as_ref().map(|i| i.kind));
        }
    }

    /// Live (feel retaken 11-18): a point counts for alignment once a snap has landed on it, with no rest (a 10 ms rest
    /// or a slow pass woke it; its wakeupInferenceSource), and stays woken; passing by 9 px off it, outside its 15 px
    /// square, doesn't (the origin always counts).
    #[test]
    fn a_point_aligns_once_a_snap_lands_on_it() {
        for (off, snapped) in [(9.0, false), (5.0, true), (0.0, true)] {
            let (mut ed, mut d) = editor(Tool::Line);
            let pt = d.sketch.add_point(v(200.0 * PX, 300.0 * PX));
            click(&mut ed, &mut d, v(500.0 * PX, 100.0 * PX));
            ed.pointer_move(&mut d, v((200.0 + off) * PX, 300.0 * PX), false, false);
            ed.pointer_move(&mut d, v(203.0 * PX, 120.0 * PX), false, false);
            let aligned = ed.inference.as_ref().is_some_and(|i| i.constraints.iter().any(|c| matches!(c, AutoConstraint::VerticalWith(p) if *p == pt)));
            assert_eq!(aligned, snapped, "passed {off} px off the point: {:?}", ed.inference);
        }
    }

    /// Live ui-parity revolve-dialog-axis / sketch-mirror-undo-midway: the point the pointer rests on when the tool
    /// starts doesn't wake while the pointer stays on it; it does once the pointer comes back onto it.
    #[test]
    fn a_point_under_the_pointer_at_the_tools_start_wakes_only_on_arriving() {
        let (mut ed, mut d) = editor(Tool::Select);
        let pt = d.sketch.add_point(v(200.0 * PX, 300.0 * PX));
        ed.hover = Some(pt);
        ed.set_tool(Tool::Line);
        ed.pointer_move(&mut d, v(202.0 * PX, 301.0 * PX), false, false);
        click(&mut ed, &mut d, v(500.0 * PX, 100.0 * PX));
        ed.pointer_move(&mut d, v(203.0 * PX, 120.0 * PX), false, false);
        let aligned = |ed: &SketchEditor| ed.inference.as_ref().is_some_and(|i| i.constraints.iter().any(|c| matches!(c, AutoConstraint::VerticalWith(p) if *p == pt)));
        assert!(!aligned(&ed), "{:?}", ed.inference);
        ed.pointer_move(&mut d, v(200.0 * PX, 300.0 * PX), false, false);
        ed.pointer_move(&mut d, v(203.0 * PX, 120.0 * PX), false, false);
        assert!(aligned(&ed), "{:?}", ed.inference);
    }

    /// Live errors-1118: a dimension the Dimension tool added driven (it over-defines a dimensioned rectangle) takes a
    /// typed value as a driving dimension, the redundant 40 as the conflicting 50: kept, the sketch not solved, its
    /// shape as it was, both it and the opposite 40 red. Enter on the untouched box (no value typed) leaves it driven (live qa4 r5 s5).
    /// Live shortcuts-sk-1118: a draw tool started from its key with the pointer resting draws nothing at the pointer
    /// until it moves (ours drew the tool's blue dot there).
    #[test]
    fn a_draw_tool_from_its_key_draws_nothing_until_the_pointer_moves() {
        for tool in [Tool::Circle, Tool::CenterRect, Tool::CornerRect, Tool::Line, Tool::Arc3, Tool::Point] {
            let (mut ed, mut d) = editor(Tool::Select);
            let units = UnitPrefs::default();
            let _ = ed.pointer_move(&mut d, v(0.01, 0.01), false, false);
            ed.set_tool(tool);
            let (_, points) = ed.overlay(&d, &units);
            assert!(points.points.is_empty(), "{tool:?}: {} points at the resting pointer", points.points.len());
            let _ = ed.pointer_move(&mut d, v(0.01, 0.01), false, false);
            assert!(ed.overlay(&d, &units).1.points.is_empty(), "{tool:?}: a move to the same spot isn't a move");
            let _ = ed.pointer_move(&mut d, v(0.011, 0.01), false, false);
            assert!(!ed.overlay(&d, &units).1.points.is_empty(), "{tool:?}: drawn once the pointer moves");
        }
    }

    /// Feel row "Dimension arrows" (live sketch/12: 16 × 7 px): a dimension's arrowhead is filled from its tip back
    /// 16 px, 7 px across its base, at any zoom (`s` world per px).
    #[test]
    fn a_dimension_arrowhead_is_16_by_7_px() {
        for s in [1.0, 2.5e-4] {
            let mut out = Vec::new();
            arrow(&mut out, DVec2::new(3.0, 4.0) * s, DVec2::new(0.0, -1.0), s);
            assert!(out.iter().all(|[a, _]| (*a - DVec2::new(3.0, 4.0) * s).length() < 1e-12), "every stroke from the tip");
            let along: Vec<f64> = out.iter().map(|[_, b]| (b.y / s - 4.0).abs()).collect();
            let across: Vec<f64> = out.iter().map(|[_, b]| b.x / s - 3.0).collect();
            assert!(along.iter().all(|l| (l - 16.0).abs() < 1e-6), "16 px long: {along:?}");
            let w = across.iter().cloned().fold(f64::MIN, f64::max) - across.iter().cloned().fold(f64::MAX, f64::min);
            assert!((w - 7.0).abs() < 1e-6, "7 px across: {w}");
        }
    }

    /// Feel row "Drawing preview": the geometry being drawn is rgb(68,156,205), 2 px (live SKETCH_PREVIEW_COLOR).
    #[test]
    fn geometry_being_drawn_is_lives_preview_blue() {
        for tool in [Tool::Line, Tool::Circle, Tool::CornerRect] {
            let (mut ed, mut d) = editor(tool);
            click(&mut ed, &mut d, v(0.0, 0.0));
            let _ = ed.pointer_move(&mut d, v(0.03, 0.017), true, false);
            let (lines, _) = ed.overlay(&d, &UnitPrefs::default());
            let blue: Vec<_> = lines.segments.iter().filter(|s| s.color == [68, 156, 205, 255]).collect();
            assert!(!blue.is_empty(), "{tool:?}: the preview drawn in rgb(68,156,205)");
            assert!(blue.iter().all(|s| s.width == 2.0), "{tool:?}: 2 px");
        }
    }

    #[test]
    fn a_typed_value_makes_an_over_defining_dimension_driving() {
        for typed in ["40", "50"] {
            let (mut ed, mut d) = editor(Tool::Dimension);
            let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.03));
            let [bottom, right, top, _left] = [r.curves[0], r.curves[1], r.curves[2], r.curves[3]];
            let forty = d.sketch.add_constraint(ConstraintKind::Length { curve: bottom, value: 0.04 }).unwrap();
            let thirty = d.sketch.add_constraint(ConstraintKind::Length { curve: right, value: 0.03 }).unwrap();
            assert!(d.sketch.solve().is_solved());
            let cid = add_dimension(&mut d, ConstraintKind::Length { curve: top, value: 0.0 }, None, None).unwrap();
            assert!(!d.sketch.constraint(cid).unwrap().driving, "placed driven");
            ed.dim_edit = Some(DimEdit { constraint: cid, text: "40 mm".into(), created: true });
            let shape: Vec<DVec2> = r.points.iter().map(|p| d.sketch.point(*p).unwrap()).collect();
            let res = ed.apply_dimension(&mut d, cid, typed, &UnitPrefs::default(), &noshape_units::EmptyScope);
            assert_eq!(res, Err(SKETCH_NOT_SOLVED.to_string()), "{typed}");
            let c = d.sketch.constraint(cid).unwrap();
            assert!(c.driving && (c.kind.value().unwrap() - typed.parse::<f64>().unwrap() * 1e-3).abs() < 1e-12, "{typed}: {c:?}");
            let rep = d.sketch.solve();
            assert!(!rep.is_solved());
            // Both dimensions red, the redundant 40 as the conflicting 50 (live errors-1118 / recheck 11-19); not the 30.
            let red = |c| rep.conflicting().contains(&c) || rep.redundant.contains(&c);
            assert!(red(cid) && red(forty) && !red(thirty), "{typed}: {:?} / {:?}", rep.conflicting(), rep.redundant);
            for (p, was) in r.points.iter().zip(&shape) {
                assert!((d.sketch.point(*p).unwrap() - *was).length() < 1e-9, "{typed}: the shape stays");
            }
        }
    }

    /// Live align-mid-1118: a pass over a line's midpoint (MIDPOINT) wakes it; the click on the horizontal alignment with it
    /// makes a point there, MIDPOINT on the line, and the new end HORIZONTAL with it; dragging the line's end then keeps
    /// the new end level with the line's midpoint. Without the pass there's no alignment.
    #[test]
    fn a_passed_midpoint_gives_alignment_and_a_point_there() {
        for pass in [false, true] {
            let (mut ed, mut d) = editor(Tool::Line);
            let l = d.sketch.add_line(v(-300.0 * PX, -200.0 * PX), v(-100.0 * PX, 100.0 * PX));
            click(&mut ed, &mut d, v(300.0 * PX, 300.0 * PX));
            if pass {
                ed.pointer_move(&mut d, v(-200.0 * PX, -50.0 * PX), false, false);
                assert_eq!(ed.inference.as_ref().map(|i| i.kind), Some(InferenceKind::Midpoint));
            }
            let at = v(250.0 * PX, -48.0 * PX);
            ed.pointer_move(&mut d, at, false, false);
            let aligned = ed.inference.as_ref().is_some_and(|i| i.kind == InferenceKind::AlignHorizontal);
            assert_eq!(aligned, pass, "passed over the midpoint: {pass}: {:?}", ed.inference);
            if !pass {
                continue;
            }
            click(&mut ed, &mut d, at);
            let q = d.sketch.constraints().find_map(|c| match c.kind {
                ConstraintKind::Midpoint { point, curve } if curve == l => Some(point),
                _ => None,
            });
            let q = q.expect("a point at the midpoint, MIDPOINT on the line");
            assert!((d.sketch.point(q).unwrap() - v(-200.0 * PX, -50.0 * PX)).length() < 1e-9);
            let end = d.sketch.constraints().find_map(|c| match c.kind {
                ConstraintKind::Horizontal { a, b: Some(b) } if b == q => Some(a),
                ConstraintKind::Horizontal { a, b: Some(b) } if a == q => Some(b),
                _ => None,
            });
            let end = end.expect("the new end horizontal with it");
            let le = d.sketch.endpoints(l).unwrap().1;
            assert!(d.sketch.drag(&[(DragHandle::Point(le), v(-100.0 * PX, 200.0 * PX))]).is_solved());
            let (m, e) = (d.sketch.point(q).unwrap(), d.sketch.point(end).unwrap());
            assert!((m.y - e.y).abs() < 1e-9, "the new end follows the midpoint's level: {m} {e}");
        }
    }

    /// Live (feel retaken 11-18, dragged line end): no alignment with points the drag hasn't snapped onto; once it has
    /// snapped onto a point, moving on aligns with it (Vertical on release).
    #[test]
    fn a_drag_aligns_with_a_point_it_snapped_onto() {
        for snap_first in [false, true] {
            let (mut ed, mut d) = editor(Tool::Select);
            let t = d.sketch.add_point(v(100.0 * PX, 100.0 * PX));
            let l = d.sketch.add_line(v(300.0 * PX, 300.0 * PX), v(400.0 * PX, 300.0 * PX));
            let end = d.sketch.endpoints(l).unwrap().1;
            ed.tool_down(&mut d, v(400.0 * PX, 300.0 * PX), Some(end), false, false);
            ed.tool_drag(&mut d, v(380.0 * PX, 280.0 * PX));
            if snap_first {
                ed.tool_drag(&mut d, v(101.0 * PX, 101.0 * PX));
            }
            ed.tool_drag(&mut d, v(103.0 * PX, 200.0 * PX));
            ed.tool_up(&mut d, v(103.0 * PX, 200.0 * PX));
            let at = d.sketch.point(end).unwrap();
            let vertical = d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Vertical { a, b: Some(b) } if (a, b) == (end, t)));
            assert_eq!((vertical, (at.x - 100.0 * PX).abs() < 1e-9), (snap_first, snap_first), "snapped onto it first: {snap_first}, end at {at}");
        }
    }

    /// Live2 "feel (sketch)" (5): a press on an entity becomes a drag only at about 5 px from the press, on screen;
    /// less is a click (it selects).
    #[test]
    fn a_press_drags_from_5_px() {
        for (moved, drags) in [(4.0, false), (6.0, true)] {
            let (mut ed, mut d) = editor(Tool::Select);
            let pt = d.sketch.add_point(v(0.01, 0.01));
            ed.tool_down(&mut d, v(0.01, 0.01), Some(pt), false, false);
            ed.tool_drag(&mut d, v(0.01 + moved * PX, 0.01));
            ed.tool_up(&mut d, v(0.01 + moved * PX, 0.01));
            let at = d.sketch.point(pt).unwrap();
            assert_eq!(((at - v(0.01, 0.01)).length() > 1e-9, ed.selection.contains(&pt)), (drags, !drags), "moved {moved} px");
        }
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

    /// Live workflow 32 step 42 (qa FEEL-DIM-SELECTED-REF-LINE-YELLOW): a selected dimension with the pointer still on
    /// it lights its line pre-highlight orange, not the selection's translucent yellow; the pointer off, yellow.
    #[test]
    fn a_selected_dimension_hovered_lights_its_line_orange() {
        let (mut ed, mut d) = editor(Tool::Select);
        let line = d.sketch.add_line(v(0.0, 0.0), v(0.04, 0.0));
        let cid = d.sketch.add_constraint(ConstraintKind::Length { curve: line, value: 0.04 }).unwrap();
        ed.selected_constraints = vec![cid];
        let orange = crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT);
        let halo = [0xf2, 0xff, 0x80, 150];
        let looks = |ed: &SketchEditor| {
            let (lines, _) = ed.overlay(&d, &noshape_units::UnitPrefs::default());
            (lines.segments.iter().any(|s| s.color == orange && s.width >= 3.0), lines.segments.iter().any(|s| s.color == halo))
        };
        let off = looks(&ed);
        ed.hover_constraint = Some(cid);
        assert_eq!((off, looks(&ed)), ((false, true), (true, false)), "(orange, yellow) selected, then hovered too");
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
        // Live: 15 previews the original, 5 copies and a dashed 7th.
        ed.pattern_second_count = 1;
        ed.pattern_count = 15;
        let shown = ed.pattern_transforms();
        assert_eq!(shown.len(), 6);
        assert_eq!(shown.iter().filter(|(_, dashed)| *dashed).count(), 1);
        assert!(shown[5].1);
        ed.pattern_count = 3;
        ed.pattern_second_count = 2;
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

    /// Live (ui-parity sketch-mirror-tool): once a curve is mirrored nothing stays selected and the prompt stays.
    #[test]
    fn mirror_leaves_nothing_selected() {
        let (mut ed, mut d) = editor(Tool::Mirror);
        d.sketch.add_line(v(0.0, -0.01), v(0.0, 0.03));
        d.sketch.add_line(v(0.005, 0.0), v(0.02, 0.01));
        click(&mut ed, &mut d, v(0.0, 0.02));
        assert_eq!(ed.message.as_deref(), Some("Select entities to be mirrored."));
        click(&mut ed, &mut d, v(0.00875, 0.0025));
        assert_eq!(count(&d, EntityKind::Line), 3);
        assert!(ed.selection.is_empty(), "{:?}", ed.selection);
    }

    #[test]
    fn a_click_in_space_places_the_offset_as_previewed() {
        let (mut ed, mut d) = editor(Tool::Offset);
        d.sketch.add_line(v(-0.005, 0.005), v(0.025, 0.005));
        click(&mut ed, &mut d, v(0.018, 0.005));
        assert!(ed.offset_pending.is_some(), "picking the line previews its offset");
        // Live (workflows-1122 22 s16–18): previewed right of the line's direction, below; the click 9 mm above makes it
        // there, 5 mm below, and opens its distance box.
        let (tmp, created) = ed.offset_result(&d.sketch).unwrap();
        assert!(tmp.curve(created[0]).unwrap().start_point().y < 0.005, "previewed below");
        click(&mut ed, &mut d, v(0.018, 0.014));
        assert_eq!(count(&d, EntityKind::Line), 2);
        let cid = ed.dim_edit.as_ref().expect("its distance box").constraint;
        assert!(matches!(d.sketch.constraint(cid).map(|c| &c.kind), Some(ConstraintKind::Distance { value, .. }) if (*value - 0.005).abs() < 1e-9));
        let below = d.sketch.entities().filter_map(|e| d.sketch.curve(e.id)).any(|c| c.start_point().y.abs() < 1e-9);
        assert!(below, "as previewed, 5 mm below");
        // Its distance sits between the lines at their midpoint (live 22 s35), not pushed along them.
        let kind = d.sketch.constraint(cid).unwrap().kind.clone();
        let at = ed.label_pos(&d, cid, &kind).unwrap();
        assert!(at.distance(v(0.01, 0.0025)) < 1e-9, "{at:?}");
        // The picked line clicked again: no offset, nothing previewed (live ui-parity sketch-offset).
        let (mut ed, mut d) = editor(Tool::Offset);
        d.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
        click(&mut ed, &mut d, v(0.0075, 0.0));
        click(&mut ed, &mut d, v(0.02, 0.0));
        assert!(ed.offset_pending.is_none() && ed.selection.is_empty());
        assert_eq!(count(&d, EntityKind::Line), 1);
        // With nothing previewed a click in space makes nothing.
        let (mut ed, mut d) = editor(Tool::Offset);
        d.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
        click(&mut ed, &mut d, v(0.08, 0.08));
        assert_eq!(count(&d, EntityKind::Line), 1);
    }

    #[test]
    fn offset_preview_arrow_and_repeat() {
        let (mut ed, mut d) = editor(Tool::Offset);
        let l1 = d.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        d.sketch.add_line(v(0.01, 0.03), v(0.03, 0.03));
        click(&mut ed, &mut d, v(0.02, 0.01));
        assert!(ed.offset_pending.is_some(), "picking a curve previews the offset");
        assert_eq!(ed.selection, vec![l1], "the picked curve stays selected (live)");
        assert_eq!(count(&d, EntityKind::Line), 2, "nothing is created yet");
        let (base, tip) = ed.offset_arrow(&d.sketch).unwrap();
        let (u, from) = ((tip - base).normalize(), ed.offset_distance);
        // Live 11-18 sketch-tools: the drag adds to the offset at the press, on 0.1 mm steps (5 mm and a 5.08 mm drag:
        // 10.1 mm); on over the curve the offset goes to its other side.
        ed.pointer_move(&mut d, tip, false, false);
        ed.tool_down(&mut d, tip, None, false, false);
        ed.tool_drag(&mut d, tip + u * 0.00508);
        assert!((ed.offset_distance - ((from + 0.00508) / 1e-4).round() * 1e-4).abs() < 1e-12, "{} from {from}", ed.offset_distance);
        for k in 1..=4 {
            ed.tool_drag(&mut d, (tip + u * 0.00508).lerp(tip - u * (from + 0.004), k as f64 / 4.0));
        }
        ed.tool_up(&mut d, tip - u * (from + 0.004));
        assert!((ed.offset_distance - 0.004).abs() < 1e-9, "{}", ed.offset_distance);
        let y = 0.01 - u.y * 0.004;
        assert!(ed.commit_offset(&mut d).changed);
        let cid = ed.dim_edit.as_ref().expect("Enter opens the distance box").constraint;
        assert!(matches!(d.sketch.constraint(cid).map(|c| &c.kind), Some(ConstraintKind::Distance { .. })));
        ed.dim_edit = None;
        let beyond = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).filter_map(|e| d.sketch.curve(e.id)).any(|c| (c.start_point().y - y).abs() < 1e-9);
        assert!(beyond, "offset toward the dragged side");
        // later clicks repeat the offset right away
        click(&mut ed, &mut d, v(0.02, 0.03));
        assert_eq!(count(&d, EntityKind::Line), 4);
        let _ = l1;
    }

    /// Live offset-look-1121 (3): the pointer on the distance label hides the dimension's two arrowheads, not its line.
    #[test]
    fn the_offset_label_hovered_hides_its_arrowheads() {
        let (mut ed, mut d) = editor(Tool::Offset);
        d.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01));
        let black = |ed: &SketchEditor| ed.overlay(&d, &UnitPrefs::default()).0.segments.iter().filter(|s| s.color == [0, 0, 0, 255]).count();
        let shown = black(&ed);
        ed.offset_label_hovered = true;
        assert_eq!(black(&ed), shown - 14, "2 arrowheads of 7 strokes go");
        ed.set_tool(Tool::Select);
        assert!(!ed.offset_label_hovered);
    }

    /// Live (behaviours "qa4 round 5 s1 follow-up 2" (3)-(4), qa4 OFF-SIDE,
    /// OFF-TOAST, OFF-BOX): the first side is away from the region (a
    /// rectangle's top edge up, its bottom down, a circle out); a flip is not
    /// remembered by the next offset; a curve off the chain only warns.
    #[test]
    fn offset_side_away_from_the_region_and_one_chain() {
        let (mut ed, mut d) = editor(Tool::Offset);
        d.sketch.corner_rectangle(v(0.0, 0.0), v(0.03, 0.02));
        let circle = d.sketch.add_circle(v(0.1, 0.0), 0.01);
        let copy_y = |ed: &SketchEditor, d: &SketchData| {
            let (tmp, created) = ed.offset_result(&d.sketch).unwrap();
            tmp.curve(created[0]).unwrap().start_point().y
        };
        click(&mut ed, &mut d, v(0.015, 0.02));
        assert!(copy_y(&ed, &d) > 0.02, "top edge: up, out of the rectangle");
        // A curve off the chain: warned, not added.
        click(&mut ed, &mut d, v(0.1, 0.01));
        assert_eq!(ed.toast.take().as_deref(), Some("Can only offset a single chain at once."));
        assert_eq!(ed.offset_pending.as_ref().map(Vec::len), Some(1));
        // A connected edge joins the chain and the side stays out.
        click(&mut ed, &mut d, v(0.03, 0.01));
        assert_eq!(ed.offset_pending.as_ref().map(Vec::len), Some(2));
        assert!(ed.toast.is_none());
        let (tmp, created) = ed.offset_result(&d.sketch).unwrap();
        let bb = created.iter().filter_map(|e| tmp.curve(*e)).fold(noshape_sketch::geom::BBox::from_points([]), |b, c| b.union(&c.bbox()));
        assert!((bb.max.y - 0.025).abs() < 1e-9 && (bb.max.x - 0.035).abs() < 1e-9, "outside the corner: {bb:?}");
        ed.offset_flip = true;
        assert!(ed.cancel());
        ed.set_tool(Tool::Offset);
        click(&mut ed, &mut d, v(0.015, 0.0));
        assert!(!ed.offset_flip, "the flip is not remembered");
        assert!(copy_y(&ed, &d) < 0.0, "bottom edge: down, out of the rectangle");
        assert!(ed.commit_offset(&mut d).changed);
        assert!(ed.dim_edit.is_some(), "Enter opens the value box");
        ed.dim_edit = None;
        ed.offset_repeat = false;
        click(&mut ed, &mut d, v(0.11, 0.0));
        let (tmp, created) = ed.offset_result(&d.sketch).unwrap();
        let Some(Curve2::Circle { radius, .. }) = tmp.curve(created[0]) else { panic!() };
        assert!(radius > 0.01, "a circle offsets outward: {radius}");
        let _ = circle;
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
        assert_eq!(direction_lines(&d), 1, "an edit replaces the direction line with the instances");
        // Esc cancels a pattern being set up
        ed.selection = vec![c];
        assert!(ed.cancel());
        assert!(ed.selection.is_empty());
    }

    fn direction_lines(d: &SketchData) -> usize {
        d.sketch.entities().filter(|e| e.kind() == EntityKind::Line && e.construction).count()
    }

    /// Live sketch-linear-pattern-click-commit: a circle's linear pattern adds no Equal; a construction line runs
    /// from the seed's centre to the first instance's (both ends Coincident), horizontal, its length the spacing.
    #[test]
    fn a_linear_pattern_draws_its_direction_line_as_live() {
        let (mut ed, mut d) = editor(Tool::Select);
        let c = d.sketch.add_circle(v(0.01, 0.01), 0.002);
        ed.selection = vec![c];
        ed.set_tool(Tool::LinearPattern);
        ed.pattern_count = 3;
        ed.pattern_distance = 0.01;
        ed.pattern_angle = 0.0;
        click(&mut ed, &mut d, v(0.04, 0.04));
        let kinds: Vec<&str> = d.sketch.constraints().map(|c| c.kind.name()).collect();
        assert!(!kinds.contains(&"Equal"), "{kinds:?}");
        assert_eq!(direction_lines(&d), 1);
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line && e.construction).unwrap().id;
        let (a, b) = d.sketch.endpoints(line).unwrap();
        assert!((d.sketch.point(a).unwrap() - v(0.01, 0.01)).length() < 1e-9);
        assert!((d.sketch.point(b).unwrap() - v(0.02, 0.01)).length() < 1e-9);
        let on = |p: EntityId| d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Coincident { a, .. } if a == p));
        assert!(on(a) && on(b), "{kinds:?}");
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Horizontal { a, b: None } if a == line)));
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Length { curve, value } if curve == line && (value - 0.01).abs() < 1e-12)));
        assert!(d.sketch.solve().is_solved());
        // The instances keep the seed's radius through the pattern itself.
        d.sketch.add_constraint(ConstraintKind::Radius { curve: c, value: 0.003 }).unwrap();
        assert!(d.sketch.solve().is_solved());
        let radii: Vec<f64> = d.sketch.entities().filter_map(|e| match d.sketch.curve(e.id) { Some(Curve2::Circle { radius, .. }) => Some(radius), _ => None }).collect();
        assert!(radii.iter().all(|r| (r - radii[0]).abs() < 1e-9), "{radii:?}");
    }

    #[test]
    fn fillet_radius_manipulator_and_conic_rho_box() {
        let (mut ed, mut d) = editor(Tool::Fillet);
        let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.02));
        let p0 = d.sketch.point(r.points[0]).unwrap();
        click(&mut ed, &mut d, p0);
        let (arc, m) = ed.fillet_handle().expect("manipulator");
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

    /// Live2 11-17 (REQUESTS (d)): a 5 mm circle 3 mm off a point. Coincident with a fixed point, the circle shifts
    /// 3 mm toward it, its radius kept; with a free point, the point moves onto the circle and the circle stays.
    #[test]
    fn a_circle_coincident_with_a_fixed_point_shifts_and_a_free_point_moves() {
        for fixed in [true, false] {
            let mut sk = Sketch::new();
            let c = sk.add_circle(v(0.0, 0.008), 0.005);
            let p = sk.add_point(v(0.0, 0.0));
            if fixed {
                sk.add_constraint(ConstraintKind::Fix { entity: p }).unwrap();
            }
            assert!(apply_constraint(&mut sk, ConstraintTool::Coincident, &[c, p]).is_ok_and(|n| n > 0));
            assert!(solve_after_constraint(&mut sk, ConstraintTool::Coincident, &[c, p]));
            let Some(Curve2::Circle { center, radius }) = sk.curve(c) else { panic!("a circle") };
            let at = sk.point(p).unwrap();
            assert!((radius - 0.005).abs() < 1e-9, "fixed {fixed}: radius kept, {radius}");
            if fixed {
                assert!((center - v(0.0, 0.005)).length() < 1e-9 && at.length() < 1e-12, "the circle shifts 3 mm: {center}");
            } else {
                assert!((center - v(0.0, 0.008)).length() < 1e-9 && (at - v(0.0, 0.003)).length() < 1e-9, "the point moves: {at}");
            }
        }
    }

    /// Live2 mirror-sym-1118: Symmetric on two lines and an axis: one mirror of the lines, the second line onto the
    /// first's mirror image with its length and place kept; shortening the first then leaves the second as it is.
    #[test]
    fn symmetric_lines_mirror_the_lines_with_their_ends_free() {
        let mut sk = Sketch::new();
        let axis = sk.add_line(v(0.0, -0.05), v(0.0, 0.05));
        sk.add_constraint(ConstraintKind::Fix { entity: axis }).unwrap();
        let a = sk.add_line(v(-0.03, 0.0), v(-0.01, 0.02));
        let b = sk.add_line(v(0.012, 0.0), v(0.05, 0.004));
        let ids = [a, b, axis];
        assert!(apply_constraint(&mut sk, ConstraintTool::Symmetric, &ids).is_ok_and(|n| n == 1));
        assert!(solve_after_constraint(&mut sk, ConstraintTool::Symmetric, &ids));
        let ends = |sk: &Sketch, l: EntityId| match sk.curve(l) {
            Some(Curve2::Line { a, b }) => (a, b),
            _ => panic!("a line"),
        };
        let (a0, a1) = ends(&sk, a);
        let (b0, b1) = ends(&sk, b);
        assert!((a0 - v(-0.03, 0.0)).length() < 1e-9 && (a1 - v(-0.01, 0.02)).length() < 1e-9, "the first stays");
        let mirror = |p: DVec2| v(-p.x, p.y);
        assert!((b1 - b0).normalize().perp_dot((mirror(a1) - mirror(a0)).normalize()).abs() < 1e-9, "b along a's mirror");
        assert!((b0 - mirror(a0)).perp_dot((mirror(a1) - mirror(a0)).normalize()).abs() < 1e-9, "b on a's mirrored line");
        assert!(((b1 - b0).length() - v(0.038, 0.004).length()).abs() < 1e-9, "b's length kept");
        let start = sk.entity(a).and_then(|e| e.point_refs().first().copied()).unwrap();
        assert!(sk.drag(&[(noshape_sketch::DragHandle::Point(start), v(-0.02, 0.01))]).is_solved());
        let (c0, c1) = ends(&sk, b);
        assert!((c0 - b0).length() < 1e-9 && (c1 - b1).length() < 1e-9, "shortening the first leaves the second: {c0} {c1}");
    }

    /// Live re-measured 11-18 (live2's case): B's start goes to the foot of its perpendicular on A's mirrored line.
    #[test]
    fn symmetric_lines_put_the_seconds_start_at_its_foot_as_live() {
        let mut sk = Sketch::new();
        let axis = sk.add_line(v(0.06, -0.05), v(0.06, 0.05));
        sk.add_constraint(ConstraintKind::Fix { entity: axis }).unwrap();
        let a = sk.add_line(v(0.03, -0.01), v(0.05, 0.02));
        let b = sk.add_line(v(0.07, -0.005), v(0.085, 0.015));
        let ids = [a, b, axis];
        assert!(apply_constraint(&mut sk, ConstraintTool::Symmetric, &ids).is_ok_and(|n| n == 1));
        assert!(solve_after_constraint(&mut sk, ConstraintTool::Symmetric, &ids));
        let Some(Curve2::Line { a: b0, b: b1 }) = sk.curve(b) else { panic!("a line") };
        assert!((b0 - v(0.081538, 0.002692)).length() < 1e-6 && (b1 - v(0.067671, 0.023494)).length() < 1e-6, "{b0} {b1}");
    }

    /// Live (c2-surf-intro-mutualtrim's Title, live title-frame-1118): a text box's corner made coincident with a fixed
    /// line 40 mm below it: the box moves down whole, its size and direction kept (the solve alone scaled it).
    #[test]
    fn a_text_box_coincident_with_a_fixed_line_moves_whole() {
        let mut sk = Sketch::new();
        let bx = sk.add_text_box(v(-0.010943, -0.026971), 0.01524, 10.3).unwrap();
        let line = sk.add_line(v(-0.053, -0.067431), v(0.2538, -0.067431));
        sk.add_constraint(ConstraintKind::Fix { entity: line }).unwrap();
        let corner = bx.points[0];
        let before: Vec<DVec2> = bx.points.iter().map(|p| sk.point(*p).unwrap()).collect();
        assert!(apply_constraint(&mut sk, ConstraintTool::Coincident, &[corner, line]).is_ok_and(|n| n > 0));
        assert!(solve_after_constraint(&mut sk, ConstraintTool::Coincident, &[corner, line]));
        for (p, was) in bx.points.iter().zip(&before) {
            let at = sk.point(*p).unwrap();
            assert!((at - (*was + v(0.0, -0.04046))).length() < 1e-9, "corner {p:?}: {was} -> {at}");
        }
    }

    /// Live2 11-17 part 2b: two free lines, L1 picked first, then a constraint from the selection. L1 never moves; L2
    /// moves as live's (mm): Parallel about its midpoint, Perpendicular about where the lines cross, Coincident onto
    /// L1's line, lengths kept; Equal keeps its start and direction.
    #[test]
    fn the_second_picked_line_moves_as_lives() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let cases: [(ConstraintTool, [f64; 4]); 4] = [
            (ConstraintTool::Parallel, [47.82, 16.98, 96.93, 24.37]),
            (ConstraintTool::Perpendicular, [-21.48, 48.60, -28.88, 97.70]),
            (ConstraintTool::Coincident, [51.69, -8.70, 100.80, -1.30]),
            (ConstraintTool::Equal, [49.85, 10.23, 86.51, 27.23]),
        ];
        for (c, want) in cases {
            let mut sk = Sketch::new();
            let l1 = sk.add_line(mm(44.92, -9.72), mm(84.88, -3.70));
            let l2 = sk.add_line(mm(49.85, 10.23), mm(94.90, 31.12));
            assert!(apply_constraint(&mut sk, c, &[l1, l2]).is_ok_and(|n| n > 0), "{c:?}");
            assert!(solve_after_constraint(&mut sk, c, &[l1, l2]), "{c:?}");
            let Some(Curve2::Line { a: a1, b: b1 }) = sk.curve(l1) else { panic!() };
            assert!((a1 - mm(44.92, -9.72)).length() < 1e-9 && (b1 - mm(84.88, -3.70)).length() < 1e-9, "{c:?}: L1 stays");
            let Some(Curve2::Line { a, b }) = sk.curve(l2) else { panic!() };
            let off = (a - mm(want[0], want[1])).length().max((b - mm(want[2], want[3])).length());
            // (Perpendicular turns about a point ~70 mm off: live's 0.01 mm rounding of the lines grows there.)
            let tol = if c == ConstraintTool::Perpendicular { 0.15e-3 } else { 0.02e-3 };
            assert!(((b - a).length() - mm(45.05, 20.89).length()).abs() < 1e-9 || c == ConstraintTool::Equal, "{c:?}: length kept");
            assert!(off < tol, "{c:?}: L2 {:?} → {:?}, live ({}, {}) → ({}, {}): {:.4} mm off", a * 1e3, b * 1e3, want[0], want[1], want[2], want[3], off * 1e3);
        }
    }

    /// Live 2270 (hover-p0 constraint buttons): a selection a constraint can't take is refused with live's words in the
    /// banner (std's errorstringenum), the selection cleared, no tool started, nothing added.
    #[test]
    fn a_constraint_refuses_a_selection_with_lives_words() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let cases: [(ConstraintTool, &str, &str); 5] = [
            (ConstraintTool::Parallel, "a line and a circle", "A parallel constraint requires two lines."),
            (ConstraintTool::Tangent, "two lines", "A tangent constraint requires two curves, one of which must not be a line."),
            (ConstraintTool::Coincident, "a line's two ends", "A coincident constraint cannot be added to two points from the same curve."),
            (ConstraintTool::Concentric, "two lines", "A concentric constraint can only be added to circles, arcs, ellipses and points."),
            (ConstraintTool::Coincident, "a line and a circle", "A coincident constraint cannot be applied to two curves of different types."),
        ];
        for (c, what, words) in cases {
            let (mut ed, mut d) = editor(Tool::Select);
            let l1 = d.sketch.add_line(mm(0.0, 0.0), mm(40.0, 5.0));
            let l2 = d.sketch.add_line(mm(0.0, 20.0), mm(40.0, 30.0));
            let circle = d.sketch.add_circle(mm(70.0, 10.0), 8e-3);
            let ends = d.sketch.endpoints(l1).expect("its ends");
            ed.selection = match what {
                "a line and a circle" => vec![l1, circle],
                "two lines" => vec![l1, l2],
                _ => vec![ends.0, ends.1],
            };
            let before = d.sketch.constraints().count();
            let applied = ed.constraint_command(&mut d, c);
            assert_eq!(ed.refusal.as_deref(), Some(words), "{c:?} on {what}");
            assert!(!applied && ed.selection.is_empty() && ed.tool == Tool::Select && d.sketch.constraints().count() == before, "{c:?} on {what}: refused, the selection cleared, no tool, nothing added");
        }
    }

    /// Live2 glyph-select (11-17): one constraint glyph selected at a time: a click (Shift too) on another replaces it
    /// and Delete takes only that one; a glyph click drops a selected line and a line click drops the glyph;
    /// Shift+click on the selected glyph and a click on empty space deselect it.
    #[test]
    fn constraint_glyphs_select_one_at_a_time_as_lives() {
        let (mut ed, mut d) = editor(Tool::CornerRect);
        click(&mut ed, &mut d, v(-0.03, -0.02));
        click(&mut ed, &mut d, v(0.03, 0.02));
        ed.set_tool(Tool::Select);
        let find = |d: &SketchData, par: bool| {
            d.sketch.constraints().find(|c| if par { matches!(c.kind, ConstraintKind::Parallel { .. }) } else { matches!(c.kind, ConstraintKind::Perpendicular { .. }) }).map(|c| (c.id, c.kind.entities()[0])).expect("the rectangle's glyph")
        };
        let ((perp, pe), (par, pa)) = (find(&d, false), find(&d, true));
        let empty = v(0.1, 0.1);
        let glyph = |ed: &mut SketchEditor, d: &mut SketchData, g: (ConstraintId, EntityId), shift: bool| {
            ed.pressed_glyph = Some((g.0, g.1, shift));
            ed.tool_down(d, empty, None, shift, false);
            ed.tool_up(d, empty);
        };
        glyph(&mut ed, &mut d, (perp, pe), false);
        glyph(&mut ed, &mut d, (par, pa), false);
        assert_eq!(ed.selected_constraints, vec![par], "the second replaces the first");
        glyph(&mut ed, &mut d, (perp, pe), true);
        assert_eq!(ed.selected_constraints, vec![perp], "Shift+click on another replaces it too");
        glyph(&mut ed, &mut d, (perp, pe), true);
        assert!(ed.selected_constraints.is_empty(), "Shift+click on the selected one deselects it");
        glyph(&mut ed, &mut d, (par, pa), false);
        let n = d.sketch.constraints().count();
        assert!(ed.delete_selection(&mut d));
        assert!(d.sketch.constraint(par).is_none() && d.sketch.constraint(perp).is_some() && d.sketch.constraints().count() == n - 1, "Delete takes the selected glyph's alone");
        // A glyph click drops a selected line; a line click drops the glyph.
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).map(|e| e.id).unwrap();
        ed.selection = vec![line];
        glyph(&mut ed, &mut d, (perp, pe), false);
        assert!(ed.selection.is_empty() && ed.selected_constraints == vec![perp], "the glyph click drops the line");
        let Some(Curve2::Line { a, b }) = d.sketch.curve(line) else { panic!() };
        let mid = (a + b) * 0.5;
        ed.pointer_move(&mut d, mid, false, false);
        ed.hover = Some(line);
        ed.tool_down(&mut d, mid, Some(line), false, false);
        ed.tool_up(&mut d, mid);
        assert!(ed.selection == vec![line] && ed.selected_constraints.is_empty(), "the line click drops the glyph");
        // An empty click deselects.
        glyph(&mut ed, &mut d, (perp, pe), false);
        ed.pointer_move(&mut d, empty, false, false);
        ed.hover = None;
        ed.tool_down(&mut d, empty, None, false, false);
        ed.tool_up(&mut d, empty);
        assert!(ed.selected_constraints.is_empty() && ed.selection.is_empty(), "an empty click deselects");
    }

    /// Live's getInference (WT@9146346 lastInferenceTarget; live 11-18 sticky-snap, real pointer): between two lines'
    /// bands the one snapped to last stays though the other is nearer; coming in from elsewhere the nearer one lights; a
    /// point coming into reach takes over from the line all the same.
    #[test]
    fn the_line_snapped_to_last_stays_against_its_rank_not_a_point() {
        let (mut ed, mut d) = editor(Tool::Line);
        let low = d.sketch.add_line(v(-0.02, 0.0), v(0.02, 0.0));
        let high = d.sketch.add_line(v(-0.02, 6.0 * PX), v(0.02, 6.0 * PX));
        let point = d.sketch.add_point(v(0.012, 9.0 * PX));
        click(&mut ed, &mut d, v(-0.013, -0.031));
        let lit = |ed: &mut SketchEditor, d: &mut SketchData, q: DVec2| {
            ed.pointer_move(d, q, false, false);
            ed.inference.as_ref().map(|i| (i.kind, i.entity))
        };
        let on = |e| Some((InferenceKind::OnCurve, Some(e)));
        let between = v(0.004, 2.0 * PX);
        assert_eq!(lit(&mut ed, &mut d, v(0.004, 0.01)), None, "nothing");
        assert_eq!(lit(&mut ed, &mut d, between), on(low), "from elsewhere: the nearer line");
        assert_eq!(lit(&mut ed, &mut d, v(-0.01, 8.0 * PX)), on(high), "on the upper line, out of the lower one's band");
        assert_eq!(lit(&mut ed, &mut d, between), on(high), "along it into the lower one's band: the upper line stays");
        assert_eq!(lit(&mut ed, &mut d, v(0.012, 5.0 * PX)), Some((InferenceKind::Endpoint, Some(point))), "a point in reach takes over");
    }

    /// Live (11-17 sketch-snap-1117): a snap lights at 7 px and holds until the pointer is 9–10 px out (a point; a
    /// curve or a horizontal band 8–10); coming in from far, 9 px doesn't light.
    #[test]
    fn a_lit_snap_holds_to_its_let_go_reach() {
        let (mut ed, mut d) = editor(Tool::Line);
        let p = v(0.01, 0.01);
        d.sketch.add_point(p);
        click(&mut ed, &mut d, v(-0.01, -0.02));
        let snapped = |ed: &SketchEditor| ed.inference.as_ref().is_some_and(|i| i.kind == InferenceKind::Endpoint && (i.position - p).length() < 1e-12);
        let at = |ed: &mut SketchEditor, d: &mut SketchData, px: f64| {
            ed.pointer_move(d, p + v(px * PX, 0.0), false, false);
            snapped(ed)
        };
        assert!(!at(&mut ed, &mut d, 9.0), "9 px from far: not lit");
        assert!(at(&mut ed, &mut d, 6.0), "6 px: lit");
        assert!(at(&mut ed, &mut d, 9.0), "9 px out: held");
        assert!(!at(&mut ed, &mut d, 10.5), "10.5 px: let go");
        assert!(!at(&mut ed, &mut d, 9.0), "back to 9 px: not lit again");
    }

    /// A line's own vertical holds to 8 px off plumb, not a point's 9.5 (live sketch-snap-1117 "lets go at 8–10";
    /// ui-parity sketch-vertical-key: 9 px off after coming through the band isn't vertical).
    #[test]
    fn a_lines_own_vertical_lets_go_at_8_px() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        let at = |ed: &mut SketchEditor, d: &mut SketchData, px: f64| {
            ed.pointer_move(d, v(px * PX, 80.0 * PX), false, false);
            ed.inference.as_ref().is_some_and(|i| i.constraints == [AutoConstraint::VerticalLine])
        };
        assert!(at(&mut ed, &mut d, 6.0), "6 px: lit");
        assert!(at(&mut ed, &mut d, 7.8), "7.8 px: held");
        assert!(!at(&mut ed, &mut d, 9.0), "9 px: let go");
    }

    /// qa FEEL-LINE-SNAP-MODEL-LOOK (live qa walk 1 w4-vertex, w4-mid): the Line tool before its first click on a model
    /// vertex lying on the sketch's horizontal axis shows × and —, and no blue point at the pointer (the lit vertex and
    /// the snap square alone); off the axes × alone. Away from any snap the point shows.
    #[test]
    fn a_model_vertex_snapped_to_before_the_first_click_shows_no_cursor_point() {
        let vertex = noshape_model::reference::Reference::SketchEntity { sketch: FeatureId(9), entity: 3 };
        let units = UnitPrefs::default();
        let cursor_points = |ed: &SketchEditor, d: &SketchData| ed.overlay(d, &units).1.points.iter().filter(|p| p.size == 6.0).count();
        for (at, icons) in [(v(0.04, 0.0), vec!["coincident", "horizontal"]), (v(0.04, 0.012), vec!["coincident"])] {
            let (mut ed, mut d) = editor(Tool::Line);
            ed.pointer_move(&mut d, at, false, false);
            ed.apply_external_snap(ExternalSnap::Vertex { position: at, reference: vertex.clone() });
            assert_eq!(ed.snap_icons(), icons, "at {at:?}");
            assert_eq!(cursor_points(&ed, &d), 0, "no point at the pointer on the vertex at {at:?}");
        }
        let (mut ed, mut d) = editor(Tool::Line);
        ed.pointer_move(&mut d, v(0.03, 0.017), false, false);
        assert_eq!(cursor_points(&ed, &d), 1, "the point at the pointer, no snap");
    }

    /// qa 11-17 SKSNAP-ALIGN-WOKEN-POINT (live 11-17 sketch-snap-1117 row 2): another sketch's point rested on (an
    /// external vertex snap; live feel retaken 11-18: the snap wakes it, no rest) wakes as a point of the sketch does: 60 px below it, 6 px off its vertical, a
    /// line's end aligns with it, and the click asks for that point used (the alignment then on the used point).
    #[test]
    fn a_rested_point_of_another_sketch_offers_its_alignment() {
        let (mut ed, mut d) = editor(Tool::Line);
        let other = noshape_model::reference::Reference::SketchEntity { sketch: FeatureId(9), entity: 3 };
        let w = v(0.02, 0.012);
        ed.pointer_move(&mut d, w, false, false);
        ed.apply_external_snap(ExternalSnap::Vertex { position: w, reference: other.clone() });
        ed.pointer_move(&mut d, w + v(PX, 0.0), false, false);
        assert_eq!(ed.woken_point.as_ref().map(|p| &p.0), Some(&other), "woken by the snap onto it");
        click(&mut ed, &mut d, v(0.0, 0.0));
        ed.pointer_move(&mut d, w + v(6.0 * PX, -60.0 * PX), false, false);
        let i = ed.inference.clone().expect("an inference");
        assert!(i.kind == InferenceKind::AlignVertical && (i.position.x - w.x).abs() < 1e-12, "aligned under it: {i:?}");
        assert_eq!(ed.needs_woken_edge(), Some(&other), "the click uses it");
        let used = d.sketch.add_point(w);
        ed.link_woken_edge(&d.sketch, used);
        assert!(ed.inference.as_ref().unwrap().constraints.iter().any(|c| matches!(c, AutoConstraint::VerticalWith(p) if *p == used)), "aligned with the used point");
    }

    /// Live 11-17 sketch-snap-1117 (2) and (4): a line's own horizontal is a 7 px band at any length (40 to 320 px from
    /// its start), a woken point's vertical the same at 60 and 150 px from it, and Shift held while drawing infers
    /// nothing: the end lands at the raw pointer and the click adds no constraint.
    #[test]
    fn drawing_a_line_snaps_in_px_bands_and_shift_snaps_nothing_as_lives() {
        let start = v(0.0, 0.0);
        let lit = |kind: InferenceKind, at: DVec2, wake: Option<DVec2>| {
            let (mut ed, mut d) = editor(Tool::Line);
            click(&mut ed, &mut d, start);
            if let Some(w) = wake {
                ed.woken = vec![d.sketch.add_point(w)];
            }
            ed.pointer_move(&mut d, at, false, false);
            ed.inference.as_ref().is_some_and(|i| i.kind == kind)
        };
        for len in [40.0, 80.0, 160.0, 320.0] {
            assert!(lit(InferenceKind::Horizontal, v(len * PX, 6.0 * PX), None), "{len} px long, 6 px off level: horizontal");
            assert!(!lit(InferenceKind::Horizontal, v(len * PX, 8.5 * PX), None), "{len} px long, 8.5 px off level: free");
        }
        let w = v(0.02, 0.012);
        for from in [60.0, 150.0] {
            assert!(lit(InferenceKind::AlignVertical, w + v(6.0 * PX, -from * PX), Some(w)), "{from} px below the woken point, 6 px off: aligned");
            assert!(!lit(InferenceKind::AlignVertical, w + v(8.5 * PX, -from * PX), Some(w)), "{from} px below, 8.5 px off: free");
        }
        let (mut ed, mut d) = editor(Tool::Line);
        let p = d.sketch.add_point(v(0.01, 0.0021));
        click(&mut ed, &mut d, v(0.002, 0.0021 + 2.0 * PX));
        let end = v(0.01 + PX, 0.0021);
        ed.pointer_move(&mut d, end, true, false);
        assert!(ed.inference.is_none() && ed.snap_icons().is_empty(), "Shift: no snap, no glyph");
        let h = d.sketch.pick(end, PX * 4.0);
        ed.tool_down(&mut d, end, h, true, false);
        ed.tool_up(&mut d, end);
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).map(|e| e.id).expect("a line");
        let (_, b) = d.sketch.entity(line).and_then(|e| e.endpoints()).unwrap();
        assert!((d.sketch.point(b).unwrap() - end).length() < 1e-12 && b != p, "the end at the raw pointer, 1 px from the point");
        assert_eq!(d.sketch.constraints().count(), 0, "no constraint: not coincident, not horizontal (2 px off level)");
    }

    /// Live2 11-17 part 2b (p2b2.json "apply"): two free lines picked, every constraint button as live's: Parallel,
    /// Perpendicular, Equal and Coincident add one, Horizontal, Vertical and Fix one per line, each clearing the
    /// selection with no tool left armed; Tangent, Concentric, Midpoint and Normal refuse in std's words, the selection
    /// cleared; Symmetric arms its tool with both lines kept for the third pick.
    #[test]
    fn two_lines_through_every_constraint_button_as_lives() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let applies = [(ConstraintTool::Parallel, 1), (ConstraintTool::Perpendicular, 1), (ConstraintTool::Equal, 1), (ConstraintTool::Coincident, 1), (ConstraintTool::Horizontal, 2), (ConstraintTool::Vertical, 2), (ConstraintTool::Fix, 2)];
        let refuses = [ConstraintTool::Tangent, ConstraintTool::Concentric, ConstraintTool::Midpoint, ConstraintTool::Normal];
        let fresh = || {
            let (mut ed, mut d) = editor(Tool::Select);
            let l1 = d.sketch.add_line(mm(44.92, -9.72), mm(84.88, -3.70));
            let l2 = d.sketch.add_line(mm(49.85, 10.23), mm(94.90, 31.12));
            ed.selection = vec![l1, l2];
            (ed, d)
        };
        for (c, n) in applies {
            let (mut ed, mut d) = fresh();
            assert!(ed.constraint_command(&mut d, c), "{c:?} applies");
            assert_eq!(d.sketch.constraints().count(), n, "{c:?}");
            assert!(ed.selection.is_empty() && ed.tool == Tool::Select && ed.refusal.is_none(), "{c:?}: selection cleared, no tool");
        }
        for c in refuses {
            let (mut ed, mut d) = fresh();
            assert!(!ed.constraint_command(&mut d, c), "{c:?} refuses");
            assert!(ed.refusal.is_some() && d.sketch.constraints().count() == 0 && ed.selection.is_empty() && ed.tool == Tool::Select, "{c:?}");
        }
        let (mut ed, mut d) = fresh();
        ed.constraint_command(&mut d, ConstraintTool::Symmetric);
        assert!(ed.tool == Tool::Constraint(ConstraintTool::Symmetric) && ed.selection.len() == 2 && d.sketch.constraints().count() == 0, "Symmetric arms");
    }

    /// Live2 11-17 part 2b: Horizontal / Vertical on two lines, one each; both turn about their midpoints.
    #[test]
    fn horizontal_and_vertical_turn_both_lines_about_their_midpoints() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        // live2 p2b2.json "apply": L1, L2 as picked; the ends after.
        let cases = [
            (ConstraintTool::Horizontal, [[44.6972, -6.7073, 85.1054, -6.7073], [47.5463, 20.6754, 97.2065, 20.6754]]),
            (ConstraintTool::Vertical, [[64.9013, -26.9114, 64.9013, 13.4968], [72.3764, -4.1547, 72.3764, 45.5055]]),
        ];
        for (c, want) in cases {
            let mut sk = Sketch::new();
            let l1 = sk.add_line(mm(44.92, -9.72), mm(84.88, -3.70));
            let l2 = sk.add_line(mm(49.85, 10.23), mm(94.90, 31.12));
            assert!(apply_constraint(&mut sk, c, &[l1, l2]).is_ok_and(|n| n == 2), "{c:?}");
            assert!(solve_after_constraint(&mut sk, c, &[l1, l2]), "{c:?}");
            for (l, w) in [l1, l2].into_iter().zip(want) {
                let Some(Curve2::Line { a, b }) = sk.curve(l) else { panic!() };
                let off = (a - mm(w[0], w[1])).length().max((b - mm(w[2], w[3])).length());
                assert!(off < 0.02e-3, "{c:?} {l:?}: {a:?} {b:?}");
            }
        }
    }

    /// Live sketch-tool-conic: a conic from the Origin to a point level with it, then its shoulder: Coincident at the
    /// Origin and Horizontal between the ends, as a line's would be.
    #[test]
    fn a_conics_second_end_takes_its_inferences() {
        let (mut ed, mut d) = editor(Tool::Conic);
        for p in [v(0.0, 0.0), v(0.02, 0.0), v(0.01, 0.01)] {
            click(&mut ed, &mut d, p);
        }
        let kinds: Vec<&str> = d.sketch.constraints().map(|c| c.kind.name()).collect();
        assert!(kinds.contains(&"Coincident"), "{kinds:?}");
        assert!(kinds.contains(&"Horizontal"), "{kinds:?}");
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

    /// SKETCH-SPLINE-HANDLE-ARM: a spline's handle arm picks (near its tip, where the curve has left it), takes
    /// Horizontal like a line, and dragging it moves the handle, not the spline's end.
    #[test]
    fn a_spline_handle_arm_picks_and_takes_horizontal() {
        let (mut ed, mut d) = editor(Tool::Select);
        let s = d.sketch.add_fit_spline_with_handles(&[v(0.0, 0.0), v(0.03, 0.02), v(0.06, 0.0)]).unwrap();
        let Some(arm) = d.sketch.handle_arms(s)[0] else { panic!("arms") };
        let (start, tip) = d.sketch.endpoints(arm).unwrap();
        let (a, b) = (d.sketch.point(start).unwrap(), d.sketch.point(tip).unwrap());
        let near_tip = a + (b - a) * 0.85;
        assert_eq!(d.sketch.pick(near_tip, PX * 4.0), Some(arm), "the arm picks");
        assert!(constraint_applicable(&d.sketch, ConstraintTool::Horizontal, &[arm]));
        assert_eq!(apply_constraint(&mut d.sketch, ConstraintTool::Horizontal, &[arm]), Ok(1));
        assert!(d.sketch.solve().is_solved());
        let (a, b) = (d.sketch.point(start).unwrap(), d.sketch.point(tip).unwrap());
        assert!((a.y - b.y).abs() < 1e-9);
        // Dragging the arm: the tip follows, the spline's start stays.
        let grab = a + (b - a) * 0.85;
        drag(&mut ed, &mut d, grab, grab + v(0.005, 0.0));
        assert!((d.sketch.point(start).unwrap() - a).length() < 1e-9, "the spline's end stays");
    }

    /// Live 2270: a constraint pressed with a selection it can't take refuses with std's words, clears the selection
    /// and starts no tool (a spline's handle arm too); Midpoint on two points and Symmetric on two lines start their
    /// tool with them; Fix applies.
    #[test]
    fn a_constraint_a_selection_cant_take_refuses_with_lives_words() {
        let (mut ed, mut d) = editor(Tool::Select);
        let a = d.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
        let b = d.sketch.add_line(v(0.0, 0.01), v(0.03, 0.02));
        let c = d.sketch.add_circle(v(0.05, 0.0), 0.005);
        let p = d.sketch.add_point(v(0.06, 0.02));
        let centre = d.sketch.entity(c).and_then(|e| e.center()).unwrap();
        let (b0, b1) = d.sketch.endpoints(b).unwrap();
        let s = d.sketch.add_fit_spline_with_handles(&[v(0.0, 0.03), v(0.015, 0.04), v(0.03, 0.03)]).unwrap();
        let Some(arm) = d.sketch.handle_arms(s)[0] else { panic!("arms") };
        let n = d.sketch.constraints().count();
        let refused = [
            (ConstraintTool::Concentric, vec![a, c], "A concentric constraint can only be added to circles, arcs, ellipses and points."),
            (ConstraintTool::Concentric, vec![arm, c], "A concentric constraint can only be added to circles, arcs, ellipses and points."),
            // (live2 11-17 part 2b: two lines picked.)
            (ConstraintTool::Concentric, vec![a, b], "A concentric constraint can only be added to circles, arcs, ellipses and points."),
            (ConstraintTool::Midpoint, vec![a, b], "Midpoint constraint requires a point and a line or arc."),
            (ConstraintTool::Normal, vec![a, b], "A normal constraint requires a line and a curve."),
            (ConstraintTool::Parallel, vec![a, c], "A parallel constraint requires two lines."),
            (ConstraintTool::Perpendicular, vec![a, b, c], "Could not add constraint."),
            (ConstraintTool::Tangent, vec![a, b], "A tangent constraint requires two curves, one of which must not be a line."),
            (ConstraintTool::Equal, vec![a, c], "An equal constraint can only be added between two lines or two curves."),
            (ConstraintTool::Normal, vec![c, p], "A normal constraint requires a line and a curve."),
            (ConstraintTool::Horizontal, vec![c], "The entities must be all points or all lines and ellipses."),
            (ConstraintTool::Coincident, vec![b0, b1], "A coincident constraint cannot be added to two points from the same curve."),
            (ConstraintTool::Coincident, vec![a, c], "A coincident constraint cannot be applied to two curves of different types."),
            (ConstraintTool::Pierce, vec![a, p], "Could not add constraint."),
            (ConstraintTool::Pierce, vec![a, c, p], "A pierce constraint requires a curve or point from the sketch and an external edge."),
            (ConstraintTool::Curvature, vec![a, c], "An equal curvature constraint requires two curves one of which must be a conic or spline."),
        ];
        for (tool, sel, words) in refused {
            ed.selection = sel;
            assert!(!ed.constraint_command(&mut d, tool), "{tool:?}");
            assert_eq!(ed.refusal.take().as_deref(), Some(words), "{tool:?}");
            assert!(ed.selection.is_empty() && ed.tool == Tool::Select, "{tool:?}: selection cleared, no tool");
        }
        assert_eq!(d.sketch.constraints().count(), n, "nothing added");
        for (tool, sel) in [(ConstraintTool::Midpoint, vec![p, centre]), (ConstraintTool::Symmetric, vec![a, b])] {
            ed.set_tool(Tool::Select);
            ed.selection = sel.clone();
            assert!(!ed.constraint_command(&mut d, tool));
            assert!(ed.refusal.is_none(), "{tool:?}: no message");
            assert_eq!(ed.tool, Tool::Constraint(tool), "{tool:?}: its tool starts");
            assert_eq!(ed.selection, sel, "{tool:?}: the picks stay selected");
        }
        ed.set_tool(Tool::Select);
        ed.selection = vec![a];
        assert!(ed.constraint_command(&mut d, ConstraintTool::Fix));
        assert!(ed.refusal.is_none());
    }

    /// SKETCH-SPLINE-END-DERIVATIVES: a spline's end-tangent handle drags
    /// (the curve's end tangent follows) and dimensions alone as its length,
    /// which then holds it.
    #[test]
    fn a_spline_handle_drags_and_dimensions() {
        let (mut ed, mut d) = editor(Tool::Select);
        let s = d.sketch.add_fit_spline_with_handles(&[v(0.0, 0.0), v(0.02, 0.01), v(0.04, 0.0)]).unwrap();
        let (h0, _) = d.sketch.entity(s).unwrap().spline_handles().unwrap();
        let (start, _) = d.sketch.endpoints(s).unwrap();
        let hp = d.sketch.point(h0).unwrap();
        let before = d.sketch.tessellate(s, 1e-5);
        assert_eq!(d.sketch.pick(hp, PX * 4.0), Some(h0), "the handle picks");
        drag(&mut ed, &mut d, hp, hp + v(0.0, 0.01));
        let after = d.sketch.tessellate(s, 1e-5);
        assert!((d.sketch.point(h0).unwrap() - (hp + v(0.0, 0.01))).length() < 1e-9, "the handle follows the pointer");
        assert!((after[after.len() / 4] - before[before.len() / 4]).length() > 1e-4, "the curve reshapes");
        let Some(ConstraintKind::Distance { a, b, .. }) = smart_dimension(&d.sketch, &[h0], hp) else { panic!("a handle's dimension") };
        assert_eq!((a, b), (start, h0));
        d.sketch.add_constraint(ConstraintKind::Distance { a, b, value: 0.008, mode: DistanceMode::Center }).unwrap();
        assert!(d.sketch.solve().is_solved());
        assert!(((d.sketch.point(h0).unwrap() - d.sketch.point(start).unwrap()).length() - 0.008).abs() < 1e-9);
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

    /// Live (behaviours "qa (2026-10-28) sketch.md 7.22"): a conic, then a
    /// spline that doesn't touch it: the second pick is refused with the
    /// warning, the picks clear and the tool stays on; hovering the spline
    /// doesn't drag the conic's Rho along.
    #[test]
    fn dimension_between_free_splines_is_refused_at_the_second_pick() {
        let (mut ed, mut d) = editor(Tool::Dimension);
        let conic = d.sketch.add_conic(v(0.0, 0.0), v(0.01, 0.02), v(0.02, 0.0), 0.5).unwrap();
        let spline = d.sketch.add_fit_spline(&[v(0.04, 0.0), v(0.05, 0.01), v(0.06, 0.0)]).unwrap();
        let on = |e: EntityId, d: &SketchData| {
            let c = d.sketch.curve(e).unwrap();
            let (a, b) = c.domain();
            c.point(a + (b - a) * 0.3)
        };
        let (pc, ps) = (on(conic, &d), on(spline, &d));
        click(&mut ed, &mut d, pc);
        assert_eq!(ed.picked(None), vec![conic]);
        ed.pointer_move(&mut d, ps, false, false);
        ed.hover = d.sketch.pick(ps, PX * 4.0);
        assert_eq!(ed.hover, Some(spline));
        assert!(ed.dimension_preview(&d).is_none(), "no Rho preview over the spline");
        click(&mut ed, &mut d, ps);
        assert_eq!(ed.toast.as_deref(), Some(DIMENSION_FAILED));
        assert!(ed.picked(None).is_empty(), "the picks clear");
        assert_eq!(ed.tool, Tool::Dimension, "the tool stays on");
        assert!(ed.dim_edit.is_none());
    }

    /// qa FEEL-DIM-TWO-POINTS-ZERO (live feel round 2 R3): the two ends of a line drawn from the origin, the dimension
    /// placed above-left: the value box reads the ends' distance (horizontal there), never 0.
    #[test]
    fn a_dimension_between_a_lines_ends_from_the_origin_reads_their_distance() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.03, 0.02));
        assert!(ed.cancel());
        ed.set_tool(Tool::Dimension);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.03, 0.02));
        let picks: Vec<_> = ed.picks.iter().map(|x| (d.sketch.kind(x.0), d.sketch.point(x.0))).collect();
        assert!(picks.iter().all(|(k, _)| *k == Some(EntityKind::Point)), "{picks:?}");
        ed.pointer_move(&mut d, v(0.005, 0.025), false, false);
        click(&mut ed, &mut d, v(0.005, 0.025));
        let text = ed.dim_edit.as_ref().map(|e| e.text.clone());
        assert_eq!(text.as_deref(), Some("30 mm"), "{picks:?}");
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

    /// Live2 sk-feel2-1118: a new dimension's label has no inference to another dimension's text or line: swept 1 px at
    /// a time across it, it follows the pointer 1:1; placed 3 px off it, it stays there.
    #[test]
    fn a_new_dimensions_label_snaps_to_no_other_dimension() {
        let (mut ed, mut d) = editor(Tool::Dimension);
        let a = d.sketch.add_line(v(0.01, 0.01), v(0.03, 0.01));
        let b = d.sketch.add_line(v(0.04, 0.01), v(0.06, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.015));
        ed.dim_edit = None;
        let first = d.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::Length { curve, .. } if curve == a)).unwrap().id;
        assert_eq!(d.dimension_labels.get(&first.0), Some(&[0.02, 0.015]));
        click(&mut ed, &mut d, v(0.05, 0.01));
        for k in -4..=4 {
            let at = v(0.05, 0.015 + k as f64 * PX);
            ed.pointer_move(&mut d, at, false, false);
            let (_, pos, _) = ed.dimension_preview(&d).expect("the preview");
            assert!((pos - at).length() < 1e-12, "{k} px: the label at {pos:?}, the pointer at {at:?}");
        }
        let off = v(0.05, 0.015 + 3.0 * PX);
        click(&mut ed, &mut d, off);
        let second = d.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::Length { curve, .. } if curve == b)).unwrap().id;
        assert_eq!(d.dimension_labels.get(&second.0), Some(&[off.x, off.y]), "placed 3 px off the first one's line, it stays");
    }

    /// Live 1853 addendum: `q` with nothing selected turns construction on,
    /// Escape out of the Line tool turns it off; `q` on a selected line makes
    /// it construction and the next line too; the Construction button shows
    /// a selection of construction curves.
    #[test]
    fn construction_mode_as_live() {
        let (mut ed, mut d) = editor(Tool::Select);
        ed.toggle_construction(&mut d);
        assert!(ed.construction_shown(&d.sketch));
        ed.set_tool(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.02, 0.0));
        assert!(d.sketch.entities().any(|e| e.kind() == EntityKind::Line && e.construction));
        while ed.tool != Tool::Select {
            ed.cancel();
        }
        assert!(!ed.construction_shown(&d.sketch), "Escape out of Line ends it");
        let l = d.sketch.add_line(v(0.0, 0.01), v(0.02, 0.01));
        ed.selection = vec![l];
        assert!(!ed.construction_shown(&d.sketch));
        assert!(ed.toggle_construction(&mut d));
        assert!(d.sketch.entity(l).unwrap().construction && ed.construction && ed.construction_shown(&d.sketch));
        ed.construction = false;
        assert!(ed.construction_shown(&d.sketch), "a construction line selected alone");
    }

    /// Live (ui-parity revolve-dialog-axis): a Line clicked over a rectangle's
    /// constraint glyph draws; the glyph doesn't take the press.
    #[test]
    fn a_line_started_over_a_glyph_draws() {
        let (mut ed, mut d) = editor(Tool::CornerRect);
        click(&mut ed, &mut d, v(0.010, 0.0));
        click(&mut ed, &mut d, v(0.020, 0.020));
        let glyph = d.sketch.constraints().find(|c| matches!(c.kind, ConstraintKind::Horizontal { .. })).map(|c| (c.id, c.kind.entities()[0])).expect("a Horizontal");
        ed.set_tool(Tool::Line);
        for p in [v(0.0, 0.0), v(0.0, 0.020)] {
            ed.pressed_glyph = Some((glyph.0, glyph.1, false));
            click(&mut ed, &mut d, p);
        }
        ed.cancel();
        assert_eq!(count(&d, EntityKind::Line), 5, "the rectangle and the new line");
    }

    /// Live (ui-parity sketch-dimension-edit): the placement click lands on
    /// the line's Horizontal glyph and places the dimension; its box shows
    /// the length to 1e-8 m ("29.73371 mm").
    #[test]
    fn a_dimension_placed_over_a_glyph_opens_its_box() {
        let (mut ed, mut d) = editor(Tool::Dimension);
        let l = d.sketch.add_line(v(0.0, 0.0), v(0.0302569196, 0.0));
        let h = d.sketch.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
        click(&mut ed, &mut d, v(0.015, 0.0));
        ed.pressed_glyph = Some((h, l, false));
        click(&mut ed, &mut d, v(0.015, -0.008));
        let e = ed.dim_edit.as_ref().expect("placement opens the value box");
        assert_eq!(e.text, "30.25692 mm");
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

    /// Live (behaviours "feel (new rows) 11-18: a sketch dimension's edit box, second click and drag"): a second
    /// single click on a selected dimension's label deselects it (only a double-click opens the box); a 20 px drag of
    /// the selected label moves it and keeps it selected.
    #[test]
    fn a_selected_labels_second_click_deselects_and_its_drag_keeps_it() {
        let (mut ed, mut d) = editor(Tool::Select);
        let line = d.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
        let cid = d.sketch.add_constraint(ConstraintKind::Length { curve: line, value: 0.03 }).unwrap();
        let at = v(0.015, 0.004);
        d.dimension_labels.insert(cid.0, [at.x, at.y]);
        let press = |ed: &mut SketchEditor, d: &mut SketchData, to: DVec2| {
            ed.pressed_label = Some((cid, false, true));
            ed.tool_down(d, at, None, false, false);
            ed.tool_drag(d, to);
            ed.tool_up(d, to);
        };
        press(&mut ed, &mut d, at);
        assert_eq!(ed.selected_constraints, vec![cid]);
        press(&mut ed, &mut d, at);
        assert!(ed.selected_constraints.is_empty(), "the second click deselects");
        press(&mut ed, &mut d, at);
        let to = at + v(20.0 * PX, 0.0);
        press(&mut ed, &mut d, to);
        assert_eq!(ed.selected_constraints, vec![cid], "a drag keeps it selected");
        assert_eq!(d.dimension_labels.get(&cid.0), Some(&[to.x, to.y]));
    }

    /// Live (behaviours "feel (Sketch tools) retaken 11-18", sktools-1118 st-trim-all; live recheck b5b664aa): a line
    /// passing ~1 px by a chain's corner, outside it or across it, doesn't cross there; a click near its free end trims
    /// it up to its crossing with the next segment.
    #[test]
    fn a_trim_past_a_chains_corner_goes_to_the_next_crossing() {
        let px = |x: f64, y: f64| v(x * PX, -y * PX);
        let corner = v(600.0, 650.0);
        let d = v(220.0, -200.0).normalize();
        for off in [0.9, -0.6] {
            let (mut ed, mut data) = editor(Tool::Trim);
            let pts: Vec<EntityId> = [(500.0, 650.0), (600.0, 650.0), (650.0, 560.0), (750.0, 600.0)].iter().map(|(x, y)| data.sketch.add_point(px(*x, *y))).collect();
            for w in pts.windows(2) {
                data.sketch.add_line_between(w[0], w[1]).unwrap();
            }
            let through = corner + d.perp() * off;
            let (a, b) = (through - d * 150.0, through + d * 120.0);
            let line = data.sketch.add_line(px(a.x, a.y), px(b.x, b.y));
            let pick = through - d * 110.0;
            click(&mut ed, &mut data, px(pick.x, pick.y));
            let c = data.sketch.curve(line).expect("the piece past the third segment stays");
            let (s, e) = (c.start_point() / PX, c.end_point() / PX);
            let (lo, hi) = if s.x < e.x { (s, e) } else { (e, s) };
            assert!((hi - v(b.x, -b.y)).length() < 1e-6, "{off}: {hi:?}");
            assert!(lo.x > 670.0 && lo.x < 700.0, "{off} px off the corner: it starts at the third segment's crossing, not by the corner: {lo:?}");
            assert_eq!(count(&data, EntityKind::Line), 4);
        }
    }

    /// Live sketch-edit-feel-1122 S4 (trim-hover-upper-arc.png): Trim's hovered piece of a circle (its upper arc between
    /// the line's crossings) is lit like any hovered curve: the hover orange, 5 px, with an orange dot at each crossing.
    #[test]
    fn trims_hovered_piece_is_the_hover_orange_with_a_dot_at_each_crossing() {
        let (mut ed, mut data) = editor(Tool::Trim);
        let units = UnitPrefs::default();
        let mm = |x: f64, y: f64| v(x, y) * 1e-3;
        data.sketch.add_line(mm(0.0, 0.0), mm(40.0, 0.0));
        let c = data.sketch.add_circle(mm(14.0, 3.0), 6e-3);
        (ed.hover, ed.cursor) = (Some(c), Some(mm(14.0, 9.0)));
        let (lines, points) = ed.overlay(&data, &units);
        let orange = crate::palette::rgb8(crate::palette::viewport::PREHIGHLIGHT);
        let band: Vec<&LineSegment> = lines.segments.iter().filter(|g| g.color == orange && g.width == 5.0).collect();
        assert!(band.len() > 10 && band.iter().all(|g| g.a[1].min(g.b[1]) > -1e-6), "the upper arc's band: {} segments", band.len());
        let x = 27.0_f64.sqrt();
        let dots: Vec<[f32; 3]> = points.points.iter().filter(|p| p.color == orange).map(|p| p.position).collect();
        assert_eq!(dots.len(), 2, "{dots:?}");
        for q in [mm(14.0 - x, 0.0), mm(14.0 + x, 0.0)] {
            let w = PlaneFrame::TOP.to_world(q);
            assert!(dots.iter().any(|d| (glam::DVec3::new(d[0] as f64, d[1] as f64, d[2] as f64) - w).length() < 1e-5), "a dot at {q:?}: {dots:?}");
        }
    }

    #[test]
    fn polygon_sides_typed_and_midpoint_line() {
        let (mut ed, mut d) = editor(Tool::Polygon);
        click(&mut ed, &mut d, v(0.011, 0.013));
        ed.polygon_sides = 5;
        click(&mut ed, &mut d, v(0.031, 0.013));
        assert!(ed.polygon_sides_pending(), "the circumference is locked first");
        // moving away from the polygon adds a side per 10 px, back restores them
        ed.pointer_move(&mut d, v(0.031 + PX * 21.0, 0.013), false, false);
        assert_eq!(ed.polygon_sides, 7);
        ed.pointer_move(&mut d, v(0.031 - PX * 19.0, 0.013), false, false);
        assert_eq!(ed.polygon_sides, 3);
        ed.pointer_move(&mut d, v(0.031, 0.013), false, false);
        assert_eq!(ed.polygon_sides, 5);
        click(&mut ed, &mut d, v(0.031, 0.013));
        assert_eq!(count(&d, EntityKind::Line), 5);
        assert!(ed.otf.is_none(), "no diameter box (live makes no dimension)");
        let (mut ed, mut d) = editor(Tool::MidpointLine);
        click(&mut ed, &mut d, v(0.021, 0.023));
        click(&mut ed, &mut d, v(0.031, 0.027));
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).unwrap().id;
        let c = d.sketch.curve(line).unwrap();
        assert!(((c.start_point() + c.end_point()) * 0.5 - v(0.021, 0.023)).length() < 1e-9);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Midpoint { .. })));
        assert!((c.start_point() - v(0.031, 0.027)).length() < 1e-9, "live: from the clicked end");
    }

    /// Live (behaviours "skasm (2026-10-27) sketch value boxes right after
    /// drawing"): a center point circle's Ø box sits opposite the rim click,
    /// on that axis, its near side ~14 px off the rim.
    #[test]
    fn circle_box_opposite_the_rim_click() {
        let units = UnitPrefs::default();
        for (rim, side) in [(v(0.010, 0.0), v(-1.0, 0.0)), (v(0.0, 0.010), v(0.0, -1.0)), (v(-0.010, 0.0), v(1.0, 0.0)), (v(0.0, -0.010), v(0.0, 1.0))] {
            let (mut ed, mut d) = editor(Tool::Circle);
            let c = v(0.031, 0.027);
            click(&mut ed, &mut d, c);
            click(&mut ed, &mut d, c + rim);
            let boxes = ed.otf_boxes(&d, &units);
            let at = boxes[0].0 - c;
            let along = at.dot(side);
            assert!(along > 0.010 + 14.0 * PX, "{rim:?}: box at {at:?}");
            assert!((at - side * along).length() < 1e-9, "{rim:?}: box off the axis {at:?}");
        }
    }

    /// Live workflow 05 (qa SKETCH-LENGTH-LABEL-IN-PLANE): a Line's length box lies in the sketch plane; a circle's Ø
    /// and a rectangle's boxes stay on the screen (not seen in a 3D view yet).
    #[test]
    fn only_a_line_length_box_lies_in_the_plane() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.010, 0.010));
        click(&mut ed, &mut d, v(0.030, 0.015));
        assert!(ed.otf_active_in_plane(&d), "line length box");
        for tool in [Tool::Circle, Tool::CornerRect] {
            let (mut ed, mut d) = editor(tool);
            click(&mut ed, &mut d, v(0.010, 0.010));
            click(&mut ed, &mut d, v(0.030, 0.025));
            assert!(!ed.otf_boxes(&d, &UnitPrefs::default()).is_empty() && !ed.otf_active_in_plane(&d), "{tool:?}");
        }
    }

    /// Live (behaviours "skasm (2026-10-27) sketch value boxes … rule
    /// capture"): a corner rectangle's width box outside the first click's
    /// horizontal edge, its height text outside the first click's vertical
    /// edge and, as the box, inside by the other vertical edge; a 3 point
    /// circle's box opposite the first click, a center point arc's opposite
    /// the start, a 3 point arc's on the chord's midpoint.
    #[test]
    fn value_boxes_where_live_puts_them() {
        let units = UnitPrefs::default();
        // top-left (first) → bottom-right
        let (mut ed, mut d) = editor(Tool::CornerRect);
        let (p0, p1) = (v(0.010, 0.030), v(0.040, 0.010));
        click(&mut ed, &mut d, p0);
        click(&mut ed, &mut d, p1);
        let b = ed.otf_boxes(&d, &units);
        assert!(b[0].0.y > 0.030 + 14.0 * PX && (b[0].0.x - 0.025).abs() < 1e-9, "width box above the top edge: {:?}", b[0].0);
        assert!(b[1].0.x < 0.010 - 16.0 * PX && (b[1].0.y - 0.020).abs() < 1e-9, "height text left of the left edge: {:?}", b[1].0);
        ed.otf_step(1);
        let b = ed.otf_boxes(&d, &units);
        assert!(b[1].0.x < 0.040 - 14.0 * PX && b[1].0.x > 0.025 && (b[1].0.y - 0.020).abs() < 1e-9, "height box inside by the right edge: {:?}", b[1].0);
        // 3 point circle, first click on top: the box below
        let (mut ed, mut d) = editor(Tool::ThreePointCircle);
        for p in [v(0.030, 0.040), v(0.040, 0.030), v(0.020, 0.030)] {
            click(&mut ed, &mut d, p);
        }
        let b = ed.otf_boxes(&d, &units);
        assert!(b[0].0.y < 0.020 - 14.0 * PX && (b[0].0.x - 0.030).abs() < 1e-9, "box below: {:?}", b[0].0);
        // center point arc, start to the right: the box left
        let (mut ed, mut d) = editor(Tool::CenterArc);
        click(&mut ed, &mut d, v(0.030, 0.030));
        click(&mut ed, &mut d, v(0.040, 0.030));
        for a in [0.3f64, 0.8, 1.3, 1.5] {
            ed.pointer_move(&mut d, v(0.030, 0.030) + DVec2::from_angle(a) * 0.01, false, false);
        }
        click(&mut ed, &mut d, v(0.030, 0.040));
        let b = ed.otf_boxes(&d, &units);
        assert!(b[0].0.x < 0.020 - 14.0 * PX && (b[0].0.y - 0.030).abs() < 1e-9, "box left: {:?}", b[0].0);
        // 3 point arc: on the chord's midpoint
        let (mut ed, mut d) = editor(Tool::Arc3);
        for p in [v(0.020, 0.030), v(0.040, 0.030), v(0.030, 0.036)] {
            click(&mut ed, &mut d, p);
        }
        let b = ed.otf_boxes(&d, &units);
        assert!((b[0].0 - v(0.030, 0.030)).length() < 1e-6, "box on the chord: {:?}", b[0].0);
    }

    /// DUP-COINCIDENT: a line end held on the origin, dragged and released
    /// (it can't move) keeps its one Coincident; every release added another.
    #[test]
    fn a_point_held_on_the_origin_keeps_one_coincident() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.0, 0.0), v(0.02, 0.01));
        let end = d.sketch.endpoints(l).unwrap().0;
        let origin = origin_point(&mut d.sketch);
        d.sketch.add_constraint(ConstraintKind::Coincident { a: end, b: origin }).unwrap();
        d.sketch.solve();
        let count = |d: &SketchData| d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::Coincident { .. })).count();
        let n = count(&d);
        for _ in 0..2 {
            ed.hover = Some(end);
            ed.pointer_move(&mut d, v(0.0, 0.0), false, false);
            ed.hover = Some(end);
            ed.tool_down(&mut d, v(0.0, 0.0), Some(end), false, false);
            for k in 1..=5 {
                ed.pointer_move(&mut d, v(0.002 * k as f64, 0.0), false, false);
                ed.tool_drag(&mut d, v(0.002 * k as f64, 0.0));
            }
            ed.tool_up(&mut d, v(0.01, 0.0));
            assert_eq!(count(&d), n, "no second coincident");
        }
    }

    /// SKETCH-START-INFERENCE (live request 1550): a line started on a
    /// line's end and moved along its perpendicular gets Perpendicular; one
    /// started on an arc's end along its tangent gets Tangent.
    #[test]
    fn a_line_started_on_an_end_infers_from_that_curve() {
        let (mut ed, mut d) = editor(Tool::Line);
        // Slanted: an axis-aligned direction would infer Horizontal / Vertical first (live).
        let l0 = d.sketch.add_line(v(0.0, 0.0), v(0.02, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01));
        click(&mut ed, &mut d, v(0.02, 0.01) + v(-0.01, 0.02).normalize() * 0.015);
        let l1 = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).map(|e| e.id).find(|e| *e != l0).unwrap();
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Perpendicular { a, b } if (a == l1 && b == l0) || (a == l0 && b == l1))), "{:?}", d.sketch.constraints().map(|c| c.kind.clone()).collect::<Vec<_>>());
        ed.cancel();
        let (mut ed, mut d) = editor(Tool::Line);
        // A 60° arc centred on the origin; at its end the tangent runs (-sin 60°, cos 60°).
        let (s60, c60) = 60f64.to_radians().sin_cos();
        let end = v(0.02 * c60, 0.02 * s60);
        let arc = d.sketch.add_arc(v(0.0, 0.0), v(0.02, 0.0), end);
        click(&mut ed, &mut d, end);
        click(&mut ed, &mut d, end + v(-s60, c60) * 0.015);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Tangent { a, b } if a == arc || b == arc)), "{:?}", d.sketch.constraints().map(|c| c.kind.clone()).collect::<Vec<_>>());
    }

    /// Live 1716: one Flow glyph, on the spline, two-thirds along it from
    /// the line it flows into.
    #[test]
    fn a_flow_glyph_sits_two_thirds_along_the_spline() {
        let (ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(-0.03, 0.0), v(0.0, 0.0));
        let s = d.sketch.add_fit_spline_with_handles(&[v(0.0, 0.0), v(0.02, 0.01), v(0.04, 0.0)]).unwrap();
        d.sketch.add_constraint(ConstraintKind::Flow { a: s, b: l }).unwrap();
        let g: Vec<_> = ed.constraint_glyphs(&d).into_iter().filter(|g| g.icon == "flow").collect();
        assert_eq!(g.len(), 1);
        let c = d.sketch.curve(s).unwrap();
        let (t0, t1) = c.domain();
        assert!((g[0].anchor - c.point(t0 + (t1 - t0) * 2.0 / 3.0)).length() < 1e-12, "{:?}", g[0].anchor);
    }

    /// ORIGIN-COINCIDENT-GLYPH (live "qa4 r5 s6" (4)): a line drawn from the
    /// Origin shows the coincident glyph at that end, grey on white like any
    /// other (not the external references' blue).
    #[test]
    fn a_line_from_the_origin_shows_its_coincident_glyph() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.02, 0.01));
        ed.cancel();
        let l = d.sketch.entities().find(|e| e.kind() == EntityKind::Line && !e.construction).map(|e| e.id).unwrap();
        let start = d.sketch.endpoints(l).unwrap().0;
        assert!(d.sketch.point(start).unwrap().length() < 1e-12);
        let g: Vec<_> = ed.constraint_glyphs(&d).into_iter().filter(|g| g.icon == "coincident").collect();
        assert_eq!(g.len(), 1, "one coincident glyph: {:?}", g.iter().map(|g| (g.entity, g.anchor)).collect::<Vec<_>>());
        assert!(!g[0].external, "the Origin's glyph isn't tinted as an external reference");
    }

    /// Live sketch-edit-feel-1122 S2: a Ø dimension's text outside the circle has one leader to the near rim (not the
    /// whole diameter); inside it, the whole diameter through the text. An R's text off its arc's ends: a short leader to
    /// the nearer end.
    #[test]
    fn radial_dimension_leaders_follow_the_text_as_live() {
        let (ed, mut d) = editor(Tool::Select);
        let c = d.sketch.add_circle(v(0.0, 0.0), 0.01);
        let cid = add_dimension(&mut d, ConstraintKind::Diameter { curve: c, value: 0.02 }, Some(v(0.03, 0.0)), None).unwrap();
        let segs = |d: &SketchData| ed.dimension_graphics(d).into_iter().find(|g| g.constraint == cid).unwrap().segments;
        let reach = |d: &SketchData| segs(d).iter().flatten().fold(f64::MAX, |m, p| m.min(p.x));
        assert!(reach(&d) > 0.009, "outside: from the near rim only, nothing past the centre: {}", reach(&d));
        d.dimension_labels.insert(cid.0, [0.003, 0.0]);
        assert!(reach(&d) < -0.0099, "inside: the whole diameter: {}", reach(&d));
        let arc = d.sketch.add_arc_angles(v(0.1, 0.0), 0.01, 0.0, std::f64::consts::PI);
        let rid = add_dimension(&mut d, ConstraintKind::Radius { curve: arc, value: 0.01 }, Some(v(0.08, -0.004)), None).unwrap();
        let g = ed.dimension_graphics(&d).into_iter().find(|g| g.constraint == rid).unwrap().segments;
        assert!(g.iter().flatten().all(|p| p.x < 0.0905 && p.y < 0.0005), "off the arc's ends: a leader to the left end only: {g:?}");
    }

    /// Live sketch-edit-feel-1122 S2: an angle keeps the sector it was placed in; its text dragged outside it (into the
    /// vertically opposite sector), the arc runs on past the line to the text, the sector still drawn.
    #[test]
    fn an_angle_keeps_its_placed_sector() {
        let (ed, mut d) = editor(Tool::Select);
        let a = d.sketch.add_line(v(0.0, 0.0), v(0.04, 0.0));
        let dir = DVec2::from_angle(58.57f64.to_radians());
        let b = d.sketch.add_line(v(0.0, 0.0), dir * 0.04);
        let inside = DVec2::from_angle(29f64.to_radians()) * 0.02;
        let cid = add_dimension(&mut d, ConstraintKind::Angle { a, b, value: 58.57f64.to_radians() }, Some(inside), None).unwrap();
        let opposite = DVec2::from_angle(233f64.to_radians()) * 0.02;
        d.dimension_labels.insert(cid.0, [opposite.x, opposite.y]);
        let g = ed.dimension_graphics(&d).into_iter().find(|g| g.constraint == cid).unwrap().segments;
        let at = |deg: f64| g.iter().flatten().any(|p| (p.length() - 0.02).abs() < 1e-4 && (p.to_angle().to_degrees().rem_euclid(360.0) - deg).abs() < 3.0);
        assert!(at(29.0), "the placed sector's arc is still drawn");
        assert!(at(233.0), "the arc runs on to the text");
    }

    /// Live (behaviours "skasm (2026-10-28) dimension labels vs glyph rows"):
    /// a line's length 25.5 px above it (its glyph goes below; lenbox-1122: 25–27); a corner
    /// rectangle's width 42 px below the bottom edge (past the edge's glyph)
    /// and its height on the right edge, 41 px inside.
    #[test]
    fn value_box_dimensions_keep_clear_of_the_glyphs() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.04));
        click(&mut ed, &mut d, v(0.04, 0.04));
        let f = &ed.otf.as_ref().unwrap().fields[0];
        assert!((f.label - v(0.02, 0.04 + 25.5 * PX)).length() < 1e-9, "above the line: {:?}", f.label);
        ed.cancel();
        let (mut ed, mut d) = editor(Tool::CornerRect);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.03, 0.02));
        let o = ed.otf.as_ref().unwrap();
        assert!((o.fields[0].label - v(0.015, -42.0 * PX)).length() < 1e-9, "width: {:?}", o.fields[0].label);
        let ConstraintKind::Length { curve, .. } = o.fields[1].kind else { panic!() };
        let Some(Curve2::Line { a, b }) = d.sketch.curve(curve) else { panic!() };
        assert!((a.x - 0.03).abs() < 1e-9 && (b.x - 0.03).abs() < 1e-9, "the right edge: {a:?} {b:?}");
        assert!((o.fields[1].label - v(0.03 - 41.0 * PX, 0.01)).length() < 1e-9, "height inside: {:?}", o.fields[1].label);
        // Typed 20 x 10: the labels follow the edges as they are then.
        let units = UnitPrefs::default();
        let vars = std::collections::BTreeMap::<String, noshape_units::Quantity>::new();
        ed.commit_otf(&mut d, "20", &units, &vars).unwrap();
        ed.commit_otf(&mut d, "10", &units, &vars).unwrap();
        let label_of = |d: &SketchData, k: fn(&ConstraintKind) -> bool| d.sketch.constraints().find(|c| k(&c.kind)).and_then(|c| d.dimension_labels.get(&c.id.0)).map(|l| DVec2::new(l[0], l[1])).unwrap();
        let w = label_of(&d, |k| matches!(k, ConstraintKind::Length { value, .. } if (*value - 0.02).abs() < 1e-9));
        let h = label_of(&d, |k| matches!(k, ConstraintKind::Length { value, .. } if (*value - 0.01).abs() < 1e-9));
        assert!((w - v(0.01, -42.0 * PX)).length() < 1e-9, "width label: {w:?}");
        assert!((h - v(0.02 - 41.0 * PX, 0.005)).length() < 1e-9, "height label: {h:?}");
    }

    /// SKETCH-RECT-DIM-GLYPHS (b) (live, behaviours "skasm (2026-10-28)
    /// rectangle glyphs"): a Coincident glyph at each corner of a rectangle
    /// (ours shares the corner point); deleting one splits that corner.
    #[test]
    fn a_rectangles_corners_show_coincident_glyphs_and_deleting_one_splits_it() {
        // (Off the Origin, whose own Coincident would add a glyph.)
        let (mut ed, mut d) = editor(Tool::CornerRect);
        click(&mut ed, &mut d, v(0.01, 0.01));
        click(&mut ed, &mut d, v(0.04, 0.03));
        ed.cancel();
        let corners: Vec<_> = ed.constraint_glyphs(&d).into_iter().filter(|g| g.icon == "coincident").collect();
        assert_eq!(corners.len(), 4, "{:?}", corners.iter().map(|g| g.anchor).collect::<Vec<_>>());
        assert!(corners.iter().all(|g| g.on_point && !g.external));
        ed.selected_constraints.push(corners[0].constraint);
        assert_eq!(ed.picked(Some(&d)).len(), 2, "the two edges meeting there");
        assert!(ed.lit_picks(Some(&d)).is_empty(), "a selected glyph's edges don't light with the pointer away (live sketch-edit-feel-1122)");
        ed.selected_constraints.clear();
        let corner = corners[0].anchor;
        assert!(ed.delete_constraint(&mut d, corners[0].constraint));
        assert_eq!(ed.constraint_glyphs(&d).into_iter().filter(|g| g.icon == "coincident").count(), 3);
        let lines: Vec<_> = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line && !e.construction).map(|e| e.id).collect();
        let ends: Vec<EntityId> = lines.iter().flat_map(|l| { let (a, b) = d.sketch.endpoints(*l).unwrap(); [a, b] }).filter(|q| (d.sketch.point(*q).unwrap() - corner).length() < 1e-9).collect();
        assert_eq!(ends.len(), 2, "both lines still end at the corner");
        assert_ne!(ends[0], ends[1], "on their own points now");
    }

    /// SKETCH-VDIM-SIDE-ARROWS (live, behaviours "skasm (2026-10-28)
    /// rectangle glyphs and vertical dimension text"): a vertical line's value
    /// box lands left of its drawing direction (lenbox-1122: right of one drawn downward; a horizontal one's above),
    /// and a short dimension keeps its arrows inside
    /// (their tips at the extension lines, pointing out).
    #[test]
    fn a_vertical_lines_value_box_goes_left_and_short_arrows_stay_inside() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.04));
        click(&mut ed, &mut d, v(0.0, 0.0));
        let f = &ed.otf.as_ref().unwrap().fields[0];
        // (Its centre 40.5 px off, a horizontal line's 25.5 px: lenbox-1122 41 and 25–27.)
        assert!((f.label - v(40.5 * PX, 0.02)).length() < 1e-9, "right of a line drawn downward: {:?}", f.label);
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.04, 0.0));
        let f = &ed.otf.as_ref().unwrap().fields[0];
        assert!((f.label - v(0.02, 25.5 * PX)).length() < 1e-9, "above the line: {:?}", f.label);
        // A 6 px dimension: from each tip the arrow runs inward (overlapping
        // the other), and no shaft sticks out past the extension lines.
        let mut out = Vec::new();
        linear_dim(&mut out, v(0.0, 0.0), v(6.0, 0.0), Some(DVec2::X), v(3.0, 5.0), 1.0);
        for seg in out.iter().filter(|s| (s[0].y - 5.0).abs() < 1e-9 || (s[1].y - 5.0).abs() < 1e-9) {
            for (tip, other) in [(seg[0], seg[1]), (seg[1], seg[0])] {
                if tip.x.abs() < 1e-9 && (tip.y - 5.0).abs() < 1e-9 {
                    assert!(other.x >= -1e-9, "left tip points out: {seg:?}");
                }
                if (tip.x - 6.0).abs() < 1e-9 && (tip.y - 5.0).abs() < 1e-9 {
                    assert!(other.x <= 6.0 + 1e-9, "right tip points out: {seg:?}");
                }
            }
        }
    }

    /// Live (behaviours "skasm (2026-10-27, P1) sketch value boxes: center
    /// point rectangle and tangent arc", request 1503): a center point
    /// rectangle's width box 14 px below the bottom edge, its height text left
    /// of the left edge and, as the box, centred 40 px left of the vertical
    /// edge opposite the corner click (outside the left edge, inside the
    /// right one); a tangent arc's radius box 30 px inside the arc on the ray
    /// through its sweep's middle.
    #[test]
    fn center_rectangle_and_tangent_arc_value_boxes_as_live() {
        let units = UnitPrefs::default();
        for (corner, opposite_x) in [(v(0.040, 0.010), 0.020), (v(0.020, 0.010), 0.040)] {
            let (mut ed, mut d) = editor(Tool::CenterRect);
            click(&mut ed, &mut d, v(0.030, 0.020));
            click(&mut ed, &mut d, corner);
            let b = ed.otf_boxes(&d, &units);
            assert!(b[0].0.y < 0.010 - 14.0 * PX && (b[0].0.x - 0.030).abs() < 1e-9, "width box below the bottom edge: {:?}", b[0].0);
            assert!(b[1].0.x < 0.020 - 16.0 * PX && (b[1].0.y - 0.020).abs() < 1e-9, "height text left of the left edge: {:?}", b[1].0);
            ed.otf_step(1);
            let b = ed.otf_boxes(&d, &units);
            let left_of = opposite_x - b[1].0.x;
            assert!(left_of > 14.0 * PX && left_of < 60.0 * PX && (b[1].0.y - 0.020).abs() < 1e-9, "height box left of x {opposite_x}: {:?}", b[1].0);
            let Some(ConstraintKind::Length { curve, .. }) = ed.otf.as_ref().map(|o| o.fields[1].kind.clone()) else { panic!() };
            let Some(Curve2::Line { a, .. }) = d.sketch.curve(curve) else { panic!() };
            assert!((a.x - opposite_x).abs() < 1e-9, "the height on the edge opposite the corner");
        }
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.020, 0.0));
        ed.set_tool(Tool::TangentArc);
        click(&mut ed, &mut d, v(0.020, 0.0));
        click(&mut ed, &mut d, v(0.030, 0.010));
        let b = ed.otf_boxes(&d, &units);
        let arc = d.sketch.entities().find(|e| e.kind() == EntityKind::Arc).map(|e| e.id).unwrap();
        let Some(Curve2::Arc { center, radius, start_angle, end_angle }) = d.sketch.curve(arc) else { panic!() };
        let want = center + DVec2::from_angle((start_angle + end_angle) * 0.5) * (radius - 30.0 * PX);
        assert!((b[0].0 - want).length() < 1e-9, "radius box {:?}, want {want:?}", b[0].0);
    }

    /// Live sketch-tools-1119: the Tangent arc tool makes one arc per start: after it nothing follows the pointer and
    /// the next arc starts with a click on an end; its preview is a smooth arc, not a few straight pieces.
    #[test]
    fn a_tangent_arc_ends_its_run_and_previews_smoothly() {
        let (mut ed, mut d) = editor(Tool::TangentArc);
        d.sketch.add_line(v(0.0, 0.0), v(0.020, 0.0));
        click(&mut ed, &mut d, v(0.020, 0.0));
        ed.pointer_move(&mut d, v(0.030, 0.010), false, false);
        let (lines, _) = ed.overlay(&d, &UnitPrefs::default());
        assert!(lines.segments.len() >= 20, "a smooth preview: {} pieces", lines.segments.len());
        click(&mut ed, &mut d, v(0.030, 0.010));
        assert_eq!(d.sketch.entities().filter(|e| e.kind() == EntityKind::Arc).count(), 1);
        assert_eq!((ed.tool, ed.chain_end), (Tool::TangentArc, None), "the tool stays, nothing goes on from the arc");
        ed.pointer_move(&mut d, v(0.040, 0.0), false, false);
        click(&mut ed, &mut d, v(0.040, 0.0));
        assert_eq!(d.sketch.entities().filter(|e| e.kind() == EntityKind::Arc).count(), 1, "a click off any end starts nothing");
        let end = d.sketch.entities().find(|e| e.kind() == EntityKind::Arc).and_then(|e| e.endpoints()).map(|(_, e)| d.sketch.point(e).unwrap()).unwrap();
        click(&mut ed, &mut d, end);
        click(&mut ed, &mut d, end + v(0.010, -0.010));
        assert_eq!(d.sketch.entities().filter(|e| e.kind() == EntityKind::Arc).count(), 2, "a new start on the arc's end");
    }

    /// Live (behaviours "spec: skasm sketch tools"): a center point
    /// rectangle's centre is a construction point, Midpoint of both
    /// diagonals (no diagonal lines); a 3 point circle has a construction
    /// point Coincident with it at each click.
    #[test]
    fn center_rectangle_and_three_point_circle_as_live() {
        let (mut ed, mut d) = editor(Tool::CenterRect);
        click(&mut ed, &mut d, v(0.011, 0.013));
        click(&mut ed, &mut d, v(0.021, 0.019));
        assert_eq!(count(&d, EntityKind::Line), 4, "no construction diagonals");
        let mids = d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::MidpointOf { .. })).count();
        assert_eq!(mids, 2);
        let centre = d.sketch.entities().find(|e| e.kind() == EntityKind::Point && e.construction).map(|e| e.id).unwrap();
        assert!((d.sketch.point(centre).unwrap() - v(0.011, 0.013)).length() < 1e-9);
        assert_eq!(d.sketch.solve().dof, Some(4));

        let (mut ed, mut d) = editor(Tool::ThreePointCircle);
        for p in [v(0.011, 0.013), v(0.021, 0.019), v(0.015, 0.025)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!(count(&d, EntityKind::Circle), 1);
        let cons: Vec<EntityId> = d.sketch.entities().filter(|e| e.kind() == EntityKind::Point && e.construction).map(|e| e.id).collect();
        assert_eq!(cons.len(), 3);
        let on = d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::Coincident { a, .. } if cons.contains(&a))).count();
        assert_eq!(on, 3);
    }

    /// Live sk2-1121 3pt-circle-on-points: a circle through a line's ends, the first at the Origin, passes through the
    /// line's own start (no point of its own there): one construction point, four coincidents and the horizontal.
    #[test]
    fn a_three_point_circle_on_a_line_start_at_the_origin_takes_the_start() {
        let (mut ed, mut d) = editor(Tool::Line);
        click(&mut ed, &mut d, v(0.0, 0.0));
        click(&mut ed, &mut d, v(0.03, 0.0));
        ed.set_tool(Tool::ThreePointCircle);
        for p in [v(0.0, 0.0), v(0.03, 0.0), v(0.015, 0.01)] {
            click(&mut ed, &mut d, p);
        }
        let cons = d.sketch.entities().filter(|e| e.kind() == EntityKind::Point && e.construction && !e.external).count();
        let coincident = d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::Coincident { .. })).count();
        assert_eq!((count(&d, EntityKind::Circle), cons, coincident), (1, 1, 4), "(circles, construction points, coincidents)");
    }

    /// Live sk2-1121 split-at-crossing (geo-list3 / geo-list6): a split by a line's middle cuts at the midpoint, MIDPOINT
    /// of the line's ends, the pieces collinear and joined; elsewhere where clicked, no MIDPOINT.
    #[test]
    fn a_split_by_a_lines_middle_takes_the_midpoint() {
        let midpoints = |d: &SketchData| d.sketch.constraints().filter(|c| matches!(c.kind, ConstraintKind::MidpointOf { .. })).count();
        for (click_at, cut, mids) in [(v(0.0153, 0.0002), v(0.015, 0.0), 1), (v(0.02, 0.0001), v(0.02, 0.0), 0)] {
            let (mut ed, mut d) = editor(Tool::Split);
            d.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
            click(&mut ed, &mut d, click_at);
            let ends: Vec<DVec2> = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line).filter_map(|e| d.sketch.endpoints(e.id)).map(|(_, b)| d.sketch.point(b).unwrap()).collect();
            assert_eq!(count(&d, EntityKind::Line), 2, "split in two");
            assert!(ends.iter().any(|q| (*q - cut).length() < 1e-9), "cut at {cut}: {ends:?}");
            assert_eq!(midpoints(&d), mids, "MIDPOINT of the ends at the middle only");
            assert!(matches!(d.sketch.solve().status, noshape_sketch::SolveStatus::Solved), "consistent");
        }
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
    fn mirror_takes_a_lone_preselected_line_as_its_line() {
        let (mut ed, mut d) = editor(Tool::Select);
        let axis = d.sketch.add_line(v(0.0, 0.0), v(0.0, 0.04));
        let c = d.sketch.add_circle(v(0.01, 0.02), 0.003);
        ed.selection = vec![axis];
        ed.set_tool(Tool::Mirror);
        ed.apply_preselection(&mut d);
        assert_eq!(ed.message.as_deref(), Some("Select entities to be mirrored."));
        click(&mut ed, &mut d, v(0.013, 0.02));
        let _ = c;
        assert_eq!(count(&d, EntityKind::Circle), 2);
        assert_eq!(ed.tool, Tool::Mirror);
    }

    /// Live (sketch-mirror-preselected-batch, 0.275 mm a px): a circle centred at (20, 20) with its radius point at
    /// (23, 20) is tangent to the line from (5, 0) to (15, 10) (its tangent radius 3.54 mm, 1.85 px further); 3 px
    /// off a tangent radius it isn't (the snaps matrix).
    #[test]
    fn a_circle_snaps_tangent_to_a_line_near_its_radius() {
        let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
        let tangent = |radius_mm: f64| {
            let (mut ed, mut d) = editor(Tool::Circle);
            ed.world_per_px = 0.275365e-3;
            let l = d.sketch.add_line(mm(5.0, 0.0), mm(15.0, 10.0));
            click(&mut ed, &mut d, mm(20.0, 20.0));
            click(&mut ed, &mut d, mm(20.0 + radius_mm, 20.0));
            d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Tangent { b, .. } if b == l))
        };
        assert!(tangent(3.0), "1.85 px off: tangent");
        let tangent_r = 5.0 / 2f64.sqrt();
        assert!(!tangent(tangent_r - 3.0 * 0.275365), "3 px off: not");
    }

    /// Live (sketch-mirror-preselected-batch): a left-to-right box over a line and a circle selects the two, not their
    /// own points; (sketch-mirror-construction-line) a copy mirrored in construction mode is what its source is.
    #[test]
    fn a_box_selects_curves_without_their_points_and_mirror_copies_keep_their_kind() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.01, 0.0), v(0.015, 0.01));
        let c = d.sketch.add_circle(v(0.02, 0.02), 0.003);
        drag(&mut ed, &mut d, v(0.003, -0.003), v(0.027, 0.026));
        let mut sel = ed.selection.clone();
        sel.sort();
        assert_eq!(sel, vec![l, c]);
        let axis = d.sketch.add_line(v(0.0, -0.01), v(0.0, 0.03));
        ed.selection.clear();
        ed.construction = true;
        ed.set_tool(Tool::Mirror);
        click(&mut ed, &mut d, v(0.0, 0.025));
        click(&mut ed, &mut d, v(0.0125, 0.005));
        let copy = d.sketch.entities().filter(|e| e.kind() == EntityKind::Line && e.id != l && e.id != axis).map(|e| e.id).next().expect("the copy");
        assert!(!d.sketch.entity(copy).unwrap().construction, "the copy isn't construction");
    }

    /// Live (sketch-mirror-construction-line, -escape-after-line): a click on the mirror line again deselects it (it
    /// stays the mirror line); Escape leaves it unselected, so Mirror again asks for a line.
    #[test]
    fn the_mirror_line_deselects_on_a_click_and_on_escape() {
        let (mut ed, mut d) = editor(Tool::Mirror);
        let axis = d.sketch.add_line(v(0.0, 0.0), v(0.0, 0.04));
        click(&mut ed, &mut d, v(0.0, 0.02));
        assert_eq!(ed.selection, vec![axis]);
        click(&mut ed, &mut d, v(0.0, 0.03));
        assert!(ed.selection.is_empty() && ed.has_mirror_axis(), "deselected, still the mirror line");
        ed.selection = vec![axis];
        ed.cancel();
        assert!(ed.selection.is_empty(), "Escape: not selected");
        ed.set_tool(Tool::Mirror);
        assert_eq!(ed.message.as_deref(), Some(MIRROR_LINE_PROMPT));
    }

    /// Live (sketch-mirror-preselected-batch): entities preselected, then the line clicked: mirrored at once, the
    /// tool stays on, asking for more.
    #[test]
    fn mirror_with_preselection_resolves_and_stays_on() {
        let (mut ed, mut d) = editor(Tool::Select);
        let axis = d.sketch.add_line(v(0.0, 0.0), v(0.0, 0.04));
        let c = d.sketch.add_circle(v(0.01, 0.02), 0.003);
        ed.selection = vec![c];
        ed.set_tool(Tool::Mirror);
        click(&mut ed, &mut d, v(0.0, 0.03));
        let _ = axis;
        assert_eq!(count(&d, EntityKind::Circle), 2);
        assert_eq!((ed.tool, ed.message.as_deref()), (Tool::Mirror, Some(MIRROR_ENTITIES_PROMPT)));
    }

    /// Live (sketch-mirror-preselected-batch): a circle and a line preselected, then Mirror: the line is the mirror
    /// line, the circle mirrored at once (3 → 4), nothing stays selected, the tool asks for more.
    #[test]
    fn mirror_takes_the_line_of_a_preselection_and_mirrors_the_rest() {
        let (mut ed, mut d) = editor(Tool::Select);
        let axis = d.sketch.add_line(v(0.0, 0.0), v(0.0, 0.04));
        let c = d.sketch.add_circle(v(0.01, 0.02), 0.003);
        ed.selection = vec![c, axis];
        ed.set_tool(Tool::Mirror);
        ed.apply_preselection(&mut d);
        assert_eq!(count(&d, EntityKind::Circle), 2);
        assert!(ed.selection.is_empty());
        assert_eq!((ed.tool, ed.message.as_deref()), (Tool::Mirror, Some(MIRROR_ENTITIES_PROMPT)));
    }

    /// Live2 (feel/mirror-preselected): two lines and a circle selected, Mirror takes neither line, copies nothing and
    /// asks for the mirror line with all three still selected; Escape clears them.
    #[test]
    fn mirror_with_two_preselected_lines_asks_for_the_line() {
        let (mut ed, mut d) = editor(Tool::Select);
        let a = d.sketch.add_line(v(0.0, 0.0), v(0.0, 0.04));
        let b = d.sketch.add_line(v(0.005, 0.0), v(0.015, 0.01));
        let c = d.sketch.add_circle(v(0.02, 0.02), 0.003);
        ed.selection = vec![b, c, a];
        ed.set_tool(Tool::Mirror);
        ed.apply_preselection(&mut d);
        assert_eq!((count(&d, EntityKind::Line), count(&d, EntityKind::Circle)), (2, 1), "nothing copied");
        assert_eq!(ed.selection, vec![b, c, a]);
        assert!(!ed.has_mirror_axis());
        assert_eq!((ed.tool, ed.message.as_deref()), (Tool::Mirror, Some(MIRROR_LINE_PROMPT)));
        assert!(ed.cancel());
        assert!(ed.selection.is_empty());
        assert_eq!(ed.tool, Tool::Select);
    }

    #[test]
    fn copy_paste_selection() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.0, 0.0), v(0.01, 0.0));
        ed.selection = vec![l];
        let clip = ed.copy_selection(&d).unwrap();
        ed.pointer_move(&mut d, v(0.05, 0.03), false, false);
        assert!(ed.paste_clipboard(&mut d, &clip));
        assert_eq!(count(&d, EntityKind::Line), 2);
        assert_eq!(ed.selection.len(), 1);
        assert_ne!(ed.selection[0], l);
        // Live: centred on the pointer, the Transform tool on it, no prompt.
        let c = d.sketch.curve(ed.selection[0]).unwrap();
        assert!(((c.start_point() + c.end_point()) * 0.5 - v(0.05, 0.03)).length() < 1e-9);
        assert_eq!(ed.tool, Tool::Transform);
        assert_eq!(ed.message, None);
        assert!((ed.pattern_center - v(0.045, 0.03)).length() < 1e-9, "the triad sits on the copy's lowest, leftmost end: {}", ed.pattern_center);
        assert!(!ed.paste_clipboard(&mut d, "hello"));
        // Live2 11-18 keys-1118: a 40 x 30 rectangle's copy lands with its bounding box's centre on the pointer,
        // selected, Transform's triad on its lowest-left corner, and the constraints among its entities come along.
        let (mut ed, mut d) = editor(Tool::Select);
        let r = d.sketch.corner_rectangle(v(0.0, 0.0), v(0.04, 0.03));
        let before = d.sketch.constraints().count();
        ed.selection = r.curves.clone();
        let clip = ed.copy_selection(&d).unwrap();
        ed.pointer_move(&mut d, v(0.1, 0.05), false, false);
        assert!(ed.paste_clipboard(&mut d, &clip));
        let b = ed.selection.iter().filter_map(|e| d.sketch.entity_bbox(*e)).reduce(|a, b| a.union(&b)).unwrap();
        assert!((b.center() - v(0.1, 0.05)).length() < 1e-9, "bbox centre on the pointer: {:?}", b.center());
        assert_eq!((ed.selection.len(), ed.tool, d.sketch.constraints().count()), (4, Tool::Transform, 2 * before), "(selected, tool, constraints doubled)");
        assert!((ed.pattern_center - v(0.08, 0.035)).length() < 1e-9, "the triad on the copy's lowest-left corner: {}", ed.pattern_center);
    }

    /// Live (feel round 2 R6, cov2-copypaste): the pasted copy follows the pointer, its triad with it; a click
    /// places it where it is and leaves the tool with nothing selected.
    #[test]
    fn a_paste_follows_the_pointer_until_a_click() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
        ed.selection = vec![l];
        let clip = ed.copy_selection(&d).unwrap();
        ed.pointer_move(&mut d, v(0.015, 0.0), false, false);
        assert!(ed.paste_clipboard(&mut d, &clip));
        let copy = ed.selection[0];
        let mid = |d: &SketchData| d.sketch.curve(copy).map(|c| (c.start_point() + c.end_point()) * 0.5).unwrap();
        assert!(ed.pointer_move(&mut d, v(0.04, 0.03), false, false).changed);
        assert!((mid(&d) - v(0.04, 0.03)).length() < 1e-9, "the copy follows: {}", mid(&d));
        assert!((ed.pattern_center - v(0.025, 0.03)).length() < 1e-9, "and its triad: {}", ed.pattern_center);
        click(&mut ed, &mut d, v(0.04, 0.03));
        assert_eq!((ed.tool, ed.selection.len(), count(&d, EntityKind::Line)), (Tool::Select, 0, 2));
        assert!((mid(&d) - v(0.04, 0.03)).length() < 1e-9);
        assert!(!ed.pointer_move(&mut d, v(0.06, 0.01), false, false).changed, "placed: it stays");
        assert!((d.sketch.curve(l).unwrap().start_point() - v(0.0, 0.0)).length() < 1e-12, "the original never moves");
    }

    /// Live uip sketch-entity-copy-paste: a copy pasted from the menu stays where it lands; a click on it keeps it
    /// selected and the Transform tool on.
    #[test]
    fn a_menu_paste_stays_and_a_click_on_it_keeps_the_tool() {
        let (mut ed, mut d) = editor(Tool::Select);
        let l = d.sketch.add_line(v(0.0, 0.0), v(0.03, 0.0));
        ed.selection = vec![l];
        let clip = ed.copy_selection(&d).unwrap();
        ed.pointer_move(&mut d, v(0.015, 0.02), false, false);
        assert!(ed.paste_clipboard(&mut d, &clip));
        ed.paste_follow = None;
        let copy = ed.selection[0];
        let mid = |d: &SketchData| d.sketch.curve(copy).map(|c| (c.start_point() + c.end_point()) * 0.5).unwrap();
        assert!(!ed.pointer_move(&mut d, v(0.04, 0.03), false, false).changed, "it doesn't follow");
        ed.pointer_move(&mut d, v(0.015, 0.02), false, false);
        click(&mut ed, &mut d, v(0.015, 0.02));
        assert_eq!((ed.tool, ed.selection.clone(), count(&d, EntityKind::Line)), (Tool::Transform, vec![copy], 2));
        assert!((mid(&d) - v(0.015, 0.02)).length() < 1e-9, "where it landed: {}", mid(&d));
    }

    /// qa4 round 5 v3 s6 with live's answers: Normal takes either pick
    /// order; a pick the constraint can't use isn't taken; Tangent between
    /// a line and a circle moves the circle and keeps its radius.
    #[test]
    fn constraint_picks_and_application_as_live() {
        let mut sk = Sketch::new();
        let line = sk.add_line(v(0.0, 0.0), v(0.04, 0.0));
        let line2 = sk.add_line(v(0.0, 0.02), v(0.04, 0.025));
        let circle = sk.add_circle(v(0.02, 0.01), 0.004);
        let p = sk.add_point(v(0.05, 0.05));
        assert_eq!(make_constraint(&sk, ConstraintTool::Normal, &[circle, line]), Some(ConstraintKind::Normal { line, curve: circle }));
        assert_eq!(make_constraint(&sk, ConstraintTool::Normal, &[line, circle]), Some(ConstraintKind::Normal { line, curve: circle }));
        assert!(!pick_fits(&sk, ConstraintTool::Parallel, &[], circle));
        assert!(pick_fits(&sk, ConstraintTool::Parallel, &[line], line2));
        assert!(!pick_fits(&sk, ConstraintTool::Parallel, &[line], circle));
        assert!(!pick_fits(&sk, ConstraintTool::Tangent, &[], p));
        assert!(pick_fits(&sk, ConstraintTool::Horizontal, &[], p) && !pick_fits(&sk, ConstraintTool::Horizontal, &[p], circle));
        sk.add_constraint(ConstraintKind::Tangent { a: line, b: circle }).unwrap();
        assert!(solve_after_constraint(&mut sk, ConstraintTool::Tangent, &[line, circle]));
        let Some(Curve2::Circle { center, radius }) = sk.curve(circle) else { panic!() };
        assert!((radius - 0.004).abs() < 1e-12, "radius {radius}");
        assert!((center.y.abs() - 0.004).abs() < 1e-9, "{center:?}");
        let Some(Curve2::Line { a, b }) = sk.curve(line) else { panic!() };
        assert!(a.distance(v(0.0, 0.0)) < 1e-12 && b.distance(v(0.04, 0.0)) < 1e-12);
    }

    /// Live "qa4 round 5 s5 driven dims": a driven dimension reads "Ø10",
    /// not "(Ø10)".
    #[test]
    fn driven_labels_as_live() {
        let (ed, mut data) = editor(Tool::Select);
        let c = data.sketch.add_circle(v(0.0, 0.0), 0.005);
        add_dimension(&mut data, ConstraintKind::Diameter { curve: c, value: 0.01 }, Some(v(0.01, 0.01)), None).unwrap();
        let driven = add_dimension(&mut data, ConstraintKind::Diameter { curve: c, value: 0.01 }, Some(v(-0.01, 0.01)), None).unwrap();
        assert!(!data.sketch.constraint(driven).unwrap().driving);
        for (_, _, text, _) in ed.dimension_labels(&data, &UnitPrefs::default()) {
            assert!(!text.starts_with('('), "{text}");
        }
    }

    /// Live walk3-1121 (2): a Line chain through the Origin keeps every segment: (-30, 0) → Origin → (0, 20) → (-30, 20)
    /// makes 3 lines (qa FEEL-CHAIN-ORIGIN-DROP).
    #[test]
    fn a_line_chain_through_the_origin_keeps_its_segments() {
        let (mut ed, mut d) = editor(Tool::Line);
        for p in [v(-0.03, 0.0), v(0.0, 0.0), v(0.0, 0.02), v(-0.03, 0.02)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!(count(&d, EntityKind::Line), 3, "{:?}", d.sketch.entities().filter_map(|e| d.sketch.curve(e.id)).collect::<Vec<_>>());
    }

    /// Live walk3-1121 (6a): a tangent arc started at a line's end on the Origin is made, Tangent to the line and
    /// Coincident with its end (qa FEEL-TANGENT-ARC-AT-ORIGIN).
    #[test]
    fn a_tangent_arc_from_a_line_end_on_the_origin() {
        let (mut ed, mut d) = editor(Tool::Line);
        for p in [v(-0.03, 0.0), v(0.0, 0.0)] {
            click(&mut ed, &mut d, p);
        }
        ed.set_tool(Tool::TangentArc);
        for p in [v(0.0, 0.0), v(0.015, 0.012)] {
            click(&mut ed, &mut d, p);
        }
        assert_eq!(count(&d, EntityKind::Arc), 1, "{:?}", ed.message);
        assert!(d.sketch.constraints().any(|c| matches!(c.kind, ConstraintKind::Tangent { .. })));
        // Live walk3-1121 (6b): a Slot clicked on that line at the Origin, then Enter: 2 arcs, 3 lines.
        let (mut ed, mut d) = editor(Tool::Line);
        for p in [v(-0.03, 0.0), v(0.0, 0.0)] {
            click(&mut ed, &mut d, p);
        }
        ed.set_tool(Tool::Slot);
        let line = d.sketch.entities().find(|e| e.kind() == EntityKind::Line).map(|e| e.id).unwrap();
        let end = d.sketch.endpoints(line).map(|(a, b)| if d.sketch.point(a).unwrap().length() < 1e-9 { a } else { b }).unwrap();
        ed.pointer_move(&mut d, v(0.0, 0.0), false, false);
        ed.tool_down(&mut d, v(0.0, 0.0), Some(end), false, false);
        ed.tool_up(&mut d, v(0.0, 0.0));
        assert!(ed.commit_slot(&mut d).changed, "{:?}", ed.message);
        assert_eq!((count(&d, EntityKind::Arc), count(&d, EntityKind::Line)), (2, 3));
    }

    /// Live walk3-1121 (5) c2still / c3still: with the pointer still after the 2nd and 3rd click, the spline through the
    /// clicks stays drawn (qa FEEL-SPLINE-PREVIEW-VANISHES).
    #[test]
    fn a_spline_stays_drawn_with_the_pointer_still_after_a_click() {
        let (mut ed, mut d) = editor(Tool::Spline);
        for p in [v(0.0, 0.0), v(0.02, 0.01), v(0.04, 0.0)] {
            click(&mut ed, &mut d, p);
            let (lines, _) = ed.overlay(&d, &UnitPrefs::default());
            let blue = lines.segments.iter().filter(|s| s.color == [68, 156, 205, 255]).count();
            let want = match ed.clicks.len() { 0 | 1 => 0, 2 => 1, _ => 5 };
            assert!(blue >= want, "after click {}: {blue} preview segments", ed.clicks.len());
        }
    }

    /// Live sketch/132: with the Text dialog open only the dash-dot box and its 4 corner points, no text.
    #[test]
    fn the_text_box_is_drawn_without_its_text_while_the_dialog_is_open() {
        let (mut ed, mut d) = editor(Tool::Text);
        click(&mut ed, &mut d, v(-0.02, 0.0));
        click(&mut ed, &mut d, v(0.02, 0.01));
        assert!(ed.pending_text.is_some());
        let (lines, points) = ed.overlay(&d, &UnitPrefs::default());
        let blue: Vec<_> = lines.segments.iter().filter(|s| s.color == PREVIEW_COLOR).collect();
        assert!(blue.len() > 8, "dash-dot pieces: {}", blue.len());
        let on_box = |p: [f32; 3]| [(p[0] + 0.02).abs(), (p[0] - 0.02).abs(), p[1].abs(), (p[1] - 0.01).abs()].iter().any(|d| *d < 1e-6);
        assert!(blue.iter().all(|s| on_box(s.a) && on_box(s.b) && s.style & LineSegment::DASHED == 0), "text strokes drawn");
        for c in [v(-0.02, 0.0), v(0.02, 0.0), v(0.02, 0.01), v(-0.02, 0.01)] {
            assert!(points.points.iter().any(|p| p.color == PREVIEW_COLOR && (p.position[0] as f64 - c.x).abs() < 1e-6 && (p.position[1] as f64 - c.y).abs() < 1e-6), "corner {c}");
        }
    }
}

#[cfg(test)]
mod dash_dot_tests {
    use super::dash_dot_pieces;
    use glam::DVec2;

    /// qa5 POLY1: the pattern runs along the whole circle, not per segment.
    #[test]
    fn dash_dot_follows_the_whole_polyline() {
        // A 100-long line in 50 segments of 2: period 10 → 10 dashes of 5.5
        // and 10 dots of 1.2, whatever the segment length.
        let pts: Vec<DVec2> = (0..=50).map(|i| DVec2::new(i as f64 * 2.0, 0.0)).collect();
        let pieces = dash_dot_pieces(&pts, 10.0);
        assert_eq!(pieces.len(), 20);
        let len = |p: &Vec<DVec2>| p.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f64>();
        assert!((len(&pieces[0]) - 5.5).abs() < 1e-9, "{}", len(&pieces[0]));
        assert!((len(&pieces[1]) - 1.2).abs() < 1e-9);
        assert!((pieces[2][0].x - 10.0).abs() < 1e-9);
    }
}
