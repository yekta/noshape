//! The graphics area's display settings that aren't saved with the
//! document: Isolate… / Make transparent… / Exit isolate, Highlight
//! boundary edges, Orient normal to sketch on edit, Turn thickness analysis
//! off; an assembly's Hide/show mates, Show mates mode and Snap mode. Kept
//! per tab in the window (ui_state); with the app open the window does the
//! same, and render_image `view: "window"` draws with them.

use super::{doc_field, s, strings};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt};
use crate::window::{DisplaySet, Isolation, TabDisplay, UiRequest};
use crate::{Ctx, OpenDoc};
use noshape_model::document::{TabContent, TabId};
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    let b = || json!({ "type": "boolean" });
    vec![Op {
        group: "view",
        verb: "display",
        summary: "Display settings of a Part Studio or Assembly window: isolate, transparent, boundary edges, orient on edit, thickness off, mates, snap mode",
        description: "What the view and context menus change without editing the document, kept per tab (`ui_state`; nothing given: read them). `isolate` (Isolate…: these parts, the rest as ghosts) or `transparent` (Make transparent…: these as ghosts) replace the current isolation; `exit_isolate` ends it. Part Studios: `highlight_boundary` (Highlight boundary edges), `orient_on_edit` (Orient normal to sketch on edit), `thickness_analysis: false` (Turn thickness analysis off), `context_hidden` (the Assembly context banner's Hide/show instances). Assemblies (instances for isolate / transparent): `show_mates` (Hide/show mates: the mate indicators), `show_mates_mode` (Show mates mode), `snap_mode` (Snap mode). With the app open the window does the same; render_image `view: \"window\"` draws with them.",
        fields: vec![
            doc_field(),
            opt("tab", s(), "Part Studio or Assembly tab, by name or id. Default: the window's tab."),
            opt("isolate", strings(), "Isolate…: parts (ids or names) or, in an assembly, instances."),
            opt("transparent", strings(), "Make transparent…: parts or instances."),
            opt("exit_isolate", b(), "Exit isolate."),
            opt("highlight_boundary", b(), "Part Studio: Highlight boundary edges."),
            opt("orient_on_edit", b(), "Part Studio: Orient normal to sketch on edit."),
            opt("thickness_analysis", b(), "Part Studio: false = Turn thickness analysis off (run the analysis with analysis_thickness)."),
            opt("context_hidden", strings(), "Part Studio: the shown Assembly context's Hide/show instances: the top-level instances (names or ids, context_list) to hide; [] shows all."),
            opt("show_mates", b(), "Assembly: Hide/show mates."),
            opt("show_mates_mode", b(), "Assembly: Show mates mode."),
            opt("snap_mode", b(), "Assembly: Snap mode."),
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
    if assembly {
        v["show_mates"] = json!(d.show_mates);
        v["show_mates_mode"] = json!(d.show_mates_mode);
        v["snap_mode"] = json!(d.snap_mode);
    } else {
        v["highlight_boundary"] = json!(d.highlight_boundary);
        v["context_hidden"] = json!(d.context_hidden);
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
            let ids = (0..list.len()).map(|i| super::assembly::find_instance(asm, &names(i), &format!("{field}[{i}]"))).collect::<Result<Vec<_>, _>>()?;
            d.isolate = Some(Isolation { transparent, ids: ids.iter().map(|i| format!("inst:{}", i.0)).collect() });
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
            d.isolate = Some(Isolation { transparent, ids: parts.iter().map(crate::refs::body_id).collect() });
            sets.push(DisplaySet::ExitIsolate);
            sets.push(DisplaySet::IsolateParts { transparent, parts });
        }
    }
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
    let out = display_json(&d, assembly);
    od.window.display.insert(tab, d);
    let mut shown = ctx.serving;
    for set in sets {
        shown = super::show(ctx, doc, UiRequest::Display { tab, set })?;
    }
    let mut out = out;
    out["tab"] = super::tab_json(ctx.doc(doc)?, tab);
    out["shown_in_app"] = json!(shown);
    Ok(out.into())
}
