//! Parts list: rename a part, set its properties (the Properties dialog).

use super::{doc_field, edit_studio, s, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, refs, studio};
use noshape_model::reference::{BodyId, Reference};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "part",
            verb: "where_used",
            summary: "Where a part is used (part menu › Where used…)",
            description: "The assemblies, drawings and Derived features that use the part: in this document's workspace (version \"Main\") and in the other documents on this computer, each with the version of this document it uses. Live's panel lists versions only; this also reads the workspace.",
            fields: vec![doc_field(), tab_field(), req("part", s(), "Part id (`body:…`) or name."), opt("documents", json!({ "type": "array", "items": { "type": "string" } }), "Other documents to search (paths; default: the recent documents and the documents folder).")],
            mutates: false,
            destructive: false,
            run: where_used,
        },
        Op {
            group: "part",
            verb: "rename",
            summary: "Rename a part (Parts list › Rename)",
            description: "The part's name in the parts list, BOMs and drawings (\"Rename : <name>\").",
            fields: vec![doc_field(), tab_field(), req("part", s(), "Part id (`body:…` from query_parts) or name."), req("name", s(), "New name.")],
            mutates: true,
            destructive: false,
            run: rename,
        },
        Op {
            group: "part",
            verb: "set_properties",
            summary: "Set a part's properties, or several parts' at once (the Properties dialog)",
            description: "Name, description, part number, revision, vendor, project, product line, titles, Not revision managed, Exclude from all BOMs, Unit of measure and a mass override (kg); State is set by releases. Fields not given keep their value; an empty text clears it. Fields configured for the active configuration keep the part's own value, as in the dialog. With `parts` (the Parts list's Properties… on several parts) the given fields go to each part and the others keep each part's own value.",
            fields: vec![
                doc_field(),
                tab_field(),
                opt("part", s(), "Part id (`body:…`) or name."),
                opt("parts", json!({ "type": "array", "items": { "type": "string" } }), "Several parts' ids or names (instead of `part`)."),
                opt("name", s(), "Name."),
                opt("description", s(), "Description."),
                opt("part_number", s(), "Part number."),
                opt("revision", s(), "Revision."),
                opt("mass_override", json!({ "type": ["number", "null"] }), "Mass override in kg (null: computed)."),
            ]
            .into_iter()
            .chain(meta_fields())
            .collect(),
            mutates: true,
            destructive: false,
            run: set_properties,
        },
        Op {
            group: "part",
            verb: "configured_properties",
            summary: "Configured part properties: add a property column, set a value, delete a column or the table",
            description: "The Configuration panel's configured properties table of a part (Configure part properties): without `action`, replies with the table (`properties`, `rows`: each row's configuration and values). `add_property` adds a column (\"Add configured property\"; the first one creates the table with a row for the active configuration; values start from the part's own); `configure_property` is the Appearances row's Configure appearance (the same, but a new table gets a row for every configuration of the first List input); `set` sets `property` to `value` in the row of the active configuration (or `configuration`: {input: value}), adding that row when missing (\"Edit configured properties\"); `delete_column` is a column's right-click › Delete column (\"Delete configured property\"); `delete_table` is ⋯ › Delete table (\"Delete configured properties table\").",
            fields: vec![
                doc_field(),
                tab_field(),
                req("part", s(), "Part id (`body:…`) or name."),
                opt("action", json!({ "type": "string", "enum": ["add_property", "configure_property", "set", "delete_column", "delete_table"] }), "What to do (default: read the table)."),
                opt("property", json!({ "type": "string", "enum": noshape_model::configured_properties::PROPERTIES }), "The property (column)."),
                opt("value", s(), "set: the value (\"true\" / \"false\" for checkboxes)."),
                opt("configuration", json!({ "type": "object" }), "set: the row's configuration, input id → value (default: the active one)."),
            ],
            mutates: true,
            destructive: false,
            run: configured_properties,
        },
        Op {
            group: "part",
            verb: "annotation",
            summary: "Part Studio annotations: add a datum or an MBD dimension, list, delete",
            description: "Inspection panel › Datum / Dimension and the annotations' right-click › Delete annotation (Model definitions; annotations aren't features). `add_datum` puts datum `letter` (default the next free letter) on planar face `face`, lying in the face's plane at its center (\"Datum\"); `add_dimension` measures `entities` (distance between two of them, angle between two planar faces, radius of a cylindrical face, minimum / maximum distance), lying in `plane`, its text at `position`, with an optional `tolerance` (\"Dimension\"; drawings of the part show it in the views facing its plane); `delete` removes annotation `annotation` (\"Delete annotation\"); `move` is dragging its letter box: `offset` [right, up] from the triangle in screen pixels along the symbol's plane axes, drawn with a dashed leader ([0, 0] puts it back; \"Move annotation\"). Without `action`, lists them (with each moved box's `offset`).",
            fields: vec![
                doc_field(),
                tab_field(),
                opt("action", json!({ "type": "string", "enum": ["add_datum", "add_dimension", "delete", "move"] }), "What to do (default: list)."),
                opt("entities", json!({ "type": "array", "items": { "type": "string" } }), "add_dimension: the measured entities (faces, edges, vertices; one cylindrical face for a radius)."),
                opt("measure", json!({ "type": "string", "enum": ["distance", "angle", "radius", "minimum", "maximum"] }), "add_dimension: what it measures (default distance; radius with one entity)."),
                opt("plane", json!({ "type": "string", "enum": ["top", "front", "right"] }), "add_dimension: the plane it lies in (default: the first of front, top, right containing the measurement)."),
                opt("position", json!({ "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }), "add_dimension: its text position [x, y, z] mm (default 10 mm off the measurement)."),
                opt("tolerance", json!({ "type": "object" }), "add_dimension: {type: Default | Symmetrical | Deviation | Limits | Min | Max | Basic, upper, lower, precision}."),
                opt("face", s(), "add_datum: a planar face id."),
                opt("letter", s(), "add_datum: the datum letter."),
                opt("annotation", json!({ "type": "integer" }), "delete / move: the annotation id."),
                opt("offset", json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }), "move: [right, up] in screen pixels."),
            ],
            mutates: true,
            destructive: false,
            run: annotation,
        },
    ]
}

