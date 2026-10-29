//! NURBS curves and surfaces (OCCT's B-splines made non-periodic, knots listed with their multiplicities):
//! points and derivatives (Piegl & Tiller, The NURBS Book, A2.3 / A3.2 / A3.6 / A4.2 / A4.4), and a point's
//! parameters (from the nearest sample, then Newton's method).

use glam::{DVec2, DVec3};

/// A curve in 3D (2D ones have z = 0): its degree, knots (each as often as its multiplicity), poles, and weights
/// (empty: polynomial).
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve {
    pub degree: usize,
    pub knots: Vec<f64>,
    pub poles: Vec<DVec3>,
    pub weights: Vec<f64>,
}

/// A surface: degrees, knots, poles row by row (`poles[i * nv + j]`, i along u), weights likewise (empty:
/// polynomial).
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    pub udegree: usize,
    pub vdegree: usize,
    pub uknots: Vec<f64>,
    pub vknots: Vec<f64>,
    pub nu: usize,
    pub nv: usize,
    pub poles: Vec<DVec3>,
    pub weights: Vec<f64>,
}

/// The knot span holding `t` (A2.1): `knots[s] <= t < knots[s + 1]`, the last span at the end.
fn span(knots: &[f64], degree: usize, n: usize, t: f64) -> usize {
    // n poles: spans degree..n-1.
    if t >= knots[n] {
        return n - 1;
    }
    if t <= knots[degree] {
        return degree;
    }
    let (mut lo, mut hi) = (degree, n);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if t < knots[mid] {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

/// The non-zero basis functions at `t` and their derivatives up to `nd` (A2.3): `ders[k][j]` is the k-th
/// derivative of N(s - p + j).
fn basis_derivs(knots: &[f64], s: usize, t: f64, p: usize, nd: usize) -> Vec<Vec<f64>> {
    let mut ndu = vec![vec![0.0; p + 1]; p + 1];
    let (mut left, mut right) = (vec![0.0; p + 1], vec![0.0; p + 1]);
    ndu[0][0] = 1.0;
    for j in 1..=p {
        left[j] = t - knots[s + 1 - j];
        right[j] = knots[s + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            ndu[j][r] = right[r + 1] + left[j - r];
            let tmp = ndu[r][j - 1] / ndu[j][r];
            ndu[r][j] = saved + right[r + 1] * tmp;
            saved = left[j - r] * tmp;
        }
        ndu[j][j] = saved;
    }
    let nd = nd.min(p);
    let mut ders = vec![vec![0.0; p + 1]; nd + 1];
    for j in 0..=p {
        ders[0][j] = ndu[j][p];
    }
    let mut a = vec![vec![0.0; p + 1]; 2];
    for r in 0..=p {
        let (mut s1, mut s2) = (0usize, 1usize);
        a[0][0] = 1.0;
        for k in 1..=nd {
            let mut d = 0.0;
            let (rk, pk) = (r as isize - k as isize, p - k);
            if r >= k {
                a[s2][0] = a[s1][0] / ndu[pk + 1][rk as usize];
                d = a[s2][0] * ndu[rk as usize][pk];
            }
            let j1 = if rk >= -1 { 1 } else { (-rk) as usize };
            let j2 = if (r as isize - 1) <= pk as isize { k - 1 } else { p - r };
            for j in j1..=j2 {
                let rkj = (rk + j as isize) as usize;
                a[s2][j] = (a[s1][j] - a[s1][j - 1]) / ndu[pk + 1][rkj];
                d += a[s2][j] * ndu[rkj][pk];
            }
            if r <= pk {
                a[s2][k] = -a[s1][k - 1] / ndu[pk + 1][r];
                d += a[s2][k] * ndu[r][pk];
            }
            ders[k][r] = d;
            std::mem::swap(&mut s1, &mut s2);
        }
    }
    // Row k times p! / (p - k)!.
    let mut f = p as f64;
    for (k, row) in ders.iter_mut().enumerate().skip(1) {
        for v in row.iter_mut() {
            *v *= f;
        }
        f *= (p - k) as f64;
    }
    ders
}

/// Binomial coefficient.
fn binom(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

impl NurbsCurve {
    /// Its parameter range.
    pub fn range(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.poles.len()])
    }

    /// The point at `t` and its first and second derivatives.
    pub fn derivs(&self, t: f64) -> [DVec3; 3] {
        let (p, n) = (self.degree, self.poles.len());
        let s = span(&self.knots, p, n, t);
        let b = basis_derivs(&self.knots, s, t, p, 2);
        // Homogeneous derivatives (weighted points and weights), then the rational quotient rule (A4.2).
        let mut aw = [DVec3::ZERO; 3];
        let mut w = [0.0; 3];
        for (k, row) in b.iter().enumerate() {
            for (j, &nj) in row.iter().enumerate() {
                let i = s - p + j;
                let wi = if self.weights.is_empty() { 1.0 } else { self.weights[i] };
                aw[k] += self.poles[i] * (nj * wi);
                w[k] += nj * wi;
            }
        }
        let mut c = [DVec3::ZERO; 3];
        for k in 0..3 {
            let mut v = aw[k];
            for i in 1..=k {
                v -= c[k - i] * (binom(k, i) * w[i]);
            }
            c[k] = v / w[0];
        }
        c
    }

    pub fn at(&self, t: f64) -> DVec3 {
        self.derivs(t)[0]
    }

    /// The parameter of the point of it nearest `p` (from 16 samples a span, then Newton's method), and the
    /// distance.
    pub fn param_of(&self, p: DVec3) -> (f64, f64) {
        let (t0, t1) = self.range();
        let n = 16 * (self.poles.len() - self.degree).max(1);
        let mut best = (t0, f64::INFINITY);
        for k in 0..=n {
            let t = t0 + (t1 - t0) * k as f64 / n as f64;
            let d = (self.at(t) - p).length_squared();
            if d < best.1 {
                best = (t, d);
            }
        }
        let mut t = best.0;
        for _ in 0..50 {
            let [c, d1, d2] = self.derivs(t);
            let q = c - p;
            let g = q.dot(d1);
            let gp = d1.length_squared() + q.dot(d2);
            if gp.abs() < 1e-300 {
                break;
            }
            let nt = (t - g / gp).clamp(t0, t1);
            if (nt - t).abs() <= 1e-15 * (1.0 + t.abs()) {
                t = nt;
                break;
            }
            t = nt;
        }
        (t, (self.at(t) - p).length())
    }
}

impl NurbsSurface {
    /// Its parameter ranges, u then v.
    pub fn range(&self) -> ((f64, f64), (f64, f64)) {
        ((self.uknots[self.udegree], self.uknots[self.nu]), (self.vknots[self.vdegree], self.vknots[self.nv]))
    }

    /// The point at (u, v) and its derivatives: [S, Su, Sv, Suu, Suv, Svv].
    pub fn derivs(&self, u: f64, v: f64) -> [DVec3; 6] {
        let (p, q) = (self.udegree, self.vdegree);
        let (su, sv) = (span(&self.uknots, p, self.nu, u), span(&self.vknots, q, self.nv, v));
        let bu = basis_derivs(&self.uknots, su, u, p, 2);
        let bv = basis_derivs(&self.vknots, sv, v, q, 2);
        // Homogeneous derivatives SKL (k along u, l along v, k + l <= 2), then the quotient rule (A4.4).
        let mut aw = [[DVec3::ZERO; 3]; 3];
        let mut w = [[0.0; 3]; 3];
        for (k, ru) in bu.iter().enumerate() {
            for (l, rv) in bv.iter().enumerate() {
                if k + l > 2 {
                    continue;
                }
                for (a, &na) in ru.iter().enumerate() {
                    let i = su - p + a;
                    for (b, &nb) in rv.iter().enumerate() {
                        let j = sv - q + b;
                        let idx = i * self.nv + j;
                        let wi = if self.weights.is_empty() { 1.0 } else { self.weights[idx] };
                        let f = na * nb * wi;
                        aw[k][l] += self.poles[idx] * f;
                        w[k][l] += f;
                    }
                }
            }
        }
        let mut s = [[DVec3::ZERO; 3]; 3];
        for k in 0..=2 {
            for l in 0..=(2 - k) {
                let mut v = aw[k][l];
                for j in 1..=l {
                    v -= s[k][l - j] * (binom(l, j) * w[0][j]);
                }
                for i in 1..=k {
                    v -= s[k - i][l] * (binom(k, i) * w[i][0]);
                    let mut v2 = DVec3::ZERO;
                    for j in 1..=l {
                        v2 += s[k - i][l - j] * (binom(l, j) * w[i][j]);
                    }
                    v -= v2 * binom(k, i);
                }
                s[k][l] = v / w[0][0];
            }
        }
        [s[0][0], s[1][0], s[0][1], s[2][0], s[1][1], s[0][2]]
    }

    pub fn at(&self, u: f64, v: f64) -> DVec3 {
        self.derivs(u, v)[0]
    }

    /// The parameters of the point of it nearest `p` (from a grid of samples, 8 a span each way, or near
    /// `seed`; then Newton's method on the distance's gradient), and the distance.
    pub fn param_of(&self, p: DVec3, seed: Option<DVec2>) -> (DVec2, f64) {
        let ((u0, u1), (v0, v1)) = self.range();
        let mut best = (DVec2::new(u0, v0), f64::INFINITY);
        if let Some(s) = seed {
            best = (s, (self.at(s.x, s.y) - p).length_squared());
        } else {
            let mu = 8 * (self.nu - self.udegree).max(1);
            let mv = 8 * (self.nv - self.vdegree).max(1);
            for i in 0..=mu {
                let u = u0 + (u1 - u0) * i as f64 / mu as f64;
                for j in 0..=mv {
                    let v = v0 + (v1 - v0) * j as f64 / mv as f64;
                    let d = (self.at(u, v) - p).length_squared();
                    if d < best.1 {
                        best = (DVec2::new(u, v), d);
                    }
                }
            }
        }
        let mut uv = best.0;
        for _ in 0..50 {
            let [s, su, sv, suu, suv, svv] = self.derivs(uv.x, uv.y);
            let r = s - p;
            let (f, g) = (r.dot(su), r.dot(sv));
            let (a, b, d) = (su.length_squared() + r.dot(suu), su.dot(sv) + r.dot(suv), sv.length_squared() + r.dot(svv));
            let det = a * d - b * b;
            if det.abs() < 1e-300 {
                break;
            }
            let step = DVec2::new((d * f - b * g) / det, (a * g - b * f) / det);
            let next = DVec2::new((uv.x - step.x).clamp(u0, u1), (uv.y - step.y).clamp(v0, v1));
            let moved = (next - uv).abs();
            uv = next;
            if moved.x <= 1e-15 * (1.0 + uv.x.abs()) && moved.y <= 1e-15 * (1.0 + uv.y.abs()) {
                break;
            }
        }
        (uv, (self.at(uv.x, uv.y) - p).length())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_1_SQRT_2;

    /// A quarter circle of radius 2 as a rational quadratic.
    fn quarter() -> NurbsCurve {
        NurbsCurve {
            degree: 2,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            poles: vec![DVec3::new(2.0, 0.0, 0.0), DVec3::new(2.0, 2.0, 0.0), DVec3::new(0.0, 2.0, 0.0)],
            weights: vec![1.0, FRAC_1_SQRT_2, 1.0],
        }
    }

    #[test]
    fn a_rational_quarter_circle() {
        let c = quarter();
        for k in 0..=10 {
            let t = k as f64 / 10.0;
            let [p, d1, d2] = c.derivs(t);
            assert!((p.length() - 2.0).abs() < 1e-14, "{t}: {p}");
            // Tangent along the circle, second derivative's normal part towards the centre.
            assert!(p.dot(d1).abs() < 1e-12, "{t}");
            let h = 1e-6;
            let fd = (c.at((t + h).min(1.0)) - c.at((t - h).max(0.0))) / ((t + h).min(1.0) - (t - h).max(0.0));
            assert!((fd - d1).length() < 1e-6 * d1.length(), "{t}: {fd} vs {d1}");
            let fd2 = (c.derivs((t + h).min(1.0))[1] - c.derivs((t - h).max(0.0))[1]) / ((t + h).min(1.0) - (t - h).max(0.0));
            assert!((fd2 - d2).length() < 1e-5 * d2.length().max(1.0), "{t}: {fd2} vs {d2}");
        }
        let (t, d) = c.param_of(DVec3::new(3.0, 3.0, 0.0));
        assert!(d - (18f64.sqrt() - 2.0) < 1e-12 && (t - 0.5).abs() < 1e-9, "{t} {d}");
    }

    #[test]
    fn a_rational_quarter_cylinder() {
        // u: the quarter circle of radius 2, v: height 0..3.
        let c = quarter();
        let mut poles = Vec::new();
        let mut weights = Vec::new();
        for i in 0..3 {
            for z in [0.0, 3.0] {
                poles.push(c.poles[i] + DVec3::Z * z);
                weights.push(c.weights[i]);
            }
        }
        let s = NurbsSurface { udegree: 2, vdegree: 1, uknots: c.knots.clone(), vknots: vec![0.0, 0.0, 1.0, 1.0], nu: 3, nv: 2, poles, weights };
        let h = 1e-5;
        for (u, v) in [(0.1, 0.2), (0.5, 0.5), (0.9, 0.95)] {
            let d = s.derivs(u, v);
            assert!((d[0].truncate().length() - 2.0).abs() < 1e-14 && (d[0].z - 3.0 * v).abs() < 1e-14);
            let fd = |f: &dyn Fn(f64, f64) -> DVec3, du: f64, dv: f64| (f(u + du, v + dv) - f(u - du, v - dv)) / (2.0 * h);
            let at = |a: f64, b: f64| s.at(a, b);
            let su = |a: f64, b: f64| s.derivs(a, b)[1];
            let sv = |a: f64, b: f64| s.derivs(a, b)[2];
            for (got, want) in [(d[1], fd(&at, h, 0.0)), (d[2], fd(&at, 0.0, h)), (d[3], fd(&su, h, 0.0)), (d[4], fd(&su, 0.0, h)), (d[5], fd(&sv, 0.0, h))] {
                assert!((got - want).length() < 1e-5 * want.length().max(1.0), "({u}, {v}): {got} vs {want}");
            }
        }
    }

    #[test]
    fn a_bicubic_patch_and_its_points() {
        // z = x y over [0, 3]², as a bicubic with two spans each way (exact: bilinear raised).
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.5, 3.0, 3.0, 3.0, 3.0];
        let greville = |i: usize| (knots[i + 1] + knots[i + 2] + knots[i + 3]) / 3.0;
        let n = 5;
        let mut poles = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let (x, y) = (greville(i), greville(j));
                poles.push(DVec3::new(x, y, x * y));
            }
        }
        let s = NurbsSurface { udegree: 3, vdegree: 3, uknots: knots.clone(), vknots: knots, nu: n, nv: n, poles, weights: Vec::new() };
        for (u, v) in [(0.0, 0.0), (0.7, 2.2), (1.5, 1.5), (3.0, 0.4), (2.9, 3.0)] {
            let [p, su, sv, suu, suv, svv] = s.derivs(u, v);
            assert!((p - DVec3::new(u, v, u * v)).length() < 1e-13, "{u} {v}: {p}");
            assert!((su - DVec3::new(1.0, 0.0, v)).length() < 1e-12 && (sv - DVec3::new(0.0, 1.0, u)).length() < 1e-12);
            assert!(suu.length() < 1e-11 && (suv - DVec3::Z).length() < 1e-11 && svv.length() < 1e-11, "{suu} {suv} {svv}");
        }
        let q = DVec3::new(1.2, 0.8, 0.96 + 0.3);
        let (uv, d) = s.param_of(q, None);
        let foot = s.at(uv.x, uv.y);
        let [_, su, sv, ..] = s.derivs(uv.x, uv.y);
        assert!((q - foot).dot(su).abs() < 1e-12 && (q - foot).dot(sv).abs() < 1e-12 && (d - (q - foot).length()).abs() < 1e-15);
    }
}
