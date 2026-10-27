//! Minimal NURBS curve toolkit (evaluation, derivatives, knot insertion,
//! splitting, reversal, global interpolation and conics).

#![allow(clippy::needless_range_loop)]

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A (possibly rational) B-spline curve in the plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Nurbs {
    pub degree: usize,
    /// Knot vector, `ctrl.len() + degree + 1` entries.
    pub knots: Vec<f64>,
    pub ctrl: Vec<DVec2>,
    /// Optional weights (same length as `ctrl`); `None` = non-rational.
    pub weights: Option<Vec<f64>>,
}

/// Clamped uniform knot vector on `[0, 1]`.
pub fn clamped_uniform_knots(n_ctrl: usize, degree: usize) -> Vec<f64> {
    let p = degree;
    let interior = n_ctrl.saturating_sub(p + 1);
    let mut k = Vec::with_capacity(n_ctrl + p + 1);
    k.extend(std::iter::repeat_n(0.0, p + 1));
    for i in 1..=interior {
        k.push(i as f64 / (interior + 1) as f64);
    }
    k.extend(std::iter::repeat_n(1.0, p + 1));
    k
}

impl Nurbs {
    /// Non-rational clamped uniform B-spline through the given control points.
    pub fn new(degree: usize, ctrl: Vec<DVec2>) -> Self {
        let degree = degree.min(ctrl.len().saturating_sub(1)).max(1);
        let knots = clamped_uniform_knots(ctrl.len(), degree);
        Nurbs { degree, knots, ctrl, weights: None }
    }

    /// Checks structural validity.
    pub fn is_valid(&self) -> bool {
        self.degree >= 1
            && self.ctrl.len() > self.degree
            && self.knots.len() == self.ctrl.len() + self.degree + 1
            && self.knots.windows(2).all(|w| w[1] >= w[0])
            && self.weights.as_ref().is_none_or(|w| w.len() == self.ctrl.len() && w.iter().all(|x| *x > 0.0))
            && self.knots[self.degree] < self.knots[self.ctrl.len()]
    }

    /// Rational quadratic Bézier conic with Onshape-style `rho` (0 < rho < 1).
    /// rho = 0.5 is a parabola, < 0.5 an ellipse, > 0.5 a hyperbola.
    pub fn conic(start: DVec2, control: DVec2, end: DVec2, rho: f64) -> Self {
        let rho = rho.clamp(1e-6, 1.0 - 1e-6);
        let w = rho / (1.0 - rho);
        Nurbs {
            degree: 2,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            ctrl: vec![start, control, end],
            weights: Some(vec![1.0, w, 1.0]),
        }
    }

