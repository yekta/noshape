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

use super::solver::{self, DragTarget, Frame, RelationKind, SolverBody, SolverMate, SolverRelation};
use super::{Assembly, ExplodedView, InstanceId, InstanceSource, MateConnectorRef, MateId, MateType, Placement, RelationType};
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
    relations: Vec<(MateId, RelationKind, MateId, MateId, f64, bool)>,
    /// Mates that could not be resolved, with messages.
    pub failed_mates: Vec<(MateId, String)>,
    pub inactive_mates: HashSet<MateId>,
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
        let stored: HashMap<InstanceId, Placement> = self.instances.iter().cloned().collect();
        let mut bodies: Vec<SolverBody> = self
            .reps
            .iter()
            .enumerate()
            .map(|(i, id)| SolverBody { pose: pose_of(*id, &stored[id]), fixed: self.fixed[i] || opts.hold.iter().any(|h| self.body_of.get(h).is_some_and(|(b, _)| *b == i)) })
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
        for (id, kind, a, b, ratio, reverse) in &self.relations {
            let (Some(&ia), Some(&ib)) = (mate_index.get(a), mate_index.get(b)) else { continue };
            let va = solver::mate_values(&poses0, &mates[ia]);
            let vb = solver::mate_values(&poses0, &mates[ib]);
            let (base_a, base_b) = match kind {
                RelationKind::Gear => (va[0], vb[0]),
                RelationKind::Linear => (va[3], vb[3]),
                RelationKind::RackAndPinion => (va[0], vb[3]),
                RelationKind::Screw => (vb[0], vb[3]),
            };
            rels.push(SolverRelation { kind: *kind, mate_a: ia, mate_b: ib, ratio: *ratio, reverse: *reverse, base_a, base_b });
            rel_ids.push(*id);
        }
        // Unconstrained bodies follow a drag by pure translation (no solve).
        let constrained = |b: usize| mates.iter().any(|m| m.a == b || m.b == b);
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
    pub prepared: Arc<Prepared>,
    pub eval_ms: f32,
}

