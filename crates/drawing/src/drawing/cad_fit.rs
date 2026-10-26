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
    if TAU - w < 1e-12 { 0.0 } else { w }
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
