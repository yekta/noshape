//! Curve-curve intersections (analytic for lines/circles/arcs, subdivision +
//! Newton refinement for ellipses and NURBS), robust to tangency and to
//! endpoints lying on the other curve within tolerance.

use crate::geom::{angle_in_range, norm_angle, BBox, Curve2};
use glam::DVec2;
use std::cell::OnceCell;

/// A curve with its box, size and samples worked out once: a sketch's
/// regions intersect every pair of curves, and a spline's box, size and
/// samples are each an adaptive tessellation (a 94-point fit spline's were
/// made again for every one of its partners: seconds a regeneration).
pub(crate) struct Prepared<'a> {
    pub curve: &'a Curve2,
    pub bbox: BBox,
    size: f64,
    samples: OnceCell<Vec<(f64, DVec2)>>,
}

impl<'a> Prepared<'a> {
    pub fn new(curve: &'a Curve2) -> Self {
        Self::with_bbox(curve, curve.bbox())
    }

    /// With the curve's box already known.
    pub fn with_bbox(curve: &'a Curve2, bbox: BBox) -> Self {
        Prepared { curve, bbox, size: bbox.size().max_element(), samples: OnceCell::new() }
    }

    /// Samples at 2e-3 of the curve's size.
    fn samples(&self) -> &[(f64, DVec2)] {
        self.samples.get_or_init(|| self.curve.tessellate_params(self.size.max(1e-9) * 2e-3))
    }

    fn closest(&self, p: DVec2) -> (f64, DVec2, f64) {
        match self.curve {
            Curve2::Line { .. } | Curve2::Circle { .. } | Curve2::Arc { .. } => self.curve.closest(p),
            c => c.closest_from_samples(p, self.samples()),
        }
    }
}

/// An intersection point with the parameters on both curves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intersection {
    pub t1: f64,
    pub t2: f64,
    pub point: DVec2,
}

/// Parameter of point `p` (known to lie on a circle/arc) on curve `c`, or
/// `None` if outside an arc's range.
fn circ_param(c: &Curve2, p: DVec2, tol: f64) -> Option<f64> {
    match c {
        Curve2::Circle { center, .. } => Some(norm_angle((p - *center).to_angle())),
        Curve2::Arc { center, radius, start_angle, end_angle } => {
            let eps = tol / radius.max(1e-300);
            angle_in_range((p - *center).to_angle(), *start_angle, end_angle - start_angle, eps)
        }
        _ => None,
    }
}

fn line_line(a1: DVec2, b1: DVec2, a2: DVec2, b2: DVec2, tol: f64, out: &mut Vec<Intersection>) {
    let d1 = b1 - a1;
    let d2 = b2 - a2;
    let (l1, l2) = (d1.length(), d2.length());
    if l1 < 1e-300 || l2 < 1e-300 {
        return;
    }
    let den = d1.perp_dot(d2);
    if den.abs() <= 1e-12 * l1 * l2 {
        // parallel: overlapping collinear segments meet at overlap endpoints
        let dist = d1.perp_dot(a2 - a1).abs() / l1;
        if dist > tol {
            return;
        }
        let proj1 = |p: DVec2| (p - a1).dot(d1) / (l1 * l1);
        let proj2 = |p: DVec2| (p - a2).dot(d2) / (l2 * l2);
        let (e1, e2) = (tol / l1, tol / l2);
        for (t, p) in [(0.0, a1), (1.0, b1)] {
            let u = proj2(p);
            if (-e2..=1.0 + e2).contains(&u) {
                out.push(Intersection { t1: t, t2: u.clamp(0.0, 1.0), point: p });
            }
        }
        for (u, p) in [(0.0, a2), (1.0, b2)] {
            let t = proj1(p);
            if (-e1..=1.0 + e1).contains(&t) {
                out.push(Intersection { t1: t.clamp(0.0, 1.0), t2: u, point: p });
            }
        }
        return;
    }
    let w = a2 - a1;
    let t = w.perp_dot(d2) / den;
    let u = w.perp_dot(d1) / den;
    let (e1, e2) = (tol / l1, tol / l2);
    if (-e1..=1.0 + e1).contains(&t) && (-e2..=1.0 + e2).contains(&u) {
        let t = t.clamp(0.0, 1.0);
        out.push(Intersection { t1: t, t2: u.clamp(0.0, 1.0), point: a1 + d1 * t });
    }
}

