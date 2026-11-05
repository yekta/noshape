//! Exact curves back from sampled polylines for DXF / DWG export (live
//! Onshape writes view edges and circles as CIRCLE / ARC / ELLIPSE, not
//! polylines): the sheet's projected edges are points on their curves, so a
//! least-squares circle or ellipse that passes through every point (within
//! a tight tolerance) is the curve itself.

/// A curve a polyline is exactly on.
#[derive(Clone, Debug, PartialEq)]
pub enum FitCurve {
    Circle { center: [f64; 2], radius: f64 },
    /// Counter-clockwise from `start` to `end` (radians).
    Arc { center: [f64; 2], radius: f64, start: f64, end: f64 },
    /// `major`: the major axis end relative to the center; `ratio` minor /
    /// major; parameters counter-clockwise from `start` to `end` (a full
    /// ellipse is 0..2π).
    Ellipse { center: [f64; 2], major: [f64; 2], ratio: f64, start: f64, end: f64 },
}

const TAU: f64 = std::f64::consts::TAU;

/// Solve the n×n system `a x = b` (Gaussian elimination, partial pivoting).
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        let (pivot_row, pivot_b) = (a[c].clone(), b[c]);
        for (row, br) in a.iter_mut().zip(b.iter_mut()).skip(c + 1) {
            let f = row[c] / pivot_row[c];
            for (v, pv) in row.iter_mut().zip(&pivot_row).skip(c) {
                *v -= f * pv;
            }
            *br -= f * pivot_b;
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = a[r].iter().zip(&x).skip(r + 1).map(|(v, xv)| v * xv).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// Least squares: minimise |M x - y| (normal equations).
fn least_squares(rows: &[Vec<f64>], y: &[f64]) -> Option<Vec<f64>> {
    let n = rows.first()?.len();
    let mut a = vec![vec![0.0; n]; n];
    let mut b = vec![0.0; n];
    for (r, yi) in rows.iter().zip(y) {
        for ((ai, bi), ri) in a.iter_mut().zip(b.iter_mut()).zip(r) {
            *bi += ri * yi;
            for (aij, rj) in ai.iter_mut().zip(r) {
                *aij += ri * rj;
            }
        }
    }
    solve(a, b)
}

/// An angle in [0, 2π), with round-off just below 0 (or 2π) giving 0.
fn wrap(a: f64) -> f64 {
    let w = a.rem_euclid(TAU);
    // A fitted angle a hair short of a full turn is 0 (live writes 0, not 360: knob-tim's Front view arcs).
    if TAU - w < 1e-6 { 0.0 } else { w }
}

/// The signed angle steps along the points about `c`, if they all turn the
/// same way (a single arc, less than a full turn plus a step).
fn sweep(points: &[[f64; 2]], c: [f64; 2], param: impl Fn([f64; 2]) -> f64) -> Option<(f64, f64)> {
    let angles: Vec<f64> = points.iter().map(|p| param([p[0] - c[0], p[1] - c[1]])).collect();
    let mut total = 0.0;
    let mut sign = 0.0;
    for w in angles.windows(2) {
        let mut d = w[1] - w[0];
        while d > std::f64::consts::PI {
            d -= TAU;
        }
        while d < -std::f64::consts::PI {
            d += TAU;
        }
        if d.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = d.signum();
        } else if d.signum() != sign {
            return None;
        }
        total += d;
    }
    (sign != 0.0 && total.abs() <= TAU + 1e-6).then_some((angles[0], total))
}

/// The circle, arc or ellipse a polyline lies on, if it does to within
/// `tol` (sheet mm). Needs at least 6 distinct points.
/// A clamped B-spline's degree (3, or less with fewer points) and knots
/// when `points` are its poles: chord-length parameters averaged (the
/// NURBS Book 9.8), so the curve follows the points and ends on the first
/// and last.
pub fn clamped_knots(points: &[[f64; 2]]) -> (usize, Vec<f64>) {
    let n = points.len();
    let p = n.saturating_sub(1).clamp(1, 3);
    let mut t = vec![0.0; n];
    for i in 1..n {
        t[i] = t[i - 1] + (points[i][0] - points[i - 1][0]).hypot(points[i][1] - points[i - 1][1]);
    }
    let total = t[n - 1].max(1e-300);
    let mut knots = vec![0.0; p + 1];
    for j in 1..n.saturating_sub(p) {
        knots.push(t[j..j + p].iter().sum::<f64>() / (p as f64 * total));
    }
    knots.extend(std::iter::repeat_n(1.0, p + 1));
    (p, knots)
}

