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

fn mate_residuals(bodies: &[Frame], m: &SolverMate, out: &mut Vec<f64>) {
    let r = relative(bodies, m);
    let z = r.rotation * DVec3::Z;
    let t = r.origin - m.offset;
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
        // Tangent/Width are reduced to point-on-plane style constraints by the
        // caller (connector frames placed at contact); treat as planar contact.
        MateType::Tangent => out.push(t.z),
        MateType::Width => out.push(t.z),
    }
    // Limits: penalize violation of free values.
    let vals = mate_values(bodies, m);
    for &(i, lo, hi) in &m.limits {
        let v = vals[i.min(3)];
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
    match r.kind {
        RelationKind::Gear => wrap_angle((vb[0] - r.base_b) + sign * r.ratio * wrap_angle(va[0] - r.base_a)),
        RelationKind::Linear => (vb[3] - r.base_b) - sign * r.ratio * (va[3] - r.base_a),
        RelationKind::RackAndPinion => (vb[3] - r.base_b) - sign * r.ratio * wrap_angle(va[0] - r.base_a),
        RelationKind::Screw => (vb[3] - r.base_b) - sign * r.ratio * wrap_angle(vb[0] - r.base_a),
    }
}

pub struct Problem<'a> {
    pub bodies: &'a [SolverBody],
    pub mates: &'a [SolverMate],
    pub relations: &'a [SolverRelation],
    pub drags: &'a [DragTarget],
}

const DRAG_WEIGHT: f64 = 1e-2;

fn residuals(p: &Problem, poses: &[Frame], out: &mut Vec<f64>) {
    out.clear();
    for m in p.mates {
        mate_residuals(poses, m, out);
    }
    for r in p.relations {
        out.push(relation_residual(poses, p.mates, r));
    }
    for d in p.drags {
        let w = poses[d.body].transform_point(d.local_point) - d.target;
        out.extend_from_slice(&[w.x * DRAG_WEIGHT, w.y * DRAG_WEIGHT, w.z * DRAG_WEIGHT]);
    }
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
    if !drags.is_empty() {
        // Phase 1: follow the drag (soft); phase 2: project back onto the
        // constraint manifold exactly, starting from the dragged pose.
        solve_inner(bodies, mates, relations, drags);
        return solve_inner(bodies, mates, relations, &[]);
    }
    solve_inner(bodies, mates, relations, drags)
}

fn solve_inner(bodies: &mut [SolverBody], mates: &[SolverMate], relations: &[SolverRelation], drags: &[DragTarget]) -> SolveResult {
    let free: Vec<usize> = (0..bodies.len()).filter(|&i| !bodies[i].fixed).collect();
    let n = free.len() * 6;
    let mut poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
    let problem = Problem { bodies, mates, relations, drags };
    let mut r = Vec::new();
    residuals(&problem, &poses, &mut r);
    let mut result = SolveResult { mate_ok: vec![false; mates.len()], body_dof: vec![0; bodies.len()], ..Default::default() };
    if n == 0 || r.is_empty() {
        result.converged = r.iter().all(|v| v.abs() < 1e-7);
        result.max_residual = r.iter().fold(0.0f64, |a, v| a.max(v.abs()));
        fill_mate_status(&problem, &poses, &mut result);
        return result;
    }

    // Scale translations so rotations and lengths are comparable.
    let scale = characteristic_length(&poses, mates).max(1e-3);
    let mut lambda = 1e-3;
    let mut cost: f64 = r.iter().map(|v| v * v).sum();
    let eps = 1e-7;
    let mut iterations = 0;
    let mut r2 = Vec::new();
    let mut jac = DMatrix::<f64>::zeros(r.len(), n);
    for it in 0..100 {
        iterations = it + 1;
        if cost.sqrt() < 1e-10 {
            break;
        }
        // Finite-difference Jacobian.
        jac = DMatrix::<f64>::zeros(r.len(), n);
        for (k, &bi) in free.iter().enumerate() {
            for j in 0..6 {
                let mut d = [0.0; 6];
                d[j] = if j < 3 { eps * scale } else { eps };
                let saved = poses[bi];
                poses[bi] = apply_delta(&saved, &d);
                residuals(&problem, &poses, &mut r2);
                poses[bi] = saved;
                let col = k * 6 + j;
                for row in 0..r.len() {
                    jac[(row, col)] = (r2[row] - r[row]) / eps;
                }
            }
        }
        let rv = DVector::from_vec(r.clone());
        let jt = jac.transpose();
        let jtj = &jt * &jac;
        let g = &jt * &rv;
        let mut improved = false;
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
            residuals(&problem, &trial, &mut r2);
            let c2: f64 = r2.iter().map(|v| v * v).sum();
            if c2 < cost {
                poses = trial;
                std::mem::swap(&mut r, &mut r2);
                cost = c2;
                lambda = (lambda * 0.3).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
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
    result.iterations = iterations;
    fill_mate_status(&problem, &poses, &mut result);

    // DOF analysis on the constraint Jacobian (without drag rows).
    let rows = mate_r.len();
    if rows > 0 {
        let sub = jac.rows(0, rows.min(jac.nrows())).into_owned();
        let svd = sub.svd(false, true);
        let tol = 1e-6 * svd.singular_values.max().max(1.0);
        let rank = svd.singular_values.iter().filter(|s| **s > tol).count();
        result.dof = n - rank;
        if let Some(vt) = svd.v_t {
            // Null-space rows: right singular vectors with ~0 singular value.
            let sv: Vec<f64> = svd.singular_values.iter().copied().collect();
            for (k, &bi) in free.iter().enumerate() {
                let mut dof_count = 0;
                for row in 0..vt.nrows() {
                    let s = sv.get(row).copied().unwrap_or(0.0);
                    if s > tol {
                        continue;
                    }
                    let mag: f64 = (0..6).map(|j| vt[(row, k * 6 + j)].powi(2)).sum::<f64>().sqrt();
                    if mag > 1e-6 {
                        dof_count += 1;
                    }
                }
                // Null vectors beyond the row count are also null.
                let extra = n.saturating_sub(vt.nrows());
                result.body_dof[bi] = (dof_count + extra).min(6);
            }
        }
    } else {
        result.dof = n;
        for &bi in &free {
            result.body_dof[bi] = 6;
        }
    }

    for (b, p) in bodies.iter_mut().zip(poses) {
        if !b.fixed {
            b.pose = p;
        }
    }
    result
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
        SolverMate { kind, a, a_local: la, b, b_local: lb, flip_primary: false, reorient: 0, offset: DVec3::ZERO, rotation: 0.0, limits: vec![] }
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
        let rel = vec![SolverRelation { kind: RelationKind::Gear, mate_a: 0, mate_b: 1, ratio: 2.0, reverse: false, base_a, base_b }];
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
