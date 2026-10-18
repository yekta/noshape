//! Assembly evaluation: resolves instances (parts, rigid Part Studios and
//! sub-assemblies, recursively), resolves mate connectors (explicit Part
//! Studio connectors and implicit inferred ones) in instance coordinates,
//! runs the mate solver and produces a kernel-free display model.
//!
//! Evaluation is split in two stages so interactive operations stay cheap:
//! * [`AssemblyEvaluator::prepare`] regenerates referenced Part Studios
//!   (cached per (tab, configuration) and studio content hash), flattens the
//!   instance tree and builds the solver problem ([`Prepared`]);
//! * [`Prepared::solve`] runs the solver from given placements with optional
//!   drag targets or a driven mate value (dragging, animation) and is a pure
//!   function of its inputs, so it can run on any thread.

use super::solver::{self, DragTarget, Follow, FollowXform, Frame, RelationKind, SolverBody, SolverMate, SolverRelation, TangentFace, TangentGeom, TangentPair};
use super::{AsmMateConnector, Assembly, ExplodedView, InstanceId, InstanceSource, MateConnectorRef, MateId, MateType, NamedPosition, PatternKind, Placement, Relation, RelationType};
use crate::configuration::ConfigValues;
use crate::display::{BodyKind, MeshBuffers};
use crate::document::{Document, TabContent, TabId};
use crate::params::ParamValues;
use crate::reference::{BodyId, FeatureId, Inference, Reference};
use crate::regen::context::Context;
use crate::regen::eval::EvalCtx;
use crate::regen::{RegenOptions, RegenResult, Regenerator};
use glam::{DMat4, DQuat, DVec3};
use noshape_units::{Dim, EmptyScope, ExprValue, UnitPrefs};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

/// Path of instance ids from the top-level assembly down to an instance.
pub type InstancePath = Vec<InstanceId>;

/// A geometry reference on a specific occurrence (instance path + studio reference).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OccurrenceRef {
    pub path: InstancePath,
    pub reference: Reference,
}

pub fn frame_to_placement(f: &Frame) -> Placement {
    Placement { translation: f.origin.into(), rotation: [f.rotation.x, f.rotation.y, f.rotation.z, f.rotation.w] }
}

pub fn placement_to_frame(p: &Placement) -> Frame {
    let [x, y, z, w] = p.rotation;
    let q = DQuat::from_xyzw(x, y, z, w);
    let q = if q.length_squared() < 1e-24 { DQuat::IDENTITY } else { q.normalize() };
    Frame::new(p.translation.into(), q)
}

pub fn frame_matrix(f: &Frame) -> DMat4 {
    DMat4::from_rotation_translation(f.rotation, f.origin)
}

pub fn matrix_frame(m: &DMat4) -> Frame {
    let (_, r, t) = m.to_scale_rotation_translation();
    Frame::new(t, r.normalize())
}

/// One displayed body of one occurrence.
#[derive(Clone)]
pub struct PartOccurrence {
    /// Full instance path (top-level first) of the leaf instance owning the body.
    pub path: InstancePath,
    pub body: BodyId,
    pub studio: TabId,
    pub config: ConfigValues,
    /// Instance name (leaf) and body name.
    pub instance_name: String,
    pub body_name: String,
    pub kind: BodyKind,
    /// Linear RGBA.
    pub color: [f32; 4],
    pub mesh: Arc<MeshBuffers>,
    /// Body coordinates (Part Studio) -> leaf's top-level instance coordinates.
    pub inner: DMat4,
    /// Body coordinates -> assembly world (after solving / explode).
    pub transform: DMat4,
    pub face_refs: Vec<Reference>,
    pub edge_refs: Vec<Reference>,
    pub vertex_refs: Vec<Reference>,
    pub hidden: bool,
    /// Bounds in body coordinates.
    pub local_bounds: Option<(DVec3, DVec3)>,
    /// Regenerated studio (for inference and interference).
    pub context: Arc<Context>,
    /// Mirrored (opposite-handed) version of the part.
    pub mirrored: bool,
    /// External reference key for parts inserted from another document.
    pub external: Option<String>,
    /// Part of a flexible subassembly: (child instance of the subassembly,
    /// body coordinates -> child coordinates). Its transform follows the
    /// child's solved placement inside the subassembly.
    pub flex: Option<(InstanceId, DMat4)>,
}

impl PartOccurrence {
    pub fn top(&self) -> InstanceId {
        self.path[0]
    }
    /// World-space axis-aligned bounds.
    pub fn world_bounds(&self) -> Option<(DVec3, DVec3)> {
        let (lo, hi) = self.local_bounds?;
        let mut acc: Option<(DVec3, DVec3)> = None;
        for i in 0..8 {
            let c = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
            let w = self.transform.transform_point3(c);
            acc = Some(match acc {
                None => (w, w),
                Some((a, b)) => (a.min(w), b.max(w)),
            });
        }
        acc
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Part,
    PartStudio,
    Assembly,
}

/// Instance tree node for the instances list.
#[derive(Clone, Debug)]
pub struct InstanceNode {
    pub path: InstancePath,
    pub name: String,
    pub kind: NodeKind,
    pub fixed: bool,
    pub hidden: bool,
    pub suppressed: bool,
    /// Remaining DOF for top-level instances (None for nested ones).
    pub dof: Option<usize>,
    pub error: Option<String>,
    pub children: Vec<InstanceNode>,
}

#[derive(Clone, Debug)]
pub struct MateStatus {
    pub id: MateId,
    pub ok: bool,
    /// Inactive: suppressed mate or suppressed instance.
    pub inactive: bool,
    pub message: Option<String>,
    /// World frames of the two connectors after solving.
    pub frame_a: Option<Frame>,
    pub frame_b: Option<Frame>,
    /// Free values (rotation about Z, translations X/Y/Z) after solving.
    pub values: [f64; 4],
}

/// Assembly mate connector after solving.
#[derive(Clone, Debug)]
pub struct ConnectorStatus {
    pub id: MateId,
    pub owner: InstanceId,
    /// Owner child inside a flexible subassembly (`local` is then relative to it).
    pub child: Option<InstanceId>,
    /// Frame in the owner's instance coordinates.
    pub local: Option<Frame>,
    /// World frame after solving.
    pub frame: Option<Frame>,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RelationStatus {
    pub id: MateId,
    pub ok: bool,
    pub message: Option<String>,
}

/// Interactive solve inputs.
#[derive(Clone, Debug, Default)]
pub struct SolveOptions {
    /// Drag targets: (top-level instance, point in instance coordinates, world target).
    pub drags: Vec<(InstanceId, DVec3, DVec3)>,
    /// Drive a mate's first free value (rotation for revolute/cylindrical,
    /// translation for slider) to a value (animation).
    pub drive: Option<(MateId, f64)>,
    /// Starting placements overriding the assembly's stored ones.
    pub start: Option<HashMap<InstanceId, Placement>>,
    /// Instances held in place for this solve (e.g. the stationary side of a
    /// mate being created).
    pub hold: Vec<InstanceId>,
    /// Requested placements (triad move / rotate). Unconstrained instances
    /// take them exactly; constrained ones move toward them within their DOF.
    pub targets: Vec<(InstanceId, Placement)>,
    /// Mates held at given free values (rotation about Z, translations
    /// X/Y/Z), e.g. when restoring a named position.
    pub locks: Vec<(MateId, [f64; 4])>,
    /// One free value of a mate held at a target (mate, value index 0 = Rz,
    /// 1..3 = Tx/Ty/Tz, target); the mate's other degrees of freedom stay
    /// free (Animate, Apply limit position). The mate's limits are ignored.
    pub hold_values: Vec<(MateId, usize, f64)>,
    /// Starting placements of flexible subassembly children (relative to
    /// their subassembly instance), overriding the stored ones.
    pub nested_start: Option<HashMap<(InstanceId, InstanceId), Placement>>,
    /// Drags of flexible subassembly children: ((subassembly, child), point in
    /// child coordinates, world target).
    pub nested_drags: Vec<((InstanceId, InstanceId), DVec3, DVec3)>,
}

#[derive(Clone, Debug, Default)]
pub struct Solved {
    pub placements: Vec<(InstanceId, Placement)>,
    pub converged: bool,
    pub dof: usize,
    pub instance_dof: HashMap<InstanceId, usize>,
    pub mate_ok: HashMap<MateId, bool>,
    pub mate_frames: HashMap<MateId, (Frame, Frame)>,
    pub mate_values: HashMap<MateId, [f64; 4]>,
    pub relation_ok: HashMap<MateId, bool>,
    /// Flexible subassembly children: (subassembly, child) -> placement in the subassembly.
    pub nested: HashMap<(InstanceId, InstanceId), Placement>,
}

impl Solved {
    pub fn placement(&self, id: InstanceId) -> Option<Placement> {
        self.placements.iter().find(|(i, _)| *i == id).map(|(_, p)| *p)
    }
    /// World frame of a flexible subassembly child (None for other instances).
    pub fn nested_world(&self, sub: InstanceId, child: InstanceId) -> Option<Frame> {
        let q = self.nested.get(&(sub, child))?;
        Some(placement_to_frame(&self.placement(sub)?).mul(&placement_to_frame(q)))
    }
}

struct LeafInstance {
    path: InstancePath,
    inner: DMat4,
    studio: TabId,
    body: Option<BodyId>,
    config: ConfigValues,
    regen: Arc<RegenResult>,
    name: String,
    hidden: bool,
    mirrored: bool,
    external: Option<String>,
    /// Flexible subassembly part: (child of the subassembly, leaf -> child coordinates).
    flex: Option<(InstanceId, DMat4)>,
}

/// Where a connector lives: a top-level instance or a child of a flexible
/// subassembly (top-level subassembly instance, child).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Owner {
    Top(InstanceId),
    Nested(InstanceId, InstanceId),
}

/// Prepared solver problem (instance graph + resolved connector frames).
#[derive(Clone, Default)]
pub struct Prepared {
    /// Top-level instance order (non-suppressed), with stored placement.
    pub instances: Vec<(InstanceId, Placement)>,
    /// Instance -> (solver body, offset from the body's representative frame).
    body_of: HashMap<InstanceId, (usize, Frame)>,
    /// Representative instance per solver body.
    reps: Vec<InstanceId>,
    fixed: Vec<bool>,
    mates: Vec<(MateId, MateType, SolverMate)>,
    relations: Vec<PreparedRelation>,
    /// Mates that could not be resolved, with messages.
    pub failed_mates: Vec<(MateId, String)>,
    pub inactive_mates: HashSet<MateId>,
    pub failed_relations: Vec<(MateId, String)>,
    /// Assembly mate connectors: (id, owner top-level instance or ORIGIN,
    /// frame in the owner's instance coordinates).
    pub connectors: Vec<(MateId, InstanceId, Frame)>,
    pub failed_connectors: Vec<(MateId, String)>,
    /// Pattern followers (pattern id, failure message).
    pub failed_patterns: Vec<(MateId, String)>,
    /// Flexible subassemblies: (subassembly instance, child) -> (solver body,
    /// child frame in body coordinates). Children fixed in the subassembly
    /// ride on the subassembly instance's body.
    nested_body_of: HashMap<(InstanceId, InstanceId), (usize, Frame)>,
    /// Children with their own solver body: (body, subassembly instance,
    /// child, stored placement of the child in the subassembly).
    nested_bodies: Vec<(usize, InstanceId, InstanceId, Frame)>,
    /// Assembly mate connectors owned by a flexible subassembly child.
    connector_child: HashMap<MateId, InstanceId>,
    /// Top-level instances that are flexible subassemblies.
    pub flexible: HashSet<InstanceId>,
}

#[derive(Clone, Debug)]
struct PreparedRelation {
    id: MateId,
    kind: RelationKind,
    a: MateId,
    b: MateId,
    ratio: f64,
    reverse: bool,
    values: (usize, usize),
    base: Option<[f64; 2]>,
}

/// Which free value of a mate a relation couples: (rotational, translational).
fn rotational_value(t: MateType) -> Option<usize> {
    match t {
        MateType::Revolute | MateType::Cylindrical | MateType::PinSlot | MateType::Planar | MateType::Parallel => Some(0),
        _ => None,
    }
}

fn translational_value(t: MateType) -> Option<usize> {
    match t {
        MateType::Slider | MateType::Cylindrical => Some(3),
        MateType::PinSlot | MateType::Planar => Some(1),
        MateType::Parallel => Some(3),
        _ => None,
    }
}

/// Free value indices a relation couples on its two mates, or why the
/// mates do not fit the relation type.
pub fn relation_values(kind: RelationType, a: MateType, b: MateType) -> Result<(usize, usize), String> {
    if matches!(a, MateType::Tangent | MateType::Width) || matches!(b, MateType::Tangent | MateType::Width) {
        return Err("Tangent and width mates cannot be used in relations".into());
    }
    match kind {
        RelationType::Gear => match (rotational_value(a), rotational_value(b)) {
            (Some(x), Some(y)) => Ok((x, y)),
            _ => Err("Gear relations require two rotational degrees of freedom".into()),
        },
        RelationType::RackAndPinion => match (rotational_value(a), translational_value(b)) {
            (Some(x), Some(y)) => Ok((x, y)),
            _ => Err("Rack and pinion relations require one rotational and one translational degree of freedom".into()),
        },
        RelationType::Screw => match a {
            MateType::Cylindrical => Ok((0, 3)),
            _ => Err("Screw relations require one cylindrical mate".into()),
        },
        RelationType::Linear => match (translational_value(a), translational_value(b)) {
            (Some(x), Some(y)) => Ok((x, y)),
            _ => Err("Linear relations require two translational degrees of freedom".into()),
        },
    }
}

fn relation_kind(t: RelationType) -> RelationKind {
    match t {
        RelationType::Gear => RelationKind::Gear,
        RelationType::RackAndPinion => RelationKind::RackAndPinion,
        RelationType::Screw => RelationKind::Screw,
        RelationType::Linear => RelationKind::Linear,
    }
}

/// New placement of an instance so that its connector (`source`, world)
/// lands face-to-face on `target` (world), as a fastened mate would place it.
pub fn snap_placement(pose: &Placement, source: &Frame, target: &Frame) -> Placement {
    let r0 = Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI));
    let p = placement_to_frame(pose);
    let np = target.mul(&r0.inverse()).mul(&source.inverse()).mul(&p);
    frame_to_placement(&np)
}

