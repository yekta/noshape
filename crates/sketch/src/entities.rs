//! Sketch entities.
//!
//! Curves reference point entities for their defining points (endpoints,
//! centers, control points) so those points can be constrained, dimensioned,
//! dragged and shared between curves (a shared point is equivalent to a
//! coincident constraint, but cheaper to solve).

use crate::ids::EntityId;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Geometry of a sketch entity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    Point { pos: DVec2 },
    /// Line segment between two point entities.
    Line { start: EntityId, end: EntityId },
    Circle { center: EntityId, radius: f64 },
    /// Circular arc, CCW from `start` to `end`. Its radius is `|start - center|`;
    /// the solver keeps `|end - center|` equal to it.
    Arc { center: EntityId, start: EntityId, end: EntityId },
    /// Ellipse; `major` is the semi-major axis vector (direction * radius).
    Ellipse { center: EntityId, major: DVec2, minor: f64 },
    /// Elliptical arc, CCW from `start` to `end` (both kept on the ellipse).
    EllipticalArc { center: EntityId, major: DVec2, minor: f64, start: EntityId, end: EntityId },
    /// Control-point B-spline (optionally rational).
    BSpline { control: Vec<EntityId>, degree: usize, knots: Vec<f64>, weights: Option<Vec<f64>> },
    /// Interpolating spline through the given points (cubic, chord-length).
    /// `handles`: tangent handle points at the start / end (Onshape's white
    /// handles, see [`crate::nurbs::Nurbs::interpolate_with_handles`]).
    /// Without handles the ends are natural.
    FitSpline {
        points: Vec<EntityId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handles: Option<(EntityId, EntityId)>,
    },
    /// Conic as a rational quadratic Bézier (Onshape rho value).
    Conic { start: EntityId, control: EntityId, end: EntityId, rho: f64 },
}

/// Coarse classification used for constraint validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Point,
    Line,
    Circle,
    Arc,
    Ellipse,
    EllipticalArc,
    BSpline,
    FitSpline,
    Conic,
}

impl EntityKind {
    pub fn is_curve(self) -> bool {
        self != EntityKind::Point
    }
    /// Circle or arc.
    pub fn is_circular(self) -> bool {
        matches!(self, EntityKind::Circle | EntityKind::Arc)
    }
    pub fn is_elliptical(self) -> bool {
        matches!(self, EntityKind::Ellipse | EntityKind::EllipticalArc)
    }
    /// Spline-like curves (endpoint tangency / curvature are supported).
    pub fn is_spline(self) -> bool {
        matches!(self, EntityKind::BSpline | EntityKind::FitSpline | EntityKind::Conic)
    }
    /// Has a center point.
    pub fn has_center(self) -> bool {
        self.is_circular() || self.is_elliptical()
    }
}

/// A sketch entity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub geometry: Geometry,
    /// Construction geometry is ignored for regions/wires.
    pub construction: bool,
    /// External (projected via "Use"/"Convert") geometry is fixed: the
    /// solver never moves it.
    pub external: bool,
}

impl Entity {
    pub fn kind(&self) -> EntityKind {
        match self.geometry {
            Geometry::Point { .. } => EntityKind::Point,
            Geometry::Line { .. } => EntityKind::Line,
            Geometry::Circle { .. } => EntityKind::Circle,
            Geometry::Arc { .. } => EntityKind::Arc,
            Geometry::Ellipse { .. } => EntityKind::Ellipse,
            Geometry::EllipticalArc { .. } => EntityKind::EllipticalArc,
            Geometry::BSpline { .. } => EntityKind::BSpline,
            Geometry::FitSpline { .. } => EntityKind::FitSpline,
            Geometry::Conic { .. } => EntityKind::Conic,
        }
    }

