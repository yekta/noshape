//! Curves in a face's parameter plane: straight segments, circular arcs, trigonometric curves (an ellipse's
//! arc on a plane, a plane's section on a cylinder: a sinusoid) and hyperbolic ones (a hyperbola's arc on a plane
//! or on the cone it is a section of), the last two done numerically.

use crate::nurbs::NurbsCurve;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;
use std::sync::Arc;

/// A segment from `a` to `b`, an arc of the circle (`c`, `r`) from angle `a0` through `sweep` (positive:
/// counter-clockwise), the curve c + u cos t + v sin t + w t, or the curve c + u cosh t + v sinh t + w atan(k sinh t),
/// for t from `t0` through `sweep`, or a NURBS curve's part from its parameter `t0` to `t1` (its x and y; a curve
/// on a free-form face, OCCT's, in the face's own parameters).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum C2 {
    Seg { a: DVec2, b: DVec2 },
    Arc { c: DVec2, r: f64, a0: f64, sweep: f64 },
    Trig { c: DVec2, u: DVec2, v: DVec2, w: DVec2, t0: f64, sweep: f64 },
    Hyp { c: DVec2, u: DVec2, v: DVec2, w: DVec2, k: f64, t0: f64, sweep: f64 },
    Nurbs { c: Arc<NurbsCurve>, t0: f64, t1: f64 },
    /// A 3D NURBS curve's part from `t0` to `t1` seen in a plane's or cylinder's / cone's parameter plane.
    Mapped { c: Arc<NurbsCurve>, t0: f64, t1: f64, m: Map },
}

/// A face's map from 3D to its parameter plane (the Boolean's own): a plane's frame, or a cylinder's or cone's
/// (axis origin and direction, angle frame, scale `r`, `flip`: heights the other way, angles from `th0` through
/// 2π; `mid`: the curve's x halfway, which says on which side of the angles' ends its ends are).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Map {
    Plane { origin: DVec3, u: DVec3, v: DVec3 },
    Cyl { o: DVec3, a: DVec3, x: DVec3, y: DVec3, r: f64, flip: bool, th0: f64, mid: f64 },
}

impl Map {
    /// Point `p` and its derivative `d` (along a curve) mapped.
    pub fn map(&self, p: DVec3, d: DVec3) -> (DVec2, DVec2) {
        self.map_on(p, d, None)
    }

    /// Whether `p` is at a cylinder's angles' ends (where a curve's side there matters).
    pub fn at_seam(&self, p: DVec3) -> bool {
        match *self {
            Map::Plane { .. } => false,
            Map::Cyl { o, x, y, th0, .. } => {
                let w = p - o;
                let th = (w.dot(y).atan2(w.dot(x)) - th0).rem_euclid(TAU);
                th <= 1e-9 || th >= TAU - 1e-9
            }
        }
    }

    /// The same for a point of a curve going on from it along `d` (`end` false) or arriving at it along `d` (`end`
    /// true): at a cylinder's angles' ends, on the side the curve is on there (a curve from end to end of the
    /// angles, once round, starts at one and ends at the other).
    pub fn map_on(&self, p: DVec3, d: DVec3, end: Option<bool>) -> (DVec2, DVec2) {
        match *self {
            Map::Plane { origin, u, v } => (DVec2::new((p - origin).dot(u), (p - origin).dot(v)), DVec2::new(d.dot(u), d.dot(v))),
            Map::Cyl { o, a, x, y, r, flip, th0, mid } => {
                let w = p - o;
                let (wx, wy) = (w.dot(x), w.dot(y));
                let mut th = th0 + (wy.atan2(wx) - th0).rem_euclid(TAU);
                let dth = (wx * d.dot(y) - wy * d.dot(x)) / (wx * wx + wy * wy);
                // At the angles' ends: the side the curve is on (where it goes from there, or comes from; else the
                // side of its middle).
                let eps = 1e-9;
                let at_end = (th - th0).abs() <= eps || (th - th0 - TAU).abs() <= eps;
                let into = match end {
                    Some(e) if at_end && (r * dth).abs() > 1e-6 * d.length() => Some(if e { -dth } else { dth }),
                    _ => None,
                };
                if let Some(into) = into {
                    th = if into > 0.0 { th0 } else { th0 + TAU };
                } else if (th - th0).abs() <= eps && mid > r * (th0 + std::f64::consts::PI) {
                    th = th0 + TAU;
                } else if (th - th0 - TAU).abs() <= eps && mid < r * (th0 + std::f64::consts::PI) {
                    th = th0;
                }
                let s = if flip { -1.0 } else { 1.0 };
                (DVec2::new(r * th, s * w.dot(a)), DVec2::new(r * dth, s * d.dot(a)))
            }
        }
    }
}

