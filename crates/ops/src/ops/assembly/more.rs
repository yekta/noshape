//! Assembly patterns (linear, circular, mirror, part pattern, replicate),
//! the Bill of Materials and Interference detection.

use super::{assembly, asm_field, commit, describe, find_instance, find_mate, parse_connector, resolve_entity, solve, target};
use crate::error::OpError;
use crate::ops::{doc_field, s};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, assembly as sa};
use noshape_model::assembly::analysis::{instance_interferences, matched_targets};
use noshape_model::assembly::bom::{self, BomColumn, BomType};
use noshape_model::assembly::eval::SolveOptions;
use noshape_model::assembly::{Assembly, InstanceId, InstancePattern, InstanceSource, LinearDirection, MateId, MirrorPlacement, PatternKind, ReplicateScope, ScopeEntity};
use noshape_model::document::{TabContent, TabId};
use noshape_units::ExprValue;
use serde_json::{Value, json};

const SCOPES: [(&str, ReplicateScope); 6] = [
    ("faces_in_parts", ReplicateScope::FacesInParts),
    ("faces_in_features", ReplicateScope::FacesInFeatures),
    ("individual_faces", ReplicateScope::IndividualFaces),
    ("edges_on_plane", ReplicateScope::EdgesOnPlane),
    ("edges_on_face", ReplicateScope::EdgesOnFace),
    ("individual_edges", ReplicateScope::IndividualEdges),
];
const KINDS: [&str; 5] = ["linear", "circular", "mirror", "part_pattern", "replicate"];

