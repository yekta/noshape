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
            fields: vec![
                doc_field(),
                opt("category", s(), "Only this category: Sketch, Solid (Create), Detail, Pattern, Transform, Direct, Surface, Curve, Reference, SheetMetal, Frame, Variable, Import, Custom."),
                opt("search", s(), "Only types whose name has every word of this (case-insensitive, word starts: \"move face\", \"fill\"), or whose id it is; when none, those whose description contains the words."),
            ],
            mutates: false,
            destructive: false,
            run: types,
        },
        Op {
            group: "feature",
            verb: "schema",
            summary: "The parameters of a feature type (JSON schema, defaults, options, units)",
            description: "The same parameters, defaults, options and visibility rules as the feature's dialog. Parameters shown only for some values of others are ignored while hidden; each says whether the dialog shows it with the defaults and which other parameter's values show or hide it (e.g. hole `holeDepthV3`: \"shown with the defaults; hidden, and ignored, when endStyleV2 is UP_TO_NEXT or UP_TO_ENTITY or THROUGH\"). `feature_get` lists a feature's currently `visible` parameters.",
            fields: vec![doc_field(), req("type", s(), "Feature type id, e.g. \"extrude\".")],
            mutates: false,
            destructive: false,
            run: schema,
        },
        Op {
            group: "feature",
            verb: "add",
            summary: "Add a feature to a Part Studio (what accepting its dialog does)",
            description: "Creates the feature with the dialog's defaults, applies `params`, makes the dialog's own choices for the fields not given (Extrude / Revolve / Sweep / Loft: Add into the one part the new body touches, with that part as merge scope, else New; a Remove Extrude on a part points into it; Thicken from the faces' parts; Hole: merge scope = the parts it reaches, flipped when more lie behind; a tapped hole's linked depths), inserts it at the rollback bar (or `index`) and regenerates. As in Onshape, a feature that fails is still added and shown with its error: check `feature_status` and `failed` in the reply. Sketches are made with `sketch_create`.",
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
            description: "Only the given parameters change. Configured parameters go to the active configuration's value (config_activate), as in the dialog.",
            fields: vec![doc_field(), tab_field(), req("feature", s(), "Feature id or name."), opt("params", json!({ "type": "object" }), PARAMS_DOC), opt("name", s(), "New name.")],
            mutates: true,
            destructive: false,
            run: edit,
        },
        Op {
            group: "feature",
            verb: "get",
            summary: "A feature's type, parameters and regeneration state",
            description: "Parameters in the same JSON forms `feature_add` takes; `visible` lists the ones its dialog currently shows. A custom (FeatureScript) feature also has the `notices` of its last run (message, severity, stack of `at` line:col and `in` module (function), as the FeatureScript notices panel's Monitor shows them) and its `println` `output`.",
            fields: vec![doc_field(), tab_field(), req("feature", s(), "Feature id or name.")],
            mutates: false,
            destructive: false,
            run: get,
        },
        Op {
            group: "feature",
            verb: "list",
            summary: "The feature list: every feature with its status, the parts, failures",
            description: "Regenerates the Part Studio and reports each feature (id, name, type, status Ok/Info/Warning/Error/Suppressed/RolledBack, error message; custom features their FeatureScript `notices` and `output`), the parts and the rollback bar.",
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
            description: "Unsuppress also removes a dynamic suppression expression (feature_dynamic_suppression), like the menu's Unsuppress on such a feature.",
            fields: vec![doc_field(), tab_field(), req("features", strings(), "Feature ids or names."), opt("suppressed", json!({ "type": "boolean", "default": true }), "true: suppress, false: unsuppress.")],
            mutates: true,
            destructive: false,
            run: suppress,
        },
        Op {
            group: "feature",
            verb: "dynamic_suppression",
            summary: "Set or remove a feature's dynamic suppression expression",
            description: "The Dynamic suppression dialog (a feature's Edit expression… / Remove expression): the feature is active while `unsuppressed_if` is true (an expression of variables and configuration inputs, e.g. \"#holes > 2\"). Empty or omitted removes the expression (feature_suppress with suppressed false does too). With `folder` instead of `feature`: the folder row's Edit dynamic suppression, written onto every feature in the folder.",
            fields: vec![doc_field(), tab_field(), opt("feature", s(), "Feature id or name."), opt("folder", s(), "Instead of `feature`: a folder's name (feature_list lists folders)."), opt("unsuppressed_if", s(), "Unsuppressed if: expression.")],
            mutates: true,
            destructive: false,
            run: dynamic_suppression,
        },
        Op {
            group: "feature",
            verb: "dependencies",
            summary: "Show dependencies: the features a feature uses and the features using it",
            description: "Parents are the earlier features whose geometry, sketches, planes or #variables the feature references; children are the later features referencing it. In history order.",
            fields: vec![doc_field(), tab_field(), req("feature", s(), "Feature id or name.")],
            mutates: false,
            destructive: false,
            run: dependencies,
        },
        Op {
            group: "feature",
            verb: "add_folder",
            summary: "Add features to a new folder",
            description: "Add selection to folder… (or, without `features`, the list's New folder: an empty folder at the rollback bar). Folders hold a contiguous run: the features gather after the first one. Features all in one folder make a nested folder. feature_list lists folders.",
            fields: vec![doc_field(), tab_field(), opt("features", strings(), "Feature ids or names."), opt("name", json!({ "type": "string", "default": studio::NEW_FOLDER_NAME }), "Folder name.")],
            mutates: true,
            destructive: false,
            run: add_folder,
        },
        Op {
            group: "feature",
            verb: "rename_folder",
            summary: "Rename a feature folder",
            description: "",
            fields: vec![doc_field(), tab_field(), req("folder", json!({ "type": ["integer", "string"] }), "Folder id or name (feature_list)."), req("name", s(), "New name.")],
            mutates: true,
            destructive: false,
            run: rename_folder,
        },
        Op {
            group: "feature",
            verb: "unpack_folder",
            summary: "Unpack a feature folder",
            description: "Removes the folder; its features (and sub-folders) stay in place, in the enclosing folder.",
            fields: vec![doc_field(), tab_field(), req("folder", json!({ "type": ["integer", "string"] }), "Folder id or name (feature_list).")],
            mutates: true,
            destructive: false,
            run: unpack_folder,
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
            verb: "size_plane",
            summary: "Size a plane to the Part Studio (plane menu)",
            description: "Size to Part Studio: the plane is drawn large enough to cover the shown parts (10 % over). `plane`: `plane:top|front|right` or a Plane feature's `fplane:…`.",
            fields: vec![doc_field(), tab_field(), req("plane", s(), "Plane reference id.")],
            mutates: true,
            destructive: false,
            run: size_plane,
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

/// What the feature's dialog decides on its own for a new feature, for the
/// fields not given (as the Part Studio's dialog does, with the same shared
/// rules): a tapped hole's linked depths, Thicken's / Extrude's operation and
/// flip from the parts the profiles lie on, then from a preview the
/// operation and merge scope of Extrude / Revolve / Sweep / Loft (Add into
/// the one part the tool touches) and a Hole's scope and flip. Features
/// whose dialog runs std's editing logic run it instead.
fn dialog_logic(od: &mut crate::OpenDoc, tab: noshape_model::document::TabId, feature: &mut noshape_model::Feature, index: usize, given: &serde_json::Map<String, Value>) -> Result<(), OpError> {
    let set = |k: &str| given.contains_key(k);
    let units = od.doc().settings.units;
    let apply = |f: &mut noshape_model::Feature, changes: studio::AutoChanges| -> bool {
        let mut any = false;
        for (k, v) in changes {
            if !set(k) && f.params.get(k) != Some(&v) {
                f.params.set(k, v);
                any = true;
            }
        }
        any
    };
    if feature.type_id == "hole" {
        let ps = doc::studio(od.doc(), tab)?;
        if let Some(defaults) = ps.new_feature(&feature.type_id, &units) {
            let linked = noshape_model::features::hole_tables::link_tapped_depths(&defaults.params, &feature.params, &units);
            apply(feature, linked);
        }
    }
    // Features whose dialog runs std's "Editing Logic Function" (Extrude,
    // Fillet, Loft: the app's `std_edit_logic`) get the same here, on the
    // model before the feature, until it settles.
    if noshape_model::custom::std_feature::has_editing_logic(&feature.type_id) {
        let ctx = od.regenerate_with(tab, None, |o| o.upto = Some(index))?.context;
        let specified: Vec<String> = given.keys().cloned().collect();
        let mut old = noshape_model::params::ParamValues::default();
        for _ in 0..3 {
            let Some(changed) = noshape_model::custom::std_feature::editing_logic_in((*ctx).clone(), &feature.type_id, feature.id, &feature.name, &units, &old, &feature.params, &specified, true) else { break };
            old = feature.params.clone();
            let mut any = false;
            for (k, v) in changed.0 {
                if !set(&k) && feature.params.get(&k) != Some(&v) {
                    feature.params.set(&k, v);
                    any = true;
                }
            }
            if !any {
                break;
            }
        }
        return Ok(());
    }
    if !matches!(feature.type_id.as_str(), "extrude" | "revolve" | "sweep" | "loft" | "thicken" | "hole") {
        return Ok(());
    }
    let before = od.regenerate(tab, None)?;
    let ps = doc::studio(od.doc(), tab)?.clone();
    let changes = studio::auto_operation(&ps, &before.display, feature, &set);
    apply(feature, changes);
    // The preview's touched parts drive the rest; each change previews again
    // (a flipped hole reaches other parts), as the open dialog does.
    for _ in 0..3 {
        let mut trial = ps.clone();
        studio::insert_feature(&mut trial, feature.clone(), index);
        let r = od.preview(tab, &trial)?;
        let Some(st) = r.display.feature_state(feature.id).filter(|s| s.status != noshape_model::display::FeatureStatus::RolledBack) else { break };
        let mut changes = studio::boolean_choices(feature, &set, st);
        changes.extend(studio::hole_scope_logic(feature, &set, &st.touched, &st.touched_behind));
        if !apply(feature, changes) {
            break;
        }
    }
    Ok(())
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
    let words: Vec<String> = a.opt_str("search")?.unwrap_or_default().split_whitespace().map(str::to_lowercase).collect();
    let has_all = |text: String| words.iter().all(|w| text.contains(w.as_str()));
    // Each word starts a word of the name ("move face": Move face, not Move boundary), or the words spell the id.
    let in_name = |t: &features::FeatureType| {
        let name = t.name.to_lowercase();
        let name_words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        (!words.is_empty() && words.iter().all(|w| name_words.iter().any(|n| n.starts_with(w.as_str())))) || words.concat() == t.id.to_lowercase()
    };
    let in_text = |t: &features::FeatureType| has_all(format!("{} {} {}", t.name, t.id, t.description).to_lowercase());
    let listed: Vec<&features::FeatureType> = features::all().chain(custom.iter().copied()).filter(|t| cat.is_none_or(|c| category_name(t.category).eq_ignore_ascii_case(c) || t.category.label().eq_ignore_ascii_case(c))).collect();
    // Types named by the words; else those whose descriptions have them.
    let by_name = listed.iter().any(|t| in_name(t));
    let mut out: Vec<Value> = listed
        .into_iter()
        .filter(|t| if by_name { in_name(t) } else { in_text(t) })
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
        "params": values::feature_schema(&ty.params, &units, noshape_model::partstudio::PartStudio::new().new_feature(ty.id, &units).map(|f| f.params).as_ref()),
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
    dialog_logic(od, tab, &mut feature, index, &params)?;
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
    let config = od.current_config(tab)?;
    let ps = doc::studio(od.doc(), tab)?;
    let mut feature = ps.configuration.for_editing(&ps.features[index], &config);
    load_custom_types(od, &feature.type_id);
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
    let type_id = ps.feature(id).expect("found").type_id.clone();
    load_custom_types(od, &type_id);
    let report = regen_report(od, tab)?;
    let ps = doc::studio(od.doc(), tab)?;
    let f = ps.feature(id).expect("found");
    let mut v = feature_json(f);
    v["status"] = this_feature_status(&report, &id.to_string());
    let tab_name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_default();
    if let Some(o) = super::custom_output(od.doc(), &tab_name, f) {
        v["notices"] = o["notices"].clone();
        v["output"] = o["output"].clone();
    }
    Ok(v.into())
}

/// Custom feature types (`custom:<tab>:<name>`) exist once the document's
/// Feature Studios are compiled in this process (cached by their sources):
/// a fresh CLI process has to before it can read their parameters.
fn load_custom_types(od: &crate::OpenDoc, type_id: &str) {
    if noshape_model::custom::is_custom(type_id) {
        let _ = noshape_model::custom::DocumentScripts::from_document(od.doc());
    }
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut r = regen_report(od, tab)?;
    let ps = doc::studio(od.doc(), tab)?;
    r["tab"] = json!({ "id": tab.to_string(), "name": od.doc().tab(tab).map(|t| t.name.clone()) });
    r["folders"] = json!(ps.folders.iter().map(|f| json!({ "id": f.id, "name": f.name, "parent": f.parent, "features": ps.features.iter().filter(|x| x.folder == Some(f.id)).map(|x| x.id.to_string()).collect::<Vec<_>>() })).collect::<Vec<_>>());
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
        |rf| crate::studio::body_of(&r.display, rf),
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

fn dynamic_suppression(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let expr = a.opt_str("unsuppressed_if")?.map(str::trim).filter(|e| !e.is_empty()).map(str::to_string);
    if let Some(name) = a.opt_str("folder")? {
        if a.get("feature").is_some() {
            return Err(OpError::param("folder", "give `feature` or `folder`"));
        }
        let folder = ps.folders.iter().find(|f| f.name == name || f.name.eq_ignore_ascii_case(name)).map(|f| f.id).ok_or_else(|| OpError::not_found(format!("no folder `{name}`")).with_param("folder"))?;
        edit_studio(od, tab, studio::suppression_expr_description(expr.as_deref()), |ps| {
            ps.set_folder_suppression_expr(folder, expr.clone());
            Ok(())
        })?;
        return Ok(json!({ "folder": name, "unsuppressed_if": expr, "regeneration": regen_report(od, tab)? }).into());
    }
    let id = feature_arg(ps, a, "feature")?;
    edit_studio(od, tab, studio::suppression_expr_description(expr.as_deref()), |ps| {
        studio::set_suppression_expr(ps, id, expr.clone());
        Ok(())
    })?;
    Ok(json!({ "feature": id.to_string(), "unsuppressed_if": expr, "regeneration": regen_report(od, tab)? }).into())
}

fn dependencies(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::interaction;
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let id = feature_arg(ps, a, "feature")?;
    let idx = ps.index_of(id).expect("found");
    let rows = |ids: Vec<noshape_model::FeatureId>| -> Vec<Value> { ids.into_iter().filter_map(|i| ps.feature(i)).map(|f| json!({ "id": f.id.to_string(), "name": f.name, "type": f.type_id })).collect() };
    Ok(json!({
        "feature": id.to_string(),
        "parents": rows(interaction::feature_dependencies(&ps.features, idx)),
        "children": rows(interaction::feature_dependents(&ps.features, idx)),
    })
    .into())
}

/// A `folder` argument: folder id or name.
fn folder_arg(ps: &noshape_model::PartStudio, a: &Args) -> Result<u32, OpError> {
    let found = match a.get("folder") {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| ps.folder(n as u32)),
        Some(Value::String(t)) => ps.folders.iter().find(|f| f.name == *t).or_else(|| t.parse().ok().and_then(|n| ps.folder(n))),
        _ => return Err(OpError::param("folder", "a folder id or name")),
    };
    found.map(|f| f.id).ok_or_else(|| OpError::param("folder", "no such folder (see feature_list)"))
}

fn add_folder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let ids = if a.get("features").is_some() { features_arg(ps, a, "features")? } else { Vec::new() };
    let name = a.opt_str("name")?.unwrap_or(studio::NEW_FOLDER_NAME).to_string();
    let mut folder = 0;
    edit_studio(od, tab, studio::ADD_FOLDER_DESCRIPTION, |ps| {
        folder = studio::add_folder(ps, &ids, &name);
        Ok(())
    })?;
    let ps = doc::studio(od.doc(), tab)?;
    let f = ps.folder(folder).expect("added");
    Ok(json!({ "folder": folder, "name": f.name, "parent": f.parent, "features": ps.features.iter().filter(|x| x.folder == Some(folder)).map(|x| x.id.to_string()).collect::<Vec<_>>() }).into())
}

