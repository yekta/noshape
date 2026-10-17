//! Constraint and dimension definitions.

use crate::entities::{EntityKind, Geometry};
use crate::geom::{ccw_sweep, Curve2};
use crate::ids::{ConstraintId, EntityId};
use crate::sketch::Sketch;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::f64::consts::{PI, TAU};

/// How a distance to a circle/arc is measured (Onshape offers min/max/center).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DistanceMode {
    #[default]
    Center,
    Minimum,
    Maximum,
}

/// Geometric constraints and dimensions.
///
/// Dimension values are in SI units (meters, radians).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConstraintKind {
    /// Point-point, point-on-curve, line-line (collinear) or circle-circle (same circle).
    Coincident { a: EntityId, b: EntityId },
    /// Centers of circles/arcs/ellipses (or a point) coincide.
    Concentric { a: EntityId, b: EntityId },
    Parallel { a: EntityId, b: EntityId },
    Perpendicular { a: EntityId, b: EntityId },
    /// A line (`b = None`), an ellipse major axis, or two points.
    Horizontal { a: EntityId, b: Option<EntityId> },
    Vertical { a: EntityId, b: Option<EntityId> },
    /// Line-circle/arc, circle-circle, arc-arc, line-ellipse, spline endpoints.
    Tangent { a: EntityId, b: EntityId },
    /// Equal line lengths or equal radii.
    Equal { a: EntityId, b: EntityId },
    /// Point at the midpoint of a line or arc.
    Midpoint { point: EntityId, curve: EntityId },
    /// `a` and `b` (points, lines, circles or arcs) symmetric about line `axis`.
    Symmetric { a: EntityId, b: EntityId, axis: EntityId },
    /// Locks the entity at its current position.
    Fix { entity: EntityId },
    /// Line normal to a curve (through a circle's center, or perpendicular at a spline end).
    Normal { line: EntityId, curve: EntityId },
    Collinear { a: EntityId, b: EntityId },
    /// G2 continuity at a joint (spline/arc/line endpoints). Implies tangency.
    Curvature { a: EntityId, b: EntityId },
    /// A sketch point pinned to external (projected) geometry.
    Pierce { point: EntityId, target: EntityId },

    // ----- dimensions -----
    /// Point-point, point-line, line-line (parallel) or anything-circle distance.
    Distance { a: EntityId, b: EntityId, value: f64, mode: DistanceMode },
    HorizontalDistance { a: EntityId, b: EntityId, value: f64 },
    VerticalDistance { a: EntityId, b: EntityId, value: f64 },
    /// Line length or arc length.
    Length { curve: EntityId, value: f64 },
    Radius { curve: EntityId, value: f64 },
    Diameter { curve: EntityId, value: f64 },
    /// Angle between two lines. The measured quadrant is stored in the
    /// constraint's [`Hints`] so the dimension is stable under edits.
    Angle { a: EntityId, b: EntityId, value: f64 },
    /// Sweep angle of an arc.
    ArcAngle { arc: EntityId, value: f64 },
    /// Major or minor radius of an ellipse.
    EllipseRadius { ellipse: EntityId, major: bool, value: f64 },
}

impl ConstraintKind {
    pub fn is_dimension(&self) -> bool {
        self.value().is_some()
    }