/// The poles of the degree-`p` B-spline on `knots` ([`clamped_knots`]) through `points` at their chord-length
/// parameters (global interpolation): a model edge's projected points lie on it, so its SPLINE runs through them,
/// not inside its polygon (live 1857's knob silhouette: poles at the points cut a 5-point piece 0.1 mm short).
/// The points themselves when the system can't be solved.
pub fn interpolating_poles(points: &[[f64; 2]], p: usize, knots: &[f64]) -> Vec<[f64; 2]> {
    let n = points.len();
    if n <= p + 1 || knots.len() != n + p + 1 {
        return points.to_vec();
    }
    let mut t = vec![0.0; n];
    for i in 1..n {
        t[i] = t[i - 1] + (points[i][0] - points[i - 1][0]).hypot(points[i][1] - points[i - 1][1]);
    }
    let total = t[n - 1];
    if total <= 0.0 {
        return points.to_vec();
    }
    for x in t.iter_mut() {
        *x /= total;
    }
    // Row k: the p + 1 basis functions nonzero at t[k], from column `span - p`.
    let basis = |u: f64| -> (usize, Vec<f64>) {
        let span = if u >= knots[n] { n - 1 } else { (p..n).rfind(|&i| knots[i] <= u).unwrap_or(p) };
        let mut nb = vec![0.0; p + 1];
        let (mut left, mut right) = (vec![0.0; p + 1], vec![0.0; p + 1]);
        nb[0] = 1.0;
        for j in 1..=p {
            left[j] = u - knots[span + 1 - j];
            right[j] = knots[span + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                let den = right[r + 1] + left[j - r];
                let temp = if den.abs() < 1e-300 { 0.0 } else { nb[r] / den };
                nb[r] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            nb[j] = saved;
        }
        (span - p, nb)
    };
    // The banded system (no pivoting: the collocation matrix is totally positive).
    let mut a = vec![vec![0.0; n]; n];
    for (k, &u) in t.iter().enumerate() {
        let (first, nb) = basis(u);
        for (j, v) in nb.into_iter().enumerate() {
            a[k][first + j] = v;
        }
    }
    let mut b: Vec<[f64; 2]> = points.to_vec();
    let band = p + 1;
    for col in 0..n {
        let piv = a[col][col];
        if piv.abs() < 1e-14 {
            return points.to_vec();
        }
        for row in col + 1..(col + band + 1).min(n) {
            let f = a[row][col] / piv;
            if f == 0.0 {
                continue;
            }
            let end = (col + band + 1).min(n);
            let (upper, lower) = a.split_at_mut(row);
            for (x, y) in lower[0][col..end].iter_mut().zip(&upper[col][col..end]) {
                *x -= f * y;
            }
            b[row] = [b[row][0] - f * b[col][0], b[row][1] - f * b[col][1]];
        }
    }
    let mut x = vec![[0.0; 2]; n];
    for row in (0..n).rev() {
        let mut acc = b[row];
        for c in row + 1..(row + band + 1).min(n) {
            acc = [acc[0] - a[row][c] * x[c][0], acc[1] - a[row][c] * x[c][1]];
        }
        x[row] = [acc[0] / a[row][row], acc[1] / a[row][row]];
    }
    if x.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) {
        return points.to_vec();
    }
    x
}