    #[inline]
    pub fn weight(&self, i: usize) -> f64 {
        self.weights.as_ref().map_or(1.0, |w| w[i])
    }

    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.ctrl.len()])
    }

    pub fn find_span(&self, u: f64) -> usize {
        let n = self.ctrl.len() - 1;
        let p = self.degree;
        let k = &self.knots;
        if u >= k[n + 1] {
            // last non-empty span
            let mut s = n;
            while s > p && k[s] >= k[n + 1] {
                s -= 1;
            }
            return s;
        }
        if u <= k[p] {
            let mut s = p;
            while s < n && k[s + 1] <= k[p] {
                s += 1;
            }
            return s;
        }
        let (mut lo, mut hi) = (p, n + 1);
        let mut mid = (lo + hi) / 2;
        while u < k[mid] || u >= k[mid + 1] {
            if u < k[mid] {
                hi = mid;
            } else {
                lo = mid;
            }
            mid = (lo + hi) / 2;
        }
        mid
    }

    /// Basis functions and derivatives up to `nd` at `u` in span `span`
    /// (The NURBS Book, A2.3). Returns `ders[k][j]`, k in 0..=nd, j in 0..=p.
    pub fn basis_ders(&self, span: usize, u: f64, nd: usize) -> Vec<Vec<f64>> {
        let p = self.degree;
        let kn = &self.knots;
        let mut ndu = vec![vec![0.0; p + 1]; p + 1];
        let mut left = vec![0.0; p + 1];
        let mut right = vec![0.0; p + 1];
        ndu[0][0] = 1.0;
        for j in 1..=p {
            left[j] = u - kn[span + 1 - j];
            right[j] = kn[span + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                ndu[j][r] = right[r + 1] + left[j - r];
                let temp = if ndu[j][r] != 0.0 { ndu[r][j - 1] / ndu[j][r] } else { 0.0 };
                ndu[r][j] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            ndu[j][j] = saved;
        }
        let mut ders = vec![vec![0.0; p + 1]; nd + 1];
        for j in 0..=p {
            ders[0][j] = ndu[j][p];
        }
        let ndc = nd.min(p);
        let mut a = [vec![0.0; p + 1], vec![0.0; p + 1]];
        for r in 0..=p {
            let (mut s1, mut s2) = (0usize, 1usize);
            a[0][0] = 1.0;
            for k in 1..=ndc {
                let mut d = 0.0;
                let rk = r as isize - k as isize;
                let pk = p - k;
                if r >= k {
                    let den = ndu[pk + 1][rk as usize];
                    a[s2][0] = if den != 0.0 { a[s1][0] / den } else { 0.0 };
                    d = a[s2][0] * ndu[rk as usize][pk];
                }
                let j1: usize = if rk >= -1 { 1 } else { (-rk) as usize };
                let j2: usize = if (r as isize - 1) <= pk as isize { k - 1 } else { p - r };
                let mut j = j1;
                while j <= j2 {
                    let idx = (rk + j as isize) as usize;
                    let den = ndu[pk + 1][idx];
                    a[s2][j] = if den != 0.0 { (a[s1][j] - a[s1][j - 1]) / den } else { 0.0 };
                    d += a[s2][j] * ndu[idx][pk];
                    j += 1;
                }
                if r <= pk {
                    let den = ndu[pk + 1][r];
                    a[s2][k] = if den != 0.0 { -a[s1][k - 1] / den } else { 0.0 };
                    d += a[s2][k] * ndu[r][pk];
                }
                ders[k][r] = d;
                std::mem::swap(&mut s1, &mut s2);
            }
        }
        let mut f = p as f64;
        for (k, row) in ders.iter_mut().enumerate().take(ndc + 1).skip(1) {
            for v in row.iter_mut() {
                *v *= f;
            }
            f *= (p - k) as f64;
        }
        ders
    }

    /// [`Nurbs::basis_ders`] for degree ≤ 3 and `nd` ≤ 2 without allocating.
    pub(crate) fn basis_ders3(&self, span: usize, u: f64, nd: usize) -> [[f64; 4]; 3] {
        let p = self.degree;
        debug_assert!(p <= 3 && nd <= 2);
        let kn = &self.knots;
        let mut ndu = [[0.0; 4]; 4];
        let mut left = [0.0; 4];
        let mut right = [0.0; 4];
        ndu[0][0] = 1.0;
        for j in 1..=p {
            left[j] = u - kn[span + 1 - j];
            right[j] = kn[span + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                ndu[j][r] = right[r + 1] + left[j - r];
                let temp = if ndu[j][r] != 0.0 { ndu[r][j - 1] / ndu[j][r] } else { 0.0 };
                ndu[r][j] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            ndu[j][j] = saved;
        }
        let mut ders = [[0.0; 4]; 3];
        for j in 0..=p {
            ders[0][j] = ndu[j][p];
        }
        let ndc = nd.min(p);
        let mut a = [[0.0; 4]; 2];
        for r in 0..=p {
            let (mut s1, mut s2) = (0usize, 1usize);
            a[0][0] = 1.0;
            for k in 1..=ndc {
                let mut d = 0.0;
                let rk = r as isize - k as isize;
                let pk = p - k;
                if r >= k {
                    let den = ndu[pk + 1][rk as usize];
                    a[s2][0] = if den != 0.0 { a[s1][0] / den } else { 0.0 };
                    d = a[s2][0] * ndu[rk as usize][pk];
                }
                let j1: usize = if rk >= -1 { 1 } else { (-rk) as usize };
                let j2: usize = if (r as isize - 1) <= pk as isize { k - 1 } else { p - r };
                let mut j = j1;
                while j <= j2 {
                    let idx = (rk + j as isize) as usize;
                    let den = ndu[pk + 1][idx];
                    a[s2][j] = if den != 0.0 { (a[s1][j] - a[s1][j - 1]) / den } else { 0.0 };
                    d += a[s2][j] * ndu[idx][pk];
                    j += 1;
                }
                if r <= pk {
                    let den = ndu[pk + 1][r];
                    a[s2][k] = if den != 0.0 { -a[s1][k - 1] / den } else { 0.0 };
                    d += a[s2][k] * ndu[r][pk];
                }
                ders[k][r] = d;
                std::mem::swap(&mut s1, &mut s2);
            }
        }
        let mut f = p as f64;
        for (k, row) in ders.iter_mut().enumerate().take(ndc + 1).skip(1) {
            for v in row.iter_mut() {
                *v *= f;
            }
            f *= (p - k) as f64;
        }
        ders
    }

    /// Point and first two derivatives at `u`.
    pub fn eval_ders(&self, u: f64) -> [DVec2; 3] {
        let p = self.degree;
        let span = self.find_span(u);
        let mut a = [DVec2::ZERO; 3];
        let mut w = [0.0; 3];
        // Up to cubics (every sketch curve but imported ones) without allocating.
        let (d3, dn) = if p <= 3 { (self.basis_ders3(span, u, 2), Vec::new()) } else { ([[0.0; 4]; 3], self.basis_ders(span, u, 2)) };
        for k in 0..3 {
            for j in 0..=p {
                let i = span - p + j;
                let wi = self.weight(i);
                let dk = if p <= 3 { d3[k][j] } else { dn[k][j] };
                a[k] += self.ctrl[i] * (dk * wi);
                w[k] += dk * wi;
            }
        }
        if self.weights.is_none() {
            return a;
        }
        let c0 = a[0] / w[0];
        let c1 = (a[1] - c0 * w[1]) / w[0];
        let c2 = (a[2] - c1 * (2.0 * w[1]) - c0 * w[2]) / w[0];
        [c0, c1, c2]
    }

    pub fn point(&self, u: f64) -> DVec2 {
        self.eval_ders(u)[0]
    }

    /// Returns `(span, basis values N_{span-p..=span}(u), first derivatives)`
    /// of the *rational* basis R_i(u) = N_i w_i / W, used for analytic
    /// Jacobians of point-on-spline constraints.
    pub fn rational_basis(&self, u: f64) -> (usize, Vec<f64>, Vec<f64>) {
        let p = self.degree;
        let span = self.find_span(u);
        let d = self.basis_ders(span, u, 1);
        let mut w0 = 0.0;
        let mut w1 = 0.0;
        for j in 0..=p {
            let wi = self.weight(span - p + j);
            w0 += d[0][j] * wi;
            w1 += d[1][j] * wi;
        }
        let mut r0 = vec![0.0; p + 1];
        let mut r1 = vec![0.0; p + 1];
        for j in 0..=p {
            let wi = self.weight(span - p + j);
            r0[j] = d[0][j] * wi / w0;
            r1[j] = (d[1][j] * wi * w0 - d[0][j] * wi * w1) / (w0 * w0);
        }
        (span, r0, r1)
    }

    fn homog(&self) -> Vec<[f64; 3]> {
        self.ctrl
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let w = self.weight(i);
                [p.x * w, p.y * w, w]
            })
            .collect()
    }

    fn from_homog(degree: usize, knots: Vec<f64>, pw: Vec<[f64; 3]>, rational: bool) -> Self {
        let ctrl = pw.iter().map(|q| DVec2::new(q[0] / q[2], q[1] / q[2])).collect();
        let weights = rational.then(|| pw.iter().map(|q| q[2]).collect());
        Nurbs { degree, knots, ctrl, weights }
    }

    /// Inserts knot `u` once (Boehm's algorithm).
    pub fn insert_knot(&self, u: f64) -> Nurbs {
        let p = self.degree;
        let k = self.find_span(u);
        let pw = self.homog();
        let n = pw.len();
        let mut q = Vec::with_capacity(n + 1);
        for (i, pi) in pw.iter().enumerate().take(k - p + 1) {
            let _ = i;
            q.push(*pi);
        }
        for i in (k - p + 1)..=k {
            let den = self.knots[i + p] - self.knots[i];
            let a = if den > 0.0 { (u - self.knots[i]) / den } else { 0.0 };
            let (p0, p1) = (pw[i - 1], pw[i]);
            q.push([
                (1.0 - a) * p0[0] + a * p1[0],
                (1.0 - a) * p0[1] + a * p1[1],
                (1.0 - a) * p0[2] + a * p1[2],
            ]);
        }
        for pi in pw.iter().skip(k) {
            q.push(*pi);
        }
        let mut knots = self.knots.clone();
        knots.insert(k + 1, u);
        Nurbs::from_homog(p, knots, q, self.weights.is_some())
    }

    fn multiplicity(&self, u: f64) -> usize {
        self.knots.iter().filter(|k| (**k - u).abs() <= 1e-14 * (1.0 + u.abs())).count()
    }

    /// Splits at parameter `u` (strictly inside the domain).
    pub fn split(&self, u: f64) -> Option<(Nurbs, Nurbs)> {
        let (a, b) = self.domain();
        let eps = 1e-12 * (b - a);
        if u <= a + eps || u >= b - eps {
            return None;
        }
        let p = self.degree;
        let mut c = self.clone();
        // snap u to an existing knot if extremely close
        let u = c.knots.iter().copied().find(|k| (k - u).abs() <= eps).unwrap_or(u);
        while c.multiplicity(u) < p {
            c = c.insert_knot(u);
        }
        let i0 = c.knots.iter().position(|k| (*k - u).abs() <= 1e-14 * (1.0 + u.abs()))?;
        let pw = c.homog();
        let rational = c.weights.is_some();
        let mut lk: Vec<f64> = c.knots[..i0].to_vec();
        lk.extend(std::iter::repeat_n(u, p + 1));
        let lc = pw[..i0].to_vec();
        let mut rk: Vec<f64> = vec![u; p + 1];
        rk.extend_from_slice(&c.knots[i0 + p..]);
        let rc = pw[i0 - 1..].to_vec();
        Some((Nurbs::from_homog(p, lk, lc, rational), Nurbs::from_homog(p, rk, rc, rational)))
    }

    /// Sub-curve on `[u0, u1]` (u0 < u1).
    pub fn sub_curve(&self, u0: f64, u1: f64) -> Nurbs {
        let (a, b) = self.domain();
        let mut c = self.clone();
        if u0 > a
            && let Some((_, r)) = c.split(u0) {
                c = r;
            }
        if u1 < b
            && let Some((l, _)) = c.split(u1) {
                c = l;
            }
        c
    }

    /// Same curve with reversed direction; parameter `u` maps to `a + b - u`.
    pub fn reversed(&self) -> Nurbs {
        let (a, b) = (self.knots[0], *self.knots.last().unwrap());
        let knots = self.knots.iter().rev().map(|k| a + b - k).collect();
        let mut ctrl = self.ctrl.clone();
        ctrl.reverse();
        let weights = self.weights.as_ref().map(|w| w.iter().rev().copied().collect());
        Nurbs { degree: self.degree, knots, ctrl, weights }
    }

    /// Global interpolation through `pts` with chord-length parameterization
    /// (The NURBS Book, A9.1). Degree is `min(3, n-1)`.
    pub fn interpolate(pts: &[DVec2]) -> Option<Nurbs> {
        if pts.len() < 2 {
            return None;
        }
        FitSystem::new(&chord_params(pts).0, pts, None).solve()
    }

    /// Interpolation through `pts` with tangent handles `h0` (start) and
    /// `h1` (end): like Bezier control points, a handle at a third of the
    /// first / last segment gives the natural-looking end tangent.
    pub fn interpolate_with_handles(pts: &[DVec2], h0: DVec2, h1: DVec2) -> Option<Nurbs> {
        if pts.len() < 2 {
            return None;
        }
        let (d0, d1) = handle_ends(pts, h0, h1);
        Self::interpolate_with_ends(pts, d0, d1)
    }

    /// The seam derivative (chord-length parameter on [0, 1]) of the
    /// periodic cubic spline through `pts`, whose last point repeats the
    /// first: [`Nurbs::interpolate_with_ends`] with it at both ends closes
    /// the curve smoothly (Onshape's closed fit splines are periodic).
    pub fn periodic_end_derivative(pts: &[DVec2]) -> Option<DVec2> {
        let n = pts.len().checked_sub(1)?;
        if n < 3 || (pts[n] - pts[0]).length() > 1e-12 * (1.0 + pts[0].length()) {
            return None;
        }
        let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
        if total <= 1e-300 {
            return None;
        }
        let h: Vec<f64> = (0..n).map(|k| (pts[k + 1] - pts[k]).length() / total).collect();
        if h.iter().any(|x| *x <= 1e-300) {
            return None;
        }
        // Cyclic system for the second derivatives M_k (M_n = M_0).
        let mut a = nalgebra::DMatrix::<f64>::zeros(n, n);
        let mut bx = nalgebra::DVector::<f64>::zeros(n);
        let mut by = nalgebra::DVector::<f64>::zeros(n);
        for k in 0..n {
            let km = (k + n - 1) % n;
            a[(k, km)] += h[km];
            a[(k, k)] += 2.0 * (h[km] + h[k]);
            a[(k, (k + 1) % n)] += h[k];
            let r = (pts[k + 1] - pts[k]) / h[k] - (pts[k] - pts[if k == 0 { n - 1 } else { k - 1 }]) / h[km];
            bx[k] = 6.0 * r.x;
            by[k] = 6.0 * r.y;
        }
        let lu = a.lu();
        let (mx, my) = (lu.solve(&bx)?, lu.solve(&by)?);
        let m0 = DVec2::new(mx[0], my[0]);
        let m1 = DVec2::new(mx[1 % n], my[1 % n]);
        Some((pts[1] - pts[0]) / h[0] - (2.0 * m0 + m1) * (h[0] / 6.0))
    }

    /// Handle positions giving the natural end tangents of [`Nurbs::interpolate`].
    pub fn natural_handles(pts: &[DVec2]) -> Option<(DVec2, DVec2)> {
        let n = pts.len();
        let c = Nurbs::interpolate(pts)?;
        let (a, b) = c.domain();
        let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f64>().max(1e-300);
        let first = (pts[1] - pts[0]).length();
        let last = (pts[n - 1] - pts[n - 2]).length();
        Some((pts[0] + c.eval_ders(a)[1] * (first / (3.0 * total)), pts[n - 1] - c.eval_ders(b)[1] * (last / (3.0 * total))))
    }

    /// Interpolation through `pts` with prescribed end derivatives `d0`,
    /// `d1` (with respect to the chord-length parameter on [0, 1]); cubic
    /// with `n + 2` control points (The NURBS Book, section 9.2.2).
    pub fn interpolate_with_ends(pts: &[DVec2], d0: DVec2, d1: DVec2) -> Option<Nurbs> {
        if pts.len() < 2 {
            return None;
        }
        FitSystem::new(&chord_params(pts).0, pts, Some((d0, d1))).solve()
    }

    /// Least-squares cubic B-spline with `n_ctrl` control points (clamped
    /// uniform knots) approximating `pts` (ordered samples), interpolating
    /// the first and last sample. Parameters start chord-length and are
    /// refined by projecting the samples onto the fit a few times.
    pub fn fit_least_squares(pts: &[DVec2], n_ctrl: usize) -> Option<Nurbs> {
        let m = pts.len();
        let p = 3usize;
        if n_ctrl < p + 1 || m < n_ctrl + 2 {
            return None;
        }
        let mut total = 0.0;
        let mut ub = vec![0.0; m];
        for k in 1..m {
            total += (pts[k] - pts[k - 1]).length();
            ub[k] = total;
        }
        if total <= 1e-300 {
            return None;
        }
        ub.iter_mut().for_each(|u| *u /= total);
        ub[m - 1] = 1.0;
        // Knots spread evenly over a blend of arc length and turning angle, so
        // spans concentrate where the curve bends.
        let mut g = vec![0.0; m];
        let mut turn_total = 0.0;
        let mut turns = vec![0.0; m];
        for k in 1..m - 1 {
            let (d0, d1) = (pts[k] - pts[k - 1], pts[k + 1] - pts[k]);
            turns[k] = d0.perp_dot(d1).atan2(d0.dot(d1)).abs();
            turn_total += turns[k];
        }
        for k in 1..m {
            let lf = (ub[k] - ub[k - 1]).max(0.0);
            let tf = if turn_total > 1e-12 { 0.5 * (turns[k - 1] + turns[k]) / turn_total } else { lf };
            g[k] = g[k - 1] + 0.5 * lf + 0.5 * tf;
        }
        let gt = g[m - 1].max(1e-300);
        let interior = n_ctrl - p - 1;
        let mut knots = vec![0.0; p + 1];
        for i in 1..=interior {
            let target = gt * i as f64 / (interior + 1) as f64;
            let k = g.partition_point(|x| *x < target).clamp(1, m - 1);
            let f = if g[k] > g[k - 1] { (target - g[k - 1]) / (g[k] - g[k - 1]) } else { 0.0 };
            knots.push(ub[k - 1] + (ub[k] - ub[k - 1]) * f);
        }
        knots.extend(std::iter::repeat_n(1.0, p + 1));
        if knots.windows(2).any(|w| w[1] < w[0]) {
            knots = clamped_uniform_knots(n_ctrl, p);
        }
        let mut fit = Nurbs { degree: p, knots: knots.clone(), ctrl: vec![DVec2::ZERO; n_ctrl], weights: None };
        let (p0, pn) = (pts[0], pts[m - 1]);
        for iter in 0..4 {
            // Normal equations for the interior control points.
            let ni = n_ctrl - 2;
            let mut ata = nalgebra::DMatrix::<f64>::zeros(ni, ni);
            let mut atb = nalgebra::DMatrix::<f64>::zeros(ni, 2);
            for (k, &u) in ub.iter().enumerate().take(m - 1).skip(1) {
                let span = fit.find_span(u);
                let d = fit.basis_ders(span, u, 0);
                let mut row = vec![0.0; n_ctrl];
                for j in 0..=p {
                    row[span - p + j] = d[0][j];
                }
                let r = pts[k] - p0 * row[0] - pn * row[n_ctrl - 1];
                for a in 0..ni {
                    let ra = row[a + 1];
                    if ra == 0.0 {
                        continue;
                    }
                    atb[(a, 0)] += ra * r.x;
                    atb[(a, 1)] += ra * r.y;
                    for b in 0..ni {
                        ata[(a, b)] += ra * row[b + 1];
                    }
                }
            }
            let sol = ata.lu().solve(&atb)?;
            fit.ctrl[0] = p0;
            fit.ctrl[n_ctrl - 1] = pn;
            for a in 0..ni {
                fit.ctrl[a + 1] = DVec2::new(sol[(a, 0)], sol[(a, 1)]);
            }
            if iter == 3 {
                break;
            }
            // Parameter correction (Newton step of the foot point).
            for (k, u) in ub.iter_mut().enumerate().take(m - 1).skip(1) {
                let [c, d1, d2] = fit.eval_ders(*u);
                let r = c - pts[k];
                let den = d1.dot(d1) + r.dot(d2);
                if den.abs() > 1e-300 {
                    *u = (*u - r.dot(d1) / den).clamp(0.0, 1.0);
                }
            }
        }
        Some(fit)
    }

    /// Derivatives of an end of a clamped curve expressed through the first
    /// three control points (P0 at the end, P1, P2 inward):
    /// `C'(end) = alpha (P1-P0)`, `C''(end) = b1 (P2-P1) - b0 (P1-P0)`,
    /// where derivatives are taken in the direction pointing *away* from the end.
    pub fn end_coeffs(&self, at_end: bool) -> (f64, f64, f64) {
        let c = if at_end { self.reversed() } else { self.clone() };
        let p = c.degree as f64;
        let u = &c.knots;
        let pd = c.degree;
        let alpha = p / (u[pd + 1] - u[1]);
        if pd < 2 || c.ctrl.len() < 3 {
            return (alpha, 0.0, 0.0);
        }
        // Q0 = alpha (P1-P0), Q1 = p (P2-P1)/(u[p+2]-u[2]); C'' = (p-1)(Q1-Q0)/(u[p+1]-u[2])
        let q1 = p / (u[pd + 2] - u[2]);
        let f = (p - 1.0) / (u[pd + 1] - u[2]);
        (alpha, f * q1, f * alpha)
    }
}

