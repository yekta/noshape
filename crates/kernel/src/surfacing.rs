//! Surfacing helpers: wrap / unwrap faces between developable surfaces,
//! isocline curves on faces and B-spline surface fitting to points.
//!
//! **Developments.** Planes, cylinders and cones are developable: each has an
//! isometric map to a flat 2D plane ("flat" coordinates). A wrap maps a face
//! from one surface to another by going through both developments with a 2D
//! rigid motion ([`Placement2d`]) in between, so lengths and areas are
//! preserved. Flat coordinates:
//!
//! * plane: `((p - o)·x, (p - o)·y)` with `y = axis × x`;
//! * cylinder: `(R·φ, h)` with `φ` the angle around the axis from `x` and
//!   `h = (p - o)·axis`;
//! * cone: polar development around the apex, `ρ·(cos ψ, sin ψ)` with `ρ` the
//!   slant distance from the apex and `ψ = φ·sin(half_angle)`.
//!
//! Angles around the axis are taken within ±π of a reference angle (the
//! anchor), so a wrapped region must stay within one turn.

use glam::{DMat2, DVec2, DVec3};

use crate::error::{KernelError, Result};
use crate::ffi::ffi;
use crate::geom::{Plane, Surface, p3};
use crate::history::OpResult;
use crate::shape::{Shape, ck, list_to_vec, vec_to_list};

fn wrap_pi(mut a: f64) -> f64 {
    use std::f64::consts::PI;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a <= -PI {
        a += 2.0 * PI;
    }
    a
}

/// A developable surface (wrap source or target).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Developable {
    Plane { origin: DVec3, x_dir: DVec3, normal: DVec3 },
    Cylinder { origin: DVec3, axis: DVec3, x_dir: DVec3, radius: f64 },
    /// `radius` is the radius at `origin` (h = 0); it grows along `axis`
    /// (`half_angle` in (0, π/2)).
    Cone { origin: DVec3, axis: DVec3, x_dir: DVec3, radius: f64, half_angle: f64 },
}

impl Developable {
    /// The developable surface a face lies on (plane, cylinder or cone).
    pub fn of_face(face: &Shape) -> Result<Developable> {
        Ok(match face.face_surface()? {
            Surface::Plane(p) => Developable::Plane { origin: p.origin, x_dir: p.unit_x(), normal: p.unit_normal() },
            Surface::Cylinder { origin, axis, x_dir, radius } => Developable::Cylinder { origin, axis: axis.normalize(), x_dir, radius },
            Surface::Cone { origin, axis, x_dir, ref_radius, half_angle, .. } => {
                let (axis, half_angle) = if half_angle < 0.0 { (-axis.normalize(), -half_angle) } else { (axis.normalize(), half_angle) };
                Developable::Cone { origin, axis, x_dir, radius: ref_radius, half_angle }
            }
            _ => return Err(KernelError::invalid("the face is not planar, cylindrical or conical")),
        })
    }

    fn frame(&self) -> (DVec3, DVec3, DVec3, DVec3) {
        let (o, z, x) = match *self {
            Developable::Plane { origin, x_dir, normal } => (origin, normal, x_dir),
            Developable::Cylinder { origin, axis, x_dir, .. } | Developable::Cone { origin, axis, x_dir, .. } => (origin, axis, x_dir),
        };
        let z = z.normalize_or_zero();
        let x = (x - z * x.dot(z)).normalize_or_zero();
        (o, x, z.cross(x), z)
    }

    /// Angle of a point around the axis, within ±π of `ref_angle` (0 for planes).
    pub fn angle_of(&self, p: DVec3, ref_angle: f64) -> f64 {
        if matches!(self, Developable::Plane { .. }) {
            return 0.0;
        }
        let (o, x, y, _) = self.frame();
        let d = p - o;
        ref_angle + wrap_pi(d.dot(y).atan2(d.dot(x)) - ref_angle)
    }