fn line_circ(a: DVec2, b: DVec2, c: &Curve2, tol: f64, out: &mut Vec<Intersection>) {
    let (center, r) = match c {
        Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. } => (*center, *radius),
        _ => return,
    };
    let d = b - a;
    let l = d.length();
    if l < 1e-300 {
        return;
    }
    let h = d.perp_dot(center - a) / l; // signed distance of center to line
    let foot_t = (center - a).dot(d) / (l * l);
    let e = tol / l;
    let mut ts: Vec<f64> = Vec::new();
    if (h.abs() - r).abs() <= tol {
        ts.push(foot_t);
    } else if h.abs() < r {
        let half = (r * r - h * h).sqrt() / l;
        ts.push(foot_t - half);
        ts.push(foot_t + half);
    }
    for t in ts {
        if !(-e..=1.0 + e).contains(&t) {
            continue;
        }
        let t = t.clamp(0.0, 1.0);
        let p = a + d * t;
        // project onto circle for parameter
        let q = center + (p - center).normalize_or(DVec2::X) * r;
        if let Some(u) = circ_param(c, q, tol) {
            out.push(Intersection { t1: t, t2: u, point: p });
        }
    }
}

fn circ_circ(c1: &Curve2, c2: &Curve2, tol: f64, out: &mut Vec<Intersection>) {
    let (Curve2::Circle { center: o1, radius: r1 } | Curve2::Arc { center: o1, radius: r1, .. }) = c1 else { return };
    let (Curve2::Circle { center: o2, radius: r2 } | Curve2::Arc { center: o2, radius: r2, .. }) = c2 else { return };
    let (o1, r1, o2, r2) = (*o1, *r1, *o2, *r2);
    let dv = o2 - o1;
    let d = dv.length();
    if d <= tol && (r1 - r2).abs() <= tol {
        // same circle: overlap endpoints
        for (curve, other, first) in [(c1, c2, true), (c2, c1, false)] {
            if let Curve2::Arc { start_angle, end_angle, .. } = curve {
                for t in [*start_angle, *end_angle] {
                    let p = curve.point(t);
                    if let Some(u) = circ_param(other, p, tol) {
                        out.push(if first { Intersection { t1: t, t2: u, point: p } } else { Intersection { t1: u, t2: t, point: p } });
                    }
                }
            }
        }
        return;
    }
    if d <= tol || d > r1 + r2 + tol || d < (r1 - r2).abs() - tol {
        return;
    }
    let e = dv / d;
    let a = ((d * d + r1 * r1 - r2 * r2) / (2.0 * d)).clamp(-r1, r1);
    let h2 = r1 * r1 - a * a;
    let tangent = (d - (r1 + r2)).abs() <= tol || (d - (r1 - r2).abs()).abs() <= tol || h2 <= 0.0;
    let mut pts = Vec::new();
    if tangent {
        pts.push(o1 + e * a);
    } else {
        let h = h2.sqrt();
        pts.push(o1 + e * a + e.perp() * h);
        pts.push(o1 + e * a - e.perp() * h);
    }
    for p in pts {
        let q1 = o1 + (p - o1).normalize_or(DVec2::X) * r1;
        let q2 = o2 + (p - o2).normalize_or(DVec2::X) * r2;
        if let (Some(t1), Some(t2)) = (circ_param(c1, q1, tol), circ_param(c2, q2, tol)) {
            out.push(Intersection { t1, t2, point: p });
        }
    }
}

