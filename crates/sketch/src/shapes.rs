//! Compound creation tools (Onshape toolbar): rectangles, polygons, slots,
//! arcs and circles, built from primitive entities plus constraints.

use crate::constraints::ConstraintKind;
use crate::entities::Geometry;
use crate::ids::{ConstraintId, EntityId};
use crate::sketch::{Result, Sketch, SketchError};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// Entities and constraints created by a shape tool.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShapeIds {
    /// Defining points (corners, centers, ...).
    pub points: Vec<EntityId>,
    /// Regular (profile) curves, in loop order.
    pub curves: Vec<EntityId>,
    /// Construction entities (diagonals, construction circles, center lines).
    pub construction: Vec<EntityId>,
    pub constraints: Vec<ConstraintId>,
}

/// Circle through three points: (center, radius).
pub fn circle_through(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<(DVec2, f64)> {
    let a = p2 - p1;
    let b = p3 - p1;
    let d = 2.0 * a.perp_dot(b);
    if d.abs() < 1e-14 * a.length_squared().max(b.length_squared()).max(1e-300) {
        return None;
    }
    let (a2, b2) = (a.length_squared(), b.length_squared());
    let c = DVec2::new(b.y * a2 - a.y * b2, a.x * b2 - b.x * a2) / d;
    Some((p1 + c, c.length()))
}

impl Sketch {
    fn closed_polyline(&mut self, pts: &[EntityId]) -> Vec<EntityId> {
        let n = pts.len();
        (0..n).map(|k| self.add_line_between(pts[k], pts[(k + 1) % n]).unwrap()).collect()
    }

    fn c(&mut self, out: &mut ShapeIds, k: ConstraintKind) {
        if let Ok(id) = self.add_constraint(k) {
            out.constraints.push(id);
        }
    }

    /// Axis-aligned rectangle from two opposite corners (2 horizontal + 2 vertical).
    pub fn corner_rectangle(&mut self, p0: DVec2, p1: DVec2) -> ShapeIds {
        let (lo, hi) = (p0.min(p1), p0.max(p1));
        let pts: Vec<EntityId> = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)].iter().map(|p| self.add_point(*p)).collect();
        let lines = self.closed_polyline(&pts);
        let mut out = ShapeIds { points: pts, curves: lines.clone(), ..Default::default() };
        self.c(&mut out, ConstraintKind::Horizontal { a: lines[0], b: None });
        self.c(&mut out, ConstraintKind::Horizontal { a: lines[2], b: None });
        self.c(&mut out, ConstraintKind::Vertical { a: lines[1], b: None });
        self.c(&mut out, ConstraintKind::Vertical { a: lines[3], b: None });
        out
    }

    /// Axis-aligned rectangle from its center and a corner. A construction
    /// diagonal carries a midpoint constraint to the center point.
    pub fn center_rectangle(&mut self, center: DVec2, corner: DVec2) -> ShapeIds {
        let h = (corner - center).abs();
        let mut out = self.corner_rectangle(center - h, center + h);
        let diag = self.add_line_between(out.points[0], out.points[2]).unwrap();
        self.set_construction(diag, true).unwrap();
        let cp = self.add_point(center);
        self.c(&mut out, ConstraintKind::Midpoint { point: cp, curve: diag });
        out.construction.push(diag);
        out.points.push(cp);
        out
    }

    /// Rectangle whose first edge runs from `p0` to `p1`; `p2` sets the width
    /// (projected onto the edge normal). Free rotation.
    pub fn three_point_rectangle(&mut self, p0: DVec2, p1: DVec2, p2: DVec2) -> Result<ShapeIds> {
        let d = p1 - p0;
        if d.length() < 1e-12 {
            return Err(SketchError::InvalidGeometry("degenerate rectangle edge".into()));
        }
        let n = d.normalize().perp();
        let h = (p2 - p0).dot(n);
        if h.abs() < 1e-12 {
            return Err(SketchError::InvalidGeometry("degenerate rectangle width".into()));
        }
        let corners = [p0, p1, p1 + n * h, p0 + n * h];
        let corners = if h > 0.0 { corners } else { [corners[0], corners[3], corners[2], corners[1]] };
        let pts: Vec<EntityId> = corners.iter().map(|p| self.add_point(*p)).collect();
        let l = self.closed_polyline(&pts);
        let mut out = ShapeIds { points: pts, curves: l.clone(), ..Default::default() };
        self.c(&mut out, ConstraintKind::Perpendicular { a: l[0], b: l[1] });
        self.c(&mut out, ConstraintKind::Parallel { a: l[0], b: l[2] });
        self.c(&mut out, ConstraintKind::Parallel { a: l[1], b: l[3] });
        Ok(out)
    }

    /// Rotated rectangle centered at `center`, with its first axis pointing at
    /// `axis_point`; `corner` gives the half extents in that frame.
    pub fn aligned_rectangle(&mut self, center: DVec2, axis_point: DVec2, corner: DVec2) -> Result<ShapeIds> {
        let u = (axis_point - center).normalize_or_zero();
        if u == DVec2::ZERO {
            return Err(SketchError::InvalidGeometry("degenerate rectangle axis".into()));
        }
        let v = u.perp();
        let rel = corner - center;
        let (a, b) = (rel.dot(u).abs(), rel.dot(v).abs());
        if a < 1e-12 || b < 1e-12 {
            return Err(SketchError::InvalidGeometry("degenerate rectangle".into()));
        }
        let corners = [center - u * a - v * b, center + u * a - v * b, center + u * a + v * b, center - u * a + v * b];
        let pts: Vec<EntityId> = corners.iter().map(|p| self.add_point(*p)).collect();
        let l = self.closed_polyline(&pts);
        let mut out = ShapeIds { points: pts.clone(), curves: l.clone(), ..Default::default() };
        self.c(&mut out, ConstraintKind::Perpendicular { a: l[0], b: l[1] });
        self.c(&mut out, ConstraintKind::Parallel { a: l[0], b: l[2] });
        self.c(&mut out, ConstraintKind::Parallel { a: l[1], b: l[3] });
        let diag = self.add_line_between(pts[0], pts[2]).unwrap();
        self.set_construction(diag, true).unwrap();
        let cp = self.add_point(center);
        self.c(&mut out, ConstraintKind::Midpoint { point: cp, curve: diag });
        out.construction.push(diag);
        out.points.push(cp);
        Ok(out)
    }

    /// Regular polygon with `n` sides. `inscribed`: `vertex` is a polygon
    /// vertex (vertices on the construction circle); otherwise `vertex` is an
    /// edge midpoint (polygon circumscribed about the construction circle).
    pub fn polygon(&mut self, center: DVec2, vertex: DVec2, n: usize, inscribed: bool) -> Result<ShapeIds> {
        if n < 3 {
            return Err(SketchError::InvalidGeometry("polygon needs at least 3 sides".into()));
        }
        let r0 = (vertex - center).length();
        if r0 < 1e-12 {
            return Err(SketchError::InvalidGeometry("degenerate polygon".into()));
        }
        let a0 = (vertex - center).to_angle();
        let (big_r, start) = if inscribed { (r0, a0) } else { (r0 / (PI / n as f64).cos(), a0 - PI / n as f64) };
        let cp = self.add_point(center);
        let pts: Vec<EntityId> = (0..n)
            .map(|k| self.add_point(center + DVec2::from_angle(start + TAU * k as f64 / n as f64) * big_r))
            .collect();
        let l = self.closed_polyline(&pts);
        let mut out = ShapeIds { points: pts.clone(), curves: l.clone(), ..Default::default() };
        out.points.push(cp);
        let outer = self.add_circle_at(cp, big_r)?;
        self.set_construction(outer, true)?;
        out.construction.push(outer);
        for p in &pts {
            self.c(&mut out, ConstraintKind::Coincident { a: *p, b: outer });
        }
        for k in 1..n {
            self.c(&mut out, ConstraintKind::Equal { a: l[0], b: l[k] });
        }
        if !inscribed {
            let inner = self.add_circle_at(cp, r0)?;
            self.set_construction(inner, true)?;
            out.construction.push(inner);
            self.c(&mut out, ConstraintKind::Tangent { a: l[0], b: inner });
        }
        Ok(out)
    }

    /// Straight slot between centers `p0` and `p1` with total `width`.
    pub fn straight_slot(&mut self, p0: DVec2, p1: DVec2, width: f64) -> Result<ShapeIds> {
        let d = p1 - p0;
        if d.length() < 1e-12 || width <= 0.0 {
            return Err(SketchError::InvalidGeometry("degenerate slot".into()));
        }
        let n = d.normalize().perp() * (width * 0.5);
        let c0 = self.add_point(p0);
        let c1 = self.add_point(p1);
        let b0 = self.add_point(p0 - n);
        let b1 = self.add_point(p1 - n);
        let a1 = self.add_point(p1 + n);
        let a0 = self.add_point(p0 + n);
        let bottom = self.add_line_between(b0, b1)?;
        let arc1 = self.add_arc_between(c1, b1, a1)?;
        let top = self.add_line_between(a1, a0)?;
        let arc0 = self.add_arc_between(c0, a0, b0)?;
        let axis = self.add_line_between(c0, c1)?;
        self.set_construction(axis, true)?;
        let mut out = ShapeIds {
            points: vec![c0, c1, b0, b1, a1, a0],
            curves: vec![bottom, arc1, top, arc0],
            construction: vec![axis],
            constraints: Vec::new(),
        };
        self.c(&mut out, ConstraintKind::Tangent { a: bottom, b: arc1 });
        self.c(&mut out, ConstraintKind::Tangent { a: top, b: arc1 });
        self.c(&mut out, ConstraintKind::Tangent { a: top, b: arc0 });
        self.c(&mut out, ConstraintKind::Tangent { a: bottom, b: arc0 });
        self.c(&mut out, ConstraintKind::Equal { a: arc0, b: arc1 });
        Ok(out)
    }

    /// Arc from `start` through `mid` to `end`.
    pub fn three_point_arc(&mut self, start: DVec2, mid: DVec2, end: DVec2) -> Result<EntityId> {
        let (c, _) = circle_through(start, mid, end).ok_or_else(|| SketchError::InvalidGeometry("collinear points".into()))?;
        // CCW from start to end passes through mid?
        let a0 = (start - c).to_angle();
        let sweep_end = crate::geom::ccw_sweep(a0, (end - c).to_angle());
        let sweep_mid = crate::geom::ccw_sweep(a0, (mid - c).to_angle());
        if sweep_mid < sweep_end {
            Ok(self.add_arc(c, start, end))
        } else {
            Ok(self.add_arc(c, end, start))
        }
    }

    /// Arc around `center` starting at `start`, ending in the direction of
    /// `end` (projected onto the radius), counter-clockwise if `ccw`.
    pub fn center_point_arc(&mut self, center: DVec2, start: DVec2, end: DVec2, ccw: bool) -> EntityId {
        if ccw { self.add_arc(center, start, end) } else {
            let r = (start - center).length();
            let e = center + (end - center).normalize_or(DVec2::X) * r;
            self.add_arc(center, e, start)
        }
    }

    /// Circle through three points.
    pub fn three_point_circle(&mut self, p1: DVec2, p2: DVec2, p3: DVec2) -> Result<EntityId> {
        let (c, r) = circle_through(p1, p2, p3).ok_or_else(|| SketchError::InvalidGeometry("collinear points".into()))?;
        Ok(self.add_circle(c, r))
    }

    /// Arc tangent to the curve ending at point `from`, from that point to
    /// `end`. Adds the tangent constraint and shares the start point.
    pub fn tangent_arc(&mut self, from: EntityId, end: DVec2) -> Result<ShapeIds> {
        let p = self.point(from).ok_or(SketchError::UnknownEntity(from))?;
        let owner = self
            .entities()
            .find(|e| e.endpoints().is_some_and(|(s, en)| s == from || en == from))
            .map(|e| e.id)
            .ok_or_else(|| SketchError::InvalidGeometry("point is not a curve endpoint".into()))?;
        let curve = self.curve(owner).unwrap();
        let (s, _) = self.endpoints(owner).unwrap();
        let (a, b) = curve.domain();
        // outgoing direction continuing past the endpoint
        let t = if s == from { -curve.tangent(a) } else { curve.tangent(b) };
        let t = t.normalize_or_zero();
        let n = t.perp();
        let w = end - p;
        let nd = n.dot(w);
        if t == DVec2::ZERO || nd.abs() < 1e-12 * w.length().max(1e-300) {
            return Err(SketchError::InvalidGeometry("tangent arc would be straight".into()));
        }
        let sdist = w.length_squared() / (2.0 * nd);
        let c = p + n * sdist;
        let cp = self.add_point(c);
        let ep = self.add_point(end);
        let arc = if sdist > 0.0 { self.add_arc_between(cp, from, ep)? } else { self.add_arc_between(cp, ep, from)? };
        let mut out = ShapeIds { points: vec![cp, ep], curves: vec![arc], ..Default::default() };
        self.c(&mut out, ConstraintKind::Tangent { a: owner, b: arc });
        Ok(out)
    }

    /// Circle by center and radius (convenience alias used by the UI).
    pub fn center_circle(&mut self, center: DVec2, radius: f64) -> EntityId {
        self.add_circle(center, radius)
    }

    /// Ellipse through center, major axis end point and a point giving the minor radius.
    pub fn ellipse_3pt(&mut self, center: DVec2, major_end: DVec2, minor_point: DVec2) -> Result<EntityId> {
        let m = major_end - center;
        if m.length() < 1e-12 {
            return Err(SketchError::InvalidGeometry("degenerate ellipse".into()));
        }
        let b = m.normalize().perp().dot(minor_point - center).abs();
        if b < 1e-12 {
            return Err(SketchError::InvalidGeometry("degenerate ellipse".into()));
        }
        Ok(self.add_ellipse(center, m, b))
    }

    #[allow(dead_code)]
    pub(crate) fn arc_points(&self, id: EntityId) -> Option<(EntityId, EntityId, EntityId)> {
        match self.entity(id)?.geometry {
            Geometry::Arc { center, start, end } => Some((center, start, end)),
            _ => None,
        }
    }
}
