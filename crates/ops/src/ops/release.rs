//! Release management: a tab's or parts' Release… (the Release candidate
//! dialog's Release) and a part number's Revision history.

use super::{doc_field, s, strings};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, release as rel};
use noshape_model::document::TabContent;
use noshape_model::release::{Package, PackageKind};
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "release",
            verb: "create",
            summary: "Release a tab or parts: part numbers, revisions, a version named after the release",
            description: "Release… (a tab's menu, or a part's): the Release candidate dialog with its items, then Release. Items as the dialog lists them: a Part Studio's parts (or `parts` only), an Assembly with its Bill of Materials, a Drawing with what its views show, a Variable Studio or file itself; `include_drawings` adds the drawings showing them. Each item's part number is its own (Properties) unless `part_numbers` gives one ({item name or part id: number}); its revision is the next of the revision scheme (A, B, …; `revisions` overrides, {item name or part id: revision}). The dialog's checks refuse it with their messages (a part number missing, a revision too low, Company settings' release conditions). Releasing writes the part numbers onto the parts (\"Release <name>\"), records the release at a new version named `name`; with Company settings' automatic obsoletion on, a new revision obsoletes the part number's previous one.",
            fields: vec![
                doc_field(),
                req("tab", s(), "The tab to release (name or id)."),
                opt("parts", strings(), "Part Studio: only these parts (ids or names)."),
                req("name", s(), "Release name (the version's name)."),
                opt("notes", s(), "Release notes (the version's description)."),
                opt("part_numbers", json!({ "type": "object" }), "{item name or part id: part number}."),
                opt("revisions", json!({ "type": "object" }), "{item name or part id: revision}."),
                opt("include_drawings", json!({ "type": "boolean" }), "Include associated drawings."),
            ],
            mutates: true,
            destructive: false,
            run: create,
        },
        Op {
            group: "release",
            verb: "revision_history",
            summary: "A part number's revisions: release, version, state",
            description: "Revision history (a released part's or tab's menu): the part number's revisions, newest first, each with its release's name and time, the version it was released at, the item type, and whether it is obsolete (by an obsoletion, or by a later revision). Without `part_number`, the release log's packages.",
            fields: vec![doc_field(), opt("part_number", s(), "The part number.")],
            mutates: false,
            destructive: false,
            run: history,
        },
    ]
}

