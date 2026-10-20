//! Part Studio features: every feature type of the feature registry, with
//! the parameters its dialog has.

use super::{doc_field, edit_studio, feature_arg, features_arg, regen_report, s, strings, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, doc, studio, values};
use noshape_model::features::{self, Category};
use noshape_model::params::ParamKind;
use serde_json::{Value, json};

const PARAMS_DOC: &str = "Parameter values by parameter id (see `feature_schema`): booleans; enum option ids (\"BLIND\"); numbers as expression strings with units (\"25 mm\", \"0.5 in\", \"30 deg\", \"#width / 2\", \"2 * #t + 1 mm\"; a bare number uses the workspace units); selections as arrays of reference ids from the query and sketch tools (\"region:…\", \"face:…\", \"edge:…\", \"plane:top\", \"body:…\"); array parameters as arrays of objects of their item parameters.";

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "feature",
            verb: "types",
            summary: "List the Part Studio feature types",
            description: "Every feature of the Part Studio toolbar and tool search (Extrude, Revolve, Fillet, Hole, Sheet metal, Frames...), plus the document's custom FeatureScript features. Use the `type` id with `feature_add`.",
            fields: vec![doc_field(), opt("category", s(), "Only this category: Sketch, Solid (Create), Detail, Pattern, Transform, Direct, Surface, Curve, Reference, SheetMetal, Frame, Variable, Import, Custom.")],
            mutates: false,
            destructive: false,
            run: types,
        },
        Op {
            group: "feature",
            verb: "schema",
            summary: "The parameters of a feature type (JSON schema, defaults, options, units)",
            description: "The same parameters, defaults, options and visibility rules as the feature's dialog. Parameters shown only for some values of others (e.g. `depth` only when `endBound` is BLIND) are ignored while hidden.",
            fields: vec![doc_field(), req("type", s(), "Feature type id, e.g. \"extrude\".")],
            mutates: false,
            destructive: false,
            run: schema,
        },
        Op {
            group: "feature",
            verb: "add",
            summary: "Add a feature to a Part Studio (what accepting its dialog does)",
            description: "Creates the feature with the dialog's defaults, applies `params`, inserts it at the rollback bar (or `index`) and regenerates. As in Onshape, a feature that fails is still added and shown with its error: check `feature_status` and `failed` in the reply. Sketches are made with `sketch_create`.",
            fields: vec![
                doc_field(),
                tab_field(),
                req("type", s(), "Feature type id (see `feature_types`)."),
                opt("params", json!({ "type": "object" }), PARAMS_DOC),
                opt("name", s(), "Feature name (default: \"Extrude 1\", ...)."),
                opt("index", json!({ "type": "integer", "minimum": 0 }), "Position in the feature list (default: at the rollback bar)."),
            ],
            mutates: true,
            destructive: false,
            run: add,
        },
        Op {
            group: "feature",
            verb: "edit",
            summary: "Change a feature's parameters or name (what Edit and accepting its dialog does)",
            description: "Only the given parameters change. Configured parameters go to the active (default) configuration's value, as in the dialog.",
            fields: vec![doc_field(), tab_field(), req("feature", s(), "Feature id or name."), opt("params", json!({ "type": "object" }), PARAMS_DOC), opt("name", s(), "New name.")],
            mutates: true,
            destructive: false,
            run: edit,
        },
        Op {
            group: "feature",
            verb: "get",
            summary: "A feature's type, parameters and regeneration state",
            description: "Parameters in the same JSON forms `feature_add` takes; `visible` lists the ones its dialog currently shows.",
            fields: vec![doc_field(), tab_field(), req("feature", s(), "Feature id or name.")],
            mutates: false,
            destructive: false,
            run: get,
        },
        Op {
            group: "feature",
            verb: "list",
            summary: "The feature list: every feature with its status, the parts, failures",
            description: "Regenerates the Part Studio and reports each feature (id, name, type, status Ok/Info/Warning/Error/Suppressed/RolledBack, error message), the parts and the rollback bar.",
            fields: vec![doc_field(), tab_field()],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "feature",
            verb: "delete",
            summary: "Delete features",
            description: "Features that use them fail with \"missing selection\" errors, as in Onshape.",
            fields: vec![doc_field(), tab_field(), req("features", strings(), "Feature ids or names.")],
            mutates: true,
            destructive: true,
            run: delete,
        },
        Op {
            group: "feature",
            verb: "suppress",
            summary: "Suppress or unsuppress features",
            description: "",
            fields: vec![doc_field(), tab_field(), req("features", strings(), "Feature ids or names."), opt("suppressed", json!({ "type": "boolean", "default": true }), "true: suppress, false: unsuppress.")],
            mutates: true,
            destructive: false,
            run: suppress,
        },
        Op {
            group: "feature",
            verb: "rollback",
            summary: "Move the rollback bar",
            description: "Give `after` (the bar goes below that feature), `index` (number of features before the bar) or neither (to the end). New features are inserted at the bar.",
            fields: vec![doc_field(), tab_field(), opt("after", s(), "Feature id or name."), opt("index", json!({ "type": "integer", "minimum": 0 }), "Features above the bar.")],
            mutates: true,
            destructive: false,
            run: rollback,
        },
        Op {
            group: "feature",
            verb: "rename",
            summary: "Rename a feature",
            description: "",
            fields: vec![doc_field(), tab_field(), req("feature", s(), "Feature id or name."), req("name", s(), "New name.")],
            mutates: true,
            destructive: false,
            run: rename,
        },
        Op {
            group: "feature",
            verb: "reorder",
            summary: "Move features to another place in the list",
            description: "Like dragging rows in the feature list: refused when a feature would move above something it depends on.",
            fields: vec![doc_field(), tab_field(), req("features", strings(), "Feature ids or names to move."), req("to", json!({ "type": "integer", "minimum": 0 }), "Row index to drop them at.")],
            mutates: true,
            destructive: false,
            run: reorder,
        },
        Op {
            group: "feature",
            verb: "set_visibility",
            summary: "Hide or show parts, sketches, planes, mate connectors or default geometry",
            description: "Targets are reference ids: `body:…` parts (or any face/edge of one), `sketch:…`, `fplane:…`, `mc:…`, `feature:…` (a feature that made parts hides its parts), `plane:top|front|right`, `origin`.",
            fields: vec![doc_field(), tab_field(), req("targets", strings(), "Reference ids."), opt("hidden", json!({ "type": "boolean", "default": true }), "true: hide, false: show.")],
            mutates: true,
            destructive: false,
            run: set_visibility,
        },
        Op {
            group: "feature",
            verb: "show_all",
            summary: "Show all parts, sketches, planes and default geometry",
            description: "",
            fields: vec![doc_field(), tab_field()],
            mutates: true,
            destructive: false,
            run: show_all,
        },
    ]
}