/// The `part` argument: a `body:…` id or the name the parts list shows.
fn part_arg(od: &mut crate::OpenDoc, tab: noshape_model::document::TabId, a: &Args) -> Result<BodyId, OpError> {
    part_by_key(od, tab, a.str("part")?)
}

fn part_by_key(od: &mut crate::OpenDoc, tab: noshape_model::document::TabId, key: &str) -> Result<BodyId, OpError> {
    if let Some(Reference::Body(b)) = refs::parse_str(key) {
        return Ok(b);
    }
    let r = od.regenerate(tab, None)?;
    Ok(super::query::find_part(&r, key)?.id.clone())
}

fn where_used(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let body = part_arg(od, tab, a)?;
    if !od.regenerate(tab, None)?.display.bodies.iter().any(|b| b.id == body) {
        return Err(OpError::param("part", "no such part in the Part Studio"));
    }
    let others: Vec<std::path::PathBuf> = match a.opt_array("documents")? {
        Some(list) => list.iter().filter_map(|v| v.as_str()).map(std::path::PathBuf::from).collect(),
        None => {
            let mut v = crate::doc_library::recent();
            if let Ok(rd) = std::fs::read_dir(noshape_scene::documents_dir()) {
                v.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape")));
            }
            v
        }
    };
    let (name, path) = (od.session.name(), od.session.path.clone());
    let uses = crate::where_used::where_used(od.doc(), &name, &path, tab, &body, &others);
    let rows: Vec<serde_json::Value> = uses.iter().map(|u| json!({ "document": u.document, "path": u.path.display().to_string(), "element": u.element, "kind": u.kind, "version": u.version })).collect();
    Ok(json!({ "part": refs::to_id(&Reference::Body(body)), "uses": rows }).into())
}

fn rename(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let body = part_arg(od, tab, a)?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "a name"));
    }
    if !od.regenerate(tab, None)?.display.bodies.iter().any(|b| b.id == body) {
        return Err(OpError::param("part", "no such part in the Part Studio"));
    }
    edit_studio(od, tab, &studio::rename_part_description(&name), |ps| {
        studio::rename_part(ps, &body, &name);
        Ok(())
    })?;
    Ok(json!({ "part": refs::body_id(&body), "name": name }).into())
}

