//! Free-form faces' surfaces as the Boolean evaluates them, in OCCT's own parameters: NURBS, tori, spheres, a
//! curve swept along a direction or revolved about an axis.

use crate::nurbs::{NurbsCurve, NurbsSurface};
use std::f64::consts::TAU;
use glam::{DVec2, DVec3};

/// A surface by its parameters (u, v), OCCT's.
#[derive(Clone, Debug, PartialEq)]
pub enum FreeSurface {
    Nurbs(NurbsSurface),
    /// o + (major + minor cos v)(cos u x + sin u y) + minor sin v z (OCCT's frame: z is its direction, which a
    /// left-handed frame has as minus x × y).
    Torus { o: DVec3, x: DVec3, y: DVec3, z: DVec3, major: f64, minor: f64 },
    /// o + r cos v (cos u x + sin u y) + r sin v z.
    Sphere { o: DVec3, x: DVec3, y: DVec3, z: DVec3, r: f64 },
    /// c(u) + v d (d unit).
    Extrusion { c: NurbsCurve, d: DVec3 },
    /// c(v) turned by u about the axis (o, a) (a unit).
    Revolution { c: NurbsCurve, o: DVec3, a: DVec3 },
}

impl FreeSurface {
    /// Its parameters' natural range (None along an unbounded or periodic direction: the face's own bounds
    /// apply).
    pub fn range(&self) -> [Option<(f64, f64)>; 2] {
        match self {
            FreeSurface::Nurbs(s) => {
                let (u, v) = s.range();
                [Some(u), Some(v)]
            }
            FreeSurface::Sphere { .. } => [None, Some((-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2))],
            FreeSurface::Torus { .. } => [None, None],
            FreeSurface::Extrusion { c, .. } => [Some(c.range()), None],
            FreeSurface::Revolution { c, .. } => [None, Some(c.range())],
        }
    }

    /// Whether its parameters go round along u and along v (a whole turn of 2π: a sphere's, a torus's, a revolution's
    /// angles).
    pub fn periodic(&self) -> [bool; 2] {
        match self {
            FreeSurface::Sphere { .. } | FreeSurface::Revolution { .. } => [true, false],
            FreeSurface::Torus { .. } => [true, true],
            FreeSurface::Nurbs(_) | FreeSurface::Extrusion { .. } => [false, false],
        }
    }

    /// Knot values along u (`along_u`) or v where it is less smooth (a NURBS surface's spans, a swept curve's).
    pub fn breaks(&self, along_u: bool) -> Vec<f64> {
        let k = |c: &NurbsCurve| c.knots[c.degree..=c.poles.len()].to_vec();
        match self {
            FreeSurface::Nurbs(s) => {
                if along_u {
                    s.uknots[s.udegree..=s.nu].to_vec()
                } else {
                    s.vknots[s.vdegree..=s.nv].to_vec()
                }
            }
            FreeSurface::Extrusion { c, .. } if along_u => k(c),
            FreeSurface::Revolution { c, .. } if !along_u => k(c),
            _ => Vec::new(),
        }
    }