fn category_name(c: Category) -> String {
    format!("{c:?}")
}

fn types(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    // Custom features need the document's Feature Studios compiled.
    let custom = match ctx.doc(a.opt_str("doc")?) {
        Ok(od) => noshape_model::custom::DocumentScripts::from_document(od.doc()).feature_types(),
        Err(_) => Vec::new(),
    };
    let cat = a.opt_str("category")?;
    let mut out: Vec<Value> = features::all()
        .chain(custom.iter().copied())
        .filter(|t| cat.is_none_or(|c| category_name(t.category).eq_ignore_ascii_case(c) || t.category.label().eq_ignore_ascii_case(c)))
        .map(|t| json!({ "type": t.id, "name": t.name, "category": category_name(t.category), "description": t.description, "shortcut": t.shortcut }))
        .collect();
    out.sort_by(|x, y| x["category"].as_str().cmp(&y["category"].as_str()).then(x["name"].as_str().cmp(&y["name"].as_str())));
    Ok(json!({ "feature_types": out }).into())
}

/// A feature type by id, compiling the document's Feature Studios for `custom:` ids.
fn feature_type(ctx: &mut Ctx, a: &Args, id: &str) -> Result<&'static features::FeatureType, OpError> {
    if noshape_model::custom::is_custom(id)
        && let Ok(od) = ctx.doc(a.opt_str("doc")?)
    {
        let _ = noshape_model::custom::DocumentScripts::from_document(od.doc());
    }
    features::get(id).ok_or_else(|| OpError::param("type", &format!("unknown feature type `{id}`; see feature_types")))
}

