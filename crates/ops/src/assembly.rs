//! Assembly edits, shared by the Assembly view and the agent API. Every
//! commit first folds in the latest solved placements ([`fold_solved`]), as
//! the view does, so instances stay where the mates put them.

use noshape_model::assembly::analysis::InstanceInterference;
use noshape_model::assembly::eval::AssemblyResult;
use noshape_model::assembly::config::{self as acfg, ConfigTarget};
use noshape_model::assembly::{AsmMateConnector, Assembly, Group, Instance, InstanceId, InstancePattern, InstanceSource, Mate, MateConnectorRef, MateId, MateType, NamedPosition, Placement, Relation, RelationType, SubassemblyLock};
use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabId;
use noshape_model::params::ParamValue;
use noshape_model::reference::BodyId;
use noshape_units::ExprValue;

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
    let (source, base, size) = match part {
        Some((b, n, bb)) => (InstanceSource::Part { studio, body: b, config }, n, bb.map(|(lo, hi)| hi - lo)),
        None => (InstanceSource::PartStudio { studio, config }, studio_name.to_string(), None),
    };
    // Live (qa5 pass 7, A18): the same part again starts at +1/8 of its
    // bounding box per earlier instance (a 40 x 20 x 10 block: 5, 2.5,
    // 1.25 mm); a different part at the origin.
    let same = a.instances.iter().filter(|i| i.source == source).count() as f64;
    let step = size.map(|d| d / 8.0).unwrap_or(glam::DVec3::new(0.05 / 8.0, 0.0, 0.0));
    let placement = Placement { translation: (step * same).into(), rotation: [0.0, 0.0, 0.0, 1.0] };
    let name = a.next_instance_name(&base);
    a.instances.push(Instance { id, name, source, placement, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None });
    id
}