    /// The point at (u, v) and its derivatives: [S, Su, Sv, Suu, Suv, Svv].
    pub fn derivs(&self, u: f64, v: f64) -> [DVec3; 6] {
        match self {
            FreeSurface::Nurbs(s) => s.derivs(u, v),
            &FreeSurface::Torus { o, x, y, z, major, minor } => {
                let (cu, su, cv, sv) = (u.cos(), u.sin(), v.cos(), v.sin());
                let (rad, drad) = (x * cu + y * su, -x * su + y * cu);
                let w = major + minor * cv;
                [
                    o + rad * w + z * (minor * sv),
                    drad * w,
                    rad * (-minor * sv) + z * (minor * cv),
                    -rad * w,
                    drad * (-minor * sv),
                    rad * (-minor * cv) - z * (minor * sv),
                ]
            }
            &FreeSurface::Sphere { o, x, y, z, r } => {
                let (cu, su, cv, sv) = (u.cos(), u.sin(), v.cos(), v.sin());
                let (rad, drad) = (x * cu + y * su, -x * su + y * cu);
                [o + rad * (r * cv) + z * (r * sv), drad * (r * cv), rad * (-r * sv) + z * (r * cv), -rad * (r * cv), drad * (-r * sv), rad * (-r * cv) - z * (r * sv)]
            }
            FreeSurface::Extrusion { c, d } => {
                let [p, p1, p2] = c.derivs(u);
                [p + *d * v, p1, *d, p2, DVec3::ZERO, DVec3::ZERO]
            }
            FreeSurface::Revolution { c, o, a } => {
                // Turning q about a by u: (q·a) a + cos u q⊥ + sin u (a × q).
                let [p, p1, p2] = c.derivs(v);
                let (cu, su) = (u.cos(), u.sin());
                let turn = |q: DVec3, c0: f64, s0: f64| (q - *a * q.dot(*a)) * c0 + a.cross(q) * s0;
                let rel = p - *o;
                let along = |q: DVec3| *a * q.dot(*a);
                [
                    *o + along(rel) + turn(rel, cu, su),
                    turn(rel, -su, cu),
                    along(p1) + turn(p1, cu, su),
                    turn(rel, -cu, -su),
                    turn(p1, -su, cu),
                    along(p2) + turn(p2, cu, su),
                ]
            }
        }
    }