/// The end derivatives (chord-length parameter on [0, 1]) of handles `h0`,
/// `h1` on the fit spline through `pts` (at least 2 points).
pub(crate) fn handle_ends(pts: &[DVec2], h0: DVec2, h1: DVec2) -> (DVec2, DVec2) {
    let n = pts.len();
    let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f64>().max(1e-300);
    let first = (pts[1] - pts[0]).length().max(total * 1e-9);
    let last = (pts[n - 1] - pts[n - 2]).length().max(total * 1e-9);
    ((h0 - pts[0]) * (3.0 * total / first), (pts[n - 1] - h1) * (3.0 * total / last))
}

/// Chord-length parameters of `pts` on [0, 1] (uniform when all points
/// coincide) and the polyline's length.
pub(crate) fn chord_params(pts: &[DVec2]) -> (Vec<f64>, f64) {
    let n = pts.len();
    let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let mut ub = vec![0.0; n];
    for k in 1..n {
        ub[k] = if total <= 1e-300 { k as f64 / (n - 1) as f64 } else { ub[k - 1] + (pts[k] - pts[k - 1]).length() / total };
    }
    ub[n - 1] = 1.0;
    (ub, total)
}

/// The linear system of a fit spline (The NURBS Book, 9.2.1 / 9.2.2):
/// through `pts` at the parameters `ub`, with averaged knots; with `ends`,
/// a cubic with the end derivatives prescribed as well (`n + 2` control
/// points), else degree min(3, n - 1) with `n`. Nonzero only in a band, so
/// it factors in O(n).
pub(crate) struct FitSystem {
    pub degree: usize,
    pub knots: Vec<f64>,
    /// One row per control point: parameter, derivative order, right-hand side.
    pub rows: Vec<(f64, usize, DVec2)>,
}

