//! Assembly contexts (Onshape "Modeling in context", help
//! Assembly__modeling_in_context): a Part Studio edited in the context of
//! an assembly keeps a snapshot of the other instances' geometry, placed
//! relative to the *primary instance* (the part Edit in context started
//! from). Features may reference that geometry (faces, edges, vertices of
//! the snapshot bodies); the snapshot never changes on its own ("references
//! are never lost or broken") until Update context replaces it.
//!
//! Snapshot bodies take part in regeneration only for features that
//! reference them: the regenerator appends them to the feature's context
//! and drops them again afterwards, so they are never merged, patterned or
//! listed as parts. Their topology is named from a synthetic creator
//! feature (`context_feature`), faces `Role::New(i)` in kernel order.

use crate::assembly::eval::{AssemblyResult, InstancePath};
use crate::display::BodyKind;
use crate::document::TabId;
use crate::params::{ParamValue, ParamValues};
use crate::reference::{BodyId, FeatureId, Reference, Role, TopoName};
use crate::regen::context::Body;
use glam::DMat4;
use noshape_kernel as k;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Bit marking the synthetic creator features of context bodies.
const CONTEXT_BIT: u64 = 1 << 62;

/// Synthetic creator feature of body `body` of context `context`.
pub fn context_feature(context: u32, body: u32) -> FeatureId {
    FeatureId(CONTEXT_BIT | ((context as u64) << 24) | body as u64)
}

/// Whether `f` is a context body's synthetic creator.
pub fn is_context_feature(f: FeatureId) -> bool {
    f.0 & CONTEXT_BIT != 0
}

/// The context of a synthetic creator feature.
pub fn context_of(f: FeatureId) -> Option<u32> {
    is_context_feature(f).then_some(((f.0 & !CONTEXT_BIT) >> 24) as u32)
}

/// One instance's part in the snapshot, in the Part Studio's coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextBody {
    /// Instance path in the assembly (top-level first).
    pub path: Vec<u64>,
    /// Instance name ("Part 1 <2>") and part name.
    pub instance: String,
    pub part: String,
    /// Display color (linear RGBA).
    pub color: [f32; 4],
    /// The geometry (OCCT BREP text, already placed).
    pub brep: Arc<str>,
}

/// An Assembly context of a Part Studio (the "Assembly contexts" list
/// above its Feature list).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssemblyContext {
    pub id: u32,
    pub name: String,
    pub assembly: TabId,
    /// The primary instance (solid context arrow) and its part.
    pub primary: Vec<u64>,
    #[serde(default)]
    pub primary_body: Option<BodyId>,
    pub bodies: Vec<ContextBody>,
    /// Bumped by Update context (features referencing it regenerate).
    #[serde(default)]
    pub revision: u32,
    /// Context instances hidden in the Part Studio (Hide/show context instances).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<Vec<u64>>,
    /// The assembly's state when the snapshot was taken ([`state_hash`]);
    /// a different current state means "Update available".
    #[serde(default)]
    pub state: u64,
}

impl AssemblyContext {
    /// The snapshot as regeneration bodies, named from the synthetic
    /// creators. Bodies whose BREP does not read are skipped.
    pub fn regen_bodies(&self) -> Vec<Arc<Body>> {
        let mut out = Vec::new();
        for (i, b) in self.bodies.iter().enumerate() {
            let Ok(shape) = k::Shape::from_brep_bytes(b.brep.as_bytes()) else { continue };
            let feature = context_feature(self.id, i as u32);
            let nf = shape.count(k::ShapeType::Face);
            let faces = (0..nf).map(|f| Arc::new(TopoName::Face { feature, role: Role::New(f as u32) })).collect();
            let kind = if nf == 0 { BodyKind::Wire } else if shape.count(k::ShapeType::Solid) > 0 { BodyKind::Solid } else { BodyKind::Surface };
            out.push(Arc::new(Body::new(BodyId { feature, index: 0 }, b.instance.clone(), kind, shape, faces, 0, feature)));
        }
        out
    }
}

fn path_u64(p: &InstancePath) -> Vec<u64> {
    p.iter().map(|i| i.0).collect()
}

/// Snapshot of an evaluated assembly around the primary instance `primary`
/// (top-level path): every other visible part, placed in the coordinates
/// of the primary part's Part Studio.
pub fn snapshot(result: &AssemblyResult, assembly: TabId, id: u32, name: &str, primary: &InstancePath, instance_name: &dyn Fn(&InstancePath) -> String) -> Option<AssemblyContext> {
    let own = result.occurrences.iter().find(|o| o.path.starts_with(primary))?;
    let mut c = snapshot_with(result, assembly, id, name, own.transform.inverse(), Some(primary), instance_name);
    c.primary_body = Some(own.body.clone());
    c.state = state_hash(result, primary).unwrap_or(0);
    Some(c)
}