/// Instance menu "Select instances with same configuration" (`same_part`
/// false: any part of the same Part Studio in the same configuration) and
/// "… with same part and same configuration" (live qa5 CA2).
pub fn same_configuration(a: &Assembly, id: InstanceId, same_part: bool) -> Vec<InstanceId> {
    let Some(inst) = a.instance(id) else { return Vec::new() };
    let key = |s: &InstanceSource| match s {
        InstanceSource::Part { studio, body, config } => Some((*studio, Some(body.clone()), config.clone())),
        InstanceSource::PartStudio { studio, config } => Some((*studio, None, config.clone())),
        _ => None,
    };
    let Some((studio, body, config)) = key(&inst.source) else { return vec![id] };
    a.instances
        .iter()
        .filter(|i| key(&i.source).is_some_and(|(s, b, c)| s == studio && c == config && (!same_part || b == body)))
        .map(|i| i.id)
        .collect()
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

/// "Delete : Fastened 1" (a mate, relation, group, mate connector or pattern).
pub fn delete_mate_description(a: &Assembly, id: MateId) -> String {
    let name = a
        .mate(id)
        .map(|m| m.name.clone())
        .or_else(|| a.relations.iter().find(|r| r.id == id).map(|r| r.name.clone()))
        .or_else(|| a.groups.iter().find(|g| g.id == id).map(|g| g.name.clone()))
        .or_else(|| a.connectors.iter().find(|c| c.id == id).map(|c| c.name.clone()))
        .or_else(|| a.pattern(id).map(|p| p.name.clone()));
    format!("Delete : {}", name.unwrap_or_default())
}

pub fn delete_mate(a: &mut Assembly, id: MateId) {
    a.remove_mate(id);
    a.prune_configured();
}

/// Default primary-axis alignment of a new mate (live assembly/760–780):
/// the moving (first) instance keeps its orientation, so the Z axes agree
/// when they point the same way now and are opposed otherwise. Perpendicular
/// axes (a tie) align them (live "qa4 round 5 s3 mate flip" (4): a pin laid
/// on its side gets the aligned Revolute of the upright pin).
pub fn default_flip_primary(result: &AssemblyResult, a: &noshape_model::assembly::MateConnectorRef, b: &noshape_model::assembly::MateConnectorRef, units: &noshape_units::UnitPrefs) -> Option<bool> {
    let (fa, fb) = (result.connector_world(a, units)?, result.connector_world(b, units)?);
    Some(flip_for_axes(fa.z(), fb.z()))
}

/// [`default_flip_primary`] for two connector Z axes (true: aligned).
pub fn flip_for_axes(za: glam::DVec3, zb: glam::DVec3) -> bool {
    za.dot(zb) > -1e-9
}

/// Targets of a part pattern / replicate: (code, target connector, initial placement).
pub type PatternTargets = Vec<(u32, noshape_model::assembly::MateConnectorRef, Option<Placement>)>;

/// "Insert : Linear pattern 1" / "Edit : Linear pattern 1".
pub fn pattern_description(name: &str, editing: bool) -> String {
    format!("{} : {name}", if editing { "Edit" } else { "Insert" })
}

/// The pattern dialog's default distance (angle) and instance count (live
/// "qa5 assembly" 6: Distance 25 mm, Angle 360 deg, Instance count 2).
pub fn pattern_defaults(circular: bool) -> (&'static str, &'static str) {
    if circular { ("360 deg", "2") } else { ("25 mm", "2") }
}

/// The mate a part pattern / replicate follows: the selected mate when it
/// uses the seed, else the seed's only own (not generated) mate.
pub fn seed_mate(a: &Assembly, seed: InstanceId, selected: Option<MateId>) -> Option<MateId> {
    let mates: Vec<&Mate> = a.mates.iter().filter(|m| m.uses_instance(seed) && a.mate_generated_by(m.id).is_none()).collect();
    if let Some(sel) = selected
        && mates.iter().any(|m| m.id == sel)
    {
        return Some(sel);
    }
    (mates.len() == 1).then(|| mates[0].id)
}

/// Replicate's default match scope (help: Replicate 6): the first edge
/// option when the seed mate's target is an edge, else the first face one.
pub fn default_replicate_scope(a: &Assembly, seed: InstanceId, mate: MateId) -> noshape_model::assembly::ReplicateScope {
    use noshape_model::assembly::ReplicateScope;
    use noshape_model::reference::Reference;
    let Some(m) = a.mate(mate) else { return ReplicateScope::FacesInParts };
    let target = if m.a.instance == seed { &m.b } else { &m.a };
    let entity = match &target.reference {
        Reference::Implicit { on, .. } => &**on,
        r => r,
    };
    if matches!(entity, Reference::Edge(_)) { ReplicateScope::EdgesOnPlane } else { ReplicateScope::FacesInParts }
}

/// Accept the pattern dialog: the pattern replaces its old version (or is
/// appended), then its instances are generated, on `targets` for a part
/// pattern / replicate. Returns the instances created.
pub fn commit_pattern(a: &mut Assembly, pat: InstancePattern, targets: Option<PatternTargets>) -> Vec<InstanceId> {
    let id = pat.id;
    match a.patterns.iter().position(|p| p.id == id) {
        Some(i) => a.patterns[i] = pat,
        None => a.patterns.push(pat),
    }
    match targets {
        Some(t) => a.sync_matched_pattern(id, t),
        None => a.sync_pattern(id),
    }
}

/// The Interference detection dialog's rows: (row index, "A and B", the
/// overlap's size "w x h x d"); with Show top level only, one row per pair
/// of top-level instances, named by them.
pub fn interference_rows(result: &AssemblyResult, rows: &[InstanceInterference], top_only: bool, units: &noshape_units::UnitPrefs) -> Vec<(usize, String, String)> {
    let len = |x: f64| noshape_units::format_quantity(noshape_units::Quantity::new(x, noshape_units::Dim::LENGTH), units);
    let name = |p: &[InstanceId]| -> String {
        let p = if top_only { &p[..1] } else { p };
        result.node(p).map(|n| n.name.clone()).unwrap_or_default()
    };
    let mut out = Vec::new();
    let mut seen: Vec<(InstanceId, InstanceId)> = Vec::new();
    for (k, r) in rows.iter().enumerate() {
        if top_only {
            let pair = (r.a[0].min(r.b[0]), r.a[0].max(r.b[0]));
            if r.a[0] == r.b[0] || seen.contains(&pair) {
                continue;
            }
            seen.push(pair);
        }
        let (lo, hi) = r.overlap.bounds;
        let size = hi - lo;
        out.push((k, format!("{} and {}", name(&r.a), name(&r.b)), format!("{} x {} x {}", len(size.x), len(size.y), len(size.z))));
    }
    out
}

/// "Insert : Group" / "Edit : Group".
pub fn group_description(editing: bool) -> &'static str {
    if editing { "Edit : Group" } else { "Insert : Group" }
}

/// The Group dialog's check.
pub const GROUP_TOO_FEW: &str = "Select at least two instances to group.";

