//! The window's selection: select entities or instances, clear the
//! selection, Select ▸ (related entities), Select other… (what is under a
//! point of the graphics area), Select instances with same configuration.
//! Kept per tab (ui_state); with the app open the window selects the same.
//! view_camera zoom_selection uses it when no `refs` are given.

use super::{doc_field, s};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::window::{Selected, UiRequest};
use crate::{Ctx, OpenDoc};
use glam::DVec2;
use noshape_model::document::{TabContent, TabId};
use noshape_model::reference::Reference;
use noshape_model::select::Criterion;
use noshape_scene::camera_ops as co;
use serde_json::json;

const ACTIONS: [&str; 9] = ["get", "set", "add", "remove", "clear", "related", "create_selection", "select_other", "same_configuration"];

const CRITERIA: [Criterion; 12] = [
    Criterion::TangentFaces,
    Criterion::FaceLoop,
    Criterion::TangentEdges,
    Criterion::ParallelEdges,
    Criterion::EqualRadius,
    Criterion::PartFaces,
    Criterion::Protrusion,
    Criterion::Pocket,
    Criterion::Hole,
    Criterion::Fillets,
    Criterion::EdgeLoop,
    Criterion::BoundedFaces,
];

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "ui",
        verb: "select",
        summary: "The window's selection: select, clear, Select ▸ related entities, Select other…, same configuration",
        description: "What is selected in a Part Studio, Assembly or Drawing window (per tab; `get` reads it). Part Studios: `set` / `add` / `remove` entity ids (query_entities: parts, faces, edges, vertices, sketches, planes), `clear` (Clear selection), `related` (the context menu's Select ▸ item `criterion` grown from the first of `refs`; Bounded faces stops at the other `refs`, which stay selected), `create_selection` (Create selection…: the same from the `refs` (its Selection field), with `patterns` (Include patterns) the pattern instances too, then Add selection: added to the selection), `select_other` (Select other…: the faces under graphics-area point `at` [x, y] through the window's camera (view_camera), nearest first; `index` picks one, else the list is returned; in an assembly the instances). Assemblies: `set` / `add` / `remove` instances (ids or names), `clear`, `same_configuration` (Select instances with same configuration as the first of `refs`; `same_part` for same part and same configuration). Drawings: `clear`. With the app open the window selects the same.",
        fields: vec![
            doc_field(),
            opt("tab", s(), "Part Studio, Assembly or Drawing tab, by name or id. Default: the window's tab."),
            req("action", json!({ "type": "string", "enum": ACTIONS }), "What to do."),
            opt("refs", json!({ "type": "array", "items": { "type": "string" } }), "Entity ids, or in an assembly instance ids or names."),
            opt("criterion", json!({ "type": "string", "enum": CRITERIA.map(|c| c.label()) }), "related: the Select ▸ item."),
            opt("at", json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }), "select_other: the point [x, y] in graphics-area pixels."),
            opt("index", json!({ "type": "integer", "minimum": 0 }), "select_other: select the index-th entity under the point (0 = nearest)."),
            opt("same_part", json!({ "type": "boolean" }), "same_configuration: same part and same configuration."),
            opt("patterns", json!({ "type": "boolean" }), "create_selection: Include patterns."),
        ],
        mutates: false,
        destructive: false,
        run: select,
    }]
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Studio,
    Assembly,
    Drawing,
}

fn select_tab(od: &OpenDoc, a: &Args) -> Result<(TabId, Kind), OpError> {
    let kind = |c: &TabContent| match c {
        TabContent::PartStudio(_) => Some(Kind::Studio),
        TabContent::Assembly(_) => Some(Kind::Assembly),
        TabContent::Drawing(_) => Some(Kind::Drawing),
        _ => None,
    };
    let d = od.doc();
    let tab = match a.opt_str("tab")? {
        Some(k) => crate::doc::find_tab(d, k)?,
        None => od.window.active_tab.filter(|t| d.tab(*t).is_some_and(|t| kind(&t.content).is_some())).or_else(|| d.ordered_tabs().into_iter().find(|t| kind(&t.content).is_some()).map(|t| t.id)).ok_or_else(|| OpError::not_found("the document has no Part Studio, Assembly or Drawing").with_param("tab"))?,
    };
    let t = d.tab(tab).expect("found");
    let k = kind(&t.content).ok_or_else(|| OpError::invalid(format!("tab `{}` is a {}: selections are a Part Studio's, Assembly's or Drawing's", t.name, t.content.kind_name())).with_param("tab"))?;
    Ok((tab, k))
}