    pub fn value(&self) -> Option<f64> {
        use ConstraintKind::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | Length { value, .. }
            | Radius { value, .. }
            | Diameter { value, .. }
            | Angle { value, .. }
            | ArcAngle { value, .. }
            | EllipseRadius { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub fn set_value(&mut self, v: f64) -> bool {
        use ConstraintKind::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | Length { value, .. }
            | Radius { value, .. }
            | Diameter { value, .. }
            | Angle { value, .. }
            | ArcAngle { value, .. }
            | EllipseRadius { value, .. } => {
                *value = v;
                true
            }
            _ => false,
        }
    }

    /// Entities referenced by the constraint.
    pub fn entities(&self) -> SmallVec<[EntityId; 3]> {
        use ConstraintKind::*;
        let mut v = SmallVec::new();
        match self {
            Coincident { a, b }
            | Concentric { a, b }
            | Parallel { a, b }
            | Perpendicular { a, b }
            | Tangent { a, b }
            | Equal { a, b }
            | Collinear { a, b }
            | Curvature { a, b }
            | Distance { a, b, .. }
            | HorizontalDistance { a, b, .. }
            | VerticalDistance { a, b, .. }
            | Angle { a, b, .. } => {
                v.push(*a);
                v.push(*b);
            }
            Horizontal { a, b } | Vertical { a, b } => {
                v.push(*a);
                if let Some(b) = b {
                    v.push(*b);
                }
            }
            Midpoint { point, curve } => {
                v.push(*point);
                v.push(*curve);
            }
            Symmetric { a, b, axis } => {
                v.push(*a);
                v.push(*b);
                v.push(*axis);
            }
            Fix { entity } => v.push(*entity),
            Normal { line, curve } => {
                v.push(*line);
                v.push(*curve);
            }
            Pierce { point, target } => {
                v.push(*point);
                v.push(*target);
            }
            Length { curve, .. } | Radius { curve, .. } | Diameter { curve, .. } => v.push(*curve),
            ArcAngle { arc, .. } => v.push(*arc),
            EllipseRadius { ellipse, .. } => v.push(*ellipse),
        }
        v
    }

    /// Replaces every reference to `from` by `to`.
    pub(crate) fn replace_entity(&mut self, from: EntityId, to: EntityId) {
        use ConstraintKind::*;
        let r = |x: &mut EntityId| {
            if *x == from {
                *x = to
            }
        };
        match self {
            Coincident { a, b }
            | Concentric { a, b }
            | Parallel { a, b }
            | Perpendicular { a, b }
            | Tangent { a, b }
            | Equal { a, b }
            | Collinear { a, b }
            | Curvature { a, b }
            | Distance { a, b, .. }
            | HorizontalDistance { a, b, .. }
            | VerticalDistance { a, b, .. }
            | Angle { a, b, .. } => {
                r(a);
                r(b)
            }
            Horizontal { a, b } | Vertical { a, b } => {
                r(a);
                if let Some(b) = b {
                    r(b)
                }
            }
            Midpoint { point, curve } => {
                r(point);
                r(curve)
            }
            Symmetric { a, b, axis } => {
                r(a);
                r(b);
                r(axis)
            }
            Fix { entity } => r(entity),
            Normal { line, curve } => {
                r(line);
                r(curve)
            }
            Pierce { point, target } => {
                r(point);
                r(target)
            }
            Length { curve, .. } | Radius { curve, .. } | Diameter { curve, .. } => r(curve),
            ArcAngle { arc, .. } => r(arc),
            EllipseRadius { ellipse, .. } => r(ellipse),
        }
    }

    /// Short human readable name ("Coincident", "Distance", ...).
    pub fn name(&self) -> &'static str {
        use ConstraintKind::*;
        match self {
            Coincident { .. } => "Coincident",
            Concentric { .. } => "Concentric",
            Parallel { .. } => "Parallel",
            Perpendicular { .. } => "Perpendicular",
            Horizontal { .. } => "Horizontal",
            Vertical { .. } => "Vertical",
            Tangent { .. } => "Tangent",
            Equal { .. } => "Equal",
            Midpoint { .. } => "Midpoint",
            Symmetric { .. } => "Symmetric",
            Fix { .. } => "Fix",
            Normal { .. } => "Normal",
            Collinear { .. } => "Collinear",
            Curvature { .. } => "Curvature",
            Pierce { .. } => "Pierce",
            Distance { .. } => "Distance",
            HorizontalDistance { .. } => "Horizontal distance",
            VerticalDistance { .. } => "Vertical distance",
            Length { .. } => "Length",
            Radius { .. } => "Radius",
            Diameter { .. } => "Diameter",
            Angle { .. } => "Angle",
            ArcAngle { .. } => "Arc angle",
            EllipseRadius { .. } => "Ellipse radius",
        }
    }
}

