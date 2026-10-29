//! The Part Studio's "Sheet metal table and flat view" panel (shared logic in
//! `crate::sheetmetal`): the Bends and Other joints tables of a sheet metal
//! model, a bend's Radius / K factor edit, Move up / Move down, Convert to
//! rip, and Export DXF/DWG of Flat pattern.

use super::{doc_field, edit_studio, regen_report, s, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::sheetmetal as sm;
use crate::{Ctx, OpenDoc, doc, refs};
use noshape_model::document::TabId;
use noshape_model::reference::{BodyId, FeatureId};
use noshape_model::sheetmetal::SheetMetalData;
use serde_json::{Value, json};
use std::sync::Arc;

fn model_field() -> crate::registry::Field {
    opt("model", s(), "The sheet metal model: its feature (id or name, e.g. \"Sheet metal model 1\"). Default: the panel's, the first by name.")
}

fn bend_field() -> crate::registry::Field {
    req("bend", json!({ "type": ["string", "integer"] }), "The Bends row: its name (\"Bend A\", \"Hem B\") or its index in sheetmetal_table's `bends`.")
}

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "sheetmetal",
            verb: "table",
            summary: "The sheet metal table: a model's bends (radius, angle, direction, K factor) and other joints",
            description: "The \"Sheet metal table and flat view\" panel: the sheet metal models (the Sheet metal context select), and for `model` its Bends rows in the table's order (name as \"Bend A\" / \"Hem B\", radius, angle, Up / Down from the flat pattern's bend lines, K factor, the bend's face ids) and its Other joints rows (name, style).",
            fields: vec![doc_field(), tab_field(), model_field()],
            mutates: false,
            destructive: false,
            run: table,
        },
        Op {
            group: "sheetmetal",
            verb: "edit_bend",
            summary: "Change a bend's radius or K factor in the sheet metal table (adds a Modify joint)",
            description: "Typing in a Bends row's Radius or K factor cell: a \"Modify joint\" of that bend with the value overridden, at the rollback bar or before the model's Finish sheet metal model; a later edit of the same bend changes that Modify joint (live). One edit per field (\"Modify joint\"). A bare radius is in the workspace's length unit.",
            fields: vec![doc_field(), tab_field(), model_field(), bend_field(), opt("radius", json!({ "type": ["string", "number"] }), "New inner radius (\"3 mm\", 3)."), opt("k_factor", json!({ "type": ["string", "number"] }), "New K factor (0.45).")],
            mutates: true,
            destructive: false,
            run: edit_bend,
        },
        Op {
            group: "sheetmetal",
            verb: "move_bend",
            summary: "Move a bend up or down in the sheet metal table",
            description: "A Bends row's menu › Move up / Move down (the table's order is stored with the Part Studio). One edit (\"Move up\" / \"Move down\").",
            fields: vec![doc_field(), tab_field(), model_field(), bend_field(), req("by", json!({ "type": "integer", "enum": [-1, 1] }), "-1: up, 1: down.")],
            mutates: true,
            destructive: false,
            run: move_bend,
        },
        Op {
            group: "sheetmetal",
            verb: "convert_to_rip",
            summary: "Convert a bend to a rip (a Modify joint: Rip, Edge joint)",
            description: "A Bends row's menu › Convert <bend> to rip: a Modify joint of the bend set to Rip / Edge joint, at the rollback bar or before the model's Finish. One edit (\"Convert joint to rip.\").",
            fields: vec![doc_field(), tab_field(), model_field(), bend_field()],
            mutates: true,
            destructive: false,
            run: convert_to_rip,
        },
        Op {
            group: "sheetmetal",
            verb: "export_flat",
            summary: "Export a sheet metal part's flat pattern as DXF or SVG",
            description: "The flat view's menu › Export DXF/DWG of Flat pattern of <part>…: the flat pattern as live writes it (outline, bend centerlines on the SHEETMETAL_BEND_LINES_UP / _DOWN layers), or an SVG of it.",
            fields: vec![doc_field(), tab_field(), req("part", s(), "The sheet metal part (`body:…` id or name)."), req("file", s(), "Where to write it."), opt("format", json!({ "type": "string", "enum": ["DXF", "SVG"], "default": "DXF" }), "File format.")],
            mutates: false,
            destructive: false,
            run: export_flat,
        },
    ]
}

