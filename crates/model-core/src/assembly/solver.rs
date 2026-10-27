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

/// Free values of a mate (rotation about Z, translations) as currently posed:
/// live's sense, the first connector relative to the second, so `relative`
/// (the second seen from the first) negated (oracle_asm S1: B's bottom 15 mm
/// above A's top reads −15 mm; S2's limits 0..10 mm then seat it on A, R2's
/// 0..45° turn a pin started at 90° back to 0°).
pub fn mate_values(bodies: &[Frame], m: &SolverMate) -> [f64; 4] {
    let r = relative(bodies, m);
    let x = r.rotation * DVec3::X;
    let angle = x.y.atan2(x.x);
    [-angle, -r.origin.x, -r.origin.y, -r.origin.z]
}

/// A Width mate's connectors grouped by part (api benchmark F19: the
/// block's two faces given first, the base's two after, placed the block
/// right but read as conflicting): the tab pair is the connectors on the
/// first one's part, the width pair the two on the other part, in whatever
/// order they were picked (live solves every order). Layout: `a` tab 1, `b`
/// width 1, `extra` [width 2, tab 2?].
pub fn pair_width_connectors(m: &mut SolverMate) {
    let mut all = vec![(m.a, m.a_local), (m.b, m.b_local)];
    all.extend(m.extra.iter().copied());
    let first = all[0].0;
    let (tabs, widths): (Vec<_>, Vec<_>) = all.into_iter().partition(|c| c.0 == first);
    if widths.len() != 2 || widths[0].0 != widths[1].0 || !(1..=2).contains(&tabs.len()) {
        return;
    }
    (m.a, m.a_local) = tabs[0];
    (m.b, m.b_local) = widths[0];
    m.extra = std::iter::once(widths[1]).chain(tabs.get(1).copied()).collect();
}

/// `m` as a Fastened mate holding the free values `v` ([`mate_values`]'s
/// sense): the turn `-v[0]` and the offset that puts the first connector at
/// `v[1..]` from the second (Apply limit position, named positions, locks).
pub fn fasten_at(m: &mut SolverMate, v: [f64; 4]) {
    // The offset and rotation move the first connector in its own frame,
    // as the values measure it.
    m.kind = MateType::Fastened;
    m.rotation = v[0];
    m.offset = DVec3::new(v[1], v[2], v[3]);
    m.limits.clear();
}

/// The offset along Z that holds a slider at `value` ([`mate_values`]'s sense).
pub fn slider_offset_z(_m: &SolverMate, value: f64) -> f64 {
    value
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
    // Offset and rotation move the first connector along and about its own
    // Z, relative to the second: rel.origin = -offset in the first's frame,
    // rel = Rz(-rotation) (live assembly/740–748: the plate picked first ends
    // 10 mm above the other, both Z up; live oracle_asm F2: a block fastened
    // bottom on top with offset 5 / 90° sinks 5 mm into the other and turns
    // −90° about world z, its connector's Z pointing down).
    let t = r.origin + m.offset;
    let rot_target = DQuat::from_rotation_z(-m.rotation);
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
    /// Characteristic length (rotation residuals in length units).
    scale: f64,
}

const DRAG_WEIGHT: f64 = 1e-2;
/// Pull of an undragged body toward its pose before the drag step (least
/// change): 10⁴× weaker than the drag, so a linkage the drag must move still
/// follows it to a fraction of a micrometre (near-straight chains move many
/// links far for a little end motion), but it
/// pins the free directions the drag doesn't ask for (qa4 DRAG-RUNAWAY: a
/// Planar stack's cubes flung millions of mm away).
const STAY_WEIGHT: f64 = 1e-6;