fn set_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let bodies = match (a.opt_str("part")?, a.opt_array("parts")?) {
        (Some(_), Some(_)) => return Err(OpError::param("parts", "give `part` or `parts`, not both")),
        (Some(_), None) => vec![part_arg(od, tab, a)?],
        (None, Some(list)) => {
            let keys: Vec<&str> = list.iter().map(|v| v.as_str().ok_or_else(|| OpError::param("parts", "each part is an id or a name"))).collect::<Result<_, _>>()?;
            if keys.is_empty() {
                return Err(OpError::param("parts", "at least one part"));
            }
            keys.into_iter().map(|k| part_by_key(od, tab, k)).collect::<Result<Vec<_>, _>>()?
        }
        (None, None) => return Err(OpError::param("part", "required string")),
    };
    let r = od.regenerate(tab, None)?;
    let config = od.current_config(tab)?;
    let ps = crate::doc::studio(od.doc(), tab)?;
    let text = |k: &str| -> Result<Option<String>, OpError> { Ok(a.opt_str(k)?.map(|x| x.trim().to_string())) };
    if text("name")?.is_some_and(|x| x.is_empty()) {
        return Err(OpError::param("name", "a part needs a name"));
    }
    // Each part's values as its dialog opens (configured fields shown as configured), the given fields on top.
    const FIELDS: [&str; 10] = ["Name", "Description", "Part number", "Revision", "Vendor", "Project", "Product line", "Title 1", "Title 2", "Title 3"];
    let mut edits = Vec::new();
    for body in &bodies {
        let shown = r.display.bodies.iter().find(|b| b.id == *body).map(|b| b.name.clone()).ok_or_else(|| OpError::param(if a.get("parts").is_some() { "parts" } else { "part" }, "no such part in the Part Studio"))?;
        let p = ps.part_properties.get(body).cloned().unwrap_or_default();
        let configured: Vec<&str> = FIELDS.into_iter().filter(|f| noshape_model::configured_properties::configured_value(ps, body, f, &config).is_some()).collect();
        let mut v = studio::PartPropertyValues {
            name: p.name.clone().unwrap_or(shown),
            description: p.description.clone().unwrap_or_default(),
            part_number: p.part_number.clone().unwrap_or_default(),
            revision: p.revision.clone().unwrap_or_default(),
            meta: p.meta.clone(),
            mass_override: p.mass_override,
        };
        if let Some(x) = text("name")? {
            v.name = x;
        }
        for (k, f) in [("description", &mut v.description), ("part_number", &mut v.part_number), ("revision", &mut v.revision)] {
            if let Some(x) = text(k)? {
                *f = x;
            }
        }
        apply_meta(a, &mut v.meta)?;
        match a.get("mass_override") {
            None => {}
            Some(serde_json::Value::Null) => v.mass_override = None,
            Some(x) => v.mass_override = Some(x.as_f64().filter(|m| *m > 0.0).ok_or_else(|| OpError::param("mass_override", "a mass in kg greater than zero, or null"))?),
        }
        edits.push((body.clone(), v, configured));
    }
    let desc = match &edits[..] {
        [(_, v, _)] => studio::properties_description(&v.name),
        _ => studio::properties_description(&format!("{} parts", edits.len())),
    };
    edit_studio(od, tab, &desc, |ps| {
        for (body, v, configured) in &edits {
            studio::set_part_properties(ps, body, v, configured);
        }
        Ok(())
    })?;
    let out: Vec<serde_json::Value> = edits
        .iter()
        .map(|(body, v, configured)| {
            let mut o = json!({ "part": refs::body_id(body), "name": v.name });
            if !configured.is_empty() {
                o["configured_fields_kept"] = json!(configured);
            }
            o
        })
        .collect();
    match &out[..] {
        [one] if a.get("parts").is_none() => Ok(one.clone().into()),
        _ => Ok(json!({ "parts": out }).into()),
    }
}