fn one() -> f64 {
    1.0
}

/// Orientation choices captured when a constraint is created so the solver
/// keeps the same configuration (side of a line, internal vs external
/// tangency, angle quadrant, which spline end, ...).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hints {
    #[serde(default = "one")]
    pub sign: f64,
    #[serde(default = "one")]
    pub sign2: f64,
    #[serde(default)]
    pub flip_a: bool,
    #[serde(default)]
    pub flip_b: bool,
    /// Angle: measure from b to a. Symmetric lines: pair a.start with b.end.
    #[serde(default)]
    pub reverse: bool,
    /// Spline/arc joint is at the end (true) or start (false) of `a`.
    #[serde(default)]
    pub end_a: bool,
    #[serde(default)]
    pub end_b: bool,
    /// Internal tangency / internal min distance between circles.
    #[serde(default)]
    pub internal: bool,
    /// Fix constraint anchor values.
    #[serde(default)]
    pub anchor: Vec<f64>,
}

impl Default for Hints {
    fn default() -> Self {
        Hints {
            sign: 1.0,
            sign2: 1.0,
            flip_a: false,
            flip_b: false,
            reverse: false,
            end_a: false,
            end_b: false,
            internal: false,
            anchor: Vec::new(),
        }
    }
}

/// A constraint or dimension stored in a sketch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: ConstraintId,
    pub kind: ConstraintKind,
    /// Driving dimensions are enforced; driven (reference) dimensions are only
    /// measured. Always true for geometric constraints.
    pub driving: bool,
    pub hints: Hints,
    /// Auxiliary solver unknowns owned by the constraint (e.g. the curve
    /// parameter of a point-on-spline), kept for warm starts.
    #[serde(default)]
    pub aux: Vec<f64>,
}

impl Constraint {
    pub fn is_dimension(&self) -> bool {
        self.kind.is_dimension()
    }
    pub fn value(&self) -> Option<f64> {
        self.kind.value()
    }
}

/// Items that a Fix constraint locks, in a canonical order.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FixItem {
    PointCoord(EntityId, usize),
    Own(EntityId, usize),
    /// Keeps `point` on the ray from `center` at the anchored angle.
    Ray { center: EntityId, point: EntityId },
}

pub(crate) fn fix_items(sk: &Sketch, id: EntityId) -> Vec<FixItem> {
    let Some(e) = sk.entity(id) else { return Vec::new() };
    let pt = |p: EntityId, v: &mut Vec<FixItem>| {
        v.push(FixItem::PointCoord(p, 0));
        v.push(FixItem::PointCoord(p, 1));
    };
    let mut v = Vec::new();
    match &e.geometry {
        Geometry::Point { .. } => pt(id, &mut v),
        Geometry::Line { start, end } => {
            pt(*start, &mut v);
            pt(*end, &mut v)
        }
        Geometry::Circle { center, .. } => {
            pt(*center, &mut v);
            v.push(FixItem::Own(id, 0))
        }
        Geometry::Arc { center, start, end } => {
            pt(*center, &mut v);
            pt(*start, &mut v);
            v.push(FixItem::Ray { center: *center, point: *end })
        }
        Geometry::Ellipse { center, .. } => {
            pt(*center, &mut v);
            (0..3).for_each(|k| v.push(FixItem::Own(id, k)))
        }
        Geometry::EllipticalArc { center, start, end, .. } => {
            pt(*center, &mut v);
            (0..3).for_each(|k| v.push(FixItem::Own(id, k)));
            v.push(FixItem::Ray { center: *center, point: *start });
            v.push(FixItem::Ray { center: *center, point: *end })
        }
        Geometry::BSpline { control, .. } => control.iter().for_each(|p| pt(*p, &mut v)),
        Geometry::FitSpline { points } => points.iter().for_each(|p| pt(*p, &mut v)),
        Geometry::Conic { start, control, end, .. } => {
            pt(*start, &mut v);
            pt(*control, &mut v);
            pt(*end, &mut v)
        }
    }
    v
}