/// Assembly mate connector frame relative to its origin entity's frame:
/// flip, reorient, then move (translation in the connector frame, rotation about a local axis).
pub fn asm_connector_offset(c: &AsmMateConnector, units: &UnitPrefs) -> Frame {
    let mut f = Frame::default();
    if c.flip_primary {
        f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
    }
    if !c.reorient.is_multiple_of(4) {
        f = f.mul(&Frame::new(DVec3::ZERO, quarter_rot(c.reorient)));
    }
    let t = DVec3::new(eval_len(&c.translation[0], units), eval_len(&c.translation[1], units), eval_len(&c.translation[2], units));
    let ang = eval_angle(&c.rotation, units);
    let axis = match c.rotation_axis {
        0 => DVec3::X,
        1 => DVec3::Y,
        _ => DVec3::Z,
    };
    f.mul(&Frame::new(t, DQuat::from_axis_angle(axis, ang)))
}

fn quarter_rot(k: u8) -> DQuat {
    DQuat::from_rotation_z((k % 4) as f64 * std::f64::consts::FRAC_PI_2)
}

impl Prepared {
    pub fn has_instance(&self, id: InstanceId) -> bool {
        self.body_of.contains_key(&id)
    }

    /// Does this child of a flexible subassembly move on its own (not fixed
    /// in the subassembly)?
    pub fn is_flexible_child(&self, sub: InstanceId, child: InstanceId) -> bool {
        self.nested_bodies.iter().any(|(_, s, c, _)| *s == sub && *c == child)
            || self.nested_body_of.get(&(sub, child)).is_some_and(|(b, _)| self.body_of.get(&sub).is_none_or(|(sb, _)| sb != b))
    }

    /// Solve for a named position: its placements as the start, its stored
    /// driving mate values imposed.
    pub fn solve_named_position(&self, np: &NamedPosition) -> Solved {
        let mut start: HashMap<InstanceId, Placement> = self.instances.iter().cloned().collect();
        for (id, p) in &np.placements {
            if start.contains_key(id) {
                start.insert(*id, *p);
            }
        }
        self.solve(&SolveOptions { start: Some(start), locks: np.mate_values.clone(), ..Default::default() })
    }

    /// Solve from the stored (or `opts.start`) placements.
    pub fn solve(&self, opts: &SolveOptions) -> Solved {
        let pose_of = |id: InstanceId, stored: &Placement| -> Frame {
            let p = opts.start.as_ref().and_then(|s| s.get(&id)).unwrap_or(stored);
            placement_to_frame(p)
        };
        let mut stored: HashMap<InstanceId, Placement> = self.instances.iter().cloned().collect();
        stored.insert(InstanceId::ORIGIN, Placement::default());
        let mut bodies: Vec<SolverBody> = self
            .reps
            .iter()
            .enumerate()
            .map(|(i, id)| SolverBody { pose: if *id == InstanceId::ORIGIN { Frame::default() } else { pose_of(*id, &stored[id]) }, fixed: self.fixed[i] || opts.hold.iter().any(|h| self.body_of.get(h).is_some_and(|(b, _)| *b == i)) })
            .collect();
        // Flexible subassembly children: world pose = subassembly pose * child placement.
        for (b, sid, child, q0) in &self.nested_bodies {
            let ps = stored.get(sid).map(|p| pose_of(*sid, p)).unwrap_or_default();
            let q = opts.nested_start.as_ref().and_then(|n| n.get(&(*sid, *child))).map(placement_to_frame).unwrap_or(*q0);
            bodies[*b].pose = ps.mul(&q);
            // Holding the subassembly holds its parts.
            if opts.hold.contains(sid) {
                bodies[*b].fixed = true;
            }
        }
        // A target placement of a flexible subassembly carries its parts along.
        for (id, p) in &opts.targets {
            if !self.flexible.contains(id) {
                continue;
            }
            let cur = stored.get(id).map(|s| pose_of(*id, s)).unwrap_or_default();
            let delta = placement_to_frame(p).mul(&cur.inverse());
            for (b, sid, _, _) in &self.nested_bodies {
                if sid == id && !bodies[*b].fixed {
                    bodies[*b].pose = delta.mul(&bodies[*b].pose);
                }
            }
        }
        let mut mates: Vec<SolverMate> = self.mates.iter().map(|(_, _, m)| m.clone()).collect();
        if let Some((mid, value)) = opts.drive
            && let Some(i) = self.mates.iter().position(|(id, _, _)| *id == mid)
        {
            let (_, kind, _) = self.mates[i];
            let m = &mut mates[i];
            match kind {
                MateType::Revolute => {
                    m.kind = MateType::Fastened;
                    m.rotation = value;
                }
                MateType::Cylindrical => {
                    m.kind = MateType::Slider;
                    m.rotation = value;
                }
                MateType::Slider => {
                    m.kind = MateType::Fastened;
                    m.offset.z = value;
                }
                _ => {}
            }
            m.limits.clear();
        }
        for (mid, v) in &opts.locks {
            let Some(i) = self.mates.iter().position(|(id, _, _)| id == mid) else { continue };
            if matches!(self.mates[i].1, MateType::Fastened | MateType::Ball | MateType::Tangent | MateType::Width) {
                continue;
            }
            // Every free value fixed: the mate becomes a fastened one at those values.
            let m = &mut mates[i];
            m.kind = MateType::Fastened;
            m.rotation = v[0];
            m.offset = DVec3::new(v[1], v[2], v[3]);
            m.limits.clear();
        }
        for (mid, ix, v) in &opts.hold_values {
            let Some(i) = self.mates.iter().position(|(id, _, _)| id == mid) else { continue };
            mates[i].hold.push((*ix, *v));
            mates[i].limits.clear();
        }
        let mate_index: HashMap<MateId, usize> = self.mates.iter().enumerate().map(|(i, (id, _, _))| (*id, i)).collect();
        let poses0: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        let mut rels = Vec::new();
        let mut rel_ids = Vec::new();
        for r in &self.relations {
            let (Some(&ia), Some(&ib)) = (mate_index.get(&r.a), mate_index.get(&r.b)) else { continue };
            let (base_a, base_b) = match r.base {
                Some([x, y]) => (x, y),
                None => (solver::mate_values(&poses0, &mates[ia])[r.values.0], solver::mate_values(&poses0, &mates[ib])[r.values.1]),
            };
            rels.push(SolverRelation { kind: r.kind, mate_a: ia, mate_b: ib, ratio: r.ratio, reverse: r.reverse, base_a, base_b, value_a: r.values.0, value_b: r.values.1 });
            rel_ids.push(r.id);
        }
        // Unconstrained bodies follow a drag by pure translation (no solve).
        let constrained = |b: usize| mates.iter().any(|m| m.a == b || m.b == b || m.extra.iter().any(|(e, _)| *e == b) || m.follow.is_some_and(|f| f.axis_body == b && m.b != b));
        let mut drags: Vec<DragTarget> = Vec::new();
        for (id, local, target) in &opts.drags {
            let Some((b, off)) = self.body_of.get(id) else { continue };
            let lp = off.transform_point(*local);
            if !constrained(*b) {
                if !bodies[*b].fixed {
                    let cur = bodies[*b].pose.transform_point(lp);
                    bodies[*b].pose.origin += *target - cur;
                }
                continue;
            }
            drags.push(DragTarget { body: *b, local_point: lp, target: *target });
        }
        for (key, local, target) in &opts.nested_drags {
            let Some((b, off)) = self.nested_body_of.get(key) else { continue };
            let lp = off.transform_point(*local);
            if !constrained(*b) {
                if !bodies[*b].fixed {
                    let cur = bodies[*b].pose.transform_point(lp);
                    bodies[*b].pose.origin += *target - cur;
                }
                continue;
            }
            drags.push(DragTarget { body: *b, local_point: lp, target: *target });
        }
        // Placement targets (triad rotate / typed values).
        let scale = self.instances.iter().map(|(_, p)| DVec3::from(p.translation).length()).fold(0.01f64, f64::max) * 0.5;
        for (id, p) in &opts.targets {
            let Some((b, off)) = self.body_of.get(id) else { continue };
            if bodies[*b].fixed {
                continue;
            }
            let want = placement_to_frame(p).mul(&off.inverse());
            if !constrained(*b) {
                bodies[*b].pose = want;
                continue;
            }
            for lp in [DVec3::ZERO, DVec3::X * scale, DVec3::Y * scale, DVec3::Z * scale] {
                drags.push(DragTarget { body: *b, local_point: lp, target: want.transform_point(lp) });
            }
        }
        let r = solver::solve(&mut bodies, &mates, &rels, &drags);
        let poses: Vec<Frame> = bodies.iter().map(|b| b.pose).collect();
        let mut out = Solved { converged: r.converged, dof: r.dof, ..Default::default() };
        for (id, _) in &self.instances {
            let (b, off) = self.body_of[id];
            let f = poses[b].mul(&off);
            out.placements.push((*id, frame_to_placement(&f)));
            out.instance_dof.insert(*id, if self.fixed[b] { 0 } else { r.body_dof.get(b).copied().unwrap_or(6) });
        }
        for ((sid, child), (b, off)) in &self.nested_body_of {
            let Some(&(sb, soff)) = self.body_of.get(sid) else { continue };
            let ps = poses[sb].mul(&soff);
            out.nested.insert((*sid, *child), frame_to_placement(&ps.inverse().mul(&poses[*b].mul(off))));
        }
        for (b, sid, _, _) in &self.nested_bodies {
            if let Some(d) = out.instance_dof.get_mut(sid) {
                *d += r.body_dof.get(*b).copied().unwrap_or(0);
            }
        }
        for (i, (id, _, m)) in self.mates.iter().enumerate() {
            out.mate_ok.insert(*id, r.mate_ok.get(i).copied().unwrap_or(false));
            let fa = poses[m.a].mul(&m.a_local);
            let fb = poses[m.b].mul(&m.b_local);
            out.mate_frames.insert(*id, (fa, fb));
            out.mate_values.insert(*id, solver::mate_values(&poses, m));
        }
        for (k, rel) in rels.iter().enumerate() {
            let va = solver::mate_values(&poses, &mates[rel.mate_a]);
            let vb = solver::mate_values(&poses, &mates[rel.mate_b]);
            let _ = (va, vb);
            out.relation_ok.insert(rel_ids[k], r.converged || r.max_residual < 1e-4);
        }
        out
    }
}

/// Result of evaluating an assembly.
#[derive(Clone, Default)]
pub struct AssemblyResult {
    pub occurrences: Vec<PartOccurrence>,
    pub nodes: Vec<InstanceNode>,
    pub solved: Solved,
    pub mates: Vec<MateStatus>,
    pub relations: Vec<RelationStatus>,
    pub connectors: Vec<ConnectorStatus>,
    /// Pattern status messages (failed patterns).
    pub pattern_errors: Vec<(MateId, String)>,
    pub prepared: Arc<Prepared>,
    pub eval_ms: f32,
    /// Explode lines of the displayed exploded view: (start, end) of each
    /// step's instance centers.
    pub explode_trails: Vec<(DVec3, DVec3)>,
}

impl AssemblyResult {
    /// Recompute occurrence world transforms from solved placements and an
    /// optional exploded view (display only).
    pub fn apply_placements(&mut self, solved: Solved, explode: Option<&ExplodedView>) {
        self.apply_placements_progress(solved, explode.map(|e| (e, f64::INFINITY)));
    }

