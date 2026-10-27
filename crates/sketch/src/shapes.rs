//! Compound creation tools (Onshape toolbar): rectangles, polygons, slots,
//! arcs and circles, built from primitive entities plus constraints.

use crate::constraints::ConstraintKind;
use crate::ids::{ConstraintId, EntityId};
use crate::sketch::{Result, Sketch, SketchError};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

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

    /// Axis-aligned rectangle from two opposite corners. Live Onshape (REST,
    /// qa5 sketch tools): Perpendicular top / left, Parallel bottom / top and
    /// left / right, Horizontal top.
    pub fn corner_rectangle(&mut self, p0: DVec2, p1: DVec2) -> ShapeIds {
        let (lo, hi) = (p0.min(p1), p0.max(p1));
        let pts: Vec<EntityId> = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)].iter().map(|p| self.add_point(*p)).collect();
        let lines = self.closed_polyline(&pts);
        let mut out = ShapeIds { points: pts, curves: lines.clone(), ..Default::default() };
        let [bottom, right, top, left] = [lines[0], lines[1], lines[2], lines[3]];
        self.c(&mut out, ConstraintKind::Perpendicular { a: top, b: left });
        self.c(&mut out, ConstraintKind::Parallel { a: bottom, b: top });
        self.c(&mut out, ConstraintKind::Parallel { a: left, b: right });
        self.c(&mut out, ConstraintKind::Horizontal { a: top, b: None });
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

    /// Regular polygon with `n` sides, as live Onshape builds it (behaviours
    /// "skasm (2026-10-26) Sketch polygon commits"): a construction circle
    /// about the centre, one seed edge at the click and a circular pattern
    /// of it about the circle's centre (each copy's ends linked to the
    /// seed's; the first copy's start Coincident with the seed's end, the
    /// pattern holds the other joints). `on_vertex` (Circumscribed polygon):
    /// `click` is a vertex, the seed ends there and lies on the circle.
    /// Otherwise (Inscribed polygon) `click` is the middle of the seed, a
    /// construction point there is its Midpoint and the circle is Tangent to
    /// it. `points[0]` is the point at the click, the last the centre.
    pub fn polygon(&mut self, center: DVec2, click: DVec2, n: usize, on_vertex: bool) -> Result<ShapeIds> {
        if n < 3 {
            return Err(SketchError::InvalidGeometry("polygon needs at least 3 sides".into()));
        }
        let r = (click - center).length();
        if r < 1e-12 {
            return Err(SketchError::InvalidGeometry("degenerate polygon".into()));
        }
        let a0 = (click - center).to_angle();
        let half = PI / n as f64;
        // The seed runs counter-clockwise: ending at the clicked vertex, or through the click.
        let (s0, s1) = if on_vertex {
            (center + DVec2::from_angle(a0 - 2.0 * half) * r, click)
        } else {
            let big_r = r / half.cos();
            (center + DVec2::from_angle(a0 - half) * big_r, center + DVec2::from_angle(a0 + half) * big_r)
        };
        let cp = self.add_point(center);
        let circle = self.add_circle_at(cp, r)?;
        self.set_construction(circle, true)?;
        let (p0, p1) = (self.add_point(s0), self.add_point(s1));
        let seed = self.add_line_between(p0, p1)?;
        let mut out = ShapeIds { curves: vec![seed], construction: vec![circle], ..Default::default() };
        let mut corners = vec![p0, p1];
        for k in 1..n {
            let angle = 2.0 * half * k as f64;
            let turn = |p: DVec2| center + DVec2::from_angle(angle).rotate(p - center);
            let (q0, q1) = (self.add_point(turn(s0)), self.add_point(turn(s1)));
            out.curves.push(self.add_line_between(q0, q1)?);
            self.c(&mut out, ConstraintKind::CircularPatternLink { seed: p0, inst: q0, center: cp, angle });
            self.c(&mut out, ConstraintKind::CircularPatternLink { seed: p1, inst: q1, center: cp, angle });
            if k == 1 {
                self.c(&mut out, ConstraintKind::Coincident { a: q0, b: p1 });
            }
            corners.extend([q0, q1]);
        }
        let at_click = if on_vertex {
            self.c(&mut out, ConstraintKind::Coincident { a: p1, b: circle });
            p1
        } else {
            let m = self.add_point(click);
            self.set_construction(m, true)?;
            out.construction.push(m);
            self.c(&mut out, ConstraintKind::Tangent { a: seed, b: circle });
            self.c(&mut out, ConstraintKind::Midpoint { point: m, curve: seed });
            m
        };
        out.points = std::iter::once(at_click).chain(corners.into_iter().filter(|p| *p != at_click)).chain(std::iter::once(cp)).collect();
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

    /// Slot of total `width` around a line or arc centerline (Onshape's Slot
    /// tool): two side curves joined by semicircular end caps centered on
    /// the centerline's end points. The centerline stays a normal (solid)
    /// curve, as in live Onshape; the end caps are equal so one dimension
    /// sets the width.
    pub fn slot_around(&mut self, id: EntityId, width: f64) -> Result<ShapeIds> {
        if width <= 0.0 {
            return Err(SketchError::InvalidGeometry("slot width must be positive".into()));
        }
        let h = width * 0.5;
        let e = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.clone();
        let mut out = ShapeIds::default();
        match e.geometry {
            crate::entities::Geometry::Line { start, end } => {
                // Live (REST, qa5 sketch tools): end caps on their own centre
                // points, each joint a COINCIDENT, 4 TANGENT, 3 OFFSET from the
                // centerline (both sides and the start cap, by the end cap's
                // radius); the width dimension goes on the end cap.
                let (p0, p1) = (self.point(start).unwrap(), self.point(end).unwrap());
                let d = p1 - p0;
                if d.length() < 1e-12 {
                    return Err(SketchError::InvalidGeometry("degenerate slot centerline".into()));
                }
                let n = d.normalize().perp() * h;
                let c0 = self.add_point(p0);
                let c1 = self.add_point(p1);
                let (sa, sb) = (self.add_point(p0 + n), self.add_point(p0 - n));
                let (eb, ea) = (self.add_point(p1 - n), self.add_point(p1 + n));
                let (la, lb) = (self.add_point(p0 + n), self.add_point(p1 + n));
                let (ra, rb) = (self.add_point(p0 - n), self.add_point(p1 - n));
                // Caps run CCW: the start cap from the left side round the back.
                let start_cap = self.add_arc_between(c0, sa, sb)?;
                let end_cap = self.add_arc_between(c1, eb, ea)?;
                let left = self.add_line_between(la, lb)?;
                let right = self.add_line_between(ra, rb)?;
                out.points = vec![c0, c1, sa, sb, eb, ea, la, lb, ra, rb];
                out.curves = vec![right, end_cap, left, start_cap];
                for (a, b) in [(c0, start), (c1, end), (sa, la), (sb, ra), (eb, rb), (ea, lb)] {
                    self.c(&mut out, ConstraintKind::Coincident { a, b });
                }
                for (l, c) in [(left, start_cap), (right, start_cap), (left, end_cap), (right, end_cap)] {
                    self.c(&mut out, ConstraintKind::Tangent { a: c, b: l });
                }
                for (curve, master_ref, master, side) in [(left, end, id, 1.0), (right, end, id, -1.0), (start_cap, end, start, 1.0)] {
                    self.c(&mut out, ConstraintKind::Offset { reference: end_cap, curve, master_ref, master, side });
                }
            }
            crate::entities::Geometry::Arc { center, start, end } => {
                let (c, s, en) = (self.point(center).unwrap(), self.point(start).unwrap(), self.point(end).unwrap());
                let r = (s - c).length();
                if r - h <= 1e-12 {
                    return Err(SketchError::InvalidGeometry("slot is wider than the arc".into()));
                }
                let us = (s - c).normalize();
                let ue = (en - c).normalize();
                let os = self.add_point(c + us * (r + h));
                let is = self.add_point(c + us * (r - h));
                let oe = self.add_point(c + ue * (r + h));
                let ie = self.add_point(c + ue * (r - h));
                let outer = self.add_arc_between(center, os, oe)?;
                let inner = self.add_arc_between(center, is, ie)?;
                // caps run CCW around their centers, bulging away from the arc
                let cap_s = self.add_arc_between(start, is, os)?;
                let cap_e = self.add_arc_between(end, oe, ie)?;
                out.points = vec![os, is, oe, ie];
                out.curves = vec![outer, cap_e, inner, cap_s];
                for (a, b) in [(outer, cap_s), (inner, cap_s), (outer, cap_e), (inner, cap_e)] {
                    self.c(&mut out, ConstraintKind::Tangent { a, b });
                }
                self.c(&mut out, ConstraintKind::Equal { a: cap_s, b: cap_e });
            }
            _ => return Err(SketchError::InvalidGeometry("slots follow lines and arcs".into())),
        }
        Ok(out)
    }

    /// Adds a defining point to a spline at `at` (Spline control point tool):
    /// a fit point for fit splines, a control point on the control polygon
    /// for Bezier curves (raising their degree, at most 15).
    pub fn insert_spline_point(&mut self, id: EntityId, at: DVec2) -> Result<EntityId> {
        let e = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.clone();
        match e.geometry {
            crate::entities::Geometry::FitSpline { mut points, handles } => {
                let curve = self.curve(id).ok_or(SketchError::UnknownEntity(id))?;
                let (t, q, _) = curve.closest(at);
                // fit points are interpolated in order: insert before the first one past t
                let params: Vec<f64> = points.iter().map(|p| curve.closest(self.point(*p).unwrap()).0).collect();
                let k = params.iter().position(|pt| *pt > t).unwrap_or(points.len() - 1).max(1);
                let np = self.add_point(q);
                points.insert(k, np);
                self.replace_geometry(id, crate::entities::Geometry::FitSpline { points, handles });
                Ok(np)
            }
            crate::entities::Geometry::BSpline { mut control, weights: None, .. } => {
                if control.len() >= 16 {
                    return Err(SketchError::InvalidGeometry("The maximum bezier degree (15) has been reached.".into()));
                }
                let pts: Vec<DVec2> = control.iter().map(|p| self.point(*p).unwrap()).collect();
                let mut best = (f64::INFINITY, 1, at);
                for k in 0..pts.len() - 1 {
                    let (a, b) = (pts[k], pts[k + 1]);
                    let d = b - a;
                    let t = if d.length_squared() < 1e-30 { 0.0 } else { ((at - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) };
                    let q = a + d * t;
                    if (q - at).length() < best.0 {
                        best = ((q - at).length(), k + 1, q);
                    }
                }
                let np = self.add_point(best.2);
                control.insert(best.1, np);
                let degree = control.len() - 1;
                let knots = crate::nurbs::clamped_uniform_knots(control.len(), degree);
                self.replace_geometry(id, crate::entities::Geometry::BSpline { control, degree, knots, weights: None });
                Ok(np)
            }
            _ => Err(SketchError::InvalidGeometry("click a spline or a bezier curve".into())),
        }
    }

    /// Degree of a Bezier curve (a single-segment B-spline), if it is one.
    pub fn bezier_degree(&self, id: EntityId) -> Option<usize> {
        match &self.entity(id)?.geometry {
            crate::entities::Geometry::BSpline { control, degree, weights: None, .. } if *degree + 1 == control.len() => Some(*degree),
            _ => None,
        }
    }

    /// Changes a Bezier curve's degree (1..=15). Raising the degree keeps
    /// the shape exactly (degree elevation); lowering it approximates the
    /// curve by least squares with the end points kept.
    pub fn set_bezier_degree(&mut self, id: EntityId, degree: usize) -> Result<()> {
        let Some(cur) = self.bezier_degree(id) else { return Err(SketchError::InvalidGeometry("not a bezier curve".into())) };
        if !(1..=15).contains(&degree) {
            return Err(SketchError::InvalidGeometry("The maximum bezier degree (15) has been reached.".into()));
        }
        if degree == cur {
            return Ok(());
        }
        let crate::entities::Geometry::BSpline { control, .. } = self.entity(id).unwrap().geometry.clone() else { unreachable!() };
        let pts: Vec<DVec2> = control.iter().map(|p| self.point(*p).unwrap()).collect();
        let bez = |c: &[DVec2], t: f64| -> DVec2 {
            let mut v = c.to_vec();
            while v.len() > 1 {
                for i in 0..v.len() - 1 {
                    v[i] = v[i].lerp(v[i + 1], t);
                }
                v.pop();
            }
            v[0]
        };
        let new_pts: Vec<DVec2> = if degree > cur {
            let mut c = pts.clone();
            while c.len() < degree + 1 {
                let n = c.len() - 1;
                let mut e = Vec::with_capacity(c.len() + 1);
                e.push(c[0]);
                for i in 1..=n {
                    let a = i as f64 / (n + 1) as f64;
                    e.push(c[i - 1] * a + c[i] * (1.0 - a));
                }
                e.push(c[n]);
                c = e;
            }
            c
        } else {
            // Least squares with fixed ends on Bernstein polynomials.
            let m = 64;
            let samples: Vec<(f64, DVec2)> = (0..=m).map(|k| {
                let t = k as f64 / m as f64;
                (t, bez(&pts, t))
            }).collect();
            let d = degree;
            let binom = |n: usize, k: usize| -> f64 { (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64) };
            let b = |i: usize, t: f64| binom(d, i) * t.powi(i as i32) * (1.0 - t).powi((d - i) as i32);
            let (p0, pn) = (pts[0], pts[pts.len() - 1]);
            let inner = d.saturating_sub(1);
            let mut c = vec![p0];
            if inner > 0 {
                let mut a = nalgebra::DMatrix::<f64>::zeros(samples.len(), inner);
                let mut rx = nalgebra::DVector::<f64>::zeros(samples.len());
                let mut ry = nalgebra::DVector::<f64>::zeros(samples.len());
                for (r, (t, q)) in samples.iter().enumerate() {
                    for j in 0..inner {
                        a[(r, j)] = b(j + 1, *t);
                    }
                    let rest = *q - p0 * b(0, *t) - pn * b(d, *t);
                    rx[r] = rest.x;
                    ry[r] = rest.y;
                }
                let at = a.transpose();
                let ata = &at * &a;
                let lu = ata.lu();
                let x = lu.solve(&(&at * rx)).ok_or_else(|| SketchError::OperationFailed("degree reduction failed".into()))?;
                let y = lu.solve(&(&at * ry)).ok_or_else(|| SketchError::OperationFailed("degree reduction failed".into()))?;
                c.extend((0..inner).map(|j| DVec2::new(x[j], y[j])));
            }
            c.push(pn);
            c
        };
        // Keep the end points (and their constraints); rebuild the interior.
        let (first, last) = (control[0], control[control.len() - 1]);
        let mut ids = vec![first];
        for p in &new_pts[1..new_pts.len() - 1] {
            ids.push(self.add_point(*p));
        }
        ids.push(last);
        let old_inner: Vec<EntityId> = control[1..control.len() - 1].to_vec();
        let knots = crate::nurbs::clamped_uniform_knots(ids.len(), degree);
        self.replace_geometry(id, crate::entities::Geometry::BSpline { control: ids, degree, knots, weights: None });
        let orphans: Vec<EntityId> = old_inner.into_iter().filter(|p| self.users_of_point(*p).is_empty()).collect();
        self.remove_entities(&orphans);
        Ok(())
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

}