pub fn fit_curve(points: &[[f64; 2]], closed: bool, tol: f64) -> Option<FitCurve> {
    let mut pts: Vec<[f64; 2]> = points.to_vec();
    pts.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-12);
    let first_last = pts.len() > 2 && (pts[0][0] - pts[pts.len() - 1][0]).hypot(pts[0][1] - pts[pts.len() - 1][1]) < tol;
    let closed = closed || first_last;
    if pts.len() < 6 {
        return None;
    }
    // Translate to the centroid for conditioning.
    let n = pts.len() as f64;
    let o = pts.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    let q: Vec<[f64; 2]> = pts.iter().map(|p| [p[0] - o[0], p[1] - o[1]]).collect();
    let size = q.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max);
    if size < 1e-9 {
        return None;
    }
    // Circle: x² + y² + D x + E y + F = 0.
    let rows: Vec<Vec<f64>> = q.iter().map(|p| vec![p[0], p[1], 1.0]).collect();
    let y: Vec<f64> = q.iter().map(|p| -(p[0] * p[0] + p[1] * p[1])).collect();
    if let Some(s) = least_squares(&rows, &y) {
        let c = [-s[0] / 2.0, -s[1] / 2.0];
        let r2 = c[0] * c[0] + c[1] * c[1] - s[2];
        if r2 > 0.0 {
            let r = r2.sqrt();
            // A circle much bigger than the points is a near-straight line.
            if r < size * 50.0 && q.iter().all(|p| ((p[0] - c[0]).hypot(p[1] - c[1]) - r).abs() < tol) {
                let center = [c[0] + o[0], c[1] + o[1]];
                if closed {
                    return Some(FitCurve::Circle { center, radius: r });
                }
                let (a0, total) = sweep(&q, c, |d| d[1].atan2(d[0]))?;
                let (start, end) = if total > 0.0 { (a0, a0 + total) } else { (a0 + total, a0) };
                return Some(FitCurve::Arc { center, radius: r, start: wrap(start), end: wrap(end) });
            }
        }
    }
    // Ellipse: x² + B xy + C y² + D x + E y + F = 0.
    let rows: Vec<Vec<f64>> = q.iter().map(|p| vec![p[0] * p[1], p[1] * p[1], p[0], p[1], 1.0]).collect();
    let y: Vec<f64> = q.iter().map(|p| -p[0] * p[0]).collect();
    let s = least_squares(&rows, &y)?;
    let (a, b, c, d, e, f) = (1.0, s[0], s[1], s[2], s[3], s[4]);
    if b * b - 4.0 * a * c >= 0.0 {
        return None;
    }
    // Center: the gradient is zero.
    let det = 4.0 * a * c - b * b;
    let cx = (b * e - 2.0 * c * d) / det;
    let cy = (b * d - 2.0 * a * e) / det;
    // Axes: eigen of [[a, b/2], [b/2, c]], scaled by the value at the center.
    let f0 = a * cx * cx + b * cx * cy + c * cy * cy + d * cx + e * cy + f;
    let tr = a + c;
    let disc = ((a - c) * (a - c) + b * b).sqrt();
    let (l1, l2) = ((tr - disc) / 2.0, (tr + disc) / 2.0);
    if l1 <= 0.0 || l2 <= 0.0 || f0 >= 0.0 {
        return None;
    }
    let (ra, rb) = ((-f0 / l1).sqrt(), (-f0 / l2).sqrt());
    // Eigenvector of the smaller eigenvalue: the major axis direction.
    let dir = if b.abs() > 1e-15 { [b / 2.0, l1 - a] } else if a <= c { [1.0, 0.0] } else { [0.0, 1.0] };
    let dl = dir[0].hypot(dir[1]);
    let u = [dir[0] / dl, dir[1] / dl];
    let v = [-u[1], u[0]];
    let local = |p: [f64; 2]| {
        let d = [p[0] - cx, p[1] - cy];
        [(d[0] * u[0] + d[1] * u[1]) / ra, (d[0] * v[0] + d[1] * v[1]) / rb]
    };
    // Geometric check: every point on the fitted ellipse.
    if !q.iter().all(|p| {
        let l = local(*p);
        let t = l[1].atan2(l[0]);
        let on = [cx + u[0] * ra * t.cos() + v[0] * rb * t.sin(), cy + u[1] * ra * t.cos() + v[1] * rb * t.sin()];
        (on[0] - p[0]).hypot(on[1] - p[1]) < tol
    }) {
        return None;
    }
    let center = [cx + o[0], cy + o[1]];
    let major = [u[0] * ra, u[1] * ra];
    let ratio = rb / ra;
    if closed {
        return Some(FitCurve::Ellipse { center, major, ratio, start: 0.0, end: TAU });
    }
    let (t0, total) = sweep(&q, [cx, cy], |d| {
        let l = [(d[0] * u[0] + d[1] * u[1]) / ra, (d[0] * v[0] + d[1] * v[1]) / rb];
        l[1].atan2(l[0])
    })?;
    let (start, end) = if total > 0.0 { (t0, t0 + total) } else { (t0 + total, t0) };
    Some(FitCurve::Ellipse { center, major, ratio, start: wrap(start), end: wrap(end) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(c: [f64; 2], a: f64, b: f64, rot: f64, t0: f64, t1: f64, n: usize) -> Vec<[f64; 2]> {
        (0..=n)
            .map(|i| {
                let t = t0 + (t1 - t0) * i as f64 / n as f64;
                let (x, y) = (a * t.cos(), b * t.sin());
                [c[0] + x * rot.cos() - y * rot.sin(), c[1] + x * rot.sin() + y * rot.cos()]
            })
            .collect()
    }

    /// A cubic through points on a circle: it passes through them and between them stays on the circle.
    #[test]
    fn interpolating_poles_run_through_the_points() {
        let pts: Vec<[f64; 2]> = (0..9).map(|i| {
            let t = i as f64 / 8.0 * std::f64::consts::FRAC_PI_2;
            [10.0 * t.cos(), 10.0 * t.sin()]
        }).collect();
        let (p, knots) = clamped_knots(&pts);
        let poles = interpolating_poles(&pts, p, &knots);
        assert_eq!(poles.len(), pts.len());
        assert!((poles[0][0] - 10.0).abs() < 1e-12 && (poles[8][1] - 10.0).abs() < 1e-12);
        // De Boor at the middle point's parameter: back on the circle.
        let eval = |u: f64| -> [f64; 2] {
            let n = poles.len();
            let k = (p..n).rfind(|&i| knots[i] <= u).unwrap_or(p).min(n - 1);
            let mut d: Vec<[f64; 2]> = (0..=p).map(|j| poles[j + k - p]).collect();
            for r in 1..=p {
                for j in (r..=p).rev() {
                    let i = j + k - p;
                    let den = knots[i + p - r + 1] - knots[i];
                    let a = if den == 0.0 { 0.0 } else { (u - knots[i]) / den };
                    d[j] = [(1.0 - a) * d[j - 1][0] + a * d[j][0], (1.0 - a) * d[j - 1][1] + a * d[j][1]];
                }
            }
            d[p]
        };
        for u in [0.25, 0.5, 0.61] {
            let q = eval(u);
            assert!((q[0].hypot(q[1]) - 10.0).abs() < 1e-3, "{u}: {q:?}");
        }
    }

    #[test]
    fn circles_arcs_and_ellipses_come_back() {
        let c = fit_curve(&ring([10.0, 20.0], 5.0, 5.0, 0.0, 0.0, TAU, 48), true, 1e-6).unwrap();
        assert!(matches!(c, FitCurve::Circle { center, radius } if (center[0] - 10.0).abs() < 1e-9 && (radius - 5.0).abs() < 1e-9));
        // A clockwise quarter arc comes back counter-clockwise.
        let mut pts = ring([0.0, 0.0], 3.0, 3.0, 0.0, 0.0, std::f64::consts::FRAC_PI_2, 12);
        pts.reverse();
        match fit_curve(&pts, false, 1e-6).unwrap() {
            FitCurve::Arc { radius, start, end, .. } => assert!((radius - 3.0).abs() < 1e-9 && start.abs() < 1e-9 && (end - std::f64::consts::FRAC_PI_2).abs() < 1e-9),
            other => panic!("{other:?}"),
        }
        // Round-off just below 0 (Windows' atan2) is 0, not 2π.
        assert!(wrap(-1e-16) == 0.0 && wrap(TAU) == 0.0 && (wrap(-0.5) - (TAU - 0.5)).abs() < 1e-15);
        // A tilted ellipse (an isometric circle).
        match fit_curve(&ring([1.0, 2.0], 6.0, 3.5, 0.5, 0.0, TAU, 64), true, 1e-6).unwrap() {
            FitCurve::Ellipse { major, ratio, .. } => {
                assert!((major[0].hypot(major[1]) - 6.0).abs() < 1e-6 && (ratio - 3.5 / 6.0).abs() < 1e-6);
                assert!((major[1].atan2(major[0]).rem_euclid(std::f64::consts::PI) - 0.5).abs() < 1e-6);
            }
            other => panic!("{other:?}"),
        }
        // Lines and rectangles stay polylines.
        let line: Vec<[f64; 2]> = (0..10).map(|i| [i as f64, 2.0 * i as f64]).collect();
        assert!(fit_curve(&line, false, 1e-6).is_none());
        let rect = [[0.0, 0.0], [4.0, 0.0], [4.0, 0.0], [4.0, 2.0], [2.0, 2.0], [0.0, 2.0], [0.0, 1.0], [0.0, 0.0]];
        assert!(fit_curve(&rect, true, 1e-6).is_none());
    }
}