    /// Like [`apply_placements`](Self::apply_placements) with an exploded view
    /// played up to `progress` steps (fractional: the last step partially).
    pub fn apply_placements_progress(&mut self, solved: Solved, explode: Option<(&ExplodedView, f64)>) {
        self.solved = solved;
        let place: HashMap<InstanceId, DMat4> = self.solved.placements.iter().map(|(id, p)| (*id, frame_matrix(&placement_to_frame(p)))).collect();
        for o in &mut self.occurrences {
            let top = place.get(&o.top()).copied().unwrap_or(DMat4::IDENTITY);
            o.transform = match o.flex.and_then(|(c, rest)| self.solved.nested.get(&(o.path[0], c)).map(|q| (q.to_dmat4(), rest))) {
                Some((q, rest)) => top * q * rest,
                None => top * o.inner,
            };
        }
        self.explode_trails.clear();
        if let Some((ev, progress)) = explode {
            let (steps, trails) = explode_matrices(self, ev, progress);
            for o in &mut self.occurrences {
                if let Some(m) = steps.get(&o.top()) {
                    o.transform = *m * o.transform;
                }
            }
            self.explode_trails = trails;
        }
        let inst: HashMap<InstanceId, Frame> = self.solved.placements.iter().map(|(id, p)| (*id, placement_to_frame(p))).collect();
        for c in &mut self.connectors {
            let owner = match c.child.and_then(|ch| self.solved.nested_world(c.owner, ch)) {
                Some(f) => f,
                None if c.owner == InstanceId::ORIGIN => Frame::default(),
                None => inst.get(&c.owner).copied().unwrap_or_default(),
            };
            c.frame = c.local.map(|l| owner.mul(&l));
        }
        for m in &mut self.mates {
            if let Some((a, b)) = self.solved.mate_frames.get(&m.id) {
                m.frame_a = Some(*a);
                m.frame_b = Some(*b);
            }
            if let Some(v) = self.solved.mate_values.get(&m.id) {
                m.values = *v;
            }
        }
    }

    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        let mut acc: Option<(DVec3, DVec3)> = None;
        for o in self.occurrences.iter().filter(|o| !o.hidden) {
            if let Some((lo, hi)) = o.world_bounds() {
                acc = Some(match acc {
                    None => (lo, hi),
                    Some((a, b)) => (a.min(lo), b.max(hi)),
                });
            }
        }
        acc
    }

    pub fn node(&self, path: &[InstanceId]) -> Option<&InstanceNode> {
        fn find<'a>(nodes: &'a [InstanceNode], path: &[InstanceId]) -> Option<&'a InstanceNode> {
            for n in nodes {
                if n.path == path {
                    return Some(n);
                }
                if path.starts_with(&n.path)
                    && let Some(x) = find(&n.children, path)
                {
                    return Some(x);
                }
            }
            None
        }
        find(&self.nodes, path)
    }

    /// World frame of a connector on an occurrence (for previews).
    pub fn occurrence_transform(&self, path: &[InstanceId]) -> Option<DMat4> {
        self.occurrences.iter().find(|o| o.path == path).map(|o| o.transform)
    }

    /// World frame of an assembly mate connector.
    pub fn connector_frame(&self, id: MateId) -> Option<Frame> {
        self.connectors.iter().find(|c| c.id == id).and_then(|c| c.frame)
    }

    /// World-space bounds of a top-level instance (display transforms).
    pub fn instance_bounds(&self, id: InstanceId) -> Option<(DVec3, DVec3)> {
        self.occurrences.iter().filter(|o| o.top() == id).filter_map(|o| o.world_bounds()).fold(None, |acc, (a, b)| Some(acc.map(|(x, y): (DVec3, DVec3)| (x.min(a), y.max(b))).unwrap_or((a, b))))
    }

    /// World frame of any mate connector reference (occurrence geometry,
    /// assembly connector or the origin) at the current display transforms.
    pub fn connector_world(&self, c: &MateConnectorRef, units: &UnitPrefs) -> Option<Frame> {
        let mut f = if let Some(id) = c.connector {
            self.connector_frame(id)?
        } else if c.instance == InstanceId::ORIGIN {
            Frame::default()
        } else {
            let mut path = vec![c.instance];
            path.extend(c.path.iter().copied());
            let o = self.occurrences.iter().find(|o| o.path == path)?;
            let local = resolve_connector_aligned(&o.context, Some(&o.body), &c.reference, c.align.as_deref(), units)?;
            matrix_frame(&o.transform).mul(&local)
        };
        if c.flip {
            f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
        }
        if !c.rotate_quarters.is_multiple_of(4) {
            f = f.mul(&Frame::new(DVec3::ZERO, quarter_rot(c.rotate_quarters)));
        }
        Some(f)
    }

    /// Current values of the mates a named position stores (all mates with
    /// free values that can be imposed: not fastened, ball, tangent or width).
    pub fn named_position_values(&self, asm: &Assembly) -> Vec<(MateId, [f64; 4])> {
        asm.mates
            .iter()
            .filter(|m| !m.suppressed && !matches!(m.mate_type, MateType::Fastened | MateType::Ball | MateType::Tangent | MateType::Width))
            .filter(|m| self.mates.iter().any(|s| s.id == m.id && s.ok && !s.inactive))
            .filter_map(|m| self.solved.mate_values.get(&m.id).map(|v| (m.id, *v)))
            .collect()
    }

    /// Relation reference values (the current free values of its two mates)
    /// to store in [`Relation::base`] when a relation is created or edited.
    pub fn relation_base(&self, asm: &Assembly, kind: RelationType, mates: [MateId; 2]) -> Option<[f64; 2]> {
        let ta = asm.mate(mates[0])?.mate_type;
        let tb = asm.mate(mates[1])?.mate_type;
        let (ia, ib) = relation_values(kind, ta, tb).ok()?;
        let va = self.solved.mate_values.get(&mates[0])?;
        let vb = self.solved.mate_values.get(&if kind == RelationType::Screw { mates[0] } else { mates[1] })?;
        Some([va[ia], vb[ib]])
    }
}

/// Bounds center of a top-level instance under an extra display matrix.
fn instance_center(result: &AssemblyResult, id: InstanceId, pre: DMat4) -> Option<DVec3> {
    let mut bb: Option<(DVec3, DVec3)> = None;
    for o in result.occurrences.iter().filter(|o| o.top() == id) {
        let Some((lo, hi)) = o.local_bounds else { continue };
        let m = pre * o.transform;
        for i in 0..8 {
            let c = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
            let w = m.transform_point3(c);
            bb = Some(bb.map(|(a, b)| (a.min(w), b.max(w))).unwrap_or((w, w)));
        }
    }
    bb.map(|(a, b)| (a + b) * 0.5)
}

/// Per-instance display matrices of an exploded view played to `progress`,
/// and the explode lines (instance center before / after each step).
/// Steps compose in order; rotation pivots default to the current bounds
/// center of the step's instances.
fn explode_matrices(result: &AssemblyResult, ev: &ExplodedView, progress: f64) -> (HashMap<InstanceId, DMat4>, Vec<(DVec3, DVec3)>) {
    let mut acc: HashMap<InstanceId, DMat4> = HashMap::new();
    let mut trails = Vec::new();
    for (k, step) in ev.steps.iter().enumerate() {
        let t = (progress - k as f64).clamp(0.0, 1.0);
        if t <= 0.0 {
            break;
        }
        let before: Vec<(InstanceId, Option<DVec3>)> = step.instances.iter().map(|id| (*id, instance_center(result, *id, acc.get(id).copied().unwrap_or(DMat4::IDENTITY)))).collect();
        let mut m = DMat4::from_translation(DVec3::from(step.translation) * t);
        if let Some(axis) = step.rotation_axis {
            let pivot = match step.pivot {
                Some(p) => DVec3::from(p),
                None => {
                    let mut bb: Option<(DVec3, DVec3)> = None;
                    for o in result.occurrences.iter().filter(|o| step.instances.contains(&o.top())) {
                        let Some((lo, hi)) = o.local_bounds else { continue };
                        let pre = acc.get(&o.top()).copied().unwrap_or(DMat4::IDENTITY) * o.transform;
                        for i in 0..8 {
                            let c = DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
                            let w = pre.transform_point3(c);
                            bb = Some(bb.map(|(a, b)| (a.min(w), b.max(w))).unwrap_or((w, w)));
                        }
                    }
                    bb.map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO)
                }
            };
            m = m * DMat4::from_translation(pivot) * DMat4::from_axis_angle(DVec3::from(axis).normalize_or_zero(), step.rotation_angle * t) * DMat4::from_translation(-pivot);
        }
        for id in &step.instances {
            let e = acc.entry(*id).or_insert(DMat4::IDENTITY);
            *e = m * *e;
        }
        for (id, c0) in before {
            if let (Some(a), Some(b)) = (c0, instance_center(result, id, acc.get(&id).copied().unwrap_or(DMat4::IDENTITY)))
                && a.distance(b) > 1e-12
            {
                trails.push((a, b));
            }
        }
    }
    (acc, trails)
}

/// Explode lines of an exploded view played to `progress` (for a result
/// whose occurrence transforms are the assembled ones).
pub fn explode_trails(result: &AssemblyResult, ev: &ExplodedView, progress: f64) -> Vec<(DVec3, DVec3)> {
    explode_matrices(result, ev, progress).1
}

struct StudioCacheEntry {
    hash: u64,
    regen: Regenerator,
    result: Arc<RegenResult>,
}

/// Cache key of a mirrored part: (tab, configuration, body, plane bits).
type MirrorKey = (TabId, String, BodyId, [u64; 6]);

/// A regenerated external Part Studio: (its tab in the pinned document, result).
pub type ExternalStudio = (TabId, Arc<RegenResult>);

/// Resolves Part Studios of other documents (version-pinned snapshots) for
/// [`InstanceSource::External`] instances. Implemented by the app.
pub trait ExternalStudios: Send + Sync {
    /// The pinned document and the Part Studio tab a key refers to.
    fn studio(&self, key: &str) -> Option<(Arc<Document>, TabId)>;
}

/// Evaluates assemblies; keeps Part Studio regenerations cached across calls.
#[derive(Default)]
pub struct AssemblyEvaluator {
    studios: HashMap<(TabId, String), StudioCacheEntry>,
    /// Mirrored parts: (source regeneration, mirrored result).
    mirrored: HashMap<MirrorKey, (Arc<RegenResult>, Arc<RegenResult>)>,
    external: Option<Arc<dyn ExternalStudios>>,
    /// Regenerated external Part Studios by (key, configuration); pinned
    /// snapshots never change.
    external_studios: HashMap<(String, String), Option<ExternalStudio>>,
}

/// Reflect a mesh across the plane through `p` with unit normal `n`
/// (triangle winding reversed so faces stay front-facing).
fn mirror_mesh(m: &MeshBuffers, p: DVec3, n: DVec3) -> MeshBuffers {
    let rp = |q: &[f32; 3]| -> [f32; 3] {
        let v = DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64);
        let w = v - n * (2.0 * (v - p).dot(n));
        [w.x as f32, w.y as f32, w.z as f32]
    };
    let rv = |q: &[f32; 3]| -> [f32; 3] {
        let v = DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64);
        let w = v - n * (2.0 * v.dot(n));
        [w.x as f32, w.y as f32, w.z as f32]
    };
    let mut indices = m.indices.clone();
    for t in indices.as_chunks_mut::<3>().0 {
        t.swap(1, 2);
    }
    MeshBuffers {
        positions: m.positions.iter().map(rp).collect(),
        normals: m.normals.iter().map(rv).collect(),
        indices,
        face_ranges: m.face_ranges.clone(),
        edges: m.edges.iter().map(|pl| pl.iter().map(rp).collect()).collect(),
        vertices: m.vertices.iter().map(rp).collect(),
    }
}

/// A regeneration result holding only `body`, mirrored across a plane
/// (kernel mirror transform; face, edge and vertex names are kept).
pub fn mirrored_result(src: &RegenResult, body: &BodyId, point: DVec3, normal: DVec3) -> Option<RegenResult> {
    let n = normal.normalize_or_zero();
    if n == DVec3::ZERO {
        return None;
    }
    let b = src.context.body(body)?;
    let shape = b.shape.transformed(&noshape_kernel::Transform::Mirror(noshape_kernel::Plane::from_normal(point, n))).ok()?;
    let mut nb = b.renamed(b.name.clone());
    nb.shape = shape;
    let mut ctx: Context = (*src.context).clone();
    ctx.bodies = vec![Arc::new(nb)];
    let reflect = |v: DVec3| v - n * (2.0 * v.dot(n));
    for c in &mut ctx.connectors {
        c.origin = point + reflect(c.origin - point);
        c.x = reflect(c.x);
        c.z = reflect(c.z);
    }
    let mut display = src.display.clone();
    display.bodies.retain(|d| &d.id == body);
    for d in &mut display.bodies {
        let mesh = mirror_mesh(&d.mesh, point, n);
        d.bounds = mesh.positions.iter().map(|q| DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)).fold(None, |acc: Option<(DVec3, DVec3)>, v| Some(acc.map(|(a, b)| (a.min(v), b.max(v))).unwrap_or((v, v))));
        d.mesh = Arc::new(mesh);
    }
    Some(RegenResult { display, needs_refine: false, context: Arc::new(ctx) })
}

fn hash_of<T: serde::Serialize>(v: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(v).unwrap_or_default().hash(&mut h);
    h.finish()
}

