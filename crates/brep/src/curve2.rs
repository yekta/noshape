//! Curves in a face's parameter plane: straight segments and circular arcs.

use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// A segment from `a` to `b`, or an arc of the circle (`c`, `r`) from angle `a0` through `sweep` (positive:
/// counter-clockwise).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum C2 {
    Seg { a: DVec2, b: DVec2 },
    Arc { c: DVec2, r: f64, a0: f64, sweep: f64 },
}

fn dir(a: f64) -> DVec2 {
    DVec2::new(a.cos(), a.sin())
}

impl C2 {
    pub fn at(&self, t: f64) -> DVec2 {
        match *self {
            C2::Seg { a, b } => a + (b - a) * t,
            C2::Arc { c, r, a0, sweep } => c + dir(a0 + sweep * t) * r,
        }
    }

    pub fn start(&self) -> DVec2 {
        self.at(0.0)
    }

    pub fn end(&self) -> DVec2 {
        self.at(1.0)
    }

    pub fn reversed(&self) -> C2 {
        match *self {
            C2::Seg { a, b } => C2::Seg { a: b, b: a },
            C2::Arc { c, r, a0, sweep } => C2::Arc { c, r, a0: a0 + sweep, sweep: -sweep },
        }
    }

    pub fn length(&self) -> f64 {
        match *self {
            C2::Seg { a, b } => (b - a).length(),
            C2::Arc { r, sweep, .. } => r * sweep.abs(),
        }
    }

    /// The unit direction at `t`.
    pub fn tangent(&self, t: f64) -> DVec2 {
        match *self {
            C2::Seg { a, b } => (b - a).normalize(),
            C2::Arc { a0, sweep, .. } => dir(a0 + sweep * t).perp() * sweep.signum(),
        }
    }

    /// Signed curvature (positive turning left).
    pub fn curvature(&self) -> f64 {
        match *self {
            C2::Seg { .. } => 0.0,
            C2::Arc { r, sweep, .. } => sweep.signum() / r,
        }
    }

    /// ½ ∫ (x dy − y dx): a closed loop's terms add up to its signed area.
    pub fn area_term(&self) -> f64 {
        match *self {
            C2::Seg { a, b } => 0.5 * a.perp_dot(b),
            C2::Arc { c, r, a0, sweep } => {
                let a1 = a0 + sweep;
                0.5 * (r * r * sweep + r * c.x * (a1.sin() - a0.sin()) - r * c.y * (a1.cos() - a0.cos()))
            }
        }
    }

    /// The parameter of angle `a` on an arc, if inside it.
    fn arc_t(a0: f64, sweep: f64, a: f64) -> Option<f64> {
        let d = if sweep >= 0.0 { (a - a0).rem_euclid(TAU) } else { (a0 - a).rem_euclid(TAU) };
        let t = d / sweep.abs();
        (t <= 1.0).then_some(t)
    }

    /// Where the ray `m + s d` (s > 0) meets it.
    pub fn ray_hits(&self, m: DVec2, d: DVec2, out: &mut Vec<f64>) {
        match *self {
            C2::Seg { a, b } => {
                let e = b - a;
                let den = d.perp_dot(e);
                if den.abs() < 1e-300 {
                    return;
                }
                let w = a - m;
                let s = w.perp_dot(e) / den;
                let u = w.perp_dot(d) / den;
                if s > 0.0 && (0.0..=1.0).contains(&u) {
                    out.push(s);
                }
            }
            C2::Arc { c, r, a0, sweep } => {
                let w = m - c;
                let (b, cc) = (w.dot(d), w.length_squared() - r * r);
                let disc = b * b - cc;
                if disc < 0.0 {
                    return;
                }
                for s in [-b - disc.sqrt(), -b + disc.sqrt()] {
                    let p = w + d * s;
                    if s > 0.0 && C2::arc_t(a0, sweep, p.y.atan2(p.x)).is_some() {
                        out.push(s);
                    }
                }
            }
        }
    }