    /// A box holding it over [u.0, u.1] × [v.0, v.1], from its control points' hull (None where a weight isn't
    /// positive).
    pub fn hull_box(&self, u: (f64, f64), v: (f64, f64)) -> Option<(DVec3, DVec3)> {
        let poles = |p: &[DVec3], w: &[f64]| -> Option<(DVec3, DVec3)> {
            if w.iter().any(|&w| w <= 0.0) {
                return None;
            }
            Some(p.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(l, h), &q| (l.min(q), h.max(q))))
        };
        match self {
            FreeSurface::Nurbs(s) => s.hull_box(u, v),
            // (Over v: from the axis at most major + minor cos v, along it minor sin v, as a ring of revolution.)
            &FreeSurface::Sphere { o, z, r, .. } => {
                let (c, s) = cos_sin_ranges(v);
                Some(ring_box(o, z, (r * s.0, r * s.1), r * c.0.abs().max(c.1.abs())))
            }
            &FreeSurface::Torus { o, z, major, minor, .. } => {
                let (c, s) = cos_sin_ranges(v);
                let (h0, h1) = if minor >= 0.0 { (minor * s.0, minor * s.1) } else { (minor * s.1, minor * s.0) };
                Some(ring_box(o, z, (h0, h1), (major + minor * c.0).abs().max((major + minor * c.1).abs())))
            }
            FreeSurface::Extrusion { c, d } => {
                if !v.0.is_finite() || !v.1.is_finite() {
                    return None;
                }
                let (l, h) = c.part_box(u.0, u.1).or_else(|| poles(&c.poles, &c.weights))?;
                Some(((l + *d * v.0).min(l + *d * v.1), (h + *d * v.0).max(h + *d * v.1)))
            }
            FreeSurface::Revolution { c, o, a } => {
                // (Along the axis, its part's hull's range; across it, the hull's farthest corner from it.)
                let (l, h) = c.part_box(v.0, v.1).or_else(|| poles(&c.poles, &c.weights))?;
                let (mut h0, mut h1, mut r) = (f64::INFINITY, f64::NEG_INFINITY, 0.0f64);
                for q in (0..8).map(|k| DVec3::new(if k & 1 == 0 { l.x } else { h.x }, if k & 2 == 0 { l.y } else { h.y }, if k & 4 == 0 { l.z } else { h.z })) {
                    let rel = q - *o;
                    let h = rel.dot(*a);
                    h0 = h0.min(h);
                    h1 = h1.max(h);
                    r = r.max((rel - *a * h).length());
                }
                Some(ring_box(*o, *a, (h0, h1), r))
            }
        }
    }

    pub fn at(&self, u: f64, v: f64) -> DVec3 {
        match self {
            FreeSurface::Nurbs(s) => s.at(u, v),
            _ => self.derivs(u, v)[0],
        }
    }

    /// Its points at each (u, v) of `us` × `vs`, row by row in u (as [`Self::at`]: an extrusion's curve evaluated
    /// once a u, a revolution's once a v).
    pub fn grid(&self, us: &[f64], vs: &[f64]) -> Vec<DVec3> {
        let mut out = Vec::with_capacity(us.len() * vs.len());
        match self {
            FreeSurface::Extrusion { c, d } => {
                for &u in us {
                    let p = c.derivs(u)[0];
                    out.extend(vs.iter().map(|&v| p + *d * v));
                }
            }
            FreeSurface::Revolution { c, o, a } => {
                let rows: Vec<(DVec3, DVec3, DVec3)> = vs
                    .iter()
                    .map(|&v| {
                        let rel = c.derivs(v)[0] - *o;
                        (*o + *a * rel.dot(*a), rel - *a * rel.dot(*a), a.cross(rel))
                    })
                    .collect();
                for &u in us {
                    let (cu, su) = (u.cos(), u.sin());
                    out.extend(rows.iter().map(|&(base, perp, side)| base + (perp * cu + side * su)));
                }
            }
            _ => {
                for &u in us {
                    out.extend(vs.iter().map(|&v| self.at(u, v)));
                }
            }
        }
        out
    }

    /// The parameters of the point of it nearest `p` within `uv` (u0, u1, v0, v1), from `seed` (Newton's method on
    /// the distance's gradient), and the distance.
    pub fn param_of(&self, p: DVec3, seed: DVec2, uv: [f64; 4]) -> (DVec2, f64) {
        // (A sphere's or torus's: by its angles, the turn within the box nearest the seed's (an angle round its axis at
        // a pole or on the axis: the seed's); Newton's lost its way near a sphere's pole, an edge through it not found
        // on it (learning-center-michael's ball tools).)
        let near = |a: f64, to: f64, lo: f64, hi: f64| {
            let base = a + TAU * ((to - a) / TAU).round();
            let off = |c: f64| (lo.min(hi) - c).max(c - lo.max(hi)).max(0.0);
            [base - TAU, base, base + TAU].into_iter().min_by(|&c, &d| off(c).total_cmp(&off(d)).then((c - to).abs().total_cmp(&(d - to).abs()))).unwrap_or(base)
        };
        let clamp = |q: DVec2| DVec2::new(q.x.clamp(uv[0].min(uv[1]), uv[0].max(uv[1])), q.y.clamp(uv[2].min(uv[3]), uv[2].max(uv[3])));
        match *self {
            FreeSurface::Sphere { o, x, y, z, .. } => {
                let w = p - o;
                let (wx, wy, wz) = (w.dot(x), w.dot(y), w.dot(z));
                let rho = (wx * wx + wy * wy).sqrt();
                let u = if rho > 1e-12 * w.length().max(1e-300) { near(wy.atan2(wx), seed.x, uv[0], uv[1]) } else { seed.x };
                let q = clamp(DVec2::new(u, wz.atan2(rho)));
                return (q, (self.at(q.x, q.y) - p).length());
            }
            FreeSurface::Torus { o, x, y, z, major, .. } => {
                let w = p - o;
                let (wx, wy, wz) = (w.dot(x), w.dot(y), w.dot(z));
                let rho = (wx * wx + wy * wy).sqrt();
                let u = if rho > 1e-12 * w.length().max(1e-300) { near(wy.atan2(wx), seed.x, uv[0], uv[1]) } else { seed.x };
                let v = near(wz.atan2(rho - major), seed.y, uv[2], uv[3]);
                let q = clamp(DVec2::new(u, v));
                return (q, (self.at(q.x, q.y) - p).length());
            }
            _ => {}
        }
        let mut q = seed;
        for _ in 0..50 {
            let [s, su, sv, suu, suv, svv] = self.derivs(q.x, q.y);
            let r = s - p;
            let (f, g) = (r.dot(su), r.dot(sv));
            let (a, b, d) = (su.length_squared() + r.dot(suu), su.dot(sv) + r.dot(suv), sv.length_squared() + r.dot(svv));
            let det = a * d - b * b;
            if det.abs() < 1e-300 {
                break;
            }
            let step = DVec2::new((d * f - b * g) / det, (a * g - b * f) / det);
            let next = DVec2::new((q.x - step.x).clamp(uv[0].min(uv[1]), uv[0].max(uv[1])), (q.y - step.y).clamp(uv[2].min(uv[3]), uv[2].max(uv[3])));
            let moved = (next - q).abs();
            q = next;
            if moved.x <= 1e-15 * (1.0 + q.x.abs()) && moved.y <= 1e-15 * (1.0 + q.y.abs()) {
                break;
            }
        }
        (q, (self.at(q.x, q.y) - p).length())
    }
}