fn eval_len(e: &Option<ExprValue>, units: &UnitPrefs) -> f64 {
    e.as_ref().and_then(|e| noshape_units::evaluate_as(&e.text, Dim::LENGTH, units, &EmptyScope).ok()).map(|q| q.value).unwrap_or(0.0)
}

fn eval_angle(e: &Option<ExprValue>, units: &UnitPrefs) -> f64 {
    e.as_ref().and_then(|e| noshape_units::evaluate_as(&e.text, Dim::ANGLE, units, &EmptyScope).ok()).map(|q| q.value).unwrap_or(0.0)
}

fn eval_opt(e: &Option<ExprValue>, dim: Dim, units: &UnitPrefs) -> Option<f64> {
    let e = e.as_ref()?;
    if e.text.trim().is_empty() {
        return None;
    }
    noshape_units::evaluate_as(&e.text, dim, units, &EmptyScope).ok().map(|q| q.value)
}

/// Limit key -> solver free value index.
pub fn limit_index(key: &str) -> Option<usize> {
    match key {
        "rz" => Some(0),
        "x" => Some(1),
        "y" => Some(2),
        "z" => Some(3),
        // Ball mate: maximum cone angle between the Z axes.
        "cone" => Some(4),
        _ => None,
    }
}

/// Which limit keys apply to a mate type.
pub fn limit_keys(t: MateType) -> &'static [&'static str] {
    match t {
        MateType::Revolute => &["rz"],
        MateType::Slider => &["z"],
        MateType::Cylindrical => &["z", "rz"],
        MateType::PinSlot => &["x", "rz"],
        MateType::Planar => &["x", "y", "rz"],
        MateType::Parallel => &["z"],
        MateType::Ball => &["cone"],
        _ => &[],
    }
}

/// Degrees of freedom of a mate type that can be driven (Animate, named
/// positions): (free value index, Onshape `MateDOFType` label).
pub fn mate_dofs(t: MateType) -> &'static [(usize, &'static str)] {
    match t {
        MateType::Revolute => &[(0, "Rotational Z")],
        MateType::Slider => &[(3, "Translational Z")],
        MateType::Cylindrical => &[(3, "Translational Z"), (0, "Rotational Z")],
        MateType::PinSlot => &[(1, "Translational X"), (0, "Rotational Z")],
        MateType::Planar => &[(1, "Translational X"), (2, "Translational Y"), (0, "Rotational Z")],
        MateType::Parallel => &[(1, "Translational X"), (2, "Translational Y"), (3, "Translational Z"), (0, "Rotational Z")],
        _ => &[],
    }
}

/// Which offset axes apply to a mate type: (x, y, z, rotation).
pub fn offset_axes(t: MateType) -> (bool, bool, bool, bool) {
    match t {
        MateType::Fastened => (true, true, true, true),
        MateType::Revolute => (false, false, true, true),
        MateType::Slider => (true, true, false, true),
        MateType::PinSlot | MateType::Planar => (false, false, true, true),
        _ => (false, false, false, false),
    }
}

/// Limits, offsets, rotation offset and alignment of a mate on its solver mate.
fn fill_solver_mate(m: &super::Mate, sm: &mut SolverMate, units: &UnitPrefs) {
    let mut limits = Vec::new();
    for (key, l) in &m.limits {
        let Some(ix) = limit_index(key) else { continue };
        if !limit_keys(m.mate_type).contains(&key.as_str()) {
            continue;
        }
        let dim = if ix == 0 || ix == 4 { Dim::ANGLE } else { Dim::LENGTH };
        let lo = eval_opt(&l.min, dim, units);
        let hi = eval_opt(&l.max, dim, units);
        if lo.is_some() || hi.is_some() {
            limits.push((ix, lo, hi));
        }
    }
    let (ox, oy, oz, orot) = offset_axes(m.mate_type);
    sm.offset = DVec3::new(
        if ox { eval_len(&m.offset_x, units) } else { 0.0 },
        if oy { eval_len(&m.offset_y, units) } else { 0.0 },
        if oz { eval_len(&m.offset_z, units) } else { 0.0 },
    );
    let rot = if orot { eval_angle(&m.rotation, units) } else { 0.0 };
    if m.rotation_axis < 2 && rot != 0.0 {
        // Rotation about the first connector's X or Y axis: the offset
        // moves its whole coordinate system (translation, then rotation).
        let axis = if m.rotation_axis == 0 { DVec3::X } else { DVec3::Y };
        sm.a_local = sm.a_local.mul(&Frame::new(sm.offset, DQuat::from_axis_angle(axis, rot)));
        sm.offset = DVec3::ZERO;
        sm.rotation = 0.0;
    } else {
        sm.rotation = rot;
    }
    sm.kind = m.mate_type;
    sm.flip_primary = m.flip_primary;
    sm.reorient = m.reorient_secondary;
    sm.limits = limits;
}

/// Merge instance configuration over the studio defaults.
fn effective_config(doc: &Document, studio: TabId, cfg: &ConfigValues) -> ConfigValues {
    let mut base = match doc.tab(studio).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => ps.configuration.defaults(),
        _ => ConfigValues::default(),
    };
    for (k, v) in &cfg.0 {
        base.0.insert(k.clone(), v.clone());
    }
    base
}

/// Resolve a mate connector reference in a regenerated studio, restricted to
/// one body when given. Returns the frame in studio coordinates.
pub fn resolve_connector_in(ctx: &Arc<Context>, body: Option<&BodyId>, reference: &Reference, units: &UnitPrefs) -> Option<Frame> {
    resolve_connector_aligned(ctx, body, reference, None, units)
}

fn connector_frame(origin: DVec3, z: DVec3) -> Frame {
    let z = z.normalize();
    let mut x = DVec3::X - z * z.x;
    if x.length_squared() < 1e-12 {
        x = DVec3::Y - z * z.y;
    }
    Frame::from_axes(origin, z, x.normalize())
}

/// Like [`resolve_connector_in`], with the primary axis taken from `align`
/// (a face: its normal at the connector origin; an edge: its tangent there).
/// Also resolves the inference points the regenerator does not know:
/// a cone's virtual sharp (`Center` on a conical face) and the center of a
/// face loop (`Centroid` on an edge, aligned to the face).
pub fn resolve_connector_aligned(ctx: &Arc<Context>, body: Option<&BodyId>, reference: &Reference, align: Option<&Reference>, units: &UnitPrefs) -> Option<Frame> {
    let mut c: Context = (**ctx).clone();
    if let Some(b) = body {
        c.bodies.retain(|x| &x.id == b);
    }
    let params = ParamValues::default();
    let vars = BTreeMap::new();
    let ev = EvalCtx::new(FeatureId(0), "", &params, units, c, &vars);
    let special = match reference {
        Reference::Implicit { on, inference: Inference::Center } if matches!(**on, Reference::Face(_)) => {
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(&**on)).first()?;
            match ev.face_shape(bi, fi).ok()?.face_surface().ok()? {
                noshape_kernel::Surface::Cone { axis, apex, .. } => Some(connector_frame(apex, axis)),
                noshape_kernel::Surface::Sphere { center, axis, .. } => Some(connector_frame(center, axis)),
                _ => None,
            }
        }
        Reference::Implicit { on, inference: Inference::Centroid } if matches!(**on, Reference::Edge(_)) => {
            let face_ref = align.filter(|a| matches!(a, Reference::Face(_)))?;
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(face_ref)).first()?;
            let (be, ei) = *ev.resolve_edges(std::slice::from_ref(&**on)).first()?;
            let face = ev.face_shape(bi, fi).ok()?;
            let edge = ev.edge_shape(be, ei).ok()?;
            let wire = face.wires().into_iter().find(|w| w.edges().iter().any(|e| e.is_same(&edge)))?;
            let p = noshape_kernel::make_face_from_wire(&wire, true).ok().and_then(|f| f.centroid().ok()).or_else(|| {
                // Not a planar loop: length-weighted edge midpoints.
                let (mut sum, mut len) = (DVec3::ZERO, 0.0);
                for e in wire.edges() {
                    let i = e.edge_info().ok()?;
                    let m = e.point_at((i.first_param + i.last_param) * 0.5).ok()?;
                    sum += m * i.length;
                    len += i.length;
                }
                (len > 0.0).then(|| sum / len)
            })?;
            Some(connector_frame(p, face.normal_at_point(p).ok()?))
        }
        _ => None,
    };
    let frame = match special {
        Some(f) => f,
        None => {
            let conn = match reference {
                Reference::MateConnector { .. } | Reference::Implicit { .. } => ev.resolve_connector(reference)?,
                // A bare entity: infer its default connector.
                other => crate::regen::implicit::implicit_connector(&ev, other, default_inference(other))?,
            };
            Frame::from_axes(conn.origin, conn.z, conn.x)
        }
    };
    let Some(align) = align else { return Some(frame) };
    let z = match align {
        Reference::Face(_) => {
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(align)).first()?;
            ev.face_shape(bi, fi).ok()?.normal_at_point(frame.origin).ok()?
        }
        Reference::Edge(_) => {
            let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(align)).first()?;
            let e = ev.edge_shape(bi, ei).ok()?;
            let i = e.edge_info().ok()?;
            // Tangent at the edge parameter nearest to the connector origin.
            let ts = [i.first_param, (i.first_param + i.last_param) * 0.5, i.last_param];
            let t = ts.iter().copied().min_by(|a, b| {
                let da = e.point_at(*a).map(|p| p.distance(frame.origin)).unwrap_or(f64::MAX);
                let db = e.point_at(*b).map(|p| p.distance(frame.origin)).unwrap_or(f64::MAX);
                da.total_cmp(&db)
            })?;
            e.tangent_at(t).ok()?
        }
        _ => return Some(frame),
    };
    Some(connector_frame(frame.origin, z))
}

fn default_inference(r: &Reference) -> Inference {
    match r {
        Reference::Vertex(_) => Inference::Vertex,
        Reference::Edge(_) => Inference::Midpoint,
        Reference::Origin => Inference::Origin,
        _ => Inference::Centroid,
    }
}

/// One implicit mate connector candidate on a hovered entity.
#[derive(Clone, Debug, PartialEq)]
pub struct InferencePoint {
    pub reference: Reference,
    /// Primary axis entity (see [`MateConnectorRef::align`]).
    pub align: Option<Reference>,
    /// Frame in studio coordinates.
    pub frame: Frame,
}

impl InferencePoint {
    /// The mate connector this point stands for on an occurrence.
    pub fn connector(&self, path: &[InstanceId]) -> MateConnectorRef {
        let mut c = MateConnectorRef::on(path[0], path[1..].to_vec(), self.reference.clone());
        c.align = self.align.clone().map(Box::new);
        c
    }
}

