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