/// The box of points within `r` of the axis (o, a) (a unit) and from `h.0` to `h.1` along it.
fn ring_box(o: DVec3, a: DVec3, h: (f64, f64), r: f64) -> (DVec3, DVec3) {
    let (e0, e1) = (o + a * h.0, o + a * h.1);
    let across = DVec3::new((1.0 - a.x * a.x).max(0.0).sqrt(), (1.0 - a.y * a.y).max(0.0).sqrt(), (1.0 - a.z * a.z).max(0.0).sqrt()) * r;
    (e0.min(e1) - across, e0.max(e1) + across)
}

/// The ranges of cos and of sin over [v.0, v.1] (all of [-1, 1] over a turn or an unbounded range).
fn cos_sin_ranges(v: (f64, f64)) -> ((f64, f64), (f64, f64)) {
    if !(v.0.is_finite() && v.1.is_finite()) || v.1 - v.0 >= TAU {
        return ((-1.0, 1.0), (-1.0, 1.0));
    }
    let (mut c, mut s) = ((v.0.cos().min(v.1.cos()), v.0.cos().max(v.1.cos())), (v.0.sin().min(v.1.sin()), v.0.sin().max(v.1.sin())));
    // (The quarter turns inside it: cos and sin at ±1 there.)
    let mut k = (v.0 / std::f64::consts::FRAC_PI_2).ceil();
    while k * std::f64::consts::FRAC_PI_2 <= v.1 {
        match (k as i64).rem_euclid(4) {
            0 => c.1 = 1.0,
            1 => s.1 = 1.0,
            2 => c.0 = -1.0,
            _ => s.0 = -1.0,
        }
        k += 1.0;
    }
    (c, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(s: &FreeSurface, u: f64, v: f64) {
        let d = s.derivs(u, v);
        let h = 1e-6;
        let fd = |f: &dyn Fn(f64, f64) -> DVec3, du: f64, dv: f64| (f(u + du, v + dv) - f(u - du, v - dv)) / (2.0 * h);
        let at = |a: f64, b: f64| s.derivs(a, b)[0];
        let su = |a: f64, b: f64| s.derivs(a, b)[1];
        let sv = |a: f64, b: f64| s.derivs(a, b)[2];
        for (got, want) in [(d[1], fd(&at, h, 0.0)), (d[2], fd(&at, 0.0, h)), (d[3], fd(&su, h, 0.0)), (d[4], fd(&su, 0.0, h)), (d[5], fd(&sv, 0.0, h))] {
            assert!((got - want).length() < 1e-5 * want.length().max(1.0), "{s:?} ({u}, {v}): {got} vs {want}");
        }
    }

    #[test]
    fn derivatives_of_each_kind() {
        let (x, y, z) = (DVec3::X, DVec3::Y, DVec3::Z);
        let line = NurbsCurve { degree: 1, knots: vec![0.0, 0.0, 2.0, 2.0], poles: vec![DVec3::new(3.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 2.0)], weights: Vec::new(), spans: Default::default() };
        for s in [
            FreeSurface::Torus { o: DVec3::new(1.0, 2.0, 3.0), x, y, z, major: 5.0, minor: 1.5 },
            FreeSurface::Torus { o: DVec3::ZERO, x, y: -y, z, major: 5.0, minor: 1.5 },
            FreeSurface::Sphere { o: DVec3::new(1.0, 0.0, 0.0), x, y, z, r: 2.0 },
            FreeSurface::Extrusion { c: line.clone(), d: DVec3::new(0.0, 0.6, 0.8) },
            FreeSurface::Revolution { c: line, o: DVec3::ZERO, a: z },
        ] {
            for (u, v) in [(0.3, 0.4), (2.0, -0.7), (4.0, 1.1)] {
                check(&s, u, v);
            }
        }
    }
}
