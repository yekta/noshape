//! Materials and appearances: list the material libraries, assign a
//! material to parts (density for mass properties), set the appearance of
//! parts, faces or features, remove appearances.

use super::{doc_field, edit_studio, s, strings, studio_target, tab_field};
use crate::appearance::{self as ap, AppearanceEditTarget};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, doc, refs};
use noshape_model::partstudio::TessQuality;
use noshape_model::reference::{BodyId, Reference};
use serde_json::{Value, json};

const TESS: [(&str, TessQuality); 5] = [("auto", TessQuality::Auto), ("coarse", TessQuality::Coarse), ("medium", TessQuality::Medium), ("fine", TessQuality::Fine), ("very_fine", TessQuality::VeryFine)];

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "material",
            verb: "list",
            summary: "List the materials of the material libraries",
            description: "The Material panel's libraries: the Onshape Material Library and the document's Material Library tabs, with each material's category and density (kg/m³).",
            fields: vec![doc_field(), opt("search", s(), "Only materials whose name or category contains this.")],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "part",
            verb: "set_material",
            summary: "Assign a material to parts (density for mass properties)",
            description: "The Material panel: a library material by name (`material_list`), or a custom material with `density`. One edit (\"Assign material\").",
            fields: vec![
                doc_field(),
                tab_field(),
                req("parts", strings(), "Part ids (`body:…`) or names."),
                opt("material", s(), "Library material name, e.g. \"Aluminum - 6061\"; or the custom material's name with `density`."),
                opt("library", s(), "Library to take it from (default: the first that has it)."),
                opt("density", json!({ "type": "number", "exclusiveMinimum": 0 }), "Custom material: density in kg/m³."),
            ],
            mutates: true,
            destructive: false,
            run: set_material,
        },
        Op {
            group: "part",
            verb: "set_appearance",
            summary: "Set the color and transparency of parts, faces or a feature (the Appearance editor)",
            description: "Give `parts`, `faces` or `feature`. Color as \"#3b61b4\" or [r, g, b] (0–255, sRGB); transparency 0 (opaque) to 1. Parts also take the tessellation quality. One edit (\"Edit appearance\").",
            fields: vec![
                doc_field(),
                tab_field(),
                opt("parts", strings(), "Part ids or names."),
                opt("faces", strings(), "Face ids (`face:…`)."),
                opt("feature", s(), "Feature id or name (its faces; a sketch: its curves)."),
                req("color", json!({ "type": ["string", "array"] }), "\"#rrggbb\" or [r, g, b]."),
                opt("transparency", json!({ "type": "number", "minimum": 0, "maximum": 1 }), "0 opaque (default) … 1 invisible."),
                opt("tessellation", json!({ "type": "string", "enum": TESS.iter().map(|t| t.0).collect::<Vec<_>>() }), "Parts: tessellation quality (default auto)."),
            ],
            mutates: true,
            destructive: false,
            run: set_appearance,
        },
        Op {
            group: "part",
            verb: "remove_appearance",
            summary: "Remove custom appearances (Appearance panel › Remove appearance)",
            description: "A part's own appearance (`parts`), or face / feature appearances by index (`appearances`, from part_appearances).",
            fields: vec![doc_field(), tab_field(), opt("parts", strings(), "Part ids or names."), opt("appearances", json!({ "type": "array", "items": { "type": "integer", "minimum": 0 } }), "Face / feature appearance indices.")],
            mutates: true,
            destructive: true,
            run: remove_appearance,
        },
        Op {
            group: "part",
            verb: "appearances",
            summary: "The Part Studio's custom appearances and materials (the Appearance panel)",
            description: "Each part's material, density and own appearance, and the face / feature appearances with their index.",
            fields: vec![doc_field(), tab_field()],
            mutates: false,
            destructive: false,
            run: appearances,
        },
    ]
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

fn color_json(c: [f32; 4]) -> Value {
    json!({ "color": hex([ap::lin_to_srgb(c[0]), ap::lin_to_srgb(c[1]), ap::lin_to_srgb(c[2])]), "transparency": ((1.0 - c[3]) * 1000.0).round() / 1000.0 })
}

/// Parts by id or by the name the parts list shows.
fn parts_arg(od: &mut crate::OpenDoc, tab: noshape_model::document::TabId, a: &Args, name: &str) -> Result<Vec<BodyId>, OpError> {
    let r = od.regenerate(tab, None)?;
    a.array(name)?
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let key = v.as_str().ok_or_else(|| OpError::param(&format!("{name}[{i}]"), "a part id or name"))?;
            let id = match refs::parse_str(key) {
                Some(Reference::Body(b)) => Some(b),
                _ => None,
            };
            r.display
                .bodies
                .iter()
                .find(|b| id.as_ref() == Some(&b.id) || b.name == key || b.name.eq_ignore_ascii_case(key))
                .map(|b| b.id.clone())
                .ok_or_else(|| OpError::param(&format!("{name}[{i}]"), &format!("no part `{key}`; parts: {}", r.display.bodies.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", "))))
        })
        .collect()
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let q = a.opt_str("search")?.map(|s| s.to_lowercase());
    let rows: Vec<Value> = ap::library_materials(od.doc())
        .into_iter()
        .filter(|m| q.as_ref().is_none_or(|q| m.name.to_lowercase().contains(q) || m.category.to_lowercase().contains(q)))
        .map(|m| json!({ "library": m.library, "name": m.name, "category": m.category, "density_kg_m3": (m.density * 1000.0).round() / 1000.0 }))
        .collect();
    Ok(json!({ "materials": rows }).into())
}