pub fn ops() -> Vec<Op> {
    let connector = json!({ "type": "object" });
    let behaviors: Vec<&str> = noshape_model::assembly::BomBehavior::ALL.iter().map(|b| b.label()).collect();
    vec![
        Op {
            group: "assembly",
            verb: "measure",
            summary: "Measure entities in an assembly (lengths, areas, radii, distances, angles)",
            description: "The Measure tool in an assembly: one entity gives its length / area / radius...; two give the minimum distance (with dx, dy, dz) and the angle between them, where the parts sit. Values in workspace units.",
            fields: vec![doc_field(), asm_field(), req("entities", json!({ "type": "array", "items": { "type": "object" } }), "Picks: {\"instance\": name, id or \"Sub <1> > Part 1 <1>\", \"entity\": a face / edge / vertex id (assembly_entities)}.")],
            mutates: false,
            destructive: false,
            run: measure,
        },
        Op {
            group: "assembly",
            verb: "mass_properties",
            summary: "Mass and section properties of an assembly or some of its instances",
            description: "Volume, surface area, and with a material (or density, or a mass override) on every part: mass, center of mass and inertia about the center of mass, as the Mass and section properties tool shows them. Parts' and subassemblies' mass overrides count; the whole assembly's own override replaces its mass (`mass_overridden`). Lengths in workspace units.",
            fields: vec![doc_field(), asm_field(), opt("instances", json!({ "type": "array", "items": { "type": "string" } }), "Top-level instances (name or id); default the whole assembly.")],
            mutates: false,
            destructive: false,
            run: mass_properties,
        },
        Op {
            group: "assembly",
            verb: "update_references",
            summary: "Update references to other documents' versions (the Reference manager)",
            description: "The toolbar's Update all references and an instance's Change to version…: every instance (or `instances`) referencing another document's version moves to its latest named version, or to `version` (the dialog's Selective update; `document` picks one referenced document by file name). This document's own parts and subassemblies: assembly_change_to_version. One edit per reference (\"Update to V2\").",
            fields: vec![
                doc_field(),
                asm_field(),
                opt("instances", json!({ "type": "array", "items": { "type": "string" } }), "Only these instances (Change to version…), names or ids."),
                opt("document", s(), "Only this referenced document (file name, with or without .noshape)."),
                opt("version", s(), "A named version to move to (default: each document's latest)."),
            ],
            mutates: true,
            destructive: false,
            run: update_references,
        },
        Op {
            group: "assembly",
            verb: "properties",
            summary: "An assembly's Properties (its top row's Properties…, Tab › Properties…)",
            description: "Without fields, replies with the assembly's properties. Name (the tab's), description, part number, revision, Subassembly BOM behavior, a mass override (kg), vendor, project, product line, titles, Not revision managed, Exclude from all BOMs, Unit of measure and custom properties; State is set by releases. Fields not given keep their value; an empty text clears it. One edit (\"Properties : <name>\"). In a version (history_view) they are the version's own: name, part_number, revision, mass_override and not_revision_managed can't be edited there; the others are kept with the version, not written into the workspace.",
            fields: vec![
                doc_field(),
                asm_field(),
                opt("name", s(), "Name (the tab's)."),
                opt("description", s(), "Description."),
                opt("part_number", s(), "Part number."),
                opt("revision", s(), "Revision."),
                opt("bom_behavior", json!({ "type": "string", "enum": behaviors }), "Subassembly BOM behavior."),
                opt("mass_override", json!({ "type": ["number", "null"] }), "Mass override in kg (null: computed)."),
            ]
            .into_iter()
            .chain(crate::ops::parts::meta_fields())
            .collect(),
            mutates: true,
            destructive: false,
            run: properties,
        },
        Op {
            group: "assembly",
            verb: "linked_part_properties",
            summary: "Properties… of an instance of another document's part (at a version)",
            description: "Without fields, replies with the part's properties at the version the instance uses. Description, part number, revision, vendor, project, product line, titles, Exclude from all BOMs, Unit of measure and custom properties are saved into that version of the other document (every instance of it at that version shows them; its workspace keeps its own); Name, Not revision managed and the mass are greyed in the dialog and can't be set here. State is set by releases.",
            fields: vec![doc_field(), asm_field(), req("instance", s(), "An instance of another document's part, id or name."), opt("description", s(), "Description."), opt("part_number", s(), "Part number."), opt("revision", s(), "Revision.")]
                .into_iter()
                .chain(crate::ops::parts::meta_fields().into_iter().filter(|f| f.name != "not_revision_managed"))
                .collect(),
            mutates: true,
            destructive: false,
            run: linked_part_properties,
        },
        Op {
            group: "assembly",
            verb: "pin_reference",
            summary: "Pin reference / Unpin reference of an instance of another document's version",
            description: "The instance menu's Pin reference: the instance stays on its version when references are updated (update_references and the Reference manager leave it), its row shows the pinned icon, and Edit in context isn't offered; `pinned: false` is Unpin reference. One edit (\"Pin reference\" / \"Unpin reference\").",
            fields: vec![doc_field(), asm_field(), req("instance", s(), "An instance of another document's version, id or name."), opt("pinned", json!({ "type": "boolean" }), "false: Unpin reference (default true).")],
            mutates: true,
            destructive: false,
            run: pin_reference,
        },
        Op {
            group: "assembly",
            verb: "pattern",
            summary: "Pattern instances: linear, circular, mirror, part pattern or replicate",
            description: "The Linear / Circular pattern, Mirror, Part pattern and Replicate dialogs. Linear copies the seeds along a direction (an edge, a face's normal or a mate connector's Z axis), optionally in a second direction; circular copies them about an axis; mirror copies them across a plane. Part pattern re-creates the seed's mate on every copy of its target made by a Part Studio pattern; replicate re-creates it on all matching geometry (every hole of the same size). Give `edit` to change an existing pattern: fields left out keep their values. One edit (\"Insert : Linear pattern 1\").",
            fields: vec![
                doc_field(),
                asm_field(),
                opt("kind", json!({ "type": "string", "enum": KINDS }), "Pattern type (required for a new pattern)."),
                opt("seeds", json!({ "type": "array", "items": { "type": "string" } }), "Instances to pattern (ids or names); replicate: the first is the matched mate's, the others follow it through their mates to it (a bolt and its washer); a part pattern uses the first."),
                opt("direction", connector.clone(), "Linear: the direction; circular: the axis; mirror: the plane. A mate connector, as in assembly_mate (an edge, a face or {\"connector\": ...})."),
                opt("distance", json!({ "type": ["string", "number"] }), "Linear: the spacing (default 25 mm); circular: the angle (default 360 deg)."),
                opt("count", json!({ "type": "integer", "minimum": 1 }), "Instance count, the seed included (default 2)."),
                opt("opposite", json!({ "type": "boolean" }), "Opposite direction."),
                opt("equal_spacing", json!({ "type": "boolean" }), "Linear: `distance` is the total; circular: spread over the angle (default true for circular)."),
                opt("second", json!({ "type": ["object", "boolean"] }), "Linear: a second direction {\"direction\": connector, \"distance\", \"count\", \"opposite\", \"equal_spacing\", \"centered\"} (distance and count default as the first's); false removes it."),
                opt("third", json!({ "type": ["object", "boolean"] }), "Linear, with a second direction: a third, as `second`; false removes it."),
                opt("centered", json!({ "type": "boolean" }), "Linear / circular: the seed in the middle, `count` instances (the seed included) each way."),
                opt("handed", json!({ "type": "boolean" }), "Mirror: every seed's Mirror action Derived (shorthand for `strategy`)."),
                opt(
                    "strategy",
                    json!({ "type": "array", "items": { "type": "object", "properties": { "instance": { "type": "string" }, "action": { "type": "string", "enum": ["transform", "derived"] }, "placement": { "type": "string", "enum": PLACEMENTS.map(|p| p.0) } }, "required": ["instance"] } }),
                    "Mirror: the Mirror strategy table, per seed {\"instance\", \"action\", \"placement\"}. `transform` (default) places a rigid copy of the part; `derived` uses its opposite-handed version from the Part Studio's \"<Part Studio>-Mirrored\" Part Studio (made when missing: one more edit, \"Create mirrored Part Studio\", in the folder \"Mirrored parts and assemblies\"). `placement` (Derived): auto (default) or the bounding box / origin plane the copy is placed by. Seeds left out keep their row (a new pattern: transform, auto).",
                ),
                opt("mate", s(), "Part pattern / replicate: the seed's mate to follow (default: the seed's only mate)."),
                opt("scope", json!({ "type": "string", "enum": SCOPES.iter().map(|x| x.0).collect::<Vec<_>>() }), "Replicate: where to match (default: faces_in_parts, or edges_on_face when the seed mate is on an edge)."),
                opt("scope_refs", json!({ "type": "array", "items": { "type": "object" } }), "Replicate: \"Parts to find match in\": [{\"instance\": id or name, \"entity\": a face / edge id (none: the whole part)}]."),
                opt("name", s(), "Pattern name (default \"Linear pattern 1\"...)."),
                opt("edit", s(), "An existing pattern to edit (id or name)."),
            ],
            mutates: true,
            destructive: false,
            run: pattern,
        },
        Op {
            group: "assembly",
            verb: "dissolve_pattern",
            summary: "Dissolve a pattern: its instances and mates become regular ones",
            description: "The pattern's context menu › Dissolve pattern. One edit (\"Dissolve pattern\").",
            fields: vec![doc_field(), asm_field(), req("pattern", s(), "Pattern id or name.")],
            mutates: true,
            destructive: false,
            run: dissolve,
        },
        Op {
            group: "assembly",
            verb: "bom",
            summary: "The Bill of Materials of an Assembly",
            description: "The BOM table with the assembly's columns and type (flattened or structured), as the BOM panel shows it; `csv` writes it the way ⋯ › Export to CSV does. `type`, `columns`, `show_excluded` and `show_top_level` override the stored table settings for this call only; assembly_bom_settings changes them.",
            fields: vec![
                doc_field(),
                asm_field(),
                opt("type", json!({ "type": "string", "enum": ["flattened", "structured"] }), "BOM type."),
                opt("columns", json!({ "type": "array", "items": { "type": "string" } }), "Columns by name (\"Item\", \"Quantity\", \"Part number\", \"Material\", \"Mass\"...)."),
                opt("show_excluded", json!({ "type": "boolean" }), "Show excluded and suppressed items."),
                opt("show_top_level", json!({ "type": "boolean" }), "Show the top level assembly row."),
                opt("csv", s(), "Write the table to this CSV file."),
                opt("copy", json!({ "type": "boolean" }), "Copy table (the BOM's ⋯ menu): `copied` is the text it puts on the clipboard (tab-separated rows), with its toast message."),
            ],
            mutates: false,
            destructive: false,
            run: bom_table,
        },
        Op {
            group: "assembly",
            verb: "bom_settings",
            summary: "Change the BOM table's type and columns",
            description: "The BOM panel's settings: BOM type, the columns (Item stays first) and the ⋯ menu's Show excluded/suppressed and Show top level assembly row. Each change is its own edit, as in the panel (\"BOM type\", \"Add column Material\", \"Move left\"...).",
            fields: vec![
                doc_field(),
                asm_field(),
                opt("type", json!({ "type": "string", "enum": ["flattened", "structured"] }), "BOM type."),
                opt("columns", json!({ "type": "array", "items": { "type": "string" } }), "The columns in order (Item is always first)."),
                opt("show_excluded", json!({ "type": "boolean" }), "Show excluded and suppressed items."),
                opt("show_top_level", json!({ "type": "boolean" }), "Show the top level assembly row."),
            ],
            mutates: true,
            destructive: false,
            run: bom_settings,
        },
        Op {
            group: "assembly",
            verb: "interference",
            summary: "Interference detection: overlaps between instances",
            description: "Analysis tools › Interference detection: the overlapping solid parts among the given top-level instances (default: all), with the overlap's volume and size. Parts of one Part Studio instance aren't checked against each other.",
            fields: vec![
                doc_field(),
                asm_field(),
                opt("instances", json!({ "type": "array", "items": { "type": "string" } }), "Top-level instances (ids or names); default all."),
                opt("include_standard", json!({ "type": "boolean" }), "Include standard content (fasteners)."),
                opt("top_only", json!({ "type": "boolean" }), "Show top level only: one row per pair of top-level instances."),
            ],
            mutates: false,
            destructive: false,
            run: interference,
        },
        Op {
            group: "assembly",
            verb: "change_to_version",
            summary: "Change to version: instances show a version of this document",
            description: "Instance › Change to version… (the Reference manager): the `instances` (default all) that reference this document's workspace (parts, Part Studios, subassemblies) show version `version` instead (\"Change to version : V1\"); instances already showing a version of this document move to `version` (\"Update to V1\"). Versions: history_create_version. Sketch, mirrored and standard content instances are left alone.",
            fields: vec![doc_field(), asm_field(), req("version", s(), "Version name."), opt("instances", json!({ "type": "array", "items": { "type": "string" } }), "Instance ids or names (default all).")],
            mutates: true,
            destructive: false,
            run: change_to_version,
        },
    ]
}

