//! The graphics area's display settings that aren't saved with the
//! document: Isolate… / Make transparent… / Exit isolate, Highlight
//! boundary edges, Orient normal to sketch on edit, Turn thickness analysis
//! off; the Isolate dialog's and the Assembly context banner's controls;
//! an assembly's Hide/show mates, Show mates mode, Snap mode and active
//! exploded view (Activate <view>, its rollback bar, Done). Kept
//! per tab in the window (ui_state); with the app open the window does the
//! same, and render_image `view: "window"` draws with them.

use super::{doc_field, s, strings};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt};
use crate::window::{CONTEXT_TRANSPARENCY, DisplaySet, IsoExpand, Isolation, ShownExplode, TabDisplay, UiRequest};
use crate::{Ctx, OpenDoc};
use noshape_model::document::{TabContent, TabId};
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    let b = || json!({ "type": "boolean" });
    vec![Op {
        group: "view",
        verb: "display",
        summary: "Display settings of a Part Studio or Assembly window: isolate, transparent (and the dialog's transparency, select transparent, expand), boundary edges, orient on edit, thickness off, the Assembly context banner, mates, snap mode, the active exploded view",
        description: "What the view and context menus change without editing the document, kept per tab (`ui_state`; nothing given: read them). `isolate` (Isolate…: these parts, the rest as ghosts) or `transparent` (Make transparent…: these as ghosts) replace the current isolation; `exit_isolate` ends it. The Isolate / Make transparent dialog (with one shown): `transparency` (its slider, 0 to 1), `select_transparent` (Select transparent geometry), `expand` (Expand's steps: a Part Studio's distance slider 0 to 10; an assembly's one instance group per step, by `expand_by` connectivity (its default) or distance); `expanded` lists what Expand added. Part Studios: `highlight_boundary` (Highlight boundary edges), `orient_on_edit` (Orient normal to sketch on edit), `thickness_analysis: false` (Turn thickness analysis off), `context_hidden` (the Assembly context banner's Hide/show instances), `context_show_internal` / `context_select_transparent` / `context_transparency` (its Show internal geometry, Select transparent geometry and transparency slider, 0 to 1). Assemblies (instances for isolate / transparent): `show_mates` (Hide/show mates: the mate indicators), `show_mates_mode` (Show mates mode), `snap_mode` (Snap mode), `exploded_view` (an exploded view's Activate <view>: shown exploded, zoomed to fit; null: Done) and `explode_progress` (its rollback bar: the steps shown); `exploded` lists where the shown steps move the instances. With the app open the window does the same; render_image `view: \"window\"` draws with them.",
        fields: vec![
            doc_field(),
            opt("tab", s(), "Part Studio or Assembly tab, by name or id. Default: the window's tab."),
            opt("isolate", strings(), "Isolate…: parts (ids or names) or, in an assembly, instances (one inside a subassembly instance by its path, \"Sub <1>/Part 1 <1>\")."),
            opt("transparent", strings(), "Make transparent…: parts or instances."),
            opt("exit_isolate", b(), "Exit isolate."),
            opt("transparency", json!({ "type": "number", "minimum": 0, "maximum": 1 }), "The Isolate / Make transparent dialog's transparency slider, 0 (left) to 1."),
            opt("select_transparent", b(), "The Isolate / Make transparent dialog's Select transparent geometry."),
            opt("expand", json!({ "type": "integer", "minimum": 0 }), "The dialog's Expand slider in steps: a Part Studio's 0 to 10 (distance), an assembly's 0 to its number of instance groups."),
            opt("expand_by", json!({ "type": "string", "enum": ["connectivity", "distance"] }), "Assembly: Expand: connectivity (mates; the default) or distance. Changing it starts at 0 steps."),
            opt("highlight_boundary", b(), "Part Studio: Highlight boundary edges."),
            opt("orient_on_edit", b(), "Part Studio: Orient normal to sketch on edit."),
            opt("thickness_analysis", b(), "Part Studio: false = Turn thickness analysis off (run the analysis with analysis_thickness)."),
            opt("context_hidden", strings(), "Part Studio: the shown Assembly context's Hide/show instances: the top-level instances (names or ids, context_list) to hide; [] shows all."),
            opt("context_show_internal", b(), "Part Studio: the Assembly context banner's Show internal geometry."),
            opt("context_select_transparent", b(), "Part Studio: the Assembly context banner's Select transparent geometry."),
            opt("context_transparency", json!({ "type": "number", "minimum": 0, "maximum": 1 }), "Part Studio: the Assembly context banner's transparency slider, 0 to 1."),
            opt("show_mates", b(), "Assembly: Hide/show mates."),
            opt("show_mates_mode", b(), "Assembly: Show mates mode."),
            opt("snap_mode", b(), "Assembly: Snap mode."),
            opt("exploded_view", json!({ "type": ["string", "null"] }), "Assembly: Activate <view> (an exploded view's name: shown exploded, zoomed to fit); null: Done (assembled again)."),
            opt("explode_progress", json!({ "type": "integer", "minimum": 0 }), "Assembly: the active exploded view's rollback bar: the steps shown, 0 to its number of steps."),
        ],
        mutates: false,
        destructive: false,
        run: display,
    }]
}