    /// Crossings of the ray from `p` towards +x (half-open in y, so a loop's crossings add up right).
    pub fn crossings(&self, p: DVec2) -> usize {
        match *self {
            C2::Seg { a, b } => {
                if (a.y > p.y) != (b.y > p.y) {
                    let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
                    usize::from(x > p.x)
                } else {
                    0
                }
            }
            C2::Arc { c, r, a0, sweep } => {
                // Pieces monotone in y, split where the tangent is horizontal (angles π/2 + kπ).
                let a1 = a0 + sweep;
                let (lo, hi) = (a0.min(a1), a0.max(a1));
                let mut cuts = vec![lo];
                let mut k = ((lo - FRAC_PI_2) / PI).floor() + 1.0;
                while FRAC_PI_2 + k * PI < hi {
                    cuts.push(FRAC_PI_2 + k * PI);
                    k += 1.0;
                }
                cuts.push(hi);
                let mut n = 0;
                for w in cuts.windows(2) {
                    let (ya, yb) = (c.y + r * w[0].sin(), c.y + r * w[1].sin());
                    if (ya > p.y) != (yb > p.y) {
                        // On this monotone piece, the angle where y = p.y.
                        let s = ((p.y - c.y) / r).clamp(-1.0, 1.0).asin();
                        let mut found = None;
                        for cand in [s, PI - s] {
                            let mut a = cand;
                            while a < w[0] - 1e-12 {
                                a += TAU;
                            }
                            while a > w[1] + 1e-12 {
                                a -= TAU;
                            }
                            if a >= w[0] - 1e-12 && a <= w[1] + 1e-12 {
                                found = Some(a);
                                break;
                            }
                        }
                        if let Some(a) = found
                            && c.x + r * a.cos() > p.x
                        {
                            n += 1;
                        }
                    }
                }
                n
            }
        }
    }

    /// The parameter of the point nearest `p`, and its distance.
    pub fn param_of(&self, p: DVec2) -> (f64, f64) {
        match *self {
            C2::Seg { a, b } => {
                let e = b - a;
                let t = (p - a).dot(e) / e.length_squared();
                (t, (a + e * t - p).length())
            }
            C2::Arc { c, r, a0, sweep } => {
                let w = p - c;
                let d = (w.length() - r).abs();
                let a = w.y.atan2(w.x);
                let rel = if sweep >= 0.0 { (a - a0).rem_euclid(TAU) } else { (a0 - a).rem_euclid(TAU) };
                (rel / sweep.abs(), d)
            }
        }
    }

    pub fn distance(&self, p: DVec2) -> f64 {
        match *self {
            C2::Seg { a, b } => {
                let e = b - a;
                let l2 = e.length_squared();
                let t = if l2 > 0.0 { ((p - a).dot(e) / l2).clamp(0.0, 1.0) } else { 0.0 };
                (a + e * t - p).length()
            }
            C2::Arc { c, r, a0, sweep } => {
                let w = p - c;
                if C2::arc_t(a0, sweep, w.y.atan2(w.x)).is_some() {
                    (w.length() - r).abs()
                } else {
                    (self.start() - p).length().min((self.end() - p).length())
                }
            }
        }
    }