/// Accept the Group dialog: a new group ("Group 1"...) or the edited
/// group's instances. Returns the group's id.
pub fn commit_group(a: &mut Assembly, editing: Option<MateId>, ids: Vec<InstanceId>) -> MateId {
    if let Some(gid) = editing {
        if let Some(g) = a.groups.iter_mut().find(|g| g.id == gid) {
            g.instances = ids;
        }
        return gid;
    }
    let id = MateId(a.next_id());
    let name = a.next_feature_name("Group");
    a.groups.push(Group { id, name, instances: ids, suppressed: false });
    id
}

/// A relation's base name ("Gear" -> "Gear 1", "Insert : Gear relation").
pub fn relation_name(kind: RelationType) -> &'static str {
    match kind {
        RelationType::Gear => "Gear",
        RelationType::RackAndPinion => "Rack and pinion",
        RelationType::Screw => "Screw",
        RelationType::Linear => "Linear",
    }
}

/// The relation dialog's default ratio (gear / linear) or distance per
/// revolution (rack and pinion, screw); live (behaviours.md "Assembly
/// relation dialogs"): "Relation ratio 1", "Distance per revolution 25 mm".
pub fn relation_default_ratio(kind: RelationType) -> &'static str {
    match kind {
        RelationType::Gear | RelationType::Linear => "1",
        RelationType::RackAndPinion | RelationType::Screw => "25 mm",
    }
}

/// The relation dialog's value row label.
pub fn relation_value_label(kind: RelationType) -> &'static str {
    match kind {
        RelationType::Gear | RelationType::Linear => "Relation ratio",
        RelationType::RackAndPinion | RelationType::Screw => "Distance per revolution",
    }
}

/// A mate's entry in the relation dialog's Mate(s) field: the coupled
/// degree of freedom ("⟳ - z - Revolute 1": rotation about z, "↹ - z -
/// Slider 1": translation along z); a screw's one cylindrical mate is just
/// its name (it uses both). `slot`: 0 the first mate, 1 the second.
pub fn relation_mate_label(kind: RelationType, slot: usize, mate_type: MateType, name: &str) -> String {
    let rotational = match kind {
        RelationType::Gear => true,
        RelationType::RackAndPinion => slot == 0,
        RelationType::Linear => false,
        RelationType::Screw => return name.to_string(),
    };
    let axis = match (rotational, mate_type) {
        (true, MateType::Revolute | MateType::Cylindrical | MateType::PinSlot | MateType::Planar | MateType::Parallel) => "z",
        (false, MateType::Slider | MateType::Cylindrical | MateType::Parallel) => "z",
        (false, MateType::PinSlot | MateType::Planar) => "x",
        _ => return name.to_string(),
    };
    format!("{} - {axis} - {name}", if rotational { "⟳" } else { "↹" })
}

/// "Insert : Gear relation" (also when editing, as the dialog does).
pub fn relation_description(kind: RelationType) -> String {
    format!("Insert : {} relation", relation_name(kind))
}

/// The relation dialog's checks: two different mates (a screw uses one),
/// of types the relation couples. `Err((is_error, message))`.
pub fn check_relation(a: &Assembly, kind: RelationType, first: Option<MateId>, second: Option<MateId>) -> Result<[MateId; 2], (bool, String)> {
    let screw = kind == RelationType::Screw;
    let (Some(x), Some(y)) = (first, if screw { first } else { second }) else { return Err((false, "Select two mates for the relation.".into())) };
    if x == y && !screw {
        return Err((false, "A relation needs two different mates.".into()));
    }
    if let (Some(ma), Some(mb)) = (a.mate(x), a.mate(y))
        && let Err(e) = noshape_model::assembly::eval::relation_values(kind, ma.mate_type, mb.mate_type)
    {
        return Err((true, e));
    }
    Ok([x, y])
}

/// Accept the relation dialog. `base`: the mates' current values
/// (`AssemblyResult::relation_base`), the coupling's reference.
pub fn commit_relation(a: &mut Assembly, editing: Option<MateId>, kind: RelationType, mates: [MateId; 2], ratio: ExprValue, reverse: bool, base: Option<[f64; 2]>) -> MateId {
    if let Some(id) = editing
        && let Some(x) = a.relations.iter_mut().find(|x| x.id == id)
    {
        x.relation_type = kind;
        x.mates = mates;
        x.ratio = ratio;
        x.reverse = reverse;
        x.base = base;
        return id;
    }
    let id = MateId(a.next_id());
    let name = a.next_feature_name(relation_name(kind));
    a.relations.push(Relation { id, name, relation_type: kind, mates, ratio, reverse, suppressed: false, base });
    id
}