fn item_json(i: &noshape_model::release::ReleaseItem) -> Value {
    let mut v = json!({ "name": i.name, "type": i.kind.label(), "part_number": i.part_number, "revision": i.revision });
    if let Some(b) = &i.part {
        v["part"] = json!(crate::refs::body_id(b));
    }
    v
}

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = crate::doc::find_tab(od.doc(), a.str("tab")?)?;
    let provider = crate::derived::DocStudios::new(od.doc().clone(), od.studio_cache.clone());
    let mut items = rel::tab_items(&od.session, &*provider, tab);
    if items.is_empty() {
        return Err(OpError::param("tab", "nothing to release in this tab"));
    }
    if let Some(list) = a.opt_array("parts")? {
        if !matches!(od.doc().tab(tab).map(|t| &t.content), Some(TabContent::PartStudio(_))) {
            return Err(OpError::param("parts", "parts of a Part Studio tab"));
        }
        let mut keep = Vec::new();
        for (i, v) in list.iter().enumerate() {
            let k = v.as_str().unwrap_or_default();
            let it = items.iter().find(|it| it.part.as_ref().is_some_and(|b| crate::refs::body_id(b) == k) || it.name == k).ok_or_else(|| OpError::param(&format!("parts[{i}]"), &format!("no part `{k}`; parts: {}", items.iter().map(|i| i.name.clone()).collect::<Vec<_>>().join(", "))))?;
            keep.push(it.clone());
        }
        items = keep;
    }
    if a.bool_or("include_drawings", false)? {
        let extra: Vec<_> = rel::associated_drawings(&od.session, &*provider, &items).into_iter().filter(|d| !items.iter().any(|i| i.same_object(d))).collect();
        items.extend(extra);
    }
    let pick = |field: &str, items: &mut Vec<noshape_model::release::ReleaseItem>, set: &dyn Fn(&mut noshape_model::release::ReleaseItem, String)| -> Result<(), OpError> {
        for (k, v) in a.object(field)?.into_iter().flatten() {
            let it = items.iter_mut().find(|it| it.part.as_ref().is_some_and(|b| crate::refs::body_id(b) == *k) || it.name == *k).ok_or_else(|| OpError::param(field, &format!("no item `{k}`")))?;
            set(it, v.as_str().ok_or_else(|| OpError::param(field, "text values"))?.trim().to_string());
        }
        Ok(())
    };
    // A part number given: that number's next revision (as the dialog
    // renumbers a row), unless a revision is given too.
    let log = rel::release_log(&od.session);
    pick("part_numbers", &mut items, &|it, v| {
        if !it.not_revision_managed {
            it.revision = log.next_revision(&v);
        }
        it.part_number = v;
    })?;
    pick("revisions", &mut items, &|it, v| it.revision = v)?;
    let name = a.str("name")?.trim().to_string();
    if name.is_empty() {
        return Err(OpError::param("name", "a release name"));
    }
    // The dialog's checks.
    let mut issues = log.validate(&items);
    for (row, held) in issues.iter_mut().zip(crate::release_conditions::condition_issues(&od.session, &items)) {
        row.extend(held);
    }
    let problems: Vec<String> = items.iter().zip(&issues).filter(|(_, v)| !v.is_empty()).map(|(it, v)| format!("{}: {}", it.name, v.iter().map(|i| i.message()).collect::<Vec<_>>().join("; "))).collect();
    if !problems.is_empty() {
        return Err(OpError::invalid(format!("the release candidate needs attention: {}", problems.join(" | "))));
    }
    let created = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let pkg = Package { id: 0, kind: PackageKind::Release, name, notes: a.opt_str("notes")?.unwrap_or("").to_string(), created, version: None, items, re_releasable: false };
    let pkg = rel::commit(&mut od.session, pkg).map_err(|e| OpError::new("release", format!("{e:#}")))?;
    Ok(json!({ "release": pkg.name, "version": pkg.version, "items": pkg.items.iter().map(item_json).collect::<Vec<_>>() }).into())
}

fn history(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let log = rel::release_log(&od.session);
    let versions = od.session.store.list_versions().unwrap_or_default();
    let version_name = |id: &Option<String>| id.as_ref().and_then(|v| versions.iter().find(|x| x.id.to_string() == *v)).map(|v| v.name.clone());
    let Some(pn) = a.opt_str("part_number")? else {
        let packages: Vec<Value> = log.packages.iter().rev().map(|p| json!({ "name": p.name, "kind": format!("{:?}", p.kind), "created": p.created, "version": version_name(&p.version), "items": p.items.iter().map(item_json).collect::<Vec<_>>() })).collect();
        return Ok(json!({ "packages": packages }).into());
    };
    let revs = log.revisions(pn);
    if revs.is_empty() {
        return Err(OpError::param("part_number", "no released revision of this part number"));
    }
    let rows: Vec<Value> = revs
        .into_iter()
        .rev()
        .map(|e| {
            let p = &log.packages[e.release];
            let item = p.items.iter().find(|i| i.part_number == pn);
            json!({
                "revision": e.revision,
                "release": p.name,
                "created": p.created,
                "version": version_name(&p.version),
                "type": item.map_or("Part", |i| i.kind.label()),
                "obsolete": e.obsoleted_by.is_some() || e.auto_obsolete,
                "obsoleted_by_later_revision": e.auto_obsolete && e.obsoleted_by.is_none(),
            })
        })
        .collect();
    Ok(json!({ "part_number": pn, "revisions": rows }).into())
}