impl FitSystem {
    pub(crate) fn new(ub: &[f64], pts: &[DVec2], ends: Option<(DVec2, DVec2)>) -> Self {
        let n = pts.len();
        let degree = if ends.is_some() { 3 } else { 3.min(n - 1) };
        let nc = if ends.is_some() { n + 2 } else { n };
        let mut knots = vec![0.0; nc + degree + 1];
        for k in knots.iter_mut().skip(nc) {
            *k = 1.0;
        }
        Self::set_knots(ub, degree, ends.is_some(), &mut knots);
        let rows = match ends {
            None => ub.iter().zip(pts).map(|(&u, &q)| (u, 0, q)).collect(),
            // Rows: position at 0, derivative at 0, interior positions, derivative at 1, position at 1.
            Some((d0, d1)) => {
                let mut rows: Vec<(f64, usize, DVec2)> = vec![(0.0, 0, pts[0]), (0.0, 1, d0)];
                rows.extend((1..n - 1).map(|k| (ub[k], 0, pts[k])));
                rows.extend([(1.0, 1, d1), (1.0, 0, pts[n - 1])]);
                rows
            }
        };
        FitSystem { degree, knots, rows }
    }

    /// The internal knots: averages of the parameters (eq. 9.22).
    pub(crate) fn set_knots(ub: &[f64], p: usize, ends: bool, knots: &mut [f64]) {
        let n = ub.len();
        if ends {
            for j in 0..n.saturating_sub(2) {
                let s: f64 = (j..j + p).map(|i| ub[i.min(n - 1)]).sum();
                knots[p + 1 + j] = s / p as f64;
            }
        } else {
            for j in 1..n - p {
                let s: f64 = ub[j..j + p].iter().sum();
                knots[j + p] = s / p as f64;
            }
        }
    }