    /// Flat (development) coordinates of a point on (or near) the surface.
    pub fn flat(&self, p: DVec3, ref_angle: f64) -> DVec2 {
        let (o, x, y, z) = self.frame();
        let d = p - o;
        match *self {
            Developable::Plane { .. } => DVec2::new(d.dot(x), d.dot(y)),
            Developable::Cylinder { radius, .. } => DVec2::new(radius * self.angle_of(p, ref_angle), d.dot(z)),
            Developable::Cone { radius, half_angle, .. } => {
                let h = d.dot(z);
                let rr = radius + h * half_angle.tan();
                let rho = rr / half_angle.sin();
                let psi = self.angle_of(p, ref_angle) * half_angle.sin();
                DVec2::new(rho * psi.cos(), rho * psi.sin())
            }
        }
    }

    /// Point on the surface at flat coordinates.
    pub fn point(&self, f: DVec2, ref_angle: f64) -> DVec3 {
        let (o, x, y, z) = self.frame();
        match *self {
            Developable::Plane { .. } => o + x * f.x + y * f.y,
            Developable::Cylinder { radius, .. } => {
                let phi = f.x / radius;
                o + (x * phi.cos() + y * phi.sin()) * radius + z * f.y
            }
            Developable::Cone { radius, half_angle, .. } => {
                let sa = half_angle.sin();
                let rho = f.length();
                let psi_ref = ref_angle * sa;
                let psi = psi_ref + wrap_pi(f.y.atan2(f.x) - psi_ref);
                let phi = psi / sa;
                let rr = rho * sa;
                let h = (rr - radius) / half_angle.tan();
                o + (x * phi.cos() + y * phi.sin()) * rr + z * h
            }
        }
    }

    /// 3D unit directions of the flat X and Y axes at a flat point, and
    /// their cross product (the surface normal of the development's
    /// orientation).
    pub fn flat_axes(&self, f: DVec2, ref_angle: f64) -> (DVec3, DVec3, DVec3) {
        let (_, x, y, z) = self.frame();
        let (ax, ay) = match *self {
            Developable::Plane { .. } => (x, y),
            Developable::Cylinder { radius, .. } => {
                let phi = f.x / radius;
                (-x * phi.sin() + y * phi.cos(), z)
            }
            Developable::Cone { half_angle, .. } => {
                let p = self.point(f, ref_angle);
                let phi = self.angle_of(p, ref_angle);
                let radial = x * phi.cos() + y * phi.sin();
                let circ = -x * phi.sin() + y * phi.cos();
                let genr = radial * half_angle.sin() + z * half_angle.cos();
                let psi = f.y.atan2(f.x);
                (genr * psi.cos() - circ * psi.sin(), genr * psi.sin() + circ * psi.cos())
            }
        };
        (ax, ay, ax.cross(ay))
    }

    /// Flat coordinates of the closest point of the surface to `p`.
    pub fn project(&self, p: DVec3, ref_angle: f64) -> DVec2 {
        match *self {
            Developable::Plane { .. } | Developable::Cylinder { .. } => self.flat(p, ref_angle),
            Developable::Cone { half_angle, .. } => {
                // Closest point on the generator through p's angle.
                let (_, x, y, z) = self.frame();
                let phi = self.angle_of(p, ref_angle);
                let radial = x * phi.cos() + y * phi.sin();
                let genr = radial * half_angle.sin() + z * half_angle.cos();
                let apex = self.apex();
                let s = (p - apex).dot(genr).max(0.0);
                self.flat(apex + genr * s, ref_angle)
            }
        }
    }

    /// Cone apex (the origin for planes and cylinders).
    pub fn apex(&self) -> DVec3 {
        let (o, _, _, z) = self.frame();
        match *self {
            Developable::Cone { radius, half_angle, .. } => o - z * (radius / half_angle.tan()),
            _ => o,
        }
    }

    fn ffi(&self, ref_angle: f64) -> ffi::DevSurf {
        let (o, x, _, z) = self.frame();
        let (kind, radius, half_angle) = match *self {
            Developable::Plane { .. } => (0, 0.0, 0.0),
            Developable::Cylinder { radius, .. } => (1, radius, 0.0),
            Developable::Cone { radius, half_angle, .. } => (2, radius, half_angle),
        };
        ffi::DevSurf { kind, origin: p3(o), axis: p3(z), xdir: p3(x), radius, half_angle, ref_angle }
    }
}