fn set_material(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let bodies = parts_arg(od, tab, a, "parts")?;
    let (name, density) = match (a.opt_str("material")?, a.opt_f64("density")?) {
        (Some(n), Some(d)) => {
            if d <= 0.0 {
                return Err(OpError::param("density", "must be greater than zero (kg/m³)"));
            }
            (n.trim().to_string(), d)
        }
        (Some(n), None) => {
            let lib = a.opt_str("library")?;
            let m = ap::library_materials(od.doc())
                .into_iter()
                .find(|m| m.name.eq_ignore_ascii_case(n.trim()) && lib.is_none_or(|l| m.library.eq_ignore_ascii_case(l)))
                .ok_or_else(|| OpError::param("material", "no such library material (see material_list), or give `density` for a custom one"))?;
            (m.name, m.density)
        }
        (None, _) => return Err(OpError::param("material", "a material name")),
    };
    if name.is_empty() {
        return Err(OpError::param("material", "a material name"));
    }
    edit_studio(od, tab, ap::ASSIGN_MATERIAL, |ps| {
        ap::assign_material(ps, &bodies, &name, density);
        Ok(())
    })?;
    Ok(json!({ "parts": bodies.iter().map(refs::body_id).collect::<Vec<_>>(), "material": name, "density_kg_m3": density }).into())
}

fn set_appearance(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let rgb = match a.get("color").expect("required") {
        Value::String(h) => ap::parse_hex(h).ok_or_else(|| OpError::param("color", "\"#rrggbb\""))?,
        Value::Array(c) if c.len() == 3 => {
            let ch = |i: usize| c[i].as_u64().filter(|v| *v <= 255).map(|v| v as u8).ok_or_else(|| OpError::param("color", "[r, g, b] with 0–255"));
            [ch(0)?, ch(1)?, ch(2)?]
        }
        _ => return Err(OpError::param("color", "\"#rrggbb\" or [r, g, b]")),
    };
    let transparency = a.opt_f64("transparency")?.unwrap_or(0.0);
    if !(0.0..=1.0).contains(&transparency) {
        return Err(OpError::param("transparency", "0 to 1"));
    }
    let tess = match a.opt_str("tessellation")? {
        Some(t) => TESS.iter().find(|x| x.0 == t).map(|x| x.1).ok_or_else(|| OpError::param("tessellation", "unknown quality"))?,
        None => TessQuality::Auto,
    };
    let target = match (a.get("parts"), a.get("faces"), a.opt_str("feature")?) {
        (Some(_), None, None) => AppearanceEditTarget::Parts(parts_arg(od, tab, a, "parts")?),
        (None, Some(_), None) => AppearanceEditTarget::Faces(a.array("faces")?.iter().enumerate().map(|(i, v)| refs::parse(v).map_err(|e| e.with_param(format!("faces[{i}]")))).collect::<Result<_, _>>()?, None),
        (None, None, Some(f)) => AppearanceEditTarget::Feature(doc::find_feature(doc::studio(od.doc(), tab)?, f).map_err(|e| e.with_param("feature"))?, None),
        _ => return Err(OpError::invalid("give one of `parts`, `faces` or `feature`")),
    };
    edit_studio(od, tab, ap::EDIT_APPEARANCE, |ps| {
        ap::apply_appearance(ps, &target, rgb, transparency as f32, tess);
        Ok(())
    })?;
    Ok(json!({ "color": hex(rgb), "transparency": transparency }).into())
}

fn remove_appearance(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    if a.get("parts").is_some() {
        let bodies = parts_arg(od, tab, a, "parts")?;
        edit_studio(od, tab, ap::EDIT_APPEARANCE, |ps| {
            for b in &bodies {
                ap::reset_part_appearance(ps, b);
            }
            Ok(())
        })?;
    }
    if let Some(list) = a.opt_array("appearances")? {
        let n = doc::studio(od.doc(), tab)?.appearances.len();
        let mut idx: Vec<usize> = list.iter().enumerate().map(|(i, v)| v.as_u64().map(|x| x as usize).filter(|x| *x < n).ok_or_else(|| OpError::param(&format!("appearances[{i}]"), &format!("an index below {n}")))).collect::<Result<_, _>>()?;
        // Highest first, so the others keep their index.
        idx.sort_unstable_by(|x, y| y.cmp(x));
        idx.dedup();
        for i in idx {
            edit_studio(od, tab, ap::REMOVE_APPEARANCE, |ps| {
                ap::remove_appearance(ps, i);
                Ok(())
            })?;
        }
    }
    appearances(ctx, a)
}

fn appearances(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let r = od.regenerate(tab, None)?;
    let ps = doc::studio(od.doc(), tab)?;
    let parts: Vec<Value> = r
        .display
        .bodies
        .iter()
        .map(|b| {
            let p = ps.part_properties.get(&b.id);
            let mut j = json!({ "id": refs::body_id(&b.id), "name": b.name, "material": p.and_then(|p| p.material.clone()), "density_kg_m3": p.and_then(|p| p.density) });
            if let Some(ap) = p.and_then(|p| p.appearance) {
                j["appearance"] = color_json(ap.color);
            }
            j
        })
        .collect();
    let other: Vec<Value> = ps
        .appearances
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let mut j = color_json(x.color);
            j["index"] = json!(i);
            match &x.target {
                noshape_model::partstudio::AppearanceTarget::Faces(f) => j["faces"] = json!(f.iter().map(refs::to_id).collect::<Vec<_>>()),
                noshape_model::partstudio::AppearanceTarget::Feature(f) => j["feature"] = json!(f.to_string()),
            }
            j
        })
        .collect();
    Ok(json!({ "parts": parts, "appearances": other }).into())
}
