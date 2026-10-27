//! The window's lists: the Part Studio feature list's folders (Expand all /
//! Collapse all in list) and filter (Filter feature list); the Assembly's
//! instance tree (Expand / Collapse selected / all, Find in instance list)
//! and mate rows (Expand / Collapse). View state, kept per tab (ui_state);
//! with the app open the window shows the same.

use super::{doc_field, s, strings};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::window::{ListAct, UiRequest};
use crate::{Ctx, OpenDoc};
use noshape_model::assembly::InstanceId;
use noshape_model::assembly::eval::InstanceNode;
use noshape_model::document::{TabContent, TabId};
use serde_json::{Value, json};

const ACTIONS: [&str; 7] = ["get", "expand_all", "collapse_all", "expand", "collapse", "filter", "find"];

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "ui",
        verb: "list",
        summary: "The feature and instance lists: expand / collapse folders, subassemblies and mates, filter the feature list, find an instance",
        description: "View state of a tab's lists (`get` reads it). Part Studios: `expand_all` / `collapse_all` (Expand all in list / Collapse all in list: the folders), `expand` / `collapse` folders `items` (names), `filter` (Filter feature list: `text` as the list's search box takes it — names and type names, or `:errors`, `:suppressed`, `:variable \"name\"`, `:type x`, …; \"\" clears; returns the features shown). Assemblies: `expand_all` / `collapse_all` (the instance tree, patterns and mates), `expand` / `collapse` `items`: subassembly instances (Expand selected / Collapse selected) or mates (a mate row's Expand / Collapse), `find` (Find in instance list: selects the instance, its list row shown). With the app open the window shows the same.",
        fields: vec![
            doc_field(),
            opt("tab", s(), "Part Studio or Assembly tab, by name or id. Default: the window's tab."),
            req("action", json!({ "type": "string", "enum": ACTIONS }), "What to do."),
            opt("items", strings(), "expand / collapse / find: folder names (Part Studio); instance or mate ids or names (Assembly)."),
            opt("text", s(), "filter: the feature list's search text."),
        ],
        mutates: false,
        destructive: false,
        run: list,
    }]
}

fn list_tab(od: &OpenDoc, a: &Args) -> Result<(TabId, bool), OpError> {
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
    let asm = kind(&t.content).ok_or_else(|| OpError::invalid(format!("tab `{}` is a {}: lists are a Part Studio's or Assembly's", t.name, t.content.kind_name())).with_param("tab"))?;
    Ok((tab, asm))
}