fn schema(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let ty = feature_type(ctx, a, a.str("type")?)?;
    let units = ctx.doc(a.opt_str("doc")?).map(|od| od.doc().settings.units).unwrap_or_default();
    Ok(json!({
        "type": ty.id,
        "name": ty.name,
        "category": category_name(ty.category),
        "description": ty.description,
        "params": values::feature_schema(&ty.params, &units),
        "order": ty.params.iter().filter(|p| !matches!(p.kind, ParamKind::Sketch)).map(|p| p.id).collect::<Vec<_>>(),
    })
    .into())
}

fn feature_json(f: &noshape_model::Feature) -> Value {
    let specs = f.feature_type().map(|t| &t.params[..]);
    let mut v = json!({
        "id": f.id.to_string(),
        "name": f.name,
        "type": f.type_id,
        "params": values::values_to_json(specs, &f.params),
        "suppressed": f.suppressed,
        "hidden": f.hidden,
    });
    if let Some(specs) = specs {
        v["visible"] = json!(values::visible_ids(specs, &f.params));
    }
    if let Some(e) = &f.suppression_expr {
        v["suppression_expression"] = json!(e);
    }
    v
}

fn this_feature_status(report: &Value, id: &str) -> Value {
    report["features"].as_array().and_then(|fs| fs.iter().find(|f| f["id"] == id)).cloned().unwrap_or(Value::Null)
}

fn add(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let type_id = a.str("type")?.to_string();
    if type_id == "sketch" {
        return Err(OpError::param("type", "make sketches with sketch_create, then draw with the sketch tools"));
    }
    let ty = feature_type(ctx, a, &type_id)?;
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let ps = doc::studio(od.doc(), tab)?;
    let mut feature = ps.new_feature(ty.id, &units).ok_or_else(|| OpError::param("type", "unknown feature type"))?;
    let params = a.object("params")?.cloned().unwrap_or_default();
    values::apply(&ty.params, &mut feature.params, &params, &units, od.doc()).map_err(|e| e.under("params"))?;
    // Plane: the type follows the selection when none is given (cPlaneLogic).
    if ty.id == "cPlane" && !params.contains_key("cplaneType") && !feature.params.query("entities").is_empty() {
        let r = od.regenerate(tab, None)?;
        noshape_model::regen::features::cplane::cplane_logic(&r.context, &mut feature.params);
    }
    if let Some(n) = a.opt_str("name")? {
        feature.name = n.trim().to_string();
        feature.custom_name = true;
    }
    let ps = doc::studio(od.doc(), tab)?;
    let index = match a.opt_u64("index")? {
        Some(i) => (i as usize).min(ps.features.len()),
        None => ps.rollback_index(),
    };
    let (id, name) = (feature.id, feature.name.clone());
    edit_studio(od, tab, &studio::insert_description(&name), |ps| Ok(studio::insert_feature(ps, feature, index)))?;
    let report = regen_report(od, tab)?;
    let st = this_feature_status(&report, &id.to_string());
    Ok(json!({ "feature": id.to_string(), "name": name, "feature_status": st, "regeneration": report }).into())
}

fn edit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let ps = doc::studio(od.doc(), tab)?;
    let id = feature_arg(ps, a, "feature")?;
    let index = ps.index_of(id).expect("found");
    // The dialog edits the active configuration's values.
    let config = ps.configuration.defaults();
    let mut feature = ps.configuration.for_editing(&ps.features[index], &config);
    let ty = feature.feature_type().ok_or_else(|| OpError::invalid(format!("feature type `{}` is not available", feature.type_id)))?;
    if let Some(params) = a.object("params")? {
        if feature.type_id == "sketch" && params.contains_key("sketch") {
            return Err(OpError::param("params.sketch", "edit sketch geometry with the sketch tools"));
        }
        values::apply(&ty.params, &mut feature.params, params, &units, od.doc()).map_err(|e| e.under("params"))?;
    }
    if let Some(n) = a.opt_str("name")? {
        feature.name = n.trim().to_string();
        feature.custom_name = true;
    }
    let name = feature.name.clone();
    edit_studio(od, tab, &studio::edit_description(&name), |ps| {
        studio::edit_feature(ps, index, feature, &config);
        Ok(())
    })?;
    let report = regen_report(od, tab)?;
    let st = this_feature_status(&report, &id.to_string());
    Ok(json!({ "feature": id.to_string(), "name": name, "feature_status": st, "regeneration": report }).into())
}