fn refs(a: &Args) -> Result<Vec<String>, OpError> {
    a.opt_array("refs")?.map(|l| l.iter().enumerate().map(|(i, v)| v.as_str().map(str::to_string).ok_or_else(|| OpError::param(&format!("refs[{i}]"), "an id"))).collect()).unwrap_or(Ok(Vec::new()))
}

fn select(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc = a.opt_str("doc")?;
    let od = ctx.doc(doc)?;
    let (tab, kind) = select_tab(od, a)?;
    let action = a.str("action")?;
    if !ACTIONS.contains(&action) {
        return Err(OpError::param("action", &ACTIONS.join(", ")));
    }
    let current = od.window.selection.get(&tab).cloned().unwrap_or_default();
    let given = refs(a)?;
    let mut extra = serde_json::Map::new();
    let now: Vec<String> = match (kind, action) {
        (_, "get") => current.clone(),
        (_, "clear") => Vec::new(),
        (Kind::Drawing, _) => return Err(OpError::param("action", "in a drawing: get or clear")),
        (Kind::Studio, "set" | "add" | "remove") => {
            let parsed = studio_refs(&given)?;
            combine(action, current.clone(), parsed.iter().map(crate::refs::to_id).collect())
        }
        (Kind::Studio, "related" | "create_selection") => {
            let parsed = studio_refs(&given)?;
            let seed = parsed.first().ok_or_else(|| OpError::param("refs", "the entity to grow from"))?;
            let name = a.opt_str("criterion")?.ok_or_else(|| OpError::param("criterion", "a Select ▸ item"))?;
            let c = CRITERIA.into_iter().find(|c| c.label().eq_ignore_ascii_case(name)).ok_or_else(|| OpError::param("criterion", &CRITERIA.map(|c| c.label()).join(", ")))?;
            let r = od.regenerate(tab, None)?;
            let found = match seed {
                Reference::Face(_) if c == Criterion::BoundedFaces => {
                    let boundary: Vec<Reference> = parsed[1..].to_vec();
                    let mut found = noshape_model::select::select_bounded(&r.context, seed, &boundary);
                    for b in boundary {
                        if !found.contains(&b) {
                            found.push(b);
                        }
                    }
                    found
                }
                Reference::SketchEntity { .. } | Reference::SketchRegion { .. } => return Err(OpError::param("refs", "a part's face or edge (sketch entities: sketch_get lists their connections)")),
                _ => noshape_model::select::select_related(&r.context, seed, c),
            };
            if action == "create_selection" {
                // Include patterns, then Add selection.
                let found = if a.bool_or("patterns", false)? { noshape_model::select::with_pattern_instances(&r.context, &found) } else { found };
                combine("add", current.clone(), found.iter().map(crate::refs::to_id).collect())
            } else {
                // Each item replaces the selection; nothing found clears it (live).
                found.iter().map(crate::refs::to_id).collect()
            }
        }
        (Kind::Studio, "select_other") => {
            let at = a.opt_array("at")?.and_then(|v| Some(DVec2::new(v.first()?.as_f64()?, v.get(1)?.as_f64()?))).ok_or_else(|| OpError::param("at", "[x, y] in graphics-area pixels"))?;
            let vc = od.window.cameras.get(&tab).cloned().ok_or_else(|| OpError::invalid("the window has no camera for this tab yet: set it with view_camera").with_param("at"))?;
            let r = od.regenerate(tab, None)?;
            let (origin, dir) = vc.camera.ray(at, vc.viewport);
            let hits: Vec<String> = co::faces_under(&r.display, origin, dir).iter().map(|(_, r)| crate::refs::to_id(r)).collect();
            extra.insert("under".into(), json!(hits));
            match a.opt_u64("index")? {
                Some(i) => vec![hits.get(i as usize).cloned().ok_or_else(|| OpError::param("index", &format!("{} entities under the point", hits.len())))?],
                None => current.clone(),
            }
        }
        (Kind::Assembly, "set" | "add" | "remove") => {
            let asm = super::assembly::assembly(od, tab)?;
            let ids = given.iter().enumerate().map(|(i, k)| super::assembly::find_instance(asm, k, &format!("refs[{i}]")).map(|id| format!("inst:{}", id.0))).collect::<Result<Vec<_>, _>>()?;
            combine(action, current.clone(), ids)
        }
        (Kind::Assembly, "select_other") => {
            let at = a.opt_array("at")?.and_then(|v| Some(DVec2::new(v.first()?.as_f64()?, v.get(1)?.as_f64()?))).ok_or_else(|| OpError::param("at", "[x, y] in graphics-area pixels"))?;
            let vc = od.window.cameras.get(&tab).cloned().ok_or_else(|| OpError::invalid("the window has no camera for this tab yet: set it with view_camera").with_param("at"))?;
            // Isolate on: only the isolated instances.
            let isolate: Option<std::collections::HashSet<noshape_model::assembly::InstanceId>> = od.window.display.get(&tab).and_then(|d| d.isolate.as_ref()).filter(|i| !i.transparent).map(|i| i.ids.iter().filter_map(|k| k.strip_prefix("inst:")?.parse().ok().map(noshape_model::assembly::InstanceId)).collect());
            let asm = super::assembly::assembly(od, tab)?.clone();
            let result = super::assembly::solve_assembly(od, tab, &asm, &Default::default());
            let (origin, dir) = vc.camera.ray(at, vc.viewport);
            let hits: Vec<String> = crate::assembly::instances_under(&result, isolate.as_ref(), origin, dir).iter().map(|id| format!("inst:{}", id.0)).collect();
            extra.insert("under".into(), json!(hits));
            match a.opt_u64("index")? {
                Some(i) => vec![hits.get(i as usize).cloned().ok_or_else(|| OpError::param("index", &format!("{} instances under the point", hits.len())))?],
                None => current.clone(),
            }
        }
        (Kind::Assembly, "same_configuration") => {
            let asm = super::assembly::assembly(od, tab)?;
            let top = super::assembly::find_instance(asm, given.first().ok_or_else(|| OpError::param("refs", "the instance"))?, "refs[0]")?;
            crate::assembly::same_configuration(asm, top, a.bool_or("same_part", false)?).iter().map(|id| format!("inst:{}", id.0)).collect()
        }
        _ => return Err(OpError::param("action", "related and select_other are a Part Studio's; same_configuration an Assembly's")),
    };
    let changed = now != current;
    od.window.selection.insert(tab, now.clone());
    let shown = if changed || action == "clear" {
        let sel = match kind {
            _ if now.is_empty() => Selected::Clear,
            Kind::Studio => Selected::Studio(studio_refs(&now)?),
            _ => Selected::Instances(now.iter().filter_map(|k| k.strip_prefix("inst:")?.parse().ok().map(noshape_model::assembly::InstanceId)).collect()),
        };
        super::show(ctx, doc, UiRequest::Select { tab, sel })?
    } else {
        ctx.serving
    };
    let mut out = json!({ "tab": super::tab_json(ctx.doc(doc)?, tab), "selected": now, "shown_in_app": shown });
    for (k, v) in extra {
        out[k] = v;
    }
    Ok(out.into())
}

fn studio_refs(ids: &[String]) -> Result<Vec<Reference>, OpError> {
    ids.iter().enumerate().map(|(i, k)| crate::refs::parse_str(k).ok_or_else(|| OpError::param(&format!("refs[{i}]"), "an entity id (query_entities)"))).collect()
}

/// `set` / `add` / `remove` on the current ids, keeping the order.
fn combine(action: &str, mut current: Vec<String>, ids: Vec<String>) -> Vec<String> {
    match action {
        "set" => {
            current.clear();
            for i in ids {
                if !current.contains(&i) {
                    current.push(i);
                }
            }
        }
        "add" => {
            for i in ids {
                if !current.contains(&i) {
                    current.push(i);
                }
            }
        }
        _ => current.retain(|c| !ids.contains(c)),
    }
    current
}