/// Points along a curve to look for crossings between (a trigonometric curve's turns at most a quarter of a
/// turn apart, so a crossing is near a sample's segment).
const SAMPLES: usize = 64;

/// Gauss–Legendre on [0, 1]: 8 points (nodes, weights).
const GAUSS: [(f64, f64); 8] = [
    (0.019_855_071_751_231_88, 0.050_614_268_145_188_13),
    (0.101_666_761_293_186_6, 0.111_190_517_226_687_2),
    (0.237_233_795_041_835_5, 0.156_853_322_938_943_6),
    (0.408_282_678_752_175_1, 0.181_341_891_689_181),
    (0.591_717_321_247_825, 0.181_341_891_689_181),
    (0.762_766_204_958_164_5, 0.156_853_322_938_943_6),
    (0.898_333_238_706_813_4, 0.111_190_517_226_687_2),
    (0.980_144_928_248_768_1, 0.050_614_268_145_188_13),
];

fn dir(a: f64) -> DVec2 {
    DVec2::new(a.cos(), a.sin())
}

impl C2 {
    pub fn at(&self, t: f64) -> DVec2 {
        match *self {
            C2::Seg { a, b } => a + (b - a) * t,
            C2::Arc { c, r, a0, sweep } => c + dir(a0 + sweep * t) * r,
            C2::Trig { c, u, v, w, t0, sweep } => {
                let a = t0 + sweep * t;
                c + u * a.cos() + v * a.sin() + w * a
            }
            C2::Hyp { c, u, v, w, k, t0, sweep } => {
                let a = t0 + sweep * t;
                c + u * a.cosh() + v * a.sinh() + w * (k * a.sinh()).atan()
            }
            C2::Nurbs { ref c, t0, t1 } => c.at(t0 + (t1 - t0) * t).truncate(),
            C2::Mapped { ref c, t0, t1, m } => {
                let u = t0 + (t1 - t0) * t;
                let p = c.at(u);
                if m.at_seam(p) {
                    let [_, d1, _] = c.derivs(u);
                    m.map_on(p, d1 * (t1 - t0), Some(t > 0.5)).0
                } else {
                    m.map(p, DVec3::ZERO).0
                }
            }
        }
    }

    /// d/dt and d²/dt² of the point at `t` (0..1).
    fn derivs(&self, t: f64) -> (DVec2, DVec2) {
        match *self {
            C2::Seg { a, b } => (b - a, DVec2::ZERO),
            C2::Arc { r, a0, sweep, .. } => {
                let a = a0 + sweep * t;
                (dir(a).perp() * (r * sweep), -dir(a) * (r * sweep * sweep))
            }
            C2::Trig { u, v, w, t0, sweep, .. } => {
                let a = t0 + sweep * t;
                ((-u * a.sin() + v * a.cos() + w) * sweep, (-u * a.cos() - v * a.sin()) * (sweep * sweep))
            }
            C2::Hyp { u, v, w, k, t0, sweep, .. } => {
                // atan(g)' = g' / (1 + g²), atan(g)'' = (g'' (1 + g²) - 2 g g'²) / (1 + g²)², g = k sinh a.
                let a = t0 + sweep * t;
                let (g, g1) = (k * a.sinh(), k * a.cosh());
                let q = 1.0 + g * g;
                let (d1, d2) = (g1 / q, (g * q - 2.0 * g * g1 * g1) / (q * q));
                ((u * a.sinh() + v * a.cosh() + w * d1) * sweep, (u * a.cosh() + v * a.sinh() + w * d2) * (sweep * sweep))
            }
            C2::Nurbs { ref c, t0, t1 } => {
                let [_, d1, d2] = c.derivs(t0 + (t1 - t0) * t);
                (d1.truncate() * (t1 - t0), d2.truncate() * ((t1 - t0) * (t1 - t0)))
            }
            C2::Mapped { ref c, t0, t1, m } => {
                // The second derivative by differences of the first (Newton's steps only need it roughly).
                let d = |s: f64| {
                    let [p, d1, _] = c.derivs(t0 + (t1 - t0) * s);
                    m.map(p, d1).1 * (t1 - t0)
                };
                let h = 1e-5;
                let (a, b) = ((t - h).max(0.0), (t + h).min(1.0));
                (d(t), (d(b) - d(a)) / (b - a))
            }
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
            C2::Trig { c, u, v, w, t0, sweep } => C2::Trig { c, u, v, w, t0: t0 + sweep, sweep: -sweep },
            C2::Hyp { c, u, v, w, k, t0, sweep } => C2::Hyp { c, u, v, w, k, t0: t0 + sweep, sweep: -sweep },
            C2::Nurbs { ref c, t0, t1 } => C2::Nurbs { c: c.clone(), t0: t1, t1: t0 },
            C2::Mapped { ref c, t0, t1, m } => C2::Mapped { c: c.clone(), t0: t1, t1: t0, m },
        }
    }