/// Accept the Mate connector dialog: mates using the connector follow its
/// owner; the connector replaces its old version or is appended.
pub fn commit_connector(a: &mut Assembly, c: AsmMateConnector) {
    for m in &mut a.mates {
        for r in std::iter::once(&mut m.a).chain(std::iter::once(&mut m.b)).chain(m.extra.iter_mut()) {
            if r.connector == Some(c.id) {
                r.instance = c.on.instance;
            }
        }
    }
    match a.connectors.iter().position(|x| x.id == c.id) {
        Some(i) => a.connectors[i] = c,
        None => a.connectors.push(c),
    }
}

/// Suppress / Unsuppress of a mate, relation or group ("Suppress mate"):
/// a configured mate's value changes in configuration `values`.
pub fn toggle_mate_suppressed(a: &mut Assembly, id: MateId, values: &ConfigValues) {
    let now = a.configured(values).mate(id).map(|m| m.suppressed);
    if let Some(now) = now
        && a.set_configured_value(ConfigTarget::Mate(id), acfg::param::UNSUPPRESSED, values, ParamValue::Bool(now))
    {
        return;
    }
    if let Some(m) = a.mates.iter_mut().find(|m| m.id == id) {
        m.suppressed = !m.suppressed;
    }
    if let Some(r) = a.relations.iter_mut().find(|r| r.id == id) {
        r.suppressed = !r.suppressed;
    }
    if let Some(g) = a.groups.iter_mut().find(|g| g.id == id) {
        g.suppressed = !g.suppressed;
    }
}

/// "Replace instance : Part 2".
pub fn replace_description(base: &str) -> String {
    format!("Replace instance : {base}")
}

/// Replace instance `id` (with `all`, every instance of the same source)
/// by `source`. Mates keep their references: where the replacement lacks
/// the same geometry names they fail, as live ("Mate cannot resolve mate
/// connectors.", qa5 MT9). Returns (replaced, 0); `old` / `new` are kept for
/// the agent API's signature.
pub fn replace_instance(a: &mut Assembly, id: InstanceId, source: InstanceSource, base: &str, all: bool, _old: &AssemblyResult, _new: Option<(&noshape_model::regen::context::Context, Option<&BodyId>)>) -> (usize, usize) {
    (a.replace_instance(id, source, base, all).len(), 0)
}

/// The message after Replace instance.
pub fn replace_message(count: usize, remapped: usize) -> String {
    if remapped > 0 { format!("Replaced {count} instance(s); {remapped} mate reference(s) matched to the nearest geometry of the same type.") } else { format!("Replaced {count} instance(s); mates are kept where the new geometry has the same names.") }
}

/// The degrees of freedom of a mate that a named position can drive, in
/// the Driving mate selector's order (0 = rotation about Z, 1..3 =
/// translation along X / Y / Z).
pub fn driven_dofs(t: noshape_model::assembly::MateType) -> &'static [u8] {
    use noshape_model::assembly::MateType;
    match t {
        MateType::Revolute => &[0],
        MateType::Slider => &[3],
        MateType::Cylindrical => &[3, 0],
        MateType::PinSlot => &[1, 0],
        MateType::Planar => &[1, 2, 0],
        MateType::Parallel => &[1, 2, 3, 0],
        _ => &[],
    }
}

/// Add named position's default name: "Named position N".
pub fn next_named_position_name(a: &Assembly) -> String {
    let mut k = a.named_positions.len() + 1;
    while a.named_positions.iter().any(|p| p.name == format!("Named position {k}")) {
        k += 1;
    }
    format!("Named position {k}")
}

/// A named position holding the current state (placements and the driving
/// mates' values).
pub fn capture_named_position(a: &Assembly, result: &AssemblyResult, name: String) -> NamedPosition {
    NamedPosition { name, placements: result.solved.placements.clone(), mate_values: result.named_position_values(a) }
}

/// Apply a position: its driving values are imposed (the other mates stay
/// as they are); without driving mates its stored placements are solved.
pub fn apply_named_position(a: &Assembly, result: &AssemblyResult, np: &NamedPosition) -> noshape_model::assembly::eval::Solved {
    let hold: Vec<(MateId, usize, f64)> = a.position_drivers.iter().filter_map(|(m, ix)| np.mate_values.iter().find(|(x, _)| x == m).map(|(_, v)| (*m, *ix as usize, v[*ix as usize]))).collect();
    if hold.is_empty() {
        return result.prepared.solve_named_position(np);
    }
    let mut start: std::collections::HashMap<InstanceId, Placement> = result.solved.placements.iter().cloned().collect();
    for (id, p) in &np.placements {
        start.insert(*id, *p);
    }
    result.prepared.solve(&noshape_model::assembly::eval::SolveOptions { start: Some(start), hold_values: hold, ..Default::default() })
}