    /// The parameter index row `r` sits at (`None`: an end row).
    pub(crate) fn row_param(&self, r: usize, ends: bool) -> Option<usize> {
        let nc = self.rows.len();
        if !ends {
            Some(r)
        } else if r >= 2 && r + 2 < nc {
            Some(r - 1)
        } else {
            None
        }
    }

    /// A curve with this system's knots (control points zero) for basis evaluations.
    pub(crate) fn frame(&self) -> Nurbs {
        Nurbs { degree: self.degree, knots: self.knots.clone(), ctrl: vec![DVec2::ZERO; self.rows.len()], weights: None }
    }

    /// The `der`-th derivative basis at `u`: the first control point index and the p + 1 coefficients.
    /// Entries past the degree are zero.
    pub(crate) fn basis_row(frame: &Nurbs, u: f64, der: usize) -> (usize, [f64; 4]) {
        let span = frame.find_span(u);
        (span - frame.degree, frame.basis_ders3(span, u, der)[der])
    }

    pub(crate) fn factor(&self) -> Option<crate::linalg::BandLu> {
        let nc = self.rows.len();
        let frame = self.frame();
        let mut a = vec![0.0; nc * nc];
        let (mut kl, mut ku) = (0usize, 0usize);
        for (r, &(u, der, _)) in self.rows.iter().enumerate() {
            let (first, c) = Self::basis_row(&frame, u, der);
            for (j, v) in c[..=self.degree].iter().enumerate() {
                let col = first + j;
                a[r * nc + col] = *v;
                if *v != 0.0 {
                    kl = kl.max(r.saturating_sub(col));
                    ku = ku.max(col.saturating_sub(r));
                }
            }
        }
        crate::linalg::BandLu::factor(a, nc, kl, ku)
    }

