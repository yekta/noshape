//! Assembly mate solver.
//!
//! Every instance is a rigid body with a world pose. A mate relates two mate
//! connector frames `Fa = Pa * La` and `Fb = Pb * Lb` (instance pose times the
//! connector's local frame). With `M = Fa⁻¹ · Fb` (frame B seen from frame A)
//! and `R0` the base alignment (Z axes opposed, X axes aligned — face-to-face
//! contact; `flip_primary` makes the Z axes agree), each mate type constrains
//! some components of `M`:
//!
//! | type        | locked                                     | free DOF          |
//! |-------------|--------------------------------------------|-------------------|
//! | Fastened    | rotation, translation                      | –                 |
//! | Revolute    | Z alignment, translation                   | rot Z             |
//! | Slider      | rotation, translation X/Y                  | trans Z           |
//! | Cylindrical | Z alignment, translation X/Y               | rot Z, trans Z    |
//! | Pin slot    | Z alignment, translation Y/Z               | rot Z, trans X    |
//! | Planar      | Z alignment, translation Z                 | trans X/Y, rot Z  |
//! | Ball        | translation                                | all rotations     |
//! | Parallel    | Z alignment                                | translations, rot Z |
//!
//! Offsets (along/around A's axes) are folded into the target. All mates are
//! solved together with Levenberg–Marquardt over 6 parameters per free body
//! (translation + rotation vector increment) using a finite-difference
//! Jacobian. Dragging adds weak residuals pulling a body point toward a
//! target. Relations (gear, rack & pinion, screw, linear) couple the free
//! values of two mates. DOF analysis uses the Jacobian rank.

use crate::assembly::MateType;
use glam::{DMat3, DQuat, DVec3};
use nalgebra::{DMatrix, DVector};
use std::collections::HashMap;

/// Rigid frame: rotation + translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub rotation: DQuat,
    pub origin: DVec3,
}

impl Default for Frame {
    fn default() -> Self {
        Self { rotation: DQuat::IDENTITY, origin: DVec3::ZERO }
    }
}

impl Frame {
    pub fn new(origin: DVec3, rotation: DQuat) -> Self {
        Self { rotation, origin }
    }
    /// Frame from origin and Z/X axes (X is orthogonalized against Z).
    pub fn from_axes(origin: DVec3, z: DVec3, x: DVec3) -> Self {
        let z = z.normalize();
        let mut x = x - z * x.dot(z);
        if x.length_squared() < 1e-18 {
            x = z.any_orthonormal_vector();
        }
        let x = x.normalize();
        let y = z.cross(x);
        Self { origin, rotation: DQuat::from_mat3(&DMat3::from_cols(x, y, z)).normalize() }
    }
    pub fn mul(&self, o: &Frame) -> Frame {
        Frame { rotation: (self.rotation * o.rotation).normalize(), origin: self.origin + self.rotation * o.origin }
    }
    pub fn inverse(&self) -> Frame {
        let r = self.rotation.inverse();
        Frame { rotation: r, origin: -(r * self.origin) }
    }
    pub fn transform_point(&self, p: DVec3) -> DVec3 {
        self.origin + self.rotation * p
    }
    pub fn x(&self) -> DVec3 {
        self.rotation * DVec3::X
    }
    pub fn y(&self) -> DVec3 {
        self.rotation * DVec3::Y
    }
    pub fn z(&self) -> DVec3 {
        self.rotation * DVec3::Z
    }
}

#[derive(Clone, Debug)]
pub struct SolverBody {
    pub pose: Frame,
    pub fixed: bool,
}

#[derive(Clone, Debug)]
pub struct SolverMate {
    pub kind: MateType,
    pub a: usize,
    pub a_local: Frame,
    pub b: usize,
    pub b_local: Frame,
    pub flip_primary: bool,
    /// Secondary axis reorientation in 90° steps.
    pub reorient: u8,
    pub offset: DVec3,
    /// Rotation offset about Z (radians), for Fastened/Revolute... targets.
    pub rotation: f64,
    /// Limits on the free values: (value index, min, max).
    pub limits: Vec<(usize, Option<f64>, Option<f64>)>,
    /// Additional connectors (body, local frame): Width uses
    /// `[width 2]` or `[width 2, tab 2]` (`a` = tab 1, `b` = width 1).
    pub extra: Vec<(usize, Frame)>,
    /// Tangent: contact geometry on `a` and `b` (body-local coordinates).
    pub geom: Option<TangentPair>,
    /// Pattern / mirror follower: `b` (instance frame `b_local`) is placed
    /// relative to `a` (instance frame `a_local`) by a transform about an axis frame.
    pub follow: Option<Follow>,
    /// Free values held at targets (value index, target): animation drives
    /// one degree of freedom, Apply limit position moves one to a limit.
    pub hold: Vec<(usize, f64)>,
}

impl Default for SolverMate {
    fn default() -> Self {
        Self {
            kind: MateType::Fastened,
            a: 0,
            a_local: Frame::default(),
            b: 0,
            b_local: Frame::default(),
            flip_primary: false,
            reorient: 0,
            offset: DVec3::ZERO,
            rotation: 0.0,
            limits: vec![],
            extra: vec![],
            geom: None,
            follow: None,
            hold: vec![],
        }
    }
}

impl SolverMate {
    pub fn new(kind: MateType, a: usize, a_local: Frame, b: usize, b_local: Frame) -> Self {
        Self { kind, a, a_local, b, b_local, ..Default::default() }
    }
}

/// Analytic contact geometry for tangent mates (body-local coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TangentGeom {
    /// Plane through `point` with outward `normal`.
    Plane { point: DVec3, normal: DVec3 },
    /// Cylinder (radius 0: a line / straight edge).
    Cylinder { point: DVec3, axis: DVec3, radius: f64 },
    /// Sphere (radius 0: a point / vertex).
    Sphere { center: DVec3, radius: f64 },
    /// Cone with its apex, unit axis pointing into the cone and half angle.
    Cone { apex: DVec3, axis: DVec3, half_angle: f64 },
    /// Torus (minor radius 0: a circular edge).
    Torus { center: DVec3, axis: DVec3, major: f64, minor: f64 },
}

impl TangentGeom {
    pub fn transformed(&self, f: &Frame) -> TangentGeom {
        match *self {
            TangentGeom::Plane { point, normal } => TangentGeom::Plane { point: f.transform_point(point), normal: f.rotation * normal },
            TangentGeom::Cylinder { point, axis, radius } => TangentGeom::Cylinder { point: f.transform_point(point), axis: f.rotation * axis, radius },
            TangentGeom::Sphere { center, radius } => TangentGeom::Sphere { center: f.transform_point(center), radius },
            TangentGeom::Cone { apex, axis, half_angle } => TangentGeom::Cone { apex: f.transform_point(apex), axis: f.rotation * axis, half_angle },
            TangentGeom::Torus { center, axis, major, minor } => TangentGeom::Torus { center: f.transform_point(center), axis: f.rotation * axis, major, minor },
        }
    }