impl AssemblyResult {
    /// Recompute occurrence world transforms from solved placements and an
    /// optional exploded view (display only).
    pub fn apply_placements(&mut self, solved: Solved, explode: Option<&ExplodedView>) {
        self.solved = solved;
        let place: HashMap<InstanceId, DMat4> = self.solved.placements.iter().map(|(id, p)| (*id, frame_matrix(&placement_to_frame(p)))).collect();
        for o in &mut self.occurrences {
            let top = place.get(&o.top()).copied().unwrap_or(DMat4::IDENTITY);
            let mut t = top * o.inner;
            if let Some(ev) = explode {
                for step in &ev.steps {
                    if step.instances.contains(&o.path[0]) {
                        let mut m = DMat4::from_translation(step.translation.into());
                        if let Some(axis) = step.rotation_axis {
                            let c = (top * o.inner).transform_point3(o.local_bounds.map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO));
                            m = m * DMat4::from_translation(c) * DMat4::from_axis_angle(DVec3::from(axis).normalize_or_zero(), step.rotation_angle) * DMat4::from_translation(-c);
                        }
                        t = m * t;
                    }
                }
            }
            o.transform = t;
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
        let units = doc.settings.units;
        let _ = units;
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
                let ok = r.suppressed || solved.relation_ok.get(&r.id).copied().unwrap_or(false);
                RelationStatus { id: r.id, ok, message: (!ok).then(|| "Relation could not be solved (both mates must be valid and of a compatible type)".into()) }
            })
            .collect();
        let mut nodes = nodes;
        for n in &mut nodes {
            n.dof = Some(solved.instance_dof.get(&n.path[0]).copied().unwrap_or(0));
        }
        let mut result = AssemblyResult { occurrences, nodes, solved: Solved::default(), mates, relations, prepared: Arc::new(prepared), eval_ms: 0.0 };
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
        for inst in &asm.instances {
            let mut node = InstanceNode {
                path: vec![inst.id],
                name: inst.name.clone(),
                kind: NodeKind::Part,
                fixed: inst.fixed,
                hidden: inst.hidden,
                suppressed: inst.suppressed,
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
                                node.error = Some("Part no longer exists in its Part Studio".into());
                            } else if !inst.suppressed {
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
                            if !inst.suppressed {
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
                                if !inst.suppressed {
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
        let active: Vec<&super::Instance> = asm.instances.iter().filter(|i| !i.suppressed).collect();
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
        prepared.body_of = assigned;

        // ---------------------------------------------------- mates
        for m in &asm.mates {
            if m.suppressed {
                prepared.inactive_mates.insert(m.id);
                continue;
            }
            let susp = |c: &MateConnectorRef| asm.instances.iter().find(|i| i.id == c.instance).is_some_and(|i| i.suppressed);
            if susp(&m.a) || susp(&m.b) {
                prepared.inactive_mates.insert(m.id);
                continue;
            }
            let resolve = |c: &MateConnectorRef| -> Result<(usize, Frame), String> {
                let (bi, off) = *prepared.body_of.get(&c.instance).ok_or("Instance no longer exists")?;
                let mut full = vec![c.instance];
                full.extend(c.path.iter().copied());
                let leaf = leaves.iter().find(|l| l.path == full).ok_or("Instance no longer exists")?;
                let f = resolve_connector_in(&leaf.regen.context, leaf.body.as_ref(), &c.reference, &units).ok_or("Mate connector could not be resolved (missing geometry)")?;
                let mut f = matrix_frame(&leaf.inner).mul(&f);
                if c.flip {
                    f = f.mul(&Frame::new(DVec3::ZERO, DQuat::from_rotation_x(std::f64::consts::PI)));
                }
                if !c.rotate_quarters.is_multiple_of(4) {
                    f = f.mul(&Frame::new(DVec3::ZERO, quarter_rot(c.rotate_quarters)));
                }
                Ok((bi, off.mul(&f)))
            };
            let (a, b) = match (resolve(&m.a), resolve(&m.b)) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => {
                    prepared.failed_mates.push((m.id, e));
                    continue;
                }
            };
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
            let offset = DVec3::new(
                if ox { eval_len(&m.offset_x, &units) } else { 0.0 },
                if oy { eval_len(&m.offset_y, &units) } else { 0.0 },
                if oz { eval_len(&m.offset_z, &units) } else { 0.0 },
            );
            let rotation = if orot { eval_angle(&m.rotation, &units) } else { 0.0 };
            prepared.mates.push((
                m.id,
                m.mate_type,
                SolverMate {
                    kind: m.mate_type,
                    a: a.0,
                    a_local: a.1,
                    b: b.0,
                    b_local: b.1,
                    flip_primary: m.flip_primary,
                    reorient: m.reorient_secondary,
                    offset,
                    rotation,
                    limits,
                },
            ));
        }
        for r in asm.relations.iter().filter(|r| !r.suppressed) {
            let kind = match r.relation_type {
                RelationType::Gear => RelationKind::Gear,
                RelationType::RackAndPinion => RelationKind::RackAndPinion,
                RelationType::Screw => RelationKind::Screw,
                RelationType::Linear => RelationKind::Linear,
            };
            let dim = match r.relation_type {
                RelationType::Gear | RelationType::Linear => Dim::UNITLESS,
                RelationType::RackAndPinion | RelationType::Screw => Dim::LENGTH,
            };
            let ratio = noshape_units::evaluate_as(&r.ratio.text, dim, &units, &EmptyScope)
                .or_else(|_| noshape_units::evaluate_as(&r.ratio.text, Dim::UNITLESS, &units, &EmptyScope))
                .map(|q| q.value)
                .unwrap_or(1.0);
            prepared.relations.push((r.id, kind, r.mates[0], r.mates[1], ratio, r.reverse));
        }
        (leaves, nodes, prepared)
    }
}

fn prefix_paths(n: &mut InstanceNode, id: InstanceId) {
    n.path.insert(0, id);
    for c in &mut n.children {
        prefix_paths(c, id);
    }
}