/// Hash of what a snapshot around `primary` captures: the other parts'
/// positions relative to the primary instance and their geometry (mesh
/// sizes and bounds). `None` when the primary instance is gone.
pub fn state_hash(result: &AssemblyResult, primary: &InstancePath) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let own = result.occurrences.iter().find(|o| o.path.starts_with(primary))?;
    let to_studio = own.transform.inverse();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let q = |v: f64| (v * 1e7).round() as i64;
    for o in result.occurrences.iter().filter(|o| !o.path.starts_with(primary) && !o.hidden && matches!(o.kind, BodyKind::Solid | BodyKind::Surface)) {
        o.path.iter().for_each(|i| i.0.hash(&mut h));
        (to_studio * o.transform).to_cols_array().iter().for_each(|v| q(*v).hash(&mut h));
        o.mesh.positions.len().hash(&mut h);
        if let Some((a, b)) = o.local_bounds {
            [a.x, a.y, a.z, b.x, b.y, b.z].iter().for_each(|v| q(*v).hash(&mut h));
        }
    }
    Some(h.finish())
}

/// Snapshot of every visible part except the primary instance's, mapped by
/// `to_studio` (assembly → Part Studio coordinates). Create Part Studio in
/// context has no primary instance.
pub fn snapshot_with(result: &AssemblyResult, assembly: TabId, id: u32, name: &str, to_studio: DMat4, primary: Option<&InstancePath>, instance_name: &dyn Fn(&InstancePath) -> String) -> AssemblyContext {
    let mut bodies = Vec::new();
    for o in result.occurrences.iter().filter(|o| !primary.is_some_and(|p| o.path.starts_with(p)) && !o.hidden && matches!(o.kind, BodyKind::Solid | BodyKind::Surface)) {
        let Some(src) = o.context.bodies.iter().find(|b| b.id == o.body) else { continue };
        let m = to_studio * o.transform;
        let Ok(shape) = src.shape.transformed(&k::Transform::Matrix(m)) else { continue };
        let Ok(bytes) = shape.to_brep_bytes() else { continue };
        bodies.push(ContextBody { path: path_u64(&o.path), instance: instance_name(&o.path), part: o.body_name.clone(), color: o.color, brep: String::from_utf8_lossy(&bytes).into() });
    }
    AssemblyContext { id, name: name.to_string(), assembly, primary: primary.map(path_u64).unwrap_or_default(), primary_body: None, bodies, revision: 0, hidden: Vec::new(), state: 0 }
}

/// Next free context id and its default name ("Context N").
pub fn next_context(contexts: &[AssemblyContext]) -> (u32, String) {
    let id = contexts.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    let mut n = contexts.len() + 1;
    while contexts.iter().any(|c| c.name == format!("Context {n}")) {
        n += 1;
    }
    (id, format!("Context {n}"))
}

fn reference_uses_context(r: &Reference, ctx: Option<u32>) -> bool {
    let hit = |f: Option<FeatureId>| f.is_some_and(|f| is_context_feature(f) && ctx.is_none_or(|c| context_of(f) == Some(c)));
    match r {
        Reference::Face(n) | Reference::Edge(n) | Reference::Vertex(n) => topo_uses_context(n, &hit),
        Reference::Body(b) => hit(Some(b.feature)),
        Reference::Implicit { on, .. } => reference_uses_context(on, ctx),
        _ => false,
    }
}

fn topo_uses_context(n: &TopoName, hit: &dyn Fn(Option<FeatureId>) -> bool) -> bool {
    match n {
        TopoName::Face { feature, .. } | TopoName::Wire { feature, .. } => hit(Some(*feature)),
        TopoName::Split { parent, .. } => topo_uses_context(parent, hit),
        TopoName::Edge { faces, .. } => faces.iter().any(|f| topo_uses_context(f, hit)),
        TopoName::Vertex { faces, .. } => faces.iter().any(|f| topo_uses_context(f, hit)),
        TopoName::Boundary { face, .. } => topo_uses_context(face, hit),
    }
}

/// Whether feature parameters reference context geometry (of context
/// `ctx`, or of any context).
pub fn params_use_context(params: &ParamValues, ctx: Option<u32>) -> bool {
    params.0.values().any(|v| value_uses_context(v, ctx))
}

