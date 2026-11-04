//! The corner face of a fillet R running round a face's corner that is a smaller round Rc (a block's top loop
//! filleted over its vertical corner fillets): Parasolid's rolling-ball blend of radius Rc between the two R
//! cylinders (Onshape G24 / G25; kernel cpp/vertex_blend.cpp makes the same face after OCCT's fillet). The ball's
//! centre runs along the two cylinders' axes' intersection offset in by R - Rc, an ellipse (the spine); each section
//! is the circle arc of radius Rc between the ball's contact points on the two cylinders; the face's sides are
//! ellipses on the cylinders, its third edge the Rc cylinder's arc, and it closes in a point on the face.

use crate::nurbs::NurbsSurface;
use glam::{DVec3, DVec4};
use std::f64::consts::FRAC_PI_2;

/// The corner: the R cylinders' axes meet at `q`, along `d1` / `d2`; the Rc cylinder's axis is `l0` from `q` along
/// `b` (in the axes' plane); `n` is square to that plane, towards the face the loop bounds.
#[derive(Clone, Copy, Debug)]
pub struct RollCorner {
    pub q: DVec3,
    pub b: DVec3,
    pub n: DVec3,
    pub d1: DVec3,
    pub d2: DVec3,
    pub l0: f64,
    pub r: f64,
    pub rc: f64,
}

/// A section: its ends on cylinder 1 (`a`) and 2 (`c`), the arc's middle, its middle control point and weight.
struct Section {
    a: DVec3,
    c: DVec3,
    mid: DVec3,
    m: DVec3,
    w: f64,
}

impl RollCorner {
    /// The ball's centre at spine angle `th` (0: on the Rc cylinder's axis, π/2: under the face's corner).
    pub fn spine(&self, th: f64) -> DVec3 {
        self.q + self.b * (self.l0 * th.cos()) + self.n * ((self.r - self.rc) * th.sin())
    }

    /// The unit normal of the cylinder along `d` through `q` at the ball's centre `s` (from its axis).
    fn contact(&self, d: DVec3, s: DVec3) -> DVec3 {
        let x = s - self.q;
        (x - d * x.dot(d)).normalize()
    }

    fn section(&self, th: f64) -> Section {
        let s = self.spine(th);
        let (u1, u2) = (self.contact(self.d1, s), self.contact(self.d2, s));
        let (a, c) = (s + u1 * self.rc, s + u2 * self.rc);
        let w = ((1.0 + u1.dot(u2).clamp(-1.0, 1.0)) / 2.0).max(0.0).sqrt();
        let bis = u1 + u2;
        if bis.length() < 1e-12 || w < 1e-9 {
            return Section { a, c, mid: a, m: a, w: 1.0 };
        }
        let bis = bis.normalize();
        Section { a, c, mid: s + bis * self.rc, m: s + bis * (self.rc / w), w }
    }

    /// The face's surface over [0, π/2] (spine angle) × [0, 1] (the arc from cylinder 1 to 2): rational quadratic
    /// across, a C2 cubic along interpolating `n` + 1 exact sections in their homogeneous form (The NURBS Book A9.1,
    /// knots by averaging; C2, so integrating over it takes one piece, not one per span); None if it strays more than
    /// `tol` from the exact sections between them.
    pub fn surface(&self, n: usize, tol: f64) -> Option<NurbsSurface> {
        let h = FRAC_PI_2 / n as f64;
        let rows: Vec<[DVec4; 3]> = (0..=n)
            .map(|i| {
                let x = self.section(h * i as f64);
                [x.a.extend(1.0), (x.m * x.w).extend(x.w), x.c.extend(1.0)]
            })
            .collect();
        let params: Vec<f64> = (0..=n).map(|i| h * i as f64).collect();
        let knots = averaged_knots(&params, 3);
        let solved = interpolate(&params, &knots, 3, &rows)?;
        let (mut poles, mut weights) = (Vec::with_capacity(solved.len() * 3), Vec::with_capacity(solved.len() * 3));
        for row in &solved {
            for p in row {
                if p.w.is_nan() || p.w <= 1e-6 {
                    return None;
                }
                poles.push(p.truncate() / p.w);
                weights.push(p.w);
            }
        }
        let s = NurbsSurface { udegree: 3, vdegree: 2, uknots: knots, vknots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], nu: solved.len(), nv: 3, poles, weights };
        for i in 0..n {
            let th = h * (i as f64 + 0.5);
            let x = self.section(th);
            if s.at(th, 0.0).distance(x.a) > tol || s.at(th, 1.0).distance(x.c) > tol || s.at(th, 0.5).distance(x.mid) > tol {
                return None;
            }
        }
        Some(s)
    }

    /// The face's side on cylinder `side` (1 or 2): the spine scaled from that cylinder's axis by R / (R - Rc), a
    /// planar ellipse: centre `q`, its point at spine angle th `q + p cos th + n R sin th`. As (centre, x direction,
    /// normal, major, minor), (p, n R) its principal axes (p is square to n), the normal n × p: counter-clockwise
    /// about it the short way from the face's point (th π/2) to the cylinder's (th 0).
    pub fn side(&self, side: u8) -> (DVec3, DVec3, DVec3, f64, f64) {
        let d = if side == 1 { self.d1 } else { self.d2 };
        let k = self.r / (self.r - self.rc);
        let along = d * d.dot(self.b);
        let p = (along + (self.b - along) * k) * self.l0;
        let (pl, nr) = (p.length(), self.r);
        let normal = self.n.cross(p / pl);
        if pl >= nr { (self.q, p / pl, normal, pl, nr) } else { (self.q, self.n, normal, nr, pl) }
    }

    /// The point on cylinder `side` at spine angle `th`.
    pub fn side_point(&self, side: u8, th: f64) -> DVec3 {
        let x = self.section(th);
        if side == 1 { x.a } else { x.c }
    }
}