/// The tab a call names, else the window's (a Part Studio or Assembly),
/// else the first Part Studio. True for an assembly.
fn display_tab(od: &OpenDoc, a: &Args) -> Result<(TabId, bool), OpError> {
    let kind = |c: &TabContent| match c {
        TabContent::PartStudio(_) => Some(false),
        TabContent::Assembly(_) => Some(true),
        _ => None,
    };
    let d = od.doc();
    let tab = match a.opt_str("tab")? {
        Some(k) => crate::doc::find_tab(d, k)?,
        None => od.window.active_tab.filter(|t| d.tab(*t).is_some_and(|t| kind(&t.content).is_some())).or_else(|| d.ordered_tabs().into_iter().find(|t| kind(&t.content).is_some()).map(|t| t.id)).ok_or_else(|| OpError::not_found("the document has no Part Studio or Assembly").with_param("tab"))?,
    };
    let t = d.tab(tab).expect("found");
    let asm = kind(&t.content).ok_or_else(|| OpError::invalid(format!("tab `{}` is a {}: display settings are a Part Studio's or Assembly's", t.name, t.content.kind_name())).with_param("tab"))?;
    Ok((tab, asm))
}

pub(crate) fn display_json(d: &TabDisplay, assembly: bool) -> Value {
    let mut v = json!({
        "isolate": d.isolate.as_ref().filter(|i| !i.transparent).map(|i| i.ids.clone()),
        "transparent": d.isolate.as_ref().filter(|i| i.transparent).map(|i| i.ids.clone()),
    });
    if let Some(i) = &d.isolate {
        v["select_transparent"] = json!(i.select_transparent);
        v["transparency"] = json!(i.transparency);
        v["expand"] = json!(i.expand.map(|e| json!({ "by": if e.connectivity { "connectivity" } else { "distance" }, "steps": e.steps })));
        v["expanded"] = json!(i.expanded);
    }
    if assembly {
        v["show_mates"] = json!(d.show_mates);
        v["show_mates_mode"] = json!(d.show_mates_mode);
        v["snap_mode"] = json!(d.snap_mode);
        v["exploded_view"] = json!(d.explode.as_ref().map(|e| json!({ "name": e.view, "progress": e.progress })));
    } else {
        v["highlight_boundary"] = json!(d.highlight_boundary);
        v["context_hidden"] = json!(d.context_hidden);
        v["context_show_internal"] = json!(d.context_show_internal.unwrap_or(false));
        v["context_select_transparent"] = json!(d.context_select_transparent.unwrap_or(true));
        v["context_transparency"] = json!(d.context_transparency.unwrap_or(CONTEXT_TRANSPARENCY));
        v["orient_on_edit"] = json!(d.orient_on_edit);
    }
    v
}

