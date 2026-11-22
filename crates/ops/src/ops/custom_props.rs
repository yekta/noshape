//! Company settings › Custom properties through the agent API: list,
//! create, edit and delete definitions (drawing.md A15). Items' values are
//! set with their properties ops (`custom`).

use crate::Ctx;
use crate::custom_properties::{CustomKind, CustomProperty, CustomState, CustomTarget, custom_properties, set_custom_properties};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use super::{s, strings};
use noshape_model::release::ItemKind;
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    vec![
    Op {
        group: "company",
        verb: "custom_properties",
        summary: "Company settings › Custom properties: list, create, edit, delete",
        description: "Custom property definitions (kept for every document on this computer, as a company's are for its documents). Without `action`, lists them. `create` adds `name` with `type` (string, integer, double, boolean, date, enum with `values`), `applies` (part, assembly, drawing; default [part]), `state` (pending, active, inactive; default active), `default` and `description`; `edit` changes the fields given of property `name` (`new_name` renames it; items keep values under the old name); `delete` removes it. Active properties show at the bottom of the Properties dialogs of what they apply to and in drawings' Insert property lists (`{part.custom.<name>}`, `{drawing.custom.<name>}`); values are set with part_set_properties / drawing_element_properties `custom`.",
        fields: vec![
            opt("action", json!({ "type": "string", "enum": ["create", "edit", "delete"] }), "What to do (default: list)."),
            opt("name", json!({ "type": "string" }), "The property's name."),
            opt("new_name", json!({ "type": "string" }), "edit: a new name."),
            opt("type", json!({ "type": "string", "enum": ["string", "integer", "double", "boolean", "date", "enum"] }), "Value type."),
            opt("values", json!({ "type": "array", "items": { "type": "string" } }), "enum: the values."),
            opt("applies", json!({ "type": "array", "items": { "type": "string", "enum": ["part", "assembly", "drawing"] } }), "What it applies to."),
            opt("state", json!({ "type": "string", "enum": ["pending", "active", "inactive"] }), "Publish state (only active ones show)."),
            opt("default", json!({ "type": "string" }), "Default value."),
            opt("description", json!({ "type": "string" }), "Description."),
        ],
        mutates: false,
        destructive: false,
        run: custom_props,
    },
    Op {
        group: "company",
        verb: "release_settings",
        summary: "Company settings › Release management: read or change",
        description: "Release management settings (kept for every document on this computer, as a company's are for its documents): the revision scheme (`scheme`: \"Alphabetical\", \"Numeric\", …, or {\"Custom\": [labels]}), `unreleased_suffix`, `managed_workflows`, `access` [3], `require_new_revision`, watermarks (`show_watermark`, `hide_watermark_unmanaged`, `watermarks`), release `conditions` [8], the Create Release candidate dialog's `require_approver`, `no_creator_approver`, `require_all_approvers`, `require_note`, `allow_other_documents`, `custom_display_names` / `display_names` [4], `auto_obsolete` and the `revision_table` template. Without `set`, returns them; `set` changes the fields it names (as Save release settings).",
        fields: vec![opt("set", json!({ "type": "object" }), "The fields to change, as returned.")],
        mutates: false,
        destructive: false,
        run: release_settings,
    },
    Op {
        group: "company",
        verb: "numbering_schemes",
        summary: "Company settings › Numbering schemes: read or change",
        description: "Numbering scheme settings: `release_objects`, `standard_content`, `items` and `release_names` (each its number generation, prefixes, minimum length and next number), `drawing_reuse` and `propagation`. Without `set`, returns them; `set` changes the fields it names (as Save).",
        fields: vec![opt("set", json!({ "type": "object" }), "The fields to change, as returned.")],
        mutates: false,
        destructive: false,
        run: numbering_schemes,
    },
    Op {
        group: "company",
        verb: "revisions",
        summary: "Company settings › Revision tools › Find revisions: a part number's revisions of one object type",
        description: "Revision tools › Reset part number type › Find revisions: `part_number` and `object_type` (Part, Assembly, Drawing or Blob), the revisions released with them across the documents on this computer (recently opened, the documents folder, the watched folders and the documents open here; `documents` lists others), newest first: part number, type, revision, document name, release name, state, created, re-releasable and can change type.",
        fields: vec![
            req("part_number", s(), "Part number."),
            req("object_type", object_type(), "Object type."),
            opt("documents", strings(), "Search these documents (paths) instead."),
        ],
        mutates: false,
        destructive: false,
        run: revisions,
    },
    Op {
        group: "company",
        verb: "reset_part_number_type",
        summary: "Company settings › Revision tools › Reset part number type",
        description: "Reset part number type (after its confirmation, \"This action cannot be undone.\"): every revision company_revisions finds for `part_number` and `object_type` is marked obsolete and re-releasable, so the part number can be released again with another object type. Revision history still shows the previous revisions. Refused when none is left to reset (the dialog's button is disabled then). Returns the revisions as Find revisions lists them afterwards.",
        fields: vec![
            req("part_number", s(), "Part number."),
            req("object_type", object_type(), "Object type."),
            opt("documents", strings(), "Search these documents (paths) instead."),
        ],
        mutates: true,
        destructive: true,
        run: reset_part_number_type,
    }]
}