/// One block of residual rows: a mate (with its held values), a relation or
/// a drag, in the order `residuals` writes them.
#[derive(Clone, Copy)]
enum Term {
    Mate(usize),
    Relation(usize),
    Drag(usize),
    /// A body's change from its pose at the start of a drag step.
    Stay(usize),
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
        Term::Stay(b) => vec![b],
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
        Term::Stay(b) => {
            let (s, c) = (p.bodies[b].pose, poses[b]);
            let t = (c.origin - s.origin) * STAY_WEIGHT;
            let r = log_rot(s.rotation.inverse() * c.rotation) * (STAY_WEIGHT * p.scale);
            out.extend_from_slice(&[t.x, t.y, t.z, r.x, r.y, r.z]);
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
    orient_axial_mates(bodies, mates);
    seat_cone_limits(bodies, mates);
    if !drags.is_empty()
        && let Some(r) = solve_dragged_only(bodies, mates, relations, drags, dof)
    {
        return r;
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

/// Bodies set to a requested pose (`placed`: Transform's typed values, the
/// triad's rotation) projected onto the mates exactly: the placed bodies
/// alone first, every other body held (as a drag moves only the dragged
/// part); when their mates can't hold that way, the whole assembly from the
/// requested poses. Either way the least change from the request
/// ([`least_change`]): live keeps a typed in-plane value of a Planar-mated
/// part exactly.
pub fn solve_placed(bodies: &mut [SolverBody], mates: &[SolverMate], relations: &[SolverRelation], placed: &[usize], dof: bool) -> SolveResult {
    let mut trial: Vec<SolverBody> = bodies.iter().enumerate().map(|(i, b)| SolverBody { pose: b.pose, fixed: b.fixed || !placed.contains(&i) }).collect();
    let r = solve_with(&mut trial, mates, relations, &[], false);
    let held = if relations.is_empty() { mates.iter().enumerate().filter(|(_, m)| mate_bodies(m).any(|b| placed.contains(&b))).all(|(i, _)| r.mate_ok[i]) } else { r.converged };
    if !held {
        return solve_with(bodies, mates, relations, &[], dof);
    }
    for (b, t) in bodies.iter_mut().zip(&trial) {
        if !b.fixed {
            b.pose = t.pose;
        }
    }
    if dof { solve_with(bodies, mates, relations, &[], true) } else { r }
}

/// Least change first (live DRAG-RUNAWAY: dragging the top cube of a Planar
/// stack moves only that cube; Onshape answers each step in ≈20 ms whatever
/// the stack's size): the dragged bodies alone are solved, every other body
/// held. Taken when the mates hold and the drag points reach their targets;
/// otherwise (a linkage the drag has to move) the whole group is solved.
fn solve_dragged_only(bodies: &mut [SolverBody], mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget], dof: bool) -> Option<SolveResult> {
    let moving: Vec<usize> = drags.iter().map(|d| d.body).filter(|&b| !bodies[b].fixed).collect();
    if moving.is_empty() {
        return None;
    }
    let mut trial: Vec<SolverBody> = bodies.iter().enumerate().map(|(i, b)| SolverBody { pose: b.pose, fixed: b.fixed || !moving.contains(&i) }).collect();
    solve_inner(&mut trial, mates, relations, drags, false);
    let r = solve_inner(&mut trial, mates, relations, &[], false);
    let reached = drags.iter().all(|d| trial[d.body].pose.transform_point(d.local_point).distance(d.target) < 1e-7);
    // The dragged bodies' mates must hold (a failing mate elsewhere isn't
    // this step's business); with relations, everything must.
    let held = if relations.is_empty() { mates.iter().enumerate().filter(|(_, m)| mate_bodies(m).any(|b| moving.contains(&b))).all(|(i, _)| r.mate_ok[i]) } else { r.converged };
    if !held {
        return None;
    }
    // A target off what the dragged bodies alone can reach (the usual case:
    // the pointer moves in the view plane) is still their answer unless
    // moving the rest of their group gets the drag closer: one damped
    // Gauss-Newton step on the whole group must find (almost) nothing.
    if !reached {
        let poses: Vec<Frame> = trial.iter().map(|b| b.pose).collect();
        let scale = characteristic_length(&poses, mates).max(1e-3);
        let held_bodies: &[SolverBody] = bodies;
        let (groups, _) = group_terms(held_bodies, mates, relations, drags, false);
        for (free, terms) in groups.iter().filter(|(_, ts)| ts.iter().any(|t| matches!(t, Term::Drag(_)))) {
            let problem = Problem { bodies: held_bodies, mates, relations, drags, terms: terms.clone(), scale };
            let mut res = Vec::new();
            residuals(&problem, &poses, &mut res);
            // The mates near rigid (the drag rows a further 1000× softer: a
            // soft mate would let the step "help" the drag by bending every
            // mate a little, a gain growing with the group), and a constant,
            // tiny damping: relative damping would smother a coupled motion
            // the mates make stiff on the diagonal (a gear).
            let mut jac = jacobian(&problem, &poses, free, &res, scale, 1e-7);
            let drag_rows = 3 * terms.iter().filter(|t| matches!(t, Term::Drag(_))).count();
            for row in res.len() - drag_rows..res.len() {
                res[row] *= 1e-3;
                for c in 0..jac.ncols() {
                    jac[(row, c)] *= 1e-3;
                }
            }
            let cost: f64 = res.iter().map(|v| v * v).sum();
            // Predicted cost drop of one Gauss-Newton step over `cols`.
            let gain = |cols: &[usize]| -> Option<f64> {
                let j = jac.select_columns(cols.iter());
                let jt = j.transpose();
                let mut a = &jt * &j;
                for i in 0..a.nrows() {
                    a[(i, i)] += 1e-16;
                }
                let g = &jt * DVector::from_column_slice(&res);
                let delta = a.cholesky()?.solve(&(-&g));
                Some(-g.dot(&delta))
            };
            let all: Vec<usize> = (0..jac.ncols()).collect();
            let own: Vec<usize> = free.iter().enumerate().filter(|(_, b)| moving.contains(b)).flat_map(|(k, _)| k * 6..k * 6 + 6).collect();
            let (Some(g_all), Some(g_own)) = (gain(&all), gain(&own)) else { return None };
            if g_all - g_own > 1e-3 * cost {
                return None;
            }
        }
    }
    for (b, t) in bodies.iter_mut().zip(&trial) {
        if !b.fixed {
            b.pose = t.pose;
        }
    }
    // The degrees of freedom are the whole assembly's (nothing held).
    Some(if dof { solve_inner(bodies, mates, relations, &[], true) } else { r })
}

/// Mates that align the Z axes only as lines (Revolute, Cylindrical, Pin
/// slot, Planar, Parallel: their residual is Z's x and y, which an opposed Z
/// meets too) start on the requested side: a moving body whose connector Z
/// points the wrong way is turned 180° about its connector's X axis (live
/// behaviours "qa4 round 5 s3 mate flip": Flip primary axis turns the pin
/// over for Revolute, Cylindrical and Planar).
fn orient_axial_mates(bodies: &mut [SolverBody], mates: &[SolverMate]) {
    for m in mates {
        if m.follow.is_some() || !matches!(m.kind, MateType::Revolute | MateType::Cylindrical | MateType::PinSlot | MateType::Planar | MateType::Parallel) {
            continue;
        }
        let poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        if (relative(&poses, m).rotation * DVec3::Z).z >= 0.0 {
            continue;
        }
        let (moving, local) = if !bodies[m.a].fixed {
            (m.a, m.a_local)
        } else if !bodies[m.b].fixed {
            (m.b, m.b_local)
        } else {
            continue;
        };
        let c = bodies[moving].pose.mul(&local);
        let turn = DQuat::from_axis_angle(c.rotation * DVec3::X, std::f64::consts::PI);
        let p = bodies[moving].pose;
        bodies[moving].pose = Frame { origin: c.origin + turn * (p.origin - c.origin), rotation: (turn * p.rotation).normalize() };
    }
}

/// A Ball mate's cone limit broken at the start is met by the least turn
/// (live oracle_asm B2: a block started upright on a 30° cone about the
/// face-to-face direction turns 150° about y): the moving connector's
/// axis turns toward the cone's centre by the excess, about their cross
/// product, about its own Y when they are exactly opposed (the limit
/// residual has no direction there), around the connector's origin.
fn seat_cone_limits(bodies: &mut [SolverBody], mates: &[SolverMate]) {
    for m in mates {
        if m.kind != MateType::Ball || m.follow.is_some() {
            continue;
        }
        let Some(max) = m.limits.iter().find(|l| l.0 == 4).and_then(|l| l.2) else { continue };
        let poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        let (fa, fb) = (poses[m.a].mul(&m.a_local), poses[m.b].mul(&m.b_local));
        // The cone value is the angle between `a`'s Z and `b`'s Z with the
        // base alignment taken out.
        let za = fa.z();
        let ub = fb.rotation * (base_rotation(m.flip_primary, m.reorient).inverse() * DVec3::Z);
        let angle = za.dot(ub).clamp(-1.0, 1.0).acos();
        if angle <= max + 1e-12 {
            continue;
        }
        let (moving, conn, from, to) = if !bodies[m.a].fixed {
            (m.a, fa, za, ub)
        } else if !bodies[m.b].fixed {
            (m.b, fb, ub, za)
        } else {
            continue;
        };
        let cross = from.cross(to);
        let axis = if cross.length() > 1e-9 { cross.normalize() } else { conn.y() };
        let turn = DQuat::from_axis_angle(axis, angle - max);
        let p = bodies[moving].pose;
        bodies[moving].pose = Frame { origin: conn.origin + turn * (p.origin - conn.origin), rotation: (turn * p.rotation).normalize() };
    }
}

/// Free bodies linked by a term (fixed bodies link nothing: they don't move)
/// form groups solved one by one: a drag moves only its group, and the dense
/// linear algebra scales with the group, not the assembly. With `stays`
/// (following a drag) every other free body also gets its stay term. Returns
/// the groups (free bodies, terms in residual order: mates, relations, drags,
/// stays) and the free bodies no term reads.
/// A group: its free bodies and its terms.
type Group = (Vec<usize>, Vec<Term>);

fn group_terms(bodies: &[SolverBody], mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget], stays: bool) -> (Vec<Group>, Vec<usize>) {
    let mut parent: Vec<usize> = (0..bodies.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let probe = Problem { bodies, mates, relations, drags, terms: Vec::new(), scale: 1.0 };
    let mut groups: Vec<Group> = Vec::new();
    let mut term_root: Vec<(Term, Option<usize>)> = Vec::new();
    let stay_terms: Vec<Term> = if stays && !drags.is_empty() { (0..bodies.len()).filter(|&b| !bodies[b].fixed && !drags.iter().any(|d| d.body == b)).map(Term::Stay).collect() } else { Vec::new() };
    for t in all_terms(mates, relations, drags).chain(stay_terms) {
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
    let mut loose = Vec::new();
    for (i, b) in bodies.iter().enumerate() {
        if b.fixed {
            continue;
        }
        match group_of.get(&root(&mut parent, i)) {
            Some(&g) => groups[g].0.push(i),
            None => loose.push(i),
        }
    }
    (groups, loose)
}

fn solve_inner(bodies: &mut [SolverBody], mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget], dof: bool) -> SolveResult {
    let mut poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
    let scale = characteristic_length(&poses, mates).max(1e-3);
    let mut result = SolveResult { mate_ok: vec![false; mates.len()], body_dof: vec![0; bodies.len()], ..Default::default() };
    let probe = Problem { bodies, mates, relations, drags, terms: Vec::new(), scale };
    let (groups, loose) = group_terms(bodies, mates, relations, drags, true);
    if dof {
        // Unconstrained and not dragged: free in all 6 directions.
        for i in loose {
            result.dof += 6;
            result.body_dof[i] = 6;
        }
    }

    let eps = 1e-7;
    for (free, terms) in &groups {
        let problem = Problem { bodies, mates, relations, drags, terms: terms.clone(), scale };
        let dragged = terms.iter().any(|t| matches!(t, Term::Drag(_)));
        result.iterations = result.iterations.max(levenberg_marquardt(&problem, &mut poses, free, scale, dragged));
        if !dragged && free.iter().any(|&b| poses[b] != bodies[b].pose) {
            least_change(&problem, &mut poses, free, scale);
        }
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
        let mut nodrag = Problem { bodies, mates, relations, drags, terms: terms.iter().copied().filter(|t| !matches!(t, Term::Drag(_) | Term::Stay(_))).collect(), scale };
        // The group's own length scale, so the analysis (and its cache key)
        // doesn't change when an instance far away is inserted or moved.
        nodrag.scale = group_length(&nodrag, &poses);
        let n = free.len() * 6;
        if nodrag.terms.is_empty() {
            result.dof += n;
            for &bi in free {
                result.body_dof[bi] = 6;
            }
            continue;
        }
        let (key, ids) = dof_key(&nodrag, &poses, free);
        if let Some((d, ranks)) = DOF_CACHE.get_or_init(Default::default).lock().ok().and_then(|c| c.get(&key).cloned()) {
            result.dof += d;
            for &bi in free {
                result.body_dof[bi] = ranks[ids[&bi]];
            }
            continue;
        }
        let mut base = Vec::new();
        residuals(&nodrag, &poses, &mut base);
        let j = jacobian(&nodrag, &poses, free, &base, nodrag.scale, eps);
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
        if let Ok(mut c) = DOF_CACHE.get_or_init(Default::default).lock() {
            if c.len() > 4096 {
                c.clear();
            }
            let mut ranks = vec![0; ids.len()];
            for &bi in free {
                ranks[ids[&bi]] = result.body_dof[bi];
            }
            c.insert(key, (null.len(), ranks));
        }
    }

    for (b, p) in bodies.iter_mut().zip(poses) {
        if !b.fixed {
            b.pose = p;
        }
    }
    result
}

/// DOF analyses of mate groups (degrees of freedom; each body's, by its
/// number in [`dof_key`]) by an exact key of what decides them: an evaluation after adding one mate
/// re-analyses only the groups it changed (200 pins in 20 stacks spent ~40 ms
/// of eigen-decompositions on every evaluation).
type DofCache = std::sync::Mutex<HashMap<u64, (usize, Vec<usize>)>>;
static DOF_CACHE: std::sync::OnceLock<DofCache> = std::sync::OnceLock::new();

/// The group's terms (every field of their mates and relations), the poses
/// of the bodies they read and which are fixed, the free bodies and the
/// scale, hashed exactly (`Debug` writes floats round-trip). Bodies are
/// numbered by first use in the terms, so the key survives a renumbering
/// (an instance inserted elsewhere); returns the key and each body's number.
fn dof_key(p: &Problem, poses: &[Frame], free: &[usize]) -> (u64, HashMap<usize, usize>) {
    use std::fmt::Write;
    use std::hash::{Hash, Hasher};
    let mut ids: HashMap<usize, usize> = HashMap::new();
    for &t in &p.terms {
        for b in term_bodies(p, t) {
            let n = ids.len();
            ids.entry(b).or_insert(n);
        }
    }
    let canon = |m: &SolverMate| {
        let mut m = m.clone();
        m.a = ids[&m.a];
        m.b = ids[&m.b];
        for e in &mut m.extra {
            e.0 = ids[&e.0];
        }
        if let Some(f) = &mut m.follow {
            f.axis_body = ids[&f.axis_body];
        }
        m
    };
    let mut s = String::new();
    let _ = write!(s, "{:?}|", p.scale);
    for &t in &p.terms {
        match t {
            Term::Mate(i) => {
                let _ = write!(s, "M{:?}", canon(&p.mates[i]));
            }
            Term::Relation(i) => {
                let r = SolverRelation { mate_a: 0, mate_b: 0, ..p.relations[i].clone() };
                let _ = write!(s, "R{r:?}{:?}{:?}", canon(&p.mates[p.relations[i].mate_a]), canon(&p.mates[p.relations[i].mate_b]));
            }
            Term::Drag(_) | Term::Stay(_) => {}
        }
    }
    let mut by_id: Vec<(usize, usize)> = ids.iter().map(|(&b, &n)| (n, b)).collect();
    by_id.sort_unstable();
    for (n, b) in by_id {
        let _ = write!(s, "B{n}{:?}{}{}", poses[b], p.bodies[b].fixed, free.contains(&b));
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    (h.finish(), ids)
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
    // The stay rows (last in a group's terms) aren't progress: stalling is
    // judged on the drag and mate rows alone.
    let stay_rows = 6 * problem.terms.iter().filter(|t| matches!(t, Term::Stay(_))).count();
    let core = |v: &[f64]| v[..v.len() - stay_rows].iter().map(|x| x * x).sum::<f64>();
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
                let (c0, c1) = (core(&r), core(&r2));
                stalled = dragged && c0 - c1 < 1e-6 * c0;
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

/// Least change, exactly (live: Transform keeps the coordinates the mates
/// leave free; api benchmark F9: a Planar-mated part moved to [30, 5, 40] mm
/// ends at exactly [30, 5, 12]; oracle_asm M2: a part turning on a Revolute
/// keeps its start turn while the part under it is fastened back from 30°).
/// Levenberg-Marquardt's damped path drifts along the free directions; from
/// its solution, Gauss-Newton steps `δ = N Nᵀ Δ − J⁺ r`: along the mates'
/// free directions `N` (the null space of the Jacobian) back toward the start
/// poses `Δ`, and the minimum-norm correction of what's left of the mate
/// rows. A body with no free direction isn't pulled at all (its pull back to
/// a far start would leak into the others' free directions through the
/// finite-difference Jacobian). Kept only when the mates hold as well as
/// before.
fn least_change(problem: &Problem, poses: &mut [Frame], free: &[usize], scale: f64) {
    let n = free.len() * 6;
    let mut r = Vec::new();
    residuals(problem, poses, &mut r);
    let cost = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>().sqrt();
    let before = cost(&r).max(1e-10);
    let mut trial = poses.to_vec();
    // About 10× closer each step (oracle R1: 0.1 rad → 2e-8 after 6); the
    // fine difference step keeps the null space exact enough to go on.
    for _ in 0..12 {
        let jac = jacobian(problem, &trial, free, &r, scale, 1e-9);
        let eig = (jac.transpose() * &jac).symmetric_eigen();
        let maxv = eig.eigenvalues.iter().fold(0.0f64, |a, v| a.max(v.abs()));
        let null: Vec<usize> = (0..n).filter(|&i| eig.eigenvalues[i].abs() <= 1e-8 * maxv.max(1.0)).collect();
        let mut basis = DMatrix::<f64>::from_fn(n, null.len(), |row, c| eig.eigenvectors[(row, null[c])]);
        // The change is measured at each body's mate connectors (their mean),
        // not at its part origin (oracle_asm R1: a pin on a Revolute keeps
        // its turn, where pulling its far part origin back turned it −6°):
        // in coordinates (anchor move, turn), anchor move = t + w × lever.
        let levers: Vec<DVec3> = free.iter().map(|&bi| trial[bi].rotation * anchor(problem, bi) / scale).collect();
        let to_anchor = |v: &mut DMatrix<f64>, sign: f64| {
            for (k, l) in levers.iter().enumerate() {
                for c in 0..v.ncols() {
                    let w = DVec3::new(v[(k * 6 + 3, c)], v[(k * 6 + 4, c)], v[(k * 6 + 5, c)]);
                    let m = w.cross(*l) * sign;
                    for j in 0..3 {
                        v[(k * 6 + j, c)] += m[j];
                    }
                }
            }
        };
        to_anchor(&mut basis, 1.0);
        let basis = if null.is_empty() { basis } else { basis.qr().q().columns(0, null.len()).into_owned() };
        let mut back = DMatrix::<f64>::zeros(n, 1);
        for (k, &bi) in free.iter().enumerate() {
            if (0..6).all(|j| (0..null.len()).all(|c| basis[(k * 6 + j, c)].abs() < 1e-6)) {
                continue;
            }
            let (s, c) = (problem.bodies[bi].pose, trial[bi]);
            let a = anchor(problem, bi);
            let t = (s.transform_point(a) - c.transform_point(a)) / scale;
            let w = log_rot(s.rotation * c.rotation.inverse());
            for (j, v) in [t.x, t.y, t.z, w.x, w.y, w.z].into_iter().enumerate() {
                back[(k * 6 + j, 0)] = v;
            }
        }
        let mut along = &basis * (basis.transpose() * &back);
        to_anchor(&mut along, -1.0);
        let along = DVector::from_column_slice(along.as_slice());
        let svd = jac.svd(true, true);
        let tol = 1e-10 * svd.singular_values.iter().fold(0.0f64, |a, v| a.max(*v));
        let Ok(fix) = svd.solve(&(-DVector::from_column_slice(&r)), tol) else { return };
        let delta = along + fix;
        for (k, &bi) in free.iter().enumerate() {
            let d: Vec<f64> = (0..6).map(|j| delta[k * 6 + j] * if j < 3 { scale } else { 1.0 }).collect();
            trial[bi] = apply_delta(&trial[bi], &d);
        }
        residuals(problem, &trial, &mut r);
        if delta.amax() < 1e-15 {
            break;
        }
    }
    if cost(&r) <= before {
        poses.copy_from_slice(&trial);
    }
}

/// Body-local point where [`least_change`] measures `body`'s move: the mean
/// origin of its connectors in the problem's mates (the part origin when it
/// has none).
fn anchor(p: &Problem, body: usize) -> DVec3 {
    let (mut sum, mut n) = (DVec3::ZERO, 0.0);
    let mut add = |m: &SolverMate| {
        for (b, f) in [(m.a, &m.a_local), (m.b, &m.b_local)].into_iter().chain(m.extra.iter().map(|(b, f)| (*b, f))) {
            if b == body {
                sum += f.origin;
                n += 1.0;
            }
        }
    };
    for &t in &p.terms {
        match t {
            Term::Mate(i) => add(&p.mates[i]),
            Term::Relation(i) => {
                add(&p.mates[p.relations[i].mate_a]);
                add(&p.mates[p.relations[i].mate_b]);
            }
            Term::Drag(_) | Term::Stay(_) => {}
        }
    }
    if n > 0.0 { sum / n } else { DVec3::ZERO }
}

fn fill_mate_status(p: &Problem, poses: &[Frame], result: &mut SolveResult) {
    let scale = characteristic_length(poses, p.mates).max(1e-3);
    for (i, m) in p.mates.iter().enumerate() {
        let mut r = Vec::new();
        mate_residuals(poses, m, &mut r);
        result.mate_ok[i] = r.iter().all(|v| v.abs() < 1e-5 * scale.max(1.0));
    }
}

/// [`characteristic_length`] of one group: the poses of the bodies its terms
/// read and the frames of its mates.
fn group_length(p: &Problem, poses: &[Frame]) -> f64 {
    let mut m: f64 = 0.0;
    let mut mate = |mt: &SolverMate| {
        for b in mate_bodies(mt) {
            m = m.max(poses[b].origin.length());
        }
        m = m.max(mt.a_local.origin.length()).max(mt.b_local.origin.length());
        for (_, f) in &mt.extra {
            m = m.max(f.origin.length());
        }
    };
    for &t in &p.terms {
        match t {
            Term::Mate(i) => mate(&p.mates[i]),
            Term::Relation(i) => {
                mate(&p.mates[p.relations[i].mate_a]);
                mate(&p.mates[p.relations[i].mate_b]);
            }
            Term::Drag(_) | Term::Stay(_) => {}
        }
    }
    m.max(0.01)
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

    /// Live (behaviours "qa4 round 5 s3 mate flip"): a Ø10×40 pin's top-face
    /// connector on a fixed plate's top face (z 10). Aligned Z axes (live's
    /// default) hang the pin through the plate, t (0,0,−30); Flip primary axis
    /// stands it upside down, t (0,0,50), Z (0,0,−1), for Revolute,
    /// Cylindrical and Planar alike.
    #[test]
    fn flip_primary_turns_axial_mates_over() {
        let mm = 1e-3;
        let pin_top = Frame::from_axes(DVec3::new(0.0, 0.0, 40.0 * mm), DVec3::Z, DVec3::X);
        let plate_top = Frame::from_axes(DVec3::new(0.0, 0.0, 10.0 * mm), DVec3::Z, DVec3::X);
        for kind in [MateType::Revolute, MateType::Cylindrical, MateType::Planar] {
            // Internal flip_primary true = Z axes aligned (live primaryAxisAlignment false).
            for (aligned, z, tz) in [(true, 1.0, -30.0), (false, -1.0, 50.0)] {
                let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.0, 0.0, -30.0 * mm), false)];
                let mut m = mate(kind, 1, pin_top, 0, plate_top);
                m.flip_primary = aligned;
                let r = solve(&mut bodies, &[m], &[], &[]);
                assert!(r.converged, "{kind:?} aligned={aligned}");
                let p = bodies[1].pose;
                assert!(((p.rotation * DVec3::Z).z - z).abs() < 1e-6, "{kind:?} aligned={aligned}: Z {:?}", p.rotation * DVec3::Z);
                assert!((p.origin.z - tz * mm).abs() < 1e-6, "{kind:?} aligned={aligned}: t {:?}", p.origin);
            }
        }
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

    /// qa4 DRAG-RUNAWAY (round 5 v3 session 10): 19 cubes (20 mm) stacked by
    /// Planar mates on a fixed plate; dragging the top one must leave the
    /// others where they are (least change: their in-plane freedom isn't
    /// the drag's business), not fling them away.
    #[test]
    fn planar_stack_drag_moves_only_the_dragged_cube() {
        let mm = 1e-3;
        let n = 19;
        let mut bodies = vec![body(DVec3::ZERO, true)];
        bodies.extend((0..n).map(|i| body(DVec3::new(0.0, 0.0, (10.0 + 20.0 * i as f64) * mm), false)));
        let top = |z: f64| Frame::from_axes(DVec3::new(10.0 * mm, 10.0 * mm, z * mm), DVec3::Z, DVec3::X);
        let bot = Frame::from_axes(DVec3::new(10.0 * mm, 10.0 * mm, 0.0), DVec3::NEG_Z, DVec3::X);
        let mates: Vec<SolverMate> = (1..=n).map(|i| mate(MateType::Planar, i, bot, i - 1, if i == 1 { top(10.0) } else { top(20.0) })).collect();
        assert!(solve(&mut bodies, &mates, &[], &[]).converged);
        let before: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        let lp = DVec3::new(10.0 * mm, 10.0 * mm, 20.0 * mm);
        for step in 1..=10 {
            let target = before[n].transform_point(lp) + DVec3::new(5.0, 3.0, 0.0) * mm * step as f64;
            let drags = vec![DragTarget { body: n, local_point: lp, target }];
            let r = solve_with(&mut bodies, &mates, &[], &drags, false);
            assert!(r.converged);
        }
        for i in 1..n {
            let d = bodies[i].pose.origin.distance(before[i].origin);
            let a = bodies[i].pose.rotation.angle_between(before[i].rotation);
            assert!(d < 1e-6 && a < 1e-6, "cube {i} moved {d} m, turned {a} rad");
        }
        let moved = bodies[n].pose.transform_point(lp).distance(before[n].transform_point(lp) + DVec3::new(50.0, 30.0, 0.0) * mm);
        assert!(moved < 1e-6, "dragged cube off its target by {moved} m");
        // A target off the cube's plane can't be reached by the cube alone, so
        // the whole stack is solved: the others still stay put.
        let start: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        for step in 1..=5 {
            let target = start[n].transform_point(lp) + DVec3::new(-4.0, 2.0, 3.0) * mm * step as f64;
            let drags = vec![DragTarget { body: n, local_point: lp, target }];
            assert!(solve_with(&mut bodies, &mates, &[], &drags, false).converged);
        }
        for i in 1..n {
            let d = bodies[i].pose.origin.distance(start[i].origin);
            assert!(d < 1e-6, "cube {i} moved {d} m on the out-of-plane drag");
        }
        let moved = bodies[n].pose.transform_point(lp) - start[n].transform_point(lp);
        assert!((moved - DVec3::new(-20.0, 10.0, 0.0) * mm).length() < 1e-6, "dragged cube moved {moved:?}");
    }

    /// The whole group must move (a 3-link revolute chain, its end dragged)
    /// while 10 cubes ride Planar-stacked on the last link: the cubes' free
    /// in-plane motion stays unused (least change), the chain follows.
    #[test]
    fn coupled_drag_leaves_free_bodies_put() {
        let mm = 1e-3;
        let (mut bodies, mut mates) = chain(4);
        let cube0 = bodies.len();
        let top = |z: f64| Frame::from_axes(DVec3::new(30.0 * mm, 6.0 * mm, z * mm), DVec3::Z, DVec3::X);
        let bot = Frame::from_axes(DVec3::new(10.0 * mm, 10.0 * mm, 0.0), DVec3::NEG_Z, DVec3::X);
        for i in 0..10 {
            let link_top = bodies[3].pose.origin.z / mm + 5.0;
            bodies.push(body(DVec3::new(bodies[3].pose.origin.x + 20.0 * mm, bodies[3].pose.origin.y - 4.0 * mm, (link_top + 20.0 * i as f64) * mm), false));
            let below = if i == 0 { 3 } else { cube0 + i - 1 };
            mates.push(mate(MateType::Planar, cube0 + i, bot, below, if i == 0 { top(5.0) } else { Frame::from_axes(DVec3::new(10.0 * mm, 10.0 * mm, 20.0 * mm), DVec3::Z, DVec3::X) }));
        }
        assert!(solve(&mut bodies, &mates, &[], &[]).converged);
        let before: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        let lp = DVec3::new(0.03, 0.006, 0.0);
        for step in 1..=5 {
            let target = before[3].transform_point(lp) + DVec3::new(-1.0, 2.0, 0.0) * mm * step as f64;
            let drags = vec![DragTarget { body: 3, local_point: lp, target }];
            assert!(solve_with(&mut bodies, &mates, &[], &drags, false).converged);
            assert!((bodies[3].pose.transform_point(lp) - target).length() < 1e-6, "step {step}: the chain end lags");
        }
        // The links turn about Z, so the plane the cubes sit on keeps its
        // height: nothing asks them to move at all.
        for i in cube0..bodies.len() {
            let d = bodies[i].pose.origin.distance(before[i].origin);
            assert!(d < 1e-6, "cube {} moved {d} m", i - cube0);
        }
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

    /// Moving a Planar-mated part to a pose (Transform, agent
    /// `assembly_transform`) projects the pose onto the mate's free
    /// directions exactly: live keeps the requested X and Y (and the turn
    /// about the normal) and fixes only Z (api benchmark F9: [30, 5, 40] mm
    /// ended at [30.00172, 5.000287, 12]).
    #[test]
    fn planar_transform_keeps_the_free_coordinates() {
        let mm = 1e-3;
        // A 100×60×12 plate (fixed) and a 50×50×30 cup on it, the cup's
        // bottom face picked first.
        let top = Frame::from_axes(DVec3::new(50.0, 30.0, 12.0) * mm, DVec3::Z, DVec3::X);
        let bottom = Frame::from_axes(DVec3::new(25.0, 25.0, 0.0) * mm, DVec3::NEG_Z, DVec3::X);
        let mates = vec![mate(MateType::Planar, 1, bottom, 0, top)];
        for turn in [0.0, 0.5, -2.0] {
            let mut bodies = vec![body(DVec3::ZERO, true), body(DVec3::new(0.0, 0.0, 30.0) * mm, false)];
            assert!(solve(&mut bodies, &mates, &[], &[]).converged);
            let rotation = DQuat::from_rotation_z(turn);
            bodies[1].pose = Frame::new(DVec3::new(30.0, 5.0, 40.0) * mm, rotation);
            let r = solve(&mut bodies, &mates, &[], &[]);
            assert!(r.converged, "{r:?}");
            let p = bodies[1].pose;
            let o = p.origin / mm;
            assert!((o - DVec3::new(30.0, 5.0, 12.0)).abs().max_element() < 1e-9, "turn {turn}: at {o:?} mm");
            // The turn from the vector part (angle_between's acos reads one ulp as 3e-8).
            let turned = 2.0 * (p.rotation * rotation.inverse()).xyz().length();
            assert!(turned < 1e-12, "turn {turn}: turned {turned}");
        }
    }

    /// oracle_asm M2 (live): B fastened on A from a turned start, C on a
    /// Revolute on B's top: C keeps its start turn exactly (unturned).
    #[test]
    fn revolute_on_a_fastened_part_keeps_its_turn() {
        let mm = 1e-3;
        let top = Frame::from_axes(DVec3::new(20.0, 10.0, 10.0) * mm, DVec3::Z, DVec3::X);
        let bottom = Frame::from_axes(DVec3::new(20.0, 10.0, 0.0) * mm, DVec3::NEG_Z, DVec3::X);
        let mut bodies = vec![
            body(DVec3::ZERO, true),
            SolverBody { pose: Frame::new(DVec3::new(100.0, 50.0, 30.0) * mm, DQuat::from_rotation_z(30f64.to_radians())), fixed: false },
            body(DVec3::new(0.0, 0.0, 40.0) * mm, false),
        ];
        let mates = vec![mate(MateType::Fastened, 1, bottom, 0, top), mate(MateType::Revolute, 2, bottom, 1, top)];
        let r = solve(&mut bodies, &mates, &[], &[]);
        assert!(r.converged, "{r:?}");
        let c = bodies[2].pose;
        assert!(c.rotation.angle_between(DQuat::IDENTITY) < 1e-12, "C turned {}", c.rotation.angle_between(DQuat::IDENTITY));
        assert!((c.origin / mm - DVec3::new(0.0, 0.0, 20.0)).length() < 1e-9, "{:?}", c.origin / mm);
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
