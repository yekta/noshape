//! Drawing editor: Onshape's smart Dimension tool (d). "The type of curve
//! selected determines the type of dimension": a circle gives a diameter,
//! an arc a radius, a line its length; two points, a point and a line, two
//! parallel lines (distance) or two lines at an angle (angular), a circle
//! with anything (from its center), two concentric circles (wall
//! thickness). Clicking a picked edge again deselects it.
//!
//! The tool itself is GUI-free: `noshape_scene::drawing_tools` (smart.rs).