fn kind_id(k: &PatternKind) -> &'static str {
    match k {
        PatternKind::Linear { .. } => "linear",
        PatternKind::Circular { .. } => "circular",
        PatternKind::Mirror { .. } => "mirror",
        PatternKind::PartPattern { .. } => "part_pattern",
        PatternKind::Replicate { .. } => "replicate",
    }
}

fn find_pattern(a: &Assembly, key: &str) -> Result<MateId, OpError> {
    let k = key.trim();
    let by_id = k.strip_prefix("pattern:").or_else(|| k.strip_prefix("mate:")).unwrap_or(k).parse::<u64>().ok().map(MateId);
    a.patterns
        .iter()
        .find(|p| Some(p.id) == by_id || p.name == k || p.name.eq_ignore_ascii_case(k))
        .map(|p| p.id)
        .ok_or_else(|| OpError::not_found(format!("no pattern `{key}`; patterns: {}", a.patterns.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "))).with_param("edit"))
}

/// A pattern, group or mate connector by id ("pattern:7") or name (for
/// assembly_delete).
pub(super) fn find_feature(a: &Assembly, k: &str) -> Option<MateId> {
    let by_id = k.split_once(':').map_or(k, |(_, n)| n).parse::<u64>().ok().map(MateId);
    let hit = |id: MateId, name: &str| Some(id) == by_id || name == k;
    a.patterns.iter().find(|p| hit(p.id, &p.name)).map(|p| p.id).or_else(|| a.groups.iter().find(|g| hit(g.id, &g.name)).map(|g| g.id)).or_else(|| a.connectors.iter().find(|c| hit(c.id, &c.name)).map(|c| c.id))
}

/// The patterns of an assembly, for assembly_list.
pub(super) fn patterns_json(a: &Assembly) -> Vec<Value> {
    a.patterns
        .iter()
        .map(|p| {
            let mut v = json!({
                "id": format!("pattern:{}", p.id.0),
                "name": p.name,
                "kind": kind_id(&p.kind),
                "seeds": p.seeds.iter().map(|i| format!("inst:{}", i.0)).collect::<Vec<_>>(),
                "instances": p.generated.iter().map(|g| format!("inst:{}", g.instance.0)).collect::<Vec<_>>(),
                "suppressed": p.suppressed,
            });
            // The spacing and count as assembly_pattern takes them.
            match &p.kind {
                PatternKind::Linear { distance, count, .. } => (v["distance"], v["count"]) = (json!(distance.text), json!(count)),
                PatternKind::Circular { angle, count, .. } => (v["distance"], v["count"]) = (json!(angle.text), json!(count)),
                PatternKind::Mirror { .. } | PatternKind::PartPattern { .. } | PatternKind::Replicate { .. } => {}
            }
            if let PatternKind::Mirror { handed, derived, placement, .. } = &p.kind {
                v["strategy"] = json!(p
                    .seeds
                    .iter()
                    .map(|id| {
                        let place = placement.iter().find(|(s, _)| s == id).map_or(MirrorPlacement::Auto, |(_, p)| *p);
                        json!({
                            "instance": format!("inst:{}", id.0),
                            "action": if *handed || derived.contains(id) { "derived" } else { "transform" },
                            "placement": PLACEMENTS.iter().find(|x| x.1 == place).map_or("auto", |x| x.0),
                        })
                    })
                    .collect::<Vec<_>>());
            }
            v
        })
        .collect()
}

fn text_arg(a: &Args, key: &str) -> Result<Option<String>, OpError> {
    Ok(match a.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(t)) => Some(t.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(_) => return Err(OpError::param(key, "an expression (\"50 mm\") or a number")),
    })
}

fn count_arg(v: Option<&Value>, key: &str) -> Result<Option<u32>, OpError> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().map(|n| Some(n.round().clamp(1.0, 2500.0) as u32)).ok_or_else(|| OpError::param(key, "Instance count must be a whole number")),
    }
}

/// The Mirror strategy's Placement type column.
const PLACEMENTS: [(&str, MirrorPlacement); 7] = [
    ("auto", MirrorPlacement::Auto),
    ("bounding_box_xy", MirrorPlacement::BoxXY),
    ("bounding_box_yz", MirrorPlacement::BoxYZ),
    ("bounding_box_zx", MirrorPlacement::BoxZX),
    ("origin_xy", MirrorPlacement::OriginXY),
    ("origin_yz", MirrorPlacement::OriginYZ),
    ("origin_zx", MirrorPlacement::OriginZX),
];

