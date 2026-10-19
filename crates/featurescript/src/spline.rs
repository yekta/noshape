//! `@evaluateSpline`: points and derivatives of a (rational) B-spline curve
//! given as std's `BSplineCurve` (degree, controlPoints, weights, knots,
//! isPeriodic). Piegl & Tiller, The NURBS Book, A2.1, A2.3 and A4.2.

use crate::interp::{Interp, R};
use crate::value::Value;

fn magnitude(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n, _) => Some(*n),
        Value::Map(..) => v.get("value").as_number(),
        _ => None,
    }
}

fn numbers(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().filter_map(magnitude).collect()).unwrap_or_default()
}

struct Curve {
    degree: usize,
    /// Homogeneous control points (x w, y w, z w, w).
    points: Vec<[f64; 4]>,
    knots: Vec<f64>,
}

impl Curve {
    fn from_value(v: &Value) -> Option<Curve> {
        let degree = v.get("degree").as_number()? as usize;
        let mut cps: Vec<[f64; 3]> = Vec::new();
        for p in v.get("controlPoints").as_array()?.iter() {
            let c = numbers(p);
            cps.push([*c.first()?, *c.get(1)?, c.get(2).copied().unwrap_or(0.0)]);
        }
        let weights = if v.get("isRational").as_bool().unwrap_or(false) { numbers(v.get("weights")) } else { Vec::new() };
        let mut knots = numbers(v.get("knots"));
        let mut points: Vec<[f64; 4]> = cps.iter().enumerate().map(|(i, c)| {
            let w = weights.get(i).copied().unwrap_or(1.0);
            [c[0] * w, c[1] * w, c[2] * w, w]
        }).collect();
        // Periodic curves list each control point once (knots: n + 1 of them):
        // wrap the first `degree` points and extend the knots periodically.
        if v.get("isPeriodic").as_bool().unwrap_or(false) && knots.len() == points.len() + 1 && knots.len() >= 2 {
            let n = points.len();
            let period = knots[n] - knots[0];
            for i in 0..degree {
                points.push(points[i % n]);
            }
            let mut ext: Vec<f64> = (0..degree).map(|i| knots[n - degree + i] - period).collect();
            ext.extend_from_slice(&knots);
            ext.extend((1..=degree).map(|i| knots[i] + period));
            knots = ext;
        }
        (degree >= 1 && !points.is_empty() && knots.len() == points.len() + degree + 1).then_some(Curve { degree, points, knots })
    }

    fn span(&self, u: f64) -> usize {
        let (p, n) = (self.degree, self.points.len() - 1);
        if u >= self.knots[n + 1] {
            return n;
        }
        if u <= self.knots[p] {
            return p;
        }
        let (mut lo, mut hi) = (p, n + 1);
        let mut mid = (lo + hi) / 2;
        while u < self.knots[mid] || u >= self.knots[mid + 1] {
            if u < self.knots[mid] { hi = mid } else { lo = mid }
            mid = (lo + hi) / 2;
        }
        mid
    }

    /// Basis function derivatives `ders[k][j]` at span `i` (A2.3).
    fn basis_ders(&self, i: usize, u: f64, n: usize) -> Vec<Vec<f64>> {
        let p = self.degree;
        let u_k = &self.knots;
        let mut ndu = vec![vec![0.0; p + 1]; p + 1];
        let (mut left, mut right) = (vec![0.0; p + 1], vec![0.0; p + 1]);
        ndu[0][0] = 1.0;
        for j in 1..=p {
            left[j] = u - u_k[i + 1 - j];
            right[j] = u_k[i + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                ndu[j][r] = right[r + 1] + left[j - r];
                let temp = if ndu[j][r] != 0.0 { ndu[r][j - 1] / ndu[j][r] } else { 0.0 };
                ndu[r][j] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            ndu[j][j] = saved;
        }
        let mut ders = vec![vec![0.0; p + 1]; n + 1];
        for j in 0..=p {
            ders[0][j] = ndu[j][p];
        }
        let mut a = vec![vec![0.0; p + 1]; 2];
        for r in 0..=p {
            let (mut s1, mut s2) = (0usize, 1usize);
            a[0][0] = 1.0;
            for k in 1..=n {
                let mut d = 0.0;
                let rk = r as isize - k as isize;
                let pk = p as isize - k as isize;
                if r >= k {
                    a[s2][0] = if ndu[(pk + 1) as usize][rk as usize] != 0.0 { a[s1][0] / ndu[(pk + 1) as usize][rk as usize] } else { 0.0 };
                    d = a[s2][0] * ndu[rk as usize][pk as usize];
                }
                let j1 = if rk >= -1 { 1 } else { (-rk) as usize };
                let j2 = if (r as isize - 1) <= pk { k - 1 } else { p - r };
                for j in j1..=j2 {
                    let idx = (rk + j as isize) as usize;
                    let den = ndu[(pk + 1) as usize][idx];
                    a[s2][j] = if den != 0.0 { (a[s1][j] - a[s1][j - 1]) / den } else { 0.0 };
                    d += a[s2][j] * ndu[idx][pk as usize];
                }
                if r as isize <= pk {
                    let den = ndu[(pk + 1) as usize][r];
                    a[s2][k] = if den != 0.0 { -a[s1][k - 1] / den } else { 0.0 };
                    d += a[s2][k] * ndu[r][pk as usize];
                }
                ders[k][r] = d;
                std::mem::swap(&mut s1, &mut s2);
            }
        }
        // Row k times p! / (p - k)!.
        let mut f = p as f64;
        for (k, row) in ders.iter_mut().enumerate().skip(1) {
            for x in row.iter_mut() {
                *x *= f;
            }
            f *= p as f64 - k as f64;
        }
        ders
    }

    /// Points and derivatives 0..=n at u (A4.2 for the rational quotient).
    fn eval(&self, u: f64, n: usize) -> Vec<[f64; 3]> {
        let p = self.degree;
        let span = self.span(u);
        let nd = self.basis_ders(span, u, n.min(p));
        let mut aw = vec![[0.0; 4]; n + 1];
        for (acc, row) in aw.iter_mut().zip(&nd) {
            for (b, c) in row.iter().zip(&self.points[span - p..=span]) {
                for (a, x) in acc.iter_mut().zip(c) {
                    *a += b * x;
                }
            }
        }
        let mut ck = vec![[0.0; 3]; n + 1];
        for k in 0..=n {
            let mut v = [aw[k][0], aw[k][1], aw[k][2]];
            for i in 1..=k {
                let b = binom(k, i) * aw[i][3];
                for d in 0..3 {
                    v[d] -= b * ck[k - i][d];
                }
            }
            for d in 0..3 {
                ck[k][d] = v[d] / aw[0][3];
            }
        }
        ck
    }
}

fn binom(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

pub fn evaluate_spline(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let def = a.first().cloned().unwrap_or_default();
    let Some(curve) = Curve::from_value(def.get("spline")) else {
        return Err(i.error("@evaluateSpline: Invalid spline"));
    };
    let n = def.get("nDerivatives").as_number().unwrap_or(0.0).clamp(0.0, 10.0) as usize;
    let params = numbers(def.get("parameters"));
    let mut out = vec![Vec::new(); n + 1];
    for u in params {
        for (k, d) in curve.eval(u, n).into_iter().enumerate() {
            out[k].push(Value::array(d.iter().map(|x| Value::num(*x)).collect()));
        }
    }
    Ok(Value::array(out.into_iter().map(Value::array).collect()))
}