fn display(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc = a.opt_str("doc")?;
    let od = ctx.doc(doc)?;
    let (tab, assembly) = display_tab(od, a)?;
    let mut d = od.window.display.get(&tab).cloned().unwrap_or_default();
    let mut sets = Vec::new();
    let only = |k: &str, asm: bool| -> Result<Option<bool>, OpError> {
        let Some(v) = a.get(k) else { return Ok(None) };
        if asm != assembly {
            return Err(OpError::param(k, if asm { "an Assembly's setting" } else { "a Part Studio's setting" }));
        }
        v.as_bool().map(Some).ok_or_else(|| OpError::param(k, "true or false"))
    };
    // Isolate / Make transparent: the parts or instances, checked.
    let iso = match (a.opt_array("isolate")?, a.opt_array("transparent")?) {
        (Some(_), Some(_)) => return Err(OpError::param("transparent", "give `isolate` or `transparent`")),
        (Some(l), None) => Some((false, "isolate", l.clone())),
        (None, Some(l)) => Some((true, "transparent", l.clone())),
        (None, None) => None,
    };
    if a.bool_or("exit_isolate", false)? {
        if iso.is_some() {
            return Err(OpError::param("exit_isolate", "not with isolate / transparent"));
        }
        d.isolate = None;
        sets.push(DisplaySet::ExitIsolate);
    }
    if let Some((transparent, field, list)) = iso {
        if list.is_empty() {
            return Err(OpError::param(field, "Select or hover over a part first."));
        }
        let names = |i: usize| list[i].as_str().unwrap_or_default().to_string();
        if assembly {
            let asm = super::assembly::assembly(od, tab)?;
            // An instance inside a subassembly by its path ("Sub <1>/Part 1 <1>").
            let ids = (0..list.len()).map(|i| super::assembly::find_occurrence(od.doc(), asm, &names(i), &format!("{field}[{i}]"))).collect::<Result<Vec<_>, _>>()?;
            d.isolate = Some(Isolation::new(transparent, ids.iter().map(|p| format!("inst:{}", p.iter().map(|i| i.0.to_string()).collect::<Vec<_>>().join("/"))).collect()));
            sets.push(DisplaySet::ExitIsolate);
            sets.push(DisplaySet::IsolateInstances { transparent, instances: ids });
        } else {
            let r = od.regenerate(tab, None)?;
            let parts = (0..list.len())
                .map(|i| {
                    let k = names(i);
                    r.display.bodies.iter().find(|b| crate::refs::body_id(&b.id) == k || b.name == k).map(|b| b.id.clone()).ok_or_else(|| OpError::param(&format!("{field}[{i}]"), "a part (id or name)"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            d.isolate = Some(Isolation::new(transparent, parts.iter().map(crate::refs::body_id).collect()));
            sets.push(DisplaySet::ExitIsolate);
            sets.push(DisplaySet::IsolateParts { transparent, parts });
        }
    }
    iso_controls(od, tab, assembly, a, &mut d, &mut sets)?;
    if let Some(on) = only("highlight_boundary", false)? {
        d.highlight_boundary = on;
        sets.push(DisplaySet::HighlightBoundary(on));
    }
    if let Some(on) = only("orient_on_edit", false)? {
        d.orient_on_edit = on;
        sets.push(DisplaySet::OrientOnEdit(on));
    }
    if let Some(on) = only("thickness_analysis", false)? {
        if on {
            return Err(OpError::param("thickness_analysis", "only false (Turn thickness analysis off); run it with analysis_thickness"));
        }
        sets.push(DisplaySet::ThicknessOff);
    }
    if let Some(list) = a.opt_array("context_hidden")? {
        if assembly {
            return Err(OpError::param("context_hidden", "a Part Studio's setting"));
        }
        let ps = crate::doc::studio(od.doc(), tab)?;
        let c = ps.active_context.and_then(|id| ps.contexts.iter().find(|c| c.id == id)).ok_or_else(|| OpError::invalid("no Assembly context is shown (context_show)").with_param("context_hidden"))?;
        let asm = match od.doc().tab(c.assembly).map(|t| &t.content) {
            Some(TabContent::Assembly(a)) => Some(a),
            _ => None,
        };
        let tops: Vec<u64> = c.bodies.iter().filter_map(|b| b.path.first().copied()).fold(Vec::new(), |mut v, t| {
            if !v.contains(&t) {
                v.push(t);
            }
            v
        });
        let name_of = |t: u64| asm.and_then(|a| a.instance(noshape_model::assembly::InstanceId(t))).map(|i| i.name.clone()).unwrap_or_else(|| format!("inst:{t}"));
        let mut hidden = Vec::new();
        for (i, v) in list.iter().enumerate() {
            let k = v.as_str().unwrap_or_default();
            let t = tops.iter().copied().find(|t| name_of(*t) == k || format!("inst:{t}") == k).ok_or_else(|| OpError::param(&format!("context_hidden[{i}]"), &format!("a context instance: {}", tops.iter().map(|t| name_of(*t)).collect::<Vec<_>>().join(", "))))?;
            hidden.push(noshape_model::assembly::InstanceId(t));
        }
        d.context_hidden = hidden.iter().map(|t| format!("inst:{}", t.0)).collect();
        sets.push(DisplaySet::ContextHidden(hidden));
    }
    let show_internal = only("context_show_internal", false)?;
    let select_transparent = only("context_select_transparent", false)?;
    let transparency = slider(a, "context_transparency")?;
    if let Some(k) = [(show_internal.is_some(), "context_show_internal"), (select_transparent.is_some(), "context_select_transparent"), (transparency.is_some(), "context_transparency")].iter().find(|g| g.0).map(|g| g.1) {
        if assembly {
            return Err(OpError::param(k, "a Part Studio's setting"));
        }
        let ps = crate::doc::studio(od.doc(), tab)?;
        if ps.active_context.is_none() {
            return Err(OpError::invalid("no Assembly context is shown (context_show)").with_param(k));
        }
        d.context_show_internal = show_internal.or(d.context_show_internal);
        d.context_select_transparent = select_transparent.or(d.context_select_transparent);
        d.context_transparency = transparency.or(d.context_transparency);
        sets.push(DisplaySet::ContextControls { show_internal, select_transparent, transparency });
    }
    if let Some(on) = only("show_mates", true)? {
        d.show_mates = on;
        sets.push(DisplaySet::ShowMates(on));
    }
    if let Some(on) = only("show_mates_mode", true)? {
        d.show_mates_mode = on;
        sets.push(DisplaySet::ShowMatesMode(on));
    }
    if let Some(on) = only("snap_mode", true)? {
        d.snap_mode = on;
        sets.push(DisplaySet::SnapMode(on));
    }
    explode(od, tab, assembly, a, &mut d, &mut sets)?;
    let mut out = display_json(&d, assembly);
    if let Some(e) = &d.explode {
        let ev = super::assembly::assembly(od, tab)?.exploded_views.iter().find(|v| v.name == e.view).cloned();
        if let Some(ev) = ev {
            out["exploded_view"]["steps"] = json!(ev.steps.len());
            out["exploded"] = json!(super::assembly::exploded_offsets(od, tab, &ev, e.progress.map_or(f64::INFINITY, |p| p as f64))?);
        }
    }
    od.window.display.insert(tab, d);
    // Activate <view> zooms to fit the exploded view.
    if sets.iter().any(|s| matches!(s, DisplaySet::Explode(Some(_))))
        && let Some(mut vc) = od.window.cameras.get(&tab).cloned()
    {
        let asm = super::assembly::assembly(od, tab)?.clone();
        let r = super::assembly::shown_assembly(od, tab, &asm);
        if let Some((lo, hi)) = super::assembly::fit_box(&asm, &r) {
            let mut to = vc.camera;
            noshape_scene::camera_ops::fit_camera_to_box(&mut to, lo, hi, vc.viewport, 0.0);
            vc.set(to);
            od.window.cameras.insert(tab, vc);
        }
    }
    let mut shown = ctx.serving;
    for set in sets {
        shown = super::show(ctx, doc, UiRequest::Display { tab, set })?;
    }
    out["tab"] = super::tab_json(ctx.doc(doc)?, tab);
    out["shown_in_app"] = json!(shown);
    Ok(out.into())
}

/// Activate <view> / Done and the rollback bar of an assembly's exploded
/// views.
fn explode(od: &mut OpenDoc, tab: TabId, assembly: bool, a: &Args, d: &mut TabDisplay, sets: &mut Vec<DisplaySet>) -> Result<(), OpError> {
    // `null` is Done: read the raw value (`get` drops nulls).
    let view = a.value.get("exploded_view");
    let progress = a.get("explode_progress");
    if let Some(k) = [(view.is_some(), "exploded_view"), (progress.is_some(), "explode_progress")].iter().find(|g| g.0).map(|g| g.1)
        && !assembly
    {
        return Err(OpError::param(k, "an Assembly's setting"));
    }
    let views: Vec<String> = if assembly { super::assembly::assembly(od, tab)?.exploded_views.iter().map(|v| v.name.clone()).collect() } else { Vec::new() };
    match view {
        None => {}
        Some(Value::Null) => {
            d.explode = None;
            sets.push(DisplaySet::Explode(None));
        }
        Some(v) => {
            let name = v.as_str().unwrap_or_default();
            let i = views.iter().position(|n| n == name).ok_or_else(|| {
                OpError::param("exploded_view", &if views.is_empty() { "the assembly has no exploded views (assembly_exploded_view add)".to_string() } else { format!("an exploded view: {}", views.join(", ")) })
            })?;
            d.explode = Some(ShownExplode { view: name.to_string(), progress: None });
            sets.push(DisplaySet::Explode(Some(i)));
        }
    }
    let Some(p) = progress else { return Ok(()) };
    let e = d.explode.as_mut().ok_or_else(|| OpError::invalid("no exploded view is active (exploded_view)").with_param("explode_progress"))?;
    let n = super::assembly::assembly(od, tab)?.exploded_views.iter().find(|v| v.name == e.view).map_or(0, |v| v.steps.len());
    let steps = p.as_u64().filter(|s| *s <= n as u64).ok_or_else(|| OpError::param("explode_progress", &format!("0 to {n} (the steps of {})", e.view)))? as usize;
    e.progress = Some(steps);
    sets.push(DisplaySet::ExplodeProgress(steps));
    Ok(())
}

/// A 0-to-1 slider position.
fn slider(a: &Args, k: &str) -> Result<Option<f32>, OpError> {
    let Some(v) = a.get(k) else { return Ok(None) };
    v.as_f64().filter(|f| (0.0..=1.0).contains(f)).map(|f| Some(f as f32)).ok_or_else(|| OpError::param(k, "a number from 0 to 1 (the slider's position)"))
}

/// The Isolate / Make transparent dialog's controls on the shown isolation.
fn iso_controls(od: &mut OpenDoc, tab: TabId, assembly: bool, a: &Args, d: &mut TabDisplay, sets: &mut Vec<DisplaySet>) -> Result<(), OpError> {
    let transparency = slider(a, "transparency")?;
    let select = match a.get("select_transparent") {
        Some(v) => Some(v.as_bool().ok_or_else(|| OpError::param("select_transparent", "true or false"))?),
        None => None,
    };
    let steps = match a.get("expand") {
        Some(v) => Some(v.as_u64().ok_or_else(|| OpError::param("expand", "a whole number of steps, 0 or more"))? as usize),
        None => None,
    };
    let by = a.opt_str("expand_by")?;
    let given = [(transparency.is_some(), "transparency"), (select.is_some(), "select_transparent"), (steps.is_some(), "expand"), (by.is_some(), "expand_by")];
    let Some(field) = given.iter().find(|g| g.0).map(|g| g.1) else { return Ok(()) };
    let Some(iso) = d.isolate.as_mut() else {
        return Err(OpError::invalid("no Isolate or Make transparent is shown (isolate / transparent first)").with_param(field));
    };
    if let Some(t) = transparency {
        iso.transparency = Some(t);
    }
    if let Some(on) = select {
        iso.select_transparent = on;
    }
    let mut expand = None;
    if steps.is_some() || by.is_some() {
        let connectivity = match by {
            Some("connectivity") if !assembly => return Err(OpError::param("expand_by", "Expand: connectivity is not available in a Part Studio (no mates); use distance")),
            Some("connectivity") => true,
            Some("distance") => false,
            Some(_) => return Err(OpError::param("expand_by", "connectivity or distance")),
            // A switch starts over; else the current way (an assembly's default: connectivity).
            None => iso.expand.map_or(assembly, |e| e.connectivity),
        };
        let switched = iso.expand.map_or(assembly, |e| e.connectivity) != connectivity;
        let steps = steps.unwrap_or(if switched { 0 } else { iso.expand.map_or(0, |e| e.steps) });
        let e = IsoExpand { connectivity, steps };
        iso.expanded = if assembly { asm_expansion(od, tab, &iso.ids, e)? } else { studio_expansion(od, tab, &iso.ids, steps)? };
        iso.expand = Some(e);
        expand = Some(e);
    }
    sets.push(DisplaySet::IsolateControls { select_transparent: select, transparency, expand });
    Ok(())
}

/// The parts a Part Studio's Expand: distance slider adds at `steps` (as the window: what Zoom to fit frames is the
/// model's box).
fn studio_expansion(od: &mut OpenDoc, tab: TabId, ids: &[String], steps: usize) -> Result<Vec<String>, OpError> {
    if steps > 10 {
        return Err(OpError::param("expand", "0 to 10 (the Expand: distance slider's steps)"));
    }
    let r = od.regenerate(tab, None)?;
    let co = noshape_scene::camera_ops::studio_fit_box(&r.display, noshape_scene::camera_ops::studio_fit_bounds(&r.display, true, None), r.display.show_origin, |pl| !pl.hidden);
    let Some(model) = co.filter(|_| steps > 0) else { return Ok(Vec::new()) };
    let chosen = |b: &noshape_model::display::BodyDisplay| ids.contains(&crate::refs::body_id(&b.id));
    let seeds: Vec<_> = r.display.bodies.iter().filter(|b| chosen(b)).filter_map(|b| b.bounds).collect();
    let others: Vec<_> = r.display.bodies.iter().filter(|b| !chosen(b)).filter_map(|b| Some((b.id.clone(), b.bounds?))).collect();
    Ok(noshape_scene::camera_ops::expand_by_distance(model, &seeds, &others, steps).iter().map(crate::refs::body_id).collect())
}

/// The top-level instances an assembly's Expand adds: one group per step (mate connectivity or distance from the
/// isolated top-level instances), as the window.
fn asm_expansion(od: &mut OpenDoc, tab: TabId, ids: &[String], e: IsoExpand) -> Result<Vec<String>, OpError> {
    let asm = super::assembly::assembly(od, tab)?.clone();
    let mut seeds: Vec<noshape_model::assembly::InstanceId> = ids.iter().filter_map(|k| k.strip_prefix("inst:")?.parse().ok().map(noshape_model::assembly::InstanceId)).collect();
    seeds.sort();
    let result = super::assembly::solve_assembly(od, tab, &asm, &Default::default());
    let order = noshape_model::assembly::analysis::isolate_expansion(&result, &asm, &seeds, e.connectivity);
    if e.steps > order.len() {
        let by = if e.connectivity { "connectivity" } else { "distance" };
        return Err(OpError::param("expand", &format!("0 to {} (Expand: {by} has {} steps here)", order.len(), order.len())));
    }
    Ok(order.iter().take(e.steps).flatten().map(|i| format!("inst:{}", i.0)).filter(|k| !ids.contains(k)).collect())
}