fn pattern(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let asm = assembly(od, tab)?.clone();
    let before = solve(od, tab, &SolveOptions::default())?;
    let old: Option<InstancePattern> = match a.opt_str("edit")? {
        Some(k) => asm.pattern(find_pattern(&asm, k)?).cloned(),
        None => None,
    };
    let kind = match (a.opt_str("kind")?, &old) {
        (Some(k), _) => KINDS.iter().find(|x| **x == k.to_lowercase().replace([' ', '-'], "_")).copied().ok_or_else(|| OpError::param("kind", "linear, circular, mirror, part_pattern or replicate"))?,
        (None, Some(p)) => kind_id(&p.kind),
        (None, None) => return Err(OpError::param("kind", "required for a new pattern")),
    };
    let seeds: Vec<InstanceId> = match a.get("seeds") {
        Some(Value::Array(v)) => v.iter().enumerate().map(|(i, x)| find_instance(&asm, x.as_str().unwrap_or_default(), &format!("seeds[{i}]"))).collect::<Result<_, _>>()?,
        _ => old.as_ref().map(|p| p.seeds.clone()).unwrap_or_default(),
    };
    // A generated instance can't be a seed (the dialog drops them).
    let seeds: Vec<InstanceId> = seeds.into_iter().filter(|id| asm.generated_by(*id).is_none() || old.as_ref().is_some_and(|p| p.seeds.contains(id))).collect();
    if seeds.is_empty() {
        return Err(OpError::param("seeds", "Select the instances to pattern (seed)"));
    }
    let direction = match a.get("direction") {
        Some(v) if !v.is_null() => Some(resolve_entity(&before, parse_connector(&asm, od, tab, v, "direction")?, &units)),
        _ => None,
    };
    // The old pattern's values, for fields left out.
    let mut o = Old::default();
    match old.as_ref().map(|p| &p.kind) {
        Some(PatternKind::Linear { direction, distance, count, opposite, equal_spacing, second, centered, third }) => {
            o = Old { dir: Some(direction.clone()), dist: Some(distance.text.clone()), count: Some(*count), opposite: *opposite, equal: Some(*equal_spacing), second: second.clone(), third: third.clone(), centered: *centered, ..o };
        }
        Some(PatternKind::Circular { axis, angle, count, opposite, equal_spacing, centered }) => {
            o = Old { dir: Some(axis.clone()), dist: Some(angle.text.clone()), count: Some(*count), opposite: *opposite, equal: Some(*equal_spacing), centered: *centered, ..o };
        }
        Some(PatternKind::Mirror { plane, handed, derived, placement, .. }) => o = Old { dir: Some(plane.clone()), handed: *handed, derived: derived.clone(), placement: placement.clone(), ..o },
        Some(PatternKind::PartPattern { mate }) => o.mate = Some(*mate),
        Some(PatternKind::Replicate { mate, scope, scope_refs, .. }) => o = Old { mate: Some(*mate), scope: *scope, scope_refs: scope_refs.clone(), ..o },
        None => {}
    }
    let Old { dir: o_dir, dist: o_dist, count: o_count, opposite: o_opp, equal: o_equal, second: o_second, third: o_third, centered: o_centered, handed: o_handed, derived: o_derived, placement: o_placement, mate: o_mate, scope: o_scope, scope_refs: o_refs } = o;
    let same_kind = old.as_ref().is_some_and(|p| kind_id(&p.kind) == kind);
    let keep = |o: bool| same_kind && o;
    let (d_dist, d_count) = sa::pattern_defaults(kind == "circular");
    let distance = ExprValue::new(text_arg(a, "distance")?.or(if same_kind { o_dist } else { None }).unwrap_or_else(|| d_dist.into()));
    let count = count_arg(a.get("count"), "count")?.or(if same_kind { o_count } else { None }).unwrap_or_else(|| d_count.parse().unwrap_or(2));
    let opposite = a.get("opposite").and_then(Value::as_bool).unwrap_or(keep(o_opp));
    let equal = a.get("equal_spacing").and_then(Value::as_bool).or(if same_kind { o_equal } else { None }).unwrap_or(kind == "circular");
    let axis = direction.or(if same_kind { o_dir } else { None });
    let centered = a.get("centered").and_then(Value::as_bool).unwrap_or(keep(o_centered));
    let mate = || -> Result<MateId, OpError> {
        let m = match a.opt_str("mate")? {
            Some(k) => Some(find_mate(&asm, k).map_err(|e| e.with_param("mate"))?),
            None if same_kind => o_mate,
            None => None,
        };
        m.or_else(|| sa::seed_mate(&asm, seeds[0], None)).ok_or_else(|| OpError::param("mate", if kind == "part_pattern" { "The seed needs exactly one mate to follow (or select the mate in the list)" } else { "The seed needs exactly one mate to match (or select the mate in the list)" }))
    };
    let pk = match kind {
        "linear" => {
            // A further direction: given, removed (null), else as it was. Live 2204: its distance and count default as
            // the first direction's.
            let further = |key: &str, was: Option<LinearDirection>| -> Result<Option<LinearDirection>, OpError> {
                let o = match a.get(key) {
                    Some(Value::Null | Value::Bool(false)) => return Ok(None),
                    Some(Value::Object(o)) => o,
                    Some(_) => return Err(OpError::param(key, "a direction object, or false to remove it")),
                    None => return Ok(if same_kind { was } else { None }),
                };
                let d = o.get("direction").ok_or_else(|| OpError::param(&format!("{key}.direction"), "Select a direction (edge, face or mate connector)"))?;
                let direction = resolve_entity(&before, parse_connector(&asm, od, tab, d, &format!("{key}.direction"))?, &units);
                let distance = match o.get("distance") {
                    Some(Value::String(t)) => t.clone(),
                    Some(Value::Number(n)) => n.to_string(),
                    _ => d_dist.to_string(),
                };
                let flag = |k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
                let count = count_arg(o.get("count"), &format!("{key}.count"))?.unwrap_or_else(|| d_count.parse().unwrap_or(2));
                Ok(Some(LinearDirection { direction, distance: ExprValue::new(distance), count, opposite: flag("opposite"), equal_spacing: flag("equal_spacing"), centered: flag("centered") }))
            };
            let second = further("second", o_second)?;
            let third = further("third", o_third.map(|t| *t))?.map(Box::new);
            if third.is_some() && second.is_none() {
                return Err(OpError::param("third", "a third direction needs a second one"));
            }
            PatternKind::Linear { direction: axis.ok_or_else(|| OpError::param("direction", "Select a direction (edge, face or mate connector)"))?, distance, count, opposite, equal_spacing: equal, second, centered, third }
        }
        "circular" => PatternKind::Circular { axis: axis.ok_or_else(|| OpError::param("direction", "Select the axis of pattern"))?, angle: distance, count, opposite, equal_spacing: equal, centered },
        "mirror" => {
            let plane = axis.ok_or_else(|| OpError::param("direction", "Select the mirror plane (planar face or mate connector)"))?;
            let (mut derived, mut placement) = if same_kind { (o_derived, o_placement) } else { (Vec::new(), Vec::new()) };
            let mut handed = keep(o_handed);
            if let Some(h) = a.get("handed").and_then(Value::as_bool) {
                // The dialog writes per-seed rows; `handed` stays for older documents.
                handed = false;
                derived = if h { seeds.clone() } else { Vec::new() };
            }
            for (i, row) in a.opt_array("strategy")?.into_iter().flatten().enumerate() {
                let f = format!("strategy[{i}]");
                let id = find_instance(&asm, row.get("instance").and_then(Value::as_str).unwrap_or_default(), &format!("{f}.instance"))?;
                if !seeds.contains(&id) {
                    return Err(OpError::param(&format!("{f}.instance"), "not a seed of this mirror"));
                }
                if handed {
                    handed = false;
                    derived = seeds.clone();
                }
                match row.get("action").and_then(Value::as_str) {
                    Some("derived") if !derived.contains(&id) => derived.push(id),
                    Some("transform") => derived.retain(|d| *d != id),
                    Some("derived") | None => {}
                    Some(_) => return Err(OpError::param(&format!("{f}.action"), "transform or derived")),
                }
                if let Some(p) = row.get("placement") {
                    let p = PLACEMENTS.iter().find(|x| Some(x.0) == p.as_str()).map(|x| x.1).ok_or_else(|| OpError::param(&format!("{f}.placement"), &PLACEMENTS.map(|p| p.0).join(", ")))?;
                    placement.retain(|(s, _)| *s != id);
                    if p != MirrorPlacement::Auto {
                        placement.push((id, p));
                    }
                }
            }
            derived.retain(|d| seeds.contains(d));
            placement.retain(|(d, _)| seeds.contains(d));
            // Update: the Derived parts' "-Mirrored" Part Studios.
            let missing: Vec<TabId> = sa::derived_seed_studios(&asm, &derived).into_iter().filter(|s| od.doc().mirrored_studio(*s).is_none()).collect();
            if !missing.is_empty() {
                od.session.edit_document(sa::CREATE_MIRRORED_STUDIO, |d| {
                    for s in &missing {
                        d.ensure_mirrored_studio(*s);
                    }
                })?;
            }
            let mirrored_studios = sa::mirrored_studios(od.doc(), &asm, &derived);
            PatternKind::Mirror { plane, handed, derived, placement, mirrored_studios }
        }
        "part_pattern" => PatternKind::PartPattern { mate: mate()? },
        _ => {
            let mate = mate()?;
            let scope = match a.opt_str("scope")? {
                Some(k) => SCOPES.iter().find(|x| x.0 == k).map(|x| x.1).ok_or_else(|| OpError::param("scope", "faces_in_parts, faces_in_features, individual_faces, edges_on_plane, edges_on_face or individual_edges"))?,
                None => o_scope.filter(|_| same_kind).unwrap_or_else(|| sa::default_replicate_scope(&asm, seeds[0], mate)),
            };
            let scope_refs = match a.opt_array("scope_refs")? {
                Some(list) => list
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let f = format!("scope_refs[{i}]");
                        let inst = find_instance(&asm, v.get("instance").and_then(Value::as_str).unwrap_or_default(), &format!("{f}.instance"))?;
                        let entity = match v.get("entity") {
                            Some(e) if !e.is_null() => Some(crate::refs::parse(e).map_err(|e| e.with_param(format!("{f}.entity")))?),
                            _ => None,
                        };
                        Ok(ScopeEntity { instance: inst, path: Vec::new(), entity })
                    })
                    .collect::<Result<Vec<_>, OpError>>()?,
                None if same_kind => o_refs,
                None => Vec::new(),
            };
            // The dialog writes the scope; `all_instances` is for older documents.
            PatternKind::Replicate { mate, all_instances: false, scope: Some(scope), scope_refs }
        }
    };
    let matched = matches!(pk, PatternKind::PartPattern { .. } | PatternKind::Replicate { .. });
    let editing = old.is_some();
    let id = old.as_ref().map(|p| p.id).unwrap_or_else(|| MateId(asm.next_id()));
    let title = match kind {
        "linear" => "Linear pattern",
        "circular" => "Circular pattern",
        "mirror" => "Mirror",
        "part_pattern" => "Part pattern",
        _ => "Replicate",
    };
    let name = match a.opt_str("name")? {
        Some(n) => n.trim().to_string(),
        None => old.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| asm.next_feature_name(title)),
    };
    let generated = old.as_ref().map(|p| p.generated.clone()).unwrap_or_default();
    let seeds = if kind == "part_pattern" { seeds[..1].to_vec() } else { seeds };
    let pat = InstancePattern { id, name: name.clone(), kind: pk, seeds, generated, suppressed: old.as_ref().is_some_and(|p| p.suppressed) };
    let targets = if matched { Some(matched_targets(&before, &asm, &pat, &units).map_err(|e| OpError::param("mate", &e))?) } else { None };
    let created = commit(od, tab, &sa::pattern_description(&name, editing), &before, |a| sa::commit_pattern(a, pat, targets))?;
    let mut v = describe(od, tab)?;
    v["pattern"] = json!(format!("pattern:{}", id.0));
    v["name"] = json!(name);
    v["created"] = json!(created.iter().map(|i| format!("inst:{}", i.0)).collect::<Vec<_>>());
    v["message"] = json!(format!("{} instance(s) created.", created.len()));
    Ok(v.into())
}

