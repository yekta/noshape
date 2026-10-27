//! Resolved 2D curve geometry: evaluation, tessellation, closest point and
//! bounding boxes. Entities are resolved to [`Curve2`] for everything that
//! is not constraint solving.

use crate::nurbs::Nurbs;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub min: DVec2,
    pub max: DVec2,
}

impl BBox {
    pub const EMPTY: BBox = BBox { min: DVec2::splat(f64::INFINITY), max: DVec2::splat(f64::NEG_INFINITY) };

    pub fn from_points<I: IntoIterator<Item = DVec2>>(pts: I) -> BBox {
        let mut b = BBox::EMPTY;
        for p in pts {
            b.add(p);
        }
        b
    }
    pub fn add(&mut self, p: DVec2) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn union(&self, o: &BBox) -> BBox {
        BBox { min: self.min.min(o.min), max: self.max.max(o.max) }
    }
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y
    }
    pub fn expanded(&self, d: f64) -> BBox {
        BBox { min: self.min - DVec2::splat(d), max: self.max + DVec2::splat(d) }
    }
    pub fn intersects(&self, o: &BBox) -> bool {
        self.min.x <= o.max.x && o.min.x <= self.max.x && self.min.y <= o.max.y && o.min.y <= self.max.y
    }
    pub fn contains(&self, p: DVec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
    pub fn size(&self) -> DVec2 {
        if self.is_empty() { DVec2::ZERO } else { self.max - self.min }
    }
    pub fn center(&self) -> DVec2 {
        (self.min + self.max) * 0.5
    }
}

/// A concrete planar curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Curve2 {
    /// Segment, parameter `t in [0,1]`.
    Line { a: DVec2, b: DVec2 },
    /// Full circle, parameter = angle in `[0, 2pi]`.
    Circle { center: DVec2, radius: f64 },
    /// CCW arc from `start_angle` to `end_angle` (`end_angle > start_angle`,
    /// sweep at most 2pi). Parameter = angle.
    Arc { center: DVec2, radius: f64, start_angle: f64, end_angle: f64 },
    /// Full ellipse `c + major cos t + minor * perp(major_dir) sin t`.
    Ellipse { center: DVec2, major: DVec2, minor: f64 },
    /// CCW elliptical arc, parameter `t in [start_param, end_param]`.
    EllipticalArc { center: DVec2, major: DVec2, minor: f64, start_param: f64, end_param: f64 },
    /// NURBS curve (B-spline, fit spline, conic).
    Nurbs(Nurbs),
}

/// Normalizes an angle into `[0, 2pi)`.
pub fn norm_angle(a: f64) -> f64 {
    let r = a.rem_euclid(TAU);
    if r >= TAU { 0.0 } else { r }
}

/// CCW sweep from angle `a0` to `a1` in `(0, 2pi]`.
pub fn ccw_sweep(a0: f64, a1: f64) -> f64 {
    let s = norm_angle(a1 - a0);
    if s <= 1e-15 { TAU } else { s }
}

/// Is angle `a` within the CCW range `[a0, a0 + sweep]` (with tolerance `eps` radians)?
pub fn angle_in_range(a: f64, a0: f64, sweep: f64, eps: f64) -> Option<f64> {
    let d = norm_angle(a - a0);
    if d <= sweep + eps {
        Some(a0 + d.min(sweep))
    } else if d >= TAU - eps {
        Some(a0)
    } else {
        None
    }
}

impl Curve2 {
    pub fn arc_from_points(center: DVec2, start: DVec2, end: DVec2) -> Curve2 {
        let r = (start - center).length();
        let a0 = (start - center).to_angle();
        let a1 = a0 + ccw_sweep(a0, (end - center).to_angle());
        Curve2::Arc { center, radius: r, start_angle: a0, end_angle: a1 }
    }

    pub fn domain(&self) -> (f64, f64) {
        match self {
            Curve2::Line { .. } => (0.0, 1.0),
            Curve2::Circle { .. } | Curve2::Ellipse { .. } => (0.0, TAU),
            Curve2::Arc { start_angle, end_angle, .. } => (*start_angle, *end_angle),
            Curve2::EllipticalArc { start_param, end_param, .. } => (*start_param, *end_param),
            Curve2::Nurbs(n) => n.domain(),
        }
    }

    /// True for periodic closed curves (circle, ellipse) and NURBS whose ends meet.
    pub fn is_closed(&self) -> bool {
        match self {
            Curve2::Circle { .. } | Curve2::Ellipse { .. } => true,
            Curve2::Nurbs(n) => {
                let (a, b) = n.domain();
                let size = BBox::from_points(n.ctrl.iter().copied()).size().max_element();
                (n.point(a) - n.point(b)).length() <= 1e-9 * size.max(1e-9)
            }
            _ => false,
        }
    }