/// Duplicate named position's name: "<name> (1)", "<name> (2)"...
pub fn duplicate_position_name(a: &Assembly, name: &str) -> String {
    let mut k = 1;
    while a.named_positions.iter().any(|p| p.name == format!("{name} ({k})")) {
        k += 1;
    }
    format!("{name} ({k})")
}

/// Set a driving value of position `row` (a mate's value index `ix`);
/// `current`: the mate's values now, for a mate the position hasn't stored.
pub fn set_position_value(a: &mut Assembly, row: usize, mate: MateId, ix: usize, value: f64, current: [f64; 4]) {
    if let Some(np) = a.named_positions.get_mut(row) {
        match np.mate_values.iter_mut().find(|(m, _)| *m == mate) {
            Some((_, v)) => v[ix] = value,
            None => {
                let mut v = current;
                v[ix] = value;
                np.mate_values.push((mate, v));
            }
        }
    }
}

/// Add exploded view's name: "Explode N".
pub fn next_exploded_view_name(a: &Assembly) -> String {
    format!("Explode {}", a.exploded_views.len() + 1)
}

#[cfg(test)]
mod tests {
    /// Live (behaviours "qa4 round 5 s3 mate flip" (4), assembly/760–780): the
    /// Z axes agree when they point the same way and when they are
    /// perpendicular; opposed ones flip.
    #[test]
    fn default_flip_ties_align() {
        use glam::DVec3;
        assert!(flip_for_axes(DVec3::Z, DVec3::Z));
        assert!(flip_for_axes(DVec3::NEG_Y, DVec3::Z));
        assert!(!flip_for_axes(DVec3::NEG_Z, DVec3::Z));
    }

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
        // The same part again starts at +1/8 of its bounding box (live qa5
        // pass 7: 5, 2.5, 1.25 for a 40 x 20 x 10 block); it then follows
        // the cursor once that enters the graphics area.
        let second = insert_part(&mut a, studio, "Part Studio 1", Some(part(0, 0.04)), ConfigValues::default());
        assert!(!inserted_into_empty(&a, &[second]));
        let t = DVec3::from(a.instance(second).unwrap().placement.translation);
        assert!((t - DVec3::new(0.005, 0.00125, 0.00125)).length() < 1e-12, "{t:?}");
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

    /// qa5 A13: the relation dialog's defaults and Mate(s) entries as live.
    #[test]
    fn relation_dialog_matches_live() {
        assert_eq!(relation_default_ratio(RelationType::Gear), "1");
        assert_eq!(relation_default_ratio(RelationType::Screw), "25 mm");
        assert_eq!(relation_value_label(RelationType::RackAndPinion), "Distance per revolution");
        assert_eq!(relation_value_label(RelationType::Linear), "Relation ratio");
        assert_eq!(relation_mate_label(RelationType::Gear, 1, MateType::Cylindrical, "Cylindrical 1"), "⟳ - z - Cylindrical 1");
        assert_eq!(relation_mate_label(RelationType::RackAndPinion, 1, MateType::Slider, "Slider 1"), "↹ - z - Slider 1");
        assert_eq!(relation_mate_label(RelationType::Linear, 1, MateType::Cylindrical, "Cylindrical 1"), "↹ - z - Cylindrical 1");
        assert_eq!(relation_mate_label(RelationType::Screw, 0, MateType::Cylindrical, "Cylindrical 1"), "Cylindrical 1");
    }

    /// qa5 CA2: the instance menu's "Select instances with same (part and
    /// same) configuration".
    #[test]
    fn select_instances_with_same_configuration() {
        let mut a = Assembly::default();
        let studio = TabId(7);
        let big = ConfigValues([("size".to_string(), "Big".to_string())].into_iter().collect());
        let p0 = insert_part(&mut a, studio, "Part Studio 1", Some(part(0, 0.04)), big.clone());
        let p0b = insert_part(&mut a, studio, "Part Studio 1", Some(part(0, 0.04)), big.clone());
        let p1 = insert_part(&mut a, studio, "Part Studio 1", Some(part(1, 0.02)), big.clone());
        let small = insert_part(&mut a, studio, "Part Studio 1", Some(part(0, 0.04)), ConfigValues::default());
        assert_eq!(same_configuration(&a, p0, false), vec![p0, p0b, p1]);
        assert_eq!(same_configuration(&a, p0, true), vec![p0, p0b]);
        assert_eq!(same_configuration(&a, small, true), vec![small]);
    }
}

// ------------------------------------------------------------ instance menu