fn dissolve(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let id = find_pattern(assembly(od, tab)?, a.str("pattern")?).map_err(|e| e.with_param("pattern"))?;
    let result = solve(od, tab, &SolveOptions::default())?;
    commit(od, tab, "Dissolve pattern", &result, |a| a.dissolve_pattern(id))?;
    Ok(describe(od, tab)?.into())
}

fn column(name: &str, field: &str) -> Result<BomColumn, OpError> {
    let n = name.trim().to_lowercase().replace(['_', '-'], " ");
    BomColumn::DEFAULT
        .into_iter()
        .chain(BomColumn::ADDABLE)
        .find(|c| c.label().to_lowercase() == n || format!("{c:?}").to_lowercase() == n.replace(' ', ""))
        .ok_or_else(|| OpError::param(field, &format!("unknown column; columns: {}", BomColumn::DEFAULT.into_iter().chain(BomColumn::ADDABLE).map(|c| c.label()).collect::<Vec<_>>().join(", "))))
}

/// The stored settings with this call's overrides.
fn settings_from(a: &Args, mut s: bom::BomSettings) -> Result<bom::BomSettings, OpError> {
    if let Some(t) = a.opt_str("type")? {
        s.bom_type = BomType::ALL.into_iter().find(|b| b.label().eq_ignore_ascii_case(t)).ok_or_else(|| OpError::param("type", "flattened or structured"))?;
    }
    if let Some(cols) = a.get("columns").and_then(Value::as_array) {
        let mut out = vec![BomColumn::Item];
        for (i, c) in cols.iter().enumerate() {
            let c = column(c.as_str().unwrap_or_default(), &format!("columns[{i}]"))?;
            if !out.contains(&c) {
                out.push(c);
            }
        }
        s.columns = out;
    }
    if let Some(b) = a.get("show_excluded").and_then(Value::as_bool) {
        s.show_excluded = b;
    }
    if let Some(b) = a.get("show_top_level").and_then(Value::as_bool) {
        s.show_top_level = b;
    }
    Ok(s)
}