/// Object type's choices (Revision tools: a file is a "Blob").
const OBJECT_TYPES: [(&str, ItemKind); 4] = [("Part", ItemKind::Part), ("Assembly", ItemKind::Assembly), ("Drawing", ItemKind::Drawing), ("Blob", ItemKind::File)];

fn object_type() -> Value {
    json!({ "type": "string", "enum": OBJECT_TYPES.map(|(n, _)| n) })
}

/// The part number, object type and documents of a Revision tools call.
fn revision_search(ctx: &Ctx, a: &Args) -> Result<(String, ItemKind, Vec<std::path::PathBuf>), OpError> {
    let number = a.str("part_number")?.trim().to_string();
    if number.is_empty() {
        return Err(OpError::param("part_number", "a part number"));
    }
    let ty = a.str("object_type")?;
    let Some(kind) = OBJECT_TYPES.iter().find(|(n, _)| n.eq_ignore_ascii_case(ty)).map(|(_, k)| *k) else {
        return Err(OpError::param("object_type", "Part, Assembly, Drawing or Blob"));
    };
    let canon = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    let mut push = |p: std::path::PathBuf| {
        let c = canon(&p);
        if c.extension().is_some_and(|e| e == "noshape") && c.exists() && !paths.contains(&c) {
            paths.push(c);
        }
    };
    if let Some(list) = a.opt_array("documents")? {
        for (i, v) in list.iter().enumerate() {
            let path = std::path::PathBuf::from(v.as_str().unwrap_or_default());
            if !path.exists() {
                return Err(OpError::param(&format!("documents[{i}]"), "no such document"));
            }
            push(path);
        }
    } else {
        ctx.open_paths().into_iter().for_each(|p| push(p.to_path_buf()));
        crate::doc_library::recent().into_iter().for_each(&mut push);
        for dir in std::iter::once(noshape_scene::documents_dir()).chain(crate::doc_library::watched_folders()) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                rd.flatten().for_each(|e| push(e.path()));
            }
        }
    }
    Ok((number, kind, paths))
}

fn found_json(number: &str, kind: ItemKind, found: &[crate::release::FoundRevision]) -> Value {
    let label = OBJECT_TYPES.iter().find(|(_, k)| *k == kind).map_or("", |(n, _)| *n);
    let rows: Vec<Value> = found
        .iter()
        .map(|f| {
            let r = &f.revision;
            json!({ "part_number": number, "type": label, "revision": r.revision, "document": f.document, "path": f.path.display().to_string(), "release": r.release_name, "state": r.state.label(), "created": r.created, "re_releasable": r.re_releasable, "can_change_type": r.re_releasable })
        })
        .collect();
    json!({ "part_number": number, "object_type": label, "revisions": rows })
}