/// Implicit mate connector candidates on an entity (studio coordinates),
/// the points Onshape shows while hovering it:
/// * planar face: centroid, every vertex, edge midpoints, arc and circle
///   centers and the centers of inner loops (slots, cut outs), all with the
///   primary axis normal to the face;
/// * cylindrical face: axis points at both ends and the middle;
/// * conical face: the same plus the virtual sharp (apex);
/// * spherical face: the center; other faces: the centroid;
/// * circular edge: its center only; arc: center, midpoint and ends;
///   other edges: midpoint and ends (primary axis along the edge);
/// * vertex: the point.
pub fn inference_candidates(ctx: &Arc<Context>, body: Option<&BodyId>, on: &Reference, units: &UnitPrefs) -> Vec<InferencePoint> {
    let implicit = |on: &Reference, inference: Inference| Reference::Implicit { on: Box::new(on.clone()), inference };
    let mut refs: Vec<(Reference, Option<Reference>)> = Vec::new();
    let bodies = || ctx.bodies.iter().filter(move |b| body.is_none_or(|id| &b.id == id));
    match on {
        Reference::Face(n) => {
            let mut kind = None;
            for b in bodies() {
                let Some(&fi) = b.find_faces(n).first() else { continue };
                let Ok(face) = b.shape.sub_shape(noshape_kernel::ShapeType::Face, fi) else { continue };
                kind = face.face_surface().ok();
                if !matches!(kind, Some(noshape_kernel::Surface::Plane(_))) {
                    break;
                }
                refs.push((implicit(on, Inference::Centroid), None));
                // Edges and vertices of the face.
                let Ok(edge_faces) = b.shape.edge_faces() else { break };
                // Outer loop: the wire with the largest extent (it encloses the others).
                let outer = face.wires().into_iter().max_by(|a, b| {
                    let d = |w: &noshape_kernel::Shape| w.bounding_box(false).map(|bb| bb.diagonal()).unwrap_or(0.0);
                    d(a).total_cmp(&d(b))
                });
                // Edges bounding the face that have two distinct ends (vertices
                // only on closed edges, like a circle's seam, are not offered).
                let mut open_edges: HashSet<u32> = HashSet::new();
                for (ei, faces) in edge_faces.iter().enumerate() {
                    if !faces.contains(&(fi as u32)) {
                        continue;
                    }
                    let (Some(en), Ok(e)) = (b.edges.get(ei), b.shape.sub_shape(noshape_kernel::ShapeType::Edge, ei)) else { continue };
                    let Ok(info) = e.edge_info() else { continue };
                    if !info.closed {
                        open_edges.insert(ei as u32);
                    }
                    let edge_ref = Reference::Edge((**en).clone());
                    if matches!(info.curve, noshape_kernel::Curve::Circle { .. }) {
                        refs.push((implicit(&edge_ref, Inference::Center), Some(on.clone())));
                    }
                    if !info.closed {
                        refs.push((implicit(&edge_ref, Inference::Midpoint), Some(on.clone())));
                    }
                    // Inner loops (holes, slots): their center.
                    let inner = outer.as_ref().is_some_and(|w| !w.edges().iter().any(|x| x.is_same(&e)));
                    if inner && !info.closed {
                        refs.push((implicit(&edge_ref, Inference::Centroid), Some(on.clone())));
                    }
                }
                if let Ok(ve) = b.shape.vertex_edges() {
                    for (vi, edges) in ve.iter().enumerate() {
                        if edges.iter().any(|e| open_edges.contains(e))
                            && let Some(vn) = b.vertices.get(vi)
                        {
                            refs.push((implicit(&Reference::Vertex((**vn).clone()), Inference::Vertex), Some(on.clone())));
                        }
                    }
                }
                break;
            }
            match kind {
                Some(noshape_kernel::Surface::Plane(_)) => {}
                Some(noshape_kernel::Surface::Cylinder { .. }) => {
                    for inf in [Inference::MidAxis, Inference::TopAxis, Inference::BottomAxis] {
                        refs.push((implicit(on, inf), None));
                    }
                }
                Some(noshape_kernel::Surface::Cone { .. }) => {
                    for inf in [Inference::MidAxis, Inference::TopAxis, Inference::BottomAxis, Inference::Center] {
                        refs.push((implicit(on, inf), None));
                    }
                }
                Some(noshape_kernel::Surface::Sphere { .. }) => refs.push((implicit(on, Inference::Center), None)),
                _ => refs.push((implicit(on, Inference::Centroid), None)),
            }
        }
        Reference::Edge(n) => {
            let mut done = false;
            for b in bodies() {
                let Some(&ei) = b.find_edges(n).first() else { continue };
                let Ok(e) = b.shape.sub_shape(noshape_kernel::ShapeType::Edge, ei) else { continue };
                let Ok(info) = e.edge_info() else { continue };
                let circle = matches!(info.curve, noshape_kernel::Curve::Circle { .. });
                if circle {
                    refs.push((implicit(on, Inference::Center), None));
                }
                if info.closed && !circle {
                    refs.push((implicit(on, Inference::Midpoint), None));
                }
                if !info.closed {
                    refs.push((implicit(on, Inference::Midpoint), None));
                    // Both ends, primary axis along the edge.
                    if let Ok(ve) = b.shape.vertex_edges() {
                        for (vi, edges) in ve.iter().enumerate() {
                            if edges.contains(&(ei as u32))
                                && let Some(vn) = b.vertices.get(vi)
                            {
                                refs.push((implicit(&Reference::Vertex((**vn).clone()), Inference::Vertex), Some(on.clone())));
                            }
                        }
                    }
                }
                done = true;
                break;
            }
            if !done {
                // Sketch curves and edges outside the restricted body.
                refs.push((implicit(on, Inference::Center), None));
                refs.push((implicit(on, Inference::Midpoint), None));
            }
        }
        Reference::Vertex(_) => refs.push((implicit(on, Inference::Vertex), None)),
        Reference::Origin => refs.push((implicit(on, Inference::Origin), None)),
        Reference::MateConnector { .. } => refs.push((on.clone(), None)),
        _ => {}
    }
    let mut out: Vec<InferencePoint> = Vec::new();
    for (r, align) in refs {
        if let Some(f) = resolve_connector_aligned(ctx, body, &r, align.as_ref(), units)
            && !out.iter().any(|p| p.frame.origin.distance(f.origin) < 1e-9 && p.frame.z().dot(f.z()) > 0.9999)
        {
            out.push(InferencePoint { reference: r, align, frame: f });
        }
    }
    out
}

/// Nearest inference point to a point (body coordinates), used for Shift
/// (lock mate inference): the connector snaps to the closest point of the
/// locked entity.
pub fn nearest_inference(points: &[InferencePoint], p: DVec3) -> Option<usize> {
    points.iter().enumerate().min_by(|(_, a), (_, b)| a.frame.origin.distance(p).total_cmp(&b.frame.origin.distance(p))).map(|(i, _)| i)
}