fn get(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let id = feature_arg(ps, a, "feature")?;
    let mut v = feature_json(ps.feature(id).expect("found"));
    let report = regen_report(od, tab)?;
    v["status"] = this_feature_status(&report, &id.to_string());
    Ok(v.into())
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut r = regen_report(od, tab)?;
    let ps = doc::studio(od.doc(), tab)?;
    r["tab"] = json!({ "id": tab.to_string(), "name": od.doc().tab(tab).map(|t| t.name.clone()) });
    r["folders"] = json!(ps.folders.iter().map(|f| json!({ "id": f.id, "name": f.name, "features": ps.features.iter().filter(|x| x.folder == Some(f.id)).map(|x| x.id.to_string()).collect::<Vec<_>>() })).collect::<Vec<_>>());
    Ok(r.into())
}

fn delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let ids = features_arg(ps, a, "features")?;
    let desc = studio::delete_description(ps, &ids);
    edit_studio(od, tab, &desc, |ps| {
        studio::delete_features(ps, &ids);
        Ok(())
    })?;
    Ok(json!({ "deleted": ids.iter().map(|i| i.to_string()).collect::<Vec<_>>(), "regeneration": regen_report(od, tab)? }).into())
}

fn suppress(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let ids = features_arg(ps, a, "features")?;
    let on = a.bool_or("suppressed", true)?;
    edit_studio(od, tab, studio::suppress_description(on), |ps| {
        studio::set_suppressed(ps, &ids, on);
        Ok(())
    })?;
    Ok(json!({ "regeneration": regen_report(od, tab)? }).into())
}

fn rollback(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let index = match (a.opt_str("after")?, a.opt_u64("index")?) {
        (Some(_), _) => Some(ps.index_of(feature_arg(ps, a, "after")?).expect("found") + 1),
        (None, Some(i)) => Some(i as usize),
        (None, None) => None,
    };
    edit_studio(od, tab, studio::ROLLBACK_DESCRIPTION, |ps| {
        studio::set_rollback(ps, index);
        Ok(())
    })?;
    Ok(json!({ "regeneration": regen_report(od, tab)? }).into())
}

fn rename(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let id = feature_arg(ps, a, "feature")?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "must not be empty"));
    }
    edit_studio(od, tab, &studio::rename_description(&name), |ps| {
        studio::rename_feature(ps, id, &name);
        Ok(())
    })?;
    Ok(json!({ "feature": id.to_string(), "name": name }).into())
}

fn reorder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::interaction;
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let moving = features_arg(ps, a, "features")?;
    let to = a.opt_u64("to")?.unwrap_or(0) as usize;
    let order = interaction::reordered(&ps.features, &moving, to).ok_or_else(|| OpError::param("to", "nothing to move there"))?;
    interaction::check_order(&ps.features, &order).map_err(|m| OpError::invalid(m).with_param("to"))?;
    let desc = if moving.len() == 1 { "Reorder feature" } else { "Reorder features" };
    edit_studio(od, tab, desc, |ps| {
        interaction::apply_order(ps, &order);
        Ok(())
    })?;
    Ok(json!({ "regeneration": regen_report(od, tab)? }).into())
}

fn set_visibility(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let targets = a.array("targets")?.iter().enumerate().map(|(i, v)| crate::refs::parse(v).map_err(|e| e.with_param(format!("targets[{i}]")))).collect::<Result<Vec<_>, _>>()?;
    let hidden = a.bool_or("hidden", true)?;
    let r = od.regenerate(tab, None)?;
    let t = studio::visibility_targets(
        &targets,
        |f| r.display.bodies.iter().filter(|b| b.id.feature == f).map(|b| b.id.clone()).collect(),
        |rf| crate::ops::query::body_of(&r.display, rf),
    );
    if t.is_empty() {
        return Err(OpError::param("targets", "nothing to hide or show"));
    }
    edit_studio(od, tab, studio::visibility_description(hidden), |ps| {
        studio::set_visibility(ps, &t, hidden);
        Ok(())
    })?;
    Ok(json!({ "parts": t.bodies.iter().map(crate::refs::body_id).collect::<Vec<_>>(), "features": t.features.iter().map(|f| f.to_string()).collect::<Vec<_>>(), "hidden": hidden }).into())
}

fn show_all(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    edit_studio(od, tab, studio::SHOW_ALL_DESCRIPTION, |ps| {
        studio::show_all(ps);
        Ok(())
    })?;
    Ok(json!({ "ok": true }).into())
}