/// 2D rigid motion or reflection between developments: `p' = m·p + t`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement2d {
    pub m: DMat2,
    pub t: DVec2,
}

impl Placement2d {
    pub const IDENTITY: Placement2d = Placement2d { m: DMat2::IDENTITY, t: DVec2::ZERO };

    pub fn apply(&self, p: DVec2) -> DVec2 {
        self.m * p + self.t
    }

    /// The motion taking point `from` with direction `from_dir` to `to` with
    /// direction `to_dir`; a reflection (about `to_dir`) when `mirror`.
    pub fn align(from: DVec2, from_dir: DVec2, to: DVec2, to_dir: DVec2, mirror: bool) -> Placement2d {
        let a = from_dir.normalize_or(DVec2::X);
        let b = to_dir.normalize_or(DVec2::X);
        let rot = |d: DVec2| DMat2::from_cols(d, d.perp());
        let flip = if mirror { DMat2::from_diagonal(DVec2::new(1.0, -1.0)) } else { DMat2::IDENTITY };
        let m = rot(b) * flip * rot(a).transpose();
        Placement2d { m, t: to - m * from }
    }

    /// Rotate by `angle` around `center` after this placement, then translate.
    pub fn then_rotate_shift(&self, center: DVec2, angle: f64, shift: DVec2) -> Placement2d {
        let r = DMat2::from_angle(angle);
        let m = r * self.m;
        let t = r * (self.t - center) + center + shift;
        Placement2d { m, t }
    }
}

/// Map faces lying on `src` onto `dst` through the developments with a 2D
/// placement (see the module doc). `src_ref` / `dst_ref` are the reference
/// angles of the two surfaces (results stay within ±π of `dst_ref`). Plane
/// and cylinder combinations map edges exactly; cones are approximated within
/// `tolerance`. Every result face lies on the exact target surface.
///
/// History: arg 0 = compound of the input faces; each input face and edge
/// `Generated` its image. Output: the face, or a compound of faces in input
/// order.
pub fn wrap_faces(faces: &[Shape], src: &Developable, src_ref: f64, dst: &Developable, dst_ref: f64, placement: &Placement2d, tolerance: f64) -> Result<OpResult> {
    if faces.is_empty() {
        return Err(KernelError::invalid("wrap needs at least one face"));
    }
    let l = vec_to_list(faces);
    let m = placement.m;
    let xf = [m.x_axis.x, m.y_axis.x, m.x_axis.y, m.y_axis.y, placement.t.x, placement.t.y];
    Ok(OpResult::from_ffi(ck(ffi::op_wrap_faces(&l, &src.ffi(src_ref), &dst.ffi(dst_ref), &xf, tolerance))?))
}

/// Isocline curves of a face: where the angle between the face (oriented
/// outward) and `direction` equals `angle`, i.e. `normal·direction =
/// sin(angle)` (a draft angle). Returns edges lying on the face's surface,
/// clipped to the face; with `extend_past_boundary` the curve ends overshoot
/// the face boundary slightly (for splitting the face with [`crate::imprint`]).
pub fn isocline_curves(face: &Shape, direction: DVec3, angle: f64, extend_past_boundary: bool) -> Result<Vec<Shape>> {
    let l = ck(ffi::isocline_edges(face.raw(), p3(direction), angle, extend_past_boundary))?;
    Ok(list_to_vec(&l))
}

/// B-spline face approximating a height grid over `frame`:
/// `heights[i * ny + j]` is the height (along the frame normal) at plane
/// coordinates `(x0 + i·dx, y0 + j·dy)`. The face spans the grid.
#[allow(clippy::too_many_arguments)]
pub fn bspline_surface_from_grid(frame: &Plane, x0: f64, dx: f64, y0: f64, dy: f64, heights: &[f64], nx: usize, ny: usize, max_degree: u32, tolerance: f64) -> Result<Shape> {
    grid_face(frame, x0, dx, y0, dy, heights, nx, ny, max_degree, tolerance, &[])
}