/// Rank used to normalize the operand order of distance dimensions.
pub(crate) fn dist_rank(k: EntityKind) -> u8 {
    match k {
        EntityKind::Point => 0,
        EntityKind::Line => 1,
        _ => 2,
    }
}

/// Rank used to normalize the operand order of tangent constraints.
pub(crate) fn tangent_rank(k: EntityKind) -> u8 {
    match k {
        EntityKind::Line => 0,
        EntityKind::Circle | EntityKind::Arc => 1,
        EntityKind::Ellipse | EntityKind::EllipticalArc => 2,
        _ => 3,
    }
}

fn sgn(x: f64) -> f64 {
    if x < 0.0 { -1.0 } else { 1.0 }
}

fn signed_line_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let l = d.length();
    if l < 1e-300 { (p - a).length() } else { d.perp_dot(p - a) / l }
}

impl Sketch {
    fn kind_of(&self, id: EntityId) -> Option<EntityKind> {
        self.entity(id).map(|e| e.kind())
    }

    /// Radius of a circle/arc.
    pub(crate) fn radius_of(&self, id: EntityId) -> Option<f64> {
        match self.curve(id)? {
            Curve2::Circle { radius, .. } | Curve2::Arc { radius, .. } => Some(radius),
            _ => None,
        }
    }

    pub(crate) fn center_pos(&self, id: EntityId) -> Option<DVec2> {
        let e = self.entity(id)?;
        if e.kind() == EntityKind::Point {
            return self.point(id);
        }
        self.point(e.center()?)
    }

    pub(crate) fn line_pts(&self, id: EntityId) -> Option<(DVec2, DVec2)> {
        match self.entity(id)?.geometry {
            Geometry::Line { start, end } => Some((self.point(start)?, self.point(end)?)),
            _ => None,
        }
    }

    /// Which end (false = start, true = end) of `a` is closest to entity `b`.
    pub(crate) fn nearest_end(&self, a: EntityId, b: EntityId) -> bool {
        let Some(ea) = self.entity(a) else { return false };
        let Some((s, e)) = ea.endpoints() else { return false };
        let (ps, pe) = (self.point(s).unwrap_or_default(), self.point(e).unwrap_or_default());
        let eb = self.entity(b);
        // shared point ids win
        if let Some((bs, be)) = eb.and_then(|x| x.endpoints()) {
            if e == bs || e == be {
                return true;
            }
            if s == bs || s == be {
                return false;
            }
        }
        let dist = |p: DVec2| -> f64 {
            match self.curve(b) {
                Some(c) => match eb.and_then(|x| x.endpoints()) {
                    Some((bs, be)) => {
                        let q1 = self.point(bs).unwrap_or_default();
                        let q2 = self.point(be).unwrap_or_default();
                        (p - q1).length().min((p - q2).length()).min(c.distance(p) * 1.0001)
                    }
                    None => c.distance(p),
                },
                None => self.point(b).map_or(0.0, |q| (q - p).length()),
            }
        };
        dist(pe) < dist(ps)
    }