pub const MOVE_TO_NEW_SUBASSEMBLY: &str = "Move to new subassembly";
pub const CREATE_EMPTY_SUBASSEMBLY: &str = "Create empty subassembly";
pub const DISSOLVE_SUBASSEMBLY: &str = "Dissolve subassembly";

fn assembly_mut(doc: &mut noshape_model::Document, tab: TabId) -> Option<&mut Assembly> {
    match doc.tab_mut(tab).map(|t| &mut t.content) {
        Some(noshape_model::document::TabContent::Assembly(a)) => Some(a),
        _ => None,
    }
}

/// Instances Move to new subassembly takes (pattern / mirror copies stay).
pub fn movable_to_subassembly(a: &Assembly, ids: &[InstanceId]) -> Vec<InstanceId> {
    ids.iter().copied().filter(|id| a.instance(*id).is_some() && a.generated_by(*id).is_none()).collect()
}

/// Move to new subassembly: a new Assembly tab holding the instances (and
/// their mates), inserted in their place. Returns the new instance.
pub fn move_to_new_subassembly(doc: &mut noshape_model::Document, tab: TabId, ids: &[InstanceId]) -> Option<InstanceId> {
    let name = noshape_model::assembly::ops::next_assembly_tab_name(doc);
    let sub_tab = doc.add_tab(&name, noshape_model::document::TabContent::Assembly(Box::default()));
    let (sub, sid) = assembly_mut(doc, tab)?.move_to_new_subassembly(ids, sub_tab, &name);
    if let Some(t) = doc.tab_mut(sub_tab) {
        t.content = noshape_model::document::TabContent::Assembly(Box::new(sub));
    }
    Some(sid)
}

/// Create empty subassembly: a new, empty Assembly tab inserted here.
pub fn create_empty_subassembly(doc: &mut noshape_model::Document, tab: TabId) -> Option<InstanceId> {
    let name = noshape_model::assembly::ops::next_assembly_tab_name(doc);
    let sub_tab = doc.add_tab(&name, noshape_model::document::TabContent::Assembly(Box::default()));
    let parent = assembly_mut(doc, tab)?;
    let id = InstanceId(parent.next_id());
    let n = parent.next_instance_name(&name);
    parent.instances.push(Instance::new(id, n, InstanceSource::Assembly { assembly: sub_tab, config: Default::default() }, Placement::default()));
    Some(id)
}

/// Dissolve subassembly: its instances and mates move up into this
/// assembly; the subassembly tab is left empty when nothing else uses it.
/// Returns the instances made.
pub fn dissolve_subassemblies(doc: &mut noshape_model::Document, tab: TabId, ids: &[InstanceId]) -> Vec<InstanceId> {
    use noshape_model::document::TabContent;
    let mut created = Vec::new();
    for sid in ids {
        let Some(InstanceSource::Assembly { assembly: sub_tab, .. }) = assembly_mut(doc, tab).and_then(|p| p.instance(*sid)).map(|i| i.source.clone()) else { continue };
        let Some(TabContent::Assembly(sub)) = doc.tab(sub_tab).map(|t| t.content.clone()) else { continue };
        if let Some(parent) = assembly_mut(doc, tab) {
            created.extend(parent.dissolve_subassembly(*sid, &sub));
        }
        let used_elsewhere = doc.tabs.iter().any(|t| match &t.content {
            TabContent::Assembly(a) => a.instances.iter().any(|i| matches!(i.source, InstanceSource::Assembly { assembly, .. } if assembly == sub_tab)),
            _ => false,
        });
        if !used_elsewhere && let Some(t) = doc.tab_mut(sub_tab) {
            t.content = TabContent::Assembly(Box::default());
        }
    }
    created
}

/// Paste's history entry ("Paste Part 1 <1>", "Paste 3 items").
pub fn paste_description(clip: &noshape_model::assembly::ops::InstanceClipboard) -> String {
    match clip.instances.as_slice() {
        [one] => format!("Paste {}", one.name),
        all => format!("Paste {} items", all.len()),
    }
}

pub fn best_tessellation_description(on: bool) -> &'static str {
    if on { "Use best available tessellation" } else { "Use automatic tessellation setting" }
}

pub fn set_best_tessellation(a: &mut Assembly, ids: &[InstanceId], on: bool) {
    for id in ids {
        if let Some(i) = a.instance_mut(*id) {
            i.best_tessellation = on;
        }
    }
}

pub const INSTANCE_PROPERTIES_DESCRIPTION: &str = "Instance properties";

