//! Assembly contexts: the model core's `in_context` (context bodies in regeneration) plus the snapshots taken from an
//! evaluated assembly (they need `AssemblyResult`).

pub use noshape_model_core::in_context::*;
use crate::assembly::eval::{AssemblyResult, InstancePath};
use crate::display::BodyKind;
use crate::document::TabId;
use glam::DMat4;
use noshape_kernel as k;

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
    AssemblyContext { id, name: name.to_string(), assembly, primary: primary.map(path_u64).unwrap_or_default(), primary_body: None, bodies, revision: 0, hidden: Vec::new(), state: 0, origin: None }
}