fn wrap(c: &Curve2, t: f64) -> f64 {
    let (a, b) = c.domain();
    if c.is_periodic() { a + (t - a).rem_euclid(b - a) } else { t.clamp(a, b) }
}

/// Samples of both curves (each at 2e-3 of its own size), crossing
/// segments refined by Newton.
fn generic(g1: &Prepared, g2: &Prepared, tol: f64, out: &mut Vec<Intersection>) {
    let (c1, c2) = (g1.curve, g2.curve);
    let s = g1.size.max(g2.size).max(1e-9);
    let (p1, p2) = (g1.samples(), g2.samples());
    let bb2: Vec<(DVec2, DVec2)> = p2.windows(2).map(|w| (w[0].1.min(w[1].1), w[0].1.max(w[1].1))).collect();
    let slack = s * 1e-3;
    for w1 in p1.windows(2) {
        let (ta, pa) = w1[0];
        let (tb, pb) = w1[1];
        let mn = pa.min(pb) - DVec2::splat(slack);
        let mx = pa.max(pb) + DVec2::splat(slack);
        for (k, w2) in p2.windows(2).enumerate() {
            let (bmin, bmax) = bb2[k];
            if bmin.x > mx.x || bmax.x < mn.x || bmin.y > mx.y || bmax.y < mn.y {
                continue;
            }
            let (ua, qa) = w2[0];
            let (ub, qb) = w2[1];
            let d1 = pb - pa;
            let d2 = qb - qa;
            let den = d1.perp_dot(d2);
            let (mut s1, mut s2) = if den.abs() > 1e-300 {
                let w = qa - pa;
                (w.perp_dot(d2) / den, w.perp_dot(d1) / den)
            } else {
                (0.5, 0.5)
            };
            if !(-0.25..=1.25).contains(&s1) || !(-0.25..=1.25).contains(&s2) {
                // allow near misses (tangency) to be tried by Newton below
                let m1 = (pa + pb) * 0.5;
                let m2 = (qa + qb) * 0.5;
                if (m1 - m2).length() > 2.0 * slack {
                    continue;
                }
                s1 = 0.5;
                s2 = 0.5;
            }
            let mut t = ta + (tb - ta) * s1.clamp(0.0, 1.0);
            let mut u = ua + (ub - ua) * s2.clamp(0.0, 1.0);
            // Newton on C1(t) - C2(u) = 0
            let mut ok = false;
            for _ in 0..30 {
                let d1 = c1.ders(t);
                let d2 = c2.ders(u);
                let f = d1[0] - d2[0];
                if f.length() <= tol * 0.01 {
                    ok = true;
                    break;
                }
                let j = [d1[1], -d2[1]];
                let det = j[0].perp_dot(j[1]);
                if det.abs() < 1e-300 {
                    break;
                }
                // solve [j0 j1] [dt du]^T = -f
                let dt = -f.perp_dot(j[1]) / det;
                let du = -j[0].perp_dot(f) / det;
                t = wrap(c1, t + dt);
                u = wrap(c2, u + du);
            }
            let f = c1.point(t) - c2.point(u);
            if ok || f.length() <= tol {
                out.push(Intersection { t1: t, t2: u, point: c1.point(t) });
            }
        }
    }
}

/// Intersections of two curves (including touching endpoints within `tol`).
pub fn intersect(c1: &Curve2, c2: &Curve2, tol: f64) -> Vec<Intersection> {
    intersect_prepared(&Prepared::new(c1), &Prepared::new(c2), tol)
}