/// Instance properties…: the Reference designator (empty clears it).
pub fn set_reference_designator(a: &mut Assembly, id: InstanceId, value: &str) {
    if let Some(i) = a.instance_mut(id) {
        i.reference_designator = Some(value.trim().to_string()).filter(|s| !s.is_empty());
    }
}

/// Lock / follow position: the history entry and the message the view shows.
pub fn lock_description(name: &str, lock: &SubassemblyLock) -> (String, String) {
    let message = match lock {
        SubassemblyLock::Flexible => "Assembly is unlocked from named position.".to_string(),
        SubassemblyLock::Current => format!("{name} locked at current position."),
        SubassemblyLock::Named(n) => format!("{name} locked at named position {n}."),
    };
    (format!("Lock/follow position : {name}"), message)
}

pub fn set_lock(a: &mut Assembly, id: InstanceId, lock: SubassemblyLock) {
    if let Some(i) = a.instance_mut(id) {
        i.lock = lock;
    }
}

/// What a Dynamic suppression expression is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynTarget {
    Instance(InstanceId),
    Mate(MateId),
    /// An instance folder (its instances).
    Folder(MateId),
}

pub const DYNAMIC_SUPPRESSION_DESCRIPTION: &str = "Dynamic suppression";

/// Set (or remove, `None`) an "Unsuppressed if" expression.
pub fn set_unsuppressed_if(a: &mut Assembly, target: DynTarget, text: Option<String>) {
    let text = text.filter(|t| !t.trim().is_empty());
    match target {
        DynTarget::Instance(id) => {
            if let Some(i) = a.instance_mut(id) {
                i.unsuppressed_if = text;
            }
        }
        DynTarget::Mate(id) => {
            if let Some(m) = a.mates.iter_mut().find(|m| m.id == id) {
                m.unsuppressed_if = text;
            }
        }
        DynTarget::Folder(id) => {
            if let Some(f) = a.folders.iter_mut().find(|f| f.id == id) {
                f.unsuppressed_if = text;
            }
        }
    }
}

pub const ADD_FOLDER_DESCRIPTION: &str = "Add selection to folder";
pub const UNPACK_FOLDER_DESCRIPTION: &str = "Unpack folder";

/// Add selection to folder… with the name typed in its popup (empty: the
/// default "New folder").
pub fn add_folder(a: &mut Assembly, instances: &[InstanceId], features: &[MateId], name: &str) -> MateId {
    let id = a.add_folder(instances, features);
    let name = name.trim();
    if !name.is_empty()
        && let Some(f) = a.folders.iter_mut().find(|f| f.id == id)
    {
        f.name = name.to_string();
    }
    id
}

pub fn change_configuration_description(name: &str) -> String {
    format!("Change configuration : {name}")
}

/// Does assembly `tab` (transitively) contain assembly `target`?
pub fn contains_assembly(doc: &noshape_model::Document, tab: TabId, target: TabId, depth: usize) -> bool {
    if tab == target || depth > 16 {
        return true;
    }
    let Some(noshape_model::document::TabContent::Assembly(a)) = doc.tab(tab).map(|t| &t.content) else { return false };
    a.instances.iter().any(|i| match &i.source {
        InstanceSource::Assembly { assembly, .. } => contains_assembly(doc, *assembly, target, depth + 1),
        _ => false,
    })
}

/// Insert a Part Studio sketch as an instance ("Insert : Sketch 1").
pub fn insert_sketch(a: &mut Assembly, studio: TabId, sketch: noshape_model::FeatureId, name: &str, config: ConfigValues) -> InstanceId {
    let id = InstanceId(a.next_id());
    let n = a.next_instance_name(name);
    a.instances.push(Instance::new(id, n, InstanceSource::Sketch { studio, sketch, config }, Placement::default()));
    id
}

// ------------------------------------------------------------ standard content

/// Where Insert puts standard content in a hole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stack {
    /// Insert: a washer or nut goes under a bolt already in the hole.
    Plain,
    /// Insert closest to selection: against the picked face, what was there
    /// moves onto the new part.
    Closest,
    /// Insert furthest from selection: at the far end of the stack.
    Furthest,
}

/// Live (behaviours "Standard content: … stacking").
pub const STACK_TOPPED: &str = "Component on top of existing stack cannot accept additional top stack components.";

fn stacks(a: &Assembly, id: InstanceId) -> bool {
    use noshape_model::assembly::standard;
    a.instance(id).and_then(|i| i.source.standard()).and_then(standard::component).is_some_and(|c| !c.shape.has_length())
}

fn same_connector(c: &MateConnectorRef, at: &MateConnectorRef) -> bool {
    c.instance == at.instance && c.path == at.path && c.reference == at.reference && c.align == at.align
}

