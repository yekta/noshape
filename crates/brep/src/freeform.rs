//! Free-form faces' surfaces as the Boolean evaluates them, in OCCT's own parameters: NURBS, tori, spheres, a
//! curve swept along a direction or revolved about an axis.

use crate::nurbs::{NurbsCurve, NurbsSurface};
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

    pub fn at(&self, u: f64, v: f64) -> DVec3 {
        match self {
            FreeSurface::Nurbs(s) => s.at(u, v),
            _ => self.derivs(u, v)[0],
        }
    }

    /// The parameters of the point of it nearest `p` within `uv` (u0, u1, v0, v1), from `seed` (Newton's method on
    /// the distance's gradient), and the distance.
    pub fn param_of(&self, p: DVec3, seed: DVec2, uv: [f64; 4]) -> (DVec2, f64) {
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
            let next = DVec2::new((q.x - step.x).clamp(uv[0], uv[1]), (q.y - step.y).clamp(uv[2], uv[3]));
            let moved = (next - q).abs();
            q = next;
            if moved.x <= 1e-15 * (1.0 + q.x.abs()) && moved.y <= 1e-15 * (1.0 + q.y.abs()) {
                break;
            }
        }
        (q, (self.at(q.x, q.y) - p).length())
    }
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
