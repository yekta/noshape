//! Assembly edits, shared by the Assembly view and the agent API. Every
//! commit first folds in the latest solved placements ([`fold_solved`]), as
//! the view does, so instances stay where the mates put them.

use noshape_model::assembly::eval::AssemblyResult;
use noshape_model::assembly::{Assembly, Instance, InstanceId, InstanceSource, Mate, MateId, Placement, SubassemblyLock};
use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabId;
use noshape_model::reference::BodyId;

/// Copy the solver's placements (and flexible subassemblies' nested
/// positions) into the instances before an edit is committed.
pub fn fold_solved(a: &mut Assembly, result: &AssemblyResult) {
    let solved = &result.solved;
    let flexible = &result.prepared.flexible;
    for (id, p) in &solved.placements {
        if let Some(i) = a.instance_mut(*id) {
            i.placement = *p;
            // Flexible subassemblies keep their parts' positions here.
            if flexible.contains(id) {
                let mut n: Vec<(InstanceId, Placement)> = solved.nested.iter().filter(|((s, _), _)| s == id).map(|((_, c), q)| (*c, *q)).collect();
                n.sort_by_key(|(c, _)| *c);
                i.nested = n;
            }
        }
    }
}

/// A part of a Part Studio to insert: (body, part name, bounding box).
pub type CatalogPart = (BodyId, String, Option<(glam::DVec3, glam::DVec3)>);

/// "Insert : Part 1" / "Insert : Part Studio 1".
pub fn insert_description(studio_name: &str, part: Option<&CatalogPart>) -> String {
    match part {
        Some((_, n, _)) => format!("Insert : {n}"),
        None => format!("Insert : {studio_name}"),
    }
}

/// Insert dialog › a part (or the whole Part Studio) in configuration
/// `config`. Later instances of the same item are offset along X so they
/// don't overlap.
pub fn insert_part(a: &mut Assembly, studio: TabId, studio_name: &str, part: Option<CatalogPart>, config: ConfigValues) -> InstanceId {
    let id = InstanceId(a.next_id());
    let (source, base, width) = match part {
        Some((b, n, bb)) => (InstanceSource::Part { studio, body: b, config }, n, bb.map(|(lo, hi)| hi.x - lo.x).unwrap_or(0.02)),
        None => (InstanceSource::PartStudio { studio, config }, studio_name.to_string(), 0.05),
    };
    let same = a.instances.iter().filter(|i| i.source == source).count();
    let placement = Placement { translation: [same as f64 * (width + 0.01), 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] };
    let name = a.next_instance_name(&base);
    a.instances.push(Instance { id, name, source, placement, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None });
    id
}

/// "Insert : Fastened 1" / "Edit : Fastened 1".
pub fn mate_description(m: &Mate, editing: bool) -> String {
    if editing { format!("Edit : {}", m.name) } else { format!("Insert : {}", m.name) }
}

/// Accept the mate dialog: a new mate is appended; an edited one replaces
/// the old, with configured parameters going to the current configuration.
pub fn commit_mate(a: &mut Assembly, mut m: Mate, editing: Option<MateId>, values: &ConfigValues) {
    match editing.and_then(|id| a.mates.iter().position(|x| x.id == id)) {
        Some(i) => {
            a.store_mate_edit(&mut m, values);
            a.mates[i] = m;
        }
        None => a.mates.push(m),
    }
}

/// Solve options while the mate dialog previews `m` (before Solve): only
/// this mate is solved, and the second connector's instance stays put
/// unless one of the two is fixed (help: Mates).
pub fn mate_preview_options(a: &Assembly, m: &Mate) -> noshape_model::assembly::eval::SolveOptions {
    let mut opts = noshape_model::assembly::eval::SolveOptions::default();
    let fixed = |id: InstanceId| a.instance(id).is_some_and(|i| i.fixed);
    if !fixed(m.a.instance) && !fixed(m.b.instance) {
        opts.hold.push(m.b.instance);
    }
    opts.only_mate = Some(m.id);
    opts
}

/// The assembly as evaluated in configuration `values` (the configuration
/// chosen above the instance list).
pub fn effective(a: &Assembly, values: &ConfigValues) -> Assembly {
    let mut asm = a.clone();
    if asm.has_configured() {
        asm.apply_configuration(values);
    }
    asm
}

/// "Fix" / "Unfix".
pub fn fix_description(fixed: bool) -> &'static str {
    if fixed { "Fix" } else { "Unfix" }
}

pub fn set_fixed(a: &mut Assembly, ids: &[InstanceId], fixed: bool) {
    for id in ids {
        if let Some(i) = a.instance_mut(*id) {
            i.fixed = fixed;
        }
    }
}

/// "Hide" / "Show".
pub fn hide_description(hidden: bool) -> &'static str {
    if hidden { "Hide" } else { "Show" }
}

pub fn set_hidden(a: &mut Assembly, ids: &[InstanceId], hidden: bool) {
    for id in ids {
        if let Some(i) = a.instance_mut(*id) {
            i.hidden = hidden;
        }
    }
}

/// "Suppress" / "Unsuppress".
pub fn suppress_description(s: bool) -> &'static str {
    if s { "Suppress" } else { "Unsuppress" }
}

/// Suppress instances; a configured suppression changes the current
/// configuration's value instead.
pub fn set_suppressed(a: &mut Assembly, ids: &[InstanceId], s: bool, values: &ConfigValues) {
    use noshape_model::assembly::config as acfg;
    for id in ids {
        if a.set_configured_value(acfg::ConfigTarget::Instance(*id), acfg::param::UNSUPPRESSED, values, noshape_model::params::ParamValue::Bool(!s)) {
            continue;
        }
        if let Some(i) = a.instance_mut(*id) {
            i.suppressed = s;
        }
    }
}

pub const DELETE_INSTANCES: &str = "Delete instances";

pub fn delete_instances(a: &mut Assembly, ids: &[InstanceId]) {
    for id in ids {
        a.remove_instance(*id);
    }
    a.prune_configured();
}

/// "Delete : Fastened 1" (a mate, relation or group).
pub fn delete_mate_description(a: &Assembly, id: MateId) -> String {
    let name = a.mate(id).map(|m| m.name.clone()).or_else(|| a.relations.iter().find(|r| r.id == id).map(|r| r.name.clone())).or_else(|| a.groups.iter().find(|g| g.id == id).map(|g| g.name.clone()));
    format!("Delete : {}", name.unwrap_or_default())
}

pub fn delete_mate(a: &mut Assembly, id: MateId) {
    a.remove_mate(id);
    a.prune_configured();
}

/// Default primary-axis alignment of a new mate (live assembly/760–780):
/// the moving (first) instance keeps its orientation, so the Z axes agree
/// when they point the same way now and are opposed otherwise.
pub fn default_flip_primary(result: &AssemblyResult, a: &noshape_model::assembly::MateConnectorRef, b: &noshape_model::assembly::MateConnectorRef, units: &noshape_units::UnitPrefs) -> Option<bool> {
    let (fa, fb) = (result.connector_world(a, units)?, result.connector_world(b, units)?);
    Some(fa.z().dot(fb.z()) > 0.0)
}