    /// True if the parameterization is periodic (circle / ellipse).
    pub fn is_periodic(&self) -> bool {
        matches!(self, Curve2::Circle { .. } | Curve2::Ellipse { .. })
    }

    fn ellipse_frame(major: DVec2, minor: f64) -> (DVec2, DVec2) {
        let a = major.length().max(1e-300);
        let u = major / a;
        (major, u.perp() * minor)
    }

    /// Point and first two derivatives.
    pub fn ders(&self, t: f64) -> [DVec2; 3] {
        match self {
            Curve2::Line { a, b } => [*a + (*b - *a) * t, *b - *a, DVec2::ZERO],
            Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. } => {
                let (s, c) = t.sin_cos();
                [
                    *center + DVec2::new(c, s) * *radius,
                    DVec2::new(-s, c) * *radius,
                    DVec2::new(-c, -s) * *radius,
                ]
            }
            Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. } => {
                let (m, n) = Self::ellipse_frame(*major, *minor);
                let (s, c) = t.sin_cos();
                [*center + m * c + n * s, -m * s + n * c, -m * c - n * s]
            }
            Curve2::Nurbs(nb) => nb.eval_ders(t),
        }
    }

    pub fn point(&self, t: f64) -> DVec2 {
        self.ders(t)[0]
    }
    pub fn tangent(&self, t: f64) -> DVec2 {
        self.ders(t)[1]
    }
    pub fn start_point(&self) -> DVec2 {
        self.point(self.domain().0)
    }
    pub fn end_point(&self) -> DVec2 {
        self.point(self.domain().1)
    }

    /// Signed curvature at `t` (positive = turning left along the parameterization).
    pub fn curvature(&self, t: f64) -> f64 {
        let d = self.ders(t);
        let l = d[1].length();
        if l < 1e-300 { 0.0 } else { d[1].perp_dot(d[2]) / (l * l * l) }
    }

    /// Characteristic size (for relative tolerances).
    pub fn size(&self) -> f64 {
        self.bbox().size().max_element()
    }

    /// Tessellates into `(param, point)` samples with chordal deviation `<= tol`.
    pub fn tessellate_params(&self, tol: f64) -> Vec<(f64, DVec2)> {
        let tol = tol.max(1e-12);
        let (t0, t1) = self.domain();
        match self {
            Curve2::Line { a, b } => vec![(0.0, *a), (1.0, *b)],
            Curve2::Circle { radius, .. } | Curve2::Arc { radius, .. } => {
                let n = arc_segments(*radius, t1 - t0, tol);
                (0..=n).map(|i| {
                    let t = t0 + (t1 - t0) * i as f64 / n as f64;
                    (t, self.point(t))
                }).collect()
            }
            Curve2::Ellipse { major, minor, .. } | Curve2::EllipticalArc { major, minor, .. } => {
                let r = major.length().max(*minor);
                let n = arc_segments(r, t1 - t0, tol).max(8);
                (0..=n).map(|i| {
                    let t = t0 + (t1 - t0) * i as f64 / n as f64;
                    (t, self.point(t))
                }).collect()
            }
            Curve2::Nurbs(nb) => {
                // initial samples at knots (subdivided), then adaptive refinement
                let mut ts: Vec<f64> = Vec::new();
                let mut uniq: Vec<f64> = nb.knots[nb.degree..=nb.ctrl.len()].to_vec();
                uniq.dedup_by(|a, b| (*a - *b).abs() < 1e-15);
                for w in uniq.windows(2) {
                    for k in 0..4 {
                        ts.push(w[0] + (w[1] - w[0]) * k as f64 / 4.0);
                    }
                }
                ts.push(t1);
                let mut out = vec![(ts[0], self.point(ts[0]))];
                for w in ts.windows(2) {
                    let pa = self.point(w[0]);
                    let pb = self.point(w[1]);
                    refine(self, w[0], pa, w[1], pb, tol, 0, &mut out);
                }
                out
            }
        }
    }

    pub fn tessellate(&self, tol: f64) -> Vec<DVec2> {
        self.tessellate_params(tol).into_iter().map(|(_, p)| p).collect()
    }

    /// Closest parameter / point / distance to `p` (restricted to the curve's domain).
    pub fn closest(&self, p: DVec2) -> (f64, DVec2, f64) {
        match self {
            Curve2::Line { a, b } => {
                let d = *b - *a;
                let l2 = d.length_squared();
                let t = if l2 > 0.0 { ((p - *a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
                let q = *a + d * t;
                (t, q, (p - q).length())
            }
            Curve2::Circle { center, .. } => {
                let v = p - *center;
                let a = if v.length_squared() > 0.0 { norm_angle(v.to_angle()) } else { 0.0 };
                let q = self.point(a);
                (a, q, (p - q).length())
            }
            Curve2::Arc { center, start_angle, end_angle, .. } => {
                let v = p - *center;
                let ang = if v.length_squared() > 0.0 { v.to_angle() } else { *start_angle };
                let sweep = end_angle - start_angle;
                let t = match angle_in_range(ang, *start_angle, sweep, 0.0) {
                    Some(t) => t,
                    None => {
                        let ps = self.point(*start_angle);
                        let pe = self.point(*end_angle);
                        if (ps - p).length() <= (pe - p).length() { *start_angle } else { *end_angle }
                    }
                };
                let q = self.point(t);
                (t, q, (p - q).length())
            }
            _ => self.closest_numeric(p),
        }
    }

    fn closest_numeric(&self, p: DVec2) -> (f64, DVec2, f64) {
        let size = self.size().max(1e-9);
        self.closest_from_samples(p, &self.tessellate_params(size * 2e-3))
    }

    /// [`Curve2::closest`] from samples made at 2e-3 of the curve's size
    /// (analytic for lines, circles and arcs).
    pub(crate) fn closest_from_samples(&self, p: DVec2, samples: &[(f64, DVec2)]) -> (f64, DVec2, f64) {
        if matches!(self, Curve2::Line { .. } | Curve2::Circle { .. } | Curve2::Arc { .. }) {
            return self.closest(p);
        }
        let (t0, t1) = self.domain();
        let mut best = (t0, f64::MAX);
        // closest on polyline segments
        for w in samples.windows(2) {
            let (ta, pa) = w[0];
            let (tb, pb) = w[1];
            let d = pb - pa;
            let l2 = d.length_squared();
            let s = if l2 > 0.0 { ((p - pa).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let dist = (pa + d * s - p).length();
            if dist < best.1 {
                best = (ta + (tb - ta) * s, dist);
            }
        }
        let periodic = self.is_periodic();
        let mut t = best.0;
        for _ in 0..30 {
            let d = self.ders(t);
            let r = d[0] - p;
            let f = r.dot(d[1]);
            let df = d[1].length_squared() + r.dot(d[2]);
            if df.abs() < 1e-300 {
                break;
            }
            let mut tn = t - f / df;
            if periodic {
                tn = tn.rem_euclid(t1 - t0) + t0;
            } else {
                tn = tn.clamp(t0, t1);
            }
            let done = (tn - t).abs() < 1e-14 * (t1 - t0).max(1.0);
            t = tn;
            if done {
                break;
            }
        }
        let q = self.point(t);
        let dist = (q - p).length();
        if dist <= best.1 + 1e-12 {
            (t, q, dist)
        } else {
            let q = self.point(best.0);
            (best.0, q, (q - p).length())
        }
    }

    pub fn distance(&self, p: DVec2) -> f64 {
        self.closest(p).2
    }

    pub fn bbox(&self) -> BBox {
        match self {
            Curve2::Line { a, b } => BBox::from_points([*a, *b]),
            Curve2::Circle { center, radius } => BBox { min: *center - DVec2::splat(*radius), max: *center + DVec2::splat(*radius) },
            Curve2::Arc { center, radius, start_angle, end_angle } => {
                let mut b = BBox::from_points([self.point(*start_angle), self.point(*end_angle)]);
                for k in 0..4 {
                    let a = k as f64 * PI / 2.0;
                    if angle_in_range(a, *start_angle, end_angle - start_angle, 0.0).is_some() {
                        b.add(*center + DVec2::new(a.cos(), a.sin()) * *radius);
                    }
                }
                b
            }
            Curve2::Ellipse { center, major, minor } => {
                let (m, n) = Self::ellipse_frame(*major, *minor);
                let h = DVec2::new((m.x * m.x + n.x * n.x).sqrt(), (m.y * m.y + n.y * n.y).sqrt());
                BBox { min: *center - h, max: *center + h }
            }
            Curve2::EllipticalArc { center, major, minor, start_param, end_param } => {
                let (m, n) = Self::ellipse_frame(*major, *minor);
                let mut b = BBox::from_points([self.point(*start_param), self.point(*end_param)]);
                // extremal params: x' = 0 -> tan t = n.x/m.x ; y' = 0 -> tan t = n.y/m.y
                for base in [n.x.atan2(m.x), n.y.atan2(m.y)] {
                    for k in 0..2 {
                        let t = base + k as f64 * PI;
                        if let Some(tt) = angle_in_range(t, *start_param, end_param - start_param, 0.0) {
                            b.add(self.point(tt));
                        }
                    }
                }
                let _ = center;
                b
            }
            Curve2::Nurbs(nb) => {
                let size = BBox::from_points(nb.ctrl.iter().copied()).size().max_element();
                BBox::from_points(self.tessellate(size * 1e-4 + 1e-12))
            }
        }
    }

    /// Approximate arc length.
    pub fn length(&self) -> f64 {
        match self {
            Curve2::Line { a, b } => (*b - *a).length(),
            Curve2::Circle { radius, .. } => TAU * radius,
            Curve2::Arc { radius, start_angle, end_angle, .. } => radius * (end_angle - start_angle),
            _ => {
                let s = self.size().max(1e-12);
                self.tessellate(s * 1e-6).windows(2).map(|w| (w[1] - w[0]).length()).sum()
            }
        }
    }

    /// Sub-curve between parameters `t0 < t1` (for periodic curves `t1` may
    /// exceed the period end).
    pub fn sub_curve(&self, t0: f64, t1: f64) -> Curve2 {
        match self {
            Curve2::Line { .. } => Curve2::Line { a: self.point(t0), b: self.point(t1) },
            Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. } => {
                Curve2::Arc { center: *center, radius: *radius, start_angle: t0, end_angle: t1 }
            }
            Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. } => {
                Curve2::EllipticalArc { center: *center, major: *major, minor: *minor, start_param: t0, end_param: t1 }
            }
            Curve2::Nurbs(n) => Curve2::Nurbs(n.sub_curve(t0, t1)),
        }
    }
}

fn arc_segments(r: f64, sweep: f64, tol: f64) -> usize {
    let r = r.abs().max(1e-300);
    let x = (1.0 - (tol / r).min(1.0)).clamp(-1.0, 1.0);
    let max_step = (2.0 * x.acos()).max(1e-4);
    ((sweep.abs() / max_step).ceil() as usize).clamp(2, 4096)
}

#[allow(clippy::too_many_arguments)]
fn refine(c: &Curve2, ta: f64, pa: DVec2, tb: f64, pb: DVec2, tol: f64, depth: u32, out: &mut Vec<(f64, DVec2)>) {
    let tm = 0.5 * (ta + tb);
    let pm = c.point(tm);
    // deviation of the curve midpoint (and quarter points) from the chord
    let chord = pb - pa;
    let l = chord.length();
    let dev = |q: DVec2| if l > 1e-300 { (q - pa).perp_dot(chord).abs() / l } else { (q - pa).length() };
    let q1 = c.point(0.5 * (ta + tm));
    let q3 = c.point(0.5 * (tm + tb));
    let d = dev(pm).max(dev(q1)).max(dev(q3));
    if d > tol && depth < 14 {
        refine(c, ta, pa, tm, pm, tol, depth + 1, out);
        refine(c, tm, pm, tb, pb, tol, depth + 1, out);
    } else {
        out.push((tb, pb));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_bbox_and_closest() {
        let a = Curve2::arc_from_points(DVec2::ZERO, DVec2::new(1.0, 0.0), DVec2::new(0.0, 1.0));
        let b = a.bbox();
        assert!((b.max - DVec2::new(1.0, 1.0)).length() < 1e-12);
        assert!(b.min.length() < 1e-12);
        let (_, q, d) = a.closest(DVec2::new(2.0, 2.0));
        assert!((q.length() - 1.0).abs() < 1e-12);
        assert!((d - (8f64.sqrt() - 1.0)).abs() < 1e-12);
        let (_, q, _) = a.closest(DVec2::new(-1.0, -0.1));
        assert!((q - DVec2::new(1.0, 0.0)).length() < 1e-12 || (q - DVec2::new(0.0, 1.0)).length() < 1e-12);
    }

    #[test]
    fn ellipse_closest_and_tess() {
        let e = Curve2::Ellipse { center: DVec2::ZERO, major: DVec2::new(2.0, 0.0), minor: 1.0 };
        let (_, q, d) = e.closest(DVec2::new(0.0, 3.0));
        assert!((q - DVec2::new(0.0, 1.0)).length() < 1e-9);
        assert!((d - 2.0).abs() < 1e-9);
        let pts = e.tessellate(1e-3);
        assert!(pts.len() > 20);
        let bb = e.bbox();
        assert!((bb.max - DVec2::new(2.0, 1.0)).length() < 1e-12);
    }
}
