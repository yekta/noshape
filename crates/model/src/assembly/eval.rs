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

use super::solver::{self, DragTarget, Follow, FollowXform, Frame, RelationKind, SolverBody, SolverMate, SolverRelation, TangentGeom, TangentPair};
use super::{AsmMateConnector, Assembly, ExplodedView, InstanceId, InstanceSource, MateConnectorRef, MateId, MateType, PatternKind, Placement, Relation, RelationType};
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
}

impl Solved {
    pub fn placement(&self, id: InstanceId) -> Option<Placement> {
        self.placements.iter().find(|(i, _)| *i == id).map(|(_, p)| *p)
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
            o.transform = top * o.inner;
        }
        if let Some((ev, progress)) = explode {
            let steps = explode_matrices(self, ev, progress);
            for o in &mut self.occurrences {
                if let Some(m) = steps.get(&o.top()) {
                    o.transform = *m * o.transform;
                }
            }
        }
        let inst: HashMap<InstanceId, Frame> = self.solved.placements.iter().map(|(id, p)| (*id, placement_to_frame(p))).collect();
        for c in &mut self.connectors {
            c.frame = c.local.map(|l| if c.owner == InstanceId::ORIGIN { l } else { inst.get(&c.owner).copied().unwrap_or_default().mul(&l) });
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
            let local = resolve_connector_in(&o.context, Some(&o.body), &c.reference, units)?;
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

/// Per-instance display matrices of an exploded view played to `progress`.
/// Steps compose in order; rotation pivots default to the current bounds
/// center of the step's instances.
fn explode_matrices(result: &AssemblyResult, ev: &ExplodedView, progress: f64) -> HashMap<InstanceId, DMat4> {
    let mut acc: HashMap<InstanceId, DMat4> = HashMap::new();
    for (k, step) in ev.steps.iter().enumerate() {
        let t = (progress - k as f64).clamp(0.0, 1.0);
        if t <= 0.0 {
            break;
        }
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
    }
    acc
}

struct StudioCacheEntry {
    hash: u64,
    regen: Regenerator,
    result: Arc<RegenResult>,
}

/// Evaluates assemblies; keeps Part Studio regenerations cached across calls.
#[derive(Default)]
pub struct AssemblyEvaluator {
    studios: HashMap<(TabId, String), StudioCacheEntry>,
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
    let mut c: Context = (**ctx).clone();
    if let Some(b) = body {
        c.bodies.retain(|x| &x.id == b);
    }
    let params = ParamValues::default();
    let vars = BTreeMap::new();
    let ev = EvalCtx::new(FeatureId(0), "", &params, units, c, &vars);
    let conn = match reference {
        Reference::MateConnector { .. } | Reference::Implicit { .. } => ev.resolve_connector(reference)?,
        // A bare entity: infer its default connector.
        other => crate::regen::implicit::implicit_connector(&ev, other, default_inference(other))?,
    };
    Some(Frame::from_axes(conn.origin, conn.z, conn.x))
}

fn default_inference(r: &Reference) -> Inference {
    match r {
        Reference::Vertex(_) => Inference::Vertex,
        Reference::Edge(_) => Inference::Midpoint,
        Reference::Origin => Inference::Origin,
        _ => Inference::Centroid,
    }
}

/// Implicit mate connector candidates on an entity (studio coordinates):
/// (implicit reference, frame). Used for hover inference.
pub fn inference_candidates(ctx: &Arc<Context>, body: Option<&BodyId>, on: &Reference, units: &UnitPrefs) -> Vec<(Reference, Frame)> {
    let infs: &[Inference] = match on {
        Reference::Face(_) => &[Inference::Centroid, Inference::TopAxis, Inference::MidAxis, Inference::BottomAxis],
        Reference::Edge(_) => &[Inference::Center, Inference::Midpoint, Inference::Vertex],
        Reference::Vertex(_) => &[Inference::Vertex],
        Reference::Origin => &[Inference::Origin],
        Reference::MateConnector { .. } => {
            return resolve_connector_in(ctx, body, on, units).map(|f| vec![(on.clone(), f)]).unwrap_or_default();
        }
        _ => return Vec::new(),
    };
    let mut out: Vec<(Reference, Frame)> = Vec::new();
    let mut refs: Vec<Reference> = infs.iter().map(|inf| Reference::Implicit { on: Box::new(on.clone()), inference: *inf }).collect();
    // Planar faces also offer the centers of their circular edges (holes, bosses).
    if let Reference::Face(n) = on {
        for b in ctx.bodies.iter().filter(|b| body.is_none_or(|id| &b.id == id)) {
            if b.find_faces(n).is_empty() {
                continue;
            }
            for (ei, e) in b.edges.iter().enumerate() {
                let crate::reference::TopoName::Edge { faces, .. } = &**e else { continue };
                if !faces.iter().any(|f| f.descends_from(n)) {
                    continue;
                }
                let is_circle = b.shape.sub_shape(noshape_kernel::ShapeType::Edge, ei).ok().and_then(|s| s.edge_info().ok()).is_some_and(|i| matches!(i.curve, noshape_kernel::Curve::Circle { .. }));
                if is_circle {
                    refs.push(Reference::Implicit { on: Box::new(Reference::Edge((**e).clone())), inference: Inference::Center });
                }
            }
        }
    }
    for r in refs {
        if let Some(f) = resolve_connector_in(ctx, body, &r, units)
            && !out.iter().any(|(_, g)| g.origin.distance(f.origin) < 1e-9 && g.z().dot(f.z()) > 0.9999)
        {
            out.push((r, f));
        }
    }
    out
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

    /// Drop cache entries for tabs that no longer exist.
    pub fn retain_tabs(&mut self, doc: &Document) {
        self.studios.retain(|(t, _), _| doc.tab(*t).is_some());
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
                ConnectorStatus { id: c.id, owner: resolved.map(|r| r.1).unwrap_or(c.on.instance), local: resolved.map(|r| r.2), frame: None, message }
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
        let mut result = AssemblyResult { occurrences, nodes, solved: Solved::default(), mates, relations, connectors, pattern_errors, prepared: Arc::new(prepared), eval_ms: 0.0 };
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
                                });
                            }
                        }
                        None => node.error = Some("Part Studio was deleted".into()),
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
                            let solved = sub_prep.solve(&SolveOptions::default());
                            let place: HashMap<InstanceId, DMat4> = solved.placements.iter().map(|(id, p)| (*id, frame_matrix(&placement_to_frame(p)))).collect();
                            for mut l in sub_leaves {
                                let top = place.get(&l.path[0]).copied().unwrap_or(DMat4::IDENTITY);
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
        // The assembly origin: a fixed body at the world frame.
        let ground = prepared.reps.len();
        prepared.reps.push(InstanceId::ORIGIN);
        prepared.fixed.push(true);
        assigned.insert(InstanceId::ORIGIN, (ground, Frame::default()));
        prepared.body_of = assigned;
        let stored: HashMap<InstanceId, Frame> = active.iter().map(|i| (i.id, placement_to_frame(&i.placement))).collect();

        // ---------------------------------------------------- assembly mate connectors
        let resolve_entity = |c: &MateConnectorRef| -> Result<(InstanceId, Frame), String> {
            if c.instance == InstanceId::ORIGIN {
                // The origin (or a plane / implicit connector on it).
                let f = match &c.reference {
                    Reference::Implicit { on, .. } if matches!(**on, Reference::Origin) => Frame::default(),
                    _ => Frame::default(),
                };
                return Ok((InstanceId::ORIGIN, f));
            }
            if inactive.contains(&c.instance) {
                return Err("Instance is suppressed".into());
            }
            let mut full = vec![c.instance];
            full.extend(c.path.iter().copied());
            let leaf = leaves.iter().find(|l| l.path == full).ok_or("Instance no longer exists")?;
            let f = resolve_connector_in(&leaf.regen.context, leaf.body.as_ref(), &c.reference, &units).ok_or("Mate connector could not be resolved (missing geometry)")?;
            Ok((c.instance, matrix_frame(&leaf.inner).mul(&f)))
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
                    prepared.connectors.push((mc.id, owner, f.mul(&asm_connector_offset(mc, &units))));
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
                    (*owner, *f)
                }
                None => resolve_entity(c)?,
            };
            if c.flip {
                f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
            }
            if !c.rotate_quarters.is_multiple_of(4) {
                f = f.mul(&Frame::new(DVec3::ZERO, quarter_rot(c.rotate_quarters)));
            }
            let (bi, off) = *prepared.body_of.get(&owner).ok_or("Instance no longer exists")?;
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
            let mut limits = Vec::new();
            for (key, l) in &m.limits {
                let Some(ix) = limit_index(key) else { continue };
                if !limit_keys(m.mate_type).contains(&key.as_str()) {
                    continue;
                }
                let dim = if ix == 0 { Dim::ANGLE } else { Dim::LENGTH };
                let lo = eval_opt(&l.min, dim, &units);
                let hi = eval_opt(&l.max, dim, &units);
                if lo.is_some() || hi.is_some() {
                    limits.push((ix, lo, hi));
                }
            }
            let (ox, oy, oz, orot) = offset_axes(m.mate_type);
            sm.offset = DVec3::new(
                if ox { eval_len(&m.offset_x, &units) } else { 0.0 },
                if oy { eval_len(&m.offset_y, &units) } else { 0.0 },
                if oz { eval_len(&m.offset_z, &units) } else { 0.0 },
            );
            sm.rotation = if orot { eval_angle(&m.rotation, &units) } else { 0.0 };
            sm.kind = m.mate_type;
            sm.flip_primary = m.flip_primary;
            sm.reorient = m.reorient_secondary;
            sm.limits = limits;
            prepared.mates.push((m.id, m.mate_type, sm));
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
                    Box::new(move |index, _| {
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
                    Box::new(move |index, _| Some((ab, al, FollowXform::Rigid(Frame::new(DVec3::ZERO, DQuat::from_rotation_z(step * index as f64))))))
                }
                PatternKind::Mirror { plane } => {
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
                    Box::new(move |_, seed| {
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
                let Some((axis_body, axis_local, xform)) = make(g.index, g.seed) else { continue };
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
        let geom = |c: &MateConnectorRef| -> Result<(usize, TangentGeom, Frame), String> {
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
            let g = tangent_geometry(&leaf.regen.context, leaf.body.as_ref(), &entity, units).ok_or("Tangent mate geometry not supported (use planes, cylinders, spheres, straight edges or vertices)")?;
            let (bi, off) = *prepared.body_of.get(&c.instance).ok_or("Instance no longer exists")?;
            let to_body = off.mul(&matrix_frame(&leaf.inner));
            let world = stored.get(&prepared.reps[bi]).copied().unwrap_or_default().mul(&to_body);
            Ok((bi, g.transformed(&to_body), world))
        };
        let (a, ga, wa) = geom(&m.a)?;
        let (b, gb, wb) = geom(&m.b)?;
        if a == b {
            return Err("Both entities are on the same rigid set".into());
        }
        // Internal contact when one curved surface currently encloses the other.
        let (wga, wgb) = (ga.transformed(&prepared_pose(stored, prepared, a)), gb.transformed(&prepared_pose(stored, prepared, b)));
        let _ = (wa, wb);
        let internal = match (wga, wgb) {
            (TangentGeom::Cylinder { point: c1, axis, radius: r1 }, TangentGeom::Cylinder { point: c2, radius: r2, .. })
            | (TangentGeom::Cylinder { point: c1, axis, radius: r1 }, TangentGeom::Sphere { center: c2, radius: r2 })
            | (TangentGeom::Sphere { center: c2, radius: r2 }, TangentGeom::Cylinder { point: c1, axis, radius: r1 }) => {
                let d = c2 - c1;
                (d - axis * d.dot(axis)).length() < r1.max(r2)
            }
            (TangentGeom::Sphere { center: c1, radius: r1 }, TangentGeom::Sphere { center: c2, radius: r2 }) => (c2 - c1).length() < r1.max(r2),
            _ => false,
        };
        Ok((a, b, TangentPair { a: ga, b: gb, internal }))
    }
}

/// Pattern index + seed -> (axis body, axis frame, follower transform).
type FollowMaker<'a> = dyn Fn(u32, InstanceId) -> Option<(usize, Frame, FollowXform)> + 'a;

fn prepared_pose(stored: &HashMap<InstanceId, Frame>, prepared: &Prepared, body: usize) -> Frame {
    stored.get(&prepared.reps[body]).copied().unwrap_or_default()
}

/// Analytic tangent-mate geometry of a face / edge / vertex (studio coordinates).
pub fn tangent_geometry(ctx: &Arc<Context>, body: Option<&BodyId>, entity: &Reference, units: &UnitPrefs) -> Option<TangentGeom> {
    let mut c: Context = (**ctx).clone();
    if let Some(b) = body {
        c.bodies.retain(|x| &x.id == b);
    }
    let params = ParamValues::default();
    let vars = BTreeMap::new();
    let ev = EvalCtx::new(FeatureId(0), "", &params, units, c, &vars);
    match entity {
        Reference::Face(_) => {
            let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(entity)).first()?;
            let face = ev.face_shape(bi, fi).ok()?;
            match face.face_surface().ok()? {
                noshape_kernel::Surface::Plane(_) => {
                    let p = face.centroid().ok()?;
                    let n = face.normal_at_point(p).ok()?;
                    Some(TangentGeom::Plane { point: p, normal: n.normalize() })
                }
                noshape_kernel::Surface::Cylinder { origin, axis, radius, .. } => Some(TangentGeom::Cylinder { point: origin, axis: axis.normalize(), radius }),
                noshape_kernel::Surface::Sphere { center, radius, .. } => Some(TangentGeom::Sphere { center, radius }),
                _ => None,
            }
        }
        Reference::Edge(_) => {
            let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(entity)).first()?;
            let e = ev.edge_shape(bi, ei).ok()?;
            match e.edge_curve().ok()? {
                noshape_kernel::Curve::Line { origin, direction } => Some(TangentGeom::Cylinder { point: origin, axis: direction.normalize(), radius: 0.0 }),
                _ => None,
            }
        }
        Reference::Vertex(_) => ev.resolve_vertex_point(entity).map(|p| TangentGeom::Sphere { center: p, radius: 0.0 }),
        Reference::Origin => Some(TangentGeom::Sphere { center: DVec3::ZERO, radius: 0.0 }),
        _ => None,
    }
}

fn prefix_paths(n: &mut InstanceNode, id: InstanceId) {
    n.path.insert(0, id);
    for c in &mut n.children {
        prefix_paths(c, id);
    }
}