impl AssemblyEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Regenerate a Part Studio for a configuration (cached).
    pub fn studio(&mut self, doc: &Document, tab: TabId, config: &ConfigValues) -> Option<Arc<RegenResult>> {
        let ps = match &doc.tab(tab)?.content {
            TabContent::PartStudio(ps) => ps,
            _ => return None,
        };
        let cfg = effective_config(doc, tab, config);
        let key = (tab, serde_json::to_string(&cfg).unwrap_or_default());
        let hash = hash_of(&(ps, &doc.settings.units));
        let entry = self.studios.entry(key).or_insert_with(|| StudioCacheEntry { hash: hash ^ 1, regen: Regenerator::new(), result: Arc::new(RegenResult::default()) });
        if entry.hash != hash {
            let r = entry.regen.regenerate(ps, &cfg, &doc.settings.units, &RegenOptions::default());
            entry.result = Arc::new(r);
            entry.hash = hash;
        }
        Some(entry.result.clone())
    }

    /// Set the resolver for parts inserted from other documents.
    pub fn set_external(&mut self, provider: Arc<dyn ExternalStudios>) {
        self.external = Some(provider);
    }

    /// Regenerate a Part Studio of another document (cached per key and configuration).
    pub fn external_studio(&mut self, key: &str, config: &ConfigValues) -> Option<ExternalStudio> {
        let ck = (key.to_string(), serde_json::to_string(config).unwrap_or_default());
        if let Some(r) = self.external_studios.get(&ck) {
            return r.clone();
        }
        let provider = self.external.clone()?;
        let r = provider.studio(key).and_then(|(doc, tab)| {
            let TabContent::PartStudio(ps) = &doc.tab(tab)?.content else { return None };
            let cfg = effective_config(&doc, tab, config);
            let r = Regenerator::new().regenerate(ps, &cfg, &doc.settings.units, &RegenOptions::default());
            Some((tab, Arc::new(r)))
        });
        self.external_studios.insert(ck, r.clone());
        r
    }

    /// Mirrored version of one part of a regenerated studio (cached).
    fn mirrored_studio(&mut self, tab: TabId, config: &ConfigValues, body: &BodyId, src: &Arc<RegenResult>, point: DVec3, normal: DVec3) -> Option<Arc<RegenResult>> {
        let bits = [point.x.to_bits(), point.y.to_bits(), point.z.to_bits(), normal.x.to_bits(), normal.y.to_bits(), normal.z.to_bits()];
        let key = (tab, serde_json::to_string(config).unwrap_or_default(), body.clone(), bits);
        if let Some((s, r)) = self.mirrored.get(&key)
            && Arc::ptr_eq(s, src)
        {
            return Some(r.clone());
        }
        let r = Arc::new(mirrored_result(src, body, point, normal)?);
        self.mirrored.insert(key, (src.clone(), r.clone()));
        Some(r)
    }

    /// Drop cache entries for tabs that no longer exist.
    pub fn retain_tabs(&mut self, doc: &Document) {
        self.studios.retain(|(t, _), _| doc.tab(*t).is_some());
        self.mirrored.retain(|(t, _, _, _), _| doc.tab(*t).is_some());
    }

    /// Full evaluation: prepare, solve, build display.
    pub fn evaluate(&mut self, doc: &Document, assembly: &Assembly, opts: &SolveOptions, explode: Option<usize>) -> AssemblyResult {
        let t0 = Instant::now();
        let mut visited = Vec::new();
        let (leaves, nodes, prepared) = self.prepare_inner(doc, assembly, &mut visited);
        let solved = prepared.solve(opts);
        let mut occurrences = Vec::new();
        for leaf in &leaves {
            for b in &leaf.regen.display.bodies {
                if let Some(id) = &leaf.body
                    && &b.id != id
                {
                    continue;
                }
                if leaf.body.is_none() && (b.hidden || matches!(b.kind, BodyKind::Wire | BodyKind::Point)) {
                    continue;
                }
                occurrences.push(PartOccurrence {
                    path: leaf.path.clone(),
                    body: b.id.clone(),
                    studio: leaf.studio,
                    config: leaf.config.clone(),
                    instance_name: leaf.name.clone(),
                    body_name: b.name.clone(),
                    kind: b.kind,
                    color: b.color,
                    mesh: b.mesh.clone(),
                    inner: leaf.inner,
                    transform: leaf.inner,
                    face_refs: b.face_refs.clone(),
                    edge_refs: b.edge_refs.clone(),
                    vertex_refs: b.vertex_refs.clone(),
                    hidden: leaf.hidden,
                    local_bounds: b.bounds,
                    context: leaf.regen.context.clone(),
                    mirrored: leaf.mirrored,
                    external: leaf.external.clone(),
                    flex: leaf.flex,
                });
            }
        }
        let mut mates = Vec::new();
        for m in &assembly.mates {
            let failed = prepared.failed_mates.iter().find(|(id, _)| *id == m.id).map(|(_, s)| s.clone());
            let inactive = prepared.inactive_mates.contains(&m.id);
            let ok = !inactive && failed.is_none() && solved.mate_ok.get(&m.id).copied().unwrap_or(false);
            let message = if inactive {
                None
            } else if let Some(f) = failed {
                Some(f)
            } else if !ok {
                Some("Mate could not be solved (over-constrained or conflicting)".into())
            } else {
                None
            };
            mates.push(MateStatus { id: m.id, ok: ok || inactive, inactive, message, frame_a: None, frame_b: None, values: [0.0; 4] });
        }
        let relations = assembly
            .relations
            .iter()
            .map(|r| {
                if let Some((_, msg)) = prepared.failed_relations.iter().find(|(id, _)| *id == r.id) {
                    return RelationStatus { id: r.id, ok: false, message: Some(msg.clone()) };
                }
                let ok = r.suppressed || solved.relation_ok.get(&r.id).copied().unwrap_or(false);
                RelationStatus { id: r.id, ok, message: (!ok).then(|| "Relation could not be solved (conflicts with other mates or relations)".into()) }
            })
            .collect();
        let connectors = assembly
            .connectors
            .iter()
            .map(|c| {
                let resolved = prepared.connectors.iter().find(|(id, _, _)| *id == c.id);
                let message = prepared.failed_connectors.iter().find(|(id, _)| *id == c.id).map(|(_, m)| m.clone());
                ConnectorStatus { id: c.id, owner: resolved.map(|r| r.1).unwrap_or(c.on.instance), child: prepared.connector_child.get(&c.id).copied(), local: resolved.map(|r| r.2), frame: None, message }
            })
            .collect();
        let mut nodes = nodes;
        // Copies of linear / circular / mirror patterns follow their seed: no own DOF.
        let followers: HashSet<InstanceId> = assembly
            .patterns
            .iter()
            .filter(|p| matches!(p.kind, PatternKind::Linear { .. } | PatternKind::Circular { .. } | PatternKind::Mirror { .. }))
            .flat_map(|p| p.generated.iter().map(|g| g.instance))
            .collect();
        for n in &mut nodes {
            let id = n.path[0];
            n.dof = Some(if followers.contains(&id) { 0 } else { solved.instance_dof.get(&id).copied().unwrap_or(0) });
        }
        let pattern_errors = prepared.failed_patterns.clone();
        let mut result = AssemblyResult { occurrences, nodes, solved: Solved::default(), mates, relations, connectors, pattern_errors, prepared: Arc::new(prepared), eval_ms: 0.0, explode_trails: Vec::new() };
        let ev = explode.and_then(|i| assembly.exploded_views.get(i));
        result.apply_placements(solved, ev);
        result.eval_ms = t0.elapsed().as_secs_f32() * 1000.0;
        result
    }

    /// Flatten an assembly into leaves (relative to their top-level
    /// instance), list nodes and the prepared solver problem.
    fn prepare_inner(&mut self, doc: &Document, asm: &Assembly, visited: &mut Vec<u64>) -> (Vec<LeafInstance>, Vec<InstanceNode>, Prepared) {
        let units = doc.settings.units;
        let mut leaves: Vec<LeafInstance> = Vec::new();
        let mut nodes = Vec::new();
        let mut prepared = Prepared::default();
        // Flexible subassemblies: (instance, content, stored child placements in the subassembly).
        let mut flex_subs: Vec<(InstanceId, Assembly, HashMap<InstanceId, Frame>)> = Vec::new();
        // Copies of a suppressed pattern (or of a suppressed seed) are inactive.
        let mut inactive: HashSet<InstanceId> = asm.instances.iter().filter(|i| i.suppressed).map(|i| i.id).collect();
        for p in &asm.patterns {
            for g in &p.generated {
                if p.suppressed || inactive.contains(&g.seed) {
                    inactive.insert(g.instance);
                }
            }
        }
        for inst in &asm.instances {
            let suppressed = inactive.contains(&inst.id);
            let mut node = InstanceNode {
                path: vec![inst.id],
                name: inst.name.clone(),
                kind: NodeKind::Part,
                fixed: inst.fixed,
                hidden: inst.hidden,
                suppressed,
                dof: None,
                error: None,
                children: Vec::new(),
            };
            match &inst.source {
                InstanceSource::Part { studio, body, config } => {
                    node.kind = NodeKind::Part;
                    match self.studio(doc, *studio, config) {
                        Some(r) => {
                            if r.context.body(body).is_none() {
                                node.error = Some("Part no longer exists in its Part Studio (or in this configuration)".into());
                            } else if !suppressed {
                                leaves.push(LeafInstance {
                                    path: vec![inst.id],
                                    inner: DMat4::IDENTITY,
                                    studio: *studio,
                                    body: Some(body.clone()),
                                    config: config.clone(),
                                    regen: r,
                                    name: inst.name.clone(),
                                    hidden: inst.hidden,
                                    mirrored: false,
                                    external: None,
                                    flex: None,
                                });
                            }
                        }
                        None => node.error = Some("Part Studio was deleted".into()),
                    }
                }
                InstanceSource::MirroredPart { studio, body, config, point, normal } => {
                    node.kind = NodeKind::Part;
                    match self.studio(doc, *studio, config) {
                        Some(r) => {
                            if r.context.body(body).is_none() {
                                node.error = Some("Part no longer exists in its Part Studio (or in this configuration)".into());
                            } else if !suppressed {
                                match self.mirrored_studio(*studio, config, body, &r, DVec3::from(*point), DVec3::from(*normal)) {
                                    Some(m) => leaves.push(LeafInstance {
                                        path: vec![inst.id],
                                        inner: DMat4::IDENTITY,
                                        studio: *studio,
                                        body: Some(body.clone()),
                                        config: config.clone(),
                                        regen: m,
                                        name: inst.name.clone(),
                                        hidden: inst.hidden,
                                        mirrored: true,
                                        external: None,
                                        flex: None,
                                    }),
                                    None => node.error = Some("The mirrored part could not be created".into()),
                                }
                            }
                        }
                        None => node.error = Some("Part Studio was deleted".into()),
                    }
                }
                InstanceSource::External { key, body, config } => {
                    node.kind = if body.is_some() { NodeKind::Part } else { NodeKind::PartStudio };
                    match self.external_studio(key, config) {
                        Some((tab, r)) => {
                            if body.as_ref().is_some_and(|b| r.context.body(b).is_none()) {
                                node.error = Some("Part no longer exists in this version of the other document".into());
                            } else if !suppressed {
                                leaves.push(LeafInstance {
                                    path: vec![inst.id],
                                    inner: DMat4::IDENTITY,
                                    studio: tab,
                                    body: body.clone(),
                                    config: config.clone(),
                                    regen: r,
                                    name: inst.name.clone(),
                                    hidden: inst.hidden,
                                    mirrored: false,
                                    external: Some(key.clone()),
                                    flex: None,
                                });
                            }
                        }
                        None => node.error = Some("The other document or its Part Studio could not be read".into()),
                    }
                }
                InstanceSource::PartStudio { studio, config } => {
                    node.kind = NodeKind::PartStudio;
                    match self.studio(doc, *studio, config) {
                        Some(r) => {
                            if !suppressed {
                                leaves.push(LeafInstance {
                                    path: vec![inst.id],
                                    inner: DMat4::IDENTITY,
                                    studio: *studio,
                                    body: None,
                                    config: config.clone(),
                                    regen: r,
                                    name: inst.name.clone(),
                                    hidden: inst.hidden,
                                    mirrored: false,
                                    external: None,
                                    flex: None,
                                });
                            }
                        }
                        None => node.error = Some("Part Studio was deleted".into()),
                    }
                }
                InstanceSource::Assembly { assembly } => {
                    node.kind = NodeKind::Assembly;
                    let sub = match doc.tab(*assembly).map(|t| &t.content) {
                        Some(TabContent::Assembly(a)) => Some(a),
                        _ => None,
                    };
                    match sub {
                        None => node.error = Some("Assembly was deleted".into()),
                        Some(_) if visited.contains(&assembly.0) || visited.len() > 16 => node.error = Some("Circular assembly reference".into()),
                        Some(sub) => {
                            visited.push(assembly.0);
                            let (sub_leaves, sub_nodes, sub_prep) = self.prepare_inner(doc, sub, visited);
                            visited.pop();
                            // Positions of the subassembly's instances: its named position
                            // (Lock/follow position to), else solved from the positions
                            // stored in this assembly for them.
                            let start: HashMap<InstanceId, Placement> = sub_prep.instances.iter().map(|(id, p)| (*id, inst.nested.iter().find(|(c, _)| c == id).map(|(_, q)| *q).unwrap_or(*p))).collect();
                            let solved = match &inst.lock {
                                super::SubassemblyLock::Named(n) => match sub.named_positions.iter().find(|np| &np.name == n) {
                                    Some(np) => sub_prep.solve_named_position(np),
                                    None => sub_prep.solve(&SolveOptions { start: Some(start), ..Default::default() }),
                                },
                                _ => sub_prep.solve(&SolveOptions { start: Some(start), ..Default::default() }),
                            };
                            // Flexible (Onshape's default): its mates join this assembly's
                            // solve. Subassemblies with patterns, relations, tangent mates or
                            // nested flexible subassemblies stay rigid.
                            let flexible = inst.lock.is_flexible()
                                && !suppressed
                                && sub.patterns.is_empty()
                                && sub.relations.iter().all(|r| r.suppressed)
                                && !sub.mates.iter().any(|m| m.mate_type == MateType::Tangent && !m.suppressed)
                                && sub_prep.flexible.is_empty();
                            let place: HashMap<InstanceId, DMat4> = solved.placements.iter().map(|(id, p)| (*id, frame_matrix(&placement_to_frame(p)))).collect();
                            if flexible {
                                prepared.flexible.insert(inst.id);
                                flex_subs.push((inst.id, sub.clone(), solved.placements.iter().map(|(id, p)| (*id, placement_to_frame(p))).collect()));
                            }
                            for mut l in sub_leaves {
                                let top = place.get(&l.path[0]).copied().unwrap_or(DMat4::IDENTITY);
                                if flexible {
                                    l.flex = Some((l.path[0], l.inner));
                                }
                                l.inner = top * l.inner;
                                let mut path = vec![inst.id];
                                path.extend(l.path.iter().copied());
                                l.path = path;
                                l.hidden |= inst.hidden;
                                if !suppressed {
                                    leaves.push(l);
                                }
                            }
                            node.children = sub_nodes
                                .into_iter()
                                .map(|mut n| {
                                    prefix_paths(&mut n, inst.id);
                                    n
                                })
                                .collect();
                            if !sub_prep.failed_mates.is_empty() {
                                node.error = Some("Subassembly contains mate error".into());
                            }
                        }
                    }
                }
            }
            nodes.push(node);
        }

        // ---------------------------------------------------- solver bodies
        let active: Vec<&super::Instance> = asm.instances.iter().filter(|i| !inactive.contains(&i.id)).collect();
        prepared.instances = active.iter().map(|i| (i.id, i.placement)).collect();
        let mut assigned: HashMap<InstanceId, (usize, Frame)> = HashMap::new();
        for g in asm.groups.iter().filter(|g| !g.suppressed) {
            let members: Vec<&&super::Instance> = g.instances.iter().filter_map(|id| active.iter().find(|i| i.id == *id)).filter(|i| !assigned.contains_key(&i.id)).collect();
            let Some(rep) = members.first() else { continue };
            let bi = prepared.reps.len();
            prepared.reps.push(rep.id);
            prepared.fixed.push(members.iter().any(|m| m.fixed));
            let rep_f = placement_to_frame(&rep.placement);
            for m in &members {
                let off = rep_f.inverse().mul(&placement_to_frame(&m.placement));
                assigned.insert(m.id, (bi, off));
            }
        }
        for i in &active {
            if assigned.contains_key(&i.id) {
                continue;
            }
            let bi = prepared.reps.len();
            prepared.reps.push(i.id);
            prepared.fixed.push(i.fixed);
            assigned.insert(i.id, (bi, Frame::default()));
        }
        // Flexible subassembly children: own bodies, except those fixed in the
        // subassembly (and their groups), which ride on the subassembly's body.
        for (sid, sub, q) in &flex_subs {
            let Some(&(sbody, _)) = assigned.get(sid) else { continue };
            let sub_active: Vec<&super::Instance> = sub.instances.iter().filter(|i| !i.suppressed && q.contains_key(&i.id)).collect();
            let mut done: HashSet<InstanceId> = HashSet::new();
            for g in sub.groups.iter().filter(|g| !g.suppressed) {
                let members: Vec<&&super::Instance> = g.instances.iter().filter_map(|id| sub_active.iter().find(|i| i.id == *id)).filter(|i| !done.contains(&i.id)).collect();
                let Some(rep) = members.first() else { continue };
                let on_sub = members.iter().any(|m| m.fixed);
                let rep_q = q[&rep.id];
                let body = if on_sub {
                    sbody
                } else {
                    let b = prepared.reps.len();
                    prepared.reps.push(*sid);
                    prepared.fixed.push(false);
                    prepared.nested_bodies.push((b, *sid, rep.id, rep_q));
                    b
                };
                for m in &members {
                    let off = if on_sub { q[&m.id] } else { rep_q.inverse().mul(&q[&m.id]) };
                    prepared.nested_body_of.insert((*sid, m.id), (body, off));
                    done.insert(m.id);
                }
            }
            for i in &sub_active {
                if done.contains(&i.id) {
                    continue;
                }
                if i.fixed {
                    prepared.nested_body_of.insert((*sid, i.id), (sbody, q[&i.id]));
                } else {
                    let b = prepared.reps.len();
                    prepared.reps.push(*sid);
                    prepared.fixed.push(false);
                    prepared.nested_bodies.push((b, *sid, i.id, q[&i.id]));
                    prepared.nested_body_of.insert((*sid, i.id), (b, Frame::default()));
                }
            }
            // Nothing rides on the subassembly's own frame (no part fixed in it,
            // nothing mated to its origin): the frame is bookkeeping only and
            // adds no degrees of freedom.
            let on_frame = prepared.nested_body_of.iter().any(|((s2, _), (b, _))| s2 == sid && *b == sbody);
            let origin_used = sub.mates.iter().any(|m| !m.suppressed && m.connectors().any(|c| c.is_origin())) || sub.connectors.iter().any(|c| c.on.is_origin());
            let shared = assigned.iter().filter(|(_, (b, _))| *b == sbody).count() > 1;
            if !on_frame && !origin_used && !shared {
                prepared.fixed[sbody] = true;
            }
        }
        // The assembly origin: a fixed body at the world frame.
        let ground = prepared.reps.len();
        prepared.reps.push(InstanceId::ORIGIN);
        prepared.fixed.push(true);
        assigned.insert(InstanceId::ORIGIN, (ground, Frame::default()));
        prepared.body_of = assigned;
        let stored: HashMap<InstanceId, Frame> = active.iter().map(|i| (i.id, placement_to_frame(&i.placement))).collect();

        // ---------------------------------------------------- assembly mate connectors
        // Frame of a connector relative to its owner (a flexible subassembly
        // child owns the connectors on its parts).
        let resolve_entity = |c: &MateConnectorRef| -> Result<(Owner, Frame), String> {
            if c.instance == InstanceId::ORIGIN {
                return Ok((Owner::Top(InstanceId::ORIGIN), Frame::default()));
            }
            if inactive.contains(&c.instance) {
                return Err("Instance is suppressed".into());
            }
            let mut full = vec![c.instance];
            full.extend(c.path.iter().copied());
            let leaf = leaves.iter().find(|l| l.path == full).ok_or("Instance no longer exists")?;
            let f = resolve_connector_aligned(&leaf.regen.context, leaf.body.as_ref(), &c.reference, c.align.as_deref(), &units).ok_or("Mate connector could not be resolved (missing geometry)")?;
            Ok(match leaf.flex {
                Some((child, rest)) => (Owner::Nested(c.instance, child), matrix_frame(&rest).mul(&f)),
                None => (Owner::Top(c.instance), matrix_frame(&leaf.inner).mul(&f)),
            })
        };
        let owner_body = |o: Owner, body_of: &HashMap<InstanceId, (usize, Frame)>, nested: &HashMap<(InstanceId, InstanceId), (usize, Frame)>| -> Option<(usize, Frame)> {
            match o {
                Owner::Top(id) => body_of.get(&id).copied(),
                Owner::Nested(s, c) => nested.get(&(s, c)).copied(),
            }
        };
        for mc in &asm.connectors {
            if mc.suppressed {
                continue;
            }
            match resolve_entity(&mc.on) {
                Ok((owner, f)) => {
                    let mut f = f;
                    if mc.on.flip {
                        f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
                    }
                    let top = match owner {
                        Owner::Top(id) => id,
                        Owner::Nested(sid, child) => {
                            prepared.connector_child.insert(mc.id, child);
                            sid
                        }
                    };
                    prepared.connectors.push((mc.id, top, f.mul(&asm_connector_offset(mc, &units))));
                }
                Err(e) => prepared.failed_connectors.push((mc.id, e)),
            }
        }
        // Any connector reference -> (solver body, frame in body coordinates).
        let resolve = |c: &MateConnectorRef| -> Result<(usize, Frame), String> {
            let (owner, mut f) = match c.connector {
                Some(id) => {
                    if let Some((_, msg)) = prepared.failed_connectors.iter().find(|(x, _)| *x == id) {
                        return Err(msg.clone());
                    }
                    let (_, owner, f) = prepared.connectors.iter().find(|(x, _, _)| *x == id).ok_or("Mate connector was deleted or is suppressed")?;
                    match prepared.connector_child.get(&id) {
                        Some(child) => (Owner::Nested(*owner, *child), *f),
                        None => (Owner::Top(*owner), *f),
                    }
                }
                None => resolve_entity(c)?,
            };
            if c.flip {
                f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
            }
            if !c.rotate_quarters.is_multiple_of(4) {
                f = f.mul(&Frame::new(DVec3::ZERO, quarter_rot(c.rotate_quarters)));
            }
            let (bi, off) = owner_body(owner, &prepared.body_of, &prepared.nested_body_of).ok_or("Instance no longer exists")?;
            Ok((bi, off.mul(&f)))
        };

        // ---------------------------------------------------- mates
        for m in &asm.mates {
            if m.suppressed {
                prepared.inactive_mates.insert(m.id);
                continue;
            }
            if m.connectors().any(|c| c.instance != InstanceId::ORIGIN && inactive.contains(&c.instance)) {
                prepared.inactive_mates.insert(m.id);
                continue;
            }
            let mut sm = SolverMate::default();
            if m.mate_type == MateType::Tangent {
                match self.tangent_pair(asm, &leaves, &prepared, &stored, m, &units) {
                    Ok((a, b, pair)) => {
                        sm.a = a;
                        sm.b = b;
                        sm.geom = Some(pair);
                        // Display frames: the entities' default connectors (if any).
                        sm.a_local = resolve(&m.a).map(|x| x.1).unwrap_or_default();
                        sm.b_local = resolve(&m.b).map(|x| x.1).unwrap_or_default();
                    }
                    Err(e) => {
                        prepared.failed_mates.push((m.id, e));
                        continue;
                    }
                }
            } else {
                let (a, b) = match (resolve(&m.a), resolve(&m.b)) {
                    (Ok(a), Ok(b)) => (a, b),
                    (Err(e), _) | (_, Err(e)) => {
                        prepared.failed_mates.push((m.id, e));
                        continue;
                    }
                };
                (sm.a, sm.a_local, sm.b, sm.b_local) = (a.0, a.1, b.0, b.1);
                if m.mate_type == MateType::Width {
                    if m.extra.is_empty() {
                        prepared.failed_mates.push((m.id, "Width mate needs two width mate connectors".into()));
                        continue;
                    }
                    let mut bad = None;
                    for c in &m.extra {
                        match resolve(c) {
                            Ok(x) => sm.extra.push(x),
                            Err(e) => bad = Some(e),
                        }
                    }
                    if let Some(e) = bad {
                        prepared.failed_mates.push((m.id, e));
                        continue;
                    }
                }
            }
            if sm.a == sm.b && m.mate_type != MateType::Width {
                prepared.failed_mates.push((m.id, "Both mate connectors are on the same rigid set".into()));
                continue;
            }
            fill_solver_mate(m, &mut sm, &units);
            prepared.mates.push((m.id, m.mate_type, sm));
        }

        // ---------------------------------------------------- flexible subassembly mates
        // Solved together with this assembly's (ids outside the assembly's range).
        let mut synth = 1u64 << 62;
        for (sid, sub, _) in &flex_subs {
            let sbody = prepared.body_of.get(sid).copied();
            let entity = |c: &MateConnectorRef| -> Result<(usize, Frame), String> {
                if c.instance == InstanceId::ORIGIN {
                    // The subassembly's origin is its instance frame.
                    return sbody.ok_or_else(|| "Subassembly is suppressed".to_string());
                }
                let mut full = vec![*sid, c.instance];
                full.extend(c.path.iter().copied());
                let leaf = leaves.iter().find(|l| l.path == full).ok_or("Instance no longer exists")?;
                let rest = leaf.flex.map(|f| f.1).unwrap_or(DMat4::IDENTITY);
                let f = resolve_connector_aligned(&leaf.regen.context, leaf.body.as_ref(), &c.reference, c.align.as_deref(), &units).ok_or("Mate connector could not be resolved (missing geometry)")?;
                let (b, off) = *prepared.nested_body_of.get(&(*sid, c.instance)).ok_or("Instance no longer exists")?;
                Ok((b, off.mul(&matrix_frame(&rest)).mul(&f)))
            };
            let sub_resolve = |c: &MateConnectorRef| -> Result<(usize, Frame), String> {
                let (b, mut f) = match c.connector {
                    Some(id) => {
                        let mc = sub.connectors.iter().find(|x| x.id == id && !x.suppressed).ok_or("Mate connector was deleted or is suppressed")?;
                        let (b, mut f) = entity(&mc.on)?;
                        if mc.on.flip {
                            f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
                        }
                        (b, f.mul(&asm_connector_offset(mc, &units)))
                    }
                    None => entity(c)?,
                };
                if c.flip {
                    f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
                }
                if !c.rotate_quarters.is_multiple_of(4) {
                    f = f.mul(&Frame::new(DVec3::ZERO, quarter_rot(c.rotate_quarters)));
                }
                Ok((b, f))
            };
            for m in sub.mates.iter().filter(|m| !m.suppressed && m.mate_type != MateType::Tangent) {
                if m.connectors().any(|c| c.instance != InstanceId::ORIGIN && sub.instance(c.instance).is_none_or(|i| i.suppressed)) {
                    continue;
                }
                let (Ok(a), Ok(b)) = (sub_resolve(&m.a), sub_resolve(&m.b)) else { continue };
                let mut sm = SolverMate { a: a.0, a_local: a.1, b: b.0, b_local: b.1, ..Default::default() };
                if m.mate_type == MateType::Width {
                    let extra: Result<Vec<_>, _> = m.extra.iter().map(sub_resolve).collect();
                    match extra {
                        Ok(e) if !e.is_empty() => sm.extra = e,
                        _ => continue,
                    }
                }
                if sm.a == sm.b && m.mate_type != MateType::Width {
                    continue;
                }
                fill_solver_mate(m, &mut sm, &units);
                prepared.mates.push((MateId(synth), m.mate_type, sm));
                synth += 1;
            }
        }

        // ---------------------------------------------------- patterns (followers)
        for p in asm.patterns.iter().filter(|p| !p.suppressed) {
            let follow_of = |axis_ref: &MateConnectorRef| -> Result<(usize, Frame), String> { resolve(axis_ref).map_err(|e| format!("{}: {e}", p.name)) };
            let world_of = |b: usize, f: &Frame| -> Frame {
                let rep = prepared.reps[b];
                stored.get(&rep).copied().unwrap_or_default().mul(f)
            };
            let make: Box<FollowMaker> = match &p.kind {
                PatternKind::Linear { direction, distance, count, opposite, equal_spacing, second } => {
                    let (ab, al) = match follow_of(direction) {
                        Ok(x) => x,
                        Err(e) => {
                            prepared.failed_patterns.push((p.id, e));
                            continue;
                        }
                    };
                    let mut d1 = eval_len(&Some(distance.clone()), &units);
                    if *equal_spacing && *count > 1 {
                        d1 /= (*count - 1) as f64;
                    }
                    if *opposite {
                        d1 = -d1;
                    }
                    let a_world = world_of(ab, &al);
                    // Second direction expressed in the first direction's frame.
                    let v2 = match second {
                        Some(s) => match follow_of(&s.direction) {
                            Ok((b2, l2)) => {
                                let z2 = world_of(b2, &l2).z();
                                let d2 = eval_len(&Some(s.distance.clone()), &units) * if s.opposite { -1.0 } else { 1.0 };
                                a_world.rotation.inverse() * z2 * d2
                            }
                            Err(e) => {
                                prepared.failed_patterns.push((p.id, e));
                                continue;
                            }
                        },
                        None => DVec3::ZERO,
                    };
                    Box::new(move |index, _, _| {
                        let (i, j) = ((index & 0xffff) as f64, (index >> 16) as f64);
                        Some((ab, al, FollowXform::Rigid(Frame::new(DVec3::Z * d1 * i + v2 * j, DQuat::IDENTITY))))
                    })
                }
                PatternKind::Circular { axis, angle, count, opposite, equal_spacing } => {
                    let (ab, al) = match follow_of(axis) {
                        Ok(x) => x,
                        Err(e) => {
                            prepared.failed_patterns.push((p.id, e));
                            continue;
                        }
                    };
                    let mut step = eval_angle(&Some(angle.clone()), &units);
                    if *equal_spacing && *count > 1 {
                        let full = (step.abs() - std::f64::consts::TAU).abs() < 1e-9;
                        step /= if full { *count as f64 } else { (*count - 1) as f64 };
                    }
                    if *opposite {
                        step = -step;
                    }
                    Box::new(move |index, _, _| Some((ab, al, FollowXform::Rigid(Frame::new(DVec3::ZERO, DQuat::from_rotation_z(step * index as f64))))))
                }
                PatternKind::Mirror { plane, .. } => {
                    let (ab, al) = match follow_of(plane) {
                        Ok(x) => x,
                        Err(e) => {
                            prepared.failed_patterns.push((p.id, e));
                            continue;
                        }
                    };
                    let n_world = world_of(ab, &al).z();
                    let leaves_ref = &leaves;
                    let stored_ref = &stored;
                    let reflection_of = |id: InstanceId| match asm.instance(id).map(|i| &i.source) {
                        Some(InstanceSource::MirroredPart { point, normal, .. }) => Some((DVec3::from(*point), DVec3::from(*normal))),
                        _ => None,
                    };
                    Box::new(move |_, seed, copy| {
                        // Opposite-handed copy (or seed): the part's own mirror
                        // plane completes the reflection exactly.
                        match (reflection_of(seed), reflection_of(copy)) {
                            (Some((p, n)), None) | (None, Some((p, n))) => return Some((ab, al, FollowXform::Mirror { local_point: p, local_normal: n })),
                            _ => {}
                        }
                        // Seed's local symmetry plane: the bounds mid-plane whose normal
                        // is closest to the mirror normal (in the seed's coordinates).
                        let pose = stored_ref.get(&seed).copied().unwrap_or_default();
                        let n_local = pose.rotation.inverse() * n_world;
                        let mut bb: Option<(DVec3, DVec3)> = None;
                        for l in leaves_ref.iter().filter(|l| l.path[0] == seed) {
                            for b in l.regen.display.bodies.iter().filter(|b| l.body.as_ref().is_none_or(|id| &b.id == id)) {
                                let Some((lo, hi)) = b.bounds else { continue };
                                for k in 0..8 {
                                    let c = DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z });
                                    let w = l.inner.transform_point3(c);
                                    bb = Some(bb.map(|(a, b)| (a.min(w), b.max(w))).unwrap_or((w, w)));
                                }
                            }
                        }
                        let center = bb.map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO);
                        let a = n_local.abs();
                        let axis = if a.x >= a.y && a.x >= a.z {
                            DVec3::X
                        } else if a.y >= a.z {
                            DVec3::Y
                        } else {
                            DVec3::Z
                        };
                        Some((ab, al, FollowXform::Mirror { local_point: center, local_normal: axis }))
                    })
                }
                // Matched patterns are driven by their generated mates.
                PatternKind::PartPattern { .. } | PatternKind::Replicate { .. } => continue,
            };
            for g in &p.generated {
                let (Some(&(sb, soff)), Some(&(gb, goff))) = (prepared.body_of.get(&g.seed), prepared.body_of.get(&g.instance)) else { continue };
                if sb == gb {
                    continue;
                }
                let Some((axis_body, axis_local, xform)) = make(g.index, g.seed, g.instance) else { continue };
                let sm = SolverMate { a: sb, a_local: soff, b: gb, b_local: goff, follow: Some(Follow { axis_body, axis_local, xform }), ..Default::default() };
                prepared.mates.push((MateId(u64::MAX - prepared.mates.len() as u64), MateType::Fastened, sm));
            }
        }

        // ---------------------------------------------------- relations
        for r in asm.relations.iter().filter(|r| !r.suppressed) {
            match self.prepare_relation(asm, r, &prepared, &units) {
                Ok(pr) => prepared.relations.push(pr),
                Err(e) => prepared.failed_relations.push((r.id, e)),
            }
        }
        (leaves, nodes, prepared)
    }

    fn prepare_relation(&self, asm: &Assembly, r: &Relation, prepared: &Prepared, units: &UnitPrefs) -> Result<PreparedRelation, String> {
        let b_id = if r.relation_type == RelationType::Screw { r.mates[0] } else { r.mates[1] };
        let ma = asm.mate(r.mates[0]).ok_or("Relation mate does not exist")?;
        let mb = asm.mate(b_id).ok_or("Relation mate does not exist")?;
        if ma.suppressed || mb.suppressed {
            return Err("Relation mate is suppressed".into());
        }
        if r.relation_type != RelationType::Screw && ma.id == mb.id {
            return Err("A relation needs two different mates".into());
        }
        let values = relation_values(r.relation_type, ma.mate_type, mb.mate_type)?;
        if !prepared.mates.iter().any(|(id, _, _)| *id == ma.id) || !prepared.mates.iter().any(|(id, _, _)| *id == mb.id) {
            return Err("Relation mate could not be resolved".into());
        }
        let dim = match r.relation_type {
            RelationType::Gear | RelationType::Linear => Dim::UNITLESS,
            RelationType::RackAndPinion | RelationType::Screw => Dim::LENGTH,
        };
        let mut ratio = noshape_units::evaluate_as(&r.ratio.text, dim, units, &EmptyScope)
            .or_else(|_| noshape_units::evaluate_as(&r.ratio.text, Dim::UNITLESS, units, &EmptyScope))
            .map(|q| q.value)
            .unwrap_or(1.0);
        // Screw / rack and pinion values are distances per revolution.
        if matches!(r.relation_type, RelationType::Screw | RelationType::RackAndPinion) {
            ratio /= std::f64::consts::TAU;
        }
        Ok(PreparedRelation { id: r.id, kind: relation_kind(r.relation_type), a: ma.id, b: mb.id, ratio, reverse: r.reverse, values, base: r.base })
    }

    /// Contact geometry for a tangent mate: (body a, body b, geometry in body coordinates).
    fn tangent_pair(&self, _asm: &Assembly, leaves: &[LeafInstance], prepared: &Prepared, stored: &HashMap<InstanceId, Frame>, m: &super::Mate, units: &UnitPrefs) -> Result<(usize, usize, TangentPair), String> {
        let geom = |c: &MateConnectorRef| -> Result<(usize, TangentGeom, Vec<TangentFace>), String> {
            if c.connector.is_some() || c.instance == InstanceId::ORIGIN {
                return Err("Tangent mates take faces, edges or vertices (not mate connectors)".into());
            }
            let mut full = vec![c.instance];
            full.extend(c.path.iter().copied());
            let leaf = leaves.iter().find(|l| l.path == full).ok_or("Instance no longer exists")?;
            let entity = match &c.reference {
                Reference::Implicit { on, .. } => (**on).clone(),
                r => r.clone(),
            };
            let g = tangent_geometry(&leaf.regen.context, leaf.body.as_ref(), &entity, units).ok_or("Tangent mate geometry not supported (use planes, cylinders, cones, spheres, tori, straight or circular edges, or vertices)")?;
            let (bi, to_body) = match leaf.flex {
                Some((child, rest)) => {
                    let (bi, off) = *prepared.nested_body_of.get(&(c.instance, child)).ok_or("Instance no longer exists")?;
                    (bi, off.mul(&matrix_frame(&rest)))
                }
                None => {
                    let (bi, off) = *prepared.body_of.get(&c.instance).ok_or("Instance no longer exists")?;
                    (bi, off.mul(&matrix_frame(&leaf.inner)))
                }
            };
            let chain = if m.tangent_propagation { tangent_chain(&leaf.regen.context, leaf.body.as_ref(), &entity, units) } else { Vec::new() };
            // A single face needs no propagation.
            let chain: Vec<TangentFace> = if chain.len() > 1 { chain.iter().map(|f| face_transformed(f, &to_body)).collect() } else { Vec::new() };
            Ok((bi, g.transformed(&to_body), chain))
        };
        let (a, ga, chain_a) = geom(&m.a)?;
        let (b, gb, chain_b) = geom(&m.b)?;
        if a == b {
            return Err("Both entities are on the same rigid set".into());
        }
        if !solver::tangent_supported(&ga, &gb) {
            return Err("Tangent mates between these two surface types are not supported (cones and tori mate with planes, spheres and vertices)".into());
        }
        // Internal contact when one curved surface currently encloses the other.
        let (wga, wgb) = (ga.transformed(&prepared_pose(stored, prepared, a)), gb.transformed(&prepared_pose(stored, prepared, b)));
        let internal = match (wga, wgb) {
            (TangentGeom::Cylinder { point: c1, axis, radius: r1 }, TangentGeom::Cylinder { point: c2, radius: r2, .. })
            | (TangentGeom::Cylinder { point: c1, axis, radius: r1 }, TangentGeom::Sphere { center: c2, radius: r2 })
            | (TangentGeom::Sphere { center: c2, radius: r2 }, TangentGeom::Cylinder { point: c1, axis, radius: r1 }) => {
                let d = c2 - c1;
                (d - axis * d.dot(axis)).length() < r1.max(r2)
            }
            (TangentGeom::Sphere { center: c1, radius: r1 }, TangentGeom::Sphere { center: c2, radius: r2 }) => (c2 - c1).length() < r1.max(r2),
            (g @ (TangentGeom::Cone { .. } | TangentGeom::Torus { .. }), TangentGeom::Sphere { center, .. }) | (TangentGeom::Sphere { center, .. }, g @ (TangentGeom::Cone { .. } | TangentGeom::Torus { .. })) => g.signed_distance(center) < 0.0,
            _ => false,
        };
        Ok((a, b, TangentPair { a: ga, b: gb, internal, chain_a, chain_b }))
    }
}