/// The Properties dialog's item fields of a part or an assembly: vendor, project, product line, titles, Not revision
/// managed, Exclude from all BOMs, Unit of measure and custom properties (fields not given keep their value).
pub(crate) fn apply_meta(a: &Args, meta: &mut noshape_model::partstudio::ItemMeta) -> Result<(), OpError> {
    let text = |k: &str| -> Result<Option<String>, OpError> { Ok(a.opt_str(k)?.map(|x| x.trim().to_string())) };
    for (k, f) in [("vendor", &mut meta.vendor), ("project", &mut meta.project), ("product_line", &mut meta.product_line), ("title1", &mut meta.title1), ("title2", &mut meta.title2), ("title3", &mut meta.title3)] {
        if let Some(x) = text(k)? {
            *f = Some(x).filter(|s| !s.is_empty());
        }
    }
    meta.not_revision_managed = a.bool_or("not_revision_managed", meta.not_revision_managed)?;
    meta.exclude_from_bom = a.bool_or("exclude_from_bom", meta.exclude_from_bom)?;
    if let Some(u) = a.opt_str("unit_of_measure")? {
        use noshape_model::configured_properties::UNITS_OF_MEASURE;
        let u = UNITS_OF_MEASURE.iter().find(|x| x.eq_ignore_ascii_case(u.trim())).ok_or_else(|| OpError::param("unit_of_measure", &format!("one of {}", UNITS_OF_MEASURE.join(", "))))?;
        // The first ("No value") is the default, stored as none.
        meta.unit_of_measure = (*u != UNITS_OF_MEASURE[0]).then(|| u.to_string());
    }
    super::custom_props::apply_custom(a, &mut meta.custom)
}

/// The fields [`apply_meta`] reads.
pub(crate) fn meta_fields() -> Vec<crate::registry::Field> {
    vec![
        opt("vendor", s(), "Vendor."),
        opt("project", s(), "Project."),
        opt("product_line", s(), "Product line."),
        opt("title1", s(), "Title 1."),
        opt("title2", s(), "Title 2."),
        opt("title3", s(), "Title 3."),
        opt("not_revision_managed", json!({ "type": "boolean" }), "Not revision managed."),
        opt("exclude_from_bom", json!({ "type": "boolean" }), "Exclude from all BOMs."),
        opt("unit_of_measure", json!({ "type": "string", "enum": noshape_model::configured_properties::UNITS_OF_MEASURE }), "Unit of measure."),
        opt("custom", json!({ "type": "object" }), "Custom properties: name → value (\"\" or null clears); company_custom_properties defines them."),
    ]
}

fn table_json(ps: &noshape_model::PartStudio, body: &BodyId) -> serde_json::Value {
    match noshape_model::configured_properties::table(ps, body) {
        None => json!({ "properties": [], "rows": [] }),
        Some(t) => json!({
            "properties": t.properties,
            "rows": t.rows.iter().map(|r| json!({ "configuration": r.config.0, "values": r.values })).collect::<Vec<_>>(),
        }),
    }
}

fn configured_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::configured_properties as cp;
    let (od, tab) = studio_target(ctx, a)?;
    let body = part_arg(od, tab, a)?;
    let r = od.regenerate(tab, None)?;
    let part_name = r.display.bodies.iter().find(|b| b.id == body).map(|b| b.name.clone()).ok_or_else(|| OpError::param("part", "no such part in the Part Studio"))?;
    let ps = crate::doc::studio(od.doc(), tab)?.clone();
    let has_table = cp::table(&ps, &body).is_some();
    let prop = || -> Result<&'static str, OpError> {
        let p = a.opt_str("property")?.ok_or_else(|| OpError::param("property", "required for this action"))?;
        cp::PROPERTIES.into_iter().find(|x| *x == p).ok_or_else(|| OpError::param("property", &format!("one of {}", cp::PROPERTIES.join(", "))))
    };
    let in_table = |p: &str| cp::table(&ps, &body).is_some_and(|t| t.properties.iter().any(|x| x == p));
    match a.opt_str("action")? {
        None => {}
        Some(action @ ("add_property" | "configure_property")) => {
            let p = prop()?;
            if in_table(p) {
                return Err(OpError::param("property", "already a column"));
            }
            let active = od.current_config(tab)?;
            edit_studio(od, tab, "Add configured property", |ps| {
                if action == "add_property" {
                    cp::add_property(ps, &body, p, &active, &part_name);
                } else {
                    cp::configure_property(ps, &body, p, &active, &part_name);
                }
                Ok(())
            })?;
        }
        Some("set") => {
            let p = prop()?;
            if !in_table(p) {
                return Err(OpError::param("property", "not a column of the part's table (add_property first)"));
            }
            let value = a.str("value")?.to_string();
            let config = match a.object("configuration")? {
                Some(o) => noshape_model::configuration::ConfigValues(o.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect()),
                None => od.current_config(tab)?,
            };
            edit_studio(od, tab, "Edit configured properties", |ps| {
                if cp::set_for_configuration(ps, &body, p, &config, &value) { Ok(()) } else { Err(OpError::param("configuration", "no row for that configuration")) }
            })?;
        }
        Some("delete_column") => {
            let p = prop()?;
            if !in_table(p) {
                return Err(OpError::param("property", "not a column of the part's table"));
            }
            edit_studio(od, tab, "Delete configured property", |ps| {
                cp::delete_column(ps, &body, p);
                Ok(())
            })?;
        }
        Some("delete_table") => {
            if !has_table {
                return Err(OpError::param("part", "the part has no configured properties table"));
            }
            edit_studio(od, tab, "Delete configured properties table", |ps| {
                cp::delete_table(ps, &body);
                Ok(())
            })?;
        }
        Some(_) => return Err(OpError::param("action", "add_property, configure_property, set, delete_column or delete_table")),
    }
    let ps = crate::doc::studio(od.doc(), tab)?;
    Ok(json!({ "part": refs::body_id(&body), "table": table_json(ps, &body) }).into())
}