    /// Point entities this entity references (in a fixed order).
    pub fn point_refs(&self) -> SmallVec<[EntityId; 4]> {
        match &self.geometry {
            Geometry::Point { .. } => SmallVec::new(),
            Geometry::Line { start, end } => SmallVec::from_slice(&[*start, *end]),
            Geometry::Circle { center, .. } | Geometry::Ellipse { center, .. } => SmallVec::from_slice(&[*center]),
            Geometry::Arc { center, start, end } | Geometry::EllipticalArc { center, start, end, .. } => {
                SmallVec::from_slice(&[*center, *start, *end])
            }
            Geometry::BSpline { control, .. } => control.iter().copied().collect(),
            Geometry::FitSpline { points, handles } => points.iter().copied().chain(handles.iter().flat_map(|h| [h.0, h.1])).collect(),
            Geometry::Conic { start, control, end, .. } => SmallVec::from_slice(&[*start, *control, *end]),
        }
    }

    /// Tangent handle points of a fit spline.
    pub fn spline_handles(&self) -> Option<(EntityId, EntityId)> {
        match &self.geometry {
            Geometry::FitSpline { handles, .. } => *handles,
            _ => None,
        }
    }

    /// Number of own scalar parameters (not counting referenced points).
    pub(crate) fn own_param_count(&self) -> usize {
        match self.geometry {
            Geometry::Point { .. } => 2,
            Geometry::Circle { .. } => 1,
            Geometry::Ellipse { .. } | Geometry::EllipticalArc { .. } => 3,
            _ => 0,
        }
    }

    pub(crate) fn own_params(&self) -> SmallVec<[f64; 3]> {
        match &self.geometry {
            Geometry::Point { pos } => SmallVec::from_slice(&[pos.x, pos.y]),
            Geometry::Circle { radius, .. } => SmallVec::from_slice(&[*radius]),
            Geometry::Ellipse { major, minor, .. } | Geometry::EllipticalArc { major, minor, .. } => {
                SmallVec::from_slice(&[major.x, major.y, *minor])
            }
            _ => SmallVec::new(),
        }
    }

    pub(crate) fn set_own_param(&mut self, slot: usize, v: f64) {
        match &mut self.geometry {
            Geometry::Point { pos } => {
                if slot == 0 { pos.x = v } else { pos.y = v }
            }
            Geometry::Circle { radius, .. } => *radius = v,
            Geometry::Ellipse { major, minor, .. } | Geometry::EllipticalArc { major, minor, .. } => match slot {
                0 => major.x = v,
                1 => major.y = v,
                _ => *minor = v,
            },
            _ => {}
        }
    }

    /// Endpoints (start, end) of open curves.
    pub fn endpoints(&self) -> Option<(EntityId, EntityId)> {
        match &self.geometry {
            Geometry::Line { start, end }
            | Geometry::Arc { start, end, .. }
            | Geometry::EllipticalArc { start, end, .. }
            | Geometry::Conic { start, end, .. } => Some((*start, *end)),
            Geometry::BSpline { control, .. } => Some((*control.first()?, *control.last()?)),
            Geometry::FitSpline { points, .. } => Some((*points.first()?, *points.last()?)),
            _ => None,
        }
    }

    /// Center point of circles, arcs and ellipses.
    pub fn center(&self) -> Option<EntityId> {
        match &self.geometry {
            Geometry::Circle { center, .. }
            | Geometry::Arc { center, .. }
            | Geometry::Ellipse { center, .. }
            | Geometry::EllipticalArc { center, .. } => Some(*center),
            _ => None,
        }
    }

    /// Replaces references to point `from` by `to`.
    pub(crate) fn replace_point(&mut self, from: EntityId, to: EntityId) {
        let r = |x: &mut EntityId| {
            if *x == from {
                *x = to
            }
        };
        match &mut self.geometry {
            Geometry::Point { .. } => {}
            Geometry::Line { start, end } => {
                r(start);
                r(end)
            }
            Geometry::Circle { center, .. } | Geometry::Ellipse { center, .. } => r(center),
            Geometry::Arc { center, start, end } | Geometry::EllipticalArc { center, start, end, .. } => {
                r(center);
                r(start);
                r(end)
            }
            Geometry::BSpline { control, .. } => control.iter_mut().for_each(r),
            Geometry::FitSpline { points, handles } => {
                points.iter_mut().for_each(r);
                if let Some((h0, h1)) = handles {
                    r(h0);
                    r(h1);
                }
            }
            Geometry::Conic { start, control, end, .. } => {
                r(start);
                r(control);
                r(end)
            }
        }
    }
}
