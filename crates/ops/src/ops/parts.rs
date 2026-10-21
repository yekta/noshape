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