#[allow(clippy::too_many_arguments)]
fn grid_face(frame: &Plane, x0: f64, dx: f64, y0: f64, dy: f64, heights: &[f64], nx: usize, ny: usize, max_degree: u32, tolerance: f64, bounds: &[f64]) -> Result<Shape> {
    Shape::from_res(ffi::bspline_grid_face(&frame.ffi(), x0, dx, y0, dy, heights, nx as u32, ny as u32, max_degree as i32, tolerance, bounds))
}

/// Largest distance from the points to the face's surface (within its
/// parameter range).
pub fn max_deviation(face: &Shape, points: &[DVec3]) -> Result<f64> {
    let flat: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    ck(ffi::face_max_deviation(face.raw(), &flat))
}

/// Options for [`fit_surface`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceFitOptions {
    /// Approximation tolerance of the B-spline (meters).
    pub tolerance: f64,
    /// Pass through the points exactly (thin-plate interpolation) instead of
    /// a smoothing local least-squares fit.
    pub interpolate: bool,
    /// Denser grid and higher continuity (smoother, slower).
    pub smooth: bool,
}

impl Default for SurfaceFitOptions {
    fn default() -> Self {
        Self { tolerance: 1e-5, interpolate: true, smooth: false }
    }
}

/// Result of [`fit_surface`].
#[derive(Debug, Clone)]
pub struct SurfaceFit {
    pub face: Shape,
    /// Largest distance from the input points to the surface.
    pub max_deviation: f64,
}

/// Fit a B-spline surface to scattered points (e.g. mesh vertices). The
/// surface is a height field over the points' best-fit plane, spanning
/// their extent. `normals` (point index, normal) constrain the surface's
/// tangent plane at those points (interpolation mode) and orient the result.
pub fn fit_surface(points: &[DVec3], normals: &[(usize, DVec3)], opts: &SurfaceFitOptions) -> Result<SurfaceFit> {
    if points.len() < 3 {
        return Err(KernelError::invalid("at least three points are needed to fit a surface"));
    }
    let n = points.len() as f64;
    let c = points.iter().copied().sum::<DVec3>() / n;
    let mut cov = [[0.0f64; 3]; 3];
    for p in points {
        let d = *p - c;
        let v = [d.x, d.y, d.z];
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += v[i] * v[j];
            }
        }
    }
    let (vals, vecs) = jacobi3(cov);
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| vals[b].total_cmp(&vals[a]));
    let col = |k: usize| DVec3::new(vecs[0][k], vecs[1][k], vecs[2][k]);
    let mut x = col(order[0]).normalize_or_zero();
    let mut z = col(order[2]).normalize_or_zero();
    if vals[order[1]] <= 1e-18 * vals[order[0]].max(1e-300) || x.length() < 0.5 || z.length() < 0.5 {
        return Err(KernelError::invalid("the points are collinear"));
    }
    let avg_n: DVec3 = normals.iter().map(|(_, v)| v.normalize_or_zero()).sum();
    if avg_n.dot(z) < 0.0 {
        z = -z;
        x = -x;
    }
    let frame = Plane::new(c, z, x);
    let local: Vec<(DVec2, f64)> = points.iter().map(|p| (frame.to_local(*p), frame.signed_distance(*p))).collect();
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for (q, _) in &local {
        lo = lo.min(*q);
        hi = hi.max(*q);
    }
    let size = hi - lo;
    if size.x < 1e-12 || size.y < 1e-12 {
        return Err(KernelError::invalid("the points do not span an area"));
    }
    let base = match (opts.interpolate, opts.smooth) {
        (true, true) => 81.0,
        (true, false) => 49.0,
        (false, true) => 41.0,
        (false, false) => 25.0,
    };
    let aspect = size.x / size.y;
    let nx = ((base * aspect.sqrt()).round() as usize).clamp(5, 120);
    let ny = ((base / aspect.sqrt()).round() as usize).clamp(5, 120);
    // One extra cell around the points keeps spline end effects outside
    // them; the face is trimmed to the points' extent.
    let bounds = [lo.x, hi.x, lo.y, hi.y];
    let (dx, dy) = (size.x / (nx - 3) as f64, size.y / (ny - 3) as f64);
    let lo = lo - DVec2::new(dx, dy);
    let heights = if opts.interpolate && points.len() <= 800 {
        thin_plate_grid(&local, normals, &frame, lo, dx, dy, nx, ny)?
    } else {
        least_squares_grid(&local, lo, dx, dy, nx, ny)
    };
    let face = grid_face(&frame, lo.x, dx, lo.y, dy, &heights, nx, ny, if opts.smooth { 5 } else { 8 }, opts.tolerance.max(1e-9), &bounds)?;
    let max_deviation = max_deviation(&face, points)?;
    Ok(SurfaceFit { face, max_deviation })
}

