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

/// Whether freshly inserted `ids` are all the assembly has (they went into an
/// empty assembly). Onshape places that first insert with the Part Studio
/// origin on the assembly origin, and it does not follow the cursor; later
/// inserts start there too (the same part again at a slight offset,
/// [`insert_part`]) and follow the cursor once it enters the graphics area,
/// landing where it is clicked (help: Insert parts and assemblies; live
/// behaviours "Insert dialog").
pub fn inserted_into_empty(a: &Assembly, ids: &[InstanceId]) -> bool {
    !ids.is_empty() && a.instances.iter().all(|i| ids.contains(&i.id))
}

/// "Insert : Fastened 1" / "Edit : Fastened 1".
pub fn mate_description(m: &Mate, editing: bool) -> String {
    if editing { format!("Edit : {}", m.name) } else { format!("Insert : {}", m.name) }
}

/// Accept the mate dialog: a new mate is appended, hidden like Onshape's
/// (live: a new mate's row is light grey, tooltip "… (Hidden)"; j shows
/// mates); an edited one replaces the old, with configured parameters going
/// to the current configuration.
pub fn commit_mate(a: &mut Assembly, mut m: Mate, editing: Option<MateId>, values: &ConfigValues) {
    match editing.and_then(|id| a.mates.iter().position(|x| x.id == id)) {
        Some(i) => {
            a.store_mate_edit(&mut m, values);
            a.mates[i] = m;
        }
        None => {
            m.hidden = true;
            a.mates.push(m);
        }
    }
}

/// The instance a mate preview moves (live: drawn amber while the dialog is
/// open): the first connector's, unless it is fixed (then the second's).
pub fn mate_preview_moving(a: &Assembly, m: &Mate) -> InstanceId {
    let fixed = |id: InstanceId| id == InstanceId::ORIGIN || a.instance(id).is_some_and(|i| i.fixed);
    if fixed(m.a.instance) { m.b.instance } else { m.a.instance }
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

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn part(index: u32, width: f64) -> CatalogPart {
        let body = BodyId { feature: noshape_model::reference::FeatureId(1), index };
        (body, format!("Part {}", index + 1), Some((DVec3::ZERO, DVec3::new(width, 0.01, 0.01))))
    }

    #[test]
    fn mate_preview_moves_the_first_picked_unless_fixed() {
        use noshape_model::assembly::{MateConnectorRef, MateType};
        use noshape_model::reference::Reference;
        let mut a = Assembly::default();
        let p0 = insert_part(&mut a, TabId(7), "Part Studio 1", Some(part(0, 0.04)), ConfigValues::default());
        let p1 = insert_part(&mut a, TabId(7), "Part Studio 1", Some(part(1, 0.02)), ConfigValues::default());
        let c = |i: InstanceId| MateConnectorRef::on(i, vec![], Reference::Origin);
        let m = Mate::new(MateId(9), "Revolute 1".into(), MateType::Revolute, c(p1), c(p0));
        assert_eq!(mate_preview_moving(&a, &m), p1, "the first picked moves");
        a.instance_mut(p1).unwrap().fixed = true;
        assert_eq!(mate_preview_moving(&a, &m), p0, "a fixed first pick stays; the other moves");
    }

    #[test]
    fn new_mates_are_hidden_edited_ones_keep_their_visibility() {
        use noshape_model::assembly::{MateConnectorRef, MateType};
        use noshape_model::reference::Reference;
        let mut a = Assembly::default();
        let c = |i: u64| MateConnectorRef::on(InstanceId(i), vec![], Reference::Origin);
        let m = Mate::new(MateId(5), "Fastened 1".into(), MateType::Fastened, c(1), c(2));
        commit_mate(&mut a, m, None, &ConfigValues::default());
        assert!(a.mates[0].hidden, "live: a new mate is hidden");
        let mut shown = a.mates[0].clone();
        shown.hidden = false;
        commit_mate(&mut a, shown, Some(MateId(5)), &ConfigValues::default());
        assert!(!a.mates[0].hidden, "editing keeps the mate's own visibility");
    }

    #[test]
    fn first_insert_lands_at_the_origin_later_ones_follow_the_cursor() {
        let mut a = Assembly::default();
        let studio = TabId(7);
        // The first instance into an empty assembly: origins aligned, placed
        // there without following the cursor.
        let first = insert_part(&mut a, studio, "Part Studio 1", Some(part(0, 0.04)), ConfigValues::default());
        assert!(inserted_into_empty(&a, &[first]));
        assert_eq!(a.instance(first).unwrap().placement, Placement::default());
        // The same part again starts at a slight offset (it then follows the
        // cursor in the graphics area).
        let second = insert_part(&mut a, studio, "Part Studio 1", Some(part(0, 0.04)), ConfigValues::default());
        assert!(!inserted_into_empty(&a, &[second]));
        assert_eq!(a.instance(second).unwrap().placement.translation, [0.05, 0.0, 0.0]);
        // Another part starts with its origin on the assembly origin.
        let other = insert_part(&mut a, studio, "Part Studio 1", Some(part(1, 0.02)), ConfigValues::default());
        assert!(!inserted_into_empty(&a, &[other]));
        assert_eq!(a.instance(other).unwrap().placement, Placement::default());
        // A whole Part Studio's parts into an empty assembly: all at the origin.
        let mut b = Assembly::default();
        let ids = vec![insert_part(&mut b, studio, "Part Studio 1", Some(part(0, 0.04)), ConfigValues::default()), insert_part(&mut b, studio, "Part Studio 1", Some(part(1, 0.02)), ConfigValues::default())];
        assert!(inserted_into_empty(&b, &ids));
        assert!(ids.iter().all(|id| b.instance(*id).unwrap().placement == Placement::default()));
    }
}
