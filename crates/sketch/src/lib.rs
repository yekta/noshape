//! noshape-sketch: the 2D sketcher core.
//!
//! * [`Sketch`] stores [`Entity`]s (points, lines, circles, arcs, ellipses,
//!   B-splines, fit splines, conics) and [`Constraint`]s (geometric
//!   constraints and driving/driven dimensions).
//! * [`Sketch::solve`] / [`Sketch::drag`] run the geometric constraint solver
//!   (damped Gauss-Newton / Levenberg-Marquardt with exact Jacobians, sparse
//!   envelope Cholesky, component decomposition) and report degrees of
//!   freedom and per-entity constraint state.
//! * [`Sketch::regions`] builds the planar arrangement of all
//!   non-construction curves and returns every bounded region (with holes);
//!   [`Sketch::wires`] returns connected chains of curves.
//! * [`tools`] implements editing operations (trim, extend, split, offset,
//!   fillet, mirror, patterns, transform, use/convert) and [`shapes`] the
//!   compound creation tools (rectangles, polygons, slots, arcs, circles).
//! * [`inference`] computes snapping / auto-constraint candidates for the UI.
//!
//! All quantities are SI (meters, radians) in `f64`.

pub mod constraints;
mod dof;
mod dual;
pub mod entities;
mod equations;
pub mod eval;
pub mod geom;
pub mod ids;
pub mod inference;
pub mod intersect;
mod linalg;
pub mod nurbs;
pub mod regions;
pub mod shapes;
pub mod sketch;
pub mod solver;
pub mod tools;

pub use constraints::{reflect, Constraint, ConstraintKind, DistanceMode, Hints};
pub use entities::{Entity, EntityKind, Geometry};
pub use geom::{BBox, Curve2};
pub use ids::{ConstraintId, EntityId};
pub use inference::{infer, AutoConstraint, Inference, InferenceContext, InferenceKind, SketchTool};
pub use nurbs::Nurbs;
pub use regions::{ProfileLoop, ProfileSegment, Region, RegionId, SegmentSource, Wire};
pub use shapes::*;
pub use sketch::{Sketch, SketchError};
pub use solver::{ConstraintState, DragHandle, SolveOptions, SolveReport, SolveStatus};
pub use tools::{EditResult, Transform};

pub use glam::DVec2;