/// A sheet metal model: its feature, name, first part and data.
type Model = (FeatureId, String, BodyId, Arc<SheetMetalData>);

/// The regenerated Part Studio's sheet metal models, by name.
fn models(od: &mut OpenDoc, tab: TabId) -> Result<(Vec<Model>, noshape_model::regen::RegenResult), OpError> {
    let r = od.regenerate(tab, None)?;
    let ps = doc::studio(od.doc(), tab)?;
    let mut out: Vec<Model> = Vec::new();
    for (b, d) in &r.context.sheet_metal {
        // (Only parts that exist: a std rebuild can leave an earlier part's entry.)
        if out.iter().any(|m| m.0 == d.model) || r.context.body(b).is_none() {
            continue;
        }
        let name = ps.feature(d.model).map(|f| f.name.clone()).unwrap_or_else(|| "Sheet metal model".into());
        out.push((d.model, name, b.clone(), d.clone()));
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok((out, r))
}

/// The model a call names (default: the first by name).
fn pick_model(list: &[Model], a: &Args) -> Result<usize, OpError> {
    if list.is_empty() {
        return Err(OpError::invalid("No sheet metal models.").with_param("model"));
    }
    let Some(k) = a.opt_str("model")? else { return Ok(0) };
    list.iter()
        .position(|m| m.0.to_string() == k || m.1 == k || m.1.eq_ignore_ascii_case(k))
        .ok_or_else(|| OpError::not_found(format!("no sheet metal model `{k}`; models: {}", list.iter().map(|m| m.1.clone()).collect::<Vec<_>>().join(", "))).with_param("model"))
}

/// The rows (model order) with each bend's flat-pattern direction.
fn rows(od: &OpenDoc, tab: TabId, r: &noshape_model::regen::RegenResult, body: &BodyId, data: &SheetMetalData) -> Result<(Vec<sm::JointRow>, Vec<sm::JointRow>), OpError> {
    let ps = doc::studio(od.doc(), tab)?;
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, body).ok();
    let up = |i: usize| flat.as_ref().and_then(|f| f.bend_lines.iter().find(|l| l.bend == i)).map(|l| l.up);
    let order = |f: FeatureId| ps.features.iter().position(|x| x.id == f).unwrap_or(usize::MAX);
    Ok(sm::joint_rows(data, &up, &order))
}

/// The bend a call names (model index).
fn pick_bend(bends: &[sm::JointRow], a: &Args) -> Result<usize, OpError> {
    let names = || bends.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(", ");
    match a.get("bend") {
        Some(Value::Number(n)) => n.as_u64().map(|n| n as usize).filter(|n| *n < bends.len()).ok_or_else(|| OpError::param("bend", &format!("no bend {n}; bends: {}", names()))),
        Some(Value::String(k)) => bends.iter().position(|b| b.name.eq_ignore_ascii_case(k.trim())).ok_or_else(|| OpError::not_found(format!("no bend `{k}`; bends: {}", names())).with_param("bend")),
        _ => Err(OpError::param("bend", "a bend name or index")),
    }
}