fn rename_folder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let folder = folder_arg(ps, a)?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "must not be empty"));
    }
    edit_studio(od, tab, &studio::rename_folder_description(&name), |ps| {
        studio::rename_folder(ps, folder, &name);
        Ok(())
    })?;
    Ok(json!({ "folder": folder, "name": name }).into())
}

fn unpack_folder(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let ps = doc::studio(od.doc(), tab)?;
    let folder = folder_arg(ps, a)?;
    edit_studio(od, tab, studio::UNPACK_FOLDER_DESCRIPTION, |ps| {
        ps.dissolve_folder(folder);
        Ok(())
    })?;
    Ok(json!({ "unpacked": folder }).into())
}

fn size_plane(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let r = crate::refs::parse_str(a.str("plane")?).filter(|r| matches!(r, noshape_model::reference::Reference::Plane(_) | noshape_model::reference::Reference::FeaturePlane(_))).ok_or_else(|| OpError::param("plane", "`plane:top|front|right` or `fplane:…`"))?;
    let shown = od.regenerate(tab, None)?;
    if !shown.display.planes.iter().any(|p| p.reference == r) {
        return Err(OpError::param("plane", "no such plane in this Part Studio"));
    }
    let (hw, hh) = studio::plane_size_to_studio(&shown.display, &r).ok_or_else(|| OpError::invalid("no shown parts to size the plane to"))?;
    edit_studio(od, tab, studio::SIZE_PLANE_DESCRIPTION, |ps| {
        studio::set_plane_size(ps, &r, hw, hh);
        Ok(())
    })?;
    let k = crate::ops::query::len_scale(&od.doc().settings.units);
    Ok(json!({ "plane": a.str("plane")?, "half_width": hw * k, "half_height": hh * k }).into())
}