/// Eigen decomposition of a symmetric 3x3 matrix (cyclic Jacobi). Returns
/// (eigenvalues, eigenvectors as columns).
#[allow(clippy::needless_range_loop)]
fn jacobi3(mut a: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off < 1e-30 {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let cs = 1.0 / (t * t + 1.0).sqrt();
            let sn = t * cs;
            for k in 0..3 {
                let akp = a[k][p];
                let akq = a[k][q];
                a[k][p] = cs * akp - sn * akq;
                a[k][q] = sn * akp + cs * akq;
            }
            for k in 0..3 {
                let apk = a[p][k];
                let aqk = a[q][k];
                a[p][k] = cs * apk - sn * aqk;
                a[q][k] = sn * apk + cs * aqk;
            }
            for row in v.iter_mut() {
                let vkp = row[p];
                let vkq = row[q];
                row[p] = cs * vkp - sn * vkq;
                row[q] = sn * vkp + cs * vkq;
            }
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}

/// Thin-plate spline through the points (plus tangent-plane helper points
/// for normal constraints), sampled on the grid.
#[allow(clippy::too_many_arguments)]
fn thin_plate_grid(local: &[(DVec2, f64)], normals: &[(usize, DVec3)], frame: &Plane, lo: DVec2, dx: f64, dy: f64, nx: usize, ny: usize) -> Result<Vec<f64>> {
    let mut sites: Vec<(DVec2, f64)> = local.to_vec();
    let extent = DVec2::new(dx * (nx - 1) as f64, dy * (ny - 1) as f64).length();
    let eps = extent * 0.01;
    let z = frame.unit_normal();
    let (ex, ey) = (frame.unit_x(), frame.unit_y());
    for &(i, nrm) in normals {
        let Some(&(q, h)) = local.get(i) else { continue };
        let nz = nrm.dot(z);
        if nz.abs() < 1e-6 {
            return Err(KernelError::invalid("a point normal is parallel to the fitted surface"));
        }
        let (gx, gy) = (-nrm.dot(ex) / nz, -nrm.dot(ey) / nz);
        for (ox, oy) in [(eps, 0.0), (-eps, 0.0), (0.0, eps), (0.0, -eps)] {
            sites.push((q + DVec2::new(ox, oy), h + gx * ox + gy * oy));
        }
    }
    // Merge coincident sites (keep the first).
    let mut uniq: Vec<(DVec2, f64)> = Vec::new();
    for s in sites {
        if !uniq.iter().any(|u| (u.0 - s.0).length() < extent * 1e-9) {
            uniq.push(s);
        }
    }
    let n = uniq.len();
    let phi = |r: f64| if r < 1e-300 { 0.0 } else { r * r * r.ln() };
    let m = n + 3;
    let mut a = vec![0.0f64; m * m];
    let mut b = vec![0.0f64; m];
    for i in 0..n {
        for j in 0..n {
            a[i * m + j] = phi((uniq[i].0 - uniq[j].0).length());
        }
        let p = uniq[i].0;
        for (k, v) in [1.0, p.x, p.y].into_iter().enumerate() {
            a[i * m + n + k] = v;
            a[(n + k) * m + i] = v;
        }
        b[i] = uniq[i].1;
    }
    let w = solve_dense(&mut a, &mut b, m).ok_or_else(|| KernelError::invalid("the points cannot be interpolated (duplicates or degenerate layout)"))?;
    let mut out = Vec::with_capacity(nx * ny);
    for i in 0..nx {
        for j in 0..ny {
            let q = lo + DVec2::new(dx * i as f64, dy * j as f64);
            let mut h = w[n] + w[n + 1] * q.x + w[n + 2] * q.y;
            for (k, s) in uniq.iter().enumerate() {
                h += w[k] * phi((q - s.0).length());
            }
            out.push(h);
        }
    }
    Ok(out)
}

/// Gaussian elimination with partial pivoting (in place); `None` if singular.
fn solve_dense(a: &mut [f64], b: &mut [f64], n: usize) -> Option<Vec<f64>> {
    let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs())).max(1e-300);
    for col in 0..n {
        let piv = (col..n).max_by(|&i, &j| a[i * n + col].abs().total_cmp(&a[j * n + col].abs()))?;
        if a[piv * n + col].abs() < scale * 1e-14 {
            return None;
        }
        if piv != col {
            for k in 0..n {
                a.swap(col * n + k, piv * n + k);
            }
            b.swap(col, piv);
        }
        let d = a[col * n + col];
        for r in col + 1..n {
            let f = a[r * n + col] / d;
            if f == 0.0 {
                continue;
            }
            for k in col..n {
                a[r * n + k] -= f * a[col * n + k];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let mut s = b[r];
        for k in r + 1..n {
            s -= a[r * n + k] * x[k];
        }
        x[r] = s / a[r * n + r];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// Local weighted least-squares plane fit at every grid node (smooths noise;
/// suited to dense mesh vertices).
fn least_squares_grid(local: &[(DVec2, f64)], lo: DVec2, dx: f64, dy: f64, nx: usize, ny: usize) -> Vec<f64> {
    let cell = dx.max(dy);
    // Bucket the points.
    let bx = nx + 1;
    let by = ny + 1;
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); bx * by];
    let key = |q: DVec2| {
        let i = (((q.x - lo.x) / dx).floor().max(0.0) as usize).min(bx - 1);
        let j = (((q.y - lo.y) / dy).floor().max(0.0) as usize).min(by - 1);
        (i, j)
    };
    for (k, (q, _)) in local.iter().enumerate() {
        let (i, j) = key(*q);
        buckets[i * by + j].push(k);
    }
    let mut out = Vec::with_capacity(nx * ny);
    for i in 0..nx {
        for j in 0..ny {
            let q = lo + DVec2::new(dx * i as f64, dy * j as f64);
            let mut reach = 2usize;
            let h = loop {
                let radius = cell * reach as f64;
                let sigma = radius * 0.5;
                // Weighted normal equations for h = a + b*x + c*y (x, y relative to q).
                let mut m = [[0.0f64; 3]; 3];
                let mut r = [0.0f64; 3];
                let mut count = 0;
                let (ci, cj) = key(q);
                for bi in ci.saturating_sub(reach)..=(ci + reach).min(bx - 1) {
                    for bj in cj.saturating_sub(reach)..=(cj + reach).min(by - 1) {
                        for &k in &buckets[bi * by + bj] {
                            let (p, hp) = local[k];
                            let d = p - q;
                            let dist = d.length();
                            if dist > radius {
                                continue;
                            }
                            let w = (-(dist * dist) / (2.0 * sigma * sigma)).exp();
                            let v = [1.0, d.x, d.y];
                            for a in 0..3 {
                                for b in 0..3 {
                                    m[a][b] += w * v[a] * v[b];
                                }
                                r[a] += w * v[a] * hp;
                            }
                            count += 1;
                        }
                    }
                }
                if count >= 3 {
                    let mut a = vec![m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2]];
                    let mut b = r.to_vec();
                    if let Some(x) = solve_dense(&mut a, &mut b, 3) {
                        break x[0];
                    }
                    if m[0][0] > 0.0 {
                        break r[0] / m[0][0];
                    }
                }
                if reach > nx.max(ny) * 2 {
                    break 0.0;
                }
                reach *= 2;
            };
            out.push(h);
        }
    }
    out
}