fn annotation(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::annotations::{self as ann, Annotation, AnnotationKind};
    let (od, tab) = studio_target(ctx, a)?;
    match a.opt_str("action")? {
        None => {}
        Some("add_datum") => {
            let face = refs::parse(a.get("face").ok_or_else(|| OpError::param("face", "a planar face id"))?).map_err(|e| e.with_param("face"))?;
            let r = od.regenerate(tab, None)?;
            // At the face's center, in its plane, facing out (as the query tools report it).
            let placed = match &face {
                Reference::Face(n) => r.context.bodies.iter().find_map(|b| {
                    let fi = *b.find_faces(n).first()?;
                    let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, fi).ok()?;
                    let p = f.face_plane().ok()??;
                    Some((f.centroid().ok()?, p.normal, p.normal.cross(p.unit_x()).normalize_or_zero()))
                }),
                _ => None,
            };
            let (at, normal, up) = placed.ok_or_else(|| OpError::param("face", "Select a planar face"))?;
            let ps = crate::doc::studio(od.doc(), tab)?;
            let letter = match a.opt_str("letter")? {
                Some(l) if !l.trim().is_empty() => l.trim().to_string(),
                Some(_) => return Err(OpError::param("letter", "a letter")),
                None => ann::next_datum_letter(&ps.annotations),
            };
            let mut x = Annotation { id: 0, kind: AnnotationKind::Datum { letter }, face, at: at.to_array(), normal: normal.to_array(), up: up.to_array(), offset: None };
            edit_studio(od, tab, "Datum", |ps| {
                x.id = ps.annotations.iter().map(|y| y.id).max().unwrap_or(0) + 1;
                ps.annotations.push(x);
                Ok(())
            })?;
        }
        Some("delete") => {
            let id = a.opt_u64("annotation")?.ok_or_else(|| OpError::param("annotation", "an annotation id (part_annotation)"))? as u32;
            if !crate::doc::studio(od.doc(), tab)?.annotations.iter().any(|x| x.id == id) {
                return Err(OpError::param("annotation", "no such annotation"));
            }
            edit_studio(od, tab, "Delete annotation", |ps| {
                ps.annotations.retain(|x| x.id != id);
                Ok(())
            })?;
        }
        Some("move") => {
            let id = a.opt_u64("annotation")?.ok_or_else(|| OpError::param("annotation", "an annotation id (part_annotation)"))? as u32;
            if !crate::doc::studio(od.doc(), tab)?.annotations.iter().any(|x| x.id == id) {
                return Err(OpError::param("annotation", "no such annotation"));
            }
            let o = a.opt_array("offset")?.ok_or_else(|| OpError::param("offset", "[right, up] in screen pixels"))?;
            let v: Vec<f64> = o.iter().filter_map(serde_json::Value::as_f64).collect();
            let [x, y] = v[..] else { return Err(OpError::param("offset", "two numbers")) };
            let offset = if x == 0.0 && y == 0.0 { None } else { Some([x, y]) };
            edit_studio(od, tab, "Move annotation", |ps| {
                if let Some(x) = ps.annotations.iter_mut().find(|x| x.id == id) {
                    x.offset = offset;
                }
                Ok(())
            })?;
        }
        Some("add_dimension") => {
            let ents = a.opt_array("entities")?.ok_or_else(|| OpError::param("entities", "one or two entity ids"))?;
            let rs: Vec<Reference> = ents.iter().map(refs::parse).collect::<Result<_, _>>().map_err(|e| e.with_param("entities"))?;
            let measure = match a.opt_str("measure")? {
                None if rs.len() == 1 => ann::Measure::Radius,
                None | Some("distance") => ann::Measure::Distance,
                Some("angle") => ann::Measure::Angle,
                Some("radius") => ann::Measure::Radius,
                Some("minimum") => ann::Measure::Minimum,
                Some("maximum") => ann::Measure::Maximum,
                Some(_) => return Err(OpError::param("measure", "distance, angle, radius, minimum or maximum")),
            };
            let want = if measure == ann::Measure::Radius { 1 } else { 2 };
            if rs.len() != want {
                return Err(OpError::param("entities", if want == 1 { "one cylindrical (fillet) face" } else { "two entities" }));
            }
            let r = od.regenerate(tab, None)?;
            let mut x = Annotation { id: 0, kind: AnnotationKind::Dimension { measure, refs: rs.clone(), tolerance: Default::default() }, face: rs[0].clone(), at: [0.0; 3], normal: [0.0, 0.0, 1.0], up: [0.0, 1.0, 0.0], offset: None };
            let plane = a.opt_str("plane")?;
            let position = match a.opt_array("position")? {
                Some(p) => {
                    let v: Vec<f64> = p.iter().filter_map(serde_json::Value::as_f64).collect();
                    let [px, py, pz] = v[..] else { return Err(OpError::param("position", "[x, y, z] in mm")) };
                    Some(glam::DVec3::new(px, py, pz) * 0.001)
                }
                None => None,
            };
            noshape_model::drawing::mbd::place_dimension(&r.context, &mut x, plane, position).map_err(|e| OpError::param("entities", &e))?;
            if let Some(t) = a.get("tolerance") {
                let t = t.as_object().ok_or_else(|| OpError::param("tolerance", "an object: type, upper, lower, precision"))?;
                if let AnnotationKind::Dimension { tolerance, .. } = &mut x.kind {
                    if let Some(k) = t.get("type").and_then(|v| v.as_str()) {
                        tolerance.kind = ["Default", "Symmetrical", "Deviation", "Limits", "Min", "Max", "Basic"].into_iter().find(|n| n.eq_ignore_ascii_case(k)).ok_or_else(|| OpError::param("tolerance.type", "Default, Symmetrical, Deviation, Limits, Min, Max or Basic"))?.to_string();
                    }
                    for (key, f) in [("upper", &mut tolerance.upper), ("lower", &mut tolerance.lower)] {
                        if let Some(v) = t.get(key) {
                            *f = match v {
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::String(s) => s.clone(),
                                _ => return Err(OpError::param(&format!("tolerance.{key}"), "a number or text")),
                            };
                        }
                    }
                    if let Some(p) = t.get("precision") {
                        tolerance.precision = p.as_u64().filter(|p| *p <= 6).map(|p| p as u8);
                    }
                }
            }
            edit_studio(od, tab, "Dimension", |ps| {
                x.id = ps.annotations.iter().map(|y| y.id).max().unwrap_or(0) + 1;
                ps.annotations.push(x);
                Ok(())
            })?;
        }
        Some(_) => return Err(OpError::param("action", "add_datum, add_dimension, delete or move")),
    }
    let r = od.regenerate(tab, None)?;
    let ps = crate::doc::studio(od.doc(), tab)?;
    let list: Vec<serde_json::Value> = ps
        .annotations
        .iter()
        .map(|x| {
            let kind = match &x.kind {
                AnnotationKind::Datum { letter } => json!({ "datum": letter }),
                AnnotationKind::Dimension { measure, refs: rs, tolerance } => {
                    let value = noshape_model::drawing::mbd::measure(&r.context, x).map(|(_, v)| if *measure == ann::Measure::Angle { v.to_degrees() } else { v * 1000.0 });
                    json!({ "dimension": { "measure": format!("{measure:?}").to_lowercase(), "value": value, "entities": rs.iter().map(refs::to_id).collect::<Vec<_>>(), "tolerance": tolerance.kind, "upper": tolerance.upper, "lower": tolerance.lower, "precision": tolerance.precision } })
                }
            };
            let mut v = json!({ "id": x.id, "kind": kind, "face": refs::to_id(&x.face) });
            if let Some(o) = x.offset {
                v["offset"] = json!(o);
            }
            v
        })
        .collect();
    Ok(json!({ "annotations": list }).into())
}