    pub fn length(&self) -> f64 {
        match *self {
            C2::Seg { a, b } => (b - a).length(),
            C2::Arc { r, sweep, .. } => r * sweep.abs(),
            C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. } => self.gauss(|t| self.derivs(t).0.length()),
        }
    }

    /// ∫ f over 0..1, in 8 parts.
    fn gauss(&self, f: impl Fn(f64) -> f64) -> f64 {
        let mut sum = 0.0;
        for k in 0..8 {
            for &(x, wt) in &GAUSS {
                sum += wt * f((k as f64 + x) / 8.0);
            }
        }
        sum / 8.0
    }

    /// The unit direction at `t`.
    pub fn tangent(&self, t: f64) -> DVec2 {
        match *self {
            C2::Seg { a, b } => (b - a).normalize(),
            C2::Arc { a0, sweep, .. } => dir(a0 + sweep * t).perp() * sweep.signum(),
            C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. } => self.derivs(t).0.normalize(),
        }
    }

    /// Signed curvature (positive turning left) at the start.
    pub fn curvature(&self) -> f64 {
        match *self {
            C2::Seg { .. } => 0.0,
            C2::Arc { r, sweep, .. } => sweep.signum() / r,
            C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. } => {
                let (d, dd) = self.derivs(0.0);
                d.perp_dot(dd) / d.length().powi(3)
            }
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
            C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. } => self.gauss(|t| 0.5 * self.at(t).perp_dot(self.derivs(t).0)),
        }
    }

    /// The parameters of `n + 1` points evenly along it.
    fn samples(&self) -> usize {
        match self {
            C2::Seg { .. } => 1,
            // (8 a knot span of the part it is.)
            C2::Nurbs { c, t0, t1 } | C2::Mapped { c, t0, t1, .. } => {
                let (lo, hi) = (t0.min(*t1), t0.max(*t1));
                let spans = c.knots[c.degree..=c.poles.len()].windows(2).filter(|w| w[1] > w[0] && w[1] > lo && w[0] < hi).count();
                SAMPLES.max(8 * spans)
            }
            _ => SAMPLES,
        }
    }

    /// The roots in 0..1 of `f` along it (sign changes between samples, then bisection; exact zeros too).
    fn roots(&self, f: impl Fn(f64) -> f64) -> Vec<f64> {
        let n = self.samples();
        let mut out: Vec<f64> = Vec::new();
        let mut prev = f(0.0);
        if prev == 0.0 {
            out.push(0.0);
        }
        for k in 1..=n {
            let t1 = k as f64 / n as f64;
            let cur = f(t1);
            if cur == 0.0 {
                out.push(t1);
            } else if prev != 0.0 && (prev < 0.0) != (cur < 0.0) {
                let (mut lo, mut hi, mut flo) = ((k - 1) as f64 / n as f64, t1, prev);
                for _ in 0..80 {
                    let mid = 0.5 * (lo + hi);
                    let fm = f(mid);
                    if (fm < 0.0) == (flo < 0.0) {
                        lo = mid;
                        flo = fm;
                    } else {
                        hi = mid;
                    }
                }
                out.push(0.5 * (lo + hi));
            }
            prev = cur;
        }
        out
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
            C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. } => {
                for t in self.roots(|t| (self.at(t) - m).perp_dot(d)) {
                    let s = (self.at(t) - m).dot(d);
                    if s > 0.0 {
                        out.push(s);
                    }
                }
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
            C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. } => {
                // The nearest sample, then Newton on (q - p)·q' (within 0..1).
                let mut best = (0.0, f64::INFINITY);
                let n = self.samples();
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let d = (self.at(t) - p).length();
                    if d < best.1 {
                        best = (t, d);
                    }
                }
                let mut t = best.0;
                for _ in 0..30 {
                    let (d1, d2) = self.derivs(t);
                    let q = self.at(t) - p;
                    let g = q.dot(d1);
                    let gp = d1.dot(d1) + q.dot(d2);
                    if gp.abs() < 1e-300 {
                        break;
                    }
                    let nt = (t - g / gp).clamp(0.0, 1.0);
                    if (nt - t).abs() < 1e-15 {
                        t = nt;
                        break;
                    }
                    t = nt;
                }
                (t, (self.at(t) - p).length())
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
            C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. } => self.param_of(p).1,
        }
    }

    /// [`Self::intersections`] by sampling both into polylines and refining each crossing near a pair of their
    /// segments by Newton's method on the curves themselves; none where they run together (most of one's
    /// samples within `tol` of the other).
    /// A box it lies in, when one comes cheaply: a NURBS part's poles' (a cylinder's map: their heights, all its
    /// angles).
    pub(crate) fn quick_box(&self) -> Option<(DVec2, DVec2)> {
        let hull = |q: &mut dyn Iterator<Item = DVec2>| q.fold((DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)), |(l, h), x| (l.min(x), h.max(x)));
        match *self {
            C2::Seg { a, b } => Some((a.min(b), a.max(b))),
            C2::Arc { c, r, .. } => Some((c - DVec2::splat(r), c + DVec2::splat(r))),
            C2::Nurbs { ref c, t0, t1 } => Some(hull(&mut c.poles_of(t0, t1).iter().map(|p| p.truncate()))),
            C2::Mapped { ref c, t0, t1, m: Map::Plane { origin, u, v } } => Some(hull(&mut c.poles_of(t0, t1).iter().map(|p| DVec2::new((*p - origin).dot(u), (*p - origin).dot(v))))),
            C2::Mapped { ref c, t0, t1, m: Map::Cyl { o, a, r, flip, th0, .. } } => {
                let s = if flip { -1.0 } else { 1.0 };
                let (lo, hi) = hull(&mut c.poles_of(t0, t1).iter().map(|p| DVec2::splat(s * (*p - o).dot(a))));
                Some((DVec2::new(r * th0, lo.y), DVec2::new(r * (th0 + TAU), hi.y)))
            }
            C2::Trig { .. } | C2::Hyp { .. } => None,
        }
    }

    fn numeric_intersections(&self, o: &C2, tol: f64) -> Vec<(f64, f64)> {
        // (Boxes apart: none.)
        if let (Some(a), Some(b)) = (self.quick_box(), o.quick_box()) {
            let g = 4.0 * tol;
            if a.1.x + g < b.0.x || b.1.x + g < a.0.x || a.1.y + g < b.0.y || b.1.y + g < a.0.y {
                return Vec::new();
            }
        }
        let together = |a: &C2, b: &C2| (0..=8).filter(|&k| b.distance(a.at(k as f64 / 8.0)) <= tol).count() >= 5;
        if together(self, o) || together(o, self) {
            return Vec::new();
        }
        let (na, nb) = (self.samples(), o.samples());
        let pa: Vec<DVec2> = (0..=na).map(|k| self.at(k as f64 / na as f64)).collect();
        let pb: Vec<DVec2> = (0..=nb).map(|k| o.at(k as f64 / nb as f64)).collect();
        let scale = pa.iter().chain(&pb).fold(0.0_f64, |m, p| m.max(p.abs().max_element())).max(1e-9);
        let mut out: Vec<(f64, f64)> = Vec::new();
        // Runs of 16 chords each, their boxes grown as the chords' below: only runs whose boxes meet are looked at.
        const RUN: usize = 16;
        let runs = |p: &[DVec2], n: usize, pad: f64| -> Vec<(DVec2, DVec2)> {
            (0..n.div_ceil(RUN))
                .map(|r| {
                    let q = &p[r * RUN..=((r + 1) * RUN).min(n)];
                    let (lo, hi) = q.iter().fold((DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)), |(l, h), &x| (l.min(x), h.max(x)));
                    let long = q.windows(2).fold(0.0_f64, |m, w| m.max((w[1] - w[0]).length()));
                    (lo - DVec2::splat(0.25 * long + pad), hi + DVec2::splat(0.25 * long + pad))
                })
                .collect()
        };
        let (ra, rb) = (runs(&pa, na, 4.0 * tol), runs(&pb, nb, 0.0));
        let chords = |ia: std::ops::Range<usize>, ib: std::ops::Range<usize>, out: &mut Vec<(f64, f64)>| {
            for i in ia {
                let (a0, a1) = (pa[i], pa[i + 1]);
                let (alo, ahi) = (a0.min(a1) - DVec2::splat(4.0 * tol), a0.max(a1) + DVec2::splat(4.0 * tol));
                for j in ib.clone() {
                    let (b0, b1) = (pb[j], pb[j + 1]);
                    // (Chords bulge from their curve: seeds from any pair whose boxes, grown, meet.)
                    let grow = 0.25 * ((a1 - a0).length() + (b1 - b0).length());
                    let (blo, bhi) = (b0.min(b1) - DVec2::splat(grow), b0.max(b1) + DVec2::splat(grow));
                    if ahi.x < blo.x || bhi.x < alo.x || ahi.y < blo.y || bhi.y < alo.y {
                        continue;
                    }
                    let (mut s, mut t) = ((i as f64 + 0.5) / na as f64, (j as f64 + 0.5) / nb as f64);
                    let mut ok = false;
                    for _ in 0..40 {
                        let f = self.at(s) - o.at(t);
                        let (da, db) = (self.derivs(s).0, -o.derivs(t).0);
                        let det = da.perp_dot(db);
                        if det.abs() < 1e-300 {
                            break;
                        }
                        let ds = -f.perp_dot(db) / det;
                        let dt = -da.perp_dot(f) / det;
                        s += ds;
                        t += dt;
                        if !(-0.5..=1.5).contains(&s) || !(-0.5..=1.5).contains(&t) {
                            break;
                        }
                        if ds.abs() < 1e-15 && dt.abs() < 1e-15 {
                            ok = true;
                            break;
                        }
                    }
                    let f = (self.at(s) - o.at(t)).length();
                    if !(ok || f <= 1e-12 * scale) || f > 1e-9 * scale {
                        continue;
                    }
                    let e = 1e-9;
                    if s < -e || s > 1.0 + e || t < -e || t > 1.0 + e {
                        continue;
                    }
                    let (s, t) = (s.clamp(0.0, 1.0), t.clamp(0.0, 1.0));
                    if !out.iter().any(|x| (x.0 - s).abs() < 1e-9 && (x.1 - t).abs() < 1e-9) {
                        out.push((s, t));
                    }
                }
            }
        };
        for (ri, a) in ra.iter().enumerate() {
            for (rj, b) in rb.iter().enumerate() {
                if a.1.x < b.0.x || b.1.x < a.0.x || a.1.y < b.0.y || b.1.y < a.0.y {
                    continue;
                }
                chords(ri * RUN..((ri + 1) * RUN).min(na), rj * RUN..((rj + 1) * RUN).min(nb), &mut out);
            }
        }
        out.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
        out
    }

    /// Parameters (on `self`, on `o`) where the two cross, ends included; none where they run together
    /// (within `tol`: collinear segments, arcs of one circle), which their ends lying on each other show.
    pub fn intersections(&self, o: &C2, tol: f64) -> Vec<(f64, f64)> {
        if matches!(self, C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. }) || matches!(o, C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. }) {
            return self.numeric_intersections(o, tol);
        }
        let mut out = Vec::new();
        match (self.clone(), o.clone()) {
            (C2::Seg { a, b }, C2::Seg { a: c, b: d }) => {
                let (e, f) = (b - a, d - c);
                let off = |p: DVec2, q: DVec2, x: DVec2| (x - p).perp_dot((q - p).normalize()).abs();
                if (off(a, b, c) <= tol && off(a, b, d) <= tol) || (off(c, d, a) <= tol && off(c, d, b) <= tol) {
                    return out;
                }
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
            (C2::Arc { .. }, C2::Seg { .. }) => out = o.intersections(self, tol).into_iter().map(|(u, t)| (t, u)).collect(),
            (C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. }, _) | (_, C2::Trig { .. } | C2::Hyp { .. } | C2::Nurbs { .. } | C2::Mapped { .. }) => unreachable!(),
            (C2::Arc { c: c1, r: r1, a0: s1, sweep: w1 }, C2::Arc { c: c2, r: r2, a0: s2, sweep: w2 }) => {
                let d = c2 - c1;
                let dl = d.length();
                if dl <= tol && (r1 - r2).abs() <= tol {
                    return out;
                }
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

/// Whether `p` is inside the loops of `edges` (even-odd), by a ray in a direction no input favours; another
/// one when it passes through an end of a curve.
pub(crate) fn inside(p: DVec2, edges: &[C2]) -> bool {
    const ANGLES: [f64; 4] = [0.371_1, 1.913_7, 4.170_9, 2.653_1];
    let scale = edges.iter().map(|e| e.start().abs().max_element()).fold(p.abs().max_element(), f64::max).max(1e-3);
    let mut odd = false;
    for a in ANGLES {
        let d = DVec2::new(a.cos(), a.sin());
        let mut hits = Vec::new();
        let mut clean = true;
        for e in edges {
            let n = hits.len();
            e.ray_hits(p, d, &mut hits);
            for &s in &hits[n..] {
                let q = p + d * s;
                if (q - e.start()).length() <= 1e-9 * scale || (q - e.end()).length() <= 1e-9 * scale {
                    clean = false;
                }
            }
        }
        odd = hits.len() % 2 == 1;
        if clean {
            break;
        }
    }
    odd
}

/// The distance from `p` to the nearest of `edges`.
pub(crate) fn distance(p: DVec2, edges: &[C2]) -> f64 {
    // Nearest box first; none whose box is farther than the nearest found yet.
    let mut order: Vec<(f64, &C2)> = edges.iter().map(|e| (e.quick_box().map_or(0.0, |(lo, hi)| (lo - p).max(p - hi).max(DVec2::ZERO).length()), e)).collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut best = f64::INFINITY;
    for (away, e) in order {
        if away >= best {
            break;
        }
        best = best.min(e.distance(p));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn a_disk_with_a_square_hole() {
        let disk = C2::Arc { c: DVec2::ZERO, r: 2.0, a0: 0.3, sweep: TAU };
        let sq = [(-0.5, -0.5), (-0.5, 0.5), (0.5, 0.5), (0.5, -0.5)];
        let mut edges = vec![disk.clone()];
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
        // The case that found the half-open rule's fault: a full circle's start level with the point.
        let full = C2::Arc { c: DVec2::ZERO, r: 0.0025, a0: 0.0, sweep: -TAU };
        assert!(!inside(DVec2::new(-0.02, 0.0), std::slice::from_ref(&full)));
        assert!(inside(DVec2::new(-0.001, 0.0), std::slice::from_ref(&full)));
        let half = C2::Arc { c: DVec2::ZERO, r: 1.0, a0: 0.0, sweep: -PI };
        assert!(inside(DVec2::new(0.0, -0.5), &[half, C2::Seg { a: DVec2::new(-1.0, 0.0), b: DVec2::new(1.0, 0.0) }]));
        let line = C2::Seg { a: DVec2::new(-3.0, 1.0), b: DVec2::new(3.0, 1.0) };
        assert_eq!(line.intersections(&disk, 1e-9).len(), 2);
        let mut hits = Vec::new();
        disk.ray_hits(DVec2::ZERO, DVec2::X, &mut hits);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn a_curve_once_round_a_cylinder_spans_its_angles() {
        // A circle's NURBS rising along the axis: from the cylinder's seam once round to it, 0.8 higher.
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let xy = [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.0)];
        let c = crate::nurbs::NurbsCurve {
            degree: 2,
            knots: vec![0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0],
            poles: xy.iter().enumerate().map(|(k, &(x, y))| DVec3::new(x, y, 0.1 * k as f64)).collect(),
            weights: (0..9).map(|k| if k % 2 == 0 { 1.0 } else { h }).collect(),
            spans: Default::default(),
        };
        let m = Map::Cyl { o: DVec3::ZERO, a: DVec3::Z, x: DVec3::X, y: DVec3::Y, r: 1.0, flip: false, th0: 0.0, mid: PI };
        let up = C2::Mapped { c: std::sync::Arc::new(c), t0: 0.0, t1: 1.0, m };
        assert!(up.start().distance(DVec2::new(0.0, 0.0)) < 1e-12, "{:?}", up.start());
        assert!(up.end().distance(DVec2::new(TAU, 0.8)) < 1e-12, "{:?}", up.end());
        let down = up.reversed();
        assert!(down.start().distance(DVec2::new(TAU, 0.8)) < 1e-12 && down.end().distance(DVec2::ZERO) < 1e-12);
    }

    #[test]
    fn an_ellipse_and_a_sinusoid() {
        // x²/4 + y² = 1 counter-clockwise, its start at (2, 0).
        let el = C2::Trig { c: DVec2::ZERO, u: DVec2::new(2.0, 0.0), v: DVec2::new(0.0, 1.0), w: DVec2::ZERO, t0: 0.0, sweep: TAU };
        assert!((el.area_term() - 2.0 * PI).abs() < 1e-12, "{}", el.area_term());
        assert!((el.reversed().area_term() + 2.0 * PI).abs() < 1e-12);
        // Its perimeter (Ramanujan's second approximation is good to ~1e-10 here).
        let h: f64 = 1.0 / 9.0;
        let ramanujan = PI * 3.0 * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()));
        assert!((el.length() - ramanujan).abs() < 1e-8, "{} {ramanujan}", el.length());
        assert!(inside(DVec2::new(1.5, 0.3), std::slice::from_ref(&el)));
        assert!(!inside(DVec2::new(1.5, 0.8), std::slice::from_ref(&el)));
        assert!((distance(DVec2::new(3.0, 0.0), std::slice::from_ref(&el)) - 1.0).abs() < 1e-12);
        assert!((distance(DVec2::new(0.0, -3.0), std::slice::from_ref(&el)) - 2.0).abs() < 1e-12);
        let (t, d) = el.param_of(DVec2::new(0.0, 1.0));
        assert!((t - 0.25).abs() < 1e-12 && d < 1e-12);
        // A segment across it: x = ±√3 at y = 1/2.
        let line = C2::Seg { a: DVec2::new(-3.0, 0.5), b: DVec2::new(3.0, 0.5) };
        let mut xs: Vec<f64> = line.intersections(&el, 1e-9).iter().map(|&(s, t)| {
            assert!((line.at(s) - el.at(t)).length() < 1e-12);
            line.at(s).x
        }).collect();
        xs.sort_by(f64::total_cmp);
        assert_eq!(xs.len(), 2);
        assert!((xs[0] + 3f64.sqrt()).abs() < 1e-12 && (xs[1] - 3f64.sqrt()).abs() < 1e-12, "{xs:?}");
        // A circle of radius 1.5 crosses it four times (x² = 5/3); the other way round alike.
        let circle = C2::Arc { c: DVec2::ZERO, r: 1.5, a0: 0.1, sweep: TAU };
        let hits = el.intersections(&circle, 1e-9);
        assert_eq!(hits.len(), 4, "{hits:?}");
        for &(s, t) in &hits {
            assert!((el.at(s) - circle.at(t)).length() < 1e-12);
            assert!((el.at(s).x.abs() - (5f64 / 3.0).sqrt()).abs() < 1e-12);
        }
        assert_eq!(circle.intersections(&el, 1e-9).len(), 4);
        // Itself: running together, no crossings.
        assert!(el.intersections(&el, 1e-9).is_empty());
        // On a cylinder's parameter plane (angle, height): a plane's section is h + k sin(angle).
        let wave = C2::Trig { c: DVec2::new(0.0, 0.5), u: DVec2::ZERO, v: DVec2::new(0.0, 0.25), w: DVec2::X, t0: 0.0, sweep: TAU };
        let level = C2::Seg { a: DVec2::new(-1.0, 0.5), b: DVec2::new(7.0, 0.5) };
        let mut us: Vec<f64> = level.intersections(&wave, 1e-9).iter().map(|&(s, _)| level.at(s).x).collect();
        us.sort_by(f64::total_cmp);
        assert_eq!(us.len(), 3, "{us:?}");
        for (u, want) in us.iter().zip([0.0, PI, TAU]) {
            assert!((u - want).abs() < 1e-12, "{us:?}");
        }
        assert!((wave.curvature() - 0.0).abs() < 1e-12);
        assert!((wave.tangent(0.25) - DVec2::X).length() < 1e-12);
    }
}