    /// Computes orientation hints and initial auxiliary unknowns for a new constraint.
    pub(crate) fn compute_hints(&self, kind: &ConstraintKind) -> (Hints, Vec<f64>) {
        use ConstraintKind as K;
        let mut h = Hints::default();
        let mut aux = Vec::new();
        match kind {
            K::Coincident { a, b } | K::Pierce { point: a, target: b } => {
                let (ka, kb) = (self.kind_of(*a), self.kind_of(*b));
                let (p, c) = if ka == Some(EntityKind::Point) { (*a, *b) } else { (*b, *a) };
                let kc = if ka == Some(EntityKind::Point) { kb } else { ka };
                if let Some(kc) = kc
                    && kc.is_spline()
                        && let (Some(pp), Some(cv)) = (self.point(p), self.curve(c)) {
                            aux.push(cv.closest(pp).0);
                        }
            }
            K::Tangent { a, b } | K::Curvature { a, b } => {
                let (Some(ka), Some(kb)) = (self.kind_of(*a), self.kind_of(*b)) else { return (h, aux) };
                let (a, b, ka, kb) = if tangent_rank(ka) <= tangent_rank(kb) { (*a, *b, ka, kb) } else { (*b, *a, kb, ka) };
                h.end_a = self.nearest_end(a, b);
                h.end_b = self.nearest_end(b, a);
                if ka == EntityKind::Line && kb.is_circular() {
                    if let (Some((p, q)), Some(c)) = (self.line_pts(a), self.center_pos(b)) {
                        h.sign = sgn(signed_line_dist(c, p, q));
                    }
                } else if ka.is_circular() && kb.is_circular() {
                    if let (Some(c1), Some(c2), Some(r1), Some(r2)) =
                        (self.center_pos(a), self.center_pos(b), self.radius_of(a), self.radius_of(b))
                    {
                        let d = (c2 - c1).length();
                        h.internal = (d - (r1 - r2).abs()).abs() < (d - (r1 + r2)).abs();
                        h.sign2 = sgn(r1 - r2);
                    }
                } else if ka == EntityKind::Line && kb.is_elliptical()
                    && let (Some((p, q)), Some(Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. })) =
                        (self.line_pts(a), self.curve(b))
                    {
                        let d = q - p;
                        let m = major;
                        let n = major.normalize_or_zero().perp() * minor;
                        let t0 = d.perp_dot(n).atan2(d.perp_dot(m));
                        let pick = |t: f64| signed_line_dist(center + m * t.cos() + n * t.sin(), p, q).abs();
                        let t = if pick(t0) <= pick(t0 + PI) { t0 } else { t0 + PI };
                        aux.push(t);
                    }
            }
            K::Normal { line, curve } => {
                h.end_b = self.nearest_end(*curve, *line);
            }
            K::Symmetric { a, b, axis } => {
                if let (Some(EntityKind::Line), Some(EntityKind::Line), Some((p, q))) =
                    (self.kind_of(*a), self.kind_of(*b), self.line_pts(*axis))
                    && let (Some((a0, a1)), Some((b0, _b1))) = (self.line_pts(*a), self.line_pts(*b)) {
                        let m0 = reflect(a0, p, q);
                        let m1 = reflect(a1, p, q);
                        h.reverse = (m1 - b0).length() < (m0 - b0).length();
                    }
            }
            K::Fix { entity } => {
                for item in fix_items(self, *entity) {
                    let v = match item {
                        FixItem::PointCoord(p, k) => self.point(p).map_or(0.0, |v| v[k]),
                        FixItem::Own(e, k) => self.entity(e).map_or(0.0, |e| e.own_params().get(k).copied().unwrap_or(0.0)),
                        FixItem::Ray { center, point } => {
                            let c = self.point(center).unwrap_or_default();
                            let p = self.point(point).unwrap_or_default();
                            (p - c).to_angle()
                        }
                    };
                    h.anchor.push(v);
                }
            }
            K::Distance { a, b, mode, .. } => {
                let (Some(ka), Some(kb)) = (self.kind_of(*a), self.kind_of(*b)) else { return (h, aux) };
                let (a, b, ka, kb) = if dist_rank(ka) <= dist_rank(kb) { (*a, *b, ka, kb) } else { (*b, *a, kb, ka) };
                let ra = dist_rank(ka);
                let rb = dist_rank(kb);
                match (ra, rb) {
                    (0, 1) => {
                        if let (Some(p), Some((l0, l1))) = (self.point(a), self.line_pts(b)) {
                            h.sign = sgn(signed_line_dist(p, l0, l1));
                        }
                    }
                    (1, 1) => {
                        if let (Some((a0, a1)), Some((b0, b1))) = (self.line_pts(a), self.line_pts(b)) {
                            h.sign = sgn(signed_line_dist((b0 + b1) * 0.5, a0, a1));
                        }
                    }
                    (0, 2) => {
                        if let (Some(p), Some(c), Some(r)) = (self.point(a), self.center_pos(b), self.radius_of(b)) {
                            h.sign = sgn((p - c).length() - r);
                        }
                    }
                    (1, 2) => {
                        if let (Some((l0, l1)), Some(c)) = (self.line_pts(a), self.center_pos(b)) {
                            h.sign = sgn(signed_line_dist(c, l0, l1));
                        }
                    }
                    (2, 2) => {
                        if let (Some(c1), Some(c2), Some(r1), Some(r2)) =
                            (self.center_pos(a), self.center_pos(b), self.radius_of(a), self.radius_of(b))
                        {
                            let d = (c2 - c1).length();
                            h.internal = *mode == DistanceMode::Minimum && d < r1 + r2 && d < (r1 - r2).abs();
                            h.sign2 = sgn(r1 - r2);
                        }
                    }
                    _ => {}
                }
            }
            K::HorizontalDistance { a, b, value } | K::VerticalDistance { a, b, value } => {
                let k = if matches!(kind, K::HorizontalDistance { .. }) { 0 } else { 1 };
                if let (Some(pa), Some(pb)) = (self.ref_point_pos(*a), self.ref_point_pos(*b)) {
                    let d = pb[k] - pa[k];
                    h.sign = if d.abs() < 1e-12 { sgn(*value) } else { sgn(d) };
                }
            }
            K::Angle { a, b, value } => {
                if let (Some((a0, a1)), Some((b0, b1))) = (self.line_pts(*a), self.line_pts(*b)) {
                    let (u, v) = (a1 - a0, b1 - b0);
                    let mut best: Option<(f64, bool, bool, bool)> = None;
                    for combo in 0..8u8 {
                        let (fa, fb, rev) = (combo & 1 != 0, combo & 2 != 0, combo & 4 != 0);
                        let uu = if fa { -u } else { u };
                        let vv = if fb { -v } else { v };
                        let (x, y) = if rev { (vv, uu) } else { (uu, vv) };
                        let phi = crate::geom::norm_angle(y.to_angle() - x.to_angle());
                        if phi > PI + 1e-9 {
                            continue;
                        }
                        let err = (phi - value).abs();
                        if best.is_none_or(|b| err < b.0 - 1e-12) {
                            best = Some((err, fa, fb, rev));
                        }
                    }
                    if let Some((_, fa, fb, rev)) = best {
                        h.flip_a = fa;
                        h.flip_b = fb;
                        h.reverse = rev;
                    }
                }
            }
            _ => {}
        }
        (h, aux)
    }

    /// Reference point used by horizontal/vertical distances: the point
    /// itself, a line's start point, or a circle/arc/ellipse center.
    pub(crate) fn ref_point(&self, id: EntityId) -> Option<EntityId> {
        let e = self.entity(id)?;
        match &e.geometry {
            Geometry::Point { .. } => Some(id),
            Geometry::Line { start, .. } => Some(*start),
            _ => e.center().or_else(|| e.endpoints().map(|x| x.0)),
        }
    }

    pub(crate) fn ref_point_pos(&self, id: EntityId) -> Option<DVec2> {
        self.point(self.ref_point(id)?)
    }

    /// Current measured value of a dimension (meters / radians).
    pub fn measure(&self, id: ConstraintId) -> Option<f64> {
        let c = self.constraint(id)?;
        self.measure_kind(&c.kind, &c.hints)
    }

    pub(crate) fn measure_kind(&self, kind: &ConstraintKind, h: &Hints) -> Option<f64> {
        use ConstraintKind as K;
        match kind {
            K::Distance { a, b, mode, .. } => {
                let (ka, kb) = (self.kind_of(*a)?, self.kind_of(*b)?);
                let (a, b, ka, kb) = if dist_rank(ka) <= dist_rank(kb) { (*a, *b, ka, kb) } else { (*b, *a, kb, ka) };
                match (dist_rank(ka), dist_rank(kb)) {
                    (0, 0) => Some((self.point(a)? - self.point(b)?).length()),
                    (0, 1) => {
                        let (l0, l1) = self.line_pts(b)?;
                        Some(signed_line_dist(self.point(a)?, l0, l1).abs())
                    }
                    (1, 1) => {
                        let (a0, a1) = self.line_pts(a)?;
                        let (b0, b1) = self.line_pts(b)?;
                        Some(signed_line_dist((b0 + b1) * 0.5, a0, a1).abs())
                    }
                    (0, 2) | (1, 2) => {
                        let c = self.center_pos(b)?;
                        let r = self.radius_of(b).unwrap_or(0.0);
                        let d = if dist_rank(ka) == 0 {
                            (self.point(a)? - c).length()
                        } else {
                            let (l0, l1) = self.line_pts(a)?;
                            signed_line_dist(c, l0, l1).abs()
                        };
                        Some(match mode {
                            DistanceMode::Center => d,
                            DistanceMode::Minimum => (d - r).abs(),
                            DistanceMode::Maximum => d + r,
                        })
                    }
                    (2, 2) => {
                        let d = (self.center_pos(a)? - self.center_pos(b)?).length();
                        let r1 = self.radius_of(a).unwrap_or(0.0);
                        let r2 = self.radius_of(b).unwrap_or(0.0);
                        Some(match mode {
                            DistanceMode::Center => d,
                            DistanceMode::Minimum => {
                                if h.internal { (r1 - r2).abs() - d } else { d - r1 - r2 }
                            }
                            DistanceMode::Maximum => d + r1 + r2,
                        })
                    }
                    _ => None,
                }
            }
            K::HorizontalDistance { a, b, .. } => Some((self.ref_point_pos(*b)?.x - self.ref_point_pos(*a)?.x).abs()),
            K::VerticalDistance { a, b, .. } => Some((self.ref_point_pos(*b)?.y - self.ref_point_pos(*a)?.y).abs()),
            K::Length { curve, .. } => Some(self.curve(*curve)?.length()),
            K::Radius { curve, .. } => self.radius_of(*curve),
            K::Diameter { curve, .. } => self.radius_of(*curve).map(|r| 2.0 * r),
            K::Angle { a, b, .. } => {
                let (a0, a1) = self.line_pts(*a)?;
                let (b0, b1) = self.line_pts(*b)?;
                let mut u = a1 - a0;
                let mut v = b1 - b0;
                if h.flip_a {
                    u = -u;
                }
                if h.flip_b {
                    v = -v;
                }
                let (x, y) = if h.reverse { (v, u) } else { (u, v) };
                let phi = y.to_angle() - x.to_angle();
                let phi = phi.rem_euclid(TAU);
                Some(if phi > TAU - 1e-12 { 0.0 } else { phi })
            }
            K::ArcAngle { arc, .. } => match self.curve(*arc)? {
                Curve2::Arc { start_angle, end_angle, .. } => Some(end_angle - start_angle),
                _ => None,
            },
            K::EllipseRadius { ellipse, major, .. } => match self.entity(*ellipse)?.geometry {
                Geometry::Ellipse { major: m, minor, .. } | Geometry::EllipticalArc { major: m, minor, .. } => {
                    Some(if *major { m.length() } else { minor })
                }
                _ => None,
            },
            _ => None,
        }
    }
}

/// Mirror of `p` about the line through `a` and `b`.
pub fn reflect(p: DVec2, a: DVec2, b: DVec2) -> DVec2 {
    let d = (b - a).normalize_or_zero();
    let v = p - a;
    a + d * (2.0 * v.dot(d)) - v
}

#[allow(dead_code)]
pub(crate) fn sweep_of(c: DVec2, s: DVec2, e: DVec2) -> f64 {
    ccw_sweep((s - c).to_angle(), (e - c).to_angle())
}