fn bom_table(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let settings = settings_from(a, assembly(od, tab)?.bom.clone())?;
    let result = solve(od, tab, &SolveOptions::default())?;
    let d = od.doc();
    let lines = bom::lines(d, tab, &result, &settings);
    let rows: Vec<Value> = lines
        .iter()
        .map(|l| {
            let cells: serde_json::Map<String, Value> = settings.columns.iter().map(|c| (c.label().to_string(), json!(bom::cell(d, &result, l, *c).text()))).collect();
            json!({ "item": l.item, "depth": l.depth, "name": l.name, "quantity": l.quantity, "excluded": l.excluded, "cells": cells })
        })
        .collect();
    let mut out = json!({ "type": settings.bom_type.label(), "columns": settings.columns.iter().map(|c| c.label()).collect::<Vec<_>>(), "rows": rows });
    if a.bool_or("copy", false)? {
        out["copied"] = json!(bom::table_text(d, &result, &lines, &settings, false));
        out["message"] = json!(format!("{} BOM table copied to clipboard.", settings.bom_type.label()));
    }
    if let Some(path) = a.opt_str("csv")? {
        let text = bom::table_text(d, &result, &lines, &settings, true);
        std::fs::write(path, text).map_err(|e| OpError::param("csv", &e.to_string()))?;
        out["csv"] = json!(path);
    }
    Ok(out.into())
}

/// The BOM panel's edits that turn `from` into `to`, each with the panel's
/// history text: BOM type, the ⋯ toggles, then Remove column / Add column
/// and Move left until the columns are in order.
fn bom_steps(from: &bom::BomSettings, to: &bom::BomSettings) -> Vec<(String, bom::BomSettings)> {
    let mut cur = from.clone();
    let mut out = Vec::new();
    let mut step = |cur: &mut bom::BomSettings, desc: String, f: &dyn Fn(&mut bom::BomSettings)| {
        f(cur);
        out.push((desc, cur.clone()));
    };
    if cur.bom_type != to.bom_type {
        step(&mut cur, "BOM type".into(), &|b| b.bom_type = to.bom_type);
    }
    if cur.show_excluded != to.show_excluded {
        step(&mut cur, "Show excluded/suppressed".into(), &|b| b.show_excluded = !b.show_excluded);
    }
    if cur.show_top_level != to.show_top_level {
        step(&mut cur, "Show top level assembly row".into(), &|b| b.show_top_level = !b.show_top_level);
    }
    for c in from.columns.iter().filter(|c| !c.frozen() && !to.columns.contains(c)) {
        step(&mut cur, "Remove column".into(), &|b| b.remove_column(*c));
    }
    for c in to.columns.iter().filter(|c| !from.columns.contains(c)) {
        step(&mut cur, format!("Add column {}", c.label()), &|b| b.add_column(*c));
    }
    for (i, c) in to.columns.iter().enumerate() {
        while cur.columns.iter().position(|x| x == c).is_some_and(|j| j > i) && cur.can_move(*c, -1) {
            step(&mut cur, "Move left".into(), &|b| b.move_column(*c, -1));
        }
    }
    out
}

fn bom_settings(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let from = assembly(od, tab)?.bom.clone();
    let to = settings_from(a, from.clone())?;
    let steps = bom_steps(&from, &to);
    for (desc, s) in &steps {
        let result = solve(od, tab, &SolveOptions::default())?;
        let s = s.clone();
        commit(od, tab, desc, &result, move |a| a.bom = s)?;
    }
    let now = assembly(od, tab)?.bom.clone();
    Ok(json!({ "type": now.bom_type.label(), "columns": now.columns.iter().map(|c| c.label()).collect::<Vec<_>>(), "show_excluded": now.show_excluded, "show_top_level": now.show_top_level, "edits": steps.len() }).into())
}