    /// Parameters (on `self`, on `o`) where the two meet, ends included.
    pub fn intersections(&self, o: &C2) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        match (*self, *o) {
            (C2::Seg { a, b }, C2::Seg { a: c, b: d }) => {
                let (e, f) = (b - a, d - c);
                let den = e.perp_dot(f);
                if den.abs() > 1e-300 {
                    let w = c - a;
                    let (t, u) = (w.perp_dot(f) / den, w.perp_dot(e) / den);
                    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                        out.push((t, u));
                    }
                }
            }
            (C2::Seg { a, b }, C2::Arc { c, r, a0, sweep }) => {
                let e = b - a;
                let w = a - c;
                let (qa, qb, qc) = (e.length_squared(), w.dot(e), w.length_squared() - r * r);
                let disc = qb * qb - qa * qc;
                if disc >= 0.0 && qa > 0.0 {
                    for t in [(-qb - disc.sqrt()) / qa, (-qb + disc.sqrt()) / qa] {
                        let p = w + e * t;
                        if (0.0..=1.0).contains(&t)
                            && let Some(u) = C2::arc_t(a0, sweep, p.y.atan2(p.x))
                        {
                            out.push((t, u));
                        }
                    }
                }
            }
            (C2::Arc { .. }, C2::Seg { .. }) => out = o.intersections(self).into_iter().map(|(u, t)| (t, u)).collect(),
            (C2::Arc { c: c1, r: r1, a0: s1, sweep: w1 }, C2::Arc { c: c2, r: r2, a0: s2, sweep: w2 }) => {
                let d = c2 - c1;
                let dl = d.length();
                if dl > 1e-300 && dl <= r1 + r2 && dl >= (r1 - r2).abs() {
                    let x = (dl * dl + r1 * r1 - r2 * r2) / (2.0 * dl);
                    let h = (r1 * r1 - x * x).max(0.0).sqrt();
                    let base = c1 + d * (x / dl);
                    for p in [base + d.perp() * (h / dl), base - d.perp() * (h / dl)] {
                        let (q1, q2) = (p - c1, p - c2);
                        if let (Some(t), Some(u)) = (C2::arc_t(s1, w1, q1.y.atan2(q1.x)), C2::arc_t(s2, w2, q2.y.atan2(q2.x))) {
                            out.push((t, u));
                        }
                    }
                }
            }
        }
        out
    }
}

/// Whether `p` is inside the loops of `edges` (even-odd).
pub(crate) fn inside(p: DVec2, edges: &[C2]) -> bool {
    edges.iter().map(|e| e.crossings(p)).sum::<usize>() % 2 == 1
}

/// The distance from `p` to the nearest of `edges`.
pub(crate) fn distance(p: DVec2, edges: &[C2]) -> f64 {
    edges.iter().map(|e| e.distance(p)).fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disk_with_a_square_hole() {
        let disk = C2::Arc { c: DVec2::ZERO, r: 2.0, a0: 0.3, sweep: TAU };
        let sq = [(-0.5, -0.5), (-0.5, 0.5), (0.5, 0.5), (0.5, -0.5)];
        let mut edges = vec![disk];
        for k in 0..4 {
            let (a, b) = (sq[k], sq[(k + 1) % 4]);
            edges.push(C2::Seg { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1) });
        }
        let area: f64 = edges.iter().map(|e| e.area_term()).sum();
        assert!((area - (4.0 * PI - 1.0)).abs() < 1e-12);
        assert!(inside(DVec2::new(1.0, 0.0), &edges));
        assert!(inside(DVec2::new(0.0, 1.9), &edges));
        assert!(!inside(DVec2::new(0.1, 0.1), &edges));
        assert!(!inside(DVec2::new(2.1, 0.0), &edges));
        assert!((distance(DVec2::new(3.0, 0.0), &edges) - 1.0).abs() < 1e-12);
        let half = C2::Arc { c: DVec2::ZERO, r: 1.0, a0: 0.0, sweep: -PI };
        assert!(inside(DVec2::new(0.0, -0.5), &[half, C2::Seg { a: DVec2::new(-1.0, 0.0), b: DVec2::new(1.0, 0.0) }]));
        let line = C2::Seg { a: DVec2::new(-3.0, 1.0), b: DVec2::new(3.0, 1.0) };
        assert_eq!(line.intersections(&disk).len(), 2);
        let mut hits = Vec::new();
        disk.ray_hits(DVec2::ZERO, DVec2::X, &mut hits);
        assert_eq!(hits.len(), 1);
    }
}