/// [`intersect`] of prepared curves.
pub(crate) fn intersect_prepared(g1: &Prepared, g2: &Prepared, tol: f64) -> Vec<Intersection> {
    let (c1, c2) = (g1.curve, g2.curve);
    let mut out = Vec::new();
    if !g1.bbox.expanded(tol).intersects(&g2.bbox.expanded(tol)) {
        return out;
    }
    let circ = |c: &Curve2| matches!(c, Curve2::Circle { .. } | Curve2::Arc { .. });
    match (c1, c2) {
        (Curve2::Line { a: a1, b: b1 }, Curve2::Line { a: a2, b: b2 }) => line_line(*a1, *b1, *a2, *b2, tol, &mut out),
        (Curve2::Line { a, b }, c) if circ(c) => line_circ(*a, *b, c, tol, &mut out),
        (c, Curve2::Line { a, b }) if circ(c) => {
            let mut tmp = Vec::new();
            line_circ(*a, *b, c, tol, &mut tmp);
            out.extend(tmp.into_iter().map(|x| Intersection { t1: x.t2, t2: x.t1, point: x.point }));
        }
        (a, b) if circ(a) && circ(b) => circ_circ(a, b, tol, &mut out),
        _ => generic(g1, g2, tol, &mut out),
    }
    // endpoints lying on the other curve
    for (ca, gb, first) in [(c1, g2, true), (c2, g1, false)] {
        if ca.is_periodic() {
            continue;
        }
        let (a, b) = ca.domain();
        for t in [a, b] {
            let p = ca.point(t);
            let (u, _, d) = gb.closest(p);
            if d <= tol {
                out.push(if first { Intersection { t1: t, t2: u, point: p } } else { Intersection { t1: u, t2: t, point: p } });
            }
        }
    }
    // dedupe by position
    let mut res: Vec<Intersection> = Vec::new();
    for x in out {
        if !res.iter().any(|y| (y.point - x.point).length() <= tol * 4.0) {
            res.push(x);
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_intersections() {
        let l1 = Curve2::Line { a: DVec2::new(-2.0, 0.0), b: DVec2::new(2.0, 0.0) };
        let c = Curve2::Circle { center: DVec2::ZERO, radius: 1.0 };
        assert_eq!(intersect(&l1, &c, 1e-9).len(), 2);
        let tl = Curve2::Line { a: DVec2::new(-2.0, 1.0), b: DVec2::new(2.0, 1.0) };
        let x = intersect(&tl, &c, 1e-9);
        assert_eq!(x.len(), 1);
        assert!((x[0].point - DVec2::new(0.0, 1.0)).length() < 1e-9);
        let c2 = Curve2::Circle { center: DVec2::new(1.0, 0.0), radius: 1.0 };
        assert_eq!(intersect(&c, &c2, 1e-9).len(), 2);
        let c3 = Curve2::Circle { center: DVec2::new(2.0, 0.0), radius: 1.0 };
        assert_eq!(intersect(&c, &c3, 1e-9).len(), 1);
        let e = Curve2::Ellipse { center: DVec2::ZERO, major: DVec2::new(2.0, 0.0), minor: 1.0 };
        let x = intersect(&l1, &e, 1e-9);
        assert_eq!(x.len(), 2, "{x:?}");
        for i in &x {
            assert!((i.point.x.abs() - 2.0).abs() < 1e-7 || i.point.y.abs() < 1e-9);
        }
        let l2 = Curve2::Line { a: DVec2::new(0.0, -3.0), b: DVec2::new(0.0, 3.0) };
        let x = intersect(&l2, &e, 1e-9);
        assert_eq!(x.len(), 2);
        for i in &x {
            assert!((i.point.y.abs() - 1.0).abs() < 1e-8);
        }
        // T-junction
        let l3 = Curve2::Line { a: DVec2::new(0.0, 0.0), b: DVec2::new(0.0, 5.0) };
        assert_eq!(intersect(&l1, &l3, 1e-9).len(), 1);
        // collinear overlap
        let l4 = Curve2::Line { a: DVec2::new(1.0, 0.0), b: DVec2::new(3.0, 0.0) };
        assert_eq!(intersect(&l1, &l4, 1e-9).len(), 2);
    }
}