    /// The interpolating curve, `None` if the system is singular.
    pub(crate) fn solve(&self) -> Option<Nurbs> {
        Some(self.solve_with(&self.factor()?))
    }

    pub(crate) fn solve_with(&self, lu: &crate::linalg::BandLu) -> Nurbs {
        let mut x: Vec<f64> = self.rows.iter().map(|r| r.2.x).collect();
        let mut y: Vec<f64> = self.rows.iter().map(|r| r.2.y).collect();
        lu.solve(&mut x);
        lu.solve(&mut y);
        let ctrl = x.into_iter().zip(y).map(|(x, y)| DVec2::new(x, y)).collect();
        Nurbs { degree: self.degree, knots: self.knots.clone(), ctrl, weights: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Nurbs {
        Nurbs::new(
            3,
            vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(1.0, 2.0),
                DVec2::new(3.0, 3.0),
                DVec2::new(4.0, 0.0),
                DVec2::new(6.0, 1.0),
            ],
        )
    }

    #[test]
    fn endpoints_and_derivs() {
        let c = sample();
        assert!((c.point(0.0) - DVec2::ZERO).length() < 1e-12);
        assert!((c.point(1.0) - DVec2::new(6.0, 1.0)).length() < 1e-12);
        let h = 1e-6;
        for &u in &[0.1, 0.37, 0.5, 0.81] {
            let d = c.eval_ders(u);
            let fd = (c.point(u + h) - c.point(u - h)) / (2.0 * h);
            assert!((d[1] - fd).length() < 1e-6, "{u}");
            let fd2 = (c.eval_ders(u + h)[1] - c.eval_ders(u - h)[1]) / (2.0 * h);
            assert!((d[2] - fd2).length() < 1e-4);
        }
        let (alpha, b1, b0) = c.end_coeffs(false);
        let d = c.eval_ders(0.0);
        let p = &c.ctrl;
        assert!((d[1] - (p[1] - p[0]) * alpha).length() < 1e-9);
        assert!((d[2] - ((p[2] - p[1]) * b1 - (p[1] - p[0]) * b0)).length() < 1e-9);
    }

    #[test]
    fn split_and_reverse() {
        let c = sample();
        let (l, r) = c.split(0.4).unwrap();
        for i in 0..=10 {
            let u = 0.4 * i as f64 / 10.0;
            assert!((l.point(u) - c.point(u)).length() < 1e-10);
            let u2 = 0.4 + 0.6 * i as f64 / 10.0;
            assert!((r.point(u2) - c.point(u2)).length() < 1e-10);
        }
        let rv = c.reversed();
        assert!((rv.point(0.3) - c.point(0.7)).length() < 1e-10);
        let conic = Nurbs::conic(DVec2::new(1.0, 0.0), DVec2::new(1.0, 1.0), DVec2::new(0.0, 1.0), std::f64::consts::FRAC_1_SQRT_2 / (1.0 + std::f64::consts::FRAC_1_SQRT_2));
        // weight = cos(45deg) -> exact quarter circle
        for i in 0..=8 {
            let p = conic.point(i as f64 / 8.0);
            assert!((p.length() - 1.0).abs() < 1e-9);
        }
        let (a, b) = conic.split(0.3).unwrap();
        assert!((a.point(0.2) - conic.point(0.2)).length() < 1e-10);
        assert!((b.point(0.9) - conic.point(0.9)).length() < 1e-10);
    }

    #[test]
    fn interpolation_passes_through_points() {
        let pts = vec![DVec2::new(0.0, 0.0), DVec2::new(1.0, 1.0), DVec2::new(2.0, 0.5), DVec2::new(3.0, 2.0), DVec2::new(4.0, 0.0)];
        let c = Nurbs::interpolate(&pts).unwrap();
        assert!(c.is_valid());
        // every fit point must lie on the curve
        for q in &pts {
            let mut best = f64::MAX;
            for i in 0..=4000 {
                best = best.min((c.point(i as f64 / 4000.0) - *q).length());
            }
            assert!(best < 1e-3, "{best}");
        }
        assert!((c.point(0.0) - pts[0]).length() < 1e-12);
        assert!((c.point(1.0) - pts[4]).length() < 1e-12);
    }
}