fn value_uses_context(v: &ParamValue, ctx: Option<u32>) -> bool {
    match v {
        ParamValue::Query(refs) => refs.iter().any(|r| reference_uses_context(r, ctx)),
        ParamValue::Array(items) => items.iter().any(|p| params_use_context(p, ctx)),
        ParamValue::Sketch(s) => s.external_refs.values().any(|r| reference_uses_context(r, ctx)),
        _ => false,
    }
}

/// The references of `params` into context geometry (any context), in
/// parameter order: what a feature's context arrow points at.
pub fn context_refs(params: &ParamValues) -> Vec<Reference> {
    fn walk(v: &ParamValue, out: &mut Vec<Reference>) {
        let push = |r: &Reference| {
            if reference_uses_context(r, None) && !out.contains(r) {
                out.push(r.clone());
            }
        };
        match v {
            ParamValue::Query(refs) => refs.iter().for_each(push),
            ParamValue::Array(items) => items.iter().for_each(|p| p.0.values().for_each(|v| walk(v, out))),
            ParamValue::Sketch(s) => s.external_refs.values().for_each(push),
            _ => {}
        }
    }
    let mut out = Vec::new();
    params.0.values().for_each(|v| walk(v, &mut out));
    out
}

/// The contexts some feature of `features` references (contexts that must
/// persist after the context is closed).
pub fn referenced_contexts(features: &[crate::partstudio::Feature]) -> Vec<u32> {
    let mut out = Vec::new();
    for f in features {
        for v in f.params.0.values() {
            collect_contexts(v, &mut out);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn collect_contexts(v: &ParamValue, out: &mut Vec<u32>) {
    let push_ref = |r: &Reference, out: &mut Vec<u32>| {
        let mut ids = Vec::new();
        ref_contexts(r, &mut ids);
        out.extend(ids);
    };
    match v {
        ParamValue::Query(refs) => refs.iter().for_each(|r| push_ref(r, out)),
        ParamValue::Array(items) => items.iter().for_each(|p| p.0.values().for_each(|v| collect_contexts(v, out))),
        ParamValue::Sketch(s) => s.external_refs.values().for_each(|r| push_ref(r, out)),
        _ => {}
    }
}

fn ref_contexts(r: &Reference, out: &mut Vec<u32>) {
    fn topo(n: &TopoName, out: &mut Vec<u32>) {
        match n {
            TopoName::Face { feature, .. } | TopoName::Wire { feature, .. } => out.extend(context_of(*feature)),
            TopoName::Split { parent, .. } => topo(parent, out),
            TopoName::Edge { faces, .. } => faces.iter().for_each(|f| topo(f, out)),
            TopoName::Vertex { faces, .. } => faces.iter().for_each(|f| topo(f, out)),
            TopoName::Boundary { face, .. } => topo(face, out),
        }
    }
    match r {
        Reference::Face(n) | Reference::Edge(n) | Reference::Vertex(n) => topo(n, out),
        Reference::Body(b) => out.extend(context_of(b.feature)),
        Reference::Implicit { on, .. } => ref_contexts(on, out),
        _ => {}
    }
}

/// Context bodies as display bodies: translucent (Onshape draws the
/// assembly context at about 20 % opacity), pickable for references,
/// hidden ones per Hide/show context instances.
pub fn display_bodies(c: &AssemblyContext, bodies: &[Arc<Body>]) -> Vec<crate::display::BodyDisplay> {
    let mut out = Vec::new();
    for (i, b) in bodies.iter().enumerate() {
        let Some(src) = c.bodies.get(i) else { continue };
        let mesh = crate::regen::tessellate_shape(&b.shape, 1.0).unwrap_or_default();
        let bounds = b.shape.bounding_box(false).ok().map(|x| (x.min, x.max));
        out.push(crate::display::BodyDisplay {
            id: b.id.clone(),
            name: src.instance.clone(),
            kind: b.kind,
            color: [src.color[0], src.color[1], src.color[2], CONTEXT_ALPHA],
            hidden: c.hidden.contains(&src.path),
            mesh: Arc::new(mesh),
            face_refs: b.faces.iter().map(|n| Reference::Face((**n).clone())).collect(),
            edge_refs: b.edges.iter().map(|n| Reference::Edge((**n).clone())).collect(),
            vertex_refs: b.vertices.iter().map(|n| Reference::Vertex((**n).clone())).collect(),
            bounds,
            composite: None,
            is_mesh: b.shape.is_mesh(),
        });
    }
    out
}

/// Default opacity of context bodies.
pub const CONTEXT_ALPHA: f32 = 0.2;