/// The fastened mate on connector `at`: its index and whether `at` is its `b`.
fn held(a: &Assembly, at: &MateConnectorRef) -> Option<(usize, bool)> {
    a.mates.iter().position(|m| !m.suppressed && m.mate_type == MateType::Fastened && (same_connector(&m.b, at) || same_connector(&m.a, at))).map(|k| (k, same_connector(&a.mates[k].b, at)))
}

/// Insert furthest from selection: the free connector at the outer end of
/// the stack on `hole` (past every nut and washer); None when a bolt or
/// screw tops it.
fn stack_end(a: &Assembly, hole: &MateConnectorRef) -> Option<MateConnectorRef> {
    use noshape_model::assembly::standard;
    let mut at = hole.clone();
    for _ in 0..32 {
        let Some((k, at_is_b)) = held(a, &at) else { return Some(at) };
        let other = if at_is_b { a.mates[k].a.clone() } else { a.mates[k].b.clone() };
        if !stacks(a, other.instance) {
            return None;
        }
        let next = if other.reference == standard::connector_reference(0) { 1 } else { 0 };
        at = MateConnectorRef::on(other.instance, other.path.clone(), standard::connector_reference(next));
    }
    Some(at)
}

/// Insert furthest onto a stack topped by a bolt is refused.
pub fn stack_topped(a: &Assembly, holes: &[MateConnectorRef]) -> bool {
    holes.iter().any(|h| stack_end(a, h).is_none())
}

pub fn insert_standard_description(spec: &noshape_model::assembly::standard::StandardSpec) -> String {
    format!("Insert : {}", spec.part_name())
}

/// Insert standard content: one instance per hole, fastened there (or one
/// unplaced instance without holes). `flip`: the `a` key while inserting.
pub fn insert_standard(a: &mut Assembly, spec: &noshape_model::assembly::standard::StandardSpec, holes: &[MateConnectorRef], flip: bool, mode: Stack) -> Vec<InstanceId> {
    use noshape_model::assembly::standard;
    let base = spec.part_name();
    let mut ids = Vec::new();
    for _ in 0..holes.len().max(1) {
        let id = InstanceId(a.next_id());
        let name = a.next_instance_name(&base);
        a.instances.push(Instance::new(id, name, InstanceSource::Standard { spec: spec.clone() }, Placement::default()));
        ids.push(id);
    }
    for (id, hole) in ids.iter().zip(holes) {
        let mut src = MateConnectorRef::on(*id, vec![], standard::connector_reference(0));
        src.flip = flip;
        let top = MateConnectorRef::on(*id, vec![], standard::connector_reference(1));
        let fastened = |a: &mut Assembly, x: MateConnectorRef, y: MateConnectorRef| {
            let mid = MateId(a.next_id());
            let n = a.next_feature_name("Fastened");
            a.mates.push(Mate::new(mid, n, MateType::Fastened, x, y));
        };
        if mode == Stack::Furthest {
            let at = stack_end(a, hole).unwrap_or_else(|| hole.clone());
            fastened(a, src, at);
            continue;
        }
        // Closest (and Insert of a washer / nut into a hole holding a
        // bolt): the part goes against the face. Live: the face's
        // Fastened is re-pointed from what sat there to the new part,
        // and a new Fastened joins the new part's top to it.
        let under = held(a, hole).filter(|(k, at_is_b)| {
            let other = if *at_is_b { &a.mates[*k].a } else { &a.mates[*k].b };
            stacks(a, *id) && (mode == Stack::Closest || !stacks(a, other.instance))
        });
        match under {
            Some((k, at_is_b)) => {
                let m = &mut a.mates[k];
                let was = if at_is_b { std::mem::replace(&mut m.a, src) } else { std::mem::replace(&mut m.b, src) };
                fastened(a, top, was);
            }
            None => fastened(a, src, hole.clone()),
        }
    }
    ids
}

pub fn edit_standard_description(spec: &noshape_model::assembly::standard::StandardSpec) -> String {
    format!("Edit standard content : {}", spec.part_name())
}

/// Edit standard content instance: the instances take the new size /
/// length (mates stay: face names do not depend on the size).
pub fn edit_standard(a: &mut Assembly, ids: &[InstanceId], spec: &noshape_model::assembly::standard::StandardSpec) {
    let name = spec.part_name();
    for id in ids {
        let base = a.next_instance_name(&name);
        if let Some(i) = a.instance_mut(*id) {
            i.source = InstanceSource::Standard { spec: spec.clone() };
            i.name = base;
        }
    }
}