fn interference(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = crate::ops::query::len_scale(&units);
    let asm = assembly(od, tab)?.clone();
    let ids: Vec<InstanceId> = match a.get("instances").and_then(Value::as_array) {
        Some(v) => v.iter().enumerate().map(|(i, x)| find_instance(&asm, x.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<_, _>>()?,
        None => asm.instances.iter().map(|i| i.id).collect(),
    };
    let result = solve(od, tab, &SolveOptions::default())?;
    let rows = if ids.is_empty() { Vec::new() } else { instance_interferences(&result, &ids, a.bool_or("include_standard", false)?) };
    let shown = sa::interference_rows(&result, &rows, a.bool_or("top_only", false)?, &units);
    let out: Vec<Value> = shown
        .iter()
        .map(|(i, label, size)| {
            let r = &rows[*i];
            json!({ "label": label, "size": size, "volume": r.overlap.volume * k * k * k, "a": r.a.iter().map(|x| format!("inst:{}", x.0)).collect::<Vec<_>>(), "b": r.b.iter().map(|x| format!("inst:{}", x.0)).collect::<Vec<_>>() })
        })
        .collect();
    Ok(json!({ "units": crate::ops::query::units_json(&units), "interferences": out, "count": out.len() }).into())
}

/// An edited pattern's values (for the fields a call leaves out).
#[derive(Default)]
struct Old {
    dir: Option<noshape_model::assembly::MateConnectorRef>,
    dist: Option<String>,
    count: Option<u32>,
    opposite: bool,
    equal: Option<bool>,
    second: Option<LinearDirection>,
    third: Option<Box<LinearDirection>>,
    centered: bool,
    handed: bool,
    derived: Vec<InstanceId>,
    placement: Vec<(InstanceId, MirrorPlacement)>,
    mate: Option<MateId>,
    scope: Option<ReplicateScope>,
    scope_refs: Vec<ScopeEntity>,
}

fn change_to_version(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use crate::derived::ExternalRef;
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let name = a.str("version")?;
    let versions = od.session.store.list_versions().map_err(|e| OpError::new("store", e.to_string()))?;
    let v = versions.iter().find(|v| v.name == name).ok_or_else(|| OpError::param("version", &format!("this document's versions: {}", if versions.is_empty() { "none (history_create_version)".to_string() } else { versions.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(", ") })))?;
    let scope: Option<Vec<InstanceId>> = match a.opt_array("instances")? {
        Some(list) => Some(list.iter().enumerate().map(|(i, x)| find_instance(&asm, x.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<_, _>>()?),
        None => None,
    };
    let in_scope = |i: &noshape_model::assembly::Instance| scope.as_ref().is_none_or(|s| s.contains(&i.id));
    let path = od.session.path.clone();
    let result = solve(od, tab, &SolveOptions::default())?;
    let own_version = |key: &str| ExternalRef::parse(key).filter(|r| r.path == path);
    // Workspace references of this document become version references.
    let workspace = asm.instances.iter().filter(|i| in_scope(i)).any(|i| matches!(i.source, InstanceSource::Part { .. } | InstanceSource::PartStudio { .. } | InstanceSource::Assembly { .. }));
    let mut changed = 0usize;
    if workspace {
        changed += commit(od, tab, &format!("Change to version : {}", v.name), &result, |x| sa::local_to_version(x, &path, v.microversion, scope.as_deref()))?;
    }
    // References to another version of this document: to this one.
    let versioned = asm.instances.iter().filter(|i| in_scope(i)).any(|i| matches!(&i.source, InstanceSource::External { key, .. } if own_version(key).is_some_and(|r| r.microversion != v.microversion)));
    if versioned {
        let result = solve(od, tab, &SolveOptions::default())?;
        changed += commit(od, tab, &format!("Update to {}", v.name), &result, |x| {
            let mut n = 0;
            for i in x.instances.iter_mut().filter(|i| in_scope(i)) {
                if let InstanceSource::External { key, .. } = &mut i.source
                    && let Some(r) = own_version(key).filter(|r| r.microversion != v.microversion)
                {
                    *key = ExternalRef { microversion: v.microversion, ..r }.key();
                    n += 1;
                }
            }
            n
        })?;
    }
    if changed == 0 {
        return Err(OpError::invalid("no instance references this document's workspace or another of its versions"));
    }
    let mut out = describe(od, tab)?;
    out["changed"] = json!(changed);
    Ok(out.into())
}

fn properties_json(name: &str, p: &noshape_model::assembly::AssemblyProperties) -> Value {
    json!({
        "name": name,
        "description": p.description, "part_number": p.part_number, "revision": p.revision,
        "state": p.meta.state(), "vendor": p.meta.vendor, "project": p.meta.project, "product_line": p.meta.product_line,
        "title1": p.meta.title1, "title2": p.meta.title2, "title3": p.meta.title3,
        "not_revision_managed": p.meta.not_revision_managed, "exclude_from_bom": p.meta.exclude_from_bom,
        "unit_of_measure": p.meta.unit_of_measure.clone().unwrap_or_else(|| noshape_model::configured_properties::UNITS_OF_MEASURE[0].into()),
        "bom_behavior": p.bom_behavior.label(), "mass_override": p.mass_override, "custom": p.meta.custom,
    })
}

fn properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let mut p = (*assembly(od, tab)?.properties).clone();
    let mut name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    let before = (name.clone(), p.clone());
    let text = |k: &str| -> Result<Option<String>, OpError> { Ok(a.opt_str(k)?.map(|x| x.trim().to_string())) };
    if let Some(x) = text("name")? {
        if x.is_empty() {
            return Err(OpError::param("name", "an assembly needs a name"));
        }
        name = x;
    }
    for (k, f) in [("description", &mut p.description), ("part_number", &mut p.part_number), ("revision", &mut p.revision)] {
        if let Some(x) = text(k)? {
            *f = Some(x).filter(|s| !s.is_empty());
        }
    }
    if let Some(b) = a.opt_str("bom_behavior")? {
        let all = noshape_model::assembly::BomBehavior::ALL;
        p.bom_behavior = *all.iter().find(|x| x.label().eq_ignore_ascii_case(b.trim())).ok_or_else(|| OpError::param("bom_behavior", &all.iter().map(|x| x.label()).collect::<Vec<_>>().join(", ")))?;
    }
    match a.get("mass_override") {
        None => {}
        Some(Value::Null) => p.mass_override = None,
        Some(x) => p.mass_override = Some(x.as_f64().filter(|m| *m > 0.0).ok_or_else(|| OpError::param("mass_override", "a mass in kg greater than zero, or null"))?),
    }
    crate::ops::parts::apply_meta(a, &mut p.meta)?;
    // In a version (live feel/release.json 12_studios_drawing_assembly_properties): its own properties; the ones a
    // release sets, the name and the mass override stay.
    if od.session.viewing.is_some() {
        let locked = [("name", "Name"), ("part_number", "Part number"), ("revision", "Revision"), ("mass_override", "Mass override"), ("not_revision_managed", "Not revision managed")];
        if let Some((k, f)) = locked.iter().find(|(k, _)| a.get(k).is_some()) {
            return Err(OpError::param(k, &format!("{f} can't be edited in a version")));
        }
        if p != before.1 {
            crate::version_props::set_assembly_values(&mut od.session, tab, p.clone()).map_err(|e| OpError::new("io", e.to_string()))?;
        }
        return Ok(json!({ "properties": properties_json(&name, &p) }).into());
    }
    if (name.clone(), p.clone()) != before {
        let label = format!("Properties : {name}");
        od.session.edit_document(&label, |d| {
            if let Some(t) = d.tab_mut(tab) {
                t.name = name.clone();
                if let TabContent::Assembly(asm) = &mut t.content {
                    *asm.properties = p.clone();
                }
            }
        })?;
    }
    Ok(json!({ "properties": properties_json(&name, &p) }).into())
}

fn linked_part_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let id = find_instance(asm, a.str("instance")?, "instance")?;
    let inst = asm.instance(id).ok_or_else(|| OpError::param("instance", "no such instance"))?;
    let (InstanceSource::External { key, body: Some(body), .. }, name) = (&inst.source, inst.name.clone()) else {
        return Err(OpError::param("instance", "not an instance of another document's part"));
    };
    let r = crate::derived::ExternalRef::parse(key).ok_or_else(|| OpError::param("instance", "its reference can't be read"))?;
    let snap = crate::derived::external_snapshot(&r.path, r.microversion).map_err(|e| OpError::param("instance", &format!("the other document can't be read: {e:#}")))?;
    let before = match snap.tabs.iter().find(|t| t.id.to_string() == r.tab).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => ps.part_properties.get(body).cloned().unwrap_or_default(),
        _ => return Err(OpError::param("instance", "its Part Studio is not in that version")),
    };
    let mut p = before.clone();
    let text = |k: &str| -> Result<Option<String>, OpError> { Ok(a.opt_str(k)?.map(|x| x.trim().to_string())) };
    for (k, f) in [("description", &mut p.description), ("part_number", &mut p.part_number), ("revision", &mut p.revision)] {
        if let Some(x) = text(k)? {
            *f = Some(x).filter(|s| !s.is_empty());
        }
    }
    crate::ops::parts::apply_meta(a, &mut p.meta)?;
    if p != before {
        crate::derived::save_version_part_properties(&r, body, p.clone()).map_err(|e| OpError::invalid(format!("the other document can't be written: {e:#}")))?;
    }
    let part = name.rsplit_once(" <").map(|(b, _)| b.to_string()).unwrap_or(name);
    Ok(json!({ "instance": format!("inst:{}", id.0), "properties": json!({
        "name": p.name.clone().unwrap_or(part),
        "description": p.description, "part_number": p.part_number, "revision": p.revision,
        "state": p.meta.state(), "vendor": p.meta.vendor, "project": p.meta.project, "product_line": p.meta.product_line,
        "title1": p.meta.title1, "title2": p.meta.title2, "title3": p.meta.title3,
        "not_revision_managed": p.meta.not_revision_managed, "exclude_from_bom": p.meta.exclude_from_bom,
        "unit_of_measure": p.meta.unit_of_measure.clone().unwrap_or_else(|| noshape_model::configured_properties::UNITS_OF_MEASURE[0].into()),
        "mass_override": p.mass_override, "custom": p.meta.custom,
    }) }).into())
}

fn pin_reference(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let id = find_instance(asm, a.str("instance")?, "instance")?;
    if asm.instance(id).and_then(|i| i.source.external_key()).is_none() {
        return Err(OpError::param("instance", "not an instance of another document's version"));
    }
    let on = a.bool_or("pinned", true)?;
    if asm.pinned_references.contains(&id) != on {
        let r = solve(od, tab, &SolveOptions::default())?;
        commit(od, tab, sa::pin_reference_description(on), &r, |x| sa::set_reference_pinned(x, id, on))?;
    }
    Ok(json!({ "instance": format!("inst:{}", id.0), "pinned": on }).into())
}

fn measure(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = crate::ops::query::len_scale(&units);
    let asm = assembly(od, tab)?.clone();
    let mut keys = Vec::new();
    for (i, e) in a.array("entities")?.iter().enumerate() {
        let field = format!("entities[{i}]");
        let inst = e.get("instance").and_then(Value::as_str).ok_or_else(|| OpError::param(&format!("{field}.instance"), "an instance name or id"))?;
        let path = super::find_occurrence(od.doc(), &asm, inst, &format!("{field}.instance"))?;
        let r = crate::refs::parse(e.get("entity").ok_or_else(|| OpError::param(&format!("{field}.entity"), "a face, edge or vertex id"))?).map_err(|err| err.with_param(format!("{field}.entity")))?;
        keys.push((field, path, r));
    }
    if keys.is_empty() {
        return Err(OpError::param("entities", "pick one or more entities"));
    }
    let result = solve(od, tab, &SolveOptions::default())?;
    let mut picks = Vec::new();
    for (field, path, r) in keys {
        // The part under that instance that has the entity.
        let o = result.occurrences.iter().find(|o| o.path.starts_with(&path) && noshape_model::assembly::analysis::pick_exists(&result, &(o.path.clone(), r.clone()))).ok_or_else(|| OpError::param(&format!("{field}.entity"), "not an entity of that instance"))?;
        picks.push((o.path.clone(), r));
    }
    let out: Vec<Value> = noshape_model::assembly::analysis::measure_entities(&result, &picks).into_iter().map(|(label, m)| crate::ops::query::measurement_json(label, m, k)).collect();
    Ok(json!({ "units": crate::ops::query::units_json(&units), "measurements": out }).into())
}

fn mass_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use crate::ops::query::{pt, r6};
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = crate::ops::query::len_scale(&units);
    let asm = assembly(od, tab)?.clone();
    let tops: Vec<InstanceId> = match a.opt_array("instances")? {
        Some(list) => list.iter().enumerate().map(|(i, v)| find_instance(&asm, v.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    let result = solve(od, tab, &SolveOptions::default())?;
    let densities = noshape_model::assembly::analysis::part_densities(od.doc(), &asm, &result);
    let density = |o: &noshape_model::assembly::eval::PartOccurrence| densities.get(&(o.path.clone(), o.body.clone())).copied().flatten();
    let m = noshape_model::assembly::analysis::assembly_mass_properties(&result, &tops, &density).ok_or_else(|| OpError::invalid("nothing to measure: no solid parts"))?;
    let mut v = json!({ "units": crate::ops::query::units_json(&units), "volume": r6(m.props.volume * k * k * k), "surface_area": r6(m.props.surface_area * k * k) });
    let whole = tops.is_empty() || asm.instances.iter().all(|i| tops.contains(&i.id));
    let overridden = asm.properties.mass_override.filter(|_| whole);
    match m.props.mass.or(overridden) {
        Some(mass) => {
            let (c, i, f) = (m.props.center_of_mass, m.props.inertia, k * k);
            v["mass_kg"] = json!(r6(overridden.unwrap_or(mass)));
            v["mass_overridden"] = json!(overridden.is_some());
            v["center_of_mass"] = pt(c, k);
            v["inertia"] = json!([[r6(i.x_axis.x * f), r6(i.y_axis.x * f), r6(i.z_axis.x * f)], [r6(i.x_axis.y * f), r6(i.y_axis.y * f), r6(i.z_axis.y * f)], [r6(i.x_axis.z * f), r6(i.y_axis.z * f), r6(i.z_axis.z * f)]]);
            v["inertia_unit"] = json!(format!("kg·{}²", units.length));
        }
        None => {
            let names: Vec<String> = m.without_material.iter().filter_map(|p| result.node(p).map(|n| n.name.clone())).collect();
            v["without_material"] = json!(names);
            v["note"] = json!("Assign a material (or density) to every part to get mass, center of mass and inertia.");
        }
    }
    Ok(v.into())
}

fn update_references(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use crate::derived::ExternalRef;
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let scope: Option<Vec<InstanceId>> = match a.opt_array("instances")? {
        Some(list) => Some(list.iter().enumerate().map(|(i, v)| find_instance(&asm, v.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<_, _>>()?),
        None => None,
    };
    let in_scope = |id: InstanceId| scope.as_ref().is_none_or(|s| s.contains(&id));
    let version = a.opt_str("version")?.map(str::trim).filter(|v| !v.is_empty());
    let document = a.opt_str("document")?.map(|d| d.trim().trim_end_matches(".noshape").to_string());
    let named = |p: &std::path::Path| document.as_ref().is_none_or(|d| p.file_stem().is_some_and(|s| s.to_string_lossy() == *d));
    let mut updated = 0;
    // Other documents' (and this one's pinned) versions.
    let mut keys: Vec<String> = Vec::new();
    for i in asm.instances.iter().filter(|i| in_scope(i.id)) {
        if let Some(k) = i.source.external_key().filter(|k| !keys.iter().any(|x| x == k)) {
            keys.push(k.to_string());
        }
    }
    let mut unknown_version = None;
    for k in keys {
        let Some(r) = ExternalRef::parse(&k) else { continue };
        if !named(&r.path) {
            continue;
        }
        let Ok(info) = crate::derived::external_doc_info(&r.path) else { continue };
        let versions: Vec<_> = info.versions.iter().filter(|v| v.label != "Current state").collect();
        let to = match version {
            Some(v) => match versions.iter().find(|x| x.label == v) {
                Some(x) => *x,
                None => {
                    unknown_version = Some(info.name.clone());
                    continue;
                }
            },
            None => match versions.first() {
                Some(x) => *x,
                None => continue,
            },
        };
        if to.microversion == r.microversion {
            continue;
        }
        let new_key = ExternalRef { microversion: to.microversion, ..r }.key();
        let result = solve(od, tab, &SolveOptions::default())?;
        updated += commit(od, tab, &format!("Update to {}", to.label), &result, |asm| sa::repoint_external(asm, &k, &new_key, scope.as_deref()))?;
    }
    if updated == 0
        && let (Some(v), Some(d)) = (version, unknown_version)
    {
        return Err(OpError::param("version", &format!("{d} has no version named {v}")));
    }
    let mut v = describe(od, tab)?;
    v["updated"] = json!(updated);
    Ok(v.into())
}
