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
            verb: "rename",
            summary: "Rename a part (Parts list › Rename)",
            description: "The part's name in the parts list, BOMs and drawings (\"Rename : <name>\").",
            fields: vec![doc_field(), tab_field(), req("part", s(), "Part id (`body:…` from query_parts)."), req("name", s(), "New name.")],
            mutates: true,
            destructive: false,
            run: rename,
        },
        Op {
            group: "part",
            verb: "set_properties",
            summary: "Set a part's properties (the Properties dialog)",
            description: "Name, description, part number, revision, vendor, project, product line, titles and a mass override (kg). Fields not given keep their value; an empty text clears it. Fields configured for the active configuration keep the part's own value, as in the dialog.",
            fields: vec![
                doc_field(),
                tab_field(),
                req("part", s(), "Part id (`body:…`)."),
                opt("name", s(), "Name."),
                opt("description", s(), "Description."),
                opt("part_number", s(), "Part number."),
                opt("revision", s(), "Revision."),
                opt("vendor", s(), "Vendor."),
                opt("project", s(), "Project."),
                opt("product_line", s(), "Product line."),
                opt("title1", s(), "Title 1."),
                opt("title2", s(), "Title 2."),
                opt("title3", s(), "Title 3."),
                opt("mass_override", json!({ "type": ["number", "null"] }), "Mass override in kg (null: computed)."),
            ],
            mutates: true,
            destructive: false,
            run: set_properties,
        },
        Op {
            group: "part",
            verb: "configured_properties",
            summary: "Configured part properties: add a property column, set a value, delete a column or the table",
            description: "The Configuration panel's configured properties table of a part (Configure part properties): without `action`, replies with the table (`properties`, `rows`: each row's configuration and values). `add_property` adds a column (\"Add configured property\"; the first one creates the table with a row for the active configuration; values start from the part's own); `set` sets `property` to `value` in the row of the active configuration (or `configuration`: {input: value}), adding that row when missing (\"Edit configured properties\"); `delete_column` is a column's right-click › Delete column (\"Delete configured property\"); `delete_table` is ⋯ › Delete table (\"Delete configured properties table\").",
            fields: vec![
                doc_field(),
                tab_field(),
                req("part", s(), "Part id (`body:…`)."),
                opt("action", json!({ "type": "string", "enum": ["add_property", "set", "delete_column", "delete_table"] }), "What to do (default: read the table)."),
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
            summary: "Part Studio annotations: add a datum, list, delete",
            description: "Inspection panel › Datum and the annotations' right-click › Delete annotation (Model definitions; annotations aren't features). `add_datum` puts datum `letter` (default the next free letter) on planar face `face`, lying in the face's plane at its center (\"Datum\"); `delete` removes annotation `annotation` (\"Delete annotation\"). Without `action`, lists them.",
            fields: vec![
                doc_field(),
                tab_field(),
                opt("action", json!({ "type": "string", "enum": ["add_datum", "delete"] }), "What to do (default: list)."),
                opt("face", s(), "add_datum: a planar face id."),
                opt("letter", s(), "add_datum: the datum letter."),
                opt("annotation", json!({ "type": "integer" }), "delete: the annotation id."),
            ],
            mutates: true,
            destructive: false,
            run: annotation,
        },
    ]
}

fn part_arg(a: &Args) -> Result<BodyId, OpError> {
    match refs::parse_str(a.str("part")?) {
        Some(Reference::Body(b)) => Ok(b),
        _ => Err(OpError::param("part", "a part id (`body:…` from query_parts)")),
    }
}

fn rename(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let body = part_arg(a)?;
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
    let body = part_arg(a)?;
    let r = od.regenerate(tab, None)?;
    let shown = r.display.bodies.iter().find(|b| b.id == body).map(|b| b.name.clone()).ok_or_else(|| OpError::param("part", "no such part in the Part Studio"))?;
    let config = od.current_config(tab)?;
    let ps = crate::doc::studio(od.doc(), tab)?;
    let p = ps.part_properties.get(&body).cloned().unwrap_or_default();
    // The dialog opens with the part's values (configured fields shown as configured).
    const FIELDS: [&str; 10] = ["Name", "Description", "Part number", "Revision", "Vendor", "Project", "Product line", "Title 1", "Title 2", "Title 3"];
    let configured: Vec<&str> = FIELDS.into_iter().filter(|f| noshape_model::configured_properties::configured_value(ps, &body, f, &config).is_some()).collect();
    let mut v = studio::PartPropertyValues {
        name: p.name.clone().unwrap_or(shown),
        description: p.description.clone().unwrap_or_default(),
        part_number: p.part_number.clone().unwrap_or_default(),
        revision: p.revision.clone().unwrap_or_default(),
        meta: p.meta.clone(),
        mass_override: p.mass_override,
    };
    let text = |k: &str| -> Result<Option<String>, OpError> { Ok(a.opt_str(k)?.map(|x| x.trim().to_string())) };
    if let Some(x) = text("name")? {
        if x.is_empty() {
            return Err(OpError::param("name", "a part needs a name"));
        }
        v.name = x;
    }
    for (k, f) in [("description", &mut v.description), ("part_number", &mut v.part_number), ("revision", &mut v.revision)] {
        if let Some(x) = text(k)? {
            *f = x;
        }
    }
    for (k, f) in [("vendor", &mut v.meta.vendor), ("project", &mut v.meta.project), ("product_line", &mut v.meta.product_line), ("title1", &mut v.meta.title1), ("title2", &mut v.meta.title2), ("title3", &mut v.meta.title3)] {
        if let Some(x) = text(k)? {
            *f = Some(x).filter(|s| !s.is_empty());
        }
    }
    match a.get("mass_override") {
        None => {}
        Some(serde_json::Value::Null) => v.mass_override = None,
        Some(x) => v.mass_override = Some(x.as_f64().filter(|m| *m > 0.0).ok_or_else(|| OpError::param("mass_override", "a mass in kg greater than zero, or null"))?),
    }
    let desc = studio::properties_description(&v.name);
    edit_studio(od, tab, &desc, |ps| {
        studio::set_part_properties(ps, &body, &v, &configured);
        Ok(())
    })?;
    let mut out = json!({ "part": refs::body_id(&body), "name": v.name });
    if !configured.is_empty() {
        out["configured_fields_kept"] = json!(configured);
    }
    Ok(out.into())
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
    let body = part_arg(a)?;
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
        Some("add_property") => {
            let p = prop()?;
            if in_table(p) {
                return Err(OpError::param("property", "already a column"));
            }
            let active = od.current_config(tab)?;
            edit_studio(od, tab, "Add configured property", |ps| {
                cp::add_property(ps, &body, p, &active, &part_name);
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
        Some(_) => return Err(OpError::param("action", "add_property, set, delete_column or delete_table")),
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
        Some(_) => return Err(OpError::param("action", "add_datum or delete")),
    }
    let ps = crate::doc::studio(od.doc(), tab)?;
    let list: Vec<serde_json::Value> = ps
        .annotations
        .iter()
        .map(|x| {
            let kind = match &x.kind {
                AnnotationKind::Datum { letter } => json!({ "datum": letter }),
            };
            json!({ "id": x.id, "kind": kind, "face": refs::to_id(&x.face) })
        })
        .collect();
    Ok(json!({ "annotations": list }).into())
}
