//! Modeling in context (help: Modeling in context), shared by the assembly
//! and Part Studio views and the agent API: Edit in context and Create Part
//! Studio in context snapshot the assembly into the Part Studio as an
//! Assembly context; Update context re-snapshots it; Insert and go to
//! assembly places a Part Studio created in context at its origin.

use noshape_model::assembly::eval::{AssemblyResult, InstancePath};
use noshape_model::assembly::{Assembly, Instance, InstanceId, InstanceSource, Placement};
use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabId;
use noshape_model::in_context::{self as ic, AssemblyContext};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{BodyId, FeatureId};

/// The Part Studio's contexts of `assembly` that exist for the user (a
/// feature references them; live assembly/548: Edit in context alone keeps
/// none): (id, name).
pub fn contexts_of(ps: &PartStudio, assembly: TabId) -> Vec<(u32, String)> {
    let referenced = ic::referenced_contexts(&ps.features);
    ps.contexts.iter().filter(|c| c.assembly == assembly && referenced.contains(&c.id)).map(|c| (c.id, c.name.clone())).collect()
}

/// The context Edit in context shows: `existing` (Edit Context N) when it
/// is one of this assembly's, else a new snapshot "Context N" around the
/// `primary` instance (None: Create Part Studio in context, the assembly
/// mapped by `to_studio`). Returns its id and the snapshot to install.
pub fn context_for(ps: &PartStudio, result: &AssemblyResult, assembly: TabId, primary: Option<&InstancePath>, existing: Option<u32>, to_studio: glam::DMat4, name_of: &dyn Fn(&InstancePath) -> String) -> Option<(u32, Option<AssemblyContext>)> {
    if let Some(id) = existing.filter(|id| ps.contexts.iter().any(|c| c.id == *id && c.assembly == assembly)) {
        return Some((id, None));
    }
    let referenced = ic::referenced_contexts(&ps.features);
    let kept: Vec<_> = ps.contexts.iter().filter(|c| referenced.contains(&c.id)).cloned().collect();
    let (id, name) = ic::next_context(&kept);
    let c = match primary {
        Some(p) => ic::snapshot(result, assembly, id, &name, p, name_of)?,
        None => ic::snapshot_with(result, assembly, id, &name, to_studio, None, name_of),
    };
    Some((id, Some(c)))
}

/// Show context `id` (installing a new snapshot first: contexts no feature
/// references don't persist, Onshape, and a new one replaces them).
pub fn install(ps: &mut PartStudio, id: u32, new: Option<AssemblyContext>) {
    if let Some(c) = new {
        let referenced = ic::referenced_contexts(&ps.features);
        ps.contexts.retain(|c| referenced.contains(&c.id));
        ps.contexts.push(c);
    }
    ps.active_context = Some(id);
}

/// Exit context: no context shown; it stays only while a feature
/// references it ("An entity must be referenced for the assembly context
/// to persist").
pub fn exit(ps: &mut PartStudio, id: u32) {
    ps.active_context = None;
    if !ic::referenced_contexts(&ps.features).contains(&id) {
        ps.contexts.retain(|c| c.id != id);
    }
}

pub fn update_description(name: &str) -> String {
    format!("Update context : {name}")
}

/// Update context: a new snapshot of the assembly as it is now around the
/// context's primary instance (or `fallback` when that is gone), keeping
/// its name and hidden instances; features referencing it regenerate.
pub fn refreshed(old: &AssemblyContext, result: &AssemblyResult, assembly: &Assembly, fallback: Option<InstanceId>) -> Option<AssemblyContext> {
    let name_of = |p: &InstancePath| assembly.instance(p[0]).map(|i| i.name.clone()).unwrap_or_default();
    let primary: Vec<InstanceId> = match old.primary.first() {
        Some(p) if assembly.instance(InstanceId(*p)).is_some() => old.primary.iter().map(|i| InstanceId(*i)).collect(),
        _ => vec![fallback?],
    };
    let mut fresh = ic::snapshot(result, old.assembly, old.id, &old.name, &primary, &name_of)?;
    fresh.revision = old.revision + 1;
    fresh.hidden = old.hidden.clone();
    fresh.origin = old.origin;
    Some(fresh)
}

/// The context is out of date: the assembly around its primary instance
/// changed since the snapshot (the blue dot).
pub fn out_of_date(c: &AssemblyContext, result: &AssemblyResult) -> bool {
    let path: Vec<InstanceId> = c.primary.iter().map(|i| InstanceId(*i)).collect();
    c.state != 0 && ic::state_hash(result, &path).is_some_and(|h| h != c.state)
}

/// Insert and go to assembly: the Part Studio's parts and sketches as new
/// instances at `at` (the origin the Part Studio was created at). Returns
/// their ids.
pub fn insert_from_context(a: &mut Assembly, studio: TabId, parts: &[(BodyId, String)], sketches: &[(FeatureId, String)], at: Placement) -> Vec<InstanceId> {
    let sources = parts.iter().map(|(b, n)| (n.as_str(), InstanceSource::Part { studio, body: b.clone(), config: ConfigValues::default() }));
    let sketch_sources = sketches.iter().map(|(f, n)| (n.as_str(), InstanceSource::Sketch { studio, sketch: *f, config: ConfigValues::default() }));
    let mut out = Vec::new();
    for (base, source) in sources.chain(sketch_sources) {
        let id = InstanceId(a.next_id());
        let name = a.next_instance_name(base);
        let mut inst = Instance::new(id, name, source, at);
        inst.hidden = false;
        a.instances.push(inst);
        out.push(id);
    }
    out
}