fn items(a: &Args) -> Result<Vec<String>, OpError> {
    let l = a.opt_array("items")?.filter(|l| !l.is_empty()).ok_or_else(|| OpError::param("items", "what to expand, collapse or find"))?;
    l.iter().enumerate().map(|(i, v)| v.as_str().map(str::to_string).ok_or_else(|| OpError::param(&format!("items[{i}]"), "a name or id"))).collect()
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let doc = a.opt_str("doc")?;
    let od = ctx.doc(doc)?;
    let (tab, assembly) = list_tab(od, a)?;
    let action = a.str("action")?;
    if !ACTIONS.contains(&action) {
        return Err(OpError::param("action", &ACTIONS.join(", ")));
    }
    let mut st = od.window.lists.get(&tab).cloned().unwrap_or_default();
    let mut acts = Vec::new();
    let mut out = json!({});
    if assembly {
        let asm = super::assembly::assembly(od, tab)?.clone();
        let result = super::assembly::solve_assembly(od, tab, &asm, &Default::default());
        match action {
            "get" => {}
            "expand_all" => {
                // The app's: every node with children, the patterns, every mate row.
                fn walk(nodes: &[InstanceNode], out: &mut Vec<Vec<InstanceId>>) {
                    for n in nodes {
                        if !n.children.is_empty() {
                            out.push(n.path.clone());
                            walk(&n.children, out);
                        }
                    }
                }
                let mut paths = Vec::new();
                walk(&result.nodes, &mut paths);
                paths.extend(asm.patterns.iter().map(|p| vec![InstanceId(p.id.0)]));
                st.expanded = paths.iter().map(|p| p.iter().map(|i| i.0).collect()).collect();
                st.expanded_mates = asm.mates.iter().map(|m| m.id.0).collect();
                acts.push(ListAct::All(true));
            }
            "collapse_all" => {
                st.expanded.clear();
                st.expanded_mates.clear();
                acts.push(ListAct::All(false));
            }
            "expand" | "collapse" => {
                let expand = action == "expand";
                let (mut tops, mut mates) = (Vec::new(), Vec::new());
                for (i, k) in items(a)?.iter().enumerate() {
                    let field = format!("items[{i}]");
                    if let Some(m) = asm.mates.iter().find(|m| m.name == *k || format!("mate:{}", m.id.0) == *k) {
                        mates.push(m.id);
                        continue;
                    }
                    let id = super::assembly::find_instance(&asm, k, &field)?;
                    if !result.node(&[id]).is_some_and(|n| !n.children.is_empty()) {
                        return Err(OpError::param(&field, "a subassembly (an instance with children) or a mate"));
                    }
                    tops.push(id);
                }
                for id in &tops {
                    if expand {
                        st.expanded.insert(vec![id.0]);
                    } else {
                        st.expanded.retain(|p| p[0] != id.0);
                    }
                }
                for m in &mates {
                    if expand {
                        st.expanded_mates.insert(m.0);
                    } else {
                        st.expanded_mates.remove(&m.0);
                    }
                }
                if !tops.is_empty() {
                    acts.push(ListAct::Instances { tops, expand });
                }
                if !mates.is_empty() {
                    acts.push(ListAct::Mates { mates, expand });
                }
            }
            "find" => {
                let k = items(a)?;
                let id = super::assembly::find_instance(&asm, &k[0], "items[0]")?;
                od.window.selection.insert(tab, vec![format!("inst:{}", id.0)]);
                acts.push(ListAct::Find(id));
                out["selected"] = json!([format!("inst:{}", id.0)]);
            }
            _ => return Err(OpError::param("action", "in an assembly: get, expand_all, collapse_all, expand, collapse or find")),
        }
        let name = |p: &Vec<u64>| p.iter().map(|i| asm.instance(InstanceId(*i)).map_or(format!("inst:{i}"), |x| x.name.clone())).collect::<Vec<_>>().join(" > ");
        out["expanded"] = st.expanded.iter().map(name).collect::<Vec<_>>().into();
        out["expanded_mates"] = st.expanded_mates.iter().map(|m| asm.mates.iter().find(|x| x.id.0 == *m).map_or(format!("mate:{m}"), |x| x.name.clone())).collect::<Vec<_>>().into();
    } else {
        let ps = crate::doc::studio(od.doc(), tab)?.clone();
        let folder = |k: &str, field: &str| ps.folders.iter().find(|f| f.name == k || f.name.eq_ignore_ascii_case(k)).map(|f| f.id).ok_or_else(|| OpError::not_found(format!("no folder `{k}`; folders: {}", ps.folders.iter().map(|f| f.name.clone()).collect::<Vec<_>>().join(", "))).with_param(field));
        match action {
            "get" => {}
            "expand_all" | "collapse_all" => {
                let open = action == "expand_all";
                for f in &ps.folders {
                    st.folders_open.insert(f.id, open);
                }
                acts.push(ListAct::All(open));
            }
            "expand" | "collapse" => {
                let open = action == "expand";
                let ids = items(a)?.iter().enumerate().map(|(i, k)| folder(k, &format!("items[{i}]"))).collect::<Result<Vec<_>, _>>()?;
                for id in &ids {
                    st.folders_open.insert(*id, open);
                }
                acts.push(ListAct::Folders { ids, open });
            }
            "filter" => {
                let text = a.opt_str("text")?.ok_or_else(|| OpError::param("text", "the search text (\"\" clears)"))?.to_string();
                st.filter = text.clone();
                acts.push(ListAct::Filter(text));
            }
            _ => return Err(OpError::param("action", "in a Part Studio: get, expand_all, collapse_all, expand, collapse or filter")),
        }
        out["folders"] = ps.folders.iter().map(|f| json!({ "name": f.name, "open": st.folders_open.get(&f.id).copied().unwrap_or(!f.collapsed) })).collect::<Vec<_>>().into();
        out["filter"] = json!(st.filter);
        if !st.filter.trim().is_empty() {
            let r = od.regenerate(tab, None)?;
            // A command still waiting for its argument (":type") doesn't filter.
            let shown = noshape_model::interaction::filter_features(&ps, &r.display, &st.filter);
            out["shown_features"] = ps.features.iter().filter(|f| shown.as_ref().is_none_or(|s| s.contains(&f.id))).map(|f| json!(f.name)).collect::<Vec<Value>>().into();
        }
    }
    od.window.lists.insert(tab, st);
    let mut shown = ctx.serving;
    for act in acts {
        shown = super::show(ctx, doc, UiRequest::List { tab, act })?;
    }
    out["tab"] = super::tab_json(ctx.doc(doc)?, tab);
    out["shown_in_app"] = json!(shown);
    Ok(out.into())
}