fn revisions(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (number, kind, paths) = revision_search(ctx, a)?;
    Ok(found_json(&number, kind, &crate::release::find_revisions(&paths, &number, kind)).into())
}

fn reset_part_number_type(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (number, kind, paths) = revision_search(ctx, a)?;
    let found = crate::release::find_revisions(&paths, &number, kind);
    if !found.iter().any(|f| !f.revision.re_releasable) {
        return Err(OpError::param("part_number", &format!("no revisions of {number} left to reset ({} found)", found.len())));
    }
    let mut docs: Vec<std::path::PathBuf> = found.iter().map(|f| f.path.clone()).collect();
    docs.dedup();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    crate::release::reset_part_number_type(&docs, &number, kind, now).map_err(|e| OpError::new("io", format!("{e:#}")))?;
    Ok(found_json(&number, kind, &crate::release::find_revisions(&paths, &number, kind)).into())
}

/// `current` with the fields of `set` replaced, read back as `T` (an error names the field at fault).
fn merged<T: serde::Serialize + serde::de::DeserializeOwned>(current: &T, set: &Value) -> Result<T, OpError> {
    let Value::Object(changes) = set else { return Err(OpError::param("set", "an object of the fields to change")) };
    let mut v = serde_json::to_value(current).map_err(|e| OpError::new("company", e.to_string()))?;
    for (k, x) in changes {
        let Some(slot) = v.get_mut(k) else { return Err(OpError::param("set", &format!("no setting `{k}`"))) };
        *slot = x.clone();
    }
    serde_json::from_value(v).map_err(|e| OpError::param("set", &e.to_string()))
}

fn release_settings(_ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use crate::release_settings::{release_settings, set_release_settings};
    let mut s = release_settings();
    if let Some(set) = a.get("set") {
        s = merged(&s, set)?;
        set_release_settings(&s);
    }
    Ok(serde_json::to_value(&s).map_err(|e| OpError::new("company", e.to_string()))?.into())
}

fn numbering_schemes(_ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use crate::release::{numbering_schemes, set_numbering_schemes};
    let mut s = numbering_schemes();
    if let Some(set) = a.get("set") {
        s = merged(&s, set)?;
        set_numbering_schemes(&s);
    }
    Ok(serde_json::to_value(&s).map_err(|e| OpError::new("company", e.to_string()))?.into())
}

pub fn def_json(p: &CustomProperty) -> Value {
    let mut v = json!({
        "name": p.name,
        "type": p.kind.label().to_lowercase(),
        "applies": p.applies.iter().map(|t| t.label().to_lowercase()).collect::<Vec<_>>(),
        "state": p.state.label().to_lowercase(),
        "description": p.description,
        "default": p.default,
    });
    if let CustomKind::List(o) = &p.kind {
        v["values"] = json!(o);
    }
    v
}

