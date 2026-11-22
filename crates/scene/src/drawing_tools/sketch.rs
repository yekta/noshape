//! The drawing sketch tools and constraints (from
//! `crates/app/src/drawing_sketch.rs`).

/// Drawing sketch tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkTool {
    Line,
    Circle,
    Circle3,
    TangentArc,
    CornerRect,
    CenterRect,
    Spline,
    SplinePoint,
    Point,
    Trim,
}

impl SkTool {
    pub fn name(self) -> &'static str {
        match self {
            SkTool::Line => "Line",
            SkTool::Circle => "Center point circle",
            SkTool::Circle3 => "3 point circle",
            SkTool::TangentArc => "Tangent arc",
            SkTool::CornerRect => "Corner rectangle",
            SkTool::CenterRect => "Center point rectangle",
            SkTool::Spline => "Spline",
            SkTool::SplinePoint => "Spline point",
            SkTool::Point => "Sketch point",
            SkTool::Trim => "Trim",
        }
    }
    pub fn prompt(self, n: usize) -> &'static str {
        match (self, n) {
            (SkTool::Line, 0) => "Click to start the line",
            (SkTool::Line, _) => "Click to add a segment; Esc ends the line",
            (SkTool::Circle, 0) => "Click the center",
            (SkTool::Circle, _) => "Click to set the radius",
            (SkTool::Circle3, _) => "Click three points on the circle",
            (SkTool::TangentArc, 0) => "Click a line end to start the tangent arc",
            (SkTool::TangentArc, _) => "Click the end of the arc",
            (SkTool::CornerRect, 0) => "Click a corner",
            (SkTool::CornerRect, _) => "Click the diagonal corner",
            (SkTool::CenterRect, 0) => "Click the center",
            (SkTool::CenterRect, _) => "Click a corner",
            (SkTool::Spline, _) => "Click points; double-click or Esc ends the spline",
            (SkTool::SplinePoint, _) => "Click a spline to add a point",
            (SkTool::Point, _) => "Click to place a sketch point",
            (SkTool::Trim, _) => "Click a curve or point to trim it",
        }
    }
}

/// Drawing constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkCon {
    Horizontal,
    Vertical,
    Coincident,
    Perpendicular,
    Parallel,
    Tangent,
}

impl SkCon {
    pub fn name(self) -> &'static str {
        match self {
            SkCon::Horizontal => "Horizontal",
            SkCon::Vertical => "Vertical",
            SkCon::Coincident => "Coincident",
            SkCon::Perpendicular => "Perpendicular",
            SkCon::Parallel => "Parallel",
            SkCon::Tangent => "Tangent",
        }
    }
    /// Entities the constraint needs (Horizontal / Vertical: one line, or two points).
    pub fn needs(self) -> usize {
        match self {
            SkCon::Horizontal | SkCon::Vertical => 1,
            _ => 2,
        }
    }
}

/// Whether `picks` (sketch entity ids) complete constraint `kind`: Horizontal / Vertical take one line, or two points;
/// the rest two.
pub fn constraint_ready(sk: &noshape_sketch::Sketch, kind: SkCon, picks: &[u32]) -> bool {
    let one_line = picks.len() == 1 && matches!(sk.entity(noshape_sketch::EntityId(picks[0])).map(|e| &e.geometry), Some(noshape_sketch::Geometry::Line { .. }));
    (kind.needs() == 1 && one_line) || picks.len() >= 2
}

/// The sketch constraint of `kind` on `picks` (sketch entity ids, in pick order).
pub fn sketch_constraint(kind: SkCon, picks: &[u32]) -> Option<noshape_sketch::ConstraintKind> {
    use noshape_sketch::{ConstraintKind, EntityId};
    let (a, b) = (EntityId(*picks.first()?), picks.get(1).map(|x| EntityId(*x)));
    match (kind, b) {
        (SkCon::Horizontal, b) => Some(ConstraintKind::Horizontal { a, b }),
        (SkCon::Vertical, b) => Some(ConstraintKind::Vertical { a, b }),
        (SkCon::Coincident, Some(b)) => Some(ConstraintKind::Coincident { a, b }),
        (SkCon::Perpendicular, Some(b)) => Some(ConstraintKind::Perpendicular { a, b }),
        (SkCon::Parallel, Some(b)) => Some(ConstraintKind::Parallel { a, b }),
        (SkCon::Tangent, Some(b)) => Some(ConstraintKind::Tangent { a, b }),
        _ => None,
    }
}

impl super::SheetPick<'_> {
    /// What a constraint tool's click at `c` picks: a sheet sketch point or curve (its id), else a view's straight edge
    /// or face-on circle as a sheet curve (the caller copies it into the sketch as fixed external geometry).
    pub fn constrain_pick(&self, c: [f64; 2]) -> Option<Result<noshape_sketch::EntityId, noshape_sketch::Curve2>> {
        use noshape_sketch::{Curve2, DVec2};
        let v2 = |p: [f64; 2]| DVec2::new(p[0], p[1]);
        if let Some((id, _)) = self.sketch_point_at(c) {
            return Some(Ok(id));
        }
        if let Some(id) = self.sketch_entity_at() {
            return Some(Ok(id));
        }
        if let Some(super::Pick::Line { a, b }) = self.line_pick() {
            return Some(Err(Curve2::Line { a: v2(self.pick_sheet(&a)?), b: v2(self.pick_sheet(&b)?) }));
        }
        match self.circle_pick(c) {
            Some(super::Pick::Circle { view, center, radius, .. }) => self.placement(view).map(|pl| Err(Curve2::Circle { center: v2(pl.to_sheet(center)), radius: pl.len_to_sheet(radius) })),
            _ => None,
        }
    }
}