/// Pattern index + seed + copy -> (axis body, axis frame, follower transform).
type FollowMaker<'a> = dyn Fn(u32, InstanceId, InstanceId) -> Option<(usize, Frame, FollowXform)> + 'a;

fn prepared_pose(stored: &HashMap<InstanceId, Frame>, prepared: &Prepared, body: usize) -> Frame {
    let rep = stored.get(&prepared.reps[body]).copied().unwrap_or_default();
    match prepared.nested_bodies.iter().find(|(b, ..)| *b == body) {
        Some((_, _, _, q)) => rep.mul(q),
        None => rep,
    }
}

fn studio_eval_ctx<'a>(ctx: &Arc<Context>, body: Option<&BodyId>, units: &'a UnitPrefs, params: &'a ParamValues, vars: &'a BTreeMap<String, noshape_units::Quantity>) -> EvalCtx<'a> {
    let mut c: Context = (**ctx).clone();
    if let Some(b) = body {
        c.bodies.retain(|x| &x.id == b);
    }
    EvalCtx::new(FeatureId(0), "", params, units, c, vars)
}

/// Analytic tangent geometry of a face shape (outward normal for planes).
fn face_tangent_geometry(face: &noshape_kernel::Shape) -> Option<TangentGeom> {
    match face.face_surface().ok()? {
        noshape_kernel::Surface::Plane(_) => {
            let p = face.centroid().ok()?;
            let n = face.normal_at_point(p).ok()?;
            Some(TangentGeom::Plane { point: p, normal: n.normalize() })
        }
        noshape_kernel::Surface::Cylinder { origin, axis, radius, .. } => {
            // Axis point next to the face (keeps contact estimates local).
            let a = axis.normalize();
            let c = face.centroid().unwrap_or(origin);
            Some(TangentGeom::Cylinder { point: origin + a * (c - origin).dot(a), axis: a, radius })
        }
        noshape_kernel::Surface::Sphere { center, radius, .. } => Some(TangentGeom::Sphere { center, radius }),
        noshape_kernel::Surface::Cone { axis, half_angle, apex, .. } => {
            // Axis from the apex into the cone (toward the face).
            let mut a = axis.normalize();
            if let Ok(c) = face.centroid()
                && (c - apex).dot(a) < 0.0
            {
                a = -a;
            }
            Some(TangentGeom::Cone { apex, axis: a, half_angle: half_angle.abs() })
        }
        noshape_kernel::Surface::Torus { center, axis, major_radius, minor_radius } => Some(TangentGeom::Torus { center, axis: axis.normalize(), major: major_radius, minor: minor_radius }),
        _ => None,
    }
}