fn custom_props(_ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let mut list = custom_properties();
    let find = |list: &[CustomProperty], name: &str| list.iter().position(|p| p.name.eq_ignore_ascii_case(name.trim()));
    let strs = |k: &str| -> Result<Option<Vec<String>>, OpError> {
        match a.get(k) {
            None => Ok(None),
            Some(Value::Array(v)) => v.iter().map(|x| x.as_str().map(|s| s.trim().to_string()).ok_or_else(|| OpError::param(k, "a list of texts"))).collect::<Result<Vec<_>, _>>().map(Some),
            Some(_) => Err(OpError::param(k, "a list of texts")),
        }
    };
    let apply = |p: &mut CustomProperty| -> Result<(), OpError> {
        let values = strs("values")?;
        if let Some(t) = a.opt_str("type")? {
            p.kind = match t {
                "string" => CustomKind::Text,
                "integer" => CustomKind::Integer,
                "double" => CustomKind::Number,
                "boolean" => CustomKind::Boolean,
                "date" => CustomKind::Date,
                "enum" => CustomKind::List(values.clone().unwrap_or_default()),
                _ => return Err(OpError::param("type", "string, integer, double, boolean, date or enum")),
            };
        } else if let (CustomKind::List(o), Some(v)) = (&mut p.kind, values) {
            *o = v;
        }
        if matches!(&p.kind, CustomKind::List(o) if o.is_empty()) {
            return Err(OpError::param("values", "an enum needs its values"));
        }
        if let Some(ap) = strs("applies")? {
            p.applies = ap
                .iter()
                .map(|s| match s.as_str() {
                    "part" => Ok(CustomTarget::Part),
                    "assembly" => Ok(CustomTarget::Assembly),
                    "drawing" => Ok(CustomTarget::Drawing),
                    _ => Err(OpError::param("applies", "part, assembly or drawing")),
                })
                .collect::<Result<_, _>>()?;
            if p.applies.is_empty() {
                return Err(OpError::param("applies", "at least one of part, assembly, drawing"));
            }
        }
        if let Some(s) = a.opt_str("state")? {
            p.state = CustomState::ALL.into_iter().find(|x| x.label().eq_ignore_ascii_case(s)).ok_or_else(|| OpError::param("state", "pending, active or inactive"))?;
        }
        if let Some(d) = a.opt_str("description")? {
            p.description = d.to_string();
        }
        if let Some(d) = a.opt_str("default")? {
            p.default = p.check(d).map_err(|e| OpError::param("default", &e))?;
        }
        Ok(())
    };
    match a.opt_str("action")? {
        None => {}
        Some("create") => {
            let name = a.opt_str("name")?.map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| OpError::param("name", "a name"))?.to_string();
            if find(&list, &name).is_some() {
                return Err(OpError::param("name", "a property with that name exists"));
            }
            let mut p = CustomProperty { name, ..Default::default() };
            apply(&mut p)?;
            list.push(p);
        }
        Some("edit") => {
            let i = a.opt_str("name")?.and_then(|n| find(&list, n)).ok_or_else(|| OpError::param("name", "no such custom property"))?;
            let mut p = list[i].clone();
            apply(&mut p)?;
            if let Some(n) = a.opt_str("new_name")?.map(str::trim).filter(|s| !s.is_empty()) {
                if find(&list, n).is_some_and(|j| j != i) {
                    return Err(OpError::param("new_name", "a property with that name exists"));
                }
                p.name = n.to_string();
            }
            list[i] = p;
        }
        Some("delete") => {
            let i = a.opt_str("name")?.and_then(|n| find(&list, n)).ok_or_else(|| OpError::param("name", "no such custom property"))?;
            list.remove(i);
        }
        Some(_) => return Err(OpError::param("action", "create, edit or delete")),
    }
    if a.opt_str("action")?.is_some() {
        set_custom_properties(&list);
    }
    Ok(json!({ "custom_properties": list.iter().map(def_json).collect::<Vec<_>>() }).into())
}

/// An item's `custom` argument applied to its values: name → value ("" or
/// null clears), each name a defined custom property, each value checked
/// against its type.
pub fn apply_custom(a: &Args, values: &mut std::collections::BTreeMap<String, String>) -> Result<(), OpError> {
    let Some(v) = a.get("custom") else { return Ok(()) };
    let Value::Object(m) = v else { return Err(OpError::param("custom", "an object: property name → value")) };
    let defs = custom_properties();
    for (k, x) in m {
        let def = defs.iter().find(|p| p.name.eq_ignore_ascii_case(k.trim())).ok_or_else(|| OpError::param("custom", &format!("no custom property named \"{k}\" (company_custom_properties lists them)")))?;
        let text = match x {
            Value::Null => String::new(),
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            _ => return Err(OpError::param("custom", "values are texts, numbers or booleans")),
        };
        let checked = def.check(&text).map_err(|e| OpError::param("custom", &e))?;
        if checked.is_empty() {
            values.remove(&def.name);
        } else {
            values.insert(def.name.clone(), checked);
        }
    }
    Ok(())
}