    /// Signed distance of a point to the surface (positive outside: on the
    /// normal side of a plane, outside spheres, cylinders, cones and tori).
    pub fn signed_distance(&self, q: DVec3) -> f64 {
        match *self {
            TangentGeom::Plane { point, normal } => (q - point).dot(normal),
            TangentGeom::Sphere { center, radius } => (q - center).length() - radius,
            TangentGeom::Cylinder { point, axis, radius } => {
                let d = q - point;
                (d - axis * d.dot(axis)).length() - radius
            }
            TangentGeom::Cone { apex, axis, half_angle } => {
                let d = q - apex;
                let h = d.dot(axis);
                let rho = (d - axis * h).length();
                rho * half_angle.cos() - h * half_angle.sin()
            }
            TangentGeom::Torus { center, axis, major, minor } => {
                let d = q - center;
                let h = d.dot(axis);
                let rho = (d - axis * h).length();
                ((rho - major).powi(2) + h * h).sqrt() - minor
            }
        }
    }

    /// Closest point of the (unbounded) surface to `q`.
    pub fn closest_point(&self, q: DVec3) -> DVec3 {
        let perp_dir = |d: DVec3, axis: DVec3| {
            let p = d - axis * d.dot(axis);
            if p.length_squared() > 1e-24 { p.normalize() } else { axis.any_orthonormal_vector() }
        };
        match *self {
            TangentGeom::Plane { point, normal } => q - normal * (q - point).dot(normal),
            TangentGeom::Sphere { center, radius } => {
                let d = q - center;
                center + if d.length_squared() > 1e-24 { d.normalize() * radius } else { DVec3::X * radius }
            }
            TangentGeom::Cylinder { point, axis, radius } => {
                let d = q - point;
                point + axis * d.dot(axis) + perp_dir(d, axis) * radius
            }
            TangentGeom::Cone { apex, axis, half_angle } => {
                let d = q - apex;
                let u = perp_dir(d, axis);
                let g = axis * half_angle.cos() + u * half_angle.sin();
                apex + g * d.dot(g).max(0.0)
            }
            TangentGeom::Torus { center, axis, major, minor } => {
                let d = q - center;
                let c = center + perp_dir(d, axis) * major;
                let e = q - c;
                c + if e.length_squared() > 1e-24 { e.normalize() * minor } else { axis * minor }
            }
        }
    }
}

/// Whether a tangent mate between two geometry kinds is supported.
pub fn tangent_supported(a: &TangentGeom, b: &TangentGeom) -> bool {
    use TangentGeom::*;
    matches!(
        (a, b),
        (Plane { .. } | Sphere { .. } | Cylinder { .. }, Plane { .. } | Sphere { .. } | Cylinder { .. }) | (Cone { .. } | Torus { .. }, Plane { .. } | Sphere { .. }) | (Plane { .. } | Sphere { .. }, Cone { .. } | Torus { .. })
    )
}

/// One face of a tangent-propagation chain (body-local coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentFace {
    pub geom: TangentGeom,
    /// Face bounds and centroid.
    pub lo: DVec3,
    pub hi: DVec3,
    pub centroid: DVec3,
    /// Concave curved face (hole, fillet groove): contact with it is internal.
    pub concave: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TangentPair {
    pub a: TangentGeom,
    pub b: TangentGeom,
    /// Internal contact (one curved surface inside the other) for
    /// cylinder/sphere pairs; decided from the initial positions.
    pub internal: bool,
    /// Tangent propagation: faces tangent-connected to the selected ones
    /// (including them). When either is non-empty the contact uses the face
    /// pair whose contact point lies on both faces.
    pub chain_a: Vec<TangentFace>,
    pub chain_b: Vec<TangentFace>,
}

impl TangentPair {
    pub fn new(a: TangentGeom, b: TangentGeom, internal: bool) -> Self {
        Self { a, b, internal, chain_a: vec![], chain_b: vec![] }
    }
}

/// Transform of a pattern / mirror follower relative to its axis frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FollowXform {
    /// `E = A · X · A⁻¹ · S` (linear: translation along Z; circular: rotation about Z).
    Rigid(Frame),
    /// Mirror across the axis frame's XY plane combined with a reflection in
    /// the seed's local symmetry plane (point, normal in seed instance coordinates),
    /// which composes to a proper rigid motion.
    Mirror { local_point: DVec3, local_normal: DVec3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    pub axis_body: usize,
    pub axis_local: Frame,
    pub xform: FollowXform,
}

/// Reflection matrix across the plane through `p` with unit normal `n`.
pub fn reflection(p: DVec3, n: DVec3) -> glam::DMat4 {
    let n = n.normalize_or_zero();
    let m3 = glam::DMat3::IDENTITY - 2.0 * glam::DMat3::from_cols(n * n.x, n * n.y, n * n.z);
    let t = 2.0 * p.dot(n) * n;
    glam::DMat4::from_cols(m3.x_axis.extend(0.0), m3.y_axis.extend(0.0), m3.z_axis.extend(0.0), t.extend(1.0))
}

fn frame_mat(f: &Frame) -> glam::DMat4 {
    glam::DMat4::from_rotation_translation(f.rotation, f.origin)
}