/// Knots for interpolating at `t` with degree `p` (The NURBS Book (9.8): each inner knot the mean of `p` parameters).
pub(super) fn averaged_knots(t: &[f64], p: usize) -> Vec<f64> {
    let n = t.len() - 1;
    let mut k = vec![t[0]; p + 1];
    for j in 1..=n - p {
        k.push(t[j..j + p].iter().sum::<f64>() / p as f64);
    }
    k.extend(std::iter::repeat_n(t[n], p + 1));
    k
}

/// The degree-`p` basis functions non-zero at `u` in span `s` (A2.2).
fn basis(knots: &[f64], p: usize, s: usize, u: f64) -> Vec<f64> {
    let mut n = vec![0.0; p + 1];
    let (mut left, mut right) = (vec![0.0; p + 1], vec![0.0; p + 1]);
    n[0] = 1.0;
    for j in 1..=p {
        left[j] = u - knots[s + 1 - j];
        right[j] = knots[s + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let tmp = n[r] / (right[r + 1] + left[j - r]);
            n[r] = saved + right[r + 1] * tmp;
            saved = left[j - r] * tmp;
        }
        n[j] = saved;
    }
    n
}

/// Poles (one row of three homogeneous points each) whose degree-`p` curve on `knots` passes through `rows` at `t`
/// (A9.1: the collocation system, solved by Gaussian elimination with partial pivoting); None if singular.
pub(super) fn interpolate(t: &[f64], knots: &[f64], p: usize, rows: &[[DVec4; 3]]) -> Option<Vec<[DVec4; 3]>> {
    let n = t.len();
    let mut a = vec![vec![0.0; n]; n];
    for (k, &u) in t.iter().enumerate() {
        // The span: knots[s] <= u < knots[s + 1] (the last span at the end).
        let s = if u >= knots[n] { n - 1 } else { (p..n).rfind(|&s| knots[s] <= u).unwrap_or(p) };
        for (r, v) in basis(knots, p, s, u).into_iter().enumerate() {
            a[k][s - p + r] = v;
        }
    }
    let mut b: Vec<[DVec4; 3]> = rows.to_vec();
    for col in 0..n {
        let piv = (col..n).max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs()))?;
        if a[piv][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        for r in col + 1..n {
            let f = a[r][col] / a[col][col];
            if f == 0.0 {
                continue;
            }
            let (top, rest) = a.split_at_mut(r);
            for (x, y) in rest[0][col..n].iter_mut().zip(&top[col][col..n]) {
                *x -= f * y;
            }
            let bc = b[col];
            for k in 0..3 {
                b[r][k] -= bc[k] * f;
            }
        }
    }
    let mut x = vec![[DVec4::ZERO; 3]; n];
    for r in (0..n).rev() {
        let mut acc = b[r];
        for c in r + 1..n {
            for k in 0..3 {
                acc[k] -= x[c][k] * a[r][c];
            }
        }
        for k in 0..3 {
            x[r][k] = acc[k] / a[r][r];
        }
    }
    Some(x)
}