fn table(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let (list, r) = models(od, tab)?;
    if list.is_empty() {
        return Ok(json!({ "models": [], "note": "No sheet metal models." }).into());
    }
    let m = pick_model(&list, a)?;
    let (model, name, body, data) = &list[m];
    let (bends, joints) = rows(od, tab, &r, body, data)?;
    let units = od.doc().settings.units;
    let k = 1.0 / units.si_factor(noshape_units::Dim::LENGTH);
    let order = sm::bend_row_order(data, &sm::stored_order(doc::studio(od.doc(), tab)?, *model));
    let bend_rows: Vec<Value> = order
        .iter()
        .map(|&i| {
            let b = &bends[i];
            json!({
                "name": b.name,
                "index": i,
                "radius": sm::plain(b.radius * k),
                "angle_deg": sm::plain(b.angle),
                "direction": b.up.map(|u| if u { "Up" } else { "Down" }),
                "k_factor": sm::plain(b.k_factor),
                "faces": sm::bend_faces(&data.bends[i]).iter().map(refs::to_id).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({
        "models": list.iter().map(|m| json!({ "model": m.0.to_string(), "name": m.1 })).collect::<Vec<_>>(),
        "model": name,
        "units": { "length": units.length.to_string() },
        "bends": bend_rows,
        "joints": joints.iter().map(|j| json!({ "name": j.name, "style": j.style })).collect::<Vec<_>>(),
    })
    .into())
}

fn text(v: &Value, field: &str) -> Result<String, OpError> {
    match v {
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => Ok(s.trim().to_string()),
        _ => Err(OpError::param(field, "a number or an expression")),
    }
}

fn edit_bend(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let edits: Vec<(sm::BendField, String)> = [("radius", sm::BendField::Radius), ("k_factor", sm::BendField::KFactor)]
        .iter()
        .filter_map(|(k, f)| a.get(k).map(|v| text(v, k).map(|t| (*f, t))))
        .collect::<Result<_, _>>()?;
    if edits.is_empty() {
        return Err(OpError::invalid("give `radius` or `k_factor`"));
    }
    let mut changed = false;
    for (field, value) in edits {
        // Each cell is its own edit, on the model as the previous left it.
        let (list, r) = models(od, tab)?;
        let m = pick_model(&list, a)?;
        let (model, _, body, data) = list[m].clone();
        let (bends, _) = rows(od, tab, &r, &body, &data)?;
        let bend = pick_bend(&bends, a)?;
        let mut copy = doc::studio(od.doc(), tab)?.clone();
        if sm::edit_bend(&mut copy, model, &data, bend, field, &value, &units) {
            edit_studio(od, tab, sm::MODIFY_JOINT, |ps| {
                *ps = copy;
                Ok(())
            })?;
            changed = true;
        }
    }
    let mut v = json!({ "regeneration": regen_report(od, tab)? });
    if !changed {
        v["note"] = json!("the value is the bend's own: nothing changed");
    }
    Ok(v.into())
}

fn move_bend(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let (list, r) = models(od, tab)?;
    let m = pick_model(&list, a)?;
    let (model, _, body, data) = list[m].clone();
    let (bends, _) = rows(od, tab, &r, &body, &data)?;
    let bend = pick_bend(&bends, a)?;
    let delta: isize = if a.get("by").and_then(Value::as_i64).ok_or_else(|| OpError::param("by", "-1 or 1"))? < 0 { -1 } else { 1 };
    let mut copy = doc::studio(od.doc(), tab)?.clone();
    if !sm::move_bend(&mut copy, model, &data, bend, delta) {
        return Err(OpError::invalid(format!("{} is already the {} row", bends[bend].name, if delta < 0 { "first" } else { "last" })).with_param("by"));
    }
    edit_studio(od, tab, sm::move_description(delta), |ps| {
        *ps = copy;
        Ok(())
    })?;
    table(ctx, a)
}

fn convert_to_rip(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let (list, r) = models(od, tab)?;
    let m = pick_model(&list, a)?;
    let (model, _, body, data) = list[m].clone();
    let (bends, _) = rows(od, tab, &r, &body, &data)?;
    let bend = pick_bend(&bends, a)?;
    let face = sm::bend_face(&data.bends[bend]).ok_or_else(|| OpError::invalid("the bend has no faces").with_param("bend"))?;
    edit_studio(od, tab, sm::CONVERT_TO_RIP, |ps| {
        sm::convert_to_rip(ps, model, face, &units);
        Ok(())
    })?;
    Ok(json!({ "regeneration": regen_report(od, tab)? }).into())
}

fn export_flat(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let r = od.regenerate(tab, None)?;
    let part = super::query::find_part(&r, a.str("part")?)?;
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &part.id).map_err(|e| OpError::invalid(e).with_param("part"))?;
    let bytes = match a.opt_str("format")?.unwrap_or("DXF") {
        "SVG" => noshape_model::drawing::export::to_svg(&flat.to_sheet_geometry()).into_bytes(),
        _ => noshape_model::sheetmetal::flat_dxf::flat_dxf(&flat, &Default::default()).map_err(OpError::invalid)?,
    };
    let file = a.str("file")?;
    std::fs::write(file, &bytes).map_err(|e| OpError::param("file", &e.to_string()))?;
    Ok(json!({ "file": file, "bytes": bytes.len(), "bend_lines": flat.bend_lines.len() }).into())
}