impl Follow {
    /// Expected world frame of the follower instance given the seed instance frame.
    pub fn expected(&self, bodies: &[Frame], seed: &Frame) -> Frame {
        let a = bodies[self.axis_body].mul(&self.axis_local);
        match self.xform {
            FollowXform::Rigid(x) => a.mul(&x).mul(&a.inverse()).mul(seed),
            FollowXform::Mirror { local_point, local_normal } => {
                let am = frame_mat(&a);
                let mw = am * reflection(DVec3::ZERO, DVec3::Z) * am.inverse();
                let m = mw * frame_mat(seed) * reflection(local_point, local_normal);
                let (_, r, t) = m.to_scale_rotation_translation();
                // A reflection pair is a proper rotation; strip any numerical scale sign.
                Frame::new(t, r.normalize())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RelationKind {
    /// value_b = -ratio * value_a (gears turn opposite unless reversed)
    Gear,
    /// translation_b = ratio * rotation_a (ratio = pitch radius)
    RackAndPinion,
    /// translation = ratio * rotation (lead per radian) on one cylindrical mate pair
    Screw,
    /// value_b = ratio * value_a
    Linear,
}

#[derive(Clone, Debug)]
pub struct SolverRelation {
    pub kind: RelationKind,
    pub mate_a: usize,
    pub mate_b: usize,
    pub ratio: f64,
    pub reverse: bool,
    /// Free values at the time the relation was established (reference).
    pub base_a: f64,
    pub base_b: f64,
    /// Which free value of each mate is coupled (see [`mate_values`]:
    /// 0 = rotation about Z, 1..3 = translation X/Y/Z).
    pub value_a: usize,
    pub value_b: usize,
}

#[derive(Clone, Debug)]
pub struct DragTarget {
    pub body: usize,
    /// Point in body-local coordinates.
    pub local_point: DVec3,
    pub target: DVec3,
}

#[derive(Clone, Debug, Default)]
pub struct SolveResult {
    pub converged: bool,
    pub iterations: usize,
    pub max_residual: f64,
    /// Per-mate satisfied flag.
    pub mate_ok: Vec<bool>,
    /// Remaining degrees of freedom of the whole assembly.
    pub dof: usize,
    /// Per-body remaining DOF (0 = fully constrained).
    pub body_dof: Vec<usize>,
}

/// Base alignment between connector frames.
fn base_rotation(flip_primary: bool, reorient: u8) -> DQuat {
    let r0 = if flip_primary { DQuat::IDENTITY } else { DQuat::from_rotation_x(std::f64::consts::PI) };
    r0 * DQuat::from_rotation_z(reorient as f64 * std::f64::consts::FRAC_PI_2)
}

/// Relative transform `Fa⁻¹ Fb` expressed after removing the base alignment.
fn relative(bodies: &[Frame], m: &SolverMate) -> Frame {
    let fa = bodies[m.a].mul(&m.a_local);
    let fb = bodies[m.b].mul(&m.b_local);
    let rel = fa.inverse().mul(&fb);
    // Remove base alignment: rel = Rel' * R0  =>  Rel' = rel * R0⁻¹ (rotation only)
    let r0 = base_rotation(m.flip_primary, m.reorient);
    Frame { rotation: (rel.rotation * r0.inverse()).normalize(), origin: rel.origin }
}

/// Rotation vector (axis * angle) of a quaternion, shortest path.
fn log_rot(q: DQuat) -> DVec3 {
    let q = if q.w < 0.0 { -q } else { q };
    let v = DVec3::new(q.x, q.y, q.z);
    let s = v.length();
    if s < 1e-12 {
        return v * 2.0;
    }
    let angle = 2.0 * s.atan2(q.w);
    v / s * angle
}

/// Free values of a mate (rotation about Z, translations) as currently posed.
pub fn mate_values(bodies: &[Frame], m: &SolverMate) -> [f64; 4] {
    let r = relative(bodies, m);
    let x = r.rotation * DVec3::X;
    let angle = x.y.atan2(x.x);
    [angle, r.origin.x, r.origin.y, r.origin.z]
}

fn tangent_residuals(ga: TangentGeom, gb: TangentGeom, flip: bool, internal: bool, out: &mut Vec<f64>) {
    use TangentGeom::*;
    let sgn = if flip { -1.0 } else { 1.0 };
    let curved = |r1: f64, r2: f64| if internal { (r1 - r2).abs() } else { r1 + r2 };
    match (ga, gb) {
        (Plane { point, normal }, Cone { apex, axis, half_angle }) | (Cone { apex, axis, half_angle }, Plane { point, normal }) => {
            // The cone lies on the plane along a generator: apex on the plane,
            // axis tilted by the half angle away from it.
            out.push((apex - point).dot(normal));
            out.push(axis.dot(normal) - sgn * half_angle.sin());
        }
        (Plane { point, normal }, Torus { center, axis, major, minor }) | (Torus { center, axis, major, minor }, Plane { point, normal }) => {
            let n_perp = (normal - axis * axis.dot(normal)).length();
            out.push((center - point).dot(normal) - sgn * (major * n_perp + minor));
        }
        (g @ (Cone { .. } | Torus { .. }), Sphere { center, radius }) | (Sphere { center, radius }, g @ (Cone { .. } | Torus { .. })) => {
            let d = g.signed_distance(center);
            out.push(if internal { d + radius } else { d - radius });
        }
        (Cone { .. } | Torus { .. }, _) | (_, Cone { .. } | Torus { .. }) => {
            // Unsupported pair (rejected when the mate is prepared).
            out.push(0.0);
        }
        (Plane { point: pa, normal: na }, Plane { point: pb, normal: nb }) => {
            // Faces touch: normals opposed (agree when flipped), coplanar.
            let e = nb + na * sgn;
            out.extend_from_slice(&[e.x, e.y, e.z, (pb - pa).dot(na)]);
        }
        (Plane { point, normal }, Cylinder { point: c, axis, radius }) | (Cylinder { point: c, axis, radius }, Plane { point, normal }) => {
            out.push(axis.dot(normal));
            out.push((c - point).dot(normal) - sgn * radius);
        }
        (Plane { point, normal }, Sphere { center, radius }) | (Sphere { center, radius }, Plane { point, normal }) => {
            out.push((center - point).dot(normal) - sgn * radius);
        }
        (Cylinder { point: c1, axis: a1, radius: r1 }, Cylinder { point: c2, axis: a2, radius: r2 }) => {
            let x = a1.cross(a2);
            let d = c2 - c1;
            let perp = d - a1 * d.dot(a1);
            out.extend_from_slice(&[x.x, x.y, x.z, perp.length() - curved(r1, r2)]);
        }
        (Cylinder { point: c1, axis, radius: r1 }, Sphere { center, radius: r2 }) | (Sphere { center, radius: r2 }, Cylinder { point: c1, axis, radius: r1 }) => {
            let d = center - c1;
            let perp = d - axis * d.dot(axis);
            out.push(perp.length() - curved(r1, r2));
        }
        (Sphere { center: c1, radius: r1 }, Sphere { center: c2, radius: r2 }) => out.push((c2 - c1).length() - curved(r1, r2)),
    }
}

/// Number of residual rows of a propagated tangent mate (fixed so the
/// Jacobian keeps its shape when the contact moves to another face pair).
const CHAIN_ROWS: usize = 4;

/// Distance of a point outside an axis-aligned box (0 inside).
fn box_excess(p: DVec3, lo: DVec3, hi: DVec3) -> f64 {
    (lo - p).max(p - hi).max(DVec3::ZERO).length()
}

/// Approximate contact point between two surfaces (alternating projections
/// starting from a point near the first face).
pub fn contact_point(a: &TangentGeom, b: &TangentGeom, start: DVec3) -> DVec3 {
    let mut x = a.closest_point(start);
    let mut y = b.closest_point(x);
    for _ in 0..3 {
        x = a.closest_point(y);
        y = b.closest_point(x);
    }
    (x + y) * 0.5
}

/// Residuals of a tangent mate with propagation: among all supported face
/// pairs of the two chains, the pair whose contact point lies on both faces
/// (else the one closest to doing so) with the smallest residual.
fn chain_residuals(pa: &Frame, pb: &Frame, g: &TangentPair, flip: bool, out: &mut Vec<f64>) {
    let single_a = [TangentFace { geom: g.a, lo: DVec3::splat(-1e30), hi: DVec3::splat(1e30), centroid: DVec3::ZERO, concave: false }];
    let single_b = [TangentFace { geom: g.b, lo: DVec3::splat(-1e30), hi: DVec3::splat(1e30), centroid: DVec3::ZERO, concave: false }];
    let ca: &[TangentFace] = if g.chain_a.is_empty() { &single_a } else { &g.chain_a };
    let cb: &[TangentFace] = if g.chain_b.is_empty() { &single_b } else { &g.chain_b };
    let (ia, ib) = (pa.inverse(), pb.inverse());
    let mut best: Option<(f64, f64, Vec<f64>)> = None;
    let mut rows = Vec::with_capacity(CHAIN_ROWS);
    for fa in ca {
        for fb in cb {
            if !tangent_supported(&fa.geom, &fb.geom) {
                continue;
            }
            let ga = fa.geom.transformed(pa);
            let gb = fb.geom.transformed(pb);
            let internal = if g.chain_a.is_empty() && g.chain_b.is_empty() { g.internal } else { fa.concave || fb.concave || (g.internal && fa.geom == g.a && fb.geom == g.b) };
            let start = if g.chain_a.is_empty() { pb.transform_point(fb.centroid) } else { pa.transform_point(fa.centroid) };
            let cp = contact_point(&ga, &gb, start);
            let size = ((fa.hi - fa.lo).length() + (fb.hi - fb.lo).length()).clamp(1e-6, 1e6);
            let tol = 1e-6 + 1e-3 * size;
            let out_a = (box_excess(ia.transform_point(cp), fa.lo, fa.hi) - tol).max(0.0);
            let out_b = (box_excess(ib.transform_point(cp), fb.lo, fb.hi) - tol).max(0.0);
            rows.clear();
            tangent_residuals(ga, gb, flip, internal, &mut rows);
            let norm: f64 = rows.iter().map(|v| v * v).sum::<f64>().sqrt();
            let outside = out_a + out_b;
            let better = match &best {
                None => true,
                Some((o, n, _)) => outside < *o - 1e-12 || ((outside - *o).abs() <= 1e-12 && norm < *n),
            };
            if better {
                best = Some((outside, norm, rows.clone()));
            }
        }
    }
    let mut r = best.map(|b| b.2).unwrap_or_default();
    r.resize(CHAIN_ROWS, 0.0);
    out.extend_from_slice(&r);
}

fn mate_residuals(bodies: &[Frame], m: &SolverMate, out: &mut Vec<f64>) {
    if let Some(f) = &m.follow {
        let seed = bodies[m.a].mul(&m.a_local);
        let e = f.expected(bodies, &seed);
        let cur = bodies[m.b].mul(&m.b_local);
        let r = log_rot(e.rotation.inverse() * cur.rotation);
        let t = cur.origin - e.origin;
        out.extend_from_slice(&[r.x, r.y, r.z, t.x, t.y, t.z]);
        return;
    }
    if m.kind == MateType::Tangent
        && let Some(g) = &m.geom
    {
        if !g.chain_a.is_empty() || !g.chain_b.is_empty() {
            chain_residuals(&bodies[m.a], &bodies[m.b], g, m.flip_primary, out);
            return;
        }
        let ga = g.a.transformed(&bodies[m.a]);
        let gb = g.b.transformed(&bodies[m.b]);
        tangent_residuals(ga, gb, m.flip_primary, g.internal, out);
        return;
    }
    if m.kind == MateType::Width && !m.extra.is_empty() {
        // Tab connector(s) centered between the two width connectors' XY planes.
        let t1 = bodies[m.a].mul(&m.a_local);
        let w1 = bodies[m.b].mul(&m.b_local);
        let (wb, wl) = m.extra[0];
        let w2 = bodies[wb].mul(&wl);
        let c = (w1.origin + w2.origin) * 0.5;
        let n = w1.z();
        let (mid, tabs) = match m.extra.get(1) {
            Some((tb, tl)) => {
                let t2 = bodies[*tb].mul(tl);
                ((t1.origin + t2.origin) * 0.5, vec![t1.z(), t2.z()])
            }
            None => (t1.origin, vec![t1.z()]),
        };
        out.push((mid - c).dot(n));
        for z in tabs {
            let x = z.cross(n);
            out.extend_from_slice(&[x.x, x.y, x.z]);
        }
        return;
    }
    let r = relative(bodies, m);
    let z = r.rotation * DVec3::Z;
    // Offset: the first connector sits at +offset in the second's frame
    // (bundle `getSecondMateConnectorOffsetTransform`; live assembly/740–748:
    // the plate picked first ends 10 mm above the other). In the first's
    // frame: rel.origin = -R_rel * offset, R_rel the full relative rotation.
    let full = bodies[m.a].mul(&m.a_local).inverse().mul(&bodies[m.b].mul(&m.b_local));
    let t = r.origin + full.rotation * m.offset;
    let rot_target = DQuat::from_rotation_z(m.rotation);
    match m.kind {
        MateType::Fastened => {
            let e = log_rot(rot_target.inverse() * r.rotation);
            out.extend_from_slice(&[e.x, e.y, e.z, t.x, t.y, t.z]);
        }
        MateType::Revolute => out.extend_from_slice(&[z.x, z.y, t.x, t.y, t.z]),
        MateType::Slider => {
            let e = log_rot(rot_target.inverse() * r.rotation);
            out.extend_from_slice(&[e.x, e.y, e.z, t.x, t.y]);
        }
        MateType::Cylindrical => out.extend_from_slice(&[z.x, z.y, t.x, t.y]),
        MateType::PinSlot => out.extend_from_slice(&[z.x, z.y, t.y, t.z]),
        MateType::Planar => out.extend_from_slice(&[z.x, z.y, t.z]),
        MateType::Ball => out.extend_from_slice(&[t.x, t.y, t.z]),
        MateType::Parallel => out.extend_from_slice(&[z.x, z.y]),
        // Without contact geometry / width connectors (not resolvable):
        // point-on-plane fallback between the two connectors.
        MateType::Tangent => out.push(t.z),
        MateType::Width => out.push(t.z),
    }
    // Limits: penalize violation of free values.
    let vals = mate_values(bodies, m);
    for &(i, lo, hi) in &m.limits {
        // Index 4: cone angle between the connectors' Z axes (Ball mate limit).
        let v = if i == 4 { (relative(bodies, m).rotation * DVec3::Z).z.clamp(-1.0, 1.0).acos() } else { vals[i.min(3)] };
        let mut e = 0.0;
        if let Some(lo) = lo
            && v < lo
        {
            e = v - lo;
        }
        if let Some(hi) = hi
            && v > hi
        {
            e = v - hi;
        }
        out.push(e);
    }
}

fn wrap_angle(a: f64) -> f64 {
    let mut a = a % std::f64::consts::TAU;
    if a > std::f64::consts::PI {
        a -= std::f64::consts::TAU;
    }
    if a < -std::f64::consts::PI {
        a += std::f64::consts::TAU;
    }
    a
}

fn relation_residual(bodies: &[Frame], mates: &[SolverMate], r: &SolverRelation) -> f64 {
    let va = mate_values(bodies, &mates[r.mate_a]);
    let vb = mate_values(bodies, &mates[r.mate_b]);
    let sign = if r.reverse { -1.0 } else { 1.0 };
    let delta = |v: &[f64; 4], i: usize, base: f64| if i == 0 { wrap_angle(v[0] - base) } else { v[i.min(3)] - base };
    let da = delta(&va, r.value_a, r.base_a);
    let db = delta(&vb, r.value_b, r.base_b);
    match r.kind {
        RelationKind::Gear => wrap_angle(db + sign * r.ratio * da),
        RelationKind::Linear | RelationKind::RackAndPinion | RelationKind::Screw => db - sign * r.ratio * da,
    }
}

pub struct Problem<'a> {
    pub bodies: &'a [SolverBody],
    pub mates: &'a [SolverMate],
    pub relations: &'a [SolverRelation],
    pub drags: &'a [DragTarget],
    /// The residual terms solved together (one group of connected bodies).
    terms: Vec<Term>,
}

const DRAG_WEIGHT: f64 = 1e-2;

/// One block of residual rows: a mate (with its held values), a relation or
/// a drag, in the order `residuals` writes them.
#[derive(Clone, Copy)]
enum Term {
    Mate(usize),
    Relation(usize),
    Drag(usize),
}

fn all_terms(mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget]) -> impl Iterator<Item = Term> {
    (0..mates.len()).map(Term::Mate).chain((0..relations.len()).map(Term::Relation)).chain((0..drags.len()).map(Term::Drag))
}

fn terms<'p>(p: &'p Problem) -> impl Iterator<Item = Term> + 'p {
    p.terms.iter().copied()
}