/// Analytic tangent-mate geometry of a face / edge / vertex (studio coordinates).
pub fn tangent_geometry(ctx: &Arc<Context>, body: Option<&BodyId>, entity: &Reference, units: &UnitPrefs) -> Option<TangentGeom> {
    let params = ParamValues::default();
    let vars = BTreeMap::new();
    let ev = studio_eval_ctx(ctx, body, units, &params, &vars);
    match entity {
        Reference::Face(_) => {
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(entity)).first()?;
            face_tangent_geometry(&ev.face_shape(bi, fi).ok()?)
        }
        Reference::Edge(_) => {
            let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(entity)).first()?;
            let e = ev.edge_shape(bi, ei).ok()?;
            match e.edge_curve().ok()? {
                noshape_kernel::Curve::Line { origin, direction } => Some(TangentGeom::Cylinder { point: origin, axis: direction.normalize(), radius: 0.0 }),
                noshape_kernel::Curve::Circle { center, axis, radius, .. } => Some(TangentGeom::Torus { center, axis: axis.normalize(), major: radius, minor: 0.0 }),
                _ => None,
            }
        }
        Reference::Vertex(_) => ev.resolve_vertex_point(entity).map(|p| TangentGeom::Sphere { center: p, radius: 0.0 }),
        Reference::Origin => Some(TangentGeom::Sphere { center: DVec3::ZERO, radius: 0.0 }),
        _ => None,
    }
}

fn face_transformed(f: &TangentFace, t: &Frame) -> TangentFace {
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for i in 0..8 {
        let c = DVec3::new(if i & 1 == 0 { f.lo.x } else { f.hi.x }, if i & 2 == 0 { f.lo.y } else { f.hi.y }, if i & 4 == 0 { f.lo.z } else { f.hi.z });
        let w = t.transform_point(c);
        lo = lo.min(w);
        hi = hi.max(w);
    }
    TangentFace { geom: f.geom.transformed(t), lo, hi, centroid: t.transform_point(f.centroid), concave: f.concave }
}

/// Faces of a body tangent-connected to a face (the face first), as tangent
/// geometry with bounds (studio coordinates). Empty for edges / vertices or
/// unsupported faces. At most 32 faces.
pub fn tangent_chain(ctx: &Arc<Context>, body: Option<&BodyId>, entity: &Reference, units: &UnitPrefs) -> Vec<TangentFace> {
    let Reference::Face(_) = entity else { return Vec::new() };
    let params = ParamValues::default();
    let vars = BTreeMap::new();
    let ev = studio_eval_ctx(ctx, body, units, &params, &vars);
    let Some(&(bi, fi)) = ev.resolve_faces(std::slice::from_ref(entity)).first() else { return Vec::new() };
    let Some(b) = ev.ctx.bodies.get(bi) else { return Vec::new() };
    let shape = &b.shape;
    let faces = shape.faces();
    let edges = shape.edges();
    let Ok(edge_faces) = shape.edge_faces() else { return Vec::new() };
    let info = |i: usize| -> Option<TangentFace> {
        let f = faces.get(i)?;
        let geom = face_tangent_geometry(f)?;
        let bb = f.bounding_box(true).ok()?;
        let centroid = f.centroid().ok()?;
        let concave = match geom {
            TangentGeom::Plane { .. } => false,
            g => {
                // Outward normal pointing into the surface's inside: a concave face.
                let n = f.normal_at_point(centroid).unwrap_or(DVec3::ZERO);
                let q = g.closest_point(centroid);
                let probe = q + n * 1e-6;
                g.signed_distance(probe) < g.signed_distance(q) - 1e-9
            }
        };
        Some(TangentFace { geom, lo: bb.min, hi: bb.max, centroid, concave })
    };
    let Some(first) = info(fi) else { return Vec::new() };
    let mut out = vec![first];
    let mut done = vec![fi];
    let mut queue = vec![fi];
    while let Some(f) = queue.pop() {
        for (ei, adj) in edge_faces.iter().enumerate() {
            if adj.len() != 2 || !adj.contains(&(f as u32)) {
                continue;
            }
            let g = if adj[0] as usize == f { adj[1] as usize } else { adj[0] as usize };
            if done.contains(&g) || out.len() >= 32 {
                continue;
            }
            let Some(e) = edges.get(ei) else { continue };
            let Ok(ei_info) = e.edge_info() else { continue };
            let mid = e.point_at((ei_info.first_param + ei_info.last_param) * 0.5).unwrap_or((ei_info.start + ei_info.end) * 0.5);
            let (Some(ff), Some(gf)) = (faces.get(f), faces.get(g)) else { continue };
            let (Ok(n1), Ok(n2)) = (ff.normal_at_point(mid), gf.normal_at_point(mid)) else { continue };
            if n1.normalize_or_zero().dot(n2.normalize_or_zero()) < 0.9995 {
                continue;
            }
            done.push(g);
            if let Some(t) = info(g) {
                out.push(t);
                queue.push(g);
            }
        }
    }
    out
}

fn prefix_paths(n: &mut InstanceNode, id: InstanceId) {
    n.path.insert(0, id);
    for c in &mut n.children {
        prefix_paths(c, id);
    }
}