/// The bodies whose poses a mate's rows read.
fn mate_bodies(m: &SolverMate) -> impl Iterator<Item = usize> + '_ {
    [m.a, m.b].into_iter().chain(m.extra.iter().map(|(b, _)| *b)).chain(m.follow.map(|f| f.axis_body))
}

fn term_bodies(p: &Problem, t: Term) -> Vec<usize> {
    match t {
        Term::Mate(i) => mate_bodies(&p.mates[i]).collect(),
        Term::Relation(i) => mate_bodies(&p.mates[p.relations[i].mate_a]).chain(mate_bodies(&p.mates[p.relations[i].mate_b])).collect(),
        Term::Drag(i) => vec![p.drags[i].body],
    }
}

fn term_residuals(p: &Problem, poses: &[Frame], t: Term, out: &mut Vec<f64>) {
    match t {
        Term::Mate(i) => {
            let m = &p.mates[i];
            mate_residuals(poses, m, out);
            if !m.hold.is_empty() {
                let v = mate_values(poses, m);
                for (i, target) in &m.hold {
                    out.push(if *i == 0 { wrap_angle(v[0] - target) } else { v[(*i).min(3)] - target });
                }
            }
        }
        Term::Relation(i) => out.push(relation_residual(poses, p.mates, &p.relations[i])),
        Term::Drag(i) => {
            let d = &p.drags[i];
            let w = poses[d.body].transform_point(d.local_point) - d.target;
            out.extend_from_slice(&[w.x * DRAG_WEIGHT, w.y * DRAG_WEIGHT, w.z * DRAG_WEIGHT]);
        }
    }
}

fn residuals(p: &Problem, poses: &[Frame], out: &mut Vec<f64>) {
    out.clear();
    for t in terms(p) {
        term_residuals(p, poses, t, out);
    }
}

/// Finite-difference Jacobian of `residuals` (value `r` at `poses`) over the
/// free bodies' 6 pose parameters each. A body's columns re-evaluate only
/// the terms that read it (a 20-link chain: 2 of 19 mates per body).
fn jacobian(p: &Problem, poses: &[Frame], free: &[usize], r: &[f64], scale: f64, eps: f64) -> DMatrix<f64> {
    let mut spans = Vec::new();
    let mut by_body: Vec<Vec<usize>> = vec![Vec::new(); poses.len()];
    let mut row = 0;
    let mut buf = Vec::new();
    for (ti, t) in terms(p).enumerate() {
        buf.clear();
        term_residuals(p, poses, t, &mut buf);
        spans.push((t, row, buf.len()));
        row += buf.len();
        for b in term_bodies(p, t) {
            if let Some(v) = by_body.get_mut(b)
                && v.last() != Some(&ti)
            {
                v.push(ti);
            }
        }
    }
    debug_assert_eq!(row, r.len());
    let mut jac = DMatrix::<f64>::zeros(r.len(), free.len() * 6);
    let mut work = poses.to_vec();
    for (k, &bi) in free.iter().enumerate() {
        for j in 0..6 {
            let mut d = [0.0; 6];
            d[j] = if j < 3 { eps * scale } else { eps };
            work[bi] = apply_delta(&poses[bi], &d);
            for &ti in &by_body[bi] {
                let (t, start, len) = spans[ti];
                buf.clear();
                term_residuals(p, &work, t, &mut buf);
                // A term whose row count changed with the step (none should):
                // leave its column entries at zero rather than misalign rows.
                if buf.len() != len {
                    continue;
                }
                for (i, v) in buf.iter().enumerate() {
                    jac[(start + i, k * 6 + j)] = (v - r[start + i]) / eps;
                }
            }
            work[bi] = poses[bi];
        }
    }
    jac
}

fn apply_delta(pose: &Frame, d: &[f64]) -> Frame {
    let dt = DVec3::new(d[0], d[1], d[2]);
    let dr = DVec3::new(d[3], d[4], d[5]);
    let angle = dr.length();
    let q = if angle > 1e-15 { DQuat::from_axis_angle(dr / angle, angle) } else { DQuat::IDENTITY };
    Frame { rotation: (q * pose.rotation).normalize(), origin: pose.origin + dt }
}

/// Solve in place. Returns convergence info.
pub fn solve(bodies: &mut [SolverBody], mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget]) -> SolveResult {
    solve_with(bodies, mates, relations, drags, true)
}

/// [`solve`], with the degrees-of-freedom analysis optional (`dof` false:
/// `dof` and `body_dof` are left at zero, e.g. for each step of a drag).
pub fn solve_with(bodies: &mut [SolverBody], mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget], dof: bool) -> SolveResult {
    // Followers (pattern / mirror copies) start at their expected pose.
    for _ in 0..2 {
        for m in mates {
            let Some(f) = &m.follow else { continue };
            if bodies[m.b].fixed {
                continue;
            }
            let poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
            let seed = poses[m.a].mul(&m.a_local);
            let e = f.expected(&poses, &seed);
            bodies[m.b].pose = e.mul(&m.b_local.inverse());
        }
    }
    if !drags.is_empty() {
        // Phase 1: follow the drag (soft); phase 2: project back onto the
        // constraint manifold exactly, starting from the dragged pose.
        // Phase 1 needs no DOF analysis (phase 2 does it at the final poses).
        solve_inner(bodies, mates, relations, drags, false);
        return solve_inner(bodies, mates, relations, &[], dof);
    }
    solve_inner(bodies, mates, relations, drags, dof)
}

fn solve_inner(bodies: &mut [SolverBody], mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget], dof: bool) -> SolveResult {
    let mut poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
    let scale = characteristic_length(&poses, mates).max(1e-3);
    let mut result = SolveResult { mate_ok: vec![false; mates.len()], body_dof: vec![0; bodies.len()], ..Default::default() };
    // Free bodies linked by a term (fixed bodies link nothing: they don't
    // move) form groups solved one by one: a drag moves only its group, and
    // the dense linear algebra scales with the group, not the assembly.
    let mut parent: Vec<usize> = (0..bodies.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let probe = Problem { bodies, mates, relations, drags, terms: Vec::new() };
    let mut groups: Vec<(Vec<usize>, Vec<Term>)> = Vec::new();
    let mut term_root: Vec<(Term, Option<usize>)> = Vec::new();
    for t in all_terms(mates, relations, drags) {
        let free: Vec<usize> = term_bodies(&probe, t).into_iter().filter(|&b| !bodies[b].fixed).collect();
        for w in free.windows(2) {
            let (a, b) = (root(&mut parent, w[0]), root(&mut parent, w[1]));
            parent[a] = b;
        }
        term_root.push((t, free.first().copied()));
    }
    let mut group_of: HashMap<usize, usize> = HashMap::new();
    for (t, b) in term_root {
        let Some(b) = b else { continue };
        let g = *group_of.entry(root(&mut parent, b)).or_insert_with(|| {
            groups.push((Vec::new(), Vec::new()));
            groups.len() - 1
        });
        groups[g].1.push(t);
    }
    for (i, b) in bodies.iter().enumerate() {
        if b.fixed {
            continue;
        }
        match group_of.get(&root(&mut parent, i)) {
            Some(&g) => groups[g].0.push(i),
            // Unconstrained and not dragged: free in all 6 directions.
            None if dof => {
                result.dof += 6;
                result.body_dof[i] = 6;
            }
            None => {}
        }
    }

    let eps = 1e-7;
    for (free, terms) in &groups {
        let problem = Problem { bodies, mates, relations, drags, terms: terms.clone() };
        let dragged = terms.iter().any(|t| matches!(t, Term::Drag(_)));
        result.iterations = result.iterations.max(levenberg_marquardt(&problem, &mut poses, free, scale, dragged));
    }

    // Mate residuals only (exclude drag terms) decide convergence.
    let mut mate_r = Vec::new();
    for m in mates {
        mate_residuals(&poses, m, &mut mate_r);
    }
    for rel in relations {
        mate_r.push(relation_residual(&poses, mates, rel));
    }
    result.max_residual = mate_r.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    result.converged = result.max_residual < 1e-6 * scale.max(1.0);
    fill_mate_status(&probe, &poses, &mut result);

    // DOF analysis on each group's constraint Jacobian (without drag rows),
    // evaluated at the final poses: null space of JᵀJ via a symmetric
    // eigendecomposition.
    for (free, terms) in groups.iter().filter(|_| dof) {
        let nodrag = Problem { bodies, mates, relations, drags, terms: terms.iter().copied().filter(|t| !matches!(t, Term::Drag(_))).collect() };
        let n = free.len() * 6;
        if nodrag.terms.is_empty() {
            result.dof += n;
            for &bi in free {
                result.body_dof[bi] = 6;
            }
            continue;
        }
        let mut base = Vec::new();
        residuals(&nodrag, &poses, &mut base);
        let j = jacobian(&nodrag, &poses, free, &base, scale, eps);
        let jtj = j.transpose() * &j;
        let eig = jtj.symmetric_eigen();
        let maxv = eig.eigenvalues.iter().fold(0.0f64, |a, v| a.max(v.abs()));
        let tol = 1e-8 * maxv.max(1.0);
        let null: Vec<usize> = (0..n).filter(|&i| eig.eigenvalues[i].abs() <= tol).collect();
        result.dof += null.len();
        for (k, &bi) in free.iter().enumerate() {
            // Rank of the null space restricted to this body's 6 columns.
            let mut m = DMatrix::<f64>::zeros(6, null.len());
            for (c, &ni) in null.iter().enumerate() {
                for r in 0..6 {
                    m[(r, c)] = eig.eigenvectors[(k * 6 + r, ni)];
                }
            }
            let rank = if null.is_empty() { 0 } else { m.svd(false, false).singular_values.iter().filter(|s| **s > 1e-6).count() };
            result.body_dof[bi] = rank.min(6);
        }
    }

    for (b, p) in bodies.iter_mut().zip(poses) {
        if !b.fixed {
            b.pose = p;
        }
    }
    result
}

/// Damped Gauss-Newton on one group's terms over its free bodies' poses (in
/// place). Returns the iterations used.
fn levenberg_marquardt(problem: &Problem, poses: &mut Vec<Frame>, free: &[usize], scale: f64, dragged: bool) -> usize {
    let n = free.len() * 6;
    let eps = 1e-7;
    let mut r = Vec::new();
    residuals(problem, poses, &mut r);
    if n == 0 || r.is_empty() {
        return 0;
    }
    // Damping adds lambda·(1 + diag) per variable; along the mates' free
    // directions the only curvature is the drag term's (DRAG_WEIGHT² = 1e-4),
    // so a drag starts nearly undamped (else each step follows ~10 % of it).
    let mut lambda = if dragged { 1e-9 } else { 1e-3 };
    let mut cost: f64 = r.iter().map(|v| v * v).sum();
    let mut iterations = 0;
    let mut r2 = Vec::new();
    for it in 0..100 {
        iterations = it + 1;
        if cost.sqrt() < 1e-10 {
            break;
        }
        let jac = jacobian(problem, poses, free, &r, scale, eps);
        let rv = DVector::from_vec(r.clone());
        let jt = jac.transpose();
        let jtj = &jt * &jac;
        let g = &jt * &rv;
        let mut improved = false;
        let mut stalled = false;
        for _ in 0..12 {
            let mut a = jtj.clone();
            for i in 0..n {
                a[(i, i)] += lambda * (1.0 + jtj[(i, i)]);
            }
            let Some(delta) = a.cholesky().map(|c| c.solve(&(-&g))) else {
                lambda *= 10.0;
                continue;
            };
            let mut trial = poses.clone();
            for (k, &bi) in free.iter().enumerate() {
                let mut d = [0.0; 6];
                for j in 0..6 {
                    d[j] = delta[k * 6 + j] * if j < 3 { scale } else { 1.0 };
                }
                trial[bi] = apply_delta(&poses[bi], &d);
            }
            residuals(problem, &trial, &mut r2);
            let c2: f64 = r2.iter().map(|v| v * v).sum();
            if c2 < cost {
                // Following a drag, the soft target may be out of reach
                // (Gauss-Newton then converges only linearly): stop once a
                // step gains less than 1e-6 of the cost.
                stalled = dragged && cost - c2 < 1e-6 * cost;
                *poses = trial;
                std::mem::swap(&mut r, &mut r2);
                cost = c2;
                lambda = (lambda * 0.3).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 10.0;
        }
        if !improved || stalled {
            break;
        }
    }
    iterations
}

fn fill_mate_status(p: &Problem, poses: &[Frame], result: &mut SolveResult) {
    let scale = characteristic_length(poses, p.mates).max(1e-3);
    for (i, m) in p.mates.iter().enumerate() {
        let mut r = Vec::new();
        mate_residuals(poses, m, &mut r);
        result.mate_ok[i] = r.iter().all(|v| v.abs() < 1e-5 * scale.max(1.0));
    }
}

fn characteristic_length(poses: &[Frame], mates: &[SolverMate]) -> f64 {
    let mut m: f64 = 0.0;
    for p in poses {
        m = m.max(p.origin.length());
    }
    for mt in mates {
        m = m.max(mt.a_local.origin.length()).max(mt.b_local.origin.length());
        for (_, f) in &mt.extra {
            m = m.max(f.origin.length());
        }
    }
    m.max(0.01)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(origin: DVec3, fixed: bool) -> SolverBody {
        SolverBody { pose: Frame::new(origin, DQuat::IDENTITY), fixed }
    }

    fn mate(kind: MateType, a: usize, la: Frame, b: usize, lb: Frame) -> SolverMate {
        SolverMate::new(kind, a, la, b, lb)
    }

    #[test]
    fn fastened_stacks_blocks_face_to_face() {
        // Block A fixed; top face connector at z=0.01 (normal +Z).
        // Block B: bottom face connector at its local z=0 with normal -Z.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.05, 0.03, -0.02), false)];
        let la = Frame::from_axes(DVec3::new(0.0, 0.0, 0.01), DVec3::Z, DVec3::X);
        let lb = Frame::from_axes(DVec3::ZERO, DVec3::NEG_Z, DVec3::X);
        let mates = vec![mate(MateType::Fastened, 0, la, 1, lb)];
        let r = solve(&mut bodies, &mates, &[], &[]);
        assert!(r.converged, "{r:?}");
        let p = bodies[1].pose;
        assert!((p.origin - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-7, "{:?}", p.origin);
        // B keeps its orientation (bottom face down).
        assert!((p.z() - DVec3::Z).length() < 1e-6);
        assert_eq!(r.dof, 0);
    }

    #[test]
    fn revolute_leaves_one_dof() {
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.01, 0.0, 0.0), false)];
        let la = Frame::from_axes(DVec3::ZERO, DVec3::Z, DVec3::X);
        let lb = Frame::from_axes(DVec3::ZERO, DVec3::NEG_Z, DVec3::X);
        let mates = vec![mate(MateType::Revolute, 0, la, 1, lb)];
        let r = solve(&mut bodies, &mates, &[], &[]);
        assert!(r.converged);
        assert_eq!(r.dof, 1);
        assert_eq!(r.body_dof[1], 1);
    }

    #[test]
    fn slider_with_drag_moves_along_axis_only() {
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::ZERO, false)];
        let la = Frame::from_axes(DVec3::ZERO, DVec3::Z, DVec3::X);
        let lb = Frame::from_axes(DVec3::ZERO, DVec3::NEG_Z, DVec3::X);
        let mates = vec![mate(MateType::Slider, 0, la, 1, lb)];
        let drags = vec![DragTarget { body: 1, local_point: DVec3::ZERO, target: DVec3::new(0.02, 0.0, 0.05) }];
        let r = solve(&mut bodies, &mates, &[], &drags);
        assert!(r.converged);
        let o = bodies[1].pose.origin;
        assert!(o.x.abs() < 1e-6 && o.y.abs() < 1e-6, "{o:?}");
        assert!((o.z - 0.05).abs() < 1e-3, "{o:?}");
    }

    /// The six everyday mates (Onshape: Fastened 0 DOF; Revolute rotate Z;
    /// Slider translate Z; Cylindrical translate + rotate Z; Planar translate
    /// X, Y + rotate Z; Ball rotate X, Y, Z): their DOF and which way a drag
    /// of the moving part's origin can carry it.
    #[test]
    fn core_mates_leave_their_degrees_of_freedom() {
        // (mate, DOF, which of the dragged x, y, z the origin follows)
        let cases = [
            (MateType::Fastened, 0, [false, false, false]),
            (MateType::Revolute, 1, [false, false, false]),
            (MateType::Slider, 1, [false, false, true]),
            (MateType::Cylindrical, 2, [false, false, true]),
            (MateType::Planar, 3, [true, true, false]),
            (MateType::Ball, 3, [false, false, false]),
        ];
        let la = Frame::from_axes(DVec3::ZERO, DVec3::Z, DVec3::X);
        let lb = Frame::from_axes(DVec3::ZERO, DVec3::NEG_Z, DVec3::X);
        let target = DVec3::new(0.02, -0.03, 0.05);
        for (kind, dof, follows) in cases {
            let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::ZERO, false)];
            let mates = vec![mate(kind, 0, la, 1, lb)];
            let r = solve(&mut bodies, &mates, &[], &[]);
            assert!(r.converged, "{kind:?}");
            assert_eq!(r.dof, dof, "{kind:?} DOF");
            assert_eq!(r.body_dof[1], dof, "{kind:?} body DOF");
            let drags = vec![DragTarget { body: 1, local_point: DVec3::ZERO, target }];
            let r = solve(&mut bodies, &mates, &[], &drags);
            assert!(r.converged, "{kind:?} drag");
            let o = bodies[1].pose.origin;
            for (k, f) in follows.iter().enumerate() {
                let want = if *f { target[k] } else { 0.0 };
                assert!((o[k] - want).abs() < 1e-3, "{kind:?}: origin {o:?}, axis {k} should be {want}");
            }
        }
        // Ball: dragging a point off the centre turns the part about it.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::ZERO, false)];
        let mates = vec![mate(MateType::Ball, 0, la, 1, lb)];
        let drags = vec![DragTarget { body: 1, local_point: DVec3::new(0.01, 0.0, 0.0), target: DVec3::new(0.0, 0.0, 0.01) }];
        let r = solve(&mut bodies, &mates, &[], &drags);
        assert!(r.converged);
        let p = bodies[1].pose;
        assert!(p.origin.length() < 1e-6, "{:?}", p.origin);
        assert!((p.transform_point(DVec3::new(0.01, 0.0, 0.0)) - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-3);
    }

    /// 20 links (60 mm, holes 44 mm apart) chained by revolutes, link 0 fixed.
    fn chain(n: usize) -> (Vec<SolverBody>, Vec<SolverMate>) {
        let mm = 1e-3;
        let bodies: Vec<SolverBody> = (0..n).map(|i| body(DVec3::new(44.0 * i as f64 * mm, 0.0, 5.0 * i as f64 * mm), i == 0)).collect();
        let top = Frame::from_axes(DVec3::new(52.0 * mm, 6.0 * mm, 5.0 * mm), DVec3::Z, DVec3::X);
        let bot = Frame::from_axes(DVec3::new(8.0 * mm, 6.0 * mm, 0.0), DVec3::NEG_Z, DVec3::X);
        let mates = (1..n).map(|i| mate(MateType::Revolute, i, bot, i - 1, top)).collect();
        (bodies, mates)
    }

    /// Dragging the free end of a 20-link revolute chain: each step follows
    /// the pointer exactly, and the drag phase starts nearly undamped (it
    /// took 111 iterations over these 5 steps with the old damping).
    #[test]
    fn chain_drag_follows_in_few_iterations() {
        let (mut bodies, mates) = chain(20);
        assert!(solve(&mut bodies, &mates, &[], &[]).converged);
        let lp = DVec3::new(0.03, 0.006, 0.0);
        let mut iterations = 0;
        for _ in 0..5 {
            let target = bodies[19].pose.transform_point(lp) + DVec3::new(-0.002, 0.001, 0.0);
            let drags = vec![DragTarget { body: 19, local_point: lp, target }];
            iterations += solve_inner(&mut bodies.clone(), &mates, &[], &drags, false).iterations;
            let r = solve(&mut bodies, &mates, &[], &drags);
            assert!(r.converged);
            assert_eq!(r.dof, 19);
            assert!((bodies[19].pose.transform_point(lp) - target).length() < 1e-6);
        }
        assert!(iterations <= 80, "{iterations} drag-phase iterations");
    }

    /// A revolute pair among 18 unconstrained parts (20 in all): each part is
    /// its own group, so the unrelated parts keep 6 DOF each and a drag of the
    /// pin leaves them where they are.
    #[test]
    fn drag_moves_only_its_group() {
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::ZERO, false)];
        bodies.extend((0..18).map(|i| body(DVec3::new(0.1 * i as f64, 0.2, 0.0), false)));
        let la = Frame::from_axes(DVec3::ZERO, DVec3::Z, DVec3::X);
        let lb = Frame::from_axes(DVec3::ZERO, DVec3::NEG_Z, DVec3::X);
        let mates = vec![mate(MateType::Revolute, 1, la, 0, lb)];
        let r = solve(&mut bodies, &mates, &[], &[]);
        assert!(r.converged);
        assert_eq!(r.dof, 1 + 18 * 6);
        assert_eq!(r.body_dof[1], 1);
        assert!(r.body_dof[2..].iter().all(|d| *d == 6));
        let before: Vec<DVec3> = bodies.iter().map(|b| b.pose.origin).collect();
        let drags = vec![DragTarget { body: 1, local_point: DVec3::X * 0.01, target: DVec3::Y * 0.01 }];
        let r = solve(&mut bodies, &mates, &[], &drags);
        assert!(r.converged);
        assert!((bodies[1].pose.transform_point(DVec3::X * 0.01) - DVec3::Y * 0.01).length() < 1e-6);
        assert!(bodies[2..].iter().zip(&before[2..]).all(|(b, o)| b.pose.origin == *o && b.pose.rotation == DQuat::IDENTITY));
    }

    #[test]
    fn tangent_sphere_on_plane_and_sphere_on_sphere() {
        // Ball (r = 10 mm) on a fixed plane z = 0 (normal +Z): center at z = 10 mm, 5 DOF left.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.03, 0.01, 0.05), false)];
        let mut m = mate(MateType::Tangent, 0, Frame::default(), 1, Frame::default());
        m.geom = Some(TangentPair::new(TangentGeom::Plane { point: DVec3::ZERO, normal: DVec3::Z }, TangentGeom::Sphere { center: DVec3::ZERO, radius: 0.01 }, false));
        let r = solve(&mut bodies, &[m.clone()], &[], &[]);
        assert!(r.converged, "{r:?}");
        assert!((bodies[1].pose.origin.z - 0.01).abs() < 1e-8, "{:?}", bodies[1].pose.origin);
        assert_eq!(r.body_dof[1], 5);
        // Flip puts the ball below the plane.
        m.flip_primary = true;
        let r = solve(&mut bodies, &[m], &[], &[]);
        assert!(r.converged);
        assert!((bodies[1].pose.origin.z + 0.01).abs() < 1e-8);
        // Two balls (10 mm and 5 mm) touching externally: centers 15 mm apart.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.05, 0.0, 0.0), false)];
        let mut m = mate(MateType::Tangent, 0, Frame::default(), 1, Frame::default());
        m.geom = Some(TangentPair::new(TangentGeom::Sphere { center: DVec3::ZERO, radius: 0.01 }, TangentGeom::Sphere { center: DVec3::ZERO, radius: 0.005 }, false));
        let r = solve(&mut bodies, &[m], &[], &[]);
        assert!(r.converged);
        assert!((bodies[1].pose.origin.length() - 0.015).abs() < 1e-8);
    }

    #[test]
    fn tangent_cone_and_torus_residuals() {
        // Cone (half angle 30°) lying on the plane z = 0: apex on the plane,
        // axis tilted 30° up; 4 DOF left.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.01, 0.0, 0.03), false)];
        let mut m = mate(MateType::Tangent, 0, Frame::default(), 1, Frame::default());
        let half = 30f64.to_radians();
        m.geom = Some(TangentPair::new(TangentGeom::Plane { point: DVec3::ZERO, normal: DVec3::Z }, TangentGeom::Cone { apex: DVec3::ZERO, axis: DVec3::new(1.0, 0.0, 1.0).normalize(), half_angle: half }, false));
        let r = solve(&mut bodies, &[m], &[], &[]);
        assert!(r.converged, "{r:?}");
        let p = bodies[1].pose;
        assert!(p.origin.z.abs() < 1e-8);
        let axis = p.rotation * DVec3::new(1.0, 0.0, 1.0).normalize();
        assert!((axis.z - half.sin()).abs() < 1e-7, "{axis:?}");
        assert_eq!(r.body_dof[1], 4);
        // Torus (R 10 mm, r 2 mm) lying flat on the plane: center 2 mm up; tilted: R sinθ + r.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.0, 0.0, 0.05), false)];
        let mut m = mate(MateType::Tangent, 0, Frame::default(), 1, Frame::default());
        m.geom = Some(TangentPair::new(TangentGeom::Plane { point: DVec3::ZERO, normal: DVec3::Z }, TangentGeom::Torus { center: DVec3::ZERO, axis: DVec3::new(0.0, 0.6, 0.8), major: 0.01, minor: 0.002 }, false));
        let r = solve(&mut bodies, &[m], &[], &[]);
        assert!(r.converged, "{r:?}");
        let a = bodies[1].pose.rotation * DVec3::new(0.0, 0.6, 0.8);
        let expect = 0.01 * (1.0 - a.z * a.z).sqrt() + 0.002;
        assert!((bodies[1].pose.origin.z - expect).abs() < 1e-8);
        // Ball (r 3 mm) in a circular edge (torus with minor 0, R 2 mm): seated 2.236 mm above.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.0, 0.0, 0.01), false)];
        let mut m = mate(MateType::Tangent, 0, Frame::default(), 1, Frame::default());
        m.geom = Some(TangentPair::new(TangentGeom::Torus { center: DVec3::ZERO, axis: DVec3::Z, major: 0.002, minor: 0.0 }, TangentGeom::Sphere { center: DVec3::ZERO, radius: 0.003 }, false));
        let r = solve(&mut bodies, &[m], &[], &[]);
        assert!(r.converged);
        let o = bodies[1].pose.origin;
        let g = TangentGeom::Torus { center: DVec3::ZERO, axis: DVec3::Z, major: 0.002, minor: 0.0 };
        assert!((g.signed_distance(o) - 0.003).abs() < 1e-9);
        // Unsupported pairs.
        assert!(!tangent_supported(&TangentGeom::Cone { apex: DVec3::ZERO, axis: DVec3::Z, half_angle: 0.3 }, &TangentGeom::Cylinder { point: DVec3::ZERO, axis: DVec3::Z, radius: 0.01 }));
    }

    #[test]
    fn closest_points_lie_on_surfaces() {
        let q = DVec3::new(0.3, -0.2, 0.5);
        for g in [
            TangentGeom::Plane { point: DVec3::new(0.0, 0.0, 0.1), normal: DVec3::Z },
            TangentGeom::Sphere { center: DVec3::X, radius: 0.2 },
            TangentGeom::Cylinder { point: DVec3::ZERO, axis: DVec3::Y, radius: 0.1 },
            TangentGeom::Cone { apex: DVec3::ZERO, axis: DVec3::Z, half_angle: 0.4 },
            TangentGeom::Torus { center: DVec3::ZERO, axis: DVec3::Z, major: 0.3, minor: 0.05 },
        ] {
            let c = g.closest_point(q);
            assert!(g.signed_distance(c).abs() < 1e-9, "{g:?}: {}", g.signed_distance(c));
        }
    }

    #[test]
    fn gear_relation_couples_rotations() {
        // Two revolute mates to ground, with a 2:1 gear relation.
        let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::ZERO, false), body(DVec3::new(0.03, 0.0, 0.0), false)];
        let za = Frame::from_axes(DVec3::ZERO, DVec3::Z, DVec3::X);
        let zb = Frame::from_axes(DVec3::new(0.03, 0.0, 0.0), DVec3::Z, DVec3::X);
        let down = Frame::from_axes(DVec3::ZERO, DVec3::NEG_Z, DVec3::X);
        let mates = vec![mate(MateType::Revolute, 0, za, 1, down), mate(MateType::Revolute, 0, zb, 2, down)];
        let poses0: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        let base_a = mate_values(&poses0, &mates[0])[0];
        let base_b = mate_values(&poses0, &mates[1])[0];
        let rel = vec![SolverRelation { kind: RelationKind::Gear, mate_a: 0, mate_b: 1, ratio: 2.0, reverse: false, base_a, base_b, value_a: 0, value_b: 0 }];
        // Drag body 1 to rotate it by pulling a point.
        let drags = vec![DragTarget { body: 1, local_point: DVec3::new(0.01, 0.0, 0.0), target: DVec3::new(0.0, 0.01, 0.0) }];
        let r = solve(&mut bodies, &mates, &rel, &drags);
        assert!(r.converged, "{r:?}");
        let poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        let a = wrap_angle(mate_values(&poses, &mates[0])[0] - base_a);
        let b = wrap_angle(mate_values(&poses, &mates[1])[0] - base_b);
        assert!(a.abs() > 0.5, "driver should rotate: {a}");
        assert!((b + 2.0 * a).abs() < 1e-4 || (wrap_angle(b + 2.0 * a)).abs() < 1e-4, "a={a} b={b}");
    }
}
